#[path = "tests/allocation.rs"]
mod allocation;
#[path = "tests/composed_backing.rs"]
mod composed_backing;
#[path = "tests/compute_xgmi_fixture.rs"]
pub(super) mod compute_xgmi_fixture;
#[path = "tests/compute_xgmi_transition.rs"]
mod compute_xgmi_transition;
#[path = "tests/device_backing.rs"]
mod device_backing;
#[path = "tests/device_initialization.rs"]
pub(super) mod device_initialization;
#[path = "tests/device_pool.rs"]
mod device_pool;
#[path = "tests/dispatch_retention.rs"]
mod dispatch_retention;
#[path = "tests/foundation_restore.rs"]
mod foundation_restore;
#[path = "tests/host_backing.rs"]
mod host_backing;
#[path = "tests/live_coherent_insertion.rs"]
pub(super) mod live_coherent_insertion;
#[path = "tests/live_insertion.rs"]
pub(super) mod live_insertion;
#[path = "tests/mapping_snapshot_bytes.rs"]
mod mapping_snapshot_bytes;
#[path = "tests/native_backing.rs"]
mod native_backing;
#[path = "tests/preparation.rs"]
pub(super) mod preparation;
#[path = "tests/primary_construction.rs"]
mod primary_construction;
#[path = "tests/primary_projection.rs"]
pub(super) mod primary_projection;
#[path = "tests/pristine_abort.rs"]
pub(super) mod pristine_abort;
#[path = "tests/queue_construction.rs"]
pub(super) mod queue_construction;
#[path = "tests/retained_pair_operational.rs"]
mod retained_pair_operational;
#[path = "tests/sdma_single.rs"]
mod sdma_single;
#[path = "tests/transitions.rs"]
mod transitions;
#[path = "tests/xgmi_backing.rs"]
mod xgmi_backing;
use super::*;
use core::cell::Cell;
use fe2o3_kfd_uapi::KfdIoctlAllocMemoryOfGpuArgs;
use fe2o3_runtime_model as model;
use sha2::{Digest, Sha256};

use crate::memory::KernelOutcome;

struct FakeMapping {
    address: u64,
    bytes: Vec<u8>,
    byte_offset: usize,
    active: bool,
    writable: bool,
    corrupt_readback: bool,
    readback_calls: Cell<usize>,
    panic_access: Option<&'static str>,
    sdma_bytes: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum CleanupCallV1 {
    UnmapGpu(u64, u32),
    UnmapCpu(u64, usize, usize),
    Free(u64),
    ReleaseVa(u64, usize),
}

struct FakeBackend {
    gpu_id: u32,
    opener_pid_override: Option<u32>,
    next_va: u64,
    next_handle: u64,
    flags: Vec<u32>,
    fail_operation: Option<&'static str>,
    capacity_error_operation: Option<&'static str>,
    panic_operation: Option<&'static str>,
    fixed_va: Option<u64>,
    map_progress: u32,
    unmap_progress: u32,
    map_errno: bool,
    unmap_errno: bool,
    alloc_oom: bool,
    corrupt_flags: bool,
    allocation_output_mutator: Option<fn(&mut KfdIoctlAllocMemoryOfGpuArgs)>,
    last_allocation_output: Option<KfdIoctlAllocMemoryOfGpuArgs>,
    corrupt_mapping_address: bool,
    currentness_calls: usize,
    fail_currentness_at: Option<usize>,
    panic_currentness_at: Option<usize>,
    quarantine_request_at: Option<(usize, crate::Gfx942RetainedRequestV1)>,
    operational_currentness_calls: usize,
    fail_operational_currentness_at: Option<usize>,
    panic_operational_currentness_at: Option<usize>,
    reserve_va_calls: usize,
    alloc_calls: usize,
    map_cpu_calls: usize,
    map_cpu_inputs: Vec<((u64, usize), u64, usize)>,
    map_gpu_calls: usize,
    map_gpu_inputs: Vec<(u64, u32)>,
    unmap_gpu_calls: usize,
    cleanup_calls: Vec<CleanupCallV1>,
    cleanup_fault: Option<(usize, &'static str, bool)>,
    unmap_outcome_at: Option<(usize, u32, bool)>,
    multi_map_script: Vec<(u32, bool)>,
    multi_unmap_script: Vec<(u32, bool)>,
    panic_multi_map_at: Option<usize>,
    panic_multi_unmap_at: Option<usize>,
    multi_map_inputs: Vec<(Vec<u32>, u32)>,
    multi_unmap_inputs: Vec<(Vec<u32>, u32)>,
    free_calls: usize,
    release_va_calls: usize,
    corrupt_readback: bool,
    last_unmapped_bytes: Option<Vec<u8>>,
    last_unmapped_readback_calls: usize,
    operations: Vec<&'static str>,
    last_userptr_input: Option<(u64, u64, u64)>,
    userptr_mmap_offset: Option<u64>,
}

impl FakeBackend {
    fn good() -> Self {
        Self {
            gpu_id: 7,
            opener_pid_override: None,
            next_va: 0x2_0000,
            next_handle: 1,
            flags: Vec::new(),
            fail_operation: None,
            capacity_error_operation: None,
            panic_operation: None,
            fixed_va: None,
            map_progress: 1,
            unmap_progress: 1,
            map_errno: false,
            unmap_errno: false,
            alloc_oom: false,
            corrupt_flags: false,
            allocation_output_mutator: None,
            last_allocation_output: None,
            corrupt_mapping_address: false,
            currentness_calls: 0,
            fail_currentness_at: None,
            panic_currentness_at: None,
            quarantine_request_at: None,
            operational_currentness_calls: 0,
            fail_operational_currentness_at: None,
            panic_operational_currentness_at: None,
            reserve_va_calls: 0,
            alloc_calls: 0,
            map_cpu_calls: 0,
            map_cpu_inputs: Vec::new(),
            map_gpu_calls: 0,
            map_gpu_inputs: Vec::new(),
            unmap_gpu_calls: 0,
            cleanup_calls: Vec::new(),
            cleanup_fault: None,
            unmap_outcome_at: None,
            multi_map_script: Vec::new(),
            multi_unmap_script: Vec::new(),
            panic_multi_map_at: None,
            panic_multi_unmap_at: None,
            multi_map_inputs: Vec::new(),
            multi_unmap_inputs: Vec::new(),
            free_calls: 0,
            release_va_calls: 0,
            corrupt_readback: false,
            last_unmapped_bytes: None,
            last_unmapped_readback_calls: 0,
            operations: Vec::new(),
            last_userptr_input: None,
            userptr_mmap_offset: None,
        }
    }

    fn check(&self, operation: &'static str) -> Result<(), MemorySessionError> {
        if self.capacity_error_operation == Some(operation) {
            return Err(MemorySessionError::DeviceBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity,
            ));
        }
        if let Some((at, selected, panic)) = self.cleanup_fault
            && at == self.cleanup_calls.len()
            && selected == operation
        {
            if panic {
                std::panic::panic_any(("N2 native panic", operation));
            }
            return Err(MemorySessionError::Injected(operation));
        }
        if self.panic_operation == Some(operation) {
            std::panic::panic_any(("N2 native panic", operation));
        }
        if self.fail_operation == Some(operation) {
            Err(MemorySessionError::Injected(operation))
        } else {
            Ok(())
        }
    }
}

macro_rules! fake_host_scope_operation {
    ($name:ident($($argument:ident: $type:ty),*) -> $result:ty, $error:literal) => {
        fn $name(
            mapping: &mut Self::Mapping,
            _requested_bytes: usize,
            $($argument: $type),*
        ) -> Result<$result, MemorySessionError> {
            if mapping.panic_access == Some(stringify!($name)) {
                std::panic::panic_any(("N1 mapped panic", stringify!($name)));
            }
            Err(MemorySessionError::KernelResultMalformed($error))
        }
    };
}

fn fake_sdma_bytes<'a>(
    mapping: &'a mut FakeMapping,
    requested: usize,
    operation: &'static str,
    unsupported: &'static str,
) -> Result<&'a mut [u8], MemorySessionError> {
    if mapping.panic_access == Some(operation) {
        std::panic::panic_any(("N1 mapped panic", operation));
    }
    if !mapping.sdma_bytes {
        return Err(MemorySessionError::KernelResultMalformed(unsupported));
    }
    if !mapping.active || !mapping.writable {
        return Err(MemorySessionError::KernelResultMalformed(
            "fake SDMA mapping state",
        ));
    }
    let end = mapping
        .byte_offset
        .checked_add(requested)
        .ok_or(MemorySessionError::SizeOverflow)?;
    mapping.bytes.get_mut(mapping.byte_offset..end).ok_or(
        MemorySessionError::KernelResultMalformed("fake SDMA mapping extent"),
    )
}

impl MemoryBackend for FakeBackend {
    type Reservation = (u64, usize);
    type Mapping = FakeMapping;

    fn opener_pid(&self) -> u32 {
        self.opener_pid_override.unwrap_or_else(std::process::id)
    }
    fn gpu_id(&self) -> u32 {
        self.gpu_id
    }
    fn gpuvm_aperture(&self) -> crate::InclusiveAperture {
        crate::InclusiveAperture::from_checked_parts_for_memory_tests(0x1_0000, 0x1_0000_0000_0000)
    }
    fn page_size(&self) -> usize {
        4096
    }
    fn check_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.currentness_calls += 1;
        if self
            .quarantine_request_at
            .as_ref()
            .is_some_and(|(at, _)| *at == self.currentness_calls)
        {
            self.quarantine_request_at.take().unwrap().1.quarantine();
        }
        if self.panic_currentness_at == Some(self.currentness_calls) {
            std::panic::panic_any(("N2 native panic", "currentness"));
        }
        if self.fail_currentness_at == Some(self.currentness_calls) {
            Err(MemorySessionError::Injected("currentness"))
        } else {
            self.check("currentness")
        }
    }
    fn check_operational_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.operational_currentness_calls += 1;
        if self.panic_operational_currentness_at == Some(self.operational_currentness_calls) {
            std::panic::panic_any(("N1 native panic", "operational_currentness"));
        }
        if self.fail_operational_currentness_at == Some(self.operational_currentness_calls) {
            Err(MemorySessionError::Injected("operational_currentness"))
        } else {
            self.check("operational_currentness")
        }
    }
    fn acquire_vm(&mut self) -> Result<(), MemorySessionError> {
        self.check("acquire_vm")
    }
    fn reserve_va(&mut self, bytes: usize) -> Result<Self::Reservation, MemorySessionError> {
        self.reserve_va_calls += 1;
        self.check("reserve_va")?;
        let address = self.fixed_va.unwrap_or(self.next_va);
        self.next_va = self
            .next_va
            .checked_add(bytes as u64)
            .and_then(|value| value.checked_add(4096))
            .unwrap();
        Ok((address, bytes))
    }
    fn reservation_address(reservation: &Self::Reservation) -> u64 {
        reservation.0
    }
    fn alloc(
        &mut self,
        va: u64,
        bytes: u64,
        flags: KfdAllocMemoryFlags,
    ) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs> {
        self.alloc_calls += 1;
        self.flags.push(flags.bits());
        let handle = self.next_handle;
        self.next_handle += 1;
        let mut args = KfdIoctlAllocMemoryOfGpuArgs::new(va, bytes, self.gpu_id, flags);
        args.handle = handle;
        args.mmap_offset = 0x40_000 + handle * 4096;
        if self.corrupt_flags {
            args.flags ^= 1;
        }
        if let Some(mutate) = self.allocation_output_mutator {
            mutate(&mut args);
        }
        self.last_allocation_output = Some(args);
        KernelOutcome {
            value: args,
            result: if self.alloc_oom {
                Err(MemorySessionError::Syscall {
                    operation: "AMDKFD_IOC_ALLOC_MEMORY_OF_GPU",
                    source: rustix::io::Errno::NOMEM,
                })
            } else {
                self.check("alloc")
            },
        }
    }
    fn prepare_userptr(
        &mut self,
        reservation: &mut Self::Reservation,
        bytes: usize,
    ) -> Result<Self::Mapping, MemorySessionError> {
        self.operations.push("prepare_userptr");
        self.check("prepare_userptr")?;
        if reservation.1 != bytes {
            return Err(MemorySessionError::KernelResultMalformed(
                "fake USERPTR reservation geometry",
            ));
        }
        let mut mapping = FakeMapping {
            address: reservation.0,
            bytes: vec![0; bytes],
            byte_offset: 0,
            active: true,
            writable: false,
            corrupt_readback: self.corrupt_readback,
            readback_calls: Cell::new(0),
            panic_access: self.panic_operation,
            sdma_bytes: false,
        };
        self.prepare_cpu_mapping(&mut mapping)?;
        Ok(mapping)
    }
    fn alloc_userptr(
        &mut self,
        address: u64,
        bytes: u64,
        flags: KfdAllocMemoryFlags,
    ) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs> {
        self.alloc_calls += 1;
        self.operations.push("alloc_userptr");
        self.flags.push(flags.bits());
        self.last_userptr_input = Some((address, address, bytes));
        let handle = self.next_handle;
        self.next_handle += 1;
        let mut args = if flags == KfdAllocMemoryFlags::USERPTR_EXECUTABLE {
            KfdIoctlAllocMemoryOfGpuArgs::new_userptr(address, bytes, self.gpu_id)
        } else if flags == KfdAllocMemoryFlags::USERPTR_QUEUE_CONTROL {
            KfdIoctlAllocMemoryOfGpuArgs::new_userptr_queue_control(address, bytes, self.gpu_id)
        } else {
            KfdIoctlAllocMemoryOfGpuArgs::new(address, bytes, self.gpu_id, flags)
        };
        args.handle = handle;
        // KFD overwrites the input CPU pointer with an opaque BO offset.
        args.mmap_offset = self.userptr_mmap_offset.unwrap_or(0x90_000 + handle * 4096);
        if self.corrupt_flags {
            args.flags ^= 1;
        }
        if let Some(mutate) = self.allocation_output_mutator {
            mutate(&mut args);
        }
        self.last_allocation_output = Some(args);
        KernelOutcome {
            value: args,
            result: if self.alloc_oom {
                Err(MemorySessionError::Syscall {
                    operation: "AMDKFD_IOC_ALLOC_MEMORY_OF_GPU(USERPTR)",
                    source: rustix::io::Errno::NOMEM,
                })
            } else {
                self.check("alloc_userptr")
            },
        }
    }
    fn map_cpu(
        &mut self,
        reservation: &mut Self::Reservation,
        mmap_offset: u64,
        bytes: usize,
    ) -> Result<Self::Mapping, MemorySessionError> {
        self.map_cpu_calls += 1;
        self.map_cpu_inputs.push((*reservation, mmap_offset, bytes));
        self.operations.push("map_cpu");
        self.check("map_cpu")?;
        Ok(FakeMapping {
            address: if self.corrupt_mapping_address {
                reservation.0 + 4096
            } else {
                reservation.0
            },
            bytes: vec![0; bytes],
            byte_offset: 0,
            active: true,
            writable: false,
            corrupt_readback: self.corrupt_readback,
            readback_calls: Cell::new(0),
            panic_access: self.panic_operation,
            sdma_bytes: false,
        })
    }
    fn mapping_address(mapping: &Self::Mapping) -> u64 {
        mapping.address
    }
    fn prepare_cpu_mapping(
        &mut self,
        mapping: &mut Self::Mapping,
    ) -> Result<(), MemorySessionError> {
        self.operations.push("prepare_cpu_mapping");
        self.check("prepare_cpu_mapping")?;
        mapping.writable = true;
        Ok(())
    }
    fn protect_cpu_read_only(
        &mut self,
        mapping: &mut Self::Mapping,
    ) -> Result<(), MemorySessionError> {
        self.check("protect_cpu_read_only")?;
        mapping.writable = false;
        Ok(())
    }
    fn map_gpu(&mut self, handle: u64, old_success: u32) -> KernelOutcome<u32> {
        self.map_gpu_calls += 1;
        self.map_gpu_inputs.push((handle, old_success));
        self.operations.push("map_gpu");
        KernelOutcome {
            value: self.map_progress,
            result: if self.map_errno {
                Err(MemorySessionError::Injected("map_gpu"))
            } else {
                self.check("map_gpu")
            },
        }
    }
    fn unmap_gpu(&mut self, handle: u64, old_success: u32) -> KernelOutcome<u32> {
        self.unmap_gpu_calls += 1;
        self.cleanup_calls
            .push(CleanupCallV1::UnmapGpu(handle, old_success));
        self.operations.push("unmap_gpu");
        let (progress, errno) = self
            .unmap_outcome_at
            .filter(|(at, _, _)| *at == self.unmap_gpu_calls)
            .map(|(_, progress, errno)| (progress, errno))
            .unwrap_or((self.unmap_progress, self.unmap_errno));
        KernelOutcome {
            value: progress,
            result: if errno {
                Err(MemorySessionError::Injected("unmap_gpu"))
            } else {
                self.check("unmap_gpu")
            },
        }
    }
    fn map_gpu_ids(
        &mut self,
        _handle: u64,
        gpu_ids: &[u32],
        old_success: u32,
    ) -> KernelOutcome<u32> {
        let call = self.multi_map_inputs.len();
        self.multi_map_inputs.push((gpu_ids.to_vec(), old_success));
        if self.panic_multi_map_at == Some(call + 1) {
            std::panic::panic_any(("N2 native panic", "map_gpu_ids"));
        }
        let (value, errno) = self
            .multi_map_script
            .get(call)
            .copied()
            .unwrap_or((gpu_ids.len() as u32, false));
        KernelOutcome {
            value,
            result: if errno {
                Err(MemorySessionError::Injected("multi_map_gpu"))
            } else {
                Ok(())
            },
        }
    }
    fn unmap_gpu_ids(
        &mut self,
        _handle: u64,
        gpu_ids: &[u32],
        old_success: u32,
    ) -> KernelOutcome<u32> {
        let call = self.multi_unmap_inputs.len();
        self.multi_unmap_inputs
            .push((gpu_ids.to_vec(), old_success));
        if self.panic_multi_unmap_at == Some(call + 1) {
            std::panic::panic_any(("N2 native panic", "unmap_gpu_ids"));
        }
        let (value, errno) = self
            .multi_unmap_script
            .get(call)
            .copied()
            .unwrap_or((gpu_ids.len() as u32, false));
        KernelOutcome {
            value,
            result: if errno {
                Err(MemorySessionError::Injected("multi_unmap_gpu"))
            } else {
                Ok(())
            },
        }
    }
    fn with_bytes<R>(
        mapping: &Self::Mapping,
        requested_bytes: usize,
        f: impl FnOnce(&[u8]) -> R,
    ) -> R {
        if mapping.panic_access == Some("with_bytes") {
            std::panic::panic_any(("N2 native panic", "with_bytes"));
        }
        assert!(mapping.active);
        mapping.readback_calls.set(mapping.readback_calls.get() + 1);
        if mapping.corrupt_readback {
            let mut corrupted =
                mapping.bytes[mapping.byte_offset..mapping.byte_offset + requested_bytes].to_vec();
            corrupted[0] ^= 1;
            f(&corrupted)
        } else {
            f(&mapping.bytes[mapping.byte_offset..mapping.byte_offset + requested_bytes])
        }
    }
    fn with_bytes_mut<R>(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> R {
        if mapping.panic_access == Some("with_bytes_mut") {
            std::panic::panic_any(("N2 native panic", "with_bytes_mut"));
        }
        assert!(mapping.active && mapping.writable);
        f(&mut mapping.bytes[mapping.byte_offset..mapping.byte_offset + requested_bytes])
    }
    fn observe_i64_acquire(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        offset: usize,
    ) -> Result<i64, MemorySessionError> {
        if mapping.panic_access == Some("observe_i64_acquire") {
            std::panic::panic_any(("N1 mapped panic", "observe_i64_acquire"));
        }
        let end = offset.checked_add(core::mem::size_of::<i64>()).ok_or(
            MemorySessionError::KernelResultMalformed("fake acquired i64 range"),
        )?;
        let bytes: [u8; 8] = mapping
            .bytes
            .get(mapping.byte_offset..)
            .and_then(|bytes| bytes.get(offset..end.min(requested_bytes)))
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(MemorySessionError::KernelResultMalformed(
                "fake acquired i64 range",
            ))?;
        Ok(i64::from_le_bytes(bytes))
    }
    fn observe_aql_counters(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
    ) -> Result<(u64, u64), MemorySessionError> {
        let bytes = fake_sdma_bytes(
            mapping,
            requested_bytes,
            "observe_aql_counters",
            "AQL mapped counter backend",
        )?;
        let read = |offset: usize| -> Result<u64, MemorySessionError> {
            let value: [u8; 8] = bytes
                .get(offset..offset + 8)
                .and_then(|v| v.try_into().ok())
                .ok_or(MemorySessionError::KernelResultMalformed(
                    "fake SDMA counter range",
                ))?;
            Ok(u64::from_le_bytes(value))
        };
        Ok((
            read(crate::queue_resources::AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1)?,
            read(crate::queue_resources::AMD_AQL_READ_DISPATCH_ID_OFFSET_V1)?,
        ))
    }
    fake_host_scope_operation!(fetch_add_aql_write(_increment: u64) -> u64, "AQL mapped write backend");
    fn publish_sdma_write_release(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        expected: u64,
        new: u64,
    ) -> Result<(), MemorySessionError> {
        if new <= expected {
            return Err(MemorySessionError::KernelResultMalformed(
                "SDMA write-pointer progression",
            ));
        }
        let bytes = fake_sdma_bytes(
            mapping,
            requested_bytes,
            "publish_sdma_write_release",
            "SDMA visible write-pointer backend",
        )?;
        let offset = crate::queue_resources::AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1;
        let target =
            bytes
                .get_mut(offset..offset + 8)
                .ok_or(MemorySessionError::KernelResultMalformed(
                    "fake SDMA write range",
                ))?;
        if target != expected.to_le_bytes() {
            return Err(MemorySessionError::KernelResultMalformed(
                "fake SDMA expected write",
            ));
        }
        target.copy_from_slice(&new.to_le_bytes());
        Ok(())
    }
    fn write_sdma_slot(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        slot_index: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        let bytes = fake_sdma_bytes(
            mapping,
            requested_bytes,
            "write_sdma_slot",
            "SDMA mapped slot backend",
        )?;
        let start = usize::try_from(slot_index)
            .ok()
            .and_then(|slot| slot.checked_mul(64))
            .ok_or(MemorySessionError::SizeOverflow)?;
        let end = start
            .checked_add(64)
            .ok_or(MemorySessionError::SizeOverflow)?;
        bytes
            .get_mut(start..end)
            .ok_or(MemorySessionError::KernelResultMalformed(
                "fake SDMA packet range",
            ))?
            .copy_from_slice(packet);
        Ok(())
    }
    fake_host_scope_operation!(write_aql_slot(_slot_index: u32, _packet: &[u8; 64]) -> (), "AQL mapped slot backend");
    fake_host_scope_operation!(publish_aql_header(_slot_index: u32, _header: u16) -> (), "AQL mapped publication backend");
    fake_host_scope_operation!(observe_aql_packet_header_acquire(_packet_id: u64) -> (u32, u16, u16), "AQL packet observation backend");
    fake_host_scope_operation!(observe_completion_signal_acquire(_slot_index: u32) -> fe2o3_aql::AqlCompletionObservationV1, "AQL completion observation backend");
    fake_host_scope_operation!(observe_completion_signal_state_acquire(_slot_index: u32) -> (i64, i64), "AQL completion state observation backend");
    fake_host_scope_operation!(reset_completion_signal_release(_slot_index: u32) -> (), "AQL completion reset backend");
    fn unmap_cpu(&mut self, mapping: &mut Self::Mapping) -> Result<(), MemorySessionError> {
        self.cleanup_calls.push(CleanupCallV1::UnmapCpu(
            mapping.address,
            mapping.bytes.as_ptr() as usize,
            mapping.bytes.len(),
        ));
        self.operations.push("unmap_cpu");
        self.check("unmap_cpu")?;
        self.last_unmapped_bytes = Some(mapping.bytes.clone());
        self.last_unmapped_readback_calls = mapping.readback_calls.get();
        mapping.active = false;
        Ok(())
    }
    fn release_va_reservation(
        &mut self,
        reservation: &mut Self::Reservation,
    ) -> Result<(), MemorySessionError> {
        self.release_va_calls += 1;
        self.cleanup_calls
            .push(CleanupCallV1::ReleaseVa(reservation.0, reservation.1));
        self.check("release_va_reservation")
    }
    fn free(&mut self, handle: u64) -> Result<(), MemorySessionError> {
        self.free_calls += 1;
        self.cleanup_calls.push(CleanupCallV1::Free(handle));
        self.operations.push("free");
        self.check("free")
    }
}

fn acquired() -> SharedMemoryEngine<FakeBackend> {
    SharedMemoryEngine::acquire(FakeBackend::good()).unwrap()
}

fn device_vm(generation: u64) -> (DeviceKeyV1, VmKeyV1) {
    let device = DeviceKeyV1 {
        physical: fe2o3_runtime_model::PhysicalDeviceIdV1(9),
        generation: fe2o3_runtime_model::DeviceGenerationV1(generation),
    };
    (
        device,
        VmKeyV1 {
            device,
            id: VmIdV1(11),
        },
    )
}

fn model_digest(seed: u8) -> model::IdentityDigestV1 {
    model::IdentityDigestV1::from_untrusted_bytes([seed; model::IDENTITY_DIGEST_BYTES_V1])
}

fn model_domain() -> model::DeviceObservationDomainIdV1 {
    model::DeviceObservationDomainIdV1::from_untrusted_digest(model_digest(1))
}

fn model_correlation() -> model::ModelCorrelatedDeviceV1 {
    model_correlation_for_gpu(7)
}

fn model_correlation_for_gpu(gpu_id: u32) -> model::ModelCorrelatedDeviceV1 {
    let domain_id = model_domain();
    let epoch = model::ObservationEpochV1(9);
    let node = match gpu_id {
        7 => 1,
        1001 => 2,
        1002 => 3,
        _ => panic!("unknown fixture GPU"),
    };
    let pci = model::PciAddressV1 {
        domain: 0,
        bus: 1,
        device: node as u8,
        function: 0,
    };
    let profile = model::DeviceAdmissionProfileV1::gfx942_xnack_minus_spx_nps1_kfd_1_18_drm_3_64_0(
        model::DeviceAdmissionProfileIdV1::from_untrusted_digest(model_digest(2)),
        model_digest(3),
        model_digest(4),
    );
    model::UntrustedDeviceInventoryV1::from_untrusted_observations(
        model::UntrustedKfdObservationV1 {
            domain_id,
            epoch,
            node: model::DeviceNodeV1 {
                major: 511,
                minor: model::KFD_DEVICE_MINOR_V1,
            },
            uapi_major: model::KFD_UAPI_MAJOR_V1,
            uapi_minor: model::KFD_UAPI_MINOR_V1,
            schema_identity: model_digest(3),
            xnack: model::XnackObservationV1::Disabled,
        },
        vec![model::UntrustedTopologyObservationV1 {
            domain_id,
            epoch,
            topology_node_id: node,
            kfd_gpu_id: gpu_id,
            gpu_unique_id: 94 + u64::from(gpu_id),
            drm_render_minor: model::DRM_RENDER_MIN_MINOR_V1 + node,
            pci,
            vendor_id: model::AMD_PCI_VENDOR_ID_V1,
            device_id: model::MI300X_PCI_DEVICE_ID_V1,
            target: model::GpuTargetObservationV1::Gfx942,
            compute_partition: model::ComputePartitionObservationV1::Spx,
            memory_partition: model::MemoryPartitionObservationV1::Nps1,
        }],
        vec![model::UntrustedRenderObservationV1 {
            domain_id,
            epoch,
            node: model::DeviceNodeV1 {
                major: model::DRM_DEVICE_MAJOR_V1,
                minor: model::DRM_RENDER_MIN_MINOR_V1 + node,
            },
            gpu_unique_id: 94 + u64::from(gpu_id),
            pci,
            vendor_id: model::AMD_PCI_VENDOR_ID_V1,
            device_id: model::MI300X_PCI_DEVICE_ID_V1,
            pci_revision_id: 0,
            drm_schema_identity: model_digest(4),
            driver_name: model::DrmDriverNameObservationV1::Amdgpu,
            drm_major: model::DRM_DRIVER_MAJOR_V1,
            drm_minor: model::DRM_DRIVER_MINOR_V1,
            drm_patch: model::DRM_DRIVER_PATCH_V1,
            acceleration_working: true,
            family: model::DrmFamilyObservationV1::AmdgpuFamilyAi,
        }],
    )
    .unwrap()
    .correlate_model_only(&profile)
    .unwrap()
}

fn transferred_model_foundation() -> (
    model::DeviceIdentityStateV1,
    MemoryLifecycleStateV1,
    ModelDeviceAdmissionV1,
    VmKeyV1,
) {
    transferred_model_foundation_with_aperture(0x20_0000)
}

fn transferred_model_foundation_with_aperture(
    byte_len: u64,
) -> (
    model::DeviceIdentityStateV1,
    MemoryLifecycleStateV1,
    ModelDeviceAdmissionV1,
    VmKeyV1,
) {
    transferred_model_foundation_with_correlation(byte_len, model_correlation())
}

fn transferred_model_foundation_with_correlation(
    byte_len: u64,
    correlation: model::ModelCorrelatedDeviceV1,
) -> (
    model::DeviceIdentityStateV1,
    MemoryLifecycleStateV1,
    ModelDeviceAdmissionV1,
    VmKeyV1,
) {
    let domain_id = model_domain();
    let (identity, device) = model::DeviceIdentityStateV1::new(domain_id)
        .register_device_model_only(correlation, model::DeviceGenerationV1(1))
        .unwrap();
    let correlated = device.correlation();
    let (identity, vm) = identity
        .register_vm_model_only(
            device,
            model::UntrustedVmObservationV1 {
                domain_id,
                device: device.model_key(),
                vm_id: VmIdV1(1),
                kfd_gpu_id: correlated.kfd_gpu_id(),
                render_node: correlated.render_node(),
                pci: correlated.identity().pci,
            },
        )
        .unwrap();
    let memory = MemoryLifecycleStateV1::new_monotonic_non_reusable(domain_id)
        .next(MemoryTransitionV1::AcquireVm {
            admission: vm,
            mapping_devices: vec![device],
            handle: UntrustedVmHandleObservationV1(1),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len,
            },
        })
        .unwrap();
    (identity, memory, device, vm.model_key())
}

struct BackingConstructorFixture {
    engine: SharedMemoryEngine<FakeBackend>,
    ownership: QueueModelOwnershipV1,
    foundation: QueueModelFoundationV1,
    device: ModelDeviceAdmissionV1,
    vm: VmKeyV1,
}

impl BackingConstructorFixture {
    fn new(budget: Option<Gfx942DeviceBackingBudgetV1>) -> Self {
        Self::with_aperture(budget, 0x20_0000)
    }

    fn with_aperture(budget: Option<Gfx942DeviceBackingBudgetV1>, bytes: u64) -> Self {
        let (identity, memory, device, vm) = transferred_model_foundation_with_aperture(bytes);
        let mut fixture = Self {
            engine: acquired(),
            ownership: QueueModelOwnershipV1::new(),
            foundation: QueueModelFoundationV1::uncertified(identity, memory),
            device,
            vm,
        };
        fixture.configure(budget).unwrap();
        fixture
    }

    fn configure(
        &mut self,
        budget: Option<Gfx942DeviceBackingBudgetV1>,
    ) -> Result<(), MemorySessionError> {
        self.ownership.configure_optional_device_backing_budget(
            &mut self.engine,
            self.device,
            self.vm,
            budget,
        )
    }

    fn transfer(
        &mut self,
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<QueueModelFoundationV1, MemorySessionError> {
        self.engine.validate_complete_dispatch_device_memory_set(
            authorities,
            self.device.model_key(),
            self.vm,
        )?;
        self.ownership
            .take_foundation(&mut self.engine, &mut self.foundation, self.device, self.vm)
    }

    fn mapped_device(&mut self) -> Gfx942DeviceMemoryDispatchAuthorityV1 {
        let lease = self
            .engine
            .allocate_device_memory(self.device.model_key(), self.vm, 17, 4)
            .and_then(|lease| self.engine.map_device_memory(lease))
            .unwrap();
        let record = &self.engine.device_memory[0];
        Gfx942DeviceMemoryDispatchAuthorityV1 {
            facts: Gfx942DeviceMemoryDispatchFactsV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
                gpu_va: record.gpu_va,
                layout: record.layout,
            },
            lease,
        }
    }

    fn usage(&self) -> Option<Gfx942DeviceBackingUsageV1> {
        self.engine
            .device_backing_account
            .as_ref()
            .map(DeviceBackingAccountV1::usage)
    }
}

fn allocate_public_device_memory(
    engine: &mut SharedMemoryEngine<FakeBackend>,
) -> Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryUnmappedV1> {
    let (device, vm) = device_vm(7);
    engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            4096,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap()
}

fn content(bytes: &[u8]) -> Gfx942DeviceContentDescriptorV1 {
    let role = crate::Gfx942DeviceContentRoleV1::new([0x51; 32], 7).unwrap();
    Gfx942DeviceContentDescriptorV1::from_bytes(role, bytes).unwrap()
}

fn repeated_content(byte_len: u64, repeated_byte: u8) -> Gfx942RepeatedByteContentV1 {
    let role = crate::Gfx942DeviceContentRoleV1::new([0x62; 32], 8).unwrap();
    Gfx942RepeatedByteContentV1::new(role, byte_len, repeated_byte).unwrap()
}

#[path = "tests/coherent_access_tests.rs"]
mod coherent_access_tests;
#[path = "tests/identity_tests.rs"]
mod identity_tests;
#[path = "tests/mapping_failure_tests.rs"]
mod mapping_failure_tests;
#[path = "tests/substitution_tests.rs"]
mod substitution_tests;

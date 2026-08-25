//! Minimal unsafe Linux boundary for the owned memory transaction.

use core::ffi::c_void;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd};

use fe2o3_aql::{
    AMD_SIGNAL_BYTES_V1, AMD_SIGNAL_VALUE_PENDING_V1, AQL_INVALID_PACKET_HEADER_V1,
    AQL_KERNEL_DISPATCH_PACKET_BYTES_V1, AqlCompletionObservationV1, AqlRingCapacityV1,
    classify_acquired_completion_value_v1, is_reviewed_aql_publication_v1,
};
use fe2o3_kfd_uapi::{
    AMDKFD_IOC_ACQUIRE_VM, AMDKFD_IOC_ALLOC_MEMORY_OF_GPU, AMDKFD_IOC_FREE_MEMORY_OF_GPU,
    AMDKFD_IOC_MAP_MEMORY_TO_GPU, AMDKFD_IOC_UNMAP_MEMORY_FROM_GPU, KfdAllocMemoryFlags,
    KfdIoctlAcquireVmArgs, KfdIoctlAllocMemoryOfGpuArgs, KfdIoctlFreeMemoryOfGpuArgs,
    KfdIoctlMapMemoryToGpuArgs, KfdIoctlUnmapMemoryFromGpuArgs,
};
use fe2o3_runtime_model::{
    DeviceIdentityStateV1, ModelDeviceAdmissionV1, ModelVmAdmissionV1, VmIdV1,
};
use rustix::ioctl::{Opcode, Setter, Updater};
use rustix::mm::{Advice, MapFlags, MprotectFlags, ProtFlags};

use super::memory::{KernelOutcome, MemoryBackend, MemorySessionError};
use super::queue_resources::{
    AMD_AQL_READ_DISPATCH_ID_OFFSET_V1, AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1,
};
use crate::{CheckedGfx942XnackMinusDevice, InclusiveAperture};

const ACQUIRE_VM_OPCODE: Opcode = AMDKFD_IOC_ACQUIRE_VM as Opcode;
const ALLOC_MEMORY_OPCODE: Opcode = AMDKFD_IOC_ALLOC_MEMORY_OF_GPU as Opcode;
const FREE_MEMORY_OPCODE: Opcode = AMDKFD_IOC_FREE_MEMORY_OF_GPU as Opcode;
const MAP_MEMORY_OPCODE: Opcode = AMDKFD_IOC_MAP_MEMORY_TO_GPU as Opcode;
const UNMAP_MEMORY_OPCODE: Opcode = AMDKFD_IOC_UNMAP_MEMORY_FROM_GPU as Opcode;
#[cfg(feature = "live-validation")]
const LINUX_ENOMEM: i32 = 12;

#[cfg(feature = "live-validation")]
unsafe extern "C" {
    fn mincore(address: *mut c_void, length: usize, residency: *mut u8) -> i32;
}

pub(super) struct LinuxMemoryBackend {
    device: CheckedGfx942XnackMinusDevice,
}

pub(super) struct LinuxVaReservation {
    address: NonNull<c_void>,
    bytes: usize,
    replaced: bool,
}

pub(super) struct LinuxCpuMapping {
    address: NonNull<c_void>,
    bytes: usize,
    active: bool,
    accessible: bool,
}

impl LinuxMemoryBackend {
    pub(super) fn new(device: CheckedGfx942XnackMinusDevice) -> Self {
        Self { device }
    }

    pub(super) fn bind_model_vm(
        &mut self,
        vm_id: VmIdV1,
    ) -> Result<(DeviceIdentityStateV1, ModelVmAdmissionV1), MemorySessionError> {
        self.device
            .register_memory_vm_model_only(vm_id)
            .map_err(MemorySessionError::Device)
    }

    pub(super) fn kfd_fd(&self) -> BorrowedFd<'_> {
        self.device.kfd.opened.fd.as_fd()
    }

    pub(super) fn model_device(&self) -> ModelDeviceAdmissionV1 {
        self.device.model_admission()
    }

    pub(super) fn model_aperture(&self) -> InclusiveAperture {
        self.device.observation().aperture().gpuvm()
    }

    pub(super) fn plan_aql_queue_resources(
        &self,
        ring_bytes: u32,
    ) -> Result<crate::Gfx942AqlQueueResourcePlanV1, crate::Gfx942QueueResourcePlanningError> {
        crate::plan_gfx942_aql_queue_resources(
            self.device.topology_snapshot(),
            self.device.observation().unique_id(),
            ring_bytes,
        )
    }

    fn discard_unprepared_mapping_or_abort(mapping: &mut LinuxCpuMapping) {
        if !mapping.active {
            return;
        }
        // SAFETY: no readable/writable access has been enabled and no slice has
        // been formed. Returning an ambiguously inheritable VMA would violate
        // the safe API contract, so failed synchronous cleanup is fail-stop.
        if unsafe { rustix::mm::munmap(mapping.address.as_ptr(), mapping.bytes) }.is_err() {
            std::process::abort();
        }
        mapping.active = false;
        mapping.accessible = false;
    }

    #[cfg(feature = "live-validation")]
    pub(super) fn verify_dontfork_child_negative(
        &self,
        mapping: &LinuxCpuMapping,
    ) -> Result<(), MemorySessionError> {
        let tasks = std::fs::read_dir("/proc/self/task")
            .map_err(|_| MemorySessionError::ChildProbe("read /proc/self/task"))?
            .take(2)
            .count();
        if tasks != 1 {
            return Err(MemorySessionError::IsolationRequired);
        }
        let mut residency = vec![
            0_u8;
            mapping.bytes.div_ceil(
                super::memory::HOST_VISIBLE_MEMORY_PAGE_BYTES_V1 as usize
            )
        ];
        // SAFETY: this probe admits fork only after observing exactly one task,
        // holds no user-visible mapping borrow, and performs only mincore then
        // exit_group in the child. The parent synchronously waits.
        match unsafe { rustix::runtime::kernel_fork() }
            .map_err(|source| Self::syscall("fork DONTFORK child probe", source))?
        {
            rustix::runtime::Fork::Child(_) => {
                // SAFETY: mincore treats the address as an integer range and
                // reports ENOMEM for a DONTFORK-removed VMA; it does not
                // dereference the absent userspace mapping.
                let result = unsafe {
                    mincore(
                        mapping.address.as_ptr(),
                        mapping.bytes,
                        residency.as_mut_ptr(),
                    )
                };
                let code = if result == -1
                    && std::io::Error::last_os_error().raw_os_error() == Some(LINUX_ENOMEM)
                {
                    0
                } else if result == 0 {
                    1
                } else {
                    2
                };
                rustix::runtime::exit_group(code);
            }
            rustix::runtime::Fork::ParentOf(child) => {
                let (_, status) =
                    rustix::process::waitpid(Some(child), rustix::process::WaitOptions::empty())
                        .map_err(|source| Self::syscall("wait DONTFORK child probe", source))?
                        .ok_or(MemorySessionError::ChildProbe("child was not waitable"))?;
                match status.exit_status() {
                    Some(0) => Ok(()),
                    Some(1) => Err(MemorySessionError::DontForkMappingInherited),
                    _ => Err(MemorySessionError::ChildProbe("child mincore protocol")),
                }
            }
        }
    }

    fn syscall(operation: &'static str, source: rustix::io::Errno) -> MemorySessionError {
        MemorySessionError::Syscall { operation, source }
    }

    fn exact_progress(
        operation: &'static str,
        handle: u64,
        gpu_id: u32,
        old_success: u32,
        unmap: bool,
        kfd: &rustix::fd::OwnedFd,
    ) -> KernelOutcome<u32> {
        let device_ids = [gpu_id];
        let pointer = device_ids.as_ptr() as usize as u64;
        if unmap {
            let mut args = KfdIoctlUnmapMemoryFromGpuArgs::retry(
                handle,
                pointer,
                device_ids.len() as u32,
                old_success,
            );
            // SAFETY: the opcode and LP64 layout are frozen by the KFD 1.18
            // oracle. `device_ids` and the initialized in/out record remain
            // live and immutably located for the complete call.
            let request = unsafe { Updater::<UNMAP_MEMORY_OPCODE, _>::new(&mut args) };
            // SAFETY: request, nested pointer, lengths, and exclusive output
            // borrow are established above. The result remains untrusted.
            let result = unsafe { rustix::ioctl::ioctl(kfd, request) }
                .map_err(|source| Self::syscall(operation, source));
            if args.handle != handle
                || args.device_ids_array_ptr != pointer
                || args.n_devices != 1
                || args.n_success < old_success
                || args.n_success > 1
            {
                return KernelOutcome {
                    value: args.n_success,
                    result: Err(MemorySessionError::KernelResultMalformed(
                        "UNMAP_MEMORY_FROM_GPU immutable request or cumulative progress",
                    )),
                };
            }
            KernelOutcome {
                value: args.n_success,
                result,
            }
        } else {
            let mut args = KfdIoctlMapMemoryToGpuArgs::retry(
                handle,
                pointer,
                device_ids.len() as u32,
                old_success,
            );
            // SAFETY: same reviewed nested-pointer contract as the unmap path.
            let request = unsafe { Updater::<MAP_MEMORY_OPCODE, _>::new(&mut args) };
            // SAFETY: request and backing array stay live; output is exclusive.
            let result = unsafe { rustix::ioctl::ioctl(kfd, request) }
                .map_err(|source| Self::syscall(operation, source));
            if args.handle != handle
                || args.device_ids_array_ptr != pointer
                || args.n_devices != 1
                || args.n_success < old_success
                || args.n_success > 1
            {
                return KernelOutcome {
                    value: args.n_success,
                    result: Err(MemorySessionError::KernelResultMalformed(
                        "MAP_MEMORY_TO_GPU immutable request or cumulative progress",
                    )),
                };
            }
            KernelOutcome {
                value: args.n_success,
                result,
            }
        }
    }
}

impl MemoryBackend for LinuxMemoryBackend {
    type Reservation = LinuxVaReservation;
    type Mapping = LinuxCpuMapping;

    fn opener_pid(&self) -> u32 {
        self.device.process_incarnation().pid()
    }

    fn gpu_id(&self) -> u32 {
        self.device.observation().kfd_gpu_id()
    }

    fn gpuvm_aperture(&self) -> InclusiveAperture {
        self.device.observation().aperture().gpuvm()
    }

    fn page_size(&self) -> usize {
        rustix::param::page_size()
    }

    fn check_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.device.check_observable_currentness()?;
        Ok(())
    }

    fn acquire_vm(&mut self) -> Result<(), MemorySessionError> {
        let raw_fd = self.device.render_fd.as_raw_fd();
        let drm_fd = u32::try_from(raw_fd)
            .map_err(|_| MemorySessionError::KernelResultMalformed("render descriptor number"))?;
        let args = KfdIoctlAcquireVmArgs::new(drm_fd, self.gpu_id());
        // SAFETY: opcode and input-only C layout are frozen by the independent
        // KFD 1.18 oracle. Both retained descriptors outlive the call.
        let request = unsafe { Setter::<ACQUIRE_VM_OPCODE, _>::new(args) };
        // SAFETY: the input-only request and retained KFD descriptor satisfy
        // the reviewed request contract. Success is rechecked for currentness.
        unsafe { rustix::ioctl::ioctl(&self.device.kfd.opened.fd, request) }
            .map_err(|source| Self::syscall("AMDKFD_IOC_ACQUIRE_VM", source))?;
        self.device.retire_model_on_drop = false;
        Ok(())
    }

    fn reserve_va(&mut self, bytes: usize) -> Result<Self::Reservation, MemorySessionError> {
        // SAFETY: null lets the kernel select a fresh range; a nonzero,
        // page-rounded length is supplied. No references exist to the result.
        let address = unsafe {
            rustix::mm::mmap_anonymous(
                core::ptr::null_mut(),
                bytes,
                ProtFlags::empty(),
                MapFlags::PRIVATE | MapFlags::NORESERVE,
            )
        }
        .map_err(|source| Self::syscall("reserve anonymous GPU VA", source))?;
        let address = NonNull::new(address).ok_or(MemorySessionError::KernelResultMalformed(
            "anonymous mmap address",
        ))?;
        Ok(LinuxVaReservation {
            address,
            bytes,
            replaced: false,
        })
    }

    fn reservation_address(reservation: &Self::Reservation) -> u64 {
        reservation.address.as_ptr() as usize as u64
    }

    fn alloc(
        &mut self,
        va: u64,
        bytes: u64,
        flags: KfdAllocMemoryFlags,
    ) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs> {
        let mut args = KfdIoctlAllocMemoryOfGpuArgs::new(va, bytes, self.gpu_id(), flags);
        // SAFETY: the opcode and in/out C layout are frozen by the KFD 1.18
        // oracle, and initialized exclusive storage remains live for the call.
        let request = unsafe { Updater::<ALLOC_MEMORY_OPCODE, _>::new(&mut args) };
        // SAFETY: request contract is established above; every field is still
        // treated as untrusted even if ioctl returns success.
        let result = unsafe { rustix::ioctl::ioctl(&self.device.kfd.opened.fd, request) }
            .map_err(|source| Self::syscall("AMDKFD_IOC_ALLOC_MEMORY_OF_GPU", source));
        KernelOutcome {
            value: args,
            result,
        }
    }

    fn prepare_userptr(
        &mut self,
        reservation: &mut Self::Reservation,
        bytes: usize,
    ) -> Result<Self::Mapping, MemorySessionError> {
        if reservation.replaced || bytes == 0 || bytes != reservation.bytes {
            return Err(MemorySessionError::KernelResultMalformed(
                "USERPTR reservation geometry",
            ));
        }
        let mut mapping = LinuxCpuMapping {
            address: reservation.address,
            bytes,
            active: true,
            accessible: false,
        };
        // FE reuses its kernel-selected PRIVATE|ANONYMOUS|NORESERVE guard and
        // makes those exact pages accessible in place. ROCr instead replaces a
        // reserved range with MAP_FIXED pages; DONTFORK and NORESERVE are FE
        // safety differences and do not claim syscall-identical allocation.
        if let Err(error) = self.prepare_cpu_mapping(&mut mapping) {
            reservation.replaced = true;
            return Err(error);
        }
        reservation.replaced = true;
        Ok(mapping)
    }

    fn alloc_userptr(
        &mut self,
        address: u64,
        bytes: u64,
        flags: KfdAllocMemoryFlags,
    ) -> KernelOutcome<KfdIoctlAllocMemoryOfGpuArgs> {
        let mut args = if flags == KfdAllocMemoryFlags::USERPTR_EXECUTABLE {
            KfdIoctlAllocMemoryOfGpuArgs::new_userptr(address, bytes, self.gpu_id())
        } else if flags == KfdAllocMemoryFlags::USERPTR_QUEUE_CONTROL {
            KfdIoctlAllocMemoryOfGpuArgs::new_userptr_queue_control(address, bytes, self.gpu_id())
        } else {
            return KernelOutcome {
                value: KfdIoctlAllocMemoryOfGpuArgs::new(address, bytes, self.gpu_id(), flags),
                result: Err(MemorySessionError::KernelResultMalformed(
                    "unsupported USERPTR allocation profile",
                )),
            };
        };
        // SAFETY: the exact USERPTR input VMA is page-aligned, DONTFORK,
        // read/write, and retained by the safe engine through explicit FREE.
        let request = unsafe { Updater::<ALLOC_MEMORY_OPCODE, _>::new(&mut args) };
        // SAFETY: the reviewed in/out record and live VMA remain exclusively
        // owned for the call; all kernel-written fields remain untrusted.
        let result = unsafe { rustix::ioctl::ioctl(&self.device.kfd.opened.fd, request) }
            .map_err(|source| Self::syscall("AMDKFD_IOC_ALLOC_MEMORY_OF_GPU(USERPTR)", source));
        KernelOutcome {
            value: args,
            result,
        }
    }

    fn map_cpu(
        &mut self,
        reservation: &mut Self::Reservation,
        mmap_offset: u64,
        bytes: usize,
        retain_gpu_va_guard: bool,
    ) -> Result<Self::Mapping, MemorySessionError> {
        if reservation.replaced || bytes == 0 || bytes > reservation.bytes {
            return Err(MemorySessionError::KernelResultMalformed(
                "VA reservation replacement",
            ));
        }
        if !retain_gpu_va_guard {
            // SAFETY: this exact anonymous reservation is owned and has no
            // Rust references. The single-allocation compatibility path does
            // not require a persistent userspace VA guard.
            unsafe { rustix::mm::munmap(reservation.address.as_ptr(), reservation.bytes) }
                .map_err(|source| Self::syscall("release anonymous GPU VA reservation", source))?;
            reservation.replaced = true;
        }
        // SAFETY: null requests a kernel-selected CPU VMA. It is deliberately
        // PROT_NONE until DONTFORK succeeds, so the setup gap cannot expose BO
        // bytes even if an external raw fork violates the named contract.
        let mapped = unsafe {
            rustix::mm::mmap(
                core::ptr::null_mut(),
                bytes,
                ProtFlags::empty(),
                MapFlags::SHARED,
                &self.device.render_fd,
                mmap_offset,
            )
        }
        .map_err(|source| Self::syscall("mmap AMDGPU BO", source))?;
        let Some(address) = NonNull::new(mapped) else {
            // A mapping at address zero is outside the admitted profile. It is
            // still a live VMA and must not be returned ambiguously.
            // SAFETY: `mapped` and `bytes` are the exact successful mmap range.
            if unsafe { rustix::mm::munmap(mapped, bytes) }.is_err() {
                std::process::abort();
            }
            return Err(MemorySessionError::KernelResultMalformed(
                "AMDGPU BO mmap address",
            ));
        };
        Ok(LinuxCpuMapping {
            address,
            bytes,
            active: true,
            accessible: false,
        })
    }

    fn prepare_cpu_mapping(
        &mut self,
        mapping: &mut Self::Mapping,
    ) -> Result<(), MemorySessionError> {
        if !mapping.active || mapping.accessible {
            return Err(MemorySessionError::KernelResultMalformed(
                "CPU mapping setup state",
            ));
        }
        // SAFETY: the mapping is live, page-aligned, exclusively borrowed, and
        // PROT_NONE. DONTFORK is mandatory because TTM lacks VM_DONTCOPY and
        // would otherwise create a child VMA/BO reference.
        let advised = unsafe {
            rustix::mm::madvise(
                mapping.address.as_ptr(),
                mapping.bytes,
                Advice::LinuxDontFork,
            )
        };
        if let Err(source) = advised {
            Self::discard_unprepared_mapping_or_abort(mapping);
            return Err(Self::syscall("madvise MADV_DONTFORK", source));
        }
        // SAFETY: the exact still-live VMA has DONTFORK installed and no slice
        // exists. Read/write access is enabled only after that ordering point.
        let protected = unsafe {
            rustix::mm::mprotect(
                mapping.address.as_ptr(),
                mapping.bytes,
                MprotectFlags::READ | MprotectFlags::WRITE,
            )
        };
        if let Err(source) = protected {
            Self::discard_unprepared_mapping_or_abort(mapping);
            return Err(Self::syscall("mprotect AMDGPU BO read/write", source));
        }
        mapping.accessible = true;
        Ok(())
    }

    fn protect_cpu_read_only(
        &mut self,
        mapping: &mut Self::Mapping,
    ) -> Result<(), MemorySessionError> {
        if !mapping.active || !mapping.accessible {
            return Err(MemorySessionError::KernelResultMalformed(
                "CPU mapping protection state",
            ));
        }
        // SAFETY: the mapping is live, exclusively borrowed, and no slice can
        // escape a safe closure. This removes CPU write access atomically for
        // the complete VMA; it does not constrain GPU writes.
        unsafe {
            rustix::mm::mprotect(mapping.address.as_ptr(), mapping.bytes, MprotectFlags::READ)
        }
        .map_err(|source| Self::syscall("mprotect AMDGPU BO read-only", source))
    }

    fn map_gpu(&mut self, handle: u64, old_success: u32) -> KernelOutcome<u32> {
        Self::exact_progress(
            "AMDKFD_IOC_MAP_MEMORY_TO_GPU",
            handle,
            self.gpu_id(),
            old_success,
            false,
            &self.device.kfd.opened.fd,
        )
    }

    fn unmap_gpu(&mut self, handle: u64, old_success: u32) -> KernelOutcome<u32> {
        Self::exact_progress(
            "AMDKFD_IOC_UNMAP_MEMORY_FROM_GPU",
            handle,
            self.gpu_id(),
            old_success,
            true,
            &self.device.kfd.opened.fd,
        )
    }

    fn with_bytes<R>(
        mapping: &Self::Mapping,
        requested_bytes: usize,
        f: impl FnOnce(&[u8]) -> R,
    ) -> R {
        debug_assert!(mapping.active && mapping.accessible && requested_bytes <= mapping.bytes);
        // SAFETY: the live mapping covers this range. The safe engine checks
        // phase and process before entering this boundary.
        let bytes = unsafe {
            core::slice::from_raw_parts(mapping.address.as_ptr().cast(), requested_bytes)
        };
        f(bytes)
    }

    fn with_bytes_mut<R>(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> R {
        debug_assert!(mapping.active && mapping.accessible && requested_bytes <= mapping.bytes);
        // SAFETY: the exclusive mapping borrow covers the slice, and the safe
        // engine checks phase and process before entering this boundary.
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(mapping.address.as_ptr().cast(), requested_bytes)
        };
        f(bytes)
    }

    fn observe_aql_counters(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
    ) -> Result<(u64, u64), MemorySessionError> {
        let write = checked_atomic_u64(
            mapping,
            requested_bytes,
            AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1,
        )?
        .load(Ordering::Acquire);
        let read =
            checked_atomic_u64(mapping, requested_bytes, AMD_AQL_READ_DISPATCH_ID_OFFSET_V1)?
                .load(Ordering::Acquire);
        Ok((write, read))
    }

    fn fetch_add_aql_write(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        increment: u64,
    ) -> Result<u64, MemorySessionError> {
        Ok(checked_atomic_u64(
            mapping,
            requested_bytes,
            AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1,
        )?
        .fetch_add(increment, Ordering::AcqRel))
    }

    fn write_aql_slot(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        slot_index: u32,
        packet: &[u8; 64],
    ) -> Result<(), MemorySessionError> {
        let unpublished = u32::from_le_bytes(
            packet[..4]
                .try_into()
                .map_err(|_| malformed_aql_mapping("packet header"))?,
        );
        let setup = unpublished >> 16;
        if unpublished & 0xffff != u32::from(AQL_INVALID_PACKET_HEADER_V1) || setup > 3 {
            return Err(malformed_aql_mapping("unpublished packet header"));
        }
        let offset = usize::try_from(slot_index)
            .ok()
            .and_then(|index| index.checked_mul(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1))
            .ok_or_else(|| malformed_aql_mapping("packet slot offset"))?;
        let pointer = checked_mapping_pointer(
            mapping,
            requested_bytes,
            offset,
            AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
            core::mem::align_of::<AtomicU32>(),
        )?;
        // SAFETY: each header AtomicU32 was initialized before GPU mapping;
        // the checked slot is aligned and remains owned by this mapping.
        unsafe { &*pointer.cast::<AtomicU32>() }.store(unpublished.to_le(), Ordering::Relaxed);
        // SAFETY: the checked slot contains 64 bytes. Offset zero remains an
        // AtomicU32 and the remaining 60 bytes are the unpublished body.
        unsafe {
            core::ptr::copy_nonoverlapping(
                packet.as_ptr().add(4),
                pointer.add(4),
                AQL_KERNEL_DISPATCH_PACKET_BYTES_V1 - 4,
            );
        }
        Ok(())
    }

    fn publish_aql_header(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        slot_index: u32,
        header: u16,
    ) -> Result<(), MemorySessionError> {
        let offset = usize::try_from(slot_index)
            .ok()
            .and_then(|index| index.checked_mul(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1))
            .ok_or_else(|| malformed_aql_mapping("packet slot offset"))?;
        let pointer = checked_mapping_pointer(
            mapping,
            requested_bytes,
            offset,
            core::mem::size_of::<AtomicU32>(),
            core::mem::align_of::<AtomicU32>(),
        )?;
        // SAFETY: this is the exact initialized header AtomicU32.
        let atomic = unsafe { &*pointer.cast::<AtomicU32>() };
        let unpublished = u32::from_le(atomic.load(Ordering::Relaxed));
        let setup = unpublished >> 16;
        if unpublished & 0xffff != u32::from(AQL_INVALID_PACKET_HEADER_V1)
            || !is_reviewed_aql_publication_v1(header, setup as u16)
        {
            return Err(malformed_aql_mapping("packet no longer unpublished"));
        }
        atomic.store(
            ((setup << 16) | u32::from(header)).to_le(),
            Ordering::Release,
        );
        Ok(())
    }

    fn observe_aql_packet_header_acquire(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        packet_id: u64,
    ) -> Result<(u32, u16, u16), MemorySessionError> {
        let ring_bytes = u32::try_from(requested_bytes)
            .map_err(|_| malformed_aql_mapping("packet observation ring length"))?;
        let capacity = AqlRingCapacityV1::from_ring_bytes(ring_bytes)
            .map_err(|_| malformed_aql_mapping("packet observation ring capacity"))?;
        let slot_index = u32::try_from(packet_id & capacity.mask())
            .map_err(|_| malformed_aql_mapping("packet observation slot index"))?;
        let offset = usize::try_from(slot_index)
            .ok()
            .and_then(|index| index.checked_mul(AQL_KERNEL_DISPATCH_PACKET_BYTES_V1))
            .ok_or_else(|| malformed_aql_mapping("packet observation slot offset"))?;
        let pointer = checked_mapping_pointer(
            mapping,
            requested_bytes,
            offset,
            core::mem::size_of::<AtomicU32>(),
            core::mem::align_of::<AtomicU32>(),
        )?;
        // SAFETY: every admitted ring slot header is an initialized AtomicU32.
        let full_header =
            u32::from_le(unsafe { &*pointer.cast::<AtomicU32>() }.load(Ordering::Acquire));
        Ok((slot_index, full_header as u16, (full_header >> 16) as u16))
    }

    fn observe_completion_signal_acquire(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        slot_index: u32,
    ) -> Result<AqlCompletionObservationV1, MemorySessionError> {
        let value =
            checked_completion_value(mapping, requested_bytes, slot_index)?.load(Ordering::Acquire);
        Ok(classify_acquired_completion_value_v1(value))
    }

    fn observe_completion_signal_state_acquire(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        slot_index: u32,
    ) -> Result<(i64, i64), MemorySessionError> {
        let offset = usize::try_from(slot_index)
            .ok()
            .and_then(|index| index.checked_mul(AMD_SIGNAL_BYTES_V1))
            .ok_or_else(|| malformed_aql_mapping("completion state slot offset"))?;
        let kind_pointer = checked_mapping_pointer(
            mapping,
            requested_bytes,
            offset,
            core::mem::size_of::<i64>(),
            core::mem::align_of::<i64>(),
        )?;
        // SAFETY: the exact admitted signal kind word remains inside the live
        // mapping and immutable after initialization.
        let kind = i64::from_le(unsafe { core::ptr::read_volatile(kind_pointer.cast::<i64>()) });
        let value =
            checked_completion_value(mapping, requested_bytes, slot_index)?.load(Ordering::Acquire);
        Ok((kind, value))
    }

    fn reset_completion_signal_release(
        mapping: &mut Self::Mapping,
        requested_bytes: usize,
        slot_index: u32,
    ) -> Result<(), MemorySessionError> {
        checked_completion_value(mapping, requested_bytes, slot_index)?
            .store(AMD_SIGNAL_VALUE_PENDING_V1, Ordering::Release);
        Ok(())
    }

    fn unmap_cpu(&mut self, mapping: &mut Self::Mapping) -> Result<(), MemorySessionError> {
        if !mapping.active || !mapping.accessible {
            return Err(MemorySessionError::KernelResultMalformed(
                "CPU mapping state",
            ));
        }
        // SAFETY: the mapping is exclusively borrowed and no safe slice can
        // survive a closure call. The engine establishes the backing-specific
        // ordering relative to FREE before invoking this primitive.
        unsafe { rustix::mm::munmap(mapping.address.as_ptr(), mapping.bytes) }
            .map_err(|source| Self::syscall("munmap AMDGPU BO", source))?;
        mapping.active = false;
        mapping.accessible = false;
        Ok(())
    }

    fn release_va_reservation(
        &mut self,
        reservation: &mut Self::Reservation,
    ) -> Result<(), MemorySessionError> {
        if reservation.replaced {
            return Err(MemorySessionError::KernelResultMalformed(
                "GPU VA reservation release state",
            ));
        }
        // SAFETY: the PROT_NONE guard is owned by the session, no references
        // can exist to it, and the associated KFD allocation has been freed.
        unsafe { rustix::mm::munmap(reservation.address.as_ptr(), reservation.bytes) }
            .map_err(|source| Self::syscall("release retained GPU VA reservation", source))?;
        reservation.replaced = true;
        Ok(())
    }

    fn free(&mut self, handle: u64) -> Result<(), MemorySessionError> {
        let args = KfdIoctlFreeMemoryOfGpuArgs::new(handle);
        // SAFETY: the input-only opcode/layout are oracle-frozen. The safe
        // engine invokes this operation at most once.
        let request = unsafe { Setter::<FREE_MEMORY_OPCODE, _>::new(args) };
        // SAFETY: request and retained KFD descriptor satisfy that contract.
        unsafe { rustix::ioctl::ioctl(&self.device.kfd.opened.fd, request) }
            .map_err(|source| Self::syscall("AMDKFD_IOC_FREE_MEMORY_OF_GPU", source))
    }
}

fn malformed_aql_mapping(detail: &'static str) -> MemorySessionError {
    MemorySessionError::KernelResultMalformed(detail)
}

fn checked_mapping_pointer(
    mapping: &mut LinuxCpuMapping,
    requested_bytes: usize,
    offset: usize,
    byte_len: usize,
    alignment: usize,
) -> Result<*mut u8, MemorySessionError> {
    let end = offset
        .checked_add(byte_len)
        .ok_or_else(|| malformed_aql_mapping("mapped range overflow"))?;
    if !mapping.active
        || !mapping.accessible
        || requested_bytes > mapping.bytes
        || end > requested_bytes
        || alignment == 0
        || !alignment.is_power_of_two()
    {
        return Err(malformed_aql_mapping("mapped range"));
    }
    // SAFETY: offset is bounded by the live retained mapping above. The raw
    // pointer remains inside this private backend and no slice/reference is
    // returned to safe queue code.
    let pointer = unsafe { mapping.address.as_ptr().cast::<u8>().add(offset) };
    if !(pointer as usize).is_multiple_of(alignment) {
        return Err(malformed_aql_mapping("mapped alignment"));
    }
    Ok(pointer)
}

fn checked_atomic_u64(
    mapping: &mut LinuxCpuMapping,
    requested_bytes: usize,
    offset: usize,
) -> Result<&AtomicU64, MemorySessionError> {
    let pointer = checked_mapping_pointer(
        mapping,
        requested_bytes,
        offset,
        core::mem::size_of::<AtomicU64>(),
        core::mem::align_of::<AtomicU64>(),
    )?;
    // SAFETY: both control AtomicU64 objects were explicitly initialized
    // before GPU mapping and the exact object remains live until teardown.
    Ok(unsafe { &*pointer.cast::<AtomicU64>() })
}

fn checked_completion_value(
    mapping: &mut LinuxCpuMapping,
    requested_bytes: usize,
    slot_index: u32,
) -> Result<&AtomicI64, MemorySessionError> {
    let signal_offset = usize::try_from(slot_index)
        .ok()
        .and_then(|index| index.checked_mul(AMD_SIGNAL_BYTES_V1))
        .ok_or_else(|| malformed_aql_mapping("completion signal offset"))?;
    let signal = checked_mapping_pointer(
        mapping,
        requested_bytes,
        signal_offset,
        AMD_SIGNAL_BYTES_V1,
        AMD_SIGNAL_BYTES_V1,
    )?;
    // SAFETY: the checked signal range contains 64 bytes, and the frozen ABI
    // places the AtomicI64 value at byte offset eight.
    let pointer = unsafe { signal.add(8) };
    // SAFETY: the completion arena initializer created one exact signal
    // object in every 64-byte slot before GPU mapping. Its AtomicI64 value
    // remains alive and mapped until explicit queue teardown.
    Ok(unsafe { &*pointer.cast::<AtomicI64>() })
}

impl Drop for LinuxVaReservation {
    fn drop(&mut self) {
        // Deliberately no implicit munmap after an ambiguous operation.
    }
}

impl Drop for LinuxCpuMapping {
    fn drop(&mut self) {
        // Deliberately no implicit munmap or FREE retry.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_aql::{
        AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
        AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1, AmdBusyCompletionSignalV1,
        AqlCompletionObservationV1,
    };

    #[repr(C, align(64))]
    struct TwoSignals([AmdBusyCompletionSignalV1; 2]);

    #[repr(C, align(64))]
    struct OnePacket([u8; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1]);

    #[repr(C, align(64))]
    struct MinimumRing([u8; 4096]);

    #[repr(C, align(64))]
    struct AmdAqlControl([u8; 4096]);

    #[test]
    fn aql_counter_operations_use_the_reviewed_amd_control_offsets() {
        let mut control = AmdAqlControl([0xff; 4096]);
        crate::queue::submit::initialize_amd_aql_control(&mut control.0).unwrap();
        control.0[..8].copy_from_slice(&0xaaaa_aaaa_aaaa_aaaa_u64.to_le_bytes());
        control.0[8..16].copy_from_slice(&0xbbbb_bbbb_bbbb_bbbb_u64.to_le_bytes());
        let mut mapping = LinuxCpuMapping {
            address: NonNull::from(&mut control).cast(),
            bytes: 4096,
            active: true,
            accessible: true,
        };
        checked_atomic_u64(&mut mapping, 4096, AMD_AQL_WRITE_DISPATCH_ID_OFFSET_V1)
            .unwrap()
            .store(17, Ordering::Relaxed);
        checked_atomic_u64(&mut mapping, 4096, AMD_AQL_READ_DISPATCH_ID_OFFSET_V1)
            .unwrap()
            .store(9, Ordering::Relaxed);

        assert_eq!(
            LinuxMemoryBackend::observe_aql_counters(&mut mapping, 4096).unwrap(),
            (17, 9)
        );
        assert_eq!(
            LinuxMemoryBackend::fetch_add_aql_write(&mut mapping, 4096, 4).unwrap(),
            17
        );
        assert_eq!(
            LinuxMemoryBackend::observe_aql_counters(&mut mapping, 4096).unwrap(),
            (21, 9)
        );
        assert_eq!(&control.0[..8], &0xaaaa_aaaa_aaaa_aaaa_u64.to_le_bytes());
        assert_eq!(&control.0[8..16], &0xbbbb_bbbb_bbbb_bbbb_u64.to_le_bytes());
    }

    #[test]
    fn mapped_packet_accepts_exact_wait_for_prior_release_header() {
        let mut packet = OnePacket([0; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1]);
        packet.0[..4].copy_from_slice(&0x0002_0001_u32.to_le_bytes());
        let mut mapping = LinuxCpuMapping {
            address: NonNull::from(&mut packet).cast(),
            bytes: AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
            active: true,
            accessible: true,
        };

        LinuxMemoryBackend::publish_aql_header(
            &mut mapping,
            AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
            0,
            AQL_SYSTEM_SCOPED_WAIT_FOR_PRIOR_KERNEL_DISPATCH_HEADER_V1,
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(packet.0[..4].try_into().unwrap()),
            0x0002_1502
        );

        packet.0[..4].copy_from_slice(&0x0002_0001_u32.to_le_bytes());
        assert!(
            LinuxMemoryBackend::publish_aql_header(
                &mut mapping,
                AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
                0,
                0x1503,
            )
            .is_err()
        );
        assert_eq!(
            u32::from_le_bytes(packet.0[..4].try_into().unwrap()),
            0x0002_0001
        );
    }

    #[test]
    fn mapped_packet_accepts_only_zero_setup_for_barrier_and_header() {
        let mut packet = OnePacket([0; AQL_KERNEL_DISPATCH_PACKET_BYTES_V1]);
        packet.0[..4].copy_from_slice(&1_u32.to_le_bytes());
        let mut mapping = LinuxCpuMapping {
            address: NonNull::from(&mut packet).cast(),
            bytes: AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
            active: true,
            accessible: true,
        };

        LinuxMemoryBackend::publish_aql_header(
            &mut mapping,
            AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
            0,
            AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
        )
        .unwrap();
        assert_eq!(
            u32::from_le_bytes(packet.0[..4].try_into().unwrap()),
            0x0000_1403
        );

        packet.0[..4].copy_from_slice(&0x0001_0001_u32.to_le_bytes());
        assert!(
            LinuxMemoryBackend::publish_aql_header(
                &mut mapping,
                AQL_KERNEL_DISPATCH_PACKET_BYTES_V1,
                0,
                AQL_SYSTEM_SCOPED_BARRIER_AND_HEADER_V1,
            )
            .is_err()
        );
    }

    #[test]
    fn mapped_packet_observation_is_acquiring_and_wraps_private_slot() {
        let mut ring = MinimumRing([0; 4096]);
        let wrapped_slot = 3_usize;
        let offset = wrapped_slot * AQL_KERNEL_DISPATCH_PACKET_BYTES_V1;
        ring.0[offset..offset + 4].copy_from_slice(&0x0003_1502_u32.to_le_bytes());
        let mut mapping = LinuxCpuMapping {
            address: NonNull::from(&mut ring).cast(),
            bytes: ring.0.len(),
            active: true,
            accessible: true,
        };

        assert_eq!(
            LinuxMemoryBackend::observe_aql_packet_header_acquire(
                &mut mapping,
                4096,
                64 + wrapped_slot as u64,
            )
            .unwrap(),
            (wrapped_slot as u32, 0x1502, 3)
        );
        assert!(
            LinuxMemoryBackend::observe_aql_packet_header_acquire(&mut mapping, 63, 0).is_err()
        );
    }

    #[test]
    fn mapped_completion_slots_use_exact_acquire_and_release_atomics() {
        let mut signals = TwoSignals([
            AmdBusyCompletionSignalV1::new_pending(),
            AmdBusyCompletionSignalV1::new_pending(),
        ]);
        let mut mapping = LinuxCpuMapping {
            address: NonNull::from(&mut signals).cast(),
            bytes: 2 * AMD_SIGNAL_BYTES_V1,
            active: true,
            accessible: true,
        };
        assert_eq!(
            LinuxMemoryBackend::observe_completion_signal_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                1,
            )
            .unwrap(),
            AqlCompletionObservationV1::Pending
        );
        assert_eq!(
            LinuxMemoryBackend::observe_completion_signal_state_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                1,
            )
            .unwrap(),
            (
                fe2o3_aql::AMD_SIGNAL_KIND_USER_V1,
                AMD_SIGNAL_VALUE_PENDING_V1
            )
        );
        checked_completion_value(&mut mapping, 2 * AMD_SIGNAL_BYTES_V1, 1)
            .unwrap()
            .store(0, Ordering::Release);
        assert_eq!(
            LinuxMemoryBackend::observe_completion_signal_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                1,
            )
            .unwrap(),
            AqlCompletionObservationV1::Completed
        );
        assert_eq!(
            LinuxMemoryBackend::observe_completion_signal_state_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                1,
            )
            .unwrap(),
            (fe2o3_aql::AMD_SIGNAL_KIND_USER_V1, 0)
        );
        LinuxMemoryBackend::reset_completion_signal_release(
            &mut mapping,
            2 * AMD_SIGNAL_BYTES_V1,
            1,
        )
        .unwrap();
        assert_eq!(
            LinuxMemoryBackend::observe_completion_signal_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                1,
            )
            .unwrap(),
            AqlCompletionObservationV1::Pending
        );
        assert!(
            LinuxMemoryBackend::observe_completion_signal_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                2,
            )
            .is_err()
        );
        assert!(
            LinuxMemoryBackend::observe_completion_signal_state_acquire(
                &mut mapping,
                2 * AMD_SIGNAL_BYTES_V1,
                2,
            )
            .is_err()
        );
    }
}

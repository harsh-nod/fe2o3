//! Test observations only; every operation uses the original constructed engine.

use super::device_initialization::allocation_cases::{AllocationSnapshot, allocation_snapshot};
use super::device_initialization::{NativeSnapshot, RootSnapshot, root_snapshot, snapshot};
use super::preparation::PreparationMemoryFixtureV1;
use super::*;
use crate::shared_memory::device_initialization::{
    DeviceInitializationStageV1 as Stage, InitializationLeaseV1,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DeviceInsertionMemorySnapshotV1 {
    pub(crate) next_id: u64,
    pub(crate) calls: [usize; 5],
    pub(crate) operations: Vec<&'static str>,
    pub(crate) terminal: Option<RootSnapshot>,
    pub(crate) allocation_terminal: Option<AllocationSnapshot>,
    pub(crate) terminal_occupied: bool,
    pub(crate) account: Option<usize>,
    pub(crate) terminal_storage: (usize, usize),
    pub(crate) readback_calls: usize,
    pub(crate) initialized_bytes: Option<Vec<u8>>,
    records: Vec<NativeSnapshot>,
    cpu_inputs: Vec<((u64, usize), u64, usize)>,
    gpu_inputs: Vec<(u64, u32)>,
}

pub(crate) struct DeviceInsertionPrefixV1<'a> {
    pub(crate) calls: [usize; 5],
    pub(crate) phase: Option<&'static str>,
    pub(crate) handle: bool,
    pub(crate) cpu_writable: Option<bool>,
    pub(crate) written: bool,
    pub(crate) operations: &'a [&'static str],
}

impl DeviceInitializationCustodyV1 {
    pub(crate) fn insertion_snapshot_for_test(&self) -> RootSnapshot {
        root_snapshot(&self.0)
    }
}

impl DeviceAllocationCustodyV1 {
    pub(crate) fn insertion_snapshot_for_test(&self) -> AllocationSnapshot {
        allocation_snapshot(&self.0)
    }
}

enum InsertionRootRef<'a> {
    Initialized(&'a crate::shared_memory::device_initialization::DeviceInitializationCustodyV1),
    Allocation(&'a crate::shared_memory::device_allocation::DeviceAllocationCustodyV1),
}

#[test]
fn device_allocator_partition_does_not_invent_pending_custody_after_extraction() {
    use crate::queue::dispatch_binding::preparation::PreparationOwnerRefsV1;

    for configured in [false, true] {
        let mut memory = PreparationMemoryFixtureV1::new(configured);
        let mut data = memory.roster();
        let before = memory.insertion_memory_snapshot_v1();
        let layout = device_memory_layout(17, 4096, KfdAllocMemoryFlags::DEVICE_LOCAL).unwrap();
        let mut root = DeviceAllocationCustodyV1::new();
        memory
            .primary_prepare_device_allocation_v1(&mut root, 17, 4096)
            .unwrap();
        {
            let mut refs = PreparationOwnerRefsV1::default();
            refs.data(&data);
            memory.insertion_assert_allocation_partition_v1(
                &refs.device_leases,
                &refs.device_authorities,
                Some(&root),
                before.next_id,
                layout,
                &[],
            );
        }
        data.push(crate::Gfx942FixedDispatchDataV1::uninitialized(
            root.take_complete().unwrap(),
        ));
        let state = root.insertion_snapshot_for_test();
        assert!(state.started() && state.native_started() && !state.failed());
        assert_eq!(state.lease(), None);
        assert_eq!(state.progress(), (true, Some(true), Some(1)));
        let mut refs = PreparationOwnerRefsV1::default();
        refs.data(&data);
        memory.insertion_assert_allocation_partition_v1(
            &refs.device_leases,
            &refs.device_authorities,
            Some(&root),
            before.next_id,
            layout,
            &[],
        );
        assert!(!memory.insertion_memory_snapshot_v1().terminal_occupied);
    }
}

impl PreparationMemoryFixtureV1 {
    pub(crate) fn primary_sdma_recycle_disposition_v1(
        &self,
        owner: fe2o3_runtime_model::QueueKeyV1,
        device_limits: Option<crate::Gfx942DevicePoolLimitsV1>,
        host_limits: Option<crate::Gfx942HostPoolLimitsV1>,
        free: &[crate::Gfx942SdmaBufferV1],
        buffer: &crate::Gfx942SdmaBufferV1,
    ) -> Result<bool, MemorySessionError> {
        use crate::sdma::{Gfx942SdmaBufferKindV1, host_pool_policy, pool_policy};
        let device = self.fixture.device.model_key();
        let vm = self.fixture.vm;
        match buffer.kind() {
            Gfx942SdmaBufferKindV1::DeviceLocal => {
                let Some(limits) = device_limits else {
                    return Ok(false);
                };
                self.fixture.engine.require_active()?;
                if owner.vm != vm || vm.device != device {
                    return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
                }
                pool_policy::device_pool_recycle_decision_with_v1(
                    owner,
                    limits,
                    free,
                    buffer,
                    &mut |lease| {
                        self.fixture
                            .engine
                            .device_pool_backing_bytes_v1(lease, device, vm)
                    },
                )
                .map(|decision| decision == pool_policy::DevicePoolDispositionV1::Dispose)
                .map_err(|_| MemorySessionError::Injected("device recycle policy"))
            }
            Gfx942SdmaBufferKindV1::HostVisibleCoherent => {
                let Some(limits) = host_limits else {
                    return Ok(false);
                };
                self.fixture
                    .engine
                    .validate_host_pool_domain_v1(device, vm)?;
                host_pool_policy::host_pool_recycle_decision_with_v1(
                    owner,
                    limits,
                    free,
                    buffer,
                    &mut |token| {
                        self.fixture
                            .engine
                            .host_pool_backing_bytes_v1(token, device, vm)
                    },
                )
                .map(|decision| decision == host_pool_policy::HostPoolDispositionV1::Dispose)
                .map_err(|_| MemorySessionError::Injected("host recycle policy"))
            }
        }
    }

    pub(crate) fn primary_validate_sdma_device_mapping_v1(
        &self,
        buffer: &crate::Gfx942SdmaBufferV1,
    ) -> Result<(), crate::Gfx942SdmaErrorV1> {
        buffer.validate_physical_device_mapping_with_v1(|lease| {
            self.fixture.engine.mapped_device_memory_facts_v1(
                lease,
                self.fixture.device.model_key(),
                self.fixture.vm,
            )
        })
    }

    pub(crate) fn primary_validate_sdma_demotion_mapping_v1(
        &self,
        allocation: &crate::Gfx942DirectionalQueuePersistentAllocationV1,
    ) -> Result<(), MemorySessionError> {
        self.fixture
            .engine
            .mapped_device_memory_facts_v1(
                allocation
                    .owner
                    .local_native_for_sdma()
                    .expect("admitted local allocation"),
                self.fixture.device.model_key(),
                self.fixture.vm,
            )
            .map(|_| ())
    }

    pub(crate) fn insertion_release_device_v1(
        &mut self,
        data: crate::Gfx942FixedDispatchDataV1,
    ) -> Result<Gfx942DeviceMemoryIdentityV1, MemorySessionError> {
        let crate::queue::dispatch_binding::DispatchDataInputStorageV1::Device(lease) =
            data.into_parts().storage
        else {
            panic!("device-only successful release prerequisite");
        };
        let identity = lease.storage_identity();
        let unmapped = self.fixture.engine.unmap_device_memory(lease)?;
        self.fixture.engine.release_device_memory(unmapped)?;
        Ok(identity)
    }

    pub(crate) fn insertion_assert_native_prefix_v1(
        &self,
        before: &DeviceInsertionMemorySnapshotV1,
        expected: DeviceInsertionPrefixV1<'_>,
        source: &[u8],
    ) {
        let layout = device_memory_layout(
            source.len() as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
        self.insertion_assert_native_prefix_with_layout_v1(before, expected, layout, Some(source));
    }

    pub(crate) fn insertion_assert_native_prefix_with_layout_v1(
        &self,
        before: &DeviceInsertionMemorySnapshotV1,
        expected: DeviceInsertionPrefixV1<'_>,
        layout: Gfx942DeviceMemoryLayoutV1,
        source: Option<&[u8]>,
    ) {
        let after = self.insertion_memory_snapshot_v1();
        if source.is_none() {
            assert!(!expected.written);
            assert_eq!(expected.calls[3], 0);
            assert_eq!(expected.cpu_writable, None);
            assert_eq!(after.readback_calls, before.readback_calls);
            assert_eq!(after.initialized_bytes, before.initialized_bytes);
        }
        assert_eq!(
            core::array::from_fn::<_, 5, _>(|i| after.calls[i] - before.calls[i]),
            expected.calls,
            "exact currentness/reserve/alloc/CPU-map/GPU-map call prefix"
        );
        assert_eq!(
            &after.operations[..before.operations.len()],
            before.operations
        );
        assert_eq!(
            &after.operations[before.operations.len()..],
            expected.operations
        );
        let count = usize::from(expected.phase.is_some());
        assert_eq!(
            after.records.len(),
            before.records.len() + count,
            "exact returned native-record prefix"
        );
        assert_eq!(
            &after.records[..before.records.len()],
            before.records,
            "all original records unchanged"
        );
        assert_eq!(after.next_id, before.next_id + count as u64);
        assert_eq!(
            &after.cpu_inputs[..before.cpu_inputs.len()],
            before.cpu_inputs
        );
        assert_eq!(
            &after.gpu_inputs[..before.gpu_inputs.len()],
            before.gpu_inputs
        );
        assert_eq!(
            after.cpu_inputs.len() - before.cpu_inputs.len(),
            expected.calls[3]
        );
        assert_eq!(
            after.gpu_inputs.len() - before.gpu_inputs.len(),
            expected.calls[4]
        );
        let Some(phase) = expected.phase else {
            return;
        };
        let e = &self.fixture.engine;
        let record = e.device_memory.last().unwrap();
        assert_eq!(
            (
                record.id,
                record.generation,
                record.device,
                record.vm,
                record.layout
            ),
            (
                before.next_id,
                1,
                self.fixture.device.model_key(),
                self.fixture.vm,
                layout
            )
        );
        assert_eq!(format!("{:?}", record.phase), phase);
        assert_eq!(
            record.reservation,
            Some((record.gpu_va, layout.backing_bytes() as usize))
        );
        assert_eq!(record.handle.is_some(), expected.handle);
        assert!(!record.free_attempted);
        if expected.handle {
            let output = e.backend.last_allocation_output.unwrap();
            assert_eq!(output.flags, layout.uapi_flags());
            assert_eq!(
                (
                    record.handle,
                    record.mmap_offset,
                    record.gpu_va,
                    layout.backing_bytes()
                ),
                (
                    Some(output.handle),
                    output.mmap_offset,
                    output.va_addr,
                    output.size
                )
            );
        } else {
            assert_eq!(record.mmap_offset, 0);
        }
        assert_eq!(
            record.mapping.as_ref().map(|m| m.writable),
            expected.cpu_writable
        );
        if let Some(mapping) = &record.mapping {
            assert!(mapping.active);
            assert_eq!(mapping.address, record.gpu_va);
            assert_eq!(mapping.bytes.len(), layout.backing_bytes() as usize);
            assert_eq!(mapping.byte_offset, 0);
            if expected.written {
                let source = source.unwrap();
                assert_eq!(&mapping.bytes[..source.len()], source);
            } else {
                assert!(mapping.bytes.iter().all(|&b| b == 0));
            }
        } else if expected.written {
            let source = source.unwrap();
            assert_eq!(
                &after.initialized_bytes.as_ref().unwrap()[..source.len()],
                source
            );
            assert!(after.readback_calls > 0);
        }
        if expected.calls[3] == 1 {
            assert_eq!(
                after.cpu_inputs.last(),
                Some(&(
                    record.reservation.unwrap(),
                    record.mmap_offset,
                    layout.backing_bytes() as usize
                ))
            );
        }
        if expected.calls[4] == 1 {
            assert_eq!(after.gpu_inputs.last(), Some(&(record.handle.unwrap(), 0)));
        }
    }

    pub(crate) fn primary_prepare_device_initialization_v1(
        &mut self,
        root: &mut DeviceInitializationCustodyV1,
        alignment: u64,
    ) -> Result<(), MemorySessionError> {
        let f = &mut self.fixture;
        root.prepare_with_engine(&mut f.engine, f.device.model_key(), f.vm, alignment)
    }

    pub(crate) fn primary_prepare_device_allocation_v1(
        &mut self,
        root: &mut DeviceAllocationCustodyV1,
        requested_bytes: u64,
        alignment: u64,
    ) -> Result<(), MemorySessionError> {
        let f = &mut self.fixture;
        root.prepare_with_engine(
            &mut f.engine,
            f.device.model_key(),
            f.vm,
            requested_bytes,
            alignment,
        )
    }

    pub(crate) fn primary_retain_device_allocation_v1(&mut self, root: DeviceAllocationCustodyV1) {
        root.retain_with_engine(&mut self.fixture.engine);
    }

    pub(crate) fn primary_retain_device_initialization_v1(
        &mut self,
        root: DeviceInitializationCustodyV1,
    ) {
        root.retain_with_engine(&mut self.fixture.engine);
    }

    pub(crate) fn insertion_memory_snapshot_v1(&self) -> DeviceInsertionMemorySnapshotV1 {
        let e = &self.fixture.engine;
        let b = &e.backend;
        DeviceInsertionMemorySnapshotV1 {
            next_id: e.next_device_memory_id,
            calls: [
                b.currentness_calls,
                b.reserve_va_calls,
                b.alloc_calls,
                b.map_cpu_calls,
                b.map_gpu_calls,
            ],
            operations: b.operations.clone(),
            terminal: e.terminal_device_initialization.as_ref().map(root_snapshot),
            allocation_terminal: e
                .terminal_device_initialization
                .allocation_as_ref()
                .map(allocation_snapshot),
            terminal_occupied: e.terminal_device_initialization.is_some(),
            account: e
                .device_backing_account
                .as_ref()
                .map(DeviceBackingAccountV1::domain_identity_for_test),
            terminal_storage: e.terminal_device_initialization.storage_for_test(),
            readback_calls: b.last_unmapped_readback_calls,
            initialized_bytes: b.last_unmapped_bytes.clone(),
            records: e.device_memory.iter().map(snapshot).collect(),
            cpu_inputs: b.map_cpu_inputs.clone(),
            gpu_inputs: b.map_gpu_inputs.clone(),
        }
    }

    pub(crate) fn insertion_terminal_output_unavailable_v1(&self) -> bool {
        let slot = &self.fixture.engine.terminal_device_initialization;
        let initialized = slot.as_ref();
        let allocation = slot.allocation_as_ref();
        assert_eq!(
            usize::from(initialized.is_some()) + usize::from(allocation.is_some()),
            usize::from(slot.is_some())
        );
        initialized.is_some_and(|root| root.completed().is_err())
            || allocation.is_some_and(|root| root.completed().is_err())
    }

    pub(crate) fn insertion_arm_currentness_v1(&mut self, offset: usize, panic: bool) {
        let b = &mut self.fixture.engine.backend;
        assert!((1..=6).contains(&offset));
        let at = b.currentness_calls + offset;
        if panic {
            b.panic_currentness_at = Some(at);
        } else {
            b.fail_currentness_at = Some(at);
        }
    }

    pub(crate) fn insertion_arm_map_v1(&mut self, prefix: u32, errno: bool) {
        let b = &mut self.fixture.engine.backend;
        b.map_progress = prefix;
        b.map_errno = errno;
    }

    pub(crate) fn insertion_corrupt_readback_v1(&mut self) {
        self.fixture.engine.backend.corrupt_readback = true;
    }

    pub(crate) fn insertion_clear_faults_v1(&mut self) {
        let b = &mut self.fixture.engine.backend;
        b.fail_operation = None;
        b.panic_operation = None;
        b.fail_currentness_at = None;
        b.panic_currentness_at = None;
        b.map_progress = 1;
        b.map_errno = false;
        b.corrupt_readback = false;
    }

    pub(crate) fn insertion_assert_device_partition_v1(
        &self,
        leases: &[&Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>],
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
        external: Option<&DeviceInitializationCustodyV1>,
        attempted_id: u64,
        expected_layout: Gfx942DeviceMemoryLayoutV1,
        released: &[Gfx942DeviceMemoryIdentityV1],
    ) {
        self.insertion_assert_device_partition_with_root_v1(
            leases,
            authorities,
            external.map(|r| InsertionRootRef::Initialized(&r.0)),
            attempted_id,
            expected_layout,
            released,
        );
    }

    pub(crate) fn insertion_assert_allocation_partition_v1(
        &self,
        leases: &[&Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>],
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
        external: Option<&DeviceAllocationCustodyV1>,
        attempted_id: u64,
        expected_layout: Gfx942DeviceMemoryLayoutV1,
        released: &[Gfx942DeviceMemoryIdentityV1],
    ) {
        self.insertion_assert_device_partition_with_root_v1(
            leases,
            authorities,
            external.map(|r| InsertionRootRef::Allocation(&r.0)),
            attempted_id,
            expected_layout,
            released,
        );
    }

    fn insertion_assert_device_partition_with_root_v1(
        &self,
        leases: &[&Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>],
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
        external: Option<InsertionRootRef<'_>>,
        attempted_id: u64,
        expected_layout: Gfx942DeviceMemoryLayoutV1,
        released: &[Gfx942DeviceMemoryIdentityV1],
    ) {
        let e = &self.fixture.engine;
        let slot = &e.terminal_device_initialization;
        let initialized = slot.as_ref();
        let allocation = slot.allocation_as_ref();
        assert_eq!(
            usize::from(initialized.is_some()) + usize::from(allocation.is_some()),
            usize::from(slot.is_some())
        );
        let terminal = initialized
            .map(InsertionRootRef::Initialized)
            .or_else(|| allocation.map(InsertionRootRef::Allocation));
        assert!(
            !(external.is_some() && terminal.is_some()),
            "device custody rooted in exactly one place"
        );
        let root = external.or(terminal);
        let mut owned = leases
            .iter()
            .map(|l| (l.storage_identity(), l.layout()))
            .collect::<Vec<_>>();
        owned.extend(authorities.iter().map(|a| {
            let l = &a.lease;
            assert_eq!(
                (
                    a.facts.id,
                    a.facts.generation,
                    a.facts.device,
                    a.facts.vm,
                    a.facts.layout
                ),
                (l.id, l.generation, l.device, l.vm, l.layout)
            );
            (l.storage_identity(), l.layout())
        }));
        let pending = match root {
            Some(InsertionRootRef::Initialized(root)) => {
                match &root.lease {
                    InitializationLeaseV1::None => {}
                    InitializationLeaseV1::Unmapped(l) => {
                        owned.push((l.storage_identity(), l.layout()))
                    }
                    InitializationLeaseV1::Complete(output) => {
                        owned.push((output.lease.storage_identity(), output.lease.layout()))
                    }
                }
                root.native_started
                    && root.stage == Stage::Allocate
                    && matches!(root.lease, InitializationLeaseV1::None)
            }
            Some(InsertionRootRef::Allocation(root)) => {
                match &root.lease {
                    device_allocation::AllocationLeaseV1::None => {}
                    device_allocation::AllocationLeaseV1::Unmapped(l) => {
                        owned.push((l.storage_identity(), l.layout()))
                    }
                    device_allocation::AllocationLeaseV1::Mapped(l) => {
                        owned.push((l.storage_identity(), l.layout()))
                    }
                }
                root.native_started
                    && !root.progress.attempted
                    && matches!(root.lease, device_allocation::AllocationLeaseV1::None)
            }
            None => false,
        };
        let mut pending_records = 0;
        for record in &e.device_memory {
            let identity = Gfx942DeviceMemoryIdentityV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
            };
            let matches = owned
                .iter()
                .filter(|(id, _)| *id == identity)
                .collect::<Vec<_>>();
            if released.contains(&identity) {
                assert_eq!(released.iter().filter(|&&id| id == identity).count(), 1);
                assert!(matches.is_empty());
                assert!(record.is_fully_released());
                assert!(record.backing_charge.is_none());
                continue;
            }
            if pending && record.id == attempted_id {
                assert!(
                    matches.is_empty(),
                    "unreturned native record is not a fabricated lease"
                );
                assert_eq!(
                    (
                        record.generation,
                        record.device,
                        record.vm,
                        record.layout,
                        record.phase
                    ),
                    (
                        1,
                        self.fixture.device.model_key(),
                        self.fixture.vm,
                        expected_layout,
                        DeviceMemoryPhaseV1::Ambiguous
                    )
                );
                pending_records += 1;
            } else {
                assert_eq!(
                    matches.len(),
                    1,
                    "each original/new native record has exactly one owner"
                );
                assert_eq!(matches[0].1, record.layout);
            }
            for owner in authorities
                .iter()
                .filter(|a| a.lease.storage_identity() == identity)
            {
                assert_eq!(owner.facts.gpu_va, record.gpu_va);
            }
            if let Some(account) = &e.device_backing_account {
                assert!(record.backing_charge.as_ref().unwrap().matches(
                    account,
                    e.session_id,
                    record.device,
                    record.vm,
                    record.id,
                    record.generation,
                    record.layout
                ));
            } else {
                assert!(record.backing_charge.is_none());
            }
        }
        assert_eq!(
            owned.len() + pending_records + released.len(),
            e.device_memory.len()
        );
        assert!(pending_records <= 1);
        if let Some(account) = &e.device_backing_account {
            let usage = account.usage();
            let no_record_charge = usize::from(pending && pending_records == 0);
            let backing: u64 = e
                .device_memory
                .iter()
                .filter(|r| !r.is_fully_released())
                .map(|r| r.layout.backing_bytes())
                .sum();
            assert_eq!(
                usage.used_backing_bytes,
                backing + no_record_charge as u64 * expected_layout.backing_bytes()
            );
            assert_eq!(
                usage.used_allocation_records,
                (e.device_memory.len() - released.len() + no_record_charge) as u64
            );
            assert_eq!(
                usage.retained_records,
                e.device_memory.len() - released.len()
            );
            assert_eq!(usage.quarantined_records, no_record_charge);
            assert_eq!(usage.reserved_records, 0);
        }
    }
}

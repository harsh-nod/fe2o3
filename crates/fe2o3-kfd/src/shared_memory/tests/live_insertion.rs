//! Test observations only; every operation uses the original constructed engine.

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

impl PreparationMemoryFixtureV1 {
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
        let after = self.insertion_memory_snapshot_v1();
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
        let layout = device_memory_layout(
            source.len() as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
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
                assert_eq!(&mapping.bytes[..source.len()], source);
            } else {
                assert!(mapping.bytes.iter().all(|&b| b == 0));
            }
        } else if expected.written {
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
        self.fixture
            .engine
            .terminal_device_initialization
            .as_ref()
            .is_some_and(|root| root.completed().is_err())
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
        let e = &self.fixture.engine;
        let terminal = e.terminal_device_initialization.as_ref();
        assert!(
            !(external.is_some() && terminal.is_some()),
            "initializer rooted in exactly one place"
        );
        let root = external.map(|r| &r.0).or(terminal);
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
        if let Some(root) = root {
            match &root.lease {
                InitializationLeaseV1::None => {}
                InitializationLeaseV1::Unmapped(l) => {
                    owned.push((l.storage_identity(), l.layout()))
                }
                InitializationLeaseV1::Complete(output) => {
                    owned.push((output.lease.storage_identity(), output.lease.layout()))
                }
            }
        }
        let pending = root.is_some_and(|root| {
            root.native_started
                && root.stage == Stage::Allocate
                && matches!(root.lease, InitializationLeaseV1::None)
        });
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

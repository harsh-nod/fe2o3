use super::*;
use control_cleanup::{CleanupStageV1 as Stage, ControlCleanupObservationV1};
use std::panic::{AssertUnwindSafe, catch_unwind};
use transitions::ProjectionFaultV1 as Fault;

#[path = "data_tests.rs"]
mod data;

#[derive(Clone, Debug, Eq, PartialEq)]
enum MappingBytesV1 {
    Zeroes(usize),
    Dense(Vec<u8>),
}

impl MappingBytesV1 {
    fn capture(bytes: &[u8]) -> Self {
        const ZERO_PAGE: [u8; 4096] = [0; 4096];
        // Compare every byte without cloning large zero-filled context-save mappings.
        if bytes
            .chunks(ZERO_PAGE.len())
            .all(|chunk| chunk == &ZERO_PAGE[..chunk.len()])
        {
            Self::Zeroes(bytes.len())
        } else {
            Self::Dense(bytes.to_vec())
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Zeroes(len) => *len,
            Self::Dense(bytes) => bytes.len(),
        }
    }
}

#[test]
fn cleanup_mapping_snapshot_zero_encoding_preserves_every_byte_and_length() {
    for len in [0, 1, 4095, 4096, 4097, 8193] {
        let mut bytes = vec![0; len];
        let zero = MappingBytesV1::capture(&bytes);
        assert_eq!(zero, MappingBytesV1::Zeroes(len));
        assert_eq!(zero.len(), len);
        for index in [0, 4095, 4096, len.saturating_sub(1)] {
            if index >= len {
                continue;
            }
            bytes[index] = 0x5a;
            let changed = MappingBytesV1::capture(&bytes);
            assert_eq!(changed, MappingBytesV1::Dense(bytes.clone()));
            assert_eq!(changed.len(), len);
            assert_ne!(changed, zero);
            bytes[index] = 0;
            assert_eq!(MappingBytesV1::capture(&bytes), zero);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MappingSnapshot {
    address: u64,
    pointer: usize,
    capacity: usize,
    bytes: MappingBytesV1,
    byte_offset: usize,
    active: bool,
    writable: bool,
    reads: usize,
}

fn mapping_snapshot(m: &FakeMapping) -> MappingSnapshot {
    MappingSnapshot {
        address: m.address,
        pointer: m.bytes.as_ptr() as usize,
        capacity: m.bytes.capacity(),
        bytes: MappingBytesV1::capture(&m.bytes),
        byte_offset: m.byte_offset,
        active: m.active,
        writable: m.writable,
        reads: m.readback_calls.get(),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RecordSnapshot {
    id: u64,
    generation: u64,
    profile: SharedGttProfileV1,
    layout: SharedGttAllocationLayoutV1,
    gpu_va: u64,
    mmap_offset: u64,
    userptr: bool,
    reservation: Option<(u64, usize)>,
    mapping: Option<MappingSnapshot>,
    handle: Option<u64>,
    free_attempted: bool,
    phase: SharedAllocationPhaseV1,
    indexed: Option<usize>,
    charged: bool,
}

fn allocation_records(e: &SharedMemoryEngine<FakeBackend>) -> Vec<RecordSnapshot> {
    e.allocations
        .iter()
        .map(|r| RecordSnapshot {
            id: r.id,
            generation: r.generation,
            profile: r.profile,
            layout: r.layout,
            gpu_va: r.gpu_va,
            mmap_offset: r.mmap_offset,
            userptr: r.userptr,
            reservation: r.reservation,
            mapping: r.mapping.as_ref().map(mapping_snapshot),
            handle: r.handle,
            free_attempted: r.free_attempted,
            phase: r.phase,
            indexed: e.allocation_record_slots.get(&r.id).copied(),
            charged: r.host_backing_charge.is_some(),
        })
        .collect()
}

pub(crate) struct PristineControlRecordSnapshotV1 {
    session_id: u64,
    controls: Vec<SharedGttAllocationIdentityV1>,
    records: Vec<RecordSnapshot>,
    devices: Vec<DeviceSnapshot>,
    usage: (
        Option<Gfx942HostVisibleBackingUsageV1>,
        Option<Gfx942DeviceBackingUsageV1>,
    ),
}

impl PristineAbortMemoryFixtureV1 {
    pub(crate) fn control_record_snapshot(
        &self,
        controls: Vec<SharedGttAllocationIdentityV1>,
    ) -> PristineControlRecordSnapshotV1 {
        let e = &self.fixture.engine;
        let records = allocation_records(e);
        for (i, identity) in controls.iter().enumerate() {
            assert_eq!(identity.session_id, e.session_id);
            assert!(!controls[..i].contains(identity));
            let r = records.iter().find(|r| r.id == identity.id).unwrap();
            assert_eq!(r.generation, identity.generation);
            assert!(matches!(
                r.phase,
                SharedAllocationPhaseV1::GpuAccessibleMutable
                    | SharedAllocationPhaseV1::GpuAccessibleExecutable
            ));
        }
        PristineControlRecordSnapshotV1 {
            session_id: e.session_id,
            controls,
            records,
            devices: device_records(e),
            usage: (
                e.host_backing_account.as_ref().map(|a| a.usage()),
                self.fixture.usage(),
            ),
        }
    }
}

impl PristineControlRecordSnapshotV1 {
    pub(crate) fn assert_after(
        &self,
        memory: &PristineAbortMemoryFixtureV1,
        completed: usize,
        touched: usize,
    ) {
        assert!(completed <= touched && touched <= self.controls.len());
        assert_eq!(memory.fixture.engine.session_id, self.session_id);
        assert_eq!(device_records(&memory.fixture.engine), self.devices);
        assert_eq!(
            (
                memory
                    .fixture
                    .engine
                    .host_backing_account
                    .as_ref()
                    .map(|a| a.usage()),
                memory.fixture.usage()
            ),
            self.usage
        );
        let after = allocation_records(&memory.fixture.engine);
        assert_eq!(after.len(), self.records.len());
        for (before, after) in self.records.iter().zip(after) {
            let index = self.controls.iter().position(|id| id.id == before.id);
            if index.is_some_and(|i| i < completed) {
                let mut expected = before.clone();
                expected.mapping = None;
                expected.handle = None;
                expected.reservation = None;
                expected.free_attempted = true;
                expected.phase = SharedAllocationPhaseV1::Released;
                expected.indexed = None;
                assert_eq!(after, expected, "exact completed control prefix");
            } else if index.is_some_and(|i| i < touched) {
                assert_eq!((after.id, after.generation), (before.id, before.generation));
                assert_ne!(after.phase, SharedAllocationPhaseV1::Released);
            } else {
                assert_eq!(after, *before, "untouched native record changed");
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeviceSnapshot {
    identity: Gfx942DeviceMemoryIdentityV1,
    layout: Gfx942DeviceMemoryLayoutV1,
    gpu_va: u64,
    mmap_offset: u64,
    reservation: Option<(u64, usize)>,
    mapping: Option<MappingSnapshot>,
    handle: Option<u64>,
    free_attempted: bool,
    phase: DeviceMemoryPhaseV1,
    indexed: Option<usize>,
    charged: bool,
}

fn device_records(e: &SharedMemoryEngine<FakeBackend>) -> Vec<DeviceSnapshot> {
    e.device_memory
        .iter()
        .map(|r| DeviceSnapshot {
            identity: Gfx942DeviceMemoryIdentityV1 {
                id: r.id,
                generation: r.generation,
                device: r.device,
                vm: r.vm,
            },
            layout: r.layout,
            gpu_va: r.gpu_va,
            mmap_offset: r.mmap_offset,
            reservation: r.reservation,
            mapping: r.mapping.as_ref().map(mapping_snapshot),
            handle: r.handle,
            free_attempted: r.free_attempted,
            phase: r.phase,
            indexed: e.device_memory_record_slots.get(&r.id).copied(),
            charged: r.backing_charge.is_some(),
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Snapshot {
    model: MemoryLifecycleStateV1,
    identity: fe2o3_runtime_model::DeviceIdentityStateV1,
    certificate: Option<crate::queue::QueueCertificateSnapshotV1>,
    controls: Vec<ControlCleanupObservationV1>,
    records: Vec<RecordSnapshot>,
    devices: Vec<DeviceSnapshot>,
    phase: SharedMemorySessionPhaseV1,
    retained_va: u64,
    retained_device_bytes: u64,
    usage: (
        Option<Gfx942HostVisibleBackingUsageV1>,
        Option<Gfx942DeviceBackingUsageV1>,
    ),
    calls: Vec<CleanupCallV1>,
    operations: Vec<&'static str>,
    currentness: usize,
    process_poisoned: usize,
    storage: [(usize, usize); 2],
}

impl PristineAbortMemoryFixtureV1 {
    pub(crate) fn memory_snapshot(&self) -> Snapshot {
        Snapshot::from_fixture(
            &self.fixture,
            &self.fixture.foundation,
            self.process_poisoned,
        )
    }
}

impl crate::shared_memory::PreparationMemoryFixtureV1 {
    pub(crate) fn data_release_snapshot_v1(&self, queue: &QueueModelFoundationV1) -> Snapshot {
        self.assert_disposed_controls_v1();
        Snapshot::from_fixture(
            &self.fixture,
            self.coherent_active_foundation_v1(queue),
            self.data_release_process_poisoned,
        )
    }
}

impl Snapshot {
    fn from_fixture(
        f: &BackingConstructorFixture,
        foundation: &QueueModelFoundationV1,
        process_poisoned: usize,
    ) -> Self {
        let e = &f.engine;
        Snapshot {
            model: foundation.memory().clone(),
            identity: foundation.identity().clone(),
            certificate: foundation.certificate_snapshot_for_test(),
            controls: Vec::new(),
            records: allocation_records(e),
            devices: device_records(e),
            phase: e.phase,
            retained_va: e.retained_gpu_va_bytes,
            retained_device_bytes: e.retained_device_memory_bytes,
            usage: (
                e.host_backing_account.as_ref().map(|a| a.usage()),
                f.usage(),
            ),
            calls: e.backend.cleanup_calls.clone(),
            operations: e.backend.operations.clone(),
            currentness: e.backend.currentness_calls,
            process_poisoned,
            storage: [
                (e.allocations.as_ptr() as usize, e.allocations.capacity()),
                (
                    e.device_memory.as_ptr() as usize,
                    e.device_memory.capacity(),
                ),
            ],
        }
    }
}

impl Snapshot {
    pub(crate) fn assert_control_transition(
        &self,
        memory: &PristineAbortMemoryFixtureV1,
        order: &[SharedGttAllocationIdentityV1],
        completed: usize,
        unmapped: bool,
        partial_calls: usize,
        active_native: Option<(bool, usize, bool, bool)>,
    ) {
        let after = memory.memory_snapshot();
        let mut expected_model = self.model.clone();
        for (index, id) in order
            .iter()
            .enumerate()
            .take(completed + usize::from(unmapped))
        {
            let (reservation, allocation, mapping) =
                model_keys(memory.fixture.vm, id.id, id.generation);
            expected_model = project_unmap(&expected_model, mapping).unwrap();
            if index < completed {
                expected_model =
                    project_release(&expected_model, reservation, allocation, mapping).unwrap();
            }
        }
        assert_eq!(
            after.model, expected_model,
            "exact committed cleanup prefix"
        );
        let mut expected_calls = self.calls.clone();
        for (index, id) in order
            .iter()
            .enumerate()
            .take(completed + usize::from(partial_calls > 0))
        {
            assert_eq!(id.session_id, memory.fixture.engine.session_id);
            let r = self
                .records
                .iter()
                .find(|r| (r.id, r.generation) == (id.id, id.generation))
                .unwrap();
            let m = r.mapping.as_ref().unwrap();
            let reservation = r.reservation.unwrap();
            let calls = [
                CleanupCallV1::UnmapGpu(r.handle.unwrap(), 0),
                CleanupCallV1::UnmapCpu(m.address, m.pointer, m.bytes.len()),
                CleanupCallV1::Free(r.handle.unwrap()),
                CleanupCallV1::ReleaseVa(reservation.0, reservation.1),
            ];
            expected_calls.extend(calls.into_iter().take(if index < completed {
                4
            } else {
                partial_calls
            }));
        }
        assert_eq!(
            after.calls, expected_calls,
            "exact native identities and order"
        );
        assert_eq!(after.devices, self.devices);
        assert_eq!(after.usage, self.usage);
        assert_eq!(after.storage, self.storage);
        assert_eq!(after.identity, self.identity);
        let mut released_va = 0;
        for r in &self.records {
            let index = order
                .iter()
                .position(|id| id.id == r.id && id.generation == r.generation);
            let expectation = if index.is_some_and(|i| i < completed) {
                Some((true, 4, true, true))
            } else if index == Some(completed) {
                active_native
            } else {
                None
            };
            let expected = if let Some((unmapped, prefix, settled, free_attempted)) = expectation {
                let mut record = expected_native_record(r, unmapped, prefix, settled);
                record.free_attempted |= free_attempted;
                if settled {
                    released_va += r.layout.gpu_va_bytes();
                }
                record
            } else {
                r.clone()
            };
            assert_eq!(
                after.records.iter().find(|a| a.id == r.id).unwrap(),
                &expected,
                "exact native record"
            );
        }
        assert_eq!(after.records.len(), self.records.len());
        assert_eq!(after.retained_va, self.retained_va - released_va);
        assert_eq!(after.process_poisoned, self.process_poisoned);
        match (&self.certificate, &after.certificate) {
            (Some(a), Some(b)) => {
                assert_eq!(
                    (a.0, a.1, a.2, a.3, a.4, a.5),
                    (b.0, b.1, b.2, b.3, b.4, b.5)
                );
            }
            (None, None) => {}
            _ => panic!("cleanup changed certificate identity"),
        }
    }

    pub(crate) fn assert_certificate_revision(&self, revision: u64) {
        assert_eq!(self.certificate.as_ref().map(|c| c.6), Some(revision));
    }
}

struct Fixture {
    memory: PristineAbortMemoryFixtureV1,
    controls: [ControlCleanupCustodyV1; 3],
    _hosts: [Host; 2],
    _devices: [Gfx942DeviceMemoryDispatchAuthorityV1; 3],
}

impl Fixture {
    fn new(configured: bool) -> Self {
        let mut memory = PristineAbortMemoryFixtureV1::new_configured(configured);
        let code0 = memory.code();
        let code1 = memory.code();
        let kernarg = memory.kernarg();
        let hosts = [memory.host(), memory.host()];
        let devices = [memory.device(), memory.device(), memory.device()];
        Self {
            memory,
            controls: [
                ControlCleanupCustodyV1::kernarg(kernarg.into_token()),
                ControlCleanupCustodyV1::code(code1.into_token()),
                ControlCleanupCustodyV1::code(code0.into_token()),
            ],
            _hosts: hosts,
            _devices: devices,
        }
    }

    fn release(&mut self, index: usize) -> Result<(), MemorySessionError> {
        self.memory.release_control(&mut self.controls[index])
    }

    fn snapshot(&self) -> Snapshot {
        let mut snapshot = self.memory.memory_snapshot();
        snapshot.controls = self
            .controls
            .iter()
            .map(ControlCleanupCustodyV1::observation)
            .collect();
        snapshot
    }

    fn clear_faults(&mut self) {
        self.memory.control_failure = None;
        self.memory.control_unmap = None;
        self.memory.projection_fault = None;
        let b = &mut self.memory.fixture.engine.backend;
        b.fail_operation = None;
        b.panic_operation = None;
        b.fail_currentness_at = None;
        b.panic_currentness_at = None;
        b.unmap_progress = 1;
        b.unmap_errno = false;
    }

    fn reject_retry(&mut self, index: usize) {
        self.clear_faults();
        let before = self.snapshot();
        let f = &mut self.memory.fixture;
        assert!(matches!(
            control_cleanup::release_v1(
                &mut f.engine,
                &mut control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm),
                &mut self.controls[index],
                || panic!("retry cannot enter revision preflight"),
            ),
            Err(MemorySessionError::InvalidAllocationAuthority)
        ));
        assert_eq!(
            self.snapshot(),
            before,
            "retry changed retained cleanup state"
        );
    }

    fn expected_model(
        &self,
        before: &Snapshot,
        completed: usize,
        unmapped: bool,
    ) -> MemoryLifecycleStateV1 {
        let mut model = before.model.clone();
        for (index, control) in before
            .controls
            .iter()
            .enumerate()
            .take(completed + usize::from(unmapped))
        {
            let (reservation, allocation, mapping) = model_keys(
                self.memory.fixture.vm,
                control.identity.id,
                control.identity.generation,
            );
            model = project_unmap(&model, mapping).unwrap();
            if index < completed {
                model = project_release(&model, reservation, allocation, mapping).unwrap();
            }
        }
        model
    }

    fn check_unrelated(&self, before: &Snapshot) {
        let after = self.snapshot();
        assert_eq!(after.identity, before.identity);
        if let Some(prior) = &before.certificate {
            let current = after
                .certificate
                .as_ref()
                .expect("certificate remains retained");
            assert_eq!(
                (
                    current.0, current.1, current.2, current.3, current.4, current.5
                ),
                (prior.0, prior.1, prior.2, prior.3, prior.4, prior.5)
            );
            let f = &self.memory.fixture;
            f.foundation
                .authenticate(f.engine.session_id, f.device, f.vm, prior.0)
                .unwrap();
        } else {
            assert!(after.certificate.is_none());
        }
        assert_eq!(after.devices, before.devices);
        assert_eq!(after.usage, before.usage);
        assert_eq!(after.storage, before.storage);
        for record in &before.records {
            if record.profile == SharedGttProfileV1::HostVisibleCoherent {
                assert_eq!(
                    after.records.iter().find(|r| r.id == record.id).unwrap(),
                    record
                );
            }
        }
        for (index, control) in after.controls.iter().enumerate() {
            if !control.started {
                assert_eq!(control, &before.controls[index]);
                assert_eq!(
                    record(&after, index),
                    record(before, index),
                    "unvisited native control changed"
                );
            } else {
                assert_eq!(control.identity, before.controls[index].identity);
                assert_eq!(control.layout, before.controls[index].layout);
                assert_eq!(control.profile_type, before.controls[index].profile_type);
                let expected_type = if control.owner == "Mapped" {
                    before.controls[index].state_type
                } else if index == 0 {
                    std::any::TypeId::of::<GttCpuWritableV1>()
                } else {
                    std::any::TypeId::of::<GttExecutableImmutableV1>()
                };
                assert_eq!(control.state_type, expected_type);
                assert_eq!(control.native_disposed, control.owner == "NativeDisposed");
                assert_eq!(control.failed, control.stage != Stage::Complete);
                if control.stage == Stage::Complete {
                    assert_eq!(
                        record(&after, index),
                        &expected_record(before, index, true, 4, true)
                    );
                }
            }
        }
    }
}

fn record(before: &Snapshot, index: usize) -> &RecordSnapshot {
    before
        .records
        .iter()
        .find(|r| r.id == before.controls[index].identity.id)
        .unwrap()
}

fn expected_calls(before: &Snapshot, completed: usize, partial: usize) -> Vec<CleanupCallV1> {
    let mut calls = before.calls.clone();
    for index in 0..(completed + usize::from(partial != 0)) {
        let r = record(before, index);
        let mapping = r.mapping.as_ref().unwrap();
        let reservation = r.reservation.unwrap();
        let sequence = [
            CleanupCallV1::UnmapGpu(r.handle.unwrap(), 0),
            CleanupCallV1::UnmapCpu(mapping.address, mapping.pointer, mapping.bytes.len()),
            CleanupCallV1::Free(r.handle.unwrap()),
            CleanupCallV1::ReleaseVa(reservation.0, reservation.1),
        ];
        calls.extend(
            sequence
                .into_iter()
                .take(if index < completed { 4 } else { partial }),
        );
    }
    calls
}

fn expected_record(
    before: &Snapshot,
    index: usize,
    unmapped: bool,
    native_prefix: usize,
    settled: bool,
) -> RecordSnapshot {
    expected_native_record(record(before, index), unmapped, native_prefix, settled)
}

fn expected_native_record(
    before: &RecordSnapshot,
    unmapped: bool,
    native_prefix: usize,
    settled: bool,
) -> RecordSnapshot {
    let mut r = before.clone();
    if unmapped {
        r.phase = if r.profile == SharedGttProfileV1::Kernarg {
            SharedAllocationPhaseV1::CpuWritable
        } else {
            SharedAllocationPhaseV1::ExecutableImmutable
        };
    }
    if native_prefix >= 2 {
        r.mapping = None;
    }
    if native_prefix >= 3 {
        r.handle = None;
        r.free_attempted = true;
    }
    if native_prefix >= 4 {
        r.reservation = None;
    }
    if settled {
        r.phase = SharedAllocationPhaseV1::Released;
        r.indexed = None;
    }
    r
}

fn check_completed_prefix(f: &Fixture, before: &Snapshot, completed: usize) {
    let after = f.snapshot();
    for index in 0..completed {
        assert!(f.controls[index].is_complete());
        assert_eq!(
            after.controls[index].identity,
            before.controls[index].identity
        );
        assert_eq!(
            record(&after, index),
            &expected_record(before, index, true, 4, true)
        );
    }
    let released: u64 = (0..completed)
        .map(|i| record(before, i).layout.gpu_va_bytes())
        .sum();
    assert_eq!(after.retained_va, before.retained_va - released);
}

#[test]
fn cleanup_success_commits_original_model_and_exact_native_control_order() {
    for configured in [false, true] {
        let mut f = Fixture::new(configured);
        let before = f.snapshot();
        for i in 0..3 {
            f.release(i).unwrap();
            let after = f.snapshot();
            assert_eq!(after.model, f.expected_model(&before, i + 1, false));
            assert_eq!(after.calls, expected_calls(&before, i + 1, 0));
            assert_eq!(after.phase, SharedMemorySessionPhaseV1::Active);
            check_completed_prefix(&f, &before, i + 1);
            f.check_unrelated(&before);
            f.reject_retry(i);
        }
    }
}

#[test]
fn cleanup_native_failure_retains_typed_control_and_exact_destructive_prefix() {
    for configured in [false, true] {
        for index in 0..3 {
            for (operation_index, operation) in
                ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                    .into_iter()
                    .enumerate()
            {
                for panic in [false, true] {
                    let mut f = Fixture::new(configured);
                    let before = f.snapshot();
                    for prior in 0..index {
                        f.release(prior).unwrap();
                    }
                    f.memory.fail_control(index + 1, operation, panic);
                    let result = catch_unwind(AssertUnwindSafe(|| f.release(index)));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", operation))
                        );
                    } else {
                        assert!(
                            matches!(result.unwrap(), Err(MemorySessionError::Injected(op)) if op == operation)
                        );
                    }
                    let after = f.snapshot();
                    let owner = &after.controls[index];
                    assert_eq!(owner.identity, before.controls[index].identity);
                    assert_eq!(owner.layout, before.controls[index].layout);
                    assert_eq!(owner.profile_type, before.controls[index].profile_type);
                    assert_eq!(
                        owner.owner,
                        if operation_index == 0 {
                            "Mapped"
                        } else {
                            "Unmapped"
                        }
                    );
                    assert_eq!(
                        owner.stage,
                        if operation_index == 0 {
                            Stage::NativeUnmap
                        } else {
                            Stage::NativeRelease
                        }
                    );
                    assert!(owner.started && owner.failed && !owner.native_disposed);
                    let returned = (!panic).then_some(false);
                    let mut disposal = [(false, None); 3];
                    if operation_index == 0 {
                        assert_eq!(owner.unmap, (true, returned, (!panic).then_some(1)));
                    } else {
                        assert_eq!(owner.unmap, (true, Some(true), Some(1)));
                        for item in disposal.iter_mut().take(operation_index - 1) {
                            *item = (true, Some(true));
                        }
                        disposal[operation_index - 1] = (true, returned);
                    }
                    assert_eq!(owner.disposal, disposal);
                    let mut expected = expected_record(
                        &before,
                        index,
                        operation_index > 0,
                        operation_index,
                        false,
                    );
                    if operation_index == 2 {
                        expected.free_attempted = true;
                    }
                    assert_eq!(record(&after, index), &expected);
                    assert_eq!(
                        after.model,
                        f.expected_model(&before, index, operation_index > 0)
                    );
                    assert_eq!(
                        after.calls,
                        expected_calls(&before, index, operation_index + 1)
                    );
                    assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
                    assert_eq!(&after.controls[index + 1..], &before.controls[index + 1..]);
                    check_completed_prefix(&f, &before, index);
                    f.check_unrelated(&before);
                    f.reject_retry(index);
                }
            }
        }
    }
}

#[test]
fn cleanup_unmap_outcomes_preserve_prefix_errno_precedence_without_retag() {
    for index in 0..3 {
        for (prefix, errno, error) in [
            (0, false, "shared UNMAP_MEMORY_FROM_GPU full prefix"),
            (
                2,
                false,
                "shared UNMAP_MEMORY_FROM_GPU cumulative n_success",
            ),
            (0, true, "unmap_gpu"),
            (1, true, "unmap_gpu"),
            (2, true, "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"),
        ] {
            let mut f = Fixture::new(true);
            let before = f.snapshot();
            for prior in 0..index {
                f.release(prior).unwrap();
            }
            f.memory.unmap_control(index + 1, prefix, errno);
            match f.release(index).unwrap_err() {
                MemorySessionError::Injected(op) if prefix <= 1 && errno => assert_eq!(op, error),
                MemorySessionError::KernelResultMalformed(detail) => assert_eq!(detail, error),
                other => panic!("unexpected unmap error: {other:?}"),
            }
            let after = f.snapshot();
            assert_eq!(after.controls[index].owner, "Mapped");
            assert_eq!(
                after.controls[index].state_type,
                before.controls[index].state_type
            );
            assert_eq!(
                after.controls[index].unmap,
                (true, Some(!errno), Some(prefix))
            );
            assert_eq!(record(&after, index), record(&before, index));
            assert_eq!(after.calls, expected_calls(&before, index, 1));
            assert_eq!(after.model, f.expected_model(&before, index, false));
            assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
            check_completed_prefix(&f, &before, index);
            f.check_unrelated(&before);
            f.reject_retry(index);
        }
    }
}

#[test]
fn cleanup_projection_failures_keep_native_disposal_distinct_from_model_commit() {
    for configured in [false, true] {
        for index in 0..3 {
            for stage in [
                Stage::UnmapProjection,
                Stage::UnmapCommit,
                Stage::ReleaseProjection,
                Stage::ReleaseCommit,
            ] {
                for panic in [false, true] {
                    let mut f = Fixture::new(configured);
                    let before = f.snapshot();
                    for prior in 0..index {
                        f.release(prior).unwrap();
                    }
                    f.memory.projection_fault = Some((
                        index + 1,
                        stage,
                        if panic { Fault::Panic } else { Fault::Error },
                    ));
                    let result = catch_unwind(AssertUnwindSafe(|| f.release(index)));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                            Some(&("control cleanup projection", stage))
                        );
                    } else {
                        assert!(matches!(
                            result.unwrap(),
                            Err(MemorySessionError::Injected("control cleanup projection"))
                        ));
                    }
                    let after = f.snapshot();
                    let disposed = stage == Stage::ReleaseCommit;
                    let unmapped = matches!(stage, Stage::ReleaseProjection | Stage::ReleaseCommit);
                    assert_eq!(after.model, f.expected_model(&before, index, unmapped));
                    assert_eq!(
                        after.controls[index].owner,
                        if disposed {
                            "NativeDisposed"
                        } else {
                            "Unmapped"
                        }
                    );
                    assert_eq!(
                        after.controls[index].identity,
                        before.controls[index].identity
                    );
                    assert_eq!(after.controls[index].stage, stage);
                    assert!(after.controls[index].failed);
                    assert!(!f.controls[index].is_complete());
                    assert_eq!(after.controls[index].native_disposed, disposed);
                    assert_eq!(
                        record(&after, index),
                        &expected_record(
                            &before,
                            index,
                            true,
                            if disposed { 4 } else { 1 },
                            disposed
                        )
                    );
                    assert_eq!(
                        after.calls,
                        expected_calls(&before, index, if disposed { 4 } else { 1 })
                    );
                    let reclaimed: u64 = (0..index + usize::from(disposed))
                        .map(|i| record(&before, i).layout.gpu_va_bytes())
                        .sum();
                    assert_eq!(after.retained_va, before.retained_va - reclaimed);
                    assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
                    assert_eq!(&after.controls[index + 1..], &before.controls[index + 1..]);
                    f.check_unrelated(&before);
                    f.reject_retry(index);
                }
            }
        }
    }
}

#[test]
fn cleanup_currentness_sweep_retains_disposed_receipt_before_final_check() {
    for configured in [false, true] {
        for offset in 1..=18 {
            for panic in [false, true] {
                let mut f = Fixture::new(configured);
                let before = f.snapshot();
                let index = (offset - 1) / 6;
                let point = (offset - 1) % 6 + 1;
                f.memory.fail_currentness(offset, panic);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    for i in 0..=index {
                        f.release(i)?;
                    }
                    Ok::<(), MemorySessionError>(())
                }));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("currentness"))
                    ));
                }
                let after = f.snapshot();
                let owner = &after.controls[index];
                assert_eq!(owner.identity, before.controls[index].identity);
                assert_eq!(
                    owner.owner,
                    if point <= 2 {
                        "Mapped"
                    } else if point == 6 {
                        "NativeDisposed"
                    } else {
                        "Unmapped"
                    }
                );
                assert_eq!(owner.native_disposed, point == 6);
                assert_eq!(
                    owner.stage,
                    if point <= 2 {
                        Stage::NativeUnmap
                    } else {
                        Stage::NativeRelease
                    }
                );
                assert_eq!(
                    owner.unmap,
                    if point == 1 {
                        (false, None, None)
                    } else {
                        (true, Some(true), Some(1))
                    }
                );
                let successful = point.saturating_sub(3);
                assert_eq!(
                    owner.disposal,
                    std::array::from_fn(|i| if i < successful {
                        (true, Some(true))
                    } else {
                        (false, None)
                    })
                );
                assert!(owner.failed);
                let native_prefix = point.saturating_sub(2).max(usize::from(point >= 2));
                assert_eq!(
                    record(&after, index),
                    &expected_record(&before, index, point >= 3, native_prefix, false)
                );
                assert_eq!(after.model, f.expected_model(&before, index, point >= 3));
                assert_eq!(after.calls, expected_calls(&before, index, native_prefix));
                assert_eq!(after.currentness - before.currentness, offset);
                assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
                check_completed_prefix(&f, &before, index);
                f.check_unrelated(&before);
                f.reject_retry(index);
            }
        }
    }
}

#[test]
fn cleanup_preserves_two_separate_revision_preflights_and_last_valid_commit() {
    for headroom in 0..=2 {
        let mut f = Fixture::new(false);
        let engine = &f.memory.fixture.engine;
        f.memory
            .fixture
            .foundation
            .mint_invariant_certificate(
                engine.session_id,
                f.memory.fixture.device,
                f.memory.fixture.vm,
            )
            .unwrap();
        f.memory
            .fixture
            .foundation
            .set_certificate_revision_for_test(u64::MAX - headroom)
            .unwrap();
        let before = f.snapshot();
        let result = f.release(0);
        let after = f.snapshot();
        if headroom == 2 {
            result.unwrap();
            assert!(f.controls[0].is_complete());
            assert_eq!(after.process_poisoned, 0);
            assert_eq!(after.model, f.expected_model(&before, 1, false));
        } else {
            assert!(matches!(
                result,
                Err(MemorySessionError::Model(
                    "queue foundation certificate revision exhausted"
                ))
            ));
            assert_eq!(after.process_poisoned, 1);
            assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(
                after.controls[0].owner,
                if headroom == 0 { "Mapped" } else { "Unmapped" }
            );
            assert_eq!(after.model, f.expected_model(&before, 0, headroom == 1));
        }
        assert_eq!(
            after.calls,
            expected_calls(
                &before,
                usize::from(headroom == 2),
                usize::from(headroom == 1)
            )
        );
        assert_eq!(after.certificate.as_ref().unwrap().6, u64::MAX);
        assert!(
            f.memory
                .fixture
                .foundation
                .preflight_memory_transition_revisions(1)
                .is_err()
        );
        assert_eq!(
            after.controls[0].stage,
            if headroom == 0 {
                Stage::UnmapPreflight
            } else if headroom == 1 {
                Stage::ReleasePreflight
            } else {
                Stage::Complete
            }
        );
        assert_eq!(
            after.controls[0].unmap,
            if headroom == 0 {
                (false, None, None)
            } else {
                (true, Some(true), Some(1))
            }
        );
        assert_eq!(
            after.controls[0].disposal,
            [(headroom == 2, (headroom == 2).then_some(true)); 3]
        );
        let complete = headroom == 2;
        assert_eq!(
            record(&after, 0),
            &expected_record(
                &before,
                0,
                headroom > 0,
                if complete {
                    4
                } else {
                    usize::from(headroom == 1)
                },
                complete,
            )
        );
        assert_eq!(
            after.retained_va,
            before.retained_va
                - if complete {
                    record(&before, 0).layout.gpu_va_bytes()
                } else {
                    0
                }
        );
        assert_eq!(
            after.phase,
            if complete {
                SharedMemorySessionPhaseV1::Active
            } else {
                SharedMemorySessionPhaseV1::Quarantined
            }
        );
        f.check_unrelated(&before);
        f.reject_retry(0);
    }
}

#[test]
fn cleanup_actual_commit_rejection_keeps_unmapped_or_disposed_custody() {
    for stage in [Stage::UnmapCommit, Stage::ReleaseCommit] {
        let mut f = Fixture::new(true);
        let fixture = &mut f.memory.fixture;
        fixture
            .foundation
            .mint_invariant_certificate(fixture.engine.session_id, fixture.device, fixture.vm)
            .unwrap();
        let before = f.snapshot();
        f.memory.projection_fault = Some((1, stage, Fault::ExhaustRevision));
        assert!(matches!(
            f.release(0),
            Err(MemorySessionError::Model(
                "queue foundation certificate revision exhausted"
            ))
        ));
        let after = f.snapshot();
        assert_eq!(after.controls[0].stage, stage);
        assert_eq!(
            after.controls[0].owner,
            if stage == Stage::ReleaseCommit {
                "NativeDisposed"
            } else {
                "Unmapped"
            }
        );
        assert_eq!(
            after.model,
            f.expected_model(&before, 0, stage == Stage::ReleaseCommit)
        );
        assert_eq!(after.phase, SharedMemorySessionPhaseV1::Quarantined);
        let disposed = stage == Stage::ReleaseCommit;
        assert_eq!(
            record(&after, 0),
            &expected_record(&before, 0, true, if disposed { 4 } else { 1 }, disposed)
        );
        assert_eq!(
            after.calls,
            expected_calls(&before, 0, if disposed { 4 } else { 1 })
        );
        assert_eq!(after.controls[0].unmap, (true, Some(true), Some(1)));
        assert_eq!(
            after.controls[0].disposal,
            [(disposed, disposed.then_some(true)); 3]
        );
        assert_eq!(
            after.retained_va,
            before.retained_va
                - if disposed {
                    record(&before, 0).layout.gpu_va_bytes()
                } else {
                    0
                }
        );
        assert_eq!(after.process_poisoned, 0);
        assert_eq!(after.certificate.as_ref().unwrap().6, u64::MAX);
        assert!(
            f.memory
                .fixture
                .foundation
                .preflight_memory_transition_revisions(1)
                .is_err()
        );
        f.check_unrelated(&before);
        f.reject_retry(0);
    }
}

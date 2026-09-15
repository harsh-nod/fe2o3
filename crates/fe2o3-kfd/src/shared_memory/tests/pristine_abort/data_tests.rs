use super::*;
use crate::queue::dispatch_binding::{
    Gfx942FixedDispatchDataV1, Gfx942FixedDispatchStorageIdentityV1 as FixedIdentity,
};
use crate::sdma::Gfx942SdmaBufferStorageIdentityV1 as Identity;
use crate::shared_memory::data_cleanup::{self, DataCleanupCustodyV1, DispatchDataReleaseV1};

impl PristineAbortMemoryFixtureV1 {
    pub(crate) fn fail_data(&mut self, ordinal: usize, operation: &'static str, panic: bool) {
        self.data_failure = Some((ordinal, operation, panic));
    }

    pub(crate) fn unmap_data(&mut self, ordinal: usize, prefix: u32, errno: bool) {
        self.data_unmap = Some((ordinal, prefix, errno));
    }

    pub(crate) fn fail_data_projection(&mut self, ordinal: usize, stage: Stage, panic: bool) {
        self.data_projection_fault = Some((
            ordinal,
            stage,
            if panic { Fault::Panic } else { Fault::Error },
        ));
    }

    pub(crate) fn exhaust_data_commit(&mut self, ordinal: usize, release: bool) {
        let f = &mut self.fixture;
        f.foundation
            .mint_invariant_certificate(f.engine.session_id, f.device, f.vm)
            .unwrap();
        self.data_projection_fault = Some((
            ordinal,
            if release {
                Stage::ReleaseCommit
            } else {
                Stage::UnmapCommit
            },
            Fault::ExhaustRevision,
        ));
    }

    pub(crate) fn clear_cleanup_faults(&mut self) {
        self.data_failure = None;
        self.data_unmap = None;
        self.data_projection_fault = None;
        self.control_failure = None;
        self.control_unmap = None;
        self.projection_fault = None;
        let b = &mut self.fixture.engine.backend;
        b.fail_operation = None;
        b.panic_operation = None;
        b.fail_currentness_at = None;
        b.panic_currentness_at = None;
        b.unmap_progress = 1;
        b.unmap_errno = false;
    }

    fn typed_data(&mut self, kind: usize) -> Gfx942FixedDispatchDataV1 {
        match kind {
            0 => Gfx942FixedDispatchDataV1::uninitialized(self.device().into_lease()),
            1 => Gfx942FixedDispatchDataV1::host_visible_uninitialized(self.host().into_token()),
            2 => {
                let bytes = vec![0x5a; 17].into_boxed_slice();
                let content = content(&bytes);
                let f = &mut self.fixture;
                let initialized = crate::shared_memory::device_initialization::initialize_bytes_v1(
                    &mut f.engine,
                    f.device.model_key(),
                    f.vm,
                    bytes,
                    4,
                    content,
                )
                .unwrap();
                Gfx942FixedDispatchDataV1::initialized(initialized)
            }
            3 => Gfx942FixedDispatchDataV1::host_visible_initialized(
                crate::shared_memory::coherent_initialization::initialize_v1(self, &[0x5a; 17])
                    .unwrap(),
            ),
            // Synthetic completed-dispatch typing; not a hardware completion observation.
            4 => Gfx942FixedDispatchDataV1::initialized_after_dispatch_for_test(
                self.device().into_lease(),
            ),
            _ => unreachable!(),
        }
    }
}

impl crate::shared_memory::coherent_initialization::CoherentInitializationV1
    for PristineAbortMemoryFixtureV1
{
    fn allocate(
        &mut self,
        length: usize,
    ) -> Result<crate::shared_memory::coherent_initialization::CpuAllocation, MemorySessionError>
    {
        Ok(self.allocate::<HostVisibleCoherentGttV1>(length))
    }
    fn copy(
        &mut self,
        allocation: crate::shared_memory::coherent_initialization::CpuAllocation,
        source: &[u8],
    ) -> Result<crate::shared_memory::coherent_initialization::CpuAllocation, MemorySessionError>
    {
        transitions::copy_coherent_v1(&mut self.fixture.engine, allocation, source)
    }
    fn map(
        &mut self,
        allocation: crate::shared_memory::coherent_initialization::CpuAllocation,
    ) -> Result<crate::shared_memory::coherent_initialization::MappedAllocation, MemorySessionError>
    {
        Ok(self.map(allocation))
    }
}

impl DispatchDataReleaseV1 for PristineAbortMemoryFixtureV1 {
    fn release_data(
        &mut self,
        custody: &mut DataCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.data_ordinal += 1;
        if let Some((ordinal, operation, panic)) = self.data_failure
            && ordinal == self.data_ordinal
        {
            self.fail(operation, panic);
        }
        if let Some((ordinal, prefix, errno)) = self.data_unmap
            && ordinal == self.data_ordinal
        {
            self.fixture.engine.backend.unmap_progress = prefix;
            self.fixture.engine.backend.unmap_errno = errno;
        }
        let f = &mut self.fixture;
        let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
        projection.fault = self
            .data_projection_fault
            .filter(|(ordinal, _, _)| *ordinal == self.data_ordinal)
            .map(|(_, stage, fault)| (stage, fault));
        data_cleanup::release_v1(&mut f.engine, &mut projection, custody, || {
            self.process_poisoned += 1
        })
    }
}

impl DispatchDataReleaseV1 for crate::shared_memory::PreparationMemoryFixtureV1 {
    fn release_data(
        &mut self,
        custody: &mut DataCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        let f = &mut self.fixture;
        data_cleanup::release_v1(
            &mut f.engine,
            &mut control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm),
            custody,
            || panic!("unexpected prepared data cleanup preflight failure"),
        )
    }
}

fn shared_calls(r: &RecordSnapshot) -> [CleanupCallV1; 4] {
    let m = r.mapping.as_ref().unwrap();
    let va = r.reservation.unwrap();
    [
        CleanupCallV1::UnmapGpu(r.handle.unwrap(), 0),
        CleanupCallV1::UnmapCpu(m.address, m.pointer, m.bytes.len()),
        CleanupCallV1::Free(r.handle.unwrap()),
        CleanupCallV1::ReleaseVa(va.0, va.1),
    ]
}

impl Snapshot {
    fn assert_data_identity(&self, id: Identity, active: &DataCleanupObservationV1) {
        if active.owner == "Original" {
            return;
        }
        match id {
            Identity::Host(id) => {
                let r = self
                    .records
                    .iter()
                    .find(|r| (r.id, r.generation) == (id.id, id.generation))
                    .unwrap();
                let host = active
                    .host
                    .as_ref()
                    .expect("retained coherent token or receipt");
                assert_eq!(host.identity, id);
                assert_eq!(host.layout, r.layout);
                assert_eq!(
                    host.profile_type,
                    std::any::TypeId::of::<HostVisibleCoherentGttV1>()
                );
                assert_eq!(
                    host.state_type,
                    if active.owner == "Mapped" {
                        std::any::TypeId::of::<GttGpuAccessibleMutableV1>()
                    } else {
                        std::any::TypeId::of::<GttCpuWritableV1>()
                    }
                );
                assert_eq!(active.device, None);
            }
            Identity::Device(id) => {
                let r = self.devices.iter().find(|r| r.identity == id).unwrap();
                assert_eq!(
                    active.device,
                    Some((id, r.layout)),
                    "exact retained device token or receipt"
                );
                assert!(active.host.is_none());
            }
        }
    }

    pub(crate) fn assert_data_prefix(
        &self,
        memory: &PristineAbortMemoryFixtureV1,
        controls: &[SharedGttAllocationIdentityV1],
        order: &[Identity],
        completed: usize,
        active: Option<&DataCleanupObservationV1>,
    ) {
        let after = memory.memory_snapshot();
        let mut records = self.records.clone();
        let mut devices = self.devices.clone();
        let mut model = self.model.clone();
        let mut calls = self.calls.clone();
        let mut retained_va = self.retained_va;
        let mut retained_device = self.retained_device_bytes;
        let mut usage = self.usage;
        for id in controls {
            let r = records
                .iter_mut()
                .find(|r| (r.id, r.generation) == (id.id, id.generation))
                .unwrap();
            calls.extend(shared_calls(r));
            let (reservation, allocation, mapping) =
                model_keys(memory.fixture.vm, id.id, id.generation);
            model = project_unmap(&model, mapping).unwrap();
            model = project_release(&model, reservation, allocation, mapping).unwrap();
            retained_va -= r.layout.gpu_va_bytes();
            *r = expected_native_record(r, true, 4, true);
        }
        for (i, id) in order
            .iter()
            .enumerate()
            .take(completed + usize::from(active.is_some()))
        {
            let done = i < completed;
            let a = (!done).then_some(active).flatten();
            if let Some(active) = a {
                self.assert_data_identity(*id, active);
            }
            let attempts = a.map_or([true; 4], |a| {
                [a.unmap.0, a.disposal[0].0, a.disposal[1].0, a.disposal[2].0]
            });
            let success = a.map_or([true; 4], |a| {
                [
                    a.owner != "Mapped" && a.owner != "Original",
                    a.disposal[0].1 == Some(true),
                    a.disposal[1].1 == Some(true),
                    a.disposal[2].1 == Some(true),
                ]
            });
            match id {
                Identity::Host(id) => {
                    let r = records
                        .iter_mut()
                        .find(|r| (r.id, r.generation) == (id.id, id.generation))
                        .unwrap();
                    calls.extend(
                        shared_calls(r)
                            .into_iter()
                            .zip(attempts)
                            .filter_map(|(call, attempted)| attempted.then_some(call)),
                    );
                    let settled = done || a.is_some_and(|a| a.stage == Stage::ReleaseCommit);
                    if success[0] {
                        r.phase = SharedAllocationPhaseV1::CpuWritable;
                    }
                    if success[1] {
                        r.mapping = None;
                    }
                    if success[2] {
                        r.handle = None;
                    }
                    if success[3] {
                        r.reservation = None;
                    }
                    r.free_attempted |= attempts[2];
                    let (reservation, allocation, mapping) =
                        model_keys(memory.fixture.vm, id.id, id.generation);
                    if done
                        || a.is_some_and(|a| {
                            matches!(
                                a.stage,
                                Stage::ReleasePreflight
                                    | Stage::ReleaseEvidence
                                    | Stage::ReleaseProjection
                                    | Stage::NativeRelease
                                    | Stage::ReleaseCommit
                            )
                        })
                    {
                        model = project_unmap(&model, mapping).unwrap();
                    }
                    if done {
                        model = project_release(&model, reservation, allocation, mapping).unwrap();
                    }
                    if settled {
                        r.phase = SharedAllocationPhaseV1::Released;
                        r.indexed = None;
                        retained_va -= r.layout.gpu_va_bytes();
                        if r.charged {
                            let usage = usage.0.as_mut().unwrap();
                            usage.used_backing_bytes -= r.layout.gpu_va_bytes();
                            usage.used_allocation_records -= 1;
                            usage.retained_records -= 1;
                            r.charged = false;
                        }
                    }
                }
                Identity::Device(id) => {
                    let r = devices.iter_mut().find(|r| r.identity == *id).unwrap();
                    let va = r.reservation.unwrap();
                    let native = [
                        CleanupCallV1::UnmapGpu(r.handle.unwrap(), 0),
                        CleanupCallV1::Free(r.handle.unwrap()),
                        CleanupCallV1::ReleaseVa(va.0, va.1),
                    ];
                    calls.extend(
                        native
                            .into_iter()
                            .zip([attempts[0], attempts[2], attempts[3]])
                            .filter_map(|(call, attempted)| attempted.then_some(call)),
                    );
                    if attempts[0] {
                        r.phase = DeviceMemoryPhaseV1::Ambiguous;
                    }
                    if success[0] {
                        r.phase = DeviceMemoryPhaseV1::Unmapped;
                    }
                    if attempts[2] {
                        r.phase = DeviceMemoryPhaseV1::Ambiguous;
                    }
                    r.free_attempted |= attempts[2];
                    if success[2] {
                        r.handle = None;
                    }
                    if success[3] {
                        r.reservation = None;
                    }
                    if done {
                        r.phase = DeviceMemoryPhaseV1::Released;
                        r.indexed = None;
                        retained_device -= r.layout.backing_bytes();
                        if r.charged {
                            let usage = usage.1.as_mut().unwrap();
                            usage.used_backing_bytes -= r.layout.backing_bytes();
                            usage.used_allocation_records -= 1;
                            usage.retained_records -= 1;
                            r.charged = false;
                        }
                    }
                }
            }
        }
        assert_eq!(
            after.calls, calls,
            "exact data cleanup arguments and forward order"
        );
        assert_eq!(
            after.records, records,
            "exact host prefix and untouched suffix"
        );
        assert_eq!(
            after.devices, devices,
            "exact device prefix and untouched suffix"
        );
        assert_eq!(
            after.model, model,
            "only coherent cleanup changes the memory model"
        );
        assert_eq!(after.usage, usage, "refund only settled native disposal");
        assert_eq!(after.retained_va, retained_va);
        assert_eq!(after.retained_device_bytes, retained_device);
        assert_eq!(after.storage, self.storage);
        assert_eq!(after.identity, self.identity);
    }
}

fn fixture(
    kind: usize,
    configured: bool,
) -> (PristineAbortMemoryFixtureV1, DataCleanupCustodyV1, Identity) {
    let mut memory = PristineAbortMemoryFixtureV1::new_configured(configured);
    let data = memory.typed_data(kind);
    let identity = data.sdma_storage_identity();
    let _neighbor_host = memory.host();
    let _neighbor_device = memory.device();
    (memory, DataCleanupCustodyV1::new(data), identity)
}

fn no_retry(memory: &mut PristineAbortMemoryFixtureV1, root: &mut DataCleanupCustodyV1) {
    memory.clear_cleanup_faults();
    let before = memory.memory_snapshot();
    let custody = root.observation();
    assert!(matches!(
        memory.release_data(root),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    assert_eq!(memory.memory_snapshot(), before);
    assert_eq!(root.observation(), custody);
}

#[test]
fn typed_data_cleanup_preserves_all_five_inputs_and_refunds_once() {
    for configured in [false, true] {
        for kind in 0..5 {
            let (mut memory, mut root, id) = fixture(kind, configured);
            let before = memory.memory_snapshot();
            let metadata = root.observation().metadata;
            let DataCleanupMetadataV1::Fixed {
                identity,
                fully_initialized,
                initialized_content,
                ..
            } = &metadata
            else {
                panic!("fixed input lost its metadata variant")
            };
            let expected_identity = match (kind, id) {
                (0, Identity::Device(id)) => FixedIdentity::DeviceUninitialized(id),
                (1, Identity::Host(id)) => FixedIdentity::HostVisibleUninitialized(id),
                (2, Identity::Device(id)) => FixedIdentity::DeviceInitializedContent(id),
                (3, Identity::Host(id)) => FixedIdentity::HostVisibleInitialized(id),
                (4, Identity::Device(id)) => FixedIdentity::DeviceInitializedAfterDispatch(id),
                _ => panic!("fixture storage does not match its requested kind"),
            };
            assert_eq!(*identity, expected_identity);
            assert_eq!(*fully_initialized, kind >= 2);
            assert_eq!(
                *initialized_content,
                (kind == 2).then(|| content(&[0x5a; 17]))
            );
            memory.release_data(&mut root).unwrap();
            let after = root.observation();
            assert_eq!(after.metadata, metadata);
            assert!(after.complete && after.native_disposed && !after.failed);
            assert_eq!(after.owner, "NativeDisposed");
            before.assert_data_identity(id, &after);
            assert_eq!(after.unmap, (true, Some(true), Some(1)));
            assert_eq!(
                after.disposal,
                [
                    (
                        matches!(id, Identity::Host(_)),
                        matches!(id, Identity::Host(_)).then_some(true)
                    ),
                    (true, Some(true)),
                    (true, Some(true))
                ]
            );
            before.assert_data_prefix(&memory, &[], &[id], 1, None);
            no_retry(&mut memory, &mut root);
        }
    }
}

#[test]
fn typed_data_consuming_wrapper_returns_success_without_retention() {
    for kind in 0..5 {
        let (mut memory, root, id) = fixture(kind, true);
        let before = memory.memory_snapshot();
        data_cleanup::release_owned_with_v1(root, &mut memory, |_| {
            panic!("successful typed disposal retained its owner")
        })
        .unwrap();
        before.assert_data_prefix(&memory, &[], &[id], 1, None);
    }
}

#[test]
fn public_typed_disposal_bridges_route_through_retained_cleanup() {
    let source = include_str!("../../../shared_memory.rs");
    let initialized = source
        .split("    pub fn release_initialized_gfx942_device_memory(")
        .nth(1)
        .unwrap()
        .split("    pub fn release_fixed_dispatch_data(")
        .next()
        .unwrap();
    assert!(initialized.contains("self.release_fixed_dispatch_data(crate::Gfx942FixedDispatchDataV1::initialized(initialized))"));
    assert!(
        !initialized.contains(".into_parts()") && !initialized.contains(".unmap_device_memory(")
    );
    let typed = source
        .split("    pub fn release_fixed_dispatch_data(")
        .nth(1)
        .unwrap()
        .split("    pub fn model_journal_summary(")
        .next()
        .unwrap();
    assert!(typed.contains("data_cleanup::release_owned_with_v1("));
    assert!(typed.contains("DataCleanupCustodyV1::new(data),"));
    assert!(typed.contains("core::mem::forget,"));
    assert!(!typed.contains(".into_parts()") && !typed.contains(".unmap_device_memory("));
}

#[test]
fn typed_data_cleanup_retains_native_error_and_panic_prefixes() {
    for configured in [false, true] {
        for kind in 0..5 {
            let host = matches!(kind, 1 | 3);
            let operations: &[&'static str] = if host {
                &["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
            } else {
                &["unmap_gpu", "free", "release_va_reservation"]
            };
            for (index, operation) in operations.iter().enumerate() {
                for panic in [false, true] {
                    let (mut memory, mut root, id) = fixture(kind, configured);
                    let before = memory.memory_snapshot();
                    let metadata = root.observation().metadata;
                    memory.fail_data(1, operation, panic);
                    let result = catch_unwind(AssertUnwindSafe(|| memory.release_data(&mut root)));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", *operation))
                        );
                    } else {
                        assert!(
                            matches!(result.unwrap(), Err(MemorySessionError::Injected(actual)) if actual == *operation)
                        );
                    }
                    let after = root.observation();
                    assert_eq!(after.metadata, metadata);
                    assert!(
                        after.started && after.failed && !after.complete && !after.native_disposed
                    );
                    assert_eq!(after.owner, if index == 0 { "Mapped" } else { "Unmapped" });
                    let mut expected = [(false, None); 3];
                    let slots: &[usize] = if host { &[0, 1, 2] } else { &[1, 2] };
                    for (ordinal, slot) in slots.iter().enumerate().take(index) {
                        expected[*slot] = (
                            true,
                            if ordinal + 1 < index {
                                Some(true)
                            } else {
                                (!panic).then_some(false)
                            },
                        );
                    }
                    assert_eq!(after.disposal, expected);
                    assert_eq!(
                        after.unmap,
                        if index == 0 {
                            (true, (!panic).then_some(false), (!panic).then_some(1))
                        } else {
                            (true, Some(true), Some(1))
                        }
                    );
                    assert!(memory.is_quarantined());
                    before.assert_data_prefix(&memory, &[], &[id], 0, Some(&after));
                    no_retry(&mut memory, &mut root);
                }
            }
        }
    }
}

#[test]
fn typed_data_cleanup_checks_all_currentness_boundaries_and_disposal_receipts() {
    for configured in [false, true] {
        for kind in 0..5 {
            let host = matches!(kind, 1 | 3);
            for offset in 1..=if host { 6 } else { 5 } {
                for panic in [false, true] {
                    let (mut memory, mut root, id) = fixture(kind, configured);
                    let before = memory.memory_snapshot();
                    let metadata = root.observation().metadata;
                    memory.fail_currentness(offset, panic);
                    let result = catch_unwind(AssertUnwindSafe(|| memory.release_data(&mut root)));
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
                    assert_eq!(memory.currentness_calls() - before.currentness, offset);
                    assert!(memory.is_quarantined());
                    let after = root.observation();
                    assert_eq!(after.metadata, metadata);
                    assert!(!after.complete && after.failed);
                    let terminal = offset == if host { 6 } else { 5 };
                    assert_eq!(after.native_disposed, terminal);
                    assert_eq!(
                        after.owner,
                        if offset <= 2 {
                            "Mapped"
                        } else if terminal {
                            "NativeDisposed"
                        } else {
                            "Unmapped"
                        }
                    );
                    assert_eq!(
                        after.unmap,
                        if offset == 1 {
                            (false, None, None)
                        } else {
                            (true, Some(true), Some(1))
                        }
                    );
                    let expected = if host {
                        [offset >= 4, offset >= 5, offset >= 6]
                    } else {
                        [false, offset >= 4, offset >= 5]
                    };
                    assert_eq!(
                        after.disposal,
                        expected.map(|ran| (ran, ran.then_some(true)))
                    );
                    assert_eq!(
                        after.stage,
                        if offset <= 2 {
                            Stage::NativeUnmap
                        } else {
                            Stage::NativeRelease
                        }
                    );
                    before.assert_data_prefix(&memory, &[], &[id], 0, Some(&after));
                    no_retry(&mut memory, &mut root);
                }
            }
        }
    }
}

#[test]
fn typed_data_cleanup_preserves_partial_unmap_results() {
    for kind in 0..5 {
        for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
            let (mut memory, mut root, id) = fixture(kind, true);
            let before = memory.memory_snapshot();
            memory.unmap_data(1, prefix, errno);
            let error = memory.release_data(&mut root).unwrap_err();
            let host = matches!(id, Identity::Host(_));
            if prefix > 1 {
                assert!(
                    matches!(error, MemorySessionError::KernelResultMalformed(detail) if detail == if host {
                    "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"
                } else { "device-memory UNMAP_MEMORY_FROM_GPU cumulative n_success" })
                );
            } else if errno {
                assert!(matches!(error, MemorySessionError::Injected("unmap_gpu")));
            } else {
                assert!(
                    matches!(error, MemorySessionError::KernelResultMalformed(detail) if detail == if host {
                "shared UNMAP_MEMORY_FROM_GPU full prefix"
            } else { "device-memory UNMAP_MEMORY_FROM_GPU full prefix" })
                );
            }
            let after = root.observation();
            assert_eq!(after.owner, "Mapped");
            assert_eq!(after.unmap, (true, Some(!errno), Some(prefix)));
            assert_eq!(after.disposal, [(false, None); 3]);
            before.assert_data_prefix(&memory, &[], &[id], 0, Some(&after));
            no_retry(&mut memory, &mut root);
        }
    }
}

#[test]
fn typed_host_cleanup_retains_projection_and_actual_commit_failures() {
    for kind in [1, 3] {
        for stage in [
            Stage::UnmapProjection,
            Stage::UnmapCommit,
            Stage::ReleaseProjection,
            Stage::ReleaseCommit,
        ] {
            for fault in [Fault::Error, Fault::Panic, Fault::ExhaustRevision] {
                if matches!(fault, Fault::ExhaustRevision)
                    && !matches!(stage, Stage::UnmapCommit | Stage::ReleaseCommit)
                {
                    continue;
                }
                let (mut memory, mut root, id) = fixture(kind, true);
                if matches!(fault, Fault::ExhaustRevision) {
                    memory.exhaust_data_commit(1, stage == Stage::ReleaseCommit);
                } else {
                    memory.fail_data_projection(1, stage, matches!(fault, Fault::Panic));
                }
                let before = memory.memory_snapshot();
                let result = catch_unwind(AssertUnwindSafe(|| memory.release_data(&mut root)));
                if matches!(fault, Fault::Panic) {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                        Some(&("control cleanup projection", stage))
                    );
                } else if matches!(fault, Fault::ExhaustRevision) {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Model(
                            "queue foundation certificate revision exhausted"
                        ))
                    ));
                    memory
                        .memory_snapshot()
                        .assert_certificate_revision(u64::MAX);
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(MemorySessionError::Injected("control cleanup projection"))
                    ));
                }
                let after = root.observation();
                assert_eq!(after.stage, stage);
                assert_eq!(after.native_disposed, stage == Stage::ReleaseCommit);
                before.assert_data_prefix(&memory, &[], &[id], 0, Some(&after));
                no_retry(&mut memory, &mut root);
            }
        }
    }
}

#[test]
fn typed_data_cleanup_retains_terminal_receipt_on_accounting_underflow() {
    for kind in [0, 1, 2, 3, 4] {
        let (mut memory, mut root, id) = fixture(kind, true);
        let usage = memory.usage();
        if matches!(kind, 1 | 3) {
            memory.fixture.engine.retained_gpu_va_bytes = 0;
        } else {
            memory.fixture.engine.retained_device_memory_bytes = 0;
        }
        let before = memory.memory_snapshot();
        let error = memory.release_data(&mut root).unwrap_err();
        assert!(
            matches!(error, MemorySessionError::KernelResultMalformed(detail) if detail == if matches!(kind, 1 | 3) {
            "shared retained GPU VA accounting"
        } else { "retained device-memory accounting" })
        );
        let after = root.observation();
        before.assert_data_identity(id, &after);
        assert_eq!(after.owner, "NativeDisposed");
        assert!(after.native_disposed && after.failed && !after.complete);
        assert_eq!(memory.usage(), usage);
        let native = memory.memory_snapshot();
        let mut records = before.records.clone();
        let mut devices = before.devices.clone();
        let mut calls = before.calls.clone();
        match id {
            Identity::Host(id) => {
                let r = records.iter_mut().find(|r| r.id == id.id).unwrap();
                calls.extend(shared_calls(r));
                *r = expected_native_record(r, true, 4, true);
                assert_eq!(native.retained_va, 0);
            }
            Identity::Device(id) => {
                let r = devices.iter_mut().find(|r| r.identity == id).unwrap();
                let va = r.reservation.unwrap();
                calls.extend([
                    CleanupCallV1::UnmapGpu(r.handle.unwrap(), 0),
                    CleanupCallV1::Free(r.handle.unwrap()),
                    CleanupCallV1::ReleaseVa(va.0, va.1),
                ]);
                r.handle = None;
                r.reservation = None;
                r.free_attempted = true;
                r.phase = DeviceMemoryPhaseV1::Released;
                r.indexed = None;
                assert_eq!(native.retained_device_bytes, 0);
            }
        }
        assert_eq!(native.records, records);
        assert_eq!(native.devices, devices);
        assert_eq!(native.calls, calls);
        no_retry(&mut memory, &mut root);
    }
}

#[test]
fn typed_data_rejection_keeps_healthy_receiving_session_unchanged() {
    for kind in 0..5 {
        let (mut origin, mut root, _) = fixture(kind, true);
        let mut receiver = PristineAbortMemoryFixtureV1::new();
        let before = receiver.memory_snapshot();
        assert!(receiver.release_data(&mut root).is_err());
        assert_eq!(receiver.memory_snapshot(), before);
        assert!(!receiver.is_quarantined());
        assert_eq!(root.observation().unmap, (false, None, None));
        no_retry(&mut origin, &mut root);
    }
}

#[test]
fn typed_data_owned_wrapper_retains_errors_panics_and_incomplete_callbacks() {
    struct Incomplete;
    impl DispatchDataReleaseV1 for Incomplete {
        fn release_data(&mut self, _: &mut DataCleanupCustodyV1) -> Result<(), MemorySessionError> {
            Ok(())
        }
    }
    for kind in 0..5 {
        let (_, root, _) = fixture(kind, true);
        let before = root.observation();
        let mut retained = None;
        assert!(
            data_cleanup::release_owned_with_v1(root, &mut Incomplete, |root| retained =
                Some(root))
            .is_err()
        );
        assert_eq!(retained.unwrap().observation(), before);
        for panic in [false, true] {
            let (mut memory, root, _) = fixture(kind, true);
            let metadata = root.observation().metadata;
            memory.fail_data(1, "free", panic);
            let mut retained = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                data_cleanup::release_owned_with_v1(root, &mut memory, |root| retained = Some(root))
            }));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "free"))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
            let mut root = retained.unwrap();
            assert_eq!(root.observation().metadata, metadata);
            no_retry(&mut memory, &mut root);
        }
    }
}

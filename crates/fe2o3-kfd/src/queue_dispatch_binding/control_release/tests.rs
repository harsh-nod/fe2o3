use super::*;
use crate::shared_memory::{
    CleanupStageV1 as Stage, ControlCleanupObservationV1, Gfx942DeviceMemoryDispatchFactsV1,
    PreparationMemoryFixtureV1 as Memory, SharedGttAllocationLayoutV1,
    SharedGttMappedResourceFactsV1,
};

type Root = ReturningControlCleanupCustodyV1;
type Mode = ReturningControlModeV1;

#[path = "detached_tests.rs"]
mod detached;

#[path = "persistent_tests.rs"]
mod persistent;

#[path = "ordinary_tests.rs"]
mod ordinary;

#[derive(Debug, Eq, PartialEq)]
struct HostFacts {
    gpu_va: u64,
    logical_bytes: usize,
    cpu_mapping_bytes: usize,
    gpu_va_bytes: u64,
    mapping: MemoryMappingKeyV1,
    publication: fe2o3_runtime_model::MemoryPublicationKeyV1,
}

fn host_facts(f: &SharedGttMappedResourceFactsV1) -> HostFacts {
    HostFacts {
        gpu_va: f.gpu_va(),
        logical_bytes: f.logical_bytes(),
        cpu_mapping_bytes: f.cpu_mapping_bytes(),
        gpu_va_bytes: f.gpu_va_bytes(),
        mapping: f.mapping(),
        publication: f.publication(),
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Control {
    identity: SharedGttAllocationIdentityV1,
    layout: SharedGttAllocationLayoutV1,
    facts: HostFacts,
}

#[derive(Debug, Eq, PartialEq)]
enum DataFacts {
    Device(Gfx942DeviceMemoryDispatchFactsV1),
    Host(HostFacts),
}

#[derive(Debug, Eq, PartialEq)]
struct Data {
    identity: Gfx942SdmaBufferStorageIdentityV1,
    facts: DataFacts,
}

#[derive(Debug, Eq, PartialEq)]
struct PersistentData {
    identity: Gfx942SdmaBufferStorageIdentityV1,
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content: Option<Gfx942DeviceContentDescriptorV1>,
}

fn persistent_data(data: &[Gfx942FixedDispatchDataV1]) -> Vec<PersistentData> {
    data.iter()
        .map(|data| PersistentData {
            identity: data.sdma_storage_identity(),
            layout: data.layout(),
            initialized: data.is_fully_initialized(),
            content: data.initialized_content(),
        })
        .collect()
}

fn data(a: &DispatchDataAuthorityV1) -> Data {
    Data {
        identity: Memory::data_storage(a),
        facts: match a {
            DispatchDataAuthorityV1::Device(a) => DataFacts::Device(*a.facts()),
            DispatchDataAuthorityV1::HostVisible(a) => DataFacts::Host(host_facts(a.facts())),
        },
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Premise {
    value: RetainedDataPremiseV1,
    storage: [(usize, usize); 2],
}

fn premise(p: &RetainedDataPremiseV1) -> Premise {
    Premise {
        value: RetainedDataPremiseV1 {
            layout: p.layout,
            role_identity: p.role_identity,
            valid_bytes: p.valid_bytes,
            effect: p.effect,
            initialized_content: p.initialized_content,
            fully_initialized: p.fully_initialized,
            writable_ranges: p.writable_ranges.clone(),
            completed_snapshots: p.completed_snapshots.clone(),
        },
        storage: [
            (p.writable_ranges.as_ptr() as usize, p.writable_ranges.len()),
            (
                p.completed_snapshots.as_ptr() as usize,
                p.completed_snapshots.len(),
            ),
        ],
    }
}

fn storage<T>(v: &Vec<T>) -> (usize, usize) {
    (v.as_ptr() as usize, v.capacity())
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    mode: Mode,
    kernarg: Option<Control>,
    code: Vec<Control>,
    code_pointer: usize,
    code_identity: Vec<ResolvedCodeIdentityV1>,
    packets: Vec<PreparedDispatchPacketV1>,
    data: Vec<Data>,
    premises: Vec<Premise>,
    generation: DispatchGenerationOwnerV1,
    slots_pointer: usize,
    persistent: PersistentFixedDispatchControlStateV1,
    active: Option<ControlCleanupObservationV1>,
    returned: Vec<(Data, Premise)>,
    returned_generation: Option<u64>,
    persistent_returned: Vec<PersistentData>,
    persistent_output: PersistentOutputStateV1,
    storage: [(usize, usize); 6],
    started: bool,
    complete: bool,
}

fn snapshot(r: &Root) -> Snapshot {
    Snapshot {
        mode: r.mode,
        kernarg: r.kernarg.as_ref().map(|a| Control {
            identity: Memory::kernarg_identity(a),
            layout: a.layout(),
            facts: host_facts(a.facts()),
        }),
        code: r
            .code
            .as_slice()
            .iter()
            .map(|a| Control {
                identity: Memory::code_identity(a),
                layout: a.layout(),
                facts: host_facts(a.facts()),
            })
            .collect(),
        code_pointer: r.code.as_slice().as_ptr() as usize,
        code_identity: r.code_identity.clone(),
        packets: r.packets.clone(),
        data: r.data.iter().map(data).collect(),
        premises: r.data_premises.iter().map(premise).collect(),
        generation: r.generation.clone(),
        slots_pointer: r.generation.slots.as_ptr() as usize,
        persistent: r.persistent_control,
        active: r
            .active_control
            .as_ref()
            .map(ControlCleanupCustodyV1::observation),
        returned: r
            .returned
            .iter()
            .map(|d| (data(&d.authority), premise(&d.premise)))
            .collect(),
        returned_generation: r.returned_generation,
        persistent_returned: persistent_data(&r.persistent_returned),
        persistent_output: r.persistent_output,
        storage: [
            storage(&r.code_identity),
            storage(&r.packets),
            storage(&r.data),
            storage(&r.data_premises),
            storage(&r.returned),
            storage(&r.persistent_returned),
        ],
        started: r.started,
        complete: r.complete,
    }
}

fn order(s: &Snapshot) -> Vec<SharedGttAllocationIdentityV1> {
    std::iter::once(s.kernarg.as_ref().unwrap().identity)
        .chain(s.code.iter().map(|a| a.identity))
        .collect()
}

fn assert_inputs(r: &Root, before: &Snapshot) {
    let after = snapshot(r);
    assert_eq!(after.mode, before.mode);
    assert_eq!(after.code_identity, before.code_identity);
    assert_eq!(after.packets, before.packets);
    assert_eq!(after.data, before.data);
    assert_eq!(after.premises, before.premises);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.slots_pointer, before.slots_pointer);
    assert_eq!(after.persistent, before.persistent);
    assert_eq!(after.storage[..4], before.storage[..4]);
    assert!(after.returned.is_empty());
    assert!(!after.complete);
}

fn new_root(owner: DispatchResourceOwnerV1, mode: Mode) -> Root {
    let before = Snapshot {
        mode,
        kernarg: Some(Control {
            identity: Memory::kernarg_identity(&owner.kernarg),
            layout: owner.kernarg.layout(),
            facts: host_facts(owner.kernarg.facts()),
        }),
        code: owner
            .code
            .iter()
            .map(|a| Control {
                identity: Memory::code_identity(a),
                layout: a.layout(),
                facts: host_facts(a.facts()),
            })
            .collect(),
        code_pointer: owner.code.as_ptr() as usize,
        code_identity: owner.code_identity.clone(),
        packets: owner.packets.clone(),
        data: owner.data.iter().map(data).collect(),
        premises: owner.data_premises.iter().map(premise).collect(),
        generation: owner.generation.clone(),
        slots_pointer: owner.generation.slots.as_ptr() as usize,
        persistent: owner.persistent_control,
        active: None,
        returned: Vec::new(),
        returned_generation: None,
        persistent_returned: Vec::new(),
        persistent_output: PersistentOutputStateV1::Unprepared,
        storage: [
            storage(&owner.code_identity),
            storage(&owner.packets),
            storage(&owner.data),
            storage(&owner.data_premises),
            storage(&Vec::<ReturnedDispatchDataLeaseV1>::new()),
            storage(&Vec::<Gfx942FixedDispatchDataV1>::new()),
        ],
        started: false,
        complete: false,
    };
    let root = Root::new(owner, mode);
    assert_eq!(
        snapshot(&root),
        before,
        "constructor changed original owner"
    );
    root
}

struct NoEntry;
impl PristineControlReleaseV1 for NoEntry {
    fn release_control(
        &mut self,
        _: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        panic!("returning retry entered cleanup callback")
    }
}

fn reject_retry(r: &mut Root) {
    let before = snapshot(r);
    assert!(matches!(
        r.release_in_place(&mut NoEntry),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(snapshot(r), before);
    assert!(matches!(
        r.take_completed(),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(snapshot(r), before);
}

// Synthetic completion observations drive the real generation transitions.
fn publish(
    g: &mut DispatchGenerationOwnerV1,
) -> (DispatchEpochIdentityV1, CompletionBatchOccurrenceV1) {
    let next = g.next_generation;
    let epoch = g
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(next))
        .unwrap();
    let completion = test_completion_occurrence_v1(next);
    g.mark_published(epoch, completion).unwrap();
    (epoch, completion)
}

fn recycle(g: &mut DispatchGenerationOwnerV1) -> u64 {
    let (epoch, completion) = publish(g);
    g.complete_epoch(epoch, completion).unwrap();
    g.recycle_epoch(epoch, completion).unwrap();
    epoch.dispatch_generation
}

fn fixture(mode: Mode) -> (crate::shared_memory::PristineAbortMemoryFixtureV1, Root) {
    let (memory, mut owner) = pristine_abort::pristine_dispatch_fixture_v1(8);
    if matches!(mode, Mode::AfterRecycle | Mode::PersistentAfterRecycle) {
        assert_eq!(recycle(&mut owner.generation), 8);
    }
    (memory, new_root(owner, mode))
}

#[test]
fn returning_success_preserves_exact_data_premises_storage_and_forward_order() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        let (mut memory, mut r) = fixture(mode);
        let before = snapshot(&r);
        let native = memory.memory_snapshot();
        let records = memory.control_record_snapshot(order(&before));
        let currentness = memory.currentness_calls();
        r.release_in_place(&mut memory).unwrap();
        assert_eq!(memory.currentness_calls() - currentness, 18);
        native.assert_control_transition(&memory, &order(&before), 3, false, 0, None);
        records.assert_after(&memory, 3, 3);
        assert!(r.complete && r.active_control.is_none() && r.kernarg.is_none());
        assert!(r.code.as_slice().is_empty());
        assert!(r.data.is_empty() && r.data_premises.is_empty());
        assert_eq!(snapshot(&r).storage[..4], before.storage[..4]);
        assert_eq!(r.generation, before.generation);
        assert!(r.returned.capacity() >= before.data.len());
        let output_storage = storage(&r.returned);
        let returned = r.take_completed().unwrap();
        assert_eq!(returned.data.len(), before.data.len());
        assert_eq!(
            returned.generation(),
            if mode == Mode::AfterRecycle { 8 } else { 0 }
        );
        assert_eq!(storage(&returned.data), output_storage);
        for ((lease, expected_data), expected_premise) in returned
            .data
            .into_iter()
            .zip(before.data)
            .zip(before.premises)
        {
            assert_eq!(data(&lease.authority), expected_data);
            assert_eq!(premise(&lease.premise), expected_premise);
            let converted = lease.into_data();
            assert_eq!(converted.sdma_storage_identity(), expected_data.identity);
            assert_eq!(
                converted.is_fully_initialized(),
                expected_premise.value.fully_initialized
            );
            assert_eq!(
                converted.initialized_content(),
                None,
                "stale content is not returned authority"
            );
        }
        reject_retry(&mut r);
    }
}

#[test]
fn returning_generation_rejection_precedes_cardinality_and_preserves_full_owner() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for phase in 0..4 {
            let (mut memory, mut r) = fixture(mode);
            match phase {
                0 => r.generation.poison(),
                1 => {
                    let g = r.generation.next_generation;
                    r.generation
                        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(g))
                        .unwrap();
                }
                2 => {
                    publish(&mut r.generation);
                }
                3 => {
                    let (e, c) = publish(&mut r.generation);
                    r.generation.complete_epoch(e, c).unwrap();
                }
                _ => unreachable!(),
            }
            let removed = r.data_premises.pop().unwrap();
            let before = snapshot(&r);
            let native = memory.memory_snapshot();
            let error = r.release_in_place(&mut memory);
            if phase == 0 {
                assert!(matches!(error, Err(Gfx942DispatchBindingErrorV1::Poisoned)));
            } else {
                assert!(matches!(
                    error,
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            }
            assert_inputs(&r, &before);
            assert_eq!(snapshot(&r).kernarg, before.kernarg);
            assert_eq!(snapshot(&r).code, before.code);
            assert_eq!(memory.memory_snapshot(), native);
            assert!(r.active_control.is_none());
            reject_retry(&mut r);
            drop(removed);
        }
    }
}

#[test]
fn returning_cardinality_and_capacity_reject_before_native_entry() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for invalid in 0..3 {
            let (mut memory, mut r) = fixture(mode);
            let mut retained_premise = None;
            if invalid == 0 {
                retained_premise = r.data_premises.pop();
            } else {
                r.return_capacity_override = Some(if invalid == 1 { usize::MAX } else { 0 });
            }
            let before = snapshot(&r);
            let native = memory.memory_snapshot();
            let result = r.release_in_place(&mut memory);
            if invalid == 0 {
                assert!(matches!(
                    result,
                    Err(Gfx942DispatchBindingErrorV1::InvalidData {
                        index: 4,
                        detail: "retained data/premise cardinality"
                    })
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "returning dispatch data"
                    })
                ));
            }
            assert_inputs(&r, &before);
            assert_eq!(snapshot(&r).kernarg, before.kernarg);
            assert_eq!(snapshot(&r).code, before.code);
            assert_eq!(memory.memory_snapshot(), native);
            reject_retry(&mut r);
            drop(retained_premise);
        }
    }
}

#[test]
fn returning_modes_preserve_cancelled_history_max_recycle_and_exhaustion() {
    let (mut memory, owner) = pristine_abort::pristine_dispatch_fixture_v1(8);
    let mut r = new_root(owner, Mode::AfterRecycle);
    let native = memory.memory_snapshot();
    assert!(matches!(
        r.release_in_place(&mut memory),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(memory.memory_snapshot(), native);
    reject_retry(&mut r);
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for exhausted in [false, true] {
            let (mut memory, mut owner) =
                pristine_abort::pristine_dispatch_fixture_v1(if exhausted {
                    u64::MAX - 1
                } else {
                    8
                });
            let expected = if exhausted {
                recycle(&mut owner.generation)
            } else {
                let (a, ac) = publish(&mut owner.generation);
                let (b, bc) = publish(&mut owner.generation);
                for (e, c) in [(b, bc), (a, ac)] {
                    owner.generation.complete_epoch(e, c).unwrap();
                    owner.generation.recycle_epoch(e, c).unwrap();
                }
                let next = owner.generation.next_generation;
                let cancelled = owner
                    .generation
                    .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(next))
                    .unwrap();
                owner.generation.cancel_epoch(cancelled).unwrap();
                b.dispatch_generation
            };
            let returned = release_returning_with_v1(new_root(owner, mode), &mut memory, |_| {
                panic!("success retained root")
            })
            .unwrap();
            assert_eq!(returned.generation(), expected);
        }
    }
}

fn assert_interrupted(r: &mut Root, before: &Snapshot, index: usize, owner: &str) {
    assert_inputs(r, before);
    let after = snapshot(r);
    let active = after.active.as_ref().expect("failed control stays rooted");
    assert_eq!(active.identity, order(before)[index]);
    let original = if index == 0 {
        before.kernarg.as_ref().unwrap()
    } else {
        &before.code[index - 1]
    };
    assert_eq!(active.layout, original.layout);
    assert_eq!(
        active.profile_type,
        if index == 0 {
            std::any::TypeId::of::<KernargGttV1>()
        } else {
            std::any::TypeId::of::<ExecutableGttV1>()
        }
    );
    assert_eq!(
        active.state_type,
        match (index == 0, owner == "Mapped") {
            (true, true) => std::any::TypeId::of::<GttGpuAccessibleMutableV1>(),
            (false, true) => std::any::TypeId::of::<GttGpuAccessibleExecutableV1>(),
            (true, false) => std::any::TypeId::of::<crate::shared_memory::GttCpuWritableV1>(),
            (false, false) =>
                std::any::TypeId::of::<crate::shared_memory::GttExecutableImmutableV1>(),
        }
    );
    if matches!(before.mode, Mode::DetachedPersistent { .. }) {
        assert_eq!(r.returned.capacity(), 0);
        assert_eq!(r.returned_generation, None);
    } else {
        assert!(
            r.returned.capacity() >= before.data.len(),
            "return capacity precedes disposal"
        );
        assert_eq!(
            r.returned_generation,
            Some(match before.mode {
                Mode::AfterRecycle => before.generation.returned_generation().unwrap(),
                Mode::ReturningDestroy => before.generation.returning_destroy_generation().unwrap(),
                Mode::Ordinary
                | Mode::DetachedPersistent { .. }
                | Mode::PersistentBeforePublication
                | Mode::PersistentAfterRecycle => unreachable!(),
            })
        );
    }
    assert_eq!(active.owner, owner);
    assert_eq!(after.kernarg, None);
    assert_eq!(after.code, before.code[index..]);
    assert_eq!(
        after.code_pointer,
        before.code_pointer + index * core::mem::size_of::<CodeAuthority>()
    );
    assert!(after.started && !after.complete);
    reject_retry(r);
}

#[test]
fn returning_native_failures_retain_every_control_position_and_exact_prefix() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for index in 0..3 {
            for (op, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                .into_iter()
                .enumerate()
            {
                for panic in [false, true] {
                    let (mut memory, mut r) = fixture(mode);
                    let before = snapshot(&r);
                    let native = memory.memory_snapshot();
                    let records = memory.control_record_snapshot(order(&before));
                    memory.fail_control(index + 1, operation, panic);
                    let result = catch_unwind(AssertUnwindSafe(|| r.release_in_place(&mut memory)));
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, &str)>(),
                            Some(&("N2 native panic", operation))
                        );
                    } else {
                        assert!(
                            matches!(result.unwrap(), Err(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(actual))) if actual == operation)
                        );
                    }
                    assert_interrupted(
                        &mut r,
                        &before,
                        index,
                        if op == 0 { "Mapped" } else { "Unmapped" },
                    );
                    let active = r.active_control.as_ref().unwrap().observation();
                    assert!(active.started && active.failed && !active.native_disposed);
                    assert_eq!(
                        active.stage,
                        if op == 0 {
                            Stage::NativeUnmap
                        } else {
                            Stage::NativeRelease
                        }
                    );
                    let returned = (!panic).then_some(false);
                    assert_eq!(
                        active.unmap,
                        if op == 0 {
                            (true, returned, (!panic).then_some(1))
                        } else {
                            (true, Some(true), Some(1))
                        }
                    );
                    let mut disposal = [(false, None); 3];
                    if op > 0 {
                        for p in disposal.iter_mut().take(op - 1) {
                            *p = (true, Some(true));
                        }
                        disposal[op - 1] = (true, returned);
                    }
                    assert_eq!(active.disposal, disposal);
                    native.assert_control_transition(
                        &memory,
                        &order(&before),
                        index,
                        op > 0,
                        op + 1,
                        Some((op > 0, op, false, op == 2)),
                    );
                    records.assert_after(&memory, index, index + 1);
                    assert!(memory.is_quarantined() && memory.data_is_retained());
                }
            }
        }
    }
}

#[test]
fn returning_currentness_failures_preserve_mapped_unmapped_and_disposed_custody() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for offset in 1_usize..=18 {
            for panic in [false, true] {
                let (mut memory, mut r) = fixture(mode);
                let before = snapshot(&r);
                let native = memory.memory_snapshot();
                let currentness = memory.currentness_calls();
                memory.fail_currentness(offset, panic);
                let result = catch_unwind(AssertUnwindSafe(|| r.release_in_place(&mut memory)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(Gfx942DispatchBindingErrorV1::Memory(
                            MemorySessionError::Injected("currentness")
                        ))
                    ));
                }
                let index = (offset - 1) / 6;
                let point = (offset - 1) % 6 + 1;
                assert_interrupted(
                    &mut r,
                    &before,
                    index,
                    if point <= 2 {
                        "Mapped"
                    } else if point == 6 {
                        "NativeDisposed"
                    } else {
                        "Unmapped"
                    },
                );
                assert_eq!(
                    r.active_control
                        .as_ref()
                        .unwrap()
                        .observation()
                        .native_disposed,
                    point == 6
                );
                let calls = point.saturating_sub(2).max(usize::from(point >= 2));
                let active = r.active_control.as_ref().unwrap().observation();
                assert_eq!(
                    active.stage,
                    if point <= 2 {
                        Stage::NativeUnmap
                    } else {
                        Stage::NativeRelease
                    }
                );
                assert_eq!(
                    active.unmap,
                    if point == 1 {
                        (false, None, None)
                    } else {
                        (true, Some(true), Some(1))
                    }
                );
                assert_eq!(
                    active.disposal,
                    std::array::from_fn(|i| if i < point.saturating_sub(3) {
                        (true, Some(true))
                    } else {
                        (false, None)
                    })
                );
                assert!(active.started && active.failed);
                native.assert_control_transition(
                    &memory,
                    &order(&before),
                    index,
                    point >= 3,
                    calls,
                    Some((point >= 3, calls, false, false)),
                );
                assert_eq!(memory.currentness_calls() - currentness, offset);
                assert!(memory.is_quarantined());
            }
        }
    }
}

#[test]
fn returning_projection_failures_retain_disposal_receipt_and_uncommitted_model() {
    for index in 0..3 {
        for release in [false, true] {
            for panic in [false, true] {
                let (mut memory, mut r) = fixture(Mode::AfterRecycle);
                let before = snapshot(&r);
                let native = memory.memory_snapshot();
                memory.fail_control_commit(index + 1, release, panic);
                let result = catch_unwind(AssertUnwindSafe(|| r.release_in_place(&mut memory)));
                if panic {
                    assert_eq!(
                        result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                        Some(&(
                            "control cleanup projection",
                            if release {
                                Stage::ReleaseCommit
                            } else {
                                Stage::UnmapCommit
                            }
                        ))
                    );
                } else {
                    assert!(matches!(
                        result.unwrap(),
                        Err(Gfx942DispatchBindingErrorV1::Memory(
                            MemorySessionError::Injected("control cleanup projection")
                        ))
                    ));
                }
                assert_interrupted(
                    &mut r,
                    &before,
                    index,
                    if release {
                        "NativeDisposed"
                    } else {
                        "Unmapped"
                    },
                );
                native.assert_control_transition(
                    &memory,
                    &order(&before),
                    index,
                    release,
                    if release { 4 } else { 1 },
                    Some((true, if release { 4 } else { 1 }, release, false)),
                );
                assert_eq!(
                    r.active_control
                        .as_ref()
                        .unwrap()
                        .observation()
                        .native_disposed,
                    release
                );
            }
        }
    }
}

#[test]
fn returning_incomplete_callbacks_cannot_advance_extract_or_retry() {
    struct Incomplete<'a> {
        memory: &'a mut crate::shared_memory::PristineAbortMemoryFixtureV1,
        stop: usize,
        entered: usize,
    }
    impl PristineControlReleaseV1 for Incomplete<'_> {
        fn release_control(
            &mut self,
            control: &mut ControlCleanupCustodyV1,
        ) -> Result<(), MemorySessionError> {
            let index = self.entered;
            self.entered += 1;
            if index == self.stop {
                Ok(())
            } else {
                self.memory.release_control(control)
            }
        }
    }
    for index in 0..3 {
        let (mut memory, mut r) = fixture(Mode::AfterRecycle);
        let before = snapshot(&r);
        let native = memory.memory_snapshot();
        let mut callback = Incomplete {
            memory: &mut memory,
            stop: index,
            entered: 0,
        };
        assert!(matches!(
            r.release_in_place(&mut callback),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert_eq!(callback.entered, index + 1);
        assert_interrupted(&mut r, &before, index, "Mapped");
        assert!(!r.active_control.as_ref().unwrap().observation().started);
        native.assert_control_transition(&memory, &order(&before), index, false, 0, None);
    }
}

#[test]
fn returning_wrapper_retains_full_root_before_error_or_original_panic() {
    for scenario in 0..5 {
        let (mut memory, mut r) = fixture(Mode::AfterRecycle);
        match scenario {
            0 => r.generation.poison(),
            1 => r.return_capacity_override = Some(usize::MAX),
            2 => memory.fail_control(2, "free", false),
            3 => memory.fail_control(3, "unmap_cpu", true),
            _ => {}
        }
        let before = snapshot(&r);
        let mut retained = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            release_returning_with_v1(r, &mut memory, |root| {
                assert!(retained.is_none());
                retained = Some(root);
            })
        }));
        if scenario == 4 {
            assert!(result.unwrap().is_ok());
            assert!(retained.is_none());
        } else {
            let r = retained
                .as_mut()
                .expect("failure retained its complete root");
            assert_inputs(r, &before);
            reject_retry(r);
            if scenario == 3 {
                let Err(payload) = result else {
                    panic!("original cleanup panic was lost")
                };
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "unmap_cpu"))
                );
            } else {
                assert!(result.unwrap().is_err());
            }
        }
    }
}

#[test]
fn returning_populated_preparation_metadata_survives_rejection_and_cleanup() {
    for fail in [false, true] {
        let (mut memory, owner) = preparation::control_release_fixture_v1();
        let mut r = new_root(owner, Mode::ReturningDestroy);
        let before = snapshot(&r);
        assert!(!before.code_identity.is_empty() && !before.packets.is_empty());
        assert!(
            before
                .premises
                .iter()
                .any(|p| !p.value.writable_ranges.is_empty())
        );
        if fail {
            r.return_capacity_override = Some(usize::MAX);
            assert!(r.release_in_place(&mut memory).is_err());
            assert_inputs(&r, &before);
            reject_retry(&mut r);
        } else {
            r.release_in_place(&mut memory).unwrap();
            let after = snapshot(&r);
            assert_eq!(after.code_identity, before.code_identity);
            assert_eq!(after.packets, before.packets);
            assert_eq!(after.generation, before.generation);
            assert_eq!(after.slots_pointer, before.slots_pointer);
            assert_eq!(after.storage[..4], before.storage[..4]);
            let returned = r.take_completed().unwrap();
            assert_eq!(returned.data.len(), before.data.len());
            for ((returned, expected_data), expected_premise) in returned
                .data
                .into_iter()
                .zip(before.data)
                .zip(before.premises)
            {
                assert_eq!(data(&returned.authority), expected_data);
                assert_eq!(premise(&returned.premise), expected_premise);
            }
        }
    }
}

#[test]
fn returning_consuming_methods_use_shared_root_before_validation() {
    let source = include_str!("../../queue_dispatch_binding.rs");
    for (method, mode) in [
        ("release_non_data_after_recycle", "AfterRecycle"),
        ("release_non_data_for_returning_destroy", "ReturningDestroy"),
    ] {
        let body = source
            .split(&format!("fn {method}("))
            .nth(1)
            .unwrap()
            .split("\n    }")
            .next()
            .unwrap();
        let compact: String = body.split_whitespace().collect();
        assert!(compact.contains("self.release_non_data(memory,"));
        assert!(body.contains(mode));
        assert!(!body.contains("self.generation"));
    }
    let body = source
        .split("fn release_non_data(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(body.contains("control_release::release_returning_with_v1("));
    assert!(body.contains("control_release::ReturningControlCleanupCustodyV1::new(self, mode)"));
    assert!(body.contains("core::mem::forget"));
    assert!(!body.contains("self.generation") && !body.contains("memory.unmap"));
}

#[test]
fn returning_partial_unmap_keeps_original_authority_at_each_position() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for index in 0..3 {
            for (progress, errno) in [(0, false), (2, false), (0, true), (1, true), (2, true)] {
                let (mut memory, mut r) = fixture(mode);
                let before = snapshot(&r);
                let native = memory.memory_snapshot();
                memory.unmap_control(index + 1, progress, errno);
                match r.release_in_place(&mut memory).unwrap_err() {
                    Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(
                        "unmap_gpu",
                    )) if errno && progress <= 1 => {}
                    Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::KernelResultMalformed(detail),
                    ) => {
                        assert_eq!(
                            detail,
                            if progress == 2 {
                                "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"
                            } else {
                                "shared UNMAP_MEMORY_FROM_GPU full prefix"
                            }
                        );
                        assert!(progress == 2 || !errno);
                    }
                    error => panic!("unexpected returning unmap error: {error:?}"),
                }
                assert_interrupted(&mut r, &before, index, "Mapped");
                let active = r.active_control.as_ref().unwrap().observation();
                assert_eq!(active.stage, Stage::NativeUnmap);
                assert_eq!(active.unmap, (true, Some(!errno), Some(progress)));
                assert_eq!(active.disposal, [(false, None); 3]);
                assert!(active.failed && !active.native_disposed);
                native.assert_control_transition(
                    &memory,
                    &order(&before),
                    index,
                    false,
                    1,
                    Some((false, 0, false, false)),
                );
                assert!(memory.is_quarantined());
            }
        }
    }
}

#[test]
fn returning_actual_commit_rejection_retains_native_settlement_without_model_commit() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for index in 0..3 {
            for release in [false, true] {
                let (mut memory, mut r) = fixture(mode);
                memory.exhaust_control_commit(index + 1, release);
                let before = snapshot(&r);
                let native = memory.memory_snapshot();
                assert!(matches!(
                    r.release_in_place(&mut memory),
                    Err(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::Model(
                            "queue foundation certificate revision exhausted"
                        )
                    ))
                ));
                assert_interrupted(
                    &mut r,
                    &before,
                    index,
                    if release {
                        "NativeDisposed"
                    } else {
                        "Unmapped"
                    },
                );
                let active = r.active_control.as_ref().unwrap().observation();
                assert_eq!(
                    active.stage,
                    if release {
                        Stage::ReleaseCommit
                    } else {
                        Stage::UnmapCommit
                    }
                );
                assert_eq!(active.native_disposed, release);
                assert_eq!(active.unmap, (true, Some(true), Some(1)));
                assert_eq!(
                    active.disposal,
                    if release {
                        [(true, Some(true)); 3]
                    } else {
                        [(false, None); 3]
                    }
                );
                native.assert_control_transition(
                    &memory,
                    &order(&before),
                    index,
                    release,
                    if release { 4 } else { 1 },
                    Some((true, if release { 4 } else { 1 }, release, false)),
                );
                memory
                    .memory_snapshot()
                    .assert_certificate_revision(u64::MAX);
                assert!(memory.is_quarantined());
            }
        }
    }
}

#[test]
fn returning_cancelled_only_history_returns_zero_without_recycle() {
    let (mut memory, mut owner) = pristine_abort::pristine_dispatch_fixture_v1(8);
    let epoch = owner
        .generation
        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(8))
        .unwrap();
    owner.generation.cancel_epoch(epoch).unwrap();
    let mut r = new_root(owner, Mode::ReturningDestroy);
    let before = snapshot(&r);
    r.release_in_place(&mut memory).unwrap();
    let returned = r.take_completed().unwrap();
    assert_eq!(returned.generation(), 0);
    assert_eq!(returned.data.len(), before.data.len());
    assert_eq!(r.generation, before.generation);
    reject_retry(&mut r);
}

#[test]
fn returning_populated_metadata_survives_real_destructive_errors_and_panics() {
    struct FailAt<'a> {
        memory: &'a mut Memory,
        index: usize,
        entered: usize,
        panic: bool,
    }
    impl PristineControlReleaseV1 for FailAt<'_> {
        fn release_control(
            &mut self,
            control: &mut ControlCleanupCustodyV1,
        ) -> Result<(), MemorySessionError> {
            if self.entered == self.index {
                self.memory.fail_cleanup("free", self.panic);
            }
            self.entered += 1;
            self.memory.release_control(control)
        }
    }
    for index in 0..4 {
        for panic in [false, true] {
            let (mut memory, owner) = preparation::control_release_fixture_v1();
            let mut r = new_root(owner, Mode::ReturningDestroy);
            let before = snapshot(&r);
            assert_eq!(order(&before).len(), 4);
            assert!(!before.packets.is_empty());
            let mut callback = FailAt {
                memory: &mut memory,
                index,
                entered: 0,
                panic,
            };
            let result = catch_unwind(AssertUnwindSafe(|| r.release_in_place(&mut callback)));
            if panic {
                assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "free"))
                );
            } else {
                assert!(matches!(
                    result.unwrap(),
                    Err(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::Injected("free")
                    ))
                ));
            }
            assert_eq!(callback.entered, index + 1);
            assert_interrupted(&mut r, &before, index, "Unmapped");
        }
    }
}

use super::*;
use crate::shared_memory::PristineAbortMemoryFixtureV1 as CleanupMemory;

#[derive(Default)]
struct Outside {
    detached: Vec<Gfx942FixedDispatchDataV1>,
    extra_data: Vec<DispatchDataAuthorityV1>,
    extra_premises: Vec<RetainedDataPremiseV1>,
}

#[derive(Debug, Eq, PartialEq)]
struct DetachedData {
    identity: Gfx942SdmaBufferStorageIdentityV1,
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content: Option<Gfx942DeviceContentDescriptorV1>,
}

fn detached_data(data: &[Gfx942FixedDispatchDataV1]) -> Vec<DetachedData> {
    data.iter()
        .map(|d| DetachedData {
            identity: d.sdma_storage_identity(),
            layout: d.layout(),
            initialized: d.is_fully_initialized(),
            content: d.initialized_content(),
        })
        .collect()
}

#[derive(Debug, Eq, PartialEq)]
struct OutsideSnapshot {
    detached: Vec<DetachedData>,
    extra_data: Vec<Data>,
    extra_premises: Vec<Premise>,
    storage: [(usize, usize); 3],
}

fn outside_snapshot(outside: &Outside) -> OutsideSnapshot {
    OutsideSnapshot {
        detached: detached_data(&outside.detached),
        extra_data: outside.extra_data.iter().map(data).collect(),
        extra_premises: outside.extra_premises.iter().map(premise).collect(),
        storage: [
            storage(&outside.detached),
            storage(&outside.extra_data),
            storage(&outside.extra_premises),
        ],
    }
}

// This lower cleanup fixture supplies state premises, not authenticated code.
fn attached_fixture(
    count: usize,
    next_generation: u64,
) -> (CleanupMemory, DispatchResourceOwnerV1, Outside) {
    assert!(matches!(count, 1 | 3));
    let (memory, mut owner) = pristine_abort::pristine_dispatch_fixture_v1(next_generation);
    let outside = Outside {
        detached: Vec::new(),
        extra_data: owner.data.split_off(count),
        extra_premises: owner.data_premises.split_off(count),
    };
    let role = |index| Gfx942DeviceContentRoleV1::new([0x61 + index as u8; 32], 0).unwrap();
    let identity = if count == 1 {
        BoundedPersistentFixedDispatchControlIdentityV1::from_single(
            PersistentFixedDispatchControlIdentityV1 {
                queue: test_dispatch_queue_v1(),
                semantic_sha256: [0x71; 32],
                content_role: role(0),
                data_layout: owner.data_premises[0].layout,
                data_storage: Memory::data_storage(&owner.data[0]),
                effect: DeviceDataEffectV1::ReadWrite,
            },
        )
    } else {
        BoundedPersistentFixedDispatchControlIdentityV1::from_three(
            ThreeBindingPersistentFixedDispatchControlIdentityV1 {
                queue: test_dispatch_queue_v1(),
                semantic_sha256: [0x72; 32],
                content_roles: std::array::from_fn(role),
                data_layouts: std::array::from_fn(|i| owner.data_premises[i].layout),
                data_storage: std::array::from_fn(|i| Memory::data_storage(&owner.data[i])),
                effects: [
                    DeviceDataEffectV1::ReadOnly,
                    DeviceDataEffectV1::ReadOnly,
                    DeviceDataEffectV1::WriteOnly,
                ],
            },
        )
    };
    owner.persistent_control = PersistentFixedDispatchControlStateV1::Attached(identity);
    (memory, owner, outside)
}

fn detach(mut owner: DispatchResourceOwnerV1, outside: &mut Outside) -> Root {
    let PersistentFixedDispatchControlStateV1::Attached(identity) = owner.persistent_control else {
        panic!("fixture must start attached")
    };
    let generation = owner.generation.next_generation;
    let mut completion = test_completion_occurrence_v1(generation);
    completion.queue = identity.queue;
    completion.dispatch_roster.queue = identity.queue;
    completion.signal_mapping.allocation.vm = identity.queue.vm;
    let epoch = owner
        .generation
        .reserve(identity.queue, completion.dispatch_roster)
        .unwrap();
    owner.generation.mark_published(epoch, completion).unwrap();
    owner.generation.complete_epoch(epoch, completion).unwrap();
    owner.generation.recycle_epoch(epoch, completion).unwrap();
    let (returned_generation, detached) = owner
        .detach_persistent_replay_data_after_recycle_v1()
        .unwrap();
    assert_eq!(returned_generation, generation);
    assert_eq!(detached.len(), identity.binding_count());
    assert!(owner.data.is_empty());
    assert_eq!(owner.data_premises.len(), identity.binding_count());
    outside.detached = detached;
    new_root(
        owner,
        Mode::DetachedPersistent {
            expected_generation: generation,
        },
    )
}

fn fixture(count: usize) -> (CleanupMemory, Root, Outside) {
    let (memory, owner, mut outside) = attached_fixture(count, 8);
    let root = detach(owner, &mut outside);
    (memory, root, outside)
}

fn assert_rejected(r: &mut Root, before: Snapshot) {
    let mut expected = before;
    expected.started = true;
    assert_eq!(snapshot(r), expected);
    reject_retry(r);
}

fn assert_metadata(r: &Root, before: &Snapshot) {
    let after = snapshot(r);
    assert_eq!(after.mode, before.mode);
    assert_eq!(after.code_identity, before.code_identity);
    assert_eq!(after.packets, before.packets);
    assert_eq!(after.data, before.data);
    assert_eq!(after.premises, before.premises);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.slots_pointer, before.slots_pointer);
    assert_eq!(after.persistent, before.persistent);
    assert_eq!(after.storage, before.storage);
    assert_eq!(after.returned_generation, None);
    assert!(after.returned.is_empty());
    assert_eq!(r.returned.capacity(), 0);
}

#[test]
fn detached_success_preserves_separate_data_metadata_order_and_zero_return_storage() {
    for count in [1, 3] {
        let (mut memory, mut r, outside) = fixture(count);
        r.return_capacity_override = Some(usize::MAX);
        let before = snapshot(&r);
        let separate = outside_snapshot(&outside);
        let native = memory.memory_snapshot();
        let records = memory.control_record_snapshot(order(&before));
        let currentness = memory.currentness_calls();
        r.release_in_place(&mut memory).unwrap();
        assert!(r.complete && r.active_control.is_none() && r.kernarg.is_none());
        assert!(r.code.as_slice().is_empty());
        assert_metadata(&r, &before);
        native.assert_control_transition(&memory, &order(&before), 3, false, 0, None);
        records.assert_after(&memory, 3, 3);
        assert_eq!(memory.currentness_calls() - currentness, 18);
        reject_retry(&mut r);
        let completed = memory.memory_snapshot();
        drop(r);
        assert_eq!(memory.memory_snapshot(), completed);
        assert_eq!(outside_snapshot(&outside), separate);
        assert!(memory.data_is_retained());
    }
}

#[test]
fn detached_generation_errors_precede_malformed_state_without_cleanup_entry() {
    for count in [1, 3] {
        for phase in 0..6 {
            let (mut memory, mut owner, outside) = attached_fixture(count, 8);
            match phase {
                0 => {}
                1 | 2 => {
                    let epoch = owner
                        .generation
                        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(8))
                        .unwrap();
                    if phase == 1 {
                        owner.generation.cancel_epoch(epoch).unwrap();
                    }
                }
                3 | 4 => {
                    let (epoch, completion) = publish(&mut owner.generation);
                    if phase == 4 {
                        owner.generation.complete_epoch(epoch, completion).unwrap();
                    }
                }
                5 => owner.generation.poison(),
                _ => unreachable!(),
            }
            let removed = owner.data_premises.pop().unwrap();
            let mut r = new_root(
                owner,
                Mode::DetachedPersistent {
                    expected_generation: 8,
                },
            );
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
            let native = memory.memory_snapshot();
            let result = r.release_in_place(&mut memory);
            if phase == 5 {
                assert!(matches!(
                    result,
                    Err(Gfx942DispatchBindingErrorV1::Poisoned)
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            }
            assert_eq!(memory.memory_snapshot(), native);
            assert_rejected(&mut r, before);
            assert_eq!(outside_snapshot(&outside), separate);
            drop(removed);
        }
    }
}

#[test]
fn detached_state_cardinality_and_generation_mismatch_retain_exact_owner() {
    for count in [1, 3] {
        for invalid in 0..6 {
            let (mut memory, mut r, mut outside) = fixture(count);
            let mut removed = None;
            match invalid {
                0 => r.persistent_control = PersistentFixedDispatchControlStateV1::Ordinary,
                1 => {
                    let PersistentFixedDispatchControlStateV1::DataDetached(identity) =
                        r.persistent_control
                    else {
                        unreachable!()
                    };
                    r.persistent_control =
                        PersistentFixedDispatchControlStateV1::Attached(identity);
                }
                2 => r.data.push(outside.extra_data.pop().unwrap()),
                3 => removed = r.data_premises.pop(),
                4 => r.data_premises.push(premise(&r.data_premises[0]).value),
                5 => {
                    r.mode = Mode::DetachedPersistent {
                        expected_generation: 9,
                    }
                }
                _ => unreachable!(),
            }
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
            let native = memory.memory_snapshot();
            assert!(matches!(
                r.release_in_place(&mut memory),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            assert_eq!(memory.memory_snapshot(), native);
            assert_rejected(&mut r, before);
            assert_eq!(outside_snapshot(&outside), separate);
            drop(removed);
        }
    }
}

#[test]
fn detached_cleanup_allows_cancelled_later_reservation_and_exhausted_next_generation() {
    for count in [1, 3] {
        for exhausted in [false, true] {
            let (mut memory, owner, mut outside) =
                attached_fixture(count, if exhausted { u64::MAX - 1 } else { 8 });
            let mut r = detach(owner, &mut outside);
            if !exhausted {
                let generation = r.generation.next_generation;
                let epoch = r
                    .generation
                    .reserve(
                        test_dispatch_queue_v1(),
                        test_completion_roster_v1(generation),
                    )
                    .unwrap();
                r.generation.cancel_epoch(epoch).unwrap();
            }
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
            let native = memory.memory_snapshot();
            let records = memory.control_record_snapshot(order(&before));
            let currentness = memory.currentness_calls();
            r.release_in_place(&mut memory).unwrap();
            assert!(r.complete && r.active_control.is_none() && r.kernarg.is_none());
            assert!(r.code.as_slice().is_empty());
            native.assert_control_transition(&memory, &order(&before), 3, false, 0, None);
            records.assert_after(&memory, 3, 3);
            assert_eq!(memory.currentness_calls() - currentness, 18);
            assert_metadata(&r, &before);
            reject_retry(&mut r);
            assert_eq!(outside_snapshot(&outside), separate);
        }
    }
}

#[test]
fn detached_generation_rejects_with_otherwise_valid_detached_state() {
    for count in [1, 3] {
        for phase in 0..6 {
            let (mut memory, mut r, outside) = fixture(count);
            // Fresh/cancelled-only generation with detached controls is corruption.
            let old_generation = (phase <= 1).then(|| {
                core::mem::replace(
                    &mut r.generation,
                    DispatchGenerationOwnerV1::with_next_generation(8).unwrap(),
                )
            });
            if phase <= 1 {
                r.mode = Mode::DetachedPersistent {
                    expected_generation: 0,
                };
            }
            match phase {
                0 => {}
                1 | 2 => {
                    let generation = r.generation.next_generation;
                    let epoch = r
                        .generation
                        .reserve(
                            test_dispatch_queue_v1(),
                            test_completion_roster_v1(generation),
                        )
                        .unwrap();
                    if phase == 1 {
                        r.generation.cancel_epoch(epoch).unwrap();
                    }
                }
                3 | 4 => {
                    let (epoch, completion) = publish(&mut r.generation);
                    if phase == 4 {
                        r.generation.complete_epoch(epoch, completion).unwrap();
                    }
                }
                5 => r.generation.poison(),
                _ => unreachable!(),
            }
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
            let native = memory.memory_snapshot();
            let result = r.release_in_place(&mut memory);
            if phase == 5 {
                assert!(matches!(
                    result,
                    Err(Gfx942DispatchBindingErrorV1::Poisoned)
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            }
            assert_eq!(memory.memory_snapshot(), native);
            assert_rejected(&mut r, before);
            assert_eq!(outside_snapshot(&outside), separate);
            drop(old_generation);
        }
    }
}

#[test]
fn detached_native_errors_and_panics_preserve_each_control_and_destructive_prefix() {
    for count in [1, 3] {
        for index in 0..3 {
            for (op, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                .into_iter()
                .enumerate()
            {
                for panic in [false, true] {
                    let (mut memory, mut r, outside) = fixture(count);
                    let before = snapshot(&r);
                    let separate = outside_snapshot(&outside);
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
                        assert!(matches!(result.unwrap(),
                            Err(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(actual)))
                            if actual == operation));
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
                    assert_eq!(outside_snapshot(&outside), separate);
                }
            }
        }
    }
}

#[test]
fn detached_currentness_boundaries_keep_exact_mapped_unmapped_and_disposed_custody() {
    for count in [1, 3] {
        for offset in 1_usize..=18 {
            for panic in [false, true] {
                let (mut memory, mut r, outside) = fixture(count);
                let before = snapshot(&r);
                let separate = outside_snapshot(&outside);
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
                let active = r.active_control.as_ref().unwrap().observation();
                assert!(active.started && active.failed);
                assert_eq!(active.native_disposed, point == 6);
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
                    std::array::from_fn(|i| {
                        if i < point.saturating_sub(3) {
                            (true, Some(true))
                        } else {
                            (false, None)
                        }
                    })
                );
                let calls = point.saturating_sub(2).max(usize::from(point >= 2));
                native.assert_control_transition(
                    &memory,
                    &order(&before),
                    index,
                    point >= 3,
                    calls,
                    Some((point >= 3, calls, false, false)),
                );
                assert_eq!(memory.currentness_calls() - currentness, offset);
                assert!(memory.is_quarantined() && memory.data_is_retained());
                assert_eq!(outside_snapshot(&outside), separate);
            }
        }
    }
}

#[test]
fn detached_projection_failures_keep_native_receipts_and_uncommitted_model() {
    for count in [1, 3] {
        for index in 0..3 {
            for release in [false, true] {
                for panic in [false, true] {
                    let (mut memory, mut r, outside) = fixture(count);
                    let before = snapshot(&r);
                    let separate = outside_snapshot(&outside);
                    let native = memory.memory_snapshot();
                    memory.fail_control_commit(index + 1, release, panic);
                    let result = catch_unwind(AssertUnwindSafe(|| r.release_in_place(&mut memory)));
                    let stage = if release {
                        Stage::ReleaseCommit
                    } else {
                        Stage::UnmapCommit
                    };
                    if panic {
                        assert_eq!(
                            result.unwrap_err().downcast_ref::<(&str, Stage)>(),
                            Some(&("control cleanup projection", stage))
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
                    let active = r.active_control.as_ref().unwrap().observation();
                    assert!(active.failed && active.started);
                    assert_eq!(active.stage, stage);
                    assert_eq!(active.native_disposed, release);
                    native.assert_control_transition(
                        &memory,
                        &order(&before),
                        index,
                        release,
                        if release { 4 } else { 1 },
                        Some((true, if release { 4 } else { 1 }, release, false)),
                    );
                    assert!(memory.is_quarantined() && memory.data_is_retained());
                    assert_eq!(outside_snapshot(&outside), separate);
                }
            }
        }
    }
}

#[test]
fn detached_partial_unmap_never_advances_or_reconstructs_authority() {
    for count in [1, 3] {
        for index in 0..3 {
            for (progress, errno) in [(0, false), (2, false), (0, true), (1, true), (2, true)] {
                let (mut memory, mut r, outside) = fixture(count);
                let before = snapshot(&r);
                let separate = outside_snapshot(&outside);
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
                    error => panic!("unexpected detached unmap error: {error:?}"),
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
                assert!(memory.is_quarantined() && memory.data_is_retained());
                assert_eq!(outside_snapshot(&outside), separate);
            }
        }
    }
}

#[test]
fn detached_actual_commit_rejection_keeps_native_settlement_without_model_commit() {
    for count in [1, 3] {
        for index in 0..3 {
            for release in [false, true] {
                let (mut memory, mut r, outside) = fixture(count);
                memory.exhaust_control_commit(index + 1, release);
                let before = snapshot(&r);
                let separate = outside_snapshot(&outside);
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
                assert!(memory.is_quarantined() && memory.data_is_retained());
                assert_eq!(outside_snapshot(&outside), separate);
            }
        }
    }
}

#[test]
fn detached_incomplete_callback_retains_active_owner_and_cannot_retry_or_extract() {
    struct Incomplete<'a> {
        memory: &'a mut CleanupMemory,
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
    for count in [1, 3] {
        for index in 0..3 {
            let (mut memory, mut r, outside) = fixture(count);
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
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
            assert_eq!(outside_snapshot(&outside), separate);
        }
    }
}

#[test]
fn detached_wrapper_retains_root_before_error_and_original_panic_and_drops_only_complete() {
    for count in [1, 3] {
        for scenario in 0..4 {
            let (mut memory, mut r, outside) = fixture(count);
            match scenario {
                0 => r.generation.poison(),
                1 => memory.fail_control(2, "free", false),
                2 => memory.fail_control(3, "unmap_cpu", true),
                3 => r.return_capacity_override = Some(usize::MAX),
                _ => unreachable!(),
            }
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
            let native = memory.memory_snapshot();
            let mut retained = None;
            let result = catch_unwind(AssertUnwindSafe(|| {
                release_detached_persistent_with_v1(r, &mut memory, |mut root| {
                    assert!(retained.is_none());
                    assert_inputs(&root, &before);
                    if scenario == 0 {
                        let after = snapshot(&root);
                        assert_eq!(after.kernarg, before.kernarg);
                        assert_eq!(after.code, before.code);
                        assert_eq!(after.code_pointer, before.code_pointer);
                        assert_eq!(after.storage, before.storage);
                        assert_eq!(after.active, None);
                        assert_eq!(after.returned_generation, None);
                        assert!(after.started);
                        reject_retry(&mut root);
                    } else {
                        assert_interrupted(&mut root, &before, scenario, "Unmapped");
                    }
                    retained = Some(root);
                })
            }));
            match scenario {
                0 => {
                    assert!(matches!(
                        result.unwrap(),
                        Err(Gfx942DispatchBindingErrorV1::Poisoned)
                    ));
                    assert_eq!(memory.memory_snapshot(), native);
                }
                1 => assert!(matches!(
                    result.unwrap(),
                    Err(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::Injected("free")
                    ))
                )),
                2 => assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "unmap_cpu"))
                ),
                3 => {
                    result.unwrap().unwrap();
                    native.assert_control_transition(&memory, &order(&before), 3, false, 0, None);
                }
                _ => unreachable!(),
            }
            assert_eq!(retained.is_some(), scenario != 3);
            assert_eq!(outside_snapshot(&outside), separate);
        }
    }
}

#[test]
fn detached_and_returning_wrappers_reject_wrong_modes_without_losing_outputs() {
    for mode in [Mode::AfterRecycle, Mode::ReturningDestroy] {
        for completed in [false, true] {
            let (mut memory, mut r) = super::fixture(mode);
            if completed {
                r.release_in_place(&mut memory).unwrap();
                assert!(!r.returned.is_empty());
            }
            let before = snapshot(&r);
            let native = memory.memory_snapshot();
            let mut retained = None;
            assert!(matches!(
                release_detached_persistent_with_v1(r, &mut NoEntry, |root| {
                    assert_eq!(snapshot(&root), before);
                    retained = Some(root);
                }),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            let r = retained.as_mut().expect("wrong-mode root retained");
            assert_eq!(snapshot(r), before);
            if completed {
                let returned = r.take_completed().unwrap();
                assert_eq!(returned.data.len(), before.returned.len());
                for (actual, expected) in returned.data.iter().zip(&before.returned) {
                    assert_eq!(
                        (data(&actual.authority), premise(&actual.premise)),
                        *expected
                    );
                }
            }
            assert_eq!(memory.memory_snapshot(), native);
        }
    }
    for count in [1, 3] {
        for completed in [false, true] {
            let (mut memory, mut r, outside) = fixture(count);
            if completed {
                r.release_in_place(&mut memory).unwrap();
            }
            let before = snapshot(&r);
            let separate = outside_snapshot(&outside);
            let native = memory.memory_snapshot();
            let mut retained = None;
            assert!(matches!(
                release_returning_with_v1(r, &mut NoEntry, |root| {
                    assert_eq!(snapshot(&root), before);
                    retained = Some(root);
                }),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            let r = retained.as_mut().expect("detached root retained");
            assert_eq!(snapshot(r), before);
            assert!(matches!(
                r.take_completed(),
                Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            assert_eq!(snapshot(r), before);
            assert_eq!(memory.memory_snapshot(), native);
            assert_eq!(outside_snapshot(&outside), separate);
        }
    }
}

#[test]
fn detached_consuming_method_roots_owner_before_generation_validation() {
    let source = include_str!("../../queue_dispatch_binding.rs");
    let body = source
        .split("fn release_detached_persistent_control_v1(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    let compact: String = body.split_whitespace().collect();
    assert!(compact.contains("control_release::release_detached_persistent_with_v1("));
    assert!(compact.contains("control_release::ReturningControlCleanupCustodyV1::new(self,"));
    let mode = compact
        .split("ReturningControlModeV1::DetachedPersistent{")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    assert_eq!(mode.trim_end_matches(','), "expected_generation");
    assert!(compact.contains("core::mem::forget"));
    assert!(!body.contains("self.generation") && !body.contains("self.validate_"));
    assert!(!body.contains("memory.unmap") && !body.contains("memory.release"));
}

fn prepared(count: usize) -> (Memory, Root, Outside) {
    let (memory, owner) = match count {
        1 => preparation::single_persistent_control_fixture_v1(),
        3 => preparation::three_persistent_control_fixture_v1(),
        _ => unreachable!(),
    };
    let mut outside = Outside::default();
    let r = detach(owner, &mut outside);
    assert_eq!(r.data_premises.len(), count);
    assert_eq!(r.packets.len(), 1);
    assert_eq!(r.code_identity.len(), if count == 1 { 3 } else { 1 });
    assert!(
        r.data_premises
            .iter()
            .any(|p| !p.writable_ranges.is_empty())
    );
    (memory, r, outside)
}

struct Recorded<'a> {
    memory: &'a mut Memory,
    entered: Vec<SharedGttAllocationIdentityV1>,
    failure: Option<(usize, bool)>,
}

impl PristineControlReleaseV1 for Recorded<'_> {
    fn release_control(
        &mut self,
        control: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        if let Some((index, panic)) = self.failure
            && index == self.entered.len()
        {
            self.memory.fail_cleanup("free", panic);
        }
        self.entered.push(control.observation().identity);
        self.memory.release_control(control)
    }
}

#[test]
fn detached_real_preparation_preserves_populated_metadata_and_separate_native_data() {
    for count in [1, 3] {
        let (mut memory, mut r, outside) = prepared(count);
        let extras = memory.roster();
        let extra_storage = storage(&extras);
        let extra_data = detached_data(&extras);
        let separate = outside_snapshot(&outside);
        let before = snapshot(&r);
        let native = memory.observation();
        let mut callback = Recorded {
            memory: &mut memory,
            entered: Vec::new(),
            failure: None,
        };
        r.return_capacity_override = Some(usize::MAX);
        r.release_in_place(&mut callback).unwrap();
        assert_eq!(callback.entered, order(&before));
        assert!(r.complete && r.active_control.is_none() && r.kernarg.is_none());
        assert!(r.code.as_slice().is_empty());
        assert_metadata(&r, &before);
        reject_retry(&mut r);
        memory.assert_data_unchanged(&native);
        assert_eq!(memory.observation().controls, 0);
        let completed = memory.observation();
        drop(r);
        assert_eq!(memory.observation(), completed);
        assert_eq!(outside_snapshot(&outside), separate);
        assert_eq!(detached_data(&extras), extra_data);
        assert_eq!(storage(&extras), extra_storage);
    }
}

#[test]
fn detached_real_preparation_error_and_panic_keep_metadata_controls_and_data() {
    for count in [1, 3] {
        for index in 0..if count == 1 { 4 } else { 2 } {
            for panic in [false, true] {
                let (mut memory, mut r, outside) = prepared(count);
                let extras = memory.roster();
                let extra_data = detached_data(&extras);
                let extra_storage = storage(&extras);
                let before = snapshot(&r);
                let separate = outside_snapshot(&outside);
                let native = memory.observation();
                let mut callback = Recorded {
                    memory: &mut memory,
                    entered: Vec::new(),
                    failure: Some((index, panic)),
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
                assert_eq!(callback.entered, order(&before)[..=index]);
                assert_interrupted(&mut r, &before, index, "Unmapped");
                memory.assert_data_unchanged(&native);
                assert_eq!(outside_snapshot(&outside), separate);
                assert_eq!(detached_data(&extras), extra_data);
                assert_eq!(storage(&extras), extra_storage);
            }
        }
    }
}

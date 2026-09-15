use super::*;

const MODES: [Mode; 2] = [
    Mode::PersistentBeforePublication,
    Mode::PersistentAfterRecycle,
];

enum Failure {
    Error(Gfx942DispatchBindingErrorV1),
    Panic(Box<dyn std::any::Any + Send>),
}

struct Failed {
    failure: Failure,
    data: Vec<Gfx942FixedDispatchDataV1>,
    root: Root,
}

fn failed(root: Root, memory: &mut impl PristineControlReleaseV1) -> Failed {
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        release_persistent_with_v1(root, memory, |root| {
            assert!(retained.is_none(), "persistent root retained twice");
            retained = Some(root);
        })
    }));
    let (failure, data) = match result {
        Ok(Err((error, data))) => (Failure::Error(error), data),
        Err(payload) => (Failure::Panic(payload), Vec::new()),
        Ok(Ok(_)) => panic!("persistent cleanup unexpectedly succeeded"),
    };
    Failed {
        failure,
        data,
        root: retained.expect("persistent cleanup lost its retained root"),
    }
}

fn generation(mode: Mode) -> u64 {
    match mode {
        Mode::PersistentBeforePublication => 0,
        Mode::PersistentAfterRecycle => 8,
        _ => unreachable!(),
    }
}

fn assert_output(output: &[Gfx942FixedDispatchDataV1], before: &Snapshot) {
    let expected: Vec<_> = before
        .data
        .iter()
        .zip(&before.premises)
        .map(|(data, premise)| PersistentData {
            identity: data.identity,
            layout: premise.value.layout,
            initialized: premise.value.fully_initialized,
            content: None,
        })
        .collect();
    assert_eq!(
        persistent_data(output),
        expected,
        "persistent output lost identity, order or initialization"
    );
}

fn assert_metadata(root: &Root, before: &Snapshot) {
    let after = snapshot(root);
    assert_eq!(after.mode, before.mode);
    assert_eq!(after.code_identity, before.code_identity);
    assert_eq!(after.packets, before.packets);
    assert_eq!(after.premises, before.premises);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.slots_pointer, before.slots_pointer);
    assert_eq!(after.persistent, before.persistent);
    assert_eq!(after.storage[..4], before.storage[..4]);
    assert!(after.returned.is_empty());
    assert_eq!(after.returned_generation, None);
    assert_eq!(
        root.returned.capacity(),
        0,
        "persistent cleanup allocated ordinary output"
    );
}

fn assert_no_take(root: &mut Root) {
    let before = snapshot(root);
    assert!(matches!(
        root.take_persistent_data(),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(
        snapshot(root),
        before,
        "rejected persistent extraction changed custody"
    );
    reject_retry(root);
}

fn assert_split(failed: &mut Failed, before: &Snapshot, expected_generation: u64, panic: bool) {
    assert_metadata(&failed.root, before);
    assert!(failed.root.started && !failed.root.complete);
    if panic {
        assert!(failed.data.is_empty());
        assert_eq!(
            snapshot(&failed.root).data,
            before.data,
            "panic lost original data custody"
        );
        assert_eq!(
            failed.root.persistent_output,
            PersistentOutputStateV1::Prepared(expected_generation)
        );
        assert!(failed.root.persistent_returned.capacity() >= before.data.len());
    } else {
        assert_output(&failed.data, before);
        assert!(failed.data.capacity() >= before.data.len());
        assert!(
            failed.root.data.is_empty(),
            "normal error retained already-returned data"
        );
        assert_eq!(
            failed.root.persistent_output,
            PersistentOutputStateV1::Taken
        );
        assert_eq!(failed.root.persistent_returned.capacity(), 0);
    }
    assert_no_take(&mut failed.root);
}

fn assert_control(root: &Root, before: &Snapshot, index: usize, owner: &str) {
    let after = snapshot(root);
    let active = after
        .active
        .as_ref()
        .expect("persistent failure lost active control");
    let original = if index == 0 {
        before.kernarg.as_ref().unwrap()
    } else {
        &before.code[index - 1]
    };
    assert_eq!(active.identity, order(before)[index]);
    assert_eq!(active.layout, original.layout);
    assert_eq!(active.owner, owner);
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
    assert_eq!(after.kernarg, None);
    assert_eq!(after.code, before.code[index..]);
    assert_eq!(
        after.code_pointer,
        before.code_pointer + index * core::mem::size_of::<CodeAuthority>()
    );
}

fn assert_native_failure(failure: &Failure, operation: &'static str, panic: bool) {
    match (failure, panic) {
        (
            Failure::Error(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(
                actual,
            ))),
            false,
        ) => assert_eq!(*actual, operation),
        (Failure::Panic(payload), true) => assert_eq!(
            payload.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", operation))
        ),
        _ => panic!("unexpected persistent native failure"),
    }
}

#[test]
fn persistent_success_preserves_exact_mixed_data_and_forward_cleanup() {
    for mode in MODES {
        for wrapper in [false, true] {
            let (mut memory, mut root) = fixture(mode);
            let before = snapshot(&root);
            assert_eq!(before.data.len(), 5);
            assert!(before.premises[2].value.initialized_content.is_some());
            let native = memory.memory_snapshot();
            let records = memory.control_record_snapshot(order(&before));
            let currentness = memory.currentness_calls();
            let (actual_generation, output) = if wrapper {
                match release_persistent_with_v1(root, &mut memory, |_| {
                    panic!("success retained root")
                }) {
                    Ok(output) => output,
                    Err(_) => panic!("persistent wrapper rejected success"),
                }
            } else {
                root.release_in_place(&mut memory).unwrap();
                assert!(root.complete && root.active_control.is_none());
                assert!(root.kernarg.is_none() && root.code.as_slice().is_empty());
                assert_metadata(&root, &before);
                assert_eq!(snapshot(&root).data, before.data);
                assert_eq!(
                    root.persistent_output,
                    PersistentOutputStateV1::Returnable(generation(mode))
                );
                let output = root.take_persistent_data().unwrap();
                assert_metadata(&root, &before);
                assert_no_take(&mut root);
                output
            };
            assert_eq!(actual_generation, generation(mode));
            assert_output(&output, &before);
            native.assert_control_transition(&memory, &order(&before), 3, false, 0, None);
            records.assert_after(&memory, 3, 3);
            assert_eq!(memory.currentness_calls() - currentness, 18);
            assert!(memory.data_is_retained());
        }
    }
}

#[test]
fn persistent_generation_errors_precede_cardinality_and_capacity() {
    for mode in MODES {
        for phase in 0..5 {
            if phase == 4 && mode == Mode::PersistentBeforePublication {
                continue;
            }
            let (mut memory, owner) = pristine_abort::pristine_dispatch_fixture_v1(8);
            let mut root = new_root(owner, mode);
            match phase {
                0 => {
                    root.generation
                        .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(8))
                        .unwrap();
                }
                1 | 2 => {
                    let (epoch, completion) = publish(&mut root.generation);
                    if phase == 2 {
                        root.generation.complete_epoch(epoch, completion).unwrap();
                    }
                }
                3 => root.generation.poison(),
                4 => {}
                _ => unreachable!(),
            }
            let removed = root.data_premises.pop().unwrap();
            root.return_capacity_override = Some(usize::MAX);
            let mut before = snapshot(&root);
            let native = memory.memory_snapshot();
            let mut failed = failed(root, &mut memory);
            if phase == 3 {
                assert!(matches!(
                    failed.failure,
                    Failure::Error(Gfx942DispatchBindingErrorV1::Poisoned)
                ));
            } else {
                assert!(matches!(
                    failed.failure,
                    Failure::Error(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            }
            assert!(failed.data.is_empty());
            before.started = true;
            assert_eq!(snapshot(&failed.root), before);
            assert_eq!(memory.memory_snapshot(), native);
            assert_no_take(&mut failed.root);
            drop(removed);
        }
    }
}

#[test]
fn persistent_cardinality_and_capacity_reject_before_disposal() {
    for mode in MODES {
        for invalid in 0..3 {
            let (mut memory, mut root) = fixture(mode);
            let removed = if invalid == 0 {
                root.data_premises.pop()
            } else {
                None
            };
            root.return_capacity_override = Some(if invalid == 2 { 0 } else { usize::MAX });
            let mut before = snapshot(&root);
            let native = memory.memory_snapshot();
            let mut failed = failed(root, &mut memory);
            if invalid == 0 {
                assert!(matches!(
                    failed.failure,
                    Failure::Error(Gfx942DispatchBindingErrorV1::InvalidData {
                        index: 4,
                        detail: "retained data/premise cardinality"
                    })
                ));
            } else {
                assert!(matches!(
                    failed.failure,
                    Failure::Error(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                        operation: "persistent dispatch data"
                    })
                ));
            }
            assert!(failed.data.is_empty());
            before.started = true;
            assert_eq!(snapshot(&failed.root), before);
            assert_eq!(memory.memory_snapshot(), native);
            assert_no_take(&mut failed.root);
            drop(removed);
        }
    }
}

#[test]
fn persistent_zero_data_uses_explicit_readiness_and_one_shot_transfer() {
    for mode in MODES {
        for scenario in 0..3 {
            let (mut memory, mut root) = fixture(mode);
            let outside = (
                core::mem::take(&mut root.data),
                core::mem::take(&mut root.data_premises),
            );
            let before = snapshot(&root);
            if scenario == 0 {
                root.release_in_place(&mut memory).unwrap();
                assert_eq!(
                    root.persistent_output,
                    PersistentOutputStateV1::Returnable(generation(mode))
                );
                let (actual_generation, output) = root.take_persistent_data().unwrap();
                assert_eq!(actual_generation, generation(mode));
                assert!(output.is_empty());
                assert_eq!(root.persistent_output, PersistentOutputStateV1::Taken);
                assert_no_take(&mut root);
            } else {
                let panic = scenario == 2;
                memory.fail_control(1, "free", panic);
                let mut failed = failed(root, &mut memory);
                assert_native_failure(&failed.failure, "free", panic);
                assert_split(&mut failed, &before, generation(mode), panic);
            }
            assert_eq!(outside.0.len(), 5);
            assert!(memory.data_is_retained());
        }
    }
}

#[test]
fn persistent_cancelled_history_and_exhausted_next_generation_preserve_admission() {
    for mode in MODES {
        for history in 0..4 {
            let next = if history == 3 { u64::MAX - 1 } else { 8 };
            let (mut memory, mut owner) = pristine_abort::pristine_dispatch_fixture_v1(next);
            let expected = if history >= 2 {
                recycle(&mut owner.generation)
            } else {
                0
            };
            if matches!(history, 1 | 2) {
                let next = owner.generation.next_generation;
                let epoch = owner
                    .generation
                    .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(next))
                    .unwrap();
                owner.generation.cancel_epoch(epoch).unwrap();
            }
            if history == 3 {
                assert_eq!(expected, u64::MAX - 1);
                assert!(
                    owner
                        .generation
                        .reserve(
                            test_dispatch_queue_v1(),
                            test_completion_roster_v1(u64::MAX)
                        )
                        .is_err()
                );
            }
            let root = new_root(owner, mode);
            let before = snapshot(&root);
            if expected == 0 && mode == Mode::PersistentAfterRecycle {
                let mut failed = failed(root, &mut memory);
                assert!(matches!(
                    failed.failure,
                    Failure::Error(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
                assert!(failed.data.is_empty());
                let mut expected_root = before;
                expected_root.started = true;
                assert_eq!(snapshot(&failed.root), expected_root);
                assert_no_take(&mut failed.root);
            } else {
                let Ok((actual, output)) = release_persistent_with_v1(root, &mut memory, |_| {
                    panic!("valid generation retained root")
                }) else {
                    panic!("valid persistent generation rejected");
                };
                assert_eq!(actual, expected);
                assert_output(&output, &before);
            }
        }
    }
}

#[test]
fn persistent_native_errors_return_data_while_panics_retain_it_at_every_control() {
    for mode in MODES {
        for index in 0..3 {
            for (op, operation) in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
                .into_iter()
                .enumerate()
            {
                for panic in [false, true] {
                    let (mut memory, root) = fixture(mode);
                    let before = snapshot(&root);
                    let native = memory.memory_snapshot();
                    let records = memory.control_record_snapshot(order(&before));
                    memory.fail_control(index + 1, operation, panic);
                    let mut failed = failed(root, &mut memory);
                    assert_native_failure(&failed.failure, operation, panic);
                    assert_split(&mut failed, &before, generation(mode), panic);
                    assert_control(
                        &failed.root,
                        &before,
                        index,
                        if op == 0 { "Mapped" } else { "Unmapped" },
                    );
                    let active = failed.root.active_control.as_ref().unwrap().observation();
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
                        for item in disposal.iter_mut().take(op - 1) {
                            *item = (true, Some(true));
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
fn persistent_currentness_failures_preserve_all_eighteen_control_boundaries() {
    for mode in MODES {
        for offset in 1_usize..=18 {
            for panic in [false, true] {
                let (mut memory, root) = fixture(mode);
                let before = snapshot(&root);
                let native = memory.memory_snapshot();
                let currentness = memory.currentness_calls();
                memory.fail_currentness(offset, panic);
                let mut failed = failed(root, &mut memory);
                assert_native_failure(&failed.failure, "currentness", panic);
                assert_split(&mut failed, &before, generation(mode), panic);
                let index = (offset - 1) / 6;
                let point = (offset - 1) % 6 + 1;
                assert_control(
                    &failed.root,
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
                let active = failed.root.active_control.as_ref().unwrap().observation();
                let calls = point.saturating_sub(2).max(usize::from(point >= 2));
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
fn persistent_partial_unmap_returns_data_without_losing_original_control() {
    for mode in MODES {
        for index in 0..3 {
            for (progress, errno) in [(0, false), (2, false), (0, true), (1, true), (2, true)] {
                let (mut memory, root) = fixture(mode);
                let before = snapshot(&root);
                let native = memory.memory_snapshot();
                memory.unmap_control(index + 1, progress, errno);
                let mut failed = failed(root, &mut memory);
                match &failed.failure {
                    Failure::Error(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::Injected("unmap_gpu"),
                    )) if errno && progress <= 1 => {}
                    Failure::Error(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::KernelResultMalformed(detail),
                    )) => {
                        assert_eq!(
                            *detail,
                            if progress == 2 {
                                "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"
                            } else {
                                "shared UNMAP_MEMORY_FROM_GPU full prefix"
                            }
                        );
                        assert!(progress == 2 || !errno);
                    }
                    _ => panic!("unexpected persistent partial-unmap result"),
                }
                assert_split(&mut failed, &before, generation(mode), false);
                assert_control(&failed.root, &before, index, "Mapped");
                let active = failed.root.active_control.as_ref().unwrap().observation();
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
fn persistent_projection_and_actual_commit_failures_preserve_receipts_and_output_split() {
    for mode in MODES {
        for index in 0..3 {
            for release in [false, true] {
                for kind in 0..3 {
                    let (mut memory, root) = fixture(mode);
                    let before = snapshot(&root);
                    let panic = kind == 1;
                    if kind == 2 {
                        memory.exhaust_control_commit(index + 1, release);
                    } else {
                        memory.fail_control_commit(index + 1, release, panic);
                    }
                    let native = memory.memory_snapshot();
                    let mut failed = failed(root, &mut memory);
                    let stage = if release {
                        Stage::ReleaseCommit
                    } else {
                        Stage::UnmapCommit
                    };
                    match (&failed.failure, kind) {
                        (Failure::Panic(payload), 1) => assert_eq!(
                            payload.downcast_ref::<(&str, Stage)>(),
                            Some(&("control cleanup projection", stage))
                        ),
                        (
                            Failure::Error(Gfx942DispatchBindingErrorV1::Memory(
                                MemorySessionError::Injected("control cleanup projection"),
                            )),
                            0,
                        ) => {}
                        (
                            Failure::Error(Gfx942DispatchBindingErrorV1::Memory(
                                MemorySessionError::Model(
                                    "queue foundation certificate revision exhausted",
                                ),
                            )),
                            2,
                        ) => {}
                        _ => panic!("unexpected persistent commit failure"),
                    }
                    assert_split(&mut failed, &before, generation(mode), panic);
                    assert_control(
                        &failed.root,
                        &before,
                        index,
                        if release {
                            "NativeDisposed"
                        } else {
                            "Unmapped"
                        },
                    );
                    let active = failed.root.active_control.as_ref().unwrap().observation();
                    assert_eq!(active.stage, stage);
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
                    if kind == 2 {
                        memory
                            .memory_snapshot()
                            .assert_certificate_revision(u64::MAX);
                    }
                }
            }
        }
    }
}

#[test]
fn persistent_incomplete_callbacks_return_data_but_never_claim_control_completion() {
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
    for mode in MODES {
        for index in 0..3 {
            let (mut memory, root) = fixture(mode);
            let before = snapshot(&root);
            let native = memory.memory_snapshot();
            let mut callback = Incomplete {
                memory: &mut memory,
                stop: index,
                entered: 0,
            };
            let mut failed = failed(root, &mut callback);
            assert_eq!(callback.entered, index + 1);
            assert!(matches!(
                failed.failure,
                Failure::Error(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
            assert_split(&mut failed, &before, generation(mode), false);
            assert_control(&failed.root, &before, index, "Mapped");
            assert!(
                !failed
                    .root
                    .active_control
                    .as_ref()
                    .unwrap()
                    .observation()
                    .started
            );
            native.assert_control_transition(&memory, &order(&before), index, false, 0, None);
        }
    }
}

#[test]
fn persistent_wrong_wrappers_and_repeated_extraction_reject_before_effects() {
    for mode in [
        Mode::AfterRecycle,
        Mode::ReturningDestroy,
        Mode::DetachedPersistent {
            expected_generation: 8,
        },
    ] {
        let (_memory, root) = fixture(mode);
        let before = snapshot(&root);
        let failed = failed(root, &mut NoEntry);
        assert!(matches!(
            failed.failure,
            Failure::Error(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert!(failed.data.is_empty());
        assert_eq!(snapshot(&failed.root), before);
    }
    for mode in MODES {
        for detached in [false, true] {
            let (_memory, root) = fixture(mode);
            let before = snapshot(&root);
            let mut retained = None;
            if detached {
                assert!(matches!(
                    release_detached_persistent_with_v1(root, &mut NoEntry, |root| retained =
                        Some(root)),
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            } else {
                assert!(matches!(
                    release_returning_with_v1(root, &mut NoEntry, |root| retained = Some(root)),
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ));
            }
            assert_eq!(snapshot(retained.as_ref().unwrap()), before);
        }
        let (mut memory, mut root) = fixture(mode);
        root.release_in_place(&mut memory).unwrap();
        let ready = snapshot(&root);
        assert!(matches!(
            root.take_completed(),
            Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert_eq!(snapshot(&root), ready);
        let _output = root.take_persistent_data().unwrap();
        assert_no_take(&mut root);
        let before = snapshot(&root);
        let failed = failed(root, &mut NoEntry);
        assert!(failed.data.is_empty());
        assert_eq!(snapshot(&failed.root), before);
    }
}

fn prepared(count: usize, mode: Mode) -> (Memory, Root, u64) {
    let (memory, mut owner) = match count {
        1 => preparation::single_persistent_control_fixture_v1(),
        3 => preparation::three_persistent_control_fixture_v1(),
        _ => unreachable!(),
    };
    let expected = if mode == Mode::PersistentAfterRecycle {
        let PersistentFixedDispatchControlStateV1::Attached(identity) = owner.persistent_control
        else {
            panic!("genuine preparation was not attached")
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
        generation
    } else {
        0
    };
    let root = new_root(owner, mode);
    assert_eq!(root.data.len(), count);
    assert_eq!(root.data_premises.len(), count);
    assert_eq!(root.packets.len(), 1);
    assert_eq!(root.code_identity.len(), if count == 1 { 3 } else { 1 });
    assert!(
        root.data_premises
            .iter()
            .any(|p| !p.writable_ranges.is_empty())
    );
    (memory, root, expected)
}

#[test]
fn persistent_genuine_single_and_three_binding_preparation_preserve_metadata_and_data() {
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
    for mode in MODES {
        for count in [1, 3] {
            let controls = if count == 1 { 4 } else { 2 };
            for scenario in 0..=controls * 2 {
                let (mut memory, mut root, expected_generation) = prepared(count, mode);
                let before = snapshot(&root);
                let extras = memory.roster();
                let native = memory.observation();
                let extra_data = persistent_data(&extras);
                let extra_storage = storage(&extras);
                let failure = (scenario > 0).then(|| ((scenario - 1) / 2, scenario % 2 == 0));
                let mut callback = Recorded {
                    memory: &mut memory,
                    entered: Vec::with_capacity(controls),
                    failure,
                };
                if let Some((index, panic)) = failure {
                    let mut failed = failed(root, &mut callback);
                    assert_native_failure(&failed.failure, "free", panic);
                    assert_split(&mut failed, &before, expected_generation, panic);
                    assert_control(&failed.root, &before, index, "Unmapped");
                    assert_eq!(callback.entered, order(&before)[..=index]);
                } else {
                    root.release_in_place(&mut callback).unwrap();
                    assert_eq!(callback.entered, order(&before));
                    assert_metadata(&root, &before);
                    let (actual_generation, output) = root.take_persistent_data().unwrap();
                    assert_eq!(actual_generation, expected_generation);
                    assert_output(&output, &before);
                    assert_metadata(&root, &before);
                    assert_no_take(&mut root);
                }
                memory.assert_data_unchanged(&native);
                assert_eq!(persistent_data(&extras), extra_data);
                assert_eq!(storage(&extras), extra_storage);
            }
        }
    }
}

#[test]
fn persistent_output_capacity_survives_success_and_error_extraction() {
    for mode in MODES {
        for error in [false, true] {
            let (mut memory, mut root) = fixture(mode);
            if error {
                memory.fail_control(2, "free", false);
            }
            let before = snapshot(&root);
            assert_eq!(root.persistent_returned.capacity(), 0);
            let result = root.release_in_place(&mut memory);
            assert_eq!(result.is_err(), error);
            let reserved = storage(&root.persistent_returned);
            assert!(reserved.1 >= before.data.len());
            assert_eq!(root.returned.capacity(), 0);
            let (actual_generation, output) = root.take_persistent_data().unwrap();
            assert_eq!(actual_generation, generation(mode));
            assert_eq!(
                storage(&output),
                reserved,
                "persistent extraction changed preallocated output storage"
            );
            assert_output(&output, &before);
            assert_no_take(&mut root);
        }
    }
}

#[test]
fn persistent_panic_root_rejects_extraction_without_mutating_retained_custody() {
    for mode in MODES {
        for index in 0..3 {
            let (mut memory, mut root) = fixture(mode);
            let before = snapshot(&root);
            memory.fail_control(index + 1, "free", true);
            let Err(payload) =
                catch_unwind(AssertUnwindSafe(|| root.release_in_place(&mut memory)))
            else {
                panic!("expected original native panic")
            };
            assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "free"))
            );
            let retained = snapshot(&root);
            assert!(
                matches!(
                    root.take_persistent_data(),
                    Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
                ),
                "panic root must not authorize persistent extraction"
            );
            assert_eq!(snapshot(&root), retained);
            assert_eq!(
                root.persistent_output,
                PersistentOutputStateV1::Prepared(generation(mode))
            );
            assert_eq!(retained.data, before.data);
            assert_metadata(&root, &before);
            assert_control(&root, &before, index, "Unmapped");
            assert_no_take(&mut root);
        }
    }
}

#[test]
fn persistent_consuming_methods_route_full_owner_before_validation() {
    let source = include_str!("../../queue_dispatch_binding.rs");
    for (method, mode) in [
        (
            "release_persistent_data_before_publication",
            "PersistentBeforePublication",
        ),
        (
            "release_persistent_data_after_recycle",
            "PersistentAfterRecycle",
        ),
    ] {
        let body = source
            .split(&format!("fn {method}("))
            .nth(1)
            .unwrap()
            .split("\n    }")
            .next()
            .unwrap();
        let compact: String = body.split_whitespace().collect();
        assert!(compact.contains("self.release_persistent_data(memory,"));
        assert!(body.contains(mode));
        assert!(!body.contains("self.generation"));
    }
    let body = source
        .split("fn release_persistent_data(")
        .nth(1)
        .unwrap()
        .split("\n    }")
        .next()
        .unwrap();
    assert!(body.contains("control_release::release_persistent_with_v1("));
    assert!(body.contains("control_release::ReturningControlCleanupCustodyV1::new(self, mode)"));
    assert!(body.contains("core::mem::forget"));
    assert!(
        !body.contains("self.generation")
            && !body.contains("memory.unmap")
            && !body.contains(".collect()")
    );
}

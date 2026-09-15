use super::*;
use crate::shared_memory::{
    DataCleanupMetadataV1, DataCleanupObservationV1, PristineAbortMemoryFixtureV1,
};

#[derive(Debug, Eq, PartialEq)]
struct OrdinarySnapshot {
    base: Snapshot,
    remaining: Vec<Data>,
    pointer: usize,
    active: Option<DataCleanupObservationV1>,
}

fn ordinary_snapshot(root: &Root) -> OrdinarySnapshot {
    OrdinarySnapshot {
        base: snapshot(root),
        remaining: root.remaining_data.as_slice().iter().map(data).collect(),
        pointer: root.remaining_data.as_slice().as_ptr() as usize,
        active: root
            .active_data
            .as_ref()
            .map(DataCleanupCustodyV1::observation),
    }
}

fn metadata(root: &Root, before: &Snapshot) {
    let after = snapshot(root);
    assert_eq!(after.mode, Mode::Ordinary);
    assert_eq!(after.code_identity, before.code_identity);
    assert_eq!(after.packets, before.packets);
    assert_eq!(after.premises, before.premises);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.slots_pointer, before.slots_pointer);
    assert_eq!(after.persistent, before.persistent);
    assert_eq!(after.storage[..2], before.storage[..2]);
    assert_eq!(after.storage[3], before.storage[3]);
    assert_eq!(root.returned.capacity(), 0);
    assert_eq!(root.persistent_returned.capacity(), 0);
    assert_eq!(root.returned_generation, None);
    assert_eq!(root.persistent_output, PersistentOutputStateV1::Unprepared);
}

fn no_retry(root: &mut Root, memory: &mut PristineAbortMemoryFixtureV1) {
    memory.clear_cleanup_faults();
    let before = ordinary_snapshot(root);
    let native = memory.memory_snapshot();
    assert!(matches!(
        root.release_ordinary_in_place(memory),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert!(root.release_in_place(memory).is_err());
    assert!(root.take_completed().is_err());
    assert!(root.take_persistent_data().is_err());
    assert_eq!(ordinary_snapshot(root), before);
    assert_eq!(memory.memory_snapshot(), native);
}

fn failed(
    root: Root,
    memory: &mut (impl PristineControlReleaseV1 + DispatchDataReleaseV1),
) -> (
    Root,
    Result<Gfx942DispatchBindingErrorV1, Box<dyn std::any::Any + Send>>,
) {
    let mut retained = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        release_ordinary_with_v1(root, memory, |root| {
            assert!(retained.is_none());
            retained = Some(root);
        })
    }));
    let failure = match result {
        Ok(Err(error)) => Ok(error),
        Err(payload) => Err(payload),
        Ok(Ok(())) => panic!("ordinary cleanup unexpectedly succeeded"),
    };
    (retained.expect("ordinary cleanup lost its owner"), failure)
}

fn active_data(root: &Root, before: &Snapshot, index: usize) -> DataCleanupObservationV1 {
    metadata(root, before);
    assert!(root.started && !root.complete);
    assert!(root.active_control.is_none() && root.kernarg.is_none());
    assert!(root.code.as_slice().is_empty() && root.data.is_empty());
    assert_eq!(
        root.remaining_data
            .as_slice()
            .iter()
            .map(data)
            .collect::<Vec<_>>(),
        before.data[index + 1..]
    );
    assert_eq!(
        root.remaining_data.as_slice().as_ptr() as usize,
        before.storage[2].0 + (index + 1) * core::mem::size_of::<DispatchDataAuthorityV1>()
    );
    let active = root
        .active_data
        .as_ref()
        .expect("retained active ordinary data")
        .observation();
    match (&active.metadata, &before.data[index].facts) {
        (DataCleanupMetadataV1::HostDispatch(actual), DataFacts::Host(expected)) => {
            assert_eq!(host_facts(actual), *expected)
        }
        (DataCleanupMetadataV1::DeviceDispatch(actual), DataFacts::Device(expected)) => {
            assert_eq!(actual, expected)
        }
        _ => panic!("ordinary cleanup replaced original dispatch facts"),
    }
    active
}

#[test]
fn ordinary_cleanup_preserves_vacant_generation_policy_without_output_allocation() {
    for history in 0..4 {
        let (mut memory, mut root) = fixture(Mode::Ordinary);
        match history {
            0 => {}
            1 => {
                let epoch = root
                    .generation
                    .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(8))
                    .unwrap();
                root.generation.cancel_epoch(epoch).unwrap();
            }
            2 => {
                assert_eq!(recycle(&mut root.generation), 8);
            }
            3 => {
                root.generation.next_generation = u64::MAX;
            }
            _ => unreachable!(),
        }
        let before = snapshot(&root);
        let native = memory.memory_snapshot();
        root.release_ordinary_in_place(&mut memory).unwrap();
        metadata(&root, &before);
        assert!(root.complete && root.active_control.is_none() && root.active_data.is_none());
        assert!(root.remaining_data.as_slice().is_empty() && root.data.is_empty());
        native.assert_data_prefix(
            &memory,
            &order(&before),
            &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
            before.data.len(),
            None,
        );
        no_retry(&mut root, &mut memory);
    }
}

#[test]
fn ordinary_consuming_wrapper_returns_success_without_retention() {
    let (mut memory, root) = fixture(Mode::Ordinary);
    let before = snapshot(&root);
    let native = memory.memory_snapshot();
    release_ordinary_with_v1(root, &mut memory, |_| {
        panic!("successful ordinary cleanup retained its owner")
    })
    .unwrap();
    native.assert_data_prefix(
        &memory,
        &order(&before),
        &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
        before.data.len(),
        None,
    );
}

#[test]
fn ordinary_cleanup_preserves_nonempty_prepared_metadata_through_late_failure() {
    struct Late {
        memory: Memory,
        panic: bool,
    }
    impl PristineControlReleaseV1 for Late {
        fn release_control(
            &mut self,
            root: &mut ControlCleanupCustodyV1,
        ) -> Result<(), MemorySessionError> {
            self.memory.release_control(root)
        }
    }
    impl DispatchDataReleaseV1 for Late {
        fn release_data(
            &mut self,
            root: &mut DataCleanupCustodyV1,
        ) -> Result<(), MemorySessionError> {
            self.memory
                .fail_cleanup("release_va_reservation", self.panic);
            self.memory.release_data(root)
        }
    }
    for count in [1, 3] {
        for outcome in 0..3 {
            let (mut memory, owner) = if count == 1 {
                preparation::single_persistent_control_fixture_v1()
            } else {
                preparation::three_persistent_control_fixture_v1()
            };
            assert!(!owner.code_identity.is_empty() && !owner.packets.is_empty());
            assert!(
                owner
                    .data_premises
                    .iter()
                    .any(|p| !p.writable_ranges.is_empty())
            );
            let mut root = new_root(owner, Mode::Ordinary);
            let before = snapshot(&root);
            if outcome == 0 {
                root.release_ordinary_in_place(&mut memory).unwrap();
                metadata(&root, &before);
                assert!(root.complete);
            } else {
                let (root, failure) = failed(
                    root,
                    &mut Late {
                        memory,
                        panic: outcome == 2,
                    },
                );
                let active = active_data(&root, &before, 0);
                assert_eq!(active.owner, "Unmapped");
                if outcome == 2 {
                    assert_eq!(
                        failure.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "release_va_reservation"))
                    );
                } else {
                    assert!(matches!(
                        failure,
                        Ok(Gfx942DispatchBindingErrorV1::Memory(
                            MemorySessionError::Injected("release_va_reservation")
                        ))
                    ));
                }
            }
        }
    }
}

#[test]
fn ordinary_consuming_entry_roots_owner_before_validation_or_effects() {
    let source = include_str!("../../queue_dispatch_binding.rs");
    let body = source
        .split("    pub(super) fn release(\n")
        .nth(1)
        .unwrap()
        .split("    pub(super) fn release_non_data_after_recycle(")
        .next()
        .unwrap();
    assert!(body.contains("control_release::release_ordinary_with_v1("));
    assert!(body.contains("control_release::ReturningControlCleanupCustodyV1::new("));
    assert!(body.contains("control_release::ReturningControlModeV1::Ordinary"));
    assert!(body.contains("core::mem::forget"));
    assert!(
        !body.contains("self.require_prepared()")
            && !body.contains("memory.unmap_")
            && !body.contains("for data in")
    );
}

#[test]
fn ordinary_cleanup_roots_owner_before_poison_or_inflight_rejection() {
    for phase in 0..4 {
        let (mut memory, mut root) = fixture(Mode::Ordinary);
        match phase {
            0 => {
                root.generation.poison();
            }
            1 => {
                root.generation
                    .reserve(test_dispatch_queue_v1(), test_completion_roster_v1(8))
                    .unwrap();
            }
            2 => {
                publish(&mut root.generation);
            }
            3 => {
                let (e, c) = publish(&mut root.generation);
                root.generation.complete_epoch(e, c).unwrap();
            }
            _ => unreachable!(),
        }
        let removed = root.data_premises.pop().unwrap();
        let mut before = ordinary_snapshot(&root);
        let native = memory.memory_snapshot();
        let (mut root, error) = failed(root, &mut memory);
        if phase == 0 {
            assert!(matches!(error, Ok(Gfx942DispatchBindingErrorV1::Poisoned)));
        } else {
            assert!(matches!(
                error,
                Ok(Gfx942DispatchBindingErrorV1::ResourcePhase)
            ));
        }
        before.base.started = true;
        assert_eq!(ordinary_snapshot(&root), before);
        assert_eq!(memory.memory_snapshot(), native);
        no_retry(&mut root, &mut memory);
        drop(removed);
    }
}

#[test]
fn ordinary_cleanup_preserves_zero_data_and_mismatched_premise_behavior() {
    for zero in [false, true] {
        let (mut memory, mut root) = fixture(Mode::Ordinary);
        let outside = if zero {
            core::mem::take(&mut root.data)
        } else {
            Vec::new()
        };
        let removed = root.data_premises.pop().unwrap();
        root.return_capacity_override = Some(usize::MAX);
        let before = snapshot(&root);
        let native = memory.memory_snapshot();
        root.release_ordinary_in_place(&mut memory).unwrap();
        assert!(root.complete);
        metadata(&root, &before);
        native.assert_data_prefix(
            &memory,
            &order(&before),
            &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
            before.data.len(),
            None,
        );
        no_retry(&mut root, &mut memory);
        drop((outside, removed));
    }
}

#[test]
fn ordinary_cleanup_retains_controls_and_all_data_on_control_failures() {
    for index in 0..3 {
        for operation in ["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"] {
            for panic in [false, true] {
                let (mut memory, root) = fixture(Mode::Ordinary);
                let before = snapshot(&root);
                let records = memory.control_record_snapshot(order(&before));
                memory.fail_control(index + 1, operation, panic);
                let (mut root, failure) = failed(root, &mut memory);
                if panic {
                    assert_eq!(
                        failure.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", operation))
                    );
                } else {
                    assert!(
                        matches!(failure, Ok(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(actual))) if actual == operation)
                    );
                }
                metadata(&root, &before);
                assert_eq!(root.data.iter().map(data).collect::<Vec<_>>(), before.data);
                assert_eq!(storage(&root.data), before.storage[2]);
                assert!(root.remaining_data.as_slice().is_empty() && root.active_data.is_none());
                assert_eq!(snapshot(&root).code, before.code[index..]);
                let active = root.active_control.as_ref().unwrap().observation();
                assert_eq!(active.identity, order(&before)[index]);
                assert!(active.failed && !root.complete);
                records.assert_after(&memory, index, index + 1);
                no_retry(&mut root, &mut memory);
            }
        }
    }
}

#[test]
fn ordinary_cleanup_retains_complete_owner_at_all_control_currentness_boundaries() {
    for offset in 1usize..=18 {
        for panic in [false, true] {
            let (mut memory, root) = fixture(Mode::Ordinary);
            let before = snapshot(&root);
            let native = memory.memory_snapshot();
            let start = memory.currentness_calls();
            memory.fail_currentness(offset, panic);
            let (mut root, failure) = failed(root, &mut memory);
            if panic {
                assert_eq!(
                    failure.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "currentness"))
                );
            } else {
                assert!(matches!(
                    failure,
                    Ok(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::Injected("currentness")
                    ))
                ));
            }
            assert_eq!(memory.currentness_calls() - start, offset);
            metadata(&root, &before);
            assert_eq!(root.data.iter().map(data).collect::<Vec<_>>(), before.data);
            assert_eq!(storage(&root.data), before.storage[2]);
            assert!(root.active_data.is_none() && root.remaining_data.as_slice().is_empty());
            let index = (offset - 1) / 6;
            let point = (offset - 1) % 6 + 1;
            let active = root.active_control.as_ref().unwrap().observation();
            assert_eq!(active.identity, order(&before)[index]);
            assert_eq!(
                active.owner,
                if point <= 2 {
                    "Mapped"
                } else if point == 6 {
                    "NativeDisposed"
                } else {
                    "Unmapped"
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
            let calls = point.saturating_sub(2).max(usize::from(point >= 2));
            native.assert_control_transition(
                &memory,
                &order(&before),
                index,
                point >= 3,
                calls,
                Some((point >= 3, calls, false, false)),
            );
            no_retry(&mut root, &mut memory);
        }
    }
}

#[test]
fn ordinary_cleanup_retains_active_data_suffix_and_exact_native_prefix() {
    for index in 0usize..5 {
        let operations: &[&'static str] = if index >= 3 {
            &["unmap_gpu", "unmap_cpu", "free", "release_va_reservation"]
        } else {
            &["unmap_gpu", "free", "release_va_reservation"]
        };
        for operation in operations {
            for panic in [false, true] {
                let (mut memory, root) = fixture(Mode::Ordinary);
                let before = snapshot(&root);
                let native = memory.memory_snapshot();
                memory.fail_data(index + 1, operation, panic);
                let (mut root, failure) = failed(root, &mut memory);
                if panic {
                    assert_eq!(
                        failure.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", *operation))
                    );
                } else {
                    assert!(
                        matches!(failure, Ok(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::Injected(actual))) if actual == *operation)
                    );
                }
                let active = active_data(&root, &before, index);
                assert!(active.started && active.failed && !active.complete);
                assert_eq!(
                    active.owner,
                    if *operation == "unmap_gpu" {
                        "Mapped"
                    } else {
                        "Unmapped"
                    }
                );
                native.assert_data_prefix(
                    &memory,
                    &order(&before),
                    &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
                    index,
                    Some(&active),
                );
                no_retry(&mut root, &mut memory);
            }
        }
    }
}

#[test]
fn ordinary_cleanup_retains_data_at_every_closing_currentness_boundary() {
    for index in 0usize..5 {
        let earlier = 18 + index.min(3) * 5 + index.saturating_sub(3) * 6;
        for local in 1..=if index < 3 { 5 } else { 6 } {
            for panic in [false, true] {
                let (mut memory, root) = fixture(Mode::Ordinary);
                let before = snapshot(&root);
                let native = memory.memory_snapshot();
                let calls = memory.currentness_calls();
                memory.fail_currentness(earlier + local, panic);
                let (mut root, failure) = failed(root, &mut memory);
                if panic {
                    assert_eq!(
                        failure.unwrap_err().downcast_ref::<(&str, &str)>(),
                        Some(&("N2 native panic", "currentness"))
                    );
                } else {
                    assert!(matches!(
                        failure,
                        Ok(Gfx942DispatchBindingErrorV1::Memory(
                            MemorySessionError::Injected("currentness")
                        ))
                    ));
                }
                assert_eq!(memory.currentness_calls() - calls, earlier + local);
                let active = active_data(&root, &before, index);
                let terminal = local == if index < 3 { 5 } else { 6 };
                assert_eq!(
                    active.owner,
                    if local <= 2 {
                        "Mapped"
                    } else if terminal {
                        "NativeDisposed"
                    } else {
                        "Unmapped"
                    }
                );
                assert_eq!(active.native_disposed, terminal);
                let expected = if index >= 3 {
                    [local >= 4, local >= 5, local >= 6]
                } else {
                    [false, local >= 4, local >= 5]
                };
                assert_eq!(
                    active.disposal,
                    expected.map(|ran| (ran, ran.then_some(true)))
                );
                assert_eq!(
                    active.unmap,
                    if local == 1 {
                        (false, None, None)
                    } else {
                        (true, Some(true), Some(1))
                    }
                );
                native.assert_data_prefix(
                    &memory,
                    &order(&before),
                    &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
                    index,
                    Some(&active),
                );
                no_retry(&mut root, &mut memory);
            }
        }
    }
}

#[test]
fn ordinary_cleanup_retains_partial_unmap_and_host_commit_failures() {
    for index in 0..5 {
        for (prefix, errno) in [(0, false), (0, true), (1, true), (2, false)] {
            let (mut memory, root) = fixture(Mode::Ordinary);
            let before = snapshot(&root);
            let native = memory.memory_snapshot();
            memory.unmap_data(index + 1, prefix, errno);
            let (mut root, failure) = failed(root, &mut memory);
            if prefix > 1 {
                assert!(
                    matches!(failure, Ok(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::KernelResultMalformed(detail))) if detail == if index >= 3 {
                    "shared UNMAP_MEMORY_FROM_GPU cumulative n_success"
                } else { "device-memory UNMAP_MEMORY_FROM_GPU cumulative n_success" })
                );
            } else if errno {
                assert!(matches!(
                    failure,
                    Ok(Gfx942DispatchBindingErrorV1::Memory(
                        MemorySessionError::Injected("unmap_gpu")
                    ))
                ));
            } else {
                assert!(
                    matches!(failure, Ok(Gfx942DispatchBindingErrorV1::Memory(MemorySessionError::KernelResultMalformed(detail))) if detail == if index >= 3 {
                "shared UNMAP_MEMORY_FROM_GPU full prefix"
            } else { "device-memory UNMAP_MEMORY_FROM_GPU full prefix" })
                );
            }
            let active = active_data(&root, &before, index);
            assert_eq!(active.unmap, (true, Some(!errno), Some(prefix)));
            assert_eq!(active.owner, "Mapped");
            native.assert_data_prefix(
                &memory,
                &order(&before),
                &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
                index,
                Some(&active),
            );
            no_retry(&mut root, &mut memory);
        }
    }
    for index in [3, 4] {
        for release in [false, true] {
            let (mut memory, root) = fixture(Mode::Ordinary);
            let before = snapshot(&root);
            memory.exhaust_data_commit(index + 1, release);
            let native = memory.memory_snapshot();
            let (mut root, failure) = failed(root, &mut memory);
            assert!(matches!(
                failure,
                Ok(Gfx942DispatchBindingErrorV1::Memory(
                    MemorySessionError::Model("queue foundation certificate revision exhausted")
                ))
            ));
            memory
                .memory_snapshot()
                .assert_certificate_revision(u64::MAX);
            let active = active_data(&root, &before, index);
            assert_eq!(
                active.stage,
                if release {
                    Stage::ReleaseCommit
                } else {
                    Stage::UnmapCommit
                }
            );
            assert_eq!(active.native_disposed, release);
            native.assert_data_prefix(
                &memory,
                &order(&before),
                &before.data.iter().map(|d| d.identity).collect::<Vec<_>>(),
                index,
                Some(&active),
            );
            no_retry(&mut root, &mut memory);
        }
    }
}

#[test]
fn ordinary_cleanup_rejects_incomplete_data_callback_and_wrong_modes() {
    let (mut memory, mut root) = fixture(Mode::Ordinary);
    let before = ordinary_snapshot(&root);
    let native = memory.memory_snapshot();
    assert!(matches!(
        root.release_in_place(&mut memory),
        Err(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    assert_eq!(ordinary_snapshot(&root), before);
    assert_eq!(memory.memory_snapshot(), native);

    struct Incomplete(PristineAbortMemoryFixtureV1);
    impl PristineControlReleaseV1 for Incomplete {
        fn release_control(
            &mut self,
            root: &mut ControlCleanupCustodyV1,
        ) -> Result<(), MemorySessionError> {
            self.0.release_control(root)
        }
    }
    impl DispatchDataReleaseV1 for Incomplete {
        fn release_data(&mut self, _: &mut DataCleanupCustodyV1) -> Result<(), MemorySessionError> {
            Ok(())
        }
    }
    let (memory, root) = fixture(Mode::Ordinary);
    let before = snapshot(&root);
    let native = memory.memory_snapshot();
    let mut memory = Incomplete(memory);
    let (mut root, failure) = failed(root, &mut memory);
    assert!(matches!(
        failure,
        Ok(Gfx942DispatchBindingErrorV1::ResourcePhase)
    ));
    let active = active_data(&root, &before, 0);
    assert_eq!(active.owner, "Original");
    assert!(!active.started && !active.complete);
    native.assert_data_prefix(&memory.0, &order(&before), &[], 0, None);
    no_retry(&mut root, &mut memory.0);
    for mode in [
        Mode::AfterRecycle,
        Mode::ReturningDestroy,
        Mode::PersistentBeforePublication,
        Mode::PersistentAfterRecycle,
        Mode::DetachedPersistent {
            expected_generation: 8,
        },
    ] {
        let (mut memory, root) = fixture(mode);
        let before = ordinary_snapshot(&root);
        let native = memory.memory_snapshot();
        let (root, failure) = failed(root, &mut memory);
        assert!(matches!(
            failure,
            Ok(Gfx942DispatchBindingErrorV1::ResourcePhase)
        ));
        assert_eq!(ordinary_snapshot(&root), before);
        assert_eq!(memory.memory_snapshot(), native);
    }
}

#[test]
fn ordinary_mode_cannot_enter_any_returning_wrapper() {
    for wrapper in 0..3 {
        let (mut memory, root) = fixture(Mode::Ordinary);
        let before = ordinary_snapshot(&root);
        let native = memory.memory_snapshot();
        let mut retained = None;
        match wrapper {
            0 => {
                assert!(
                    release_returning_with_v1(root, &mut memory, |r| retained = Some(r)).is_err()
                );
            }
            1 => {
                assert!(
                    release_persistent_with_v1(root, &mut memory, |r| retained = Some(r)).is_err()
                );
            }
            2 => {
                assert!(
                    release_detached_persistent_with_v1(root, &mut memory, |r| retained = Some(r))
                        .is_err()
                );
            }
            _ => unreachable!(),
        }
        assert_eq!(ordinary_snapshot(&retained.unwrap()), before);
        assert_eq!(memory.memory_snapshot(), native);
    }
}

use super::*;

#[test]
fn cleanup_mapping_snapshot_zero_encoding_preserves_every_byte_and_length() {
    for len in [0, 1, 4095, 4096, 4097, 8193] {
        let mut bytes = vec![0; len];
        let zero = MappingBytesV1::capture(&bytes);
        assert_eq!(zero.to_dense(), bytes);
        assert_eq!(zero.len(), len);
        for index in [0, 4095, 4096, len.saturating_sub(1)] {
            if index >= len {
                continue;
            }
            bytes[index] = 0x5a;
            let changed = MappingBytesV1::capture(&bytes);
            assert_eq!(changed.to_dense(), bytes);
            assert_eq!(changed.len(), len);
            assert_ne!(changed, zero);
            bytes[index] = 0;
            assert_eq!(MappingBytesV1::capture(&bytes), zero);
        }
    }
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

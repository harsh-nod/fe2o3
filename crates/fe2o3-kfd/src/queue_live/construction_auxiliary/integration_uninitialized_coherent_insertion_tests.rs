//! Allocation-only cases use the original coherent fixture without a source buffer.

use super::*;

const BYTES: usize = 17;

impl CoherentFixture {
    fn insert_uninitialized(
        &mut self,
        index: Option<usize>,
        requested_bytes: usize,
    ) -> SettledDataInsertionV1 {
        settle_data_insertion_v1(
            &mut CoherentContext {
                base: self.base.context(),
                trace: &mut self.trace,
            },
            CoherentAllocationCustodyV1::new(),
            index.map_or(
                DataInsertionIndexV1::RequiredHole,
                DataInsertionIndexV1::Explicit,
            ),
            requested_bytes,
        )
    }

    fn assert_retry_uninitialized(&mut self) {
        self.assert_retry_using(|this| this.insert_uninitialized(Some(0), BYTES));
    }

    fn assert_no_copy(&self, map_attempted: bool) {
        assert_eq!(self.trace.memory.stages, [1, 0, usize::from(map_attempted)]);
        assert!(self.trace.memory.source.is_none());
    }
}

fn success_prefix() -> Prefix {
    Prefix {
        calls: [5, 1, 1, 1, 1],
        operations: OPERATIONS.to_vec(),
        copied: 0,
        record_phase: Some("GpuAccessibleMutable"),
        pending: None,
    }
}

fn check_native_failure(
    ordinal: usize,
    fault: Fault,
    prefix: Prefix,
    progress: Option<NativeProgress>,
) {
    let mut f = CoherentFixture::new(ordinal);
    let before = f.base.memory().coherent_insertion_snapshot_v1();
    let accounting = f.base.memory().observation();
    let ledger = f.base.context().ledger_snapshot();
    let admitted = prefix.calls[1] != 0;
    let pending = prefix.pending.is_some();
    f.trace.fault = fault;
    let settled = f.insert_uninitialized(Some(2), BYTES);
    assert!(settled.transport);
    assert_eq!(
        f.base.memory().observation().phase,
        SharedMemorySessionPhaseV1::Quarantined
    );
    match (fault, &settled.result) {
        (Fault::Currentness(_, true), Err(p)) => assert_eq!(
            p.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", "currentness"))
        ),
        (Fault::Native(op, true), Err(p)) => assert_eq!(
            p.downcast_ref::<(&str, &str)>(),
            Some(&("N2 native panic", op))
        ),
        (
            Fault::Currentness(_, false),
            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(
                "currentness",
            )))),
        ) => {}
        (
            Fault::Native(op, false),
            Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(actual)))),
        ) => assert_eq!(op, *actual),
        (Fault::Map(n, errno), Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(error)))) => {
            match error {
                MemorySessionError::KernelResultMalformed(
                    "shared MAP_MEMORY_TO_GPU cumulative n_success",
                ) => assert_eq!(n, 2),
                MemorySessionError::KernelResultMalformed(
                    "shared MAP_MEMORY_TO_GPU full prefix",
                ) => assert_eq!((n, errno), (0, false)),
                MemorySessionError::Injected("map_gpu") => assert!(errno && n <= 1),
                _ => panic!("unexpected map failure: {error:?}"),
            }
        }
        _ => panic!("exact uninitialized coherent native failure required"),
    }
    assert_eq!(f.base.context().ledger_snapshot(), ledger);
    assert!(f.trace.before_retake.is_none() && f.trace.before_failure.is_none());
    f.assert_no_copy(progress.is_some());
    let marker = progress.map(|progress| {
        let token = f.trace.memory.allocated.as_ref().unwrap();
        token.assert_id(before.next_id);
        f.base
            .memory()
            .coherent_assert_terminal_v1(token, "Map", false, progress);
        token.identity
    });
    f.base
        .memory()
        .coherent_assert_prefix_for_length_v1(&before, BYTES, None, prefix);
    f.assert_account_delta(&accounting, admitted, pending);
    f.assert_model(u8::from(progress.is_some()));
    f.assert_retry_uninitialized();
    f.finish(true, marker);
}

#[test]
fn coherent_allocation_insertion_currentness_matrix_retains_exact_prefix_and_model() {
    for ordinal in 0..3 {
        for panic in [false, true] {
            for check in 1..=5 {
                let prefix = match check {
                    1 => failed_prefix([1, 0, 0, 0, 0], 0, false, None),
                    2 => failed_prefix(
                        [2, 1, 1, 0, 0],
                        0,
                        false,
                        Some(("CheckAllocation", true, true, None)),
                    ),
                    3 => failed_prefix(
                        [3, 1, 1, 1, 0],
                        0,
                        false,
                        Some(("CheckMapping", true, true, Some(true))),
                    ),
                    _ => failed_prefix([check, 1, 1, 1, usize::from(check == 5)], 0, true, None),
                };
                let progress = (check >= 4).then_some(if check == 5 {
                    (true, Some(true), Some(1))
                } else {
                    (false, None, None)
                });
                check_native_failure(ordinal, Fault::Currentness(check, panic), prefix, progress);
            }
        }
    }
}

#[test]
fn coherent_allocation_insertion_native_allocation_matrix_preserves_pending_owner() {
    for ordinal in 0..3 {
        for panic in [false, true] {
            for operation in ["reserve_va", "alloc", "map_cpu", "prepare_cpu_mapping"] {
                let mut prefix = match operation {
                    "reserve_va" => failed_prefix(
                        [1, 1, 0, 0, 0],
                        0,
                        false,
                        Some(("ReserveVa", false, false, None)),
                    ),
                    "alloc" => failed_prefix(
                        [1, 1, 1, 0, 0],
                        0,
                        false,
                        Some(("Allocate", true, !panic, None)),
                    ),
                    "map_cpu" => failed_prefix(
                        [2, 1, 1, 1, 0],
                        0,
                        false,
                        Some(("MapCpu", true, true, None)),
                    ),
                    _ => failed_prefix(
                        [2, 1, 1, 1, 0],
                        0,
                        false,
                        Some(("PrepareCpuMapping", true, true, Some(false))),
                    ),
                };
                if operation == "map_cpu" {
                    prefix.operations.truncate(1);
                }
                check_native_failure(ordinal, Fault::Native(operation, panic), prefix, None);
            }
        }
    }
}

#[test]
fn coherent_allocation_insertion_map_matrix_retains_cpu_authority() {
    for ordinal in 0..3 {
        for (n, errno) in [(0, false), (0, true), (1, true), (2, false), (2, true)] {
            check_native_failure(
                ordinal,
                Fault::Map(n, errno),
                failed_prefix([4, 1, 1, 1, 1], 0, true, None),
                Some((true, Some(!errno), Some(n))),
            );
        }
        for panic in [false, true] {
            check_native_failure(
                ordinal,
                Fault::Native("map_gpu", panic),
                failed_prefix([4, 1, 1, 1, 1], 0, true, None),
                Some(if panic {
                    (true, None, None)
                } else {
                    (true, Some(false), Some(1))
                }),
            );
        }
    }
}

#[test]
fn coherent_allocation_insertion_projection_matrix_preserves_checkpoint_and_exact_successor() {
    for ordinal in 0..3 {
        for allocation in [true, false] {
            for (index, case) in PrimaryProjectionCaseV1::cases(allocation)
                .into_iter()
                .enumerate()
            {
                let mut f = CoherentFixture::new(ordinal);
                let before = f.base.memory().coherent_insertion_snapshot_v1();
                let accounting = f.base.memory().observation();
                let ledger = f.base.context().ledger_snapshot();
                f.trace.fault = Fault::Projection(allocation, case);
                let settled = f.insert_uninitialized(Some(2), BYTES);
                assert!(settled.transport);
                if case.panic {
                    case.assert_panic(&*settled.result.err().unwrap());
                } else {
                    assert!(matches!(
                        settled.result,
                        Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                            MemorySessionError::Injected("session projection")
                        )))
                    ));
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                assert!(f.trace.before_retake.is_none() && f.trace.before_failure.is_none());
                let mapped = !allocation && index >= 4;
                let mut prefix = failed_prefix(
                    [if mapped { 5 } else { 3 }, 1, 1, 1, usize::from(mapped)],
                    0,
                    true,
                    None,
                );
                if mapped {
                    prefix.record_phase = Some("GpuAccessibleMutable");
                }
                f.base
                    .memory()
                    .coherent_assert_prefix_for_length_v1(&before, BYTES, None, prefix);
                f.assert_no_copy(!allocation);
                let engine = &f.base.scope.primary.completed.as_ref().unwrap().engine;
                f.base
                    .memory()
                    .coherent_assert_projection_v1(&engine.foundation);
                f.assert_model(u8::from(!allocation));
                f.assert_account_delta(&accounting, true, false);
                let marker = f.trace.memory.allocated.as_ref().map(|t| {
                    t.assert_id(before.next_id);
                    t.identity
                });
                f.assert_retry_uninitialized();
                f.finish(true, marker);
            }
        }
    }
}

#[test]
fn coherent_allocation_insertion_success_preserves_exact_uninitialized_order_and_accounts() {
    for ordinal in 0..3 {
        for index in [0, 2, 4] {
            for (requested_bytes, backing_bytes) in [(1, 4096), (17, 4096), (4097, 8192)] {
                let mut f = CoherentFixture::new(ordinal);
                let before = f.base.memory().coherent_insertion_snapshot_v1();
                let accounting = f.base.memory().observation();
                let ledger = f.base.context().ledger_snapshot();
                let loan = f.base.loan_state();
                let settled = f.insert_uninitialized(Some(index), requested_bytes);
                assert!(!settled.transport);
                let data = settled.into_result().unwrap();
                let expected = f.trace.memory.allocated.as_ref().unwrap().mapped();
                assert_eq!(f.trace.before_retake.as_ref(), Some(&expected));
                f.assert_no_copy(true);
                assert_eq!(
                    data.storage_identity(),
                    Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(
                        expected.identity
                    )
                );
                let mut identities = ledger.0;
                identities.insert(index, data.storage_identity());
                assert_eq!(
                    f.base.context().ledger_snapshot(),
                    (identities, ledger.1 + 1, None)
                );
                assert_eq!(f.base.loan_state(), (loan.0, None, loan.2 + 1));
                f.base.memory().coherent_assert_prefix_for_length_v1(
                    &before,
                    requested_bytes,
                    None,
                    success_prefix(),
                );
                f.assert_account_delta_with_backing(&accounting, true, false, backing_bytes);
                f.assert_model_for_length(requested_bytes, 2);
                f.base.data.insert(index, data);
                f.finish(false, None);
            }
        }
    }
}

#[test]
fn coherent_allocation_insertion_completed_owner_survives_retake_and_commit_failure() {
    for ordinal in 0..3 {
        for closing in 0..6 {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            let offset = trace().borrow().calls.len();
            let occurrence = |name| {
                trace()
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&s| s == name)
                    .count()
                    + 1
            };
            let pre_occurrence = occurrence("auxiliary-retake");
            let post_occurrence = occurrence("auxiliary-retake-complete");
            match closing {
                0 => f.base.scope.parent.faults.reclaim_before = Outcome::Error,
                1 => f.base.scope.parent.faults.reclaim_before = Outcome::Panic,
                2 => f.base.scope.parent.faults.reclaim_after = Outcome::Error,
                3 => f.base.scope.parent.faults.reclaim_after = Outcome::Panic,
                4 => f.base.scope.parent.faults.regress_revision = true,
                _ => f.base.trace.commit_panic = true,
            }
            let settled = f.insert_uninitialized(Some(2), BYTES);
            assert!(settled.transport);
            assert_eq!(settled.result.is_err(), matches!(closing, 1 | 3 | 5));
            match &settled.result {
                Err(payload) if closing == 5 => assert_eq!(
                    payload.downcast_ref::<&str>(),
                    Some(&"insertion commit ledger access")
                ),
                Err(payload) => assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(
                        if closing == 1 {
                            "auxiliary-retake"
                        } else {
                            "auxiliary-retake-complete"
                        },
                        if closing == 1 {
                            pre_occurrence
                        } else {
                            post_occurrence
                        }
                    ))
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(name))) => assert_eq!(
                    *name,
                    if closing == 0 {
                        "auxiliary-retake"
                    } else {
                        "auxiliary-retake-complete"
                    }
                ),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Model(name))))
                    if closing == 4 =>
                {
                    assert_eq!(*name, "fixture live foundation reclaim")
                }
                _ => panic!("exact uninitialized coherent closing failure required"),
            }
            for call in ["auxiliary-loan", "auxiliary-retake"] {
                assert_eq!(
                    trace().borrow().calls[offset..]
                        .iter()
                        .filter(|&&s| s == call)
                        .count(),
                    1
                );
            }
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            let expected = f.trace.memory.allocated.as_ref().unwrap().mapped();
            assert_eq!(f.trace.before_retake.as_ref(), Some(&expected));
            assert_eq!(
                f.trace.before_failure.as_ref(),
                Some(&expected),
                "Complete retained until the full ledger commit"
            );
            f.base.memory().coherent_assert_terminal_v1(
                &expected,
                "LiveInsertion",
                true,
                (false, None, None),
            );
            f.assert_no_copy(true);
            f.base.memory().coherent_assert_prefix_for_length_v1(
                &before,
                BYTES,
                None,
                success_prefix(),
            );
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
            f.assert_retry_uninitialized();
            f.finish(true, Some(expected.identity));
        }
    }
}

#[test]
fn coherent_allocation_insertion_required_hole_validation_and_lower_size_rejection() {
    for ordinal in 0..3 {
        for case in 0..6 {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let ledger = f.base.context().ledger_snapshot();
            if case == 2 {
                f.base.trace.reserve = Outcome::Error;
            }
            if case == 3 {
                f.base.trace.reserve = Outcome::Panic;
            }
            let bytes = match case {
                4 => 0,
                5 => usize::MAX,
                _ => BYTES,
            };
            let settled = f.insert_uninitialized(
                match case {
                    0 => None,
                    1 => Some(5),
                    _ => Some(0),
                },
                bytes,
            );
            assert_eq!(settled.transport, case >= 3);
            assert!(!matches!(settled.result, Ok(Ok(_))));
            match &settled.result {
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase,
                ))) => assert_eq!(case, 0),
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::InvalidData { index, detail },
                ))) => {
                    assert_eq!(case, 1);
                    assert_eq!((*index, *detail), (4, "detached insertion ordinal"));
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract("insertion-reserve"))) => {
                    assert_eq!(case, 2)
                }
                Err(payload) => {
                    assert_eq!(case, 3);
                    assert_eq!(
                        payload.downcast_ref::<(&str, usize)>().unwrap().0,
                        "insertion-reserve"
                    );
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::InvalidRequestedSize,
                ))) => assert_eq!(case, 4),
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::SizeOverflow,
                ))) => assert_eq!(case, 5),
                _ => panic!("exact uninitialized coherent rejection required"),
            }
            assert_eq!(f.base.trace.reserve_calls, usize::from(case >= 2));
            assert_eq!(f.base.trace.prepare_calls, usize::from(case >= 4));
            assert_eq!(f.trace.memory.stages, [usize::from(case >= 4), 0, 0]);
            assert!(f.trace.memory.source.is_none());
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), before);
            assert_eq!(f.base.memory().observation(), accounting);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            if case >= 4 {
                let engine = &f.base.scope.primary.completed.as_ref().unwrap().engine;
                f.base.memory().coherent_assert_model_for_length_v1(
                    &engine.foundation,
                    &mut f.trace.memory,
                    bytes,
                    0,
                );
            }
            if settled.transport {
                f.assert_retry_uninitialized();
            }
            f.finish(settled.transport, None);
        }
    }
}

#[test]
fn coherent_allocation_insertion_opening_failure_and_missing_complete_never_commit() {
    for ordinal in 0..3 {
        for case in 0..3 {
            let mut f = CoherentFixture::new(ordinal);
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let ledger = f.base.context().ledger_snapshot();
            let offset = trace().borrow().calls.len();
            let occurrence = trace()
                .borrow()
                .calls
                .iter()
                .filter(|&&s| s == "auxiliary-loan")
                .count()
                + 1;
            if case == 2 {
                f.base.trace.skip_prepare = true;
            } else {
                f.base.scope.parent.faults.loan = if case == 0 {
                    Outcome::Error
                } else {
                    Outcome::Panic
                };
            }
            let settled = f.insert_uninitialized(Some(0), BYTES);
            assert!(settled.transport);
            match &settled.result {
                Err(payload) => {
                    assert_eq!(case, 1);
                    assert_eq!(
                        payload.downcast_ref::<(&str, usize)>(),
                        Some(&("auxiliary-loan", occurrence))
                    );
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Contract("auxiliary-loan"))) => {
                    assert_eq!(case, 0)
                }
                Ok(Err(ComputeAqlQueueSessionErrorV1::Memory(
                    MemorySessionError::InvalidAllocationAuthority,
                ))) => assert_eq!(case, 2),
                _ => panic!("exact opening or absent-Complete failure required"),
            }
            assert_eq!(
                trace().borrow().calls[offset..]
                    .iter()
                    .filter(|&&s| s == "auxiliary-retake")
                    .count(),
                usize::from(case == 2)
            );
            assert!(f.trace.before_retake.is_none() && f.trace.before_failure.is_none());
            assert_eq!(f.trace.memory.stages, [0; 3]);
            assert!(f.trace.memory.source.is_none());
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), before);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            f.assert_retry_uninitialized();
            f.finish(true, None);
        }
    }
}

#[test]
fn coherent_allocation_insertion_admits_sixteenth_then_rejects_full_before_index_or_reservation() {
    for ordinal in 0..3 {
        let mut f = CoherentFixture::new(ordinal);
        for count in 4..16 {
            f.trace = CoherentTrace::default();
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let (mut identities, n, hole) = f.base.context().ledger_snapshot();
            assert_eq!((n, hole), (count, None));
            let settled = f.insert_uninitialized(Some(count), BYTES);
            assert!(!settled.transport);
            let data = settled.into_result().unwrap();
            assert!(matches!(
                data.storage_identity(),
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(_)
            ));
            identities.push(data.storage_identity());
            f.base.data.push(data);
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (identities, count + 1, None)
            );
            f.assert_no_copy(true);
            f.base.memory().coherent_assert_prefix_for_length_v1(
                &before,
                BYTES,
                None,
                success_prefix(),
            );
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
        }
        let before = f.base.memory().coherent_insertion_snapshot_v1();
        let ledger = f.base.context().ledger_snapshot();
        let calls = (f.base.trace.reserve_calls, f.base.trace.prepare_calls);
        f.base.trace.reserve = Outcome::Panic;
        for index in [Some(0), Some(17), None] {
            let settled = f.insert_uninitialized(index, 0);
            assert!(!settled.transport);
            assert!(matches!(
                settled.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::DataLeaseCount {
                        requested: 17,
                        maximum: 16
                    }
                )))
            ));
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), before);
            assert_eq!(f.base.context().ledger_snapshot(), ledger);
            assert_eq!(
                (f.base.trace.reserve_calls, f.base.trace.prepare_calls),
                calls
            );
        }
        f.finish(false, None);
    }
}

#[test]
fn coherent_allocation_insertion_uses_real_release_hole_and_explicit_override() {
    for ordinal in 0..3 {
        for index in [None, Some(1)] {
            let mut f = CoherentFixture::new(ordinal);
            let old = f.base.data[2].storage_identity();
            let mut data = Some(f.base.data.remove(2));
            let mut released = None;
            let (operation, retake) = f
                .base
                .scope
                .parent
                .with_preparation_custody(|memory| {
                    released = Some(memory.insertion_release_device_v1(data.take().unwrap())?);
                    Ok(())
                })
                .unwrap();
            retake.unwrap();
            operation.unwrap();
            f.base.released.push(released.unwrap());
            {
                let mut context = f.base.context();
                let ledger = context.ledger();
                assert_eq!(ledger.identities.remove(2), old);
                *ledger.count -= 1;
                *ledger.next = Some(2);
            }
            let before = f.base.memory().coherent_insertion_snapshot_v1();
            let accounting = f.base.memory().observation();
            let (mut identities, count, hole) = f.base.context().ledger_snapshot();
            assert_eq!((count, hole), (3, Some(2)));
            let settled = f.insert_uninitialized(index, BYTES);
            assert!(!settled.transport);
            let output = settled.into_result().unwrap();
            assert!(matches!(
                output.storage_identity(),
                Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(_)
            ));
            identities.insert(index.unwrap_or(2), output.storage_identity());
            f.base.data.insert(index.unwrap_or(2), output);
            assert_eq!(
                f.base.context().ledger_snapshot(),
                (identities.clone(), 4, None)
            );
            f.assert_no_copy(true);
            f.base.memory().coherent_assert_prefix_for_length_v1(
                &before,
                BYTES,
                None,
                success_prefix(),
            );
            f.assert_account_delta(&accounting, true, false);
            f.assert_model(2);
            let native = f.base.memory().coherent_insertion_snapshot_v1();
            let calls = (f.base.trace.reserve_calls, f.base.trace.prepare_calls);
            let rejected = f.insert_uninitialized(None, BYTES);
            assert!(!rejected.transport);
            assert!(matches!(
                rejected.result,
                Ok(Err(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::ResourcePhase
                )))
            ));
            assert_eq!(f.base.memory().coherent_insertion_snapshot_v1(), native);
            assert_eq!(
                (f.base.trace.reserve_calls, f.base.trace.prepare_calls),
                calls
            );
            assert_eq!(f.base.context().ledger_snapshot(), (identities, 4, None));
            f.finish(false, None);
        }
    }
}

#[test]
fn coherent_allocation_insertion_operation_panic_wins_secondary_retake_error_or_panic() {
    for ordinal in 0..3 {
        for after in [false, true] {
            for closing in [Outcome::Success, Outcome::Error, Outcome::Panic] {
                let mut f = CoherentFixture::new(ordinal);
                let before = f.base.memory().coherent_insertion_snapshot_v1();
                let accounting = f.base.memory().observation();
                let ledger = f.base.context().ledger_snapshot();
                let calls = trace().borrow().calls.len();
                f.trace.fault = Fault::Native("map_gpu", true);
                if after {
                    f.base.scope.parent.faults.reclaim_after = closing;
                } else {
                    f.base.scope.parent.faults.reclaim_before = closing;
                }
                let settled = f.insert_uninitialized(Some(2), BYTES);
                assert!(settled.transport);
                assert_eq!(
                    settled.result.err().unwrap().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "map_gpu"))
                );
                assert_eq!(
                    trace().borrow().calls[calls..]
                        .iter()
                        .filter(|&&c| c == "auxiliary-retake")
                        .count(),
                    1
                );
                assert_eq!(
                    trace().borrow().calls[calls..]
                        .iter()
                        .filter(|&&c| c == "auxiliary-retake-complete")
                        .count(),
                    usize::from(after || matches!(closing, Outcome::Success)),
                    "configured closing hook is reached"
                );
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let token = f.trace.memory.allocated.as_ref().unwrap();
                f.base.memory().coherent_assert_terminal_v1(
                    token,
                    "Map",
                    false,
                    (true, None, None),
                );
                let marker = token.identity;
                f.assert_no_copy(true);
                f.base.memory().coherent_assert_prefix_for_length_v1(
                    &before,
                    BYTES,
                    None,
                    failed_prefix([4, 1, 1, 1, 1], 0, true, None),
                );
                f.assert_account_delta(&accounting, true, false);
                f.assert_model(1);
                f.assert_retry_uninitialized();
                f.finish(true, Some(marker));
            }
        }
    }
}

#[test]
fn coherent_allocation_insertion_retake_failure_precedes_ordinary_operation_error() {
    for ordinal in 0..3 {
        for after in [false, true] {
            for closing in [Outcome::Error, Outcome::Panic] {
                let mut f = CoherentFixture::new(ordinal);
                let before = f.base.memory().coherent_insertion_snapshot_v1();
                let accounting = f.base.memory().observation();
                let ledger = f.base.context().ledger_snapshot();
                let name = if after {
                    "auxiliary-retake-complete"
                } else {
                    "auxiliary-retake"
                };
                let occurrence = trace()
                    .borrow()
                    .calls
                    .iter()
                    .filter(|&&c| c == name)
                    .count()
                    + 1;
                f.trace.fault = Fault::Currentness(5, false);
                if after {
                    f.base.scope.parent.faults.reclaim_after = closing;
                } else {
                    f.base.scope.parent.faults.reclaim_before = closing;
                }
                let settled = f.insert_uninitialized(Some(2), BYTES);
                assert!(settled.transport);
                match settled.result {
                    Err(payload) => {
                        assert_eq!(closing, Outcome::Panic);
                        assert_eq!(
                            payload.downcast_ref::<(&str, usize)>(),
                            Some(&(name, occurrence))
                        );
                    }
                    Ok(Err(ComputeAqlQueueSessionErrorV1::Contract(actual))) => {
                        assert_eq!(closing, Outcome::Error);
                        assert_eq!(actual, name);
                    }
                    _ => panic!("retake must precede the ordinary operation error"),
                }
                assert_eq!(f.base.context().ledger_snapshot(), ledger);
                let token = f.trace.memory.allocated.as_ref().unwrap();
                let marker = token.identity;
                f.base.memory().coherent_assert_terminal_v1(
                    token,
                    "Map",
                    false,
                    (true, Some(true), Some(1)),
                );
                f.assert_no_copy(true);
                f.base.memory().coherent_assert_prefix_for_length_v1(
                    &before,
                    BYTES,
                    None,
                    failed_prefix([5, 1, 1, 1, 1], 0, true, None),
                );
                f.assert_account_delta(&accounting, true, false);
                f.assert_model(1);
                f.assert_retry_uninitialized();
                f.finish(true, Some(marker));
            }
        }
    }
}

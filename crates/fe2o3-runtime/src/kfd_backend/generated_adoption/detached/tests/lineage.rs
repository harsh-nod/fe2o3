#![cfg(test)]

use super::*;

// These are ordinary drop-counted originals and inert receipt fixtures. They
// exercise the production correspondence/container methods, not native detach.
fn with_producer(test: impl FnOnce(&GeneratedShellPlanV1, &issue::GeneratedSubmissionV1)) {
    let (mut backend, plan, roster) = super::super::super::tests::shells_with_roster();
    let producer = issue::GeneratedSubmissionV1 {
        id: 101,
        roster,
        receipt: ReceiptV1::Recycled.into(),
    };
    test(&plan, &producer);
    backend.dispose_generated_shells_v1(&plan);
}

fn copy_metadata(producer: &issue::GeneratedSubmissionV1) -> issue::GeneratedSubmissionV1 {
    issue::GeneratedSubmissionV1 {
        id: producer.id,
        roster: producer.roster.clone(),
        receipt: ReceiptV1::Recycled.into(),
    }
}

#[test]
fn exact_producer_snapshot_roots_original_identity_until_single_consume() {
    with_producer(|plan, producer| {
        let identity = producer.roster.source_identity.singleton_for_test();
        let references = std::sync::Arc::strong_count(identity);
        let drops = Rc::new(Cell::new(0));
        let mut slot = RetainedDetachedV1::empty();
        slot.capture_producer(plan, producer, 31, || {
            Ok::<_, ()>(Original {
                lineage: 31,
                drops: drops.clone(),
            })
        })
        .unwrap();
        assert_eq!(std::sync::Arc::strong_count(identity), references + 1);
        assert!(slot.matches_producer(plan, producer, |owner| (owner.lineage, plan.count)));
        assert!(!slot.is_disposed_or_unentered());
        let calls = Cell::new(0);
        assert_eq!(
            slot.capture_producer(plan, producer, 32, || {
                calls.set(1);
                Err(())
            }),
            Err(CaptureErrorV1::Occupied)
        );
        assert_eq!(calls.get(), 0);
        let original = slot
            .take_producer_checked(plan, producer, |owner| (owner.lineage, plan.count))
            .unwrap();
        assert_eq!(std::sync::Arc::strong_count(identity), references);
        assert!(Rc::ptr_eq(&original.drops, &drops));
        assert!(slot.is_disposed_or_unentered());
        assert!(
            slot.take_producer_checked(plan, producer, |_| (31, plan.count))
                .is_none()
        );
        assert_eq!(drops.get(), 0);
        drop(original);
        assert_eq!(drops.get(), 1);
    });
}

#[test]
fn wrong_returned_generation_or_shape_keeps_the_original() {
    with_producer(|plan, producer| {
        let drops = Rc::new(Cell::new(0));
        let mut slot = RetainedDetachedV1::empty();
        slot.capture_producer(plan, producer, 41, || {
            Ok::<_, ()>(Original {
                lineage: 41,
                drops: drops.clone(),
            })
        })
        .unwrap();
        let original = slot.owner.as_ref().unwrap() as *const Original;
        for facts in [
            (0, plan.count),
            (40, plan.count),
            (42, plan.count),
            (41, 0),
            (41, plan.count + 1),
        ] {
            assert!(!slot.matches_producer(plan, producer, |_| facts));
            assert!(
                slot.take_producer_checked(plan, producer, |_| facts)
                    .is_none()
            );
            assert_eq!(slot.owner.as_ref().unwrap() as *const Original, original);
            assert!(slot.producer.is_some());
            assert_eq!(drops.get(), 0);
        }
        drop(
            slot.take_producer_checked(plan, producer, |owner| (owner.lineage, plan.count))
                .unwrap(),
        );
        assert_eq!(drops.get(), 1);
    });
}

#[test]
fn producer_plan_source_roster_and_submission_substitutions_refuse_before_observation() {
    with_producer(|plan, producer| {
        let drops = Rc::new(Cell::new(0));
        let mut slot = RetainedDetachedV1::empty();
        slot.capture_producer(plan, producer, 51, || {
            Ok::<_, ()>(Original {
                lineage: 51,
                drops: drops.clone(),
            })
        })
        .unwrap();
        for case in 0..13 {
            let mut changed_plan = *plan;
            let mut changed = copy_metadata(producer);
            match case {
                0 => changed.id += 1,
                1 => changed_plan.key += 1,
                2 => changed_plan.binding.context_generation += 1,
                3 => changed_plan.binding.hold += 1,
                4 => changed_plan.members[0].as_mut().unwrap().backend += 1,
                5 => {
                    changed_plan.members[0]
                        .as_mut()
                        .unwrap()
                        .description
                        .ordinal += 1
                }
                6 => changed.roster.source_identity = std::sync::Arc::new(()).into(),
                7 => changed.roster.buffers[0].as_mut().unwrap().ordinal += 1,
                8 => changed.roster.buffers[0].as_mut().unwrap().bytes += 4,
                9 => changed.roster.readback_bytes += 4,
                10 => changed.roster.fixup_count += 1,
                11 => {
                    changed
                        .roster
                        .dispatch_contract_sha256
                        .singleton_mut_for_test()[0] ^= 1
                }
                12 => changed.receipt = NativeReceiptV1::Cohort3(ReceiptV1::Recycled),
                _ => unreachable!(),
            }
            let calls = Cell::new(0);
            assert!(
                slot.take_producer_checked(&changed_plan, &changed, |_| {
                    calls.set(1);
                    (51, plan.count)
                })
                .is_none()
            );
            assert_eq!(calls.get(), 0);
            assert!(slot.is_held());
            assert_eq!(drops.get(), 0);
        }
        drop(
            slot.take_producer_checked(plan, producer, |owner| (owner.lineage, plan.count))
                .unwrap(),
        );
        assert_eq!(drops.get(), 1);
    });
}

#[test]
fn invalid_pre_detach_lineage_never_enters_lower() {
    with_producer(|plan, producer| {
        for case in 0..9 {
            let mut changed_plan = *plan;
            let mut changed = copy_metadata(producer);
            let mut generation = 61;
            match case {
                0 => generation = 0,
                1 => changed.id = 0,
                2 => changed.receipt = ReceiptV1::Ready.into(),
                3 => changed_plan.count = 0,
                4 => changed_plan.count = changed_plan.members.len() + 1,
                5 => {
                    changed_plan.members[0]
                        .as_mut()
                        .unwrap()
                        .description
                        .ordinal = 1
                }
                6 => changed.roster.buffers[0].as_mut().unwrap().ordinal = 1,
                7 => changed_plan.members[plan.count] = plan.members[0],
                8 => changed.roster.buffers[plan.count] = producer.roster.buffers[0],
                _ => unreachable!(),
            }
            let calls = Cell::new(0);
            let mut slot = RetainedDetachedV1::<Original>::empty();
            assert_eq!(
                slot.capture_producer(&changed_plan, &changed, generation, || {
                    calls.set(1);
                    Err(())
                }),
                Err(CaptureErrorV1::Lineage)
            );
            assert_eq!(calls.get(), 0);
            assert!(slot.is_disposed_or_unentered());
            assert!(slot.producer.is_none());
        }
    });
}

#[test]
fn lower_refusal_and_unwind_retain_producer_identity_and_forbid_retry() {
    with_producer(|plan, producer| {
        for unwind in [false, true] {
            let mut slot = RetainedDetachedV1::<Original>::empty();
            let result = catch_unwind(AssertUnwindSafe(|| {
                slot.capture_producer(plan, producer, 71, || {
                    if unwind {
                        panic!("injected bound detach unwind");
                    }
                    Err("injected bound detach refusal")
                })
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(
                    result.unwrap(),
                    Err(CaptureErrorV1::Lower("injected bound detach refusal"))
                );
            }
            assert_eq!(slot.phase, DetachedPhaseV1::InLower);
            assert!(slot.producer.is_some());
            assert!(!slot.is_disposed_or_unentered());
            let calls = Cell::new(0);
            assert_eq!(
                slot.capture_producer(plan, producer, 71, || {
                    calls.set(1);
                    Err(())
                }),
                Err(CaptureErrorV1::Occupied)
            );
            assert_eq!(calls.get(), 0);
        }
    });
}

#[test]
fn returned_fact_check_panic_keeps_both_original_and_producer() {
    with_producer(|plan, producer| {
        let drops = Rc::new(Cell::new(0));
        let mut slot = RetainedDetachedV1::empty();
        slot.capture_producer(plan, producer, 81, || {
            Ok::<_, ()>(Original {
                lineage: 81,
                drops: drops.clone(),
            })
        })
        .unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| {
            slot.take_producer_checked(plan, producer, |_| {
                panic!("injected bound observation unwind")
            })
        }));
        assert!(result.is_err());
        assert!(slot.is_held());
        assert!(slot.producer.is_some());
        assert_eq!(drops.get(), 0);
        drop(
            slot.take_producer_checked(plan, producer, |owner| (owner.lineage, plan.count))
                .unwrap(),
        );
        assert_eq!(drops.get(), 1);
    });
}

#[test]
fn rejected_retirement_is_preserved_but_never_rebranded_completed() {
    with_producer(|plan, producer| {
        let mut rejected = copy_metadata(producer);
        rejected.receipt = ReceiptV1::RejectedUnpublished {
            prior: RetirementV1::Recycled,
            error: fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                "injected retained rejection",
            ),
        }
        .into();
        let mut slot = RetainedDetachedV1::empty();
        slot.capture_producer(plan, &rejected, 91, || Ok::<_, ()>(91u64))
            .unwrap();
        assert!(
            slot.take_producer_checked(plan, producer, |generation| (*generation, plan.count))
                .is_none()
        );
        assert!(slot.matches_producer(plan, &rejected, |generation| (*generation, plan.count)));
        assert_eq!(
            slot.take_producer_checked(plan, &rejected, |generation| (*generation, plan.count)),
            Some(91)
        );
        assert!(rejected.receipt.retirement().is_none());
        assert_eq!(
            rejected.receipt.rejected_retirement(),
            Some(RetirementV1::Recycled)
        );
    });
}

#[test]
fn substituted_source_refuses_actual_backend_entry_without_touching_originals() {
    let (mut backend, plan, mut roster) = super::super::super::tests::shells_with_roster();
    roster.source_identity = std::sync::Arc::new(()).into();
    let mut native = metadata_native(PhaseV1::Adopted);
    native.submission = Some(issue::GeneratedSubmissionV1 {
        id: 101,
        roster,
        receipt: ReceiptV1::Recycled.into(),
    });
    backend.generated_shells.get_mut(&plan.key).unwrap().native = Some(native);
    backend.generated_submissions.insert(101, plan.key);
    assert!(!backend.generated_submission_owner_matches_v1(101, &plan));
    assert!(matches!(
        backend.retain_generated_completed_data_v1(&plan, 101),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(matches!(
        backend.retire_generated_data_v1(&plan),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    let native = backend.generated_shells[&plan.key].native.as_ref().unwrap();
    assert_eq!(native.phase, PhaseV1::Adopted);
    assert!(native.detached.is_disposed_or_unentered());
    assert_eq!(native.submission.as_ref().unwrap().id, 101);
    assert_eq!(backend.generated_submissions.get(&101), Some(&plan.key));
    assert!(backend.queue.is_none());
    assert!(!backend.terminal);
    // Only handle-free metadata is removed; no lower DATA or native lane exists.
    backend.generated_shells.get_mut(&plan.key).unwrap().native = None;
    backend.generated_submissions.clear();
    backend.dispose_generated_shells_v1(&plan);
}

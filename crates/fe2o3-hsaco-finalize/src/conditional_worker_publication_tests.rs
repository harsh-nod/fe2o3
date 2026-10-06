use super::super::tests::{Scratch, attempt, inputs, mutations};
use super::*;
use fe2o3_artifact_transaction::{BuildInvocation, BuildSession, begin_build_attempt};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn original_publication_terminal_exact_one_short_and_partial_failure_accounting() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    const OUTSIDE: usize = MAX_INERT_REFINED_FORWARDING_STORAGE_V1 + 1;
    let work = Budget::STORAGE_WINDOW_WORK_V1 + ENTRY_WORK + 5;
    let peak = OUTSIDE + 7 + Budget::STORAGE_WINDOW_SCRATCH_V1 + FRAME + 19;
    for (work_limit, storage_limit, mode) in [
        (work, peak, 0),
        (work - 1, peak, 1),
        (work, peak - 1, 2),
        (work, peak, 3),
        (work, peak, 4),
    ] {
        let mut owned = Owned::new(Work::new(work_limit), storage_limit);
        owned.with_budget(|b| {
            b.reserve_storage(OUTSIDE).unwrap();
            let identity = b.storage_account_identity_v1();
            let ledger = b.work_ledger_identity_v1();
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                original_terminal(b, 7, |b| {
                    terminal_using(b, 7, true, |b| {
                        b.charge_work(5)?;
                        b.reserve_storage(19)?;
                        if mode == 3 {
                            return Err(Error::Mismatch("component partial refusal"));
                        }
                        if mode == 4 {
                            panic!("component partial unwind");
                        }
                        Ok((23, 19))
                    })
                    .map_err(|e| e.0)
                })
            }));
            if mode == 0 {
                let (owner, charge) = outcome.unwrap().unwrap();
                assert_eq!(owner, 23);
                assert_eq!(charge.retained_storage(), 19);
                assert_eq!(b.storage(), OUTSIDE);
                assert_eq!((b.work(), b.peak_storage()), (work, peak));
            } else {
                assert!(matches!(outcome, Ok(Err(_)) | Err(_)));
                assert!(b.storage() > OUTSIDE);
                if mode >= 3 {
                    assert_eq!(b.storage(), peak);
                }
            }
            assert_eq!(b.storage_account_identity_v1(), identity);
            assert_eq!(b.work_ledger_identity_v1(), ledger);
            assert_eq!(b.storage_limit(), storage_limit);
        });
    }
    let mut owned = Owned::new(Work::new(work), peak);
    owned.with_budget(|b| {
        b.reserve_storage(OUTSIDE).unwrap();
        assert!(terminal::<()>(b, 7, |_| panic!("legacy cap bypass")).is_err());
        assert!(original_terminal::<()>(b, OUTSIDE + 1, |_| panic!("unpaid input")).is_err());
        assert!(original_terminal::<()>(b, usize::MAX, |_| panic!("overflow")).is_err());
        assert_eq!(b.storage(), OUTSIDE);
    });
}

#[test]
fn conditional_plan_preserves_storage_coordinates_without_native_domain_aliases() {
    let input = inputs();
    let intent = derive_plan(inputs());
    let native = super::super::derive_plan(inputs());
    assert_eq!(derive_plan(inputs()), intent);
    assert_eq!(intent.plan.attempt(), input.attempt);
    assert_eq!(intent.plan.scope().package(), input.package);
    assert_eq!(intent.plan.linked_output().as_bytes(), input.raw.sha256());
    assert_eq!(
        intent.plan.finalized_output().as_bytes(),
        input.output.sha256()
    );
    assert_ne!(intent.plan.request(), native.durable_plan().request());
    assert_ne!(intent.plan_identity(), native.plan_identity().as_bytes());
    assert_ne!(intent.identity(), native.identity().as_bytes());
    assert!(!intent.grants_publication_authority());
    assert!(!intent.grants_load_authority());
    assert!(!intent.grants_launch_authority());
}

#[test]
fn all_conditional_plan_axes_bind_request_plan_and_intent() {
    let original = derive_plan(inputs());
    for (index, mutate) in mutations().iter().enumerate() {
        let mut changed = inputs();
        mutate(&mut changed);
        let changed = derive_plan(changed);
        assert_ne!(
            changed.plan.request(),
            original.plan.request(),
            "axis {index}"
        );
        assert_ne!(
            changed.plan_identity(),
            original.plan_identity(),
            "axis {index}"
        );
        assert_ne!(changed.identity(), original.identity(), "axis {index}");
    }
}

#[test]
fn conditional_domains_are_pairwise_distinct_and_separate_from_native() {
    let conditional = [
        DOMAINS.context,
        DOMAINS.request,
        DOMAINS.plan,
        DOMAINS.intent,
        DOMAINS.kernel,
        DOMAINS.target,
        DOMAINS.worker,
        DOMAINS.response,
        DOMAINS.finalization,
        DOMAINS.publication,
    ];
    let native = [
        CONTEXT_DOMAIN,
        REQUEST_DOMAIN,
        PLAN_DOMAIN,
        INTENT_DOMAIN,
        KERNEL_DOMAIN,
        TARGET_DOMAIN,
        WORKER_DOMAIN,
        RESPONSE_DOMAIN,
        FINALIZATION_DOMAIN,
        PUBLICATION_DOMAIN,
    ];
    for (index, domain) in conditional.iter().enumerate() {
        for other in conditional[..index].iter().chain(native.iter()) {
            assert_ne!(domain, other);
            assert_ne!(
                hash_parts(domain, &[&[7; 32]]),
                hash_parts(other, &[&[7; 32]])
            );
        }
    }
}

#[test]
fn conditional_preparation_requires_fresh_custody() {
    assert!(require_custody(Custody::ConsumedPublication, Custody::ConsumedPublication).is_ok());
    assert!(require_custody(Custody::RecoveredTranscript, Custody::RecoveredTranscript).is_ok());
    assert!(require_custody(Custody::RecoveredTranscript, Custody::ConsumedPublication).is_err());
    assert!(require_custody(Custody::ConsumedPublication, Custody::RecoveredTranscript).is_err());
}

#[test]
fn terminal_success_returns_only_exact_unreserved_delta() {
    let mut work = Work::new(ENTRY_WORK + 5);
    let mut b = Budget::new(&mut work, 7 + FRAME + 19);
    b.reserve_storage(7).unwrap();
    let ledger = b.work_ledger_identity_v1();
    let (owner, storage) = terminal(&mut b, 7, |b| {
        b.charge_work(5)?;
        b.reserve_storage(19)?;
        Ok((23, 19))
    })
    .unwrap();
    assert_eq!(owner, 23);
    assert_eq!(storage.retained_storage(), 19);
    assert_eq!(b.storage(), 7);
    assert_eq!(b.work(), ENTRY_WORK + 5);
    assert!(b.work_ledger_identity_v1() == ledger);
    b.reserve_storage(storage.retained_storage()).unwrap();
    assert_eq!(b.storage(), 26);
}

#[test]
fn terminal_refusal_and_unwind_never_refund_partial_reservations() {
    for unwind in [false, true] {
        let mut work = Work::new(ENTRY_WORK + 5);
        let mut b = Budget::new(&mut work, 7 + FRAME + 19);
        b.reserve_storage(7).unwrap();
        let ledger = b.work_ledger_identity_v1();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            terminal::<()>(&mut b, 7, |b| {
                b.charge_work(5)?;
                b.reserve_storage(19)?;
                if unwind {
                    panic!("terminal recovery unwind");
                }
                Err(Error::Resource(Resource::Accounting))
            })
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let error = outcome.unwrap().unwrap_err();
            assert!(std::error::Error::source(&error).is_none());
        }
        assert_eq!(b.storage(), 7 + FRAME + 19);
        assert_eq!(b.work(), ENTRY_WORK + 5);
        assert!(b.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn terminal_refuses_hidden_or_missing_output_charges_without_refunding() {
    for actual in [18, 20] {
        let mut work = Work::new(ENTRY_WORK);
        let mut b = Budget::new(&mut work, 7 + FRAME + 20);
        b.reserve_storage(7).unwrap();
        let error = terminal(&mut b, 7, |b| {
            b.reserve_storage(actual)?;
            Ok(((), 19))
        })
        .unwrap_err();
        assert!(matches!(error.0, Error::Resource(Resource::Accounting)));
        assert_eq!(b.storage(), 7 + FRAME + actual);
    }
}

#[test]
fn terminal_checks_input_floor_cap_and_exact_entry_limits_before_callback() {
    for (quota, limit, input, floor) in [
        (ENTRY_WORK - 1, 7 + FRAME, 7, 7),
        (ENTRY_WORK, 7 + FRAME - 1, 7, 7),
        (ENTRY_WORK, 7 + FRAME, 6, 7),
        (
            ENTRY_WORK,
            MAX_INERT_REFINED_FORWARDING_STORAGE_V1 + 1,
            7,
            7,
        ),
    ] {
        let mut work = Work::new(quota);
        let mut b = Budget::new(&mut work, limit);
        b.reserve_storage(input).unwrap();
        assert!(terminal::<()>(&mut b, floor, |_| panic!("must refuse before callback")).is_err());
        assert_eq!(b.storage(), input);
    }
}

#[test]
fn terminal_refuses_a_replaced_work_ledger() {
    let mut original = Work::new(ENTRY_WORK);
    let mut replacement = Work::new(ENTRY_WORK);
    let mut b = Budget::new(&mut original, FRAME);
    let error = terminal(&mut b, 0, |b| {
        *b = Budget::new(&mut replacement, FRAME);
        b.reserve_storage(FRAME)?;
        Ok(((), 0))
    })
    .unwrap_err();
    assert!(matches!(error.0, Error::Resource(Resource::Accounting)));
    assert_eq!(b.storage(), FRAME);
}

#[test]
fn terminal_transfer_overflow_preserves_reservations() {
    let mut work = Work::new(ENTRY_WORK);
    let mut b = Budget::new(&mut work, FRAME);
    let error = terminal(&mut b, 0, |_| Ok(((), usize::MAX))).unwrap_err();
    assert!(matches!(error.0, Error::Resource(Resource::Arithmetic)));
    assert_eq!(b.storage(), FRAME);
}

#[test]
fn terminal_callback_resource_denials_keep_the_entry_frame() {
    for deny_work in [false, true] {
        let mut work = Work::new(ENTRY_WORK);
        let mut b = Budget::new(&mut work, 7 + FRAME);
        b.reserve_storage(7).unwrap();
        let error = terminal::<()>(&mut b, 7, |b| {
            if deny_work {
                b.charge_work(1)?;
            } else {
                b.reserve_storage(1)?;
            }
            panic!("resource denial must stop callback");
        })
        .unwrap_err();
        assert!(matches!(error.0, Error::Resource(_)));
        assert!(std::error::Error::source(&error).is_none());
        assert_eq!(b.storage(), 7 + FRAME);
        assert_eq!(b.work(), ENTRY_WORK);
        if deny_work {
            assert_eq!(b.failed_work(), Some(ENTRY_WORK + 1));
            assert_eq!(b.failed_storage(), None);
        } else {
            assert_eq!(b.failed_work(), None);
            assert_eq!(b.failed_storage(), Some(7 + FRAME + 1));
        }
    }
}

fn policy() -> ConditionalWorkerRecoveryPolicyV5<'static> {
    ConditionalWorkerRecoveryPolicyV5 {
        roots: &[],
        history_limits: fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1 {
            refinement: Default::default(),
            forwarding: Default::default(),
        },
        target: fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
    }
}

#[test]
fn opaque_journal_commit_never_promotes_malformed_or_other_family_source_to_v5() {
    // These are deliberately inert bytes, not a fabricated source/proof owner.
    for magic in [b"F2O3IHV3", b"F2O3IHV4", b"F2O3IHV5"] {
        let scratch = Scratch::new();
        let producer =
            ProducerIdentity::from_codegen("conditional_restart_negative", None).unwrap();
        let attempt = begin_build_attempt(
            &scratch.0,
            &producer,
            BuildInvocation::from_bytes([3; 32]),
            BuildSession::from_bytes([2; 16]),
        )
        .unwrap();
        let output = b"inert conditional artifact".to_vec();
        let mut input = inputs();
        input.package = producer_package_identity_v1(&producer);
        input.attempt = attempt;
        input.output = ContentIdentityV1::calculate(&output);
        let plan = derive_plan(input).durable_plan();
        let mut outer = vec![0; 256];
        outer[..8].copy_from_slice(magic);
        let attachments = WorkerV3FinalizerReplayAttachmentsV1::new(
            outer,
            Vec::new(),
            b"invalid V5 transcript".to_vec(),
        )
        .unwrap();
        let stored = persist_worker_v3_publication_intent_v1(
            &scratch.0,
            &producer,
            attempt,
            plan,
            attachments,
            output,
        )
        .unwrap();
        check_record_inputs(&producer, attempt, &stored).unwrap();
        let record = stored.record();
        assert_eq!(record.plan(), plan);
        assert!(!stored.grants_publication_authority());
        drop(stored);
        let mut work = Work::new(1_000_000_000);
        let mut b = Budget::new(&mut work, MAX_INERT_REFINED_FORWARDING_STORAGE_V1);
        b.reserve_storage(19).unwrap();
        let error = recover_conditional_worker_hsaco_publication_v5(
            &scratch.0,
            &producer,
            attempt,
            policy(),
            &mut b,
        )
        .err()
        .unwrap();
        assert!(matches!(
            error.0,
            Error::Stage {
                phase: "V5 decode" | "V5 decode quote",
                ..
            }
        ));
        assert!(std::error::Error::source(&error).is_none());
        assert_eq!(b.storage(), 19 + FRAME + 256 + METADATA);
        assert!(b.work() >= ENTRY_WORK);
        let stored =
            recover_worker_v3_publication_intent_v1(&scratch.0, &producer, attempt).unwrap();
        assert_eq!(
            stored.record(),
            record,
            "failed source admission must not erase journal commit"
        );
    }
}

#[test]
fn unavailable_journal_failure_keeps_entry_reservations() {
    let scratch = Scratch::new();
    let producer = ProducerIdentity::from_codegen("missing_conditional_restart", None).unwrap();
    let mut work = Work::new(ENTRY_WORK);
    let mut b = Budget::new(&mut work, 3 + FRAME);
    b.reserve_storage(3).unwrap();
    let error = recover_conditional_worker_hsaco_publication_v5(
        &scratch.0,
        &producer,
        attempt(1, 2, 3),
        policy(),
        &mut b,
    )
    .err()
    .unwrap();
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(b.storage(), 3 + FRAME);
    assert_eq!(b.work(), ENTRY_WORK);
}

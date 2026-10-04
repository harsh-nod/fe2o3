use super::super::super::history::PreparedRefinedForwardingHistoryClaimsV1 as Claims;
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::{
    RefinedForwardingDescriptorErrorV1, fixtures,
};
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryErrorV1 as HError,
    CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    check_canonical_refined_forwarding_history_v1,
};

fn with_claims(
    erased: bool,
    profile: Profile,
    next: impl FnOnce(&mut PreparedLoopUnrollNativeOutputV1, &Claims, &mut Budget<'_>),
) {
    with_prepared(erased, profile, Some(3), |native, budget| {
        let floor = budget.storage();
        let claims =
            Claims::prepare_source_v1(native.owner.source(), &native.prefix_execution, budget)
                .unwrap();
        assert_eq!(budget.storage(), floor);
        let retained = claims.retained_storage();
        budget.reserve_storage(retained).unwrap();
        next(native, &claims, budget);
        drop(claims);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
#[test]
fn loop_unroll_native_history_replays_real_f_and_never_relabels_u_as_f() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_claims(erased, profile, |native, claims, budget| {
                let floor = budget.storage();
                let fixed = size_of::<Inputs<'_>>() + size_of::<FinalSource<'_>>();
                budget.reserve_storage(fixed).unwrap();
                {
                    let inputs =
                        claims.source_inputs_v1(native.owner.source(), &native.prefix_execution);
                    assert!(std::ptr::eq(inputs.output, native.forwarding_output()));
                    assert!(!std::ptr::eq(inputs.output, native.output()));
                    assert_ne!(
                        inputs.output.canonical().identity(),
                        native.output().canonical().identity()
                    );
                    let checked =
                        check_canonical_refined_forwarding_history_v1(inputs, budget).unwrap();
                    let storage = checked.storage().retained_storage();
                    budget.reserve_storage(storage).unwrap();
                    assert!(std::ptr::eq(checked.output(), native.forwarding_output()));
                    drop(checked);
                    budget.release_storage(storage).unwrap();
                    assert!(matches!(
                        check_canonical_refined_forwarding_history_v1(
                            Inputs {
                                output: native.output(),
                                ..inputs
                            },
                            budget
                        ),
                        Err(HError::Forwarding(_))
                    ));
                }
                budget.release_storage(fixed).unwrap();
                claims
                    .check_source_v1(native.owner.source(), &native.prefix_execution, budget)
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn loop_unroll_native_original_p7_swap_requires_paid_foreign_backing_and_refuses() {
    for erased in [false, true] {
        let limit =
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT).unwrap();
        let mut first_work = Work::new(limit);
        let mut second_work = Work::new(limit);
        let mut first_budget = Budget::new(
            &mut first_work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let mut second_budget = Budget::new(
            &mut second_work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        let budget = &mut first_budget;
        let other = &mut second_budget;
        let first_ledger = budget.work_ledger_identity_v1();
        let second_ledger = other.work_ledger_identity_v1();
        assert!(first_ledger != second_ledger);
        let mut first_fixture = prepare_pair_fixture(erased, Profile::Gfx942, budget);
        let mut second_fixture = prepare_pair_fixture(erased, Profile::Gfx950, other);
        let first = &mut first_fixture.native;
        let second = &mut second_fixture.native;
        let first_floor = budget.storage();
        let second_floor = other.storage();
        let claims =
            Claims::prepare_source_v1(first.owner.source(), &first.prefix_execution, budget)
                .unwrap();
        assert_eq!(budget.storage(), first_floor);
        let claims_storage = claims.retained_storage();
        budget.reserve_storage(claims_storage).unwrap();
        let other_claims =
            Claims::prepare_source_v1(second.owner.source(), &second.prefix_execution, other)
                .unwrap();
        assert_eq!(other.storage(), second_floor);
        let other_claims_storage = other_claims.retained_storage();
        other.reserve_storage(other_claims_storage).unwrap();
        let floor = budget.storage();
        let other_floor = other.storage();
        let ledger = budget.work_ledger_identity_v1();
        let other_ledger = other.work_ledger_identity_v1();
        assert_ne!(
            first.prefix_execution.canonical_bytes(),
            second.prefix_execution.canonical_bytes()
        );
        let incoming = second.prefix_execution.retained_storage();
        let other_incoming = first.prefix_execution.retained_storage();
        budget.reserve_storage(incoming).unwrap();
        other.reserve_storage(other_incoming).unwrap();
        std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
        for result in [
            claims.check_source_v1(first.owner.source(), &first.prefix_execution, budget),
            other_claims.check_source_v1(second.owner.source(), &second.prefix_execution, other),
        ] {
            assert!(matches!(
                result,
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Execution("Policy7 complete execution transcript")
                ))
            ));
        }
        std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
        other.release_storage(other_incoming).unwrap();
        budget.release_storage(incoming).unwrap();
        claims
            .check_source_v1(first.owner.source(), &first.prefix_execution, budget)
            .unwrap();
        other_claims
            .check_source_v1(second.owner.source(), &second.prefix_execution, other)
            .unwrap();
        assert_eq!((budget.storage(), other.storage()), (floor, other_floor));
        assert!(
            budget.work_ledger_identity_v1() == ledger
                && other.work_ledger_identity_v1() == other_ledger
        );
        drop(other_claims);
        other.release_storage(other_claims_storage).unwrap();
        assert_eq!(other.storage(), second_floor);
        release_pair_fixture(second_fixture, other);
        drop(claims);
        budget.release_storage(claims_storage).unwrap();
        assert_eq!(budget.storage(), first_floor);
        release_pair_fixture(first_fixture, budget);
        assert!(budget.work_ledger_identity_v1() == first_ledger);
        assert!(other.work_ledger_identity_v1() == second_ledger);
    }
}
#[test]
fn loop_unroll_native_descriptor_uses_actual_u_reports_and_original_source_roots() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, budget| {
                let floor = budget.storage();
                let owner = native.owner.descriptor();
                let roots = fixtures::typed_roots(owner.final_f());
                assert!(std::ptr::eq(owner.output(), native.output()));
                assert!(!std::ptr::eq(owner.output(), owner.final_f().output()));
                validate_unrolled_descriptor_evidence_v1(owner, &roots, profile, budget).unwrap();
                crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::loop_unroll_v1::tests::exercise_unrolled_report_omission_v1(owner, &roots, profile, budget);
                crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::loop_unroll_v1::tests::exercise_unrolled_first_header_denial_v1(owner, &roots, profile, floor);
                assert_eq!(budget.storage(), floor);
                assert!(std::ptr::eq(
                    native.forwarding_output(),
                    owner.final_f().output()
                ));
            });
        }
    }
}

#[test]
fn loop_unroll_native_pair_rejects_changed_limits_and_all_six_incomplete_rosters() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, budget| {
                let floor = budget.storage();
                let tail = tail(&native.owner);
                let mut limits = native.limits();
                limits.max_iterations -= 1;
                assert!(matches!(
                    tail.replay(native.forwarding_output(), limits, budget),
                    Err(fe2o3_kernel_opt::OwnedLoopUnrollErrorV1::LimitsMismatch)
                ));
                let header =
                    size_of::<fe2o3_kernel_analysis::CanonicalKirLoopUnrollOriginsV1<'_>>() * 2;
                budget.reserve_storage(header).unwrap();
                {
                    let rows = tail.origins();
                    assert!(
                        !rows.blocks.is_empty()
                            && !rows.definitions.is_empty()
                            && !rows.operations.is_empty()
                            && !rows.terminators.is_empty()
                            && !rows.edges.is_empty()
                            && !rows.arguments.is_empty()
                    );
                    for kind in 0..6 {
                        let mut bad = rows;
                        match kind {
                            0 => bad.blocks = &rows.blocks[1..],
                            1 => bad.definitions = &rows.definitions[1..],
                            2 => bad.operations = &rows.operations[1..],
                            3 => bad.terminators = &rows.terminators[1..],
                            4 => bad.edges = &rows.edges[1..],
                            5 => bad.arguments = &rows.arguments[1..],
                            _ => unreachable!(),
                        }
                        assert!(matches!(
                            fe2o3_kernel_analysis::check_canonical_kir_loop_unroll_pair_v1(
                                native.forwarding_output(),
                                native.output(),
                                bad,
                                native.limits(),
                                budget
                            ),
                            Err(
                                fe2o3_kernel_analysis::CanonicalKirLoopUnrollErrorV1::Mismatch(
                                    "complete ordered origin"
                                )
                            )
                        ));
                    }
                }
                budget.release_storage(header).unwrap();
                native.verify_equivalence(budget).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn loop_unroll_native_descriptor_rejects_all_typed_source_launch_abi_substitutions() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, budget| {
                let floor = budget.storage();
                for case in 0..8 {
                    let owner = native.owner.descriptor();
                    let mut roots = fixtures::typed_roots(owner.final_f());
                    let expected = fixtures::hostile(&mut roots, case);
                    match validate_unrolled_descriptor_evidence_v1(owner, &roots, profile, budget) {
                    Err(LoopUnrollDescriptorErrorV1::Descriptor(e)) => match *e {
                        crate::compiler_descriptor::CompilerDescriptorError::ProductionDescriptorMismatch(actual) => assert_eq!(actual, expected),
                        other => panic!("exact U descriptor source refusal: {other:?}"),
                    },
                    other => panic!("typed U descriptor refusal: {other:?}"),
                }
                    assert_eq!(budget.storage(), floor);
                }
            });
        }
    }
}
#[test]
fn loop_unroll_native_descriptor_rejects_wrong_profile_at_original_coordinate_join() {
    for erased in [false, true] {
        for profile in PROFILES {
            with_prepared(erased, profile, Some(3), |native, budget| {
                let floor = budget.storage();
                let owner = native.owner.descriptor();
                let roots = fixtures::typed_roots(owner.final_f());
                let wrong = if profile == Profile::Gfx942 {
                    Profile::Gfx950
                } else {
                    Profile::Gfx942
                };
                assert!(
                    matches!(validate_unrolled_descriptor_evidence_v1(owner, &roots, wrong, budget),
                Err(LoopUnrollDescriptorErrorV1::Descriptor(e)) if matches!(*e, crate::compiler_descriptor::CompilerDescriptorError::CheckedOutputTarget(_)))
                );
                validate_unrolled_descriptor_evidence_v1(owner, &roots, profile, budget).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}
#[test]
fn loop_unroll_native_new_wrapper_header_accounts_complete_original_custody_once() {
    let prepared = size_of::<PreparedLoopUnrollNativeOutputV1>();
    let claims = size_of::<Claims>();
    let complete = size_of::<LoopUnrollNativeProductionCompilationV1>();
    let wrapper = complete
        .checked_sub(prepared)
        .unwrap()
        .checked_sub(claims)
        .unwrap();
    assert_eq!(wrapper + prepared + claims, complete);
    assert!(wrapper >= size_of::<AuthenticatedProductionBindings>());
    for erased in [false, true] {
        assert_eq!(
            prepared_header(erased).unwrap()
                + Unrolled::active_header(erased)
                + size_of::<Policy7ExecutionWitnessV1>()
                + size_of::<String>(),
            prepared
        );
        assert_eq!(
            composed_header(erased).unwrap()
                + if erased {
                    size_of::<ErasedComposed>()
                } else {
                    size_of::<DirectComposed>()
                },
            size_of::<Composed>()
        );
    }
    // Retain the old descriptor family as a distinct type, not an encoded U grant.
    assert_ne!(size_of::<LoopUnrollDescriptorErrorV1>(), 0);
    assert_ne!(size_of::<RefinedForwardingDescriptorErrorV1>(), 0);
}

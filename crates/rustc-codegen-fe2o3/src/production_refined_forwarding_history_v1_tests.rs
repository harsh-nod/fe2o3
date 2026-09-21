//! Real source-owned histories; no reconstructed native or executed-policy owner.
use super::super::history::PreparedRefinedForwardingHistoryClaimsV1 as Claims;
use super::*;
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryErrorV1 as HError,
    check_canonical_refined_forwarding_history_v1,
};

#[path = "production_refined_forwarding_history_resources_v1_tests.rs"]
mod resources;

fn with_native(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(&mut PreparedRefinedForwardingNativeOutputV1, &Claims, &mut Budget<'_>),
) {
    with_prefix(erased, profile, mutation, |prefix, budget| {
        let floor = budget.storage();
        let (mut native, receipt) = prepare(
            prefix,
            profile,
            Limits::default(),
            ForwardingLimits::default(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let claims = Claims::prepare(&native, budget).unwrap();
        let retained = claims.retained_storage();
        budget.reserve_storage(retained).unwrap();
        next(&mut native, &claims, budget);
        drop(claims);
        budget.release_storage(retained).unwrap();
        release(native, receipt, budget);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn refined_forwarding_history_actual_source_owners_replay_complete_mutations_and_noops() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for mutation in [false, true] {
                with_native(erased, profile, mutation, |native, claims, budget| {
                    let floor = budget.storage();
                    let ledger = budget.work_ledger_identity_v1();
                    shape(
                        native,
                        erased,
                        if mutation { 2 } else { 0 },
                        if mutation { 2 } else { 0 },
                        budget,
                    );
                    let checked = claims.check(native, budget).unwrap();
                    let retained = checked.retained_storage();
                    budget.reserve_storage(retained).unwrap();
                    let (i, j, final_graph, actual) = checked.observed();
                    let inputs = claims.inputs_for_test(native);
                    assert!(std::ptr::eq(i, inputs.prefix.prefix.prefix.output));
                    assert!(std::ptr::eq(j, inputs.prefix.prefix.output));
                    assert!(std::ptr::eq(final_graph, native.output()));
                    assert!(std::ptr::eq(final_graph, actual));
                    assert!(!std::ptr::eq(i, final_graph) && !std::ptr::eq(j, final_graph));
                    assert_eq!(inputs.limits.refinement, native.refinement_limits());
                    assert_eq!(inputs.limits.forwarding, native.limits());
                    drop(checked);
                    budget.release_storage(retained).unwrap();
                    native.verify_equivalence(budget).unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                });
            }
        }
    }
}

#[test]
fn refined_forwarding_history_actual_mutation_requires_all_five_complete_origin_sets() {
    for erased in [false, true] {
        with_native(erased, Profile::Gfx942, true, |native, claims, budget| {
            let inputs = claims.inputs_for_test(native);
            assert!(!inputs.promotion_origins.is_empty());
            assert!(!inputs.preheader_rows.is_empty());
            assert!(!inputs.licm_origins.is_empty());
            assert!(!inputs.refinement_origins.is_empty());
            assert!(!inputs.forwarding_origins.is_empty());
            let floor = budget.storage();
            for stage in 0..5 {
                let mut bad = inputs;
                match stage {
                    0 => bad.promotion_origins = &inputs.promotion_origins[1..],
                    1 => bad.preheader_rows = &inputs.preheader_rows[1..],
                    2 => bad.licm_origins = &inputs.licm_origins[1..],
                    3 => bad.refinement_origins = &inputs.refinement_origins[1..],
                    4 => bad.forwarding_origins = &inputs.forwarding_origins[1..],
                    _ => unreachable!(),
                }
                let error = match check_canonical_refined_forwarding_history_v1(bad, budget) {
                    Err(error) => error,
                    Ok(_) => panic!("incomplete actual history accepted"),
                };
                assert!(match stage {
                    0 => matches!(error, HError::Promotion(_)),
                    1 => matches!(error, HError::Preheaders(_)),
                    2 => matches!(error, HError::Licm(_)),
                    3 => matches!(error, HError::Refinement(_)),
                    4 => matches!(error, HError::Forwarding(_)),
                    _ => false,
                });
                assert_eq!(budget.storage(), floor);
            }
            for changed in [inputs.licm, inputs.refined, inputs.prefix.output] {
                let bad = fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryInputsV1 {
                    output: changed,
                    ..inputs
                };
                assert!(matches!(
                    check_canonical_refined_forwarding_history_v1(bad, budget),
                    Err(HError::Forwarding(_))
                ));
                assert_eq!(budget.storage(), floor);
            }
        });
    }
}

#[test]
fn refined_forwarding_history_actual_p7_swap_is_refused_without_relabeling_f() {
    with_native(false, Profile::Gfx942, true, |first, claims, budget| {
        with_native(
            false,
            Profile::Gfx950,
            true,
            |second, other_claims, other| {
                let floor = budget.storage();
                let other_floor = other.storage();
                assert_ne!(
                    first.prefix_execution.canonical_bytes(),
                    second.prefix_execution.canonical_bytes()
                );
                assert_eq!(
                    first.prefix_execution.retained_storage(),
                    second.prefix_execution.retained_storage()
                );
                std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
                assert!(matches!(
                    claims.check(first, budget),
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        P7Error::Execution("Policy7 complete execution transcript")
                    ))
                ));
                std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
                let checked = claims.check(first, budget).unwrap();
                let retained = checked.retained_storage();
                budget.reserve_storage(retained).unwrap();
                drop(checked);
                budget.release_storage(retained).unwrap();
                let checked = other_claims.check(second, other).unwrap();
                let retained = checked.retained_storage();
                other.reserve_storage(retained).unwrap();
                drop(checked);
                other.release_storage(retained).unwrap();
                assert_eq!(budget.storage(), floor);
                assert_eq!(other.storage(), other_floor);
            },
        );
    });
}

#[test]
fn refined_forwarding_history_requires_claim_backing_above_the_actual_owner_floor() {
    with_native(true, Profile::Gfx950, true, |native, claims, parent| {
        let floor = parent.storage();
        let required = native.retained_storage_floor_v1() + claims.retained_storage();
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        budget.reserve_storage(required - 1).unwrap();
        assert!(matches!(
            claims.check(native, &mut budget),
            Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                P7Error::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), required - 1);
        assert_eq!(parent.storage(), floor);
    });
}

#[test]
fn refined_forwarding_history_new_full_wrapper_header_is_counted_once() {
    let old_embedded = size_of::<PreparedRefinedForwardingNativeOutputV1>();
    let claims = size_of::<Claims>();
    let complete = size_of::<RefinedForwardingNativeProductionCompilationV1>();
    let wrapper = complete
        .checked_sub(old_embedded)
        .unwrap()
        .checked_sub(claims)
        .unwrap();
    assert_eq!(old_embedded + claims + wrapper, complete);
    assert!(wrapper >= size_of::<AuthenticatedProductionBindings>());
    assert!(claims > 0);
}

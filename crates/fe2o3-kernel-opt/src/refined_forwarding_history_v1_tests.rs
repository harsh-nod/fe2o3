//! Reuse the genuine fixed P8 fixture without reconstructing executed owners.
use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as FLimits,
    CanonicalKirInductionRefinementOriginV1 as ROrigin, CanonicalKirLoopLimitsV1 as RLimits,
};
type HInputs<'a> = CanonicalRefinedForwardingHistoryInputsV1<'a>;
type HError = CanonicalRefinedForwardingHistoryErrorV1;
type HLimits = CanonicalRefinedForwardingHistoryLimitsV1;
const HWORK: usize = 1_000_000_000;
const HSTORE: usize = 1_000_000_000;

#[path = "refined_forwarding_history_resources_v1_tests.rs"]
mod resources;

struct Complete {
    prefix: Prepared8,
    promoted: OwnedPrivateCellPromotionContinuationV1,
    preheaders: OwnedLoopPreheadersContinuationV1,
    licm: OwnedLicmContinuationV1,
    refined: OwnedInductionRefinementContinuationV1,
    forwarded: OwnedCrossBlockForwardingV1,
    floor: usize,
}
impl Complete {
    fn inputs(&self) -> HInputs<'_> {
        HInputs {
            prefix: self.prefix.inputs(),
            promoted: self.promoted.output(),
            selected_allocations: self.promoted.selected_allocations(),
            promotion_origins: self.promoted.origins(),
            preheaders: self.preheaders.output(),
            preheader_rows: self.preheaders.preheaders(),
            licm: self.licm.output(),
            licm_origins: self.licm.origins(),
            refined: self.refined.output(),
            refinement_origins: self.refined.origins(),
            output: self.forwarded.output(),
            forwarding_origins: self.forwarded.origins(),
            limits: HLimits {
                refinement: self.refined.limits(),
                forwarding: self.forwarded.limits(),
            },
        }
    }
}
fn complete(prefix: Prepared8) -> Complete {
    let mut work = Work::new(HWORK);
    let mut budget = Budget::new(&mut work, HSTORE);
    budget.reserve_storage(prefix.floor).unwrap();
    let promoted =
        prepare_owned_private_cell_promotion_v1(prefix.tail.output(), &mut budget).unwrap();
    budget.reserve_storage(promoted.retained_storage()).unwrap();
    let preheaders = prepare_owned_loop_preheaders_v1(promoted.output(), &mut budget).unwrap();
    budget
        .reserve_storage(preheaders.retained_storage())
        .unwrap();
    let licm = prepare_owned_licm_v1(preheaders.output(), &mut budget).unwrap();
    budget.reserve_storage(licm.retained_storage()).unwrap();
    let refined =
        prepare_owned_induction_refinement_v1(licm.output(), RLimits::default(), &mut budget)
            .unwrap();
    budget.reserve_storage(refined.retained_storage()).unwrap();
    let forwarded =
        prepare_owned_cross_block_forwarding_v1(refined.output(), FLimits::default(), &mut budget)
            .unwrap();
    budget
        .reserve_storage(forwarded.retained_storage())
        .unwrap();
    let floor = budget.storage();
    Complete {
        prefix,
        promoted,
        preheaders,
        licm,
        refined,
        forwarded,
        floor,
    }
}
fn check(p: &Complete, inputs: HInputs<'_>) -> Result<(), HError> {
    let mut work = Work::new(HWORK);
    let mut budget = Budget::new(&mut work, HSTORE);
    budget.reserve_storage(p.floor).unwrap();
    let token = budget.work_ledger_identity_v1();
    let result =
        check_canonical_refined_forwarding_history_v1(inputs, &mut budget).map(|receipt| {
            let bytes = receipt.storage().retained_storage();
            budget.reserve_storage(bytes).unwrap();
            assert!(!receipt.grants_authority());
            assert!(!receipt.authenticates_execution());
            assert!(std::ptr::eq(
                receipt.prefix().output(),
                inputs.prefix.output
            ));
            assert!(std::ptr::eq(
                receipt.promotion().input(),
                receipt.prefix().output()
            ));
            assert!(std::ptr::eq(
                receipt.preheaders().input(),
                receipt.promotion().output()
            ));
            assert!(std::ptr::eq(
                receipt.licm().input(),
                receipt.preheaders().output()
            ));
            assert!(std::ptr::eq(
                receipt.refinement().input(),
                receipt.licm().output()
            ));
            assert!(std::ptr::eq(
                receipt.forwarding().input(),
                receipt.refinement().output()
            ));
            assert!(std::ptr::eq(
                receipt.output(),
                receipt.forwarding().output()
            ));
            assert!(std::ptr::eq(receipt.output(), inputs.output));
            receipt.replay(inputs.limits, &mut budget).unwrap();
            drop(receipt);
            budget.release_storage(bytes).unwrap();
        });
    assert_eq!(budget.storage(), p.floor);
    assert!(budget.work_ledger_identity_v1() == token);
    result
}

#[test]
fn refined_forwarding_history_checks_empty_and_actual_mutating_policy8_prefixes() {
    let empty = complete(prepared_module(&Module::new("complete-empty-history")));
    check(&empty, empty.inputs()).unwrap();
    for mutation in [false, true] {
        let p = complete(prepared(mutation, mutation));
        assert_eq!(p.prefix.tail.proved_pairs(), usize::from(mutation));
        assert_eq!(p.promoted.selected_allocations().len(), 1);
        assert!(p.preheaders.preheaders().is_empty());
        assert!(
            p.refined
                .origins()
                .iter()
                .all(|r| matches!(r, ROrigin::Unchanged { .. }))
        );
        assert!(p.forwarded.origins().iter().all(|r| r.store.is_none()));
        check(&p, p.inputs()).unwrap();
    }
}

#[test]
fn refined_forwarding_history_rejects_every_foreign_intermediate_endpoint() {
    let p = complete(prepared(false, false));
    let foreign = complete(prepared(true, true));
    for stage in 0..5 {
        let mut inputs = p.inputs();
        match stage {
            0 => inputs.promoted = foreign.promoted.output(),
            1 => inputs.preheaders = foreign.preheaders.output(),
            2 => inputs.licm = foreign.licm.output(),
            3 => inputs.refined = foreign.refined.output(),
            4 => inputs.output = foreign.forwarded.output(),
            _ => unreachable!(),
        }
        let error = check(&p, inputs).unwrap_err();
        assert!(match stage {
            0 => matches!(error, HError::Promotion(_)),
            1 => matches!(error, HError::Preheaders(_)),
            2 => matches!(error, HError::Licm(_)),
            3 => matches!(error, HError::Refinement(_)),
            4 => matches!(error, HError::Forwarding(_)),
            _ => false,
        });
    }
}

#[test]
fn refined_forwarding_history_requires_complete_ordered_origins_at_every_tail() {
    let p = complete(prepared(true, true));
    assert_eq!(p.promoted.selected_allocations().len(), 1);
    let mut missing_selection = p.inputs();
    missing_selection.selected_allocations = &[];
    assert!(matches!(
        check(&p, missing_selection),
        Err(HError::Promotion(_))
    ));
    let mut promotion = p.promoted.origins().to_vec();
    let mut licm = p.licm.origins().to_vec();
    let mut refined = p.refined.origins().to_vec();
    let mut forwarded = p.forwarded.origins().to_vec();
    assert!(promotion.len() > 1 && licm.len() > 1 && refined.len() > 1 && forwarded.len() > 1);
    for omit in [false, true] {
        for stage in 0..4 {
            let mut inputs = p.inputs();
            match stage {
                0 => {
                    promotion.swap(0, 1);
                    inputs.promotion_origins = if omit { &promotion[1..] } else { &promotion };
                }
                1 => {
                    licm.swap(0, 1);
                    inputs.licm_origins = if omit { &licm[1..] } else { &licm };
                }
                2 => {
                    refined.swap(0, 1);
                    inputs.refinement_origins = if omit { &refined[1..] } else { &refined };
                }
                3 => {
                    forwarded.swap(0, 1);
                    inputs.forwarding_origins = if omit { &forwarded[1..] } else { &forwarded };
                }
                _ => unreachable!(),
            }
            let error = check(&p, inputs).unwrap_err();
            assert!(match stage {
                0 => matches!(error, HError::Promotion(_)),
                1 => matches!(error, HError::Licm(_)),
                2 => matches!(error, HError::Refinement(_)),
                3 => matches!(error, HError::Forwarding(_)),
                _ => false,
            });
        }
    }
    let mut inputs = p.inputs();
    let unexpected = [fe2o3_kernel_analysis::CanonicalKirLoopPreheaderV1 {
        header: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
        preheader: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
    }];
    inputs.preheader_rows = &unexpected;
    assert!(matches!(check(&p, inputs), Err(HError::Preheaders(_))));
}

#[test]
fn refined_forwarding_history_preserves_old_policy_and_record_identities() {
    let p = complete(prepared(true, true));
    let inputs = p.inputs();
    let mut p4 = inputs.prefix.prefix.prefix.prefix.policy4_wire.to_vec();
    let mut p7 = inputs.prefix.prefix.continuation.execution_record.to_vec();
    let mut transition = inputs
        .prefix
        .prefix
        .prefix
        .continuation
        .transition_wire
        .to_vec();
    p4[0] ^= 1;
    p7[16] ^= 1;
    transition[0] ^= 1;
    for stage in 0..4 {
        let mut inputs = inputs;
        match stage {
            0 => inputs.prefix.prefix.prefix.prefix.policy4_wire = &p4,
            1 => inputs.prefix.prefix.continuation.execution_record = &p7,
            2 => inputs.prefix.prefix.prefix.continuation.transition_wire = &transition,
            3 => inputs.prefix.continuation.pass_name = "not-the-fixed-policy8-pass",
            _ => unreachable!(),
        }
        assert!(matches!(check(&p, inputs), Err(HError::Prefix(_))));
    }
}

#[test]
fn refined_forwarding_history_replay_keeps_exact_limits_even_when_both_admit() {
    let p = complete(prepared(false, false));
    let inputs = p.inputs();
    let mut work = Work::new(HWORK);
    let mut budget = Budget::new(&mut work, HSTORE);
    budget.reserve_storage(p.floor).unwrap();
    let receipt = check_canonical_refined_forwarding_history_v1(inputs, &mut budget).unwrap();
    let bytes = receipt.storage().retained_storage();
    budget.reserve_storage(bytes).unwrap();
    for field in 0..2 {
        for increase in [false, true] {
            let mut changed = inputs.limits;
            let operations = if field == 0 {
                &mut changed.refinement.operations
            } else {
                &mut changed.forwarding.memory.operations
            };
            *operations = if increase {
                *operations + 1
            } else {
                *operations - 1
            };
            let mut changed_inputs = inputs;
            changed_inputs.limits = changed;
            let alternative =
                check_canonical_refined_forwarding_history_v1(changed_inputs, &mut budget).unwrap();
            let alternative_storage = alternative.storage().retained_storage();
            budget.reserve_storage(alternative_storage).unwrap();
            drop(alternative);
            budget.release_storage(alternative_storage).unwrap();
            assert!(matches!(
                receipt.replay(changed, &mut budget),
                Err(HError::LimitsMismatch)
            ));
            assert_eq!(budget.storage(), p.floor + bytes);
        }
    }
    drop(receipt);
    budget.release_storage(bytes).unwrap();
}

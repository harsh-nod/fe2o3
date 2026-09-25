//! One actual owned J/K/P/H/L/R/F chain, with borrowed semantic claims only.
use super::{Budget, Owner, Prefix, ProductionPipelineError, Resource, join_error, resource};
use crate::production_pipeline::checked_output_policy7_v1::Policy7ExecutionWitnessV1;
use fe2o3_kernel_ir::{
    InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    InertCanonicalKirTransitionReceiptV1 as Transition,
};
use fe2o3_kernel_opt::{
    CanonicalPolicy5SemanticInputsV1, CanonicalPolicy6ContinuationClaimsV1,
    CanonicalPolicy6SemanticInputsV1, CanonicalPolicy7ContinuationClaimsV1,
    CanonicalPolicy7SemanticInputsV1, CanonicalPolicy8ContinuationClaimsV1,
    CanonicalPolicy8SemanticInputsV1, CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    CanonicalRefinedForwardingHistoryLimitsV1 as Limits,
    InertCanonicalPolicy4ExecutionReceiptV1 as Policy4, OwnedCrossBlockForwardingV1 as Forwarded,
    OwnedInductionRefinementContinuationV1 as Refined, OwnedLicmContinuationV1 as Licm,
    OwnedLoopPreheadersContinuationV1 as Preheaders,
    OwnedPrivateCellPromotionContinuationV1 as Promoted, OwnedRedundantStoreContinuationV1 as Tail,
    POLICY8_COMMUTATIVE_PASS_NAME_V1, encode_checked_canonical_policy4_execution_receipt_v1,
    prepare_owned_cross_block_forwarding_v1, prepare_owned_induction_refinement_v1,
    prepare_owned_licm_v1, prepare_owned_loop_preheaders_v1,
    prepare_owned_private_cell_promotion_v1, prepare_owned_redundant_store_continuation_v1,
};
use fe2o3_pliron::{
    OwnedCommutativeBitwiseContinuationV1 as Commutative,
    prepare_owned_commutative_bitwise_continuation_v1,
};
use std::mem::size_of;

pub(in crate::production_pipeline) struct FinalChain {
    tail: Tail,
    commutative: Commutative,
    promoted: Promoted,
    preheaders: Preheaders,
    licm: Licm,
    refined: Refined,
    forwarded: Forwarded,
    policy4: Policy4,
    transition: Transition,
    transition_storage: usize,
    execution: Policy7ExecutionWitnessV1,
    expected_limits: Limits,
    prefix_floor: usize,
    retained: usize,
}

impl FinalChain {
    /// Inputs and the containing wrapper are prepaid. Each existing constructor
    /// returns unreserved ownership, reserved immediately on this same account.
    /// This internal helper leaves the complete returned chain reserved.
    pub(super) fn prepare(
        bound: &Owner,
        checked: &Prefix,
        expected_limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<Self, ProductionPipelineError> {
        let prefix_floor = budget.storage();
        budget
            .reserve_storage(header().map_err(resource)?)
            .map_err(resource)?;
        let tail = prepare_owned_redundant_store_continuation_v1(checked.owner(), budget)
            .map_err(join_error)?;
        budget
            .reserve_storage(tail.retained_storage())
            .map_err(resource)?;
        let execution = Policy7ExecutionWitnessV1::prepare_conditional_v1(checked, &tail, budget)?;
        budget
            .reserve_storage(execution.retained_storage())
            .map_err(resource)?;
        let commutative = prepare_owned_commutative_bitwise_continuation_v1(tail.output(), budget)
            .map_err(join_error)?;
        budget
            .reserve_storage(commutative.retained_storage())
            .map_err(resource)?;
        let promoted = prepare_owned_private_cell_promotion_v1(commutative.output(), budget)
            .map_err(join_error)?;
        budget
            .reserve_storage(promoted.retained_storage())
            .map_err(resource)?;
        let preheaders =
            prepare_owned_loop_preheaders_v1(promoted.output(), budget).map_err(join_error)?;
        budget
            .reserve_storage(preheaders.retained_storage())
            .map_err(resource)?;
        let licm = prepare_owned_licm_v1(preheaders.output(), budget).map_err(join_error)?;
        budget
            .reserve_storage(licm.retained_storage())
            .map_err(resource)?;
        let refined = prepare_owned_induction_refinement_v1(
            licm.output(),
            expected_limits.refinement,
            budget,
        )
        .map_err(join_error)?;
        budget
            .reserve_storage(refined.retained_storage())
            .map_err(resource)?;
        let forwarded = prepare_owned_cross_block_forwarding_v1(
            refined.output(),
            expected_limits.forwarding,
            budget,
        )
        .map_err(join_error)?;
        budget
            .reserve_storage(forwarded.retained_storage())
            .map_err(resource)?;
        let policy4 = encode_checked_canonical_policy4_execution_receipt_v1(
            bound,
            checked.intermediate_policy5().intermediate_policy4(),
            budget,
        )
        .map_err(join_error)?;
        budget
            .reserve_storage(policy4.storage().retained_storage())
            .map_err(resource)?;
        let (transition, storage) = Transition::from_candidate_with_budget(
            checked
                .intermediate_policy5()
                .owner()
                .canonical()
                .identity(),
            checked.owner().canonical().identity(),
            checked.continuation().occurrences().candidate(),
            budget,
        )
        .map_err(join_error)?;
        let transition_storage = storage.retained_storage();
        budget
            .reserve_storage(transition_storage)
            .map_err(resource)?;
        let retained = budget
            .storage()
            .checked_sub(prefix_floor)
            .ok_or_else(|| resource(Resource::Accounting))?;
        Ok(Self {
            tail,
            commutative,
            promoted,
            preheaders,
            licm,
            refined,
            forwarded,
            policy4,
            transition,
            transition_storage,
            execution,
            expected_limits,
            prefix_floor,
            retained,
        })
    }

    #[cfg(test)]
    pub(super) fn output(&self) -> &Owner {
        self.forwarded.output()
    }

    pub(super) fn expected_limits(&self) -> Limits {
        self.expected_limits
    }

    /// Borrowed history lacks capacities and original reservations. These
    /// once-built owners have private fields and immutable accessors; their
    /// constructors' full capacity receipts cannot change while retained here.
    /// Lower replays semantics once; this check only admits their complete floor.
    pub(super) fn check_owned(&self, budget: &mut Budget<'_>) -> Result<(), Resource> {
        budget.charge_work(32)?;
        let retained = [
            self.tail.retained_storage(),
            self.commutative.retained_storage(),
            self.promoted.retained_storage(),
            self.preheaders.retained_storage(),
            self.licm.retained_storage(),
            self.refined.retained_storage(),
            self.forwarded.retained_storage(),
            self.policy4.storage().retained_storage(),
            self.transition_storage,
            self.execution.retained_storage(),
        ]
        .into_iter()
        .try_fold(header()?, |n, item| n.checked_add(item))
        .ok_or(Resource::Arithmetic)?;
        let required = self
            .prefix_floor
            .checked_add(retained)
            .ok_or(Resource::Arithmetic)?;
        if retained != self.retained
            || budget.storage() < required
            || self.refined.limits() != self.expected_limits.refinement
            || self.forwarded.limits() != self.expected_limits.forwarding
        {
            return Err(Resource::Accounting);
        }
        Ok(())
    }

    pub(super) fn inputs<'a>(&'a self, bound: &'a Owner, checked: &'a Prefix) -> Inputs<'a> {
        let p5 = checked.intermediate_policy5();
        let p4 = p5.intermediate_policy4();
        Inputs {
            prefix: CanonicalPolicy8SemanticInputsV1 {
                prefix: CanonicalPolicy7SemanticInputsV1 {
                    prefix: CanonicalPolicy6SemanticInputsV1 {
                        prefix: CanonicalPolicy5SemanticInputsV1 {
                            input: bound,
                            intermediate: p4.intermediate_policy3().owner(),
                            stored: p4.owner(),
                            output: p5.owner(),
                            policy4_wire: self.policy4.canonical_bytes(),
                            policy5_record: p5.execution().canonical_bytes(),
                            load_rows: p5.load_forwarding_rows(),
                        },
                        output: checked.owner(),
                        continuation: CanonicalPolicy6ContinuationClaimsV1 {
                            composition_record: checked.execution().canonical_bytes(),
                            integer_record: checked.continuation().execution().canonical_bytes(),
                            transition_wire: self.transition.canonical_bytes(),
                        },
                    },
                    output: self.tail.output(),
                    continuation: CanonicalPolicy7ContinuationClaimsV1 {
                        execution_record: self.execution.canonical_bytes(),
                        deletion_rows: self.tail.rows(),
                        retained_operations: self.tail.retained_operations(),
                    },
                },
                output: self.commutative.output(),
                continuation: CanonicalPolicy8ContinuationClaimsV1 {
                    pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                    input: Identity::from_verified(self.tail.output().canonical().identity()),
                    output: Identity::from_verified(
                        self.commutative.output().canonical().identity(),
                    ),
                    occurrences: self.commutative.occurrences().candidate(),
                },
            },
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
            limits: Limits {
                refinement: self.refined.limits(),
                forwarding: self.forwarded.limits(),
            },
        }
    }
}

fn header() -> Result<usize, Resource> {
    [
        size_of::<Tail>(),
        size_of::<Commutative>(),
        size_of::<Promoted>(),
        size_of::<Preheaders>(),
        size_of::<Licm>(),
        size_of::<Refined>(),
        size_of::<Forwarded>(),
        size_of::<Policy4>(),
        size_of::<Transition>(),
        size_of::<Policy7ExecutionWitnessV1>(),
    ]
    .into_iter()
    .try_fold(size_of::<FinalChain>(), |n, item| n.checked_sub(item))
    .ok_or(Resource::Arithmetic)
}

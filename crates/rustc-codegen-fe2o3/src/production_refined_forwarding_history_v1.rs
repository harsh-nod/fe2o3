//! Actual retained B-through-F claims and complete independent semantic replay.
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::semantic::{
    PortablePolicy7ClaimsV1, check_portable_history_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKirTransitionReceiptErrorV1, InertCanonicalKirTransitionGraphIdentityV1 as Identity,
    InertCanonicalKirTransitionReceiptV1 as Transition,
};
use fe2o3_kernel_opt::{
    CanonicalPolicy4ExecutionReceiptErrorV1, CanonicalPolicy5SemanticInputsV1,
    CanonicalPolicy6ContinuationClaimsV1, CanonicalPolicy6SemanticInputsV1,
    CanonicalPolicy7ContinuationClaimsV1, CanonicalPolicy7SemanticInputsV1,
    CanonicalPolicy8ContinuationClaimsV1, CanonicalPolicy8SemanticInputsV1,
    CanonicalRefinedForwardingHistoryErrorV1, CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    CanonicalRefinedForwardingHistoryLimitsV1 as HistoryLimits,
    CheckedCanonicalPolicy6ExecutionRelationV1 as Policy6,
    CheckedCanonicalPolicy7ContinuationRelationV1 as Policy7,
    CheckedCanonicalRefinedForwardingHistoryV1 as Semantic,
    InertCanonicalPolicy4ExecutionReceiptV1 as Policy4, POLICY8_COMMUTATIVE_PASS_NAME_V1,
    check_canonical_refined_forwarding_history_v1,
    encode_checked_canonical_policy4_execution_receipt_v1,
};

#[derive(Debug)]
pub(crate) enum RefinedForwardingHistoryErrorV1 {
    Policy4(CanonicalPolicy4ExecutionReceiptErrorV1),
    Transition(CanonicalKirTransitionReceiptErrorV1),
    Semantics(Box<CanonicalRefinedForwardingHistoryErrorV1>),
}
impl fmt::Display for RefinedForwardingHistoryErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RefinedForwardingHistoryErrorV1 {}
fn history_error(value: RefinedForwardingHistoryErrorV1) -> ProductionPipelineError {
    error(RefinedForwardingNativeStageErrorV1::History(Box::new(
        value,
    )))
}

/// Only the two new bounded claim owners. All graph/row/other record custody
/// remains in the actual composed native owner, with no duplicated graph identity.
pub(super) struct PreparedRefinedForwardingHistoryClaimsV1 {
    policy4: Policy4,
    transition: Transition,
    retained: usize,
}
type Claims = PreparedRefinedForwardingHistoryClaimsV1;

/// Closed actual-stage authentication plus the independent complete semantics.
/// Its actual owner borrow cannot be reconstructed from canonical record bytes.
pub(super) struct AuthenticatedRefinedForwardingHistoryV1<'a> {
    policy6: Policy6<'a>,
    policy7: Policy7<'a>,
    semantic: Semantic<'a>,
    actual: &'a PreparedRefinedForwardingNativeOutputV1,
    retained: usize,
}
type Checked<'a> = AuthenticatedRefinedForwardingHistoryV1<'a>;
impl Checked<'_> {
    pub(super) const fn retained_storage(&self) -> usize {
        self.retained
    }
    #[cfg(test)]
    pub(super) fn observed(&self) -> (&Graph, &Graph, &Graph, &Graph) {
        (
            self.policy6.execution_owner().owner(),
            self.policy7.relation().output(),
            self.semantic.output(),
            self.actual.output(),
        )
    }
}

fn claim_header() -> Result<usize> {
    size_of::<Claims>()
        .checked_sub(size_of::<Policy4>())
        .and_then(|n| n.checked_sub(size_of::<Transition>()))
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn checked_header() -> Result<usize> {
    size_of::<Checked<'_>>()
        .checked_sub(size_of::<Policy6<'_>>())
        .and_then(|n| n.checked_sub(size_of::<Policy7<'_>>()))
        .and_then(|n| n.checked_sub(size_of::<Semantic<'_>>()))
        .ok_or_else(|| resource(Resource::Arithmetic))
}

impl Claims {
    pub(super) const fn retained_storage(&self) -> usize {
        self.retained
    }

    #[cfg(test)]
    pub(super) fn inputs_for_test<'a>(
        &'a self,
        native: &'a PreparedRefinedForwardingNativeOutputV1,
    ) -> Inputs<'a> {
        self.inputs(native)
    }

    /// Input owners are never cloned or moved out. Return only the unreserved
    /// additional claim backing; the sole full-owner factory reserves it next.
    pub(super) fn prepare(
        native: &PreparedRefinedForwardingNativeOutputV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        scoped(native.retained_storage_floor_v1(), budget, |budget| {
            native.verify_equivalence(budget)?;
            let header = claim_header()?;
            budget.reserve_storage(header).map_err(resource)?;
            budget.charge_work(16).map_err(resource)?;
            let (bound, checked) = native.owner.history().portable_prefix();
            let p5 = checked.intermediate_policy5();
            let policy4 = encode_checked_canonical_policy4_execution_receipt_v1(
                bound,
                p5.intermediate_policy4(),
                budget,
            )
            .map_err(|e| history_error(RefinedForwardingHistoryErrorV1::Policy4(e)))?;
            let p4_storage = policy4.storage().retained_storage();
            budget.reserve_storage(p4_storage).map_err(resource)?;
            let (transition, ts) = Transition::from_candidate_with_budget(
                p5.owner().canonical().identity(),
                checked.owner().canonical().identity(),
                checked.continuation().occurrences().candidate(),
                budget,
            )
            .map_err(|e| history_error(RefinedForwardingHistoryErrorV1::Transition(e)))?;
            budget
                .reserve_storage(ts.retained_storage())
                .map_err(resource)?;
            let retained = header
                .checked_add(p4_storage)
                .and_then(|n| n.checked_add(ts.retained_storage()))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let claims = Self {
                policy4,
                transition,
                retained,
            };
            let checked = claims.check(native, budget)?;
            let retained = checked.retained_storage();
            budget.reserve_storage(retained).map_err(resource)?;
            drop(checked);
            budget.release_storage(retained).map_err(resource)?;
            Ok(claims)
        })
    }

    pub(super) fn inputs<'a>(
        &'a self,
        native: &'a PreparedRefinedForwardingNativeOutputV1,
    ) -> Inputs<'a> {
        macro_rules! inputs {
            ($owner:expr) => {{
                let f = $owner;
                let r = f.prefix();
                let l = r.prefix();
                let h = l.prefix();
                let p = h.prefix();
                let k = p.prefix();
                let j = k.prefix();
                let i = j.prefix();
                let checked = i.checked_output();
                let p5 = checked.intermediate_policy5();
                let p4 = p5.intermediate_policy4();
                Inputs {
                    prefix: CanonicalPolicy8SemanticInputsV1 {
                        prefix: CanonicalPolicy7SemanticInputsV1 {
                            prefix: CanonicalPolicy6SemanticInputsV1 {
                                prefix: CanonicalPolicy5SemanticInputsV1 {
                                    input: i.bound(),
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
                                    integer_record: checked
                                        .continuation()
                                        .execution()
                                        .canonical_bytes(),
                                    transition_wire: self.transition.canonical_bytes(),
                                },
                            },
                            output: j.output(),
                            continuation: CanonicalPolicy7ContinuationClaimsV1 {
                                execution_record: native.prefix_execution.canonical_bytes(),
                                deletion_rows: j.continuation().rows(),
                                retained_operations: j.continuation().retained_operations(),
                            },
                        },
                        output: k.output(),
                        continuation: CanonicalPolicy8ContinuationClaimsV1 {
                            pass_name: POLICY8_COMMUTATIVE_PASS_NAME_V1,
                            input: Identity::from_verified(j.output().canonical().identity()),
                            output: Identity::from_verified(k.output().canonical().identity()),
                            occurrences: k.continuation().occurrences().candidate(),
                        },
                    },
                    promoted: p.output(),
                    selected_allocations: p.continuation().selected_allocations(),
                    promotion_origins: p.continuation().origins(),
                    preheaders: h.output(),
                    preheader_rows: h.continuation().preheaders(),
                    licm: l.output(),
                    licm_origins: l.continuation().origins(),
                    refined: r.output(),
                    refinement_origins: r.continuation().origins(),
                    output: f.output(),
                    forwarding_origins: f.continuation().origins(),
                    limits: HistoryLimits {
                        refinement: r.limits(),
                        forwarding: f.limits(),
                    },
                }
            }};
        }
        match &native.owner {
            Composed::Direct(owner) => inputs!(owner),
            Composed::Erased(owner) => inputs!(owner),
        }
    }

    /// Checks generated claims against actual P6/P7 execution, then separately
    /// replays fixed B-through-F semantics. Historical claims stay about I/J/K.
    /// The complete actual source/native replay remains mandatory in the caller.
    pub(super) fn check<'a>(
        &'a self,
        native: &'a PreparedRefinedForwardingNativeOutputV1,
        budget: &mut Budget<'_>,
    ) -> Result<Checked<'a>> {
        let required = native
            .retained_storage_floor_v1()
            .checked_add(self.retained)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        scoped(required, budget, |budget| {
            let header = checked_header()?;
            budget.reserve_storage(header).map_err(resource)?;
            // Fixed borrowed schema assembly; no per-operation clone or search.
            budget.charge_work(128).map_err(resource)?;
            let inputs = self.inputs(native);
            let p7 = inputs.prefix.prefix;
            let p6 = p7.prefix;
            let p5 = p6.prefix;
            let (policy6, policy7) = check_portable_history_v1(
                native.owner.history(),
                &native.prefix_execution,
                PortablePolicy7ClaimsV1 {
                    policy4_wire: p5.policy4_wire,
                    policy5_record: p5.policy5_record,
                    load_rows: p5.load_rows,
                    policy6: p6.continuation,
                    policy7_record: p7.continuation.execution_record,
                },
                budget,
            )?;
            // check_portable_history_v1 returns these two receipts still reserved.
            let p6_storage = policy6.storage().retained_storage();
            let p7_storage = policy7.storage().retained_storage();
            let semantic =
                check_canonical_refined_forwarding_history_v1(inputs, budget).map_err(|e| {
                    history_error(RefinedForwardingHistoryErrorV1::Semantics(Box::new(e)))
                })?;
            let semantic_storage = semantic.storage().retained_storage();
            budget.reserve_storage(semantic_storage).map_err(resource)?;
            let retained = header
                .checked_add(p6_storage)
                .and_then(|n| n.checked_add(p7_storage))
                .and_then(|n| n.checked_add(semantic_storage))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.charge_work(1).map_err(resource)?;
            Ok(Checked {
                policy6,
                policy7,
                semantic,
                actual: native,
                retained,
            })
        })
    }
}

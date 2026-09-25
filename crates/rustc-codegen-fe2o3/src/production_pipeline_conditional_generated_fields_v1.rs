//! Generated-field projection borrows the existing compilation's retained roots.
//! No reconstructed root slice or source digest can construct this scope.
use super::AuthenticatedProductionBindings;
use crate::compiler_descriptor::{
    TypedDescriptorRootV1,
    conditional_contract_projection_v1::with_conditional_contract_projection_v1,
    conditional_generated_fields_v1::{ConditionalGeneratedFieldErrorV1, with_generated_fields_v1},
};
use crate::production_ranked_projection_v1::{
    ProductionRankedSemanticProgramV1, ProductionRankedVerificationErrorV1,
};
use crate::production_reference_effect_join_v2::ProductionReferenceEffectJoinErrorV2;
use fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1;

#[path = "production_pipeline_conditional_contract_retention_v1.rs"]
mod retention;
pub(crate) use retention::RetainedConditionalContractV1;

#[path = "production_pipeline_conditional_checked_output_v1.rs"]
mod checked_output;

#[cfg(test)]
pub(crate) use crate::compiler_descriptor::conditional_generated_fields_v1::observation;

/// Borrowed custody from an existing compilation stage, not an admission token.
/// Only the retained replay below constructs it. No work identity is retained.
pub(crate) struct ConditionalGeneratedFieldOwnerV1<'a> {
    source: &'a ProductionPreRankedKirOwnerV1,
    roots: &'a [TypedDescriptorRootV1],
    contexts: &'a crate::collector::RetainedContextEntriesV29,
}

impl ConditionalGeneratedFieldOwnerV1<'_> {
    pub(crate) fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        self.source
    }
    pub(crate) fn roots(&self) -> &[TypedDescriptorRootV1] {
        self.roots
    }
    pub(crate) fn contexts(&self) -> &crate::collector::RetainedContextEntriesV29 {
        self.contexts
    }
}

/// The caller destructures the existing ranked stage, retaining `bindings`
/// while moving `ranked` through its ONE original replay. Its callback lends
/// the actual materialized owner alongside the source-bound request and the
/// reimported signed execution. No shared borrow of the consumed stage remains.
pub(super) fn replay_conditional_roots_v1(
    ranked: ProductionRankedSemanticProgramV1,
    bindings: &AuthenticatedProductionBindings,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedVerificationErrorV1> {
    replay_with_check_v1(ranked, bindings, |_, _| Ok(()))
}

/// Direct policy6 only. These immutable owners and their reservations stay on
/// the existing target phase while the callback borrows the original source
/// phase. This does not yield ordinary evidence or continue to native F.
pub(super) fn replay_conditional_policy6_roots_v1(
    ranked: ProductionRankedSemanticProgramV1,
    bindings: &AuthenticatedProductionBindings,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    target: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedVerificationErrorV1> {
    let result = replay_with_check_v1(ranked, bindings, |request, source| {
        checked_output::check(
            request,
            bound,
            checked,
            bindings.rustc_target.profile(),
            target,
            source,
        )
        .map_err(|error| ProductionReferenceEffectJoinErrorV2::ProofExecution(error.to_string()))
    });
    #[cfg(test)]
    if result.is_ok() {
        checked_output::tests::replay_completed();
    }
    result
}

/// The F entry's complete actual owners stay on the original target account.
/// Lower checks N-to-I once and the whole source-through-F history in this replay.
pub(super) fn replay_conditional_prefix_for_f_v1(
    ranked: ProductionRankedSemanticProgramV1,
    bindings: &AuthenticatedProductionBindings,
    bound: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    checked: &fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1,
    chain: &super::checked_output_policy6_v1::conditional_prefix_v1::FinalChain,
    target: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedVerificationErrorV1> {
    replay_with_check_v1(ranked, bindings, |request, source| {
        super::checked_output_policy6_v1::conditional_prefix_v1::check(
            request,
            bound,
            checked,
            chain,
            bindings.rustc_target.profile(),
            target,
            source,
        )
        .map_err(|error| ProductionReferenceEffectJoinErrorV2::ProofExecution(error.to_string()))
    })
}

fn replay_with_check_v1(
    ranked: ProductionRankedSemanticProgramV1,
    bindings: &AuthenticatedProductionBindings,
    mut check: impl FnMut(
        &fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1<'_>,
        &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(), ProductionReferenceEffectJoinErrorV2>,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedVerificationErrorV1> {
    let ranked = ranked.replay_conditional_roots_v1(
        &bindings.reference_effect_bindings,
        |root, source, request, execution, budget| {
            // The existing enclosing replay has reimported the genuine receipt;
            // its source/graph/account postchecks still surround this callback.
            check(request, budget)?;
            with_generated_fields_v1(
                &ConditionalGeneratedFieldOwnerV1 {
                    source,
                    roots: &bindings.typed_descriptor_roots,
                    contexts: &bindings.context_entries,
                },
                request,
                budget,
                |fields, budget| {
                    fields.require_replayed_root_v1(root, budget)?;
                    #[cfg(test)]
                    observation::projection_callback(&fields, execution, budget);
                    with_conditional_contract_projection_v1(
                        &fields,
                        execution,
                        budget,
                        |view, budget| {
                            RetainedConditionalContractV1::copy_unreserved(
                                view.canonical_bytes(),
                                budget,
                            )
                        },
                    )
                    .map_err(ConditionalGeneratedFieldErrorV1::Contract)?
                },
            )
            .map_err(|error| {
                ProductionReferenceEffectJoinErrorV2::ProofExecution(error.to_string())
            })
        },
    )?;
    // This is later than descriptor, verifier, lower and original-phase
    // postchecks. Test observations do not alter the retained owning program.
    #[cfg(test)]
    observation::replay_completed();
    Ok(ranked)
}

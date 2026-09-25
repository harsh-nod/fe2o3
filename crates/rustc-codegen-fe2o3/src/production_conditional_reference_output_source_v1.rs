//! Authenticated backend adapters to the shared content-only CPU/source join.
//! Source rederivation and the one retained formula runtime stay backend-owned.

use super::*;
use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1 as Binding;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
use fe2o3_verifier::conditional_reference_v1::{
    self as portable, ConditionalReferenceInputV1, SourceBoundCpuCorrespondenceV1,
};
use fe2o3_verifier::portable_reference_v1::ReferenceReplayInputV1;

type Error = ProductionReferenceEffectJoinErrorV2;

#[path = "production_conditional_cpu_read_premises_v1.rs"]
pub(crate) mod read_premises_v1;

fn borrowed_input(binding: &Binding) -> ConditionalReferenceInputV1<'_> {
    ConditionalReferenceInputV1 {
        kernel: &binding.kernel,
        reference: &binding.reference,
        replay: ReferenceReplayInputV1 {
            signature_preimage: &binding.signature_preimage,
            effect_ir: &binding.effect_ir,
            effect_ir_sha256: binding.effect_ir_sha256,
            observable_output_writes: &binding.observable_output_writes,
        },
    }
}

pub(crate) fn retain_source_bound_cpu_formula_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &Request<'_>,
    references: &AuthenticatedReferenceEffectBindingsV1,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<fe2o3_verifier::RetainedProductionConditionalFormulaV1, Error> {
    let [binding] = references.as_slice() else {
        return Err(Error::UnsupportedReference(
            "conditional CPU join requires one authenticated binding",
        ));
    };
    with_cpu_binding(request, binding, semantic_root, budget, |cpu, budget| {
        cpu.require_subjects(budget)?;
        fe2o3_verifier::execute_and_retain_conditional_ranked_formula_v1(
            runtime,
            cpu.request(),
            budget,
            timeout_seconds,
        )
        .map_err(|error| Error::ProofExecution(error.to_string()))
    })
}

pub(crate) fn replay_source_bound_cpu_formula_v1<R>(
    retained: &fe2o3_verifier::RetainedProductionConditionalFormulaV1,
    request: &Request<'_>,
    binding: &Binding,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl for<'proof> FnOnce(
        &'proof fe2o3_verifier::ProductionConditionalFormulaExecutionV1,
        &mut Budget<'_>,
    ) -> R,
) -> Result<R, Error> {
    with_cpu_binding(request, binding, semantic_root, budget, |cpu, budget| {
        cpu.require_subjects(budget)?;
        retained
            .with_replayed_request_v1(cpu.request(), budget, consume)
            .map_err(|error| Error::ProofExecution(error.to_string()))
    })
}

fn with_cpu_binding<R>(
    request: &Request<'_>,
    binding: &Binding,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl for<'cpu> FnOnce(
        &'cpu SourceBoundCpuCorrespondenceV1<'_>,
        &mut Budget<'_>,
    ) -> Result<R, Error>,
) -> Result<R, Error> {
    portable::with_source_bound_cpu_correspondence_v1(
        request,
        borrowed_input(binding),
        semantic_root,
        budget,
        consume,
    )
    .map_err(Error::from)?
}

pub(crate) fn subjects(binding: &Binding) -> Result<FunctionalRefinementSubjectsV2, Error> {
    portable::reference_subjects_v1(&binding.kernel, &binding.reference).map_err(Error::from)
}

#[cfg(test)]
fn require_read_origins(
    blocks: &[fe2o3_pliron::ProductionRankedBlockV1],
    load: &fe2o3_pliron::ProductionSemanticLoadV2,
    source_argument: u32,
    adjusted_argument: u32,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    portable::require_read_origins(blocks, load, source_argument, adjusted_argument, budget)
        .map_err(Error::from)
}

#[cfg(test)]
#[path = "production_conditional_read_origins_v1_tests.rs"]
mod read_origin_tests;

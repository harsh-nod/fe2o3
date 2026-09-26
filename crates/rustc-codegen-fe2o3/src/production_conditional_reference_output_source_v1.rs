//! Authenticated backend adapters to the shared content-only CPU/source join.
//! Source rederivation and the one retained formula runtime stay backend-owned.

use super::*;
use crate::reference_effect_v1::AuthenticatedReferenceEffectBindingV1 as Binding;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
use fe2o3_verifier::conditional_reference_v1 as portable;
use fe2o3_verifier::portable_reference_v1::ReferenceReplayInputV1;
use fe2o3_verifier::portable_reference_v1::codec::{NativeCpuAssociationV1, NativeCpuInputV1};
#[cfg(test)]
use portable::{ConditionalReferenceInputV1, SourceBoundCpuCorrespondenceV1};

type Error = ProductionReferenceEffectJoinErrorV2;

#[cfg(test)]
#[path = "production_conditional_cpu_read_premises_v1.rs"]
pub(crate) mod read_premises_v1;

#[cfg(test)]
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

// Borrow the authenticated source row without projecting either complete
// identity. V2 checks the exact root against the original semantic root roster.
fn native_cpu_input_v1<'a>(
    request: &Request<'_>,
    binding: &'a Binding,
    semantic_root: u32,
) -> NativeCpuInputV1<'a> {
    NativeCpuInputV1 {
        association: NativeCpuAssociationV1 {
            semantic_mir_sha256: *request
                .source()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
            semantic_root,
            registration_path: &binding.registration_path,
            logical_kernel_name: &binding.logical_kernel_name,
        },
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

pub(crate) fn retain_source_bound_cpu_formula_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &Request<'_>,
    references: &AuthenticatedReferenceEffectBindingsV1,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<fe2o3_verifier::RetainedProductionConditionalFormulaV2, Error> {
    let [binding] = references.as_slice() else {
        return Err(Error::UnsupportedReference(
            "conditional CPU join requires one authenticated binding",
        ));
    };
    fe2o3_verifier::execute_and_retain_conditional_ranked_formula_v2(
        runtime,
        request,
        native_cpu_input_v1(request, binding, semantic_root),
        budget,
        timeout_seconds,
    )
    .map_err(|error| Error::ProofExecution(error.to_string()))
}

pub(crate) fn replay_source_bound_cpu_formula_v2<R>(
    retained: &fe2o3_verifier::RetainedProductionConditionalFormulaV2,
    request: &Request<'_>,
    binding: &Binding,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl for<'proof> FnOnce(
        &'proof fe2o3_verifier::ProductionConditionalFormulaExecutionV2,
        &mut Budget<'_>,
    ) -> R,
) -> Result<R, Error> {
    #[cfg(test)]
    composition_tests::on_replay(request, binding, semantic_root, budget);
    #[cfg(test)]
    formula_v2_tests::on_replay(retained, request, binding, semantic_root, budget);
    retained
        .with_replayed_request_v2(
            request,
            native_cpu_input_v1(request, binding, semantic_root),
            budget,
            |execution, budget| Ok(consume(execution, budget)),
        )
        .map_err(|error| Error::ProofExecution(error.to_string()))
}

#[cfg(test)]
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
#[path = "production_conditional_reference_genuine_composition_v1_tests.rs"]
pub(crate) mod composition_tests;

#[cfg(test)]
#[path = "production_conditional_formula_genuine_v2_tests.rs"]
pub(crate) mod formula_v2_tests;

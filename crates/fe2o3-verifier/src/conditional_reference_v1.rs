//! Shared, content-only CPU/source correspondence for conditional requests.
//!
//! Callers own admitted inputs, original resource accounts, source authentication,
//! and formula execution. Successful correspondence grants no ordinary, native,
//! compiler-origin, proof, publication, or launch authority.

use crate::portable_reference_v1::*;
use crate::portable_reference_v1::{
    self as portable, ReferenceFunctionIdentityV1, ReferenceReplayInputV1,
};
use fe2o3_functional_proof::{FunctionalRefinementSubjectsV2, SafeReferenceKindV2};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionConditionalSourceArgumentV1 as Argument,
    ProductionSourceBoundConditionalAggregateRequestV1 as Request,
};
use fe2o3_pliron::{
    ProductionConditionalReadBindingV1 as Read, ProductionEffectRefinementContractV2,
    ProductionNumericalContractV2, ProductionOverflowContractV2, ProductionRankedKernelV1,
    ProductionRankedOperationV1, ProductionRankedValueIdV1, ProductionRankedValueV1,
    ProductionSemanticBinaryOpV2, ProductionSemanticCastV2, ProductionSemanticComparisonV2,
    ProductionSemanticExpressionV2, ProductionSemanticLoadV2, ProductionSemanticScalarTypeV2,
    ProductionSemanticUnaryOpV2,
};
use fe2o3_proof_contracts::DigestV1;

#[doc(hidden)]
pub mod conversion;
mod error;
#[doc(hidden)]
pub mod read_premises;
mod source;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

use conversion::{ReferenceGpuLoadsV2, reference_expression_inner_checked_v2, reference_scalar_v2};
pub use error::ConditionalReferenceErrorV1;
/// Low-level source/adjusted-origin check; not source admission.
#[doc(hidden)]
pub use source::require_read_origins;
use source::{argument, check_outputs, check_source_identity, semantic_expression, visit_loads};

type Error = ConditionalReferenceErrorV1;
type Expr = ProductionSemanticExpressionV2;
type Op = ProductionRankedOperationV1;
type Value = ProductionRankedValueV1;

/// Full existing inert identity tuples and borrowed replay records.
/// Registration and safe-source/helper authenticity remain caller-owned.
pub struct ConditionalReferenceInputV1<'a> {
    /// Existing kernel identity, not authenticated by constructing this view.
    pub kernel: &'a ReferenceFunctionIdentityV1,
    /// Existing reference identity, not authenticated by constructing this view.
    pub reference: &'a ReferenceFunctionIdentityV1,
    /// Original signature, CPU IR, digest and retained output claims.
    pub replay: ReferenceReplayInputV1<'a>,
}

/// Callback-scoped content correspondence, not a proof or lowering owner.
/// Its fields and constructor are private; borrowing it cannot grant ordinary
/// source authority.
///
/// ```compile_fail,E0308
/// use fe2o3_verifier::conditional_reference_v1::SourceBoundCpuCorrespondenceV1;
/// use fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1;
/// fn promote(cpu: &SourceBoundCpuCorrespondenceV1<'_>) -> ProductionFormalMemoryOwnerV1 {
///     cpu
/// }
/// ```
pub struct SourceBoundCpuCorrespondenceV1<'a> {
    request: &'a Request<'a>,
    binding: &'a ConditionalReferenceInputV1<'a>,
}

impl SourceBoundCpuCorrespondenceV1<'_> {
    /// The exact borrowed request checked in this scope.
    pub fn request(&self) -> &Request<'_> {
        self.request
    }

    /// Rechecks subjects immediately before the caller's formula consumption.
    /// This preserves the existing live join's separate debit and check.
    pub fn require_subjects(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        charge(budget, 256)?;
        require(
            self.request.pliron_input().reference_subjects() == subjects(self.binding)?,
            "CPU subjects changed after correspondence",
        )
    }
}

/// Replays CPU content and joins it to the exact source-bound request. Inputs
/// and successful results do not authenticate compiler origin or safe source.
/// The callback runs under the existing replay-owner reservation; no borrowed
/// correspondence can escape.
///
/// ```compile_fail
/// use fe2o3_verifier::conditional_reference_v1::{
///     ConditionalReferenceInputV1, SourceBoundCpuCorrespondenceV1,
///     with_source_bound_cpu_correspondence_v1,
/// };
/// use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'a>(request: &Request<'_>, input: ConditionalReferenceInputV1<'_>,
///     budget: &mut Budget<'_>) -> &'a SourceBoundCpuCorrespondenceV1<'a> {
///     with_source_bound_cpu_correspondence_v1(request, input, 0, budget, |cpu, _| cpu).unwrap()
/// }
/// ```
pub fn with_source_bound_cpu_correspondence_v1<R>(
    request: &Request<'_>,
    input: ConditionalReferenceInputV1<'_>,
    semantic_root: u32,
    budget: &mut Budget<'_>,
    consume: impl for<'checked> FnOnce(
        &'checked SourceBoundCpuCorrespondenceV1<'_>,
        &mut Budget<'_>,
    ) -> R,
) -> Result<R, Error> {
    check_source_identity(request, &input, semantic_root, budget)?;
    portable::with_replayed_output_writes_v1(
        ReferenceReplayInputV1 {
            signature_preimage: input.replay.signature_preimage,
            effect_ir: input.replay.effect_ir,
            effect_ir_sha256: input.replay.effect_ir_sha256,
            observable_output_writes: input.replay.observable_output_writes,
        },
        budget,
        |replay, budget| {
            check_outputs(request, &input, &replay.writes, budget)?;
            read_premises::check_source_bound_reads_v1(request, &input, replay, budget)?;
            let cpu = SourceBoundCpuCorrespondenceV1 {
                request,
                binding: &input,
            };
            Ok(consume(&cpu, budget))
        },
    )
    .map_err(|error| Error::ProofExecution(error.to_string()))?
}

/// Reconstructs the existing descriptive MIR subjects, not source authenticity.
pub fn reference_subjects_v1(
    kernel: &ReferenceFunctionIdentityV1,
    reference: &ReferenceFunctionIdentityV1,
) -> Result<FunctionalRefinementSubjectsV2, Error> {
    FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        DigestV1::from_untrusted_bytes(reference.function_sha256),
        DigestV1::ZERO,
        DigestV1::from_untrusted_bytes(reference.rustc_mir_body_sha256),
        DigestV1::from_untrusted_bytes(kernel.function_sha256),
        DigestV1::from_untrusted_bytes(kernel.rustc_mir_body_sha256),
    )
    .map_err(|error| Error::Subjects(error.to_string()))
}

fn subjects(
    binding: &ConditionalReferenceInputV1<'_>,
) -> Result<FunctionalRefinementSubjectsV2, Error> {
    reference_subjects_v1(binding.kernel, binding.reference)
}

fn reject(why: &'static str) -> Error {
    Error::UnsupportedReference(why)
}

fn charge(budget: &mut Budget<'_>, work: usize) -> Result<(), Error> {
    budget.charge_work(work).map_err(resource)
}

fn resource(error: Resource) -> Error {
    Error::ProofExecution(format!("conditional CPU correspondence resource: {error}"))
}

fn require(condition: bool, why: &'static str) -> Result<(), Error> {
    if condition { Ok(()) } else { Err(reject(why)) }
}

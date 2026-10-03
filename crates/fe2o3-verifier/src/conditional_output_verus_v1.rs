//! Protected proof of a singleton guarded, unsigned-32 output.
//!
//! The live classifier establishes the write domain and unique identity index.
//! The generated theorem proves its coverage under N <= G, and replays the
//! actual coordinate, domain, precondition and value expressions together. The
//! pending launch condition is retained, never converted into TotalView.
//! Compiler extraction/projection and classifier soundness remain trusted;
//! neither the receipt nor its embedded key establishes compiler origin.

use std::{error::Error, fmt};

use fe2o3_functional_proof::{FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2};
use fe2o3_pliron::{
    ProductionConditionalOutputStagingErrorV1, ProductionConditionalOutputStagingV1,
    ProductionMiddleEndEvidenceV5, ProductionRankedKernelLoweringInputV1,
    ProductionRankedOperationV1, ProductionRefinementStagingPolicyV2,
    require_conditional_output_staging_v1,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::functional_refinement_receipt_v2::{
    RetainedImportedFunctionalRefinementReceiptV2,
    execute_and_import_generated_conditional_composition_locally_v1,
    generate_ranked_conditional_effect_formula_v1, ranked_effect_formula_replay_prelude_v2,
};
use crate::{
    CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusExecutionErrorV2,
    FunctionalRefinementVerusRuntimeLeaseV1,
};

pub(crate) const OBLIGATION_DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-GUARDED-OUTPUT-VERUS/V1\0";

/// Move-only conditional proof. Neither its receipt nor its pending condition
/// is accepted as an unconditional aggregate, packed-ABI proof or launch token.
///
/// ```compile_fail
/// use fe2o3_verifier::ProductionConditionalOutputVerusExecutionV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ProductionConditionalOutputVerusExecutionV1>();
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::{ProductionConditionalOutputVerusExecutionV1,
///     ProductionMirPlironPerCompilationVerusExecutionV1};
/// fn cannot_promote(conditional: ProductionConditionalOutputVerusExecutionV1)
///     -> ProductionMirPlironPerCompilationVerusExecutionV1 { conditional }
/// ```
#[derive(Debug)]
#[must_use = "retain the conditional receipt and discharge its actual launch requirements"]
pub struct ProductionConditionalOutputVerusExecutionV1 {
    staging: ProductionConditionalOutputStagingV1,
    source_semantic_identity: [u8; 32],
    ranked_kernel_identity: [u8; 32],
    generated_source_identity: DigestV1,
    binding: FunctionalRefinementBindingV2,
    obligation_preimage: Vec<u8>,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
}

impl ProductionConditionalOutputVerusExecutionV1 {
    pub const fn staging(&self) -> &ProductionConditionalOutputStagingV1 {
        &self.staging
    }
    pub const fn source_semantic_identity(&self) -> &[u8; 32] {
        &self.source_semantic_identity
    }
    pub const fn ranked_kernel_identity(&self) -> &[u8; 32] {
        &self.ranked_kernel_identity
    }
    pub const fn generated_source_identity(&self) -> DigestV1 {
        self.generated_source_identity
    }
    pub const fn binding(&self) -> FunctionalRefinementBindingV2 {
        self.binding
    }
    /// Exact signed-obligation input retained for later conditional transport.
    /// This is not an independently admitted or unconditional evidence codec.
    pub fn obligation_preimage(&self) -> &[u8] {
        &self.obligation_preimage
    }
    pub const fn boundary(&self) -> FunctionalRefinementBoundaryV2 {
        self.retained.proof().boundary()
    }
    pub const fn signed_receipt_wire(&self) -> &[u8] {
        self.retained.wire()
    }
    pub const fn receipt_verifying_key(&self) -> &[u8; 32] {
        self.retained.verifying_key()
    }
    pub const fn retains_strictly_imported_signed_receipt(&self) -> bool {
        self.retained.proof().signature_and_policy_verified()
    }
    pub const fn requires_packed_extent_and_launch_discharge(&self) -> bool {
        true
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionConditionalOutputVerusErrorV1 {
    Staging(ProductionConditionalOutputStagingErrorV1),
    UnsupportedProfile,
    RetainedReceiptMismatch,
    GeneratedSource(String),
    Execution(FunctionalRefinementVerusExecutionErrorV2),
}

impl fmt::Display for ProductionConditionalOutputVerusErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Staging(error) => error.fmt(f),
            Self::UnsupportedProfile => f.write_str("conditional aggregate requires one guarded unsigned-32 identity output"),
            Self::RetainedReceiptMismatch => f.write_str("conditional aggregate effect receipt or compiler subjects differ from retained staging"),
            Self::GeneratedSource(error) => write!(f, "conditional aggregate source: {error}"),
            Self::Execution(error) => error.fmt(f),
        }
    }
}

impl Error for ProductionConditionalOutputVerusErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Staging(error) => Some(error),
            Self::Execution(error) => Some(error),
            _ => None,
        }
    }
}

/// Derives source and subjects only from the exact retained ranked owner. The
/// proof is universal over N and G; it does not inspect or admit a host launch.
pub fn execute_conditional_output_per_compilation_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    timeout_seconds: u32,
) -> Result<
    (
        ProductionConditionalOutputVerusExecutionV1,
        ProductionRefinementStagingPolicyV2,
    ),
    ProductionConditionalOutputVerusErrorV1,
> {
    use ProductionConditionalOutputVerusErrorV1 as E;
    let staging = require_conditional_output_staging_v1(ranked, evidence).map_err(E::Staging)?;
    if staging.coverage().element_width() != 32 {
        return Err(E::UnsupportedProfile);
    }
    let mut effects = ranked
        .kernel()
        .blocks()
        .iter()
        .enumerate()
        .flat_map(|(block, body)| {
            body.operations()
                .iter()
                .enumerate()
                .filter_map(move |(operation, op)| match op {
                    ProductionRankedOperationV1::RequireEffectRefinement { proof, .. } => {
                        Some((block, operation, proof))
                    }
                    _ => None,
                })
        });
    let (block, operation, proof) = effects.next().ok_or(E::UnsupportedProfile)?;
    if effects.next().is_some() {
        return Err(E::UnsupportedProfile);
    }
    let subjects = proof.binding().subjects();
    let receipts = ranked.retained_policy_checked_refinement_staging();
    if receipts.len() != 1
        || receipts.iter().any(|receipt| {
            receipt.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToKernelMir
                || receipt.binding().subjects() != subjects
        })
        || receipts
            .iter()
            .filter(|receipt| {
                receipt.receipt_identity() == proof.receipt_identity()
                    && receipt.binding() == proof.binding()
            })
            .count()
            != 1
    {
        return Err(E::RetainedReceiptMismatch);
    }
    let lemma = generate_ranked_conditional_effect_formula_v1(ranked.kernel(), block, operation)
        .map_err(E::Execution)?;
    let source = CanonicalGeneratedVerusProofInputV3::new(
        format!(
            "use vstd::prelude::*;\nverus! {{\n{}\n{lemma}\n}}\n",
            ranked_effect_formula_replay_prelude_v2(),
        )
        .into_bytes(),
    )
    .map_err(|error| E::GeneratedSource(error.to_string()))?;
    let generated_source_identity = DigestV1::from_untrusted_bytes(source.identity().as_bytes());
    let (obligation, obligation_preimage) = obligation_identity(
        &staging,
        evidence,
        ranked,
        block,
        operation,
        generated_source_identity,
    );
    let binding = FunctionalRefinementBindingV2::from_subjects(subjects, obligation)
        .map_err(|error| E::GeneratedSource(error.to_string()))?;
    let (retained, policy) = execute_and_import_generated_conditional_composition_locally_v1(
        runtime,
        source,
        binding,
        timeout_seconds,
    )
    .map_err(E::Execution)?;
    let imported = retained.proof();
    if imported.binding() != binding
        || imported.boundary()
            != FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePlironConditionalCoverage
        || !imported.signature_and_policy_verified()
        || !policy.accepts_signer(imported.signer_identity())
        || policy.toolchain() != imported.toolchain()
    {
        return Err(E::RetainedReceiptMismatch);
    }
    Ok((
        ProductionConditionalOutputVerusExecutionV1 {
            staging,
            source_semantic_identity: *evidence.source_semantic_identity(),
            ranked_kernel_identity: *evidence.ranked_kernel_identity(),
            generated_source_identity,
            binding,
            obligation_preimage,
            retained,
        },
        policy,
    ))
}

fn obligation_identity(
    staging: &ProductionConditionalOutputStagingV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    ranked: &ProductionRankedKernelLoweringInputV1,
    block: usize,
    operation: usize,
    source: DigestV1,
) -> (DigestV1, Vec<u8>) {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(OBLIGATION_DOMAIN);
    for identity in [
        staging.evidence_identity(),
        evidence.source_semantic_identity(),
        evidence.ranked_kernel_identity(),
        source.as_bytes(),
    ] {
        bytes.extend_from_slice(identity);
    }
    let coverage = staging.coverage();
    bytes.extend_from_slice(&(coverage.view_name().len() as u64).to_le_bytes());
    bytes.extend_from_slice(coverage.view_name().as_bytes());
    for location in [
        coverage.view_definition(),
        coverage.contract_location(),
        coverage.guard_location(),
        coverage.write_location(),
    ] {
        bytes.extend_from_slice(&(location.block() as u64).to_le_bytes());
        bytes.extend_from_slice(&(location.operation() as u64).to_le_bytes());
    }
    for value in [
        coverage.ranked_extent_argument() as u64,
        u64::from(staging.reference_output_argument()),
        coverage.allocation_origin(),
        coverage.noalias_class(),
        u64::from(coverage.element_width()),
        coverage.grid_identity(),
        coverage.static_global_x_extent().unwrap_or(0),
        coverage.workgroup_extents()[0],
        coverage.workgroup_extents()[1],
        coverage.workgroup_extents()[2],
        coverage.subgroup_size(),
        u64::from(coverage.requires_full_physical_workgroups()),
        block as u64,
        operation as u64,
        ranked.retained_policy_checked_refinement_staging().len() as u64,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    for receipt in ranked.retained_policy_checked_refinement_staging() {
        let subjects = receipt.binding().subjects();
        let toolchain = receipt.toolchain();
        for identity in [
            subjects.safe_reference_identity(),
            subjects.safe_reference_source_hash(),
            subjects.safe_reference_mir_hash(),
            subjects.kernel_subject_identity(),
            subjects.kernel_mir_hash(),
            receipt.binding().normalized_obligation_effect_ir_hash(),
            receipt.receipt_identity().digest(),
            receipt.signer_identity(),
            receipt.execution_identity(),
            toolchain.verus_executable(),
            toolchain.verus_configuration(),
            toolchain.solver_executable(),
            toolchain.solver_configuration(),
            toolchain.runtime_closure(),
        ] {
            bytes.extend_from_slice(identity.as_bytes());
        }
    }
    (
        DigestV1::from_untrusted_bytes(Sha256::digest(&bytes).into()),
        bytes,
    )
}

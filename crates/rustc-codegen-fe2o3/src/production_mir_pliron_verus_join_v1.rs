//! Compiler-private retained-runtime join for aggregate MIR/PLIRON conditional lemmas.

use std::{error::Error, fmt};

use fe2o3_functional_proof::{MirPlironSemanticContractV1, ParallelReferenceContractV1};
use fe2o3_pliron::{
    ProductionMiddleEndEvidenceV5, ProductionMirPlironSemanticContractReportV1,
    ProductionParallelReferenceContractReportV1, ProductionRankedKernelLoweringInputV1,
};
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1,
    ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
    ProductionMirPlironPerCompilationVerusErrorV1, ProductionMirPlironPerCompilationVerusReportV1,
    execute_effect_ir_derived_mir_pliron_semantic_contract_per_compilation_v3,
};

const RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
const PER_COMPILATION_PROOF_TIMEOUT_SECONDS_V1: u32 = 120;

/// Compiler-owned aggregate proof and its ephemeral import policy.
#[must_use = "dropping this value abandons authenticated conditional-composition evidence"]
pub(crate) struct AuthenticatedMirPlironPerCompilationVerificationV2 {
    verified: ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
}

impl AuthenticatedMirPlironPerCompilationVerificationV2 {
    pub(crate) const fn report(&self) -> ProductionMirPlironPerCompilationVerusReportV1 {
        self.verified.per_compilation_verus_report()
    }

    pub(crate) const fn verified(
        &self,
    ) -> &ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3 {
        &self.verified
    }

    pub(crate) fn into_verified(self) -> ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3 {
        self.verified
    }
}

/// Production integration point after mandatory middle-end evidence and exact
/// semantic-contract reconciliation, and before target-neutral lowering.
pub(crate) fn authenticate_mir_pliron_contract_per_compilation_v2(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    contract: &MirPlironSemanticContractV1,
    structural_report: ProductionMirPlironSemanticContractReportV1,
    parallel_contract: &ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
) -> Result<AuthenticatedMirPlironPerCompilationVerificationV2, ProductionMirPlironVerusJoinErrorV1>
{
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1,
    )
    .map_err(
        |error| ProductionMirPlironVerusJoinErrorV1::RuntimeUnavailable {
            root: RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1,
            detail: error.to_string(),
        },
    )?;
    let verified = execute_effect_ir_derived_mir_pliron_semantic_contract_per_compilation_v3(
        &runtime,
        ranked,
        evidence,
        contract,
        structural_report,
        parallel_contract,
        parallel_report,
        PER_COMPILATION_PROOF_TIMEOUT_SECONDS_V1,
    )
    .map_err(ProductionMirPlironVerusJoinErrorV1::Verification)?;
    Ok(AuthenticatedMirPlironPerCompilationVerificationV2 { verified })
}

#[derive(Debug)]
pub(crate) enum ProductionMirPlironVerusJoinErrorV1 {
    RuntimeUnavailable { root: &'static str, detail: String },
    Verification(ProductionMirPlironPerCompilationVerusErrorV1),
}

impl fmt::Display for ProductionMirPlironVerusJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeUnavailable { root, detail } => write!(
                formatter,
                "per-compilation MIR/PLIRON proof runtime is unavailable at {root}: {detail}",
            ),
            Self::Verification(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionMirPlironVerusJoinErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RuntimeUnavailable { .. } => None,
            Self::Verification(error) => Some(error),
        }
    }
}

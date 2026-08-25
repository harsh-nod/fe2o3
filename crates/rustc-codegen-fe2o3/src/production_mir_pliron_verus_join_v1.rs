//! Compiler-private retained-runtime join for aggregate MIR/PLIRON conditional lemmas.

use std::{error::Error, fmt};

use fe2o3_functional_proof::{MirPlironSemanticContractV1, ParallelReferenceContractV1};
use fe2o3_pliron::{
    ProductionFunctionalRefinementTrustPolicyV2, ProductionMiddleEndEvidenceV5,
    ProductionMirPlironSemanticContractReportV1, ProductionParallelReferenceContractReportV1,
    ProductionRankedKernelLoweringInputV1,
};
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1, ProductionMirPlironPerCompilationVerusErrorV1,
    ProductionMirPlironPerCompilationVerusReportV1,
    execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1,
};

const RETAINED_FUNCTIONAL_REFINEMENT_RUNTIME_ROOT_V1: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";
const PER_COMPILATION_PROOF_TIMEOUT_SECONDS_V1: u32 = 120;

/// Compiler-owned aggregate proof and its ephemeral import policy.
#[must_use = "dropping this value abandons authenticated conditional-composition evidence"]
pub(crate) struct AuthenticatedMirPlironPerCompilationVerificationV1 {
    report: ProductionMirPlironPerCompilationVerusReportV1,
    _policy: ProductionFunctionalRefinementTrustPolicyV2,
}

impl AuthenticatedMirPlironPerCompilationVerificationV1 {
    pub(crate) const fn report(&self) -> ProductionMirPlironPerCompilationVerusReportV1 {
        self.report
    }
}

/// Production integration point after mandatory middle-end evidence and exact
/// semantic-contract reconciliation, and before target-neutral lowering.
pub(crate) fn authenticate_mir_pliron_contract_per_compilation_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    contract: &MirPlironSemanticContractV1,
    structural_report: ProductionMirPlironSemanticContractReportV1,
    parallel_contract: &ParallelReferenceContractV1,
    parallel_report: ProductionParallelReferenceContractReportV1,
) -> Result<AuthenticatedMirPlironPerCompilationVerificationV1, ProductionMirPlironVerusJoinErrorV1>
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
    let (report, policy) = execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1(
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
    Ok(AuthenticatedMirPlironPerCompilationVerificationV1 {
        report,
        _policy: policy,
    })
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

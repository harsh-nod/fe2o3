//! Compiler-private retained-runtime join for aggregate MIR/PLIRON conditional lemmas.

use std::{error::Error, fmt};

use fe2o3_functional_proof::{MirPlironSemanticContractV1, ParallelReferenceContractV1};
use fe2o3_pliron::{
    ProductionMiddleEndEvidenceV5, ProductionMirPlironSemanticContractReportV1,
    ProductionParallelReferenceContractReportV1, ProductionRankedKernelLoweringInputV1,
    ProductionRefinementStagingPolicyV2,
};
use fe2o3_verifier::{
    FunctionalRefinementVerusRuntimeLeaseV1, ProductionMirPlironPerCompilationVerusErrorV1,
    ProductionMirPlironPerCompilationVerusExecutionV1,
    ProductionMirPlironPerCompilationVerusReportV1,
    execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1,
};

const PER_COMPILATION_PROOF_TIMEOUT_SECONDS_V1: u32 = 120;

#[derive(Debug)]
pub(crate) struct AuthenticatedConditionalOutputVerificationV1 {
    execution: fe2o3_verifier::ProductionConditionalOutputVerusExecutionV1,
    _staging_policy: ProductionRefinementStagingPolicyV2,
}

impl AuthenticatedConditionalOutputVerificationV1 {
    pub(crate) const fn execution(
        &self,
    ) -> &fe2o3_verifier::ProductionConditionalOutputVerusExecutionV1 {
        &self.execution
    }
}

pub(crate) fn authenticate_conditional_output_per_compilation_v1(
    ranked: &ProductionRankedKernelLoweringInputV1,
    evidence: &ProductionMiddleEndEvidenceV5,
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
) -> Result<AuthenticatedConditionalOutputVerificationV1, ProductionMirPlironVerusJoinErrorV1> {
    let (execution, policy) = fe2o3_verifier::execute_conditional_output_per_compilation_v1(
        runtime,
        ranked,
        evidence,
        PER_COMPILATION_PROOF_TIMEOUT_SECONDS_V1,
    )
    .map_err(ProductionMirPlironVerusJoinErrorV1::ConditionalVerification)?;
    Ok(AuthenticatedConditionalOutputVerificationV1 {
        execution,
        _staging_policy: policy,
    })
}

/// Compiler-owned aggregate proof and its ephemeral import policy.
#[must_use = "dropping this value abandons authenticated conditional-composition evidence"]
pub(crate) struct AuthenticatedMirPlironPerCompilationVerificationV1 {
    execution: ProductionMirPlironPerCompilationVerusExecutionV1,
    _staging_policy: ProductionRefinementStagingPolicyV2,
}

impl AuthenticatedMirPlironPerCompilationVerificationV1 {
    pub(crate) const fn report(&self) -> ProductionMirPlironPerCompilationVerusReportV1 {
        self.execution.report()
    }

    pub(crate) const fn execution(&self) -> &ProductionMirPlironPerCompilationVerusExecutionV1 {
        &self.execution
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
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
) -> Result<AuthenticatedMirPlironPerCompilationVerificationV1, ProductionMirPlironVerusJoinErrorV1>
{
    let (execution, policy) = execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1(
        runtime,
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
        execution,
        _staging_policy: policy,
    })
}

#[derive(Debug)]
pub(crate) enum ProductionMirPlironVerusJoinErrorV1 {
    Verification(ProductionMirPlironPerCompilationVerusErrorV1),
    ConditionalVerification(fe2o3_verifier::ProductionConditionalOutputVerusErrorV1),
}

impl fmt::Display for ProductionMirPlironVerusJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(error) => error.fmt(formatter),
            Self::ConditionalVerification(error) => error.fmt(formatter),
        }
    }
}

impl Error for ProductionMirPlironVerusJoinErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Verification(error) => Some(error),
            Self::ConditionalVerification(error) => Some(error),
        }
    }
}

//! Compiler-owned generation and execution of proofs over live Pliron state.
//!
//! This integration layer may consume live middle-end objects. The independent
//! verifier consumes only canonical inputs and signed execution evidence.

#![forbid(unsafe_code)]

mod functional_refinement_receipt_v2;
mod mir_pliron_per_compilation_verus_v1;

pub use functional_refinement_receipt_v2::{
    FunctionalRefinementVerusExecutionErrorKindV2, FunctionalRefinementVerusExecutionErrorV2,
    MAX_FUNCTIONAL_REFINEMENT_FORMULA_DEPTH_V2, MAX_FUNCTIONAL_REFINEMENT_FORMULA_EDGES_V2,
    MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2, MAX_FUNCTIONAL_REFINEMENT_FORMULA_WORK_V2,
    MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
    MAX_FUNCTIONAL_REFINEMENT_VERUS_TIMEOUT_SECONDS_V2, PreparedFunctionalRefinementReceiptV2,
    execute_and_import_ranked_functional_refinement_locally_v2,
    functional_refinement_verus_toolchain_identity_v2,
    prepare_ranked_functional_refinement_receipt_v2,
};
pub use mir_pliron_per_compilation_verus_v1::{
    MAX_PRODUCTION_AGGREGATE_EFFECT_FORMULA_OUTPUTS_V1,
    ProductionMirPlironPerCompilationVerusErrorV1,
    ProductionMirPlironPerCompilationVerusExecutionV1,
    ProductionMirPlironPerCompilationVerusReportV1, ProductionVerusVerifiedMirPlironKernelV1,
    execute_mir_pliron_semantic_contract_per_compilation_borrowed_v1,
    execute_mir_pliron_semantic_contract_per_compilation_v1,
};

impl fe2o3_verifier::ProductionMirPlironVerusExecutionViewV1
    for ProductionMirPlironPerCompilationVerusExecutionV1
{
    fn contract_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().contract_identity()
    }
    fn parallel_contract_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().parallel_contract_identity()
    }
    fn pliron_evidence_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().pliron_evidence_identity()
    }
    fn composition_template_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().composition_template_identity()
    }
    fn generated_source_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().generated_source_identity()
    }
    fn obligation_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().obligation_identity()
    }
    fn binding(&self) -> fe2o3_functional_proof::FunctionalRefinementBindingV2 {
        self.report().binding()
    }
    fn signer_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().signer_identity()
    }
    fn toolchain(&self) -> fe2o3_functional_proof::VerusToolchainIdentityV2 {
        self.report().toolchain()
    }
    fn execution_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().execution_identity()
    }
    fn receipt_identity(&self) -> fe2o3_proof_contracts::DigestV1 {
        self.report().receipt_identity().digest()
    }
    fn retained_policy_checked_staging(&self) -> u64 {
        self.report().retained_policy_checked_staging()
    }
    fn receipt_verifying_key(&self) -> &[u8; 32] {
        self.receipt_verifying_key()
    }
    fn signed_receipt_wire(&self) -> &[u8] {
        self.signed_receipt_wire()
    }
}

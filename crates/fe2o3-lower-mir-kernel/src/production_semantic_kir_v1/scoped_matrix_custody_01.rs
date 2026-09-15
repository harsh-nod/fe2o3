// Source-SSA use custody only; never a KIR or numerical refinement witness.
mod scoped_matrix_custody_01 {
    use super::capability_ssa_graph_01::{
        CapabilityDefinitionSiteV1 as Site, CapabilityLoanV1 as Loan, CapabilitySsaGraphV1 as Graph,
    };
    use super::*;
    use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticDefinedCapabilityContractV1, SemanticDefinedMatrixIdentityV1,
        SemanticPolicyGfx950NarrowV1, SemanticPolicyMatrixBindV1,
    };
    use fe2o3_mir_model::{
        SemanticCallInstanceIdV1, SemanticExpandedStatementOriginV1,
        SemanticExpandedTerminatorOriginV1,
    };

    type Result<T> = std::result::Result<T, ProductionSemanticKirErrorV1>;
    fn reject(detail: &'static str) -> ProductionSemanticKirErrorV1 {
        unsupported(0, None, None, detail)
    }
    fn mismatch() -> ProductionSemanticKirErrorV1 {
        ProductionSemanticKirErrorV1::CorrespondenceMismatch
    }

    mod bf16;
    mod bf16_constructor;
    mod occurrences;
    mod relation;
    mod resolve;
    mod session;
    mod typed_uses;
    mod typed_sites;
    mod typed_flow;
    mod typed_inventory;
    mod old_epoch;
    mod old_epoch_seeds;
    pub use typed_uses::ProductionTransposeOwnedSourceUsesV1;
    pub use bf16::{ProductionScopedBf16LaneRelationV1, ProductionScopedBf16LaneUseV1};
    pub use relation::{ProductionScopedMatrixUseRelationV1, ProductionScopedMatrixUseV1};
    pub use session::ProductionScopedMatrixSourceSessionV1;

    #[cfg(test)]
    mod tests;
}
pub use scoped_matrix_custody_01::{
    ProductionScopedBf16LaneRelationV1, ProductionScopedBf16LaneUseV1,
    ProductionScopedMatrixSourceSessionV1, ProductionScopedMatrixUseRelationV1,
    ProductionScopedMatrixUseV1, ProductionTransposeOwnedSourceUsesV1,
};

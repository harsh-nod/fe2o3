//! Original source facts for the closed epoch-bound transpose issuer.
//! Canonical source replay, not a physical LDS allocation receipt.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticGfx950TransposeContractV1, SemanticGfx950TransposeOperationV1,
};

#[path = "gfx950_transpose_v1/issuer/body_auth.rs"]
mod body_auth;
#[path = "gfx950_transpose_v1/issuer/signature.rs"]
mod signature;
#[path = "gfx950_transpose_v1/issuer/transactions.rs"]
mod transactions;

#[cfg(test)]
#[path = "gfx950_transpose_v1/terminal_tests.rs"]
mod terminal_tests;

/// Private facts returned only after source, body, ABI and root checks.
/// The canonical builder preserves every retained source edge.
pub(super) struct IssuerSourceV1 {
    pub(super) partition_reference: SemanticTypeIdV1,
    pub(super) partition: SemanticTypeIdV1,
    pub(super) tile: SemanticTypeIdV1,
    pub(super) signature: SemanticExecutionCapabilitySignatureV1,
    pub(super) format: SemanticGfx950LdsTransposeFormatV1,
    pub(super) execution_brand: SemanticTypeIdentityV1,
    pub(super) subgroup_brand: SemanticTypeIdentityV1,
    pub(super) epoch: SemanticTypeIdentityV1,
    pub(super) provenance: SemanticKernelCapabilityProvenanceV1,
    pub(super) source_identity: SemanticFunctionIdentityV1,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    terminal: crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: &AuthenticatedProductionKernelContextRootV1,
    source_identity: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionExecutionTerminalV1 as Terminal;
    if terminal != Terminal::Gfx950TransposeIssue {
        return transactions::operation(
            tcx,
            instance,
            terminal,
            abi,
            types,
            root,
            source_identity,
            contexts,
        );
    }
    let facts = observe_issuer(
        tcx,
        instance,
        abi,
        types,
        Some(root),
        source_identity,
        contexts,
    )?;
    let transpose = SemanticGfx950TransposeContractV1::new(
        SemanticGfx950TransposeOperationV1::Issue {
            partition_reference: facts.partition_reference,
            partition: facts.partition,
            tile: facts.tile,
        },
        facts.format,
        facts.subgroup_brand,
        None,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    let contract = SemanticExecutionCapabilityContractV1::new(
        SemanticExecutionCapabilityOperationV1::Gfx950Transpose(transpose),
        facts.signature,
        facts.provenance,
        facts.execution_brand,
        facts.epoch,
        None,
        facts.source_identity,
    )
    .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?;
    Ok(SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn observe_issuer<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
    root: Option<&AuthenticatedProductionKernelContextRootV1>,
    source_identity: SemanticFunctionIdentityV1,
    contexts: &AuthenticatedProductionKernelContextsV1,
) -> Result<IssuerSourceV1, ProductionSemanticImportErrorV1> {
    let rejected = |detail| ProductionSemanticImportErrorV1::KernelContextBinding(detail);
    let root = root.ok_or_else(|| rejected("LDS transpose issuer lost authenticated root"))?;
    let observed = signature::observe_signature_layout(tcx, instance, root, abi, types)?;
    if observed.source_identity != source_identity
        || observed.instance != instance
        || !body_auth::observe_body(
            tcx,
            instance,
            tcx.instance_mir(instance.def),
            observed.partition_reference,
            observed.tile,
        )
    {
        return Err(rejected(
            "LDS transpose issuer changed original source identity or body",
        ));
    }
    let [partition_reference] = abi.source_input_types() else {
        return Err(rejected(
            "LDS transpose issuer changed canonical source arity",
        ));
    };
    let partition_reference = *partition_reference;
    let partition = pointer_pointee_v1(types, partition_reference)?;
    let tile = abi.source_output_type();
    // ABI validation above checked the complete input/output types. Resolve the
    // pointee directly through that edge; do not rescan the global type roster.
    if types
        .get(partition_reference.index() as usize)
        .is_none_or(|ty| ty.identity() != rustc_type_identity_v1(tcx, observed.partition_reference))
        || types
            .get(partition.index() as usize)
            .is_none_or(|ty| ty.identity() != rustc_type_identity_v1(tcx, observed.partition))
        || types
            .get(tile.index() as usize)
            .is_none_or(|ty| ty.identity() != rustc_type_identity_v1(tcx, observed.tile))
    {
        return Err(rejected(
            "LDS transpose issuer changed canonical reference or result edge",
        ));
    }
    Ok(IssuerSourceV1 {
        partition_reference,
        partition,
        tile,
        signature: SemanticExecutionCapabilitySignatureV1::new(&[partition_reference], tile)
            .map_err(ProductionSemanticImportErrorV1::SemanticSchema)?,
        format: observed.format,
        execution_brand: rustc_type_identity_v1(tcx, observed.execution_brand),
        subgroup_brand: rustc_type_identity_v1(tcx, observed.subgroup_brand),
        epoch: rustc_type_identity_v1(tcx, observed.epoch),
        provenance: capability_memory_provenance_v1(root, contexts)?,
        source_identity,
    })
}

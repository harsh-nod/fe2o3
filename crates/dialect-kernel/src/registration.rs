use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AccessKindAttr, AlgorithmOp, AlgorithmType, AllocationEffectOp, AllocationOriginAttr,
    AnalysisSplitControlCountAttr, AnalysisSplitOp, AtomicOrderingAttr, AtomicScopeAttr,
    BranchArgsOp, BranchOp, CheckedTiledIndex2DOp, DIALECT_NAME, DeterministicJoinOp,
    DimensionAttr, DimensionOp, GeneralGemmAbiSchemaAttr, GeneralGemmEpilogueSchemaAttr,
    GeneralGemmOp, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp, IndexEqualBranchArgsOp,
    IndexEqualBranchOp, IndexLessThanBranchArgsOp, IndexLessThanBranchOp, IndexType,
    IndexUnknownOp, IndexValueAttr, InvocationDimensionAttr, InvocationIndexOp,
    IterationDomainAttr, LaunchExtentAttr, MemorySpaceAttr, NoAliasClassAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, RequireEquivalentOp, ReturnOp, SemanticBinaryKindAttr,
    SemanticBinaryOp, SemanticConstantAttr, SemanticConstantOp, SemanticScalarType,
    SemanticSymbolAttr, SemanticSymbolOp, TensorConvergenceAttr, TensorFragmentAttr,
    TensorInstructionAttr, TensorLayoutOp, TrapOp,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<AlgorithmType>()?;
    service.register_attribute::<IterationDomainAttr>()?;
    service.register_attribute::<GeneralGemmAbiSchemaAttr>()?;
    service.register_attribute::<GeneralGemmEpilogueSchemaAttr>()?;
    service.register_type::<RankedViewType>()?;
    service.register_type::<IndexType>()?;
    service.register_type::<SemanticScalarType>()?;
    service.register_attribute::<IndexValueAttr>()?;
    service.register_attribute::<DimensionAttr>()?;
    service.register_attribute::<AccessKindAttr>()?;
    service.register_attribute::<AtomicOrderingAttr>()?;
    service.register_attribute::<AtomicScopeAttr>()?;
    service.register_attribute::<MemorySpaceAttr>()?;
    service.register_attribute::<AllocationOriginAttr>()?;
    service.register_attribute::<NoAliasClassAttr>()?;
    service.register_attribute::<InvocationDimensionAttr>()?;
    service.register_attribute::<LaunchExtentAttr>()?;
    service.register_attribute::<AnalysisSplitControlCountAttr>()?;
    service.register_attribute::<IndexBinaryKindAttr>()?;
    service.register_attribute::<SemanticSymbolAttr>()?;
    service.register_attribute::<SemanticConstantAttr>()?;
    service.register_attribute::<SemanticBinaryKindAttr>()?;
    service.register_attribute::<TensorConvergenceAttr>()?;
    service.register_attribute::<TensorInstructionAttr>()?;
    service.register_attribute::<TensorFragmentAttr>()?;
    service.register_operation::<AlgorithmOp>()?;
    service.register_operation::<GeneralGemmOp>()?;
    service.register_operation::<RankedViewOp>()?;
    service.register_operation::<IndexConstantOp>()?;
    service.register_operation::<IndexUnknownOp>()?;
    service.register_operation::<InvocationIndexOp>()?;
    service.register_operation::<IndexBinaryOp>()?;
    service.register_operation::<DeterministicJoinOp>()?;
    service.register_operation::<CheckedTiledIndex2DOp>()?;
    service.register_operation::<DimensionOp>()?;
    service.register_operation::<RankedAccessOp>()?;
    service.register_operation::<AllocationEffectOp>()?;
    service.register_operation::<IndexLessThanBranchOp>()?;
    service.register_operation::<IndexLessThanBranchArgsOp>()?;
    service.register_operation::<IndexEqualBranchOp>()?;
    service.register_operation::<IndexEqualBranchArgsOp>()?;
    service.register_operation::<AnalysisSplitOp>()?;
    service.register_operation::<BranchOp>()?;
    service.register_operation::<BranchArgsOp>()?;
    service.register_operation::<ReturnOp>()?;
    service.register_operation::<TrapOp>()?;
    service.register_operation::<SemanticSymbolOp>()?;
    service.register_operation::<SemanticConstantOp>()?;
    service.register_operation::<SemanticBinaryOp>()?;
    service.register_operation::<RequireEquivalentOp>()?;
    service.register_operation::<TensorLayoutOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

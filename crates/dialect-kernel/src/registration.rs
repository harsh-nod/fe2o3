use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AccessKindAttr, AlgorithmOp, AlgorithmType, AllocationEffectOp, AllocationOriginAttr,
    AnalysisSplitControlCountAttr, AnalysisSplitOp, AtomicOrderingAttr, AtomicScopeAttr,
    BranchArgsOp, BranchOp, CheckedRowStripedIndex2DOp, CheckedTiledIndex2DOp, DIALECT_NAME,
    DeterministicJoinOp, DimensionAttr, DimensionOp, IndexBinaryKindAttr, IndexBinaryOp,
    IndexConstantOp, IndexEqualBranchArgsOp, IndexEqualBranchOp, IndexLessThanBranchArgsOp,
    IndexLessThanBranchOp, IndexType, IndexUnknownOp, IndexUnsignedCastOp, IndexValueAttr,
    InvocationDimensionAttr, InvocationIndexOp, IterationDomainAttr, LaunchExtentAttr,
    MemorySpaceAttr, NoAliasClassAttr, OwnershipContractOp, OwnershipCoverageAttr,
    OwnershipPartitionAttr, RankedAccessOp, RankedViewOp, RankedViewType, RequireEquivalentOp,
    RequireFiniteFoldOp, RequireFiniteRecurrenceOp, RequirePermutationGatherOp, ReturnOp,
    SemanticBinaryKindAttr, SemanticBinaryOp, SemanticConstantAttr, SemanticConstantOp,
    SemanticCoverageBindingAttr, SemanticDomainBoundAttr, SemanticEvaluationOrderAttr,
    SemanticExceptionalValueAttr, SemanticExpressionCommitmentAttr, SemanticExpressionCommitmentOp,
    SemanticIeeeRoundingAttr, SemanticNumericalPolicyAttr, SemanticOverflowAttr,
    SemanticScalarKindAttr, SemanticScalarType, SemanticStepBoundAttr, SemanticSymbolAttr,
    SemanticSymbolOp, SemanticTypedBinaryKindAttr, SemanticTypedBinaryOp,
    SemanticTypedCastKindAttr, SemanticTypedCastOp, SemanticTypedCompareKindAttr,
    SemanticTypedCompareOp, SemanticTypedConstantOp, SemanticTypedExpressionRootOp,
    SemanticTypedSelectOp, SemanticTypedSymbolOp, SemanticTypedUnaryKindAttr, SemanticTypedUnaryOp,
    TensorConvergenceAttr, TensorFragmentAttr, TensorInstructionAttr, TensorLayoutOp,
    TensorResultComponentOp, TensorValueRootAttr, TrapOp,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<AlgorithmType>()?;
    service.register_attribute::<IterationDomainAttr>()?;
    service.register_type::<RankedViewType>()?;
    service.register_type::<IndexType>()?;
    service.register_type::<SemanticScalarType>()?;
    service.register_attribute::<SemanticScalarKindAttr>()?;
    service.register_attribute::<SemanticTypedUnaryKindAttr>()?;
    service.register_attribute::<SemanticTypedBinaryKindAttr>()?;
    service.register_attribute::<SemanticOverflowAttr>()?;
    service.register_attribute::<SemanticTypedCompareKindAttr>()?;
    service.register_attribute::<SemanticTypedCastKindAttr>()?;
    service.register_attribute::<SemanticNumericalPolicyAttr>()?;
    service.register_attribute::<SemanticIeeeRoundingAttr>()?;
    service.register_attribute::<SemanticExceptionalValueAttr>()?;
    service.register_attribute::<IndexValueAttr>()?;
    service.register_attribute::<DimensionAttr>()?;
    service.register_attribute::<AccessKindAttr>()?;
    service.register_attribute::<AtomicOrderingAttr>()?;
    service.register_attribute::<AtomicScopeAttr>()?;
    service.register_attribute::<MemorySpaceAttr>()?;
    service.register_attribute::<AllocationOriginAttr>()?;
    service.register_attribute::<NoAliasClassAttr>()?;
    service.register_attribute::<OwnershipCoverageAttr>()?;
    service.register_attribute::<OwnershipPartitionAttr>()?;
    service.register_attribute::<InvocationDimensionAttr>()?;
    service.register_attribute::<LaunchExtentAttr>()?;
    service.register_attribute::<AnalysisSplitControlCountAttr>()?;
    service.register_attribute::<IndexBinaryKindAttr>()?;
    service.register_attribute::<SemanticSymbolAttr>()?;
    service.register_attribute::<SemanticConstantAttr>()?;
    service.register_attribute::<SemanticExpressionCommitmentAttr>()?;
    service.register_attribute::<SemanticBinaryKindAttr>()?;
    service.register_attribute::<SemanticDomainBoundAttr>()?;
    service.register_attribute::<SemanticStepBoundAttr>()?;
    service.register_attribute::<SemanticEvaluationOrderAttr>()?;
    service.register_attribute::<SemanticCoverageBindingAttr>()?;
    service.register_attribute::<TensorConvergenceAttr>()?;
    service.register_attribute::<TensorInstructionAttr>()?;
    service.register_attribute::<TensorFragmentAttr>()?;
    service.register_attribute::<TensorValueRootAttr>()?;
    service.register_operation::<AlgorithmOp>()?;
    service.register_operation::<RankedViewOp>()?;
    service.register_operation::<IndexConstantOp>()?;
    service.register_operation::<IndexUnknownOp>()?;
    service.register_operation::<InvocationIndexOp>()?;
    service.register_operation::<IndexUnsignedCastOp>()?;
    service.register_operation::<IndexBinaryOp>()?;
    service.register_operation::<DeterministicJoinOp>()?;
    service.register_operation::<CheckedTiledIndex2DOp>()?;
    service.register_operation::<CheckedRowStripedIndex2DOp>()?;
    service.register_operation::<DimensionOp>()?;
    service.register_operation::<RankedAccessOp>()?;
    service.register_operation::<OwnershipContractOp>()?;
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
    service.register_operation::<SemanticExpressionCommitmentOp>()?;
    service.register_operation::<SemanticBinaryOp>()?;
    service.register_operation::<SemanticTypedSymbolOp>()?;
    service.register_operation::<TensorResultComponentOp>()?;
    service.register_operation::<SemanticTypedConstantOp>()?;
    service.register_operation::<SemanticTypedUnaryOp>()?;
    service.register_operation::<SemanticTypedBinaryOp>()?;
    service.register_operation::<SemanticTypedCompareOp>()?;
    service.register_operation::<SemanticTypedSelectOp>()?;
    service.register_operation::<SemanticTypedCastOp>()?;
    service.register_operation::<SemanticTypedExpressionRootOp>()?;
    service.register_operation::<RequireEquivalentOp>()?;
    service.register_operation::<RequireFiniteFoldOp>()?;
    service.register_operation::<RequireFiniteRecurrenceOp>()?;
    service.register_operation::<RequirePermutationGatherOp>()?;
    service.register_operation::<TensorLayoutOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

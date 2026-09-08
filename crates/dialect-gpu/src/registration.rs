use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AddressSpaceAttr, AllocaOp, AtomicOp, BarrierOp, CanonicalBarrierOp, CanonicalFenceOp,
    CanonicalKirOperationAttr, CanonicalKirTerminatorAttr, DIALECT_NAME, ExecutionCapabilityOp,
    ExecutionDomainAttr, ExecutionExtentAttr, ExecutionLayoutOp, FenceOp, Gfx950LdsTransposeOp,
    GridIdentityAttr, GuardedLoadOp, GuardedStoreOp, HierarchyAttr, HierarchyIdOp,
    HierarchyIndexType, InlineAssemblyOp, IntegerSwitchOp, IntrinsicOp, MatrixOp,
    MemoryIntrinsicOp, MemoryOrderAttr, MemoryScopeAttr, MemorySpaceOp, MemorySpaceType,
    SubgroupSizeAttr, SwitchOp, UnreachableOp, WaveOp, WorkgroupBarrierOp, WorkgroupMemoryOp,
    optimization_v1::{
        AccessModeAttr, BFloat16Attr, BFloat16Type, BinaryKindAttr, BinaryOp, BranchOp, CallOp,
        CastKindAttr, CastOp, CompareOp, ComparePredicateAttr, CondBranchOp, ConstantOp,
        GetElementPointerOp, IndexAttr, IndexType, LoadOp, MemoryAlignmentAttr, PointerType,
        PreservedOperationKindAttr, PreservedOperationOp, PreservedTerminatorKindAttr,
        PreservedTerminatorOp, ReturnOp, SelectOp, SliceDataOp, SliceLengthOp, SliceType, StoreOp,
        UnaryKindAttr, UnaryOp, VolatileAttr,
    },
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_attribute::<HierarchyAttr>()?;
    service.register_attribute::<AddressSpaceAttr>()?;
    service.register_attribute::<MemoryScopeAttr>()?;
    service.register_attribute::<MemoryOrderAttr>()?;
    service.register_attribute::<GridIdentityAttr>()?;
    service.register_attribute::<ExecutionExtentAttr>()?;
    service.register_attribute::<ExecutionDomainAttr>()?;
    service.register_attribute::<SubgroupSizeAttr>()?;
    service.register_attribute::<AccessModeAttr>()?;
    service.register_attribute::<UnaryKindAttr>()?;
    service.register_attribute::<BinaryKindAttr>()?;
    service.register_attribute::<ComparePredicateAttr>()?;
    service.register_attribute::<CastKindAttr>()?;
    service.register_attribute::<IndexAttr>()?;
    service.register_attribute::<BFloat16Attr>()?;
    service.register_attribute::<MemoryAlignmentAttr>()?;
    service.register_attribute::<VolatileAttr>()?;
    service.register_attribute::<CanonicalKirOperationAttr>()?;
    service.register_attribute::<CanonicalKirTerminatorAttr>()?;
    service.register_attribute::<PreservedOperationKindAttr>()?;
    service.register_attribute::<PreservedTerminatorKindAttr>()?;
    service.register_type::<HierarchyIndexType>()?;
    service.register_type::<MemorySpaceType>()?;
    service.register_type::<IndexType>()?;
    service.register_type::<BFloat16Type>()?;
    service.register_type::<PointerType>()?;
    service.register_type::<SliceType>()?;
    service.register_operation::<HierarchyIdOp>()?;
    service.register_operation::<ExecutionLayoutOp>()?;
    service.register_operation::<MemorySpaceOp>()?;
    service.register_operation::<BarrierOp>()?;
    service.register_operation::<FenceOp>()?;
    service.register_operation::<ConstantOp>()?;
    service.register_operation::<UnaryOp>()?;
    service.register_operation::<BinaryOp>()?;
    service.register_operation::<CompareOp>()?;
    service.register_operation::<CastOp>()?;
    service.register_operation::<SelectOp>()?;
    service.register_operation::<CallOp>()?;
    service.register_operation::<ReturnOp>()?;
    service.register_operation::<BranchOp>()?;
    service.register_operation::<CondBranchOp>()?;
    service.register_operation::<SliceLengthOp>()?;
    service.register_operation::<SliceDataOp>()?;
    service.register_operation::<GetElementPointerOp>()?;
    service.register_operation::<LoadOp>()?;
    service.register_operation::<StoreOp>()?;
    service.register_operation::<PreservedOperationOp>()?;
    service.register_operation::<PreservedTerminatorOp>()?;
    service.register_operation::<IntrinsicOp>()?;
    service.register_operation::<MemoryIntrinsicOp>()?;
    service.register_operation::<AllocaOp>()?;
    service.register_operation::<GuardedLoadOp>()?;
    service.register_operation::<GuardedStoreOp>()?;
    service.register_operation::<CanonicalBarrierOp>()?;
    service.register_operation::<AtomicOp>()?;
    service.register_operation::<CanonicalFenceOp>()?;
    service.register_operation::<WorkgroupBarrierOp>()?;
    service.register_operation::<WorkgroupMemoryOp>()?;
    service.register_operation::<MatrixOp>()?;
    service.register_operation::<Gfx950LdsTransposeOp>()?;
    service.register_operation::<WaveOp>()?;
    service.register_operation::<InlineAssemblyOp>()?;
    service.register_operation::<SwitchOp>()?;
    service.register_operation::<IntegerSwitchOp>()?;
    service.register_operation::<UnreachableOp>()?;
    service.register_operation::<ExecutionCapabilityOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

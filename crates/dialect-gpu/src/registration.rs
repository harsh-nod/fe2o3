use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AddressSpaceAttr, BarrierOp, DIALECT_NAME, ExecutionDomainAttr, ExecutionExtentAttr,
    ExecutionLayoutOp, FenceOp, GridIdentityAttr, HierarchyAttr, HierarchyIdOp, HierarchyIndexType,
    MemoryOrderAttr, MemoryScopeAttr, MemorySpaceOp, MemorySpaceType, SubgroupSizeAttr,
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
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

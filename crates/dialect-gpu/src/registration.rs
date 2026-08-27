use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AddressSpaceAttr, BarrierOp, DIALECT_NAME, ExecutionDomainAttr, ExecutionExtentAttr,
    ExecutionLayoutOp, FenceOp, GridIdentityAttr, HierarchyAttr, HierarchyIdOp, HierarchyIndexType,
    MemoryOrderAttr, MemoryScopeAttr, MemorySpaceOp, MemorySpaceType, SubgroupSizeAttr,
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
    service.register_type::<HierarchyIndexType>()?;
    service.register_type::<MemorySpaceType>()?;
    service.register_operation::<HierarchyIdOp>()?;
    service.register_operation::<ExecutionLayoutOp>()?;
    service.register_operation::<MemorySpaceOp>()?;
    service.register_operation::<BarrierOp>()?;
    service.register_operation::<FenceOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AddressSpaceAttr, BarrierOp, DIALECT_NAME, ExecutionLayoutOp, FenceOp, GeneralGemmEpilogueAttr,
    GeneralGemmEpilogueOp, GeneralGemmEpochAttr, GeneralGemmEpochOp, GeneralGemmGlobalTransferAttr,
    GeneralGemmGlobalTransferOp, GeneralGemmGridMappingAttr, GeneralGemmGridMappingOp,
    GeneralGemmLdsTransferAttr, GeneralGemmLdsTransferOp, GeneralGemmMfmaAttr, GeneralGemmMfmaOp,
    GeneralGemmPhaseLoopAttr, GeneralGemmPhaseLoopOp, GeneralGemmRuntimeAbiAttr,
    GeneralGemmRuntimeAbiOp, GridIdentityAttr, HierarchyAttr, HierarchyIdOp, HierarchyIndexType,
    MemoryOrderAttr, MemoryScopeAttr, MemorySpaceOp, MemorySpaceType, SubgroupSizeAttr,
    WorkgroupSizeAttr,
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
    service.register_attribute::<WorkgroupSizeAttr>()?;
    service.register_attribute::<SubgroupSizeAttr>()?;
    service.register_attribute::<GeneralGemmRuntimeAbiAttr>()?;
    service.register_attribute::<GeneralGemmGridMappingAttr>()?;
    service.register_attribute::<GeneralGemmPhaseLoopAttr>()?;
    service.register_attribute::<GeneralGemmGlobalTransferAttr>()?;
    service.register_attribute::<GeneralGemmLdsTransferAttr>()?;
    service.register_attribute::<GeneralGemmEpochAttr>()?;
    service.register_attribute::<GeneralGemmMfmaAttr>()?;
    service.register_attribute::<GeneralGemmEpilogueAttr>()?;
    service.register_type::<HierarchyIndexType>()?;
    service.register_type::<MemorySpaceType>()?;
    service.register_operation::<HierarchyIdOp>()?;
    service.register_operation::<ExecutionLayoutOp>()?;
    service.register_operation::<MemorySpaceOp>()?;
    service.register_operation::<BarrierOp>()?;
    service.register_operation::<FenceOp>()?;
    service.register_operation::<GeneralGemmRuntimeAbiOp>()?;
    service.register_operation::<GeneralGemmGridMappingOp>()?;
    service.register_operation::<GeneralGemmPhaseLoopOp>()?;
    service.register_operation::<GeneralGemmGlobalTransferOp>()?;
    service.register_operation::<GeneralGemmLdsTransferOp>()?;
    service.register_operation::<GeneralGemmEpochOp>()?;
    service.register_operation::<GeneralGemmMfmaOp>()?;
    service.register_operation::<GeneralGemmEpilogueOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

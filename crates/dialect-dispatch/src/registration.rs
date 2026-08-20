use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    DIALECT_NAME, DependencyIntentOp, DependencyKindAttr, DispatchIdAttr, DispatchModeAttr,
    EventRefType, GraphCapacityAttr, GraphIntentOp, GraphRefType, SelectionIntentOp,
    SelectionPolicyAttr, WorkspaceClassAttr, WorkspaceIntentOp, WorkspaceLifetimeAttr,
    WorkspaceRefType,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_attribute::<DispatchIdAttr>()?;
    service.register_attribute::<GraphCapacityAttr>()?;
    service.register_attribute::<DispatchModeAttr>()?;
    service.register_attribute::<DependencyKindAttr>()?;
    service.register_attribute::<WorkspaceClassAttr>()?;
    service.register_attribute::<WorkspaceLifetimeAttr>()?;
    service.register_attribute::<SelectionPolicyAttr>()?;
    service.register_type::<GraphRefType>()?;
    service.register_type::<EventRefType>()?;
    service.register_type::<WorkspaceRefType>()?;
    service.register_operation::<GraphIntentOp>()?;
    service.register_operation::<DependencyIntentOp>()?;
    service.register_operation::<WorkspaceIntentOp>()?;
    service.register_operation::<SelectionIntentOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

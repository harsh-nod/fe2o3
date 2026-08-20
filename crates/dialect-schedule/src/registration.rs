use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    DIALECT_NAME, GeneralGemmPhasePlanAttr, GeneralGemmPlanOp, GeneralGemmScheduleAttr,
    GeneralGemmTransferPlanAttr, ParametersAttr, PlanOp, PlanType,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<PlanType>()?;
    service.register_attribute::<ParametersAttr>()?;
    service.register_attribute::<GeneralGemmScheduleAttr>()?;
    service.register_attribute::<GeneralGemmPhasePlanAttr>()?;
    service.register_attribute::<GeneralGemmTransferPlanAttr>()?;
    service.register_operation::<PlanOp>()?;
    service.register_operation::<GeneralGemmPlanOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

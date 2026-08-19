use fe2o3_pliron::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{CandidateBudgetAttr, CandidateSetOp, CandidateSetType, DIALECT_NAME};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<CandidateSetType>()?;
    service.register_attribute::<CandidateBudgetAttr>()?;
    service.register_operation::<CandidateSetOp>()?;
    Ok(())
}

/// Returns the owner-bound autotune registration consumed by `fe2o3-pliron`.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

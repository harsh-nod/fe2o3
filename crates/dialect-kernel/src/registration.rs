use fe2o3_pliron::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AlgorithmOp, AlgorithmType, DIALECT_NAME, GeneralGemmAbiSchemaAttr,
    GeneralGemmEpilogueSchemaAttr, GeneralGemmOp, IterationDomainAttr,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<AlgorithmType>()?;
    service.register_attribute::<IterationDomainAttr>()?;
    service.register_attribute::<GeneralGemmAbiSchemaAttr>()?;
    service.register_attribute::<GeneralGemmEpilogueSchemaAttr>()?;
    service.register_operation::<AlgorithmOp>()?;
    service.register_operation::<GeneralGemmOp>()?;
    Ok(())
}

/// Returns the owner-bound kernel registration consumed by `fe2o3-pliron`.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

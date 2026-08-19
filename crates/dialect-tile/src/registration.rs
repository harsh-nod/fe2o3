use fe2o3_pliron::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    DIALECT_NAME, DistributedTileType, DistributionAttr, GeneralGemmXor4MappingAttr,
    GeneralGemmXor4Op, MaterializeOp,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<DistributedTileType>()?;
    service.register_attribute::<DistributionAttr>()?;
    service.register_attribute::<GeneralGemmXor4MappingAttr>()?;
    service.register_operation::<MaterializeOp>()?;
    service.register_operation::<GeneralGemmXor4Op>()?;
    Ok(())
}

/// Returns the owner-bound tile registration consumed by `fe2o3-pliron`.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

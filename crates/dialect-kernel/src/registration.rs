use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AccessKindAttr, AlgorithmOp, AlgorithmType, BranchOp, DIALECT_NAME, DimensionAttr, DimensionOp,
    GeneralGemmAbiSchemaAttr, GeneralGemmEpilogueSchemaAttr, GeneralGemmOp, IndexConstantOp,
    IndexLessThanBranchOp, IndexType, IndexValueAttr, IterationDomainAttr, RankedAccessOp,
    RankedViewOp, RankedViewType, ReturnOp,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_type::<AlgorithmType>()?;
    service.register_attribute::<IterationDomainAttr>()?;
    service.register_attribute::<GeneralGemmAbiSchemaAttr>()?;
    service.register_attribute::<GeneralGemmEpilogueSchemaAttr>()?;
    service.register_type::<RankedViewType>()?;
    service.register_type::<IndexType>()?;
    service.register_attribute::<IndexValueAttr>()?;
    service.register_attribute::<DimensionAttr>()?;
    service.register_attribute::<AccessKindAttr>()?;
    service.register_operation::<AlgorithmOp>()?;
    service.register_operation::<GeneralGemmOp>()?;
    service.register_operation::<RankedViewOp>()?;
    service.register_operation::<IndexConstantOp>()?;
    service.register_operation::<DimensionOp>()?;
    service.register_operation::<RankedAccessOp>()?;
    service.register_operation::<IndexLessThanBranchOp>()?;
    service.register_operation::<BranchOp>()?;
    service.register_operation::<ReturnOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

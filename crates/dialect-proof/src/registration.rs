use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, NameError, RegistrationHookError,
};

use crate::{
    AbsoluteErrorF64BitsAttr, CoveredBoundaryAttr, DIALECT_NAME, EvidenceRefOp, EvidenceRefType,
    EvidenceStatusAttr, ObligationOp, ObligationRefType, ProofIdAttr, PropertyAttr,
    RelativeErrorF64BitsAttr, RequireEffectRefinementOp, RequireNumericalRefinementOp,
    RequireRefinementOp,
};

fn registration_hook(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect(DIALECT_NAME)?;
    service.register_attribute::<ProofIdAttr>()?;
    service.register_attribute::<PropertyAttr>()?;
    service.register_attribute::<EvidenceStatusAttr>()?;
    service.register_attribute::<CoveredBoundaryAttr>()?;
    service.register_attribute::<AbsoluteErrorF64BitsAttr>()?;
    service.register_attribute::<RelativeErrorF64BitsAttr>()?;
    service.register_type::<ObligationRefType>()?;
    service.register_type::<EvidenceRefType>()?;
    service.register_operation::<ObligationOp>()?;
    service.register_operation::<EvidenceRefOp>()?;
    service.register_operation::<RequireRefinementOp>()?;
    service.register_operation::<RequireEffectRefinementOp>()?;
    service.register_operation::<RequireNumericalRefinementOp>()?;
    Ok(())
}

/// Returns the core-owned adapter consumed by the full `fe2o3-pliron` shell.
pub fn dialect_registration() -> Result<DialectRegistration, NameError> {
    DialectRegistration::new(DIALECT_NAME, registration_hook)
}

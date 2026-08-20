//! Focused typed dialect-registration capability tests.

use fe2o3_pliron_owner_core::{
    DialectRegistration, DialectRegistrationService, HARD_MAX_DIALECT_REGISTRATION_ACTIONS,
    RegistrationHookError,
};
use pliron::{builtin::attributes::UnitAttr, context::Context, dialect::DialectName};

fn at_action_limit(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect("builtin")?;
    for _ in 0..HARD_MAX_DIALECT_REGISTRATION_ACTIONS {
        service.register_attribute::<UnitAttr>()?;
    }
    Ok(())
}

fn over_action_limit(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.require_dialect("builtin")?;
    for _ in 0..=HARD_MAX_DIALECT_REGISTRATION_ACTIONS {
        service.register_attribute::<UnitAttr>()?;
    }
    Ok(())
}

fn foreign_entity(
    service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    service.register_attribute::<UnitAttr>()
}

fn empty_hook(_service: &mut DialectRegistrationService<'_>) -> Result<(), RegistrationHookError> {
    Ok(())
}

#[test]
fn registration_actions_accept_the_cap_and_reject_the_next_action() {
    let dialect_name = DialectName::try_new("builtin").expect("valid builtin namespace");
    let mut accepted = Context::new();
    DialectRegistration::new("builtin", at_action_limit)
        .expect("valid registration")
        .register_into(&mut accepted, &dialect_name)
        .expect("the exact action cap is admitted");

    let mut rejected = Context::new();
    let error = DialectRegistration::new("builtin", over_action_limit)
        .expect("valid registration")
        .register_into(&mut rejected, &dialect_name)
        .expect_err("one action past the cap must be rejected");
    assert_eq!(
        error.to_string(),
        "dialect registration action limit exceeded"
    );
}

#[test]
fn registration_rejects_namespace_mismatches() {
    let hostile_name = DialectName::try_new("hostile").expect("valid hostile namespace");
    let mut foreign = Context::new();
    let error = DialectRegistration::new("hostile", foreign_entity)
        .expect("valid registration")
        .register_into(&mut foreign, &hostile_name)
        .expect_err("builtin entity cannot register in a hostile namespace");
    assert_eq!(
        error.to_string(),
        "registered entity belongs to a different dialect"
    );

    let expected = DialectRegistration::new("expected", empty_hook).expect("valid registration");
    let wrong_name = DialectName::try_new("other").expect("valid other namespace");
    let error = expected
        .register_into(&mut Context::new(), &wrong_name)
        .expect_err("registration and owner namespace must agree");
    assert_eq!(error.to_string(), "dialect registration name mismatch");
}

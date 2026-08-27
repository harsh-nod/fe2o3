#![forbid(unsafe_code)]

//! Independent hostile conformance tests for context-owned Pliron artifacts.

use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_mir::{
    MirTypeId,
    pliron::{MirDialectLimits, MirModuleOp},
};
use fe2o3_lower_mir_kernel as lower_mir_kernel;
use fe2o3_pliron::{
    CONTEXT_IDENTITY_MARKER_KEY, ContextBuildError, Diagnostic, DiagnosticCode,
    DialectRegistration, DialectRegistrationService, PlironSession, RegistrationHookError,
    ShellLimits,
};
use pliron::{context::Context, identifier::Identifier, op::Op, operation::Operation};

fn mir_source(context: &mut Context, identity: &str) -> MirModuleOp {
    let limits = MirDialectLimits::new(2, 2, 64).expect("bounded MIR limits");
    let module = MirModuleOp::try_new(context, identity, limits).expect("valid MIR module");
    module
        .append_function(context, format!("{identity}::entry"), &[MirTypeId(1)])
        .expect("valid MIR function");
    module
}

fn mir_config() -> lower_mir_kernel::LoweringConfig {
    lower_mir_kernel::LoweringConfig::new(
        lower_mir_kernel::LoweringLimits::new(1, 2, 2, 16, 2).expect("bounded MIR lowering limits"),
        1,
    )
    .expect("bounded MIR lowering configuration")
}

fn lower_mir(context: &mut Context, identity: &str) -> lower_mir_kernel::LoweringResult {
    let source = mir_source(context, identity);
    let mut service = lower_mir_kernel::MirKernelLoweringPass::new(mir_config());
    service
        .run_checked(source.get_operation(), context)
        .expect("MIR lowering succeeds");
    service.take_result().expect("MIR result exists")
}

fn populated_context() -> (Context, lower_mir_kernel::LoweringResult) {
    let mut context = Context::new();
    lower_mir_kernel::register_pass(&mut context).expect("MIR registration succeeds");
    let result = lower_mir(&mut context, "owner-boundary-mir");
    (context, result)
}

fn marker_key(value: &str) -> Identifier {
    value.try_into().expect("fixed marker key is valid")
}

fn take_marker(context: &mut Context, key: &str) -> Box<dyn Any> {
    let index = context
        .aux_data_map
        .remove(&marker_key(key))
        .expect("marker is indexed");
    context.aux_data.remove(index).expect("marker is present")
}

fn install_marker(context: &mut Context, key: &str, marker: Box<dyn Any>) {
    let index = context.aux_data.insert(marker);
    context.aux_data_map.insert(marker_key(key), index);
}

fn transplant_marker(owner: &mut Context, foreign: &mut Context, key: &str) {
    let owner_marker = take_marker(owner, key);
    drop(take_marker(foreign, key));
    install_marker(foreign, key, owner_marker);
}

#[test]
fn equal_slot_foreign_contexts_reject_owner_bound_mir_results() {
    let (owner_context, owner) = populated_context();
    let (foreign_context, foreign) = populated_context();

    assert_eq!(owner.source_root(), foreign.source_root());
    assert_eq!(owner.operations(), foreign.operations());
    assert_eq!(owner.validate(&owner_context), Ok(()));

    let rejection = catch_unwind(AssertUnwindSafe(|| owner.validate(&foreign_context)))
        .expect("foreign validation must not unwind");
    assert_eq!(
        rejection,
        Err(lower_mir_kernel::PostconditionError::ContextMismatch)
    );
}

#[test]
fn transplanted_identity_and_registration_markers_cannot_transfer_ownership() {
    let (mut owner_context, owner) = populated_context();
    let (mut foreign_context, foreign) = populated_context();
    assert_eq!(owner.operations(), foreign.operations());

    for key in [
        CONTEXT_IDENTITY_MARKER_KEY,
        lower_mir_kernel::PASS_REGISTRATION_MARKER_KEY,
    ] {
        transplant_marker(&mut owner_context, &mut foreign_context, key);
    }

    assert_eq!(
        lower_mir_kernel::register_pass(&mut foreign_context),
        Err(lower_mir_kernel::PassRegistrationError::CorruptMarker)
    );
    let rejection = catch_unwind(AssertUnwindSafe(|| owner.validate(&foreign_context)))
        .expect("transplanted-marker validation must not unwind");
    assert_eq!(
        rejection,
        Err(lower_mir_kernel::PostconditionError::ContextMismatch)
    );
}

#[test]
fn erased_owner_handles_return_typed_errors_without_unwinding() {
    let mut context = Context::new();
    lower_mir_kernel::register_pass(&mut context).expect("MIR registration succeeds");
    let erased_source = lower_mir(&mut context, "erased-source");
    let erased_output = lower_mir(&mut context, "erased-output");

    Operation::erase(erased_source.source_root(), &mut context);
    Operation::erase(erased_output.operations()[0], &mut context);

    let rejection = catch_unwind(AssertUnwindSafe(|| {
        (
            erased_source.validate(&context),
            erased_output.validate(&context),
        )
    }))
    .expect("erased-handle validation must not unwind");
    assert_eq!(
        rejection.0,
        Err(lower_mir_kernel::PostconditionError::SourceNoLongerValid)
    );
    assert_eq!(
        rejection.1,
        Err(lower_mir_kernel::PostconditionError::InvalidKernelOperation { index: 0 })
    );
}

fn panicking_registration(
    _service: &mut DialectRegistrationService<'_>,
) -> Result<(), RegistrationHookError> {
    panic!("hostile-hook-payload");
}

fn panic_diagnostic(limits: ShellLimits) -> Diagnostic {
    let registration =
        DialectRegistration::new("hostile", panicking_registration).expect("valid registration");
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        PlironSession::new(limits, [registration])
    }))
    .expect("a panicking registration hook must be contained");
    match outcome {
        Err(ContextBuildError::RegistrationFailed(diagnostic)) => diagnostic,
        _ => panic!("a contained registration failure was expected"),
    }
}

#[test]
fn panicking_hooks_produce_bounded_deterministic_diagnostics() {
    let limits = ShellLimits::new(1, 1, 23).expect("bounded shell limits");
    let first = panic_diagnostic(limits);
    let second = panic_diagnostic(limits);

    assert_eq!(first, second);
    assert_eq!(first.code(), DiagnosticCode::DialectHookFailed);
    assert_eq!(first.stage(), Some("hostile"));
    assert!(first.message().len() <= limits.max_diagnostic_bytes());
    assert!(first.message().is_char_boundary(first.message().len()));
    assert!(!first.message().contains("hostile-hook-payload"));
}

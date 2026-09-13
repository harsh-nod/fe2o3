#![forbid(unsafe_code)]

//! Independent hostile conformance tests for context-owned Pliron artifacts.

use std::panic::{AssertUnwindSafe, catch_unwind};

use dialect_mir::MirTypeId;
use fe2o3_lower_mir_kernel as lower_mir_kernel;
use fe2o3_pliron::{
    ContextBuildError, Diagnostic, DiagnosticCode, DialectRegistration, DialectRegistrationService,
    PlironSession, RegistrationHookError, ShellLimits,
};

fn mir_input(identity: &str) -> lower_mir_kernel::MirKernelLoweringConformanceInputV1 {
    lower_mir_kernel::MirKernelLoweringConformanceInputV1::new(
        identity,
        vec![
            lower_mir_kernel::MirKernelLoweringConformanceFunctionV1::new(
                format!("{identity}::entry"),
                vec![MirTypeId(1)],
            ),
        ],
    )
}

fn mir_config() -> lower_mir_kernel::LoweringConfig {
    lower_mir_kernel::LoweringConfig::new(
        lower_mir_kernel::LoweringLimits::new(1, 2, 2, 16, 2).expect("bounded MIR lowering limits"),
        1,
    )
    .expect("bounded MIR lowering configuration")
}

#[test]
fn lowering_facade_returns_pointer_independent_observations_across_private_sessions() {
    let first = lower_mir_kernel::MirKernelLoweringConformanceV1
        .run(&mir_input("owner-boundary-mir"), mir_config())
        .expect("first private session");
    let second = lower_mir_kernel::MirKernelLoweringConformanceV1
        .run(&mir_input("owner-boundary-mir"), mir_config())
        .expect("second private session");

    assert_eq!(first, second);
    assert_eq!(first.record().source().identity(), "owner-boundary-mir");
    assert!(!first.grants_authority());
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

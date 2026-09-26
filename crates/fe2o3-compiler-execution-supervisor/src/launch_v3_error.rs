#[path = "launch_native_error.rs"]
mod shared;
shared::preparation_error!(
    ProtectedIssuerLaunchPreparationErrorV3,
    ProtectedIssuerSupervisorErrorV3,
    ProtectedIssuerHandoffErrorV3,
    CompilerExecutionServiceLaunchManifestErrorV3
);

#[cfg(test)]
#[path = "launch_native_error_tests.rs"]
mod tests;

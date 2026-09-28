#[path = "launch_native_error.rs"]
mod shared;
shared::preparation_error!(
    ProtectedIssuerLaunchPreparationErrorV2,
    ProtectedIssuerSupervisorErrorV2,
    ProtectedIssuerHandoffErrorV2,
    CompilerExecutionServiceLaunchManifestErrorV2
);

#[cfg(test)]
#[path = "launch_native_error_tests.rs"]
mod tests;

use fe2o3_compiler_execution_supervisor::{
    NATIVE_ISSUER_STARTUP_INPUT_STORAGE_V2 as INPUT, ProtectedIssuerDispatchLimitsV2 as Dispatch,
    ProtectedIssuerSessionLimitsV2 as Session, ProtectedIssuerWaitV2 as Wait,
    run_inherited_protected_issuer_service_v2 as run,
};
include!("main_native.rs");

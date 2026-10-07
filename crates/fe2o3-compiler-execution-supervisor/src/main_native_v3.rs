use fe2o3_compiler_execution_supervisor::{
    NATIVE_ISSUER_STARTUP_INPUT_STORAGE_V3 as INPUT, ProtectedIssuerDispatchLimitsV3 as Dispatch,
    ProtectedIssuerSessionLimitsV3 as Session, ProtectedIssuerWaitV3 as Wait,
    run_inherited_native_supervisor_v3 as run,
};
include!("main_native.rs");

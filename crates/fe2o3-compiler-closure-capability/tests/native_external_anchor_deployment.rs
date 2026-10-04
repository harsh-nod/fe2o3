#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![forbid(unsafe_code)]

mod v2 {
    use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorDeploymentCapabilityV2 as Cap;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V2 as BYTES,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V2 as DECODE_STORAGE,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2 as DECODE_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as SUPERVISOR_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as SUPERVISOR_WORK,
        CompilerExecutionExternalAnchorDeploymentErrorV2 as DecodeError,
        CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
        CompilerExecutionExternalAnchorDeploymentV3 as OtherDeployment,
        CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionSupervisorDeploymentV2 as Supervisor,
        CompilerExecutionSupervisorDeploymentV3 as OtherSupervisor,
    };
    const MAGIC: &[u8; 8] = b"F2O3CEA2";
    const VERSION: u16 = 2;
    const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V2\0";
    const OTHER_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V3\0";
    fn decode_error(error: &Error) -> &DecodeError {
        match error {
            Error::ExternalAnchorDeployment(error) => error,
            other => panic!("lost native anchor error: {other:?}"),
        }
    }
    include!("support/native_external_anchor_deployment_cases.rs");
}

mod v3 {
    use fe2o3_compiler_closure_capability::CompilerExecutionExternalAnchorDeploymentCapabilityV3 as Cap;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3 as BYTES,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3 as DECODE_STORAGE,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as DECODE_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as SUPERVISOR_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as SUPERVISOR_WORK,
        CompilerExecutionExternalAnchorDeploymentErrorV3 as DecodeError,
        CompilerExecutionExternalAnchorDeploymentV2 as OtherDeployment,
        CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as OtherSupervisor,
        CompilerExecutionSupervisorDeploymentV3 as Supervisor,
    };
    const MAGIC: &[u8; 8] = b"F2O3CEA3";
    const VERSION: u16 = 3;
    const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V3\0";
    const OTHER_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V2\0";
    fn decode_error(error: &Error) -> &DecodeError {
        match error {
            Error::ExternalAnchorDeploymentV3(error) => error,
            other => panic!("lost native anchor error: {other:?}"),
        }
    }
    include!("support/native_external_anchor_deployment_cases.rs");
}

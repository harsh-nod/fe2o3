#![cfg(all(target_os = "linux", target_arch = "x86_64"))]
#![forbid(unsafe_code)]

mod v2 {
    use fe2o3_compiler_closure_capability::CompilerExecutionSupervisorDeploymentCapabilityV2 as Cap;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V2 as BYTES,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as DECODE_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as DECODE_WORK,
        CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionSupervisorDeploymentErrorV2 as DecodeError,
        CompilerExecutionSupervisorDeploymentV2 as Deployment,
        CompilerExecutionSupervisorDeploymentV3 as OtherDeployment,
    };
    const MAGIC: &[u8; 8] = b"F2O3CED2";
    const VERSION: u16 = 2;
    const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V2\0";
    fn decode_error(error: &Error) -> &DecodeError {
        match error {
            Error::Deployment(error) => error,
            other => panic!("lost native deployment error: {other:?}"),
        }
    }
    include!("support/native_supervisor_deployment_cases.rs");
}

mod v3 {
    use fe2o3_compiler_closure_capability::CompilerExecutionSupervisorDeploymentCapabilityV3 as Cap;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3 as BYTES,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as DECODE_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as DECODE_WORK,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentErrorV3 as DecodeError,
        CompilerExecutionSupervisorDeploymentV2 as OtherDeployment,
        CompilerExecutionSupervisorDeploymentV3 as Deployment,
    };
    const MAGIC: &[u8; 8] = b"F2O3CED3";
    const VERSION: u16 = 3;
    const DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V3\0";
    fn decode_error(error: &Error) -> &DecodeError {
        match error {
            Error::DeploymentV3(error) => error,
            other => panic!("lost native deployment error: {other:?}"),
        }
    }
    include!("support/native_supervisor_deployment_cases.rs");
}

use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityV1 as Anchor,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionSupervisorReadyErrorV1 as Framing,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};

const LIMIT: usize = 10_000_000;
const PID: u32 = 123;
const BYTES: usize = 88;

fn key(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn measurement(seed: u8) -> Measurement {
    Measurement::new([seed; 32], 4096).unwrap()
}
fn reseal(bytes: &mut [u8; BYTES], version: u16) {
    let domain: &[u8] = match version {
        2 => b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-READY/V2\0",
        3 => b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-READY/V3\0",
        _ => unreachable!(),
    };
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(56u64.to_le_bytes());
    hash.update(&bytes[..56]);
    bytes[56..].copy_from_slice(&hash.finalize());
}

macro_rules! deployment {
    ($Policy:ty, $Deployment:ty, $generation:expr, $budget:expr) => {{
        let b = $budget;
        let (policy, delta) = <$Policy>::new(
            $generation,
            measurement(1),
            measurement(2),
            key(3),
            key(4),
            b,
        )
        .unwrap();
        b.reserve_storage(delta.additional_storage()).unwrap();
        let (deployment, delta) = <$Deployment>::new(
            1001,
            1002,
            Anchor::new(2001, 2002).unwrap(),
            measurement(5),
            measurement(6),
            &policy,
            b,
        )
        .unwrap();
        b.reserve_storage(delta.additional_storage()).unwrap();
        let charge = policy.retained_storage();
        drop(policy);
        b.release_storage(charge).unwrap();
        deployment
    }};
}

mod v2 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V2 as SCRATCH,
        COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V2 as WORK,
        CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionSupervisorDeploymentV2 as Deployment,
        CompilerExecutionSupervisorDeploymentV3 as OtherDeployment,
        CompilerExecutionSupervisorReadyErrorV2 as Error,
        CompilerExecutionSupervisorReadyV2 as Ready,
        CompilerExecutionSupervisorReadyV3 as OtherReady,
    };
    const VERSION: u16 = 2;
    const OTHER_VERSION: u16 = 3;
    include!("support/native_supervisor_ready_cases.rs");
}
mod v3 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SUPERVISOR_READY_STORAGE_V3 as SCRATCH,
        COMPILER_EXECUTION_SUPERVISOR_READY_WORK_V3 as WORK,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as OtherDeployment,
        CompilerExecutionSupervisorDeploymentV3 as Deployment,
        CompilerExecutionSupervisorReadyErrorV3 as Error,
        CompilerExecutionSupervisorReadyV2 as OtherReady,
        CompilerExecutionSupervisorReadyV3 as Ready,
    };
    const VERSION: u16 = 3;
    const OTHER_VERSION: u16 = 2;
    include!("support/native_supervisor_ready_cases.rs");
}

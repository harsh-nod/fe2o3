use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorServiceIdentityErrorV1 as ServiceError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy,
    CompilerExecutionSupervisorDeploymentErrorV1 as Framing,
    CompilerExecutionSupervisorDeploymentV1 as Legacy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};

const LIMIT: usize = 10_000_000;
const EXTRA: usize = 19;

fn key(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn measurement(seed: u8, len: u64) -> Measurement {
    Measurement::new([seed; 32], len).unwrap()
}
fn service() -> Service {
    Service::new(6001, 7001).unwrap()
}
fn legacy_policy() -> LegacyPolicy {
    LegacyPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        key(0x51),
        key(0x52),
    )
    .unwrap()
}
fn legacy_deployment(policy: &LegacyPolicy) -> Legacy {
    Legacy::new(
        1234,
        5678,
        service(),
        measurement(0x71, 4096),
        measurement(0x72, 8192),
        policy,
    )
    .unwrap()
}

fn reseal(bytes: &mut [u8; 184], version: u16) {
    let domain: &[u8] = match version {
        1 => b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V1\0",
        2 => b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V2\0",
        3 => b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V3\0",
        _ => unreachable!(),
    };
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(152u64.to_le_bytes());
    hash.update(&bytes[..152]);
    bytes[152..].copy_from_slice(&hash.finalize());
}

// Independent wire transcript, not the implementation's private codec.
fn wire(version: u16, policy: &[u8; 32]) -> [u8; 184] {
    let mut bytes = [0; 184];
    bytes[..8].copy_from_slice(match version {
        1 => b"F2O3CED1",
        2 => b"F2O3CED2",
        3 => b"F2O3CED3",
        _ => unreachable!(),
    });
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    for (offset, value) in [(12, 184u32), (24, 1234), (28, 5678), (32, 6001), (36, 7001)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[40..72].fill(0x71);
    bytes[72..80].copy_from_slice(&4096u64.to_le_bytes());
    bytes[80..112].fill(0x72);
    bytes[112..120].copy_from_slice(&8192u64.to_le_bytes());
    bytes[120..152].copy_from_slice(policy);
    reseal(&mut bytes, version);
    bytes
}

mod v2 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V2 as BYTES,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as WORK,
        CompilerExecutionAttestationStorageV2 as Storage,
        CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionSupervisorDeploymentErrorV2 as Error,
        CompilerExecutionSupervisorDeploymentV2 as Deployment,
        MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V2 as MAX_EXECUTABLE,
        MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V2 as MAX_LAUNCHER,
    };
    const VERSION: u16 = 2;
    const OTHER_VERSION: u16 = 3;
    include!("support/native_supervisor_deployment_cases.rs");
}

mod v3 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V3 as BYTES,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as WORK,
        CompilerExecutionAttestationStorageV3 as Storage,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentErrorV3 as Error,
        CompilerExecutionSupervisorDeploymentV3 as Deployment,
        MAX_COMPILER_EXECUTION_SUPERVISOR_EXECUTABLE_BYTES_V3 as MAX_EXECUTABLE,
        MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V3 as MAX_LAUNCHER,
    };
    const VERSION: u16 = 3;
    const OTHER_VERSION: u16 = 2;
    include!("support/native_supervisor_deployment_cases.rs");
}

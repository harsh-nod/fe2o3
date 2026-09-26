use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentErrorV1 as Framing,
    CompilerExecutionExternalAnchorDeploymentV1 as Legacy,
    CompilerExecutionExternalAnchorServiceIdentityErrorV1 as ServiceError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy,
    CompilerExecutionSupervisorDeploymentV1 as LegacySupervisor,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};

const LIMIT: usize = 20_000_000;
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
fn executable() -> Measurement {
    measurement(0x73, 16384)
}

fn legacy_context() -> (LegacySupervisor, LegacyPolicy) {
    let policy = LegacyPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        key(0x51),
        key(0x52),
    )
    .unwrap();
    let supervisor = LegacySupervisor::new(
        1234,
        5678,
        service(),
        measurement(0x71, 4096),
        measurement(0x72, 8192),
        &policy,
    )
    .unwrap();
    (supervisor, policy)
}

fn reseal(bytes: &mut [u8; 168], version: u16) {
    let domain: &[u8] = match version {
        1 => b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V1\0",
        2 => b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V2\0",
        3 => b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V3\0",
        _ => unreachable!(),
    };
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(136u64.to_le_bytes());
    hash.update(&bytes[..136]);
    bytes[136..].copy_from_slice(&hash.finalize());
}

// Independent canonical transcript; no production codec helper is used.
fn wire(version: u16, supervisor: &[u8; 32]) -> [u8; 168] {
    let mut bytes = [0; 168];
    bytes[..8].copy_from_slice(match version {
        1 => b"F2O3CEA1",
        2 => b"F2O3CEA2",
        3 => b"F2O3CEA3",
        _ => unreachable!(),
    });
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    bytes[12..16].copy_from_slice(&168u32.to_le_bytes());
    bytes[24..28].copy_from_slice(&6001u32.to_le_bytes());
    bytes[28..32].copy_from_slice(&7001u32.to_le_bytes());
    bytes[32..64].copy_from_slice(&key(0x52));
    bytes[64..96].copy_from_slice(supervisor);
    bytes[96..128].fill(0x73);
    bytes[128..136].copy_from_slice(&16384u64.to_le_bytes());
    reseal(&mut bytes, version);
    bytes
}

mod v2 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V2 as BYTES,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V2 as STORAGE,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2 as WORK,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V2 as SUPERVISOR_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as SUPERVISOR_WORK,
        CompilerExecutionAttestationStorageV2 as Storage,
        CompilerExecutionExternalAnchorDeploymentErrorV2 as Error,
        CompilerExecutionExternalAnchorDeploymentIdentityV2 as Identity,
        CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
        CompilerExecutionExternalAnchorDeploymentV3 as OtherDeployment,
        CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionSupervisorDeploymentV2 as Supervisor,
        CompilerExecutionSupervisorDeploymentV3 as OtherSupervisor,
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V2 as MAX_EXECUTABLE,
    };
    const VERSION: u16 = 2;
    const OTHER_VERSION: u16 = 3;
    include!("support/native_external_anchor_deployment_cases.rs");
}

mod v3 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V3 as BYTES,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_STORAGE_V3 as STORAGE,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as WORK,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_STORAGE_V3 as SUPERVISOR_STORAGE,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as SUPERVISOR_WORK,
        CompilerExecutionAttestationStorageV3 as Storage,
        CompilerExecutionExternalAnchorDeploymentErrorV3 as Error,
        CompilerExecutionExternalAnchorDeploymentIdentityV3 as Identity,
        CompilerExecutionExternalAnchorDeploymentV2 as OtherDeployment,
        CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as OtherSupervisor,
        CompilerExecutionSupervisorDeploymentV3 as Supervisor,
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V3 as MAX_EXECUTABLE,
    };
    const VERSION: u16 = 3;
    const OTHER_VERSION: u16 = 2;
    include!("support/native_external_anchor_deployment_cases.rs");
}

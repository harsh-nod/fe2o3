use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionExternalAnchorDeploymentV1 as LegacyDeployment,
    CompilerExecutionExternalAnchorProvisioningErrorV1 as Framing,
    CompilerExecutionExternalAnchorProvisioningV1 as Legacy,
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

const LIMIT: usize = 10_000_000;
const EXTRA: usize = 19;

fn key(seed: u8) -> [u8; 32] {
    SigningKey::from_bytes(&[seed; 32])
        .verifying_key()
        .to_bytes()
}
fn measurement(seed: u8, length: u64) -> Measurement {
    Measurement::new([seed; 32], length).unwrap()
}
fn helper() -> Measurement {
    measurement(0x74, 32768)
}
fn legacy_deployment() -> LegacyDeployment {
    let p = LegacyPolicy::new(
        7,
        measurement(0x61, 12345),
        measurement(0x62, 67890),
        key(0x51),
        key(0x52),
    )
    .unwrap();
    let s = LegacySupervisor::new(
        1234,
        5678,
        Service::new(6001, 7001).unwrap(),
        measurement(0x71, 4096),
        measurement(0x72, 8192),
        &p,
    )
    .unwrap();
    LegacyDeployment::new(&s, &p, measurement(0x73, 16384)).unwrap()
}
fn reseal(bytes: &mut [u8; 128], version: u16) {
    let domain: &[u8] = match version {
        1 => b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V1\0",
        2 => b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V2\0",
        3 => b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V3\0",
        _ => unreachable!(),
    };
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(96u64.to_le_bytes());
    hash.update(&bytes[..96]);
    bytes[96..].copy_from_slice(&hash.finalize());
}

// Independent transcript, without calling the private production codec.
fn wire(version: u16, deployment: &[u8; 32]) -> [u8; 128] {
    let mut bytes = [0; 128];
    bytes[..8].copy_from_slice(match version {
        1 => b"F2O3CEP1",
        2 => b"F2O3CEP2",
        3 => b"F2O3CEP3",
        _ => unreachable!(),
    });
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    bytes[12..16].copy_from_slice(&128u32.to_le_bytes());
    bytes[24..56].copy_from_slice(deployment);
    bytes[56..88].fill(0x74);
    bytes[88..96].copy_from_slice(&32768u64.to_le_bytes());
    reseal(&mut bytes, version);
    bytes
}

mod v2 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V2 as DEPLOYMENT_WORK,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V2 as BYTES,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V2 as STORAGE,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V2 as WORK,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V2 as SUPERVISOR_WORK,
        CompilerExecutionAttestationStorageV2 as Storage,
        CompilerExecutionExternalAnchorDeploymentV2 as Deployment,
        CompilerExecutionExternalAnchorDeploymentV3 as OtherDeployment,
        CompilerExecutionExternalAnchorProvisioningErrorV2 as Error,
        CompilerExecutionExternalAnchorProvisioningIdentityV2 as Identity,
        CompilerExecutionExternalAnchorProvisioningV2 as Provisioning,
        CompilerExecutionExternalAnchorProvisioningV3 as OtherProvisioning,
        CompilerExecutionIssuerPolicyV2 as Policy, CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionSupervisorDeploymentV2 as Supervisor,
        CompilerExecutionSupervisorDeploymentV3 as OtherSupervisor,
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V2 as MAX_HELPER,
    };
    const VERSION: u16 = 2;
    const OTHER_VERSION: u16 = 3;
    include!("support/native_external_anchor_provisioning_cases.rs");
}

mod v3 {
    use super::*;
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_WORK_V3 as DEPLOYMENT_WORK,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3 as BYTES,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3 as STORAGE,
        COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3 as WORK,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
        COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_WORK_V3 as SUPERVISOR_WORK,
        CompilerExecutionAttestationStorageV3 as Storage,
        CompilerExecutionExternalAnchorDeploymentV2 as OtherDeployment,
        CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
        CompilerExecutionExternalAnchorProvisioningErrorV3 as Error,
        CompilerExecutionExternalAnchorProvisioningIdentityV3 as Identity,
        CompilerExecutionExternalAnchorProvisioningV2 as OtherProvisioning,
        CompilerExecutionExternalAnchorProvisioningV3 as Provisioning,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy, CompilerExecutionIssuerPolicyV3 as Policy,
        CompilerExecutionSupervisorDeploymentV2 as OtherSupervisor,
        CompilerExecutionSupervisorDeploymentV3 as Supervisor,
        MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3 as MAX_HELPER,
    };
    const VERSION: u16 = 3;
    const OTHER_VERSION: u16 = 2;
    include!("support/native_external_anchor_provisioning_cases.rs");
}

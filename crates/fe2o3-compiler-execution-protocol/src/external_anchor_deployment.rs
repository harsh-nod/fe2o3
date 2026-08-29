#![forbid(unsafe_code)]

//! Canonical trusted-provisioning binding for the independently operated external anchor.

use std::{error::Error, fmt};

use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyV1, CompilerExecutionSupervisorDeploymentIdentityV1,
    CompilerExecutionSupervisorDeploymentV1,
};

const HEADER_BYTES: usize = 24;
const PREIMAGE_BYTES: usize = 136;
const SHA256_BYTES: usize = 32;
const MAGIC: [u8; 8] = *b"F2O3CEA1";
const VERSION_V1: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-DEPLOYMENT/V1\0";

/// Exact canonical byte length of one external-anchor deployment manifest.
pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1: usize =
    PREIMAGE_BYTES + SHA256_BYTES;
/// Maximum admitted external-anchor executable size.
pub const MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1: u64 = 128 * 1024 * 1024;

/// Domain-separated identity of one canonical external-anchor deployment manifest.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionExternalAnchorDeploymentIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionExternalAnchorDeploymentIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionExternalAnchorDeploymentIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionExternalAnchorDeploymentIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Immutable trust configuration supplied to the external-anchor process.
///
/// This manifest pins the exact dedicated anchor credentials, anchor verification key, supervisor
/// deployment identity, and external-anchor executable measurement. It contains no secret key,
/// path, descriptor, state, compiler, publication, load, launch, or GPU authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionExternalAnchorDeploymentV1 {
    service: CompilerExecutionExternalAnchorServiceIdentityV1,
    verifying_key: [u8; SHA256_BYTES],
    supervisor_deployment_identity: CompilerExecutionSupervisorDeploymentIdentityV1,
    executable: CompilerExecutionIssuerMeasurementV1,
    identity: CompilerExecutionExternalAnchorDeploymentIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1],
}

impl CompilerExecutionExternalAnchorDeploymentV1 {
    pub fn new(
        supervisor: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        executable: CompilerExecutionIssuerMeasurementV1,
    ) -> Result<Self, CompilerExecutionExternalAnchorDeploymentErrorV1> {
        if !supervisor.matches_policy(policy) {
            return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::SupervisorPolicyMismatch);
        }
        Self::from_parts(
            supervisor.external_anchor_service(),
            *policy.external_anchor_verifying_key(),
            supervisor.identity(),
            executable,
        )
    }

    fn from_parts(
        service: CompilerExecutionExternalAnchorServiceIdentityV1,
        verifying_key: [u8; SHA256_BYTES],
        supervisor_deployment_identity: CompilerExecutionSupervisorDeploymentIdentityV1,
        executable: CompilerExecutionIssuerMeasurementV1,
    ) -> Result<Self, CompilerExecutionExternalAnchorDeploymentErrorV1> {
        validate_verifying_key(verifying_key)?;
        if executable.byte_len() > MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1 {
            return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::ExecutableMeasurement);
        }
        let mut bytes = [0_u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1];
        encode_header(&mut bytes);
        bytes[24..28].copy_from_slice(&service.uid().to_le_bytes());
        bytes[28..32].copy_from_slice(&service.gid().to_le_bytes());
        bytes[32..64].copy_from_slice(&verifying_key);
        bytes[64..96].copy_from_slice(supervisor_deployment_identity.as_bytes());
        bytes[96..128].copy_from_slice(&executable.sha256());
        bytes[128..136].copy_from_slice(&executable.byte_len().to_le_bytes());
        let identity = CompilerExecutionExternalAnchorDeploymentIdentityV1(derive_identity(
            &bytes[..PREIMAGE_BYTES],
        ));
        bytes[PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        Ok(Self {
            service,
            verifying_key,
            supervisor_deployment_identity,
            executable,
            identity,
            bytes,
        })
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionExternalAnchorDeploymentErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1 {
            return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Length);
        }
        validate_header(bytes)?;
        let service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            read_u32(bytes, 24),
            read_u32(bytes, 28),
        )
        .map_err(CompilerExecutionExternalAnchorDeploymentErrorV1::ServiceIdentity)?;
        let verifying_key = bytes[32..64]
            .try_into()
            .expect("external-anchor verifying key has fixed width");
        validate_verifying_key(verifying_key)?;
        let supervisor_deployment_identity =
            CompilerExecutionSupervisorDeploymentIdentityV1::from_bytes_for_protocol(
                bytes[64..96]
                    .try_into()
                    .expect("supervisor deployment identity has fixed width"),
            )
            .ok_or(CompilerExecutionExternalAnchorDeploymentErrorV1::SupervisorIdentity)?;
        let executable = CompilerExecutionIssuerMeasurementV1::new(
            bytes[96..128]
                .try_into()
                .expect("external-anchor executable digest has fixed width"),
            read_u64(bytes, 128),
        )
        .map_err(|_| CompilerExecutionExternalAnchorDeploymentErrorV1::ExecutableMeasurement)?;
        if executable.byte_len() > MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1 {
            return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::ExecutableMeasurement);
        }
        let identity = CompilerExecutionExternalAnchorDeploymentIdentityV1(
            bytes[PREIMAGE_BYTES..]
                .try_into()
                .expect("external-anchor deployment identity has fixed width"),
        );
        if !identity.matches_canonical_bytes(bytes) {
            return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Identity);
        }
        let canonical = Self::from_parts(
            service,
            verifying_key,
            supervisor_deployment_identity,
            executable,
        )?;
        if canonical.bytes.as_slice() != bytes {
            return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Canonical);
        }
        Ok(canonical)
    }

    pub const fn service(&self) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.service
    }

    pub const fn verifying_key(&self) -> &[u8; SHA256_BYTES] {
        &self.verifying_key
    }

    pub const fn supervisor_deployment_identity(
        &self,
    ) -> CompilerExecutionSupervisorDeploymentIdentityV1 {
        self.supervisor_deployment_identity
    }

    /// Returns the exact admitted external-anchor executable measurement.
    pub const fn executable(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.executable
    }

    pub const fn identity(&self) -> CompilerExecutionExternalAnchorDeploymentIdentityV1 {
        self.identity
    }

    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1] {
        &self.bytes
    }

    pub fn matches_supervisor_deployment(
        &self,
        supervisor: &CompilerExecutionSupervisorDeploymentV1,
    ) -> bool {
        self.service == supervisor.external_anchor_service()
            && self.supervisor_deployment_identity == supervisor.identity()
    }

    pub fn matches_supervisor_and_policy(
        &self,
        supervisor: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> bool {
        self.matches_supervisor_deployment(supervisor)
            && supervisor.matches_policy(policy)
            && self.verifying_key == *policy.external_anchor_verifying_key()
    }

    /// Requires exact supervisor, policy, and executable agreement.
    pub fn matches_supervisor_policy_and_executable(
        &self,
        supervisor: &CompilerExecutionSupervisorDeploymentV1,
        policy: &CompilerExecutionIssuerPolicyV1,
        executable: CompilerExecutionIssuerMeasurementV1,
    ) -> bool {
        self.matches_supervisor_and_policy(supervisor, policy) && self.executable == executable
    }
}

fn validate_verifying_key(
    bytes: [u8; SHA256_BYTES],
) -> Result<(), CompilerExecutionExternalAnchorDeploymentErrorV1> {
    let key = VerifyingKey::from_bytes(&bytes)
        .map_err(|_| CompilerExecutionExternalAnchorDeploymentErrorV1::VerifyingKey)?;
    if key.is_weak() {
        return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::VerifyingKey);
    }
    Ok(())
}

fn encode_header(bytes: &mut [u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1]) {
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
    bytes[12..16].copy_from_slice(
        &(COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1 as u32).to_le_bytes(),
    );
}

fn validate_header(bytes: &[u8]) -> Result<(), CompilerExecutionExternalAnchorDeploymentErrorV1> {
    if bytes[..8] != MAGIC {
        return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Magic);
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION_V1 {
        return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Version);
    }
    if bytes[10..12].iter().any(|byte| *byte != 0)
        || bytes[16..HEADER_BYTES].iter().any(|byte| *byte != 0)
    {
        return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Reserved);
    }
    if read_u32(bytes, 12) as usize != COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1 {
        return Err(CompilerExecutionExternalAnchorDeploymentErrorV1::Length);
    }
    Ok(())
}

fn derive_identity(bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerExecutionExternalAnchorDeploymentErrorV1 {
    Length,
    Magic,
    Version,
    Reserved,
    ServiceIdentity(CompilerExecutionExternalAnchorServiceIdentityErrorV1),
    SupervisorPolicyMismatch,
    VerifyingKey,
    SupervisorIdentity,
    ExecutableMeasurement,
    Identity,
    Canonical,
}

impl fmt::Display for CompilerExecutionExternalAnchorDeploymentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "invalid compiler external-anchor deployment length",
            Self::Magic => "invalid compiler external-anchor deployment magic",
            Self::Version => "unsupported compiler external-anchor deployment version",
            Self::Reserved => "nonzero compiler external-anchor deployment reserved bytes",
            Self::ServiceIdentity(_) => "invalid compiler external-anchor service identity",
            Self::SupervisorPolicyMismatch => {
                "issuer policy differs from the supervisor deployment"
            }
            Self::VerifyingKey => "invalid compiler external-anchor verifying key",
            Self::SupervisorIdentity => "invalid supervisor deployment identity",
            Self::ExecutableMeasurement => "invalid external-anchor executable measurement",
            Self::Identity => "invalid compiler external-anchor deployment identity",
            Self::Canonical => "noncanonical compiler external-anchor deployment",
        })
    }
}

impl Error for CompilerExecutionExternalAnchorDeploymentErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ServiceIdentity(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;
    use crate::{
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
        CompilerExecutionSupervisorDeploymentV1,
    };

    fn policy() -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            7,
            CompilerExecutionIssuerMeasurementV1::new([0x11; 32], 4096).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x22; 32], 8192).unwrap(),
            SigningKey::from_bytes(&[0x33; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[0x44; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn supervisor() -> CompilerExecutionSupervisorDeploymentV1 {
        let policy = policy();
        CompilerExecutionSupervisorDeploymentV1::new(
            1001,
            1002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1003, 1004).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap(),
            &policy,
        )
        .unwrap()
    }

    fn deployment() -> CompilerExecutionExternalAnchorDeploymentV1 {
        let supervisor = supervisor();
        CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy(),
            CompilerExecutionIssuerMeasurementV1::new([0x66; 32], 32768).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn deployment_round_trips_and_binds_the_supervisor() {
        let deployment = deployment();
        let decoded =
            CompilerExecutionExternalAnchorDeploymentV1::decode(deployment.canonical_bytes())
                .unwrap();
        assert_eq!(decoded, deployment);
        assert_eq!(decoded.service().uid(), 1003);
        assert!(decoded.matches_supervisor_deployment(&supervisor()));
        assert!(decoded.matches_supervisor_and_policy(&supervisor(), &policy()));
        assert_eq!(decoded.executable().sha256(), [0x66; 32]);
        assert!(decoded.matches_supervisor_policy_and_executable(
            &supervisor(),
            &policy(),
            CompilerExecutionIssuerMeasurementV1::new([0x66; 32], 32768).unwrap()
        ));
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
    }

    #[test]
    fn every_single_byte_mutation_is_rejected() {
        let deployment = deployment();
        for index in 0..COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_BYTES_V1 {
            let mut mutated = *deployment.canonical_bytes();
            mutated[index] ^= 1;
            assert!(
                CompilerExecutionExternalAnchorDeploymentV1::decode(&mutated).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn mismatched_policy_weak_key_and_zero_supervisor_identity_are_rejected() {
        let supervisor = supervisor();
        let wrong_policy = CompilerExecutionIssuerPolicyV1::new(
            8,
            CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 4096).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 8192).unwrap(),
            SigningKey::from_bytes(&[0x63; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[0x64; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(
            CompilerExecutionExternalAnchorDeploymentV1::new(
                &supervisor,
                &wrong_policy,
                CompilerExecutionIssuerMeasurementV1::new([0x66; 32], 32768).unwrap(),
            ),
            Err(CompilerExecutionExternalAnchorDeploymentErrorV1::SupervisorPolicyMismatch)
        );
        let mut weak_key = *deployment().canonical_bytes();
        weak_key[32..64].fill(0);
        assert_eq!(
            CompilerExecutionExternalAnchorDeploymentV1::decode(&weak_key),
            Err(CompilerExecutionExternalAnchorDeploymentErrorV1::VerifyingKey)
        );
        let mut bytes = *deployment().canonical_bytes();
        bytes[64..96].fill(0);
        assert_eq!(
            CompilerExecutionExternalAnchorDeploymentV1::decode(&bytes),
            Err(CompilerExecutionExternalAnchorDeploymentErrorV1::SupervisorIdentity)
        );
        let mut executable = *deployment().canonical_bytes();
        executable[96..128].fill(0);
        assert_eq!(
            CompilerExecutionExternalAnchorDeploymentV1::decode(&executable),
            Err(CompilerExecutionExternalAnchorDeploymentErrorV1::ExecutableMeasurement)
        );
    }
}

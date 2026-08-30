//! Canonical protected-supervisor deployment readiness evidence.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionSupervisorDeploymentIdentityV1, CompilerExecutionSupervisorDeploymentV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const PREIMAGE_BYTES: usize = HEADER_BYTES + SHA256_BYTES;
const MAGIC: [u8; 8] = *b"F2O3CSR1";
const VERSION_V1: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-READY/V1\0";

/// Exact canonical byte length of one protected-supervisor readiness record.
pub const COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1: usize = PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one exact supervisor-readiness record.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionSupervisorReadyIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionSupervisorReadyIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionSupervisorReadyIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionSupervisorReadyIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Canonical inert evidence that one exact deployed supervisor completed admission.
///
/// This record grants no process, listener, signing, compiler, publication, loading, launch, or
/// execution authority. Its private bootstrap provenance and exact pidfd liveness remain root
/// deployment-coordinator obligations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionSupervisorReadyV1 {
    supervisor_pid: u32,
    deployment_identity: CompilerExecutionSupervisorDeploymentIdentityV1,
    identity: CompilerExecutionSupervisorReadyIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1],
}

impl CompilerExecutionSupervisorReadyV1 {
    /// Constructs readiness for one exact deployed supervisor occurrence.
    pub fn new(
        supervisor_pid: u32,
        deployment: &CompilerExecutionSupervisorDeploymentV1,
    ) -> Result<Self, CompilerExecutionSupervisorReadyErrorV1> {
        if supervisor_pid == 0 {
            return Err(CompilerExecutionSupervisorReadyErrorV1::SupervisorPid);
        }
        Ok(Self::from_parts(supervisor_pid, deployment.identity()))
    }

    fn from_parts(
        supervisor_pid: u32,
        deployment_identity: CompilerExecutionSupervisorDeploymentIdentityV1,
    ) -> Self {
        let mut bytes = [0_u8; COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1];
        bytes[..8].copy_from_slice(&MAGIC);
        bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
        bytes[12..16]
            .copy_from_slice(&(COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1 as u32).to_le_bytes());
        bytes[16..20].copy_from_slice(&supervisor_pid.to_le_bytes());
        bytes[24..56].copy_from_slice(deployment_identity.as_bytes());
        let identity =
            CompilerExecutionSupervisorReadyIdentityV1(derive_identity(&bytes[..PREIMAGE_BYTES]));
        bytes[PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        Self {
            supervisor_pid,
            deployment_identity,
            identity,
            bytes,
        }
    }

    /// Strictly decodes and independently re-encodes one readiness record.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionSupervisorReadyErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1 {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Length);
        }
        if bytes[..8] != MAGIC {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Magic);
        }
        if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION_V1 {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Version);
        }
        if bytes[10..12].iter().any(|byte| *byte != 0)
            || bytes[20..24].iter().any(|byte| *byte != 0)
        {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Reserved);
        }
        if read_u32(bytes, 12) as usize != COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1 {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Length);
        }
        let supervisor_pid = read_u32(bytes, 16);
        if supervisor_pid == 0 {
            return Err(CompilerExecutionSupervisorReadyErrorV1::SupervisorPid);
        }
        let deployment_bytes: [u8; SHA256_BYTES] = bytes[24..56].try_into().unwrap();
        let deployment_identity =
            CompilerExecutionSupervisorDeploymentIdentityV1::from_bytes_for_protocol(
                deployment_bytes,
            )
            .ok_or(CompilerExecutionSupervisorReadyErrorV1::DeploymentIdentity)?;
        let identity =
            CompilerExecutionSupervisorReadyIdentityV1(bytes[PREIMAGE_BYTES..].try_into().unwrap());
        if !identity.matches_canonical_bytes(bytes) {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Identity);
        }
        let canonical = Self::from_parts(supervisor_pid, deployment_identity);
        if canonical.bytes.as_slice() != bytes {
            return Err(CompilerExecutionSupervisorReadyErrorV1::Canonical);
        }
        Ok(canonical)
    }

    /// Returns the exact protected-supervisor PID.
    pub const fn supervisor_pid(&self) -> u32 {
        self.supervisor_pid
    }

    /// Returns the exact deployment identity admitted by the supervisor.
    pub const fn deployment_identity(&self) -> CompilerExecutionSupervisorDeploymentIdentityV1 {
        self.deployment_identity
    }

    /// Returns the terminal readiness identity.
    pub const fn identity(&self) -> CompilerExecutionSupervisorReadyIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1] {
        &self.bytes
    }

    /// Requires exact agreement with one root-retained child and deployment.
    pub fn matches_deployment(
        &self,
        supervisor_pid: u32,
        deployment: &CompilerExecutionSupervisorDeploymentV1,
    ) -> bool {
        self.supervisor_pid == supervisor_pid && self.deployment_identity == deployment.identity()
    }
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

/// Stable strict supervisor-readiness codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionSupervisorReadyErrorV1 {
    /// The record has the wrong encoded or supplied length.
    Length,
    /// The record has the wrong protocol magic.
    Magic,
    /// The record has an unsupported version.
    Version,
    /// A reserved byte is nonzero.
    Reserved,
    /// The supervisor PID is zero.
    SupervisorPid,
    /// The deployment identity is zero or invalid.
    DeploymentIdentity,
    /// The terminal record identity is invalid.
    Identity,
    /// Independent canonical re-encoding differs.
    Canonical,
}

impl fmt::Display for CompilerExecutionSupervisorReadyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "compiler supervisor readiness has the wrong length",
            Self::Magic => "compiler supervisor readiness has the wrong magic",
            Self::Version => "compiler supervisor readiness has the wrong version",
            Self::Reserved => "compiler supervisor readiness has nonzero reserved bytes",
            Self::SupervisorPid => "compiler supervisor readiness has a zero supervisor PID",
            Self::DeploymentIdentity => {
                "compiler supervisor readiness has an invalid deployment identity"
            }
            Self::Identity => "compiler supervisor readiness identity is invalid",
            Self::Canonical => "compiler supervisor readiness is not canonical",
        })
    }
}

impl Error for CompilerExecutionSupervisorReadyErrorV1 {}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;
    use crate::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1,
    };

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            SigningKey::from_bytes(&[seed; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[seed.wrapping_add(1); 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn deployment(seed: u8) -> CompilerExecutionSupervisorDeploymentV1 {
        CompilerExecutionSupervisorDeploymentV1::new(
            1_001,
            1_002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(2_001, 2_002).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 3; 32], 789).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 4; 32], 1_024).unwrap(),
            &policy(seed),
        )
        .unwrap()
    }

    #[test]
    fn exact_readiness_round_trips_and_matches_only_exact_deployment() {
        let expected = deployment(7);
        let ready = CompilerExecutionSupervisorReadyV1::new(5_678, &expected).unwrap();
        assert!(ready.matches_deployment(5_678, &expected));
        assert!(!ready.matches_deployment(5_679, &expected));
        assert!(!ready.matches_deployment(5_678, &deployment(8)));
        assert_eq!(
            CompilerExecutionSupervisorReadyV1::decode(ready.canonical_bytes()).unwrap(),
            ready
        );
        assert_eq!(
            CompilerExecutionSupervisorReadyV1::new(0, &expected),
            Err(CompilerExecutionSupervisorReadyErrorV1::SupervisorPid)
        );
        assert_eq!(
            ready.identity().as_bytes(),
            &[
                0x69, 0xfc, 0xe9, 0x99, 0xb3, 0x6d, 0x04, 0xb7, 0xb7, 0x53, 0xd7, 0x73, 0x4f, 0x07,
                0xfc, 0x10, 0x4a, 0xa7, 0x9f, 0x15, 0xcc, 0x6a, 0x64, 0xbc, 0x25, 0x35, 0xc1, 0x13,
                0x7d, 0x70, 0x86, 0x17,
            ]
        );
    }

    #[test]
    fn every_mutation_wrong_length_and_resealed_invalid_fields_reject() {
        let ready = CompilerExecutionSupervisorReadyV1::new(5_678, &deployment(7)).unwrap();
        for index in 0..ready.canonical_bytes().len() {
            let mut bytes = *ready.canonical_bytes();
            bytes[index] ^= 1;
            assert!(
                CompilerExecutionSupervisorReadyV1::decode(&bytes).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
        assert!(
            CompilerExecutionSupervisorReadyV1::decode(
                &ready.canonical_bytes()[..COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1 - 1]
            )
            .is_err()
        );
        let mut extended = ready.canonical_bytes().to_vec();
        extended.push(0);
        assert!(CompilerExecutionSupervisorReadyV1::decode(&extended).is_err());

        for (range, expected) in [
            (
                16..20,
                CompilerExecutionSupervisorReadyErrorV1::SupervisorPid,
            ),
            (
                24..56,
                CompilerExecutionSupervisorReadyErrorV1::DeploymentIdentity,
            ),
        ] {
            let mut bytes = *ready.canonical_bytes();
            bytes[range].fill(0);
            reseal(&mut bytes);
            assert_eq!(
                CompilerExecutionSupervisorReadyV1::decode(&bytes),
                Err(expected)
            );
        }
        for offset in [10, 20] {
            let mut bytes = *ready.canonical_bytes();
            bytes[offset] = 1;
            reseal(&mut bytes);
            assert_eq!(
                CompilerExecutionSupervisorReadyV1::decode(&bytes),
                Err(CompilerExecutionSupervisorReadyErrorV1::Reserved)
            );
        }
    }

    fn reseal(bytes: &mut [u8; COMPILER_EXECUTION_SUPERVISOR_READY_BYTES_V1]) {
        let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    }
}

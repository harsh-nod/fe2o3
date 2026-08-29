//! Canonical trusted-provisioning binding for the protected issuer supervisor.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
    CompilerExecutionIssuerPolicyIdentityV1, CompilerExecutionIssuerPolicyV1,
};

const HEADER_BYTES: usize = 24;
const PREIMAGE_BYTES: usize = 112;
const SHA256_BYTES: usize = 32;
const MAGIC: [u8; 8] = *b"F2O3CED1";
const VERSION_V1: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-DEPLOYMENT/V1\0";

/// Maximum admitted static pre-exec launcher size.
pub const MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1: u64 = 128 * 1024 * 1024;

/// Exact canonical byte length of one protected-supervisor deployment manifest.
pub const COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1: usize = PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one canonical protected-supervisor deployment manifest.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionSupervisorDeploymentIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionSupervisorDeploymentIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Independently rederives this identity from exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }

    pub(crate) fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Option<Self> {
        (bytes != [0; SHA256_BYTES]).then_some(Self(bytes))
    }
}

impl fmt::Debug for CompilerExecutionSupervisorDeploymentIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionSupervisorDeploymentIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Immutable trust configuration supplied by protected service provisioning.
///
/// The manifest pins the supervisor's dedicated credentials, the independently operated external
/// anchor's credentials, the exact static pre-exec launcher, and the exact issuer policy. It
/// contains no path, descriptor, secret, timeout, compiler, publication, load, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionSupervisorDeploymentV1 {
    service_uid: u32,
    service_gid: u32,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    launcher: CompilerExecutionIssuerMeasurementV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    identity: CompilerExecutionSupervisorDeploymentIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1],
}

impl CompilerExecutionSupervisorDeploymentV1 {
    /// Constructs one exact trusted deployment binding.
    pub fn new(
        service_uid: u32,
        service_gid: u32,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
        launcher: CompilerExecutionIssuerMeasurementV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, CompilerExecutionSupervisorDeploymentErrorV1> {
        Self::from_parts(
            service_uid,
            service_gid,
            external_anchor_service,
            launcher,
            policy.identity(),
        )
    }

    fn from_parts(
        service_uid: u32,
        service_gid: u32,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
        launcher: CompilerExecutionIssuerMeasurementV1,
        policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    ) -> Result<Self, CompilerExecutionSupervisorDeploymentErrorV1> {
        validate_service_identity(service_uid, service_gid)?;
        if service_uid == external_anchor_service.uid() {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::SharedServiceUid);
        }
        if launcher.byte_len() > MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1 {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::LauncherMeasurement);
        }
        let mut bytes = [0_u8; COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1];
        encode_header(&mut bytes);
        bytes[24..28].copy_from_slice(&service_uid.to_le_bytes());
        bytes[28..32].copy_from_slice(&service_gid.to_le_bytes());
        bytes[32..36].copy_from_slice(&external_anchor_service.uid().to_le_bytes());
        bytes[36..40].copy_from_slice(&external_anchor_service.gid().to_le_bytes());
        bytes[40..72].copy_from_slice(&launcher.sha256());
        bytes[72..80].copy_from_slice(&launcher.byte_len().to_le_bytes());
        bytes[80..112].copy_from_slice(policy_identity.as_bytes());
        let identity = CompilerExecutionSupervisorDeploymentIdentityV1(derive_identity(
            &bytes[..PREIMAGE_BYTES],
        ));
        bytes[PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        Ok(Self {
            service_uid,
            service_gid,
            external_anchor_service,
            launcher,
            policy_identity,
            identity,
            bytes,
        })
    }

    /// Strictly decodes and independently re-encodes one canonical deployment manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionSupervisorDeploymentErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1 {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::Length);
        }
        validate_header(bytes)?;
        let service_uid = read_u32(bytes, 24);
        let service_gid = read_u32(bytes, 28);
        validate_service_identity(service_uid, service_gid)?;
        let external_anchor_service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            read_u32(bytes, 32),
            read_u32(bytes, 36),
        )
        .map_err(CompilerExecutionSupervisorDeploymentErrorV1::ExternalAnchorServiceIdentity)?;
        if service_uid == external_anchor_service.uid() {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::SharedServiceUid);
        }
        let launcher = CompilerExecutionIssuerMeasurementV1::new(
            bytes[40..72]
                .try_into()
                .expect("launcher digest has fixed width"),
            read_u64(bytes, 72),
        )
        .map_err(|_| CompilerExecutionSupervisorDeploymentErrorV1::LauncherMeasurement)?;
        if launcher.byte_len() > MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1 {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::LauncherMeasurement);
        }
        let policy_identity_bytes: [u8; SHA256_BYTES] = bytes[80..112]
            .try_into()
            .expect("policy identity has fixed width");
        if policy_identity_bytes == [0; SHA256_BYTES] {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::PolicyIdentity);
        }
        let policy_identity =
            CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(policy_identity_bytes);
        let identity = CompilerExecutionSupervisorDeploymentIdentityV1(
            bytes[PREIMAGE_BYTES..]
                .try_into()
                .expect("deployment identity has fixed width"),
        );
        if !identity.matches_canonical_bytes(bytes) {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::Identity);
        }
        let canonical = Self::from_parts(
            service_uid,
            service_gid,
            external_anchor_service,
            launcher,
            policy_identity,
        )?;
        if canonical.bytes.as_slice() != bytes {
            return Err(CompilerExecutionSupervisorDeploymentErrorV1::Canonical);
        }
        Ok(canonical)
    }

    /// Returns the exact dedicated supervisor UID.
    pub const fn service_uid(&self) -> u32 {
        self.service_uid
    }

    /// Returns the exact dedicated supervisor GID.
    pub const fn service_gid(&self) -> u32 {
        self.service_gid
    }

    /// Returns the exact independently operated external-anchor credentials.
    pub const fn external_anchor_service(
        &self,
    ) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.external_anchor_service
    }

    /// Returns the exact trusted static pre-exec launcher measurement.
    pub const fn launcher(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.launcher
    }

    /// Returns the exact expected issuer-policy identity.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the terminal deployment-manifest identity.
    pub const fn identity(&self) -> CompilerExecutionSupervisorDeploymentIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical bytes.
    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1] {
        &self.bytes
    }

    /// Requires this deployment to name the exact supplied issuer policy.
    pub fn matches_policy(&self, policy: &CompilerExecutionIssuerPolicyV1) -> bool {
        self.policy_identity == policy.identity()
    }
}

fn validate_service_identity(
    uid: u32,
    gid: u32,
) -> Result<(), CompilerExecutionSupervisorDeploymentErrorV1> {
    if uid == 0 || uid == u32::MAX {
        return Err(CompilerExecutionSupervisorDeploymentErrorV1::ServiceUid);
    }
    if gid == 0 || gid == u32::MAX {
        return Err(CompilerExecutionSupervisorDeploymentErrorV1::ServiceGid);
    }
    Ok(())
}

fn encode_header(bytes: &mut [u8; COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1]) {
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
    bytes[12..16]
        .copy_from_slice(&(COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1 as u32).to_le_bytes());
}

fn validate_header(bytes: &[u8]) -> Result<(), CompilerExecutionSupervisorDeploymentErrorV1> {
    if bytes[..8] != MAGIC {
        return Err(CompilerExecutionSupervisorDeploymentErrorV1::Magic);
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION_V1 {
        return Err(CompilerExecutionSupervisorDeploymentErrorV1::Version);
    }
    if bytes[10..12].iter().any(|byte| *byte != 0)
        || bytes[16..HEADER_BYTES].iter().any(|byte| *byte != 0)
    {
        return Err(CompilerExecutionSupervisorDeploymentErrorV1::Reserved);
    }
    if read_u32(bytes, 12) as usize != COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1 {
        return Err(CompilerExecutionSupervisorDeploymentErrorV1::Length);
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

/// Stable canonical deployment-manifest rejection categories.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerExecutionSupervisorDeploymentErrorV1 {
    /// The byte length or declared length is not exact.
    Length,
    /// The record magic is invalid.
    Magic,
    /// The record version is unsupported.
    Version,
    /// Reserved bytes or flags are nonzero.
    Reserved,
    /// The protected supervisor UID is privileged or invalid.
    ServiceUid,
    /// The protected supervisor GID is privileged or invalid.
    ServiceGid,
    /// The external-anchor service identity is invalid.
    ExternalAnchorServiceIdentity(CompilerExecutionExternalAnchorServiceIdentityErrorV1),
    /// Supervisor and external-anchor services share one UID.
    SharedServiceUid,
    /// The static launcher measurement is empty or invalid.
    LauncherMeasurement,
    /// The issuer-policy identity is zero.
    PolicyIdentity,
    /// The terminal identity does not match the canonical preimage.
    Identity,
    /// Independent reconstruction did not reproduce the input bytes.
    Canonical,
}

impl fmt::Display for CompilerExecutionSupervisorDeploymentErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "invalid compiler-execution supervisor deployment length",
            Self::Magic => "invalid compiler-execution supervisor deployment magic",
            Self::Version => "unsupported compiler-execution supervisor deployment version",
            Self::Reserved => "nonzero compiler-execution supervisor deployment reserved bytes",
            Self::ServiceUid => "invalid compiler-execution supervisor service UID",
            Self::ServiceGid => "invalid compiler-execution supervisor service GID",
            Self::ExternalAnchorServiceIdentity(_) => {
                "invalid compiler-execution supervisor external-anchor service identity"
            }
            Self::SharedServiceUid => {
                "compiler-execution supervisor and external-anchor services share one UID"
            }
            Self::LauncherMeasurement => {
                "invalid compiler-execution supervisor static-launcher measurement"
            }
            Self::PolicyIdentity => "invalid compiler-execution supervisor policy identity",
            Self::Identity => "invalid compiler-execution supervisor deployment identity",
            Self::Canonical => "noncanonical compiler-execution supervisor deployment",
        })
    }
}

impl Error for CompilerExecutionSupervisorDeploymentErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ExternalAnchorServiceIdentity(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;
    use crate::CompilerExecutionIssuerPolicyV1;

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

    fn deployment() -> CompilerExecutionSupervisorDeploymentV1 {
        CompilerExecutionSupervisorDeploymentV1::new(
            1001,
            1002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1003, 1004).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap(),
            &policy(),
        )
        .unwrap()
    }

    #[test]
    fn deployment_round_trips_and_exposes_only_inert_trust_configuration() {
        let deployment = deployment();
        let decoded =
            CompilerExecutionSupervisorDeploymentV1::decode(deployment.canonical_bytes()).unwrap();
        assert_eq!(decoded, deployment);
        assert_eq!(decoded.service_uid(), 1001);
        assert_eq!(decoded.service_gid(), 1002);
        assert_eq!(decoded.external_anchor_service().uid(), 1003);
        assert_eq!(decoded.launcher().byte_len(), 16384);
        assert!(decoded.matches_policy(&policy()));
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
    }

    #[test]
    fn every_single_byte_mutation_is_rejected() {
        let deployment = deployment();
        for index in 0..COMPILER_EXECUTION_SUPERVISOR_DEPLOYMENT_BYTES_V1 {
            let mut mutated = *deployment.canonical_bytes();
            mutated[index] ^= 1;
            assert!(
                CompilerExecutionSupervisorDeploymentV1::decode(&mutated).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn privileged_shared_and_substituted_identities_are_rejected() {
        let anchor = CompilerExecutionExternalAnchorServiceIdentityV1::new(1003, 1004).unwrap();
        let launcher = CompilerExecutionIssuerMeasurementV1::new([0x55; 32], 16384).unwrap();
        let policy = policy();
        assert_eq!(
            CompilerExecutionSupervisorDeploymentV1::new(0, 1002, anchor, launcher, &policy),
            Err(CompilerExecutionSupervisorDeploymentErrorV1::ServiceUid)
        );
        assert_eq!(
            CompilerExecutionSupervisorDeploymentV1::new(1003, 1002, anchor, launcher, &policy),
            Err(CompilerExecutionSupervisorDeploymentErrorV1::SharedServiceUid)
        );
        let oversized = CompilerExecutionIssuerMeasurementV1::new(
            [0x56; 32],
            MAX_COMPILER_EXECUTION_SUPERVISOR_LAUNCHER_BYTES_V1 + 1,
        )
        .unwrap();
        assert_eq!(
            CompilerExecutionSupervisorDeploymentV1::new(1001, 1002, anchor, oversized, &policy),
            Err(CompilerExecutionSupervisorDeploymentErrorV1::LauncherMeasurement)
        );
        let other_policy = CompilerExecutionIssuerPolicyV1::new(
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
        assert!(!deployment().matches_policy(&other_policy));
    }
}

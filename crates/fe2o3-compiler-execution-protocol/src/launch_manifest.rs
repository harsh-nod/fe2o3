//! Canonical supervisor-to-issuer launch binding.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerPolicyIdentityV1,
    CompilerExecutionIssuerPolicyV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const PREIMAGE_BYTES: usize = HEADER_BYTES + 16 + 8 + SHA256_BYTES;
const MAGIC: [u8; 8] = *b"F2O3CEL1";
const VERSION_V1: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SERVICE-LAUNCH-MANIFEST/V1\0";

/// Exact canonical byte length of one compiler-execution service launch manifest.
pub const COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1: usize =
    PREIMAGE_BYTES + SHA256_BYTES;

/// Exact kernel-observed identity of one compiler service client process.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionClientProcessIdentityV1 {
    pid: u32,
    uid: u32,
    gid: u32,
}

impl CompilerExecutionClientProcessIdentityV1 {
    /// Creates one exact nonzero client PID and credential tuple.
    pub fn new(
        pid: u32,
        uid: u32,
        gid: u32,
    ) -> Result<Self, CompilerExecutionServiceLaunchManifestErrorV1> {
        if pid == 0 {
            return Err(CompilerExecutionServiceLaunchManifestErrorV1::ClientPid);
        }
        Ok(Self { pid, uid, gid })
    }

    /// Returns the exact client PID.
    pub const fn pid(self) -> u32 {
        self.pid
    }

    /// Returns the exact client UID.
    pub const fn uid(self) -> u32 {
        self.uid
    }

    /// Returns the exact client GID.
    pub const fn gid(self) -> u32 {
        self.gid
    }
}

/// Domain-separated identity of one canonical service launch manifest.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionServiceLaunchManifestIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionServiceLaunchManifestIdentityV1 {
    pub(crate) const fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Independently rederives this identity from exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionServiceLaunchManifestIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionServiceLaunchManifestIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Canonical inert binding supplied by the protected issuer supervisor.
///
/// The manifest binds the exact expected client credentials and external-anchor service identity
/// to the exact caller-pinned issuer policy. It grants no process, signing, compiler, publication,
/// loading, or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionServiceLaunchManifestV1 {
    client: CompilerExecutionClientProcessIdentityV1,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    identity: CompilerExecutionServiceLaunchManifestIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1],
}

impl CompilerExecutionServiceLaunchManifestV1 {
    /// Constructs one canonical expected-client and issuer-policy binding.
    pub fn new(
        client: CompilerExecutionClientProcessIdentityV1,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Self {
        Self::from_parts(client, external_anchor_service, policy.identity())
    }

    fn from_parts(
        client: CompilerExecutionClientProcessIdentityV1,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
        policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    ) -> Self {
        let mut bytes = [0_u8; COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1];
        encode_header(&mut bytes);
        bytes[24..28].copy_from_slice(&client.pid.to_le_bytes());
        bytes[28..32].copy_from_slice(&client.uid.to_le_bytes());
        bytes[32..36].copy_from_slice(&client.gid.to_le_bytes());
        bytes[40..44].copy_from_slice(&external_anchor_service.uid().to_le_bytes());
        bytes[44..48].copy_from_slice(&external_anchor_service.gid().to_le_bytes());
        bytes[48..80].copy_from_slice(policy_identity.as_bytes());
        let identity = CompilerExecutionServiceLaunchManifestIdentityV1(derive_identity(
            &bytes[..PREIMAGE_BYTES],
        ));
        bytes[PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        Self {
            client,
            external_anchor_service,
            policy_identity,
            identity,
            bytes,
        }
    }

    /// Strictly decodes and independently re-encodes one canonical manifest.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionServiceLaunchManifestErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1 {
            return Err(CompilerExecutionServiceLaunchManifestErrorV1::Length);
        }
        validate_header(bytes)?;
        if bytes[36..40].iter().any(|byte| *byte != 0) {
            return Err(CompilerExecutionServiceLaunchManifestErrorV1::Reserved);
        }
        let client = CompilerExecutionClientProcessIdentityV1::new(
            read_u32(bytes, 24),
            read_u32(bytes, 28),
            read_u32(bytes, 32),
        )?;
        let external_anchor_service = CompilerExecutionExternalAnchorServiceIdentityV1::new(
            read_u32(bytes, 40),
            read_u32(bytes, 44),
        )
        .map_err(CompilerExecutionServiceLaunchManifestErrorV1::ExternalAnchorServiceIdentity)?;
        let policy_identity_bytes: [u8; SHA256_BYTES] = bytes[48..80]
            .try_into()
            .expect("policy identity has a fixed width");
        if policy_identity_bytes == [0; SHA256_BYTES] {
            return Err(CompilerExecutionServiceLaunchManifestErrorV1::PolicyIdentity);
        }
        let policy_identity =
            CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(policy_identity_bytes);
        let identity = CompilerExecutionServiceLaunchManifestIdentityV1(
            bytes[PREIMAGE_BYTES..]
                .try_into()
                .expect("manifest identity has a fixed width"),
        );
        if !identity.matches_canonical_bytes(bytes) {
            return Err(CompilerExecutionServiceLaunchManifestErrorV1::Identity);
        }
        let canonical = Self::from_parts(client, external_anchor_service, policy_identity);
        if canonical.policy_identity != policy_identity || canonical.bytes.as_slice() != bytes {
            return Err(CompilerExecutionServiceLaunchManifestErrorV1::Canonical);
        }
        Ok(Self {
            client,
            external_anchor_service,
            policy_identity,
            identity,
            bytes: bytes.try_into().expect("manifest length checked"),
        })
    }

    /// Returns the exact expected client identity.
    pub const fn client(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.client
    }

    /// Returns the exact pinned external-anchor service credential identity.
    pub const fn external_anchor_service(
        &self,
    ) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.external_anchor_service
    }

    /// Returns the exact expected issuer-policy identity.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the terminal manifest identity.
    pub const fn identity(&self) -> CompilerExecutionServiceLaunchManifestIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical bytes.
    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1] {
        &self.bytes
    }

    /// Requires this manifest to name the exact supplied caller-pinned policy.
    pub fn matches_policy(&self, policy: &CompilerExecutionIssuerPolicyV1) -> bool {
        self.policy_identity == policy.identity()
    }

    /// Requires this manifest to name the exact supplied external-anchor service identity.
    pub fn matches_external_anchor_service(
        &self,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    ) -> bool {
        self.external_anchor_service == external_anchor_service
    }
}

fn encode_header(bytes: &mut [u8; COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1]) {
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
    bytes[12..16].copy_from_slice(
        &(COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1 as u32).to_le_bytes(),
    );
}

fn validate_header(bytes: &[u8]) -> Result<(), CompilerExecutionServiceLaunchManifestErrorV1> {
    if bytes[..8] != MAGIC {
        return Err(CompilerExecutionServiceLaunchManifestErrorV1::Magic);
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION_V1 {
        return Err(CompilerExecutionServiceLaunchManifestErrorV1::Version);
    }
    if bytes[10..12].iter().any(|byte| *byte != 0) || bytes[16..24].iter().any(|byte| *byte != 0) {
        return Err(CompilerExecutionServiceLaunchManifestErrorV1::Reserved);
    }
    if read_u32(bytes, 12) as usize != COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1 {
        return Err(CompilerExecutionServiceLaunchManifestErrorV1::Length);
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

/// Stable strict launch-manifest codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionServiceLaunchManifestErrorV1 {
    Length,
    Magic,
    Version,
    Reserved,
    ClientPid,
    ExternalAnchorServiceIdentity(CompilerExecutionExternalAnchorServiceIdentityErrorV1),
    PolicyIdentity,
    Identity,
    Canonical,
}

impl fmt::Display for CompilerExecutionServiceLaunchManifestErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "compiler service launch manifest has the wrong length",
            Self::Magic => "compiler service launch manifest has the wrong magic",
            Self::Version => "compiler service launch manifest has the wrong version",
            Self::Reserved => "compiler service launch manifest has nonzero reserved bytes",
            Self::ClientPid => "compiler service launch manifest has a zero client PID",
            Self::ExternalAnchorServiceIdentity(_) => {
                "compiler service launch manifest has an invalid external-anchor service identity"
            }
            Self::PolicyIdentity => {
                "compiler service launch manifest has an invalid policy identity"
            }
            Self::Identity => "compiler service launch manifest identity is invalid",
            Self::Canonical => "compiler service launch manifest is not canonical",
        })
    }
}

impl Error for CompilerExecutionServiceLaunchManifestErrorV1 {
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
    use crate::CompilerExecutionIssuerMeasurementV1;

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        let signing_key = SigningKey::from_bytes(&[seed; 32]);
        CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            signing_key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[seed.wrapping_add(1); 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap()
    }

    fn external_anchor_service(seed: u32) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        CompilerExecutionExternalAnchorServiceIdentityV1::new(6_000 + seed, 7_000 + seed).unwrap()
    }

    #[test]
    fn exact_manifest_round_trips_and_binds_client_and_policy() {
        let issuer_policy = policy(7);
        let client = CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1001).unwrap();
        let manifest = CompilerExecutionServiceLaunchManifestV1::new(
            client,
            external_anchor_service(1),
            &issuer_policy,
        );
        assert_eq!(
            manifest.canonical_bytes().len(),
            COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1
        );
        assert_eq!(manifest.client(), client);
        assert_eq!(
            manifest.external_anchor_service(),
            external_anchor_service(1)
        );
        assert!(manifest.matches_external_anchor_service(external_anchor_service(1)));
        assert!(!manifest.matches_external_anchor_service(external_anchor_service(2)));
        assert!(manifest.matches_policy(&issuer_policy));
        assert_eq!(
            CompilerExecutionServiceLaunchManifestV1::decode(manifest.canonical_bytes()).unwrap(),
            manifest
        );

        let other_client = CompilerExecutionClientProcessIdentityV1::new(1235, 1000, 1001).unwrap();
        assert_ne!(
            CompilerExecutionServiceLaunchManifestV1::new(
                other_client,
                external_anchor_service(1),
                &issuer_policy
            )
            .identity(),
            manifest.identity()
        );
        assert_ne!(
            CompilerExecutionServiceLaunchManifestV1::new(
                client,
                external_anchor_service(2),
                &issuer_policy
            )
            .identity(),
            manifest.identity()
        );
        assert!(!manifest.matches_policy(&policy(8)));
    }

    #[test]
    fn every_byte_mutation_and_wrong_length_rejects() {
        let manifest = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1001).unwrap(),
            external_anchor_service(1),
            &policy(7),
        );
        for index in 0..manifest.canonical_bytes().len() {
            let mut bytes = *manifest.canonical_bytes();
            bytes[index] ^= 1;
            assert!(
                CompilerExecutionServiceLaunchManifestV1::decode(&bytes).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
        assert!(
            CompilerExecutionServiceLaunchManifestV1::decode(
                &manifest.canonical_bytes()[..manifest.canonical_bytes().len() - 1]
            )
            .is_err()
        );
        let mut extended = manifest.canonical_bytes().to_vec();
        extended.push(0);
        assert!(CompilerExecutionServiceLaunchManifestV1::decode(&extended).is_err());
    }

    #[test]
    fn independently_resealed_invalid_fields_still_reject() {
        assert_eq!(
            CompilerExecutionClientProcessIdentityV1::new(0, 1000, 1001),
            Err(CompilerExecutionServiceLaunchManifestErrorV1::ClientPid)
        );
        let manifest = CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1001).unwrap(),
            external_anchor_service(1),
            &policy(7),
        );

        for (offset, expected) in [
            (10, CompilerExecutionServiceLaunchManifestErrorV1::Reserved),
            (16, CompilerExecutionServiceLaunchManifestErrorV1::Reserved),
            (36, CompilerExecutionServiceLaunchManifestErrorV1::Reserved),
        ] {
            let mut bytes = *manifest.canonical_bytes();
            bytes[offset] = 1;
            reseal(&mut bytes);
            assert_eq!(
                CompilerExecutionServiceLaunchManifestV1::decode(&bytes),
                Err(expected)
            );
        }

        let mut zero_pid = *manifest.canonical_bytes();
        zero_pid[24..28].fill(0);
        reseal(&mut zero_pid);
        assert_eq!(
            CompilerExecutionServiceLaunchManifestV1::decode(&zero_pid),
            Err(CompilerExecutionServiceLaunchManifestErrorV1::ClientPid)
        );

        for range in [40..44, 44..48] {
            let mut invalid_anchor_service = *manifest.canonical_bytes();
            invalid_anchor_service[range].fill(0);
            reseal(&mut invalid_anchor_service);
            assert!(matches!(
                CompilerExecutionServiceLaunchManifestV1::decode(&invalid_anchor_service),
                Err(
                    CompilerExecutionServiceLaunchManifestErrorV1::ExternalAnchorServiceIdentity(_)
                )
            ));
        }

        let mut zero_policy = *manifest.canonical_bytes();
        zero_policy[48..80].fill(0);
        reseal(&mut zero_policy);
        assert_eq!(
            CompilerExecutionServiceLaunchManifestV1::decode(&zero_policy),
            Err(CompilerExecutionServiceLaunchManifestErrorV1::PolicyIdentity)
        );
    }

    fn reseal(bytes: &mut [u8; COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1]) {
        let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    }
}

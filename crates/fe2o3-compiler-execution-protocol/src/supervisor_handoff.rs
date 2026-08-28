//! Canonical direct-parent rustc handoff to the protected supervisor.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1, CompilerExecutionClientProcessIdentityV1,
    CompilerExecutionServiceLaunchManifestErrorV1, CompilerExecutionServiceLaunchManifestV1,
};

const SHA256_BYTES: usize = 32;
const HEADER_BYTES: usize = 24;
const SUBMITTER_BYTES: usize = 16;
const MANIFEST_OFFSET: usize = HEADER_BYTES + SUBMITTER_BYTES;
const PREIMAGE_BYTES: usize = MANIFEST_OFFSET + COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V1;
const MAGIC: [u8; 8] = *b"F2O3CEH1";
const VERSION_V1: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-SUPERVISOR-HANDOFF/V1\0";

/// Exact canonical byte length of one direct-parent supervisor handoff record.
pub const COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1: usize = PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one canonical protected-supervisor handoff.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionSupervisorHandoffIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionSupervisorHandoffIdentityV1 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Independently rederives this identity from exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionSupervisorHandoffIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionSupervisorHandoffIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Canonical inert binding between the direct Cargo parent and one rustc launch manifest.
///
/// The submitter must be a separate process with the exact UID/GID of the rustc child. This record
/// grants no process, signing, compiler, publication, loading, or execution authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionSupervisorHandoffV1 {
    submitter: CompilerExecutionClientProcessIdentityV1,
    launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    identity: CompilerExecutionSupervisorHandoffIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1],
}

impl CompilerExecutionSupervisorHandoffV1 {
    /// Constructs one canonical direct-parent and rustc launch binding.
    pub fn new(
        submitter: CompilerExecutionClientProcessIdentityV1,
        launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    ) -> Result<Self, CompilerExecutionSupervisorHandoffErrorV1> {
        validate_relationship(submitter, launch_manifest.client())?;
        Ok(Self::from_parts(submitter, launch_manifest))
    }

    fn from_parts(
        submitter: CompilerExecutionClientProcessIdentityV1,
        launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    ) -> Self {
        let mut bytes = [0_u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1];
        encode_header(&mut bytes);
        bytes[24..28].copy_from_slice(&submitter.pid().to_le_bytes());
        bytes[28..32].copy_from_slice(&submitter.uid().to_le_bytes());
        bytes[32..36].copy_from_slice(&submitter.gid().to_le_bytes());
        bytes[MANIFEST_OFFSET..PREIMAGE_BYTES].copy_from_slice(launch_manifest.canonical_bytes());
        let identity =
            CompilerExecutionSupervisorHandoffIdentityV1(derive_identity(&bytes[..PREIMAGE_BYTES]));
        bytes[PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        Self {
            submitter,
            launch_manifest,
            identity,
            bytes,
        }
    }

    /// Strictly decodes and independently re-encodes one complete canonical handoff.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionSupervisorHandoffErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1 {
            return Err(CompilerExecutionSupervisorHandoffErrorV1::Length);
        }
        validate_header(bytes)?;
        if bytes[36..40].iter().any(|byte| *byte != 0) {
            return Err(CompilerExecutionSupervisorHandoffErrorV1::Reserved);
        }
        let submitter = CompilerExecutionClientProcessIdentityV1::new(
            read_u32(bytes, 24),
            read_u32(bytes, 28),
            read_u32(bytes, 32),
        )
        .map_err(|_| CompilerExecutionSupervisorHandoffErrorV1::SubmitterPid)?;
        let launch_manifest = CompilerExecutionServiceLaunchManifestV1::decode(
            &bytes[MANIFEST_OFFSET..PREIMAGE_BYTES],
        )
        .map_err(CompilerExecutionSupervisorHandoffErrorV1::LaunchManifest)?;
        validate_relationship(submitter, launch_manifest.client())?;
        let identity = CompilerExecutionSupervisorHandoffIdentityV1(
            bytes[PREIMAGE_BYTES..]
                .try_into()
                .expect("handoff identity has a fixed width"),
        );
        if !identity.matches_canonical_bytes(bytes) {
            return Err(CompilerExecutionSupervisorHandoffErrorV1::Identity);
        }
        let canonical = Self::from_parts(submitter, launch_manifest);
        if canonical.bytes.as_slice() != bytes {
            return Err(CompilerExecutionSupervisorHandoffErrorV1::Canonical);
        }
        Ok(canonical)
    }

    /// Returns the exact direct-parent process identity authorized to submit this record.
    pub const fn submitter(&self) -> CompilerExecutionClientProcessIdentityV1 {
        self.submitter
    }

    /// Returns the exact nested supervisor-to-issuer launch manifest.
    pub const fn launch_manifest(&self) -> &CompilerExecutionServiceLaunchManifestV1 {
        &self.launch_manifest
    }

    /// Returns the terminal handoff identity.
    pub const fn identity(&self) -> CompilerExecutionSupervisorHandoffIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical handoff bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1] {
        &self.bytes
    }
}

fn validate_relationship(
    submitter: CompilerExecutionClientProcessIdentityV1,
    client: CompilerExecutionClientProcessIdentityV1,
) -> Result<(), CompilerExecutionSupervisorHandoffErrorV1> {
    if submitter.pid() == client.pid() {
        return Err(CompilerExecutionSupervisorHandoffErrorV1::SubmitterIsClient);
    }
    if submitter.uid() != client.uid() || submitter.gid() != client.gid() {
        return Err(CompilerExecutionSupervisorHandoffErrorV1::CredentialMismatch);
    }
    Ok(())
}

fn encode_header(bytes: &mut [u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1]) {
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
    bytes[12..16]
        .copy_from_slice(&(COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1 as u32).to_le_bytes());
}

fn validate_header(bytes: &[u8]) -> Result<(), CompilerExecutionSupervisorHandoffErrorV1> {
    if bytes[..8] != MAGIC {
        return Err(CompilerExecutionSupervisorHandoffErrorV1::Magic);
    }
    if u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != VERSION_V1 {
        return Err(CompilerExecutionSupervisorHandoffErrorV1::Version);
    }
    if bytes[10..12].iter().any(|byte| *byte != 0) || bytes[16..24].iter().any(|byte| *byte != 0) {
        return Err(CompilerExecutionSupervisorHandoffErrorV1::Reserved);
    }
    if read_u32(bytes, 12) as usize != COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1 {
        return Err(CompilerExecutionSupervisorHandoffErrorV1::Length);
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

/// Stable strict supervisor-handoff codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionSupervisorHandoffErrorV1 {
    Length,
    Magic,
    Version,
    Reserved,
    SubmitterPid,
    SubmitterIsClient,
    CredentialMismatch,
    LaunchManifest(CompilerExecutionServiceLaunchManifestErrorV1),
    Identity,
    Canonical,
}

impl fmt::Display for CompilerExecutionSupervisorHandoffErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => formatter.write_str("compiler supervisor handoff has the wrong length"),
            Self::Magic => formatter.write_str("compiler supervisor handoff has the wrong magic"),
            Self::Version => {
                formatter.write_str("compiler supervisor handoff has the wrong version")
            }
            Self::Reserved => {
                formatter.write_str("compiler supervisor handoff has nonzero reserved bytes")
            }
            Self::SubmitterPid => {
                formatter.write_str("compiler supervisor handoff has a zero submitter PID")
            }
            Self::SubmitterIsClient => {
                formatter.write_str("compiler supervisor handoff submitter is the rustc client")
            }
            Self::CredentialMismatch => formatter
                .write_str("compiler supervisor handoff submitter and rustc credentials differ"),
            Self::LaunchManifest(error) => {
                write!(formatter, "invalid nested launch manifest: {error}")
            }
            Self::Identity => {
                formatter.write_str("compiler supervisor handoff identity is invalid")
            }
            Self::Canonical => formatter.write_str("compiler supervisor handoff is not canonical"),
        }
    }
}

impl Error for CompilerExecutionSupervisorHandoffErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LaunchManifest(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;
    use crate::{CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1};

    fn manifest() -> CompilerExecutionServiceLaunchManifestV1 {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 456).unwrap(),
            signing_key.verifying_key().to_bytes(),
        )
        .unwrap();
        CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(200, 1000, 1001).unwrap(),
            &policy,
        )
    }

    #[test]
    fn exact_handoff_round_trips_and_binds_direct_parent() {
        let submitter = CompilerExecutionClientProcessIdentityV1::new(100, 1000, 1001).unwrap();
        let handoff = CompilerExecutionSupervisorHandoffV1::new(submitter, manifest()).unwrap();
        assert_eq!(handoff.submitter(), submitter);
        assert_eq!(handoff.launch_manifest().client().pid(), 200);
        assert_eq!(
            CompilerExecutionSupervisorHandoffV1::decode(handoff.canonical_bytes()).unwrap(),
            handoff
        );
    }

    #[test]
    fn every_byte_mutation_and_wrong_length_rejects() {
        let handoff = CompilerExecutionSupervisorHandoffV1::new(
            CompilerExecutionClientProcessIdentityV1::new(100, 1000, 1001).unwrap(),
            manifest(),
        )
        .unwrap();
        for index in 0..handoff.canonical_bytes().len() {
            let mut bytes = *handoff.canonical_bytes();
            bytes[index] ^= 1;
            assert!(
                CompilerExecutionSupervisorHandoffV1::decode(&bytes).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
        assert!(
            CompilerExecutionSupervisorHandoffV1::decode(
                &handoff.canonical_bytes()[..handoff.canonical_bytes().len() - 1]
            )
            .is_err()
        );
        let mut extended = handoff.canonical_bytes().to_vec();
        extended.push(0);
        assert!(CompilerExecutionSupervisorHandoffV1::decode(&extended).is_err());
    }

    #[test]
    fn independently_resealed_invalid_relationships_reject() {
        let handoff = CompilerExecutionSupervisorHandoffV1::new(
            CompilerExecutionClientProcessIdentityV1::new(100, 1000, 1001).unwrap(),
            manifest(),
        )
        .unwrap();
        for (offset, value, expected) in [
            (
                24,
                0_u32,
                CompilerExecutionSupervisorHandoffErrorV1::SubmitterPid,
            ),
            (
                24,
                200,
                CompilerExecutionSupervisorHandoffErrorV1::SubmitterIsClient,
            ),
            (
                28,
                999,
                CompilerExecutionSupervisorHandoffErrorV1::CredentialMismatch,
            ),
            (
                32,
                999,
                CompilerExecutionSupervisorHandoffErrorV1::CredentialMismatch,
            ),
        ] {
            let mut bytes = *handoff.canonical_bytes();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            reseal(&mut bytes);
            assert_eq!(
                CompilerExecutionSupervisorHandoffV1::decode(&bytes),
                Err(expected)
            );
        }
        let mut reserved = *handoff.canonical_bytes();
        reserved[36] = 1;
        reseal(&mut reserved);
        assert_eq!(
            CompilerExecutionSupervisorHandoffV1::decode(&reserved),
            Err(CompilerExecutionSupervisorHandoffErrorV1::Reserved)
        );
    }

    fn reseal(bytes: &mut [u8; COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1]) {
        let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    }
}

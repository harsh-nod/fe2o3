//! Canonical direct-parent rustc handoff to the protected supervisor.

use std::{error::Error, fmt};

use crate::{
    CompilerExecutionClientProcessIdentityV1, CompilerExecutionServiceLaunchManifestErrorV1,
    CompilerExecutionServiceLaunchManifestV1, supervisor_handoff_codec as codec,
};

const SHA256_BYTES: usize = 32;

/// Exact canonical byte length of one direct-parent supervisor handoff record.
pub const COMPILER_EXECUTION_SUPERVISOR_HANDOFF_BYTES_V1: usize = codec::BYTES;

/// Domain-separated identity of one canonical protected-supervisor handoff.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionSupervisorHandoffIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionSupervisorHandoffIdentityV1 {
    pub(crate) const fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Independently rederives this identity from exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        codec::matches(self.0, bytes)
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
        let frame = codec::encode(
            submitter,
            launch_manifest.client(),
            launch_manifest.canonical_bytes(),
        )?;
        Ok(Self::from_parts(frame, launch_manifest))
    }

    fn from_parts(
        frame: codec::Frame,
        launch_manifest: CompilerExecutionServiceLaunchManifestV1,
    ) -> Self {
        Self {
            submitter: frame.submitter,
            launch_manifest,
            identity: CompilerExecutionSupervisorHandoffIdentityV1(frame.identity),
            bytes: frame.bytes,
        }
    }

    /// Strictly decodes and independently re-encodes one complete canonical handoff.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionSupervisorHandoffErrorV1> {
        let (frame, launch) = codec::decode(bytes)?;
        Ok(Self::from_parts(
            frame,
            CompilerExecutionServiceLaunchManifestV1::from_record(launch),
        ))
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
    use crate::supervisor_handoff_codec::{PREIMAGE_BYTES, derive_identity};
    use crate::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1,
    };

    fn manifest() -> CompilerExecutionServiceLaunchManifestV1 {
        let signing_key = SigningKey::from_bytes(&[7; 32]);
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 456).unwrap(),
            signing_key.verifying_key().to_bytes(),
            SigningKey::from_bytes(&[8; 32]).verifying_key().to_bytes(),
        )
        .unwrap();
        CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(200, 1000, 1001).unwrap(),
            CompilerExecutionExternalAnchorServiceIdentityV1::new(6_000, 7_000).unwrap(),
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

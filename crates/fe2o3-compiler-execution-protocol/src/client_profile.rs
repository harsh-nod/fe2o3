#![forbid(unsafe_code)]

use std::{error::Error, fmt};

use crate::client_profile_codec as codec;

use crate::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1, CompilerExecutionAttestationErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityErrorV1,
    CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerPolicyV1,
};

const HEADER_BYTES: usize = 8 + 2 + 2 + 4;
const IDENTITY_BYTES: usize = 32;
const PREIMAGE_BYTES: usize =
    HEADER_BYTES + 4 + 4 + 4 + 4 + COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V1;

/// Exact canonical byte length of one compiler-execution client profile V1.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1: usize = PREIMAGE_BYTES + IDENTITY_BYTES;

/// Domain-separated identity of one canonical compiler-execution client profile.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionClientProfileIdentityV1([u8; IDENTITY_BYTES]);

impl CompilerExecutionClientProfileIdentityV1 {
    /// Returns the exact domain-separated SHA-256 identity bytes.
    pub const fn as_bytes(&self) -> &[u8; IDENTITY_BYTES] {
        &self.0
    }

    /// Independently rederives this identity from exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionClientProfileIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionClientProfileIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Public trust configuration used by Cargo to authenticate the sole protected supervisor.
///
/// This record contains only the dedicated non-root supervisor identity, dedicated external-anchor
/// service identity, and caller-pinned issuer policy. It grants no compiler, signing, publication,
/// loading, launch, or execution authority.
#[derive(Clone, Eq, PartialEq)]
pub struct CompilerExecutionClientProfileV1 {
    supervisor_uid: u32,
    supervisor_gid: u32,
    external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
    policy: CompilerExecutionIssuerPolicyV1,
    identity: CompilerExecutionClientProfileIdentityV1,
    canonical_bytes: [u8; COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1],
}

impl CompilerExecutionClientProfileV1 {
    /// Constructs one exact client profile for a dedicated non-root supervisor identity.
    pub fn new(
        supervisor_uid: u32,
        supervisor_gid: u32,
        external_anchor_service: CompilerExecutionExternalAnchorServiceIdentityV1,
        policy: CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, CompilerExecutionClientProfileErrorV1> {
        codec::validate_credentials(supervisor_uid, supervisor_gid)?;
        if !policy
            .identity()
            .matches_canonical_bytes(policy.canonical_bytes())
        {
            return Err(CompilerExecutionClientProfileErrorV1::PolicyIdentity);
        }

        let (bytes, identity) = codec::V1.encode(
            supervisor_uid,
            supervisor_gid,
            external_anchor_service,
            policy.canonical_bytes(),
        );
        let identity = CompilerExecutionClientProfileIdentityV1(identity);

        Ok(Self {
            supervisor_uid,
            supervisor_gid,
            external_anchor_service,
            policy,
            identity,
            canonical_bytes: bytes,
        })
    }

    /// Strictly decodes one exact canonical client profile.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionClientProfileErrorV1> {
        let parsed = codec::V1.parse(bytes)?;
        let policy = CompilerExecutionIssuerPolicyV1::decode(parsed.policy)
            .map_err(CompilerExecutionClientProfileErrorV1::Policy)?;
        codec::V1.check_identity(bytes)?;
        let decoded = Self::new(parsed.uid, parsed.gid, parsed.service, policy)?;
        if decoded.canonical_bytes.as_slice() != bytes {
            return Err(CompilerExecutionClientProfileErrorV1::Canonical);
        }
        Ok(decoded)
    }

    /// Returns the expected protected-supervisor effective UID.
    pub const fn supervisor_uid(&self) -> u32 {
        self.supervisor_uid
    }

    /// Returns the expected protected-supervisor effective GID.
    pub const fn supervisor_gid(&self) -> u32 {
        self.supervisor_gid
    }

    /// Returns the exact pinned external-anchor service credential identity.
    pub const fn external_anchor_service(
        &self,
    ) -> CompilerExecutionExternalAnchorServiceIdentityV1 {
        self.external_anchor_service
    }

    /// Returns the exact caller-pinned issuer policy.
    pub const fn policy(&self) -> &CompilerExecutionIssuerPolicyV1 {
        &self.policy
    }

    /// Returns the complete profile identity.
    pub const fn identity(&self) -> CompilerExecutionClientProfileIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical profile bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1] {
        &self.canonical_bytes
    }
}

impl fmt::Debug for CompilerExecutionClientProfileV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CompilerExecutionClientProfileV1")
            .field("supervisor_uid", &self.supervisor_uid)
            .field("supervisor_gid", &self.supervisor_gid)
            .field("external_anchor_service", &self.external_anchor_service)
            .field("policy", &self.policy)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

/// Stable failure categories for canonical client-profile construction and decoding.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerExecutionClientProfileErrorV1 {
    /// The encoded record has the wrong exact length.
    Length,
    /// The encoded record has the wrong magic.
    Magic,
    /// The encoded record has an unsupported version.
    Version(u16),
    /// The encoded record sets unsupported flags.
    UnsupportedFlags(u16),
    /// The encoded record declares a noncanonical length.
    DeclaredLength,
    /// UID zero or the Linux `-1` sentinel cannot identify the dedicated supervisor.
    InvalidSupervisorUid,
    /// GID zero or the Linux `-1` sentinel cannot identify the dedicated supervisor.
    InvalidSupervisorGid,
    /// The pinned external-anchor service credential identity is invalid.
    ExternalAnchorServiceIdentity(CompilerExecutionExternalAnchorServiceIdentityErrorV1),
    /// The nested issuer policy is malformed.
    Policy(CompilerExecutionAttestationErrorV1),
    /// The nested policy does not carry its independently rederived identity.
    PolicyIdentity,
    /// The terminal profile identity is zero or does not match the exact preimage.
    Identity,
    /// The decoded fields do not re-encode to byte-identical canonical form.
    Canonical,
}

impl fmt::Display for CompilerExecutionClientProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length => {
                formatter.write_str("compiler-execution client profile length mismatch")
            }
            Self::Magic => formatter.write_str("compiler-execution client profile magic mismatch"),
            Self::Version(version) => write!(
                formatter,
                "unsupported compiler-execution client profile version {version}"
            ),
            Self::UnsupportedFlags(flags) => write!(
                formatter,
                "unsupported compiler-execution client profile flags {flags:#x}"
            ),
            Self::DeclaredLength => {
                formatter.write_str("compiler-execution client profile declared length mismatch")
            }
            Self::InvalidSupervisorUid => {
                formatter.write_str("invalid compiler-execution supervisor UID")
            }
            Self::InvalidSupervisorGid => {
                formatter.write_str("invalid compiler-execution supervisor GID")
            }
            Self::ExternalAnchorServiceIdentity(error) => {
                write!(
                    formatter,
                    "invalid compiler external-anchor service identity: {error}"
                )
            }
            Self::Policy(error) => write!(formatter, "invalid compiler-execution policy: {error}"),
            Self::PolicyIdentity => {
                formatter.write_str("compiler-execution policy identity mismatch")
            }
            Self::Identity => {
                formatter.write_str("compiler-execution client profile identity mismatch")
            }
            Self::Canonical => {
                formatter.write_str("compiler-execution client profile is not canonical")
            }
        }
    }
}

impl Error for CompilerExecutionClientProfileErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ExternalAnchorServiceIdentity(error) => Some(error),
            Self::Policy(error) => Some(error),
            _ => None,
        }
    }
}

fn derive_identity(preimage: &[u8]) -> [u8; IDENTITY_BYTES] {
    codec::V1.identity(preimage)
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::*;
    use crate::CompilerExecutionIssuerMeasurementV1;

    fn policy(seed: u8) -> CompilerExecutionIssuerPolicyV1 {
        let key = SigningKey::from_bytes(&[seed; 32]);
        CompilerExecutionIssuerPolicyV1::new(
            u64::from(seed),
            CompilerExecutionIssuerMeasurementV1::new([seed + 1; 32], 123).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([seed + 2; 32], 456).unwrap(),
            key.verifying_key().to_bytes(),
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
    fn exact_profile_round_trips_and_binds_every_byte() {
        let profile = CompilerExecutionClientProfileV1::new(
            1_234,
            5_678,
            external_anchor_service(1),
            policy(7),
        )
        .unwrap();
        assert_eq!(
            profile.canonical_bytes().len(),
            COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1
        );
        assert_eq!(profile.supervisor_uid(), 1_234);
        assert_eq!(profile.supervisor_gid(), 5_678);
        assert_eq!(
            profile.external_anchor_service(),
            external_anchor_service(1)
        );
        assert_eq!(profile.policy(), &policy(7));
        assert!(
            profile
                .identity()
                .matches_canonical_bytes(profile.canonical_bytes())
        );
        assert_eq!(
            CompilerExecutionClientProfileV1::decode(profile.canonical_bytes()).unwrap(),
            profile
        );

        for index in 0..COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V1 {
            let mut mutated = *profile.canonical_bytes();
            mutated[index] ^= 1;
            assert!(
                CompilerExecutionClientProfileV1::decode(&mutated).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
    }

    #[test]
    fn invalid_service_identities_fail_closed() {
        for uid in [0, u32::MAX] {
            assert!(matches!(
                CompilerExecutionClientProfileV1::new(
                    uid,
                    5_678,
                    external_anchor_service(1),
                    policy(7)
                ),
                Err(CompilerExecutionClientProfileErrorV1::InvalidSupervisorUid)
            ));
        }
        for gid in [0, u32::MAX] {
            assert!(matches!(
                CompilerExecutionClientProfileV1::new(
                    1_234,
                    gid,
                    external_anchor_service(1),
                    policy(7)
                ),
                Err(CompilerExecutionClientProfileErrorV1::InvalidSupervisorGid)
            ));
        }

        let profile = CompilerExecutionClientProfileV1::new(
            1_234,
            5_678,
            external_anchor_service(1),
            policy(7),
        )
        .unwrap();
        for range in [24..28, 28..32] {
            let mut bytes = *profile.canonical_bytes();
            bytes[range].fill(0);
            let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
            bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
            assert!(matches!(
                CompilerExecutionClientProfileV1::decode(&bytes),
                Err(CompilerExecutionClientProfileErrorV1::ExternalAnchorServiceIdentity(_))
            ));
        }
    }

    #[test]
    fn independently_resealed_substitutions_remain_distinct() {
        let first = CompilerExecutionClientProfileV1::new(
            1_234,
            5_678,
            external_anchor_service(1),
            policy(7),
        )
        .unwrap();
        let uid = CompilerExecutionClientProfileV1::new(
            1_235,
            5_678,
            external_anchor_service(1),
            policy(7),
        )
        .unwrap();
        let gid = CompilerExecutionClientProfileV1::new(
            1_234,
            5_679,
            external_anchor_service(1),
            policy(7),
        )
        .unwrap();
        let anchor = CompilerExecutionClientProfileV1::new(
            1_234,
            5_678,
            external_anchor_service(2),
            policy(7),
        )
        .unwrap();
        let issuer = CompilerExecutionClientProfileV1::new(
            1_234,
            5_678,
            external_anchor_service(1),
            policy(8),
        )
        .unwrap();
        assert_ne!(first.identity(), uid.identity());
        assert_ne!(first.identity(), gid.identity());
        assert_ne!(first.identity(), anchor.identity());
        assert_ne!(first.identity(), issuer.identity());
    }
}

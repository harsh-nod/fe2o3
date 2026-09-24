//! Canonical protected-issuer readiness evidence.

use std::{error::Error, fmt};

use crate::{
    CompilerExecutionIssuerPolicyIdentityV1, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionServiceLaunchManifestIdentityV1, CompilerExecutionServiceLaunchManifestV1,
    service_ready_codec as codec,
};

const SHA256_BYTES: usize = 32;

/// Exact canonical byte length of one protected-issuer readiness record.
pub const COMPILER_EXECUTION_SERVICE_READY_BYTES_V1: usize = codec::BYTES;

/// Domain-separated identity of one exact readiness record.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionServiceReadyIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionServiceReadyIdentityV1 {
    pub(crate) const fn from_bytes_for_protocol(bytes: [u8; SHA256_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the exact identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }
}

impl fmt::Debug for CompilerExecutionServiceReadyIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionServiceReadyIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Canonical inert evidence that the exact protected issuer completed admission and recovery.
///
/// This record grants no process, signing, compiler, publication, loading, launch, or execution
/// authority. Its private pipe provenance and exact child liveness remain supervisor obligations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionServiceReadyV1 {
    issuer_pid: u32,
    launch_manifest_identity: CompilerExecutionServiceLaunchManifestIdentityV1,
    policy_identity: CompilerExecutionIssuerPolicyIdentityV1,
    identity: CompilerExecutionServiceReadyIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1],
}

impl CompilerExecutionServiceReadyV1 {
    /// Constructs readiness for one exact admitted issuer occurrence.
    pub fn new(
        issuer_pid: u32,
        launch: &CompilerExecutionServiceLaunchManifestV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> Result<Self, CompilerExecutionServiceReadyErrorV1> {
        if issuer_pid == 0 {
            return Err(CompilerExecutionServiceReadyErrorV1::IssuerPid);
        }
        if !launch.matches_policy(policy) {
            return Err(CompilerExecutionServiceReadyErrorV1::PolicyMismatch);
        }
        Ok(Self::from_record(codec::encode(
            issuer_pid,
            *launch.identity().as_bytes(),
            *policy.identity().as_bytes(),
        )))
    }

    fn from_record(record: codec::Record) -> Self {
        Self {
            issuer_pid: record.issuer_pid,
            launch_manifest_identity:
                CompilerExecutionServiceLaunchManifestIdentityV1::from_bytes_for_protocol(
                    record.launch_manifest,
                ),
            policy_identity: CompilerExecutionIssuerPolicyIdentityV1::from_bytes_for_protocol(
                record.policy,
            ),
            identity: CompilerExecutionServiceReadyIdentityV1(record.identity),
            bytes: record.bytes,
        }
    }

    /// Strictly decodes and independently re-encodes one readiness record.
    pub fn decode(bytes: &[u8]) -> Result<Self, CompilerExecutionServiceReadyErrorV1> {
        codec::decode(bytes).map(Self::from_record)
    }

    /// Returns the exact protected issuer PID.
    pub const fn issuer_pid(&self) -> u32 {
        self.issuer_pid
    }

    /// Returns the exact launch-manifest identity admitted by the issuer.
    pub const fn launch_manifest_identity(
        &self,
    ) -> CompilerExecutionServiceLaunchManifestIdentityV1 {
        self.launch_manifest_identity
    }

    /// Returns the exact issuer-policy identity admitted by the issuer.
    pub const fn policy_identity(&self) -> CompilerExecutionIssuerPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the terminal readiness identity.
    pub const fn identity(&self) -> CompilerExecutionServiceReadyIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical bytes.
    pub const fn canonical_bytes(&self) -> &[u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1] {
        &self.bytes
    }

    /// Requires exact agreement with one supervisor-retained launch and policy.
    pub fn matches_launch(
        &self,
        issuer_pid: u32,
        launch: &CompilerExecutionServiceLaunchManifestV1,
        policy: &CompilerExecutionIssuerPolicyV1,
    ) -> bool {
        self.issuer_pid == issuer_pid
            && self.launch_manifest_identity == launch.identity()
            && self.policy_identity == policy.identity()
            && launch.matches_policy(policy)
    }
}

/// Stable strict readiness codec failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerExecutionServiceReadyErrorV1 {
    Length,
    Magic,
    Version,
    Reserved,
    IssuerPid,
    LaunchManifestIdentity,
    PolicyIdentity,
    PolicyMismatch,
    Identity,
    Canonical,
}

impl fmt::Display for CompilerExecutionServiceReadyErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "compiler service readiness has the wrong length",
            Self::Magic => "compiler service readiness has the wrong magic",
            Self::Version => "compiler service readiness has the wrong version",
            Self::Reserved => "compiler service readiness has nonzero reserved bytes",
            Self::IssuerPid => "compiler service readiness has a zero issuer PID",
            Self::LaunchManifestIdentity => {
                "compiler service readiness has an invalid launch-manifest identity"
            }
            Self::PolicyIdentity => "compiler service readiness has an invalid policy identity",
            Self::PolicyMismatch => "compiler service readiness launch names another policy",
            Self::Identity => "compiler service readiness identity is invalid",
            Self::Canonical => "compiler service readiness is not canonical",
        })
    }
}

impl Error for CompilerExecutionServiceReadyErrorV1 {}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use super::codec::{PREIMAGE_BYTES, derive_identity};
    use super::*;
    use crate::{
        CompilerExecutionClientProcessIdentityV1, CompilerExecutionExternalAnchorServiceIdentityV1,
        CompilerExecutionIssuerMeasurementV1,
    };

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

    fn launch(seed: u8) -> CompilerExecutionServiceLaunchManifestV1 {
        CompilerExecutionServiceLaunchManifestV1::new(
            CompilerExecutionClientProcessIdentityV1::new(1234, 1000, 1001).unwrap(),
            CompilerExecutionExternalAnchorServiceIdentityV1::new(6_000, 7_000).unwrap(),
            &policy(seed),
        )
    }

    #[test]
    fn exact_readiness_round_trips_and_matches_only_the_exact_launch() {
        let issuer_policy = policy(7);
        let manifest = launch(7);
        let ready = CompilerExecutionServiceReadyV1::new(5678, &manifest, &issuer_policy).unwrap();
        assert!(ready.matches_launch(5678, &manifest, &issuer_policy));
        assert!(!ready.matches_launch(5679, &manifest, &issuer_policy));
        assert!(!ready.matches_launch(5678, &launch(8), &policy(8)));
        assert_eq!(
            CompilerExecutionServiceReadyV1::decode(ready.canonical_bytes()).unwrap(),
            ready
        );
        assert_eq!(
            CompilerExecutionServiceReadyV1::new(0, &manifest, &issuer_policy),
            Err(CompilerExecutionServiceReadyErrorV1::IssuerPid)
        );
        assert_eq!(
            CompilerExecutionServiceReadyV1::new(5678, &launch(8), &issuer_policy),
            Err(CompilerExecutionServiceReadyErrorV1::PolicyMismatch)
        );
    }

    #[test]
    fn every_mutation_wrong_length_and_resealed_invalid_fields_reject() {
        let ready = CompilerExecutionServiceReadyV1::new(5678, &launch(7), &policy(7)).unwrap();
        for index in 0..ready.canonical_bytes().len() {
            let mut bytes = *ready.canonical_bytes();
            bytes[index] ^= 1;
            assert!(
                CompilerExecutionServiceReadyV1::decode(&bytes).is_err(),
                "mutation at byte {index} was accepted"
            );
        }
        assert!(
            CompilerExecutionServiceReadyV1::decode(
                &ready.canonical_bytes()[..COMPILER_EXECUTION_SERVICE_READY_BYTES_V1 - 1]
            )
            .is_err()
        );
        let mut extended = ready.canonical_bytes().to_vec();
        extended.push(0);
        assert!(CompilerExecutionServiceReadyV1::decode(&extended).is_err());

        for (range, expected) in [
            (16..20, CompilerExecutionServiceReadyErrorV1::IssuerPid),
            (
                24..56,
                CompilerExecutionServiceReadyErrorV1::LaunchManifestIdentity,
            ),
            (56..88, CompilerExecutionServiceReadyErrorV1::PolicyIdentity),
        ] {
            let mut bytes = *ready.canonical_bytes();
            bytes[range].fill(0);
            reseal(&mut bytes);
            assert_eq!(
                CompilerExecutionServiceReadyV1::decode(&bytes),
                Err(expected)
            );
        }
        for offset in [10, 20] {
            let mut bytes = *ready.canonical_bytes();
            bytes[offset] = 1;
            reseal(&mut bytes);
            assert_eq!(
                CompilerExecutionServiceReadyV1::decode(&bytes),
                Err(CompilerExecutionServiceReadyErrorV1::Reserved)
            );
        }
    }

    fn reseal(bytes: &mut [u8; COMPILER_EXECUTION_SERVICE_READY_BYTES_V1]) {
        let identity = derive_identity(&bytes[..PREIMAGE_BYTES]);
        bytes[PREIMAGE_BYTES..].copy_from_slice(&identity);
    }
}

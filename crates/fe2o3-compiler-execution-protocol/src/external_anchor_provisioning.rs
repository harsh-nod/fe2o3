#![forbid(unsafe_code)]

//! Canonical trust binding for the unprivileged external-anchor provisioning helper.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CompilerExecutionExternalAnchorDeploymentIdentityV1,
    CompilerExecutionExternalAnchorDeploymentV1, CompilerExecutionIssuerMeasurementV1,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1,
};

const HEADER_BYTES: usize = 24;
const PREIMAGE_BYTES: usize = 96;
const SHA256_BYTES: usize = 32;
const MAGIC: [u8; 8] = *b"F2O3CEP1";
const VERSION_V1: u16 = 1;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/COMPILER-EXECUTION-EXTERNAL-ANCHOR-PROVISIONING/V1\0";

/// Exact canonical byte length of one external-anchor provisioning manifest.
pub const COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1: usize =
    PREIMAGE_BYTES + SHA256_BYTES;

/// Domain-separated identity of one canonical external-anchor provisioning manifest.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CompilerExecutionExternalAnchorProvisioningIdentityV1([u8; SHA256_BYTES]);

impl CompilerExecutionExternalAnchorProvisioningIdentityV1 {
    /// Returns the exact domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Independently rederives this identity from exact canonical bytes.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        bytes.len() == COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1
            && bytes[PREIMAGE_BYTES..] == self.0
            && derive_identity(&bytes[..PREIMAGE_BYTES]) == self.0
    }
}

impl fmt::Debug for CompilerExecutionExternalAnchorProvisioningIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("CompilerExecutionExternalAnchorProvisioningIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Immutable trust configuration for one unprivileged anchor provisioning helper.
///
/// The manifest binds one exact external-anchor deployment to one bounded helper executable. It
/// contains no secret key, state, path, descriptor, process, endpoint, launch, or GPU authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerExecutionExternalAnchorProvisioningV1 {
    deployment_identity: CompilerExecutionExternalAnchorDeploymentIdentityV1,
    helper: CompilerExecutionIssuerMeasurementV1,
    identity: CompilerExecutionExternalAnchorProvisioningIdentityV1,
    bytes: [u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1],
}

impl CompilerExecutionExternalAnchorProvisioningV1 {
    /// Binds one deployment to the exact trusted helper measurement.
    pub fn new(
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
        helper: CompilerExecutionIssuerMeasurementV1,
    ) -> Result<Self, CompilerExecutionExternalAnchorProvisioningErrorV1> {
        Self::from_parts(deployment.identity(), helper)
    }

    fn from_parts(
        deployment_identity: CompilerExecutionExternalAnchorDeploymentIdentityV1,
        helper: CompilerExecutionIssuerMeasurementV1,
    ) -> Result<Self, CompilerExecutionExternalAnchorProvisioningErrorV1> {
        if helper.byte_len() > MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1 {
            return Err(CompilerExecutionExternalAnchorProvisioningErrorV1::HelperMeasurement);
        }
        let mut bytes = [0_u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1];
        encode_header(&mut bytes);
        bytes[24..56].copy_from_slice(deployment_identity.as_bytes());
        bytes[56..88].copy_from_slice(&helper.sha256());
        bytes[88..96].copy_from_slice(&helper.byte_len().to_le_bytes());
        let identity = CompilerExecutionExternalAnchorProvisioningIdentityV1(derive_identity(
            &bytes[..PREIMAGE_BYTES],
        ));
        bytes[PREIMAGE_BYTES..].copy_from_slice(identity.as_bytes());
        Ok(Self {
            deployment_identity,
            helper,
            identity,
            bytes,
        })
    }

    /// Decodes only an exact canonical provisioning manifest.
    pub fn decode(
        bytes: &[u8],
    ) -> Result<Self, CompilerExecutionExternalAnchorProvisioningErrorV1> {
        if bytes.len() != COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1 {
            return Err(CompilerExecutionExternalAnchorProvisioningErrorV1::Length);
        }
        validate_header(bytes)?;
        let deployment_identity =
            CompilerExecutionExternalAnchorDeploymentIdentityV1::from_bytes_for_protocol(
                bytes[24..56]
                    .try_into()
                    .expect("external-anchor deployment identity has fixed width"),
            )
            .ok_or(CompilerExecutionExternalAnchorProvisioningErrorV1::DeploymentIdentity)?;
        let helper = CompilerExecutionIssuerMeasurementV1::new(
            bytes[56..88]
                .try_into()
                .expect("external-anchor provisioning helper digest has fixed width"),
            read_u64(bytes, 88),
        )
        .map_err(|_| CompilerExecutionExternalAnchorProvisioningErrorV1::HelperMeasurement)?;
        if helper.byte_len() > MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1 {
            return Err(CompilerExecutionExternalAnchorProvisioningErrorV1::HelperMeasurement);
        }
        let identity = CompilerExecutionExternalAnchorProvisioningIdentityV1(
            bytes[PREIMAGE_BYTES..]
                .try_into()
                .expect("external-anchor provisioning identity has fixed width"),
        );
        if !identity.matches_canonical_bytes(bytes) {
            return Err(CompilerExecutionExternalAnchorProvisioningErrorV1::Identity);
        }
        let canonical = Self::from_parts(deployment_identity, helper)?;
        if canonical.bytes.as_slice() != bytes {
            return Err(CompilerExecutionExternalAnchorProvisioningErrorV1::Canonical);
        }
        Ok(canonical)
    }

    /// Returns the exact external-anchor deployment identity.
    pub const fn deployment_identity(&self) -> CompilerExecutionExternalAnchorDeploymentIdentityV1 {
        self.deployment_identity
    }

    /// Returns the exact trusted provisioning-helper measurement.
    pub const fn helper(&self) -> CompilerExecutionIssuerMeasurementV1 {
        self.helper
    }

    /// Returns the complete provisioning-manifest identity.
    pub const fn identity(&self) -> CompilerExecutionExternalAnchorProvisioningIdentityV1 {
        self.identity
    }

    /// Returns the exact canonical bytes.
    pub const fn canonical_bytes(
        &self,
    ) -> &[u8; COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1] {
        &self.bytes
    }

    /// Requires exact agreement with the external-anchor deployment.
    pub fn matches_deployment(
        &self,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
    ) -> bool {
        self.deployment_identity == deployment.identity()
    }

    /// Requires exact deployment and helper agreement.
    pub fn matches_deployment_and_helper(
        &self,
        deployment: &CompilerExecutionExternalAnchorDeploymentV1,
        helper: CompilerExecutionIssuerMeasurementV1,
    ) -> bool {
        self.matches_deployment(deployment) && self.helper == helper
    }
}

fn encode_header(bytes: &mut [u8]) {
    bytes[..8].copy_from_slice(&MAGIC);
    bytes[8..10].copy_from_slice(&VERSION_V1.to_le_bytes());
    bytes[12..16].copy_from_slice(
        &(COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1 as u32).to_le_bytes(),
    );
}

fn validate_header(bytes: &[u8]) -> Result<(), CompilerExecutionExternalAnchorProvisioningErrorV1> {
    if bytes[..8] != MAGIC
        || read_u16(bytes, 8) != VERSION_V1
        || read_u32(bytes, 12) != COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V1 as u32
        || bytes[10..12].iter().any(|byte| *byte != 0)
        || bytes[16..HEADER_BYTES].iter().any(|byte| *byte != 0)
    {
        return Err(CompilerExecutionExternalAnchorProvisioningErrorV1::Header);
    }
    Ok(())
}

fn derive_identity(preimage: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN);
    digest.update((preimage.len() as u64).to_le_bytes());
    digest.update(preimage);
    digest.finalize().into()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

/// Stable external-anchor provisioning-manifest rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerExecutionExternalAnchorProvisioningErrorV1 {
    Length,
    Header,
    DeploymentIdentity,
    HelperMeasurement,
    Identity,
    Canonical,
}

impl fmt::Display for CompilerExecutionExternalAnchorProvisioningErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "invalid compiler external-anchor provisioning length",
            Self::Header => "invalid compiler external-anchor provisioning header",
            Self::DeploymentIdentity => "invalid external-anchor deployment identity",
            Self::HelperMeasurement => "invalid external-anchor provisioning-helper measurement",
            Self::Identity => "invalid compiler external-anchor provisioning identity",
            Self::Canonical => "noncanonical compiler external-anchor provisioning manifest",
        })
    }
}

impl Error for CompilerExecutionExternalAnchorProvisioningErrorV1 {}

#[cfg(test)]
mod tests {
    use ed25519_dalek::SigningKey;

    use crate::{
        CompilerExecutionExternalAnchorServiceIdentityV1, CompilerExecutionIssuerPolicyV1,
        CompilerExecutionSupervisorDeploymentV1,
    };

    use super::*;

    fn deployment(seed: u8) -> CompilerExecutionExternalAnchorDeploymentV1 {
        let policy = CompilerExecutionIssuerPolicyV1::new(
            1,
            CompilerExecutionIssuerMeasurementV1::new([1; 32], 1).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([2; 32], 2).unwrap(),
            SigningKey::from_bytes(&[3; 32]).verifying_key().to_bytes(),
            SigningKey::from_bytes(&[seed; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        let supervisor = CompilerExecutionSupervisorDeploymentV1::new(
            1001,
            1002,
            CompilerExecutionExternalAnchorServiceIdentityV1::new(1003, 1004).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([3; 32], 3).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([4; 32], 4).unwrap(),
            &policy,
        )
        .unwrap();
        CompilerExecutionExternalAnchorDeploymentV1::new(
            &supervisor,
            &policy,
            CompilerExecutionIssuerMeasurementV1::new([5; 32], 5).unwrap(),
        )
        .unwrap()
    }

    fn helper(seed: u8) -> CompilerExecutionIssuerMeasurementV1 {
        CompilerExecutionIssuerMeasurementV1::new([seed; 32], 32768).unwrap()
    }

    #[test]
    fn provisioning_round_trips_and_binds_deployment_and_helper() {
        let expected_deployment = deployment(7);
        let provisioning =
            CompilerExecutionExternalAnchorProvisioningV1::new(&expected_deployment, helper(8))
                .unwrap();
        let decoded =
            CompilerExecutionExternalAnchorProvisioningV1::decode(provisioning.canonical_bytes())
                .unwrap();
        assert_eq!(decoded, provisioning);
        assert!(decoded.matches_deployment_and_helper(&expected_deployment, helper(8)));
        assert!(!decoded.matches_deployment(&deployment(9)));
        assert!(!decoded.matches_deployment_and_helper(&expected_deployment, helper(10)));
        assert!(
            decoded
                .identity()
                .matches_canonical_bytes(decoded.canonical_bytes())
        );
    }

    #[test]
    fn every_byte_mutation_and_wrong_length_rejects() {
        let canonical =
            CompilerExecutionExternalAnchorProvisioningV1::new(&deployment(7), helper(8)).unwrap();
        for index in 0..canonical.canonical_bytes().len() {
            let mut mutated = *canonical.canonical_bytes();
            mutated[index] ^= 1;
            assert!(
                CompilerExecutionExternalAnchorProvisioningV1::decode(&mutated).is_err(),
                "mutation at byte {index} was admitted"
            );
        }
        for bytes in [
            &canonical.canonical_bytes()[..canonical.canonical_bytes().len() - 1],
            &[canonical.canonical_bytes().as_slice(), &[0]].concat(),
        ] {
            assert_eq!(
                CompilerExecutionExternalAnchorProvisioningV1::decode(bytes),
                Err(CompilerExecutionExternalAnchorProvisioningErrorV1::Length)
            );
        }
    }

    #[test]
    fn invalid_identities_and_oversize_helper_reject_before_identity() {
        let canonical =
            CompilerExecutionExternalAnchorProvisioningV1::new(&deployment(7), helper(8)).unwrap();
        let mut zero_deployment = *canonical.canonical_bytes();
        zero_deployment[24..56].fill(0);
        assert_eq!(
            CompilerExecutionExternalAnchorProvisioningV1::decode(&zero_deployment),
            Err(CompilerExecutionExternalAnchorProvisioningErrorV1::DeploymentIdentity)
        );
        let oversized = CompilerExecutionIssuerMeasurementV1::new(
            [8; 32],
            MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_EXECUTABLE_BYTES_V1 + 1,
        )
        .unwrap();
        assert_eq!(
            CompilerExecutionExternalAnchorProvisioningV1::new(&deployment(7), oversized),
            Err(CompilerExecutionExternalAnchorProvisioningErrorV1::HelperMeasurement)
        );
    }
}

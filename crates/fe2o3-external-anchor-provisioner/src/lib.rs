#![deny(missing_docs, unsafe_code)]
#![doc = include_str!("../README.md")]

#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
compile_error!("fe2o3-external-anchor-provisioner requires Linux x86-64");

#[allow(unsafe_code)]
mod entrypoint;

pub use entrypoint::{
    EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1, EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1,
    EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1, EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1,
    ExternalAnchorProvisioningHelperErrorV1, run_inherited_external_anchor_provisioning_helper_v1,
};

const READY_MAGIC_V1: [u8; 8] = *b"F2O3AHR1";
const READY_VERSION_V1: u16 = 1;

/// Exact byte length of one helper-ready bootstrap record.
pub const EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1: usize = 16;

/// Durable-state disposition reported with the transferred supervisor endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExternalAnchorProvisioningReadyDispositionV1 {
    /// Existing canonical state was admitted without mutation.
    Existing = 1,
    /// Canonical genesis was durably created from exact state-file absence.
    Initialized = 2,
}

/// Exact helper-ready record carried with one `SCM_RIGHTS` supervisor endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExternalAnchorProvisioningReadyV1 {
    disposition: ExternalAnchorProvisioningReadyDispositionV1,
    bytes: [u8; EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1],
}

impl ExternalAnchorProvisioningReadyV1 {
    /// Constructs one canonical ready record.
    pub fn new(disposition: ExternalAnchorProvisioningReadyDispositionV1) -> Self {
        let mut bytes = [0_u8; EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1];
        bytes[..8].copy_from_slice(&READY_MAGIC_V1);
        bytes[8..10].copy_from_slice(&READY_VERSION_V1.to_le_bytes());
        bytes[10] = disposition as u8;
        Self { disposition, bytes }
    }

    /// Strictly decodes one exact canonical record.
    pub fn decode(bytes: &[u8]) -> Result<Self, ExternalAnchorProvisioningReadyErrorV1> {
        if bytes.len() != EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1 {
            return Err(ExternalAnchorProvisioningReadyErrorV1::Length);
        }
        if bytes[..8] != READY_MAGIC_V1
            || u16::from_le_bytes(bytes[8..10].try_into().unwrap()) != READY_VERSION_V1
            || bytes[11..].iter().any(|byte| *byte != 0)
        {
            return Err(ExternalAnchorProvisioningReadyErrorV1::Canonical);
        }
        let disposition = match bytes[10] {
            1 => ExternalAnchorProvisioningReadyDispositionV1::Existing,
            2 => ExternalAnchorProvisioningReadyDispositionV1::Initialized,
            _ => return Err(ExternalAnchorProvisioningReadyErrorV1::Disposition),
        };
        let canonical = Self::new(disposition);
        if canonical.bytes.as_slice() != bytes {
            return Err(ExternalAnchorProvisioningReadyErrorV1::Canonical);
        }
        Ok(canonical)
    }

    /// Returns the durable-state disposition.
    pub const fn disposition(self) -> ExternalAnchorProvisioningReadyDispositionV1 {
        self.disposition
    }

    /// Returns the exact canonical bytes.
    pub const fn canonical_bytes(&self) -> &[u8; EXTERNAL_ANCHOR_PROVISIONING_READY_BYTES_V1] {
        &self.bytes
    }
}

/// Stable rejection for one helper-ready bootstrap record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ExternalAnchorProvisioningReadyErrorV1 {
    /// The record does not have the exact V1 width.
    Length,
    /// The durable-state disposition is unknown.
    Disposition,
    /// Magic, version, or reserved bytes are noncanonical.
    Canonical,
}

impl std::fmt::Display for ExternalAnchorProvisioningReadyErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Length => "invalid external-anchor helper-ready length",
            Self::Disposition => "invalid external-anchor helper-ready disposition",
            Self::Canonical => "noncanonical external-anchor helper-ready record",
        })
    }
}

impl std::error::Error for ExternalAnchorProvisioningReadyErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_record_round_trips_and_rejects_every_mutation() {
        for disposition in [
            ExternalAnchorProvisioningReadyDispositionV1::Existing,
            ExternalAnchorProvisioningReadyDispositionV1::Initialized,
        ] {
            let ready = ExternalAnchorProvisioningReadyV1::new(disposition);
            assert_eq!(
                ExternalAnchorProvisioningReadyV1::decode(ready.canonical_bytes()).unwrap(),
                ready
            );
            for index in 0..ready.canonical_bytes().len() {
                let mut mutated = *ready.canonical_bytes();
                mutated[index] ^= 1;
                assert!(
                    ExternalAnchorProvisioningReadyV1::decode(&mutated).is_err(),
                    "mutation at byte {index} was admitted"
                );
            }
        }
    }

    #[test]
    fn fixed_helper_descriptors_are_disjoint_from_manifest_descriptors() {
        assert_eq!(EXTERNAL_ANCHOR_HELPER_BOOTSTRAP_FD_V1, 3);
        assert_eq!(EXTERNAL_ANCHOR_HELPER_ROOT_FD_V1, 4);
        assert_eq!(EXTERNAL_ANCHOR_HELPER_DAEMON_EXECUTABLE_FD_V1, 5);
        assert_eq!(EXTERNAL_ANCHOR_HELPER_LIFECYCLE_FD_V1, 6);
        assert_eq!(
            fe2o3_compiler_closure_capability::COMPILER_EXECUTION_EXTERNAL_ANCHOR_DEPLOYMENT_FD_V1,
            221
        );
        assert_eq!(
            fe2o3_compiler_closure_capability::COMPILER_EXECUTION_EXTERNAL_ANCHOR_SIGNING_KEY_FD_V1,
            222
        );
        assert_eq!(
            fe2o3_compiler_closure_capability::COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_FD_V1,
            223
        );
    }
}

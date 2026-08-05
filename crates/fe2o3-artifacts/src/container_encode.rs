use crate::{ArtifactContainerV1, DigestAlgorithm};

pub const CONTAINER_MAGIC: [u8; 8] = *b"FE2O3AC\0";
pub const CONTAINER_VERSION: u16 = 1;
pub const CONTAINER_HEADER_BYTES: usize = 24;
pub const PAYLOAD_DESCRIPTOR_BYTES: usize = 40;
pub const MAX_CONTAINER_BYTES: usize = CONTAINER_HEADER_BYTES
    + crate::MAX_MANIFEST_BYTES
    + crate::MAX_EMBEDDED_PAYLOAD_BYTES
    + crate::MAX_CODE_OBJECTS * PAYLOAD_DESCRIPTOR_BYTES;

impl ArtifactContainerV1 {
    /// Encodes the validated container using the canonical v1 binary format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let manifest = self.manifest().to_bytes();
        let payload_bytes: usize = self
            .payloads()
            .iter()
            .map(|payload| payload.bytes().len())
            .sum();
        let capacity = CONTAINER_HEADER_BYTES
            + manifest.len()
            + self.payloads().len() * PAYLOAD_DESCRIPTOR_BYTES
            + payload_bytes;
        let mut bytes = Vec::with_capacity(capacity);

        bytes.extend_from_slice(&CONTAINER_MAGIC);
        bytes.extend_from_slice(&CONTAINER_VERSION.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&digest_algorithm_tag(self.digest_algorithm()).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.payloads().len() as u32).to_le_bytes());
        bytes.extend_from_slice(&manifest);

        for payload in self.payloads() {
            bytes.extend_from_slice(payload.digest().bytes().as_bytes());
            bytes.extend_from_slice(&(payload.bytes().len() as u64).to_le_bytes());
        }
        for payload in self.payloads() {
            bytes.extend_from_slice(payload.bytes());
        }

        debug_assert_eq!(bytes.len(), capacity);
        debug_assert!(bytes.len() <= MAX_CONTAINER_BYTES);
        bytes
    }
}

const fn digest_algorithm_tag(algorithm: DigestAlgorithm) -> u16 {
    match algorithm {
        DigestAlgorithm::Sha256 => 1,
    }
}

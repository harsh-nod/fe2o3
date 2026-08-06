use crate::{
    DigestAlgorithm, DirectLinkBindingExpectationV1, DirectLinkBindingV1,
    DirectLinkBundleEvidenceV1, DirectLinkToolchainIdentityV1, DirectLinkWorkerIdentityV1,
    IdentityText, PayloadDigest,
};

pub const DIRECT_LINK_EVIDENCE_MAGIC: [u8; 8] = *b"FE2O3DL\0";
pub const DIRECT_LINK_EVIDENCE_VERSION: u16 = 1;
pub const DIRECT_LINK_EVIDENCE_HEADER_BYTES: usize = 49;
pub const MAX_DIRECT_LINK_EVIDENCE_BYTES: usize = 2 * 1024 * 1024;

impl DirectLinkBundleEvidenceV1 {
    /// Encodes this evidence using the canonical direct-link V1 format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.bytes(&DIRECT_LINK_EVIDENCE_MAGIC);
        writer.u16(DIRECT_LINK_EVIDENCE_VERSION);
        writer.u16(0);
        writer.payload_digest(self.bundle_index_identity().digest());
        writer.u16(self.bindings().len() as u16);
        writer.u16(0);
        for binding in self.bindings() {
            writer.binding(binding);
        }
        debug_assert!(writer.bytes.len() <= MAX_DIRECT_LINK_EVIDENCE_BYTES);
        writer.bytes
    }

    /// Calculates a digest over the complete canonical evidence record.
    pub fn digest(&self, algorithm: DigestAlgorithm) -> PayloadDigest {
        algorithm.calculate(&self.to_bytes())
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(1024),
        }
    }

    fn bytes(&mut self, value: &[u8]) {
        self.bytes.extend_from_slice(value);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn text(&mut self, value: &IdentityText) {
        self.u16(value.as_str().len() as u16);
        self.bytes(value.as_str().as_bytes());
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        self.u8(digest_algorithm_tag(value.algorithm()));
        self.bytes(value.bytes().as_bytes());
    }

    fn worker(&mut self, tool: &DirectLinkWorkerIdentityV1) {
        self.text(tool.name());
        self.text(tool.version());
        self.payload_digest(tool.executable_digest().digest());
        self.payload_digest(tool.configuration_digest().digest());
    }

    fn toolchain(&mut self, tool: &DirectLinkToolchainIdentityV1) {
        self.text(tool.name());
        self.text(tool.version());
        self.payload_digest(tool.executable_digest().digest());
        self.payload_digest(tool.configuration_digest().digest());
    }

    fn expectation(&mut self, expectation: &DirectLinkBindingExpectationV1) {
        self.payload_digest(expectation.request_identity().digest());
        self.worker(expectation.worker());
        self.toolchain(expectation.toolchain());
        self.payload_digest(expectation.response_identity().digest());
        self.payload_digest(expectation.linked_output_identity().digest());
        self.payload_digest(expectation.finalization_identity().digest());
        self.payload_digest(expectation.finalized_payload_identity().digest());
        self.payload_digest(expectation.ffi_contract_identity().digest());
    }

    fn binding(&mut self, binding: &DirectLinkBindingV1) {
        self.payload_digest(binding.container_identity().digest());
        self.expectation(binding.expectation());
    }
}

pub(crate) const fn digest_algorithm_tag(algorithm: DigestAlgorithm) -> u8 {
    match algorithm {
        DigestAlgorithm::Sha256 => 0,
    }
}

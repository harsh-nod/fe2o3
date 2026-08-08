use crate::{
    ConfigurationEntry, DigestAlgorithm, IdentityText, MeasuredToolIdentity, Name, PayloadDigest,
    ProofOutcome, ProofProperty, ProofRecordV1, TrustedItem, VerificationModelIdentity,
};

pub const PROOF_RECORD_MAGIC: [u8; 8] = *b"FE2O3PR\0";
pub const PROOF_RECORD_VERSION: u16 = 1;
pub const MAX_PROOF_RECORD_BYTES: usize = 1024 * 1024;

impl ProofRecordV1 {
    /// Returns the exact length of this record's canonical v1 encoding without allocating.
    pub fn encoded_len(&self) -> usize {
        const DIGEST_BYTES: usize = 1 + 32;
        let text_len = |value: &str| 2 + value.len();
        let tool_len = |tool: &MeasuredToolIdentity| {
            text_len(tool.name().as_str()) + text_len(tool.version().as_str()) + 2 * DIGEST_BYTES
        };

        let mut length = PROOF_RECORD_MAGIC.len() + 2 + 2 + 13 * DIGEST_BYTES;
        length += 2;
        for entry in self.configuration() {
            length += text_len(entry.key().as_str()) + text_len(entry.value().as_str());
        }
        length += text_len(self.execution().model().version().as_str()) + DIGEST_BYTES;
        length += tool_len(self.execution().verifier());
        length += tool_len(self.execution().solver());
        length += tool_len(self.execution().evidence_recorder());
        length += DIGEST_BYTES + 1;
        length += 2 + self.proved_properties().len();
        length += 2;
        for item in self.trusted_items() {
            length += text_len(item.name().as_str()) + DIGEST_BYTES;
        }
        debug_assert!(length <= MAX_PROOF_RECORD_BYTES);
        length
    }

    /// Encodes this validated record using the canonical proof-record v1 format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::new();
        writer.bytes(&PROOF_RECORD_MAGIC);
        writer.u16(PROOF_RECORD_VERSION);
        writer.u16(0);

        let artifact = self.target().artifact();
        writer.payload_digest(artifact.kernel_id());
        writer.payload_digest(artifact.instance_digest());
        writer.payload_digest(artifact.source_tree_digest());
        writer.payload_digest(artifact.crate_graph_digest());
        writer.payload_digest(artifact.executable_digest());
        writer.payload_digest(artifact.environment_digest());
        writer.payload_digest(artifact.artifact_selection_digest());
        writer.payload_digest(artifact.artifact_contract_digest());
        let contracts = self.target().source_contracts();
        writer.payload_digest(contracts.memory_digest());
        writer.payload_digest(contracts.effects_digest());
        writer.payload_digest(contracts.type_layout_digest());
        writer.payload_digest(contracts.capability_semantics_digest());
        writer.payload_digest(contracts.functional_specification_digest());

        writer.u16(self.configuration().len() as u16);
        for entry in self.configuration() {
            writer.configuration(entry);
        }
        writer.model(self.execution().model());
        writer.measured_tool(self.execution().verifier());
        writer.measured_tool(self.execution().solver());
        writer.measured_tool(self.execution().evidence_recorder());
        writer.payload_digest(self.execution().invocation_digest());
        writer.u8(outcome_tag(self.outcome()));

        writer.u16(self.proved_properties().len() as u16);
        for property in self.proved_properties() {
            writer.u8(property_tag(*property));
        }

        writer.u16(self.trusted_items().len() as u16);
        for item in self.trusted_items() {
            writer.trusted_item(item);
        }

        debug_assert_eq!(writer.bytes.len(), self.encoded_len());
        writer.bytes
    }

    /// Calculates an explicitly identified digest over canonical v1 bytes.
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
            bytes: Vec::with_capacity(512),
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

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }

    fn name(&mut self, value: &Name) {
        self.text(value.as_str());
    }

    fn identity_text(&mut self, value: &IdentityText) {
        self.text(value.as_str());
    }

    fn payload_digest(&mut self, value: PayloadDigest) {
        self.u8(digest_algorithm_tag(value.algorithm()));
        self.bytes(value.bytes().as_bytes());
    }

    fn configuration(&mut self, entry: &ConfigurationEntry) {
        self.name(entry.key());
        self.identity_text(entry.value());
    }

    fn model(&mut self, model: &VerificationModelIdentity) {
        self.identity_text(model.version());
        self.payload_digest(model.axioms_digest());
    }

    fn measured_tool(&mut self, tool: &MeasuredToolIdentity) {
        self.identity_text(tool.name());
        self.identity_text(tool.version());
        self.payload_digest(tool.executable_digest());
        self.payload_digest(tool.configuration_digest());
    }

    fn trusted_item(&mut self, item: &TrustedItem) {
        self.name(item.name());
        self.payload_digest(item.contract_digest());
    }
}

pub(crate) const fn digest_algorithm_tag(value: DigestAlgorithm) -> u8 {
    match value {
        DigestAlgorithm::Sha256 => 0,
    }
}

pub(crate) const fn outcome_tag(value: ProofOutcome) -> u8 {
    match value {
        ProofOutcome::Proved => 0,
        ProofOutcome::Failed => 1,
        ProofOutcome::TimedOut => 2,
    }
}

pub(crate) const fn property_tag(value: ProofProperty) -> u8 {
    match value {
        ProofProperty::Bounds => 0,
        ProofProperty::AddressOverflowFreedom => 1,
        ProofProperty::MemorySafety => 2,
        ProofProperty::Initialization => 3,
        ProofProperty::RaceFreedom => 4,
        ProofProperty::LaunchValidity => 5,
        ProofProperty::FunctionalCorrectness => 6,
    }
}

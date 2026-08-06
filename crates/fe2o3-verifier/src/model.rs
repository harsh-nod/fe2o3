use std::fmt;

pub const MAX_TEXT_BYTES: usize = 256;
pub const MAX_CONFIGURATION_ENTRIES: usize = 256;
pub const MAX_PROPERTIES: usize = 32;
pub const MAX_TRUSTED_ITEMS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Digest([u8; 32]);

impl Digest {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }

    pub fn from_hex(value: &str) -> Result<Self, ModelError> {
        Ok(Self(decode_hex::<32>(value, "digest")?))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CorrelationId([u8; 16]);

impl CorrelationId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        encode_hex(&self.0)
    }

    pub fn from_hex(value: &str) -> Result<Self, ModelError> {
        Ok(Self(decode_hex::<16>(value, "correlation ID")?))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Text(String);

impl Text {
    pub fn new(field: &'static str, value: impl Into<String>) -> Result<Self, ModelError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAX_TEXT_BYTES {
            return Err(ModelError::LengthOutOfRange {
                field,
                max: MAX_TEXT_BYTES,
            });
        }
        if value.trim() != value || !value.bytes().all(|byte| (0x20..=0x7e).contains(&byte)) {
            return Err(ModelError::NonCanonicalText { field });
        }
        Ok(Self(value))
    }

    pub fn identifier(field: &'static str, value: impl Into<String>) -> Result<Self, ModelError> {
        let value = Self::new(field, value)?;
        let mut bytes = value.0.bytes();
        let first = bytes.next().expect("Text rejects empty values");
        if !(first.is_ascii_alphabetic() || first == b'_')
            || !bytes.all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'$')
            })
        {
            return Err(ModelError::InvalidIdentifier { field });
        }
        Ok(value)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ConfigurationEntry {
    key: Text,
    value: Text,
}

impl ConfigurationEntry {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, ModelError> {
        Ok(Self {
            key: Text::identifier("configuration key", key)?,
            value: Text::new("configuration value", value)?,
        })
    }

    pub const fn key(&self) -> &Text {
        &self.key
    }

    pub const fn value(&self) -> &Text {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Configuration(Vec<ConfigurationEntry>);

impl Configuration {
    pub fn new(mut entries: Vec<ConfigurationEntry>) -> Result<Self, ModelError> {
        if entries.len() > MAX_CONFIGURATION_ENTRIES {
            return Err(ModelError::TooManyItems {
                field: "configuration",
                max: MAX_CONFIGURATION_ENTRIES,
            });
        }
        entries.sort_unstable_by(|left, right| left.key.cmp(&right.key));
        if entries.windows(2).any(|pair| pair[0].key == pair[1].key) {
            return Err(ModelError::DuplicateItem {
                field: "configuration key",
            });
        }
        Ok(Self(entries))
    }

    pub fn entries(&self) -> &[ConfigurationEntry] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasuredToolIdentity {
    name: Text,
    version: Text,
    executable_digest: Digest,
    configuration_digest: Digest,
}

impl MeasuredToolIdentity {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        executable_digest: Digest,
        configuration_digest: Digest,
    ) -> Result<Self, ModelError> {
        Ok(Self {
            name: Text::new("tool name", name)?,
            version: Text::new("tool version", version)?,
            executable_digest,
            configuration_digest,
        })
    }

    pub const fn name(&self) -> &Text {
        &self.name
    }

    pub const fn version(&self) -> &Text {
        &self.version
    }

    pub const fn executable_digest(&self) -> Digest {
        self.executable_digest
    }

    pub const fn configuration_digest(&self) -> Digest {
        self.configuration_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionTools {
    verifier: MeasuredToolIdentity,
    solver: MeasuredToolIdentity,
    evidence_recorder: MeasuredToolIdentity,
}

impl ExecutionTools {
    pub const fn new(
        verifier: MeasuredToolIdentity,
        solver: MeasuredToolIdentity,
        evidence_recorder: MeasuredToolIdentity,
    ) -> Self {
        Self {
            verifier,
            solver,
            evidence_recorder,
        }
    }

    pub const fn verifier(&self) -> &MeasuredToolIdentity {
        &self.verifier
    }

    pub const fn solver(&self) -> &MeasuredToolIdentity {
        &self.solver
    }

    pub const fn evidence_recorder(&self) -> &MeasuredToolIdentity {
        &self.evidence_recorder
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationModelIdentity {
    version: Text,
    axioms_digest: Digest,
}

impl VerificationModelIdentity {
    pub fn new(version: impl Into<String>, axioms_digest: Digest) -> Result<Self, ModelError> {
        Ok(Self {
            version: Text::new("verification model version", version)?,
            axioms_digest,
        })
    }

    pub const fn version(&self) -> &Text {
        &self.version
    }

    pub const fn axioms_digest(&self) -> Digest {
        self.axioms_digest
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProofProperty {
    Bounds,
    AddressOverflowFreedom,
    MemorySafety,
    Initialization,
    RaceFreedom,
    LaunchValidity,
    FunctionalCorrectness,
}

impl ProofProperty {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bounds => "bounds",
            Self::AddressOverflowFreedom => "address-overflow-freedom",
            Self::MemorySafety => "memory-safety",
            Self::Initialization => "initialization",
            Self::RaceFreedom => "race-freedom",
            Self::LaunchValidity => "launch-validity",
            Self::FunctionalCorrectness => "functional-correctness",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ModelError> {
        match value {
            "bounds" => Ok(Self::Bounds),
            "address-overflow-freedom" => Ok(Self::AddressOverflowFreedom),
            "memory-safety" => Ok(Self::MemorySafety),
            "initialization" => Ok(Self::Initialization),
            "race-freedom" => Ok(Self::RaceFreedom),
            "launch-validity" => Ok(Self::LaunchValidity),
            "functional-correctness" => Ok(Self::FunctionalCorrectness),
            _ => Err(ModelError::UnknownValue {
                field: "proof property",
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProofOutcome {
    Proved,
    Failed,
    TimedOut,
}

impl ProofOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proved => "proved",
            Self::Failed => "failed",
            Self::TimedOut => "timed-out",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ModelError> {
        match value {
            "proved" => Ok(Self::Proved),
            "failed" => Ok(Self::Failed),
            "timed-out" => Ok(Self::TimedOut),
            _ => Err(ModelError::UnknownValue {
                field: "proof outcome",
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TrustedItem {
    name: Text,
    contract_digest: Digest,
}

impl TrustedItem {
    pub fn new(name: impl Into<String>, contract_digest: Digest) -> Result<Self, ModelError> {
        Ok(Self {
            name: Text::identifier("trusted item name", name)?,
            contract_digest,
        })
    }

    pub const fn name(&self) -> &Text {
        &self.name
    }

    pub const fn contract_digest(&self) -> Digest {
        self.contract_digest
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AxiomPolicy {
    allowed: Vec<TrustedItem>,
}

impl AxiomPolicy {
    pub fn deny_all() -> Self {
        Self { allowed: vec![] }
    }

    pub fn allow_list(mut allowed: Vec<TrustedItem>) -> Result<Self, ModelError> {
        canonicalize_trusted(&mut allowed)?;
        Ok(Self { allowed })
    }

    pub fn allowed(&self) -> &[TrustedItem] {
        &self.allowed
    }

    pub fn validate(&self, requested: &[TrustedItem]) -> Result<(), ModelError> {
        if let Some(item) = requested
            .iter()
            .find(|item| self.allowed.binary_search(item).is_err())
        {
            return Err(ModelError::AxiomRejected(item.name.as_str().to_owned()));
        }
        Ok(())
    }
}

/// Identities matching the target portion of the artifact proof-record model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProofTargetIdentity {
    pub kernel_id: Digest,
    pub instance_digest: Digest,
    pub source_tree_digest: Digest,
    pub crate_graph_digest: Digest,
    pub executable_digest: Digest,
    pub environment_digest: Digest,
    pub artifact_selection_digest: Digest,
    pub artifact_contract_digest: Digest,
    pub memory_contract_digest: Digest,
    pub effects_contract_digest: Digest,
    pub type_layout_digest: Digest,
    pub capability_semantics_digest: Digest,
    pub functional_specification_digest: Digest,
}

impl ProofTargetIdentity {
    pub const fn digests(self) -> [Digest; 13] {
        [
            self.kernel_id,
            self.instance_digest,
            self.source_tree_digest,
            self.crate_graph_digest,
            self.executable_digest,
            self.environment_digest,
            self.artifact_selection_digest,
            self.artifact_contract_digest,
            self.memory_contract_digest,
            self.effects_contract_digest,
            self.type_layout_digest,
            self.capability_semantics_digest,
            self.functional_specification_digest,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRequestV1 {
    correlation_id: CorrelationId,
    target: ProofTargetIdentity,
    configuration: Configuration,
    model: VerificationModelIdentity,
    properties: Vec<ProofProperty>,
    trusted_items: Vec<TrustedItem>,
}

impl ProofRequestV1 {
    pub fn new(
        correlation_id: CorrelationId,
        target: ProofTargetIdentity,
        configuration: Configuration,
        model: VerificationModelIdentity,
        mut properties: Vec<ProofProperty>,
        mut trusted_items: Vec<TrustedItem>,
    ) -> Result<Self, ModelError> {
        canonicalize_properties(&mut properties)?;
        canonicalize_trusted(&mut trusted_items)?;
        Ok(Self {
            correlation_id,
            target,
            configuration,
            model,
            properties,
            trusted_items,
        })
    }

    pub const fn correlation_id(&self) -> CorrelationId {
        self.correlation_id
    }

    pub const fn target(&self) -> ProofTargetIdentity {
        self.target
    }

    pub const fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub const fn model(&self) -> &VerificationModelIdentity {
        &self.model
    }

    pub fn properties(&self) -> &[ProofProperty] {
        &self.properties
    }

    pub fn trusted_items(&self) -> &[TrustedItem] {
        &self.trusted_items
    }

    /// Canonical binary input for the external evidence recorder.
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut writer = Writer::default();
        writer.bytes(b"FE2O3VRQ");
        writer.u16(1);
        writer.bytes(self.correlation_id.as_bytes());
        for digest in self.target.digests() {
            writer.bytes(digest.as_bytes());
        }
        writer.u16(self.configuration.entries().len() as u16);
        for entry in self.configuration.entries() {
            writer.text(entry.key.as_str());
            writer.text(entry.value.as_str());
        }
        writer.text(self.model.version.as_str());
        writer.bytes(self.model.axioms_digest.as_bytes());
        writer.u16(self.properties.len() as u16);
        for property in &self.properties {
            writer.text(property.as_str());
        }
        writer.u16(self.trusted_items.len() as u16);
        for item in &self.trusted_items {
            writer.text(item.name.as_str());
            writer.bytes(item.contract_digest.as_bytes());
        }
        writer.0
    }
}

fn canonicalize_properties(properties: &mut [ProofProperty]) -> Result<(), ModelError> {
    if properties.is_empty() || properties.len() > MAX_PROPERTIES {
        return Err(ModelError::CountOutOfRange {
            field: "proof properties",
            min: 1,
            max: MAX_PROPERTIES,
        });
    }
    properties.sort_unstable();
    if properties.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ModelError::DuplicateItem {
            field: "proof property",
        });
    }
    Ok(())
}

fn canonicalize_trusted(items: &mut [TrustedItem]) -> Result<(), ModelError> {
    if items.len() > MAX_TRUSTED_ITEMS {
        return Err(ModelError::TooManyItems {
            field: "trusted items",
            max: MAX_TRUSTED_ITEMS,
        });
    }
    items.sort_unstable();
    if items.windows(2).any(|pair| pair[0].name == pair[1].name) {
        return Err(ModelError::DuplicateItem {
            field: "trusted item name",
        });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ModelError {
    LengthOutOfRange {
        field: &'static str,
        max: usize,
    },
    CountOutOfRange {
        field: &'static str,
        min: usize,
        max: usize,
    },
    TooManyItems {
        field: &'static str,
        max: usize,
    },
    NonCanonicalText {
        field: &'static str,
    },
    InvalidIdentifier {
        field: &'static str,
    },
    InvalidHex {
        field: &'static str,
    },
    UnknownValue {
        field: &'static str,
    },
    DuplicateItem {
        field: &'static str,
    },
    AxiomRejected(String),
}

impl fmt::Display for ModelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LengthOutOfRange { field, max } => {
                write!(formatter, "{field} must contain 1..={max} bytes")
            }
            Self::CountOutOfRange { field, min, max } => {
                write!(formatter, "{field} count must be in {min}..={max}")
            }
            Self::TooManyItems { field, max } => {
                write!(formatter, "{field} exceeds the limit of {max}")
            }
            Self::NonCanonicalText { field } => write!(formatter, "{field} is not canonical text"),
            Self::InvalidIdentifier { field } => write!(formatter, "{field} is not an identifier"),
            Self::InvalidHex { field } => write!(formatter, "{field} is not canonical hex"),
            Self::UnknownValue { field } => write!(formatter, "unknown {field}"),
            Self::DuplicateItem { field } => write!(formatter, "duplicate {field}"),
            Self::AxiomRejected(name) => write!(formatter, "trusted item {name} is not allowed"),
        }
    }
}

impl std::error::Error for ModelError {}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn bytes(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn text(&mut self, value: &str) {
        self.u16(value.len() as u16);
        self.bytes(value.as_bytes());
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn decode_hex<const N: usize>(value: &str, field: &'static str) -> Result<[u8; N], ModelError> {
    if value.len() != N * 2 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ModelError::InvalidHex { field });
    }
    if value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(ModelError::InvalidHex { field });
    }
    let mut bytes = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(bytes)
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => unreachable!("decode_hex validates lowercase hexadecimal input"),
    }
}

use std::fmt;

use crate::{
    Configuration, CorrelationId, ExecutionTools, InvocationPlan, MAX_PROPERTIES,
    MAX_TRUSTED_ITEMS, ModelError, ProofOutcome, ProofProperty, ProofTargetIdentity, Text,
    TrustedItem, VerificationModelIdentity,
};

pub const MAX_RESULT_BYTES: usize = 64 * 1024;
const RESULT_MAGIC: &str = "FE2O3-VERIFIER-RESULT-V1";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecorderTermination {
    Exited(i32),
    TimedOut,
    Signaled(i32),
}

/// Validated proof evidence. This value does not grant load or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofResultV1 {
    target: ProofTargetIdentity,
    configuration: Configuration,
    model: VerificationModelIdentity,
    tools: ExecutionTools,
    outcome: ProofOutcome,
    proved_properties: Vec<ProofProperty>,
    trusted_items: Vec<TrustedItem>,
    diagnostic: Option<Text>,
}

impl ProofResultV1 {
    pub const fn target(&self) -> ProofTargetIdentity {
        self.target
    }

    pub const fn configuration(&self) -> &Configuration {
        &self.configuration
    }

    pub const fn model(&self) -> &VerificationModelIdentity {
        &self.model
    }

    pub const fn tools(&self) -> &ExecutionTools {
        &self.tools
    }

    pub const fn outcome(&self) -> ProofOutcome {
        self.outcome
    }

    pub fn proved_properties(&self) -> &[ProofProperty] {
        &self.proved_properties
    }

    pub fn trusted_items(&self) -> &[TrustedItem] {
        &self.trusted_items
    }

    pub const fn diagnostic(&self) -> Option<&Text> {
        self.diagnostic.as_ref()
    }
}

/// Parses the canonical envelope produced by the external evidence recorder.
///
/// Exactly six newline-terminated lines are accepted:
/// magic, correlation, outcome, properties, trusted items, and hex diagnostic.
pub fn parse_recorder_result(
    bytes: &[u8],
    plan: &InvocationPlan,
    termination: RecorderTermination,
) -> Result<ProofResultV1, ResultError> {
    if termination != RecorderTermination::Exited(0) {
        return Err(ResultError::RecorderDidNotSucceed(termination));
    }
    if bytes.len() > MAX_RESULT_BYTES {
        return Err(ResultError::TooLarge {
            max: MAX_RESULT_BYTES,
        });
    }
    let text = std::str::from_utf8(bytes).map_err(|_| ResultError::InvalidUtf8)?;
    if !text.ends_with('\n') {
        return Err(ResultError::MalformedEnvelope);
    }
    let lines: Vec<_> = text.split_terminator('\n').collect();
    if lines.len() != 6 || lines[0] != RESULT_MAGIC {
        return Err(ResultError::MalformedEnvelope);
    }

    let correlation = CorrelationId::from_hex(field(lines[1], "correlation")?)?;
    if correlation != plan.request().correlation_id() {
        return Err(ResultError::CorrelationMismatch);
    }
    let outcome = ProofOutcome::parse(field(lines[2], "outcome")?)?;
    let properties = parse_properties(field(lines[3], "properties")?)?;
    let trusted_items = parse_trusted(field(lines[4], "trusted")?)?;
    let diagnostic = parse_diagnostic(field(lines[5], "diagnostic-hex")?)?;

    if trusted_items != plan.request().trusted_items() {
        return Err(ResultError::TrustedItemsMismatch);
    }
    match outcome {
        ProofOutcome::Proved if properties != plan.request().properties() => {
            return Err(ResultError::IncompleteProof);
        }
        ProofOutcome::Failed | ProofOutcome::TimedOut if !properties.is_empty() => {
            return Err(ResultError::ClaimsOnIncompleteProof);
        }
        ProofOutcome::Proved | ProofOutcome::Failed | ProofOutcome::TimedOut => {}
    }

    Ok(ProofResultV1 {
        target: plan.request().target(),
        configuration: plan.request().configuration().clone(),
        model: plan.request().model().clone(),
        tools: plan.tools().clone(),
        outcome,
        proved_properties: properties,
        trusted_items,
        diagnostic,
    })
}

fn field<'a>(line: &'a str, expected: &'static str) -> Result<&'a str, ResultError> {
    line.strip_prefix(expected)
        .and_then(|value| value.strip_prefix('='))
        .ok_or(ResultError::UnexpectedField { expected })
}

fn parse_properties(value: &str) -> Result<Vec<ProofProperty>, ResultError> {
    if value.is_empty() {
        return Ok(vec![]);
    }
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() > MAX_PROPERTIES {
        return Err(ResultError::CountOutOfRange {
            field: "properties",
            max: MAX_PROPERTIES,
        });
    }
    let properties: Vec<_> = parts
        .into_iter()
        .map(ProofProperty::parse)
        .collect::<Result<_, _>>()?;
    if properties.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ResultError::NonCanonicalOrder {
            field: "properties",
        });
    }
    Ok(properties)
}

fn parse_trusted(value: &str) -> Result<Vec<TrustedItem>, ResultError> {
    if value.is_empty() {
        return Ok(vec![]);
    }
    let parts: Vec<_> = value.split(',').collect();
    if parts.len() > MAX_TRUSTED_ITEMS {
        return Err(ResultError::CountOutOfRange {
            field: "trusted items",
            max: MAX_TRUSTED_ITEMS,
        });
    }
    let mut items = Vec::with_capacity(parts.len());
    for value in parts {
        let (name, digest) = value
            .split_once('@')
            .ok_or(ResultError::MalformedTrustedItem)?;
        if digest.contains('@') {
            return Err(ResultError::MalformedTrustedItem);
        }
        items.push(TrustedItem::new(name, crate::Digest::from_hex(digest)?)?);
    }
    if items.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ResultError::NonCanonicalOrder {
            field: "trusted items",
        });
    }
    Ok(items)
}

fn parse_diagnostic(value: &str) -> Result<Option<Text>, ResultError> {
    if value.is_empty() {
        return Ok(None);
    }
    if !value.len().is_multiple_of(2)
        || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        || value.bytes().any(|byte| byte.is_ascii_uppercase())
    {
        return Err(ResultError::InvalidDiagnostic);
    }
    let mut decoded = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        decoded.push((hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?);
    }
    let decoded = String::from_utf8(decoded).map_err(|_| ResultError::InvalidDiagnostic)?;
    Ok(Some(Text::new("diagnostic", decoded)?))
}

fn hex_nibble(value: u8) -> Result<u8, ResultError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(ResultError::InvalidDiagnostic),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ResultError {
    TooLarge { max: usize },
    InvalidUtf8,
    RecorderDidNotSucceed(RecorderTermination),
    MalformedEnvelope,
    UnexpectedField { expected: &'static str },
    CorrelationMismatch,
    CountOutOfRange { field: &'static str, max: usize },
    NonCanonicalOrder { field: &'static str },
    MalformedTrustedItem,
    InvalidDiagnostic,
    TrustedItemsMismatch,
    IncompleteProof,
    ClaimsOnIncompleteProof,
    Model(ModelError),
}

impl fmt::Display for ResultError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => write!(formatter, "result exceeds {max} bytes"),
            Self::InvalidUtf8 => write!(formatter, "result is not UTF-8"),
            Self::RecorderDidNotSucceed(termination) => {
                write!(
                    formatter,
                    "evidence recorder did not succeed: {termination:?}"
                )
            }
            Self::MalformedEnvelope => write!(formatter, "result envelope is malformed"),
            Self::UnexpectedField { expected } => write!(formatter, "expected {expected} field"),
            Self::CorrelationMismatch => {
                write!(formatter, "result correlation does not match request")
            }
            Self::CountOutOfRange { field, max } => write!(formatter, "{field} exceeds {max}"),
            Self::NonCanonicalOrder { field } => write!(formatter, "{field} is not canonical"),
            Self::MalformedTrustedItem => write!(formatter, "trusted item is malformed"),
            Self::InvalidDiagnostic => write!(formatter, "diagnostic is not canonical hex text"),
            Self::TrustedItemsMismatch => write!(formatter, "trusted items do not match request"),
            Self::IncompleteProof => {
                write!(formatter, "proved result does not establish every request")
            }
            Self::ClaimsOnIncompleteProof => {
                write!(
                    formatter,
                    "incomplete result contains proved-property claims"
                )
            }
            Self::Model(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResultError {}

impl From<ModelError> for ResultError {
    fn from(value: ModelError) -> Self {
        Self::Model(value)
    }
}

//! Additive, bounded JSONL service for profiler Variant V1 comparison.
//!
//! The frozen Agent Profiler V1 service does not accept Variant V1 evidence.
//! This extension carries the exact evidence bytes as canonical lowercase hex,
//! admits them through the production Variant comparator, and returns no
//! execution, attach, scheduling, or collection authority.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::io::{BufRead, Write};

use fe2o3_semantic_import::{CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    MAX_PROFILER_VARIANT_RESULT_BYTES_V1, MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1,
    ProfilerVariantComparisonV1, ProfilerVariantTreatmentInputV1,
    build_profiler_variant_request_v1, compare_profiler_variants_v1,
};

pub const AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-variant-request-v1";
pub const AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1: &str =
    "fe2o3-agent-profiler-variant-response-v1";
pub const AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V1: &str = "fe2o3-profiler-variant-v1";
pub const MAX_AGENT_PROFILER_VARIANT_REQUESTS_V1: u32 = 64;
pub const MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1: u64 =
    (MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 * 4) + (64 * 1024);
pub const MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1: u64 =
    MAX_PROFILER_VARIANT_RESULT_BYTES_V1 + (16 * 1024);

const VARIANT_SERVICE_CONTRACT_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.variant-service.contract.v1\0";
const VARIANT_SERVICE_CONTRACT_BYTES_V1: &[u8] = b"additive-variant-v1;canonical-lowercase-hex-exact-inputs;bounded-jsonl;strict-revision;content-bound-comparison;read-only;no-execution-attach-scheduling-or-collection-authority";
const VARIANT_RESPONSE_BINDING_DOMAIN_V1: &[u8] =
    b"fe2o3.agent-profiler.variant-service.response.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantOperationV1 {
    DiscoverCapabilities,
    CompareVariants,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantAuthorityV1 {
    ReadOnlyNoExecutionAttachSchedulingOrCollectionAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerVariantCapabilityV1 {
    pub operation: AgentProfilerVariantOperationV1,
    pub available: bool,
    pub request_schema: &'static str,
    pub result_schema: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerVariantCapabilitiesV1 {
    pub service_contract: ContentIdentityRecordV1,
    pub operations: Vec<AgentProfilerVariantCapabilityV1>,
    pub authority: AgentProfilerVariantAuthorityV1,
    pub exact_input_encoding: &'static str,
    pub required_treatment_inputs: Vec<&'static str>,
    pub optional_treatment_inputs: Vec<&'static str>,
    pub max_request_bytes: u64,
    pub max_response_bytes: u64,
    pub max_requests: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerVariantTreatmentHexV1 {
    pub manifest_hex: String,
    pub semantic_workload_hex: String,
    pub raw_profiler_source_hex: String,
    pub bundle_hex: String,
    pub schedule_hex: String,
    pub artifact_hex: String,
    #[serde(default)]
    pub isa_projection_hex: Option<String>,
    #[serde(default)]
    pub counters_hex: Option<String>,
    #[serde(default)]
    pub pc_samples_hex: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentProfilerVariantRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    CompareVariants {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        baseline: Box<AgentProfilerVariantTreatmentHexV1>,
        candidate: Box<AgentProfilerVariantTreatmentHexV1>,
    },
}

impl AgentProfilerVariantRequestV1 {
    fn request_id(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities { request_id, .. }
            | Self::CompareVariants { request_id, .. } => *request_id,
        }
    }

    fn expected_revision(&self) -> u64 {
        match self {
            Self::DiscoverCapabilities {
                expected_revision, ..
            }
            | Self::CompareVariants {
                expected_revision, ..
            } => *expected_revision,
        }
    }

    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. } | Self::CompareVariants { schema, .. } => {
                schema
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentProfilerVariantResultV1 {
    Capabilities {
        capabilities: AgentProfilerVariantCapabilitiesV1,
    },
    Comparison {
        comparison: Box<ProfilerVariantComparisonV1>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    StaleRevision,
    RequestBudgetExhausted,
    RequestTooLarge,
    InvalidEvidenceEncoding,
    EvidenceTooLarge,
    EvidenceAdmissionFailed,
    ResponseTooLarge,
    InternalEvidenceMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentProfilerVariantResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: AgentProfilerVariantResultV1,
        response_identity: ContentIdentityRecordV1,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerVariantErrorCodeV1,
        terminal: bool,
        response_identity: ContentIdentityRecordV1,
    },
}

impl AgentProfilerVariantResponseV1 {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum AgentProfilerVariantResponsePreimageV1<'a> {
    Ok {
        schema: &'a str,
        request_id: u64,
        response_revision: u64,
        value: &'a AgentProfilerVariantResultV1,
    },
    Error {
        schema: &'a str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerVariantErrorCodeV1,
        terminal: bool,
    },
}

pub struct AgentProfilerVariantServiceV1 {
    service_contract: ContentIdentityRecordV1,
    response_revision: u64,
    remaining_requests: u32,
    seen_request_ids: BTreeSet<u64>,
}

impl AgentProfilerVariantServiceV1 {
    pub fn new() -> Result<Self, AgentProfilerVariantServiceErrorV1> {
        Ok(Self {
            service_contract: content_identity(
                VARIANT_SERVICE_CONTRACT_DOMAIN_V1,
                VARIANT_SERVICE_CONTRACT_BYTES_V1,
            )?,
            response_revision: 0,
            remaining_requests: MAX_AGENT_PROFILER_VARIANT_REQUESTS_V1,
            seen_request_ids: BTreeSet::new(),
        })
    }

    pub fn handle(
        &mut self,
        request: AgentProfilerVariantRequestV1,
    ) -> Result<AgentProfilerVariantResponseV1, AgentProfilerVariantServiceErrorV1> {
        if self.remaining_requests == 0 {
            return self.error(
                Some(request.request_id()),
                AgentProfilerVariantErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.remaining_requests -= 1;
        let request_id = request.request_id();
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if !self.seen_request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if request.expected_revision() != self.response_revision {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV1::StaleRevision,
                false,
            );
        }
        let value = match request {
            AgentProfilerVariantRequestV1::DiscoverCapabilities { .. } => {
                AgentProfilerVariantResultV1::Capabilities {
                    capabilities: AgentProfilerVariantCapabilitiesV1 {
                        service_contract: self.service_contract,
                        operations: vec![
                            AgentProfilerVariantCapabilityV1 {
                                operation: AgentProfilerVariantOperationV1::DiscoverCapabilities,
                                available: true,
                                request_schema: AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
                                result_schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1,
                            },
                            AgentProfilerVariantCapabilityV1 {
                                operation: AgentProfilerVariantOperationV1::CompareVariants,
                                available: true,
                                request_schema: AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V1,
                                result_schema: AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V1,
                            },
                        ],
                        authority: AgentProfilerVariantAuthorityV1::ReadOnlyNoExecutionAttachSchedulingOrCollectionAuthority,
                        exact_input_encoding: "canonical_lowercase_hex_of_exact_bytes",
                        required_treatment_inputs: vec![
                            "manifest",
                            "semantic_workload",
                            "raw_profiler_source",
                            "bundle",
                            "schedule",
                            "artifact",
                        ],
                        optional_treatment_inputs: vec![
                            "isa_projection",
                            "counters",
                            "pc_samples",
                        ],
                        max_request_bytes: MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1,
                        max_response_bytes: MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1,
                        max_requests: MAX_AGENT_PROFILER_VARIANT_REQUESTS_V1,
                    },
                }
            }
            AgentProfilerVariantRequestV1::CompareVariants {
                baseline,
                candidate,
                ..
            } => {
                let baseline = match OwnedTreatmentV1::decode(*baseline) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let candidate = match OwnedTreatmentV1::decode(*candidate) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let comparison_request = match build_profiler_variant_request_v1(
                    &baseline.semantic_workload,
                    &baseline.manifest,
                    &candidate.manifest,
                ) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentProfilerVariantErrorCodeV1::EvidenceAdmissionFailed,
                            false,
                        );
                    }
                };
                let comparison = match compare_profiler_variants_v1(
                    comparison_request,
                    baseline.input(),
                    candidate.input(),
                ) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentProfilerVariantErrorCodeV1::EvidenceAdmissionFailed,
                            false,
                        );
                    }
                };
                AgentProfilerVariantResultV1::Comparison {
                    comparison: Box::new(comparison),
                }
            }
        };
        self.ok(request_id, value)
    }

    pub fn terminal_protocol_error(
        &mut self,
        code: AgentProfilerVariantErrorCodeV1,
    ) -> Result<AgentProfilerVariantResponseV1, AgentProfilerVariantServiceErrorV1> {
        self.error(None, code, true)
    }

    pub fn encode_response(
        &self,
        response: &AgentProfilerVariantResponseV1,
    ) -> Result<Vec<u8>, AgentProfilerVariantServiceErrorV1> {
        validate_response_identity(response)?;
        encode_bounded(response)
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: AgentProfilerVariantResultV1,
    ) -> Result<AgentProfilerVariantResponseV1, AgentProfilerVariantServiceErrorV1> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerVariantServiceErrorV1::RevisionOverflow)?;
        let preimage = AgentProfilerVariantResponsePreimageV1::Ok {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.response_revision,
            value: &value,
        };
        let response_identity = response_identity(&preimage)?;
        Ok(AgentProfilerVariantResponseV1::Ok {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.response_revision,
            value,
            response_identity,
        })
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentProfilerVariantErrorCodeV1,
        terminal: bool,
    ) -> Result<AgentProfilerVariantResponseV1, AgentProfilerVariantServiceErrorV1> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerVariantServiceErrorV1::RevisionOverflow)?;
        let preimage = AgentProfilerVariantResponsePreimageV1::Error {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
        };
        let response_identity = response_identity(&preimage)?;
        Ok(AgentProfilerVariantResponseV1::Error {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
            response_identity,
        })
    }
}

struct OwnedTreatmentV1 {
    manifest: Vec<u8>,
    semantic_workload: Vec<u8>,
    raw_profiler_source: Vec<u8>,
    bundle: Vec<u8>,
    schedule: Vec<u8>,
    artifact: Vec<u8>,
    isa_projection: Option<Vec<u8>>,
    counters: Option<Vec<u8>>,
    pc_samples: Option<Vec<u8>>,
}

impl OwnedTreatmentV1 {
    fn decode(
        value: AgentProfilerVariantTreatmentHexV1,
    ) -> Result<Self, AgentProfilerVariantErrorCodeV1> {
        let result = Self {
            manifest: decode_hex(&value.manifest_hex)?,
            semantic_workload: decode_hex(&value.semantic_workload_hex)?,
            raw_profiler_source: decode_hex(&value.raw_profiler_source_hex)?,
            bundle: decode_hex(&value.bundle_hex)?,
            schedule: decode_hex(&value.schedule_hex)?,
            artifact: decode_hex(&value.artifact_hex)?,
            isa_projection: decode_optional_hex(value.isa_projection_hex.as_deref())?,
            counters: decode_optional_hex(value.counters_hex.as_deref())?,
            pc_samples: decode_optional_hex(value.pc_samples_hex.as_deref())?,
        };
        let total = result
            .parts()
            .try_fold(0_u64, |sum, bytes| sum.checked_add(bytes.len() as u64))
            .ok_or(AgentProfilerVariantErrorCodeV1::EvidenceTooLarge)?;
        if total == 0 || total > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 {
            return Err(AgentProfilerVariantErrorCodeV1::EvidenceTooLarge);
        }
        Ok(result)
    }

    fn parts(&self) -> impl Iterator<Item = &[u8]> {
        [
            Some(self.manifest.as_slice()),
            Some(self.semantic_workload.as_slice()),
            Some(self.raw_profiler_source.as_slice()),
            Some(self.bundle.as_slice()),
            Some(self.schedule.as_slice()),
            Some(self.artifact.as_slice()),
            self.isa_projection.as_deref(),
            self.counters.as_deref(),
            self.pc_samples.as_deref(),
        ]
        .into_iter()
        .flatten()
    }

    fn input(&self) -> ProfilerVariantTreatmentInputV1<'_> {
        ProfilerVariantTreatmentInputV1 {
            manifest: &self.manifest,
            semantic_workload: &self.semantic_workload,
            raw_profiler_source: &self.raw_profiler_source,
            bundle: &self.bundle,
            schedule: &self.schedule,
            artifact: &self.artifact,
            isa_projection: self.isa_projection.as_deref(),
            counters: self.counters.as_deref(),
            pc_samples: self.pc_samples.as_deref(),
        }
    }
}

fn decode_optional_hex(
    value: Option<&str>,
) -> Result<Option<Vec<u8>>, AgentProfilerVariantErrorCodeV1> {
    value.map(decode_hex).transpose()
}

fn decode_hex(value: &str) -> Result<Vec<u8>, AgentProfilerVariantErrorCodeV1> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || value.len() as u64 > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 * 2
    {
        return Err(AgentProfilerVariantErrorCodeV1::InvalidEvidenceEncoding);
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high =
            hex_nibble(pair[0]).ok_or(AgentProfilerVariantErrorCodeV1::InvalidEvidenceEncoding)?;
        let low =
            hex_nibble(pair[1]).ok_or(AgentProfilerVariantErrorCodeV1::InvalidEvidenceEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn content_identity(
    domain: &[u8],
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    let digest = CaptureIdentityV1::new(hasher.finalize().into())
        .map_err(|_| AgentProfilerVariantServiceErrorV1::Identity)?;
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest,
        canonical_len: bytes.len() as u64,
    })
}

fn response_identity<T: Serialize>(
    preimage: &T,
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV1> {
    let value = serde_json::to_value(preimage)
        .map_err(|_| AgentProfilerVariantServiceErrorV1::JsonEncode)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| AgentProfilerVariantServiceErrorV1::JsonEncode)?;
    content_identity(VARIANT_RESPONSE_BINDING_DOMAIN_V1, &bytes)
}

pub fn validate_agent_profiler_variant_response_line_v1(
    line: &[u8],
) -> Result<(), AgentProfilerVariantServiceErrorV1> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1 {
        return Err(AgentProfilerVariantServiceErrorV1::InvalidResponse);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentProfilerVariantServiceErrorV1::InvalidResponse);
    }
    let mut value: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|_| AgentProfilerVariantServiceErrorV1::InvalidResponse)?;
    let object = value
        .as_object_mut()
        .ok_or(AgentProfilerVariantServiceErrorV1::InvalidResponse)?;
    if object.get("schema").and_then(serde_json::Value::as_str)
        != Some(AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V1)
    {
        return Err(AgentProfilerVariantServiceErrorV1::InvalidResponse);
    }
    let supplied = object
        .remove("response_identity")
        .ok_or(AgentProfilerVariantServiceErrorV1::InvalidResponse)?;
    let supplied: ContentIdentityRecordV1 = serde_json::from_value(supplied)
        .map_err(|_| AgentProfilerVariantServiceErrorV1::InvalidResponse)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| AgentProfilerVariantServiceErrorV1::JsonEncode)?;
    if content_identity(VARIANT_RESPONSE_BINDING_DOMAIN_V1, &bytes)? != supplied {
        return Err(AgentProfilerVariantServiceErrorV1::InvalidResponse);
    }
    Ok(())
}

fn validate_response_identity(
    response: &AgentProfilerVariantResponseV1,
) -> Result<(), AgentProfilerVariantServiceErrorV1> {
    let (preimage, supplied) = match response {
        AgentProfilerVariantResponseV1::Ok {
            schema,
            request_id,
            response_revision,
            value,
            response_identity,
        } => (
            AgentProfilerVariantResponsePreimageV1::Ok {
                schema,
                request_id: *request_id,
                response_revision: *response_revision,
                value,
            },
            response_identity,
        ),
        AgentProfilerVariantResponseV1::Error {
            schema,
            request_id,
            response_revision,
            code,
            terminal,
            response_identity,
        } => (
            AgentProfilerVariantResponsePreimageV1::Error {
                schema,
                request_id: *request_id,
                response_revision: *response_revision,
                code: *code,
                terminal: *terminal,
            },
            response_identity,
        ),
    };
    if &response_identity(&preimage)? != supplied {
        return Err(AgentProfilerVariantServiceErrorV1::InvalidResponse);
    }
    Ok(())
}

pub fn decode_agent_profiler_variant_request_line_v1(
    line: &[u8],
) -> Result<AgentProfilerVariantRequestV1, AgentProfilerVariantServiceErrorV1> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1 {
        return Err(AgentProfilerVariantServiceErrorV1::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentProfilerVariantServiceErrorV1::InvalidRequest);
    }
    serde_json::from_slice(payload).map_err(|_| AgentProfilerVariantServiceErrorV1::InvalidRequest)
}

pub fn read_agent_profiler_variant_request_line_v1<R: BufRead>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, AgentProfilerVariantServiceErrorV1> {
    let mut line = Vec::new();
    let max = usize::try_from(MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V1)
        .map_err(|_| AgentProfilerVariantServiceErrorV1::SizeOverflow)?;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| AgentProfilerVariantServiceErrorV1::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(AgentProfilerVariantServiceErrorV1::InvalidRequest)
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |position| position + 1);
        let remaining = max.saturating_add(1).saturating_sub(line.len());
        let consumed = available.min(remaining);
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if line.len() > max {
            return Err(AgentProfilerVariantServiceErrorV1::RequestTooLarge);
        }
        if newline.is_some() && consumed == available {
            return Ok(Some(line));
        }
    }
}

pub fn run_agent_profiler_variant_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentProfilerVariantServiceErrorV1> {
    let mut service = AgentProfilerVariantServiceV1::new()?;
    loop {
        let line = match read_agent_profiler_variant_request_line_v1(input) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(AgentProfilerVariantServiceErrorV1::RequestTooLarge) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV1::RequestTooLarge,
                )?;
                return Err(AgentProfilerVariantServiceErrorV1::ProtocolTerminated);
            }
            Err(_) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV1::InvalidRequest,
                )?;
                return Err(AgentProfilerVariantServiceErrorV1::ProtocolTerminated);
            }
        };
        let request = match decode_agent_profiler_variant_request_line_v1(&line) {
            Ok(request) => request,
            Err(_) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV1::InvalidRequest,
                )?;
                return Err(AgentProfilerVariantServiceErrorV1::ProtocolTerminated);
            }
        };
        let response = service.handle(request)?;
        let terminal = response.is_terminal();
        output
            .write_all(&service.encode_response(&response)?)
            .map_err(|_| AgentProfilerVariantServiceErrorV1::Io)?;
        output
            .flush()
            .map_err(|_| AgentProfilerVariantServiceErrorV1::Io)?;
        if terminal {
            return Err(AgentProfilerVariantServiceErrorV1::ProtocolTerminated);
        }
    }
}

fn write_terminal<W: Write>(
    output: &mut W,
    service: &mut AgentProfilerVariantServiceV1,
    code: AgentProfilerVariantErrorCodeV1,
) -> Result<(), AgentProfilerVariantServiceErrorV1> {
    let response = service.terminal_protocol_error(code)?;
    output
        .write_all(&service.encode_response(&response)?)
        .map_err(|_| AgentProfilerVariantServiceErrorV1::Io)?;
    output
        .flush()
        .map_err(|_| AgentProfilerVariantServiceErrorV1::Io)
}

fn encode_bounded(
    response: &AgentProfilerVariantResponseV1,
) -> Result<Vec<u8>, AgentProfilerVariantServiceErrorV1> {
    let bytes =
        serde_json::to_vec(response).map_err(|_| AgentProfilerVariantServiceErrorV1::JsonEncode)?;
    if bytes.len() as u64 + 1 > MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V1 {
        return Err(AgentProfilerVariantServiceErrorV1::ResponseTooLarge);
    }
    let mut output = bytes;
    output.push(b'\n');
    Ok(output)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProfilerVariantServiceErrorV1 {
    Identity,
    InvalidRequest,
    RequestTooLarge,
    InvalidResponse,
    ResponseTooLarge,
    JsonEncode,
    RevisionOverflow,
    SizeOverflow,
    Io,
    ProtocolTerminated,
}

impl fmt::Display for AgentProfilerVariantServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "profiler Variant V1 extension rejected input: {self:?}"
        )
    }
}

impl Error for AgentProfilerVariantServiceErrorV1 {}

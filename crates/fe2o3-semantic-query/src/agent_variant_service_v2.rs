//! Strict, bounded JSONL service for Profiler Variant V2.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;
use std::io::{BufRead, Write};

use fe2o3_semantic_import::{CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    MAX_PROFILER_VARIANT_RESULT_BYTES_V2, ProfilerVariantComparisonV2,
    ProfilerVariantDecodedAttSourceIsaEvidenceV2, ProfilerVariantPcSourceIsaEvidenceV2,
    ProfilerVariantTreatmentInputV1, ProfilerVariantTreatmentInputV2,
    build_profiler_variant_request_v2, compare_profiler_variants_v2,
};

pub const AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V2: &str =
    "fe2o3-agent-profiler-variant-request-v2";
pub const AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V2: &str =
    "fe2o3-agent-profiler-variant-response-v2";
pub const AGENT_PROFILER_VARIANT_COMPARISON_SCHEMA_V2: &str = "fe2o3-profiler-variant-v2";
pub const MAX_AGENT_PROFILER_VARIANT_REQUESTS_V2: u32 = 64;
pub const MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V2: u64 = 512 * 1024 * 1024;
pub const MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V2: u64 =
    MAX_PROFILER_VARIANT_RESULT_BYTES_V2 + (16 * 1024);

const MAX_AGENT_PROFILER_VARIANT_FIELD_BYTES_V2: u64 = 64 * 1024 * 1024;
const MAX_AGENT_PROFILER_VARIANT_TREATMENT_BYTES_V2: u64 = 256 * 1024 * 1024;
const CONTRACT_DOMAIN_V2: &[u8] = b"fe2o3.agent-profiler.variant-service.contract.v2\0";
const CONTRACT_BYTES_V2: &[u8] = b"variant-v2;exact-content-bound-treatment-and-correlation-evidence;strict-lowercase-hex;bounded-deterministic-jsonl;positive-observations-only;sampled-or-incomplete-absence-is-not-add-remove-or-causality;read-only";
const RESPONSE_DOMAIN_V2: &[u8] = b"fe2o3.agent-profiler.variant-service.response.v2\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantOperationV2 {
    DiscoverCapabilities,
    CompareVariants,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantAuthorityV2 {
    ReadOnlyNoExecutionAttachSchedulingCollectionOrDecoderAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct AgentProfilerVariantCapabilitiesV2 {
    pub service_contract: ContentIdentityRecordV1,
    pub operations: Vec<AgentProfilerVariantOperationV2>,
    pub authority: AgentProfilerVariantAuthorityV2,
    pub exact_input_encoding: &'static str,
    pub comparison_semantics: &'static str,
    pub maximum_requests: u32,
    pub maximum_request_bytes: u64,
    pub maximum_response_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerVariantTreatmentHexV2 {
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
    #[serde(default)]
    pub pc_source_isa: Option<AgentProfilerVariantPcSourceIsaHexV2>,
    #[serde(default)]
    pub decoded_att_source_isa: Option<AgentProfilerVariantDecodedAttSourceIsaHexV2>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerVariantPcSourceIsaHexV2 {
    pub source_hex: String,
    pub relation_hex: String,
    pub characteristic_hex: String,
    pub sample_identities: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AgentProfilerVariantDecodedAttSourceIsaHexV2 {
    pub interchange_hex: String,
    pub characteristic_hex: String,
    pub code_object_identity: CaptureIdentityV1,
    pub record_identities: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentProfilerVariantRequestV2 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    CompareVariants {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        baseline: Box<AgentProfilerVariantTreatmentHexV2>,
        candidate: Box<AgentProfilerVariantTreatmentHexV2>,
    },
}

impl AgentProfilerVariantRequestV2 {
    fn schema(&self) -> &str {
        match self {
            Self::DiscoverCapabilities { schema, .. } | Self::CompareVariants { schema, .. } => {
                schema
            }
        }
    }

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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum AgentProfilerVariantResultV2 {
    Capabilities {
        capabilities: AgentProfilerVariantCapabilitiesV2,
    },
    Comparison {
        comparison: Box<ProfilerVariantComparisonV2>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProfilerVariantErrorCodeV2 {
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
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentProfilerVariantResponseV2 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: AgentProfilerVariantResultV2,
        response_identity: ContentIdentityRecordV1,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerVariantErrorCodeV2,
        terminal: bool,
        response_identity: ContentIdentityRecordV1,
    },
}

impl AgentProfilerVariantResponseV2 {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ResponsePreimageV2<'a> {
    Ok {
        schema: &'a str,
        request_id: u64,
        response_revision: u64,
        value: &'a AgentProfilerVariantResultV2,
    },
    Error {
        schema: &'a str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentProfilerVariantErrorCodeV2,
        terminal: bool,
    },
}

pub struct AgentProfilerVariantServiceV2 {
    service_contract: ContentIdentityRecordV1,
    response_revision: u64,
    remaining_requests: u32,
    seen_request_ids: BTreeSet<u64>,
}

impl AgentProfilerVariantServiceV2 {
    pub fn new() -> Result<Self, AgentProfilerVariantServiceErrorV2> {
        Ok(Self {
            service_contract: content_identity(CONTRACT_DOMAIN_V2, CONTRACT_BYTES_V2, 2)?,
            response_revision: 0,
            remaining_requests: MAX_AGENT_PROFILER_VARIANT_REQUESTS_V2,
            seen_request_ids: BTreeSet::new(),
        })
    }

    pub fn handle(
        &mut self,
        request: AgentProfilerVariantRequestV2,
    ) -> Result<AgentProfilerVariantResponseV2, AgentProfilerVariantServiceErrorV2> {
        if self.remaining_requests == 0 {
            return self.error(
                Some(request.request_id()),
                AgentProfilerVariantErrorCodeV2::RequestBudgetExhausted,
                true,
            );
        }
        self.remaining_requests -= 1;
        let request_id = request.request_id();
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV2::InvalidRequestId,
                false,
            );
        }
        if !self.seen_request_ids.insert(request_id) {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV2::DuplicateRequestId,
                false,
            );
        }
        if request.schema() != AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V2 {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV2::InvalidSchema,
                false,
            );
        }
        if request.expected_revision() != self.response_revision {
            return self.error(
                Some(request_id),
                AgentProfilerVariantErrorCodeV2::StaleRevision,
                false,
            );
        }
        let value = match request {
            AgentProfilerVariantRequestV2::DiscoverCapabilities { .. } => {
                AgentProfilerVariantResultV2::Capabilities {
                    capabilities: AgentProfilerVariantCapabilitiesV2 {
                        service_contract: self.service_contract,
                        operations: vec![
                            AgentProfilerVariantOperationV2::DiscoverCapabilities,
                            AgentProfilerVariantOperationV2::CompareVariants,
                        ],
                        authority: AgentProfilerVariantAuthorityV2::ReadOnlyNoExecutionAttachSchedulingCollectionOrDecoderAuthority,
                        exact_input_encoding: "canonical_lowercase_hex_of_exact_bytes",
                        comparison_semantics: "positive_exact_source_mir_pairs_only;sampled_or_incomplete_absence_never_means_added_removed_or_causal",
                        maximum_requests: MAX_AGENT_PROFILER_VARIANT_REQUESTS_V2,
                        maximum_request_bytes: MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V2,
                        maximum_response_bytes: MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V2,
                    },
                }
            }
            AgentProfilerVariantRequestV2::CompareVariants {
                baseline,
                candidate,
                ..
            } => {
                let baseline = match OwnedTreatmentV2::decode(*baseline) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let candidate = match OwnedTreatmentV2::decode(*candidate) {
                    Ok(value) => value,
                    Err(code) => return self.error(Some(request_id), code, false),
                };
                let comparison_request = match build_profiler_variant_request_v2(
                    &baseline.semantic_workload,
                    baseline.input(),
                    candidate.input(),
                ) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentProfilerVariantErrorCodeV2::EvidenceAdmissionFailed,
                            false,
                        );
                    }
                };
                let comparison = match compare_profiler_variants_v2(
                    comparison_request,
                    baseline.input(),
                    candidate.input(),
                ) {
                    Ok(value) => value,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentProfilerVariantErrorCodeV2::EvidenceAdmissionFailed,
                            false,
                        );
                    }
                };
                AgentProfilerVariantResultV2::Comparison {
                    comparison: Box::new(comparison),
                }
            }
        };
        self.ok(request_id, value)
    }

    pub fn encode_response(
        &self,
        response: &AgentProfilerVariantResponseV2,
    ) -> Result<Vec<u8>, AgentProfilerVariantServiceErrorV2> {
        validate_response_identity(response)?;
        let mut bytes = serde_json::to_vec(response)
            .map_err(|_| AgentProfilerVariantServiceErrorV2::JsonEncode)?;
        if bytes.len() as u64 + 1 > MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V2 {
            return Err(AgentProfilerVariantServiceErrorV2::ResponseTooLarge);
        }
        bytes.push(b'\n');
        Ok(bytes)
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: AgentProfilerVariantResultV2,
    ) -> Result<AgentProfilerVariantResponseV2, AgentProfilerVariantServiceErrorV2> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerVariantServiceErrorV2::RevisionOverflow)?;
        let preimage = ResponsePreimageV2::Ok {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V2,
            request_id,
            response_revision: self.response_revision,
            value: &value,
        };
        let response_identity = response_identity(&preimage)?;
        Ok(AgentProfilerVariantResponseV2::Ok {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V2,
            request_id,
            response_revision: self.response_revision,
            value,
            response_identity,
        })
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentProfilerVariantErrorCodeV2,
        terminal: bool,
    ) -> Result<AgentProfilerVariantResponseV2, AgentProfilerVariantServiceErrorV2> {
        self.response_revision = self
            .response_revision
            .checked_add(1)
            .ok_or(AgentProfilerVariantServiceErrorV2::RevisionOverflow)?;
        let preimage = ResponsePreimageV2::Error {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V2,
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
        };
        Ok(AgentProfilerVariantResponseV2::Error {
            schema: AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V2,
            request_id,
            response_revision: self.response_revision,
            code,
            terminal,
            response_identity: response_identity(&preimage)?,
        })
    }

    fn terminal_protocol_error(
        &mut self,
        code: AgentProfilerVariantErrorCodeV2,
    ) -> Result<AgentProfilerVariantResponseV2, AgentProfilerVariantServiceErrorV2> {
        self.error(None, code, true)
    }
}

fn validate_response_identity(
    response: &AgentProfilerVariantResponseV2,
) -> Result<(), AgentProfilerVariantServiceErrorV2> {
    let (preimage, supplied) = match response {
        AgentProfilerVariantResponseV2::Ok {
            schema,
            request_id,
            response_revision,
            value,
            response_identity,
        } => (
            ResponsePreimageV2::Ok {
                schema,
                request_id: *request_id,
                response_revision: *response_revision,
                value,
            },
            response_identity,
        ),
        AgentProfilerVariantResponseV2::Error {
            schema,
            request_id,
            response_revision,
            code,
            terminal,
            response_identity,
        } => (
            ResponsePreimageV2::Error {
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
        return Err(AgentProfilerVariantServiceErrorV2::InvalidResponse);
    }
    Ok(())
}

pub fn validate_agent_profiler_variant_response_line_v2(
    line: &[u8],
) -> Result<(), AgentProfilerVariantServiceErrorV2> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_VARIANT_RESPONSE_BYTES_V2 {
        return Err(AgentProfilerVariantServiceErrorV2::InvalidResponse);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentProfilerVariantServiceErrorV2::InvalidResponse);
    }
    let mut value: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|_| AgentProfilerVariantServiceErrorV2::InvalidResponse)?;
    let object = value
        .as_object_mut()
        .ok_or(AgentProfilerVariantServiceErrorV2::InvalidResponse)?;
    if object.get("schema").and_then(serde_json::Value::as_str)
        != Some(AGENT_PROFILER_VARIANT_RESPONSE_SCHEMA_V2)
    {
        return Err(AgentProfilerVariantServiceErrorV2::InvalidResponse);
    }
    let supplied: ContentIdentityRecordV1 = serde_json::from_value(
        object
            .remove("response_identity")
            .ok_or(AgentProfilerVariantServiceErrorV2::InvalidResponse)?,
    )
    .map_err(|_| AgentProfilerVariantServiceErrorV2::InvalidResponse)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| AgentProfilerVariantServiceErrorV2::JsonEncode)?;
    if content_identity(RESPONSE_DOMAIN_V2, &bytes, 2)? != supplied {
        return Err(AgentProfilerVariantServiceErrorV2::InvalidResponse);
    }
    Ok(())
}

struct OwnedPcSourceIsaV2 {
    source: Vec<u8>,
    relation: Vec<u8>,
    characteristic: Vec<u8>,
    sample_identities: Vec<CaptureIdentityV1>,
}

struct OwnedDecodedAttSourceIsaV2 {
    interchange: Vec<u8>,
    characteristic: Vec<u8>,
    code_object_identity: CaptureIdentityV1,
    record_identities: Vec<CaptureIdentityV1>,
}

struct OwnedTreatmentV2 {
    manifest: Vec<u8>,
    semantic_workload: Vec<u8>,
    raw_profiler_source: Vec<u8>,
    bundle: Vec<u8>,
    schedule: Vec<u8>,
    artifact: Vec<u8>,
    isa_projection: Option<Vec<u8>>,
    counters: Option<Vec<u8>>,
    pc_samples: Option<Vec<u8>>,
    pc_source_isa: Option<OwnedPcSourceIsaV2>,
    decoded_att_source_isa: Option<OwnedDecodedAttSourceIsaV2>,
}

impl OwnedTreatmentV2 {
    fn decode(
        value: AgentProfilerVariantTreatmentHexV2,
    ) -> Result<Self, AgentProfilerVariantErrorCodeV2> {
        let pc_source_isa = value
            .pc_source_isa
            .map(|evidence| {
                Ok(OwnedPcSourceIsaV2 {
                    source: decode_hex(&evidence.source_hex)?,
                    relation: decode_hex(&evidence.relation_hex)?,
                    characteristic: decode_hex(&evidence.characteristic_hex)?,
                    sample_identities: evidence.sample_identities,
                })
            })
            .transpose()?;
        let decoded_att_source_isa = value
            .decoded_att_source_isa
            .map(|evidence| {
                Ok(OwnedDecodedAttSourceIsaV2 {
                    interchange: decode_hex(&evidence.interchange_hex)?,
                    characteristic: decode_hex(&evidence.characteristic_hex)?,
                    code_object_identity: evidence.code_object_identity,
                    record_identities: evidence.record_identities,
                })
            })
            .transpose()?;
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
            pc_source_isa,
            decoded_att_source_isa,
        };
        let total = result.parts().try_fold(0_u64, |sum, bytes| {
            sum.checked_add(bytes.len() as u64)
                .ok_or(AgentProfilerVariantErrorCodeV2::EvidenceTooLarge)
        })?;
        if total == 0 || total > MAX_AGENT_PROFILER_VARIANT_TREATMENT_BYTES_V2 {
            return Err(AgentProfilerVariantErrorCodeV2::EvidenceTooLarge);
        }
        Ok(result)
    }

    fn parts(&self) -> impl Iterator<Item = &[u8]> {
        let pc_relation = self
            .pc_source_isa
            .as_ref()
            .map(|value| value.relation.as_slice());
        let pc_source = self
            .pc_source_isa
            .as_ref()
            .map(|value| value.source.as_slice());
        let pc_characteristic = self
            .pc_source_isa
            .as_ref()
            .map(|value| value.characteristic.as_slice());
        let att_interchange = self
            .decoded_att_source_isa
            .as_ref()
            .map(|value| value.interchange.as_slice());
        let att_characteristic = self
            .decoded_att_source_isa
            .as_ref()
            .map(|value| value.characteristic.as_slice());
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
            pc_source,
            pc_relation,
            pc_characteristic,
            att_interchange,
            att_characteristic,
        ]
        .into_iter()
        .flatten()
    }

    fn input(&self) -> ProfilerVariantTreatmentInputV2<'_> {
        ProfilerVariantTreatmentInputV2 {
            treatment: ProfilerVariantTreatmentInputV1 {
                manifest: &self.manifest,
                semantic_workload: &self.semantic_workload,
                raw_profiler_source: &self.raw_profiler_source,
                bundle: &self.bundle,
                schedule: &self.schedule,
                artifact: &self.artifact,
                isa_projection: self.isa_projection.as_deref(),
                counters: self.counters.as_deref(),
                pc_samples: self.pc_samples.as_deref(),
            },
            pc_source_isa: self.pc_source_isa.as_ref().map(|evidence| {
                ProfilerVariantPcSourceIsaEvidenceV2 {
                    source: &evidence.source,
                    relation: &evidence.relation,
                    characteristic: &evidence.characteristic,
                    sample_identities: &evidence.sample_identities,
                }
            }),
            decoded_att_source_isa: self.decoded_att_source_isa.as_ref().map(|evidence| {
                ProfilerVariantDecodedAttSourceIsaEvidenceV2 {
                    interchange: &evidence.interchange,
                    characteristic: &evidence.characteristic,
                    code_object_identity: evidence.code_object_identity,
                    record_identities: &evidence.record_identities,
                }
            }),
        }
    }
}

fn decode_optional_hex(
    value: Option<&str>,
) -> Result<Option<Vec<u8>>, AgentProfilerVariantErrorCodeV2> {
    value.map(decode_hex).transpose()
}

fn decode_hex(value: &str) -> Result<Vec<u8>, AgentProfilerVariantErrorCodeV2> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || value.len() as u64 > MAX_AGENT_PROFILER_VARIANT_FIELD_BYTES_V2 * 2
    {
        return Err(AgentProfilerVariantErrorCodeV2::InvalidEvidenceEncoding);
    }
    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let high =
            hex_nibble(pair[0]).ok_or(AgentProfilerVariantErrorCodeV2::InvalidEvidenceEncoding)?;
        let low =
            hex_nibble(pair[1]).ok_or(AgentProfilerVariantErrorCodeV2::InvalidEvidenceEncoding)?;
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

pub fn decode_agent_profiler_variant_request_line_v2(
    line: &[u8],
) -> Result<AgentProfilerVariantRequestV2, AgentProfilerVariantServiceErrorV2> {
    if line.is_empty() || line.len() as u64 > MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V2 {
        return Err(AgentProfilerVariantServiceErrorV2::RequestTooLarge);
    }
    let payload = line.strip_suffix(b"\n").unwrap_or(line);
    if payload.is_empty() || payload.contains(&b'\n') || payload.contains(&b'\r') {
        return Err(AgentProfilerVariantServiceErrorV2::InvalidRequest);
    }
    serde_json::from_slice(payload).map_err(|_| AgentProfilerVariantServiceErrorV2::InvalidRequest)
}

pub fn run_agent_profiler_variant_jsonl_v2<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentProfilerVariantServiceErrorV2> {
    let mut service = AgentProfilerVariantServiceV2::new()?;
    loop {
        let line = match read_line(input) {
            Ok(Some(line)) => line,
            Ok(None) => return Ok(()),
            Err(AgentProfilerVariantServiceErrorV2::RequestTooLarge) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV2::RequestTooLarge,
                )?;
                return Err(AgentProfilerVariantServiceErrorV2::ProtocolTerminated);
            }
            Err(_) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV2::InvalidRequest,
                )?;
                return Err(AgentProfilerVariantServiceErrorV2::ProtocolTerminated);
            }
        };
        let request = match decode_agent_profiler_variant_request_line_v2(&line) {
            Ok(value) => value,
            Err(_) => {
                write_terminal(
                    output,
                    &mut service,
                    AgentProfilerVariantErrorCodeV2::InvalidRequest,
                )?;
                return Err(AgentProfilerVariantServiceErrorV2::ProtocolTerminated);
            }
        };
        let response = service.handle(request)?;
        let terminal = response.is_terminal();
        output
            .write_all(&service.encode_response(&response)?)
            .map_err(|_| AgentProfilerVariantServiceErrorV2::Io)?;
        output
            .flush()
            .map_err(|_| AgentProfilerVariantServiceErrorV2::Io)?;
        if terminal {
            return Err(AgentProfilerVariantServiceErrorV2::ProtocolTerminated);
        }
    }
}

fn read_line<R: BufRead>(
    reader: &mut R,
) -> Result<Option<Vec<u8>>, AgentProfilerVariantServiceErrorV2> {
    let mut line = Vec::new();
    let maximum = usize::try_from(MAX_AGENT_PROFILER_VARIANT_REQUEST_BYTES_V2)
        .map_err(|_| AgentProfilerVariantServiceErrorV2::SizeOverflow)?;
    loop {
        let buffer = reader
            .fill_buf()
            .map_err(|_| AgentProfilerVariantServiceErrorV2::Io)?;
        if buffer.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(AgentProfilerVariantServiceErrorV2::InvalidRequest)
            };
        }
        let newline = buffer.iter().position(|byte| *byte == b'\n');
        let available = newline.map_or(buffer.len(), |position| position + 1);
        let consumed = available.min(maximum.saturating_add(1).saturating_sub(line.len()));
        line.extend_from_slice(&buffer[..consumed]);
        reader.consume(consumed);
        if line.len() > maximum {
            return Err(AgentProfilerVariantServiceErrorV2::RequestTooLarge);
        }
        if newline.is_some() && consumed == available {
            return Ok(Some(line));
        }
    }
}

fn write_terminal<W: Write>(
    output: &mut W,
    service: &mut AgentProfilerVariantServiceV2,
    code: AgentProfilerVariantErrorCodeV2,
) -> Result<(), AgentProfilerVariantServiceErrorV2> {
    let response = service.terminal_protocol_error(code)?;
    output
        .write_all(&service.encode_response(&response)?)
        .map_err(|_| AgentProfilerVariantServiceErrorV2::Io)?;
    output
        .flush()
        .map_err(|_| AgentProfilerVariantServiceErrorV2::Io)
}

fn response_identity<T: Serialize>(
    value: &T,
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV2> {
    let value =
        serde_json::to_value(value).map_err(|_| AgentProfilerVariantServiceErrorV2::JsonEncode)?;
    let bytes =
        serde_json::to_vec(&value).map_err(|_| AgentProfilerVariantServiceErrorV2::JsonEncode)?;
    content_identity(RESPONSE_DOMAIN_V2, &bytes, 2)
}

fn content_identity(
    domain: &[u8],
    bytes: &[u8],
    format_version: u16,
) -> Result<ContentIdentityRecordV1, AgentProfilerVariantServiceErrorV2> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version,
        digest: CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| AgentProfilerVariantServiceErrorV2::Identity)?,
        canonical_len: bytes.len() as u64,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentProfilerVariantServiceErrorV2 {
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

impl fmt::Display for AgentProfilerVariantServiceErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "profiler Variant V2 service rejected input: {self:?}"
        )
    }
}

impl Error for AgentProfilerVariantServiceErrorV2 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_fields_uppercase_hex_and_duplicate_request_ids() {
        assert!(decode_agent_profiler_variant_request_line_v2(
            br#"{"operation":"discover_capabilities","schema":"fe2o3-agent-profiler-variant-request-v2","request_id":1,"expected_revision":0,"unknown":true}\n"#,
        )
        .is_err());
        assert_eq!(
            decode_hex("AA").unwrap_err(),
            AgentProfilerVariantErrorCodeV2::InvalidEvidenceEncoding
        );
        let mut service = AgentProfilerVariantServiceV2::new().unwrap();
        let request = || AgentProfilerVariantRequestV2::DiscoverCapabilities {
            schema: AGENT_PROFILER_VARIANT_REQUEST_SCHEMA_V2.to_owned(),
            request_id: 7,
            expected_revision: 0,
        };
        let accepted = service.handle(request()).unwrap();
        assert!(matches!(
            &accepted,
            AgentProfilerVariantResponseV2::Ok { .. }
        ));
        let mut forged = accepted.clone();
        match &mut forged {
            AgentProfilerVariantResponseV2::Ok {
                response_identity, ..
            }
            | AgentProfilerVariantResponseV2::Error {
                response_identity, ..
            } => response_identity.digest = CaptureIdentityV1::new([99; 32]).unwrap(),
        }
        assert_eq!(
            service.encode_response(&forged).unwrap_err(),
            AgentProfilerVariantServiceErrorV2::InvalidResponse
        );
        assert!(matches!(
            service.handle(request()).unwrap(),
            AgentProfilerVariantResponseV2::Error {
                code: AgentProfilerVariantErrorCodeV2::DuplicateRequestId,
                ..
            }
        ));
    }
}

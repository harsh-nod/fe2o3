//! Exact, address-free attribution of admitted rocprof PC samples to a
//! production source/ISA characteristic archive.

use std::error::Error;
use std::fmt;
use std::io::{self, BufRead, Write};

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, LossStatusV1,
    MAX_IMPORT_SOURCE_BYTES_V1, MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
    MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1, PcSampleCaptureCoverageV3,
    decode_pc_sample_capture_v3, decode_pc_sample_code_object_relation_v1,
};
use fe2o3_source_isa_observation::characteristic_v1::{
    InertSourceIsaCharacteristicCollectionV1, MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1,
    MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1, SourceIsaCharacteristicCategoryV1,
    SourceIsaCharacteristicCollectionV1, SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
    SourceIsaCharacteristicIsaIntervalV1, SourceIsaCharacteristicKindV1,
    SourceIsaCharacteristicKirCoordinateV1, SourceIsaCharacteristicMemoryFormV1,
    SourceIsaCharacteristicMirCoordinateV1, SourceIsaCharacteristicMissingReasonV1,
    SourceIsaCharacteristicObservationErrorV1, SourceIsaCharacteristicRecordKindV1,
    SourceIsaCharacteristicScanStateV1, SourceIsaCharacteristicSourceCoordinateV1,
    SourceIsaCharacteristicTargetProfileV1, SourceIsaCharacteristicTargetV1,
    SourceIsaCharacteristicTransformationV1, SourceIsaCharacteristicUnavailableReasonV1,
    source_isa_characteristic_target_correlation_match_identity_v1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1, PcSampleCodeObjectQueryErrorV1,
    PcSampleCodeObjectQueryLimitsV1, PcSampleCodeObjectQueryResultV1,
    PcSampleCodeObjectQuerySessionV1, PcSampleCodeObjectQueryUnavailableReasonV1,
    PcSampleResolvedSymbolPcV1,
};

pub const AGENT_PC_SOURCE_ISA_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-pc-source-isa-request-v1";
pub const AGENT_PC_SOURCE_ISA_RESPONSE_SCHEMA_V1: &str = "fe2o3-agent-pc-source-isa-response-v1";
pub const MAX_AGENT_PC_SOURCE_ISA_REQUESTS_V1: u32 = 64;
pub const MAX_AGENT_PC_SOURCE_ISA_PAGE_ITEMS_V1: u16 = MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1;
pub const MAX_AGENT_PC_SOURCE_ISA_RESPONSE_BYTES_V1: u64 = 1024 * 1024;
pub const MAX_AGENT_PC_SOURCE_ISA_REQUEST_BYTES_V1: u64 = 2
    * (MAX_IMPORT_SOURCE_BYTES_V1
        + MAX_PC_SAMPLE_CAPTURE_BYTES_V3
        + MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1
        + MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1
        + MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64)
    + 64 * 1024;

const INPUT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.pc-source-isa.input.v1\0";
const BINDING_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.pc-source-isa.binding.v1\0";
const QUERY_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.pc-source-isa.query.v1\0";
const ITEM_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.pc-source-isa.item.v1\0";
const CURSOR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.pc-source-isa.cursor.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSourceIsaLimitsV1 {
    pub max_source_bytes: u64,
    pub max_capture_bytes: u64,
    pub max_artifact_bytes: u64,
    pub max_relation_bytes: u64,
    pub max_characteristic_bytes: u64,
    pub max_page_items: u16,
    pub max_response_bytes: u64,
}

impl Default for PcSourceIsaLimitsV1 {
    fn default() -> Self {
        Self {
            max_source_bytes: MAX_IMPORT_SOURCE_BYTES_V1,
            max_capture_bytes: MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
            max_artifact_bytes: MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1,
            max_relation_bytes: MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1,
            max_characteristic_bytes: MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64,
            max_page_items: MAX_AGENT_PC_SOURCE_ISA_PAGE_ITEMS_V1,
            max_response_bytes: MAX_AGENT_PC_SOURCE_ISA_RESPONSE_BYTES_V1,
        }
    }
}

impl PcSourceIsaLimitsV1 {
    pub fn validate(self) -> Result<Self, PcSourceIsaErrorV1> {
        if self.max_source_bytes == 0
            || self.max_source_bytes > MAX_IMPORT_SOURCE_BYTES_V1
            || self.max_capture_bytes == 0
            || self.max_capture_bytes > MAX_PC_SAMPLE_CAPTURE_BYTES_V3
            || self.max_artifact_bytes == 0
            || self.max_artifact_bytes > MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1
            || self.max_relation_bytes == 0
            || self.max_relation_bytes > MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1
            || self.max_characteristic_bytes == 0
            || self.max_characteristic_bytes
                > MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64
            || self.max_page_items == 0
            || self.max_page_items > MAX_AGENT_PC_SOURCE_ISA_PAGE_ITEMS_V1
            || !(4096..=MAX_AGENT_PC_SOURCE_ISA_RESPONSE_BYTES_V1)
                .contains(&self.max_response_bytes)
        {
            return Err(PcSourceIsaErrorV1::LimitOutOfRange);
        }
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaInputIdentityV1 {
    pub scheme: &'static str,
    pub digest: CaptureIdentityV1,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSourceIsaScanAvailabilityV1 {
    Complete,
    Missing,
    Unavailable,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaScanSummaryV1 {
    pub availability: PcSourceIsaScanAvailabilityV1,
    pub reason_code: Option<u16>,
    pub reason: Option<&'static str>,
    pub catalog_record_count: u64,
    pub catalog_records_scanned: u64,
    pub target_operation_count: u64,
    pub target_operations_scanned: u64,
    pub retained_target_correlations: u64,
    pub target_eliminated_correlations: u64,
    pub correlations_without_source_provenance: u64,
    pub pre_kir_eliminations: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSourceIsaMovedStateV1 {
    UnavailableNotRepresentedByCharacteristicV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSourceIsaComparisonStateV1 {
    UnavailableRequiresTwoExactComparableBoundSessions,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaBindingV1 {
    pub binding_identity: CaptureIdentityV1,
    pub source_input: PcSourceIsaInputIdentityV1,
    pub capture_input: PcSourceIsaInputIdentityV1,
    pub relation_input: PcSourceIsaInputIdentityV1,
    pub artifact_input: PcSourceIsaInputIdentityV1,
    pub characteristic_input: PcSourceIsaInputIdentityV1,
    pub capture_identity: ContentIdentityRecordV1,
    pub relation_identity: ContentIdentityRecordV1,
    pub artifact_identity: ContentIdentityRecordV1,
    pub characteristic_identity: CaptureIdentityV1,
    pub exact_artifact_target: String,
    pub characteristic_target: &'static str,
    pub capture_coverage: PcSampleCaptureCoverageV3,
    pub loss: LossStatusV1,
    pub characteristic_scan: PcSourceIsaScanSummaryV1,
    pub authority: &'static str,
    pub source_archive_authentication: &'static str,
    pub scope: &'static str,
    pub moved_transformation_state: PcSourceIsaMovedStateV1,
    pub semantic_ir_isa_delta_comparison: PcSourceIsaComparisonStateV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSourceIsaCursorV1 {
    pub binding_identity: CaptureIdentityV1,
    pub query_identity: CaptureIdentityV1,
    pub next_ordinal: u64,
    pub preceding_item_identity: CaptureIdentityV1,
    pub cursor_identity: CaptureIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSourceIsaPageRequestV1 {
    pub limit: u16,
    #[serde(default)]
    pub cursor: Option<PcSourceIsaCursorV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSourceIsaAttributionStateV1 {
    UniqueSource,
    UniqueNoSourceProvenance,
    DuplicatedIntervalOccurrences,
    AmbiguousOverlappingCorrelations,
    NoMatchingIsaInterval,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaSpanV1 {
    pub file_identity: CaptureIdentityV1,
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaSourceV1 {
    pub node_identity: CaptureIdentityV1,
    pub span: PcSourceIsaSpanV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaMirV1 {
    pub body_ordinal: u64,
    pub block_ordinal: u64,
    pub statement_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaKirV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub operation_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaLlvmV1 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub instruction_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaIntervalV1 {
    pub kernel_ordinal: u64,
    pub symbol_relative_start: u64,
    pub symbol_relative_end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaAttributionItemV1 {
    pub item_identity: CaptureIdentityV1,
    pub correlation_occurrence_identity: CaptureIdentityV1,
    pub characteristic_identity: CaptureIdentityV1,
    pub interval_ordinal: u64,
    pub catalog_record_ordinal: u64,
    pub category_code: u16,
    pub category: &'static str,
    pub kind_code: u16,
    pub kind: &'static str,
    pub record_kind_code: u8,
    pub record_kind: &'static str,
    pub source: Option<PcSourceIsaSourceV1>,
    pub mir_node_identity: Option<CaptureIdentityV1>,
    pub mir: Option<PcSourceIsaMirV1>,
    pub neutral_kir_node_identity: Option<CaptureIdentityV1>,
    pub neutral_kir: Option<PcSourceIsaKirV1>,
    pub target_kir: PcSourceIsaKirV1,
    pub semantic_operation_identity: CaptureIdentityV1,
    pub compiler_handoff_llvm: PcSourceIsaLlvmV1,
    pub isa: PcSourceIsaIntervalV1,
    pub transformation_code: u8,
    pub transformation: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PcSourceIsaPageV1 {
    pub binding: PcSourceIsaBindingV1,
    pub resolved_pc: PcSampleResolvedSymbolPcV1,
    pub query_identity: CaptureIdentityV1,
    pub attribution_state: PcSourceIsaAttributionStateV1,
    pub singular_attribution: bool,
    pub total_matching_interval_occurrences: u64,
    pub returned: u16,
    pub next_cursor: Option<PcSourceIsaCursorV1>,
    pub items: Vec<PcSourceIsaAttributionItemV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum PcSourceIsaQueryResultV1 {
    Binding {
        binding: PcSourceIsaBindingV1,
    },
    AttributionPage {
        page: Box<PcSourceIsaPageV1>,
    },
    PcUnavailable {
        binding: PcSourceIsaBindingV1,
        reason: PcSampleCodeObjectQueryUnavailableReasonV1,
    },
}

pub struct PcSourceIsaSessionV1 {
    pc: PcSampleCodeObjectQuerySessionV1,
    characteristic: SourceIsaCharacteristicCollectionV1,
    binding: PcSourceIsaBindingV1,
    limits: PcSourceIsaLimitsV1,
}

impl PcSourceIsaSessionV1 {
    pub fn open(
        source: &[u8],
        capture: &[u8],
        artifact: &[u8],
        relation: &[u8],
        characteristic: &[u8],
        limits: PcSourceIsaLimitsV1,
    ) -> Result<Self, PcSourceIsaErrorV1> {
        let limits = limits.validate()?;
        for (bytes, limit) in [
            (source, limits.max_source_bytes),
            (capture, limits.max_capture_bytes),
            (artifact, limits.max_artifact_bytes),
            (relation, limits.max_relation_bytes),
            (characteristic, limits.max_characteristic_bytes),
        ] {
            check_size(bytes, limit)?;
        }
        let decoded_capture = decode_pc_sample_capture_v3(capture)
            .map_err(|_| PcSourceIsaErrorV1::CaptureAdmission)?;
        let decoded_relation = decode_pc_sample_code_object_relation_v1(relation, capture)
            .map_err(|_| PcSourceIsaErrorV1::RelationAdmission)?;
        let pc = PcSampleCodeObjectQuerySessionV1::open(
            source,
            capture,
            artifact,
            relation,
            PcSampleCodeObjectQueryLimitsV1::new(
                limits.max_source_bytes,
                limits.max_capture_bytes,
                limits.max_relation_bytes,
                limits.max_artifact_bytes,
            )
            .map_err(PcSourceIsaErrorV1::Pc)?,
        )
        .map_err(PcSourceIsaErrorV1::Pc)?;
        let input_identities = [
            input_identity(source),
            input_identity(capture),
            input_identity(relation),
            input_identity(artifact),
            input_identity(characteristic),
        ];
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(characteristic)
            .map_err(|_| PcSourceIsaErrorV1::CharacteristicAdmission)?;
        let characteristic = inert.into_self_claimed_archive_for_agent_inspection_v1();
        let characteristic_artifact = characteristic.binding().artifact();
        if decoded_relation.artifact_identity.scheme != ContentSchemeV1::RawCanonicalSha256
            || decoded_relation.artifact_identity.format_version != 1
            || decoded_relation.artifact_identity.digest.as_bytes()
                != characteristic_artifact.sha256()
            || decoded_relation.artifact_identity.canonical_len
                != characteristic_artifact.byte_len()
        {
            return Err(PcSourceIsaErrorV1::ArtifactSubstitution);
        }
        let inspected = fe2o3_hsaco::inspect(artifact)
            .map_err(|_| PcSourceIsaErrorV1::ArtifactTargetAdmission)?;
        let exact_artifact_target = inspected.target().to_string();
        let characteristic_target = target_name(characteristic.binding().target_profile());
        if exact_artifact_target != characteristic_target {
            return Err(PcSourceIsaErrorV1::TargetSubstitution);
        }
        let mut digest = Sha256::new();
        digest.update(BINDING_IDENTITY_DOMAIN_V1);
        for input in &input_identities {
            digest.update(input.digest.as_bytes());
            digest.update(input.byte_len.to_le_bytes());
        }
        digest.update(characteristic.identity());
        digest.update(exact_artifact_target.as_bytes());
        let binding_identity = identity(digest.finalize().into());
        let characteristic_scan = scan_summary(&characteristic);
        let capture_coverage = decoded_capture.coverage;
        let binding = PcSourceIsaBindingV1 {
            binding_identity,
            source_input: input_identities[0].clone(),
            capture_input: input_identities[1].clone(),
            relation_input: input_identities[2].clone(),
            artifact_input: input_identities[3].clone(),
            characteristic_input: input_identities[4].clone(),
            capture_identity: decoded_relation.capture_identity,
            relation_identity: pc_relation_identity(relation, capture)?,
            artifact_identity: decoded_relation.artifact_identity,
            characteristic_identity: identity(characteristic.identity()),
            exact_artifact_target,
            characteristic_target,
            loss: capture_coverage.loss,
            capture_coverage,
            characteristic_scan,
            authority: "read_only_no_collection_compiler_proof_load_dispatch_attach_or_execution_authority",
            source_archive_authentication: "canonical_self_claimed_archive_exactly_bound_to_artifact_bytes_not_producer_authenticated",
            scope: "stochastic_sample_and_sparse_characteristic_intervals_only_not_complete_instruction_history_or_schedule",
            moved_transformation_state:
                PcSourceIsaMovedStateV1::UnavailableNotRepresentedByCharacteristicV1,
            semantic_ir_isa_delta_comparison:
                PcSourceIsaComparisonStateV1::UnavailableRequiresTwoExactComparableBoundSessions,
        };
        Ok(Self {
            pc,
            characteristic,
            binding,
            limits,
        })
    }

    pub const fn binding(&self) -> &PcSourceIsaBindingV1 {
        &self.binding
    }

    pub fn inspect_binding(&self) -> PcSourceIsaQueryResultV1 {
        PcSourceIsaQueryResultV1::Binding {
            binding: self.binding.clone(),
        }
    }

    pub fn lookup_sample(
        &self,
        sample: CaptureIdentityV1,
        page: PcSourceIsaPageRequestV1,
    ) -> Result<PcSourceIsaQueryResultV1, PcSourceIsaErrorV1> {
        self.page(self.pc.lookup_sample(sample), page)
    }

    pub fn lookup_code_object_pc(
        &self,
        code_object: CaptureIdentityV1,
        code_object_offset: u64,
        page: PcSourceIsaPageRequestV1,
    ) -> Result<PcSourceIsaQueryResultV1, PcSourceIsaErrorV1> {
        self.page(
            self.pc
                .lookup_code_object_pc(code_object, code_object_offset),
            page,
        )
    }

    pub fn query_json(
        &self,
        result: &PcSourceIsaQueryResultV1,
    ) -> Result<Vec<u8>, PcSourceIsaErrorV1> {
        encode_bounded(result, self.limits.max_response_bytes)
    }

    fn page(
        &self,
        pc: PcSampleCodeObjectQueryResultV1,
        request: PcSourceIsaPageRequestV1,
    ) -> Result<PcSourceIsaQueryResultV1, PcSourceIsaErrorV1> {
        if request.limit == 0 || request.limit > self.limits.max_page_items {
            return Err(PcSourceIsaErrorV1::PageLimit);
        }
        let PcSampleCodeObjectQueryResultV1::Resolved { pc } = pc else {
            let PcSampleCodeObjectQueryResultV1::Unavailable { reason } = pc else {
                unreachable!()
            };
            return Ok(PcSourceIsaQueryResultV1::PcUnavailable {
                binding: self.binding.clone(),
                reason,
            });
        };
        let query_identity = query_identity(self.binding.binding_identity, pc);
        let start = request.cursor.map_or(Ok(0), |cursor| {
            validate_cursor(self.binding.binding_identity, query_identity, cursor)
        })?;
        let mut total = 0_u64;
        let mut preceding = None;
        let mut items = Vec::new();
        items
            .try_reserve_exact(usize::from(request.limit))
            .map_err(|_| PcSourceIsaErrorV1::AllocationFailure)?;
        let mut first_correlation = None;
        let mut multiple_correlations = false;
        let mut any_without_source = false;
        for target in self.characteristic.targets() {
            for correlation in target.correlations() {
                let correlation_occurrence = identity(
                    source_isa_characteristic_target_correlation_match_identity_v1(
                        self.characteristic.identity(),
                        target.identity(),
                        correlation.catalog_record_ordinal(),
                    ),
                );
                for (interval_ordinal, interval) in correlation.isa_intervals().iter().enumerate() {
                    if !interval
                        .contains(u64::from(pc.metadata_kernel_ordinal), pc.symbol_relative_pc)
                    {
                        continue;
                    }
                    let interval_ordinal = u64::try_from(interval_ordinal)
                        .map_err(|_| PcSourceIsaErrorV1::ResourceLimit)?;
                    let item_identity = attribution_identity(
                        self.binding.binding_identity,
                        correlation_occurrence,
                        interval_ordinal,
                        *interval,
                    );
                    multiple_correlations |=
                        first_correlation.is_some_and(|first| first != correlation_occurrence);
                    if first_correlation.is_none() {
                        first_correlation = Some(correlation_occurrence);
                    }
                    any_without_source |= correlation.source().is_none();
                    if total
                        .checked_add(1)
                        .ok_or(PcSourceIsaErrorV1::ResourceLimit)?
                        <= start
                    {
                        preceding = Some(item_identity);
                    } else if items.len() < usize::from(request.limit) {
                        items.push(project_item(
                            target,
                            correlation_occurrence,
                            item_identity,
                            interval_ordinal,
                            correlation,
                            *interval,
                        ));
                    }
                    total = total
                        .checked_add(1)
                        .ok_or(PcSourceIsaErrorV1::ResourceLimit)?;
                }
            }
        }
        if start > total
            || request.cursor.is_some() && (start == 0 || start == total)
            || request
                .cursor
                .is_some_and(|cursor| preceding != Some(cursor.preceding_item_identity))
        {
            return Err(PcSourceIsaErrorV1::CursorMismatch);
        }
        let returned = u16::try_from(items.len()).map_err(|_| PcSourceIsaErrorV1::ResourceLimit)?;
        let next = start
            .checked_add(u64::from(returned))
            .ok_or(PcSourceIsaErrorV1::ResourceLimit)?;
        let next_cursor = if next < total {
            Some(make_cursor(
                self.binding.binding_identity,
                query_identity,
                next,
                items
                    .last()
                    .ok_or(PcSourceIsaErrorV1::CursorMismatch)?
                    .item_identity,
            ))
        } else {
            None
        };
        let attribution_state = if total == 0 {
            PcSourceIsaAttributionStateV1::NoMatchingIsaInterval
        } else if multiple_correlations {
            PcSourceIsaAttributionStateV1::AmbiguousOverlappingCorrelations
        } else if total > 1 {
            PcSourceIsaAttributionStateV1::DuplicatedIntervalOccurrences
        } else if any_without_source {
            PcSourceIsaAttributionStateV1::UniqueNoSourceProvenance
        } else {
            PcSourceIsaAttributionStateV1::UniqueSource
        };
        Ok(PcSourceIsaQueryResultV1::AttributionPage {
            page: Box::new(PcSourceIsaPageV1 {
                binding: self.binding.clone(),
                resolved_pc: pc,
                query_identity,
                singular_attribution: matches!(
                    attribution_state,
                    PcSourceIsaAttributionStateV1::UniqueSource
                        | PcSourceIsaAttributionStateV1::UniqueNoSourceProvenance
                ),
                attribution_state,
                total_matching_interval_occurrences: total,
                returned,
                next_cursor,
                items,
            }),
        })
    }
}

fn check_size(bytes: &[u8], limit: u64) -> Result<(), PcSourceIsaErrorV1> {
    let len = u64::try_from(bytes.len()).map_err(|_| PcSourceIsaErrorV1::ResourceLimit)?;
    if len == 0 || len > limit {
        return Err(PcSourceIsaErrorV1::InputTooLarge);
    }
    Ok(())
}

fn pc_relation_identity(
    relation: &[u8],
    capture: &[u8],
) -> Result<ContentIdentityRecordV1, PcSourceIsaErrorV1> {
    fe2o3_semantic_import::pc_sample_code_object_relation_content_identity_v1(relation, capture)
        .map_err(|_| PcSourceIsaErrorV1::RelationAdmission)
}

pub(crate) fn target_name(target: SourceIsaCharacteristicTargetProfileV1) -> &'static str {
    match target {
        SourceIsaCharacteristicTargetProfileV1::Gfx942 => "gfx942:xnack-",
        SourceIsaCharacteristicTargetProfileV1::Gfx950 => "gfx950:xnack-",
    }
}

pub(crate) fn scan_summary(
    collection: &SourceIsaCharacteristicCollectionV1,
) -> PcSourceIsaScanSummaryV1 {
    let scan = collection.scan();
    let (availability, reason_code, reason) = match scan.state() {
        SourceIsaCharacteristicScanStateV1::Complete => {
            (PcSourceIsaScanAvailabilityV1::Complete, None, None)
        }
        SourceIsaCharacteristicScanStateV1::Missing(value) => (
            PcSourceIsaScanAvailabilityV1::Missing,
            Some(value as u16),
            Some(match value {
                SourceIsaCharacteristicMissingReasonV1::NoQualifyingTargetOperation => {
                    "no_qualifying_target_operation"
                }
                SourceIsaCharacteristicMissingReasonV1::NoAdmittedCorrelation => {
                    "no_admitted_correlation"
                }
            }),
        ),
        SourceIsaCharacteristicScanStateV1::Unavailable(reason) => (
            PcSourceIsaScanAvailabilityV1::Unavailable,
            Some(reason as u16),
            Some(match reason {
                SourceIsaCharacteristicUnavailableReasonV1::CatalogUnavailable => {
                    "catalog_unavailable"
                }
                SourceIsaCharacteristicUnavailableReasonV1::StructuralBridgeUnavailable => {
                    "structural_bridge_unavailable"
                }
                SourceIsaCharacteristicUnavailableReasonV1::ClassifierUnavailable => {
                    "classifier_unavailable"
                }
                SourceIsaCharacteristicUnavailableReasonV1::SourceProjectionUnavailable => {
                    "source_projection_unavailable"
                }
            }),
        ),
        SourceIsaCharacteristicScanStateV1::Error(value) => (
            PcSourceIsaScanAvailabilityV1::Error,
            Some(value as u16),
            Some(match value {
                SourceIsaCharacteristicObservationErrorV1::InvalidCatalog => "invalid_catalog",
                SourceIsaCharacteristicObservationErrorV1::InvalidStructuralBridge => {
                    "invalid_structural_bridge"
                }
                SourceIsaCharacteristicObservationErrorV1::InvalidClassification => {
                    "invalid_classification"
                }
                SourceIsaCharacteristicObservationErrorV1::ConflictingEvidence => {
                    "conflicting_evidence"
                }
                SourceIsaCharacteristicObservationErrorV1::ResourceLimit => "resource_limit",
                SourceIsaCharacteristicObservationErrorV1::AllocationFailure => {
                    "allocation_failure"
                }
            }),
        ),
    };
    PcSourceIsaScanSummaryV1 {
        availability,
        reason_code,
        reason,
        catalog_record_count: scan.catalog_record_count(),
        catalog_records_scanned: scan.catalog_records_scanned(),
        target_operation_count: scan.target_operation_count(),
        target_operations_scanned: scan.target_operations_scanned(),
        retained_target_correlations: scan.retained_target_correlation_count(),
        target_eliminated_correlations: collection
            .targets()
            .iter()
            .flat_map(|target| target.correlations())
            .filter(|correlation| {
                correlation.transformation() == SourceIsaCharacteristicTransformationV1::Eliminated
            })
            .count() as u64,
        correlations_without_source_provenance: collection
            .targets()
            .iter()
            .flat_map(|target| target.correlations())
            .filter(|correlation| correlation.source().is_none())
            .count() as u64,
        pre_kir_eliminations: scan.pre_kir_elimination_count(),
    }
}

pub(crate) fn project_item(
    target: &SourceIsaCharacteristicTargetV1,
    correlation_occurrence_identity: CaptureIdentityV1,
    item_identity: CaptureIdentityV1,
    interval_ordinal: u64,
    correlation: &fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicTargetCorrelationV1,
    interval: SourceIsaCharacteristicIsaIntervalV1,
) -> PcSourceIsaAttributionItemV1 {
    PcSourceIsaAttributionItemV1 {
        item_identity,
        correlation_occurrence_identity,
        characteristic_identity: identity(target.identity()),
        interval_ordinal,
        catalog_record_ordinal: correlation.catalog_record_ordinal(),
        category_code: target.category().code(),
        category: category_name(target.category()),
        kind_code: target.kind().code(),
        kind: kind_name(target.kind()),
        record_kind_code: correlation.kind().code(),
        record_kind: record_kind_name(correlation.kind()),
        source: correlation.source().map(project_source),
        mir_node_identity: correlation.mir_node_identity().map(identity),
        mir: correlation.mir().map(project_mir),
        neutral_kir_node_identity: correlation.neutral_kir_node_identity().map(identity),
        neutral_kir: correlation.neutral_kir().map(project_kir),
        target_kir: project_kir(correlation.target_kir()),
        semantic_operation_identity: identity(correlation.semantic_operation_identity()),
        compiler_handoff_llvm: project_llvm(correlation.compiler_handoff_llvm()),
        isa: project_interval(interval),
        transformation_code: correlation.transformation().code(),
        transformation: transformation_name(correlation.transformation()),
    }
}

fn project_source(value: SourceIsaCharacteristicSourceCoordinateV1) -> PcSourceIsaSourceV1 {
    let span = value.span();
    PcSourceIsaSourceV1 {
        node_identity: identity(value.node_identity()),
        span: PcSourceIsaSpanV1 {
            file_identity: identity(span.file_identity()),
            byte_start: span.byte_start(),
            byte_end: span.byte_end(),
            line: span.line(),
            column: span.column(),
        },
    }
}

fn project_mir(value: SourceIsaCharacteristicMirCoordinateV1) -> PcSourceIsaMirV1 {
    PcSourceIsaMirV1 {
        body_ordinal: value.body_ordinal(),
        block_ordinal: value.block_ordinal(),
        statement_ordinal: value.statement_ordinal(),
    }
}

fn project_kir(value: SourceIsaCharacteristicKirCoordinateV1) -> PcSourceIsaKirV1 {
    PcSourceIsaKirV1 {
        function_ordinal: value.function_ordinal(),
        block_ordinal: value.block_ordinal(),
        operation_ordinal: value.operation_ordinal(),
    }
}

fn project_llvm(
    value: SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
) -> PcSourceIsaLlvmV1 {
    PcSourceIsaLlvmV1 {
        function_ordinal: value.function_ordinal(),
        block_ordinal: value.block_ordinal(),
        instruction_ordinal: value.instruction_ordinal(),
    }
}

fn project_interval(value: SourceIsaCharacteristicIsaIntervalV1) -> PcSourceIsaIntervalV1 {
    PcSourceIsaIntervalV1 {
        kernel_ordinal: value.kernel_ordinal(),
        symbol_relative_start: value.symbol_relative_start(),
        symbol_relative_end: value.symbol_relative_end(),
    }
}

fn transformation_name(value: SourceIsaCharacteristicTransformationV1) -> &'static str {
    match value {
        SourceIsaCharacteristicTransformationV1::Preserved => "preserved",
        SourceIsaCharacteristicTransformationV1::Duplicated => "duplicated",
        SourceIsaCharacteristicTransformationV1::Coalesced => "coalesced",
        SourceIsaCharacteristicTransformationV1::DuplicatedAndCoalesced => {
            "duplicated_and_coalesced"
        }
        SourceIsaCharacteristicTransformationV1::Eliminated => "eliminated",
    }
}

fn category_name(value: SourceIsaCharacteristicCategoryV1) -> &'static str {
    match value {
        SourceIsaCharacteristicCategoryV1::TargetKirGlobalStore => "target_kir_global_store",
        SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsRead => {
            "target_kir_workgroup_lds_read"
        }
        SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupLdsWrite => {
            "target_kir_workgroup_lds_write"
        }
        SourceIsaCharacteristicCategoryV1::TargetKirWorkgroupBarrier => {
            "target_kir_workgroup_barrier"
        }
        SourceIsaCharacteristicCategoryV1::TargetKirBf16MfmaExact => "target_kir_bf16_mfma_exact",
    }
}

fn kind_name(value: SourceIsaCharacteristicKindV1) -> &'static str {
    match value {
        SourceIsaCharacteristicKindV1::GlobalStore { form } => match form {
            SourceIsaCharacteristicMemoryFormV1::Plain => "global_store_plain",
            SourceIsaCharacteristicMemoryFormV1::Guarded => "global_store_guarded",
            SourceIsaCharacteristicMemoryFormV1::MatrixTile => "global_store_matrix_tile",
        },
        SourceIsaCharacteristicKindV1::WorkgroupLoad { form } => match form {
            SourceIsaCharacteristicMemoryFormV1::Plain => "workgroup_load_plain",
            SourceIsaCharacteristicMemoryFormV1::Guarded => "workgroup_load_guarded",
            SourceIsaCharacteristicMemoryFormV1::MatrixTile => "workgroup_load_matrix_tile",
        },
        SourceIsaCharacteristicKindV1::WorkgroupStore { form } => match form {
            SourceIsaCharacteristicMemoryFormV1::Plain => "workgroup_store_plain",
            SourceIsaCharacteristicMemoryFormV1::Guarded => "workgroup_store_guarded",
            SourceIsaCharacteristicMemoryFormV1::MatrixTile => "workgroup_store_matrix_tile",
        },
        SourceIsaCharacteristicKindV1::WorkgroupBarrier => "workgroup_barrier",
        SourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
            "bf16_f32_m16n16k16_wave64_matrix_multiply_accumulate"
        }
    }
}

fn record_kind_name(value: SourceIsaCharacteristicRecordKindV1) -> &'static str {
    match value {
        SourceIsaCharacteristicRecordKindV1::EliminatedBeforeKir => "eliminated_before_kir",
        SourceIsaCharacteristicRecordKindV1::SourceAnchored => "source_anchored",
        SourceIsaCharacteristicRecordKindV1::NoSourceProvenance => "no_source_provenance",
    }
}

fn query_identity(binding: CaptureIdentityV1, pc: PcSampleResolvedSymbolPcV1) -> CaptureIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(QUERY_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update(pc.capture_identity.digest.as_bytes());
    digest.update(pc.relation_identity.digest.as_bytes());
    digest.update(pc.artifact_identity.digest.as_bytes());
    digest.update(pc.code_object_identity.as_bytes());
    digest.update(pc.kernel_symbol_identity.as_bytes());
    digest.update(pc.metadata_kernel_ordinal.to_le_bytes());
    digest.update(pc.symbol_relative_pc.to_le_bytes());
    if let Some(sample) = pc.sample_identity {
        digest.update([1]);
        digest.update(sample.as_bytes());
    } else {
        digest.update([0]);
    }
    if let Some(dispatch) = pc.dispatch_identity {
        digest.update([1]);
        digest.update(dispatch.as_bytes());
    } else {
        digest.update([0]);
    }
    identity(digest.finalize().into())
}

fn attribution_identity(
    binding: CaptureIdentityV1,
    correlation: CaptureIdentityV1,
    ordinal: u64,
    interval: SourceIsaCharacteristicIsaIntervalV1,
) -> CaptureIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(ITEM_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update(correlation.as_bytes());
    digest.update(ordinal.to_le_bytes());
    digest.update(interval.kernel_ordinal().to_le_bytes());
    digest.update(interval.symbol_relative_start().to_le_bytes());
    digest.update(interval.symbol_relative_end().to_le_bytes());
    identity(digest.finalize().into())
}

fn make_cursor(
    binding: CaptureIdentityV1,
    query: CaptureIdentityV1,
    next: u64,
    preceding: CaptureIdentityV1,
) -> PcSourceIsaCursorV1 {
    let mut digest = Sha256::new();
    digest.update(CURSOR_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update(query.as_bytes());
    digest.update(next.to_le_bytes());
    digest.update(preceding.as_bytes());
    PcSourceIsaCursorV1 {
        binding_identity: binding,
        query_identity: query,
        next_ordinal: next,
        preceding_item_identity: preceding,
        cursor_identity: identity(digest.finalize().into()),
    }
}

fn validate_cursor(
    binding: CaptureIdentityV1,
    query: CaptureIdentityV1,
    cursor: PcSourceIsaCursorV1,
) -> Result<u64, PcSourceIsaErrorV1> {
    if cursor.next_ordinal == 0
        || cursor.binding_identity != binding
        || cursor.query_identity != query
        || cursor
            != make_cursor(
                binding,
                query,
                cursor.next_ordinal,
                cursor.preceding_item_identity,
            )
    {
        return Err(PcSourceIsaErrorV1::CursorMismatch);
    }
    Ok(cursor.next_ordinal)
}

fn input_identity(bytes: &[u8]) -> PcSourceIsaInputIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(INPUT_IDENTITY_DOMAIN_V1);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    PcSourceIsaInputIdentityV1 {
        scheme: "domain_separated_sha256",
        digest: identity(digest.finalize().into()),
        byte_len: bytes.len() as u64,
    }
}

fn identity(bytes: [u8; 32]) -> CaptureIdentityV1 {
    CaptureIdentityV1::new(bytes).expect("domain-separated SHA-256 is nonzero")
}

fn encode_bounded<T: Serialize>(value: &T, max: u64) -> Result<Vec<u8>, PcSourceIsaErrorV1> {
    let mut output = Vec::new();
    let capacity = usize::try_from(max).map_err(|_| PcSourceIsaErrorV1::ResourceLimit)?;
    output
        .try_reserve_exact(capacity)
        .map_err(|_| PcSourceIsaErrorV1::AllocationFailure)?;
    let mut writer = BoundedWriterV1 {
        output: &mut output,
        max,
        exceeded: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.exceeded {
            PcSourceIsaErrorV1::ResponseTooLarge
        } else {
            PcSourceIsaErrorV1::Json
        }
    })?;
    Ok(output)
}

struct BoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    max: u64,
    exceeded: bool,
}

impl Write for BoundedWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let len = u64::try_from(bytes.len()).map_err(|_| io::Error::other("size overflow"))?;
        if (self.output.len() as u64)
            .checked_add(len)
            .is_none_or(|total| total > self.max)
        {
            self.exceeded = true;
            return Err(io::Error::other("bounded response exceeded"));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum PcSourceIsaErrorV1 {
    LimitOutOfRange,
    InputTooLarge,
    CaptureAdmission,
    RelationAdmission,
    CharacteristicAdmission,
    ArtifactTargetAdmission,
    ArtifactSubstitution,
    TargetSubstitution,
    Pc(PcSampleCodeObjectQueryErrorV1),
    PageLimit,
    CursorMismatch,
    AllocationFailure,
    ResourceLimit,
    ResponseTooLarge,
    Json,
}

impl fmt::Display for PcSourceIsaErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "PC/source-ISA attribution rejected: {self:?}")
    }
}

impl Error for PcSourceIsaErrorV1 {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentPcSourceIsaRequestV1 {
    DiscoverCapabilities {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    OpenEvidence {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        source_hex: String,
        capture_hex: String,
        artifact_hex: String,
        relation_hex: String,
        characteristic_hex: String,
    },
    InspectBinding {
        schema: String,
        request_id: u64,
        expected_revision: u64,
    },
    LookupSample {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        sample_identity: CaptureIdentityV1,
        page: PcSourceIsaPageRequestV1,
    },
    LookupCodeObjectPc {
        schema: String,
        request_id: u64,
        expected_revision: u64,
        code_object_identity: CaptureIdentityV1,
        code_object_offset: u64,
        page: PcSourceIsaPageRequestV1,
    },
}

impl AgentPcSourceIsaRequestV1 {
    fn metadata(&self) -> (&str, u64, u64) {
        match self {
            Self::DiscoverCapabilities {
                schema,
                request_id,
                expected_revision,
            }
            | Self::OpenEvidence {
                schema,
                request_id,
                expected_revision,
                ..
            }
            | Self::InspectBinding {
                schema,
                request_id,
                expected_revision,
            }
            | Self::LookupSample {
                schema,
                request_id,
                expected_revision,
                ..
            }
            | Self::LookupCodeObjectPc {
                schema,
                request_id,
                expected_revision,
                ..
            } => (schema, *request_id, *expected_revision),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPcSourceIsaErrorCodeV1 {
    InvalidRequest,
    RequestTooLarge,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    StaleRevision,
    RequestBudgetExhausted,
    EvidenceNotOpen,
    EvidenceEncoding,
    EvidenceAdmission,
    PageLimit,
    CursorMismatch,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentPcSourceIsaResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        value: Box<PcSourceIsaQueryResultV1>,
    },
    Capabilities {
        schema: &'static str,
        request_id: u64,
        response_revision: u64,
        operations: [&'static str; 5],
        max_requests: u32,
        max_page_items: u16,
        max_response_bytes: u64,
        authority: &'static str,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        response_revision: u64,
        code: AgentPcSourceIsaErrorCodeV1,
        terminal: bool,
    },
}

impl AgentPcSourceIsaResponseV1 {
    fn is_terminal(&self) -> bool {
        matches!(self, Self::Error { terminal: true, .. })
    }
}

pub struct AgentPcSourceIsaServiceV1 {
    session: Option<PcSourceIsaSessionV1>,
    request_ids: Vec<u64>,
    request_count: u32,
    revision: u64,
    terminal: bool,
}

impl AgentPcSourceIsaServiceV1 {
    pub fn new() -> Self {
        Self {
            session: None,
            request_ids: Vec::new(),
            request_count: 0,
            revision: 0,
            terminal: false,
        }
    }

    pub fn handle(&mut self, request: AgentPcSourceIsaRequestV1) -> AgentPcSourceIsaResponseV1 {
        let (schema, request_id, expected_revision) = request.metadata();
        if self.terminal || self.request_count >= MAX_AGENT_PC_SOURCE_ISA_REQUESTS_V1 {
            self.terminal = true;
            return self.error(
                None,
                AgentPcSourceIsaErrorCodeV1::RequestBudgetExhausted,
                true,
            );
        }
        self.request_count += 1;
        if request_id == 0 {
            return self.error(None, AgentPcSourceIsaErrorCodeV1::InvalidRequestId, false);
        }
        if self.request_ids.contains(&request_id) {
            return self.error(
                Some(request_id),
                AgentPcSourceIsaErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        self.request_ids.push(request_id);
        if schema != AGENT_PC_SOURCE_ISA_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentPcSourceIsaErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if expected_revision != self.revision {
            return self.error(
                Some(request_id),
                AgentPcSourceIsaErrorCodeV1::StaleRevision,
                false,
            );
        }
        match request {
            AgentPcSourceIsaRequestV1::DiscoverCapabilities { .. } => {
                self.revision += 1;
                AgentPcSourceIsaResponseV1::Capabilities {
                    schema: AGENT_PC_SOURCE_ISA_RESPONSE_SCHEMA_V1,
                    request_id,
                    response_revision: self.revision,
                    operations: [
                        "discover_capabilities",
                        "open_evidence",
                        "inspect_binding",
                        "lookup_sample",
                        "lookup_code_object_pc",
                    ],
                    max_requests: MAX_AGENT_PC_SOURCE_ISA_REQUESTS_V1,
                    max_page_items: MAX_AGENT_PC_SOURCE_ISA_PAGE_ITEMS_V1,
                    max_response_bytes: MAX_AGENT_PC_SOURCE_ISA_RESPONSE_BYTES_V1,
                    authority: "read_only_no_collection_compiler_proof_load_dispatch_attach_or_execution_authority",
                }
            }
            AgentPcSourceIsaRequestV1::OpenEvidence {
                source_hex,
                capture_hex,
                artifact_hex,
                relation_hex,
                characteristic_hex,
                ..
            } => {
                let opened = decode_open(
                    source_hex,
                    capture_hex,
                    artifact_hex,
                    relation_hex,
                    characteristic_hex,
                )
                .and_then(
                    |(source, capture, artifact, relation, characteristic)| {
                        PcSourceIsaSessionV1::open(
                            &source,
                            &capture,
                            &artifact,
                            &relation,
                            &characteristic,
                            PcSourceIsaLimitsV1::default(),
                        )
                        .map_err(|_| AgentPcSourceIsaErrorCodeV1::EvidenceAdmission)
                    },
                );
                match opened {
                    Ok(session) => {
                        let value = session.inspect_binding();
                        self.session = Some(session);
                        self.ok(request_id, value)
                    }
                    Err(code) => self.error(Some(request_id), code, false),
                }
            }
            AgentPcSourceIsaRequestV1::InspectBinding { .. } => match self.session.as_ref() {
                Some(session) => self.ok(request_id, session.inspect_binding()),
                None => self.error(
                    Some(request_id),
                    AgentPcSourceIsaErrorCodeV1::EvidenceNotOpen,
                    false,
                ),
            },
            AgentPcSourceIsaRequestV1::LookupSample {
                sample_identity,
                page,
                ..
            } => {
                let value = self
                    .session
                    .as_ref()
                    .ok_or(AgentPcSourceIsaErrorCodeV1::EvidenceNotOpen)
                    .and_then(|session| {
                        session
                            .lookup_sample(sample_identity, page)
                            .map_err(map_query_error)
                    });
                match value {
                    Ok(value) => self.ok(request_id, value),
                    Err(code) => self.error(Some(request_id), code, false),
                }
            }
            AgentPcSourceIsaRequestV1::LookupCodeObjectPc {
                code_object_identity,
                code_object_offset,
                page,
                ..
            } => {
                let value = self
                    .session
                    .as_ref()
                    .ok_or(AgentPcSourceIsaErrorCodeV1::EvidenceNotOpen)
                    .and_then(|session| {
                        session
                            .lookup_code_object_pc(code_object_identity, code_object_offset, page)
                            .map_err(map_query_error)
                    });
                match value {
                    Ok(value) => self.ok(request_id, value),
                    Err(code) => self.error(Some(request_id), code, false),
                }
            }
        }
    }

    fn ok(
        &mut self,
        request_id: u64,
        value: PcSourceIsaQueryResultV1,
    ) -> AgentPcSourceIsaResponseV1 {
        self.revision += 1;
        AgentPcSourceIsaResponseV1::Ok {
            schema: AGENT_PC_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.revision,
            value: Box::new(value),
        }
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentPcSourceIsaErrorCodeV1,
        terminal: bool,
    ) -> AgentPcSourceIsaResponseV1 {
        self.revision = self.revision.saturating_add(1);
        AgentPcSourceIsaResponseV1::Error {
            schema: AGENT_PC_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            response_revision: self.revision,
            code,
            terminal,
        }
    }

    fn reject_record(&mut self, code: AgentPcSourceIsaErrorCodeV1) -> AgentPcSourceIsaResponseV1 {
        if self.request_count >= MAX_AGENT_PC_SOURCE_ISA_REQUESTS_V1 {
            self.terminal = true;
            self.error(
                None,
                AgentPcSourceIsaErrorCodeV1::RequestBudgetExhausted,
                true,
            )
        } else {
            self.request_count += 1;
            self.error(None, code, false)
        }
    }
}

impl Default for AgentPcSourceIsaServiceV1 {
    fn default() -> Self {
        Self::new()
    }
}

fn map_query_error(error: PcSourceIsaErrorV1) -> AgentPcSourceIsaErrorCodeV1 {
    match error {
        PcSourceIsaErrorV1::PageLimit => AgentPcSourceIsaErrorCodeV1::PageLimit,
        PcSourceIsaErrorV1::CursorMismatch => AgentPcSourceIsaErrorCodeV1::CursorMismatch,
        _ => AgentPcSourceIsaErrorCodeV1::EvidenceAdmission,
    }
}

type OpenEvidenceV1 = (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>);

fn decode_open(
    source: String,
    capture: String,
    artifact: String,
    relation: String,
    characteristic: String,
) -> Result<OpenEvidenceV1, AgentPcSourceIsaErrorCodeV1> {
    Ok((
        decode_hex(&source, MAX_IMPORT_SOURCE_BYTES_V1)?,
        decode_hex(&capture, MAX_PC_SAMPLE_CAPTURE_BYTES_V3)?,
        decode_hex(&artifact, MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1)?,
        decode_hex(&relation, MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1)?,
        decode_hex(
            &characteristic,
            MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64,
        )?,
    ))
}

fn decode_hex(value: &str, max: u64) -> Result<Vec<u8>, AgentPcSourceIsaErrorCodeV1> {
    if value.is_empty() || !value.len().is_multiple_of(2) || value.len() as u64 > max * 2 {
        return Err(AgentPcSourceIsaErrorCodeV1::EvidenceEncoding);
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(value.len() / 2)
        .map_err(|_| AgentPcSourceIsaErrorCodeV1::EvidenceEncoding)?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high = nibble(pair[0]).ok_or(AgentPcSourceIsaErrorCodeV1::EvidenceEncoding)?;
        let low = nibble(pair[1]).ok_or(AgentPcSourceIsaErrorCodeV1::EvidenceEncoding)?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

pub fn run_agent_pc_source_isa_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentPcSourceIsaServiceErrorV1> {
    let mut service = AgentPcSourceIsaServiceV1::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let mut bounded =
            std::io::Read::take(&mut *input, MAX_AGENT_PC_SOURCE_ISA_REQUEST_BYTES_V1 + 2);
        let read = bounded
            .read_until(b'\n', &mut line)
            .map_err(|_| AgentPcSourceIsaServiceErrorV1::Io)?;
        if read == 0 {
            return Ok(());
        }
        if line.last() != Some(&b'\n')
            || line.len() as u64 > MAX_AGENT_PC_SOURCE_ISA_REQUEST_BYTES_V1 + 1
        {
            let mut response = service.reject_record(AgentPcSourceIsaErrorCodeV1::RequestTooLarge);
            if let AgentPcSourceIsaResponseV1::Error { terminal, .. } = &mut response {
                *terminal = true;
            }
            write_service_response(output, &response)?;
            return Ok(());
        }
        line.pop();
        let request: AgentPcSourceIsaRequestV1 = match serde_json::from_slice(&line) {
            Ok(request)
                if serde_json::to_vec(&request).ok().as_deref() == Some(line.as_slice()) =>
            {
                request
            }
            _ => {
                let response = service.reject_record(AgentPcSourceIsaErrorCodeV1::InvalidRequest);
                write_service_response(output, &response)?;
                if response.is_terminal() {
                    return Ok(());
                }
                continue;
            }
        };
        let response = service.handle(request);
        match write_service_response(output, &response) {
            Ok(()) => {}
            Err(AgentPcSourceIsaServiceErrorV1::ResponseTooLarge) => {
                let terminal =
                    service.error(None, AgentPcSourceIsaErrorCodeV1::ResponseTooLarge, true);
                write_service_response(output, &terminal)?;
                return Ok(());
            }
            Err(error) => return Err(error),
        }
        if response.is_terminal() {
            return Ok(());
        }
    }
}

fn write_service_response(
    output: &mut impl Write,
    response: &AgentPcSourceIsaResponseV1,
) -> Result<(), AgentPcSourceIsaServiceErrorV1> {
    let mut bytes = encode_bounded(response, MAX_AGENT_PC_SOURCE_ISA_RESPONSE_BYTES_V1 - 1)
        .map_err(|error| match error {
            PcSourceIsaErrorV1::ResponseTooLarge => {
                AgentPcSourceIsaServiceErrorV1::ResponseTooLarge
            }
            _ => AgentPcSourceIsaServiceErrorV1::Json,
        })?;
    bytes.push(b'\n');
    output
        .write_all(&bytes)
        .map_err(|_| AgentPcSourceIsaServiceErrorV1::Io)?;
    output
        .flush()
        .map_err(|_| AgentPcSourceIsaServiceErrorV1::Io)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentPcSourceIsaServiceErrorV1 {
    Io,
    Json,
    ResponseTooLarge,
}

impl fmt::Display for AgentPcSourceIsaServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "PC/source-ISA agent service failed: {self:?}")
    }
}
impl Error for AgentPcSourceIsaServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_import::{
        ArtifactClaimV1, ImportLimitsV1, RocprofCaptureBindingV1, RocprofPcSampleCaptureBindingV3,
        admit_rocprofv3_pc_sample_code_object_relation_v1, encode_pc_sample_capture_v3,
        encode_pc_sample_code_object_relation_v1, import_rocprofv3_pc_sample_capture_v3,
    };
    use fe2o3_semantic_trace::{KernelIrIdentityClaimV1, OpaqueIdentityV1, WaveWidthV1};
    use fe2o3_source_isa_observation::characteristic_v1::{
        SourceIsaCharacteristicBindingV1, SourceIsaCharacteristicContentIdentityV1,
        SourceIsaCharacteristicKindV1, SourceIsaCharacteristicMemoryFormV1,
        SourceIsaCharacteristicMissingReasonV1, SourceIsaCharacteristicPreKirEliminationV1,
        SourceIsaCharacteristicScanSummaryV1, SourceIsaCharacteristicSourceSpanV1,
        SourceIsaCharacteristicStructuralCountsV1, SourceIsaCharacteristicTargetCorrelationV1,
        SourceIsaCharacteristicTargetV1,
    };

    const ARTIFACT: &[u8] =
        include_bytes!("../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    const ROCPROF: &[u8] = include_bytes!(
        "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-stochastic-pc-sampling.json"
    );

    struct EvidenceV1 {
        source: Vec<u8>,
        capture: Vec<u8>,
        relation: Vec<u8>,
        characteristic: Vec<u8>,
        samples: Vec<CaptureIdentityV1>,
    }

    #[derive(Clone, Copy)]
    enum CorrelationFixtureV1 {
        UniqueSource,
        UniqueNoSource,
        Duplicated,
        Ambiguous,
        EliminatedOnly,
        Partial,
    }

    fn id(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn content(byte: u8, len: u64) -> SourceIsaCharacteristicContentIdentityV1 {
        SourceIsaCharacteristicContentIdentityV1::new(id(byte), len).unwrap()
    }

    fn source_coordinate(byte: u8) -> SourceIsaCharacteristicSourceCoordinateV1 {
        SourceIsaCharacteristicSourceCoordinateV1::new(
            id(byte),
            SourceIsaCharacteristicSourceSpanV1::new(
                id(byte + 1),
                u64::from(byte),
                u64::from(byte) + 4,
                u32::from(byte),
                1,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn rocprof_source() -> Vec<u8> {
        let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(ARTIFACT).unwrap();
        assert_eq!(inspected.inspection().target().to_string(), "gfx942:xnack-");
        let layout = inspected.load_layout().unwrap();
        let kernel = inspected.bindings()[0];
        let metadata_kernel = &inspected.inspection().kernels()[0];
        let mut document: serde_json::Value = serde_json::from_slice(ROCPROF).unwrap();
        let process = &mut document["rocprofiler-sdk-tool"][0];
        let deltas = [0x1000_0000_i64, 0x2000_0000_i64];
        process["code_objects"] = serde_json::Value::Array(
            [2_u64, 3]
                .into_iter()
                .zip(deltas)
                .map(|(code_object_id, delta)| {
                    serde_json::json!({
                        "code_object_id": code_object_id,
                        "agent_id": {"handle": 18217},
                        "uri": format!("file:///capture/{code_object_id}.hsaco"),
                        "load_base": layout.virtual_base() + u64::try_from(delta).unwrap(),
                        "load_size": layout.memory_size(),
                        "load_delta": delta,
                        "storage_type": 1,
                        "memory_base": 0,
                        "memory_size": 0
                    })
                })
                .collect(),
        );
        process["kernel_symbols"] = serde_json::Value::Array(
            [2_u64, 3]
                .into_iter()
                .zip(deltas)
                .enumerate()
                .map(|(index, (code_object_id, delta))| {
                    serde_json::json!({
                        "size": 80,
                        "kernel_id": 100 + index,
                        "code_object_id": code_object_id,
                        "kernel_name": "vecadd",
                        "kernel_object": kernel.descriptor_address() + u64::try_from(delta).unwrap(),
                        "kernarg_segment_size": kernel.descriptor().kernarg_size(),
                        "kernarg_segment_alignment": metadata_kernel.kernarg_segment_alignment(),
                        "group_segment_size": kernel.descriptor().group_segment_fixed_size(),
                        "private_segment_size": kernel.descriptor().private_segment_fixed_size(),
                        "formatted_kernel_name": "vecadd",
                        "demangled_kernel_name": "vecadd",
                        "truncated_kernel_name": "vecadd"
                    })
                })
                .collect(),
        );
        let relative_entry = kernel.entry_address() - layout.virtual_base();
        for (index, sample) in process["buffer_records"]["pc_sample_stochastic"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .take(4)
            .enumerate()
        {
            sample["record"]["pc"]["code_object_offset"] =
                serde_json::json!(relative_entry + u64::try_from(index % 2).unwrap() * 4);
        }
        serde_json::to_vec(&document).unwrap()
    }

    fn correlation(
        ordinal: u64,
        with_source: bool,
        intervals: Vec<SourceIsaCharacteristicIsaIntervalV1>,
        transformation: SourceIsaCharacteristicTransformationV1,
    ) -> SourceIsaCharacteristicTargetCorrelationV1 {
        let source = with_source.then(|| source_coordinate(20 + u8::try_from(ordinal).unwrap()));
        SourceIsaCharacteristicTargetCorrelationV1::new(
            ordinal,
            if with_source {
                fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicRecordKindV1::SourceAnchored
            } else {
                fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicRecordKindV1::NoSourceProvenance
            },
            source,
            with_source.then(|| id(40 + u8::try_from(ordinal).unwrap())),
            with_source.then(|| SourceIsaCharacteristicMirCoordinateV1::new(0, 1, ordinal).unwrap()),
            with_source.then(|| id(50 + u8::try_from(ordinal).unwrap())),
            with_source.then(|| SourceIsaCharacteristicKirCoordinateV1::new(0, 0, ordinal).unwrap()),
            SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap(),
            id(60 + u8::try_from(ordinal).unwrap()),
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, ordinal).unwrap(),
            intervals,
            transformation,
        )
        .unwrap()
    }

    fn characteristic(kind: CorrelationFixtureV1) -> Vec<u8> {
        let artifact_sha: [u8; 32] = Sha256::digest(ARTIFACT).into();
        characteristic_with(
            kind,
            SourceIsaCharacteristicTargetProfileV1::Gfx942,
            artifact_sha,
        )
    }

    fn characteristic_with(
        kind: CorrelationFixtureV1,
        target_profile: SourceIsaCharacteristicTargetProfileV1,
        artifact_sha: [u8; 32],
    ) -> Vec<u8> {
        let interval = SourceIsaCharacteristicIsaIntervalV1::new(0, 0, 4).unwrap();
        let mut correlations = match kind {
            CorrelationFixtureV1::UniqueSource | CorrelationFixtureV1::Partial => {
                vec![correlation(
                    0,
                    true,
                    vec![interval],
                    SourceIsaCharacteristicTransformationV1::Preserved,
                )]
            }
            CorrelationFixtureV1::UniqueNoSource => vec![correlation(
                0,
                false,
                vec![interval],
                SourceIsaCharacteristicTransformationV1::Preserved,
            )],
            CorrelationFixtureV1::Duplicated => vec![correlation(
                0,
                true,
                vec![interval, interval],
                SourceIsaCharacteristicTransformationV1::Duplicated,
            )],
            CorrelationFixtureV1::Ambiguous => vec![
                correlation(
                    0,
                    true,
                    vec![interval],
                    SourceIsaCharacteristicTransformationV1::Coalesced,
                ),
                correlation(
                    1,
                    false,
                    vec![interval],
                    SourceIsaCharacteristicTransformationV1::Coalesced,
                ),
            ],
            CorrelationFixtureV1::EliminatedOnly => vec![correlation(
                0,
                false,
                Vec::new(),
                SourceIsaCharacteristicTransformationV1::Eliminated,
            )],
        };
        correlations.sort_unstable_by_key(|value| value.catalog_record_ordinal());
        let pre = SourceIsaCharacteristicPreKirEliminationV1::new(
            u64::try_from(correlations.len()).unwrap(),
            source_coordinate(30),
            id(70),
            SourceIsaCharacteristicMirCoordinateV1::new(0, 3, 0).unwrap(),
        )
        .unwrap();
        let records = u64::try_from(correlations.len()).unwrap() + 1;
        let scan_state = if matches!(kind, CorrelationFixtureV1::Partial) {
            SourceIsaCharacteristicScanStateV1::Missing(
                SourceIsaCharacteristicMissingReasonV1::NoAdmittedCorrelation,
            )
        } else {
            SourceIsaCharacteristicScanStateV1::Complete
        };
        let collection = SourceIsaCharacteristicCollectionV1::new(
            SourceIsaCharacteristicBindingV1::new(
                target_profile,
                fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicKirVersionV1::V8,
                id(1),
                SourceIsaCharacteristicStructuralCountsV1 {
                    functions: 1,
                    defined_bodies: 1,
                    blocks: 1,
                    operations: 1
                        + u64::from(matches!(kind, CorrelationFixtureV1::Partial)),
                },
                content(2, 20),
                content(3, 30),
                content(4, 40),
                SourceIsaCharacteristicContentIdentityV1::new(
                    artifact_sha,
                    ARTIFACT.len() as u64,
                )
                .unwrap(),
                content(6, 60),
                content(7, 70),
                id(8),
                id(9),
            )
            .unwrap(),
            SourceIsaCharacteristicScanSummaryV1::new(
                records + u64::from(matches!(kind, CorrelationFixtureV1::Partial)),
                records,
                1 + u64::from(matches!(kind, CorrelationFixtureV1::Partial)),
                1,
                1,
                u64::try_from(correlations.len()).unwrap(),
                1,
                records,
                scan_state,
            )
            .unwrap(),
            vec![SourceIsaCharacteristicTargetV1::new(
                SourceIsaCharacteristicKindV1::GlobalStore {
                    form: SourceIsaCharacteristicMemoryFormV1::Plain,
                },
                SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap(),
                correlations,
            )
            .unwrap()],
            vec![pre],
        )
        .unwrap();
        collection.encode_canonical().unwrap()
    }

    fn evidence(kind: CorrelationFixtureV1) -> EvidenceV1 {
        let source = rocprof_source();
        let digest: [u8; 32] = Sha256::digest(ARTIFACT).into();
        let capture = import_rocprofv3_pc_sample_capture_v3(
            &source,
            RocprofPcSampleCaptureBindingV3 {
                capture: RocprofCaptureBindingV1 {
                    kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(
                        OpaqueIdentityV1::new(id(90)).unwrap(),
                        97,
                    )
                    .unwrap(),
                    artifact: Some(ArtifactClaimV1 {
                        identity: OpaqueIdentityV1::new(digest).unwrap(),
                        canonical_len: ARTIFACT.len() as u64,
                        format_version: 1,
                    }),
                    source_map: None,
                    wave_width: WaveWidthV1::Wave64,
                },
                sampling_interval_cycles: 1_048_576,
            },
            ImportLimitsV1::default(),
        )
        .unwrap();
        let samples = capture
            .samples
            .iter()
            .map(|sample| sample.identity)
            .collect();
        let capture = encode_pc_sample_capture_v3(&capture).unwrap();
        let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture,
            ARTIFACT,
            ImportLimitsV1::default(),
        )
        .unwrap();
        let relation = encode_pc_sample_code_object_relation_v1(&admitted, &capture).unwrap();
        EvidenceV1 {
            source,
            capture,
            relation,
            characteristic: characteristic(kind),
            samples,
        }
    }

    fn open(kind: CorrelationFixtureV1) -> (PcSourceIsaSessionV1, EvidenceV1) {
        let evidence = evidence(kind);
        let session = PcSourceIsaSessionV1::open(
            &evidence.source,
            &evidence.capture,
            ARTIFACT,
            &evidence.relation,
            &evidence.characteristic,
            PcSourceIsaLimitsV1::default(),
        )
        .unwrap();
        (session, evidence)
    }

    fn page(result: PcSourceIsaQueryResultV1) -> PcSourceIsaPageV1 {
        let PcSourceIsaQueryResultV1::AttributionPage { page } = result else {
            panic!("expected attribution page")
        };
        *page
    }

    #[test]
    fn exact_sample_projects_every_semantic_axis_without_addresses() {
        let (session, evidence) = open(CorrelationFixtureV1::UniqueSource);
        let page = page(
            session
                .lookup_sample(
                    evidence.samples[0],
                    PcSourceIsaPageRequestV1 {
                        limit: 8,
                        cursor: None,
                    },
                )
                .unwrap(),
        );
        assert_eq!(
            page.attribution_state,
            PcSourceIsaAttributionStateV1::UniqueSource
        );
        assert!(page.singular_attribution);
        assert_eq!(page.total_matching_interval_occurrences, 1);
        let item = &page.items[0];
        assert!(item.source.is_some());
        assert!(item.mir.is_some());
        assert!(item.neutral_kir.is_some());
        assert_eq!(item.target_kir.operation_ordinal, 1);
        assert_eq!(item.compiler_handoff_llvm.block_ordinal, 2);
        assert_eq!(item.isa.symbol_relative_start, 0);
        assert_eq!(item.category, "target_kir_global_store");
        assert_eq!(item.kind, "global_store_plain");
        assert_eq!(item.record_kind, "source_anchored");
        assert_eq!(item.transformation, "preserved");
        assert_eq!(
            page.binding.semantic_ir_isa_delta_comparison,
            PcSourceIsaComparisonStateV1::UnavailableRequiresTwoExactComparableBoundSessions
        );
        assert_eq!(page.resolved_pc.metadata_kernel_ordinal, 0);
        assert_ne!(page.resolved_pc.kernel_symbol_identity.as_bytes(), [0; 32]);
        let json = session
            .query_json(&PcSourceIsaQueryResultV1::AttributionPage {
                page: Box::new(page),
            })
            .unwrap();
        let text = std::str::from_utf8(&json).unwrap();
        assert!(!text.contains("load_base"));
        assert!(!text.contains("kernel_object"));
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(
            value["page"]["resolved_pc"]["claims"]["retains_native_addresses"],
            false
        );
    }

    #[test]
    fn duplication_overlap_no_source_elimination_loss_and_partial_states_are_explicit() {
        for (fixture, expected) in [
            (
                CorrelationFixtureV1::UniqueNoSource,
                PcSourceIsaAttributionStateV1::UniqueNoSourceProvenance,
            ),
            (
                CorrelationFixtureV1::Duplicated,
                PcSourceIsaAttributionStateV1::DuplicatedIntervalOccurrences,
            ),
            (
                CorrelationFixtureV1::Ambiguous,
                PcSourceIsaAttributionStateV1::AmbiguousOverlappingCorrelations,
            ),
            (
                CorrelationFixtureV1::EliminatedOnly,
                PcSourceIsaAttributionStateV1::NoMatchingIsaInterval,
            ),
        ] {
            let (session, evidence) = open(fixture);
            let page = page(
                session
                    .lookup_sample(
                        evidence.samples[0],
                        PcSourceIsaPageRequestV1 {
                            limit: 8,
                            cursor: None,
                        },
                    )
                    .unwrap(),
            );
            assert_eq!(page.attribution_state, expected);
            assert_eq!(
                page.singular_attribution,
                matches!(
                    expected,
                    PcSourceIsaAttributionStateV1::UniqueNoSourceProvenance
                )
            );
            assert_eq!(page.binding.loss, page.binding.capture_coverage.loss);
            assert_eq!(
                page.binding.loss.state,
                fe2o3_semantic_import::LossStateV1::Unknown
            );
            assert_eq!(
                page.binding.loss.unavailable_reason,
                Some(fe2o3_semantic_import::CaptureUnavailableReasonV1::CollectorLossUnknown)
            );
            assert_eq!(
                page.binding.moved_transformation_state,
                PcSourceIsaMovedStateV1::UnavailableNotRepresentedByCharacteristicV1
            );
            if matches!(fixture, CorrelationFixtureV1::EliminatedOnly) {
                assert_eq!(
                    page.binding
                        .characteristic_scan
                        .target_eliminated_correlations,
                    1
                );
                assert_eq!(page.binding.characteristic_scan.pre_kir_eliminations, 1);
            }
            if matches!(fixture, CorrelationFixtureV1::Duplicated) {
                assert!(
                    page.items
                        .iter()
                        .all(|item| item.transformation == "duplicated")
                );
            }
            if matches!(fixture, CorrelationFixtureV1::Ambiguous) {
                assert!(
                    page.items
                        .iter()
                        .all(|item| item.transformation == "coalesced")
                );
            }
        }
        let (session, evidence) = open(CorrelationFixtureV1::Partial);
        let page = page(
            session
                .lookup_sample(
                    evidence.samples[0],
                    PcSourceIsaPageRequestV1 {
                        limit: 8,
                        cursor: None,
                    },
                )
                .unwrap(),
        );
        assert_eq!(
            page.binding.characteristic_scan.availability,
            PcSourceIsaScanAvailabilityV1::Missing
        );
        assert_eq!(
            page.binding.characteristic_scan.reason,
            Some("no_admitted_correlation")
        );
        assert!(
            page.binding.characteristic_scan.catalog_records_scanned
                < page.binding.characteristic_scan.catalog_record_count
        );
    }

    #[test]
    fn interval_occurrences_page_and_cursors_bind_every_input_and_query() {
        let (session, evidence) = open(CorrelationFixtureV1::Duplicated);
        let first = page(
            session
                .lookup_sample(
                    evidence.samples[0],
                    PcSourceIsaPageRequestV1 {
                        limit: 1,
                        cursor: None,
                    },
                )
                .unwrap(),
        );
        assert_eq!(first.returned, 1);
        let cursor = first.next_cursor.unwrap();
        let second = page(
            session
                .lookup_sample(
                    evidence.samples[0],
                    PcSourceIsaPageRequestV1 {
                        limit: 1,
                        cursor: Some(cursor),
                    },
                )
                .unwrap(),
        );
        assert_eq!(second.returned, 1);
        assert!(second.next_cursor.is_none());
        assert_ne!(first.items[0].item_identity, second.items[0].item_identity);
        assert!(matches!(
            session.lookup_sample(
                evidence.samples[1],
                PcSourceIsaPageRequestV1 {
                    limit: 1,
                    cursor: Some(cursor)
                }
            ),
            Err(PcSourceIsaErrorV1::CursorMismatch)
        ));
        let mut hostile = cursor;
        hostile.binding_identity = identity(id(99));
        assert!(matches!(
            session.lookup_sample(
                evidence.samples[0],
                PcSourceIsaPageRequestV1 {
                    limit: 1,
                    cursor: Some(hostile)
                }
            ),
            Err(PcSourceIsaErrorV1::CursorMismatch)
        ));
    }

    #[test]
    fn artifact_target_and_characteristic_substitution_fail_closed() {
        let evidence = evidence(CorrelationFixtureV1::UniqueSource);
        let artifact_sha: [u8; 32] = Sha256::digest(ARTIFACT).into();
        let wrong_target = characteristic_with(
            CorrelationFixtureV1::UniqueSource,
            SourceIsaCharacteristicTargetProfileV1::Gfx950,
            artifact_sha,
        );
        assert!(matches!(
            PcSourceIsaSessionV1::open(
                &evidence.source,
                &evidence.capture,
                ARTIFACT,
                &evidence.relation,
                &wrong_target,
                PcSourceIsaLimitsV1::default()
            ),
            Err(PcSourceIsaErrorV1::TargetSubstitution)
        ));
        let wrong_artifact = characteristic_with(
            CorrelationFixtureV1::UniqueSource,
            SourceIsaCharacteristicTargetProfileV1::Gfx942,
            id(98),
        );
        assert!(matches!(
            PcSourceIsaSessionV1::open(
                &evidence.source,
                &evidence.capture,
                ARTIFACT,
                &evidence.relation,
                &wrong_artifact,
                PcSourceIsaLimitsV1::default()
            ),
            Err(PcSourceIsaErrorV1::ArtifactSubstitution)
        ));
        let mut corrupt = evidence.characteristic.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(matches!(
            PcSourceIsaSessionV1::open(
                &evidence.source,
                &evidence.capture,
                ARTIFACT,
                &evidence.relation,
                &corrupt,
                PcSourceIsaLimitsV1::default()
            ),
            Err(PcSourceIsaErrorV1::CharacteristicAdmission)
        ));
        let mut corrupt_relation = evidence.relation.clone();
        *corrupt_relation.last_mut().unwrap() ^= 1;
        assert!(matches!(
            PcSourceIsaSessionV1::open(
                &evidence.source,
                &evidence.capture,
                ARTIFACT,
                &corrupt_relation,
                &evidence.characteristic,
                PcSourceIsaLimitsV1::default()
            ),
            Err(PcSourceIsaErrorV1::RelationAdmission)
        ));
        let mut changed_capture = decode_pc_sample_capture_v3(&evidence.capture).unwrap();
        changed_capture.coverage.sampling.interval += 1;
        let changed_capture = encode_pc_sample_capture_v3(&changed_capture).unwrap();
        assert!(matches!(
            PcSourceIsaSessionV1::open(
                &evidence.source,
                &changed_capture,
                ARTIFACT,
                &evidence.relation,
                &evidence.characteristic,
                PcSourceIsaLimitsV1::default()
            ),
            Err(PcSourceIsaErrorV1::RelationAdmission)
        ));
        let small = PcSourceIsaLimitsV1 {
            max_source_bytes: 1,
            ..PcSourceIsaLimitsV1::default()
        };
        assert!(matches!(
            PcSourceIsaSessionV1::open(
                &evidence.source,
                &evidence.capture,
                ARTIFACT,
                &evidence.relation,
                &evidence.characteristic,
                small
            ),
            Err(PcSourceIsaErrorV1::InputTooLarge)
        ));
    }

    #[test]
    fn response_and_record_budgets_fail_closed() {
        let (session, evidence) = open(CorrelationFixtureV1::UniqueSource);
        assert!(matches!(
            session.lookup_sample(
                evidence.samples[0],
                PcSourceIsaPageRequestV1 {
                    limit: MAX_AGENT_PC_SOURCE_ISA_PAGE_ITEMS_V1 + 1,
                    cursor: None,
                },
            ),
            Err(PcSourceIsaErrorV1::PageLimit)
        ));
        let result = session
            .lookup_sample(
                evidence.samples[0],
                PcSourceIsaPageRequestV1 {
                    limit: 1,
                    cursor: None,
                },
            )
            .unwrap();
        assert!(matches!(
            encode_bounded(&result, 32),
            Err(PcSourceIsaErrorV1::ResponseTooLarge)
        ));
        let mut input = Vec::new();
        for _ in 0..=MAX_AGENT_PC_SOURCE_ISA_REQUESTS_V1 {
            input.extend_from_slice(b"{}\n");
        }
        let mut output = Vec::new();
        run_agent_pc_source_isa_jsonl_v1(&mut input.as_slice(), &mut output).unwrap();
        let lines: Vec<_> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(
            lines.len(),
            usize::try_from(MAX_AGENT_PC_SOURCE_ISA_REQUESTS_V1).unwrap() + 1
        );
        let terminal: serde_json::Value = serde_json::from_slice(lines.last().unwrap()).unwrap();
        assert_eq!(terminal["code"], "request_budget_exhausted");
        assert_eq!(terminal["terminal"], true);

        let unavailable = session
            .lookup_sample(
                identity(id(97)),
                PcSourceIsaPageRequestV1 {
                    limit: 1,
                    cursor: None,
                },
            )
            .unwrap();
        assert!(matches!(
            unavailable,
            PcSourceIsaQueryResultV1::PcUnavailable {
                reason: PcSampleCodeObjectQueryUnavailableReasonV1::UnknownSample,
                ..
            }
        ));
    }
}

//! Exact, read-only attribution of admitted decoded ATT PCs to a production
//! source/ISA characteristic archive.

use std::error::Error;
use std::fmt;
use std::io::{BufRead, Read, Write};

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, DecodedAttCompletenessV1,
    DecodedAttCoverageV1, DecodedAttDecoderIdentityV1, DecodedAttLossStateV1,
    DecodedAttPcAvailabilityV1, DecodedAttRawDecodeRelationV1, DecodedAttRecordOriginV1,
    MAX_DECODED_ATT_INTERCHANGE_BYTES_V1, SemanticDecodedAttV1, decode_decoded_att_v1,
    decoded_att_code_object_identity_v1, decoded_att_content_identity_v1,
};
use fe2o3_source_isa_observation::characteristic_v1::{
    InertSourceIsaCharacteristicCollectionV1, MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1,
    MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1, SourceIsaCharacteristicCollectionV1,
    source_isa_characteristic_target_correlation_match_identity_v1,
};
use serde::de::Visitor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    PcSourceIsaAttributionItemV1, PcSourceIsaAttributionStateV1, PcSourceIsaInputIdentityV1,
    PcSourceIsaScanSummaryV1, project_item, scan_summary, target_name,
};

pub const MAX_DECODED_ATT_SOURCE_ISA_ARTIFACT_BYTES_V1: u64 = 64 * 1024 * 1024;
pub const MAX_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1: u64 = 1024 * 1024;
pub const MAX_AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_ATTEMPTS_V1: u64 = 64;
pub const MAX_AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_BYTES_V1: u64 = 2
    * (MAX_DECODED_ATT_INTERCHANGE_BYTES_V1
        + MAX_DECODED_ATT_SOURCE_ISA_ARTIFACT_BYTES_V1
        + MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64)
    + 64 * 1024;
pub const MAX_AGENT_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1: u64 =
    MAX_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1 + 16 * 1024;

pub const AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1: &str =
    "fe2o3-agent-decoded-att-source-isa-request-v1";
pub const AGENT_DECODED_ATT_SOURCE_ISA_RESPONSE_SCHEMA_V1: &str =
    "fe2o3-agent-decoded-att-source-isa-response-v1";

const MIN_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1: u64 = 4 * 1024;
const INPUT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-source-isa.input.v1\0";
const BINDING_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-source-isa.binding.v1\0";
const SYMBOL_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-source-isa.symbol.v1\0";
const QUERY_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-source-isa.query.v1\0";
const ITEM_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-source-isa.item.v1\0";
const CURSOR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.decoded-att-source-isa.cursor.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DecodedAttSourceIsaLimitsV1 {
    pub max_interchange_bytes: u64,
    pub max_artifact_bytes: u64,
    pub max_characteristic_bytes: u64,
    pub max_page_items: u16,
    pub max_response_bytes: u64,
}

impl Default for DecodedAttSourceIsaLimitsV1 {
    fn default() -> Self {
        Self {
            max_interchange_bytes: MAX_DECODED_ATT_INTERCHANGE_BYTES_V1,
            max_artifact_bytes: MAX_DECODED_ATT_SOURCE_ISA_ARTIFACT_BYTES_V1,
            max_characteristic_bytes: MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64,
            max_page_items: MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1,
            max_response_bytes: MAX_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1,
        }
    }
}

impl DecodedAttSourceIsaLimitsV1 {
    pub fn validate(self) -> Result<Self, DecodedAttSourceIsaErrorV1> {
        if self.max_interchange_bytes == 0
            || self.max_interchange_bytes > MAX_DECODED_ATT_INTERCHANGE_BYTES_V1
            || self.max_artifact_bytes == 0
            || self.max_artifact_bytes > MAX_DECODED_ATT_SOURCE_ISA_ARTIFACT_BYTES_V1
            || self.max_characteristic_bytes == 0
            || self.max_characteristic_bytes
                > MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64
            || self.max_page_items == 0
            || self.max_page_items > MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1
            || !(MIN_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1
                ..=MAX_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1)
                .contains(&self.max_response_bytes)
        {
            return Err(DecodedAttSourceIsaErrorV1::LimitOutOfRange);
        }
        Ok(self)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttSourceIsaRelationKindV1 {
    ExactDecodedPcToBoundElfSymbolToCharacteristicInterval,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttSourceIsaBindingV1 {
    pub binding_identity: CaptureIdentityV1,
    pub interchange_input: PcSourceIsaInputIdentityV1,
    pub artifact_input: PcSourceIsaInputIdentityV1,
    pub characteristic_input: PcSourceIsaInputIdentityV1,
    pub interchange_identity: ContentIdentityRecordV1,
    pub decoded_export_source: ContentIdentityRecordV1,
    pub att_bundle: ContentIdentityRecordV1,
    pub att_manifest: ContentIdentityRecordV1,
    pub decoder: DecodedAttDecoderIdentityV1,
    pub code_object_identity: CaptureIdentityV1,
    pub artifact_identity: ContentIdentityRecordV1,
    pub characteristic_identity: CaptureIdentityV1,
    pub exact_artifact_target: String,
    pub characteristic_target: &'static str,
    pub decoded_coverage: DecodedAttCoverageV1,
    pub raw_decode_relation: DecodedAttRawDecodeRelationV1,
    pub characteristic_scan: PcSourceIsaScanSummaryV1,
    pub relation_kind: DecodedAttSourceIsaRelationKindV1,
    pub authority: &'static str,
    pub decoder_custody: &'static str,
    pub source_archive_authentication: &'static str,
    pub scope: &'static str,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttSourceIsaPcRecordKindV1 {
    Occupancy,
    Instruction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttSourceIsaResolvedPcV1 {
    pub record_identity: Option<CaptureIdentityV1>,
    pub record_kind: DecodedAttSourceIsaPcRecordKindV1,
    pub record_origin: DecodedAttRecordOriginV1,
    pub code_object_identity: CaptureIdentityV1,
    pub metadata_kernel_ordinal: u32,
    pub kernel_symbol_identity: CaptureIdentityV1,
    pub kernel_symbol_byte_len: u64,
    pub symbol_relative_pc: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttSourceIsaCursorV1 {
    pub binding_identity: CaptureIdentityV1,
    pub query_identity: CaptureIdentityV1,
    pub next_ordinal: u64,
    pub preceding_item_identity: CaptureIdentityV1,
    pub cursor_identity: CaptureIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttSourceIsaPageRequestV1 {
    pub limit: u16,
    pub cursor: Option<DecodedAttSourceIsaCursorV1>,
}

impl Default for DecodedAttSourceIsaPageRequestV1 {
    fn default() -> Self {
        Self {
            limit: MAX_SOURCE_ISA_CHARACTERISTIC_PAGE_ITEMS_V1,
            cursor: None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttSourceIsaAttributionItemV1 {
    pub relation_kind: DecodedAttSourceIsaRelationKindV1,
    pub decoded_completeness: DecodedAttCompletenessV1,
    pub decoded_loss: DecodedAttLossStateV1,
    pub raw_decode_relation: DecodedAttRawDecodeRelationV1,
    pub attribution: PcSourceIsaAttributionItemV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecodedAttSourceIsaPageV1 {
    pub binding: DecodedAttSourceIsaBindingV1,
    pub resolved_pc: DecodedAttSourceIsaResolvedPcV1,
    pub query_identity: CaptureIdentityV1,
    pub attribution_state: PcSourceIsaAttributionStateV1,
    pub singular_attribution: bool,
    pub total_matching_interval_occurrences: u64,
    pub returned: u16,
    pub next_cursor: Option<DecodedAttSourceIsaCursorV1>,
    pub items: Vec<DecodedAttSourceIsaAttributionItemV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecodedAttSourceIsaUnavailableReasonV1 {
    UnknownRecord,
    NativeVirtualAddressRedacted,
    DifferentCodeObject,
    UnalignedElfVirtualAddress,
    OutsideLoadedCodeObject,
    OutsideKernelSymbol,
    AmbiguousOverlappingKernelSymbols,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum DecodedAttSourceIsaQueryResultV1 {
    Binding {
        binding: DecodedAttSourceIsaBindingV1,
    },
    AttributionPage {
        page: Box<DecodedAttSourceIsaPageV1>,
    },
    PcUnavailable {
        binding: DecodedAttSourceIsaBindingV1,
        record_identity: Option<CaptureIdentityV1>,
        reason: DecodedAttSourceIsaUnavailableReasonV1,
    },
}

#[derive(Clone, Copy)]
struct ResolvedInputV1 {
    record_identity: Option<CaptureIdentityV1>,
    record_kind: DecodedAttSourceIsaPcRecordKindV1,
    pc: fe2o3_semantic_import::DecodedAttPcV1,
}

#[derive(Clone, Copy)]
struct BoundKernelSymbolV1 {
    metadata_kernel_ordinal: u32,
    entry_address: u64,
    entry_size: u64,
}

pub struct DecodedAttSourceIsaSessionV1 {
    decoded: SemanticDecodedAttV1,
    characteristic: SourceIsaCharacteristicCollectionV1,
    load_layout: fe2o3_hsaco::CodeObjectLoadLayout,
    symbols: Vec<BoundKernelSymbolV1>,
    binding: DecodedAttSourceIsaBindingV1,
    limits: DecodedAttSourceIsaLimitsV1,
}

impl DecodedAttSourceIsaSessionV1 {
    pub fn open(
        interchange: &[u8],
        code_object_identity: CaptureIdentityV1,
        artifact: &[u8],
        characteristic: &[u8],
        limits: DecodedAttSourceIsaLimitsV1,
    ) -> Result<Self, DecodedAttSourceIsaErrorV1> {
        let limits = limits.validate()?;
        check_size(interchange, limits.max_interchange_bytes)?;
        check_size(artifact, limits.max_artifact_bytes)?;
        check_size(characteristic, limits.max_characteristic_bytes)?;
        let inputs = [
            input_identity(interchange)?,
            input_identity(artifact)?,
            input_identity(characteristic)?,
        ];
        let decoded = decode_decoded_att_v1(interchange)
            .map_err(|_| DecodedAttSourceIsaErrorV1::InterchangeAdmission)?;
        let code_object = decoded
            .code_objects
            .binary_search_by_key(&code_object_identity, |value| value.identity)
            .ok()
            .and_then(|index| decoded.code_objects.get(index))
            .ok_or(DecodedAttSourceIsaErrorV1::UnknownCodeObject)?;
        let recomputed = decoded_att_code_object_identity_v1(
            code_object.selector,
            code_object.artifact,
            code_object.load_size,
        )
        .map_err(|_| DecodedAttSourceIsaErrorV1::CodeObjectIdentityMismatch)?;
        if recomputed != code_object_identity {
            return Err(DecodedAttSourceIsaErrorV1::CodeObjectIdentityMismatch);
        }
        let artifact_identity = exact_artifact_identity(artifact)?;
        if code_object.artifact != artifact_identity {
            return Err(DecodedAttSourceIsaErrorV1::ArtifactSubstitution);
        }
        let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(artifact)
            .map_err(|_| DecodedAttSourceIsaErrorV1::ArtifactAdmission)?;
        let layout = inspected
            .load_layout()
            .ok_or(DecodedAttSourceIsaErrorV1::ArtifactAdmission)?;
        if code_object.load_size != layout.memory_size() {
            return Err(DecodedAttSourceIsaErrorV1::ArtifactLoadSizeMismatch);
        }
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(characteristic)
            .map_err(|_| DecodedAttSourceIsaErrorV1::CharacteristicAdmission)?;
        let characteristic = inert.into_self_claimed_archive_for_agent_inspection_v1();
        let characteristic_artifact = characteristic.binding().artifact();
        if characteristic_artifact.sha256() != artifact_identity.digest.as_bytes()
            || characteristic_artifact.byte_len() != artifact_identity.canonical_len
        {
            return Err(DecodedAttSourceIsaErrorV1::CharacteristicArtifactSubstitution);
        }
        let exact_artifact_target = inspected.inspection().target().to_string();
        let characteristic_target = target_name(characteristic.binding().target_profile());
        if exact_artifact_target != characteristic_target {
            return Err(DecodedAttSourceIsaErrorV1::TargetSubstitution);
        }
        let mut symbols = Vec::new();
        symbols
            .try_reserve_exact(inspected.bindings().len())
            .map_err(|_| DecodedAttSourceIsaErrorV1::AllocationFailure)?;
        for symbol in inspected.bindings() {
            symbols.push(BoundKernelSymbolV1 {
                metadata_kernel_ordinal: u32::try_from(symbol.kernel_index())
                    .map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?,
                entry_address: symbol.entry_address(),
                entry_size: symbol.entry_size(),
            });
        }
        let interchange_identity = decoded_att_content_identity_v1(interchange)
            .map_err(|_| DecodedAttSourceIsaErrorV1::InterchangeAdmission)?;
        let binding_identity = binding_identity(
            &inputs,
            interchange_identity,
            code_object_identity,
            artifact_identity,
            characteristic.identity(),
            exact_artifact_target.as_bytes(),
        )?;
        let binding = DecodedAttSourceIsaBindingV1 {
            binding_identity,
            interchange_input: inputs[0].clone(),
            artifact_input: inputs[1].clone(),
            characteristic_input: inputs[2].clone(),
            interchange_identity,
            decoded_export_source: decoded.export_source,
            att_bundle: decoded.att_bundle,
            att_manifest: decoded.att_manifest,
            decoder: decoded.decoder,
            code_object_identity,
            artifact_identity,
            characteristic_identity: identity(characteristic.identity())?,
            exact_artifact_target,
            characteristic_target,
            decoded_coverage: decoded.coverage,
            raw_decode_relation: decoded.raw_decode_relation,
            characteristic_scan: scan_summary(&characteristic),
            relation_kind:
                DecodedAttSourceIsaRelationKindV1::ExactDecodedPcToBoundElfSymbolToCharacteristicInterval,
            authority: "read_only_no_decoder_collection_compiler_load_dispatch_attach_or_execution_authority",
            decoder_custody: "external_decoder_declared_not_authenticated",
            source_archive_authentication: "canonical_self_claimed_archive_exactly_bound_to_artifact_bytes_not_producer_authenticated",
            scope: "selected_code_object_and_decoded_att_pc_records_only_not_complete_execution_or_schedule",
        };
        Ok(Self {
            decoded,
            characteristic,
            load_layout: layout,
            symbols,
            binding,
            limits,
        })
    }

    pub const fn binding(&self) -> &DecodedAttSourceIsaBindingV1 {
        &self.binding
    }

    pub fn inspect_binding(&self) -> DecodedAttSourceIsaQueryResultV1 {
        DecodedAttSourceIsaQueryResultV1::Binding {
            binding: self.binding.clone(),
        }
    }

    pub fn lookup_record(
        &self,
        record_identity: CaptureIdentityV1,
        page: DecodedAttSourceIsaPageRequestV1,
    ) -> Result<DecodedAttSourceIsaQueryResultV1, DecodedAttSourceIsaErrorV1> {
        let Some(input) = self.find_record(record_identity) else {
            return Ok(self.unavailable(
                Some(record_identity),
                DecodedAttSourceIsaUnavailableReasonV1::UnknownRecord,
            ));
        };
        self.resolve_and_page(input, page)
    }

    pub fn query_json(
        &self,
        result: &DecodedAttSourceIsaQueryResultV1,
    ) -> Result<Vec<u8>, DecodedAttSourceIsaErrorV1> {
        encode_bounded(result, self.limits.max_response_bytes)
    }

    fn find_record(&self, identity: CaptureIdentityV1) -> Option<ResolvedInputV1> {
        if let Some(record) = self
            .decoded
            .occupancy
            .iter()
            .find(|value| value.identity == identity)
        {
            return Some(ResolvedInputV1 {
                record_identity: Some(identity),
                record_kind: DecodedAttSourceIsaPcRecordKindV1::Occupancy,
                pc: record.pc,
            });
        }
        self.decoded.waves.iter().find_map(|wave| {
            wave.instructions
                .iter()
                .find(|value| value.identity == identity)
                .map(|record| ResolvedInputV1 {
                    record_identity: Some(identity),
                    record_kind: DecodedAttSourceIsaPcRecordKindV1::Instruction,
                    pc: record.pc,
                })
        })
    }

    fn resolve_and_page(
        &self,
        input: ResolvedInputV1,
        page: DecodedAttSourceIsaPageRequestV1,
    ) -> Result<DecodedAttSourceIsaQueryResultV1, DecodedAttSourceIsaErrorV1> {
        let resolved = match self.resolve_pc(input) {
            Ok(value) => value,
            Err(reason) => return Ok(self.unavailable(input.record_identity, reason)),
        };
        self.page(resolved, page)
    }

    fn resolve_pc(
        &self,
        input: ResolvedInputV1,
    ) -> Result<DecodedAttSourceIsaResolvedPcV1, DecodedAttSourceIsaUnavailableReasonV1> {
        if input.pc.availability != DecodedAttPcAvailabilityV1::ElfVirtualAddress {
            return Err(DecodedAttSourceIsaUnavailableReasonV1::NativeVirtualAddressRedacted);
        }
        if input.pc.code_object != Some(self.binding.code_object_identity) {
            return Err(DecodedAttSourceIsaUnavailableReasonV1::DifferentCodeObject);
        }
        let address = input
            .pc
            .elf_virtual_address
            .ok_or(DecodedAttSourceIsaUnavailableReasonV1::NativeVirtualAddressRedacted)?;
        if !address.is_multiple_of(4) {
            return Err(DecodedAttSourceIsaUnavailableReasonV1::UnalignedElfVirtualAddress);
        }
        let load_end = self
            .load_layout
            .virtual_base()
            .checked_add(self.load_layout.memory_size())
            .ok_or(DecodedAttSourceIsaUnavailableReasonV1::OutsideLoadedCodeObject)?;
        if address < self.load_layout.virtual_base() || address >= load_end {
            return Err(DecodedAttSourceIsaUnavailableReasonV1::OutsideLoadedCodeObject);
        }
        let symbol = resolve_kernel_symbol(&self.symbols, address)?;
        let metadata_kernel_ordinal = symbol.metadata_kernel_ordinal;
        let symbol_relative_pc = address
            .checked_sub(symbol.entry_address)
            .ok_or(DecodedAttSourceIsaUnavailableReasonV1::OutsideKernelSymbol)?;
        Ok(DecodedAttSourceIsaResolvedPcV1 {
            record_identity: input.record_identity,
            record_kind: input.record_kind,
            record_origin: input.pc.origin,
            code_object_identity: self.binding.code_object_identity,
            metadata_kernel_ordinal,
            kernel_symbol_identity: symbol_identity(
                self.binding.binding_identity,
                self.binding.code_object_identity,
                metadata_kernel_ordinal,
                symbol.entry_address,
                symbol.entry_size,
            )?,
            kernel_symbol_byte_len: symbol.entry_size,
            symbol_relative_pc,
        })
    }

    fn unavailable(
        &self,
        record_identity: Option<CaptureIdentityV1>,
        reason: DecodedAttSourceIsaUnavailableReasonV1,
    ) -> DecodedAttSourceIsaQueryResultV1 {
        DecodedAttSourceIsaQueryResultV1::PcUnavailable {
            binding: self.binding.clone(),
            record_identity,
            reason,
        }
    }

    fn page(
        &self,
        resolved: DecodedAttSourceIsaResolvedPcV1,
        request: DecodedAttSourceIsaPageRequestV1,
    ) -> Result<DecodedAttSourceIsaQueryResultV1, DecodedAttSourceIsaErrorV1> {
        if request.limit == 0 || request.limit > self.limits.max_page_items {
            return Err(DecodedAttSourceIsaErrorV1::PageLimit);
        }
        let query_identity = query_identity(self.binding.binding_identity, resolved)?;
        let start = request.cursor.map_or(Ok(0), |cursor| {
            validate_cursor(self.binding.binding_identity, query_identity, cursor)
        })?;
        let mut total = 0_u64;
        let mut preceding = None;
        let mut items = Vec::new();
        items
            .try_reserve_exact(usize::from(request.limit))
            .map_err(|_| DecodedAttSourceIsaErrorV1::AllocationFailure)?;
        let mut first_correlation = None;
        let mut multiple_correlations = false;
        let mut any_without_source = false;
        for target in self.characteristic.targets() {
            for correlation in target.correlations() {
                let occurrence = identity(
                    source_isa_characteristic_target_correlation_match_identity_v1(
                        self.characteristic.identity(),
                        target.identity(),
                        correlation.catalog_record_ordinal(),
                    ),
                )?;
                for (interval_ordinal, interval) in correlation.isa_intervals().iter().enumerate() {
                    if !interval.contains(
                        u64::from(resolved.metadata_kernel_ordinal),
                        resolved.symbol_relative_pc,
                    ) {
                        continue;
                    }
                    let interval_ordinal = u64::try_from(interval_ordinal)
                        .map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?;
                    let item_identity = attribution_identity(
                        self.binding.binding_identity,
                        resolved.kernel_symbol_identity,
                        occurrence,
                        interval_ordinal,
                        interval.kernel_ordinal(),
                        interval.symbol_relative_start(),
                        interval.symbol_relative_end(),
                    )?;
                    multiple_correlations |=
                        first_correlation.is_some_and(|first| first != occurrence);
                    if first_correlation.is_none() {
                        first_correlation = Some(occurrence);
                    }
                    any_without_source |= correlation.source().is_none();
                    let next_total = total
                        .checked_add(1)
                        .ok_or(DecodedAttSourceIsaErrorV1::ResourceLimit)?;
                    if next_total <= start {
                        preceding = Some(item_identity);
                    } else if items.len() < usize::from(request.limit) {
                        items.push(DecodedAttSourceIsaAttributionItemV1 {
                            relation_kind: self.binding.relation_kind,
                            decoded_completeness: self.binding.decoded_coverage.completeness,
                            decoded_loss: self.binding.decoded_coverage.loss,
                            raw_decode_relation: self.binding.raw_decode_relation,
                            attribution: project_item(
                                target,
                                occurrence,
                                item_identity,
                                interval_ordinal,
                                correlation,
                                *interval,
                            ),
                        });
                    }
                    total = next_total;
                }
            }
        }
        if start > total
            || request.cursor.is_some() && (start == 0 || start == total)
            || request
                .cursor
                .is_some_and(|cursor| preceding != Some(cursor.preceding_item_identity))
        {
            return Err(DecodedAttSourceIsaErrorV1::CursorMismatch);
        }
        let returned =
            u16::try_from(items.len()).map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?;
        let next = start
            .checked_add(u64::from(returned))
            .ok_or(DecodedAttSourceIsaErrorV1::ResourceLimit)?;
        let next_cursor = if next < total {
            Some(make_cursor(
                self.binding.binding_identity,
                query_identity,
                next,
                items
                    .last()
                    .ok_or(DecodedAttSourceIsaErrorV1::CursorMismatch)?
                    .attribution
                    .item_identity,
            )?)
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
        Ok(DecodedAttSourceIsaQueryResultV1::AttributionPage {
            page: Box::new(DecodedAttSourceIsaPageV1 {
                binding: self.binding.clone(),
                resolved_pc: resolved,
                query_identity,
                attribution_state,
                singular_attribution: matches!(
                    attribution_state,
                    PcSourceIsaAttributionStateV1::UniqueSource
                        | PcSourceIsaAttributionStateV1::UniqueNoSourceProvenance
                ),
                total_matching_interval_occurrences: total,
                returned,
                next_cursor,
                items,
            }),
        })
    }
}

fn resolve_kernel_symbol(
    symbols: &[BoundKernelSymbolV1],
    address: u64,
) -> Result<BoundKernelSymbolV1, DecodedAttSourceIsaUnavailableReasonV1> {
    let mut matches = symbols.iter().copied().filter(|symbol| {
        symbol.entry_address <= address
            && symbol
                .entry_address
                .checked_add(symbol.entry_size)
                .is_some_and(|end| address < end)
    });
    let symbol = matches
        .next()
        .ok_or(DecodedAttSourceIsaUnavailableReasonV1::OutsideKernelSymbol)?;
    if matches.next().is_some() {
        return Err(DecodedAttSourceIsaUnavailableReasonV1::AmbiguousOverlappingKernelSymbols);
    }
    Ok(symbol)
}

fn check_size(bytes: &[u8], maximum: u64) -> Result<(), DecodedAttSourceIsaErrorV1> {
    let len = u64::try_from(bytes.len()).map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?;
    if len == 0 || len > maximum {
        return Err(DecodedAttSourceIsaErrorV1::InputTooLarge);
    }
    Ok(())
}

fn exact_artifact_identity(
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, DecodedAttSourceIsaErrorV1> {
    let canonical_len =
        u64::try_from(bytes.len()).map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?;
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::RawCanonicalSha256,
        format_version: 1,
        digest: identity(Sha256::digest(bytes).into())?,
        canonical_len,
    })
}

fn input_identity(bytes: &[u8]) -> Result<PcSourceIsaInputIdentityV1, DecodedAttSourceIsaErrorV1> {
    let byte_len =
        u64::try_from(bytes.len()).map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?;
    let mut digest = Sha256::new();
    digest.update(INPUT_IDENTITY_DOMAIN_V1);
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    Ok(PcSourceIsaInputIdentityV1 {
        scheme: "domain_separated_sha256",
        digest: identity(digest.finalize().into())?,
        byte_len,
    })
}

fn binding_identity(
    inputs: &[PcSourceIsaInputIdentityV1; 3],
    interchange: ContentIdentityRecordV1,
    code_object: CaptureIdentityV1,
    artifact: ContentIdentityRecordV1,
    characteristic: [u8; 32],
    target: &[u8],
) -> Result<CaptureIdentityV1, DecodedAttSourceIsaErrorV1> {
    let mut digest = Sha256::new();
    digest.update(BINDING_IDENTITY_DOMAIN_V1);
    for input in inputs {
        digest.update(input.digest.as_bytes());
        digest.update(input.byte_len.to_le_bytes());
    }
    digest.update(interchange.digest.as_bytes());
    digest.update(interchange.canonical_len.to_le_bytes());
    digest.update(code_object.as_bytes());
    digest.update(artifact.digest.as_bytes());
    digest.update(artifact.canonical_len.to_le_bytes());
    digest.update(characteristic);
    digest.update(
        u64::try_from(target.len())
            .map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?
            .to_le_bytes(),
    );
    digest.update(target);
    identity(digest.finalize().into())
}

fn symbol_identity(
    binding: CaptureIdentityV1,
    code_object: CaptureIdentityV1,
    kernel_ordinal: u32,
    entry_address: u64,
    entry_size: u64,
) -> Result<CaptureIdentityV1, DecodedAttSourceIsaUnavailableReasonV1> {
    let mut digest = Sha256::new();
    digest.update(SYMBOL_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update(code_object.as_bytes());
    digest.update(kernel_ordinal.to_le_bytes());
    digest.update(entry_address.to_le_bytes());
    digest.update(entry_size.to_le_bytes());
    CaptureIdentityV1::new(digest.finalize().into())
        .map_err(|_| DecodedAttSourceIsaUnavailableReasonV1::OutsideKernelSymbol)
}

fn query_identity(
    binding: CaptureIdentityV1,
    pc: DecodedAttSourceIsaResolvedPcV1,
) -> Result<CaptureIdentityV1, DecodedAttSourceIsaErrorV1> {
    let mut digest = Sha256::new();
    digest.update(QUERY_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update([record_kind_tag(pc.record_kind)]);
    match pc.record_identity {
        Some(identity) => {
            digest.update([1]);
            digest.update(identity.as_bytes());
        }
        None => digest.update([0]),
    }
    digest.update(pc.code_object_identity.as_bytes());
    digest.update(pc.metadata_kernel_ordinal.to_le_bytes());
    digest.update(pc.kernel_symbol_identity.as_bytes());
    digest.update(pc.kernel_symbol_byte_len.to_le_bytes());
    digest.update(pc.symbol_relative_pc.to_le_bytes());
    identity(digest.finalize().into())
}

const fn record_kind_tag(kind: DecodedAttSourceIsaPcRecordKindV1) -> u8 {
    match kind {
        DecodedAttSourceIsaPcRecordKindV1::Occupancy => 0,
        DecodedAttSourceIsaPcRecordKindV1::Instruction => 1,
    }
}

fn attribution_identity(
    binding: CaptureIdentityV1,
    symbol: CaptureIdentityV1,
    correlation: CaptureIdentityV1,
    ordinal: u64,
    kernel_ordinal: u64,
    interval_start: u64,
    interval_end: u64,
) -> Result<CaptureIdentityV1, DecodedAttSourceIsaErrorV1> {
    let mut digest = Sha256::new();
    digest.update(ITEM_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update(symbol.as_bytes());
    digest.update(correlation.as_bytes());
    digest.update(ordinal.to_le_bytes());
    digest.update(kernel_ordinal.to_le_bytes());
    digest.update(interval_start.to_le_bytes());
    digest.update(interval_end.to_le_bytes());
    identity(digest.finalize().into())
}

fn make_cursor(
    binding: CaptureIdentityV1,
    query: CaptureIdentityV1,
    next: u64,
    preceding: CaptureIdentityV1,
) -> Result<DecodedAttSourceIsaCursorV1, DecodedAttSourceIsaErrorV1> {
    let mut digest = Sha256::new();
    digest.update(CURSOR_IDENTITY_DOMAIN_V1);
    digest.update(binding.as_bytes());
    digest.update(query.as_bytes());
    digest.update(next.to_le_bytes());
    digest.update(preceding.as_bytes());
    Ok(DecodedAttSourceIsaCursorV1 {
        binding_identity: binding,
        query_identity: query,
        next_ordinal: next,
        preceding_item_identity: preceding,
        cursor_identity: identity(digest.finalize().into())?,
    })
}

fn validate_cursor(
    binding: CaptureIdentityV1,
    query: CaptureIdentityV1,
    cursor: DecodedAttSourceIsaCursorV1,
) -> Result<u64, DecodedAttSourceIsaErrorV1> {
    if cursor.next_ordinal == 0
        || cursor.binding_identity != binding
        || cursor.query_identity != query
        || cursor
            != make_cursor(
                binding,
                query,
                cursor.next_ordinal,
                cursor.preceding_item_identity,
            )?
    {
        return Err(DecodedAttSourceIsaErrorV1::CursorMismatch);
    }
    Ok(cursor.next_ordinal)
}

fn identity(bytes: [u8; 32]) -> Result<CaptureIdentityV1, DecodedAttSourceIsaErrorV1> {
    CaptureIdentityV1::new(bytes).map_err(|_| DecodedAttSourceIsaErrorV1::Identity)
}

fn encode_bounded(
    value: &impl Serialize,
    maximum: u64,
) -> Result<Vec<u8>, DecodedAttSourceIsaErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(
            usize::try_from(maximum.min(64 * 1024))
                .map_err(|_| DecodedAttSourceIsaErrorV1::ResourceLimit)?,
        )
        .map_err(|_| DecodedAttSourceIsaErrorV1::AllocationFailure)?;
    let mut writer = BoundedWriterV1 {
        output: &mut output,
        maximum,
        too_large: false,
        allocation_failed: false,
    };
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        if writer.too_large {
            DecodedAttSourceIsaErrorV1::ResponseTooLarge
        } else if writer.allocation_failed {
            DecodedAttSourceIsaErrorV1::AllocationFailure
        } else {
            DecodedAttSourceIsaErrorV1::Json
        }
    })?;
    Ok(output)
}

struct BoundedWriterV1<'a> {
    output: &'a mut Vec<u8>,
    maximum: u64,
    too_large: bool,
    allocation_failed: bool,
}

impl Write for BoundedWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = u64::try_from(self.output.len()).ok().and_then(|value| {
            u64::try_from(bytes.len())
                .ok()
                .and_then(|bytes| value.checked_add(bytes))
        });
        if next.is_none_or(|value| value > self.maximum) {
            self.too_large = true;
            return Err(std::io::Error::other(
                "decoded ATT source/ISA response too large",
            ));
        }
        if self.output.try_reserve_exact(bytes.len()).is_err() {
            self.allocation_failed = true;
            return Err(std::io::Error::other(
                "decoded ATT source/ISA response allocation failed",
            ));
        }
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum DecodedAttSourceIsaErrorV1 {
    LimitOutOfRange,
    InputTooLarge,
    InterchangeAdmission,
    UnknownCodeObject,
    CodeObjectIdentityMismatch,
    ArtifactSubstitution,
    ArtifactAdmission,
    ArtifactLoadSizeMismatch,
    CharacteristicAdmission,
    CharacteristicArtifactSubstitution,
    TargetSubstitution,
    PageLimit,
    CursorMismatch,
    ResponseTooLarge,
    ResourceLimit,
    AllocationFailure,
    Identity,
    Json,
}

impl fmt::Display for DecodedAttSourceIsaErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "decoded ATT source/ISA query rejected: {self:?}")
    }
}

impl Error for DecodedAttSourceIsaErrorV1 {}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum AgentDecodedAttSourceIsaRequestV1 {
    Open {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        code_object_identity: CaptureIdentityV1,
        #[serde(deserialize_with = "deserialize_interchange_hex")]
        interchange_hex: String,
        #[serde(deserialize_with = "deserialize_artifact_hex")]
        artifact_hex: String,
        #[serde(deserialize_with = "deserialize_characteristic_hex")]
        characteristic_hex: String,
    },
    Binding {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
    LookupRecord {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
        record_identity: CaptureIdentityV1,
        page: DecodedAttSourceIsaPageRequestV1,
    },
    Close {
        #[serde(deserialize_with = "deserialize_agent_schema")]
        schema: String,
        request_id: u64,
        revision: u64,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "operation", content = "value", rename_all = "snake_case")]
pub enum AgentDecodedAttSourceIsaResultV1 {
    Open(DecodedAttSourceIsaQueryResultV1),
    Binding(DecodedAttSourceIsaQueryResultV1),
    LookupRecord(DecodedAttSourceIsaQueryResultV1),
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDecodedAttSourceIsaErrorCodeV1 {
    InvalidRequest,
    InvalidSchema,
    InvalidRequestId,
    DuplicateRequestId,
    RevisionMismatch,
    RevisionExhausted,
    RequestAttemptLimit,
    RequestTooLarge,
    EvidenceRejected,
    SessionNotOpen,
    SessionAlreadyOpen,
    QueryRejected,
    ResponseTooLarge,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AgentDecodedAttSourceIsaResponseV1 {
    Ok {
        schema: &'static str,
        request_id: u64,
        revision: u64,
        terminal: bool,
        result: Box<AgentDecodedAttSourceIsaResultV1>,
    },
    Error {
        schema: &'static str,
        request_id: Option<u64>,
        revision: u64,
        terminal: bool,
        code: AgentDecodedAttSourceIsaErrorCodeV1,
    },
}

struct AgentDecodedAttSourceIsaServiceV1 {
    revision: u64,
    request_attempts: u64,
    request_ids: Vec<u64>,
    session: Option<DecodedAttSourceIsaSessionV1>,
    terminal: bool,
}

impl AgentDecodedAttSourceIsaServiceV1 {
    fn new() -> Self {
        Self {
            revision: 0,
            request_attempts: 0,
            request_ids: Vec::new(),
            session: None,
            terminal: false,
        }
    }

    fn begin_attempt(&mut self) -> Result<(), AgentDecodedAttSourceIsaResponseV1> {
        if self.request_attempts >= MAX_AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_ATTEMPTS_V1 {
            return Err(self.error(
                None,
                AgentDecodedAttSourceIsaErrorCodeV1::RequestAttemptLimit,
                true,
            ));
        }
        self.request_attempts = self.request_attempts.checked_add(1).ok_or_else(|| {
            self.error(
                None,
                AgentDecodedAttSourceIsaErrorCodeV1::RequestAttemptLimit,
                true,
            )
        })?;
        Ok(())
    }

    fn handle(
        &mut self,
        request: AgentDecodedAttSourceIsaRequestV1,
    ) -> AgentDecodedAttSourceIsaResponseV1 {
        let (schema, request_id, revision) = agent_request_header(&request);
        if request_id == 0 {
            return self.error(
                Some(request_id),
                AgentDecodedAttSourceIsaErrorCodeV1::InvalidRequestId,
                false,
            );
        }
        if self.request_ids.contains(&request_id) {
            return self.error(
                Some(request_id),
                AgentDecodedAttSourceIsaErrorCodeV1::DuplicateRequestId,
                false,
            );
        }
        if self.request_ids.try_reserve_exact(1).is_err() {
            return self.error(
                Some(request_id),
                AgentDecodedAttSourceIsaErrorCodeV1::ResponseTooLarge,
                true,
            );
        }
        self.request_ids.push(request_id);
        if schema != AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1 {
            return self.error(
                Some(request_id),
                AgentDecodedAttSourceIsaErrorCodeV1::InvalidSchema,
                false,
            );
        }
        if revision != self.revision {
            return self.error(
                Some(request_id),
                AgentDecodedAttSourceIsaErrorCodeV1::RevisionMismatch,
                false,
            );
        }
        match request {
            AgentDecodedAttSourceIsaRequestV1::Open {
                code_object_identity,
                interchange_hex,
                artifact_hex,
                characteristic_hex,
                ..
            } => {
                if self.session.is_some() {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttSourceIsaErrorCodeV1::SessionAlreadyOpen,
                        false,
                    );
                }
                let inputs = match (
                    decode_lower_hex(&interchange_hex, MAX_DECODED_ATT_INTERCHANGE_BYTES_V1),
                    decode_lower_hex(&artifact_hex, MAX_DECODED_ATT_SOURCE_ISA_ARTIFACT_BYTES_V1),
                    decode_lower_hex(
                        &characteristic_hex,
                        MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1 as u64,
                    ),
                ) {
                    (Ok(interchange), Ok(artifact), Ok(characteristic)) => {
                        (interchange, artifact, characteristic)
                    }
                    _ => {
                        return self.error(
                            Some(request_id),
                            AgentDecodedAttSourceIsaErrorCodeV1::EvidenceRejected,
                            false,
                        );
                    }
                };
                let session = match DecodedAttSourceIsaSessionV1::open(
                    &inputs.0,
                    code_object_identity,
                    &inputs.1,
                    &inputs.2,
                    DecodedAttSourceIsaLimitsV1::default(),
                ) {
                    Ok(session) => session,
                    Err(_) => {
                        return self.error(
                            Some(request_id),
                            AgentDecodedAttSourceIsaErrorCodeV1::EvidenceRejected,
                            false,
                        );
                    }
                };
                let result = session.inspect_binding();
                if session.query_json(&result).is_err() {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttSourceIsaErrorCodeV1::ResponseTooLarge,
                        true,
                    );
                }
                self.session = Some(session);
                self.success(
                    request_id,
                    false,
                    AgentDecodedAttSourceIsaResultV1::Open(result),
                )
            }
            AgentDecodedAttSourceIsaRequestV1::Binding { .. } => {
                let Some(session) = &self.session else {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttSourceIsaErrorCodeV1::SessionNotOpen,
                        false,
                    );
                };
                let result = session.inspect_binding();
                self.success(
                    request_id,
                    false,
                    AgentDecodedAttSourceIsaResultV1::Binding(result),
                )
            }
            AgentDecodedAttSourceIsaRequestV1::LookupRecord {
                record_identity,
                page,
                ..
            } => {
                let result = {
                    let Some(session) = &self.session else {
                        return self.error(
                            Some(request_id),
                            AgentDecodedAttSourceIsaErrorCodeV1::SessionNotOpen,
                            false,
                        );
                    };
                    match session.lookup_record(record_identity, page) {
                        Ok(result) if session.query_json(&result).is_ok() => result,
                        Ok(_) | Err(DecodedAttSourceIsaErrorV1::ResponseTooLarge) => {
                            return self.error(
                                Some(request_id),
                                AgentDecodedAttSourceIsaErrorCodeV1::ResponseTooLarge,
                                true,
                            );
                        }
                        Err(_) => {
                            return self.error(
                                Some(request_id),
                                AgentDecodedAttSourceIsaErrorCodeV1::QueryRejected,
                                false,
                            );
                        }
                    }
                };
                self.success(
                    request_id,
                    false,
                    AgentDecodedAttSourceIsaResultV1::LookupRecord(result),
                )
            }
            AgentDecodedAttSourceIsaRequestV1::Close { .. } => {
                if self.session.is_none() {
                    return self.error(
                        Some(request_id),
                        AgentDecodedAttSourceIsaErrorCodeV1::SessionNotOpen,
                        false,
                    );
                }
                self.session = None;
                self.success(request_id, true, AgentDecodedAttSourceIsaResultV1::Closed)
            }
        }
    }

    fn success(
        &mut self,
        request_id: u64,
        terminal: bool,
        result: AgentDecodedAttSourceIsaResultV1,
    ) -> AgentDecodedAttSourceIsaResponseV1 {
        let Some(revision) = self.revision.checked_add(1) else {
            return self.error(
                Some(request_id),
                AgentDecodedAttSourceIsaErrorCodeV1::RevisionExhausted,
                true,
            );
        };
        self.revision = revision;
        self.terminal = terminal;
        AgentDecodedAttSourceIsaResponseV1::Ok {
            schema: AGENT_DECODED_ATT_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            revision,
            terminal,
            result: Box::new(result),
        }
    }

    fn error(
        &mut self,
        request_id: Option<u64>,
        code: AgentDecodedAttSourceIsaErrorCodeV1,
        terminal: bool,
    ) -> AgentDecodedAttSourceIsaResponseV1 {
        self.terminal |= terminal;
        AgentDecodedAttSourceIsaResponseV1::Error {
            schema: AGENT_DECODED_ATT_SOURCE_ISA_RESPONSE_SCHEMA_V1,
            request_id,
            revision: self.revision,
            terminal,
            code,
        }
    }
}

fn agent_request_header(request: &AgentDecodedAttSourceIsaRequestV1) -> (&str, u64, u64) {
    match request {
        AgentDecodedAttSourceIsaRequestV1::Open {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentDecodedAttSourceIsaRequestV1::Binding {
            schema,
            request_id,
            revision,
        }
        | AgentDecodedAttSourceIsaRequestV1::LookupRecord {
            schema,
            request_id,
            revision,
            ..
        }
        | AgentDecodedAttSourceIsaRequestV1::Close {
            schema,
            request_id,
            revision,
        } => (schema, *request_id, *revision),
    }
}

pub fn run_agent_decoded_att_source_isa_jsonl_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
) -> Result<(), AgentDecodedAttSourceIsaServiceErrorV1> {
    run_agent_decoded_att_source_isa_jsonl_with_limit_v1(
        input,
        output,
        MAX_AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_BYTES_V1,
    )
}

fn run_agent_decoded_att_source_isa_jsonl_with_limit_v1<R: BufRead, W: Write>(
    input: &mut R,
    output: &mut W,
    request_bytes: u64,
) -> Result<(), AgentDecodedAttSourceIsaServiceErrorV1> {
    let mut service = AgentDecodedAttSourceIsaServiceV1::new();
    let mut line = Vec::new();
    loop {
        line.clear();
        let read_limit = request_bytes.saturating_add(2);
        let mut bounded = Read::take(&mut *input, read_limit);
        let read = bounded
            .read_until(b'\n', &mut line)
            .map_err(|_| AgentDecodedAttSourceIsaServiceErrorV1::Io)?;
        if read == 0 {
            return Ok(());
        }
        if let Err(response) = service.begin_attempt() {
            write_agent_response(output, &response)?;
            return Ok(());
        }
        if line.last() != Some(&b'\n')
            || u64::try_from(line.len()).unwrap_or(u64::MAX) > request_bytes.saturating_add(1)
        {
            let response = service.error(
                None,
                AgentDecodedAttSourceIsaErrorCodeV1::RequestTooLarge,
                true,
            );
            write_agent_response(output, &response)?;
            return Ok(());
        }
        line.pop();
        let request: AgentDecodedAttSourceIsaRequestV1 = match serde_json::from_slice(&line) {
            Ok(request)
                if encode_agent_json(
                    &request,
                    MAX_AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_BYTES_V1,
                )
                .ok()
                .as_deref()
                    == Some(line.as_slice()) =>
            {
                request
            }
            _ => {
                let response = service.error(
                    None,
                    AgentDecodedAttSourceIsaErrorCodeV1::InvalidRequest,
                    false,
                );
                write_agent_response(output, &response)?;
                continue;
            }
        };
        let response = service.handle(request);
        if write_agent_response_or_terminal(output, &mut service, &response)? {
            return Ok(());
        }
    }
}

fn write_agent_response_or_terminal(
    output: &mut impl Write,
    service: &mut AgentDecodedAttSourceIsaServiceV1,
    response: &AgentDecodedAttSourceIsaResponseV1,
) -> Result<bool, AgentDecodedAttSourceIsaServiceErrorV1> {
    match write_agent_response(output, response) {
        Ok(()) => Ok(service.terminal),
        Err(AgentDecodedAttSourceIsaServiceErrorV1::ResponseTooLarge)
        | Err(AgentDecodedAttSourceIsaServiceErrorV1::AllocationFailure) => {
            let terminal = service.error(
                None,
                AgentDecodedAttSourceIsaErrorCodeV1::ResponseTooLarge,
                true,
            );
            write_agent_response(output, &terminal)?;
            Ok(true)
        }
        Err(error) => Err(error),
    }
}

fn write_agent_response(
    output: &mut impl Write,
    response: &AgentDecodedAttSourceIsaResponseV1,
) -> Result<(), AgentDecodedAttSourceIsaServiceErrorV1> {
    let bytes = encode_agent_json(
        response,
        MAX_AGENT_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1 - 1,
    )?;
    output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .and_then(|()| output.flush())
        .map_err(|_| AgentDecodedAttSourceIsaServiceErrorV1::Io)
}

fn encode_agent_json(
    value: &impl Serialize,
    maximum: u64,
) -> Result<Vec<u8>, AgentDecodedAttSourceIsaServiceErrorV1> {
    encode_bounded(value, maximum).map_err(|error| match error {
        DecodedAttSourceIsaErrorV1::ResponseTooLarge => {
            AgentDecodedAttSourceIsaServiceErrorV1::ResponseTooLarge
        }
        DecodedAttSourceIsaErrorV1::AllocationFailure => {
            AgentDecodedAttSourceIsaServiceErrorV1::AllocationFailure
        }
        _ => AgentDecodedAttSourceIsaServiceErrorV1::Json,
    })
}

fn decode_lower_hex(value: &str, maximum_bytes: u64) -> Result<Vec<u8>, ()> {
    let maximum_hex = maximum_bytes.checked_mul(2).ok_or(())?;
    if !value.len().is_multiple_of(2) || u64::try_from(value.len()).map_err(|_| ())? > maximum_hex {
        return Err(());
    }
    let mut output = Vec::new();
    output.try_reserve_exact(value.len() / 2).map_err(|_| ())?;
    for pair in value.as_bytes().chunks_exact(2) {
        let high = lower_hex_nibble(pair[0]).ok_or(())?;
        let low = lower_hex_nibble(pair[1]).ok_or(())?;
        output.push((high << 4) | low);
    }
    Ok(output)
}

fn lower_hex_nibble(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn deserialize_agent_schema<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(deserializer, 128, "agent schema")
}

fn deserialize_interchange_hex<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(
        deserializer,
        usize::try_from(MAX_DECODED_ATT_INTERCHANGE_BYTES_V1 * 2).unwrap_or(usize::MAX),
        "decoded ATT interchange hex",
    )
}

fn deserialize_artifact_hex<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(
        deserializer,
        usize::try_from(MAX_DECODED_ATT_SOURCE_ISA_ARTIFACT_BYTES_V1 * 2).unwrap_or(usize::MAX),
        "HSACO hex",
    )
}

fn deserialize_characteristic_hex<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_string(
        deserializer,
        MAX_SOURCE_ISA_CHARACTERISTIC_COLLECTION_BYTES_V1.saturating_mul(2),
        "Characteristic V1 hex",
    )
}

fn deserialize_bounded_string<'de, D>(
    deserializer: D,
    maximum: usize,
    label: &'static str,
) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct BoundedStringVisitorV1 {
        maximum: usize,
        label: &'static str,
    }
    impl Visitor<'_> for BoundedStringVisitorV1 {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "an ASCII {} no longer than {} bytes",
                self.label, self.maximum
            )
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if value.len() > self.maximum || !value.is_ascii() {
                return Err(E::custom("bounded decoded ATT source/ISA string rejected"));
            }
            let mut output = String::new();
            output.try_reserve_exact(value.len()).map_err(|_| {
                E::custom("bounded decoded ATT source/ISA string allocation failed")
            })?;
            output.push_str(value);
            Ok(output)
        }
    }
    deserializer.deserialize_str(BoundedStringVisitorV1 { maximum, label })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentDecodedAttSourceIsaServiceErrorV1 {
    Io,
    Json,
    ResponseTooLarge,
    AllocationFailure,
}

impl fmt::Display for AgentDecodedAttSourceIsaServiceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "decoded ATT source/ISA agent service failed: {self:?}"
        )
    }
}

impl Error for AgentDecodedAttSourceIsaServiceErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_import::{
        ContentSchemeV1, DecodedAttAuthenticityV1, DecodedAttImportBindingV1,
        DecodedAttImportLimitsV1, ProfilerAttArtifactBindingV4, ProfilerAttBindingV4,
        ProfilerDeviceBindingV4, ProfilerEnvironmentBindingV4,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1,
        ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1, encode_decoded_att_v1,
        encode_profiler_bundle_v4, import_rocprofiler_sdk_decoded_att_v1,
        import_rocprofv3_att_profiler_bundle_v4,
    };
    use fe2o3_source_isa_observation::characteristic_v1::{
        SourceIsaCharacteristicBindingV1, SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1,
        SourceIsaCharacteristicContentIdentityV1, SourceIsaCharacteristicIsaIntervalV1,
        SourceIsaCharacteristicKindV1, SourceIsaCharacteristicKirCoordinateV1,
        SourceIsaCharacteristicKirVersionV1, SourceIsaCharacteristicMemoryFormV1,
        SourceIsaCharacteristicMirCoordinateV1, SourceIsaCharacteristicMissingReasonV1,
        SourceIsaCharacteristicPreKirEliminationV1, SourceIsaCharacteristicRecordKindV1,
        SourceIsaCharacteristicScanStateV1, SourceIsaCharacteristicScanSummaryV1,
        SourceIsaCharacteristicSourceCoordinateV1, SourceIsaCharacteristicSourceSpanV1,
        SourceIsaCharacteristicStructuralCountsV1, SourceIsaCharacteristicTargetCorrelationV1,
        SourceIsaCharacteristicTargetProfileV1, SourceIsaCharacteristicTargetV1,
        SourceIsaCharacteristicTransformationV1,
    };

    const ARTIFACT: &[u8] =
        include_bytes!("../../fe2o3-runtime/fixtures/trusted-gfx942-vecadd-v1/vecadd.hsaco");
    const EXPORT: &[u8] = include_bytes!(
        "../../fe2o3-semantic-import/tests/fixtures/rocprofiler-sdk-7.2.4-decoded-att-v1.json"
    );
    const MANIFEST: &[u8] = br#"{"counter_names":[],"gfxip":9,"gfxv":"vega","global_begin_time":0,"is_pcs_stochastic":false,"pc_sampling":false,"thread_trace":true,"version":"3.0.0","wave_filenames":{"0":{"0":{"0":{"0":["waves/se0.json",10,20]}}}},"se_filenames":["se0.json"]}"#;

    #[derive(Clone, Copy)]
    enum CorrelationFixtureV1 {
        Unique,
        Duplicated,
        Partial,
        NoSource,
    }

    struct EvidenceV1 {
        decoded: Vec<u8>,
        code_object: CaptureIdentityV1,
        instruction: CaptureIdentityV1,
        native: CaptureIdentityV1,
        characteristic: Vec<u8>,
    }

    fn fixture_content(byte: u8, len: u64, scheme: ContentSchemeV1) -> ContentIdentityRecordV1 {
        ContentIdentityRecordV1 {
            scheme,
            format_version: 1,
            digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
            canonical_len: len,
        }
    }

    fn decoder_binding() -> DecodedAttImportBindingV1 {
        DecodedAttImportBindingV1 {
            trace_decoder_types_header: ContentIdentityRecordV1 {
                scheme: ContentSchemeV1::RawCanonicalSha256,
                format_version: 1,
                digest: CaptureIdentityV1::new(
                    ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_SHA256_V1,
                )
                .unwrap(),
                canonical_len: ROCPROFILER_SDK_7_2_4_TRACE_DECODER_TYPES_HEADER_BYTES_V1,
            },
            trace_decoder_api_header: ContentIdentityRecordV1 {
                scheme: ContentSchemeV1::RawCanonicalSha256,
                format_version: 1,
                digest: CaptureIdentityV1::new(
                    ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_SHA256_V1,
                )
                .unwrap(),
                canonical_len: ROCPROFILER_SDK_7_2_4_TRACE_DECODER_API_HEADER_BYTES_V1,
            },
            decoder_library: fixture_content(50, 50_000, ContentSchemeV1::RawCanonicalSha256),
            exporter_tool: fixture_content(51, 25_000, ContentSchemeV1::RawCanonicalSha256),
        }
    }

    fn bundle() -> Vec<u8> {
        let bundle = import_rocprofv3_att_profiler_bundle_v4(
            MANIFEST,
            ProfilerAttBindingV4 {
                environment: ProfilerEnvironmentBindingV4 {
                    environment: fixture_content(10, 200, ContentSchemeV1::DomainSeparatedSha256),
                    collector_tool: fixture_content(11, 50, ContentSchemeV1::DomainSeparatedSha256),
                    collector_configuration: fixture_content(
                        12,
                        80,
                        ContentSchemeV1::DomainSeparatedSha256,
                    ),
                    stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                        source_agent_id: 17,
                        stable_identity: fixture_content(
                            20,
                            64,
                            ContentSchemeV1::DomainSeparatedSha256,
                        ),
                    }],
                },
                source_agent_id: 17,
                referenced_artifacts: vec![
                    ProfilerAttArtifactBindingV4 {
                        reference: "waves/se0.json".to_owned(),
                        content: fixture_content(31, 401, ContentSchemeV1::DomainSeparatedSha256),
                    },
                    ProfilerAttArtifactBindingV4 {
                        reference: "se0.json".to_owned(),
                        content: fixture_content(32, 402, ContentSchemeV1::DomainSeparatedSha256),
                    },
                ],
            },
        )
        .unwrap();
        encode_profiler_bundle_v4(&bundle).unwrap()
    }

    fn decoded_for_artifact_with_load_and_pc(
        artifact: &[u8],
        load_size: u64,
        pc_override: Option<u64>,
    ) -> (Vec<u8>, SemanticDecodedAttV1) {
        let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(ARTIFACT).unwrap();
        let layout = inspected.load_layout().unwrap();
        let pc_address = pc_override.unwrap_or(inspected.bindings()[0].entry_address());
        let digest = serde_json::to_string(
            &CaptureIdentityV1::new(Sha256::digest(artifact).into()).unwrap(),
        )
        .unwrap();
        let digest = digest.trim_matches('"');
        let mut export = std::str::from_utf8(EXPORT)
            .unwrap()
            .strip_suffix('\n')
            .unwrap()
            .to_owned();
        export = export.replacen(
            "2929292929292929292929292929292929292929292929292929292929292929",
            digest,
            1,
        );
        export = export.replacen(
            "\"load_address\":1048576,\"load_size\":4096",
            &format!(
                "\"load_address\":{},\"load_size\":{load_size}",
                layout.virtual_base()
            ),
            1,
        );
        export = export.replacen(
            "\"canonical_len\":4096",
            &format!("\"canonical_len\":{}", artifact.len()),
            1,
        );
        for address in [
            256_u64, 260, 264, 268, 272, 276, 280, 284, 288, 292, 296, 300, 304,
        ] {
            export = export.replace(
                &format!("\"address\":{address},\"code_object_id\":77"),
                &format!("\"address\":{pc_address},\"code_object_id\":77"),
            );
        }
        let decoded = import_rocprofiler_sdk_decoded_att_v1(
            export.as_bytes(),
            &bundle(),
            decoder_binding(),
            DecodedAttImportLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            decoded.decoder.authenticity,
            DecodedAttAuthenticityV1::UnavailableSelfClaimedExternalDecoder
        );
        let encoded = encode_decoded_att_v1(&decoded).unwrap();
        (encoded, decoded)
    }

    fn decoded_for_artifact(artifact: &[u8]) -> (Vec<u8>, SemanticDecodedAttV1) {
        let load_size = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(ARTIFACT)
            .unwrap()
            .load_layout()
            .unwrap()
            .memory_size();
        decoded_for_artifact_with_load_and_pc(artifact, load_size, None)
    }

    fn source_coordinate(byte: u8) -> SourceIsaCharacteristicSourceCoordinateV1 {
        SourceIsaCharacteristicSourceCoordinateV1::new(
            [byte; 32],
            SourceIsaCharacteristicSourceSpanV1::new(
                [byte + 1; 32],
                u64::from(byte),
                u64::from(byte) + 4,
                u32::from(byte),
                1,
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn characteristic_content(byte: u8, len: u64) -> SourceIsaCharacteristicContentIdentityV1 {
        SourceIsaCharacteristicContentIdentityV1::new([byte; 32], len).unwrap()
    }

    fn correlation(
        with_source: bool,
        intervals: Vec<SourceIsaCharacteristicIsaIntervalV1>,
        transformation: SourceIsaCharacteristicTransformationV1,
    ) -> SourceIsaCharacteristicTargetCorrelationV1 {
        SourceIsaCharacteristicTargetCorrelationV1::new(
            0,
            if with_source {
                SourceIsaCharacteristicRecordKindV1::SourceAnchored
            } else {
                SourceIsaCharacteristicRecordKindV1::NoSourceProvenance
            },
            with_source.then(|| source_coordinate(20)),
            with_source.then_some([40; 32]),
            with_source.then(|| SourceIsaCharacteristicMirCoordinateV1::new(0, 1, 0).unwrap()),
            with_source.then_some([50; 32]),
            with_source.then(|| SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 0).unwrap()),
            SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap(),
            [60; 32],
            SourceIsaCharacteristicCompilerHandoffLlvmCoordinateV1::new(0, 2, 0).unwrap(),
            intervals,
            transformation,
        )
        .unwrap()
    }

    fn characteristic(artifact: &[u8], kind: CorrelationFixtureV1) -> Vec<u8> {
        let interval = SourceIsaCharacteristicIsaIntervalV1::new(0, 0, 4).unwrap();
        let correlations = match kind {
            CorrelationFixtureV1::Unique | CorrelationFixtureV1::Partial => vec![correlation(
                true,
                vec![interval],
                SourceIsaCharacteristicTransformationV1::Preserved,
            )],
            CorrelationFixtureV1::Duplicated => vec![correlation(
                true,
                vec![interval, interval],
                SourceIsaCharacteristicTransformationV1::Duplicated,
            )],
            CorrelationFixtureV1::NoSource => vec![correlation(
                false,
                vec![interval],
                SourceIsaCharacteristicTransformationV1::Preserved,
            )],
        };
        let record_count = u64::try_from(correlations.len()).unwrap() + 1;
        let partial = matches!(kind, CorrelationFixtureV1::Partial);
        let collection = SourceIsaCharacteristicCollectionV1::new(
            SourceIsaCharacteristicBindingV1::new(
                SourceIsaCharacteristicTargetProfileV1::Gfx942,
                SourceIsaCharacteristicKirVersionV1::V8,
                [1; 32],
                SourceIsaCharacteristicStructuralCountsV1 {
                    functions: 1,
                    defined_bodies: 1,
                    blocks: 1,
                    operations: 1 + u64::from(partial),
                },
                characteristic_content(2, 20),
                characteristic_content(3, 30),
                characteristic_content(4, 40),
                SourceIsaCharacteristicContentIdentityV1::new(
                    Sha256::digest(artifact).into(),
                    u64::try_from(artifact.len()).unwrap(),
                )
                .unwrap(),
                characteristic_content(6, 60),
                characteristic_content(7, 70),
                [8; 32],
                [9; 32],
            )
            .unwrap(),
            SourceIsaCharacteristicScanSummaryV1::new(
                record_count + u64::from(partial),
                record_count,
                1 + u64::from(partial),
                1,
                1,
                u64::try_from(correlations.len()).unwrap(),
                1,
                record_count,
                if partial {
                    SourceIsaCharacteristicScanStateV1::Missing(
                        SourceIsaCharacteristicMissingReasonV1::NoAdmittedCorrelation,
                    )
                } else {
                    SourceIsaCharacteristicScanStateV1::Complete
                },
            )
            .unwrap(),
            vec![
                SourceIsaCharacteristicTargetV1::new(
                    SourceIsaCharacteristicKindV1::GlobalStore {
                        form: SourceIsaCharacteristicMemoryFormV1::Plain,
                    },
                    SourceIsaCharacteristicKirCoordinateV1::new(0, 0, 1).unwrap(),
                    correlations,
                )
                .unwrap(),
            ],
            vec![
                SourceIsaCharacteristicPreKirEliminationV1::new(
                    1,
                    source_coordinate(30),
                    [70; 32],
                    SourceIsaCharacteristicMirCoordinateV1::new(0, 3, 0).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap();
        collection.encode_canonical().unwrap()
    }

    fn evidence(kind: CorrelationFixtureV1) -> EvidenceV1 {
        let (decoded, admitted) = decoded_for_artifact(ARTIFACT);
        EvidenceV1 {
            code_object: admitted.code_objects[0].identity,
            instruction: admitted.waves[0].instructions[0].identity,
            native: admitted.occupancy[1].identity,
            decoded,
            characteristic: characteristic(ARTIFACT, kind),
        }
    }

    fn open(kind: CorrelationFixtureV1) -> (DecodedAttSourceIsaSessionV1, EvidenceV1) {
        let evidence = evidence(kind);
        let session = DecodedAttSourceIsaSessionV1::open(
            &evidence.decoded,
            evidence.code_object,
            ARTIFACT,
            &evidence.characteristic,
            DecodedAttSourceIsaLimitsV1::default(),
        )
        .unwrap();
        (session, evidence)
    }

    fn page(result: DecodedAttSourceIsaQueryResultV1) -> DecodedAttSourceIsaPageV1 {
        let DecodedAttSourceIsaQueryResultV1::AttributionPage { page } = result else {
            panic!("expected attribution page")
        };
        *page
    }

    fn lower_hex(bytes: &[u8]) -> String {
        const DIGITS: &[u8; 16] = b"0123456789abcdef";
        let mut output = String::with_capacity(bytes.len() * 2);
        for byte in bytes {
            output.push(char::from(DIGITS[usize::from(byte >> 4)]));
            output.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
        }
        output
    }

    fn append_request(input: &mut Vec<u8>, request: &AgentDecodedAttSourceIsaRequestV1) {
        input.extend(serde_json::to_vec(request).unwrap());
        input.push(b'\n');
    }

    #[test]
    fn exact_symbol_pc_projects_every_characteristic_axis_and_att_truth() {
        let (session, evidence) = open(CorrelationFixtureV1::Unique);
        let first = page(
            session
                .lookup_record(
                    evidence.instruction,
                    DecodedAttSourceIsaPageRequestV1::default(),
                )
                .unwrap(),
        );
        assert_eq!(
            first.attribution_state,
            PcSourceIsaAttributionStateV1::UniqueSource
        );
        assert!(first.singular_attribution);
        assert_eq!(first.resolved_pc.metadata_kernel_ordinal, 0);
        assert_eq!(first.resolved_pc.symbol_relative_pc, 0);
        let item = &first.items[0];
        assert_eq!(
            item.relation_kind,
            DecodedAttSourceIsaRelationKindV1::ExactDecodedPcToBoundElfSymbolToCharacteristicInterval
        );
        assert_eq!(
            item.decoded_completeness,
            DecodedAttCompletenessV1::IncompleteInfoReported
        );
        assert_eq!(
            item.decoded_loss,
            DecodedAttLossStateV1::ExternalDecoderReportedDataLost
        );
        assert_eq!(item.raw_decode_relation, first.binding.raw_decode_relation);
        assert!(item.attribution.source.is_some());
        assert!(item.attribution.mir.is_some());
        assert!(item.attribution.neutral_kir.is_some());
        assert_eq!(item.attribution.target_kir.operation_ordinal, 1);
        assert_eq!(
            item.attribution.compiler_handoff_llvm.instruction_ordinal,
            0
        );
        assert_eq!(item.attribution.isa.symbol_relative_start, 0);
        let result = DecodedAttSourceIsaQueryResultV1::AttributionPage {
            page: Box::new(first),
        };
        let encoded = session.query_json(&result).unwrap();
        assert_eq!(encoded, session.query_json(&result).unwrap());
        let text = std::str::from_utf8(&encoded).unwrap();
        assert!(!text.contains("elf_virtual_address"));
        assert!(!text.contains("kernel_name"));
        assert!(!text.contains("vecadd"));
    }

    #[test]
    fn pages_duplicate_occurrences_and_rejects_cross_archive_cursor() {
        let (session, evidence) = open(CorrelationFixtureV1::Duplicated);
        let first = page(
            session
                .lookup_record(
                    evidence.instruction,
                    DecodedAttSourceIsaPageRequestV1 {
                        limit: 1,
                        cursor: None,
                    },
                )
                .unwrap(),
        );
        assert_eq!(
            first.attribution_state,
            PcSourceIsaAttributionStateV1::DuplicatedIntervalOccurrences
        );
        assert_eq!(first.total_matching_interval_occurrences, 2);
        let cursor = first.next_cursor.unwrap();
        let second = page(
            session
                .lookup_record(
                    evidence.instruction,
                    DecodedAttSourceIsaPageRequestV1 {
                        limit: 1,
                        cursor: Some(cursor),
                    },
                )
                .unwrap(),
        );
        assert_eq!(second.returned, 1);
        assert!(second.next_cursor.is_none());
        assert_ne!(
            first.items[0].attribution.item_identity,
            second.items[0].attribution.item_identity
        );

        let other_characteristic = characteristic(ARTIFACT, CorrelationFixtureV1::NoSource);
        let other = DecodedAttSourceIsaSessionV1::open(
            &evidence.decoded,
            evidence.code_object,
            ARTIFACT,
            &other_characteristic,
            DecodedAttSourceIsaLimitsV1::default(),
        )
        .unwrap();
        assert!(matches!(
            other.lookup_record(
                evidence.instruction,
                DecodedAttSourceIsaPageRequestV1 {
                    limit: 1,
                    cursor: Some(cursor),
                },
            ),
            Err(DecodedAttSourceIsaErrorV1::CursorMismatch)
        ));
    }

    #[test]
    fn native_address_is_typed_unavailable_and_never_serialized() {
        let (session, evidence) = open(CorrelationFixtureV1::Partial);
        assert_eq!(
            session.binding().characteristic_scan.availability,
            crate::PcSourceIsaScanAvailabilityV1::Missing
        );
        let result = session
            .lookup_record(evidence.native, DecodedAttSourceIsaPageRequestV1::default())
            .unwrap();
        assert!(matches!(
            result,
            DecodedAttSourceIsaQueryResultV1::PcUnavailable {
                reason: DecodedAttSourceIsaUnavailableReasonV1::NativeVirtualAddressRedacted,
                ..
            }
        ));
        let encoded = session.query_json(&result).unwrap();
        let text = std::str::from_utf8(&encoded).unwrap();
        assert!(!text.contains("3735928559"));
        assert!(!text.contains("deadbeef"));
    }

    #[test]
    fn unavailable_source_and_pc_boundaries_remain_typed() {
        let (session, evidence) = open(CorrelationFixtureV1::NoSource);
        let no_source = page(
            session
                .lookup_record(
                    evidence.instruction,
                    DecodedAttSourceIsaPageRequestV1::default(),
                )
                .unwrap(),
        );
        assert_eq!(
            no_source.attribution_state,
            PcSourceIsaAttributionStateV1::UniqueNoSourceProvenance
        );
        assert!(no_source.items[0].attribution.source.is_none());
        let layout_size = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(ARTIFACT)
            .unwrap()
            .load_layout()
            .unwrap()
            .memory_size();
        for (address, expected) in [
            (
                1,
                DecodedAttSourceIsaUnavailableReasonV1::UnalignedElfVirtualAddress,
            ),
            (
                0,
                DecodedAttSourceIsaUnavailableReasonV1::OutsideKernelSymbol,
            ),
        ] {
            let (decoded, admitted) =
                decoded_for_artifact_with_load_and_pc(ARTIFACT, layout_size, Some(address));
            let boundary = DecodedAttSourceIsaSessionV1::open(
                &decoded,
                admitted.code_objects[0].identity,
                ARTIFACT,
                &evidence.characteristic,
                DecodedAttSourceIsaLimitsV1::default(),
            )
            .unwrap();
            let result = boundary
                .lookup_record(
                    admitted.waves[0].instructions[0].identity,
                    DecodedAttSourceIsaPageRequestV1::default(),
                )
                .unwrap();
            assert!(matches!(
                result,
                DecodedAttSourceIsaQueryResultV1::PcUnavailable { reason, .. } if reason == expected
            ));
        }
        assert!(matches!(
            session.lookup_record(
                CaptureIdentityV1::new([99; 32]).unwrap(),
                DecodedAttSourceIsaPageRequestV1::default(),
            ),
            Ok(DecodedAttSourceIsaQueryResultV1::PcUnavailable {
                reason: DecodedAttSourceIsaUnavailableReasonV1::UnknownRecord,
                ..
            })
        ));
    }

    #[test]
    fn artifact_characteristic_load_and_symbol_substitutions_are_rejected() {
        let evidence = evidence(CorrelationFixtureV1::Unique);
        assert!(matches!(
            DecodedAttSourceIsaSessionV1::open(
                &evidence.decoded,
                CaptureIdentityV1::new([99; 32]).unwrap(),
                ARTIFACT,
                &evidence.characteristic,
                DecodedAttSourceIsaLimitsV1::default(),
            ),
            Err(DecodedAttSourceIsaErrorV1::UnknownCodeObject)
        ));
        let mut changed_artifact = ARTIFACT.to_vec();
        changed_artifact[0x100] ^= 1;
        assert!(matches!(
            DecodedAttSourceIsaSessionV1::open(
                &evidence.decoded,
                evidence.code_object,
                &changed_artifact,
                &evidence.characteristic,
                DecodedAttSourceIsaLimitsV1::default(),
            ),
            Err(DecodedAttSourceIsaErrorV1::ArtifactSubstitution)
        ));

        let wrong_characteristic = characteristic(&changed_artifact, CorrelationFixtureV1::Unique);
        assert!(matches!(
            DecodedAttSourceIsaSessionV1::open(
                &evidence.decoded,
                evidence.code_object,
                ARTIFACT,
                &wrong_characteristic,
                DecodedAttSourceIsaLimitsV1::default(),
            ),
            Err(DecodedAttSourceIsaErrorV1::CharacteristicArtifactSubstitution)
        ));

        let (decoded_bytes, decoded) = decoded_for_artifact_with_load_and_pc(ARTIFACT, 4096, None);
        assert!(matches!(
            DecodedAttSourceIsaSessionV1::open(
                &decoded_bytes,
                decoded.code_objects[0].identity,
                ARTIFACT,
                &evidence.characteristic,
                DecodedAttSourceIsaLimitsV1::default(),
            ),
            Err(DecodedAttSourceIsaErrorV1::ArtifactLoadSizeMismatch)
        ));

        let mut changed_symbol = ARTIFACT.to_vec();
        changed_symbol[0xdd8] ^= 4;
        let (decoded_bytes, decoded) = decoded_for_artifact(&changed_symbol);
        assert!(matches!(
            DecodedAttSourceIsaSessionV1::open(
                &decoded_bytes,
                decoded.code_objects[0].identity,
                &changed_symbol,
                &characteristic(&changed_symbol, CorrelationFixtureV1::Unique),
                DecodedAttSourceIsaLimitsV1::default(),
            ),
            Err(DecodedAttSourceIsaErrorV1::ArtifactAdmission)
        ));
    }

    #[test]
    fn overlapping_symbol_domains_are_explicitly_ambiguous() {
        let symbols = [
            BoundKernelSymbolV1 {
                metadata_kernel_ordinal: 0,
                entry_address: 0x100,
                entry_size: 0x20,
            },
            BoundKernelSymbolV1 {
                metadata_kernel_ordinal: 1,
                entry_address: 0x110,
                entry_size: 0x20,
            },
        ];
        assert!(matches!(
            resolve_kernel_symbol(&symbols, 0x114),
            Err(DecodedAttSourceIsaUnavailableReasonV1::AmbiguousOverlappingKernelSymbols)
        ));
        assert_eq!(
            resolve_kernel_symbol(&symbols, 0x104)
                .unwrap()
                .metadata_kernel_ordinal,
            0
        );
        assert!(matches!(
            resolve_kernel_symbol(&symbols, 0x200),
            Err(DecodedAttSourceIsaUnavailableReasonV1::OutsideKernelSymbol)
        ));
    }

    #[test]
    fn strict_agent_service_opens_queries_and_closes_without_address_or_name_disclosure() {
        let evidence = evidence(CorrelationFixtureV1::Unique);
        let mut input = Vec::new();
        append_request(
            &mut input,
            &AgentDecodedAttSourceIsaRequestV1::Open {
                schema: AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 1,
                revision: 0,
                code_object_identity: evidence.code_object,
                interchange_hex: lower_hex(&evidence.decoded),
                artifact_hex: lower_hex(ARTIFACT),
                characteristic_hex: lower_hex(&evidence.characteristic),
            },
        );
        append_request(
            &mut input,
            &AgentDecodedAttSourceIsaRequestV1::LookupRecord {
                schema: AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                revision: 1,
                record_identity: evidence.instruction,
                page: DecodedAttSourceIsaPageRequestV1::default(),
            },
        );
        append_request(
            &mut input,
            &AgentDecodedAttSourceIsaRequestV1::Close {
                schema: AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 3,
                revision: 2,
            },
        );
        let mut output = Vec::new();
        run_agent_decoded_att_source_isa_jsonl_v1(&mut input.as_slice(), &mut output).unwrap();
        let lines: Vec<&[u8]> = output
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .collect();
        assert_eq!(lines.len(), 3);
        for line in &lines {
            let response: serde_json::Value = serde_json::from_slice(line).unwrap();
            assert_eq!(response["status"], "ok");
        }
        let text = std::str::from_utf8(&output).unwrap();
        assert!(!text.contains("elf_virtual_address"));
        assert!(!text.contains("vecadd"));
        assert!(!text.contains("3735928559"));
    }

    #[test]
    fn agent_rejects_unknown_fields_duplicate_ids_and_oversize_unterminated_records() {
        let malformed = format!(
            "{{\"operation\":\"binding\",\"schema\":\"{}\",\"request_id\":1,\"revision\":0,\"unknown\":true}}\n",
            AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1
        );
        let mut duplicate = Vec::new();
        append_request(
            &mut duplicate,
            &AgentDecodedAttSourceIsaRequestV1::Binding {
                schema: AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                revision: 0,
            },
        );
        append_request(
            &mut duplicate,
            &AgentDecodedAttSourceIsaRequestV1::Binding {
                schema: AGENT_DECODED_ATT_SOURCE_ISA_REQUEST_SCHEMA_V1.to_owned(),
                request_id: 2,
                revision: 0,
            },
        );
        let mut input = malformed.into_bytes();
        input.extend(duplicate);
        let mut output = Vec::new();
        run_agent_decoded_att_source_isa_jsonl_v1(&mut input.as_slice(), &mut output).unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        assert!(text.contains("invalid_request"));
        assert!(text.contains("session_not_open"));
        assert!(text.contains("duplicate_request_id"));

        let hostile = b"{}{}";
        let mut output = Vec::new();
        run_agent_decoded_att_source_isa_jsonl_with_limit_v1(
            &mut hostile.as_slice(),
            &mut output,
            2,
        )
        .unwrap();
        let text = std::str::from_utf8(&output).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.contains("request_too_large"));
        assert!(text.contains("\"terminal\":true"));
    }

    #[test]
    fn page_and_response_resources_are_hard_bounded() {
        let (session, evidence) = open(CorrelationFixtureV1::Unique);
        assert!(matches!(
            session.lookup_record(
                evidence.instruction,
                DecodedAttSourceIsaPageRequestV1 {
                    limit: 0,
                    cursor: None,
                },
            ),
            Err(DecodedAttSourceIsaErrorV1::PageLimit)
        ));
        assert!(matches!(
            encode_bounded(
                &"x".repeat(usize::try_from(MIN_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1).unwrap()),
                MIN_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1 - 1,
            ),
            Err(DecodedAttSourceIsaErrorV1::ResponseTooLarge)
        ));
        assert!(matches!(
            DecodedAttSourceIsaLimitsV1 {
                max_response_bytes: MIN_DECODED_ATT_SOURCE_ISA_RESPONSE_BYTES_V1 - 1,
                ..DecodedAttSourceIsaLimitsV1::default()
            }
            .validate(),
            Err(DecodedAttSourceIsaErrorV1::LimitOutOfRange)
        ));
    }
}

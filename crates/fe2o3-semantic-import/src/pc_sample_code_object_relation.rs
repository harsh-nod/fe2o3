use std::error::Error;
use std::fmt;

use fe2o3_hsaco::{CodeObjectLoadLayout, InspectedKernelBindings};
use fe2o3_semantic_trace::{
    ContentIdentitySchemeV1, ContentIdentityV1, KernelIrIdentityClaimV1, OpaqueIdentityV1,
    WaveWidthV1,
};
use serde::de::{IgnoredAny, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    ArtifactClaimV1, BoundedVec, CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1,
    IdentityFactV1, ImportErrorV1, ImportLimitsV1, MAX_ROCPROF_PROCESSES_V1,
    PcSampleCaptureErrorV3, RocprofCaptureBindingV1, RocprofPcBufferRecords,
    RocprofPcSampleCaptureBindingV3, SemanticPcSampleCaptureV3, TruthOriginV1,
    decode_pc_sample_capture_v3, encode_pc_sample_capture_v3,
    import_rocprofv3_pc_sample_capture_v3, parse_rocprof_json_document_v1,
    pc_sample_capture_content_identity_v3,
};

pub const PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1: u64 = 8 * 1024 * 1024;
pub const MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1: usize = 16_384;
pub const MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1: usize = 65_536;
pub const PC_SAMPLE_CODE_OBJECT_RELATION_IDENTITY_DOMAIN_V1: &[u8] =
    b"fe2o3.semantic-pc-sample-code-object-relation.v1\0";

#[derive(Deserialize)]
struct RocprofRelationDocumentV1 {
    #[serde(rename = "rocprofiler-sdk-tool")]
    processes: BoundedVec<RocprofRelationProcessV1, MAX_ROCPROF_PROCESSES_V1>,
}

#[derive(Deserialize)]
struct RocprofRelationCatalogDocumentV1 {
    #[serde(rename = "rocprofiler-sdk-tool")]
    processes: BoundedVec<RocprofRelationCatalogProcessV1, MAX_ROCPROF_PROCESSES_V1>,
}

#[derive(Deserialize)]
struct RocprofRelationCatalogProcessV1 {
    #[serde(default)]
    code_objects: CountedJsonArrayV1,
    #[serde(default)]
    kernel_symbols: CountedJsonArrayV1,
}

#[derive(Clone, Copy, Default)]
struct CountedJsonArrayV1(usize);

impl<'de> Deserialize<'de> for CountedJsonArrayV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CountVisitorV1;

        impl<'de> Visitor<'de> for CountVisitorV1 {
            type Value = CountedJsonArrayV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON array with a representable item count")
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut count = 0_usize;
                while sequence.next_element::<IgnoredAny>()?.is_some() {
                    count = count
                        .checked_add(1)
                        .ok_or_else(|| serde::de::Error::custom("JSON item count overflow"))?;
                }
                Ok(CountedJsonArrayV1(count))
            }
        }

        deserializer.deserialize_seq(CountVisitorV1)
    }
}

#[derive(Deserialize)]
struct RocprofRelationProcessV1 {
    buffer_records: RocprofPcBufferRecords,
    #[serde(default)]
    code_objects: BoundedVec<RocprofCodeObjectLoadV1, MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1>,
    #[serde(default)]
    kernel_symbols: BoundedVec<RocprofKernelSymbolV1, MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1>,
}

#[derive(Clone, Copy, Deserialize)]
struct RocprofRelationHandleV1 {
    handle: u64,
}

#[derive(Clone, Copy, Deserialize)]
struct RocprofCodeObjectLoadV1 {
    code_object_id: u64,
    #[serde(default)]
    agent_id: Option<RocprofRelationHandleV1>,
    #[serde(default)]
    rocp_agent: Option<RocprofRelationHandleV1>,
    load_base: u64,
    load_size: u64,
    load_delta: i64,
}

impl RocprofCodeObjectLoadV1 {
    fn agent_handle(self) -> Result<u64, PcSampleCodeObjectRelationErrorV1> {
        match (self.agent_id, self.rocp_agent) {
            (Some(agent), None) | (None, Some(agent)) => Ok(agent.handle),
            (Some(agent), Some(deprecated)) if agent.handle == deprecated.handle => {
                Ok(agent.handle)
            }
            _ => Err(PcSampleCodeObjectRelationErrorV1::InvalidStructuredLoad),
        }
    }
}

#[derive(Clone, Copy, Deserialize)]
struct RocprofKernelSymbolV1 {
    kernel_id: u64,
    code_object_id: u64,
    kernel_object: u64,
    kernarg_segment_size: u32,
    kernarg_segment_alignment: u32,
    group_segment_size: u32,
    private_segment_size: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleCodeObjectRelationUnavailableReasonV1 {
    MissingStructuredLoad,
    AmbiguousStructuredLoad,
    MissingStructuredKernelSymbols,
    IncompleteStructuredKernelSymbols,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleCodeObjectRelationStatusV1 {
    ExactDeclaredArtifactStructure,
    Unavailable(PcSampleCodeObjectRelationUnavailableReasonV1),
}

/// One capture-local code-object relation. Raw rocprof IDs and native load
/// addresses are deliberately absent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleCodeObjectRelationRecordV1 {
    pub code_object_identity: CaptureIdentityV1,
    pub source_code_object_ordinal: u64,
    pub process_index: u32,
    pub device_identity: CaptureIdentityV1,
    pub status: PcSampleCodeObjectRelationStatusV1,
    pub loaded_code_object_size: Option<u64>,
}

/// Exact symbol interval expressed only in the code-object-relative offset
/// domain used by rocprof PC samples.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleKernelSymbolDomainV1 {
    pub code_object_identity: CaptureIdentityV1,
    pub metadata_kernel_ordinal: u32,
    pub code_object_offset: u64,
    pub byte_len: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleCodeObjectRelationClaimsV1 {
    pub retains_native_addresses: bool,
    pub grants_load_or_execution_authority: bool,
    pub claims_runtime_loaded_bytes_equal_artifact: bool,
    pub claims_complete_code_object_lifetime: bool,
    pub identifies_a_live_pc: bool,
    pub claims_complete_sample_coverage: bool,
    pub claims_complete_instruction_history: bool,
    pub claims_schedule_correlation: bool,
    pub claims_source_attribution: bool,
}

impl PcSampleCodeObjectRelationClaimsV1 {
    const NONE: Self = Self {
        retains_native_addresses: false,
        grants_load_or_execution_authority: false,
        claims_runtime_loaded_bytes_equal_artifact: false,
        claims_complete_code_object_lifetime: false,
        identifies_a_live_pc: false,
        claims_complete_sample_coverage: false,
        claims_complete_instruction_history: false,
        claims_schedule_correlation: false,
        claims_source_attribution: false,
    };
}

/// Canonical sidecar for frozen Semantic PC Sample Capture V3.
///
/// This inert document does not carry the rocprof native addresses needed to
/// establish admission. Consumers that need authenticated answers must replay
/// [`admit_rocprofv3_pc_sample_code_object_relation_v1`] over the exact source,
/// capture, and artifact bytes and compare the canonical sidecar bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticPcSampleCodeObjectRelationV1 {
    pub schema_version: u16,
    pub source_identity: ContentIdentityRecordV1,
    pub capture_identity: ContentIdentityRecordV1,
    pub artifact_identity: ContentIdentityRecordV1,
    pub records: Vec<PcSampleCodeObjectRelationRecordV1>,
    pub symbol_domains: Vec<PcSampleKernelSymbolDomainV1>,
    pub claims: PcSampleCodeObjectRelationClaimsV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticPcSampleCodeObjectRelationWireV1 {
    schema_version: u16,
    source_identity: ContentIdentityRecordV1,
    capture_identity: ContentIdentityRecordV1,
    artifact_identity: ContentIdentityRecordV1,
    records: BoundedVec<PcSampleCodeObjectRelationRecordV1, MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1>,
    symbol_domains: BoundedVec<PcSampleKernelSymbolDomainV1, MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1>,
    claims: PcSampleCodeObjectRelationClaimsV1,
}

impl<'de> Deserialize<'de> for SemanticPcSampleCodeObjectRelationV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = SemanticPcSampleCodeObjectRelationWireV1::deserialize(deserializer)?;
        Ok(Self {
            schema_version: wire.schema_version,
            source_identity: wire.source_identity,
            capture_identity: wire.capture_identity,
            artifact_identity: wire.artifact_identity,
            records: wire.records.0,
            symbol_domains: wire.symbol_domains.0,
            claims: wire.claims,
        })
    }
}

impl SemanticPcSampleCodeObjectRelationV1 {
    pub fn validate_against_capture(
        &self,
        capture_bytes: &[u8],
    ) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
        if self.schema_version != PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1 {
            return Err(PcSampleCodeObjectRelationErrorV1::UnsupportedVersion(
                self.schema_version,
            ));
        }
        let capture = decode_pc_sample_capture_v3(capture_bytes)
            .map_err(PcSampleCodeObjectRelationErrorV1::Capture)?;
        if self.capture_identity
            != pc_sample_capture_content_identity_v3(capture_bytes)
                .map_err(PcSampleCodeObjectRelationErrorV1::Capture)?
            || self.source_identity != capture.runs[0].source
        {
            return Err(PcSampleCodeObjectRelationErrorV1::StaleCapture);
        }
        validate_content_identity(self.artifact_identity)?;
        if self.artifact_identity.scheme != ContentSchemeV1::RawCanonicalSha256 {
            return Err(PcSampleCodeObjectRelationErrorV1::InvalidArtifactIdentity);
        }
        if capture.dispatches.iter().any(|dispatch| {
            dispatch.artifact
                != (IdentityFactV1 {
                    origin: TruthOriginV1::Declared,
                    value: Some(self.artifact_identity),
                    unavailable_reason: None,
                })
        }) {
            return Err(PcSampleCodeObjectRelationErrorV1::ArtifactSubstitution);
        }
        if self.claims != PcSampleCodeObjectRelationClaimsV1::NONE {
            return Err(PcSampleCodeObjectRelationErrorV1::InvalidClaims);
        }
        if self.records.len() != capture.code_objects.len()
            || self.records.len() > MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1
            || self.symbol_domains.len() > MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1
        {
            return Err(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog);
        }

        if !self
            .records
            .windows(2)
            .all(|pair| pair[0].code_object_identity < pair[1].code_object_identity)
        {
            return Err(PcSampleCodeObjectRelationErrorV1::NonCanonicalOrder);
        }
        let mut capture_code_objects = Vec::new();
        capture_code_objects
            .try_reserve_exact(capture.code_objects.len())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
        capture_code_objects.extend(
            capture
                .code_objects
                .iter()
                .map(|code_object| (code_object.identity, code_object.source_code_object_ordinal)),
        );
        capture_code_objects.sort_unstable();
        if self
            .records
            .iter()
            .map(|record| {
                (
                    record.code_object_identity,
                    record.source_code_object_ordinal,
                )
            })
            .ne(capture_code_objects.iter().copied())
        {
            return Err(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog);
        }
        let expected_ownership = capture_relation_ownership(&capture)?;
        for record in &self.records {
            let (_, expected_process, expected_device) = expected_ownership
                .binary_search_by_key(&record.code_object_identity, |owner| owner.0)
                .ok()
                .and_then(|index| expected_ownership.get(index))
                .copied()
                .ok_or(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog)?;
            if record.process_index != expected_process {
                return Err(PcSampleCodeObjectRelationErrorV1::ProcessMismatch);
            }
            if record.device_identity != expected_device {
                return Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch);
            }
            match (record.status, record.loaded_code_object_size) {
                (
                    PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure,
                    Some(size),
                ) if size != 0 => {}
                (PcSampleCodeObjectRelationStatusV1::Unavailable(_), None) => {}
                _ => return Err(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog),
            }
        }

        if !self.symbol_domains.windows(2).all(|pair| pair[0] < pair[1]) {
            return Err(PcSampleCodeObjectRelationErrorV1::NonCanonicalOrder);
        }
        let mut domain_counts = Vec::new();
        domain_counts
            .try_reserve_exact(self.records.len())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
        domain_counts.resize(self.records.len(), 0_u64);
        let mut prior_kernel = None;
        for domain in &self.symbol_domains {
            let record_index = self
                .records
                .binary_search_by_key(&domain.code_object_identity, |record| {
                    record.code_object_identity
                })
                .ok()
                .ok_or(PcSampleCodeObjectRelationErrorV1::InvalidSymbolDomain)?;
            let record = self.records[record_index];
            let end = domain
                .code_object_offset
                .checked_add(domain.byte_len)
                .ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
            if record.status != PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure
                || domain.byte_len == 0
                || domain.code_object_offset % 4 != 0
                || domain.byte_len % 4 != 0
                || record
                    .loaded_code_object_size
                    .is_none_or(|load_size| end > load_size)
                || prior_kernel
                    == Some((domain.code_object_identity, domain.metadata_kernel_ordinal))
            {
                return Err(PcSampleCodeObjectRelationErrorV1::InvalidSymbolDomain);
            }
            prior_kernel = Some((domain.code_object_identity, domain.metadata_kernel_ordinal));
            domain_counts[record_index] = domain_counts[record_index]
                .checked_add(1)
                .ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
        }
        if self
            .records
            .iter()
            .zip(domain_counts)
            .any(|(record, count)| {
                (count != 0)
                    != (record.status
                        == PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure)
            })
        {
            return Err(PcSampleCodeObjectRelationErrorV1::InvalidSymbolDomain);
        }
        Ok(())
    }
}

fn capture_relation_ownership(
    capture: &SemanticPcSampleCaptureV3,
) -> Result<Vec<(CaptureIdentityV1, u32, CaptureIdentityV1)>, PcSampleCodeObjectRelationErrorV1> {
    let mut code_objects = Vec::new();
    code_objects
        .try_reserve_exact(capture.code_objects.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    code_objects.extend(
        capture
            .code_objects
            .iter()
            .enumerate()
            .map(|(ordinal, code_object)| (code_object.identity, ordinal)),
    );
    code_objects.sort_unstable_by_key(|item| item.0);
    let mut dispatches = Vec::new();
    dispatches
        .try_reserve_exact(capture.dispatches.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    dispatches.extend(capture.dispatches.iter().map(|dispatch| {
        (
            dispatch.identity,
            dispatch.process_index,
            dispatch.device_identity,
        )
    }));
    dispatches.sort_unstable_by_key(|item| item.0);
    let mut ownership = Vec::new();
    ownership
        .try_reserve_exact(capture.code_objects.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    ownership.resize(capture.code_objects.len(), None);
    for sample in &capture.samples {
        let Some(code_object_identity) = sample.pc.code_object_identity else {
            continue;
        };
        let ordinal = code_objects
            .binary_search_by_key(&code_object_identity, |item| item.0)
            .ok()
            .and_then(|index| code_objects.get(index))
            .map(|item| item.1)
            .ok_or(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog)?;
        let (_, process_index, device_identity) = dispatches
            .binary_search_by_key(&sample.dispatch_identity, |item| item.0)
            .ok()
            .and_then(|index| dispatches.get(index))
            .copied()
            .ok_or(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog)?;
        match ownership[ordinal] {
            None => ownership[ordinal] = Some((process_index, device_identity)),
            Some((process, _)) if process != process_index => {
                return Err(PcSampleCodeObjectRelationErrorV1::ProcessMismatch);
            }
            Some((_, device)) if device != device_identity => {
                return Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch);
            }
            Some(_) => {}
        }
    }
    let mut exact = Vec::new();
    exact
        .try_reserve_exact(capture.code_objects.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (code_object, owner) in capture.code_objects.iter().zip(ownership) {
        let (process, device) =
            owner.ok_or(PcSampleCodeObjectRelationErrorV1::InvalidRecordCatalog)?;
        exact.push((code_object.identity, process, device));
    }
    exact.sort_unstable_by_key(|item| item.0);
    Ok(exact)
}

/// Successfully admitted relation. Construction owns no load or execution
/// capability and retains no raw rocprof handle or native address.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmittedPcSampleCodeObjectRelationV1 {
    relation: SemanticPcSampleCodeObjectRelationV1,
}

impl AdmittedPcSampleCodeObjectRelationV1 {
    pub const fn relation(&self) -> &SemanticPcSampleCodeObjectRelationV1 {
        &self.relation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawCodeObjectKey {
    process_index: u32,
    code_object_id: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CodeObjectOwnerV1 {
    process_index: u32,
    device_identity: CaptureIdentityV1,
    agent_handle: u64,
}

#[derive(Clone, Copy)]
struct IndexedLoadV1 {
    key: RawCodeObjectKey,
    load: RocprofCodeObjectLoadV1,
}

#[derive(Clone, Copy)]
struct IndexedSymbolV1 {
    key: RawCodeObjectKey,
    symbol: RocprofKernelSymbolV1,
}

impl Ord for RawCodeObjectKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.process_index, self.code_object_id).cmp(&(other.process_index, other.code_object_id))
    }
}

impl PartialOrd for RawCodeObjectKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct KernelSignatureV1 {
    kernel_object: u64,
    kernarg_size: u32,
    kernarg_alignment: u32,
    group_segment_size: u32,
    private_segment_size: u32,
}

/// Replays the frozen V3 importer and admits an exact, address-redacted load
/// relation from structured rocprof code-object and kernel-symbol records.
pub fn admit_rocprofv3_pc_sample_code_object_relation_v1(
    source: &[u8],
    capture_bytes: &[u8],
    artifact_bytes: &[u8],
    limits: ImportLimitsV1,
) -> Result<AdmittedPcSampleCodeObjectRelationV1, PcSampleCodeObjectRelationErrorV1> {
    let capture = decode_pc_sample_capture_v3(capture_bytes)
        .map_err(PcSampleCodeObjectRelationErrorV1::Capture)?;
    replay_exact_capture(source, capture_bytes, &capture, limits)?;
    validate_relation_source_catalogs(source)?;
    let artifact_identity = exact_artifact_identity(&capture, artifact_bytes)?;
    let inspected = fe2o3_hsaco::inspect_and_bind_kernel_descriptors(artifact_bytes)
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::InvalidHsacoArtifact)?;
    if inspected.bindings().is_empty() {
        return Err(PcSampleCodeObjectRelationErrorV1::InvalidHsacoArtifact);
    }
    validate_capture_artifact_wave_widths(&capture, &inspected)?;
    let document = parse_relation_document_v1(source)?;
    reject_overlapping_loads(&document)?;

    let keys = capture_code_object_keys(&document, &capture)?;
    let ownership = capture_code_object_ownership(&document, &capture, &keys)?;
    let load_index = build_load_index(&document)?;
    let symbol_index = build_symbol_index(&document)?;
    let mut records = Vec::new();
    records
        .try_reserve_exact(capture.code_objects.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    let mut symbol_domains = Vec::new();

    for (ordinal, code_object) in capture.code_objects.iter().enumerate() {
        let key = keys
            .get(ordinal)
            .copied()
            .ok_or(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
        let owner = ownership
            .get(ordinal)
            .copied()
            .flatten()
            .ok_or(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
        if owner.process_index != key.process_index {
            return Err(PcSampleCodeObjectRelationErrorV1::ProcessMismatch);
        }
        let loads = equal_key_range(&load_index, key, |entry| entry.key);
        for indexed_load in loads {
            if indexed_load.load.agent_handle()? != owner.agent_handle {
                return Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch);
            }
            validate_exact_load(&indexed_load.load, &inspected)?;
        }
        let (status, loaded_size) = match loads {
            [] => (
                PcSampleCodeObjectRelationStatusV1::Unavailable(
                    PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredLoad,
                ),
                None,
            ),
            [indexed_load] => {
                let load = indexed_load.load;
                let load_layout = validate_exact_load(&load, &inspected)?;
                let symbols = equal_key_range(&symbol_index, key, |entry| entry.key);
                if symbols.is_empty() {
                    (
                        PcSampleCodeObjectRelationStatusV1::Unavailable(
                            PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredKernelSymbols,
                        ),
                        None,
                    )
                } else if symbols.len() < inspected.bindings().len() {
                    validate_incomplete_symbols(&load, symbols, &inspected)?;
                    (
                        PcSampleCodeObjectRelationStatusV1::Unavailable(
                            PcSampleCodeObjectRelationUnavailableReasonV1::IncompleteStructuredKernelSymbols,
                        ),
                        None,
                    )
                } else {
                    let domains = bind_exact_symbols(
                        &load,
                        load_layout,
                        symbols,
                        &inspected,
                        code_object.identity,
                    )?;
                    let new_len = symbol_domains
                        .len()
                        .checked_add(domains.len())
                        .ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
                    if new_len > MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1 {
                        return Err(PcSampleCodeObjectRelationErrorV1::TooManyKernelSymbols);
                    }
                    symbol_domains
                        .try_reserve(domains.len())
                        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
                    symbol_domains.extend(domains);
                    (
                        PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure,
                        Some(load.load_size),
                    )
                }
            }
            _ => (
                PcSampleCodeObjectRelationStatusV1::Unavailable(
                    PcSampleCodeObjectRelationUnavailableReasonV1::AmbiguousStructuredLoad,
                ),
                None,
            ),
        };
        records.push(PcSampleCodeObjectRelationRecordV1 {
            code_object_identity: code_object.identity,
            source_code_object_ordinal: code_object.source_code_object_ordinal,
            process_index: owner.process_index,
            device_identity: owner.device_identity,
            status,
            loaded_code_object_size: loaded_size,
        });
    }
    drop(keys);
    drop(ownership);
    drop(load_index);
    drop(symbol_index);
    drop(inspected);
    drop(document);
    records.sort_by_key(|record| record.code_object_identity);
    symbol_domains.sort_unstable();
    for sample in &capture.samples {
        let (Some(code_object_identity), Some(offset)) =
            (sample.pc.code_object_identity, sample.pc.code_object_offset)
        else {
            continue;
        };
        let record = records
            .binary_search_by_key(&code_object_identity, |record| record.code_object_identity)
            .ok()
            .and_then(|index| records.get(index))
            .ok_or(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
        if record.status == PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure
            && record
                .loaded_code_object_size
                .is_none_or(|load_size| offset >= load_size)
        {
            return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
        }
    }
    let source_identity = capture.runs[0].source;
    let capture_identity = pc_sample_capture_content_identity_v3(capture_bytes)
        .map_err(PcSampleCodeObjectRelationErrorV1::Capture)?;
    drop(capture);
    let relation = SemanticPcSampleCodeObjectRelationV1 {
        schema_version: PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1,
        source_identity,
        capture_identity,
        artifact_identity,
        records,
        symbol_domains,
        claims: PcSampleCodeObjectRelationClaimsV1::NONE,
    };
    relation.validate_against_capture(capture_bytes)?;
    Ok(AdmittedPcSampleCodeObjectRelationV1 { relation })
}

fn validate_relation_source_catalogs(
    source: &[u8],
) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
    let catalog = parse_relation_catalog_document_v1(source)?;
    let (loads, symbols) = catalog
        .processes
        .iter()
        .try_fold((0_usize, 0_usize), |(loads, symbols), process| {
            Some((
                loads.checked_add(process.code_objects.0)?,
                symbols.checked_add(process.kernel_symbols.0)?,
            ))
        })
        .ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
    if loads > MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1 {
        return Err(PcSampleCodeObjectRelationErrorV1::TooManyCodeObjectLoads);
    }
    if symbols > MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1 {
        return Err(PcSampleCodeObjectRelationErrorV1::TooManyKernelSymbols);
    }
    Ok(())
}

fn parse_relation_document_v1(
    source: &[u8],
) -> Result<RocprofRelationDocumentV1, PcSampleCodeObjectRelationErrorV1> {
    parse_rocprof_json_document_v1(source).map_err(|error| {
        map_rocprof_json_error_v1(error, PcSampleCodeObjectRelationErrorV1::InvalidRocprofJson)
    })
}

fn parse_relation_catalog_document_v1(
    source: &[u8],
) -> Result<RocprofRelationCatalogDocumentV1, PcSampleCodeObjectRelationErrorV1> {
    parse_rocprof_json_document_v1(source).map_err(|error| {
        map_rocprof_json_error_v1(error, PcSampleCodeObjectRelationErrorV1::InvalidRocprofJson)
    })
}

fn replay_exact_capture(
    source: &[u8],
    capture_bytes: &[u8],
    capture: &SemanticPcSampleCaptureV3,
    limits: ImportLimitsV1,
) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
    let first = capture
        .dispatches
        .first()
        .ok_or(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
    if capture.dispatches.iter().any(|dispatch| {
        dispatch.kernel_ir != first.kernel_ir
            || dispatch.artifact != first.artifact
            || dispatch.source_map != first.source_map
            || dispatch.launch.wave_width != first.launch.wave_width
    }) {
        return Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch);
    }
    let kernel_ir = KernelIrIdentityClaimV1::canonical_v7_claim(
        OpaqueIdentityV1::new(first.kernel_ir.digest.as_bytes())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?,
        first.kernel_ir.canonical_len,
    )
    .map_err(|_| PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
    let artifact = match first.artifact {
        IdentityFactV1 {
            origin: TruthOriginV1::Declared,
            value: Some(value),
            unavailable_reason: None,
        } => Some(ArtifactClaimV1 {
            identity: OpaqueIdentityV1::new(value.digest.as_bytes())
                .map_err(|_| PcSampleCodeObjectRelationErrorV1::InvalidArtifactIdentity)?,
            canonical_len: value.canonical_len,
            format_version: value.format_version,
        }),
        _ => return Err(PcSampleCodeObjectRelationErrorV1::ArtifactClaimUnavailable),
    };
    let source_map = match first.source_map {
        IdentityFactV1 {
            origin: TruthOriginV1::Declared,
            value: Some(value),
            unavailable_reason: None,
        } => Some(
            ContentIdentityV1::new(
                content_scheme(value.scheme),
                value.format_version,
                OpaqueIdentityV1::new(value.digest.as_bytes())
                    .map_err(|_| PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?,
                value.canonical_len,
            )
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?,
        ),
        IdentityFactV1 {
            origin: TruthOriginV1::Unavailable,
            value: None,
            unavailable_reason: Some(_),
        } => None,
        _ => return Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch),
    };
    let wave_width = match first.launch.wave_width {
        32 => WaveWidthV1::Wave32,
        64 => WaveWidthV1::Wave64,
        _ => return Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch),
    };
    let replay = import_rocprofv3_pc_sample_capture_v3(
        source,
        RocprofPcSampleCaptureBindingV3 {
            capture: RocprofCaptureBindingV1 {
                kernel_ir_claim: kernel_ir,
                artifact,
                source_map,
                wave_width,
            },
            sampling_interval_cycles: capture.coverage.sampling.interval,
        },
        limits,
    )
    .map_err(|error| {
        map_rocprof_json_error_v1(
            error,
            PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch,
        )
    })?;
    let replay_bytes =
        encode_pc_sample_capture_v3(&replay).map_err(PcSampleCodeObjectRelationErrorV1::Capture)?;
    if replay_bytes != capture_bytes {
        return Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch);
    }
    Ok(())
}

fn validate_capture_artifact_wave_widths(
    capture: &SemanticPcSampleCaptureV3,
    inspected: &InspectedKernelBindings,
) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
    let kernels = inspected.inspection().kernels();
    if kernels.is_empty()
        || capture.dispatches.iter().any(|dispatch| {
            kernels
                .iter()
                .any(|kernel| kernel.wavefront_size() != u32::from(dispatch.launch.wave_width))
        })
    {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    Ok(())
}

fn exact_artifact_identity(
    capture: &SemanticPcSampleCaptureV3,
    artifact_bytes: &[u8],
) -> Result<ContentIdentityRecordV1, PcSampleCodeObjectRelationErrorV1> {
    if artifact_bytes.is_empty() || artifact_bytes.len() > fe2o3_hsaco::MAX_HSACO_BYTES {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactSizeOutOfRange);
    }
    let expected = capture
        .dispatches
        .first()
        .and_then(|dispatch| dispatch.artifact.value)
        .ok_or(PcSampleCodeObjectRelationErrorV1::ArtifactClaimUnavailable)?;
    validate_content_identity(expected)?;
    let artifact_len = u64::try_from(artifact_bytes.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
    if expected.scheme != ContentSchemeV1::RawCanonicalSha256
        || expected.canonical_len != artifact_len
        || expected.digest.as_bytes() != <[u8; 32]>::from(Sha256::digest(artifact_bytes))
    {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactSubstitution);
    }
    Ok(expected)
}

fn capture_code_object_keys(
    document: &RocprofRelationDocumentV1,
    capture: &SemanticPcSampleCaptureV3,
) -> Result<Vec<RawCodeObjectKey>, PcSampleCodeObjectRelationErrorV1> {
    let mut occurrences = Vec::new();
    occurrences
        .try_reserve_exact(capture.samples.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    let mut encounter = 0_u64;
    for (process_index, process) in document.processes.iter().enumerate() {
        let process_index = u32::try_from(process_index)
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
        for sample in process.buffer_records.pc_sample_stochastic.iter() {
            if sample.record.pc.code_object_id == 0 {
                continue;
            }
            occurrences.push((
                RawCodeObjectKey {
                    process_index,
                    code_object_id: sample.record.pc.code_object_id,
                },
                encounter,
            ));
            encounter = encounter
                .checked_add(1)
                .ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
        }
    }
    occurrences.sort_unstable_by_key(|item| (item.0, item.1));
    let mut first_encounters = Vec::new();
    first_encounters
        .try_reserve_exact(capture.code_objects.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (key, ordinal) in occurrences {
        if first_encounters
            .last()
            .is_none_or(|(prior, _)| *prior != key)
        {
            if first_encounters.len() == capture.code_objects.len() {
                return Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch);
            }
            first_encounters.push((key, ordinal));
        }
    }
    first_encounters.sort_unstable_by_key(|item| item.1);
    let mut keys = Vec::new();
    keys.try_reserve_exact(first_encounters.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    keys.extend(first_encounters.into_iter().map(|item| item.0));
    if keys.len() != capture.code_objects.len() {
        return Err(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch);
    }
    Ok(keys)
}

fn capture_code_object_ownership(
    document: &RocprofRelationDocumentV1,
    capture: &SemanticPcSampleCaptureV3,
    keys: &[RawCodeObjectKey],
) -> Result<Vec<Option<CodeObjectOwnerV1>>, PcSampleCodeObjectRelationErrorV1> {
    let mut ownership = Vec::new();
    ownership
        .try_reserve_exact(keys.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    ownership.resize(keys.len(), None);
    let mut code_objects = Vec::new();
    code_objects
        .try_reserve_exact(capture.code_objects.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    code_objects.extend(
        capture
            .code_objects
            .iter()
            .enumerate()
            .map(|(ordinal, code_object)| (code_object.identity, ordinal)),
    );
    code_objects.sort_unstable_by_key(|item| item.0);
    let mut dispatches = Vec::new();
    dispatches
        .try_reserve_exact(capture.dispatches.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    dispatches.extend(capture.dispatches.iter().map(|dispatch| {
        (
            dispatch.identity,
            (
                dispatch.process_index,
                dispatch.dispatch_index,
                dispatch.device_identity,
            ),
        )
    }));
    dispatches.sort_unstable_by_key(|item| item.0);
    for sample in &capture.samples {
        let Some(code_object_identity) = sample.pc.code_object_identity else {
            continue;
        };
        let ordinal = code_objects
            .binary_search_by_key(&code_object_identity, |item| item.0)
            .ok()
            .and_then(|index| code_objects.get(index))
            .map(|item| item.1)
            .ok_or(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
        let (process_index, dispatch_index, device_identity) = dispatches
            .binary_search_by_key(&sample.dispatch_identity, |item| item.0)
            .ok()
            .and_then(|index| dispatches.get(index))
            .map(|item| item.1)
            .ok_or(PcSampleCodeObjectRelationErrorV1::CaptureSourceMismatch)?;
        if keys
            .get(ordinal)
            .is_none_or(|key| key.process_index != process_index)
        {
            return Err(PcSampleCodeObjectRelationErrorV1::ProcessMismatch);
        }
        let dispatch = document
            .processes
            .get(
                usize::try_from(process_index)
                    .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?,
            )
            .and_then(|process| {
                process
                    .buffer_records
                    .kernel_dispatch
                    .get(usize::try_from(dispatch_index).ok()?)
            })
            .ok_or(PcSampleCodeObjectRelationErrorV1::ProcessMismatch)?;
        let agent = dispatch
            .dispatch_info
            .agent_id
            .ok_or(PcSampleCodeObjectRelationErrorV1::DeviceMismatch)?
            .handle;
        let owner = CodeObjectOwnerV1 {
            process_index,
            device_identity,
            agent_handle: agent,
        };
        match ownership[ordinal] {
            None => ownership[ordinal] = Some(owner),
            Some(existing) if existing == owner => {}
            Some(existing) if existing.process_index != process_index => {
                return Err(PcSampleCodeObjectRelationErrorV1::ProcessMismatch);
            }
            Some(_) => return Err(PcSampleCodeObjectRelationErrorV1::DeviceMismatch),
        }
    }
    Ok(ownership)
}

fn reject_overlapping_loads(
    document: &RocprofRelationDocumentV1,
) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
    for process in document.processes.iter() {
        let mut intervals = Vec::new();
        intervals
            .try_reserve_exact(process.code_objects.len())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
        for load in process.code_objects.iter() {
            if load.code_object_id == 0 || load.load_size == 0 {
                return Err(PcSampleCodeObjectRelationErrorV1::InvalidStructuredLoad);
            }
            let end = load
                .load_base
                .checked_add(load.load_size)
                .ok_or(PcSampleCodeObjectRelationErrorV1::AddressOverflow)?;
            intervals.push((load.agent_handle()?, load.load_base, end));
        }
        intervals.sort_unstable();
        if intervals
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[1].1 < pair[0].2)
        {
            return Err(PcSampleCodeObjectRelationErrorV1::OverlappingStructuredLoads);
        }
    }
    Ok(())
}

fn build_load_index(
    document: &RocprofRelationDocumentV1,
) -> Result<Vec<IndexedLoadV1>, PcSampleCodeObjectRelationErrorV1> {
    let count = document
        .processes
        .iter()
        .try_fold(0_usize, |count, process| {
            count.checked_add(process.code_objects.len())
        });
    let count = count.ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
    if count > MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1 {
        return Err(PcSampleCodeObjectRelationErrorV1::TooManyCodeObjectLoads);
    }
    let mut index = Vec::new();
    index
        .try_reserve_exact(count)
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (process_index, process) in document.processes.iter().enumerate() {
        let process_index = u32::try_from(process_index)
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
        index.extend(
            process
                .code_objects
                .iter()
                .copied()
                .map(|load| IndexedLoadV1 {
                    key: RawCodeObjectKey {
                        process_index,
                        code_object_id: load.code_object_id,
                    },
                    load,
                }),
        );
    }
    index.sort_unstable_by_key(|entry| entry.key);
    Ok(index)
}

fn build_symbol_index(
    document: &RocprofRelationDocumentV1,
) -> Result<Vec<IndexedSymbolV1>, PcSampleCodeObjectRelationErrorV1> {
    let count = document
        .processes
        .iter()
        .try_fold(0_usize, |count, process| {
            count.checked_add(process.kernel_symbols.len())
        });
    let count = count.ok_or(PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
    if count > MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1 {
        return Err(PcSampleCodeObjectRelationErrorV1::TooManyKernelSymbols);
    }
    let mut index = Vec::new();
    index
        .try_reserve_exact(count)
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (process_index, process) in document.processes.iter().enumerate() {
        let process_index = u32::try_from(process_index)
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
        index.extend(
            process
                .kernel_symbols
                .iter()
                .copied()
                .map(|symbol| IndexedSymbolV1 {
                    key: RawCodeObjectKey {
                        process_index,
                        code_object_id: symbol.code_object_id,
                    },
                    symbol,
                }),
        );
    }
    index.sort_unstable_by_key(|entry| entry.key);
    Ok(index)
}

fn equal_key_range<T>(
    index: &[T],
    key: RawCodeObjectKey,
    key_of: impl Fn(&T) -> RawCodeObjectKey,
) -> &[T] {
    let begin = index.partition_point(|entry| key_of(entry) < key);
    let end = index.partition_point(|entry| key_of(entry) <= key);
    &index[begin..end]
}

fn bind_exact_symbols(
    load: &RocprofCodeObjectLoadV1,
    layout: CodeObjectLoadLayout,
    symbols: &[IndexedSymbolV1],
    inspected: &InspectedKernelBindings,
    code_object_identity: CaptureIdentityV1,
) -> Result<Vec<PcSampleKernelSymbolDomainV1>, PcSampleCodeObjectRelationErrorV1> {
    let load_end = load
        .load_base
        .checked_add(load.load_size)
        .ok_or(PcSampleCodeObjectRelationErrorV1::AddressOverflow)?;
    let kernels = inspected.inspection().kernels();
    let observed_count = symbols.len();
    if kernels.len() != inspected.bindings().len() || kernels.len() != observed_count {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(kernels.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (kernel, binding) in kernels.iter().zip(inspected.bindings()) {
        let kernel_object = checked_signed_add(binding.descriptor_address(), load.load_delta)?;
        expected.push((
            expected_signature(kernel, kernel_object)?,
            binding.kernel_index(),
            checked_signed_add(binding.entry_address(), load.load_delta)?,
            binding.entry_size(),
        ));
    }
    expected.sort_unstable_by_key(|item| item.0);
    let mut observed = Vec::new();
    observed
        .try_reserve_exact(observed_count)
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    let mut kernel_ids = Vec::new();
    kernel_ids
        .try_reserve_exact(observed_count)
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for indexed_symbol in symbols {
        let symbol = &indexed_symbol.symbol;
        kernel_ids.push(symbol.kernel_id);
        observed.push(observed_signature(symbol));
    }
    kernel_ids.sort_unstable();
    if kernel_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    observed.sort_unstable();
    if expected
        .iter()
        .map(|item| item.0)
        .ne(observed.iter().copied())
    {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    let mut domains = Vec::new();
    domains
        .try_reserve_exact(expected.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (_, kernel_ordinal, runtime_entry, entry_size) in expected {
        let end = runtime_entry
            .checked_add(entry_size)
            .ok_or(PcSampleCodeObjectRelationErrorV1::AddressOverflow)?;
        if runtime_entry < load.load_base || end > load_end {
            return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
        }
        let code_object_offset = inspected.bindings()[kernel_ordinal]
            .entry_address()
            .checked_sub(layout.virtual_base())
            .ok_or(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)?;
        if code_object_offset
            .checked_add(entry_size)
            .is_none_or(|end| end > layout.memory_size())
        {
            return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
        }
        domains.push(PcSampleKernelSymbolDomainV1 {
            code_object_identity,
            metadata_kernel_ordinal: u32::try_from(kernel_ordinal)
                .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?,
            code_object_offset,
            byte_len: entry_size,
        });
    }
    Ok(domains)
}

fn validate_incomplete_symbols(
    load: &RocprofCodeObjectLoadV1,
    symbols: &[IndexedSymbolV1],
    inspected: &InspectedKernelBindings,
) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
    let kernels = inspected.inspection().kernels();
    if kernels.len() != inspected.bindings().len() || symbols.len() >= kernels.len() {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    let mut expected = Vec::new();
    expected
        .try_reserve_exact(kernels.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for (kernel, binding) in kernels.iter().zip(inspected.bindings()) {
        expected.push(expected_signature(
            kernel,
            checked_signed_add(binding.descriptor_address(), load.load_delta)?,
        )?);
    }
    expected.sort_unstable();
    let mut observed = Vec::new();
    observed
        .try_reserve_exact(symbols.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    let mut kernel_ids = Vec::new();
    kernel_ids
        .try_reserve_exact(symbols.len())
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::AllocationFailure)?;
    for indexed_symbol in symbols {
        let symbol = indexed_symbol.symbol;
        observed.push(observed_signature(&symbol));
        kernel_ids.push(symbol.kernel_id);
    }
    kernel_ids.sort_unstable();
    if kernel_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    observed.sort_unstable();
    let mut expected_index = 0;
    for signature in observed {
        while expected
            .get(expected_index)
            .is_some_and(|candidate| *candidate < signature)
        {
            expected_index += 1;
        }
        if expected.get(expected_index) != Some(&signature) {
            return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
        }
        expected_index += 1;
    }
    Ok(())
}

fn validate_exact_load(
    load: &RocprofCodeObjectLoadV1,
    inspected: &InspectedKernelBindings,
) -> Result<CodeObjectLoadLayout, PcSampleCodeObjectRelationErrorV1> {
    let layout = inspected
        .load_layout()
        .ok_or(PcSampleCodeObjectRelationErrorV1::InvalidHsacoArtifact)?;
    let expected_load_base = checked_signed_add(layout.virtual_base(), load.load_delta)?;
    if load.load_base != expected_load_base || load.load_size != layout.memory_size() {
        return Err(PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution);
    }
    load.load_base
        .checked_add(load.load_size)
        .ok_or(PcSampleCodeObjectRelationErrorV1::AddressOverflow)?;
    Ok(layout)
}

fn expected_signature(
    kernel: &fe2o3_hsaco::InspectedKernel,
    kernel_object: u64,
) -> Result<KernelSignatureV1, PcSampleCodeObjectRelationErrorV1> {
    Ok(KernelSignatureV1 {
        kernel_object,
        kernarg_size: u32::try_from(kernel.kernarg_segment_size())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)?,
        kernarg_alignment: u32::try_from(kernel.kernarg_segment_alignment())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)?,
        group_segment_size: u32::try_from(kernel.group_segment_fixed_size())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)?,
        private_segment_size: u32::try_from(kernel.private_segment_fixed_size())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::ArtifactLoadSubstitution)?,
    })
}

fn observed_signature(symbol: &RocprofKernelSymbolV1) -> KernelSignatureV1 {
    KernelSignatureV1 {
        kernel_object: symbol.kernel_object,
        kernarg_size: symbol.kernarg_segment_size,
        kernarg_alignment: symbol.kernarg_segment_alignment,
        group_segment_size: symbol.group_segment_size,
        private_segment_size: symbol.private_segment_size,
    }
}

fn checked_signed_add(value: u64, delta: i64) -> Result<u64, PcSampleCodeObjectRelationErrorV1> {
    if delta >= 0 {
        value
            .checked_add(delta as u64)
            .ok_or(PcSampleCodeObjectRelationErrorV1::AddressOverflow)
    } else {
        value
            .checked_sub(delta.unsigned_abs())
            .ok_or(PcSampleCodeObjectRelationErrorV1::AddressOverflow)
    }
}

fn content_scheme(scheme: ContentSchemeV1) -> ContentIdentitySchemeV1 {
    match scheme {
        ContentSchemeV1::RawCanonicalSha256 => ContentIdentitySchemeV1::RawCanonicalSha256,
        ContentSchemeV1::DomainSeparatedSha256 => ContentIdentitySchemeV1::DomainSeparatedSha256,
    }
}

fn validate_content_identity(
    identity: ContentIdentityRecordV1,
) -> Result<(), PcSampleCodeObjectRelationErrorV1> {
    if identity.format_version == 0
        || identity.canonical_len == 0
        || identity.digest.as_bytes() == [0; 32]
    {
        return Err(PcSampleCodeObjectRelationErrorV1::InvalidArtifactIdentity);
    }
    Ok(())
}

pub fn encode_pc_sample_code_object_relation_v1(
    admitted: &AdmittedPcSampleCodeObjectRelationV1,
    capture_bytes: &[u8],
) -> Result<Vec<u8>, PcSampleCodeObjectRelationErrorV1> {
    admitted.relation.validate_against_capture(capture_bytes)?;
    let bytes = serde_json::to_vec(&admitted.relation)
        .map_err(|_| PcSampleCodeObjectRelationErrorV1::JsonEncode)?;
    if u64::try_from(bytes.len()).map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?
        > MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1
    {
        return Err(PcSampleCodeObjectRelationErrorV1::RelationTooLarge);
    }
    Ok(bytes)
}

pub fn decode_pc_sample_code_object_relation_v1(
    bytes: &[u8],
    capture_bytes: &[u8],
) -> Result<SemanticPcSampleCodeObjectRelationV1, PcSampleCodeObjectRelationErrorV1> {
    let len =
        u64::try_from(bytes.len()).map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?;
    if bytes.is_empty() || len > MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1 {
        return Err(PcSampleCodeObjectRelationErrorV1::RelationTooLarge);
    }
    let relation: SemanticPcSampleCodeObjectRelationV1 = parse_rocprof_json_document_v1(bytes)
        .map_err(|error| {
            map_rocprof_json_error_v1(error, PcSampleCodeObjectRelationErrorV1::JsonDecode)
        })?;
    relation.validate_against_capture(capture_bytes)?;
    if serde_json::to_vec(&relation).map_err(|_| PcSampleCodeObjectRelationErrorV1::JsonEncode)?
        != bytes
    {
        return Err(PcSampleCodeObjectRelationErrorV1::NonCanonicalEncoding);
    }
    Ok(relation)
}

fn map_rocprof_json_error_v1(
    error: ImportErrorV1,
    invalid: PcSampleCodeObjectRelationErrorV1,
) -> PcSampleCodeObjectRelationErrorV1 {
    match error {
        ImportErrorV1::AllocationFailure => PcSampleCodeObjectRelationErrorV1::AllocationFailure,
        _ => invalid,
    }
}

pub fn pc_sample_code_object_relation_content_identity_v1(
    bytes: &[u8],
    capture_bytes: &[u8],
) -> Result<ContentIdentityRecordV1, PcSampleCodeObjectRelationErrorV1> {
    let _ = decode_pc_sample_code_object_relation_v1(bytes, capture_bytes)?;
    let mut digest = Sha256::new();
    digest.update(PC_SAMPLE_CODE_OBJECT_RELATION_IDENTITY_DOMAIN_V1);
    digest.update(bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1,
        digest: CaptureIdentityV1::new(digest.finalize().into())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::IdentityFailure)?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| PcSampleCodeObjectRelationErrorV1::SizeOverflow)?,
    })
}

#[derive(Debug)]
pub enum PcSampleCodeObjectRelationErrorV1 {
    UnsupportedVersion(u16),
    Capture(PcSampleCaptureErrorV3),
    InvalidRocprofJson,
    CaptureSourceMismatch,
    StaleCapture,
    ArtifactClaimUnavailable,
    ArtifactSizeOutOfRange,
    InvalidArtifactIdentity,
    ArtifactSubstitution,
    InvalidHsacoArtifact,
    ArtifactLoadSubstitution,
    InvalidStructuredLoad,
    OverlappingStructuredLoads,
    ProcessMismatch,
    DeviceMismatch,
    InvalidRecordCatalog,
    InvalidSymbolDomain,
    InvalidClaims,
    NonCanonicalOrder,
    TooManyCodeObjectLoads,
    TooManyKernelSymbols,
    AddressOverflow,
    SizeOverflow,
    AllocationFailure,
    RelationTooLarge,
    IdentityFailure,
    NonCanonicalEncoding,
    JsonEncode,
    JsonDecode,
}

impl fmt::Display for PcSampleCodeObjectRelationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "semantic PC sample code-object relation rejected: {self:?}"
        )
    }
}

impl Error for PcSampleCodeObjectRelationErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &[u8] =
        include_bytes!("../tests/fixtures/rocprofv3-1.1-stochastic-pc-sampling.json");

    fn with_injected_rocprof_json_allocation_failure_v1<T>(operation: impl FnOnce() -> T) -> T {
        crate::INJECT_ROCPROF_JSON_ALLOCATION_FAILURE_V1.with(|inject| {
            assert!(
                inject
                    .replace(Some(crate::RocprofJsonAllocationInjectionSiteV1::Any))
                    .is_none(),
                "nested allocation injection"
            );
        });
        let result = operation();
        crate::INJECT_ROCPROF_JSON_ALLOCATION_FAILURE_V1.with(|inject| inject.set(None));
        result
    }

    fn allocation_test_capture(source: &[u8], artifact: &[u8]) -> Vec<u8> {
        let artifact_digest: [u8; 32] = Sha256::digest(artifact).into();
        let capture = import_rocprofv3_pc_sample_capture_v3(
            source,
            RocprofPcSampleCaptureBindingV3 {
                capture: RocprofCaptureBindingV1 {
                    kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(
                        OpaqueIdentityV1::new([1; 32]).unwrap(),
                        97,
                    )
                    .unwrap(),
                    artifact: Some(ArtifactClaimV1 {
                        identity: OpaqueIdentityV1::new(artifact_digest).unwrap(),
                        canonical_len: artifact.len() as u64,
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
        encode_pc_sample_capture_v3(&capture).unwrap()
    }

    fn unsupported_relation_bytes() -> Vec<u8> {
        let identity = ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: 1,
            digest: CaptureIdentityV1::new([1; 32]).unwrap(),
            canonical_len: 1,
        };
        serde_json::to_vec(&SemanticPcSampleCodeObjectRelationV1 {
            schema_version: PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1 + 1,
            source_identity: identity,
            capture_identity: identity,
            artifact_identity: identity,
            records: Vec::new(),
            symbol_domains: Vec::new(),
            claims: PcSampleCodeObjectRelationClaimsV1::NONE,
        })
        .unwrap()
    }

    fn structured_source() -> Vec<u8> {
        let mut value: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
        let process = &mut value["rocprofiler-sdk-tool"][0];
        process["code_objects"] = serde_json::json!([
            {"code_object_id":2,"agent_id":{"handle":18217},"load_base":1048576,"load_size":65536,"load_delta":1044480},
            {"code_object_id":3,"agent_id":{"handle":18217},"load_base":2097152,"load_size":65536,"load_delta":2093056}
        ]);
        process["kernel_symbols"] = serde_json::json!([
            {"size":80,"kernel_id":11,"code_object_id":2,"kernel_name":"vecadd","kernel_object":1052672,"kernarg_segment_size":16,"kernarg_segment_alignment":16,"group_segment_size":0,"private_segment_size":0,"formatted_kernel_name":"vecadd","demangled_kernel_name":"vecadd","truncated_kernel_name":"vecadd"},
            {"size":80,"kernel_id":12,"code_object_id":3,"kernel_name":"vecadd","kernel_object":2101248,"kernarg_segment_size":16,"kernarg_segment_alignment":16,"group_segment_size":0,"private_segment_size":0,"formatted_kernel_name":"vecadd","demangled_kernel_name":"vecadd","truncated_kernel_name":"vecadd"}
        ]);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn structured_catalog_uses_process_local_ids_and_rejects_native_overlap() {
        let source = structured_source();
        let document: RocprofRelationDocumentV1 = serde_json::from_slice(&source).unwrap();
        reject_overlapping_loads(&document).unwrap();
        assert_eq!(document.processes[0].code_objects.len(), 2);
        assert_eq!(document.processes[0].kernel_symbols.len(), 2);

        let mut conflicting_agent: serde_json::Value = serde_json::from_slice(&source).unwrap();
        conflicting_agent["rocprofiler-sdk-tool"][0]["code_objects"][0]["rocp_agent"] =
            serde_json::json!({"handle": 18218});
        let conflicting_agent: RocprofRelationDocumentV1 =
            serde_json::from_slice(&serde_json::to_vec(&conflicting_agent).unwrap()).unwrap();
        assert!(matches!(
            reject_overlapping_loads(&conflicting_agent),
            Err(PcSampleCodeObjectRelationErrorV1::InvalidStructuredLoad)
        ));

        let mut value: serde_json::Value = serde_json::from_slice(&source).unwrap();
        value["rocprofiler-sdk-tool"][0]["code_objects"][1]["load_base"] =
            serde_json::json!(1048600);
        let hostile: RocprofRelationDocumentV1 =
            serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(matches!(
            reject_overlapping_loads(&hostile),
            Err(PcSampleCodeObjectRelationErrorV1::OverlappingStructuredLoads)
        ));
    }

    #[test]
    fn signed_load_delta_and_symbol_signature_substitutions_are_exact() {
        assert_eq!(checked_signed_add(0x1000, 0x200).unwrap(), 0x1200);
        assert_eq!(checked_signed_add(0x1000, -0x200).unwrap(), 0xe00);
        assert!(matches!(
            checked_signed_add(0, -1),
            Err(PcSampleCodeObjectRelationErrorV1::AddressOverflow)
        ));
        let symbol = RocprofKernelSymbolV1 {
            kernel_id: 1,
            code_object_id: 2,
            kernel_object: 0x1100,
            kernarg_segment_size: 16,
            kernarg_segment_alignment: 16,
            group_segment_size: 0,
            private_segment_size: 0,
        };
        let signature = observed_signature(&symbol);
        let mut hostile = symbol;
        hostile.kernel_object += 4;
        assert_ne!(observed_signature(&hostile), signature);
    }

    #[test]
    fn catalog_preflight_rejects_cumulative_one_over_without_record_vectors() {
        let split = MAX_PC_SAMPLE_CODE_OBJECT_LOADS_V1 / 2 + 1;
        let process = || {
            serde_json::json!({
                "code_objects": vec![serde_json::Value::Null; split],
                "kernel_symbols": []
            })
        };
        let source = serde_json::to_vec(&serde_json::json!({
            "rocprofiler-sdk-tool": [process(), process()]
        }))
        .unwrap();
        assert!(source.len() < crate::MAX_IMPORT_SOURCE_BYTES_V1 as usize);
        assert!(matches!(
            validate_relation_source_catalogs(&source),
            Err(PcSampleCodeObjectRelationErrorV1::TooManyCodeObjectLoads)
        ));

        let split = MAX_PC_SAMPLE_KERNEL_SYMBOLS_V1 / 2 + 1;
        let process = || {
            serde_json::json!({
                "code_objects": [],
                "kernel_symbols": vec![serde_json::Value::Null; split]
            })
        };
        let source = serde_json::to_vec(&serde_json::json!({
            "rocprofiler-sdk-tool": [process(), process()]
        }))
        .unwrap();
        assert!(source.len() < crate::MAX_IMPORT_SOURCE_BYTES_V1 as usize);
        assert!(matches!(
            validate_relation_source_catalogs(&source),
            Err(PcSampleCodeObjectRelationErrorV1::TooManyKernelSymbols)
        ));
    }

    #[test]
    fn public_raw_source_admission_preserves_allocation_failure_and_recovers_same_thread() {
        let source = structured_source();
        let artifact = b"not an ELF code object";
        let capture_bytes = allocation_test_capture(&source, artifact);

        assert!(matches!(
            with_injected_rocprof_json_allocation_failure_v1(|| {
                admit_rocprofv3_pc_sample_code_object_relation_v1(
                    &source,
                    &capture_bytes,
                    artifact,
                    ImportLimitsV1::default(),
                )
            }),
            Err(PcSampleCodeObjectRelationErrorV1::AllocationFailure)
        ));
        crate::ROCPROF_JSON_PARSE_FAILURE_V1.with(|state| assert_eq!(state.get(), None));
        assert!(matches!(
            admit_rocprofv3_pc_sample_code_object_relation_v1(
                &source,
                &capture_bytes,
                artifact,
                ImportLimitsV1::default(),
            ),
            Err(PcSampleCodeObjectRelationErrorV1::InvalidHsacoArtifact)
        ));
    }

    #[test]
    fn direct_relation_source_parsers_clear_allocation_failure_for_same_thread_reuse() {
        let source = structured_source();

        assert!(matches!(
            with_injected_rocprof_json_allocation_failure_v1(|| {
                validate_relation_source_catalogs(&source)
            }),
            Err(PcSampleCodeObjectRelationErrorV1::AllocationFailure)
        ));
        crate::ROCPROF_JSON_PARSE_FAILURE_V1.with(|state| assert_eq!(state.get(), None));
        validate_relation_source_catalogs(&source).unwrap();

        assert!(matches!(
            with_injected_rocprof_json_allocation_failure_v1(|| {
                parse_relation_document_v1(&source)
            }),
            Err(PcSampleCodeObjectRelationErrorV1::AllocationFailure)
        ));
        crate::ROCPROF_JSON_PARSE_FAILURE_V1.with(|state| assert_eq!(state.get(), None));
        parse_relation_document_v1(&source).unwrap();
    }

    #[test]
    fn public_relation_decode_preserves_allocation_failure_and_recovers_same_thread() {
        let bytes = unsupported_relation_bytes();

        assert!(matches!(
            with_injected_rocprof_json_allocation_failure_v1(|| {
                decode_pc_sample_code_object_relation_v1(&bytes, b"unused capture")
            }),
            Err(PcSampleCodeObjectRelationErrorV1::AllocationFailure)
        ));
        crate::ROCPROF_JSON_PARSE_FAILURE_V1.with(|state| assert_eq!(state.get(), None));
        assert!(matches!(
            decode_pc_sample_code_object_relation_v1(&bytes, b"unused capture"),
            Err(PcSampleCodeObjectRelationErrorV1::UnsupportedVersion(version))
                if version == PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1 + 1
        ));
    }
}

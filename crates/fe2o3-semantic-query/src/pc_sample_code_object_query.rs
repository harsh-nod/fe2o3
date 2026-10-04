use std::error::Error;
use std::fmt;

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ImportLimitsV1, MAX_IMPORT_SOURCE_BYTES_V1,
    MAX_PC_SAMPLE_CAPTURE_BYTES_V3, MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1,
    PcSampleCodeObjectRelationClaimsV1, PcSampleCodeObjectRelationErrorV1,
    PcSampleCodeObjectRelationStatusV1, PcSampleCodeObjectRelationUnavailableReasonV1,
    SemanticPcSampleCaptureV3, SemanticPcSampleCodeObjectRelationV1,
    admit_rocprofv3_pc_sample_code_object_relation_v1, decode_pc_sample_capture_v3,
    decode_pc_sample_code_object_relation_v1, pc_sample_code_object_relation_content_identity_v1,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub const MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1: u64 = 64 * 1024 * 1024;
const PC_SAMPLE_KERNEL_SYMBOL_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.pc-sample.kernel-symbol.v1\0";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcSampleCodeObjectQueryLimitsV1 {
    max_source_bytes: u64,
    max_capture_bytes: u64,
    max_relation_bytes: u64,
    max_artifact_bytes: u64,
}

impl Default for PcSampleCodeObjectQueryLimitsV1 {
    fn default() -> Self {
        Self {
            max_source_bytes: MAX_IMPORT_SOURCE_BYTES_V1,
            max_capture_bytes: MAX_PC_SAMPLE_CAPTURE_BYTES_V3,
            max_relation_bytes: MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1,
            max_artifact_bytes: MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1,
        }
    }
}

impl PcSampleCodeObjectQueryLimitsV1 {
    pub fn new(
        source: u64,
        capture: u64,
        relation: u64,
        artifact: u64,
    ) -> Result<Self, PcSampleCodeObjectQueryErrorV1> {
        if source == 0
            || source > MAX_IMPORT_SOURCE_BYTES_V1
            || capture == 0
            || capture > MAX_PC_SAMPLE_CAPTURE_BYTES_V3
            || relation == 0
            || relation > MAX_PC_SAMPLE_CODE_OBJECT_RELATION_BYTES_V1
            || artifact == 0
            || artifact > MAX_PC_SAMPLE_CODE_OBJECT_QUERY_ARTIFACT_BYTES_V1
        {
            return Err(PcSampleCodeObjectQueryErrorV1::LimitOutOfRange);
        }
        Ok(Self {
            max_source_bytes: source,
            max_capture_bytes: capture,
            max_relation_bytes: relation,
            max_artifact_bytes: artifact,
        })
    }

    pub const fn max_source_bytes(self) -> u64 {
        self.max_source_bytes
    }

    pub const fn max_capture_bytes(self) -> u64 {
        self.max_capture_bytes
    }

    pub const fn max_relation_bytes(self) -> u64 {
        self.max_relation_bytes
    }

    pub const fn max_artifact_bytes(self) -> u64 {
        self.max_artifact_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PcSampleCodeObjectQueryUnavailableReasonV1 {
    UnknownSample,
    CaptureDispatchMismatch,
    RelativePcUnavailable,
    UnknownCodeObject,
    MissingStructuredLoad,
    AmbiguousStructuredLoad,
    MissingStructuredKernelSymbols,
    IncompleteStructuredKernelSymbols,
    UnalignedPc,
    OutsideLoadedCodeObject,
    OutsideKernelSymbol,
    AmbiguousKernelSymbol,
}

impl From<PcSampleCodeObjectRelationUnavailableReasonV1>
    for PcSampleCodeObjectQueryUnavailableReasonV1
{
    fn from(value: PcSampleCodeObjectRelationUnavailableReasonV1) -> Self {
        match value {
            PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredLoad => {
                Self::MissingStructuredLoad
            }
            PcSampleCodeObjectRelationUnavailableReasonV1::AmbiguousStructuredLoad => {
                Self::AmbiguousStructuredLoad
            }
            PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredKernelSymbols => {
                Self::MissingStructuredKernelSymbols
            }
            PcSampleCodeObjectRelationUnavailableReasonV1::IncompleteStructuredKernelSymbols => {
                Self::IncompleteStructuredKernelSymbols
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PcSampleResolvedSymbolPcV1 {
    pub capture_identity: ContentIdentityRecordV1,
    pub relation_identity: ContentIdentityRecordV1,
    pub artifact_identity: ContentIdentityRecordV1,
    pub sample_identity: Option<CaptureIdentityV1>,
    pub dispatch_identity: Option<CaptureIdentityV1>,
    pub process_index: u32,
    pub device_identity: CaptureIdentityV1,
    pub code_object_identity: CaptureIdentityV1,
    pub metadata_kernel_ordinal: u32,
    /// Relation-bound identity of the exact code-object symbol interval. This
    /// is deliberately independent of an untrusted or ambiguous symbol name.
    pub kernel_symbol_identity: CaptureIdentityV1,
    pub kernel_symbol_code_object_offset: u64,
    pub kernel_symbol_byte_len: u64,
    pub symbol_relative_pc: u64,
    pub from_stochastic_sample: bool,
    pub claims: PcSampleCodeObjectRelationClaimsV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
// Resolved query evidence stays fixed-size, Copy, and allocation-free.
#[allow(clippy::large_enum_variant)]
pub enum PcSampleCodeObjectQueryResultV1 {
    Resolved {
        pc: PcSampleResolvedSymbolPcV1,
    },
    Unavailable {
        reason: PcSampleCodeObjectQueryUnavailableReasonV1,
    },
}

/// Read-only exact query session. Opening replays admission from the native-
/// address-bearing rocprof source, but the session outputs and durable sidecar
/// expose neither those addresses nor any load/execution authority.
pub struct PcSampleCodeObjectQuerySessionV1 {
    capture: SemanticPcSampleCaptureV3,
    relation: SemanticPcSampleCodeObjectRelationV1,
    relation_identity: ContentIdentityRecordV1,
    sample_index: Vec<(CaptureIdentityV1, usize)>,
    dispatch_index: Vec<(CaptureIdentityV1, usize)>,
}

impl PcSampleCodeObjectQuerySessionV1 {
    pub fn open(
        source: &[u8],
        capture_bytes: &[u8],
        artifact_bytes: &[u8],
        relation_bytes: &[u8],
        limits: PcSampleCodeObjectQueryLimitsV1,
    ) -> Result<Self, PcSampleCodeObjectQueryErrorV1> {
        check_size(source, limits.max_source_bytes)?;
        check_size(capture_bytes, limits.max_capture_bytes)?;
        check_size(artifact_bytes, limits.max_artifact_bytes)?;
        check_size(relation_bytes, limits.max_relation_bytes)?;
        let decoded = decode_pc_sample_code_object_relation_v1(relation_bytes, capture_bytes)
            .map_err(PcSampleCodeObjectQueryErrorV1::Relation)?;
        let relation_identity =
            pc_sample_code_object_relation_content_identity_v1(relation_bytes, capture_bytes)
                .map_err(PcSampleCodeObjectQueryErrorV1::Relation)?;
        let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
            source,
            capture_bytes,
            artifact_bytes,
            ImportLimitsV1::new(limits.max_source_bytes)
                .map_err(|_| PcSampleCodeObjectQueryErrorV1::LimitOutOfRange)?,
        )
        .map_err(PcSampleCodeObjectQueryErrorV1::Relation)?;
        if admitted.relation() != &decoded {
            return Err(PcSampleCodeObjectQueryErrorV1::StaleOrSubstitutedRelation);
        }
        drop(admitted);
        let capture = decode_pc_sample_capture_v3(capture_bytes)
            .map_err(PcSampleCodeObjectQueryErrorV1::Capture)?;
        let mut sample_index = Vec::new();
        sample_index
            .try_reserve_exact(capture.samples.len())
            .map_err(|_| PcSampleCodeObjectQueryErrorV1::AllocationFailure)?;
        sample_index.extend(
            capture
                .samples
                .iter()
                .enumerate()
                .map(|(index, sample)| (sample.identity, index)),
        );
        sample_index.sort_unstable_by_key(|item| item.0);
        let mut dispatch_index = Vec::new();
        dispatch_index
            .try_reserve_exact(capture.dispatches.len())
            .map_err(|_| PcSampleCodeObjectQueryErrorV1::AllocationFailure)?;
        dispatch_index.extend(
            capture
                .dispatches
                .iter()
                .enumerate()
                .map(|(index, dispatch)| (dispatch.identity, index)),
        );
        dispatch_index.sort_unstable_by_key(|item| item.0);
        if sample_index.windows(2).any(|pair| pair[0].0 == pair[1].0)
            || dispatch_index.windows(2).any(|pair| pair[0].0 == pair[1].0)
            || capture.samples.iter().any(|sample| {
                let Some((_, dispatch_index_value)) = dispatch_index
                    .binary_search_by_key(&sample.dispatch_identity, |item| item.0)
                    .ok()
                    .and_then(|index| dispatch_index.get(index))
                else {
                    return true;
                };
                let Some(dispatch) = capture.dispatches.get(*dispatch_index_value) else {
                    return true;
                };
                let Some(code_object_identity) = sample.pc.code_object_identity else {
                    return false;
                };
                decoded
                    .records
                    .binary_search_by_key(&code_object_identity, |record| {
                        record.code_object_identity
                    })
                    .ok()
                    .and_then(|index| decoded.records.get(index))
                    .is_none_or(|record| {
                        record.process_index != dispatch.process_index
                            || record.device_identity != dispatch.device_identity
                    })
            })
        {
            return Err(PcSampleCodeObjectQueryErrorV1::CaptureIndexMismatch);
        }
        Ok(Self {
            capture,
            relation: decoded,
            relation_identity,
            sample_index,
            dispatch_index,
        })
    }

    pub fn lookup_sample(
        &self,
        sample_identity: CaptureIdentityV1,
    ) -> PcSampleCodeObjectQueryResultV1 {
        let Ok(position) = self
            .sample_index
            .binary_search_by_key(&sample_identity, |item| item.0)
        else {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnknownSample);
        };
        let Some(sample) = self
            .sample_index
            .get(position)
            .and_then(|(_, index)| self.capture.samples.get(*index))
        else {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnknownSample);
        };
        let (Some(code_object), Some(offset)) =
            (sample.pc.code_object_identity, sample.pc.code_object_offset)
        else {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::RelativePcUnavailable);
        };
        let Some(dispatch) = self
            .dispatch_index
            .binary_search_by_key(&sample.dispatch_identity, |item| item.0)
            .ok()
            .and_then(|index| self.dispatch_index.get(index))
            .and_then(|(_, index)| self.capture.dispatches.get(*index))
            .filter(|dispatch| dispatch.identity == sample.dispatch_identity)
        else {
            return unavailable(
                PcSampleCodeObjectQueryUnavailableReasonV1::CaptureDispatchMismatch,
            );
        };
        self.lookup(
            code_object,
            offset,
            Some((
                sample.identity,
                dispatch.identity,
                dispatch.process_index,
                dispatch.device_identity,
            )),
        )
    }

    pub fn lookup_code_object_pc(
        &self,
        code_object_identity: CaptureIdentityV1,
        code_object_offset: u64,
    ) -> PcSampleCodeObjectQueryResultV1 {
        self.lookup(code_object_identity, code_object_offset, None)
    }

    fn lookup(
        &self,
        code_object_identity: CaptureIdentityV1,
        code_object_offset: u64,
        sample_context: Option<(CaptureIdentityV1, CaptureIdentityV1, u32, CaptureIdentityV1)>,
    ) -> PcSampleCodeObjectQueryResultV1 {
        let Ok(record_index) = self
            .relation
            .records
            .binary_search_by_key(&code_object_identity, |record| record.code_object_identity)
        else {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnknownCodeObject);
        };
        let record = self.relation.records[record_index];
        if sample_context.is_some_and(|(_, _, process_index, device_identity)| {
            process_index != record.process_index || device_identity != record.device_identity
        }) {
            return unavailable(
                PcSampleCodeObjectQueryUnavailableReasonV1::CaptureDispatchMismatch,
            );
        }
        if let PcSampleCodeObjectRelationStatusV1::Unavailable(reason) = record.status {
            return unavailable(reason.into());
        }
        if !code_object_offset.is_multiple_of(4) {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnalignedPc);
        }
        if record
            .loaded_code_object_size
            .is_none_or(|size| code_object_offset >= size)
        {
            return unavailable(
                PcSampleCodeObjectQueryUnavailableReasonV1::OutsideLoadedCodeObject,
            );
        }
        let begin = self
            .relation
            .symbol_domains
            .partition_point(|domain| domain.code_object_identity < code_object_identity);
        let end = self
            .relation
            .symbol_domains
            .partition_point(|domain| domain.code_object_identity <= code_object_identity);
        let mut matches = self.relation.symbol_domains[begin..end]
            .iter()
            .filter(|domain| {
                domain
                    .code_object_offset
                    .checked_add(domain.byte_len)
                    .is_some_and(|domain_end| {
                        domain.code_object_offset <= code_object_offset
                            && code_object_offset < domain_end
                    })
            });
        let Some(domain) = matches.next() else {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::OutsideKernelSymbol);
        };
        if matches.next().is_some() {
            return unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::AmbiguousKernelSymbol);
        }
        PcSampleCodeObjectQueryResultV1::Resolved {
            pc: PcSampleResolvedSymbolPcV1 {
                capture_identity: self.relation.capture_identity,
                relation_identity: self.relation_identity,
                artifact_identity: self.relation.artifact_identity,
                sample_identity: sample_context.map(|context| context.0),
                dispatch_identity: sample_context.map(|context| context.1),
                process_index: record.process_index,
                device_identity: record.device_identity,
                code_object_identity,
                metadata_kernel_ordinal: domain.metadata_kernel_ordinal,
                kernel_symbol_identity: kernel_symbol_identity(self.relation_identity, *domain),
                kernel_symbol_code_object_offset: domain.code_object_offset,
                kernel_symbol_byte_len: domain.byte_len,
                symbol_relative_pc: code_object_offset - domain.code_object_offset,
                from_stochastic_sample: sample_context.is_some(),
                claims: self.relation.claims,
            },
        }
    }
}

fn kernel_symbol_identity(
    relation: ContentIdentityRecordV1,
    domain: fe2o3_semantic_import::PcSampleKernelSymbolDomainV1,
) -> CaptureIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(PC_SAMPLE_KERNEL_SYMBOL_IDENTITY_DOMAIN_V1);
    digest.update(relation.digest.as_bytes());
    digest.update(relation.canonical_len.to_le_bytes());
    digest.update(domain.code_object_identity.as_bytes());
    digest.update(domain.metadata_kernel_ordinal.to_le_bytes());
    digest.update(domain.code_object_offset.to_le_bytes());
    digest.update(domain.byte_len.to_le_bytes());
    CaptureIdentityV1::new(digest.finalize().into()).expect("domain-separated SHA-256 is nonzero")
}

fn check_size(bytes: &[u8], limit: u64) -> Result<(), PcSampleCodeObjectQueryErrorV1> {
    let len =
        u64::try_from(bytes.len()).map_err(|_| PcSampleCodeObjectQueryErrorV1::SizeOverflow)?;
    if len == 0 || len > limit {
        return Err(PcSampleCodeObjectQueryErrorV1::InputTooLarge);
    }
    Ok(())
}

const fn unavailable(
    reason: PcSampleCodeObjectQueryUnavailableReasonV1,
) -> PcSampleCodeObjectQueryResultV1 {
    PcSampleCodeObjectQueryResultV1::Unavailable { reason }
}

#[derive(Debug)]
pub enum PcSampleCodeObjectQueryErrorV1 {
    LimitOutOfRange,
    InputTooLarge,
    SizeOverflow,
    AllocationFailure,
    CaptureIndexMismatch,
    Capture(fe2o3_semantic_import::PcSampleCaptureErrorV3),
    Relation(PcSampleCodeObjectRelationErrorV1),
    StaleOrSubstitutedRelation,
}

impl fmt::Display for PcSampleCodeObjectQueryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "semantic PC sample code-object query rejected: {self:?}"
        )
    }
}

impl Error for PcSampleCodeObjectQueryErrorV1 {}

#[cfg(test)]
#[path = "../../fe2o3-semantic-import/tests/fixtures/pc_sample_code_object_hsaco_fixture.rs"]
mod exact_hsaco_fixture;

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_semantic_import::{
        ArtifactClaimV1, CaptureIdentityV1, ContentSchemeV1, PcSampleCodeObjectRelationRecordV1,
        PcSampleKernelSymbolDomainV1, RocprofCaptureBindingV1, RocprofPcSampleCaptureBindingV3,
        TruthOriginV1, admit_rocprofv3_pc_sample_code_object_relation_v1,
        encode_pc_sample_capture_v3, encode_pc_sample_code_object_relation_v1,
        import_rocprofv3_pc_sample_capture_v3, pc_sample_capture_content_identity_v3,
    };
    use fe2o3_semantic_trace::{KernelIrIdentityClaimV1, OpaqueIdentityV1, WaveWidthV1};
    use sha2::{Digest, Sha256};

    const SOURCE: &[u8] = include_bytes!(
        "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-stochastic-pc-sampling.json"
    );

    fn opaque(byte: u8) -> OpaqueIdentityV1 {
        OpaqueIdentityV1::new([byte; 32]).unwrap()
    }

    fn claims() -> PcSampleCodeObjectRelationClaimsV1 {
        PcSampleCodeObjectRelationClaimsV1 {
            retains_native_addresses: false,
            grants_load_or_execution_authority: false,
            claims_runtime_loaded_bytes_equal_artifact: false,
            claims_complete_code_object_lifetime: false,
            identifies_a_live_pc: false,
            claims_complete_sample_coverage: false,
            claims_complete_instruction_history: false,
            claims_schedule_correlation: false,
            claims_source_attribution: false,
        }
    }

    fn session() -> PcSampleCodeObjectQuerySessionV1 {
        let capture = import_rocprofv3_pc_sample_capture_v3(
            SOURCE,
            RocprofPcSampleCaptureBindingV3 {
                capture: RocprofCaptureBindingV1 {
                    kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97)
                        .unwrap(),
                    artifact: Some(ArtifactClaimV1 {
                        identity: opaque(2),
                        canonical_len: 4096,
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
        let capture_bytes = fe2o3_semantic_import::encode_pc_sample_capture_v3(&capture).unwrap();
        let first_code_object = capture.code_objects[0].identity;
        let mut records: Vec<_> = capture
            .code_objects
            .iter()
            .map(|code_object| {
                let sample = capture
                    .samples
                    .iter()
                    .find(|sample| sample.pc.code_object_identity == Some(code_object.identity))
                    .unwrap();
                let dispatch = capture
                    .dispatches
                    .iter()
                    .find(|dispatch| dispatch.identity == sample.dispatch_identity)
                    .unwrap();
                PcSampleCodeObjectRelationRecordV1 {
                    code_object_identity: code_object.identity,
                    source_code_object_ordinal: code_object.source_code_object_ordinal,
                    process_index: dispatch.process_index,
                    device_identity: dispatch.device_identity,
                    status: if code_object.identity == first_code_object {
                        PcSampleCodeObjectRelationStatusV1::ExactDeclaredArtifactStructure
                    } else {
                        PcSampleCodeObjectRelationStatusV1::Unavailable(
                            PcSampleCodeObjectRelationUnavailableReasonV1::MissingStructuredLoad,
                        )
                    },
                    loaded_code_object_size: (code_object.identity == first_code_object)
                        .then_some(16_384),
                }
            })
            .collect();
        records.sort_by_key(|record| record.code_object_identity);
        let relation = SemanticPcSampleCodeObjectRelationV1 {
            schema_version: fe2o3_semantic_import::PC_SAMPLE_CODE_OBJECT_RELATION_SCHEMA_VERSION_V1,
            source_identity: capture.runs[0].source,
            capture_identity: pc_sample_capture_content_identity_v3(&capture_bytes).unwrap(),
            artifact_identity: ContentIdentityRecordV1 {
                scheme: ContentSchemeV1::RawCanonicalSha256,
                format_version: 1,
                digest: CaptureIdentityV1::new([2; 32]).unwrap(),
                canonical_len: 4096,
            },
            records,
            symbol_domains: vec![PcSampleKernelSymbolDomainV1 {
                code_object_identity: first_code_object,
                metadata_kernel_ordinal: 0,
                code_object_offset: 7_936,
                byte_len: 128,
            }],
            claims: claims(),
        };
        let mut sample_index: Vec<_> = capture
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| (sample.identity, index))
            .collect();
        sample_index.sort_unstable_by_key(|item| item.0);
        let mut dispatch_index: Vec<_> = capture
            .dispatches
            .iter()
            .enumerate()
            .map(|(index, dispatch)| (dispatch.identity, index))
            .collect();
        dispatch_index.sort_unstable_by_key(|item| item.0);
        PcSampleCodeObjectQuerySessionV1 {
            capture,
            relation,
            relation_identity: ContentIdentityRecordV1 {
                scheme: ContentSchemeV1::DomainSeparatedSha256,
                format_version: 1,
                digest: CaptureIdentityV1::new([3; 32]).unwrap(),
                canonical_len: 1024,
            },
            sample_index,
            dispatch_index,
        }
    }

    #[test]
    fn forward_and_reverse_queries_return_only_symbol_relative_pc() {
        let session = session();
        let sample = session.capture.samples[0];
        let PcSampleCodeObjectQueryResultV1::Resolved { pc } =
            session.lookup_sample(sample.identity)
        else {
            panic!("expected exact sampled symbol PC");
        };
        let dispatch = session
            .capture
            .dispatches
            .iter()
            .find(|dispatch| dispatch.identity == sample.dispatch_identity)
            .unwrap();
        assert_eq!(pc.sample_identity, Some(sample.identity));
        assert_eq!(pc.dispatch_identity, Some(dispatch.identity));
        assert_eq!(pc.process_index, dispatch.process_index);
        assert_eq!(pc.device_identity, dispatch.device_identity);
        assert_eq!(pc.symbol_relative_pc, 24);
        assert!(pc.from_stochastic_sample);
        assert!(!pc.claims.retains_native_addresses);
        assert!(!pc.claims.grants_load_or_execution_authority);
        assert!(!pc.claims.claims_runtime_loaded_bytes_equal_artifact);
        assert!(!pc.claims.claims_complete_code_object_lifetime);
        assert!(!pc.claims.claims_complete_sample_coverage);
        assert!(!pc.claims.claims_schedule_correlation);
        assert!(!pc.claims.claims_source_attribution);
        assert_eq!(
            session.lookup_code_object_pc(sample.pc.code_object_identity.unwrap(), 7_960),
            PcSampleCodeObjectQueryResultV1::Resolved {
                pc: PcSampleResolvedSymbolPcV1 {
                    sample_identity: None,
                    dispatch_identity: None,
                    from_stochastic_sample: false,
                    ..pc
                }
            }
        );
        let serialized = serde_json::to_value(pc).unwrap();
        assert_eq!(
            serialized["dispatch_identity"],
            serde_json::to_value(dispatch.identity).unwrap()
        );
        assert_eq!(
            serialized["device_identity"],
            serde_json::to_value(dispatch.device_identity).unwrap()
        );
    }

    #[test]
    fn unavailable_and_ambiguous_pc_states_are_not_guessed() {
        let mut session = session();
        let first = session.capture.samples[0];
        let second_object = session.capture.code_objects[1].identity;
        assert_eq!(
            session.lookup_code_object_pc(first.pc.code_object_identity.unwrap(), 7_961),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnalignedPc)
        );
        assert_eq!(
            session.lookup_code_object_pc(first.pc.code_object_identity.unwrap(), 8_100),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::OutsideKernelSymbol)
        );
        assert_eq!(
            session.lookup_code_object_pc(first.pc.code_object_identity.unwrap(), 20_000),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::OutsideLoadedCodeObject)
        );
        assert_eq!(
            session.lookup_code_object_pc(second_object, 9_216),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::MissingStructuredLoad)
        );
        assert_eq!(
            session.lookup_sample(session.capture.samples[4].identity),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::RelativePcUnavailable)
        );
        assert_eq!(
            session.lookup_sample(CaptureIdentityV1::new([99; 32]).unwrap()),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnknownSample)
        );
        assert_eq!(
            session.lookup_code_object_pc(CaptureIdentityV1::new([98; 32]).unwrap(), 0),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnknownCodeObject)
        );

        session
            .relation
            .symbol_domains
            .push(PcSampleKernelSymbolDomainV1 {
                code_object_identity: first.pc.code_object_identity.unwrap(),
                metadata_kernel_ordinal: 1,
                code_object_offset: 7_952,
                byte_len: 64,
            });
        session.relation.symbol_domains.sort_unstable();
        assert_eq!(
            session.lookup_sample(first.identity),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::AmbiguousKernelSymbol)
        );
    }

    #[test]
    fn sampled_dispatch_consistency_fails_closed() {
        let mut session = session();
        let sample = session.capture.samples[0];
        let position = session
            .dispatch_index
            .binary_search_by_key(&sample.dispatch_identity, |item| item.0)
            .unwrap();
        session.dispatch_index[position].1 = 1;
        assert_eq!(
            session.lookup_sample(sample.identity),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::CaptureDispatchMismatch)
        );
    }

    #[test]
    fn query_limits_are_closed_and_bounded() {
        assert!(PcSampleCodeObjectQueryLimitsV1::default().max_artifact_bytes > 0);
        assert!(matches!(
            PcSampleCodeObjectQueryLimitsV1::new(0, 1, 1, 1),
            Err(PcSampleCodeObjectQueryErrorV1::LimitOutOfRange)
        ));
        assert_eq!(TruthOriginV1::Observed, session().capture.samples[0].origin);
    }

    #[test]
    fn official_two_kernel_relation_reopens_and_queries_exact_boundaries() {
        let artifact = exact_hsaco_fixture::exact_sparse_two_kernel_hsaco_v1();
        let source = exact_hsaco_fixture::official_rocprof_source_v1(&artifact);
        let digest: [u8; 32] = Sha256::digest(&artifact.bytes).into();
        let capture = import_rocprofv3_pc_sample_capture_v3(
            &source,
            RocprofPcSampleCaptureBindingV3 {
                capture: RocprofCaptureBindingV1 {
                    kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97)
                        .unwrap(),
                    artifact: Some(ArtifactClaimV1 {
                        identity: OpaqueIdentityV1::new(digest).unwrap(),
                        canonical_len: artifact.bytes.len() as u64,
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
        let capture_bytes = encode_pc_sample_capture_v3(&capture).unwrap();
        let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture_bytes,
            &artifact.bytes,
            ImportLimitsV1::default(),
        )
        .unwrap();
        let relation_bytes =
            encode_pc_sample_code_object_relation_v1(&admitted, &capture_bytes).unwrap();
        let session = PcSampleCodeObjectQuerySessionV1::open(
            &source,
            &capture_bytes,
            &artifact.bytes,
            &relation_bytes,
            PcSampleCodeObjectQueryLimitsV1::default(),
        )
        .unwrap();

        let sample = capture.samples[2];
        let code_object_identity = sample.pc.code_object_identity.unwrap();
        let domain = admitted
            .relation()
            .symbol_domains
            .iter()
            .find(|domain| {
                domain.code_object_identity == code_object_identity
                    && domain.metadata_kernel_ordinal == 1
            })
            .copied()
            .unwrap();
        let PcSampleCodeObjectQueryResultV1::Resolved { pc } =
            session.lookup_sample(sample.identity)
        else {
            panic!("official-shaped sampled PC must resolve");
        };
        assert_eq!(pc.sample_identity, Some(sample.identity));
        assert_eq!(pc.dispatch_identity, Some(sample.dispatch_identity));
        let dispatch = capture
            .dispatches
            .iter()
            .find(|dispatch| dispatch.identity == sample.dispatch_identity)
            .unwrap();
        assert_eq!(pc.process_index, dispatch.process_index);
        assert_eq!(pc.device_identity, dispatch.device_identity);
        assert_eq!(pc.symbol_relative_pc, 0);
        assert!(matches!(
            session.lookup_code_object_pc(code_object_identity, domain.code_object_offset),
            PcSampleCodeObjectQueryResultV1::Resolved {
                pc: PcSampleResolvedSymbolPcV1 {
                    symbol_relative_pc: 0,
                    dispatch_identity: None,
                    ..
                }
            }
        ));
        assert!(matches!(
            session.lookup_code_object_pc(
                code_object_identity,
                domain.code_object_offset + domain.byte_len - 4
            ),
            PcSampleCodeObjectQueryResultV1::Resolved {
                pc: PcSampleResolvedSymbolPcV1 {
                    symbol_relative_pc,
                    ..
                }
            } if symbol_relative_pc == domain.byte_len - 4
        ));
        let first_domain = admitted
            .relation()
            .symbol_domains
            .iter()
            .find(|candidate| {
                candidate.code_object_identity == code_object_identity
                    && candidate.metadata_kernel_ordinal == 0
            })
            .unwrap();
        assert!(
            first_domain.code_object_offset + first_domain.byte_len < artifact.memory_size,
            "the symbol-exclusive-end check must remain inside the loaded code object"
        );
        assert_eq!(
            session.lookup_code_object_pc(
                code_object_identity,
                first_domain.code_object_offset + first_domain.byte_len
            ),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::OutsideKernelSymbol)
        );
        assert_eq!(
            session.lookup_code_object_pc(
                code_object_identity,
                domain.code_object_offset + domain.byte_len
            ),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::OutsideLoadedCodeObject)
        );
        assert_eq!(
            session.lookup_code_object_pc(code_object_identity, domain.code_object_offset + 1),
            unavailable(PcSampleCodeObjectQueryUnavailableReasonV1::UnalignedPc)
        );

        let mut stale_artifact = artifact.bytes.clone();
        *stale_artifact.last_mut().unwrap() ^= 1;
        assert!(matches!(
            PcSampleCodeObjectQuerySessionV1::open(
                &source,
                &capture_bytes,
                &stale_artifact,
                &relation_bytes,
                PcSampleCodeObjectQueryLimitsV1::default()
            ),
            Err(PcSampleCodeObjectQueryErrorV1::Relation(
                PcSampleCodeObjectRelationErrorV1::ArtifactSubstitution
            ))
        ));

        let mut stale_relation = admitted.relation().clone();
        let stale_domain = stale_relation
            .symbol_domains
            .iter_mut()
            .find(|candidate| **candidate == domain)
            .unwrap();
        stale_domain.code_object_offset += 4;
        stale_domain.byte_len -= 4;
        let stale_relation_bytes = serde_json::to_vec(&stale_relation).unwrap();
        assert!(matches!(
            PcSampleCodeObjectQuerySessionV1::open(
                &source,
                &capture_bytes,
                &artifact.bytes,
                &stale_relation_bytes,
                PcSampleCodeObjectQueryLimitsV1::default()
            ),
            Err(PcSampleCodeObjectQueryErrorV1::StaleOrSubstitutedRelation)
        ));
    }

    #[test]
    fn process_local_numeric_ids_preserve_context_in_both_query_directions() {
        let artifact = exact_hsaco_fixture::exact_sparse_two_kernel_hsaco_v1();
        let mut source: serde_json::Value =
            serde_json::from_slice(&exact_hsaco_fixture::official_rocprof_source_v1(&artifact))
                .unwrap();
        let mut second_process = source["rocprofiler-sdk-tool"][0].clone();
        second_process["metadata"]["pid"] = serde_json::json!(41_053);
        source["rocprofiler-sdk-tool"]
            .as_array_mut()
            .unwrap()
            .push(second_process);
        let source = serde_json::to_vec(&source).unwrap();
        let digest: [u8; 32] = Sha256::digest(&artifact.bytes).into();
        let capture = import_rocprofv3_pc_sample_capture_v3(
            &source,
            RocprofPcSampleCaptureBindingV3 {
                capture: RocprofCaptureBindingV1 {
                    kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97)
                        .unwrap(),
                    artifact: Some(ArtifactClaimV1 {
                        identity: OpaqueIdentityV1::new(digest).unwrap(),
                        canonical_len: artifact.bytes.len() as u64,
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
        let capture_bytes = encode_pc_sample_capture_v3(&capture).unwrap();
        let admitted = admit_rocprofv3_pc_sample_code_object_relation_v1(
            &source,
            &capture_bytes,
            &artifact.bytes,
            ImportLimitsV1::default(),
        )
        .unwrap();
        let relation_bytes =
            encode_pc_sample_code_object_relation_v1(&admitted, &capture_bytes).unwrap();
        let session = PcSampleCodeObjectQuerySessionV1::open(
            &source,
            &capture_bytes,
            &artifact.bytes,
            &relation_bytes,
            PcSampleCodeObjectQueryLimitsV1::default(),
        )
        .unwrap();

        let mut observed = Vec::new();
        for process_index in [0_u32, 1] {
            let dispatch = capture
                .dispatches
                .iter()
                .find(|dispatch| dispatch.process_index == process_index)
                .unwrap();
            let sample = capture
                .samples
                .iter()
                .find(|sample| {
                    sample.dispatch_identity == dispatch.identity
                        && sample.pc.code_object_identity.is_some()
                })
                .unwrap();
            let code_object_identity = sample.pc.code_object_identity.unwrap();
            let PcSampleCodeObjectQueryResultV1::Resolved { pc: forward } =
                session.lookup_sample(sample.identity)
            else {
                panic!("process-local sampled PC must resolve");
            };
            assert_eq!(forward.process_index, process_index);
            assert_eq!(forward.device_identity, dispatch.device_identity);
            assert_eq!(forward.dispatch_identity, Some(dispatch.identity));

            let PcSampleCodeObjectQueryResultV1::Resolved { pc: reverse } = session
                .lookup_code_object_pc(code_object_identity, sample.pc.code_object_offset.unwrap())
            else {
                panic!("process-local reverse PC must resolve");
            };
            assert_eq!(reverse.process_index, process_index);
            assert_eq!(reverse.device_identity, dispatch.device_identity);
            assert_eq!(reverse.sample_identity, None);
            assert_eq!(reverse.dispatch_identity, None);
            observed.push((code_object_identity, dispatch.device_identity));
        }
        assert_ne!(observed[0].0, observed[1].0);
        assert_ne!(observed[0].1, observed[1].1);
    }
}

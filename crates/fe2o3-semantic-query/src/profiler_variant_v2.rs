//! Exact, bounded cross-treatment source/MIR/KIR/ISA comparison.
//!
//! Variant V2 composes two admitted Variant V1 treatments with optional PC
//! sample or decoded-ATT source/ISA evidence. It reports positive
//! co-observations only: sampled or incomplete evidence never establishes
//! absence, addition, removal, causality, or superiority.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, DecodedAttCoverageV1,
    PcSampleCaptureCoverageV3, TruthOriginV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    DecodedAttSourceIsaLimitsV1, DecodedAttSourceIsaPageRequestV1,
    DecodedAttSourceIsaQueryResultV1, DecodedAttSourceIsaSessionV1, PcSourceIsaAttributionItemV1,
    PcSourceIsaAttributionStateV1, PcSourceIsaLimitsV1, PcSourceIsaPageRequestV1,
    PcSourceIsaQueryResultV1, PcSourceIsaScanSummaryV1, PcSourceIsaSessionV1,
    ProfilerVariantComparisonRequestV1, ProfilerVariantComparisonV1, ProfilerVariantErrorV1,
    ProfilerVariantTreatmentInputV1, build_profiler_variant_request_v1,
    compare_profiler_variants_v1,
};

pub const PROFILER_VARIANT_SCHEMA_VERSION_V2: u16 = 2;
pub const MAX_PROFILER_VARIANT_SELECTORS_V2: usize = 64;
pub const MAX_PROFILER_VARIANT_OCCURRENCES_V2: usize = 512;
pub const MAX_PROFILER_VARIANT_RESULT_BYTES_V2: u64 = 4 * 1024 * 1024;

const EVIDENCE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.profiler-variant.evidence.v2\0";
const SCHEDULE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.profiler-variant.semantic-schedule.v2\0";
const REQUEST_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.profiler-variant.request.v2\0";
const OCCURRENCE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.profiler-variant.occurrence.v2\0";
const CHANGE_IDENTITY_DOMAIN_V2: &[u8] = b"fe2o3.profiler-variant.change.v2\0";

#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantPcSourceIsaEvidenceV2<'a> {
    pub source: &'a [u8],
    pub relation: &'a [u8],
    pub characteristic: &'a [u8],
    pub sample_identities: &'a [CaptureIdentityV1],
}

#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantDecodedAttSourceIsaEvidenceV2<'a> {
    pub interchange: &'a [u8],
    pub characteristic: &'a [u8],
    pub code_object_identity: CaptureIdentityV1,
    pub record_identities: &'a [CaptureIdentityV1],
}

#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantTreatmentInputV2<'a> {
    pub treatment: ProfilerVariantTreatmentInputV1<'a>,
    pub pc_source_isa: Option<ProfilerVariantPcSourceIsaEvidenceV2<'a>>,
    pub decoded_att_source_isa: Option<ProfilerVariantDecodedAttSourceIsaEvidenceV2<'a>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantComparisonRequestV2 {
    pub schema_version: u16,
    pub comparison_v1: ProfilerVariantComparisonRequestV1,
    pub baseline_evidence: ContentIdentityRecordV1,
    pub candidate_evidence: ContentIdentityRecordV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantObservationKindV2 {
    PcSample,
    DecodedAtt,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantChangeAxisV2 {
    SemanticOperation,
    NeutralKir,
    TargetKir,
    CompilerHandoffLlvm,
    IsaInterval,
    Transformation,
    Classification,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantCausalUnavailableReasonV2 {
    PositiveCoObservationsDoNotProveCausation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantScheduleBindingV2 {
    pub origin: TruthOriginV1,
    pub content_identity: ContentIdentityRecordV1,
    pub semantic_schedule_identity: ContentIdentityRecordV1,
    pub semantics: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfilerVariantCoverageV2 {
    PcSample { coverage: PcSampleCaptureCoverageV3 },
    DecodedAtt { coverage: DecodedAttCoverageV1 },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantEvidenceBindingV2 {
    pub observation_kind: ProfilerVariantObservationKindV2,
    pub binding_identity: CaptureIdentityV1,
    pub artifact_identity: ContentIdentityRecordV1,
    pub characteristic_identity: CaptureIdentityV1,
    pub characteristic_scan: PcSourceIsaScanSummaryV1,
    pub selector_count: u16,
    pub occurrence_count: u16,
    pub coverage: ProfilerVariantCoverageV2,
    pub absence_semantics: String,
    pub evidence_ids: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantSourceSiteV2 {
    pub source_node_identity: CaptureIdentityV1,
    pub file_identity: CaptureIdentityV1,
    pub byte_start: u64,
    pub byte_end: u64,
    pub mir_node_identity: CaptureIdentityV1,
    pub mir_body_ordinal: u64,
    pub mir_block_ordinal: u64,
    pub mir_statement_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantKirCoordinateV2 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub operation_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantLlvmCoordinateV2 {
    pub function_ordinal: u64,
    pub block_ordinal: u64,
    pub instruction_ordinal: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantIsaIntervalV2 {
    pub kernel_ordinal: u64,
    pub symbol_relative_start: u64,
    pub symbol_relative_end: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantOccurrenceV2 {
    pub occurrence_identity: CaptureIdentityV1,
    pub observation_kind: ProfilerVariantObservationKindV2,
    pub selector_identity: CaptureIdentityV1,
    pub stable_source_mir_site: Option<ProfilerVariantSourceSiteV2>,
    pub semantic_operation_identity: CaptureIdentityV1,
    pub neutral_kir_node_identity: Option<CaptureIdentityV1>,
    pub neutral_kir: Option<ProfilerVariantKirCoordinateV2>,
    pub target_kir: ProfilerVariantKirCoordinateV2,
    pub compiler_handoff_llvm: ProfilerVariantLlvmCoordinateV2,
    pub isa: ProfilerVariantIsaIntervalV2,
    pub category_code: u16,
    pub kind_code: u16,
    pub record_kind_code: u8,
    pub transformation_code: u8,
    pub evidence_ids: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantChangedOccurrenceV2 {
    pub change_identity: CaptureIdentityV1,
    pub observation_kind: ProfilerVariantObservationKindV2,
    pub stable_source_mir_site: ProfilerVariantSourceSiteV2,
    pub baseline_occurrence_identity: CaptureIdentityV1,
    pub candidate_occurrence_identity: CaptureIdentityV1,
    pub changed_axes: Vec<ProfilerVariantChangeAxisV2>,
    pub evidence_ids: Vec<CaptureIdentityV1>,
    pub interpretation: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantUnavailableKindV2 {
    ComparisonAxesNotExact,
    BaselineCorrelationEvidenceMissing,
    CandidateCorrelationEvidenceMissing,
    StableSourceMirSiteUnavailable,
    UnmatchedObservationCannotEstablishAdditionOrRemoval,
    ProfilerKirToCharacteristicKirBridgeUnavailable,
    CharacteristicProducerAuthentication,
    CausalAttribution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantUnavailableV2 {
    pub kind: ProfilerVariantUnavailableKindV2,
    pub origin: TruthOriginV1,
    pub reason: String,
    pub evidence_ids: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantComparisonV2 {
    pub schema_version: u16,
    pub request_identity: ContentIdentityRecordV1,
    pub comparison_v1: ProfilerVariantComparisonV1,
    pub baseline_schedule: ProfilerVariantScheduleBindingV2,
    pub candidate_schedule: ProfilerVariantScheduleBindingV2,
    pub baseline_evidence: Vec<ProfilerVariantEvidenceBindingV2>,
    pub candidate_evidence: Vec<ProfilerVariantEvidenceBindingV2>,
    pub baseline_occurrences: Vec<ProfilerVariantOccurrenceV2>,
    pub candidate_occurrences: Vec<ProfilerVariantOccurrenceV2>,
    pub changed_occurrences: Vec<ProfilerVariantChangedOccurrenceV2>,
    pub unavailable: Vec<ProfilerVariantUnavailableV2>,
    pub causal_attribution: ProfilerVariantCausalUnavailableReasonV2,
    pub ranking_policy: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProfilerVariantErrorV2 {
    VariantV1Admission(ProfilerVariantErrorV1),
    InvalidRequest,
    RequestMismatch,
    EvidenceTooLarge,
    DuplicateSelector,
    MissingPcCapture,
    PcEvidenceAdmission,
    DecodedAttEvidenceAdmission,
    SelectorUnavailableOrForeign,
    AmbiguousAttribution,
    ResultTooLarge,
    IdentityFailure,
}

impl fmt::Display for ProfilerVariantErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "profiler Variant V2 evidence rejected: {self:?}")
    }
}

impl Error for ProfilerVariantErrorV2 {}

pub fn build_profiler_variant_request_v2(
    semantic_workload: &[u8],
    baseline: ProfilerVariantTreatmentInputV2<'_>,
    candidate: ProfilerVariantTreatmentInputV2<'_>,
) -> Result<ProfilerVariantComparisonRequestV2, ProfilerVariantErrorV2> {
    validate_selectors(baseline)?;
    validate_selectors(candidate)?;
    Ok(ProfilerVariantComparisonRequestV2 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V2,
        comparison_v1: build_profiler_variant_request_v1(
            semantic_workload,
            baseline.treatment.manifest,
            candidate.treatment.manifest,
        )
        .map_err(ProfilerVariantErrorV2::VariantV1Admission)?,
        baseline_evidence: treatment_evidence_identity(baseline)?,
        candidate_evidence: treatment_evidence_identity(candidate)?,
    })
}

pub fn compare_profiler_variants_v2(
    request: ProfilerVariantComparisonRequestV2,
    baseline: ProfilerVariantTreatmentInputV2<'_>,
    candidate: ProfilerVariantTreatmentInputV2<'_>,
) -> Result<ProfilerVariantComparisonV2, ProfilerVariantErrorV2> {
    if request.schema_version != PROFILER_VARIANT_SCHEMA_VERSION_V2 {
        return Err(ProfilerVariantErrorV2::InvalidRequest);
    }
    validate_selectors(baseline)?;
    validate_selectors(candidate)?;
    if treatment_evidence_identity(baseline)? != request.baseline_evidence
        || treatment_evidence_identity(candidate)? != request.candidate_evidence
    {
        return Err(ProfilerVariantErrorV2::RequestMismatch);
    }
    let comparison_v1 = compare_profiler_variants_v1(
        request.comparison_v1,
        baseline.treatment,
        candidate.treatment,
    )
    .map_err(ProfilerVariantErrorV2::VariantV1Admission)?;
    let request_identity = request_identity(request)?;
    let baseline_schedule = schedule_binding(
        comparison_v1.baseline_treatment.schedule,
        baseline.treatment.schedule,
    )?;
    let candidate_schedule = schedule_binding(
        comparison_v1.candidate_treatment.schedule,
        candidate.treatment.schedule,
    )?;

    let mut unavailable_facts = Vec::new();
    let (baseline_evidence, baseline_occurrences) = admit_correlation_evidence(
        baseline,
        comparison_v1.baseline_treatment.artifact,
        &mut unavailable_facts,
        true,
    )?;
    let (candidate_evidence, candidate_occurrences) = admit_correlation_evidence(
        candidate,
        comparison_v1.candidate_treatment.artifact,
        &mut unavailable_facts,
        false,
    )?;

    let changed_occurrences = if comparison_v1.comparable {
        compare_occurrences(
            &baseline_occurrences,
            &candidate_occurrences,
            &mut unavailable_facts,
        )?
    } else {
        unavailable_facts.push(unavailable(
            ProfilerVariantUnavailableKindV2::ComparisonAxesNotExact,
            "Variant V1 environment, collector, device, workload, or launch axes are not all exact",
            Vec::new(),
        ));
        Vec::new()
    };

    if baseline_evidence.is_empty() {
        unavailable_facts.push(unavailable(
            ProfilerVariantUnavailableKindV2::BaselineCorrelationEvidenceMissing,
            "no PC/source-ISA or decoded-ATT/source-ISA evidence was supplied for the baseline",
            Vec::new(),
        ));
    }
    if candidate_evidence.is_empty() {
        unavailable_facts.push(unavailable(
            ProfilerVariantUnavailableKindV2::CandidateCorrelationEvidenceMissing,
            "no PC/source-ISA or decoded-ATT/source-ISA evidence was supplied for the candidate",
            Vec::new(),
        ));
    }
    unavailable_facts.push(unavailable(
        ProfilerVariantUnavailableKindV2::ProfilerKirToCharacteristicKirBridgeUnavailable,
        "Variant V1 profiler KIR claims and Characteristic KIR identities have no admitted structural bridge; correlated Characteristic coordinates remain exact to their artifact but are not substituted for profiler KIR identity",
        Vec::new(),
    ));
    let mut characteristic_evidence = baseline_evidence
        .iter()
        .chain(&candidate_evidence)
        .flat_map(|binding| binding.evidence_ids.iter().copied())
        .collect::<Vec<_>>();
    sort_dedup(&mut characteristic_evidence);
    if !characteristic_evidence.is_empty() {
        unavailable_facts.push(unavailable(
            ProfilerVariantUnavailableKindV2::CharacteristicProducerAuthentication,
            "Characteristic archives are canonical and exactly artifact-bound but remain self-claimed; V2 does not promote their source/MIR identities to authenticated compiler provenance",
            characteristic_evidence,
        ));
    }
    let mut causal_evidence = changed_occurrences
        .iter()
        .flat_map(|change| change.evidence_ids.iter().copied())
        .collect::<Vec<_>>();
    sort_dedup(&mut causal_evidence);
    unavailable_facts.push(unavailable(
        ProfilerVariantUnavailableKindV2::CausalAttribution,
        "exact paired observations and Variant V1 ranked co-observations do not prove that a semantic, IR, ISA, schedule, or resource change caused a duration or counter delta",
        causal_evidence,
    ));
    unavailable_facts.sort_by_key(|fact| fact.kind);

    let result = ProfilerVariantComparisonV2 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V2,
        request_identity,
        comparison_v1,
        baseline_schedule,
        candidate_schedule,
        baseline_evidence,
        candidate_evidence,
        baseline_occurrences,
        candidate_occurrences,
        changed_occurrences,
        unavailable: unavailable_facts,
        causal_attribution:
            ProfilerVariantCausalUnavailableReasonV2::PositiveCoObservationsDoNotProveCausation,
        ranking_policy:
            "exact_source_mir_pairing_then_canonical_axis_order;co_observation_only_not_causality_or_superiority"
                .to_owned(),
    };
    let encoded =
        serde_json::to_vec(&result).map_err(|_| ProfilerVariantErrorV2::ResultTooLarge)?;
    if encoded.len() as u64 > MAX_PROFILER_VARIANT_RESULT_BYTES_V2 {
        return Err(ProfilerVariantErrorV2::ResultTooLarge);
    }
    Ok(result)
}

fn validate_selectors(
    treatment: ProfilerVariantTreatmentInputV2<'_>,
) -> Result<(), ProfilerVariantErrorV2> {
    for selectors in [
        treatment.pc_source_isa.map(|value| value.sample_identities),
        treatment
            .decoded_att_source_isa
            .map(|value| value.record_identities),
    ]
    .into_iter()
    .flatten()
    {
        if selectors.is_empty() || selectors.len() > MAX_PROFILER_VARIANT_SELECTORS_V2 {
            return Err(ProfilerVariantErrorV2::EvidenceTooLarge);
        }
        if selectors.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(ProfilerVariantErrorV2::DuplicateSelector);
        }
    }
    Ok(())
}

fn treatment_evidence_identity(
    treatment: ProfilerVariantTreatmentInputV2<'_>,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV2> {
    let mut digest = Sha256::new();
    digest.update(EVIDENCE_IDENTITY_DOMAIN_V2);
    let mut canonical_len = 0_u64;
    for bytes in [treatment.treatment.manifest, treatment.treatment.schedule] {
        hash_bytes(&mut digest, bytes, &mut canonical_len)?;
    }
    match treatment.pc_source_isa {
        Some(evidence) => {
            digest.update([1]);
            hash_bytes(&mut digest, evidence.source, &mut canonical_len)?;
            hash_bytes(&mut digest, evidence.relation, &mut canonical_len)?;
            hash_bytes(&mut digest, evidence.characteristic, &mut canonical_len)?;
            hash_selectors(&mut digest, evidence.sample_identities, &mut canonical_len)?;
        }
        None => digest.update([0]),
    }
    match treatment.decoded_att_source_isa {
        Some(evidence) => {
            digest.update([1]);
            hash_bytes(&mut digest, evidence.interchange, &mut canonical_len)?;
            hash_bytes(&mut digest, evidence.characteristic, &mut canonical_len)?;
            digest.update(evidence.code_object_identity.as_bytes());
            canonical_len = canonical_len
                .checked_add(32)
                .ok_or(ProfilerVariantErrorV2::EvidenceTooLarge)?;
            hash_selectors(&mut digest, evidence.record_identities, &mut canonical_len)?;
        }
        None => digest.update([0]),
    }
    content_identity_from_digest(digest.finalize().into(), canonical_len, 2)
}

fn hash_bytes(
    digest: &mut Sha256,
    bytes: &[u8],
    canonical_len: &mut u64,
) -> Result<(), ProfilerVariantErrorV2> {
    let len = u64::try_from(bytes.len()).map_err(|_| ProfilerVariantErrorV2::EvidenceTooLarge)?;
    if bytes.is_empty() {
        return Err(ProfilerVariantErrorV2::EvidenceTooLarge);
    }
    digest.update(len.to_le_bytes());
    digest.update(bytes);
    *canonical_len = canonical_len
        .checked_add(len)
        .ok_or(ProfilerVariantErrorV2::EvidenceTooLarge)?;
    Ok(())
}

fn hash_selectors(
    digest: &mut Sha256,
    selectors: &[CaptureIdentityV1],
    canonical_len: &mut u64,
) -> Result<(), ProfilerVariantErrorV2> {
    digest.update(
        u64::try_from(selectors.len())
            .map_err(|_| ProfilerVariantErrorV2::EvidenceTooLarge)?
            .to_le_bytes(),
    );
    for selector in selectors {
        digest.update(selector.as_bytes());
    }
    *canonical_len = canonical_len
        .checked_add(
            u64::try_from(selectors.len())
                .map_err(|_| ProfilerVariantErrorV2::EvidenceTooLarge)?
                .checked_mul(32)
                .ok_or(ProfilerVariantErrorV2::EvidenceTooLarge)?,
        )
        .ok_or(ProfilerVariantErrorV2::EvidenceTooLarge)?;
    Ok(())
}

fn request_identity(
    request: ProfilerVariantComparisonRequestV2,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV2> {
    let bytes = serde_json::to_vec(&request).map_err(|_| ProfilerVariantErrorV2::InvalidRequest)?;
    domain_content_identity(REQUEST_IDENTITY_DOMAIN_V2, &bytes, 2)
}

fn schedule_binding(
    content_identity: ContentIdentityRecordV1,
    bytes: &[u8],
) -> Result<ProfilerVariantScheduleBindingV2, ProfilerVariantErrorV2> {
    Ok(ProfilerVariantScheduleBindingV2 {
        origin: TruthOriginV1::Declared,
        content_identity,
        semantic_schedule_identity: domain_content_identity(SCHEDULE_IDENTITY_DOMAIN_V2, bytes, 2)?,
        semantics: "content_bound_caller_declared_schedule_not_observed_execution_or_authenticated_producer_semantics".to_owned(),
    })
}

fn admit_correlation_evidence(
    input: ProfilerVariantTreatmentInputV2<'_>,
    expected_artifact: ContentIdentityRecordV1,
    unavailable_facts: &mut Vec<ProfilerVariantUnavailableV2>,
    baseline: bool,
) -> Result<
    (
        Vec<ProfilerVariantEvidenceBindingV2>,
        Vec<ProfilerVariantOccurrenceV2>,
    ),
    ProfilerVariantErrorV2,
> {
    let mut bindings = Vec::new();
    let mut occurrences = Vec::new();
    if let Some(evidence) = input.pc_source_isa {
        let capture = input
            .treatment
            .pc_samples
            .ok_or(ProfilerVariantErrorV2::MissingPcCapture)?;
        let session = PcSourceIsaSessionV1::open(
            evidence.source,
            capture,
            input.treatment.artifact,
            evidence.relation,
            evidence.characteristic,
            PcSourceIsaLimitsV1::default(),
        )
        .map_err(|_| ProfilerVariantErrorV2::PcEvidenceAdmission)?;
        if session.binding().artifact_identity != expected_artifact {
            return Err(ProfilerVariantErrorV2::PcEvidenceAdmission);
        }
        let start = occurrences.len();
        for selector in evidence.sample_identities {
            collect_pc_occurrences(&session, *selector, &mut occurrences, unavailable_facts)?;
        }
        let mut evidence_ids = vec![session.binding().binding_identity];
        evidence_ids.extend_from_slice(evidence.sample_identities);
        sort_dedup(&mut evidence_ids);
        bindings.push(ProfilerVariantEvidenceBindingV2 {
            observation_kind: ProfilerVariantObservationKindV2::PcSample,
            binding_identity: session.binding().binding_identity,
            artifact_identity: session.binding().artifact_identity,
            characteristic_identity: session.binding().characteristic_identity,
            characteristic_scan: session.binding().characteristic_scan,
            selector_count: evidence.sample_identities.len() as u16,
            occurrence_count: u16::try_from(occurrences.len() - start)
                .map_err(|_| ProfilerVariantErrorV2::EvidenceTooLarge)?,
            coverage: ProfilerVariantCoverageV2::PcSample {
                coverage: session.binding().capture_coverage,
            },
            absence_semantics:
                "stochastic samples and sparse characteristic intervals cannot establish absence, addition, removal, complete instruction history, or schedule execution"
                    .to_owned(),
            evidence_ids,
        });
    }
    if let Some(evidence) = input.decoded_att_source_isa {
        let session = DecodedAttSourceIsaSessionV1::open(
            evidence.interchange,
            evidence.code_object_identity,
            input.treatment.artifact,
            evidence.characteristic,
            DecodedAttSourceIsaLimitsV1::default(),
        )
        .map_err(|_| ProfilerVariantErrorV2::DecodedAttEvidenceAdmission)?;
        if session.binding().artifact_identity != expected_artifact {
            return Err(ProfilerVariantErrorV2::DecodedAttEvidenceAdmission);
        }
        let start = occurrences.len();
        for selector in evidence.record_identities {
            collect_att_occurrences(&session, *selector, &mut occurrences, unavailable_facts)?;
        }
        let mut evidence_ids = vec![session.binding().binding_identity];
        evidence_ids.extend_from_slice(evidence.record_identities);
        sort_dedup(&mut evidence_ids);
        bindings.push(ProfilerVariantEvidenceBindingV2 {
            observation_kind: ProfilerVariantObservationKindV2::DecodedAtt,
            binding_identity: session.binding().binding_identity,
            artifact_identity: session.binding().artifact_identity,
            characteristic_identity: session.binding().characteristic_identity,
            characteristic_scan: session.binding().characteristic_scan,
            selector_count: evidence.record_identities.len() as u16,
            occurrence_count: u16::try_from(occurrences.len() - start)
                .map_err(|_| ProfilerVariantErrorV2::EvidenceTooLarge)?,
            coverage: ProfilerVariantCoverageV2::DecodedAtt {
                coverage: session.binding().decoded_coverage,
            },
            absence_semantics:
                "decoder completeness and loss are preserved exactly; decoded records and sparse characteristic intervals cannot establish absence, addition, removal, or complete execution"
                    .to_owned(),
            evidence_ids,
        });
    }
    if occurrences.len() > MAX_PROFILER_VARIANT_OCCURRENCES_V2 {
        return Err(ProfilerVariantErrorV2::EvidenceTooLarge);
    }
    occurrences.sort_by_key(|item| item.occurrence_identity);
    if occurrences
        .windows(2)
        .any(|pair| pair[0].occurrence_identity == pair[1].occurrence_identity)
    {
        return Err(ProfilerVariantErrorV2::DuplicateSelector);
    }
    let missing_kind = if baseline {
        ProfilerVariantUnavailableKindV2::BaselineCorrelationEvidenceMissing
    } else {
        ProfilerVariantUnavailableKindV2::CandidateCorrelationEvidenceMissing
    };
    if !bindings.is_empty() && occurrences.is_empty() {
        unavailable_facts.push(unavailable(
            missing_kind,
            "supplied correlation evidence produced no positive source/IR/ISA occurrence",
            bindings
                .iter()
                .flat_map(|binding| binding.evidence_ids.iter().copied())
                .collect(),
        ));
    }
    Ok((bindings, occurrences))
}

fn collect_pc_occurrences(
    session: &PcSourceIsaSessionV1,
    selector: CaptureIdentityV1,
    output: &mut Vec<ProfilerVariantOccurrenceV2>,
    unavailable_facts: &mut Vec<ProfilerVariantUnavailableV2>,
) -> Result<(), ProfilerVariantErrorV2> {
    let result = session
        .lookup_sample(
            selector,
            PcSourceIsaPageRequestV1 {
                limit: MAX_PROFILER_VARIANT_SELECTORS_V2 as u16,
                cursor: None,
            },
        )
        .map_err(|_| ProfilerVariantErrorV2::PcEvidenceAdmission)?;
    let PcSourceIsaQueryResultV1::AttributionPage { page } = result else {
        return Err(ProfilerVariantErrorV2::SelectorUnavailableOrForeign);
    };
    if page.attribution_state == PcSourceIsaAttributionStateV1::AmbiguousOverlappingCorrelations {
        return Err(ProfilerVariantErrorV2::AmbiguousAttribution);
    }
    if page.attribution_state == PcSourceIsaAttributionStateV1::NoMatchingIsaInterval {
        unavailable_facts.push(unavailable(
            ProfilerVariantUnavailableKindV2::StableSourceMirSiteUnavailable,
            "the selected PC sample has no matching admitted Characteristic ISA interval",
            vec![page.binding.binding_identity, selector, page.query_identity],
        ));
    }
    append_page_items(
        ProfilerVariantObservationKindV2::PcSample,
        selector,
        page.binding.binding_identity,
        page.query_identity,
        &page.items,
        output,
        unavailable_facts,
    )?;
    let mut cursor = page.next_cursor;
    while let Some(next) = cursor {
        let result = session
            .lookup_sample(
                selector,
                PcSourceIsaPageRequestV1 {
                    limit: MAX_PROFILER_VARIANT_SELECTORS_V2 as u16,
                    cursor: Some(next),
                },
            )
            .map_err(|_| ProfilerVariantErrorV2::PcEvidenceAdmission)?;
        let PcSourceIsaQueryResultV1::AttributionPage { page } = result else {
            return Err(ProfilerVariantErrorV2::SelectorUnavailableOrForeign);
        };
        append_page_items(
            ProfilerVariantObservationKindV2::PcSample,
            selector,
            page.binding.binding_identity,
            page.query_identity,
            &page.items,
            output,
            unavailable_facts,
        )?;
        cursor = page.next_cursor;
    }
    Ok(())
}

fn collect_att_occurrences(
    session: &DecodedAttSourceIsaSessionV1,
    selector: CaptureIdentityV1,
    output: &mut Vec<ProfilerVariantOccurrenceV2>,
    unavailable_facts: &mut Vec<ProfilerVariantUnavailableV2>,
) -> Result<(), ProfilerVariantErrorV2> {
    let mut cursor = None;
    loop {
        let result = session
            .lookup_record(
                selector,
                DecodedAttSourceIsaPageRequestV1 {
                    limit: MAX_PROFILER_VARIANT_SELECTORS_V2 as u16,
                    cursor,
                },
            )
            .map_err(|_| ProfilerVariantErrorV2::DecodedAttEvidenceAdmission)?;
        let DecodedAttSourceIsaQueryResultV1::AttributionPage { page } = result else {
            return Err(ProfilerVariantErrorV2::SelectorUnavailableOrForeign);
        };
        if page.attribution_state == PcSourceIsaAttributionStateV1::AmbiguousOverlappingCorrelations
        {
            return Err(ProfilerVariantErrorV2::AmbiguousAttribution);
        }
        if page.attribution_state == PcSourceIsaAttributionStateV1::NoMatchingIsaInterval {
            unavailable_facts.push(unavailable(
                ProfilerVariantUnavailableKindV2::StableSourceMirSiteUnavailable,
                "the selected decoded ATT record has no matching admitted Characteristic ISA interval",
                vec![page.binding.binding_identity, selector, page.query_identity],
            ));
        }
        let items = page
            .items
            .iter()
            .map(|item| item.attribution.clone())
            .collect::<Vec<_>>();
        append_page_items(
            ProfilerVariantObservationKindV2::DecodedAtt,
            selector,
            page.binding.binding_identity,
            page.query_identity,
            &items,
            output,
            unavailable_facts,
        )?;
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(());
        }
    }
}

fn append_page_items(
    kind: ProfilerVariantObservationKindV2,
    selector: CaptureIdentityV1,
    binding: CaptureIdentityV1,
    query: CaptureIdentityV1,
    items: &[PcSourceIsaAttributionItemV1],
    output: &mut Vec<ProfilerVariantOccurrenceV2>,
    unavailable_facts: &mut Vec<ProfilerVariantUnavailableV2>,
) -> Result<(), ProfilerVariantErrorV2> {
    for item in items {
        if output.len() >= MAX_PROFILER_VARIANT_OCCURRENCES_V2 {
            return Err(ProfilerVariantErrorV2::EvidenceTooLarge);
        }
        let stable_source_mir_site = stable_site(item);
        let mut evidence_ids = vec![
            binding,
            selector,
            query,
            item.item_identity,
            item.correlation_occurrence_identity,
            item.characteristic_identity,
        ];
        sort_dedup(&mut evidence_ids);
        if stable_source_mir_site.is_none() {
            unavailable_facts.push(unavailable(
                ProfilerVariantUnavailableKindV2::StableSourceMirSiteUnavailable,
                "a positive source/ISA occurrence lacks the exact source-plus-MIR identity required for cross-treatment pairing",
                evidence_ids.clone(),
            ));
        }
        output.push(ProfilerVariantOccurrenceV2 {
            occurrence_identity: occurrence_identity(kind, selector, binding, item)?,
            observation_kind: kind,
            selector_identity: selector,
            stable_source_mir_site,
            semantic_operation_identity: item.semantic_operation_identity,
            neutral_kir_node_identity: item.neutral_kir_node_identity,
            neutral_kir: item.neutral_kir.map(project_kir),
            target_kir: project_kir(item.target_kir),
            compiler_handoff_llvm: ProfilerVariantLlvmCoordinateV2 {
                function_ordinal: item.compiler_handoff_llvm.function_ordinal,
                block_ordinal: item.compiler_handoff_llvm.block_ordinal,
                instruction_ordinal: item.compiler_handoff_llvm.instruction_ordinal,
            },
            isa: ProfilerVariantIsaIntervalV2 {
                kernel_ordinal: item.isa.kernel_ordinal,
                symbol_relative_start: item.isa.symbol_relative_start,
                symbol_relative_end: item.isa.symbol_relative_end,
            },
            category_code: item.category_code,
            kind_code: item.kind_code,
            record_kind_code: item.record_kind_code,
            transformation_code: item.transformation_code,
            evidence_ids,
        });
    }
    Ok(())
}

fn stable_site(item: &PcSourceIsaAttributionItemV1) -> Option<ProfilerVariantSourceSiteV2> {
    let source = item.source?;
    let mir_node_identity = item.mir_node_identity?;
    let mir = item.mir?;
    Some(ProfilerVariantSourceSiteV2 {
        source_node_identity: source.node_identity,
        file_identity: source.span.file_identity,
        byte_start: source.span.byte_start,
        byte_end: source.span.byte_end,
        mir_node_identity,
        mir_body_ordinal: mir.body_ordinal,
        mir_block_ordinal: mir.block_ordinal,
        mir_statement_ordinal: mir.statement_ordinal,
    })
}

fn project_kir(value: crate::PcSourceIsaKirV1) -> ProfilerVariantKirCoordinateV2 {
    ProfilerVariantKirCoordinateV2 {
        function_ordinal: value.function_ordinal,
        block_ordinal: value.block_ordinal,
        operation_ordinal: value.operation_ordinal,
    }
}

fn compare_occurrences(
    baseline: &[ProfilerVariantOccurrenceV2],
    candidate: &[ProfilerVariantOccurrenceV2],
    unavailable_facts: &mut Vec<ProfilerVariantUnavailableV2>,
) -> Result<Vec<ProfilerVariantChangedOccurrenceV2>, ProfilerVariantErrorV2> {
    type Key = (
        ProfilerVariantObservationKindV2,
        ProfilerVariantSourceSiteV2,
    );
    let mut left = BTreeMap::<Key, Vec<&ProfilerVariantOccurrenceV2>>::new();
    let mut right = BTreeMap::<Key, Vec<&ProfilerVariantOccurrenceV2>>::new();
    for occurrence in baseline {
        if let Some(site) = occurrence.stable_source_mir_site {
            left.entry((occurrence.observation_kind, site))
                .or_default()
                .push(occurrence);
        }
    }
    for occurrence in candidate {
        if let Some(site) = occurrence.stable_source_mir_site {
            right
                .entry((occurrence.observation_kind, site))
                .or_default()
                .push(occurrence);
        }
    }
    let keys = left
        .keys()
        .chain(right.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut changed = Vec::new();
    for key in keys {
        match (left.get(&key), right.get(&key)) {
            (Some(left), Some(right)) if left.len() == 1 && right.len() == 1 => {
                let left = left[0];
                let right = right[0];
                let axes = changed_axes(left, right);
                if !axes.is_empty() {
                    let mut evidence_ids = left
                        .evidence_ids
                        .iter()
                        .chain(&right.evidence_ids)
                        .copied()
                        .collect::<Vec<_>>();
                    sort_dedup(&mut evidence_ids);
                    changed.push(ProfilerVariantChangedOccurrenceV2 {
                        change_identity: change_identity(key, left, right, &axes)?,
                        observation_kind: key.0,
                        stable_source_mir_site: key.1,
                        baseline_occurrence_identity: left.occurrence_identity,
                        candidate_occurrence_identity: right.occurrence_identity,
                        changed_axes: axes,
                        evidence_ids,
                        interpretation: "exact paired positive observations at one source-plus-MIR site; not a causal attribution or complete execution-history delta".to_owned(),
                    });
                }
            }
            (Some(left), Some(right)) => {
                let evidence_ids = left
                    .iter()
                    .chain(right.iter())
                    .flat_map(|occurrence| occurrence.evidence_ids.iter().copied())
                    .collect::<Vec<_>>();
                unavailable_facts.push(unavailable(
                    ProfilerVariantUnavailableKindV2::UnmatchedObservationCannotEstablishAdditionOrRemoval,
                    "multiple positive occurrences share one source-plus-MIR key; V2 does not choose an arbitrary cross-treatment pairing",
                    evidence_ids,
                ));
            }
            (Some(values), None) | (None, Some(values)) => {
                unavailable_facts.push(unavailable(
                    ProfilerVariantUnavailableKindV2::UnmatchedObservationCannotEstablishAdditionOrRemoval,
                    "an occurrence was positively observed on only one side; sampled or incomplete coverage cannot classify it as added or removed",
                    values
                        .iter()
                        .flat_map(|occurrence| occurrence.evidence_ids.iter().copied())
                        .collect(),
                ));
            }
            (None, None) => unreachable!(),
        }
    }
    changed.sort_by_key(|change| change.change_identity);
    Ok(changed)
}

fn changed_axes(
    baseline: &ProfilerVariantOccurrenceV2,
    candidate: &ProfilerVariantOccurrenceV2,
) -> Vec<ProfilerVariantChangeAxisV2> {
    let mut axes = Vec::new();
    if baseline.semantic_operation_identity != candidate.semantic_operation_identity {
        axes.push(ProfilerVariantChangeAxisV2::SemanticOperation);
    }
    if baseline.neutral_kir_node_identity != candidate.neutral_kir_node_identity
        || baseline.neutral_kir != candidate.neutral_kir
    {
        axes.push(ProfilerVariantChangeAxisV2::NeutralKir);
    }
    if baseline.target_kir != candidate.target_kir {
        axes.push(ProfilerVariantChangeAxisV2::TargetKir);
    }
    if baseline.compiler_handoff_llvm != candidate.compiler_handoff_llvm {
        axes.push(ProfilerVariantChangeAxisV2::CompilerHandoffLlvm);
    }
    if baseline.isa != candidate.isa {
        axes.push(ProfilerVariantChangeAxisV2::IsaInterval);
    }
    if baseline.transformation_code != candidate.transformation_code {
        axes.push(ProfilerVariantChangeAxisV2::Transformation);
    }
    if baseline.category_code != candidate.category_code
        || baseline.kind_code != candidate.kind_code
        || baseline.record_kind_code != candidate.record_kind_code
    {
        axes.push(ProfilerVariantChangeAxisV2::Classification);
    }
    axes
}

fn occurrence_identity(
    kind: ProfilerVariantObservationKindV2,
    selector: CaptureIdentityV1,
    binding: CaptureIdentityV1,
    item: &PcSourceIsaAttributionItemV1,
) -> Result<CaptureIdentityV1, ProfilerVariantErrorV2> {
    let mut digest = Sha256::new();
    digest.update(OCCURRENCE_IDENTITY_DOMAIN_V2);
    digest.update([observation_kind_tag(kind)]);
    digest.update(binding.as_bytes());
    digest.update(selector.as_bytes());
    digest.update(item.item_identity.as_bytes());
    capture_identity(digest.finalize().into())
}

fn change_identity(
    key: (
        ProfilerVariantObservationKindV2,
        ProfilerVariantSourceSiteV2,
    ),
    baseline: &ProfilerVariantOccurrenceV2,
    candidate: &ProfilerVariantOccurrenceV2,
    axes: &[ProfilerVariantChangeAxisV2],
) -> Result<CaptureIdentityV1, ProfilerVariantErrorV2> {
    let mut digest = Sha256::new();
    digest.update(CHANGE_IDENTITY_DOMAIN_V2);
    digest.update([observation_kind_tag(key.0)]);
    hash_site(&mut digest, key.1);
    digest.update(baseline.occurrence_identity.as_bytes());
    digest.update(candidate.occurrence_identity.as_bytes());
    for axis in axes {
        digest.update([change_axis_tag(*axis)]);
    }
    capture_identity(digest.finalize().into())
}

fn hash_site(digest: &mut Sha256, site: ProfilerVariantSourceSiteV2) {
    digest.update(site.source_node_identity.as_bytes());
    digest.update(site.file_identity.as_bytes());
    digest.update(site.byte_start.to_le_bytes());
    digest.update(site.byte_end.to_le_bytes());
    digest.update(site.mir_node_identity.as_bytes());
    digest.update(site.mir_body_ordinal.to_le_bytes());
    digest.update(site.mir_block_ordinal.to_le_bytes());
    digest.update(site.mir_statement_ordinal.to_le_bytes());
}

const fn observation_kind_tag(kind: ProfilerVariantObservationKindV2) -> u8 {
    match kind {
        ProfilerVariantObservationKindV2::PcSample => 0,
        ProfilerVariantObservationKindV2::DecodedAtt => 1,
    }
}

const fn change_axis_tag(axis: ProfilerVariantChangeAxisV2) -> u8 {
    match axis {
        ProfilerVariantChangeAxisV2::SemanticOperation => 0,
        ProfilerVariantChangeAxisV2::NeutralKir => 1,
        ProfilerVariantChangeAxisV2::TargetKir => 2,
        ProfilerVariantChangeAxisV2::CompilerHandoffLlvm => 3,
        ProfilerVariantChangeAxisV2::IsaInterval => 4,
        ProfilerVariantChangeAxisV2::Transformation => 5,
        ProfilerVariantChangeAxisV2::Classification => 6,
    }
}

fn unavailable(
    kind: ProfilerVariantUnavailableKindV2,
    reason: &str,
    mut evidence_ids: Vec<CaptureIdentityV1>,
) -> ProfilerVariantUnavailableV2 {
    sort_dedup(&mut evidence_ids);
    ProfilerVariantUnavailableV2 {
        kind,
        origin: TruthOriginV1::Unavailable,
        reason: reason.to_owned(),
        evidence_ids,
    }
}

fn sort_dedup(values: &mut Vec<CaptureIdentityV1>) {
    values.sort_unstable();
    values.dedup();
}

fn domain_content_identity(
    domain: &[u8],
    bytes: &[u8],
    format_version: u16,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV2> {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    content_identity_from_digest(
        digest.finalize().into(),
        u64::try_from(bytes.len()).map_err(|_| ProfilerVariantErrorV2::EvidenceTooLarge)?,
        format_version,
    )
}

fn content_identity_from_digest(
    digest: [u8; 32],
    canonical_len: u64,
    format_version: u16,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV2> {
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version,
        digest: capture_identity(digest)?,
        canonical_len,
    })
}

fn capture_identity(bytes: [u8; 32]) -> Result<CaptureIdentityV1, ProfilerVariantErrorV2> {
    CaptureIdentityV1::new(bytes).map_err(|_| ProfilerVariantErrorV2::IdentityFailure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pc_sample_source_isa_v1::tests::{
        ARTIFACT, CorrelationFixtureV1, EvidenceV1, evidence,
    };
    use crate::{
        PcSourceIsaIntervalV1, PcSourceIsaKirV1, PcSourceIsaLlvmV1, PcSourceIsaMirV1,
        PcSourceIsaSourceV1, PcSourceIsaSpanV1, ProfilerVariantManifestInputV1,
        build_profiler_variant_manifest_v1,
    };
    use fe2o3_semantic_import::{
        ArtifactClaimV1, ContentSchemeV1, ProfilerDeviceBindingV4, ProfilerDispatchBindingV4,
        ProfilerEnvironmentBindingV4, encode_profiler_bundle_v4,
        import_rocprofv3_json_profiler_bundle_v4,
    };
    use fe2o3_semantic_trace::{KernelIrIdentityClaimV1, OpaqueIdentityV1, WaveWidthV1};

    const STRICT_PROFILER_SOURCE: &[u8] = include_bytes!(
        "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-installed-97f5574-kernel-dispatch-schema.json"
    );

    fn id(value: u8) -> CaptureIdentityV1 {
        CaptureIdentityV1::new([value; 32]).unwrap()
    }

    fn occurrence(value: u8, isa_start: u64) -> ProfilerVariantOccurrenceV2 {
        ProfilerVariantOccurrenceV2 {
            occurrence_identity: id(value),
            observation_kind: ProfilerVariantObservationKindV2::PcSample,
            selector_identity: id(value + 1),
            stable_source_mir_site: Some(ProfilerVariantSourceSiteV2 {
                source_node_identity: id(10),
                file_identity: id(11),
                byte_start: 3,
                byte_end: 7,
                mir_node_identity: id(12),
                mir_body_ordinal: 0,
                mir_block_ordinal: 1,
                mir_statement_ordinal: 2,
            }),
            semantic_operation_identity: id(13),
            neutral_kir_node_identity: Some(id(14)),
            neutral_kir: Some(ProfilerVariantKirCoordinateV2 {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: 1,
            }),
            target_kir: ProfilerVariantKirCoordinateV2 {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: 2,
            },
            compiler_handoff_llvm: ProfilerVariantLlvmCoordinateV2 {
                function_ordinal: 0,
                block_ordinal: 0,
                instruction_ordinal: 3,
            },
            isa: ProfilerVariantIsaIntervalV2 {
                kernel_ordinal: 0,
                symbol_relative_start: isa_start,
                symbol_relative_end: isa_start + 4,
            },
            category_code: 1,
            kind_code: 2,
            record_kind_code: 3,
            transformation_code: 0,
            evidence_ids: vec![id(value), id(value + 1)],
        }
    }

    struct PcTreatmentFixture {
        manifest: Vec<u8>,
        raw_profiler_source: Vec<u8>,
        bundle: Vec<u8>,
        schedule: Vec<u8>,
        evidence: EvidenceV1,
    }

    impl PcTreatmentFixture {
        fn input(&self, sample_ordinal: usize) -> ProfilerVariantTreatmentInputV2<'_> {
            ProfilerVariantTreatmentInputV2 {
                treatment: ProfilerVariantTreatmentInputV1 {
                    manifest: &self.manifest,
                    semantic_workload: b"pc-variant-v2-workload",
                    raw_profiler_source: &self.raw_profiler_source,
                    bundle: &self.bundle,
                    schedule: &self.schedule,
                    artifact: ARTIFACT,
                    isa_projection: None,
                    counters: None,
                    pc_samples: Some(&self.evidence.capture),
                },
                pc_source_isa: Some(ProfilerVariantPcSourceIsaEvidenceV2 {
                    source: &self.evidence.source,
                    relation: &self.evidence.relation,
                    characteristic: &self.evidence.characteristic,
                    sample_identities: &self.evidence.samples[sample_ordinal..=sample_ordinal],
                }),
                decoded_att_source_isa: None,
            }
        }
    }

    fn pc_treatment(kind: CorrelationFixtureV1, schedule: &[u8]) -> PcTreatmentFixture {
        let evidence = evidence(kind);
        let mut treatment_document: serde_json::Value =
            serde_json::from_slice(STRICT_PROFILER_SOURCE).unwrap();
        let node = treatment_document["rocprofiler-sdk-tool"][0]["agents"][0]["node_id"]
            .as_u64()
            .unwrap();
        treatment_document["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"][0]["dispatch_info"]
            ["workgroup_size"] = serde_json::json!({"x": 256, "y": 1, "z": 1});
        treatment_document["rocprofiler-sdk-tool"][0]["buffer_records"]["kernel_dispatch"][0]["dispatch_info"]
            ["grid_size"] = serde_json::json!({"x": 256, "y": 1, "z": 1});
        let raw_profiler_source = serde_json::to_vec(&treatment_document).unwrap();
        let record = |byte| ContentIdentityRecordV1 {
            scheme: ContentSchemeV1::DomainSeparatedSha256,
            format_version: 1,
            digest: id(byte),
            canonical_len: 32,
        };
        let artifact_digest: [u8; 32] = Sha256::digest(ARTIFACT).into();
        let binding = ProfilerDispatchBindingV4 {
            environment: ProfilerEnvironmentBindingV4 {
                environment: record(81),
                collector_tool: record(82),
                collector_configuration: record(83),
                stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                    source_agent_id: node,
                    stable_identity: record(84),
                }],
            },
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(
                OpaqueIdentityV1::new([90; 32]).unwrap(),
                97,
            )
            .unwrap(),
            artifact: Some(ArtifactClaimV1 {
                identity: OpaqueIdentityV1::new(artifact_digest).unwrap(),
                canonical_len: ARTIFACT.len() as u64,
                format_version: 1,
            }),
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        };
        let bundle = encode_profiler_bundle_v4(
            &import_rocprofv3_json_profiler_bundle_v4(&raw_profiler_source, binding).unwrap(),
        )
        .unwrap();
        let manifest = build_profiler_variant_manifest_v1(ProfilerVariantManifestInputV1 {
            semantic_workload: b"pc-variant-v2-workload",
            raw_profiler_source: &raw_profiler_source,
            bundle: &bundle,
            schedule,
            artifact: ARTIFACT,
            kernel_ordinal: 0,
            isa_projection: None,
            counters: None,
            pc_samples: Some(&evidence.capture),
        })
        .unwrap();
        PcTreatmentFixture {
            manifest,
            raw_profiler_source,
            bundle,
            schedule: schedule.to_vec(),
            evidence,
        }
    }

    #[test]
    fn exact_source_mir_pair_reports_positive_semantic_ir_isa_changes_without_causality() {
        let left = occurrence(20, 0);
        let mut right = occurrence(30, 4);
        right.semantic_operation_identity = id(31);
        right.neutral_kir_node_identity = Some(id(32));
        right.target_kir.operation_ordinal = 9;
        right.compiler_handoff_llvm.instruction_ordinal = 10;
        let mut unavailable = Vec::new();
        let changes = compare_occurrences(&[left], &[right], &mut unavailable).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(
            changes[0].changed_axes,
            vec![
                ProfilerVariantChangeAxisV2::SemanticOperation,
                ProfilerVariantChangeAxisV2::NeutralKir,
                ProfilerVariantChangeAxisV2::TargetKir,
                ProfilerVariantChangeAxisV2::CompilerHandoffLlvm,
                ProfilerVariantChangeAxisV2::IsaInterval,
            ]
        );
        assert!(changes[0].interpretation.contains("not a causal"));
    }

    #[test]
    fn unmatched_or_duplicated_observations_never_become_added_or_removed() {
        let left = occurrence(20, 0);
        let duplicate = occurrence(21, 8);
        let mut unavailable = Vec::new();
        assert!(
            compare_occurrences(&[left.clone(), duplicate], &[], &mut unavailable)
                .unwrap()
                .is_empty()
        );
        assert!(unavailable.iter().all(|fact| {
            fact.kind
                == ProfilerVariantUnavailableKindV2::UnmatchedObservationCannotEstablishAdditionOrRemoval
        }));

        unavailable.clear();
        assert!(
            compare_occurrences(
                &[left.clone(), left],
                &[occurrence(30, 4)],
                &mut unavailable
            )
            .unwrap()
            .is_empty()
        );
        assert!(unavailable[0].reason.contains("does not choose"));
    }

    #[test]
    fn stable_site_requires_both_source_and_mir_identity() {
        let item = PcSourceIsaAttributionItemV1 {
            item_identity: id(1),
            correlation_occurrence_identity: id(2),
            characteristic_identity: id(3),
            interval_ordinal: 0,
            catalog_record_ordinal: 0,
            category_code: 1,
            category: "test",
            kind_code: 1,
            kind: "test",
            record_kind_code: 1,
            record_kind: "test",
            source: Some(PcSourceIsaSourceV1 {
                node_identity: id(4),
                span: PcSourceIsaSpanV1 {
                    file_identity: id(5),
                    byte_start: 0,
                    byte_end: 1,
                    line: 1,
                    column: 1,
                },
            }),
            mir_node_identity: None,
            mir: Some(PcSourceIsaMirV1 {
                body_ordinal: 0,
                block_ordinal: 0,
                statement_ordinal: 0,
            }),
            neutral_kir_node_identity: None,
            neutral_kir: None,
            target_kir: PcSourceIsaKirV1 {
                function_ordinal: 0,
                block_ordinal: 0,
                operation_ordinal: 0,
            },
            semantic_operation_identity: id(6),
            compiler_handoff_llvm: PcSourceIsaLlvmV1 {
                function_ordinal: 0,
                block_ordinal: 0,
                instruction_ordinal: 0,
            },
            isa: PcSourceIsaIntervalV1 {
                kernel_ordinal: 0,
                symbol_relative_start: 0,
                symbol_relative_end: 4,
            },
            transformation_code: 0,
            transformation: "retained",
        };
        assert!(stable_site(&item).is_none());
    }

    #[test]
    fn exact_pc_sessions_compose_and_report_one_evidence_bound_isa_change() {
        let baseline = pc_treatment(CorrelationFixtureV1::UniqueSource, b"schedule-a");
        let candidate = pc_treatment(CorrelationFixtureV1::UniqueSourceShifted, b"schedule-b");
        let request = build_profiler_variant_request_v2(
            b"pc-variant-v2-workload",
            baseline.input(0),
            candidate.input(1),
        )
        .unwrap();
        let result =
            compare_profiler_variants_v2(request, baseline.input(0), candidate.input(1)).unwrap();
        assert!(result.comparison_v1.comparable);
        assert_eq!(result.baseline_evidence.len(), 1);
        assert_eq!(result.candidate_evidence.len(), 1);
        assert_eq!(result.baseline_occurrences.len(), 1);
        assert_eq!(result.candidate_occurrences.len(), 1);
        assert_eq!(result.changed_occurrences.len(), 1);
        assert_eq!(
            result.changed_occurrences[0].changed_axes,
            vec![ProfilerVariantChangeAxisV2::IsaInterval]
        );
        assert!(result.changed_occurrences[0].evidence_ids.len() >= 6);
        assert!(matches!(
            result.baseline_evidence[0].coverage,
            ProfilerVariantCoverageV2::PcSample { .. }
        ));
        assert_eq!(
            result.causal_attribution,
            ProfilerVariantCausalUnavailableReasonV2::PositiveCoObservationsDoNotProveCausation
        );
    }

    #[test]
    fn foreign_pc_selector_and_overlapping_characteristic_fail_closed() {
        let valid = pc_treatment(CorrelationFixtureV1::UniqueSource, b"schedule");
        let foreign = id(99);
        let foreign_input = ProfilerVariantTreatmentInputV2 {
            treatment: valid.input(0).treatment,
            pc_source_isa: Some(ProfilerVariantPcSourceIsaEvidenceV2 {
                source: &valid.evidence.source,
                relation: &valid.evidence.relation,
                characteristic: &valid.evidence.characteristic,
                sample_identities: std::slice::from_ref(&foreign),
            }),
            decoded_att_source_isa: None,
        };
        let request = build_profiler_variant_request_v2(
            b"pc-variant-v2-workload",
            foreign_input,
            foreign_input,
        )
        .unwrap();
        assert_eq!(
            compare_profiler_variants_v2(request, foreign_input, foreign_input).unwrap_err(),
            ProfilerVariantErrorV2::SelectorUnavailableOrForeign
        );

        let ambiguous = pc_treatment(CorrelationFixtureV1::Ambiguous, b"schedule");
        let request = build_profiler_variant_request_v2(
            b"pc-variant-v2-workload",
            ambiguous.input(0),
            ambiguous.input(0),
        )
        .unwrap();
        assert_eq!(
            compare_profiler_variants_v2(request, ambiguous.input(0), ambiguous.input(0))
                .unwrap_err(),
            ProfilerVariantErrorV2::AmbiguousAttribution
        );
    }
}

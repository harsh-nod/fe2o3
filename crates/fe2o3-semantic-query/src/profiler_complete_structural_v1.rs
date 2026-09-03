//! Exact structural multiplicity deltas from two complete production archives.
//!
//! This contract is additive to Profiler Variant V3. It can call a classified
//! target-KIR occurrence added or removed only when both supplied production
//! owners retain complete admitted catalogs and complete Characteristic scans
//! in the same exact workload and stable source/MIR universe. Profiler samples
//! and decoded thread traces are deliberately excluded from absence decisions.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use fe2o3_hsaco_finalize::{
    AdmittedProductionProfilerKirArchiveV1, PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
    ProductionSourceIsaCatalogRecordV1, ProductionSourceIsaCharacteristicCollectionV1,
    ProductionSourceIsaCharacteristicCorrelationV1, ProductionSourceIsaCharacteristicKindV1,
    ProductionSourceIsaCharacteristicMemoryFormV1, ProductionSourceIsaCharacteristicWitnessV1,
};
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, TruthOriginV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    MAX_PROFILER_VARIANT_RESULT_BYTES_V3, ProfilerVariantComparisonRequestV3,
    ProfilerVariantComparisonV3, ProfilerVariantErrorV3, ProfilerVariantKirCoordinateV2,
    ProfilerVariantProductionKirEvidenceV3, ProfilerVariantStructuralContentIdentityV3,
    ProfilerVariantTreatmentInputV2, ProfilerVariantTreatmentInputV3,
    ProfilerVariantTreatmentSideV3, build_profiler_variant_request_v3,
    compare_profiler_variants_v3,
};

pub const PROFILER_COMPLETE_STRUCTURAL_SCHEMA_VERSION_V1: u16 = 1;
pub const PROFILER_COMPLETE_STRUCTURAL_SCHEMA_V1: &str =
    "fe2o3-profiler-complete-structural-comparison-v1";
pub const MAX_PROFILER_COMPLETE_STRUCTURAL_DELTAS_V1: usize = 4_096;
pub const MAX_PROFILER_COMPLETE_STRUCTURAL_DELTA_OCCURRENCES_V1: usize = 4_096;
pub const MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1: u64 = 16 * 1024 * 1024;

const REQUEST_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.profiler-complete-structural.request.v1\0";
const COMPARISON_DOMAIN_IDENTITY_V1: &[u8] =
    b"fe2o3.profiler-complete-structural.comparison-domain.v1\0";
const SOURCE_MIR_SET_IDENTITY_V1: &[u8] = b"fe2o3.profiler-complete-structural.source-mir-set.v1\0";
const OCCURRENCE_KEY_IDENTITY_V1: &[u8] = b"fe2o3.profiler-complete-structural.occurrence-key.v1\0";
const CORRELATION_SET_IDENTITY_V1: &[u8] =
    b"fe2o3.profiler-complete-structural.correlation-set.v1\0";
const OCCURRENCE_IDENTITY_V1: &[u8] = b"fe2o3.profiler-complete-structural.occurrence.v1\0";
const DELTA_IDENTITY_V1: &[u8] = b"fe2o3.profiler-complete-structural.delta.v1\0";

#[derive(Clone, Copy, Debug)]
pub struct ProfilerCompleteStructuralTreatmentInputV1<'a> {
    pub treatment: ProfilerVariantTreatmentInputV2<'a>,
    pub archive: Option<&'a AdmittedProductionProfilerKirArchiveV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralComparisonRequestV1 {
    pub schema_version: u16,
    pub comparison_v3: ProfilerVariantComparisonRequestV3,
    pub baseline_archive: Option<ContentIdentityRecordV1>,
    pub candidate_archive: Option<ContentIdentityRecordV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCompleteStructuralMemoryFormV1 {
    Plain,
    Guarded,
    MatrixTile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProfilerCompleteStructuralCharacteristicV1 {
    GlobalStore {
        form: ProfilerCompleteStructuralMemoryFormV1,
    },
    WorkgroupLoad {
        form: ProfilerCompleteStructuralMemoryFormV1,
    },
    WorkgroupStore {
        form: ProfilerCompleteStructuralMemoryFormV1,
    },
    WorkgroupBarrier,
    Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralCatalogV1 {
    pub side: ProfilerVariantTreatmentSideV3,
    pub archive_identity: ContentIdentityRecordV1,
    pub catalog_identity: CaptureIdentityV1,
    pub catalog_record_count: u64,
    pub classified_target_operation_count: u64,
    pub retained_correlation_count: u64,
    pub pre_kir_elimination_count: u64,
    pub source_map_v2: ProfilerVariantStructuralContentIdentityV3,
    pub stable_source_mir_universe_identity: CaptureIdentityV1,
    pub stable_source_mir_site_count: u64,
    pub complete_admitted_catalog_projection: bool,
    pub complete_characteristic_scan: bool,
    pub semantics: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralComparisonDomainV1 {
    pub identity: ContentIdentityRecordV1,
    pub semantic_workload: ContentIdentityRecordV1,
    pub stable_source_mir_universe_identity: CaptureIdentityV1,
    pub stable_source_mir_site_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralOccurrenceV1 {
    pub occurrence_identity: CaptureIdentityV1,
    pub side: ProfilerVariantTreatmentSideV3,
    pub target_kir: ProfilerVariantKirCoordinateV2,
    pub catalog_correlation_identity: CaptureIdentityV1,
    pub catalog_correlation_count: u64,
    pub evidence_ids: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCompleteStructuralDeltaDirectionV1 {
    Added,
    Removed,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralDeltaV1 {
    pub delta_identity: CaptureIdentityV1,
    pub direction: ProfilerCompleteStructuralDeltaDirectionV1,
    pub comparison_key_identity: CaptureIdentityV1,
    pub characteristic: ProfilerCompleteStructuralCharacteristicV1,
    pub stable_source_mir_set_identity: CaptureIdentityV1,
    pub stable_source_mir_site_count: u64,
    pub baseline_multiplicity: u64,
    pub candidate_multiplicity: u64,
    pub occurrence_delta: u64,
    pub baseline_occurrences: Vec<ProfilerCompleteStructuralOccurrenceV1>,
    pub candidate_occurrences: Vec<ProfilerCompleteStructuralOccurrenceV1>,
    pub origin: TruthOriginV1,
    pub evidence_ids: Vec<CaptureIdentityV1>,
    pub interpretation: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerCompleteStructuralUnavailableKindV1 {
    BaselineCompleteCatalogCoverage,
    CandidateCompleteCatalogCoverage,
    CrossDomainIdentity,
    StableOccurrenceIdentityCoverage,
    ResultBudget,
    ScheduleExecution,
    CausalAttribution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralUnavailableV1 {
    pub kind: ProfilerCompleteStructuralUnavailableKindV1,
    pub reason_code: &'static str,
    pub origin: TruthOriginV1,
    pub semantics: &'static str,
    pub evidence_ids: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerCompleteStructuralComparisonV1 {
    pub schema_version: u16,
    pub request_identity: ContentIdentityRecordV1,
    pub comparison_v3: Box<ProfilerVariantComparisonV3>,
    pub comparison_domain: Option<ProfilerCompleteStructuralComparisonDomainV1>,
    pub baseline_catalog: Option<ProfilerCompleteStructuralCatalogV1>,
    pub candidate_catalog: Option<ProfilerCompleteStructuralCatalogV1>,
    pub added: Vec<ProfilerCompleteStructuralDeltaV1>,
    pub removed: Vec<ProfilerCompleteStructuralDeltaV1>,
    pub unavailable: Vec<ProfilerCompleteStructuralUnavailableV1>,
    pub absence_basis: &'static str,
    pub authority: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProfilerCompleteStructuralErrorV1 {
    VariantV3(ProfilerVariantErrorV3),
    InvalidRequest,
    RequestMismatch,
    ProductionEvidenceMismatch,
    EvidenceTooLarge,
    ResultTooLarge,
    IdentityFailure,
}

impl fmt::Display for ProfilerCompleteStructuralErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "complete profiler structural comparison rejected: {self:?}"
        )
    }
}

impl Error for ProfilerCompleteStructuralErrorV1 {}

pub fn build_profiler_complete_structural_request_v1(
    semantic_workload: &[u8],
    baseline: ProfilerCompleteStructuralTreatmentInputV1<'_>,
    candidate: ProfilerCompleteStructuralTreatmentInputV1<'_>,
) -> Result<ProfilerCompleteStructuralComparisonRequestV1, ProfilerCompleteStructuralErrorV1> {
    let baseline_v3 = variant_input(baseline);
    let candidate_v3 = variant_input(candidate);
    Ok(ProfilerCompleteStructuralComparisonRequestV1 {
        schema_version: PROFILER_COMPLETE_STRUCTURAL_SCHEMA_VERSION_V1,
        comparison_v3: build_profiler_variant_request_v3(
            semantic_workload,
            baseline_v3,
            candidate_v3,
        )
        .map_err(ProfilerCompleteStructuralErrorV1::VariantV3)?,
        baseline_archive: baseline.archive.map(archive_content_identity).transpose()?,
        candidate_archive: candidate
            .archive
            .map(archive_content_identity)
            .transpose()?,
    })
}

pub fn compare_profiler_complete_structural_v1(
    request: ProfilerCompleteStructuralComparisonRequestV1,
    baseline: ProfilerCompleteStructuralTreatmentInputV1<'_>,
    candidate: ProfilerCompleteStructuralTreatmentInputV1<'_>,
) -> Result<ProfilerCompleteStructuralComparisonV1, ProfilerCompleteStructuralErrorV1> {
    if request.schema_version != PROFILER_COMPLETE_STRUCTURAL_SCHEMA_VERSION_V1 {
        return Err(ProfilerCompleteStructuralErrorV1::InvalidRequest);
    }
    let expected = build_profiler_complete_structural_request_v1(
        baseline.treatment.treatment.semantic_workload,
        baseline,
        candidate,
    )?;
    if expected != request {
        return Err(ProfilerCompleteStructuralErrorV1::RequestMismatch);
    }
    let comparison_v3 = compare_profiler_variants_v3(
        request.comparison_v3,
        variant_input(baseline),
        variant_input(candidate),
    )
    .map_err(ProfilerCompleteStructuralErrorV1::VariantV3)?;
    let request_identity = content_identity(REQUEST_IDENTITY_DOMAIN_V1, &request)?;
    let semantic_workload = request
        .comparison_v3
        .comparison_v2
        .comparison_v1
        .semantic_workload;
    let mut unavailable_facts = Vec::new();

    let baseline_catalog =
        catalog_summary(ProfilerVariantTreatmentSideV3::Baseline, baseline.archive)?;
    let candidate_catalog =
        catalog_summary(ProfilerVariantTreatmentSideV3::Candidate, candidate.archive)?;

    if baseline_catalog.as_ref().is_none_or(|catalog| {
        !catalog.complete_admitted_catalog_projection || !catalog.complete_characteristic_scan
    }) {
        unavailable_facts.push(unavailable_fact(
            ProfilerCompleteStructuralUnavailableKindV1::BaselineCompleteCatalogCoverage,
            "baseline_complete_catalog_coverage_unavailable",
            "baseline has no fully admitted archive owner with an internally consistent complete catalog projection and complete target-KIR Characteristic scan; no absence claim is made",
            evidence_for_catalog(baseline_catalog.as_ref()),
        ));
    }
    if candidate_catalog.as_ref().is_none_or(|catalog| {
        !catalog.complete_admitted_catalog_projection || !catalog.complete_characteristic_scan
    }) {
        unavailable_facts.push(unavailable_fact(
            ProfilerCompleteStructuralUnavailableKindV1::CandidateCompleteCatalogCoverage,
            "candidate_complete_catalog_coverage_unavailable",
            "candidate has no fully admitted archive owner with an internally consistent complete catalog projection and complete target-KIR Characteristic scan; no absence claim is made",
            evidence_for_catalog(candidate_catalog.as_ref()),
        ));
    }

    let comparison_domain = match (&baseline_catalog, &candidate_catalog) {
        (Some(left), Some(right))
            if left.complete_admitted_catalog_projection
                && left.complete_characteristic_scan
                && right.complete_admitted_catalog_projection
                && right.complete_characteristic_scan
                && same_catalog_domain(left, right) =>
        {
            Some(comparison_domain(
                semantic_workload,
                left.stable_source_mir_universe_identity,
                left.stable_source_mir_site_count,
            )?)
        }
        (Some(left), Some(right))
            if left.complete_admitted_catalog_projection
                && left.complete_characteristic_scan
                && right.complete_admitted_catalog_projection
                && right.complete_characteristic_scan =>
        {
            unavailable_facts.push(unavailable_fact(
                ProfilerCompleteStructuralUnavailableKindV1::CrossDomainIdentity,
                "cross_domain_source_mir_universe_identity",
                "the complete archives contain different exact stable source/MIR site universes; this contract has no authenticated lineage proof and does not compare absence across domains",
                evidence_for_pair(left, right),
            ));
            None
        }
        _ => None,
    };

    let mut added = Vec::new();
    let mut removed = Vec::new();
    if let (Some(domain), Some(left), Some(right), Some(left_evidence), Some(right_evidence)) = (
        comparison_domain,
        baseline_catalog.as_ref(),
        candidate_catalog.as_ref(),
        baseline.archive.map(production_evidence),
        candidate.archive.map(production_evidence),
    ) {
        match (
            project_occurrences(
                ProfilerVariantTreatmentSideV3::Baseline,
                left.archive_identity,
                left_evidence,
            )?,
            project_occurrences(
                ProfilerVariantTreatmentSideV3::Candidate,
                right.archive_identity,
                right_evidence,
            )?,
        ) {
            (Some(baseline_occurrences), Some(candidate_occurrences)) => {
                let comparison = compare_occurrence_multisets(
                    domain.identity.digest,
                    baseline_occurrences,
                    candidate_occurrences,
                )?;
                if comparison.retained_occurrences
                    > MAX_PROFILER_COMPLETE_STRUCTURAL_DELTA_OCCURRENCES_V1
                    || comparison.added.len() + comparison.removed.len()
                        > MAX_PROFILER_COMPLETE_STRUCTURAL_DELTAS_V1
                {
                    unavailable_facts.push(unavailable_fact(
                        ProfilerCompleteStructuralUnavailableKindV1::ResultBudget,
                        "complete_structural_delta_result_budget_exceeded",
                        "the exact delta exceeds the bounded V1 result budget; the service returns no partial added/removed set",
                        evidence_for_pair(left, right),
                    ));
                } else {
                    added = comparison.added;
                    removed = comparison.removed;
                }
            }
            _ => unavailable_facts.push(unavailable_fact(
                ProfilerCompleteStructuralUnavailableKindV1::StableOccurrenceIdentityCoverage,
                "stable_source_mir_identity_coverage_incomplete",
                "at least one classified target-KIR occurrence has no exact source-plus-MIR identity; the service returns no partial added/removed set",
                evidence_for_pair(left, right),
            )),
        }
    }

    unavailable_facts.push(unavailable_fact(
        ProfilerCompleteStructuralUnavailableKindV1::ScheduleExecution,
        "schedule_execution_unavailable",
        "archive admission and complete structural comparison do not authenticate that either caller-declared schedule executed",
        Vec::new(),
    ));
    unavailable_facts.push(unavailable_fact(
        ProfilerCompleteStructuralUnavailableKindV1::CausalAttribution,
        "causal_attribution_unavailable",
        "an exact structural multiplicity delta does not prove that the structural change caused a measured performance delta",
        added
            .iter()
            .chain(&removed)
            .flat_map(|delta| delta.evidence_ids.iter().copied())
            .collect(),
    ));
    unavailable_facts.sort_by_key(|fact| fact.kind);

    let result = ProfilerCompleteStructuralComparisonV1 {
        schema_version: PROFILER_COMPLETE_STRUCTURAL_SCHEMA_VERSION_V1,
        request_identity,
        comparison_v3: Box::new(comparison_v3),
        comparison_domain,
        baseline_catalog,
        candidate_catalog,
        added,
        removed,
        unavailable: unavailable_facts,
        absence_basis: "exact_multiset_difference_of_complete_admitted_production_characteristic_scans_in_one_workload_and_stable_source_mir_universe;sampled_pc_and_partial_or_lossy_att_absence_excluded",
        authority: "read_only_no_execution_attach_scheduling_collection_decoder_publication_load_launch_dispatch_or_runtime_authority",
    };
    let encoded = serde_json::to_vec(&result)
        .map_err(|_| ProfilerCompleteStructuralErrorV1::ResultTooLarge)?;
    if encoded.len() as u64 > MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1 {
        return Err(ProfilerCompleteStructuralErrorV1::ResultTooLarge);
    }
    Ok(result)
}

fn variant_input<'a>(
    input: ProfilerCompleteStructuralTreatmentInputV1<'a>,
) -> ProfilerVariantTreatmentInputV3<'a> {
    ProfilerVariantTreatmentInputV3 {
        treatment: input.treatment,
        production_kir: input.archive.map(production_evidence),
    }
}

fn catalog_summary(
    side: ProfilerVariantTreatmentSideV3,
    archive: Option<&AdmittedProductionProfilerKirArchiveV1>,
) -> Result<Option<ProfilerCompleteStructuralCatalogV1>, ProfilerCompleteStructuralErrorV1> {
    let Some(archive) = archive else {
        return Ok(None);
    };
    let archive_identity = archive_content_identity(archive)?;
    let evidence = production_evidence(archive);
    crate::profiler_variant_v3::validate_production_pair(evidence)
        .map_err(|_| ProfilerCompleteStructuralErrorV1::ProductionEvidenceMismatch)?;
    let characteristic = evidence.characteristic;
    let complete_projection = characteristic_projection_is_complete(characteristic, evidence);
    let (stable_source_mir_universe_identity, stable_source_mir_site_count) =
        catalog_source_mir_universe(evidence.catalog.records())?;
    Ok(Some(ProfilerCompleteStructuralCatalogV1 {
        side,
        archive_identity,
        catalog_identity: capture(evidence.catalog.identity())?,
        catalog_record_count: characteristic.catalog_record_count(),
        classified_target_operation_count: characteristic.classified_target_operation_count(),
        retained_correlation_count: characteristic.retained_correlation_count(),
        pre_kir_elimination_count: characteristic.pre_kir_elimination_count(),
        source_map_v2: ProfilerVariantStructuralContentIdentityV3 {
            digest: capture(&characteristic.source_map_v2_identity().sha256())?,
            canonical_len: characteristic.source_map_v2_identity().byte_len(),
        },
        stable_source_mir_universe_identity,
        stable_source_mir_site_count,
        complete_admitted_catalog_projection: complete_projection,
        complete_characteristic_scan: characteristic.scan_is_complete(),
        semantics: "complete bounded finalizer-admitted catalog projection and full supported-characteristic scan of the exact target KIR; not complete machine-instruction, execution, or external-provenance coverage",
    }))
}

fn production_evidence(
    archive: &AdmittedProductionProfilerKirArchiveV1,
) -> ProfilerVariantProductionKirEvidenceV3<'_> {
    ProfilerVariantProductionKirEvidenceV3 {
        bridge: archive.bridge(),
        catalog: archive.catalog(),
        characteristic: archive.characteristic(),
    }
}

fn archive_content_identity(
    archive: &AdmittedProductionProfilerKirArchiveV1,
) -> Result<ContentIdentityRecordV1, ProfilerCompleteStructuralErrorV1> {
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PRODUCTION_PROFILER_KIR_ARCHIVE_VERSION_V1,
        digest: capture(archive.identity().as_bytes())?,
        canonical_len: archive.canonical_len(),
    })
}

fn characteristic_projection_is_complete(
    characteristic: &ProductionSourceIsaCharacteristicCollectionV1,
    evidence: ProfilerVariantProductionKirEvidenceV3<'_>,
) -> bool {
    let catalog = evidence.catalog.records();
    let Ok(catalog_count) = u64::try_from(catalog.len()) else {
        return false;
    };
    let Ok(classified_count) = u64::try_from(characteristic.characteristics().len()) else {
        return false;
    };
    let Ok(elimination_count) =
        u64::try_from(characteristic.pre_kir_eliminated_catalog_records().len())
    else {
        return false;
    };
    let correlations = characteristic
        .characteristics()
        .iter()
        .flat_map(ProductionSourceIsaCharacteristicWitnessV1::correlations);
    let Ok(retained_count) = u64::try_from(correlations.clone().count()) else {
        return false;
    };
    characteristic.catalog_record_count() == catalog_count
        && characteristic.classified_target_operation_count() == classified_count
        && characteristic.pre_kir_elimination_count() == elimination_count
        && characteristic.retained_correlation_count() == retained_count
        && correlations
            .chain(characteristic.pre_kir_eliminated_catalog_records())
            .all(|correlation| correlation_matches_catalog(correlation, catalog))
}

fn correlation_matches_catalog(
    correlation: &ProductionSourceIsaCharacteristicCorrelationV1,
    catalog: &[ProductionSourceIsaCatalogRecordV1],
) -> bool {
    usize::try_from(correlation.catalog_record_ordinal())
        .ok()
        .and_then(|ordinal| catalog.get(ordinal))
        == Some(correlation.record())
}

fn comparison_domain(
    semantic_workload: ContentIdentityRecordV1,
    stable_source_mir_universe_identity: CaptureIdentityV1,
    stable_source_mir_site_count: u64,
) -> Result<ProfilerCompleteStructuralComparisonDomainV1, ProfilerCompleteStructuralErrorV1> {
    #[derive(Serialize)]
    struct Preimage {
        semantic_workload: ContentIdentityRecordV1,
        stable_source_mir_universe_identity: CaptureIdentityV1,
        stable_source_mir_site_count: u64,
    }
    let preimage = Preimage {
        semantic_workload,
        stable_source_mir_universe_identity,
        stable_source_mir_site_count,
    };
    Ok(ProfilerCompleteStructuralComparisonDomainV1 {
        identity: content_identity(COMPARISON_DOMAIN_IDENTITY_V1, &preimage)?,
        semantic_workload,
        stable_source_mir_universe_identity,
        stable_source_mir_site_count,
    })
}

fn same_catalog_domain(
    baseline: &ProfilerCompleteStructuralCatalogV1,
    candidate: &ProfilerCompleteStructuralCatalogV1,
) -> bool {
    baseline.stable_source_mir_universe_identity == candidate.stable_source_mir_universe_identity
        && baseline.stable_source_mir_site_count == candidate.stable_source_mir_site_count
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct StableSourceMirSiteV1 {
    source_node_identity: [u8; 32],
    file_identity: [u8; 32],
    byte_start: u64,
    byte_end: u64,
    line: u32,
    column: u32,
    mir_node_identity: [u8; 32],
    mir_body_ordinal: u64,
    mir_block_ordinal: u64,
    mir_statement_ordinal: u64,
}

fn catalog_source_mir_universe(
    records: &[ProductionSourceIsaCatalogRecordV1],
) -> Result<(CaptureIdentityV1, u64), ProfilerCompleteStructuralErrorV1> {
    let mut sites = records.iter().filter_map(stable_site).collect::<Vec<_>>();
    sites.sort_unstable();
    sites.dedup();
    let count = u64::try_from(sites.len())
        .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?;
    Ok((hash_stable_sites(&sites)?, count))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct OccurrenceKeyV1 {
    characteristic: ProfilerCompleteStructuralCharacteristicV1,
    stable_source_mir_set_identity: CaptureIdentityV1,
    stable_source_mir_site_count: u64,
}

#[derive(Clone, Debug)]
struct ProjectedOccurrenceV1 {
    key: OccurrenceKeyV1,
    occurrence: ProfilerCompleteStructuralOccurrenceV1,
}

fn project_occurrences(
    side: ProfilerVariantTreatmentSideV3,
    archive_identity: ContentIdentityRecordV1,
    evidence: ProfilerVariantProductionKirEvidenceV3<'_>,
) -> Result<Option<Vec<ProjectedOccurrenceV1>>, ProfilerCompleteStructuralErrorV1> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(evidence.characteristic.characteristics().len())
        .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?;
    for witness in evidence.characteristic.characteristics() {
        let mut sites = witness
            .correlations()
            .iter()
            .filter_map(|correlation| stable_site(correlation.record()))
            .collect::<Vec<_>>();
        sites.sort_unstable();
        sites.dedup();
        if sites.is_empty() {
            return Ok(None);
        }
        let stable_source_mir_set_identity = hash_stable_sites(&sites)?;
        let characteristic = project_characteristic(witness.kind());
        let key = OccurrenceKeyV1 {
            characteristic,
            stable_source_mir_set_identity,
            stable_source_mir_site_count: u64::try_from(sites.len())
                .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?,
        };
        let catalog_correlation_identity = correlation_set_identity(
            evidence.catalog.identity(),
            witness.target_kir(),
            witness.correlations(),
        )?;
        let occurrence_identity = occurrence_identity(
            side,
            archive_identity,
            key,
            witness.target_kir(),
            catalog_correlation_identity,
        )?;
        let mut evidence_ids = vec![
            archive_identity.digest,
            capture(evidence.catalog.identity())?,
            capture(evidence.bridge.identity())?,
            stable_source_mir_set_identity,
            catalog_correlation_identity,
        ];
        sort_dedup(&mut evidence_ids);
        output.push(ProjectedOccurrenceV1 {
            key,
            occurrence: ProfilerCompleteStructuralOccurrenceV1 {
                occurrence_identity,
                side,
                target_kir: project_kir(witness.target_kir()),
                catalog_correlation_identity,
                catalog_correlation_count: u64::try_from(witness.correlations().len())
                    .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?,
                evidence_ids,
            },
        });
    }
    output.sort_by_key(|value| (value.key, value.occurrence.occurrence_identity));
    if output.windows(2).any(|pair| {
        pair[0].occurrence.occurrence_identity == pair[1].occurrence.occurrence_identity
    }) {
        return Err(ProfilerCompleteStructuralErrorV1::ProductionEvidenceMismatch);
    }
    Ok(Some(output))
}

fn stable_site(record: &ProductionSourceIsaCatalogRecordV1) -> Option<StableSourceMirSiteV1> {
    let source_node_identity = record.source_node_identity()?;
    let span = record.source_span()?;
    let mir_node_identity = record.mir_node_identity()?;
    let mir = record.mir()?;
    Some(StableSourceMirSiteV1 {
        source_node_identity,
        file_identity: span.file_identity(),
        byte_start: span.byte_start(),
        byte_end: span.byte_end(),
        line: span.line(),
        column: span.column(),
        mir_node_identity,
        mir_body_ordinal: mir.body_ordinal(),
        mir_block_ordinal: mir.block_ordinal(),
        mir_statement_ordinal: mir.statement_ordinal(),
    })
}

fn hash_stable_sites(
    sites: &[StableSourceMirSiteV1],
) -> Result<CaptureIdentityV1, ProfilerCompleteStructuralErrorV1> {
    let mut digest = Sha256::new();
    digest.update(SOURCE_MIR_SET_IDENTITY_V1);
    digest.update(
        u64::try_from(sites.len())
            .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?
            .to_le_bytes(),
    );
    for site in sites {
        digest.update(site.source_node_identity);
        digest.update(site.file_identity);
        digest.update(site.byte_start.to_le_bytes());
        digest.update(site.byte_end.to_le_bytes());
        digest.update(site.line.to_le_bytes());
        digest.update(site.column.to_le_bytes());
        digest.update(site.mir_node_identity);
        digest.update(site.mir_body_ordinal.to_le_bytes());
        digest.update(site.mir_block_ordinal.to_le_bytes());
        digest.update(site.mir_statement_ordinal.to_le_bytes());
    }
    capture(&digest.finalize())
}

fn correlation_set_identity(
    catalog_identity: &[u8; 32],
    target_kir: fe2o3_hsaco_finalize::ProductionSourceIsaKirCoordinateV1,
    correlations: &[ProductionSourceIsaCharacteristicCorrelationV1],
) -> Result<CaptureIdentityV1, ProfilerCompleteStructuralErrorV1> {
    let mut digest = Sha256::new();
    digest.update(CORRELATION_SET_IDENTITY_V1);
    digest.update(catalog_identity);
    hash_kir(&mut digest, target_kir);
    digest.update(
        u64::try_from(correlations.len())
            .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?
            .to_le_bytes(),
    );
    for correlation in correlations {
        digest.update(correlation.catalog_record_ordinal().to_le_bytes());
    }
    capture(&digest.finalize())
}

fn occurrence_key_identity(
    key: OccurrenceKeyV1,
) -> Result<CaptureIdentityV1, ProfilerCompleteStructuralErrorV1> {
    let mut digest = Sha256::new();
    digest.update(OCCURRENCE_KEY_IDENTITY_V1);
    hash_characteristic(&mut digest, key.characteristic);
    digest.update(key.stable_source_mir_set_identity.as_bytes());
    digest.update(key.stable_source_mir_site_count.to_le_bytes());
    capture(&digest.finalize())
}

fn occurrence_identity(
    side: ProfilerVariantTreatmentSideV3,
    archive_identity: ContentIdentityRecordV1,
    key: OccurrenceKeyV1,
    target_kir: fe2o3_hsaco_finalize::ProductionSourceIsaKirCoordinateV1,
    correlation_identity: CaptureIdentityV1,
) -> Result<CaptureIdentityV1, ProfilerCompleteStructuralErrorV1> {
    let mut digest = Sha256::new();
    digest.update(OCCURRENCE_IDENTITY_V1);
    digest.update([side_code(side)]);
    digest.update(archive_identity.digest.as_bytes());
    digest.update(archive_identity.canonical_len.to_le_bytes());
    digest.update(occurrence_key_identity(key)?.as_bytes());
    hash_kir(&mut digest, target_kir);
    digest.update(correlation_identity.as_bytes());
    capture(&digest.finalize())
}

struct MultisetComparisonV1 {
    added: Vec<ProfilerCompleteStructuralDeltaV1>,
    removed: Vec<ProfilerCompleteStructuralDeltaV1>,
    retained_occurrences: usize,
}

fn compare_occurrence_multisets(
    domain_identity: CaptureIdentityV1,
    baseline: Vec<ProjectedOccurrenceV1>,
    candidate: Vec<ProjectedOccurrenceV1>,
) -> Result<MultisetComparisonV1, ProfilerCompleteStructuralErrorV1> {
    let mut baseline_groups = group_occurrences(baseline);
    let mut candidate_groups = group_occurrences(candidate);
    let keys = baseline_groups
        .keys()
        .chain(candidate_groups.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut retained_occurrences = 0_usize;
    for key in keys {
        let baseline = baseline_groups.remove(&key).unwrap_or_default();
        let candidate = candidate_groups.remove(&key).unwrap_or_default();
        if baseline.len() == candidate.len() {
            continue;
        }
        retained_occurrences = retained_occurrences
            .checked_add(baseline.len())
            .and_then(|value| value.checked_add(candidate.len()))
            .ok_or(ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?;
        let direction = if candidate.len() > baseline.len() {
            ProfilerCompleteStructuralDeltaDirectionV1::Added
        } else {
            ProfilerCompleteStructuralDeltaDirectionV1::Removed
        };
        let delta = structural_delta(domain_identity, key, direction, baseline, candidate)?;
        match direction {
            ProfilerCompleteStructuralDeltaDirectionV1::Added => added.push(delta),
            ProfilerCompleteStructuralDeltaDirectionV1::Removed => removed.push(delta),
        }
    }
    added.sort_by_key(|delta| delta.delta_identity);
    removed.sort_by_key(|delta| delta.delta_identity);
    Ok(MultisetComparisonV1 {
        added,
        removed,
        retained_occurrences,
    })
}

fn group_occurrences(
    occurrences: Vec<ProjectedOccurrenceV1>,
) -> BTreeMap<OccurrenceKeyV1, Vec<ProfilerCompleteStructuralOccurrenceV1>> {
    let mut groups = BTreeMap::new();
    for occurrence in occurrences {
        groups
            .entry(occurrence.key)
            .or_insert_with(Vec::new)
            .push(occurrence.occurrence);
    }
    for group in groups.values_mut() {
        group.sort_by_key(|occurrence| occurrence.occurrence_identity);
    }
    groups
}

fn structural_delta(
    domain_identity: CaptureIdentityV1,
    key: OccurrenceKeyV1,
    direction: ProfilerCompleteStructuralDeltaDirectionV1,
    baseline_occurrences: Vec<ProfilerCompleteStructuralOccurrenceV1>,
    candidate_occurrences: Vec<ProfilerCompleteStructuralOccurrenceV1>,
) -> Result<ProfilerCompleteStructuralDeltaV1, ProfilerCompleteStructuralErrorV1> {
    let comparison_key_identity = occurrence_key_identity(key)?;
    let baseline_multiplicity = u64::try_from(baseline_occurrences.len())
        .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?;
    let candidate_multiplicity = u64::try_from(candidate_occurrences.len())
        .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?;
    let occurrence_delta = baseline_multiplicity.abs_diff(candidate_multiplicity);
    let mut digest = Sha256::new();
    digest.update(DELTA_IDENTITY_V1);
    digest.update(domain_identity.as_bytes());
    digest.update([match direction {
        ProfilerCompleteStructuralDeltaDirectionV1::Added => 1,
        ProfilerCompleteStructuralDeltaDirectionV1::Removed => 2,
    }]);
    digest.update(comparison_key_identity.as_bytes());
    digest.update(baseline_multiplicity.to_le_bytes());
    for occurrence in &baseline_occurrences {
        digest.update(occurrence.occurrence_identity.as_bytes());
    }
    digest.update(candidate_multiplicity.to_le_bytes());
    for occurrence in &candidate_occurrences {
        digest.update(occurrence.occurrence_identity.as_bytes());
    }
    let delta_identity = capture(&digest.finalize())?;
    let mut evidence_ids = baseline_occurrences
        .iter()
        .chain(&candidate_occurrences)
        .flat_map(|occurrence| occurrence.evidence_ids.iter().copied())
        .collect::<Vec<_>>();
    evidence_ids.extend([domain_identity, comparison_key_identity]);
    sort_dedup(&mut evidence_ids);
    Ok(ProfilerCompleteStructuralDeltaV1 {
        delta_identity,
        direction,
        comparison_key_identity,
        characteristic: key.characteristic,
        stable_source_mir_set_identity: key.stable_source_mir_set_identity,
        stable_source_mir_site_count: key.stable_source_mir_site_count,
        baseline_multiplicity,
        candidate_multiplicity,
        occurrence_delta,
        baseline_occurrences,
        candidate_occurrences,
        origin: TruthOriginV1::Inferred,
        evidence_ids,
        interpretation: "exact structural occurrence multiplicity difference; every duplicate side occurrence remains separately identified, but continuity of an individual duplicate across treatments is not asserted",
    })
}

fn project_characteristic(
    kind: ProductionSourceIsaCharacteristicKindV1,
) -> ProfilerCompleteStructuralCharacteristicV1 {
    match kind {
        ProductionSourceIsaCharacteristicKindV1::GlobalStore { form } => {
            ProfilerCompleteStructuralCharacteristicV1::GlobalStore {
                form: project_memory_form(form),
            }
        }
        ProductionSourceIsaCharacteristicKindV1::WorkgroupLoad { form } => {
            ProfilerCompleteStructuralCharacteristicV1::WorkgroupLoad {
                form: project_memory_form(form),
            }
        }
        ProductionSourceIsaCharacteristicKindV1::WorkgroupStore { form } => {
            ProfilerCompleteStructuralCharacteristicV1::WorkgroupStore {
                form: project_memory_form(form),
            }
        }
        ProductionSourceIsaCharacteristicKindV1::WorkgroupBarrier => {
            ProfilerCompleteStructuralCharacteristicV1::WorkgroupBarrier
        }
        ProductionSourceIsaCharacteristicKindV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
            ProfilerCompleteStructuralCharacteristicV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate
        }
    }
}

const fn project_memory_form(
    form: ProductionSourceIsaCharacteristicMemoryFormV1,
) -> ProfilerCompleteStructuralMemoryFormV1 {
    match form {
        ProductionSourceIsaCharacteristicMemoryFormV1::Plain => {
            ProfilerCompleteStructuralMemoryFormV1::Plain
        }
        ProductionSourceIsaCharacteristicMemoryFormV1::Guarded => {
            ProfilerCompleteStructuralMemoryFormV1::Guarded
        }
        ProductionSourceIsaCharacteristicMemoryFormV1::MatrixTile => {
            ProfilerCompleteStructuralMemoryFormV1::MatrixTile
        }
    }
}

fn project_kir(
    coordinate: fe2o3_hsaco_finalize::ProductionSourceIsaKirCoordinateV1,
) -> ProfilerVariantKirCoordinateV2 {
    ProfilerVariantKirCoordinateV2 {
        function_ordinal: coordinate.function_ordinal(),
        block_ordinal: coordinate.block_ordinal(),
        operation_ordinal: coordinate.operation_ordinal(),
    }
}

fn hash_kir(
    digest: &mut Sha256,
    coordinate: fe2o3_hsaco_finalize::ProductionSourceIsaKirCoordinateV1,
) {
    digest.update(coordinate.function_ordinal().to_le_bytes());
    digest.update(coordinate.block_ordinal().to_le_bytes());
    digest.update(coordinate.operation_ordinal().to_le_bytes());
}

fn hash_characteristic(digest: &mut Sha256, kind: ProfilerCompleteStructuralCharacteristicV1) {
    match kind {
        ProfilerCompleteStructuralCharacteristicV1::GlobalStore { form } => {
            digest.update([1, memory_form_code(form)])
        }
        ProfilerCompleteStructuralCharacteristicV1::WorkgroupLoad { form } => {
            digest.update([2, memory_form_code(form)])
        }
        ProfilerCompleteStructuralCharacteristicV1::WorkgroupStore { form } => {
            digest.update([3, memory_form_code(form)])
        }
        ProfilerCompleteStructuralCharacteristicV1::WorkgroupBarrier => digest.update([4, 0]),
        ProfilerCompleteStructuralCharacteristicV1::Bf16F32M16N16K16Wave64MatrixMultiplyAccumulate => {
            digest.update([5, 0])
        }
    }
}

const fn memory_form_code(form: ProfilerCompleteStructuralMemoryFormV1) -> u8 {
    match form {
        ProfilerCompleteStructuralMemoryFormV1::Plain => 1,
        ProfilerCompleteStructuralMemoryFormV1::Guarded => 2,
        ProfilerCompleteStructuralMemoryFormV1::MatrixTile => 3,
    }
}

const fn side_code(side: ProfilerVariantTreatmentSideV3) -> u8 {
    match side {
        ProfilerVariantTreatmentSideV3::Baseline => 1,
        ProfilerVariantTreatmentSideV3::Candidate => 2,
    }
}

fn evidence_for_catalog(
    catalog: Option<&ProfilerCompleteStructuralCatalogV1>,
) -> Vec<CaptureIdentityV1> {
    catalog.map_or_else(Vec::new, |catalog| {
        vec![catalog.archive_identity.digest, catalog.catalog_identity]
    })
}

fn evidence_for_pair(
    baseline: &ProfilerCompleteStructuralCatalogV1,
    candidate: &ProfilerCompleteStructuralCatalogV1,
) -> Vec<CaptureIdentityV1> {
    let mut evidence = vec![
        baseline.archive_identity.digest,
        baseline.catalog_identity,
        candidate.archive_identity.digest,
        candidate.catalog_identity,
    ];
    sort_dedup(&mut evidence);
    evidence
}

fn unavailable_fact(
    kind: ProfilerCompleteStructuralUnavailableKindV1,
    reason_code: &'static str,
    semantics: &'static str,
    mut evidence_ids: Vec<CaptureIdentityV1>,
) -> ProfilerCompleteStructuralUnavailableV1 {
    sort_dedup(&mut evidence_ids);
    ProfilerCompleteStructuralUnavailableV1 {
        kind,
        reason_code,
        origin: TruthOriginV1::Unavailable,
        semantics,
        evidence_ids,
    }
}

fn content_identity<T: Serialize>(
    domain: &[u8],
    value: &T,
) -> Result<ContentIdentityRecordV1, ProfilerCompleteStructuralErrorV1> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| ProfilerCompleteStructuralErrorV1::IdentityFailure)?;
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(&bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PROFILER_COMPLETE_STRUCTURAL_SCHEMA_VERSION_V1,
        digest: capture(&digest.finalize())?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| ProfilerCompleteStructuralErrorV1::EvidenceTooLarge)?,
    })
}

fn capture(bytes: &[u8]) -> Result<CaptureIdentityV1, ProfilerCompleteStructuralErrorV1> {
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ProfilerCompleteStructuralErrorV1::IdentityFailure)?;
    CaptureIdentityV1::new(bytes).map_err(|_| ProfilerCompleteStructuralErrorV1::IdentityFailure)
}

fn sort_dedup(values: &mut Vec<CaptureIdentityV1>) {
    values.sort_unstable();
    values.dedup();
}

const _: () = assert!(
    MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1 >= MAX_PROFILER_VARIANT_RESULT_BYTES_V3
);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> CaptureIdentityV1 {
        CaptureIdentityV1::new([value; 32]).unwrap()
    }

    fn occurrence(
        side: ProfilerVariantTreatmentSideV3,
        identity: u8,
        key: OccurrenceKeyV1,
        function: u64,
    ) -> ProjectedOccurrenceV1 {
        ProjectedOccurrenceV1 {
            key,
            occurrence: ProfilerCompleteStructuralOccurrenceV1 {
                occurrence_identity: id(identity),
                side,
                target_kir: ProfilerVariantKirCoordinateV2 {
                    function_ordinal: function,
                    block_ordinal: 0,
                    operation_ordinal: 0,
                },
                catalog_correlation_identity: id(identity.wrapping_add(30)),
                catalog_correlation_count: 1,
                evidence_ids: vec![id(identity)],
            },
        }
    }

    fn key(identity: u8) -> OccurrenceKeyV1 {
        OccurrenceKeyV1 {
            characteristic: ProfilerCompleteStructuralCharacteristicV1::WorkgroupBarrier,
            stable_source_mir_set_identity: id(identity),
            stable_source_mir_site_count: 1,
        }
    }

    #[test]
    fn exact_multiset_delta_preserves_every_duplicate_identity() {
        let key = key(9);
        let result = compare_occurrence_multisets(
            id(1),
            vec![
                occurrence(ProfilerVariantTreatmentSideV3::Baseline, 2, key, 0),
                occurrence(ProfilerVariantTreatmentSideV3::Baseline, 3, key, 1),
            ],
            vec![
                occurrence(ProfilerVariantTreatmentSideV3::Candidate, 4, key, 0),
                occurrence(ProfilerVariantTreatmentSideV3::Candidate, 5, key, 1),
                occurrence(ProfilerVariantTreatmentSideV3::Candidate, 6, key, 2),
            ],
        )
        .unwrap();
        assert!(result.removed.is_empty());
        assert_eq!(result.added.len(), 1);
        let delta = &result.added[0];
        assert_eq!(delta.occurrence_delta, 1);
        assert_eq!(delta.baseline_multiplicity, 2);
        assert_eq!(delta.candidate_multiplicity, 3);
        assert_eq!(delta.baseline_occurrences.len(), 2);
        assert_eq!(delta.candidate_occurrences.len(), 3);
        assert_eq!(
            delta.candidate_occurrences[2].target_kir.function_ordinal,
            2
        );
    }

    #[test]
    fn equal_multiplicity_is_not_changed_by_archive_local_identity_substitution() {
        let key = key(9);
        let result = compare_occurrence_multisets(
            id(1),
            vec![occurrence(
                ProfilerVariantTreatmentSideV3::Baseline,
                2,
                key,
                0,
            )],
            vec![occurrence(
                ProfilerVariantTreatmentSideV3::Candidate,
                99,
                key,
                8,
            )],
        )
        .unwrap();
        assert!(result.added.is_empty());
        assert!(result.removed.is_empty());
    }

    #[test]
    fn cross_key_substitution_is_an_added_and_removed_group() {
        let result = compare_occurrence_multisets(
            id(1),
            vec![occurrence(
                ProfilerVariantTreatmentSideV3::Baseline,
                2,
                key(9),
                0,
            )],
            vec![occurrence(
                ProfilerVariantTreatmentSideV3::Candidate,
                3,
                key(10),
                0,
            )],
        )
        .unwrap();
        assert_eq!(result.added.len(), 1);
        assert_eq!(result.removed.len(), 1);
        assert_ne!(
            result.added[0].comparison_key_identity,
            result.removed[0].comparison_key_identity
        );
    }

    #[test]
    fn unavailable_codes_are_stable_and_do_not_claim_execution_or_causality() {
        let schedule = unavailable_fact(
            ProfilerCompleteStructuralUnavailableKindV1::ScheduleExecution,
            "schedule_execution_unavailable",
            "fixture",
            Vec::new(),
        );
        let causality = unavailable_fact(
            ProfilerCompleteStructuralUnavailableKindV1::CausalAttribution,
            "causal_attribution_unavailable",
            "fixture",
            Vec::new(),
        );
        assert_eq!(schedule.reason_code, "schedule_execution_unavailable");
        assert_eq!(causality.reason_code, "causal_attribution_unavailable");
        assert_eq!(schedule.origin, TruthOriginV1::Unavailable);
        assert_eq!(causality.origin, TruthOriginV1::Unavailable);
    }

    #[test]
    fn source_mir_universe_digest_or_count_substitution_is_cross_domain() {
        let catalog = |identity, count| ProfilerCompleteStructuralCatalogV1 {
            side: ProfilerVariantTreatmentSideV3::Baseline,
            archive_identity: ContentIdentityRecordV1 {
                scheme: ContentSchemeV1::DomainSeparatedSha256,
                format_version: 1,
                digest: id(3),
                canonical_len: 1,
            },
            catalog_identity: id(4),
            catalog_record_count: 1,
            classified_target_operation_count: 1,
            retained_correlation_count: 1,
            pre_kir_elimination_count: 0,
            source_map_v2: ProfilerVariantStructuralContentIdentityV3 {
                digest: id(5),
                canonical_len: 40,
            },
            stable_source_mir_universe_identity: id(identity),
            stable_source_mir_site_count: count,
            complete_admitted_catalog_projection: true,
            complete_characteristic_scan: true,
            semantics: "fixture",
        };
        let baseline = catalog(1, 4);
        assert!(same_catalog_domain(&baseline, &catalog(1, 4)));
        let mut different_kir_bound_source_map = catalog(1, 4);
        different_kir_bound_source_map.source_map_v2.digest = id(6);
        assert!(same_catalog_domain(
            &baseline,
            &different_kir_bound_source_map
        ));
        assert!(!same_catalog_domain(&baseline, &catalog(2, 4)));
        assert!(!same_catalog_domain(&baseline, &catalog(1, 5)));
    }

    #[test]
    fn bounds_are_additive_and_do_not_reduce_variant_v3() {
        const {
            assert!(
                MAX_PROFILER_COMPLETE_STRUCTURAL_RESULT_BYTES_V1
                    >= MAX_PROFILER_VARIANT_RESULT_BYTES_V3
            );
        }
        assert_eq!(PROFILER_COMPLETE_STRUCTURAL_SCHEMA_VERSION_V1, 1);
        assert_eq!(MAX_PROFILER_COMPLETE_STRUCTURAL_DELTAS_V1, 4_096);
    }
}

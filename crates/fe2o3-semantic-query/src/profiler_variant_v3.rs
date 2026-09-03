//! Exact production KIR bridge for positive profiler Variant observations.
//!
//! Variant V3 is additive. It recomputes Variant V2 and, when the caller owns
//! admitted production evidence, binds Bundle V4's canonical KIR V7 claim to
//! exact production KIR V8 Characteristic occurrences. The bridge is
//! structural only: it does not prove schedule execution, causality, or that an
//! occurrence absent from a partial capture was added or removed.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use fe2o3_hsaco_finalize::{
    ProductionKirV7BridgePointV1, ProductionKirV7BridgeSiteV1, ProductionKirV7StructuralBridgeV1,
    ProductionSourceIsaCatalogRecordKindV1, ProductionSourceIsaCatalogRecordV1,
    ProductionSourceIsaCatalogTransformationV1, ProductionSourceIsaCatalogV1,
    ProductionSourceIsaCharacteristicCollectionV1,
    readmit_exact_production_source_isa_characteristic_projection_v1,
};
use fe2o3_semantic_import::{
    CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1, TruthOriginV1,
    decode_profiler_bundle_v4,
};
use fe2o3_semantic_trace::{KERNEL_IR_IDENTITY_POLICY_V1, KERNEL_IR_WIRE_VERSION_V7};
use fe2o3_source_isa_observation::characteristic_v1::{
    InertSourceIsaCharacteristicCollectionV1, SourceIsaCharacteristicBindingV1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    MAX_PROFILER_VARIANT_OCCURRENCES_V2, MAX_PROFILER_VARIANT_RESULT_BYTES_V2,
    PcSourceIsaScanAvailabilityV1, PcSourceIsaScanSummaryV1, ProfilerVariantChangeAxisV2,
    ProfilerVariantComparisonRequestV2, ProfilerVariantComparisonV2, ProfilerVariantErrorV2,
    ProfilerVariantKirCoordinateV2, ProfilerVariantLlvmCoordinateV2,
    ProfilerVariantObservationKindV2, ProfilerVariantOccurrenceV2, ProfilerVariantTreatmentInputV2,
    ProfilerVariantUnavailableKindV2, build_profiler_variant_request_v2,
    compare_profiler_variants_v2,
};

pub const PROFILER_VARIANT_SCHEMA_VERSION_V3: u16 = 3;
pub const MAX_PROFILER_VARIANT_STRUCTURAL_BINDINGS_V3: usize = 2;
pub const MAX_PROFILER_VARIANT_STRUCTURAL_OCCURRENCES_V3: usize =
    MAX_PROFILER_VARIANT_OCCURRENCES_V2;
pub const MAX_PROFILER_VARIANT_RESULT_BYTES_V3: u64 = 8 * 1024 * 1024;

const STRUCTURAL_EVIDENCE_IDENTITY_DOMAIN_V3: &[u8] =
    b"fe2o3.profiler-variant.structural-evidence.v3\0";
const REQUEST_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.profiler-variant.request.v3\0";
const OCCURRENCE_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.profiler-variant.occurrence.v3\0";
const CHANGE_IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3.profiler-variant.change.v3\0";

/// Already-admitted producer evidence. Canonical bridge or catalog bytes alone
/// are intentionally insufficient because both decoded wire owners are inert.
#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantProductionKirEvidenceV3<'a> {
    pub bridge: &'a ProductionKirV7StructuralBridgeV1,
    pub catalog: &'a ProductionSourceIsaCatalogV1,
    pub characteristic: &'a ProductionSourceIsaCharacteristicCollectionV1,
}

#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantTreatmentInputV3<'a> {
    pub treatment: ProfilerVariantTreatmentInputV2<'a>,
    pub production_kir: Option<ProfilerVariantProductionKirEvidenceV3<'a>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantComparisonRequestV3 {
    pub schema_version: u16,
    pub comparison_v2: ProfilerVariantComparisonRequestV2,
    pub baseline_structural_evidence: Option<ContentIdentityRecordV1>,
    pub candidate_structural_evidence: Option<ContentIdentityRecordV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantStructuralContentIdentityV3 {
    pub digest: CaptureIdentityV1,
    pub canonical_len: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantTreatmentSideV3 {
    Baseline,
    Candidate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantStructuralBindingV3 {
    pub side: ProfilerVariantTreatmentSideV3,
    pub origin: TruthOriginV1,
    pub evidence_identity: ContentIdentityRecordV1,
    pub profiler_bundle_kir_v7: ProfilerVariantStructuralContentIdentityV3,
    pub production_neutral_kir_v8: ProfilerVariantStructuralContentIdentityV3,
    pub production_target_kir_v8: ProfilerVariantStructuralContentIdentityV3,
    pub source_map_v2: ProfilerVariantStructuralContentIdentityV3,
    pub artifact: ProfilerVariantStructuralContentIdentityV3,
    pub bridge_identity: CaptureIdentityV1,
    pub catalog_identity: CaptureIdentityV1,
    pub structural_identity: CaptureIdentityV1,
    pub correlation_identity: CaptureIdentityV1,
    pub semantic_map_identity: CaptureIdentityV1,
    pub dispatch_claim_count: u64,
    pub structural_record_count: u64,
    pub characteristic_identities: Vec<CaptureIdentityV1>,
    pub complete_characteristic_projection_count: u16,
    pub partial_characteristic_binding_count: u16,
    pub semantics: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantCharacteristicProjectionV3 {
    ExactAdmittedProductionProjection,
    PartialSelfClaimedBindingWithExactPositiveCatalogMatch,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantStructuralOccurrenceV3 {
    pub occurrence_identity: CaptureIdentityV1,
    pub side: ProfilerVariantTreatmentSideV3,
    pub observed_occurrence_identity: CaptureIdentityV1,
    pub observation_kind: ProfilerVariantObservationKindV2,
    pub selector_identity: CaptureIdentityV1,
    pub profiler_kir_v7: ProfilerVariantKirCoordinateV2,
    pub production_neutral_kir_v8: ProfilerVariantKirCoordinateV2,
    pub production_target_kir_v8: ProfilerVariantKirCoordinateV2,
    pub catalog_record_ordinal: u64,
    pub compiler_handoff_llvm: ProfilerVariantLlvmCoordinateV2,
    pub characteristic_projection: ProfilerVariantCharacteristicProjectionV3,
    pub evidence_ids: Vec<CaptureIdentityV1>,
    pub semantics: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantStructuralChangeBasisV3 {
    ExactStructuralPositiveCoObservation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantStructuralChangeV3 {
    pub change_identity: CaptureIdentityV1,
    pub comparison_v2_change_identity: CaptureIdentityV1,
    pub baseline_structural_occurrence_identity: CaptureIdentityV1,
    pub candidate_structural_occurrence_identity: CaptureIdentityV1,
    pub changed_axes: Vec<ProfilerVariantChangeAxisV2>,
    pub basis: ProfilerVariantStructuralChangeBasisV3,
    pub evidence_ids: Vec<CaptureIdentityV1>,
    pub interpretation: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantUnavailableKindV3 {
    BaselineProductionStructuralEvidenceMissing,
    CandidateProductionStructuralEvidenceMissing,
    BaselineCharacteristicScanIncomplete,
    CandidateCharacteristicScanIncomplete,
    IncompleteEvidenceCannotEstablishAdditionOrRemoval,
    ScheduleExecution,
    CausalAttribution,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantUnavailableV3 {
    pub kind: ProfilerVariantUnavailableKindV3,
    pub origin: TruthOriginV1,
    pub reason: String,
    pub evidence_ids: Vec<CaptureIdentityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantComparisonV3 {
    pub schema_version: u16,
    pub request_identity: ContentIdentityRecordV1,
    pub comparison_v2: ProfilerVariantComparisonV2,
    pub structural_bindings: Vec<ProfilerVariantStructuralBindingV3>,
    pub baseline_structural_occurrences: Vec<ProfilerVariantStructuralOccurrenceV3>,
    pub candidate_structural_occurrences: Vec<ProfilerVariantStructuralOccurrenceV3>,
    pub structural_changes: Vec<ProfilerVariantStructuralChangeV3>,
    pub resolved_v2_unavailable: Vec<ProfilerVariantUnavailableKindV2>,
    pub unavailable: Vec<ProfilerVariantUnavailableV3>,
    pub authority: String,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProfilerVariantErrorV3 {
    VariantV2(ProfilerVariantErrorV2),
    InvalidRequest,
    RequestMismatch,
    BundleAdmission,
    BundleKirSubstitution,
    ProductionEvidenceMismatch,
    CharacteristicAdmission,
    CharacteristicSubstitution,
    UnknownStructuralSite,
    CatalogOccurrenceSubstitution,
    AmbiguousCatalogOccurrence,
    EvidenceTooLarge,
    ResultTooLarge,
    IdentityFailure,
}

impl fmt::Display for ProfilerVariantErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "profiler Variant V3 evidence rejected: {self:?}")
    }
}

impl Error for ProfilerVariantErrorV3 {}

pub fn build_profiler_variant_request_v3(
    semantic_workload: &[u8],
    baseline: ProfilerVariantTreatmentInputV3<'_>,
    candidate: ProfilerVariantTreatmentInputV3<'_>,
) -> Result<ProfilerVariantComparisonRequestV3, ProfilerVariantErrorV3> {
    Ok(ProfilerVariantComparisonRequestV3 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V3,
        comparison_v2: build_profiler_variant_request_v2(
            semantic_workload,
            baseline.treatment,
            candidate.treatment,
        )
        .map_err(ProfilerVariantErrorV3::VariantV2)?,
        baseline_structural_evidence: baseline
            .production_kir
            .map(structural_evidence_identity)
            .transpose()?,
        candidate_structural_evidence: candidate
            .production_kir
            .map(structural_evidence_identity)
            .transpose()?,
    })
}

pub fn compare_profiler_variants_v3(
    request: ProfilerVariantComparisonRequestV3,
    baseline: ProfilerVariantTreatmentInputV3<'_>,
    candidate: ProfilerVariantTreatmentInputV3<'_>,
) -> Result<ProfilerVariantComparisonV3, ProfilerVariantErrorV3> {
    if request.schema_version != PROFILER_VARIANT_SCHEMA_VERSION_V3 {
        return Err(ProfilerVariantErrorV3::InvalidRequest);
    }
    if baseline
        .production_kir
        .map(structural_evidence_identity)
        .transpose()?
        != request.baseline_structural_evidence
        || candidate
            .production_kir
            .map(structural_evidence_identity)
            .transpose()?
            != request.candidate_structural_evidence
    {
        return Err(ProfilerVariantErrorV3::RequestMismatch);
    }
    let comparison_v2 = compare_profiler_variants_v2(
        request.comparison_v2,
        baseline.treatment,
        candidate.treatment,
    )
    .map_err(ProfilerVariantErrorV3::VariantV2)?;
    let request_identity = request_identity(request)?;
    let mut structural_bindings = Vec::new();
    let mut unavailable = Vec::new();

    let baseline_structural_occurrences = admit_treatment_structural_evidence(
        ProfilerVariantTreatmentSideV3::Baseline,
        baseline,
        &comparison_v2.baseline_occurrences,
        &comparison_v2.baseline_evidence,
        &mut structural_bindings,
        &mut unavailable,
    )?;
    let candidate_structural_occurrences = admit_treatment_structural_evidence(
        ProfilerVariantTreatmentSideV3::Candidate,
        candidate,
        &comparison_v2.candidate_occurrences,
        &comparison_v2.candidate_evidence,
        &mut structural_bindings,
        &mut unavailable,
    )?;
    let structural_changes = compare_structural_occurrences(
        &comparison_v2,
        &baseline_structural_occurrences,
        &candidate_structural_occurrences,
    )?;
    let resolved_v2_unavailable = if structural_bindings.len() == 2
        && !baseline_structural_occurrences.is_empty()
        && !candidate_structural_occurrences.is_empty()
    {
        vec![ProfilerVariantUnavailableKindV2::ProfilerKirToCharacteristicKirBridgeUnavailable]
    } else {
        Vec::new()
    };

    unavailable.push(unavailable_fact(
        ProfilerVariantUnavailableKindV3::IncompleteEvidenceCannotEstablishAdditionOrRemoval,
        "Variant V3 retains only positive bounded profiler observations; a missing occurrence cannot establish addition or removal unless the underlying capture and Characteristic scan are complete for that claim",
        structural_bindings
            .iter()
            .flat_map(|binding| binding.characteristic_identities.iter().copied())
            .collect(),
    ));
    unavailable.push(unavailable_fact(
        ProfilerVariantUnavailableKindV3::ScheduleExecution,
        "the treatment schedule is content-bound caller evidence, not an observed or authenticated execution schedule",
        Vec::new(),
    ));
    unavailable.push(unavailable_fact(
        ProfilerVariantUnavailableKindV3::CausalAttribution,
        "exact structural joins and paired positive observations do not prove that a changed semantic, KIR, LLVM, ISA, schedule, or resource axis caused a measured delta",
        structural_changes
            .iter()
            .flat_map(|change| change.evidence_ids.iter().copied())
            .collect(),
    ));
    unavailable.sort_by_key(|fact| fact.kind);
    if structural_bindings.len() > MAX_PROFILER_VARIANT_STRUCTURAL_BINDINGS_V3
        || baseline_structural_occurrences.len() > MAX_PROFILER_VARIANT_STRUCTURAL_OCCURRENCES_V3
        || candidate_structural_occurrences.len() > MAX_PROFILER_VARIANT_STRUCTURAL_OCCURRENCES_V3
        || structural_changes.len() > MAX_PROFILER_VARIANT_STRUCTURAL_OCCURRENCES_V3
    {
        return Err(ProfilerVariantErrorV3::EvidenceTooLarge);
    }

    let result = ProfilerVariantComparisonV3 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V3,
        request_identity,
        comparison_v2,
        structural_bindings,
        baseline_structural_occurrences,
        candidate_structural_occurrences,
        structural_changes,
        resolved_v2_unavailable,
        unavailable,
        authority: "read_only_exact_structural_projection_no_execution_attach_scheduling_collection_decoder_load_dispatch_or_publication_authority".to_owned(),
    };
    let encoded =
        serde_json::to_vec(&result).map_err(|_| ProfilerVariantErrorV3::ResultTooLarge)?;
    if encoded.len() as u64 > MAX_PROFILER_VARIANT_RESULT_BYTES_V3 {
        return Err(ProfilerVariantErrorV3::ResultTooLarge);
    }
    Ok(result)
}

fn admit_treatment_structural_evidence(
    side: ProfilerVariantTreatmentSideV3,
    input: ProfilerVariantTreatmentInputV3<'_>,
    occurrences: &[ProfilerVariantOccurrenceV2],
    evidence_bindings: &[crate::ProfilerVariantEvidenceBindingV2],
    structural_bindings: &mut Vec<ProfilerVariantStructuralBindingV3>,
    unavailable: &mut Vec<ProfilerVariantUnavailableV3>,
) -> Result<Vec<ProfilerVariantStructuralOccurrenceV3>, ProfilerVariantErrorV3> {
    let Some(production) = input.production_kir else {
        unavailable.push(unavailable_fact(
            match side {
                ProfilerVariantTreatmentSideV3::Baseline => {
                    ProfilerVariantUnavailableKindV3::BaselineProductionStructuralEvidenceMissing
                }
                ProfilerVariantTreatmentSideV3::Candidate => {
                    ProfilerVariantUnavailableKindV3::CandidateProductionStructuralEvidenceMissing
                }
            },
            "no already-admitted production KIR V7 bridge, Source/ISA catalog, and Characteristic producer projection was supplied for this treatment",
            Vec::new(),
        ));
        return Ok(Vec::new());
    };

    validate_production_pair(production)?;
    let bundle = decode_profiler_bundle_v4(input.treatment.treatment.bundle)
        .map_err(|_| ProfilerVariantErrorV3::BundleAdmission)?;
    let capture = bundle
        .dispatch_capture
        .as_ref()
        .ok_or(ProfilerVariantErrorV3::BundleAdmission)?;
    let v7 = production.bridge.simulator_v7_identity();
    if capture.dispatches.is_empty()
        || capture.dispatches.iter().any(|dispatch| {
            dispatch.kernel_ir.origin != TruthOriginV1::Declared
                || dispatch.kernel_ir.wire_version != KERNEL_IR_WIRE_VERSION_V7
                || dispatch.kernel_ir.identity_policy != KERNEL_IR_IDENTITY_POLICY_V1
                || dispatch.kernel_ir.digest.as_bytes() != v7.sha256()
                || dispatch.kernel_ir.canonical_len != v7.byte_len()
        })
    {
        return Err(ProfilerVariantErrorV3::BundleKirSubstitution);
    }

    let characteristic_inputs = characteristic_inputs(input.treatment);
    let mut characteristic_identities = Vec::new();
    let mut characteristic_projection = BTreeMap::new();
    let mut complete_characteristic_projection_count = 0_u16;
    let mut partial_characteristic_binding_count = 0_u16;
    for (kind, bytes) in characteristic_inputs {
        let inert = InertSourceIsaCharacteristicCollectionV1::decode_canonical(bytes)
            .map_err(|_| ProfilerVariantErrorV3::CharacteristicAdmission)?;
        validate_characteristic_binding(inert.claimed_binding(), production)?;
        let binding = evidence_bindings
            .iter()
            .find(|binding| binding.observation_kind == kind)
            .ok_or(ProfilerVariantErrorV3::CharacteristicSubstitution)?;
        if binding.characteristic_scan.availability == PcSourceIsaScanAvailabilityV1::Complete {
            let exact = readmit_exact_production_source_isa_characteristic_projection_v1(
                inert,
                production.characteristic,
            )
            .map_err(|_| ProfilerVariantErrorV3::CharacteristicSubstitution)?;
            characteristic_identities.push(identity(&exact.identity())?);
            characteristic_projection.insert(
                kind,
                ProfilerVariantCharacteristicProjectionV3::ExactAdmittedProductionProjection,
            );
            complete_characteristic_projection_count = complete_characteristic_projection_count
                .checked_add(1)
                .ok_or(ProfilerVariantErrorV3::EvidenceTooLarge)?;
        } else {
            let partial = inert.into_self_claimed_archive_for_agent_inspection_v1();
            characteristic_identities.push(identity(&partial.identity())?);
            characteristic_projection.insert(
                kind,
                ProfilerVariantCharacteristicProjectionV3::PartialSelfClaimedBindingWithExactPositiveCatalogMatch,
            );
            partial_characteristic_binding_count = partial_characteristic_binding_count
                .checked_add(1)
                .ok_or(ProfilerVariantErrorV3::EvidenceTooLarge)?;
            unavailable.push(scan_unavailable(
                side,
                binding.characteristic_scan,
                vec![binding.binding_identity, binding.characteristic_identity],
            ));
        }
    }
    characteristic_identities.sort_unstable();
    characteristic_identities.dedup();

    let evidence_identity = structural_evidence_identity(production)?;
    structural_bindings.push(ProfilerVariantStructuralBindingV3 {
        side,
        origin: TruthOriginV1::Inferred,
        evidence_identity,
        profiler_bundle_kir_v7: structural_content(v7.sha256(), v7.byte_len())?,
        production_neutral_kir_v8: structural_content(
            production.bridge.neutral_production_identity().sha256(),
            production.bridge.neutral_production_identity().byte_len(),
        )?,
        production_target_kir_v8: structural_content(
            production.bridge.target_production_identity().sha256(),
            production.bridge.target_production_identity().byte_len(),
        )?,
        source_map_v2: structural_content(
            production.bridge.source_map_v2_identity().sha256(),
            production.bridge.source_map_v2_identity().byte_len(),
        )?,
        artifact: structural_content(
            production.bridge.artifact_identity().sha256(),
            production.bridge.artifact_identity().byte_len(),
        )?,
        bridge_identity: identity(production.bridge.identity())?,
        catalog_identity: identity(production.catalog.identity())?,
        structural_identity: identity(production.bridge.structural_identity())?,
        correlation_identity: identity(production.bridge.correlation_identity())?,
        semantic_map_identity: identity(production.bridge.semantic_map_identity())?,
        dispatch_claim_count: u64::try_from(capture.dispatches.len())
            .map_err(|_| ProfilerVariantErrorV3::EvidenceTooLarge)?,
        structural_record_count: u64::try_from(production.bridge.records().len())
            .map_err(|_| ProfilerVariantErrorV3::EvidenceTooLarge)?,
        characteristic_identities,
        complete_characteristic_projection_count,
        partial_characteristic_binding_count,
        semantics: "exact Bundle V4 KIR V7 content claim joined through an already-admitted identity-coordinate production bridge to KIR V8, source map, artifact, catalog, correlation, and semantic-map identities; complete Characteristic archives are re-admitted against producer evidence, while partial archives can contribute only individually catalog-matched positive structural occurrences".to_owned(),
    });

    let mut output = Vec::new();
    output
        .try_reserve_exact(occurrences.len())
        .map_err(|_| ProfilerVariantErrorV3::EvidenceTooLarge)?;
    for occurrence in occurrences {
        let projection = characteristic_projection
            .get(&occurrence.observation_kind)
            .copied()
            .ok_or(ProfilerVariantErrorV3::CharacteristicSubstitution)?;
        output.push(admit_structural_occurrence(
            side, occurrence, production, projection,
        )?);
    }
    output.sort_by_key(|occurrence| occurrence.occurrence_identity);
    if output
        .windows(2)
        .any(|pair| pair[0].occurrence_identity == pair[1].occurrence_identity)
    {
        return Err(ProfilerVariantErrorV3::AmbiguousCatalogOccurrence);
    }
    Ok(output)
}

pub(crate) fn validate_production_pair(
    evidence: ProfilerVariantProductionKirEvidenceV3<'_>,
) -> Result<(), ProfilerVariantErrorV3> {
    let characteristic = evidence.characteristic;
    let structural = evidence.catalog.structural_binding();
    let characteristic_counts = characteristic.structural_counts();
    if evidence.catalog.identity() != evidence.bridge.catalog_identity()
        || evidence.catalog.correlation_identity() != evidence.bridge.correlation_identity()
        || evidence.catalog.semantic_map_identity() != evidence.bridge.semantic_map_identity()
        || structural.identity() != *evidence.bridge.structural_identity()
        || structural.counts() != evidence.bridge.structural_counts()
        || characteristic.catalog_identity() != evidence.catalog.identity()
        || characteristic.structural_bridge_identity() != evidence.bridge.identity()
        || characteristic.structural_binding_identity() != evidence.bridge.structural_identity()
        || characteristic.correlation_identity() != evidence.bridge.correlation_identity()
        || characteristic.semantic_map_identity() != evidence.bridge.semantic_map_identity()
        || characteristic_counts.functions() != evidence.bridge.structural_counts().functions()
        || characteristic_counts.defined_bodies()
            != evidence.bridge.structural_counts().defined_bodies()
        || characteristic_counts.blocks() != evidence.bridge.structural_counts().blocks()
        || characteristic_counts.operations() != evidence.bridge.structural_counts().operations()
    {
        return Err(ProfilerVariantErrorV3::ProductionEvidenceMismatch);
    }
    let artifact = evidence.catalog.artifact_identity();
    let bridge_artifact = evidence.bridge.artifact_identity();
    let source_map = evidence.catalog.source_map_v2_identity();
    let bridge_source_map = evidence.bridge.source_map_v2_identity();
    if artifact.sha256() != &bridge_artifact.sha256()
        || artifact.byte_len() != bridge_artifact.byte_len()
        || source_map.sha256() != bridge_source_map.sha256()
        || source_map.byte_len() != bridge_source_map.byte_len()
        || characteristic.source_map_v2_identity().sha256() != bridge_source_map.sha256()
        || characteristic.source_map_v2_identity().byte_len() != bridge_source_map.byte_len()
        || characteristic.artifact_identity() != artifact
        || characteristic.neutral_kir_identity().sha256()
            != evidence.bridge.neutral_production_identity().sha256()
        || characteristic.neutral_kir_identity().byte_len()
            != evidence.bridge.neutral_production_identity().byte_len()
        || characteristic.target_kir_identity().sha256()
            != evidence.bridge.target_production_identity().sha256()
        || characteristic.target_kir_identity().byte_len()
            != evidence.bridge.target_production_identity().byte_len()
    {
        return Err(ProfilerVariantErrorV3::ProductionEvidenceMismatch);
    }
    Ok(())
}

fn validate_characteristic_binding(
    binding: SourceIsaCharacteristicBindingV1,
    evidence: ProfilerVariantProductionKirEvidenceV3<'_>,
) -> Result<(), ProfilerVariantErrorV3> {
    let structural = evidence.catalog.structural_binding();
    let bridge = evidence.bridge;
    let artifact = bridge.artifact_identity();
    let counts = binding.structural_counts();
    if binding.kir_version().code() != 8
        || binding.structural_identity() != *bridge.structural_identity()
        || counts.functions != bridge.structural_counts().functions()
        || counts.defined_bodies != bridge.structural_counts().defined_bodies()
        || counts.blocks != bridge.structural_counts().blocks()
        || counts.operations != bridge.structural_counts().operations()
        || !same_characteristic_content(
            binding.source_map_v2(),
            bridge.source_map_v2_identity().sha256(),
            bridge.source_map_v2_identity().byte_len(),
        )
        || !same_characteristic_content(
            binding.neutral_kir(),
            bridge.neutral_production_identity().sha256(),
            bridge.neutral_production_identity().byte_len(),
        )
        || !same_characteristic_content(
            binding.target_kir(),
            bridge.target_production_identity().sha256(),
            bridge.target_production_identity().byte_len(),
        )
        || !same_characteristic_content(binding.artifact(), artifact.sha256(), artifact.byte_len())
        || !same_characteristic_content(
            binding.catalog(),
            *evidence.catalog.identity(),
            evidence
                .catalog
                .canonical_byte_len()
                .map_err(|_| ProfilerVariantErrorV3::ProductionEvidenceMismatch)?,
        )
        || !same_characteristic_content(
            binding.structural_bridge(),
            *bridge.identity(),
            bridge
                .canonical_byte_len()
                .map_err(|_| ProfilerVariantErrorV3::ProductionEvidenceMismatch)?,
        )
        || binding.correlation_identity() != *bridge.correlation_identity()
        || binding.semantic_map_identity() != *bridge.semantic_map_identity()
        || structural.neutral_kernel_ir().sha256() != bridge.neutral_production_identity().sha256()
        || structural.target_bound_kernel_ir().sha256()
            != bridge.target_production_identity().sha256()
    {
        return Err(ProfilerVariantErrorV3::CharacteristicSubstitution);
    }
    Ok(())
}

fn same_characteristic_content(
    actual: fe2o3_source_isa_observation::characteristic_v1::SourceIsaCharacteristicContentIdentityV1,
    digest: [u8; 32],
    length: u64,
) -> bool {
    actual.sha256() == digest && actual.byte_len() == length
}

fn admit_structural_occurrence(
    side: ProfilerVariantTreatmentSideV3,
    occurrence: &ProfilerVariantOccurrenceV2,
    evidence: ProfilerVariantProductionKirEvidenceV3<'_>,
    characteristic_projection: ProfilerVariantCharacteristicProjectionV3,
) -> Result<ProfilerVariantStructuralOccurrenceV3, ProfilerVariantErrorV3> {
    let site = ProductionKirV7BridgeSiteV1::operation(
        occurrence.target_kir.function_ordinal,
        occurrence.target_kir.block_ordinal,
        occurrence.target_kir.operation_ordinal,
    );
    let record = evidence
        .bridge
        .query_simulator_v7(site)
        .map_err(|_| ProfilerVariantErrorV3::UnknownStructuralSite)?;
    if record.simulator_v7() != site
        || record.neutral_production() != site
        || record.target_production() != site
        || !matches!(
            record.target_production().point(),
            ProductionKirV7BridgePointV1::Operation { operation_ordinal }
                if operation_ordinal == occurrence.target_kir.operation_ordinal
        )
    {
        return Err(ProfilerVariantErrorV3::UnknownStructuralSite);
    }
    let matches = evidence
        .bridge
        .query_target_catalog(evidence.catalog, site)
        .map_err(|_| ProfilerVariantErrorV3::UnknownStructuralSite)?;
    let mut matching_ordinals = Vec::new();
    for candidate in matches {
        if catalog_record_matches_occurrence(candidate, occurrence) {
            let ordinal = evidence
                .catalog
                .records()
                .iter()
                .position(|record| std::ptr::eq(record, candidate))
                .ok_or(ProfilerVariantErrorV3::CatalogOccurrenceSubstitution)?;
            matching_ordinals.push(
                u64::try_from(ordinal).map_err(|_| ProfilerVariantErrorV3::EvidenceTooLarge)?,
            );
        }
    }
    let catalog_record_ordinal = select_unique_catalog_ordinal(&matching_ordinals)?;
    let mut evidence_ids = occurrence.evidence_ids.clone();
    evidence_ids.extend([
        identity(evidence.bridge.identity())?,
        identity(evidence.catalog.identity())?,
        identity(evidence.bridge.structural_identity())?,
        identity(evidence.bridge.correlation_identity())?,
        identity(evidence.bridge.semantic_map_identity())?,
    ]);
    sort_dedup(&mut evidence_ids);
    let occurrence_identity = structural_occurrence_identity(
        side,
        occurrence.occurrence_identity,
        evidence.bridge.identity(),
        evidence.catalog.identity(),
        catalog_record_ordinal,
    )?;
    Ok(ProfilerVariantStructuralOccurrenceV3 {
        occurrence_identity,
        side,
        observed_occurrence_identity: occurrence.occurrence_identity,
        observation_kind: occurrence.observation_kind,
        selector_identity: occurrence.selector_identity,
        profiler_kir_v7: occurrence.target_kir,
        production_neutral_kir_v8: occurrence.target_kir,
        production_target_kir_v8: occurrence.target_kir,
        catalog_record_ordinal,
        compiler_handoff_llvm: occurrence.compiler_handoff_llvm,
        characteristic_projection,
        evidence_ids,
        semantics: "exact coordinate identity through the admitted V7-to-V8 bridge and a unique exact catalog record matching this positive Characteristic occurrence; not execution completeness or causality".to_owned(),
    })
}

fn select_unique_catalog_ordinal(matching_ordinals: &[u64]) -> Result<u64, ProfilerVariantErrorV3> {
    match matching_ordinals {
        [] => Err(ProfilerVariantErrorV3::CatalogOccurrenceSubstitution),
        [ordinal] => Ok(*ordinal),
        _ => Err(ProfilerVariantErrorV3::AmbiguousCatalogOccurrence),
    }
}

fn catalog_record_matches_occurrence(
    record: &ProductionSourceIsaCatalogRecordV1,
    occurrence: &ProfilerVariantOccurrenceV2,
) -> bool {
    record_kind_code(record.kind()) == occurrence.record_kind_code
        && record.source_node_identity()
            == occurrence
                .stable_source_mir_site
                .as_ref()
                .map(|site| site.source_node_identity.as_bytes())
        && record
            .source_span()
            .map(|span| (span.file_identity(), span.byte_start(), span.byte_end()))
            == occurrence.stable_source_mir_site.as_ref().map(|site| {
                (
                    site.file_identity.as_bytes(),
                    site.byte_start,
                    site.byte_end,
                )
            })
        && record.mir_node_identity()
            == occurrence
                .stable_source_mir_site
                .as_ref()
                .map(|site| site.mir_node_identity.as_bytes())
        && record.mir().map(|coordinate| {
            (
                coordinate.body_ordinal(),
                coordinate.block_ordinal(),
                coordinate.statement_ordinal(),
            )
        }) == occurrence.stable_source_mir_site.as_ref().map(|site| {
            (
                site.mir_body_ordinal,
                site.mir_block_ordinal,
                site.mir_statement_ordinal,
            )
        })
        && record.neutral_kir_node_identity()
            == occurrence
                .neutral_kir_node_identity
                .as_ref()
                .map(|value| value.as_bytes())
        && record.neutral_kir().map(project_catalog_kir) == occurrence.neutral_kir
        && record.target_kir().map(project_catalog_kir) == Some(occurrence.target_kir)
        && record.semantic_operation_id() == Some(occurrence.semantic_operation_identity.as_bytes())
        && record
            .compiler_handoff_llvm()
            .map(|coordinate| ProfilerVariantLlvmCoordinateV2 {
                function_ordinal: coordinate.function_ordinal(),
                block_ordinal: coordinate.block_ordinal(),
                instruction_ordinal: coordinate.instruction_ordinal(),
            })
            == Some(occurrence.compiler_handoff_llvm)
        && record.isa().iter().any(|interval| {
            interval.kernel_ordinal() == occurrence.isa.kernel_ordinal
                && interval.byte_start() == occurrence.isa.symbol_relative_start
                && interval.byte_end() == occurrence.isa.symbol_relative_end
        })
        && record.transformation().map(transformation_code) == Some(occurrence.transformation_code)
}

fn project_catalog_kir(
    coordinate: fe2o3_hsaco_finalize::ProductionSourceIsaKirCoordinateV1,
) -> ProfilerVariantKirCoordinateV2 {
    ProfilerVariantKirCoordinateV2 {
        function_ordinal: coordinate.function_ordinal(),
        block_ordinal: coordinate.block_ordinal(),
        operation_ordinal: coordinate.operation_ordinal(),
    }
}

const fn record_kind_code(kind: ProductionSourceIsaCatalogRecordKindV1) -> u8 {
    match kind {
        ProductionSourceIsaCatalogRecordKindV1::EliminatedBeforeKir => 1,
        ProductionSourceIsaCatalogRecordKindV1::SourceAnchored => 2,
        ProductionSourceIsaCatalogRecordKindV1::NoSourceProvenance => 3,
    }
}

const fn transformation_code(value: ProductionSourceIsaCatalogTransformationV1) -> u8 {
    match value {
        ProductionSourceIsaCatalogTransformationV1::Preserved => 1,
        ProductionSourceIsaCatalogTransformationV1::Duplicated => 2,
        ProductionSourceIsaCatalogTransformationV1::Coalesced => 3,
        ProductionSourceIsaCatalogTransformationV1::DuplicatedAndCoalesced => 4,
        ProductionSourceIsaCatalogTransformationV1::Eliminated => 5,
    }
}

fn compare_structural_occurrences(
    comparison: &ProfilerVariantComparisonV2,
    baseline: &[ProfilerVariantStructuralOccurrenceV3],
    candidate: &[ProfilerVariantStructuralOccurrenceV3],
) -> Result<Vec<ProfilerVariantStructuralChangeV3>, ProfilerVariantErrorV3> {
    let baseline = baseline
        .iter()
        .map(|value| (value.observed_occurrence_identity, value))
        .collect::<BTreeMap<_, _>>();
    let candidate = candidate
        .iter()
        .map(|value| (value.observed_occurrence_identity, value))
        .collect::<BTreeMap<_, _>>();
    let mut output = Vec::new();
    for change in &comparison.changed_occurrences {
        let (Some(left), Some(right)) = (
            baseline.get(&change.baseline_occurrence_identity),
            candidate.get(&change.candidate_occurrence_identity),
        ) else {
            continue;
        };
        let mut evidence_ids = left
            .evidence_ids
            .iter()
            .chain(&right.evidence_ids)
            .chain(&change.evidence_ids)
            .copied()
            .collect::<Vec<_>>();
        sort_dedup(&mut evidence_ids);
        let mut changed_axes = change.changed_axes.clone();
        if left.characteristic_projection
            != ProfilerVariantCharacteristicProjectionV3::ExactAdmittedProductionProjection
            || right.characteristic_projection
                != ProfilerVariantCharacteristicProjectionV3::ExactAdmittedProductionProjection
        {
            // Category/kind classification is supplied by Characteristic V1,
            // not by the structural catalog. A partial self-claim can support
            // its uniquely matching structural fields but cannot upgrade a
            // classification-only delta to production evidence.
            changed_axes.retain(|axis| *axis != ProfilerVariantChangeAxisV2::Classification);
        }
        if changed_axes.is_empty() {
            continue;
        }
        output.push(ProfilerVariantStructuralChangeV3 {
            change_identity: structural_change_identity(change.change_identity, left, right)?,
            comparison_v2_change_identity: change.change_identity,
            baseline_structural_occurrence_identity: left.occurrence_identity,
            candidate_structural_occurrence_identity: right.occurrence_identity,
            changed_axes,
            basis: ProfilerVariantStructuralChangeBasisV3::ExactStructuralPositiveCoObservation,
            evidence_ids,
            interpretation: "changed axes from paired positive observations whose KIR coordinates independently resolve through exact admitted production bridges and unique catalog records; not causal attribution, superiority, or an add/remove classification".to_owned(),
        });
    }
    output.sort_by_key(|value| value.change_identity);
    Ok(output)
}

fn characteristic_inputs(
    treatment: ProfilerVariantTreatmentInputV2<'_>,
) -> Vec<(ProfilerVariantObservationKindV2, &[u8])> {
    let mut output = Vec::new();
    if let Some(evidence) = treatment.pc_source_isa {
        output.push((
            ProfilerVariantObservationKindV2::PcSample,
            evidence.characteristic,
        ));
    }
    if let Some(evidence) = treatment.decoded_att_source_isa {
        output.push((
            ProfilerVariantObservationKindV2::DecodedAtt,
            evidence.characteristic,
        ));
    }
    output
}

fn scan_unavailable(
    side: ProfilerVariantTreatmentSideV3,
    scan: PcSourceIsaScanSummaryV1,
    evidence_ids: Vec<CaptureIdentityV1>,
) -> ProfilerVariantUnavailableV3 {
    unavailable_fact(
        match side {
            ProfilerVariantTreatmentSideV3::Baseline => {
                ProfilerVariantUnavailableKindV3::BaselineCharacteristicScanIncomplete
            }
            ProfilerVariantTreatmentSideV3::Candidate => {
                ProfilerVariantUnavailableKindV3::CandidateCharacteristicScanIncomplete
            }
        },
        &format!(
            "Characteristic scan is {:?}: scanned {}/{} catalog records and {}/{} target operations; positive exact matches remain usable but absence is not meaningful",
            scan.availability,
            scan.catalog_records_scanned,
            scan.catalog_record_count,
            scan.target_operations_scanned,
            scan.target_operation_count,
        ),
        evidence_ids,
    )
}

fn structural_evidence_identity(
    evidence: ProfilerVariantProductionKirEvidenceV3<'_>,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV3> {
    validate_production_pair(evidence)?;
    let mut digest = Sha256::new();
    digest.update(STRUCTURAL_EVIDENCE_IDENTITY_DOMAIN_V3);
    digest.update(evidence.bridge.identity());
    digest.update(
        evidence
            .bridge
            .canonical_byte_len()
            .map_err(|_| ProfilerVariantErrorV3::ProductionEvidenceMismatch)?
            .to_le_bytes(),
    );
    digest.update(evidence.catalog.identity());
    digest.update(
        evidence
            .catalog
            .canonical_byte_len()
            .map_err(|_| ProfilerVariantErrorV3::ProductionEvidenceMismatch)?
            .to_le_bytes(),
    );
    digest.update(evidence.characteristic.structural_bridge_identity());
    digest.update(evidence.characteristic.catalog_identity());
    digest.update(evidence.characteristic.source_map_v2_identity().sha256());
    digest.update(evidence.characteristic.artifact_identity().sha256());
    // Canonical length describes the exact identity preimage above, excluding
    // the domain separator: two (digest, length) pairs and four digests.
    let canonical_len = (2_u64 * (32 + 8)) + (4 * 32);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PROFILER_VARIANT_SCHEMA_VERSION_V3,
        digest: identity(&digest.finalize())?,
        canonical_len,
    })
}

fn request_identity(
    request: ProfilerVariantComparisonRequestV3,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV3> {
    let bytes = serde_json::to_vec(&request).map_err(|_| ProfilerVariantErrorV3::InvalidRequest)?;
    let mut digest = Sha256::new();
    digest.update(REQUEST_IDENTITY_DOMAIN_V3);
    digest.update(&bytes);
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PROFILER_VARIANT_SCHEMA_VERSION_V3,
        digest: identity(&digest.finalize())?,
        canonical_len: u64::try_from(bytes.len())
            .map_err(|_| ProfilerVariantErrorV3::EvidenceTooLarge)?,
    })
}

fn structural_content(
    digest: [u8; 32],
    canonical_len: u64,
) -> Result<ProfilerVariantStructuralContentIdentityV3, ProfilerVariantErrorV3> {
    Ok(ProfilerVariantStructuralContentIdentityV3 {
        digest: identity(&digest)?,
        canonical_len,
    })
}

fn structural_occurrence_identity(
    side: ProfilerVariantTreatmentSideV3,
    occurrence: CaptureIdentityV1,
    bridge: &[u8; 32],
    catalog: &[u8; 32],
    catalog_record_ordinal: u64,
) -> Result<CaptureIdentityV1, ProfilerVariantErrorV3> {
    let mut digest = Sha256::new();
    digest.update(OCCURRENCE_IDENTITY_DOMAIN_V3);
    digest.update([match side {
        ProfilerVariantTreatmentSideV3::Baseline => 0,
        ProfilerVariantTreatmentSideV3::Candidate => 1,
    }]);
    digest.update(occurrence.as_bytes());
    digest.update(bridge);
    digest.update(catalog);
    digest.update(catalog_record_ordinal.to_le_bytes());
    identity(&digest.finalize())
}

fn structural_change_identity(
    v2_change: CaptureIdentityV1,
    baseline: &ProfilerVariantStructuralOccurrenceV3,
    candidate: &ProfilerVariantStructuralOccurrenceV3,
) -> Result<CaptureIdentityV1, ProfilerVariantErrorV3> {
    let mut digest = Sha256::new();
    digest.update(CHANGE_IDENTITY_DOMAIN_V3);
    digest.update(v2_change.as_bytes());
    digest.update(baseline.occurrence_identity.as_bytes());
    digest.update(candidate.occurrence_identity.as_bytes());
    identity(&digest.finalize())
}

fn unavailable_fact(
    kind: ProfilerVariantUnavailableKindV3,
    reason: &str,
    mut evidence_ids: Vec<CaptureIdentityV1>,
) -> ProfilerVariantUnavailableV3 {
    sort_dedup(&mut evidence_ids);
    ProfilerVariantUnavailableV3 {
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

fn identity(bytes: &[u8]) -> Result<CaptureIdentityV1, ProfilerVariantErrorV3> {
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| ProfilerVariantErrorV3::IdentityFailure)?;
    CaptureIdentityV1::new(bytes).map_err(|_| ProfilerVariantErrorV3::IdentityFailure)
}

const _: () = assert!(MAX_PROFILER_VARIANT_RESULT_BYTES_V3 >= MAX_PROFILER_VARIANT_RESULT_BYTES_V2);

#[cfg(test)]
mod tests {
    use super::*;

    fn id(value: u8) -> CaptureIdentityV1 {
        CaptureIdentityV1::new([value; 32]).unwrap()
    }

    #[test]
    fn hostile_occurrence_substitution_is_rejected() {
        assert_eq!(
            select_unique_catalog_ordinal(&[]),
            Err(ProfilerVariantErrorV3::CatalogOccurrenceSubstitution)
        );
        assert_eq!(select_unique_catalog_ordinal(&[7]), Ok(7),);
    }

    #[test]
    fn structural_identity_binds_side_and_catalog_ordinal() {
        let baseline = structural_occurrence_identity(
            ProfilerVariantTreatmentSideV3::Baseline,
            id(1),
            &[2; 32],
            &[3; 32],
            4,
        )
        .unwrap();
        let candidate = structural_occurrence_identity(
            ProfilerVariantTreatmentSideV3::Candidate,
            id(1),
            &[2; 32],
            &[3; 32],
            4,
        )
        .unwrap();
        let substituted = structural_occurrence_identity(
            ProfilerVariantTreatmentSideV3::Baseline,
            id(1),
            &[2; 32],
            &[3; 32],
            5,
        )
        .unwrap();
        assert_ne!(baseline, candidate);
        assert_ne!(baseline, substituted);
    }

    #[test]
    fn ambiguity_is_rejected_instead_of_selecting_a_match() {
        assert_eq!(
            select_unique_catalog_ordinal(&[4, 9]),
            Err(ProfilerVariantErrorV3::AmbiguousCatalogOccurrence)
        );
    }

    #[test]
    fn partial_scan_remains_typed_unavailable() {
        let fact = scan_unavailable(
            ProfilerVariantTreatmentSideV3::Baseline,
            PcSourceIsaScanSummaryV1 {
                availability: PcSourceIsaScanAvailabilityV1::Unavailable,
                reason_code: Some(7),
                reason: Some("partial fixture"),
                catalog_record_count: 10,
                catalog_records_scanned: 3,
                target_operation_count: 8,
                target_operations_scanned: 2,
                retained_target_correlations: 1,
                target_eliminated_correlations: 0,
                correlations_without_source_provenance: 0,
                pre_kir_eliminations: 0,
            },
            vec![id(1)],
        );
        assert_eq!(
            fact.kind,
            ProfilerVariantUnavailableKindV3::BaselineCharacteristicScanIncomplete
        );
        assert!(fact.reason.contains("3/10"));
        assert_eq!(fact.origin, TruthOriginV1::Unavailable);
    }

    #[test]
    fn result_bound_dominates_v2_without_removing_v2_bound() {
        const {
            assert!(MAX_PROFILER_VARIANT_RESULT_BYTES_V3 >= MAX_PROFILER_VARIANT_RESULT_BYTES_V2);
        }
        assert_eq!(MAX_PROFILER_VARIANT_STRUCTURAL_OCCURRENCES_V3, 512);
        assert_eq!(PROFILER_VARIANT_SCHEMA_VERSION_V3, 3);
    }
}

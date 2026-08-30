//! Strict, content-bound comparison of two profiler treatment variants.
//!
//! This is deliberately separate from the Bundle V4 comparator. Bundle V4
//! answers whether two captures describe the same compiled artifact. A variant
//! comparison instead fixes the semantic workload and capture environment while
//! permitting KIR, schedule, artifact, ISA projection, and static resources to
//! differ. It reports co-observed changes, never causation or superiority.

use std::error::Error;
use std::fmt;

use fe2o3_hsaco::inspect;
use fe2o3_semantic_import::{
    CaptureDispatchV1, CaptureIdentityV1, ContentIdentityRecordV1, ContentSchemeV1,
    CounterDefinitionV2, CounterDispatchV2, IdentityFactV1, ImportLimitsV1, KernelIrClaimRecordV1,
    SemanticCounterCaptureV2, SemanticProfilerBundleV4, TruthOriginV1,
    counter_capture_content_identity_v2, decode_counter_capture_v2, decode_pc_sample_capture_v3,
    decode_profiler_bundle_v4, pc_sample_capture_content_identity_v3,
    profiler_bundle_content_identity_v4, rocprofv3_json_source_content_identity_v1,
    validate_rocprofv3_bundle_raw_source_relation_v1,
    validate_rocprofv3_counter_bundle_relation_v1, validate_rocprofv3_pc_bundle_relation_v1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PROFILER_VARIANT_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_PROFILER_VARIANT_MANIFEST_BYTES_V1: u64 = 16 * 1024;
pub const MAX_PROFILER_VARIANT_OPAQUE_EVIDENCE_BYTES_V1: u64 = 16 * 1024 * 1024;
pub const MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1: u64 = 64 * 1024 * 1024;
pub const MAX_PROFILER_VARIANT_DISPATCHES_V1: usize = 256;
pub const MAX_PROFILER_VARIANT_COUNTER_VALUES_V1: usize = 512;
pub const MAX_PROFILER_VARIANT_RESULT_BYTES_V1: u64 = 1024 * 1024;

const WORKLOAD_DOMAIN_V1: &[u8] = b"fe2o3.profiler-variant.semantic-workload.v1\0";
const SCHEDULE_DOMAIN_V1: &[u8] = b"fe2o3.profiler-variant.schedule.v1\0";
const ISA_DOMAIN_V1: &[u8] = b"fe2o3.profiler-variant.isa-projection.v1\0";
const MANIFEST_DOMAIN_V1: &[u8] = b"fe2o3.profiler-variant.treatment-manifest.v1\0";
const REQUEST_DOMAIN_V1: &[u8] = b"fe2o3.profiler-variant.request.v1\0";
const RESOURCE_DOMAIN_V1: &[u8] = b"fe2o3.profiler-variant.static-resources.v1\0";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantTreatmentManifestV1 {
    pub schema_version: u16,
    pub semantic_workload: ContentIdentityRecordV1,
    pub raw_profiler_source: ContentIdentityRecordV1,
    pub bundle: ContentIdentityRecordV1,
    pub schedule: ContentIdentityRecordV1,
    pub artifact: ContentIdentityRecordV1,
    pub kernel_ordinal: u32,
    pub isa_projection: Option<ContentIdentityRecordV1>,
    pub counters: Option<ContentIdentityRecordV1>,
    pub pc_samples: Option<ContentIdentityRecordV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantComparisonRequestV1 {
    pub schema_version: u16,
    pub semantic_workload: ContentIdentityRecordV1,
    pub baseline_manifest: ContentIdentityRecordV1,
    pub candidate_manifest: ContentIdentityRecordV1,
}

/// Exact byte inputs for one treatment. The manifest binds every optional
/// input; `None` and `Some` cannot be substituted after the request is signed.
#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantTreatmentInputV1<'a> {
    pub manifest: &'a [u8],
    pub semantic_workload: &'a [u8],
    pub raw_profiler_source: &'a [u8],
    pub bundle: &'a [u8],
    pub schedule: &'a [u8],
    pub artifact: &'a [u8],
    pub isa_projection: Option<&'a [u8]>,
    pub counters: Option<&'a [u8]>,
    pub pc_samples: Option<&'a [u8]>,
}

/// Inputs used to create a canonical, content-bound treatment manifest.
#[derive(Clone, Copy, Debug)]
pub struct ProfilerVariantManifestInputV1<'a> {
    pub semantic_workload: &'a [u8],
    pub raw_profiler_source: &'a [u8],
    pub bundle: &'a [u8],
    pub schedule: &'a [u8],
    pub artifact: &'a [u8],
    pub kernel_ordinal: u32,
    pub isa_projection: Option<&'a [u8]>,
    pub counters: Option<&'a [u8]>,
    pub pc_samples: Option<&'a [u8]>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantCompatibilityAxisV1 {
    SemanticWorkload,
    Environment,
    CollectorTool,
    CollectorConfiguration,
    StableDevices,
    DispatchWorkloadAndLaunch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantCompatibilityStatusV1 {
    Exact,
    Mismatch,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantCompatibilityFactV1 {
    pub axis: ProfilerVariantCompatibilityAxisV1,
    pub status: ProfilerVariantCompatibilityStatusV1,
    pub origin: TruthOriginV1,
    pub evidence: Vec<ProfilerVariantEvidenceV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantEvidenceRoleV1 {
    SemanticWorkload,
    BaselineRawProfilerSource,
    CandidateRawProfilerSource,
    BaselineManifest,
    CandidateManifest,
    BaselineBundle,
    CandidateBundle,
    BaselineSchedule,
    CandidateSchedule,
    BaselineArtifact,
    CandidateArtifact,
    BaselineStaticResources,
    CandidateStaticResources,
    BaselineCounterCapture,
    CandidateCounterCapture,
    BaselinePcCapture,
    CandidatePcCapture,
    BaselineDispatch,
    CandidateDispatch,
    BaselineCounterValue,
    CandidateCounterValue,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantEvidenceV1 {
    pub role: ProfilerVariantEvidenceRoleV1,
    pub origin: TruthOriginV1,
    pub identity: CaptureIdentityV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerStaticResourcesV1 {
    pub origin: TruthOriginV1,
    pub artifact: ContentIdentityRecordV1,
    pub kernel_ordinal: u32,
    pub kernarg_segment_size: u64,
    pub kernarg_segment_alignment: u64,
    pub group_segment_fixed_size: u64,
    pub private_segment_fixed_size: u64,
    pub wavefront_size: u32,
    pub sgpr_count: u16,
    pub vgpr_count: u16,
    pub agpr_count: Option<u32>,
    pub sgpr_spill_count: Option<u32>,
    pub vgpr_spill_count: Option<u32>,
    pub max_flat_workgroup_size: u32,
    pub required_workgroup_size: Option<[u32; 3]>,
    pub max_workgroups: [Option<u32>; 3],
    pub identity: ContentIdentityRecordV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerStaticResourceKindV1 {
    KernargSegmentSize,
    KernargSegmentAlignment,
    GroupSegmentFixedSize,
    PrivateSegmentFixedSize,
    WavefrontSize,
    SgprCount,
    VgprCount,
    AgprCount,
    SgprSpillCount,
    VgprSpillCount,
    MaxFlatWorkgroupSize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerStaticResourceDeltaV1 {
    pub resource: ProfilerStaticResourceKindV1,
    pub origin: TruthOriginV1,
    pub baseline: Option<u64>,
    pub candidate: Option<u64>,
    pub signed_delta: Option<i128>,
    pub evidence: Vec<ProfilerVariantEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantDurationDeltaV1 {
    pub dispatch_ordinal: u32,
    pub origin: TruthOriginV1,
    pub baseline_ticks: u64,
    pub candidate_ticks: u64,
    pub signed_delta_ticks: i128,
    pub evidence: Vec<ProfilerVariantEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantCounterDeltaV1 {
    pub dispatch_ordinal: u32,
    pub counter_ordinal: u32,
    pub counter_name: String,
    pub origin: TruthOriginV1,
    pub baseline_f64_bits: u64,
    pub candidate_f64_bits: u64,
    pub delta_f64_bits: u64,
    pub evidence: Vec<ProfilerVariantEvidenceV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantUnavailableKindV1 {
    DecodedAttEvents,
    RuntimeApiEvents,
    CopyEvents,
    PcToSemanticOrIsaCorrelation,
    SemanticIrIsaChangeLocalization,
    CausalRegressionAttribution,
    CounterComparison,
    CounterCompletenessAndDimensions,
    CompleteWorkloadAndArguments,
    ClockDomainAndNormalization,
    PcCaptureBinding,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantUnavailableV1 {
    pub kind: ProfilerVariantUnavailableKindV1,
    pub origin: TruthOriginV1,
    pub reason: String,
    pub evidence: Vec<ProfilerVariantEvidenceV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerVariantExplanationKindV1 {
    LongerCapturedDurationWithScheduleAndStaticResourceChanges,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantExplanationV1 {
    pub rank: u16,
    pub kind: ProfilerVariantExplanationKindV1,
    pub origin: TruthOriginV1,
    pub rule: String,
    pub statement: String,
    pub evidence: Vec<ProfilerVariantEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantComparisonV1 {
    pub schema_version: u16,
    pub request_identity: ContentIdentityRecordV1,
    pub baseline_treatment: ProfilerVariantTreatmentSummaryV1,
    pub candidate_treatment: ProfilerVariantTreatmentSummaryV1,
    pub comparable: bool,
    pub compatibility: Vec<ProfilerVariantCompatibilityFactV1>,
    pub baseline_resources: ProfilerStaticResourcesV1,
    pub candidate_resources: ProfilerStaticResourcesV1,
    pub resource_deltas: Vec<ProfilerStaticResourceDeltaV1>,
    pub duration_deltas: Vec<ProfilerVariantDurationDeltaV1>,
    pub counter_deltas: Vec<ProfilerVariantCounterDeltaV1>,
    pub ranked_explanations: Vec<ProfilerVariantExplanationV1>,
    pub ranking_policy: String,
    pub unavailable: Vec<ProfilerVariantUnavailableV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfilerVariantTreatmentSummaryV1 {
    pub binding_origin: TruthOriginV1,
    pub manifest: ContentIdentityRecordV1,
    pub semantic_workload: ContentIdentityRecordV1,
    pub raw_profiler_source: ContentIdentityRecordV1,
    pub bundle: ContentIdentityRecordV1,
    pub schedule: ContentIdentityRecordV1,
    pub artifact: ContentIdentityRecordV1,
    pub isa_projection: Option<ContentIdentityRecordV1>,
    pub kernel_ir: Vec<KernelIrClaimRecordV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum ProfilerVariantErrorV1 {
    EmptyEvidence,
    EvidenceTooLarge,
    InvalidManifest,
    NonCanonicalManifest,
    StaleIdentity,
    BundleAdmission,
    RawSourceAdmission,
    ArtifactUnavailable,
    ArtifactMismatch,
    HsacoInspection,
    KernelOrdinalOutOfRange,
    AmbiguousKernelBinding,
    LaunchViolatesArtifact,
    CounterAdmission,
    CounterBindingMismatch,
    PcAdmission,
    PcBindingMismatch,
    TooManyDispatches,
    TooManyCounterValues,
    RequestMismatch,
    ResultTooLarge,
    InvalidResult,
    IdentityFailure,
}

impl fmt::Display for ProfilerVariantErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "profiler variant evidence rejected: {self:?}")
    }
}

impl Error for ProfilerVariantErrorV1 {}

struct AdmittedTreatment {
    manifest_identity: ContentIdentityRecordV1,
    manifest: ProfilerVariantTreatmentManifestV1,
    bundle: SemanticProfilerBundleV4,
    counters: Option<AdmittedCounterEvidence>,
    counter_binding: SideCaptureBinding,
    pc_binding: SideCaptureBinding,
    resources: ProfilerStaticResourcesV1,
}

struct AdmittedCounterEvidence {
    capture: SemanticCounterCaptureV2,
    bundle_dispatch_ordinals: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SideCaptureBinding {
    NotProvided,
    Exact,
    Unavailable,
}

pub fn build_profiler_variant_manifest_v1(
    input: ProfilerVariantManifestInputV1<'_>,
) -> Result<Vec<u8>, ProfilerVariantErrorV1> {
    check_opaque(input.semantic_workload)?;
    check_opaque(input.raw_profiler_source)?;
    check_opaque(input.schedule)?;
    check_opaque(input.artifact)?;
    if let Some(bytes) = input.isa_projection {
        check_opaque(bytes)?;
    }
    let admitted_bundle = decode_profiler_bundle_v4(input.bundle)
        .map_err(|_| ProfilerVariantErrorV1::BundleAdmission)?;
    validate_rocprofv3_bundle_raw_source_relation_v1(
        input.raw_profiler_source,
        &admitted_bundle,
        ImportLimitsV1::default(),
    )
    .map_err(|_| ProfilerVariantErrorV1::RawSourceAdmission)?;
    let artifact_identity = artifact_identity(&admitted_bundle, input.artifact)?;
    let manifest = ProfilerVariantTreatmentManifestV1 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V1,
        semantic_workload: opaque_identity(WORKLOAD_DOMAIN_V1, input.semantic_workload)?,
        raw_profiler_source: rocprofv3_json_source_content_identity_v1(
            input.raw_profiler_source,
            ImportLimitsV1::default(),
        )
        .map_err(|_| ProfilerVariantErrorV1::RawSourceAdmission)?,
        bundle: profiler_bundle_content_identity_v4(input.bundle)
            .map_err(|_| ProfilerVariantErrorV1::BundleAdmission)?,
        schedule: opaque_identity(SCHEDULE_DOMAIN_V1, input.schedule)?,
        artifact: artifact_identity,
        kernel_ordinal: input.kernel_ordinal,
        isa_projection: input
            .isa_projection
            .map(|bytes| opaque_identity(ISA_DOMAIN_V1, bytes))
            .transpose()?,
        counters: input
            .counters
            .map(|bytes| {
                counter_capture_content_identity_v2(bytes)
                    .map_err(|_| ProfilerVariantErrorV1::CounterAdmission)
            })
            .transpose()?,
        pc_samples: input
            .pc_samples
            .map(|bytes| {
                pc_sample_capture_content_identity_v3(bytes)
                    .map_err(|_| ProfilerVariantErrorV1::PcAdmission)
            })
            .transpose()?,
    };
    let bytes =
        serde_json::to_vec(&manifest).map_err(|_| ProfilerVariantErrorV1::InvalidManifest)?;
    if bytes.len() as u64 > MAX_PROFILER_VARIANT_MANIFEST_BYTES_V1 {
        return Err(ProfilerVariantErrorV1::EvidenceTooLarge);
    }
    Ok(bytes)
}

pub fn build_profiler_variant_request_v1(
    semantic_workload: &[u8],
    baseline_manifest: &[u8],
    candidate_manifest: &[u8],
) -> Result<ProfilerVariantComparisonRequestV1, ProfilerVariantErrorV1> {
    check_opaque(semantic_workload)?;
    Ok(ProfilerVariantComparisonRequestV1 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V1,
        semantic_workload: opaque_identity(WORKLOAD_DOMAIN_V1, semantic_workload)?,
        baseline_manifest: manifest_identity(baseline_manifest)?,
        candidate_manifest: manifest_identity(candidate_manifest)?,
    })
}

pub fn compare_profiler_variants_v1(
    request: ProfilerVariantComparisonRequestV1,
    baseline: ProfilerVariantTreatmentInputV1<'_>,
    candidate: ProfilerVariantTreatmentInputV1<'_>,
) -> Result<ProfilerVariantComparisonV1, ProfilerVariantErrorV1> {
    let left = admit_treatment(baseline)?;
    let right = admit_treatment(candidate)?;
    validate_request(request, &left, &right)?;
    let request_identity = request_identity(request)?;

    let compatibility = vec![
        compatibility_fact(
            ProfilerVariantCompatibilityAxisV1::SemanticWorkload,
            left.manifest.semantic_workload == right.manifest.semantic_workload,
            vec![evidence(
                ProfilerVariantEvidenceRoleV1::SemanticWorkload,
                TruthOriginV1::Declared,
                request.semantic_workload.digest,
            )],
        ),
        identity_axis(
            ProfilerVariantCompatibilityAxisV1::Environment,
            &left,
            &right,
            |bundle| bundle.environment.value,
        ),
        identity_axis(
            ProfilerVariantCompatibilityAxisV1::CollectorTool,
            &left,
            &right,
            |bundle| bundle.collector_tool.value,
        ),
        identity_axis(
            ProfilerVariantCompatibilityAxisV1::CollectorConfiguration,
            &left,
            &right,
            |bundle| bundle.collector_configuration.value,
        ),
        device_compatibility_fact(&left, &right),
        compatibility_fact(
            ProfilerVariantCompatibilityAxisV1::DispatchWorkloadAndLaunch,
            workloads_match(&left.bundle, &right.bundle),
            bundle_pair_evidence(&left, &right),
        ),
    ];
    let comparable = compatibility
        .iter()
        .all(|fact| fact.status == ProfilerVariantCompatibilityStatusV1::Exact);

    let resource_deltas = if comparable {
        resource_deltas(&left.resources, &right.resources)
    } else {
        Vec::new()
    };
    let duration_deltas = if comparable {
        duration_deltas(&left, &right)
    } else {
        Vec::new()
    };
    let (counter_deltas, counter_unavailable) = if comparable {
        compare_counters(&left, &right)
    } else {
        (Vec::new(), Some("comparison axes are not exact"))
    };
    let mut unavailable = fixed_unavailable(&left, &right);
    if let Some(reason) = counter_unavailable {
        unavailable.push(ProfilerVariantUnavailableV1 {
            kind: ProfilerVariantUnavailableKindV1::CounterComparison,
            origin: TruthOriginV1::Unavailable,
            reason: reason.to_owned(),
            evidence: counter_pair_evidence(&left, &right),
        });
    }
    unavailable.sort_by_key(|fact| fact.kind);

    let ranked_explanations = explanations(
        comparable,
        &left,
        &right,
        &resource_deltas,
        &duration_deltas,
    );
    let result = ProfilerVariantComparisonV1 {
        schema_version: PROFILER_VARIANT_SCHEMA_VERSION_V1,
        request_identity,
        baseline_treatment: treatment_summary(&left),
        candidate_treatment: treatment_summary(&right),
        comparable,
        compatibility,
        baseline_resources: left.resources,
        candidate_resources: right.resources,
        resource_deltas,
        duration_deltas,
        counter_deltas,
        ranked_explanations,
        ranking_policy: "canonical_rule_order_only_not_likelihood_or_causality".to_owned(),
        unavailable,
    };
    validate_result_size(&result)?;
    Ok(result)
}

pub fn encode_profiler_variant_comparison_v1(
    request: ProfilerVariantComparisonRequestV1,
    baseline: ProfilerVariantTreatmentInputV1<'_>,
    candidate: ProfilerVariantTreatmentInputV1<'_>,
    result: &ProfilerVariantComparisonV1,
) -> Result<Vec<u8>, ProfilerVariantErrorV1> {
    let expected = compare_profiler_variants_v1(request, baseline, candidate)?;
    if &expected != result {
        return Err(ProfilerVariantErrorV1::InvalidResult);
    }
    let bytes = serde_json::to_vec(result).map_err(|_| ProfilerVariantErrorV1::InvalidResult)?;
    if bytes.len() as u64 > MAX_PROFILER_VARIANT_RESULT_BYTES_V1 {
        return Err(ProfilerVariantErrorV1::ResultTooLarge);
    }
    Ok(bytes)
}

pub fn decode_profiler_variant_comparison_v1(
    bytes: &[u8],
    request: ProfilerVariantComparisonRequestV1,
    baseline: ProfilerVariantTreatmentInputV1<'_>,
    candidate: ProfilerVariantTreatmentInputV1<'_>,
) -> Result<ProfilerVariantComparisonV1, ProfilerVariantErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_PROFILER_VARIANT_RESULT_BYTES_V1 {
        return Err(ProfilerVariantErrorV1::ResultTooLarge);
    }
    let result: ProfilerVariantComparisonV1 =
        serde_json::from_slice(bytes).map_err(|_| ProfilerVariantErrorV1::InvalidResult)?;
    let canonical = encode_profiler_variant_comparison_v1(request, baseline, candidate, &result)?;
    if canonical != bytes {
        return Err(ProfilerVariantErrorV1::InvalidResult);
    }
    Ok(result)
}

fn admit_treatment(
    input: ProfilerVariantTreatmentInputV1<'_>,
) -> Result<AdmittedTreatment, ProfilerVariantErrorV1> {
    check_total_input(input)?;
    check_opaque(input.semantic_workload)?;
    check_opaque(input.raw_profiler_source)?;
    check_opaque(input.schedule)?;
    check_opaque(input.artifact)?;
    if let Some(bytes) = input.isa_projection {
        check_opaque(bytes)?;
    }
    let manifest_identity = manifest_identity(input.manifest)?;
    let manifest: ProfilerVariantTreatmentManifestV1 = serde_json::from_slice(input.manifest)
        .map_err(|_| ProfilerVariantErrorV1::InvalidManifest)?;
    if manifest.schema_version != PROFILER_VARIANT_SCHEMA_VERSION_V1
        || serde_json::to_vec(&manifest).map_err(|_| ProfilerVariantErrorV1::InvalidManifest)?
            != input.manifest
    {
        return Err(ProfilerVariantErrorV1::NonCanonicalManifest);
    }
    let bundle = decode_profiler_bundle_v4(input.bundle)
        .map_err(|_| ProfilerVariantErrorV1::BundleAdmission)?;
    let raw_source_relation = validate_rocprofv3_bundle_raw_source_relation_v1(
        input.raw_profiler_source,
        &bundle,
        ImportLimitsV1::default(),
    )
    .map_err(|_| ProfilerVariantErrorV1::RawSourceAdmission)?;
    let dispatches = bundle
        .dispatch_capture
        .as_ref()
        .ok_or(ProfilerVariantErrorV1::BundleAdmission)?
        .dispatches
        .as_slice();
    if dispatches.len() > MAX_PROFILER_VARIANT_DISPATCHES_V1 {
        return Err(ProfilerVariantErrorV1::TooManyDispatches);
    }
    require_identity(
        manifest.semantic_workload,
        opaque_identity(WORKLOAD_DOMAIN_V1, input.semantic_workload)?,
    )?;
    require_identity(
        manifest.raw_profiler_source,
        rocprofv3_json_source_content_identity_v1(
            input.raw_profiler_source,
            ImportLimitsV1::default(),
        )
        .map_err(|_| ProfilerVariantErrorV1::RawSourceAdmission)?,
    )?;
    require_identity(
        manifest.bundle,
        profiler_bundle_content_identity_v4(input.bundle)
            .map_err(|_| ProfilerVariantErrorV1::BundleAdmission)?,
    )?;
    require_identity(
        manifest.schedule,
        opaque_identity(SCHEDULE_DOMAIN_V1, input.schedule)?,
    )?;
    require_identity(
        manifest.artifact,
        artifact_identity(&bundle, input.artifact)?,
    )?;
    require_optional_identity(
        manifest.isa_projection,
        input
            .isa_projection
            .map(|bytes| opaque_identity(ISA_DOMAIN_V1, bytes))
            .transpose()?,
    )?;

    let (counters, counter_binding) = match input.counters {
        Some(bytes) => {
            let identity = counter_capture_content_identity_v2(bytes)
                .map_err(|_| ProfilerVariantErrorV1::CounterAdmission)?;
            require_optional_identity(manifest.counters, Some(identity))?;
            let capture = decode_counter_capture_v2(bytes)
                .map_err(|_| ProfilerVariantErrorV1::CounterAdmission)?;
            validate_counter_cardinality(&capture)?;
            if let Ok(relation) = validate_rocprofv3_counter_bundle_relation_v1(
                input.raw_profiler_source,
                &bundle,
                raw_source_relation,
                &capture,
                ImportLimitsV1::default(),
            ) {
                (
                    Some(AdmittedCounterEvidence {
                        capture,
                        bundle_dispatch_ordinals: relation.bundle_dispatch_ordinals().to_vec(),
                    }),
                    SideCaptureBinding::Exact,
                )
            } else {
                (None, SideCaptureBinding::Unavailable)
            }
        }
        None => {
            require_optional_identity(manifest.counters, None)?;
            (None, SideCaptureBinding::NotProvided)
        }
    };

    let pc_binding = match input.pc_samples {
        Some(bytes) => {
            let identity = pc_sample_capture_content_identity_v3(bytes)
                .map_err(|_| ProfilerVariantErrorV1::PcAdmission)?;
            require_optional_identity(manifest.pc_samples, Some(identity))?;
            let capture = decode_pc_sample_capture_v3(bytes)
                .map_err(|_| ProfilerVariantErrorV1::PcAdmission)?;
            if validate_rocprofv3_pc_bundle_relation_v1(
                input.raw_profiler_source,
                &bundle,
                raw_source_relation,
                &capture,
                ImportLimitsV1::default(),
            )
            .is_ok()
            {
                SideCaptureBinding::Exact
            } else {
                SideCaptureBinding::Unavailable
            }
        }
        None => {
            require_optional_identity(manifest.pc_samples, None)?;
            SideCaptureBinding::NotProvided
        }
    };

    let resources = inspect_resources(
        input.artifact,
        manifest.artifact,
        manifest.kernel_ordinal,
        dispatches,
    )?;
    Ok(AdmittedTreatment {
        manifest_identity,
        manifest,
        bundle,
        counters,
        counter_binding,
        pc_binding,
        resources,
    })
}

fn inspect_resources(
    artifact: &[u8],
    artifact_identity: ContentIdentityRecordV1,
    kernel_ordinal: u32,
    dispatches: &[CaptureDispatchV1],
) -> Result<ProfilerStaticResourcesV1, ProfilerVariantErrorV1> {
    let inspected = inspect(artifact).map_err(|_| ProfilerVariantErrorV1::HsacoInspection)?;
    if inspected.kernels().len() != 1 {
        return Err(ProfilerVariantErrorV1::AmbiguousKernelBinding);
    }
    let kernel = inspected
        .kernels()
        .get(kernel_ordinal as usize)
        .ok_or(ProfilerVariantErrorV1::KernelOrdinalOutOfRange)?;
    for dispatch in dispatches {
        let flat = dispatch
            .launch
            .workgroup_size
            .into_iter()
            .try_fold(1_u64, |left, right| left.checked_mul(u64::from(right)))
            .ok_or(ProfilerVariantErrorV1::LaunchViolatesArtifact)?;
        if dispatch.launch.wave_width != kernel.wavefront_size() as u16
            || flat > u64::from(kernel.max_flat_workgroup_size())
            || kernel
                .required_workgroup_size()
                .is_some_and(|required| required != dispatch.launch.workgroup_size)
            || kernel
                .max_workgroups()
                .into_iter()
                .zip(dispatch.launch.grid_workgroups)
                .any(|(limit, actual)| limit.is_some_and(|limit| actual > limit))
        {
            return Err(ProfilerVariantErrorV1::LaunchViolatesArtifact);
        }
    }
    let mut resources = ProfilerStaticResourcesV1 {
        origin: TruthOriginV1::Observed,
        artifact: artifact_identity,
        kernel_ordinal,
        kernarg_segment_size: kernel.kernarg_segment_size(),
        kernarg_segment_alignment: kernel.kernarg_segment_alignment(),
        group_segment_fixed_size: kernel.group_segment_fixed_size(),
        private_segment_fixed_size: kernel.private_segment_fixed_size(),
        wavefront_size: kernel.wavefront_size(),
        sgpr_count: kernel.sgpr_count(),
        vgpr_count: kernel.vgpr_count(),
        agpr_count: kernel.agpr_count(),
        sgpr_spill_count: kernel.sgpr_spill_count(),
        vgpr_spill_count: kernel.vgpr_spill_count(),
        max_flat_workgroup_size: kernel.max_flat_workgroup_size(),
        required_workgroup_size: kernel.required_workgroup_size(),
        max_workgroups: kernel.max_workgroups(),
        identity: placeholder_identity()?,
    };
    resources.identity = resource_identity(&resources)?;
    Ok(resources)
}

fn artifact_identity(
    bundle: &SemanticProfilerBundleV4,
    artifact: &[u8],
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    check_opaque(artifact)?;
    let dispatches = bundle
        .dispatch_capture
        .as_ref()
        .ok_or(ProfilerVariantErrorV1::ArtifactUnavailable)?
        .dispatches
        .as_slice();
    let first = dispatches
        .first()
        .and_then(|dispatch| available_identity(dispatch.artifact))
        .ok_or(ProfilerVariantErrorV1::ArtifactUnavailable)?;
    if first.scheme != ContentSchemeV1::RawCanonicalSha256
        || dispatches
            .iter()
            .any(|dispatch| available_identity(dispatch.artifact) != Some(first))
    {
        return Err(ProfilerVariantErrorV1::ArtifactMismatch);
    }
    let actual = raw_identity(first.format_version, artifact)?;
    require_identity(first, actual)?;
    Ok(actual)
}

fn validate_counter_cardinality(
    counters: &SemanticCounterCaptureV2,
) -> Result<(), ProfilerVariantErrorV1> {
    let count = counters
        .dispatches
        .iter()
        .try_fold(0_usize, |sum, dispatch| {
            sum.checked_add(dispatch.values.len())
        })
        .ok_or(ProfilerVariantErrorV1::TooManyCounterValues)?;
    if count > MAX_PROFILER_VARIANT_COUNTER_VALUES_V1 {
        return Err(ProfilerVariantErrorV1::TooManyCounterValues);
    }
    Ok(())
}

fn validate_request(
    request: ProfilerVariantComparisonRequestV1,
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Result<(), ProfilerVariantErrorV1> {
    if request.schema_version != PROFILER_VARIANT_SCHEMA_VERSION_V1
        || request.semantic_workload != left.manifest.semantic_workload
        || request.semantic_workload != right.manifest.semantic_workload
        || request.baseline_manifest != left.manifest_identity
        || request.candidate_manifest != right.manifest_identity
    {
        return Err(ProfilerVariantErrorV1::RequestMismatch);
    }
    Ok(())
}

fn identity_axis(
    axis: ProfilerVariantCompatibilityAxisV1,
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
    select: impl Fn(&SemanticProfilerBundleV4) -> Option<ContentIdentityRecordV1>,
) -> ProfilerVariantCompatibilityFactV1 {
    let baseline = select(&left.bundle);
    let candidate = select(&right.bundle);
    let status = match (baseline, candidate) {
        (Some(baseline), Some(candidate)) if baseline == candidate => {
            ProfilerVariantCompatibilityStatusV1::Exact
        }
        (Some(_), Some(_)) => ProfilerVariantCompatibilityStatusV1::Mismatch,
        _ => ProfilerVariantCompatibilityStatusV1::Unavailable,
    };
    ProfilerVariantCompatibilityFactV1 {
        axis,
        status,
        origin: if status == ProfilerVariantCompatibilityStatusV1::Unavailable {
            TruthOriginV1::Unavailable
        } else {
            TruthOriginV1::Declared
        },
        evidence: bundle_pair_evidence(left, right),
    }
}

fn compatibility_fact(
    axis: ProfilerVariantCompatibilityAxisV1,
    exact: bool,
    evidence: Vec<ProfilerVariantEvidenceV1>,
) -> ProfilerVariantCompatibilityFactV1 {
    ProfilerVariantCompatibilityFactV1 {
        axis,
        status: if exact {
            ProfilerVariantCompatibilityStatusV1::Exact
        } else {
            ProfilerVariantCompatibilityStatusV1::Mismatch
        },
        origin: TruthOriginV1::Declared,
        evidence,
    }
}

fn device_compatibility_fact(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> ProfilerVariantCompatibilityFactV1 {
    let status = match dispatch_devices_match(&left.bundle, &right.bundle) {
        Some(true) => ProfilerVariantCompatibilityStatusV1::Exact,
        Some(false) => ProfilerVariantCompatibilityStatusV1::Mismatch,
        None => ProfilerVariantCompatibilityStatusV1::Unavailable,
    };
    ProfilerVariantCompatibilityFactV1 {
        axis: ProfilerVariantCompatibilityAxisV1::StableDevices,
        status,
        origin: if status == ProfilerVariantCompatibilityStatusV1::Unavailable {
            TruthOriginV1::Unavailable
        } else {
            TruthOriginV1::Declared
        },
        evidence: bundle_pair_evidence(left, right),
    }
}

fn workloads_match(left: &SemanticProfilerBundleV4, right: &SemanticProfilerBundleV4) -> bool {
    if left.source_kind != right.source_kind {
        return false;
    }
    let Some(left_capture) = &left.dispatch_capture else {
        return false;
    };
    let Some(right_capture) = &right.dispatch_capture else {
        return false;
    };
    left_capture.dispatches.len() == right_capture.dispatches.len()
        && left_capture
            .dispatches
            .iter()
            .zip(&right_capture.dispatches)
            .all(|(left_dispatch, right_dispatch)| {
                left_dispatch.process_index == right_dispatch.process_index
                    && left_dispatch.dispatch_index == right_dispatch.dispatch_index
                    && left_dispatch.source_record_ordinal == right_dispatch.source_record_ordinal
                    && left_dispatch.launch == right_dispatch.launch
            })
}

fn dispatch_devices_match(
    left: &SemanticProfilerBundleV4,
    right: &SemanticProfilerBundleV4,
) -> Option<bool> {
    let Some(left_capture) = &left.dispatch_capture else {
        return None;
    };
    let Some(right_capture) = &right.dispatch_capture else {
        return None;
    };
    if left_capture.dispatches.len() != right_capture.dispatches.len() {
        return Some(false);
    }
    let mut exact = true;
    for (left_dispatch, right_dispatch) in left_capture
        .dispatches
        .iter()
        .zip(&right_capture.dispatches)
    {
        let left_device =
            stable_dispatch_device(left_capture, left, left_dispatch.device_identity)?;
        let right_device =
            stable_dispatch_device(right_capture, right, right_dispatch.device_identity)?;
        exact &= left_device == right_device;
    }
    Some(exact)
}

fn stable_dispatch_device(
    capture: &fe2o3_semantic_import::SemanticCaptureV1,
    bundle: &SemanticProfilerBundleV4,
    identity: CaptureIdentityV1,
) -> Option<ContentIdentityRecordV1> {
    let ordinal = capture_device_ordinal(&capture.devices, identity)?;
    bundle
        .devices
        .iter()
        .find(|device| device.ordinal as usize == ordinal)
        .and_then(|device| device.stable_identity.value)
}

fn capture_device_ordinal(
    devices: &[fe2o3_semantic_import::CaptureDeviceV1],
    identity: CaptureIdentityV1,
) -> Option<usize> {
    devices
        .iter()
        .position(|device| device.identity == identity)
}

fn duration_deltas(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantDurationDeltaV1> {
    let left_dispatches = &left.bundle.dispatch_capture.as_ref().unwrap().dispatches;
    let right_dispatches = &right.bundle.dispatch_capture.as_ref().unwrap().dispatches;
    left_dispatches
        .iter()
        .zip(right_dispatches)
        .enumerate()
        .map(
            |(ordinal, (baseline, candidate))| ProfilerVariantDurationDeltaV1 {
                dispatch_ordinal: ordinal as u32,
                origin: TruthOriginV1::Inferred,
                baseline_ticks: baseline.duration_ticks,
                candidate_ticks: candidate.duration_ticks,
                signed_delta_ticks: i128::from(candidate.duration_ticks)
                    - i128::from(baseline.duration_ticks),
                evidence: vec![
                    evidence(
                        ProfilerVariantEvidenceRoleV1::BaselineDispatch,
                        TruthOriginV1::Observed,
                        baseline.identity,
                    ),
                    evidence(
                        ProfilerVariantEvidenceRoleV1::CandidateDispatch,
                        TruthOriginV1::Observed,
                        candidate.identity,
                    ),
                ],
            },
        )
        .collect()
}

fn compare_counters(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> (Vec<ProfilerVariantCounterDeltaV1>, Option<&'static str>) {
    if left.counter_binding == SideCaptureBinding::Unavailable
        || right.counter_binding == SideCaptureBinding::Unavailable
    {
        return (
            Vec::new(),
            Some(
                "counter exact supplied-source and dispatch-id relation to a treatment Bundle V4 is unavailable",
            ),
        );
    }
    let (Some(left_capture), Some(right_capture)) = (&left.counters, &right.counters) else {
        return (
            Vec::new(),
            Some("both treatments must bind Counter Capture V2 evidence"),
        );
    };
    if !counter_definitions_match(&left_capture.capture, &right_capture.capture)
        || left_capture.bundle_dispatch_ordinals.len()
            != right_capture.bundle_dispatch_ordinals.len()
    {
        return (
            Vec::new(),
            Some("counter definitions or dispatch workload differ"),
        );
    }
    let Some(left_by_bundle) = counters_by_bundle_dispatch(left_capture) else {
        return (
            Vec::new(),
            Some("baseline counter relation is not bijective"),
        );
    };
    let Some(right_by_bundle) = counters_by_bundle_dispatch(right_capture) else {
        return (
            Vec::new(),
            Some("candidate counter relation is not bijective"),
        );
    };
    let mut output = Vec::new();
    for (dispatch_ordinal, (baseline, candidate)) in
        left_by_bundle.into_iter().zip(right_by_bundle).enumerate()
    {
        if baseline.values.len() != candidate.values.len() {
            return (Vec::new(), Some("counter value coverage differs"));
        }
        for (counter_ordinal, (left_value, right_value)) in
            baseline.values.iter().zip(&candidate.values).enumerate()
        {
            let Some(left_definition) =
                definition_for(&left_capture.capture, left_value.counter_identity)
            else {
                return (
                    Vec::new(),
                    Some("counter definition binding is unavailable"),
                );
            };
            let Some(right_definition) =
                definition_for(&right_capture.capture, right_value.counter_identity)
            else {
                return (
                    Vec::new(),
                    Some("counter definition binding is unavailable"),
                );
            };
            if definition_key(&left_capture.capture, left_definition)
                != definition_key(&right_capture.capture, right_definition)
            {
                return (Vec::new(), Some("counter record identity axes differ"));
            }
            let delta = right_value.value() - left_value.value();
            if !delta.is_finite() {
                return (
                    Vec::new(),
                    Some("finite observed counter values overflowed the derived binary64 delta"),
                );
            }
            output.push(ProfilerVariantCounterDeltaV1 {
                dispatch_ordinal: dispatch_ordinal as u32,
                counter_ordinal: counter_ordinal as u32,
                counter_name: left_definition.name.clone(),
                origin: TruthOriginV1::Inferred,
                baseline_f64_bits: left_value.value_f64_bits,
                candidate_f64_bits: right_value.value_f64_bits,
                delta_f64_bits: delta.to_bits(),
                evidence: vec![
                    evidence(
                        ProfilerVariantEvidenceRoleV1::BaselineCounterValue,
                        TruthOriginV1::Observed,
                        left_value.identity,
                    ),
                    evidence(
                        ProfilerVariantEvidenceRoleV1::CandidateCounterValue,
                        TruthOriginV1::Observed,
                        right_value.identity,
                    ),
                ],
            });
        }
    }
    (output, None)
}

fn counters_by_bundle_dispatch(
    evidence: &AdmittedCounterEvidence,
) -> Option<Vec<&CounterDispatchV2>> {
    let mut ordered = vec![None; evidence.bundle_dispatch_ordinals.len()];
    for (counter, ordinal) in evidence
        .capture
        .dispatches
        .iter()
        .zip(&evidence.bundle_dispatch_ordinals)
    {
        let slot = ordered.get_mut(*ordinal as usize)?;
        if slot.replace(counter).is_some() {
            return None;
        }
    }
    ordered.into_iter().collect()
}

fn counter_definitions_match(
    left: &SemanticCounterCaptureV2,
    right: &SemanticCounterCaptureV2,
) -> bool {
    left.counter_definitions.len() == right.counter_definitions.len()
        && left
            .counter_definitions
            .iter()
            .zip(&right.counter_definitions)
            .all(|(left_definition, right_definition)| {
                definition_key(left, left_definition) == definition_key(right, right_definition)
            })
}

fn definition_for(
    capture: &SemanticCounterCaptureV2,
    identity: CaptureIdentityV1,
) -> Option<&CounterDefinitionV2> {
    capture
        .counter_definitions
        .iter()
        .find(|definition| definition.identity == identity)
}

fn definition_key<'a>(
    capture: &SemanticCounterCaptureV2,
    definition: &'a CounterDefinitionV2,
) -> (usize, &'a str, bool, bool) {
    let device_ordinal = capture
        .devices
        .iter()
        .position(|device| device.identity == definition.device_identity)
        .unwrap_or(usize::MAX);
    (
        device_ordinal,
        &definition.name,
        definition.is_constant,
        definition.is_derived,
    )
}

fn resource_deltas(
    left: &ProfilerStaticResourcesV1,
    right: &ProfilerStaticResourcesV1,
) -> Vec<ProfilerStaticResourceDeltaV1> {
    let values = [
        (
            ProfilerStaticResourceKindV1::KernargSegmentSize,
            Some(left.kernarg_segment_size),
            Some(right.kernarg_segment_size),
        ),
        (
            ProfilerStaticResourceKindV1::KernargSegmentAlignment,
            Some(left.kernarg_segment_alignment),
            Some(right.kernarg_segment_alignment),
        ),
        (
            ProfilerStaticResourceKindV1::GroupSegmentFixedSize,
            Some(left.group_segment_fixed_size),
            Some(right.group_segment_fixed_size),
        ),
        (
            ProfilerStaticResourceKindV1::PrivateSegmentFixedSize,
            Some(left.private_segment_fixed_size),
            Some(right.private_segment_fixed_size),
        ),
        (
            ProfilerStaticResourceKindV1::WavefrontSize,
            Some(u64::from(left.wavefront_size)),
            Some(u64::from(right.wavefront_size)),
        ),
        (
            ProfilerStaticResourceKindV1::SgprCount,
            Some(u64::from(left.sgpr_count)),
            Some(u64::from(right.sgpr_count)),
        ),
        (
            ProfilerStaticResourceKindV1::VgprCount,
            Some(u64::from(left.vgpr_count)),
            Some(u64::from(right.vgpr_count)),
        ),
        (
            ProfilerStaticResourceKindV1::AgprCount,
            left.agpr_count.map(u64::from),
            right.agpr_count.map(u64::from),
        ),
        (
            ProfilerStaticResourceKindV1::SgprSpillCount,
            left.sgpr_spill_count.map(u64::from),
            right.sgpr_spill_count.map(u64::from),
        ),
        (
            ProfilerStaticResourceKindV1::VgprSpillCount,
            left.vgpr_spill_count.map(u64::from),
            right.vgpr_spill_count.map(u64::from),
        ),
        (
            ProfilerStaticResourceKindV1::MaxFlatWorkgroupSize,
            Some(u64::from(left.max_flat_workgroup_size)),
            Some(u64::from(right.max_flat_workgroup_size)),
        ),
    ];
    let evidence = vec![
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineStaticResources,
            TruthOriginV1::Observed,
            left.identity.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateStaticResources,
            TruthOriginV1::Observed,
            right.identity.digest,
        ),
    ];
    values
        .into_iter()
        .map(
            |(resource, baseline, candidate)| ProfilerStaticResourceDeltaV1 {
                resource,
                origin: if baseline.is_some() && candidate.is_some() {
                    TruthOriginV1::Inferred
                } else {
                    TruthOriginV1::Unavailable
                },
                baseline,
                candidate,
                signed_delta: baseline
                    .zip(candidate)
                    .map(|(baseline, candidate)| i128::from(candidate) - i128::from(baseline)),
                evidence: evidence.clone(),
            },
        )
        .collect()
}

fn explanations(
    comparable: bool,
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
    resources: &[ProfilerStaticResourceDeltaV1],
    durations: &[ProfilerVariantDurationDeltaV1],
) -> Vec<ProfilerVariantExplanationV1> {
    let resource_changed = resources
        .iter()
        .any(|delta| delta.signed_delta.is_some_and(|delta| delta != 0));
    let duration_increased = durations.iter().any(|delta| delta.signed_delta_ticks > 0);
    if !comparable
        || left.manifest.schedule == right.manifest.schedule
        || !resource_changed
        || !duration_increased
    {
        return Vec::new();
    }
    let mut evidence = vec![
        evidence(
            ProfilerVariantEvidenceRoleV1::SemanticWorkload,
            TruthOriginV1::Declared,
            left.manifest.semantic_workload.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineManifest,
            TruthOriginV1::Declared,
            left.manifest_identity.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateManifest,
            TruthOriginV1::Declared,
            right.manifest_identity.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineBundle,
            TruthOriginV1::Observed,
            left.manifest.bundle.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateBundle,
            TruthOriginV1::Observed,
            right.manifest.bundle.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineSchedule,
            TruthOriginV1::Declared,
            left.manifest.schedule.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateSchedule,
            TruthOriginV1::Declared,
            right.manifest.schedule.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineStaticResources,
            TruthOriginV1::Observed,
            left.resources.identity.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateStaticResources,
            TruthOriginV1::Observed,
            right.resources.identity.digest,
        ),
    ];
    for duration in durations
        .iter()
        .filter(|delta| delta.signed_delta_ticks > 0)
    {
        evidence.extend_from_slice(&duration.evidence);
    }
    vec![ProfilerVariantExplanationV1 {
        rank: 1,
        kind: ProfilerVariantExplanationKindV1::LongerCapturedDurationWithScheduleAndStaticResourceChanges,
        origin: TruthOriginV1::Inferred,
        rule: "exact_axes_and_longer_duration_and_changed_schedule_and_changed_static_resources_v1"
            .to_owned(),
        statement: "On exact comparison axes, candidate captured duration increased while the declared schedule identity and observed HSACO static resources also changed; this is co-observation, not a causal claim."
            .to_owned(),
        evidence,
    }]
}

fn treatment_summary(treatment: &AdmittedTreatment) -> ProfilerVariantTreatmentSummaryV1 {
    ProfilerVariantTreatmentSummaryV1 {
        binding_origin: TruthOriginV1::Declared,
        manifest: treatment.manifest_identity,
        semantic_workload: treatment.manifest.semantic_workload,
        raw_profiler_source: treatment.manifest.raw_profiler_source,
        bundle: treatment.manifest.bundle,
        schedule: treatment.manifest.schedule,
        artifact: treatment.manifest.artifact,
        isa_projection: treatment.manifest.isa_projection,
        kernel_ir: treatment
            .bundle
            .dispatch_capture
            .as_ref()
            .map(|capture| {
                capture
                    .dispatches
                    .iter()
                    .map(|dispatch| dispatch.kernel_ir)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn fixed_unavailable(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantUnavailableV1> {
    let pc_evidence = pc_pair_evidence(left, right);
    let mut facts = vec![
        unavailable(
            ProfilerVariantUnavailableKindV1::DecodedAttEvents,
            "Bundle V4 carries ATT references but no admitted decoded ATT event schema",
            bundle_pair_evidence(left, right),
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::RuntimeApiEvents,
            "Bundle V4 has no admitted runtime API event producer",
            bundle_pair_evidence(left, right),
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::CopyEvents,
            "Bundle V4 has no admitted copy event producer",
            bundle_pair_evidence(left, right),
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::PcToSemanticOrIsaCorrelation,
            "PC Capture V3 positions and code-object identities are capture-local and have no content-bound semantic/ISA map",
            pc_evidence,
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::SemanticIrIsaChangeLocalization,
            "schedule and optional ISA projection bytes are content-bound declarations, not an admitted semantic-to-IR-to-ISA map",
            vec![
                evidence(
                    ProfilerVariantEvidenceRoleV1::BaselineSchedule,
                    TruthOriginV1::Declared,
                    left.manifest.schedule.digest,
                ),
                evidence(
                    ProfilerVariantEvidenceRoleV1::CandidateSchedule,
                    TruthOriginV1::Declared,
                    right.manifest.schedule.digest,
                ),
            ],
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::CausalRegressionAttribution,
            "the admitted evidence proves exact identities and co-observed deltas but does not prove causation or superiority",
            vec![
                evidence(
                    ProfilerVariantEvidenceRoleV1::BaselineManifest,
                    TruthOriginV1::Declared,
                    left.manifest_identity.digest,
                ),
                evidence(
                    ProfilerVariantEvidenceRoleV1::CandidateManifest,
                    TruthOriginV1::Declared,
                    right.manifest_identity.digest,
                ),
            ],
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::CounterCompletenessAndDimensions,
            "Counter Capture V2 declares partial semantic history, unknown collector loss, and no instance-dimension identity; absent records are not zero",
            counter_pair_evidence(left, right),
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::CompleteWorkloadAndArguments,
            "the semantic-workload identity is caller-declared; Bundle V4 does not authenticate dispatch argument/input contents or loss-free complete-workload coverage",
            complete_workload_evidence(left, right),
        ),
        unavailable(
            ProfilerVariantUnavailableKindV1::ClockDomainAndNormalization,
            "dispatch durations are opaque collector ticks; Bundle V4 does not admit a frequency or wall-clock normalization",
            bundle_pair_evidence(left, right),
        ),
    ];
    if left.pc_binding == SideCaptureBinding::Unavailable
        || right.pc_binding == SideCaptureBinding::Unavailable
    {
        facts.push(unavailable(
            ProfilerVariantUnavailableKindV1::PcCaptureBinding,
            "PC exact supplied-source and dispatch-id relation to a treatment Bundle V4 is unavailable",
            pc_pair_evidence(left, right),
        ));
    }
    facts
}

fn unavailable(
    kind: ProfilerVariantUnavailableKindV1,
    reason: &str,
    evidence: Vec<ProfilerVariantEvidenceV1>,
) -> ProfilerVariantUnavailableV1 {
    ProfilerVariantUnavailableV1 {
        kind,
        origin: TruthOriginV1::Unavailable,
        reason: reason.to_owned(),
        evidence,
    }
}

fn bundle_pair_evidence(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantEvidenceV1> {
    vec![
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineBundle,
            TruthOriginV1::Observed,
            left.manifest.bundle.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateBundle,
            TruthOriginV1::Observed,
            right.manifest.bundle.digest,
        ),
    ]
}

fn raw_source_pair_evidence(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantEvidenceV1> {
    vec![
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineRawProfilerSource,
            TruthOriginV1::Observed,
            left.manifest.raw_profiler_source.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateRawProfilerSource,
            TruthOriginV1::Observed,
            right.manifest.raw_profiler_source.digest,
        ),
    ]
}

fn complete_workload_evidence(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantEvidenceV1> {
    let mut output = raw_source_pair_evidence(left, right);
    output.extend([
        evidence(
            ProfilerVariantEvidenceRoleV1::SemanticWorkload,
            TruthOriginV1::Declared,
            left.manifest.semantic_workload.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::BaselineBundle,
            TruthOriginV1::Observed,
            left.manifest.bundle.digest,
        ),
        evidence(
            ProfilerVariantEvidenceRoleV1::CandidateBundle,
            TruthOriginV1::Observed,
            right.manifest.bundle.digest,
        ),
    ]);
    output
}

fn counter_pair_evidence(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantEvidenceV1> {
    optional_pair_evidence(
        left.manifest.counters,
        right.manifest.counters,
        ProfilerVariantEvidenceRoleV1::BaselineCounterCapture,
        ProfilerVariantEvidenceRoleV1::CandidateCounterCapture,
    )
}

fn pc_pair_evidence(
    left: &AdmittedTreatment,
    right: &AdmittedTreatment,
) -> Vec<ProfilerVariantEvidenceV1> {
    optional_pair_evidence(
        left.manifest.pc_samples,
        right.manifest.pc_samples,
        ProfilerVariantEvidenceRoleV1::BaselinePcCapture,
        ProfilerVariantEvidenceRoleV1::CandidatePcCapture,
    )
}

fn optional_pair_evidence(
    baseline: Option<ContentIdentityRecordV1>,
    candidate: Option<ContentIdentityRecordV1>,
    baseline_role: ProfilerVariantEvidenceRoleV1,
    candidate_role: ProfilerVariantEvidenceRoleV1,
) -> Vec<ProfilerVariantEvidenceV1> {
    baseline
        .map(|identity| evidence(baseline_role, TruthOriginV1::Observed, identity.digest))
        .into_iter()
        .chain(
            candidate
                .map(|identity| evidence(candidate_role, TruthOriginV1::Observed, identity.digest)),
        )
        .collect()
}

fn evidence(
    role: ProfilerVariantEvidenceRoleV1,
    origin: TruthOriginV1,
    identity: CaptureIdentityV1,
) -> ProfilerVariantEvidenceV1 {
    ProfilerVariantEvidenceV1 {
        role,
        origin,
        identity,
    }
}

fn available_identity(fact: IdentityFactV1) -> Option<ContentIdentityRecordV1> {
    match (fact.origin, fact.value, fact.unavailable_reason) {
        (TruthOriginV1::Declared, Some(value), None) => Some(value),
        _ => None,
    }
}

fn require_identity(
    expected: ContentIdentityRecordV1,
    actual: ContentIdentityRecordV1,
) -> Result<(), ProfilerVariantErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(ProfilerVariantErrorV1::StaleIdentity)
    }
}

fn require_optional_identity(
    expected: Option<ContentIdentityRecordV1>,
    actual: Option<ContentIdentityRecordV1>,
) -> Result<(), ProfilerVariantErrorV1> {
    if expected == actual {
        Ok(())
    } else {
        Err(ProfilerVariantErrorV1::StaleIdentity)
    }
}

fn check_opaque(bytes: &[u8]) -> Result<(), ProfilerVariantErrorV1> {
    let len = bytes.len() as u64;
    if len == 0 {
        Err(ProfilerVariantErrorV1::EmptyEvidence)
    } else if len > MAX_PROFILER_VARIANT_OPAQUE_EVIDENCE_BYTES_V1 {
        Err(ProfilerVariantErrorV1::EvidenceTooLarge)
    } else {
        Ok(())
    }
}

fn check_total_input(
    input: ProfilerVariantTreatmentInputV1<'_>,
) -> Result<(), ProfilerVariantErrorV1> {
    let required = [
        input.manifest.len(),
        input.semantic_workload.len(),
        input.raw_profiler_source.len(),
        input.bundle.len(),
        input.schedule.len(),
        input.artifact.len(),
    ];
    let total = required
        .into_iter()
        .chain(input.isa_projection.map(<[u8]>::len))
        .chain(input.counters.map(<[u8]>::len))
        .chain(input.pc_samples.map(<[u8]>::len))
        .try_fold(0_u64, |sum, len| {
            sum.checked_add(len as u64)
                .ok_or(ProfilerVariantErrorV1::EvidenceTooLarge)
        })?;
    if total > MAX_PROFILER_VARIANT_TREATMENT_BYTES_V1 {
        Err(ProfilerVariantErrorV1::EvidenceTooLarge)
    } else {
        Ok(())
    }
}

fn opaque_identity(
    domain: &[u8],
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    check_opaque(bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(bytes);
    identity_record(ContentSchemeV1::DomainSeparatedSha256, bytes.len(), hasher)
}

fn raw_identity(
    format_version: u16,
    bytes: &[u8],
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    if format_version == 0 {
        return Err(ProfilerVariantErrorV1::ArtifactMismatch);
    }
    check_opaque(bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let mut identity = identity_record(ContentSchemeV1::RawCanonicalSha256, bytes.len(), hasher)?;
    identity.format_version = format_version;
    Ok(identity)
}

fn manifest_identity(bytes: &[u8]) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    let len = bytes.len() as u64;
    if len == 0 || len > MAX_PROFILER_VARIANT_MANIFEST_BYTES_V1 {
        return Err(ProfilerVariantErrorV1::EvidenceTooLarge);
    }
    let manifest: ProfilerVariantTreatmentManifestV1 =
        serde_json::from_slice(bytes).map_err(|_| ProfilerVariantErrorV1::InvalidManifest)?;
    if serde_json::to_vec(&manifest).map_err(|_| ProfilerVariantErrorV1::InvalidManifest)? != bytes
    {
        return Err(ProfilerVariantErrorV1::NonCanonicalManifest);
    }
    opaque_identity(MANIFEST_DOMAIN_V1, bytes)
}

fn request_identity(
    request: ProfilerVariantComparisonRequestV1,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    let bytes =
        serde_json::to_vec(&request).map_err(|_| ProfilerVariantErrorV1::IdentityFailure)?;
    opaque_identity(REQUEST_DOMAIN_V1, &bytes)
}

fn resource_identity(
    resources: &ProfilerStaticResourcesV1,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    #[derive(Serialize)]
    struct IdentityPayload<'a> {
        origin: TruthOriginV1,
        artifact: ContentIdentityRecordV1,
        kernel_ordinal: u32,
        kernarg_segment_size: u64,
        kernarg_segment_alignment: u64,
        group_segment_fixed_size: u64,
        private_segment_fixed_size: u64,
        wavefront_size: u32,
        sgpr_count: u16,
        vgpr_count: u16,
        agpr_count: Option<u32>,
        sgpr_spill_count: Option<u32>,
        vgpr_spill_count: Option<u32>,
        max_flat_workgroup_size: u32,
        required_workgroup_size: Option<[u32; 3]>,
        max_workgroups: &'a [Option<u32>; 3],
    }
    let bytes = serde_json::to_vec(&IdentityPayload {
        origin: resources.origin,
        artifact: resources.artifact,
        kernel_ordinal: resources.kernel_ordinal,
        kernarg_segment_size: resources.kernarg_segment_size,
        kernarg_segment_alignment: resources.kernarg_segment_alignment,
        group_segment_fixed_size: resources.group_segment_fixed_size,
        private_segment_fixed_size: resources.private_segment_fixed_size,
        wavefront_size: resources.wavefront_size,
        sgpr_count: resources.sgpr_count,
        vgpr_count: resources.vgpr_count,
        agpr_count: resources.agpr_count,
        sgpr_spill_count: resources.sgpr_spill_count,
        vgpr_spill_count: resources.vgpr_spill_count,
        max_flat_workgroup_size: resources.max_flat_workgroup_size,
        required_workgroup_size: resources.required_workgroup_size,
        max_workgroups: &resources.max_workgroups,
    })
    .map_err(|_| ProfilerVariantErrorV1::IdentityFailure)?;
    opaque_identity(RESOURCE_DOMAIN_V1, &bytes)
}

fn placeholder_identity() -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    Ok(ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: PROFILER_VARIANT_SCHEMA_VERSION_V1,
        digest: CaptureIdentityV1::new([0xff; 32])
            .map_err(|_| ProfilerVariantErrorV1::IdentityFailure)?,
        canonical_len: 1,
    })
}

fn identity_record(
    scheme: ContentSchemeV1,
    len: usize,
    hasher: Sha256,
) -> Result<ContentIdentityRecordV1, ProfilerVariantErrorV1> {
    Ok(ContentIdentityRecordV1 {
        scheme,
        format_version: PROFILER_VARIANT_SCHEMA_VERSION_V1,
        digest: CaptureIdentityV1::new(hasher.finalize().into())
            .map_err(|_| ProfilerVariantErrorV1::IdentityFailure)?,
        canonical_len: u64::try_from(len).map_err(|_| ProfilerVariantErrorV1::EvidenceTooLarge)?,
    })
}

fn validate_result_size(
    result: &ProfilerVariantComparisonV1,
) -> Result<(), ProfilerVariantErrorV1> {
    let bytes = serde_json::to_vec(result).map_err(|_| ProfilerVariantErrorV1::InvalidResult)?;
    if bytes.len() as u64 > MAX_PROFILER_VARIANT_RESULT_BYTES_V1 {
        Err(ProfilerVariantErrorV1::ResultTooLarge)
    } else {
        Ok(())
    }
}

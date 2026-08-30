//! Inert capability-comparison and overhead-qualification records.

use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::OpaqueIdentityV1;

pub const QUALIFICATION_MANIFEST_SCHEMA_V1: &str = "fe2o3-debug-qualification-manifest-v1";
pub const MAX_QUALIFICATION_MANIFEST_BYTES_V1: usize = 256 * 1024;
pub const MAX_QUALIFICATION_TEXT_BYTES_V1: usize = 512;
pub const MAX_QUALIFICATION_URL_BYTES_V1: usize = 2 * 1024;
pub const MAX_QUALIFICATION_REPETITIONS_V1: u16 = 10_000;
pub const MAX_QUALIFICATION_COLLECTION_MILLISECONDS_V1: u64 = 900_000;
pub const MAX_QUALIFICATION_STORAGE_BYTES_V1: u64 = 4 * 1024 * 1024 * 1024;
pub const MAX_QUALIFICATION_RELATIVE_OVERHEAD_BASIS_POINTS_V1: u32 = 100_000;
pub const MAX_QUALIFICATION_CONTROL_LATENCY_NANOSECONDS_V1: u64 = 60_000_000_000;

const REQUIRED_COMPONENTS_V1: [QualificationComponentV1; 7] = [
    QualificationComponentV1::Fe2o3NativeKfdDebugger,
    QualificationComponentV1::Fe2o3RocgdbKfdDebugger,
    QualificationComponentV1::Rocgdb,
    QualificationComponentV1::Rocprofv3,
    QualificationComponentV1::RocprofComputeViewerAtt,
    QualificationComponentV1::HipAmdWorkflow,
    QualificationComponentV1::MojoGpuWorkflow,
];

const REQUIRED_CAPTURE_MODES_V1: [CaptureModeV1; 6] = [
    CaptureModeV1::NoCapture,
    CaptureModeV1::Counters,
    CaptureModeV1::PcSampling,
    CaptureModeV1::Att,
    CaptureModeV1::DebuggerStop,
    CaptureModeV1::Instrumented,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum QualificationManifestSchemaV1 {
    #[serde(rename = "fe2o3-debug-qualification-manifest-v1")]
    V1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationManifestV1 {
    pub schema: QualificationManifestSchemaV1,
    pub qualification_date_utc: String,
    pub environment: QualificationEnvironmentV1,
    pub components: Vec<ComponentQualificationV1>,
    pub baseline_comparator: CanonicalBaselineComparatorV1,
    pub overhead_budgets: Vec<CaptureModeQualificationV1>,
}

impl QualificationManifestV1 {
    pub fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        validate_date(&self.qualification_date_utc)?;
        self.environment.validate()?;

        if self.components.len() != REQUIRED_COMPONENTS_V1.len() {
            return Err(QualificationValidationErrorV1::IncompleteComponentMatrix);
        }
        for (record, expected) in self.components.iter().zip(REQUIRED_COMPONENTS_V1) {
            if record.component != expected {
                return Err(QualificationValidationErrorV1::IncompleteComponentMatrix);
            }
            record.validate()?;
        }

        self.baseline_comparator.validate(self)?;

        if self.overhead_budgets.len() != REQUIRED_CAPTURE_MODES_V1.len() {
            return Err(QualificationValidationErrorV1::IncompleteOverheadMatrix);
        }
        if self.overhead_budgets.iter().any(|record| {
            record.configuration_sha256 == self.baseline_comparator.raw_configuration_sha256
        }) || self
            .overhead_budgets
            .iter()
            .enumerate()
            .any(|(index, record)| {
                self.overhead_budgets[index + 1..]
                    .iter()
                    .any(|later| later.configuration_sha256 == record.configuration_sha256)
            })
        {
            return Err(QualificationValidationErrorV1::InvalidBudgetConfiguration);
        }
        for (record, expected) in self.overhead_budgets.iter().zip(REQUIRED_CAPTURE_MODES_V1) {
            if record.mode != expected {
                return Err(QualificationValidationErrorV1::IncompleteOverheadMatrix);
            }
            record.validate(self)?;
        }
        Ok(())
    }

    pub fn identity(&self) -> Result<OpaqueIdentityV1, QualificationValidationErrorV1> {
        self.validate()?;
        let encoded =
            serde_json::to_vec(self).map_err(|_| QualificationValidationErrorV1::EncodingFailed)?;
        if encoded.len() > MAX_QUALIFICATION_MANIFEST_BYTES_V1 {
            return Err(QualificationValidationErrorV1::ManifestTooLarge);
        }
        let mut hasher = Sha256::new();
        hasher.update(b"fe2o3-debug-qualification-manifest-v1\0");
        hasher.update(
            u64::try_from(encoded.len())
                .map_err(|_| QualificationValidationErrorV1::ManifestTooLarge)?
                .to_le_bytes(),
        );
        hasher.update(encoded);
        let digest: [u8; 32] = hasher.finalize().into();
        OpaqueIdentityV1::new(digest).map_err(|_| QualificationValidationErrorV1::EncodingFailed)
    }

    pub fn component(
        &self,
        component: QualificationComponentV1,
    ) -> Option<&ComponentQualificationV1> {
        self.components
            .iter()
            .find(|record| record.component == component)
    }

    pub fn evaluate_overhead(
        &self,
        mode: CaptureModeV1,
    ) -> Result<OverheadAssessmentV1, QualificationValidationErrorV1> {
        self.validate()?;
        let record = self
            .overhead_budgets
            .iter()
            .find(|record| record.mode == mode)
            .ok_or(QualificationValidationErrorV1::IncompleteOverheadMatrix)?;
        Ok(record.assessment_after_manifest_validation())
    }

    /// Decoding this inert, caller-supplied record never authenticates a tool,
    /// capture, measurement, or hardware observation.
    pub const fn grants_observation_authority(&self) -> bool {
        false
    }
}

pub fn decode_qualification_manifest_v1(
    bytes: &[u8],
) -> Result<QualificationManifestV1, QualificationDecodeErrorV1> {
    if bytes.len() > MAX_QUALIFICATION_MANIFEST_BYTES_V1 {
        return Err(QualificationDecodeErrorV1::ManifestTooLarge);
    }
    let manifest: QualificationManifestV1 =
        serde_json::from_slice(bytes).map_err(|_| QualificationDecodeErrorV1::MalformedJson)?;
    manifest
        .validate()
        .map_err(QualificationDecodeErrorV1::InvalidManifest)?;
    Ok(manifest)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QualificationEnvironmentV1 {
    pub host_class: String,
    pub kernel_release: String,
    pub os_release_sha256: OpaqueIdentityV1,
    pub kernel_build_sha256: OpaqueIdentityV1,
    pub target: String,
    pub kfd_topology_sha256: OpaqueIdentityV1,
}

impl QualificationEnvironmentV1 {
    fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        validate_text(&self.host_class, "host_class")?;
        validate_text(&self.kernel_release, "kernel_release")?;
        validate_text(&self.target, "target")
    }

    pub fn identity(&self) -> Result<OpaqueIdentityV1, QualificationValidationErrorV1> {
        self.validate()?;
        let encoded =
            serde_json::to_vec(self).map_err(|_| QualificationValidationErrorV1::EncodingFailed)?;
        let mut hasher = Sha256::new();
        hasher.update(b"fe2o3-debug-qualification-environment-v1\0");
        hasher.update(
            u64::try_from(encoded.len())
                .map_err(|_| QualificationValidationErrorV1::EncodingFailed)?
                .to_le_bytes(),
        );
        hasher.update(encoded);
        let digest: [u8; 32] = hasher.finalize().into();
        OpaqueIdentityV1::new(digest).map_err(|_| QualificationValidationErrorV1::EncodingFailed)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CanonicalBaselineComparatorV1 {
    pub raw_configuration_sha256: OpaqueIdentityV1,
    pub no_capture_configuration_sha256: OpaqueIdentityV1,
    pub availability: BaselineComparatorAvailabilityV1,
}

impl CanonicalBaselineComparatorV1 {
    fn validate(
        &self,
        manifest: &QualificationManifestV1,
    ) -> Result<(), QualificationValidationErrorV1> {
        if self.raw_configuration_sha256 == self.no_capture_configuration_sha256 {
            return Err(QualificationValidationErrorV1::InvalidBaselineComparator);
        }
        let no_capture = manifest
            .overhead_budgets
            .first()
            .filter(|record| record.mode == CaptureModeV1::NoCapture)
            .ok_or(QualificationValidationErrorV1::IncompleteOverheadMatrix)?;
        if no_capture.configuration_sha256 != self.no_capture_configuration_sha256 {
            return Err(QualificationValidationErrorV1::InvalidBaselineComparator);
        }
        let BaselineComparatorAvailabilityV1::CallerBoundAvailable { record } = &self.availability
        else {
            return Ok(());
        };
        let collector_component = manifest
            .component(QualificationComponentV1::Fe2o3NativeKfdDebugger)
            .ok_or(QualificationValidationErrorV1::BaselineComparatorUnavailable)?;
        if !collector_component.installation.is_usable() {
            return Err(QualificationValidationErrorV1::BaselineComparatorUnavailable);
        }
        let collector = collector_component
            .installation
            .identity()
            .ok_or(QualificationValidationErrorV1::BaselineComparatorUnavailable)?;
        if record.environment_identity != manifest.environment.identity()?
            || record.collector_content_sha256 != collector.content_sha256
            || record.raw_evidence_id == record.no_capture_evidence_id
            || record.raw_duration_nanoseconds == 0
            || record.no_capture_duration_nanoseconds == 0
            || record.warmups == 0
            || record.warmups > MAX_QUALIFICATION_REPETITIONS_V1
            || record.repetitions == 0
            || record.repetitions > MAX_QUALIFICATION_REPETITIONS_V1
        {
            return Err(QualificationValidationErrorV1::InvalidBaselineComparator);
        }
        validate_text(&record.clock_domain, "baseline clock domain")
    }

    pub fn identity(&self) -> Result<OpaqueIdentityV1, QualificationValidationErrorV1> {
        let encoded =
            serde_json::to_vec(self).map_err(|_| QualificationValidationErrorV1::EncodingFailed)?;
        let mut hasher = Sha256::new();
        hasher.update(b"fe2o3-debug-canonical-baseline-comparator-v1\0");
        hasher.update(
            u64::try_from(encoded.len())
                .map_err(|_| QualificationValidationErrorV1::EncodingFailed)?
                .to_le_bytes(),
        );
        hasher.update(encoded);
        let digest: [u8; 32] = hasher.finalize().into();
        OpaqueIdentityV1::new(digest).map_err(|_| QualificationValidationErrorV1::EncodingFailed)
    }

    fn available_record(&self) -> Option<&CallerBoundBaselineComparatorRecordV1> {
        match &self.availability {
            BaselineComparatorAvailabilityV1::CallerBoundAvailable { record } => Some(record),
            BaselineComparatorAvailabilityV1::Unavailable { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum BaselineComparatorAvailabilityV1 {
    CallerBoundAvailable {
        record: Box<CallerBoundBaselineComparatorRecordV1>,
    },
    Unavailable {
        reason: BaselineComparatorUnavailableReasonV1,
        evidence_id: OpaqueIdentityV1,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselineComparatorUnavailableReasonV1 {
    NotMeasured,
    ComparatorUnavailable,
    TargetUnsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CallerBoundBaselineComparatorRecordV1 {
    pub workload_identity: OpaqueIdentityV1,
    pub input_identity: OpaqueIdentityV1,
    pub artifact_identity: OpaqueIdentityV1,
    pub environment_identity: OpaqueIdentityV1,
    pub device_identity: OpaqueIdentityV1,
    pub collector_content_sha256: OpaqueIdentityV1,
    pub raw_evidence_id: OpaqueIdentityV1,
    pub no_capture_evidence_id: OpaqueIdentityV1,
    pub warmups: u16,
    pub repetitions: u16,
    pub statistic: DurationStatisticV1,
    pub clock_domain: String,
    pub raw_duration_nanoseconds: u64,
    pub no_capture_duration_nanoseconds: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationComponentV1 {
    Fe2o3NativeKfdDebugger,
    Fe2o3RocgdbKfdDebugger,
    Rocgdb,
    Rocprofv3,
    RocprofComputeViewerAtt,
    HipAmdWorkflow,
    MojoGpuWorkflow,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentQualificationV1 {
    pub component: QualificationComponentV1,
    pub installation: ComponentInstallationV1,
    pub capabilities: ComparisonCapabilitiesV1,
}

impl ComponentQualificationV1 {
    fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        self.installation.validate()?;
        self.capabilities.validate(self.installation.is_usable())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ComponentInstallationV1 {
    CallerBoundObservedUsable {
        identity: CallerBoundComponentIdentityV1,
        evidence_id: OpaqueIdentityV1,
    },
    CallerBoundObservedUnusable {
        identity: CallerBoundComponentIdentityV1,
        reason: InstallationUnavailableReasonV1,
        evidence_id: OpaqueIdentityV1,
    },
    Unavailable {
        reason: InstallationUnavailableReasonV1,
        evidence_id: OpaqueIdentityV1,
    },
}

impl ComponentInstallationV1 {
    fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        match self {
            Self::CallerBoundObservedUsable { identity, .. } => {
                identity.validate()?;
                if !matches!(identity.version, VersionEvidenceV1::Exact { .. }) {
                    return Err(QualificationValidationErrorV1::UsableVersionUnavailable);
                }
            }
            Self::CallerBoundObservedUnusable { identity, .. } => identity.validate()?,
            Self::Unavailable { .. } => {}
        }
        Ok(())
    }

    const fn is_usable(&self) -> bool {
        matches!(self, Self::CallerBoundObservedUsable { .. })
    }

    pub const fn identity(&self) -> Option<&CallerBoundComponentIdentityV1> {
        match self {
            Self::CallerBoundObservedUsable { identity, .. }
            | Self::CallerBoundObservedUnusable { identity, .. } => Some(identity),
            Self::Unavailable { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationUnavailableReasonV1 {
    NotInstalled,
    DependencyUnavailable,
    VersionProbeUnavailable,
    TargetUnsupported,
    NotQualified,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CallerBoundComponentIdentityV1 {
    pub kind: ComponentArtifactKindV1,
    pub version: VersionEvidenceV1,
    pub content_sha256: OpaqueIdentityV1,
    pub configuration_sha256: OpaqueIdentityV1,
}

impl CallerBoundComponentIdentityV1 {
    fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        if let VersionEvidenceV1::Exact { value } = &self.version {
            validate_text(value, "component version")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentArtifactKindV1 {
    Executable,
    KernelInterface,
    WorkflowDriver,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum VersionEvidenceV1 {
    Exact { value: String },
    Unavailable { reason: VersionUnavailableReasonV1 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionUnavailableReasonV1 {
    ProbeFailed,
    NotReported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComparisonCapabilitiesV1 {
    pub live_gpu_state: CapabilityEvidenceV1,
    pub source_break_and_step: CapabilityEvidenceV1,
    pub deterministic_cpu_diagnosis: CapabilityEvidenceV1,
    pub semantic_identity_graph: CapabilityEvidenceV1,
    pub dispatch_counter_pc_att_collection: CapabilityEvidenceV1,
    pub decoded_att_visualization: CapabilityEvidenceV1,
    pub bounded_read_only_agent_queries: CapabilityEvidenceV1,
    pub supported_hardware_path: CapabilityEvidenceV1,
}

impl ComparisonCapabilitiesV1 {
    fn validate(&self, installation_usable: bool) -> Result<(), QualificationValidationErrorV1> {
        for capability in [
            &self.live_gpu_state,
            &self.source_break_and_step,
            &self.deterministic_cpu_diagnosis,
            &self.semantic_identity_graph,
            &self.dispatch_counter_pc_att_collection,
            &self.decoded_att_visualization,
            &self.bounded_read_only_agent_queries,
            &self.supported_hardware_path,
        ] {
            capability.validate()?;
            if matches!(capability, CapabilityEvidenceV1::CallerBoundObserved { .. })
                && !installation_usable
            {
                return Err(QualificationValidationErrorV1::ObservedCapabilityWithoutUsableTool);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
pub enum CapabilityEvidenceV1 {
    CallerBoundObserved {
        evidence_id: OpaqueIdentityV1,
        limitations: String,
    },
    Documented {
        reference: DocumentationReferenceV1,
        limitations: String,
    },
    Unavailable {
        reason: ComparisonCapabilityUnavailableReasonV1,
        evidence_id: OpaqueIdentityV1,
        limitations: String,
    },
}

impl CapabilityEvidenceV1 {
    fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        let limitations = match self {
            Self::CallerBoundObserved { limitations, .. }
            | Self::Documented { limitations, .. }
            | Self::Unavailable { limitations, .. } => limitations,
        };
        validate_text(limitations, "capability limitations")?;
        if let Self::Documented { reference, .. } = self {
            reference.validate()?;
        }
        Ok(())
    }

    pub const fn grants_observation_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonCapabilityUnavailableReasonV1 {
    NotImplemented,
    NotInstalled,
    InstallationUnusable,
    NotDocumented,
    NotObserved,
    RequiresDifferentHardware,
    CollectorOutputNotAdmitted,
    DependencyUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DocumentationReferenceV1 {
    pub title: String,
    pub url: String,
    pub version_or_access_date: String,
    pub reference_identity: OpaqueIdentityV1,
}

impl DocumentationReferenceV1 {
    fn validate(&self) -> Result<(), QualificationValidationErrorV1> {
        validate_text(&self.title, "documentation title")?;
        validate_text(&self.version_or_access_date, "documentation version")?;
        validate_url(&self.url)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureModeV1 {
    NoCapture,
    Counters,
    PcSampling,
    Att,
    DebuggerStop,
    Instrumented,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureModeQualificationV1 {
    pub mode: CaptureModeV1,
    pub collector: QualificationComponentV1,
    pub configuration_sha256: OpaqueIdentityV1,
    pub policy: OverheadBudgetPolicyV1,
    pub observation: OverheadObservationV1,
}

impl CaptureModeQualificationV1 {
    fn validate(
        &self,
        manifest: &QualificationManifestV1,
    ) -> Result<(), QualificationValidationErrorV1> {
        if self.collector != expected_collector(self.mode) {
            return Err(QualificationValidationErrorV1::InvalidBudgetCollector);
        }
        let component = manifest
            .component(self.collector)
            .ok_or(QualificationValidationErrorV1::UnknownBudgetCollector)?;
        self.policy.validate(self.mode)?;
        self.observation.validate(self, component, manifest)?;
        Ok(())
    }

    fn assessment_after_manifest_validation(&self) -> OverheadAssessmentV1 {
        if self.policy.status == BudgetPolicyStatusV1::Candidate {
            return OverheadAssessmentV1::CandidatePolicy;
        }
        let OverheadObservationV1::Measured {
            measurement: measured,
        } = &self.observation
        else {
            return OverheadAssessmentV1::Unavailable;
        };
        if !measured.loss_free || measured.truncated {
            return OverheadAssessmentV1::Failed;
        }
        if measured.storage_bytes > self.policy.max_storage_bytes
            || measured.collection_milliseconds > self.policy.max_collection_milliseconds
            || measured.warmups < self.policy.min_warmups
            || measured.repetitions < self.policy.min_repetitions
            || measured.statistic != self.policy.statistic
        {
            return OverheadAssessmentV1::Failed;
        }
        let passed = match (&self.policy.metric, &measured.metric) {
            (
                OverheadBudgetMetricV1::RelativeDuration {
                    max_overhead_basis_points,
                },
                MeasuredOverheadMetricV1::RelativeDuration {
                    baseline_nanoseconds,
                    captured_nanoseconds,
                },
            ) => {
                relative_overhead_basis_points(*baseline_nanoseconds, *captured_nanoseconds)
                    <= u64::from(*max_overhead_basis_points)
            }
            (
                OverheadBudgetMetricV1::StopResumeControlLatency {
                    max_latency_nanoseconds,
                },
                MeasuredOverheadMetricV1::StopResumeControlLatency {
                    latency_nanoseconds,
                },
            ) => latency_nanoseconds <= max_latency_nanoseconds,
            _ => false,
        };
        if passed {
            OverheadAssessmentV1::CallerBoundPolicySatisfied
        } else {
            OverheadAssessmentV1::Failed
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OverheadBudgetPolicyV1 {
    pub status: BudgetPolicyStatusV1,
    pub metric: OverheadBudgetMetricV1,
    pub max_storage_bytes: u64,
    pub max_collection_milliseconds: u64,
    pub min_warmups: u16,
    pub min_repetitions: u16,
    pub statistic: DurationStatisticV1,
}

impl OverheadBudgetPolicyV1 {
    fn validate(&self, mode: CaptureModeV1) -> Result<(), QualificationValidationErrorV1> {
        if self.max_storage_bytes > MAX_QUALIFICATION_STORAGE_BYTES_V1 {
            return Err(QualificationValidationErrorV1::BudgetOutOfRange("storage"));
        }
        if self.max_collection_milliseconds == 0
            || self.max_collection_milliseconds > MAX_QUALIFICATION_COLLECTION_MILLISECONDS_V1
        {
            return Err(QualificationValidationErrorV1::BudgetOutOfRange(
                "collection duration",
            ));
        }
        if self.min_warmups == 0
            || self.min_warmups > MAX_QUALIFICATION_REPETITIONS_V1
            || self.min_repetitions == 0
            || self.min_repetitions > MAX_QUALIFICATION_REPETITIONS_V1
        {
            return Err(QualificationValidationErrorV1::BudgetOutOfRange(
                "repetitions",
            ));
        }
        match self.metric {
            OverheadBudgetMetricV1::RelativeDuration {
                max_overhead_basis_points,
            } => {
                if mode == CaptureModeV1::DebuggerStop
                    || max_overhead_basis_points
                        > MAX_QUALIFICATION_RELATIVE_OVERHEAD_BASIS_POINTS_V1
                {
                    return Err(QualificationValidationErrorV1::InvalidBudgetMetric);
                }
            }
            OverheadBudgetMetricV1::StopResumeControlLatency {
                max_latency_nanoseconds,
            } => {
                if mode != CaptureModeV1::DebuggerStop
                    || max_latency_nanoseconds == 0
                    || max_latency_nanoseconds > MAX_QUALIFICATION_CONTROL_LATENCY_NANOSECONDS_V1
                {
                    return Err(QualificationValidationErrorV1::InvalidBudgetMetric);
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPolicyStatusV1 {
    Candidate,
    Approved,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "metric", rename_all = "snake_case", deny_unknown_fields)]
pub enum OverheadBudgetMetricV1 {
    RelativeDuration { max_overhead_basis_points: u32 },
    StopResumeControlLatency { max_latency_nanoseconds: u64 },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DurationStatisticV1 {
    Median,
    P95,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum OverheadObservationV1 {
    Measured {
        measurement: Box<MeasuredOverheadV1>,
    },
    Unavailable {
        reason: OverheadUnavailableReasonV1,
        evidence_id: OpaqueIdentityV1,
    },
}

impl OverheadObservationV1 {
    fn validate(
        &self,
        qualification: &CaptureModeQualificationV1,
        component: &ComponentQualificationV1,
        manifest: &QualificationManifestV1,
    ) -> Result<(), QualificationValidationErrorV1> {
        let Self::Measured {
            measurement: measured,
        } = self
        else {
            return Ok(());
        };
        if !component.installation.is_usable() {
            return Err(QualificationValidationErrorV1::MeasuredWithUnusableCollector);
        }
        if measured.configuration_sha256 != qualification.configuration_sha256 {
            return Err(QualificationValidationErrorV1::MeasurementConfigurationMismatch);
        }
        let identity = component
            .installation
            .identity()
            .ok_or(QualificationValidationErrorV1::MeasuredWithUnusableCollector)?;
        let baseline = manifest
            .baseline_comparator
            .available_record()
            .ok_or(QualificationValidationErrorV1::BaselineComparatorUnavailable)?;
        if measured.baseline_comparator_identity != manifest.baseline_comparator.identity()? {
            return Err(QualificationValidationErrorV1::MeasurementBaselineMismatch);
        }
        let expected_baseline_configuration = if qualification.mode == CaptureModeV1::NoCapture {
            manifest.baseline_comparator.raw_configuration_sha256
        } else {
            manifest.baseline_comparator.no_capture_configuration_sha256
        };
        let expected_baseline_evidence = if qualification.mode == CaptureModeV1::NoCapture {
            baseline.raw_evidence_id
        } else {
            baseline.no_capture_evidence_id
        };
        if measured.comparison.collector_content_sha256 != identity.content_sha256
            || measured.comparison.captured_configuration_sha256
                != qualification.configuration_sha256
            || measured.comparison.baseline_configuration_sha256 != expected_baseline_configuration
            || measured.baseline_evidence_id != expected_baseline_evidence
            || measured.captured_evidence_id == expected_baseline_evidence
            || measured.comparison.workload_identity != baseline.workload_identity
            || measured.comparison.input_identity != baseline.input_identity
            || measured.comparison.artifact_identity != baseline.artifact_identity
            || measured.comparison.environment_identity != baseline.environment_identity
            || measured.comparison.device_identity != baseline.device_identity
        {
            return Err(QualificationValidationErrorV1::MeasurementBaselineMismatch);
        }
        if qualification.mode == CaptureModeV1::NoCapture
            && measured.captured_evidence_id != baseline.no_capture_evidence_id
        {
            return Err(QualificationValidationErrorV1::MeasurementBaselineMismatch);
        }
        if measured.warmups == 0
            || measured.warmups > MAX_QUALIFICATION_REPETITIONS_V1
            || measured.repetitions == 0
            || measured.repetitions > MAX_QUALIFICATION_REPETITIONS_V1
            || measured.collection_milliseconds == 0
            || measured.collection_milliseconds > MAX_QUALIFICATION_COLLECTION_MILLISECONDS_V1
            || measured.storage_bytes > MAX_QUALIFICATION_STORAGE_BYTES_V1
        {
            return Err(QualificationValidationErrorV1::MeasurementOutOfRange);
        }
        validate_text(&measured.clock_domain, "measurement clock domain")?;
        match (&qualification.policy.metric, &measured.metric) {
            (
                OverheadBudgetMetricV1::RelativeDuration { .. },
                MeasuredOverheadMetricV1::RelativeDuration {
                    baseline_nanoseconds,
                    captured_nanoseconds,
                },
            ) if *baseline_nanoseconds != 0 && *captured_nanoseconds != 0 => {
                if measured.warmups != baseline.warmups
                    || measured.repetitions != baseline.repetitions
                    || measured.statistic != baseline.statistic
                    || measured.clock_domain != baseline.clock_domain
                {
                    return Err(QualificationValidationErrorV1::MeasurementBaselineMismatch);
                }
                let expected_baseline_duration = if qualification.mode == CaptureModeV1::NoCapture {
                    baseline.raw_duration_nanoseconds
                } else {
                    baseline.no_capture_duration_nanoseconds
                };
                if *baseline_nanoseconds != expected_baseline_duration
                    || (qualification.mode == CaptureModeV1::NoCapture
                        && *captured_nanoseconds != baseline.no_capture_duration_nanoseconds)
                {
                    Err(QualificationValidationErrorV1::MeasurementBaselineMismatch)
                } else {
                    Ok(())
                }
            }
            (
                OverheadBudgetMetricV1::StopResumeControlLatency { .. },
                MeasuredOverheadMetricV1::StopResumeControlLatency {
                    latency_nanoseconds,
                },
            ) if *latency_nanoseconds != 0 => Ok(()),
            _ => Err(QualificationValidationErrorV1::InvalidMeasurementMetric),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OverheadUnavailableReasonV1 {
    NotMeasured,
    ToolUnavailable,
    CapabilityUnsupported,
    TargetUnsupported,
    ComparatorUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MeasuredOverheadV1 {
    pub configuration_sha256: OpaqueIdentityV1,
    pub baseline_comparator_identity: OpaqueIdentityV1,
    pub baseline_evidence_id: OpaqueIdentityV1,
    pub captured_evidence_id: OpaqueIdentityV1,
    pub comparison: OverheadComparisonAxesV1,
    pub warmups: u16,
    pub repetitions: u16,
    pub statistic: DurationStatisticV1,
    pub clock_domain: String,
    pub metric: MeasuredOverheadMetricV1,
    pub storage_bytes: u64,
    pub collection_milliseconds: u64,
    pub loss_free: bool,
    pub truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OverheadComparisonAxesV1 {
    pub workload_identity: OpaqueIdentityV1,
    pub input_identity: OpaqueIdentityV1,
    pub artifact_identity: OpaqueIdentityV1,
    pub environment_identity: OpaqueIdentityV1,
    pub device_identity: OpaqueIdentityV1,
    pub collector_content_sha256: OpaqueIdentityV1,
    pub baseline_configuration_sha256: OpaqueIdentityV1,
    pub captured_configuration_sha256: OpaqueIdentityV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "metric", rename_all = "snake_case", deny_unknown_fields)]
pub enum MeasuredOverheadMetricV1 {
    RelativeDuration {
        baseline_nanoseconds: u64,
        captured_nanoseconds: u64,
    },
    StopResumeControlLatency {
        latency_nanoseconds: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OverheadAssessmentV1 {
    CandidatePolicy,
    Unavailable,
    CallerBoundPolicySatisfied,
    Failed,
}

impl OverheadAssessmentV1 {
    pub const fn grants_qualification_authority(self) -> bool {
        false
    }
}

const fn expected_collector(mode: CaptureModeV1) -> QualificationComponentV1 {
    match mode {
        CaptureModeV1::NoCapture | CaptureModeV1::Instrumented => {
            QualificationComponentV1::Fe2o3NativeKfdDebugger
        }
        CaptureModeV1::Counters | CaptureModeV1::PcSampling | CaptureModeV1::Att => {
            QualificationComponentV1::Rocprofv3
        }
        CaptureModeV1::DebuggerStop => QualificationComponentV1::Fe2o3RocgdbKfdDebugger,
    }
}

fn relative_overhead_basis_points(baseline: u64, captured: u64) -> u64 {
    if baseline == 0 || captured == 0 {
        return u64::MAX;
    }
    let difference = captured.saturating_sub(baseline);
    u64::try_from(
        u128::from(difference)
            .saturating_mul(10_000)
            .div_ceil(u128::from(baseline)),
    )
    .unwrap_or(u64::MAX)
}

fn validate_text(value: &str, field: &'static str) -> Result<(), QualificationValidationErrorV1> {
    if value.is_empty()
        || value.len() > MAX_QUALIFICATION_TEXT_BYTES_V1
        || value.chars().any(char::is_control)
    {
        return Err(QualificationValidationErrorV1::InvalidText(field));
    }
    Ok(())
}

fn validate_url(value: &str) -> Result<(), QualificationValidationErrorV1> {
    if value.len() > MAX_QUALIFICATION_URL_BYTES_V1
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || !value.starts_with("https://")
    {
        return Err(QualificationValidationErrorV1::InvalidDocumentationUrl);
    }
    let remainder = &value["https://".len()..];
    let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..authority_end];
    if authority.is_empty()
        || authority.contains('@')
        || authority.contains('\\')
        || authority.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(QualificationValidationErrorV1::InvalidDocumentationUrl);
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port)),
        Some(_) => return Err(QualificationValidationErrorV1::InvalidDocumentationUrl),
        None => (authority, None),
    };
    if host.is_empty()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(QualificationValidationErrorV1::InvalidDocumentationUrl);
    }
    if let Some(port) = port {
        let parsed = port
            .parse::<u16>()
            .map_err(|_| QualificationValidationErrorV1::InvalidDocumentationUrl)?;
        if parsed == 0 {
            return Err(QualificationValidationErrorV1::InvalidDocumentationUrl);
        }
    }
    Ok(())
}

fn validate_date(value: &str) -> Result<(), QualificationValidationErrorV1> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(QualificationValidationErrorV1::InvalidQualificationDate);
    }
    let year = u32::from(bytes[0] - b'0') * 1_000
        + u32::from(bytes[1] - b'0') * 100
        + u32::from(bytes[2] - b'0') * 10
        + u32::from(bytes[3] - b'0');
    let month = usize::from((bytes[5] - b'0') * 10 + bytes[6] - b'0');
    let day = usize::from((bytes[8] - b'0') * 10 + bytes[9] - b'0');
    if year == 0 || !(1..=12).contains(&month) {
        return Err(QualificationValidationErrorV1::InvalidQualificationDate);
    }
    let leap = year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400);
    let month_days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day == 0 || day > month_days[month - 1] {
        return Err(QualificationValidationErrorV1::InvalidQualificationDate);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QualificationDecodeErrorV1 {
    ManifestTooLarge,
    MalformedJson,
    InvalidManifest(QualificationValidationErrorV1),
}

impl fmt::Display for QualificationDecodeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ManifestTooLarge => formatter.write_str("qualification manifest is too large"),
            Self::MalformedJson => formatter.write_str("qualification manifest JSON is malformed"),
            Self::InvalidManifest(error) => {
                write!(formatter, "invalid qualification manifest: {error}")
            }
        }
    }
}

impl std::error::Error for QualificationDecodeErrorV1 {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QualificationValidationErrorV1 {
    InvalidQualificationDate,
    InvalidText(&'static str),
    InvalidDocumentationUrl,
    IncompleteComponentMatrix,
    IncompleteOverheadMatrix,
    UsableVersionUnavailable,
    ObservedCapabilityWithoutUsableTool,
    UnknownBudgetCollector,
    InvalidBudgetCollector,
    InvalidBudgetConfiguration,
    InvalidBaselineComparator,
    BaselineComparatorUnavailable,
    BudgetOutOfRange(&'static str),
    InvalidBudgetMetric,
    MeasuredWithUnusableCollector,
    MeasurementConfigurationMismatch,
    MeasurementBaselineMismatch,
    MeasurementOutOfRange,
    InvalidMeasurementMetric,
    ManifestTooLarge,
    EncodingFailed,
}

impl fmt::Display for QualificationValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidQualificationDate => formatter.write_str("qualification date is invalid"),
            Self::InvalidText(field) => write!(formatter, "{field} is empty, oversized, or contains control characters"),
            Self::InvalidDocumentationUrl => formatter.write_str("documentation URL is not a bounded HTTPS URL"),
            Self::IncompleteComponentMatrix => formatter.write_str("component matrix is incomplete, duplicated, or out of canonical order"),
            Self::IncompleteOverheadMatrix => formatter.write_str("overhead matrix is incomplete, duplicated, or out of canonical order"),
            Self::UsableVersionUnavailable => formatter.write_str("an observed usable component lacks an exact version"),
            Self::ObservedCapabilityWithoutUsableTool => formatter.write_str("a caller-bound observation claim is attached to a component that was not recorded as caller-bound usable"),
            Self::UnknownBudgetCollector => formatter.write_str("overhead budget names an unknown collector component"),
            Self::InvalidBudgetCollector => formatter.write_str("capture mode names the wrong collector component"),
            Self::InvalidBudgetConfiguration => formatter.write_str("capture-mode configurations are duplicated or collide with the raw baseline"),
            Self::InvalidBaselineComparator => formatter.write_str("canonical baseline comparator is inconsistent with the manifest"),
            Self::BaselineComparatorUnavailable => formatter.write_str("canonical no-capture baseline comparator is unavailable"),
            Self::BudgetOutOfRange(field) => write!(formatter, "{field} budget is out of range"),
            Self::InvalidBudgetMetric => formatter.write_str("capture mode and budget metric are incompatible"),
            Self::MeasuredWithUnusableCollector => formatter.write_str("measured overhead names a collector that was not observed usable"),
            Self::MeasurementConfigurationMismatch => formatter.write_str("measurement configuration does not match its declared budget"),
            Self::MeasurementBaselineMismatch => formatter.write_str("measurement does not bind the canonical baseline comparator, axes, evidence, or configuration"),
            Self::MeasurementOutOfRange => formatter.write_str("overhead measurement is out of range"),
            Self::InvalidMeasurementMetric => formatter.write_str("measurement and budget metrics are incompatible or zero"),
            Self::ManifestTooLarge => formatter.write_str("qualification manifest is too large"),
            Self::EncodingFailed => formatter.write_str("qualification manifest identity encoding failed"),
        }
    }
}

impl std::error::Error for QualificationValidationErrorV1 {}

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

        if self.overhead_budgets.len() != REQUIRED_CAPTURE_MODES_V1.len() {
            return Err(QualificationValidationErrorV1::IncompleteOverheadMatrix);
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
        self.observation
            .validate(self, component, manifest.environment.identity()?)?;
        Ok(())
    }

    pub fn assessment(&self) -> OverheadAssessmentV1 {
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
        environment_identity: OpaqueIdentityV1,
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
        if measured.comparison.collector_content_sha256 != identity.content_sha256
            || measured.comparison.captured_configuration_sha256
                != qualification.configuration_sha256
            || measured.comparison.baseline_configuration_sha256
                == measured.comparison.captured_configuration_sha256
            || measured.comparison.environment_identity != environment_identity
        {
            return Err(QualificationValidationErrorV1::MeasurementComparisonMismatch);
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
            ) if *baseline_nanoseconds != 0 && *captured_nanoseconds != 0 => Ok(()),
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
        || value.chars().any(char::is_control)
        || !value.starts_with("https://")
    {
        return Err(QualificationValidationErrorV1::InvalidDocumentationUrl);
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
    let month = (bytes[5] - b'0') * 10 + bytes[6] - b'0';
    let day = (bytes[8] - b'0') * 10 + bytes[9] - b'0';
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
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
    BudgetOutOfRange(&'static str),
    InvalidBudgetMetric,
    MeasuredWithUnusableCollector,
    MeasurementConfigurationMismatch,
    MeasurementComparisonMismatch,
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
            Self::BudgetOutOfRange(field) => write!(formatter, "{field} budget is out of range"),
            Self::InvalidBudgetMetric => formatter.write_str("capture mode and budget metric are incompatible"),
            Self::MeasuredWithUnusableCollector => formatter.write_str("measured overhead names a collector that was not observed usable"),
            Self::MeasurementConfigurationMismatch => formatter.write_str("measurement configuration does not match its declared budget"),
            Self::MeasurementComparisonMismatch => formatter.write_str("measurement comparison axes do not match the manifest environment, collector, or configuration"),
            Self::MeasurementOutOfRange => formatter.write_str("overhead measurement is out of range"),
            Self::InvalidMeasurementMetric => formatter.write_str("measurement and budget metrics are incompatible or zero"),
            Self::ManifestTooLarge => formatter.write_str("qualification manifest is too large"),
            Self::EncodingFailed => formatter.write_str("qualification manifest identity encoding failed"),
        }
    }
}

impl std::error::Error for QualificationValidationErrorV1 {}

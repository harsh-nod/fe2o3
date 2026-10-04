//! Canonical juxtaposition of one direct-KFD runtime capture and one rocprofv3 run.

use fe2o3_profiler_protocol::{
    KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1, decode_kfd_runtime_profile_v1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;

pub(crate) const LIVE_QUALIFICATION_FILE_V1: &str =
    "fe2o3-direct-kfd-rocprof-qualification-v1.json";
pub(crate) const LIVE_QUALIFICATION_REDO_FILE_V1: &str =
    ".fe2o3-direct-kfd-rocprof-qualification-v1.redo";
pub(crate) const LIVE_RUNTIME_CAPTURE_FILE_V1: &str = "fe2o3-direct-kfd-runtime-profile-v1.json";
pub(crate) const LIVE_RUNTIME_CAPTURE_REDO_FILE_V1: &str =
    ".fe2o3-direct-kfd-runtime-profile-v1.redo";
pub(crate) const MAX_LIVE_QUALIFICATION_BYTES_V1: usize = 4 * 1024 * 1024;
const LIVE_QUALIFICATION_SCHEMA_V1: &str = "fe2o3-direct-kfd-rocprof-qualification-v1";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawContentIdentityV1 {
    pub(crate) sha256: [u8; 32],
    pub(crate) byte_len: u64,
}

impl RawContentIdentityV1 {
    pub(crate) fn observed(bytes: &[u8]) -> Result<Self, LiveQualificationErrorV1> {
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: u64::try_from(bytes.len())
                .map_err(|_| LiveQualificationErrorV1::SizeOverflow)?,
        })
    }

    fn validate(self) -> Result<(), LiveQualificationErrorV1> {
        if self.sha256 == [0; 32] {
            return Err(LiveQualificationErrorV1::InvalidIdentity);
        }
        Ok(())
    }

    fn validate_nonempty(self) -> Result<(), LiveQualificationErrorV1> {
        self.validate()?;
        if self.byte_len == 0 {
            return Err(LiveQualificationErrorV1::InvalidIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CollectorArtifactV1 {
    pub(crate) relative_path: String,
    pub(crate) content: RawContentIdentityV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CollectorReleaseV1 {
    RocprofilerSdk1_1_0Git97f5574,
    UnavailableUnrecognizedExactTool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum LiveQualificationOutcomeV1 {
    #[serde(rename = "runtime_dispatch_observed_collector_completed_no_artifacts")]
    DispatchObservedCollectorCompletedNoArtifacts,
    #[serde(rename = "runtime_dispatch_observed_collector_artifacts_present_unjoined")]
    DispatchObservedCollectorArtifactsPresentUnjoined,
    #[serde(rename = "runtime_capture_contains_no_dispatch")]
    CaptureContainsNoDispatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CapabilityFactV1 {
    NotRequestedOrProbed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) enum JoinUnavailableReasonV1 {
    #[serde(rename = "no_admitted_common_dispatch_identity")]
    MissingCommonDispatchIdentity,
    #[serde(rename = "no_admitted_runtime_to_code_object_relation")]
    RuntimeCodeObjectUnrelated,
    #[serde(rename = "no_admitted_common_clock_relation")]
    IncomparableClocks,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeCaptureSummaryV1 {
    pub(crate) content: RawContentIdentityV1,
    pub(crate) target_profile: String,
    pub(crate) wave_width: u16,
    pub(crate) observed_events: u64,
    pub(crate) dropped_events: u64,
    pub(crate) complete_runtime_operation_history: bool,
    pub(crate) dispatches_published: u64,
    pub(crate) dispatches_completed: u64,
    pub(crate) submissions_released: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DirectKfdRocprofQualificationV1 {
    pub(crate) schema: String,
    pub(crate) schema_version: u16,
    pub(crate) plan_sha256: [u8; 32],
    pub(crate) collector_executable: RawContentIdentityV1,
    pub(crate) collector_release: CollectorReleaseV1,
    pub(crate) collector_closure: RawContentIdentityV1,
    pub(crate) collector_configuration: RawContentIdentityV1,
    pub(crate) collector_argv: RawContentIdentityV1,
    pub(crate) collector_environment: RawContentIdentityV1,
    pub(crate) target_executable: RawContentIdentityV1,
    pub(crate) target_argv: RawContentIdentityV1,
    pub(crate) collector_exit_success: bool,
    pub(crate) collector_stdout: RawContentIdentityV1,
    pub(crate) collector_stdout_overflow: bool,
    pub(crate) collector_stderr: RawContentIdentityV1,
    pub(crate) collector_stderr_overflow: bool,
    pub(crate) collector_inventory_complete: bool,
    pub(crate) collector_artifacts: Vec<CollectorArtifactV1>,
    pub(crate) runtime: RuntimeCaptureSummaryV1,
    pub(crate) outcome: LiveQualificationOutcomeV1,
    pub(crate) dispatch_join: JoinUnavailableReasonV1,
    pub(crate) code_object_join: JoinUnavailableReasonV1,
    pub(crate) clock_join: JoinUnavailableReasonV1,
    pub(crate) att_capability: CapabilityFactV1,
    pub(crate) pc_sampling_capability: CapabilityFactV1,
    pub(crate) grants_collection_authority: bool,
    pub(crate) grants_dispatch_authority: bool,
    pub(crate) proves_universal_collector_inability: bool,
}

pub(crate) struct QualificationInputsV1<'a> {
    pub(crate) plan_sha256: [u8; 32],
    pub(crate) collector_executable: RawContentIdentityV1,
    pub(crate) collector_release: CollectorReleaseV1,
    pub(crate) collector_closure: RawContentIdentityV1,
    pub(crate) collector_configuration: RawContentIdentityV1,
    pub(crate) collector_argv: RawContentIdentityV1,
    pub(crate) collector_environment: RawContentIdentityV1,
    pub(crate) target_executable: RawContentIdentityV1,
    pub(crate) target_argv: RawContentIdentityV1,
    pub(crate) collector_stdout: RawContentIdentityV1,
    pub(crate) collector_stdout_overflow: bool,
    pub(crate) collector_stderr: RawContentIdentityV1,
    pub(crate) collector_stderr_overflow: bool,
    pub(crate) collector_artifacts: Vec<CollectorArtifactV1>,
    pub(crate) runtime_capture_bytes: &'a [u8],
}

pub(crate) fn build_live_qualification_v1(
    inputs: QualificationInputsV1<'_>,
) -> Result<DirectKfdRocprofQualificationV1, LiveQualificationErrorV1> {
    let runtime_capture = decode_kfd_runtime_profile_v1(inputs.runtime_capture_bytes)
        .map_err(|_| LiveQualificationErrorV1::InvalidRuntimeCapture)?;
    let runtime = summarize_runtime(inputs.runtime_capture_bytes, &runtime_capture)?;
    let outcome = if runtime.dispatches_published == 0 {
        LiveQualificationOutcomeV1::CaptureContainsNoDispatch
    } else if inputs.collector_artifacts.is_empty() {
        LiveQualificationOutcomeV1::DispatchObservedCollectorCompletedNoArtifacts
    } else {
        LiveQualificationOutcomeV1::DispatchObservedCollectorArtifactsPresentUnjoined
    };
    let record = DirectKfdRocprofQualificationV1 {
        schema: LIVE_QUALIFICATION_SCHEMA_V1.to_owned(),
        schema_version: 1,
        plan_sha256: inputs.plan_sha256,
        collector_executable: inputs.collector_executable,
        collector_release: inputs.collector_release,
        collector_closure: inputs.collector_closure,
        collector_configuration: inputs.collector_configuration,
        collector_argv: inputs.collector_argv,
        collector_environment: inputs.collector_environment,
        target_executable: inputs.target_executable,
        target_argv: inputs.target_argv,
        collector_exit_success: true,
        collector_stdout: inputs.collector_stdout,
        collector_stdout_overflow: inputs.collector_stdout_overflow,
        collector_stderr: inputs.collector_stderr,
        collector_stderr_overflow: inputs.collector_stderr_overflow,
        collector_inventory_complete: true,
        collector_artifacts: inputs.collector_artifacts,
        runtime,
        outcome,
        dispatch_join: JoinUnavailableReasonV1::MissingCommonDispatchIdentity,
        code_object_join: JoinUnavailableReasonV1::RuntimeCodeObjectUnrelated,
        clock_join: JoinUnavailableReasonV1::IncomparableClocks,
        att_capability: CapabilityFactV1::NotRequestedOrProbed,
        pc_sampling_capability: CapabilityFactV1::NotRequestedOrProbed,
        grants_collection_authority: false,
        grants_dispatch_authority: false,
        proves_universal_collector_inability: false,
    };
    record.validate()?;
    Ok(record)
}

fn summarize_runtime(
    bytes: &[u8],
    capture: &KfdRuntimeProfileV1,
) -> Result<RuntimeCaptureSummaryV1, LiveQualificationErrorV1> {
    let mut published = 0_u64;
    let mut completed = 0_u64;
    let mut released = 0_u64;
    for event in &capture.events {
        match event.event {
            KfdRuntimeProfileEventKindV1::DispatchPublished { .. } => published += 1,
            KfdRuntimeProfileEventKindV1::DispatchCompleted { .. } => completed += 1,
            KfdRuntimeProfileEventKindV1::SubmissionReleased { .. } => released += 1,
            _ => {}
        }
    }
    Ok(RuntimeCaptureSummaryV1 {
        content: RawContentIdentityV1::observed(bytes)?,
        target_profile: capture.device.target_profile.clone(),
        wave_width: capture.device.wave_width,
        observed_events: capture.coverage.observed_events,
        dropped_events: capture.coverage.dropped_events,
        complete_runtime_operation_history: capture.coverage.complete_runtime_operation_history,
        dispatches_published: published,
        dispatches_completed: completed,
        submissions_released: released,
    })
}

impl DirectKfdRocprofQualificationV1 {
    pub(crate) fn validate(&self) -> Result<(), LiveQualificationErrorV1> {
        if self.schema != LIVE_QUALIFICATION_SCHEMA_V1
            || self.schema_version != 1
            || self.plan_sha256 == [0; 32]
            || !self.collector_exit_success
            || !self.collector_inventory_complete
            || self.collector_stdout_overflow
            || self.collector_stderr_overflow
            || self.grants_collection_authority
            || self.grants_dispatch_authority
            || self.proves_universal_collector_inability
            || self.collector_artifacts.len() > 4096
            || self.runtime.target_profile.is_empty()
            || self.runtime.target_profile.len() > 64
            || self.runtime.wave_width == 0
            || self.runtime.observed_events == 0
            || self.runtime.dispatches_completed > self.runtime.dispatches_published
            || self.runtime.submissions_released > self.runtime.dispatches_completed
            || self.runtime.complete_runtime_operation_history != (self.runtime.dropped_events == 0)
        {
            return Err(LiveQualificationErrorV1::InvalidRecord);
        }
        for identity in [
            self.collector_executable,
            self.collector_closure,
            self.collector_configuration,
            self.collector_argv,
            self.collector_environment,
            self.target_executable,
            self.target_argv,
            self.runtime.content,
        ] {
            identity.validate_nonempty()?;
        }
        self.collector_stdout.validate()?;
        self.collector_stderr.validate()?;
        for artifact in &self.collector_artifacts {
            if artifact.relative_path.is_empty()
                || artifact.relative_path.len() > 4096
                || artifact.relative_path.starts_with('/')
                || artifact.relative_path.contains('\\')
                || artifact
                    .relative_path
                    .split('/')
                    .any(|component| component.is_empty() || component == "." || component == "..")
            {
                return Err(LiveQualificationErrorV1::InvalidRecord);
            }
            artifact.content.validate_nonempty()?;
        }
        let expected_outcome = if self.runtime.dispatches_published == 0 {
            LiveQualificationOutcomeV1::CaptureContainsNoDispatch
        } else if self.collector_artifacts.is_empty() {
            LiveQualificationOutcomeV1::DispatchObservedCollectorCompletedNoArtifacts
        } else {
            LiveQualificationOutcomeV1::DispatchObservedCollectorArtifactsPresentUnjoined
        };
        if self.outcome != expected_outcome
            || self.dispatch_join != JoinUnavailableReasonV1::MissingCommonDispatchIdentity
            || self.code_object_join != JoinUnavailableReasonV1::RuntimeCodeObjectUnrelated
            || self.clock_join != JoinUnavailableReasonV1::IncomparableClocks
            || self.att_capability != CapabilityFactV1::NotRequestedOrProbed
            || self.pc_sampling_capability != CapabilityFactV1::NotRequestedOrProbed
        {
            return Err(LiveQualificationErrorV1::InvalidRecord);
        }
        Ok(())
    }
}

pub(crate) fn encode_live_qualification_v1(
    record: &DirectKfdRocprofQualificationV1,
) -> Result<Vec<u8>, LiveQualificationErrorV1> {
    record.validate()?;
    let bytes = serde_json::to_vec(record).map_err(|_| LiveQualificationErrorV1::JsonEncode)?;
    if bytes.is_empty() || bytes.len() > MAX_LIVE_QUALIFICATION_BYTES_V1 {
        return Err(LiveQualificationErrorV1::SizeOverflow);
    }
    Ok(bytes)
}

pub(crate) fn decode_live_qualification_v1(
    bytes: &[u8],
) -> Result<DirectKfdRocprofQualificationV1, LiveQualificationErrorV1> {
    if bytes.is_empty() || bytes.len() > MAX_LIVE_QUALIFICATION_BYTES_V1 {
        return Err(LiveQualificationErrorV1::SizeOverflow);
    }
    let record: DirectKfdRocprofQualificationV1 =
        serde_json::from_slice(bytes).map_err(|_| LiveQualificationErrorV1::JsonDecode)?;
    record.validate()?;
    if serde_json::to_vec(&record).map_err(|_| LiveQualificationErrorV1::JsonEncode)? != bytes {
        return Err(LiveQualificationErrorV1::NonCanonicalEncoding);
    }
    Ok(record)
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum LiveQualificationErrorV1 {
    InvalidIdentity,
    InvalidRuntimeCapture,
    InvalidRecord,
    SizeOverflow,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
}

impl fmt::Display for LiveQualificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "direct-KFD rocprof qualification rejected: {self:?}"
        )
    }
}

impl Error for LiveQualificationErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileBindingV1, KfdProfileDeviceV1, KfdProfileHostContentModeV1,
        KfdProfileHostTimingV1, KfdProfileLaunchV1, KfdProfileResourceKindV1,
        ProfileContentIdentityV1, ProfileIdentityV1, encode_kfd_runtime_profile_v1,
        push_observed_event_v1, resource_identity_v1,
    };

    fn runtime_bytes(with_dispatch: bool) -> Vec<u8> {
        let scope = ProfileIdentityV1::new([1; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 5).unwrap();
        let mut events = Vec::new();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: ProfileContentIdentityV1::observed(b"artifact").unwrap(),
            },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name: ProfileContentIdentityV1::observed(b"kernel").unwrap(),
                signature: ProfileContentIdentityV1::observed(b"signature").unwrap(),
            },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        if with_dispatch {
            for event in [
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch,
                    queue,
                    stream,
                    kernel,
                    dispatch_shape: ProfileContentIdentityV1::observed(b"shape").unwrap(),
                    launch: KfdProfileLaunchV1 {
                        grid: [64, 1, 1],
                        workgroup: [64, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    bindings: Vec::<KfdProfileBindingV1>::new(),
                },
                KfdRuntimeProfileEventKindV1::DispatchCompleted {
                    dispatch,
                    host_timing: KfdProfileHostTimingV1::default(),
                },
                KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            ] {
                push_observed_event_v1(scope, &mut events, event).unwrap();
            }
        }
        for event in [
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        let capture = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            events,
            0,
        )
        .unwrap();
        encode_kfd_runtime_profile_v1(&capture).unwrap()
    }

    fn identity(seed: u8) -> RawContentIdentityV1 {
        RawContentIdentityV1::observed(&[seed]).unwrap()
    }

    fn inputs(runtime: &[u8]) -> QualificationInputsV1<'_> {
        QualificationInputsV1 {
            plan_sha256: [1; 32],
            collector_executable: identity(1),
            collector_release: CollectorReleaseV1::RocprofilerSdk1_1_0Git97f5574,
            collector_closure: identity(2),
            collector_configuration: identity(3),
            collector_argv: identity(4),
            collector_environment: identity(5),
            target_executable: identity(6),
            target_argv: identity(7),
            collector_stdout: RawContentIdentityV1::observed(b"").unwrap(),
            collector_stdout_overflow: false,
            collector_stderr: RawContentIdentityV1::observed(b"").unwrap(),
            collector_stderr_overflow: false,
            collector_artifacts: Vec::new(),
            runtime_capture_bytes: runtime,
        }
    }

    #[test]
    fn no_artifact_run_is_exactly_scoped_not_a_universal_claim() {
        let runtime = runtime_bytes(true);
        let record = build_live_qualification_v1(inputs(&runtime)).unwrap();
        assert_eq!(
            record.outcome,
            LiveQualificationOutcomeV1::DispatchObservedCollectorCompletedNoArtifacts
        );
        assert_eq!(record.runtime.dispatches_published, 1);
        assert!(record.runtime.complete_runtime_operation_history);
        assert!(!record.proves_universal_collector_inability);
        assert!(!record.grants_collection_authority);
        assert!(!record.grants_dispatch_authority);
        let bytes = encode_live_qualification_v1(&record).unwrap();
        assert_eq!(decode_live_qualification_v1(&bytes).unwrap(), record);
    }

    #[test]
    fn runtime_without_dispatch_is_not_upgraded_to_dispatch_evidence() {
        let runtime = runtime_bytes(false);
        let record = build_live_qualification_v1(inputs(&runtime)).unwrap();
        assert_eq!(
            record.outcome,
            LiveQualificationOutcomeV1::CaptureContainsNoDispatch
        );
    }

    #[test]
    fn stale_runtime_and_qualification_substitutions_are_rejected() {
        let runtime = runtime_bytes(true);
        let mut stale_runtime = runtime.clone();
        let position = stale_runtime.iter().position(|byte| *byte == b'1').unwrap();
        stale_runtime[position] = b'2';
        assert!(build_live_qualification_v1(inputs(&stale_runtime)).is_err());

        let record = build_live_qualification_v1(inputs(&runtime)).unwrap();
        let bytes = encode_live_qualification_v1(&record).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["plan_sha256"] = serde_json::to_value([0_u8; 32]).unwrap();
        assert!(decode_live_qualification_v1(&serde_json::to_vec(&value).unwrap()).is_err());

        let mut noncanonical = bytes;
        noncanonical.push(b'\n');
        assert_eq!(
            decode_live_qualification_v1(&noncanonical),
            Err(LiveQualificationErrorV1::NonCanonicalEncoding)
        );
    }

    #[test]
    fn artifact_inventory_cannot_be_removed_without_recomputing_outcome() {
        let runtime = runtime_bytes(true);
        let mut qualified_inputs = inputs(&runtime);
        qualified_inputs.collector_artifacts = vec![CollectorArtifactV1 {
            relative_path: "capture.json".to_owned(),
            content: identity(9),
        }];
        let record = build_live_qualification_v1(qualified_inputs).unwrap();
        assert_eq!(
            record.outcome,
            LiveQualificationOutcomeV1::DispatchObservedCollectorArtifactsPresentUnjoined
        );
        let bytes = encode_live_qualification_v1(&record).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["collector_artifacts"] = serde_json::json!([]);
        assert!(decode_live_qualification_v1(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn artifact_paths_are_component_validated() {
        let runtime = runtime_bytes(true);
        for path in [
            "../capture.json",
            "nested/../capture.json",
            "nested//capture.json",
        ] {
            let mut qualified_inputs = inputs(&runtime);
            qualified_inputs.collector_artifacts = vec![CollectorArtifactV1 {
                relative_path: path.to_owned(),
                content: identity(9),
            }];
            assert!(build_live_qualification_v1(qualified_inputs).is_err());
        }

        let mut qualified_inputs = inputs(&runtime);
        qualified_inputs.collector_artifacts = vec![CollectorArtifactV1 {
            relative_path: "capture..json".to_owned(),
            content: identity(9),
        }];
        assert!(build_live_qualification_v1(qualified_inputs).is_ok());
    }

    #[test]
    fn checked_in_mi300x_evidence_is_canonical_and_content_bound() {
        let qualification = include_bytes!(
            "../../../docs/evidence/mi300x-direct-kfd-rocprof-qualification-v1.json"
        );
        let runtime =
            include_bytes!("../../../docs/evidence/mi300x-direct-kfd-runtime-profile-v1.json");
        let record = decode_live_qualification_v1(qualification).unwrap();
        assert_eq!(
            record.outcome,
            LiveQualificationOutcomeV1::DispatchObservedCollectorCompletedNoArtifacts
        );
        assert_eq!(
            record.runtime.content,
            RawContentIdentityV1::observed(runtime).unwrap()
        );
        assert_eq!(record.runtime.dispatches_published, 3);
        assert_eq!(record.runtime.dispatches_completed, 3);
        assert_eq!(record.runtime.submissions_released, 3);
        assert_eq!(record.runtime.dropped_events, 0);
        assert!(record.runtime.complete_runtime_operation_history);
        assert!(record.collector_artifacts.is_empty());
        assert!(!record.proves_universal_collector_inability);
        assert!(!record.grants_collection_authority);
        assert!(!record.grants_dispatch_authority);
    }
}

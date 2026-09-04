//! Read-only reporting over direct-KFD host timestamp observations.
//!
//! This path accepts only the runtime-owned, non-constructible custody bundle.
//! It reports exact host observation points without converting them into GPU
//! begin/end timestamps or a globally synchronized time domain.

use std::error::Error;
use std::fmt;

use fe2o3_profiler_protocol::{
    KfdRuntimeProfileV1, NativeRuntimeDispatchTimestampCaptureV1,
    NativeRuntimeDispatchTimestampCoverageV1, NativeRuntimeHostClockV1,
    NativeRuntimeHostTimestampPointV1, ProfileContentIdentityV1, ProfileIdentityV1,
    encode_kfd_runtime_profile_v1, encode_native_runtime_dispatch_timestamp_capture_v1,
    native_runtime_dispatch_timestamp_capture_content_identity_v1,
};
use fe2o3_runtime::AuthenticatedKfdRuntimeDispatchTimestampsV1;
use fe2o3_semantic_import::TruthOriginV1;
use serde::Serialize;

pub const NATIVE_RUNTIME_DISPATCH_TIMESTAMP_REPORT_SCHEMA_V1: &str =
    "fe2o3-native-runtime-dispatch-timestamp-report-v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRuntimeDispatchTimestampCapabilityNameV1 {
    AuthenticatedHostPublicationObservations,
    AuthenticatedHostCompletionObservations,
    CompleteHostObservationIntervals,
    GpuDispatchBegin,
    GpuDispatchEnd,
    DeviceClockDomain,
    GloballySynchronizedTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRuntimeDispatchTimestampUnavailableReasonV1 {
    NoObservedDispatches,
    CapturePartialOrLost,
    NativeRuntimeDoesNotObserveGpuDispatchBegin,
    NativeRuntimeDoesNotObserveGpuDispatchEnd,
    NoNativeDeviceClockObservation,
    HostClockIsProcessLocalAndUnsynchronized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum NativeRuntimeDispatchTimestampCapabilityAvailabilityV1 {
    Available {
        origin: TruthOriginV1,
    },
    Unavailable {
        origin: TruthOriginV1,
        reason: NativeRuntimeDispatchTimestampUnavailableReasonV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeDispatchTimestampCapabilityV1 {
    pub name: NativeRuntimeDispatchTimestampCapabilityNameV1,
    pub availability: NativeRuntimeDispatchTimestampCapabilityAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeDispatchTimestampRecordSummaryV1 {
    pub record_identity: ProfileIdentityV1,
    pub dispatch: ProfileIdentityV1,
    pub queue: ProfileIdentityV1,
    pub device: ProfileIdentityV1,
    pub kernel: ProfileIdentityV1,
    pub artifact: ProfileContentIdentityV1,
    pub publication: NativeRuntimeHostTimestampPointV1,
    pub completion: Option<NativeRuntimeHostTimestampPointV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRuntimeDispatchTimestampReportAuthorityV1 {
    AuthenticatedNativeRuntimeCustodyReadOnlyNoCollectionDispatchOrExecutionAuthority,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeDispatchTimestampReportV1 {
    pub schema: &'static str,
    pub runtime_profile: ProfileContentIdentityV1,
    pub timestamp_capture: ProfileContentIdentityV1,
    pub runtime_capture_scope: ProfileIdentityV1,
    pub recorder_occurrence: ProfileIdentityV1,
    pub host_clock: NativeRuntimeHostClockV1,
    pub coverage: NativeRuntimeDispatchTimestampCoverageV1,
    pub capabilities: Vec<NativeRuntimeDispatchTimestampCapabilityV1>,
    pub records: Vec<NativeRuntimeDispatchTimestampRecordSummaryV1>,
    pub authority: NativeRuntimeDispatchTimestampReportAuthorityV1,
}

pub fn report_authenticated_native_runtime_dispatch_timestamps_v1(
    evidence: &AuthenticatedKfdRuntimeDispatchTimestampsV1,
) -> Result<NativeRuntimeDispatchTimestampReportV1, NativeRuntimeDispatchTimestampReportErrorV1> {
    report_validated_native_runtime_dispatch_timestamps_v1(
        evidence.runtime_profile(),
        evidence.dispatch_timestamps(),
    )
}

fn report_validated_native_runtime_dispatch_timestamps_v1(
    runtime_profile: &KfdRuntimeProfileV1,
    capture: &NativeRuntimeDispatchTimestampCaptureV1,
) -> Result<NativeRuntimeDispatchTimestampReportV1, NativeRuntimeDispatchTimestampReportErrorV1> {
    let runtime_bytes = encode_kfd_runtime_profile_v1(runtime_profile)
        .map_err(|_| NativeRuntimeDispatchTimestampReportErrorV1::RuntimeProfileRejected)?;
    let timestamp_bytes =
        encode_native_runtime_dispatch_timestamp_capture_v1(capture, &runtime_bytes)
            .map_err(|_| NativeRuntimeDispatchTimestampReportErrorV1::TimestampCaptureRejected)?;
    let timestamp_capture = native_runtime_dispatch_timestamp_capture_content_identity_v1(
        &timestamp_bytes,
        &runtime_bytes,
    )
    .map_err(|_| NativeRuntimeDispatchTimestampReportErrorV1::TimestampCaptureRejected)?;

    let mut records = Vec::new();
    records
        .try_reserve_exact(capture.records().len())
        .map_err(|_| NativeRuntimeDispatchTimestampReportErrorV1::AllocationFailure)?;
    records.extend(capture.records().iter().map(|record| {
        NativeRuntimeDispatchTimestampRecordSummaryV1 {
            record_identity: record.identity(),
            dispatch: record.dispatch(),
            queue: record.queue(),
            device: record.device(),
            kernel: record.kernel(),
            artifact: record.artifact(),
            publication: record.publication(),
            completion: record.completion(),
        }
    }));

    Ok(NativeRuntimeDispatchTimestampReportV1 {
        schema: NATIVE_RUNTIME_DISPATCH_TIMESTAMP_REPORT_SCHEMA_V1,
        runtime_profile: capture.runtime_profile(),
        timestamp_capture,
        runtime_capture_scope: capture.runtime_capture_scope(),
        recorder_occurrence: capture.recorder_occurrence(),
        host_clock: capture.host_clock(),
        coverage: capture.coverage(),
        capabilities: capabilities(capture.coverage()),
        records,
        authority: NativeRuntimeDispatchTimestampReportAuthorityV1::AuthenticatedNativeRuntimeCustodyReadOnlyNoCollectionDispatchOrExecutionAuthority,
    })
}

fn capabilities(
    coverage: NativeRuntimeDispatchTimestampCoverageV1,
) -> Vec<NativeRuntimeDispatchTimestampCapabilityV1> {
    let positive_reason = NativeRuntimeDispatchTimestampUnavailableReasonV1::NoObservedDispatches;
    let interval_reason = if coverage.runtime_profile_dispatches == 0 {
        NativeRuntimeDispatchTimestampUnavailableReasonV1::NoObservedDispatches
    } else {
        NativeRuntimeDispatchTimestampUnavailableReasonV1::CapturePartialOrLost
    };
    vec![
        capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::AuthenticatedHostPublicationObservations,
            coverage.observed_publications != 0,
            positive_reason,
        ),
        capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::AuthenticatedHostCompletionObservations,
            coverage.observed_completions != 0,
            positive_reason,
        ),
        capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::CompleteHostObservationIntervals,
            coverage.runtime_profile_dispatches != 0
                && coverage.complete_runtime_operation_history,
            interval_reason,
        ),
        unavailable_capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::GpuDispatchBegin,
            NativeRuntimeDispatchTimestampUnavailableReasonV1::NativeRuntimeDoesNotObserveGpuDispatchBegin,
        ),
        unavailable_capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::GpuDispatchEnd,
            NativeRuntimeDispatchTimestampUnavailableReasonV1::NativeRuntimeDoesNotObserveGpuDispatchEnd,
        ),
        unavailable_capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::DeviceClockDomain,
            NativeRuntimeDispatchTimestampUnavailableReasonV1::NoNativeDeviceClockObservation,
        ),
        unavailable_capability(
            NativeRuntimeDispatchTimestampCapabilityNameV1::GloballySynchronizedTime,
            NativeRuntimeDispatchTimestampUnavailableReasonV1::HostClockIsProcessLocalAndUnsynchronized,
        ),
    ]
}

const fn capability(
    name: NativeRuntimeDispatchTimestampCapabilityNameV1,
    available: bool,
    unavailable_reason: NativeRuntimeDispatchTimestampUnavailableReasonV1,
) -> NativeRuntimeDispatchTimestampCapabilityV1 {
    let availability = if available {
        NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Available {
            origin: TruthOriginV1::Observed,
        }
    } else {
        NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Unavailable {
            origin: TruthOriginV1::Unavailable,
            reason: unavailable_reason,
        }
    };
    NativeRuntimeDispatchTimestampCapabilityV1 { name, availability }
}

const fn unavailable_capability(
    name: NativeRuntimeDispatchTimestampCapabilityNameV1,
    reason: NativeRuntimeDispatchTimestampUnavailableReasonV1,
) -> NativeRuntimeDispatchTimestampCapabilityV1 {
    NativeRuntimeDispatchTimestampCapabilityV1 {
        name,
        availability: NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Unavailable {
            origin: TruthOriginV1::Unavailable,
            reason,
        },
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeRuntimeDispatchTimestampReportErrorV1 {
    RuntimeProfileRejected,
    TimestampCaptureRejected,
    AllocationFailure,
}

impl fmt::Display for NativeRuntimeDispatchTimestampReportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native runtime dispatch timestamp report rejected: {self:?}"
        )
    }
}

impl Error for NativeRuntimeDispatchTimestampReportErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileDeviceV1, KfdProfileHostContentModeV1, KfdProfileHostTimingV1,
        KfdProfileLaunchV1, KfdProfileResourceKindV1, KfdRuntimeProfileEventKindV1,
        NativeRuntimeDispatchTimestampRecorderV1, push_observed_event_v1, resource_identity_v1,
    };

    fn availability(
        capabilities: &[NativeRuntimeDispatchTimestampCapabilityV1],
        name: NativeRuntimeDispatchTimestampCapabilityNameV1,
    ) -> NativeRuntimeDispatchTimestampCapabilityAvailabilityV1 {
        capabilities
            .iter()
            .find(|capability| capability.name == name)
            .unwrap()
            .availability
    }

    fn one_dispatch_capture() -> (
        KfdRuntimeProfileV1,
        fe2o3_profiler_protocol::NativeRuntimeDispatchTimestampRecorderOutputV1,
    ) {
        let scope = ProfileIdentityV1::new([51; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 5).unwrap();
        let mut timestamps = NativeRuntimeDispatchTimestampRecorderV1::new(scope, 16).unwrap();
        let mut events = Vec::new();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: ProfileContentIdentityV1::observed(b"object").unwrap(),
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
                bindings: Vec::new(),
            },
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: KfdProfileHostTimingV1::default(),
            },
        ] {
            let sample = timestamps.sample(&event).unwrap();
            push_observed_event_v1(scope, &mut events, event).unwrap();
            timestamps.commit(sample, events.last().unwrap());
        }
        for event in [
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        let runtime = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            events,
            0,
        )
        .unwrap();
        let timestamps = timestamps.finish(&runtime).unwrap();
        (runtime, timestamps)
    }

    #[test]
    fn complete_host_capture_never_claims_gpu_or_global_time() {
        let capabilities = capabilities(NativeRuntimeDispatchTimestampCoverageV1 {
            runtime_profile_dispatches: 1,
            observed_publications: 1,
            observed_completions: 1,
            dropped_observations: 0,
            complete_runtime_operation_history: true,
        });
        assert_eq!(
            availability(
                &capabilities,
                NativeRuntimeDispatchTimestampCapabilityNameV1::CompleteHostObservationIntervals,
            ),
            NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Available {
                origin: TruthOriginV1::Observed,
            }
        );
        for name in [
            NativeRuntimeDispatchTimestampCapabilityNameV1::GpuDispatchBegin,
            NativeRuntimeDispatchTimestampCapabilityNameV1::GpuDispatchEnd,
            NativeRuntimeDispatchTimestampCapabilityNameV1::DeviceClockDomain,
            NativeRuntimeDispatchTimestampCapabilityNameV1::GloballySynchronizedTime,
        ] {
            assert!(matches!(
                availability(&capabilities, name),
                NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Unavailable {
                    origin: TruthOriginV1::Unavailable,
                    ..
                }
            ));
        }
    }

    #[test]
    fn partial_capture_does_not_advertise_complete_intervals() {
        let capabilities = capabilities(NativeRuntimeDispatchTimestampCoverageV1 {
            runtime_profile_dispatches: 1,
            observed_publications: 1,
            observed_completions: 0,
            dropped_observations: 1,
            complete_runtime_operation_history: false,
        });
        assert_eq!(
            availability(
                &capabilities,
                NativeRuntimeDispatchTimestampCapabilityNameV1::CompleteHostObservationIntervals,
            ),
            NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Unavailable {
                origin: TruthOriginV1::Unavailable,
                reason: NativeRuntimeDispatchTimestampUnavailableReasonV1::CapturePartialOrLost,
            }
        );
    }

    #[test]
    fn empty_lossless_capture_reports_no_observed_dispatches() {
        let scope = ProfileIdentityV1::new([52; 32]).unwrap();
        let runtime = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            Vec::new(),
            0,
        )
        .unwrap();
        let timestamps = NativeRuntimeDispatchTimestampRecorderV1::new(scope, 16)
            .unwrap()
            .finish(&runtime)
            .unwrap();
        let report =
            report_validated_native_runtime_dispatch_timestamps_v1(&runtime, timestamps.capture())
                .unwrap();
        assert!(report.records.is_empty());
        for name in [
            NativeRuntimeDispatchTimestampCapabilityNameV1::AuthenticatedHostPublicationObservations,
            NativeRuntimeDispatchTimestampCapabilityNameV1::AuthenticatedHostCompletionObservations,
            NativeRuntimeDispatchTimestampCapabilityNameV1::CompleteHostObservationIntervals,
        ] {
            assert_eq!(
                availability(&report.capabilities, name),
                NativeRuntimeDispatchTimestampCapabilityAvailabilityV1::Unavailable {
                    origin: TruthOriginV1::Unavailable,
                    reason: NativeRuntimeDispatchTimestampUnavailableReasonV1::NoObservedDispatches,
                }
            );
        }
    }

    #[test]
    fn validated_consumer_preserves_exact_host_points_without_inference() {
        let (runtime, timestamps) = one_dispatch_capture();
        let report =
            report_validated_native_runtime_dispatch_timestamps_v1(&runtime, timestamps.capture())
                .unwrap();
        assert_eq!(report.records.len(), 1);
        assert_eq!(
            report.records[0].publication.runtime_event,
            runtime.events[4].identity
        );
        assert_eq!(
            report.records[0].completion.unwrap().runtime_event,
            runtime.events[5].identity
        );
        assert_eq!(
            report.runtime_profile,
            timestamps.capture().runtime_profile()
        );
        assert_eq!(report.timestamp_capture.byte_len as usize, {
            let runtime_bytes = encode_kfd_runtime_profile_v1(&runtime).unwrap();
            encode_native_runtime_dispatch_timestamp_capture_v1(
                timestamps.capture(),
                &runtime_bytes,
            )
            .unwrap()
            .len()
        });
    }
}

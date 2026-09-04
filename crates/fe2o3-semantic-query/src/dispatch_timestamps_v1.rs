//! Capture-scoped reporting for structurally admitted dispatch timestamp claims.
//!
//! No trusted direct-KFD or collector adapter exists in this tranche. A
//! complete input therefore exposes bounded producer-claimed raw ticks for
//! inspection, but does not advertise authenticated timestamp capability and
//! is not projected into Semantic Trace observed events.

use std::error::Error;
use std::fmt;

use fe2o3_profiler_protocol::{
    DispatchClockCorrelationBracketV1, DispatchClockDomainsV1, DispatchTimestampCaptureErrorV1,
    DispatchTimestampClaimOriginV1, DispatchTimestampCompletenessV1, DispatchTimestampCoverageV1,
    DispatchTimestampPointV1, DispatchTimestampProducerKindV1,
    DispatchTimestampProvenanceAdmissionV1, DispatchTimestampRecordV1, ProfileContentIdentityV1,
    ProfileIdentityV1, decode_dispatch_timestamp_capture_with_content_identity_v1,
    decode_kfd_runtime_profile_with_content_identity_v1,
};
use fe2o3_semantic_import::TruthOriginV1;
use serde::Serialize;

pub const DISPATCH_TIMESTAMP_REPORT_SCHEMA_V1: &str = "fe2o3-dispatch-timestamp-report-v1";

#[derive(Clone, Copy, Debug)]
pub struct DispatchTimestampEvidenceInputV1<'a> {
    pub capture_bytes: &'a [u8],
    pub producer_evidence_bytes: &'a [u8],
    pub collection_configuration_bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampCapabilityNameV1 {
    StructurallyAdmittedProducerClaims,
    AuthenticatedPerDispatchDeviceTimestamps,
    DevicePublicationTimestamp,
    ClockFrequencyAndNanosecondNormalization,
    GloballySynchronizedTime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampUnavailableReasonV1 {
    NoTimestampCaptureSupplied,
    CapturePartialLostOrTruncated,
    NoAuthenticatedProducerAdapter,
    CpuPublicationOnly,
    OpaqueTicksWithoutFrequency,
    NoGlobalClockSynchronization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DispatchTimestampCapabilityAvailabilityV1 {
    Available {
        origin: TruthOriginV1,
    },
    Unavailable {
        origin: TruthOriginV1,
        reason: DispatchTimestampUnavailableReasonV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampCapabilityV1 {
    pub name: DispatchTimestampCapabilityNameV1,
    pub availability: DispatchTimestampCapabilityAvailabilityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum DispatchTimestampStructuralAdmissionV1 {
    NotSupplied,
    StructurallyAdmittedProducerClaim {
        producer: DispatchTimestampProducerKindV1,
        producer_claimed_origin: DispatchTimestampClaimOriginV1,
        provenance: DispatchTimestampProvenanceAdmissionV1,
        completeness: DispatchTimestampCompletenessV1,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampRecordSummaryV1 {
    pub record_identity: ProfileIdentityV1,
    pub dispatch: ProfileIdentityV1,
    pub queue: ProfileIdentityV1,
    pub device: ProfileIdentityV1,
    pub kernel: ProfileIdentityV1,
    pub artifact: ProfileContentIdentityV1,
    pub publication_sequence: u64,
    pub publication: DispatchTimestampPointV1,
    pub device_start: DispatchTimestampPointV1,
    pub device_end: DispatchTimestampPointV1,
    pub correlation_before: DispatchClockCorrelationBracketV1,
    pub correlation_after: DispatchClockCorrelationBracketV1,
}

impl From<&DispatchTimestampRecordV1> for DispatchTimestampRecordSummaryV1 {
    fn from(value: &DispatchTimestampRecordV1) -> Self {
        Self {
            record_identity: value.identity(),
            dispatch: value.dispatch(),
            queue: value.queue(),
            device: value.device(),
            kernel: value.kernel(),
            artifact: value.artifact(),
            publication_sequence: value.publication_sequence(),
            publication: value.publication(),
            device_start: value.device_start(),
            device_end: value.device_end(),
            correlation_before: value.correlation_before(),
            correlation_after: value.correlation_after(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampReportV1 {
    pub schema: &'static str,
    pub runtime_profile: ProfileContentIdentityV1,
    pub timestamp_capture: Option<ProfileContentIdentityV1>,
    pub structural_admission: DispatchTimestampStructuralAdmissionV1,
    pub producer_evidence: Option<ProfileContentIdentityV1>,
    pub collection_configuration: Option<ProfileContentIdentityV1>,
    pub clock_domains: Option<DispatchClockDomainsV1>,
    pub coverage: Option<DispatchTimestampCoverageV1>,
    pub capabilities: Vec<DispatchTimestampCapabilityV1>,
    pub records: Vec<DispatchTimestampRecordSummaryV1>,
    pub authority: DispatchTimestampReportAuthorityV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampReportAuthorityV1 {
    ReadOnlyNoCollectionDispatchClockConversionOrExecutionAuthority,
}

pub fn report_dispatch_timestamps_v1(
    runtime_profile_bytes: &[u8],
    evidence: Option<DispatchTimestampEvidenceInputV1<'_>>,
) -> Result<DispatchTimestampReportV1, DispatchTimestampReportErrorV1> {
    let authority =
        DispatchTimestampReportAuthorityV1::ReadOnlyNoCollectionDispatchClockConversionOrExecutionAuthority;
    let Some(evidence) = evidence else {
        let (_, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(runtime_profile_bytes)
                .map_err(|_| DispatchTimestampReportErrorV1::RuntimeProfileRejected)?;
        return Ok(DispatchTimestampReportV1 {
            schema: DISPATCH_TIMESTAMP_REPORT_SCHEMA_V1,
            runtime_profile,
            timestamp_capture: None,
            structural_admission: DispatchTimestampStructuralAdmissionV1::NotSupplied,
            producer_evidence: None,
            collection_configuration: None,
            clock_domains: None,
            coverage: None,
            capabilities: unavailable_capabilities(
                DispatchTimestampUnavailableReasonV1::NoTimestampCaptureSupplied,
            ),
            records: Vec::new(),
            authority,
        });
    };
    let (capture, timestamp_capture) = decode_dispatch_timestamp_capture_with_content_identity_v1(
        evidence.capture_bytes,
        runtime_profile_bytes,
        evidence.producer_evidence_bytes,
        evidence.collection_configuration_bytes,
    )
    .map_err(|error| match error {
        DispatchTimestampCaptureErrorV1::RuntimeProfileRejected => {
            DispatchTimestampReportErrorV1::RuntimeProfileRejected
        }
        _ => DispatchTimestampReportErrorV1::TimestampCaptureRejected,
    })?;
    let runtime_profile = capture.runtime_profile();
    let capabilities =
        admitted_capture_capabilities(capture.has_complete_producer_claimed_device_timestamps());
    let records = capture.records().iter().map(Into::into).collect();
    Ok(DispatchTimestampReportV1 {
        schema: DISPATCH_TIMESTAMP_REPORT_SCHEMA_V1,
        runtime_profile,
        timestamp_capture: Some(timestamp_capture),
        structural_admission:
            DispatchTimestampStructuralAdmissionV1::StructurallyAdmittedProducerClaim {
                producer: capture.producer(),
                producer_claimed_origin:
                    DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                provenance: capture.provenance_admission(),
                completeness: capture.coverage().completeness,
            },
        producer_evidence: Some(capture.producer_evidence()),
        collection_configuration: Some(capture.collection_configuration()),
        clock_domains: Some(capture.clock_domains()),
        coverage: Some(capture.coverage()),
        capabilities,
        records,
        authority,
    })
}

fn admitted_capture_capabilities(complete: bool) -> Vec<DispatchTimestampCapabilityV1> {
    let timestamp_reason = if complete {
        DispatchTimestampUnavailableReasonV1::NoAuthenticatedProducerAdapter
    } else {
        DispatchTimestampUnavailableReasonV1::CapturePartialLostOrTruncated
    };
    let mut capabilities = vec![DispatchTimestampCapabilityV1 {
        name: DispatchTimestampCapabilityNameV1::StructurallyAdmittedProducerClaims,
        availability: DispatchTimestampCapabilityAvailabilityV1::Available {
            // This is availability of the checked producer declaration, not
            // authentication of the claimed hardware observation.
            origin: TruthOriginV1::Declared,
        },
    }];
    capabilities.extend(unavailable_timestamp_capabilities(timestamp_reason));
    capabilities
}

fn unavailable_capabilities(
    timestamp_reason: DispatchTimestampUnavailableReasonV1,
) -> Vec<DispatchTimestampCapabilityV1> {
    [
        DispatchTimestampCapabilityNameV1::StructurallyAdmittedProducerClaims,
        DispatchTimestampCapabilityNameV1::AuthenticatedPerDispatchDeviceTimestamps,
        DispatchTimestampCapabilityNameV1::DevicePublicationTimestamp,
        DispatchTimestampCapabilityNameV1::ClockFrequencyAndNanosecondNormalization,
        DispatchTimestampCapabilityNameV1::GloballySynchronizedTime,
    ]
    .into_iter()
    .map(|name| DispatchTimestampCapabilityV1 {
        name,
        availability: unavailable(timestamp_reason),
    })
    .collect()
}

fn unavailable_timestamp_capabilities(
    timestamp_reason: DispatchTimestampUnavailableReasonV1,
) -> Vec<DispatchTimestampCapabilityV1> {
    vec![
        DispatchTimestampCapabilityV1 {
            name: DispatchTimestampCapabilityNameV1::AuthenticatedPerDispatchDeviceTimestamps,
            availability: unavailable(timestamp_reason),
        },
        DispatchTimestampCapabilityV1 {
            name: DispatchTimestampCapabilityNameV1::DevicePublicationTimestamp,
            availability: unavailable(DispatchTimestampUnavailableReasonV1::CpuPublicationOnly),
        },
        DispatchTimestampCapabilityV1 {
            name: DispatchTimestampCapabilityNameV1::ClockFrequencyAndNanosecondNormalization,
            availability: unavailable(
                DispatchTimestampUnavailableReasonV1::OpaqueTicksWithoutFrequency,
            ),
        },
        DispatchTimestampCapabilityV1 {
            name: DispatchTimestampCapabilityNameV1::GloballySynchronizedTime,
            availability: unavailable(
                DispatchTimestampUnavailableReasonV1::NoGlobalClockSynchronization,
            ),
        },
    ]
}

const fn unavailable(
    reason: DispatchTimestampUnavailableReasonV1,
) -> DispatchTimestampCapabilityAvailabilityV1 {
    DispatchTimestampCapabilityAvailabilityV1::Unavailable {
        origin: TruthOriginV1::Unavailable,
        reason,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchTimestampReportErrorV1 {
    RuntimeProfileRejected,
    TimestampCaptureRejected,
}

impl fmt::Display for DispatchTimestampReportErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "dispatch timestamp report rejected: {self:?}")
    }
}

impl Error for DispatchTimestampReportErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_profiler_protocol::{
        KfdProfileDeviceV1, KfdProfileHostContentModeV1, KfdProfileResourceKindV1,
        KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1, encode_kfd_runtime_profile_v1,
        push_observed_event_v1, resource_identity_v1,
    };

    fn empty_runtime() -> Vec<u8> {
        let scope = ProfileIdentityV1::new([8; 32]).unwrap();
        let device = KfdProfileDeviceV1::observed(8, "gfx942:xnack-", 64).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let mut events = Vec::new();
        push_observed_event_v1(
            scope,
            &mut events,
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
        )
        .unwrap();
        push_observed_event_v1(
            scope,
            &mut events,
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        )
        .unwrap();
        let profile = KfdRuntimeProfileV1::new(
            scope,
            device,
            KfdProfileHostContentModeV1::RangeOnly,
            events,
            0,
        )
        .unwrap();
        encode_kfd_runtime_profile_v1(&profile).unwrap()
    }

    #[test]
    fn no_capture_keeps_every_timestamp_capability_unavailable() {
        let report = report_dispatch_timestamps_v1(&empty_runtime(), None).unwrap();
        assert_eq!(
            report.structural_admission,
            DispatchTimestampStructuralAdmissionV1::NotSupplied
        );
        assert!(report.records.is_empty());
        assert!(report.capabilities.iter().all(|capability| matches!(
            capability.availability,
            DispatchTimestampCapabilityAvailabilityV1::Unavailable {
                origin: TruthOriginV1::Unavailable,
                reason: DispatchTimestampUnavailableReasonV1::NoTimestampCaptureSupplied,
            }
        )));
    }

    #[test]
    fn malformed_claim_is_rejected_instead_of_reported_unavailable() {
        let runtime = empty_runtime();
        assert_eq!(
            report_dispatch_timestamps_v1(
                &runtime,
                Some(DispatchTimestampEvidenceInputV1 {
                    capture_bytes: b"{}",
                    producer_evidence_bytes: b"receipt",
                    collection_configuration_bytes: b"configuration",
                }),
            ),
            Err(DispatchTimestampReportErrorV1::TimestampCaptureRejected)
        );
    }

    #[test]
    fn complete_structural_claim_does_not_advertise_authenticated_timestamps() {
        let capabilities = admitted_capture_capabilities(true);
        assert_eq!(
            capabilities[0].availability,
            DispatchTimestampCapabilityAvailabilityV1::Available {
                origin: TruthOriginV1::Declared,
            }
        );
        assert_eq!(
            capabilities[1],
            DispatchTimestampCapabilityV1 {
                name: DispatchTimestampCapabilityNameV1::AuthenticatedPerDispatchDeviceTimestamps,
                availability: DispatchTimestampCapabilityAvailabilityV1::Unavailable {
                    origin: TruthOriginV1::Unavailable,
                    reason: DispatchTimestampUnavailableReasonV1::NoAuthenticatedProducerAdapter,
                },
            }
        );
    }

    #[test]
    fn structural_report_uses_only_explicit_producer_declared_origin() {
        let admission = DispatchTimestampStructuralAdmissionV1::StructurallyAdmittedProducerClaim {
            producer: DispatchTimestampProducerKindV1::DirectKfdDeviceTimestamp,
            producer_claimed_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
            provenance:
                DispatchTimestampProvenanceAdmissionV1::StructurallyAdmittedProducerClaimOnly,
            completeness: DispatchTimestampCompletenessV1::CompleteAllRuntimeProfileDispatches,
        };
        assert!(matches!(
            admission,
            DispatchTimestampStructuralAdmissionV1::StructurallyAdmittedProducerClaim {
                producer_claimed_origin:
                    DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                ..
            }
        ));
    }

    #[test]
    fn partial_structural_claim_discloses_loss_in_capability() {
        let capabilities = admitted_capture_capabilities(false);
        assert_eq!(
            capabilities[1].availability,
            DispatchTimestampCapabilityAvailabilityV1::Unavailable {
                origin: TruthOriginV1::Unavailable,
                reason: DispatchTimestampUnavailableReasonV1::CapturePartialLostOrTruncated,
            }
        );
    }
}

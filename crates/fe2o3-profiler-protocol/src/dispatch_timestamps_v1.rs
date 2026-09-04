//! Exact, bounded correlation records for producer-observed dispatch clocks.
//!
//! This schema is separate from the frozen KFD Runtime Profile V1 wire. It is
//! descriptive and grants no runtime or execution authority. In particular, a
//! CPU publication tick is not a device publication timestamp, correlation
//! brackets are not a clock conversion, and raw ticks are not nanoseconds.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1, ProfileContentIdentityV1, ProfileIdentityV1,
    decode_kfd_runtime_profile_with_content_identity_v1,
};

pub const DISPATCH_TIMESTAMP_CAPTURE_SCHEMA_VERSION_V1: u16 = 1;
pub const DISPATCH_TIMESTAMP_CAPTURE_SCHEMA_V1: &str = "fe2o3-dispatch-timestamp-capture-v1";
pub const MAX_DISPATCH_TIMESTAMP_CAPTURE_BYTES_V1: u64 = 16 * 1024 * 1024;
pub const MAX_DISPATCH_TIMESTAMP_RECORDS_V1: usize = 16_384;
pub const MAX_DISPATCH_TIMESTAMP_PRODUCER_EVIDENCE_BYTES_V1: u64 = 16 * 1024 * 1024;
pub const MAX_DISPATCH_TIMESTAMP_CONFIGURATION_BYTES_V1: u64 = 64 * 1024;

const CLOCK_DOMAIN_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.dispatch-timestamp.clock-domain.v1\0";
const RECORD_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.dispatch-timestamp.record.v1\0";
const CAPTURE_CONTENT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.dispatch-timestamp.capture.v1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampProducerKindV1 {
    /// A future direct-KFD producer that observes real device dispatch ticks.
    DirectKfdDeviceTimestamp,
    /// A ROCprofiler SDK producer joined to the exact runtime dispatch.
    RocprofilerSdkDispatchCorrelation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampProvenanceAdmissionV1 {
    /// Structure, external byte identities, and runtime joins were checked,
    /// but no trusted collector adapter authenticated the producer claim.
    StructurallyAdmittedProducerClaimOnly,
}

/// Truth status of timestamp values in this structurally admitted wire.
///
/// This is intentionally distinct from [`crate::ProfileTruthOriginV1`]. Until a trusted collector
/// adapter exists, decoding this wire establishes only that a producer declared an observation;
/// it does not turn caller-supplied ticks into an authenticated observation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampClaimOriginV1 {
    ProducerDeclaredObservation,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchClockDomainKindV1 {
    ProducerCpuMonotonicCounter,
    ProducerDeviceCounter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchClockDomainV1 {
    pub identity: ProfileIdentityV1,
    pub kind: DispatchClockDomainKindV1,
    /// The schema intentionally carries opaque ticks without a frequency.
    pub unit: DispatchClockTickUnitV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchClockTickUnitV1 {
    OpaqueProducerTicks,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchClockDomainsV1 {
    pub cpu: DispatchClockDomainV1,
    pub device: DispatchClockDomainV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampStageV1 {
    Publish,
    DeviceStart,
    DeviceEnd,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampPointV1 {
    pub claim_origin: DispatchTimestampClaimOriginV1,
    pub stage: DispatchTimestampStageV1,
    pub clock_domain: ProfileIdentityV1,
    pub ticks: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchClockCorrelationBracketV1 {
    pub claim_origin: DispatchTimestampClaimOriginV1,
    pub cpu_clock_domain: ProfileIdentityV1,
    pub device_clock_domain: ProfileIdentityV1,
    pub cpu_before_ticks: u64,
    pub device_ticks: u64,
    pub cpu_after_ticks: u64,
}

/// Raw producer input for one exact runtime dispatch.
///
/// The runtime identities are repeated deliberately. Admission compares each
/// one with the exact runtime-profile owner instead of joining by order alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct DispatchTimestampObservationInputV1 {
    pub dispatch: ProfileIdentityV1,
    pub queue: ProfileIdentityV1,
    pub device: ProfileIdentityV1,
    pub kernel: ProfileIdentityV1,
    pub artifact: ProfileContentIdentityV1,
    pub publication_sequence: u64,
    pub publish_cpu_ticks: u64,
    pub device_start_ticks: u64,
    pub device_end_ticks: u64,
    pub correlation_before: DispatchClockCorrelationInputV1,
    pub correlation_after: DispatchClockCorrelationInputV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg(test)]
pub struct DispatchClockCorrelationInputV1 {
    pub cpu_before_ticks: u64,
    pub device_ticks: u64,
    pub cpu_after_ticks: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampRecordV1 {
    identity: ProfileIdentityV1,
    claim_origin: DispatchTimestampClaimOriginV1,
    dispatch: ProfileIdentityV1,
    queue: ProfileIdentityV1,
    device: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    publication_sequence: u64,
    publication: DispatchTimestampPointV1,
    device_start: DispatchTimestampPointV1,
    device_end: DispatchTimestampPointV1,
    correlation_before: DispatchClockCorrelationBracketV1,
    correlation_after: DispatchClockCorrelationBracketV1,
}

impl DispatchTimestampRecordV1 {
    pub const fn identity(&self) -> ProfileIdentityV1 {
        self.identity
    }

    pub const fn dispatch(&self) -> ProfileIdentityV1 {
        self.dispatch
    }

    pub const fn queue(&self) -> ProfileIdentityV1 {
        self.queue
    }

    pub const fn device(&self) -> ProfileIdentityV1 {
        self.device
    }

    pub const fn kernel(&self) -> ProfileIdentityV1 {
        self.kernel
    }

    pub const fn artifact(&self) -> ProfileContentIdentityV1 {
        self.artifact
    }

    pub const fn publication_sequence(&self) -> u64 {
        self.publication_sequence
    }

    pub const fn publication(&self) -> DispatchTimestampPointV1 {
        self.publication
    }

    pub const fn device_start(&self) -> DispatchTimestampPointV1 {
        self.device_start
    }

    pub const fn device_end(&self) -> DispatchTimestampPointV1 {
        self.device_end
    }

    pub const fn correlation_before(&self) -> DispatchClockCorrelationBracketV1 {
        self.correlation_before
    }

    pub const fn correlation_after(&self) -> DispatchClockCorrelationBracketV1 {
        self.correlation_after
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampCompletenessV1 {
    CompleteAllRuntimeProfileDispatches,
    PartialRuntimeProfileDispatches,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampCoverageV1 {
    pub claim_origin: DispatchTimestampClaimOriginV1,
    pub completeness: DispatchTimestampCompletenessV1,
    pub runtime_profile_dispatches: u64,
    pub producer_claimed_dispatches: u64,
    pub dropped_records: u64,
    pub truncated: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DispatchTimestampUnavailableFactV1 {
    AuthenticatedProducerProvenance,
    DevicePublicationTimestamp,
    ClockFrequencyAndNanosecondNormalization,
    GloballySynchronizedTime,
}

pub const DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1: [DispatchTimestampUnavailableFactV1; 4] = [
    DispatchTimestampUnavailableFactV1::AuthenticatedProducerProvenance,
    DispatchTimestampUnavailableFactV1::DevicePublicationTimestamp,
    DispatchTimestampUnavailableFactV1::ClockFrequencyAndNanosecondNormalization,
    DispatchTimestampUnavailableFactV1::GloballySynchronizedTime,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchTimestampCaptureV1 {
    schema: String,
    schema_version: u16,
    runtime_profile: ProfileContentIdentityV1,
    runtime_capture_scope: ProfileIdentityV1,
    producer: DispatchTimestampProducerKindV1,
    provenance_admission: DispatchTimestampProvenanceAdmissionV1,
    producer_evidence: ProfileContentIdentityV1,
    collection_configuration: ProfileContentIdentityV1,
    clock_domains: DispatchClockDomainsV1,
    records: Vec<DispatchTimestampRecordV1>,
    coverage: DispatchTimestampCoverageV1,
    unavailable: [DispatchTimestampUnavailableFactV1; 4],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DispatchTimestampCaptureWireV1 {
    schema: String,
    schema_version: u16,
    runtime_profile: ProfileContentIdentityV1,
    runtime_capture_scope: ProfileIdentityV1,
    producer: DispatchTimestampProducerKindV1,
    provenance_admission: DispatchTimestampProvenanceAdmissionV1,
    producer_evidence: ProfileContentIdentityV1,
    collection_configuration: ProfileContentIdentityV1,
    clock_domains: DispatchClockDomainsV1,
    #[serde(deserialize_with = "deserialize_bounded_records")]
    records: Vec<DispatchTimestampRecordV1>,
    coverage: DispatchTimestampCoverageV1,
    unavailable: [DispatchTimestampUnavailableFactV1; 4],
}

impl From<DispatchTimestampCaptureWireV1> for DispatchTimestampCaptureV1 {
    fn from(wire: DispatchTimestampCaptureWireV1) -> Self {
        Self {
            schema: wire.schema,
            schema_version: wire.schema_version,
            runtime_profile: wire.runtime_profile,
            runtime_capture_scope: wire.runtime_capture_scope,
            producer: wire.producer,
            provenance_admission: wire.provenance_admission,
            producer_evidence: wire.producer_evidence,
            collection_configuration: wire.collection_configuration,
            clock_domains: wire.clock_domains,
            records: wire.records,
            coverage: wire.coverage,
            unavailable: wire.unavailable,
        }
    }
}

#[derive(Clone, Copy)]
struct RuntimeDispatchOwnerV1 {
    queue: ProfileIdentityV1,
    device: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    publication_sequence: u64,
}

impl DispatchTimestampCaptureV1 {
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    fn observed(
        runtime_profile_bytes: &[u8],
        producer: DispatchTimestampProducerKindV1,
        producer_evidence_bytes: &[u8],
        collection_configuration_bytes: &[u8],
        observations: Vec<DispatchTimestampObservationInputV1>,
        dropped_records: u64,
        truncated: bool,
    ) -> Result<Self, DispatchTimestampCaptureErrorV1> {
        validate_external_size(
            producer_evidence_bytes.len(),
            MAX_DISPATCH_TIMESTAMP_PRODUCER_EVIDENCE_BYTES_V1,
        )?;
        validate_external_size(
            collection_configuration_bytes.len(),
            MAX_DISPATCH_TIMESTAMP_CONFIGURATION_BYTES_V1,
        )?;
        if producer_evidence_bytes.is_empty() || collection_configuration_bytes.is_empty() {
            return Err(DispatchTimestampCaptureErrorV1::EmptyProducerEvidence);
        }
        let (runtime, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(runtime_profile_bytes)
                .map_err(|_| DispatchTimestampCaptureErrorV1::RuntimeProfileRejected)?;
        let producer_evidence = ProfileContentIdentityV1::observed(producer_evidence_bytes)
            .map_err(|_| DispatchTimestampCaptureErrorV1::Identity)?;
        let collection_configuration =
            ProfileContentIdentityV1::observed(collection_configuration_bytes)
                .map_err(|_| DispatchTimestampCaptureErrorV1::Identity)?;
        let clock_domains = derive_clock_domains(
            runtime_profile,
            runtime.device.identity,
            producer,
            producer_evidence,
            collection_configuration,
        )?;
        if observations.len() > MAX_DISPATCH_TIMESTAMP_RECORDS_V1 {
            return Err(DispatchTimestampCaptureErrorV1::RecordLimitExceeded);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(observations.len())
            .map_err(|_| DispatchTimestampCaptureErrorV1::AllocationFailure)?;
        for input in observations {
            let publication = DispatchTimestampPointV1 {
                claim_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                stage: DispatchTimestampStageV1::Publish,
                clock_domain: clock_domains.cpu.identity,
                ticks: input.publish_cpu_ticks,
            };
            let device_start = DispatchTimestampPointV1 {
                claim_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                stage: DispatchTimestampStageV1::DeviceStart,
                clock_domain: clock_domains.device.identity,
                ticks: input.device_start_ticks,
            };
            let device_end = DispatchTimestampPointV1 {
                claim_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                stage: DispatchTimestampStageV1::DeviceEnd,
                clock_domain: clock_domains.device.identity,
                ticks: input.device_end_ticks,
            };
            let correlation_before = correlation(clock_domains, input.correlation_before);
            let correlation_after = correlation(clock_domains, input.correlation_after);
            let mut record = DispatchTimestampRecordV1 {
                identity: ProfileIdentityV1::new([1; 32])
                    .map_err(|_| DispatchTimestampCaptureErrorV1::Identity)?,
                claim_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                dispatch: input.dispatch,
                queue: input.queue,
                device: input.device,
                kernel: input.kernel,
                artifact: input.artifact,
                publication_sequence: input.publication_sequence,
                publication,
                device_start,
                device_end,
                correlation_before,
                correlation_after,
            };
            record.identity = derive_record_identity(&record)?;
            records.push(record);
        }
        let runtime_dispatches = runtime_dispatch_owners(&runtime)?;
        let expected = u64::try_from(runtime_dispatches.len())
            .map_err(|_| DispatchTimestampCaptureErrorV1::SizeOverflow)?;
        let observed = u64::try_from(records.len())
            .map_err(|_| DispatchTimestampCaptureErrorV1::SizeOverflow)?;
        let complete = runtime.coverage.complete_runtime_operation_history
            && expected != 0
            && !truncated
            && dropped_records == 0
            && observed == expected;
        let capture = Self {
            schema: DISPATCH_TIMESTAMP_CAPTURE_SCHEMA_V1.to_owned(),
            schema_version: DISPATCH_TIMESTAMP_CAPTURE_SCHEMA_VERSION_V1,
            runtime_profile,
            runtime_capture_scope: runtime.capture_scope,
            producer,
            provenance_admission:
                DispatchTimestampProvenanceAdmissionV1::StructurallyAdmittedProducerClaimOnly,
            producer_evidence,
            collection_configuration,
            clock_domains,
            records,
            coverage: DispatchTimestampCoverageV1 {
                claim_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
                completeness: if complete {
                    DispatchTimestampCompletenessV1::CompleteAllRuntimeProfileDispatches
                } else {
                    DispatchTimestampCompletenessV1::PartialRuntimeProfileDispatches
                },
                runtime_profile_dispatches: expected,
                producer_claimed_dispatches: observed,
                dropped_records,
                truncated,
            },
            unavailable: DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1,
        };
        capture.validate_against(
            &runtime,
            runtime_profile,
            producer_evidence,
            collection_configuration,
        )?;
        Ok(capture)
    }

    pub fn validate_against_bytes(
        &self,
        runtime_profile_bytes: &[u8],
        producer_evidence_bytes: &[u8],
        collection_configuration_bytes: &[u8],
    ) -> Result<(), DispatchTimestampCaptureErrorV1> {
        validate_external_size(
            producer_evidence_bytes.len(),
            MAX_DISPATCH_TIMESTAMP_PRODUCER_EVIDENCE_BYTES_V1,
        )?;
        validate_external_size(
            collection_configuration_bytes.len(),
            MAX_DISPATCH_TIMESTAMP_CONFIGURATION_BYTES_V1,
        )?;
        if producer_evidence_bytes.is_empty() || collection_configuration_bytes.is_empty() {
            return Err(DispatchTimestampCaptureErrorV1::EmptyProducerEvidence);
        }
        let (runtime, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(runtime_profile_bytes)
                .map_err(|_| DispatchTimestampCaptureErrorV1::RuntimeProfileRejected)?;
        let producer_evidence = ProfileContentIdentityV1::observed(producer_evidence_bytes)
            .map_err(|_| DispatchTimestampCaptureErrorV1::Identity)?;
        let configuration = ProfileContentIdentityV1::observed(collection_configuration_bytes)
            .map_err(|_| DispatchTimestampCaptureErrorV1::Identity)?;
        self.validate_against(&runtime, runtime_profile, producer_evidence, configuration)
    }

    fn validate_against(
        &self,
        runtime: &KfdRuntimeProfileV1,
        runtime_profile: ProfileContentIdentityV1,
        producer_evidence: ProfileContentIdentityV1,
        configuration: ProfileContentIdentityV1,
    ) -> Result<(), DispatchTimestampCaptureErrorV1> {
        if self.schema != DISPATCH_TIMESTAMP_CAPTURE_SCHEMA_V1
            || self.schema_version != DISPATCH_TIMESTAMP_CAPTURE_SCHEMA_VERSION_V1
        {
            return Err(DispatchTimestampCaptureErrorV1::UnsupportedVersion);
        }
        if self.provenance_admission
            != DispatchTimestampProvenanceAdmissionV1::StructurallyAdmittedProducerClaimOnly
        {
            return Err(DispatchTimestampCaptureErrorV1::InvalidProvenanceAdmission);
        }
        if self.runtime_profile != runtime_profile
            || self.runtime_capture_scope != runtime.capture_scope
        {
            return Err(DispatchTimestampCaptureErrorV1::StaleRuntimeProfile);
        }
        if self.producer_evidence != producer_evidence
            || self.collection_configuration != configuration
        {
            return Err(DispatchTimestampCaptureErrorV1::ProducerEvidenceMismatch);
        }
        if self.clock_domains
            != derive_clock_domains(
                runtime_profile,
                runtime.device.identity,
                self.producer,
                producer_evidence,
                configuration,
            )?
        {
            return Err(DispatchTimestampCaptureErrorV1::ClockDomainMismatch);
        }
        if self.records.len() > MAX_DISPATCH_TIMESTAMP_RECORDS_V1 {
            return Err(DispatchTimestampCaptureErrorV1::RecordLimitExceeded);
        }
        let owners = runtime_dispatch_owners(runtime)?;
        let expected = u64::try_from(owners.len())
            .map_err(|_| DispatchTimestampCaptureErrorV1::SizeOverflow)?;
        let observed = u64::try_from(self.records.len())
            .map_err(|_| DispatchTimestampCaptureErrorV1::SizeOverflow)?;
        let expected_complete = runtime.coverage.complete_runtime_operation_history
            && expected != 0
            && !self.coverage.truncated
            && self.coverage.dropped_records == 0
            && observed == expected;
        if self.coverage.claim_origin != DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            || self.coverage.runtime_profile_dispatches != expected
            || self.coverage.producer_claimed_dispatches != observed
            || self.coverage.completeness
                != if expected_complete {
                    DispatchTimestampCompletenessV1::CompleteAllRuntimeProfileDispatches
                } else {
                    DispatchTimestampCompletenessV1::PartialRuntimeProfileDispatches
                }
            || self.unavailable != DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1
        {
            return Err(DispatchTimestampCaptureErrorV1::InvalidCoverage);
        }
        let mut prior_sequence = None;
        let mut seen = BTreeSet::new();
        for record in &self.records {
            if record.claim_origin != DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
                || record.identity != derive_record_identity(record)?
            {
                return Err(DispatchTimestampCaptureErrorV1::StaleRecordIdentity);
            }
            if prior_sequence.is_some_and(|prior| prior >= record.publication_sequence) {
                return Err(DispatchTimestampCaptureErrorV1::NonCanonicalRecordOrder);
            }
            prior_sequence = Some(record.publication_sequence);
            if !seen.insert(record.dispatch) {
                return Err(DispatchTimestampCaptureErrorV1::DuplicateDispatch);
            }
            let owner = owners
                .get(&record.dispatch)
                .ok_or(DispatchTimestampCaptureErrorV1::DispatchIdentityMismatch)?;
            if record.queue != owner.queue
                || record.device != owner.device
                || record.kernel != owner.kernel
                || record.artifact != owner.artifact
                || record.publication_sequence != owner.publication_sequence
            {
                return Err(DispatchTimestampCaptureErrorV1::DispatchIdentityMismatch);
            }
            validate_record_clocks(record, self.clock_domains)?;
        }
        if expected_complete && seen.len() != owners.len() {
            return Err(DispatchTimestampCaptureErrorV1::MissingDispatch);
        }
        Ok(())
    }

    pub const fn has_complete_producer_claimed_device_timestamps(&self) -> bool {
        matches!(
            self.coverage.completeness,
            DispatchTimestampCompletenessV1::CompleteAllRuntimeProfileDispatches
        )
    }

    pub const fn runtime_profile(&self) -> ProfileContentIdentityV1 {
        self.runtime_profile
    }

    pub const fn runtime_capture_scope(&self) -> ProfileIdentityV1 {
        self.runtime_capture_scope
    }

    pub const fn producer(&self) -> DispatchTimestampProducerKindV1 {
        self.producer
    }

    pub const fn provenance_admission(&self) -> DispatchTimestampProvenanceAdmissionV1 {
        self.provenance_admission
    }

    pub const fn producer_evidence(&self) -> ProfileContentIdentityV1 {
        self.producer_evidence
    }

    pub const fn collection_configuration(&self) -> ProfileContentIdentityV1 {
        self.collection_configuration
    }

    pub const fn clock_domains(&self) -> DispatchClockDomainsV1 {
        self.clock_domains
    }

    pub fn records(&self) -> &[DispatchTimestampRecordV1] {
        &self.records
    }

    pub const fn coverage(&self) -> DispatchTimestampCoverageV1 {
        self.coverage
    }

    pub fn unavailable(&self) -> &[DispatchTimestampUnavailableFactV1] {
        &self.unavailable
    }
}

fn deserialize_bounded_records<'de, D>(
    deserializer: D,
) -> Result<Vec<DispatchTimestampRecordV1>, D::Error>
where
    D: Deserializer<'de>,
{
    struct RecordsVisitorV1;

    impl<'de> Visitor<'de> for RecordsVisitorV1 {
        type Value = Vec<DispatchTimestampRecordV1>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {MAX_DISPATCH_TIMESTAMP_RECORDS_V1} dispatch timestamp records"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let capacity = sequence
                .size_hint()
                .unwrap_or(0)
                .min(MAX_DISPATCH_TIMESTAMP_RECORDS_V1);
            let mut records = Vec::new();
            records
                .try_reserve_exact(capacity)
                .map_err(|_| serde::de::Error::custom("dispatch timestamp allocation failed"))?;
            while let Some(record) = sequence.next_element()? {
                if records.len() == MAX_DISPATCH_TIMESTAMP_RECORDS_V1 {
                    return Err(serde::de::Error::custom(
                        "dispatch timestamp record limit exceeded",
                    ));
                }
                records.push(record);
            }
            Ok(records)
        }
    }

    deserializer.deserialize_seq(RecordsVisitorV1)
}

#[cfg(test)]
fn correlation(
    domains: DispatchClockDomainsV1,
    input: DispatchClockCorrelationInputV1,
) -> DispatchClockCorrelationBracketV1 {
    DispatchClockCorrelationBracketV1 {
        claim_origin: DispatchTimestampClaimOriginV1::ProducerDeclaredObservation,
        cpu_clock_domain: domains.cpu.identity,
        device_clock_domain: domains.device.identity,
        cpu_before_ticks: input.cpu_before_ticks,
        device_ticks: input.device_ticks,
        cpu_after_ticks: input.cpu_after_ticks,
    }
}

fn validate_record_clocks(
    record: &DispatchTimestampRecordV1,
    domains: DispatchClockDomainsV1,
) -> Result<(), DispatchTimestampCaptureErrorV1> {
    let observed_point = |point: DispatchTimestampPointV1,
                          stage: DispatchTimestampStageV1,
                          domain: ProfileIdentityV1| {
        point.claim_origin == DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            && point.stage == stage
            && point.clock_domain == domain
    };
    if !observed_point(
        record.publication,
        DispatchTimestampStageV1::Publish,
        domains.cpu.identity,
    ) || !observed_point(
        record.device_start,
        DispatchTimestampStageV1::DeviceStart,
        domains.device.identity,
    ) || !observed_point(
        record.device_end,
        DispatchTimestampStageV1::DeviceEnd,
        domains.device.identity,
    ) {
        return Err(DispatchTimestampCaptureErrorV1::ClockDomainMismatch);
    }
    for bracket in [record.correlation_before, record.correlation_after] {
        if bracket.claim_origin != DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            || bracket.cpu_clock_domain != domains.cpu.identity
            || bracket.device_clock_domain != domains.device.identity
            || bracket.cpu_before_ticks > bracket.cpu_after_ticks
        {
            return Err(DispatchTimestampCaptureErrorV1::InvalidCorrelationBracket);
        }
    }
    if record.device_start.ticks > record.device_end.ticks
        || record.correlation_before.cpu_after_ticks > record.publication.ticks
        || record.publication.ticks > record.correlation_after.cpu_before_ticks
        || record.correlation_before.device_ticks > record.device_start.ticks
        || record.device_end.ticks > record.correlation_after.device_ticks
    {
        return Err(DispatchTimestampCaptureErrorV1::ImpossibleTimestampOrder);
    }
    Ok(())
}

fn runtime_dispatch_owners(
    runtime: &KfdRuntimeProfileV1,
) -> Result<BTreeMap<ProfileIdentityV1, RuntimeDispatchOwnerV1>, DispatchTimestampCaptureErrorV1> {
    let mut modules = BTreeMap::new();
    let mut kernels = BTreeMap::new();
    let mut dispatches = BTreeMap::new();
    for event in &runtime.events {
        match &event.event {
            KfdRuntimeProfileEventKindV1::ModuleLoaded { module, artifact } => {
                modules.insert(*module, *artifact);
            }
            KfdRuntimeProfileEventKindV1::KernelResolved { kernel, module, .. } => {
                kernels.insert(*kernel, *module);
            }
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                kernel,
                ..
            } => {
                let module = kernels
                    .get(kernel)
                    .ok_or(DispatchTimestampCaptureErrorV1::RuntimeProfileRejected)?;
                let artifact = modules
                    .get(module)
                    .ok_or(DispatchTimestampCaptureErrorV1::RuntimeProfileRejected)?;
                dispatches.insert(
                    *dispatch,
                    RuntimeDispatchOwnerV1 {
                        queue: *queue,
                        device: runtime.device.identity,
                        kernel: *kernel,
                        artifact: *artifact,
                        publication_sequence: event.sequence,
                    },
                );
            }
            _ => {}
        }
    }
    Ok(dispatches)
}

fn derive_clock_domains(
    runtime_profile: ProfileContentIdentityV1,
    device: ProfileIdentityV1,
    producer: DispatchTimestampProducerKindV1,
    producer_evidence: ProfileContentIdentityV1,
    configuration: ProfileContentIdentityV1,
) -> Result<DispatchClockDomainsV1, DispatchTimestampCaptureErrorV1> {
    let base = serde_json::to_vec(&(
        runtime_profile,
        device,
        producer,
        producer_evidence,
        configuration,
    ))
    .map_err(|_| DispatchTimestampCaptureErrorV1::JsonEncode)?;
    let domain = |kind: DispatchClockDomainKindV1| {
        let kind_bytes =
            serde_json::to_vec(&kind).map_err(|_| DispatchTimestampCaptureErrorV1::JsonEncode)?;
        identity(CLOCK_DOMAIN_IDENTITY_DOMAIN_V1, &[&base, &kind_bytes])
    };
    Ok(DispatchClockDomainsV1 {
        cpu: DispatchClockDomainV1 {
            identity: domain(DispatchClockDomainKindV1::ProducerCpuMonotonicCounter)?,
            kind: DispatchClockDomainKindV1::ProducerCpuMonotonicCounter,
            unit: DispatchClockTickUnitV1::OpaqueProducerTicks,
        },
        device: DispatchClockDomainV1 {
            identity: domain(DispatchClockDomainKindV1::ProducerDeviceCounter)?,
            kind: DispatchClockDomainKindV1::ProducerDeviceCounter,
            unit: DispatchClockTickUnitV1::OpaqueProducerTicks,
        },
    })
}

#[derive(Serialize)]
struct RecordIdentityPreimageV1 {
    claim_origin: DispatchTimestampClaimOriginV1,
    dispatch: ProfileIdentityV1,
    queue: ProfileIdentityV1,
    device: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    publication_sequence: u64,
    publication: DispatchTimestampPointV1,
    device_start: DispatchTimestampPointV1,
    device_end: DispatchTimestampPointV1,
    correlation_before: DispatchClockCorrelationBracketV1,
    correlation_after: DispatchClockCorrelationBracketV1,
}

fn derive_record_identity(
    record: &DispatchTimestampRecordV1,
) -> Result<ProfileIdentityV1, DispatchTimestampCaptureErrorV1> {
    let preimage = RecordIdentityPreimageV1 {
        claim_origin: record.claim_origin,
        dispatch: record.dispatch,
        queue: record.queue,
        device: record.device,
        kernel: record.kernel,
        artifact: record.artifact,
        publication_sequence: record.publication_sequence,
        publication: record.publication,
        device_start: record.device_start,
        device_end: record.device_end,
        correlation_before: record.correlation_before,
        correlation_after: record.correlation_after,
    };
    let bytes =
        serde_json::to_vec(&preimage).map_err(|_| DispatchTimestampCaptureErrorV1::JsonEncode)?;
    identity(RECORD_IDENTITY_DOMAIN_V1, &[&bytes])
}

fn identity(
    domain: &[u8],
    parts: &[&[u8]],
) -> Result<ProfileIdentityV1, DispatchTimestampCaptureErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    ProfileIdentityV1::new(hasher.finalize().into())
        .map_err(|_| DispatchTimestampCaptureErrorV1::Identity)
}

fn validate_external_size(len: usize, max: u64) -> Result<(), DispatchTimestampCaptureErrorV1> {
    let actual = u64::try_from(len).map_err(|_| DispatchTimestampCaptureErrorV1::SizeOverflow)?;
    if actual > max {
        return Err(DispatchTimestampCaptureErrorV1::InputTooLarge { actual, max });
    }
    Ok(())
}

pub fn encode_dispatch_timestamp_capture_v1(
    capture: &DispatchTimestampCaptureV1,
    runtime_profile_bytes: &[u8],
    producer_evidence_bytes: &[u8],
    collection_configuration_bytes: &[u8],
) -> Result<Vec<u8>, DispatchTimestampCaptureErrorV1> {
    capture.validate_against_bytes(
        runtime_profile_bytes,
        producer_evidence_bytes,
        collection_configuration_bytes,
    )?;
    let bytes =
        serde_json::to_vec(capture).map_err(|_| DispatchTimestampCaptureErrorV1::JsonEncode)?;
    validate_external_size(bytes.len(), MAX_DISPATCH_TIMESTAMP_CAPTURE_BYTES_V1)?;
    Ok(bytes)
}

/// Structurally admits an external producer claim against exact evidence.
///
/// This checks canonical bytes, bounds, identities, current runtime ownership,
/// and clock ordering. It does not authenticate that the named producer made
/// the observations. Callers must not treat the result as collection custody
/// or hardware-observation authority.
pub fn decode_dispatch_timestamp_capture_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
    producer_evidence_bytes: &[u8],
    collection_configuration_bytes: &[u8],
) -> Result<DispatchTimestampCaptureV1, DispatchTimestampCaptureErrorV1> {
    decode_dispatch_timestamp_capture_with_content_identity_v1(
        bytes,
        runtime_profile_bytes,
        producer_evidence_bytes,
        collection_configuration_bytes,
    )
    .map(|(capture, _)| capture)
}

/// Structurally admits a capture and derives its content identity in the same pass.
pub fn decode_dispatch_timestamp_capture_with_content_identity_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
    producer_evidence_bytes: &[u8],
    collection_configuration_bytes: &[u8],
) -> Result<(DispatchTimestampCaptureV1, ProfileContentIdentityV1), DispatchTimestampCaptureErrorV1>
{
    validate_external_size(bytes.len(), MAX_DISPATCH_TIMESTAMP_CAPTURE_BYTES_V1)?;
    let capture = serde_json::from_slice::<DispatchTimestampCaptureWireV1>(bytes)
        .map(DispatchTimestampCaptureV1::from)
        .map_err(|_| DispatchTimestampCaptureErrorV1::JsonDecode)?;
    capture.validate_against_bytes(
        runtime_profile_bytes,
        producer_evidence_bytes,
        collection_configuration_bytes,
    )?;
    if serde_json::to_vec(&capture).map_err(|_| DispatchTimestampCaptureErrorV1::JsonEncode)?
        != bytes
    {
        return Err(DispatchTimestampCaptureErrorV1::NonCanonicalEncoding);
    }
    let content_identity = ProfileContentIdentityV1 {
        digest: identity(CAPTURE_CONTENT_IDENTITY_DOMAIN_V1, &[bytes])?,
        byte_len: u64::try_from(bytes.len())
            .map_err(|_| DispatchTimestampCaptureErrorV1::SizeOverflow)?,
    };
    Ok((capture, content_identity))
}

pub fn dispatch_timestamp_capture_content_identity_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
    producer_evidence_bytes: &[u8],
    collection_configuration_bytes: &[u8],
) -> Result<ProfileContentIdentityV1, DispatchTimestampCaptureErrorV1> {
    decode_dispatch_timestamp_capture_with_content_identity_v1(
        bytes,
        runtime_profile_bytes,
        producer_evidence_bytes,
        collection_configuration_bytes,
    )
    .map(|(_, identity)| identity)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchTimestampCaptureErrorV1 {
    UnsupportedVersion,
    InputTooLarge { actual: u64, max: u64 },
    EmptyProducerEvidence,
    RecordLimitExceeded,
    AllocationFailure,
    SizeOverflow,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
    Identity,
    RuntimeProfileRejected,
    StaleRuntimeProfile,
    InvalidProvenanceAdmission,
    ProducerEvidenceMismatch,
    ClockDomainMismatch,
    StaleRecordIdentity,
    DispatchIdentityMismatch,
    DuplicateDispatch,
    MissingDispatch,
    NonCanonicalRecordOrder,
    InvalidCorrelationBracket,
    ImpossibleTimestampOrder,
    InvalidCoverage,
}

impl fmt::Display for DispatchTimestampCaptureErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "dispatch timestamp capture rejected: {self:?}")
    }
}

impl Error for DispatchTimestampCaptureErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KfdProfileHostContentModeV1, KfdProfileHostTimingV1, KfdProfileLaunchV1,
        KfdProfileResourceKindV1, KfdRuntimeProfileEventKindV1, KfdRuntimeProfileV1,
        encode_kfd_runtime_profile_v1, push_observed_event_v1, resource_identity_v1,
    };

    const EVIDENCE: &[u8] = b"real-producer-receipt-v1";
    const CONFIGURATION: &[u8] = b"device-timestamps=true;correlation=bracketed";

    struct FixtureV1 {
        runtime_bytes: Vec<u8>,
        inputs: Vec<DispatchTimestampObservationInputV1>,
    }

    fn fixture(dispatch_count: u64, host_mode: KfdProfileHostContentModeV1) -> FixtureV1 {
        let scope = ProfileIdentityV1::new([7; 32]).unwrap();
        let device = crate::KfdProfileDeviceV1::observed(71, "gfx942:xnack-", 64).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let artifact = ProfileContentIdentityV1::observed(b"exact-hsaco").unwrap();
        let mut events = Vec::new();
        for event in [
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::ModuleLoaded { module, artifact },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name: ProfileContentIdentityV1::observed(b"kernel").unwrap(),
                signature: ProfileContentIdentityV1::observed(b"signature").unwrap(),
            },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        let mut inputs = Vec::new();
        for ordinal in 0..dispatch_count {
            let dispatch =
                resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 10 + ordinal)
                    .unwrap();
            let publication_sequence = events.len() as u64;
            push_observed_event_v1(
                scope,
                &mut events,
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch,
                    queue,
                    stream,
                    kernel,
                    dispatch_shape: ProfileContentIdentityV1::observed(&ordinal.to_le_bytes())
                        .unwrap(),
                    launch: KfdProfileLaunchV1 {
                        grid: [64, 1, 1],
                        workgroup: [64, 1, 1],
                        dynamic_shared_bytes: 0,
                    },
                    bindings: Vec::new(),
                },
            )
            .unwrap();
            push_observed_event_v1(
                scope,
                &mut events,
                KfdRuntimeProfileEventKindV1::DispatchCompleted {
                    dispatch,
                    host_timing: KfdProfileHostTimingV1::default(),
                },
            )
            .unwrap();
            push_observed_event_v1(
                scope,
                &mut events,
                KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            )
            .unwrap();
            let cpu = 100 + ordinal * 100;
            let gpu = 1_000 + ordinal * 1_000;
            inputs.push(DispatchTimestampObservationInputV1 {
                dispatch,
                queue,
                device: device.identity,
                kernel,
                artifact,
                publication_sequence,
                publish_cpu_ticks: cpu + 20,
                device_start_ticks: gpu + 20,
                device_end_ticks: gpu + 80,
                correlation_before: DispatchClockCorrelationInputV1 {
                    cpu_before_ticks: cpu,
                    device_ticks: gpu,
                    cpu_after_ticks: cpu + 10,
                },
                correlation_after: DispatchClockCorrelationInputV1 {
                    cpu_before_ticks: cpu + 30,
                    device_ticks: gpu + 100,
                    cpu_after_ticks: cpu + 40,
                },
            });
        }
        for event in [
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        let runtime = KfdRuntimeProfileV1::new(scope, device, host_mode, events, 0).unwrap();
        FixtureV1 {
            runtime_bytes: encode_kfd_runtime_profile_v1(&runtime).unwrap(),
            inputs,
        }
    }

    fn capture(fixture: &FixtureV1) -> DispatchTimestampCaptureV1 {
        DispatchTimestampCaptureV1::observed(
            &fixture.runtime_bytes,
            DispatchTimestampProducerKindV1::DirectKfdDeviceTimestamp,
            EVIDENCE,
            CONFIGURATION,
            fixture.inputs.clone(),
            0,
            false,
        )
        .unwrap()
    }

    fn reauthenticate(record: &mut DispatchTimestampRecordV1) {
        record.identity = derive_record_identity(record).unwrap();
    }

    fn validate(
        capture: &DispatchTimestampCaptureV1,
        fixture: &FixtureV1,
    ) -> Result<(), DispatchTimestampCaptureErrorV1> {
        capture.validate_against_bytes(&fixture.runtime_bytes, EVIDENCE, CONFIGURATION)
    }

    #[test]
    fn complete_capture_round_trips_with_raw_clock_boundaries() {
        let fixture = fixture(2, KfdProfileHostContentModeV1::RangeOnly);
        let capture = capture(&fixture);
        assert!(capture.has_complete_producer_claimed_device_timestamps());
        assert_eq!(
            capture.coverage.completeness,
            DispatchTimestampCompletenessV1::CompleteAllRuntimeProfileDispatches
        );
        assert_eq!(
            capture.coverage.claim_origin,
            DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
        );
        assert_eq!(capture.records.len(), 2);
        for record in capture.records() {
            assert_eq!(
                record.claim_origin,
                DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            );
            assert_eq!(
                record.publication().claim_origin,
                DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            );
            assert_eq!(
                record.device_start().claim_origin,
                DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            );
            assert_eq!(
                record.device_end().claim_origin,
                DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            );
            assert_eq!(
                record.correlation_before().claim_origin,
                DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            );
            assert_eq!(
                record.correlation_after().claim_origin,
                DispatchTimestampClaimOriginV1::ProducerDeclaredObservation
            );
        }
        assert_eq!(capture.unavailable, DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1);
        let bytes = encode_dispatch_timestamp_capture_v1(
            &capture,
            &fixture.runtime_bytes,
            EVIDENCE,
            CONFIGURATION,
        )
        .unwrap();
        assert_eq!(
            decode_dispatch_timestamp_capture_v1(
                &bytes,
                &fixture.runtime_bytes,
                EVIDENCE,
                CONFIGURATION,
            )
            .unwrap(),
            capture
        );
        assert_eq!(
            dispatch_timestamp_capture_content_identity_v1(
                &bytes,
                &fixture.runtime_bytes,
                EVIDENCE,
                CONFIGURATION,
            )
            .unwrap(),
            dispatch_timestamp_capture_content_identity_v1(
                &bytes,
                &fixture.runtime_bytes,
                EVIDENCE,
                CONFIGURATION,
            )
            .unwrap()
        );
    }

    #[test]
    fn exact_runtime_producer_and_configuration_bytes_are_currentness_inputs() {
        let baseline = fixture(1, KfdProfileHostContentModeV1::RangeOnly);
        let capture = capture(&baseline);
        let same_scope_different_bytes = fixture(1, KfdProfileHostContentModeV1::ContentIdentity);
        assert_eq!(
            capture.validate_against_bytes(
                &same_scope_different_bytes.runtime_bytes,
                EVIDENCE,
                CONFIGURATION,
            ),
            Err(DispatchTimestampCaptureErrorV1::StaleRuntimeProfile)
        );
        assert_eq!(
            capture.validate_against_bytes(
                &baseline.runtime_bytes,
                b"substituted-receipt",
                CONFIGURATION,
            ),
            Err(DispatchTimestampCaptureErrorV1::ProducerEvidenceMismatch)
        );
        assert_eq!(
            capture.validate_against_bytes(
                &baseline.runtime_bytes,
                EVIDENCE,
                b"substituted-configuration",
            ),
            Err(DispatchTimestampCaptureErrorV1::ProducerEvidenceMismatch)
        );
    }

    #[test]
    fn every_runtime_identity_bearing_axis_rejects_substitution() {
        let fixture = fixture(1, KfdProfileHostContentModeV1::RangeOnly);
        for mutate in 0..6 {
            let mut hostile = capture(&fixture);
            let record = &mut hostile.records[0];
            let other = ProfileIdentityV1::new([90 + mutate; 32]).unwrap();
            match mutate {
                0 => record.dispatch = other,
                1 => record.queue = other,
                2 => record.device = other,
                3 => record.kernel = other,
                4 => record.artifact = ProfileContentIdentityV1::observed(b"other-hsaco").unwrap(),
                5 => record.publication_sequence += 1,
                _ => unreachable!(),
            }
            reauthenticate(record);
            assert_eq!(
                validate(&hostile, &fixture),
                Err(DispatchTimestampCaptureErrorV1::DispatchIdentityMismatch)
            );
        }
    }

    #[test]
    fn producer_capture_scope_and_top_level_clock_domains_reject_substitution() {
        let fixture = fixture(1, KfdProfileHostContentModeV1::RangeOnly);

        let mut producer = capture(&fixture);
        producer.producer = DispatchTimestampProducerKindV1::RocprofilerSdkDispatchCorrelation;
        assert_eq!(
            validate(&producer, &fixture),
            Err(DispatchTimestampCaptureErrorV1::ClockDomainMismatch)
        );

        let mut scope = capture(&fixture);
        scope.runtime_capture_scope = ProfileIdentityV1::new([88; 32]).unwrap();
        assert_eq!(
            validate(&scope, &fixture),
            Err(DispatchTimestampCaptureErrorV1::StaleRuntimeProfile)
        );

        let mut clock = capture(&fixture);
        clock.clock_domains.device.identity = ProfileIdentityV1::new([89; 32]).unwrap();
        assert_eq!(
            validate(&clock, &fixture),
            Err(DispatchTimestampCaptureErrorV1::ClockDomainMismatch)
        );
    }

    #[test]
    fn stage_domain_and_impossible_clock_orders_fail_closed() {
        let fixture = fixture(1, KfdProfileHostContentModeV1::RangeOnly);

        let mut wrong_stage = capture(&fixture);
        wrong_stage.records[0].publication.stage = DispatchTimestampStageV1::DeviceStart;
        reauthenticate(&mut wrong_stage.records[0]);
        assert_eq!(
            validate(&wrong_stage, &fixture),
            Err(DispatchTimestampCaptureErrorV1::ClockDomainMismatch)
        );

        let mut wrong_domain = capture(&fixture);
        wrong_domain.records[0].device_start.clock_domain = wrong_domain.clock_domains.cpu.identity;
        reauthenticate(&mut wrong_domain.records[0]);
        assert_eq!(
            validate(&wrong_domain, &fixture),
            Err(DispatchTimestampCaptureErrorV1::ClockDomainMismatch)
        );

        for mutate in 0..5 {
            let mut hostile = capture(&fixture);
            let record = &mut hostile.records[0];
            match mutate {
                0 => record.device_start.ticks = record.device_end.ticks + 1,
                1 => {
                    record.correlation_before.cpu_before_ticks =
                        record.correlation_before.cpu_after_ticks + 1
                }
                2 => record.correlation_before.cpu_after_ticks = record.publication.ticks + 1,
                3 => record.correlation_after.cpu_before_ticks = record.publication.ticks - 1,
                4 => record.correlation_after.device_ticks = record.device_end.ticks - 1,
                _ => unreachable!(),
            }
            reauthenticate(record);
            assert!(matches!(
                validate(&hostile, &fixture),
                Err(DispatchTimestampCaptureErrorV1::InvalidCorrelationBracket)
                    | Err(DispatchTimestampCaptureErrorV1::ImpossibleTimestampOrder)
            ));
        }
    }

    #[test]
    fn loss_truncation_duplicates_missing_records_and_order_never_claim_complete() {
        let fixture = fixture(2, KfdProfileHostContentModeV1::RangeOnly);
        for (dropped, truncated) in [(1, false), (0, true)] {
            let partial = DispatchTimestampCaptureV1::observed(
                &fixture.runtime_bytes,
                DispatchTimestampProducerKindV1::DirectKfdDeviceTimestamp,
                EVIDENCE,
                CONFIGURATION,
                fixture.inputs.clone(),
                dropped,
                truncated,
            )
            .unwrap();
            assert!(!partial.has_complete_producer_claimed_device_timestamps());
            assert_eq!(
                partial.coverage.completeness,
                DispatchTimestampCompletenessV1::PartialRuntimeProfileDispatches
            );
        }

        let mut missing = capture(&fixture);
        missing.records.pop();
        missing.coverage.producer_claimed_dispatches -= 1;
        assert_eq!(
            validate(&missing, &fixture),
            Err(DispatchTimestampCaptureErrorV1::InvalidCoverage)
        );

        let mut duplicate = capture(&fixture);
        duplicate.records[1].dispatch = duplicate.records[0].dispatch;
        reauthenticate(&mut duplicate.records[1]);
        assert_eq!(
            validate(&duplicate, &fixture),
            Err(DispatchTimestampCaptureErrorV1::DuplicateDispatch)
        );

        let mut reversed = capture(&fixture);
        reversed.records.reverse();
        assert_eq!(
            validate(&reversed, &fixture),
            Err(DispatchTimestampCaptureErrorV1::NonCanonicalRecordOrder)
        );
    }

    #[test]
    fn stale_record_noncanonical_json_and_version_changes_are_rejected() {
        let fixture = fixture(1, KfdProfileHostContentModeV1::RangeOnly);
        let mut stale = capture(&fixture);
        stale.records[0].device_end.ticks += 1;
        assert_eq!(
            validate(&stale, &fixture),
            Err(DispatchTimestampCaptureErrorV1::StaleRecordIdentity)
        );

        let capture = capture(&fixture);
        let mut bytes = encode_dispatch_timestamp_capture_v1(
            &capture,
            &fixture.runtime_bytes,
            EVIDENCE,
            CONFIGURATION,
        )
        .unwrap();
        bytes.push(b'\n');
        assert_eq!(
            decode_dispatch_timestamp_capture_v1(
                &bytes,
                &fixture.runtime_bytes,
                EVIDENCE,
                CONFIGURATION,
            ),
            Err(DispatchTimestampCaptureErrorV1::NonCanonicalEncoding)
        );

        let mut wrong_version = capture;
        wrong_version.schema_version += 1;
        assert_eq!(
            validate(&wrong_version, &fixture),
            Err(DispatchTimestampCaptureErrorV1::UnsupportedVersion)
        );
    }
}

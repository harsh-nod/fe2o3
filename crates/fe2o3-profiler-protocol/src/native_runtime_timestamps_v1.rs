//! Typed host-monotonic timestamp production for the native KFD runtime.
//!
//! The producer samples its own monotonic clock and commits an opaque sample
//! only after the corresponding runtime-profile event is retained. The
//! recorder output is intentionally not deserializable: decoding stored bytes
//! re-establishes structure and currentness, but not producer custody.
//! GPU dispatch begin/end and device-clock facts are not represented.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::time::Instant;

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    KfdRuntimeProfileEventKindV1, KfdRuntimeProfileEventV1, KfdRuntimeProfileV1,
    ProfileContentIdentityV1, ProfileIdentityV1,
    decode_kfd_runtime_profile_with_content_identity_v1, encode_kfd_runtime_profile_v1,
};

pub const NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_SCHEMA_V1: &str =
    "fe2o3-native-runtime-dispatch-timestamps-v1";
pub const NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMP_RECORDS_V1: usize = 16_384;
pub const MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_BYTES_V1: u64 = 16 * 1024 * 1024;

const CLOCK_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.native-runtime.host-clock.v1\0";
const RECORD_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.native-runtime.dispatch-timestamp.v1\0";
const CAPTURE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.native-runtime.timestamp-capture.v1\0";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRuntimeHostClockUnitV1 {
    NanosecondsSinceRecorderStart,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeHostClockV1 {
    pub identity: ProfileIdentityV1,
    pub unit: NativeRuntimeHostClockUnitV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRuntimeHostTimestampStageV1 {
    PublicationObserved,
    CompletionObserved,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeHostTimestampPointV1 {
    pub stage: NativeRuntimeHostTimestampStageV1,
    pub runtime_event: ProfileIdentityV1,
    pub runtime_event_sequence: u64,
    pub clock_domain: ProfileIdentityV1,
    pub nanoseconds_since_recorder_start: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeDispatchTimestampRecordV1 {
    identity: ProfileIdentityV1,
    dispatch: ProfileIdentityV1,
    queue: ProfileIdentityV1,
    device: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    publication: NativeRuntimeHostTimestampPointV1,
    completion: Option<NativeRuntimeHostTimestampPointV1>,
}

impl NativeRuntimeDispatchTimestampRecordV1 {
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

    pub const fn publication(&self) -> NativeRuntimeHostTimestampPointV1 {
        self.publication
    }

    pub const fn completion(&self) -> Option<NativeRuntimeHostTimestampPointV1> {
        self.completion
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeDispatchTimestampCoverageV1 {
    pub runtime_profile_dispatches: u64,
    pub observed_publications: u64,
    pub observed_completions: u64,
    pub dropped_observations: u64,
    pub complete_runtime_operation_history: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeRuntimeDispatchTimestampUnavailableFactV1 {
    GpuDispatchStart,
    GpuDispatchEnd,
    DeviceClockDomain,
    GloballySynchronizedHostTime,
}

pub const NATIVE_RUNTIME_DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1:
    [NativeRuntimeDispatchTimestampUnavailableFactV1; 4] = [
    NativeRuntimeDispatchTimestampUnavailableFactV1::GpuDispatchStart,
    NativeRuntimeDispatchTimestampUnavailableFactV1::GpuDispatchEnd,
    NativeRuntimeDispatchTimestampUnavailableFactV1::DeviceClockDomain,
    NativeRuntimeDispatchTimestampUnavailableFactV1::GloballySynchronizedHostTime,
];

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NativeRuntimeDispatchTimestampCaptureV1 {
    schema: String,
    schema_version: u16,
    runtime_profile: ProfileContentIdentityV1,
    runtime_capture_scope: ProfileIdentityV1,
    recorder_occurrence: ProfileIdentityV1,
    host_clock: NativeRuntimeHostClockV1,
    records: Vec<NativeRuntimeDispatchTimestampRecordV1>,
    coverage: NativeRuntimeDispatchTimestampCoverageV1,
    unavailable: [NativeRuntimeDispatchTimestampUnavailableFactV1; 4],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeRuntimeDispatchTimestampCaptureWireV1 {
    schema: String,
    schema_version: u16,
    runtime_profile: ProfileContentIdentityV1,
    runtime_capture_scope: ProfileIdentityV1,
    recorder_occurrence: ProfileIdentityV1,
    host_clock: NativeRuntimeHostClockV1,
    #[serde(deserialize_with = "deserialize_bounded_records")]
    records: Vec<NativeRuntimeDispatchTimestampRecordV1>,
    coverage: NativeRuntimeDispatchTimestampCoverageV1,
    unavailable: [NativeRuntimeDispatchTimestampUnavailableFactV1; 4],
}

impl From<NativeRuntimeDispatchTimestampCaptureWireV1> for NativeRuntimeDispatchTimestampCaptureV1 {
    fn from(wire: NativeRuntimeDispatchTimestampCaptureWireV1) -> Self {
        Self {
            schema: wire.schema,
            schema_version: wire.schema_version,
            runtime_profile: wire.runtime_profile,
            runtime_capture_scope: wire.runtime_capture_scope,
            recorder_occurrence: wire.recorder_occurrence,
            host_clock: wire.host_clock,
            records: wire.records,
            coverage: wire.coverage,
            unavailable: wire.unavailable,
        }
    }
}

impl NativeRuntimeDispatchTimestampCaptureV1 {
    pub const fn runtime_profile(&self) -> ProfileContentIdentityV1 {
        self.runtime_profile
    }

    pub const fn runtime_capture_scope(&self) -> ProfileIdentityV1 {
        self.runtime_capture_scope
    }

    /// Fresh producer occurrence that separates process-local clock epochs.
    pub const fn recorder_occurrence(&self) -> ProfileIdentityV1 {
        self.recorder_occurrence
    }

    pub const fn host_clock(&self) -> NativeRuntimeHostClockV1 {
        self.host_clock
    }

    pub fn records(&self) -> &[NativeRuntimeDispatchTimestampRecordV1] {
        &self.records
    }

    pub const fn coverage(&self) -> NativeRuntimeDispatchTimestampCoverageV1 {
        self.coverage
    }

    pub const fn unavailable(&self) -> &[NativeRuntimeDispatchTimestampUnavailableFactV1; 4] {
        &self.unavailable
    }

    pub fn validate_against_runtime_profile_bytes(
        &self,
        runtime_profile_bytes: &[u8],
    ) -> Result<(), NativeRuntimeDispatchTimestampErrorV1> {
        let (runtime, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(runtime_profile_bytes)
                .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
        self.validate_against(&runtime, runtime_profile)
    }

    fn validate_against(
        &self,
        runtime: &KfdRuntimeProfileV1,
        runtime_profile: ProfileContentIdentityV1,
    ) -> Result<(), NativeRuntimeDispatchTimestampErrorV1> {
        if self.schema != NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_SCHEMA_V1
            || self.schema_version != NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_SCHEMA_VERSION_V1
        {
            return Err(NativeRuntimeDispatchTimestampErrorV1::UnsupportedVersion);
        }
        if self.runtime_profile != runtime_profile
            || self.runtime_capture_scope != runtime.capture_scope
        {
            return Err(NativeRuntimeDispatchTimestampErrorV1::StaleRuntimeProfile);
        }
        let expected_clock = derive_clock(
            runtime_profile,
            runtime.capture_scope,
            self.recorder_occurrence,
        )?;
        if self.host_clock != expected_clock {
            return Err(NativeRuntimeDispatchTimestampErrorV1::ClockDomainMismatch);
        }
        if self.records.len() > MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMP_RECORDS_V1 {
            return Err(NativeRuntimeDispatchTimestampErrorV1::RecordLimitExceeded);
        }
        if self.unavailable != NATIVE_RUNTIME_DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1 {
            return Err(NativeRuntimeDispatchTimestampErrorV1::InvalidCoverage);
        }
        let owners = runtime_dispatch_owners(runtime)?;
        let mut seen_dispatches = HashSet::new();
        seen_dispatches
            .try_reserve(self.records.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
        let mut prior_sequence = None;
        let mut completed = 0_u64;
        for record in &self.records {
            if !seen_dispatches.insert(record.dispatch) {
                return Err(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch);
            }
            if prior_sequence
                .is_some_and(|prior| prior >= record.publication.runtime_event_sequence)
            {
                return Err(NativeRuntimeDispatchTimestampErrorV1::NonCanonicalRecordOrder);
            }
            prior_sequence = Some(record.publication.runtime_event_sequence);
            let owner = owners
                .get(&record.dispatch)
                .ok_or(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch)?;
            if record.queue != owner.queue
                || record.device != runtime.device.identity
                || record.kernel != owner.kernel
                || record.artifact != owner.artifact
                || record.publication
                    != point(
                        NativeRuntimeHostTimestampStageV1::PublicationObserved,
                        owner.publication_event,
                        owner.publication_sequence,
                        self.host_clock.identity,
                        record.publication.nanoseconds_since_recorder_start,
                    )
                || record.identity != derive_record_identity(record)?
            {
                return Err(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch);
            }
            match (record.completion, owner.completion) {
                (None, _) => {}
                (Some(observed), Some((event, sequence)))
                    if observed
                        == point(
                            NativeRuntimeHostTimestampStageV1::CompletionObserved,
                            event,
                            sequence,
                            self.host_clock.identity,
                            observed.nanoseconds_since_recorder_start,
                        )
                        && observed.runtime_event_sequence
                            > record.publication.runtime_event_sequence
                        && observed.nanoseconds_since_recorder_start
                            >= record.publication.nanoseconds_since_recorder_start =>
                {
                    completed = completed
                        .checked_add(1)
                        .ok_or(NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
                }
                _ => return Err(NativeRuntimeDispatchTimestampErrorV1::CompletionMismatch),
            }
        }
        let runtime_dispatches = u64::try_from(owners.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
        let publications = u64::try_from(self.records.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
        let complete = runtime.coverage.complete_runtime_operation_history
            && self.coverage.dropped_observations == 0
            && publications == runtime_dispatches
            && completed == runtime_dispatches;
        if self.coverage.runtime_profile_dispatches != runtime_dispatches
            || self.coverage.observed_publications != publications
            || self.coverage.observed_completions != completed
            || self.coverage.complete_runtime_operation_history != complete
        {
            return Err(NativeRuntimeDispatchTimestampErrorV1::InvalidCoverage);
        }
        Ok(())
    }
}

/// Non-deserializable evidence that the typed recorder sampled and committed
/// every retained timestamp point in this capture.
///
/// This proves recorder custody, not that a particular runtime owned the
/// recorder. The native runtime wraps this output in its own private boundary.
#[derive(Debug)]
pub struct NativeRuntimeDispatchTimestampRecorderOutputV1 {
    capture: NativeRuntimeDispatchTimestampCaptureV1,
}

impl NativeRuntimeDispatchTimestampRecorderOutputV1 {
    pub const fn capture(&self) -> &NativeRuntimeDispatchTimestampCaptureV1 {
        &self.capture
    }
}

#[derive(Clone, Copy, Debug)]
enum SampleKindV1 {
    Publication { dispatch: ProfileIdentityV1 },
    Completion { dispatch: ProfileIdentityV1 },
}

/// Opaque timestamp sampled by [`NativeRuntimeDispatchTimestampRecorderV1`].
#[derive(Debug)]
pub struct NativeRuntimeDispatchTimestampSampleV1 {
    kind: SampleKindV1,
    nanoseconds_since_recorder_start: u64,
}

#[derive(Clone, Copy, Debug)]
struct RawRecordV1 {
    dispatch: ProfileIdentityV1,
    publication_event: ProfileIdentityV1,
    publication_sequence: u64,
    publication_ns: u64,
    completion: Option<(ProfileIdentityV1, u64, u64)>,
}

/// Typed producer that samples its own host-monotonic clock.
#[derive(Debug)]
pub struct NativeRuntimeDispatchTimestampRecorderV1 {
    started: Instant,
    capture_scope: ProfileIdentityV1,
    recorder_occurrence: ProfileIdentityV1,
    max_records: usize,
    records: Vec<RawRecordV1>,
    positions: HashMap<ProfileIdentityV1, usize>,
    dropped_observations: u64,
    invalid: bool,
}

impl NativeRuntimeDispatchTimestampRecorderV1 {
    pub fn new(
        capture_scope: ProfileIdentityV1,
        max_records: usize,
    ) -> Result<Self, NativeRuntimeDispatchTimestampErrorV1> {
        if max_records == 0 || max_records > MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMP_RECORDS_V1 {
            return Err(NativeRuntimeDispatchTimestampErrorV1::RecordLimitExceeded);
        }
        let mut records = Vec::new();
        records
            .try_reserve_exact(max_records)
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
        let mut positions = HashMap::new();
        positions
            .try_reserve(max_records)
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
        let recorder_occurrence = fresh_recorder_occurrence()?;
        Ok(Self {
            started: Instant::now(),
            capture_scope,
            recorder_occurrence,
            max_records,
            records,
            positions,
            dropped_observations: 0,
            invalid: false,
        })
    }

    /// Samples only runtime events that delimit a host-observed dispatch.
    pub fn sample(
        &self,
        event: &KfdRuntimeProfileEventKindV1,
    ) -> Option<NativeRuntimeDispatchTimestampSampleV1> {
        let kind = match event {
            KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch, .. } => {
                SampleKindV1::Publication {
                    dispatch: *dispatch,
                }
            }
            KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                SampleKindV1::Completion {
                    dispatch: *dispatch,
                }
            }
            _ => return None,
        };
        Some(NativeRuntimeDispatchTimestampSampleV1 {
            kind,
            nanoseconds_since_recorder_start: u64::try_from(self.started.elapsed().as_nanos())
                .unwrap_or(u64::MAX),
        })
    }

    /// Commits a sample only against the exact retained runtime event.
    pub fn commit(
        &mut self,
        sample: NativeRuntimeDispatchTimestampSampleV1,
        retained: &KfdRuntimeProfileEventV1,
    ) {
        let matches = match (&sample.kind, &retained.event) {
            (
                SampleKindV1::Publication { dispatch: sampled },
                KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch, .. },
            )
            | (
                SampleKindV1::Completion { dispatch: sampled },
                KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. },
            ) => sampled == dispatch,
            _ => false,
        };
        if !matches {
            self.invalid = true;
            return;
        }
        match sample.kind {
            SampleKindV1::Publication { dispatch } => {
                if self.records.len() == self.max_records || self.positions.contains_key(&dispatch)
                {
                    self.invalid = true;
                    return;
                }
                let index = self.records.len();
                self.records.push(RawRecordV1 {
                    dispatch,
                    publication_event: retained.identity,
                    publication_sequence: retained.sequence,
                    publication_ns: sample.nanoseconds_since_recorder_start,
                    completion: None,
                });
                if self.positions.insert(dispatch, index).is_some() {
                    self.invalid = true;
                }
            }
            SampleKindV1::Completion { dispatch } => {
                let Some(index) = self.positions.get(&dispatch).copied() else {
                    self.invalid = true;
                    return;
                };
                let record = &mut self.records[index];
                if record.completion.is_some()
                    || sample.nanoseconds_since_recorder_start < record.publication_ns
                {
                    self.invalid = true;
                    return;
                }
                record.completion = Some((
                    retained.identity,
                    retained.sequence,
                    sample.nanoseconds_since_recorder_start,
                ));
            }
        }
    }

    /// Accounts for a sampled dispatch delimiter that the runtime profile did
    /// not retain. The sample is consumed and its tick is never published.
    pub fn discard(&mut self, _sample: NativeRuntimeDispatchTimestampSampleV1) {
        self.dropped_observations = self.dropped_observations.saturating_add(1);
    }

    pub fn finish(
        self,
        runtime: &KfdRuntimeProfileV1,
    ) -> Result<NativeRuntimeDispatchTimestampRecorderOutputV1, NativeRuntimeDispatchTimestampErrorV1>
    {
        if self.invalid || self.capture_scope != runtime.capture_scope {
            return Err(NativeRuntimeDispatchTimestampErrorV1::RecorderStateMismatch);
        }
        let runtime_bytes = encode_kfd_runtime_profile_v1(runtime)
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
        let (_, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(&runtime_bytes)
                .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
        let owners = runtime_dispatch_owners(runtime)?;
        let host_clock = derive_clock(
            runtime_profile,
            runtime.capture_scope,
            self.recorder_occurrence,
        )?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(self.records.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
        for raw in self.records {
            let owner = owners
                .get(&raw.dispatch)
                .ok_or(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch)?;
            if owner.publication_event != raw.publication_event
                || owner.publication_sequence != raw.publication_sequence
            {
                return Err(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch);
            }
            let completion = match (raw.completion, owner.completion) {
                (None, _) => None,
                (Some((event, sequence, ticks)), Some((owner_event, owner_sequence)))
                    if event == owner_event && sequence == owner_sequence =>
                {
                    Some(point(
                        NativeRuntimeHostTimestampStageV1::CompletionObserved,
                        event,
                        sequence,
                        host_clock.identity,
                        ticks,
                    ))
                }
                _ => return Err(NativeRuntimeDispatchTimestampErrorV1::CompletionMismatch),
            };
            let mut record = NativeRuntimeDispatchTimestampRecordV1 {
                identity: ProfileIdentityV1::new([1; 32])
                    .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::Identity)?,
                dispatch: raw.dispatch,
                queue: owner.queue,
                device: runtime.device.identity,
                kernel: owner.kernel,
                artifact: owner.artifact,
                publication: point(
                    NativeRuntimeHostTimestampStageV1::PublicationObserved,
                    raw.publication_event,
                    raw.publication_sequence,
                    host_clock.identity,
                    raw.publication_ns,
                ),
                completion,
            };
            record.identity = derive_record_identity(&record)?;
            records.push(record);
        }
        let runtime_dispatches = u64::try_from(owners.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
        let observed_publications = u64::try_from(records.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
        let observed_completions = u64::try_from(
            records
                .iter()
                .filter(|record| record.completion.is_some())
                .count(),
        )
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
        let complete = runtime.coverage.complete_runtime_operation_history
            && self.dropped_observations == 0
            && observed_publications == runtime_dispatches
            && observed_completions == runtime_dispatches;
        let capture = NativeRuntimeDispatchTimestampCaptureV1 {
            schema: NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_SCHEMA_V1.to_owned(),
            schema_version: NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_SCHEMA_VERSION_V1,
            runtime_profile,
            runtime_capture_scope: runtime.capture_scope,
            recorder_occurrence: self.recorder_occurrence,
            host_clock,
            records,
            coverage: NativeRuntimeDispatchTimestampCoverageV1 {
                runtime_profile_dispatches: runtime_dispatches,
                observed_publications,
                observed_completions,
                dropped_observations: self.dropped_observations,
                complete_runtime_operation_history: complete,
            },
            unavailable: NATIVE_RUNTIME_DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1,
        };
        capture.validate_against(runtime, runtime_profile)?;
        Ok(NativeRuntimeDispatchTimestampRecorderOutputV1 { capture })
    }
}

#[derive(Clone, Copy)]
struct RuntimeDispatchOwnerV1 {
    queue: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    publication_event: ProfileIdentityV1,
    publication_sequence: u64,
    completion: Option<(ProfileIdentityV1, u64)>,
}

fn runtime_dispatch_owners(
    runtime: &KfdRuntimeProfileV1,
) -> Result<HashMap<ProfileIdentityV1, RuntimeDispatchOwnerV1>, NativeRuntimeDispatchTimestampErrorV1>
{
    let mut modules = HashMap::new();
    let mut kernels = HashMap::new();
    let mut dispatches = HashMap::new();
    modules
        .try_reserve(runtime.events.len())
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
    kernels
        .try_reserve(runtime.events.len())
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
    dispatches
        .try_reserve(runtime.events.len())
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::AllocationFailure)?;
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
                    .ok_or(NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
                let artifact = modules
                    .get(module)
                    .ok_or(NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
                if dispatches
                    .insert(
                        *dispatch,
                        RuntimeDispatchOwnerV1 {
                            queue: *queue,
                            kernel: *kernel,
                            artifact: *artifact,
                            publication_event: event.identity,
                            publication_sequence: event.sequence,
                            completion: None,
                        },
                    )
                    .is_some()
                {
                    return Err(NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected);
                }
            }
            KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                let owner = dispatches
                    .get_mut(dispatch)
                    .ok_or(NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
                if owner
                    .completion
                    .replace((event.identity, event.sequence))
                    .is_some()
                {
                    return Err(NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected);
                }
            }
            _ => {}
        }
    }
    Ok(dispatches)
}

const fn point(
    stage: NativeRuntimeHostTimestampStageV1,
    runtime_event: ProfileIdentityV1,
    runtime_event_sequence: u64,
    clock_domain: ProfileIdentityV1,
    nanoseconds_since_recorder_start: u64,
) -> NativeRuntimeHostTimestampPointV1 {
    NativeRuntimeHostTimestampPointV1 {
        stage,
        runtime_event,
        runtime_event_sequence,
        clock_domain,
        nanoseconds_since_recorder_start,
    }
}

fn derive_clock(
    runtime_profile: ProfileContentIdentityV1,
    capture_scope: ProfileIdentityV1,
    recorder_occurrence: ProfileIdentityV1,
) -> Result<NativeRuntimeHostClockV1, NativeRuntimeDispatchTimestampErrorV1> {
    Ok(NativeRuntimeHostClockV1 {
        identity: identity(
            CLOCK_IDENTITY_DOMAIN_V1,
            &[
                &runtime_profile.digest.as_bytes(),
                &runtime_profile.byte_len.to_le_bytes(),
                &capture_scope.as_bytes(),
                &recorder_occurrence.as_bytes(),
            ],
        )?,
        unit: NativeRuntimeHostClockUnitV1::NanosecondsSinceRecorderStart,
    })
}

fn fresh_recorder_occurrence() -> Result<ProfileIdentityV1, NativeRuntimeDispatchTimestampErrorV1> {
    const MAX_ATTEMPTS: usize = 4;
    for _ in 0..MAX_ATTEMPTS {
        let mut bytes = [0_u8; 32];
        let mut filled = 0;
        while filled < bytes.len() {
            let count = rustix::rand::getrandom(
                &mut bytes[filled..],
                rustix::rand::GetRandomFlags::empty(),
            )
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::EntropyUnavailable)?;
            if count == 0 {
                return Err(NativeRuntimeDispatchTimestampErrorV1::EntropyUnavailable);
            }
            filled += count;
        }
        if let Ok(identity) = ProfileIdentityV1::new(bytes) {
            return Ok(identity);
        }
    }
    Err(NativeRuntimeDispatchTimestampErrorV1::EntropyUnavailable)
}

#[derive(Serialize)]
struct RecordIdentityPreimageV1 {
    dispatch: ProfileIdentityV1,
    queue: ProfileIdentityV1,
    device: ProfileIdentityV1,
    kernel: ProfileIdentityV1,
    artifact: ProfileContentIdentityV1,
    publication: NativeRuntimeHostTimestampPointV1,
    completion: Option<NativeRuntimeHostTimestampPointV1>,
}

fn derive_record_identity(
    record: &NativeRuntimeDispatchTimestampRecordV1,
) -> Result<ProfileIdentityV1, NativeRuntimeDispatchTimestampErrorV1> {
    let bytes = serde_json::to_vec(&RecordIdentityPreimageV1 {
        dispatch: record.dispatch,
        queue: record.queue,
        device: record.device,
        kernel: record.kernel,
        artifact: record.artifact,
        publication: record.publication,
        completion: record.completion,
    })
    .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::JsonEncode)?;
    identity(RECORD_IDENTITY_DOMAIN_V1, &[&bytes])
}

fn identity(
    domain: &[u8],
    parts: &[&[u8]],
) -> Result<ProfileIdentityV1, NativeRuntimeDispatchTimestampErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    ProfileIdentityV1::new(hasher.finalize().into())
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::Identity)
}

fn deserialize_bounded_records<'de, D>(
    deserializer: D,
) -> Result<Vec<NativeRuntimeDispatchTimestampRecordV1>, D::Error>
where
    D: Deserializer<'de>,
{
    struct RecordsVisitorV1;

    impl<'de> Visitor<'de> for RecordsVisitorV1 {
        type Value = Vec<NativeRuntimeDispatchTimestampRecordV1>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMP_RECORDS_V1} native runtime timestamp records"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let capacity = sequence
                .size_hint()
                .unwrap_or(0)
                .min(MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMP_RECORDS_V1);
            let mut records = Vec::new();
            records.try_reserve_exact(capacity).map_err(|_| {
                serde::de::Error::custom("native runtime timestamp allocation failed")
            })?;
            while let Some(record) = sequence.next_element()? {
                if records.len() == MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMP_RECORDS_V1 {
                    return Err(serde::de::Error::custom(
                        "native runtime timestamp record limit exceeded",
                    ));
                }
                records.push(record);
            }
            Ok(records)
        }
    }

    deserializer.deserialize_seq(RecordsVisitorV1)
}

pub fn encode_native_runtime_dispatch_timestamp_capture_v1(
    capture: &NativeRuntimeDispatchTimestampCaptureV1,
    runtime_profile_bytes: &[u8],
) -> Result<Vec<u8>, NativeRuntimeDispatchTimestampErrorV1> {
    capture.validate_against_runtime_profile_bytes(runtime_profile_bytes)?;
    let bytes = serde_json::to_vec(capture)
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::JsonEncode)?;
    validate_capture_size(bytes.len())?;
    Ok(bytes)
}

/// Decodes canonical bytes and validates exact runtime currentness. This does
/// not recreate [`NativeRuntimeDispatchTimestampRecorderOutputV1`].
pub fn decode_native_runtime_dispatch_timestamp_capture_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
) -> Result<NativeRuntimeDispatchTimestampCaptureV1, NativeRuntimeDispatchTimestampErrorV1> {
    validate_capture_size(bytes.len())?;
    let (runtime, runtime_profile) =
        decode_kfd_runtime_profile_with_content_identity_v1(runtime_profile_bytes)
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::RuntimeProfileRejected)?;
    let capture = serde_json::from_slice::<NativeRuntimeDispatchTimestampCaptureWireV1>(bytes)
        .map(NativeRuntimeDispatchTimestampCaptureV1::from)
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::JsonDecode)?;
    capture.validate_against(&runtime, runtime_profile)?;
    if serde_json::to_vec(&capture)
        .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::JsonEncode)?
        != bytes
    {
        return Err(NativeRuntimeDispatchTimestampErrorV1::NonCanonicalEncoding);
    }
    Ok(capture)
}

pub fn native_runtime_dispatch_timestamp_capture_content_identity_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
) -> Result<ProfileContentIdentityV1, NativeRuntimeDispatchTimestampErrorV1> {
    decode_native_runtime_dispatch_timestamp_capture_v1(bytes, runtime_profile_bytes)?;
    Ok(ProfileContentIdentityV1 {
        digest: identity(CAPTURE_IDENTITY_DOMAIN_V1, &[bytes])?,
        byte_len: u64::try_from(bytes.len())
            .map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?,
    })
}

fn validate_capture_size(len: usize) -> Result<(), NativeRuntimeDispatchTimestampErrorV1> {
    let actual =
        u64::try_from(len).map_err(|_| NativeRuntimeDispatchTimestampErrorV1::SizeOverflow)?;
    if actual == 0 || actual > MAX_NATIVE_RUNTIME_DISPATCH_TIMESTAMPS_BYTES_V1 {
        return Err(NativeRuntimeDispatchTimestampErrorV1::InputSizeOutOfRange);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeRuntimeDispatchTimestampErrorV1 {
    UnsupportedVersion,
    InputSizeOutOfRange,
    RecordLimitExceeded,
    AllocationFailure,
    EntropyUnavailable,
    SizeOverflow,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
    Identity,
    RuntimeProfileRejected,
    StaleRuntimeProfile,
    RecorderStateMismatch,
    ClockDomainMismatch,
    DispatchIdentityMismatch,
    CompletionMismatch,
    NonCanonicalRecordOrder,
    InvalidCoverage,
}

impl fmt::Display for NativeRuntimeDispatchTimestampErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "native runtime dispatch timestamps rejected: {self:?}"
        )
    }
}

impl Error for NativeRuntimeDispatchTimestampErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KfdProfileDeviceV1, KfdProfileHostContentModeV1, KfdProfileHostTimingV1,
        KfdProfileLaunchV1, KfdProfileResourceKindV1, push_observed_event_v1, resource_identity_v1,
    };

    struct FixtureV1 {
        runtime: KfdRuntimeProfileV1,
        timestamps: NativeRuntimeDispatchTimestampRecorderOutputV1,
        dispatch: ProfileIdentityV1,
    }

    fn fixture(dropped_completion: bool) -> FixtureV1 {
        let scope = ProfileIdentityV1::new([41; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 5).unwrap();
        let mut recorder = NativeRuntimeDispatchTimestampRecorderV1::new(scope, 16).unwrap();
        let mut events = Vec::new();
        let artifact = ProfileContentIdentityV1::observed(b"native-code-object").unwrap();
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
        let publication = KfdRuntimeProfileEventKindV1::DispatchPublished {
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
        };
        let sample = recorder.sample(&publication).unwrap();
        push_observed_event_v1(scope, &mut events, publication).unwrap();
        recorder.commit(sample, events.last().unwrap());

        let completion = KfdRuntimeProfileEventKindV1::DispatchCompleted {
            dispatch,
            host_timing: KfdProfileHostTimingV1::default(),
        };
        let sample = recorder.sample(&completion).unwrap();
        push_observed_event_v1(scope, &mut events, completion).unwrap();
        if dropped_completion {
            recorder.discard(sample);
        } else {
            recorder.commit(sample, events.last().unwrap());
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
            u64::from(dropped_completion),
        )
        .unwrap();
        let timestamps = recorder.finish(&runtime).unwrap();
        FixtureV1 {
            runtime,
            timestamps,
            dispatch,
        }
    }

    #[test]
    fn typed_recorder_binds_host_points_to_exact_runtime_events() {
        let fixture = fixture(false);
        let capture = fixture.timestamps.capture();
        let record = &capture.records()[0];
        assert_eq!(record.dispatch(), fixture.dispatch);
        assert_eq!(record.publication().runtime_event_sequence, 4);
        assert_eq!(record.completion().unwrap().runtime_event_sequence, 5);
        assert!(
            record
                .completion()
                .unwrap()
                .nanoseconds_since_recorder_start
                >= record.publication().nanoseconds_since_recorder_start
        );
        assert_eq!(
            capture.coverage(),
            NativeRuntimeDispatchTimestampCoverageV1 {
                runtime_profile_dispatches: 1,
                observed_publications: 1,
                observed_completions: 1,
                dropped_observations: 0,
                complete_runtime_operation_history: true,
            }
        );
        assert_eq!(
            capture.unavailable(),
            &NATIVE_RUNTIME_DISPATCH_TIMESTAMP_UNAVAILABLE_FACTS_V1
        );

        let runtime_bytes = encode_kfd_runtime_profile_v1(&fixture.runtime).unwrap();
        let timestamp_bytes =
            encode_native_runtime_dispatch_timestamp_capture_v1(capture, &runtime_bytes).unwrap();
        let decoded =
            decode_native_runtime_dispatch_timestamp_capture_v1(&timestamp_bytes, &runtime_bytes)
                .unwrap();
        assert_eq!(&decoded, capture);
        assert_eq!(
            native_runtime_dispatch_timestamp_capture_content_identity_v1(
                &timestamp_bytes,
                &runtime_bytes,
            )
            .unwrap()
            .byte_len,
            timestamp_bytes.len() as u64
        );
    }

    #[test]
    fn distinct_recorders_never_alias_process_local_clock_epochs() {
        let scope = ProfileIdentityV1::new([42; 32]).unwrap();
        let runtime = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            Vec::new(),
            0,
        )
        .unwrap();
        let first = NativeRuntimeDispatchTimestampRecorderV1::new(scope, 1)
            .unwrap()
            .finish(&runtime)
            .unwrap();
        let second = NativeRuntimeDispatchTimestampRecorderV1::new(scope, 1)
            .unwrap()
            .finish(&runtime)
            .unwrap();
        assert_ne!(
            first.capture().recorder_occurrence(),
            second.capture().recorder_occurrence()
        );
        assert_ne!(
            first.capture().host_clock().identity,
            second.capture().host_clock().identity
        );
    }

    #[test]
    fn discarded_delimiter_is_explicit_and_never_inferred() {
        let fixture = fixture(true);
        let capture = fixture.timestamps.capture();
        assert_eq!(capture.records()[0].completion(), None);
        assert_eq!(capture.coverage().dropped_observations, 1);
        assert!(!capture.coverage().complete_runtime_operation_history);
    }

    #[test]
    fn capture_is_current_to_exact_runtime_profile_bytes() {
        let fixture = fixture(false);
        let runtime_bytes = encode_kfd_runtime_profile_v1(&fixture.runtime).unwrap();
        let timestamp_bytes = encode_native_runtime_dispatch_timestamp_capture_v1(
            fixture.timestamps.capture(),
            &runtime_bytes,
        )
        .unwrap();
        let mut stale_runtime = fixture.runtime;
        stale_runtime.coverage.dropped_events = 1;
        stale_runtime.coverage.complete_runtime_operation_history = false;
        let stale_bytes = encode_kfd_runtime_profile_v1(&stale_runtime).unwrap();
        assert_eq!(
            decode_native_runtime_dispatch_timestamp_capture_v1(&timestamp_bytes, &stale_bytes,),
            Err(NativeRuntimeDispatchTimestampErrorV1::StaleRuntimeProfile)
        );
    }

    #[test]
    fn rehashed_timestamp_event_substitution_still_rejects() {
        let fixture = fixture(false);
        let runtime_bytes = encode_kfd_runtime_profile_v1(&fixture.runtime).unwrap();
        let mut substituted = fixture.timestamps.capture().clone();
        substituted.records[0].publication.runtime_event =
            ProfileIdentityV1::new([99; 32]).unwrap();
        substituted.records[0].identity = derive_record_identity(&substituted.records[0]).unwrap();
        let bytes = serde_json::to_vec(&substituted).unwrap();
        assert_eq!(
            decode_native_runtime_dispatch_timestamp_capture_v1(&bytes, &runtime_bytes),
            Err(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch)
        );
    }

    #[test]
    fn raw_decoder_rejects_duplicate_dispatch_and_noncanonical_bytes() {
        let fixture = fixture(false);
        let runtime_bytes = encode_kfd_runtime_profile_v1(&fixture.runtime).unwrap();
        let canonical = encode_native_runtime_dispatch_timestamp_capture_v1(
            fixture.timestamps.capture(),
            &runtime_bytes,
        )
        .unwrap();
        let mut prefixed = Vec::with_capacity(canonical.len() + 1);
        prefixed.push(b' ');
        prefixed.extend_from_slice(&canonical);
        assert_eq!(
            decode_native_runtime_dispatch_timestamp_capture_v1(&prefixed, &runtime_bytes),
            Err(NativeRuntimeDispatchTimestampErrorV1::NonCanonicalEncoding)
        );

        let mut duplicated = fixture.timestamps.capture().clone();
        duplicated.records.push(duplicated.records[0].clone());
        duplicated.coverage.observed_publications = 2;
        duplicated.coverage.observed_completions = 2;
        let bytes = serde_json::to_vec(&duplicated).unwrap();
        assert_eq!(
            decode_native_runtime_dispatch_timestamp_capture_v1(&bytes, &runtime_bytes),
            Err(NativeRuntimeDispatchTimestampErrorV1::DispatchIdentityMismatch)
        );
    }

    #[test]
    fn mismatched_sample_and_retained_event_poison_the_recorder() {
        let scope = ProfileIdentityV1::new([42; 32]).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 1).unwrap();
        let other = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 2).unwrap();
        let mut recorder = NativeRuntimeDispatchTimestampRecorderV1::new(scope, 1).unwrap();
        let sampled = KfdRuntimeProfileEventKindV1::DispatchCompleted {
            dispatch,
            host_timing: KfdProfileHostTimingV1::default(),
        };
        let retained_kind = KfdRuntimeProfileEventKindV1::DispatchCompleted {
            dispatch: other,
            host_timing: KfdProfileHostTimingV1::default(),
        };
        let sample = recorder.sample(&sampled).unwrap();
        let mut events = Vec::new();
        push_observed_event_v1(scope, &mut events, retained_kind).unwrap();
        recorder.commit(sample, events.last().unwrap());
        let runtime = KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            Vec::new(),
            0,
        )
        .unwrap();
        assert!(matches!(
            recorder.finish(&runtime),
            Err(NativeRuntimeDispatchTimestampErrorV1::RecorderStateMismatch)
        ));
    }
}

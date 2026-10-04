//! Canonical typed semantic sidecar for frozen KFD Runtime Profile V1.
//!
//! The sidecar does not modify the frozen V1 event wire. It classifies every
//! retained dispatch publication exactly once, binds the exact runtime-profile
//! and publication identities, and remains authority-free when decoded alone.

use std::error::Error;
use std::fmt;

use serde::de::{SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    KfdProfileLaunchV1, KfdProfileSemanticContractV1, KfdRuntimeProfileEventKindV1,
    KfdRuntimeProfileV1, ProfileContentIdentityV1, ProfileIdentityV1,
    decode_kfd_runtime_profile_with_content_identity_v1, encode_kfd_runtime_profile_v1,
};

pub const KFD_RUNTIME_SEMANTIC_PROFILE_SCHEMA_V1: &str = "fe2o3-kfd-runtime-semantic-profile-v1";
pub const KFD_RUNTIME_SEMANTIC_PROFILE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_KFD_RUNTIME_SEMANTIC_PROFILE_RECORDS_V1: usize =
    crate::MAX_KFD_RUNTIME_PROFILE_EVENTS_V1 as usize;
pub const MAX_KFD_RUNTIME_SEMANTIC_PROFILE_BYTES_V1: u64 = 16 * 1024 * 1024;

const RECORD_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime.semantic-record.v1\0";
const CAPTURE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime.semantic-profile.v1\0";

/// Producer input for one retained V1 dispatch publication.
///
/// The observations must be in exact runtime publication order and cover
/// every retained publication. `None` explicitly classifies an ordinary
/// launch; it is not a missing value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdRuntimeSemanticObservationV1 {
    pub dispatch: ProfileIdentityV1,
    pub semantic_contract: Option<KfdProfileSemanticContractV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdRuntimeSemanticRecordV1 {
    identity: ProfileIdentityV1,
    runtime_event: ProfileIdentityV1,
    runtime_event_sequence: u64,
    dispatch: ProfileIdentityV1,
    dispatch_shape: ProfileContentIdentityV1,
    launch: KfdProfileLaunchV1,
    semantic_contract: Option<KfdProfileSemanticContractV1>,
}

impl KfdRuntimeSemanticRecordV1 {
    pub const fn identity(&self) -> ProfileIdentityV1 {
        self.identity
    }

    pub const fn runtime_event(&self) -> ProfileIdentityV1 {
        self.runtime_event
    }

    pub const fn runtime_event_sequence(&self) -> u64 {
        self.runtime_event_sequence
    }

    pub const fn dispatch(&self) -> ProfileIdentityV1 {
        self.dispatch
    }

    pub const fn dispatch_shape(&self) -> ProfileContentIdentityV1 {
        self.dispatch_shape
    }

    pub const fn launch(&self) -> KfdProfileLaunchV1 {
        self.launch
    }

    pub const fn semantic_contract(&self) -> Option<KfdProfileSemanticContractV1> {
        self.semantic_contract
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdRuntimeSemanticCoverageV1 {
    pub runtime_profile_dispatches: u64,
    pub typed_semantic_contracts: u64,
    pub ordinary_dispatches: u64,
    pub complete_retained_dispatch_classification: bool,
    pub complete_runtime_operation_history: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdRuntimeSemanticProfileV1 {
    schema: String,
    schema_version: u16,
    runtime_profile: ProfileContentIdentityV1,
    runtime_capture_scope: ProfileIdentityV1,
    records: Vec<KfdRuntimeSemanticRecordV1>,
    coverage: KfdRuntimeSemanticCoverageV1,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct KfdRuntimeSemanticProfileWireV1 {
    schema: String,
    schema_version: u16,
    runtime_profile: ProfileContentIdentityV1,
    runtime_capture_scope: ProfileIdentityV1,
    #[serde(deserialize_with = "deserialize_bounded_records")]
    records: Vec<KfdRuntimeSemanticRecordV1>,
    coverage: KfdRuntimeSemanticCoverageV1,
}

impl From<KfdRuntimeSemanticProfileWireV1> for KfdRuntimeSemanticProfileV1 {
    fn from(wire: KfdRuntimeSemanticProfileWireV1) -> Self {
        Self {
            schema: wire.schema,
            schema_version: wire.schema_version,
            runtime_profile: wire.runtime_profile,
            runtime_capture_scope: wire.runtime_capture_scope,
            records: wire.records,
            coverage: wire.coverage,
        }
    }
}

impl KfdRuntimeSemanticProfileV1 {
    pub fn new(
        runtime: &KfdRuntimeProfileV1,
        observations: Vec<KfdRuntimeSemanticObservationV1>,
    ) -> Result<Self, KfdRuntimeSemanticProfileErrorV1> {
        let runtime_bytes = encode_kfd_runtime_profile_v1(runtime)
            .map_err(|_| KfdRuntimeSemanticProfileErrorV1::RuntimeProfileRejected)?;
        let (_, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(&runtime_bytes)
                .map_err(|_| KfdRuntimeSemanticProfileErrorV1::RuntimeProfileRejected)?;
        let dispatch_count = runtime
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                )
            })
            .count();
        if dispatch_count > MAX_KFD_RUNTIME_SEMANTIC_PROFILE_RECORDS_V1
            || observations.len() != dispatch_count
        {
            return Err(KfdRuntimeSemanticProfileErrorV1::IncompleteDispatchJoin);
        }

        let mut records = Vec::new();
        records
            .try_reserve_exact(dispatch_count)
            .map_err(|_| KfdRuntimeSemanticProfileErrorV1::AllocationFailure)?;
        let mut observations = observations.into_iter();
        for event in &runtime.events {
            let KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                dispatch_shape,
                launch,
                ..
            } = &event.event
            else {
                continue;
            };
            let observation = observations
                .next()
                .ok_or(KfdRuntimeSemanticProfileErrorV1::IncompleteDispatchJoin)?;
            if observation.dispatch != *dispatch {
                return Err(KfdRuntimeSemanticProfileErrorV1::InvalidDispatchJoin);
            }
            if observation
                .semantic_contract
                .is_some_and(|contract| !contract.is_valid_for_launch(*launch))
            {
                return Err(KfdRuntimeSemanticProfileErrorV1::InvalidSemanticContract);
            }
            let identity = derive_record_identity_fields_v1(
                runtime_profile,
                event.identity,
                event.sequence,
                *dispatch,
                *dispatch_shape,
                *launch,
                observation.semantic_contract,
            )?;
            records.push(KfdRuntimeSemanticRecordV1 {
                identity,
                runtime_event: event.identity,
                runtime_event_sequence: event.sequence,
                dispatch: *dispatch,
                dispatch_shape: *dispatch_shape,
                launch: *launch,
                semantic_contract: observation.semantic_contract,
            });
        }
        if observations.next().is_some() {
            return Err(KfdRuntimeSemanticProfileErrorV1::IncompleteDispatchJoin);
        }
        let typed_semantic_contracts = records
            .iter()
            .filter(|record| record.semantic_contract.is_some())
            .count() as u64;
        let runtime_profile_dispatches = records.len() as u64;
        let capture = Self {
            schema: KFD_RUNTIME_SEMANTIC_PROFILE_SCHEMA_V1.to_owned(),
            schema_version: KFD_RUNTIME_SEMANTIC_PROFILE_SCHEMA_VERSION_V1,
            runtime_profile,
            runtime_capture_scope: runtime.capture_scope,
            records,
            coverage: KfdRuntimeSemanticCoverageV1 {
                runtime_profile_dispatches,
                typed_semantic_contracts,
                ordinary_dispatches: runtime_profile_dispatches - typed_semantic_contracts,
                complete_retained_dispatch_classification: true,
                complete_runtime_operation_history: runtime
                    .coverage
                    .complete_runtime_operation_history,
            },
        };
        capture.validate_against(runtime, runtime_profile)?;
        Ok(capture)
    }

    pub const fn runtime_profile(&self) -> ProfileContentIdentityV1 {
        self.runtime_profile
    }

    pub const fn runtime_capture_scope(&self) -> ProfileIdentityV1 {
        self.runtime_capture_scope
    }

    pub fn records(&self) -> &[KfdRuntimeSemanticRecordV1] {
        &self.records
    }

    pub const fn coverage(&self) -> KfdRuntimeSemanticCoverageV1 {
        self.coverage
    }

    pub fn validate_against_runtime_profile_bytes(
        &self,
        runtime_profile_bytes: &[u8],
    ) -> Result<(), KfdRuntimeSemanticProfileErrorV1> {
        let (runtime, runtime_profile) =
            decode_kfd_runtime_profile_with_content_identity_v1(runtime_profile_bytes)
                .map_err(|_| KfdRuntimeSemanticProfileErrorV1::RuntimeProfileRejected)?;
        self.validate_against(&runtime, runtime_profile)
    }

    fn validate_against(
        &self,
        runtime: &KfdRuntimeProfileV1,
        runtime_profile: ProfileContentIdentityV1,
    ) -> Result<(), KfdRuntimeSemanticProfileErrorV1> {
        if self.schema != KFD_RUNTIME_SEMANTIC_PROFILE_SCHEMA_V1
            || self.schema_version != KFD_RUNTIME_SEMANTIC_PROFILE_SCHEMA_VERSION_V1
        {
            return Err(KfdRuntimeSemanticProfileErrorV1::UnsupportedVersion);
        }
        if self.runtime_profile != runtime_profile
            || self.runtime_capture_scope != runtime.capture_scope
        {
            return Err(KfdRuntimeSemanticProfileErrorV1::StaleRuntimeProfile);
        }
        if self.records.len() > MAX_KFD_RUNTIME_SEMANTIC_PROFILE_RECORDS_V1 {
            return Err(KfdRuntimeSemanticProfileErrorV1::RecordLimitExceeded);
        }
        let dispatches = runtime.events.iter().filter_map(|event| {
            let KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                dispatch_shape,
                launch,
                ..
            } = &event.event
            else {
                return None;
            };
            Some((event, *dispatch, *dispatch_shape, *launch))
        });
        let dispatch_count = runtime
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                )
            })
            .count();
        if self.records.len() != dispatch_count {
            return Err(KfdRuntimeSemanticProfileErrorV1::IncompleteDispatchJoin);
        }
        for (record, (event, dispatch, dispatch_shape, launch)) in
            self.records.iter().zip(dispatches)
        {
            if record
                .semantic_contract
                .is_some_and(|contract| !contract.is_valid_for_launch(launch))
            {
                return Err(KfdRuntimeSemanticProfileErrorV1::InvalidSemanticContract);
            }
            if record.runtime_event != event.identity
                || record.runtime_event_sequence != event.sequence
                || record.dispatch != dispatch
                || record.dispatch_shape != dispatch_shape
                || record.launch != launch
                || record.identity != derive_record_identity_v1(runtime_profile, record)?
            {
                return Err(KfdRuntimeSemanticProfileErrorV1::InvalidDispatchJoin);
            }
        }
        let typed = self
            .records
            .iter()
            .filter(|record| record.semantic_contract.is_some())
            .count() as u64;
        let dispatch_count = self.records.len() as u64;
        if self.coverage.runtime_profile_dispatches != dispatch_count
            || self.coverage.typed_semantic_contracts != typed
            || self.coverage.ordinary_dispatches != dispatch_count.saturating_sub(typed)
            || !self.coverage.complete_retained_dispatch_classification
            || self.coverage.complete_runtime_operation_history
                != runtime.coverage.complete_runtime_operation_history
        {
            return Err(KfdRuntimeSemanticProfileErrorV1::InvalidCoverage);
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct RecordIdentityPreimageV1 {
    runtime_profile: ProfileContentIdentityV1,
    runtime_event: ProfileIdentityV1,
    runtime_event_sequence: u64,
    dispatch: ProfileIdentityV1,
    dispatch_shape: ProfileContentIdentityV1,
    launch: KfdProfileLaunchV1,
    semantic_contract: Option<KfdProfileSemanticContractV1>,
}

fn derive_record_identity_v1(
    runtime_profile: ProfileContentIdentityV1,
    record: &KfdRuntimeSemanticRecordV1,
) -> Result<ProfileIdentityV1, KfdRuntimeSemanticProfileErrorV1> {
    derive_record_identity_fields_v1(
        runtime_profile,
        record.runtime_event,
        record.runtime_event_sequence,
        record.dispatch,
        record.dispatch_shape,
        record.launch,
        record.semantic_contract,
    )
}

fn derive_record_identity_fields_v1(
    runtime_profile: ProfileContentIdentityV1,
    runtime_event: ProfileIdentityV1,
    runtime_event_sequence: u64,
    dispatch: ProfileIdentityV1,
    dispatch_shape: ProfileContentIdentityV1,
    launch: KfdProfileLaunchV1,
    semantic_contract: Option<KfdProfileSemanticContractV1>,
) -> Result<ProfileIdentityV1, KfdRuntimeSemanticProfileErrorV1> {
    let bytes = serde_json::to_vec(&RecordIdentityPreimageV1 {
        runtime_profile,
        runtime_event,
        runtime_event_sequence,
        dispatch,
        dispatch_shape,
        launch,
        semantic_contract,
    })
    .map_err(|_| KfdRuntimeSemanticProfileErrorV1::JsonEncode)?;
    identity(RECORD_IDENTITY_DOMAIN_V1, &[&bytes])
}

pub fn encode_kfd_runtime_semantic_profile_v1(
    capture: &KfdRuntimeSemanticProfileV1,
    runtime_profile_bytes: &[u8],
) -> Result<Vec<u8>, KfdRuntimeSemanticProfileErrorV1> {
    capture.validate_against_runtime_profile_bytes(runtime_profile_bytes)?;
    let bytes =
        serde_json::to_vec(capture).map_err(|_| KfdRuntimeSemanticProfileErrorV1::JsonEncode)?;
    validate_size(bytes.len())?;
    Ok(bytes)
}

pub fn decode_kfd_runtime_semantic_profile_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
) -> Result<KfdRuntimeSemanticProfileV1, KfdRuntimeSemanticProfileErrorV1> {
    validate_size(bytes.len())?;
    let capture: KfdRuntimeSemanticProfileWireV1 =
        serde_json::from_slice(bytes).map_err(|_| KfdRuntimeSemanticProfileErrorV1::JsonDecode)?;
    let capture = KfdRuntimeSemanticProfileV1::from(capture);
    capture.validate_against_runtime_profile_bytes(runtime_profile_bytes)?;
    if serde_json::to_vec(&capture).map_err(|_| KfdRuntimeSemanticProfileErrorV1::JsonEncode)?
        != bytes
    {
        return Err(KfdRuntimeSemanticProfileErrorV1::NonCanonicalEncoding);
    }
    Ok(capture)
}

pub fn kfd_runtime_semantic_profile_content_identity_v1(
    bytes: &[u8],
    runtime_profile_bytes: &[u8],
) -> Result<ProfileContentIdentityV1, KfdRuntimeSemanticProfileErrorV1> {
    decode_kfd_runtime_semantic_profile_v1(bytes, runtime_profile_bytes)?;
    Ok(ProfileContentIdentityV1 {
        digest: identity(CAPTURE_IDENTITY_DOMAIN_V1, &[bytes])?,
        byte_len: u64::try_from(bytes.len())
            .map_err(|_| KfdRuntimeSemanticProfileErrorV1::SizeOverflow)?,
    })
}

fn validate_size(len: usize) -> Result<(), KfdRuntimeSemanticProfileErrorV1> {
    let actual = u64::try_from(len).map_err(|_| KfdRuntimeSemanticProfileErrorV1::SizeOverflow)?;
    if actual == 0 || actual > MAX_KFD_RUNTIME_SEMANTIC_PROFILE_BYTES_V1 {
        return Err(KfdRuntimeSemanticProfileErrorV1::CaptureSizeOutOfRange);
    }
    Ok(())
}

fn identity(
    domain: &[u8],
    parts: &[&[u8]],
) -> Result<ProfileIdentityV1, KfdRuntimeSemanticProfileErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    ProfileIdentityV1::new(hasher.finalize().into())
        .map_err(|_| KfdRuntimeSemanticProfileErrorV1::IdentityFailure)
}

fn deserialize_bounded_records<'de, D>(
    deserializer: D,
) -> Result<Vec<KfdRuntimeSemanticRecordV1>, D::Error>
where
    D: Deserializer<'de>,
{
    struct RecordsVisitorV1;

    impl<'de> Visitor<'de> for RecordsVisitorV1 {
        type Value = Vec<KfdRuntimeSemanticRecordV1>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(
                formatter,
                "at most {MAX_KFD_RUNTIME_SEMANTIC_PROFILE_RECORDS_V1} semantic profile records"
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let hinted = sequence
                .size_hint()
                .unwrap_or(0)
                .min(MAX_KFD_RUNTIME_SEMANTIC_PROFILE_RECORDS_V1);
            let mut records = Vec::new();
            records.try_reserve(hinted).map_err(|_| {
                serde::de::Error::custom("semantic profile record allocation failed")
            })?;
            while let Some(record) = sequence.next_element()? {
                if records.len() == MAX_KFD_RUNTIME_SEMANTIC_PROFILE_RECORDS_V1 {
                    return Err(serde::de::Error::custom(
                        "semantic profile record limit exceeded",
                    ));
                }
                records.push(record);
            }
            Ok(records)
        }
    }

    deserializer.deserialize_seq(RecordsVisitorV1)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdRuntimeSemanticProfileErrorV1 {
    RuntimeProfileRejected,
    UnsupportedVersion,
    StaleRuntimeProfile,
    RecordLimitExceeded,
    IncompleteDispatchJoin,
    InvalidDispatchJoin,
    InvalidSemanticContract,
    InvalidCoverage,
    CaptureSizeOutOfRange,
    SizeOverflow,
    AllocationFailure,
    IdentityFailure,
    JsonEncode,
    JsonDecode,
    NonCanonicalEncoding,
}

impl fmt::Display for KfdRuntimeSemanticProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "KFD runtime semantic profile rejected: {self:?}")
    }
}

impl Error for KfdRuntimeSemanticProfileErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        KfdProfileAtomicContractV1, KfdProfileAtomicOperationV1, KfdProfileCollectiveContractV1,
        KfdProfileCollectiveOperationV1, KfdProfileDeviceV1, KfdProfileHostContentModeV1,
        KfdProfileHostTimingV1, KfdProfileMemoryOrderV1, KfdProfileMemoryScopeV1,
        KfdProfileResourceKindV1, push_observed_event_v1, resource_identity_v1,
    };

    fn fixture() -> (KfdRuntimeProfileV1, ProfileIdentityV1, ProfileIdentityV1) {
        let scope = ProfileIdentityV1::new([61; 32]).unwrap();
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let atomic = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 5).unwrap();
        let collective =
            resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 6).unwrap();
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
        for dispatch in [atomic, collective] {
            push_observed_event_v1(
                scope,
                &mut events,
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch,
                    queue,
                    stream,
                    kernel,
                    dispatch_shape: ProfileContentIdentityV1::observed(&dispatch.as_bytes())
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
        }
        for event in [
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            push_observed_event_v1(scope, &mut events, event).unwrap();
        }
        (
            KfdRuntimeProfileV1::new(
                scope,
                KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
                KfdProfileHostContentModeV1::RangeOnly,
                events,
                0,
            )
            .unwrap(),
            atomic,
            collective,
        )
    }

    fn atomic_contract() -> KfdProfileSemanticContractV1 {
        KfdProfileSemanticContractV1::Atomic(KfdProfileAtomicContractV1 {
            operation: KfdProfileAtomicOperationV1::CompareExchange,
            scope: KfdProfileMemoryScopeV1::Device,
            order: KfdProfileMemoryOrderV1::AcquireRelease,
            failure_order: Some(KfdProfileMemoryOrderV1::Acquire),
            weak: true,
            geometry: KfdProfileLaunchV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
    }

    fn collective_contract() -> KfdProfileSemanticContractV1 {
        KfdProfileSemanticContractV1::Collective(KfdProfileCollectiveContractV1 {
            operation: KfdProfileCollectiveOperationV1::AllReduceSum,
            scope: KfdProfileMemoryScopeV1::Workgroup,
            order: KfdProfileMemoryOrderV1::AcquireRelease,
            participants: 64,
            geometry: KfdProfileLaunchV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
        })
    }

    #[test]
    fn sidecar_is_canonical_and_covers_every_frozen_v1_publication() {
        let (runtime, atomic, collective) = fixture();
        let runtime_bytes = encode_kfd_runtime_profile_v1(&runtime).unwrap();
        let capture = KfdRuntimeSemanticProfileV1::new(
            &runtime,
            vec![
                KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(atomic_contract()),
                },
                KfdRuntimeSemanticObservationV1 {
                    dispatch: collective,
                    semantic_contract: Some(collective_contract()),
                },
            ],
        )
        .unwrap();
        let bytes = encode_kfd_runtime_semantic_profile_v1(&capture, &runtime_bytes).unwrap();
        assert!(
            !runtime_bytes
                .windows(b"semantic_contract".len())
                .any(|window| window == b"semantic_contract")
        );
        assert_eq!(
            decode_kfd_runtime_semantic_profile_v1(&bytes, &runtime_bytes).unwrap(),
            capture
        );
        assert_eq!(capture.coverage().typed_semantic_contracts, 2);
        assert!(capture.coverage().complete_retained_dispatch_classification);
        assert_ne!(
            capture.records()[0].identity(),
            capture.records()[1].identity()
        );
    }

    #[test]
    fn missing_duplicate_reordered_and_invalid_contract_joins_fail_closed() {
        let (runtime, atomic, collective) = fixture();
        assert_eq!(
            KfdRuntimeSemanticProfileV1::new(
                &runtime,
                vec![KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(atomic_contract()),
                }],
            ),
            Err(KfdRuntimeSemanticProfileErrorV1::IncompleteDispatchJoin)
        );
        for observations in [
            vec![
                KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(atomic_contract()),
                },
                KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(collective_contract()),
                },
            ],
            vec![
                KfdRuntimeSemanticObservationV1 {
                    dispatch: collective,
                    semantic_contract: Some(collective_contract()),
                },
                KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(atomic_contract()),
                },
            ],
        ] {
            assert_eq!(
                KfdRuntimeSemanticProfileV1::new(&runtime, observations),
                Err(KfdRuntimeSemanticProfileErrorV1::InvalidDispatchJoin)
            );
        }

        let mut invalid = match atomic_contract() {
            KfdProfileSemanticContractV1::Atomic(contract) => contract,
            _ => unreachable!(),
        };
        invalid.geometry.grid[0] = 32;
        assert_eq!(
            KfdRuntimeSemanticProfileV1::new(
                &runtime,
                vec![
                    KfdRuntimeSemanticObservationV1 {
                        dispatch: atomic,
                        semantic_contract: Some(KfdProfileSemanticContractV1::Atomic(invalid)),
                    },
                    KfdRuntimeSemanticObservationV1 {
                        dispatch: collective,
                        semantic_contract: Some(collective_contract()),
                    },
                ],
            ),
            Err(KfdRuntimeSemanticProfileErrorV1::InvalidSemanticContract)
        );
    }

    #[test]
    fn semantic_contract_legality_rejects_scope_order_weak_and_participant_mismatches() {
        let launch = KfdProfileLaunchV1 {
            grid: [64, 1, 1],
            workgroup: [64, 1, 1],
            dynamic_shared_bytes: 0,
        };
        let valid_atomic = match atomic_contract() {
            KfdProfileSemanticContractV1::Atomic(contract) => contract,
            _ => unreachable!(),
        };
        for invalid in [
            KfdProfileAtomicContractV1 {
                scope: KfdProfileMemoryScopeV1::System,
                ..valid_atomic
            },
            KfdProfileAtomicContractV1 {
                failure_order: Some(KfdProfileMemoryOrderV1::Release),
                ..valid_atomic
            },
            KfdProfileAtomicContractV1 {
                operation: KfdProfileAtomicOperationV1::Add,
                failure_order: None,
                weak: true,
                ..valid_atomic
            },
        ] {
            assert!(!KfdProfileSemanticContractV1::Atomic(invalid).is_valid_for_launch(launch));
        }

        let valid_collective = match collective_contract() {
            KfdProfileSemanticContractV1::Collective(contract) => contract,
            _ => unreachable!(),
        };
        for invalid in [
            KfdProfileCollectiveContractV1 {
                scope: KfdProfileMemoryScopeV1::Device,
                ..valid_collective
            },
            KfdProfileCollectiveContractV1 {
                participants: 32,
                ..valid_collective
            },
        ] {
            assert!(!KfdProfileSemanticContractV1::Collective(invalid).is_valid_for_launch(launch));
        }
    }

    #[test]
    fn contract_substitution_breaks_record_identity_and_runtime_substitution_breaks_currentness() {
        let (runtime, atomic, collective) = fixture();
        let runtime_bytes = encode_kfd_runtime_profile_v1(&runtime).unwrap();
        let capture = KfdRuntimeSemanticProfileV1::new(
            &runtime,
            vec![
                KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(atomic_contract()),
                },
                KfdRuntimeSemanticObservationV1 {
                    dispatch: collective,
                    semantic_contract: Some(collective_contract()),
                },
            ],
        )
        .unwrap();
        let mut substituted = capture.clone();
        substituted.records[0].semantic_contract = None;
        assert_eq!(
            encode_kfd_runtime_semantic_profile_v1(&substituted, &runtime_bytes),
            Err(KfdRuntimeSemanticProfileErrorV1::InvalidDispatchJoin)
        );

        let mut stale_runtime = runtime;
        stale_runtime.coverage.dropped_events = 1;
        stale_runtime.coverage.complete_runtime_operation_history = false;
        let stale_runtime_bytes = encode_kfd_runtime_profile_v1(&stale_runtime).unwrap();
        let bytes = serde_json::to_vec(&capture).unwrap();
        assert_eq!(
            decode_kfd_runtime_semantic_profile_v1(&bytes, &stale_runtime_bytes),
            Err(KfdRuntimeSemanticProfileErrorV1::StaleRuntimeProfile)
        );
    }

    #[test]
    fn noncanonical_bytes_and_rehashed_structural_substitution_are_distinct() {
        let (runtime, atomic, collective) = fixture();
        let runtime_bytes = encode_kfd_runtime_profile_v1(&runtime).unwrap();
        let capture = KfdRuntimeSemanticProfileV1::new(
            &runtime,
            vec![
                KfdRuntimeSemanticObservationV1 {
                    dispatch: atomic,
                    semantic_contract: Some(atomic_contract()),
                },
                KfdRuntimeSemanticObservationV1 {
                    dispatch: collective,
                    semantic_contract: Some(collective_contract()),
                },
            ],
        )
        .unwrap();
        let canonical = encode_kfd_runtime_semantic_profile_v1(&capture, &runtime_bytes).unwrap();
        let mut prefixed = Vec::with_capacity(canonical.len() + 1);
        prefixed.push(b' ');
        prefixed.extend_from_slice(&canonical);
        assert_eq!(
            decode_kfd_runtime_semantic_profile_v1(&prefixed, &runtime_bytes),
            Err(KfdRuntimeSemanticProfileErrorV1::NonCanonicalEncoding)
        );

        let mut rehashed = capture;
        rehashed.records[0].semantic_contract = None;
        rehashed.coverage.typed_semantic_contracts -= 1;
        rehashed.coverage.ordinary_dispatches += 1;
        rehashed.records[0].identity =
            derive_record_identity_v1(rehashed.runtime_profile, &rehashed.records[0]).unwrap();
        let bytes = serde_json::to_vec(&rehashed).unwrap();
        let decoded = decode_kfd_runtime_semantic_profile_v1(&bytes, &runtime_bytes).unwrap();
        assert_eq!(decoded.records()[0].semantic_contract(), None);
        assert_ne!(
            kfd_runtime_semantic_profile_content_identity_v1(&canonical, &runtime_bytes).unwrap(),
            kfd_runtime_semantic_profile_content_identity_v1(&bytes, &runtime_bytes).unwrap()
        );
    }
}

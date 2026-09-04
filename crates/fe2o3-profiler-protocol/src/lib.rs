#![forbid(unsafe_code)]
//! Bounded, authority-free observations from the direct-KFD runtime.
//!
//! The records in this crate are descriptive. They cannot authorize loading,
//! dispatch, compilation, or proof. In particular, host monotonic durations
//! are not GPU timestamps and a missing collector fact is never synthesized.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

mod dispatch_timestamps_v1;
pub use dispatch_timestamps_v1::*;

pub const KFD_RUNTIME_PROFILE_SCHEMA_VERSION_V1: u16 = 1;
pub const KFD_RUNTIME_PROFILE_SCHEMA_V1: &str = "fe2o3-kfd-runtime-profile-v1";
pub const AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1: &str = "fe2o3-agent-kfd-profiler-request-v1";
pub const AGENT_KFD_PROFILER_RESPONSE_SCHEMA_V1: &str = "fe2o3-agent-kfd-profiler-response-v1";
pub const MAX_KFD_RUNTIME_PROFILE_BYTES_V1: u64 = 16 * 1024 * 1024;
/// Reserved for the fixed capture envelope, including maximal integer fields.
/// Event producers may use the remainder for encoded event objects and commas.
pub const MAX_KFD_RUNTIME_PROFILE_FIXED_JSON_BYTES_V1: u64 = 4 * 1024;
pub const MAX_KFD_RUNTIME_PROFILE_EVENTS_V1: u32 = 16_384;
pub const MAX_KFD_RUNTIME_PROFILE_BINDINGS_V1: usize = 64;
pub const MAX_AGENT_KFD_PROFILER_REQUEST_BYTES_V1: u64 = 64 * 1024;
pub const MAX_AGENT_KFD_PROFILER_RESPONSE_BYTES_V1: u64 = 2 * 1024 * 1024;
pub const MAX_AGENT_KFD_PROFILER_PAGE_ITEMS_V1: u16 = 4_096;
pub const MAX_KFD_PROFILE_TARGET_BYTES_V1: usize = 64;

const PROFILE_CONTENT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime-profile.content.v1\0";
const EVENT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime-profile.event.v1\0";
const RESOURCE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime-profile.resource.v1\0";
const DEVICE_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime-profile.device.v1\0";
const CONTENT_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime-profile.content-claim.v1\0";
const CURSOR_IDENTITY_DOMAIN_V1: &[u8] = b"fe2o3.kfd-runtime-profile.cursor.v1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProfileIdentityV1([u8; 32]);

impl ProfileIdentityV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, KfdRuntimeProfileErrorV1> {
        if bytes == [0; 32] {
            return Err(KfdRuntimeProfileErrorV1::ZeroIdentity);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl Serialize for ProfileIdentityV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut encoded = [0_u8; 64];
        for (index, byte) in self.0.iter().copied().enumerate() {
            encoded[index * 2] = hex_digit(byte >> 4);
            encoded[index * 2 + 1] = hex_digit(byte & 0x0f);
        }
        serializer.serialize_str(std::str::from_utf8(&encoded).expect("hex is ASCII"))
    }
}

impl<'de> Deserialize<'de> for ProfileIdentityV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdentityVisitorV1;
        impl Visitor<'_> for IdentityVisitorV1 {
            type Value = ProfileIdentityV1;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("exactly 64 lowercase hexadecimal characters")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.len() != 64 {
                    return Err(E::invalid_length(value.len(), &self));
                }
                let mut bytes = [0_u8; 32];
                for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
                    bytes[index] = (parse_hex(pair[0])
                        .ok_or_else(|| E::custom("identity is not lowercase hex"))?
                        << 4)
                        | parse_hex(pair[1])
                            .ok_or_else(|| E::custom("identity is not lowercase hex"))?;
                }
                ProfileIdentityV1::new(bytes).map_err(E::custom)
            }
        }
        deserializer.deserialize_str(IdentityVisitorV1)
    }
}

const fn hex_digit(value: u8) -> u8 {
    if value < 10 {
        b'0' + value
    } else {
        b'a' + value - 10
    }
}

const fn parse_hex(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileTruthOriginV1 {
    Observed,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdProfileResourceKindV1 {
    NativeQueue,
    Stream,
    Allocation,
    Module,
    Kernel,
    Dispatch,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdProfileMemoryKindV1 {
    HostVisible,
    DeviceLocalHostStaged,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdProfileAccessV1 {
    Read,
    Write,
    ReadWrite,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdProfileHostContentModeV1 {
    RangeOnly,
    ContentIdentity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KfdProfileUnavailableFactV1 {
    Rocprofv3DispatchCorrelation,
    DeviceClockTimestamps,
    DeviceCopyEngineEvents,
    HardwareCounters,
    PcSamples,
    DecodedAttEvents,
    SourceIrIsaCorrelation,
    SemanticExecutionHistory,
}

pub const KFD_RUNTIME_PROFILE_UNAVAILABLE_FACTS_V1: [KfdProfileUnavailableFactV1; 8] = [
    KfdProfileUnavailableFactV1::Rocprofv3DispatchCorrelation,
    KfdProfileUnavailableFactV1::DeviceClockTimestamps,
    KfdProfileUnavailableFactV1::DeviceCopyEngineEvents,
    KfdProfileUnavailableFactV1::HardwareCounters,
    KfdProfileUnavailableFactV1::PcSamples,
    KfdProfileUnavailableFactV1::DecodedAttEvents,
    KfdProfileUnavailableFactV1::SourceIrIsaCorrelation,
    KfdProfileUnavailableFactV1::SemanticExecutionHistory,
];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileContentIdentityV1 {
    pub digest: ProfileIdentityV1,
    pub byte_len: u64,
}

impl ProfileContentIdentityV1 {
    pub fn observed(bytes: &[u8]) -> Result<Self, KfdRuntimeProfileErrorV1> {
        let byte_len =
            u64::try_from(bytes.len()).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?;
        let sha256 = Sha256::digest(bytes).into();
        Self::observed_sha256(byte_len, sha256)
    }

    pub fn observed_sha256(
        byte_len: u64,
        sha256: [u8; 32],
    ) -> Result<Self, KfdRuntimeProfileErrorV1> {
        Ok(Self {
            digest: domain_identity(
                CONTENT_IDENTITY_DOMAIN_V1,
                &[&byte_len.to_le_bytes(), &sha256],
            )?,
            byte_len,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum KfdProfileHostContentV1 {
    RangeOnly { byte_len: u64 },
    ContentIdentity { content: ProfileContentIdentityV1 },
}

impl KfdProfileHostContentV1 {
    pub const fn byte_len(self) -> u64 {
        match self {
            Self::RangeOnly { byte_len } => byte_len,
            Self::ContentIdentity { content } => content.byte_len,
        }
    }

    const fn mode(self) -> KfdProfileHostContentModeV1 {
        match self {
            Self::RangeOnly { .. } => KfdProfileHostContentModeV1::RangeOnly,
            Self::ContentIdentity { .. } => KfdProfileHostContentModeV1::ContentIdentity,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdProfileDeviceV1 {
    pub identity: ProfileIdentityV1,
    pub target_profile: String,
    pub wave_width: u16,
}

impl KfdProfileDeviceV1 {
    pub fn observed(
        unique_id: u64,
        target_profile: &str,
        wave_width: u16,
    ) -> Result<Self, KfdRuntimeProfileErrorV1> {
        if unique_id == 0 {
            return Err(KfdRuntimeProfileErrorV1::ZeroDeviceIdentityInput);
        }
        validate_target_profile(target_profile)?;
        if wave_width == 0 {
            return Err(KfdRuntimeProfileErrorV1::InvalidDeviceOrEventCount);
        }
        Ok(Self {
            identity: domain_identity(
                DEVICE_IDENTITY_DOMAIN_V1,
                &[
                    &unique_id.to_le_bytes(),
                    target_profile.as_bytes(),
                    &wave_width.to_le_bytes(),
                ],
            )?,
            target_profile: target_profile.to_owned(),
            wave_width,
        })
    }
}

fn validate_target_profile(target: &str) -> Result<(), KfdRuntimeProfileErrorV1> {
    if target.is_empty()
        || target.len() > MAX_KFD_PROFILE_TARGET_BYTES_V1
        || !target.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b':' | b'+' | b'-' | b'_')
        })
    {
        return Err(KfdRuntimeProfileErrorV1::InvalidTargetProfile);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdProfileLaunchV1 {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
    pub dynamic_shared_bytes: u32,
}

impl KfdProfileLaunchV1 {
    pub fn validate(self) -> Result<(), KfdRuntimeProfileErrorV1> {
        if self.grid.contains(&0) || self.workgroup.contains(&0) {
            return Err(KfdRuntimeProfileErrorV1::InvalidLaunch);
        }
        self.workgroup
            .into_iter()
            .try_fold(1_u32, u32::checked_mul)
            .ok_or(KfdRuntimeProfileErrorV1::InvalidLaunch)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdProfileBindingV1 {
    pub allocation: ProfileIdentityV1,
    pub access: KfdProfileAccessV1,
    pub byte_offset: u64,
    pub byte_len: u64,
    pub kernarg_byte_offset: u32,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdProfileHostTimingV1 {
    pub preparation_ns: u64,
    pub bound_snapshot_ns: u64,
    pub authority_ns: u64,
    pub native_binding_ns: u64,
    pub publication_ns: u64,
    pub publish_to_completion_ns: u64,
    pub completed_readback_ns: u64,
    pub recycle_ns: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum KfdRuntimeProfileEventKindV1 {
    NativeQueueCreated {
        queue: ProfileIdentityV1,
    },
    NativeQueueDestroyed {
        queue: ProfileIdentityV1,
    },
    StreamCreated {
        stream: ProfileIdentityV1,
    },
    StreamDestroyed {
        stream: ProfileIdentityV1,
    },
    AllocationCreated {
        allocation: ProfileIdentityV1,
        memory_kind: KfdProfileMemoryKindV1,
        byte_len: u64,
        alignment: u64,
    },
    HostWrite {
        allocation: ProfileIdentityV1,
        byte_offset: u64,
        content: KfdProfileHostContentV1,
    },
    HostRead {
        allocation: ProfileIdentityV1,
        byte_offset: u64,
        content: KfdProfileHostContentV1,
    },
    AllocationReleased {
        allocation: ProfileIdentityV1,
    },
    ModuleLoaded {
        module: ProfileIdentityV1,
        artifact: ProfileContentIdentityV1,
    },
    KernelResolved {
        kernel: ProfileIdentityV1,
        module: ProfileIdentityV1,
        name: ProfileContentIdentityV1,
        signature: ProfileContentIdentityV1,
    },
    ModuleUnloaded {
        module: ProfileIdentityV1,
    },
    DispatchPublished {
        dispatch: ProfileIdentityV1,
        queue: ProfileIdentityV1,
        stream: ProfileIdentityV1,
        kernel: ProfileIdentityV1,
        dispatch_shape: ProfileContentIdentityV1,
        launch: KfdProfileLaunchV1,
        bindings: Vec<KfdProfileBindingV1>,
    },
    DispatchCompleted {
        dispatch: ProfileIdentityV1,
        host_timing: KfdProfileHostTimingV1,
    },
    SubmissionReleased {
        dispatch: ProfileIdentityV1,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdRuntimeProfileEventV1 {
    pub sequence: u64,
    pub identity: ProfileIdentityV1,
    pub origin: ProfileTruthOriginV1,
    pub event: KfdRuntimeProfileEventKindV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdRuntimeProfileCoverageV1 {
    pub origin: ProfileTruthOriginV1,
    pub observed_events: u64,
    pub dropped_events: u64,
    pub complete_runtime_operation_history: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct KfdRuntimeProfileV1 {
    pub schema: String,
    pub schema_version: u16,
    pub capture_scope: ProfileIdentityV1,
    pub device: KfdProfileDeviceV1,
    pub host_content_mode: KfdProfileHostContentModeV1,
    pub events: Vec<KfdRuntimeProfileEventV1>,
    pub coverage: KfdRuntimeProfileCoverageV1,
    pub unavailable: Vec<KfdProfileUnavailableFactV1>,
}

impl KfdRuntimeProfileV1 {
    pub fn new(
        capture_scope: ProfileIdentityV1,
        device: KfdProfileDeviceV1,
        host_content_mode: KfdProfileHostContentModeV1,
        events: Vec<KfdRuntimeProfileEventV1>,
        dropped_events: u64,
    ) -> Result<Self, KfdRuntimeProfileErrorV1> {
        let observed_events =
            u64::try_from(events.len()).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?;
        let capture = Self {
            schema: KFD_RUNTIME_PROFILE_SCHEMA_V1.to_owned(),
            schema_version: KFD_RUNTIME_PROFILE_SCHEMA_VERSION_V1,
            capture_scope,
            device,
            host_content_mode,
            events,
            coverage: KfdRuntimeProfileCoverageV1 {
                origin: ProfileTruthOriginV1::Observed,
                observed_events,
                dropped_events,
                complete_runtime_operation_history: dropped_events == 0,
            },
            unavailable: KFD_RUNTIME_PROFILE_UNAVAILABLE_FACTS_V1.to_vec(),
        };
        capture.validate()?;
        Ok(capture)
    }

    pub fn validate(&self) -> Result<(), KfdRuntimeProfileErrorV1> {
        if self.schema != KFD_RUNTIME_PROFILE_SCHEMA_V1
            || self.schema_version != KFD_RUNTIME_PROFILE_SCHEMA_VERSION_V1
        {
            return Err(KfdRuntimeProfileErrorV1::UnsupportedVersion);
        }
        validate_target_profile(&self.device.target_profile)?;
        if self.device.wave_width == 0
            || self.events.len() > MAX_KFD_RUNTIME_PROFILE_EVENTS_V1 as usize
        {
            return Err(KfdRuntimeProfileErrorV1::InvalidDeviceOrEventCount);
        }
        let expected_observed =
            u64::try_from(self.events.len()).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?;
        if self.coverage.origin != ProfileTruthOriginV1::Observed
            || self.coverage.observed_events != expected_observed
            || self.coverage.complete_runtime_operation_history
                != (self.coverage.dropped_events == 0)
            || self.unavailable != KFD_RUNTIME_PROFILE_UNAVAILABLE_FACTS_V1
        {
            return Err(KfdRuntimeProfileErrorV1::InvalidCoverage);
        }

        struct LiveDispatchV1 {
            queue: ProfileIdentityV1,
            stream: ProfileIdentityV1,
            kernel: ProfileIdentityV1,
            allocations: BTreeSet<ProfileIdentityV1>,
        }

        let mut resource_identities = BTreeSet::new();
        let mut queues = BTreeSet::new();
        let mut streams = BTreeSet::new();
        let mut allocations = BTreeMap::new();
        let mut modules = BTreeSet::new();
        let mut kernels = BTreeMap::new();
        let mut dispatches: BTreeMap<ProfileIdentityV1, LiveDispatchV1> = BTreeMap::new();
        let mut completed = BTreeSet::new();
        for (sequence, event) in (0_u64..).zip(&self.events) {
            if event.sequence != sequence
                || event.origin != ProfileTruthOriginV1::Observed
                || event.identity
                    != derive_event_identity_v1(self.capture_scope, sequence, &event.event)?
            {
                return Err(KfdRuntimeProfileErrorV1::StaleEventIdentity);
            }
            match &event.event {
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => {
                    if !resource_identities.insert(*queue) || !queues.insert(*queue) {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue } => {
                    if !streams.is_empty()
                        || dispatches.values().any(|owner| owner.queue == *queue)
                        || !queues.remove(queue)
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::StreamCreated { stream } => {
                    if !resource_identities.insert(*stream) || !streams.insert(*stream) {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::StreamDestroyed { stream } => {
                    if dispatches.iter().any(|(dispatch, owner)| {
                        !completed.contains(dispatch) && owner.stream == *stream
                    }) || !streams.remove(stream)
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::AllocationCreated {
                    allocation,
                    byte_len,
                    alignment,
                    ..
                } => {
                    if *byte_len == 0
                        || *alignment == 0
                        || !alignment.is_power_of_two()
                        || !resource_identities.insert(*allocation)
                        || allocations.insert(*allocation, *byte_len).is_some()
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::HostWrite {
                    allocation,
                    byte_offset,
                    content,
                }
                | KfdRuntimeProfileEventKindV1::HostRead {
                    allocation,
                    byte_offset,
                    content,
                } => {
                    let in_bounds = allocations.get(allocation).is_some_and(|allocation_len| {
                        byte_offset
                            .checked_add(content.byte_len())
                            .is_some_and(|end| end <= *allocation_len)
                    });
                    if content.mode() != self.host_content_mode
                        || !in_bounds
                        || dispatches.iter().any(|(dispatch, owner)| {
                            !completed.contains(dispatch) && owner.allocations.contains(allocation)
                        })
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::AllocationReleased { allocation } => {
                    if dispatches.iter().any(|(dispatch, owner)| {
                        !completed.contains(dispatch) && owner.allocations.contains(allocation)
                    }) || allocations.remove(allocation).is_none()
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::ModuleLoaded { module, artifact } => {
                    if artifact.byte_len == 0
                        || !resource_identities.insert(*module)
                        || !modules.insert(*module)
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::KernelResolved {
                    kernel,
                    module,
                    name,
                    ..
                } => {
                    if !modules.contains(module)
                        || name.byte_len == 0
                        || !resource_identities.insert(*kernel)
                        || kernels.insert(*kernel, *module).is_some()
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::ModuleUnloaded { module } => {
                    if dispatches.iter().any(|(dispatch, owner)| {
                        !completed.contains(dispatch)
                            && kernels
                                .get(&owner.kernel)
                                .is_some_and(|owner_module| owner_module == module)
                    }) || !modules.remove(module)
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                    kernels.retain(|_, owner| owner != module);
                }
                KfdRuntimeProfileEventKindV1::DispatchPublished {
                    dispatch,
                    queue,
                    stream,
                    kernel,
                    launch,
                    bindings,
                    ..
                } => {
                    launch.validate()?;
                    if !queues.contains(queue)
                        || !streams.contains(stream)
                        || !kernels.contains_key(kernel)
                        || bindings.len() > MAX_KFD_RUNTIME_PROFILE_BINDINGS_V1
                        || bindings.iter().any(|binding| {
                            binding.byte_len == 0
                                || allocations.get(&binding.allocation).is_none_or(
                                    |allocation_len| {
                                        binding
                                            .byte_offset
                                            .checked_add(binding.byte_len)
                                            .is_none_or(|end| end > *allocation_len)
                                    },
                                )
                        })
                        || !resource_identities.insert(*dispatch)
                    {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                    let allocations = bindings.iter().map(|binding| binding.allocation).collect();
                    dispatches.insert(
                        *dispatch,
                        LiveDispatchV1 {
                            queue: *queue,
                            stream: *stream,
                            kernel: *kernel,
                            allocations,
                        },
                    );
                }
                KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch, .. } => {
                    if !dispatches.contains_key(dispatch) || !completed.insert(*dispatch) {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
                KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch } => {
                    if !completed.remove(dispatch) || dispatches.remove(dispatch).is_none() {
                        return Err(KfdRuntimeProfileErrorV1::InvalidLifecycle);
                    }
                }
            }
        }
        if self.coverage.complete_runtime_operation_history
            && (!queues.is_empty()
                || !streams.is_empty()
                || !allocations.is_empty()
                || !modules.is_empty()
                || !kernels.is_empty()
                || !dispatches.is_empty())
        {
            return Err(KfdRuntimeProfileErrorV1::IncompleteLifecycleMarkedComplete);
        }
        Ok(())
    }
}

pub fn resource_identity_v1(
    capture_scope: ProfileIdentityV1,
    kind: KfdProfileResourceKindV1,
    private_runtime_handle: u64,
) -> Result<ProfileIdentityV1, KfdRuntimeProfileErrorV1> {
    if private_runtime_handle == 0 {
        return Err(KfdRuntimeProfileErrorV1::ZeroResourceHandle);
    }
    let tag = [match kind {
        KfdProfileResourceKindV1::NativeQueue => 1,
        KfdProfileResourceKindV1::Stream => 2,
        KfdProfileResourceKindV1::Allocation => 3,
        KfdProfileResourceKindV1::Module => 4,
        KfdProfileResourceKindV1::Kernel => 5,
        KfdProfileResourceKindV1::Dispatch => 6,
    }];
    domain_identity(
        RESOURCE_IDENTITY_DOMAIN_V1,
        &[
            &capture_scope.as_bytes(),
            &tag,
            &private_runtime_handle.to_le_bytes(),
        ],
    )
}

pub fn push_observed_event_v1(
    capture_scope: ProfileIdentityV1,
    events: &mut Vec<KfdRuntimeProfileEventV1>,
    event: KfdRuntimeProfileEventKindV1,
) -> Result<(), KfdRuntimeProfileErrorV1> {
    push_observed_event_with_encoded_len_v1(capture_scope, events, event).map(|_| ())
}

pub fn push_observed_event_with_encoded_len_v1(
    capture_scope: ProfileIdentityV1,
    events: &mut Vec<KfdRuntimeProfileEventV1>,
    event: KfdRuntimeProfileEventKindV1,
) -> Result<u64, KfdRuntimeProfileErrorV1> {
    if events.len() >= MAX_KFD_RUNTIME_PROFILE_EVENTS_V1 as usize {
        return Err(KfdRuntimeProfileErrorV1::EventLimitExceeded);
    }
    let sequence =
        u64::try_from(events.len()).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?;
    let payload = serde_json::to_vec(&event).map_err(|_| KfdRuntimeProfileErrorV1::JsonEncode)?;
    let identity = derive_event_identity_from_payload_v1(capture_scope, sequence, &payload)?;
    events.push(KfdRuntimeProfileEventV1 {
        sequence,
        identity,
        origin: ProfileTruthOriginV1::Observed,
        event,
    });
    const FIXED_OBSERVED_EVENT_JSON_BYTES: u64 = 120;
    let payload_len =
        u64::try_from(payload.len()).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?;
    FIXED_OBSERVED_EVENT_JSON_BYTES
        .checked_add(decimal_digits(sequence))
        .and_then(|len| len.checked_add(payload_len))
        .ok_or(KfdRuntimeProfileErrorV1::SizeOverflow)
}

const fn decimal_digits(mut value: u64) -> u64 {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn derive_event_identity_v1(
    capture_scope: ProfileIdentityV1,
    sequence: u64,
    event: &KfdRuntimeProfileEventKindV1,
) -> Result<ProfileIdentityV1, KfdRuntimeProfileErrorV1> {
    let payload = serde_json::to_vec(event).map_err(|_| KfdRuntimeProfileErrorV1::JsonEncode)?;
    derive_event_identity_from_payload_v1(capture_scope, sequence, &payload)
}

fn derive_event_identity_from_payload_v1(
    capture_scope: ProfileIdentityV1,
    sequence: u64,
    payload: &[u8],
) -> Result<ProfileIdentityV1, KfdRuntimeProfileErrorV1> {
    domain_identity(
        EVENT_IDENTITY_DOMAIN_V1,
        &[&capture_scope.as_bytes(), &sequence.to_le_bytes(), payload],
    )
}

fn domain_identity(
    domain: &[u8],
    parts: &[&[u8]],
) -> Result<ProfileIdentityV1, KfdRuntimeProfileErrorV1> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    ProfileIdentityV1::new(hasher.finalize().into())
}

pub fn encode_kfd_runtime_profile_v1(
    capture: &KfdRuntimeProfileV1,
) -> Result<Vec<u8>, KfdRuntimeProfileErrorV1> {
    capture.validate()?;
    let bytes = serde_json::to_vec(capture).map_err(|_| KfdRuntimeProfileErrorV1::JsonEncode)?;
    validate_size(bytes.len())?;
    Ok(bytes)
}

pub fn decode_kfd_runtime_profile_v1(
    bytes: &[u8],
) -> Result<KfdRuntimeProfileV1, KfdRuntimeProfileErrorV1> {
    decode_kfd_runtime_profile_with_content_identity_v1(bytes).map(|(capture, _)| capture)
}

/// Decodes one canonical runtime profile and derives its content identity in the same pass.
pub fn decode_kfd_runtime_profile_with_content_identity_v1(
    bytes: &[u8],
) -> Result<(KfdRuntimeProfileV1, ProfileContentIdentityV1), KfdRuntimeProfileErrorV1> {
    validate_size(bytes.len())?;
    let capture: KfdRuntimeProfileV1 =
        serde_json::from_slice(bytes).map_err(|_| KfdRuntimeProfileErrorV1::JsonDecode)?;
    capture.validate()?;
    if serde_json::to_vec(&capture).map_err(|_| KfdRuntimeProfileErrorV1::JsonEncode)? != bytes {
        return Err(KfdRuntimeProfileErrorV1::NonCanonicalEncoding);
    }
    let content_identity = ProfileContentIdentityV1 {
        digest: domain_identity(PROFILE_CONTENT_IDENTITY_DOMAIN_V1, &[bytes])?,
        byte_len: u64::try_from(bytes.len()).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?,
    };
    Ok((capture, content_identity))
}

pub fn kfd_runtime_profile_content_identity_v1(
    bytes: &[u8],
) -> Result<ProfileContentIdentityV1, KfdRuntimeProfileErrorV1> {
    decode_kfd_runtime_profile_with_content_identity_v1(bytes).map(|(_, identity)| identity)
}

fn validate_size(len: usize) -> Result<(), KfdRuntimeProfileErrorV1> {
    let actual = u64::try_from(len).map_err(|_| KfdRuntimeProfileErrorV1::SizeOverflow)?;
    if actual == 0 || actual > MAX_KFD_RUNTIME_PROFILE_BYTES_V1 {
        return Err(KfdRuntimeProfileErrorV1::CaptureSizeOutOfRange { actual });
    }
    Ok(())
}

#[derive(Debug)]
pub enum KfdRuntimeProfileErrorV1 {
    ZeroIdentity,
    ZeroDeviceIdentityInput,
    ZeroResourceHandle,
    InvalidTargetProfile,
    UnsupportedVersion,
    InvalidDeviceOrEventCount,
    InvalidCoverage,
    InvalidLaunch,
    InvalidLifecycle,
    IncompleteLifecycleMarkedComplete,
    StaleEventIdentity,
    EventLimitExceeded,
    CaptureSizeOutOfRange { actual: u64 },
    SizeOverflow,
    NonCanonicalEncoding,
    JsonEncode,
    JsonDecode,
}

impl fmt::Display for KfdRuntimeProfileErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "direct-KFD runtime profile rejected: {self:?}")
    }
}

impl Error for KfdRuntimeProfileErrorV1 {}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKfdProfilerOperationV1 {
    DiscoverCapabilities,
    InspectCapture,
    ListEvents,
    InspectDispatch,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentKfdProfilerRequestV1 {
    pub schema: String,
    pub request_id: u64,
    pub operation: AgentKfdProfilerOperationV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<ProfileIdentityV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<ProfileIdentityV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKfdProfilerCapabilityStateV1 {
    Available,
    Unavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentKfdProfilerCapabilityV1 {
    pub operation: String,
    pub state: AgentKfdProfilerCapabilityStateV1,
    pub reason: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentKfdProfilerCursorV1 {
    pub identity: ProfileIdentityV1,
    pub next_sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "response", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::large_enum_variant)]
pub enum AgentKfdProfilerResponseBodyV1 {
    Capabilities {
        capabilities: Vec<AgentKfdProfilerCapabilityV1>,
    },
    Capture {
        capture_identity: ProfileContentIdentityV1,
        device: KfdProfileDeviceV1,
        coverage: KfdRuntimeProfileCoverageV1,
        unavailable: Vec<KfdProfileUnavailableFactV1>,
    },
    Events {
        capture_identity: ProfileContentIdentityV1,
        items: Vec<KfdRuntimeProfileEventV1>,
        next_cursor: Option<AgentKfdProfilerCursorV1>,
    },
    Dispatch {
        capture_identity: ProfileContentIdentityV1,
        publication: KfdRuntimeProfileEventV1,
        completion: Option<KfdRuntimeProfileEventV1>,
    },
    Error {
        code: String,
        detail: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AgentKfdProfilerResponseV1 {
    pub schema: String,
    pub request_id: u64,
    pub body: AgentKfdProfilerResponseBodyV1,
}

pub fn encode_agent_kfd_profiler_request_v1(
    request: &AgentKfdProfilerRequestV1,
) -> Result<Vec<u8>, AgentKfdProfilerErrorV1> {
    if request.schema != AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1 {
        return Err(AgentKfdProfilerErrorV1::InvalidRequest);
    }
    let bytes = serde_json::to_vec(request).map_err(|_| AgentKfdProfilerErrorV1::InvalidRequest)?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_AGENT_KFD_PROFILER_REQUEST_BYTES_V1 {
        return Err(AgentKfdProfilerErrorV1::RequestSizeOutOfRange);
    }
    Ok(bytes)
}

pub fn answer_agent_kfd_profiler_request_v1(
    capture_bytes: &[u8],
    request_bytes: &[u8],
) -> Result<Vec<u8>, AgentKfdProfilerErrorV1> {
    if request_bytes.is_empty()
        || request_bytes.len() as u64 > MAX_AGENT_KFD_PROFILER_REQUEST_BYTES_V1
    {
        return Err(AgentKfdProfilerErrorV1::RequestSizeOutOfRange);
    }
    let request: AgentKfdProfilerRequestV1 = serde_json::from_slice(request_bytes)
        .map_err(|_| AgentKfdProfilerErrorV1::InvalidRequest)?;
    if request.schema != AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1 {
        return Err(AgentKfdProfilerErrorV1::InvalidRequest);
    }
    if serde_json::to_vec(&request).map_err(|_| AgentKfdProfilerErrorV1::InvalidRequest)?
        != request_bytes
    {
        return Err(AgentKfdProfilerErrorV1::InvalidRequest);
    }
    let capture =
        decode_kfd_runtime_profile_v1(capture_bytes).map_err(AgentKfdProfilerErrorV1::Capture)?;
    let capture_identity = kfd_runtime_profile_content_identity_v1(capture_bytes)
        .map_err(AgentKfdProfilerErrorV1::Capture)?;
    let body = match answer_agent_operation_v1(&capture, capture_identity, &request) {
        Ok(body) => body,
        Err(error) => AgentKfdProfilerResponseBodyV1::Error {
            code: error.code().to_owned(),
            detail: error.detail().to_owned(),
        },
    };
    encode_agent_kfd_profiler_response_v1(AgentKfdProfilerResponseV1 {
        schema: AGENT_KFD_PROFILER_RESPONSE_SCHEMA_V1.to_owned(),
        request_id: request.request_id,
        body,
    })
}

fn answer_agent_operation_v1(
    capture: &KfdRuntimeProfileV1,
    capture_identity: ProfileContentIdentityV1,
    request: &AgentKfdProfilerRequestV1,
) -> Result<AgentKfdProfilerResponseBodyV1, AgentKfdProfilerErrorV1> {
    let body = match request.operation {
        AgentKfdProfilerOperationV1::DiscoverCapabilities => {
            reject_selectors(request, false, false)?;
            AgentKfdProfilerResponseBodyV1::Capabilities {
                capabilities: capabilities(capture.host_content_mode),
            }
        }
        AgentKfdProfilerOperationV1::InspectCapture => {
            reject_selectors(request, false, false)?;
            AgentKfdProfilerResponseBodyV1::Capture {
                capture_identity,
                device: capture.device.clone(),
                coverage: capture.coverage,
                unavailable: capture.unavailable.clone(),
            }
        }
        AgentKfdProfilerOperationV1::ListEvents => {
            if request.dispatch.is_some() {
                return Err(AgentKfdProfilerErrorV1::UnexpectedSelector);
            }
            let limit = request.limit.unwrap_or(64);
            if limit == 0 || limit > MAX_AGENT_KFD_PROFILER_PAGE_ITEMS_V1 {
                return Err(AgentKfdProfilerErrorV1::PageLimitOutOfRange);
            }
            let start = match request.cursor {
                None => 0,
                Some(cursor) => resolve_cursor(capture_identity, cursor, capture.events.len())?,
            };
            let end = start
                .saturating_add(limit as usize)
                .min(capture.events.len());
            let items = capture.events[start..end].to_vec();
            let next_cursor = if end < capture.events.len() {
                let next_sequence = end as u64;
                Some(AgentKfdProfilerCursorV1 {
                    identity: derive_cursor_identity(capture_identity, next_sequence)?,
                    next_sequence,
                })
            } else {
                None
            };
            AgentKfdProfilerResponseBodyV1::Events {
                capture_identity,
                items,
                next_cursor,
            }
        }
        AgentKfdProfilerOperationV1::InspectDispatch => {
            if request.cursor.is_some() || request.limit.is_some() {
                return Err(AgentKfdProfilerErrorV1::UnexpectedSelector);
            }
            let dispatch = request
                .dispatch
                .ok_or(AgentKfdProfilerErrorV1::MissingDispatch)?;
            let publication = capture.events.iter().find(|event| matches!(
                &event.event,
                KfdRuntimeProfileEventKindV1::DispatchPublished { dispatch: candidate, .. } if *candidate == dispatch
            )).cloned().ok_or(AgentKfdProfilerErrorV1::UnknownDispatch)?;
            let completion = capture.events.iter().find(|event| matches!(
                &event.event,
                KfdRuntimeProfileEventKindV1::DispatchCompleted { dispatch: candidate, .. } if *candidate == dispatch
            )).cloned();
            AgentKfdProfilerResponseBodyV1::Dispatch {
                capture_identity,
                publication,
                completion,
            }
        }
    };
    Ok(body)
}

pub fn encode_agent_kfd_profiler_error_response_v1(
    request_id: u64,
    error: &AgentKfdProfilerErrorV1,
) -> Result<Vec<u8>, AgentKfdProfilerErrorV1> {
    encode_agent_kfd_profiler_response_v1(AgentKfdProfilerResponseV1 {
        schema: AGENT_KFD_PROFILER_RESPONSE_SCHEMA_V1.to_owned(),
        request_id,
        body: AgentKfdProfilerResponseBodyV1::Error {
            code: error.code().to_owned(),
            detail: error.detail().to_owned(),
        },
    })
}

fn encode_agent_kfd_profiler_response_v1(
    response: AgentKfdProfilerResponseV1,
) -> Result<Vec<u8>, AgentKfdProfilerErrorV1> {
    let bytes = serde_json::to_vec(&response).map_err(|_| AgentKfdProfilerErrorV1::JsonEncode)?;
    if bytes.len() as u64 > MAX_AGENT_KFD_PROFILER_RESPONSE_BYTES_V1 {
        return Err(AgentKfdProfilerErrorV1::ResponseSizeExceeded);
    }
    Ok(bytes)
}

pub fn decode_agent_kfd_profiler_response_v1(
    bytes: &[u8],
) -> Result<AgentKfdProfilerResponseV1, AgentKfdProfilerErrorV1> {
    if bytes.is_empty() || bytes.len() as u64 > MAX_AGENT_KFD_PROFILER_RESPONSE_BYTES_V1 {
        return Err(AgentKfdProfilerErrorV1::ResponseSizeExceeded);
    }
    let response: AgentKfdProfilerResponseV1 =
        serde_json::from_slice(bytes).map_err(|_| AgentKfdProfilerErrorV1::InvalidResponse)?;
    if response.schema != AGENT_KFD_PROFILER_RESPONSE_SCHEMA_V1 {
        return Err(AgentKfdProfilerErrorV1::InvalidResponse);
    }
    let canonical =
        serde_json::to_vec(&response).map_err(|_| AgentKfdProfilerErrorV1::JsonEncode)?;
    if canonical != bytes {
        return Err(AgentKfdProfilerErrorV1::InvalidResponse);
    }
    Ok(response)
}

fn reject_selectors(
    request: &AgentKfdProfilerRequestV1,
    cursor: bool,
    dispatch: bool,
) -> Result<(), AgentKfdProfilerErrorV1> {
    if (!cursor && (request.cursor.is_some() || request.limit.is_some()))
        || (!dispatch && request.dispatch.is_some())
    {
        return Err(AgentKfdProfilerErrorV1::UnexpectedSelector);
    }
    Ok(())
}

fn capabilities(
    host_content_mode: KfdProfileHostContentModeV1,
) -> Vec<AgentKfdProfilerCapabilityV1> {
    let mut values = vec![
        available("inspect_capture"),
        available("list_events"),
        available("inspect_dispatch"),
    ];
    values.push(match host_content_mode {
        KfdProfileHostContentModeV1::RangeOnly => AgentKfdProfilerCapabilityV1 {
            operation: "query_host_staging_content_identity".to_owned(),
            state: AgentKfdProfilerCapabilityStateV1::Unavailable,
            reason: Some("host_content_identity_not_requested".to_owned()),
        },
        KfdProfileHostContentModeV1::ContentIdentity => {
            available("query_host_staging_content_identity")
        }
    });
    for (operation, reason) in [
        (
            "correlate_rocprofv3_dispatch",
            "rocprofv3_dispatch_correlation_unavailable",
        ),
        (
            "query_device_timestamps",
            "only_host_monotonic_elapsed_durations_observed",
        ),
        (
            "query_device_copy_engine",
            "only_host_staging_reads_and_writes_observed",
        ),
        ("query_hardware_counters", "hardware_counters_not_captured"),
        ("query_pc_samples", "pc_samples_not_captured"),
        ("query_decoded_att", "decoded_att_events_not_captured"),
        (
            "resolve_source_ir_isa",
            "authenticated_source_correlation_not_captured",
        ),
    ] {
        values.push(AgentKfdProfilerCapabilityV1 {
            operation: operation.to_owned(),
            state: AgentKfdProfilerCapabilityStateV1::Unavailable,
            reason: Some(reason.to_owned()),
        });
    }
    values
}

fn available(operation: &str) -> AgentKfdProfilerCapabilityV1 {
    AgentKfdProfilerCapabilityV1 {
        operation: operation.to_owned(),
        state: AgentKfdProfilerCapabilityStateV1::Available,
        reason: None,
    }
}

fn derive_cursor_identity(
    capture: ProfileContentIdentityV1,
    next_sequence: u64,
) -> Result<ProfileIdentityV1, AgentKfdProfilerErrorV1> {
    domain_identity(
        CURSOR_IDENTITY_DOMAIN_V1,
        &[
            &capture.digest.as_bytes(),
            &capture.byte_len.to_le_bytes(),
            &next_sequence.to_le_bytes(),
        ],
    )
    .map_err(AgentKfdProfilerErrorV1::Capture)
}

fn resolve_cursor(
    capture: ProfileContentIdentityV1,
    cursor: ProfileIdentityV1,
    event_count: usize,
) -> Result<usize, AgentKfdProfilerErrorV1> {
    for next in 1..event_count {
        let next_sequence = next as u64;
        if derive_cursor_identity(capture, next_sequence)? == cursor {
            return Ok(next);
        }
    }
    Err(AgentKfdProfilerErrorV1::InvalidCursor)
}

#[derive(Debug)]
pub enum AgentKfdProfilerErrorV1 {
    RequestSizeOutOfRange,
    InvalidRequest,
    UnexpectedSelector,
    MissingDispatch,
    UnknownDispatch,
    PageLimitOutOfRange,
    InvalidCursor,
    ResponseSizeExceeded,
    JsonEncode,
    InvalidResponse,
    Capture(KfdRuntimeProfileErrorV1),
}

impl AgentKfdProfilerErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RequestSizeOutOfRange => "request_size_out_of_range",
            Self::InvalidRequest => "invalid_request",
            Self::UnexpectedSelector => "unexpected_selector",
            Self::MissingDispatch => "missing_dispatch",
            Self::UnknownDispatch => "unknown_dispatch",
            Self::PageLimitOutOfRange => "page_limit_out_of_range",
            Self::InvalidCursor => "invalid_cursor",
            Self::ResponseSizeExceeded => "response_size_exceeded",
            Self::JsonEncode => "json_encode",
            Self::InvalidResponse => "invalid_response",
            Self::Capture(_) => "capture_rejected",
        }
    }

    pub const fn detail(&self) -> &'static str {
        match self {
            Self::RequestSizeOutOfRange => "request exceeds the canonical byte bound",
            Self::InvalidRequest => "request is not canonical profiler query JSON",
            Self::UnexpectedSelector => "operation received an unsupported selector",
            Self::MissingDispatch => "inspect_dispatch requires a dispatch identity",
            Self::UnknownDispatch => "dispatch identity is absent from this capture",
            Self::PageLimitOutOfRange => "event page limit is outside the admitted bound",
            Self::InvalidCursor => "cursor is not bound to an available page in this capture",
            Self::ResponseSizeExceeded => "response exceeds the canonical byte bound",
            Self::JsonEncode => "response encoding failed",
            Self::InvalidResponse => "response is not canonical profiler query JSON",
            Self::Capture(_) => "capture failed canonical validation",
        }
    }
}

impl fmt::Display for AgentKfdProfilerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "agent direct-KFD profiler query rejected: {self:?}"
        )
    }
}

impl Error for AgentKfdProfilerErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(seed: u8) -> ProfileIdentityV1 {
        ProfileIdentityV1::new([seed; 32]).unwrap()
    }

    fn closed_capture() -> KfdRuntimeProfileV1 {
        let scope = identity(1);
        let queue = resource_identity_v1(scope, KfdProfileResourceKindV1::NativeQueue, 1).unwrap();
        let stream = resource_identity_v1(scope, KfdProfileResourceKindV1::Stream, 1).unwrap();
        let allocation =
            resource_identity_v1(scope, KfdProfileResourceKindV1::Allocation, 2).unwrap();
        let module = resource_identity_v1(scope, KfdProfileResourceKindV1::Module, 3).unwrap();
        let kernel = resource_identity_v1(scope, KfdProfileResourceKindV1::Kernel, 4).unwrap();
        let dispatch = resource_identity_v1(scope, KfdProfileResourceKindV1::Dispatch, 5).unwrap();
        let mut events = Vec::new();
        for event in [
            KfdRuntimeProfileEventKindV1::StreamCreated { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
            KfdRuntimeProfileEventKindV1::AllocationCreated {
                allocation,
                memory_kind: KfdProfileMemoryKindV1::HostVisible,
                byte_len: 64,
                alignment: 8,
            },
            KfdRuntimeProfileEventKindV1::HostWrite {
                allocation,
                byte_offset: 0,
                content: KfdProfileHostContentV1::ContentIdentity {
                    content: ProfileContentIdentityV1::observed(&[1; 64]).unwrap(),
                },
            },
            KfdRuntimeProfileEventKindV1::HostRead {
                allocation,
                byte_offset: 0,
                content: KfdProfileHostContentV1::ContentIdentity {
                    content: ProfileContentIdentityV1::observed(&[1; 64]).unwrap(),
                },
            },
            KfdRuntimeProfileEventKindV1::ModuleLoaded {
                module,
                artifact: ProfileContentIdentityV1::observed(&[2; 16]).unwrap(),
            },
            KfdRuntimeProfileEventKindV1::KernelResolved {
                kernel,
                module,
                name: ProfileContentIdentityV1::observed(b"kernel").unwrap(),
                signature: ProfileContentIdentityV1::observed(&[9; 32]).unwrap(),
            },
            KfdRuntimeProfileEventKindV1::DispatchPublished {
                dispatch,
                queue,
                stream,
                kernel,
                dispatch_shape: ProfileContentIdentityV1::observed(&[10; 32]).unwrap(),
                launch: KfdProfileLaunchV1 {
                    grid: [64, 1, 1],
                    workgroup: [64, 1, 1],
                    dynamic_shared_bytes: 0,
                },
                bindings: vec![KfdProfileBindingV1 {
                    allocation,
                    access: KfdProfileAccessV1::ReadWrite,
                    byte_offset: 0,
                    byte_len: 64,
                    kernarg_byte_offset: 0,
                }],
            },
            KfdRuntimeProfileEventKindV1::DispatchCompleted {
                dispatch,
                host_timing: KfdProfileHostTimingV1::default(),
            },
            KfdRuntimeProfileEventKindV1::SubmissionReleased { dispatch },
            KfdRuntimeProfileEventKindV1::ModuleUnloaded { module },
            KfdRuntimeProfileEventKindV1::AllocationReleased { allocation },
            KfdRuntimeProfileEventKindV1::StreamDestroyed { stream },
            KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        ] {
            let encoded_len =
                push_observed_event_with_encoded_len_v1(scope, &mut events, event).unwrap();
            assert_eq!(
                encoded_len,
                serde_json::to_vec(events.last().unwrap()).unwrap().len() as u64
            );
        }
        KfdRuntimeProfileV1::new(
            scope,
            KfdProfileDeviceV1::observed(7, "gfx942:xnack-", 64).unwrap(),
            KfdProfileHostContentModeV1::ContentIdentity,
            events,
            0,
        )
        .unwrap()
    }

    fn reauthenticate(capture: &mut KfdRuntimeProfileV1) {
        for (sequence, event) in capture.events.iter_mut().enumerate() {
            event.sequence = sequence as u64;
            event.identity =
                derive_event_identity_v1(capture.capture_scope, event.sequence, &event.event)
                    .unwrap();
        }
        capture.coverage.observed_events = capture.events.len() as u64;
    }

    #[test]
    fn canonical_capture_rejects_event_substitution() {
        let capture = closed_capture();
        let bytes = encode_kfd_runtime_profile_v1(&capture).unwrap();
        assert_eq!(decode_kfd_runtime_profile_v1(&bytes).unwrap(), capture);
        let mut substituted = capture;
        let byte_len = substituted
            .events
            .iter_mut()
            .find_map(|event| match &mut event.event {
                KfdRuntimeProfileEventKindV1::AllocationCreated { byte_len, .. } => Some(byte_len),
                _ => None,
            })
            .unwrap();
        *byte_len = 32;
        assert!(matches!(
            substituted.validate(),
            Err(KfdRuntimeProfileErrorV1::StaleEventIdentity)
        ));
    }

    #[test]
    fn incomplete_lifecycle_requires_disclosed_loss() {
        let mut capture = closed_capture();
        capture.events.pop();
        capture.coverage.observed_events -= 1;
        assert!(matches!(
            capture.validate(),
            Err(KfdRuntimeProfileErrorV1::IncompleteLifecycleMarkedComplete)
        ));
        capture.coverage.dropped_events = 1;
        capture.coverage.complete_runtime_operation_history = false;
        capture.validate().unwrap();
    }

    #[test]
    fn complete_capture_rejects_parent_teardown_before_dispatch_completion() {
        for parent in 0..4 {
            let mut capture = closed_capture();
            let teardown = capture
                .events
                .iter()
                .position(|event| {
                    matches!(
                        (&event.event, parent),
                        (KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { .. }, 0)
                            | (KfdRuntimeProfileEventKindV1::StreamDestroyed { .. }, 1)
                            | (KfdRuntimeProfileEventKindV1::AllocationReleased { .. }, 2)
                            | (KfdRuntimeProfileEventKindV1::ModuleUnloaded { .. }, 3)
                    )
                })
                .unwrap();
            let teardown = capture.events.remove(teardown);
            let before_completion = capture
                .events
                .iter()
                .position(|event| {
                    matches!(
                        event.event,
                        KfdRuntimeProfileEventKindV1::DispatchCompleted { .. }
                    )
                })
                .unwrap();
            capture.events.insert(before_completion, teardown);
            reauthenticate(&mut capture);
            assert!(matches!(
                capture.validate(),
                Err(KfdRuntimeProfileErrorV1::InvalidLifecycle)
            ));
        }
    }

    #[test]
    fn capture_rejects_host_access_while_a_dispatch_owns_the_allocation() {
        for read in [false, true] {
            let mut capture = closed_capture();
            let host_access = capture
                .events
                .iter()
                .position(|event| {
                    matches!(
                        (&event.event, read),
                        (KfdRuntimeProfileEventKindV1::HostWrite { .. }, false)
                            | (KfdRuntimeProfileEventKindV1::HostRead { .. }, true)
                    )
                })
                .unwrap();
            let host_access = capture.events.remove(host_access);
            let after_publication = capture
                .events
                .iter()
                .position(|event| {
                    matches!(
                        event.event,
                        KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                    )
                })
                .unwrap()
                + 1;
            capture.events.insert(after_publication, host_access);
            reauthenticate(&mut capture);
            assert!(matches!(
                capture.validate(),
                Err(KfdRuntimeProfileErrorV1::InvalidLifecycle)
            ));
        }
    }

    #[test]
    fn host_access_and_parent_teardown_are_valid_after_dispatch_completion() {
        let mut capture = closed_capture();
        let mut quiescent_events = Vec::new();
        for predicate in [
            |event: &KfdRuntimeProfileEventV1| {
                matches!(event.event, KfdRuntimeProfileEventKindV1::HostRead { .. })
            },
            |event: &KfdRuntimeProfileEventV1| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::ModuleUnloaded { .. }
                )
            },
            |event: &KfdRuntimeProfileEventV1| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::AllocationReleased { .. }
                )
            },
            |event: &KfdRuntimeProfileEventV1| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::StreamDestroyed { .. }
                )
            },
        ] as [fn(&KfdRuntimeProfileEventV1) -> bool; 4]
        {
            let index = capture.events.iter().position(predicate).unwrap();
            quiescent_events.push(capture.events.remove(index));
        }
        let before_release = capture
            .events
            .iter()
            .position(|event| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::SubmissionReleased { .. }
                )
            })
            .unwrap();
        capture
            .events
            .splice(before_release..before_release, quiescent_events);
        reauthenticate(&mut capture);
        capture.validate().unwrap();
    }

    #[test]
    fn capture_rejects_cross_kind_and_historical_resource_identity_reuse() {
        let mut cross_kind = closed_capture();
        let stream = cross_kind
            .events
            .iter()
            .find_map(|event| match event.event {
                KfdRuntimeProfileEventKindV1::StreamCreated { stream } => Some(stream),
                _ => None,
            })
            .unwrap();
        for event in &mut cross_kind.events {
            match &mut event.event {
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue }
                | KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue }
                | KfdRuntimeProfileEventKindV1::DispatchPublished { queue, .. } => *queue = stream,
                _ => {}
            }
        }
        reauthenticate(&mut cross_kind);
        assert!(matches!(
            cross_kind.validate(),
            Err(KfdRuntimeProfileErrorV1::InvalidLifecycle)
        ));

        let mut historical = closed_capture();
        let queue = historical
            .events
            .iter()
            .find_map(|event| match event.event {
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue } => Some(queue),
                _ => None,
            })
            .unwrap();
        historical.events.push(KfdRuntimeProfileEventV1 {
            sequence: 0,
            identity: identity(9),
            origin: ProfileTruthOriginV1::Observed,
            event: KfdRuntimeProfileEventKindV1::NativeQueueCreated { queue },
        });
        historical.events.push(KfdRuntimeProfileEventV1 {
            sequence: 0,
            identity: identity(9),
            origin: ProfileTruthOriginV1::Observed,
            event: KfdRuntimeProfileEventKindV1::NativeQueueDestroyed { queue },
        });
        reauthenticate(&mut historical);
        assert!(matches!(
            historical.validate(),
            Err(KfdRuntimeProfileErrorV1::InvalidLifecycle)
        ));
    }

    #[test]
    fn host_content_policy_is_explicit_and_cached_sha256_is_equivalent() {
        let bytes = [7_u8; 64];
        let sha256 = Sha256::digest(bytes).into();
        assert_eq!(
            ProfileContentIdentityV1::observed(&bytes).unwrap(),
            ProfileContentIdentityV1::observed_sha256(bytes.len() as u64, sha256).unwrap()
        );
        let mut capture = closed_capture();
        capture.host_content_mode = KfdProfileHostContentModeV1::RangeOnly;
        assert!(matches!(
            capture.validate(),
            Err(KfdRuntimeProfileErrorV1::InvalidLifecycle)
        ));
    }

    #[test]
    fn agent_pages_are_capture_bound_and_capabilities_are_explicit() {
        let bytes = encode_kfd_runtime_profile_v1(&closed_capture()).unwrap();
        let request = encode_agent_kfd_profiler_request_v1(&AgentKfdProfilerRequestV1 {
            schema: AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 1,
            operation: AgentKfdProfilerOperationV1::ListEvents,
            cursor: None,
            limit: Some(2),
            dispatch: None,
        })
        .unwrap();
        let response = answer_agent_kfd_profiler_request_v1(&bytes, &request).unwrap();
        let response: serde_json::Value = serde_json::from_slice(&response).unwrap();
        assert_eq!(response["body"]["items"].as_array().unwrap().len(), 2);
        assert!(response["body"]["next_cursor"].is_object());

        let capabilities = encode_agent_kfd_profiler_request_v1(&AgentKfdProfilerRequestV1 {
            schema: AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 2,
            operation: AgentKfdProfilerOperationV1::DiscoverCapabilities,
            cursor: None,
            limit: None,
            dispatch: None,
        })
        .unwrap();
        let response = answer_agent_kfd_profiler_request_v1(&bytes, &capabilities).unwrap();
        let text = String::from_utf8(response).unwrap();
        assert!(text.contains("rocprofv3_dispatch_correlation_unavailable"));
        assert!(text.contains("only_host_monotonic_elapsed_durations_observed"));
        assert!(text.contains("query_host_staging_content_identity"));
    }

    #[test]
    fn valid_but_rejected_requests_return_canonical_typed_errors() {
        let bytes = encode_kfd_runtime_profile_v1(&closed_capture()).unwrap();
        let request = encode_agent_kfd_profiler_request_v1(&AgentKfdProfilerRequestV1 {
            schema: AGENT_KFD_PROFILER_REQUEST_SCHEMA_V1.to_owned(),
            request_id: 41,
            operation: AgentKfdProfilerOperationV1::InspectCapture,
            cursor: None,
            limit: Some(1),
            dispatch: None,
        })
        .unwrap();
        let response = answer_agent_kfd_profiler_request_v1(&bytes, &request).unwrap();
        let decoded = decode_agent_kfd_profiler_response_v1(&response).unwrap();
        assert_eq!(decoded.request_id, 41);
        assert!(matches!(
            decoded.body,
            AgentKfdProfilerResponseBodyV1::Error { ref code, .. }
                if code == "unexpected_selector"
        ));

        let mut noncanonical = response;
        noncanonical.push(b'\n');
        assert!(matches!(
            decode_agent_kfd_profiler_response_v1(&noncanonical),
            Err(AgentKfdProfilerErrorV1::InvalidResponse)
        ));
    }

    #[test]
    fn target_profile_is_bounded_but_not_architecture_hard_coded() {
        let future = KfdProfileDeviceV1::observed(9, "gfx1201:xnack+", 32).unwrap();
        assert_eq!(future.target_profile, "gfx1201:xnack+");
        assert_eq!(future.wave_width, 32);
        assert!(matches!(
            KfdProfileDeviceV1::observed(9, "gfx942 xnack-", 64),
            Err(KfdRuntimeProfileErrorV1::InvalidTargetProfile)
        ));
    }

    #[test]
    fn authenticated_event_identity_does_not_admit_out_of_range_binding() {
        let mut capture = closed_capture();
        let event = capture
            .events
            .iter_mut()
            .find(|event| {
                matches!(
                    event.event,
                    KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
                )
            })
            .unwrap();
        if let KfdRuntimeProfileEventKindV1::DispatchPublished { bindings, .. } = &mut event.event {
            bindings[0].byte_offset = 63;
            bindings[0].byte_len = 2;
        } else {
            panic!("fixture dispatch publication moved");
        }
        event.identity =
            derive_event_identity_v1(capture.capture_scope, event.sequence, &event.event).unwrap();
        assert!(matches!(
            capture.validate(),
            Err(KfdRuntimeProfileErrorV1::InvalidLifecycle)
        ));
    }

    #[test]
    fn fixed_capture_envelope_fits_its_reserved_wire_budget() {
        let capture = KfdRuntimeProfileV1::new(
            identity(1),
            KfdProfileDeviceV1::observed(7, &"a".repeat(MAX_KFD_PROFILE_TARGET_BYTES_V1), u16::MAX)
                .unwrap(),
            KfdProfileHostContentModeV1::RangeOnly,
            Vec::new(),
            u64::MAX,
        )
        .unwrap();
        assert!(
            serde_json::to_vec(&capture).unwrap().len() as u64
                <= MAX_KFD_RUNTIME_PROFILE_FIXED_JSON_BYTES_V1
        );
    }
}

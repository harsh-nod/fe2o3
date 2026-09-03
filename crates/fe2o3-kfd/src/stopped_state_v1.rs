//! Bounded, redacted stopped-queue observations from a direct KFD debug session.
//!
//! Linux KFD 1.18 exposes queue suspension, queue-to-context-save-area
//! metadata, and a 40-byte context-save header. fe2o3's direct-KFD queue keeps
//! the header/control-stack copy targets and wave-state BO CPU-visible, so this
//! module can retain those header-bounded bytes as a private opaque checkpoint.
//! KFD does not specify the inner gfx942 wave, lane, or register record layout;
//! no private record is interpreted here.

use core::fmt;

use fe2o3_kfd_uapi::{
    KfdDebugContextSaveAreaHeaderV1, KfdDebugExceptionMaskV1, KfdDebugRuntimeStateV1,
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{
    KfdDebugDeviceObservationV1, KfdDebugQueueObservationV1, KfdLiveDebugSessionErrorV1,
    KfdLiveDebugSessionV1,
};

const ZERO_SCOPE: [u8; 32] = [0; 32];
const GFX942_TARGET_VERSION_V1: u32 = 90_402;
const GFX942_XCC_COUNT_V1: usize = 8;
const GFX942_CONTEXT_BYTES_PER_XCC_V1: u32 = 0x162_1000;
const GFX942_DEBUG_BYTES_V1: u32 = 0x5_f000;
const CONTEXT_HEADER_BYTES_V1: usize = core::mem::size_of::<KfdDebugContextSaveAreaHeaderV1>();
const CONTEXT_HEADER_BYTES_U32_V1: u32 = 40;
const MAX_CONTEXT_HEADERS_V1: usize = GFX942_XCC_COUNT_V1;
const MAX_CHECKPOINT_SEGMENTS_V1: usize = GFX942_XCC_COUNT_V1 * 2;
pub const DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1: u64 = 32 * 1024 * 1024;
pub const MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1: u64 =
    GFX942_CONTEXT_BYTES_PER_XCC_V1 as u64 * GFX942_XCC_COUNT_V1 as u64;

/// Exact claim boundary for the first direct-KFD stopped-state observation.
///
/// The manifest digest identifies this report schema. It grants no authority
/// and does not authenticate KFD, firmware, hardware, or target memory.
pub const KFD_STOPPED_STATE_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-direct-kfd-stopped-state-r2-v1\n",
    "target=linux-x86_64,gfx942,kfd-1.18,direct-kfd-no-hip-no-hsa\n",
    "admission=exact-ptrace-owner,pidfd-bound-live-debug-session,locally-retained-session-owned-prior-kfd-queue-suspension\n",
    "capture=direct-kfd-queue-and-device-snapshot-before,8-bounded-40-byte-process-vm-header-reads,bounded-empty-range-cursors,header-bounded-control-stack-and-wave-state-adjacent-double-read,header-reread,direct-kfd-queue-and-device-snapshot-after,exact-queue-device-binding-substitution-check\n",
    "gfx942=target-version:90402,xcc-count:8,save-bytes-per-xcc:0x1621000,debug-bytes:0x5f000\n",
    "observed=queue-exception-mask,ring-shape,queue-to-save-area-size,gfx-target,xcc-count,kfd-copied-header-and-control-stack,header-ranged-wave-state-opaque-bytes\n",
    "identity=caller-scoped-domain-separated-sha256,local-session-state-and-exact-queue-device-header-range-content-binding,opaque-correlation-not-authentication-or-secrecy\n",
    "redaction=no-pid,gpu-id,queue-id,event-id,payload-address,save-address,ring-address,pointer,fd,handle,pc-or-register-value\n",
    "bounds=default-opaque-checkpoint:33554432,hard-opaque-checkpoint:185630720,segments:16,complete-or-explicit-truncated-no-partial-content-claim\n",
    "privacy=opaque-checkpoint-bytes-private-redacted-debug-zeroized-on-drop,agent-projection-content-identity-and-bounds-only\n",
    "unavailable=decoded-wave-records,lane-state,register-records,pc,source,memory-values\n",
    "limitation=sequential-non-atomic-segment-capture,no-coherent-stopped-interval-or-runtime-reobservation,linux-kfd-uapi-publishes-header-ranges-but-no-inner-gfx942-wave-register-layout;opaque-content-is-not-a-decoded-stopped-wave-observation\n",
    "ownership=detached-inert-snapshot;local-live-session-state-retains-prior-suspension-and-must-explicitly-resume-queue,no-capture-time-suspension-reobservation\n",
    "authority=observation-only,no-address-fd-ioctl-resume-or-target-memory-authority\n",
);

/// SHA-256 of [`KFD_STOPPED_STATE_MANIFEST_V1`].
pub const KFD_STOPPED_STATE_MANIFEST_SHA256_V1: &str =
    "1e378593f7be201411298787ee8e5de4b6af38449230d286b190292a753eab74";

/// Caller-selected correlation scope for redacted stopped-state identities.
///
/// The scope is hash input and is not returned. It is not a secret-key type;
/// publishing or reusing it can make low-entropy inputs correlatable.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct KfdStoppedStateScopeV1([u8; 32]);

impl KfdStoppedStateScopeV1 {
    pub fn new(bytes: [u8; 32]) -> Result<Self, KfdStoppedStateScopeErrorV1> {
        if bytes == ZERO_SCOPE {
            Err(KfdStoppedStateScopeErrorV1::Zero)
        } else {
            Ok(Self(bytes))
        }
    }
}

impl fmt::Debug for KfdStoppedStateScopeV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KfdStoppedStateScopeV1(<redacted>)")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdStoppedStateScopeErrorV1 {
    Zero,
}

impl fmt::Display for KfdStoppedStateScopeErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("stopped-state correlation scope must be nonzero")
    }
}

impl std::error::Error for KfdStoppedStateScopeErrorV1 {}

/// Inert pseudonymous identity in one caller-selected scope and entity domain.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct KfdStoppedLogicalIdentityV1([u8; 32]);

impl KfdStoppedLogicalIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Debug for KfdStoppedLogicalIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("KfdStoppedLogicalIdentityV1(")?;
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        formatter.write_str(")")
    }
}

/// Plan for one queue that is already suspended by the exact live session.
#[derive(Clone, Copy)]
pub struct KfdStoppedQueueCapturePlanV1 {
    queue_id: u32,
    scope: KfdStoppedStateScopeV1,
    checkpoint_byte_limit: u64,
}

impl KfdStoppedQueueCapturePlanV1 {
    pub const fn new(queue_id: u32, scope: KfdStoppedStateScopeV1) -> Self {
        Self {
            queue_id,
            scope,
            checkpoint_byte_limit: DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1,
        }
    }

    pub fn with_checkpoint_byte_limit(
        queue_id: u32,
        scope: KfdStoppedStateScopeV1,
        checkpoint_byte_limit: u64,
    ) -> Result<Self, KfdStoppedStatePlanErrorV1> {
        if checkpoint_byte_limit > MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1 {
            return Err(KfdStoppedStatePlanErrorV1::CheckpointByteLimitExceeded);
        }
        Ok(Self {
            queue_id,
            scope,
            checkpoint_byte_limit,
        })
    }

    pub const fn checkpoint_byte_limit(self) -> u64 {
        self.checkpoint_byte_limit
    }
}

impl fmt::Debug for KfdStoppedQueueCapturePlanV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdStoppedQueueCapturePlanV1")
            .field("queue", &"<redacted>")
            .field("scope", &self.scope)
            .field("checkpoint_byte_limit", &self.checkpoint_byte_limit)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdStoppedStatePlanErrorV1 {
    CheckpointByteLimitExceeded,
}

impl fmt::Display for KfdStoppedStatePlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("opaque checkpoint byte limit exceeds the hard bound")
    }
}

impl std::error::Error for KfdStoppedStatePlanErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdStoppedSnapshotOwnershipV1 {
    /// Local session state retains authority from a prior successful suspend.
    /// This is not a capture-time hardware suspension reobservation. The
    /// detached report cannot resume the queue.
    SessionRetainedSuspension,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdStoppedUnavailableReasonV1 {
    ContextSaveAreaNotReported,
    GfxTargetNotGfx942,
    Gfx942XccCountMismatch,
    Gfx942SaveAreaSizeMismatch,
    TargetAddressNotRepresentable,
    TargetHeaderReadDenied,
    TargetHeaderReadPartial,
    ContextHeaderReservedNonzero,
    ContextHeaderRangePairMalformed,
    ContextHeaderRangeOutOfBounds,
    ContextHeaderRangeOverlap,
    Gfx942DebugRangeMismatch,
    ContextHeaderBindingSubstituted,
    HardwareCheckpointBytesNotCpuVisible,
    TargetCheckpointReadDenied,
    TargetCheckpointReadPartial,
    CheckpointContentChanged,
    CheckpointByteLimitExceeded,
    WaveRecordLayoutNotInKfdUapi,
    LaneStateRequiresWaveRecords,
    RegisterRecordLayoutNotInKfdUapi,
    ProgramCounterRequiresRegisterRecord,
    SourceMapNotBound,
    MemoryValuesNotCaptured,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdStoppedAvailabilityV1 {
    Available,
    Unavailable(KfdStoppedUnavailableReasonV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdOpaqueCheckpointSegmentKindV1 {
    ControlStack,
    WaveState,
}

pub struct KfdOpaqueCheckpointSegmentV1 {
    xcc_ordinal: u8,
    kind: KfdOpaqueCheckpointSegmentKindV1,
    range: KfdStoppedRelativeRangeV1,
    content_identity: KfdStoppedLogicalIdentityV1,
    bytes: Zeroizing<Vec<u8>>,
}

impl fmt::Debug for KfdOpaqueCheckpointSegmentV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdOpaqueCheckpointSegmentV1")
            .field("xcc_ordinal", &self.xcc_ordinal)
            .field("kind", &self.kind)
            .field("range", &self.range)
            .field("content_identity", &self.content_identity)
            .field("bytes", &"<private>")
            .finish()
    }
}

impl KfdOpaqueCheckpointSegmentV1 {
    pub const fn xcc_ordinal(&self) -> u8 {
        self.xcc_ordinal
    }

    pub const fn kind(&self) -> KfdOpaqueCheckpointSegmentKindV1 {
        self.kind
    }

    pub const fn range(&self) -> KfdStoppedRelativeRangeV1 {
        self.range
    }

    pub const fn content_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.content_identity
    }

    pub fn with_private_bytes<T>(&self, inspect: impl FnOnce(&[u8]) -> T) -> T {
        inspect(&self.bytes)
    }
}

#[derive(Debug)]
pub struct KfdOpaqueCheckpointV1 {
    logical_identity: KfdStoppedLogicalIdentityV1,
    content_identity: KfdStoppedLogicalIdentityV1,
    captured_bytes: u64,
    segments: Vec<KfdOpaqueCheckpointSegmentV1>,
}

impl KfdOpaqueCheckpointV1 {
    pub const fn logical_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.logical_identity
    }

    pub const fn content_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.content_identity
    }

    pub const fn captured_bytes(&self) -> u64 {
        self.captured_bytes
    }

    pub fn segments(&self) -> &[KfdOpaqueCheckpointSegmentV1] {
        &self.segments
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdOpaqueCheckpointTruncationV1 {
    required_bytes: u64,
    capture_limit_bytes: u64,
}

impl KfdOpaqueCheckpointTruncationV1 {
    pub const fn required_bytes(self) -> u64 {
        self.required_bytes
    }

    pub const fn capture_limit_bytes(self) -> u64 {
        self.capture_limit_bytes
    }
}

#[derive(Debug)]
pub enum KfdOpaqueCheckpointObservationV1 {
    Complete(KfdOpaqueCheckpointV1),
    Truncated(KfdOpaqueCheckpointTruncationV1),
    Unavailable(KfdStoppedUnavailableReasonV1),
}

/// A bounded byte range relative to one XCC context header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdStoppedRelativeRangeV1 {
    offset: u32,
    bytes: u32,
}

impl KfdStoppedRelativeRangeV1 {
    pub const fn offset(self) -> u32 {
        self.offset
    }

    pub const fn bytes(self) -> u32 {
        self.bytes
    }

    pub const fn is_empty(self) -> bool {
        self.bytes == 0
    }
}

/// Admitted CPU-visible header envelope for one gfx942 XCC.
///
/// These ranges are header metadata, not decoded hardware checkpoint bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdGfx942CwsrHeaderObservationV1 {
    xcc_ordinal: u8,
    logical_identity: KfdStoppedLogicalIdentityV1,
    control_stack: KfdStoppedRelativeRangeV1,
    wave_state: KfdStoppedRelativeRangeV1,
    debug: KfdStoppedRelativeRangeV1,
    error_binding_present: bool,
}

impl KfdGfx942CwsrHeaderObservationV1 {
    pub const fn xcc_ordinal(self) -> u8 {
        self.xcc_ordinal
    }

    pub const fn logical_identity(self) -> KfdStoppedLogicalIdentityV1 {
        self.logical_identity
    }

    pub const fn control_stack(self) -> KfdStoppedRelativeRangeV1 {
        self.control_stack
    }

    pub const fn wave_state(self) -> KfdStoppedRelativeRangeV1 {
        self.wave_state
    }

    pub const fn debug(self) -> KfdStoppedRelativeRangeV1 {
        self.debug
    }

    pub const fn error_binding_present(self) -> bool {
        self.error_binding_present
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KfdGfx942ContextSaveLayoutV1 {
    logical_identity: KfdStoppedLogicalIdentityV1,
    context_bytes_per_xcc: u32,
    total_allocation_bytes: u64,
    headers: [KfdGfx942CwsrHeaderObservationV1; GFX942_XCC_COUNT_V1],
}

impl KfdGfx942ContextSaveLayoutV1 {
    pub const fn logical_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.logical_identity
    }

    pub const fn context_bytes_per_xcc(&self) -> u32 {
        self.context_bytes_per_xcc
    }

    pub const fn total_allocation_bytes(&self) -> u64 {
        self.total_allocation_bytes
    }

    pub fn headers(&self) -> &[KfdGfx942CwsrHeaderObservationV1; GFX942_XCC_COUNT_V1] {
        &self.headers
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KfdStoppedContextSaveObservationV1 {
    Available(Box<KfdGfx942ContextSaveLayoutV1>),
    Unavailable(KfdStoppedUnavailableReasonV1),
}

/// Detached, address-free observation captured after the originating session
/// successfully suspended a queue and while its local ownership stayed intact.
#[derive(Debug)]
pub struct KfdStoppedQueueSnapshotV1 {
    logical_identity: KfdStoppedLogicalIdentityV1,
    queue_identity: KfdStoppedLogicalIdentityV1,
    device_identity: KfdStoppedLogicalIdentityV1,
    exception_status: KfdDebugExceptionMaskV1,
    ring_bytes: u32,
    queue_type: u32,
    gfx_target_version: u32,
    xcc_count: u32,
    ownership: KfdStoppedSnapshotOwnershipV1,
    context_save: KfdStoppedContextSaveObservationV1,
    opaque_checkpoint: KfdOpaqueCheckpointObservationV1,
}

impl KfdStoppedQueueSnapshotV1 {
    pub const fn logical_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.logical_identity
    }

    pub const fn queue_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.queue_identity
    }

    pub const fn device_identity(&self) -> KfdStoppedLogicalIdentityV1 {
        self.device_identity
    }

    pub const fn exception_status(&self) -> KfdDebugExceptionMaskV1 {
        self.exception_status
    }

    pub const fn ring_bytes(&self) -> u32 {
        self.ring_bytes
    }

    pub const fn queue_type(&self) -> u32 {
        self.queue_type
    }

    pub const fn gfx_target_version(&self) -> u32 {
        self.gfx_target_version
    }

    pub const fn xcc_count(&self) -> u32 {
        self.xcc_count
    }

    pub const fn ownership(&self) -> KfdStoppedSnapshotOwnershipV1 {
        self.ownership
    }

    pub const fn context_save(&self) -> &KfdStoppedContextSaveObservationV1 {
        &self.context_save
    }

    pub const fn opaque_checkpoint(&self) -> &KfdOpaqueCheckpointObservationV1 {
        &self.opaque_checkpoint
    }

    pub const fn hardware_checkpoint_bytes(&self) -> KfdStoppedAvailabilityV1 {
        match &self.opaque_checkpoint {
            KfdOpaqueCheckpointObservationV1::Complete(_) => KfdStoppedAvailabilityV1::Available,
            KfdOpaqueCheckpointObservationV1::Truncated(_) => {
                KfdStoppedAvailabilityV1::Unavailable(
                    KfdStoppedUnavailableReasonV1::CheckpointByteLimitExceeded,
                )
            }
            KfdOpaqueCheckpointObservationV1::Unavailable(reason) => {
                KfdStoppedAvailabilityV1::Unavailable(*reason)
            }
        }
    }

    pub const fn waves(&self) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi,
        )
    }

    pub const fn lanes(&self) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::LaneStateRequiresWaveRecords,
        )
    }

    pub const fn registers(&self) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::RegisterRecordLayoutNotInKfdUapi,
        )
    }

    pub const fn program_counter(&self) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::ProgramCounterRequiresRegisterRecord,
        )
    }

    pub const fn source_map(&self) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(KfdStoppedUnavailableReasonV1::SourceMapNotBound)
    }

    pub const fn memory_values(&self) -> KfdStoppedAvailabilityV1 {
        KfdStoppedAvailabilityV1::Unavailable(
            KfdStoppedUnavailableReasonV1::MemoryValuesNotCaptured,
        )
    }

    pub const fn manifest(&self) -> &'static str {
        KFD_STOPPED_STATE_MANIFEST_V1
    }

    pub const fn manifest_sha256(&self) -> &'static str {
        KFD_STOPPED_STATE_MANIFEST_SHA256_V1
    }
}

#[derive(Debug)]
pub enum KfdStoppedStateErrorV1 {
    Session(KfdLiveDebugSessionErrorV1),
    QueueNotSuspendedBySession,
    QueueMissing,
    DuplicateQueueIdentity,
    DeviceMissing,
    DuplicateDeviceIdentity,
    QueueBindingSubstituted,
    DeviceBindingSubstituted,
    RuntimeBindingSubstituted,
    SuspensionOwnershipLost,
}

impl fmt::Display for KfdStoppedStateErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Session(source) => write!(formatter, "live KFD debug session failed: {source}"),
            Self::QueueNotSuspendedBySession => {
                formatter.write_str("queue is not suspended by this live KFD debug session")
            }
            Self::QueueMissing => formatter.write_str("queue is absent from KFD snapshot"),
            Self::DuplicateQueueIdentity => {
                formatter.write_str("queue identity is duplicated in KFD snapshot")
            }
            Self::DeviceMissing => formatter.write_str("queue device is absent from KFD snapshot"),
            Self::DuplicateDeviceIdentity => {
                formatter.write_str("device identity is duplicated in KFD snapshot")
            }
            Self::QueueBindingSubstituted => {
                formatter.write_str("queue binding changed across stopped-state capture")
            }
            Self::DeviceBindingSubstituted => {
                formatter.write_str("device binding changed across stopped-state capture")
            }
            Self::RuntimeBindingSubstituted => {
                formatter.write_str("runtime binding changed across stopped-state capture")
            }
            Self::SuspensionOwnershipLost => {
                formatter.write_str("session suspension ownership changed during capture")
            }
        }
    }
}

impl std::error::Error for KfdStoppedStateErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Session(source) => Some(source),
            _ => None,
        }
    }
}

impl From<KfdLiveDebugSessionErrorV1> for KfdStoppedStateErrorV1 {
    fn from(source: KfdLiveDebugSessionErrorV1) -> Self {
        Self::Session(source)
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct NativeQueueBindingV1 {
    exception_status: KfdDebugExceptionMaskV1,
    queue_id: u32,
    gpu_id: u32,
    ring_size: u32,
    queue_type: u32,
    context_address: u64,
    context_bytes_per_xcc: u32,
}

impl NativeQueueBindingV1 {
    fn from_observation(observation: KfdDebugQueueObservationV1) -> Self {
        let (context_address, context_bytes_per_xcc) = observation
            .native_context_save_area()
            .unwrap_or((0, observation.context_save_area_size()));
        Self {
            exception_status: observation.exception_status(),
            queue_id: observation.queue_id(),
            gpu_id: observation.gpu_id(),
            ring_size: observation.ring_size(),
            queue_type: observation.queue_type(),
            context_address,
            context_bytes_per_xcc,
        }
    }
}

impl fmt::Debug for NativeQueueBindingV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NativeQueueBindingV1")
            .field("queue", &"<redacted>")
            .field("gpu", &"<redacted>")
            .field("exception_status", &self.exception_status)
            .field("ring_size", &self.ring_size)
            .field("queue_type", &self.queue_type)
            .field("context_bytes_per_xcc", &self.context_bytes_per_xcc)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DeviceBindingV1 {
    gpu_id: u32,
    gfx_target_version: u32,
    xcc_count: u32,
    identity_words: [u32; 5],
    geometry_words: [u32; 4],
    capability_bits: u32,
    debug_property_bits: u32,
}

impl DeviceBindingV1 {
    fn from_observation(observation: KfdDebugDeviceObservationV1) -> Self {
        Self {
            gpu_id: observation.gpu_id(),
            gfx_target_version: observation.gfx_target_version(),
            xcc_count: observation.xcc_count(),
            identity_words: observation.identity_words(),
            geometry_words: observation.geometry_words(),
            capability_bits: observation.capability_bits(),
            debug_property_bits: observation.debug_property_bits(),
        }
    }
}

trait TargetHeaderReaderV1 {
    fn read_header(
        &mut self,
        address: u64,
    ) -> Result<[u8; CONTEXT_HEADER_BYTES_V1], KfdStoppedUnavailableReasonV1>;

    fn read_checkpoint_bytes(
        &mut self,
        address: u64,
        byte_len: usize,
    ) -> Result<Zeroizing<Vec<u8>>, KfdStoppedUnavailableReasonV1>;
}

struct LinuxProcessVmHeaderReaderV1 {
    pid: libc::pid_t,
}

impl TargetHeaderReaderV1 for LinuxProcessVmHeaderReaderV1 {
    fn read_header(
        &mut self,
        address: u64,
    ) -> Result<[u8; CONTEXT_HEADER_BYTES_V1], KfdStoppedUnavailableReasonV1> {
        let remote_address = usize::try_from(address)
            .map_err(|_| KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)?;
        let mut bytes = [0_u8; CONTEXT_HEADER_BYTES_V1];
        let local = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: bytes.len(),
        };
        let remote = libc::iovec {
            iov_base: remote_address as *mut libc::c_void,
            iov_len: bytes.len(),
        };
        // SAFETY: both iovecs describe exact live fixed-size byte arrays for
        // this call. The remote address was checked for native width. The
        // ptrace/pidfd-bound live session supplies process-read authority.
        let result = unsafe { libc::process_vm_readv(self.pid, &local, 1, &remote, 1, 0) };
        if result < 0 {
            return Err(KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied);
        }
        if usize::try_from(result) != Ok(bytes.len()) {
            return Err(KfdStoppedUnavailableReasonV1::TargetHeaderReadPartial);
        }
        Ok(bytes)
    }

    fn read_checkpoint_bytes(
        &mut self,
        address: u64,
        byte_len: usize,
    ) -> Result<Zeroizing<Vec<u8>>, KfdStoppedUnavailableReasonV1> {
        let remote_address = usize::try_from(address)
            .map_err(|_| KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)?;
        let mut bytes = Zeroizing::new(vec![0_u8; byte_len]);
        if byte_len == 0 {
            return Ok(bytes);
        }
        let local = libc::iovec {
            iov_base: bytes.as_mut_ptr().cast(),
            iov_len: byte_len,
        };
        let remote = libc::iovec {
            iov_base: remote_address as *mut libc::c_void,
            iov_len: byte_len,
        };
        // SAFETY: both iovecs describe exact live byte ranges for this call.
        // The header-derived size is admitted against the gfx942 allocation
        // and the ptrace/pidfd-bound session supplies process-read authority.
        let result = unsafe { libc::process_vm_readv(self.pid, &local, 1, &remote, 1, 0) };
        if result < 0 {
            return Err(KfdStoppedUnavailableReasonV1::TargetCheckpointReadDenied);
        }
        if usize::try_from(result) != Ok(byte_len) {
            return Err(KfdStoppedUnavailableReasonV1::TargetCheckpointReadPartial);
        }
        Ok(bytes)
    }
}

fn find_queue(
    observations: &[KfdDebugQueueObservationV1],
    queue_id: u32,
) -> Result<NativeQueueBindingV1, KfdStoppedStateErrorV1> {
    let mut matches = observations
        .iter()
        .copied()
        .filter(|observation| observation.queue_id() == queue_id);
    let observation = matches.next().ok_or(KfdStoppedStateErrorV1::QueueMissing)?;
    if matches.next().is_some() {
        return Err(KfdStoppedStateErrorV1::DuplicateQueueIdentity);
    }
    Ok(NativeQueueBindingV1::from_observation(observation))
}

fn find_device(
    observations: &[KfdDebugDeviceObservationV1],
    gpu_id: u32,
) -> Result<DeviceBindingV1, KfdStoppedStateErrorV1> {
    let mut matches = observations
        .iter()
        .copied()
        .filter(|observation| observation.gpu_id() == gpu_id);
    let observation = matches
        .next()
        .ok_or(KfdStoppedStateErrorV1::DeviceMissing)?;
    if matches.next().is_some() {
        return Err(KfdStoppedStateErrorV1::DuplicateDeviceIdentity);
    }
    Ok(DeviceBindingV1::from_observation(observation))
}

fn read_u32(
    bytes: &[u8; CONTEXT_HEADER_BYTES_V1],
    offset: usize,
) -> Result<u32, KfdStoppedUnavailableReasonV1> {
    let end = offset
        .checked_add(4)
        .ok_or(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds)?;
    let wire = bytes
        .get(offset..end)
        .and_then(|wire| <[u8; 4]>::try_from(wire).ok())
        .ok_or(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds)?;
    Ok(u32::from_le_bytes(wire))
}

fn read_u64(
    bytes: &[u8; CONTEXT_HEADER_BYTES_V1],
    offset: usize,
) -> Result<u64, KfdStoppedUnavailableReasonV1> {
    let end = offset
        .checked_add(8)
        .ok_or(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds)?;
    let wire = bytes
        .get(offset..end)
        .and_then(|wire| <[u8; 8]>::try_from(wire).ok())
        .ok_or(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds)?;
    Ok(u64::from_le_bytes(wire))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DecodedHeaderV1 {
    control_stack: KfdStoppedRelativeRangeV1,
    wave_state: KfdStoppedRelativeRangeV1,
    debug: KfdStoppedRelativeRangeV1,
    error_payload_address: u64,
    error_event_id: u32,
}

fn admit_range(
    offset: u32,
    bytes: u32,
    limit: u32,
) -> Result<KfdStoppedRelativeRangeV1, KfdStoppedUnavailableReasonV1> {
    if bytes == 0 {
        if offset == 0 {
            return Ok(KfdStoppedRelativeRangeV1 { offset, bytes });
        }
        if offset < CONTEXT_HEADER_BYTES_U32_V1 {
            return Err(KfdStoppedUnavailableReasonV1::ContextHeaderRangePairMalformed);
        }
        if offset > limit {
            return Err(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds);
        }
        return Ok(KfdStoppedRelativeRangeV1 { offset, bytes });
    }
    if offset == 0 {
        return Err(KfdStoppedUnavailableReasonV1::ContextHeaderRangePairMalformed);
    }
    let end = offset
        .checked_add(bytes)
        .ok_or(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds)?;
    if offset < CONTEXT_HEADER_BYTES_U32_V1 || end > limit {
        return Err(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds);
    }
    Ok(KfdStoppedRelativeRangeV1 { offset, bytes })
}

fn ranges_overlap(left: KfdStoppedRelativeRangeV1, right: KfdStoppedRelativeRangeV1) -> bool {
    if left.is_empty() || right.is_empty() {
        return false;
    }
    let Some(left_end) = left.offset.checked_add(left.bytes) else {
        return true;
    };
    let Some(right_end) = right.offset.checked_add(right.bytes) else {
        return true;
    };
    left.offset < right_end && right.offset < left_end
}

fn decode_header(
    bytes: &[u8; CONTEXT_HEADER_BYTES_V1],
    xcc: usize,
) -> Result<DecodedHeaderV1, KfdStoppedUnavailableReasonV1> {
    if read_u32(bytes, 36)? != 0 {
        return Err(KfdStoppedUnavailableReasonV1::ContextHeaderReservedNonzero);
    }
    let control_stack = admit_range(
        read_u32(bytes, 0)?,
        read_u32(bytes, 4)?,
        GFX942_CONTEXT_BYTES_PER_XCC_V1,
    )?;
    let wave_state = admit_range(
        read_u32(bytes, 8)?,
        read_u32(bytes, 12)?,
        GFX942_CONTEXT_BYTES_PER_XCC_V1,
    )?;
    if ranges_overlap(control_stack, wave_state) {
        return Err(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOverlap);
    }
    let remaining_xcc = GFX942_XCC_COUNT_V1
        .checked_sub(xcc)
        .ok_or(KfdStoppedUnavailableReasonV1::Gfx942DebugRangeMismatch)?;
    let context_bytes = usize::try_from(GFX942_CONTEXT_BYTES_PER_XCC_V1)
        .map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942SaveAreaSizeMismatch)?;
    let expected_debug_offset = remaining_xcc
        .checked_mul(context_bytes)
        .and_then(|offset| u32::try_from(offset).ok())
        .ok_or(KfdStoppedUnavailableReasonV1::Gfx942DebugRangeMismatch)?;
    let debug = KfdStoppedRelativeRangeV1 {
        offset: read_u32(bytes, 16)?,
        bytes: read_u32(bytes, 20)?,
    };
    if debug.offset != expected_debug_offset || debug.bytes != GFX942_DEBUG_BYTES_V1 {
        return Err(KfdStoppedUnavailableReasonV1::Gfx942DebugRangeMismatch);
    }
    Ok(DecodedHeaderV1 {
        control_stack,
        wave_state,
        debug,
        error_payload_address: read_u64(bytes, 24)?,
        error_event_id: read_u32(bytes, 32)?,
    })
}

fn hash_start(domain: &[u8], scope: KfdStoppedStateScopeV1) -> Sha256 {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-kfd-stopped-state-v1\0");
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain);
    hash.update(scope.0);
    hash
}

fn hash_u32(hash: &mut Sha256, value: u32) {
    hash.update(value.to_le_bytes());
}

fn hash_u64(hash: &mut Sha256, value: u64) {
    hash.update(value.to_le_bytes());
}

fn finish_hash(hash: Sha256) -> KfdStoppedLogicalIdentityV1 {
    KfdStoppedLogicalIdentityV1(hash.finalize().into())
}

fn queue_identity(
    scope: KfdStoppedStateScopeV1,
    queue: NativeQueueBindingV1,
) -> KfdStoppedLogicalIdentityV1 {
    let mut hash = hash_start(b"queue", scope);
    hash_u32(&mut hash, queue.queue_id);
    hash_u32(&mut hash, queue.gpu_id);
    hash_u32(&mut hash, queue.ring_size);
    hash_u32(&mut hash, queue.queue_type);
    hash_u64(&mut hash, queue.context_address);
    hash_u32(&mut hash, queue.context_bytes_per_xcc);
    finish_hash(hash)
}

fn device_identity(
    scope: KfdStoppedStateScopeV1,
    device: DeviceBindingV1,
) -> KfdStoppedLogicalIdentityV1 {
    let mut hash = hash_start(b"device", scope);
    hash_u32(&mut hash, device.gpu_id);
    hash_u32(&mut hash, device.gfx_target_version);
    hash_u32(&mut hash, device.xcc_count);
    for word in device
        .identity_words
        .into_iter()
        .chain(device.geometry_words)
    {
        hash_u32(&mut hash, word);
    }
    hash_u32(&mut hash, device.capability_bits);
    hash_u32(&mut hash, device.debug_property_bits);
    finish_hash(hash)
}

fn runtime_state_tag(state: KfdDebugRuntimeStateV1) -> u8 {
    match state {
        KfdDebugRuntimeStateV1::Disabled => 0,
        KfdDebugRuntimeStateV1::Enabled => 1,
        KfdDebugRuntimeStateV1::EnabledBusy => 2,
        KfdDebugRuntimeStateV1::EnabledError => 3,
    }
}

fn session_identity(
    scope: KfdStoppedStateScopeV1,
    session: &KfdLiveDebugSessionV1,
) -> KfdStoppedLogicalIdentityV1 {
    let runtime = session.runtime_observation();
    let mut hash = hash_start(b"live-debug-session", scope);
    hash_u32(&mut hash, session.target_pid());
    hash_u64(&mut hash, session.enabled_exceptions().bits());
    hash.update([runtime_state_tag(runtime.state())]);
    hash.update([u8::from(runtime.ttmp_setup())]);
    hash.update([u8::from(runtime.runtime_metadata_present())]);
    hash.update(crate::KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_SHA256_V1.as_bytes());
    finish_hash(hash)
}

fn unavailable_context(
    reason: KfdStoppedUnavailableReasonV1,
) -> KfdStoppedContextSaveObservationV1 {
    KfdStoppedContextSaveObservationV1::Unavailable(reason)
}

struct ValidatedContextCaptureV1 {
    decoded: [DecodedHeaderV1; GFX942_XCC_COUNT_V1],
    wire_headers: [[u8; CONTEXT_HEADER_BYTES_V1]; GFX942_XCC_COUNT_V1],
    save_identity: KfdStoppedLogicalIdentityV1,
}

struct ContextCaptureV1 {
    observation: KfdStoppedContextSaveObservationV1,
    validated: Option<ValidatedContextCaptureV1>,
}

fn capture_context_layout<R: TargetHeaderReaderV1>(
    reader: &mut R,
    scope: KfdStoppedStateScopeV1,
    queue: NativeQueueBindingV1,
    device: DeviceBindingV1,
) -> ContextCaptureV1 {
    let capture = try_capture_context_layout(reader, scope, queue, device);
    match capture {
        Ok((layout, validated)) => ContextCaptureV1 {
            observation: KfdStoppedContextSaveObservationV1::Available(Box::new(layout)),
            validated: Some(validated),
        },
        Err(reason) => ContextCaptureV1 {
            observation: unavailable_context(reason),
            validated: None,
        },
    }
}

fn try_capture_context_layout<R: TargetHeaderReaderV1>(
    reader: &mut R,
    scope: KfdStoppedStateScopeV1,
    queue: NativeQueueBindingV1,
    device: DeviceBindingV1,
) -> Result<(KfdGfx942ContextSaveLayoutV1, ValidatedContextCaptureV1), KfdStoppedUnavailableReasonV1>
{
    if queue.context_address == 0 || queue.context_bytes_per_xcc == 0 {
        return Err(KfdStoppedUnavailableReasonV1::ContextSaveAreaNotReported);
    }
    if device.gfx_target_version != GFX942_TARGET_VERSION_V1 {
        return Err(KfdStoppedUnavailableReasonV1::GfxTargetNotGfx942);
    }
    if usize::try_from(device.xcc_count) != Ok(GFX942_XCC_COUNT_V1) {
        return Err(KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch);
    }
    if queue.context_bytes_per_xcc != GFX942_CONTEXT_BYTES_PER_XCC_V1 {
        return Err(KfdStoppedUnavailableReasonV1::Gfx942SaveAreaSizeMismatch);
    }

    let mut decoded = Vec::with_capacity(MAX_CONTEXT_HEADERS_V1);
    let mut wire_headers = Vec::with_capacity(MAX_CONTEXT_HEADERS_V1);
    for xcc in 0..GFX942_XCC_COUNT_V1 {
        let xcc_u64 = u64::try_from(xcc)
            .map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
        let offset = u64::from(queue.context_bytes_per_xcc)
            .checked_mul(xcc_u64)
            .ok_or(KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)?;
        let address = queue
            .context_address
            .checked_add(offset)
            .ok_or(KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)?;
        let bytes = reader.read_header(address)?;
        let header = decode_header(&bytes, xcc)?;
        wire_headers.push(bytes);
        decoded.push(header);
    }

    let first = decoded
        .first()
        .copied()
        .ok_or(KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
    if decoded.iter().skip(1).any(|header| {
        header.error_payload_address != first.error_payload_address
            || header.error_event_id != first.error_event_id
    }) {
        return Err(KfdStoppedUnavailableReasonV1::ContextHeaderBindingSubstituted);
    }

    let total_allocation_bytes =
        u64::from(queue.context_bytes_per_xcc) * 8_u64 + u64::from(GFX942_DEBUG_BYTES_V1);
    let mut save_hash = hash_start(b"context-save-area", scope);
    hash_u64(&mut save_hash, queue.context_address);
    hash_u32(&mut save_hash, queue.context_bytes_per_xcc);
    hash_u64(&mut save_hash, total_allocation_bytes);
    for bytes in &wire_headers {
        save_hash.update(bytes);
    }
    let save_identity = finish_hash(save_hash);

    let decoded: [DecodedHeaderV1; GFX942_XCC_COUNT_V1] = decoded
        .try_into()
        .map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
    let wire_headers: [[u8; CONTEXT_HEADER_BYTES_V1]; GFX942_XCC_COUNT_V1] = wire_headers
        .try_into()
        .map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
    let mut observed_headers = Vec::with_capacity(GFX942_XCC_COUNT_V1);
    for (xcc, (header, wire)) in decoded.iter().copied().zip(wire_headers).enumerate() {
        let mut hash = hash_start(b"context-save-xcc", scope);
        hash.update(save_identity.as_bytes());
        let xcc_u32 = u32::try_from(xcc)
            .map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
        let xcc_ordinal =
            u8::try_from(xcc).map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
        hash_u32(&mut hash, xcc_u32);
        hash.update(wire);
        observed_headers.push(KfdGfx942CwsrHeaderObservationV1 {
            xcc_ordinal,
            logical_identity: finish_hash(hash),
            control_stack: header.control_stack,
            wave_state: header.wave_state,
            debug: header.debug,
            error_binding_present: header.error_payload_address != 0 && header.error_event_id != 0,
        });
    }
    let headers = observed_headers
        .try_into()
        .map_err(|_| KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch)?;
    Ok((
        KfdGfx942ContextSaveLayoutV1 {
            logical_identity: save_identity,
            context_bytes_per_xcc: queue.context_bytes_per_xcc,
            total_allocation_bytes,
            headers,
        },
        ValidatedContextCaptureV1 {
            decoded,
            wire_headers,
            save_identity,
        },
    ))
}

fn checkpoint_required_bytes(
    context: &ValidatedContextCaptureV1,
) -> Result<u64, KfdStoppedUnavailableReasonV1> {
    context.decoded.iter().try_fold(0_u64, |total, header| {
        total
            .checked_add(u64::from(header.control_stack.bytes))
            .and_then(|total| total.checked_add(u64::from(header.wave_state.bytes)))
            .ok_or(KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds)
    })
}

fn checkpoint_segment_address(
    queue: NativeQueueBindingV1,
    xcc: usize,
    range: KfdStoppedRelativeRangeV1,
) -> Result<u64, KfdStoppedUnavailableReasonV1> {
    let xcc = u64::try_from(xcc)
        .map_err(|_| KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)?;
    queue
        .context_address
        .checked_add(
            u64::from(queue.context_bytes_per_xcc)
                .checked_mul(xcc)
                .ok_or(KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)?,
        )
        .and_then(|base| base.checked_add(u64::from(range.offset)))
        .ok_or(KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable)
}

#[derive(Clone, Copy)]
struct OpaqueCheckpointBindingV1 {
    queue: NativeQueueBindingV1,
    session_identity: KfdStoppedLogicalIdentityV1,
    queue_identity: KfdStoppedLogicalIdentityV1,
    device_identity: KfdStoppedLogicalIdentityV1,
}

fn capture_opaque_checkpoint<R: TargetHeaderReaderV1>(
    reader: &mut R,
    scope: KfdStoppedStateScopeV1,
    context: &ValidatedContextCaptureV1,
    binding: OpaqueCheckpointBindingV1,
    byte_limit: u64,
) -> KfdOpaqueCheckpointObservationV1 {
    let required_bytes = match checkpoint_required_bytes(context) {
        Ok(bytes) => bytes,
        Err(reason) => return KfdOpaqueCheckpointObservationV1::Unavailable(reason),
    };
    if required_bytes > byte_limit {
        return KfdOpaqueCheckpointObservationV1::Truncated(KfdOpaqueCheckpointTruncationV1 {
            required_bytes,
            capture_limit_bytes: byte_limit,
        });
    }

    let mut segments = Vec::with_capacity(MAX_CHECKPOINT_SEGMENTS_V1);
    let mut content_hash = hash_start(b"opaque-checkpoint-content", scope);
    content_hash.update(context.save_identity.as_bytes());
    hash_u64(&mut content_hash, required_bytes);
    let mut captured_bytes = 0_u64;
    for (xcc, header) in context.decoded.iter().copied().enumerate() {
        for (kind, range) in [
            (
                KfdOpaqueCheckpointSegmentKindV1::ControlStack,
                header.control_stack,
            ),
            (
                KfdOpaqueCheckpointSegmentKindV1::WaveState,
                header.wave_state,
            ),
        ] {
            if range.is_empty() {
                continue;
            }
            if segments.len() == MAX_CHECKPOINT_SEGMENTS_V1 {
                return KfdOpaqueCheckpointObservationV1::Unavailable(
                    KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds,
                );
            }
            let address = match checkpoint_segment_address(binding.queue, xcc, range) {
                Ok(address) => address,
                Err(reason) => return KfdOpaqueCheckpointObservationV1::Unavailable(reason),
            };
            let byte_len = match usize::try_from(range.bytes) {
                Ok(byte_len) => byte_len,
                Err(_) => {
                    return KfdOpaqueCheckpointObservationV1::Unavailable(
                        KfdStoppedUnavailableReasonV1::TargetAddressNotRepresentable,
                    );
                }
            };
            let bytes = match reader.read_checkpoint_bytes(address, byte_len) {
                Ok(bytes) => bytes,
                Err(reason) => return KfdOpaqueCheckpointObservationV1::Unavailable(reason),
            };
            let confirmation = match reader.read_checkpoint_bytes(address, byte_len) {
                Ok(bytes) => bytes,
                Err(reason) => return KfdOpaqueCheckpointObservationV1::Unavailable(reason),
            };
            if bytes.as_slice() != confirmation.as_slice() {
                return KfdOpaqueCheckpointObservationV1::Unavailable(
                    KfdStoppedUnavailableReasonV1::CheckpointContentChanged,
                );
            }
            let xcc_ordinal = match u8::try_from(xcc) {
                Ok(xcc) => xcc,
                Err(_) => {
                    return KfdOpaqueCheckpointObservationV1::Unavailable(
                        KfdStoppedUnavailableReasonV1::Gfx942XccCountMismatch,
                    );
                }
            };
            let kind_tag = match kind {
                KfdOpaqueCheckpointSegmentKindV1::ControlStack => 0_u8,
                KfdOpaqueCheckpointSegmentKindV1::WaveState => 1_u8,
            };
            content_hash.update([xcc_ordinal, kind_tag]);
            hash_u32(&mut content_hash, range.offset);
            hash_u32(&mut content_hash, range.bytes);
            content_hash.update(&*bytes);
            let mut segment_hash = hash_start(b"opaque-checkpoint-segment", scope);
            segment_hash.update(context.save_identity.as_bytes());
            segment_hash.update([xcc_ordinal, kind_tag]);
            hash_u32(&mut segment_hash, range.offset);
            hash_u32(&mut segment_hash, range.bytes);
            segment_hash.update(&*bytes);
            captured_bytes = match captured_bytes.checked_add(u64::from(range.bytes)) {
                Some(captured_bytes) => captured_bytes,
                None => {
                    return KfdOpaqueCheckpointObservationV1::Unavailable(
                        KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds,
                    );
                }
            };
            segments.push(KfdOpaqueCheckpointSegmentV1 {
                xcc_ordinal,
                kind,
                range,
                content_identity: finish_hash(segment_hash),
                bytes,
            });
        }
    }
    if captured_bytes != required_bytes {
        return KfdOpaqueCheckpointObservationV1::Unavailable(
            KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds,
        );
    }
    for (xcc, expected) in context.wire_headers.iter().enumerate() {
        let address = match checkpoint_segment_address(
            binding.queue,
            xcc,
            KfdStoppedRelativeRangeV1 {
                offset: 0,
                bytes: CONTEXT_HEADER_BYTES_U32_V1,
            },
        ) {
            Ok(address) => address,
            Err(reason) => return KfdOpaqueCheckpointObservationV1::Unavailable(reason),
        };
        let observed = match reader.read_header(address) {
            Ok(bytes) => bytes,
            Err(reason) => return KfdOpaqueCheckpointObservationV1::Unavailable(reason),
        };
        if observed != *expected {
            return KfdOpaqueCheckpointObservationV1::Unavailable(
                KfdStoppedUnavailableReasonV1::ContextHeaderBindingSubstituted,
            );
        }
    }
    let content_identity = finish_hash(content_hash);
    let mut checkpoint_hash = hash_start(b"opaque-checkpoint", scope);
    checkpoint_hash.update(binding.session_identity.as_bytes());
    checkpoint_hash.update(binding.queue_identity.as_bytes());
    checkpoint_hash.update(binding.device_identity.as_bytes());
    checkpoint_hash.update(context.save_identity.as_bytes());
    checkpoint_hash.update(content_identity.as_bytes());
    hash_u64(&mut checkpoint_hash, captured_bytes);
    hash_u64(&mut checkpoint_hash, byte_limit);
    KfdOpaqueCheckpointObservationV1::Complete(KfdOpaqueCheckpointV1 {
        logical_identity: finish_hash(checkpoint_hash),
        content_identity,
        captured_bytes,
        segments,
    })
}

fn build_snapshot<R: TargetHeaderReaderV1>(
    reader: &mut R,
    scope: KfdStoppedStateScopeV1,
    queue: NativeQueueBindingV1,
    device: DeviceBindingV1,
    session_identity: KfdStoppedLogicalIdentityV1,
    checkpoint_byte_limit: u64,
) -> KfdStoppedQueueSnapshotV1 {
    let queue_identity = queue_identity(scope, queue);
    let device_identity = device_identity(scope, device);
    let context_capture = capture_context_layout(reader, scope, queue, device);
    let opaque_checkpoint = match context_capture.validated.as_ref() {
        Some(context) => capture_opaque_checkpoint(
            reader,
            scope,
            context,
            OpaqueCheckpointBindingV1 {
                queue,
                session_identity,
                queue_identity,
                device_identity,
            },
            checkpoint_byte_limit,
        ),
        None => match &context_capture.observation {
            KfdStoppedContextSaveObservationV1::Unavailable(reason) => {
                KfdOpaqueCheckpointObservationV1::Unavailable(*reason)
            }
            KfdStoppedContextSaveObservationV1::Available(_) => {
                KfdOpaqueCheckpointObservationV1::Unavailable(
                    KfdStoppedUnavailableReasonV1::ContextHeaderBindingSubstituted,
                )
            }
        },
    };
    let context_save = context_capture.observation;
    let mut hash = hash_start(b"snapshot", scope);
    hash.update(queue_identity.as_bytes());
    hash.update(device_identity.as_bytes());
    hash_u64(&mut hash, queue.exception_status.bits());
    hash_u32(&mut hash, queue.ring_size);
    hash_u32(&mut hash, queue.queue_type);
    match &context_save {
        KfdStoppedContextSaveObservationV1::Available(layout) => {
            hash.update([1]);
            hash.update(layout.logical_identity().as_bytes());
        }
        KfdStoppedContextSaveObservationV1::Unavailable(reason) => {
            hash.update([0]);
            hash_u32(&mut hash, *reason as u32);
        }
    }
    match &opaque_checkpoint {
        KfdOpaqueCheckpointObservationV1::Complete(checkpoint) => {
            hash.update([2]);
            hash.update(checkpoint.logical_identity().as_bytes());
        }
        KfdOpaqueCheckpointObservationV1::Truncated(truncation) => {
            hash.update([1]);
            hash_u64(&mut hash, truncation.required_bytes());
            hash_u64(&mut hash, truncation.capture_limit_bytes());
        }
        KfdOpaqueCheckpointObservationV1::Unavailable(reason) => {
            hash.update([0]);
            hash_u32(&mut hash, *reason as u32);
        }
    }
    KfdStoppedQueueSnapshotV1 {
        logical_identity: finish_hash(hash),
        queue_identity,
        device_identity,
        exception_status: queue.exception_status,
        ring_bytes: queue.ring_size,
        queue_type: queue.queue_type,
        gfx_target_version: device.gfx_target_version,
        xcc_count: device.xcc_count,
        ownership: KfdStoppedSnapshotOwnershipV1::SessionRetainedSuspension,
        context_save,
        opaque_checkpoint,
    }
}

impl KfdLiveDebugSessionV1 {
    /// Captures a bounded, detached stopped-state observation for a queue whose
    /// suspension is already owned by this exact session.
    ///
    /// The method leaves the queue suspended. Call [`Self::resume_queues`]
    /// explicitly after consuming the report. A returned report is inert and
    /// cannot retain, transfer, or release native suspension authority. KFD
    /// queue/device snapshots are reobserved, but suspension and runtime state
    /// are only local session invariants at this boundary.
    pub fn capture_stopped_queue_v1(
        &mut self,
        plan: KfdStoppedQueueCapturePlanV1,
    ) -> Result<KfdStoppedQueueSnapshotV1, KfdStoppedStateErrorV1> {
        if !self.owns_suspended_queue(plan.queue_id) {
            return Err(KfdStoppedStateErrorV1::QueueNotSuspendedBySession);
        }
        let session_binding_identity = session_identity(plan.scope, self);
        let before_queues = self.queue_snapshot(KfdDebugExceptionMaskV1::NONE)?;
        let before_queue = find_queue(&before_queues, plan.queue_id)?;
        let before_devices = self.device_snapshot(KfdDebugExceptionMaskV1::NONE)?;
        let before_device = find_device(&before_devices, before_queue.gpu_id)?;
        if !self.owns_suspended_queue(plan.queue_id) {
            return Err(KfdStoppedStateErrorV1::SuspensionOwnershipLost);
        }

        let pid = libc::pid_t::try_from(self.target_pid())
            .map_err(|_| KfdStoppedStateErrorV1::QueueBindingSubstituted)?;
        let mut reader = LinuxProcessVmHeaderReaderV1 { pid };
        let snapshot = build_snapshot(
            &mut reader,
            plan.scope,
            before_queue,
            before_device,
            session_binding_identity,
            plan.checkpoint_byte_limit,
        );

        let after_queues = self.queue_snapshot(KfdDebugExceptionMaskV1::NONE)?;
        let after_queue = find_queue(&after_queues, plan.queue_id)?;
        if before_queue != after_queue {
            return Err(KfdStoppedStateErrorV1::QueueBindingSubstituted);
        }
        let after_devices = self.device_snapshot(KfdDebugExceptionMaskV1::NONE)?;
        let after_device = find_device(&after_devices, after_queue.gpu_id)?;
        if before_device != after_device {
            return Err(KfdStoppedStateErrorV1::DeviceBindingSubstituted);
        }
        if !self.owns_suspended_queue(plan.queue_id) {
            return Err(KfdStoppedStateErrorV1::SuspensionOwnershipLost);
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: u64 = 0x0000_7f12_3450_0000;
    const EVENT: u32 = 23;
    const PAYLOAD: u64 = 0x0000_7f12_3450_0fc0;

    struct FixtureReader {
        base: u64,
        headers: [[u8; CONTEXT_HEADER_BYTES_V1]; GFX942_XCC_COUNT_V1],
        fail_at: Option<(usize, KfdStoppedUnavailableReasonV1)>,
        checkpoint_failure: Option<KfdStoppedUnavailableReasonV1>,
        mutate_confirmation: bool,
        mutate_header_confirmation: bool,
        checkpoint_fill: Option<u8>,
        checkpoint_reads: usize,
        header_reads: usize,
    }

    impl TargetHeaderReaderV1 for FixtureReader {
        fn read_header(
            &mut self,
            address: u64,
        ) -> Result<[u8; CONTEXT_HEADER_BYTES_V1], KfdStoppedUnavailableReasonV1> {
            self.header_reads += 1;
            let offset = address
                .checked_sub(self.base)
                .ok_or(KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied)?;
            if !offset.is_multiple_of(u64::from(GFX942_CONTEXT_BYTES_PER_XCC_V1)) {
                return Err(KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied);
            }
            let xcc = usize::try_from(offset / u64::from(GFX942_CONTEXT_BYTES_PER_XCC_V1))
                .map_err(|_| KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied)?;
            if let Some((failed_xcc, reason)) = self.fail_at
                && xcc == failed_xcc
            {
                return Err(reason);
            }
            let mut header = self
                .headers
                .get(xcc)
                .copied()
                .ok_or(KfdStoppedUnavailableReasonV1::TargetHeaderReadDenied)?;
            if self.mutate_header_confirmation
                && self.header_reads > GFX942_XCC_COUNT_V1
                && xcc == 0
            {
                header[32] ^= 1;
            }
            Ok(header)
        }

        fn read_checkpoint_bytes(
            &mut self,
            address: u64,
            byte_len: usize,
        ) -> Result<Zeroizing<Vec<u8>>, KfdStoppedUnavailableReasonV1> {
            if let Some(reason) = self.checkpoint_failure {
                return Err(reason);
            }
            self.checkpoint_reads += 1;
            let mutation =
                u8::from(self.mutate_confirmation && self.checkpoint_reads.is_multiple_of(2));
            let bytes = (0..byte_len)
                .map(|index| {
                    self.checkpoint_fill
                        .unwrap_or_else(|| (address as u8).wrapping_add(index as u8))
                        ^ mutation
                })
                .collect();
            Ok(Zeroizing::new(bytes))
        }
    }

    fn header(xcc: usize) -> [u8; CONTEXT_HEADER_BYTES_V1] {
        let mut bytes = [0_u8; CONTEXT_HEADER_BYTES_V1];
        let debug_offset = ((GFX942_XCC_COUNT_V1 - xcc)
            * usize::try_from(GFX942_CONTEXT_BYTES_PER_XCC_V1).unwrap())
            as u32;
        bytes[16..20].copy_from_slice(&debug_offset.to_le_bytes());
        bytes[20..24].copy_from_slice(&GFX942_DEBUG_BYTES_V1.to_le_bytes());
        bytes[24..32].copy_from_slice(&PAYLOAD.to_le_bytes());
        bytes[32..36].copy_from_slice(&EVENT.to_le_bytes());
        bytes
    }

    fn reader() -> FixtureReader {
        FixtureReader {
            base: BASE,
            headers: std::array::from_fn(header),
            fail_at: None,
            checkpoint_failure: None,
            mutate_confirmation: false,
            mutate_header_confirmation: false,
            checkpoint_fill: None,
            checkpoint_reads: 0,
            header_reads: 0,
        }
    }

    fn queue() -> NativeQueueBindingV1 {
        NativeQueueBindingV1 {
            exception_status: KfdDebugExceptionMaskV1::NONE,
            queue_id: 7,
            gpu_id: 19,
            ring_size: 4096,
            queue_type: 0,
            context_address: BASE,
            context_bytes_per_xcc: GFX942_CONTEXT_BYTES_PER_XCC_V1,
        }
    }

    fn device() -> DeviceBindingV1 {
        DeviceBindingV1 {
            gpu_id: 19,
            gfx_target_version: GFX942_TARGET_VERSION_V1,
            xcc_count: GFX942_XCC_COUNT_V1 as u32,
            identity_words: [1, 2, 3, 4, 5],
            geometry_words: [304, 8, 8, 1],
            capability_bits: 0x2007_8080,
            debug_property_bits: 0x400,
        }
    }

    fn scope() -> KfdStoppedStateScopeV1 {
        KfdStoppedStateScopeV1::new([0x5a; 32]).unwrap()
    }

    fn unavailable(capture: ContextCaptureV1) -> KfdStoppedUnavailableReasonV1 {
        match capture.observation {
            KfdStoppedContextSaveObservationV1::Unavailable(reason) => reason,
            KfdStoppedContextSaveObservationV1::Available(_) => panic!("expected unavailable"),
        }
    }

    fn test_session_identity() -> KfdStoppedLogicalIdentityV1 {
        KfdStoppedLogicalIdentityV1([0x44; 32])
    }

    fn snapshot(reader: &mut FixtureReader, byte_limit: u64) -> KfdStoppedQueueSnapshotV1 {
        snapshot_with_bindings(reader, queue(), device(), byte_limit)
    }

    fn snapshot_with_bindings(
        reader: &mut FixtureReader,
        queue: NativeQueueBindingV1,
        device: DeviceBindingV1,
        byte_limit: u64,
    ) -> KfdStoppedQueueSnapshotV1 {
        build_snapshot(
            reader,
            scope(),
            queue,
            device,
            test_session_identity(),
            byte_limit,
        )
    }

    fn set_checkpoint_ranges(reader: &mut FixtureReader) {
        reader.headers[0][0..4].copy_from_slice(&64_u32.to_le_bytes());
        reader.headers[0][4..8].copy_from_slice(&128_u32.to_le_bytes());
        reader.headers[0][8..12].copy_from_slice(&4096_u32.to_le_bytes());
        reader.headers[0][12..16].copy_from_slice(&256_u32.to_le_bytes());
    }

    #[test]
    fn manifest_digest_and_bounds_are_frozen() {
        assert_eq!(CONTEXT_HEADER_BYTES_V1, 40);
        assert_eq!(CONTEXT_HEADER_BYTES_U32_V1, 40);
        assert_eq!(MAX_CONTEXT_HEADERS_V1, 8);
        assert_eq!(MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1, 185_630_720);
        assert!(KFD_STOPPED_STATE_MANIFEST_V1.contains("hard-opaque-checkpoint:185630720"));
        assert!(
            KfdStoppedQueueCapturePlanV1::with_checkpoint_byte_limit(
                7,
                scope(),
                MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1,
            )
            .is_ok()
        );
        assert!(matches!(
            KfdStoppedQueueCapturePlanV1::with_checkpoint_byte_limit(
                7,
                scope(),
                MAX_KFD_OPAQUE_CHECKPOINT_BYTES_V1 + 1,
            ),
            Err(KfdStoppedStatePlanErrorV1::CheckpointByteLimitExceeded)
        ));
        let digest = Sha256::digest(KFD_STOPPED_STATE_MANIFEST_V1.as_bytes());
        let actual = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(actual, KFD_STOPPED_STATE_MANIFEST_SHA256_V1);
    }

    #[test]
    fn zero_scope_is_rejected_and_debug_is_redacted() {
        assert_eq!(
            KfdStoppedStateScopeV1::new([0; 32]),
            Err(KfdStoppedStateScopeErrorV1::Zero)
        );
        let plan = KfdStoppedQueueCapturePlanV1::new(0xdead_beef, scope());
        let debug = format!("{plan:?}");
        assert!(!debug.contains("dead"));
        assert!(!debug.contains("5a5a"));
    }

    #[test]
    fn exact_gfx942_header_envelope_is_admitted_without_wave_claims() {
        let snapshot = snapshot(&mut reader(), DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1);
        let layout = match snapshot.context_save() {
            KfdStoppedContextSaveObservationV1::Available(layout) => layout,
            other => panic!("unexpected context observation: {other:?}"),
        };
        assert_eq!(layout.context_bytes_per_xcc(), 0x162_1000);
        assert_eq!(layout.total_allocation_bytes(), 0xb16_7000);
        assert_eq!(layout.headers().len(), 8);
        for (xcc, header) in layout.headers().iter().enumerate() {
            assert_eq!(header.xcc_ordinal(), xcc as u8);
            assert!(header.control_stack().is_empty());
            assert!(header.wave_state().is_empty());
            assert_eq!(header.debug().bytes(), GFX942_DEBUG_BYTES_V1);
            assert!(header.error_binding_present());
        }
        assert_eq!(
            snapshot.hardware_checkpoint_bytes(),
            KfdStoppedAvailabilityV1::Available
        );
        assert_eq!(
            snapshot.waves(),
            KfdStoppedAvailabilityV1::Unavailable(
                KfdStoppedUnavailableReasonV1::WaveRecordLayoutNotInKfdUapi
            )
        );
    }

    #[test]
    fn bounded_empty_range_cursors_are_preserved_without_content_claims() {
        let mut fixture = reader();
        for header in &mut fixture.headers {
            header[0..4].copy_from_slice(&0x3000_u32.to_le_bytes());
            header[8..12].copy_from_slice(&0x3000_u32.to_le_bytes());
        }
        let snapshot = snapshot(&mut fixture, DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1);
        let layout = match snapshot.context_save() {
            KfdStoppedContextSaveObservationV1::Available(layout) => layout,
            other => panic!("unexpected context observation: {other:?}"),
        };
        assert!(layout.headers().iter().all(|header| {
            header.control_stack().offset() == 0x3000
                && header.control_stack().is_empty()
                && header.wave_state().offset() == 0x3000
                && header.wave_state().is_empty()
        }));
        assert!(matches!(
            snapshot.opaque_checkpoint(),
            KfdOpaqueCheckpointObservationV1::Complete(checkpoint)
                if checkpoint.captured_bytes() == 0 && checkpoint.segments().is_empty()
        ));
    }

    #[test]
    fn debug_and_report_do_not_serialize_native_identifiers() {
        let snapshot = snapshot(&mut reader(), DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1);
        let debug = format!("{snapshot:?}");
        for forbidden in [
            "deadbeef",
            "7f1234500000",
            "7f1234500fc0",
            "queue_id: ",
            "gpu_id: ",
            "event_id: ",
            "payload_address",
            "context_address",
        ] {
            assert!(!debug.contains(forbidden), "leaked {forbidden}: {debug}");
        }
    }

    #[test]
    fn opaque_checkpoint_debug_never_exposes_private_bytes() {
        let mut fixture = reader();
        set_checkpoint_ranges(&mut fixture);
        fixture.checkpoint_fill = Some(0xa5);
        let snapshot = snapshot(&mut fixture, DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1);
        let checkpoint = match snapshot.opaque_checkpoint() {
            KfdOpaqueCheckpointObservationV1::Complete(checkpoint) => checkpoint,
            other => panic!("unexpected checkpoint observation: {other:?}"),
        };
        assert_eq!(checkpoint.captured_bytes(), 384);
        assert_eq!(checkpoint.segments().len(), 2);
        assert!(
            checkpoint
                .segments()
                .iter()
                .all(|segment| segment.with_private_bytes(|bytes| {
                    !bytes.is_empty() && bytes.iter().all(|byte| *byte == 0xa5)
                }))
        );
        let debug = format!("{snapshot:?}");
        assert!(debug.contains("<private>"));
        assert!(!debug.contains("165, 165"), "private bytes leaked: {debug}");
        assert!(!debug.contains("a5a5a5a5"), "private bytes leaked: {debug}");
    }

    #[test]
    fn byte_limit_truncation_reads_and_retains_no_segment_prefix() {
        let mut fixture = reader();
        set_checkpoint_ranges(&mut fixture);
        let snapshot = snapshot(&mut fixture, 383);
        assert_eq!(fixture.checkpoint_reads, 0);
        match snapshot.opaque_checkpoint() {
            KfdOpaqueCheckpointObservationV1::Truncated(truncation) => {
                assert_eq!(truncation.required_bytes(), 384);
                assert_eq!(truncation.capture_limit_bytes(), 383);
            }
            other => panic!("unexpected checkpoint observation: {other:?}"),
        }
        assert_eq!(
            snapshot.hardware_checkpoint_bytes(),
            KfdStoppedAvailabilityV1::Unavailable(
                KfdStoppedUnavailableReasonV1::CheckpointByteLimitExceeded
            )
        );
    }

    #[test]
    fn checkpoint_content_change_fails_closed() {
        let mut fixture = reader();
        set_checkpoint_ranges(&mut fixture);
        fixture.mutate_confirmation = true;
        let snapshot = snapshot(&mut fixture, DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1);
        assert!(matches!(
            snapshot.opaque_checkpoint(),
            KfdOpaqueCheckpointObservationV1::Unavailable(
                KfdStoppedUnavailableReasonV1::CheckpointContentChanged
            )
        ));
    }

    #[test]
    fn checkpoint_identity_binds_exact_queue_and_device_observations() {
        let mut first_reader = reader();
        set_checkpoint_ranges(&mut first_reader);
        let first = snapshot(&mut first_reader, DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1);

        let mut changed_queue = queue();
        changed_queue.queue_id += 1;
        let mut queue_reader = reader();
        set_checkpoint_ranges(&mut queue_reader);
        let queue_changed = snapshot_with_bindings(
            &mut queue_reader,
            changed_queue,
            device(),
            DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1,
        );

        let mut changed_device = device();
        changed_device.identity_words[0] += 1;
        let mut device_reader = reader();
        set_checkpoint_ranges(&mut device_reader);
        let device_changed = snapshot_with_bindings(
            &mut device_reader,
            queue(),
            changed_device,
            DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1,
        );

        let checkpoint_identities =
            |snapshot: &KfdStoppedQueueSnapshotV1| match snapshot.opaque_checkpoint() {
                KfdOpaqueCheckpointObservationV1::Complete(checkpoint) => {
                    (checkpoint.logical_identity(), checkpoint.content_identity())
                }
                other => panic!("unexpected checkpoint observation: {other:?}"),
            };
        let (first_checkpoint, first_content) = checkpoint_identities(&first);
        let (queue_checkpoint, queue_content) = checkpoint_identities(&queue_changed);
        let (device_checkpoint, device_content) = checkpoint_identities(&device_changed);
        assert_eq!(first_content, queue_content,);
        assert_eq!(first_content, device_content,);
        assert_ne!(first_checkpoint, queue_checkpoint);
        assert_ne!(first_checkpoint, device_checkpoint);
    }

    #[test]
    fn checkpoint_read_failure_and_header_reread_substitution_fail_closed() {
        let mut failed = reader();
        set_checkpoint_ranges(&mut failed);
        failed.checkpoint_failure =
            Some(KfdStoppedUnavailableReasonV1::TargetCheckpointReadPartial);
        assert!(matches!(
            snapshot(&mut failed, DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1).opaque_checkpoint(),
            KfdOpaqueCheckpointObservationV1::Unavailable(
                KfdStoppedUnavailableReasonV1::TargetCheckpointReadPartial
            )
        ));

        let mut substituted = reader();
        set_checkpoint_ranges(&mut substituted);
        substituted.mutate_header_confirmation = true;
        assert!(matches!(
            snapshot(&mut substituted, DEFAULT_KFD_OPAQUE_CHECKPOINT_BYTES_V1).opaque_checkpoint(),
            KfdOpaqueCheckpointObservationV1::Unavailable(
                KfdStoppedUnavailableReasonV1::ContextHeaderBindingSubstituted
            )
        ));
    }

    #[test]
    fn hostile_header_fields_fail_closed_to_typed_unavailability() {
        let mut reserved = reader();
        reserved.headers[3][36..40].copy_from_slice(&1_u32.to_le_bytes());
        assert_eq!(
            unavailable(capture_context_layout(
                &mut reserved,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::ContextHeaderReservedNonzero
        );

        let mut pair = reader();
        pair.headers[0][4..8].copy_from_slice(&64_u32.to_le_bytes());
        assert_eq!(
            unavailable(capture_context_layout(
                &mut pair,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::ContextHeaderRangePairMalformed
        );

        let mut empty_cursor_bounds = reader();
        empty_cursor_bounds.headers[0][0..4]
            .copy_from_slice(&(GFX942_CONTEXT_BYTES_PER_XCC_V1 + 1).to_le_bytes());
        assert_eq!(
            unavailable(capture_context_layout(
                &mut empty_cursor_bounds,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds
        );

        let mut bounds = reader();
        bounds.headers[0][0..4]
            .copy_from_slice(&(GFX942_CONTEXT_BYTES_PER_XCC_V1 - 4).to_le_bytes());
        bounds.headers[0][4..8].copy_from_slice(&8_u32.to_le_bytes());
        assert_eq!(
            unavailable(capture_context_layout(
                &mut bounds,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::ContextHeaderRangeOutOfBounds
        );

        let mut overlap = reader();
        overlap.headers[0][0..4].copy_from_slice(&64_u32.to_le_bytes());
        overlap.headers[0][4..8].copy_from_slice(&64_u32.to_le_bytes());
        overlap.headers[0][8..12].copy_from_slice(&96_u32.to_le_bytes());
        overlap.headers[0][12..16].copy_from_slice(&64_u32.to_le_bytes());
        assert_eq!(
            unavailable(capture_context_layout(
                &mut overlap,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::ContextHeaderRangeOverlap
        );
    }

    #[test]
    fn hostile_geometry_reads_and_header_substitution_are_typed() {
        let mut wrong_device = device();
        wrong_device.gfx_target_version = 90_400;
        assert_eq!(
            unavailable(capture_context_layout(
                &mut reader(),
                scope(),
                queue(),
                wrong_device
            )),
            KfdStoppedUnavailableReasonV1::GfxTargetNotGfx942
        );

        let mut wrong_queue = queue();
        wrong_queue.context_bytes_per_xcc -= 4096;
        assert_eq!(
            unavailable(capture_context_layout(
                &mut reader(),
                scope(),
                wrong_queue,
                device()
            )),
            KfdStoppedUnavailableReasonV1::Gfx942SaveAreaSizeMismatch
        );

        let mut failed = reader();
        failed.fail_at = Some((4, KfdStoppedUnavailableReasonV1::TargetHeaderReadPartial));
        assert_eq!(
            unavailable(capture_context_layout(
                &mut failed,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::TargetHeaderReadPartial
        );

        let mut substituted = reader();
        substituted.headers[7][32..36].copy_from_slice(&(EVENT + 1).to_le_bytes());
        assert_eq!(
            unavailable(capture_context_layout(
                &mut substituted,
                scope(),
                queue(),
                device()
            )),
            KfdStoppedUnavailableReasonV1::ContextHeaderBindingSubstituted
        );
    }

    #[test]
    fn native_binding_substitution_includes_hidden_addresses() {
        let before = queue();
        let mut after = before;
        after.context_address += 4096;
        assert_ne!(before, after);
        let debug = format!("{after:?}");
        assert!(!debug.contains("7f1234501000"));
    }
}

//! Data-only KFD 1.18 event and queue-exception wire schema.
//!
//! This module does not open an fd, issue an ioctl, map an event page, dereference
//! an address, wait, signal an event, or own an event or queue. Constructors only
//! admit values for a later, separately reviewed native adapter.

use core::mem::{align_of, offset_of, size_of};

use super::{IoctlDirection, IoctlRequest, encode_admitted_ioctl};

/// Stable name of this additive event/queue-exception schema.
pub const KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_ID: &str =
    "linux-kfd-event-and-queue-exception-1.18-gfx942-v1";

/// SHA-256 over [`KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_MANIFEST`].
pub const KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_SHA256: &str =
    "8d754af12ed2fcd0c238e1f9e38fbbdab053f44fc5d613b227fdcdd616fcc849";

/// Typed digest bytes of [`KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_MANIFEST`].
pub const KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_SHA256_BYTES: [u8; 32] = [
    0x8d, 0x75, 0x4a, 0xf1, 0x2e, 0xd2, 0xfc, 0xd0, 0xc2, 0x38, 0xe1, 0xf9, 0xe3, 0x8f, 0xbb, 0xda,
    0xb0, 0x53, 0xf4, 0x4f, 0xc5, 0xd6, 0x13, 0xb2, 0x27, 0xfd, 0xcd, 0xd6, 0x16, 0xfc, 0xc8, 0x49,
];

/// Stable name of the additive process-runtime exception-routing schema.
pub const KFD_RUNTIME_ENABLE_SCHEMA_ID: &str = "linux-kfd-runtime-enable-1.18-queue-exception-v1";

/// SHA-256 over [`KFD_RUNTIME_ENABLE_SCHEMA_MANIFEST`].
pub const KFD_RUNTIME_ENABLE_SCHEMA_SHA256: &str =
    "4c762d1e35a5940f0972290151de51e6e19722f81874a6446c66ddc70a062ac1";

pub const KFD_RUNTIME_ENABLE_SCHEMA_SHA256_BYTES: [u8; 32] = [
    0x4c, 0x76, 0x2d, 0x1e, 0x35, 0xa5, 0x94, 0x0f, 0x09, 0x72, 0x29, 0x01, 0x51, 0xde, 0x51, 0xe6,
    0xe1, 0x97, 0x22, 0xf8, 0x18, 0x74, 0xa6, 0x44, 0x6c, 0x66, 0xdd, 0xc7, 0x0a, 0x06, 0x2a, 0xc1,
];

/// The frozen schemas this additive schema composes without changing their identities.
pub const KFD_EVENT_PARENT_SCHEMA_BINDINGS: [(&str, &str); 4] = [
    (
        super::KFD_UAPI_SCHEMA_ID,
        "e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4",
    ),
    (
        super::KFD_MEMORY_LIFECYCLE_SCHEMA_ID,
        "e2d6987b7c8e61a405b2f775d5d004f458a096241459e4cfdf90bd4497f4d58a",
    ),
    (
        super::KFD_AQL_QUEUE_LIFECYCLE_SCHEMA_ID,
        "b11f3c8c766dd25394350646e35269e10c8a33acb98f74cba2a82e95fa185c4e",
    ),
    (
        super::KFD_GFX942_QUEUE_RESOURCE_SCHEMA_ID,
        "63753a9c0dcef0f69e0235b95b44fe6ce22cb5b0d1df6f60a971a5ed28f15904",
    ),
];

pub const KFD_EVENT_SOURCE_SHA256: &str =
    "295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a";
pub const KFD_EVENT_HEADER_SHA256: &str =
    "de275617babe153c015f22de23d4f3ed013759c0a63da96e061454114f0dd119";
pub const KFD_EVENT_PROCESS_SOURCE_SHA256: &str =
    "d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409";
pub const KFD_EVENT_DEBUG_SOURCE_SHA256: &str =
    "f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d";
pub const KFD_EVENT_ROCR_EVENTS_SOURCE_SHA256: &str =
    "a76b99eeee2aee1c282659a1e43217817b83260ef52f532c6db8a9dfd1d993d9";
pub const KFD_EVENT_ROCR_QUEUES_SOURCE_SHA256: &str =
    "b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b";
pub const KFD_EVENT_ROCR_AQL_QUEUE_SOURCE_SHA256: &str =
    "291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14";
pub const KFD_EVENT_ROCR_TYPES_HEADER_SHA256: &str =
    "fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e";

pub const KFD_RUNTIME_ENABLE_MODE_ENABLE_MASK: u32 = 1;

pub const KFD_IOC_EVENT_SIGNAL: u32 = 0;
pub const KFD_IOC_EVENT_NODECHANGE: u32 = 1;
pub const KFD_IOC_EVENT_DEVICESTATECHANGE: u32 = 2;
pub const KFD_IOC_EVENT_HW_EXCEPTION: u32 = 3;
pub const KFD_IOC_EVENT_SYSTEM_EVENT: u32 = 4;
pub const KFD_IOC_EVENT_DEBUG_EVENT: u32 = 5;
pub const KFD_IOC_EVENT_PROFILE_EVENT: u32 = 6;
pub const KFD_IOC_EVENT_QUEUE_EVENT: u32 = 7;
pub const KFD_IOC_EVENT_MEMORY: u32 = 8;

pub const KFD_IOC_WAIT_RESULT_COMPLETE: u32 = 0;
pub const KFD_IOC_WAIT_RESULT_TIMEOUT: u32 = 1;
pub const KFD_IOC_WAIT_RESULT_FAIL: u32 = 2;
pub const KFD_EVENT_TIMEOUT_IMMEDIATE: u32 = 0;
pub const KFD_EVENT_TIMEOUT_INFINITE: u32 = u32::MAX;
pub const KFD_SIGNAL_EVENT_LIMIT: u32 = 4096;
pub const KFD_EVENT_PAGE_MMAP_OFFSET: u64 = 2_u64 << 62;
pub const KFD_EVENT_PAGE_SLOT_COUNT: usize = KFD_SIGNAL_EVENT_LIMIT as usize;
pub const KFD_EVENT_PAGE_BYTES: usize = KFD_EVENT_PAGE_SLOT_COUNT * size_of::<u64>();
pub const KFD_EVENT_SLOT_UNSIGNALED: u64 = u64::MAX;
/// Exact compatibility slot count when KFD allocates the first signal page.
pub const KFD_INTERNAL_SIGNAL_PAGE_SLOT_COUNT: u32 = 256;

/// Queue exception codes admitted from `enum kfd_dbg_trap_exception_code`.
pub const KFD_QUEUE_EXCEPTION_CODES: [u32; 16] =
    [1, 2, 3, 4, 5, 6, 16, 17, 18, 19, 20, 21, 22, 23, 30, 31];
pub const KFD_QUEUE_EXCEPTION_MASK: u64 = 0x0000_0000_607f_803f;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdQueueExceptionCodeV1 {
    WaveAbort = 1,
    WaveTrap = 2,
    WaveMathError = 3,
    WaveIllegalInstruction = 4,
    WaveMemoryViolation = 5,
    WaveApertureViolation = 6,
    DispatchDimensionsInvalid = 16,
    DispatchGroupSegmentSizeInvalid = 17,
    DispatchCodeInvalid = 18,
    PacketReserved = 19,
    PacketUnsupported = 20,
    DispatchWorkgroupSizeInvalid = 21,
    DispatchRegisterInvalid = 22,
    PacketVendorUnsupported = 23,
    PreemptionError = 30,
    QueueNew = 31,
}

impl KfdQueueExceptionCodeV1 {
    pub const fn from_wire(value: u32) -> Option<Self> {
        match value {
            1 => Some(Self::WaveAbort),
            2 => Some(Self::WaveTrap),
            3 => Some(Self::WaveMathError),
            4 => Some(Self::WaveIllegalInstruction),
            5 => Some(Self::WaveMemoryViolation),
            6 => Some(Self::WaveApertureViolation),
            16 => Some(Self::DispatchDimensionsInvalid),
            17 => Some(Self::DispatchGroupSegmentSizeInvalid),
            18 => Some(Self::DispatchCodeInvalid),
            19 => Some(Self::PacketReserved),
            20 => Some(Self::PacketUnsupported),
            21 => Some(Self::DispatchWorkgroupSizeInvalid),
            22 => Some(Self::DispatchRegisterInvalid),
            23 => Some(Self::PacketVendorUnsupported),
            30 => Some(Self::PreemptionError),
            31 => Some(Self::QueueNew),
            _ => None,
        }
    }

    pub const fn mask(self) -> u64 {
        1_u64 << (self as u32 - 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdEventTypeV1 {
    Signal = KFD_IOC_EVENT_SIGNAL,
    NodeChange = KFD_IOC_EVENT_NODECHANGE,
    DeviceStateChange = KFD_IOC_EVENT_DEVICESTATECHANGE,
    HardwareException = KFD_IOC_EVENT_HW_EXCEPTION,
    System = KFD_IOC_EVENT_SYSTEM_EVENT,
    Debug = KFD_IOC_EVENT_DEBUG_EVENT,
    Profile = KFD_IOC_EVENT_PROFILE_EVENT,
    Queue = KFD_IOC_EVENT_QUEUE_EVENT,
    Memory = KFD_IOC_EVENT_MEMORY,
}

impl KfdEventTypeV1 {
    pub const fn from_wire(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Signal),
            1 => Some(Self::NodeChange),
            2 => Some(Self::DeviceStateChange),
            3 => Some(Self::HardwareException),
            4 => Some(Self::System),
            5 => Some(Self::Debug),
            6 => Some(Self::Profile),
            7 => Some(Self::Queue),
            8 => Some(Self::Memory),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdWaitResultV1 {
    Complete,
    Timeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct KfdSignalEventIdV1(u32);

impl KfdSignalEventIdV1 {
    pub const fn new(value: u32) -> Option<Self> {
        if value > 0 && value < KFD_SIGNAL_EVENT_LIMIT {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Opaque KFD allocation-handle observation accepted only on first signal creation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct KfdEventPageHandleObservationV1(u64);

impl KfdEventPageHandleObservationV1 {
    pub const fn new(value: u64) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Opaque userspace virtual-address observation; it is not a Rust pointer or mapping authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct KfdEventDataArrayAddressV1(u64);

impl KfdEventDataArrayAddressV1 {
    pub const fn new(value: u64, event_count: u32) -> Option<Self> {
        if event_count == 0
            || value == 0
            || !value.is_multiple_of(align_of::<KfdEventDataV1>() as u64)
        {
            return None;
        }
        let Some(bytes) = (event_count as u64).checked_mul(size_of::<KfdEventDataV1>() as u64)
        else {
            return None;
        };
        if value.checked_add(bytes).is_some() {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Opaque userspace virtual-address observation for the queue error-reason word.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct KfdQueueExceptionPayloadAddressV1(u64);

impl KfdQueueExceptionPayloadAddressV1 {
    pub const fn new(value: u64) -> Option<Self> {
        if value != 0
            && value.is_multiple_of(align_of::<u64>() as u64)
            && value.checked_add(8).is_some()
        {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdCreateSignalAdmissionErrorV1 {
    EventPageOffset,
    TriggerData,
    EventType,
    AutoReset,
    NodeId,
    EventId,
    SlotIndex,
    InternalSignalPageId,
}

/// Exact `struct kfd_ioctl_runtime_enable_args` wire layout.
///
/// Both constructors deliberately exclude a debugger address, TTMP setup, and
/// capability claims. They describe only the process-global transition needed
/// to route queue exceptions through a context-save header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlRuntimeEnableArgsV1 {
    r_debug: u64,
    mode_mask: u32,
    capabilities_mask: u32,
}

impl KfdIoctlRuntimeEnableArgsV1 {
    pub const fn new_queue_exception_enable() -> Self {
        Self {
            r_debug: 0,
            mode_mask: KFD_RUNTIME_ENABLE_MODE_ENABLE_MASK,
            capabilities_mask: 0,
        }
    }

    pub const fn new_queue_exception_disable() -> Self {
        Self {
            r_debug: 0,
            mode_mask: 0,
            capabilities_mask: 0,
        }
    }

    pub const fn r_debug(self) -> u64 {
        self.r_debug
    }

    pub const fn mode_mask(self) -> u32 {
        self.mode_mask
    }

    pub const fn capabilities_mask(self) -> u32 {
        self.capabilities_mask
    }

    pub const fn is_exact_queue_exception_enable(self) -> bool {
        self.r_debug == 0
            && self.mode_mask == KFD_RUNTIME_ENABLE_MODE_ENABLE_MASK
            && self.capabilities_mask == 0
    }

    pub const fn is_exact_queue_exception_disable(self) -> bool {
        self.r_debug == 0 && self.mode_mask == 0 && self.capabilities_mask == 0
    }

    /// Reconstructs untrusted bytes for hostile admission tests.
    pub const fn from_untrusted_wire(r_debug: u64, mode_mask: u32, capabilities_mask: u32) -> Self {
        Self {
            r_debug,
            mode_mask,
            capabilities_mask,
        }
    }
}

/// Exact `struct kfd_ioctl_create_event_args` wire layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlCreateEventArgsV1 {
    event_page_offset: u64,
    event_trigger_data: u32,
    event_type: u32,
    auto_reset: u32,
    node_id: u32,
    event_id: u32,
    event_slot_index: u32,
}

impl KfdIoctlCreateEventArgsV1 {
    /// A queue-exception signal event is signal type, auto-reset, and node-independent.
    pub const fn new_queue_exception_signal(
        first_event_page_handle: Option<KfdEventPageHandleObservationV1>,
    ) -> Self {
        Self {
            event_page_offset: match first_event_page_handle {
                Some(handle) => handle.get(),
                None => 0,
            },
            event_trigger_data: 0,
            event_type: KFD_IOC_EVENT_SIGNAL,
            auto_reset: 1,
            node_id: 0,
            event_id: 0,
            event_slot_index: 0,
        }
    }

    /// Reconstructs untrusted kernel-facing bytes for admission tests or a syscall adapter.
    #[allow(clippy::too_many_arguments)]
    pub const fn from_untrusted_wire(
        event_page_offset: u64,
        event_trigger_data: u32,
        event_type: u32,
        auto_reset: u32,
        node_id: u32,
        event_id: u32,
        event_slot_index: u32,
    ) -> Self {
        Self {
            event_page_offset,
            event_trigger_data,
            event_type,
            auto_reset,
            node_id,
            event_id,
            event_slot_index,
        }
    }

    /// Validates every input and output field after a successful CREATE_EVENT ioctl.
    pub const fn admit_queue_exception_signal_output(
        self,
    ) -> Result<KfdCreatedSignalEventObservationV1, KfdCreateSignalAdmissionErrorV1> {
        if self.event_page_offset != KFD_EVENT_PAGE_MMAP_OFFSET {
            return Err(KfdCreateSignalAdmissionErrorV1::EventPageOffset);
        }
        if self.event_type != KFD_IOC_EVENT_SIGNAL {
            return Err(KfdCreateSignalAdmissionErrorV1::EventType);
        }
        if self.auto_reset != 1 {
            return Err(KfdCreateSignalAdmissionErrorV1::AutoReset);
        }
        if self.node_id != 0 {
            return Err(KfdCreateSignalAdmissionErrorV1::NodeId);
        }
        let Some(event_id) = KfdSignalEventIdV1::new(self.event_id) else {
            return Err(KfdCreateSignalAdmissionErrorV1::EventId);
        };
        if self.event_trigger_data != self.event_id {
            return Err(KfdCreateSignalAdmissionErrorV1::TriggerData);
        }
        if self.event_slot_index != self.event_id {
            return Err(KfdCreateSignalAdmissionErrorV1::SlotIndex);
        }
        Ok(KfdCreatedSignalEventObservationV1 {
            id: event_id,
            event_page_mmap_offset: self.event_page_offset,
            trigger_data: self.event_trigger_data,
            slot_index: self.event_slot_index,
        })
    }

    /// Narrows admission to the active driver's first internally allocated
    /// signal page, whose compatibility size is exactly 256 slots with zero
    /// reserved. This does not prove that no foreign KFD client used the
    /// process first; the live adapter separately owns that process policy.
    pub const fn admit_first_internal_queue_exception_signal_output(
        self,
    ) -> Result<KfdCreatedSignalEventObservationV1, KfdCreateSignalAdmissionErrorV1> {
        let observation = match self.admit_queue_exception_signal_output() {
            Ok(observation) => observation,
            Err(error) => return Err(error),
        };
        if observation.id.get() >= KFD_INTERNAL_SIGNAL_PAGE_SLOT_COUNT {
            return Err(KfdCreateSignalAdmissionErrorV1::InternalSignalPageId);
        }
        Ok(observation)
    }

    pub const fn event_page_offset(self) -> u64 {
        self.event_page_offset
    }
    pub const fn event_trigger_data(self) -> u32 {
        self.event_trigger_data
    }
    pub const fn event_type(self) -> u32 {
        self.event_type
    }
    pub const fn auto_reset(self) -> u32 {
        self.auto_reset
    }
    pub const fn node_id(self) -> u32 {
        self.node_id
    }
    pub const fn event_id(self) -> u32 {
        self.event_id
    }
    pub const fn event_slot_index(self) -> u32 {
        self.event_slot_index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Validated CREATE_EVENT output data, not event ownership or operation authority.
pub struct KfdCreatedSignalEventObservationV1 {
    id: KfdSignalEventIdV1,
    event_page_mmap_offset: u64,
    trigger_data: u32,
    slot_index: u32,
}

impl KfdCreatedSignalEventObservationV1 {
    pub const fn id(self) -> KfdSignalEventIdV1 {
        self.id
    }
    pub const fn event_page_mmap_offset(self) -> u64 {
        self.event_page_mmap_offset
    }
    pub const fn trigger_data(self) -> u32 {
        self.trigger_data
    }
    pub const fn slot_index(self) -> u32 {
        self.slot_index
    }
}

macro_rules! event_id_request {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(C)]
        pub struct $name {
            event_id: u32,
            pad: u32,
        }

        impl $name {
            pub const fn new(event_id: KfdSignalEventIdV1) -> Self {
                Self {
                    event_id: event_id.get(),
                    pad: 0,
                }
            }

            pub const fn event_id(self) -> u32 {
                self.event_id
            }

            pub const fn pad(self) -> u32 {
                self.pad
            }
        }
    };
}

event_id_request!(KfdIoctlDestroyEventArgsV1);
event_id_request!(KfdIoctlSetEventArgsV1);
event_id_request!(KfdIoctlResetEventArgsV1);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdMemoryExceptionFailureV1 {
    not_present: u32,
    read_only: u32,
    no_execute: u32,
    imprecise: u32,
}

impl KfdMemoryExceptionFailureV1 {
    pub const fn from_untrusted_wire(
        not_present: u32,
        read_only: u32,
        no_execute: u32,
        imprecise: u32,
    ) -> Self {
        Self {
            not_present,
            read_only,
            no_execute,
            imprecise,
        }
    }

    pub const fn words(self) -> [u32; 4] {
        [
            self.not_present,
            self.read_only,
            self.no_execute,
            self.imprecise,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdHsaMemoryExceptionDataV1 {
    failure: KfdMemoryExceptionFailureV1,
    va: u64,
    gpu_id: u32,
    error_type: u32,
}

impl KfdHsaMemoryExceptionDataV1 {
    pub const fn from_untrusted_wire(
        failure: KfdMemoryExceptionFailureV1,
        va: u64,
        gpu_id: u32,
        error_type: u32,
    ) -> Self {
        Self {
            failure,
            va,
            gpu_id,
            error_type,
        }
    }

    pub const fn failure(self) -> KfdMemoryExceptionFailureV1 {
        self.failure
    }
    pub const fn va(self) -> u64 {
        self.va
    }
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn error_type(self) -> u32 {
        self.error_type
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdHsaHardwareExceptionDataV1 {
    reset_type: u32,
    reset_cause: u32,
    memory_lost: u32,
    gpu_id: u32,
}

impl KfdHsaHardwareExceptionDataV1 {
    pub const fn from_untrusted_wire(
        reset_type: u32,
        reset_cause: u32,
        memory_lost: u32,
        gpu_id: u32,
    ) -> Self {
        Self {
            reset_type,
            reset_cause,
            memory_lost,
            gpu_id,
        }
    }

    pub const fn words(self) -> [u32; 4] {
        [
            self.reset_type,
            self.reset_cause,
            self.memory_lost,
            self.gpu_id,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdHsaSignalEventDataV1 {
    last_event_age: u64,
}

impl KfdHsaSignalEventDataV1 {
    pub const fn new(last_event_age: u64) -> Self {
        Self { last_event_age }
    }

    pub const fn last_event_age(self) -> u64 {
        self.last_event_age
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C, align(8))]
pub struct KfdEventPayloadV1 {
    words: [u64; 4],
}

impl KfdEventPayloadV1 {
    pub const fn signal(last_event_age: u64) -> Self {
        let signal = KfdHsaSignalEventDataV1::new(last_event_age);
        Self {
            words: [signal.last_event_age(), 0, 0, 0],
        }
    }

    pub const fn from_untrusted_words(words: [u64; 4]) -> Self {
        Self { words }
    }

    pub const fn words(self) -> [u64; 4] {
        self.words
    }
}

/// Exact 48-byte `struct kfd_event_data`; its extension address is fixed null.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdEventDataV1 {
    payload: KfdEventPayloadV1,
    kfd_event_data_ext: u64,
    event_id: u32,
    pad: u32,
}

impl KfdEventDataV1 {
    pub const fn new_signal(event_id: KfdSignalEventIdV1, last_event_age: u64) -> Self {
        Self {
            payload: KfdEventPayloadV1::signal(last_event_age),
            kfd_event_data_ext: 0,
            event_id: event_id.get(),
            pad: 0,
        }
    }

    pub const fn payload(self) -> KfdEventPayloadV1 {
        self.payload
    }
    pub const fn extension_address(self) -> u64 {
        self.kfd_event_data_ext
    }
    pub const fn event_id(self) -> u32 {
        self.event_id
    }
    pub const fn pad(self) -> u32 {
        self.pad
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdWaitAdmissionErrorV1 {
    EventsAddress,
    EventCount,
    WaitMode,
    Timeout,
    KernelFailure,
    UnknownResult,
}

/// Exact `struct kfd_ioctl_wait_events_args` wire layout.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlWaitEventsArgsV1 {
    events_ptr: u64,
    num_events: u32,
    wait_for_all: u32,
    timeout: u32,
    wait_result: u32,
}

impl KfdIoctlWaitEventsArgsV1 {
    /// Builds the only admitted wait shape: one signal event, trivially wait-for-all.
    pub const fn new_one_signal(events: KfdEventDataArrayAddressV1, timeout_ms: u32) -> Self {
        Self {
            events_ptr: events.get(),
            num_events: 1,
            wait_for_all: 1,
            timeout: timeout_ms,
            wait_result: KFD_IOC_WAIT_RESULT_FAIL,
        }
    }

    pub const fn from_untrusted_wire(
        events_ptr: u64,
        num_events: u32,
        wait_for_all: u32,
        timeout: u32,
        wait_result: u32,
    ) -> Self {
        Self {
            events_ptr,
            num_events,
            wait_for_all,
            timeout,
            wait_result,
        }
    }

    /// Admits only a successful ioctl whose input fields remained exactly bound.
    pub const fn admit_successful_result(
        self,
        expected_events: KfdEventDataArrayAddressV1,
        expected_timeout_ms: u32,
    ) -> Result<KfdWaitResultV1, KfdWaitAdmissionErrorV1> {
        if self.events_ptr != expected_events.get() {
            return Err(KfdWaitAdmissionErrorV1::EventsAddress);
        }
        if self.num_events != 1 {
            return Err(KfdWaitAdmissionErrorV1::EventCount);
        }
        if self.wait_for_all != 1 {
            return Err(KfdWaitAdmissionErrorV1::WaitMode);
        }
        if self.timeout != expected_timeout_ms {
            return Err(KfdWaitAdmissionErrorV1::Timeout);
        }
        match self.wait_result {
            KFD_IOC_WAIT_RESULT_COMPLETE => Ok(KfdWaitResultV1::Complete),
            KFD_IOC_WAIT_RESULT_TIMEOUT => Ok(KfdWaitResultV1::Timeout),
            KFD_IOC_WAIT_RESULT_FAIL => Err(KfdWaitAdmissionErrorV1::KernelFailure),
            _ => Err(KfdWaitAdmissionErrorV1::UnknownResult),
        }
    }

    pub const fn events_address(self) -> u64 {
        self.events_ptr
    }
    pub const fn event_count(self) -> u32 {
        self.num_events
    }
    pub const fn wait_for_all(self) -> u32 {
        self.wait_for_all
    }
    pub const fn timeout_ms(self) -> u32 {
        self.timeout
    }
    pub const fn wait_result(self) -> u32 {
        self.wait_result
    }
}

/// Admitted queue-error bitmask copied by KFD into the configured payload word.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct KfdQueueExceptionReasonV1(u64);

impl KfdQueueExceptionReasonV1 {
    pub const fn from_untrusted_wire(value: u64) -> Option<Self> {
        if value & !KFD_QUEUE_EXCEPTION_MASK == 0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> u64 {
        self.0
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub const fn contains_code(self, exception_code: u32) -> bool {
        match KfdQueueExceptionCodeV1::from_wire(exception_code) {
            Some(code) => self.contains(code),
            None => false,
        }
    }

    pub const fn contains(self, exception_code: KfdQueueExceptionCodeV1) -> bool {
        self.0 & exception_code.mask() != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdContextSaveHeaderErrorV1 {
    DebugOffsetAlignment,
    DebugSizeAlignment,
    DebugRangeOverflow,
}

/// Exact KFD context-save-area header used for queue exception delivery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdContextSaveAreaHeaderV1 {
    control_stack_offset: u32,
    control_stack_size: u32,
    wave_state_offset: u32,
    wave_state_size: u32,
    debug_offset: u32,
    debug_size: u32,
    err_payload_addr: u64,
    err_event_id: u32,
    reserved1: u32,
}

impl KfdContextSaveAreaHeaderV1 {
    /// Constructs the initial header. Dynamic wave-save fields and reserved bits are zero.
    pub const fn new_queue_exception(
        debug_offset: u32,
        debug_size: u32,
        payload: KfdQueueExceptionPayloadAddressV1,
        event_id: KfdSignalEventIdV1,
    ) -> Result<Self, KfdContextSaveHeaderErrorV1> {
        if !debug_offset.is_multiple_of(64) {
            return Err(KfdContextSaveHeaderErrorV1::DebugOffsetAlignment);
        }
        if !debug_size.is_multiple_of(64) {
            return Err(KfdContextSaveHeaderErrorV1::DebugSizeAlignment);
        }
        if debug_offset.checked_add(debug_size).is_none() {
            return Err(KfdContextSaveHeaderErrorV1::DebugRangeOverflow);
        }
        Ok(Self {
            control_stack_offset: 0,
            control_stack_size: 0,
            wave_state_offset: 0,
            wave_state_size: 0,
            debug_offset,
            debug_size,
            err_payload_addr: payload.get(),
            err_event_id: event_id.get(),
            reserved1: 0,
        })
    }

    pub const fn wave_state_words(self) -> [u32; 4] {
        [
            self.control_stack_offset,
            self.control_stack_size,
            self.wave_state_offset,
            self.wave_state_size,
        ]
    }
    pub const fn debug_offset(self) -> u32 {
        self.debug_offset
    }
    pub const fn debug_size(self) -> u32 {
        self.debug_size
    }
    pub const fn error_payload_address(self) -> u64 {
        self.err_payload_addr
    }
    pub const fn error_event_id(self) -> u32 {
        self.err_event_id
    }
    pub const fn reserved(self) -> u32 {
        self.reserved1
    }
}

pub const AMDKFD_IOC_CREATE_EVENT: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::ReadWrite,
    super::AMDKFD_IOCTL_BASE,
    0x08,
    size_of::<KfdIoctlCreateEventArgsV1>(),
);
pub const AMDKFD_IOC_DESTROY_EVENT: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::Write,
    super::AMDKFD_IOCTL_BASE,
    0x09,
    size_of::<KfdIoctlDestroyEventArgsV1>(),
);
pub const AMDKFD_IOC_SET_EVENT: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::Write,
    super::AMDKFD_IOCTL_BASE,
    0x0a,
    size_of::<KfdIoctlSetEventArgsV1>(),
);
pub const AMDKFD_IOC_RESET_EVENT: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::Write,
    super::AMDKFD_IOCTL_BASE,
    0x0b,
    size_of::<KfdIoctlResetEventArgsV1>(),
);
pub const AMDKFD_IOC_WAIT_EVENTS: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::ReadWrite,
    super::AMDKFD_IOCTL_BASE,
    0x0c,
    size_of::<KfdIoctlWaitEventsArgsV1>(),
);

/// Exact Linux generic-IOC encoding of `AMDKFD_IOC_RUNTIME_ENABLE`.
pub const AMDKFD_IOC_RUNTIME_ENABLE: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::ReadWrite,
    super::AMDKFD_IOCTL_BASE,
    0x25,
    size_of::<KfdIoctlRuntimeEnableArgsV1>(),
);

const _: () = {
    assert!(size_of::<KfdIoctlRuntimeEnableArgsV1>() == 16);
    assert!(align_of::<KfdIoctlRuntimeEnableArgsV1>() == 8);
    assert!(offset_of!(KfdIoctlRuntimeEnableArgsV1, r_debug) == 0);
    assert!(offset_of!(KfdIoctlRuntimeEnableArgsV1, mode_mask) == 8);
    assert!(offset_of!(KfdIoctlRuntimeEnableArgsV1, capabilities_mask) == 12);
    assert!(size_of::<KfdIoctlCreateEventArgsV1>() == 32);
    assert!(align_of::<KfdIoctlCreateEventArgsV1>() == 8);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, event_page_offset) == 0);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, event_trigger_data) == 8);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, event_type) == 12);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, auto_reset) == 16);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, node_id) == 20);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, event_id) == 24);
    assert!(offset_of!(KfdIoctlCreateEventArgsV1, event_slot_index) == 28);
    assert!(size_of::<KfdIoctlDestroyEventArgsV1>() == 8);
    assert!(align_of::<KfdIoctlDestroyEventArgsV1>() == 4);
    assert!(size_of::<KfdIoctlSetEventArgsV1>() == 8);
    assert!(size_of::<KfdIoctlResetEventArgsV1>() == 8);
    assert!(size_of::<KfdMemoryExceptionFailureV1>() == 16);
    assert!(align_of::<KfdMemoryExceptionFailureV1>() == 4);
    assert!(size_of::<KfdHsaMemoryExceptionDataV1>() == 32);
    assert!(align_of::<KfdHsaMemoryExceptionDataV1>() == 8);
    assert!(offset_of!(KfdHsaMemoryExceptionDataV1, va) == 16);
    assert!(offset_of!(KfdHsaMemoryExceptionDataV1, gpu_id) == 24);
    assert!(offset_of!(KfdHsaMemoryExceptionDataV1, error_type) == 28);
    assert!(size_of::<KfdHsaHardwareExceptionDataV1>() == 16);
    assert!(align_of::<KfdHsaHardwareExceptionDataV1>() == 4);
    assert!(size_of::<KfdHsaSignalEventDataV1>() == 8);
    assert!(align_of::<KfdHsaSignalEventDataV1>() == 8);
    assert!(size_of::<KfdEventPayloadV1>() == 32);
    assert!(align_of::<KfdEventPayloadV1>() == 8);
    assert!(size_of::<KfdEventDataV1>() == 48);
    assert!(align_of::<KfdEventDataV1>() == 8);
    assert!(offset_of!(KfdEventDataV1, payload) == 0);
    assert!(offset_of!(KfdEventDataV1, kfd_event_data_ext) == 32);
    assert!(offset_of!(KfdEventDataV1, event_id) == 40);
    assert!(offset_of!(KfdEventDataV1, pad) == 44);
    assert!(size_of::<KfdIoctlWaitEventsArgsV1>() == 24);
    assert!(align_of::<KfdIoctlWaitEventsArgsV1>() == 8);
    assert!(offset_of!(KfdIoctlWaitEventsArgsV1, events_ptr) == 0);
    assert!(offset_of!(KfdIoctlWaitEventsArgsV1, num_events) == 8);
    assert!(offset_of!(KfdIoctlWaitEventsArgsV1, wait_for_all) == 12);
    assert!(offset_of!(KfdIoctlWaitEventsArgsV1, timeout) == 16);
    assert!(offset_of!(KfdIoctlWaitEventsArgsV1, wait_result) == 20);
    assert!(size_of::<KfdContextSaveAreaHeaderV1>() == 40);
    assert!(align_of::<KfdContextSaveAreaHeaderV1>() == 8);
    assert!(offset_of!(KfdContextSaveAreaHeaderV1, debug_offset) == 16);
    assert!(offset_of!(KfdContextSaveAreaHeaderV1, debug_size) == 20);
    assert!(offset_of!(KfdContextSaveAreaHeaderV1, err_payload_addr) == 24);
    assert!(offset_of!(KfdContextSaveAreaHeaderV1, err_event_id) == 32);
    assert!(offset_of!(KfdContextSaveAreaHeaderV1, reserved1) == 36);
};

/// Canonical, newline-terminated identity input for this additive schema.
pub const KFD_EVENT_QUEUE_EXCEPTION_SCHEMA_MANIFEST: &str = concat!(
    "schema_id=linux-kfd-event-and-queue-exception-1.18-gfx942-v1\n",
    "target=linux-x86_64-generic-ioc;gfx942\n",
    "source_package=amdgpu-dkms@1:6.16.13.30300400-2341068.24.04\n",
    "parent.discovery.schema_id=linux-kfd-uapi-1.18-generic-ioc-v1\n",
    "parent.discovery.sha256=e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4\n",
    "parent.memory.schema_id=linux-kfd-memory-lifecycle-1.18-generic-ioc-v1\n",
    "parent.memory.sha256=e2d6987b7c8e61a405b2f775d5d004f458a096241459e4cfdf90bd4497f4d58a\n",
    "parent.queue.schema_id=linux-kfd-aql-queue-lifecycle-1.18-generic-ioc-v1\n",
    "parent.queue.sha256=b11f3c8c766dd25394350646e35269e10c8a33acb98f74cba2a82e95fa185c4e\n",
    "parent.gfx942_resources.schema_id=linux-kfd-gfx942-queue-resources-1.18-v1\n",
    "parent.gfx942_resources.sha256=63753a9c0dcef0f69e0235b95b44fe6ce22cb5b0d1df6f60a971a5ed28f15904\n",
    "linux.uapi.path=include/uapi/linux/kfd_ioctl.h\n",
    "linux.uapi.sha256=b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d\n",
    "linux.events.path=amd/amdkfd/kfd_events.c\n",
    "linux.events.sha256=295114e5bacb3be94cdc17b6760e893198ee51d1c77d5837cfab999c3823485a\n",
    "linux.events_h.path=amd/amdkfd/kfd_events.h\n",
    "linux.events_h.sha256=de275617babe153c015f22de23d4f3ed013759c0a63da96e061454114f0dd119\n",
    "linux.process.path=amd/amdkfd/kfd_process.c\n",
    "linux.process.sha256=d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409\n",
    "linux.debug.path=amd/amdkfd/kfd_debug.c\n",
    "linux.debug.sha256=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "linux.chardev.path=amd/amdkfd/kfd_chardev.c\n",
    "linux.chardev.sha256=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "linux.priv_h.path=amd/amdkfd/kfd_priv.h\n",
    "linux.priv_h.sha256=f991330031c14725b2be0636ec1896ab530dc3d07d530ebd4f47efff97a82a99\n",
    "rocr.commit=97f5574fe2fdc7bef44fb01545347912ee9f1779\n",
    "rocr.events.path=projects/rocr-runtime/libhsakmt/src/events.c\n",
    "rocr.events.sha256=a76b99eeee2aee1c282659a1e43217817b83260ef52f532c6db8a9dfd1d993d9\n",
    "rocr.queues.path=projects/rocr-runtime/libhsakmt/src/queues.c\n",
    "rocr.queues.sha256=b7ead541340ac996c2305b2e9660cb3176edcd61ee509d4880f02659fbb6f32b\n",
    "rocr.aql_queue.path=projects/rocr-runtime/runtime/hsa-runtime/core/runtime/amd_aql_queue.cpp\n",
    "rocr.aql_queue.sha256=291f2521e2a4758e852ed20c578aca79e379d1effe4dfd83c62e11347eef2b14\n",
    "rocr.types_h.path=projects/rocr-runtime/libhsakmt/include/hsakmt/hsakmttypes.h\n",
    "rocr.types_h.sha256=fd9e3e9a0874614e70e518ee420aacd2d171452c2755d05b2cf54b55144ec78e\n",
    "create_event=size:32,align:8,offsets:0,8,12,16,20,24,28,request:c0204b08\n",
    "destroy_event=size:8,align:4,offsets:0,4,request:40084b09\n",
    "set_event=size:8,align:4,offsets:0,4,request:40084b0a\n",
    "reset_event=size:8,align:4,offsets:0,4,request:40084b0b\n",
    "event_data=size:48,align:8,payload:0,extension:32,event_id:40,pad:44\n",
    "wait_events=size:24,align:8,offsets:0,8,12,16,20,request:c0184b0c\n",
    "csa_header=size:40,align:8,wave:0,debug_offset:16,debug_size:20,payload:24,event:32,reserved:36\n",
    "queue_exception_codes=1,2,3,4,5,6,16,17,18,19,20,21,22,23,30,31;mask:00000000607f803f\n",
    "create_signal=auto-reset;node-zero;optional-opaque-first-page-handle;output-mmap-token;id=trigger=slot:1..4095\n",
    "surface=create,destroy,set,reset,wait,signal-data,csa-header,queue-reason\n",
    "authority=wire-only;opaque-addresses;no-fd;no-ioctl;no-mmap;no-deref;no-event;no-queue\n",
);

/// Canonical identity input for the process-global exception-routing schema.
///
/// This composes the frozen event schema without modifying its identity. The
/// source set is the exact reviewed path for the transition, the queue-trap
/// routing predicate, and the context-save header/event write. It is not a
/// transitive kernel-build closure or native operation authority.
pub const KFD_RUNTIME_ENABLE_SCHEMA_MANIFEST: &str = concat!(
    "schema_id=linux-kfd-runtime-enable-1.18-queue-exception-v1\n",
    "target=linux-x86_64-generic-ioc;gfx942\n",
    "source_package=amdgpu-dkms@1:6.16.13.30300400-2341068.24.04\n",
    "parent.event.schema_id=linux-kfd-event-and-queue-exception-1.18-gfx942-v1\n",
    "parent.event.sha256=8d754af12ed2fcd0c238e1f9e38fbbdab053f44fc5d613b227fdcdd616fcc849\n",
    "linux.uapi.path=include/uapi/linux/kfd_ioctl.h\n",
    "linux.uapi.sha256=b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d\n",
    "linux.chardev.path=amd/amdkfd/kfd_chardev.c\n",
    "linux.chardev.sha256=f9a8805c5d479faee25e457051aa428e4bb523ecf1c7b1618a6a5f79ca5d7bba\n",
    "linux.debug.path=amd/amdkfd/kfd_debug.c\n",
    "linux.debug.sha256=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "linux.process.path=amd/amdkfd/kfd_process.c\n",
    "linux.process.sha256=d76db8cbb546aa23dffb33b1d04244037e12246b49b752303194c68dd685e409\n",
    "runtime_enable=size:16,align:8,offsets:0,8,12,request:c0104b25\n",
    "enable=r_debug:0,mode:1,capabilities:0;ttmp-save-excluded\n",
    "disable=r_debug:0,mode:0,capabilities:0\n",
    "first-internal-signal-page=256-slots;event-id:1..255\n",
    "ordering=process-global-enable-before-any-user-queue;queue-destroy-before-event-destroy-before-runtime-disable\n",
    "failure=all-ioctl-errors-ambiguous;interrupt-retry-excluded;process-fail-stop\n",
    "scope=queue-exception-routing-preparation;actual-fault-and-delivery-evidence-excluded\n",
    "authority=wire-only;no-fd;no-ioctl;no-process-or-queue-ownership\n",
);

//! Reviewed KFD 1.18 debug-trap ABI.
//!
//! The C ABI uses a union in `struct kfd_ioctl_dbg_trap_args`. This no-unsafe
//! crate represents the union as three aligned `u64` words and provides only
//! operation-specific constructors and accessors. The admitted target is
//! little-endian Linux x86_64; another target requires a separately reviewed
//! encoding.

use core::mem::{align_of, offset_of, size_of};

use super::{
    AMDKFD_IOC_RUNTIME_ENABLE, AMDKFD_IOCTL_BASE, IoctlDirection, IoctlRequest,
    KFD_UAPI_SCHEMA_MANIFEST_SHA256, KfdIoctlRuntimeEnableArgsV1, encode_admitted_ioctl,
};

pub const KFD_DEBUG_TRAP_SCHEMA_ID_V1: &str = "linux-kfd-debug-trap-1.18-x86_64-le-v1";

pub const KFD_DEBUG_TRAP_DRIVER_SOURCE_SHA256_V1: &str =
    "f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d";

pub const KFD_DEBUG_TRAP_SCHEMA_MANIFEST_V1: &str = concat!(
    "schema_id=linux-kfd-debug-trap-1.18-x86_64-le-v1\n",
    "base_schema_sha256=e4aad5d8e3177ea6d70298adab7741c377cb091373553ce689f3525e7514d9b4\n",
    "source_header_sha256=b3721c1a428a32bb9994af579432af48c44fa65abb860049f11a63a5c093235d\n",
    "debug_source=amd/amdkfd/kfd_debug.c\n",
    "debug_source_sha256=f6c688b75fd25ead43ce3c3961bd0af210f873bad1b29dce8e84bb7fb968fe4d\n",
    "target=linux-x86_64-little-endian-generic-ioc\n",
    "kfd_uapi=1.18\n",
    "request=c0204b26,size:32,align:8,pid:0,op:4,payload:8\n",
    "runtime_enable_request=c0104b25,size:16,align:8,offsets:0,8,12\n",
    "runtime_enable_modes=enable:1,ttmp_save:2;target_profile=enable1-r_debug0-ttmp0-capabilities0\n",
    "runtime_states=disabled:0,enabled:1,enabled_busy:2,enabled_error:3\n",
    "runtime_info=size:16,queue_snapshot=size:64,device_snapshot=size:120,csa_header=size:40\n",
    "ops=enable:0,disable:1,send_runtime:2,exceptions:3,override:4,launch:5,suspend:6,resume:7,watch_set:8,watch_clear:9,flags:10,event:11,info:12,queues:13,devices:14\n",
    "authority=wire-layout-only,no-ptrace-session,no-register-or-source-state\n",
);

/// SHA-256 of [`KFD_DEBUG_TRAP_SCHEMA_MANIFEST_V1`].
pub const KFD_DEBUG_TRAP_SCHEMA_MANIFEST_SHA256_V1: &str =
    "16c606b26960c5386198d48c595b248164ba273d1b3e9032736707f5f0336e1d";

pub const KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1: usize = 4096;
pub const KFD_DEBUG_TRAP_MAX_WATCHPOINTS_PER_DEVICE_V1: usize = 4;

pub const KFD_RUNTIME_DEBUG_MODE_ENABLE_MASK_V1: u32 = 1;
pub const KFD_RUNTIME_DEBUG_MODE_TTMP_SAVE_MASK_V1: u32 = 2;

pub const KFD_DBG_QUEUE_ERROR_MASK_V1: u32 = 1 << 30;
pub const KFD_DBG_QUEUE_INVALID_MASK_V1: u32 = 1 << 31;

pub const KFD_DEBUG_TRAP_QUEUE_EXCEPTION_MASK_V1: u64 = 0x0000_0000_607f_803f;
pub const KFD_DEBUG_TRAP_DEVICE_EXCEPTION_MASK_V1: u64 = 0x0000_000f_8000_0000;
pub const KFD_DEBUG_TRAP_PROCESS_EXCEPTION_MASK_V1: u64 = 0x0001_8000_0000_0000;
pub const KFD_DEBUG_TRAP_ALL_EXCEPTION_MASK_V1: u64 = KFD_DEBUG_TRAP_QUEUE_EXCEPTION_MASK_V1
    | KFD_DEBUG_TRAP_DEVICE_EXCEPTION_MASK_V1
    | KFD_DEBUG_TRAP_PROCESS_EXCEPTION_MASK_V1;

pub const KFD_DBG_TRAP_MASK_FP_INVALID_V1: u32 = 1;
pub const KFD_DBG_TRAP_MASK_FP_INPUT_DENORMAL_V1: u32 = 2;
pub const KFD_DBG_TRAP_MASK_FP_DIVIDE_BY_ZERO_V1: u32 = 4;
pub const KFD_DBG_TRAP_MASK_FP_OVERFLOW_V1: u32 = 8;
pub const KFD_DBG_TRAP_MASK_FP_UNDERFLOW_V1: u32 = 16;
pub const KFD_DBG_TRAP_MASK_FP_INEXACT_V1: u32 = 32;
pub const KFD_DBG_TRAP_MASK_INT_DIVIDE_BY_ZERO_V1: u32 = 64;
pub const KFD_DBG_TRAP_MASK_DBG_ADDRESS_WATCH_V1: u32 = 128;
pub const KFD_DBG_TRAP_MASK_DBG_MEMORY_VIOLATION_V1: u32 = 256;
pub const KFD_DBG_TRAP_MASK_TRAP_ON_WAVE_START_V1: u32 = 1 << 30;
pub const KFD_DBG_TRAP_MASK_TRAP_ON_WAVE_END_V1: u32 = 1 << 31;
pub const KFD_DBG_TRAP_ALL_LAUNCH_OVERRIDE_MASK_V1: u32 = 0xc000_01ff;

pub const KFD_DBG_TRAP_FLAG_SINGLE_MEM_OP_V1: u32 = 1;
pub const KFD_DBG_TRAP_FLAG_SINGLE_ALU_OP_V1: u32 = 2;
pub const KFD_DBG_TRAP_ALL_FLAGS_V1: u32 = 3;

pub const KFD_CAP_WATCHPOINTS_SUPPORTED_V1: u32 = 0x0000_0080;
pub const KFD_CAP_TRAP_DEBUG_SUPPORTED_V1: u32 = 0x0000_8000;
pub const KFD_CAP_TRAP_DEBUG_LAUNCH_OVERRIDE_SUPPORTED_V1: u32 = 0x0001_0000;
pub const KFD_CAP_TRAP_DEBUG_LAUNCH_MODE_SUPPORTED_V1: u32 = 0x0002_0000;
pub const KFD_CAP_TRAP_DEBUG_PRECISE_MEMORY_SUPPORTED_V1: u32 = 0x0004_0000;
pub const KFD_CAP_TRAP_DEBUG_FIRMWARE_SUPPORTED_V1: u32 = 0x2000_0000;
pub const KFD_CAP_TRAP_DEBUG_PRECISE_ALU_SUPPORTED_V1: u32 = 0x4000_0000;
pub const KFD_DEBUG_PROP_WATCH_MASK_LOW_BITS_V1: u32 = 0x0000_000f;
pub const KFD_DEBUG_PROP_WATCH_MASK_HIGH_BITS_V1: u32 = 0x0000_03f0;
pub const KFD_DEBUG_PROP_DISPATCH_INFO_ALWAYS_VALID_V1: u32 = 0x0000_0400;
pub const KFD_DEBUG_PROP_WATCHPOINTS_EXCLUSIVE_V1: u32 = 0x0000_0800;

pub const KFD_RUNTIME_STATE_DISABLED_V1: u32 = 0;
pub const KFD_RUNTIME_STATE_ENABLED_V1: u32 = 1;
pub const KFD_RUNTIME_STATE_ENABLED_BUSY_V1: u32 = 2;
pub const KFD_RUNTIME_STATE_ENABLED_ERROR_V1: u32 = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdDebugTrapOperationV1 {
    Enable = 0,
    Disable = 1,
    SendRuntimeEvent = 2,
    SetExceptionsEnabled = 3,
    SetWaveLaunchOverride = 4,
    SetWaveLaunchMode = 5,
    SuspendQueues = 6,
    ResumeQueues = 7,
    SetNodeAddressWatch = 8,
    ClearNodeAddressWatch = 9,
    SetFlags = 10,
    QueryDebugEvent = 11,
    QueryExceptionInfo = 12,
    GetQueueSnapshot = 13,
    GetDeviceSnapshot = 14,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdDebugTrapExceptionCodeV1 {
    QueueWaveAbort = 1,
    QueueWaveTrap = 2,
    QueueWaveMathError = 3,
    QueueWaveIllegalInstruction = 4,
    QueueWaveMemoryViolation = 5,
    QueueWaveApertureViolation = 6,
    QueuePacketDispatchDimensionsInvalid = 16,
    QueuePacketDispatchGroupSegmentSizeInvalid = 17,
    QueuePacketDispatchCodeInvalid = 18,
    QueuePacketReserved = 19,
    QueuePacketUnsupported = 20,
    QueuePacketDispatchWorkgroupSizeInvalid = 21,
    QueuePacketDispatchRegisterInvalid = 22,
    QueuePacketVendorUnsupported = 23,
    QueuePreemptionError = 30,
    QueueNew = 31,
    DeviceQueueDelete = 32,
    DeviceMemoryViolation = 33,
    DeviceRasError = 34,
    DeviceFatalHalt = 35,
    DeviceNew = 36,
    ProcessRuntime = 48,
    ProcessDeviceRemove = 49,
}

impl KfdDebugTrapExceptionCodeV1 {
    pub const fn mask(self) -> u64 {
        1_u64 << (self as u32 - 1)
    }

    pub const fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            1 => Self::QueueWaveAbort,
            2 => Self::QueueWaveTrap,
            3 => Self::QueueWaveMathError,
            4 => Self::QueueWaveIllegalInstruction,
            5 => Self::QueueWaveMemoryViolation,
            6 => Self::QueueWaveApertureViolation,
            16 => Self::QueuePacketDispatchDimensionsInvalid,
            17 => Self::QueuePacketDispatchGroupSegmentSizeInvalid,
            18 => Self::QueuePacketDispatchCodeInvalid,
            19 => Self::QueuePacketReserved,
            20 => Self::QueuePacketUnsupported,
            21 => Self::QueuePacketDispatchWorkgroupSizeInvalid,
            22 => Self::QueuePacketDispatchRegisterInvalid,
            23 => Self::QueuePacketVendorUnsupported,
            30 => Self::QueuePreemptionError,
            31 => Self::QueueNew,
            32 => Self::DeviceQueueDelete,
            33 => Self::DeviceMemoryViolation,
            34 => Self::DeviceRasError,
            35 => Self::DeviceFatalHalt,
            36 => Self::DeviceNew,
            48 => Self::ProcessRuntime,
            49 => Self::ProcessDeviceRemove,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdDebugTrapOverrideModeV1 {
    Or = 0,
    Replace = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdDebugTrapWaveLaunchModeV1 {
    Normal = 0,
    Halt = 1,
    Debug = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum KfdDebugTrapAddressWatchModeV1 {
    Read = 0,
    NonRead = 1,
    Atomic = 2,
    All = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdDebugRuntimeStateV1 {
    Disabled,
    Enabled,
    EnabledBusy,
    EnabledError,
}

impl KfdDebugRuntimeStateV1 {
    pub const fn from_wire(value: u32) -> Option<Self> {
        Some(match value {
            KFD_RUNTIME_STATE_DISABLED_V1 => Self::Disabled,
            KFD_RUNTIME_STATE_ENABLED_V1 => Self::Enabled,
            KFD_RUNTIME_STATE_ENABLED_BUSY_V1 => Self::EnabledBusy,
            KFD_RUNTIME_STATE_ENABLED_ERROR_V1 => Self::EnabledError,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugExceptionMaskV1(u64);

impl KfdDebugExceptionMaskV1 {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(KFD_DEBUG_TRAP_ALL_EXCEPTION_MASK_V1);

    pub const fn new(bits: u64) -> Option<Self> {
        if bits & !KFD_DEBUG_TRAP_ALL_EXCEPTION_MASK_V1 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    pub const fn from_code(code: KfdDebugTrapExceptionCodeV1) -> Self {
        Self(code.mask())
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn contains(self, code: KfdDebugTrapExceptionCodeV1) -> bool {
        self.0 & code.mask() != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugLaunchOverrideMaskV1(u32);

impl KfdDebugLaunchOverrideMaskV1 {
    pub const NONE: Self = Self(0);

    pub const fn new(bits: u32) -> Option<Self> {
        if bits & !KFD_DBG_TRAP_ALL_LAUNCH_OVERRIDE_MASK_V1 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugTrapFlagsV1(u32);

impl KfdDebugTrapFlagsV1 {
    pub const NONE: Self = Self(0);
    pub const SINGLE_MEMORY_OPERATION: Self = Self(KFD_DBG_TRAP_FLAG_SINGLE_MEM_OP_V1);
    pub const SINGLE_ALU_OPERATION: Self = Self(KFD_DBG_TRAP_FLAG_SINGLE_ALU_OP_V1);

    pub const fn new(bits: u32) -> Option<Self> {
        if bits & !KFD_DBG_TRAP_ALL_FLAGS_V1 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdRuntimeInfoV1 {
    pub r_debug: u64,
    pub runtime_state: u32,
    pub ttmp_setup: u32,
}

impl KfdRuntimeInfoV1 {
    pub const fn state(self) -> Option<KfdDebugRuntimeStateV1> {
        KfdDebugRuntimeStateV1::from_wire(self.runtime_state)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdDebugQueueSnapshotEntryV1 {
    pub exception_status: u64,
    pub ring_base_address: u64,
    pub write_pointer_address: u64,
    pub read_pointer_address: u64,
    pub ctx_save_restore_address: u64,
    pub queue_id: u32,
    pub gpu_id: u32,
    pub ring_size: u32,
    pub queue_type: u32,
    pub ctx_save_restore_area_size: u32,
    pub reserved: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdDebugDeviceSnapshotEntryV1 {
    pub exception_status: u64,
    pub lds_base: u64,
    pub lds_limit: u64,
    pub scratch_base: u64,
    pub scratch_limit: u64,
    pub gpuvm_base: u64,
    pub gpuvm_limit: u64,
    pub gpu_id: u32,
    pub location_id: u32,
    pub vendor_id: u32,
    pub device_id: u32,
    pub revision_id: u32,
    pub subsystem_vendor_id: u32,
    pub subsystem_device_id: u32,
    pub fw_version: u32,
    pub gfx_target_version: u32,
    pub simd_count: u32,
    pub max_waves_per_simd: u32,
    pub array_count: u32,
    pub simd_arrays_per_engine: u32,
    pub num_xcc: u32,
    pub capability: u32,
    pub debug_prop: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdDebugContextSaveAreaHeaderV1 {
    pub control_stack_offset: u32,
    pub control_stack_size: u32,
    pub wave_state_offset: u32,
    pub wave_state_size: u32,
    pub debug_offset: u32,
    pub debug_size: u32,
    pub err_payload_addr: u64,
    pub err_event_id: u32,
    pub reserved: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapEnableArgsV1 {
    pub exception_mask: u64,
    pub runtime_info_address: u64,
    pub runtime_info_size: u32,
    pub notifier_fd: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapSendRuntimeEventArgsV1 {
    pub exception_mask: u64,
    pub gpu_id: u32,
    pub queue_id: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapSetExceptionsArgsV1 {
    pub exception_mask: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapLaunchOverrideArgsV1 {
    pub override_mode: u32,
    pub enable_mask: u32,
    pub support_request_mask: u32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapLaunchModeArgsV1 {
    pub launch_mode: u32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapSuspendQueuesArgsV1 {
    pub exception_mask: u64,
    pub queue_array_address: u64,
    pub queue_count: u32,
    pub grace_period: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapResumeQueuesArgsV1 {
    pub queue_array_address: u64,
    pub queue_count: u32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapSetAddressWatchArgsV1 {
    pub address: u64,
    pub mode: u32,
    pub mask: u32,
    pub gpu_id: u32,
    pub watch_id: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapClearAddressWatchArgsV1 {
    pub gpu_id: u32,
    pub watch_id: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapSetFlagsArgsV1 {
    pub flags: u32,
    pub pad: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapQueryEventArgsV1 {
    pub exception_mask: u64,
    pub gpu_id: u32,
    pub queue_id: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapQueryExceptionInfoArgsV1 {
    pub info_address: u64,
    pub info_size: u32,
    pub source_id: u32,
    pub exception_code: u32,
    pub clear_exception: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapQueueSnapshotArgsV1 {
    pub exception_mask: u64,
    pub snapshot_buffer_address: u64,
    pub queue_count: u32,
    pub entry_size: u32,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapDeviceSnapshotArgsV1 {
    pub exception_mask: u64,
    pub snapshot_buffer_address: u64,
    pub device_count: u32,
    pub entry_size: u32,
}

/// Union-free wire representation of `struct kfd_ioctl_dbg_trap_args`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct KfdIoctlDebugTrapArgsV1 {
    pid: u32,
    op: u32,
    payload: [u64; 3],
}

const fn pair_u32(low: u32, high: u32) -> u64 {
    (low as u64) | ((high as u64) << 32)
}

const fn low_u32(value: u64) -> u32 {
    value as u32
}

const fn high_u32(value: u64) -> u32 {
    (value >> 32) as u32
}

impl KfdIoctlDebugTrapArgsV1 {
    const fn new(pid: u32, op: KfdDebugTrapOperationV1, payload: [u64; 3]) -> Self {
        Self {
            pid,
            op: op as u32,
            payload,
        }
    }

    pub const fn enable(
        pid: u32,
        exceptions: KfdDebugExceptionMaskV1,
        runtime_info_address: u64,
        runtime_info_size: u32,
        notifier_fd: u32,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::Enable,
            [
                exceptions.bits(),
                runtime_info_address,
                pair_u32(runtime_info_size, notifier_fd),
            ],
        )
    }

    pub const fn disable(pid: u32) -> Self {
        Self::new(pid, KfdDebugTrapOperationV1::Disable, [0; 3])
    }

    pub const fn send_runtime_event(
        pid: u32,
        exceptions: KfdDebugExceptionMaskV1,
        gpu_id: u32,
        queue_id: u32,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SendRuntimeEvent,
            [exceptions.bits(), pair_u32(gpu_id, queue_id), 0],
        )
    }

    pub const fn set_exceptions(pid: u32, exceptions: KfdDebugExceptionMaskV1) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SetExceptionsEnabled,
            [exceptions.bits(), 0, 0],
        )
    }

    pub const fn launch_override(
        pid: u32,
        mode: KfdDebugTrapOverrideModeV1,
        enabled: KfdDebugLaunchOverrideMaskV1,
        support_requested: KfdDebugLaunchOverrideMaskV1,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SetWaveLaunchOverride,
            [
                pair_u32(mode as u32, enabled.bits()),
                pair_u32(support_requested.bits(), 0),
                0,
            ],
        )
    }

    pub const fn launch_mode(pid: u32, mode: KfdDebugTrapWaveLaunchModeV1) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SetWaveLaunchMode,
            [pair_u32(mode as u32, 0), 0, 0],
        )
    }

    pub const fn suspend_queues(
        pid: u32,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
        queue_array_address: u64,
        queue_count: u32,
        grace_period: u32,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SuspendQueues,
            [
                exceptions_to_clear.bits(),
                queue_array_address,
                pair_u32(queue_count, grace_period),
            ],
        )
    }

    pub const fn resume_queues(pid: u32, queue_array_address: u64, queue_count: u32) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::ResumeQueues,
            [queue_array_address, pair_u32(queue_count, 0), 0],
        )
    }

    pub const fn set_address_watch(
        pid: u32,
        address: u64,
        mode: KfdDebugTrapAddressWatchModeV1,
        mask: u32,
        gpu_id: u32,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SetNodeAddressWatch,
            [address, pair_u32(mode as u32, mask), pair_u32(gpu_id, 0)],
        )
    }

    pub const fn clear_address_watch(pid: u32, gpu_id: u32, watch_id: u32) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::ClearNodeAddressWatch,
            [pair_u32(gpu_id, watch_id), 0, 0],
        )
    }

    pub const fn set_flags(pid: u32, flags: KfdDebugTrapFlagsV1) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::SetFlags,
            [pair_u32(flags.bits(), 0), 0, 0],
        )
    }

    pub const fn query_event(pid: u32, exceptions_to_clear: KfdDebugExceptionMaskV1) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::QueryDebugEvent,
            [exceptions_to_clear.bits(), 0, 0],
        )
    }

    pub const fn query_exception_info(
        pid: u32,
        info_address: u64,
        info_size: u32,
        source_id: u32,
        code: KfdDebugTrapExceptionCodeV1,
        clear: bool,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::QueryExceptionInfo,
            [
                info_address,
                pair_u32(info_size, source_id),
                pair_u32(code as u32, clear as u32),
            ],
        )
    }

    pub const fn queue_snapshot(
        pid: u32,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
        buffer_address: u64,
        count: u32,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::GetQueueSnapshot,
            [
                exceptions_to_clear.bits(),
                buffer_address,
                pair_u32(count, size_of::<KfdDebugQueueSnapshotEntryV1>() as u32),
            ],
        )
    }

    pub const fn device_snapshot(
        pid: u32,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
        buffer_address: u64,
        count: u32,
    ) -> Self {
        Self::new(
            pid,
            KfdDebugTrapOperationV1::GetDeviceSnapshot,
            [
                exceptions_to_clear.bits(),
                buffer_address,
                pair_u32(count, size_of::<KfdDebugDeviceSnapshotEntryV1>() as u32),
            ],
        )
    }

    pub const fn pid(self) -> u32 {
        self.pid
    }

    pub const fn operation_raw(self) -> u32 {
        self.op
    }

    pub const fn payload_words(self) -> [u64; 3] {
        self.payload
    }

    pub const fn enable_runtime_info_size(self) -> u32 {
        low_u32(self.payload[2])
    }

    pub const fn returned_launch_override(self) -> KfdDebugLaunchOverrideMaskV1 {
        KfdDebugLaunchOverrideMaskV1(high_u32(self.payload[0]))
    }

    pub const fn returned_launch_support(self) -> KfdDebugLaunchOverrideMaskV1 {
        KfdDebugLaunchOverrideMaskV1(low_u32(self.payload[1]))
    }

    pub const fn returned_flags(self) -> KfdDebugTrapFlagsV1 {
        KfdDebugTrapFlagsV1(low_u32(self.payload[0]))
    }

    pub const fn returned_watch_id(self) -> u32 {
        high_u32(self.payload[2])
    }

    pub const fn returned_event_mask(self) -> Option<KfdDebugExceptionMaskV1> {
        KfdDebugExceptionMaskV1::new(self.payload[0])
    }

    pub const fn returned_event_gpu_id(self) -> u32 {
        low_u32(self.payload[1])
    }

    pub const fn returned_event_queue_id(self) -> u32 {
        high_u32(self.payload[1])
    }

    pub const fn returned_info_size(self) -> u32 {
        low_u32(self.payload[1])
    }

    pub const fn returned_snapshot_count(self) -> u32 {
        low_u32(self.payload[2])
    }

    pub const fn returned_snapshot_entry_size(self) -> u32 {
        high_u32(self.payload[2])
    }

    /// Test/oracle constructor for kernel-mutated wire words.
    pub const fn from_untrusted_wire(pid: u32, op: u32, payload: [u64; 3]) -> Self {
        Self { pid, op, payload }
    }
}

/// Exact Linux generic-IOC encoding of `AMDKFD_IOC_DBG_TRAP`.
pub const AMDKFD_IOC_DBG_TRAP: IoctlRequest = encode_admitted_ioctl(
    IoctlDirection::ReadWrite,
    AMDKFD_IOCTL_BASE,
    0x26,
    size_of::<KfdIoctlDebugTrapArgsV1>(),
);

const _: () = {
    assert!(KFD_UAPI_SCHEMA_MANIFEST_SHA256.len() == 64);
    assert!(size_of::<KfdIoctlRuntimeEnableArgsV1>() == 16);
    assert!(align_of::<KfdIoctlRuntimeEnableArgsV1>() == 8);
    assert!(AMDKFD_IOC_RUNTIME_ENABLE == 0xc010_4b25);
    assert!(size_of::<KfdRuntimeInfoV1>() == 16);
    assert!(align_of::<KfdRuntimeInfoV1>() == 8);
    assert!(size_of::<KfdDebugQueueSnapshotEntryV1>() == 64);
    assert!(align_of::<KfdDebugQueueSnapshotEntryV1>() == 8);
    assert!(offset_of!(KfdDebugQueueSnapshotEntryV1, queue_id) == 40);
    assert!(offset_of!(KfdDebugQueueSnapshotEntryV1, reserved) == 60);
    assert!(size_of::<KfdDebugDeviceSnapshotEntryV1>() == 120);
    assert!(align_of::<KfdDebugDeviceSnapshotEntryV1>() == 8);
    assert!(offset_of!(KfdDebugDeviceSnapshotEntryV1, gpu_id) == 56);
    assert!(offset_of!(KfdDebugDeviceSnapshotEntryV1, gfx_target_version) == 88);
    assert!(offset_of!(KfdDebugDeviceSnapshotEntryV1, debug_prop) == 116);
    assert!(size_of::<KfdDebugContextSaveAreaHeaderV1>() == 40);
    assert!(align_of::<KfdDebugContextSaveAreaHeaderV1>() == 8);
    assert!(offset_of!(KfdDebugContextSaveAreaHeaderV1, err_payload_addr) == 24);
    assert!(size_of::<KfdIoctlDebugTrapEnableArgsV1>() == 24);
    assert!(align_of::<KfdIoctlDebugTrapEnableArgsV1>() == 8);
    assert!(size_of::<KfdIoctlDebugTrapSendRuntimeEventArgsV1>() == 16);
    assert!(size_of::<KfdIoctlDebugTrapSetExceptionsArgsV1>() == 8);
    assert!(size_of::<KfdIoctlDebugTrapLaunchOverrideArgsV1>() == 16);
    assert!(align_of::<KfdIoctlDebugTrapLaunchOverrideArgsV1>() == 4);
    assert!(size_of::<KfdIoctlDebugTrapLaunchModeArgsV1>() == 8);
    assert!(size_of::<KfdIoctlDebugTrapSuspendQueuesArgsV1>() == 24);
    assert!(size_of::<KfdIoctlDebugTrapResumeQueuesArgsV1>() == 16);
    assert!(size_of::<KfdIoctlDebugTrapSetAddressWatchArgsV1>() == 24);
    assert!(size_of::<KfdIoctlDebugTrapClearAddressWatchArgsV1>() == 8);
    assert!(size_of::<KfdIoctlDebugTrapSetFlagsArgsV1>() == 8);
    assert!(size_of::<KfdIoctlDebugTrapQueryEventArgsV1>() == 16);
    assert!(size_of::<KfdIoctlDebugTrapQueryExceptionInfoArgsV1>() == 24);
    assert!(size_of::<KfdIoctlDebugTrapQueueSnapshotArgsV1>() == 24);
    assert!(size_of::<KfdIoctlDebugTrapDeviceSnapshotArgsV1>() == 24);
    assert!(size_of::<KfdIoctlDebugTrapArgsV1>() == 32);
    assert!(align_of::<KfdIoctlDebugTrapArgsV1>() == 8);
    assert!(offset_of!(KfdIoctlDebugTrapArgsV1, pid) == 0);
    assert!(offset_of!(KfdIoctlDebugTrapArgsV1, op) == 4);
    assert!(offset_of!(KfdIoctlDebugTrapArgsV1, payload) == 8);
    assert!(AMDKFD_IOC_DBG_TRAP == 0xc020_4b26);
};

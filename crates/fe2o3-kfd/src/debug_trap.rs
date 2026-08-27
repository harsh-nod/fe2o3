//! Safe state, observation, and owned live-session model for KFD dbg-trap.
//!
//! Public values never grant a descriptor, target-memory, queue, or register
//! capability. The live session proves Linux ptrace-parent ownership, pins the
//! target process with a pidfd, owns a nonblocking notifier pipe, and admits an
//! independently opened `/dev/kfd` descriptor before enabling debug-trap.

use core::fmt;
use core::num::NonZeroU32;
use std::fs;
use std::marker::PhantomData;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::rc::Rc;

use fe2o3_kfd_uapi::{
    KFD_CAP_TRAP_DEBUG_FIRMWARE_SUPPORTED_V1, KFD_CAP_TRAP_DEBUG_LAUNCH_MODE_SUPPORTED_V1,
    KFD_CAP_TRAP_DEBUG_LAUNCH_OVERRIDE_SUPPORTED_V1, KFD_CAP_TRAP_DEBUG_PRECISE_ALU_SUPPORTED_V1,
    KFD_CAP_TRAP_DEBUG_PRECISE_MEMORY_SUPPORTED_V1, KFD_CAP_TRAP_DEBUG_SUPPORTED_V1,
    KFD_CAP_WATCHPOINTS_SUPPORTED_V1, KFD_DBG_QUEUE_ERROR_MASK_V1, KFD_DBG_QUEUE_INVALID_MASK_V1,
    KFD_DEBUG_PROP_DISPATCH_INFO_ALWAYS_VALID_V1, KFD_DEBUG_PROP_WATCHPOINTS_EXCLUSIVE_V1,
    KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1, KFD_DEBUG_TRAP_MAX_WATCHPOINTS_PER_DEVICE_V1,
    KfdDebugDeviceSnapshotEntryV1, KfdDebugExceptionMaskV1, KfdDebugLaunchOverrideMaskV1,
    KfdDebugQueueSnapshotEntryV1, KfdDebugRuntimeStateV1, KfdDebugTrapAddressWatchModeV1,
    KfdDebugTrapExceptionCodeV1, KfdDebugTrapFlagsV1, KfdDebugTrapOverrideModeV1,
    KfdDebugTrapWaveLaunchModeV1, KfdHsaMemoryExceptionDataV1, KfdRuntimeInfoV1,
};

pub const KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_V1: &str = concat!(
    "profile=fe2o3-kfd-debug-session-foundation-v1\n",
    "uapi=linux-kfd-debug-trap-1.18-x86_64-le-v1\n",
    "uapi_sha256=16c606b26960c5386198d48c595b248164ba273d1b3e9032736707f5f0336e1d\n",
    "target=ptrace-owned-process,pid-nonzero,not-self\n",
    "events=typed-mask,query-until-empty,clear-explicitly\n",
    "handshake=query-process-runtime-info,send-runtime-event,enable-and-disable-transitions\n",
    "snapshots=bounded,redacted,no-ring-control-cwsr-or-aperture-addresses\n",
    "hardware=runtime-enabled-only,suspend-resume,launch-mode,launch-override,flags,address-watch\n",
    "ownership=session-retains-suspended-queues,watches,and-restorable-process-settings\n",
    "cleanup=resume,clear-watch,normal-launch,restore-override,restore-flags,disable\n",
    "live=ptrace-parent-status-sandwich,pidfd-retained-and-revalidated,exact-uapi-owned-kfd,nonblocking-owned-notifier\n",
    "target_runtime=separate-current-process-owned-runtime-enable-mode1-r_debug0-no-ttmp-no-capabilities\n",
    "missing=target-memory,wave-register-cwsr-decoder,source-map\n",
    "authority=owned-live-session-and-redacted-observation;no-fd-pointer-or-target-address-export\n",
);

pub const KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_SHA256_V1: &str =
    "c57670bb8234c6149f6188861da580dc53dc6c62e22e3088b582e5195fafefb7";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdDebugPlanErrorV1 {
    TargetPidZero,
    TargetPidOutOfRange,
    TargetIsDebugger,
    SnapshotLimitZero,
    SnapshotLimitExceeded,
}

impl fmt::Display for KfdDebugPlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KfdDebugPlanErrorV1 {}

/// Inert, bounded request for a KFD debug session.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugSessionPlanV1 {
    target_pid: NonZeroU32,
    exceptions: KfdDebugExceptionMaskV1,
    snapshot_limit: usize,
}

impl KfdDebugSessionPlanV1 {
    pub fn new(
        target_pid: u32,
        current_process_pid: u32,
        exceptions: KfdDebugExceptionMaskV1,
        snapshot_limit: usize,
    ) -> Result<Self, KfdDebugPlanErrorV1> {
        let Some(target_pid) = NonZeroU32::new(target_pid) else {
            return Err(KfdDebugPlanErrorV1::TargetPidZero);
        };
        if target_pid.get() > i32::MAX as u32 {
            return Err(KfdDebugPlanErrorV1::TargetPidOutOfRange);
        }
        if target_pid.get() == current_process_pid {
            return Err(KfdDebugPlanErrorV1::TargetIsDebugger);
        }
        if snapshot_limit == 0 {
            return Err(KfdDebugPlanErrorV1::SnapshotLimitZero);
        }
        if snapshot_limit > KFD_DEBUG_TRAP_MAX_SNAPSHOT_ENTRIES_V1 {
            return Err(KfdDebugPlanErrorV1::SnapshotLimitExceeded);
        }
        Ok(Self {
            target_pid,
            exceptions,
            snapshot_limit,
        })
    }

    pub const fn target_pid(self) -> u32 {
        self.target_pid.get()
    }

    pub const fn exceptions(self) -> KfdDebugExceptionMaskV1 {
        self.exceptions
    }

    pub const fn snapshot_limit(self) -> usize {
        self.snapshot_limit
    }

    pub const fn manifest(self) -> &'static str {
        KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_V1
    }

    pub const fn manifest_sha256(self) -> &'static str {
        KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_SHA256_V1
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugRuntimeObservationV1 {
    state: KfdDebugRuntimeStateV1,
    ttmp_setup: bool,
    runtime_metadata_present: bool,
}

impl KfdDebugRuntimeObservationV1 {
    pub const fn state(self) -> KfdDebugRuntimeStateV1 {
        self.state
    }

    pub const fn ttmp_setup(self) -> bool {
        self.ttmp_setup
    }

    pub const fn runtime_metadata_present(self) -> bool {
        self.runtime_metadata_present
    }

    pub const fn hardware_operations_available(self) -> bool {
        matches!(self.state, KfdDebugRuntimeStateV1::Enabled)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugQueueObservationV1 {
    exception_status: KfdDebugExceptionMaskV1,
    queue_id: u32,
    gpu_id: u32,
    ring_size: u32,
    queue_type: u32,
    context_save_area_size: u32,
}

impl KfdDebugQueueObservationV1 {
    pub const fn exception_status(self) -> KfdDebugExceptionMaskV1 {
        self.exception_status
    }
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn ring_size(self) -> u32 {
        self.ring_size
    }
    pub const fn queue_type(self) -> u32 {
        self.queue_type
    }
    pub const fn context_save_area_size(self) -> u32 {
        self.context_save_area_size
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugDeviceObservationV1 {
    exception_status: KfdDebugExceptionMaskV1,
    gpu_id: u32,
    location_id: u32,
    vendor_id: u32,
    device_id: u32,
    revision_id: u32,
    firmware_version: u32,
    gfx_target_version: u32,
    simd_count: u32,
    max_waves_per_simd: u32,
    array_count: u32,
    simd_arrays_per_engine: u32,
    xcc_count: u32,
    capability_bits: u32,
    debug_property_bits: u32,
}

impl KfdDebugDeviceObservationV1 {
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn gfx_target_version(self) -> u32 {
        self.gfx_target_version
    }
    pub const fn capability_bits(self) -> u32 {
        self.capability_bits
    }
    pub const fn debug_property_bits(self) -> u32 {
        self.debug_property_bits
    }
    pub const fn xcc_count(self) -> u32 {
        self.xcc_count
    }
    pub const fn supports_trap_debug(self) -> bool {
        self.capability_bits & KFD_CAP_TRAP_DEBUG_SUPPORTED_V1 != 0
    }
    pub const fn supports_watchpoints(self) -> bool {
        self.capability_bits & KFD_CAP_WATCHPOINTS_SUPPORTED_V1 != 0
    }
    pub const fn supports_launch_override(self) -> bool {
        self.capability_bits & KFD_CAP_TRAP_DEBUG_LAUNCH_OVERRIDE_SUPPORTED_V1 != 0
    }
    pub const fn supports_launch_mode(self) -> bool {
        self.capability_bits & KFD_CAP_TRAP_DEBUG_LAUNCH_MODE_SUPPORTED_V1 != 0
    }
    pub const fn supports_precise_memory_operations(self) -> bool {
        self.capability_bits & KFD_CAP_TRAP_DEBUG_PRECISE_MEMORY_SUPPORTED_V1 != 0
    }
    pub const fn supports_precise_alu_operations(self) -> bool {
        self.capability_bits & KFD_CAP_TRAP_DEBUG_PRECISE_ALU_SUPPORTED_V1 != 0
    }
    pub const fn supports_debug_firmware(self) -> bool {
        self.capability_bits & KFD_CAP_TRAP_DEBUG_FIRMWARE_SUPPORTED_V1 != 0
    }
    pub const fn dispatch_info_always_valid(self) -> bool {
        self.debug_property_bits & KFD_DEBUG_PROP_DISPATCH_INFO_ALWAYS_VALID_V1 != 0
    }
    pub const fn watchpoints_exclusive(self) -> bool {
        self.debug_property_bits & KFD_DEBUG_PROP_WATCHPOINTS_EXCLUSIVE_V1 != 0
    }
    pub const fn exception_status(self) -> KfdDebugExceptionMaskV1 {
        self.exception_status
    }
    pub const fn identity_words(self) -> [u32; 5] {
        [
            self.location_id,
            self.vendor_id,
            self.device_id,
            self.revision_id,
            self.firmware_version,
        ]
    }
    pub const fn geometry_words(self) -> [u32; 4] {
        [
            self.simd_count,
            self.max_waves_per_simd,
            self.array_count,
            self.simd_arrays_per_engine,
        ]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugEventObservationV1 {
    exceptions: KfdDebugExceptionMaskV1,
    gpu_id: u32,
    queue_id: u32,
}

impl KfdDebugEventObservationV1 {
    pub const fn exceptions(self) -> KfdDebugExceptionMaskV1 {
        self.exceptions
    }
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdDebugExceptionInfoV1 {
    NoPayload,
    Runtime(KfdDebugRuntimeObservationV1),
    DeviceMemoryViolation {
        failure_words: [u32; 4],
        fault_address_present: bool,
        gpu_id: u32,
        error_type: u32,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KfdDebugQueueOperationStateV1 {
    Complete,
    HardwareError,
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugQueueOperationObservationV1 {
    queue_id: u32,
    state: KfdDebugQueueOperationStateV1,
}

impl KfdDebugQueueOperationObservationV1 {
    pub const fn queue_id(self) -> u32 {
        self.queue_id
    }
    pub const fn state(self) -> KfdDebugQueueOperationStateV1 {
        self.state
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdDebugAddressWatchObservationV1 {
    gpu_id: u32,
    watch_id: u32,
}

impl KfdDebugAddressWatchObservationV1 {
    pub const fn gpu_id(self) -> u32 {
        self.gpu_id
    }
    pub const fn watch_id(self) -> u32 {
        self.watch_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum KfdDebugSessionErrorV1 {
    Backend(&'static str),
    InvalidRuntimeInfo,
    RuntimeNotEnabled,
    SnapshotCount,
    SnapshotEntry,
    InvalidExceptionMask,
    ExceptionInfoKind,
    QueueListEmpty,
    QueueListTooLarge,
    DuplicateQueue,
    QueueResult,
    DuplicateWatch,
    WatchCapacity,
    RuntimeEventRequired,
    SessionFinished,
    Cleanup(&'static str),
}

impl fmt::Display for KfdDebugSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for KfdDebugSessionErrorV1 {}

/// Failure at the Linux ownership/admission boundary for a live session.
#[derive(Debug)]
pub enum KfdLiveDebugSessionErrorV1 {
    TargetStatus {
        operation: &'static str,
        source: std::io::Error,
    },
    MalformedTargetStatus,
    TargetNotProcessLeader {
        target_pid: u32,
        thread_group_id: u32,
    },
    TargetNotLive,
    NotPtraceOwner {
        debugger_tid: u32,
        tracer_tid: u32,
    },
    PidfdOpen(rustix::io::Errno),
    PidfdStatus(std::io::Error),
    PidfdIdentityChanged,
    Kfd(crate::KfdAdapterError),
    NotificationPipe(rustix::io::Errno),
    NotificationRead(rustix::io::Errno),
    NotificationClosed,
    InvalidNotification,
    NotificationLimit,
    Session(KfdDebugSessionErrorV1),
}

impl fmt::Display for KfdLiveDebugSessionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetStatus { operation, source } => {
                write!(formatter, "failed to {operation}: {source}")
            }
            Self::MalformedTargetStatus => formatter.write_str("malformed target proc status"),
            Self::TargetNotProcessLeader {
                target_pid,
                thread_group_id,
            } => write!(
                formatter,
                "target {target_pid} is not process leader {thread_group_id}"
            ),
            Self::TargetNotLive => formatter.write_str("target process is no longer live"),
            Self::NotPtraceOwner {
                debugger_tid,
                tracer_tid,
            } => write!(
                formatter,
                "current task {debugger_tid} is not ptrace owner task {tracer_tid}"
            ),
            Self::PidfdOpen(source) => write!(formatter, "pidfd_open failed: {source}"),
            Self::PidfdStatus(source) => write!(formatter, "failed to inspect pidfd: {source}"),
            Self::PidfdIdentityChanged => {
                formatter.write_str("pidfd no longer identifies the admitted target")
            }
            Self::Kfd(source) => write!(formatter, "KFD admission failed: {source}"),
            Self::NotificationPipe(source) => {
                write!(formatter, "failed to create debug notifier pipe: {source}")
            }
            Self::NotificationRead(source) => {
                write!(formatter, "failed to drain debug notifier: {source}")
            }
            Self::NotificationClosed => formatter.write_str("debug notifier pipe closed"),
            Self::InvalidNotification => {
                formatter.write_str("debug notifier contained an invalid byte")
            }
            Self::NotificationLimit => formatter.write_str("invalid notification drain limit"),
            Self::Session(source) => write!(formatter, "KFD debug session failed: {source}"),
        }
    }
}

impl std::error::Error for KfdLiveDebugSessionErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::TargetStatus { source, .. } | Self::PidfdStatus(source) => Some(source),
            Self::Kfd(source) => Some(source),
            Self::Session(source) => Some(source),
            _ => None,
        }
    }
}

impl From<crate::KfdAdapterError> for KfdLiveDebugSessionErrorV1 {
    fn from(error: crate::KfdAdapterError) -> Self {
        Self::Kfd(error)
    }
}

impl From<KfdDebugSessionErrorV1> for KfdLiveDebugSessionErrorV1 {
    fn from(error: KfdDebugSessionErrorV1) -> Self {
        Self::Session(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TargetStatusV1 {
    thread_group_id: u32,
    tracer_tid: u32,
    live: bool,
}

const MAX_PROC_RECORD_BYTES: u64 = 64 * 1024;
const MAX_NOTIFICATION_DRAIN: usize = 64 * 1024;

fn read_bounded_proc(path: &str) -> Result<String, std::io::Error> {
    use std::io::Read;

    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_PROC_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_PROC_RECORD_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "proc record exceeds bound",
        ));
    }
    String::from_utf8(bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "non-UTF-8 proc record"))
}

fn unique_status_field<'a>(status: &'a str, name: &str) -> Option<&'a str> {
    let mut matches = status.lines().filter_map(|line| {
        let (field, value) = line.split_once(':')?;
        (field == name).then_some(value.trim())
    });
    let value = matches.next()?;
    if matches.next().is_some() {
        None
    } else {
        Some(value)
    }
}

fn parse_target_status(status: &str) -> Option<TargetStatusV1> {
    let thread_group_id = unique_status_field(status, "Tgid")?.parse().ok()?;
    let tracer_tid = unique_status_field(status, "TracerPid")?.parse().ok()?;
    let state = unique_status_field(status, "State")?.bytes().next()?;
    Some(TargetStatusV1 {
        thread_group_id,
        tracer_tid,
        live: !matches!(state, b'X' | b'Z'),
    })
}

fn inspect_target_status(target_pid: u32) -> Result<TargetStatusV1, KfdLiveDebugSessionErrorV1> {
    let path = format!("/proc/{target_pid}/status");
    let status = read_bounded_proc(&path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            KfdLiveDebugSessionErrorV1::TargetNotLive
        } else {
            KfdLiveDebugSessionErrorV1::TargetStatus {
                operation: "read target proc status",
                source,
            }
        }
    })?;
    parse_target_status(&status).ok_or(KfdLiveDebugSessionErrorV1::MalformedTargetStatus)
}

fn admit_ptrace_owner(target_pid: u32) -> Result<(), KfdLiveDebugSessionErrorV1> {
    let status = inspect_target_status(target_pid)?;
    if status.thread_group_id != target_pid {
        return Err(KfdLiveDebugSessionErrorV1::TargetNotProcessLeader {
            target_pid,
            thread_group_id: status.thread_group_id,
        });
    }
    if !status.live {
        return Err(KfdLiveDebugSessionErrorV1::TargetNotLive);
    }
    let debugger_tid = u32::try_from(rustix::thread::gettid().as_raw_pid())
        .map_err(|_| KfdLiveDebugSessionErrorV1::MalformedTargetStatus)?;
    if status.tracer_tid != debugger_tid {
        return Err(KfdLiveDebugSessionErrorV1::NotPtraceOwner {
            debugger_tid,
            tracer_tid: status.tracer_tid,
        });
    }
    Ok(())
}

fn inspect_pidfd_target(pidfd: &OwnedFd) -> Result<Option<u32>, KfdLiveDebugSessionErrorV1> {
    let path = format!("/proc/self/fdinfo/{}", pidfd.as_raw_fd());
    let info = read_bounded_proc(&path).map_err(KfdLiveDebugSessionErrorV1::PidfdStatus)?;
    parse_pidfd_target(&info).ok_or(KfdLiveDebugSessionErrorV1::PidfdIdentityChanged)
}

fn parse_pidfd_target(info: &str) -> Option<Option<u32>> {
    let value = unique_status_field(info, "Pid").and_then(|value| value.parse::<i64>().ok())?;
    Some(u32::try_from(value).ok().filter(|pid| *pid != 0))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BackendExceptionInfoV1 {
    None,
    Runtime(KfdRuntimeInfoV1),
    DeviceMemory(KfdHsaMemoryExceptionDataV1),
}

/// Private, safe substitution point shared by scripted tests and the Linux adapter.
trait DebugTrapBackendV1 {
    fn enable(
        &mut self,
        target_pid: u32,
        exceptions: KfdDebugExceptionMaskV1,
    ) -> Result<KfdRuntimeInfoV1, &'static str>;
    fn disable(&mut self, target_pid: u32) -> Result<(), &'static str>;
    fn send_runtime_event(
        &mut self,
        target_pid: u32,
        exceptions: KfdDebugExceptionMaskV1,
        gpu_id: u32,
        queue_id: u32,
    ) -> Result<(), &'static str>;
    fn set_exceptions(
        &mut self,
        target_pid: u32,
        exceptions: KfdDebugExceptionMaskV1,
    ) -> Result<(), &'static str>;
    fn queue_snapshot(
        &mut self,
        target_pid: u32,
        capacity: usize,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Vec<KfdDebugQueueSnapshotEntryV1>, &'static str>;
    fn device_snapshot(
        &mut self,
        target_pid: u32,
        capacity: usize,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Vec<KfdDebugDeviceSnapshotEntryV1>, &'static str>;
    fn query_event(
        &mut self,
        target_pid: u32,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Option<(u64, u32, u32)>, &'static str>;
    fn query_exception_info(
        &mut self,
        target_pid: u32,
        source_id: u32,
        code: KfdDebugTrapExceptionCodeV1,
        clear: bool,
    ) -> Result<BackendExceptionInfoV1, &'static str>;
    fn suspend_queues(
        &mut self,
        target_pid: u32,
        queues: &mut [u32],
        exceptions_to_clear: KfdDebugExceptionMaskV1,
        grace_period: u32,
    ) -> Result<usize, &'static str>;
    fn resume_queues(&mut self, target_pid: u32, queues: &mut [u32])
    -> Result<usize, &'static str>;
    fn set_launch_mode(
        &mut self,
        target_pid: u32,
        mode: KfdDebugTrapWaveLaunchModeV1,
    ) -> Result<(), &'static str>;
    fn set_launch_override(
        &mut self,
        target_pid: u32,
        mode: KfdDebugTrapOverrideModeV1,
        enabled: KfdDebugLaunchOverrideMaskV1,
        support_requested: KfdDebugLaunchOverrideMaskV1,
    ) -> Result<(KfdDebugLaunchOverrideMaskV1, KfdDebugLaunchOverrideMaskV1), &'static str>;
    fn set_flags(
        &mut self,
        target_pid: u32,
        flags: KfdDebugTrapFlagsV1,
    ) -> Result<KfdDebugTrapFlagsV1, &'static str>;
    fn set_address_watch(
        &mut self,
        target_pid: u32,
        gpu_id: u32,
        address: u64,
        mask: u32,
        mode: KfdDebugTrapAddressWatchModeV1,
    ) -> Result<u32, &'static str>;
    fn clear_address_watch(
        &mut self,
        target_pid: u32,
        gpu_id: u32,
        watch_id: u32,
    ) -> Result<(), &'static str>;
}

/// Narrow live ioctl implementation. Construction remains private so the
/// public owner must prove ptrace ownership, bind a pidfd, and retain both
/// endpoints of the nonblocking notifier before enabling the session.
#[allow(unsafe_code)]
mod linux_backend {
    use std::os::fd::{AsRawFd, OwnedFd};

    use fe2o3_kfd_uapi::{AMDKFD_IOC_DBG_TRAP, KfdIoctlDebugTrapArgsV1};
    use rustix::ioctl::{Opcode, Updater};

    use super::*;

    const DBG_TRAP_OPCODE: Opcode = AMDKFD_IOC_DBG_TRAP as Opcode;

    pub(super) struct LinuxDebugTrapBackendV1 {
        kfd: OwnedFd,
        notifier_write: OwnedFd,
    }

    impl LinuxDebugTrapBackendV1 {
        pub(super) fn from_owned_descriptors(kfd: OwnedFd, notifier_write: OwnedFd) -> Self {
            Self {
                kfd,
                notifier_write,
            }
        }

        fn update(
            &self,
            operation: &'static str,
            args: &mut KfdIoctlDebugTrapArgsV1,
        ) -> Result<(), &'static str> {
            // SAFETY: the request number and 32-byte in/out layout are pinned
            // by fe2o3-kfd-uapi. Every nested pointer the kernel may
            // dereference is backed by calling-method storage that remains live
            // and exclusively borrowed through this synchronous ioctl. The
            // metadata-only snapshot sentinel is justified at its construction.
            let request = unsafe { Updater::<DBG_TRAP_OPCODE, _>::new(args) };
            // SAFETY: the descriptor and request lifetime/layout contract is
            // established above. Kernel output is still validated separately.
            unsafe { rustix::ioctl::ioctl(&self.kfd, request) }.map_err(|_| operation)?;
            Ok(())
        }

        fn snapshot_count<T: Default + Clone>(
            &self,
            target_pid: u32,
            capacity: usize,
            exceptions_to_clear: KfdDebugExceptionMaskV1,
            operation: &'static str,
            constructor: impl Fn(u32, KfdDebugExceptionMaskV1, u64, u32) -> KfdIoctlDebugTrapArgsV1,
            expected_entry_size: u32,
        ) -> Result<Vec<T>, &'static str> {
            // The pinned KFD 1.18 queue/device snapshot implementations in
            // kfd_debug.c require a non-null `user_info` even for a count
            // probe, then return before any copy when the input count is zero.
            // This aligned dangling value is only a non-null integer sentinel;
            // the kernel must not and, in the reviewed source, does not
            // dereference it.
            let probe = core::ptr::NonNull::<T>::dangling().as_ptr() as usize as u64;
            let mut args = constructor(target_pid, exceptions_to_clear, probe, 0);
            self.update(operation, &mut args)?;
            let count = usize::try_from(args.returned_snapshot_count()).map_err(|_| operation)?;
            if count > capacity || args.returned_snapshot_entry_size() != expected_entry_size {
                return Err(operation);
            }
            if count == 0 {
                return Ok(Vec::new());
            }
            let mut entries = vec![T::default(); count];
            let mut args = constructor(
                target_pid,
                exceptions_to_clear,
                entries.as_mut_ptr() as usize as u64,
                count as u32,
            );
            self.update(operation, &mut args)?;
            if usize::try_from(args.returned_snapshot_count()).map_err(|_| operation)? != count
                || args.returned_snapshot_entry_size() != expected_entry_size
            {
                return Err(operation);
            }
            Ok(entries)
        }
    }

    impl DebugTrapBackendV1 for LinuxDebugTrapBackendV1 {
        fn enable(
            &mut self,
            target_pid: u32,
            exceptions: KfdDebugExceptionMaskV1,
        ) -> Result<KfdRuntimeInfoV1, &'static str> {
            let mut runtime = KfdRuntimeInfoV1::default();
            let notifier_fd = u32::try_from(self.notifier_write.as_raw_fd())
                .map_err(|_| "DBG_TRAP_ENABLE notifier fd")?;
            let mut args = KfdIoctlDebugTrapArgsV1::enable(
                target_pid,
                exceptions,
                (&mut runtime as *mut KfdRuntimeInfoV1) as usize as u64,
                core::mem::size_of::<KfdRuntimeInfoV1>() as u32,
                notifier_fd,
            );
            self.update("DBG_TRAP_ENABLE", &mut args)?;
            if args.enable_runtime_info_size() != core::mem::size_of::<KfdRuntimeInfoV1>() as u32 {
                return Err("DBG_TRAP_ENABLE runtime info size");
            }
            Ok(runtime)
        }

        fn disable(&mut self, target_pid: u32) -> Result<(), &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::disable(target_pid);
            self.update("DBG_TRAP_DISABLE", &mut args)
        }

        fn send_runtime_event(
            &mut self,
            target_pid: u32,
            exceptions: KfdDebugExceptionMaskV1,
            gpu_id: u32,
            queue_id: u32,
        ) -> Result<(), &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::send_runtime_event(
                target_pid, exceptions, gpu_id, queue_id,
            );
            self.update("DBG_TRAP_SEND_RUNTIME_EVENT", &mut args)
        }

        fn set_exceptions(
            &mut self,
            target_pid: u32,
            exceptions: KfdDebugExceptionMaskV1,
        ) -> Result<(), &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::set_exceptions(target_pid, exceptions);
            self.update("DBG_TRAP_SET_EXCEPTIONS_ENABLED", &mut args)
        }

        fn queue_snapshot(
            &mut self,
            target_pid: u32,
            capacity: usize,
            exceptions_to_clear: KfdDebugExceptionMaskV1,
        ) -> Result<Vec<KfdDebugQueueSnapshotEntryV1>, &'static str> {
            self.snapshot_count(
                target_pid,
                capacity,
                exceptions_to_clear,
                "DBG_TRAP_GET_QUEUE_SNAPSHOT",
                KfdIoctlDebugTrapArgsV1::queue_snapshot,
                core::mem::size_of::<KfdDebugQueueSnapshotEntryV1>() as u32,
            )
        }

        fn device_snapshot(
            &mut self,
            target_pid: u32,
            capacity: usize,
            exceptions_to_clear: KfdDebugExceptionMaskV1,
        ) -> Result<Vec<KfdDebugDeviceSnapshotEntryV1>, &'static str> {
            self.snapshot_count(
                target_pid,
                capacity,
                exceptions_to_clear,
                "DBG_TRAP_GET_DEVICE_SNAPSHOT",
                KfdIoctlDebugTrapArgsV1::device_snapshot,
                core::mem::size_of::<KfdDebugDeviceSnapshotEntryV1>() as u32,
            )
        }

        fn query_event(
            &mut self,
            target_pid: u32,
            exceptions_to_clear: KfdDebugExceptionMaskV1,
        ) -> Result<Option<(u64, u32, u32)>, &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::query_event(target_pid, exceptions_to_clear);
            let request = unsafe { Updater::<DBG_TRAP_OPCODE, _>::new(&mut args) };
            match unsafe { rustix::ioctl::ioctl(&self.kfd, request) } {
                Ok(_) => {
                    let mask = args
                        .returned_event_mask()
                        .ok_or("DBG_TRAP_QUERY_DEBUG_EVENT mask")?;
                    Ok(Some((
                        mask.bits(),
                        args.returned_event_gpu_id(),
                        args.returned_event_queue_id(),
                    )))
                }
                Err(rustix::io::Errno::AGAIN) => Ok(None),
                Err(_) => Err("DBG_TRAP_QUERY_DEBUG_EVENT"),
            }
        }

        fn query_exception_info(
            &mut self,
            target_pid: u32,
            source_id: u32,
            code: KfdDebugTrapExceptionCodeV1,
            clear: bool,
        ) -> Result<BackendExceptionInfoV1, &'static str> {
            match code {
                KfdDebugTrapExceptionCodeV1::ProcessRuntime => {
                    let mut info = KfdRuntimeInfoV1::default();
                    let mut args = KfdIoctlDebugTrapArgsV1::query_exception_info(
                        target_pid,
                        (&mut info as *mut KfdRuntimeInfoV1) as usize as u64,
                        core::mem::size_of::<KfdRuntimeInfoV1>() as u32,
                        source_id,
                        code,
                        clear,
                    );
                    self.update("DBG_TRAP_QUERY_EXCEPTION_INFO(runtime)", &mut args)?;
                    if args.returned_info_size() != core::mem::size_of::<KfdRuntimeInfoV1>() as u32
                    {
                        return Err("DBG_TRAP_QUERY_EXCEPTION_INFO runtime size");
                    }
                    Ok(BackendExceptionInfoV1::Runtime(info))
                }
                KfdDebugTrapExceptionCodeV1::DeviceMemoryViolation => {
                    let mut info = KfdHsaMemoryExceptionDataV1::from_untrusted_wire(
                        fe2o3_kfd_uapi::KfdMemoryExceptionFailureV1::default(),
                        0,
                        0,
                        0,
                    );
                    let mut args = KfdIoctlDebugTrapArgsV1::query_exception_info(
                        target_pid,
                        (&mut info as *mut KfdHsaMemoryExceptionDataV1) as usize as u64,
                        core::mem::size_of::<KfdHsaMemoryExceptionDataV1>() as u32,
                        source_id,
                        code,
                        clear,
                    );
                    self.update("DBG_TRAP_QUERY_EXCEPTION_INFO(memory)", &mut args)?;
                    if args.returned_info_size()
                        != core::mem::size_of::<KfdHsaMemoryExceptionDataV1>() as u32
                    {
                        return Err("DBG_TRAP_QUERY_EXCEPTION_INFO memory size");
                    }
                    Ok(BackendExceptionInfoV1::DeviceMemory(info))
                }
                _ => {
                    let mut byte = 0_u8;
                    let mut args = KfdIoctlDebugTrapArgsV1::query_exception_info(
                        target_pid,
                        (&mut byte as *mut u8) as usize as u64,
                        0,
                        source_id,
                        code,
                        clear,
                    );
                    self.update("DBG_TRAP_QUERY_EXCEPTION_INFO", &mut args)?;
                    if args.returned_info_size() != 0 {
                        return Err("DBG_TRAP_QUERY_EXCEPTION_INFO unexpected payload");
                    }
                    Ok(BackendExceptionInfoV1::None)
                }
            }
        }

        fn suspend_queues(
            &mut self,
            target_pid: u32,
            queues: &mut [u32],
            exceptions_to_clear: KfdDebugExceptionMaskV1,
            grace_period: u32,
        ) -> Result<usize, &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::suspend_queues(
                target_pid,
                exceptions_to_clear,
                queues.as_mut_ptr() as usize as u64,
                queues.len() as u32,
                grace_period,
            );
            self.update("DBG_TRAP_SUSPEND_QUEUES", &mut args)?;
            // The positive ioctl return count is not available through
            // rustix::ioctl::Updater, so count status words after mutation.
            Ok(queues
                .iter()
                .filter(|queue| {
                    **queue & (KFD_DBG_QUEUE_ERROR_MASK_V1 | KFD_DBG_QUEUE_INVALID_MASK_V1) == 0
                })
                .count())
        }

        fn resume_queues(
            &mut self,
            target_pid: u32,
            queues: &mut [u32],
        ) -> Result<usize, &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::resume_queues(
                target_pid,
                queues.as_mut_ptr() as usize as u64,
                queues.len() as u32,
            );
            self.update("DBG_TRAP_RESUME_QUEUES", &mut args)?;
            Ok(queues
                .iter()
                .filter(|queue| {
                    **queue & (KFD_DBG_QUEUE_ERROR_MASK_V1 | KFD_DBG_QUEUE_INVALID_MASK_V1) == 0
                })
                .count())
        }

        fn set_launch_mode(
            &mut self,
            target_pid: u32,
            mode: KfdDebugTrapWaveLaunchModeV1,
        ) -> Result<(), &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::launch_mode(target_pid, mode);
            self.update("DBG_TRAP_SET_WAVE_LAUNCH_MODE", &mut args)
        }

        fn set_launch_override(
            &mut self,
            target_pid: u32,
            mode: KfdDebugTrapOverrideModeV1,
            enabled: KfdDebugLaunchOverrideMaskV1,
            support_requested: KfdDebugLaunchOverrideMaskV1,
        ) -> Result<(KfdDebugLaunchOverrideMaskV1, KfdDebugLaunchOverrideMaskV1), &'static str>
        {
            let mut args = KfdIoctlDebugTrapArgsV1::launch_override(
                target_pid,
                mode,
                enabled,
                support_requested,
            );
            self.update("DBG_TRAP_SET_WAVE_LAUNCH_OVERRIDE", &mut args)?;
            Ok((
                args.returned_launch_override(),
                args.returned_launch_support(),
            ))
        }

        fn set_flags(
            &mut self,
            target_pid: u32,
            flags: KfdDebugTrapFlagsV1,
        ) -> Result<KfdDebugTrapFlagsV1, &'static str> {
            let mut args = KfdIoctlDebugTrapArgsV1::set_flags(target_pid, flags);
            self.update("DBG_TRAP_SET_FLAGS", &mut args)?;
            Ok(args.returned_flags())
        }

        fn set_address_watch(
            &mut self,
            target_pid: u32,
            gpu_id: u32,
            address: u64,
            mask: u32,
            mode: KfdDebugTrapAddressWatchModeV1,
        ) -> Result<u32, &'static str> {
            let mut args =
                KfdIoctlDebugTrapArgsV1::set_address_watch(target_pid, address, mode, mask, gpu_id);
            self.update("DBG_TRAP_SET_NODE_ADDRESS_WATCH", &mut args)?;
            Ok(args.returned_watch_id())
        }

        fn clear_address_watch(
            &mut self,
            target_pid: u32,
            gpu_id: u32,
            watch_id: u32,
        ) -> Result<(), &'static str> {
            let mut args =
                KfdIoctlDebugTrapArgsV1::clear_address_watch(target_pid, gpu_id, watch_id);
            self.update("DBG_TRAP_CLEAR_NODE_ADDRESS_WATCH", &mut args)
        }
    }
}

type LinuxDebugTrapSessionEngineV1 =
    DebugTrapSessionEngineV1<linux_backend::LinuxDebugTrapBackendV1>;

/// Target-side failure for the process-global KFD runtime-debug transition.
#[derive(Debug)]
pub enum KfdTargetRuntimeDebugErrorV1 {
    Kfd(crate::KfdAdapterError),
    RuntimeTransition(String),
    AlreadyFinished,
}

impl fmt::Display for KfdTargetRuntimeDebugErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Kfd(source) => write!(formatter, "KFD admission failed: {source}"),
            Self::RuntimeTransition(source) => {
                write!(formatter, "KFD runtime-debug transition failed: {source}")
            }
            Self::AlreadyFinished => formatter.write_str("KFD runtime-debug token is finished"),
        }
    }
}

impl std::error::Error for KfdTargetRuntimeDebugErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Kfd(source) => Some(source),
            _ => None,
        }
    }
}

impl From<crate::KfdAdapterError> for KfdTargetRuntimeDebugErrorV1 {
    fn from(error: crate::KfdAdapterError) -> Self {
        Self::Kfd(error)
    }
}

impl From<crate::queue_linux::LinuxDoorbellErrorV1> for KfdTargetRuntimeDebugErrorV1 {
    fn from(error: crate::queue_linux::LinuxDoorbellErrorV1) -> Self {
        Self::RuntimeTransition(error.to_string())
    }
}

/// Current-process target authority for the reviewed KFD `RUNTIME_ENABLE`
/// mode-1 transition (`r_debug=0`, no TTMP or capability claims).
///
/// This token is independent from [`KfdLiveDebugSessionV1`], which must live in
/// the ptrace-parent debugger process. When a debugger is already enabled, both
/// enable and disable wait for its `PROCESS_RUNTIME` acknowledgement. No user
/// queue may exist before enable, and no queue may be created through this
/// target-only token.
#[must_use = "explicit finish reports the target runtime-disable result"]
pub struct KfdTargetRuntimeDebugTokenV1 {
    runtime: Option<crate::queue_linux::LinuxKfdRuntimeEnabledV1>,
    kfd: crate::KfdWithAdmittedUapi,
    opener_pid: u32,
}

impl fmt::Debug for KfdTargetRuntimeDebugTokenV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdTargetRuntimeDebugTokenV1")
            .field("active", &self.runtime.is_some())
            .field("opener_pid", &self.opener_pid)
            .finish_non_exhaustive()
    }
}

impl KfdTargetRuntimeDebugTokenV1 {
    pub fn enable_current_process() -> Result<Self, KfdTargetRuntimeDebugErrorV1> {
        let kfd = crate::OpenedKfd::open_default()?.admit_uapi()?;
        let opener_pid = std::process::id();
        let runtime = crate::queue_linux::LinuxKfdRuntimeEnabledV1::enable(
            kfd.opened.fd.as_fd(),
            opener_pid,
        )?;
        Ok(Self {
            runtime: Some(runtime),
            kfd,
            opener_pid,
        })
    }

    pub const fn is_active(&self) -> bool {
        self.runtime.is_some()
    }

    fn disable(&mut self) -> Result<(), KfdTargetRuntimeDebugErrorV1> {
        let runtime = self
            .runtime
            .take()
            .ok_or(KfdTargetRuntimeDebugErrorV1::AlreadyFinished)?;
        let disabled = runtime.disable_debug_target(self.kfd.opened.fd.as_fd(), self.opener_pid)?;
        disabled.complete();
        Ok(())
    }

    pub fn finish(mut self) -> Result<(), KfdTargetRuntimeDebugErrorV1> {
        self.disable()
    }
}

impl Drop for KfdTargetRuntimeDebugTokenV1 {
    fn drop(&mut self) {
        if self.runtime.is_some() {
            let _ = self.disable();
        }
    }
}

/// Owned debugger-side KFD session for one ptrace-owned process.
///
/// Construction is the only `DBG_TRAP_ENABLE` entry point. It rejects a
/// non-leader target, a dead target, absent ptrace-parent ownership, pidfd
/// identity drift, an unreviewed KFD UAPI, or notifier setup failure before
/// issuing the enabling ioctl. The type deliberately has no descriptor or raw
/// address accessor and is neither `Clone`, `Send`, nor `Sync`. KFD compares
/// the ptrace-parent task to the exact calling task, not only its process ID.
#[must_use = "explicit finish reports debug-trap cleanup failures"]
pub struct KfdLiveDebugSessionV1 {
    engine: LinuxDebugTrapSessionEngineV1,
    pidfd: OwnedFd,
    notifier_read: OwnedFd,
    thread_bound: PhantomData<Rc<()>>,
}

impl fmt::Debug for KfdLiveDebugSessionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdLiveDebugSessionV1")
            .field("target_pid", &self.target_pid())
            .field("runtime", &self.runtime_observation())
            .finish_non_exhaustive()
    }
}

impl KfdLiveDebugSessionV1 {
    pub fn attach(plan: KfdDebugSessionPlanV1) -> Result<Self, KfdLiveDebugSessionErrorV1> {
        use rustix::pipe::{PipeFlags, pipe_with};
        use rustix::process::{Pid, PidfdFlags, pidfd_open};

        let target_pid = plan.target_pid();
        admit_ptrace_owner(target_pid)?;
        let pid =
            Pid::from_raw(target_pid as i32).ok_or(KfdLiveDebugSessionErrorV1::TargetNotLive)?;
        let pidfd =
            pidfd_open(pid, PidfdFlags::empty()).map_err(KfdLiveDebugSessionErrorV1::PidfdOpen)?;
        if inspect_pidfd_target(&pidfd)? != Some(target_pid) {
            return Err(KfdLiveDebugSessionErrorV1::PidfdIdentityChanged);
        }
        admit_ptrace_owner(target_pid)?;

        let admitted = crate::OpenedKfd::open_default()?.admit_uapi()?;
        let (notifier_read, notifier_write) =
            pipe_with(PipeFlags::CLOEXEC | PipeFlags::NONBLOCK)
                .map_err(KfdLiveDebugSessionErrorV1::NotificationPipe)?;

        // Final no-mutation sandwich check after every fallible prerequisite
        // has been acquired and immediately before DBG_TRAP_ENABLE.
        if inspect_pidfd_target(&pidfd)? != Some(target_pid) {
            return Err(KfdLiveDebugSessionErrorV1::PidfdIdentityChanged);
        }
        admit_ptrace_owner(target_pid)?;

        let backend = linux_backend::LinuxDebugTrapBackendV1::from_owned_descriptors(
            admitted.opened.fd,
            notifier_write,
        );
        let engine = DebugTrapSessionEngineV1::enable(backend, plan)?;
        Ok(Self {
            engine,
            pidfd,
            notifier_read,
            thread_bound: PhantomData,
        })
    }

    pub fn target_pid(&self) -> u32 {
        self.engine.plan.target_pid()
    }

    pub const fn runtime_observation(&self) -> KfdDebugRuntimeObservationV1 {
        self.engine.runtime
    }

    pub const fn enabled_exceptions(&self) -> KfdDebugExceptionMaskV1 {
        self.engine.exceptions
    }

    fn preflight(&self) -> Result<(), KfdLiveDebugSessionErrorV1> {
        let target_pid = self.target_pid();
        if target_pid == 0 || inspect_pidfd_target(&self.pidfd)? != Some(target_pid) {
            return Err(KfdLiveDebugSessionErrorV1::PidfdIdentityChanged);
        }
        admit_ptrace_owner(target_pid)
    }

    fn engine_mut(
        &mut self,
    ) -> Result<&mut LinuxDebugTrapSessionEngineV1, KfdLiveDebugSessionErrorV1> {
        self.preflight()?;
        Ok(&mut self.engine)
    }

    /// Drains up to `limit` one-byte KFD wakeup hints from the nonblocking
    /// notifier. Event state remains authoritative and must be queried.
    pub fn drain_notifications(
        &mut self,
        limit: usize,
    ) -> Result<usize, KfdLiveDebugSessionErrorV1> {
        if limit == 0 || limit > MAX_NOTIFICATION_DRAIN {
            return Err(KfdLiveDebugSessionErrorV1::NotificationLimit);
        }
        self.preflight()?;
        let mut total = 0;
        let mut bytes = [0_u8; 256];
        while total < limit {
            let capacity = (limit - total).min(bytes.len());
            match rustix::io::read(&self.notifier_read, &mut bytes[..capacity]) {
                Ok(0) => return Err(KfdLiveDebugSessionErrorV1::NotificationClosed),
                Ok(count) => {
                    if bytes[..count].iter().any(|byte| *byte != b'.') {
                        return Err(KfdLiveDebugSessionErrorV1::InvalidNotification);
                    }
                    total += count;
                }
                Err(rustix::io::Errno::AGAIN) => break,
                Err(rustix::io::Errno::INTR) => continue,
                Err(source) => {
                    return Err(KfdLiveDebugSessionErrorV1::NotificationRead(source));
                }
            }
        }
        Ok(total)
    }

    pub fn set_exceptions(
        &mut self,
        exceptions: KfdDebugExceptionMaskV1,
    ) -> Result<(), KfdLiveDebugSessionErrorV1> {
        self.engine_mut()?.set_exceptions(exceptions)?;
        Ok(())
    }

    /// Acknowledges the process-runtime event that blocks a target-side
    /// `AMDKFD_IOC_RUNTIME_ENABLE` transition while debugging is active.
    pub fn acknowledge_runtime_transition(
        &mut self,
        event: KfdDebugEventObservationV1,
    ) -> Result<(), KfdLiveDebugSessionErrorV1> {
        if !event
            .exceptions()
            .contains(KfdDebugTrapExceptionCodeV1::ProcessRuntime)
        {
            return Err(KfdDebugSessionErrorV1::RuntimeEventRequired.into());
        }
        self.engine_mut()?
            .acknowledge_runtime_transition(event.gpu_id(), event.queue_id())?;
        Ok(())
    }

    pub fn queue_snapshot(
        &mut self,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Vec<KfdDebugQueueObservationV1>, KfdLiveDebugSessionErrorV1> {
        Ok(self.engine_mut()?.queue_snapshot(exceptions_to_clear)?)
    }

    pub fn device_snapshot(
        &mut self,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Vec<KfdDebugDeviceObservationV1>, KfdLiveDebugSessionErrorV1> {
        Ok(self.engine_mut()?.device_snapshot(exceptions_to_clear)?)
    }

    pub fn query_event(
        &mut self,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Option<KfdDebugEventObservationV1>, KfdLiveDebugSessionErrorV1> {
        Ok(self.engine_mut()?.query_event(exceptions_to_clear)?)
    }

    pub fn query_exception_info(
        &mut self,
        source_id: u32,
        code: KfdDebugTrapExceptionCodeV1,
        clear: bool,
    ) -> Result<KfdDebugExceptionInfoV1, KfdLiveDebugSessionErrorV1> {
        Ok(self
            .engine_mut()?
            .query_exception_info(source_id, code, clear)?)
    }

    pub fn suspend_queues(
        &mut self,
        queues: &[u32],
        exceptions_to_clear: KfdDebugExceptionMaskV1,
        grace_period: u32,
    ) -> Result<Vec<KfdDebugQueueOperationObservationV1>, KfdLiveDebugSessionErrorV1> {
        Ok(self
            .engine_mut()?
            .suspend_queues(queues, exceptions_to_clear, grace_period)?)
    }

    pub fn resume_queues(
        &mut self,
        queues: &[u32],
    ) -> Result<Vec<KfdDebugQueueOperationObservationV1>, KfdLiveDebugSessionErrorV1> {
        Ok(self.engine_mut()?.resume_queues(queues)?)
    }

    pub fn set_launch_mode(
        &mut self,
        mode: KfdDebugTrapWaveLaunchModeV1,
    ) -> Result<(), KfdLiveDebugSessionErrorV1> {
        self.engine_mut()?.set_launch_mode(mode)?;
        Ok(())
    }

    pub fn set_launch_override(
        &mut self,
        mode: KfdDebugTrapOverrideModeV1,
        enabled: KfdDebugLaunchOverrideMaskV1,
        support_requested: KfdDebugLaunchOverrideMaskV1,
    ) -> Result<KfdDebugLaunchOverrideMaskV1, KfdLiveDebugSessionErrorV1> {
        Ok(self
            .engine_mut()?
            .set_launch_override(mode, enabled, support_requested)?)
    }

    pub fn set_flags(
        &mut self,
        flags: KfdDebugTrapFlagsV1,
    ) -> Result<(), KfdLiveDebugSessionErrorV1> {
        self.engine_mut()?.set_flags(flags)?;
        Ok(())
    }

    pub fn set_address_watch(
        &mut self,
        gpu_id: u32,
        address: u64,
        mask: u32,
        mode: KfdDebugTrapAddressWatchModeV1,
    ) -> Result<KfdDebugAddressWatchObservationV1, KfdLiveDebugSessionErrorV1> {
        Ok(self
            .engine_mut()?
            .set_address_watch(gpu_id, address, mask, mode)?)
    }

    pub fn clear_address_watch(
        &mut self,
        watch: KfdDebugAddressWatchObservationV1,
    ) -> Result<(), KfdLiveDebugSessionErrorV1> {
        self.engine_mut()?.clear_address_watch(watch)?;
        Ok(())
    }

    /// Restores every owned setting and disables debug-trap. Cleanup is
    /// attempted even if ptrace ownership has been lost since admission.
    pub fn finish(self) -> Result<(), KfdLiveDebugSessionErrorV1> {
        self.engine.finish().map_err(Into::into)
    }
}

struct DebugTrapSessionEngineV1<B: DebugTrapBackendV1> {
    backend: B,
    plan: KfdDebugSessionPlanV1,
    runtime: KfdDebugRuntimeObservationV1,
    exceptions: KfdDebugExceptionMaskV1,
    suspended: Vec<u32>,
    watches: Vec<KfdDebugAddressWatchObservationV1>,
    launch_mode_changed: bool,
    original_override: Option<KfdDebugLaunchOverrideMaskV1>,
    original_flags: Option<KfdDebugTrapFlagsV1>,
    finished: bool,
}

impl<B: DebugTrapBackendV1> DebugTrapSessionEngineV1<B> {
    fn enable(mut backend: B, plan: KfdDebugSessionPlanV1) -> Result<Self, KfdDebugSessionErrorV1> {
        let raw = backend
            .enable(plan.target_pid(), plan.exceptions())
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        let runtime = match admit_runtime(raw) {
            Ok(runtime) => runtime,
            Err(error) => {
                let _ = backend.disable(plan.target_pid());
                return Err(error);
            }
        };
        Ok(Self {
            backend,
            plan,
            runtime,
            exceptions: plan.exceptions(),
            suspended: Vec::new(),
            watches: Vec::new(),
            launch_mode_changed: false,
            original_override: None,
            original_flags: None,
            finished: false,
        })
    }

    fn ensure_active(&self) -> Result<(), KfdDebugSessionErrorV1> {
        if self.finished {
            Err(KfdDebugSessionErrorV1::SessionFinished)
        } else {
            Ok(())
        }
    }

    fn ensure_hardware(&self) -> Result<(), KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        if self.runtime.hardware_operations_available() {
            Ok(())
        } else {
            Err(KfdDebugSessionErrorV1::RuntimeNotEnabled)
        }
    }

    fn set_exceptions(
        &mut self,
        exceptions: KfdDebugExceptionMaskV1,
    ) -> Result<(), KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        self.backend
            .set_exceptions(self.plan.target_pid(), exceptions)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        self.exceptions = exceptions;
        Ok(())
    }

    fn acknowledge_runtime_transition(
        &mut self,
        gpu_id: u32,
        queue_id: u32,
    ) -> Result<(), KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        self.backend
            .send_runtime_event(
                self.plan.target_pid(),
                KfdDebugExceptionMaskV1::from_code(KfdDebugTrapExceptionCodeV1::ProcessRuntime),
                gpu_id,
                queue_id,
            )
            .map_err(KfdDebugSessionErrorV1::Backend)
    }

    fn queue_snapshot(
        &mut self,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Vec<KfdDebugQueueObservationV1>, KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        let entries = self
            .backend
            .queue_snapshot(
                self.plan.target_pid(),
                self.plan.snapshot_limit(),
                exceptions_to_clear,
            )
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        if entries.len() > self.plan.snapshot_limit() {
            return Err(KfdDebugSessionErrorV1::SnapshotCount);
        }
        entries.into_iter().map(admit_queue_snapshot).collect()
    }

    fn device_snapshot(
        &mut self,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Vec<KfdDebugDeviceObservationV1>, KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        let entries = self
            .backend
            .device_snapshot(
                self.plan.target_pid(),
                self.plan.snapshot_limit(),
                exceptions_to_clear,
            )
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        if entries.len() > self.plan.snapshot_limit() {
            return Err(KfdDebugSessionErrorV1::SnapshotCount);
        }
        entries.into_iter().map(admit_device_snapshot).collect()
    }

    fn query_event(
        &mut self,
        exceptions_to_clear: KfdDebugExceptionMaskV1,
    ) -> Result<Option<KfdDebugEventObservationV1>, KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        let Some((raw, gpu_id, queue_id)) = self
            .backend
            .query_event(self.plan.target_pid(), exceptions_to_clear)
            .map_err(KfdDebugSessionErrorV1::Backend)?
        else {
            return Ok(None);
        };
        let exceptions = KfdDebugExceptionMaskV1::new(raw)
            .ok_or(KfdDebugSessionErrorV1::InvalidExceptionMask)?;
        if exceptions.bits() == 0 {
            return Err(KfdDebugSessionErrorV1::InvalidExceptionMask);
        }
        Ok(Some(KfdDebugEventObservationV1 {
            exceptions,
            gpu_id,
            queue_id,
        }))
    }

    fn query_exception_info(
        &mut self,
        source_id: u32,
        code: KfdDebugTrapExceptionCodeV1,
        clear: bool,
    ) -> Result<KfdDebugExceptionInfoV1, KfdDebugSessionErrorV1> {
        self.ensure_active()?;
        let info = self
            .backend
            .query_exception_info(self.plan.target_pid(), source_id, code, clear)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        match (code, info) {
            (KfdDebugTrapExceptionCodeV1::ProcessRuntime, BackendExceptionInfoV1::Runtime(raw)) => {
                let runtime = admit_runtime(raw)?;
                self.runtime = runtime;
                if runtime.state() == KfdDebugRuntimeStateV1::Disabled {
                    // runtime_disable holds the target process mutex while KFD
                    // deactivates trap hardware: all queues are resumed, all
                    // watchpoints cleared, and launch mode/flags normalized
                    // before this observation can be returned.
                    self.suspended.clear();
                    self.watches.clear();
                    self.launch_mode_changed = false;
                    self.original_override = None;
                    self.original_flags = None;
                }
                Ok(KfdDebugExceptionInfoV1::Runtime(runtime))
            }
            (
                KfdDebugTrapExceptionCodeV1::DeviceMemoryViolation,
                BackendExceptionInfoV1::DeviceMemory(raw),
            ) => Ok(KfdDebugExceptionInfoV1::DeviceMemoryViolation {
                failure_words: raw.failure().words(),
                fault_address_present: raw.va() != 0,
                gpu_id: raw.gpu_id(),
                error_type: raw.error_type(),
            }),
            (KfdDebugTrapExceptionCodeV1::ProcessRuntime, _)
            | (KfdDebugTrapExceptionCodeV1::DeviceMemoryViolation, _) => {
                Err(KfdDebugSessionErrorV1::ExceptionInfoKind)
            }
            (_, BackendExceptionInfoV1::None) => Ok(KfdDebugExceptionInfoV1::NoPayload),
            _ => Err(KfdDebugSessionErrorV1::ExceptionInfoKind),
        }
    }

    fn suspend_queues(
        &mut self,
        queues: &[u32],
        exceptions_to_clear: KfdDebugExceptionMaskV1,
        grace_period: u32,
    ) -> Result<Vec<KfdDebugQueueOperationObservationV1>, KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        validate_queue_list(queues, self.plan.snapshot_limit())?;
        let original = queues.to_vec();
        let mut wire = original.clone();
        let completed = self
            .backend
            .suspend_queues(
                self.plan.target_pid(),
                &mut wire,
                exceptions_to_clear,
                grace_period,
            )
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        let observations = decode_queue_results(&original, &wire, completed)?;
        for observation in &observations {
            if observation.state == KfdDebugQueueOperationStateV1::Complete
                && !self.suspended.contains(&observation.queue_id)
            {
                self.suspended.push(observation.queue_id);
            }
        }
        Ok(observations)
    }

    fn resume_queues(
        &mut self,
        queues: &[u32],
    ) -> Result<Vec<KfdDebugQueueOperationObservationV1>, KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        validate_queue_list(queues, self.plan.snapshot_limit())?;
        if queues.iter().any(|queue| !self.suspended.contains(queue)) {
            return Err(KfdDebugSessionErrorV1::QueueResult);
        }
        let original = queues.to_vec();
        let mut wire = original.clone();
        let completed = self
            .backend
            .resume_queues(self.plan.target_pid(), &mut wire)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        let observations = decode_queue_results(&original, &wire, completed)?;
        for observation in &observations {
            if observation.state == KfdDebugQueueOperationStateV1::Complete {
                self.suspended
                    .retain(|queue| *queue != observation.queue_id);
            }
        }
        Ok(observations)
    }

    fn set_launch_mode(
        &mut self,
        mode: KfdDebugTrapWaveLaunchModeV1,
    ) -> Result<(), KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        self.backend
            .set_launch_mode(self.plan.target_pid(), mode)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        self.launch_mode_changed = mode != KfdDebugTrapWaveLaunchModeV1::Normal;
        Ok(())
    }

    fn set_launch_override(
        &mut self,
        mode: KfdDebugTrapOverrideModeV1,
        enabled: KfdDebugLaunchOverrideMaskV1,
        support_requested: KfdDebugLaunchOverrideMaskV1,
    ) -> Result<KfdDebugLaunchOverrideMaskV1, KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        let (previous, supported) = self
            .backend
            .set_launch_override(self.plan.target_pid(), mode, enabled, support_requested)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        if enabled.bits() & !supported.bits() != 0 {
            return Err(KfdDebugSessionErrorV1::Backend(
                "launch override output excludes enabled bits",
            ));
        }
        if self.original_override.is_none() {
            self.original_override = Some(previous);
        }
        Ok(supported)
    }

    fn set_flags(&mut self, flags: KfdDebugTrapFlagsV1) -> Result<(), KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        let previous = self
            .backend
            .set_flags(self.plan.target_pid(), flags)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        if self.original_flags.is_none() {
            self.original_flags = Some(previous);
        }
        Ok(())
    }

    fn set_address_watch(
        &mut self,
        gpu_id: u32,
        address: u64,
        mask: u32,
        mode: KfdDebugTrapAddressWatchModeV1,
    ) -> Result<KfdDebugAddressWatchObservationV1, KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        let max = self
            .plan
            .snapshot_limit()
            .checked_mul(KFD_DEBUG_TRAP_MAX_WATCHPOINTS_PER_DEVICE_V1)
            .ok_or(KfdDebugSessionErrorV1::WatchCapacity)?;
        if self.watches.len() >= max {
            return Err(KfdDebugSessionErrorV1::WatchCapacity);
        }
        let watch_id = self
            .backend
            .set_address_watch(self.plan.target_pid(), gpu_id, address, mask, mode)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        if watch_id >= KFD_DEBUG_TRAP_MAX_WATCHPOINTS_PER_DEVICE_V1 as u32
            || self
                .watches
                .iter()
                .any(|watch| watch.gpu_id == gpu_id && watch.watch_id == watch_id)
        {
            return Err(KfdDebugSessionErrorV1::DuplicateWatch);
        }
        let observation = KfdDebugAddressWatchObservationV1 { gpu_id, watch_id };
        self.watches.push(observation);
        Ok(observation)
    }

    fn clear_address_watch(
        &mut self,
        watch: KfdDebugAddressWatchObservationV1,
    ) -> Result<(), KfdDebugSessionErrorV1> {
        self.ensure_hardware()?;
        let Some(index) = self
            .watches
            .iter()
            .position(|candidate| *candidate == watch)
        else {
            return Err(KfdDebugSessionErrorV1::DuplicateWatch);
        };
        self.backend
            .clear_address_watch(self.plan.target_pid(), watch.gpu_id, watch.watch_id)
            .map_err(KfdDebugSessionErrorV1::Backend)?;
        self.watches.remove(index);
        Ok(())
    }

    fn finish(mut self) -> Result<(), KfdDebugSessionErrorV1> {
        let result = self.cleanup();
        self.finished = true;
        result
    }

    fn cleanup(&mut self) -> Result<(), KfdDebugSessionErrorV1> {
        if self.finished {
            return Ok(());
        }
        let mut first_error = None;
        if !self.suspended.is_empty() {
            let original = self.suspended.clone();
            let mut wire = original.clone();
            match self
                .backend
                .resume_queues(self.plan.target_pid(), &mut wire)
            {
                Ok(completed) => match decode_queue_results(&original, &wire, completed) {
                    Ok(results)
                        if results.iter().all(|result| {
                            result.state == KfdDebugQueueOperationStateV1::Complete
                        }) =>
                    {
                        self.suspended.clear();
                    }
                    _ => first_error = Some("resume queues"),
                },
                Err(_) => first_error = Some("resume queues"),
            }
        }
        for watch in core::mem::take(&mut self.watches) {
            if self
                .backend
                .clear_address_watch(self.plan.target_pid(), watch.gpu_id, watch.watch_id)
                .is_err()
                && first_error.is_none()
            {
                first_error = Some("clear address watch");
            }
        }
        if self.launch_mode_changed
            && self
                .backend
                .set_launch_mode(self.plan.target_pid(), KfdDebugTrapWaveLaunchModeV1::Normal)
                .is_err()
            && first_error.is_none()
        {
            first_error = Some("restore launch mode");
        }
        if let Some(previous) = self.original_override.take()
            && self
                .backend
                .set_launch_override(
                    self.plan.target_pid(),
                    KfdDebugTrapOverrideModeV1::Replace,
                    previous,
                    previous,
                )
                .is_err()
            && first_error.is_none()
        {
            first_error = Some("restore launch override");
        }
        if let Some(previous) = self.original_flags.take()
            && self
                .backend
                .set_flags(self.plan.target_pid(), previous)
                .is_err()
            && first_error.is_none()
        {
            first_error = Some("restore flags");
        }
        if self.backend.disable(self.plan.target_pid()).is_err() && first_error.is_none() {
            first_error = Some("disable debug trap");
        }
        self.finished = true;
        match first_error {
            Some(operation) => Err(KfdDebugSessionErrorV1::Cleanup(operation)),
            None => Ok(()),
        }
    }
}

impl<B: DebugTrapBackendV1> Drop for DebugTrapSessionEngineV1<B> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn admit_runtime(
    raw: KfdRuntimeInfoV1,
) -> Result<KfdDebugRuntimeObservationV1, KfdDebugSessionErrorV1> {
    let state = raw
        .state()
        .ok_or(KfdDebugSessionErrorV1::InvalidRuntimeInfo)?;
    if raw.ttmp_setup > 1 {
        return Err(KfdDebugSessionErrorV1::InvalidRuntimeInfo);
    }
    Ok(KfdDebugRuntimeObservationV1 {
        state,
        ttmp_setup: raw.ttmp_setup != 0,
        runtime_metadata_present: raw.r_debug != 0,
    })
}

fn admit_queue_snapshot(
    raw: KfdDebugQueueSnapshotEntryV1,
) -> Result<KfdDebugQueueObservationV1, KfdDebugSessionErrorV1> {
    let exception_status = KfdDebugExceptionMaskV1::new(raw.exception_status)
        .ok_or(KfdDebugSessionErrorV1::InvalidExceptionMask)?;
    if raw.reserved != 0 || raw.queue_type > 3 || raw.ring_size == 0 {
        return Err(KfdDebugSessionErrorV1::SnapshotEntry);
    }
    Ok(KfdDebugQueueObservationV1 {
        exception_status,
        queue_id: raw.queue_id,
        gpu_id: raw.gpu_id,
        ring_size: raw.ring_size,
        queue_type: raw.queue_type,
        context_save_area_size: raw.ctx_save_restore_area_size,
    })
}

fn admit_device_snapshot(
    raw: KfdDebugDeviceSnapshotEntryV1,
) -> Result<KfdDebugDeviceObservationV1, KfdDebugSessionErrorV1> {
    let exception_status = KfdDebugExceptionMaskV1::new(raw.exception_status)
        .ok_or(KfdDebugSessionErrorV1::InvalidExceptionMask)?;
    if raw.gpu_id == 0 || raw.vendor_id == 0 || raw.device_id == 0 || raw.num_xcc == 0 {
        return Err(KfdDebugSessionErrorV1::SnapshotEntry);
    }
    Ok(KfdDebugDeviceObservationV1 {
        exception_status,
        gpu_id: raw.gpu_id,
        location_id: raw.location_id,
        vendor_id: raw.vendor_id,
        device_id: raw.device_id,
        revision_id: raw.revision_id,
        firmware_version: raw.fw_version,
        gfx_target_version: raw.gfx_target_version,
        simd_count: raw.simd_count,
        max_waves_per_simd: raw.max_waves_per_simd,
        array_count: raw.array_count,
        simd_arrays_per_engine: raw.simd_arrays_per_engine,
        xcc_count: raw.num_xcc,
        capability_bits: raw.capability,
        debug_property_bits: raw.debug_prop,
    })
}

fn validate_queue_list(queues: &[u32], limit: usize) -> Result<(), KfdDebugSessionErrorV1> {
    if queues.is_empty() {
        return Err(KfdDebugSessionErrorV1::QueueListEmpty);
    }
    if queues.len() > limit {
        return Err(KfdDebugSessionErrorV1::QueueListTooLarge);
    }
    for (index, queue) in queues.iter().enumerate() {
        if queue & (KFD_DBG_QUEUE_ERROR_MASK_V1 | KFD_DBG_QUEUE_INVALID_MASK_V1) != 0
            || queues[..index].contains(queue)
        {
            return Err(KfdDebugSessionErrorV1::DuplicateQueue);
        }
    }
    Ok(())
}

fn decode_queue_results(
    original: &[u32],
    wire: &[u32],
    completed: usize,
) -> Result<Vec<KfdDebugQueueOperationObservationV1>, KfdDebugSessionErrorV1> {
    if wire.len() != original.len() || completed > original.len() {
        return Err(KfdDebugSessionErrorV1::QueueResult);
    }
    let mut observed_completed = 0;
    let mut results = Vec::with_capacity(original.len());
    for (expected, returned) in original.iter().zip(wire) {
        let id = returned & !(KFD_DBG_QUEUE_ERROR_MASK_V1 | KFD_DBG_QUEUE_INVALID_MASK_V1);
        if id != *expected
            || returned & KFD_DBG_QUEUE_ERROR_MASK_V1 != 0
                && returned & KFD_DBG_QUEUE_INVALID_MASK_V1 != 0
        {
            return Err(KfdDebugSessionErrorV1::QueueResult);
        }
        let state = if returned & KFD_DBG_QUEUE_ERROR_MASK_V1 != 0 {
            KfdDebugQueueOperationStateV1::HardwareError
        } else if returned & KFD_DBG_QUEUE_INVALID_MASK_V1 != 0 {
            KfdDebugQueueOperationStateV1::Invalid
        } else {
            observed_completed += 1;
            KfdDebugQueueOperationStateV1::Complete
        };
        results.push(KfdDebugQueueOperationObservationV1 {
            queue_id: id,
            state,
        });
    }
    if observed_completed != completed {
        return Err(KfdDebugSessionErrorV1::QueueResult);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kfd_uapi::{
        KFD_DBG_TRAP_MASK_DBG_ADDRESS_WATCH_V1, KfdDebugLaunchOverrideMaskV1,
        KfdMemoryExceptionFailureV1,
    };
    use sha2::{Digest, Sha256};

    #[derive(Default)]
    struct ScriptedBackend {
        calls: Vec<&'static str>,
        runtime: KfdRuntimeInfoV1,
        queues: Vec<KfdDebugQueueSnapshotEntryV1>,
        devices: Vec<KfdDebugDeviceSnapshotEntryV1>,
        event: Option<(u64, u32, u32)>,
        exception_info: Option<BackendExceptionInfoV1>,
        next_watch: u32,
        fail_cleanup: Option<&'static str>,
    }

    impl ScriptedBackend {
        fn call(&mut self, operation: &'static str) -> Result<(), &'static str> {
            self.calls.push(operation);
            if self.fail_cleanup == Some(operation) {
                Err(operation)
            } else {
                Ok(())
            }
        }
    }

    impl DebugTrapBackendV1 for ScriptedBackend {
        fn enable(
            &mut self,
            _: u32,
            _: KfdDebugExceptionMaskV1,
        ) -> Result<KfdRuntimeInfoV1, &'static str> {
            self.call("enable")?;
            Ok(self.runtime)
        }
        fn disable(&mut self, _: u32) -> Result<(), &'static str> {
            self.call("disable")
        }
        fn send_runtime_event(
            &mut self,
            _: u32,
            _: KfdDebugExceptionMaskV1,
            _: u32,
            _: u32,
        ) -> Result<(), &'static str> {
            self.call("runtime-event")
        }
        fn set_exceptions(
            &mut self,
            _: u32,
            _: KfdDebugExceptionMaskV1,
        ) -> Result<(), &'static str> {
            self.call("exceptions")
        }
        fn queue_snapshot(
            &mut self,
            _: u32,
            _: usize,
            _: KfdDebugExceptionMaskV1,
        ) -> Result<Vec<KfdDebugQueueSnapshotEntryV1>, &'static str> {
            self.call("queues")?;
            Ok(self.queues.clone())
        }
        fn device_snapshot(
            &mut self,
            _: u32,
            _: usize,
            _: KfdDebugExceptionMaskV1,
        ) -> Result<Vec<KfdDebugDeviceSnapshotEntryV1>, &'static str> {
            self.call("devices")?;
            Ok(self.devices.clone())
        }
        fn query_event(
            &mut self,
            _: u32,
            _: KfdDebugExceptionMaskV1,
        ) -> Result<Option<(u64, u32, u32)>, &'static str> {
            self.call("event")?;
            Ok(self.event.take())
        }
        fn query_exception_info(
            &mut self,
            _: u32,
            _: u32,
            _: KfdDebugTrapExceptionCodeV1,
            _: bool,
        ) -> Result<BackendExceptionInfoV1, &'static str> {
            self.call("info")?;
            Ok(self
                .exception_info
                .take()
                .unwrap_or(BackendExceptionInfoV1::None))
        }
        fn suspend_queues(
            &mut self,
            _: u32,
            queues: &mut [u32],
            _: KfdDebugExceptionMaskV1,
            _: u32,
        ) -> Result<usize, &'static str> {
            self.call("suspend")?;
            if queues.len() > 1 {
                queues[1] |= KFD_DBG_QUEUE_INVALID_MASK_V1;
                Ok(queues.len() - 1)
            } else {
                Ok(queues.len())
            }
        }
        fn resume_queues(&mut self, _: u32, queues: &mut [u32]) -> Result<usize, &'static str> {
            self.call("resume")?;
            Ok(queues.len())
        }
        fn set_launch_mode(
            &mut self,
            _: u32,
            mode: KfdDebugTrapWaveLaunchModeV1,
        ) -> Result<(), &'static str> {
            self.call(if mode == KfdDebugTrapWaveLaunchModeV1::Normal {
                "launch-normal"
            } else {
                "launch"
            })
        }
        fn set_launch_override(
            &mut self,
            _: u32,
            _: KfdDebugTrapOverrideModeV1,
            _: KfdDebugLaunchOverrideMaskV1,
            support: KfdDebugLaunchOverrideMaskV1,
        ) -> Result<(KfdDebugLaunchOverrideMaskV1, KfdDebugLaunchOverrideMaskV1), &'static str>
        {
            self.call("override")?;
            Ok((KfdDebugLaunchOverrideMaskV1::NONE, support))
        }
        fn set_flags(
            &mut self,
            _: u32,
            _: KfdDebugTrapFlagsV1,
        ) -> Result<KfdDebugTrapFlagsV1, &'static str> {
            self.call("flags")?;
            Ok(KfdDebugTrapFlagsV1::NONE)
        }
        fn set_address_watch(
            &mut self,
            _: u32,
            _: u32,
            _: u64,
            _: u32,
            _: KfdDebugTrapAddressWatchModeV1,
        ) -> Result<u32, &'static str> {
            self.call("watch")?;
            let id = self.next_watch;
            self.next_watch += 1;
            Ok(id)
        }
        fn clear_address_watch(&mut self, _: u32, _: u32, _: u32) -> Result<(), &'static str> {
            self.call("clear-watch")
        }
    }

    fn plan() -> KfdDebugSessionPlanV1 {
        KfdDebugSessionPlanV1::new(200, 100, KfdDebugExceptionMaskV1::ALL, 8).unwrap()
    }

    #[test]
    fn debug_session_manifest_is_frozen() {
        let digest = Sha256::digest(KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_V1);
        let rendered: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        assert_eq!(rendered, KFD_DEBUG_SESSION_FOUNDATION_MANIFEST_SHA256_V1);
    }

    fn enabled_backend() -> ScriptedBackend {
        ScriptedBackend {
            runtime: KfdRuntimeInfoV1 {
                r_debug: 0,
                runtime_state: 1,
                ttmp_setup: 1,
            },
            ..Default::default()
        }
    }

    #[test]
    fn plan_is_bounded_and_cross_process() {
        assert_eq!(
            KfdDebugSessionPlanV1::new(0, 1, KfdDebugExceptionMaskV1::NONE, 1),
            Err(KfdDebugPlanErrorV1::TargetPidZero)
        );
        assert_eq!(
            KfdDebugSessionPlanV1::new(1, 1, KfdDebugExceptionMaskV1::NONE, 1),
            Err(KfdDebugPlanErrorV1::TargetIsDebugger)
        );
        assert_eq!(
            KfdDebugSessionPlanV1::new(2, 1, KfdDebugExceptionMaskV1::NONE, 0),
            Err(KfdDebugPlanErrorV1::SnapshotLimitZero)
        );
        assert_eq!(
            KfdDebugSessionPlanV1::new(u32::MAX, 1, KfdDebugExceptionMaskV1::NONE, 1),
            Err(KfdDebugPlanErrorV1::TargetPidOutOfRange)
        );
    }

    #[test]
    fn proc_status_parser_is_fail_closed() {
        assert_eq!(
            parse_target_status("Name:\ttarget\nState:\tT (stopped)\nTgid:\t41\nTracerPid:\t17\n"),
            Some(TargetStatusV1 {
                thread_group_id: 41,
                tracer_tid: 17,
                live: true,
            })
        );
        assert_eq!(
            parse_target_status("State:\tZ (zombie)\nTgid:\t41\nTracerPid:\t17\n"),
            Some(TargetStatusV1 {
                thread_group_id: 41,
                tracer_tid: 17,
                live: false,
            })
        );
        assert!(parse_target_status("State:\tT\nTgid:\t41\n").is_none());
        assert!(
            parse_target_status("State:\tT\nTgid:\t41\nTracerPid:\t17\nTracerPid:\t17\n").is_none()
        );
        assert_eq!(parse_pidfd_target("Pid:\t41\n"), Some(Some(41)));
        assert_eq!(parse_pidfd_target("Pid:\t-1\n"), Some(None));
        assert_eq!(parse_pidfd_target("Pid:\t0\n"), Some(None));
        assert_eq!(parse_pidfd_target("Pid:\tbad\n"), None);
        assert_eq!(parse_pidfd_target("Pid:\t41\nPid:\t41\n"), None);
    }

    #[test]
    fn snapshots_and_events_are_redacted_and_validated() {
        let mut backend = enabled_backend();
        backend.queues.push(KfdDebugQueueSnapshotEntryV1 {
            queue_id: 7,
            gpu_id: 9,
            ring_size: 4096,
            queue_type: 2,
            ctx_save_restore_area_size: 8192,
            ..Default::default()
        });
        backend.devices.push(KfdDebugDeviceSnapshotEntryV1 {
            gpu_id: 9,
            vendor_id: 0x1002,
            device_id: 0x7461,
            gfx_target_version: 90402,
            num_xcc: 8,
            capability: 0xac77a280,
            debug_prop: 0x5e7,
            ..Default::default()
        });
        backend.event = Some((KfdDebugTrapExceptionCodeV1::QueueWaveTrap.mask(), 9, 7));
        let mut session = DebugTrapSessionEngineV1::enable(backend, plan()).unwrap();
        let queues = session
            .queue_snapshot(KfdDebugExceptionMaskV1::NONE)
            .unwrap();
        assert_eq!(queues[0].queue_id(), 7);
        let devices = session
            .device_snapshot(KfdDebugExceptionMaskV1::NONE)
            .unwrap();
        assert_eq!(devices[0].gfx_target_version(), 90402);
        assert!(
            session
                .query_event(KfdDebugExceptionMaskV1::NONE)
                .unwrap()
                .unwrap()
                .exceptions()
                .contains(KfdDebugTrapExceptionCodeV1::QueueWaveTrap)
        );
        assert_eq!(
            session.query_event(KfdDebugExceptionMaskV1::NONE).unwrap(),
            None
        );
    }

    #[test]
    fn typed_exception_info_rejects_payload_substitution() {
        let mut backend = enabled_backend();
        backend.exception_info = Some(BackendExceptionInfoV1::DeviceMemory(
            KfdHsaMemoryExceptionDataV1::from_untrusted_wire(
                KfdMemoryExceptionFailureV1::from_untrusted_wire(1, 0, 0, 0),
                0x1000,
                9,
                0,
            ),
        ));
        let mut session = DebugTrapSessionEngineV1::enable(backend, plan()).unwrap();
        assert!(matches!(
            session.query_exception_info(
                9,
                KfdDebugTrapExceptionCodeV1::DeviceMemoryViolation,
                false
            ),
            Ok(KfdDebugExceptionInfoV1::DeviceMemoryViolation {
                fault_address_present: true,
                ..
            })
        ));
    }

    #[test]
    fn queue_status_bits_are_decoded_and_only_success_is_owned() {
        let backend = enabled_backend();
        let mut session = DebugTrapSessionEngineV1::enable(backend, plan()).unwrap();
        let result = session
            .suspend_queues(&[3, 4], KfdDebugExceptionMaskV1::NONE, 0)
            .unwrap();
        assert_eq!(result[0].state(), KfdDebugQueueOperationStateV1::Complete);
        assert_eq!(result[1].state(), KfdDebugQueueOperationStateV1::Invalid);
        assert_eq!(session.suspended, vec![3]);
        session.resume_queues(&[3]).unwrap();
        assert!(session.suspended.is_empty());
    }

    #[test]
    fn cleanup_attempts_every_owned_restoration_after_failure() {
        let mut backend = enabled_backend();
        backend.fail_cleanup = Some("clear-watch");
        let mut session = DebugTrapSessionEngineV1::enable(backend, plan()).unwrap();
        session
            .suspend_queues(&[3], KfdDebugExceptionMaskV1::NONE, 0)
            .unwrap();
        session
            .set_address_watch(9, 0x1000, u32::MAX, KfdDebugTrapAddressWatchModeV1::All)
            .unwrap();
        session
            .set_launch_mode(KfdDebugTrapWaveLaunchModeV1::Halt)
            .unwrap();
        let mask =
            KfdDebugLaunchOverrideMaskV1::new(KFD_DBG_TRAP_MASK_DBG_ADDRESS_WATCH_V1).unwrap();
        session
            .set_launch_override(KfdDebugTrapOverrideModeV1::Or, mask, mask)
            .unwrap();
        session
            .set_flags(KfdDebugTrapFlagsV1::SINGLE_MEMORY_OPERATION)
            .unwrap();
        assert_eq!(
            session.cleanup(),
            Err(KfdDebugSessionErrorV1::Cleanup("clear address watch"))
        );
        assert!(session.backend.calls.ends_with(&[
            "resume",
            "clear-watch",
            "launch-normal",
            "override",
            "flags",
            "disable"
        ]));
    }

    #[test]
    fn hardware_operations_require_runtime_enable() {
        let backend = ScriptedBackend {
            runtime: KfdRuntimeInfoV1::default(),
            ..Default::default()
        };
        let mut session = DebugTrapSessionEngineV1::enable(backend, plan()).unwrap();
        assert_eq!(
            session.set_launch_mode(KfdDebugTrapWaveLaunchModeV1::Halt),
            Err(KfdDebugSessionErrorV1::RuntimeNotEnabled)
        );
    }

    #[test]
    fn process_runtime_info_refreshes_hardware_gate() {
        let mut backend = ScriptedBackend {
            runtime: KfdRuntimeInfoV1::default(),
            ..Default::default()
        };
        backend.exception_info = Some(BackendExceptionInfoV1::Runtime(KfdRuntimeInfoV1 {
            r_debug: 0,
            runtime_state: 1,
            ttmp_setup: 0,
        }));
        let mut session = DebugTrapSessionEngineV1::enable(backend, plan()).unwrap();
        assert_eq!(
            session.query_exception_info(0, KfdDebugTrapExceptionCodeV1::ProcessRuntime, false,),
            Ok(KfdDebugExceptionInfoV1::Runtime(
                KfdDebugRuntimeObservationV1 {
                    state: KfdDebugRuntimeStateV1::Enabled,
                    ttmp_setup: false,
                    runtime_metadata_present: false,
                }
            ))
        );
        session
            .set_launch_mode(KfdDebugTrapWaveLaunchModeV1::Halt)
            .unwrap();
    }

    #[test]
    fn runtime_disable_reconciles_kfd_invalidated_hardware_ownership() {
        let mut session = DebugTrapSessionEngineV1::enable(enabled_backend(), plan()).unwrap();
        session
            .suspend_queues(&[3], KfdDebugExceptionMaskV1::NONE, 0)
            .unwrap();
        session
            .set_address_watch(9, 0x1000, u32::MAX, KfdDebugTrapAddressWatchModeV1::All)
            .unwrap();
        session
            .set_launch_mode(KfdDebugTrapWaveLaunchModeV1::Halt)
            .unwrap();
        let mask =
            KfdDebugLaunchOverrideMaskV1::new(KFD_DBG_TRAP_MASK_DBG_ADDRESS_WATCH_V1).unwrap();
        session
            .set_launch_override(KfdDebugTrapOverrideModeV1::Or, mask, mask)
            .unwrap();
        session
            .set_flags(KfdDebugTrapFlagsV1::SINGLE_MEMORY_OPERATION)
            .unwrap();

        session.backend.exception_info =
            Some(BackendExceptionInfoV1::Runtime(KfdRuntimeInfoV1::default()));
        assert!(matches!(
            session.query_exception_info(
                0,
                KfdDebugTrapExceptionCodeV1::ProcessRuntime,
                true,
            ),
            Ok(KfdDebugExceptionInfoV1::Runtime(runtime))
                if runtime.state() == KfdDebugRuntimeStateV1::Disabled
        ));
        assert!(session.suspended.is_empty());
        assert!(session.watches.is_empty());
        assert!(!session.launch_mode_changed);
        assert_eq!(session.original_override, None);
        assert_eq!(session.original_flags, None);

        let calls_before_cleanup = session.backend.calls.len();
        session.cleanup().unwrap();
        assert_eq!(&session.backend.calls[calls_before_cleanup..], &["disable"]);
    }
}

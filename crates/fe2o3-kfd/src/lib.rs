#![deny(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use fe2o3_kfd_uapi::{
    AdmittedKfdUapi, KfdUapiVersion, KfdUapiVersionError, negotiate_kfd_uapi_version,
};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod linux;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod currentness;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod device;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod memory;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod shared_memory;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod persistent_allocation;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod persistent_sdma;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod wait;

#[cfg(target_os = "linux")]
mod queue_resources;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod queue;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod semantic_observation;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod sdma;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod debug_trap;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod stopped_state_v1;

#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
mod target_debug_telemetry_v1;
#[cfg(target_os = "linux")]
#[allow(unsafe_code)]
mod target_debug_telemetry_v2;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod queue_linux;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[allow(unsafe_code)]
mod memory_linux;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use memory::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use shared_memory::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use persistent_allocation::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use persistent_sdma::*;

#[cfg(target_os = "linux")]
pub use queue_resources::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use queue::{
    ComputeAqlQueueDestroyedV1, ComputeAqlQueueLaneDispatchV1, ComputeAqlQueueLaneV1,
    ComputeAqlQueueObservationV1, ComputeAqlQueueSessionErrorV1, ComputeAqlQueueSessionV1,
    GFX942_AQL_COMPLETION_MANIFEST_SHA256_V1, GFX942_AQL_COMPLETION_MANIFEST_V1,
    GFX942_AQL_DISPATCH_BINDING_MANIFEST_SHA256_V1, GFX942_AQL_DISPATCH_BINDING_MANIFEST_V1,
    GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1, GFX942_COMPUTE_AQL_SESSION_MANIFEST_V1,
    GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_SHA256_V1,
    GFX942_DEVICE_CONTENT_COPY_FOUNDATION_MANIFEST_V1,
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_SHA256_V1,
    GFX942_KFD_DISPATCH_TRANSACTION_MANIFEST_V1, GFX942_MAX_FIXED_DISPATCH_DATA_V1,
    GFX942_MAX_FIXED_DISPATCH_PACKETS_V1, GFX942_MAX_FIXED_DISPATCH_PROGRAMS_V1,
    Gfx942BarrierProbeExecutionObservationV1, Gfx942BarrierProbeFailureV1,
    Gfx942BarrierProbePollBoundErrorV1, Gfx942BarrierProbePollBoundV1,
    Gfx942BarrierProbeRingBackingV1, Gfx942BarrierProbeSuccessV1, Gfx942CompletedBatchV1,
    Gfx942CompletedDispatchBatchV1, Gfx942CompletedDispatchReadRequestV1,
    Gfx942CompletedDispatchReadbackV1, Gfx942CompletedDispatchSnapshotRequestV1,
    Gfx942CompletionBatchV1, Gfx942CompletionErrorV1, Gfx942CompletionPollV1,
    Gfx942CompletionPollWithProgressV1, Gfx942CompletionProgressV1,
    Gfx942CompletionRecycleObservationV1, Gfx942DetachedFixedDispatchV1,
    Gfx942DeviceContentDescriptorErrorV1, Gfx942DeviceContentDescriptorV1,
    Gfx942DeviceContentRoleV1, Gfx942DispatchBatchV1, Gfx942DispatchBindingErrorV1,
    Gfx942DispatchBufferBindingV1, Gfx942DispatchPollV1, Gfx942DispatchPollWithProgressV1,
    Gfx942DispatchProgressV1, Gfx942FixedDispatchDataKindV1, Gfx942FixedDispatchDataLayoutV1,
    Gfx942FixedDispatchDataV1, Gfx942FixedDispatchPacketV1, Gfx942KfdDebugTargetDispatchErrorV2,
    Gfx942KfdDebugTargetDispatchResultV2, Gfx942KfdDispatchBufferV1, Gfx942KfdDispatchErrorV1,
    Gfx942KfdDispatchPointerFixupV1, Gfx942KfdDispatchRequestErrorV1, Gfx942KfdDispatchRequestV1,
    Gfx942KfdDispatchResultV1, Gfx942KfdQueueExceptionObservationV1,
    Gfx942PromotedSdmaDestinationV1, Gfx942RecycledDispatchResourcesV1,
    Gfx942RecycledDispatchWriteRequestV1, Gfx942RepeatedByteContentV1,
    Gfx942SdmaBatchExecutionFailureV1, Gfx942SdmaBatchExecutionRecoveryV1,
    Gfx942SdmaBatchSubmissionFailureV1, Gfx942SdmaBufferTransitionFailureV1,
    Gfx942SdmaCompletedPromotionFailureV1, Gfx942SdmaDispatchDataBridgeV1,
    Gfx942SdmaDispatchDataDemotionFailureV1, Gfx942SdmaMultiQueueFailureCustodyV1,
    Gfx942SdmaMultiQueueFailureDispositionV1, Gfx942SdmaMultiQueueSubmissionFailureV1,
    Gfx942SdmaMultiQueueTerminalCustodyV1, Gfx942SdmaSubmissionFailureV1,
    Gfx942SdmaTerminalShardObservationV1, Gfx942TimeoutExecutionObservationV1,
    Gfx942TimeoutSignalObservationV1, KfdTargetRuntimeDebugQueueTeardownV1,
    KfdTargetRuntimeDebugQueueV1, NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_SHA256_V1,
    NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1, QuarantinedGfx942BarrierProbeV1,
    execute_gfx942_kfd_debug_target_dispatch_unchecked_v1,
    execute_gfx942_kfd_debug_target_dispatch_unchecked_v2,
    execute_gfx942_kfd_dispatch_unchecked_v1, preflight_gfx942_fixed_dispatch_replacement,
};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use sdma::{
    GFX942_SDMA_COPY_MANIFEST_SHA256_V1, GFX942_SDMA_COPY_MANIFEST_V1,
    GFX942_SDMA_COPY_PACKET_BYTES_V1, GFX942_SDMA_D2H_ENGINE_INDEX_V1,
    GFX942_SDMA_FENCE_PACKET_BYTES_V1, GFX942_SDMA_H2D_ENGINE_INDEX_V1,
    GFX942_SDMA_MAX_IN_FLIGHT_V1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
    GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1, GFX942_SDMA_MAX_MULTI_QUEUE_SHARDS_V1,
    GFX942_SDMA_MAX_STRIPED_QUEUES_V1, GFX942_SDMA_RING_BYTES_V1, GFX942_SDMA_SUBMISSION_BYTES_V1,
    Gfx942DirectionalSdmaQueueObservationV1, Gfx942NativeXgmiSdmaBatchV1,
    Gfx942NativeXgmiSdmaQueueV1, Gfx942SdmaBufferKindV1, Gfx942SdmaBufferV1,
    Gfx942SdmaCompletedCopyV1, Gfx942SdmaCopyPollV1, Gfx942SdmaCopyRequestV1,
    Gfx942SdmaCopySubmissionV1, Gfx942SdmaCopyTicketV1, Gfx942SdmaErrorV1,
    Gfx942SdmaMemoryPoolObservationV1, Gfx942SdmaMultiQueuePlanErrorV1, Gfx942SdmaMultiQueuePlanV1,
    Gfx942SdmaMultiQueueShardTicketsV1, Gfx942SdmaMultiQueueSubmissionV1, Gfx942SdmaPacketErrorV1,
    Gfx942SdmaQueueObservationV1, Gfx942SdmaQueueProgressObservationV1,
    Gfx942SdmaUnpublishedCopyRequestV1, Gfx942XgmiBatchSubmissionFailureV1,
    Gfx942XgmiBatchWaitFailureV1, Gfx942XgmiCompletedCopyV1, Gfx942XgmiCopyFailureV1,
    Gfx942XgmiCopyPollV1, Gfx942XgmiSdmaCopyRequestV1, Gfx942XgmiWaitFailureV1,
};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use semantic_observation::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use debug_trap::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use stopped_state_v1::*;

#[cfg(target_os = "linux")]
pub use target_debug_telemetry_v1::*;
#[cfg(target_os = "linux")]
pub use target_debug_telemetry_v2::*;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use currentness::{KfdClockCorrelationObservationV1, ObservableDeviceCurrentnessV1};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use device::*;

#[cfg(target_os = "linux")]
pub mod topology;

/// Default device node for the Linux KFD process interface.
pub const DEFAULT_KFD_PATH: &str = "/dev/kfd";

/// A kernel-reported UAPI version observation.
///
/// This is untrusted input. Checking it identifies a reviewed userspace schema;
/// it neither authenticates the device instance nor proves kernel behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdUapiObservation {
    reported: KfdUapiVersion,
}

impl KfdUapiObservation {
    pub const fn reported_version(self) -> KfdUapiVersion {
        self.reported
    }

    pub fn admit(self) -> Result<AdmittedKfdUapiIdentity, KfdUapiVersionError> {
        negotiate_kfd_uapi_version(self.reported).map(AdmittedKfdUapiIdentity::from_admitted)
    }
}

/// Stable schema identity available only after exact UAPI admission.
///
/// This detached value records a checked schema observation. It is not a live
/// descriptor, physical-device identity, process capability, or proof token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmittedKfdUapiIdentity {
    admitted: AdmittedKfdUapi,
}

impl AdmittedKfdUapiIdentity {
    fn from_admitted(admitted: AdmittedKfdUapi) -> Self {
        Self { admitted }
    }

    pub const fn schema_id(self) -> &'static str {
        self.admitted.schema_id()
    }

    pub const fn reported_version(self) -> KfdUapiVersion {
        self.admitted.reported_version()
    }

    pub const fn schema_manifest_sha256(self) -> &'static str {
        fe2o3_kfd_uapi::KFD_UAPI_SCHEMA_MANIFEST_SHA256
    }
}

/// Identity observed from the opened character-device file description.
///
/// These numbers make path replacement detectable within one observation, but
/// do not establish that the driver is KFD. R1 device binding must additionally
/// authenticate sysfs topology, boot/module identity, render-node correlation,
/// partition, target, and their generations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdNodeObservation {
    file_system_device: u64,
    inode: u64,
    character_device: u64,
}

impl KfdNodeObservation {
    pub const fn file_system_device(self) -> u64 {
        self.file_system_device
    }

    pub const fn inode(self) -> u64 {
        self.inode
    }

    pub const fn character_device(self) -> u64 {
        self.character_device
    }
}

/// An owned descriptor opened from `/dev/kfd` before UAPI admission.
///
/// This type deliberately has no `AsFd` or raw-descriptor implementation in
/// the public API. It is deliberately not `Sync`; the initial slice has no
/// modeled concurrent-operation policy.
pub struct OpenedKfd {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    fd: rustix::fd::OwnedFd,
    path: PathBuf,
    node: KfdNodeObservation,
    opener_pid: u32,
    not_sync: PhantomData<Cell<()>>,
}

impl fmt::Debug for OpenedKfd {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OpenedKfd")
            .field("path", &self.path)
            .field("node", &self.node)
            .field("opener_pid", &self.opener_pid)
            .finish_non_exhaustive()
    }
}

impl OpenedKfd {
    /// Opens `/dev/kfd` without following a terminal symlink.
    pub fn open_default() -> Result<Self, KfdAdapterError> {
        Self::open(DEFAULT_KFD_PATH)
    }

    /// Opens a path without granting admitted syscall authority. Kept private
    /// so production callers cannot substitute another device for `/dev/kfd`.
    fn open(path: impl AsRef<Path>) -> Result<Self, KfdAdapterError> {
        let path = path.as_ref();

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            linux::open_kfd(path)
        }

        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            let _ = path;
            Err(KfdAdapterError::UnsupportedPlatform)
        }
    }

    /// Queries the kernel-reported version without treating it as admitted.
    pub fn observe_uapi(&self) -> Result<KfdUapiObservation, KfdAdapterError> {
        self.ensure_process(std::process::id())?;

        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            linux::observe_uapi(&self.fd).map(|reported| KfdUapiObservation { reported })
        }

        #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
        {
            Err(KfdAdapterError::UnsupportedPlatform)
        }
    }

    /// Consumes the opened descriptor and records exact UAPI schema admission.
    ///
    /// The result is intentionally not device-bound operational authority.
    pub fn admit_uapi(self) -> Result<KfdWithAdmittedUapi, KfdAdapterError> {
        let uapi = self.observe_uapi()?.admit()?;
        Ok(KfdWithAdmittedUapi { opened: self, uapi })
    }

    /// Returns opening provenance, not the current identity of this pathname.
    pub fn opening_path(&self) -> &Path {
        &self.path
    }

    pub const fn node_observation(&self) -> KfdNodeObservation {
        self.node
    }

    fn ensure_process(&self, current_pid: u32) -> Result<(), KfdAdapterError> {
        if current_pid != self.opener_pid {
            return Err(KfdAdapterError::ProcessChanged {
                opener_pid: self.opener_pid,
                current_pid,
            });
        }
        Ok(())
    }
}

/// Owned descriptor with an exactly admitted KFD UAPI schema.
///
/// This is not yet a device-bound capability. VM, memory, and queue operations
/// must require a later typestate that also binds physical device, render node,
/// process, target, topology, boot/module, and contracted reset/currentness
/// observations.
pub struct KfdWithAdmittedUapi {
    opened: OpenedKfd,
    uapi: AdmittedKfdUapiIdentity,
}

impl fmt::Debug for KfdWithAdmittedUapi {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KfdWithAdmittedUapi")
            .field("path", &self.opened.path)
            .field("node", &self.opened.node)
            .field("uapi", &self.uapi)
            .finish_non_exhaustive()
    }
}

impl KfdWithAdmittedUapi {
    pub const fn uapi_identity(&self) -> AdmittedKfdUapiIdentity {
        self.uapi
    }

    /// Returns opening provenance, not the current identity of this pathname.
    pub fn opening_path(&self) -> &Path {
        self.opened.opening_path()
    }

    pub const fn node_observation(&self) -> KfdNodeObservation {
        self.opened.node_observation()
    }
}

#[derive(Debug)]
pub enum KfdAdapterError {
    UnsupportedPlatform,
    Open {
        path: PathBuf,
        source: rustix::io::Errno,
    },
    InspectDevice {
        path: PathBuf,
        source: rustix::io::Errno,
    },
    NotCharacterDevice(PathBuf),
    GetVersion(rustix::io::Errno),
    ProcessChanged {
        opener_pid: u32,
        current_pid: u32,
    },
    UnsupportedUapi(KfdUapiVersionError),
}

impl From<KfdUapiVersionError> for KfdAdapterError {
    fn from(error: KfdUapiVersionError) -> Self {
        Self::UnsupportedUapi(error)
    }
}

impl fmt::Display for KfdAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => {
                formatter.write_str("the first KFD adapter profile supports only Linux x86_64")
            }
            Self::Open { path, source } => {
                write!(formatter, "failed to open {}: {source}", path.display())
            }
            Self::InspectDevice { path, source } => {
                write!(formatter, "failed to inspect {}: {source}", path.display())
            }
            Self::NotCharacterDevice(path) => {
                write!(formatter, "{} is not a character device", path.display())
            }
            Self::GetVersion(source) => {
                write!(formatter, "KFD GET_VERSION failed: {source}")
            }
            Self::ProcessChanged {
                opener_pid,
                current_pid,
            } => write!(
                formatter,
                "KFD descriptor belongs to process {opener_pid}, not current process {current_pid}"
            ),
            Self::UnsupportedUapi(error) => {
                write!(
                    formatter,
                    "KFD reported an unreviewed UAPI version: {error:?}"
                )
            }
        }
    }
}

impl std::error::Error for KfdAdapterError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observation_is_not_implicitly_admitted() {
        let old = KfdUapiObservation {
            reported: KfdUapiVersion::new(1, 17),
        };
        assert!(matches!(
            old.admit(),
            Err(KfdUapiVersionError::MinorTooOld { .. })
        ));

        let reviewed = KfdUapiObservation {
            reported: KfdUapiVersion::new(1, 18),
        };
        let identity = reviewed.admit().unwrap();
        assert_eq!(identity.reported_version(), KfdUapiVersion::new(1, 18));
        assert_eq!(identity.schema_id(), fe2o3_kfd_uapi::KFD_UAPI_SCHEMA_ID);
        assert_eq!(
            identity.schema_manifest_sha256(),
            fe2o3_kfd_uapi::KFD_UAPI_SCHEMA_MANIFEST_SHA256
        );
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn non_kfd_descriptor_cannot_produce_a_uapi_observation() {
        let device = OpenedKfd::open("/dev/null").unwrap();
        assert!(matches!(
            device.observe_uapi(),
            Err(KfdAdapterError::GetVersion(_))
        ));
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    #[test]
    fn inherited_process_identity_is_rejected_before_an_ioctl() {
        let device = OpenedKfd::open("/dev/null").unwrap();
        let different_pid = device.opener_pid.wrapping_add(1);
        assert!(matches!(
            device.ensure_process(different_pid),
            Err(KfdAdapterError::ProcessChanged { .. })
        ));
    }
}

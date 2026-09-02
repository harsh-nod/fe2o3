#[cfg(feature = "qualification-legacy-hip-runtime")]
mod context;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod cooperative;
mod device_copy;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod device_target;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod error;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod event;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod launch;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod managed_memory;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod memory;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod memory_topology;
#[cfg(any(
    feature = "qualification-unsafe-launch",
    all(test, feature = "qualification-legacy-hip-runtime")
))]
mod module;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod operation;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod peer_access;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod pinned_memory;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod stream;
#[cfg(feature = "qualification-legacy-hip-runtime")]
mod vmm;

#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use context::GpuContext;
#[cfg(all(test, feature = "qualification-legacy-hip-runtime"))]
pub use cooperative::launch_cooperative_kernel_on_stream;
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use cooperative::{CooperativeCapabilityError, CooperativeLaunchCapability};
pub use device_copy::DeviceCopy;
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use device_target::ObservedDeviceTarget;
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use error::{Error, HipError, Result, check};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use event::{Event, EventOptions};
pub use fe2o3_macros::DeviceCopy;
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use launch::DevicePtr;
#[cfg(any(
    feature = "qualification-unsafe-launch",
    all(test, feature = "qualification-legacy-hip-runtime")
))]
pub use launch::{KernelParams, LaunchConfig, launch_kernel_on_stream};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use managed_memory::{
    ManagedAdviceReceipt, ManagedAdviceRequest, ManagedAdviceState, ManagedAllocation,
    ManagedMemoryCleanupError, ManagedMemoryError, ManagedMemoryLocation, ManagedMigrationReceipt,
    ManagedReclamationReceipt, ManagedResidencyState,
};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use memory::{
    DeviceBuffer, DeviceBufferIdentity, DeviceBufferRangeError, DeviceBufferRangeSplitMut,
    DeviceBufferRegion, DeviceBufferView, DeviceBufferViewMut,
};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use memory_topology::{
    AllocationIdentity, AllocationKind, ContextIdentity, MemoryCapabilities,
    MemoryTopologyObservation, MemoryTopologyObservationError, PhysicalDeviceIdentity,
};
#[cfg(any(
    feature = "qualification-unsafe-launch",
    all(test, feature = "qualification-legacy-hip-runtime")
))]
pub use module::{GpuFunction, GpuModule};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use operation::{BorrowedDeviceOperation, OwnedDeviceOperation};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use peer_access::{
    PeerAccess, PeerAccessCapability, PeerAccessCleanupError, PeerAccessCleanupOutcome,
    PeerAccessDirection, PeerAccessEnableError, PeerAccessObservationError,
};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use pinned_memory::PinnedHostBuffer;
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use stream::{Stream, StreamIdentity};
#[cfg(feature = "qualification-legacy-hip-runtime")]
pub use vmm::{
    VmmAccess, VmmAccessReceipt, VmmAccessibleAllocation, VmmCleanupError, VmmCleanupStage,
    VmmError, VmmLayout, VmmMapAmbiguity, VmmMappedAllocation, VmmReclamationReceipt,
    VmmUnmappedAllocation,
};

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
// Rejections retain the full move-only service or ticket. Boxing would add
// allocation and returning only an error would discharge storage borrows.
#![allow(clippy::result_large_err)]

//! Host typestate, model adapters, and typed allocation ownership for
//! persistent services.
//!
//! This crate retains caller-owned storage borrows while it checks inert
//! lifecycle, persistent-task dispatch, ticket, wait, epoch, and generation
//! descriptions. It consumes the canonical [`fe2o3_service_model`] and
//! [`fe2o3_host_api`] contracts. On Linux x86_64 its allocation module can own
//! real KFD-backed device-local and host-visible coherent allocations. Its
//! addressless fixed-batch layer composes inspected executables, exact kernarg
//! images, and checked device ranges into a long-lived KFD queue with linear
//! publish, completion, recycle, detach, rebind, and release custody.
//!
//! The ownership shape rejects early use of retained storage while a service
//! value remains live:
//!
//! ```compile_fail
//! use fe2o3_service_host::{prepare, ServiceContractV1, ServiceResourcesV1};
//!
//! fn early_queue_use<'contract, 'resource>(
//!     contract: &'contract ServiceContractV1<'contract>,
//!     queue: &'resource mut (),
//!     state: &'resource mut (),
//!     inputs: &'resource (),
//!     outputs: &'resource mut (),
//! ) {
//!     let resources = ServiceResourcesV1::new(queue, state, inputs, outputs);
//!     let service = prepare(contract, resources);
//!     let _early = &mut *queue;
//!     core::mem::drop(service);
//! }
//! ```
//!
//! Typestate also excludes skipped lifecycle edges:
//!
//! ```compile_fail
//! use fe2o3_service_host::PreparedServiceV1;
//! use fe2o3_service_model::ServiceStateV1;
//!
//! fn skip_start<'contract, 'resource>(
//!     service: PreparedServiceV1<'contract, 'resource, (), (), (), ()>,
//!     draining: &ServiceStateV1,
//! ) {
//!     let _ = service.drain(draining);
//! }
//! ```
//!
//! Tickets are move-only and therefore cannot be waited twice:
//!
//! ```compile_fail
//! use fe2o3_service_host::TaskTicketV1;
//!
//! fn duplicate(ticket: TaskTicketV1<'_, '_>) {
//!     let first = ticket;
//!     let second = ticket;
//!     core::mem::drop((first, second));
//! }
//! ```
//!
//! An outstanding ticket keeps the running service borrowed and blocks drain:
//!
//! ```compile_fail
//! use fe2o3_host_api::{DispatchRequestV1, DispatchResultV1};
//! use fe2o3_service_host::{QueueSlotBindingV1, RunningServiceV1};
//! use fe2o3_service_model::{ServiceStateV1, TaskIdV1};
//!
//! fn drain_with_ticket<'contract, 'resource>(
//!     service: RunningServiceV1<'contract, 'resource, (), (), (), ()>,
//!     slot: QueueSlotBindingV1,
//!     request: &DispatchRequestV1,
//!     result: &DispatchResultV1,
//!     draining: &ServiceStateV1,
//! ) {
//!     let ticket = service.submit(TaskIdV1(1), slot, request, result).unwrap();
//!     let _draining = service.drain(draining);
//!     core::mem::drop(ticket);
//! }
//! ```

extern crate alloc;

mod binding;
mod error;
mod lifecycle;
mod task;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod allocation;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod batch;
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod queue;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use allocation::{
    DEVICE_LOCAL_ALLOCATION_ROLES_V1, DeviceAllocationRoleMarkerV1, DeviceInputRoleV1,
    DeviceLocalAllocationV1, DeviceOutputRoleV1, DeviceStateRoleV1, DeviceWorkspaceRoleV1,
    HOST_VISIBLE_ALLOCATION_ROLES_V1, HostAllocationRoleMarkerV1, HostDownloadRoleV1,
    HostUploadRoleV1, HostVisibleAllocationV1, NeverPublishedV1, QuarantinedServiceAllocationsV1,
    SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_SHA256_V1, SERVICE_ALLOCATION_OWNERSHIP_MANIFEST_V1,
    ServiceAllocationAcquireErrorV1, ServiceAllocationErrorV1, ServiceAllocationKeyV1,
    ServiceAllocationKindMarkerV1, ServiceAllocationPhaseV1, ServiceAllocationRangePairV1,
    ServiceAllocationRangeV1, ServiceAllocationReleaseFailureV1,
    ServiceAllocationReleaseObservationV1, ServiceAllocationRoleMarkerV1,
    ServiceAllocationSessionV1, ServiceAllocationSubleaseSetV1, ServiceDeviceDispatchRangeV1,
    ServiceDispatchRangeV1, ServiceHostDispatchRangeV1,
};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use batch::{ServiceFixedBatchV1, ServiceFixedDispatchBufferV1, ServiceFixedDispatchPacketV1};
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub use queue::{
    QuarantinedServiceQueueResourcesV1, QuarantinedServiceQueueV1,
    SERVICE_QUEUE_OWNERSHIP_MANIFEST_SHA256_V1, SERVICE_QUEUE_OWNERSHIP_MANIFEST_V1,
    ServiceCompletedQueueSessionV1, ServiceCompletedReadRequestV1, ServiceCompletedReadbackV1,
    ServicePublishedQueueSessionV1, ServiceQueueBindFailureV1, ServiceQueueCreateFailureV1,
    ServiceQueueDataUpdateFailureV1, ServiceQueueErrorV1, ServiceQueueOperationFailureV1,
    ServiceQueuePartitionedDataUpdateV1, ServiceQueuePollV1, ServiceQueueReleaseFailureV1,
    ServiceQueueReleaseObservationV1, ServiceQueueSessionV1, ServiceQueueUnboundSessionV1,
    ServiceRecycledQueueSessionV1,
};

pub use binding::{QueueSlotBindingV1, ServiceContractV1, ServiceKeyV1};
pub use error::{BindingFieldV1, ServiceHostErrorV1};
pub use lifecycle::{
    DrainingServiceV1, FailedMayAccessServiceV1, FailedQuiescedServiceV1, FailedServiceV1,
    LifecycleCursorV1, LifecyclePhaseV1, PreparedServiceV1, ReleasedResourcesV1, RunningServiceV1,
    ServiceResourcesV1, StartingServiceV1, StoppedServiceV1, StoppingServiceV1,
    StorageDispositionV1, TransitionRejectedV1, prepare,
};
pub use task::{TaskCompletionV1, TaskTicketV1, TaskWaitRejectedV1};

/// Schema version represented by this adapter crate.
pub const SERVICE_HOST_SCHEMA_VERSION_V1: u16 = 1;

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
// Rejections retain the full move-only service or ticket. Boxing would add
// allocation and returning only an error would discharge storage borrows.
#![allow(clippy::result_large_err)]

//! Authority-free host typestate and model adapters for persistent services.
//!
//! This crate retains caller-owned storage borrows while it checks inert
//! lifecycle, persistent-task dispatch, ticket, wait, epoch, and generation
//! descriptions. It consumes the canonical [`fe2o3_service_model`] and
//! [`fe2o3_host_api`] contracts; it does not allocate, load, launch, execute,
//! wait, persist, authenticate, prove, or grant storage-release authority.
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

mod binding;
mod error;
mod lifecycle;
mod task;

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

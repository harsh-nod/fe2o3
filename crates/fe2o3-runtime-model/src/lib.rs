#![no_std]
#![forbid(unsafe_code)]

//! Pure Rust executable model for issue #137 runtime lifecycles.
//!
//! The model performs no I/O and grants no KFD, DRM, load, dispatch, completion,
//! or proof authority. It is the finite state-machine carrier that future Verus
//! specifications and syscall refinement layers can relate to concrete runtime
//! execution.
//!
//! All identities, observations, and transitions are intentionally constructible
//! by model clients. Therefore no value from this crate is runtime evidence.
//! Production adapters must seal identity and quiescence witnesses and prove a
//! refinement from their concrete operations before consuming modeled states.

extern crate alloc;
#[cfg(test)]
extern crate std;

mod async_queue;
mod closed_execution;
mod device_identity;
mod device_local;
mod device_projection;
mod identity;
mod kernel_semantics;
mod memory_lifecycle;
mod memory_pool;
mod model;
mod multi_device;
mod queue_lifecycle;
mod r11_runtime_semantics;
mod r12_native_concurrency;
mod r13_logical_scheduler;
mod r14_async_observer;
mod r16_worker_semantic_boundary;
mod r17_persistent_native_allocation;
mod r18_persistent_local_sdma_adapter;
mod r19_directional_persistent_local_sdma_adapter;
mod r20_runtime_facade_directional_chunking;
mod r21_runtime_scripted_failure_seam;
mod r22_batched_directional_persistent_sdma_windows;
mod r23_same_device_d2d_persistent_sdma_windows;
mod r24_portable_progress;
mod r25_persistent_compute_storage_bridge;
mod r9_native_evidence;
mod typed_async;

pub use async_queue::*;
pub use closed_execution::*;
pub use device_identity::*;
pub use device_local::*;
pub use device_projection::*;
pub use identity::*;
pub use kernel_semantics::*;
pub use memory_lifecycle::*;
pub use memory_pool::*;
pub use model::*;
pub use multi_device::*;
pub use queue_lifecycle::*;
pub use r9_native_evidence::*;
pub use r11_runtime_semantics::*;
pub use r12_native_concurrency::*;
pub use r13_logical_scheduler::*;
pub use r14_async_observer::*;
pub use r16_worker_semantic_boundary::*;
pub use r17_persistent_native_allocation::*;
pub use r18_persistent_local_sdma_adapter::*;
pub use r19_directional_persistent_local_sdma_adapter::*;
pub use r20_runtime_facade_directional_chunking::*;
pub use r21_runtime_scripted_failure_seam::*;
pub use r22_batched_directional_persistent_sdma_windows::*;
pub use r23_same_device_d2d_persistent_sdma_windows::*;
pub use r24_portable_progress::*;
pub use r25_persistent_compute_storage_bridge::*;
pub use typed_async::*;

#[cfg(test)]
mod async_queue_tests;
#[cfg(test)]
mod closed_execution_tests;
#[cfg(test)]
mod device_identity_tests;
#[cfg(test)]
mod device_local_tests;
#[cfg(test)]
mod device_projection_tests;
#[cfg(test)]
mod kernel_semantics_tests;
#[cfg(test)]
mod memory_lifecycle_tests;
#[cfg(test)]
mod memory_pool_tests;
#[cfg(test)]
mod multi_device_tests;
#[cfg(test)]
mod queue_lifecycle_tests;
#[cfg(test)]
mod r11_runtime_semantics_tests;
#[cfg(test)]
mod r12_native_concurrency_tests;
#[cfg(test)]
mod r13_logical_scheduler_tests;
#[cfg(test)]
mod r14_async_observer_tests;
#[cfg(test)]
mod r16_worker_semantic_boundary_tests;
#[cfg(test)]
mod r17_persistent_native_allocation_tests;
#[cfg(test)]
mod r18_persistent_local_sdma_adapter_tests;
#[cfg(test)]
mod r19_directional_persistent_local_sdma_adapter_tests;
#[cfg(test)]
mod r20_runtime_facade_directional_chunking_tests;
#[cfg(test)]
mod r21_runtime_scripted_failure_seam_tests;
#[cfg(test)]
mod r22_batched_directional_persistent_sdma_windows_tests;
#[cfg(test)]
mod r23_same_device_d2d_persistent_sdma_windows_tests;
#[cfg(test)]
mod r24_portable_progress_tests;
#[cfg(test)]
mod r25_persistent_compute_storage_bridge_tests;
#[cfg(test)]
mod r9_native_evidence_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod typed_async_tests;

//! One-shot real preparation for a disposable engineering process.
//! Preparation itself has no trap registration, runtime or metadata publication.
//! The explicit consuming no-queue successor is a separate engineering API.

use super::debug_metadata::{OwnedPreparedDebugMetadataV1, PreparedMetadataFactsV1};
use super::{Allocation, Backend, Context, Kernel, PerformanceCountersV1, Result, explain};
use crate::CheckedGfx950XnackMinusDevice;
use crate::engineering_gfx950_profile::{PAGE_BYTES, RING_BYTES, validate_profile};
use crate::memory::MemoryBackend;
use crate::queue_linux::ProcessGlobalKfdDebugReservationV1;
use fe2o3_aql::{AqlRingCapacityV1, AqlSingleProducerRingModelV1};
use fe2o3_kfd_uapi::KfdAllocMemoryFlags;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

#[path = "runtime_debug_cold_retention_v1.rs"]
pub(super) mod retention;
#[path = "runtime_debug_trap_gfx942_v1.rs"]
mod trap;
use retention::RetainNativeOnDropV1;

#[path = "engineering_gfx950_debug_noqueue_v1.rs"]
mod noqueue;
pub use noqueue::Gfx950DebugMetadataNoQueueOwnerV1;

#[path = "engineering_gfx950_debug_execution_v1.rs"]
mod execution;
pub use execution::{
    Gfx950DebugExecutionContractV1, Gfx950DebugExecutionPreparationErrorV1,
    Gfx950DebugExecutionPreparationV1, Gfx950DebugNativeRequirementV1,
    Gfx950DebugNativeUnavailableV1, Gfx950DebugQueueGeometryV1, Gfx950DebugQueueLifecycleErrorV1,
    Gfx950DebugQueueLifecycleEventV1, Gfx950DebugQueueLifecyclePhaseV1,
    validate_gfx950_debug_queue_lifecycle_v1,
};

#[path = "engineering_gfx950_debug_empty_local_v1.rs"]
mod empty_queue;
pub(crate) use empty_queue::DebugLocalTeardownWitnessV1;
pub use empty_queue::{
    Gfx950DebugAllocationRetirementV1, Gfx950DebugEmptyQueueV1, Gfx950DebugLocalErrorV1,
    Gfx950DebugLocalFailureV1, Gfx950DebugLocalPhaseV1, Gfx950DebugLocalReleaseCompleteV1,
    Gfx950DebugLocalStepV1, Gfx950DebugRuntimeEnableReturnedV1,
};

/// Immutable preparation facts, not a live debugger/queue/runtime capability.
/// Digests establish content identity, not source authentication or execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx950DebugColdPreparationFactsV1 {
    artifact_sha256: [u8; 32],
    artifact_bytes: usize,
    trap_sha256: [u8; 32],
    trap_bytes: usize,
    mapped_backing_bytes: u64,
    metadata_retained_bytes: usize,
}

impl Gfx950DebugColdPreparationFactsV1 {
    pub fn artifact_sha256(self) -> [u8; 32] {
        self.artifact_sha256
    }
    pub fn artifact_bytes(self) -> usize {
        self.artifact_bytes
    }
    pub fn trap_sha256(self) -> [u8; 32] {
        self.trap_sha256
    }
    pub fn trap_bytes(self) -> usize {
        self.trap_bytes
    }
    /// Logical admitted allocation backing, not process RSS.
    pub fn mapped_backing_bytes(self) -> u64 {
        self.mapped_backing_bytes
    }
    /// Logical owned metadata bytes, not allocator capacity or process RSS.
    pub fn metadata_retained_bytes(self) -> usize {
        self.metadata_retained_bytes
    }
}

struct ColdResourcesV1 {
    context: Context,
    trap: Option<Allocation>,
    metadata: Option<OwnedPreparedDebugMetadataV1>,
}

/// Owns actual checked-device, VM, one admitted kernel, reviewed trap text and
/// version-zero metadata. It is neither cloneable nor Send/Sync.
///
/// This is an explicit, one-shot, disposable-process preparation API. Once VM
/// acquisition may have begun, every error, unwind or Drop retains all native
/// resources until process teardown and poisons the exclusive process gate.
/// There is deliberately no close/retry/address API or teardown acknowledgment.
/// An explicit consuming method can register debug metadata without any queue.
/// Merely preparing this value does NOT install or execute the retained trap.
///
/// Available only on Linux x86_64 little-endian with engineering-gfx950.
pub struct Gfx950DebugColdOwnerV1 {
    // Field order is deliberate: retain resources before the gate token drops.
    resources: RetainNativeOnDropV1<ColdResourcesV1>,
    _reservation: ProcessGlobalKfdDebugReservationV1,
    facts: Gfx950DebugColdPreparationFactsV1,
}

impl Gfx950DebugColdOwnerV1 {
    pub fn prepare(
        device: CheckedGfx950XnackMinusDevice,
        object: Vec<u8>,
        symbol: String,
    ) -> Result<Self> {
        check_object_bound(object.len())?;
        // Normal admission before any VM effect: this is a check, not a
        // reconstructed Kernel, caller-supplied load bias or native authority.
        {
            let _closure = fe2o3_amdhsa_loader::validate(
                &object,
                fe2o3_amdhsa_loader::AdmittedProfile::Gfx950XnackOffCov6,
            )
            .map_err(explain)?
            .bind_kernel(&symbol)
            .map_err(explain)?;
        }
        let object_hash: [u8; 32] = Sha256::digest(&object).into();
        trap::check_text()?;
        let mut reservation = ProcessGlobalKfdDebugReservationV1::reserve().map_err(explain)?;
        let context = Context::cold_debug_context(device)?;
        let mut resources = RetainNativeOnDropV1::new(ColdResourcesV1 {
            context,
            trap: None,
            metadata: None,
        });
        resources.get_mut().context.check_currentness(true)?;
        // Exposure here means potential VM/allocation effects, NOT trap or
        // runtime publication. Set the sticky gate before the first such call.
        reservation.begin_external_transition().map_err(explain)?;
        resources.retain_before_native_effect();
        resources
            .get_mut()
            .context
            .backend
            .acquire_vm()
            .map_err(explain)?;
        resources.get_mut().context.check_currentness(true)?;
        let kernel =
            resources
                .get_mut()
                .context
                .materialize_admitted_kernel(object, object_hash, symbol)?;
        // The genuine normal loader created this Kernel and exact mapping.
        resources.get_mut().context.kernels.insert(1, kernel);
        resources.get_mut().context.next_kernel = 2;

        let text = trap::text();
        let trap_mapping = resources.get_mut().context.allocate_resource(
            text.len(),
            KfdAllocMemoryFlags::EXECUTABLE,
            |bytes| {
                bytes[..text.len()].copy_from_slice(text);
                Ok(())
            },
        )?;
        resources.get_mut().trap = Some(trap_mapping);
        {
            let resource = resources.get_mut();
            resource.context.check_currentness(true)?;
            let mapped = resource.trap.as_mut().ok_or("missing cold trap")?;
            resource
                .context
                .backend
                .protect_cpu_read_only(&mut mapped.mapping)
                .map_err(explain)?;
            resource.context.check_currentness(true)?;
        }
        {
            let resource = resources.get_mut();
            let kernel = resource
                .context
                .kernels
                .get(&1)
                .ok_or("missing cold kernel")?;
            resource.metadata =
                Some(OwnedPreparedDebugMetadataV1::from_kernel(kernel).map_err(explain)?);
        }
        let facts = resources.get_mut().finish()?;
        Ok(Self {
            resources,
            _reservation: reservation,
            facts,
        })
    }

    /// Returns preparation-time facts only; it does not revalidate currentness.
    pub fn facts(&self) -> Gfx950DebugColdPreparationFactsV1 {
        debug_assert!(self.resources.get().metadata.is_some());
        self.facts
    }
}

impl ColdResourcesV1 {
    fn finish(&mut self) -> Result<Gfx950DebugColdPreparationFactsV1> {
        self.context.check_currentness(true)?;
        require_cold_context(&self.context)?;
        if self.context.kernels.len() != 1 || self.context.handles.len() != 2 {
            return Err("cold resource cardinality".into());
        }
        let kernel: &Kernel = self.context.kernels.get(&1).ok_or("missing cold kernel")?;
        let mapped = self.trap.as_ref().ok_or("missing cold trap")?;
        let metadata = self.metadata.as_ref().ok_or("missing cold metadata")?;
        if mapped.va == kernel.code.va
            || mapped.handle == kernel.code.handle
            || mapped.requested != trap::text().len()
            || Backend::mapping_address(&mapped.mapping) != mapped.va
            || !metadata.matches_kernel(kernel)
        {
            return Err("cold resource identity".into());
        }
        let exact = Backend::with_bytes(&mapped.mapping, mapped.backing, |bytes| {
            bytes[..trap::text().len()] == *trap::text()
                && bytes[trap::text().len()..].iter().all(|byte| *byte == 0)
        });
        if !exact {
            return Err("cold trap mapping readback".into());
        }
        let PreparedMetadataFactsV1 {
            artifact_sha256,
            artifact_bytes,
            logical_retained_bytes,
        } = metadata.facts();
        let facts = Gfx950DebugColdPreparationFactsV1 {
            artifact_sha256,
            artifact_bytes,
            trap_sha256: trap::TEXT_SHA256,
            trap_bytes: trap::text().len(),
            mapped_backing_bytes: self.context.total_bytes,
            metadata_retained_bytes: logical_retained_bytes,
        };
        self.context.check_currentness(true)?;
        Ok(facts)
    }
}

impl Context {
    /// Construct only inert Rust owners. In particular do not call open(),
    /// initialize(), initialize_queue(), check_idle(), or ordinary runtime enable.
    fn cold_debug_context(device: CheckedGfx950XnackMinusDevice) -> Result<Self> {
        let unique_id = device.observation().unique_id();
        validate_profile(device.topology_snapshot(), unique_id).map_err(str::to_owned)?;
        if rustix::param::page_size() != PAGE_BYTES {
            return Err("unsupported host page size".into());
        }
        Ok(Self {
            backend: Backend::new(device),
            unique_id,
            runtime: None,
            event: None,
            queue_id: None,
            doorbell: None,
            internal: Vec::new(),
            buffers: BTreeMap::new(),
            kernels: BTreeMap::new(),
            handles: BTreeSet::new(),
            mmap_offsets: BTreeSet::new(),
            total_bytes: 0,
            next_buffer: 1,
            next_kernel: 1,
            ring: AqlSingleProducerRingModelV1::new(
                AqlRingCapacityV1::from_ring_bytes(RING_BYTES as u32).map_err(explain)?,
                0,
                0,
            )
            .map_err(explain)?,
            completed_write: 0,
            queue_epoch: 0,
            last_observed_read: 0,
            performance: None,
            counters: PerformanceCountersV1::default(),
            ordered_batch_poisoned: false,
        })
    }
}

fn require_cold_context(context: &Context) -> Result<()> {
    if context.runtime.is_some()
        || context.event.is_some()
        || context.queue_id.is_some()
        || context.doorbell.is_some()
        || !context.internal.is_empty()
        || !context.buffers.is_empty()
        || context.performance.is_some()
        || context.completed_write != 0
        || context.queue_epoch != 0
        || context.last_observed_read != 0
        || context.ordered_batch_poisoned
    {
        return Err("cold context acquired active state".into());
    }
    Ok(())
}

fn check_object_bound(len: usize) -> Result<()> {
    if len == 0 || len > fe2o3_hsaco::MAX_HSACO_BYTES {
        return Err("cold artifact bound".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "engineering_gfx950_debug_cold_v1_tests.rs"]
mod tests;

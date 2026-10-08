//! Exact whole-copy facts, retained by the original Context rather than callers.

use super::*;
use fe2o3_runtime_model::ContextAllocationReadV1;

mod copy;
mod graph;
mod selection;
mod storage;
pub use copy::*;
pub use selection::*;
pub(in crate::context) use storage::ReplicaPendingV1;
pub use storage::RuntimeReplicaStorageV1;
use storage::{ReplicaFactV1, ReplicaStateV1};

/// Fixed metadata bound, independent of the number of native queues.
pub const MAX_RUNTIME_REPLICA_RECORDS_V1: usize = 4096;
pub const MAX_RUNTIME_REPLICA_STORAGE_BYTES_V1: u64 = 64 * 1024 * 1024;

/// Locator for an actual settled copy. This is not independent data authority:
/// the original Context must revalidate both live allocation versions on use.
///
/// ```compile_fail
/// use fe2o3_runtime::RuntimeReplicaReferenceV1;
/// fn fabricate() -> RuntimeReplicaReferenceV1 {
///     RuntimeReplicaReferenceV1 { context: 1, slot: 0, incarnation: 1 }
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReplicaReferenceV1 {
    context: u64,
    slot: usize,
    incarnation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) struct ReplicaStampV1 {
    pub(in crate::context) region: RuntimeMemoryRegionV1,
    pub(in crate::context) record: AllocationRecordV1,
    pub(in crate::context) read: ContextAllocationReadV1,
}

/// Counts of retained metadata, not initialized bytes or available replicas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeReplicaUsageV1 {
    pub capacity: usize,
    pub pending: usize,
    pub settled: usize,
}

/// Returned configuration refusal retains the exact prepaid table and debit.
pub struct RuntimeReplicaConfigurationFailureV1 {
    pub storage: RuntimeReplicaStorageV1,
    pub error: RuntimeValidationErrorV1,
}

impl fmt::Debug for RuntimeReplicaConfigurationFailureV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeReplicaConfigurationFailureV1")
            .field("error", &self.error)
            .finish_non_exhaustive()
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    #[cfg(test)]
    pub(in crate::context) fn exhaust_replica_slot_for_test_v1(&mut self) {
        let table = self.replicas.as_mut().unwrap();
        assert_eq!(table.usage().pending, 0);
        assert_eq!(table.usage().settled, 0);
        for slot in table.slots.iter_mut() {
            slot.incarnation = u64::MAX;
        }
    }

    /// Installs one prepaid fixed table into this original, journaled Context.
    /// This reserves only Rust metadata, not GPU memory, routes or queues.
    pub fn configure_replica_registry_v1(
        &mut self,
        storage: RuntimeReplicaStorageV1,
    ) -> Result<(), RuntimeReplicaConfigurationFailureV1> {
        let validation = self.require_graph_access(None).and_then(|()| {
            if self.replicas.is_some()
                || self.versions.is_none()
                || !self.submissions.is_empty()
                || self.has_unpublished_holds_v1()
            {
                Err(RuntimeValidationErrorV1::ContextReserved)
            } else {
                Ok(())
            }
        });
        if let Err(error) = validation {
            return Err(RuntimeReplicaConfigurationFailureV1 { storage, error });
        }
        self.replicas = Some(storage);
        Ok(())
    }

    pub fn replica_registry_usage_v1(&self) -> Option<RuntimeReplicaUsageV1> {
        self.replicas.as_ref().map(RuntimeReplicaStorageV1::usage)
    }

    pub(super) fn pending_replicas_v1(&self) -> usize {
        use super::graph::retirement_bodies::graph_retirement_runtime_expr;
        super::graph::retirement_bodies::graph_pending_replicas_body_v1!(
            graph_retirement_runtime_expr,
            self
        )
    }

    /// Revalidates the exact original source and destination versions. Any write
    /// attempt, queued writer, disposal or Context mismatch rejects the old fact.
    pub fn validate_replica_v1(
        &self,
        reference: RuntimeReplicaReferenceV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.current_replica_v1(reference).map(|_| ())
    }

    /// Finds a current whole-copy locator by original allocation identities.
    /// Only actual settled table entries are considered; byte equality, graph
    /// reports and caller-declared versions cannot create an entry here.
    pub fn find_current_replica_v1(
        &self,
        source: RuntimeAllocationIdV1,
        destination: RuntimeAllocationIdV1,
    ) -> Result<Option<RuntimeReplicaReferenceV1>, RuntimeValidationErrorV1> {
        self.require_graph_access(None)?;
        let table = self
            .replicas
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        for (slot, entry) in table.slots.iter().enumerate() {
            if let ReplicaStateV1::Settled(fact) = entry.state
                && fact.source.region.allocation == source
                && fact.destination.region.allocation == destination
            {
                let reference = RuntimeReplicaReferenceV1 {
                    context: self.context_generation,
                    slot,
                    incarnation: entry.incarnation,
                };
                if self.current_replica_v1(reference).is_ok() {
                    return Ok(Some(reference));
                }
            }
        }
        Ok(None)
    }

    fn current_replica_v1(
        &self,
        reference: RuntimeReplicaReferenceV1,
    ) -> Result<ReplicaFactV1, RuntimeValidationErrorV1> {
        self.require_graph_access(None)?;
        if reference.context != self.context_generation {
            return Err(RuntimeValidationErrorV1::UnknownAllocation);
        }
        let fact = self
            .replicas
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?
            .fact(reference)?;
        self.validate_replica_stamp_v1(fact.source)?;
        self.validate_replica_stamp_v1(fact.destination)?;
        Ok(fact)
    }

    /// Explicitly forgets a settled metadata fact, never a pending native copy.
    /// Slot reincarnation prevents an old locator from selecting its replacement.
    pub fn forget_replica_v1(
        &mut self,
        reference: RuntimeReplicaReferenceV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(None)?;
        if reference.context != self.context_generation {
            return Err(RuntimeValidationErrorV1::UnknownAllocation);
        }
        let table = self
            .replicas
            .as_mut()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        table.fact(reference)?;
        table.slots[reference.slot].state = ReplicaStateV1::Vacant;
        Ok(())
    }
}

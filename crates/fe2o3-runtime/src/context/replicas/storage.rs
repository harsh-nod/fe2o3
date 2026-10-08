use super::*;
use fe2o3_resource_accounting::{
    HostMetadataTableV1, ResourceCreditAccountV1, ResourceCreditErrorV1,
    host_metadata_table_payload_bytes_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplicaFactV1 {
    pub(super) source: ReplicaStampV1,
    pub(super) destination: ReplicaStampV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::context) struct ReplicaPendingV1 {
    pub(in crate::context) source: ReplicaStampV1,
    pub(in crate::context) destination: ReplicaStampV1,
    pub(in crate::context) submission: Option<(RuntimeSubmissionIdV1, u64)>,
    pub(in crate::context) writer: Option<fe2o3_runtime_model::ContextWriterReferenceV1>,
    pub(in crate::context) destination_epoch: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ReplicaStateV1 {
    Vacant,
    Pending(ReplicaPendingV1),
    Settled(ReplicaFactV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplicaSlotV1 {
    pub(super) incarnation: u64,
    pub(super) state: ReplicaStateV1,
}

/// One fixed original-account table. Payload bytes are charged before allocation;
/// allocator overhead and the credit arena are excluded. No unaccounted mode.
/// A pending/ambiguous tracked copy prevents destruction of this owner before the
/// Context backend. Process termination is not cancellation or GPU quiescence.
pub struct RuntimeReplicaStorageV1 {
    pub(super) slots: HostMetadataTableV1<ReplicaSlotV1>,
}

impl RuntimeReplicaStorageV1 {
    pub fn preallocate(
        account: &ResourceCreditAccountV1,
        capacity: usize,
    ) -> Result<Self, ResourceCreditErrorV1> {
        Self::required_payload_bytes_v1(capacity)?;
        Ok(Self {
            slots: HostMetadataTableV1::try_new(capacity, Some(account), || ReplicaSlotV1 {
                incarnation: 0,
                state: ReplicaStateV1::Vacant,
            })?,
        })
    }

    /// Exact requested table payload, excluding allocator and account overhead.
    pub fn required_payload_bytes_v1(capacity: usize) -> Result<u64, ResourceCreditErrorV1> {
        if capacity == 0 || capacity > MAX_RUNTIME_REPLICA_RECORDS_V1 {
            return Err(ResourceCreditErrorV1::InvalidRecordCapacity);
        }
        let bytes = host_metadata_table_payload_bytes_v1::<ReplicaSlotV1>(capacity)
            .ok_or(ResourceCreditErrorV1::AllocationFailed)?;
        if bytes > MAX_RUNTIME_REPLICA_STORAGE_BYTES_V1 {
            return Err(ResourceCreditErrorV1::Capacity);
        }
        Ok(bytes)
    }

    pub(super) fn usage(&self) -> RuntimeReplicaUsageV1 {
        use super::super::graph::retirement_bodies::graph_retirement_runtime_expr;
        super::super::graph::retirement_bodies::graph_replica_usage_body_v1!(
            graph_retirement_runtime_expr,
            self,
            (index, pending, settled, count),
            [],
            []
        )
    }

    pub(super) fn reserve(
        &mut self,
        context: u64,
        pending: ReplicaPendingV1,
    ) -> Result<RuntimeReplicaReferenceV1, RuntimeValidationErrorV1> {
        let (slot, entry) = self
            .slots
            .iter_mut()
            .enumerate()
            .find(|(_, s)| s.state == ReplicaStateV1::Vacant && s.incarnation != u64::MAX)
            .ok_or(RuntimeValidationErrorV1::Capacity)?;
        entry.incarnation += 1;
        entry.state = ReplicaStateV1::Pending(pending);
        Ok(RuntimeReplicaReferenceV1 {
            context,
            slot,
            incarnation: entry.incarnation,
        })
    }

    pub(super) fn pending(
        &self,
        reference: RuntimeReplicaReferenceV1,
    ) -> Result<ReplicaPendingV1, RuntimeValidationErrorV1> {
        let slot = self
            .slots
            .get(reference.slot)
            .ok_or(RuntimeValidationErrorV1::UnknownSubmission)?;
        if slot.incarnation != reference.incarnation {
            return Err(RuntimeValidationErrorV1::UnknownSubmission);
        }
        match slot.state {
            ReplicaStateV1::Pending(pending) => Ok(pending),
            _ => Err(RuntimeValidationErrorV1::UnknownSubmission),
        }
    }

    pub(super) fn fact(
        &self,
        reference: RuntimeReplicaReferenceV1,
    ) -> Result<ReplicaFactV1, RuntimeValidationErrorV1> {
        let slot = self
            .slots
            .get(reference.slot)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        if slot.incarnation != reference.incarnation {
            return Err(RuntimeValidationErrorV1::UnknownAllocation);
        }
        match slot.state {
            ReplicaStateV1::Settled(fact) => Ok(fact),
            _ => Err(RuntimeValidationErrorV1::ContextReserved),
        }
    }
}

impl Drop for RuntimeReplicaStorageV1 {
    fn drop(&mut self) {
        if self.usage().pending != 0 {
            std::process::abort();
        }
    }
}

//! Ready-time original journal stamps and release-before-promotion graph join.

use super::*;
use crate::context::graph::ContextGraphSubmissionV1;

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(crate) fn preflight_graph_replica_capacity_v1(
        &self,
        count: usize,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_live()?;
        let table = self
            .replicas
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        let available = table
            .slots
            .iter()
            .filter(|slot| slot.state == ReplicaStateV1::Vacant && slot.incarnation != u64::MAX)
            .count();
        if count > available {
            return Err(RuntimeValidationErrorV1::Capacity);
        }
        Ok(())
    }

    pub(crate) fn preflight_graph_replica_regions_v1(
        &self,
        group: &RuntimeGraphGroupV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_live()?;
        if source.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
            || source.allocation == destination.allocation
            || source.byte_len != destination.byte_len
        {
            return Err(RuntimeValidationErrorV1::InvalidRange);
        }
        let versions = self
            .versions
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        for region in [source, destination] {
            let record = self
                .allocations
                .get(&region.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
            if !group.contains_device(record.device) {
                return Err(RuntimeValidationErrorV1::WrongDevice);
            }
            if region.byte_offset != 0 || region.byte_len == 0 || region.byte_len != record.byte_len
            {
                return Err(RuntimeValidationErrorV1::InvalidRange);
            }
            // Do not read a ProducedBy input early: the actual journal read and
            // writer epoch are captured only after the graph marks it Ready.
            versions
                .validate_live(region.allocation, record)
                .map_err(|_| RuntimeValidationErrorV1::ContextReserved)?;
        }
        Ok(())
    }

    pub(crate) fn prepare_graph_replica_copy_v1(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<PreparedContextGraphActionV1, RuntimeErrorV1<B::Error>> {
        let source_device = self
            .allocations
            .get(&source.allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?
            .device;
        let destination_device = self
            .allocations
            .get(&destination.allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?
            .device;
        let prepared = if source_device == destination_device {
            self.prepare_graph_copy_v1(stream, source, destination)?
        } else {
            self.prepare_graph_peer_copy_v1(stream, source, destination)?
        };
        Ok(PreparedContextGraphActionV1::ReplicaCopy {
            prepared: Box::new(prepared),
            source,
            destination,
        })
    }

    pub(in crate::context) fn submit_graph_replica_copy_v1(
        &mut self,
        token: ContextGraphReservationV1,
        prepared: PreparedContextGraphActionV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<ContextGraphSubmissionV1, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.require_graph_access(Some(token))?;
        if !matches!(
            prepared,
            PreparedContextGraphActionV1::Copy(_) | PreparedContextGraphActionV1::PeerCopy(_)
        ) {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        let pending = ReplicaPendingV1 {
            source: self.replica_stamp_with_access_v1(source, Some(token))?,
            destination: self.replica_stamp_with_access_v1(destination, Some(token))?,
            submission: None,
            writer: None,
            destination_epoch: None,
        };
        let reference = self
            .replicas
            .as_mut()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?
            .reserve(self.context_generation, pending)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            self.submit_graph_plain_action_v1(token, prepared)
        }));
        let original = match result {
            Ok(Ok(original)) => original,
            Ok(Err(error)) => {
                if !self.terminal
                    && matches!(
                        &error,
                        RuntimeErrorV1::Validation(_) | RuntimeErrorV1::BackendRejected(_)
                    )
                {
                    self.replica_slot_after_effect_v1(reference).state = ReplicaStateV1::Vacant;
                } else {
                    self.quarantine_after_async_command_panic_v1();
                }
                return Err(error);
            }
            Err(payload) => {
                self.quarantine_after_async_command_panic_v1();
                std::panic::resume_unwind(payload)
            }
        };
        match self.bind_replica_submission_v1(&original, pending) {
            Ok(pending) => {
                self.replica_slot_after_effect_v1(reference).state =
                    ReplicaStateV1::Pending(pending);
                Ok(ContextGraphSubmissionV1 {
                    original,
                    replica: Some(reference),
                })
            }
            Err(error) => {
                self.quarantine_after_async_command_panic_v1();
                Err(error.into())
            }
        }
    }

    pub(in crate::context) fn release_graph_replica_copy_v1(
        &mut self,
        token: ContextGraphReservationV1,
        original: &RuntimeSubmissionV1<()>,
        reference: RuntimeReplicaReferenceV1,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        let (status, fact) =
            self.prepare_replica_submission_retirement_v1(reference, original, Some(token))?;
        // An admitted whole graph input cannot change before this node retires.
        // Contradiction is unknown custody, never a false successful version.
        if status == RuntimeCompletionStatusV1::Succeeded && fact.is_none() {
            self.quarantine_after_async_command_panic_v1();
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription.into());
        }
        self.release_submission_ref(original, Some(token))?;
        self.replica_slot_after_effect_v1(reference).state =
            fact.map_or(ReplicaStateV1::Vacant, ReplicaStateV1::Settled);
        Ok(())
    }
}

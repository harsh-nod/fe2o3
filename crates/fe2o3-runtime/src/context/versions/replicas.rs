//! Private observations from the actual journal, never caller-supplied epochs.

use super::*;
use crate::context::replicas::{ReplicaPendingV1, ReplicaStampV1};
use fe2o3_runtime_model::ContextAllocationReadV1;

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    pub(in crate::context) fn replica_stamp_v1(
        &self,
        region: RuntimeMemoryRegionV1,
    ) -> Result<ReplicaStampV1, RuntimeValidationErrorV1> {
        self.replica_stamp_with_access_v1(region, None)
    }

    pub(in crate::context) fn replica_stamp_with_access_v1(
        &self,
        region: RuntimeMemoryRegionV1,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<ReplicaStampV1, RuntimeValidationErrorV1> {
        self.require_graph_access(access)?;
        let record = *self
            .allocations
            .get(&region.allocation)
            .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
        if region.byte_offset != 0 || region.byte_len == 0 || region.byte_len != record.byte_len {
            return Err(RuntimeValidationErrorV1::InvalidRange);
        }
        if !self
            .backend_allocations
            .contains(&record.backend_allocation)
            || !self.allocation_admission.has_expected_credit(
                region.allocation,
                record.device,
                record.byte_len,
            )
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let versions = self
            .versions
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        let allocation = versions
            .validate_live(region.allocation, &record)
            .map_err(|_| RuntimeValidationErrorV1::ContextReserved)?;
        let state = versions
            .journal
            .lookup_allocation(allocation)
            .map_err(|_| RuntimeValidationErrorV1::ContextReserved)?;
        let read = ContextAllocationReadV1 {
            allocation,
            device: state.device,
            byte_extent: state.byte_extent,
            byte_offset: 0,
            byte_len: record.byte_len,
            attempt_epoch: state.attempt_epoch,
            content_lineage: state.content_lineage,
        };
        versions
            .journal
            .validate_read(&read)
            .map_err(|_| RuntimeValidationErrorV1::ContextReserved)?;
        Ok(ReplicaStampV1 {
            region,
            record,
            read,
        })
    }

    pub(in crate::context) fn validate_replica_stamp_v1(
        &self,
        stamp: ReplicaStampV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.validate_replica_stamp_with_access_v1(stamp, None)
    }

    pub(in crate::context) fn validate_replica_stamp_with_access_v1(
        &self,
        stamp: ReplicaStampV1,
        access: Option<ContextGraphReservationV1>,
    ) -> Result<(), RuntimeValidationErrorV1> {
        if self.replica_stamp_with_access_v1(stamp.region, access)? != stamp {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        Ok(())
    }

    pub(in crate::context) fn bind_replica_submission_v1<A>(
        &self,
        submission: &RuntimeSubmissionV1<A>,
        mut pending: ReplicaPendingV1,
    ) -> Result<ReplicaPendingV1, RuntimeValidationErrorV1> {
        let record = self.submission_record(submission)?;
        if record.status != RuntimeCompletionStatusV1::Pending
            || record.quiescent
            || record.stream != submission.stream
            || record.device != pending.destination.record.device
            || record.producer_launch
            || record.same_device_copy
            || record.segmented_peer_copy
            || record.journal_producer_read.is_some()
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let versions = self
            .versions
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        let reads = self
            .validate_submission_readers_v1(submission.id, SubmissionWriterDomainV1::Ordinary)
            .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if reads.requests.as_slice() != [pending.source.read]
            || reads.sources.len() != 1
            || reads.sources[0].region != pending.source.region
            || reads.sources[0].record != pending.source.record
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let writer = record
            .journal_writer
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        let root = versions
            .submission_writers
            .get(&submission.id)
            .ok_or(RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if root.writer != writer
            || root.domain != SubmissionWriterDomainV1::Ordinary
            || root.queued.is_some()
            || root.allocations.len() != 1
            || root.members.len() != 1
            || root.allocations[0].id != pending.destination.region.allocation
            || root.allocations[0].record != pending.destination.record
            || root.allocations[0].disposed
            || root.disposal_started
            || root.journal_disposed
            || root.members[0].allocation != pending.destination.read.allocation
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        let state = versions
            .journal
            .lookup_allocation(pending.destination.read.allocation)
            .map_err(|_| RuntimeValidationErrorV1::InvalidBackendDescription)?;
        if state.pending_writer != Some(writer)
            || pending.destination.read.attempt_epoch.checked_add(1) != Some(state.attempt_epoch)
        {
            return Err(RuntimeValidationErrorV1::InvalidBackendDescription);
        }
        pending.submission = Some((submission.id, record.backend_submission));
        pending.writer = Some(writer);
        pending.destination_epoch = Some(state.attempt_epoch);
        Ok(pending)
    }
}

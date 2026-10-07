// Frozen from 535763f018e1bf2236d4e8abe7790daf3c9df242; only method names/visibility change.
// Original two-method bytes SHA256: 0a211c6e87d0a24097a01f9f8dc07f7ed8a6ac9070612c75f0f663c563b12a38
use super::*;

impl ContextVersionJournalV1 {
    fn baseline_preflight_begin_write_v1(
        &self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<usize, ContextVersionJournalErrorV1> {
        self.lookup_reserved(writer)?;
        let reserved_count = self
            .reserved_count
            .checked_sub(1)
            .ok_or(ContextVersionJournalErrorV1::InvalidState)?;
        let count = canonical.len();
        if count > self.allocation_capacity {
            return Err(ContextVersionJournalErrorV1::RosterCapacity);
        }
        for pair in canonical.windows(2) {
            if pair[0].allocation.key >= pair[1].allocation.key {
                return Err(ContextVersionJournalErrorV1::NonCanonicalRoster);
            }
        }
        for destination in canonical {
            let entry = self.exact_allocation(destination.allocation)?;
            if entry.device != destination.device {
                return Err(ContextVersionJournalErrorV1::AllocationDeviceMismatch);
            }
            if entry.byte_extent != destination.byte_extent {
                return Err(ContextVersionJournalErrorV1::AllocationExtentMismatch);
            }
            if entry.pending_member.is_some() {
                return Err(ContextVersionJournalErrorV1::AllocationBusy);
            }
            entry
                .attempt_epoch
                .checked_add(1)
                .ok_or(ContextVersionJournalErrorV1::EpochExhausted)?;
        }
        if count > self.member_free.len() {
            return Err(ContextVersionJournalErrorV1::MemberCapacity);
        }
        if count > self.scratch.len() {
            return Err(ContextVersionJournalErrorV1::InvalidState);
        }
        // Free-slot uniqueness is a preserved global invariant, not an arena scan.
        for index in 0..count {
            let member = self.free_member_slot(index);
            self.count_indexed_access();
            if self.members.get(member) != Some(&None) {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            self.count_indexed_access();
            if self.scratch[index].is_some() {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
        }
        Ok(reserved_count)
    }

    /// The caller supplies an already canonical whole-allocation roster.
    /// Full preflight precedes even scratch mutation; commit keeps the same borrow.
    pub(super) fn baseline_begin_write_v1(
        &mut self,
        writer: ContextWriterReferenceV1,
        canonical: &[ContextAllocationWriteV1],
    ) -> Result<(), ContextVersionJournalErrorV1> {
        let reserved_count = self.baseline_preflight_begin_write_v1(writer, canonical)?;
        let count = canonical.len();
        for (index, destination) in canonical.iter().enumerate() {
            let entry = self
                .read_allocation(destination.allocation.slot)
                .expect("whole-roster preflight retains exact allocations");
            let plan = BeginMemberPlanV1 {
                member_slot: self.free_member_slot(index),
                allocation: destination.allocation,
                prior_lineage: entry.content_lineage,
                attempt_epoch: entry.attempt_epoch + 1,
            };
            self.store_plan(index, plan);
        }
        let head = if count == 0 {
            None
        } else {
            self.count_indexed_access();
            Some(self.scratch[0].expect("complete member plan").member_slot)
        };
        for index in 0..count {
            self.count_indexed_access();
            let plan = self.scratch[index].take().expect("complete member plan");
            let next = if index + 1 == count {
                None
            } else {
                self.count_indexed_access();
                Some(
                    self.scratch[index + 1]
                        .expect("complete next member plan")
                        .member_slot,
                )
            };
            self.count_indexed_access();
            let _ = self.member_free.pop();
            self.count_indexed_access();
            self.members[plan.member_slot] = Some(MemberEntryV1 {
                writer,
                allocation: plan.allocation,
                prior_lineage: plan.prior_lineage,
                attempt_epoch: plan.attempt_epoch,
                next,
            });
            self.count_indexed_access();
            let entry = self.allocations[plan.allocation.slot]
                .as_mut()
                .expect("retained exact allocation");
            entry.attempt_epoch = plan.attempt_epoch;
            entry.pending_member = Some(plan.member_slot);
        }
        self.store_slot(
            writer.slot,
            Some(WriterEntryV1::Pending {
                key: writer.key,
                head,
                count,
            }),
        );
        self.reserved_count = reserved_count;
        Ok(())
    }
}

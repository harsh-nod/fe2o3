use super::*;

#[cfg(test)]
impl ContextVersionsV1 {
    pub(in crate::context) fn submission_disposal_quiescent_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Option<bool> {
        self.submission_writers
            .get(&id)
            .map(|root| root.disposal_quiescent)
    }

    pub(in crate::context) fn clear_disposal_quiescence_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        self.submission_writers
            .get_mut(&id)
            .unwrap()
            .disposal_quiescent = false;
    }

    pub(in crate::context) fn submission_disposal_member_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
        index: usize,
    ) -> (
        RuntimeAllocationIdV1,
        AllocationRecordV1,
        ContextAllocationWriteV1,
        bool,
    ) {
        let root = &self.submission_writers[&id];
        let allocation = &root.allocations[index];
        (
            allocation.id,
            allocation.record,
            root.members[index],
            root.disposal_group.map_or(allocation.disposed, |key| {
                self.group_member_disposed_v1(key, allocation.id)
            }),
        )
    }

    pub(in crate::context) fn submission_disposal_progress_for_test_v1(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Option<(usize, bool)> {
        self.submission_writers
            .get(&id)
            .map(|root| match root.disposal_group {
                None => (root.disposed_count, root.journal_disposed),
                Some(key) => (
                    root.allocations
                        .iter()
                        .filter(|entry| self.group_member_disposed_v1(key, entry.id))
                        .count(),
                    self.disposal_groups
                        [&RuntimeSubmissionIdV1::new(key.key.context_generation, key.key.local)]
                        .journal_disposed,
                ),
            })
    }

    pub(in crate::context) fn clear_disposal_phase_for_test_v1(
        &mut self,
        reference: ContextAllocationReferenceV1,
    ) {
        self.phases[reference.slot] = None;
    }

    pub(in crate::context) fn remove_submission_writer_root_for_test_v1(
        &mut self,
        id: RuntimeSubmissionIdV1,
    ) {
        self.submission_writers.remove(&id);
    }
}

//! Frozen unique native receipts for closed Ordinary writer groups.

use super::*;
use fe2o3_runtime_model::{
    ContextAllocationWriteV1, ContextQueuedWriterFlatGroupDisposalEvidenceV1,
    ContextQueuedWriterGroupDisposalErrorV1, ContextQueuedWriterGroupHeaderV1,
    ContextQueuedWriterStatusV1, ContextWriterReferenceV1, ContextWriterStateV1,
};

type E = ContextVersionJournalErrorV1;

fn group_id(writer: ContextWriterReferenceV1) -> RuntimeSubmissionIdV1 {
    RuntimeSubmissionIdV1::new(writer.key.context_generation, writer.key.local)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GroupAllocationIndexV1 {
    pub(super) key: ContextWriterReferenceV1,
    pub(super) ordinal: usize,
}

#[derive(Clone, Copy)]
struct GroupAllocationV1 {
    id: RuntimeAllocationIdV1,
    record: AllocationRecordV1,
    member: ContextAllocationWriteV1,
    disposed: bool,
}

pub(super) struct RetainedDisposalGroupV1 {
    writers: Vec<ContextQueuedWriterGroupHeaderV1>,
    members: Vec<ContextAllocationWriteV1>,
    allocations: Vec<ContextAllocationWriteV1>,
    receipts: Vec<GroupAllocationV1>,
    disposed_count: usize,
    committing: bool,
    pub(super) journal_disposed: bool,
    refunded_count: usize,
}

impl RetainedDisposalGroupV1 {
    fn evidence(&self) -> ContextQueuedWriterFlatGroupDisposalEvidenceV1<'_> {
        ContextQueuedWriterFlatGroupDisposalEvidenceV1 {
            writers: &self.writers,
            members: &self.members,
            allocations: &self.allocations,
        }
    }

    fn has_key(&self, key: ContextWriterReferenceV1) -> bool {
        self.writers
            .first()
            .is_some_and(|header| header.writer == key)
    }
}

enum PreparationFailure {
    Busy,
    Capacity,
    Journal(E),
}

impl From<E> for PreparationFailure {
    fn from(error: E) -> Self {
        Self::Journal(error)
    }
}

fn reserve<T>(values: &mut Vec<T>, additional: usize) -> Result<(), PreparationFailure> {
    values
        .try_reserve(additional)
        .map_err(|_| PreparationFailure::Capacity)
}

fn discover(
    writers: &mut Vec<ContextWriterReferenceV1>,
    seen: &mut HashMap<RuntimeSubmissionIdV1, ContextWriterReferenceV1>,
    writer: ContextWriterReferenceV1,
    capacity: usize,
) -> Result<(), PreparationFailure> {
    let id = group_id(writer);
    if let Some(existing) = seen.get(&id) {
        return if *existing == writer {
            Ok(())
        } else {
            Err(E::InvalidReference.into())
        };
    }
    if writers.len() >= capacity {
        return Err(E::InvalidState.into());
    }
    reserve(writers, 1)?;
    seen.try_reserve(1)
        .map_err(|_| PreparationFailure::Capacity)?;
    seen.insert(id, writer);
    writers.push(writer);
    Ok(())
}

impl ContextVersionsV1 {
    #[cfg(test)]
    pub(in crate::context) fn disposal_group_progress_for_test_v1(
        &self,
        key: ContextWriterReferenceV1,
    ) -> Option<(usize, bool, usize)> {
        self.disposal_groups.get(&group_id(key)).map(|group| {
            (
                group.disposed_count,
                group.journal_disposed,
                group.refunded_count,
            )
        })
    }

    #[cfg(test)]
    pub(in crate::context) fn disposal_group_counts_for_test_v1(&self) -> (usize, usize) {
        (self.disposal_groups.len(), self.disposal_allocations.len())
    }

    #[cfg(test)]
    pub(in crate::context) fn corrupt_disposal_group_index_for_test_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
    ) {
        self.disposal_allocations.get_mut(&id).unwrap().ordinal = usize::MAX;
    }

    #[cfg(test)]
    pub(super) fn group_member_disposed_v1(
        &self,
        key: ContextWriterReferenceV1,
        id: RuntimeAllocationIdV1,
    ) -> bool {
        let index = self.disposal_allocations[&id];
        assert_eq!(index.key, key);
        self.disposal_groups[&group_id(key)].receipts[index.ordinal].disposed
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn validate_group_root_v1(
        &self,
        writer: ContextWriterReferenceV1,
        group: Option<ContextWriterReferenceV1>,
    ) -> Result<&submissions::RetainedSubmissionWriterV1, E> {
        self.validate_disposal_submission_v1(group_id(writer), writer)?;
        let root = &self
            .versions
            .as_ref()
            .ok_or(E::InvalidState)?
            .submission_writers[&group_id(writer)];
        if root.domain != SubmissionWriterDomainV1::Ordinary
            || root.disposal_group != group
            || root.disposal_started != group.is_some()
            || root.journal_disposed
            || root.disposed_count != 0
            || root.members.is_empty()
            || root.members.len() != root.allocations.len()
        {
            return Err(E::InvalidState);
        }
        Ok(root)
    }

    fn build_disposal_group_v1(
        &self,
        seeds: &[ContextWriterReferenceV1],
    ) -> Result<RetainedDisposalGroupV1, PreparationFailure> {
        let versions = self.versions.as_ref().ok_or(E::InvalidState)?;
        if seeds.is_empty() || seeds.len() > versions.journal.writer_capacity() {
            return Err(E::RosterCapacity.into());
        }
        let mut writers = Vec::new();
        let mut seen = HashMap::new();
        for &writer in seeds {
            discover(
                &mut writers,
                &mut seen,
                writer,
                versions.journal.writer_capacity(),
            )?;
        }
        let mut cursor = 0;
        while cursor < writers.len() {
            let writer = writers[cursor];
            match versions.journal.lookup_writer(writer)? {
                ContextWriterStateV1::Unknown { .. } => {}
                ContextWriterStateV1::Reserved
                    if versions.journal.queued_writer_status(writer)?
                        == Some(ContextQueuedWriterStatusV1::Unknown) => {}
                _ => return Err(PreparationFailure::Busy),
            }
            self.validate_group_root_v1(writer, None)?;
            let mut result = Ok(());
            versions
                .journal
                .visit_writer_neighbors(writer, |neighbor| {
                    if result.is_ok() {
                        result = discover(
                            &mut writers,
                            &mut seen,
                            neighbor,
                            versions.journal.writer_capacity(),
                        );
                    }
                })?;
            result?;
            cursor += 1;
        }
        writers.sort_unstable_by_key(|writer| writer.key.local);
        let mut group = RetainedDisposalGroupV1 {
            writers: Vec::new(),
            members: Vec::new(),
            allocations: Vec::new(),
            receipts: Vec::new(),
            disposed_count: 0,
            committing: false,
            journal_disposed: false,
            refunded_count: 0,
        };
        reserve(&mut group.writers, writers.len())?;
        for writer in writers {
            let root = self.validate_group_root_v1(writer, None)?;
            let total = group
                .members
                .len()
                .checked_add(root.members.len())
                .ok_or(E::InvalidState)?;
            if total > versions.journal.member_capacity() {
                return Err(E::InvalidState.into());
            }
            reserve(&mut group.members, root.members.len())?;
            reserve(&mut group.receipts, root.members.len())?;
            group.writers.push(ContextQueuedWriterGroupHeaderV1 {
                writer,
                member_start: group.members.len(),
                member_count: root.members.len(),
            });
            group.members.extend_from_slice(&root.members);
            for (entry, &member) in root.allocations.iter().zip(&root.members) {
                if entry.disposed
                    || versions.disposal_allocations.contains_key(&entry.id)
                    || self.allocations.get(&entry.id) != Some(&entry.record)
                    || !self
                        .backend_allocations
                        .contains(&entry.record.backend_allocation)
                    || versions.whole_allocation(entry.id, &entry.record)? != member
                    || !self.allocation_admission.has_expected_credit(
                        entry.id,
                        entry.record.device,
                        entry.record.byte_len,
                    )
                {
                    return Err(E::InvalidAllocationReference.into());
                }
                group.receipts.push(GroupAllocationV1 {
                    id: entry.id,
                    record: entry.record,
                    member,
                    disposed: false,
                });
            }
        }
        group.receipts.sort_unstable_by_key(|entry| entry.id);
        for pair in group.receipts.windows(2) {
            if pair[0].id == pair[1].id
                && (pair[0].record != pair[1].record || pair[0].member != pair[1].member)
            {
                return Err(E::InvalidAllocationReference.into());
            }
        }
        group.receipts.dedup_by_key(|entry| entry.id);
        reserve(&mut group.allocations, group.receipts.len())?;
        group
            .allocations
            .extend(group.receipts.iter().map(|entry| entry.member));
        versions
            .journal
            .validate_flat_unknown_group_disposal(&group.evidence())?;
        Ok(group)
    }

    /// Seeds may name disconnected components. Discovery closes every component;
    /// no native release occurs until all storage and exact identities are frozen.
    pub(in crate::context) fn freeze_disposal_group_v1(
        &mut self,
        seeds: &[ContextWriterReferenceV1],
    ) -> Result<ContextWriterReferenceV1, RuntimeValidationErrorV1> {
        let group = match self.build_disposal_group_v1(seeds) {
            Ok(group) => group,
            Err(PreparationFailure::Busy) => return Err(RuntimeValidationErrorV1::ContextReserved),
            Err(PreparationFailure::Capacity) => return Err(RuntimeValidationErrorV1::Capacity),
            Err(PreparationFailure::Journal(error)) => return self.journal_result_v1(Err(error)),
        };
        let key = group.writers[0].writer;
        let versions = self.versions.as_mut().expect("configured journal");
        if versions.disposal_groups.contains_key(&group_id(key)) {
            return self.journal_result_v1(Err(E::InvalidState));
        }
        versions
            .disposal_groups
            .try_reserve(1)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        versions
            .disposal_allocations
            .try_reserve(group.receipts.len())
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        // All fallible validation and allocation precedes these index updates.
        for (ordinal, entry) in group.receipts.iter().enumerate() {
            versions
                .disposal_allocations
                .insert(entry.id, GroupAllocationIndexV1 { key, ordinal });
        }
        for header in &group.writers {
            let root = versions
                .submission_writers
                .get_mut(&group_id(header.writer))
                .expect("validated root");
            root.disposal_group = Some(key);
            root.disposal_started = true;
        }
        versions.disposal_groups.insert(group_id(key), group);
        Ok(key)
    }

    pub(super) fn prepare_group_allocation_disposal_v1(
        &self,
        id: RuntimeAllocationIdV1,
        record: AllocationRecordV1,
    ) -> Result<AllocationDisposalV1, E> {
        let versions = self.versions.as_ref().ok_or(E::InvalidState)?;
        let index = *versions
            .disposal_allocations
            .get(&id)
            .ok_or(E::InvalidReference)?;
        let group = versions
            .disposal_groups
            .get(&group_id(index.key))
            .ok_or(E::InvalidReference)?;
        let entry = group
            .receipts
            .get(index.ordinal)
            .ok_or(E::InvalidReference)?;
        if !group.has_key(index.key)
            || group.committing
            || group.journal_disposed
            || group.disposed_count >= group.receipts.len()
            || entry.disposed
            || entry.id != id
            || entry.record != record
            || group.allocations.get(index.ordinal) != Some(&entry.member)
            || self.allocations.get(&id) != Some(&record)
            || versions.whole_allocation(id, &record)? != entry.member
            || !self
                .backend_allocations
                .contains(&record.backend_allocation)
            || !self
                .allocation_admission
                .has_expected_credit(id, record.device, record.byte_len)
        {
            return Err(E::InvalidAllocationReference);
        }
        // Freeze authenticated every root. Until finalization, their rosters and
        // quiescence markers are immutable; releasing public metadata does not
        // remove them. Recheck the full group at commit, not on every member retry.
        self.validate_group_root_v1(index.key, Some(index.key))?;
        Ok(AllocationDisposalV1 {
            id,
            record,
            member: entry.member,
            kind: AllocationDisposalKindV1::Group {
                key: index.key,
                index: index.ordinal,
            },
        })
    }

    pub(super) fn finish_group_allocation_disposal_v1(
        &mut self,
        plan: AllocationDisposalV1,
    ) -> Result<(), E> {
        let AllocationDisposalKindV1::Group { key, index } = plan.kind else {
            return Err(E::InvalidState);
        };
        let versions = self.versions.as_mut().ok_or(E::InvalidState)?;
        let group = versions
            .disposal_groups
            .get_mut(&group_id(key))
            .ok_or(E::InvalidReference)?;
        if !group.has_key(key)
            || group.committing
            || group.journal_disposed
            || group.disposed_count >= group.receipts.len()
            || versions.disposal_allocations.get(&plan.id)
                != Some(&GroupAllocationIndexV1 {
                    key,
                    ordinal: index,
                })
        {
            return Err(E::InvalidState);
        }
        let entry = group.receipts.get_mut(index).ok_or(E::InvalidReference)?;
        if entry.disposed
            || entry.id != plan.id
            || entry.record != plan.record
            || entry.member != plan.member
            || versions.phases.get(plan.member.allocation.slot)
                != Some(&Some(AllocationPhaseV1::Live))
        {
            return Err(E::InvalidState);
        }
        entry.disposed = true;
        group.disposed_count += 1;
        versions.phases[plan.member.allocation.slot] = Some(AllocationPhaseV1::Disposed);
        let removed = self.allocations.remove(&plan.id);
        let indexed = self
            .backend_allocations
            .remove(&plan.record.backend_allocation);
        if removed != Some(plan.record) || !indexed {
            return Err(E::InvalidAllocationReference);
        }
        if group.disposed_count == group.receipts.len() {
            self.commit_disposal_group_v1(key)?;
        }
        Ok(())
    }

    fn validate_disposed_group_v1(&self, key: ContextWriterReferenceV1) -> Result<(), E> {
        let versions = self.versions.as_ref().ok_or(E::InvalidState)?;
        let group = versions
            .disposal_groups
            .get(&group_id(key))
            .ok_or(E::InvalidReference)?;
        if !group.has_key(key)
            || group.committing
            || group.journal_disposed
            || group.refunded_count != 0
            || group.disposed_count != group.receipts.len()
            || group.allocations.len() != group.receipts.len()
        {
            return Err(E::InvalidState);
        }
        for header in &group.writers {
            let root = self.validate_group_root_v1(header.writer, Some(key))?;
            let end = header
                .member_start
                .checked_add(header.member_count)
                .ok_or(E::InvalidState)?;
            if group.members.get(header.member_start..end) != Some(root.members.as_slice()) {
                return Err(E::InvalidReference);
            }
            for (entry, member) in root.allocations.iter().zip(&root.members) {
                let index = versions
                    .disposal_allocations
                    .get(&entry.id)
                    .ok_or(E::InvalidReference)?;
                let original = group
                    .receipts
                    .get(index.ordinal)
                    .ok_or(E::InvalidReference)?;
                if entry.disposed
                    || index.key != key
                    || original.id != entry.id
                    || original.record != entry.record
                    || original.member != *member
                {
                    return Err(E::InvalidReference);
                }
            }
        }
        for (ordinal, entry) in group.receipts.iter().enumerate() {
            if !entry.disposed
                || group.allocations[ordinal] != entry.member
                || versions.disposal_allocations.get(&entry.id)
                    != Some(&GroupAllocationIndexV1 { key, ordinal })
                || self.allocations.contains_key(&entry.id)
                || versions.phases.get(entry.member.allocation.slot)
                    != Some(&Some(AllocationPhaseV1::Disposed))
                || !self.allocation_admission.has_expected_credit(
                    entry.id,
                    entry.record.device,
                    entry.record.byte_len,
                )
            {
                return Err(E::InvalidState);
            }
        }
        Ok(())
    }

    fn commit_disposal_group_v1(&mut self, key: ContextWriterReferenceV1) -> Result<(), E> {
        self.validate_disposed_group_v1(key)?;
        let versions = self.versions.as_mut().expect("configured journal");
        let group = versions
            .disposal_groups
            .get_mut(&group_id(key))
            .expect("validated group");
        group.committing = true;
        versions
            .journal
            .dispose_flat_unknown_group(&group.evidence())
            .map_err(|failure| match failure {
                ContextQueuedWriterGroupDisposalErrorV1::Rejected(error)
                | ContextQueuedWriterGroupDisposalErrorV1::Terminal(error) => error,
            })?;
        group.journal_disposed = true;
        for entry in &group.receipts {
            versions.phases[entry.member.allocation.slot] = None;
        }
        let count = group.receipts.len();
        for ordinal in 0..count {
            let id = self
                .versions
                .as_ref()
                .expect("configured journal")
                .disposal_groups[&group_id(key)]
                .receipts[ordinal]
                .id;
            self.dispose_allocation_credits_v1(id);
            self.versions
                .as_mut()
                .expect("configured journal")
                .disposal_groups
                .get_mut(&group_id(key))
                .expect("retained group")
                .refunded_count += 1;
        }
        let versions = self.versions.as_mut().expect("configured journal");
        let group = &versions.disposal_groups[&group_id(key)];
        for header in &group.writers {
            let id = group_id(header.writer);
            if let Some(record) = self.submissions.get_mut(&id) {
                record.journal_writer = None;
            }
            versions.submission_writers.remove(&id);
        }
        for entry in &group.receipts {
            versions.disposal_allocations.remove(&entry.id);
        }
        versions.disposal_groups.remove(&group_id(key));
        Ok(())
    }
}

use super::*;
use fe2o3_runtime_model::{
    ContextAllocationWriteV1, ContextWriterDisposalEvidenceV1, ContextWriterKeyV1,
    ContextWriterKindV1, ContextWriterNoEffectEvidenceV1, ContextWriterReferenceV1,
    ContextWriterStateV1, ContextWriterSuccessEvidenceV1,
};

// These tickets never escape Context. Dropping one does not abort its writer.
pub(in crate::context) struct HostWriteTicketV1 {
    id: RuntimeAllocationIdV1,
    writer: ContextWriterReferenceV1,
    member: ContextAllocationWriteV1,
}

enum HostWriteOutcomeV1 {
    Success,
    NoEffect,
    Unknown,
}

impl ContextVersionsV1 {
    pub(super) fn retained_writer(
        &self,
        writer: ContextWriterReferenceV1,
    ) -> Result<ContextWriterStateV1, ContextVersionJournalErrorV1> {
        let state = self.journal.lookup_writer(writer)?;
        match state {
            ContextWriterStateV1::Pending { member_count }
            | ContextWriterStateV1::Unknown { member_count }
                if member_count > 0 && member_count <= self.journal.allocation_capacity() =>
            {
                Ok(state)
            }
            _ => Err(ContextVersionJournalErrorV1::InvalidState),
        }
    }

    pub(in crate::context) fn retained_writers(&self) -> usize {
        self.journal.writer_capacity() - self.journal.remaining_writer_slots()
            + self
                .submission_writers
                .values()
                .filter(|root| root.journal_disposed)
                .count()
    }

    #[cfg(test)]
    pub(in crate::context) fn journal_for_test(&self) -> &ContextVersionJournalV1 {
        &self.journal
    }

    pub(super) fn whole_allocation(
        &self,
        id: RuntimeAllocationIdV1,
        record: &AllocationRecordV1,
    ) -> Result<ContextAllocationWriteV1, ContextVersionJournalErrorV1> {
        let allocation = self.validate_live(id, record)?;
        let projection = enrollment(id, record.device, record.byte_len);
        Ok(ContextAllocationWriteV1 {
            allocation,
            device: projection.device,
            byte_extent: projection.byte_extent,
        })
    }

    pub(super) fn retire_after_disposal(
        &mut self,
        plan: AllocationDisposalV1,
    ) -> Result<(), ContextVersionJournalErrorV1> {
        if self.whole_allocation(plan.id, &plan.record)? != plan.member {
            return Err(ContextVersionJournalErrorV1::InvalidAllocationReference);
        }
        match plan.kind {
            AllocationDisposalKindV1::Synchronous(writer) => {
                self.journal.dispose_unknown(
                    writer,
                    &ContextWriterDisposalEvidenceV1 {
                        writer,
                        allocations: &[plan.member],
                    },
                )?;
                self.phases[plan.member.allocation.slot] = None;
            }
            AllocationDisposalKindV1::Unwritten => self.retire(&[plan.member.allocation])?,
            AllocationDisposalKindV1::Submission { .. } => {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
        }
        Ok(())
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    #[cfg(test)]
    pub(in crate::context) fn retain_test_writer_v1(
        &mut self,
        ids: &[RuntimeAllocationIdV1],
        kind: ContextWriterKindV1,
    ) -> ContextWriterReferenceV1 {
        // Exercise unsupported model states with genuine Context identities;
        // this supplies no backend completion or disposal premise.
        let members: Vec<_> = ids
            .iter()
            .map(|id| {
                self.versions
                    .as_ref()
                    .unwrap()
                    .whole_allocation(*id, &self.allocations[id])
                    .unwrap()
            })
            .collect();
        let key = ContextWriterKeyV1 {
            context_generation: self.context_generation,
            local: self.next_id().unwrap(),
            kind,
        };
        let journal = &mut self.versions.as_mut().unwrap().journal;
        let writer = journal.register_writer(key).unwrap();
        journal.begin_write(writer, &members).unwrap();
        journal.mark_unknown(writer).unwrap();
        writer
    }

    pub(in crate::context) fn begin_journal_host_write_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
        record: &AllocationRecordV1,
    ) -> Result<Option<HostWriteTicketV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(versions) = context.versions.as_ref() else {
                return if record.journal.is_none() {
                    Ok(None)
                } else {
                    context.journal_result_v1(Err(ContextVersionJournalErrorV1::InvalidState))
                };
            };
            let result = versions.whole_allocation(id, record).and_then(|member| {
                versions
                    .journal
                    .lookup_allocation(member.allocation)
                    .map(|state| (member, state))
            });
            let (member, state) = context.journal_result_v1(result)?;
            if let Some(writer) = state.pending_writer {
                let result = context
                    .versions
                    .as_ref()
                    .expect("configured journal")
                    .retained_writer(writer);
                context.journal_result_v1(result)?;
                return Err(RuntimeValidationErrorV1::ContextReserved);
            }
            if state.attempt_epoch == u64::MAX
                || context
                    .versions
                    .as_ref()
                    .expect("configured journal")
                    .journal
                    .remaining_writer_slots()
                    == 0
            {
                return Err(RuntimeValidationErrorV1::Capacity);
            }
            let key = ContextWriterKeyV1 {
                context_generation: context.context_generation,
                local: context.next_id()?,
                kind: ContextWriterKindV1::Synchronous,
            };
            let result = context
                .versions
                .as_mut()
                .expect("configured journal")
                .journal
                .register_writer(key);
            let writer = context.journal_result_v1(result)?;
            let result = context
                .versions
                .as_mut()
                .expect("configured journal")
                .journal
                .begin_write(writer, &[member]);
            if result.is_err() {
                let abort = context
                    .versions
                    .as_mut()
                    .expect("configured journal")
                    .journal
                    .abort_reserved(writer);
                context.journal_result_v1(abort)?;
            }
            context.journal_result_v1(result)?;
            Ok(Some(HostWriteTicketV1 { id, writer, member }))
        })
    }

    // Secondary bookkeeping failure seals Context but must not replace an
    // original backend diagnostic or panic payload, including during Drop.
    fn finish_host_write_v1(
        &mut self,
        ticket: HostWriteTicketV1,
        outcome: HostWriteOutcomeV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        let settled = catch_unwind(AssertUnwindSafe(|| {
            let journal = &mut self.versions.as_mut().expect("configured journal").journal;
            if journal
                .lookup_allocation(ticket.member.allocation)?
                .pending_writer
                != Some(ticket.writer)
            {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
            match outcome {
                HostWriteOutcomeV1::Success => journal.settle_success(
                    ticket.writer,
                    &ContextWriterSuccessEvidenceV1 {
                        writer: ticket.writer,
                    },
                ),
                HostWriteOutcomeV1::NoEffect => journal.settle_no_effect(
                    ticket.writer,
                    &ContextWriterNoEffectEvidenceV1 {
                        writer: ticket.writer,
                    },
                ),
                HostWriteOutcomeV1::Unknown => journal.mark_unknown(ticket.writer),
            }
        }));
        let result = match settled {
            Ok(result) => self.journal_result_v1(result),
            Err(payload) => {
                self.quarantine_submission_writers_v1();
                core::mem::forget(payload);
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            }
        };
        if self.terminal
            && let Err(payload) = catch_unwind(AssertUnwindSafe(|| {
                self.allocation_admission.quarantine(ticket.id);
            }))
        {
            core::mem::forget(payload);
        }
        result
    }

    pub(in crate::context) fn write_with_journal_v1(
        &mut self,
        ticket: HostWriteTicketV1,
        record: &AllocationRecordV1,
        byte_offset: u64,
        bytes: &[u8],
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            self.backend
                .write_allocation_v1(record.backend_allocation, byte_offset, bytes)
        }));
        match outcome {
            Ok(result) => {
                if matches!(&result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                    self.quarantine_submission_writers_v1();
                }
                let disposition = match &result {
                    Ok(()) => HostWriteOutcomeV1::Success,
                    // A semantic SPI premise at this exact synchronous boundary,
                    // not transferable verifier, lease or reuse authority.
                    Err(RuntimeBackendFailureV1::Rejected(_)) => HostWriteOutcomeV1::NoEffect,
                    Err(_) => HostWriteOutcomeV1::Unknown,
                };
                let settlement = self.finish_host_write_v1(ticket, disposition);
                if result.is_ok() {
                    settlement?;
                }
                self.backend_result(result)
            }
            Err(payload) => {
                self.quarantine_submission_writers_v1();
                let _ = self.finish_host_write_v1(ticket, HostWriteOutcomeV1::Unknown);
                std::panic::resume_unwind(payload);
            }
        }
    }

    pub(in crate::context) fn prepare_journal_disposal_v1(
        &mut self,
        id: RuntimeAllocationIdV1,
        record: &AllocationRecordV1,
    ) -> Result<Option<AllocationDisposalV1>, RuntimeValidationErrorV1> {
        self.guard_journal_unwind_v1(|context| {
            let Some(versions) = context.versions.as_ref() else {
                return if record.journal.is_none() {
                    Ok(None)
                } else {
                    context.journal_result_v1(Err(ContextVersionJournalErrorV1::InvalidState))
                };
            };
            let result = versions.whole_allocation(id, record).and_then(|member| {
                let state = versions.journal.lookup_allocation(member.allocation)?;
                let writer = state
                    .pending_writer
                    .map(|writer| {
                        versions
                            .retained_writer(writer)
                            .map(|state| (writer, state))
                    })
                    .transpose()?;
                Ok((member, writer))
            });
            let (member, pending) = context.journal_result_v1(result)?;
            let versions = context.versions.as_ref().expect("configured journal");
            let kind = match pending {
                None => {
                    let result = versions.validate_retirement(&[member.allocation]);
                    context.journal_result_v1(result)?;
                    AllocationDisposalKindV1::Unwritten
                }
                Some((_, ContextWriterStateV1::Pending { .. })) => {
                    return Err(RuntimeValidationErrorV1::ContextReserved);
                }
                Some((writer, ContextWriterStateV1::Unknown { member_count: 1 }))
                    if writer.key.kind == ContextWriterKindV1::Synchronous =>
                {
                    let result = versions
                        .journal
                        .validate_unknown_disposal(writer, &[member]);
                    context.journal_result_v1(result)?;
                    AllocationDisposalKindV1::Synchronous(writer)
                }
                Some((writer, ContextWriterStateV1::Unknown { .. }))
                    if writer.key.kind == ContextWriterKindV1::Submission =>
                {
                    let result = context
                        .prepare_submission_allocation_disposal_v1(id, *record, member, writer);
                    return context.journal_result_v1(result).map(Some);
                }
                Some((_, ContextWriterStateV1::Unknown { .. })) => {
                    return Err(RuntimeValidationErrorV1::Unsupported);
                }
                Some((_, ContextWriterStateV1::Reserved)) => {
                    return context
                        .journal_result_v1(Err(ContextVersionJournalErrorV1::InvalidState));
                }
            };
            Ok(Some(AllocationDisposalV1 {
                id,
                record: *record,
                member,
                kind,
            }))
        })
    }
}

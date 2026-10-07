//! Striped SDMA publication and complete-roster retirement.

use super::*;

impl Gfx942SdmaQueueSetV1 {
    // Inline partial-publication custody is preallocated before the first publication.
    #[allow(clippy::result_large_err)]
    pub(crate) fn submit_striped_multi_queue_batch(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        requests: Vec<Gfx942SdmaCopyRequestV1>,
    ) -> Result<Gfx942SdmaMultiQueueSubmissionV1, MultiQueueSdmaSubmitFailureV1> {
        let (owners, next_owner) = match self {
            Self::Striped { owners, next_owner } => (owners, *next_owner),
            Self::LogicalMuxV2 {
                owners,
                next_logical_lane,
                ..
            } => (owners, usize::from(*next_logical_lane) % 2),
            Self::Generic(_) | Self::Directional(_) | Self::TerminalRetained { .. } => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                    MultiQueueSdmaPreparationFailureV1 {
                        error: Gfx942SdmaErrorV1::Contract(
                            "multi-queue submission requires a striped SDMA queue set",
                        ),
                        requests,
                    },
                ));
            }
        };
        let mut queue_ids = Vec::new();
        if queue_ids.try_reserve_exact(owners.len()).is_err() {
            return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                MultiQueueSdmaPreparationFailureV1 {
                    error: Gfx942SdmaErrorV1::Contract(
                        "multi-queue SDMA queue identity allocation",
                    ),
                    requests,
                },
            ));
        }
        queue_ids.extend(owners.iter().map(|owner| owner.queue_id));
        let plan = match Gfx942SdmaMultiQueuePlanV1::new(&queue_ids, requests.len(), next_owner) {
            Ok(plan) => plan,
            Err(error) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                    MultiQueueSdmaPreparationFailureV1 {
                        error: map_multi_queue_plan_error(error),
                        requests,
                    },
                ));
            }
        };
        let mut completion = PreparedMultiQueueCompletionV1 {
            ordered: Vec::new(),
            completed: Vec::new(),
        };
        if completion
            .ordered
            .try_reserve_exact(plan.request_count())
            .is_err()
            || completion
                .completed
                .try_reserve_exact(plan.request_count())
                .is_err()
        {
            return Err(MultiQueueSdmaSubmitFailureV1::Preparation(
                MultiQueueSdmaPreparationFailureV1 {
                    error: Gfx942SdmaErrorV1::Contract("multi-queue completion custody allocation"),
                    requests,
                },
            ));
        }
        let prepared = match prepare_multi_queue_batch(
            owners.len(),
            plan,
            requests,
            |queue, requests| owners[queue].prepare_batch_recoverable(memory, requests),
            PreparedSdmaBatchV1::into_requests,
        ) {
            Ok(prepared) => prepared,
            Err(failure) => {
                return Err(MultiQueueSdmaSubmitFailureV1::Preparation(failure));
            }
        };
        populate_prepared_striped_completion_roster(owners, &prepared, &mut completion);
        let published = publish_multi_queue_batch(
            prepared,
            |queue, batch| {
                let queue_id = batch.queue_id;
                match owners[queue].submit_prepared_batch_with_custody(memory, batch) {
                    Ok(tickets) => {
                        debug_assert!(tickets.iter().all(|ticket| ticket.queue_id == queue_id));
                        Ok((queue_id, tickets))
                    }
                    Err(failure) => Err((queue_id, failure)),
                }
            },
            PreparedSdmaBatchV1::into_requests,
            |queue_ordinal, queue_id, request_indices, tickets| {
                Gfx942SdmaMultiQueueShardTicketsV1 {
                    queue_ordinal: queue_ordinal as u16,
                    queue_id,
                    request_indices,
                    tickets,
                }
            },
            |request_index, request| Gfx942SdmaUnpublishedCopyRequestV1 {
                request_index,
                request,
            },
            |request| request.request_index,
            |plan, shards| Gfx942SdmaMultiQueueSubmissionV1 {
                plan,
                shards,
                completion,
            },
        );
        match published {
            Ok(submission) => {
                if let Err(error) = self.confirm_striped_multi_queue_completion(&submission) {
                    return Err(MultiQueueSdmaSubmitFailureV1::PublishedValidation {
                        error,
                        submission,
                    });
                }
                Ok(submission)
            }
            // Failure deliberately performs no cursor write.
            Err(failure) => Err(MultiQueueSdmaSubmitFailureV1::Publication(failure)),
        }
    }

    pub(crate) fn commit_striped_multi_queue_success(
        &mut self,
        plan: &Gfx942SdmaMultiQueuePlanV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let Self::Striped { owners, next_owner } = self else {
            return Err(Gfx942SdmaErrorV1::Contract(
                "multi-queue cursor commit requires striped SDMA queues",
            ));
        };
        if plan.first_queue() != *next_owner
            || plan.queue_ids().len() != owners.len()
            || !plan
                .queue_ids()
                .iter()
                .copied()
                .eq(owners.iter().map(|owner| owner.queue_id))
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "stale multi-queue cursor commit",
            ));
        }
        *next_owner = cursor_after_multi_queue_outcome(
            *next_owner,
            plan,
            MultiQueueCursorOutcomeV1::CompleteSuccess,
        )?;
        Ok(())
    }

    pub(super) fn confirm_striped_multi_queue_completion(
        &self,
        submission: &Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        let owners = self.multi_queue_owners_v1()?;
        if submission.plan.queue_ids().len() != owners.len()
            || !submission
                .plan
                .queue_ids()
                .iter()
                .copied()
                .eq(owners.iter().map(|owner| owner.queue_id))
            || submission.shards.len() != submission.plan.active_shard_count()
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped completion submission topology",
            ));
        }

        let request_count = submission.plan.request_count();
        if submission.completion.ordered.len() != request_count
            || !submission.completion.completed.is_empty()
            || submission.completion.ordered.capacity() < request_count
            || submission.completion.completed.capacity() < request_count
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "striped completion storage was not preallocated",
            ));
        }
        let mut seen_requests = [false; GFX942_SDMA_MAX_MULTI_QUEUE_REQUESTS_V1];
        let mut seen_slots = [0_u64; GFX942_SDMA_MAX_STRIPED_QUEUES_V1];
        let mut seen_shards = 0_u16;

        for shard in &submission.shards {
            let queue_ordinal = shard.queue_ordinal();
            let Some(owner) = owners.get(queue_ordinal) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped completion queue ordinal",
                ));
            };
            let Some(shard_bit) = 1_u16.checked_shl(queue_ordinal as u32) else {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped completion shard bound",
                ));
            };
            if seen_shards & shard_bit != 0
                || shard.queue_id != owner.queue_id
                || shard.request_indices.len() != shard.tickets.len()
                || submission.plan.shard_count(queue_ordinal) != Some(shard.tickets.len())
            {
                return Err(Gfx942SdmaErrorV1::Contract(
                    "striped completion shard identity",
                ));
            }
            seen_shards |= shard_bit;
            for (&request_index, &ticket) in shard.request_indices.iter().zip(shard.tickets.iter())
            {
                let request_index_usize = usize::from(request_index);
                if request_index_usize >= request_count
                    || seen_requests[request_index_usize]
                    || submission.plan.queue_for_request(request_index_usize) != Some(queue_ordinal)
                {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "striped completion request identity",
                    ));
                }
                let slot = owner.validate_ticket(ticket)?;
                let slot_bit = 1_u64 << slot;
                if seen_slots[queue_ordinal] & slot_bit != 0 {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "duplicate striped completion ticket",
                    ));
                }
                seen_requests[request_index_usize] = true;
                seen_slots[queue_ordinal] |= slot_bit;
                let expected = submission.completion.ordered.get(request_index_usize);
                if expected
                    != Some(&ValidatedMultiQueueCompletionEntryV1 {
                        request_index,
                        queue_ordinal: queue_ordinal as u16,
                        ticket,
                    })
                {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "published striped completion roster mismatch",
                    ));
                }
            }
        }
        if seen_requests[..request_count].iter().any(|seen| !seen) {
            return Err(Gfx942SdmaErrorV1::Contract(
                "incomplete striped completion custody",
            ));
        }
        Ok(())
    }

    pub(crate) fn observe_prepared_striped_multi_queue_completion(
        &mut self,
        memory: &mut SharedGttMemorySessionV1,
        submission: &Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        let owners = self.multi_queue_owners_mut_v1()?;
        observe_entire_completion_roster(&submission.completion.ordered, |entry| {
            let owner = owners.get_mut(usize::from(entry.queue_ordinal)).ok_or(
                Gfx942SdmaErrorV1::Contract("striped completion owner disappeared"),
            )?;
            let slot = owner.validate_ticket(entry.ticket)?;
            owner.observe_validated_slot_in_current_scope(memory, slot)
        })
    }

    // Returning the complete submission preserves all-or-nothing custody.
    #[allow(clippy::result_large_err)]
    pub(crate) fn retire_prepared_striped_multi_queue_completion(
        &mut self,
        submission: Gfx942SdmaMultiQueueSubmissionV1,
    ) -> Result<
        Gfx942SdmaMultiQueueCompletedV1,
        (Gfx942SdmaErrorV1, Gfx942SdmaMultiQueueSubmissionV1),
    > {
        let owners = match self.multi_queue_owners_mut_v1() {
            Ok(owners) => owners,
            Err(error) => return Err((error, submission)),
        };
        // This complete pass makes the subsequent custody moves infallible:
        // neither the queue roster nor its records can change between passes.
        if !entire_validated_completion_roster_remains_present(
            &submission.completion.ordered,
            |entry| {
                owners
                    .get(usize::from(entry.queue_ordinal))
                    .is_some_and(|owner| {
                        owner
                            .validate_ticket(entry.ticket)
                            .is_ok_and(|slot| owner.validated_slot_remains_present(slot))
                    })
            },
        ) {
            return Err((
                Gfx942SdmaErrorV1::Contract("striped retirement record disappeared"),
                submission,
            ));
        }
        let (plan, _shards, mut completion) = submission.into_parts();
        for entry in completion.ordered {
            let Some(owner) = owners.get_mut(usize::from(entry.queue_ordinal)) else {
                // The immutable preflight above checked this exact owner index.
                std::process::abort();
            };
            completion
                .completed
                .push(owner.retire_validated_slot(usize::from(entry.ticket.slot)));
        }
        Ok(Gfx942SdmaMultiQueueCompletedV1 {
            plan,
            completed: completion.completed,
        })
    }
}

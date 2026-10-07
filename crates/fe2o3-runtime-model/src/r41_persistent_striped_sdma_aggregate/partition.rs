use super::*;

pub(super) fn publication_partition_model_only(
    plan: &R41PersistentStripedPlanV1,
    roster: &[R41CompletionIdentityV1],
    script: R41PublicationScriptV1,
) -> Option<R41PublicationPartitionV1> {
    let queue_count = plan.queue_plan.striped_queue_count;
    let (confirmed, indeterminate) = match script {
        R41PublicationScriptV1::Full => (queue_count, false),
        R41PublicationScriptV1::StopsAfter {
            confirmed_shards,
            next_shard_indeterminate,
            ..
        } if confirmed_shards < queue_count => (confirmed_shards, next_shard_indeterminate),
        R41PublicationScriptV1::StopsAfter { .. } => return None,
    };
    let mut shards = Vec::with_capacity(usize::from(queue_count));
    for slot in 0..queue_count {
        let queue = plan.queue_plan.striped_queue_model_only(slot)?;
        let class = if slot < confirmed {
            R41ShardPublicationClassV1::Confirmed
        } else if slot == confirmed && indeterminate {
            R41ShardPublicationClassV1::Indeterminate
        } else {
            R41ShardPublicationClassV1::Untouched
        };
        let request_indices = roster
            .iter()
            .filter(|identity| identity.ticket.queue_slot == slot)
            .map(|identity| identity.ticket.request_index)
            .collect();
        shards.push(R41ShardPublicationV1 {
            queue_slot: slot,
            queue_id: queue.queue_id,
            class,
            request_indices,
        });
    }
    let partition = R41PublicationPartitionV1 { shards };
    partition
        .is_exact_for_model_only(plan, roster)
        .then_some(partition)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn terminal_from_requests_model_only(
    plan: Option<R41PersistentStripedPlanV1>,
    requests: Vec<R41PersistentStripedRequestV1>,
    roster: &[R41CompletionIdentityV1],
    partition: Option<R41PublicationPartitionV1>,
    stage: R41TerminalStageV1,
    reason: R41TerminalReasonV1,
    observation_count: usize,
    restored_count: usize,
    cursor_before: u8,
    resulting_cursor: u8,
    cursor_committed: bool,
    mut entries: Vec<R41TerminalEntryV1>,
) -> R41PersistentStripedTerminalV1 {
    debug_assert!(entries.is_empty());
    debug_assert!(entries.capacity() >= requests.len());
    for (index, request) in requests.into_iter().enumerate() {
        let ticket = roster.get(index).map(|identity| identity.ticket);
        let publication = ticket.and_then(|ticket| {
            partition.as_ref().and_then(|partition| {
                partition
                    .shards
                    .get(usize::from(ticket.queue_slot))
                    .map(|shard| shard.class)
            })
        });
        entries.push(R41TerminalEntryV1 {
            request,
            ticket,
            publication,
            restored_before_terminal: index < restored_count,
            owner_state: R41PersistentOwnerStateV1::Quarantined,
        });
    }
    R41PersistentStripedTerminalV1 {
        plan,
        entries,
        partition,
        stage,
        reason,
        observation_count,
        restored_count,
        cursor_before,
        resulting_cursor,
        cursor_committed,
    }
}

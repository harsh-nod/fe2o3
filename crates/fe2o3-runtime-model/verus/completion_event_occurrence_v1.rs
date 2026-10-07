verus! {
spec fn table<R>(s: State<R>) -> OwnerState<()> {
    OwnerState { queue: s.queue, signal_mapping: s.signal_mapping, gpu_base: s.gpu_base,
        next_batch_id: s.next_batch_id, slots: s.slots, dependency_ledger: (), phase: s.phase }
}
spec fn occurrence<const N: usize>(session: u64, epoch: u64, r: CompletionBatchRetentionV1<N>,
    i: int, packet: Option<u64>) -> ExactCompletionOccurrenceV1 {
    ExactCompletionOccurrenceV1 { session_occurrence: session, source_acceptance_epoch: epoch,
        batch_id: r.batch_id, queue: r.queue, signal_mapping: r.signal_mapping,
        slot: r.slots@[i], dispatch_generation: r.dispatches@[i].dispatch_generation, packet_id: packet }
}
fn exact_occurrence<const N: usize>(session_occurrence: u64, source_acceptance_epoch: u64,
    retention: &CompletionBatchRetentionV1<N>, batch_index: usize, packet_id: Option<u64>)
    -> (out: Result<ExactCompletionOccurrenceV1, Gfx942CompletionErrorV1>)
    ensures out == if batch_index < N {
        Ok(occurrence(session_occurrence, source_acceptance_epoch, *retention, batch_index as int, packet_id))
    } else { Err(Gfx942CompletionErrorV1::StaleBatchGeneration) },
{ completion_exact_occurrence_body!(verus_exec_expr, session_occurrence, source_acceptance_epoch, retention, batch_index, packet_id) }
}

// Shared executable binding and publication bodies, with no allocation contract.
#![allow(unused_macros)]
use vstd::prelude::*;
use std::collections::HashMap;

include!("completion_owner_schema_v1.rs");
include!("completion_bound_cancel_execution_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/event_release_body.rs");
include!("completion_event_core_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/event_bind_body.rs");
include!("completion_event_occurrence_v1.rs");
include!("../../fe2o3-kfd/src/queue_completion/source_publish_body.rs");

verus! {
struct Gfx942CompletionBatchV1<const N: usize> { retention: CompletionBatchRetentionV1<N> }
enum ComputeAqlQueueSessionErrorV1 { Completion(Gfx942CompletionErrorV1) }
enum FixedDispatchSubmissionFailureV1 {
    RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    RetryableBeforeSideEffect(ComputeAqlQueueSessionErrorV1),
    Terminal(ComputeAqlQueueSessionErrorV1),
}

spec fn published<D, const N: usize>(s: OwnerState<D>, r: CompletionBatchRetentionV1<N>) -> bool {
    r.last_packet_id.is_some() && valid(s, r, CompletionSlotPhaseV1::Published { batch_id: r.batch_id })
}
spec fn published_error<const N: usize>(r: CompletionBatchRetentionV1<N>) -> Gfx942CompletionErrorV1 {
    if r.last_packet_id.is_none() { Gfx942CompletionErrorV1::StaleBatchGeneration } else { retention_error(N) }
}
spec fn packet<const N: usize>(r: CompletionBatchRetentionV1<N>, i: int) -> Option<u64> {
    if 0 <= i < N && N <= u64::MAX && i <= u64::MAX && r.last_packet_id.is_some()
        && r.last_packet_id.unwrap() < u64::MAX && r.last_packet_id.unwrap() + 1 >= N {
        Some((r.last_packet_id.unwrap() + 1 - N + i) as u64)
    } else { None }
}
spec fn row_error<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>,
    event: Gfx942ComputeEventOccurrenceV1, i: int) -> Option<Gfx942CompletionErrorV1> {
    if event.exact.packet_id.is_some() { Some(Gfx942CompletionErrorV1::EventAlreadyBound) }
    else if !active(s, event) { Some(Gfx942CompletionErrorV1::StaleEventOccurrence) }
    else if packet(r, i).is_none() || i < 0 || i >= N { Some(Gfx942CompletionErrorV1::StaleBatchGeneration) }
    else if event.exact != occurrence(event.exact.session_occurrence, event.exact.source_acceptance_epoch, r, i, None) {
        Some(Gfx942CompletionErrorV1::StaleEventOccurrence)
    } else { None }
}
spec fn scan_error<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>,
    rows: Seq<Gfx942ComputeEventOccurrenceV1>, i: int) -> Option<Gfx942CompletionErrorV1>
    decreases rows.len() - i,
{
    if 0 <= i < rows.len() {
        match row_error(s, r, rows[i], i) { Some(error) => Some(error), None => scan_error(s, r, rows, i + 1) }
    } else { None }
}
spec fn binding_error<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>,
    rows: Seq<Gfx942ComputeEventOccurrenceV1>) -> Option<Gfx942CompletionErrorV1> {
    if s.phase != CompletionOwnerPhaseV1::Ready { Some(Gfx942CompletionErrorV1::Poisoned) }
    else if !published(table(s), r) { Some(published_error(r)) }
    else if rows.len() != N { Some(Gfx942CompletionErrorV1::StaleEventOccurrence) }
    else { scan_error(s, r, rows, 0) }
}
spec fn bound_row<const N: usize>(row: Gfx942ComputeEventOccurrenceV1,
    r: CompletionBatchRetentionV1<N>, i: int) -> Gfx942ComputeEventOccurrenceV1 {
    Gfx942ComputeEventOccurrenceV1 { exact: ExactCompletionOccurrenceV1 { packet_id: packet(r, i), ..row.exact }, ..row }
}
spec fn bound_rows<const N: usize>(rows: Seq<Gfx942ComputeEventOccurrenceV1>,
    r: CompletionBatchRetentionV1<N>, n: int) -> Seq<Gfx942ComputeEventOccurrenceV1> {
    Seq::new(rows.len(), |i: int| if i < n { bound_row(rows[i], r, i) } else { rows[i] })
}
spec fn bound_map<const N: usize>(map: Map<u64, ExactCompletionOccurrenceV1>,
    rows: Seq<Gfx942ComputeEventOccurrenceV1>, r: CompletionBatchRetentionV1<N>, n: int)
    -> Map<u64, ExactCompletionOccurrenceV1>
    decreases n,
{
    if n <= 0 { map }
    else { bound_map(map, rows, r, n - 1).insert(rows[n - 1].event_id, bound_row(rows[n - 1], r, n - 1).exact) }
}
spec fn bound_state<R, const N: usize>(s: State<R>, rows: Seq<Gfx942ComputeEventOccurrenceV1>,
    r: CompletionBatchRetentionV1<N>, n: int) -> State<R> {
    State { events: bound_map(s.events, rows, r, n), ..s }
}
spec fn marked_published<D, const N: usize>(s: OwnerState<D>, r: CompletionBatchRetentionV1<N>, n: int) -> OwnerState<D> {
    OwnerState { slots: Seq::new(s.slots.len(), |i: int| {
        if present(r.slots@, n, i as u32) {
            CompletionSlotRecordV1 { phase: CompletionSlotPhaseV1::Published { batch_id: r.batch_id }, ..s.slots[i] }
        } else { s.slots[i] }
    }), ..s }
}
spec fn marked_state<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>) -> State<R> {
    State { slots: marked_published(table(s), r, N as int).slots, ..s }
}
spec fn terminal(error: Gfx942CompletionErrorV1) -> FixedDispatchSubmissionFailureV1 {
    FixedDispatchSubmissionFailureV1::Terminal(ComputeAqlQueueSessionErrorV1::Completion(error))
}

proof fn unique_rows<R, const N: usize>(s: State<R>, r: CompletionBatchRetentionV1<N>, rows: Seq<Gfx942ComputeEventOccurrenceV1>)
    requires rows.len() == N, published(table(s), r),
        forall|i: int| 0 <= i < N ==> row_error(s, r, #[trigger] rows[i], i).is_none(),
    ensures forall|i: int, j: int| 0 <= i < j < N ==> rows[i].event_id != rows[j].event_id,
{
    assert forall|i: int, j: int| 0 <= i < j < N implies rows[i].event_id != rows[j].event_id by {
        assert(row_error(s, r, rows[i], i).is_none());
        assert(row_error(s, r, rows[j], j).is_none());
        assert(entry(table(s), r, CompletionSlotPhaseV1::Published { batch_id: r.batch_id }, j));
        if rows[i].event_id == rows[j].event_id {
            assert(rows[i].exact == rows[j].exact);
            assert(r.slots@[i].index == r.slots@[j].index);
        }
    }
}

proof fn untouched_key<const N: usize>(map: Map<u64, ExactCompletionOccurrenceV1>,
    rows: Seq<Gfx942ComputeEventOccurrenceV1>, r: CompletionBatchRetentionV1<N>, n: int, key: u64)
    requires 0 <= n <= rows.len(), forall|i: int| 0 <= i < n ==> #[trigger] rows[i].event_id != key,
    ensures bound_map(map, rows, r, n).contains_key(key) == map.contains_key(key),
        map.contains_key(key) ==> bound_map(map, rows, r, n)[key] == map[key],
    decreases n,
{
    if n > 0 { untouched_key(map, rows, r, n - 1, key); }
}

fn packet_id_at<const N: usize>(retention: &CompletionBatchRetentionV1<N>, batch_index: usize)
    -> (out: Result<u64, Gfx942CompletionErrorV1>)
    ensures out == match packet(*retention, batch_index as int) {
        Some(id) => Ok(id), None => Err(Gfx942CompletionErrorV1::StaleBatchGeneration) },
{ completion_packet_id_at_body!(verus_exec_expr, retention, batch_index, N) }

impl<D> CompletionSignalArenaOwnerV1<D> {
    fn mark_published_retaining<const N: usize>(&mut self, mut retention: CompletionBatchRetentionV1<N>, last_packet_id: u64)
        -> (out: Result<Gfx942CompletionBatchV1<N>, (Gfx942CompletionErrorV1, CompletionBatchRetentionV1<N>)>)
        ensures match out {
            Ok(batch) => bound(old(self).owner_state(), retention)
                && batch.retention == CompletionBatchRetentionV1 { last_packet_id: Some(last_packet_id), ..retention }
                && final(self).owner_state() == marked_published(old(self).owner_state(), retention, N as int),
            Err((error, returned)) => !bound(old(self).owner_state(), retention) && error == bound_error(retention)
                && returned == retention && final(self).owner_state() == old(self).owner_state(),
        },
    {
        completion_mark_published_retaining_body!(@annotated verus_exec_expr, self, retention, last_packet_id, N, i,
            [let ghost initial = self.owner_state();
             proof { assert(initial.slots =~= marked_published(initial, retention, 0).slots); }],
            [invariant 0 <= i <= N, bound(initial, retention), initial == old(self).owner_state(),
                self.owner_state() == marked_published(initial, retention, i as int),
             decreases N - i,],
            [proof { assert(self.owner_state().slots =~= marked_published(initial, retention, i as int + 1).slots); }])
    }
}

impl<R> CompletionSignalArenaOwnerV1<CompletionDependencyLedgerV1<R>> {
    fn validate_published<const N: usize>(&self, retention: &CompletionBatchRetentionV1<N>)
        -> (out: Result<(), Gfx942CompletionErrorV1>)
        ensures out == if published(table(self.state()), *retention) { Ok(()) } else { Err(published_error(*retention)) },
    { completion_validate_published_body!(verus_exec_expr, self, retention) }

    fn bind_compute_event_batch_after_publication<const N: usize>(&mut self,
        mut events: Vec<Gfx942ComputeEventOccurrenceV1>, batch: &Gfx942CompletionBatchV1<N>)
        -> (out: Result<Vec<Gfx942ComputeEventOccurrenceV1>, (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>)>)
        ensures match out {
            Ok(returned) => binding_error(old(self).state(), batch.retention, events@).is_none()
                && returned@ == bound_rows(events@, batch.retention, events.len() as int)
                && final(self).state() == bound_state(old(self).state(), events@, batch.retention, events.len() as int),
            Err((error, returned)) => binding_error(old(self).state(), batch.retention, events@) == Some(error)
                && returned == events && final(self).state() == old(self).state(),
        },
    {
        completion_bind_event_batch_body!(@annotated verus_exec_expr, self, events, batch, N, i,
            [let ghost s = self.state(); let ghost rows = events@; let ghost r = batch.retention;],
            [invariant 0 <= i <= events.len(), events@ == rows, rows.len() == N,
                self.state() == s, s == old(self).state(), s.phase == CompletionOwnerPhaseV1::Ready,
                published(table(s), r), batch.retention == r,
                scan_error(s, r, rows, 0) == scan_error(s, r, rows, i as int),
                forall|j: int| 0 <= j < i ==> row_error(s, r, #[trigger] rows[j], j).is_none(),
             decreases events.len() - i,],
            [proof { reveal_with_fuel(scan_error, 2); }],
            [proof { assert(row_error(s, r, rows[i as int], i as int).is_none()); }],
            [proof {
                unique_rows(s, r, rows);
                assert(bound_rows(rows, r, 0) =~= rows);
            }],
            [invariant 0 <= i <= events.len(), rows.len() == N, events.len() == N,
                batch.retention == r, s == old(self).state(),
                binding_error(s, r, rows).is_none(),
                forall|j: int| 0 <= j < N ==> row_error(s, r, #[trigger] rows[j], j).is_none(),
                forall|j: int, k: int| 0 <= j < k < N ==> rows[j].event_id != rows[k].event_id,
                events@ == bound_rows(rows, r, i as int), self.state() == bound_state(s, rows, r, i as int),
             decreases events.len() - i,],
            [proof {
                assert forall|j: int| 0 <= j < i implies #[trigger] rows[j].event_id != rows[i as int].event_id by {};
                untouched_key(s.events, rows, r, i as int, rows[i as int].event_id);
                assert(row_error(s, r, rows[i as int], i as int).is_none());
            }],
            [proof {
                assert(events@ =~= bound_rows(rows, r, i as int + 1));
                assert(self.state() == bound_state(s, rows, r, i as int + 1));
            }])
    }

    fn bind_dependency_event_batch_v1<const N: usize>(&mut self,
        events: Vec<Gfx942ComputeEventOccurrenceV1>, batch: &Gfx942CompletionBatchV1<N>)
        -> (out: Result<Vec<Gfx942ComputeEventOccurrenceV1>, (Gfx942CompletionErrorV1, Vec<Gfx942ComputeEventOccurrenceV1>)>)
        ensures match out {
            Ok(returned) => binding_error(old(self).state(), batch.retention, events@).is_none()
                && returned@ == bound_rows(events@, batch.retention, events.len() as int)
                && final(self).state() == bound_state(old(self).state(), events@, batch.retention, events.len() as int),
            Err((error, returned)) => binding_error(old(self).state(), batch.retention, events@) == Some(error)
                && returned == events && final(self).state() == old(self).state(),
        },
    { completion_bind_dependency_event_batch_body!(verus_exec_expr, self, events, batch) }

    fn finish_source_publication<const N: usize>(&mut self, retention: CompletionBatchRetentionV1<N>,
        last_packet_id: u64, events: Vec<Gfx942ComputeEventOccurrenceV1>)
        -> (out: Result<(Gfx942CompletionBatchV1<N>, Vec<Gfx942ComputeEventOccurrenceV1>), FixedDispatchSubmissionFailureV1>)
        ensures if !bound(table(old(self).state()), retention) {
            out == Err(terminal(bound_error(retention))) && final(self).state() == old(self).state()
        } else {
            let mid = marked_state(old(self).state(), retention);
            let published_retention = CompletionBatchRetentionV1 { last_packet_id: Some(last_packet_id), ..retention };
            match binding_error(mid, published_retention, events@) {
                Some(error) => out == Err(terminal(error)) && final(self).state() == mid,
                None => match out {
                    Ok((batch, returned)) => batch.retention == published_retention
                        && returned@ == bound_rows(events@, published_retention, N as int)
                        && final(self).state() == bound_state(mid, events@, published_retention, N as int),
                    _ => false,
                },
            }
        },
    {
        let ghost before = self.state(); let ghost r = retention;
        completion_source_publish_body!(@annotated verus_exec_expr, self, retention, last_packet_id, events, batch, failure,
            [-> (out: FixedDispatchSubmissionFailureV1) ensures out == terminal(failure.0),],
            [-> (out: FixedDispatchSubmissionFailureV1) ensures out == terminal(failure.0),],
            [proof { assert(self.state() == marked_state(before, r)); }])
    }
}
}

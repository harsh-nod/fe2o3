// Receipt/profile values and the remaining Active fields are arbitrary non-Copy
// payloads. Native authentication, clocks and the HostMetadataTable adapter are
// explicit boundaries, not new authority in this settlement proof.
include!("compute_pipeline_publication_v1.rs");

// Verus must prove the standard-library unreachable formatter is never reached.
// unreached has a false precondition; this does not assume away panic paths.
macro_rules! unreachable {
    ($($message:tt)*) => { return vstd::pervasive::unreached() };
}

include!("../../fe2o3-runtime/src/kfd_backend/ordered_publication_settlement_body.rs");

macro_rules! modeled_active_fields {
    ($active:expr) => { $active.payload };
}

verus! {

enum Attempt<R> { Unattempted, NativeOwned, Retryable, Published(R) }
struct OrderedPublicationV1<R, P> { profile: P, attempt: Attempt<R> }
enum MaterializedCompletionReceiptV1<R> { Published(R) }
enum ActiveComputeExecutionV1<R, P, S> {
    MaterializedSuccessorPublication(OrderedPublicationV1<R, P>),
    Materialized(MaterializedCompletionReceiptV1<R>),
    Other(S),
}
struct PublicationPerformance<S> { publication: u64, rest: S }
struct PublicationFields<R, P, S> {
    stream: u64,
    kernel: u64,
    dispatch_shape_sha256: [u8; 32],
    published_at: u64,
    performance: PublicationPerformance<S>,
    execution: Option<ActiveComputeExecutionV1<R, P, S>>,
    rest: S,
}
struct OrderedPublicationObservationV1<P> {
    id: u64, stream: u64, kernel: u64, shape: [u8; 32], profile: P,
}
enum OrderedPublicationSettlementErrorV1 { MissingIdentity, RetryStage, NoOutcome, ConfirmationStage }

struct RuntimeComputePipelineV1<T> {
    slots: Vec<RuntimeComputePipelineSlotV1<T>>,
    live: usize,
    next: Option<u64>,
    frontier: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>,
}
struct PipelineState<T> {
    slots: Seq<RuntimeComputePipelineSlotV1<T>>,
    live: usize,
    next: Option<u64>,
    frontier: Option<u64>,
    staged: Option<RuntimeComputePipelineIdentityV1>,
}

impl<T> View for RuntimeComputePipelineV1<T> {
    type V = PipelineState<T>;
    closed spec fn view(&self) -> PipelineState<T> {
        PipelineState { slots: self.slots@, live: self.live, next: self.next,
            frontier: self.frontier, staged: self.staged }
    }
}

impl<T> RuntimeComputePipelineV1<T> {
    fn entry_mut_v1(&mut self, id: RuntimeComputePipelineIdentityV1)
        -> (out: Option<&mut RuntimeComputePipelineEntryV1<T>>)
        ensures out.is_some() == exact(old(self)@.slots, id),
            final(self)@.live == old(self)@.live,
            final(self)@.next == old(self)@.next,
            final(self)@.frontier == old(self)@.frontier,
            final(self)@.staged == old(self)@.staged,
            match out {
                None => final(self)@.slots == old(self)@.slots,
                Some(entry) => old(self)@.slots[id.slot as int].entry == Some(*entry)
                    && final(self)@.slots == old(self)@.slots.update(id.slot as int,
                        RuntimeComputePipelineSlotV1 {
                            generation: old(self)@.slots[id.slot as int].generation,
                            entry: Some(*final(entry)),
                        }),
            },
    {
        exact_entry_mut(self.slots.as_mut_slice(), id)
    }

    fn confirm_publication_v1(&mut self, id: RuntimeComputePipelineIdentityV1)
        -> (out: Result<(), ()>)
        ensures out.is_ok() == intact(old(self)@.slots, old(self)@.live,
                old(self)@.next, old(self)@.frontier, old(self)@.staged, id),
            final(self)@.live == old(self)@.live,
            match out {
                Err(()) => final(self)@ == old(self)@,
                Ok(()) => final(self)@.slots == old(self)@.slots.update(id.slot as int,
                        published(old(self)@.slots[id.slot as int]))
                    && final(self)@.next == successor(Some(id.logical_epoch))
                    && final(self)@.staged.is_none()
                    && final(self)@.frontier == if old(self)@.frontier.is_none() {
                        Some(id.logical_epoch) } else { old(self)@.frontier },
            },
    {
        confirm(self.slots.as_mut_slice(), self.live, &mut self.next,
            &mut self.frontier, &mut self.staged, id)
    }

    fn withdraw_publication_v1(&mut self, id: RuntimeComputePipelineIdentityV1)
        -> (out: Option<ActiveSubmissionV1<T>>)
        ensures out.is_some() == intact(old(self)@.slots, old(self)@.live,
                old(self)@.next, old(self)@.frontier, old(self)@.staged, id),
            final(self)@.next == old(self)@.next,
            final(self)@.frontier == old(self)@.frontier,
            match out {
                None => final(self)@ == old(self)@,
                Some(active) => active == old(self)@.slots[id.slot as int].entry.unwrap().active
                    && final(self)@.live == old(self)@.live - 1
                    && final(self)@.staged.is_none()
                    && final(self)@.slots == old(self)@.slots.update(id.slot as int,
                        RuntimeComputePipelineSlotV1 {
                            generation: old(self)@.slots[id.slot as int].generation,
                            entry: None,
                        }),
            },
    {
        withdraw(self.slots.as_mut_slice(), &mut self.live, self.next,
            self.frontier, &mut self.staged, id)
    }
}

spec fn root<R, P, S>(fields: PublicationFields<R, P, S>) -> Option<OrderedPublicationV1<R, P>> {
    match fields.execution {
        Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(root)) => Some(root),
        _ => None,
    }
}

impl<R, P> OrderedPublicationV1<R, P> {
    fn indexed<S>(entry: &mut RuntimeComputePipelineEntryV1<PublicationFields<R, P, S>>)
        -> (out: &mut Self)
        requires root(old(entry).active.payload).is_some(),
        ensures *out == root(old(entry).active.payload).unwrap(),
            *final(entry) == (RuntimeComputePipelineEntryV1 {
                active: ActiveSubmissionV1 {
                    payload: PublicationFields {
                        execution: Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(*final(out))),
                        ..old(entry).active.payload
                    },
                    ..old(entry).active
                },
                ..*old(entry)
            }),
    {
        ordered_publication_indexed_body!(verus_exec_expr, modeled_active_fields, entry)
    }
}

spec fn timed_slot<R, P, S>(slot: RuntimeComputePipelineSlotV1<PublicationFields<R, P, S>>,
    duration: u64, time: Option<u64>) -> RuntimeComputePipelineSlotV1<PublicationFields<R, P, S>> {
    let entry = slot.entry.unwrap();
    let fields = entry.active.payload;
    RuntimeComputePipelineSlotV1 {
        entry: Some(RuntimeComputePipelineEntryV1 {
            active: ActiveSubmissionV1 {
                payload: PublicationFields {
                    performance: PublicationPerformance { publication: duration, ..fields.performance },
                    published_at: match time { Some(t) => t, None => fields.published_at },
                    ..fields
                },
                ..entry.active
            },
            ..entry
        }),
        ..slot
    }
}

spec fn timed<R, P, S>(state: PipelineState<PublicationFields<R, P, S>>,
    id: RuntimeComputePipelineIdentityV1, duration: u64, time: Option<u64>)
    -> PipelineState<PublicationFields<R, P, S>> {
    PipelineState { slots: state.slots.update(id.slot as int,
        timed_slot(state.slots[id.slot as int], duration, time)), ..state }
}

spec fn withdrawn<T>(state: PipelineState<T>, id: RuntimeComputePipelineIdentityV1) -> PipelineState<T> {
    PipelineState {
        slots: state.slots.update(id.slot as int, RuntimeComputePipelineSlotV1 {
            generation: state.slots[id.slot as int].generation, entry: None,
        }),
        live: (state.live - 1) as usize, staged: None, ..state
    }
}

spec fn installed_fields<R, P, S>(fields: PublicationFields<R, P, S>) -> PublicationFields<R, P, S> {
    match root(fields) {
        Some(publication) => match publication.attempt {
            Attempt::Published(batch) => PublicationFields {
                execution: Some(ActiveComputeExecutionV1::Materialized(
                    MaterializedCompletionReceiptV1::Published(batch))),
                ..fields
            },
            _ => fields,
        },
        _ => fields,
    }
}

spec fn installed<R, P, S>(state: PipelineState<PublicationFields<R, P, S>>,
    id: RuntimeComputePipelineIdentityV1) -> PipelineState<PublicationFields<R, P, S>> {
    let slot = state.slots[id.slot as int];
    let entry = slot.entry.unwrap();
    PipelineState {
        slots: state.slots.update(id.slot as int, RuntimeComputePipelineSlotV1 {
            entry: Some(RuntimeComputePipelineEntryV1 {
                phase: RuntimeComputePipelinePhaseV1::Published,
                active: ActiveSubmissionV1 { payload: installed_fields(entry.active.payload), ..entry.active },
                ..entry
            }),
            ..slot
        }),
        next: successor(Some(id.logical_epoch)), staged: None,
        frontier: if state.frontier.is_none() { Some(id.logical_epoch) } else { state.frontier },
        ..state
    }
}

spec fn settlement<R, P, S>(before: PipelineState<PublicationFields<R, P, S>>,
    after: PipelineState<PublicationFields<R, P, S>>, id: RuntimeComputePipelineIdentityV1,
    duration: u64, time: u64,
    out: Result<Option<OrderedPublicationObservationV1<P>>, OrderedPublicationSettlementErrorV1>) -> bool {
    if !exact(before.slots, id) {
        out == Err(OrderedPublicationSettlementErrorV1::MissingIdentity) && after == before
    } else {
        let active = before.slots[id.slot as int].entry.unwrap().active;
        let publication = root(active.payload).unwrap();
        let accepted = intact(before.slots, before.live, before.next, before.frontier, before.staged, id);
        match publication.attempt {
            Attempt::Unattempted | Attempt::NativeOwned =>
                out == Err(OrderedPublicationSettlementErrorV1::NoOutcome)
                    && after == timed(before, id, duration, None),
            Attempt::Retryable => if accepted {
                out == Ok(None) && after == withdrawn(before, id)
            } else {
                out == Err(OrderedPublicationSettlementErrorV1::RetryStage)
                    && after == timed(before, id, duration, None)
            },
            Attempt::Published(_) => if accepted {
                out == Ok(Some(OrderedPublicationObservationV1 {
                    id: active.id, stream: active.payload.stream, kernel: active.payload.kernel,
                    shape: active.payload.dispatch_shape_sha256, profile: publication.profile,
                })) && after == installed(timed(before, id, duration, Some(time)), id)
            } else {
                out == Err(OrderedPublicationSettlementErrorV1::ConfirmationStage)
                    && after == timed(before, id, duration, Some(time))
            },
        }
    }
}

proof fn payload_update_preserves_intact<T>(slots: Seq<RuntimeComputePipelineSlotV1<T>>,
    index: int, replacement: RuntimeComputePipelineSlotV1<T>, live: usize, next: Option<u64>,
    frontier: Option<u64>, staged: Option<RuntimeComputePipelineIdentityV1>, id: RuntimeComputePipelineIdentityV1)
    requires 0 <= index < slots.len(), slots[index].entry.is_some(), replacement.entry.is_some(),
        replacement.generation == slots[index].generation,
        replacement.entry.unwrap().identity == slots[index].entry.unwrap().identity,
        replacement.entry.unwrap().phase == slots[index].entry.unwrap().phase,
        replacement.entry.unwrap().active.id == slots[index].entry.unwrap().active.id,
    ensures intact(slots, live, next, frontier, staged, id)
        == intact(slots.update(index, replacement), live, next, frontier, staged, id),
{
    occupancy_update(slots, slots.len() as int, index, replacement);
    hits_update(slots, slots.len() as int, index, replacement, frontier);
    let updated = slots.update(index, replacement);
    assert forall|j: int| 0 <= j < slots.len() implies
        older_or_same(slots[j], id) == older_or_same(updated[j], id) by {
        if j != index { assert(updated[j] == slots[j]); }
    }
    assert(exact(slots, id) == exact(updated, id));
}

proof fn timing_preserves_intact<R, P, S>(state: PipelineState<PublicationFields<R, P, S>>,
    id: RuntimeComputePipelineIdentityV1, duration: u64, time: Option<u64>)
    requires exact(state.slots, id),
    ensures intact(state.slots, state.live, state.next, state.frontier, state.staged, id)
        == intact(timed(state, id, duration, time).slots, state.live, state.next, state.frontier, state.staged, id),
{
    payload_update_preserves_intact(state.slots, id.slot as int,
        timed_slot(state.slots[id.slot as int], duration, time),
        state.live, state.next, state.frontier, state.staged, id);
}

// Native result authentication is a constructor obligation. This raw body only
// needs exact lookup to select a root of the right Rust variant; metadata and
// every attempt phase remain unconstrained.
fn settle_returned<R, P, S>(pipeline: &mut RuntimeComputePipelineV1<PublicationFields<R, P, S>>,
    identity: RuntimeComputePipelineIdentityV1, duration: u64, time: u64)
    -> (out: Result<Option<OrderedPublicationObservationV1<P>>, OrderedPublicationSettlementErrorV1>)
    requires exact(old(pipeline)@.slots, identity) ==>
        root(old(pipeline)@.slots[identity.slot as int].entry.unwrap().active.payload).is_some(),
    ensures settlement(old(pipeline)@, final(pipeline)@, identity, duration, time, out),
{
    ordered_publication_settle_body!(@annotated verus_exec_expr, modeled_active_fields, pipeline, identity,
        duration, time, entry, publication, active, id,
        [let ghost before = pipeline@;], [],
        [proof {
            assert(pipeline@ == timed(before, identity, duration, None));
            timing_preserves_intact(before, identity, duration, None);
        }],
        [proof {
            assert(pipeline@.slots =~= withdrawn(before, identity).slots);
        }],
        [proof {
            assert(pipeline@.slots =~= timed(before, identity, duration, Some(time)).slots);
            assert(pipeline@ == timed(before, identity, duration, Some(time)));
            timing_preserves_intact(before, identity, duration, Some(time));
        }], [],
        [proof {
            assert(pipeline@.slots =~= installed(timed(before, identity, duration, Some(time)), identity).slots);
        }])
}

fn witness_pipeline<R, P, S>(publication: OrderedPublicationV1<R, P>, rest: S, performance_rest: S,
    neighbor: ActiveSubmissionV1<PublicationFields<R, P, S>>)
    -> (out: RuntimeComputePipelineV1<PublicationFields<R, P, S>>)
    ensures out@.slots.len() == 3, out@.live == 2, out@.next == Some(10), out@.frontier == Some(9),
        out@.staged == Some(RuntimeComputePipelineIdentityV1 {
            slot: 0, slot_generation: 7, logical_epoch: 10, submission: 17 }),
        intact(out@.slots, out@.live, out@.next, out@.frontier, out@.staged, out@.staged.unwrap()),
        !unique_roster(out@.slots),
        root(out@.slots[0].entry.unwrap().active.payload) == Some(publication),
        out@.slots[0].entry.unwrap().active.payload.rest == rest,
        out@.slots[0].entry.unwrap().active.payload.performance.rest == performance_rest,
        out@.slots[0].entry.unwrap().active.payload.published_at == 2,
        out@.slots[0].entry.unwrap().active.payload.performance.publication == 3,
        out@.slots[2].entry.unwrap().active == neighbor,
{
    let identity = RuntimeComputePipelineIdentityV1 {
        slot: 0, slot_generation: 7, logical_epoch: 10, submission: 17,
    };
    let mut slots = Vec::new();
    slots.push(RuntimeComputePipelineSlotV1 {
        generation: 7,
        entry: Some(RuntimeComputePipelineEntryV1 {
            identity, phase: RuntimeComputePipelinePhaseV1::Publishing,
            active: ActiveSubmissionV1 {
                id: 17,
                payload: PublicationFields {
                    stream: 23, kernel: 29, dispatch_shape_sha256: [31; 32], published_at: 2,
                    performance: PublicationPerformance { publication: 3, rest: performance_rest },
                    execution: Some(ActiveComputeExecutionV1::MaterializedSuccessorPublication(publication)),
                    rest,
                },
            },
        }),
    });
    slots.push(RuntimeComputePipelineSlotV1 { generation: 11, entry: None });
    slots.push(RuntimeComputePipelineSlotV1 {
        generation: 98,
        entry: Some(RuntimeComputePipelineEntryV1 {
            identity: RuntimeComputePipelineIdentityV1 {
                slot: u16::MAX, slot_generation: 0, logical_epoch: 9, submission: 0,
            },
            phase: RuntimeComputePipelinePhaseV1::Quarantined,
            active: neighbor,
        }),
    });
    proof {
        assert(occupied(slots@, 3) == 2) by { reveal_with_fuel(occupied, 4); }
        assert(hits(slots@, 3, Some(9)) == 1) by { reveal_with_fuel(hits, 4); }
        assert forall|j: int| 0 <= j < 3 implies older_or_same(slots@[j], identity) by {
            assert(j == 0 || j == 1 || j == 2);
        }
        assert(!valid_slot(slots@[2], 2));
    }
    RuntimeComputePipelineV1 { slots, live: 2, next: Some(10), frontier: Some(9), staged: Some(identity) }
}

fn publication_witness<R, P, S>(receipt: R, profile: P, rest: S, performance_rest: S,
    neighbor: ActiveSubmissionV1<PublicationFields<R, P, S>>)
{
    let mut pipeline = witness_pipeline(
        OrderedPublicationV1 { profile, attempt: Attempt::Published(receipt) }, rest, performance_rest, neighbor);
    let id = pipeline.staged.unwrap();
    let ghost before = pipeline@;
    let result = settle_returned(&mut pipeline, id, 41, 43);
    assert(match result {
        Ok(Some(observation)) => observation.profile == profile,
        _ => false,
    });
    assert(pipeline@ == installed(timed(before, id, 41, Some(43)), id));
    assert(pipeline@.slots[2] == before.slots[2]);
    assert(pipeline@.slots[0].entry.unwrap().active.payload.execution
        == Some(ActiveComputeExecutionV1::Materialized(MaterializedCompletionReceiptV1::Published(receipt))));
    assert(pipeline@.next == Some(11) && pipeline@.frontier == Some(9) && pipeline@.staged.is_none());
}

fn refusal_witness<R, P, S>(receipt: R, profile: P, rest: S, performance_rest: S,
    neighbor: ActiveSubmissionV1<PublicationFields<R, P, S>>)
{
    let mut pipeline = witness_pipeline(
        OrderedPublicationV1 { profile, attempt: Attempt::Published(receipt) }, rest, performance_rest, neighbor);
    let id = pipeline.staged.unwrap();
    pipeline.staged = None;
    let ghost before = pipeline@;
    let result = settle_returned(&mut pipeline, id, 41, 43);
    assert(result == Err(OrderedPublicationSettlementErrorV1::ConfirmationStage));
    assert(pipeline@ == timed(before, id, 41, Some(43)));
    assert(root(pipeline@.slots[0].entry.unwrap().active.payload)
        == Some(OrderedPublicationV1 { profile, attempt: Attempt::Published(receipt) }));
}

fn retry_witness<R, P, S>(profile: P, rest: S, performance_rest: S,
    neighbor: ActiveSubmissionV1<PublicationFields<R, P, S>>)
{
    let mut pipeline = witness_pipeline(
        OrderedPublicationV1 { profile, attempt: Attempt::Retryable }, rest, performance_rest, neighbor);
    let id = pipeline.staged.unwrap();
    let ghost before = pipeline@;
    let result = settle_returned(&mut pipeline, id, 41, 43);
    assert(result == Ok(None));
    assert(pipeline@ == withdrawn(before, id));
    assert(pipeline@.slots[0].generation == 7 && pipeline@.slots[0].entry.is_none());
    assert(pipeline@.slots[2] == before.slots[2]);
    assert(pipeline@.next == Some(10) && pipeline@.frontier == Some(9) && pipeline@.staged.is_none());
}

}

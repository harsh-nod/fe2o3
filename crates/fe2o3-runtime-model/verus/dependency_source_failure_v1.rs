// Normal returns of the actual post-binding source failure join and forwarding.
// The retained dispatch table projection and its trust boundary remain unchanged.
include!("dispatch_epoch_cancel_v1.rs");
include!("../../fe2o3-kfd/src/queue_dispatch_binding/cancel_binding_body.rs");
include!("../../fe2o3-kfd/src/queue_live/dependency_source_failure_body.rs");

verus! {
struct DispatchResourceOwnerV1<C, R> {
    generation: DispatchGenerationOwnerV1<C>, resources: R,
}
struct ResourceState<C, R> { generation: State<C>, resources: R }
struct ComputeAqlQueueSessionV1<C, R, S> {
    dispatch: Option<DispatchResourceOwnerV1<C, R>>, rest: S,
}
struct SessionState<C, R, S> { dispatch: Option<ResourceState<C, R>>, rest: S }
struct NativeDependencySourceRecipeV1;

// E preserves every other error payload without imposing Copy or Clone.
enum ComputeAqlQueueSessionErrorV1<E> { DispatchBinding(Gfx942DispatchBindingErrorV1), Other(E) }
enum FixedDispatchSubmissionFailureV1<E> {
    RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1<E>),
    RetryableBeforeSideEffect(ComputeAqlQueueSessionErrorV1<E>),
    Terminal(ComputeAqlQueueSessionErrorV1<E>),
}
struct CancellationTrace { calls: nat, result: Option<Result<(), Gfx942DispatchBindingErrorV1>> }

spec fn resource_cancelled<C, R>(s: ResourceState<C, R>, id: DispatchEpochIdentityV1) -> ResourceState<C, R> {
    ResourceState { generation: cancelled(s.generation, id), ..s }
}
spec fn session_cancelled<C, R, S>(s: SessionState<C, R, S>, id: DispatchEpochIdentityV1) -> SessionState<C, R, S> {
    SessionState { dispatch: Some(resource_cancelled(s.dispatch.unwrap(), id)), ..s }
}
spec fn cancel_result<C, R, S>(s: SessionState<C, R, S>, id: DispatchEpochIdentityV1)
    -> Result<(), Gfx942DispatchBindingErrorV1>
{
    if cancellable(s.dispatch.unwrap().generation, id) { Ok(()) }
    else { Err(refusal(s.dispatch.unwrap().generation)) }
}
spec fn normal_return<C, R, S, E>(s: SessionState<C, R, S>, failure: FixedDispatchSubmissionFailureV1<E>) -> bool {
    failure is RetryableBeforeSideEffect ==> s.dispatch.is_some()
}
spec fn settlement<C, R, S, E>(s: SessionState<C, R, S>, id: DispatchEpochIdentityV1,
    failure: FixedDispatchSubmissionFailureV1<E>, out: FixedDispatchSubmissionFailureV1<E>,
    trace: CancellationTrace, after: SessionState<C, R, S>) -> bool
{
    match failure {
        FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error) => {
            let result = cancel_result(s, id);
            &&& trace.calls == 1 && trace.result == Some(result)
            &&& if result.is_ok() {
                out == FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(error)
                    && after == session_cancelled(s, id)
            } else {
                out == FixedDispatchSubmissionFailureV1::Terminal(ComputeAqlQueueSessionErrorV1::DispatchBinding(
                    Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)) && after == s
            }
        },
        FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error)
        | FixedDispatchSubmissionFailureV1::Terminal(error) =>
            trace.calls == 0 && trace.result.is_none()
                && out == FixedDispatchSubmissionFailureV1::Terminal(error) && after == s,
    }
}

impl<C, R> DispatchResourceOwnerV1<C, R> {
    spec fn state(&self) -> ResourceState<C, R> {
        ResourceState { generation: self.generation.state(), resources: self.resources }
    }
    fn cancel_binding(&mut self, identity: DispatchEpochIdentityV1)
        -> (out: Result<(), Gfx942DispatchBindingErrorV1>)
        ensures
            out == if cancellable(old(self).state().generation, identity) { Ok(()) }
                else { Err(refusal(old(self).state().generation)) },
            final(self).state() == if out.is_ok() { resource_cancelled(old(self).state(), identity) }
                else { old(self).state() },
    { dispatch_cancel_binding_body!(verus_exec_expr, self, identity) }
}
impl<C, R, S> ComputeAqlQueueSessionV1<C, R, S> {
    spec fn state(&self) -> SessionState<C, R, S> {
        SessionState { dispatch: match self.dispatch { Some(owner) => Some(owner.state()), None => None }, rest: self.rest }
    }
}
impl NativeDependencySourceRecipeV1 {
    fn cancel<C, R, S>(&mut self, session: &mut ComputeAqlQueueSessionV1<C, R, S>, identity: DispatchEpochIdentityV1)
        -> (out: Result<(), Gfx942DispatchBindingErrorV1>)
        requires old(session).dispatch.is_some(),
        ensures
            out == cancel_result(old(session).state(), identity),
            final(session).state() == if out.is_ok() { session_cancelled(old(session).state(), identity) }
                else { old(session).state() },
    { native_dependency_source_cancel_body!(verus_exec_expr, session, identity) }
}

fn settle_source_failure<C, R, S, E>(session: &mut ComputeAqlQueueSessionV1<C, R, S>,
    recipe: &mut NativeDependencySourceRecipeV1, identity: DispatchEpochIdentityV1,
    failure: FixedDispatchSubmissionFailureV1<E>)
    -> (out: (FixedDispatchSubmissionFailureV1<E>, Ghost<CancellationTrace>))
    requires normal_return(old(session).state(), failure),
    ensures settlement(old(session).state(), identity, failure, out.0, out.1@, final(session).state()),
{
    let ghost mut trace = CancellationTrace { calls: 0, result: None };
    macro_rules! observed_cancel {
        ($recipe:ident, $session:ident, $identity:ident) => { verus_exec_expr!({
            let result = dependency_source_recipe_cancel_call!($recipe, $session, $identity);
            proof { trace.calls = trace.calls + 1; trace.result = Some(result); }
            result
        }) };
    }
    let out = dependency_source_failure_body!(@annotated verus_exec_expr, session, recipe, identity, failure, observed_cancel);
    (out, Ghost(trace))
}

fn absent_nonretry_witness<C, R, S, E>(rest: S, error: E, identity: DispatchEpochIdentityV1, terminal: bool) {
    let mut session = ComputeAqlQueueSessionV1::<C, R, S> { dispatch: None, rest };
    let failure = if terminal { FixedDispatchSubmissionFailureV1::Terminal(ComputeAqlQueueSessionErrorV1::Other(error)) }
        else { FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(ComputeAqlQueueSessionErrorV1::Other(error)) };
    let ghost before = session.state(); let ghost kept = failure;
    let (out, Ghost(trace)) = settle_source_failure(&mut session, &mut NativeDependencySourceRecipeV1, identity, failure);
    assert(trace.calls == 0 && trace.result.is_none());
    assert(session.state() == before);
    assert(out is Terminal);
    assert(match kept { FixedDispatchSubmissionFailureV1::Terminal(error)
        | FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(error) => out == FixedDispatchSubmissionFailureV1::Terminal(error),
        _ => false });
}

fn retained_retry_witness<C, R, S, E>(credits: C, resources: R, rest: S, error: E,
    queue: QueueKeyV1, roster: CompletionDispatchRosterV1, neighbor: DispatchEpochSlotV1, poison: bool, stale: bool)
{
    let mut slots = Vec::new(); slots.push(neighbor);
    slots.push(DispatchEpochSlotV1 { slot_generation: u64::MAX,
        phase: DispatchEpochPhaseV1::Reserved { dispatch_generation: 0, expected_roster: roster } });
    let generation = DispatchGenerationOwnerV1 {
        next_generation: u64::MAX, recipe_occurrence: 0, recipe_queue: Some(queue),
        capacity_profile: FixedDispatchCapacityProfileV1::Qualification1024, slots, credits,
        recycled_generation: Some(0), predecessor_detached_generation: Some(u64::MAX), poisoned: poison,
    };
    let mut session = ComputeAqlQueueSessionV1 { dispatch: Some(DispatchResourceOwnerV1 { generation, resources }), rest };
    let identity = DispatchEpochIdentityV1 { queue, recipe_occurrence: if stale { 1 } else { 0 },
        slot_index: 1, slot_generation: u64::MAX, dispatch_generation: 0 };
    let failure = FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(ComputeAqlQueueSessionErrorV1::Other(error));
    let ghost before = session.state(); let ghost kept = failure;
    let (out, Ghost(trace)) = settle_source_failure(&mut session, &mut NativeDependencySourceRecipeV1, identity, failure);
    assert(trace.calls == 1);
    if poison || stale {
        assert(out == FixedDispatchSubmissionFailureV1::Terminal(ComputeAqlQueueSessionErrorV1::DispatchBinding(
            Gfx942DispatchBindingErrorV1::StaleDispatchGeneration)));
        assert(session.state() == before);
        assert(trace.result == Some(Err(if poison { Gfx942DispatchBindingErrorV1::Poisoned }
            else { Gfx942DispatchBindingErrorV1::StaleDispatchGeneration })));
    } else {
        assert(out == kept);
        assert(trace.result == Some(Ok(())));
        assert(session.state() == session_cancelled(before, identity));
        assert(session.state().dispatch.unwrap().generation.slots[0] == neighbor);
        assert(session.state().dispatch.unwrap().generation.slots[1].slot_generation == u64::MAX);
        assert(session.state().dispatch.unwrap().generation.next_generation == u64::MAX);
    }
    assert(session.state().rest == before.rest);
    assert(session.state().dispatch.unwrap().resources == before.dispatch.unwrap().resources);
    assert(session.state().dispatch.unwrap().generation.credits == before.dispatch.unwrap().generation.credits);
}
}

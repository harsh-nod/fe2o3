// Mixed-profile finite-projection witness, not native publication or journal refinement.
verus! {

#[verifier::opaque]
pub open spec fn mixed_parent(table: CompletionTableV1, id: RuntimeSubmissionIdV1)
    -> RuntimeSubmissionIdV1 {
    table.node(id).dependencies@[0].submission
}

#[verifier::opaque]
pub open spec fn mixed_observation_ready(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(id);
    let peer = mixed_parent(table, id);
    &&& custody_valid(table, id)
    &&& !node.generated_owner
    &&& node.root_kind == 2
    &&& node.record.status == RuntimeCompletionStatusV1::Pending
    &&& node.state.unwrap().terminal == Some(BackendPollV1::Succeeded)
    &&& node.state.unwrap().cursor == 0
    &&& node.dependencies@.len() == 1
    &&& pending_roots_result(table, id, 2) == Ok(())
    &&& custody_valid(table, peer)
    &&& !table.node(peer).generated_owner
    &&& table.node(peer).root_kind == 1
    &&& table.node(peer).record.status == RuntimeCompletionStatusV1::Pending
    &&& table.node(peer).state.unwrap().terminal.is_none()
    &&& table.node(peer).dependencies@.len() == 0
    &&& pending_roots_result(table, peer, 1) == Ok(())
}

pub open spec fn mixed_observation_outcome(before: CompletionProjectionV1, after: CompletionProjectionV1,
    id: RuntimeSubmissionIdV1, result: Result<CompletionStepV1, RuntimeValidationErrorV1>, steps: usize) -> bool {
    let peer = mixed_parent(before.submissions, id);
    &&& after.submissions.nodes@ == before.submissions.nodes@
    &&& after.quarantined == before.quarantined
    &&& steps == 2
    &&& result == Ok(CompletionStepV1::Observe { id: peer,
        backend: before.submissions.node(peer).record.backend_submission })
}

#[verifier::spinoff_prover]
pub proof fn mixed_iteration_shape(initial: CompletionTableV1, table: CompletionTableV1, requested: RuntimeSubmissionIdV1,
    id: RuntimeSubmissionIdV1, iteration: int)
    requires initial.wf(), table.wf(), initial.nodes@ == table.nodes@,
        mixed_observation_ready(initial, requested), 0 <= iteration <= 1,
        id == if iteration == 0 { requested } else { mixed_parent(initial, requested) },
    ensures table.contains(id), custody_valid(table, id), !table.node(id).generated_owner,
        table.node(id).record.status == RuntimeCompletionStatusV1::Pending,
        table.node(id).state.is_some(),
        table.node(id).state.unwrap().terminal == if iteration == 0 { Some(BackendPollV1::Succeeded) } else { None },
        table.node(id).state.unwrap().cursor == 0,
        table.node(id).dependencies@.len() == if iteration == 0 { 1int } else { 0int },
        table.node(id).record.directed_peer_copy ==> pending_roots_result(table, id, 1) == Ok(()),
        table.node(id).record.producer_launch ==> pending_roots_result(table, id, 2) == Ok(()),
        mixed_parent(initial, requested) == mixed_parent(table, requested),
        table.node(id).record.backend_submission == initial.node(id).record.backend_submission,
        iteration == 0 ==> table.node(id).dependencies@[0].submission == mixed_parent(initial, requested),
        iteration == 0 ==> table.node(mixed_parent(table, requested)).record.status == RuntimeCompletionStatusV1::Pending,
{
    reveal(mixed_observation_ready);
    reveal(mixed_parent);
    assert(table_fixed(initial, table));
    fixed_lookup(initial, table);
    let peer = mixed_parent(initial, requested);
    assert(initial.contains(requested));
    assert(initial.contains(peer));
    assert(table.node(requested) == initial.node(requested));
    assert(table.node(peer) == initial.node(peer));
    assert(dependency_valid(initial, requested, 0));
    assert(dependency_valid(table, requested, 0));
    assert forall|j: int| 0 <= j < table.node(requested).dependencies@.len()
        implies dependency_valid(table, requested, j) by { assert(j == 0); }
    assert(custody_valid(table, requested));
    assert(custody_valid(table, peer));
    assert(mixed_observation_ready(table, requested));
}

pub fn construct_mixed_observation() -> (context: CompletionProjectionV1)
    ensures context.submissions.wf(), context.submissions.nodes@.len() == 2,
        mixed_observation_ready(context.submissions, RuntimeSubmissionIdV1 { context_generation: 7, local: 20 }),
        mixed_parent(context.submissions, RuntimeSubmissionIdV1 { context_generation: 7, local: 20 })
            == (RuntimeSubmissionIdV1 { context_generation: 7, local: 19 }),
        context.submissions.node(RuntimeSubmissionIdV1 { context_generation: 7, local: 19 }).record.backend_submission == 101,
        !context.quarantined,
{
    let peer = RuntimeSubmissionIdV1 { context_generation: 7, local: 19 };
    let launch = RuntimeSubmissionIdV1 { context_generation: 7, local: 20 };
    let mut nodes = Vec::new();
    nodes.push(CompletionNodeV1 {
        id: peer,
        record: CompletionRecordV1 { status: RuntimeCompletionStatusV1::Pending, backend_submission: 101,
            directed_peer_copy: true, producer_launch: false, quiescent: false },
        root_kind: 1, state: Some(DirectedPeerStateV1 { depth: 1, cursor: 0, terminal: None }),
        dependencies: Vec::new(), dependencies_held: true, generated_owner: false,
        peer_roots: Ok(()), producer_roots: Ok(()), input: Ok(None), settlement_failure: 0, settlement_prefix: 0,
    });
    let mut dependencies = Vec::new();
    dependencies.push(ScalarPeerDependencyV1 { submission: peer, backend_submission: 101 });
    nodes.push(CompletionNodeV1 {
        id: launch,
        record: CompletionRecordV1 { status: RuntimeCompletionStatusV1::Pending, backend_submission: 102,
            directed_peer_copy: false, producer_launch: true, quiescent: false },
        root_kind: 2, state: Some(DirectedPeerStateV1 { depth: 2, cursor: 0, terminal: Some(BackendPollV1::Succeeded) }),
        dependencies, dependencies_held: true, generated_owner: false,
        peer_roots: Ok(()), producer_roots: Ok(()), input: Ok(Some(ContextProducerReadStatusV1::Pending)),
        settlement_failure: 0, settlement_prefix: 0,
    });
    let table = CompletionTableV1 { nodes };
    proof {
        table.slot_unique(peer, 0);
        table.slot_unique(launch, 1);
        reveal(mixed_observation_ready);
        reveal(mixed_parent);
    }
    CompletionProjectionV1 {
        submissions: table, quarantined: false,
        ordinary_checked: None, custody_checked: None, peer_checked: None, producer_checked: None,
        unsafe_settlement_attempt: false, rejection: None,
    }
}

pub fn constructed_mixed_observation() -> (result: Result<CompletionStepV1, RuntimeValidationErrorV1>)
    ensures result == Ok(CompletionStepV1::Observe {
        id: RuntimeSubmissionIdV1 { context_generation: 7, local: 19 }, backend: 101 }),
{
    let mut context = construct_mixed_observation();
    let requested = RuntimeSubmissionIdV1 { context_generation: 7, local: 20 };
    let mut steps = 0usize;
    context.plan_completion_step_v1(requested, &mut steps)
}

}

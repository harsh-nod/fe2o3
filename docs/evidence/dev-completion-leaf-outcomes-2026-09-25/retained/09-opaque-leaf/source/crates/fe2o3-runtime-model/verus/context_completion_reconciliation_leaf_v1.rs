// Constructor witnesses for the finite projection, not real Context construction.
verus! {

pub open spec fn leaf_input(input: Option<ContextProducerReadStatusV1>) -> bool {
    input.is_none() || input == Some(ContextProducerReadStatusV1::Success)
        || input == Some(ContextProducerReadStatusV1::Unknown)
}

pub open spec fn leaf_shape(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(id);
    &&& custody_valid(table, id)
    &&& !node.generated_owner
    &&& node.record.status == RuntimeCompletionStatusV1::Pending
    &&& node.state.is_some()
    &&& node.state.unwrap().terminal == Some(BackendPollV1::Succeeded)
    &&& node.dependencies@.len() == 0
    &&& pending_roots_result(table, id, node.root_kind) == Ok(())
    &&& match node.input { Ok(input) => leaf_input(input), Err(_) => false }
}

#[verifier::opaque]
pub open spec fn eligible_leaf(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    leaf_shape(table, id)
}

pub open spec fn leaf_target(table: CompletionTableV1, id: RuntimeSubmissionIdV1)
    -> RuntimeCompletionStatusV1 {
    if table.node(id).input == Ok(Some(ContextProducerReadStatusV1::Unknown)) {
        RuntimeCompletionStatusV1::QuiescentWithoutResult
    } else { RuntimeCompletionStatusV1::Succeeded }
}

#[verifier::opaque]
pub open spec fn leaf_effect(before: CompletionTableV1, after: CompletionTableV1,
    id: RuntimeSubmissionIdV1) -> bool {
    let initial = before.node(id);
    let settled = after.node(id);
    &&& table_fixed(before, after)
    &&& settled.state == initial.state
    &&& forall|i: int| 0 <= i < before.nodes@.len() && i != before.slot(id)
        ==> after.nodes@[i] == before.nodes@[i]
    &&& settled.record.status == if initial.settlement_failure == 0 { leaf_target(before, id) }
        else { RuntimeCompletionStatusV1::Pending }
    &&& settled.dependencies_held == (initial.settlement_failure != 0)
    &&& settled.record.quiescent == (initial.settlement_failure == 0)
    &&& settled.settlement_prefix == if initial.settlement_failure == 0 || initial.settlement_failure > 4 { 4 }
        else { initial.settlement_failure }
}

#[verifier::spinoff_prover]
pub proof fn leaf_iteration_shape(initial: CompletionTableV1, current: CompletionTableV1,
    id: RuntimeSubmissionIdV1, iteration: int)
    requires initial.wf(), current.wf(), eligible_leaf(initial, id),
        0 <= iteration <= 1,
        if iteration == 0 { current.nodes@ == initial.nodes@ }
        else { initial.node(id).settlement_failure == 0 && leaf_effect(initial, current, id) },
    ensures leaf_shape(initial, id), current.contains(id), custody_valid(current, id),
        !current.node(id).generated_owner,
        current.node(id).state == initial.node(id).state,
        current.node(id).dependencies@.len() == 0,
        current.node(id).input == initial.node(id).input,
        current.node(id).settlement_failure == initial.node(id).settlement_failure,
        iteration == 0 ==> leaf_shape(current, id),
        iteration == 1 ==> current.node(id).record.status == leaf_target(initial, id),
{
    reveal(eligible_leaf);
    reveal(leaf_effect);
    if iteration == 1 { fixed_lookup(initial, current); }
}

#[verifier::spinoff_prover]
pub proof fn leaf_settlement_effect(initial: CompletionTableV1, current: CompletionTableV1,
    id: RuntimeSubmissionIdV1)
    requires initial.wf(), current.wf(), eligible_leaf(initial, id),
        table_fixed(initial, current), current.node(id).state == initial.node(id).state,
        forall|i: int| 0 <= i < initial.nodes@.len() && i != initial.slot(id)
            ==> current.nodes@[i] == initial.nodes@[i],
        current.node(id).record.status == if initial.node(id).settlement_failure == 0 { leaf_target(initial, id) }
            else { RuntimeCompletionStatusV1::Pending },
        current.node(id).dependencies_held == (initial.node(id).settlement_failure != 0),
        current.node(id).record.quiescent == (initial.node(id).settlement_failure == 0),
        current.node(id).settlement_prefix == if initial.node(id).settlement_failure == 0
            || initial.node(id).settlement_failure > 4 { 4 } else { initial.node(id).settlement_failure },
    ensures leaf_effect(initial, current, id),
{ reveal(leaf_effect); }

pub open spec fn leaf_outcome(before: CompletionProjectionV1, after: CompletionProjectionV1,
    id: RuntimeSubmissionIdV1, result: Result<CompletionStepV1, RuntimeValidationErrorV1>, steps: usize) -> bool {
    let failure = before.submissions.node(id).settlement_failure;
    &&& leaf_effect(before.submissions, after.submissions, id)
    &&& after.quarantined == (before.quarantined || failure != 0)
    &&& steps == if failure == 0 { 2usize } else { 1usize }
    &&& result == if failure == 0 { Ok(CompletionStepV1::Local(leaf_target(before.submissions, id))) }
        else { Err(RuntimeValidationErrorV1::Settlement(failure)) }
}

pub fn construct_leaf(producer: bool, input: Option<ContextProducerReadStatusV1>, failure: u8)
    -> (context: CompletionProjectionV1)
    requires leaf_input(input),
    ensures context.submissions.wf(),
        context.submissions.nodes@.len() == 1,
        eligible_leaf(context.submissions, RuntimeSubmissionIdV1 { context_generation: 7, local: 19 }),
        context.submissions.nodes@[0].id == (RuntimeSubmissionIdV1 { context_generation: 7, local: 19 }),
        context.submissions.nodes@[0].root_kind == if producer { 2u8 } else { 1u8 },
        context.submissions.nodes@[0].input == Ok(input),
        context.submissions.nodes@[0].settlement_failure == failure,
        context.submissions.nodes@[0].settlement_prefix == 0,
        !context.quarantined,
{
    let id = RuntimeSubmissionIdV1 { context_generation: 7, local: 19 };
    let mut nodes = Vec::new();
    nodes.push(CompletionNodeV1 {
        id,
        record: CompletionRecordV1 {
            status: RuntimeCompletionStatusV1::Pending, backend_submission: 101,
            directed_peer_copy: !producer, producer_launch: producer, quiescent: false,
        },
        root_kind: if producer { 2 } else { 1 },
        state: Some(DirectedPeerStateV1 { depth: 1, cursor: 0, terminal: Some(BackendPollV1::Succeeded) }),
        dependencies: Vec::new(), dependencies_held: true, generated_owner: false,
        peer_roots: Ok(()), producer_roots: Ok(()), input: Ok(input),
        settlement_failure: failure, settlement_prefix: 0,
    });
    let table = CompletionTableV1 { nodes };
    proof { table.slot_unique(id, 0); reveal(eligible_leaf); }
    CompletionProjectionV1 {
        submissions: table, quarantined: false,
        ordinary_checked: None, custody_checked: None, peer_checked: None, producer_checked: None,
        unsafe_settlement_attempt: false, rejection: None,
    }
}

pub fn constructed_leaf_outcome(producer: bool, input: Option<ContextProducerReadStatusV1>, failure: u8)
    -> (result: Result<CompletionStepV1, RuntimeValidationErrorV1>)
    requires leaf_input(input),
    ensures result == if failure != 0 { Err(RuntimeValidationErrorV1::Settlement(failure)) }
        else if input == Some(ContextProducerReadStatusV1::Unknown) {
            Ok(CompletionStepV1::Local(RuntimeCompletionStatusV1::QuiescentWithoutResult))
        } else { Ok(CompletionStepV1::Local(RuntimeCompletionStatusV1::Succeeded)) },
{
    let mut context = construct_leaf(producer, input, failure);
    let id = RuntimeSubmissionIdV1 { context_generation: 7, local: 19 };
    let ghost before = context;
    let mut steps = 0usize;
    let result = context.plan_completion_step_v1(id, &mut steps);
    assert(leaf_outcome(before, context, id, result, steps));
    result
}

}

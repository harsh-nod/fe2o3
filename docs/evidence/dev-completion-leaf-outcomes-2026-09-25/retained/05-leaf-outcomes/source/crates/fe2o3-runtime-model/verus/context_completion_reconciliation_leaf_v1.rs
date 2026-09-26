// Constructor witnesses for the finite projection, not real Context construction.
verus! {

pub open spec fn leaf_input(input: Option<ContextProducerReadStatusV1>) -> bool {
    input.is_none() || input == Some(ContextProducerReadStatusV1::Success)
        || input == Some(ContextProducerReadStatusV1::Unknown)
}

pub open spec fn eligible_leaf(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(id);
    &&& custody_valid(table, id)
    &&& !node.generated_owner
    &&& node.record.status == RuntimeCompletionStatusV1::Pending
    &&& node.state.is_some()
    &&& node.state.unwrap().terminal == Some(BackendPollV1::Succeeded)
    &&& node.dependencies@.len() == 0
    &&& pending_roots_result(table, id, node.root_kind) == Ok(())
    &&& node.input.is_ok() && leaf_input(node.input.unwrap())
}

pub open spec fn leaf_target(table: CompletionTableV1, id: RuntimeSubmissionIdV1)
    -> RuntimeCompletionStatusV1 {
    if table.node(id).input == Ok(Some(ContextProducerReadStatusV1::Unknown)) {
        RuntimeCompletionStatusV1::QuiescentWithoutResult
    } else { RuntimeCompletionStatusV1::Succeeded }
}

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
    proof { table.slot_unique(id, 0); }
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

verus! {

#[verifier::opaque]
pub open spec fn selected_edge(table: CompletionTableV1, parent: RuntimeSubmissionIdV1,
    child: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(parent);
    let producer = table.node(child);
    &&& table.contains(parent) && table.contains(child)
    &&& node.record.status == RuntimeCompletionStatusV1::Pending
    &&& node.state.is_some() && producer.state.is_some()
    &&& node.state.unwrap().terminal == Some(BackendPollV1::Succeeded)
    &&& node.state.unwrap().cursor < node.dependencies@.len()
    &&& node.dependencies@[node.state.unwrap().cursor as int].submission == child
    &&& node.dependencies@[node.state.unwrap().cursor as int].backend_submission
        == producer.record.backend_submission
    &&& node.root_kind == producer.root_kind
    &&& parent.context_generation == child.context_generation
    &&& child.local < parent.local
    &&& 0 < producer.state.unwrap().depth < node.state.unwrap().depth <= MAX_RUNTIME_DEPENDENCIES_V1
}

#[verifier::spinoff_prover]
pub proof fn selected_edge_preserved(before: CompletionTableV1, after: CompletionTableV1,
    parent: RuntimeSubmissionIdV1, child: RuntimeSubmissionIdV1)
    requires before.wf(), table_fixed(before, after), selected_edge(before, parent, child),
        before.node(parent) == after.node(parent),
    ensures selected_edge(after, parent, child),
{
    reveal(selected_edge);
    fixed_lookup(before, after);
}

// Only active ancestors are selected edges. The selected child may have become
// terminal while its parent waits for the next pop and cursor advance.
#[verifier::opaque]
pub open spec fn stack_path_valid(table: CompletionTableV1, path: Seq<RuntimeSubmissionIdV1>,
    length: int, id: RuntimeSubmissionIdV1, requested: RuntimeSubmissionIdV1) -> bool {
    &&& 0 <= length <= path.len() <= MAX_RUNTIME_DEPENDENCIES_V1
    &&& length == 0 ==> id == requested
    &&& length > 0 ==> path[0] == requested && table.contains(id)
        && table.node(id).state.is_some()
        && table.node(id).state.unwrap().depth + length <= MAX_RUNTIME_DEPENDENCIES_V1
    &&& forall|j: int| #![trigger path[j]] 0 <= j < length ==> selected_edge(table, path[j],
        if j + 1 < length { path[j + 1] } else { id })
        && table.contains(path[j]) && table.node(path[j]).state.is_some()
        && id.local < path[j].local
        && table.node(path[j]).state.unwrap().depth + j <= MAX_RUNTIME_DEPENDENCIES_V1
    &&& forall|i: int, j: int| #![trigger path[i], path[j]] 0 <= i < j < length ==> path[j].local < path[i].local
}

pub open spec fn selected_reachable(table: CompletionTableV1, requested: RuntimeSubmissionIdV1,
    id: RuntimeSubmissionIdV1) -> bool {
    exists|path: Seq<RuntimeSubmissionIdV1>, length: int|
        stack_path_valid(table, path, length, id, requested)
}

#[verifier::spinoff_prover]
pub proof fn path_empty(table: CompletionTableV1, path: Seq<RuntimeSubmissionIdV1>,
    requested: RuntimeSubmissionIdV1)
    requires path.len() <= MAX_RUNTIME_DEPENDENCIES_V1,
    ensures stack_path_valid(table, path, 0, requested, requested),
{ reveal(stack_path_valid); }

#[verifier::spinoff_prover]
pub proof fn path_pop(table: CompletionTableV1, path: Seq<RuntimeSubmissionIdV1>,
    length: int, id: RuntimeSubmissionIdV1, requested: RuntimeSubmissionIdV1)
    requires stack_path_valid(table, path, length, id, requested), length > 0,
    ensures stack_path_valid(table, path, length - 1, path[length - 1], requested),
{
    reveal(stack_path_valid);
    assert forall|j: int| #![trigger path[j]] 0 <= j < length - 1 implies selected_edge(table, path[j],
        if j + 1 < length - 1 { path[j + 1] } else { path[length - 1] })
        && table.contains(path[j]) && table.node(path[j]).state.is_some()
        && path[length - 1].local < path[j].local
        && table.node(path[j]).state.unwrap().depth + j <= MAX_RUNTIME_DEPENDENCIES_V1 by {
        assert(selected_edge(table, path[j], if j + 1 < length { path[j + 1] } else { id }));
    }
}

#[verifier::spinoff_prover]
pub proof fn path_descend(table: CompletionTableV1, path: Seq<RuntimeSubmissionIdV1>,
    length: int, id: RuntimeSubmissionIdV1, requested: RuntimeSubmissionIdV1,
    child: RuntimeSubmissionIdV1)
    requires stack_path_valid(table, path, length, id, requested), length < path.len(),
        selected_edge(table, id, child),
    ensures stack_path_valid(table, path.update(length, id), length + 1, child, requested),
{
    reveal(stack_path_valid);
    reveal_with_fuel(selected_edge, 1);
    let next = path.update(length, id);
    assert forall|j: int| #![trigger next[j]] 0 <= j < length + 1 implies selected_edge(table, next[j],
        if j + 1 < length + 1 { next[j + 1] } else { child })
        && table.contains(next[j]) && table.node(next[j]).state.is_some()
        && child.local < next[j].local
        && table.node(next[j]).state.unwrap().depth + j <= MAX_RUNTIME_DEPENDENCIES_V1 by {
        if j < length {
            assert(selected_edge(table, path[j], if j + 1 < length { path[j + 1] } else { id }));
        }
    }
    assert forall|i: int, j: int| #![trigger next[i], next[j]] 0 <= i < j < length + 1 implies next[j].local < next[i].local by {
        if j == length { assert(id.local < path[i].local); }
    }
}

#[verifier::spinoff_prover]
pub proof fn path_preserved(before: CompletionTableV1, after: CompletionTableV1,
    path: Seq<RuntimeSubmissionIdV1>, length: int, id: RuntimeSubmissionIdV1,
    requested: RuntimeSubmissionIdV1)
    requires before.wf(), before.contains(id), table_fixed(before, after),
        stack_path_valid(before, path, length, id, requested),
        forall|i: int| 0 <= i < before.nodes@.len() && i != before.slot(id)
            ==> before.nodes@[i] == after.nodes@[i],
    ensures stack_path_valid(after, path, length, id, requested),
{
    reveal(stack_path_valid);
    fixed_lookup(before, after);
    assert forall|j: int| #![trigger path[j]] 0 <= j < length implies selected_edge(after, path[j],
        if j + 1 < length { path[j + 1] } else { id })
        && after.contains(path[j]) && after.node(path[j]).state.is_some()
        && id.local < path[j].local
        && after.node(path[j]).state.unwrap().depth + j <= MAX_RUNTIME_DEPENDENCIES_V1 by {
        assert(selected_edge(before, path[j], if j + 1 < length { path[j + 1] } else { id }));
        assert(path[j] != id);
        assert(before.slot(path[j]) != before.slot(id));
        assert(after.node(path[j]) == before.node(path[j]));
        selected_edge_preserved(before, after, path[j], if j + 1 < length { path[j + 1] } else { id });
    }
}

}

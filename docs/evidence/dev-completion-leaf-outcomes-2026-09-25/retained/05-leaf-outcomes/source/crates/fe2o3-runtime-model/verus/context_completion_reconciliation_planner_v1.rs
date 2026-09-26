verus! {

pub open spec fn advanced_state(state: DirectedPeerStateV1) -> DirectedPeerStateV1 {
    DirectedPeerStateV1 { cursor: (state.cursor + 1) as usize, ..state }
}

pub proof fn cursor_advance_preserves(before: CompletionTableV1, after: CompletionTableV1,
    id: RuntimeSubmissionIdV1)
    requires before.wf(), custody_valid(before, id),
        before.node(id).record.status == RuntimeCompletionStatusV1::Pending,
        !before.node(id).record.quiescent, before.node(id).state.is_some(),
        before.node(id).state.unwrap().terminal == Some(BackendPollV1::Succeeded),
        before.node(id).state.unwrap().cursor < before.node(id).dependencies@.len(),
        before.node(before.node(id).dependencies@[before.node(id).state.unwrap().cursor as int].submission)
            .record.status == RuntimeCompletionStatusV1::Succeeded,
        after.nodes@.len() == before.nodes@.len(),
        forall|i: int| 0 <= i < before.nodes@.len() && i != before.slot(id)
            ==> after.nodes@[i] == before.nodes@[i],
        node_metadata_same(before.node(id), after.nodes@[before.slot(id)]),
        after.nodes@[before.slot(id)].state == Some(advanced_state(before.node(id).state.unwrap())),
    ensures after.wf(), custody_valid(after, id), progressed(before, after), table_fixed(before, after),
{
    reveal(progressed);
    assert(table_fixed(before, after));
    fixed_lookup(before, after);
    let cursor = before.node(id).state.unwrap().cursor;
    assert forall|j: int| 0 <= j < after.node(id).dependencies@.len() implies dependency_valid(after, id, j) by {
        assert(dependency_valid(before, id, j));
        let producer = before.node(id).dependencies@[j].submission;
        assert(producer != id);
        assert(before.slot(producer) != before.slot(id));
        assert(after.node(producer) == before.node(producer));
    }
    assert forall|i: int| 0 <= i < before.nodes@.len() implies node_progress(before, after, i) by {
        if i == before.slot(id) {
            assert forall|j: int| cursor <= j < cursor + 1 implies j < after.node(id).dependencies@.len()
                && after.contains(after.node(id).dependencies@[j].submission)
                && after.node(after.node(id).dependencies@[j].submission).record.status
                    == RuntimeCompletionStatusV1::Succeeded by {
                assert(j == cursor);
                assert(dependency_valid(before, id, j));
                let producer = before.node(id).dependencies@[j].submission;
                assert(producer != id);
                assert(before.slot(producer) != before.slot(id));
                assert(after.node(producer) == before.node(producer));
            }
        }
    }
}

pub proof fn checked_pending_shape(context: CompletionProjectionV1, id: RuntimeSubmissionIdV1)
    requires checked(context, id), context.submissions.node(id).record.status == RuntimeCompletionStatusV1::Pending,
        context.submissions.node(id).state.is_some(),
    ensures !context.submissions.node(id).record.quiescent,
        context.submissions.node(id).dependencies_held,
        forall|j: int| 0 <= j < context.submissions.node(id).dependencies@.len()
            ==> dependency_valid(context.submissions, id, j),
{}

pub proof fn exhausted_success_gate(context: CompletionProjectionV1, id: RuntimeSubmissionIdV1)
    requires checked(context, id), context.submissions.node(id).record.status == RuntimeCompletionStatusV1::Pending,
        context.submissions.node(id).state.is_some(),
        context.submissions.node(id).state.unwrap().terminal == Some(BackendPollV1::Succeeded),
        context.submissions.node(id).state.unwrap().cursor == context.submissions.node(id).dependencies@.len(),
    ensures context.submissions.node(id).input == Ok(None)
        || context.submissions.node(id).input == Ok(Some(ContextProducerReadStatusV1::Success))
            ==> success_ready(context.submissions, id),
{
    checked_pending_shape(context, id);
    assert forall|j: int| 0 <= j < context.submissions.node(id).dependencies@.len() implies
        context.submissions.contains(context.submissions.node(id).dependencies@[j].submission)
        && context.submissions.node(context.submissions.node(id).dependencies@[j].submission).record.status
            == RuntimeCompletionStatusV1::Succeeded by {
        assert(dependency_valid(context.submissions, id, j));
    }
}

#[verifier::spinoff_prover]
pub proof fn checked_selected_edge(context: CompletionProjectionV1, id: RuntimeSubmissionIdV1)
    requires checked(context, id), context.submissions.node(id).record.status == RuntimeCompletionStatusV1::Pending,
        context.submissions.node(id).state.is_some(),
        context.submissions.node(id).state.unwrap().terminal == Some(BackendPollV1::Succeeded),
        context.submissions.node(id).state.unwrap().cursor < context.submissions.node(id).dependencies@.len(),
    ensures selected_edge(context.submissions, id,
        context.submissions.node(id).dependencies@[context.submissions.node(id).state.unwrap().cursor as int].submission),
{
    reveal(selected_edge);
    checked_pending_shape(context, id);
    assert(dependency_valid(context.submissions, id, context.submissions.node(id).state.unwrap().cursor as int));
}

impl CompletionProjectionV1 {
    #[verifier::spinoff_prover]
    pub fn plan_completion_step_v1(&mut self, requested: RuntimeSubmissionIdV1, steps: &mut usize)
        -> (result: Result<CompletionStepV1, RuntimeValidationErrorV1>)
        requires old(self).submissions.wf(), *old(steps) == 0,
        ensures final(self).submissions.wf(),
            eligible_leaf(old(self).submissions, requested)
                ==> leaf_outcome(*old(self), *final(self), requested, result, *final(steps)),
            0 < *final(steps) <= 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1,
            result.is_ok() ==> final(self).rejection.is_none(),
            progressed(old(self).submissions, final(self).submissions),
            !final(self).unsafe_settlement_attempt,
            old(self).quarantined ==> final(self).quarantined,
            match result {
                Ok(CompletionStepV1::Local(status)) => final(self).submissions.contains(requested)
                    && status == final(self).submissions.node(requested).record.status
                    && (status == RuntimeCompletionStatusV1::Pending ==> *final(steps) == 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1),
                Ok(CompletionStepV1::Observe { id, backend }) => checked(*final(self), id)
                    && selected_reachable(final(self).submissions, requested, id)
                    && final(self).submissions.node(id).record.status == RuntimeCompletionStatusV1::Pending
                    && backend == final(self).submissions.node(id).record.backend_submission
                    && match final(self).submissions.node(id).state {
                        None => id == requested,
                        Some(state) => state.terminal.is_none(),
                    },
                Err(error) => final(self).rejection == Some(error)
                    || error == RuntimeValidationErrorV1::UnknownSubmission && !old(self).submissions.contains(requested),
            },
    {
        let ghost initial = self.submissions;
        proof {
            self.ordinary_checked = None;
            self.custody_checked = None;
            self.peer_checked = None;
            self.producer_checked = None;
            self.unsafe_settlement_attempt = false;
            self.rejection = None;
            progress_reflexive(initial);
        }
        completion_reconciliation_body!(@annotated verus_exec_expr, self, requested,
            path, length, id, validated, remaining, record, dependencies, state, dependency,
            [
                invariant
                    initial == old(self).submissions,
                    initial.wf(),
                    eligible_leaf(initial, requested) ==> id == requested && length == 0 && *steps <= 1
                        && self.quarantined == old(self).quarantined
                        && if *steps == 0 { self.submissions.nodes@ == initial.nodes@ }
                        else { initial.node(requested).settlement_failure == 0
                            && leaf_effect(initial, self.submissions, requested) },
                    self.submissions.wf(), progressed(initial, self.submissions),
                    !self.unsafe_settlement_attempt,
                    self.rejection.is_none(),
                    *steps + remaining == 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1,
                    old(self).quarantined ==> self.quarantined,
                    remaining <= 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1,
                    remaining == 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1 ==> length == 0,
                    remaining < 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1
                        ==> stack_path_valid(self.submissions, path@, length as int, id, requested),
                    length <= MAX_RUNTIME_DEPENDENCIES_V1,
                    length == 0 ==> id == requested,
                    length > 0 ==> path@[0] == requested,
                    forall|j: int| 0 <= j < length ==> self.submissions.contains(path@[j]),
                    remaining < 2 * MAX_RUNTIME_DEPENDENCIES_V1 + 1 ==> self.submissions.contains(requested),
                    self.submissions.contains(id) || id == requested && !initial.contains(requested),
                    validated.is_some() ==> validated == Some(id) && checked(*self, id),
                decreases remaining,
            ],
            [
                *steps += 1;
                let ghost before_step = self.submissions;
                let ghost before_path = path@;
                let ghost before_length = length;
                let ghost before_id = id;
                proof {
                    if *steps == 1 { path_empty(self.submissions, path@, requested); }
                }
            ],
            [proof {
                assert(checked(*self, id));
                assert(selected_reachable(self.submissions, requested, id));
            }],
            [proof { path_pop(self.submissions, before_path, before_length as int, before_id, requested); }],
            [proof {
                cursor_advance_preserves(before_step, self.submissions, id);
                fixed_lookup(before_step, self.submissions);
                path_preserved(before_step, self.submissions, path@, length as int, id, requested);
                progress_transitive(initial, before_step, self.submissions);
            }],
            [proof {
                path_descend(self.submissions, before_path, before_length as int, before_id, requested, id);
            }],
            [proof {
                fixed_lookup(before_step, self.submissions);
                path_preserved(before_step, self.submissions, path@, length as int, id, requested);
                progress_transitive(initial, before_step, self.submissions);
            }],
            [proof {
                checked_pending_shape(*self, id);
                assert(dependency_valid(self.submissions, id, state.cursor as int));
                checked_selected_edge(*self, id);
            }],
            [proof { exhausted_success_gate(*self, id); }]
        )
    }
}

}

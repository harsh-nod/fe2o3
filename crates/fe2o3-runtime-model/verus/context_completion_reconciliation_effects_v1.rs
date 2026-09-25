verus! {

pub open spec fn node_fixed(before: CompletionNodeV1, after: CompletionNodeV1) -> bool {
    &&& before.id == after.id
    &&& before.record.backend_submission == after.record.backend_submission
    &&& before.record.directed_peer_copy == after.record.directed_peer_copy
    &&& before.record.producer_launch == after.record.producer_launch
    &&& before.root_kind == after.root_kind
    &&& before.dependencies@ == after.dependencies@
    &&& before.generated_owner == after.generated_owner
    &&& before.peer_roots == after.peer_roots
    &&& before.producer_roots == after.producer_roots
    &&& before.input == after.input
    &&& before.settlement_failure == after.settlement_failure
    &&& match (before.state, after.state) {
        (None, None) => true,
        (Some(left), Some(right)) => left.depth == right.depth && left.terminal == right.terminal,
        _ => false,
    }
}

pub open spec fn table_fixed(before: CompletionTableV1, after: CompletionTableV1) -> bool {
    &&& before.nodes@.len() == after.nodes@.len()
    &&& forall|i: int| 0 <= i < before.nodes@.len() ==> node_fixed(before.nodes@[i], after.nodes@[i])
}

pub proof fn fixed_lookup(before: CompletionTableV1, after: CompletionTableV1)
    requires before.wf(), table_fixed(before, after),
    ensures after.wf(),
        forall|id: RuntimeSubmissionIdV1| before.contains(id) == after.contains(id),
        forall|id: RuntimeSubmissionIdV1| before.contains(id) ==> before.slot(id) == after.slot(id)
            && node_fixed(before.node(id), after.node(id)),
{
    assert forall|id: RuntimeSubmissionIdV1| before.contains(id) == after.contains(id) by {
        if before.contains(id) {
            let i = before.slot(id);
            assert(after.nodes@[i].id == id);
        }
        if after.contains(id) {
            let i = after.slot(id);
            assert(before.nodes@[i].id == id);
        }
    }
    assert forall|id: RuntimeSubmissionIdV1| before.contains(id) implies before.slot(id) == after.slot(id)
        && node_fixed(before.node(id), after.node(id)) by {
        if before.contains(id) {
            after.slot_unique(id, before.slot(id));
        }
    }
}

pub open spec fn success_ready(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(id);
    &&& table.contains(id)
    &&& node.state.is_some()
    &&& node.state.unwrap().terminal == Some(BackendPollV1::Succeeded)
    &&& node.state.unwrap().cursor == node.dependencies@.len()
    &&& forall|i: int| 0 <= i < node.dependencies@.len() ==> table.contains(node.dependencies@[i].submission)
        && table.node(node.dependencies@[i].submission).record.status == RuntimeCompletionStatusV1::Succeeded
    &&& node.input == Ok(None) || node.input == Ok(Some(ContextProducerReadStatusV1::Success))
}

pub open spec fn quiescent_ready(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(id);
    &&& table.contains(id)
    &&& node.state.is_some()
    &&& node.state.unwrap().terminal == Some(BackendPollV1::Succeeded)
    &&& if node.state.unwrap().cursor < node.dependencies@.len() {
        let dependency = node.dependencies@[node.state.unwrap().cursor as int].submission;
        table.contains(dependency)
            && table.node(dependency).record.status == RuntimeCompletionStatusV1::QuiescentWithoutResult
    } else {
        node.state.unwrap().cursor == node.dependencies@.len()
            && node.input == Ok(Some(ContextProducerReadStatusV1::Unknown))
    }
}

pub open spec fn settlement_ready(table: CompletionTableV1, id: RuntimeSubmissionIdV1,
    status: RuntimeCompletionStatusV1) -> bool {
    match status {
        RuntimeCompletionStatusV1::Succeeded => success_ready(table, id),
        RuntimeCompletionStatusV1::QuiescentWithoutResult => quiescent_ready(table, id),
        _ => false,
    }
}

// This relation records semantic progress, not rollback on failure. The opaque
// effect checkpoint is deliberately not interpreted as concrete journal state.
pub open spec fn node_progress(before: CompletionTableV1, after: CompletionTableV1, i: int) -> bool {
        let left = before.nodes@[i];
        let right = after.nodes@[i];
        &&& left.record.status != RuntimeCompletionStatusV1::Pending
            ==> right.record.status == left.record.status && right.state == left.state
        &&& right.record.status == RuntimeCompletionStatusV1::Succeeded
            && left.record.status != RuntimeCompletionStatusV1::Succeeded ==> success_ready(after, right.id)
        &&& right.record.status == RuntimeCompletionStatusV1::QuiescentWithoutResult
            && left.record.status != RuntimeCompletionStatusV1::QuiescentWithoutResult ==> quiescent_ready(after, right.id)
        &&& match (left.state, right.state) {
            (Some(a), Some(b)) => a.cursor <= b.cursor
                && (forall|j: int| a.cursor <= j < b.cursor ==> j < right.dependencies@.len()
                    && after.contains(right.dependencies@[j].submission)
                    && after.node(right.dependencies@[j].submission).record.status == RuntimeCompletionStatusV1::Succeeded),
            _ => true,
        }
}

#[verifier::opaque]
pub open spec fn progressed(before: CompletionTableV1, after: CompletionTableV1) -> bool {
    &&& table_fixed(before, after)
    &&& forall|i: int| 0 <= i < before.nodes@.len() ==> node_progress(before, after, i)
}

pub proof fn progress_reflexive(table: CompletionTableV1)
    ensures progressed(table, table),
{ reveal(progressed); }

pub proof fn progress_unchanged_view(before: CompletionTableV1, after: CompletionTableV1)
    requires before.nodes@ == after.nodes@,
    ensures progressed(before, after),
{ reveal(progressed); }

pub proof fn progress_transitive(first: CompletionTableV1, middle: CompletionTableV1,
    last: CompletionTableV1)
    requires first.wf(), progressed(first, middle), progressed(middle, last),
    ensures progressed(first, last),
{
    reveal(progressed);
    fixed_lookup(first, middle);
    fixed_lookup(middle, last);
    assert forall|i: int| 0 <= i < first.nodes@.len() implies node_progress(first, last, i) by {
        assert(node_progress(first, middle, i));
        assert(node_progress(middle, last, i));
        let left = first.nodes@[i];
        let mid = middle.nodes@[i];
        let right = last.nodes@[i];
        if mid.record.status == RuntimeCompletionStatusV1::QuiescentWithoutResult
            && left.record.status != RuntimeCompletionStatusV1::QuiescentWithoutResult {
            middle.slot_unique(mid.id, i);
            last.slot_unique(right.id, i);
            assert(quiescent_ready(middle, mid.id));
            if mid.state.unwrap().cursor < mid.dependencies@.len() {
                let id = mid.dependencies@[mid.state.unwrap().cursor as int].submission;
                let k = middle.slot(id);
                assert(node_progress(middle, last, k));
            }
        }
        if mid.record.status == RuntimeCompletionStatusV1::Succeeded
            && left.record.status != RuntimeCompletionStatusV1::Succeeded {
            middle.slot_unique(mid.id, i);
            last.slot_unique(right.id, i);
            assert(success_ready(middle, mid.id));
            assert forall|j: int| 0 <= j < right.dependencies@.len() implies
                last.contains(right.dependencies@[j].submission)
                && last.node(right.dependencies@[j].submission).record.status == RuntimeCompletionStatusV1::Succeeded by {
                let id = right.dependencies@[j].submission;
                let k = middle.slot(id);
                assert(middle.nodes@[k].record.status == RuntimeCompletionStatusV1::Succeeded);
                assert(node_progress(middle, last, k));
            }
        }
        if let (Some(a), Some(b), Some(c)) = (left.state, mid.state, right.state) {
            assert forall|j: int| a.cursor <= j < c.cursor implies j < right.dependencies@.len()
                && last.contains(right.dependencies@[j].submission)
                && last.node(right.dependencies@[j].submission).record.status == RuntimeCompletionStatusV1::Succeeded by {
                if j < b.cursor {
                    assert(j < mid.dependencies@.len());
                    let id = right.dependencies@[j].submission;
                    let k = middle.slot(id);
                    assert(middle.nodes@[k].record.status == RuntimeCompletionStatusV1::Succeeded);
                    assert(node_progress(middle, last, k));
                }
            }
        }
    }
}

pub proof fn single_settlement_progress(before: CompletionTableV1, after: CompletionTableV1,
    id: RuntimeSubmissionIdV1)
    requires before.wf(), before.contains(id), table_fixed(before, after),
        forall|i: int| 0 <= i < before.nodes@.len() && i != before.slot(id)
            ==> after.nodes@[i] == before.nodes@[i],
        after.nodes@[before.slot(id)].state == before.node(id).state,
        before.node(id).record.status != RuntimeCompletionStatusV1::Pending
            ==> after.nodes@[before.slot(id)].record.status == before.node(id).record.status,
        after.nodes@[before.slot(id)].record.status == RuntimeCompletionStatusV1::Succeeded
            && before.node(id).record.status != RuntimeCompletionStatusV1::Succeeded
            ==> success_ready(before, id),
        after.nodes@[before.slot(id)].record.status == RuntimeCompletionStatusV1::QuiescentWithoutResult
            && before.node(id).record.status != RuntimeCompletionStatusV1::QuiescentWithoutResult
            ==> quiescent_ready(before, id),
    ensures progressed(before, after),
{
    reveal(progressed);
    fixed_lookup(before, after);
    assert forall|i: int| 0 <= i < before.nodes@.len() implies node_progress(before, after, i) by {
        if i == before.slot(id) && before.node(id).record.status != RuntimeCompletionStatusV1::QuiescentWithoutResult
            && after.node(id).record.status == RuntimeCompletionStatusV1::QuiescentWithoutResult {
            if before.node(id).state.unwrap().cursor < before.node(id).dependencies@.len() {
                let producer = before.node(id).dependencies@[before.node(id).state.unwrap().cursor as int].submission;
                assert(producer != id);
                assert(before.slot(producer) != before.slot(id));
                assert(after.node(producer) == before.node(producer));
            }
        }
        if i == before.slot(id) && before.node(id).record.status != RuntimeCompletionStatusV1::Succeeded
            && after.node(id).record.status == RuntimeCompletionStatusV1::Succeeded {
            assert forall|j: int| 0 <= j < after.node(id).dependencies@.len() implies
                after.contains(after.node(id).dependencies@[j].submission)
                && after.node(after.node(id).dependencies@[j].submission).record.status
                    == RuntimeCompletionStatusV1::Succeeded by {
                let producer = after.node(id).dependencies@[j].submission;
                assert(before.node(producer).record.status == RuntimeCompletionStatusV1::Succeeded);
                assert(producer != id);
            }
        }
    }
}

impl CompletionProjectionV1 {
    pub fn producer_completion_parts_v1(&self, id: RuntimeSubmissionIdV1)
        -> (result: Option<(&[ScalarPeerDependencyV1], &DirectedPeerStateV1)>)
        requires self.submissions.wf(), self.submissions.contains(id),
        ensures match result {
            Some((dependencies, state)) => self.submissions.node(id).state == Some(*state)
                && dependencies@ == self.submissions.node(id).dependencies@,
            None => self.submissions.node(id).state.is_none(),
        },
    { self.submissions.parts(id) }

    pub fn producer_completion_state_mut_v1(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Option<&mut DirectedPeerStateV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures final(self).submissions.nodes@.len() == old(self).submissions.nodes@.len(),
            final(self).rejection == old(self).rejection,
            forall|i: int| 0 <= i < old(self).submissions.nodes@.len() && i != old(self).submissions.slot(id)
                ==> final(self).submissions.nodes@[i] == old(self).submissions.nodes@[i],
            node_metadata_same(old(self).submissions.node(id),
                final(self).submissions.nodes@[old(self).submissions.slot(id)]),
            final(self).ordinary_checked == old(self).ordinary_checked,
            final(self).custody_checked == old(self).custody_checked,
            final(self).peer_checked == old(self).peer_checked,
            final(self).producer_checked == old(self).producer_checked,
            final(self).quarantined == old(self).quarantined,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            match result {
                Some(state) => old(self).submissions.node(id).state == Some(*state)
                    && final(self).submissions.nodes@[old(self).submissions.slot(id)].state == Some(*final(state)),
                None => old(self).submissions.node(id).state.is_none()
                    && final(self).submissions.nodes@[old(self).submissions.slot(id)].state.is_none(),
            },
    { self.submissions.state_mut(id) }

    pub fn directed_input_status_v1(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Result<Option<ContextProducerReadStatusV1>, RuntimeValidationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures result == match old(self).submissions.node(id).input {
                Ok(value) => Ok(value),
                Err(_) => Err(RuntimeValidationErrorV1::InvalidBackendDescription),
            },
            final(self).submissions == old(self).submissions,
            final(self).ordinary_checked == old(self).ordinary_checked,
            final(self).custody_checked == old(self).custody_checked,
            final(self).peer_checked == old(self).peer_checked,
            final(self).producer_checked == old(self).producer_checked,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            final(self).quarantined == (old(self).quarantined || result.is_err()),
            final(self).rejection == match result { Ok(_) => old(self).rejection, Err(error) => Some(error) },
    {
        let index = self.submissions.find(id).unwrap();
        let result = self.submissions.nodes[index].input;
        self.journal_result_v1(result)
    }

    pub fn transition_submission_status(&mut self, id: RuntimeSubmissionIdV1, status: RuntimeCompletionStatusV1)
        -> (result: Result<RuntimeCompletionStatusV1, RuntimeValidationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures table_fixed(old(self).submissions, final(self).submissions),
            final(self).rejection == match result { Ok(_) => old(self).rejection, Err(error) => Some(error) },
            result == if old(self).submissions.node(id).record.status != RuntimeCompletionStatusV1::Pending
                || status == RuntimeCompletionStatusV1::Pending {
                Ok(old(self).submissions.node(id).record.status)
            } else if old(self).submissions.node(id).settlement_failure != 0 {
                Err(RuntimeValidationErrorV1::Settlement(old(self).submissions.node(id).settlement_failure))
            } else { Ok(status) },
            final(self).submissions.wf(),
            forall|i: int| 0 <= i < old(self).submissions.nodes@.len() && i != old(self).submissions.slot(id)
                ==> final(self).submissions.nodes@[i] == old(self).submissions.nodes@[i],
            final(self).submissions.node(id).state == old(self).submissions.node(id).state,
            final(self).submissions.node(id).settlement_prefix ==
                if old(self).submissions.node(id).record.status != RuntimeCompletionStatusV1::Pending
                    || status == RuntimeCompletionStatusV1::Pending { old(self).submissions.node(id).settlement_prefix }
                else if old(self).submissions.node(id).settlement_failure == 0
                    || old(self).submissions.node(id).settlement_failure > 4 { 4 }
                else { old(self).submissions.node(id).settlement_failure },
            final(self).submissions.node(id).record.status ==
                if old(self).submissions.node(id).record.status != RuntimeCompletionStatusV1::Pending
                    || status == RuntimeCompletionStatusV1::Pending || old(self).submissions.node(id).settlement_failure != 0 {
                    old(self).submissions.node(id).record.status
                } else { status },
            final(self).unsafe_settlement_attempt == (old(self).unsafe_settlement_attempt || !checked(*old(self), id)
                || !settlement_ready(old(self).submissions, id, status)),
            final(self).ordinary_checked.is_none(), final(self).custody_checked.is_none(),
            final(self).peer_checked.is_none(), final(self).producer_checked.is_none(),
            old(self).quarantined ==> final(self).quarantined,
            settlement_ready(old(self).submissions, id, status)
                ==> progressed(old(self).submissions, final(self).submissions),
    {
        let ghost before = self.submissions;
        proof {
            self.unsafe_settlement_attempt = self.unsafe_settlement_attempt || !checked(*self, id)
                || !settlement_ready(self.submissions, id, status);
            self.ordinary_checked = None;
            self.custody_checked = None;
            self.peer_checked = None;
            self.producer_checked = None;
        }
        let index = self.submissions.find(id).unwrap();
        let retained = self.submissions.nodes[index].record.status;
        if retained.is_terminal() || !status.is_terminal() {
            proof { progress_unchanged_view(before, self.submissions); }
            return Ok(retained);
        }
        let node = &mut self.submissions.nodes[index];
        // Failure may leave committed effects. This opaque checkpoint does not
        // stand in for the separate executable journal-effect correspondence.
        node.settlement_prefix = if node.settlement_failure == 0 { 4 }
            else if node.settlement_failure > 4 { 4 } else { node.settlement_failure };
        if node.settlement_failure != 0 {
            self.quarantined = true;
            let error = RuntimeValidationErrorV1::Settlement(node.settlement_failure);
            proof { self.rejection = Some(error); }
            proof { single_settlement_progress(before, self.submissions, id); }
            return Err(error);
        }
        node.dependencies_held = false;
        node.record.status = status;
        node.record.quiescent = true;
        proof {
            fixed_lookup(before, self.submissions);
            if settlement_ready(before, id, status) {
                single_settlement_progress(before, self.submissions, id);
            }
        }
        Ok(status)
    }
}

}

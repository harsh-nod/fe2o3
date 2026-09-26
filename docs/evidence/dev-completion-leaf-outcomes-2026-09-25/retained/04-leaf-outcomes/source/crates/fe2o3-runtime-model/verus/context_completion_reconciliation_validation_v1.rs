verus! {

impl vstd::std_specs::cmp::PartialEqSpecImpl for RuntimeCompletionStatusV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for BackendPollV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

pub open spec fn dependency_valid(table: CompletionTableV1, id: RuntimeSubmissionIdV1,
    index: int) -> bool {
    let node = table.node(id);
    let dependency = node.dependencies@[index];
    let producer = table.node(dependency.submission);
    &&& table.contains(dependency.submission)
    &&& dependency.submission.context_generation == id.context_generation
    &&& dependency.submission.local < id.local
    &&& dependency.backend_submission == producer.record.backend_submission
    &&& producer.root_kind == node.root_kind
    &&& producer.state.is_some()
    &&& 0 < producer.state.unwrap().depth < node.state.unwrap().depth
    &&& index < node.state.unwrap().cursor ==> producer.record.status == RuntimeCompletionStatusV1::Succeeded
}

pub open spec fn custody_valid(table: CompletionTableV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = table.node(id);
    &&& table.contains(id)
    &&& node.root_kind <= 2
    &&& node.record.directed_peer_copy == (node.root_kind == 1)
    &&& node.record.producer_launch == (node.root_kind == 2)
    &&& if node.root_kind == 0 {
        node.state.is_none()
    } else {
        &&& node.state.is_some()
        &&& node.dependencies@.len() <= MAX_RUNTIME_DEPENDENCIES_V1
        &&& 0 < node.state.unwrap().depth <= MAX_RUNTIME_DEPENDENCIES_V1
        &&& node.state.unwrap().cursor <= node.dependencies@.len()
        &&& node.state.unwrap().terminal != Some(BackendPollV1::Pending)
        &&& node.state.unwrap().terminal.is_none() ==> node.state.unwrap().cursor == 0
        &&& node.dependencies_held != node.record.quiescent
        &&& node.dependencies_held ==> forall|j: int| 0 <= j < node.dependencies@.len()
            ==> dependency_valid(table, id, j)
    }
}

pub open spec fn pending_roots_valid(table: CompletionTableV1, id: RuntimeSubmissionIdV1,
    kind: u8) -> bool {
    let node = table.node(id);
    &&& table.contains(id)
    &&& node.root_kind == kind
    &&& node.record.status == RuntimeCompletionStatusV1::Pending
    &&& !node.record.quiescent
    &&& if kind == 1 { node.peer_roots.is_ok() } else { node.producer_roots.is_ok() }
}

pub open spec fn pending_roots_result(table: CompletionTableV1, id: RuntimeSubmissionIdV1,
    kind: u8) -> Result<(), JournalObservationErrorV1> {
    let node = table.node(id);
    if node.root_kind != kind || node.record.status != RuntimeCompletionStatusV1::Pending
        || node.record.quiescent {
        Err(JournalObservationErrorV1 { code: 0 })
    } else if kind == 1 { node.peer_roots } else { node.producer_roots }
}

pub struct CompletionProjectionV1 {
    pub submissions: CompletionTableV1,
    pub quarantined: bool,
    pub ghost ordinary_checked: Option<RuntimeSubmissionIdV1>,
    pub ghost custody_checked: Option<RuntimeSubmissionIdV1>,
    pub ghost peer_checked: Option<RuntimeSubmissionIdV1>,
    pub ghost producer_checked: Option<RuntimeSubmissionIdV1>,
    pub ghost unsafe_settlement_attempt: bool,
    pub ghost rejection: Option<RuntimeValidationErrorV1>,
}

pub open spec fn checked(state: CompletionProjectionV1, id: RuntimeSubmissionIdV1) -> bool {
    let node = state.submissions.node(id);
    &&& state.ordinary_checked == Some(id)
    &&& state.custody_checked == Some(id)
    &&& !node.generated_owner
    &&& custody_valid(state.submissions, id)
    &&& node.record.status == RuntimeCompletionStatusV1::Pending && node.record.directed_peer_copy
        ==> state.peer_checked == Some(id) && pending_roots_valid(state.submissions, id, 1)
    &&& node.record.status == RuntimeCompletionStatusV1::Pending && node.record.producer_launch
        ==> state.producer_checked == Some(id) && pending_roots_valid(state.submissions, id, 2)
}

impl CompletionTableV1 {
    pub fn validate_custody(&self, id: RuntimeSubmissionIdV1)
        -> (result: Result<(), RuntimeValidationErrorV1>)
        requires self.wf(), self.contains(id),
        ensures result == if custody_valid(*self, id) { Ok(()) }
            else { Err(RuntimeValidationErrorV1::InvalidBackendDescription) },
    {
        let invalid = RuntimeValidationErrorV1::InvalidBackendDescription;
        let index = self.find(id).unwrap();
        let node = &self.nodes[index];
        if node.root_kind > 2 || node.record.directed_peer_copy != (node.root_kind == 1)
            || node.record.producer_launch != (node.root_kind == 2) {
            return Err(invalid);
        }
        if node.root_kind == 0 {
            return if node.state.is_none() { Ok(()) } else { Err(invalid) };
        }
        let state = match node.state { Some(state) => state, None => return Err(invalid) };
        if node.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 || state.depth == 0
            || state.depth > MAX_RUNTIME_DEPENDENCIES_V1 || state.cursor > node.dependencies.len()
            || state.terminal == Some(BackendPollV1::Pending)
            || state.terminal.is_none() && state.cursor != 0
            || node.dependencies_held == node.record.quiescent {
            return Err(invalid);
        }
        if !node.dependencies_held { return Ok(()); }
        let mut j = 0usize;
        while j < node.dependencies.len()
            invariant self.wf(), self.contains(id), index == self.slot(id),
                invalid == RuntimeValidationErrorV1::InvalidBackendDescription,
                node == &self.nodes@[index as int],
                0 <= j <= node.dependencies@.len(),
                node.root_kind > 0 && node.root_kind <= 2,
                node.record.directed_peer_copy == (node.root_kind == 1),
                node.record.producer_launch == (node.root_kind == 2),
                node.state == Some(state),
                node.dependencies@.len() <= MAX_RUNTIME_DEPENDENCIES_V1,
                0 < state.depth <= MAX_RUNTIME_DEPENDENCIES_V1,
                state.cursor <= node.dependencies@.len(),
                state.terminal != Some(BackendPollV1::Pending),
                state.terminal.is_none() ==> state.cursor == 0,
                node.dependencies_held != node.record.quiescent,
                node.dependencies_held,
                forall|k: int| 0 <= k < j ==> dependency_valid(*self, id, k),
            decreases node.dependencies.len() - j,
        {
            let dependency = node.dependencies[j];
            if dependency.submission.context_generation != id.context_generation
                || dependency.submission.local >= id.local {
                assert(!dependency_valid(*self, id, j as int));
                return Err(invalid);
            }
            let slot = match self.find(dependency.submission) {
                Some(slot) => slot,
                None => {
                    assert(!dependency_valid(*self, id, j as int));
                    return Err(invalid);
                },
            };
            let producer = &self.nodes[slot];
            if producer.record.backend_submission != dependency.backend_submission
                || producer.root_kind != node.root_kind {
                assert(!dependency_valid(*self, id, j as int));
                return Err(invalid);
            }
            let parent = match producer.state {
                Some(state) => state,
                None => {
                    assert(!dependency_valid(*self, id, j as int));
                    return Err(invalid);
                },
            };
            if parent.depth == 0 || parent.depth >= state.depth
                || j < state.cursor && producer.record.status != RuntimeCompletionStatusV1::Succeeded {
                assert(!dependency_valid(*self, id, j as int));
                return Err(invalid);
            }
            assert(dependency_valid(*self, id, j as int));
            j += 1;
        }
        Ok(())
    }
}

impl CompletionProjectionV1 {
    pub fn require_ordinary_submission_v1(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Result<(), RuntimeValidationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures final(self).submissions == old(self).submissions,
            final(self).quarantined == old(self).quarantined,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            final(self).rejection == match result { Ok(()) => old(self).rejection, Err(error) => Some(error) },
            result == if old(self).submissions.node(id).generated_owner {
                Err(RuntimeValidationErrorV1::ContextReserved)
            } else { Ok(()) },
            result.is_ok() ==> !final(self).submissions.node(id).generated_owner
                && final(self).ordinary_checked == Some(id),
    {
        let index = self.submissions.find(id).unwrap();
        if self.submissions.nodes[index].generated_owner {
            proof { self.rejection = Some(RuntimeValidationErrorV1::ContextReserved); }
            Err(RuntimeValidationErrorV1::ContextReserved)
        } else {
            proof { self.ordinary_checked = Some(id); }
            Ok(())
        }
    }

    pub fn check_operation_custody_v1(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Result<(), RuntimeValidationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures final(self).submissions == old(self).submissions,
            final(self).quarantined == (old(self).quarantined || result.is_err()),
            final(self).ordinary_checked == old(self).ordinary_checked,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            final(self).rejection == match result { Ok(()) => old(self).rejection, Err(error) => Some(error) },
            result == if custody_valid(old(self).submissions, id) { Ok(()) }
                else { Err(RuntimeValidationErrorV1::InvalidBackendDescription) },
            result.is_ok() ==> custody_valid(final(self).submissions, id)
                && final(self).custody_checked == Some(id),
    {
        let result = self.submissions.validate_custody(id);
        match result {
            Ok(()) => { proof { self.custody_checked = Some(id); } },
            Err(error) => {
                self.quarantined = true;
                proof { self.rejection = Some(error); }
            },
        }
        result
    }

    pub fn validate_pending_roots(&mut self, id: RuntimeSubmissionIdV1, kind: u8)
        -> (result: Result<(), JournalObservationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id), kind == 1 || kind == 2,
        ensures result == pending_roots_result(old(self).submissions, id, kind),
            final(self).submissions == old(self).submissions,
            final(self).rejection == old(self).rejection,
            final(self).quarantined == old(self).quarantined,
            final(self).ordinary_checked == old(self).ordinary_checked,
            final(self).custody_checked == old(self).custody_checked,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            kind == 1 ==> final(self).producer_checked == old(self).producer_checked,
            kind == 2 ==> final(self).peer_checked == old(self).peer_checked,
            result.is_ok() ==> pending_roots_valid(final(self).submissions, id, kind)
                && if kind == 1 { final(self).peer_checked == Some(id) }
                    else { final(self).producer_checked == Some(id) },
    {
        let index = self.submissions.find(id).unwrap();
        let node = &self.submissions.nodes[index];
        if node.root_kind != kind || node.record.status != RuntimeCompletionStatusV1::Pending
            || node.record.quiescent {
            return Err(JournalObservationErrorV1 { code: 0 });
        }
        let result = if kind == 1 { node.peer_roots } else { node.producer_roots };
        if result.is_ok() {
            proof {
                if kind == 1 { self.peer_checked = Some(id); }
                else { self.producer_checked = Some(id); }
            }
        }
        result
    }

    pub fn validate_pending_peer_copy_roots_v1(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Result<(), JournalObservationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures result == pending_roots_result(old(self).submissions, id, 1),
            final(self).submissions == old(self).submissions,
            final(self).rejection == old(self).rejection,
            final(self).quarantined == old(self).quarantined,
            final(self).ordinary_checked == old(self).ordinary_checked,
            final(self).custody_checked == old(self).custody_checked,
            final(self).producer_checked == old(self).producer_checked,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            result.is_ok() ==> pending_roots_valid(final(self).submissions, id, 1)
                && final(self).peer_checked == Some(id),
    { self.validate_pending_roots(id, 1) }

    pub fn validate_pending_producer_launch_roots_v1(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Result<(), JournalObservationErrorV1>)
        requires old(self).submissions.wf(), old(self).submissions.contains(id),
        ensures result == pending_roots_result(old(self).submissions, id, 2),
            final(self).submissions == old(self).submissions,
            final(self).rejection == old(self).rejection,
            final(self).quarantined == old(self).quarantined,
            final(self).ordinary_checked == old(self).ordinary_checked,
            final(self).custody_checked == old(self).custody_checked,
            final(self).peer_checked == old(self).peer_checked,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            result.is_ok() ==> pending_roots_valid(final(self).submissions, id, 2)
                && final(self).producer_checked == Some(id),
    { self.validate_pending_roots(id, 2) }

    pub fn journal_result_v1<T>(&mut self, result: Result<T, JournalObservationErrorV1>)
        -> (returned: Result<T, RuntimeValidationErrorV1>)
        ensures returned == match result {
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
            final(self).rejection == if result.is_err() { Some(RuntimeValidationErrorV1::InvalidBackendDescription) }
                else { old(self).rejection },
    {
        match result {
            Ok(value) => Ok(value),
            Err(_) => {
                self.quarantined = true;
                proof { self.rejection = Some(RuntimeValidationErrorV1::InvalidBackendDescription); }
                Err(RuntimeValidationErrorV1::InvalidBackendDescription)
            },
        }
    }

    pub fn invalid_directed_v1<T>(&mut self) -> (result: Result<T, RuntimeValidationErrorV1>)
        ensures result == Err(RuntimeValidationErrorV1::InvalidBackendDescription),
            final(self).submissions == old(self).submissions,
            final(self).unsafe_settlement_attempt == old(self).unsafe_settlement_attempt,
            final(self).quarantined,
            final(self).rejection == Some(RuntimeValidationErrorV1::InvalidBackendDescription),
    {
        self.quarantined = true;
        proof { self.rejection = Some(RuntimeValidationErrorV1::InvalidBackendDescription); }
        Err(RuntimeValidationErrorV1::InvalidBackendDescription)
    }
}

}

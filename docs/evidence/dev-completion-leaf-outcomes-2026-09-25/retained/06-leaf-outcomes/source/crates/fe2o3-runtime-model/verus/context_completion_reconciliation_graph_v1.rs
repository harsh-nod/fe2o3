// Executable finite storage projection. This does not model HashMap allocation
// or claim correspondence with the complete Context custody/journal adapters.
verus! {

pub const MAX_RUNTIME_DEPENDENCIES_V1: usize = 256;
pub const MAX_RUNTIME_SUBMISSIONS_V1: usize = 1_048_576;

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSubmissionIdV1 {
    pub context_generation: u64,
    pub local: u64,
}

impl vstd::std_specs::cmp::PartialEqSpecImpl for RuntimeSubmissionIdV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeCompletionStatusV1 {
    Pending,
    Succeeded,
    QuiescentWithoutResult,
    Failed(u64),
}

impl RuntimeCompletionStatusV1 {
    pub fn is_terminal(&self) -> (result: bool)
        ensures result == (*self != Self::Pending),
    { !matches!(self, Self::Pending) }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BackendPollV1 {
    Pending,
    Succeeded,
    QuiescentWithoutResult,
    Failed { code: u64 },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ContextProducerReadStatusV1 { Pending, Success, NoEffect, Unknown }

#[derive(Clone, Copy)]
pub struct JournalObservationErrorV1 { pub code: u64 }

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeValidationErrorV1 {
    UnknownSubmission,
    InvalidBackendDescription,
    ContextReserved,
    Settlement(u8),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CompletionStepV1 {
    Observe { id: RuntimeSubmissionIdV1, backend: u64 },
    Local(RuntimeCompletionStatusV1),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CompletionRecordV1 {
    pub status: RuntimeCompletionStatusV1,
    pub backend_submission: u64,
    pub directed_peer_copy: bool,
    pub producer_launch: bool,
    pub quiescent: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DirectedPeerStateV1 {
    pub depth: usize,
    pub cursor: usize,
    pub terminal: Option<BackendPollV1>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ScalarPeerDependencyV1 {
    pub submission: RuntimeSubmissionIdV1,
    pub backend_submission: u64,
}

pub struct CompletionNodeV1 {
    pub id: RuntimeSubmissionIdV1,
    pub record: CompletionRecordV1,
    // 0 is an ordinary non-directed record, 1 a directed peer, 2 a producer.
    // Other values are representable and must be rejected when visited.
    pub root_kind: u8,
    pub state: Option<DirectedPeerStateV1>,
    pub dependencies: Vec<ScalarPeerDependencyV1>,
    pub dependencies_held: bool,
    pub generated_owner: bool,
    pub peer_roots: Result<(), JournalObservationErrorV1>,
    pub producer_roots: Result<(), JournalObservationErrorV1>,
    pub input: Result<Option<ContextProducerReadStatusV1>, JournalObservationErrorV1>,
    pub settlement_failure: u8,
    pub settlement_prefix: u8,
}

pub struct CompletionTableV1 {
    pub nodes: Vec<CompletionNodeV1>,
}

impl CompletionTableV1 {
    pub open spec fn wf(&self) -> bool {
        &&& self.nodes@.len() <= MAX_RUNTIME_SUBMISSIONS_V1
        &&& forall|i: int, j: int| 0 <= i < self.nodes@.len() && 0 <= j < self.nodes@.len()
            && self.nodes@[i].id == self.nodes@[j].id ==> i == j
    }

    pub open spec fn contains(&self, id: RuntimeSubmissionIdV1) -> bool {
        exists|i: int| 0 <= i < self.nodes@.len() && self.nodes@[i].id == id
    }

    pub open spec fn slot(&self, id: RuntimeSubmissionIdV1) -> int {
        choose|i: int| 0 <= i < self.nodes@.len() && self.nodes@[i].id == id
    }

    pub open spec fn node(&self, id: RuntimeSubmissionIdV1) -> CompletionNodeV1 {
        self.nodes@[self.slot(id)]
    }

    pub proof fn slot_unique(&self, id: RuntimeSubmissionIdV1, index: int)
        requires self.wf(), 0 <= index < self.nodes@.len(), self.nodes@[index].id == id,
        ensures self.contains(id), self.slot(id) == index,
    {}

    pub fn find(&self, id: RuntimeSubmissionIdV1) -> (result: Option<usize>)
        requires self.wf(),
        ensures match result {
            Some(index) => 0 <= index < self.nodes@.len()
                && self.nodes@[index as int].id == id && self.slot(id) == index,
            None => !self.contains(id),
        },
    {
        let mut index = 0usize;
        while index < self.nodes.len()
            invariant self.wf(), index <= self.nodes@.len(),
                forall|i: int| 0 <= i < index ==> self.nodes@[i].id != id,
            decreases self.nodes.len() - index,
        {
            if self.nodes[index].id == id {
                proof { self.slot_unique(id, index as int); }
                return Some(index);
            }
            index += 1;
        }
        None
    }

    pub fn get(&self, id: &RuntimeSubmissionIdV1) -> (result: Option<&CompletionRecordV1>)
        requires self.wf(),
        ensures match result {
            Some(record) => self.contains(*id) && *record == self.node(*id).record,
            None => !self.contains(*id),
        },
    {
        match self.find(*id) {
            Some(index) => Some(&self.nodes[index].record),
            None => None,
        }
    }

    pub fn parts(&self, id: RuntimeSubmissionIdV1)
        -> (result: Option<(&[ScalarPeerDependencyV1], &DirectedPeerStateV1)>)
        requires self.wf(), self.contains(id),
        ensures match result {
            Some((dependencies, state)) => self.node(id).state == Some(*state)
                && dependencies@ == self.node(id).dependencies@,
            None => self.node(id).state.is_none(),
        },
    {
        let index = self.find(id).unwrap();
        let node = &self.nodes[index];
        match &node.state {
            Some(state) => Some((node.dependencies.as_slice(), state)),
            None => None,
        }
    }

    pub fn state_mut(&mut self, id: RuntimeSubmissionIdV1)
        -> (result: Option<&mut DirectedPeerStateV1>)
        requires old(self).wf(), old(self).contains(id),
        ensures final(self).nodes@.len() == old(self).nodes@.len(),
            forall|i: int| 0 <= i < old(self).nodes@.len() && i != old(self).slot(id)
                ==> final(self).nodes@[i] == old(self).nodes@[i],
            node_metadata_same(old(self).node(id), final(self).nodes@[old(self).slot(id)]),
            match result {
                Some(state) => old(self).node(id).state == Some(*state)
                    && final(self).nodes@[old(self).slot(id)].state == Some(*final(state)),
                None => old(self).node(id).state.is_none()
                    && final(self).nodes@[old(self).slot(id)].state.is_none(),
            },
    {
        let index = self.find(id).unwrap();
        self.nodes[index].state.as_mut()
    }
}

pub open spec fn node_metadata_same(before: CompletionNodeV1, after: CompletionNodeV1) -> bool {
    &&& before.id == after.id
    &&& before.record == after.record
    &&& before.root_kind == after.root_kind
    &&& before.dependencies@ == after.dependencies@
    &&& before.dependencies_held == after.dependencies_held
    &&& before.generated_owner == after.generated_owner
    &&& before.peer_roots == after.peer_roots
    &&& before.producer_roots == after.producer_roots
    &&& before.input == after.input
    &&& before.settlement_failure == after.settlement_failure
    &&& before.settlement_prefix == after.settlement_prefix
}

impl IndexSpecImpl<&RuntimeSubmissionIdV1> for CompletionTableV1 {
    open spec fn index_req(&self, id: &&RuntimeSubmissionIdV1) -> bool {
        self.wf() && self.contains(**id)
    }
}

impl core::ops::Index<&RuntimeSubmissionIdV1> for CompletionTableV1 {
    type Output = CompletionRecordV1;
    fn index(&self, id: &RuntimeSubmissionIdV1) -> (result: &CompletionRecordV1)
        ensures *result == self.node(*id).record,
    { self.get(id).unwrap() }
}

}

// All nine native ledger fields are retained. Allocation/report payloads are
// opaque, unchanged owners; this boundary does not project them into authority.
use vstd::prelude::*;

verus! {
#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeGraphVersionStateV1 {
    AvailableAtAdmission, Planned, InFlight, Committed, NotProduced, Failed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CompletionNodeIdV1(u32);

struct Use {
    segment: usize,
    read: bool,
    write: bool,
    input: Option<usize>,
    output: Option<usize>,
}

struct Version {
    segment: usize,
    producer: Option<CompletionNodeIdV1>,
    predecessor: Option<usize>,
    state: RuntimeGraphVersionStateV1,
}

struct VersionLedger<A, R, I> {
    segments: Vec<(A, u64, u64)>,
    current: Vec<Option<usize>>,
    pending: Vec<Option<usize>>,
    uses: Vec<Vec<Use>>,
    records: Vec<Version>,
    nodes: Vec<CompletionNodeIdV1>,
    started: Vec<bool>,
    report_records: Vec<R>,
    report_inputs: Vec<I>,
}

fn r65_version_input_ready_v1(
    current: Option<usize>, expected: usize, phase: RuntimeGraphVersionStateV1,
) -> (out: bool)
    ensures out == (current == Some(expected)
        && (phase == RuntimeGraphVersionStateV1::AvailableAtAdmission
            || phase == RuntimeGraphVersionStateV1::Committed)),
{
    matches!(current, Some(actual) if actual == expected)
        && matches!(phase, RuntimeGraphVersionStateV1::AvailableAtAdmission
            | RuntimeGraphVersionStateV1::Committed)
}

fn r65_version_begin_write_v1(
    phase: RuntimeGraphVersionStateV1, pending: Option<usize>,
    current: Option<usize>, predecessor: usize,
) -> (out: bool)
    ensures out == (phase == RuntimeGraphVersionStateV1::Planned
        && pending == None && current == Some(predecessor)),
{
    matches!(phase, RuntimeGraphVersionStateV1::Planned)
        && pending.is_none()
        && matches!(current, Some(actual) if actual == predecessor)
}

fn r65_version_commit_write_v1(
    phase: RuntimeGraphVersionStateV1, pending: Option<usize>,
    output: usize, current: Option<usize>,
) -> (out: bool)
    ensures out == (phase == RuntimeGraphVersionStateV1::InFlight
        && pending == Some(output) && current == None),
{
    matches!(phase, RuntimeGraphVersionStateV1::InFlight)
        && matches!(pending, Some(actual) if actual == output)
        && current.is_none()
}

struct Snapshot {
    current: Seq<Option<usize>>,
    pending: Seq<Option<usize>>,
    records: Seq<Version>,
    started: Seq<bool>,
}

impl<A, R, I> VersionLedger<A, R, I> {
    closed spec fn snapshot(&self) -> Snapshot {
        Snapshot { current: self.current@, pending: self.pending@,
            records: self.records@, started: self.started@ }
    }

    closed spec fn frame(&self, before: Self) -> bool {
        &&& self.segments == before.segments
        &&& self.uses == before.uses
        &&& self.nodes == before.nodes
        &&& self.report_records == before.report_records
        &&& self.report_inputs == before.report_inputs
    }

    closed spec fn bounded(&self, node: usize) -> bool {
        &&& node < self.uses@.len()
        &&& node < self.started@.len()
        &&& self.current@.len() == self.pending@.len()
        &&& forall|i: int| 0 <= i < self.uses@[node as int]@.len() ==> {
            let usage = #[trigger] self.uses@[node as int]@[i];
            &&& usage.segment < self.current@.len()
            &&& usage.input.is_some() ==> usage.input.unwrap() < self.records@.len()
            &&& usage.output.is_some() ==> usage.output.unwrap() < self.records@.len()
        }
    }

    closed spec fn begin_ready(&self, node: usize) -> bool {
        !self.started@[node as int]
            && forall|i: int| 0 <= i < self.uses@[node as int]@.len()
                ==> begin_use(self.snapshot(), self.uses@[node as int]@[i])
    }

    closed spec fn commit_ready(&self, node: usize) -> bool {
        forall|i: int| 0 <= i < self.uses@[node as int]@.len()
            ==> commit_use(self.snapshot(), self.uses@[node as int]@[i])
    }
}

closed spec fn begin_use(state: Snapshot, usage: Use) -> bool {
    &&& usage.input.is_some() ==> {
        let input = usage.input.unwrap();
        state.current[usage.segment as int] == Some(input)
            && (state.records[input as int].state == RuntimeGraphVersionStateV1::AvailableAtAdmission
                || state.records[input as int].state == RuntimeGraphVersionStateV1::Committed)
    }
    &&& usage.output.is_some() ==> {
        let output = usage.output.unwrap();
        state.records[output as int].predecessor.is_some()
            && state.records[output as int].state == RuntimeGraphVersionStateV1::Planned
            && state.pending[usage.segment as int] == None
            && state.current[usage.segment as int] == state.records[output as int].predecessor
    }
}

closed spec fn commit_use(state: Snapshot, usage: Use) -> bool {
    usage.output.is_some() ==> {
        let output = usage.output.unwrap();
        state.records[output as int].state == RuntimeGraphVersionStateV1::InFlight
            && state.pending[usage.segment as int] == Some(output)
            && state.current[usage.segment as int] == None
    }
}

closed spec fn started(state: Snapshot, node: usize) -> Snapshot {
    Snapshot { started: state.started.update(node as int, true), ..state }
}

closed spec fn update(state: Snapshot, usage: Use, begin: bool) -> Snapshot {
    if let Some(output) = usage.output {
        let phase = if begin { RuntimeGraphVersionStateV1::InFlight }
            else { RuntimeGraphVersionStateV1::Committed };
        Snapshot {
            current: state.current.update(usage.segment as int, if begin { None } else { Some(output) }),
            pending: state.pending.update(usage.segment as int, if begin { Some(output) } else { None }),
            records: state.records.update(output as int, Version { state: phase, ..state.records[output as int] }),
            ..state
        }
    } else { state }
}

closed spec fn updates(state: Snapshot, uses: Seq<Use>, count: nat, begin: bool) -> Snapshot
    decreases count,
{
    if count == 0 { state }
    else { update(updates(state, uses, (count - 1) as nat, begin), uses[count as int - 1], begin) }
}
}

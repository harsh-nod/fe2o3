// Conditional executable ledger refinement, not native write/completion authority.
include!("graph_version_ledger_definitions_v1.rs");
include!("../../fe2o3-runtime/src/async_engine/graph/versions/transition_bodies.rs");

verus! {
impl<A, R, I> VersionLedger<A, R, I> {
    fn begin(&mut self, node: usize) -> (out: bool)
        requires old(self).bounded(node),
        ensures final(self).bounded(node), final(self).frame(*old(self)),
            out == old(self).begin_ready(node),
            !out ==> *final(self) == *old(self),
            out ==> final(self).snapshot() == updates(started(old(self).snapshot(), node),
                old(self).uses@[node as int]@, old(self).uses@[node as int]@.len(), true),
    {
        let ghost before = *self;
        graph_version_begin_body_v1!(verus_exec_expr, self, node, (check, write, count),
            [invariant
                *self == before, self.bounded(node),
                count == self.uses@[node as int]@.len(), check <= count,
                forall|i: int| 0 <= i < check ==> begin_use(self.snapshot(), self.uses@[node as int]@[i]),
             decreases count - check],
            [proof { assert(before.begin_ready(node)); }],
            [invariant
                self.bounded(node), self.frame(before), before.bounded(node), before.begin_ready(node),
                count == before.uses@[node as int]@.len(), write <= count,
                self.snapshot() == updates(started(before.snapshot(), node), before.uses@[node as int]@, write as nat, true),
             decreases count - write],
            [let ghost prior = self.snapshot();],
            [proof {
                assert(self.snapshot() == update(prior, before.uses@[node as int]@[write as int], true));
                reveal_with_fuel(updates, 2);
            }])
    }

    fn commit(&mut self, node: usize) -> (out: bool)
        requires old(self).bounded(node),
        ensures final(self).bounded(node), final(self).frame(*old(self)),
            out == old(self).commit_ready(node),
            !out ==> *final(self) == *old(self),
            out ==> final(self).snapshot() == updates(old(self).snapshot(),
                old(self).uses@[node as int]@, old(self).uses@[node as int]@.len(), false),
    {
        let ghost before = *self;
        graph_version_commit_body_v1!(verus_exec_expr, self, node, (check, write, count),
            [invariant
                *self == before, self.bounded(node),
                count == self.uses@[node as int]@.len(), check <= count,
                forall|i: int| 0 <= i < check ==> commit_use(self.snapshot(), self.uses@[node as int]@[i]),
             decreases count - check],
            [proof { assert(before.commit_ready(node)); }],
            [invariant
                self.bounded(node), self.frame(before), before.bounded(node), before.commit_ready(node),
                count == before.uses@[node as int]@.len(), write <= count,
                self.snapshot() == updates(before.snapshot(), before.uses@[node as int]@, write as nat, false),
             decreases count - write],
            [let ghost prior = self.snapshot();],
            [proof {
                assert(self.snapshot() == update(prior, before.uses@[node as int]@[write as int], false));
                reveal_with_fuel(updates, 2);
            }])
    }
}

// These actual executable calls inhabit all three alias shapes. A same-segment
// different-output plan is not silently normalized: begin keeps its historical
// last-writer behavior and commit refuses without changing the resulting state.
fn executable_alias_cases_are_inhabited(alias: u8)
    requires alias < 3,
{
    let second_segment = if alias == 0 { 1usize } else { 0usize };
    let second_output = if alias == 2 { 2usize } else { 3usize };
    let mut ledger = VersionLedger::<u64, u64, u64> {
        segments: vec![(1u64, 0u64, 4u64), (1u64, 4u64, 4u64)],
        current: vec![Some(0usize), Some(1usize)], pending: vec![None, None],
        uses: vec![vec![
            Use { segment: 0, read: false, write: true, input: None, output: Some(2) },
            Use { segment: second_segment, read: false, write: true, input: None, output: Some(second_output) },
        ]],
        records: vec![
            Version { segment: 0, producer: None, predecessor: None, state: RuntimeGraphVersionStateV1::AvailableAtAdmission },
            Version { segment: 1, producer: None, predecessor: None, state: RuntimeGraphVersionStateV1::AvailableAtAdmission },
            Version { segment: 0, producer: Some(CompletionNodeIdV1(1)), predecessor: Some(0), state: RuntimeGraphVersionStateV1::Planned },
            Version { segment: second_segment, producer: Some(CompletionNodeIdV1(1)), predecessor: Some(second_segment), state: RuntimeGraphVersionStateV1::Planned },
        ],
        nodes: vec![CompletionNodeIdV1(1)], started: vec![false],
        report_records: vec![91u64], report_inputs: vec![92u64],
    };
    assert(ledger.bounded(0));
    assert(ledger.begin_ready(0));
    let begun = ledger.begin(0);
    assert(begun);
    proof { reveal_with_fuel(updates, 3); }
    assert(ledger.pending@[second_segment as int] == Some(second_output));
    let ghost before_commit = ledger.snapshot();
    assert(ledger.commit_ready(0) == (alias != 1));
    let committed = ledger.commit(0);
    assert(committed == (alias != 1));
    proof { reveal_with_fuel(updates, 3); }
    if alias == 1 {
        assert(ledger.snapshot() == before_commit);
    } else {
        assert(ledger.current@[second_segment as int] == Some(second_output));
        assert(ledger.pending@[second_segment as int] == None);
        assert(ledger.records@[second_output as int].state == RuntimeGraphVersionStateV1::Committed);
    }
    assert(ledger.report_records@ == seq![91u64]);
    assert(ledger.report_inputs@ == seq![92u64]);
}
}

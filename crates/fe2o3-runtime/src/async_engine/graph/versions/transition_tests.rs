use super::*;

// The predecessor bodies are deliberately independent of the shared macros.
fn predecessor_begin(ledger: &mut VersionLedger, node: usize) -> bool {
    if ledger.uses[node].iter().any(|usage| {
        usage.input.is_some_and(|input| {
            !r65_version_input_ready_v1(
                ledger.current[usage.segment],
                input,
                ledger.records[input].state,
            )
        }) || usage.output.is_some_and(|output| {
            ledger.records[output].predecessor.is_none_or(|prior| {
                !r65_version_begin_write_v1(
                    ledger.records[output].state,
                    ledger.pending[usage.segment],
                    ledger.current[usage.segment],
                    prior,
                )
            })
        })
    }) {
        return false;
    }
    if ledger.started[node] {
        return false;
    }
    ledger.started[node] = true;
    for usage in &ledger.uses[node] {
        if let Some(output) = usage.output {
            ledger.current[usage.segment] = None;
            ledger.pending[usage.segment] = Some(output);
            ledger.records[output].state = RuntimeGraphVersionStateV1::InFlight;
        }
    }
    true
}

fn predecessor_commit(ledger: &mut VersionLedger, node: usize) -> bool {
    if ledger.uses[node].iter().any(|usage| {
        usage.output.is_some_and(|output| {
            !r65_version_commit_write_v1(
                ledger.records[output].state,
                ledger.pending[usage.segment],
                output,
                ledger.current[usage.segment],
            )
        })
    }) {
        return false;
    }
    for usage in &ledger.uses[node] {
        if let Some(output) = usage.output {
            ledger.records[output].state = RuntimeGraphVersionStateV1::Committed;
            ledger.current[usage.segment] = Some(output);
            ledger.pending[usage.segment] = None;
        }
    }
    true
}

type UseSnapshot = (usize, bool, bool, Option<usize>, Option<usize>);
type VersionSnapshot = (
    usize,
    Option<CompletionNodeIdV1>,
    Option<usize>,
    RuntimeGraphVersionStateV1,
);

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    segments: Vec<Region>,
    current: Vec<Option<usize>>,
    pending: Vec<Option<usize>>,
    uses: Vec<Vec<UseSnapshot>>,
    records: Vec<VersionSnapshot>,
    nodes: Vec<CompletionNodeIdV1>,
    started: Vec<bool>,
    report_records: Vec<RuntimeGraphVersionRecordV1>,
    report_inputs: Vec<RuntimeGraphInputVersionV1>,
    capacities: [usize; 9],
    use_capacities: Vec<usize>,
}

fn snapshot(ledger: &VersionLedger) -> Snapshot {
    Snapshot {
        segments: ledger.segments.clone(),
        current: ledger.current.clone(),
        pending: ledger.pending.clone(),
        uses: ledger
            .uses
            .iter()
            .map(|row| {
                row.iter()
                    .map(|u| (u.segment, u.read, u.write, u.input, u.output))
                    .collect()
            })
            .collect(),
        records: ledger
            .records
            .iter()
            .map(|v| (v.segment, v.producer, v.predecessor, v.state))
            .collect(),
        nodes: ledger.nodes.clone(),
        started: ledger.started.clone(),
        report_records: ledger.report_records.clone(),
        report_inputs: ledger.report_inputs.clone(),
        capacities: [
            ledger.segments.capacity(),
            ledger.current.capacity(),
            ledger.pending.capacity(),
            ledger.uses.capacity(),
            ledger.records.capacity(),
            ledger.nodes.capacity(),
            ledger.started.capacity(),
            ledger.report_records.capacity(),
            ledger.report_inputs.capacity(),
        ],
        use_capacities: ledger.uses.iter().map(Vec::capacity).collect(),
    }
}

fn fixture(
    phase: RuntimeGraphVersionStateV1,
    current: Option<usize>,
    pending: Option<usize>,
    started: bool,
    alias: usize,
    read: bool,
) -> VersionLedger {
    VersionLedger {
        segments: Vec::new(),
        current: vec![current; 2],
        pending: vec![pending; 2],
        uses: vec![vec![
            Use {
                segment: 0,
                read,
                write: true,
                input: read.then_some(0),
                output: Some(2),
            },
            Use {
                segment: usize::from(alias == 0),
                read,
                write: true,
                input: read.then_some(0),
                output: Some(if alias == 2 { 2 } else { 3 }),
            },
        ]],
        records: (0..4)
            .map(|index| Version {
                segment: index % 2,
                producer: None,
                predecessor: (index >= 2).then_some(0),
                state: if index < 2 {
                    RuntimeGraphVersionStateV1::AvailableAtAdmission
                } else {
                    phase
                },
            })
            .collect(),
        nodes: Vec::new(),
        started: vec![started],
        report_records: Vec::new(),
        report_inputs: Vec::new(),
    }
}

#[test]
fn shared_transactions_match_predecessor_for_every_phase_and_alias_shape() {
    use RuntimeGraphVersionStateV1 as P;
    for phase in [
        P::AvailableAtAdmission,
        P::Planned,
        P::InFlight,
        P::Committed,
        P::NotProduced,
        P::Failed,
    ] {
        for current in [None, Some(0), Some(2), Some(3)] {
            for pending in [None, Some(0), Some(2), Some(3)] {
                for started in [false, true] {
                    for alias in 0..3 {
                        for read in [false, true] {
                            for commit in [false, true] {
                                let mut expected =
                                    fixture(phase, current, pending, started, alias, read);
                                let mut actual =
                                    fixture(phase, current, pending, started, alias, read);
                                let before = snapshot(&actual);
                                let expected_result = if commit {
                                    predecessor_commit(&mut expected, 0)
                                } else {
                                    predecessor_begin(&mut expected, 0)
                                };
                                let result = if commit {
                                    actual.commit(0)
                                } else {
                                    actual.begin(0)
                                };
                                assert_eq!(result, expected_result);
                                assert_eq!(snapshot(&actual), snapshot(&expected));
                                if !result {
                                    assert_eq!(snapshot(&actual), before);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn same_segment_different_outputs_preserve_last_pending_and_refuse_commit() {
    let mut ledger = fixture(
        RuntimeGraphVersionStateV1::Planned,
        Some(0),
        None,
        false,
        1,
        false,
    );
    assert!(ledger.begin(0));
    assert_eq!(ledger.pending, [Some(3), None]);
    let before = snapshot(&ledger);
    assert!(!ledger.commit(0));
    assert_eq!(snapshot(&ledger), before);
}

#[test]
fn repeated_exact_output_and_empty_uses_preserve_existing_behavior() {
    let mut ledger = fixture(
        RuntimeGraphVersionStateV1::Planned,
        Some(0),
        None,
        false,
        2,
        false,
    );
    assert!(ledger.begin(0));
    assert!(ledger.commit(0));
    assert!(!ledger.commit(0));
    ledger.uses[0].clear();
    assert!(ledger.commit(0));
    assert!(!ledger.begin(0));
    ledger.started[0] = false;
    assert!(ledger.begin(0));
}

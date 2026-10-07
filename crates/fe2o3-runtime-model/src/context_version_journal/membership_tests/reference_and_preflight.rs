use super::*;

#[test]
fn short_traces_match_independent_writer_and_allocation_maps() {
    const ACTIONS: usize = 11;
    const DEPTH: u32 = 4;
    for capacity in [1, 3] {
        for trace in 0..ACTIONS.pow(DEPTH) {
            let mut journal = Journal::new(7, 3, capacity).unwrap();
            let roster = enroll(&mut journal, &[100, 200, 300]);
            let mut model = ReferenceModel {
                watermark: 0,
                writers: BTreeMap::new(),
                allocations: BTreeMap::new(),
            };
            for destination in &roster {
                model.allocations.insert(
                    destination.allocation.key.local,
                    ExpectedAllocation {
                        descriptor: *destination,
                        epoch: 0,
                        pending: None,
                    },
                );
            }
            let mut history = Vec::new();
            let mut steps = trace;
            for _ in 0..DEPTH {
                let action = steps % ACTIONS;
                steps /= ACTIONS;
                let before = snapshot(&journal);
                if action < 3 {
                    let writer = Key {
                        kind: if action == 1 {
                            Kind::Submission
                        } else {
                            Kind::Synchronous
                        },
                        ..key([1, 4, 2][action])
                    };
                    let error = if writer.local <= model.watermark {
                        Some(Error::WriterReplay)
                    } else if model.writers.len() == capacity {
                        Some(Error::WriterCapacity)
                    } else {
                        None
                    };
                    let actual = journal.register_writer(writer);
                    if let Some(error) = error {
                        assert_eq!(actual, Err(error));
                        assert_eq!(snapshot(&journal), before);
                    } else {
                        let reference = actual.unwrap();
                        assert_eq!(reference.key, writer);
                        assert!(reference.slot < capacity);
                        assert!(
                            model
                                .writers
                                .values()
                                .all(|(other, _)| other.slot != reference.slot)
                        );
                        model.writers.insert(writer.local, (reference, None));
                        model.watermark = writer.local;
                        history.push(reference);
                    }
                } else {
                    let reference = if matches!(action, 3 | 4 | 8) {
                        history.first()
                    } else {
                        history.last()
                    }
                    .copied()
                    .unwrap_or(Reference {
                        slot: usize::MAX,
                        key: key(42),
                    });
                    if action == 3 {
                        let valid = model.reserved(reference);
                        assert_eq!(
                            journal.abort_reserved(reference),
                            if valid {
                                Ok(())
                            } else {
                                Err(Error::InvalidReference)
                            }
                        );
                        if valid {
                            model.writers.remove(&reference.key.local);
                        } else {
                            assert_eq!(snapshot(&journal), before);
                        }
                    } else {
                        let destinations = match action {
                            4 => vec![roster[0]],
                            5 => vec![roster[0], roster[1]],
                            6 => Vec::new(),
                            7 => vec![Write {
                                byte_extent: roster[2].byte_extent + 1,
                                ..roster[2]
                            }],
                            8 => vec![roster[1], roster[0]],
                            9 => vec![roster[2]],
                            _ => roster.clone(),
                        };
                        let expected = model.begin(reference, &destinations);
                        assert_eq!(journal.begin_write(reference, &destinations), expected);
                        if expected.is_err() {
                            assert_eq!(snapshot(&journal), before);
                        }
                    }
                }
                assert_eq!(storage(&journal), before.storage);
                model.check(&journal, &history);
            }
        }
    }
}

#[test]
fn begin_routes_full_preflight_before_bounded_planning_and_commit() {
    let source = include_str!("../../context_version_journal.rs");
    let adapter = include_str!("../begin.rs");
    let bodies = include_str!("../begin_bodies.rs");
    let begin = source
        .split("pub fn begin_write(")
        .nth(1)
        .unwrap()
        .split("fn read_allocation(")
        .next()
        .unwrap();
    assert!(begin.contains("begin::begin_exec_v1(self, writer, canonical)"));
    assert!(adapter.contains("include!(\"begin_bodies.rs\")"));
    for body in [
        "reserved",
        "canonical",
        "destinations",
        "slots",
        "preflight",
        "stage",
        "commit",
        "execution",
    ] {
        assert!(adapter.contains(&std::format!("begin_{body}_body!")));
    }
    for forbidden in [
        "try_reserve",
        ".reserve(",
        "vec!",
        "Vec::",
        "Box::",
        "to_vec",
        "to_owned",
        ".push(",
        ".resize",
        ".collect(",
        ".sort",
        ".clone(",
        ".extend(",
        "loop {",
        "$journal.allocations.iter",
        "$journal.members.iter",
        "$journal.writers.iter",
        "< $journal.allocation_capacity",
        "< $journal.writer_capacity",
    ] {
        assert!(
            !bodies.contains(forbidden) && !adapter.contains(forbidden),
            "Begin contains {forbidden}"
        );
    }
    assert_eq!(bodies.matches("while $index < $roster.len()").count(), 3);
    assert_eq!(bodies.matches("while $index < $count").count(), 2);
    let preflight = bodies
        .split("macro_rules! begin_preflight_body")
        .nth(1)
        .unwrap()
        .split("macro_rules! begin_stage_body")
        .next()
        .unwrap();
    let mut previous = 0;
    for token in [
        "begin_reserved_exec_v1",
        "checked_sub(1)",
        "allocation_capacity",
        "begin_canonical_exec_v1",
        "begin_destinations_exec_v1",
        "member_free.len()",
        "scratch.len()",
        "begin_slots_exec_v1",
        "Ok(reserved_count)",
    ] {
        let at = preflight.find(token).unwrap();
        assert!(at > previous, "out-of-order preflight: {token}");
        previous = at;
    }
    let destinations = bodies
        .split("macro_rules! begin_destinations_body")
        .nth(1)
        .unwrap()
        .split("macro_rules! begin_slots_body")
        .next()
        .unwrap();
    let mut previous = 0;
    for token in [
        "shared_retained_allocation_v1",
        "AllocationDeviceMismatch",
        "AllocationExtentMismatch",
        "AllocationBusy",
        "EpochExhausted",
    ] {
        let at = destinations.find(token).unwrap();
        assert!(at > previous, "out-of-order destination: {token}");
        previous = at;
    }
    let execution = bodies
        .split("macro_rules! begin_execution_body")
        .nth(1)
        .unwrap();
    assert!(
        execution.find("begin_preflight_exec_v1").unwrap()
            < execution.find("begin_stage_exec_v1").unwrap()
    );
    assert!(
        execution.find("begin_stage_exec_v1").unwrap()
            < execution.find("begin_commit_exec_v1").unwrap()
    );
    let commit = bodies
        .split("macro_rules! begin_commit_body")
        .nth(1)
        .unwrap()
        .split("macro_rules! begin_execution_body")
        .next()
        .unwrap();
    assert!(commit.contains(".take().expect(\"complete member plan\")"));
    assert!(commit.contains(".as_mut().expect(\"retained exact allocation\")"));
    assert!(
        commit.find("entry.pending_member").unwrap()
            < commit.find("$journal.reserved_count =").unwrap()
    );
}

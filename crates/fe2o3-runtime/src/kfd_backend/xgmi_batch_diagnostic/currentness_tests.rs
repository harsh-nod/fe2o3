use super::*;
use fe2o3_kfd::Gfx942TopologyDiscoveryDiagnosticsV1;

fn identity(submission: u64) -> CallIdentity {
    CallIdentity {
        direction: (submission % 2) as usize,
        submission,
        published: false,
    }
}

fn timing() -> KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
    KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
        admission_validation_ns: Some(2),
        preparation_ns: Some(3),
        opening_currentness_ns: Some(100),
        submission_ns: Some(7),
        wait_ns: Some(11),
        closing_currentness_ns: Some(200),
        settlement_ns: Some(17),
        total_ns: Some(361),
    }
}

fn pair(scale: u64) -> Gfx942XgmiPairCurrentnessDiagnosticsV1 {
    Gfx942XgmiPairCurrentnessDiagnosticsV1 {
        source_before_ns: Some(scale),
        peer_before_ns: Some(2 * scale),
        topology_discovery_ns: Some(11 * scale),
        route_and_equality_ns: Some(3 * scale),
        source_after_ns: Some(4 * scale),
        peer_after_ns: Some(5 * scale),
        total_ns: Some(26 * scale),
        topology: Gfx942TopologyDiscoveryDiagnosticsV1 {
            topology_tree_ns: Some(scale),
            initial_identity_ns: Some(2 * scale),
            render_correlation_ns: Some(3 * scale),
            closing_identity_ns: Some(4 * scale),
            total_ns: Some(10 * scale),
        },
    }
}

fn details(currentness: bool) -> Option<[Gfx942XgmiPairCurrentnessDiagnosticsV1; 2]> {
    currentness.then(|| [pair(1), pair(2)])
}

fn record(recorder: &mut Recorder, submission: u64) {
    let id = identity(submission);
    assert!(recorder.begin(id, 1));
    recorder.finish(id, Some(timing()), details(recorder.is_currentness()));
}

fn pointer(records: &Records) -> *const () {
    match records {
        Records::Aggregate(records) => records.as_ptr().cast(),
        Records::Currentness(records) => records.as_ptr().cast(),
    }
}

fn observations(records: &Records) -> Vec<KfdRuntimeXgmiAggregateCallObservationV1> {
    match records {
        Records::Aggregate(records) => records.clone(),
        Records::Currentness(records) => records.iter().map(|value| value.aggregate).collect(),
    }
}

fn nested_observations(
    records: &Records,
) -> Option<Vec<KfdRuntimeXgmiAggregateCurrentnessObservationV1>> {
    match records {
        Records::Aggregate(_) => None,
        Records::Currentness(records) => Some(records.clone()),
    }
}

#[test]
fn both_modes_preallocate_once_preserve_identity_and_enforce_capacity() {
    for currentness in [false, true] {
        assert!(Recorder::with_currentness([71, 93], 0, currentness).is_err());
        assert!(Recorder::with_currentness([71, 93], MAX_RECORDS + 1, currentness).is_err());
        let boundary = Recorder::with_currentness([71, 93], MAX_RECORDS, currentness).unwrap();
        assert!(boundary.records.capacity() >= MAX_RECORDS);
        let mut recorder = Recorder::with_currentness([71, 93], 3, currentness).unwrap();
        let original_pointer = pointer(&recorder.records);
        let original_capacity = recorder.records.capacity();
        for submission in [4, 7, 11] {
            record(&mut recorder, submission);
            assert_eq!(pointer(&recorder.records), original_pointer);
            assert_eq!(recorder.records.capacity(), original_capacity);
        }
        assert!(recorder.complete());
        assert_eq!(
            observations(&recorder.records),
            [4, 7, 11].map(|submission| KfdRuntimeXgmiAggregateCallObservationV1 {
                backend_submission: submission,
                source_device: if submission % 2 == 0 { 71 } else { 93 },
                destination_device: if submission % 2 == 0 { 93 } else { 71 },
                host: timing(),
            })
        );
        if let Records::Currentness(records) = &recorder.records {
            for value in records {
                assert_eq!([value.opening, value.closing], [pair(1), pair(2)]);
            }
        }
        assert!(!recorder.begin(identity(12), 1));
        assert!(!recorder.complete());
        assert_eq!(recorder.records.len(), 3);
        assert_eq!(pointer(&recorder.records), original_pointer);
        assert_eq!(recorder.records.capacity(), original_capacity);
    }
}

#[test]
fn both_modes_reject_noncanonical_admission_and_preserve_pending_custody() {
    for currentness in [false, true] {
        for case in 0..9 {
            let mut recorder = Recorder::with_currentness([71, 93], 2, currentness).unwrap();
            let mut id = identity(7);
            let mut packets = 1;
            match case {
                0 => packets = 0,
                1 => packets = 2,
                2 => id.direction = 2,
                3 => id.submission = 0,
                4 => id.published = true,
                5 => assert!(recorder.begin(id, 1)),
                6 => record(&mut recorder, 7),
                7 => record(&mut recorder, 8),
                _ => recorder.invalidate(),
            }
            let before = (
                recorder.pending,
                recorder.last_submission,
                recorder.records.len(),
            );
            assert!(!recorder.begin(id, packets));
            assert!(recorder.invalid);
            assert_eq!(
                (
                    recorder.pending,
                    recorder.last_submission,
                    recorder.records.len()
                ),
                before
            );
            assert!(!recorder.begin(identity(9), 1));
            assert!(!recorder.complete());
        }
    }
}

#[test]
fn malformed_finish_invalidates_without_append_or_state_advancement() {
    for currentness in [false, true] {
        for case in 0..6 {
            let mut recorder = Recorder::with_currentness([71, 93], 3, currentness).unwrap();
            record(&mut recorder, 4);
            let mut id = identity(7);
            assert!(recorder.begin(id, 1));
            let mut host = Some(timing());
            let mut detail = details(currentness);
            match case {
                0 => host = None,
                1 => id.submission += 1,
                2 => id.direction = usize::MAX,
                3 => detail = details(!currentness),
                4 => host.as_mut().unwrap().total_ns = Some(0),
                _ => recorder.invalidate(),
            }
            recorder.finish(id, host, detail);
            assert!(recorder.invalid);
            assert_eq!(recorder.pending, Some(identity(7)));
            assert_eq!(recorder.last_submission, Some(4));
            assert_eq!(recorder.records.len(), 1);
            assert!(!recorder.complete());
        }
    }
}

#[test]
fn each_missing_nested_field_overflow_and_containment_failure_invalidates_capture() {
    for direction in 0..2 {
        for case in 0..18 {
            let mut detail = [pair(1), pair(2)];
            let value = &mut detail[direction];
            match case {
                0 => value.source_before_ns = None,
                1 => value.peer_before_ns = None,
                2 => value.topology_discovery_ns = None,
                3 => value.route_and_equality_ns = None,
                4 => value.source_after_ns = None,
                5 => value.peer_after_ns = None,
                6 => value.total_ns = None,
                7 => value.topology.topology_tree_ns = None,
                8 => value.topology.initial_identity_ns = None,
                9 => value.topology.render_correlation_ns = None,
                10 => value.topology.closing_identity_ns = None,
                11 => value.topology.total_ns = None,
                12 => {
                    value.source_before_ns = Some(u64::MAX);
                    value.total_ns = Some(u64::MAX);
                }
                13 => {
                    value.topology.topology_tree_ns = Some(u64::MAX);
                    value.topology.total_ns = Some(u64::MAX);
                }
                14 => value.topology.total_ns = Some(1),
                15 => value.topology.total_ns = value.topology_discovery_ns.map(|ns| ns + 1),
                16 => value.total_ns = value.total_ns.map(|ns| ns - 1),
                _ => value.total_ns = Some(if direction == 0 { 101 } else { 201 }),
            }
            let mut recorder = Recorder::with_currentness([71, 93], 1, true).unwrap();
            let id = identity(4);
            assert!(recorder.begin(id, 1));
            recorder.finish(id, Some(timing()), Some(detail));
            assert!(recorder.invalid, "direction={direction} case={case}");
            assert_eq!(recorder.records.len(), 0);
            assert_eq!(recorder.pending, Some(id));
            assert_eq!(recorder.last_submission, None);
        }
    }
}

#[test]
fn nested_and_outer_containment_allow_exact_equality_without_double_counting() {
    let mut detail = [pair(1), pair(2)];
    for value in &mut detail {
        value.topology.total_ns = value.topology_discovery_ns;
    }
    let host = KfdRuntimeXgmiAggregateCallDiagnosticsV1 {
        opening_currentness_ns: detail[0].total_ns,
        closing_currentness_ns: detail[1].total_ns,
        total_ns: Some(118),
        ..timing()
    };
    let mut recorder = Recorder::with_currentness([71, 93], 1, true).unwrap();
    let id = identity(4);
    assert!(recorder.begin(id, 1));
    recorder.finish(id, Some(host), Some(detail));
    assert!(recorder.complete());
    let Records::Currentness(records) = recorder.records else {
        panic!("currentness storage")
    };
    assert_eq!([records[0].opening, records[0].closing], detail);
    assert_eq!(records[0].aggregate.host, host);
}

#[test]
fn extraction_mode_mismatch_preserves_complete_incomplete_and_invalid_captures() {
    for currentness in [false, true] {
        for state in 0..4 {
            let mut recorder = Recorder::with_currentness([71, 93], 1, currentness).unwrap();
            if matches!(state, 0 | 2) {
                record(&mut recorder, 4);
            }
            if state == 2 {
                recorder.invalidate();
            }
            if state == 3 {
                assert!(recorder.begin(identity(4), 1));
            }
            let original_pointer = pointer(&recorder.records);
            let original_records = observations(&recorder.records);
            let original_nested = nested_observations(&recorder.records);
            let original_state = (recorder.pending, recorder.last_submission);
            let mut slot = Some(recorder);
            assert_eq!(
                take_storage(&mut slot, false, true, true, !currentness).err(),
                Some(KfdRuntimeBackendErrorKindV1::Unsupported)
            );
            let retained = slot.as_ref().unwrap();
            assert_eq!(pointer(&retained.records), original_pointer);
            assert_eq!(observations(&retained.records), original_records);
            assert_eq!(nested_observations(&retained.records), original_nested);
            assert_eq!((retained.pending, retained.last_submission), original_state);
            assert_eq!(retained.invalid, state == 2);
            let result = take_storage(&mut slot, false, true, true, currentness);
            if state == 0 {
                let records = result.unwrap();
                assert_eq!(pointer(&records), original_pointer);
                assert_eq!(observations(&records), original_records);
                assert_eq!(nested_observations(&records), original_nested);
                assert!(slot.is_none());
                assert_eq!(
                    take_storage(&mut slot, false, true, true, currentness).err(),
                    Some(KfdRuntimeBackendErrorKindV1::Unsupported)
                );
            } else {
                assert_eq!(
                    result.err(),
                    Some(KfdRuntimeBackendErrorKindV1::InvalidLaunch)
                );
                assert!(slot.is_some());
            }
        }
    }
}

#[test]
fn extraction_precedence_requires_teardown_in_both_modes_even_without_a_recorder() {
    for currentness in [false, true] {
        for present in [false, true] {
            for terminal in [false, true] {
                for shutdown in [false, true] {
                    for quiescent in [false, true] {
                        for matching in [false, true] {
                            let mut slot = present.then(|| {
                                Recorder::with_currentness([71, 93], 1, currentness).unwrap()
                            });
                            let expected = if terminal {
                                KfdRuntimeBackendErrorKindV1::Terminal
                            } else if !shutdown || !quiescent {
                                KfdRuntimeBackendErrorKindV1::Busy
                            } else if !present || !matching {
                                KfdRuntimeBackendErrorKindV1::Unsupported
                            } else {
                                KfdRuntimeBackendErrorKindV1::InvalidLaunch
                            };
                            assert_eq!(
                                take_storage(
                                    &mut slot,
                                    terminal,
                                    shutdown,
                                    quiescent,
                                    if matching { currentness } else { !currentness },
                                )
                                .err(),
                                Some(expected)
                            );
                            assert_eq!(slot.is_some(), present);
                            assert!(!slot.as_ref().is_some_and(|recorder| recorder.invalid));
                        }
                    }
                }
            }
        }
    }
}

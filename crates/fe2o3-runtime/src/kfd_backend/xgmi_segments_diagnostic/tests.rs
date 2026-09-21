use super::*;

#[test]
fn disabled_timer_never_reads_clock_or_changes_results() {
    let mut timer = Timer::<false>::new();
    assert_eq!(timer.start_with(|| panic!("disabled clock")), None);
    let mut calls = 0;
    assert_eq!(
        timer.measure(Phase::Wait, || {
            calls += 1;
            Err::<(), _>(7)
        }),
        Err(7)
    );
    timer.end(Phase::Closing, Some(Instant::now()));
    assert_eq!(calls, 1);
    assert_eq!(timer.elapsed, [0; PHASE_COUNT]);
    assert_eq!(timer.calls, [0; PHASE_COUNT]);
    assert!(!timer.invalid);
}

#[test]
fn repeated_spans_sum_but_missing_duplicate_and_overflow_are_sticky() {
    let mut timer = Timer::<true>::new();
    for value in 1..=65 {
        timer.record(Phase::Wait, Some(value));
    }
    assert_eq!(timer.elapsed[Phase::Wait as usize], 65 * 66 / 2);
    assert_eq!(timer.calls[Phase::Wait as usize], 65);
    assert!(!timer.invalid);
    for phase in [
        Phase::Admission,
        Phase::Preparation,
        Phase::Opening,
        Phase::Closing,
        Phase::Settlement,
    ] {
        let mut timer = Timer::<true>::new();
        timer.record(phase, Some(1));
        timer.record(phase, Some(1));
        assert!(timer.invalid);
        timer.record(Phase::Wait, Some(1));
        assert!(timer.invalid);
    }
    for phase in [Phase::Submission, Phase::Wait] {
        let mut missing = Timer::<true>::new();
        missing.record(phase, None);
        missing.record(phase, Some(1));
        assert!(missing.invalid);
        let mut overflow = Timer::<true>::new();
        overflow.record(phase, Some(u64::MAX));
        overflow.record(phase, Some(1));
        assert!(overflow.invalid);
        let mut count = Timer::<true>::new();
        count.calls[phase as usize] = u32::MAX;
        count.record(phase, Some(0));
        assert!(count.invalid);
    }
}

#[cfg(feature = "hardware-diagnostic")]
mod enabled {
    use super::*;
    use fe2o3_kfd::Gfx942TopologyDiscoveryDiagnosticsV1;

    fn pair() -> Gfx942XgmiPairCurrentnessDiagnosticsV1 {
        Gfx942XgmiPairCurrentnessDiagnosticsV1 {
            source_before_ns: Some(1),
            peer_before_ns: Some(1),
            topology_discovery_ns: Some(5),
            route_and_equality_ns: Some(1),
            source_after_ns: Some(1),
            peer_after_ns: Some(1),
            total_ns: Some(10),
            topology: Gfx942TopologyDiscoveryDiagnosticsV1 {
                topology_tree_ns: Some(1),
                initial_identity_ns: Some(1),
                render_correlation_ns: Some(1),
                closing_identity_ns: Some(1),
                total_ns: Some(4),
            },
        }
    }

    fn timer() -> Timer<true> {
        let mut timer = Timer::new();
        for phase in [
            Phase::Admission,
            Phase::Preparation,
            Phase::Opening,
            Phase::Closing,
            Phase::Settlement,
        ] {
            timer.record(phase, Some(10));
        }
        for _ in 0..65 {
            timer.record(Phase::Submission, Some(1));
            timer.record(Phase::Wait, Some(2));
        }
        timer.currentness = [Some(pair()); 2];
        timer
    }

    fn timing() -> KfdRuntimeXgmiSegmentsTimingV1 {
        timer().finish_ns(245, 65).unwrap()
    }

    fn identity(submission: u64) -> Identity {
        Identity {
            submission,
            direction: (submission % 2) as usize,
            descriptors: 65,
            useful_bytes: 65536,
            fresh: true,
        }
    }

    fn record(recorder: &mut Recorder, submission: u64) {
        let identity = identity(submission);
        assert!(recorder.begin(Some(identity), false));
        recorder.finish(identity, Some(timing()));
    }

    #[test]
    fn complete_timer_requires_exact_counts_checked_sum_and_nested_containment() {
        assert!(timer().finish_ns(245, 65).is_some());
        assert!(timer().finish_ns(244, 65).is_none());
        for count in [0, 1, 64, 66] {
            assert!(timer().finish_ns(245, count).is_none());
        }
        for phase in 0..PHASE_COUNT {
            let mut t = timer();
            t.calls[phase] = 0;
            assert!(t.finish_ns(u64::MAX, 65).is_none());
            let mut t = timer();
            t.elapsed[phase] = u64::MAX;
            assert!(t.finish_ns(u64::MAX, 65).is_none());
        }
        for side in 0..2 {
            let mut t = timer();
            t.currentness[side] = None;
            assert!(t.finish_ns(245, 65).is_none());
            let mut t = timer();
            t.currentness[side].as_mut().unwrap().total_ns = Some(11);
            assert!(t.finish_ns(245, 65).is_none());
            let mut t = timer();
            t.currentness[side].as_mut().unwrap().peer_after_ns = None;
            assert!(t.finish_ns(245, 65).is_none());
            let mut t = timer();
            t.currentness[side].as_mut().unwrap().topology.total_ns = Some(6);
            assert!(t.finish_ns(245, 65).is_none());
        }
    }

    #[test]
    fn bounded_storage_retains_identity_without_reallocation() {
        assert!(Recorder::new([71, 93], 0).is_err());
        assert!(Recorder::new([71, 93], 40001).is_err());
        let mut r = Recorder::new([71, 93], 3).unwrap();
        let pointer = r.records.as_ptr();
        let capacity = r.records.capacity();
        for id in [4, 7, 11] {
            record(&mut r, id);
            assert_eq!(r.records.as_ptr(), pointer);
            assert_eq!(r.records.capacity(), capacity);
            let row = r.records.last().unwrap();
            assert_eq!(row.backend_submission, id);
            assert_eq!(row.source_device, [71, 93][(id % 2) as usize]);
            assert_eq!(row.destination_device, [93, 71][(id % 2) as usize]);
            assert_eq!((row.descriptor_count, row.useful_bytes), (65, 65536));
        }
        assert!(r.complete());
        assert!(!r.begin(Some(identity(12)), false));
        assert!(!r.complete());
    }

    #[test]
    fn ineligible_progress_and_pending_then_success_cannot_be_replaced() {
        let base = identity(7);
        for candidate in [
            None,
            Some(Identity {
                fresh: false,
                ..base
            }),
            Some(Identity {
                direction: 2,
                ..base
            }),
            Some(Identity {
                submission: 0,
                ..base
            }),
            Some(Identity {
                descriptors: 0,
                ..base
            }),
            Some(Identity {
                descriptors: 4097,
                ..base
            }),
            Some(Identity {
                useful_bytes: 0,
                ..base
            }),
        ] {
            let mut r = Recorder::new([71, 93], 1).unwrap();
            assert!(!r.begin(candidate, false));
            assert!(!r.begin(Some(base), false));
            assert!(!r.complete());
        }
        let mut r = Recorder::new([71, 93], 1).unwrap();
        assert!(!r.begin(Some(base), true));
        assert!(!r.begin(Some(base), false));
        let mut r = Recorder::new([71, 93], 1).unwrap();
        assert!(r.begin(Some(base), false));
        r.finish(base, None); // Pending, failed dependency, or any error.
        assert!(!r.begin(Some(base), false));
        r.finish(base, Some(timing()));
        assert!(!r.complete());
        for changed in [
            base,
            Identity {
                submission: 8,
                ..base
            },
        ] {
            let mut r = Recorder::new([71, 93], 1).unwrap();
            assert!(r.begin(Some(base), false));
            assert!(!r.begin(Some(changed), false)); // Reentry or unclosed attempt.
            r.finish(base, Some(timing()));
            assert!(!r.complete());
        }
    }

    #[test]
    fn identity_mismatch_order_and_external_invalidation_are_sticky() {
        for candidate in [
            Identity {
                submission: 8,
                ..identity(7)
            },
            Identity {
                direction: 0,
                ..identity(7)
            },
            Identity {
                descriptors: 64,
                ..identity(7)
            },
            Identity {
                useful_bytes: 65535,
                ..identity(7)
            },
        ] {
            let mut r = Recorder::new([71, 93], 1).unwrap();
            assert!(r.begin(Some(identity(7)), false));
            r.finish(candidate, Some(timing()));
            assert!(!r.complete());
        }
        for next in [6, 7] {
            let mut r = Recorder::new([71, 93], 2).unwrap();
            record(&mut r, 7);
            assert!(!r.begin(Some(identity(next)), false));
        }
        for invalidate_after in [false, true] {
            let mut r = Recorder::new([71, 93], 1).unwrap();
            assert!(r.begin(Some(identity(7)), false));
            if !invalidate_after {
                r.invalidate();
            }
            r.finish(identity(7), Some(timing()));
            if invalidate_after {
                r.invalidate();
            }
            assert!(!r.complete());
        }
    }

    #[test]
    fn extraction_is_teardown_gated_preserves_failed_takes_and_is_single_use() {
        let mut slot = Some(Recorder::new([71, 93], 1).unwrap());
        for (terminal, shutdown, quiescent, expected) in [
            (true, false, false, KfdRuntimeBackendErrorKindV1::Terminal),
            (false, false, true, KfdRuntimeBackendErrorKindV1::Busy),
            (false, true, false, KfdRuntimeBackendErrorKindV1::Busy),
            (
                false,
                true,
                true,
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            ),
        ] {
            assert_eq!(
                take_records(&mut slot, terminal, shutdown, quiescent),
                Err(expected)
            );
            assert!(slot.is_some());
        }
        record(slot.as_mut().unwrap(), 7);
        assert_eq!(take_records(&mut slot, false, true, true).unwrap().len(), 1);
        assert_eq!(
            take_records(&mut slot, false, true, true),
            Err(KfdRuntimeBackendErrorKindV1::Unsupported)
        );
    }

    #[test]
    fn invalid_timing_cannot_cross_the_recorder_boundary() {
        for mutation in 0..6 {
            let id = identity(7);
            let mut slot = Some(Recorder::new([71, 93], 1).unwrap());
            let r = slot.as_mut().unwrap();
            assert!(r.begin(Some(id), false));
            let mut host = timing();
            match mutation {
                0 => host.total_ns = 244,
                1 => host.wait_ns = u64::MAX,
                2 => host.submission_calls = 64,
                3 => host.opening.total_ns = None,
                4 => host.closing.total_ns = Some(11),
                5 => host.closing.topology.total_ns = Some(6),
                _ => unreachable!(),
            }
            r.finish(id, Some(host));
            assert_eq!(
                take_records(&mut slot, false, true, true),
                Err(KfdRuntimeBackendErrorKindV1::InvalidLaunch)
            );
        }
    }

    #[test]
    fn diagnostic_entrypoints_guard_early_returns_and_modes_exclude_each_other() {
        // Supplemental wiring guards; scripted execution tests establish behavior.
        let backend = include_str!("../../kfd_backend.rs");
        let native = backend
            .split("impl RuntimeBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
            .nth(1)
            .unwrap();
        let poll = native
            .split("fn poll_v1(")
            .nth(1)
            .unwrap()
            .split("fn wait_v1(")
            .next()
            .unwrap();
        assert!(
            poll.find("xgmi_segments_diagnostic").unwrap()
                < poll
                    .find("xgmi_submission_has_failed_dependency_v1")
                    .unwrap()
        );
        assert!(
            poll.find("xgmi_segments_diagnostic").unwrap()
                < poll.find("active.ticket.is_none()").unwrap()
        );
        let flush = backend
            .split("impl RuntimeFlushBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
            .nth(1)
            .unwrap()
            .split("impl RuntimeCancellationBackendV1")
            .next()
            .unwrap();
        assert!(
            flush.find("xgmi_segments_diagnostic").unwrap() < flush.find("let failed =").unwrap()
        );
        assert!(
            flush.find("xgmi_segments_diagnostic").unwrap()
                < flush.find("classify_xgmi_flush_v1(").unwrap()
        );
        let cancel = backend
            .split("impl RuntimeCancellationBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
            .nth(1)
            .unwrap()
            .split("fn drain_v1(")
            .next()
            .unwrap();
        assert!(
            cancel.find("xgmi_segments_diagnostic").unwrap()
                < cancel.find(".remove(&submission)").unwrap()
        );
        let batch = include_str!("../xgmi_batch.rs")
            .split("impl RuntimePeerCopyBatchBackendV1")
            .nth(1)
            .unwrap();
        assert!(
            batch.find("xgmi_segments_diagnostic").unwrap()
                < batch.find("if self.xgmi_aggregate_diagnostic").unwrap()
        );
        for source in [
            include_str!("../xgmi_diagnostic.rs"),
            include_str!("../xgmi_batch_diagnostic.rs"),
        ] {
            assert!(source.contains("|| self.xgmi_segments_diagnostic.is_some()"));
        }
        let ordered = include_str!("../xgmi_segments.rs");
        let progress = ordered
            .split("pub(super) fn progress_peer_segments(")
            .nth(1)
            .unwrap();
        assert!(
            progress.find("recorder.invalidate()").unwrap()
                < progress
                    .find("self.progress_peer_segments_profiled(")
                    .unwrap()
        );
        assert!(
            progress.find(".begin(identity, one_step)").unwrap()
                < progress.find("self.require_live()?").unwrap()
        );
        assert!(
            progress
                .contains("super::xgmi_batch::finish_native_attempt(result, &mut self.terminal)")
        );
    }
}

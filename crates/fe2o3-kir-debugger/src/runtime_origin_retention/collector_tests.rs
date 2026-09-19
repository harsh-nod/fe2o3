use super::*;
use crate::{
    DebugTerminalDetailStateV2, DebugTerminalDetailV2, DebugWaveWidthV1, TranscriptCollectorV1,
};
use fe2o3_kir_sim::{
    DivergentWorkgroupBarrierV2, SimulationDebugSinkV1, SimulationOutOfBoundsV2,
    WorkgroupParticipantV1,
};

#[test]
fn limits_capacity_and_default_disabled_reservation_are_bounded() {
    let row = size_of::<Row>();
    let fixed = size_of::<Retention>();
    for (rows, bytes) in [
        (0, MAX_METADATA),
        (MAX_ROWS + 1, MAX_METADATA),
        (1, fixed + row - 1),
        (1, MAX_METADATA + 1),
        (usize::MAX, usize::MAX),
    ] {
        assert!(Limits::new(rows, bytes).is_err());
    }
    let disabled = Retention::with_reservation(None, |_| panic!("disabled must not allocate"));
    assert_eq!(disabled.coverage, Coverage::Disabled);
    assert_eq!(disabled.rows.capacity(), 0);
    assert_eq!(disabled.metadata_bytes(), fixed);
    let retained = Retention::new(Some(Limits::new(5, fixed + 3 * row).unwrap()));
    assert_eq!(retained.capacity_limit, 3);
    assert!(retained.metadata_bytes() <= fixed + 3 * row);
    assert!(row <= 32);
    let full = Limits::new(MAX_ROWS, MAX_METADATA).unwrap();
    assert!(full.rows * row + fixed <= full.bytes);
}

#[test]
fn failed_or_dishonest_reservations_never_create_a_valid_origin_prefix() {
    let limits = Some(fixtures::origin_limits(2));
    let failed = Retention::with_reservation(limits, |_| Err(()));
    assert_eq!(
        failed.coverage,
        Coverage::PrefixTruncated(Cutoff::AllocationFailure)
    );
    let undersized = Retention::with_reservation(limits, |_| Ok(Vec::new()));
    assert_eq!(
        undersized.coverage,
        Coverage::PrefixTruncated(Cutoff::InvalidCapacity)
    );
    let oversized = Retention::with_reservation(limits, |count| Ok(Vec::with_capacity(count + 1)));
    assert_eq!(
        oversized.coverage,
        Coverage::PrefixTruncated(Cutoff::InvalidCapacity)
    );
    for retained in [failed, undersized, oversized] {
        assert_eq!(retained.rows.len(), 0);
        assert_eq!(retained.rows.capacity(), 0);
        assert_eq!(retained.metadata_bytes(), size_of::<Retention>());
    }
}

#[test]
fn continue_stop_and_drop_commit_only_accepted_legacy_records() {
    let samples = fixtures::raw();
    let (record, context) = &samples[0];
    for control in [Control::Continue, Control::Stop, Control::DropAndStop] {
        let mut retained = Retention::new(Some(fixtures::origin_limits(1)));
        let pending = retained.prepare(0, record, *context);
        let accepted = usize::from(control != Control::DropAndStop);
        retained.commit(pending, control, accepted);
        assert_eq!(retained.rows.len(), accepted);
        assert_eq!(retained.coverage, Coverage::Complete);
        if accepted == 1 {
            let pending = retained.prepare(1, &samples[1].0, samples[1].1);
            retained.commit(pending, Control::DropAndStop, 1);
            assert_eq!(
                retained.coverage,
                Coverage::Complete,
                "a rejected record cannot truncate origins"
            );
        }
    }
    let mut retained = Retention::new(Some(fixtures::origin_limits(1)));
    let pending = retained.prepare(0, record, *context);
    retained.commit(pending, Control::Continue, 0);
    assert_eq!(retained.coverage, Coverage::InvalidJoin);
}

#[test]
fn wrong_invocation_site_ordinal_and_commit_index_fail_closed() {
    let samples = fixtures::raw();
    let mutations: [fn(&mut SimulationDebugRecordV1); 3] = [
        |r: &mut SimulationDebugRecordV1| r.invocation.launch_extent[1] += 1,
        |r: &mut SimulationDebugRecordV1| r.site.operation += 1,
        |r: &mut SimulationDebugRecordV1| r.ordinal += 1,
    ];
    for mutate in mutations {
        let mut record = samples[0].0.clone();
        mutate(&mut record);
        let mut retained = Retention::new(Some(fixtures::origin_limits(2)));
        let pending = retained.prepare(0, &record, samples[0].1);
        retained.commit(pending, Control::Continue, 1);
        assert_eq!(retained.coverage, Coverage::InvalidJoin);
        assert!(retained.rows.is_empty());
    }
    let mut retained = Retention::new(Some(fixtures::origin_limits(2)));
    let pending = retained.prepare(usize::MAX, &samples[0].0, samples[0].1);
    retained.commit(pending, Control::Continue, 0);
    assert_eq!(retained.coverage, Coverage::InvalidJoin);
}

#[test]
fn stale_pending_row_cannot_grow_the_pre_reserved_capacity() {
    let samples = fixtures::raw();
    let mut retained = Retention::new(Some(fixtures::origin_limits(1)));
    let first = retained.prepare(0, &samples[0].0, samples[0].1);
    let stale = retained.prepare(0, &samples[0].0, samples[0].1);
    let bytes = retained.metadata_bytes();
    retained.commit(first, Control::Continue, 1);
    retained.commit(stale, Control::Continue, 1);
    assert_eq!(retained.coverage, Coverage::InvalidJoin);
    assert_eq!(retained.rows.len(), 1);
    assert_eq!(retained.metadata_bytes(), bytes);
}

#[test]
fn every_runtime_unavailability_is_retained_without_tokens() {
    let samples = fixtures::raw();
    for reason in [
        RuntimeMissing::NotRequested,
        RuntimeMissing::NoMatchingOperation,
        RuntimeMissing::AggregateRecord,
        RuntimeMissing::IdentityInvariant,
    ] {
        let mut retained = Retention::new(Some(fixtures::origin_limits(1)));
        let pending = retained.prepare(0, &samples[0].0, Context::Unavailable(reason));
        retained.commit(pending, Control::Continue, 1);
        assert_eq!(
            retained.rows,
            [Row {
                activation: 0,
                attempt: 0,
                status: Status::RuntimeUnavailable(reason),
            }]
        );
    }
}

#[test]
fn row_and_byte_cutoffs_preserve_all_legacy_records_and_simulation_results() {
    let (module, request) = fixtures::memory();
    let (expected, baseline) = fixtures::legacy(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        4096,
    );
    for (limits, cutoff) in [
        (fixtures::origin_limits(1), Cutoff::RowLimit),
        (
            Limits::new(2, size_of::<Retention>() + size_of::<Row>()).unwrap(),
            Cutoff::ByteLimit,
        ),
    ] {
        let (actual, observed) = fixtures::observed(
            &module,
            &request,
            fixtures::simulation_limits(),
            false,
            Some(limits),
            4096,
        );
        fixtures::assert_result_eq(&actual, &expected);
        assert_eq!(observed.transcript, baseline);
        assert_eq!(observed.retained.rows.len(), 1);
        assert_eq!(
            observed.retained.coverage,
            Coverage::PrefixTruncated(cutoff)
        );
        assert!(observed.origin_at(0).is_ok());
        assert_eq!(
            observed.origin_at(1).unwrap_err(),
            Missing::PrefixTruncated(cutoff)
        );
        assert!(observed.retained.metadata_bytes() <= limits.bytes);
    }
    let mut sink = OriginCollector::with_retention(
        fixtures::debugger_limits(4096),
        Retention::with_reservation(Some(fixtures::origin_limits(2)), |_| Err(())),
    );
    let actual = fixtures::drive(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        &mut sink,
    );
    let observed = sink.finish(fixtures::identity(&module), DebugWaveWidthV1::Wave64, None);
    fixtures::assert_result_eq(&actual, &expected);
    assert_eq!(observed.transcript, baseline);
    assert_eq!(
        observed.origin_at(0).unwrap_err(),
        Missing::PrefixTruncated(Cutoff::AllocationFailure)
    );
}

#[test]
fn legacy_record_limit_does_not_create_a_false_metadata_truncation() {
    let (module, request) = fixtures::memory();
    let (expected, baseline) =
        fixtures::legacy(&module, &request, fixtures::simulation_limits(), false, 1);
    let (actual, observed) = fixtures::observed(
        &module,
        &request,
        fixtures::simulation_limits(),
        false,
        Some(fixtures::origin_limits(1)),
        1,
    );
    fixtures::assert_result_eq(&actual, &expected);
    assert_eq!(observed.transcript, baseline);
    assert_eq!(observed.retained.coverage, Coverage::Complete);
    assert_eq!(observed.retained.rows.len(), 1);
    assert_eq!(
        observed.paired_after(0, &mut ScanBudget::new(10).unwrap()),
        Err(PairError::Incomplete)
    );
}

#[test]
fn terminal_detail_callbacks_and_duplicate_detection_match_the_real_collector() {
    let (module, _) = fixtures::memory();
    let bounds = SimulationOutOfBoundsV2 {
        allocation: 1,
        offset: 4,
        bytes: 4,
        allocation_bytes: 4,
        legal_lower_bound: 0,
        legal_upper_bound: 4,
        abi_view: None,
    };
    let barrier = DivergentWorkgroupBarrierV2 {
        phase: 0,
        waiting: vec![WorkgroupParticipantV1 { local: [0, 0, 0] }],
        exited: vec![WorkgroupParticipantV1 { local: [1, 0, 0] }],
    };
    for mode in 0..3 {
        let mut legacy = TranscriptCollectorV1::new(fixtures::debugger_limits(1));
        let mut captured = OriginCollector::new(
            fixtures::debugger_limits(1),
            Some(fixtures::origin_limits(1)),
        );
        if mode != 1 {
            legacy.terminal_out_of_bounds_v2(bounds);
            captured.terminal_out_of_bounds_v2(bounds);
        }
        if mode != 0 {
            legacy.terminal_barrier_divergence_v2(barrier.clone());
            captured.terminal_barrier_divergence_v2(barrier.clone());
        }
        let expected =
            legacy.into_transcript(fixtures::identity(&module), DebugWaveWidthV1::Wave64, None);
        let observed = captured.finish(fixtures::identity(&module), DebugWaveWidthV1::Wave64, None);
        assert_eq!(observed.transcript, expected);
        match (mode, observed.transcript.terminal_detail_v2()) {
            (0, DebugTerminalDetailStateV2::Captured(DebugTerminalDetailV2::OutOfBounds(_)))
            | (
                1,
                DebugTerminalDetailStateV2::Captured(DebugTerminalDetailV2::BarrierDivergence(_)),
            )
            | (2, DebugTerminalDetailStateV2::InvalidMultiple) => {}
            other => panic!("unexpected terminal detail: {other:?}"),
        }
    }
}

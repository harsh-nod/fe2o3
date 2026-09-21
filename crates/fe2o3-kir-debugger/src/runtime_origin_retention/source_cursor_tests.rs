//! Actual-source acceptance requires the root's fresh, pinned normal export.
//! This test cannot establish source ownership from the supplied filename.
use super::session::*;
use super::session_tests::{before, breakpoint, stopped, watchpoint};
use super::source_cursor_fixture as input;
use super::source_cursor_topology::{Selected, select};
use super::*;
use crate::*;
use fe2o3_kernel_ir::VerifiedSimulationBundleV6;
use fe2o3_kir_sim::{SimulationDebugMemoryAccessV1 as Access, SimulationDebugRecordKindV1 as Kind};
use std::collections::BTreeSet;

fn exercise(
    mut session: ObservedSession,
    selected: Selected,
    rounds: u32,
) -> (u64, u64, usize, usize) {
    assert_eq!(session.origins.coverage, Coverage::Complete);
    assert!(session.origin_metadata_bytes() <= 4096 * size_of::<Row>() + 4096);
    let records = session.view().transcript().records();
    let focus = records
        .iter()
        .find(|r| r.invocation.global == [0, 0, 0])
        .unwrap()
        .invocation;
    assert_eq!(focus.workgroup_size, [64, 1, 1]);
    assert_eq!(focus.launch_extent, [4, 1, 1]);
    let mut activations = BTreeSet::new();
    let mut call_attempts = BTreeSet::new();
    let mut writes = Vec::new();
    for (index, record) in records.iter().enumerate() {
        let origin = session.origin_at(index).unwrap();
        if record.site == selected.helper && before(record) {
            assert_ne!(origin.row.activation, 1);
            assert!(activations.insert((record.invocation.global, origin.row.activation)));
        }
        if record.site == selected.call && before(record) {
            assert_eq!(origin.row.activation, 1);
            assert!(call_attempts.insert((record.invocation.global, origin.row.attempt)));
        }
        if let Kind::Memory {
            access: Access::WriteCommitted,
            allocation,
            byte_offset,
            byte_len,
            value,
            ..
        } = &record.kind
        {
            assert_eq!(*byte_len, 4);
            assert_eq!(
                *value,
                SimulationDebugValueV1::Scalar(ScalarBitsV1::u32(input::expected(rounds)))
            );
            writes.push((index, *allocation, *byte_offset, record.invocation));
        }
    }
    assert_eq!(activations.len(), 4 * rounds as usize);
    assert_eq!(call_attempts.len(), activations.len());
    assert_eq!(writes.len(), 4);
    assert!(writes.iter().all(|row| row.1 == writes[0].1));
    let offsets: BTreeSet<_> = writes.iter().map(|row| row.2).collect();
    assert_eq!(offsets, BTreeSet::from([4, 8, 12, 16]));
    let watched = *writes.iter().find(|row| row.3 == focus).unwrap();
    assert_eq!(watched.2, 4);
    let mut work = ReplayWork::new(MAX_WORK).unwrap();
    session
        .add_breakpoint(breakpoint(1, selected.call, focus), &mut work)
        .unwrap();
    session
        .add_breakpoint(breakpoint(2, selected.helper, focus), &mut work)
        .unwrap();
    let mut write = watchpoint(3, watched.1, 4, focus);
    write.value_equals = Some(ScalarBitsV1::u32(input::expected(rounds)));
    session.add_watchpoint(write, &mut work).unwrap();
    // Canary watches cover real allocation-relative ranges, not synthetic IDs.
    session
        .add_watchpoint(watchpoint(4, watched.1, 0, focus), &mut work)
        .unwrap();
    session
        .add_watchpoint(watchpoint(5, watched.1, 20, focus), &mut work)
        .unwrap();
    for _ in 0..rounds {
        let call = stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work)
                .unwrap(),
            DebugStopReasonV1::Breakpoint(1),
        );
        assert_eq!(session.current_origin().unwrap().record.site, selected.call);
        let caller_origin = *session.current_origin().unwrap().row;
        let helper = stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work)
                .unwrap(),
            DebugStopReasonV1::Breakpoint(2),
        );
        let helper_origin = *session.current_origin().unwrap().row;
        assert_ne!(helper_origin.activation, caller_origin.activation);
        assert_eq!(session.current_origin().unwrap().record.invocation, focus);
        session
            .step_into(Direction::Forward, focus, &mut work)
            .unwrap();
        assert_eq!(*session.current_origin().unwrap().row, helper_origin);
        assert_eq!(
            stopped(
                session
                    .step_into(Direction::Reverse, focus, &mut work)
                    .unwrap(),
                DebugStopReasonV1::Step
            ),
            helper
        );
        session
            .step_over(Direction::Forward, focus, &mut work)
            .unwrap();
        assert_eq!(*session.current_origin().unwrap().row, helper_origin);
        assert_eq!(
            stopped(
                session
                    .step_over(Direction::Reverse, focus, &mut work)
                    .unwrap(),
                DebugStopReasonV1::Step
            ),
            helper
        );
        session.seek_record(call, &mut work).unwrap();
        let after = stopped(
            session
                .step_over(Direction::Forward, focus, &mut work)
                .unwrap(),
            DebugStopReasonV1::Step,
        );
        assert!(after > helper);
        assert_eq!(*session.current_origin().unwrap().row, caller_origin);
        assert_eq!(
            stopped(
                session
                    .step_over(Direction::Reverse, focus, &mut work)
                    .unwrap(),
                DebugStopReasonV1::Step
            ),
            call
        );
        assert_eq!(
            stopped(
                session
                    .step_over(Direction::Forward, focus, &mut work)
                    .unwrap(),
                DebugStopReasonV1::Step
            ),
            after
        );
    }
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work)
                .unwrap(),
            DebugStopReasonV1::Watchpoint(3)
        ),
        watched.0
    );
    let write_origin = *session.current_origin().unwrap().row;
    let store_before = session
        .view()
        .transcript()
        .records()
        .iter()
        .enumerate()
        .find(|(index, record)| {
            record.invocation == focus
                && before(record)
                && session
                    .origin_at(*index)
                    .is_ok_and(|origin| *origin.row == write_origin)
        })
        .map(|(index, _)| index)
        .unwrap();
    session.seek_record(store_before, &mut work).unwrap();
    let store_after = stopped(
        session
            .step_over(Direction::Forward, focus, &mut work)
            .unwrap(),
        DebugStopReasonV1::Step,
    );
    assert!(store_before < watched.0 && watched.0 < store_after);
    assert_eq!(*session.current_origin().unwrap().row, write_origin);
    session
        .step_over(Direction::Reverse, focus, &mut work)
        .unwrap();
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Forward, &mut work)
                .unwrap(),
            DebugStopReasonV1::Watchpoint(3)
        ),
        watched.0
    );
    assert_eq!(
        session.view().breakpoint_hit_count(1),
        Some(u64::from(rounds))
    );
    assert_eq!(
        session.view().breakpoint_hit_count(2),
        Some(u64::from(rounds))
    );
    assert_eq!(session.view().watchpoint_hit_count(3), Some(1));
    assert_eq!(session.view().watchpoint_hit_count(4), Some(0));
    assert_eq!(session.view().watchpoint_hit_count(5), Some(0));
    let break_hits = session.view().breakpoint_hit_count(1).unwrap()
        + session.view().breakpoint_hit_count(2).unwrap();
    let watch_hits = session.view().watchpoint_hit_count(3).unwrap();
    assert_eq!(
        session
            .continue_to_stop(Direction::Forward, &mut work)
            .unwrap(),
        DebugNavigationV1::End
    );
    assert_eq!(
        stopped(
            session
                .continue_to_stop(Direction::Reverse, &mut work)
                .unwrap(),
            DebugStopReasonV1::Watchpoint(3)
        ),
        watched.0
    );
    assert_eq!(*session.current_origin().unwrap().row, write_origin);
    for remaining in (1..=rounds).rev() {
        stopped(
            session
                .continue_to_stop(Direction::Reverse, &mut work)
                .unwrap(),
            DebugStopReasonV1::Breakpoint(2),
        );
        assert_eq!(
            session.view().breakpoint_hit_count(2),
            Some(u64::from(remaining))
        );
        stopped(
            session
                .continue_to_stop(Direction::Reverse, &mut work)
                .unwrap(),
            DebugStopReasonV1::Breakpoint(1),
        );
        assert_eq!(
            session.view().breakpoint_hit_count(1),
            Some(u64::from(remaining))
        );
    }
    assert_eq!(
        session
            .continue_to_stop(Direction::Reverse, &mut work)
            .unwrap(),
        DebugNavigationV1::Beginning
    );
    for id in [1, 2] {
        assert_eq!(session.view().breakpoint_hit_count(id), Some(0));
    }
    for id in [3, 4, 5] {
        assert_eq!(session.view().watchpoint_hit_count(id), Some(0));
    }
    assert!(work.remaining() < MAX_WORK);
    (
        break_hits,
        watch_hits,
        activations.len(),
        session.view().transcript().records().len(),
    )
}

#[test]
#[ignore = "requires fresh root-owned normal source export and exact bundle identity"]
fn actual_source_loop_helper_cursor() {
    let path =
        std::env::var("FE2O3_TEST_ORIGIN_CURSOR_BUNDLE").expect("fresh bundle path required");
    let identity = std::env::var("FE2O3_TEST_ORIGIN_CURSOR_BUNDLE_IDENTITY")
        .expect("bundle model identity required");
    let expected_identity = input::input_parameters(&path, &identity).unwrap();
    let bundle = VerifiedSimulationBundleV6::from_canonical_bytes(input::read(&path)).unwrap();
    assert_eq!(
        bundle.identity().as_bytes(),
        &expected_identity,
        "not raw SHA or canonical KIR digest"
    );
    let module = input::admit(&bundle);
    // No simulation until the actual decoded owner has the retained call/cycle.
    let selected = select(module.module()).expect("retained source loop/helper topology");
    let (mut contextual, mut opt_out, mut breaks, mut watches, mut activations, mut records) =
        (0, 0, 0, 0, 0, 0);
    for rounds in [0, 1, 3] {
        for seeded in [false, true] {
            let request = input::request(rounds);
            let (execution, observed) = input::run(&module, &request, seeded, true);
            contextual += 1;
            let (baseline, disabled) = input::run(&module, &request, seeded, false);
            opt_out += 1;
            assert_eq!(execution, baseline);
            assert_eq!(observed.transcript, disabled.transcript);
            assert_eq!(disabled.retained.rows.capacity(), 0);
            assert_eq!(disabled.origin_at(0).unwrap_err(), Missing::Disabled);
            input::check_outputs(&execution, &request, input::expected(rounds));
            let summary = exercise(observed.into_session(), selected, rounds);
            breaks += summary.0;
            watches += summary.1;
            activations += summary.2;
            records += summary.3;
        }
    }
    assert_eq!(
        (contextual, opt_out, breaks, watches, activations),
        (6, 6, 16, 6, 32)
    );
    assert!(records > 0 && records <= 6 * 4096);
    assert_eq!(
        input::read(&path),
        bundle.canonical_bytes(),
        "input changed during actual-source cursor qualification"
    );
    // A bounded test marker, NOT a serialized debugger/source authority contract.
    println!(
        "ORIGIN_CURSOR_SOURCE_OK bundle_identity={identity} contextual_runs={contextual} opt_out_runs={opt_out} breakpoint_hits={breaks} watchpoint_hits={watches} helper_activations={activations} retained_records={records}"
    );
}

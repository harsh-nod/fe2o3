//! Actual interpreter controls over synthetic KIR. No source or storage-reuse claim.
use super::super::session::Direction;
use super::*;
use crate::{DebugNavigationV1, DebugStopReasonV1};
use fe2o3_kir_sim::SimulationDebugRecordKindV1 as Kind;

#[test]
fn actual_private_create_initialize_release_and_terminal_n_are_same_stop_reversible() {
    for seeded in [false, true] {
        let (module, request) = fixtures::private(false, false);
        let (expected, legacy) =
            fixtures::baseline(&module, &request, seeded, fixtures::config(4096));
        let (result, mut owner) = capture(
            &module,
            &request,
            fixtures::schedule(seeded),
            fixtures::config(4096),
        );
        super::super::fixtures::assert_result_eq(&result, &expected);
        assert_eq!(owner.view().transcript(), &legacy);
        let execution = result.unwrap();
        let expected_bytes: Vec<_> = [42_u32, 0xdeadbeef, 0xcafebabe]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect();
        assert_eq!(execution.buffer(0).unwrap().bytes(), expected_bytes);
        assert_eq!(execution.buffer(0).unwrap().initialized(), [true; 12]);
        assert!(owner.metadata_bytes() < 1024 * 1024);
        let transitions = owner.transitions().to_vec();
        let created: Vec<_> = transitions
            .iter()
            .filter(|row| row.action == Action::Create)
            .collect();
        assert_eq!(created.len(), 3);
        assert!(
            created
                .windows(2)
                .all(|pair| pair[0].allocation < pair[1].allocation)
        );
        assert_eq!(
            transitions
                .iter()
                .filter(|row| row.action == Action::Release)
                .count(),
            3
        );
        let count = owner.view().transcript().records().len();
        let root = created.last().unwrap().allocation;
        assert_eq!(transitions.last().unwrap().boundary, count);
        assert_eq!(transitions.last().unwrap().allocation, root);
        owner.seek_record(count - 1, &mut fixtures::work()).unwrap();
        assert_eq!(
            owner
                .current_allocation(root, &mut fixtures::work())
                .unwrap()
                .state,
            State::CreatedLive
        );
        assert_eq!(
            owner
                .continue_to_stop(Direction::Forward, &mut fixtures::work())
                .unwrap(),
            DebugNavigationV1::End
        );
        assert_eq!(
            owner
                .current_allocation(root, &mut fixtures::work())
                .unwrap()
                .state,
            State::Released
        );
        let focus = owner.view().transcript().records()[0].invocation;
        owner
            .step_into(Direction::Reverse, focus, &mut fixtures::work())
            .unwrap();
        assert_eq!(
            owner
                .current_allocation(root, &mut fixtures::work())
                .unwrap()
                .state,
            State::CreatedLive
        );

        let first = created[0].allocation;
        let create = created[0].boundary;
        let release = transitions
            .iter()
            .find(|row| row.allocation == first && row.action == Action::Release)
            .unwrap()
            .boundary;
        assert!(create > 0 && release < count);
        for index in [create - 1, create, release, create, create - 1] {
            owner.seek_record(index, &mut fixtures::work()).unwrap();
            let projected = owner
                .current_allocation(first, &mut fixtures::work())
                .unwrap();
            assert!(std::ptr::eq(
                projected.record.unwrap(),
                owner.view().current().unwrap()
            ));
            assert_eq!(format!("{:?}", projected.generation), "NotRepresented");
            assert_eq!(
                format!("{:?}", projected.frame_activation),
                "NotRepresented"
            );
            assert_eq!(
                projected.state,
                if index < create {
                    State::NotYetObserved
                } else if index < release {
                    State::CreatedLive
                } else {
                    State::Released
                }
            );
            if index == create {
                assert_eq!(projected.checkpoint.unwrap().initialized, [false; 4]);
            }
        }
        // Snapshot bytes, not access values, supply initialization at this stop.
        let initialized = owner.view().transcript().records().iter().position(|record| {
            matches!(&record.kind, Kind::Checkpoint { memory: fe2o3_kir_sim::SimulationDebugCollectionV1::Captured(memory), .. }
                if memory.iter().any(|a| a.allocation == first && a.initialized == [true; 4]))
        }).unwrap();
        owner
            .seek_record(initialized, &mut fixtures::work())
            .unwrap();
        assert_eq!(
            owner
                .current_allocation(first, &mut fixtures::work())
                .unwrap()
                .checkpoint
                .unwrap()
                .bytes,
            7_u32.to_le_bytes()
        );
    }
}

#[test]
fn actual_workgroups_have_separate_ids_and_scopes_not_reuse_generations() {
    let (module, request) = fixtures::workgroups();
    for seeded in [false, true] {
        let config = || {
            let mut c = fixtures::config(4096);
            c.simulation.max_invocations = 4;
            c.simulation.max_workgroups = 2;
            c
        };
        let (expected, legacy) = fixtures::baseline(&module, &request, seeded, config());
        let (result, mut owner) = capture(&module, &request, fixtures::schedule(seeded), config());
        super::super::fixtures::assert_result_eq(&result, &expected);
        assert!(result.is_ok());
        assert_eq!(owner.view().transcript(), &legacy);
        let created: Vec<_> = owner
            .transitions()
            .iter()
            .filter(|r| r.action == Action::Create)
            .copied()
            .collect();
        assert_eq!(created.len(), 2);
        assert_ne!(created[0].allocation, created[1].allocation);
        assert_ne!(created[0].scope, created[1].scope);
        for row in created {
            assert!(matches!(row.scope, Scope::Workgroup { .. }));
            let release = owner
                .transitions()
                .iter()
                .find(|r| r.action == Action::Release && r.allocation == row.allocation)
                .copied()
                .unwrap();
            assert_eq!(release.scope, row.scope);
            // Exact retained checkpoint, not an aggregate barrier observation.
            let selected = owner
                .view()
                .transcript()
                .records()
                .iter()
                .enumerate()
                .skip(row.boundary)
                .find(|(i, r)| *i < release.boundary && matches!(r.kind, Kind::Checkpoint { .. }))
                .unwrap()
                .0;
            owner.seek_record(selected, &mut fixtures::work()).unwrap();
            assert_eq!(
                owner
                    .current_allocation(row.allocation, &mut fixtures::work())
                    .unwrap()
                    .state,
                State::CreatedLive
            );
        }
    }
}

#[test]
fn actual_fault_unwind_keeps_release_rows_without_claiming_completed_terminal_state() {
    let (module, request) = fixtures::private(true, false);
    let (expected, legacy) = fixtures::baseline(&module, &request, false, fixtures::config(4096));
    let (result, mut owner) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    super::super::fixtures::assert_result_eq(&result, &expected);
    assert!(result.is_err());
    assert_eq!(owner.view().transcript(), &legacy);
    let created = owner
        .transitions()
        .iter()
        .find(|row| row.action == Action::Create)
        .copied()
        .unwrap();
    assert!(
        owner
            .transitions()
            .iter()
            .any(|r| r.allocation == created.allocation && r.action == Action::Release)
    );
    let terminal = owner
        .continue_to_stop(Direction::Forward, &mut fixtures::work())
        .unwrap();
    assert!(
        matches!(terminal, DebugNavigationV1::Stopped(stop) if stop.reason == DebugStopReasonV1::Fault)
    );
    assert!(matches!(
        owner.current_allocation(created.allocation, &mut fixtures::work()),
        Err(QueryError::Gap(Gap::ExecutionFailure))
    ));
    assert_eq!(
        owner
            .continue_to_stop(Direction::Forward, &mut fixtures::work())
            .unwrap(),
        DebugNavigationV1::End
    );
}

#[test]
fn zero_byte_allocations_have_real_create_release_without_bytes_or_generation() {
    let (module, request) = fixtures::private(false, true);
    let (result, owner) = capture(
        &module,
        &request,
        fixtures::schedule(false),
        fixtures::config(4096),
    );
    assert!(result.is_ok());
    let zero: Vec<_> = owner
        .transitions()
        .iter()
        .filter(|row| row.action == Action::Create && row.bytes == 0)
        .collect();
    assert_eq!(zero.len(), 2);
    assert!(zero.iter().all(|created| {
        owner
            .transitions()
            .iter()
            .any(|row| row.allocation == created.allocation && row.action == Action::Release)
    }));
}

#[test]
fn direct_nested_and_recursive_faults_retain_each_actual_release() {
    for shape in 0..3 {
        let (module, request) = fixtures::failure_shape(shape);
        let config = || {
            let mut c = fixtures::config(4096);
            c.simulation.max_call_depth = 4;
            c
        };
        let (expected, legacy) = fixtures::baseline(&module, &request, false, config());
        let (result, owner) = capture(&module, &request, fixtures::schedule(false), config());
        super::super::fixtures::assert_result_eq(&result, &expected);
        assert!(
            matches!(result, Err(fe2o3_kir_sim::SimulationErrorV1::Execution(error))
            if matches!(error.kind, fe2o3_kir_sim::SimulationExecutionErrorKindV1::ReachedUnreachable
                | fe2o3_kir_sim::SimulationExecutionErrorKindV1::CallDepthLimit { .. }))
        );
        assert_eq!(owner.view().transcript(), &legacy);
        let creates: Vec<_> = owner
            .transitions()
            .iter()
            .filter(|r| r.action == Action::Create)
            .map(|r| r.allocation)
            .collect();
        let releases: Vec<_> = owner
            .transitions()
            .iter()
            .filter(|r| r.action == Action::Release)
            .map(|r| r.allocation)
            .collect();
        assert!(!creates.is_empty());
        assert_eq!(releases, creates.into_iter().rev().collect::<Vec<_>>());
    }
}

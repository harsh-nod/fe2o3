//! Ignored retained-source acceptance. Root must bind a fresh normal export,
//! source/binary/census receipts before this test; a path/hash is not source custody.
use super::super::{session::Direction, source_cursor_fixture as input};
use super::*;
use crate::{DebugNavigationV1, DebuggerLimitsV1};
use fe2o3_kernel_ir::*;
use fe2o3_kir_sim::*;

fn config() -> capture::Configuration {
    let mut c = fixtures::config(131_072);
    c.simulation = SimulationLimitsV1 {
        max_canonical_bytes: input::CAP,
        max_reachable_functions: 8,
        max_reachable_operations: 512,
        max_invocations: 128,
        max_workgroups: 2,
        max_scheduled_slots: 128,
        max_steps: 65_536,
        max_call_depth: 8,
        max_ssa_values: 1024,
        max_allocations: 8,
        max_allocation_bytes: 4096,
        max_total_bytes: 16 * 1024,
        max_resident_bytes: 256 * 1024 * 1024,
        max_events: 262_144,
        max_memory_access_records: 32_768,
    };
    c.debugger = DebuggerLimitsV1::new(131_072, 4_000_000, 256 * 1024 * 1024).unwrap();
    c.capture = SimulationDebugCaptureLimitsV1::new(8, 1024, 8, 4096).unwrap();
    c.lifecycle = LifecycleLimits::new(
        32,
        size_of::<Ledger>() + 32 * size_of::<Transition>(),
        1_000_000,
    )
    .unwrap();
    c
}
fn schedule(seeded: bool) -> SimulationScheduleRequestV1<'static> {
    if seeded {
        SimulationScheduleRequestV1::RecordSeeded {
            seed: 71,
            max_decisions: 65_536,
        }
    } else {
        SimulationScheduleRequestV1::RecordCanonical {
            max_decisions: 65_536,
        }
    }
}
fn request(groups: usize) -> SimulationRequestV1 {
    let mut words = vec![ScalarBitsV1::u32(0xa5a5a5a5); groups * 64];
    words.extend([ScalarBitsV1::u32(0xdeadbeef), ScalarBitsV1::u32(0xcafebabe)]);
    let mut r = SimulationRequestV1::new(
        "workgroup_reduce_u32",
        [(groups * 64) as u64, 1, 1],
        [64, 1, 1],
        vec![
            SimulationArgumentV1::Scalar(ScalarBitsV1::u32(2)),
            SimulationArgumentV1::Buffer(
                BufferArgumentV1::from_scalars(AccessMode::ReadWrite, 4, &words, config().target)
                    .unwrap(),
            ),
        ],
    );
    r.events = EventPolicyV1::Enabled;
    r
}
fn work() -> ReplayWork {
    ReplayWork::new(super::super::session::MAX_WORK).unwrap()
}

#[test]
#[ignore = "requires fresh root-owned ordinary workgroup_reduce_u32 Bundle V5 export"]
fn actual_source_workgroup_create_release_same_stop() {
    let path = std::env::var("FE2O3_TEST_LIFECYCLE_BUNDLE").expect("fresh Bundle V5 path");
    let identity =
        std::env::var("FE2O3_TEST_LIFECYCLE_BUNDLE_IDENTITY").expect("typed bundle identity");
    let expected = input::input_parameters(&path, &identity).unwrap();
    let bytes = input::read(&path);
    let bundle = VerifiedSimulationBundleV5::from_canonical_bytes(bytes.clone()).unwrap();
    assert_eq!(
        bundle.identity().as_bytes(),
        &expected,
        "model identity, not raw SHA"
    );
    bundle.revalidate().unwrap();
    assert_eq!(bundle.target(), "gfx942:xnack-");
    assert_eq!(bundle.kernel_count(), 1);
    let canonical =
        VerifiedCanonicalKernelIrV10::from_canonical_bytes(bundle.canonical_kir_v10().to_vec())
            .unwrap();
    let module = AdmittedSimulationModuleV1::admit_v10(canonical, config().simulation).unwrap();
    let mut declarations = 0;
    let mut operations = 0;
    for function in &module.module().functions {
        if let Some(body) = &function.body {
            for block in &body.blocks {
                for operation in &block.operations {
                    operations += 1;
                    assert!(!matches!(operation.kind, OperationKind::Alloca { .. }));
                    if let OperationKind::WorkgroupMemory(memory) = &operation.kind {
                        assert_eq!(memory.element, Type::Scalar(ScalarType::U32));
                        assert_eq!(memory.extent, WorkgroupMemoryExtent::Static(64));
                        assert_eq!(memory.alignment, 4);
                        declarations += 1;
                    }
                }
            }
        }
    }
    assert_eq!(
        declarations, 1,
        "require actual retained LDS declaration before simulation"
    );
    assert!(operations > 0 && operations <= 512);
    let mut cases = 0;
    for groups in [1_usize, 2] {
        for seeded in [false, true] {
            let request = request(groups);
            // This same-event-enabled legacy baseline changes no debug behavior.
            let mut legacy = crate::TranscriptCollectorV1::new(config().debugger);
            let c = config();
            let baseline = module
                .simulate_debugged_scheduled_with_sink(
                    &request,
                    c.target,
                    c.simulation,
                    schedule(seeded),
                    c.capture,
                    &mut legacy,
                )
                .unwrap();
            let legacy =
                legacy.into_transcript(super::super::fixtures::identity(&module), c.width, None);
            let (result, mut owner) = capture(&module, &request, schedule(seeded), config());
            let execution = result.unwrap();
            assert_eq!(execution, baseline);
            assert_eq!(owner.view().transcript(), &legacy);
            let mut expected = vec![128_u32; groups * 64];
            expected.extend([0xdeadbeef, 0xcafebabe]);
            let expected: Vec<_> = expected.into_iter().flat_map(u32::to_le_bytes).collect();
            assert_eq!(execution.buffer(1).unwrap().bytes(), expected);
            assert!(
                execution
                    .buffer(1)
                    .unwrap()
                    .initialized()
                    .iter()
                    .all(|v| *v)
            );
            assert_eq!(execution.invocations_executed(), (groups * 64) as u64);
            assert_eq!(execution.workgroups_visited(), groups as u64);
            let created: Vec<_> = owner
                .transitions()
                .iter()
                .filter(|r| r.action == Action::Create)
                .copied()
                .collect();
            assert_eq!(created.len(), groups);
            let release_count = owner
                .transitions()
                .iter()
                .filter(|r| r.action == Action::Release)
                .count();
            assert_eq!(release_count, groups);
            for (group, row) in created.iter().enumerate() {
                assert_eq!(row.address_space, AddressSpace::Workgroup);
                assert_eq!(row.bytes, 256);
                assert!(
                    matches!(row.scope, Scope::Workgroup { coordinate, .. } if coordinate == [group as u64,0,0])
                );
                let release = owner
                    .transitions()
                    .iter()
                    .find(|r| r.action == Action::Release && r.allocation == row.allocation)
                    .copied()
                    .unwrap();
                let at = owner
                    .view()
                    .transcript()
                    .records()
                    .iter()
                    .enumerate()
                    .skip(row.boundary)
                    .find(|(i, r)| {
                        *i < release.boundary
                            && matches!(r.kind, SimulationDebugRecordKindV1::Checkpoint { .. })
                    })
                    .unwrap()
                    .0;
                owner.seek_record(at, &mut work()).unwrap();
                let before = owner
                    .current_allocation(row.allocation, &mut work())
                    .unwrap();
                assert_eq!(before.state, State::CreatedLive);
                let initialization = before.checkpoint.unwrap().initialized.clone();
                owner
                    .continue_to_stop(Direction::Forward, &mut work())
                    .unwrap();
                assert_eq!(
                    owner
                        .current_allocation(row.allocation, &mut work())
                        .unwrap()
                        .state,
                    State::Released
                );
                owner.seek_record(at, &mut work()).unwrap();
                assert_eq!(
                    owner
                        .current_allocation(row.allocation, &mut work())
                        .unwrap()
                        .checkpoint
                        .unwrap()
                        .initialized,
                    initialization
                );
            }
            assert_eq!(
                owner
                    .continue_to_stop(Direction::Forward, &mut work())
                    .unwrap(),
                DebugNavigationV1::End
            );
            cases += 1;
        }
    }
    assert_eq!(input::read(&path), bytes, "retained input changed");
    eprintln!(
        "lifecycle-source-v1: cases={cases} lifecycle_pairs=6 generation=not_represented frame_activation=not_represented"
    );
}

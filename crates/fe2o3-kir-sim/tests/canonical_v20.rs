//! Actual canonical V20 graph CPU controls. Inert origin bytes are not source
//! authentication; short slices do not satisfy the separate formal launch contract.
#[path = "canonical_v20/fixtures.rs"]
mod fixtures;
use fe2o3_kernel_ir::{
    AccessMode, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Module, ScalarType,
    VerifiedCanonicalKernelIrModuleV20 as Owner,
};
use fe2o3_kir_sim::*;
fn owner(module: &Module) -> Owner {
    let mut work = Work::new(16_000_000);
    let mut budget = Budget::new(&mut work, 16_000_000);
    budget.reserve_storage(97).unwrap();
    let (owner, receipt) =
        Owner::from_module_ref_with_verification_budget_v20(module, &mut budget).unwrap();
    assert_eq!(budget.storage(), 97);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    owner
}
fn admit(owner: &Owner) -> AdmittedSimulationModuleV1 {
    AdmittedSimulationModuleV1::admit_v20(owner, SimulationLimitsV1::default()).unwrap()
}
fn request(input: [u32; 3], selector: u32, length: usize, grid: u64) -> SimulationRequestV1 {
    let target = SimulationTargetV1::amdgpu_64();
    let id = BufferBackingIdV1(7);
    let bytes = (length + 4) * 4;
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        vec![0x5a; bytes],
        vec![false; bytes],
        target,
    )
    .unwrap();
    let view = BufferViewArgumentV1::new(
        id,
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        8,
        length,
        target,
    )
    .unwrap();
    let mut args = vec![SimulationArgumentV1::BufferView(view)];
    args.extend(
        input
            .into_iter()
            .chain([selector])
            .map(|x| SimulationArgumentV1::Scalar(ScalarBitsV1::u32(x))),
    );
    SimulationRequestV1::new("physical_fixture", [grid, 1, 1], [64, 1, 1], args)
        .with_shared_buffers(vec![SharedBufferV1 { id, buffer }])
}
fn output(execution: &SimulationExecutionV1, expected: u32, length: usize, grid: u64) {
    let output = execution.shared_buffer(BufferBackingIdV1(7)).unwrap();
    let written = length.min(grid as usize);
    for (index, word) in output.bytes().chunks_exact(4).enumerate() {
        let write = (2..2 + written).contains(&index);
        assert_eq!(
            word,
            if write {
                expected.to_le_bytes()
            } else {
                [0x5a; 4]
            },
            "word {index}"
        );
        assert!(
            output.initialized()[index * 4..index * 4 + 4]
                .iter()
                .all(|value| *value == write)
        );
    }
    assert!(!execution.grants_execution_authority());
}
#[test]
fn one_and_diamond_keep_actual_v20_identity_and_immutable_input() {
    for select in [false, true] {
        let source = fixtures::module(select);
        let owner = owner(&source);
        let bytes = owner.canonical_bytes().to_vec();
        let admitted = admit(&owner);
        assert_eq!(admitted.identity().wire_version(), 20);
        assert_eq!(admitted.identity().digest(), owner.identity().digest());
        assert_eq!(admitted.module(), owner.module());
        assert_ne!(
            admitted.module().functions.as_ptr(),
            owner.module().functions.as_ptr()
        );
        let request = request([19, 23, 42], 0, 65, 128);
        let original = request.clone();
        let execution = admitted
            .simulate(
                &request,
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        output(&execution, 19, 65, 128);
        assert_eq!(request, original);
        assert_eq!(owner.canonical_bytes(), bytes);
    }
}
#[test]
fn zero_partial_full_masks_and_two_workgroups_preserve_canaries() {
    // CPU semantics only: short buffers remain inadmissible to a conservative
    // production 512-byte formal bound; no runtime admission occurs in this test.
    let mut cases = 0;
    for select in [false, true] {
        let admitted = admit(&owner(&fixtures::module(select)));
        for inputs in [[0, 1, 2], [u32::MAX, 0, 0x8000_0000], [19, 23, 42]] {
            for selector in [0, 1, u32::MAX] {
                for length in [0, 1, 63, 64, 65, 127, 128, 129] {
                    for grid in [64, 128] {
                        let execution = admitted
                            .simulate(
                                &request(inputs, selector, length, grid),
                                SimulationTargetV1::amdgpu_64(),
                                SimulationLimitsV1::default(),
                            )
                            .unwrap();
                        output(
                            &execution,
                            if select && selector != 0 {
                                inputs[1]
                            } else {
                                inputs[0]
                            },
                            length,
                            grid,
                        );
                        assert_eq!(
                            execution.steps_executed(),
                            grid * if select { 25 } else { 22 }
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 288);
}
#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        assert!(self.0.len() < 8192);
        self.0.push(event.clone());
        Ok(())
    }
}
#[test]
fn selector_uses_real_edges_and_masked_lanes_emit_no_write() {
    let admitted = admit(&owner(&fixtures::module(true)));
    for selector in [0, 1, u32::MAX] {
        let mut events = Events::default();
        let execution = admitted
            .simulate_observed_with_sink(
                &request([19, 23, 42], selector, 63, 64),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
                &mut events,
            )
            .unwrap();
        output(&execution, if selector == 0 { 19 } else { 23 }, 63, 64);
        for lane in 0..64 {
            let entered: Vec<_> = events
                .0
                .iter()
                .filter(|event| {
                    event.invocation.global[0] == lane
                        && event.kind == SimulationEventKindV1::BlockEnter
                })
                .map(|event| event.site.block)
                .collect();
            assert_eq!(
                entered,
                vec![
                    BlockId(0),
                    BlockId(if selector == 0 { 2 } else { 1 }),
                    BlockId(3)
                ]
            );
            let writes = events
                .0
                .iter()
                .filter(|event| {
                    event.invocation.global[0] == lane
                        && matches!(event.kind, SimulationEventKindV1::MemoryWrite { .. })
                })
                .count();
            assert_eq!(writes, usize::from(lane < 63));
        }
    }
}
#[derive(Default)]
struct Records(usize);
impl SimulationDebugSinkV1 for Records {
    fn record(&mut self, _: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        self.0 += 1;
        SimulationDebugSinkControlV1::Continue
    }
}
#[test]
fn public_capture_and_checkpoint_routes_refuse_before_any_record() {
    let admitted = admit(&owner(&fixtures::module(true)));
    let request = request([19, 23, 42], 0, 64, 64);
    for capture in [
        SimulationDebugCaptureLimitsV1::disabled(),
        SimulationDebugCaptureLimitsV1::new(8, 768, 8, 4096).unwrap(),
    ] {
        let mut records = Records::default();
        let result = admitted.simulate_debugged_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            capture,
            &mut records,
        );
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20
            ))
        ));
        let result = admitted.simulate_debugged_scheduled_with_sink(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordCanonical { max_decisions: 256 },
            capture,
            &mut records,
        );
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20
            ))
        ));
        let mut events = Events::default();
        let result = admitted.simulate_debugged_with_observation_options(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            ObservationExecutionOptionsV1::new(capture),
            &mut events,
            &mut records,
        );
        assert!(matches!(
            result,
            Err(SimulationErrorV1::Preflight(
                SimulationPreflightErrorV1::PhysicalEntrySymbolicDebugUnavailableV20
            ))
        ));
        assert_eq!(records.0, 0);
        assert!(events.0.is_empty());
    }
}
#[test]
fn seeded_schedule_and_exact_replay_preserve_symbolic_memory_semantics() {
    let admitted = admit(&owner(&fixtures::module(true)));
    let request = request([u32::MAX, 23, 42], 1, 65, 128);
    let execution = admitted
        .simulate_scheduled(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::RecordSeeded {
                seed: 42,
                max_decisions: 512,
            },
        )
        .unwrap();
    output(&execution, 23, 65, 128);
    let replay = admitted
        .simulate_scheduled(
            &request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            SimulationScheduleRequestV1::Replay(execution.schedule_record().unwrap()),
        )
        .unwrap();
    output(&replay, 23, 65, 128);
    assert_eq!(execution.steps_executed(), replay.steps_executed());
}
#[test]
fn launch_profile_and_accounted_storage_limits_remain_closed() {
    let admitted = admit(&owner(&fixtures::module(false)));
    let target = SimulationTargetV1::amdgpu_64();
    let limits = SimulationLimitsV1::default();
    for grid in [1, 63, 65, 192] {
        assert!(
            admitted
                .preflight(&request([1, 2, 3], 0, 192, grid), target, limits)
                .is_err()
        );
    }
    assert!(
        admitted
            .preflight(
                &request([1, 2, 3], 0, 64, 64),
                SimulationTargetV1::little_endian(IndexWidthV1::Bits32),
                limits
            )
            .is_err()
    );
    let request = request([1, 2, 3], 0, 64, 64);
    let plan = admitted.preflight(&request, target, limits).unwrap();
    let small = SimulationLimitsV1 {
        max_resident_bytes: plan.resident_bytes() - 1,
        ..limits
    };
    assert!(matches!(
        admitted.preflight(&request, target, small),
        Err(SimulationPreflightErrorV1::ResourceLimit {
            resource: "resident bytes",
            ..
        })
    ));
    let small = SimulationLimitsV1 {
        max_steps: 1,
        ..limits
    };
    assert!(matches!(
        admitted.simulate(&request, target, small),
        Err(SimulationErrorV1::Execution(_))
    ));
}
#[test]
fn physical_capability_rows_are_exact_v20_gfx942_symbolic_cpu_ownership() {
    let matrix = semantic_capability_matrix_v1();
    let mut rows = 0;
    for row in &matrix.top_level_rows {
        if row.kir_wire_version == SimulationKirWireVersionV1::V21 {
            continue;
        }
        if !matches!(
            row.operation,
            SimulationOperationSurfaceV1::PhysicalEntryDeclaration
                | SimulationOperationSurfaceV1::PhysicalEntryStep
        ) {
            continue;
        }
        rows += 1;
        if row.kir_wire_version == SimulationKirWireVersionV1::V20
            && row.profile == SimulationCapabilityProfileV1::Gfx942XnackMinus
        {
            assert!(matches!(
                row.capability,
                SimulationCapabilityDispositionV1::Owned {
                    owner: SimulationSemanticOwnerV1::SymbolicPhysicalEntry,
                    ..
                }
            ));
        } else {
            assert!(matches!(
                row.capability,
                SimulationCapabilityDispositionV1::Unsupported {
                    reason: SimulationUnsupportedReasonCodeV1::PhysicalEntryProfile
                }
            ));
        }
    }
    assert_eq!(rows, 4 * 9 * 2);
    assert!(!matrix.hardware_observed);
    assert_eq!(matrix.authority, "none");
}

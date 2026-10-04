use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12;
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

#[derive(Default)]
struct Effects(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Effects {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if matches!(
            event.kind,
            EventKind::MemoryRead { .. }
                | EventKind::MemoryWrite { .. }
                | EventKind::MemoryAtomic { .. }
                | EventKind::MemoryFence { .. }
                | EventKind::AllocationPreexisting { .. }
                | EventKind::AllocationCreated { .. }
                | EventKind::AllocationReleased { .. }
                | EventKind::WorkgroupBarrierArrive { .. }
                | EventKind::WorkgroupBarrierRelease { .. }
                | EventKind::Call { .. }
                | EventKind::Return
        ) {
            if self.0.len() == 256 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete test effects bound".into(),
                });
            }
            self.0.push(event.clone());
        }
        Ok(())
    }
}
fn admitted(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let canonical = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(canonical.identity(), owner.canonical().identity());
    assert_eq!(
        canonical.canonical_bytes(),
        owner.canonical().canonical_bytes()
    );
    AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default()).unwrap()
}

#[test]
fn actual_original_and_output_sim_preserve_cpu_oracle_guards_initialization_and_effect_sequence() {
    for incoming in [
        Incoming::Branch,
        Incoming::Conditional,
        Incoming::Switch,
        Incoming::IntegerSwitch,
        Incoming::Distinct,
    ] {
        with_input(fixture(incoming), |input, budget| {
            let owner = prepare_owned_loop_preheaders_v1(input, budget).unwrap();
            budget.reserve_storage(owner.retained_storage()).unwrap();
            let original = admitted(input);
            let output = admitted(owner.output());
            let target = SimulationTargetV1::amdgpu_64();
            for condition in [false, true] {
                for (x, y) in [(0u32, 7u32), (1, 2), (5, 3)] {
                    for limit in [0u32, 1, 6] {
                        let flipped = match incoming {
                            Incoming::Branch => false,
                            Incoming::Conditional | Incoming::Distinct => !condition,
                            Incoming::Switch | Incoming::IntegerSwitch => x == 1,
                        };
                        let (seed, payload) = if flipped { (y, x) } else { (x, y) };
                        let expected = seed.max(limit) ^ payload;
                        let request = SimulationRequestV1::new(
                            "loop",
                            [1, 1, 1],
                            [1, 1, 1],
                            vec![
                                SimulationArgumentV1::Scalar(ScalarBitsV1::boolean(condition)),
                                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(x)),
                                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(y)),
                                SimulationArgumentV1::Scalar(ScalarBitsV1::u32(limit)),
                                SimulationArgumentV1::Buffer(
                                    BufferArgumentV1::new(
                                        ScalarType::U32,
                                        AccessMode::ReadWrite,
                                        4,
                                        vec![0xa5; 12],
                                        vec![false; 12],
                                        target,
                                    )
                                    .unwrap(),
                                ),
                            ],
                        );
                        let unchanged = request.clone();
                        for _ in 0..2 {
                            let mut before_effects = Effects::default();
                            let mut after_effects = Effects::default();
                            let before = original
                                .simulate_observed_with_sink(
                                    &request,
                                    target,
                                    SimulationLimitsV1::default(),
                                    &mut before_effects,
                                )
                                .unwrap();
                            let after = output
                                .simulate_observed_with_sink(
                                    &request,
                                    target,
                                    SimulationLimitsV1::default(),
                                    &mut after_effects,
                                )
                                .unwrap();
                            assert_eq!(before.arguments(), after.arguments());
                            for result in [&before, &after] {
                                let buffer = result.buffer(4).unwrap();
                                assert_eq!(&buffer.bytes()[..4], &expected.to_le_bytes());
                                assert_eq!(&buffer.bytes()[4..], &[0xa5; 8]);
                                assert_eq!(&buffer.initialized()[..4], &[true; 4]);
                                assert_eq!(&buffer.initialized()[4..], &[false; 8]);
                                assert_eq!(result.invocations_executed(), 1);
                                assert!(!result.grants_execution_authority());
                            }
                            assert_eq!(before_effects.0, after_effects.0);
                            assert_eq!(
                                before_effects
                                    .0
                                    .iter()
                                    .filter(|e| matches!(e.kind, EventKind::MemoryWrite { .. }))
                                    .count(),
                                1
                            );
                            assert_eq!(before.conflict_assessment(), after.conflict_assessment());
                            if owner.preheaders().is_empty() {
                                assert_eq!(before.steps_executed(), after.steps_executed());
                            } else {
                                assert!(after.steps_executed() > before.steps_executed());
                            }
                        }
                        assert_eq!(request, unchanged);
                    }
                }
            }
            release(owner, budget);
        });
    }
}

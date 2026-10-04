use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1 as Pair;
use fe2o3_kernel_ir::{
    BlockId, CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirOperationCoordinateV1 as Site, LaunchExtent, VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, ScalarBitsV1, SimulationArgumentV1, SimulationEventKindV1 as Kind,
    SimulationEventSinkErrorV1, SimulationEventSinkV1, SimulationEventV1 as Event,
    SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
#[derive(Default)]
struct Events(Vec<Event>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &Event) -> Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 524_288 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete source-unroll trace bound".into(),
            });
        }
        self.0.push(event.clone());
        Ok(())
    }
}
fn simulation(owner: &Graph) -> AdmittedSimulationModuleV1 {
    let bytes = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(bytes.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(bytes, SimulationLimitsV1::default()).unwrap()
}
fn translated(pair: &Pair<'_, '_, '_>, events: &[Event]) -> Vec<Event> {
    let mut blocks = std::collections::BTreeMap::new();
    for row in pair.origins().blocks {
        if let Some(output) = row.output {
            assert!(blocks.insert(output, row.input).is_none());
        }
    }
    let mut operations = std::collections::BTreeMap::new();
    for row in pair.origins().operations {
        if let Some(output) = row.output {
            assert!(operations.insert(output, row.input).is_none());
        }
    }
    let coordinate = |function: usize, id: BlockId| {
        let body = pair.output().module().functions[function]
            .body
            .as_ref()
            .unwrap();
        Block {
            function: Function(function.try_into().unwrap()),
            block: body
                .blocks
                .iter()
                .position(|b| b.id == id)
                .unwrap()
                .try_into()
                .unwrap(),
        }
    };
    let original_id = |block: Block| {
        pair.input().module().functions[block.function.0 as usize]
            .body
            .as_ref()
            .unwrap()
            .blocks[block.block as usize]
            .id
    };
    events
        .iter()
        .map(|event| {
            let mut mapped = event.clone();
            let output = coordinate(event.site.function_ordinal, event.site.block);
            let original = blocks[&output];
            mapped.site.block = original_id(original);
            if let Some(operation) = event.site.operation {
                let source = operations[&Site {
                    block: output,
                    operation,
                }];
                assert_eq!(source.block, original);
                mapped.site.operation = Some(source.operation);
            }
            if let Kind::Branch { target } = &mut mapped.kind {
                *target = original_id(blocks[&coordinate(event.site.function_ordinal, *target)]);
            }
            mapped
        })
        .collect()
}
#[test]
fn source_unroll_sim_preserves_complete_events_sites_and_nonzero_private_effects() {
    let mut comparisons = 0usize;
    for shared in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for bound in [Some(0), Some(1), Some(3), Some(8), None] {
                with_unrolled(shared, profile, bound, |owner, budget| {
                    let floor = budget.storage();
                    let receipt = {
                        let (pair, receipt) = owner
                            .tail()
                            .replay(owner.input(), owner.limits(), budget)
                            .unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        let before = simulation(owner.input());
                        let after = simulation(owner.output());
                        for kernel in &owner.input().module().kernels {
                            let final_kernel = owner
                                .output()
                                .module()
                                .kernels
                                .iter()
                                .find(|v| v.id == kernel.id)
                                .unwrap();
                            assert_eq!(kernel, final_kernel);
                            let mut grid = [1; 3];
                            for (axis, extent) in kernel.domain.extents().enumerate() {
                                let LaunchExtent::Static(value) = extent else {
                                    panic!("actual source launch")
                                };
                                grid[axis] = value.into();
                            }
                            let group = kernel.workgroup_size.unwrap();
                            let group = [group.x, group.y, group.z];
                            assert_eq!((grid, group), ([64, 1, 1], [64, 1, 1]));
                            for control in [0, 1, 2, 3] {
                                let arguments =
                                    vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control))];
                                let request = SimulationRequestV1::new(
                                    kernel.id.as_str(),
                                    grid,
                                    group,
                                    arguments.clone(),
                                );
                                let unchanged = request.clone();
                                let trips = bound.unwrap_or(control) as usize;
                                let mut previous = None;
                                for _ in 0..2 {
                                    let mut a = Events::default();
                                    let mut b = Events::default();
                                    let old = before
                                        .simulate_observed_with_sink(
                                            &request,
                                            SimulationTargetV1::amdgpu_64(),
                                            SimulationLimitsV1::default(),
                                            &mut a,
                                        )
                                        .unwrap();
                                    let new = after
                                        .simulate_observed_with_sink(
                                            &request,
                                            SimulationTargetV1::amdgpu_64(),
                                            SimulationLimitsV1::default(),
                                            &mut b,
                                        )
                                        .unwrap();
                                    comparisons += 1;
                                    assert_eq!(old.arguments(), new.arguments());
                                    assert_eq!(old.arguments(), arguments);
                                    assert_eq!(old.shared_buffers(), new.shared_buffers());
                                    assert_eq!(
                                        old.conflict_assessment(),
                                        new.conflict_assessment()
                                    );
                                    assert_eq!(old.race_assessment(), new.race_assessment());
                                    assert_eq!(a.0, translated(&pair, &b.0));
                                    assert!(!a.0.is_empty());
                                    assert_eq!(
                                        a.0.iter()
                                            .filter(|e| matches!(e.kind, Kind::MemoryWrite { .. }))
                                            .count(),
                                        trips * 64
                                    );
                                    assert_eq!(
                                        a.0.iter()
                                            .filter(|e| matches!(e.kind, Kind::MemoryRead { .. }))
                                            .count(),
                                        0
                                    );
                                    assert_eq!(
                                        (old.invocations_executed(), new.invocations_executed()),
                                        (64, 64)
                                    );
                                    assert!(
                                        !old.grants_execution_authority()
                                            && !new.grants_execution_authority()
                                    );
                                    if let Some((old_trace, new_trace)) = &previous {
                                        assert_eq!(&a.0, old_trace);
                                        assert_eq!(&b.0, new_trace);
                                    }
                                    previous = Some((a.0, b.0));
                                }
                                assert_eq!(request, unchanged);
                            }
                        }
                        receipt
                    };
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
    // 2 targets * (1 Direct + 2 UnitLocal roots) * 5 shapes * 4 controls * 2 repeats.
    assert_eq!(comparisons, 240);
}

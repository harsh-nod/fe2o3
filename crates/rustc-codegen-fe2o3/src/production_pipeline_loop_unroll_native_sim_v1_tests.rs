use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirLoopUnrollPairV1 as Pair;
use fe2o3_kernel_ir::{
    AddressSpace, BlockId, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Site,
    VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationEventKindV1 as Kind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1 as Event, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};
#[derive(Default)]
struct Events(Vec<Event>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &Event) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 524_288 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete native unroll trace bound".into(),
            });
        }
        self.0.push(event.clone());
        Ok(())
    }
}
fn simulation(owner: &Graph) -> AdmittedSimulationModuleV1 {
    let wire = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(wire.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(wire, SimulationLimitsV1::default()).unwrap()
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
fn source_launch(value: &PreparedLoopUnrollNativeOutputV1, ordinal: usize) -> ([u64; 3], [u32; 3]) {
    let (roster, semantic) = match &value.owner {
        Unrolled::Direct(v) => {
            let source = v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir();
            (
                source.source_launch_roster().unwrap(),
                source.semantic().semantic(),
            )
        }
        Unrolled::Erased(v) => {
            let source = v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source();
            (
                source.source_launch(),
                source.semantic_ssa().source_semantic(),
            )
        }
    };
    assert_eq!(
        roster.semantic_sha256(),
        semantic.semantic_sha256().as_bytes()
    );
    assert!(!roster.grants_artifact_or_launch_authority());
    assert_eq!(roster.roots().len(), 2);
    let row = roster.roots()[ordinal];
    let source = &semantic.functions()[row.selected_root().index() as usize];
    assert_eq!(source.identity(), row.semantic_root_identity());
    let entry = source.kernel_entry().unwrap();
    assert_eq!(
        *entry.kernel_binding_identity().as_bytes(),
        row.kernel_binding()
    );
    let kernel = &value.output().module().kernels[ordinal];
    assert_eq!(
        kernel.id.as_str().as_bytes(),
        entry.export_symbol().as_bytes()
    );
    assert_eq!(kernel.domain.rank(), row.source_rank());
    assert_eq!(&value.forwarding_output().module().kernels[ordinal], kernel);
    let mut original = value.original().unwrap().module().kernels[ordinal].clone();
    original.required_capabilities.insert(match value.profile {
        Profile::Gfx942 => fe2o3_kernel_ir::gfx942_xnack_minus_target_capability(),
        Profile::Gfx950 => fe2o3_kernel_ir::gfx950_xnack_minus_target_capability(),
    });
    original
        .required_capabilities
        .insert(fe2o3_kernel_ir::TargetCapability::WaveWidth(
            fe2o3_kernel_ir::WaveWidth::Wave64,
        ));
    assert_eq!(&original, kernel);
    let layout = row.layout();
    assert!(layout.full_physical_workgroups());
    let grid = layout.global_extents();
    let group = kernel.workgroup_size.unwrap();
    let group = [group.x, group.y, group.z];
    assert_eq!(group.map(u64::from), layout.workgroup_extents());
    for axis in 0..3 {
        assert_ne!(grid[axis], 0);
        assert_ne!(group[axis], 0);
        assert_eq!(grid[axis] % u64::from(group[axis]), 0);
    }
    for (axis, extent) in kernel.domain.extents().enumerate() {
        if let fe2o3_kernel_ir::LaunchExtent::Static(n) = extent {
            assert_eq!(grid[axis], u64::from(n));
        }
    }
    (grid, group)
}
#[test]
fn loop_unroll_native_sim_retains_all_events_memory_and_exact_source_launch() {
    let mut comparisons = 0;
    for erased in [false, true] {
        for profile in PROFILES {
            for bound in [Some(0), Some(1), Some(3), Some(8), None] {
                with_prepared(erased, profile, bound, |owner, budget| {
                    let floor = budget.storage();
                    let receipt = {
                        let (pair, receipt) = tail(&owner.owner)
                            .replay(owner.forwarding_output(), owner.limits(), budget)
                            .unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        let before = simulation(owner.forwarding_output());
                        let after = simulation(owner.output());
                        for (ordinal, kernel) in owner.output().module().kernels.iter().enumerate()
                        {
                            let (grid, group) = source_launch(owner, ordinal);
                            let invocations = grid.into_iter().product::<u64>() as usize;
                            for control in [0u32, 1, 2, 3] {
                                let runtime_bound = u64::from(control);
                                let trips = bound.unwrap_or(runtime_bound) as usize;
                                let mut arguments = Vec::new();
                                if erased {
                                    arguments.push(SimulationArgumentV1::Buffer(
                                        BufferArgumentV1::from_scalars(
                                            fe2o3_kernel_ir::AccessMode::ReadWrite,
                                            4,
                                            &[ScalarBitsV1::u32(123)],
                                            SimulationTargetV1::amdgpu_64(),
                                        )
                                        .unwrap(),
                                    ));
                                }
                                arguments
                                    .push(SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control)));
                                arguments.push(SimulationArgumentV1::Scalar(
                                    ScalarBitsV1::new(
                                        fe2o3_kernel_ir::ScalarType::U64,
                                        runtime_bound.into(),
                                        SimulationTargetV1::amdgpu_64(),
                                    )
                                    .unwrap(),
                                ));
                                let request = SimulationRequestV1::new(
                                    kernel.id.as_str(),
                                    grid,
                                    group,
                                    arguments.clone(),
                                );
                                let unchanged = request.clone();
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
                                    assert_eq!(old.shared_buffers(), new.shared_buffers());
                                    assert_eq!(
                                        old.conflict_assessment(),
                                        new.conflict_assessment()
                                    );
                                    assert_eq!(old.race_assessment(), new.race_assessment());
                                    assert_eq!(a.0, translated(&pair, &b.0));
                                    assert!(!a.0.is_empty());
                                    assert_eq!(
                                        (old.invocations_executed(), new.invocations_executed()),
                                        (invocations as u64, invocations as u64)
                                    );
                                    assert!(
                                        !old.grants_execution_authority()
                                            && !new.grants_execution_authority()
                                    );
                                    if erased {
                                        let expected = if trips == 0 { 7u32 } else { control & 1 };
                                        assert_eq!(
                                            old.buffer(0).unwrap().bytes(),
                                            expected.to_le_bytes()
                                        );
                                        assert!(
                                            old.buffer(0).unwrap().initialized().iter().all(|b| *b)
                                        );
                                        let globals: Vec<_> =
                                            a.0.iter()
                                                .filter_map(|e| match e.kind {
                                                    Kind::AllocationPreexisting {
                                                        allocation,
                                                        address_space: AddressSpace::Global,
                                                        ..
                                                    } => Some(allocation),
                                                    _ => None,
                                                })
                                                .collect();
                                        assert_eq!(globals.len(), 1);
                                        assert_eq!(a.0.iter().filter(|e| matches!(e.kind, Kind::MemoryWrite { allocation, offset: 0, bytes: 4 } if allocation == globals[0])).count(), 1 + trips);
                                    } else {
                                        assert_eq!(
                                            a.0.iter()
                                                .filter(|e| matches!(
                                                    e.kind,
                                                    Kind::MemoryWrite { .. }
                                                ))
                                                .count(),
                                            trips * invocations
                                        );
                                    }
                                    assert_eq!(
                                        a.0.iter()
                                            .filter(|e| matches!(e.kind, Kind::MemoryRead { .. }))
                                            .count(),
                                        0
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
    // Two profiles, two source variants, two roots, five shapes, four controls, two repeats.
    assert_eq!(comparisons, 320);
}

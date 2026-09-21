use super::*;
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationConflictAssessmentV1, SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationExecutionV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};
#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 262_144 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete source forwarding observation bound".into(),
            });
        }
        self.0.push(event.clone());
        Ok(())
    }
}
fn simulate(
    owner: &Graph,
    request: &SimulationRequestV1,
) -> (SimulationExecutionV1, Vec<SimulationEventV1>) {
    let wire = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(wire.identity(), owner.canonical().identity());
    let module =
        AdmittedSimulationModuleV1::admit_v12(wire, SimulationLimitsV1::default()).unwrap();
    let mut events = Events::default();
    let value = module
        .simulate_observed_with_sink(
            request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    (value, events.0)
}
fn source_launch(
    value: &PreparedRefinedForwardingNativeOutputV1,
    ordinal: usize,
) -> ([u64; 3], [u32; 3]) {
    let (roster, semantic) = match &value.owner {
        Composed::Direct(owner) => {
            let source = owner
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
        Composed::Erased(owner) => {
            let source = owner
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
    assert_eq!(roster.roots().len(), value.output().module().kernels.len());
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
    assert_eq!(&value.licm_input().module().kernels[ordinal], kernel);
    assert_eq!(&value.refinement_output().module().kernels[ordinal], kernel);
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
fn remaining_events(
    value: &PreparedRefinedForwardingNativeOutputV1,
    events: &[SimulationEventV1],
) -> (Vec<SimulationEventV1>, usize) {
    let mut retained = Vec::new();
    let mut removed = 0;
    for (index, event) in events.iter().enumerate() {
        if let EventKind::MemoryRead {
            allocation,
            offset,
            bytes,
        } = event.kind
        {
            let body = value.refinement_output().module().functions[event.site.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let block = body
                .blocks
                .iter()
                .position(|b| b.id == event.site.block)
                .unwrap();
            let selected = value
                .origins()
                .iter()
                .map(|row| row.canonical_origin())
                .find(|row| {
                    row.store.is_some()
                        && row.input.block.function.0 as usize == event.site.function_ordinal
                        && row.input.block.block as usize == block
                        && Some(row.input.operation) == event.site.operation
                });
            if let Some(row) = selected {
                let store = row.store.unwrap();
                assert_eq!((offset, bytes), (0, 4));
                assert_eq!(row.input, row.output);
                assert!(
                    events[..index]
                        .iter()
                        .any(|prior| prior.invocation == event.invocation
                            && prior.site.function_ordinal == event.site.function_ordinal
                            && prior.site.block == body.blocks[store.block.block as usize].id
                            && prior.site.operation == Some(store.operation)
                            && prior.kind
                                == (EventKind::MemoryWrite {
                                    allocation,
                                    offset,
                                    bytes
                                }))
                );
                assert!(events[..index].iter().any(|prior|prior.invocation==event.invocation&&matches!(prior.kind,
                    EventKind::AllocationCreated{allocation:a,address_space:AddressSpace::Private,bytes:4} if a==allocation)));
                removed += 1;
                continue;
            }
        }
        retained.push(event.clone());
    }
    (retained, removed)
}
fn normalize_refinement(
    value: &PreparedRefinedForwardingNativeOutputV1,
    events: &[SimulationEventV1],
) -> (Vec<SimulationEventV1>, usize) {
    use fe2o3_kir_sim::SimulationExecutionOutcomeV1::Completed;
    let mut retained = Vec::new();
    let mut false_pairs = 0;
    let mut index = 0;
    while index < events.len() {
        let event = &events[index];
        let function = event.site.function_ordinal;
        let before = value.licm_input().module().functions[function]
            .body
            .as_ref()
            .unwrap();
        let middle = value.refinement_output().module().functions[function]
            .body
            .as_ref()
            .unwrap();
        let block = middle
            .blocks
            .iter()
            .position(|b| b.id == event.site.block)
            .unwrap();
        assert_eq!(before.blocks[block].id, middle.blocks[block].id);
        let mut mapped = event.clone();
        if let Some(operation) = event.site.operation {
            let coordinate = Coordinate {
                block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                    function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                        function.try_into().unwrap(),
                    ),
                    block: block.try_into().unwrap(),
                },
                operation,
            };
            let mut matches = value.refinement_origins().iter().filter(|source| {
                match source.canonical_origin() {
                    Origin::Unchanged { output, .. } => output == coordinate,
                    Origin::CheckedAddSplit {
                        sum_output,
                        false_output,
                        ..
                    } => sum_output == coordinate || false_output == coordinate,
                }
            });
            let source = matches
                .next()
                .expect("complete checked actual L-to-R origins");
            assert!(matches.next().is_none());
            let input = match source.canonical_origin() {
                Origin::Unchanged { input, output } => {
                    assert_eq!(
                        super::operation(value.licm_input(), input),
                        super::operation(value.refinement_output(), output)
                    );
                    input
                }
                Origin::CheckedAddSplit {
                    input,
                    sum_output,
                    false_output,
                    ..
                } => {
                    if coordinate == false_output {
                        assert_eq!(source.synthetic_false_source_statement(), None);
                        assert_eq!(
                            super::operation(value.refinement_output(), false_output).kind,
                            OperationKind::Constant(Constant::Bool(false))
                        );
                        assert_eq!(event.kind, EventKind::OperationBegin);
                        let end = events
                            .get(index + 1)
                            .expect("synthetic false must complete");
                        assert_eq!(
                            (&end.invocation, &end.site),
                            (&event.invocation, &event.site)
                        );
                        assert_eq!(end.kind, EventKind::OperationEnd { outcome: Completed });
                        index += 2;
                        false_pairs += 1;
                        continue;
                    }
                    assert_eq!(coordinate, sum_output);
                    assert!(matches!(
                        event.kind,
                        EventKind::OperationBegin | EventKind::OperationEnd { outcome: Completed }
                    ));
                    input
                }
            };
            assert_eq!(input.block, coordinate.block);
            mapped.site.operation = Some(input.operation);
        } else {
            assert_eq!(
                before.blocks[block].terminator,
                middle.blocks[block].terminator
            );
        }
        retained.push(mapped);
        index += 1;
    }
    (retained, false_pairs)
}
#[test]
fn refined_forwarding_native_sim_four_actual_graphs_preserve_cpu_and_complete_selected_events() {
    let mut comparisons = 0;
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_prefix(erased, profile, mutation, |prefix, budget| {
                    let (value, receipt) = prepare(
                        prefix,
                        profile,
                        Limits::default(),
                        ForwardingLimits::default(),
                        budget,
                    )
                    .unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    shape(
                        &value,
                        erased,
                        if mutation { 2 } else { 0 },
                        if mutation { 2 } else { 0 },
                        budget,
                    );
                    for (ordinal, kernel) in value.output().module().kernels.iter().enumerate() {
                        let (grid, group) = source_launch(&value, ordinal);
                        let invocations = grid.into_iter().product::<u64>();
                        let controls: &[u32] = if erased && !mutation {
                            &[0]
                        } else {
                            &[0, 1, 2, u32::MAX]
                        };
                        let bounds: &[u64] = if mutation { &[0, 1, 3] } else { &[0] };
                        for &control in controls {
                            for &bound in bounds {
                                let mut args = Vec::new();
                                if erased {
                                    args.push(SimulationArgumentV1::Buffer(
                                        BufferArgumentV1::from_scalars(
                                            fe2o3_kernel_ir::AccessMode::ReadWrite,
                                            4,
                                            &[ScalarBitsV1::u32(123)],
                                            SimulationTargetV1::amdgpu_64(),
                                        )
                                        .unwrap(),
                                    ));
                                }
                                if !erased || mutation {
                                    args.push(SimulationArgumentV1::Scalar(ScalarBitsV1::u32(
                                        control,
                                    )));
                                }
                                if mutation {
                                    args.push(SimulationArgumentV1::Scalar(
                                        ScalarBitsV1::new(
                                            fe2o3_kernel_ir::ScalarType::U64,
                                            bound.into(),
                                            SimulationTargetV1::amdgpu_64(),
                                        )
                                        .unwrap(),
                                    ));
                                }
                                let request =
                                    SimulationRequestV1::new(kernel.id.as_str(), grid, group, args);
                                let unchanged = request.clone();
                                for _ in 0..2 {
                                    let original = simulate(value.original().unwrap(), &request);
                                    let before = simulate(value.licm_input(), &request);
                                    let refined = simulate(value.refinement_output(), &request);
                                    let after = simulate(value.output(), &request);
                                    assert_eq!(original.0.arguments(), before.0.arguments());
                                    assert_eq!(before.0.arguments(), refined.0.arguments());
                                    assert_eq!(refined.0.arguments(), after.0.arguments());
                                    assert_eq!(
                                        before.0.shared_buffers(),
                                        refined.0.shared_buffers()
                                    );
                                    assert_eq!(
                                        refined.0.shared_buffers(),
                                        after.0.shared_buffers()
                                    );
                                    let (normalized, false_pairs) =
                                        normalize_refinement(&value, &refined.1);
                                    assert_eq!(normalized, before.1);
                                    assert_eq!(
                                        false_pairs,
                                        if mutation {
                                            bound as usize * invocations as usize
                                        } else {
                                            0
                                        }
                                    );
                                    assert_eq!(
                                        original.0.shared_buffers(),
                                        after.0.shared_buffers()
                                    );
                                    let (expected, removed) = remaining_events(&value, &refined.1);
                                    assert_eq!(
                                        removed,
                                        if mutation && control & 1 == 0 {
                                            bound as usize * invocations as usize
                                        } else {
                                            0
                                        }
                                    );
                                    assert_eq!(expected, after.1);
                                    for (execution, events) in
                                        [&original, &before, &refined, &after]
                                    {
                                        assert_eq!(execution.invocations_executed(), invocations);
                                        assert!(!execution.grants_execution_authority());
                                        assert!(matches!(
                                            execution.conflict_assessment(),
                                            SimulationConflictAssessmentV1::NoConflictsObserved
                                        ));
                                        assert!(matches!(
                                            execution.race_assessment(),
                                            SimulationRaceAssessmentV1::NoRacesObserved { .. }
                                        ));
                                        if erased {
                                            let expected = if !mutation || bound == 0 {
                                                7
                                            } else {
                                                control & 1
                                            };
                                            let buffer = execution.buffer(0).unwrap();
                                            assert_eq!(buffer.bytes(), expected.to_le_bytes());
                                            assert!(buffer.initialized().iter().all(|b| *b));
                                            let globals: Vec<_> = events
                                                .iter()
                                                .filter_map(|event| match event.kind {
                                                    EventKind::AllocationPreexisting {
                                                        allocation,
                                                        address_space: AddressSpace::Global,
                                                        ..
                                                    } => Some(allocation),
                                                    _ => None,
                                                })
                                                .collect();
                                            assert_eq!(globals.len(), 1);
                                            assert_eq!(events.iter().filter(|event|matches!(event.kind,EventKind::MemoryWrite{allocation,offset:0,bytes:4} if allocation==globals[0])).count(),1+if mutation{bound as usize}else{0});
                                        }
                                    }
                                    if !erased && mutation {
                                        for events in [&before.1, &refined.1, &after.1] {
                                            assert_eq!(
                                                events
                                                    .iter()
                                                    .filter(|e| matches!(
                                                        e.kind,
                                                        EventKind::MemoryWrite { .. }
                                                    ))
                                                    .count(),
                                                bound as usize * invocations as usize
                                            );
                                        }
                                        assert_eq!(
                                            before
                                                .1
                                                .iter()
                                                .filter(|e| matches!(
                                                    e.kind,
                                                    EventKind::MemoryRead { .. }
                                                ))
                                                .count(),
                                            removed
                                        );
                                        assert_eq!(
                                            refined
                                                .1
                                                .iter()
                                                .filter(|e| matches!(
                                                    e.kind,
                                                    EventKind::MemoryRead { .. }
                                                ))
                                                .count(),
                                            removed
                                        );
                                        assert_eq!(
                                            after
                                                .1
                                                .iter()
                                                .filter(|e| matches!(
                                                    e.kind,
                                                    EventKind::MemoryRead { .. }
                                                ))
                                                .count(),
                                            0
                                        );
                                    }
                                    comparisons += 1;
                                }
                                assert_eq!(request, unchanged);
                            }
                        }
                    }
                    release(value, receipt, budget);
                });
            }
        }
    }
    assert_eq!(
        comparisons, 232,
        "928 real simulations of N, L, actual middle R and actual final F"
    );
}

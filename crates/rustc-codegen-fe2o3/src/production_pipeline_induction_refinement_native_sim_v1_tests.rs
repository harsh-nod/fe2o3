use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1 as Pair;
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationConflictAssessmentV1, SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationExecutionV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
};

#[derive(Default)]
struct CompleteEffects(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for CompleteEffects {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        // Exclude ALL OperationBegin/End events, not just changed pure sites.
        // Every memory/control/call/allocation/invocation payload remains exact.
        if !matches!(
            event.kind,
            EventKind::OperationBegin | EventKind::OperationEnd { .. }
        ) {
            if self.0.len() == 32_768 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete native refinement effects".into(),
                });
            }
            self.0.push(event.clone());
        }
        Ok(())
    }
}
fn run(
    owner: &Graph,
    request: &SimulationRequestV1,
) -> (SimulationExecutionV1, Vec<SimulationEventV1>) {
    let bytes = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(bytes.identity(), owner.canonical().identity());
    let module =
        AdmittedSimulationModuleV1::admit_v12(bytes, SimulationLimitsV1::default()).unwrap();
    let mut events = CompleteEffects::default();
    let execution = module
        .simulate_observed_with_sink(
            request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    (execution, events.0)
}
fn translate(pair: &Pair<'_>, events: &[SimulationEventV1]) -> Vec<SimulationEventV1> {
    events
        .iter()
        .map(|event| {
            let mut mapped = event.clone();
            let f = event.site.function_ordinal;
            let before = pair.input().module().functions[f].body.as_ref().unwrap();
            let after = pair.output().module().functions[f].body.as_ref().unwrap();
            let block = before
                .blocks
                .iter()
                .position(|b| b.id == event.site.block)
                .unwrap();
            assert_eq!(before.blocks[block].id, after.blocks[block].id);
            if let Some(ordinal) = event.site.operation {
                let input = Coordinate {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            f.try_into().unwrap(),
                        ),
                        block: block.try_into().unwrap(),
                    },
                    operation: ordinal,
                };
                let mut rows = pair.origins().iter().filter(|r| r.input() == input);
                let row = rows.next().unwrap();
                assert!(rows.next().is_none());
                let Origin::Unchanged { input, output } = *row else {
                    panic!("pure refined operations cannot manufacture semantic effects")
                };
                assert_eq!(input.block, output.block);
                assert_eq!(
                    operation(pair.input(), input),
                    operation(pair.output(), output)
                );
                mapped.site.operation = Some(output.operation);
            } else {
                assert_eq!(
                    before.blocks[block].terminator,
                    after.blocks[block].terminator
                );
            }
            mapped
        })
        .collect()
}
fn source_launch(
    value: &PreparedInductionRefinementNativeOutputV1,
    ordinal: usize,
) -> ([u64; 3], [u32; 3]) {
    let (roster, semantic) = match &value.owner {
        Refined::Direct(v) => {
            let source = v
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
        Refined::Erased(v) => {
            let source = v
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
    assert_eq!(
        value.licm_input().module().kernels.len(),
        roster.roots().len()
    );
    assert_eq!(&value.licm_input().module().kernels[ordinal], kernel);
    let original = value.original().unwrap();
    assert_eq!(original.module().kernels.len(), roster.roots().len());
    let mut bound_metadata = original.module().kernels[ordinal].clone();
    bound_metadata
        .required_capabilities
        .insert(match value.profile {
            Profile::Gfx942 => fe2o3_kernel_ir::gfx942_xnack_minus_target_capability(),
            Profile::Gfx950 => fe2o3_kernel_ir::gfx950_xnack_minus_target_capability(),
        });
    bound_metadata
        .required_capabilities
        .insert(fe2o3_kernel_ir::TargetCapability::WaveWidth(
            fe2o3_kernel_ir::WaveWidth::Wave64,
        ));
    assert_eq!(&bound_metadata, kernel);
    let layout = row.layout();
    assert!(layout.full_physical_workgroups());
    let grid = layout.global_extents();
    let workgroup = kernel.workgroup_size.unwrap();
    let workgroup = [workgroup.x, workgroup.y, workgroup.z];
    assert_eq!(workgroup.map(u64::from), layout.workgroup_extents());
    for axis in 0..3 {
        assert_ne!(grid[axis], 0);
        assert_ne!(workgroup[axis], 0);
        assert_eq!(grid[axis] % u64::from(workgroup[axis]), 0);
    }
    for (axis, extent) in kernel.domain.extents().enumerate() {
        if let fe2o3_kernel_ir::LaunchExtent::Static(extent) = extent {
            assert_eq!(grid[axis], u64::from(extent));
        }
    }
    (grid, workgroup)
}
fn compare(
    value: &PreparedInductionRefinementNativeOutputV1,
    erased: bool,
    mutation: bool,
    budget: &mut Budget<'_>,
) -> usize {
    let floor = budget.storage();
    let (comparisons, retained) = {
        let (pair, receipt) = tail(&value.owner)
            .replay_against(value.licm_input(), value.limits(), budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert!(std::ptr::eq(pair.output(), value.output()));
        let mut comparisons = 0;
        for (ordinal, kernel) in value.output().module().kernels.iter().enumerate() {
            let (grid, workgroup) = source_launch(value, ordinal);
            if erased {
                assert_eq!((grid, workgroup), ([1, 1, 1], [1, 1, 1]));
            }
            let invocations = grid.into_iter().product::<u64>();
            let controls: &[u32] = if erased && !mutation {
                &[0]
            } else {
                &[0, 1, 2, u32::MAX]
            };
            let bounds: &[u64] = if mutation { &[0, 1, 3] } else { &[0] };
            for (&control, &bound) in controls
                .iter()
                .flat_map(|a| bounds.iter().map(move |b| (a, b)))
            {
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
                if !erased || mutation {
                    arguments.push(SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control)));
                }
                if mutation {
                    arguments.push(SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(
                            ScalarType::U64,
                            bound.into(),
                            SimulationTargetV1::amdgpu_64(),
                        )
                        .unwrap(),
                    ));
                }
                let request =
                    SimulationRequestV1::new(kernel.id.as_str(), grid, workgroup, arguments);
                let untouched = request.clone();
                let trips = if mutation { bound } else { 0 };
                let reads = if mutation && control & 1 == 0 {
                    bound
                } else {
                    0
                };
                let expected_output = if trips == 0 { 7 } else { control & 1 };
                let mut previous = None;
                for _ in 0..2 {
                    let before = run(value.licm_input(), &request);
                    let after = run(value.output(), &request);
                    assert_eq!(before.0.arguments(), after.0.arguments());
                    assert_eq!(before.0.shared_buffers(), after.0.shared_buffers());
                    assert_eq!(translate(&pair, &before.1), after.1);
                    for (result, events) in [&before, &after] {
                        assert_eq!(result.invocations_executed(), invocations);
                        assert!(!result.grants_execution_authority());
                        assert!(matches!(
                            result.conflict_assessment(),
                            SimulationConflictAssessmentV1::NoConflictsObserved
                        ));
                        assert!(matches!(
                            result.race_assessment(),
                            SimulationRaceAssessmentV1::NoRacesObserved { .. }
                        ));
                        assert_eq!(
                            events
                                .iter()
                                .filter(|e| matches!(e.kind, EventKind::InvocationBegin))
                                .count(),
                            invocations as usize
                        );
                        assert_eq!(
                            events
                                .iter()
                                .filter(|e| matches!(e.kind, EventKind::InvocationEnd { .. }))
                                .count(),
                            invocations as usize
                        );
                        if erased {
                            let buffer = result.buffer(0).unwrap();
                            assert_eq!(buffer.bytes(), expected_output.to_le_bytes());
                            assert!(buffer.initialized().iter().all(|b| *b));
                            let globals: Vec<_> = events
                                .iter()
                                .filter_map(|e| match e.kind {
                                    EventKind::AllocationPreexisting {
                                        allocation,
                                        address_space: fe2o3_kernel_ir::AddressSpace::Global,
                                        ..
                                    } => Some(allocation),
                                    _ => None,
                                })
                                .collect();
                            assert_eq!(globals.len(), 1);
                            assert_eq!(events.iter().filter(|e| matches!(e.kind, EventKind::MemoryWrite { allocation, offset: 0, bytes: 4 } if allocation == globals[0])).count(), (1 + trips) as usize);
                        } else if mutation {
                            assert_eq!(
                                events
                                    .iter()
                                    .filter(|e| matches!(e.kind, EventKind::MemoryWrite { .. }))
                                    .count(),
                                trips as usize * invocations as usize
                            );
                            assert_eq!(
                                events
                                    .iter()
                                    .filter(|e| matches!(e.kind, EventKind::MemoryRead { .. }))
                                    .count(),
                                reads as usize * invocations as usize
                            );
                        }
                    }
                    let observation = (before, after);
                    if let Some(previous) = previous {
                        assert_eq!(previous, observation);
                    }
                    previous = Some(observation);
                    comparisons += 1;
                }
                assert_eq!(request, untouched);
            }
        }
        (comparisons, receipt.retained_storage())
    };
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
    comparisons
}
#[test]
fn induction_refinement_native_sim_checks_actual_final_owner_cpu_outputs_and_complete_effects() {
    let mut comparisons = 0;
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_prefix(erased, profile, mutation, |prefix, budget| {
                    let floor = budget.storage();
                    let (value, receipt) =
                        prepare(prefix, profile, Limits::default(), budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_shape(&value, mutation, budget);
                    comparisons += compare(&value, erased, mutation, budget);
                    drop(value);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), floor);
                });
            }
        }
    }
    assert_eq!(comparisons, 232); // 464 new actual old/final graph executions.
}

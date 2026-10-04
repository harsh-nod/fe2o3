use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirInductionRefinementV1 as Pair;

#[derive(Default)]
struct CompleteEffects(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for CompleteEffects {
    fn record(
        &mut self,
        event: &SimulationEventV1,
    ) -> std::result::Result<(), SimulationEventSinkErrorV1> {
        // ALL pure operation lifecycle events are excluded, not only changed
        // sites. Every control, memory, call, allocation and invocation remains.
        if !matches!(
            event.kind,
            EventKind::OperationBegin | EventKind::OperationEnd { .. }
        ) {
            if self.0.len() == 32_768 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete source refinement effects".into(),
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
                let RefineOrigin::Unchanged { input, output } = *row else {
                    panic!("pure checked refinement cannot manufacture a semantic event")
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
fn compare(
    owner: &Refined,
    launches: &[Launch],
    erased: bool,
    mutation: bool,
    budget: &mut Budget<'_>,
) -> usize {
    let floor = budget.storage();
    let (comparisons, retained) = {
        let (pair, receipt) = owner
            .tail()
            .replay_against(owner.input(), owner.limits(), budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let mut comparisons = 0;
        assert_eq!(launches.len(), owner.output().module().kernels.len());
        for (ordinal, kernel) in owner.output().module().kernels.iter().enumerate() {
            let (grid, workgroup) = launches[ordinal];
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
                    let before = run(owner.input(), &request);
                    let after = run(owner.output(), &request);
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
                            assert_eq!(events.iter().filter(|e|matches!(e.kind,EventKind::MemoryWrite{allocation,offset:0,bytes:4} if allocation==globals[0])).count(),(1+trips)as usize);
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
fn source_induction_refinement_sim_cpu_outputs_and_complete_effects_both_owners_profiles() {
    let mut comparisons = 0;
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_refined(erased, profile, mutation, |owner, launches, budget| {
                    assert_refinement(owner, mutation, budget);
                    comparisons += compare(owner, launches, erased, mutation, budget);
                });
            }
        }
    }
    assert_eq!(comparisons, 232); // 464 actual old/final graph executions.
}

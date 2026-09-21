use super::*;
pub(super) fn guarded(length: usize, cpu: bool) -> (SimulationArgumentV1, SharedBufferV1) {
    let prefix = [0x0102_0304u32, 0x1122_3344, 0x5566_7788, 0xaabb_ccdd];
    let suffix = [0x8765_4321u32, 0x1234_5678, 0xdead_beef, 0xff00_00ff];
    let values = (0..length)
        .map(|index| {
            if cpu && index < 64 {
                7
            } else {
                0x7000_0000u32 + index as u32
            }
        })
        .collect::<Vec<_>>();
    let bytes = prefix
        .iter()
        .chain(&values)
        .chain(&suffix)
        .flat_map(|v| v.to_le_bytes())
        .collect::<Vec<_>>();
    let buffer = BufferArgumentV1::new(
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        bytes.clone(),
        vec![true; bytes.len()],
        TARGET,
    )
    .unwrap();
    let view = BufferViewArgumentV1::new(
        BufferBackingIdV1(0),
        ScalarType::U32,
        AccessMode::ReadWrite,
        4,
        16,
        length,
        TARGET,
    )
    .unwrap();
    (
        SimulationArgumentV1::BufferView(view),
        SharedBufferV1 {
            id: BufferBackingIdV1(0),
            buffer,
        },
    )
}
#[derive(Default)]
struct Events(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        if self.0.len() == 262_144 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete ordinary combined event bound".into(),
            });
        }
        self.0.push(event.clone());
        Ok(())
    }
}
fn simulate(
    graph: &Graph,
    request: &SimulationRequestV1,
) -> (SimulationExecutionV1, Vec<SimulationEventV1>) {
    let wire = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        graph.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(wire.identity(), graph.canonical().identity());
    let admitted =
        AdmittedSimulationModuleV1::admit_v12(wire, SimulationLimitsV1::default()).unwrap();
    let mut events = Events::default();
    let result = admitted
        .simulate_observed_with_sink(request, TARGET, SimulationLimitsV1::default(), &mut events)
        .unwrap();
    (result, events.0)
}
fn normalize_refinement(
    value: &Graphs<'_>,
    events: &[SimulationEventV1],
) -> (Vec<SimulationEventV1>, usize) {
    let mut retained = Vec::new();
    let mut false_pairs = 0;
    let mut index = 0;
    while index < events.len() {
        let event = &events[index];
        let function = event.site.function_ordinal;
        let before = value.licm.module().functions[function]
            .body
            .as_ref()
            .unwrap();
        let middle = value.refined.module().functions[function]
            .body
            .as_ref()
            .unwrap();
        let block = middle
            .blocks
            .iter()
            .position(|block| block.id == event.site.block)
            .unwrap();
        assert_eq!(before.blocks[block].id, middle.blocks[block].id);
        let mut mapped = event.clone();
        if let Some(operation_index) = event.site.operation {
            let site = Site {
                block: Block {
                    function: Function(function.try_into().unwrap()),
                    block: block.try_into().unwrap(),
                },
                operation: operation_index,
            };
            let mut rows = value
                .refinement
                .iter()
                .filter(|row| match row.canonical_origin() {
                    Origin::Unchanged { output, .. } => output == site,
                    Origin::CheckedAddSplit {
                        sum_output,
                        false_output,
                        ..
                    } => sum_output == site || false_output == site,
                });
            let row = rows.next().expect("complete actual L-to-R relation");
            assert!(rows.next().is_none());
            let input = match row.canonical_origin() {
                Origin::Unchanged { input, output } => {
                    assert_eq!(
                        operation(value.licm, input),
                        operation(value.refined, output)
                    );
                    input
                }
                Origin::CheckedAddSplit {
                    input,
                    sum_output,
                    false_output,
                    ..
                } => {
                    if site == false_output {
                        assert_eq!(row.synthetic_false_source_statement(), None);
                        assert_eq!(
                            operation(value.refined, false_output).kind,
                            OperationKind::Constant(Constant::Bool(false))
                        );
                        assert_eq!(event.kind, EventKind::OperationBegin);
                        let end = events.get(index + 1).expect("synthetic false completion");
                        assert_eq!(
                            (&end.invocation, &end.site),
                            (&event.invocation, &event.site)
                        );
                        assert_eq!(
                            end.kind,
                            EventKind::OperationEnd {
                                outcome: SimulationExecutionOutcomeV1::Completed
                            }
                        );
                        false_pairs += 1;
                        index += 2;
                        continue;
                    }
                    assert_eq!(site, sum_output);
                    assert!(matches!(
                        event.kind,
                        EventKind::OperationBegin
                            | EventKind::OperationEnd {
                                outcome: SimulationExecutionOutcomeV1::Completed
                            }
                    ));
                    input
                }
            };
            assert_eq!(input.block, site.block);
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
fn remaining_events(
    value: &Graphs<'_>,
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
            let body = value.refined.module().functions[event.site.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let block = body
                .blocks
                .iter()
                .position(|block| block.id == event.site.block)
                .unwrap();
            let mut rows = value
                .forwarding
                .iter()
                .map(|row| row.canonical_origin())
                .filter(|row| {
                    row.store.is_some()
                        && row.input.block.function.0 as usize == event.site.function_ordinal
                        && row.input.block.block as usize == block
                        && Some(row.input.operation) == event.site.operation
                });
            if let Some(row) = rows.next() {
                assert!(rows.next().is_none());
                let store = row.store.unwrap();
                assert_eq!(row.input, row.output);
                assert_eq!((offset, bytes), (0, 4));
                let prior = events[..index].iter().rev().find(|prior|
                    prior.invocation == event.invocation && matches!(prior.kind,
                        EventKind::MemoryWrite { allocation: destination, .. } if destination == allocation))
                    .expect("actual same-invocation defining Store");
                assert_eq!(prior.site.function_ordinal, event.site.function_ordinal);
                assert_eq!(prior.site.block, body.blocks[store.block.block as usize].id);
                assert_eq!(prior.site.operation, Some(store.operation));
                assert_eq!(
                    prior.kind,
                    EventKind::MemoryWrite {
                        allocation,
                        offset,
                        bytes
                    }
                );
                assert!(events[..index].iter().any(|prior| prior.invocation == event.invocation && matches!(prior.kind,
                    EventKind::AllocationCreated { allocation: created, address_space: AddressSpace::Private, bytes: 4 } if created == allocation)));
                removed += 1;
                continue;
            }
        }
        retained.push(event.clone());
    }
    (retained, removed)
}
fn check_execution(
    result: &SimulationExecutionV1,
    events: &[SimulationEventV1],
    expected: &SharedBufferV1,
    length: usize,
) -> usize {
    assert_eq!(result.invocations_executed(), 64);
    assert!(result.steps_executed() > 0);
    assert!(!result.grants_execution_authority());
    assert!(matches!(
        result.conflict_assessment(),
        SimulationConflictAssessmentV1::NoConflictsObserved
    ));
    assert!(matches!(
        result.race_assessment(),
        SimulationRaceAssessmentV1::NoRacesObserved { .. }
    ));
    assert_eq!(result.shared_buffers(), std::slice::from_ref(expected));
    let globals = events
        .iter()
        .filter_map(|event| match event.kind {
            EventKind::AllocationPreexisting {
                allocation,
                address_space: AddressSpace::Global,
                ..
            } => Some(allocation),
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(globals.len(), 1);
    let global = *globals.first().unwrap();
    let mut offsets = Vec::new();
    for event in events {
        match event.kind {
            EventKind::MemoryWrite {
                allocation,
                offset,
                bytes,
            } if allocation == global => {
                assert_eq!(bytes, 4);
                offsets.push(offset);
            }
            EventKind::MemoryRead { allocation, .. } if allocation == global => {
                panic!("output-only source global read")
            }
            EventKind::MemoryAtomic { .. }
            | EventKind::MemoryFence { .. }
            | EventKind::WorkgroupBarrierArrive { .. }
            | EventKind::WorkgroupBarrierRelease { .. } => {
                panic!("unexpected source synchronization")
            }
            _ => (),
        }
    }
    offsets.sort_unstable();
    assert_eq!(
        offsets,
        (0..length.min(64))
            .map(|index| 16 + 4 * index)
            .collect::<Vec<_>>()
    );
    offsets.len()
}
pub(super) fn observe(value: &Graphs<'_>, case: Case) -> Vec<Scenario> {
    let row = value.launch.roots()[0];
    let grid = row.layout().global_extents();
    let kernel = &value.final_graph.module().kernels[0];
    let group = kernel.workgroup_size.unwrap();
    let group = [group.x, group.y, group.z];
    assert_eq!(group.map(u64::from), row.layout().workgroup_extents());
    assert_eq!(grid, [64, 1, 1]);
    assert_eq!(group, [64, 1, 1]);
    let mut scenarios = Vec::new();
    for length in LENGTHS {
        for control in CONTROLS {
            for bound in BOUNDS {
                let (argument, backing) = guarded(length, false);
                let (_, expected) = guarded(length, true);
                let args = vec![
                    argument,
                    SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control)),
                    SimulationArgumentV1::Scalar(
                        ScalarBitsV1::new(ScalarType::U64, bound.into(), TARGET).unwrap(),
                    ),
                ];
                let request = SimulationRequestV1::new(kernel.id.as_str(), grid, group, args)
                    .with_shared_buffers(vec![backing]);
                let untouched = request.clone();
                let mut previous = None;
                for repeat in 0..2 {
                    let original = simulate(value.original, &request);
                    let licm = simulate(value.licm, &request);
                    let refined = simulate(value.refined, &request);
                    let final_value = simulate(value.final_graph, &request);
                    let (normalized, synthetic_false_pairs) =
                        normalize_refinement(value, &refined.1);
                    assert_eq!(normalized, licm.1);
                    let (retained, removed_reads) = remaining_events(value, &refined.1);
                    assert_eq!(retained, final_value.1);
                    let trips = usize::from(case.motion())
                        * length.min(64)
                        * usize::try_from(bound).unwrap();
                    assert_eq!(synthetic_false_pairs, trips);
                    assert_eq!(removed_reads, if control & 1 == 0 { trips } else { 0 });
                    let results = [&original, &licm, &refined, &final_value];
                    for pair in results.windows(2) {
                        assert_eq!(pair[0].0.arguments(), pair[1].0.arguments());
                        assert_eq!(pair[0].0.shared_buffers(), pair[1].0.shared_buffers());
                    }
                    scenarios.push(Scenario {
                        length,
                        control,
                        bound,
                        repeat,
                        invocations: results.map(|result| result.0.invocations_executed()),
                        steps: results.map(|result| result.0.steps_executed()),
                        global_writes: results
                            .map(|result| check_execution(&result.0, &result.1, &expected, length)),
                        synthetic_false_pairs,
                        removed_reads,
                        backing: digest(expected.buffer.bytes()),
                    });
                    let observed = (original, licm, refined, final_value);
                    if let Some(previous) = previous {
                        assert_eq!(previous, observed);
                    }
                    previous = Some(observed);
                    assert_eq!(request, untouched);
                }
            }
        }
    }
    assert_eq!(scenarios.len(), 144);
    scenarios
}

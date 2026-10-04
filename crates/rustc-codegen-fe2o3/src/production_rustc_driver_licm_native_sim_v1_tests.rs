//! Actual borrowed source/preheader/final graphs; no replacement graph fixture.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirLicmOriginV1 as Origin;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BinaryOp, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Site,
    Operation, OperationKind, ScalarType, TargetCapability, Type, WaveWidth,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, BufferBackingIdV1, BufferViewArgumentV1,
    ScalarBitsV1, SharedBufferV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationExecutionV1, SimulationLimitsV1, SimulationRaceAssessmentV1,
    SimulationRequestV1, SimulationTargetV1,
};

const LENGTHS: [usize; 6] = [0, 1, 63, 64, 65, 129];
const CONTROLS: [u32; 4] = [0, 1, 2, u32::MAX];
const BOUNDS: [u64; 3] = [0, 1, 3];
const TARGET: SimulationTargetV1 = SimulationTargetV1::amdgpu_64();

pub(super) struct Input<'a> {
    pub original: &'a Graph,
    pub historical: &'a Graph,
    pub promoted: &'a Graph,
    pub input: &'a Graph,
    pub output: &'a Graph,
    pub origins: &'a [Origin],
    pub launch: &'a fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
    pub semantic: &'a fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    pub kernels: &'a [fe2o3_kernel_ir::FormalMemoryObligations],
    pub profile: Profile,
}
fn operation(graph: &Graph, site: Site) -> &Operation {
    &graph.module().functions[site.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[site.block.block as usize]
        .operations[site.operation as usize]
}
pub(super) fn shape(value: &Input<'_>, case: Case) -> usize {
    assert_eq!(
        value.launch.semantic_sha256(),
        value.semantic.semantic_sha256().as_bytes()
    );
    assert!(!value.launch.grants_artifact_or_launch_authority());
    assert_eq!(value.launch.roots().len(), 1);
    assert_eq!(value.kernels.len(), 1);
    assert!(!value.kernels[0].accesses().is_empty());
    let before = value.input.module();
    let after = value.output.module();
    assert_eq!(before.kernels, after.kernels);
    assert_eq!(before.kernels.len(), 1);
    let kernel = &after.kernels[0];
    // Owning replay checks guarded extraction; the value report has no completeness flag.
    assert_eq!(value.kernels[0].kernel(), &kernel.id);
    assert_eq!(value.kernels[0].entry(), &kernel.entry);
    assert!(value.kernels[0].inter_invocation_conflicts().is_empty());
    assert_eq!(kernel.id.as_str(), "licm_native");
    let row = value.launch.roots()[0];
    let source = &value.semantic.functions()[row.selected_root().index() as usize];
    assert_eq!(source.identity(), row.semantic_root_identity());
    let entry = source.kernel_entry().unwrap();
    assert_eq!(
        *entry.kernel_binding_identity().as_bytes(),
        row.kernel_binding()
    );
    assert_eq!(
        entry.export_symbol().as_bytes(),
        kernel.id.as_str().as_bytes()
    );
    assert_eq!(row.source_rank(), kernel.domain.rank());
    for graph in [value.original, value.historical, value.promoted] {
        assert_eq!(graph.module().kernels.len(), 1);
    }
    let mut metadata = value.original.module().kernels[0].clone();
    metadata.required_capabilities.insert(match value.profile {
        Profile::Gfx942 => fe2o3_kernel_ir::gfx942_xnack_minus_target_capability(),
        Profile::Gfx950 => fe2o3_kernel_ir::gfx950_xnack_minus_target_capability(),
    });
    metadata
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    assert_eq!(&metadata, kernel);
    let layout = row.layout();
    assert!(layout.full_physical_workgroups());
    assert_eq!(
        layout.global_extents(),
        [64, 1, 1],
        "actual source launch prerequisite"
    );
    assert_eq!(layout.workgroup_extents(), [64, 1, 1]);
    let size = kernel.workgroup_size.unwrap();
    assert_eq!([size.x, size.y, size.z], [64, 1, 1]);
    for (axis, extent) in kernel.domain.extents().enumerate() {
        if let fe2o3_kernel_ir::LaunchExtent::Static(n) = extent {
            assert_eq!(u64::from(n), layout.global_extents()[axis]);
        }
    }
    assert_eq!(before.functions.len(), after.functions.len());
    let mut total = 0;
    for (old, new) in before.functions.iter().zip(&after.functions) {
        assert_eq!(old.signature, new.signature);
        match (&old.body, &new.body) {
            (None, None) => (),
            (Some(old), Some(new)) => {
                assert_eq!(old.parameters, new.parameters);
                assert_eq!(old.blocks.len(), new.blocks.len());
                for (old, new) in old.blocks.iter().zip(&new.blocks) {
                    assert_eq!(
                        (old.id, &old.parameters, &old.terminator),
                        (new.id, &new.parameters, &new.terminator)
                    );
                    total += old.operations.len();
                }
            }
            _ => panic!("LICM changed actual function body presence"),
        }
    }
    assert_eq!(value.origins.len(), total);
    let mut sources = std::collections::BTreeSet::new();
    let mut outputs = std::collections::BTreeSet::new();
    for row in value.origins {
        assert!(sources.insert(row.input));
        assert!(outputs.insert(row.output));
        assert_eq!(
            operation(value.input, row.input),
            operation(value.output, row.output)
        );
        if row.hoist.is_none() {
            assert_eq!(row.input.block, row.output.block);
        }
    }
    let moved = value
        .origins
        .iter()
        .filter(|row| row.hoist.is_some())
        .collect::<Vec<_>>();
    if case.motion() {
        assert_ne!(
            value.input.canonical().canonical_bytes(),
            value.output.canonical().canonical_bytes()
        );
        let masks = moved
            .iter()
            .filter(|row| {
                matches!(
                    operation(value.input, row.input).kind,
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        ..
                    }
                )
            })
            .collect::<Vec<_>>();
        let comparisons = moved
            .iter()
            .filter(|row| {
                matches!(
                    operation(value.input, row.input).kind,
                    OperationKind::Compare { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            (masks.len(), comparisons.len()),
            (1, 1),
            "required live input-dependent source motion"
        );
        let mask = operation(value.input, masks[0].input);
        let function = &before.functions[masks[0].input.block.function.0 as usize];
        assert_eq!(function.id, kernel.entry);
        assert_eq!(function.signature.parameters.len(), 3);
        assert_eq!(
            function.signature.parameters[1],
            Type::Scalar(ScalarType::U32)
        );
        assert_eq!(
            function.signature.parameters[2],
            Type::Scalar(ScalarType::U64)
        );
        let control = function.body.as_ref().unwrap().parameters[1];
        let OperationKind::Binary { lhs, rhs, .. } = mask.kind else {
            panic!("mask kind")
        };
        assert!(lhs == control || rhs == control);
        let comparison = operation(value.input, comparisons[0].input);
        let OperationKind::Compare { lhs, rhs, .. } = comparison.kind else {
            panic!("comparison kind")
        };
        assert_eq!(mask.results.len(), 1);
        assert!(lhs == mask.results[0].id || rhs == mask.results[0].id);
        for row in &moved {
            assert!(matches!(
                operation(value.input, row.input).kind,
                OperationKind::Constant(_)
                    | OperationKind::Compare { .. }
                    | OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        ..
                    }
            ));
        }
        // Observation uses its own meter, outside the measured owning entry.
        let mut work = Work::new(work_limit());
        let mut budget = Budget::new(&mut work, storage_limit());
        let (inventory, inventory_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(value.input, &mut budget)
                .unwrap();
        budget
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        let (loops, loop_storage) = fe2o3_kernel_analysis::CanonicalKirLoopsV1::derive(
            &inventory,
            Default::default(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(loop_storage.retained_storage())
            .unwrap();
        assert_eq!(
            loops.loop_count(),
            1,
            "actual source has exactly one retained dynamic loop"
        );
        let natural = loops.natural_loop(0, &mut budget).unwrap();
        assert!(natural.is_single_entry());
        assert!(natural.unconditional_preheader().is_some());
        let recurrence = loops.recurrences(0, &mut budget).unwrap();
        assert_eq!(recurrence.len(), 1);
        assert_eq!(recurrence[0].scalar(), ScalarType::U64);
        assert_eq!(recurrence[0].step_bits(), 1);
        assert_eq!(masks[0].hoist.unwrap().header, natural.header());
        assert_eq!(comparisons[0].hoist.unwrap().header, natural.header());
        loops
            .replay(&inventory, Default::default(), &mut budget)
            .unwrap();
        drop(loops);
        budget
            .release_storage(loop_storage.retained_storage())
            .unwrap();
        drop(inventory);
        budget
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 0);
    } else {
        assert!(moved.is_empty());
        assert_eq!(
            value.input.canonical().canonical_bytes(),
            value.output.canonical().canonical_bytes()
        );
    }
    moved.len()
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Scenario {
    length: usize,
    control: u32,
    bound: u64,
    repeat: usize,
    invocations: [u64; 3],
    steps: [u64; 3],
    global_writes: [usize; 3],
    backing_sha256: [u8; 32],
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    scenarios: Vec<Scenario>,
}
impl Observation {
    pub(super) fn check(&self) -> Result<(), String> {
        let mut index = 0;
        for length in LENGTHS {
            for control in CONTROLS {
                for bound in BOUNDS {
                    for repeat in 0..2 {
                        let row = self
                            .scenarios
                            .get(index)
                            .ok_or("missing exact SIM scenario")?;
                        let (_, expected) = guarded(length, true);
                        if (row.length, row.control, row.bound, row.repeat)
                            != (length, control, bound, repeat)
                            || row.invocations != [64; 3]
                            || row.steps.contains(&0)
                            || row.global_writes != [length.min(64); 3]
                            || row.backing_sha256 != digest(expected.buffer.bytes())
                        {
                            return Err("exact original/preheader/final SIM scenario oracle".into());
                        }
                        index += 1;
                    }
                }
            }
        }
        if index != 144 || self.scenarios.len() != index {
            return Err("exact 144 triple-graph SIM rows".into());
        }
        Ok(())
    }
}

fn guarded(length: usize, cpu: bool) -> (SimulationArgumentV1, SharedBufferV1) {
    let prefix = [0x0102_0304u32, 0x1122_3344, 0x5566_7788, 0xaabb_ccdd];
    let suffix = [0x8765_4321u32, 0x1234_5678, 0xdead_beef, 0xff00_00ff];
    let values = (0..length)
        .map(|i| {
            if cpu && i < 64 {
                7
            } else {
                0x7000_0000u32 + i as u32
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
        if matches!(
            event.kind,
            EventKind::OperationBegin | EventKind::OperationEnd { .. }
        ) {
            return Ok(());
        }
        if self.0.len() == 262_144 {
            return Err(SimulationEventSinkErrorV1 {
                detail: "complete source LICM event bound".into(),
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
    let canonical = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        graph.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(canonical.identity(), graph.canonical().identity());
    let admitted =
        AdmittedSimulationModuleV1::admit_v12(canonical, SimulationLimitsV1::default()).unwrap();
    let mut events = Events::default();
    let result = admitted
        .simulate_observed_with_sink(request, TARGET, SimulationLimitsV1::default(), &mut events)
        .unwrap();
    (result, events.0)
}
fn translate(value: &Input<'_>, events: &[SimulationEventV1]) -> Vec<SimulationEventV1> {
    events
        .iter()
        .map(|event| {
            let mut mapped = event.clone();
            let old = value.input.module().functions[event.site.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let new = value.output.module().functions[event.site.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let block = old
                .blocks
                .iter()
                .position(|block| block.id == event.site.block)
                .unwrap();
            assert_eq!(old.blocks[block].id, new.blocks[block].id);
            if let Some(operation_index) = event.site.operation {
                let site = Site {
                    block: Block {
                        function: Function(event.site.function_ordinal.try_into().unwrap()),
                        block: block.try_into().unwrap(),
                    },
                    operation: operation_index,
                };
                let mut rows = value.origins.iter().filter(|row| row.input == site);
                let row = rows.next().unwrap();
                assert!(rows.next().is_none());
                assert_eq!(row.hoist, None);
                assert_eq!(row.input.block, row.output.block);
                assert_eq!(
                    operation(value.input, row.input),
                    operation(value.output, row.output)
                );
                mapped.site.block = new.blocks[row.output.block.block as usize].id;
                mapped.site.operation = Some(row.output.operation);
            } else {
                assert_eq!(old.blocks[block].terminator, new.blocks[block].terminator);
            }
            mapped
        })
        .collect()
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
                panic!("output-only source unexpectedly read global data")
            }
            EventKind::MemoryAtomic { .. }
            | EventKind::MemoryFence { .. }
            | EventKind::WorkgroupBarrierArrive { .. }
            | EventKind::WorkgroupBarrierRelease { .. } => {
                panic!("unexpected source synchronization effect")
            }
            _ => (),
        }
    }
    offsets.sort_unstable();
    assert_eq!(
        offsets,
        (0..length.min(64)).map(|i| 16 + 4 * i).collect::<Vec<_>>()
    );
    offsets.len()
}
pub(super) fn observe(value: &Input<'_>, case: Case) -> Observation {
    shape(value, case);
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
                let request = SimulationRequestV1::new("licm_native", [64, 1, 1], [64, 1, 1], args)
                    .with_shared_buffers(vec![backing]);
                let untouched = request.clone();
                let mut previous = None;
                for repeat in 0..2 {
                    let original = simulate(value.original, &request);
                    let before = simulate(value.input, &request);
                    let after = simulate(value.output, &request);
                    assert_eq!(original.0.arguments(), before.0.arguments());
                    assert_eq!(before.0.arguments(), after.0.arguments());
                    assert_eq!(translate(value, &before.1), after.1);
                    let results = [&original, &before, &after];
                    let writes = results
                        .map(|(result, events)| check_execution(result, events, &expected, length));
                    scenarios.push(Scenario {
                        length,
                        control,
                        bound,
                        repeat,
                        invocations: results.map(|r| r.0.invocations_executed()),
                        steps: results.map(|r| r.0.steps_executed()),
                        global_writes: writes,
                        backing_sha256: digest(expected.buffer.bytes()),
                    });
                    let observed = (original, before, after);
                    if let Some(previous) = previous {
                        assert_eq!(previous, observed);
                    }
                    previous = Some(observed);
                    assert_eq!(request, untouched);
                }
            }
        }
    }
    let observation = Observation { scenarios };
    observation.check().unwrap();
    observation
}

#[test]
fn licm_source_sim_protocol_refuses_empty_or_incomplete_scenarios() {
    assert!(Observation { scenarios: vec![] }.check().is_err());
    assert!(
        Observation {
            scenarios: vec![Scenario {
                length: 0,
                control: 0,
                bound: 0,
                repeat: 0,
                invocations: [64; 3],
                steps: [1; 3],
                global_writes: [0; 3],
                backing_sha256: [0; 32]
            }]
        }
        .check()
        .is_err()
    );
}

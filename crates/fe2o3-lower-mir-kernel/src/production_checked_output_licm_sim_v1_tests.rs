use super::*;
use fe2o3_kernel_ir::{
    CanonicalKirOperationCoordinateV1 as Coordinate, VerifiedCanonicalKernelIrModuleV12 as Owner,
    VerifiedCanonicalKernelIrV12,
};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, ScalarBitsV1, SimulationArgumentV1, SimulationConflictAssessmentV1,
    SimulationEventKindV1 as Kind, SimulationEventSinkErrorV1, SimulationEventSinkV1,
    SimulationEventV1, SimulationLimitsV1, SimulationRaceAssessmentV1, SimulationRequestV1,
    SimulationTargetV1,
};

#[derive(Default)]
struct Effects(Vec<SimulationEventV1>);
impl SimulationEventSinkV1 for Effects {
    fn record(&mut self, event: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        if matches!(
            event.kind,
            Kind::MemoryRead { .. }
                | Kind::MemoryWrite { .. }
                | Kind::MemoryAtomic { .. }
                | Kind::MemoryFence { .. }
                | Kind::AllocationPreexisting { .. }
                | Kind::AllocationCreated { .. }
                | Kind::AllocationReleased { .. }
                | Kind::WorkgroupBarrierArrive { .. }
                | Kind::WorkgroupBarrierRelease { .. }
                | Kind::Call { .. }
                | Kind::Return
        ) {
            if self.0.len() == 32_768 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete source-LICM effect bound".into(),
                });
            }
            self.0.push(event.clone());
        }
        Ok(())
    }
}

fn simulation(owner: &Owner) -> AdmittedSimulationModuleV1 {
    let bytes = VerifiedCanonicalKernelIrV12::from_canonical_bytes(
        owner.canonical().canonical_bytes().to_vec(),
    )
    .unwrap();
    assert_eq!(bytes.identity(), owner.canonical().identity());
    AdmittedSimulationModuleV1::admit_v12(bytes, SimulationLimitsV1::default()).unwrap()
}

fn translated_effects(
    before: &Owner,
    after: &Owner,
    rows: &[fe2o3_kernel_analysis::CanonicalKirLicmOriginV1],
    events: &[SimulationEventV1],
) -> Vec<SimulationEventV1> {
    events
        .iter()
        .map(|event| {
            let mut translated = event.clone();
            let function = event.site.function_ordinal;
            let input = before.module().functions[function].body.as_ref().unwrap();
            let output = after.module().functions[function].body.as_ref().unwrap();
            let block = input
                .blocks
                .iter()
                .position(|block| block.id == event.site.block)
                .unwrap();
            assert_eq!(input.blocks[block].id, output.blocks[block].id);
            if let Some(operation) = event.site.operation {
                let coordinate = Coordinate {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            u32::try_from(function).unwrap(),
                        ),
                        block: u32::try_from(block).unwrap(),
                    },
                    operation,
                };
                let mut matching = rows.iter().filter(|row| row.input == coordinate);
                let row = matching.next().unwrap();
                assert!(matching.next().is_none());
                assert_eq!(row.hoist, None, "no observable effect is hoisted");
                assert_eq!(row.output.block, row.input.block);
                let destination = &output.blocks[row.output.block.block as usize];
                assert_eq!(
                    input.blocks[block].operations[operation as usize],
                    destination.operations[row.output.operation as usize]
                );
                translated.site.block = destination.id;
                translated.site.operation = Some(row.output.operation);
            } else {
                assert_eq!(
                    input.blocks[block].terminator,
                    output.blocks[block].terminator
                );
                assert_eq!(translated.site, event.site);
            }
            translated
        })
        .collect()
}

fn compare(
    before: &Owner,
    after: &Owner,
    rows: &[fe2o3_kernel_analysis::CanonicalKirLicmOriginV1],
    mutation: bool,
) -> usize {
    let first = simulation(before);
    let last = simulation(after);
    let mut comparisons = 0;
    for kernel in &before.module().kernels {
        let output_kernel = after
            .module()
            .kernels
            .iter()
            .find(|candidate| candidate.id == kernel.id)
            .unwrap();
        assert_eq!(kernel.domain, output_kernel.domain);
        assert_eq!(kernel.workgroup_size, output_kernel.workgroup_size);
        let mut grid = [1_u64; 3];
        for (axis, extent) in kernel.domain.extents().enumerate() {
            let fe2o3_kernel_ir::LaunchExtent::Static(extent) = extent else {
                panic!("actual static source launch")
            };
            grid[axis] = extent.into();
        }
        let workgroup = kernel.workgroup_size.unwrap();
        let workgroup = [workgroup.x, workgroup.y, workgroup.z];
        assert_eq!((grid, workgroup), ([64, 1, 1], [64, 1, 1]));
        for control in [0_u32, 1, 2, u32::MAX] {
            let arguments = vec![SimulationArgumentV1::Scalar(ScalarBitsV1::u32(control))];
            let request =
                SimulationRequestV1::new(kernel.id.as_str(), grid, workgroup, arguments.clone());
            let unchanged = request.clone();
            let expected = if !mutation {
                (0, 0)
            } else {
                match control {
                    0 => (3, 3),
                    1 => (1, 0),
                    2 => (2, 2),
                    _ => (0, 0),
                }
            };
            let mut previous = None;
            for _ in 0..2 {
                let mut original_effects = Effects::default();
                let mut final_effects = Effects::default();
                let a = first
                    .simulate_observed_with_sink(
                        &request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        &mut original_effects,
                    )
                    .unwrap();
                let b = last
                    .simulate_observed_with_sink(
                        &request,
                        SimulationTargetV1::amdgpu_64(),
                        SimulationLimitsV1::default(),
                        &mut final_effects,
                    )
                    .unwrap();
                assert_eq!(a.arguments(), b.arguments());
                assert_eq!(a.arguments(), arguments);
                assert_eq!(a.shared_buffers(), b.shared_buffers());
                assert_eq!(
                    translated_effects(before, after, rows, &original_effects.0),
                    final_effects.0
                );
                for events in [&original_effects.0, &final_effects.0] {
                    assert_eq!(
                        events
                            .iter()
                            .filter(|e| matches!(e.kind, Kind::MemoryWrite { .. }))
                            .count(),
                        expected.0 * 64
                    );
                    assert_eq!(
                        events
                            .iter()
                            .filter(|e| matches!(e.kind, Kind::MemoryRead { .. }))
                            .count(),
                        expected.1 * 64
                    );
                    assert!(events.iter().all(|e| match e.kind {
                        Kind::MemoryWrite { offset, bytes, .. }
                        | Kind::MemoryRead { offset, bytes, .. } => offset == 0 && bytes == 4,
                        _ => true,
                    }));
                }
                for result in [&a, &b] {
                    assert_eq!(result.invocations_executed(), 64);
                    assert!(!result.grants_execution_authority());
                    assert!(matches!(
                        result.conflict_assessment(),
                        SimulationConflictAssessmentV1::NoConflictsObserved
                    ));
                    assert!(matches!(
                        result.race_assessment(),
                        SimulationRaceAssessmentV1::NoRacesObserved { .. }
                    ));
                }
                let current = (a, b, original_effects.0, final_effects.0);
                if let Some(previous) = previous {
                    assert_eq!(previous, current);
                }
                previous = Some(current);
                comparisons += 1;
            }
            assert_eq!(request, unchanged);
        }
    }
    comparisons
}

#[test]
fn source_licm_direct_sim_preserves_dynamic_zero_trip_and_ordered_private_effects() {
    let mut comparisons = 0;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (prefix, inherited) = direct::prefix(profile, mutation);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited).unwrap();
            let (owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
            budget.reserve_storage(added.retained_storage()).unwrap();
            owner.verify_equivalence(&mut budget).unwrap();
            if mutation {
                actual_mutation(
                    owner.prefix().output(),
                    owner.output(),
                    owner.operation_origins(),
                );
            }
            comparisons += compare(
                owner.prefix().output(),
                owner.output(),
                owner.operation_origins(),
                mutation,
            );
        }
    }
    assert_eq!(comparisons, 32);
}

#[test]
fn source_licm_unit_local_sim_preserves_both_roots_and_ordered_private_effects() {
    let mut comparisons = 0;
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for mutation in [false, true] {
            let (prefix, inherited) = erased::prefix(profile, mutation);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(inherited).unwrap();
            let (owner, added) = prefix.continue_licm_v1(&mut budget).unwrap();
            budget.reserve_storage(added.retained_storage()).unwrap();
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(owner.kernels().len(), 2);
            if mutation {
                actual_mutation(
                    owner.prefix().output(),
                    owner.output(),
                    owner.operation_origins(),
                );
            }
            comparisons += compare(
                owner.prefix().output(),
                owner.output(),
                owner.operation_origins(),
                mutation,
            );
        }
    }
    assert_eq!(comparisons, 64);
}

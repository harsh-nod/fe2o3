//! Constructed semantic-source owners, not authenticated rustc capture.
#[path = "production_pipeline_induction_refinement_source_v1_tests.rs"]
mod induction_refinement_tests;
#[path = "production_pipeline_licm_loop_induction_v1_tests.rs"]
mod loop_induction_tests;
use super::*;
use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
use crate::production_ranked_projection_v1::{
    with_backend_licm_direct_prefix_v1, with_backend_licm_erased_prefix_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLicmOriginV1 as Origin;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirOperationCoordinateV1 as Coordinate, Operation, OperationKind,
};
use std::mem::size_of_val;

fn with_prefix(
    erased: bool,
    profile: Profile,
    mutation: bool,
    next: impl FnOnce(Prefix6, &mut Budget<'_>),
) {
    if erased {
        with_backend_licm_erased_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Erased(owner), budget)
        });
    } else {
        with_backend_licm_direct_prefix_v1(profile, mutation, |owner, _, budget| {
            next(Prefix6::Direct(owner), budget)
        });
    }
}
fn rows(owner: &Licm) -> &[Origin] {
    match owner {
        Licm::Direct(v) => v.operation_origins(),
        Licm::Erased(v) => v.operation_origins(),
    }
}
fn operation(owner: &Graph, coordinate: Coordinate) -> &Operation {
    &owner.module().functions[coordinate.block.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[coordinate.block.block as usize]
        .operations[coordinate.operation as usize]
}
fn assert_shape(owner: &Licm, mutation: bool) {
    let (preheaders, parameters, incoming, reports) = match owner {
        Licm::Direct(v) => (
            v.prefix().origins(),
            v.prefix().parameter_origins(),
            v.prefix().incoming_origins(),
            v.kernels(),
        ),
        Licm::Erased(v) => (
            v.prefix().origins(),
            v.prefix().parameter_origins(),
            v.prefix().incoming_origins(),
            v.kernels(),
        ),
    };
    assert_eq!(preheaders.len(), if mutation { 2 } else { 0 });
    assert_eq!(incoming.len(), if mutation { 4 } else { 0 });
    assert_eq!(reports.len(), 2);
    if mutation {
        assert!(!parameters.is_empty());
        for row in preheaders {
            assert_eq!(row.incoming().len(), 2);
            assert!(!row.parameters().is_empty());
        }
        assert_ne!(
            owner.preheader_input().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
    } else {
        assert!(parameters.is_empty());
        assert!(rows(owner).iter().all(|row| row.hoist.is_none()));
        assert_eq!(
            owner.preheader_input().canonical().canonical_bytes(),
            owner.output().canonical().canonical_bytes()
        );
        assert!(!std::ptr::eq(owner.preheader_input(), owner.output()));
    }
    let before = owner.preheader_input().module();
    let after = owner.output().module();
    assert_eq!(before.kernels, after.kernels);
    assert_eq!(before.functions.len(), after.functions.len());
    let mut count = 0;
    for (index, (old, new)) in before.functions.iter().zip(&after.functions).enumerate() {
        assert_eq!(old.signature, new.signature);
        let (Some(old), Some(new)) = (&old.body, &new.body) else {
            continue;
        };
        assert_eq!(old.parameters, new.parameters);
        assert_eq!(old.blocks.len(), new.blocks.len());
        for (a, b) in old.blocks.iter().zip(&new.blocks) {
            assert_eq!(
                (a.id, &a.parameters, &a.terminator),
                (b.id, &b.parameters, &b.terminator)
            );
            count += a.operations.len();
        }
        if mutation {
            let moved: Vec<_> = rows(owner)
                .iter()
                .filter(|r| r.input.block.function.0 as usize == index && r.hoist.is_some())
                .collect();
            let masks: Vec<_> = moved
                .iter()
                .filter(|r| {
                    matches!(
                        operation(owner.preheader_input(), r.input).kind,
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            ..
                        }
                    )
                })
                .collect();
            let compares: Vec<_> = moved
                .iter()
                .filter(|r| {
                    matches!(
                        operation(owner.preheader_input(), r.input).kind,
                        OperationKind::Compare { .. }
                    )
                })
                .collect();
            assert_eq!((masks.len(), compares.len()), (1, 1));
            let input_ordinal = usize::from(matches!(owner, Licm::Erased(_)));
            assert_eq!(old.parameters.len(), input_ordinal + 2);
            assert_eq!(
                before.functions[index].signature.parameters[input_ordinal],
                fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)
            );
            assert_eq!(
                before.functions[index].signature.parameters[input_ordinal + 1],
                fe2o3_kernel_ir::Type::Scalar(fe2o3_kernel_ir::ScalarType::U64)
            );
            let mask = operation(owner.preheader_input(), masks[0].input);
            let OperationKind::Binary { lhs, rhs, .. } = mask.kind else {
                unreachable!()
            };
            assert!(
                lhs == old.parameters[input_ordinal] || rhs == old.parameters[input_ordinal],
                "actual input-dependent hoist, not a folded fixture constant"
            );
            let compare = operation(owner.preheader_input(), compares[0].input);
            let OperationKind::Compare { lhs, rhs, .. } = compare.kind else {
                unreachable!()
            };
            assert!(
                lhs == mask.results[0].id || rhs == mask.results[0].id,
                "dependent Compare hoist retains exact SSA producer"
            );
        }
    }
    assert_eq!(rows(owner).len(), count);
    let mut inputs = std::collections::BTreeSet::new();
    let mut destinations = std::collections::BTreeSet::new();
    for row in rows(owner) {
        assert!(inputs.insert(row.input));
        assert!(destinations.insert(row.output));
        assert_eq!(
            operation(owner.preheader_input(), row.input),
            operation(owner.output(), row.output)
        );
    }
    if mutation {
        assert!(rows(owner).iter().any(|row| row.hoist.is_none()
            && row.input.operation != row.output.operation
            && matches!(
                operation(owner.preheader_input(), row.input).kind,
                OperationKind::Store { .. }
            )));
    }
    if let Licm::Erased(v) = owner {
        let original = v
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .prefix()
            .original_source();
        assert_eq!(
            original.semantic_ssa().source_semantic().functions().len(),
            3
        );
        assert_eq!(
            v.prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased_source()
                .deleted_call_count(),
            2
        );
        assert_eq!(
            v.prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased_source()
                .deleted_function_count(),
            1
        );
    }
}

#[test]
fn licm_native_direct_and_unit_local_motion_emit_only_the_actual_final_graph_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, true, |prefix, budget| {
                let original = match &prefix {
                    Prefix6::Direct(v) => v.source_semantic_kir().pre_ranked_executable().unwrap(),
                    Prefix6::Erased(v) => v.original_source().executable(),
                }
                .canonical()
                .canonical_bytes()
                .as_ptr();
                let sibling = vec![0xb7_u8; 53];
                let sibling_bytes = size_of_val(&sibling) + sibling.capacity();
                budget.reserve_storage(sibling_bytes).unwrap();
                let floor = budget.storage();
                let ledger = budget.work_ledger_identity_v1();
                let (value, receipt) = prepare(prefix, profile, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_eq!(value.retained_storage_floor_v1(), budget.storage());
                assert_eq!(
                    value
                        .original()
                        .unwrap()
                        .canonical()
                        .canonical_bytes()
                        .as_ptr(),
                    original
                );
                assert_eq!(matches!(value.owner, Licm::Erased(_)), erased);
                assert_shape(&value.owner, true);
                assert_eq!(value.prefix_execution.policy_version(), 7);
                assert!(!value.grants_artifact_or_launch_authority());
                assert!(!value.llvm_ir().contains(".fe2o3.kd.v1"));
                value.verify_equivalence(budget).unwrap();
                let (expected, storage) = lower_native(value.output(), profile, budget).unwrap();
                budget.reserve_storage(storage).unwrap();
                assert_eq!(expected, value.llvm_ir());
                drop(expected);
                budget.release_storage(storage).unwrap();
                for historical in [
                    value.preheader_input(),
                    value.promoted_input(),
                    value.historical_p8_output(),
                ] {
                    let (text, storage) = lower_native(historical, profile, budget).unwrap();
                    budget.reserve_storage(storage).unwrap();
                    assert_ne!(text, value.llvm_ir());
                    assert!(matches!(
                        check_native_text(value.output(), profile, &text, budget),
                        Err(ProductionPipelineError::LicmNativeStage(
                            LicmNativeStageErrorV1::Mismatch("exact LICM native LLVM")
                        ))
                    ));
                    drop(text);
                    budget.release_storage(storage).unwrap();
                }
                observe_actual_licm_recurrences_v1(&value, erased, profile, budget);
                assert!(budget.work_ledger_identity_v1() == ledger);
                assert!(sibling.iter().all(|b| *b == 0xb7));
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.release_storage(sibling_bytes).unwrap();
            });
        }
    }
}

fn observe_actual_licm_recurrences_v1(
    value: &PreparedLicmNativeOutputV1,
    erased: bool,
    profile: Profile,
    budget: &mut Budget<'_>,
) {
    use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, CanonicalKirLoopsV1};

    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let result: Result<()> = scoped(floor, budget, |budget| {
        let graph = value.output();
        let (inventory, storage) = CanonicalKirInventoryV1::derive(graph, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (loops, storage) =
            CanonicalKirLoopsV1::derive(&inventory, Default::default(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        loops
            .replay(&inventory, Default::default(), budget)
            .unwrap();
        assert!(std::ptr::eq(inventory.owner(), graph));
        assert!(loops.belongs_to(&inventory));
        assert!(loops.loop_count() <= 16, "bounded diagnostic loop roster");
        let mut total = 0usize;
        eprintln!(
            "POST_LICM_RECURRENCE_BEGIN erased={erased} profile={profile:?} graph={:?} loops={}",
            graph.canonical().identity(),
            loops.loop_count(),
        );
        for index in 0..loops.loop_count() {
            let natural = loops.natural_loop(index, budget).unwrap();
            let recurrences = loops.recurrences(index, budget).unwrap();
            total = total.checked_add(recurrences.len()).unwrap();
            assert!(total <= 64, "bounded diagnostic recurrence roster");
            eprintln!(
                "POST_LICM_LOOP erased={erased} profile={profile:?} ordinal={index} header={:?} preheader={:?} recurrences={}",
                natural.header(),
                natural.unconditional_preheader(),
                recurrences.len(),
            );
            for (ordinal, recurrence) in recurrences.iter().enumerate() {
                budget.charge_work(1).unwrap();
                eprintln!(
                    "POST_LICM_RECURRENCE erased={erased} profile={profile:?} loop={index} ordinal={ordinal} row={recurrence:?}",
                );
            }
        }
        eprintln!(
            "POST_LICM_RECURRENCE_END erased={erased} profile={profile:?} loops={} recurrences={total}",
            loops.loop_count(),
        );
        Ok(())
    });
    result.unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn licm_native_direct_and_unit_local_noop_have_exact_fresh_owners_both_profiles() {
    for erased in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_prefix(erased, profile, false, |prefix, budget| {
                let floor = budget.storage();
                let (value, receipt) = prepare(prefix, profile, budget).unwrap();
                assert_eq!(budget.storage(), floor);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert_shape(&value.owner, false);
                let (text, storage) =
                    lower_native(value.preheader_input(), profile, budget).unwrap();
                budget.reserve_storage(storage).unwrap();
                assert_eq!(text, value.llvm_ir());
                assert!(!value.grants_artifact_or_launch_authority());
                value.verify_equivalence(budget).unwrap();
                drop(text);
                budget.release_storage(storage).unwrap();
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }
}

#[test]
fn licm_native_fresh_preparations_preserve_exact_bytes_origins_and_accounting() {
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                let mut previous = None;
                for _ in 0..2 {
                    with_prefix(erased, profile, mutation, |prefix, budget| {
                        let (value, receipt) = prepare(prefix, profile, budget).unwrap();
                        let observation = (
                            value.output().canonical().canonical_bytes().to_vec(),
                            value.llvm_ir().to_owned(),
                            value.prefix_execution.canonical_bytes().to_vec(),
                            rows(&value.owner).to_vec(),
                            receipt.retained_storage(),
                            budget.work(),
                            budget.peak_storage(),
                        );
                        if let Some(previous) = &previous {
                            assert_eq!(previous, &observation);
                        }
                        previous = Some(observation);
                    });
                }
            }
        }
    }
}

#[test]
fn licm_native_replay_refuses_same_length_text_wrong_target_and_missing_floor() {
    for erased in [false, true] {
        for mutation in [false, true] {
            with_prefix(erased, Profile::Gfx942, mutation, |prefix, budget| {
                let (mut value, receipt) = prepare(prefix, Profile::Gfx942, budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let floor = budget.storage();
                let capacity = value.llvm.capacity();
                let first = value.llvm.remove(0);
                value.llvm.insert(0, if first == ';' { ' ' } else { ';' });
                assert_eq!(value.llvm.capacity(), capacity);
                assert!(matches!(
                    value.verify_equivalence(budget),
                    Err(ProductionPipelineError::LicmNativeStage(
                        LicmNativeStageErrorV1::Mismatch("exact LICM native LLVM")
                    ))
                ));
                value.llvm.remove(0);
                value.llvm.insert(0, first);
                value.verify_equivalence(budget).unwrap();
                match check_native_text(value.output(), Profile::Gfx950, value.llvm_ir(), budget) {
                    Err(ProductionPipelineError::TargetLowering(e)) => assert!(
                        e.contains(dialect_amdgcn::LoweringDiagnosticCode::UnsupportedCapability)
                    ),
                    _ => panic!("wrong exact target must fail target preflight"),
                }
                assert_eq!(budget.storage(), floor);
                budget.release_storage(1).unwrap();
                let work = budget.work();
                assert!(matches!(
                    value.verify_equivalence(budget),
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        P7Error::Resource(Resource::Accounting)
                    ))
                ));
                assert_eq!((budget.storage(), budget.work()), (floor - 1, work));
                budget.reserve_storage(1).unwrap();
                drop(value);
                budget.release_storage(receipt.retained_storage()).unwrap();
            });
        }
    }
}

#[test]
fn licm_native_replay_requires_the_retained_genuine_p7_execution_witness() {
    with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
        let (mut first, receipt) = prepare(prefix, Profile::Gfx942, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let floor = budget.storage();
        with_prefix(false, Profile::Gfx950, true, |prefix, other| {
            let (mut second, receipt) = prepare(prefix, Profile::Gfx950, other).unwrap();
            other.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                first.prefix_execution.retained_storage(),
                second.prefix_execution.retained_storage()
            );
            assert_ne!(
                first.prefix_execution.canonical_bytes(),
                second.prefix_execution.canonical_bytes()
            );
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            assert!(matches!(
                first.verify_equivalence(budget),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Execution("Policy7 complete execution transcript")
                ))
            ));
            assert_eq!(budget.storage(), floor);
            std::mem::swap(&mut first.prefix_execution, &mut second.prefix_execution);
            first.verify_equivalence(budget).unwrap();
            second.verify_equivalence(other).unwrap();
            drop(second);
            other.release_storage(receipt.retained_storage()).unwrap();
        });
        drop(first);
        budget.release_storage(receipt.retained_storage()).unwrap();
    });
}

#[test]
fn licm_native_partial_error_and_panic_drop_real_completed_owners_and_keep_siblings() {
    use std::{cell::Cell, rc::Rc};
    struct Dropped(Rc<Cell<bool>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    for panic in [false, true] {
        with_prefix(false, Profile::Gfx942, true, |prefix, budget| {
            let sibling = vec![0x43_u8; 43];
            let bytes = size_of_val(&sibling) + sibling.capacity();
            budget.reserve_storage(bytes).unwrap();
            let floor = budget.storage();
            let work = budget.work();
            let ledger = budget.work_ledger_identity_v1();
            let dropped = Rc::new(Cell::new(false));
            let result: Result<()> = scoped(floor, budget, |budget| {
                let (value, receipt) = prepare(prefix, Profile::Gfx942, budget)?;
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                let _live = (value, Dropped(dropped.clone()));
                if panic {
                    std::panic::panic_any(73_u32);
                }
                Err(mismatch("injected after genuine LICM native owner"))
            });
            if panic {
                assert!(matches!(
                    result,
                    Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                        P7Error::Panicked
                    ))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionPipelineError::LicmNativeStage(
                        LicmNativeStageErrorV1::Mismatch(
                            "injected after genuine LICM native owner"
                        )
                    ))
                ));
            }
            assert!(dropped.get());
            assert_eq!(budget.storage(), floor);
            assert!(budget.work() > work && budget.peak_storage() > floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(sibling.iter().all(|b| *b == 0x43));
            budget.release_storage(bytes).unwrap();
        });
    }
}

use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationConflictAssessmentV1, SimulationEventKindV1 as EventKind, SimulationEventSinkErrorV1,
    SimulationEventSinkV1, SimulationEventV1, SimulationExecutionV1, SimulationLimitsV1,
    SimulationRaceAssessmentV1, SimulationRequestV1, SimulationTargetV1,
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
            if self.0.len() == 32_768 {
                return Err(SimulationEventSinkErrorV1 {
                    detail: "complete native LICM effect bound".into(),
                });
            }
            self.0.push(event.clone());
        }
        Ok(())
    }
}

fn translated_effects(
    before: &Graph,
    after: &Graph,
    origins: &[Origin],
    events: &[SimulationEventV1],
) -> Vec<SimulationEventV1> {
    events
        .iter()
        .map(|event| {
            let mut mapped = event.clone();
            let function = event.site.function_ordinal;
            let input = before.module().functions[function].body.as_ref().unwrap();
            let output = after.module().functions[function].body.as_ref().unwrap();
            let block = input
                .blocks
                .iter()
                .position(|b| b.id == event.site.block)
                .unwrap();
            assert_eq!(input.blocks[block].id, output.blocks[block].id);
            if let Some(op) = event.site.operation {
                let coordinate = Coordinate {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            function.try_into().unwrap(),
                        ),
                        block: block.try_into().unwrap(),
                    },
                    operation: op,
                };
                let mut matches = origins.iter().filter(|r| r.input == coordinate);
                let row = matches.next().unwrap();
                assert!(matches.next().is_none());
                assert_eq!(row.hoist, None, "observable effects may not move");
                assert_eq!(row.input.block, row.output.block);
                assert_eq!(operation(before, coordinate), operation(after, row.output));
                mapped.site.block = output.blocks[row.output.block.block as usize].id;
                mapped.site.operation = Some(row.output.operation);
            } else {
                assert_eq!(
                    input.blocks[block].terminator,
                    output.blocks[block].terminator
                );
                assert_eq!(mapped.site, event.site);
            }
            mapped
        })
        .collect()
}

fn simulate(
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
    let mut events = Effects::default();
    let result = module
        .simulate_observed_with_sink(
            request,
            SimulationTargetV1::amdgpu_64(),
            SimulationLimitsV1::default(),
            &mut events,
        )
        .unwrap();
    (result, events.0)
}

fn source_simulation_launch(
    value: &PreparedLicmNativeOutputV1,
    ordinal: usize,
) -> ([u64; 3], [u32; 3]) {
    let (roster, semantic) = match &value.owner {
        Licm::Direct(owner) => {
            let source = owner
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
        Licm::Erased(owner) => {
            let source = owner
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
    let original = value.original().unwrap();
    for graph in [original, value.preheader_input()] {
        assert_eq!(graph.module().kernels.len(), roster.roots().len());
    }
    assert_eq!(&value.preheader_input().module().kernels[ordinal], kernel);
    // Target binding adds exactly the selected processor and Wave64, not geometry.
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
        assert_ne!(
            grid[axis], 0,
            "this retained source has a concrete grid ceiling"
        );
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

fn compare_simulation(value: &PreparedLicmNativeOutputV1, erased: bool, mutation: bool) -> usize {
    let mut comparisons = 0;
    for (ordinal, kernel) in value.output().module().kernels.iter().enumerate() {
        let (grid, workgroup) = source_simulation_launch(value, ordinal);
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
            .flat_map(|control| bounds.iter().map(move |bound| (control, bound)))
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
                        fe2o3_kernel_ir::ScalarType::U64,
                        bound.into(),
                        SimulationTargetV1::amdgpu_64(),
                    )
                    .unwrap(),
                ));
            }
            let request =
                SimulationRequestV1::new(kernel.id.as_str(), grid, workgroup, arguments.clone());
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
                let original = simulate(value.original().unwrap(), &request);
                let before = simulate(value.preheader_input(), &request);
                let after = simulate(value.output(), &request);
                assert_eq!(original.0.arguments(), before.0.arguments());
                assert_eq!(before.0.arguments(), after.0.arguments());
                assert_eq!(original.0.shared_buffers(), after.0.shared_buffers());
                assert_eq!(before.0.shared_buffers(), after.0.shared_buffers());
                assert_eq!(
                    translated_effects(
                        value.preheader_input(),
                        value.output(),
                        rows(&value.owner),
                        &before.1
                    ),
                    after.1
                );
                for (result, events) in [&original, &before, &after] {
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
                        assert_eq!(events.iter().filter(|e| matches!(e.kind,
                            EventKind::MemoryWrite { allocation, offset: 0, bytes: 4 } if allocation == globals[0])).count(),
                            (1 + trips) as usize);
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
                let observation = (original, before, after);
                if let Some(previous) = previous {
                    assert_eq!(previous, observation);
                }
                previous = Some(observation);
                comparisons += 1;
            }
            assert_eq!(request, untouched);
        }
    }
    comparisons
}

#[test]
fn licm_native_sim_preserves_original_outputs_and_exact_final_effect_sites() {
    let mut comparisons = 0;
    for erased in [false, true] {
        for mutation in [false, true] {
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_prefix(erased, profile, mutation, |prefix, budget| {
                    let (value, receipt) = prepare(prefix, profile, budget).unwrap();
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_shape(&value.owner, mutation);
                    value.verify_equivalence(budget).unwrap();
                    comparisons += compare_simulation(&value, erased, mutation);
                    drop(value);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                });
            }
        }
    }
    assert_eq!(
        comparisons, 232,
        "all scalar controls and zero/one/many bounds; unchanged no-op routes"
    );
}

#[path = "production_pipeline_licm_native_resources_v1_tests.rs"]
mod resources;

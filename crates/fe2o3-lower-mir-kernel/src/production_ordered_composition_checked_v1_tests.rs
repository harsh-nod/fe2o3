//! Inert source-owner controls. They are not live rustc or native qualification.
use super::*;

#[path = "production_ordered_composition_ranked_switch_v1_tests.rs"]
mod source_shaped_switch;

fn checked_trial(
    case: CompositionCase,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (bool, usize, usize) {
    let (mut ssa, launch) = composition_owners(composition_request(case));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let pre = ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let pre_storage = pre.retained_storage().retained_storage();
    budget.reserve_storage(pre_storage).unwrap();
    let live = budget.storage();
    let bytes = pre.executable().canonical_bytes().to_vec();
    let result = ProductionOrderedCompositionCheckedKirOwnerV1::try_check(pre, &mut budget);
    assert_eq!(budget.storage(), live);
    let ok = result.is_ok();
    if let Ok(owner) = &result {
        assert_eq!(owner.executable().canonical_bytes(), bytes);
        assert!(!owner.grants_artifact_or_launch_authority());
        assert!(owner.formal_obligations().accesses().is_empty());
        assert!(owner.launch_envelope_requirements().is_empty());
        assert_eq!(
            owner
                .ranked_dependencies()
                .iter()
                .filter(|r| r.descriptor_ordinal().is_some())
                .count(),
            owner.composition().expanded_instruction_count()
        );
        assert!(owner.ranked_analysis_retained_storage_upper_bound() > 0);
    }
    let accepted = budget.work();
    let peak = budget.peak_storage();
    drop(result);
    budget
        .release_storage(pre_storage + capture.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), floor);
    (ok, accepted, peak)
}
#[test]
fn ordered_composition_checked_root_helper_twice_scalar_and_wrapping() {
    for case in [
        CompositionCase::Root,
        CompositionCase::TwoRoot,
        CompositionCase::Helper,
        CompositionCase::Twice,
        CompositionCase::RootHelper,
        CompositionCase::ScalarHelper,
        CompositionCase::Wrapping,
    ] {
        assert!(checked_trial(case, 41, 1_000_000_000, 256 * 1024 * 1024).0);
    }
}
#[test]
fn ordered_composition_checked_exact_and_one_short_keep_floor_and_work() {
    for floor in [0, 83] {
        let (ok, work, peak) = checked_trial(
            CompositionCase::Twice,
            floor,
            1_000_000_000,
            256 * 1024 * 1024,
        );
        assert!(ok);
        assert_eq!(
            checked_trial(CompositionCase::Twice, floor, work, peak),
            (true, work, peak)
        );
        assert!(!checked_trial(CompositionCase::Twice, floor, work - 1, peak).0);
        assert!(!checked_trial(CompositionCase::Twice, floor, work, peak - 1).0);
    }
}
#[test]
fn ordered_composition_checked_input_floor_is_required() {
    let (mut ssa, launch) = composition_owners(composition_request(CompositionCase::Helper));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 256 * 1024 * 1024);
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let pre = ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let floor = budget.storage();
    assert!(ProductionOrderedCompositionCheckedKirOwnerV1::try_check(pre, &mut budget).is_err());
    assert_eq!(budget.storage(), floor);
}
#[test]
fn ordered_composition_checked_replay_and_call_instance_dependencies_are_exact() {
    let (mut ssa, launch) = composition_owners(composition_request(CompositionCase::Twice));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 256 * 1024 * 1024);
    budget.reserve_storage(29).unwrap();
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let pre = ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let pre_storage = pre.retained_storage().retained_storage();
    budget.reserve_storage(pre_storage).unwrap();
    let mut owner =
        ProductionOrderedCompositionCheckedKirOwnerV1::try_check(pre, &mut budget).unwrap();
    let additional = owner.retained_storage() - pre_storage;
    budget.reserve_storage(additional).unwrap();
    let floor = budget.storage();
    owner.verify_equivalence(&mut budget).unwrap();
    let calls = owner.composition().calls();
    assert_eq!(calls.len(), 2);
    for call in calls {
        assert!(
            owner
                .ranked_dependencies()
                .iter()
                .any(|r| r.incoming_call() == Some(call.key()) && r.descriptor_ordinal().is_some())
        );
    }
    super::super::super::ordered_composition_checks_v1::substitute_dependency_for_test(&mut owner);
    assert!(owner.verify_equivalence(&mut budget).is_err());
    assert_eq!(budget.storage(), floor);
}

use fe2o3_kernel_ir as ordered_composition_fixture_ir;
#[path = "../../fe2o3-kernel-ir/tests/fixtures/ordered_composition_v1.rs"]
mod canonical_fixture;

fn canonical_memory_module(coefficient: u32) -> fe2o3_kernel_ir::Module {
    use fe2o3_kernel_ir::*;
    let mut module = canonical_fixture::module(0, &[1], &[0, 0], 3);
    let body = module.functions[0].body.as_mut().unwrap();
    let block = &mut body.blocks[0];
    let mut store = block.operations.pop().unwrap();
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(900), Type::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    ));
    let offset = if coefficient == 1 {
        ValueId(900)
    } else {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(901), Type::INDEX),
            OperationKind::Constant(Constant::Index(u64::from(coefficient))),
        ));
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(902), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Multiply,
                lhs: ValueId(900),
                rhs: ValueId(901),
            },
        ));
        ValueId(902)
    };
    block.operations.push(Operation::effect_free(
        ValueDef::new(
            ValueId(903),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::GetElementPointer {
            base: ValueId(3),
            offset,
        },
    ));
    let OperationKind::Store { pointer, .. } = &mut store.kind else {
        panic!()
    };
    *pointer = ValueId(903);
    block.operations.push(store);
    module
}
#[test]
fn ordered_composition_ranked_actual_coordinate_and_explicit_extra_bound() {
    use fe2o3_kernel_ir::*;
    with_composition(CompositionCase::Twice, |source, budget| {
        let module = canonical_memory_module(1);
        let (canonical, storage) =
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                &module, budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (owner, roster) =
            VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, budget).unwrap();
        budget.reserve_storage(roster.retained_storage()).unwrap();
        let live = budget.storage();
        let (recipe, bounds) =
            super::super::super::ordered_composition_checks_v1::project_for_test(
                &owner,
                source.source_launch(),
                budget,
            )
            .unwrap();
        assert_eq!(budget.storage(), live);
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].parameter_index(), 3);
        assert_eq!(bounds[0].minimum_byte_len(), 256);
        assert!(bounds[0].requires_write_permission());
        assert!(!bounds[0].requires_initialized_read());
        let construction = fe2o3_pliron::ProductionConstructionV1::ranked_kernel(
            "composition_memory_projection_test",
            recipe,
        )
        .unwrap();
        let checked = fe2o3_pliron::compile_ranked_kernel_for_gfx942_lowering_v1(
            construction,
            fe2o3_pliron::ProductionSessionLimitsV1::default(),
            std::iter::empty(),
        )
        .unwrap();
        assert!(checked.all_mandatory_reports_are_clean());
        drop((checked, bounds, owner));
        budget
            .release_storage(roster.retained_storage() + storage.retained_storage())
            .unwrap();
    });
}
#[test]
fn ordered_composition_ranked_different_coordinate_and_constant_write_refuse() {
    use fe2o3_kernel_ir::*;
    with_composition(CompositionCase::Twice, |source, budget| {
        for module in [
            canonical_memory_module(2),
            canonical_fixture::module(0, &[1], &[0], 1),
        ] {
            let (canonical, storage) =
                VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                    &module, budget,
                )
                .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (owner, roster) =
                VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, budget)
                    .unwrap();
            budget.reserve_storage(roster.retained_storage()).unwrap();
            let live = budget.storage();
            assert!(
                super::super::super::ordered_composition_checks_v1::project_for_test(
                    &owner,
                    source.source_launch(),
                    budget
                )
                .is_err()
            );
            assert_eq!(budget.storage(), live);
            drop(owner);
            budget
                .release_storage(roster.retained_storage() + storage.retained_storage())
                .unwrap();
        }
    });
}

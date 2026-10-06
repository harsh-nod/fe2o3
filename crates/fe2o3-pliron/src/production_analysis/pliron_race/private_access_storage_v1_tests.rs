use super::*;
use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
use dialect_kernel::{IndexType, RankedViewType, ReturnOp};
use pliron::{
    builtin::{attributes::UnitAttr, types::FunctionType},
    debug_info::set_operation_result_name,
    dialect::DialectName,
};

fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
    ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
}

fn context() -> Context {
    let mut context = Context::new();
    dialect_kernel::register_dialect(
        &mut context,
        &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    context
}

fn fixture(context: &mut Context, space: MemorySpaceAttr) -> (FuncOp, RankedViewOp) {
    fixture_with_globals(context, space, false)
}

fn fixture_with_globals(
    context: &mut Context,
    space: MemorySpaceAttr,
    globals: bool,
) -> (FuncOp, RankedViewOp) {
    let ty = IndexType::get(context).into();
    let function_type = FunctionType::get(context, vec![ty], vec![]);
    let function = FuncOp::new(
        context,
        "private_storage".try_into().unwrap(),
        function_type,
    );
    let entry = function.get_entry_block(context);
    let index = entry.deref(context).get_argument(0);
    let view_type = RankedViewType::new(context, 32, true, vec![4]).unwrap();
    let view = RankedViewOp::new_in_space(context, view_type, vec![], space).unwrap();
    view.get_operation().insert_at_back(entry, context);
    let private_value = view.result(context);
    for _ in 0..4 {
        RankedAccessOp::new(context, AccessKindAttr::Read, private_value, vec![index])
            .unwrap()
            .get_operation()
            .insert_at_back(entry, context);
    }
    if globals {
        let view_type = RankedViewType::new(context, 32, true, vec![32_768]).unwrap();
        let global =
            RankedViewOp::new_in_space(context, view_type, vec![], MemorySpaceAttr::Global)
                .unwrap();
        global.get_operation().insert_at_back(entry, context);
        let global_value = global.result(context);
        for _ in 0..4 {
            RankedAccessOp::new(context, AccessKindAttr::Write, global_value, vec![index])
                .unwrap()
                .get_operation()
                .insert_at_back(entry, context);
        }
        for ordinal in 0..12 {
            AllocationEffectOp::new(
                context,
                AccessKindAttr::Read,
                MemorySpaceAttr::Global,
                1 + ordinal % 2,
                1,
            )
            .unwrap()
            .get_operation()
            .insert_at_back(entry, context);
        }
    }
    ReturnOp::new(context)
        .get_operation()
        .insert_at_back(entry, context);
    (function, view)
}

fn census(context: &Context, function: &FuncOp) -> ProductionAnalysisInputCensusV1 {
    LivePlironStructuralIdentityProviderV1::new(context, function)
        .capture_with_resource_limits_v1(unlimited())
        .ok()
        .unwrap()
        .input_census
}

fn names(context: &Context, function: &FuncOp) -> RaceNameCensusV1 {
    let census = census(context, function);
    let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
    collect_race_name_census_v1(
        context,
        function,
        &inventory,
        census,
        unlimited(),
        |value| value.unique_name_byte_len(context),
    )
    .unwrap()
}

#[test]
fn actual_census_discounts_only_direct_private_ranked_views() {
    for (space, expected) in [
        (MemorySpaceAttr::Private, 4),
        (MemorySpaceAttr::Global, 0),
        (MemorySpaceAttr::Workgroup, 0),
    ] {
        let mut context = context();
        let (function, _) = fixture(&mut context, space);
        let observed = names(&context, &function);
        assert_eq!(observed.private_ranked_accesses, expected);
        assert_eq!(census(&context, &function).ranked_accesses, 4);
    }
}

#[test]
fn unknown_memory_space_remains_fully_charged() {
    let mut context = context();
    let (function, view) = fixture(&mut context, MemorySpaceAttr::Private);
    let valid_census = census(&context, &function);
    view.get_operation()
        .deref_mut(&context)
        .attributes
        .set("kernel_memory_space".try_into().unwrap(), UnitAttr::new());
    assert_eq!(view.memory_space(&context), None);
    assert!(
        LivePlironStructuralIdentityProviderV1::new(&context, &function)
            .capture_with_resource_limits_v1(unlimited())
            .is_err()
    );
    // The production identity boundary already refuses this malformed attribute.
    // Exercise the lower helper with unchanged structural counts as defense in depth.
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let observed = collect_race_name_census_v1(
        &context,
        &function,
        &inventory,
        valid_census,
        unlimited(),
        |value| value.unique_name_byte_len(&context),
    )
    .unwrap();
    assert_eq!(observed.private_ranked_accesses, 0);
}

#[test]
fn block_argument_view_without_definition_remains_fully_charged() {
    let mut context = context();
    let view_type = RankedViewType::new(&context, 32, true, vec![4]).unwrap();
    let index_type = IndexType::get(&context).into();
    let function_type = FunctionType::get(&context, vec![view_type.into(), index_type], vec![]);
    let function = FuncOp::new(
        &mut context,
        "argument_view".try_into().unwrap(),
        function_type,
    );
    let entry = function.get_entry_block(&context);
    let view = entry.deref(&context).get_argument(0);
    let index = entry.deref(&context).get_argument(1);
    assert!(view.defining_op().is_none());
    RankedAccessOp::new(&mut context, AccessKindAttr::Read, view, vec![index])
        .unwrap()
        .get_operation()
        .insert_at_back(entry, &context);
    ReturnOp::new(&mut context)
        .get_operation()
        .insert_at_back(entry, &context);
    assert_eq!(census(&context, &function).ranked_accesses, 1);
    assert_eq!(names(&context, &function).private_ranked_accesses, 0);
}

#[test]
fn zero_operand_ranked_access_rejects_before_any_view_lookup() {
    let mut context = context();
    let function_type = FunctionType::get(&context, vec![], vec![]);
    let function = FuncOp::new(
        &mut context,
        "missing_view".try_into().unwrap(),
        function_type,
    );
    let entry = function.get_entry_block(&context);
    let ret = ReturnOp::new(&mut context).get_operation();
    ret.insert_at_back(entry, &context);
    let mut actual = census(&context, &function);
    let raw = Operation::new(
        &mut context,
        RankedAccessOp::get_concrete_op_info(),
        vec![],
        vec![],
        vec![],
        0,
    );
    let access = RankedAccessOp::from_operation(raw);
    access.set_attr_kernel_access_kind(&context, AccessKindAttr::Read);
    raw.insert_before(&context, ret);
    actual.operations += 1;
    actual.ranked_accesses += 1;
    actual.attributes += raw.deref(&context).attributes.0.len();
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    assert_eq!(
        collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            actual,
            unlimited(),
            |_| panic!("malformed access reached value name lookup")
        )
        .unwrap_err(),
        race_name_census_error_v1()
    );
}

#[test]
fn private_census_rejects_foreign_inventory_and_mid_scan_mutation() {
    let mut context = context();
    let (function, view) = fixture(&mut context, MemorySpaceAttr::Private);
    let (foreign, _) = fixture(&mut context, MemorySpaceAttr::Private);
    let actual = census(&context, &function);
    let foreign_inventory = BoundedPlironFunctionInventoryV1::collect(&context, &foreign).unwrap();
    assert_eq!(
        collect_race_name_census_v1(
            &context,
            &function,
            &foreign_inventory,
            actual,
            unlimited(),
            |_| panic!("foreign graph reached name query"),
        )
        .unwrap_err(),
        race_name_census_error_v1(),
    );
    let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
    let mut changed = false;
    assert_eq!(
        collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            actual,
            unlimited(),
            |value| {
                if !changed {
                    changed = true;
                    set_operation_result_name(
                        &context,
                        view.get_operation(),
                        0,
                        Some("changed".try_into().unwrap()),
                    );
                }
                value.unique_name_byte_len(&context)
            },
        )
        .unwrap_err(),
        race_name_census_error_v1(),
    );
    assert!(changed);
}

#[test]
fn private_count_cannot_exceed_authenticated_ranked_accesses() {
    let census = ProductionAnalysisInputCensusV1 {
        ranked_accesses: 4,
        ..Default::default()
    };
    let mut names = component_race_names_v1();
    names.private_ranked_accesses = 5;
    assert!(matches!(
        calculate_race_resource_upper_bound_for_shape_v1(census, names, Some((32_768, 3)), None),
        Err(error) if error == race_name_census_error_v1()
    ));
}

#[test]
fn captured_v5_effect_geometry_retains_work_and_changes_only_address_storage() {
    // Captured V5 partial root: 32768 invocations, four private accumulator
    // reads, four global writes and twelve conservative allocation reads.
    // This replays the exact effect population, not its full 221-block CFG.
    let mut context = context();
    let (function, _) = fixture_with_globals(&mut context, MemorySpaceAttr::Private, true);
    let actual = census(&context, &function);
    assert_eq!((actual.ranked_accesses, actual.allocation_effects), (8, 12));
    let names = names(&context, &function);
    assert_eq!(names.private_ranked_accesses, 4);
    let candidate =
        calculate_race_resource_upper_bound_for_shape_v1(actual, names, Some((32_768, 3)), None)
            .unwrap();
    let old_population = calculate_race_resource_upper_bound_for_shape_v1(
        actual,
        RaceNameCensusV1 {
            private_ranked_accesses: 0,
            ..names
        },
        Some((32_768, 3)),
        None,
    )
    .unwrap();
    assert_eq!(candidate.effects, 20);
    assert_eq!(candidate.potential_effect_instances, 655_360);
    assert_eq!(candidate.charged_effect_instances, 655_360);
    assert_eq!(candidate.retained_effect_instances, 524_288);
    assert_eq!(
        old_population.address_state - candidate.address_state,
        12_582_912
    );
    assert_eq!(old_population.temporary - candidate.temporary, 12_582_912);
    assert_eq!(
        old_population.bound.peak_storage_upper_bound()
            - candidate.bound.peak_storage_upper_bound(),
        12_582_912
    );
    assert_eq!(
        old_population.bound.work_upper_bound(),
        candidate.bound.work_upper_bound()
    );
    assert_eq!(old_population.effect_pairs, candidate.effect_pairs);
    assert_eq!(old_population.effect_state, candidate.effect_state);
    assert_eq!(
        old_population.retained_finding_count,
        candidate.retained_finding_count
    );
    assert_eq!(
        old_population.bound.retained_storage_upper_bound(),
        candidate.bound.retained_storage_upper_bound()
    );
    let work = candidate.bound.work_upper_bound();
    let peak = candidate.bound.peak_storage_upper_bound();
    assert_eq!(
        race_resource_upper_bound_for_shape_v1(
            actual,
            names,
            Some((32_768, 3)),
            None,
            ProductionAnalysisResourceLimitsV1::new(work, peak)
        ),
        Ok(candidate.bound),
    );
    for (work, peak, resource) in [
        (work - 1, peak, "work upper bound"),
        (work, peak - 1, "peak storage upper bound"),
    ] {
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                actual,
                names,
                Some((32_768, 3)),
                None,
                ProductionAnalysisResourceLimitsV1::new(work, peak)
            )
            .unwrap_err(),
            ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource,
            },
        );
    }
    eprintln!(
        "V5_EFFECT_GEOMETRY_ONLY old_peak={} new_peak={} work={} address_saving={}",
        old_population.bound.peak_storage_upper_bound(),
        peak,
        work,
        12_582_912
    );
}

#[path = "witness_rank_storage_v1_tests.rs"]
mod witness_rank_storage_v1_tests;

//! Inert retained-donor controls only. No genuine Rust-source admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
#[path = "whole_root_inert_fixtures_v1_tests.rs"]
mod fixture;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 23;

fn access(root: u64) -> ProjectedReadViewAccessV1 {
    ProjectedReadViewAccessV1 {
        view: ProjectedReadViewV1 {
            root,
            element: SemanticTypeIdV1::from_index(0),
            allocation: AllocationContractV1 {
                allocation_origin: 7,
                noalias_class: 1,
                writable: false,
                singleton_object: false,
            },
            rows: ProjectedReadValueV1::Constant(8),
            columns: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(1)),
        },
        row: ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(2)),
        column: ProjectedReadValueV1::Constant(3),
    }
}
fn inputs() -> (
    SemanticFunctionDeclV1,
    Vec<SemanticTypeDeclV1>,
    Vec<Option<ProjectedReadViewAccessV1>>,
    Vec<Option<u32>>,
) {
    (
        fixture::positive(),
        fixture::projection_types(),
        vec![Some(access(11)), Some(access(11))],
        vec![None, Some(3), Some(3), None],
    )
}
fn same_error(left: &Backend, right: &Backend) -> bool {
    match (left, right) {
        (
            Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(a)),
            Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(b)),
        ) => a == b,
        (Backend::Unsupported(a), Backend::Unsupported(b)) => a == b,
        (Backend::Incomplete(a), Backend::Incomplete(b)) => a == b,
        _ => false,
    }
}
fn compare_legacy(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    effects: &[Option<ProjectedReadViewAccessV1>],
    origins: &[Option<u32>],
    initial_layout: Option<&ProductionRankedOperationV1>,
) {
    let mut operations = initial_layout.into_iter().cloned().collect::<Vec<_>>();
    let mut next_value = 0;
    let mut arguments = vec![None; function.locals().len()];
    let mut next_argument = 1;
    let legacy = project_strided_read_effects_v1(
        types,
        function,
        effects,
        origins,
        &mut arguments,
        &mut next_argument,
        &mut operations,
        &mut next_value,
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut original = RetainedOriginalReadsV1::new();
    let retained = original.prepare_into(
        types,
        function,
        effects,
        origins,
        initial_layout,
        &mut Prep::new(&mut budget, &mut owned),
    );
    match (&legacy, &retained) {
        (Ok(projected), Ok(())) => {
            original
                .completed_for(
                    types,
                    function,
                    effects,
                    origins,
                    &Prep::new(&mut budget, &mut owned),
                )
                .unwrap();
            assert_eq!(&original.projected, projected);
            assert_eq!(original.operations, operations);
            assert_eq!(original.arguments, arguments);
            assert_eq!(original.next_value, next_value);
            assert_eq!(original.next_argument, next_argument);
            assert!(original.pending_view.is_none());
            assert!(original.displaced_view.is_none());
            assert_eq!(original.map_slots_debited, original.views.capacity());
        }
        (Err(left), Err(right)) => assert!(same_error(left, right)),
        _ => panic!("retained donor success/refusal differs from the legacy donor"),
    }
    assert_eq!(budget.storage(), FLOOR + owned);
    drop(original);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn retained_read_original_same_root_reuse_and_distinct_roots_match_legacy() {
    let (function, types, mut effects, origins) = inputs();
    compare_legacy(&function, &types, &effects, &origins, None);
    effects[1] = Some(access(12));
    compare_legacy(&function, &types, &effects, &origins, None);
}

#[test]
fn retained_read_original_empty_and_single_constant_rows_match_legacy() {
    let (function, types, mut effects, origins) = inputs();
    effects.fill(None);
    compare_legacy(&function, &types, &effects, &origins, None);
    let mut effect = access(11);
    effect.view.rows = ProjectedReadValueV1::Constant(1);
    effect.view.columns = ProjectedReadValueV1::Constant(1);
    effect.row = ProjectedReadValueV1::Constant(0);
    effect.column = ProjectedReadValueV1::Constant(0);
    effects[1] = Some(effect);
    compare_legacy(&function, &types, &effects, &origins, None);
}

#[test]
fn retained_read_original_execution_only_prefix_matches_legacy() {
    let (function, types, effects, origins) = inputs();
    let layout = ProductionRankedOperationV1::ExecutionLayout {
        grid_identity: 1,
        global_extents: [64, 1, 1],
        workgroup_extents: [64, 1, 1],
        subgroup_size: 64,
        full_physical_workgroups: true,
    };
    compare_legacy(&function, &types, &effects, &origins, Some(&layout));
}

#[test]
fn retained_read_original_exact_legacy_contract_and_origin_refusals() {
    for mode in 0..5 {
        let (function, types, mut effects, mut origins) = inputs();
        match mode {
            0 => effects[0].as_mut().unwrap().view.allocation.writable = true,
            1 => effects[1].as_mut().unwrap().view.columns = ProjectedReadValueV1::Constant(99),
            2 => origins[1] = Some(9000),
            3 => {
                effects.pop();
            }
            _ => effects[0].as_mut().unwrap().view.element = SemanticTypeIdV1::from_index(u32::MAX),
        }
        compare_legacy(&function, &types, &effects, &origins, None);
    }
}

fn trial(work_limit: usize, extra_storage: usize) -> (bool, usize, usize, [bool; 4]) {
    let (function, types, mut effects, origins) = inputs();
    effects[1] = Some(access(12));
    let mut work = Work::new(work_limit);
    let mut budget = Budget::new(&mut work, FLOOR + extra_storage);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut original = RetainedOriginalReadsV1::new();
    let result = original.prepare_into(
        &types,
        &function,
        &effects,
        &origins,
        None,
        &mut Prep::new(&mut budget, &mut owned),
    );
    let partials = [
        original
            .pending_view
            .as_ref()
            .is_some_and(|(_, v)| v.shape.capacity() > 0),
        original.operations.iter().any(|op| {
            matches!(op,
            ProductionRankedOperationV1::ViewInSpace { shape, dynamic_extents, .. }
            if shape.capacity() > 0 && dynamic_extents.len() < 2)
        }),
        original
            .projected
            .iter()
            .flatten()
            .any(|a| a.indices.capacity() > 0 && a.comparisons.len() < 2),
        original.views.capacity() > 0,
    ];
    assert_eq!(budget.storage(), FLOOR + owned);
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        original.next_value,
        original.next_argument,
        original.projected.len(),
        original.operations.len(),
        original.arguments.len(),
        original.views.len(),
        original.views.capacity(),
    );
    if result.is_err() {
        assert_eq!(original.phase, OriginalPhase::Terminal);
        assert!(same_error(
            result.as_ref().unwrap_err(),
            original.failure.as_ref().unwrap()
        ));
    }
    let repeated = original.prepare_into(
        &types,
        &function,
        &effects,
        &origins,
        None,
        &mut Prep::new(&mut budget, &mut owned),
    );
    assert!(repeated.is_err());
    assert_eq!(original.phase, OriginalPhase::Terminal);
    if let Err(error) = &result {
        assert!(same_error(error, repeated.as_ref().unwrap_err()));
    }
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            original.next_value,
            original.next_argument,
            original.projected.len(),
            original.operations.len(),
            original.arguments.len(),
            original.views.len(),
            original.views.capacity()
        )
    );
    assert!(
        original
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    let observation = (result.is_ok(), budget.work(), owned, partials);
    drop(original);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    observation
}

#[test]
fn retained_read_original_every_work_cut_keeps_partial_nested_owners() {
    let good = trial(LIMIT, LIMIT);
    assert!(good.0);
    let mut seen = [false; 4];
    for cut in 0..good.1 {
        let observed = trial(cut, LIMIT);
        assert!(!observed.0);
        for (slot, actual) in seen.iter_mut().zip(observed.3) {
            *slot |= actual;
        }
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn retained_read_original_every_storage_cut_keeps_partial_nested_owners() {
    let good = trial(LIMIT, LIMIT);
    assert!(good.0);
    let mut seen = [false; 4];
    for cut in 0..good.2 {
        let observed = trial(LIMIT, cut);
        assert!(!observed.0);
        for (slot, actual) in seen.iter_mut().zip(observed.3) {
            *slot |= actual;
        }
    }
    assert_eq!(seen, [true; 4]);
}

#[test]
fn retained_read_original_foreign_source_counter_ledger_and_floor_refuse() {
    let (function, types, effects, origins) = inputs();
    let copies = (
        function.clone(),
        types.clone(),
        effects.clone(),
        origins.clone(),
    );
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut original = RetainedOriginalReadsV1::new();
    original
        .prepare_into(
            &types,
            &function,
            &effects,
            &origins,
            None,
            &mut Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    for (t, f, e, o) in [
        (&types, &copies.0, &effects, &origins),
        (&copies.1, &function, &effects, &origins),
        (&types, &function, &copies.2, &origins),
        (&types, &function, &effects, &copies.3),
    ] {
        assert!(
            original
                .completed_for(t, f, e, o, &Prep::new(&mut budget, &mut owned))
                .is_err()
        );
    }
    let mut counter = owned;
    assert!(
        original
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &Prep::new(&mut budget, &mut counter)
            )
            .is_err()
    );
    let mut foreign_work = Work::new(LIMIT);
    let mut foreign_budget = Budget::new(&mut foreign_work, LIMIT);
    assert!(
        original
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &Prep::new(&mut foreign_budget, &mut owned)
            )
            .is_err()
    );
    budget.release_storage(1).unwrap();
    assert!(
        original
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    budget.reserve_storage(1).unwrap();
    original
        .completed_for(
            &types,
            &function,
            &effects,
            &origins,
            &Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    drop(original);
    budget.release_storage(owned).unwrap();
}

#[test]
fn retained_read_original_unwind_leaves_all_completed_owners_attached() {
    let (function, types, effects, origins) = inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut original = RetainedOriginalReadsV1::new();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        original
            .prepare_into(
                &types,
                &function,
                &effects,
                &origins,
                None,
                &mut Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        std::panic::panic_any(());
    }));
    assert!(caught.is_err());
    assert!(!original.views.is_empty());
    assert_eq!(original.projected.len(), 2);
    assert_eq!(budget.storage(), owned);
    // This controls an outer caller unwind, not the genuine canonical callback.
    original
        .completed_for(
            &types,
            &function,
            &effects,
            &origins,
            &Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    drop(original);
    budget.release_storage(owned).unwrap();
}

#[test]
fn retained_read_original_nonlayout_prefix_refuses_before_owned_clone() {
    let (function, types, effects, origins) = inputs();
    let invalid = ProductionRankedOperationV1::IndexConstant {
        result: ProductionRankedValueIdV1::new(9),
        value: 7,
    };
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut original = RetainedOriginalReadsV1::new();
    assert!(
        original
            .prepare_into(
                &types,
                &function,
                &effects,
                &origins,
                Some(&invalid),
                &mut Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert!(original.operations.is_empty());
    assert_eq!(original.operations.capacity(), 0);
    drop(original);
    budget.release_storage(owned).unwrap();
}

#[test]
fn retained_read_original_bad_source_lengths_refuse_before_payload_allocation() {
    let (function, types, effects, mut origins) = inputs();
    origins.pop();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut original = RetainedOriginalReadsV1::new();
    assert!(
        original
            .prepare_into(
                &types,
                &function,
                &effects,
                &origins,
                None,
                &mut Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    assert_eq!(original.arguments.capacity(), 0);
    assert_eq!(original.projected.capacity(), 0);
    assert_eq!(original.views.capacity(), 0);
    drop(original);
    budget.release_storage(owned).unwrap();
}

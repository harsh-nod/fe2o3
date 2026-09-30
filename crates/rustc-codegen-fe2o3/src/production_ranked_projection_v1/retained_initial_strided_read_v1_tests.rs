//! Inert ownership and donor-parity controls, not actual-source admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
#[path = "whole_root_inert_fixtures_v1_tests.rs"]
mod fixture;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 19;
fn access(root: u64) -> ProjectedReadViewAccessV1 {
    ProjectedReadViewAccessV1 {
        view: ProjectedReadViewV1 {
            root,
            element: SemanticTypeIdV1::from_index(0),
            allocation: AllocationContractV1 {
                allocation_origin: 7,
                noalias_class: 8,
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
fn initial(budget: &mut Budget<'_>, owned: &mut usize) -> RetainedBeforeArgumentWritersV1 {
    let mut result = RetainedBeforeArgumentWritersV1::new();
    result
        .prepare_into(4, &mut Prep::new(budget, owned))
        .unwrap();
    result
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
#[test]
fn retained_initial_reads_match_legacy_values_order_and_argument_reuse() {
    for roots in [(11, 11), (11, 12)] {
        let (function, types, mut effects, origins) = inputs();
        effects[1] = Some(access(roots.1));
        let mut original_ops = Vec::new();
        let mut original_next = 0;
        let mut original_arguments = vec![None; 4];
        let mut original_argument_next = 1;
        let expected = project_strided_read_effects_v1(
            &types,
            &function,
            &effects,
            &origins,
            &mut original_arguments,
            &mut original_argument_next,
            &mut original_ops,
            &mut original_next,
        )
        .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut arguments = initial(&mut budget, &mut owned);
        let mut prefix = RootEntryPrefixV1::empty();
        let mut stage = RetainedInitialStridedReadV1::new();
        stage
            .prepare_into(
                &types,
                &function,
                &effects,
                &origins,
                &mut arguments,
                &mut prefix,
                &mut Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        stage
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &arguments,
                &prefix,
                &Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        assert_eq!(stage.projected, expected);
        assert_eq!(prefix.entry_operations, original_ops);
        assert_eq!(prefix.next_value, original_next);
        assert_eq!(arguments.runtime_index_arguments, original_arguments);
        assert_eq!(arguments.next_runtime_argument, original_argument_next);
        assert_eq!(stage.lookup_visits, 1);
        assert_eq!(stage.views.len(), if roots.0 == roots.1 { 1 } else { 2 });
        assert_eq!(budget.storage(), owned);
        drop((stage, arguments, prefix));
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
fn trial(work_extra: usize, storage_extra: usize) -> (bool, usize, usize, bool, bool, bool) {
    let (function, types, effects, origins) = inputs();
    let initial_bytes = 8 * size_of::<Option<u32>>();
    let mut work = Work::new(16 + work_extra);
    let mut budget = Budget::new(&mut work, FLOOR + initial_bytes + storage_extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut arguments = initial(&mut budget, &mut owned);
    assert_eq!(owned, initial_bytes);
    assert_eq!(budget.work(), 16);
    let mut prefix = RootEntryPrefixV1::empty();
    let mut stage = RetainedInitialStridedReadV1::new();
    let result = stage.prepare_into(
        &types,
        &function,
        &effects,
        &origins,
        &mut arguments,
        &mut prefix,
        &mut Prep::new(&mut budget, &mut owned),
    );
    let partial_view = stage
        .pending_view
        .as_ref()
        .is_some_and(|v| v.view.shape.capacity() > 0);
    let partial_operation = prefix.entry_operations.iter().any(|op| {
        matches!(op,
        ProductionRankedOperationV1::ViewInSpace { shape, dynamic_extents, .. }
            if shape.capacity() > 0 && dynamic_extents.len() < 2)
    });
    let partial_access = stage
        .projected
        .iter()
        .flatten()
        .any(|a| a.indices.capacity() > 0 && a.comparisons.len() < 2);
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        prefix.next_value,
        arguments.next_runtime_argument,
        stage.projected.len(),
        stage.views.len(),
    );
    assert_eq!(budget.storage(), FLOOR + owned);
    if result.is_err() {
        assert_eq!(stage.phase, ReadPhase::Terminal);
        assert!(same_error(
            result.as_ref().unwrap_err(),
            stage.failure.as_ref().unwrap()
        ));
    }
    let repeated = stage.prepare_into(
        &types,
        &function,
        &effects,
        &origins,
        &mut arguments,
        &mut prefix,
        &mut Prep::new(&mut budget, &mut owned),
    );
    assert!(repeated.is_err());
    assert_eq!(stage.phase, ReadPhase::Terminal);
    if let Err(error) = &result {
        assert!(same_error(error, repeated.as_ref().unwrap_err()));
    }
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            prefix.next_value,
            arguments.next_runtime_argument,
            stage.projected.len(),
            stage.views.len()
        )
    );
    assert!(
        stage
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &arguments,
                &prefix,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    let outcome = (
        result.is_ok(),
        budget.work() - 16,
        owned - initial_bytes,
        partial_view,
        partial_operation,
        partial_access,
    );
    drop((stage, arguments, prefix));
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    outcome
}
#[test]
fn retained_initial_reads_every_work_cut_retains_each_partial_nested_owner() {
    let good = trial(LIMIT, LIMIT);
    assert!(good.0);
    let mut seen = [false; 3];
    for cut in 0..good.1 {
        let value = trial(cut, LIMIT);
        assert!(!value.0);
        seen[0] |= value.3;
        seen[1] |= value.4;
        seen[2] |= value.5;
    }
    assert_eq!(seen, [true; 3]);
}
#[test]
fn retained_initial_reads_every_storage_cut_retains_each_partial_nested_owner() {
    let good = trial(LIMIT, LIMIT);
    let mut seen = [false; 3];
    for cut in 0..good.2 {
        let value = trial(LIMIT, cut);
        assert!(!value.0);
        seen[0] |= value.3;
        seen[1] |= value.4;
        seen[2] |= value.5;
    }
    assert_eq!(seen, [true; 3]);
}
#[test]
fn retained_initial_reads_none_writable_changed_root_and_bad_origin_controls() {
    for mode in 0..5 {
        let (function, types, mut effects, mut origins) = inputs();
        match mode {
            0 => effects = vec![None, None],
            1 => effects[0].as_mut().unwrap().view.allocation.writable = true,
            2 => effects[1].as_mut().unwrap().view.columns = ProjectedReadValueV1::Constant(99),
            3 => origins[1] = Some(9000),
            _ => effects.pop().map(|_| ()).unwrap(),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut arguments = initial(&mut budget, &mut owned);
        let mut prefix = RootEntryPrefixV1::empty();
        let mut stage = RetainedInitialStridedReadV1::new();
        let result = stage.prepare_into(
            &types,
            &function,
            &effects,
            &origins,
            &mut arguments,
            &mut prefix,
            &mut Prep::new(&mut budget, &mut owned),
        );
        assert_eq!(result.is_ok(), mode == 0);
        if mode == 0 {
            assert_eq!(stage.projected, vec![None, None]);
            assert!(prefix.entry_operations.is_empty());
            assert_eq!(stage.lookup_visits, 0);
        }
        if mode == 2 {
            assert_eq!(stage.views.len(), 1);
            assert_eq!(stage.projected.len(), 1);
            assert_eq!(stage.lookup_visits, 1);
        }
        assert_eq!(budget.storage(), owned);
        drop((stage, arguments, prefix));
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_initial_reads_foreign_source_counter_ledger_and_floor_refuse() {
    let (function, types, effects, origins) = inputs();
    let copied = function.clone();
    let copied_types = types.clone();
    let copied_effects = effects.clone();
    let copied_origins = origins.clone();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut arguments = initial(&mut budget, &mut owned);
    let mut prefix = RootEntryPrefixV1::empty();
    let mut stage = RetainedInitialStridedReadV1::new();
    stage
        .prepare_into(
            &types,
            &function,
            &effects,
            &origins,
            &mut arguments,
            &mut prefix,
            &mut Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    for (t, f, e, o) in [
        (&types, &copied, &effects, &origins),
        (&copied_types, &function, &effects, &origins),
        (&types, &function, &copied_effects, &origins),
        (&types, &function, &effects, &copied_origins),
    ] {
        assert!(
            stage
                .completed_for(
                    t,
                    f,
                    e,
                    o,
                    &arguments,
                    &prefix,
                    &Prep::new(&mut budget, &mut owned)
                )
                .is_err()
        );
    }
    let mut other = owned;
    assert!(
        stage
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &arguments,
                &prefix,
                &Prep::new(&mut budget, &mut other)
            )
            .is_err()
    );
    let mut foreign_work = Work::new(LIMIT);
    let mut foreign_budget = Budget::new(&mut foreign_work, LIMIT);
    assert!(
        stage
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &arguments,
                &prefix,
                &Prep::new(&mut foreign_budget, &mut owned)
            )
            .is_err()
    );
    budget.release_storage(1).unwrap();
    assert!(
        stage
            .completed_for(
                &types,
                &function,
                &effects,
                &origins,
                &arguments,
                &prefix,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    );
    budget.reserve_storage(1).unwrap();
    stage
        .completed_for(
            &types,
            &function,
            &effects,
            &origins,
            &arguments,
            &prefix,
            &Prep::new(&mut budget, &mut owned),
        )
        .unwrap();
    drop((stage, arguments, prefix));
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_reads_scalar_helper_rejects_argument_and_ssa_overflow() {
    // Scalar helper controls intentionally bypass the source-bound bridge.
    let origins = [None; 4];
    for mode in 0..2 {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut arguments = initial(&mut budget, &mut owned);
        let mut prefix = RootEntryPrefixV1::empty();
        let value = if mode == 0 {
            prefix.next_value = u32::MAX;
            ProjectedReadValueV1::Constant(1)
        } else {
            arguments.next_runtime_argument = usize::MAX;
            ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(1))
        };
        assert!(
            read_value(
                value,
                &origins,
                &mut arguments,
                &mut prefix,
                &mut Prep::new(&mut budget, &mut owned)
            )
            .is_err()
        );
        assert!(prefix.entry_operations.is_empty());
        assert!(
            arguments
                .runtime_index_arguments
                .iter()
                .all(Option::is_none)
        );
        drop((arguments, prefix));
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_initial_reads_unwind_keeps_outer_payloads_for_postflight() {
    let (function, types, effects, origins) = inputs();
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut arguments = initial(&mut budget, &mut owned);
    let mut prefix = RootEntryPrefixV1::empty();
    let mut stage = RetainedInitialStridedReadV1::new();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        stage
            .prepare_into(
                &types,
                &function,
                &effects,
                &origins,
                &mut arguments,
                &mut prefix,
                &mut Prep::new(&mut budget, &mut owned),
            )
            .unwrap();
        std::panic::panic_any(());
    }));
    assert!(caught.is_err());
    assert_eq!(stage.projected.len(), 2);
    assert_eq!(budget.storage(), owned);
    // Callback unwind is external to this completed component. A real whole
    // owner/factory must retain it through its own unwind/postflight; this test
    // does not pretend that the component can observe an arbitrary caller panic.
    drop((stage, arguments, prefix));
    budget.release_storage(owned).unwrap();
}

#[test]
fn retained_initial_reads_whole_phase_transition_revokes_old_completion_once() {
    for start in [
        WholePhase::Fresh,
        WholePhase::Terminal,
        WholePhase::BeforeArgumentWriters,
        WholePhase::InitialStridedReadsTerminal,
        WholePhase::AfterInitialStridedReads,
    ] {
        let mut phase = start;
        assert_eq!(
            whole::begin_initial_read_transition(&mut phase),
            start == WholePhase::BeforeArgumentWriters
        );
        assert_eq!(phase, WholePhase::InitialStridedReadsTerminal);
        assert!(!whole::begin_initial_read_transition(&mut phase));
        assert_eq!(phase, WholePhase::InitialStridedReadsTerminal);
    }
}

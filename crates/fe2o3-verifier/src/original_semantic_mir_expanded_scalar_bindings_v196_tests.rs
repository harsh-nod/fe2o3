use super::super::super::source_function::tile_fixture_tests::run_fixture;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionTileLayoutV1 as Layout,
};
use fe2o3_lower_mir_kernel::ProductionOptimizedSourceOperationV18 as SourceOperation;

const LIMIT: usize = 100_000_000;

fn exercise(slots: &SourceSlots<'_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    let target = TileTargetV176::derive(slots, out)?;
    let before = out.budget.storage();
    let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
    // Independent retained-owner and query scratch census, not measured replay.
    let header = size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>()
        + 2 * size_of::<Result<ExpandedScalarBindingsV196<'_, '_, '_, '_>>>()
        + 3 * size_of::<Definition>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>()
        + 24 * size_of::<usize>();
    assert_eq!(out.budget.storage() - before, header);
    let original = slots.correspondence(out)?.inventory(out.budget)?;
    let actual = target.inventory(out)?;
    assert_eq!(
        pairs.definition_counts(out)?,
        (original.definitions().len(), actual.definitions().len())
    );
    let tile = slots.tile_owner_v176(out)?;
    let neutral = tile.neutral_source_v162(out.budget)?;
    let mut matched = 0;
    let mut shifted = 0;
    let mut refused_roles = 0;
    for (index, row) in original.definitions().iter().enumerate() {
        if !matches!(
            row.ty,
            Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_)
        ) {
            assert!(matches!(
                pairs.definition(index, out),
                Err(Error::Statement(
                    "expanded scalar binding requires a scalar, pointer, slice or unit endpoint"
                ))
            ));
            refused_roles += 1;
            continue;
        }
        let descendants = neutral.definition_descendants(row.coordinate, out.budget)?;
        let [descendant] = descendants else {
            assert!(pairs.definition(index, out).is_err());
            continue;
        };
        if descendant.kind != Descendant::Retained {
            assert!(pairs.definition(index, out).is_err());
            continue;
        }
        let expected = match row.coordinate {
            Definition::Result { operation, result } => {
                if !matches!(
                    neutral.operation(operation, out.budget)?,
                    SourceOperation::Retained { .. }
                ) {
                    assert!(pairs.definition(index, out).is_err());
                    continue;
                }
                let span = tile
                    .operation_span(operation, out.budget)?
                    .unwrap()
                    .expansion;
                if span.end.checked_sub(span.first) != Some(1) {
                    assert!(pairs.definition(index, out).is_err());
                    continue;
                }
                Definition::Result {
                    operation: Operation {
                        block: span.input.block,
                        operation: span.first,
                    },
                    result,
                }
            }
            _ => descendant.output,
        };
        let expected = actual
            .definitions()
            .iter()
            .position(|row| row.coordinate == expected)
            .unwrap();
        let resolved = pairs.definition(index, out)?;
        assert_eq!(resolved, expected);
        assert_eq!(actual.definitions()[resolved].ty, row.ty);
        matched += 1;
        shifted += usize::from(index != resolved);
    }
    assert!(matched > 0);
    assert!(
        shifted > 0,
        "fixture must exercise changed dense SSA indices"
    );
    assert!(refused_roles > 0);
    assert!(pairs.definition(original.definitions().len(), out).is_err());
    assert!(pairs.definition(usize::MAX, out).is_err());
    Ok(())
}

#[test]
fn expanded_scalar_bindings_join_actual_definitions_in_both_tile_layouts() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(layout, LIMIT, LIMIT, |slots, _, out| exercise(slots, out))
            .0
            .unwrap();
    }
}

#[test]
fn expanded_scalar_bindings_have_exact_and_one_short_full_resource_bounds() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            run_fixture(layout, work, storage, |slots, _, out| exercise(slots, out))
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        run(measured.1, measured.3).0.unwrap();
        assert!(run(measured.1 - 1, measured.3).0.is_err());
        assert!(run(measured.1, measured.3 - 1).0.is_err());
    }
}

#[test]
fn expanded_scalar_bindings_retain_foreign_and_refunded_account_refusals() {
    for foreign in [false, true] {
        let mut reached = false;
        let result = run_fixture(Layout::Blocked, LIMIT, LIMIT, |slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            reached = true;
            let error = if foreign {
                let mut work = Work::new(LIMIT);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget.reserve_storage(out.budget.storage())?;
                let mut writer = Writer::new(&mut budget)?;
                pairs.definition_counts(&mut writer).unwrap_err()
            } else {
                out.budget.release_storage(1)?;
                pairs.definition_counts(out).unwrap_err()
            };
            assert!(pairs.definition_counts(out).is_err());
            assert!(pairs.definition(0, out).is_err());
            Err(error)
        });
        assert!(reached);
        assert!(result.0.is_err());
    }
}

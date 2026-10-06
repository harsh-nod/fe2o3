use super::super::super::source_function::tile_fixture_tests::run_fixture;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionTileLayoutV1 as Layout,
};
use fe2o3_lower_mir_kernel::{
    ProductionOptimizedSourceOperationV18 as SourceOperation,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};

const LIMIT: usize = 100_000_000;

fn refusal(result: Result<usize>) -> Result<()> {
    match result {
        Err(Error::Statement(_) | Error::Source(SourceError::Binding(_))) => Ok(()),
        Err(error) => Err(error),
        Ok(_) => panic!("unsupported scalar endpoint was admitted"),
    }
}

fn exercise(slots: &SourceSlots<'_, '_>, out: &mut Writer<'_, '_>) -> Result<()> {
    let target = TileTargetV176::derive(slots, out)?;
    let before = out.budget.storage();
    let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
    // Independent retained-owner and query scratch census, not measured replay.
    let retained_owners = 2 * size_of::<&()>();
    let retained_counts_and_floor = 3 * size_of::<usize>();
    let retained = retained_owners + retained_counts_and_floor;
    assert_eq!(
        size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>(),
        retained
    );
    let construction_and_query_results =
        2 * size_of::<Result<ExpandedScalarBindingsV196<'_, '_, '_, '_>>>();
    let input_predecessor_and_actual_coordinates = 3 * size_of::<Definition>();
    let expansion_span = size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>();
    let tile_projection = size_of::<TileLeaf>();
    let tile_results = 2 * size_of::<Result<Option<usize>>>();
    let bounded_query_scratch = 24 * size_of::<usize>();
    let header = retained
        + construction_and_query_results
        + input_predecessor_and_actual_coordinates
        + expansion_span
        + tile_projection
        + tile_results
        + bounded_query_scratch;
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
            match pairs.definition(index, out) {
                Err(Error::Statement(
                    "expanded scalar binding requires a scalar, pointer, slice or unit endpoint",
                )) => {}
                Err(error) => return Err(error),
                Ok(_) => panic!("execution role was admitted as a scalar endpoint"),
            }
            refused_roles += 1;
            continue;
        }
        let descendants = neutral.definition_descendants(row.coordinate, out.budget)?;
        let [descendant] = descendants else {
            refusal(pairs.definition(index, out))?;
            continue;
        };
        if descendant.kind != Descendant::Retained {
            refusal(pairs.definition(index, out))?;
            continue;
        }
        let expected = match row.coordinate {
            Definition::Result { operation, result } => {
                if !matches!(
                    neutral.operation(operation, out.budget)?,
                    SourceOperation::Retained { .. }
                ) {
                    refusal(pairs.definition(index, out))?;
                    continue;
                }
                let span = tile
                    .operation_span(operation, out.budget)?
                    .unwrap()
                    .expansion;
                if span.end.checked_sub(span.first) != Some(1) {
                    refusal(pairs.definition(index, out))?;
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
    refusal(pairs.definition(original.definitions().len(), out))?;
    refusal(pairs.definition(usize::MAX, out))?;
    exercise_tile_leaves(&pairs, out)?;
    Ok(())
}

fn exercise_tile_leaves(
    pairs: &ExpandedScalarBindingsV196<'_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use fe2o3_kernel_ir::ExecutionRoleV15 as Role;
    let original = pairs.slots.correspondence(out)?.inventory(out.budget)?;
    let actual = pairs.target.inventory(out)?;
    let mut leaves = 0;
    for (index, row) in original.definitions().iter().enumerate() {
        let Type::Execution(
            Role::MaskedTileU32 { elements, .. } | Role::LaneFragmentU32 { elements, .. },
        ) = row.ty
        else {
            continue;
        };
        if !matches!(row.coordinate, Definition::Result { .. }) {
            continue;
        }
        for field in [0, 1] {
            for element in 0..u32::from(*elements) {
                let path = [field, element];
                let TileLeaf::Scalar {
                    function,
                    value,
                    scalar,
                } = pairs.slots.tile_leaf_v162(row.coordinate, &path, out)?
                else {
                    panic!("value and mask fields must select scalar leaves");
                };
                let found = pairs.tile_leaf(index, &path, out)?.unwrap();
                let coordinate = actual.definitions()[found].coordinate;
                let owner = match coordinate {
                    Definition::FunctionArgument { function, .. } => function,
                    Definition::BlockArgument { block, .. } => block.function,
                    Definition::Result { operation, .. } => operation.block.function,
                };
                assert_eq!(owner, function);
                assert_eq!(actual.definitions()[found].value, Some(value));
                assert_eq!(actual.definitions()[found].ty, &Type::Scalar(scalar));
                assert_eq!(
                    actual
                        .definitions()
                        .iter()
                        .filter(|row| row.coordinate == coordinate)
                        .count(),
                    1
                );
                leaves += 1;
            }
        }
        assert_eq!(pairs.tile_leaf(index, &[2], out)?, None);
        assert_eq!(pairs.tile_leaf(index, &[3], out)?, None);
        for path in [&[][..], &[0][..], &[4][..], &[0, u32::from(*elements)][..]] {
            match pairs.tile_leaf(index, path, out) {
                Err(Error::Statement(_) | Error::Source(SourceError::Binding(_))) => {}
                Err(error) => return Err(error),
                Ok(_) => panic!("invalid tile field path was admitted"),
            }
        }
    }
    assert!(
        leaves > 0,
        "fixture must exercise actual scalarized tile leaves"
    );
    Ok(())
}

#[test]
fn expanded_scalar_bindings_resolve_value_mask_and_unit_tile_leaves() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(layout, LIMIT, LIMIT, |slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            exercise_tile_leaves(&pairs, out)
        })
        .0
        .unwrap();
    }
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
        assert!(matches!(run(measured.1 - 1, measured.3).0,
            Err(Error::Resource(Resource::Work(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
        assert!(matches!(run(measured.1, measured.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
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
            assert!(matches!(
                error,
                Error::Resource(Resource::Accounting)
                    | Error::Source(SourceError::Resource(Resource::Accounting))
            ));
            assert!(pairs.definition_counts(out).is_err());
            assert!(pairs.definition(0, out).is_err());
            Err(error)
        });
        assert!(reached);
        assert!(result.0.is_err());
    }
}

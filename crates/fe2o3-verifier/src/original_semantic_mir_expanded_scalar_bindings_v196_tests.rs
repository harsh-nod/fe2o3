use super::super::super::source_function::tile_fixture_tests::run_fixture;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    ExecutionTileLayoutV1 as Layout, FormalIndexWidth,
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
    let relation_scratch = 2 * size_of::<Option<usize>>()
        + 3 * size_of::<usize>()
        + size_of::<FormalIndexWidth>()
        + 2 * size_of::<Result<()>>()
        + size_of::<&Type>();
    type Endpoint<'a, 'b> = fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'a, 'b>;
    let source_relation_scratch = size_of::<Endpoint<'_, '_>>()
        + 2 * size_of::<Result<Endpoint<'_, '_>>>()
        + size_of::<Option<(usize, u32)>>()
        + size_of::<Result<()>>()
        + 12 * size_of::<usize>()
        + super::super::logical::headers();
    let bounded_query_scratch = 24 * size_of::<usize>();
    let header = retained
        + construction_and_query_results
        + input_predecessor_and_actual_coordinates
        + expansion_span
        + tile_projection
        + tile_results
        + relation_scratch
        + source_relation_scratch
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
        let start = out.text.len();
        pairs.emit_definition_relation(index, FormalIndexWidth::Bits64, out)?;
        let emitted = &out.text[start..];
        assert!(emitted.starts_with(&format!(
            "(target.values.len() == {} && ({{ ",
            actual.definitions().len()
        )));
        assert!(emitted.contains(&format!("let actual = target.values[{resolved}];")));
        assert!(emitted.contains("invocation_value_related_v36(original, actual, map, source.machine.memory, target.memory)"));
        if index != resolved {
            assert!(!emitted.contains(&format!("target.values[{index}]")));
        }
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
    exercise_source_arguments(&pairs, out)?;
    Ok(())
}

fn exercise_source_arguments(
    pairs: &ExpandedScalarBindingsV196<'_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::super::InvocationPlan;
    let relation = pairs.slots.correspondence(out)?;
    let source = relation.source(out.budget)?;
    let plan = InvocationPlan::derive(source, out)?;
    let archive = source.source_ssa(out.budget)?;
    let mut bound = 0;
    for root in 0..source.root_count(out.budget)? {
        let row = plan.instance(root, 0, out)?;
        let ssa = archive.plan_for_function(row.function).unwrap().plan();
        for entry in ssa.entry_definitions() {
            let endpoint = relation.ssa_typed_endpoint_v36(root, 0, entry.value(), out.budget)?;
            if !matches!(
                endpoint.physical_type(out.budget)?,
                Some(Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_))
            ) {
                continue;
            }
            let original = endpoint.original_definition(out.budget)?.unwrap();
            let actual = pairs.definition(original, out)?;
            let local = row.locals.start + endpoint.source_local(out.budget)?.index() as usize;
            let start = out.text.len();
            match pairs.emit_source_conjunct(
                &plan,
                root,
                0,
                entry.value(),
                FormalIndexWidth::Unknown,
                out,
            ) {
                Err(Error::Statement(
                    "expanded scalar binding differs from its retained source endpoint",
                )) => (),
                Err(error) => return Err(error),
                Ok(()) => panic!("unknown-width source relation was admitted"),
            }
            assert_eq!(out.text.len(), start);
            pairs.emit_source_conjunct(
                &plan,
                root,
                0,
                entry.value(),
                FormalIndexWidth::Bits64,
                out,
            )?;
            let text = &out.text[start..];
            assert!(text.starts_with(" && (source.machine.valid && target.valid && "));
            assert!(text.contains(&format!("let original = source.machine.values[{local}];")));
            assert!(text.contains(&format!(
                "source.machine.frames.active[0].owner == {}",
                row.function.index()
            )));
            assert!(text.contains(&format!("let actual = target.values[{actual}];")));
            assert!(text.contains("invocation_value_related_v36(original, actual, map, source.machine.memory, target.memory)"));
            bound += 1;
        }
    }
    assert!(
        bound > 0,
        "fixture must exercise source-backed root arguments"
    );
    Ok(())
}

#[test]
fn expanded_source_scalar_relations_bind_authentic_original_arguments() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(layout, LIMIT, LIMIT, |slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            exercise_source_arguments(&pairs, out)
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_and_expanded_scalar_relations_share_exact_logical_obligations() {
    use super::super::LogicalBinding;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut out = Writer::new(&mut budget).unwrap();
    LogicalBinding::Plain.emit_current(2, &mut out).unwrap();
    assert!(out.text.is_empty());
    LogicalBinding::Witness { source_type: 7 }
        .emit_current(2, &mut out)
        .unwrap();
    assert_eq!(
        out.text,
        " && invocation_source_witness_current_v38(source, 2, 7)"
    );
    out.text.clear();
    LogicalBinding::Reference {
        source_type: 7,
        origin: 3,
        generation: 5,
        instance: 1,
        block: 8,
        statement: 4,
    }
    .emit_current(2, &mut out)
    .unwrap();
    assert_eq!(
        out.text,
        " && invocation_source_reference_current_v38(source, 2, 7) && ({ let reference = source.logical.references[2]; reference.origin == 3 && reference.origin_generation == 5 && reference.borrow_instance == 1 && reference.borrow_block == 8 && reference.borrow_statement == 4 })"
    );
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
                let start = out.text.len();
                pairs.emit_tile_leaf_relation(index, &path, FormalIndexWidth::Bits64, out)?;
                let emitted = &out.text[start..];
                assert!(emitted.contains(&format!("let actual = target.values[{found}];")));
                assert!(emitted.contains(if field == 0 {
                    "byte_scalar_type_v57(actual, memory_value_modulus_v30(4))"
                } else {
                    "byte_scalar_type_v57(actual, 2)"
                }));
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
        for path in [&[2][..], &[3][..]] {
            let start = out.text.len();
            pairs.emit_tile_leaf_relation(index, path, FormalIndexWidth::Bits64, out)?;
            assert_eq!(
                &out.text[start..],
                format!(
                    "(target.values.len() == {} && ({{ original == MemoryValueV30::Unit }}))",
                    actual.definitions().len()
                )
            );
        }
    }
    assert!(
        leaves > 0,
        "fixture must exercise actual scalarized tile leaves"
    );
    Ok(())
}

#[test]
fn expanded_value_relations_require_exact_target_owner_and_known_width() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(layout, LIMIT, LIMIT, |slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let distinct_target = TileTargetV176::derive(slots, out)?;
            let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            pairs.check_owner(slots, &target, out)?;
            assert!(matches!(
                pairs.check_owner(slots, &distinct_target, out),
                Err(Error::Statement(
                    "expanded scalar binding differs from its retained source endpoint"
                ))
            ));
            let inventory = slots.correspondence(out)?.inventory(out.budget)?;
            let index = inventory
                .definitions()
                .iter()
                .position(|row| {
                    matches!(row.coordinate, Definition::FunctionArgument { .. })
                        && matches!(row.ty, Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_))
                })
                .expect("fixture must retain a scalar-compatible root argument");
            let start = out.text.len();
            assert!(matches!(
                pairs.emit_definition_relation(index, FormalIndexWidth::Unknown, out),
                Err(Error::Statement(
                    "expanded value relation requires a known INDEX width"
                ))
            ));
            assert_eq!(out.text.len(), start);
            for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
                pairs.emit_definition_relation(index, width, out)?;
            }
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn expanded_scalar_bindings_keep_invalid_tile_path_refusals_sticky() {
    for layout in [Layout::Blocked, Layout::Striped] {
        for path in [&[][..], &[0][..], &[4][..], &[0, u32::MAX][..], &[2, 0][..]] {
            let result = run_fixture(layout, LIMIT, LIMIT, |slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
                let original = slots.correspondence(out)?.inventory(out.budget)?;
                let index = original
                    .definitions()
                    .iter()
                    .position(|row| {
                        matches!(
                            row.ty,
                            Type::Execution(
                                fe2o3_kernel_ir::ExecutionRoleV15::MaskedTileU32 { .. }
                                    | fe2o3_kernel_ir::ExecutionRoleV15::LaneFragmentU32 { .. }
                            )
                        ) && matches!(row.coordinate, Definition::Result { .. })
                    })
                    .expect("fixture must retain a tile definition");
                let error = pairs.tile_leaf(index, path, out).unwrap_err();
                assert!(matches!(
                    error,
                    Error::Source(SourceError::Binding("source tile leaf field path differs"))
                ));
                assert!(matches!(
                    pairs.definition_counts(out),
                    Err(Error::Source(SourceError::Binding(
                        "source tile leaf field path differs"
                    )))
                ));
                Err(error)
            })
            .0;
            assert!(matches!(
                result,
                Err(Error::Source(SourceError::Binding(
                    "source tile leaf field path differs"
                )))
            ));
        }
    }
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

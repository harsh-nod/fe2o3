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
        Err(
            Error::Statement(_)
            | Error::Source(SourceError::Binding(_))
            | Error::SourceReconstruction { .. },
        ) => Ok(()),
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
    let refusal_coordinates = 3 * size_of::<Error>()
        + size_of::<[usize; 4]>()
        + size_of::<fe2o3_mir_model::SsaValueV1>()
        + size_of::<Option<usize>>()
        + 3 * size_of::<&'static str>()
        + 8 * size_of::<&()>();
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
    let checked_replacement_scratch = size_of::<Definition>()
        + size_of::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()
        + size_of::<fe2o3_kernel_ir::ValueId>()
        + size_of::<Option<fe2o3_kernel_ir::ValueId>>()
        + 2 * size_of::<Result<usize>>()
        + size_of::<Result<Option<usize>>>()
        + 4 * size_of::<usize>()
        + 12 * size_of::<&()>();
    let source_leaf_scratch = 2 * size_of::<Endpoint<'_, '_>>()
        + 2 * size_of::<Result<Endpoint<'_, '_>>>()
        + size_of::<super::super::super::slots::SourceAggregateLeafV42<'_, '_, '_>>()
        + 2 * size_of::<Result<Option<usize>>>()
        + size_of::<Result<()>>()
        + 14 * size_of::<usize>()
        + size_of::<Option<usize>>();
    fn product_frame<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let source_product_scratch = 2 * product_frame::<Endpoint<'_, '_>>()
        + product_frame::<super::super::super::slots::SourceProductComponentV282<'_, '_, '_>>()
        + product_frame::<super::super::super::slots::ProductAtomV282>()
        + product_frame::<fe2o3_lower_mir_kernel::ProductionSourceSsaCarrierShapeV37>()
        + product_frame::<Option<usize>>()
        + product_frame::<&[u32]>()
        + product_frame::<std::slice::Iter<'_, u32>>()
        + product_frame::<super::super::Value>()
        + product_frame::<FormalIndexWidth>()
        + product_frame::<fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1>()
        + product_frame::<fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1>()
        + product_frame::<Option<&Type>>()
        + product_frame::<()>()
        + 18 * size_of::<usize>()
        + 20 * size_of::<&()>();
    type ForwardingOwner<'a, 'b, 'c, 'd> = ExpandedScalarBindingsV196<'a, 'b, 'c, 'd>;
    let forwarding_wrapper =
        size_of::<(&ForwardingOwner<'_, '_, '_, '_>, &mut Writer<'_, '_>, usize)>();
    let forwarding_callback = size_of::<(
        &ForwardingOwner<'_, '_, '_, '_>,
        &usize,
        &mut Writer<'_, '_>,
    )>();
    let forwarding_inventory_tile_neutral = 3 * size_of::<&()>();
    let forwarding_source_current_function = 3 * size_of::<&()>();
    let forwarding_edge_and_predecessor = 2 * size_of::<&()>();
    let forwarding_descendants =
        size_of::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>();
    let forwarding_edges = size_of::<&[fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1]>();
    let forwarding_indices = 4 * size_of::<usize>() + size_of::<Option<usize>>();
    let forwarding_coordinates =
        size_of::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>() + size_of::<Definition>();
    let forwarding_iteration = size_of::<std::ops::Range<usize>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>>();
    let forwarding_results = 2 * size_of::<Result<usize>>();
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1 as RebuildBlock,
        CanonicalKirFunctionCoordinateV1 as RebuildFunction,
    };
    use reconstruction::Recipe;
    let reconstruction_vectors = size_of::<Vec<Option<Recipe>>>()
        + size_of::<Vec<u8>>()
        + size_of::<Vec<(usize, u8)>>()
        + size_of::<Vec<usize>>();
    let reconstruction_allocation_results = 2 * size_of::<Result<Vec<Option<Recipe>>>>()
        + 2 * size_of::<Result<Vec<u8>>>()
        + 2 * size_of::<Result<Vec<(usize, u8)>>>()
        + 2 * size_of::<Result<Vec<usize>>>();
    let reconstruction_query_frames = 4 * size_of::<Recipe>()
        + 4 * size_of::<Result<Recipe>>()
        + 4 * size_of::<Option<usize>>()
        + 4 * size_of::<Option<RebuildBlock>>()
        + 4 * size_of::<RebuildBlock>()
        + 3 * size_of::<RebuildFunction>()
        + 3 * size_of::<Definition>()
        + 3 * size_of::<std::ops::Range<usize>>()
        + size_of::<[Option<(RebuildBlock, usize)>; 2]>();
    let reconstruction_wrapper_borrows = 6 * size_of::<&()>();
    let reconstruction_recipe_borrows = 10 * size_of::<&()>();
    let reconstruction_phi_borrows = 9 * size_of::<&()>();
    let reconstruction_operand_borrows = 4 * size_of::<&()>();
    let reconstruction_traversal_borrows =
        size_of::<&mut [Option<Recipe>]>() + size_of::<&mut [u8]>() + 4 * size_of::<&()>();
    let reconstruction_loader_frame = (5 + 2) * size_of::<&()>()
        + size_of::<usize>()
        + size_of::<Recipe>()
        + size_of::<Result<Recipe>>();
    let reconstruction_pure_result_frame = 8 * size_of::<&()>()
        + 4 * size_of::<usize>()
        + size_of::<Definition>()
        + size_of::<fe2o3_kernel_ir::Constant>()
        + size_of::<bool>()
        + size_of::<Option<usize>>()
        + 2 * size_of::<Result<Recipe>>()
        + 2 * size_of::<Result<usize>>();
    let reconstruction_arm_iteration = size_of::<std::array::IntoIter<RebuildBlock, 2>>();
    let reconstruction_snapshot_path = size_of::<[u32; 2]>() + size_of::<&[u32]>();
    let reconstruction_slice_borrows =
        size_of::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>()
            + size_of::<&[fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1]>()
            + size_of::<&[fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>]>();
    let reconstruction_indices = (14 + 8 + 8 + 2) * size_of::<usize>();
    let reconstruction_values = 2 * size_of::<Type>()
        + 2 * size_of::<fe2o3_kernel_ir::ScalarType>()
        + 2 * size_of::<fe2o3_kernel_ir::ValueId>()
        + size_of::<FormalIndexWidth>()
        + size_of::<[bool; 2]>()
        + 2 * size_of::<bool>()
        + size_of::<u8>()
        + size_of::<u64>()
        + 3 * size_of::<Result<()>>();
    let reconstruction_iterators = 3 * size_of::<
        std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'_>>,
    >() + size_of::<
        std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirEdgeArgumentRefV1>,
    >() + size_of::<std::slice::Iter<'_, usize>>();
    #[allow(dead_code)]
    #[repr(C)]
    struct RefusalFactsFields {
        phase: &'static str,
        original: usize,
        coordinate: Definition,
        type_class: &'static str,
        scalar: Option<fe2o3_kernel_ir::ScalarType>,
        descendant_count: Option<usize>,
        first_descendant: Option<fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
        target_function: Option<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>,
        target_range: Option<(usize, usize)>,
        target_index: Option<usize>,
        binary: Option<fe2o3_kernel_ir::BinaryOp>,
    }
    let refusal_trace_headers = 6 * size_of::<RefusalFactsFields>()
        + 2 * size_of::<Error>()
        + 16 * size_of::<usize>()
        + 4 * size_of::<&()>()
        + 2 * size_of::<u8>();
    let header = retained
        + refusal_coordinates
        + refusal_trace_headers
        + construction_and_query_results
        + input_predecessor_and_actual_coordinates
        + expansion_span
        + tile_projection
        + tile_results
        + relation_scratch
        + source_relation_scratch
        + source_leaf_scratch
        + source_product_scratch
        + checked_replacement_scratch
        + forwarding_wrapper
        + forwarding_callback
        + forwarding_inventory_tile_neutral
        + forwarding_source_current_function
        + forwarding_edge_and_predecessor
        + forwarding_descendants
        + forwarding_edges
        + forwarding_indices
        + forwarding_coordinates
        + forwarding_iteration
        + forwarding_results
        + reconstruction_vectors
        + reconstruction_allocation_results
        + reconstruction_query_frames
        + reconstruction_wrapper_borrows
        + reconstruction_recipe_borrows
        + reconstruction_phi_borrows
        + reconstruction_operand_borrows
        + reconstruction_traversal_borrows
        + reconstruction_loader_frame
        + reconstruction_pure_result_frame
        + reconstruction_arm_iteration
        + reconstruction_snapshot_path
        + reconstruction_slice_borrows
        + reconstruction_indices
        + reconstruction_values
        + reconstruction_iterators
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
    exercise_source_leaves(&pairs, out)?;
    Ok(())
}

fn exercise_source_leaves(
    pairs: &ExpandedScalarBindingsV196<'_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use super::super::InvocationPlan;
    use fe2o3_kernel_ir::ExecutionRoleV15;
    use fe2o3_mir_model::{
        SsaBlockIdV1 as Block, SsaEdgeIdV1 as Edge, SsaResolvedEventV1 as Event,
    };

    let source = pairs.slots.correspondence(out)?.source(out.budget)?;
    let plan = InvocationPlan::derive(source, out)?;
    let archive = source.source_ssa(out.budget)?;
    let semantic = source.source_semantic(out.budget)?;
    let mut counts = [0usize; 3];
    for root in 0..source.root_count(out.budget)? {
        for instance in 0..plan.root(root, out)?.instances.len() {
            let row = plan.instance(root, instance, out)?;
            if !row.active {
                continue;
            }
            let ssa = archive.plan_for_function(row.function).unwrap().plan();
            let function = &semantic.functions()[row.function.index() as usize];
            let mut values = std::collections::BTreeSet::new();
            values.extend(ssa.entry_definitions().iter().map(|entry| entry.value()));
            for (block, declaration) in function.blocks().iter().enumerate() {
                let block = Block::new(block as u32);
                for (_, event) in ssa.resolved_events(block).into_iter().flatten() {
                    if let Event::Define { value, .. } = event {
                        values.insert(*value);
                    }
                }
                for edge in 0..declaration.terminator().kind().edge_count() {
                    for entry in ssa
                        .edge_definitions(Edge::new(block, edge as u32))
                        .into_iter()
                        .flatten()
                    {
                        values.insert(entry.value());
                    }
                }
            }
            for value in values {
                let endpoint = pairs
                    .slots
                    .correspondence(out)?
                    .ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
                if endpoint.carrier_shape(out.budget)?
                    != fe2o3_lower_mir_kernel::ProductionSourceSsaCarrierShapeV37::Value
                {
                    continue;
                }
                let Some(Type::Execution(
                    ExecutionRoleV15::MaskedTileU32 { .. }
                    | ExecutionRoleV15::LaneFragmentU32 { .. },
                )) = endpoint.physical_type(out.budget)?
                else {
                    continue;
                };
                if endpoint.execution_borrow_v163(out.budget)?.is_some() {
                    continue;
                }
                let ty = endpoint.source_type(out.budget)?;
                let local = row.locals.start + endpoint.source_local(out.budget)?.index() as usize;
                let count = pairs.slots.aggregate_leaf_count(ty, out)?.unwrap();
                let start = out.text.len();
                match pairs.emit_source_leaf_conjunct(
                    &plan,
                    root,
                    instance,
                    value,
                    0,
                    FormalIndexWidth::Unknown,
                    out,
                ) {
                    Err(Error::Statement(
                        "expanded scalar binding differs from its retained source endpoint",
                    )) => (),
                    Err(error) => return Err(error),
                    Ok(()) => panic!("unknown-width source leaf was admitted"),
                }
                assert_eq!(out.text.len(), start);
                for ordinal in 0..count {
                    let leaf = pairs.slots.aggregate_leaf(ty, ordinal, out)?;
                    let path = leaf.path(out)?.to_vec();
                    let scalar = leaf.scalar(out)?;
                    let start = out.text.len();
                    pairs.emit_source_leaf_conjunct(
                        &plan,
                        root,
                        instance,
                        value,
                        ordinal,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                    let text = &out.text[start..];
                    assert!(text.contains(&format!(
                        "invocation_source_execution_aggregate_current_v170(source, {local})"
                    )));
                    let fields = path
                        .iter()
                        .map(|field| format!("{field}int,"))
                        .collect::<String>();
                    assert!(text.contains(&format!(
                        "invocation_source_aggregate_leaf_v42(source, {local}, {}, seq![{fields}])",
                        ty.index()
                    )));
                    match scalar {
                        super::super::ScalarV30::Unit => {
                            assert!(text.contains("original == MemoryValueV30::Unit"));
                            counts[0] += 1;
                        }
                        super::super::ScalarV30::Bool => {
                            assert!(text.contains("byte_scalar_type_v57(actual, 2)"));
                            counts[1] += 1;
                        }
                        super::super::ScalarV30::Integer {
                            width: 32,
                            signed: false,
                        } => {
                            assert!(text.contains(
                                "byte_scalar_type_v57(actual, memory_value_modulus_v30(4))"
                            ));
                            counts[2] += 1;
                        }
                        _ => panic!("unexpected fixture leaf"),
                    }
                }
            }
        }
    }
    assert!(
        counts.into_iter().all(|count| count > 0),
        "unit, mask and payload must all be checked"
    );
    Ok(())
}

#[test]
fn expanded_source_leaf_relations_preserve_typed_payload_masks_units_and_current_lease() {
    for layout in [Layout::Blocked, Layout::Striped] {
        run_fixture(layout, LIMIT, LIMIT, |slots, _, out| {
            let target = TileTargetV176::derive(slots, out)?;
            let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
            exercise_source_leaves(&pairs, out)
        })
        .0
        .unwrap();
    }
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
                endpoint.carrier_shape(out.budget)?,
                fe2o3_lower_mir_kernel::ProductionSourceSsaCarrierShapeV37::Value
                    | fe2o3_lower_mir_kernel::ProductionSourceSsaCarrierShapeV37::Unit
            ) {
                continue;
            }
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
fn expanded_source_relations_keep_address_observable_scalars_in_explicit_memory() {
    use super::super::super::{byte_bindings::SourceByteBindings, slots::tests::with_tile_slots};
    use fe2o3_mir_model::{SsaBlockIdV1, SsaResolvedEventV1};

    for layout in [Layout::Blocked, Layout::Striped] {
        super::super::super::super::invocations::tests::run_scalar_allocation_variant(
            LIMIT, LIMIT, |plan, out| {
                with_tile_slots(plan, layout, out, |slots, out| {
                    let target = TileTargetV176::derive(slots, out)?;
                    let bindings = SourceByteBindings::derive_expanded_v188(&target, out)?;
                    let source = slots.correspondence(out)?.source(out.budget)?;
                    let archive = source.source_ssa(out.budget)?;
                    let semantic = source.source_semantic(out.budget)?;
                    let start = out.text.len();
                    bindings.emit(out)?;
                    for root in 0..2 {
                        let row = plan.instance(root, 0, out)?;
                        let (descriptor, _) = slots.legacy_descriptor_by_source(root, 0, 4, out)?
                            .expect("genuine address-observable scalar allocation");
                        let ssa = archive.plan_for_function(row.function).unwrap().plan();
                        assert!(ssa.entry_definitions().iter().all(|entry| entry.variable().get() != 4));
                        let function = &semantic.functions()[row.function.index() as usize];
                        for block in 0..function.blocks().len() {
                            let block = SsaBlockIdV1::new(block as u32);
                            let events = ssa.resolved_events(block);
                            assert_eq!(events.is_some(), ssa.is_reachable(block));
                            for (_, event) in events.into_iter().flatten() {
                                if let SsaResolvedEventV1::Define { variable, .. } = event {
                                    assert_ne!(variable.get(), 4, "memory must not become a fabricated SSA definition");
                                }
                            }
                        }
                        assert!(out.text[start..].contains(&format!("private.insert(source.slots[{descriptor}].allocation, InvocationByteBindingV36")));
                        assert!(out.text[start..].contains(&format!("spec fn invocation_source_byte_storage_related_{root}_v36")));
                    }
                    Ok(())
                })
            },
        ).0.unwrap();
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
fn expanded_source_values_follow_authenticated_scalar_substitutions_with_exact_bounds() {
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            run_fixture(layout, work, storage, |slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
                let original = slots.correspondence(out)?.inventory(out.budget)?;
                let tile = slots.tile_owner_v176(out)?;
                let neutral = tile.neutral_source_v162(out.budget)?;
                let predecessor = neutral.output_inventory(out.budget)?;
                let actual = target.inventory(out)?;
                let mut substitutions = 0;
                for (index, row) in original.definitions().iter().enumerate() {
                    if !matches!(
                        row.ty,
                        Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_)
                    ) {
                        continue;
                    }
                    let descendants = neutral.definition_descendants(row.coordinate, out.budget)?;
                    let [descendant] = descendants else {
                        continue;
                    };
                    if descendant.kind != Descendant::Substituted {
                        continue;
                    }
                    let predecessor = &predecessor.definitions()
                        [definition_index(predecessor, descendant.output)?];
                    let function = match predecessor.coordinate {
                        Definition::FunctionArgument { function, .. } => function,
                        Definition::BlockArgument { block, .. } => block.function,
                        Definition::Result { operation, .. } => operation.block.function,
                    };
                    let expected = actual
                        .definitions()
                        .iter()
                        .enumerate()
                        .filter(|(index, candidate)| {
                            actual.functions()[function.0 as usize]
                                .definitions
                                .contains(index)
                                && candidate.value == predecessor.value
                                && candidate.ty == row.ty
                        })
                        .map(|(index, _)| index)
                        .collect::<Vec<_>>();
                    assert_eq!(expected.len(), 1);
                    assert_eq!(pairs.source_definition(index, out)?, expected[0]);
                    refusal(pairs.definition(index, out))?;
                    substitutions += 1;
                }
                assert!(
                    substitutions > 0,
                    "fixture must contain genuine checked replacements"
                );
                refusal(pairs.source_definition(original.definitions().len(), out))?;
                refusal(pairs.source_definition(usize::MAX, out))?;
                Ok(())
            })
        };
        let baseline = run(LIMIT, LIMIT);
        baseline.0.unwrap();
        let exact = run(baseline.1, baseline.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (baseline.1, baseline.2, baseline.3)
        );
        assert!(matches!(run(baseline.1 - 1, baseline.3).0,
            Err(Error::Resource(Resource::Work(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == baseline.1 && error.limit() == baseline.1 - 1));
        assert!(matches!(run(baseline.1, baseline.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == baseline.3 && error.limit() == baseline.3 - 1));
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
fn expanded_scalar_forwarding_joins_original_edges_with_exact_bounds() {
    use super::super::super::source_function::tile_fixture_tests::run_fixture_with_slice_entry_edge_v286;
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            run_fixture_with_slice_entry_edge_v286(layout, work, storage, |slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
                let original = slots.correspondence(out)?.inventory(out.budget)?;
                let tile = slots.tile_owner_v176(out)?;
                let neutral = tile.neutral_source_v162(out.budget)?;
                let predecessor = neutral.output_inventory(out.budget)?;
                let actual = target.inventory(out)?;
                let mut forwarded = 0;
                let mut forwarded_slices = 0;
                let mut forwarded_scalars = 0;
                let mut missing_computations = 0;
                let mut ambiguous_inputs = 0;
                for (index, row) in original.definitions().iter().enumerate() {
                    if !matches!(row.ty, Type::Scalar(_) | Type::Slice(_))
                        || !neutral
                            .definition_descendants(row.coordinate, out.budget)?
                            .is_empty()
                    {
                        continue;
                    }
                    // This independent oracle uses a visited set and all original
                    // edges, not the production iteration bound/function slice.
                    let mut seen = std::collections::BTreeSet::new();
                    let mut chain = Vec::new();
                    let mut current = index;
                    let expected = loop {
                        if !seen.insert(current) {
                            break None;
                        }
                        chain.push(current);
                        let current_row = &original.definitions()[current];
                        assert_eq!(current_row.ty, row.ty);
                        let descendants =
                            neutral.definition_descendants(current_row.coordinate, out.budget)?;
                        if let [descendant] = descendants {
                            let prior = &predecessor.definitions()
                                [definition_index(predecessor, descendant.output)?];
                            let function = match prior.coordinate {
                                Definition::FunctionArgument { function, .. } => function,
                                Definition::BlockArgument { block, .. } => block.function,
                                Definition::Result { operation, .. } => operation.block.function,
                            };
                            let candidates = actual
                                .definitions()
                                .iter()
                                .enumerate()
                                .filter(|(at, candidate)| {
                                    actual.functions()[function.0 as usize]
                                        .definitions
                                        .contains(at)
                                        && candidate.value == prior.value
                                        && candidate.ty == row.ty
                                })
                                .map(|(at, _)| at)
                                .collect::<Vec<_>>();
                            break match candidates.as_slice() {
                                [at] => Some(*at),
                                _ => None,
                            };
                        }
                        if !descendants.is_empty() {
                            break None;
                        }
                        let Definition::BlockArgument { block, .. } = current_row.coordinate else {
                            break None;
                        };
                        let incoming = original
                            .edge_arguments()
                            .iter()
                            .filter(|edge| edge.target_definition == current)
                            .map(|edge| {
                                assert_eq!(edge.coordinate.edge.source.function, block.function);
                                assert!(
                                    original.functions()[block.function.0 as usize]
                                        .definitions
                                        .contains(&edge.incoming_definition)
                                );
                                assert_eq!(
                                    original.definitions()[edge.incoming_definition].value,
                                    Some(edge.value)
                                );
                                edge.incoming_definition
                            })
                            .collect::<std::collections::BTreeSet<_>>();
                        if incoming.len() != 1 {
                            ambiguous_inputs += usize::from(incoming.len() > 1);
                            break None;
                        }
                        current = *incoming.first().unwrap();
                    };
                    // The old API must remain closed even for newly located inputs.
                    refusal(pairs.source_definition(index, out))?;
                    match expected {
                        Some(expected) => {
                            assert_eq!(pairs.source_transport_definition(index, out)?, expected);
                            assert!(chain.len() > 1);
                            if forwarded == 0 {
                                eprintln!(
                                    "original scalar forwarding {layout:?}: {chain:?} -> actual {expected}"
                                );
                            }
                            forwarded += 1;
                            match row.ty {
                                Type::Slice(_) => forwarded_slices += 1,
                                Type::Scalar(_) => forwarded_scalars += 1,
                                _ => {
                                    unreachable!("oracle selects only complete scalar/slice types")
                                }
                            }
                        }
                        None => {
                            refusal(pairs.source_transport_definition(index, out))?;
                            missing_computations +=
                                usize::from(matches!(row.coordinate, Definition::Result { .. }));
                        }
                    }
                }
                assert!(
                    forwarded_scalars > 0,
                    "fixture must retain nonvacuous scalar forwarding"
                );
                assert!(
                    forwarded_slices > 0,
                    "fixture must retain nonvacuous whole-slice forwarding"
                );
                assert!(
                    missing_computations > 0,
                    "erased computations must remain refused"
                );
                assert!(
                    ambiguous_inputs > 0,
                    "distinct actual incoming definitions must remain refused"
                );
                refusal(pairs.source_transport_definition(original.definitions().len(), out))?;
                refusal(pairs.source_transport_definition(usize::MAX, out))?;
                Ok(())
            })
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(measured.1, measured.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
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
fn expanded_scalar_reconstruction_uses_original_diamonds_and_retained_snapshots() {
    use fe2o3_kernel_ir::{ExecutionOperationV15 as Execution, OperationKind};
    for layout in [Layout::Blocked, Layout::Striped] {
        let run = |work, storage| {
            run_fixture(layout, work, storage, |slots, _, out| {
                let target = TileTargetV176::derive(slots, out)?;
                let pairs = ExpandedScalarBindingsV196::derive(slots, &target, out)?;
                let original = slots.correspondence(out)?.inventory(out.budget)?;
                let tile = slots.tile_owner_v176(out)?;
                let neutral = tile.neutral_source_v162(out.budget)?;
                let mut parts = 0;
                let mut diamonds = 0;
                for (index, row) in original.definitions().iter().enumerate() {
                    if let Definition::Result { operation, result } = row.coordinate {
                        let op = &original.operations()
                            [crate::mixed_optimizer_refinement_v26::semantics::operation_index(
                                original, operation,
                            )?];
                        if let OperationKind::Execution(Execution::FragmentIntoPartsU32 {
                            fragment,
                            elements,
                            ..
                        }) = op.operation.kind
                        {
                            let fragment = original
                                .definitions()
                                .iter()
                                .enumerate()
                                .find(|(index, row)| {
                                    original.functions()[operation.block.function.0 as usize]
                                        .definitions
                                        .contains(index)
                                        && row.value == Some(fragment)
                                })
                                .map(|(index, _)| index)
                                .unwrap();
                            let elements = u32::from(elements);
                            let expected = pairs
                                .tile_leaf(fragment, &[result / elements, result % elements], out)?
                                .unwrap();
                            let start = out.text.len();
                            pairs.emit_source_original_relation(
                                0,
                                Some(index),
                                FormalIndexWidth::Bits64,
                                out,
                            )?;
                            let text = &out.text[start..];
                            assert!(
                                text.contains(&format!("let actual = target.values[{expected}];"))
                            );
                            assert!(!text.contains("byte_load"));
                            parts += 1;
                        }
                    }
                    if !matches!(row.coordinate, Definition::BlockArgument { .. })
                        || !matches!(row.ty, Type::Scalar(fe2o3_kernel_ir::ScalarType::U32))
                        || !neutral
                            .definition_descendants(row.coordinate, out.budget)?
                            .is_empty()
                    {
                        continue;
                    }
                    let incoming = original
                        .edge_arguments()
                        .iter()
                        .filter(|edge| edge.target_definition == index)
                        .map(|edge| edge.incoming_definition)
                        .collect::<std::collections::BTreeSet<_>>();
                    if incoming.len() != 2 {
                        continue;
                    }
                    let start = out.text.len();
                    pairs.emit_source_original_relation(
                        0,
                        Some(index),
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                    let text = &out.text[start..];
                    assert!(text.contains("reconstructed_branch_"));
                    assert!(text.contains("reconstructed_sum_"));
                    assert!(text.contains("% 4294967296"));
                    assert!(text.contains(&format!("invocation_value_related_v36(original, reconstructed_{index}, map, source.machine.memory, target.memory)")));
                    assert!(!text.contains("byte_load") && !text.contains("assume("));
                    diamonds += 1;
                }
                assert!(parts > 0 && diamonds > 0);
                Ok(())
            })
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(measured.1, measured.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
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
            assert!(pairs.source_transport_definition(0, out).is_err());
            assert!(
                pairs
                    .emit_source_original_relation(0, Some(0), FormalIndexWidth::Bits64, out)
                    .is_err()
            );
            Err(error)
        });
        assert!(reached);
        assert!(result.0.is_err());
    }
}

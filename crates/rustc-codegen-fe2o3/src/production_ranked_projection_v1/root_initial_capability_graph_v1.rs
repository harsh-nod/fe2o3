//! Shared initial source-local capability graph, before invocation propagation.
//! The legacy caller retains its allocation sites, order and later edge families.
//! The metered path is preparation, not checked origins, ranked placement or admission.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
use fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1;

pub(super) struct InitialGraphStorageV1 {
    pub(super) edges: Vec<Vec<CapabilityEdgeV1>>,
    pub(super) edge_count: usize,
    pub(super) stores: Vec<PendingEnumPayloadStoreV1>,
    pub(super) loads: Vec<PendingEnumPayloadLoadV1>,
    pub(super) borrowed: Vec<(usize, usize)>,
}
impl InitialGraphStorageV1 {
    pub(super) const fn empty() -> Self {
        Self {
            edges: Vec::new(),
            edge_count: 0,
            stores: Vec::new(),
            loads: Vec::new(),
            borrowed: Vec::new(),
        }
    }
}

fn place_work(
    place: &SemanticPlaceV1,
    locals: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let work = place
        .projections()
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(16))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    resources.work(work)?;
    if place.local().index() as usize >= locals {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a capability def-use edge outside the semantic local table",
        ));
    }
    Ok(())
}
fn operand_work(
    operand: &SemanticOperandV1,
    locals: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    resources.work(8)?;
    if let Some(place) = raw_operand_place(operand) {
        place_work(place, locals, resources)?;
    }
    Ok(())
}
fn statement_work(
    statement: &SemanticStatementV1,
    locals: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !resources.is_metered() {
        return Ok(());
    }
    resources.work(64)?;
    if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
        place_work(assignment.destination(), locals, resources)?;
        assignment
            .value()
            .kind()
            .try_visit_operands(|operand| operand_work(operand, locals, resources))?;
        match assignment.value().kind() {
            SemanticRvalueKindV1::Borrow { place, .. }
            | SemanticRvalueKindV1::AddressOf { place, .. } => {
                place_work(place, locals, resources)?
            }
            _ => {}
        }
    }
    Ok(())
}
fn call_work(
    call: &SemanticDirectCallV1,
    locals: usize,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !resources.is_metered() {
        return Ok(());
    }
    // Callable lookup, destination classification and fixed geometry/Option checks.
    resources.work(128)?;
    for operand in call.arguments() {
        operand_work(operand, locals, resources)?;
    }
    if let Some(destination) = call.destination() {
        place_work(destination.place(), locals, resources)?;
    }
    Ok(())
}
fn reserve_row<T>(
    values: &mut Vec<T>,
    legacy_error: &'static str,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if resources.is_metered() {
        resources.work(16)?;
        resources.reserve(values, 1)
    } else {
        values
            .try_reserve(1)
            .map_err(|_| ProductionRankedProjectionErrorV1::Unsupported(legacy_error))
    }
}
fn edge(
    edges: &mut [Vec<CapabilityEdgeV1>],
    count: &mut usize,
    source: usize,
    value: CapabilityEdgeV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !resources.is_metered() {
        return push_capability_edge(edges, count, source, value);
    }
    resources.work(32)?;
    // Same bounds and fixed cap, checked before accepting allocation credit.
    if source >= edges.len() || value.destination >= edges.len() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a capability def-use edge outside the semantic local table",
        ));
    }
    if *count >= MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability def-use edges exceed the charged projection limit",
        ));
    }
    resources.push(&mut edges[source], value)?;
    *count += 1;
    Ok(())
}
fn sort_stores(
    stores: &mut [PendingEnumPayloadStoreV1],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if !resources.is_metered() {
        stores.sort_unstable_by_key(|store| (store.carrier, store.variant));
        return Ok(());
    }
    // Explicit comparison/move admission; no assumed hidden library-sort bound.
    // Equal keys are refused as ambiguous before any store is selected.
    for end in 1..stores.len() {
        resources.work(8)?;
        let value = stores[end];
        let mut cursor = end;
        while cursor > 0 {
            resources.work(8)?;
            let prior = stores[cursor - 1];
            if (prior.carrier, prior.variant) <= (value.carrier, value.variant) {
                break;
            }
            resources.work(8)?;
            stores[cursor] = prior;
            cursor -= 1;
        }
        resources.work(8)?;
        stores[cursor] = value;
    }
    Ok(())
}

// Semantic data only: no raw callables/indices passed here confer source authority.
// Strict authority joins live in the real-facts context's sealed child.
#[allow(clippy::too_many_arguments)]
pub(super) fn populate_initial_graph_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    linear_launch_upper_bound: Option<u64>,
    local_definitions: &[u8],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    edges_by_source: &mut [Vec<CapabilityEdgeV1>],
    edge_count: &mut usize,
    enum_payload_stores: &mut Vec<PendingEnumPayloadStoreV1>,
    enum_payload_loads: &mut Vec<PendingEnumPayloadLoadV1>,
    borrowed_locals: &mut Vec<(usize, usize)>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    for (block_index, block) in function.blocks().iter().enumerate() {
        resources.work(32)?;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            statement_work(statement, function.locals().len(), resources)?;
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            if let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                && let SemanticAggregateKindV1::EnumVariant(variant) = aggregate.kind()
                && let [operand] = aggregate.operands()
                && let Some(source) = transparent_operand_place(operand)
            {
                if enum_payload_stores.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "single-payload enum stores exceed the charged projection limit",
                    ));
                }
                reserve_row(
                    enum_payload_stores,
                    "single-payload enum store storage cannot be reserved",
                    resources,
                )?;
                enum_payload_stores.push(PendingEnumPayloadStoreV1 {
                    carrier: assignment.destination().local().index() as usize,
                    variant: *variant,
                    source: source.local().index() as usize,
                    construction_block: block_index,
                    statement: statement_index,
                });
                continue;
            }
            if let SemanticRvalueKindV1::Use(operand) = assignment.value().kind()
                && let Some(place) = raw_operand_place(operand)
                && let Some((carrier, variant)) = enum_payload_projection(place)
            {
                if enum_payload_loads.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "single-payload enum loads exceed the charged projection limit",
                    ));
                }
                reserve_row(
                    enum_payload_loads,
                    "single-payload enum load storage cannot be reserved",
                    resources,
                )?;
                enum_payload_loads.push(PendingEnumPayloadLoadV1 {
                    carrier,
                    variant,
                    destination: assignment.destination().local().index() as usize,
                    use_block: block_index,
                    statement: statement_index,
                });
            }
            let (source, borrowed) = match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => (transparent_operand_place(operand), false),
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                    if place.projections().is_empty() =>
                {
                    (Some(place), true)
                }
                _ => (None, false),
            };
            let Some(source) = source else {
                continue;
            };
            let source = source.local().index() as usize;
            let destination = assignment.destination().local().index() as usize;
            if borrowed {
                if borrowed_locals.len() == MAX_PROJECTED_OPERATIONS_V1 {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "borrowed capability uses exceed the charged projection limit",
                    ));
                }
                reserve_row(
                    borrowed_locals,
                    "borrowed capability use storage cannot be reserved",
                    resources,
                )?;
                borrowed_locals.push((source, block_index));
            }
            edge(
                edges_by_source,
                edge_count,
                source,
                CapabilityEdgeV1 {
                    destination,
                    use_block: block_index,
                    kind: CapabilityEdgeKindV1::Alias,
                },
                resources,
            )?;
        }

        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        call_work(call, function.locals().len(), resources)?;
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        let kind = match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { .. } => {
                CapabilityEdgeKindV1::Alias
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint {
                index_space, ..
            } => CapabilityEdgeKindV1::IntoDisjoint {
                mapping: *index_space,
            },
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
                output_space,
                offset,
                ..
            }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
                output_space,
                offset,
                ..
            } => {
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked shift lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedShift {
                    mapping: *output_space,
                    offset: *offset,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                output_space,
                lanes_per_block,
                elements_per_lane,
                ..
            } => {
                let expected = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                };
                if *output_space != expected
                    || *lanes_per_block == 0
                    || *elements_per_lane == 0
                    || lanes_per_block.checked_mul(*elements_per_lane).is_none()
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a malformed blocked mapping reached ranked projection",
                    ));
                }
                if !blocked_mapping_fits_launch_v1(
                    linear_launch_upper_bound,
                    *lanes_per_block,
                    *elements_per_lane,
                ) {
                    return Err(ProductionRankedProjectionErrorV1::Incomplete(
                        "a multi-lane blocked mapping requires an authenticated finite rank-1 launch extent whose full blocked index range fits u64",
                    ));
                }
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked block lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedBlock {
                    mapping: expected,
                    lanes_per_block: *lanes_per_block,
                    elements_per_lane: *elements_per_lane,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                output_space,
                lanes_per_tile,
                tile_rows,
                tile_columns,
                elements_per_lane,
                ..
            } => {
                let expected = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
                    lanes_per_tile: *lanes_per_tile,
                    tile_rows: *tile_rows,
                    tile_columns: *tile_columns,
                    elements_per_lane: *elements_per_lane,
                };
                if *output_space != expected
                    || !tiled_2d_geometry_valid_v1(
                        *lanes_per_tile,
                        *tile_rows,
                        *tile_columns,
                        *elements_per_lane,
                    )
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a malformed tiled-2d mapping reached ranked projection",
                    ));
                }
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked tiled-2d witness lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedTiled2d {
                    mapping: expected,
                    availability,
                }
            }
            SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                output_space,
                lanes_per_row,
                elements_per_lane,
                ..
            } => {
                let expected = SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                    lanes_per_row: *lanes_per_row,
                    elements_per_lane: *elements_per_lane,
                };
                if *output_space != expected
                    || !row_striped_2d_geometry_valid_v1(*lanes_per_row, *elements_per_lane)
                {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "a malformed row-striped-2d mapping reached ranked projection",
                    ));
                }
                let destination = simple_call_destination(call)?;
                let availability = option_dominance.availability(destination).ok_or(
                    ProductionRankedProjectionErrorV1::Incomplete(
                        "a checked row-striped-2d witness lacks authenticated Option Some availability",
                    ),
                )?;
                CapabilityEdgeKindV1::CheckedRowStriped2d {
                    mapping: expected,
                    availability,
                }
            }
            _ => continue,
        };
        let destination = simple_call_destination(call)?.index() as usize;
        let source = call
            .arguments()
            .first()
            .and_then(simple_operand_local)
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "an index capability transform without one exact input local",
            ))?
            .index() as usize;
        edge(
            edges_by_source,
            edge_count,
            source,
            CapabilityEdgeV1 {
                destination,
                use_block: block_index,
                kind,
            },
            resources,
        )?;
    }

    sort_stores(enum_payload_stores, resources)?;
    for load in enum_payload_loads.iter().copied() {
        // Both partition searches and enum availability binary search are bounded
        // by machine-word index width; fixed row classification is included.
        resources.work(6 * usize::BITS as usize + 64)?;
        let key = (load.carrier, load.variant);
        let first =
            enum_payload_stores.partition_point(|store| (store.carrier, store.variant) < key);
        let end =
            enum_payload_stores.partition_point(|store| (store.carrier, store.variant) <= key);
        let matches = &enum_payload_stores[first..end];
        let Some(store) = matches.first() else {
            continue;
        };
        if matches.len() != 1 {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "an enum payload has multiple candidate capability stores",
            ));
        }
        let kind = if store.construction_block == load.use_block && store.statement < load.statement
        {
            CapabilityEdgeKindV1::Alias
        } else {
            if local_definitions.get(load.carrier).copied() != Some(1) {
                continue;
            }
            let Some(availability) = enum_payload_dominance.availability(
                SemanticLocalIdV1::from_index(load.carrier as u32),
                load.variant,
            ) else {
                continue;
            };
            CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                construction_block: store.construction_block,
                availability,
            }
        };
        edge(
            edges_by_source,
            edge_count,
            store.source,
            CapabilityEdgeV1 {
                destination: load.destination,
                use_block: load.use_block,
                kind,
            },
            resources,
        )?;
    }

    Ok(())
}

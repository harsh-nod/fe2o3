//! Shared invocation seed/index propagation. Paid results are UNJOINED DATA:
//! no actual root operation namespace, source-owner join, access or admission.
use super::bf16_nominal_preparation_resources_v1::{PreparationResourcesV1, resource};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkLedgerIdentityV1,
};
use std::mem::size_of;
type Result<T> = std::result::Result<T, ProductionRankedProjectionErrorV1>;

enum IndexQueueV1<'a> {
    Legacy(&'a mut VecDeque<usize>),
    Paid {
        values: &'a mut Vec<usize>,
        cursor: &'a mut usize,
    },
}
impl IndexQueueV1<'_> {
    fn push(&mut self, value: usize, resources: &mut PreparationResourcesV1<'_, '_>) -> Result<()> {
        match self {
            Self::Legacy(queue) => queue.push_back(value),
            Self::Paid { values, .. } => {
                resources.work(16)?;
                resources.push(values, value)?;
            }
        }
        Ok(())
    }
    fn pop(&mut self, resources: &mut PreparationResourcesV1<'_, '_>) -> Result<Option<usize>> {
        match self {
            Self::Legacy(queue) => Ok(queue.pop_front()),
            Self::Paid { values, cursor } => {
                resources.work(16)?;
                let value = values.get(**cursor).copied();
                if value.is_some() {
                    **cursor += 1;
                }
                Ok(value)
            }
        }
    }
}

fn reserve_index_operation_v1(
    operations: &mut Vec<ProductionRankedOperationV1>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    if !resources.is_metered() {
        return reserve_operation(operations); // Exact original cap/allocation/refusal.
    }
    resources.work(32)?;
    if operations.len() >= MAX_PROJECTED_OPERATIONS_V1 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "a semantic intrinsic projection exceeding the ranked operation limit",
        ));
    }
    resources.reserve(operations, 1)
}
fn next_index_value_v1(
    next: &mut u32,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<ProductionRankedValueIdV1> {
    resources.work(32)?;
    next_value_id(next) // Keep overflow and mutation ordering unchanged.
}

/// The ordinary caller lends its ACTUAL preexisting operation stream and value
/// counter. No reseeding, relocation of earlier strided-read operations, copying,
/// sorting or alternate namespace is introduced. Its VecDeque allocation and
/// diagnostic formatting remain the original behavior.
#[allow(clippy::too_many_arguments)]
pub(super) fn seed_invocation_values_legacy_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    local_definitions: &[u8],
    address_escaped: &[bool],
    option_dominance: &SemanticOptionDominanceV1,
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &mut [Option<ProjectedGridLeaderV1>],
    option_predicates: &mut [Option<GuardPredicateV1>],
    index_worklist: &mut VecDeque<usize>,
    grid_worklist: &mut VecDeque<usize>,
    launch_extent: u64,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<()> {
    seed_invocation_values_v1(
        callables,
        function,
        local_definitions,
        address_escaped,
        option_dominance,
        index_values,
        grid_leaders,
        option_predicates,
        &mut IndexQueueV1::Legacy(index_worklist),
        &mut IndexQueueV1::Legacy(grid_worklist),
        launch_extent,
        operations,
        next_value,
        Some(ranked_ir),
        &mut PreparationResourcesV1::unmetered(),
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn propagate_index_values_legacy_v1(
    local_definitions: &[u8],
    address_escaped: &[bool],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    edges_by_source: &[Vec<CapabilityEdgeV1>],
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &[Option<ProjectedGridLeaderV1>],
    option_predicates: &mut [Option<GuardPredicateV1>],
    index_worklist: &mut VecDeque<usize>,
    processed_edges: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    ranked_ir: &mut String,
) -> Result<()> {
    propagate_index_values_v1(
        local_definitions,
        address_escaped,
        option_dominance,
        enum_payload_dominance,
        edges_by_source,
        index_values,
        grid_leaders,
        option_predicates,
        &mut IndexQueueV1::Legacy(index_worklist),
        processed_edges,
        operations,
        next_value,
        Some(ranked_ir),
        &mut PreparationResourcesV1::unmetered(),
    )
}

#[allow(clippy::too_many_arguments)]
fn seed_invocation_values_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    local_definitions: &[u8],
    address_escaped: &[bool],
    option_dominance: &SemanticOptionDominanceV1,
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &mut [Option<ProjectedGridLeaderV1>],
    option_predicates: &mut [Option<GuardPredicateV1>],
    index_worklist: &mut IndexQueueV1<'_>,
    grid_worklist: &mut IndexQueueV1<'_>,
    launch_extent: u64,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    mut ranked_ir: Option<&mut String>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    let local_count = function.locals().len();
    for block in function.blocks() {
        resources.work(32)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        resources.work(128)?;
        let Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        if !matches!(
            operation,
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. }
                | SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
        ) {
            continue;
        }
        if resources.is_metered()
            && matches!(
                operation,
                SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. }
            )
        {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "unjoined index data does not support grid-leader seeds",
            ));
        }
        let destination = simple_call_destination(call)?;
        let destination = destination.index() as usize;
        if destination >= local_count {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "an invocation-capability destination outside the semantic local table",
            ));
        }
        if index_values[destination].is_some() || grid_leaders[destination].is_some() {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple invocation capabilities for one semantic local",
            ));
        }
        reserve_index_operation_v1(operations, resources)?;
        let result = next_index_value_v1(next_value, resources)?;
        operations.push(ProductionRankedOperationV1::InvocationIndex {
            result,
            dimension: 0,
            launch_extent,
        });
        if let Some(ranked_ir) = ranked_ir.as_deref_mut() {
            push_ranked_ir(
                ranked_ir,
                &format!(
                    "  %{} = kernel.invocation_index <0, dynamic>\n",
                    result.get()
                ),
            )?;
        }
        match operation {
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d { .. } => {
                require_index_scalar_custody_v1(destination, local_definitions, address_escaped)?;
                index_values[destination] = Some(ProjectedDisjointIndexV1 {
                    value: ProductionRankedValueV1::Local(result),
                    mapping: SemanticDisjointIndexSpaceV1::Index1d,
                    precondition: None,
                    availability: None,
                });
                index_worklist.push(destination, resources)?;
            }
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader } => {
                let availability = option_dominance
                    .availability(SemanticLocalIdV1::from_index(destination as u32))
                    .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                        "a grid-leader capability lacks authenticated Option Some availability",
                    ))?;
                reserve_index_operation_v1(operations, resources)?;
                let one = next_index_value_v1(next_value, resources)?;
                operations.push(ProductionRankedOperationV1::IndexConstant {
                    result: one,
                    value: 1,
                });
                if let Some(ranked_ir) = ranked_ir.as_deref_mut() {
                    push_ranked_ir(
                        ranked_ir,
                        &format!("  %{} = kernel.index_constant 1\n", one.get()),
                    )?;
                }
                option_predicates[destination] = Some(GuardPredicateV1 {
                    comparisons: vec![(
                        ProductionRankedValueV1::Local(result),
                        ProductionRankedValueV1::Local(one),
                    )],
                });
                grid_leaders[destination] = Some(ProjectedGridLeaderV1 {
                    grid_leader: *grid_leader,
                    precondition: (
                        ProductionRankedValueV1::Local(result),
                        ProductionRankedValueV1::Local(one),
                    ),
                    availability: CapabilityAvailabilityV1::Option(availability),
                });
                grid_worklist.push(destination, resources)?;
            }
            _ => unreachable!(),
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn propagate_index_values_v1(
    local_definitions: &[u8],
    address_escaped: &[bool],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    edges_by_source: &[Vec<CapabilityEdgeV1>],
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &[Option<ProjectedGridLeaderV1>],
    option_predicates: &mut [Option<GuardPredicateV1>],
    index_worklist: &mut IndexQueueV1<'_>,
    processed_edges: &mut usize,
    operations: &mut Vec<ProductionRankedOperationV1>,
    next_value: &mut u32,
    mut ranked_ir: Option<&mut String>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    while let Some(source) = index_worklist.pop(resources)? {
        resources.work(64)?;
        require_index_scalar_custody_v1(source, local_definitions, address_escaped)?;
        let input = index_values[source].ok_or(ProductionRankedProjectionErrorV1::Unsupported(
            "the capability worklist lost an index value",
        ))?;
        for edge in &edges_by_source[source] {
            resources.work(128)?;
            if resources.is_metered()
                && !matches!(
                    edge.kind,
                    CapabilityEdgeKindV1::Alias
                        | CapabilityEdgeKindV1::AuthenticatedEnumPayload { .. }
                )
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "unjoined index data does not support this edge family",
                ));
            }
            *processed_edges = processed_edges.checked_add(1).ok_or(
                ProductionRankedProjectionErrorV1::Unsupported(
                    "capability work accounting overflowed",
                ),
            )?;
            let authorization_block = match edge.kind {
                CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block, ..
                } => construction_block,
                _ => edge.use_block,
            };
            if !input.availability.is_none_or(|availability| {
                capability_availability_allows(
                    option_dominance,
                    enum_payload_dominance,
                    availability,
                    SemanticBlockIdV1::from_index(authorization_block as u32),
                )
            }) {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "an index capability is used outside its authenticated Some edge",
                ));
            }
            let projected = match edge.kind {
                CapabilityEdgeKindV1::Alias | CapabilityEdgeKindV1::AuthenticatedOptionPayload => {
                    input
                }
                CapabilityEdgeKindV1::AuthenticatedEnumPayload { availability, .. } => {
                    ProjectedDisjointIndexV1 {
                        availability: Some(CapabilityAvailabilityV1::EnumPayload(availability)),
                        ..input
                    }
                }
                CapabilityEdgeKindV1::IntoDisjoint { mapping } => {
                    ProjectedDisjointIndexV1 { mapping, ..input }
                }
                CapabilityEdgeKindV1::CheckedShift {
                    mapping,
                    offset,
                    availability,
                } => {
                    reserve_index_operation_v1(operations, resources)?;
                    let offset_value = next_index_value_v1(next_value, resources)?;
                    operations.push(ProductionRankedOperationV1::IndexConstant {
                        result: offset_value,
                        value: offset,
                    });
                    reserve_index_operation_v1(operations, resources)?;
                    let shifted = next_index_value_v1(next_value, resources)?;
                    operations.push(ProductionRankedOperationV1::IndexBinary {
                        result: shifted,
                        kind: IndexBinaryKindAttr::Add,
                        lhs: input.value,
                        rhs: ProductionRankedValueV1::Local(offset_value),
                    });
                    if let Some(ranked_ir) = ranked_ir.as_deref_mut() {
                        push_ranked_ir(
                            ranked_ir,
                            &format!(
                                "  %{} = kernel.index_constant {}\n  %{} = kernel.index_binary Add {}, %{}\n",
                                offset_value.get(),
                                offset,
                                shifted.get(),
                                ranked_value_text_v1(input.value),
                                offset_value.get(),
                            ),
                        )?;
                    }
                    let precondition = if offset == 0 {
                        input.precondition
                    } else {
                        reserve_index_operation_v1(operations, resources)?;
                        let upper = next_index_value_v1(next_value, resources)?;
                        operations.push(ProductionRankedOperationV1::IndexConstant {
                            result: upper,
                            value: u64::MAX - offset + 1,
                        });
                        if let Some(ranked_ir) = ranked_ir.as_deref_mut() {
                            push_ranked_ir(
                                ranked_ir,
                                &format!(
                                    "  %{} = kernel.index_constant {}\n",
                                    upper.get(),
                                    u64::MAX - offset + 1,
                                ),
                            )?;
                        }
                        Some((input.value, ProductionRankedValueV1::Local(upper)))
                    };
                    ProjectedDisjointIndexV1 {
                        value: ProductionRankedValueV1::Local(shifted),
                        mapping,
                        precondition,
                        availability: Some(CapabilityAvailabilityV1::Option(availability)),
                    }
                }
                CapabilityEdgeKindV1::CheckedBlock {
                    mapping,
                    lanes_per_block,
                    elements_per_lane,
                    availability,
                } => {
                    if lanes_per_block == 1 {
                        let maximum_raw = (u64::MAX - (elements_per_lane - 1)) / elements_per_lane;
                        reserve_index_operation_v1(operations, resources)?;
                        let upper = next_index_value_v1(next_value, resources)?;
                        operations.push(ProductionRankedOperationV1::IndexConstant {
                            result: upper,
                            value: maximum_raw + 1,
                        });
                        if let Some(ranked_ir) = ranked_ir.as_deref_mut() {
                            push_ranked_ir(
                                ranked_ir,
                                &format!(
                                    "  %{} = kernel.index_constant {}\n",
                                    upper.get(),
                                    maximum_raw + 1,
                                ),
                            )?;
                        }
                        ProjectedDisjointIndexV1 {
                            mapping,
                            precondition: Some((
                                input.value,
                                ProductionRankedValueV1::Local(upper),
                            )),
                            availability: Some(CapabilityAvailabilityV1::Option(availability)),
                            ..input
                        }
                    } else {
                        ProjectedDisjointIndexV1 {
                            mapping,
                            availability: Some(CapabilityAvailabilityV1::Option(availability)),
                            ..input
                        }
                    }
                }
                CapabilityEdgeKindV1::CheckedTiled2d {
                    mapping,
                    availability,
                } => ProjectedDisjointIndexV1 {
                    mapping,
                    availability: Some(CapabilityAvailabilityV1::Option(availability)),
                    ..input
                },
                CapabilityEdgeKindV1::CheckedRowStriped2d {
                    mapping,
                    availability,
                } => ProjectedDisjointIndexV1 {
                    mapping,
                    availability: Some(CapabilityAvailabilityV1::Option(availability)),
                    ..input
                },
            };
            if matches!(
                edge.kind,
                CapabilityEdgeKindV1::CheckedShift { .. }
                    | CapabilityEdgeKindV1::CheckedBlock { .. }
                    | CapabilityEdgeKindV1::CheckedTiled2d { .. }
                    | CapabilityEdgeKindV1::CheckedRowStriped2d { .. }
            ) {
                let predicate = option_predicates.get_mut(edge.destination).ok_or(
                    ProductionRankedProjectionErrorV1::Unsupported(
                        "a checked capability destination outside the semantic local table",
                    ),
                )?;
                if predicate.is_some() {
                    return Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "multiple checked predicates for one semantic local",
                    ));
                }
                *predicate = Some(GuardPredicateV1::from_precondition(projected.precondition));
            }
            assign_index_capability_v1(
                edge.destination,
                projected,
                index_values,
                grid_leaders,
                index_worklist,
                resources,
            )?;
        }
    }

    Ok(())
}
/// Keep the existing alias-cycle/conflict regressions on the shared assignment
/// implementation without exposing the metered queue adapter to the parent.
#[cfg(test)]
pub(super) fn assign_index_capability_legacy_v1(
    destination: usize,
    projected: ProjectedDisjointIndexV1,
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &[Option<ProjectedGridLeaderV1>],
    worklist: &mut VecDeque<usize>,
) -> Result<()> {
    assign_index_capability_v1(
        destination,
        projected,
        index_values,
        grid_leaders,
        &mut IndexQueueV1::Legacy(worklist),
        &mut PreparationResourcesV1::unmetered(),
    )
}

fn assign_index_capability_v1(
    destination: usize,
    projected: ProjectedDisjointIndexV1,
    index_values: &mut [Option<ProjectedDisjointIndexV1>],
    grid_leaders: &[Option<ProjectedGridLeaderV1>],
    worklist: &mut IndexQueueV1<'_>,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    resources.work(32)?;
    if destination >= index_values.len() || grid_leaders[destination].is_some() {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "an index capability escaped the semantic local table or changed capability kind",
        ));
    }
    match index_values[destination] {
        None => {
            index_values[destination] = Some(projected);
            worklist.push(destination, resources)?;
        }
        Some(existing) if existing == projected => {}
        Some(_) => {
            return Err(ProductionRankedProjectionErrorV1::Unsupported(
                "multiple index capabilities reach one semantic local",
            ));
        }
    }
    Ok(())
}

/// Intermediate payload in a LOCAL, UNJOINED value namespace. Index values are
/// not IDs in the actual root operation stream. No production consumer is lent
/// these rows until that actual stream/allocator/source graph is joined later.
pub(super) struct UnjoinedInvocationIndexPayloadV1 {
    pub(super) indices: Vec<Option<ProjectedDisjointIndexV1>>,
    pub(super) grids: Vec<Option<ProjectedGridLeaderV1>>,
    pub(super) predicates: Vec<Option<GuardPredicateV1>>,
    pub(super) operations: Vec<ProductionRankedOperationV1>,
    pub(super) next_value: u32,
    pub(super) index_fifo: Vec<usize>,
    pub(super) index_cursor: usize,
    pub(super) grid_fifo: Vec<usize>,
    pub(super) grid_cursor: usize,
    pub(super) processed_edges: usize,
}
impl UnjoinedInvocationIndexPayloadV1 {
    const fn empty() -> Self {
        Self {
            indices: Vec::new(),
            grids: Vec::new(),
            predicates: Vec::new(),
            operations: Vec::new(),
            next_value: 0,
            index_fifo: Vec::new(),
            index_cursor: 0,
            grid_fifo: Vec::new(),
            grid_cursor: 0,
            processed_edges: 0,
        }
    }
}
/// Physical OUTER owner. Keep through all enclosing facts/rich postflights and
/// drop payload/error/panic before releasing only its accepted credit counter.
/// No refund, replacement, actual namespace constructor or ready/access view.
pub(super) struct PendingUnjoinedInvocationIndicesV1 {
    payload: Option<UnjoinedInvocationIndexPayloadV1>,
    ledger: Option<(usize, CanonicalKernelIrWorkLedgerIdentityV1)>,
    completed: bool,
}
impl PendingUnjoinedInvocationIndicesV1 {
    pub(super) const fn new() -> Self {
        Self {
            payload: None,
            ledger: None,
            completed: false,
        }
    }
    #[cfg(test)]
    pub(super) fn payload_for_test(&self) -> Option<&UnjoinedInvocationIndexPayloadV1> {
        self.payload.as_ref()
    }
    #[cfg(test)]
    pub(super) fn completed_for_test(&self) -> bool {
        self.completed
    }
}

/// Bounds/data-domain checks only. This is NOT the source-bound S1 whitelist or
/// an owner/inventory/actual-call proof. Other calls are not interpreted by this
/// index component; a future real connector must still enforce S1's whole
/// closed profile and use the actual root operation namespace.
fn narrow_input_edges_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    definitions: &[u8],
    address_escaped: &[bool],
    edges: &[Vec<CapabilityEdgeV1>],
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<usize> {
    resources.work(64)?;
    let locals = function.locals().len();
    let blocks = function.blocks().len();
    if blocks == 0
        || blocks > 32
        || locals > 4096
        || callables.len() > 4096
        || definitions.len() != locals
        || address_escaped.len() != locals
        || edges.len() != locals
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "unjoined index data has unsupported source or table bounds",
        ));
    }
    for block in function.blocks() {
        resources.work(64)?;
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
            resources.work(64)?;
            let callable = callables.get(call.callee().index() as usize).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "unjoined index source callable is absent",
                ),
            )?;
            if matches!(
                callable,
                SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { .. },
                    ..
                }
            ) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "unjoined index data does not support grid-leader seeds",
                ));
            }
        }
    }
    let mut count = 0usize;
    for row in edges {
        resources.work(32)?;
        for edge in row {
            resources.work(64)?;
            if edge.destination >= locals
                || edge.use_block >= blocks
                || matches!(edge.kind, CapabilityEdgeKindV1::AuthenticatedEnumPayload {
                    construction_block, ..
                } if construction_block >= blocks)
            {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "unjoined index edge is outside the source tables",
                ));
            }
            if !matches!(
                edge.kind,
                CapabilityEdgeKindV1::Alias | CapabilityEdgeKindV1::AuthenticatedEnumPayload { .. }
            ) {
                return Err(ProductionRankedProjectionErrorV1::Incomplete(
                    "unjoined index data does not support this edge family",
                ));
            }
            count = count
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            if count > MAX_PROJECTED_OPERATIONS_V1 {
                return Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "capability def-use edges exceed the charged projection limit",
                ));
            }
        }
    }
    Ok(count)
}

/// Paid component DATA only. Raw scalar/dominance/graph parameters confer no
/// source, graph completeness or access authority. This prepares an explicitly
/// UNJOINED local operation stream beginning at zero; do not splice it into the
/// actual root. Ordinary code instead uses the legacy wrappers with its real
/// stream/value counter, including operations already emitted by strided reads.
#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_unjoined_invocation_indices_v1(
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    definitions: &[u8],
    address_escaped: &[bool],
    option_dominance: &SemanticOptionDominanceV1,
    enum_payload_dominance: &SemanticEnumPayloadDominanceV1,
    edges: &[Vec<CapabilityEdgeV1>],
    pending: &mut PendingUnjoinedInvocationIndicesV1,
    resources: &mut PreparationResourcesV1<'_, '_>,
) -> Result<()> {
    if resources.has_denial() {
        return Err(resource(Resource::Accounting));
    }
    let ledger =
        resources
            .original_ledger_v1()
            .ok_or(ProductionRankedProjectionErrorV1::Incomplete(
                "unjoined index data requires an original ledger",
            ))?;
    resources.work(64)?;
    if pending.ledger.is_some_and(|saved| saved != ledger) {
        return Err(resource(Resource::Accounting));
    }
    if pending.payload.is_some() || pending.ledger.is_some() || pending.completed {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "unjoined index pending owner cannot be replaced or retried",
        ));
    }
    let frame = 8192usize
        .checked_add(size_of::<PendingUnjoinedInvocationIndicesV1>())
        .and_then(|n| n.checked_add(size_of::<UnjoinedInvocationIndexPayloadV1>()))
        .and_then(|n| n.checked_add(2 * size_of::<IndexQueueV1<'static>>()))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    resources.work(frame)?;
    resources.reserve_storage(frame)?;
    // Install the empty outer owner BEFORE any vector growth or fallible scan.
    pending.ledger = Some(ledger);
    pending.payload = Some(UnjoinedInvocationIndexPayloadV1::empty());
    let edge_count = narrow_input_edges_v1(
        function,
        callables,
        definitions,
        address_escaped,
        edges,
        resources,
    )?;
    let payload = pending
        .payload
        .as_mut()
        .expect("empty outer index owner installed");
    let locals = function.locals().len();
    resources.work(locals)?;
    resources.reserve(&mut payload.indices, locals)?;
    payload.indices.resize(locals, None);
    resources.work(locals)?;
    resources.reserve(&mut payload.grids, locals)?;
    payload.grids.resize(locals, None);
    resources.work(locals)?;
    resources.reserve(&mut payload.predicates, locals)?;
    payload.predicates.resize_with(locals, || None);
    seed_invocation_values_v1(
        callables,
        function,
        definitions,
        address_escaped,
        option_dominance,
        &mut payload.indices,
        &mut payload.grids,
        &mut payload.predicates,
        &mut IndexQueueV1::Paid {
            values: &mut payload.index_fifo,
            cursor: &mut payload.index_cursor,
        },
        &mut IndexQueueV1::Paid {
            values: &mut payload.grid_fifo,
            cursor: &mut payload.grid_cursor,
        },
        0,
        &mut payload.operations,
        &mut payload.next_value,
        None,
        resources,
    )?;
    // The paid domain forbids GridLeader seeds/Option-recovery/transform edges.
    // Its fresh all-None grid/predicate tables remain empty. It never runs the
    // unmetered late writer or broad grid propagation.
    propagate_index_values_v1(
        definitions,
        address_escaped,
        option_dominance,
        enum_payload_dominance,
        edges,
        &mut payload.indices,
        &payload.grids,
        &mut payload.predicates,
        &mut IndexQueueV1::Paid {
            values: &mut payload.index_fifo,
            cursor: &mut payload.index_cursor,
        },
        &mut payload.processed_edges,
        &mut payload.operations,
        &mut payload.next_value,
        None,
        resources,
    )?;
    if payload.processed_edges > edge_count {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "capability worklist exceeded its charged def-use edges",
        ));
    }
    if resources.has_denial() || resources.original_ledger_v1() != Some(ledger) {
        return Err(resource(Resource::Accounting));
    }
    pending.completed = true; // DATA completion only; no source/readiness view.
    Ok(())
}

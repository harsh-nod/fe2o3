//! Closed, same-owner transport of a checked row-major Result switch.
//! A borrowed relation only: no source text, second materialization, uniform
//! sentinel, generic predicate admission, or verifier exemption.
use super::*;
use fe2o3_kernel_analysis::{CanonicalKirSparseV1, CanonicalKirSparseValueV1};
use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;

// This borrowed graph view is also exercised using original canonical-owner
// constructors in component tests; it is not a fabricated pre-ranked owner.
struct Graph<'a, 'g> {
    inventory: &'a CanonicalKirInventoryV1<'g>,
    caller: &'a CanonicalKirFunctionRefV1<'g>,
    target: fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1,
}

const MAX_FORWARDING: usize = 32;
const MAX_SWITCH_BLOCKS: usize = 32;
const REQUIRED_ELEMENTS: u64 = 256;

/// Lexical relation to the original source switch and canonical entry length.
/// This is not a detached readiness, execution, or normal-compilation token.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CheckedBf16ViewSwitchV1;
/// fn forge() { let _ = CheckedBf16ViewSwitchV1 {}; }
/// ```
pub struct CheckedBf16ViewSwitchV1<'a> {
    source: &'a SemanticFunctionDeclV1,
    source_block: SemanticBlockIdV1,
    view_call: &'a SemanticDirectCallV1,
    parameter: u32,
    source_local: SemanticLocalIdV1,
    source_argument: u32,
    success: SemanticBlockIdV1,
    failure: SemanticBlockIdV1,
    required: u64,
}
impl CheckedBf16ViewSwitchV1<'_> {
    /// Exact retained function, not an equal source clone.
    pub const fn source(&self) -> &SemanticFunctionDeclV1 {
        self.source
    }
    /// Exact source SwitchInt block.
    pub const fn source_block(&self) -> SemanticBlockIdV1 {
        self.source_block
    }
    /// Original checked-view constructor call.
    pub const fn view_call(&self) -> &SemanticDirectCallV1 {
        self.view_call
    }
    /// Actual canonical caller entry-parameter ordinal; not a ranked argument.
    pub const fn parameter(&self) -> u32 {
        self.parameter
    }
    /// Actual source local corresponding to that complete slice parameter.
    pub const fn source_local(&self) -> SemanticLocalIdV1 {
        self.source_local
    }
    /// Actual source argument ordinal, kept distinct from both other namespaces.
    pub const fn source_argument(&self) -> u32 {
        self.source_argument
    }
    /// Source edge selected when the exact checked-view validity condition holds.
    pub const fn success(&self) -> SemanticBlockIdV1 {
        self.success
    }
    /// Source edge selected when it fails. This path is never elided.
    pub const fn failure(&self) -> SemanticBlockIdV1 {
        self.failure
    }
    /// Actual graph-proved required element count.
    pub const fn required(&self) -> u64 {
        self.required
    }
}

type QueryResult<T> = Bf16CallQueryResultV1<T>;
fn unavailable(why: &'static str) -> Bf16NominalCallQueryErrorV1 {
    Bf16NominalCallQueryErrorV1::Unavailable(why)
}
fn need(ok: bool, why: &'static str) -> QueryResult<()> {
    bf16_query_require_v1(ok, why)
}
fn add(a: usize, b: usize) -> QueryResult<usize> {
    a.checked_add(b)
        .ok_or(ArgumentResourceV1::Arithmetic.into())
}
fn local(operand: &SemanticOperandV1) -> Option<SemanticLocalIdV1> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
            if place.projections().is_empty() =>
        {
            Some(place.local())
        }
        _ => None,
    }
}

// All IDs index the already authenticated inventory. Only unique edge forwarding
// is accepted: no merge, arbitrary phi selection, memory read, or recursive walk.
fn definition(
    call: &Graph<'_, '_>,
    mut value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<usize> {
    let mut seen = [None; MAX_FORWARDING];
    for at in 0..MAX_FORWARDING {
        budget.charge_work(MAX_FORWARDING + 8)?;
        need(
            !seen[..at].contains(&Some(value)),
            "cyclic checked-view selector",
        )?;
        seen[at] = Some(value);
        let index = call
            .inventory
            .definition_index_for_value(call.caller.coordinate, value, budget)
            .map_err(bf16_query_inventory_error_v1)?
            .ok_or_else(|| unavailable("checked-view value has no original definition"))?;
        budget.charge_work(4)?;
        let row = call
            .inventory
            .definitions()
            .get(index)
            .ok_or_else(|| unavailable("checked-view definition absent"))?;
        need(
            row.value == Some(value),
            "checked-view definition value differs",
        )?;
        match row.coordinate {
            Definition::FunctionArgument { function, .. } => {
                need(
                    function == call.caller.coordinate,
                    "foreign checked-view parameter",
                )?;
                return Ok(index);
            }
            Definition::Result { operation, .. } => {
                need(
                    operation.block.function == call.caller.coordinate,
                    "foreign checked-view operation",
                )?;
                return Ok(index);
            }
            Definition::BlockArgument { block, .. } => {
                need(
                    block.function == call.caller.coordinate,
                    "foreign checked-view block",
                )?;
                let mut incoming = None;
                for binding in &call.inventory.edge_arguments()[call.caller.edge_arguments.clone()]
                {
                    budget.charge_work(4)?;
                    if binding.target_definition == index {
                        need(
                            incoming.replace(binding.value).is_none(),
                            "checked-view forwarding has multiple incoming occurrences",
                        )?;
                    }
                }
                value = incoming.ok_or_else(|| unavailable("checked-view forwarding is absent"))?;
            }
        }
    }
    Err(unavailable("checked-view forwarding depth limit"))
}
fn operation<'a>(
    call: &'a Graph<'_, '_>,
    definition: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<(
    &'a Operation,
    fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
)> {
    budget.charge_work(8)?;
    let row = call
        .inventory
        .definitions()
        .get(definition)
        .ok_or_else(|| unavailable("checked-view definition absent"))?;
    let Definition::Result { operation, result } = row.coordinate else {
        return Err(unavailable(
            "checked-view expression is not an operation result",
        ));
    };
    need(
        operation.block.function == call.caller.coordinate,
        "checked-view expression belongs to another function",
    )?;
    let block_index = add(call.caller.blocks.start, operation.block.block as usize)?;
    need(
        block_index < call.caller.blocks.end,
        "checked-view block ordinal absent",
    )?;
    let block = call
        .inventory
        .blocks()
        .get(block_index)
        .ok_or_else(|| unavailable("checked-view block missing"))?;
    need(
        block.coordinate == operation.block,
        "checked-view block coordinate differs",
    )?;
    let index = add(block.operations.start, operation.operation as usize)?;
    need(
        index < block.operations.end,
        "checked-view operation ordinal absent",
    )?;
    let op = call
        .inventory
        .operations()
        .get(index)
        .ok_or_else(|| unavailable("checked-view operation missing"))?;
    need(
        op.coordinate == operation
            && op
                .operation
                .results
                .get(result as usize)
                .is_some_and(|r| Some(r.id) == row.value && &r.ty == row.ty),
        "checked-view operation/result differs",
    )?;
    Ok((op.operation, operation))
}
fn constant(
    call: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    value: ValueId,
    expected: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<Option<u128>> {
    let index = definition(call, value, budget)?;
    budget.charge_work(4)?;
    let row = &call.inventory.definitions()[index];
    need(row.ty == expected, "checked-view constant type differs")?;
    Ok(match report.value(index) {
        Some(CanonicalKirSparseValueV1::Constant(value)) if row.ty == &Type::Scalar(value.ty()) => {
            Some(value.bits())
        }
        Some(CanonicalKirSparseValueV1::Constant(_)) | None => {
            return Err(unavailable("checked-view sparse/definition type differs"));
        }
        _ => None,
    })
}

// The generic sparse pass intentionally has no target-specific Index semantics.
// This closed query has the original admitted gfx942 target and whole global
// BF16 slice contract. Keep those facts local; do not broaden generic folding.
#[derive(Clone, Copy)]
struct ExactFact {
    value: ValueId,
    ty: ScalarType,
    bits: u64,
}
const EXACT_FACTS: usize = 64;
type ExactTable = [Option<ExactFact>; EXACT_FACTS];
// The original charged query scope covers this table plus the existing bounded
// definition resolver and fixed local headers. No recursive evaluation or heap.
const _: () = assert!(std::mem::size_of::<ExactTable>() + 1024 <= 4096);

fn exact_lookup(
    graph: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    facts: &ExactTable,
    value: ValueId,
    ty: ScalarType,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<Option<u64>> {
    let index = definition(graph, value, budget)?;
    budget.charge_work(EXACT_FACTS + 4)?;
    let row = &graph.inventory.definitions()[index];
    need(
        row.ty == &Type::Scalar(ty),
        "checked-view exact operand type differs",
    )?;
    let original = row
        .value
        .ok_or_else(|| unavailable("checked-view exact operand absent"))?;
    for fact in facts.iter().flatten() {
        if fact.value == original {
            need(fact.ty == ty, "checked-view exact fact type differs")?;
            return Ok(Some(fact.bits));
        }
    }
    let bits = constant(graph, report, value, &Type::Scalar(ty), budget)?;
    bits.map(|bits| {
        let bits = u64::try_from(bits)
            .map_err(|_| unavailable("checked-view exact constant exceeds target index"))?;
        need(
            ty != ScalarType::Bool || bits <= 1,
            "checked-view exact Boolean differs",
        )?;
        Ok(bits)
    })
    .transpose()
}

fn exact_result(
    graph: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    facts: &ExactTable,
    operation: &Operation,
    result: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<Option<u64>> {
    budget.charge_work(12)?;
    let lookup = |value, ty, budget: &mut ArgumentBudgetV1<'_>| {
        exact_lookup(graph, report, facts, value, ty, budget)
    };
    let output = &operation.results[result];
    Ok(match &operation.kind {
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value,
            to,
        } if operation.results.len() == 1 && output.ty == Type::INDEX && to == &Type::INDEX => {
            // The enclosing exact query has authenticated gfx942 and its
            // 64-bit Index contract. Only the canonical U64 -> Index bridge
            // emitted by coerce_index is supported here, not a generic cast.
            let index = definition(graph, *value, budget)?;
            budget.charge_work(4)?;
            if graph.inventory.definitions()[index].ty != &Type::Scalar(ScalarType::U64) {
                return Ok(None);
            }
            // Reuse the original sparse proof for the U64 operand. Dynamic
            // inputs remain unknown; no local target-width guess or report
            // mutation turns the resulting Index into a generic sparse fact.
            constant(
                graph,
                report,
                *value,
                &Type::Scalar(ScalarType::U64),
                budget,
            )?
            .map(|bits| {
                u64::try_from(bits)
                    .map_err(|_| unavailable("checked-view U64 bridge exceeds target index"))
            })
            .transpose()?
        }
        OperationKind::Binary {
            op: BinaryOp::Checked(operator),
            lhs,
            rhs,
        } if operation.results.len() == 2
            && operation.results[0].ty == Type::INDEX
            && operation.results[1].ty == Type::BOOL =>
        {
            let (Some(lhs), Some(rhs)) = (
                lookup(*lhs, ScalarType::Index, budget)?,
                lookup(*rhs, ScalarType::Index, budget)?,
            ) else {
                return Ok(None);
            };
            // Unsigned 64-bit arithmetic is selected by the admitted target
            // below, never host usize or a width inferred from a local pointer.
            let (value, overflow) = match operator {
                CheckedBinaryOperator::Add => lhs.overflowing_add(rhs),
                CheckedBinaryOperator::Subtract => lhs.overflowing_sub(rhs),
                CheckedBinaryOperator::Multiply => lhs.overflowing_mul(rhs),
            };
            Some(if result == 0 {
                value
            } else {
                u64::from(overflow)
            })
        }
        OperationKind::Binary {
            op: BinaryOp::BitAnd | BinaryOp::BitOr,
            lhs,
            rhs,
        } if operation.results.len() == 1 && output.ty == Type::BOOL => {
            let (Some(lhs), Some(rhs)) = (
                lookup(*lhs, ScalarType::Bool, budget)?,
                lookup(*rhs, ScalarType::Bool, budget)?,
            ) else {
                return Ok(None);
            };
            Some(
                if matches!(
                    &operation.kind,
                    OperationKind::Binary {
                        op: BinaryOp::BitAnd,
                        ..
                    }
                ) {
                    lhs & rhs
                } else {
                    lhs | rhs
                },
            )
        }
        OperationKind::Unary {
            op: UnaryOp::Not,
            operand,
        } if operation.results.len() == 1 && output.ty == Type::BOOL => {
            lookup(*operand, ScalarType::Bool, budget)?.map(|value| u64::from(value == 0))
        }
        OperationKind::Compare {
            predicate,
            lhs,
            rhs,
        } if operation.results.len() == 1 && output.ty == Type::BOOL => {
            if !matches!(
                predicate,
                ComparePredicate::Equal | ComparePredicate::LessThanOrEqual
            ) {
                return Ok(None);
            }
            let (Some(lhs), Some(rhs)) = (
                lookup(*lhs, ScalarType::Index, budget)?,
                lookup(*rhs, ScalarType::Index, budget)?,
            ) else {
                return Ok(None);
            };
            Some(u64::from(match predicate {
                ComparePredicate::Equal => lhs == rhs,
                ComparePredicate::LessThanOrEqual => lhs <= rhs,
                _ => unreachable!("closed predicates checked above"),
            }))
        }
        OperationKind::Select {
            condition,
            true_value,
            false_value,
        } if operation.results.len() == 1 && output.ty == Type::INDEX => {
            let Some(condition) = lookup(*condition, ScalarType::Bool, budget)? else {
                return Ok(None);
            };
            lookup(
                if condition == 1 {
                    *true_value
                } else {
                    *false_value
                },
                ScalarType::Index,
                budget,
            )?
        }
        _ => None,
    })
}

fn exact_constant(
    graph: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    value: ValueId,
    expected: &Type,
    span: &SemanticKirTerminatorOperationSpanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<Option<u128>> {
    budget.charge_work(4)?;
    need(
        matches!(
            graph.target.architecture(),
            fe2o3_mir_model::semantic_mir_v1::SemanticTargetArchitectureV1::AmdGpuGfx942
        ) && graph.target.object_size_bound_bytes() == 1u64 << 61,
        "checked-view exact index target differs",
    )?;
    need(
        matches!(expected, Type::Scalar(ScalarType::Index | ScalarType::Bool)),
        "checked-view exact constant type is outside the closed profile",
    )?;
    if let Some(value) = constant(graph, report, value, expected, budget)? {
        need(
            value <= u128::from(u64::MAX) && (expected != &Type::BOOL || value <= 1),
            "checked-view exact literal exceeds target type",
        )?;
        return Ok(Some(value));
    }
    let definition_index = definition(graph, value, budget)?;
    let (_, last) = operation(graph, definition_index, budget)?;
    in_span(last, span, graph, budget)?;
    // A separate original query scope prepays fixed table and resolver frames
    // and preserves sticky denial, panic handling, and the same work ledger.
    bf16_call_query_scope_v1(budget, |budget| {
        let mut facts: ExactTable = [None; EXACT_FACTS];
        let mut used = 0usize;
        let block = graph
            .inventory
            .block_for_id(graph.caller.coordinate, span.kernel_ir_block(), budget)
            .map_err(bf16_query_inventory_error_v1)?
            .ok_or_else(|| unavailable("checked-view exact span block absent"))?;
        for ordinal in span.first_operation_ordinal()..=last.operation {
            budget.charge_work(8)?;
            let at = add(block.operations.start, ordinal as usize)?;
            need(
                at < block.operations.end,
                "checked-view exact operation absent",
            )?;
            let row = &graph.inventory.operations()[at];
            need(
                row.coordinate.block == last.block && row.coordinate.operation == ordinal,
                "checked-view exact operation coordinate differs",
            )?;
            for (result, output) in row.operation.results.iter().enumerate() {
                budget.charge_work(4)?;
                let ty = match output.ty {
                    Type::Scalar(ScalarType::Index) => ScalarType::Index,
                    Type::Scalar(ScalarType::Bool) => ScalarType::Bool,
                    _ => continue,
                };
                if let Some(bits) =
                    exact_result(graph, report, &facts, row.operation, result, budget)?
                {
                    need(used < EXACT_FACTS, "checked-view exact constant fact limit")?;
                    facts[used] = Some(ExactFact {
                        value: output.id,
                        ty,
                        bits,
                    });
                    used += 1;
                }
            }
        }
        let Type::Scalar(ty) = expected else {
            unreachable!("closed type checked above")
        };
        exact_lookup(graph, report, &facts, value, *ty, budget).map(|bits| bits.map(u128::from))
    })
}

fn block_id(
    call: &CheckedBf16NominalCallV1<'_>,
    source: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<BlockId> {
    let mut result = None;
    let owner = call.emission.owner;
    let root = call.emission.relation.root;
    for row in &owner.correspondence.blocks {
        budget.charge_work(2)?;
        if row.correspondence_owner == root
            && row.semantic_function == root
            && row.semantic_block == source
        {
            need(
                result.replace(row.kernel_ir_block).is_none(),
                "duplicate checked-view block correspondence",
            )?;
        }
    }
    result.ok_or_else(|| unavailable("checked-view block correspondence absent"))
}
fn in_span(
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    span: &SemanticKirTerminatorOperationSpanV1,
    call: &Graph<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<()> {
    let block = call
        .inventory
        .block_for_id(call.caller.coordinate, span.kernel_ir_block(), budget)
        .map_err(bf16_query_inventory_error_v1)?
        .ok_or_else(|| unavailable("checked-view source span block absent"))?;
    budget.charge_work(4)?;
    let end = span
        .first_operation_ordinal()
        .checked_add(span.operation_count())
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    need(
        coordinate.block == block.coordinate
            && coordinate.operation >= span.first_operation_ordinal()
            && coordinate.operation < end,
        "checked-view expression leaves its original source span",
    )
}

// Walk the actual required-element expression emitted by the checked-view
// intrinsic. Constants are sparse facts from this exact inventory, never
// values inferred from source spelling or assumed geometry.
fn checked_operands(
    graph: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    value: ValueId,
    operator: CheckedBinaryOperator,
    span: &SemanticKirTerminatorOperationSpanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<(ValueId, ValueId)> {
    let index = definition(graph, value, budget)?;
    let (op, coordinate) = operation(graph, index, budget)?;
    in_span(coordinate, span, graph, budget)?;
    budget.charge_work(8)?;
    need(
        graph.inventory.definitions()[index].ty == &Type::INDEX
            && op.results.len() == 2
            && op.results[0].id == value
            && op.results[1].ty == Type::BOOL,
        "checked-view geometry is not the checked value result",
    )?;
    let OperationKind::Binary {
        op: BinaryOp::Checked(actual),
        lhs,
        rhs,
    } = &op.kind
    else {
        return Err(unavailable(
            "checked-view geometry has unchecked arithmetic",
        ));
    };
    need(
        *actual == operator,
        "checked-view geometry arithmetic differs",
    )?;
    need(
        exact_constant(graph, report, op.results[1].id, &Type::BOOL, span, budget)? == Some(0),
        "checked-view geometry overflow is not proved absent",
    )?;
    Ok((*lhs, *rhs))
}
fn geometry(
    graph: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    required: ValueId,
    span: &SemanticKirTerminatorOperationSpanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<()> {
    let index = definition(graph, required, budget)?;
    let (op, coordinate) = operation(graph, index, budget)?;
    in_span(coordinate, span, graph, budget)?;
    let OperationKind::Select {
        condition: empty,
        true_value: offset,
        false_value: nonempty,
    } = &op.kind
    else {
        return Err(unavailable(
            "checked-view required extent lacks its original empty selection",
        ));
    };
    need(
        exact_constant(graph, report, *empty, &Type::BOOL, span, budget)? == Some(0)
            && exact_constant(graph, report, *offset, &Type::INDEX, span, budget)? == Some(0),
        "checked-view geometry is empty or has a different offset",
    )?;
    let (actual_offset, matrix) = checked_operands(
        graph,
        report,
        *nonempty,
        CheckedBinaryOperator::Add,
        span,
        budget,
    )?;
    need(
        definition(graph, actual_offset, budget)? == definition(graph, *offset, budget)?,
        "checked-view required paths have different original offsets",
    )?;
    let (row_extent, columns) = checked_operands(
        graph,
        report,
        matrix,
        CheckedBinaryOperator::Add,
        span,
        budget,
    )?;
    let (rows_minus_one, stride) = checked_operands(
        graph,
        report,
        row_extent,
        CheckedBinaryOperator::Multiply,
        span,
        budget,
    )?;
    let (rows, one) = checked_operands(
        graph,
        report,
        rows_minus_one,
        CheckedBinaryOperator::Subtract,
        span,
        budget,
    )?;
    need(
        exact_constant(graph, report, rows, &Type::INDEX, span, budget)? == Some(16)
            && exact_constant(graph, report, columns, &Type::INDEX, span, budget)? == Some(16)
            && exact_constant(graph, report, stride, &Type::INDEX, span, budget)? == Some(16)
            && exact_constant(graph, report, one, &Type::INDEX, span, budget)? == Some(1),
        "checked-view original geometry leaves the closed 16x16 row-major profile",
    )
}

// Removes only proven true conjuncts from an actual Boolean-and expression.
// The remaining comparison is still a real dynamic condition, not Uniform(true).
fn condition(
    call: &Graph<'_, '_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    mut value: ValueId,
    span: &SemanticKirTerminatorOperationSpanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<(ValueId, u64)> {
    for _ in 0..8 {
        let index = definition(call, value, budget)?;
        budget.charge_work(2)?;
        need(
            call.inventory.definitions()[index].ty == &Type::BOOL,
            "checked-view validity is not Boolean",
        )?;
        let (op, coordinate) = operation(call, index, budget)?;
        in_span(coordinate, span, call, budget)?;
        match &op.kind {
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs,
                rhs,
            } => {
                if exact_constant(call, report, *lhs, &Type::BOOL, span, budget)? == Some(1) {
                    value = *rhs;
                } else if exact_constant(call, report, *rhs, &Type::BOOL, span, budget)? == Some(1)
                {
                    value = *lhs;
                } else {
                    return Err(unavailable(
                        "checked-view safety conjunction is not proved true",
                    ));
                }
            }
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs,
                rhs,
            } => {
                geometry(call, report, *lhs, span, budget)?;
                need(
                    exact_constant(call, report, *lhs, &Type::INDEX, span, budget)?
                        == Some(REQUIRED_ELEMENTS as u128),
                    "checked-view required extent is not the closed graph constant",
                )?;
                let length = definition(call, *rhs, budget)?;
                let (length_op, length_coordinate) = operation(call, length, budget)?;
                in_span(length_coordinate, span, call, budget)?;
                let OperationKind::SliceLength { slice } = &length_op.kind else {
                    return Err(unavailable(
                        "checked-view bound is not its actual slice length",
                    ));
                };
                return Ok((*slice, REQUIRED_ELEMENTS));
            }
            _ => return Err(unavailable("unsupported checked-view validity expression")),
        }
    }
    Err(unavailable("checked-view validity depth limit"))
}

fn result_discriminants(
    variants: &[SemanticEnumVariantV1],
    view: SemanticTypeIdV1,
    error: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<(u128, u128)> {
    budget.charge_work(32)?;
    need(
        variants.len() == 2 && view != error,
        "checked-view Result does not have two distinct payload alternatives",
    )?;
    let mut ok = None;
    let mut err = None;
    for variant in variants {
        need(
            !variant.is_uninhabited(),
            "checked-view Result has an uninhabited alternative",
        )?;
        if variant.fields().fields() == [view] {
            need(
                ok.replace(variant.discriminant()).is_none(),
                "duplicate checked-view Ok alternative",
            )?;
        } else if variant.fields().fields() == [error] {
            need(
                err.replace(variant.discriminant()).is_none(),
                "duplicate checked-view Err alternative",
            )?;
        } else {
            return Err(unavailable(
                "checked-view Result has an unrelated payload alternative",
            ));
        }
    }
    let ok = ok.ok_or_else(|| unavailable("checked-view Ok alternative is absent"))?;
    let err = err.ok_or_else(|| unavailable("checked-view Err alternative is absent"))?;
    need(ok != err, "checked-view Result discriminants coincide")?;
    Ok((ok, err))
}
fn source_target(
    targets: &fe2o3_mir_model::semantic_mir_v1::SemanticSwitchTargetsV1,
    value: u128,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<SemanticBlockIdV1> {
    budget.charge_work(8)?;
    need(
        (1..=2).contains(&targets.values().len())
            && targets.otherwise().role() == SemanticEdgeRoleV1::SwitchOtherwise,
        "checked-view switch default or case census differs",
    )?;
    let mut found = None;
    for target in targets.values() {
        budget.charge_work(4)?;
        need(
            target.edge().role() == SemanticEdgeRoleV1::SwitchValue,
            "checked-view switch case has a different edge role",
        )?;
        if target.value() == value {
            need(
                found.replace(target.edge().target()).is_none(),
                "duplicate checked-view switch case",
            )?;
        }
    }
    Ok(found.unwrap_or(targets.otherwise().target()))
}

fn derive<'a>(
    call: &'a CheckedBf16NominalCallV1<'_>,
    report: &CanonicalKirSparseV1<'_, '_>,
    source_block: SemanticBlockIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> QueryResult<Option<CheckedBf16ViewSwitchV1<'a>>> {
    budget.charge_work(32)?;
    let graph = Graph {
        inventory: call.inventory,
        caller: call.caller,
        target: call.emission.owner.semantic_ssa.source_semantic().target(),
    };
    need(
        report.belongs_to(call.inventory),
        "foreign checked-view sparse report",
    )?;
    let floor = bf16_nominal_retained_floor_v1(call.emission.owner, call.inventory, budget)?;
    let report_storage = report
        .retained_storage_v1(budget)
        .map_err(|error| match error {
            fe2o3_kernel_analysis::CanonicalKirSparseErrorV1::Resource(error) => {
                Bf16NominalCallQueryErrorV1::Resource(error)
            }
            _ => unavailable("checked-view sparse storage differs"),
        })?;
    if budget.storage() < add(floor, report_storage)? {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let owner = call.emission.owner;
    let root = call.emission.relation.root;
    let source = owner
        .semantic_ssa
        .source_semantic()
        .functions()
        .get(root.index() as usize)
        .ok_or_else(|| unavailable("checked-view original source root absent"))?;
    need(
        source.blocks().len() <= MAX_SWITCH_BLOCKS,
        "checked-view source block limit",
    )?;
    let block = source
        .blocks()
        .get(source_block.index() as usize)
        .ok_or_else(|| unavailable("checked-view source block absent"))?;
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = block.terminator().kind()
    else {
        return Ok(None);
    };
    // Find the actual provider-bound view call whose original return reaches
    // this discriminant block. Unrelated switches keep conservative projection.
    let mut selected = None;
    for (index, predecessor) in source.blocks().iter().enumerate() {
        budget.charge_work(8)?;
        let SemanticTerminatorKindV1::Call(view) = predecessor.terminator().kind() else {
            continue;
        };
        let Some(destination) = view.destination() else {
            continue;
        };
        if destination.edge().target() != source_block {
            continue;
        }
        if let Some(SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::Bf16MatrixViewRowMajor {
                    result,
                    view: view_type,
                    error,
                    ..
                },
            ..
        }) = owner
            .semantic_ssa
            .source_semantic()
            .callables()
            .get(view.callee().index() as usize)
        {
            need(
                selected
                    .replace((index, view, *result, *view_type, *error))
                    .is_none(),
                "ambiguous checked-view predecessor",
            )?;
        }
    }
    let Some((view_index, view_call, result_type, view_type, error_type)) = selected else {
        return Ok(None);
    };
    budget.charge_work(8)?;
    let result_shape = owner
        .semantic_ssa
        .source_semantic()
        .types()
        .get(result_type.index() as usize)
        .ok_or_else(|| unavailable("checked-view Result source type is absent"))?;
    let SemanticTypeShapeV1::Enum { variants, .. } = result_shape.shape() else {
        return Err(unavailable(
            "checked-view Result source type is not an enum",
        ));
    };
    let source_discriminants = result_discriminants(variants, view_type, error_type, budget)?;
    let destination = view_call
        .destination()
        .ok_or_else(|| unavailable("view destination absent"))?;
    need(
        destination.place().projections().is_empty()
            && destination.edge().role() == SemanticEdgeRoleV1::CallReturn
            && view_call.arguments().len() == 5,
        "checked-view source destination shape",
    )?;
    let discriminant_local = local(discriminant)
        .ok_or_else(|| unavailable("checked-view switch is not a whole local"))?;
    let mut assignments = 0usize;
    for statement in block.statements() {
        budget.charge_work(8)?;
        match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                need(
                    assignment.destination().projections().is_empty()
                        && assignment.destination().local() == discriminant_local,
                    "checked-view switch has an unrelated assignment",
                )?;
                let SemanticRvalueKindV1::Discriminant(place) = assignment.value().kind() else {
                    return Err(unavailable(
                        "checked-view switch lacks original Result discriminant",
                    ));
                };
                need(
                    place.projections().is_empty() && place.local() == destination.place().local(),
                    "checked-view discriminant uses another Result",
                )?;
                assignments += 1;
            }
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => {}
            _ => return Err(unavailable("checked-view discriminant block has effects")),
        }
    }
    need(
        assignments == 1,
        "checked-view discriminant assignment is not unique",
    )?;
    let source_id = block_id(call, source_block, budget)?;
    let canonical = call
        .inventory
        .block_for_id(call.caller.coordinate, source_id, budget)
        .map_err(bf16_query_inventory_error_v1)?
        .ok_or_else(|| unavailable("checked-view canonical switch absent"))?;
    let Terminator::Switch {
        selector,
        cases,
        default_target,
        default_arguments,
    } = canonical.terminator
    else {
        return Err(unavailable(
            "checked-view canonical terminator is not Switch",
        ));
    };
    budget.charge_work(8)?;
    need(
        cases.len() == targets.values().len() && cases.len() <= 2 && !cases.is_empty(),
        "checked-view source/canonical case census differs",
    )?;
    // Compare each actual ordered case and default to original source targets.
    // Inventory edge payloads are the same borrowed canonical values, not a
    // guessed source SSA roster or a cloned replacement.
    for (case, source_case) in cases.iter().zip(targets.values()) {
        budget.charge_work(4)?;
        need(
            case.value as u128 == source_case.value()
                && case.target == block_id(call, source_case.edge().target(), budget)?,
            "checked-view case value or target differs",
        )?;
    }
    need(
        *default_target == block_id(call, targets.otherwise().target(), budget)?,
        "checked-view default target differs",
    )?;
    budget.charge_work(4)?;
    need(
        canonical.edges.len() == cases.len() + 1,
        "checked-view edge census differs",
    )?;
    for (ordinal, edge) in call.inventory.edges()[canonical.edges.clone()]
        .iter()
        .enumerate()
    {
        budget.charge_work(8)?;
        let (target, arguments) = if let Some(case) = cases.get(ordinal) {
            (case.target, case.arguments.as_slice())
        } else {
            (*default_target, default_arguments.as_slice())
        };
        need(
            edge.target_id == target && edge.arguments.len() == arguments.len(),
            "checked-view ordered edge differs",
        )?;
        budget.charge_work(arguments.len())?;
        need(
            edge.arguments == arguments,
            "checked-view edge argument occurrence differs",
        )?;
    }
    let selector_definition = definition(&graph, *selector, budget)?;
    let (selector_op, selector_coordinate) = operation(&graph, selector_definition, budget)?;
    let span = bf16_source_span_v1(
        &owner.correspondence,
        root,
        root,
        SemanticBlockIdV1::from_index(view_index as u32),
        budget,
    )
    .map_err(bf16_query_semantic_error_v1)?;
    in_span(selector_coordinate, span, &graph, budget)?;
    let OperationKind::Select {
        condition: valid,
        true_value,
        false_value,
    } = &selector_op.kind
    else {
        return Err(unavailable(
            "checked-view Result selector is not the original Select",
        ));
    };
    budget.charge_work(2)?;
    let selector_type = call.inventory.definitions()[selector_definition].ty;
    let ok = constant(&graph, report, *true_value, selector_type, budget)?
        .ok_or_else(|| unavailable("checked-view Ok discriminant is not constant"))?;
    let err = constant(&graph, report, *false_value, selector_type, budget)?
        .ok_or_else(|| unavailable("checked-view Err discriminant is not constant"))?;
    need(
        (ok, err) == source_discriminants,
        "checked-view Select alternatives differ from actual Result payload discriminants",
    )?;
    let success = source_target(targets, ok, budget)?;
    let failure = source_target(targets, err, budget)?;
    need(
        success != failure,
        "checked-view success and failure edges coincide",
    )?;
    let (slice, required) = condition(&graph, report, *valid, span, budget)?;
    let parameter_definition = definition(&graph, slice, budget)?;
    budget.charge_work(8)?;
    let parameter_row = &call.inventory.definitions()[parameter_definition];
    let Definition::FunctionArgument { function, argument } = parameter_row.coordinate else {
        return Err(unavailable(
            "checked-view length does not belong to an entry parameter",
        ));
    };
    need(
        function == call.caller.coordinate,
        "checked-view length has a foreign parameter",
    )?;
    let Type::Slice(slice_type) = parameter_row.ty else {
        return Err(unavailable("checked-view length parameter is not a slice"));
    };
    need(
        slice_type.address_space == AddressSpace::Global
            && slice_type.access == AccessMode::ReadOnly
            && *slice_type.element == Type::Scalar(ScalarType::U16),
        "checked-view length parameter is not read-only global BF16 storage",
    )?;
    let parameter_value = parameter_row
        .value
        .ok_or_else(|| unavailable("checked-view parameter has no value"))?;
    let mut source_local = None;
    for binding in owner.correspondence.parameter_bindings() {
        budget.charge_work(4)?;
        if binding.correspondence_owner() == root
            && binding.semantic_function() == root
            && binding.kernel_ir_value() == parameter_value
        {
            need(
                source_local.replace(binding.semantic_local()).is_none(),
                "checked-view parameter has duplicate source bindings",
            )?;
        }
    }
    for binding in owner.correspondence.parameter_component_bindings() {
        budget.charge_work(4)?;
        need(
            !(binding.correspondence_owner() == root
                && binding.semantic_function() == root
                && binding.kernel_ir_value() == parameter_value),
            "checked-view slice is not a whole source parameter",
        )?;
    }
    let source_local =
        source_local.ok_or_else(|| unavailable("checked-view parameter source absent"))?;
    need(
        local(&view_call.arguments()[0]) == Some(source_local),
        "checked-view constructor slice differs from canonical parameter",
    )?;
    budget.charge_work(4)?;
    let declaration = source
        .locals()
        .get(source_local.index() as usize)
        .ok_or_else(|| unavailable("checked-view source parameter local absent"))?;
    let SemanticLocalRoleV1::Argument(source_argument) = declaration.role() else {
        return Err(unavailable(
            "checked-view parameter source is not an argument",
        ));
    };
    need(
        call.caller
            .function
            .body
            .as_ref()
            .and_then(|body| body.parameters.get(argument as usize))
            == Some(&parameter_value),
        "checked-view argument ordinal differs",
    )?;
    Ok(Some(CheckedBf16ViewSwitchV1 {
        source,
        source_block,
        view_call,
        parameter: argument,
        source_local,
        source_argument,
        success,
        failure,
        required,
    }))
}

impl ProductionPreRankedKirOwnerV1 {
    /// Proves only the supported actual checked-view selector under the same
    /// original nominal-call owner/inventory/sparse report and cumulative budget.
    /// None means no directly preceding row-major view call; it is not uniform.
    /// Unsupported or ambiguous checked-view branches refuse. No graph/source
    /// clone, second materialization, normal admission or artifact is created.
    #[allow(clippy::too_many_arguments)]
    pub fn with_checked_bf16_view_switch_v1<'w, R: Copy + 'static>(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        report: &CanonicalKirSparseV1<'_, '_>,
        root: SemanticFunctionIdV1,
        call_block: SemanticBlockIdV1,
        source_call: &SemanticDirectCallV1,
        switch_block: SemanticBlockIdV1,
        budget: &mut ArgumentBudgetV1<'w>,
        inspect: impl for<'s> FnOnce(
            &CheckedBf16ViewSwitchV1<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Bf16CallQueryResultV1<R>,
    ) -> Bf16CallQueryResultV1<Option<R>> {
        budget.charge_work(8)?;
        // Check the incoming floor before either query adds scratch. Scratch
        // cannot stand in for an unpaid original sparse-report reservation.
        need(
            report.belongs_to(inventory),
            "foreign checked-view sparse report",
        )?;
        let owner_floor = bf16_nominal_retained_floor_v1(self, inventory, budget)?;
        let report_floor = report
            .retained_storage_v1(budget)
            .map_err(|error| match error {
                fe2o3_kernel_analysis::CanonicalKirSparseErrorV1::Resource(error) => {
                    Bf16NominalCallQueryErrorV1::Resource(error)
                }
                _ => unavailable("checked-view sparse storage differs"),
            })?;
        if budget.storage() < add(owner_floor, report_floor)? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        // A second fixed scope admits the bounded forwarding array and helper
        // frames before entry; both scopes share the original ledger. They
        // refund only their own fixed scratch after borrowed payloads drop.
        bf16_call_query_scope_v1(budget, |budget| {
            self.with_checked_bf16_nominal_call_v1(
                inventory,
                root,
                root,
                call_block,
                source_call,
                budget,
                |call, budget| match derive(call, report, switch_block, budget)? {
                    Some(view) => inspect(&view, budget).map(Some),
                    None => Ok(None),
                },
            )
        })
    }
}

#[cfg(test)]
#[path = "production_bf16_checked_view_switch_v1_tests.rs"]
mod tests;

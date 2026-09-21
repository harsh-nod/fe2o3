//! Exact observations of genuine source-owned graphs; no constructed graph fallback.
use super::*;
use fe2o3_kernel_analysis::{ControlFlowAnalysis, analyze_control_flow};
use fe2o3_lower_mir_kernel::{ProductionSemanticKirOwnerV1, SemanticKirAssertConditionOutcomeV1};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssertMessageV1, SemanticBinaryOpV1, SemanticBlockIdV1, SemanticEdgeRoleV1,
    SemanticOperandV1, SemanticRvalueKindV1, SemanticStatementKindV1, SemanticTerminatorKindV1,
};
use kir::{
    BasicBlock, BlockId, ComparePredicate, FormalAccessDomainV1, FormalGuardedPathV1,
    FormalMemoryAccessKind, FormalMemoryObligations, FormalRuntimeSliceReadDomainV1, Function,
    Module, Operation, OperationKind, Terminator, Type, ValueId,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadRow {
    site: [u32; 2],
    index: u32,
    guard_index: u32,
    phi: u32,
    header: u32,
    update: u32,
    guard: [u32; 3],
    slice: u32,
    length: u32,
    predicate: u32,
    rejected_other_index_guards: Vec<u32>,
    source_assertions: Vec<[u32; 4]>,
    source_branches: Vec<[u32; 4]>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphRows {
    root: String,
    loops: usize,
    stores: usize,
    reads: Vec<ReadRow>,
    elided_bounds_assertions: usize,
}

fn block(function: &Function, id: BlockId) -> Result<&BasicBlock> {
    function
        .body
        .as_ref()
        .and_then(|body| body.blocks.iter().find(|b| b.id == id))
        .ok_or_else(|| mismatch(Mismatch::Roster, ("actual block", id)))
}
fn operation(function: &Function, value: ValueId) -> Result<&Operation> {
    function
        .body
        .as_ref()
        .into_iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
        .find(|op| op.results.iter().any(|r| r.id == value))
        .ok_or_else(|| mismatch(Mismatch::Roster, ("actual definition", value)))
}

fn value_type(function: &Function, value: ValueId) -> Result<&Type> {
    let body = function.body.as_ref().unwrap();
    body.parameters
        .iter()
        .zip(&function.signature.parameters)
        .find_map(|(id, ty)| (*id == value).then_some(ty))
        .or_else(|| {
            body.blocks
                .iter()
                .flat_map(|b| &b.parameters)
                .chain(
                    body.blocks
                        .iter()
                        .flat_map(|b| &b.operations)
                        .flat_map(|o| &o.results),
                )
                .find(|r| r.id == value)
                .map(|r| &r.ty)
        })
        .ok_or_else(|| mismatch(Mismatch::Roster, ("actual value type", value)))
}

// Independent observer walk: actual edge occurrences and cast payloads, not
// the production guard cache or a spelling-based usize assumption.
fn representation(function: &Function, value: ValueId) -> Result<ValueId> {
    let body = function.body.as_ref().unwrap();
    let flow = kir::analyze_control_flow(function).map_err(|e| mismatch(Mismatch::Phi, e))?;
    let mut seen = std::collections::BTreeSet::new();
    let mut current = value;
    loop {
        if !seen.insert(current) {
            return Err(mismatch(Mismatch::Phi, "cyclic representation transport"));
        }
        let ty = value_type(function, current)?;
        if !matches!(
            ty,
            Type::Scalar(kir::ScalarType::Index | kir::ScalarType::U64)
        ) {
            return Err(mismatch(
                Mismatch::GuardIndex,
                ("unsupported representation type", ty),
            ));
        }
        if let Some((block, ordinal)) = body.blocks.iter().find_map(|block| {
            block
                .parameters
                .iter()
                .position(|p| p.id == current)
                .map(|i| (block, i))
        }) {
            if block.id == body.blocks[0].id {
                return Ok(current);
            }
            let edges = flow
                .incoming_edges(block.id)
                .ok_or_else(|| mismatch(Mismatch::Phi, block.id))?;
            let mut same = None;
            for edge in edges {
                if flow
                    .edge_source(*edge)
                    .is_none_or(|source| !flow.is_reachable(source))
                {
                    continue;
                }
                let input = *flow
                    .edge_arguments(function, *edge)
                    .get(ordinal)
                    .ok_or_else(|| mismatch(Mismatch::Phi, (block.id, ordinal)))?;
                if value_type(function, input)? != ty {
                    return Err(mismatch(Mismatch::Phi, "transport type changed"));
                }
                if input == current {
                    continue;
                }
                if same.is_some_and(|prior| prior != input) {
                    return Ok(current);
                }
                same = Some(input);
            }
            if let Some(input) = same {
                current = input;
                continue;
            }
            return Ok(current);
        }
        let Ok(op) = operation(function, current) else {
            return Ok(current);
        };
        let OperationKind::Cast {
            kind: kir::CastKind::Bitcast,
            value: input,
            to,
        } = &op.kind
        else {
            return Ok(current);
        };
        if !matches!(op.results.as_slice(), [r] if r.id == current && &r.ty == to) {
            return Err(mismatch(Mismatch::GuardIndex, "cast result identity/type"));
        }
        if !matches!(
            (value_type(function, *input)?, to),
            (
                Type::Scalar(kir::ScalarType::U64),
                Type::Scalar(kir::ScalarType::Index)
            ) | (
                Type::Scalar(kir::ScalarType::Index),
                Type::Scalar(kir::ScalarType::U64)
            )
        ) {
            return Ok(current);
        }
        current = *input;
    }
}

fn compare_matches(
    function: &Function,
    domain: FormalRuntimeSliceReadDomainV1,
    compare: &Operation,
) -> Result<()> {
    let OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs,
        rhs,
    } = compare.kind
    else {
        return Err(mismatch(
            Mismatch::GuardEdge,
            "strict unsigned read comparison",
        ));
    };
    if lhs != domain.guard_index()
        || value_type(function, lhs)? != &Type::INDEX
        || value_type(function, domain.index())? != &Type::INDEX
        || representation(function, lhs)? != representation(function, domain.index())?
    {
        return Err(mismatch(
            Mismatch::GuardIndex,
            (lhs, domain.index(), domain.guard_index()),
        ));
    }
    if rhs != domain.length()
        || value_type(function, rhs)? != &Type::INDEX
        || !matches!(operation(function, representation(function, rhs)?)?.kind,
        OperationKind::SliceLength { slice } if slice == domain.slice())
    {
        return Err(mismatch(
            Mismatch::GuardLength,
            (rhs, domain.length(), domain.slice()),
        ));
    }
    Ok(())
}

fn edge_success(
    function: &Function,
    control: &ControlFlowAnalysis,
    source: BlockId,
    ordinal: usize,
    target: BlockId,
    read: BlockId,
) -> Result<()> {
    let actual = block(function, source)?
        .terminator
        .as_ref()
        .ok_or_else(|| mismatch(Mismatch::GuardEdge, source))?
        .successors();
    let incoming = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|b| b.terminator.iter().flat_map(Terminator::successors))
        .filter(|candidate| *candidate == target)
        .count();
    if actual.get(ordinal) != Some(&target) || incoming != 1 || !control.dominates(target, read) {
        return Err(mismatch(
            Mismatch::GuardEdge,
            (source, ordinal, target, read, incoming),
        ));
    }
    Ok(())
}

fn changing_phi(
    function: &Function,
    control: &ControlFlowAnalysis,
    index: ValueId,
    read: BlockId,
    guard: BlockId,
) -> Result<(BlockId, ValueId)> {
    let (header, argument) = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find_map(|block| {
            block
                .parameters
                .iter()
                .position(|parameter| {
                    parameter.id == index
                        && matches!(
                            parameter.ty,
                            Type::Scalar(kir::ScalarType::Index | kir::ScalarType::U64)
                        )
                })
                .map(|argument| (block, argument))
        })
        .ok_or_else(|| {
            mismatch(
                Mismatch::Phi,
                ("exact underlying unsigned block parameter", index),
            )
        })?;
    let body = control
        .natural_loop_body(header.id)
        .ok_or_else(|| mismatch(Mismatch::Phi, ("actual natural loop", header.id)))?;
    if !body.contains(&read) || !body.contains(&guard) {
        return Err(mismatch(
            Mismatch::Phi,
            (
                "fresh guard and read inside this loop",
                header.id,
                guard,
                read,
            ),
        ));
    }
    let latches = control
        .natural_loop_latches(header.id)
        .ok_or_else(|| mismatch(Mismatch::Update, header.id))?;
    if latches.len() != 1 {
        return Err(mismatch(Mismatch::Update, latches));
    }
    let latch = block(function, *latches.first().unwrap())?;
    let Some(Terminator::Branch { target, arguments }) = &latch.terminator else {
        return Err(mismatch(Mismatch::Update, "actual unconditional latch"));
    };
    let update = *arguments
        .get(argument)
        .ok_or_else(|| mismatch(Mismatch::Update, argument))?;
    if *target != header.id || update == index {
        return Err(mismatch(Mismatch::Update, (target, update, index)));
    }
    let addition = operation(function, update)?;
    let OperationKind::Binary {
        op: kir::BinaryOp::Checked(kir::CheckedBinaryOperator::Add),
        lhs,
        rhs,
    } = addition.kind
    else {
        return Err(mismatch(Mismatch::Update, &addition.kind));
    };
    if lhs != index
        || addition.results.first().map(|r| r.id) != Some(update)
        || addition.results.first().map(|r| &r.ty) != Some(&header.parameters[argument].ty)
        || value_type(function, rhs)? != &header.parameters[argument].ty
        || !matches!(
            (
                &header.parameters[argument].ty,
                &operation(function, rhs)?.kind
            ),
            (
                Type::Scalar(kir::ScalarType::Index),
                OperationKind::Constant(kir::Constant::Index(1))
            ) | (
                Type::Scalar(kir::ScalarType::U64),
                OperationKind::Constant(kir::Constant::U64(1))
            )
        )
    {
        return Err(mismatch(Mismatch::Update, (index, lhs, rhs, update)));
    }
    Ok((header.id, update))
}

pub(super) fn graph_rows(
    case: Case,
    module: &Module,
    reports: &[FormalMemoryObligations],
) -> Result<GraphRows> {
    let [kernel] = module.kernels.as_slice() else {
        return Err(mismatch(Mismatch::Roster, "exactly one actual kernel"));
    };
    let [report] = reports else {
        return Err(mismatch(Mismatch::Roster, "one final report"));
    };
    if kernel.id.as_str() != case.roots()[0]
        || report.kernel() != &kernel.id
        || report.entry() != &kernel.entry
    {
        return Err(mismatch(
            Mismatch::Roster,
            (kernel, report.kernel(), report.entry()),
        ));
    }
    let function = module
        .functions
        .iter()
        .find(|f| f.id == kernel.entry)
        .ok_or_else(|| mismatch(Mismatch::Roster, &kernel.entry))?;
    let control = analyze_control_flow(function).map_err(|e| mismatch(Mismatch::Phi, e))?;
    let mut result = GraphRows {
        root: kernel.id.as_str().to_owned(),
        loops: control.natural_loop_headers().len(),
        stores: 0,
        reads: Vec::new(),
        elided_bounds_assertions: 0,
    };
    for access in report.accesses() {
        if access.address_space() != kir::AddressSpace::Global {
            continue;
        }
        if access.kind() == FormalMemoryAccessKind::Write {
            result.stores += 1;
            continue;
        }
        let FormalAccessDomainV1::RuntimeSliceReadBounded(domain) = access.domain() else {
            return Err(mismatch(Mismatch::Load, access));
        };
        let site = access.location();
        let load = block(function, site.block)?
            .operations
            .get(site.operation_index)
            .ok_or_else(|| mismatch(Mismatch::Load, site))?;
        if access.kind() != FormalMemoryAccessKind::Read
            || !matches!(load.kind,
            OperationKind::Load { pointer, .. } if pointer == domain.pointer())
        {
            return Err(mismatch(Mismatch::Load, (site, load)));
        }
        if !matches!(operation(function, domain.pointer())?.kind,
            OperationKind::GetElementPointer { offset, .. } if offset == domain.index())
        {
            return Err(mismatch(Mismatch::Pointer, domain));
        }
        compare_matches(function, domain, operation(function, domain.predicate())?)?;
        let FormalGuardedPathV1::TrueEdge {
            source,
            ordinal,
            target,
        } = domain.path()
        else {
            return Err(mismatch(Mismatch::GuardEdge, domain.path()));
        };
        edge_success(function, &control, source, ordinal, target, site.block)?;
        let phi = representation(function, domain.index())?;
        let (header, update) = changing_phi(function, &control, phi, site.block, source)?;
        let mut rejected = Vec::new();
        for candidate in function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|b| &b.operations)
        {
            if let OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            } = candidate.kind
                && value_type(function, lhs)? == &Type::INDEX
                && representation(function, lhs)? != phi
                && matches!(operation(function, representation(function, rhs)?).map(|op| &op.kind),
                    Ok(OperationKind::SliceLength { slice }) if *slice == domain.slice())
            {
                let error = compare_matches(function, domain, candidate).unwrap_err();
                if error.mismatch != Some(Mismatch::GuardIndex) {
                    return Err(mismatch(Mismatch::NegativeGuard, error));
                }
                rejected.push(candidate.results[0].id.0);
            }
        }
        result.reads.push(ReadRow {
            site: [site.block.0, site.operation_index as u32],
            index: domain.index().0,
            guard_index: domain.guard_index().0,
            phi: phi.0,
            header: header.0,
            update: update.0,
            guard: [source.0, ordinal as u32, target.0],
            slice: domain.slice().0,
            length: domain.length().0,
            predicate: domain.predicate().0,
            rejected_other_index_guards: rejected,
            source_assertions: Vec::new(),
            source_branches: Vec::new(),
        });
    }
    if case == Case::Control {
        if result.loops != 0 || !result.reads.is_empty() || result.stores == 0 {
            return Err(mismatch(Mismatch::Incomplete, &result));
        }
    } else if result.loops == 0 || result.reads.len() != 1 || result.stores == 0 {
        return Err(mismatch(Mismatch::Incomplete, &result));
    }
    Ok(result)
}

pub(super) fn source_rows(
    case: Case,
    source: &ProductionSemanticKirOwnerV1,
    floor: usize,
) -> Result<(GraphRows, usize)> {
    let pre = source
        .pre_ranked_executable()
        .ok_or_else(|| mismatch(Mismatch::Owner, "pre-ranked owner"))?;
    let origins = source
        .pre_ranked_assert_origins()
        .ok_or_else(|| mismatch(Mismatch::Owner, "source assertions"))?;
    if !std::ptr::eq(origins.executable(), pre) {
        return Err(mismatch(Mismatch::Owner, "origin graph"));
    }
    let [kernel] = pre.module().kernels.as_slice() else {
        return Err(mismatch(Mismatch::Roster, "source kernel"));
    };
    // This is a descriptive 64-invocation analysis, not authenticated launch authority.
    let analysis = kir::derive_kernel_memory_obligations_from_verified(
        pre.verified_module_ref_v1(),
        &kernel.id,
        kir::ExplicitLaunchExtent1d::Exact(64),
        kir::FormalIndexWidth::Bits64,
    )
    .map_err(|e| mismatch(Mismatch::Incomplete, e))?;
    if !analysis.is_complete() {
        return Err(mismatch(
            Mismatch::Incomplete,
            analysis.incomplete_reasons(),
        ));
    }
    let mut result = graph_rows(
        case,
        pre.module(),
        std::slice::from_ref(analysis.obligations()),
    )?;
    let semantic = source.semantic().semantic();
    let [root] = semantic.roots() else {
        return Err(mismatch(Mismatch::Roster, "source root"));
    };
    let selected = semantic
        .select_kernel_body_for_root_v1(*root)
        .ok_or_else(|| mismatch(Mismatch::Roster, root))?;
    let body = &semantic.functions()[selected.body().index() as usize];
    let function_ordinal = pre
        .module()
        .functions
        .iter()
        .position(|f| f.id == kernel.entry)
        .ok_or_else(|| mismatch(Mismatch::Roster, &kernel.entry))?;
    let function = &pre.module().functions[function_ordinal];
    let control = analyze_control_flow(function).map_err(|e| mismatch(Mismatch::Phi, e))?;
    let mut work = kir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let used = {
        let mut budget =
            kir::CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 256 * 1024 * 1024);
        budget
            .reserve_storage(floor)
            .map_err(|e| mismatch(Mismatch::Owner, e))?;
        for (ordinal, block) in body.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::Assert {
                expected,
                message: SemanticAssertMessageV1::BoundsCheck { .. },
                target,
                ..
            } = block.terminator().kind()
            else {
                continue;
            };
            let semantic_block = SemanticBlockIdV1::from_index(ordinal as u32);
            if !origins
                .is_materialized_block(*root, selected.body(), semantic_block, &mut budget)
                .map_err(|e| mismatch(Mismatch::Assert, e))?
            {
                continue;
            }
            let binding = origins
                .assert_condition(*root, selected.body(), semantic_block, &mut budget)
                .map_err(|e| mismatch(Mismatch::Assert, e))?;
            if !*expected
                || !binding.expected()
                || binding.semantic_success() != target.target()
                || target.role() != SemanticEdgeRoleV1::AssertSuccess
            {
                return Err(mismatch(Mismatch::Assert, binding));
            }
            let SemanticKirAssertConditionOutcomeV1::Emitted {
                definition,
                success_edge,
                ..
            } = binding.outcome()
            else {
                result.elided_bounds_assertions += 1;
                continue;
            };
            let kir::CanonicalKirDefinitionCoordinateV1::Result {
                operation: coordinate,
                result: 0,
            } = definition
            else {
                return Err(mismatch(Mismatch::Assert, definition));
            };
            if coordinate.block.function.0 as usize != function_ordinal
                || success_edge.source.function.0 as usize != function_ordinal
            {
                return Err(mismatch(
                    Mismatch::Assert,
                    "assertion belongs to another actual function",
                ));
            }
            let blocks = &function.body.as_ref().unwrap().blocks;
            let compare =
                &blocks[coordinate.block.block as usize].operations[coordinate.operation as usize];
            let OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            } = compare.kind
            else {
                return Err(mismatch(Mismatch::Assert, &compare.kind));
            };
            let source_block = blocks[success_edge.source.block as usize].id;
            let successor =
                block_successor(function, source_block, success_edge.successor as usize)?;
            for row in &mut result.reads {
                if value_type(function, lhs)? == &Type::INDEX
                    && value_type(function, rhs)? == &Type::INDEX
                    && representation(function, lhs)?.0 == row.phi
                    && representation(function, rhs)?
                        == representation(function, ValueId(row.length))?
                {
                    edge_success(
                        function,
                        &control,
                        source_block,
                        success_edge.successor as usize,
                        successor,
                        BlockId(row.site[0]),
                    )?;
                    row.source_assertions.push([
                        ordinal as u32,
                        source_block.0,
                        success_edge.successor,
                        successor.0,
                    ]);
                }
            }
        }
        // A removed redundant Assert is not invented. Join the retained source
        // Boolean branch through its exact block and statement emission records.
        for (ordinal, semantic_block) in body.blocks().iter().enumerate() {
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = semantic_block.terminator().kind()
            else {
                continue;
            };
            let [zero] = targets.values() else {
                continue;
            };
            if zero.value() != 0
                || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
            {
                continue;
            }
            let (SemanticOperandV1::Copy(condition) | SemanticOperandV1::Move(condition)) =
                discriminant
            else {
                continue;
            };
            if !condition.projections().is_empty() {
                continue;
            }
            let Some((statement, assignment)) = semantic_block
                .statements()
                .iter()
                .enumerate()
                .rev()
                .find_map(|(i, statement)| match statement.kind() {
                    SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local() == condition.local()
                            && assignment.destination().projections().is_empty() =>
                    {
                        Some((i, assignment))
                    }
                    _ => None,
                })
            else {
                continue;
            };
            if !matches!(
                assignment.value().kind(),
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    ..
                }
            ) {
                continue;
            }
            let block_id = SemanticBlockIdV1::from_index(ordinal as u32);
            if !origins
                .is_materialized_block(*root, selected.body(), block_id, &mut budget)
                .map_err(|e| mismatch(Mismatch::Assert, e))?
            {
                continue;
            }
            let physical = |id| {
                source
                    .correspondence()
                    .blocks()
                    .iter()
                    .filter(|row| {
                        row.correspondence_owner() == *root
                            && row.semantic_function() == selected.body()
                            && row.semantic_block() == id
                    })
                    .map(|row| row.kernel_ir_block())
                    .collect::<Vec<_>>()
            };
            let from = physical(block_id);
            let to = physical(targets.otherwise().target());
            let ([from], [to]) = (from.as_slice(), to.as_slice()) else {
                continue;
            };
            for span in source
                .correspondence()
                .statement_operation_spans()
                .iter()
                .filter(|span| {
                    span.correspondence_owner() == *root
                        && span.semantic_function() == selected.body()
                        && span.semantic_block() == block_id
                        && span.statement_ordinal() as usize == statement
                        && span.kernel_ir_block() == *from
                })
            {
                let operations = &block(function, *from)?.operations;
                let start = span.first_operation_ordinal() as usize;
                let end = start
                    .checked_add(span.operation_count() as usize)
                    .ok_or_else(|| mismatch(Mismatch::Assert, "statement span overflow"))?;
                let operations = operations
                    .get(start..end)
                    .ok_or_else(|| mismatch(Mismatch::Assert, span))?;
                for row in &mut result.reads {
                    if row.guard[0] != from.0 || row.guard[2] != to.0 {
                        continue;
                    }
                    if !operations.iter().any(|operation| {
                        matches!(operation.kind,
                        OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs }
                            if lhs.0 == row.guard_index && rhs.0 == row.length)
                            && operation.results.first().map(|r| r.id.0) == Some(row.predicate)
                    }) {
                        continue;
                    }
                    let Some(Terminator::ConditionalBranch {
                        condition,
                        then_target,
                        ..
                    }) = &block(function, *from)?.terminator
                    else {
                        continue;
                    };
                    if condition.0 != row.predicate || *then_target != *to || row.guard[1] != 0 {
                        continue;
                    }
                    edge_success(function, &control, *from, 0, *to, BlockId(row.site[0]))?;
                    row.source_branches
                        .push([ordinal as u32, statement as u32, from.0, to.0]);
                }
            }
        }
        if budget.storage() != floor {
            return Err(mismatch(Mismatch::Owner, "read-only query floor"));
        }
        budget.work()
    };
    for row in &result.reads {
        if row.source_assertions.is_empty() && row.source_branches.is_empty() {
            return Err(mismatch(Mismatch::Assert, row));
        }
        if matches!(case, Case::Stale | Case::Different)
            && row.rejected_other_index_guards.is_empty()
        {
            return Err(mismatch(
                Mismatch::NegativeGuard,
                "the source control was not retained as a distinct guard",
            ));
        }
    }
    Ok((result, used))
}

fn block_successor(function: &Function, source: BlockId, ordinal: usize) -> Result<BlockId> {
    block(function, source)?
        .terminator
        .as_ref()
        .and_then(|term| term.successors().get(ordinal).copied())
        .ok_or_else(|| mismatch(Mismatch::GuardEdge, (source, ordinal)))
}

pub(super) fn check_summary(case: Case, rows: &GraphRows) {
    assert_eq!(rows.root, case.roots()[0]);
    assert!(rows.stores > 0);
    if case == Case::Control {
        assert_eq!(rows.loops, 0);
        assert!(rows.reads.is_empty());
    } else {
        assert!(rows.loops > 0);
        assert_eq!(rows.reads.len(), 1);
    }
    for row in &rows.reads {
        assert_ne!(row.phi, row.update);
    }
}

pub(super) fn check_source_successes(case: Case, rows: &GraphRows) {
    for row in &rows.reads {
        assert!(!row.source_assertions.is_empty() || !row.source_branches.is_empty());
        if matches!(case, Case::Stale | Case::Different) {
            assert!(!row.rejected_other_index_guards.is_empty());
        }
    }
}

#[test]
fn guarded_loop_read_cases_select_four_distinct_ordinary_source_features() {
    let features: std::collections::BTreeSet<_> =
        Case::ALL.map(Case::feature).into_iter().collect();
    assert_eq!(features.len(), 4);
    for case in Case::ALL {
        assert_eq!(case.roots().len(), 1);
    }
}

#[test]
fn guarded_loop_read_failure_protocol_preserves_stage_and_typed_mismatch() {
    let failure = mismatch(
        Mismatch::GuardIndex,
        "a different SSA index is not the read index",
    );
    let value = serde_json::to_value(&failure).unwrap();
    let decoded: Failure = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(decoded.phase, Phase::Rows);
    assert_eq!(decoded.mismatch, Some(Mismatch::GuardIndex));
    assert_eq!(decoded.detail, failure.detail);
    let mut hostile = value;
    hostile
        .as_object_mut()
        .unwrap()
        .insert("authority".into(), serde_json::Value::Bool(true));
    assert!(serde_json::from_value::<Failure>(hostile).is_err());
    let result = Err(mismatch(Mismatch::GuardIndex, "exact serialized refusal"));
    let bytes = response_json(&result);
    let decoded: Result<Observation> = serde_json::from_slice(&bytes).unwrap();
    let decoded = decoded.unwrap_err();
    assert_eq!(decoded.phase, Phase::Rows);
    assert_eq!(decoded.mismatch, Some(Mismatch::GuardIndex));
    assert_eq!(bytes, serde_json::to_vec(&result).unwrap());
    let oversized = Err(Failure {
        phase: Phase::Rows,
        mismatch: Some(Mismatch::GuardIndex),
        detail: "x".repeat(MAX_RESPONSE_JSON_BYTES),
    });
    assert!(std::panic::catch_unwind(|| response_json(&oversized)).is_err());
}

#[test]
fn guarded_loop_read_request_rejects_missing_duplicate_and_changed_fixture_options() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(format!("{BASE}/src/lib.rs"))
        .canonicalize()
        .unwrap();
    // Parser-only argv control. Real externs and metadata still come from the
    // unchanged parent Cargo artifact path, never from this protocol fixture.
    let actual_spelling = |case: Case, profile: &str| {
        [
            "rustc",
            "--crate-name",
            "fe2o3_production_extraction_fixture",
            "--crate-type=lib",
            "--edition=2024",
            "--target=amdgcn-amd-amdhsa",
            "-Copt-level=3",
            "-Cdebug-assertions=off",
            "-Coverflow-checks=on",
            "-Zalways-encode-mir",
            "-Zunstable-options",
            "--emit=metadata",
            "-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain([
            format!("-Ctarget-cpu={profile}"),
            source.to_str().unwrap().to_owned(),
            format!("--cfg=feature=\"{}\"", case.feature()),
        ])
        .collect::<Vec<_>>()
    };
    for profile in ["gfx942", "gfx950"] {
        for case in Case::ALL {
            assert_eq!(request_case(&actual_spelling(case, profile)).unwrap(), case);
        }
    }
    let baseline = actual_spelling(Case::Fresh, "gfx942");
    let mut missing = baseline.clone();
    missing.pop();
    assert_eq!(request_case(&missing).unwrap_err().phase, Phase::Request);
    for options in [
        vec!["--cfg=feature=\"guarded-loop-read\""],
        vec!["--cfg=feature=\"guarded-loop-read-different\""],
        vec!["--cfg=feature=\"guarded-loop-read-unknown\""],
        vec!["--cfg=feature=\"some-other-fixture\""],
        vec!["--cfg", "feature=\"guarded-loop-read\""],
        vec!["-Zmir-opt-level=0"],
        vec!["-Z", "mir-opt-level=0"],
        vec!["-Zinline-mir=no"],
        vec!["-Zalways-encode-mir"],
    ] {
        let mut args = baseline.clone();
        args.extend(options.into_iter().map(str::to_owned));
        assert_eq!(request_case(&args).unwrap_err().phase, Phase::Request);
    }
    let mut unknown = missing.clone();
    unknown.push("--cfg=feature=\"guarded-loop-read-unknown\"".into());
    assert_eq!(request_case(&unknown).unwrap_err().phase, Phase::Request);
    let mut split = missing;
    split.extend([
        "--cfg".to_owned(),
        "feature=\"guarded-loop-read\"".to_owned(),
    ]);
    assert_eq!(request_case(&split).unwrap_err().phase, Phase::Request);
    let mut wrong_source = baseline;
    wrong_source.retain(|arg| arg != source.to_str().unwrap());
    wrong_source.push(
        source
            .with_file_name("guarded_loop_read.rs")
            .to_str()
            .unwrap()
            .to_owned(),
    );
    assert_eq!(
        request_case(&wrong_source).unwrap_err().phase,
        Phase::Request
    );
}

// Component controls for the observer only. The ordinary-source callback never
// substitutes this graph for its genuine captured source owner.
fn observer_bridge_fixture() -> Module {
    use kir::{AccessMode, AddressSpace, CastKind, Constant, ScalarType, ValueDef};
    let u64_ty = Type::Scalar(ScalarType::U64);
    let u32_ty = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(u32_ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            4,
            u64_ty.clone(),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(3),
                to: u64_ty.clone(),
            },
        ),
        op(
            5,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(4),
                to: Type::INDEX,
            },
        ),
        op(6, u64_ty.clone(), OperationKind::Constant(Constant::U64(1))),
    ];
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(1)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(10), u64_ty.clone()));
    header.operations = vec![
        op(
            11,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(10),
                to: Type::INDEX,
            },
        ),
        op(
            12,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(11),
                rhs: ValueId(5),
            },
        ),
    ];
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![],
    });
    let mut read = BasicBlock::new(BlockId(2));
    read.operations = vec![
        op(
            13,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(10),
                to: Type::INDEX,
            },
        ),
        op(
            14,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            15,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(14),
                offset: ValueId(13),
            },
        ),
        op(
            16,
            u32_ty.clone(),
            OperationKind::Load {
                pointer: ValueId(15),
                access: kir::MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        Operation::checked_binary(
            ValueDef::new(ValueId(17), u64_ty.clone()),
            ValueDef::new(ValueId(18), Type::BOOL),
            kir::CheckedBinaryOperator::Add,
            ValueId(10),
            ValueId(6),
        ),
    ];
    read.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(17)],
    });
    let mut exit = BasicBlock::new(BlockId(3));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("observer-bridge-control");
    module.functions.push(Function::kernel_entry(
        "entry",
        kir::Signature::new(
            vec![
                Type::slice(u32_ty, AddressSpace::Global, AccessMode::ReadOnly),
                u64_ty.clone(),
                u64_ty,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, header, read, exit],
    ));
    module.kernels.push(kir::Kernel::new(
        "observer-control",
        "entry",
        kir::LaunchDomain::D1 {
            x: kir::LaunchExtent::Dynamic,
        },
    ));
    module
}

#[test]
fn guarded_loop_read_observer_keeps_raw_bridges_and_actual_u64_recurrence_separate() {
    let module = observer_bridge_fixture();
    let verified = kir::verify_module_ref(&module).unwrap();
    let analysis = kir::derive_kernel_memory_obligations_from_verified(
        verified,
        &kir::KernelId::new("observer-control"),
        kir::ExplicitLaunchExtent1d::Exact(64),
        kir::FormalIndexWidth::Bits64,
    )
    .unwrap();
    assert!(analysis.is_complete(), "{analysis:?}");
    let [access] = analysis.obligations().accesses() else {
        panic!("exact actual read");
    };
    let FormalAccessDomainV1::RuntimeSliceReadBounded(domain) = access.domain() else {
        panic!("actual runtime read");
    };
    assert_eq!(
        (domain.index(), domain.guard_index(), domain.length()),
        (ValueId(13), ValueId(11), ValueId(5))
    );
    let function = &module.functions[0];
    assert_eq!(representation(function, ValueId(11)).unwrap(), ValueId(10));
    assert_eq!(representation(function, ValueId(13)).unwrap(), ValueId(10));
    assert_eq!(representation(function, ValueId(5)).unwrap(), ValueId(3));
    compare_matches(function, domain, operation(function, ValueId(12)).unwrap()).unwrap();
    let flow = analyze_control_flow(function).unwrap();
    assert_eq!(
        changing_phi(function, &flow, ValueId(10), BlockId(2), BlockId(1)).unwrap(),
        (BlockId(1), ValueId(17))
    );
    assert_eq!(
        changing_phi(function, &flow, ValueId(13), BlockId(2), BlockId(1))
            .unwrap_err()
            .mismatch,
        Some(Mismatch::Phi)
    );
    edge_success(function, &flow, BlockId(1), 0, BlockId(2), BlockId(2)).unwrap();
    let mut changed = module.clone();
    changed.functions[0].body.as_mut().unwrap().blocks[2].operations[0].kind =
        OperationKind::Cast {
            kind: kir::CastKind::Bitcast,
            value: ValueId(2),
            to: Type::INDEX,
        };
    kir::verify_module_ref(&changed).unwrap();
    let function = &changed.functions[0];
    assert_eq!(representation(function, ValueId(13)).unwrap(), ValueId(2));
    assert_eq!(
        compare_matches(function, domain, operation(function, ValueId(12)).unwrap())
            .unwrap_err()
            .mismatch,
        Some(Mismatch::GuardIndex)
    );
}

//! Original-source observations only. This module does not interpret execution indices.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::fmt::Write as _;

mod format;
use format::Text;

pub(super) const MAX_BYTES: usize = 16_384;
const MAX_BLOCKS: usize = 256;
const MAX_EDGES: usize = 2048;
const MAX_STATEMENTS: usize = 2048;
const MAX_OPERANDS: usize = 32;
const MAX_PROJECTIONS: usize = 32;
const MAX_CANDIDATES: usize = 24;
const MAX_PREDECESSORS: usize = 8;
const MAX_BLOCK_STATEMENTS: usize = 12;

/// Only called by the original-function plan loop, never the execution-view loop.
pub(super) fn emit(
    semantic: &AdmittedInertSemanticMirV1,
    expected: SemanticFunctionIdV1,
    error: ProductionSemanticSsaErrorV1,
) -> ProductionSemanticSsaErrorV1 {
    let detail = semantic
        .functions()
        .get(expected.index() as usize)
        .and_then(|body| {
            describe(
                expected,
                body,
                semantic.types(),
                semantic.callables(),
                &error,
            )
        });
    let Some(detail) = detail else {
        return error;
    };
    emit_to(&mut std::io::stderr().lock(), Some(&detail), error)
}

pub(super) fn emit_to(
    output: &mut impl std::io::Write,
    detail: Option<&str>,
    error: ProductionSemanticSsaErrorV1,
) -> ProductionSemanticSsaErrorV1 {
    if let Some(detail) = detail {
        // A closed stderr must not turn the original validation rejection into a panic.
        let _ = output.write_all(detail.as_bytes());
    }
    error
}

pub(super) fn describe(
    expected: SemanticFunctionIdV1,
    body: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    error: &ProductionSemanticSsaErrorV1,
) -> Option<String> {
    // No wrapper unwrapping: an expanded local/block is not an original index.
    let ProductionSemanticSsaErrorV1::PartialMove {
        function,
        block,
        statement,
        local,
        violation,
    } = error
    else {
        return None;
    };
    if *function != expected {
        return None;
    }
    let failing = body.blocks().get(*block as usize)?;
    let declaration = body.locals().get(*local as usize)?;
    if statement.is_some_and(|index| index as usize >= failing.statements().len()) {
        return None;
    }
    let mut out = Text::new();
    let _ = write!(
        out,
        "[fe2o3-source-partial-move-v1] original_function={} identity=",
        function.index()
    );
    out.hash(body.identity().as_bytes());
    let _ = write!(
        out,
        " role={:?} entry={} blocks={} locals={} abi_identity=",
        body.role(),
        body.entry().index(),
        body.blocks().len(),
        body.locals().len()
    );
    out.hash(body.abi().identity().as_bytes());
    let _ = write!(out, " source=");
    out.source(body.source());
    let _ = writeln!(
        out,
        "\nfailure=bb{block}s{statement:?} local{local} violation={violation:?}; original source coordinates only"
    );
    let _ = writeln!(
        out,
        "observations=static typed MIR; no active-state, dominance, liveness, or causal-path conclusion"
    );
    let _ = write!(
        out,
        "failed_local role={:?} type{} identity=",
        declaration.role(),
        declaration.ty().index()
    );
    out.hash(declaration.identity().as_bytes());
    let _ = write!(out, " source=");
    out.source(declaration.source());
    let _ = writeln!(out);
    out.ty(types, declaration.ty());
    if let Some(ty) = types.get(declaration.ty().index() as usize) {
        match ty.shape() {
            SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                for field in fields.fields().iter().take(6) {
                    out.ty(types, *field);
                }
                if fields.fields().len() > 6 {
                    let _ = writeln!(out, "[field types truncated]");
                }
            }
            SemanticTypeShapeV1::Array { element, .. } => out.ty(types, *element),
            SemanticTypeShapeV1::Pointer(pointer) => out.ty(types, pointer.pointee()),
            _ => {}
        }
    }
    let _ = write!(
        out,
        "EXACT_FAILING_{} bb{block} ",
        if statement.is_some() {
            "STATEMENT"
        } else {
            "TERMINATOR"
        }
    );
    if let Some(index) = statement {
        let value = &failing.statements()[*index as usize];
        let _ = write!(out, "s{index} source=");
        out.source(value.source());
        let _ = write!(out, " ");
        out.statement(body, value.kind());
    } else {
        out.source(failing.terminator().source());
        out.terminator(body, callables, failing.terminator().kind());
    }
    let _ = writeln!(out);

    // Lexical candidates may be unreachable or occur after the failure. They are
    // observations, not substitutes for the validator's live incoming state.
    let mut statements = 0;
    let mut candidates = 0;
    let mut scan_complete = body.blocks().len() <= MAX_BLOCKS;
    'scan: for (block_index, candidate) in body.blocks().iter().enumerate().take(MAX_BLOCKS) {
        for (index, value) in candidate.statements().iter().enumerate() {
            if statements == MAX_STATEMENTS || candidates == MAX_CANDIDATES || out.full() {
                scan_complete = false;
                break 'scan;
            }
            statements += 1;
            let Some(mentions) = statement_mentions(value.kind(), *local) else {
                scan_complete = false;
                continue;
            };
            if mentions {
                candidates += 1;
                let _ = write!(
                    out,
                    "static_local_candidate bb{block_index}s{index} source="
                );
                out.source(value.source());
                let _ = write!(out, " ");
                out.statement(body, value.kind());
                let _ = writeln!(out);
            }
        }
        if candidates == MAX_CANDIDATES || out.full() {
            scan_complete = false;
            break;
        }
        match terminator_mentions(body, candidate.terminator().kind(), *local) {
            Some(true) => {
                candidates += 1;
                let _ = write!(
                    out,
                    "static_local_candidate bb{block_index}terminator source="
                );
                out.source(candidate.terminator().source());
                out.terminator(body, callables, candidate.terminator().kind());
                let _ = writeln!(out);
            }
            Some(false) => {}
            None => scan_complete = false,
        }
    }
    let _ = writeln!(
        out,
        "static_local_scan complete={scan_complete} statements={statements} candidates={candidates} limits=blocks{MAX_BLOCKS}/statements{MAX_STATEMENTS}/candidates{MAX_CANDIDATES}"
    );

    let mut predecessors = [None; MAX_PREDECESSORS];
    let mut count = 0;
    let mut edges = 0;
    let mut complete = body.blocks().len() <= MAX_BLOCKS;
    for (index, candidate) in body.blocks().iter().enumerate().take(MAX_BLOCKS) {
        let scanned = candidate.terminator().kind().try_for_each_edge(|edge| {
            if edges == MAX_EDGES || out.full() {
                return Err(());
            }
            edges += 1;
            if edge.target().index() == *block {
                if !predecessors.contains(&Some(index)) {
                    if count == MAX_PREDECESSORS {
                        return Err(());
                    }
                    predecessors[count] = Some(index);
                    count += 1;
                }
                let _ = writeln!(
                    out,
                    "static_predecessor bb{index} role={:?} -> bb{block}",
                    edge.role()
                );
            }
            Ok(())
        });
        if scanned.is_err() {
            complete = false;
            break;
        }
    }
    let _ = writeln!(
        out,
        "predecessor_scan complete={complete} edges={edges} limits=blocks{MAX_BLOCKS}/edges{MAX_EDGES}/predecessors{MAX_PREDECESSORS}; edge roles retained, reachability not inferred"
    );
    dump_block(
        &mut out,
        body,
        callables,
        *block as usize,
        statement.map(|index| index as usize),
    );
    for index in predecessors.into_iter().flatten() {
        if index != *block as usize {
            dump_block(&mut out, body, callables, index, None);
        }
    }
    let entry = body.entry().index() as usize;
    if entry != *block as usize && !predecessors.contains(&Some(entry)) {
        dump_block(&mut out, body, callables, entry, Some(0));
    }
    Some(out.finish())
}

fn dump_block(
    out: &mut Text,
    body: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    index: usize,
    center: Option<usize>,
) {
    if out.full() {
        return;
    }
    let Some(block) = body.blocks().get(index) else {
        return;
    };
    let start = center
        .map(|center| center.saturating_sub(4))
        .unwrap_or_else(|| {
            block
                .statements()
                .len()
                .saturating_sub(MAX_BLOCK_STATEMENTS)
        });
    let _ = write!(out, "selected_source_bb{index} identity=");
    out.hash(block.identity().as_bytes());
    let _ = write!(out, " source=");
    out.source(block.source());
    let _ = writeln!(
        out,
        " statements={} prefix_omitted={start}",
        block.statements().len()
    );
    for (statement, value) in block
        .statements()
        .iter()
        .enumerate()
        .skip(start)
        .take(MAX_BLOCK_STATEMENTS)
    {
        if out.full() {
            break;
        }
        let _ = write!(out, "  s{statement} ");
        out.statement(body, value.kind());
        let _ = writeln!(out);
    }
    if block.statements().len() > start.saturating_add(MAX_BLOCK_STATEMENTS) {
        let _ = writeln!(out, "  [statement suffix truncated]");
    }
    let _ = write!(out, "  terminator source=");
    out.source(block.terminator().source());
    out.terminator(body, callables, block.terminator().kind());
    let _ = writeln!(out);
}

fn place_mentions(place: &SemanticPlaceV1, local: u32) -> Option<bool> {
    if place.local().index() == local {
        return Some(true);
    }
    if place.projections().len() > MAX_PROJECTIONS {
        return None;
    }
    Some(place.projections().iter().any(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Index(index) if index.index() == local)))
}

fn operand_mentions(operand: &SemanticOperandV1, local: u32) -> Option<bool> {
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            place_mentions(place, local)
        }
        SemanticOperandV1::Constant(_) => Some(false),
    }
}

fn statement_mentions(statement: &SemanticStatementKindV1, local: u32) -> Option<bool> {
    let mut found = false;
    let mut add_place = |place| {
        found |= place_mentions(place, local)?;
        Some(())
    };
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            add_place(assignment.destination())?;
            match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                | SemanticRvalueKindV1::Length(place)
                | SemanticRvalueKindV1::Discriminant(place) => add_place(place)?,
                SemanticRvalueKindV1::Load(load) => add_place(load.source())?,
                _ => {}
            }
            let mut operands = 0;
            assignment
                .value()
                .kind()
                .try_visit_operands(|operand| {
                    if operands == MAX_OPERANDS {
                        return Err(());
                    }
                    operands += 1;
                    found |= operand_mentions(operand, local).ok_or(())?;
                    Ok(())
                })
                .ok()?;
        }
        SemanticStatementKindV1::Store(store) => {
            add_place(store.destination())?;
            found |= operand_mentions(store.value(), local)?;
        }
        SemanticStatementKindV1::AtomicRmw(value) => {
            add_place(value.address())?;
            add_place(value.destination())?;
            found |= operand_mentions(value.value(), local)?;
        }
        SemanticStatementKindV1::AtomicCompareExchange(value) => {
            add_place(value.address())?;
            add_place(value.destination())?;
            found |= operand_mentions(value.expected(), local)?;
            found |= operand_mentions(value.replacement(), local)?;
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => add_place(place)?,
        SemanticStatementKindV1::StorageLive(id) | SemanticStatementKindV1::StorageDead(id) => {
            found = id.index() == local
        }
        SemanticStatementKindV1::Assume(value) => found = operand_mentions(value, local)?,
        SemanticStatementKindV1::Nop => {}
    }
    Some(found)
}

fn arguments_mention(arguments: &[SemanticOperandV1], local: u32) -> Option<bool> {
    if arguments.len() > MAX_OPERANDS {
        return None;
    }
    let mut found = false;
    for argument in arguments {
        found |= operand_mentions(argument, local)?;
    }
    Some(found)
}

fn terminator_mentions(
    body: &SemanticFunctionDeclV1,
    value: &SemanticTerminatorKindV1,
    local: u32,
) -> Option<bool> {
    match value {
        SemanticTerminatorKindV1::Call(call) => {
            let mut found = arguments_mention(call.arguments(), local)?;
            if let Some(destination) = call.destination() {
                found |= place_mentions(destination.place(), local)?;
            }
            Some(found)
        }
        SemanticTerminatorKindV1::TailCall(call) => arguments_mention(call.arguments(), local),
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            operand_mentions(discriminant, local)
        }
        SemanticTerminatorKindV1::Drop { place, .. } => place_mentions(place, local),
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            let mut found = operand_mentions(condition, local)?;
            let mut add = |value| {
                found |= operand_mentions(value, local)?;
                Some(())
            };
            match message {
                SemanticAssertMessageV1::BoundsCheck { length, index } => {
                    add(length)?;
                    add(index)?;
                }
                SemanticAssertMessageV1::Overflow { left, right, .. } => {
                    add(left)?;
                    add(right)?;
                }
                SemanticAssertMessageV1::DivisionByZero(value)
                | SemanticAssertMessageV1::RemainderByZero(value) => add(value)?,
                SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment,
                    found_alignment,
                } => {
                    add(required_alignment)?;
                    add(found_alignment)?;
                }
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => {}
            }
            Some(found)
        }
        SemanticTerminatorKindV1::Return => Some(
            body.locals()
                .get(local as usize)
                .is_some_and(|declaration| declaration.role() == SemanticLocalRoleV1::Return),
        ),
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Some(false),
    }
}

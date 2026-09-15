//! Failure-only observation of retained source and expansion. No proof queries.
use super::*;
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use rustc_middle::mir as raw;

#[path = "custody_diagnostic/bounded.rs"]
mod bounded;
#[path = "custody_diagnostic/hir.rs"]
mod hir;
use bounded::{Output, Result};

const MAX_INSTANCES: usize = 128;
const MAX_FUNCTIONS: usize = 64;

pub(in super::super) fn failure<'tcx>(
    tcx: TyCtxt<'tcx>,
    retained: &ProductionSemanticPreflightPlanV1<'tcx>,
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    block: usize,
    operation: SemanticGfx950TransposeOperationV1,
    argument: usize,
) -> String {
    let mut out = Output::new();
    let result = observe(
        tcx, retained, owner, root, block, operation, argument, &mut out,
    );
    out.finish(result)
}

// A walk on the retained call tree, not a CFG search or a custody decision.
fn retain_ancestors(
    mut index: usize,
    length: usize,
    mut parent: impl FnMut(usize) -> Option<usize>,
    selected: &mut BTreeSet<usize>,
    out: &mut Output,
) -> Result {
    loop {
        out.charge(1)?;
        if index >= length {
            return Err("call instance outside retained roster");
        }
        if selected.contains(&index) {
            return Ok(());
        }
        if selected.len() == MAX_INSTANCES {
            return Err("selected call-instance bound reached");
        }
        selected.insert(index);
        let Some(next) = parent(index) else {
            return Ok(());
        };
        if next >= index {
            return Err("retained call parent is not an earlier instance");
        }
        index = next;
    }
}

fn unique_index<T: Eq>(
    expected: &T,
    candidates: impl IntoIterator<Item = T>,
    out: &mut Output,
) -> Result<usize> {
    let mut found = None;
    for (index, candidate) in candidates.into_iter().enumerate() {
        out.charge(1)?;
        if candidate == *expected && found.replace(index).is_some() {
            return Err("source identity matches duplicate retained producers");
        }
    }
    found.ok_or("source identity has no retained producer")
}

fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    retained: &ProductionSemanticPreflightPlanV1<'tcx>,
    owner: &ProductionSemanticSsaOwnerV1,
    root: SemanticFunctionIdV1,
    failed_block: usize,
    operation: SemanticGfx950TransposeOperationV1,
    argument: usize,
    out: &mut Output,
) -> Result {
    let mir = owner.source_semantic();
    let view = owner
        .execution_view_for_root(root)
        .ok_or("missing execution view")?;
    let ssa = owner
        .execution_plan_for_root(root)
        .ok_or("missing SSA plan")?;
    out.line(format_args!(
        "{}",
        super::transition_operand_failure(mir, view, failed_block, operation, argument)
    ))?;
    out.line(format_args!(
        "TRANSPOSE_CUSTODY_BEGIN root.identity={:?} expansion={:?}; candidates are observation only",
        mir.functions().get(root.index() as usize).map(|f| f.identity()), view.identity()
    ))?;
    let origin = view
        .block_origins()
        .get(failed_block)
        .ok_or("missing failing block origin")?;
    let tile = match operation {
        SemanticGfx950TransposeOperationV1::Publish { input_tile, .. } if argument == 0 => {
            input_tile
        }
        _ => return Err("observation currently selects only Publish argument zero"),
    };
    let mut selected = BTreeSet::new();
    let parent = |i: usize| view.instances()[i].parent().map(|p| p.index() as usize);
    retain_ancestors(
        origin.instance().index() as usize,
        view.instances().len(),
        parent,
        &mut selected,
        out,
    )?;
    let mut candidates = 0usize;
    for (index, block) in view.body().blocks().iter().enumerate() {
        out.charge(1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(record) = mir
            .callables()
            .get(call.callee().index() as usize)
            .and_then(super::transpose)
        else {
            continue;
        };
        if !matches!(record.operation(),
            SemanticExecutionCapabilityOperationV1::Gfx950Transpose(contract)
            if matches!(contract.operation(), SemanticGfx950TransposeOperationV1::Stage { output_tile, .. } if output_tile == tile))
        {
            continue;
        }
        candidates += 1;
        let origin = view
            .block_origins()
            .get(index)
            .ok_or("missing Stage block origin")?;
        out.line(format_args!("candidate Stage expanded.bb={index} source.identity={:?} origin={origin:?}; NOT a producer proof", record.source_identity()))?;
        retain_ancestors(
            origin.instance().index() as usize,
            view.instances().len(),
            parent,
            &mut selected,
            out,
        )?;
    }
    out.line(format_args!(
        "Stage candidates={candidates}; selected instances={selected:?}"
    ))?;
    if candidates == 0 {
        return Err("no type-matching Stage candidate in this execution view");
    }
    let mut functions = BTreeSet::new();
    for &index in &selected {
        out.charge(1)?;
        let instance = &view.instances()[index];
        let function = mir
            .functions()
            .get(instance.function().index() as usize)
            .ok_or("missing canonical function")?;
        if instance.function_identity() != function.identity() {
            return Err("instance and canonical source identities disagree");
        }
        out.line(format_args!("instance={index} source.identity={:?} canonical.function={} parent={:?} parent.call.block={:?}", instance.function_identity(), instance.function().index(), instance.parent(), instance.call_block()))?;
        if !functions.contains(&instance.function()) && functions.len() == MAX_FUNCTIONS {
            return Err("selected source-function bound reached");
        }
        functions.insert(instance.function());
    }
    for (index, local) in view.body().locals().iter().enumerate() {
        out.charge(1)?;
        let origin = view
            .local_origins()
            .get(index)
            .ok_or("missing expanded local origin")?;
        if selected.contains(&(origin.instance().index() as usize)) {
            out.line(format_args!(
                "expanded.local={index} origin={origin:?} ty.identity={:?} role={:?}",
                mir.types()
                    .get(local.ty().index() as usize)
                    .map(|t| t.identity()),
                local.role()
            ))?;
        }
    }
    // Dump the existing expansion first, including generated return transfers
    // and their resolved SSA events. This does not rerun SSA or a loan query.
    for (index, block) in view.body().blocks().iter().enumerate() {
        out.charge(1)?;
        let origin = view
            .block_origins()
            .get(index)
            .ok_or("missing expanded block origin")?;
        if !selected.contains(&(origin.instance().index() as usize)) {
            continue;
        }
        out.line(format_args!("expanded.bb={index} instance={} canonical.function={} canonical.block={} terminator.origin={:?}", origin.instance().index(), origin.function().index(), origin.block().index(), origin.terminator()))?;
        for (statement, item) in block.statements().iter().enumerate() {
            out.charge(1)?;
            out.line(format_args!(
                " expanded.stmt={statement} origin={:?} kind={:?}",
                origin.statements().get(statement),
                item.kind()
            ))?;
        }
        out.line(format_args!(
            " expanded.term={:?}",
            block.terminator().kind()
        ))?;
        let id = SsaBlockIdV1::new(index as u32);
        let events = ssa.plan().resolved_events(id);
        out.line(format_args!(
            " ssa.reachable={} events.present={}",
            ssa.plan().is_reachable(id),
            events.is_some()
        ))?;
        if let Some(events) = events {
            for (event, resolved) in events {
                out.charge(1)?;
                out.line(format_args!(" ssa.event={event} resolved={resolved:?}"))?;
            }
        }
        if matches!(block.terminator().kind(), SemanticTerminatorKindV1::Call(_)) {
            let definitions = ssa.plan().edge_definitions(SsaEdgeIdV1::new(id, 0));
            if let Some(rows) = definitions {
                out.charge(rows.len())?;
            }
            out.line(format_args!(" ssa.normal.edge.definitions={definitions:?}"))?;
        }
    }
    // Membership only; output traversal remains in canonical function order.
    let mut hir_owners = std::collections::HashSet::new();
    for function_id in functions {
        out.charge(1)?;
        let function = &mir.functions()[function_id.index() as usize];
        let raw_index = unique_index(
            &function.identity(),
            retained
                .function_producers()
                .iter()
                .map(|p| p.identities.function()),
            out,
        )?;
        let producer = &retained.function_producers()[raw_index];
        out.line(format_args!("source.identity={:?} canonical.function={} retained.producer={} rustc.def={:?} instance={:?}; block/local spaces are DISTINCT", function.identity(), function_id.index(), raw_index, tcx.def_path_hash(producer.instance.def_id()), producer.instance))?;
        for (index, local) in function.locals().iter().enumerate() {
            out.charge(1)?;
            out.line(format_args!(
                " canonical.local={index} ty.identity={:?} role={:?}",
                mir.types()
                    .get(local.ty().index() as usize)
                    .map(|t| t.identity()),
                local.role()
            ))?;
        }
        for (index, block) in function.blocks().iter().enumerate() {
            out.charge(1)?;
            for (statement, item) in block.statements().iter().enumerate() {
                out.charge(1)?;
                out.line(format_args!(
                    " canonical.bb={index} stmt={statement} kind={:?}",
                    item.kind()
                ))?;
            }
            out.line(format_args!(
                " canonical.bb={index} term={:?}",
                block.terminator().kind()
            ))?;
        }
        let raw_id = SemanticFunctionIdV1::from_index(
            u32::try_from(raw_index).map_err(|_| "retained index overflow")?,
        );
        out.line(format_args!(
            "canonical.source.abi={:?}",
            function.abi().identity()
        ))?;
        for call in retained.direct_call_producers() {
            out.charge(1)?;
            if call.caller == raw_id {
                let callee = retained
                    .function_producers()
                    .get(call.callee.index() as usize)
                    .ok_or("retained direct-call callee is missing")?;
                out.line(format_args!("retained.direct.call raw.bb={} callee.source.identity={:?} callee.instance={:?}", call.block, callee.identities.function(), callee.instance))?;
            }
        }
        for call in retained.terminal_expansion_producers() {
            out.charge(1)?;
            if call.caller == raw_id {
                out.line(format_args!("retained.terminal.call raw.bb={} callee.source.identity={:?} callee.instance={:?} span={:?}", call.block, call.identities.function(), call.instance, call.span))?;
            }
        }
        let body = retained
            .function_mir(raw_id)
            .ok_or("source identity matched but retained MIR body missing")?;
        dump_raw(body, out)?;
        if let Some(local) = producer.instance.def_id().as_local() {
            if hir_owners.insert(local) {
                hir::dump(tcx, local, out)?;
            } else {
                out.line(format_args!(
                    " HIR already observed for source.def={:?}",
                    tcx.def_path_hash(local.to_def_id())
                ))?;
            }
        } else {
            out.line(format_args!(" HIR unavailable: external retained definition; MIR above is still its retained body"))?;
        }
    }
    out.line(format_args!(
        "TRANSPOSE_CUSTODY_END selected observation complete; no custody claim"
    ))
}

fn dump_raw(body: &raw::Body<'_>, out: &mut Output) -> Result {
    out.line(format_args!(
        " retained.MIR.phase={:?} source={:?} args={}",
        body.phase, body.source, body.arg_count
    ))?;
    for (local, declaration) in body.local_decls.iter_enumerated() {
        out.charge(1)?;
        out.line(format_args!(
            " raw.local={} ty={:?} source={:?}",
            local.as_u32(),
            declaration.ty,
            declaration.source_info
        ))?;
    }
    for (index, block) in body.basic_blocks.iter_enumerated() {
        out.charge(1)?;
        for (statement, item) in block.statements.iter().enumerate() {
            out.charge(1)?;
            out.line(format_args!(
                " raw.bb={} stmt={statement} source={:?}",
                index.as_u32(),
                item.source_info
            ))?;
            if let raw::StatementKind::Assign(assignment) = &item.kind {
                let (place, value) = &**assignment;
                out.charge(place.projection.len())?;
                out.line(format_args!(" raw.Assign destination={place:?}"))?;
                if let raw::Rvalue::Use(operand) = value {
                    dump_operand("Use", operand, out)?;
                } else {
                    out.line(format_args!(" raw.rvalue={value:?}"))?;
                }
            } else {
                out.line(format_args!(" raw.statement={:?}", item.kind))?;
            }
        }
        let term = block.terminator();
        out.line(format_args!(
            " raw.bb={} cleanup={} term.source={:?}",
            index.as_u32(),
            block.is_cleanup,
            term.source_info
        ))?;
        if let raw::TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = &term.kind
        {
            dump_operand("func", func, out)?;
            for (argument, value) in args.iter().enumerate() {
                out.charge(1)?;
                out.line(format_args!(" raw.arg={argument} span={:?}", value.span))?;
                dump_operand("arg", &value.node, out)?;
            }
            out.line(format_args!(
                " raw.destination={destination:?} target={target:?} unwind={unwind:?}"
            ))?;
        } else {
            out.line(format_args!(" raw.term={:?}", term.kind))?;
        }
    }
    Ok(())
}

fn dump_operand(label: &str, operand: &raw::Operand<'_>, out: &mut Output) -> Result {
    match operand {
        raw::Operand::Copy(place) | raw::Operand::Move(place) => {
            out.charge(place.projection.len())?;
            out.line(format_args!(
                " raw.{label}.kind={} place={place:?}",
                if matches!(operand, raw::Operand::Move(_)) {
                    "Move"
                } else {
                    "Copy"
                }
            ))
        }
        raw::Operand::RuntimeChecks(check) => out.line(format_args!(
            " raw.{label}.kind=RuntimeChecks check={check:?}; session value not evaluated"
        )),
        raw::Operand::Constant(value) => out.line(format_args!(
            " raw.{label}.kind=Constant literal.ZeroSized={} ty={:?}; payload omitted",
            matches!(value.const_, raw::Const::Val(raw::ConstValue::ZeroSized, _)),
            value.const_.ty()
        )),
    }
}

#[cfg(test)]
#[path = "custody_diagnostic/tests.rs"]
mod tests;

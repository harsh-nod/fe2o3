//! Direct-call source transitions only. The caller supplies the shared logical
//! source state and proves the actual call splice and callee body separately.
//! Both helpers consume the exact block's post-statement state. The enclosing
//! source step must execute that block's statements first; block pc alone does
//! not establish this phase, and neither helper is an alternative block step.

use super::{CallTransfers, DirectTransfer, ExpressionV30, Resource, Result, Transfer, Writer};
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn emit_steps(
    transfers: &CallTransfers<'_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    // Validate custody before reserving any emitter frame on the given ledger.
    let rows = transfers.rows(out)?;
    out.budget.reserve_storage(headers())?;
    for row in rows {
        out.budget.charge_work(1)?;
        if let Transfer::Direct(transfer) = &row.transfer {
            direct(transfer, out)?;
        }
    }
    Ok(())
}

pub(super) fn direct(transfer: &DirectTransfer, out: &mut Writer<'_, '_>) -> Result<()> {
    let key = transfer.site;
    let caller_start = transfer.caller_locals.start;
    let caller_end = transfer.caller_locals.end;
    let child_start = transfer.child_locals.start;
    let child_end = transfer.child_locals.end;
    let length = caller_end.max(child_end);
    super::super::emit_graph_v30(
        &transfer.operands.nodes,
        transfer.caller_locals.len(),
        key,
        "call_arguments",
        (0..transfer.operands.nodes.len()).map(|node| Ok(Some(node))),
        None,
        out,
    )?;
    emit!(
        out,
        "open spec fn original_call_enabled_{key}_v33(s: OriginalControlStateV31) -> bool {{\n s.pc == {key}int && s.values.len() == s.defined.len() && s.values.len() >= {length}"
    );
    for node in &transfer.operands.nodes {
        out.budget.charge_work(1)?;
        if let ExpressionV30::Argument(local) = node.expression {
            let global = caller_start
                .checked_add(local as usize)
                .ok_or(Resource::Arithmetic)?;
            let modulus = 1u128 << node.scalar.width();
            emit!(
                out,
                "\n && s.defined[{global}] && 0 <= s.values[{global}] < {modulus}int"
            );
        }
    }
    emit!(
        out,
        "\n}}\nopen spec fn original_call_enter_{key}_v33(s: OriginalControlStateV31) -> OriginalControlStateV31\n recommends original_call_enabled_{key}_v33(s),\n{{\n let n = original_call_arguments_trace_{key}_v30(s.values.subrange({caller_start}int, {caller_end}int));\n let values = s.values"
    );
    for &(target, node) in &transfer.arguments {
        out.budget.charge_work(1)?;
        emit!(out, ".update({target}int, n[{node}])");
    }
    // Non-argument callee locals become undefined on every entry, including
    // repeated loop calls to the same statically retained invocation.
    emit!(
        out,
        ";\n let defined = Seq::new(s.defined.len(), |i: int| if {child_start} <= i < {child_end} {{ false }} else {{ s.defined[i] }})"
    );
    for &local in &transfer.moved {
        out.budget.charge_work(1)?;
        emit!(out, ".update({local}int, false)");
    }
    for &(target, _) in &transfer.arguments {
        out.budget.charge_work(1)?;
        emit!(out, ".update({target}int, true)");
    }
    emit!(
        out,
        ";\n OriginalControlStateV31 {{ pc: {}int, values, defined, cells: s.cells, initialized: s.initialized, external: s.external }}\n}}\n",
        transfer.entry
    );
    emit!(
        out,
        "open spec fn original_call_return_enabled_{key}_v33(s: OriginalControlStateV31) -> bool {{\n s.values.len() == s.defined.len() && s.values.len() >= {length}"
    );
    emit!(out, "\n && (false");
    for &block in &transfer.return_blocks {
        out.budget.charge_work(1)?;
        emit!(out, " || s.pc == {block}int");
    }
    emit!(out, ")");
    if let Some(source) = transfer.returned.source {
        let modulus = 1u128 << transfer.returned.scalar.width();
        emit!(
            out,
            "\n && s.defined[{source}] && 0 <= s.values[{source}] < {modulus}int"
        );
    }
    // The return selector is the original terminator roster, not a result
    // value or a caller-selected target. Reachability remains a body proof.
    let destination = transfer.returned.destination;
    emit!(
        out,
        "\n}}\nopen spec fn original_call_return_{key}_v33(s: OriginalControlStateV31) -> OriginalControlStateV31\n recommends original_call_return_enabled_{key}_v33(s),\n{{\n let values = s.values.update({destination}int, "
    );
    if let Some(source) = transfer.returned.source {
        emit!(out, "s.values[{source}]");
    } else {
        emit!(out, "0int");
    }
    emit!(
        out,
        ");\n let defined = Seq::new(s.defined.len(), |i: int| if {child_start} <= i < {child_end} {{ false }} else {{ s.defined[i] }}).update({destination}int, true);\n OriginalControlStateV31 {{ pc: {}int, values, defined, cells: s.cells, initialized: s.initialized, external: s.external }}\n}}\n",
        transfer.returned.continuation
    );
    Ok(())
}

pub(super) fn headers() -> usize {
    size_of::<(&CallTransfers<'_, '_, '_>, &mut Writer<'_, '_>)>()
        + size_of::<(&DirectTransfer, &mut Writer<'_, '_>)>()
        + 3 * size_of::<Result<()>>()
        + 11 * size_of::<usize>()
        + size_of::<u128>()
        + size_of::<std::slice::Iter<'_, super::CallRow>>()
        + size_of::<std::slice::Iter<'_, (usize, usize)>>()
        + size_of::<std::slice::Iter<'_, usize>>()
        + size_of::<std::slice::Iter<'_, super::NodeV30>>()
}

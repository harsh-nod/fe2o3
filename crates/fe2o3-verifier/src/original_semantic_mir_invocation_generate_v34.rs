//! The single source step executes the whole selected block body before its
//! terminator transfer. A callee Return cannot bypass its final assignments.
use super::super::{ExpressionV30, control::Branch, emit_graph_v30};
use super::{Body, InvocationBodies, Resource, Result, Writer};
use std::{fmt::Write as _, mem::size_of};

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn emit_steps(
    model: &InvocationBodies<'_, '_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    model.check(out)?;
    out.budget.reserve_storage(headers())?;
    model.transfers.emit_steps(out)?;
    out.budget.charge_work(model.bodies.len())?;
    for body in model.bodies.iter().flatten() {
        for (block, original) in body.control.blocks.iter().enumerate() {
            out.budget.charge_work(1)?;
            let Some(original) = original else {
                continue;
            };
            let key = body
                .blocks
                .start
                .checked_add(block)
                .ok_or(Resource::Arithmetic)?;
            emit_graph_v30(
                &original.program.nodes,
                body.control.locals,
                key,
                "invocation_body",
                (0..original.program.nodes.len()).map(|node| Ok(Some(node))),
                None,
                out,
            )?;
            emit!(
                out,
                "open spec fn original_invocation_block_enabled_{key}_v34(s: OriginalControlStateV31) -> bool {{\n s.pc == {key}int && s.values.len() == {} && s.defined.len() == {}",
                model.locals,
                model.locals
            );
            for live in &original.live {
                out.budget.charge_work(2)?;
                let local = body
                    .locals
                    .start
                    .checked_add(live.local as usize)
                    .ok_or(Resource::Arithmetic)?;
                let scalar = body
                    .control
                    .types
                    .get(live.local as usize)
                    .ok_or(Resource::Accounting)?;
                let modulus = 1u128 << scalar.width();
                emit!(
                    out,
                    "\n && s.defined[{local}] && 0 <= s.values[{local}] < {modulus}int"
                );
            }
            emit!(out, "\n}}\n");
        }
    }
    for (root, range) in model.roots.iter().enumerate() {
        let bodies = model
            .bodies
            .get(range.clone())
            .ok_or(Resource::Accounting)?;
        emit!(
            out,
            "open spec fn original_invocation_enabled_{root}_v34(s: OriginalControlStateV31) -> bool {{\n false"
        );
        out.budget.charge_work(bodies.len())?;
        for body in bodies.iter().flatten() {
            for (block, source) in body.control.blocks.iter().enumerate() {
                out.budget.charge_work(1)?;
                if source.is_some() {
                    let key = body
                        .blocks
                        .start
                        .checked_add(block)
                        .ok_or(Resource::Arithmetic)?;
                    emit!(out, " || original_invocation_block_enabled_{key}_v34(s)");
                }
            }
        }
        emit!(
            out,
            "\n}}\nopen spec fn original_invocation_step_{root}_v34(s: OriginalControlStateV31) -> CfgStepV26<OriginalControlStateV31, int>\n recommends original_invocation_enabled_{root}_v34(s),\n{{\n"
        );
        out.budget.charge_work(bodies.len())?;
        for body in bodies.iter().flatten() {
            for (block, original) in body.control.blocks.iter().enumerate() {
                out.budget.charge_work(1)?;
                let Some(original) = original else {
                    continue;
                };
                let key = body
                    .blocks
                    .start
                    .checked_add(block)
                    .ok_or(Resource::Arithmetic)?;
                emit!(
                    out,
                    " if s.pc == {key}int {{\n let n = original_invocation_body_trace_{key}_v30(s.values.subrange({}int, {}int));\n let values = s.values",
                    body.locals.start,
                    body.locals.end
                );
                for (local, (&changed, &node)) in original
                    .changed
                    .iter()
                    .zip(&original.program.locals)
                    .enumerate()
                {
                    out.budget.charge_work(1)?;
                    if let (true, Some(node)) = (changed, node) {
                        let local = body
                            .locals
                            .start
                            .checked_add(local)
                            .ok_or(Resource::Arithmetic)?;
                        emit!(out, ".update({local}int, n[{node}])");
                    }
                }
                emit!(out, ";\n let defined = s.defined");
                for (local, (&changed, node)) in original
                    .changed
                    .iter()
                    .zip(&original.program.locals)
                    .enumerate()
                {
                    out.budget.charge_work(1)?;
                    if changed {
                        let local = body
                            .locals
                            .start
                            .checked_add(local)
                            .ok_or(Resource::Arithmetic)?;
                        emit!(out, ".update({local}int, {})", node.is_some());
                    }
                }
                emit!(
                    out,
                    ";\n let post = OriginalControlStateV31 {{ pc: {key}int, values, defined, cells: s.cells, initialized: s.initialized, external: s.external }};\n let state = "
                );
                match &original.branch {
                    Branch::Call { transfer, .. } => {
                        let call = model.transfer(*transfer, out)?;
                        if call.site != key {
                            return Err(super::mismatch());
                        }
                        emit!(out, "original_call_enter_{key}_v33(post)");
                    }
                    Branch::Return if body.returned.is_some() => {
                        let call =
                            model.transfer(body.returned.ok_or(Resource::Accounting)?, out)?;
                        emit!(out, "original_call_return_{}_v33(post)", call.site);
                    }
                    branch => {
                        emit!(out, "OriginalControlStateV31 {{ pc: ");
                        match branch {
                            Branch::Goto(next) => emit!(
                                out,
                                "{}int",
                                body.blocks
                                    .start
                                    .checked_add(next.get() as usize)
                                    .ok_or(Resource::Arithmetic)?
                            ),
                            Branch::Switch {
                                selector,
                                cases,
                                otherwise,
                            } => {
                                for &(value, next) in cases {
                                    out.budget.charge_work(1)?;
                                    emit!(
                                        out,
                                        "if n[{selector}] == {value}int {{ {}int }} else ",
                                        body.blocks
                                            .start
                                            .checked_add(next.get() as usize)
                                            .ok_or(Resource::Arithmetic)?
                                    );
                                }
                                emit!(
                                    out,
                                    "{}int",
                                    body.blocks
                                        .start
                                        .checked_add(otherwise.get() as usize)
                                        .ok_or(Resource::Arithmetic)?
                                );
                            }
                            Branch::Return => emit!(out, "-1int"),
                            Branch::Call { .. } => return Err(Resource::Accounting.into()),
                        }
                        emit!(
                            out,
                            ", values: post.values, defined: post.defined, cells: post.cells, initialized: post.initialized, external: post.external }}"
                        );
                    }
                }
                emit!(out, ";\n CfgStepV26 {{ state, events: seq![");
                for assignment in &original.program.assignments {
                    out.budget.charge_work(1)?;
                    emit!(out, "n[{}],", assignment.value);
                }
                if let Some(value) = original.program.returned {
                    emit!(out, "n[{value}],");
                }
                emit!(
                    out,
                    "], halted: {} }}\n }} else\n",
                    matches!(original.branch, Branch::Return) && body.returned.is_none()
                );
            }
        }
        emit!(
            out,
            " {{ CfgStepV26 {{ state: s, events: Seq::empty(), halted: true }} }}\n}}\n"
        );
    }
    Ok(())
}

pub(super) fn headers() -> usize {
    size_of::<(&InvocationBodies<'_, '_, '_, '_>, &mut Writer<'_, '_>)>()
        + size_of::<(&Body, &super::super::control::SourceBlock)>()
        + size_of::<&super::super::call_transfers::DirectTransfer>()
        + size_of::<std::slice::Iter<'_, Option<Body>>>()
        + size_of::<std::slice::Iter<'_, super::super::control::LiveIn>>()
        + size_of::<std::slice::Iter<'_, super::super::AssignmentV30>>()
        + 8 * size_of::<Result<()>>()
        + 14 * size_of::<usize>()
        + size_of::<u128>()
        + size_of::<ExpressionV30>()
}

//! Whole actual Rust MIR census for declarative physical-lds-exchange primitives.
//! The Rust body is one acyclic marker chain; this does not execute native steps.
use crate::production_physical_lds_exchange_call_v39::{ActualPhysicalLdsExchangeCallV39, observe};
use fe2o3_mir_model::semantic_mir_v1::{
    SEMANTIC_PHYSICAL_LDS_EXCHANGE_MAX_OCCURRENCES_V39 as MAX_OCCURRENCES,
    SemanticCompilerIntrinsicOperationV1 as Operation,
};
use rustc_middle::mir::{
    Body, Operand, Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{Instance, TyCtxt};
#[path = "production_physical_lds_exchange_transport_v39.rs"]
mod transport;
use transport::ArgumentTransport;

pub(crate) const MAX_SOURCE_BLOCKS: usize = 4096;
pub(crate) const MAX_SOURCE_ITEMS: usize = 65_536;

#[derive(Clone, Copy)]
pub(crate) struct Occurrence<'tcx> {
    pub(crate) raw_block: u32,
    pub(crate) actual: ActualPhysicalLdsExchangeCallV39<'tcx>,
}
pub(crate) struct Census<'tcx> {
    calls: [Option<Occurrence<'tcx>>; MAX_OCCURRENCES],
    count: usize,
}
impl<'tcx> Census<'tcx> {
    pub(crate) fn calls(&self) -> impl Iterator<Item = Occurrence<'tcx>> + '_ {
        self.calls[..self.count].iter().copied().flatten()
    }
    pub(crate) fn len(&self) -> usize {
        self.count
    }
}

/// Checked before whole-body hashing and traversal. Fixed stack storage only;
/// rustc queries and compiler RSS are not measured by this logical bound.
pub(crate) fn require_body_bounds(body: &Body<'_>) -> Result<usize, &'static str> {
    if body.basic_blocks.is_empty()
        || body.basic_blocks.len() > MAX_SOURCE_BLOCKS
        || !(3..=transport::MAX_LOCALS).contains(&body.local_decls.len())
    {
        return Err("physical-lds-exchange source body/local bound exceeded");
    }
    let mut items = body.local_decls.len();
    for block in body.basic_blocks.iter() {
        items = items
            .checked_add(block.statements.len())
            .and_then(|n| n.checked_add(1))
            .ok_or("physical-lds-exchange source item overflow")?;
        if items > MAX_SOURCE_ITEMS {
            return Err("physical-lds-exchange source item bound exceeded");
        }
    }
    Ok(items)
}

pub(crate) fn observe_root<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    body: &Body<'tcx>,
) -> Result<Census<'tcx>, &'static str> {
    require_body_bounds(body)?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(root.def_id()).instantiate(tcx, root.args),
    );
    if body.arg_count != 2
        || !body.return_ty().is_unit()
        || !crate::production_physical_lds_exchange_call_v39::valid_root_signature(tcx, &signature)
    {
        return Err("physical-lds-exchange root signature differs");
    }
    for (index, ty) in signature.inputs().iter().enumerate() {
        if body
            .local_decls
            .get(rustc_middle::mir::Local::from_usize(index + 1))
            .is_none_or(|local| local.ty != *ty)
        {
            return Err("physical-lds-exchange root MIR argument type differs");
        }
    }
    let mut transport = ArgumentTransport::new(body.local_decls.len())?;
    let mut visited = [false; MAX_SOURCE_BLOCKS];
    let mut census = Census {
        calls: [None; MAX_OCCURRENCES],
        count: 0,
    };
    let mut next = START_BLOCK;
    let mut labels = 0usize;
    let mut native = 0usize;
    let mut ended = false;
    loop {
        let block = body
            .basic_blocks
            .get(next)
            .ok_or("physical-lds-exchange source edge leaves body")?;
        if visited[next.index()] || block.is_cleanup {
            return Err("physical-lds-exchange source chain cycles or enters cleanup");
        }
        visited[next.index()] = true;
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::Nop => {}
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                    transport.storage(local.index())?
                }
                StatementKind::Assign(assignment) => {
                    let (destination, value) = &**assignment;
                    if !destination.projection.is_empty() {
                        return Err("physical-lds-exchange source writes a projection");
                    }
                    let Rvalue::Use(operand) = value else {
                        return Err(
                            "physical-lds-exchange source computes outside authored instructions",
                        );
                    };
                    if destination.local.as_u32() == 0 {
                        if !matches!(operand,Operand::Constant(value) if value.const_.ty().is_unit())
                        {
                            return Err("physical-lds-exchange return assignment is not unit");
                        }
                        continue;
                    }
                    if census.count != 0 {
                        return Err(
                            "physical-lds-exchange source performs argument transport after begin",
                        );
                    }
                    let (source, moved) = match operand {
                        Operand::Copy(place) => (place, false),
                        Operand::Move(place) => (place, true),
                        _ => {
                            return Err(
                                "physical-lds-exchange source contains a foreign argument assignment",
                            );
                        }
                    };
                    if !source.projection.is_empty()
                        || destination.ty(&body.local_decls, tcx).ty
                            != source.ty(&body.local_decls, tcx).ty
                    {
                        return Err(
                            "physical-lds-exchange source argument transport projects or changes type",
                        );
                    }
                    transport.assign(destination.local.index(), source.local.index(), moved)?;
                }
                _ => return Err("physical-lds-exchange source has an unadmitted statement"),
            }
        }
        next = match &block.terminator().kind {
            TerminatorKind::Goto { target } => *target,
            TerminatorKind::Call {
                func,
                args,
                destination,
                target: Some(target),
                unwind: UnwindAction::Continue | UnwindAction::Unreachable,
                ..
            } => {
                if census.count == MAX_OCCURRENCES
                    || !destination.projection.is_empty()
                    || !destination.ty(&body.local_decls, tcx).ty.is_unit()
                {
                    return Err(
                        "physical-lds-exchange marker count or unit call destination differs",
                    );
                }
                let actual = match args.len() {
                    0 => observe(tcx, root, body, func, &[])?,
                    2 => observe(tcx, root, body, func, &[&args[0].node, &args[1].node])?,
                    _ => return Err("physical-lds-exchange marker runtime arity differs"),
                };
                match actual.operation() {
                    Operation::Gfx942PhysicalLdsExchangeBegin(_) if census.count == 0 => {
                        transport
                            .consume_marker(actual.argument_locals(), actual.moved_arguments())?;
                    }
                    Operation::Gfx942PhysicalLdsExchangeLabel(0) if census.count == 1 => {
                        labels += 1;
                        if labels > 1 {
                            return Err("physical-lds-exchange source exceeds one label");
                        }
                    }
                    Operation::Gfx942PhysicalLdsExchangeStep(step)
                        if census.count > 1 && labels != 0 =>
                    {
                        if ended {
                            return Err("physical-lds-exchange source step follows endpgm");
                        }
                        native += 1;
                        ended = step.opcode() == 14;
                        if native > 40 {
                            return Err("physical-lds-exchange native instruction bound exceeded");
                        }
                    }
                    _ => return Err("physical-lds-exchange begin/label/step source order differs"),
                }
                census.calls[census.count] = Some(Occurrence {
                    raw_block: next.as_u32(),
                    actual,
                });
                census.count += 1;
                *target
            }
            TerminatorKind::Return => {
                transport.require_marker()?;
                if labels != 1
                    || native == 0
                    || !ended
                    || visited[..body.basic_blocks.len()].iter().any(|v| !*v)
                {
                    return Err(
                        "physical-lds-exchange source labels, instructions or complete chain census differ",
                    );
                }
                return Ok(census);
            }
            _ => {
                return Err(
                    "physical-lds-exchange source has a foreign call, Rust branch or hidden effect",
                );
            }
        };
    }
}

//! Independent actual-body relation to exactly one input-to-output integer cast.
use super::scratch::{Fact, Scratch};
use super::*;
use rustc_middle::mir::{BasicBlock, CastKind, Operand, Rvalue, StatementKind, TerminatorKind};

fn operand<'tcx, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    scratch: &mut Scratch,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<Fact, E> {
    cx.work(1)?;
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            if !place.projection.is_empty()
                || scratch.aliases.get(place.local.index()) != Some(&false)
            {
                return Err(cx.semantic(site, Semantic::Unsupported));
            }
            let value = scratch
                .facts
                .get_mut(place.local.index())
                .ok_or_else(|| cx.semantic(site, Semantic::Coordinate))?;
            let result = *value;
            if matches!(operand, Operand::Move(_)) {
                *value = Fact::Unknown;
            }
            Ok(result)
        }
        Operand::Constant(value) => raw::constant(tcx, value, false, site, cx)?
            .map(Fact::Known)
            .ok_or_else(|| cx.semantic(site, Semantic::Unsupported)),
        _ => Err(cx.semantic(site, Semantic::Unsupported)),
    }
}
fn value<'tcx, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'tcx>,
    value: &Rvalue<'tcx>,
    output: Ty<'tcx>,
    scratch: &mut Scratch,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<Fact, E> {
    cx.work(1)?;
    let result = match value {
        Rvalue::Use(value) => operand(tcx, value, scratch, site, cx)?,
        Rvalue::Cast(CastKind::IntToInt, input, target) => {
            let input = operand(tcx, input, scratch, site, cx)?;
            cx.work(1)?;
            match input {
                Fact::Input if *target == output => Fact::ConvertedInput,
                Fact::Known(value) => Fact::Known(
                    value
                        .cast(
                            scalar::ty(*target)
                                .ok_or_else(|| cx.semantic(site, Semantic::InvalidScalar))?,
                        )
                        .ok_or_else(|| cx.semantic(site, Semantic::InvalidScalar))?,
                ),
                _ => return Err(cx.semantic(site, Semantic::Unsupported)),
            }
        }
        Rvalue::UnaryOp(operation, value) => {
            let Fact::Known(value) = operand(tcx, value, scratch, site, cx)? else {
                return Err(cx.semantic(site, Semantic::InvalidScalar));
            };
            cx.work(1)?;
            Fact::Known(
                value
                    .unary(*operation)
                    .ok_or_else(|| cx.semantic(site, Semantic::InvalidScalar))?,
            )
        }
        Rvalue::BinaryOp(operation, values) => {
            let a = operand(tcx, &values.0, scratch, site, cx)?;
            let b = operand(tcx, &values.1, scratch, site, cx)?;
            cx.work(1)?;
            let (Fact::Known(a), Fact::Known(b)) = (a, b) else {
                return Err(cx.semantic(site, Semantic::InvalidScalar));
            };
            Fact::Known(
                a.binary(*operation, b)
                    .ok_or_else(|| cx.semantic(site, Semantic::InvalidScalar))?,
            )
        }
        _ => return Err(cx.semantic(site, Semantic::Unsupported)),
    };
    if result == Fact::Unknown {
        return Err(cx.semantic(site, Semantic::InvalidScalar));
    }
    Ok(result)
}

pub(super) fn check<'tcx, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    input: Ty<'tcx>,
    output: Ty<'tcx>,
    cx: &mut Context<'_, E, F>,
) -> Result<(), E> {
    cx.work(1)?;
    if body.arg_count != 1
        || body.local_decls.len() < 2
        || body.basic_blocks.is_empty()
        || body.local_decls[mir_local(0)].ty != output
        || body.local_decls[mir_local(1)].ty != input
        || body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || !primitive_widening_types_v1(input, output)
    {
        return Err(cx.raw(RawSiteV1::Signature, Raw::Body));
    }
    cx.structural(SemanticMirResourceV1::Locals, body.local_decls.len())?;
    cx.structural(SemanticMirResourceV1::Blocks, body.basic_blocks.len())?;
    if body.basic_blocks.len() > fe2o3_mir_model::MAX_EXECUTABLE_BLOCKS {
        return Err(cx.resource(Resource::Structural {
            resource: SemanticMirResourceV1::Blocks,
            actual: u64::try_from(body.basic_blocks.len()).map_err(|_| {
                cx.resource(Resource::SizeOverflow {
                    phase: Phase {
                        slab: Slab::Structure,
                        backing: Backing::Requested,
                    },
                })
            })?,
            maximum: fe2o3_mir_model::MAX_EXECUTABLE_BLOCKS as u64,
        }));
    }
    let mut scratch = Scratch::new(body.local_decls.len(), body.basic_blocks.len(), cx)?;
    raw::audit(tcx, instance, body, &mut scratch, cx)?;
    cx.work(1)?;
    scratch.facts[1] = Fact::Input;
    let mut block = BasicBlock::from_usize(0);
    loop {
        cx.work(1)?;
        let site = RawSiteV1::Terminator(raw::ordinal(block.index(), cx)?);
        let visited = scratch
            .visited
            .get_mut(block.index())
            .ok_or_else(|| cx.semantic(site, Semantic::Coordinate))?;
        if *visited {
            return Err(cx.semantic(site, Semantic::Cycle));
        }
        *visited = true;
        // Constants are block-local. Runtime input provenance survives edges,
        // but Move, overwrite and lifetime transitions consume it normally.
        cx.work(scratch.facts.len())?;
        for value in &mut scratch.facts {
            if matches!(value, Fact::Known(_)) {
                *value = Fact::Unknown;
            }
        }
        let data = &body.basic_blocks[block];
        for (index, statement) in data.statements.iter().enumerate() {
            cx.work(1)?;
            let site = RawSiteV1::Statement {
                block: raw::ordinal(block.index(), cx)?,
                statement: raw::ordinal(index, cx)?,
            };
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let destination = assignment.0;
                    if !destination.projection.is_empty()
                        || scratch.aliases[destination.local.index()]
                    {
                        return Err(cx.semantic(site, Semantic::Unsupported));
                    }
                    let fact = value(tcx, &assignment.1, output, &mut scratch, site, cx)?;
                    let actual = body.local_decls[destination.local].ty;
                    let correct = match fact {
                        Fact::Known(value) => scalar::ty(actual) == Some(value.ty),
                        Fact::Input => actual == input,
                        Fact::ConvertedInput => actual == output,
                        Fact::Unknown => false,
                    };
                    cx.work(1)?;
                    if !correct {
                        return Err(cx.semantic(site, Semantic::InvalidScalar));
                    }
                    scratch.facts[destination.local.index()] = fact;
                }
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                    scratch.facts[local.index()] = Fact::Unknown
                }
                StatementKind::Nop => {}
                _ => return Err(cx.semantic(site, Semantic::Unsupported)),
            }
        }
        cx.work(1)?;
        match &data
            .terminator
            .as_ref()
            .ok_or_else(|| cx.semantic(site, Semantic::Coordinate))?
            .kind
        {
            TerminatorKind::Goto { target } => {
                cx.work(1)?;
                block = *target;
            }
            TerminatorKind::SwitchInt { discr, targets } => {
                let Fact::Known(value) = operand(tcx, discr, &mut scratch, site, cx)? else {
                    return Err(cx.semantic(site, Semantic::UnknownBranch));
                };
                let mut selected = targets.otherwise();
                for (bits, target) in targets.iter() {
                    cx.work(1)?;
                    if scalar::Scalar::new(value.ty, bits).is_none() {
                        return Err(cx.semantic(site, Semantic::InvalidScalar));
                    }
                    if bits == value.bits {
                        selected = target;
                        break;
                    }
                }
                cx.work(1)?;
                block = selected;
            }
            TerminatorKind::Return => {
                cx.work(1)?;
                if scratch.facts[0] != Fact::ConvertedInput {
                    return Err(cx.semantic(site, Semantic::WrongReturn));
                }
                return Ok(());
            }
            _ => return Err(cx.semantic(site, Semantic::Unsupported)),
        }
    }
}
fn mir_local(index: usize) -> rustc_middle::mir::Local {
    rustc_middle::mir::Local::from_usize(index)
}

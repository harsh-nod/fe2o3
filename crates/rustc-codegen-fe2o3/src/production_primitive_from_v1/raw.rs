//! Exhaustive closed raw grammar, including unexecuted blocks and required constants.
use super::*;
use rustc_abi::Size;
use rustc_hir::Mutability;
use rustc_middle::{
    mir::{
        self, BinOp, CastKind, Const, ConstOperand, ConstValue, Operand, Place, Rvalue,
        StatementKind, TerminatorKind, UnwindAction,
        interpret::{AllocRange, GlobalAlloc},
    },
    ty::{ConstKind, TyKind},
};

pub(super) fn ordinal<E, F: FnMut(usize) -> std::result::Result<(), E>>(
    n: usize,
    cx: &Context<'_, E, F>,
) -> Result<u32, E> {
    u32::try_from(n).map_err(|_| {
        cx.resource(Resource::SizeOverflow {
            phase: Phase {
                slab: Slab::Structure,
                backing: Backing::Requested,
            },
        })
    })
}
fn definition<E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'_>,
    def: rustc_hir::def_id::DefId,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<(), E> {
    cx.work(1)?;
    let core = tcx
        .lang_items()
        .sized_trait()
        .ok_or_else(|| cx.raw(site, Raw::ConstantIdentity))?
        .krate;
    if def.krate != core
        || tcx.crate_name(core).as_str() != "core"
        || crate::trusted_device_items::rejected_provider(tcx, def).is_some()
        || !matches!(
            crate::device_ffi::contract_assertion_for_def(tcx, def),
            Ok(None)
        )
    {
        return Err(cx.raw(site, Raw::ConstantIdentity));
    }
    Ok(())
}

pub(super) fn constant<'tcx, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'tcx>,
    constant: &ConstOperand<'tcx>,
    function: bool,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<Option<scalar::Scalar>, E> {
    cx.work(1)?;
    match constant.const_ {
        Const::Unevaluated(value, _) => {
            if !value.args.is_empty() || value.promoted.is_some() {
                return Err(cx.raw(site, Raw::ConstantIdentity));
            }
            definition(tcx, value.def, site, cx)?;
        }
        Const::Ty(_, value) => match value.kind() {
            ConstKind::Unevaluated(value) => {
                if !value.args.is_empty() {
                    return Err(cx.raw(site, Raw::ConstantIdentity));
                }
                definition(tcx, value.def, site, cx)?;
            }
            ConstKind::Value(_) => {}
            _ => return Err(cx.raw(site, Raw::ConstantIdentity)),
        },
        Const::Val(_, _) => {}
    }
    let ty = constant.const_.ty();
    if function {
        let TyKind::FnDef(def, args) = ty.kind() else {
            return Err(cx.raw(site, Raw::Type));
        };
        if !args.is_empty() || !matches!(constant.const_, Const::Val(ConstValue::ZeroSized, _)) {
            return Err(cx.raw(site, Raw::ConstantValue));
        }
        definition(tcx, *def, site, cx)?;
    }
    cx.work(1)?;
    // Const::eval requires type-system constants to be normalized already.
    // Charge evaluation admission, but never invoke rustc's delayed-bug path.
    if matches!(constant.const_, Const::Ty(_, value) if !matches!(value.kind(), ConstKind::Value(_)))
    {
        return Err(cx.raw(site, Raw::ConstantEvaluation));
    }
    let evaluated = constant
        .const_
        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
        .map_err(|_| cx.raw(site, Raw::ConstantEvaluation))?;
    if let Some(ty) = scalar::ty(ty) {
        let ConstValue::Scalar(value) = evaluated else {
            return Err(cx.raw(site, Raw::ConstantValue));
        };
        let value = value
            .try_to_scalar_int()
            .map_err(|_| cx.raw(site, Raw::ConstantValue))?;
        let expected = if ty == fe2o3_kernel_ir::scalar_ops_v2::ScalarType::Bool {
            8
        } else {
            u64::from(ty.bit_width())
        };
        if value.size().bits() != expected {
            return Err(cx.raw(site, Raw::ConstantValue));
        }
        return scalar::Scalar::new(ty, value.to_bits(value.size()))
            .map(Some)
            .ok_or_else(|| cx.raw(site, Raw::ConstantValue));
    }
    match (ty.kind(), evaluated) {
        (TyKind::FnDef(_, _), ConstValue::ZeroSized) if function => Ok(None),
        (TyKind::Ref(_, target, Mutability::Not), ConstValue::Slice { alloc_id, meta })
            if target.is_str() =>
        {
            // Only actual literal data; no pointer-valued CTFE proof facts.
            if !matches!(constant.const_, Const::Val(ConstValue::Slice { .. }, _)) {
                return Err(cx.raw(site, Raw::LiteralAllocation));
            }
            cx.work(1)?;
            let GlobalAlloc::Memory(allocation) = tcx.global_alloc(alloc_id) else {
                return Err(cx.raw(site, Raw::LiteralAllocation));
            };
            let allocation = allocation.inner();
            let len = usize::try_from(meta).map_err(|_| cx.raw(site, Raw::LiteralAllocation))?;
            if allocation.mutability != Mutability::Not
                || meta > allocation.size().bytes()
                || !allocation.provenance().ptrs().is_empty()
            {
                return Err(cx.raw(site, Raw::LiteralAllocation));
            }
            cx.structural(SemanticMirResourceV1::ConstantBytes, allocation.len())?;
            cx.work(allocation.len())?;
            if allocation
                .init_mask()
                .is_range_initialized(AllocRange {
                    start: Size::ZERO,
                    size: allocation.size(),
                })
                .is_err()
            {
                return Err(cx.raw(site, Raw::LiteralBytes));
            }
            let bytes = allocation.inspect_with_uninit_and_ptr_outside_interpreter(0..len);
            std::str::from_utf8(bytes).map_err(|_| cx.raw(site, Raw::LiteralBytes))?;
            Ok(None)
        }
        _ => Err(cx.raw(site, Raw::ConstantValue)),
    }
}

pub(super) fn place<E, F: FnMut(usize) -> std::result::Result<(), E>>(
    place: Place<'_>,
    body: &Body<'_>,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<usize, E> {
    cx.work(1)?;
    cx.structural(SemanticMirResourceV1::Projections, place.projection.len())?;
    if !place.projection.is_empty() || place.local.index() >= body.local_decls.len() {
        return Err(cx.raw(site, Raw::Place));
    }
    Ok(place.local.index())
}
fn operand<'tcx, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    body: &Body<'tcx>,
    function: bool,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<(), E> {
    cx.work(1)?;
    cx.structural(SemanticMirResourceV1::Operands, 1)?;
    match operand {
        Operand::Copy(p) | Operand::Move(p) if !function => {
            place(*p, body, site, cx)?;
        }
        Operand::Constant(c) => {
            constant(tcx, c, function, site, cx)?;
        }
        _ => return Err(cx.raw(site, Raw::Place)),
    }
    Ok(())
}
fn edge<E, F: FnMut(usize) -> std::result::Result<(), E>>(
    target: mir::BasicBlock,
    body: &Body<'_>,
    site: RawSiteV1,
    cx: &mut Context<'_, E, F>,
) -> Result<(), E> {
    cx.work(1)?;
    if target.index() >= body.basic_blocks.len() {
        return Err(cx.raw(site, Raw::Edge));
    }
    Ok(())
}
pub(super) fn audit<'tcx, E, F: FnMut(usize) -> std::result::Result<(), E>>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    scratch: &mut scratch::Scratch,
    cx: &mut Context<'_, E, F>,
) -> Result<(), E> {
    definition(tcx, instance.def_id(), RawSiteV1::Signature, cx)?;
    for (local, data) in body.local_decls.iter_enumerated() {
        cx.work(1)?;
        if scalar::ty(data.ty).is_none() && !data.ty.is_never() {
            return Err(cx.raw(RawSiteV1::Local(ordinal(local.index(), cx)?), Raw::Type));
        }
    }
    let required = body
        .required_consts
        .as_ref()
        .ok_or_else(|| cx.raw(RawSiteV1::Signature, Raw::Body))?;
    cx.structural(SemanticMirResourceV1::Operands, required.len())?;
    for (index, value) in required.iter().enumerate() {
        constant(
            tcx,
            value,
            false,
            RawSiteV1::RequiredConstant(ordinal(index, cx)?),
            cx,
        )?;
    }
    // One complete alias census precedes scalar/path analysis. Any address-taking
    // syntax then fails the closed raw grammar, including in an unselected block.
    for (block, data) in body.basic_blocks.iter_enumerated() {
        cx.work(1)?;
        for (statement, value) in data.statements.iter().enumerate() {
            cx.work(1)?;
            if let StatementKind::Assign(pair) = &value.kind
                && let Rvalue::Ref(_, _, p) | Rvalue::RawPtr(_, p) = &pair.1
            {
                let site = RawSiteV1::Statement {
                    block: ordinal(block.index(), cx)?,
                    statement: ordinal(statement, cx)?,
                };
                let index = place(*p, body, site, cx)?;
                scratch.aliases[index] = true;
            }
        }
    }
    for (block, data) in body.basic_blocks.iter_enumerated() {
        cx.work(1)?;
        let block = ordinal(block.index(), cx)?;
        if data.is_cleanup {
            return Err(cx.raw(RawSiteV1::Terminator(block), Raw::Unwind));
        }
        cx.structural(SemanticMirResourceV1::Statements, data.statements.len())?;
        for (statement, value) in data.statements.iter().enumerate() {
            cx.work(1)?;
            let site = RawSiteV1::Statement {
                block,
                statement: ordinal(statement, cx)?,
            };
            match &value.kind {
                StatementKind::Assign(pair) => {
                    place(pair.0, body, site, cx)?;
                    match &pair.1 {
                        Rvalue::Use(a) | Rvalue::UnaryOp(mir::UnOp::Not | mir::UnOp::Neg, a) => {
                            operand(tcx, a, body, false, site, cx)?
                        }
                        Rvalue::Cast(CastKind::IntToInt, a, to) if scalar::ty(*to).is_some() => {
                            operand(tcx, a, body, false, site, cx)?
                        }
                        Rvalue::BinaryOp(
                            BinOp::Add
                            | BinOp::Sub
                            | BinOp::Mul
                            | BinOp::Div
                            | BinOp::Rem
                            | BinOp::BitAnd
                            | BinOp::BitOr
                            | BinOp::BitXor
                            | BinOp::Shl
                            | BinOp::Shr
                            | BinOp::Eq
                            | BinOp::Ne
                            | BinOp::Lt
                            | BinOp::Le
                            | BinOp::Gt
                            | BinOp::Ge,
                            operands,
                        ) => {
                            operand(tcx, &operands.0, body, false, site, cx)?;
                            operand(tcx, &operands.1, body, false, site, cx)?;
                        }
                        _ => return Err(cx.raw(site, Raw::Rvalue)),
                    }
                }
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local)
                    if local.index() < body.local_decls.len() => {}
                StatementKind::Nop => {}
                _ => return Err(cx.raw(site, Raw::Statement)),
            }
        }
        let site = RawSiteV1::Terminator(block);
        cx.work(1)?;
        match &data
            .terminator
            .as_ref()
            .ok_or_else(|| cx.raw(site, Raw::Terminator))?
            .kind
        {
            TerminatorKind::Goto { target } => edge(*target, body, site, cx)?,
            TerminatorKind::SwitchInt { discr, targets } => {
                operand(tcx, discr, body, false, site, cx)?;
                cx.structural(
                    SemanticMirResourceV1::SwitchTargets,
                    targets.all_targets().len(),
                )?;
                for target in targets.all_targets() {
                    edge(*target, body, site, cx)?;
                }
            }
            TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                unwind,
                ..
            } => {
                if !matches!(unwind, UnwindAction::Unreachable | UnwindAction::Continue) {
                    return Err(cx.raw(site, Raw::Unwind));
                }
                operand(tcx, func, body, true, site, cx)?;
                let result = crate::production_raw_call_audit_v1::audit_raw_call_v1(
                    tcx,
                    instance,
                    func,
                    &mut |n| cx.work(n),
                )?;
                let callee = result.map_err(|r| cx.raw(site, Raw::Call(r)))?;
                cx.work(1)?;
                let signature = crate::rustc_semantic_plan_v1::source_signature_v1(tcx, callee)
                    .map_err(|_| cx.raw(site, Raw::Type))?;
                place(*destination, body, site, cx)?;
                if args.len() != signature.inputs().len()
                    || destination.ty(&body.local_decls, tcx).ty != signature.output()
                {
                    return Err(cx.raw(site, Raw::Type));
                }
                cx.structural(SemanticMirResourceV1::CallArguments, args.len())?;
                for (argument, expected) in args.iter().zip(signature.inputs()) {
                    cx.work(1)?;
                    operand(tcx, &argument.node, body, false, site, cx)?;
                    if argument.node.ty(&body.local_decls, tcx) != *expected {
                        return Err(cx.raw(site, Raw::Type));
                    }
                }
                if let Some(target) = target {
                    edge(*target, body, site, cx)?;
                }
            }
            TerminatorKind::Return | TerminatorKind::Unreachable => {}
            _ => return Err(cx.raw(site, Raw::Terminator)),
        }
    }
    Ok(())
}

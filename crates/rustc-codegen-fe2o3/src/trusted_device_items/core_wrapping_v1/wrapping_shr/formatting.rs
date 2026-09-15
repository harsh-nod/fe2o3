//! Closed construction of the masked-shift precondition's failure message.
//! These helpers are checked only as part of the safe-entry proof, never
//! admitted as standalone device functions or used to waive a reachable panic.

use super::retained::{bb, call, no_required_consts, operand, returns, shape};
use super::*;
use rustc_middle::mir::interpret::{GlobalAlloc, alloc_range};
use rustc_middle::mir::{AggregateKind, AssertMessage, CastKind, ProjectionElem, RawPtrKind, UnOp};
use rustc_span::Symbol;

const PRECONDITION_MESSAGE: &[u8] = b"unsafe precondition(s) violated: u32::unchecked_shr cannot overflow\n\nThis indicates a bug in the program. This Undefined Behavior check is optional, and cannot be relied on for safety.";

fn text_ty(tcx: TyCtxt<'_>) -> Ty<'_> {
    Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, tcx.types.str_)
}

fn bytes_ty(tcx: TyCtxt<'_>) -> Ty<'_> {
    Ty::new_imm_ref(
        tcx,
        tcx.lifetimes.re_erased,
        Ty::new_slice(tcx, tcx.types.u8),
    )
}

fn format_ty(tcx: TyCtxt<'_>) -> Option<Ty<'_>> {
    Some(Ty::new_adt(
        tcx,
        tcx.adt_def(tcx.lang_items().format_arguments()?),
        tcx.mk_args(&[tcx.lifetimes.re_erased.into()]),
    ))
}

pub(super) fn identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, helper: Helper) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let def = instance.def_id();
    if !matches!(instance.def, InstanceKind::Item(_))
        || def.krate != core.krate
        || tcx.crate_name(core.krate).as_str() != "core"
        || tcx.def_kind(def) != DefKind::AssocFn
        || !tcx.is_mir_available(def)
    {
        return false;
    }
    let Some(implementation) = tcx.impl_of_assoc(def) else {
        return false;
    };
    let Some(format) = format_ty(tcx) else {
        return false;
    };
    let (name, input, output) = match helper {
        Helper::FormatFromStr => (
            "from_str",
            Ty::new_imm_ref(tcx, tcx.lifetimes.re_static, tcx.types.str_),
            format,
        ),
        Helper::StrAsPtr => ("as_ptr", text_ty(tcx), Ty::new_imm_ptr(tcx, tcx.types.u8)),
        Helper::StrLen => ("len", text_ty(tcx), tcx.types.usize),
        Helper::StrAsBytes => ("as_bytes", text_ty(tcx), bytes_ty(tcx)),
        _ => return false,
    };
    let from_str = helper == Helper::FormatFromStr;
    if if from_str {
        instance.args.as_slice() != [tcx.lifetimes.re_erased.into()]
    } else {
        !instance.args.is_empty()
    } {
        return false;
    }
    let sig = signature(tcx, instance);
    implementation.krate == core.krate
        && !tcx.impl_is_of_trait(implementation)
        && tcx.item_name(def).as_str() == name
        && if from_str {
            matches!((tcx.type_of(implementation).instantiate_identity().kind(), format.kind()),
                    (TyKind::Adt(actual, _), TyKind::Adt(expected, _)) if actual == expected)
        } else {
            tcx.type_of(implementation).instantiate_identity() == tcx.types.str_
        }
        && sig.safety == Safety::Safe
        && sig.abi == ExternAbi::Rust
        && !sig.c_variadic
        && sig.inputs() == [input]
        && sig.output() == output
}

pub(super) fn precondition<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    let Some(format) = format_ty(tcx) else {
        return false;
    };
    shape(
        body,
        1,
        &[
            tcx.types.unit,
            tcx.types.u32,
            tcx.types.bool,
            tcx.types.never,
            format,
            text_ty(tcx),
        ],
        &[1, 0, 1, 0],
        2,
    ) && retained::required_bits(tcx, body, 1)
        && matches!(assignment(&bb(body, 0).statements[0], 2), Some(Rvalue::BinaryOp(BinOp::Lt, operands))
            if operand(&operands.0, 1, false) && retained::bits(tcx, &operands.1))
        && retained::switch_local(body, 0, 2, 2, 1)
        && returns(body, 1)
        && matches!(assignment(&bb(body, 2).statements[0], 5), Some(Rvalue::Use(value))
            if precondition_message(tcx, value))
        && panic_call(tcx, body, 3, 4, 3, format)
        && call(
            tcx,
            body,
            2,
            4,
            3,
            &[(5, true)],
            Helper::FormatFromStr,
            fetch,
            budget,
        )
}

fn precondition_message<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> bool {
    let Operand::Constant(constant) = operand else {
        return false;
    };
    let Const::Val(ConstValue::Slice { alloc_id, meta }, ty) = constant.const_ else {
        return false;
    };
    if ty != text_ty(tcx) || meta != PRECONDITION_MESSAGE.len() as u64 {
        return false;
    }
    let Some(GlobalAlloc::Memory(allocation)) = tcx.try_get_global_alloc(alloc_id) else {
        return false;
    };
    let allocation = allocation.inner();
    let length = rustc_abi::Size::from_bytes(meta);
    // Read only bounded, initialized bytes. Diagnostic-only allocation reads
    // can expose uninitialized data and are unsuitable for authentication.
    allocation.mutability == rustc_hir::Mutability::Not
        && allocation.size() >= length
        && allocation
            .get_bytes_strip_provenance(&tcx, alloc_range(rustc_abi::Size::ZERO, length))
            .is_ok_and(|bytes| bytes == PRECONDITION_MESSAGE)
}

fn panic_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    block: usize,
    fmt: usize,
    dest: usize,
    format: Ty<'tcx>,
) -> bool {
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &bb(body, block).terminator().kind
    else {
        return false;
    };
    let Some(panic) = resolve(tcx, func) else {
        return false;
    };
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    if !matches!(panic.def, InstanceKind::Item(_))
        || panic.def_id().krate != core.krate
        || tcx.crate_name(core.krate).as_str() != "core"
        || !panic.args.is_empty()
        || tcx.def_kind(panic.def_id()) != DefKind::Fn
        || tcx.def_path_str(panic.def_id()) != "core::panicking::panic_nounwind_fmt"
    {
        return false;
    }
    let sig = signature(tcx, panic);
    sig.safety == Safety::Safe
        && sig.abi == ExternAbi::Rust
        && !sig.c_variadic
        && sig.inputs() == [format, tcx.types.bool]
        && sig.output() == tcx.types.never
        && matches!(&args[..], [message, flag] if operand(&message.node, fmt, true)
            && scalar(tcx, &flag.node, tcx.types.bool) == Some(0))
        && local(*destination, dest)
        && target.is_none()
        && matches!(unwind, UnwindAction::Unreachable)
}

pub(super) fn check<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    helper: Helper,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    if !no_required_consts(body) {
        return false;
    }
    match helper {
        Helper::FormatFromStr => from_str(tcx, body, fetch, budget),
        Helper::StrAsPtr => {
            shape(
                body,
                1,
                &[
                    Ty::new_imm_ptr(tcx, tcx.types.u8),
                    text_ty(tcx),
                    Ty::new_imm_ptr(tcx, tcx.types.str_),
                ],
                &[2],
                1,
            ) && matches!(assignment(&bb(body, 0).statements[0], 2), Some(Rvalue::RawPtr(RawPtrKind::Const, place))
                    if place.local.as_usize() == 1 && matches!(place.projection.as_ref(), [ProjectionElem::Deref]))
                && cast(
                    body,
                    0,
                    1,
                    0,
                    CastKind::PtrToPtr,
                    2,
                    true,
                    Ty::new_imm_ptr(tcx, tcx.types.u8),
                )
                && returns(body, 0)
        }
        Helper::StrLen => {
            shape(
                body,
                1,
                &[tcx.types.usize, text_ty(tcx), bytes_ty(tcx)],
                &[0, 1],
                1,
            ) && matches!(assignment(&bb(body, 1).statements[0], 0), Some(Rvalue::UnaryOp(UnOp::PtrMetadata, value))
                    if operand(value, 2, false))
                && returns(body, 1)
                && call(
                    tcx,
                    body,
                    0,
                    2,
                    1,
                    &[(1, false)],
                    Helper::StrAsBytes,
                    fetch,
                    budget,
                )
        }
        Helper::StrAsBytes => {
            shape(body, 1, &[bytes_ty(tcx), text_ty(tcx)], &[1], 1)
                && cast(body, 0, 0, 0, CastKind::Transmute, 1, false, bytes_ty(tcx))
                && returns(body, 0)
        }
        _ => false,
    }
}

fn from_str<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    let Some(format) = format_ty(tcx) else {
        return false;
    };
    let Some(nonnull) = tcx.get_diagnostic_item(Symbol::intern("NonNull")) else {
        return false;
    };
    let Some(decl) = body.local_decls.get(Local::from_usize(4)) else {
        return false;
    };
    let TyKind::Adt(actual_nonnull, args) = decl.ty.kind() else {
        return false;
    };
    let [argument] = args.as_slice() else {
        return false;
    };
    let Some(argument) = argument.as_type() else {
        return false;
    };
    let TyKind::Adt(argument_adt, args) = argument.kind() else {
        return false;
    };
    if actual_nonnull.did() != nonnull
        || argument_adt.did().krate != nonnull.krate
        || tcx.def_path_str(argument_adt.did()) != "core::fmt::rt::Argument"
        || args.as_slice() != [tcx.lifetimes.re_erased.into()]
        || tcx.data_layout.pointer_size().bits() != 64
    {
        return false;
    }
    let template = Ty::new_adt(tcx, *actual_nonnull, tcx.mk_args(&[tcx.types.u8.into()]));
    let encoded = Ty::new_adt(tcx, *actual_nonnull, tcx.mk_args(&[argument.into()]));
    if !shape(
        body,
        1,
        &[
            format,
            text_ty(tcx),
            template,
            Ty::new_imm_ptr(tcx, tcx.types.u8),
            encoded,
            tcx.types.usize,
            tcx.types.usize,
            tcx.types.usize,
            tcx.types.u32,
            tcx.types.bool,
        ],
        &[0, 1, 2, 4],
        1,
    ) || !cast(body, 1, 0, 2, CastKind::Transmute, 3, true, template)
        || !matches!(assignment(&bb(body, 2).statements[0], 8), Some(Rvalue::Cast(CastKind::IntToInt, value, ty))
            if *ty == tcx.types.u32 && scalar(tcx, value, tcx.types.i32) == Some(1))
        || !matches!(assignment(&bb(body, 2).statements[1], 9), Some(Rvalue::BinaryOp(BinOp::Lt, operands))
            if operand(&operands.0, 8, true) && scalar(tcx, &operands.1, tcx.types.u32) == Some(64))
    {
        return false;
    }
    let TerminatorKind::Assert {
        cond,
        expected,
        msg,
        target,
        unwind,
    } = &bb(body, 2).terminator().kind
    else {
        return false;
    };
    // The retained assertion checks the constant shift amount, not the string
    // length. Its original condition and both diagnostic operands are exact.
    if !*expected
        || !operand(cond, 9, true)
        || target.as_usize() != 3
        || !matches!(unwind, UnwindAction::Unreachable)
        || !matches!(&**msg, AssertMessage::Overflow(BinOp::Shl, left, right)
            if operand(left, 7, false) && scalar(tcx, right, tcx.types.i32) == Some(1))
        || !matches!(assignment(&bb(body, 3).statements[0], 6), Some(Rvalue::BinaryOp(BinOp::Shl, operands))
            if operand(&operands.0, 7, true) && scalar(tcx, &operands.1, tcx.types.i32) == Some(1))
        || !matches!(assignment(&bb(body, 3).statements[1], 5), Some(Rvalue::BinaryOp(BinOp::BitOr, operands))
            if operand(&operands.0, 6, true) && scalar(tcx, &operands.1, tcx.types.usize) == Some(1))
        || !cast(body, 3, 2, 4, CastKind::Transmute, 5, true, encoded)
        || !matches!(assignment(&bb(body, 3).statements[3], 0), Some(Rvalue::Aggregate(kind, fields))
            if matches!(&**kind, AggregateKind::Adt(def, variant, args, user, field)
                if Some(*def) == tcx.lang_items().format_arguments() && variant.as_usize() == 0
                    && args.as_slice() == [tcx.lifetimes.re_erased.into()] && user.is_none() && field.is_none())
                && matches!(fields.raw.as_slice(), [left, right] if operand(left, 2, true) && operand(right, 4, true)))
        || !returns(body, 3)
    {
        return false;
    }
    call(
        tcx,
        body,
        0,
        3,
        1,
        &[(1, false)],
        Helper::StrAsPtr,
        fetch,
        budget,
    ) && call(
        tcx,
        body,
        1,
        7,
        2,
        &[(1, false)],
        Helper::StrLen,
        fetch,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn cast<'tcx>(
    body: &Body<'tcx>,
    block: usize,
    statement: usize,
    dest: usize,
    kind: CastKind,
    input: usize,
    moved: bool,
    ty: Ty<'tcx>,
) -> bool {
    matches!(assignment(&bb(body, block).statements[statement], dest), Some(Rvalue::Cast(actual, value, target))
        if *actual == kind && *target == ty && operand(value, input, moved))
}

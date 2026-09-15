//! Exact nominal BITS and the retained precondition's complete failure branch.
//! Formatting callees reuse the existing closed proof and its caller's budget.

use super::*;
use rustc_middle::mir::interpret::{GlobalAlloc, alloc_range};

pub(super) fn bits<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    spec: ShiftSpec<'tcx>,
) -> bool {
    let Operand::Constant(constant) = operand else {
        return false;
    };
    constant.user_ty.is_none() && bits_const(tcx, constant.const_, spec)
}

fn bits_const<'tcx>(tcx: TyCtxt<'tcx>, constant: Const<'tcx>, spec: ShiftSpec<'tcx>) -> bool {
    let Const::Unevaluated(value, ty) = constant else {
        return false;
    };
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    let Some(implementation) = tcx.impl_of_assoc(value.def) else {
        return false;
    };
    ty == tcx.types.u32
        && value.def.krate == core.krate
        && tcx.crate_name(core.krate).as_str() == "core"
        && matches!(
            tcx.def_kind(value.def),
            DefKind::AssocConst {
                is_type_const: false
            }
        )
        && value.args.is_empty()
        && value.promoted.is_none()
        && tcx.item_name(value.def).as_str() == "BITS"
        && implementation.krate == core.krate
        && !tcx.impl_is_of_trait(implementation)
        && tcx.type_of(implementation).instantiate_identity() == spec.element
        && tcx.type_of(value.def).instantiate_identity() == tcx.types.u32
        && constant.try_eval_bits(tcx, TypingEnv::fully_monomorphized()) == Some(spec.bits.into())
}

pub(super) fn required_bits<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    count: usize,
    spec: ShiftSpec<'tcx>,
) -> bool {
    body.required_consts.as_ref().is_some_and(|constants| {
        constants.len() == count
            && constants.iter().all(|constant| {
                constant.user_ty.is_none() && bits_const(tcx, constant.const_, spec)
            })
    })
}

pub(super) fn check<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    spec: ShiftSpec<'tcx>,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    let Some(format) = tcx.lang_items().format_arguments() else {
        return false;
    };
    let format = Ty::new_adt(
        tcx,
        tcx.adt_def(format),
        tcx.mk_args(&[tcx.lifetimes.re_erased.into()]),
    );
    let text = Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, tcx.types.str_);
    let Some(element) = spec.precondition_element(tcx) else {
        return false;
    };
    let guard_spec = ShiftSpec { element, ..spec };
    retained::shape(
        body,
        1,
        &[
            tcx.types.unit,
            tcx.types.u32,
            tcx.types.bool,
            tcx.types.never,
            format,
            text,
        ],
        &[1, 0, 1, 0],
        2,
    ) && required_bits(tcx, body, 1, guard_spec)
        && matches!(assignment(&retained::bb(body, 0).statements[0], 2), Some(Rvalue::BinaryOp(BinOp::Lt, operands))
            if retained::operand(&operands.0, 1, false) && bits(tcx, &operands.1, guard_spec))
        && retained::switch_local(body, 0, 2, 2, 1)
        && retained::returns(body, 1)
        && matches!(assignment(&retained::bb(body, 2).statements[0], 5), Some(Rvalue::Use(operand)) if message(tcx, operand, text, spec))
        && panic_call(tcx, body, format)
        && retained::call(
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

fn message<'tcx>(
    tcx: TyCtxt<'tcx>,
    operand: &Operand<'tcx>,
    text: Ty<'tcx>,
    spec: ShiftSpec<'tcx>,
) -> bool {
    let Operand::Constant(constant) = operand else {
        return false;
    };
    let Const::Val(ConstValue::Slice { alloc_id, meta }, ty) = constant.const_ else {
        return false;
    };
    let expected = spec.message();
    if constant.user_ty.is_some() || ty != text || meta != expected.len() as u64 {
        return false;
    }
    let Some(GlobalAlloc::Memory(allocation)) = tcx.try_get_global_alloc(alloc_id) else {
        return false;
    };
    let allocation = allocation.inner();
    let length = rustc_abi::Size::from_bytes(meta);
    allocation.mutability == rustc_hir::Mutability::Not
        && allocation.size() >= length
        && allocation
            .get_bytes_strip_provenance(&tcx, alloc_range(rustc_abi::Size::ZERO, length))
            .is_ok_and(|bytes| bytes == expected.as_bytes())
}

fn panic_call<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, format: Ty<'tcx>) -> bool {
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &retained::bb(body, 3).terminator().kind
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
        && matches!(&args[..], [message, flag] if retained::operand(&message.node, 4, true)
            && scalar(tcx, &flag.node, tcx.types.bool) == Some(0))
        && local(*destination, 3)
        && target.is_none()
        && matches!(unwind, UnwindAction::Unreachable)
}

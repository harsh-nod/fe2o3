//! The failure block is checked structurally, but never executed or admitted
//! as a device helper: its exact lhs < 32 guard is true for every masked count.

use super::*;
use rustc_middle::mir::{AggregateKind, CastKind, ProjectionElem, RawPtrKind, UnOp};
use rustc_span::Symbol;

pub(super) fn check<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> bool {
    if body.arg_count != 1
        || body.local_decls.len() != 14
        || body.basic_blocks.len() != 3
        || body.source_scopes.len() != 6
    {
        return false;
    }
    let types = body
        .local_decls
        .iter()
        .map(|decl| decl.ty)
        .collect::<Vec<_>>();
    let TyKind::Adt(format, _) = types[5].kind() else {
        return false;
    };
    let TyKind::Adt(nonnull, args) = types[8].kind() else {
        return false;
    };
    let Some(argument) = args.as_slice().first().and_then(|arg| arg.as_type()) else {
        return false;
    };
    let TyKind::Adt(argument_adt, argument_args) = argument.kind() else {
        return false;
    };
    if Some(format.did()) != tcx.lang_items().format_arguments()
        || Some(nonnull.did()) != tcx.get_diagnostic_item(Symbol::intern("NonNull"))
        || tcx.def_path_str(argument_adt.did()) != "core::fmt::rt::Argument"
        || argument_adt.did().krate != format.did().krate
        || argument_args.as_slice() != [tcx.lifetimes.re_erased.into()]
    {
        return false;
    }
    let expected = [
        tcx.types.unit,
        tcx.types.u32,
        tcx.types.bool,
        Ty::new_imm_ref(tcx, tcx.lifetimes.re_erased, tcx.types.str_),
        tcx.types.never,
        Ty::new_adt(tcx, *format, tcx.mk_args(&[tcx.lifetimes.re_erased.into()])),
        Ty::new_adt(tcx, *nonnull, tcx.mk_args(&[tcx.types.u8.into()])),
        Ty::new_imm_ptr(tcx, tcx.types.u8),
        Ty::new_adt(tcx, *nonnull, tcx.mk_args(&[argument.into()])),
        tcx.types.usize,
        tcx.types.usize,
        tcx.types.usize,
        Ty::new_imm_ptr(tcx, tcx.types.str_),
        Ty::new_imm_ref(
            tcx,
            tcx.lifetimes.re_erased,
            Ty::new_slice(tcx, tcx.types.u8),
        ),
    ];
    if types != expected || !scopes(tcx, body, types[5]) {
        return false;
    }
    let entry = &body.basic_blocks[BasicBlock::from_usize(0)];
    let success = &body.basic_blocks[BasicBlock::from_usize(1)];
    let failure = &body.basic_blocks[BasicBlock::from_usize(2)];
    let [live, comparison] = &entry.statements[..] else {
        return false;
    };
    let Some(Rvalue::BinaryOp(BinOp::Lt, operands)) = assignment(comparison, 2) else {
        return false;
    };
    let TerminatorKind::SwitchInt { discr, targets } = &entry.terminator().kind else {
        return false;
    };
    let [dead] = &success.statements[..] else {
        return false;
    };
    if !storage(live, 2, true)
        || operand_local_v1(&operands.0) != Some(1)
        || scalar(tcx, &operands.1, tcx.types.u32) != Some(32)
        || operand_local_v1(discr) != Some(2)
        || !switch(targets, 2, 1)
        || !storage(dead, 2, false)
        || !matches!(success.terminator().kind, TerminatorKind::Return)
    {
        return false;
    }
    failure_block(tcx, body, failure)
}

fn scopes<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, fmt: Ty<'tcx>) -> bool {
    for (index, scope) in body.source_scopes.iter().enumerate() {
        if index < 2 {
            if scope.inlined.is_some() {
                return false;
            }
            continue;
        }
        let Some((instance, _)) = scope.inlined else {
            return false;
        };
        let def = instance.def_id();
        let Some(implementation) = tcx.impl_of_assoc(def) else {
            return false;
        };
        let expected = ["from_str", "as_ptr", "len", "as_bytes"][index - 2];
        if !matches!(instance.def, InstanceKind::Item(_))
            || tcx.def_kind(def) != DefKind::AssocFn
            || if index == 2 {
                !matches!(instance.args.as_slice(), [arg] if arg.as_region() == Some(tcx.lifetimes.re_erased))
            } else {
                !instance.args.is_empty()
            }
            || tcx.item_name(def).as_str() != expected
            || def.krate != tcx.lang_items().sized_trait().unwrap().krate
            || implementation.krate != def.krate
            || tcx.impl_is_of_trait(implementation)
            || !match (
                index,
                tcx.type_of(implementation).instantiate_identity().kind(),
                fmt.kind(),
            ) {
                (2, TyKind::Adt(implementation, _), TyKind::Adt(expected, _)) => {
                    implementation == expected
                }
                (_, TyKind::Str, _) if index != 2 => true,
                _ => false,
            }
        {
            return false;
        }
        let sig = signature(tcx, instance);
        let text = Ty::new_imm_ref(
            tcx,
            if index == 2 {
                tcx.lifetimes.re_static
            } else {
                tcx.lifetimes.re_erased
            },
            tcx.types.str_,
        );
        let output = match index {
            2 => fmt,
            3 => Ty::new_imm_ptr(tcx, tcx.types.u8),
            4 => tcx.types.usize,
            5 => Ty::new_imm_ref(
                tcx,
                tcx.lifetimes.re_erased,
                Ty::new_slice(tcx, tcx.types.u8),
            ),
            _ => unreachable!(),
        };
        if sig.safety != Safety::Safe
            || sig.abi != ExternAbi::Rust
            || sig.c_variadic
            || sig.inputs() != [text]
            || sig.output() != output
        {
            return false;
        }
    }
    true
}

fn failure_block<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, block: &BasicBlockData<'tcx>) -> bool {
    use Step::*;
    let expected = [
        Assign(3),
        Live(5),
        Live(6),
        Live(7),
        Live(12),
        Assign(12),
        Assign(7),
        Dead(12),
        Assign(6),
        Dead(7),
        Live(8),
        Live(9),
        Live(10),
        Live(11),
        Live(13),
        Assign(13),
        Assign(11),
        Dead(13),
        Assign(10),
        Dead(11),
        Assign(9),
        Dead(10),
        Assign(8),
        Dead(9),
        Assign(5),
        Dead(8),
        Dead(6),
    ];
    if block.statements.len() != expected.len() {
        return false;
    }
    let Some(Rvalue::Use(message)) = assignment(&block.statements[0], 3) else {
        return false;
    };
    for (statement, step) in block.statements.iter().zip(expected) {
        match step {
            Live(index) if storage(statement, index, true) => {}
            Dead(index) if storage(statement, index, false) => {}
            Assign(index) => {
                let Some(value) = assignment(statement, index) else {
                    return false;
                };
                if !failure_value(tcx, body, index, value, message)
                    || value.ty(&body.local_decls, tcx)
                        != body.local_decls[Local::from_usize(index)].ty
                {
                    return false;
                }
            }
            _ => return false,
        }
    }
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &block.terminator().kind
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
        || tcx.def_kind(panic.def_id()) != DefKind::Fn
        || !panic.args.is_empty()
        || tcx.def_path_str(panic.def_id()) != "core::panicking::panic_nounwind_fmt"
    {
        return false;
    }
    let sig = signature(tcx, panic);
    sig.safety == Safety::Safe
        && sig.abi == ExternAbi::Rust
        && !sig.c_variadic
        && sig.inputs() == [body.local_decls[Local::from_usize(5)].ty, tcx.types.bool]
        && sig.output() == tcx.types.never
        && matches!(&args[..], [fmt, flag] if operand_local_v1(&fmt.node) == Some(5) && scalar(tcx, &flag.node, tcx.types.bool) == Some(0))
        && local(*destination, 4)
        && target.is_none()
        && matches!(unwind, UnwindAction::Unreachable)
}

enum Step {
    Live(usize),
    Dead(usize),
    Assign(usize),
}

fn failure_value<'tcx>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    index: usize,
    value: &Rvalue<'tcx>,
    message: &Operand<'tcx>,
) -> bool {
    match (index, value) {
        (3, Rvalue::Use(Operand::Constant(constant))) => {
            matches!(constant.const_, Const::Val(ConstValue::Slice { .. }, ty) if ty == body.local_decls[Local::from_usize(3)].ty)
        }
        (12, Rvalue::RawPtr(RawPtrKind::Const, place)) => {
            place.local.as_usize() == 3
                && matches!(place.projection.as_ref(), [ProjectionElem::Deref])
        }
        (7, Rvalue::Cast(CastKind::PtrToPtr, operand, _)) => operand_local_v1(operand) == Some(12),
        (6, Rvalue::Cast(CastKind::Transmute, operand, _)) => operand_local_v1(operand) == Some(7),
        (13, Rvalue::Cast(CastKind::Transmute, Operand::Constant(operand), _)) => {
            matches!(message, Operand::Constant(message) if operand.const_ == message.const_)
        }
        (11, Rvalue::UnaryOp(UnOp::PtrMetadata, operand)) => operand_local_v1(operand) == Some(13),
        (10, Rvalue::BinaryOp(BinOp::Shl, operands)) => {
            operand_local_v1(&operands.0) == Some(11)
                && scalar(tcx, &operands.1, tcx.types.i32) == Some(1)
        }
        (9, Rvalue::BinaryOp(BinOp::BitOr, operands)) => {
            operand_local_v1(&operands.0) == Some(10)
                && scalar(tcx, &operands.1, tcx.types.usize) == Some(1)
        }
        (8, Rvalue::Cast(CastKind::Transmute, operand, _)) => operand_local_v1(operand) == Some(9),
        (5, Rvalue::Aggregate(kind, operands)) => {
            matches!(&**kind, AggregateKind::Adt(adt, variant, _, user, field)
            if Some(*adt) == tcx.lang_items().format_arguments() && variant.as_usize() == 0 && user.is_none() && field.is_none())
                && matches!(operands.raw.as_slice(), [template, args] if operand_local_v1(template) == Some(6) && operand_local_v1(args) == Some(8))
        }
        _ => false,
    }
}

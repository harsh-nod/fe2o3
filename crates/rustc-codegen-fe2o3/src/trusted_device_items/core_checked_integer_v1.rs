//! Closed source-origin checks for five observed core integer wrappers.

use rustc_abi::{ExternAbi, Primitive, Size, TagEncoding, Variants};
use rustc_hir::{Safety, def::DefKind};
use rustc_middle::mir::interpret::GlobalAlloc;
use rustc_middle::mir::{
    AggregateKind, AssertKind, BinOp, Body, ClearCrossCrate, ConstValue, Operand, Place,
    ProjectionElem, Rvalue, SourceScope, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv};
use rustc_span::sym;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Add,
    Div,
    Mul,
    OverflowingMul,
    Multiple,
}

fn inherent_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    name: &str,
    input: Ty<'tcx>,
    output: Ty<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_kind(instance.def_id()) != DefKind::AssocFn
        || !tcx.is_mir_available(instance.def_id())
        || !instance.args.is_empty()
        || tcx.item_name(instance.def_id()).as_str() != name
    {
        return false;
    }
    let Some(owner) = tcx.impl_of_assoc(instance.def_id()) else {
        return false;
    };
    if tcx.impl_is_of_trait(owner)
        || tcx.try_normalize_erasing_regions(
            TypingEnv::fully_monomorphized(),
            tcx.type_of(owner).instantiate(tcx, instance.args),
        ).ok() != Some(input)
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args),
    );
    let Ok(signature) =
        tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else {
        return false;
    };
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs() == [input, input]
        && signature.output() == output
}

fn option_ty<'tcx>(tcx: TyCtxt<'tcx>, input: Ty<'tcx>) -> Option<Ty<'tcx>> {
    let definition = tcx.get_diagnostic_item(sym::Option)?;
    let core = tcx.lang_items().sized_trait()?.krate;
    if definition.krate != core || tcx.crate_name(core).as_str() != "core" {
        return None;
    }
    let none = tcx.lang_items().option_none_variant()?;
    let some = tcx.lang_items().option_some_variant()?;
    let adt = tcx.adt_def(definition);
    if !adt.is_enum() || adt.variants().len() != 2 {
        return None;
    }
    let none_index = adt.variant_index_with_id(none);
    let some_index = adt.variant_index_with_id(some);
    if !adt.variant(none_index).fields.is_empty()
        || adt.variant(some_index).fields.len() != 1
        || adt.discriminant_for_variant(tcx, none_index).val != 0
        || adt.discriminant_for_variant(tcx, some_index).val != 1
    {
        return None;
    }
    Some(Ty::new_adt(tcx, adt, tcx.mk_args(&[input.into()])))
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<Family> {
    let usize_option = option_ty(tcx, tcx.types.usize)?;
    let u32_option = option_ty(tcx, tcx.types.u32)?;
    for (family, name, input, output) in [
        (Family::Add, "checked_add", tcx.types.usize, usize_option),
        (Family::Div, "checked_div", tcx.types.usize, usize_option),
        (Family::Mul, "checked_mul", tcx.types.u32, u32_option),
        (Family::OverflowingMul, "overflowing_mul", tcx.types.u32,
            Ty::new_tup(tcx, &[tcx.types.u32, tcx.types.bool])),
        (Family::Multiple, "is_multiple_of", tcx.types.u32, tcx.types.bool),
    ] {
        if inherent_signature(tcx, instance, name, input, output) {
            return Some(family);
        }
    }
    None
}

fn plain(place: &Place<'_>, local: usize) -> bool {
    place.local.as_usize() == local && place.projection.is_empty()
}

#[derive(Clone, Copy)]
enum Input {
    Copy(usize),
    Move(usize),
    Field(usize, usize),
    Zero,
}

fn operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    actual: &Operand<'tcx>,
    expected: Input,
    integer: Ty<'tcx>,
) -> bool {
    match (actual, expected) {
        (Operand::Copy(place), Input::Copy(local))
        | (Operand::Move(place), Input::Move(local)) => plain(place, local),
        (Operand::Copy(place), Input::Field(local, field)) => {
            let [ProjectionElem::Field(index, ty)] = place.projection.as_slice() else {
                return false;
            };
            place.local.as_usize() == local
                && index.as_usize() == field
                && *ty == if field == 0 { integer } else { tcx.types.bool }
        }
        (Operand::Constant(value), Input::Zero) => {
            value.const_.ty() == integer
                && value.const_.try_eval_bits(tcx, TypingEnv::fully_monomorphized()) == Some(0)
        }
        _ => false,
    }
}

// None has no live payload. Read only its initialized direct tag, using rustc's
// actual target layout; do not require or inspect uninitialized payload bytes.
fn none_constant<'tcx>(tcx: TyCtxt<'tcx>, value: &Operand<'tcx>, ty: Ty<'tcx>) -> bool {
    let Operand::Constant(value) = value else {
        return false;
    };
    if value.const_.ty() != ty {
        return false;
    }
    let Ok(layout) = LayoutCx::new(tcx, TypingEnv::fully_monomorphized()).layout_of(ty) else {
        return false;
    };
    let Variants::Multiple { tag, tag_encoding: TagEncoding::Direct, tag_field, .. } = &layout.variants else {
        return false;
    };
    let Primitive::Int(width, false) = tag.primitive() else {
        return false;
    };
    if tag_field.as_usize() >= layout.fields.count() {
        return false;
    }
    let tag_start = layout.fields.offset(tag_field.as_usize()).bytes_usize();
    let tag_size = width.size().bytes_usize();
    let Ok(ConstValue::Indirect { alloc_id, offset }) =
        value.const_.eval(tcx, TypingEnv::fully_monomorphized(), value.span)
    else {
        return false;
    };
    let GlobalAlloc::Memory(allocation) = tcx.global_alloc(alloc_id) else {
        return false;
    };
    let allocation = allocation.inner();
    let Some(end) = offset.bytes_usize().checked_add(layout.size.bytes_usize()) else {
        return false;
    };
    let Some(start) = offset.bytes_usize().checked_add(tag_start) else {
        return false;
    };
    let Some(tag_end) = start.checked_add(tag_size) else {
        return false;
    };
    if end > allocation.len() || tag_end > end || tag_size == 0 {
        return false;
    }
    let pointer_size = tcx.data_layout.pointer_size().bytes_usize();
    if allocation.provenance().ptrs().iter().any(|(at, _)| {
        at.bytes_usize() < end
            && at.bytes_usize().saturating_add(pointer_size) > offset.bytes_usize()
    }) || !(start..tag_end).all(|index| allocation.init_mask().get(Size::from_bytes(index))) {
        return false;
    }
    allocation.inspect_with_uninit_and_ptr_outside_interpreter(start..tag_end)
        .iter().all(|byte| *byte == 0)
}

#[derive(Clone, Copy)]
enum Statement {
    Live(usize),
    Dead(usize),
    Binary(usize, BinOp, Input, Input),
    Use(usize, Input),
    Some(Input),
    None,
}

fn statement<'tcx>(
    tcx: TyCtxt<'tcx>,
    actual: &StatementKind<'tcx>,
    expected: Statement,
    integer: Ty<'tcx>,
    option: Ty<'tcx>,
) -> bool {
    match (actual, expected) {
        (StatementKind::StorageLive(local), Statement::Live(index))
        | (StatementKind::StorageDead(local), Statement::Dead(index)) => local.as_usize() == index,
        (StatementKind::Assign(assignment), expected) => {
            let (destination, value) = &**assignment;
            match (value, expected) {
                (Rvalue::BinaryOp(op, args), Statement::Binary(local, expected_op, left, right)) => {
                    plain(destination, local) && *op == expected_op
                        && operand(tcx, &args.0, left, integer)
                        && operand(tcx, &args.1, right, integer)
                }
                (Rvalue::Use(value), Statement::Use(local, input)) => {
                    plain(destination, local) && operand(tcx, value, input, integer)
                }
                (Rvalue::Use(value), Statement::None) => {
                    plain(destination, 0) && none_constant(tcx, value, option)
                }
                (Rvalue::Aggregate(kind, values), Statement::Some(input)) => {
                    let AggregateKind::Adt(definition, variant, args, None, None) = &**kind else {
                        return false;
                    };
                    let TyKind::Adt(adt, expected_args) = option.kind() else {
                        return false;
                    };
                    let mut values = values.iter();
                    let (Some(value), None) = (values.next(), values.next()) else {
                        return false;
                    };
                    plain(destination, 0) && *definition == adt.did()
                        && *args == *expected_args
                        && Some(*variant) == tcx.lang_items().option_some_variant()
                            .map(|definition| adt.variant_index_with_id(definition))
                        && operand(tcx, value, input, integer)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

#[derive(Clone, Copy)]
enum Term {
    Return,
    Goto(usize),
    Switch(usize, usize, usize),
    Cold(usize),
    RemainderAssert,
}

fn terminator<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    actual: &TerminatorKind<'tcx>,
    expected: Term,
) -> bool {
    match (actual, expected) {
        (TerminatorKind::Return, Term::Return) => true,
        (TerminatorKind::Goto { target }, Term::Goto(index)) => target.as_usize() == index,
        (TerminatorKind::SwitchInt { discr, targets }, Term::Switch(local, zero, other)) => {
            let mut targets_iter = targets.iter();
            matches!(discr, Operand::Copy(place) if plain(place, local))
                && targets_iter.next().map(|(value, target)| (value, target.as_usize())) == Some((0, zero))
                && targets_iter.next().is_none() && targets.otherwise().as_usize() == other
        }
        (TerminatorKind::Call { func: Operand::Constant(function), args, destination,
            target: Some(target), unwind: UnwindAction::Unreachable, .. }, Term::Cold(local)) => {
            let TyKind::FnDef(definition, generics) = function.const_.ty().kind() else {
                return false;
            };
            args.is_empty() && plain(destination, local) && target.as_usize() == 3
                && definition.krate == instance.def_id().krate && generics.is_empty()
                && tcx.intrinsic(*definition).is_some_and(|value| value.name.as_str() == "cold_path")
        }
        (TerminatorKind::Assert { cond, expected: false, msg, target,
            unwind: UnwindAction::Unreachable }, Term::RemainderAssert) => {
            matches!(cond, Operand::Move(place) if plain(place, 4))
                && matches!(&**msg, AssertKind::RemainderByZero(Operand::Copy(place)) if plain(place, 1))
                && target.as_usize() == 3
        }
        _ => false,
    }
}

fn scopes<'tcx>(tcx: TyCtxt<'tcx>, body: &Body<'tcx>, family: Family) -> bool {
    let expected: &[(Option<usize>, Option<usize>, u8)] = match family {
        Family::Add | Family::Div => &[(None, None, 0), (Some(0), None, 1)],
        Family::Mul => &[(None, None, 0), (Some(0), None, 0), (Some(0), None, 2),
            (Some(2), Some(2), 0), (Some(1), None, 1)],
        Family::OverflowingMul => &[(None, None, 0), (Some(0), None, 0)],
        Family::Multiple => &[(None, None, 0)],
    };
    if body.source_scopes.len() != expected.len() {
        return false;
    }
    for (scope, (parent, inline_parent, kind)) in body.source_scopes.iter().zip(expected) {
        if scope.parent_scope.map(|value| value.as_usize()) != *parent
            || scope.inlined_parent_scope.map(|value| value.as_usize()) != *inline_parent
            || !matches!(&scope.local_data, ClearCrossCrate::Clear)
        {
            return false;
        }
        match (scope.inlined, kind) {
            (None, 0) => {}
            (Some((origin, _)), 1) if tcx.item_name(origin.def_id()).as_str() == "unlikely"
                && crate::production_rustc_intrinsic_v1::authenticate_reviewed_branch_hint_origin_v1(tcx, origin) => {}
            // This identifies the captured inline origin, but grants no exemption
            // to its separate source-origin/body audit in the collector.
            (Some((origin, _)), 2) if inherent_signature(tcx, origin, "overflowing_mul",
                tcx.types.u32, Ty::new_tup(tcx, &[tcx.types.u32, tcx.types.bool])) => {}
            _ => return false,
        }
    }
    true
}

fn checked_body<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let Some(family) = signature(tcx, instance) else {
        return false;
    };
    let integer = match family {
        Family::Add | Family::Div => tcx.types.usize,
        Family::Mul | Family::OverflowingMul | Family::Multiple => tcx.types.u32,
    };
    let Some(option) = option_ty(tcx, integer) else {
        return false;
    };
    let pair = Ty::new_tup(tcx, &[integer, tcx.types.bool]);
    let (locals, last_scope): (Vec<Ty<'tcx>>, usize) = match family {
        Family::Add => (vec![option, integer, integer, tcx.types.bool, pair, integer, tcx.types.unit], 1),
        Family::Div => (vec![option, integer, integer, integer, tcx.types.unit], 1),
        Family::Mul => (vec![option, integer, integer, integer, tcx.types.bool, pair, tcx.types.unit], 4),
        Family::OverflowingMul => (vec![pair, integer, integer, integer, tcx.types.bool], 0),
        Family::Multiple => (vec![tcx.types.bool, integer, integer, integer, tcx.types.bool], 0),
    };
    if body.arg_count != 2 || body.local_decls.len() != locals.len()
        || body.basic_blocks.len() != if family == Family::OverflowingMul { 1 } else { 5 }
        || !scopes(tcx, body, family)
    {
        return false;
    }
    for (index, (actual, expected)) in body.local_decls.iter().zip(&locals).enumerate() {
        let expected_scope = if index + 1 == locals.len() { last_scope } else { 0 };
        if actual.source_info.scope.as_usize() != expected_scope
            || instance.try_instantiate_mir_and_normalize_erasing_regions(tcx,
                TypingEnv::fully_monomorphized(), EarlyBinder::bind(actual.ty)).ok() != Some(*expected)
        {
            return false;
        }
    }
    use Input::{Copy as C, Field as F, Move as M, Zero as Z};
    use Statement::{Binary as B, Dead as D, Live as L, None as N, Some as S, Use as U};
    // Exact ordered operations and edges from the retained AMD-core bodies.
    let blocks: &[(&[(usize, Statement)], usize, Term)] = match family {
        Family::Add => &[
            (&[(0,L(3)),(0,L(4)),(0,B(4,BinOp::AddWithOverflow,C(1),C(2))),(0,U(3,F(4,1)))],1,Term::Switch(3,4,2)),
            (&[],0,Term::Return), (&[],1,Term::Cold(6)),
            (&[(0,D(4)),(0,D(3)),(0,N)],0,Term::Goto(1)),
            (&[(0,D(4)),(0,D(3)),(0,L(5)),(0,B(5,BinOp::AddUnchecked,C(1),C(2))),(0,S(M(5))),(0,D(5))],0,Term::Goto(1)),
        ],
        Family::Div => &[
            (&[],1,Term::Switch(2,2,4)), (&[],0,Term::Return), (&[],1,Term::Cold(4)),
            (&[(0,N)],0,Term::Goto(1)),
            (&[(0,L(3)),(0,B(3,BinOp::Div,C(1),C(2))),(0,S(M(3))),(0,D(3))],0,Term::Goto(1)),
        ],
        Family::Mul => &[
            (&[(0,L(5)),(2,B(5,BinOp::MulWithOverflow,C(1),C(2))),(0,U(3,F(5,0))),(0,U(4,F(5,1))),(0,D(5))],4,Term::Switch(4,4,2)),
            (&[],0,Term::Return), (&[],4,Term::Cold(6)),
            (&[(1,N)],1,Term::Goto(1)), (&[(1,S(C(3)))],1,Term::Goto(1)),
        ],
        Family::OverflowingMul => &[
            (&[(0,B(0,BinOp::MulWithOverflow,C(1),C(2))),(0,U(3,F(0,0))),(0,U(4,F(0,1)))],0,Term::Return),
        ],
        Family::Multiple => &[
            (&[],0,Term::Switch(2,2,1)),
            (&[(0,L(3)),(0,B(4,BinOp::Eq,C(2),Z))],0,Term::RemainderAssert),
            (&[(0,B(0,BinOp::Eq,C(1),Z))],0,Term::Goto(4)),
            (&[(0,B(3,BinOp::Rem,C(1),C(2))),(0,B(0,BinOp::Eq,M(3),Z)),(0,D(3))],0,Term::Goto(4)),
            (&[],0,Term::Return),
        ],
    };
    for (actual, (statements, scope, term)) in body.basic_blocks.iter().zip(blocks) {
        if actual.is_cleanup || actual.statements.len() != statements.len() {
            return false;
        }
        for (actual, (scope, expected)) in actual.statements.iter().zip(*statements) {
            if actual.source_info.scope.as_usize() != *scope
                || !statement(tcx, &actual.kind, *expected, integer, option)
            {
                return false;
            }
        }
        let Some(actual) = &actual.terminator else {
            return false;
        };
        if actual.source_info.scope.as_usize() != *scope
            || !terminator(tcx, instance, &actual.kind, *term)
        {
            return false;
        }
    }
    true
}

/// Source-origin only: ordinary operation import, callees and every inline
/// origin still undergo their independent existing checks.
pub(crate) fn authenticate_reviewed_safe_core_checked_integer_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>,
) -> bool {
    signature(tcx, instance).is_some() && checked_body(tcx, instance, tcx.instance_mir(instance.def))
}

#[cfg(test)]
#[path = "core_checked_integer_v1_tests.rs"]
mod tests;

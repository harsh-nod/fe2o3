//! Exact pinned-core shift proof, usable only at the safe wrapping entry point.
//! The checked count is symbolic: for every u32 r, r & 31 is in 0..32.

use super::*;
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher};
use rustc_hir::def::DefKind;
use rustc_middle::mir::{
    BasicBlock, Const, ConstOperand, ConstValue, Local, LocalDecl, Place, RuntimeChecks,
    SourceInfo, SourceScope, Statement, Terminator,
};
use rustc_middle::ty::FnSig;

#[path = "wrapping_shr/formatting.rs"]
mod formatting;
#[path = "wrapping_shr/precondition.rs"]
mod precondition;
#[path = "wrapping_shr/retained.rs"]
mod retained;

#[path = "wrapping_shr/general_shift.rs"]
mod general_shift;
pub(crate) use general_shift::{ReviewedCoreWrappingShiftV1, prove_core_wrapping_shift_v1};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ReviewedU32WrappingShrV1<'tcx> {
    instance: Instance<'tcx>,
    closure_fingerprint: [u8; 16],
}

impl<'tcx> ReviewedU32WrappingShrV1<'tcx> {
    pub(crate) fn instance(self) -> Instance<'tcx> {
        self.instance
    }

    pub(crate) fn expansion_fingerprint(self, tcx: TyCtxt<'tcx>, body: &Body<'tcx>) -> [u8; 16] {
        let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
            let mut hasher = StableHasher::new();
            "fe2o3/core-u32-wrapping-shr/masked-mir/v1".hash_stable(&mut context, &mut hasher);
            self.instance.hash_stable(&mut context, &mut hasher);
            self.closure_fingerprint
                .hash_stable(&mut context, &mut hasher);
            body.hash_stable(&mut context, &mut hasher);
            hasher.finish()
        });
        fingerprint.to_le_bytes()
    }

    /// The producer must bind this expansion to the original instance/MIR/ABI
    /// transcript. Both operations still undergo normal semantic admission.
    pub(crate) fn expand_mir(self, tcx: TyCtxt<'tcx>) -> Body<'tcx> {
        self.expand_source_mir(tcx, tcx.instance_mir(self.instance.def))
    }

    fn expand_source_mir(self, tcx: TyCtxt<'tcx>, source: &Body<'tcx>) -> Body<'tcx> {
        let mut body = source.clone();
        let source_info = SourceInfo {
            span: body.span,
            scope: SourceScope::from_usize(0),
        };
        // Materialize the proved relation, independent of source block/local
        // positions. The original body and complete proof remain fingerprinted.
        let binary = |destination, operation, left, right| {
            Statement::new(
                source_info,
                StatementKind::Assign(Box::new((
                    Local::from_usize(destination).into(),
                    Rvalue::BinaryOp(operation, Box::new((left, right))),
                ))),
            )
        };
        let mask = binary(
            3,
            BinOp::BitAnd,
            Operand::Copy(Local::from_usize(2).into()),
            Operand::Constant(Box::new(ConstOperand {
                span: body.span,
                user_ty: None,
                const_: Const::Val(
                    ConstValue::Scalar(rustc_middle::mir::interpret::Scalar::from_uint(
                        31_u32,
                        rustc_abi::Size::from_bytes(4),
                    )),
                    tcx.types.u32,
                ),
            })),
        );
        let shift = binary(
            0,
            BinOp::Shr,
            Operand::Copy(Local::from_usize(1).into()),
            Operand::Copy(Local::from_usize(3).into()),
        );
        body.basic_blocks_mut().raw.clear();
        body.basic_blocks_mut().push(BasicBlockData::new_stmts(
            vec![mask, shift],
            Some(Terminator {
                source_info,
                kind: TerminatorKind::Return,
            }),
            false,
        ));
        body.local_decls.truncate(3);
        body.local_decls
            .push(LocalDecl::new(tcx.types.u32, body.span));
        for declaration in &mut body.local_decls {
            declaration.source_info = source_info;
        }
        body.source_scopes.truncate(1);
        body.var_debug_info.clear();
        body
    }
}

/// This grants no standalone admission to unchecked_shr, its UB checker, or a
/// panic helper. Recursive discovery uses only the fully proved masked-shift
/// expansion; the original callable still undergoes normal semantic admission.
pub(crate) fn prove_core_u32_wrapping_shr_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Option<ReviewedU32WrappingShrV1<'tcx>> {
    prove_with(tcx, instance, &|instance| tcx.instance_mir(instance.def))
}

fn prove_with<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
) -> Option<ReviewedU32WrappingShrV1<'tcx>>
where
    'tcx: 'a,
{
    let mut budget = 16;
    // Bind precisely the bodies visited by the closed proof, in proof order.
    let visited = std::cell::RefCell::new(Vec::new());
    let fetch = |instance: Instance<'tcx>| {
        let source = body(instance);
        visited.borrow_mut().push((instance, source));
        source
    };
    if !check_instance(tcx, instance, Helper::Wrapping, &fetch, &mut budget) {
        return None;
    }
    let fingerprint: Fingerprint = tcx.with_stable_hashing_context(|mut context| {
        let mut hasher = StableHasher::new();
        for (instance, source) in visited.into_inner() {
            instance.hash_stable(&mut context, &mut hasher);
            source.hash_stable(&mut context, &mut hasher);
        }
        hasher.finish()
    });
    Some(ReviewedU32WrappingShrV1 {
        instance,
        closure_fingerprint: fingerprint.to_le_bytes(),
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Helper {
    Wrapping,
    Unchecked,
    LanguageUb,
    Runtime,
    Precondition,
    FormatFromStr,
    StrAsPtr,
    StrLen,
    StrAsBytes,
}

fn signature<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> FnSig<'tcx> {
    tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    )
}

fn core_item(tcx: TyCtxt<'_>, instance: Instance<'_>) -> bool {
    let Some(core) = tcx.lang_items().sized_trait() else {
        return false;
    };
    matches!(instance.def, InstanceKind::Item(_))
        && instance.def_id().krate == core.krate
        && tcx.crate_name(core.krate).as_str() == "core"
        && matches!(
            tcx.def_kind(instance.def_id()),
            DefKind::Fn | DefKind::AssocFn
        )
        && instance.args.is_empty()
        && tcx.is_mir_available(instance.def_id())
}

fn identity<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, helper: Helper) -> bool {
    if matches!(
        helper,
        Helper::FormatFromStr | Helper::StrAsPtr | Helper::StrLen | Helper::StrAsBytes
    ) {
        return formatting::identity(tcx, instance, helper);
    }
    if !core_item(tcx, instance) {
        return false;
    }
    let definition = instance.def_id();
    let sig = signature(tcx, instance);
    if sig.abi != ExternAbi::Rust
        || sig.c_variadic
        || sig.safety
            != if helper == Helper::Unchecked {
                Safety::Unsafe
            } else {
                Safety::Safe
            }
    {
        return false;
    }
    let path = tcx.def_path_str(definition);
    match helper {
        Helper::Wrapping | Helper::Unchecked => {
            let name = if helper == Helper::Wrapping {
                "wrapping_shr"
            } else {
                "unchecked_shr"
            };
            let Some(implementation) = tcx.impl_of_assoc(definition) else {
                return false;
            };
            tcx.item_name(definition).as_str() == name
                && tcx
                    .opt_associated_item(definition)
                    .is_some_and(|item| item.is_fn())
                && implementation.krate == definition.krate
                && !tcx.impl_is_of_trait(implementation)
                && tcx.type_of(implementation).instantiate_identity() == tcx.types.u32
                && sig.inputs() == [tcx.types.u32, tcx.types.u32]
                && sig.output() == tcx.types.u32
        }
        Helper::LanguageUb | Helper::Runtime => {
            sig.inputs().is_empty()
                && sig.output() == tcx.types.bool
                && path
                    == if helper == Helper::LanguageUb {
                        "core::ub_checks::check_language_ub"
                    } else {
                        "core::ub_checks::check_language_ub::runtime"
                    }
        }
        Helper::Precondition => {
            tcx.def_kind(definition) == DefKind::Fn
                && tcx.item_name(definition).as_str() == "precondition_check"
                && identity(
                    tcx,
                    Instance::mono(tcx, tcx.parent(definition)),
                    Helper::Unchecked,
                )
                && sig.inputs() == [tcx.types.u32]
                && sig.output() == tcx.types.unit
        }
        Helper::FormatFromStr | Helper::StrAsPtr | Helper::StrLen | Helper::StrAsBytes => {
            unreachable!()
        }
    }
}

fn check_instance<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    helper: Helper,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    if *budget == 0 || !identity(tcx, instance, helper) {
        return false;
    }
    *budget -= 1;
    let body = fetch(instance);
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.tainted_by_errors.is_some()
        || body.var_debug_info.len() > 32
        || body.user_type_annotations.len() > 8
        || body.basic_blocks.len() > 5
        || body.local_decls.len() > 16
        || body.source_scopes.len() > 8
        || body
            .basic_blocks
            .iter()
            .any(|block| block.is_cleanup || block.terminator.is_none())
        || body
            .basic_blocks
            .iter()
            .map(|block| block.statements.len())
            .sum::<usize>()
            > 64
    {
        return false;
    }
    if let Some(accepted) = retained::check(tcx, body, helper, fetch, budget) {
        return accepted;
    }
    if matches!(
        helper,
        Helper::FormatFromStr | Helper::StrAsPtr | Helper::StrLen | Helper::StrAsBytes
    ) {
        return formatting::check(tcx, body, helper, fetch, budget);
    }
    if helper == Helper::Precondition {
        return precondition::check(tcx, body);
    }
    let expected_scopes: &[Helper] = match helper {
        Helper::Wrapping => &[Helper::Unchecked, Helper::LanguageUb, Helper::Runtime],
        Helper::Unchecked => &[Helper::LanguageUb, Helper::Runtime],
        Helper::LanguageUb => &[Helper::Runtime],
        Helper::Runtime => &[],
        _ => unreachable!(),
    };
    if body.source_scopes.len() != expected_scopes.len() + 1
        || body.source_scopes[SourceScope::from_usize(0)]
            .inlined
            .is_some()
    {
        return false;
    }
    for (scope, &kind) in body.source_scopes.iter().skip(1).zip(expected_scopes) {
        let Some((callee, _)) = scope.inlined else {
            return false;
        };
        if !check_instance(tcx, callee, kind, fetch, budget) {
            return false;
        }
    }
    match helper {
        Helper::Wrapping | Helper::Unchecked => check_shift(tcx, body, helper, fetch, budget),
        Helper::LanguageUb | Helper::Runtime => {
            if body.arg_count != 0
                || body.local_decls.len() != 1
                || body.local_decls[Local::from_usize(0)].ty != tcx.types.bool
                || body.basic_blocks.len() != 1
            {
                return false;
            }
            let block = &body.basic_blocks[BasicBlock::from_usize(0)];
            let [statement] = &block.statements[..] else {
                return false;
            };
            let Some(Rvalue::Use(operand)) = assignment(statement, 0) else {
                return false;
            };
            matches!(block.terminator().kind, TerminatorKind::Return)
                && if helper == Helper::LanguageUb {
                    ub_checks(operand)
                } else {
                    scalar(tcx, operand, tcx.types.bool) == Some(1)
                }
        }
        _ => unreachable!(),
    }
}

fn check_shift<'tcx, 'a>(
    tcx: TyCtxt<'tcx>,
    body: &Body<'tcx>,
    helper: Helper,
    fetch: &impl Fn(Instance<'tcx>) -> &'a Body<'tcx>,
    budget: &mut usize,
) -> bool
where
    'tcx: 'a,
{
    let wrapping = helper == Helper::Wrapping;
    let count = if wrapping { 3 } else { 2 };
    let unit = count + 1;
    if body.arg_count != 2
        || body.basic_blocks.len() != 3
        || body.local_decls.len() != unit + 1
        || body.local_decls.iter_enumerated().any(|(local, decl)| {
            decl.ty
                != if local.as_usize() == unit {
                    tcx.types.unit
                } else {
                    tcx.types.u32
                }
        })
    {
        return false;
    }
    let entry = &body.basic_blocks[BasicBlock::from_usize(0)];
    let call = &body.basic_blocks[BasicBlock::from_usize(1)];
    let tail = &body.basic_blocks[BasicBlock::from_usize(2)];
    if wrapping {
        let [live, mask] = &entry.statements[..] else {
            return false;
        };
        if !storage(live, 3, true) {
            return false;
        }
        let Some(Rvalue::BinaryOp(BinOp::BitAnd, operands)) = assignment(mask, 3) else {
            return false;
        };
        if !retained::operand(&operands.0, 2, false)
            || scalar(tcx, &operands.1, tcx.types.u32) != Some(31)
        {
            return false;
        }
    } else if !entry.statements.is_empty() {
        return false;
    }
    let TerminatorKind::SwitchInt { discr, targets } = &entry.terminator().kind else {
        return false;
    };
    if !ub_checks(discr) || !switch(targets, 2, 1) || !call.statements.is_empty() {
        return false;
    }
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target,
        unwind,
        ..
    } = &call.terminator().kind
    else {
        return false;
    };
    let Some(callee) = resolve(tcx, func) else {
        return false;
    };
    if !matches!(&args[..], [arg] if retained::operand(&arg.node, count, false))
        || !local(*destination, unit)
        || target.map(|block| block.as_usize()) != Some(2)
        || !matches!(unwind, UnwindAction::Unreachable)
        || !check_instance(tcx, callee, Helper::Precondition, fetch, budget)
    {
        return false;
    }
    if tail.statements.len() != if wrapping { 2 } else { 1 }
        || !matches!(tail.terminator().kind, TerminatorKind::Return)
        || wrapping && !storage(&tail.statements[1], count, false)
    {
        return false;
    }
    matches!(assignment(&tail.statements[0], 0), Some(Rvalue::BinaryOp(BinOp::ShrUnchecked, operands))
        if retained::operand(&operands.0, 1, false) && retained::operand(&operands.1, count, false))
}

fn local(place: Place<'_>, index: usize) -> bool {
    place.projection.is_empty() && place.local.as_usize() == index
}

fn assignment<'a, 'tcx>(statement: &'a Statement<'tcx>, index: usize) -> Option<&'a Rvalue<'tcx>> {
    let StatementKind::Assign(assignment) = &statement.kind else {
        return None;
    };
    local(assignment.0, index).then_some(&assignment.1)
}

fn storage(statement: &Statement<'_>, index: usize, live: bool) -> bool {
    match statement.kind {
        StatementKind::StorageLive(local) if live => local.as_usize() == index,
        StatementKind::StorageDead(local) if !live => local.as_usize() == index,
        _ => false,
    }
}

fn ub_checks(operand: &Operand<'_>) -> bool {
    matches!(operand, Operand::RuntimeChecks(RuntimeChecks::UbChecks))
}

fn switch(targets: &rustc_middle::mir::SwitchTargets, zero: usize, otherwise: usize) -> bool {
    targets.iter().eq([(0, BasicBlock::from_usize(zero))])
        && targets.otherwise().as_usize() == otherwise
}

fn scalar<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>, ty: Ty<'tcx>) -> Option<u128> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    let Const::Val(value, actual) = constant.const_ else {
        return None;
    };
    if actual != ty {
        return None;
    }
    value
        .try_to_scalar_int()?
        .try_to_bits(
            tcx.layout_of(TypingEnv::fully_monomorphized().as_query_input(ty))
                .ok()?
                .size,
        )
        .ok()
}

fn resolve<'tcx>(tcx: TyCtxt<'tcx>, operand: &Operand<'tcx>) -> Option<Instance<'tcx>> {
    let Operand::Constant(constant) = operand else {
        return None;
    };
    let Const::Val(ConstValue::ZeroSized, ty) = constant.const_ else {
        return None;
    };
    let TyKind::FnDef(definition, args) = ty.kind() else {
        return None;
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
        .ok()
        .flatten()
}

#[cfg(test)]
#[path = "shift_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wrapping_shr/retained_tests.rs"]
pub(super) mod retained_tests;

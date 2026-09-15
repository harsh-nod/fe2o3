//! Exact lexical method occurrences. A receipt binds HIR operands to the
//! retained original call; it is not a synthetic SSA loan or an alias waiver.
use super::{
    definitions::{self, BodyRecipe, Definition, Role, Types, normalize},
    source_calls::{Call, spend},
};
use crate::collector::production_importer_v1::ProductionSemanticImportErrorV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use rustc_hir::{
    BorrowKind, Expr, ExprKind, HirId, Mutability,
    def::Res,
    intravisit::{self, Visitor},
};
use rustc_middle::{
    mir::{Operand, TerminatorKind},
    ty::{
        Instance, Ty, TyCtxt, TypeckResults, TypingEnv,
        adjustment::{Adjust, Adjustment, AutoBorrow, AutoBorrowMutability, DerefAdjustKind},
    },
};

type Result<T> = std::result::Result<T, ProductionSemanticImportErrorV1>;
fn rejected(detail: &'static str) -> ProductionSemanticImportErrorV1 {
    ProductionSemanticImportErrorV1::KernelContextBinding(detail)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Syntax {
    Method {
        expression: HirId,
        receiver: HirId,
        argument: Option<HirId>,
    },
    /// External issue_phase has no local HIR. Its only source owner must be
    /// the exact reviewed with_phase body and its exact original Issue edge.
    ReviewedIssue { wrapper: SemanticFunctionIdV1 },
}

pub(super) struct Receipt<'a, 'tcx> {
    pub call: &'a Call<'a, 'tcx>,
    pub syntax: Syntax,
    pub method: Option<Method<'tcx>>,
}

pub(super) struct Method<'tcx> {
    pub expression: &'tcx Expr<'tcx>,
    pub receiver: &'tcx Expr<'tcx>,
    pub receiver_binding: HirId,
    pub argument: Option<&'tcx Expr<'tcx>>,
    pub storage_binding: Option<HirId>,
}

pub(super) fn observe<'a, 'tcx>(
    tcx: TyCtxt<'tcx>,
    call: &'a Call<'a, 'tcx>,
    definition: &Definition<'tcx>,
    definitions: &[(SemanticFunctionIdV1, Definition<'tcx>)],
    work: &mut usize,
) -> Result<Receipt<'a, 'tcx>> {
    if call.callee_instance != definition.instance {
        return Err(rejected("phase HIR definition substitution"));
    }
    if definition.role == Role::Issue {
        let mut wrapper = None;
        for (id, candidate) in definitions {
            spend(work, 1)?;
            if *id != call.caller {
                continue;
            }
            let BodyRecipe::WithPhase {
                issue, issue_block, ..
            } = candidate.recipe
            else {
                return Err(rejected("phase Issue caller is not its exact wrapper"));
            };
            if candidate.instance != call.caller_instance
                || issue != definition.instance
                || issue_block != call.raw_block
                || wrapper.replace(*id).is_some()
            {
                return Err(rejected("phase Issue original wrapper edge changed"));
            }
        }
        return Ok(Receipt {
            call,
            method: None,
            syntax: Syntax::ReviewedIssue {
                wrapper: wrapper
                    .ok_or_else(|| rejected("phase Issue has no reviewed wrapper owner"))?,
            },
        });
    }
    let lexical = call
        .caller_instance
        .def_id()
        .as_local()
        .ok_or_else(|| rejected("phase method requires original local HIR"))?;
    if !matches!(
        tcx.def_kind(lexical),
        rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::Closure
    ) {
        return Err(rejected("phase method has no lexical function body"));
    }
    let typeck = tcx.typeck(lexical);
    let mut scan = Scan {
        tcx,
        caller: call.caller_instance,
        typeck,
        target: definition.instance,
        span: call.original.basic_blocks[call.raw_block]
            .terminator()
            .source_info
            .span,
        work,
        depth: 0,
        found: None,
        error: None,
    };
    scan.visit_expr(tcx.hir_body_owned_by(lexical).value);
    if let Some(error) = scan.error {
        return Err(error);
    }
    let expr = scan
        .found
        .ok_or_else(|| rejected("phase original call has no exact HIR occurrence"))?;
    let ExprKind::MethodCall(_, receiver, arguments, _) = expr.kind else {
        return Err(rejected("phase source method syntax changed"));
    };
    let signature = normalize(
        tcx,
        call.caller_instance,
        tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(definition.instance.def_id())
                .instantiate(tcx, definition.instance.args),
        ),
    )
    .map_err(|_| rejected("phase method source signature normalization"))?;
    if !typeck.expr_adjustments(expr).is_empty()
        || typed(tcx, call.caller_instance, typeck.expr_ty(expr))? != signature.output()
        || arguments.len() + 1 != signature.inputs().len()
    {
        return Err(rejected("phase method result or argument roster changed"));
    }
    let argument = match definition.types {
        Types::OwnerConvert { workgroup, .. } => {
            exact_value(tcx, call.caller_instance, typeck, receiver, workgroup)?;
            local(typeck, receiver)?;
            None
        }
        Types::WithPhase {
            owner_reference,
            owner,
            closure,
            ..
        } => {
            auto_borrow(
                tcx,
                call.caller_instance,
                typeck,
                receiver,
                owner,
                owner_reference,
                Mutability::Mut,
            )?;
            local(typeck, receiver)?;
            let [argument] = arguments else {
                return Err(rejected("phase wrapper closure arity"));
            };
            exact_value(tcx, call.caller_instance, typeck, argument, closure)?;
            let ExprKind::Closure(hir_closure) = argument.kind else {
                return Err(rejected(
                    "phase wrapper requires its actual lexical closure",
                ));
            };
            let rustc_middle::ty::TyKind::Closure(definition, _) = *closure.kind() else {
                return Err(rejected("phase wrapper closure nominal type"));
            };
            if hir_closure.def_id.to_def_id() != definition {
                return Err(rejected("phase wrapper substituted closure definition"));
            }
            Some(argument.hir_id)
        }
        Types::Bind {
            phase_reference,
            storage_reference,
            storage,
            phase,
            ..
        } => {
            auto_borrow(
                tcx,
                call.caller_instance,
                typeck,
                receiver,
                phase.workgroup,
                phase_reference,
                Mutability::Not,
            )?;
            local(typeck, receiver)?;
            let [argument] = arguments else {
                return Err(rejected("phase Bind storage arity"));
            };
            let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, owned) = argument.kind else {
                return Err(rejected(
                    "phase Bind requires the actual unique storage borrow",
                ));
            };
            exact_value(tcx, call.caller_instance, typeck, owned, storage)?;
            local(typeck, owned)?;
            if !identity_storage_reborrow(
                tcx,
                call.caller_instance,
                storage,
                storage_reference,
                typeck.expr_ty(argument),
                typeck.expr_ty_adjusted(argument),
                typeck.expr_adjustments(argument),
            )? {
                return Err(rejected(
                    "phase storage borrow changed its exact mutable reference adjustment",
                ));
            }
            Some(argument.hir_id)
        }
        Types::Finish { phase, .. } => {
            exact_value(tcx, call.caller_instance, typeck, receiver, phase.workgroup)?;
            local(typeck, receiver)?;
            None
        }
        Types::Issue { .. } => return Err(rejected("phase Issue lost its reviewed wrapper")),
    };
    let TerminatorKind::Call { args, .. } =
        &call.original.basic_blocks[call.raw_block].terminator().kind
    else {
        return Err(rejected("phase original HIR call changed"));
    };
    spend(work, args.len())?;
    if !matches!(
        args.first().map(|arg| &arg.node),
        Some(Operand::Copy(_) | Operand::Move(_))
    ) || (definition.role == Role::Bind
        && !matches!(
            args.get(1).map(|arg| &arg.node),
            Some(Operand::Copy(_) | Operand::Move(_))
        ))
    {
        return Err(rejected(
            "phase source authority operand was replaced by a constant",
        ));
    }
    Ok(Receipt {
        call,
        method: Some(Method {
            expression: expr,
            receiver,
            receiver_binding: local(typeck, receiver)?,
            argument: arguments.first(),
            storage_binding: if definition.role == Role::Bind {
                let ExprKind::AddrOf(BorrowKind::Ref, Mutability::Mut, owned) = arguments[0].kind
                else {
                    return Err(rejected("phase exact storage borrow changed"));
                };
                Some(local(typeck, owned)?)
            } else {
                None
            },
        }),
        syntax: Syntax::Method {
            expression: expr.hir_id,
            receiver: receiver.hir_id,
            argument,
        },
    })
}

fn typed<'tcx>(tcx: TyCtxt<'tcx>, caller: Instance<'tcx>, ty: Ty<'tcx>) -> Result<Ty<'tcx>> {
    normalize(tcx, caller, ty).map_err(|_| rejected("phase HIR type normalization"))
}
fn exact_value<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    typeck: &TypeckResults<'tcx>,
    expr: &Expr<'tcx>,
    expected: Ty<'tcx>,
) -> Result<()> {
    if !typeck.expr_adjustments(expr).is_empty()
        || typed(tcx, caller, typeck.expr_ty(expr))? != expected
    {
        #[cfg(test)]
        if std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY").is_some() {
            eprintln!(
                "fe2o3 phase HIR value: caller={caller:?} expression={:?} span={:?} actual={:?} expected={expected:?} adjustments={:?}",
                expr.hir_id,
                expr.span,
                typeck.expr_ty(expr),
                typeck.expr_adjustments(expr),
            );
        }
        return Err(rejected(
            "phase exact value type or source adjustment changed",
        ));
    }
    Ok(())
}
fn identity_storage_reborrow<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    storage: Ty<'tcx>,
    reference: Ty<'tcx>,
    raw: Ty<'tcx>,
    adjusted: Ty<'tcx>,
    adjustments: &[Adjustment<'tcx>],
) -> Result<bool> {
    if !matches!(*reference.kind(), rustc_middle::ty::TyKind::Ref(_, pointee, Mutability::Mut) if pointee == storage)
        || typed(tcx, caller, raw)? != reference
        || typed(tcx, caller, adjusted)? != reference
    {
        return Ok(false);
    }
    // Rust may reborrow an explicit &mut argument. Only its built-in identity
    // adjustment is transparent here; original MIR and loan checks remain.
    match adjustments {
        [] => Ok(true),
        [deref, borrow] => Ok(
            matches!(deref.kind, Adjust::Deref(DerefAdjustKind::Builtin))
                && matches!(
                    borrow.kind,
                    Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut { .. }))
                )
                && typed(tcx, caller, deref.target)? == storage
                && typed(tcx, caller, borrow.target)? == reference,
        ),
        _ => Ok(false),
    }
}

#[cfg(test)]
#[path = "hir_calls/tests.rs"]
mod tests;

pub(super) fn local(typeck: &TypeckResults<'_>, expr: &Expr<'_>) -> Result<HirId> {
    let ExprKind::Path(path) = &expr.kind else {
        return Err(rejected(
            "phase first source slice requires an exact local receiver",
        ));
    };
    match typeck.qpath_res(path, expr.hir_id) {
        Res::Local(id) => Ok(id),
        _ => Err(rejected("phase source receiver is not a local binding")),
    }
}
fn auto_borrow<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    typeck: &TypeckResults<'tcx>,
    expr: &Expr<'tcx>,
    pointee: Ty<'tcx>,
    reference: Ty<'tcx>,
    kind: Mutability,
) -> Result<()> {
    let [adjustment] = typeck.expr_adjustments(expr) else {
        return Err(rejected(
            "phase source receiver lost its exact borrow adjustment",
        ));
    };
    let right_kind = match (&adjustment.kind, kind) {
        (Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not)), Mutability::Not) => true,
        (Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Mut { .. })), Mutability::Mut) => {
            true
        }
        _ => false,
    };
    if !right_kind
        || typed(tcx, caller, typeck.expr_ty(expr))? != pointee
        || typed(tcx, caller, adjustment.target)? != reference
        || typed(tcx, caller, typeck.expr_ty_adjusted(expr))? != reference
    {
        return Err(rejected(
            "phase receiver substituted reference kind or pointee",
        ));
    }
    Ok(())
}

struct Scan<'w, 'tcx> {
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    target: Instance<'tcx>,
    span: rustc_span::Span,
    work: &'w mut usize,
    depth: usize,
    found: Option<&'tcx Expr<'tcx>>,
    error: Option<ProductionSemanticImportErrorV1>,
}
impl<'tcx> Scan<'_, 'tcx> {
    fn consider(&mut self, expr: &'tcx Expr<'tcx>) -> Result<()> {
        spend(self.work, 1)?;
        if expr.span != self.span || !matches!(expr.kind, ExprKind::MethodCall(..)) {
            return Ok(());
        }
        let Some(def) = self.typeck.type_dependent_def_id(expr.hir_id) else {
            return Ok(());
        };
        let args =
            definitions::normalize(self.tcx, self.caller, self.typeck.node_args(expr.hir_id))
                .map_err(|_| rejected("phase method generic arguments"))?;
        let called = Instance::try_resolve(
            self.tcx,
            TypingEnv::fully_monomorphized(),
            def,
            self.tcx.erase_and_anonymize_regions(args),
        )
        .ok()
        .flatten();
        if called == Some(self.target) && self.found.replace(expr).is_some() {
            return Err(rejected("phase HIR occurrence is not unique"));
        }
        Ok(())
    }
}
impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if self.depth == 128 {
            self.error = Some(rejected("phase HIR traversal depth ceiling"));
            return;
        }
        if let Err(error) = self.consider(expr) {
            self.error = Some(error);
            return;
        }
        if !matches!(expr.kind, ExprKind::Closure(_)) {
            self.depth += 1;
            intravisit::walk_expr(self, expr);
            self.depth -= 1;
        }
    }
}

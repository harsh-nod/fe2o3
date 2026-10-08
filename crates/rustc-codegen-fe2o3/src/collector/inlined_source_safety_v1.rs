//! Audit original source owners retained by MIR inlining, without adding call edges.
//! Logical work covers scope/argument records and bounded borrowed HIR visits.
//! Rustc query internals and interned type storage remain the compiler's domain.

use std::ops::ControlFlow;

use rustc_hir::def::DefKind;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{self as hir, Safety};
use rustc_middle::mir::Body;
use rustc_middle::ty::{EarlyBinder, Instance, InstanceKind, TyCtxt, TypingEnv};
use rustc_span::{Span, Symbol};

use crate::rustc_semantic_plan_v1::{ProductionSemanticPreflightErrorV1, SourceClosureWorkV1};

const MAX_HIR_DEPTH: usize = 256;

#[derive(Debug)]
pub(super) enum OriginRefusal {
    NonConcreteInstance,
    UnsupportedInstance,
    UnsafeSignature,
    MissingLocalBody,
    UserUnsafeBlock(Span),
    UnknownExternalSource,
    ExternalAuthentication(String),
    AdapterAuthentication(String),
    HirDepth,
}

#[derive(Debug)]
pub(super) enum AuditError<'tcx> {
    Work(ProductionSemanticPreflightErrorV1),
    CallerBodyMismatch {
        caller: Instance<'tcx>,
        span: Span,
    },
    Origin {
        instance: Instance<'tcx>,
        source: Instance<'tcx>,
        callsite: Span,
        reason: OriginRefusal,
    },
}

impl<'tcx> From<ProductionSemanticPreflightErrorV1> for AuditError<'tcx> {
    fn from(error: ProductionSemanticPreflightErrorV1) -> Self {
        Self::Work(error)
    }
}

/// The recognizer is only consulted for a concrete external original owner.
/// The parent must reuse its exact target/source authenticators, default false.
/// It receives the real shared ledger and cannot grant local-user source safety.
pub(super) fn audit<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    work: &mut SourceClosureWorkV1,
    mut external: impl FnMut(
        Instance<'tcx>,
        Span,
        &mut SourceClosureWorkV1,
    ) -> Result<bool, AuditError<'tcx>>,
) -> Result<(), AuditError<'tcx>> {
    work.charge(1)?;
    if body.source.instance != caller.def
        || body.source.promoted.is_some()
        || !std::ptr::eq(body, tcx.instance_mir(caller.def))
    {
        return Err(AuditError::CallerBodyMismatch {
            caller,
            span: body.span,
        });
    }
    work.charge(caller.args.len())?;
    if !super::is_fully_monomorphized(tcx, caller) {
        return Err(AuditError::CallerBodyMismatch {
            caller,
            span: body.span,
        });
    }
    // Audit every retained inline origin, including scopes with no surviving
    // statement. Such scopes still carry an erased empty user unsafe block.
    work.charge(body.source_scopes.len())?;
    for scope in &body.source_scopes {
        let Some((original, callsite)) = scope.inlined else {
            continue;
        };
        work.charge(1)?;
        work.charge(original.args.len())?;
        let instantiated = EarlyBinder::bind(original).instantiate(tcx, caller.args);
        let instance = tcx
            .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), instantiated)
            .map_err(|_| AuditError::Origin {
                instance: instantiated,
                source: instantiated,
                callsite,
                reason: OriginRefusal::NonConcreteInstance,
            })?;
        let reject = |reason| AuditError::Origin {
            instance,
            source: instance,
            callsite,
            reason,
        };
        work.charge(instance.args.len())?;
        if !super::is_fully_monomorphized(tcx, instance) {
            return Err(reject(OriginRefusal::NonConcreteInstance));
        }
        let source = match instance.def {
            InstanceKind::Item(_) => instance,
            InstanceKind::ClosureOnceShim { .. } => {
                work.charge(1)?;
                // Authenticate the generated adapter, then inspect its actual
                // closure source. This is not a collected or invented call edge.
                crate::closure_profile_v1::authenticate_once_shim_v1(tcx, instance)
                    .map_err(|error| {
                        reject(OriginRefusal::AdapterAuthentication(error.to_string()))
                    })?
                    .ok_or_else(|| reject(OriginRefusal::UnsupportedInstance))?
            }
            _ => return Err(reject(OriginRefusal::UnsupportedInstance)),
        };
        let reject = |reason| AuditError::Origin {
            instance,
            source,
            callsite,
            reason,
        };
        work.charge(source.args.len())?;
        if !super::is_fully_monomorphized(tcx, source) {
            return Err(reject(OriginRefusal::NonConcreteInstance));
        }
        let Some(local) = source.def_id().as_local() else {
            work.charge(1)?;
            let authenticated = external(source, callsite, work).map_err(|error| match error {
                AuditError::Origin { reason, .. } => reject(reason),
                other => other,
            })?;
            if !authenticated {
                return Err(reject(OriginRefusal::UnknownExternalSource));
            }
            continue;
        };
        work.charge(1)?;
        let safety = match tcx.def_kind(local) {
            DefKind::Fn | DefKind::AssocFn => {
                tcx.fn_sig(local)
                    .instantiate(tcx, source.args)
                    .skip_binder()
                    .safety
            }
            DefKind::Closure => source.args.as_closure().sig().skip_binder().safety,
            _ => return Err(reject(OriginRefusal::UnsupportedInstance)),
        };
        if safety != Safety::Safe {
            return Err(reject(OriginRefusal::UnsafeSignature));
        }
        work.charge(1)?;
        let hir = tcx
            .hir_maybe_body_owned_by(local)
            .ok_or_else(|| reject(OriginRefusal::MissingLocalBody))?;
        let mut visitor = SourceVisitor {
            work,
            depth: 0,
            instance,
            source,
            callsite,
        };
        if let ControlFlow::Break(error) = visitor.visit_body(hir) {
            return Err(error);
        }
    }
    Ok(())
}

type VisitResult<'tcx> = ControlFlow<AuditError<'tcx>>;

struct SourceVisitor<'a, 'tcx> {
    work: &'a mut SourceClosureWorkV1,
    depth: usize,
    instance: Instance<'tcx>,
    source: Instance<'tcx>,
    callsite: Span,
}

impl<'tcx> SourceVisitor<'_, 'tcx> {
    fn charge(&mut self, amount: usize) -> VisitResult<'tcx> {
        match self.work.charge(amount) {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => ControlFlow::Break(AuditError::Work(error)),
        }
    }

    fn refuse(&self, reason: OriginRefusal) -> VisitResult<'tcx> {
        ControlFlow::Break(AuditError::Origin {
            instance: self.instance,
            source: self.source,
            callsite: self.callsite,
            reason,
        })
    }

    fn enter(&mut self) -> VisitResult<'tcx> {
        self.charge(1)?;
        if self.depth == MAX_HIR_DEPTH {
            return self.refuse(OriginRefusal::HirDepth);
        }
        self.depth += 1;
        ControlFlow::Continue(())
    }
}

// These are the pinned rustc walk_body recursive families, also used by the
// existing raw-source metered visitor. ControlFlow stops a list on first error.
macro_rules! hir_walk {
    ($visit:ident, $walk:ident, $($arg:ident : $ty:ty),+ $(,)?) => {
        fn $visit(&mut self, $($arg: $ty),+) -> Self::Result {
            self.enter()?;
            let result = intravisit::$walk(self, $($arg),+);
            self.depth -= 1;
            result
        }
    };
}

impl<'v, 'tcx> Visitor<'v> for SourceVisitor<'_, 'tcx> {
    type Result = VisitResult<'tcx>;

    hir_walk!(visit_body, walk_body, body: &hir::Body<'v>);
    hir_walk!(visit_param, walk_param, value: &'v hir::Param<'v>);
    hir_walk!(visit_local, walk_local, value: &'v hir::LetStmt<'v>);
    hir_walk!(visit_stmt, walk_stmt, value: &'v hir::Stmt<'v>);
    hir_walk!(visit_arm, walk_arm, value: &'v hir::Arm<'v>);
    hir_walk!(visit_pat, walk_pat, value: &'v hir::Pat<'v>);
    hir_walk!(visit_pat_field, walk_pat_field, value: &'v hir::PatField<'v>);
    hir_walk!(visit_pat_expr, walk_pat_expr, value: &'v hir::PatExpr<'v>);
    hir_walk!(visit_ty, walk_ty, value: &'v hir::Ty<'v, hir::AmbigArg>);
    hir_walk!(visit_const_arg, walk_const_arg, value: &'v hir::ConstArg<'v, hir::AmbigArg>);
    hir_walk!(visit_const_item_rhs, walk_const_item_rhs, value: hir::ConstItemRhs<'v>);
    hir_walk!(visit_anon_const, walk_anon_const, value: &'v hir::AnonConst);
    hir_walk!(visit_inline_const, walk_inline_const, value: &'v hir::ConstBlock);
    hir_walk!(visit_generic_arg, walk_generic_arg, value: &'v hir::GenericArg<'v>);
    hir_walk!(visit_lifetime, walk_lifetime, value: &'v hir::Lifetime);
    hir_walk!(visit_expr_field, walk_expr_field, value: &'v hir::ExprField<'v>);
    hir_walk!(visit_const_arg_expr_field, walk_const_arg_expr_field, value: &'v hir::ConstArgExprField<'v>);
    hir_walk!(visit_pattern_type_pattern, walk_ty_pat, value: &'v hir::TyPat<'v>);
    hir_walk!(visit_generic_param, walk_generic_param, value: &'v hir::GenericParam<'v>);
    hir_walk!(visit_generics, walk_generics, value: &'v hir::Generics<'v>);
    hir_walk!(visit_where_predicate, walk_where_predicate, value: &'v hir::WherePredicate<'v>);
    hir_walk!(visit_fn_ret_ty, walk_fn_ret_ty, value: &'v hir::FnRetTy<'v>);
    hir_walk!(visit_fn_decl, walk_fn_decl, value: &'v hir::FnDecl<'v>);
    hir_walk!(visit_trait_ref, walk_trait_ref, value: &'v hir::TraitRef<'v>);
    hir_walk!(visit_param_bound, walk_param_bound, value: &'v hir::GenericBound<'v>);
    hir_walk!(visit_precise_capturing_arg, walk_precise_capturing_arg, value: &'v hir::PreciseCapturingArg<'v>);
    hir_walk!(visit_poly_trait_ref, walk_poly_trait_ref, value: &'v hir::PolyTraitRef<'v>);
    hir_walk!(visit_opaque_ty, walk_opaque_ty, value: &'v hir::OpaqueTy<'v>);
    hir_walk!(visit_path_segment, walk_path_segment, value: &'v hir::PathSegment<'v>);
    hir_walk!(visit_generic_args, walk_generic_args, value: &'v hir::GenericArgs<'v>);
    hir_walk!(visit_assoc_item_constraint, walk_assoc_item_constraint, value: &'v hir::AssocItemConstraint<'v>);

    fn visit_inline_asm(&mut self, value: &'v hir::InlineAsm<'v>, id: hir::HirId) -> Self::Result {
        self.enter()?;
        let result = (|| {
            self.charge(value.operands.len())?;
            intravisit::walk_inline_asm(self, value, id)
        })();
        self.depth -= 1;
        result
    }

    fn visit_qpath(&mut self, path: &'v hir::QPath<'v>, id: hir::HirId, _: Span) -> Self::Result {
        self.enter()?;
        let result = intravisit::walk_qpath(self, path, id);
        self.depth -= 1;
        result
    }

    fn visit_path(&mut self, path: &hir::Path<'v>, _: hir::HirId) -> Self::Result {
        self.enter()?;
        let result = intravisit::walk_path(self, path);
        self.depth -= 1;
        result
    }

    fn visit_block(&mut self, block: &'v hir::Block<'v>) -> Self::Result {
        self.enter()?;
        let result = if matches!(
            block.rules,
            hir::BlockCheckMode::UnsafeBlock(hir::UnsafeSource::UserProvided)
        ) {
            self.refuse(OriginRefusal::UserUnsafeBlock(block.span))
        } else {
            intravisit::walk_block(self, block)
        };
        self.depth -= 1;
        result
    }

    fn visit_expr(&mut self, expression: &'v hir::Expr<'v>) -> Self::Result {
        self.enter()?;
        let result = if matches!(expression.kind, hir::ExprKind::Closure(_)) {
            // An uncalled closure body is not this original owner's body.
            // A called/inlined closure has its own actual scope/Instance.
            ControlFlow::Continue(())
        } else {
            intravisit::walk_expr(self, expression)
        };
        self.depth -= 1;
        result
    }

    fn visit_id(&mut self, _: hir::HirId) -> Self::Result {
        self.charge(1)
    }
    fn visit_name(&mut self, _: Symbol) -> Self::Result {
        self.charge(1)
    }
}

/// The same reviewed external-source policy serves collected and inlined owners.
/// This recognizes only existing source exceptions; it adds no terminal or edge.
pub(super) fn reviewed_external_source<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expected_target: &str,
) -> Result<bool, String> {
    use crate::production_rustc_intrinsic_v1 as intrinsic;
    use crate::trusted_device_items as trusted;

    if instance.def_id().is_local() || !matches!(instance.def, InstanceKind::Item(_)) {
        return Ok(false);
    }
    let safety = match tcx.def_kind(instance.def_id()) {
        DefKind::Fn | DefKind::AssocFn => {
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args)
                .skip_binder()
                .safety
        }
        DefKind::Closure => instance.args.as_closure().sig().skip_binder().safety,
        _ => return Ok(false),
    };
    let atomic = intrinsic::is_reviewed_core_atomic_function_v1(tcx, instance);
    if safety == Safety::Unsafe
        && !atomic
        && !trusted::is_authenticated_gfx942_wave64_scan_instance_v1(tcx, instance, expected_target)
    {
        return Ok(false);
    }
    if trusted::classify(tcx, instance.def_id())
        .is_some_and(crate::production_semantic_terminal_v1::is_traversed_reviewed_helper_v1)
        || intrinsic::is_reviewed_device_global_mut_ptr_as_raw_v1(tcx, instance)
        || trusted::authenticate_reviewed_safe_core_slice_metadata_helper_v1(tcx, instance)
        || trusted::authenticate_reviewed_safe_core_scalar_bitcast_helper_v1(tcx, instance)
        || trusted::authenticate_reviewed_safe_core_fabs_f32_helper_v1(tcx, instance)
        || trusted::authenticate_reviewed_safe_core_wrapping_integer_helper_v1(tcx, instance)
        || trusted::authenticate_reviewed_safe_core_saturating_integer_helper_v1(tcx, instance)
        || trusted::authenticate_reviewed_safe_core_f32_is_finite_helper_v1(tcx, instance)
        || atomic
    {
        return Ok(true);
    }
    trusted::authenticate_reviewed_safe_external_helper_v1(tcx, instance.def_id())
}

impl AuditError<'_> {
    pub(super) fn describe(&self, tcx: TyCtxt<'_>) -> String {
        match self {
            Self::Work(error) => error.to_string(),
            Self::CallerBodyMismatch { caller, span } => format!(
                "cannot authenticate compiler-owned MIR for {caller:?} at {}",
                tcx.sess.source_map().span_to_diagnostic_string(*span),
            ),
            Self::Origin {
                instance,
                source,
                callsite,
                reason,
            } => {
                let detail = match reason {
                    OriginRefusal::UserUnsafeBlock(span) => format!(
                        "reaches a safe-signature local helper containing a user-provided unsafe block at {}",
                        tcx.sess.source_map().span_to_diagnostic_string(*span),
                    ),
                    OriginRefusal::UnsafeSignature => "reaches unsafe function instance".to_owned(),
                    OriginRefusal::UnknownExternalSource => "cannot authenticate the absence of user-provided unsafe blocks in external helper: cross-crate HIR is unavailable".to_owned(),
                    OriginRefusal::ExternalAuthentication(detail) =>
                        format!("rejected reviewed external helper: {detail}"),
                    OriginRefusal::AdapterAuthentication(detail) =>
                        format!("cannot authenticate inline adapter: {detail}"),
                    other => format!("cannot authenticate original inline source: {other:?}"),
                };
                format!(
                    "{detail}; original source {source:?}, inlined instance {instance:?} at {}",
                    tcx.sess.source_map().span_to_diagnostic_string(*callsite)
                )
            }
        }
    }
}

#[cfg(test)]
mod tests;

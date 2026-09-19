//! Borrowed raw-source admission and logical identity-preimage accounting.
//!
//! Units are portable logical record/edge visits, not Rust layout bytes or a
//! bound on rustc's hashing instructions. A raw record costs one census visit
//! plus its prepaid hash visits. Instance records prepay three visits (the two
//! instance hashes and the separate generic-argument hashes); body records
//! prepay one. Signature and HIR admission records have no hash prepayment.
//! Interned types are expanded per occurrence without a second graph or cache.
//! DefIds, spans, symbols and allocation IDs are opaque logical records: rustc
//! query work, interning, stable-hash context lookups, referenced allocations,
//! provider authentication and diagnostic formatting remain separate domains.
//!
//! No body, string representation or effect IR is cloned. Successful traversal
//! allocates no owned storage. The explicit recursion limit narrows admission;
//! coroutine/coverage metadata, pattern types and MIR inline assembly are refused
//! because this leaf does not census their variable payloads. This is not source
//! authority.

use std::ops::ControlFlow;

use rustc_abi::ExternAbi;
use rustc_hir::intravisit::{self, Visitor as HirVisitor};
use rustc_hir::{self as hir, Safety};
use rustc_middle::mir::visit::{TyContext, Visitor as MirVisitor};
use rustc_middle::mir::{self, Location};
use rustc_middle::ty::{self, Instance, TyCtxt, TypeSuperVisitable, TypeVisitable, TypeVisitor};
use rustc_span::{Span, Symbol};

use super::reference_extraction_work_v1::ReferenceExtractionWorkV1;
use super::{ReferenceBindingErrorV1, instantiated_signature};

const MAX_RAW_SOURCE_DEPTH_V1: usize = 256;
type VisitResult = ControlFlow<ReferenceBindingErrorV1>;

fn finish(result: VisitResult) -> Result<(), ReferenceBindingErrorV1> {
    match result {
        ControlFlow::Continue(()) => Ok(()),
        ControlFlow::Break(error) => Err(error),
    }
}

fn flow(result: Result<(), ReferenceBindingErrorV1>) -> VisitResult {
    match result {
        Ok(()) => ControlFlow::Continue(()),
        Err(error) => ControlFlow::Break(error),
    }
}

struct RawWork<'a, 'w> {
    meter: &'a ReferenceExtractionWorkV1<'w>,
    hash_visits: usize,
    depth: usize,
}

impl<'a, 'w> RawWork<'a, 'w> {
    fn new(
        meter: &'a ReferenceExtractionWorkV1<'w>,
        hash_visits: usize,
    ) -> Result<Self, ReferenceBindingErrorV1> {
        if !meter.is_shared() {
            return Err(ReferenceBindingErrorV1::new(
                "raw reference source accounting requires the inherited source work ledger",
            ));
        }
        Ok(Self {
            meter,
            hash_visits,
            depth: 0,
        })
    }

    fn records(&self, count: usize) -> Result<(), ReferenceBindingErrorV1> {
        let visits = self
            .hash_visits
            .checked_add(1)
            .ok_or_else(|| ReferenceBindingErrorV1::new("raw reference source work overflow"))?;
        self.meter.product(count, visits)
    }

    fn enter(&mut self) -> VisitResult {
        flow(self.records(1))?;
        if self.depth == MAX_RAW_SOURCE_DEPTH_V1 {
            return ControlFlow::Break(ReferenceBindingErrorV1::new(
                "raw reference source exceeds 256 recursive visit levels",
            ));
        }
        self.depth += 1;
        ControlFlow::Continue(())
    }
}

/// Debit the borrowed raw instance, instantiated signature and MIR before the
/// caller computes its existing identity hashes. Does not hash or authenticate.
pub(super) fn charge_reference_source_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    meter: &ReferenceExtractionWorkV1<'_>,
) -> Result<(), ReferenceBindingErrorV1> {
    let mut instance_work = RawTypeVisitor {
        work: RawWork::new(meter, 3)?,
    };
    instance_work.work.records(1)?;
    instance_work.work.records(instance.args.len())?;
    finish(instance.visit_with(&mut instance_work))?;

    meter.charge(1)?;
    let signature = instantiated_signature(tcx, instance);
    let mut signature_work = RawTypeVisitor {
        work: RawWork::new(meter, 0)?,
    };
    signature_work.work.records(1)?;
    signature_work
        .work
        .records(signature.inputs_and_output.len())?;
    finish(signature.visit_with(&mut signature_work))?;

    meter.charge(1)?;
    let body = tcx.instance_mir(instance.def);
    let mut visitor = RawMirVisitor {
        work: RawWork::new(meter, 1)?,
        error: None,
    };
    visitor.visit_body(body);
    visitor.finish()
}

/// Preserve the previous safe/local/Rust-ABI and user-unsafe-block admission,
/// but stop on exhausted work or excessive depth. Nested bodies remain skipped.
pub(super) fn authenticate_safe_local_reference_v1<'tcx>(
    meter: &ReferenceExtractionWorkV1<'_>,
    tcx: TyCtxt<'tcx>,
    reference: Instance<'tcx>,
) -> Result<(), ReferenceBindingErrorV1> {
    let work = RawWork::new(meter, 0)?;
    work.records(1)?;
    let signature = instantiated_signature(tcx, reference);
    if signature.safety != Safety::Safe {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' is declared unsafe",
            tcx.def_path_str(reference.def_id()),
        )));
    }
    if signature.abi != ExternAbi::Rust || signature.c_variadic {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' must use the non-variadic Rust ABI",
            tcx.def_path_str(reference.def_id()),
        )));
    }
    let Some(local) = reference.def_id().as_local() else {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' must be local so unsafe-block absence is authenticated",
            tcx.def_path_str(reference.def_id()),
        )));
    };
    work.records(1)?;
    let Some(body) = tcx.hir_maybe_body_owned_by(local) else {
        return Err(ReferenceBindingErrorV1::new(
            "safe Rust reference has no local HIR body",
        ));
    };
    let mut visitor = SafeHirVisitor {
        work,
        first_unsafe: None,
    };
    finish(visitor.visit_body(body))?;
    if let Some(span) = visitor.first_unsafe {
        return Err(ReferenceBindingErrorV1::new(format!(
            "safe Rust reference '{}' contains a user-provided unsafe block at {}",
            tcx.def_path_str(reference.def_id()),
            tcx.sess.source_map().span_to_diagnostic_string(span),
        )));
    }
    Ok(())
}

struct SafeHirVisitor<'a, 'w> {
    work: RawWork<'a, 'w>,
    first_unsafe: Option<Span>,
}

// Every recursive family reachable from walk_body has a fallible entry. The
// pinned walker propagates ControlFlow, so list traversal stops on exhaustion.
macro_rules! hir_walk {
    ($visit:ident, $walk:ident, $($arg:ident : $ty:ty),+ $(,)?) => {
        fn $visit(&mut self, $($arg: $ty),+) -> Self::Result {
            self.work.enter()?;
            let result = intravisit::$walk(self, $($arg),+);
            self.work.depth -= 1;
            result
        }
    };
}

impl<'v> HirVisitor<'v> for SafeHirVisitor<'_, '_> {
    type Result = VisitResult;

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
        self.work.enter()?;
        let result = (|| {
            // An output operand with no expression produces no child callback.
            flow(self.work.records(value.operands.len()))?;
            intravisit::walk_inline_asm(self, value, id)
        })();
        self.work.depth -= 1;
        result
    }

    fn visit_qpath(&mut self, path: &'v hir::QPath<'v>, id: hir::HirId, _: Span) -> Self::Result {
        self.work.enter()?;
        let result = intravisit::walk_qpath(self, path, id);
        self.work.depth -= 1;
        result
    }

    fn visit_path(&mut self, path: &hir::Path<'v>, _: hir::HirId) -> Self::Result {
        self.work.enter()?;
        let result = intravisit::walk_path(self, path);
        self.work.depth -= 1;
        result
    }

    fn visit_block(&mut self, block: &'v hir::Block<'v>) -> Self::Result {
        self.work.enter()?;
        if matches!(
            block.rules,
            hir::BlockCheckMode::UnsafeBlock(hir::UnsafeSource::UserProvided)
        ) {
            self.first_unsafe.get_or_insert(block.span);
        }
        let result = intravisit::walk_block(self, block);
        self.work.depth -= 1;
        result
    }

    fn visit_expr(&mut self, expression: &'v hir::Expr<'v>) -> Self::Result {
        self.work.enter()?;
        let result = if matches!(expression.kind, hir::ExprKind::Closure(_)) {
            ControlFlow::Continue(())
        } else {
            intravisit::walk_expr(self, expression)
        };
        self.work.depth -= 1;
        result
    }

    fn visit_id(&mut self, _: hir::HirId) -> Self::Result {
        flow(self.work.records(1))
    }

    fn visit_name(&mut self, _: Symbol) -> Self::Result {
        flow(self.work.records(1))
    }
}

struct RawTypeVisitor<'a, 'w> {
    work: RawWork<'a, 'w>,
}

impl<'tcx> TypeVisitor<TyCtxt<'tcx>> for RawTypeVisitor<'_, '_> {
    type Result = VisitResult;

    fn visit_ty(&mut self, value: ty::Ty<'tcx>) -> Self::Result {
        self.work.enter()?;
        // Auto-trait entries can have no type/region/const children.
        let result = (|| {
            // Pattern::visit_with recursively walks Or without a visitor hook.
            if matches!(value.kind(), ty::Pat(_, _)) {
                return ControlFlow::Break(ReferenceBindingErrorV1::new(
                    "raw reference source pattern types are outside the metered census",
                ));
            }
            if let ty::Dynamic(predicates, _) = value.kind() {
                flow(self.work.records(predicates.len()))?;
            }
            value.super_visit_with(self)
        })();
        self.work.depth -= 1;
        result
    }

    fn visit_const(&mut self, value: ty::Const<'tcx>) -> Self::Result {
        self.work.enter()?;
        let result = (|| {
            if let ty::ConstKind::Value(value) = value.kind() {
                flow(self.work.records(1))?;
                value.ty.visit_with(self)?;
                match **value.valtree {
                    ty::ValTreeKind::Leaf(_) => ControlFlow::Continue(()),
                    ty::ValTreeKind::Branch(children) => children.visit_with(self),
                }
            } else {
                value.super_visit_with(self)
            }
        })();
        self.work.depth -= 1;
        result
    }

    fn visit_region(&mut self, _: ty::Region<'tcx>) -> Self::Result {
        flow(self.work.records(1))
    }

    fn visit_binder<T: TypeVisitable<TyCtxt<'tcx>>>(
        &mut self,
        value: &ty::Binder<'tcx, T>,
    ) -> Self::Result {
        self.work.enter()?;
        let result = (|| {
            flow(self.work.records(value.bound_vars().len()))?;
            value.super_visit_with(self)
        })();
        self.work.depth -= 1;
        result
    }

    fn visit_predicate(&mut self, value: ty::Predicate<'tcx>) -> Self::Result {
        self.work.enter()?;
        let result = value.super_visit_with(self);
        self.work.depth -= 1;
        result
    }

    fn visit_clauses(&mut self, value: ty::Clauses<'tcx>) -> Self::Result {
        self.work.enter()?;
        let result = (|| {
            flow(self.work.records(value.len()))?;
            value.super_visit_with(self)
        })();
        self.work.depth -= 1;
        result
    }
}

struct RawMirVisitor<'a, 'w> {
    work: RawWork<'a, 'w>,
    error: Option<ReferenceBindingErrorV1>,
}

impl RawMirVisitor<'_, '_> {
    fn finish(self) -> Result<(), ReferenceBindingErrorV1> {
        self.error.map_or(Ok(()), Err)
    }

    fn records(&mut self, count: usize) -> bool {
        if self.error.is_some() {
            return false;
        }
        if let Err(error) = self.work.records(count) {
            self.error = Some(error);
            return false;
        }
        true
    }

    fn refuse(&mut self, message: &'static str) {
        if self.error.is_none() {
            self.error = Some(ReferenceBindingErrorV1::new(message));
        }
    }

    fn types<'tcx>(&mut self, value: &impl TypeVisitable<TyCtxt<'tcx>>) {
        if self.error.is_some() {
            return;
        }
        let mut visitor = RawTypeVisitor {
            work: RawWork {
                meter: self.work.meter,
                hash_visits: self.work.hash_visits,
                depth: 0,
            },
        };
        if let Err(error) = finish(value.visit_with(&mut visitor)) {
            self.error = Some(error);
        }
    }
}

macro_rules! mir_walk {
    ($visit:ident, $walk:ident, $($arg:ident : $ty:ty),+ $(,)?) => {
        fn $visit(&mut self, $($arg: $ty),+) {
            if self.records(1) {
                self.$walk($($arg),+);
            }
        }
    };
}

impl<'tcx> MirVisitor<'tcx> for RawMirVisitor<'_, '_> {
    fn visit_body(&mut self, body: &mir::Body<'tcx>) {
        if !self.records(1) {
            return;
        }
        if body.coroutine.is_some()
            || body.coverage_info_hi.is_some()
            || body.function_coverage_info.is_some()
        {
            self.refuse(
                "raw reference source coroutine/coverage metadata is outside the metered census",
            );
            return;
        }
        // Prepay every outer iterator: rustc's MIR visitor returns (), so after
        // an inner error the remaining outer callbacks must already be paid.
        for count in [
            body.basic_blocks.len(),
            body.local_decls.len(),
            body.source_scopes.len(),
            body.var_debug_info.len(),
            body.user_type_annotations.len(),
            body.required_consts.as_ref().map_or(0, Vec::len),
            body.mentioned_items.as_ref().map_or(0, Vec::len),
        ] {
            if !self.records(count) {
                return;
            }
        }
        self.types(&body.source.instance);
        if let Some(items) = &body.mentioned_items {
            for item in items {
                self.types(&item.node);
                if self.error.is_some() {
                    return;
                }
            }
        }
        self.super_body(body);
    }

    fn visit_basic_block_data(&mut self, block: mir::BasicBlock, data: &mir::BasicBlockData<'tcx>) {
        if self.records(1)
            && self.records(data.statements.len())
            && self.records(data.after_last_stmt_debuginfos.len())
        {
            self.super_basic_block_data(block, data);
        }
    }

    fn visit_statement(&mut self, statement: &mir::Statement<'tcx>, location: Location) {
        if !self.records(1) || !self.records(statement.debuginfos.len()) {
            return;
        }
        if matches!(statement.kind, mir::StatementKind::Coverage(_)) {
            self.refuse("raw reference source coverage statement is outside the metered census");
            return;
        }
        self.super_statement(statement, location);
    }

    fn visit_terminator(&mut self, terminator: &mir::Terminator<'tcx>, location: Location) {
        if !self.records(1) {
            return;
        }
        let count = match &terminator.kind {
            mir::TerminatorKind::SwitchInt { targets, .. } => targets.all_targets().len(),
            mir::TerminatorKind::Call { args, .. } | mir::TerminatorKind::TailCall { args, .. } => {
                args.len()
            }
            mir::TerminatorKind::InlineAsm { .. } => {
                self.refuse("raw reference source inline assembly is outside the metered census");
                return;
            }
            _ => 0,
        };
        if self.records(count) {
            self.super_terminator(terminator, location);
        }
    }

    fn visit_rvalue(&mut self, value: &mir::Rvalue<'tcx>, location: Location) {
        if !self.records(1) {
            return;
        }
        if let mir::Rvalue::Aggregate(_, operands) = value
            && !self.records(operands.len())
        {
            return;
        }
        self.super_rvalue(value, location);
    }

    fn visit_place(
        &mut self,
        place: &mir::Place<'tcx>,
        context: mir::visit::PlaceContext,
        location: Location,
    ) {
        if self.records(1) && self.records(place.projection.len()) {
            self.super_place(place, context, location);
        }
    }

    fn visit_const_operand(&mut self, value: &mir::ConstOperand<'tcx>, _: Location) {
        if self.records(1) {
            self.types(&value.const_);
        }
    }

    fn visit_local_decl(&mut self, local: mir::Local, value: &mir::LocalDecl<'tcx>) {
        if !self.records(1) {
            return;
        }
        if let Some(projections) = &value.user_ty
            && !self.records(projections.contents.len())
        {
            return;
        }
        if let mir::ClearCrossCrate::Set(info) = &value.local_info {
            if !self.records(1) {
                return;
            }
            if let mir::LocalInfo::User(mir::BindingForm::Var(binding)) = &**info {
                if !self.records(binding.introductions.len()) {
                    return;
                }
                if let Some((Some(place), _)) = &binding.opt_match_place {
                    self.visit_place(
                        place,
                        mir::visit::PlaceContext::NonUse(mir::visit::NonUseContext::VarDebugInfo),
                        Location::START,
                    );
                }
            }
        }
        self.super_local_decl(local, value);
    }

    fn visit_var_debug_info(&mut self, value: &mir::VarDebugInfo<'tcx>) {
        if !self.records(1) {
            return;
        }
        if let Some(fragment) = &value.composite
            && !self.records(fragment.projection.len())
        {
            return;
        }
        self.super_var_debug_info(value);
    }

    fn visit_user_type_projection(&mut self, value: &mir::UserTypeProjection) {
        if self.records(1) {
            self.records(value.projs.len());
        }
    }

    fn visit_user_type_annotation(
        &mut self,
        _: ty::UserTypeAnnotationIndex,
        value: &ty::CanonicalUserTypeAnnotation<'tcx>,
    ) {
        if !self.records(1) || !self.records(value.user_ty.var_kinds.len()) {
            return;
        }
        self.types(&value.user_ty.value);
        self.types(&value.inferred_ty);
    }

    fn visit_ty(&mut self, value: ty::Ty<'tcx>, _: TyContext) {
        self.types(&value);
    }
    fn visit_ty_const(&mut self, value: ty::Const<'tcx>, _: Location) {
        self.types(&value);
    }
    fn visit_args(&mut self, value: &ty::GenericArgsRef<'tcx>, _: Location) {
        if self.records(value.len()) {
            self.types(value);
        }
    }
    fn visit_region(&mut self, value: ty::Region<'tcx>, _: Location) {
        self.types(&value);
    }

    mir_walk!(visit_operand, super_operand, value: &mir::Operand<'tcx>, location: Location);
    mir_walk!(visit_source_scope_data, super_source_scope_data, value: &mir::SourceScopeData<'tcx>);
    mir_walk!(visit_statement_debuginfo, super_statement_debuginfo, value: &mir::StmtDebugInfo<'tcx>, location: Location);
    mir_walk!(visit_assert_message, super_assert_message, value: &mir::AssertMessage<'tcx>, location: Location);
}

#[cfg(test)]
#[path = "reference_raw_source_work_v1_tests.rs"]
mod tests;

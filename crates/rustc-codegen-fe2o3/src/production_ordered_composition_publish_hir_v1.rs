//! HIR-backed edit anchors, never caller-provided offsets.
use super::origin::{ObservedSite, direct_macro_site};
use super::{Error, Result, file};
use rustc_hir::def::Res;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BindingMode, Body, BodyId, Expr, ExprKind, ItemId, ItemKind, PatKind};
use rustc_middle::ty::{Instance, TyCtxt, TypeckResults};
use rustc_span::{Ident, SourceFile, Span, Symbol};
use std::ops::ControlFlow;
use std::sync::Arc;

pub(super) struct Anchor {
    pub(super) file: Arc<SourceFile>,
    pub(super) body: Span,
    pub(super) selected: Span,
    pub(super) parameters: [Ident; 3],
}

struct Scan<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    site: &'a ObservedSite,
    helper: &'a str,
    nodes: usize,
    depth: usize,
    selected: Option<[&'tcx Expr<'tcx>; 3]>,
}
impl Scan<'_, '_> {
    fn tick(&mut self) -> ControlFlow<Error> {
        self.nodes += 1;
        if self.nodes > 4096 {
            return ControlFlow::Break(Error::refused("publisher HIR node bound exceeded"));
        }
        ControlFlow::Continue(())
    }
    fn enter(&mut self) -> ControlFlow<Error> {
        self.tick()?;
        self.depth += 1;
        if self.depth > 64 {
            return ControlFlow::Break(Error::refused("publisher HIR nesting bound exceeded"));
        }
        ControlFlow::Continue(())
    }
}
impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    type Result = ControlFlow<Error>;

    fn visit_name(&mut self, name: Symbol) -> Self::Result {
        self.tick()?;
        if name.as_str() == self.helper {
            return ControlFlow::Break(Error::refused(
                "publisher helper name collides in actual HIR",
            ));
        }
        ControlFlow::Continue(())
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) -> Self::Result {
        self.enter()?;
        if matches!(expr.kind, ExprKind::Closure(_)) {
            return ControlFlow::Break(Error::refused("publisher refuses nested closure capture"));
        }
        if let ExprKind::Call(callee, args) = expr.kind
            && let ExprKind::Path(ref path) = callee.kind
            && let Res::Def(_, definition) = self.typeck.qpath_res(path, callee.hir_id)
            && definition == self.site.marker_definition
            && expr.span.from_expansion()
            && expr.span.ctxt().outer_expn_data().call_site == self.site.callsite
        {
            let callsite = match direct_macro_site(expr.span, self.site.macro_definition) {
                Ok(value) => value,
                Err(error) => return ControlFlow::Break(error),
            };
            if callsite != self.site.callsite
                || args.len() != 8
                || self.typeck.expr_ty(expr) != self.tcx.types.u32
                || !self.typeck.expr_adjustments(expr).is_empty()
                || self
                    .selected
                    .replace([&args[0], &args[1], &args[2]])
                    .is_some()
            {
                return ControlFlow::Break(Error::refused(
                    "publisher actual HIR call is ambiguous or differs",
                ));
            }
        }
        let result = intravisit::walk_expr(self, expr);
        self.depth -= 1;
        result
    }

    fn visit_pat(&mut self, pat: &'tcx rustc_hir::Pat<'tcx>) -> Self::Result {
        self.enter()?;
        let result = intravisit::walk_pat(self, pat);
        self.depth -= 1;
        result
    }

    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_block(self, block)
    }

    fn visit_stmt(&mut self, statement: &'tcx rustc_hir::Stmt<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_stmt(self, statement)
    }

    fn visit_ty(&mut self, ty: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) -> Self::Result {
        self.enter()?;
        let result = intravisit::walk_ty(self, ty);
        self.depth -= 1;
        result
    }

    fn visit_nested_body(&mut self, id: BodyId) -> Self::Result {
        self.tick()?;
        let body = self.tcx.hir_body(id);
        // Only the authenticated macro's literal packing const arguments may
        // remain opaque. User const/closure bodies could introduce unscanned
        // references changed by adding a local helper, so they are refused.
        if !body.value.span.from_expansion()
            || body.value.span.ctxt().outer_expn_data().macro_def_id
                != Some(self.site.macro_definition)
        {
            return ControlFlow::Break(Error::refused(
                "publisher refuses non-marker nested bodies",
            ));
        }
        ControlFlow::Continue(())
    }

    fn visit_nested_item(&mut self, id: ItemId) -> Self::Result {
        self.tick()?;
        let item = self.tcx.hir_item(id);
        // Local user items could shadow a generated helper or contain a use that
        // newly resolves to it. The first publisher refuses them entirely.
        // The trusted macro's const packing items are distinct retained HIR items;
        // they have no value-namespace imports and need no nested-body scan.
        let ItemKind::Const(ident, _, _, _) = item.kind else {
            return ControlFlow::Break(Error::refused(
                "publisher refuses preexisting body-local items",
            ));
        };
        if !item.span.from_expansion()
            || item.span.ctxt().outer_expn_data().macro_def_id != Some(self.site.macro_definition)
        {
            return ControlFlow::Break(Error::refused(
                "publisher refuses non-marker local const items",
            ));
        }
        self.visit_ident(ident)
    }
}

// Keep the original bound and exact-name policy without copying the namespace.
fn check_enclosing_names<'a>(
    mut names: impl ExactSizeIterator<Item = &'a str>,
    helper: &str,
) -> Result<()> {
    if names.len() > 4096 {
        return Err(Error::refused(
            "publisher enclosing namespace bound exceeded",
        ));
    }
    if names.any(|name| name == helper) {
        return Err(Error::refused(
            "publisher helper name collides in enclosing namespace",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_ordered_composition_publish_namespace_v1_tests.rs"]
mod namespace_tests;

pub(super) fn capture<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &'tcx Body<'tcx>,
    site: ObservedSite,
    helper: &str,
) -> Result<Anchor> {
    let local = instance
        .def_id()
        .as_local()
        .ok_or_else(|| Error::refused("publisher selected function is not local"))?;
    if !instance.args.is_empty()
        || tcx
            .hir_maybe_body_owned_by(local)
            .is_none_or(|current| !std::ptr::eq(current, body))
    {
        return Err(Error::refused("publisher current local HIR owner differs"));
    }
    let ExprKind::Block(block, _) = body.value.kind else {
        return Err(Error::refused(
            "publisher requires an actual enclosing block body",
        ));
    };
    if body.value.span.is_dummy()
        || body.value.span.from_expansion()
        || block.span.is_dummy()
        || block.span.from_expansion()
        || block.span.lo() > site.callsite.lo()
        || block.span.hi() < site.callsite.hi()
        || body.params.len() > 16
    {
        return Err(Error::refused(
            "publisher requires direct enclosing body and parameter spans",
        ));
    }
    let file = file::bounded_file(tcx, block.span)?;
    let selected_file = file::bounded_file(tcx, site.callsite)?;
    if !Arc::ptr_eq(&file, &selected_file) {
        return Err(Error::refused(
            "publisher body and macro are not in the same source file",
        ));
    }
    let module = tcx.parent_module_from_def_id(local);
    // This is the authenticated local HIR owner's enclosing module. The
    // external module_children query has no local provider in the pinned rustc.
    // The local resolver roster includes imports and other named children.
    let children = tcx.module_children_local(module.to_local_def_id());
    check_enclosing_names(
        children.iter().map(|child| child.ident.name.as_str()),
        helper,
    )?;
    let typeck = tcx.typeck(local);
    let mut scan = Scan {
        tcx,
        typeck,
        site: &site,
        helper,
        nodes: 0,
        depth: 0,
        selected: None,
    };
    if let ControlFlow::Break(error) = scan.visit_body(body) {
        return Err(error);
    }
    let arguments = scan
        .selected
        .ok_or_else(|| Error::refused("publisher selected actual HIR marker call is absent"))?;
    let mut parameters = [None; 3];
    for (ordinal, argument) in arguments.into_iter().enumerate() {
        if typeck.expr_ty(argument) != tcx.types.u32
            || !typeck.expr_adjustments(argument).is_empty()
        {
            return Err(Error::refused(
                "publisher argument is not an unadjusted u32",
            ));
        }
        let ExprKind::Path(ref path) = argument.kind else {
            return Err(Error::refused(
                "publisher argument is not an immutable parameter path",
            ));
        };
        let Res::Local(binding) = typeck.qpath_res(path, argument.hir_id) else {
            return Err(Error::refused(
                "publisher argument is not an actual local binding",
            ));
        };
        for parameter in body.params {
            if let PatKind::Binding(mode, id, ident, None) = parameter.pat.kind
                && id == binding
            {
                if mode != BindingMode::NONE
                    || typeck.pat_ty(parameter.pat) != tcx.types.u32
                    || ident.span.is_dummy()
                    || ident.span.from_expansion()
                    || parameters[ordinal].replace(ident).is_some()
                {
                    return Err(Error::refused("publisher parameter binding shape differs"));
                }
            }
        }
        if parameters[ordinal].is_none() {
            return Err(Error::refused(
                "publisher refuses local, constant or captured operands",
            ));
        }
    }
    let [Some(a), Some(b), Some(c)] = parameters else {
        return Err(Error::refused("publisher parameter roster is incomplete"));
    };
    Ok(Anchor {
        file,
        body: block.span,
        selected: site.callsite,
        parameters: [a, b, c],
    })
}

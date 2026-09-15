use super::bounded::{Output, Result};
use rustc_hir::{
    Expr, ExprKind, Pat, PatKind, Stmt, StmtKind,
    def::DefKind,
    def_id::LocalDefId,
    intravisit::{self, Visitor},
};
use rustc_middle::ty::{TyCtxt, TypeckResults};

const MAX_DEPTH: usize = 128;

pub(super) fn dump<'tcx>(tcx: TyCtxt<'tcx>, owner: LocalDefId, out: &mut Output) -> Result {
    if !matches!(
        tcx.def_kind(owner),
        DefKind::Fn | DefKind::AssocFn | DefKind::Closure
    ) {
        return out.line(format_args!(
            " HIR not a lexical function/closure; def={:?}",
            tcx.def_path_hash(owner.to_def_id())
        ));
    }
    let body = tcx.hir_body_owned_by(owner);
    out.line(format_args!(
        " HIR_BEGIN owner.def={:?} body={:?}; nested closures visited separately when selected",
        tcx.def_path_hash(owner.to_def_id()),
        body.id()
    ))?;
    let mut scan = Scan {
        tcx,
        typeck: tcx.typeck(owner),
        out,
        depth: 0,
        error: None,
    };
    scan.visit_body(body);
    if let Some(error) = scan.error {
        return Err(error);
    }
    scan.out.line(format_args!(" HIR_END"))
}

struct Scan<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    out: &'a mut Output,
    depth: usize,
    error: Option<&'static str>,
}

impl Scan<'_, '_> {
    fn enter(&mut self) -> bool {
        if self.error.is_some() {
            return false;
        }
        if self.depth == MAX_DEPTH {
            self.error = Some("diagnostic HIR depth bound reached");
            return false;
        }
        if let Err(error) = self.out.charge(1) {
            self.error = Some(error);
            return false;
        }
        self.depth += 1;
        true
    }
}

impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    // Typed expression/adjustment output already records these edges. Do not
    // recursively walk type syntax while observing value bindings and returns.
    fn visit_ty(&mut self, _: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) {}

    fn visit_path(&mut self, _: &rustc_hir::Path<'tcx>, _: rustc_hir::HirId) {}

    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) {
        if !self.enter() {
            return;
        }
        intravisit::walk_block(self, block);
        self.depth -= 1;
    }

    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if !self.enter() {
            return;
        }
        let result = (|| -> Result {
            let adjustments = self.typeck.expr_adjustments(expr);
            self.out.charge(adjustments.len())?;
            self.out.line(format_args!(
                " HIR.expr={:?} span={:?} ty={:?} adjusted={:?} adjustments={adjustments:?}",
                expr.hir_id,
                expr.span,
                self.typeck.expr_ty(expr),
                self.typeck.expr_ty_adjusted(expr)
            ))?;
            if let Some(def) = self.typeck.type_dependent_def_id(expr.hir_id) {
                self.out.line(format_args!(
                    " HIR.method.def={:?} generic.args={:?}",
                    self.tcx.def_path_hash(def),
                    self.typeck.node_args(expr.hir_id)
                ))?;
            }
            match expr.kind {
                ExprKind::MethodCall(_, receiver, args, _) => {
                    self.out.charge(args.len())?;
                    self.out.line(format_args!(
                        " HIR.MethodCall receiver={:?} arg.count={}",
                        receiver.hir_id,
                        args.len()
                    ))?;
                    for arg in args {
                        self.out.line(format_args!("  HIR.arg={:?}", arg.hir_id))?;
                    }
                }
                ExprKind::Call(callee, args) => {
                    self.out.charge(args.len())?;
                    self.out.line(format_args!(
                        " HIR.Call callee={:?} arg.count={}",
                        callee.hir_id,
                        args.len()
                    ))?;
                    for arg in args {
                        self.out.line(format_args!("  HIR.arg={:?}", arg.hir_id))?;
                    }
                }
                ExprKind::Path(ref path) => self.out.line(format_args!(
                    " HIR.Path resolution={:?}",
                    self.typeck.qpath_res(path, expr.hir_id)
                ))?,
                ExprKind::Closure(closure) => self.out.line(format_args!(
                    " HIR.Closure def={:?} body={:?}",
                    self.tcx.def_path_hash(closure.def_id.to_def_id()),
                    closure.body
                ))?,
                ExprKind::Block(block, _) => self.out.line(format_args!(
                    " HIR.Block tail={:?}",
                    block.expr.map(|e| e.hir_id)
                ))?,
                ExprKind::Ret(value) => self.out.line(format_args!(
                    " HIR.Return value={:?}",
                    value.map(|e| e.hir_id)
                ))?,
                _ => self.out.line(format_args!(
                    " HIR.other kind.discriminant={:?}",
                    std::mem::discriminant(&expr.kind)
                ))?,
            }
            Ok(())
        })();
        if let Err(error) = result {
            self.error = Some(error);
        }
        if self.error.is_none() {
            intravisit::walk_expr(self, expr);
        }
        self.depth -= 1;
    }

    fn visit_stmt(&mut self, stmt: &'tcx Stmt<'tcx>) {
        if !self.enter() {
            return;
        }
        let result = match stmt.kind {
            StmtKind::Let(local) => self.out.line(format_args!(
                " HIR.Let stmt={:?} pattern={:?} initializer={:?}",
                stmt.hir_id,
                local.pat.hir_id,
                local.init.map(|e| e.hir_id)
            )),
            _ => self.out.line(format_args!(
                " HIR.stmt={:?} kind.discriminant={:?}",
                stmt.hir_id,
                std::mem::discriminant(&stmt.kind)
            )),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
        if self.error.is_none() {
            intravisit::walk_stmt(self, stmt);
        }
        self.depth -= 1;
    }

    fn visit_pat(&mut self, pat: &'tcx Pat<'tcx>) {
        if !self.enter() {
            return;
        }
        let result = match pat.kind {
            PatKind::Binding(mode, binding, _, sub) => self.out.line(format_args!(
                " HIR.Binding pattern={:?} binding={binding:?} mode={mode:?} sub={:?}",
                pat.hir_id,
                sub.map(|p| p.hir_id)
            )),
            _ => self.out.line(format_args!(
                " HIR.pattern={:?} kind.discriminant={:?}",
                pat.hir_id,
                std::mem::discriminant(&pat.kind)
            )),
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
        if self.error.is_none() {
            intravisit::walk_pat(self, pat);
        }
        self.depth -= 1;
    }
}

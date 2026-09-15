use super::*;
use rustc_hir::{
    Expr, ExprKind, Pat, PatKind, Stmt, StmtKind,
    def::{DefKind, Res},
    intravisit::{self, Visitor},
};
use rustc_middle::ty::adjustment::{Adjust, AutoBorrow, AutoBorrowMutability};
use rustc_span::Span;

pub(super) struct Observed<'tcx> {
    pub(super) nodes: Nodes<'tcx>,
    pub(super) outer: Instance<'tcx>,
    pub(super) helper: Instance<'tcx>,
    pub(super) closure: Instance<'tcx>,
    pub(super) issue: Instance<'tcx>,
    pub(super) stage: Instance<'tcx>,
    pub(super) issue_span: Span,
    pub(super) helper_span: Span,
    pub(super) capture_span: Span,
    pub(super) stage_span: Span,
    pub(super) issued_type: ty::Ty<'tcx>,
    pub(super) staged_type: ty::Ty<'tcx>,
    pub(super) workgroup_type: ty::Ty<'tcx>,
}

fn local_path(typeck: &ty::TypeckResults<'_>, expr: &Expr<'_>) -> Result<HirId> {
    let ExprKind::Path(ref path) = expr.kind else {
        return Err(Error::Source("owned receiver is not an exact local path"));
    };
    match typeck.qpath_res(path, expr.hir_id) {
        Res::Local(binding) => Ok(binding),
        _ => Err(Error::Source(
            "owned receiver does not resolve to a source binding",
        )),
    }
}

fn unadjusted(typeck: &ty::TypeckResults<'_>, expr: &Expr<'_>) -> Result<()> {
    if !typeck.expr_adjustments(expr).is_empty() {
        return Err(Error::Source(
            "owned receiver has a borrow or coercion adjustment",
        ));
    }
    Ok(())
}

fn resolve<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    typeck: &ty::TypeckResults<'tcx>,
    expr: &Expr<'tcx>,
) -> Result<Instance<'tcx>> {
    let def = typeck
        .type_dependent_def_id(expr.hir_id)
        .ok_or(Error::Source(
            "owned flow method has no resolved definition",
        ))?;
    let args = normalize(tcx, instance, typeck.node_args(expr.hir_id))?;
    Instance::try_resolve(
        tcx,
        ty::TypingEnv::fully_monomorphized(),
        def,
        tcx.erase_and_anonymize_regions(args),
    )
    .ok()
    .flatten()
    .ok_or(Error::Source(
        "owned flow method lost exact monomorphic instance",
    ))
}

struct Index<'a, 'tcx> {
    expressions: Vec<&'tcx Expr<'tcx>>,
    bindings: Vec<(&'tcx Pat<'tcx>, &'tcx Expr<'tcx>)>,
    work: &'a mut usize,
    depth: usize,
    error: Option<Error>,
}

impl<'tcx> Index<'_, 'tcx> {
    fn initializer(&mut self, binding: HirId) -> Result<&'tcx Expr<'tcx>> {
        let mut result = None;
        for (pat, expr) in &self.bindings {
            bounded::charge(self.work, 1)?;
            if let PatKind::Binding(mode, id, _, sub) = pat.kind {
                if id == binding {
                    if mode != rustc_hir::BindingMode::NONE
                        || sub.is_some()
                        || result.replace(*expr).is_some()
                    {
                        return Err(Error::Source(
                            "owned binding is mutable, projected or ambiguous",
                        ));
                    }
                }
            }
        }
        result.ok_or(Error::Source("owned binding has no exact initializer"))
    }
}

impl<'tcx> Visitor<'tcx> for Index<'_, 'tcx> {
    fn visit_ty(&mut self, _: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) {}
    fn visit_path(&mut self, _: &rustc_hir::Path<'tcx>, _: HirId) {}
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if self.depth == bounded::MAX_DEPTH {
            self.error = Some(Error::Depth);
            return;
        }
        if let Err(error) = bounded::charge(self.work, 1)
            .and_then(|_| bounded::push(&mut self.expressions, expr, self.work))
        {
            self.error = Some(error);
            return;
        }
        if !matches!(expr.kind, ExprKind::Closure(_)) {
            self.depth += 1;
            intravisit::walk_expr(self, expr);
            self.depth -= 1;
        }
    }
    fn visit_stmt(&mut self, stmt: &'tcx Stmt<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = bounded::charge(self.work, 1) {
            self.error = Some(error);
            return;
        }
        if let StmtKind::Let(local) = stmt.kind {
            if let Some(init) = local.init {
                if let Err(error) = bounded::push(&mut self.bindings, (local.pat, init), self.work)
                {
                    self.error = Some(error);
                    return;
                }
            }
        }
        intravisit::walk_stmt(self, stmt);
    }
    fn visit_pat(&mut self, _: &'tcx Pat<'tcx>) {}
}

pub(super) fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    auth: &Authentication<'_, 'tcx>,
    publish: &TerminalExpansionRecipeV1<'tcx>,
    work: &mut usize,
) -> Result<Observed<'tcx>> {
    auth::terminal(tcx, auth, publish, Terminal::Gfx950TransposePublish, work)?;
    let outer = auth
        .retained
        .function_producers()
        .get(publish.caller.index() as usize)
        .ok_or(Error::Source("Publish source caller is absent"))?
        .instance;
    let lexical = outer
        .def_id()
        .as_local()
        .ok_or(Error::Source("Publish source caller has no local HIR"))?;
    if !matches!(outer.def, ty::InstanceKind::Item(_))
        || !matches!(tcx.def_kind(lexical), DefKind::Fn | DefKind::Closure)
    {
        return Err(Error::Source(
            "Publish source caller is not an original lexical body",
        ));
    }
    let body = tcx.hir_body_owned_by(lexical);
    let typeck = tcx.typeck(lexical);
    let mut index = Index {
        expressions: Vec::new(),
        bindings: Vec::new(),
        work,
        depth: 0,
        error: None,
    };
    index.visit_expr(body.value);
    if let Some(error) = index.error {
        return Err(error);
    }
    let mut call = None;
    for expr in &index.expressions {
        bounded::charge(index.work, 1)?;
        if expr.span == publish.span
            && matches!(expr.kind, ExprKind::MethodCall(..))
            && resolve(tcx, outer, typeck, expr)? == publish.instance
            && call.replace(*expr).is_some()
        {
            return Err(Error::Source("Publish source expression is ambiguous"));
        }
    }
    let call = call.ok_or(Error::Source(
        "Publish lacks its exact typed source expression",
    ))?;
    let ExprKind::MethodCall(_, receiver, [workgroup], _) = call.kind else {
        return Err(Error::Source("Publish source arity changed"));
    };
    for expr in [call, receiver, workgroup] {
        unadjusted(typeck, expr)?;
    }
    let staged_binding = local_path(typeck, receiver)?;
    let workgroup_binding = local_path(typeck, workgroup)?;
    let initializer = index.initializer(staged_binding)?;
    unadjusted(typeck, initializer)?;
    let ExprKind::MethodCall(_, _, [_, closure_expr], _) = initializer.kind else {
        return Err(Error::Source(
            "staged binding lacks the exact matrix helper initializer",
        ));
    };
    unadjusted(typeck, closure_expr)?;
    let ExprKind::Closure(closure_hir) = closure_expr.kind else {
        return Err(Error::Source(
            "matrix helper does not receive its direct source closure",
        ));
    };
    let closure_ty = normalize(tcx, outer, typeck.expr_ty(closure_expr))?;
    let ty::Closure(closure_def, closure_args) = *closure_ty.kind() else {
        return Err(Error::Source("matrix helper source closure type changed"));
    };
    if closure_def != closure_hir.def_id.to_def_id()
        || closure_args.as_closure().kind() != ty::ClosureKind::FnOnce
    {
        return Err(Error::Source(
            "transpose tile closure is not its exact FnOnce instance",
        ));
    }
    let closure = Instance::try_resolve(
        tcx,
        ty::TypingEnv::fully_monomorphized(),
        closure_def,
        tcx.erase_and_anonymize_regions(closure_args),
    )
    .ok()
    .flatten()
    .ok_or(Error::Source(
        "transpose source closure lost exact monomorphic instance",
    ))?;
    function_for_instance(auth.retained, closure, index.work)?;
    let helper = resolve(tcx, outer, typeck, initializer)?;
    function_for_instance(auth.retained, helper, index.work)?;
    let closure_body = tcx.hir_body(closure_hir.body);
    let ExprKind::Block(block, _) = closure_body.value.kind else {
        return Err(Error::Source("Stage closure has no original block tail"));
    };
    let stage = block
        .expr
        .ok_or(Error::Source("Stage closure has no result expression"))?;
    let ExprKind::MethodCall(_, tile, [_, _, _], _) = stage.kind else {
        return Err(Error::Source(
            "Stage closure result is not an exact Stage call",
        ));
    };
    let closure_typeck = tcx.typeck(closure_hir.def_id);
    for expr in [stage, tile] {
        unadjusted(closure_typeck, expr)?;
    }
    let issued_binding = local_path(closure_typeck, tile)?;
    if issued_binding == staged_binding
        || issued_binding == workgroup_binding
        || staged_binding == workgroup_binding
    {
        return Err(Error::Source("transpose source roles share a binding"));
    }
    let issue_expr = index.initializer(issued_binding)?;
    unadjusted(typeck, issue_expr)?;
    let ExprKind::MethodCall(_, _, [], _) = issue_expr.kind else {
        return Err(Error::Source(
            "issued tile binding has no exact issuer call",
        ));
    };
    let issue = resolve(tcx, outer, typeck, issue_expr)?;
    let stage_instance = resolve(tcx, closure, closure_typeck, stage)?;
    let issued_type = normalize(tcx, outer, typeck.expr_ty(issue_expr))?;
    let staged_type = normalize(tcx, outer, typeck.expr_ty(receiver))?;
    if normalize(tcx, closure, closure_typeck.expr_ty(tile))? != issued_type
        || normalize(tcx, closure, closure_typeck.expr_ty(stage))? != staged_type
        || normalize(tcx, outer, typeck.expr_ty(initializer))? != staged_type
    {
        return Err(Error::Source(
            "transpose source binding/input/result type edge changed",
        ));
    }
    let mut capture_field = None;
    for (field, capture) in tcx.closure_captures(closure_hir.def_id).iter().enumerate() {
        bounded::charge(index.work, 1 + capture.place.projections.len())?;
        use rustc_middle::hir::place::PlaceBase;
        if let PlaceBase::Upvar(upvar) = capture.place.base {
            if upvar.var_path.hir_id == issued_binding {
                if upvar.closure_expr_id != closure_hir.def_id
                    || !capture.place.projections.is_empty()
                    || capture.info.capture_kind != ty::UpvarCapture::ByValue
                    || capture.mutability != rustc_hir::Mutability::Not
                    || normalize(tcx, outer, capture.place.base_ty)?
                        != normalize(tcx, outer, typeck.expr_ty(issue_expr))?
                    || capture_field
                        .replace(u32::try_from(field).map_err(|_| Error::Work)?)
                        .is_some()
                {
                    return Err(Error::Source(
                        "tile capture is not its unique whole by-value source binding",
                    ));
                }
            }
        }
    }
    let capture_field = capture_field.ok_or(Error::Source(
        "tile binding is missing from original captures",
    ))?;
    let mut parameter = None;
    for (position, param) in body.params.iter().enumerate() {
        bounded::charge(index.work, 1)?;
        if let PatKind::Binding(mode, id, _, sub) = param.pat.kind {
            if id == workgroup_binding {
                if mode != rustc_hir::BindingMode::NONE
                    || sub.is_some()
                    || parameter.replace(position).is_some()
                {
                    return Err(Error::Source(
                        "Publish Workgroup source parameter is not a unique owned binding",
                    ));
                }
            }
        }
    }
    let nodes = Nodes {
        outer: lexical,
        closure: closure_hir.def_id,
        issued_binding,
        staged_binding,
        workgroup_binding,
        issue: issue_expr.hir_id,
        matrix_call: initializer.hir_id,
        capture: closure_expr.hir_id,
        stage: stage.hir_id,
        stage_receiver: tile.hir_id,
        publish: call.hir_id,
        publish_receiver: receiver.hir_id,
        workgroup_receiver: workgroup.hir_id,
        capture_field,
        workgroup_parameter: parameter.ok_or(Error::Source(
            "Publish Workgroup is not its retained source parameter",
        ))?,
        borrows: Vec::new(),
    };
    let work = index.work;
    let mut audit = Audit {
        tcx,
        auth,
        outer,
        caller: publish.caller,
        nodes,
        parent: None,
        lexical,
        uses: rules::Uses::default(),
        work,
        depth: 0,
        error: None,
    };
    audit.visit_body(body);
    if let Some(error) = audit.error {
        return Err(error);
    }
    audit.uses.finish(audit.nodes.borrows.len())?;
    Ok(Observed {
        nodes: audit.nodes,
        outer,
        helper,
        closure,
        issue,
        stage: stage_instance,
        issue_span: issue_expr.span,
        helper_span: initializer.span,
        capture_span: closure_expr.span,
        stage_span: stage.span,
        issued_type,
        staged_type,
        workgroup_type: normalize(tcx, outer, typeck.expr_ty(workgroup))?,
    })
}

struct Audit<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    auth: &'a Authentication<'a, 'tcx>,
    outer: Instance<'tcx>,
    caller: SemanticFunctionIdV1,
    nodes: Nodes<'tcx>,
    parent: Option<&'tcx Expr<'tcx>>,
    lexical: LocalDefId,
    uses: rules::Uses,
    work: &'a mut usize,
    depth: usize,
    error: Option<Error>,
}

impl<'tcx> Audit<'_, 'tcx> {
    fn path(&mut self, expr: &'tcx Expr<'tcx>) -> Result<()> {
        let typeck = self.tcx.typeck(self.lexical);
        let ExprKind::Path(ref path) = expr.kind else {
            return Ok(());
        };
        let Res::Local(binding) = typeck.qpath_res(path, expr.hir_id) else {
            return Ok(());
        };
        let role = if binding == self.nodes.issued_binding {
            rules::BindingRole::IssuedTile
        } else if binding == self.nodes.staged_binding {
            rules::BindingRole::StagedTile
        } else if binding == self.nodes.workgroup_binding {
            rules::BindingRole::Workgroup
        } else {
            return Ok(());
        };
        let usage = if expr.hir_id == self.nodes.stage_receiver
            && self.lexical == self.nodes.closure
        {
            rules::SourceUse::StageTile
        } else if expr.hir_id == self.nodes.publish_receiver && self.lexical == self.nodes.outer {
            rules::SourceUse::PublishTile
        } else if expr.hir_id == self.nodes.workgroup_receiver && self.lexical == self.nodes.outer {
            rules::SourceUse::PublishWorkgroup
        } else if role == rules::BindingRole::Workgroup && self.lexical == self.nodes.outer {
            let Some(parent) = self.parent else {
                return Err(Error::Source("Workgroup source use escaped its method"));
            };
            let ExprKind::MethodCall(_, receiver, _, _) = parent.kind else {
                return Err(Error::Source(
                    "Workgroup source use is not a closed shared receiver",
                ));
            };
            let [adjustment] = typeck.expr_adjustments(expr) else {
                return Err(Error::Source(
                    "Workgroup receiver lacks its exact shared autoborrow",
                ));
            };
            if receiver.hir_id != expr.hir_id
                || !matches!(
                    adjustment.kind,
                    Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not))
                )
            {
                return Err(Error::Source(
                    "Workgroup receiver is not the exact shared source borrow",
                ));
            }
            let instance = resolve(self.tcx, self.outer, typeck, parent)?;
            auth::shared_workgroup(self.tcx, self.auth, instance, self.work)?;
            let site = raw::find_call(
                self.tcx,
                self.auth.retained,
                self.caller,
                instance,
                parent.span,
                self.work,
            )?
            .0;
            bounded::push(
                &mut self.nodes.borrows,
                BorrowUse {
                    receiver: expr.hir_id,
                    method: parent.hir_id,
                    instance,
                    site,
                },
                self.work,
            )?;
            rules::SourceUse::SharedWorkgroup
        } else {
            rules::SourceUse::Other
        };
        self.uses.observe(role, usage, self.work)
    }
}

impl<'tcx> Visitor<'tcx> for Audit<'_, 'tcx> {
    fn visit_ty(&mut self, _: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) {}
    fn visit_path(&mut self, _: &rustc_hir::Path<'tcx>, _: HirId) {}
    fn visit_pat(&mut self, _: &'tcx Pat<'tcx>) {}
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if self.depth == bounded::MAX_DEPTH {
            self.error = Some(Error::Depth);
            return;
        }
        if let Err(error) = bounded::charge(self.work, 1).and_then(|_| self.path(expr)) {
            self.error = Some(error);
            return;
        }
        self.depth += 1;
        let old_parent = self.parent.replace(expr);
        if let ExprKind::Closure(closure) = expr.kind {
            let old = self.lexical;
            self.lexical = closure.def_id;
            self.visit_body(self.tcx.hir_body(closure.body));
            self.lexical = old;
        } else {
            intravisit::walk_expr(self, expr);
        }
        self.parent = old_parent;
        self.depth -= 1;
    }
}

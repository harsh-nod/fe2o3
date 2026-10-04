//! Live two-body HIR/Instance/ABI join for the checked transport relation only.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    Bf16CallInstanceErrorV1 as Error, Bf16CallInstanceRoleV1 as CheckedRole,
    CheckedBf16CallInstanceV1 as Relation,
};
use rustc_hir::{
    BodyId, Expr, ExprKind, ItemId,
    intravisit::{self, Visitor},
};
use rustc_middle::ty::{EarlyBinder, TypeckResults, TypingEnv};
use std::ops::ControlFlow;
pub(crate) struct SourceOwnedBf16TileValuesRegionV1<'a, 'tcx> {
    seed: &'a AuthenticatedBf16TileValuesSourceSeedV1<'tcx>,
    relation: &'a Relation<'a>,
    source: &'a file::Bf16MfmaSourceFileObservationV1,
    expressions: [&'tcx Expr<'tcx>; ROWS],
}
impl<'a, 'tcx> SourceOwnedBf16TileValuesRegionV1<'a, 'tcx> {
    pub(crate) const fn relation(&self) -> &'a Relation<'a> {
        self.relation
    }
    pub(crate) const fn source(&self) -> &'a file::Bf16MfmaSourceFileObservationV1 {
        self.source
    }
    pub(crate) fn source_span(&self, role: Role) -> Span {
        self.expressions[slot(role)].span
    }
    pub(crate) fn raw_block(&self, role: Role) -> u32 {
        self.seed.pending.payload[0].rows[slot(role)]
            .as_ref()
            .expect("sealed source row")
            .site
            .raw
    }
    pub(crate) fn root_mir_sha256(&self) -> &[u8; 32] {
        let p = &self.seed.pending.payload[0];
        &p.functions[p.root.index() as usize].mir_sha256
    }
    pub(crate) fn helper_mir_sha256(&self) -> &[u8; 32] {
        let p = &self.seed.pending.payload[0];
        &p.functions[p.helper.index() as usize].mir_sha256
    }
    pub(crate) fn helper_source_signature_sha256(&self) -> &[u8; 32] {
        let p = &self.seed.pending.payload[0];
        &p.functions[p.helper.index() as usize].signature_sha256
    }
    pub(crate) fn helper_fn_abi_sha256(&self) -> &[u8; 32] {
        let p = &self.seed.pending.payload[0];
        &p.functions[p.helper.index() as usize].fn_abi_sha256
    }
    pub(crate) fn helper_actual_fn_abi(
        &self,
    ) -> &'tcx rustc_target::callconv::FnAbi<'tcx, ty::Ty<'tcx>> {
        let p = &self.seed.pending.payload[0];
        p.functions[p.helper.index() as usize].fn_abi
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}
struct Scan<'a, 'tcx> {
    pair: &'a Pair<'tcx>,
    function: SemanticFunctionIdV1,
    typeck: &'tcx TypeckResults<'tcx>,
    nodes: usize,
    depth: usize,
    found: [Option<&'tcx Expr<'tcx>>; ROWS],
    helper_item: bool,
}
impl<'tcx> Scan<'_, 'tcx> {
    fn tick(&mut self) -> ControlFlow<&'static str> {
        self.nodes += 1;
        if self.nodes > 4096 {
            ControlFlow::Break("BF16 helper HIR node cap")
        } else {
            ControlFlow::Continue(())
        }
    }
    fn enter(&mut self) -> ControlFlow<&'static str> {
        self.tick()?;
        self.depth += 1;
        if self.depth > 64 {
            ControlFlow::Break("BF16 helper HIR depth cap")
        } else {
            ControlFlow::Continue(())
        }
    }
    fn resolve(&self, raw: ty::Ty<'tcx>) -> Result<Instance<'tcx>> {
        let f = &self.pair.functions[self.function.index() as usize];
        let resolved = f
            .instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                f.tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(raw),
            )
            .map_err(|_| "BF16 helper HIR normalization failed")?;
        let ty::FnDef(def, args) = *resolved.kind() else {
            return Err("BF16 helper HIR callee not direct");
        };
        Instance::try_resolve(f.tcx, TypingEnv::fully_monomorphized(), def, args)
            .map_err(|_| "BF16 helper HIR resolution failed")?
            .ok_or("BF16 helper HIR callee absent")
    }
}
impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    type Result = ControlFlow<&'static str>;
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) -> Self::Result {
        self.enter()?;
        if matches!(expr.kind, ExprKind::Closure(_)) {
            return ControlFlow::Break("BF16 helper HIR closure unavailable");
        }
        let f = &self.pair.functions[self.function.index() as usize];
        let raw = match expr.kind {
            ExprKind::Call(callee, _) => Some(self.typeck.expr_ty(callee)),
            ExprKind::MethodCall(..) => self
                .typeck
                .type_dependent_def_id(expr.hir_id)
                .map(|def| ty::Ty::new_fn_def(f.tcx, def, self.typeck.node_args(expr.hir_id))),
            _ => None,
        };
        if let Some(raw) = raw {
            let instance = match self.resolve(raw) {
                Ok(i) => i,
                Err(e) => return ControlFlow::Break(e),
            };
            for row in self.pair.rows.iter().flatten() {
                if row.function == self.function && row.callee == instance {
                    if expr.span.is_dummy()
                        || expr.span.from_expansion()
                        || expr.span != row.source_span
                        || row.function_span.is_dummy()
                        || row.function_span.from_expansion()
                        || row.function_span.ctxt() != expr.span.ctxt()
                        || row.function_span.lo() < expr.span.lo()
                        || row.function_span.hi() > expr.span.hi()
                        || self.found[slot(row.role)].replace(expr).is_some()
                    {
                        return ControlFlow::Break(
                            "BF16 helper exact HIR/MIR call span repeated or differs",
                        );
                    }
                }
            }
        }
        let result = intravisit::walk_expr(self, expr);
        self.depth -= 1;
        result
    }
    fn visit_pat(&mut self, p: &'tcx rustc_hir::Pat<'tcx>) -> Self::Result {
        self.enter()?;
        let r = intravisit::walk_pat(self, p);
        self.depth -= 1;
        r
    }
    fn visit_block(&mut self, b: &'tcx rustc_hir::Block<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_block(self, b)
    }
    fn visit_stmt(&mut self, s: &'tcx rustc_hir::Stmt<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_stmt(self, s)
    }
    fn visit_ty(&mut self, t: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) -> Self::Result {
        self.enter()?;
        let r = intravisit::walk_ty(self, t);
        self.depth -= 1;
        r
    }
    fn visit_nested_body(&mut self, _: BodyId) -> Self::Result {
        ControlFlow::Break("BF16 helper nested body unavailable")
    }
    fn visit_nested_item(&mut self, id: ItemId) -> Self::Result {
        self.tick()?;
        let root = &self.pair.functions[self.pair.root.index() as usize];
        let helper = &self.pair.functions[self.pair.helper.index() as usize];
        let item = root.tcx.hir_item(id);
        if self.function != self.pair.root
            || id.owner_id.def_id.to_def_id() != helper.instance.def_id()
            || item.span.is_dummy()
            || item.span.from_expansion()
            || self.helper_item
        {
            return ControlFlow::Break(
                "BF16 helper nested item is not the one actual selected function",
            );
        }
        self.helper_item = true;
        ControlFlow::Continue(()) // Its ACTUAL body is separately visited below.
    }
}
fn checked_role(role: Role) -> Option<CheckedRole> {
    Some(match role {
        Role::Context => CheckedRole::Context,
        Role::Lane => CheckedRole::Lane,
        Role::Lhs => CheckedRole::Lhs,
        Role::Rhs => CheckedRole::Rhs,
        Role::Zero => CheckedRole::Zero,
        Role::Matrix => CheckedRole::Result,
        Role::Values => CheckedRole::Values,
        Role::HelperCall => return None,
    })
}
pub(super) const fn scratch_bytes() -> usize {
    std::mem::size_of::<Scan<'_, '_>>()
        + std::mem::size_of::<SourceOwnedBf16TileValuesRegionV1<'_, '_>>()
        + 2 * std::mem::size_of::<[Option<&Expr<'_>>; ROWS]>()
        + file::scratch_bytes()
}
pub(super) const fn work() -> usize {
    2 * 4096 * 24 + file::work() + 8192
}
impl<'tcx> AuthenticatedBf16TileValuesSourceSeedV1<'tcx> {
    pub(crate) fn phase_storage_bytes(&self) -> usize {
        self.retained_storage_bytes() + scratch_bytes()
    }
    pub(crate) fn with_relation<'work, R: Copy + 'static>(
        &self,
        relation: &Relation<'_>,
        budget: &mut Budget<'work>,
        inspect: impl for<'a> FnOnce(
            &SourceOwnedBf16TileValuesRegionV1<'a, 'tcx>,
            &mut Budget<'work>,
        ) -> std::result::Result<R, Error>,
    ) -> std::result::Result<R, Error> {
        budget.charge_work(work())?;
        if budget.storage() < self.phase_storage_bytes() {
            return Err(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting.into(),
            );
        }
        let p = &self.pending.payload[0];
        let semantic = relation.owner().source_semantic();
        if semantic.semantic_sha256().as_bytes() != self.semantic_sha256()
            || semantic.functions().len() != 2
            || semantic.roots() != [p.root]
            || relation.root() != p.root
            || relation.helper() != p.helper
        {
            return Err(Error::Unavailable(
                "BF16 helper actual semantic owner differs",
            ));
        }
        for (index, raw) in p.functions.iter().enumerate() {
            let f = &semantic.functions()[index];
            let local = raw
                .instance
                .def_id()
                .as_local()
                .ok_or(Error::Unavailable("BF16 helper function not local"))?;
            if !same_function(raw.identities, f)
                || raw.abi != f.abi().identity()
                || canonical_function_identities_v1(raw.tcx, raw.instance) != raw.identities
                || raw
                    .tcx
                    .hir_maybe_body_owned_by(local)
                    .is_none_or(|b| !std::ptr::eq(b, raw.hir))
                || !std::ptr::eq(raw.tcx.instance_mir(raw.instance.def), raw.mir)
                || raw.source_inputs.iter().any(Option::is_none)
                || raw.fn_abi.args.len() > 16
            {
                return Err(Error::Unavailable(
                    "BF16 helper live Instance/FnABI/body custody differs",
                ));
            }
            if (index == p.root.index() as usize && !raw.source_output.is_unit())
                || (index == p.helper.index() as usize
                    && !matches!(raw.source_output.kind(), ty::Array(..)))
            {
                return Err(Error::Unavailable(
                    "BF16 helper actual source return type differs",
                ));
            }
        }
        for row in p.rows.iter().flatten() {
            let f = &semantic.functions()[row.function.index() as usize];
            let block = f
                .blocks()
                .get(row.site.semantic.index() as usize)
                .ok_or(Error::Unavailable("BF16 helper source block disappeared"))?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return Err(Error::Unavailable("BF16 helper source terminator changed"));
            };
            if block.identity() != row.site.identity
                || call.callee() != row.callable
                || row.consumed != Some(Transport::capture(call).map_err(Error::Unavailable)?)
            {
                return Err(Error::Unavailable(
                    "BF16 helper imported source transport differs",
                ));
            }
            if let Some(role) = checked_role(row.role) {
                let producer = relation.producer(role);
                if producer.function() != row.function || producer.block() != row.site.semantic {
                    return Err(Error::Unavailable(
                        "BF16 helper checked SSA producer source differs",
                    ));
                }
            } else if relation.call_block() != row.site.semantic
                || !std::ptr::eq(relation.source_call(), call)
            {
                return Err(Error::Unavailable(
                    "BF16 helper checked Defined occurrence differs",
                ));
            }
        }
        let root = &p.functions[p.root.index() as usize];
        let helper = &p.functions[p.helper.index() as usize];
        let mut source =
            file::Bf16MfmaSourceFileObservationV1::capture(root).map_err(Error::Unavailable)?;
        if !source.contains(root.hir.value.span) || !source.contains(helper.hir.value.span) {
            return Err(Error::Unavailable(
                "BF16 helper root and local helper must share actual source file",
            ));
        }
        let mut found = [None; ROWS];
        for function in [p.root, p.helper] {
            let raw = &p.functions[function.index() as usize];
            let local = raw
                .instance
                .def_id()
                .as_local()
                .ok_or(Error::Unavailable("BF16 helper HIR definition not local"))?;
            let mut scan = Scan {
                pair: p,
                function,
                typeck: raw.tcx.typeck(local),
                nodes: 0,
                depth: 0,
                found: [None; ROWS],
                helper_item: false,
            };
            if let ControlFlow::Break(e) = scan.visit_body(raw.hir) {
                return Err(Error::Unavailable(e));
            }
            if (function == p.root) != scan.helper_item {
                return Err(Error::Unavailable(
                    "BF16 helper exact body-local definition absent or repeated",
                ));
            }
            for (index, row) in scan.found.into_iter().enumerate() {
                if let Some(row) = row {
                    if found[index].replace(row).is_some() {
                        return Err(Error::Unavailable("BF16 helper source expression repeated"));
                    }
                }
            }
        }
        let mut expressions = [root.hir.value; ROWS];
        for (to, from) in expressions.iter_mut().zip(found) {
            *to = from.ok_or(Error::Unavailable("BF16 helper actual HIR role missing"))?;
            if !source.contains(to.span) {
                return Err(Error::Unavailable("BF16 helper actual HIR file differs"));
            }
        }
        let ExprKind::MethodCall(_, receiver, _, _) = expressions[slot(Role::Values)].kind else {
            return Err(Error::Unavailable(
                "BF16 helper conversion is not the direct method expression",
            ));
        };
        if !std::ptr::eq(receiver, expressions[slot(Role::Matrix)]) {
            return Err(Error::Unavailable(
                "BF16 helper MFMA and into_values are not the exact nested source region",
            ));
        }
        let ledger = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let result = inspect(
            &SourceOwnedBf16TileValuesRegionV1 {
                seed: self,
                relation,
                source: &source,
                expressions,
            },
            budget,
        );
        if budget.work_ledger_identity_v1() != ledger
            || budget.storage() < floor
            || (result.is_ok()
                && (budget.failed_work().is_some() || budget.failed_storage().is_some()))
        {
            return Err(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting.into(),
            );
        }
        source.recheck().map_err(Error::Unavailable)?;
        result
    }
}

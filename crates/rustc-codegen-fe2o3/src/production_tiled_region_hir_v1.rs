//! Same live Instance/HIR/MIR/imported-call/emission join. Not a source publisher.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionBf16MfmaEmissionViewV1 as Emission, ProductionTiledRegionInspectionErrorV1 as Error,
};
use rustc_hir::{
    BodyId, Expr, ExprKind, ItemId,
    intravisit::{self, Visitor},
};
use rustc_middle::ty::{self, EarlyBinder, TypeckResults, TypingEnv};
use std::ops::ControlFlow;

pub(crate) struct SourceOwnedBf16MfmaRegionV1<'a, 'tcx> {
    seed: &'a AuthenticatedBf16MfmaSourceSeedV1<'tcx>,
    emission: &'a Emission<'a>,
    source: &'a Bf16MfmaSourceFileObservationV1,
    expressions: [&'tcx Expr<'tcx>; 6],
}
impl<'a, 'tcx> SourceOwnedBf16MfmaRegionV1<'a, 'tcx> {
    pub(crate) const fn emission(&self) -> &'a Emission<'a> {
        self.emission
    }
    pub(crate) const fn source(&self) -> &'a Bf16MfmaSourceFileObservationV1 {
        self.source
    }
    pub(crate) fn source_span(&self, role: Role) -> Span {
        self.expressions[role_index(role)].span
    }
    pub(crate) fn raw_block(&self, role: Role) -> u32 {
        self.seed.captured.payload[0].rows[role_index(role)]
            .as_ref()
            .expect("sealed source role")
            .site
            .raw
    }
    pub(crate) fn mir_sha256(&self) -> &[u8; 32] {
        &self.seed.captured.payload[0].mir_sha256
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

struct Scan<'a, 'tcx> {
    captured: &'a Captured<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    nodes: usize,
    depth: usize,
    found: [Option<&'tcx Expr<'tcx>>; 6],
}
impl<'tcx> Scan<'_, 'tcx> {
    fn tick(&mut self) -> ControlFlow<&'static str> {
        self.nodes += 1;
        if self.nodes > 4096 {
            ControlFlow::Break("BF16 HIR node bound exceeded")
        } else {
            ControlFlow::Continue(())
        }
    }
    fn enter(&mut self) -> ControlFlow<&'static str> {
        self.tick()?;
        self.depth += 1;
        if self.depth > 64 {
            ControlFlow::Break("BF16 HIR nesting bound exceeded")
        } else {
            ControlFlow::Continue(())
        }
    }
    fn resolve(&self, raw: ty::Ty<'tcx>) -> Result<Instance<'tcx>> {
        let tcx = self.captured.tcx;
        let resolved = self
            .captured
            .instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(raw),
            )
            .map_err(|_| "BF16 HIR callee normalization failed")?;
        let ty::FnDef(def, args) = *resolved.kind() else {
            return Err("BF16 HIR callee is not a direct function");
        };
        Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def, args)
            .map_err(|_| "BF16 HIR callee resolution failed")?
            .ok_or("BF16 HIR callee is absent")
    }
}
impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    type Result = ControlFlow<&'static str>;
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) -> Self::Result {
        self.enter()?;
        if matches!(expr.kind, ExprKind::Closure(_)) {
            return ControlFlow::Break("BF16 HIR closure is unavailable");
        }
        let raw = match expr.kind {
            ExprKind::Call(callee, _) => Some(self.typeck.expr_ty(callee)),
            ExprKind::MethodCall(..) => self.typeck.type_dependent_def_id(expr.hir_id).map(|def| {
                ty::Ty::new_fn_def(self.captured.tcx, def, self.typeck.node_args(expr.hir_id))
            }),
            _ => None,
        };
        if let Some(raw) = raw {
            let instance = match self.resolve(raw) {
                Ok(v) => v,
                Err(e) => return ControlFlow::Break(e),
            };
            for row in self.captured.rows.iter().flatten() {
                if row.callee == instance {
                    if expr.span.is_dummy()
                        || expr.span.from_expansion()
                        || expr.span != row.source_span
                        || row.function_span.is_dummy()
                        || row.function_span.from_expansion()
                        || row.function_span.ctxt() != expr.span.ctxt()
                        || row.function_span.lo() < expr.span.lo()
                        || row.function_span.hi() > expr.span.hi()
                        || self.found[role_index(row.role)].replace(expr).is_some()
                    {
                        return ControlFlow::Break(
                            "BF16 actual HIR and MIR call spans differ or repeat",
                        );
                    }
                }
            }
        }
        let result = intravisit::walk_expr(self, expr);
        self.depth -= 1;
        result
    }
    fn visit_pat(&mut self, pat: &'tcx rustc_hir::Pat<'tcx>) -> Self::Result {
        self.enter()?;
        let r = intravisit::walk_pat(self, pat);
        self.depth -= 1;
        r
    }
    fn visit_block(&mut self, block: &'tcx rustc_hir::Block<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_block(self, block)
    }
    fn visit_stmt(&mut self, stmt: &'tcx rustc_hir::Stmt<'tcx>) -> Self::Result {
        self.tick()?;
        intravisit::walk_stmt(self, stmt)
    }
    fn visit_ty(&mut self, ty: &'tcx rustc_hir::Ty<'tcx, rustc_hir::AmbigArg>) -> Self::Result {
        self.enter()?;
        let r = intravisit::walk_ty(self, ty);
        self.depth -= 1;
        r
    }
    fn visit_nested_body(&mut self, _: BodyId) -> Self::Result {
        ControlFlow::Break("BF16 HIR nested body is unavailable")
    }
    fn visit_nested_item(&mut self, _: ItemId) -> Self::Result {
        ControlFlow::Break("BF16 HIR local item is unavailable")
    }
}
pub(super) const fn scratch_bytes() -> usize {
    std::mem::size_of::<Scan<'_, '_>>()
        + std::mem::size_of::<SourceOwnedBf16MfmaRegionV1<'_, '_>>()
        + std::mem::size_of::<[&Expr<'_>; 6]>()
        + file::scratch_bytes()
}
pub(super) const fn work() -> usize {
    4096 * 16 + file::work() + 4096
}

impl<'tcx> AuthenticatedBf16MfmaSourceSeedV1<'tcx> {
    // Scratch is prepaid BEFORE materialization and belongs to the lowerer's
    // protected callback floor. No resetting scope releases callback extras.
    pub(crate) fn phase_storage_bytes(&self) -> usize {
        self.retained_storage_bytes() + scratch_bytes()
    }
    pub(crate) fn with_region<'work, R>(
        &self,
        emission: &Emission<'_>,
        budget: &mut Budget<'work>,
        inspect: impl for<'a> FnOnce(
            &SourceOwnedBf16MfmaRegionV1<'a, 'tcx>,
            &mut Budget<'work>,
        ) -> std::result::Result<R, Error>,
    ) -> std::result::Result<R, Error> {
        let captured = &self.captured.payload[0];
        budget.charge_work(work())?;
        if budget.storage() < self.phase_storage_bytes() {
            return Err(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting.into(),
            );
        }
        let owner = emission.original();
        let function = emission.source_function();
        if emission.semantic_function().index() != 0
            || !same_function(captured.identities, function)
            || function.abi().identity() != captured.abi
            || owner
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes()
                != self.semantic_sha256()
            || canonical_function_identities_v1(captured.tcx, captured.instance)
                != captured.identities
            || captured
                .tcx
                .hir_maybe_body_owned_by(
                    captured
                        .instance
                        .def_id()
                        .as_local()
                        .ok_or(Error::Unavailable("BF16 root no longer local"))?,
                )
                .is_none_or(|body| !std::ptr::eq(body, captured.hir))
        {
            return Err(Error::Unavailable(
                "BF16 live source and actual SSA owner differ",
            ));
        }
        let [launch] = owner.source_launch().roots() else {
            return Err(Error::Unavailable(
                "BF16 actual source launch roster differs",
            ));
        };
        if launch.source_launch().exact_workgroup() != Some([64, 1, 1])
            || launch.source_launch().max_grid() != [1, 1, 1]
        {
            return Err(Error::Unavailable("BF16 original source launch differs"));
        }
        for role in ROLES {
            let row = captured.rows[role_index(role)]
                .as_ref()
                .ok_or(Error::Unavailable("BF16 source role missing"))?;
            if emission.producer_block(role) != row.site.semantic {
                return Err(Error::Unavailable(
                    "BF16 actual producer source block differs",
                ));
            }
            let block = function
                .blocks()
                .get(row.site.semantic.index() as usize)
                .ok_or(Error::Unavailable("BF16 producer block missing"))?;
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return Err(Error::Unavailable("BF16 producer is not an actual call"));
            };
            if block.identity() != row.site.identity
                || call.callee() != row.callable
                || row.consumed != Some(CallTransport::capture(call).map_err(Error::Unavailable)?)
            {
                return Err(Error::Unavailable(
                    "BF16 imported producer transport differs",
                ));
            }
            for mapped in row.raw_arguments.iter().flatten() {
                if captured
                    .locals
                    .iter()
                    .flatten()
                    .all(|retained| retained != mapped)
                {
                    return Err(Error::Unavailable("BF16 raw argument map missing"));
                }
            }
        }
        let mut source =
            Bf16MfmaSourceFileObservationV1::capture(captured).map_err(Error::Unavailable)?;
        if !source.contains(captured.hir.value.span) {
            return Err(Error::Unavailable("BF16 HIR body is outside actual source"));
        }
        let local = captured
            .instance
            .def_id()
            .as_local()
            .ok_or(Error::Unavailable("BF16 root is not local"))?;
        let mut scan = Scan {
            captured,
            typeck: captured.tcx.typeck(local),
            nodes: 0,
            depth: 0,
            found: [None; 6],
        };
        if let ControlFlow::Break(error) = scan.visit_body(captured.hir) {
            return Err(Error::Unavailable(error));
        }
        let mut expressions = [captured.hir.value; 6];
        for (slot, found) in expressions.iter_mut().zip(scan.found) {
            *slot = found.ok_or(Error::Unavailable("BF16 exact HIR producer missing"))?;
            if !source.contains(slot.span) {
                return Err(Error::Unavailable("BF16 HIR producer file differs"));
            }
        }
        let ledger = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let result = inspect(
            &SourceOwnedBf16MfmaRegionV1 {
                seed: self,
                emission,
                source: &source,
                expressions,
            },
            budget,
        );
        // Do not continue the source observation using a replaced meter or an
        // undercut frame. The enclosing lowerer owns refusal/panic cleanup.
        if budget.work_ledger_identity_v1() != ledger
            || budget.storage() < floor
            || (result.is_ok()
                && (budget.failed_work().is_some() || budget.failed_storage().is_some()))
        {
            drop(result);
            return Err(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting.into(),
            );
        }
        source.recheck().map_err(Error::Unavailable)?;
        result
    }
}

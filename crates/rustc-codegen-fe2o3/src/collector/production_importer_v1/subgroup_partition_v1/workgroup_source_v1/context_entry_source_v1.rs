//! Source-flow evidence for an erased, compiler-issued Context entry argument.
//! The private Context custody carries this evidence onward; it does not issue authority.

use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{Expr, ExprKind, HirId, PatKind, Stmt, StmtKind, def::Res, def_id::DefId};
use rustc_middle::mir::{self, BasicBlock, Local, Operand, StatementKind, TerminatorKind};
use rustc_middle::ty::{self, EarlyBinder, Instance, Ty, TyCtxt, TypeckResults, TypingEnv};
use std::collections::{HashMap, VecDeque};

#[cfg(test)]
pub(in crate::collector::production_importer_v1) mod tests;

type Result<T> = std::result::Result<T, &'static str>;

/// The caller must separately authenticate the root registration, nominal
/// Context type, helper ABI, and trusted issuer. This receipt proves only the
/// exact source edge between those supplied rustc instances.
pub(crate) struct ContextEntrySourceV1<'tcx> {
    root: Instance<'tcx>,
    helper: Instance<'tcx>,
    issuer: Instance<'tcx>,
    context: Ty<'tcx>,
    issuer_block: BasicBlock,
    issuer_local: Local,
    helper_block: BasicBlock,
    argument: usize,
    source_binding: HirId,
    source_use: HirId,
    boundary_uses: Vec<BoundaryBorrowV1>,
}

impl<'tcx> ContextEntrySourceV1<'tcx> {
    pub(crate) fn source_nodes(&self) -> (u32, u32) {
        (
            self.source_binding.local_id.as_u32(),
            self.source_use.local_id.as_u32(),
        )
    }
    pub(crate) fn coordinates(&self) -> (BasicBlock, Local, BasicBlock, usize) {
        (
            self.issuer_block,
            self.issuer_local,
            self.helper_block,
            self.argument,
        )
    }

    pub(crate) fn replay(&self, tcx: TyCtxt<'tcx>, work: &mut usize) -> Result<()> {
        let replay = Self::check(
            tcx,
            self.root,
            self.helper,
            self.issuer,
            self.context,
            self.argument,
            work,
        )?;
        if replay.coordinates() != self.coordinates()
            || replay.source_binding != self.source_binding
            || replay.source_use != self.source_use
            || replay.boundary_uses != self.boundary_uses
        {
            return Err("Context entry source receipt changed on replay");
        }
        Ok(())
    }

    pub(crate) fn check(
        tcx: TyCtxt<'tcx>,
        root: Instance<'tcx>,
        helper: Instance<'tcx>,
        issuer: Instance<'tcx>,
        context: Ty<'tcx>,
        argument: usize,
        work: &mut usize,
    ) -> Result<Self> {
        let root_id = root
            .def_id()
            .as_local()
            .ok_or("Context source root has no local HIR")?;
        if root.def_id() == helper.def_id()
            || root.def_id() == issuer.def_id()
            || helper.def_id() == issuer.def_id()
        {
            return Err("Context source roles overlap");
        }
        let typeck = tcx.typeck(root_id);
        let mut source = SourceEdge {
            tcx,
            typeck,
            helper: helper.def_id(),
            issuer: issuer.def_id(),
            argument,
            bindings: HashMap::new(),
            local_uses: HashMap::new(),
            uses: Vec::new(),
            boundary_uses: Vec::new(),
            issue_count: 0,
            malformed: false,
            exhausted: false,
            work,
        };
        source.visit_expr(tcx.hir_body_owned_by(root_id).value);
        if source.exhausted {
            return Err("Context entry source work limit");
        }
        if source.malformed || source.issue_count != 1 || source.uses.len() != 1 {
            return Err("Context entry source is not one exact initializer-to-argument edge");
        }
        let (source_binding, source_use) = source.uses[0];
        if source.bindings.get(&source_binding) != Some(&issuer.def_id()) {
            return Err("Context helper argument does not name the trusted initializer binding");
        }
        let boundary_uses = source.checked_uses(source_binding, source_use)?;

        let body = tcx.instance_mir(root.def);
        let normalize = |ty| {
            root.try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(ty),
            )
            .map_err(|_| "Context entry type normalization failed")
        };
        let mut issue_site = None;
        let mut helper_site = None;
        for (block, data) in body.basic_blocks.iter_enumerated() {
            charge(work)?;
            let TerminatorKind::Call {
                func,
                args,
                destination,
                target,
                ..
            } = &data.terminator().kind
            else {
                continue;
            };
            let Some(callee) = resolve_call(tcx, root, func)? else {
                continue;
            };
            if callee == issuer {
                if data.is_cleanup
                    || !args.is_empty()
                    || !destination.projection.is_empty()
                    || normalize(body.local_decls[destination.local].ty)? != context
                    || target.is_none()
                    || issue_site.replace((block, destination.local)).is_some()
                {
                    return Err("Context issuer MIR occurrence is missing or ambiguous");
                }
            }
            if callee == helper {
                let Some(value) = args.get(argument) else {
                    return Err("Context helper MIR argument is missing");
                };
                let Operand::Constant(constant) = &value.node else {
                    return Err("Context helper argument is not an erased constant");
                };
                let constant_ty = normalize(constant.const_.ty())?;
                if data.is_cleanup
                    || target.is_none()
                    || constant_ty != context
                    || constant
                        .const_
                        .eval(tcx, TypingEnv::fully_monomorphized(), constant.span)
                        .map_err(|_| "Context entry constant evaluation failed")?
                        != mir::ConstValue::ZeroSized
                    || helper_site.replace(block).is_some()
                {
                    return Err("Context helper MIR occurrence or erased argument changed");
                }
            }
        }
        let (issuer_block, issuer_local) =
            issue_site.ok_or("Context issuer MIR occurrence is missing")?;
        let helper_block = helper_site.ok_or("Context helper MIR occurrence is missing")?;
        issuer_live_at_helper(body, issuer_block, issuer_local, helper_block, work)?;
        Ok(Self {
            root,
            helper,
            issuer,
            context,
            issuer_block,
            issuer_local,
            helper_block,
            argument,
            source_binding,
            source_use,
            boundary_uses,
        })
    }
}

fn charge(work: &mut usize) -> Result<()> {
    *work = work
        .checked_sub(1)
        .ok_or("Context entry source work limit")?;
    Ok(())
}

fn direct_function(typeck: &TypeckResults<'_>, expression: &Expr<'_>) -> Option<DefId> {
    match typeck.expr_ty(expression).kind() {
        ty::FnDef(definition, _) => Some(*definition),
        _ => None,
    }
}

fn initializer<'hir>(mut expression: &'hir Expr<'hir>) -> &'hir Expr<'hir> {
    loop {
        match expression.kind {
            ExprKind::DropTemps(inner) => expression = inner,
            ExprKind::Block(block, _) if block.stmts.is_empty() => {
                if let Some(inner) = block.expr {
                    expression = inner;
                } else {
                    return expression;
                }
            }
            _ => return expression,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BoundaryBorrowV1 {
    binding: HirId,
    use_site: HirId,
    call: HirId,
    callee: DefId,
}

struct SourceEdge<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    helper: DefId,
    issuer: DefId,
    argument: usize,
    bindings: HashMap<HirId, DefId>,
    local_uses: HashMap<HirId, Vec<HirId>>,
    uses: Vec<(HirId, HirId)>,
    boundary_uses: Vec<BoundaryBorrowV1>,
    issue_count: usize,
    malformed: bool,
    exhausted: bool,
    work: &'a mut usize,
}

impl SourceEdge<'_, '_> {
    fn checked_uses(&mut self, binding: HirId, use_site: HirId) -> Result<Vec<BoundaryBorrowV1>> {
        let uses = self
            .local_uses
            .get(&binding)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        for _ in 0..uses.len() + self.boundary_uses.len() {
            charge(self.work)?;
        }
        let boundaries = self
            .boundary_uses
            .iter()
            .copied()
            .filter(|borrow| borrow.binding == binding)
            .collect::<Vec<_>>();
        if !exact_source_uses(uses, use_site, &boundaries) {
            #[cfg(test)]
            eprintln!(
                "Context entry rejected HIR use roster: observed={} accepted_boundary_borrows={} expected_transfers=1",
                uses.len(),
                boundaries.len(),
            );
            return Err("Context source binding has another use, borrow, or assignment");
        }
        Ok(boundaries)
    }
}

fn exact_source_uses(uses: &[HirId], transfer: HirId, boundaries: &[BoundaryBorrowV1]) -> bool {
    let mut remaining = uses
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    uses.len() == remaining.len()
        && remaining.remove(&transfer)
        && boundaries
            .iter()
            .all(|borrow| remaining.remove(&borrow.use_site))
        && remaining.is_empty()
}

fn is_context_boundary_binder(
    item: Option<crate::trusted_device_items::TrustedDeviceItem>,
) -> bool {
    use crate::trusted_device_items::TrustedDeviceItem as Item;
    matches!(
        item,
        Some(
            Item::CapabilityGlobalBindReadOnly
                | Item::CapabilityGlobalBindDisjointWrite
                | Item::CapabilityGlobalBindExclusiveReadWrite
                | Item::CapabilityGlobalBindAtomic
        )
    )
}

fn identity_shared_receiver_adjustments<'tcx>(
    tcx: TyCtxt<'tcx>,
    owner: Ty<'tcx>,
    raw: Ty<'tcx>,
    adjusted: Ty<'tcx>,
    adjustments: &[ty::adjustment::Adjustment<'tcx>],
) -> bool {
    use ty::adjustment::{Adjust, AutoBorrow, AutoBorrowMutability, DerefAdjustKind};
    let erase = |ty| tcx.erase_and_anonymize_regions(ty);
    let ty::Ref(_, pointee, rustc_hir::Mutability::Not) = *raw.kind() else {
        return false;
    };
    if erase(pointee) != erase(owner) || erase(adjusted) != erase(raw) {
        return false;
    }
    match adjustments {
        [] => true,
        [deref, borrow] => {
            matches!(deref.kind, Adjust::Deref(DerefAdjustKind::Builtin))
                && matches!(
                    borrow.kind,
                    Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not))
                )
                && erase(deref.target) == erase(owner)
                && erase(borrow.target) == erase(raw)
        }
        _ => false,
    }
}

fn boundary_borrow<'tcx>(
    tcx: TyCtxt<'tcx>,
    typeck: &TypeckResults<'tcx>,
    call: &'tcx Expr<'tcx>,
) -> Option<BoundaryBorrowV1> {
    let ExprKind::Call(callee, [borrow, _physical]) = call.kind else {
        return None;
    };
    let ty::FnDef(definition, args) = *typeck.expr_ty(callee).kind() else {
        return None;
    };
    if !is_context_boundary_binder(crate::trusted_device_items::classify(tcx, definition)) {
        return None;
    }
    // Only the temporary &owner passed directly to a reviewed boundary binder
    // is allowed. Stored references, method autoref, raw and mutable borrows do
    // not establish this short-lived, non-escaping source use.
    let ExprKind::AddrOf(rustc_hir::BorrowKind::Ref, rustc_hir::Mutability::Not, owner) =
        borrow.kind
    else {
        return None;
    };
    let ExprKind::Path(ref path) = owner.kind else {
        return None;
    };
    let Res::Local(binding) = typeck.qpath_res(path, owner.hir_id) else {
        return None;
    };
    if !typeck.expr_adjustments(owner).is_empty()
        || !identity_shared_receiver_adjustments(
            tcx,
            typeck.expr_ty(owner),
            typeck.expr_ty(borrow),
            typeck.expr_ty_adjusted(borrow),
            typeck.expr_adjustments(borrow),
        )
    {
        return None;
    }
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(definition).instantiate(tcx, args));
    let [receiver, _] = signature.inputs() else {
        return None;
    };
    let ty::Ref(_, pointee, rustc_hir::Mutability::Not) = *receiver.kind() else {
        return None;
    };
    if tcx.erase_and_anonymize_regions(pointee)
        != tcx.erase_and_anonymize_regions(typeck.expr_ty(owner))
        || tcx.erase_and_anonymize_regions(*receiver)
            != tcx.erase_and_anonymize_regions(typeck.expr_ty(borrow))
    {
        return None;
    }
    Some(BoundaryBorrowV1 {
        binding,
        use_site: owner.hir_id,
        call: call.hir_id,
        callee: definition,
    })
}

impl<'tcx> Visitor<'tcx> for SourceEdge<'_, 'tcx> {
    fn visit_stmt(&mut self, statement: &'tcx Stmt<'tcx>) {
        if charge(self.work).is_err() {
            self.exhausted = true;
            return;
        }
        if let StmtKind::Let(local) = statement.kind
            && let PatKind::Binding(_, id, _, None) = local.pat.kind
            && let Some(value) = local.init
            && let ExprKind::Call(callee, []) = initializer(value).kind
            && direct_function(self.typeck, callee) == Some(self.issuer)
            && self.bindings.insert(id, self.issuer).is_some()
        {
            self.malformed = true;
        }
        intravisit::walk_stmt(self, statement);
    }

    fn visit_expr(&mut self, expression: &'tcx Expr<'tcx>) {
        if charge(self.work).is_err() {
            self.exhausted = true;
            return;
        }
        if let ExprKind::Path(ref path) = expression.kind
            && let Res::Local(binding) = self.typeck.qpath_res(path, expression.hir_id)
        {
            self.local_uses
                .entry(binding)
                .or_default()
                .push(expression.hir_id);
        }
        if let ExprKind::Call(callee, arguments) = expression.kind {
            if direct_function(self.typeck, callee) == Some(self.issuer) {
                self.issue_count += 1;
            }
            if direct_function(self.typeck, callee) == Some(self.helper) {
                let binding = arguments.get(self.argument).and_then(|argument| {
                    let ExprKind::Path(ref path) = argument.kind else {
                        return None;
                    };
                    match self.typeck.qpath_res(path, argument.hir_id) {
                        Res::Local(binding) => Some((binding, argument.hir_id)),
                        _ => None,
                    }
                });
                if let Some(binding) = binding {
                    self.uses.push(binding);
                } else {
                    self.malformed = true;
                }
            }
        }
        // Record a completed boundary call only before the helper's by-value
        // argument is encountered. Its explicit temporary borrow cannot cross
        // that move; the original MIR liveness check still runs independently.
        let before_transfer = self.uses.is_empty();
        if matches!(expression.kind, ExprKind::Closure(_)) {
            // Captures live in another HIR owner. This entry proof has no
            // capture relation and must not silently omit their Context uses.
            self.malformed = true;
        } else {
            intravisit::walk_expr(self, expression);
        }
        if before_transfer
            && self.uses.is_empty()
            && let Some(borrow) = boundary_borrow(self.tcx, self.typeck, expression)
            && self.bindings.get(&borrow.binding) == Some(&self.issuer)
        {
            self.boundary_uses.push(borrow);
        }
    }
}

fn resolve_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    root: Instance<'tcx>,
    operand: &Operand<'tcx>,
) -> Result<Option<Instance<'tcx>>> {
    let Operand::Constant(callee) = operand else {
        return Ok(None);
    };
    let ty = root
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(callee.const_.ty()),
        )
        .map_err(|_| "Context call normalization failed")?;
    let ty::FnDef(definition, args) = *ty.kind() else {
        return Ok(None);
    };
    Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), definition, args)
        .map_err(|_| "Context call resolution failed")
}

fn issuer_live_at_helper(
    body: &mir::Body<'_>,
    issuer: BasicBlock,
    local: Local,
    helper: BasicBlock,
    work: &mut usize,
) -> Result<()> {
    use mir::visit::{PlaceContext, Visitor as MirVisitor};
    struct Kills {
        local: Local,
        killed: bool,
    }
    impl<'tcx> MirVisitor<'tcx> for Kills {
        fn visit_local(&mut self, local: Local, context: PlaceContext, _: mir::Location) {
            if local == self.local && (context.is_mutating_use() || context.is_storage_marker()) {
                self.killed = true;
            }
        }
        fn visit_operand(&mut self, operand: &Operand<'tcx>, location: mir::Location) {
            if matches!(operand, Operand::Move(place) if place.local == self.local) {
                self.killed = true;
            }
            self.super_operand(operand, location);
        }
    }
    let mut incoming = vec![None; body.basic_blocks.len()];
    incoming[mir::START_BLOCK.index()] = Some(false);
    let mut queue = VecDeque::from([mir::START_BLOCK]);
    let mut live_helper = None;
    while let Some(block) = queue.pop_front() {
        charge(work)?;
        let data = &body.basic_blocks[block];
        let mut live = incoming[block.index()].unwrap();
        for (statement_index, statement) in data.statements.iter().enumerate() {
            charge(work)?;
            let mut kills = Kills {
                local,
                killed: false,
            };
            kills.visit_statement(
                statement,
                mir::Location {
                    block,
                    statement_index,
                },
            );
            if matches!(statement.kind, StatementKind::StorageLive(l) | StatementKind::StorageDead(l) if l == local)
            {
                kills.killed = true;
            }
            live &= !kills.killed;
        }
        if block == helper {
            live_helper = Some(live);
        }
        let mut kills = Kills {
            local,
            killed: false,
        };
        kills.visit_terminator(
            data.terminator(),
            mir::Location {
                block,
                statement_index: data.statements.len(),
            },
        );
        live &= !kills.killed;
        for target in data.terminator().successors() {
            charge(work)?;
            if target == issuer {
                return Err("Context issuer is cyclic or not the entry occurrence");
            }
            let edge_live = if block == issuer {
                matches!(data.terminator().kind, TerminatorKind::Call { target: Some(normal), .. } if normal == target)
            } else {
                live
            };
            let merged = incoming[target.index()].map_or(edge_live, |old| old && edge_live);
            if incoming[target.index()] != Some(merged) {
                incoming[target.index()] = Some(merged);
                queue.push_back(target);
            }
        }
    }
    if live_helper != Some(true) {
        return Err("Context issuer does not remain live on every helper-entry path");
    }
    Ok(())
}

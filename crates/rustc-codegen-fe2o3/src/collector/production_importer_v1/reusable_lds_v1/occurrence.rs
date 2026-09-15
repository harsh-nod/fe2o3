//! Live typed nested-receiver observation. No source names, debug bindings, or
//! same-shaped constants establish the allocation edge. Definition and root
//! authentication are separate mandatory checks in the consuming importer.
use super::body::{self, Types};
use rustc_hir::{
    Expr, ExprKind, HirId,
    def_id::LocalDefId,
    intravisit::{self, Visitor},
};
use rustc_middle::{
    mir::{self, BasicBlock, Body, Local, Operand, TerminatorKind, UnwindAction},
    ty::{self, Instance, TyCtxt, TypeckResults, TypingEnv},
};
use rustc_span::Span;

type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SourceNodes {
    pub lexical_body: LocalDefId,
    pub conversion: HirId,
    pub allocation: HirId,
    pub workgroup: HirId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Coordinates {
    pub allocation_block: BasicBlock,
    pub allocation_local: Local,
    pub conversion_block: BasicBlock,
    pub conversion_local: Local,
    pub return_block: BasicBlock,
    pub erased: bool,
}

/// Non-Clone, private construction. Its original caller is a concrete lexical
/// Fn/closure instance, which need not equal the enclosing HIR owner.
pub(super) struct NestedReceiver<'tcx> {
    caller: Instance<'tcx>,
    conversion: Instance<'tcx>,
    allocation: Instance<'tcx>,
    types: Types<'tcx>,
    nodes: SourceNodes,
    coordinates: Coordinates,
}

fn charge(work: &mut usize, units: usize) -> Result<()> {
    *work = work
        .checked_sub(units)
        .ok_or("reusable LDS source-work ceiling")?;
    Ok(())
}

impl<'tcx> NestedReceiver<'tcx> {
    pub(super) fn nodes(&self) -> SourceNodes {
        self.nodes
    }
    pub(super) fn coordinates(&self) -> Coordinates {
        self.coordinates
    }
    pub(super) fn allocation(&self) -> Instance<'tcx> {
        self.allocation
    }
    pub(super) fn caller(&self) -> Instance<'tcx> {
        self.caller
    }
    pub(super) fn conversion(&self) -> Instance<'tcx> {
        self.conversion
    }

    pub(super) fn check(
        tcx: TyCtxt<'tcx>,
        caller: Instance<'tcx>,
        conversion: Instance<'tcx>,
        types: Types<'tcx>,
        work: &mut usize,
    ) -> Result<Self> {
        let lexical_body = caller
            .def_id()
            .as_local()
            .ok_or("reusable LDS caller lacks local HIR")?;
        if !matches!(caller.def, ty::InstanceKind::Item(_))
            || !matches!(
                tcx.def_kind(lexical_body),
                rustc_hir::def::DefKind::Fn | rustc_hir::def::DefKind::Closure
            )
            || caller.def_id() == conversion.def_id()
        {
            return Err("reusable LDS requires an original lexical caller");
        }
        let mut scan = Scan {
            tcx,
            caller,
            conversion,
            types,
            typeck: tcx.typeck(lexical_body),
            lexical_body,
            work,
            depth: 0,
            error: None,
            found: None,
        };
        scan.visit_expr(tcx.hir_body_owned_by(lexical_body).value);
        if let Some(error) = scan.error {
            return Err(error);
        }
        let (nodes, allocation, conversion_span, allocation_span) = scan
            .found
            .ok_or("reusable LDS has no exact nested allocation receiver")?;
        let coordinates = mir_edge(
            tcx,
            caller,
            tcx.instance_mir(caller.def),
            conversion,
            allocation,
            types,
            conversion_span,
            allocation_span,
            work,
        )?;
        Ok(Self {
            caller,
            conversion,
            allocation,
            types,
            nodes,
            coordinates,
        })
    }

    pub(super) fn replay(&self, tcx: TyCtxt<'tcx>, work: &mut usize) -> Result<()> {
        let checked = Self::check(tcx, self.caller, self.conversion, self.types, work)?;
        if self.nodes != checked.nodes
            || self.coordinates != checked.coordinates
            || self.allocation != checked.allocation
        {
            return Err("reusable LDS source occurrence changed on replay");
        }
        Ok(())
    }
}

struct Scan<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    conversion: Instance<'tcx>,
    types: Types<'tcx>,
    typeck: &'tcx TypeckResults<'tcx>,
    lexical_body: LocalDefId,
    work: &'a mut usize,
    depth: u32,
    error: Option<&'static str>,
    found: Option<(SourceNodes, Instance<'tcx>, Span, Span)>,
}

impl<'tcx> Scan<'_, 'tcx> {
    fn resolve(&self, expr: &Expr<'tcx>) -> Result<Instance<'tcx>> {
        let def = self
            .typeck
            .type_dependent_def_id(expr.hir_id)
            .ok_or("reusable LDS unresolved method")?;
        let args = body::normalize(self.tcx, self.caller, self.typeck.node_args(expr.hir_id))
            .ok_or("reusable LDS method arguments did not normalize")?;
        Instance::try_resolve(
            self.tcx,
            TypingEnv::fully_monomorphized(),
            def,
            self.tcx.erase_and_anonymize_regions(args),
        )
        .ok()
        .flatten()
        .ok_or("reusable LDS unresolved monomorphic method")
    }

    fn observe(&self, expr: &'tcx Expr<'tcx>) -> Result<(SourceNodes, Instance<'tcx>, Span, Span)> {
        use ty::adjustment::{Adjust, AutoBorrow, AutoBorrowMutability};
        let ExprKind::MethodCall(_, receiver, arguments, _) = expr.kind else {
            return Err("reusable LDS conversion is not a method call");
        };
        let ExprKind::MethodCall(_, workgroup, allocation_arguments, _) = receiver.kind else {
            return Err("reusable LDS receiver is not its direct allocation expression");
        };
        if self.resolve(expr)? != self.conversion
            || !arguments.is_empty()
            || !allocation_arguments.is_empty()
            || !self.typeck.expr_adjustments(receiver).is_empty()
            || !self.typeck.expr_adjustments(expr).is_empty()
            || body::normalize(self.tcx, self.caller, self.typeck.expr_ty(receiver))
                != Some(self.types.input)
            || body::normalize(
                self.tcx,
                self.caller,
                self.typeck.expr_ty_adjusted(receiver),
            ) != Some(self.types.input)
            || body::normalize(self.tcx, self.caller, self.typeck.expr_ty(expr))
                != Some(self.types.output)
        {
            return Err("reusable LDS nested receiver type, move or adjustment changed");
        }
        let [adjustment] = self.typeck.expr_adjustments(workgroup) else {
            return Err("reusable LDS allocator lacks its exact shared Workgroup borrow");
        };
        if !matches!(
            adjustment.kind,
            Adjust::Borrow(AutoBorrow::Ref(AutoBorrowMutability::Not))
        ) {
            return Err("reusable LDS allocator Workgroup adjustment changed");
        }
        let allocation = self.resolve(receiver)?;
        let signature = self.tcx.instantiate_bound_regions_with_erased(
            self.tcx
                .fn_sig(allocation.def_id())
                .instantiate(self.tcx, allocation.args),
        );
        let signature = self
            .tcx
            .try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
            .map_err(|_| "reusable LDS allocator signature normalization")?;
        if signature.inputs().len() != 1
            || signature.output() != self.types.input
            || body::normalize(
                self.tcx,
                self.caller,
                self.typeck.expr_ty_adjusted(workgroup),
            ) != Some(signature.inputs()[0])
        {
            return Err(
                "reusable LDS allocator signature does not consume the observed Workgroup borrow",
            );
        }
        Ok((
            SourceNodes {
                lexical_body: self.lexical_body,
                conversion: expr.hir_id,
                allocation: receiver.hir_id,
                workgroup: workgroup.hir_id,
            },
            allocation,
            expr.span,
            receiver.span,
        ))
    }
}

impl<'tcx> Visitor<'tcx> for Scan<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.error.is_some() {
            return;
        }
        if let Err(error) = charge(self.work, 1) {
            self.error = Some(error);
            return;
        }
        if self.depth >= 128 {
            self.error = Some("reusable LDS source expression-depth ceiling");
            return;
        }
        if self.typeck.type_dependent_def_id(expr.hir_id) == Some(self.conversion.def_id()) {
            match self.observe(expr) {
                Ok(row) => {
                    if self.found.replace(row).is_some() {
                        self.error = Some("reusable LDS source conversion occurrence is ambiguous");
                        return;
                    }
                }
                Err(error) => {
                    self.error = Some(error);
                    return;
                }
            }
        }
        if !matches!(expr.kind, ExprKind::Closure(_)) {
            self.depth += 1;
            intravisit::walk_expr(self, expr);
            self.depth -= 1;
        }
    }
}

fn callee<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
) -> Option<Instance<'tcx>> {
    if !matches!(operand, Operand::Constant(_)) {
        return None;
    }
    let ty = body::normalize(tcx, caller, operand.ty(&body.local_decls, tcx))?;
    let ty::FnDef(def, args) = *ty.kind() else {
        return None;
    };
    Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        def,
        tcx.erase_and_anonymize_regions(args),
    )
    .ok()
    .flatten()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn mir_edge<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    conversion: Instance<'tcx>,
    allocation: Instance<'tcx>,
    types: Types<'tcx>,
    conversion_span: Span,
    allocation_span: Span,
    work: &mut usize,
) -> Result<Coordinates> {
    charge(
        work,
        body.basic_blocks
            .len()
            .checked_add(body.local_decls.len())
            .ok_or("reusable LDS source-size overflow")?,
    )?;
    if body.source.instance != caller.def
        || body.source.promoted.is_some()
        || body.phase != mir::MirPhase::Runtime(mir::RuntimePhase::Optimized)
    {
        return Err("reusable LDS occurrence is not original optimized MIR");
    }
    let mut producer = None;
    let mut consumer = None;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        let terminator = data
            .terminator
            .as_ref()
            .ok_or("reusable LDS missing source terminator")?;
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = &terminator.kind
        else {
            continue;
        };
        charge(work, args.len() + 1)?;
        let called = callee(tcx, caller, body, func);
        if called == Some(allocation) {
            if data.is_cleanup
                || args.len() != 1
                || !destination.projection.is_empty()
                || body::normalize(tcx, caller, destination.ty(&body.local_decls, tcx).ty)
                    != Some(types.input)
                || !matches!(unwind, UnwindAction::Unreachable)
                || target.is_none()
                || terminator.source_info.span != allocation_span
                || producer
                    .replace((block, destination.local, target.unwrap()))
                    .is_some()
            {
                return Err("reusable LDS allocation occurrence or normal edge changed");
            }
        }
        if called == Some(conversion) {
            if data.is_cleanup
                || args.len() != 1
                || !data.statements.is_empty()
                || !destination.projection.is_empty()
                || body::normalize(tcx, caller, destination.ty(&body.local_decls, tcx).ty)
                    != Some(types.output)
                || !matches!(unwind, UnwindAction::Unreachable)
                || target.is_none()
                || terminator.source_info.span != conversion_span
                || consumer
                    .replace((block, destination.local, target.unwrap(), &args[0].node))
                    .is_some()
            {
                return Err("reusable LDS conversion occurrence, effect or normal edge changed");
            }
        }
    }
    let (allocation_block, allocation_local, next) =
        producer.ok_or("reusable LDS missing retained allocation")?;
    let (conversion_block, conversion_local, return_block, argument) =
        consumer.ok_or("reusable LDS missing retained conversion")?;
    if next != conversion_block
        || allocation_block == conversion_block
        || return_block == allocation_block
        || return_block == conversion_block
        || allocation_local == conversion_local
    {
        return Err("reusable LDS conversion is not on its allocation's immediate normal edge");
    }
    // A second predecessor would allow entry without the allocation, including
    // an unwind edge or a loop back into an already-consumed allocation.
    for (block, data) in body.basic_blocks.iter_enumerated() {
        for successor in data.terminator().successors() {
            charge(work, 1)?;
            if successor == conversion_block && block != allocation_block {
                return Err("reusable LDS conversion has a bypass predecessor");
            }
        }
    }
    let erased = match argument {
        Operand::Move(place) if place.local == allocation_local && place.projection.is_empty() => {
            false
        }
        Operand::Constant(c)
            if body::normalize(tcx, caller, c.const_.ty()) == Some(types.input)
                && body::normalize(tcx, caller, c.const_).and_then(|c| {
                    c.eval(tcx, TypingEnv::fully_monomorphized(), conversion_span)
                        .ok()
                }) == Some(mir::ConstValue::ZeroSized) =>
        {
            true
        }
        _ => {
            return Err(
                "reusable LDS receiver is not the exact owned allocation or authenticated erased argument",
            );
        }
    };
    audit_local(
        body,
        allocation_local,
        allocation_block,
        conversion_block,
        erased,
        work,
    )?;
    Ok(Coordinates {
        allocation_block,
        allocation_local,
        conversion_block,
        conversion_local,
        return_block,
        erased,
    })
}

fn audit_local(
    body: &Body<'_>,
    allocation: Local,
    producer: BasicBlock,
    consumer: BasicBlock,
    erased: bool,
    work: &mut usize,
) -> Result<()> {
    use mir::visit::{
        MutatingUseContext, NonMutatingUseContext, NonUseContext, PlaceContext, Visitor,
    };
    struct Audit<'a> {
        allocation: Local,
        producer: mir::Location,
        consumer: mir::Location,
        erased: bool,
        definitions: u32,
        moves: u32,
        error: Option<&'static str>,
        work: &'a mut usize,
    }
    impl<'tcx> Visitor<'tcx> for Audit<'_> {
        fn visit_statement(&mut self, statement: &mir::Statement<'tcx>, location: mir::Location) {
            if self.error.is_some() {
                return;
            }
            if let Err(error) = charge(self.work, 1) {
                self.error = Some(error);
                return;
            }
            self.super_statement(statement, location);
        }
        fn visit_local(&mut self, local: Local, context: PlaceContext, location: mir::Location) {
            if self.error.is_some() {
                return;
            }
            if let Err(error) = charge(self.work, 1) {
                self.error = Some(error);
                return;
            }
            if local != self.allocation {
                return;
            }
            match context {
                PlaceContext::MutatingUse(MutatingUseContext::Call)
                    if location == self.producer =>
                {
                    self.definitions += 1
                }
                PlaceContext::NonMutatingUse(NonMutatingUseContext::Move)
                    if !self.erased && location == self.consumer =>
                {
                    self.moves += 1
                }
                PlaceContext::NonUse(
                    NonUseContext::VarDebugInfo
                    | NonUseContext::StorageLive
                    | NonUseContext::StorageDead,
                ) => {}
                _ => {
                    self.error = Some(
                        "reusable LDS allocation has another read, move, borrow, write or escape",
                    )
                }
            }
        }
    }
    let mut audit = Audit {
        allocation,
        producer: body.terminator_loc(producer),
        consumer: body.terminator_loc(consumer),
        erased,
        definitions: 0,
        moves: 0,
        error: None,
        work,
    };
    audit.visit_body(body);
    if let Some(error) = audit.error {
        return Err(error);
    }
    if audit.definitions != 1 || audit.moves != u32::from(!erased) {
        return Err("reusable LDS allocation is not consumed exactly once");
    }
    Ok(())
}

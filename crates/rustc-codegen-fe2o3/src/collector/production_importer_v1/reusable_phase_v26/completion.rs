//! Connects a real closure's completion expression to its original Finish call
//! and tuple field. The erased constant is never accepted without this chain.
use super::definitions::{BodyRecipe, Definition, Error, Role, Types, charge, normalize};
use rustc_hir::{
    Expr, ExprKind, HirId,
    intravisit::{self, Visitor},
};
use rustc_middle::{
    mir::{self, BasicBlock, Local, Operand, Rvalue, StatementKind, TerminatorKind, UnwindAction},
    ty::{Instance, InstanceKind, Ty, TyCtxt, TyKind, TypingEnv},
};

type Result<T> = std::result::Result<T, Error>;
fn reject(detail: &'static str) -> Error {
    Error::Body(detail)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Field {
    RetainedResult,
    ErasedConstant,
}

/// Original coordinates only. Canonical mapping and complete incoming/root
/// validation are mandatory before this can become a defined9 relay.
pub(super) struct Relay<'tcx> {
    pub closure: Instance<'tcx>,
    pub finish: Instance<'tcx>,
    pub finish_expression: HirId,
    pub tuple_expression: HirId,
    pub finish_block: BasicBlock,
    pub finish_result: Local,
    pub pack_block: BasicBlock,
    pub pack_statement: u32,
    pub field: Field,
}

pub(super) fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    wrapper: &Definition<'tcx>,
    work: &mut usize,
) -> Result<Relay<'tcx>> {
    let BodyRecipe::WithPhase {
        invoke: closure, ..
    } = wrapper.recipe
    else {
        return Err(reject(
            "completion relay requires a checked with_phase definition",
        ));
    };
    let Types::WithPhase {
        result_pair,
        completion,
        phase,
        ..
    } = wrapper.types
    else {
        return Err(Error::Nominal);
    };
    let local = closure
        .def_id()
        .as_local()
        .ok_or_else(|| reject("phase closure has no live local HIR"))?;
    if !matches!(closure.def, InstanceKind::Item(_))
        || tcx.def_kind(local) != rustc_hir::def::DefKind::Closure
        || !tcx.is_mir_available(closure.def_id())
    {
        return Err(Error::Definition);
    }
    let body = tcx.instance_mir(closure.def);
    if normalize(tcx, closure, body.return_ty())? != result_pair
        || body.source.instance != closure.def
        || body.source.promoted.is_some()
        || body.phase != mir::MirPhase::Runtime(mir::RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
    {
        return Err(reject(
            "phase closure is not its retained original result body",
        ));
    }

    let hir = tcx.hir_body_owned_by(local);
    let tuple = tail(hir.value, work)?;
    let ExprKind::Tup(fields) = tuple.kind else {
        return Err(reject(
            "phase completion must be the actual closure result tuple",
        ));
    };
    if fields.len() != 2 {
        return Err(reject("phase result tuple arity changed"));
    }
    let finish_expr = tail(&fields[0], work)?;
    let ExprKind::MethodCall(_, receiver, arguments, _) = finish_expr.kind else {
        return Err(reject(
            "phase completion field is not its actual Finish expression",
        ));
    };
    let typeck = tcx.typeck(local);
    let def = typeck
        .type_dependent_def_id(finish_expr.hir_id)
        .ok_or(Error::Definition)?;
    let args = normalize(tcx, closure, typeck.node_args(finish_expr.hir_id))?;
    let finish = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        def,
        tcx.erase_and_anonymize_regions(args),
    )
    .ok()
    .flatten()
    .ok_or(Error::Definition)?;
    let definition = Definition::observe(tcx, finish, Role::Finish, work)?;
    let Types::Finish {
        phase: finish_phase,
        completion: actual_completion,
        ..
    } = definition.types
    else {
        return Err(Error::Nominal);
    };
    if !arguments.is_empty()
        || !typeck.expr_adjustments(receiver).is_empty()
        || !typeck.expr_adjustments(finish_expr).is_empty()
        || normalize(tcx, closure, typeck.expr_ty(receiver))? != finish_phase.workgroup
        || normalize(tcx, closure, typeck.expr_ty(finish_expr))? != completion
        || normalize(tcx, closure, typeck.expr_ty(tuple))? != result_pair
        || actual_completion != completion
        || finish_phase.root != phase.root
        || finish_phase.brand != phase.brand
        || finish_phase.dynamic_epoch != phase.dynamic_epoch
    {
        return Err(reject(
            "phase completion receiver, field or nominal epoch chain changed",
        ));
    }

    let mut scan = HirScan {
        tcx,
        owner: local,
        finish: finish.def_id(),
        work,
        depth: 0,
        calls: 0,
        returns: 0,
        failed: false,
    };
    scan.visit_expr(hir.value);
    if scan.failed || scan.calls != 1 || scan.returns != 0 {
        return Err(reject(
            "phase completion has duplicate or bypassing HIR return paths",
        ));
    }
    let mut found = None;
    for (block, data) in body.basic_blocks.iter_enumerated() {
        charge(work, data.statements.len().saturating_add(1))?;
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = &data.terminator().kind
        else {
            continue;
        };
        let TyKind::FnDef(def, args_types) =
            *normalize(tcx, closure, func.ty(&body.local_decls, tcx))?.kind()
        else {
            continue;
        };
        if def != finish.def_id() {
            continue;
        }
        let actual = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            def,
            tcx.erase_and_anonymize_regions(args_types),
        )
        .ok()
        .flatten()
        .ok_or(Error::Definition)?;
        if actual != finish
            || found.is_some()
            || args.len() != 1
            || !matches!(func, Operand::Constant(_))
            || !destination.projection.is_empty()
            || *unwind != UnwindAction::Unreachable
            || normalize(tcx, closure, body.local_decls[destination.local].ty)? != completion
        {
            return Err(reject(
                "phase Finish original call or normal result changed",
            ));
        }
        let next = target.ok_or_else(|| reject("phase Finish has no normal result"))?;
        found = Some((block, destination.local, next));
    }
    let (finish_block, finish_result, pack_block) =
        found.ok_or_else(|| reject("phase Finish has no exact original MIR call"))?;
    let pack = body.basic_blocks.get(pack_block).ok_or(Error::Definition)?;
    if pack.is_cleanup
        || !matches!(pack.terminator().kind, TerminatorKind::Return)
        || pack.statements.is_empty()
        || pack_block == mir::START_BLOCK
    {
        return Err(reject(
            "phase completion pack/return is not its immediate normal successor",
        ));
    }
    for (block, data) in body.basic_blocks.iter_enumerated() {
        charge(work, 1)?;
        for next in data.terminator().successors() {
            charge(work, 1)?;
            if next == pack_block && block != finish_block {
                return Err(reject("phase completion pack bypasses its Finish call"));
            }
        }
    }
    let pack_statement = pack.statements.len() - 1;
    for statement in &pack.statements[..pack_statement] {
        charge(work, 1)?;
        let StatementKind::Assign(assignment) = &statement.kind else {
            return Err(reject("phase completion prefix is not a retained scalar read"));
        };
        let Rvalue::Use(Operand::Copy(source)) = &assignment.1 else {
            return Err(reject("phase completion prefix changed its scalar copy"));
        };
        charge(work, source.projection.len())?;
        let destination = assignment.0;
        let ty = normalize(tcx, closure, body.local_decls[destination.local].ty)?;
        if !destination.projection.is_empty()
            || destination.local == mir::RETURN_PLACE
            || destination.local == finish_result
            || destination.local.as_usize() <= body.arg_count
            || source.local == mir::RETURN_PLACE
            || source.local == finish_result
            || normalize(tcx, closure, source.ty(&body.local_decls, tcx).ty)? != ty
            || !scalar_read_type(ty)
        {
            return Err(reject("phase completion prefix changes completion or capability custody"));
        }
    }
    let StatementKind::Assign(assignment) = &pack.statements[pack_statement].kind else {
        return Err(Error::Definition);
    };
    let Rvalue::Aggregate(kind, values) = &assignment.1 else {
        return Err(Error::Definition);
    };
    if assignment.0.local != mir::RETURN_PLACE
        || !assignment.0.projection.is_empty()
        || !matches!(**kind, mir::AggregateKind::Tuple)
        || values.len() != 2
    {
        return Err(reject(
            "phase completion is not the original returned tuple field0",
        ));
    }
    let field = match &values[rustc_abi::FieldIdx::from_usize(0)] {
        Operand::Move(place) if place.local == finish_result && place.projection.is_empty() => {
            Field::RetainedResult
        }
        Operand::Constant(c)
            if normalize(tcx, closure, c.const_.ty())? == completion
                && normalize(tcx, closure, c.const_)?
                    .eval(tcx, TypingEnv::fully_monomorphized(), c.span)
                    .ok()
                    == Some(mir::ConstValue::ZeroSized) =>
        {
            Field::ErasedConstant
        }
        _ => {
            return Err(reject(
                "phase completion tuple substituted its source Finish result",
            ));
        }
    };
    Ok(Relay {
        closure,
        finish,
        finish_expression: finish_expr.hir_id,
        tuple_expression: tuple.hir_id,
        finish_block,
        finish_result,
        pack_block,
        pack_statement: u32::try_from(pack_statement).map_err(|_| Error::Definition)?,
        field,
    })
}

fn scalar_read_type(ty: Ty<'_>) -> bool {
    fn scalar(ty: Ty<'_>) -> bool {
        matches!(ty.kind(), TyKind::Bool | TyKind::Char | TyKind::Int(_) | TyKind::Uint(_) | TyKind::Float(_))
    }
    scalar(ty)
        || matches!(ty.kind(), TyKind::Ref(_, pointee, rustc_hir::Mutability::Not) if scalar(*pointee))
}

fn tail<'tcx>(mut expr: &'tcx Expr<'tcx>, work: &mut usize) -> Result<&'tcx Expr<'tcx>> {
    for _ in 0..64 {
        charge(work, 1)?;
        expr = match expr.kind {
            ExprKind::Block(block, _) => block
                .expr
                .ok_or_else(|| reject("phase source has no tail value"))?,
            ExprKind::DropTemps(inner) => inner,
            _ => return Ok(expr),
        };
    }
    Err(reject("phase completion source nesting ceiling"))
}

struct HirScan<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    owner: rustc_hir::def_id::LocalDefId,
    finish: rustc_hir::def_id::DefId,
    work: &'a mut usize,
    depth: usize,
    calls: usize,
    returns: usize,
    failed: bool,
}
impl<'tcx> Visitor<'tcx> for HirScan<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if self.failed {
            return;
        }
        if charge(self.work, 1).is_err() || self.depth == 128 {
            self.failed = true;
            return;
        }
        if matches!(expr.kind, ExprKind::Ret(_)) {
            self.returns += 1;
        }
        if self
            .tcx
            .typeck(self.owner)
            .type_dependent_def_id(expr.hir_id)
            == Some(self.finish)
        {
            self.calls += 1;
        }
        self.depth += 1;
        intravisit::walk_expr(self, expr);
        self.depth -= 1;
    }
}

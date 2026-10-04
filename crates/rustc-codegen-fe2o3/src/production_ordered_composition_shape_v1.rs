//! Source-side full helper shape: scalar, acyclic, single return and no hidden
//! memory/call effects. Canonical structural and checked owners recheck later.
use crate::production_ordered_program_v32::MAX_PROFILE_BLOCKS;
use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1;
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;
use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1;
use rustc_middle::mir::{
    BinOp, Body, Operand, Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnOp, UnwindAction,
};
use rustc_middle::ty::{EarlyBinder, Instance, TyCtxt, TyKind, TypingEnv, UintTy};

fn u32_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    ty: rustc_middle::ty::Ty<'tcx>,
) -> bool {
    instance
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(ty),
        )
        .is_ok_and(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
}
fn scalar_operand<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    operand: &Operand<'tcx>,
) -> bool {
    if !u32_type(tcx, instance, operand.ty(body, tcx)) {
        return false;
    }
    match operand {
        Operand::Copy(place) | Operand::Move(place) => place.projection.is_empty(),
        Operand::Constant(value) => instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(value.const_),
            )
            .ok()
            .and_then(|value| value.try_eval_bits(tcx, TypingEnv::fully_monomorphized()))
            .is_some_and(|bits| bits <= u128::from(u32::MAX)),
        Operand::RuntimeChecks(..) => false,
    }
}
pub(super) fn require_scalar_helper<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    function: SemanticFunctionIdV1,
) -> Result<(), &'static str> {
    let instance = plan
        .function_producers()
        .get(function.index() as usize)
        .ok_or("ordered composition helper function missing")?
        .instance;
    let body = tcx.instance_mir(instance.def);
    if body.basic_blocks.is_empty()
        || body.basic_blocks.len() > MAX_PROFILE_BLOCKS
        || body.arg_count != 3
        || body
            .local_decls
            .iter()
            .any(|local| !u32_type(tcx, instance, local.ty))
    {
        return Err("ordered composition helper requires only direct u32 locals");
    }
    if plan
        .normalized_intrinsic_producers()
        .iter()
        .any(|recipe| recipe.caller == function)
    {
        return Err("ordered composition helper normalized intrinsic is outside scalar profile");
    }
    let scalar = |operand: &Operand<'tcx>| scalar_operand(tcx, instance, body, operand);
    for block in body.basic_blocks.iter() {
        for statement in &block.statements {
            match &statement.kind {
                StatementKind::StorageLive(_)
                | StatementKind::StorageDead(_)
                | StatementKind::Nop => {}
                StatementKind::Assign(assignment) => {
                    let (destination, value) = &**assignment;
                    if !destination.projection.is_empty() {
                        return Err("ordered composition helper projected destination refused");
                    }
                    let accepted = match value {
                        Rvalue::Use(operand) => scalar(operand),
                        Rvalue::BinaryOp(operation, operands)
                            if matches!(
                                operation,
                                BinOp::Add
                                    | BinOp::Sub
                                    | BinOp::BitAnd
                                    | BinOp::BitOr
                                    | BinOp::BitXor
                            ) =>
                        {
                            scalar(&operands.0) && scalar(&operands.1)
                        }
                        Rvalue::UnaryOp(UnOp::Not, operand) => scalar(operand),
                        _ => false,
                    };
                    if !accepted {
                        return Err(
                            "ordered composition helper non-scalar or effectful statement refused",
                        );
                    }
                }
                _ => return Err("ordered composition helper unsupported statement refused"),
            }
        }
    }
    // Fixed scratch, never caller-sized allocation. All raw helper blocks must
    // form one returning chain; unvisited/cleanup blocks and backedges refuse.
    let mut visited = [false; MAX_PROFILE_BLOCKS];
    let mut cursor = START_BLOCK;
    for index in 0..body.basic_blocks.len() {
        let raw = cursor.index();
        if raw >= body.basic_blocks.len() || visited[raw] {
            return Err("ordered composition helper has a loop or foreign edge");
        }
        visited[raw] = true;
        match &body.basic_blocks[cursor].terminator().kind {
            TerminatorKind::Goto { target } => cursor = *target,
            TerminatorKind::Call {
                target: Some(target),
                unwind: UnwindAction::Continue | UnwindAction::Unreachable,
                ..
            } => {
                let mut recipes = plan
                    .terminal_expansion_producers()
                    .iter()
                    .filter(|recipe| recipe.caller == function && recipe.block as usize == raw);
                if !recipes.next().is_some_and(|recipe| {
                    recipe.expansion == ProductionTerminalExpansionV1::Gfx942OrderedProgramE32
                }) || recipes.next().is_some()
                {
                    return Err(
                        "ordered composition helper only calls authenticated program markers",
                    );
                }
                cursor = *target;
            }
            TerminatorKind::Return if index + 1 == body.basic_blocks.len() => return Ok(()),
            _ => {
                return Err(
                    "ordered composition helper must be one nonunwinding straight-line return",
                );
            }
        }
    }
    Err("ordered composition helper has no final return")
}

//! Actual rustc call/ABI and raw-to-semantic transport joins for composition.
//! No public constructor and no source authority reconstructed from wire bytes.
use super::roster::{Binding, Operand, Site, Transport};
use crate::rustc_semantic_adapter_v1::canonical_function_identities_v1;
use crate::rustc_semantic_plan_v1::{
    ProductionSemanticPreflightPlanV1, RetainedSemanticBodyProducerV1,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{BasicBlock, Operand as MirOperand, TerminatorKind, UnwindAction};
use rustc_middle::ty::{EarlyBinder, Instance, TyCtxt, TyKind, TypingEnv, UintTy};

pub(super) fn helper_abi_is_direct(
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> bool {
    let scalar = |ty: SemanticTypeIdV1| {
        types.get(ty.index() as usize).is_some_and(|ty| {
            matches!(
                ty.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            )
        })
    };
    abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.can_unwind()
        && !abi.c_variadic()
        && abi.source_input_types().len() == 3
        && abi.fixed_count() == 3
        && abi.arguments().len() == 3
        && abi.hidden_arguments().is_empty()
        && abi.source_input_types().iter().all(|ty| scalar(*ty))
        && abi
            .arguments()
            .iter()
            .zip(abi.source_input_types())
            .all(|(argument, source)| {
                argument.role() == SemanticAbiArgumentRoleV1::Source
                    && argument.value().source_ty() == *source
                    && argument.value().adjusted_ty() == *source
                    && argument.value().pointee_override().is_none()
                    && matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
            })
        && scalar(abi.source_output_type())
        && abi.return_value().source_ty() == abi.source_output_type()
        && abi.return_value().adjusted_ty() == abi.source_output_type()
        && abi.return_value().pointee_override().is_none()
        && matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Direct(_))
}

pub(super) fn require_helper_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<(), &'static str> {
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != 3
        || signature
            .inputs()
            .iter()
            .any(|ty| !matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
        || !matches!(signature.output().kind(), TyKind::Uint(UintTy::U32))
    {
        return Err("ordered composition helper requires safe Rust fn(u32,u32,u32)->u32");
    }
    Ok(())
}

pub(super) fn mapped_local(
    retained: &RetainedSemanticBodyProducerV1,
    raw: usize,
    ty: SemanticTypeIdV1,
) -> Result<SemanticLocalIdV1, &'static str> {
    let semantic = *retained
        .raw_to_semantic_locals
        .get(raw)
        .ok_or("ordered composition raw local map missing")?;
    let local = retained
        .locals
        .get(semantic.index() as usize)
        .ok_or("ordered composition semantic local map missing")?;
    if usize::try_from(local.rustc_local).ok() != Some(raw) || local.ty != ty {
        return Err("ordered composition raw-to-semantic local/type join differs");
    }
    Ok(semantic)
}

pub(super) fn mapped_block(
    retained: &RetainedSemanticBodyProducerV1,
    raw: usize,
) -> Result<(SemanticBlockIdV1, SemanticBlockIdentityV1), &'static str> {
    let semantic = *retained
        .raw_to_semantic_blocks
        .get(raw)
        .ok_or("ordered composition raw block map missing")?;
    let block = retained
        .blocks
        .get(semantic.index() as usize)
        .ok_or("ordered composition semantic block map missing")?;
    if usize::try_from(block.rustc_block).ok() != Some(raw) {
        return Err("ordered composition raw-to-semantic block join differs");
    }
    Ok((semantic, block.identity))
}

fn unsigned_type<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    raw: rustc_middle::ty::Ty<'tcx>,
    expected: UintTy,
) -> Result<SemanticTypeIdV1, &'static str> {
    let ty = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(raw),
        )
        .map_err(|_| "ordered composition operand type normalization failed")?;
    if !matches!(ty.kind(), TyKind::Uint(kind) if *kind == expected) {
        return Err("ordered composition actual call operand type differs");
    }
    let index = plan
        .type_producers()
        .iter()
        .position(|entry| entry.ty == ty)
        .ok_or("ordered composition actual operand type has no retained binding")?;
    Ok(SemanticTypeIdV1::from_index(
        u32::try_from(index).map_err(|_| "ordered composition type index overflow")?,
    ))
}

pub(super) fn capture_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    caller_id: SemanticFunctionIdV1,
    raw_block: u32,
    count: usize,
) -> Result<(Site, Binding<Instance<'tcx>>), &'static str> {
    if !matches!(count, 3 | 8) {
        return Err("ordered composition call arity differs");
    }
    let caller = plan
        .function_producers()
        .get(caller_id.index() as usize)
        .ok_or("ordered composition caller missing")?;
    if canonical_function_identities_v1(tcx, caller.instance) != caller.identities {
        return Err("ordered composition caller Instance identities differ");
    }
    let retained = plan
        .body_producers()
        .get(caller_id.index() as usize)
        .filter(|body| body.function == caller_id)
        .ok_or("ordered composition retained caller body differs")?;
    let body = tcx.instance_mir(caller.instance.def);
    let raw = body
        .basic_blocks
        .get(BasicBlock::from_u32(raw_block))
        .ok_or("ordered composition actual call block missing")?;
    let TerminatorKind::Call {
        func,
        args,
        destination,
        target: Some(target),
        unwind,
        ..
    } = &raw.terminator().kind
    else {
        return Err("ordered composition requires an actual returning direct call");
    };
    let unwind = match unwind {
        UnwindAction::Continue => SemanticUnwindActionV1::Continue,
        UnwindAction::Unreachable => SemanticUnwindActionV1::Unreachable,
        _ => return Err("ordered composition executable unwind edge refused"),
    };
    if args.len() != count
        || !destination.projection.is_empty()
        || !matches!(func, MirOperand::Constant(_))
    {
        return Err("ordered composition actual argument/destination/callee shape differs");
    }
    let callee = crate::production_semantic_body_v1::resolve_direct_call_v1(
        tcx,
        caller.instance,
        body,
        func,
    )?;
    let callable = if let Some(index) = plan
        .function_producers()
        .iter()
        .position(|function| function.instance == callee)
    {
        index
    } else {
        plan.function_producers()
            .len()
            .checked_add(
                plan.terminal_producers()
                    .iter()
                    .position(|terminal| terminal.instance == callee)
                    .ok_or("ordered composition callee absent from actual producer roster")?,
            )
            .ok_or("ordered composition callable index overflow")?
    };
    let callable = SemanticCallableIdV1::from_index(
        u32::try_from(callable).map_err(|_| "ordered composition callable index overflow")?,
    );
    let mut arguments = [None; 8];
    for (index, argument) in args.iter().enumerate() {
        let expected = if index < 3 { UintTy::U32 } else { UintTy::U8 };
        let ty = unsigned_type(
            tcx,
            caller.instance,
            plan,
            argument.node.ty(body, tcx),
            expected,
        )?;
        arguments[index] = Some(match &argument.node {
            MirOperand::Copy(place) | MirOperand::Move(place) => {
                if index >= 3 || !place.projection.is_empty() {
                    return Err("ordered composition projected or nonliteral call operand refused");
                }
                let local = mapped_local(retained, place.local.index(), ty)?;
                if matches!(&argument.node, MirOperand::Copy(_)) {
                    Operand::Copy { local, ty }
                } else {
                    Operand::Move { local, ty }
                }
            }
            MirOperand::Constant(constant) => {
                let constant = caller
                    .instance
                    .try_instantiate_mir_and_normalize_erasing_regions(
                        tcx,
                        TypingEnv::fully_monomorphized(),
                        EarlyBinder::bind(constant.const_),
                    )
                    .map_err(|_| "ordered composition constant normalization failed")?;
                let bits = constant
                    .try_eval_bits(tcx, TypingEnv::fully_monomorphized())
                    .and_then(|bits| u32::try_from(bits).ok())
                    .ok_or("ordered composition actual scalar constant refused")?;
                let bytes = if index < 3 { 4 } else { 1 };
                if bytes == 1 && bits > u32::from(u8::MAX) {
                    return Err("ordered composition physical literal exceeds u8");
                }
                Operand::Constant { bits, bytes, ty }
            }
            _ => return Err("ordered composition unsupported MIR call operand"),
        });
    }
    let result_type = unsigned_type(
        tcx,
        caller.instance,
        plan,
        destination.ty(body, tcx).ty,
        UintTy::U32,
    )?;
    let destination = mapped_local(retained, destination.local.index(), result_type)?;
    let (block, block_identity) = mapped_block(retained, raw_block as usize)?;
    let (target, _) = mapped_block(retained, target.index())?;
    Ok((
        Site {
            function: caller_id,
            raw_block,
            block,
            block_identity,
        },
        Binding {
            caller: caller.instance,
            callee,
            callable,
            transport: Transport {
                arguments,
                count: count as u8,
                destination,
                result_type,
                target,
                unwind,
            },
        },
    ))
}

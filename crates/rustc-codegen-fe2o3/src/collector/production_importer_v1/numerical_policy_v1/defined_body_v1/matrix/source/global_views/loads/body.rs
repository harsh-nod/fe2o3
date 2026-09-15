//! Closed linear wrapper dataflow; the packer and checked read bodies remain defined.
use super::*;
use rustc_middle::mir::{
    Body, BorrowKind, CastKind, MirPhase, Operand, ProjectionElem, RETURN_PLACE, RuntimePhase,
    Rvalue, START_BLOCK, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::EarlyBinder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Value {
    View,
    Lane,
    FirstBase,
    SecondBase,
    LaneU32,
    LaneUsize,
    Registers,
    Fragment,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn wrapper<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    body: &Body<'tcx>,
    inputs: [Ty<'tcx>; 4],
    output: Ty<'tcx>,
    registers: Ty<'tcx>,
    format: Format,
    role: Role,
) -> Option<[Instance<'tcx>; 3]> {
    if body.source.instance != instance.def
        || body.source.promoted.is_some()
        || body.phase != MirPhase::Runtime(RuntimePhase::Optimized)
        || body.injection_phase.is_some()
        || body.tainted_by_errors.is_some()
        || body.coroutine.is_some()
        || body.spread_arg.is_some()
        || body.arg_count != 4
        || !(5..=16).contains(&body.local_decls.len())
        || body.basic_blocks.len() != 4
        || !body.user_type_annotations.is_empty()
        || body.basic_blocks.iter().any(|block| {
            block.is_cleanup || block.terminator.is_none() || block.statements.len() > 16
        })
    {
        return None;
    }
    let normalize = |ty| {
        instance
            .try_instantiate_mir_and_normalize_erasing_regions(
                tcx,
                TypingEnv::fully_monomorphized(),
                EarlyBinder::bind(ty),
            )
            .ok()
    };
    let types = body
        .local_decls
        .iter()
        .map(|local| normalize(local.ty))
        .collect::<Option<Vec<_>>>()?;
    if types[..5] != [output, inputs[0], inputs[1], inputs[2], inputs[3]] {
        return None;
    }
    let expected_ty = |value| match value {
        Value::View => inputs[0],
        Value::Lane => inputs[1],
        Value::FirstBase | Value::SecondBase | Value::LaneUsize => tcx.types.usize,
        Value::LaneU32 => tcx.types.u32,
        Value::Registers => registers,
        Value::Fragment => output,
    };
    let mut values = vec![None; types.len()];
    values[1..5].copy_from_slice(&[
        Some(Value::View),
        Some(Value::Lane),
        Some(Value::FirstBase),
        Some(Value::SecondBase),
    ]);
    let pack_path = match (format, role) {
        (Format::Fp4E2M1, Role::A) => "fe2o3_device::gfx950::pack_fp4_a",
        (Format::Fp4E2M1, Role::B) => "fe2o3_device::gfx950::pack_fp4_b",
        (Format::Fp8E4M3, Role::A) => "fe2o3_device::gfx950::pack_fp8_a",
        (Format::Fp8E4M3, Role::B) => "fe2o3_device::gfx950::pack_fp8_b",
    };
    let paths = [
        "fe2o3_device::wave::SubgroupLane::get",
        pack_path,
        "fe2o3_device::gfx950::Gfx950MfmaFragment::from_registers",
    ];
    let expected_args: [&[Value]; 3] = [
        &[Value::Lane],
        &[
            Value::View,
            Value::LaneUsize,
            Value::FirstBase,
            Value::SecondBase,
        ],
        &[Value::Lane, Value::Registers],
    ];
    let results = [Value::LaneU32, Value::Registers, Value::Fragment];
    let mut helpers = Vec::with_capacity(3);
    let mut visited = [false; 4];
    let mut block = START_BLOCK;
    for step in 0..4 {
        let seen = visited.get_mut(block.as_usize())?;
        if *seen {
            return None;
        }
        *seen = true;
        let data = &body.basic_blocks[block];
        for statement in &data.statements {
            match &statement.kind {
                StatementKind::Assign(assignment) => {
                    let (place, rvalue) = &**assignment;
                    if !place.projection.is_empty()
                        || place.local.as_usize() <= body.arg_count && place.local != RETURN_PLACE
                    {
                        return None;
                    }
                    let value = match rvalue {
                        Rvalue::Use(operand) => take(operand, &mut values)?,
                        Rvalue::Cast(CastKind::IntToInt, operand, target)
                            if normalize(*target) == Some(tcx.types.usize) =>
                        {
                            if take(operand, &mut values)? != Value::LaneU32 {
                                return None;
                            }
                            Value::LaneUsize
                        }
                        Rvalue::Ref(_, BorrowKind::Shared, source)
                            if source.projection.as_ref() == [ProjectionElem::Deref] =>
                        {
                            let value = values[source.local.as_usize()]?;
                            if !matches!(value, Value::View | Value::Lane) {
                                return None;
                            }
                            value
                        }
                        _ => return None,
                    };
                    let index = place.local.as_usize();
                    if types.get(index).copied()? != expected_ty(value) {
                        return None;
                    }
                    values[index] = Some(value);
                }
                StatementKind::StorageLive(local) | StatementKind::StorageDead(local)
                    if local.as_usize() > body.arg_count =>
                {
                    values[local.as_usize()] = None;
                }
                _ => return None,
            }
        }
        if step == 3 {
            if !matches!(data.terminator().kind, TerminatorKind::Return)
                || values[0] != Some(Value::Fragment)
            {
                return None;
            }
            break;
        }
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target: Some(target),
            unwind: UnwindAction::Unreachable,
            ..
        } = &data.terminator().kind
        else {
            return None;
        };
        if !matches!(func, Operand::Constant(_))
            || !destination.projection.is_empty()
            || destination.local.as_usize() <= body.arg_count && destination.local != RETURN_PLACE
        {
            return None;
        }
        let arguments = args
            .iter()
            .map(|arg| take(&arg.node, &mut values))
            .collect::<Option<Vec<_>>>()?;
        if arguments != expected_args[step] {
            return None;
        }
        let ty = normalize(func.ty(&body.local_decls, tcx))?;
        let TyKind::FnDef(definition, arguments) = *ty.kind() else {
            return None;
        };
        let callee = Instance::try_resolve(
            tcx,
            TypingEnv::fully_monomorphized(),
            definition,
            tcx.erase_and_anonymize_regions(arguments),
        )
        .ok()??;
        if !trusted_device_items::is_exact_reviewed_provider_definition_v1(
            tcx,
            callee.def_id(),
            paths[step],
        ) || !trusted_device_items::authenticate_reviewed_safe_external_helper_v1(
            tcx,
            callee.def_id(),
        )
        .ok()?
        {
            return None;
        }
        let signature = super::super::super::signature(tcx, callee).ok()?;
        if signature.inputs()
            != expected_args[step]
                .iter()
                .copied()
                .map(expected_ty)
                .collect::<Vec<_>>()
            || signature.output() != expected_ty(results[step])
            || types[destination.local.as_usize()] != signature.output()
        {
            return None;
        }
        values[destination.local.as_usize()] = Some(results[step]);
        helpers.push(callee);
        block = *target;
    }
    visited.into_iter().all(|seen| seen).then_some(())?;
    helpers.try_into().ok()
}

fn take(operand: &Operand<'_>, values: &mut [Option<Value>]) -> Option<Value> {
    let (place, moved) = match operand {
        Operand::Copy(place) => (place, false),
        Operand::Move(place) => (place, true),
        _ => return None,
    };
    if !place.projection.is_empty() {
        return None;
    }
    let slot = values.get_mut(place.local.as_usize())?;
    if moved { slot.take() } else { *slot }
}

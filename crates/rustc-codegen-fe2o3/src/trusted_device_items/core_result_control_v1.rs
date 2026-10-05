//! Closed source-origin authentication for the observed core Result helpers.

use rustc_abi::ExternAbi;
use rustc_hir::{Safety, def::DefKind, def_id::DefId};
use rustc_middle::mir::{
    AggregateKind, BasicBlock, BinOp, Body, ClearCrossCrate, NonDivergingIntrinsic, Operand, Place,
    ProjectionElem, Rvalue, SourceScope, Statement, StatementKind, TerminatorKind, UnwindAction,
};
use rustc_middle::ty::{
    EarlyBinder, FloatTy, Instance, InstanceKind, Ty, TyCtxt, TyKind, TypeVisitableExt, TypingEnv,
    UintTy,
};
use rustc_span::{Symbol, sym};

use super::{
    TrustedDeviceItem, classify, definition, reviewed_provider_semantic_definition_v1,
    validate_safe_execution_provider_definition_v1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultPayloadFamilyV1 {
    Unit,
    U32,
    Usize,
    MatrixA,
    MatrixB,
    StridedF32,
    AttentionTile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResultErrorFamilyV1 {
    Kernel,
    Matrix,
    Strided,
    CheckedExtent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResultFamilyV1 {
    payload: ResultPayloadFamilyV1,
    error: ResultErrorFamilyV1,
}

fn selected_pair_v1(payload: ResultPayloadFamilyV1, error: ResultErrorFamilyV1) -> bool {
    use ResultErrorFamilyV1 as E;
    use ResultPayloadFamilyV1 as P;
    matches!(
        (payload, error),
        (P::Unit, E::Kernel | E::Matrix | E::Strided | E::CheckedExtent)
            | (P::U32 | P::AttentionTile, E::Kernel)
            | (P::Usize, E::Kernel | E::CheckedExtent)
            | (P::MatrixA | P::MatrixB, E::Matrix)
            | (P::StridedF32, E::Strided)
    )
}

fn result_arguments_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<(Ty<'tcx>, Ty<'tcx>)> {
    let TyKind::Adt(result, arguments) = *ty.kind() else {
        return None;
    };
    let core = tcx.lang_items().sized_trait()?.krate;
    if result.did().krate != core
        || tcx.crate_name(core).as_str() != "core"
        || !tcx.is_diagnostic_item(sym::Result, result.did())
        || !result.is_enum()
        || arguments.len() != 2
    {
        return None;
    }
    let mut variants = result.variants().iter();
    if variants.next()?.def_id != tcx.lang_items().result_ok_variant()?
        || variants.next()?.def_id != tcx.lang_items().result_err_variant()?
        || variants.next().is_some()
    {
        return None;
    }
    Some((arguments[0].as_type()?, arguments[1].as_type()?))
}

fn error_family_v1(tcx: TyCtxt<'_>, ty: Ty<'_>) -> Option<ResultErrorFamilyV1> {
    let TyKind::Adt(adt, arguments) = *ty.kind() else {
        return None;
    };
    if !adt.is_enum() || !arguments.is_empty() {
        return None;
    }
    match classify(tcx, adt.did()) {
        Some(TrustedDeviceItem::KernelError) => Some(ResultErrorFamilyV1::Kernel),
        Some(TrustedDeviceItem::Bf16MfmaMatrixViewError) => Some(ResultErrorFamilyV1::Matrix),
        Some(TrustedDeviceItem::StridedReadView2DError) => Some(ResultErrorFamilyV1::Strided),
        _ => {
            // This private enum has no diagnostic item. Bind its real DefId to
            // the same complete provider closure, then its exact source path.
            let provider = reviewed_provider_semantic_definition_v1(tcx, adt.did()).ok()?;
            validate_safe_execution_provider_definition_v1(&provider).ok()?;
            (provider.canonical_definition_path == "fe2o3_device::views::CheckedStridedExtentError")
                .then_some(ResultErrorFamilyV1::CheckedExtent)
        }
    }
}

fn exact_index1d_v1<'tcx>(tcx: TyCtxt<'tcx>) -> Option<Ty<'tcx>> {
    let function = definition(tcx, TrustedDeviceItem::ThreadIndex1d)?;
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(function).instantiate_identity());
    let TyKind::Adt(adt, arguments) = *signature.output().kind() else {
        return None;
    };
    if classify(tcx, adt.did()) != Some(TrustedDeviceItem::ThreadIndex) || arguments.len() != 1 {
        return None;
    }
    arguments[0].as_type()
}

fn payload_family_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<ResultPayloadFamilyV1> {
    match *ty.kind() {
        TyKind::Tuple(fields) if fields.is_empty() => return Some(ResultPayloadFamilyV1::Unit),
        TyKind::Uint(UintTy::U32) => return Some(ResultPayloadFamilyV1::U32),
        TyKind::Uint(UintTy::Usize) => return Some(ResultPayloadFamilyV1::Usize),
        _ => {}
    }
    let TyKind::Adt(adt, arguments) = *ty.kind() else {
        return None;
    };
    match classify(tcx, adt.did())? {
        TrustedDeviceItem::Bf16MfmaMatrixView => {
            if arguments.len() != 2 || arguments[0].as_region().is_none() {
                return None;
            }
            let TyKind::Adt(role, role_arguments) = *arguments[1].as_type()?.kind() else {
                return None;
            };
            if !role_arguments.is_empty() {
                return None;
            }
            match classify(tcx, role.did())? {
                TrustedDeviceItem::MfmaOperandA => Some(ResultPayloadFamilyV1::MatrixA),
                TrustedDeviceItem::MfmaOperandB => Some(ResultPayloadFamilyV1::MatrixB),
                _ => None,
            }
        }
        TrustedDeviceItem::StridedReadView2D => {
            if arguments.len() != 2
                || arguments[0].as_region().is_none()
                || !matches!(arguments[1].as_type()?.kind(), TyKind::Float(FloatTy::F32))
            {
                return None;
            }
            Some(ResultPayloadFamilyV1::StridedF32)
        }
        TrustedDeviceItem::DisjointTile2D => {
            if arguments.len() != 5 || arguments[0].as_type()? != exact_index1d_v1(tcx)? {
                return None;
            }
            for (argument, expected) in arguments.iter().skip(1).zip([64, 16, 16, 4]) {
                if argument.as_const()?.try_to_target_usize(tcx)? != expected {
                    return None;
                }
            }
            Some(ResultPayloadFamilyV1::AttentionTile)
        }
        _ => None,
    }
}

fn result_family_v1<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> Option<ResultFamilyV1> {
    let (payload, error) = result_arguments_v1(tcx, ty)?;
    let payload = payload_family_v1(tcx, payload)?;
    let error = error_family_v1(tcx, error)?;
    selected_pair_v1(payload, error).then_some(ResultFamilyV1 { payload, error })
}

struct BranchTypesV1<'tcx> {
    input: Ty<'tcx>,
    output: Ty<'tcx>,
    residual: Ty<'tcx>,
    payload: Ty<'tcx>,
    error: Ty<'tcx>,
}

fn branch_signature_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> Option<BranchTypesV1<'tcx>> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_kind(instance.def_id()) != DefKind::AssocFn
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != 2
        || instance.args.iter().any(|arg| arg.as_type().is_none() || arg.has_param() || arg.has_escaping_bound_vars())
        || tcx.associated_item(instance.def_id()).trait_item_def_id() != tcx.lang_items().branch_fn()
        || tcx.lang_items().branch_fn().is_none()
    {
        return None;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args),
    );
    let signature = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature).ok()?;
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic {
        return None;
    }
    let [input] = signature.inputs() else { return None; };
    let (payload, error) = result_arguments_v1(tcx, *input)?;
    if instance.args[0].as_type()? != payload || instance.args[1].as_type()? != error {
        return None;
    }
    let output = signature.output();
    let TyKind::Adt(control, arguments) = *output.kind() else { return None; };
    if !tcx.is_diagnostic_item(sym::ControlFlow, control.did())
        || control.did().krate != instance.def_id().krate
        || !control.is_enum()
        || arguments.len() != 2
        || arguments[1].as_type()? != payload
    {
        return None;
    }
    let mut variants = control.variants().iter();
    if variants.next()?.def_id != tcx.lang_items().cf_continue_variant()?
        || variants.next()?.def_id != tcx.lang_items().cf_break_variant()?
        || variants.next().is_some()
    {
        return None;
    }
    let residual = arguments[0].as_type()?;
    let (empty, residual_error) = result_arguments_v1(tcx, residual)?;
    let TyKind::Adt(infallible, empty_arguments) = *empty.kind() else { return None; };
    // Infallible has no lang/diagnostic item in the pinned core. Its real
    // definition is anchored to the genuine core crate, structural path and
    // empty shape. Display paths can instead name a visible std re-export.
    if residual_error != error
        || infallible.did().krate != instance.def_id().krate
        || tcx.def_path(infallible.did()).to_string_no_crate_verbose() != "::convert::Infallible"
        || !infallible.is_enum()
        || !infallible.variants().is_empty()
        || !empty_arguments.is_empty()
    {
        return None;
    }
    Some(BranchTypesV1 { input: *input, output, residual, payload, error })
}

fn plain_local_v1(place: &Place<'_>, index: usize) -> bool {
    place.local.as_usize() == index && place.projection.is_empty()
}

fn normalize_type_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, ty: Ty<'tcx>) -> Option<Ty<'tcx>> {
    instance.try_instantiate_mir_and_normalize_erasing_regions(
        tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(ty),
    ).ok()
}

fn move_variant_field_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, statement: &Statement<'tcx>,
    destination: usize, variant: usize, field_type: Ty<'tcx>,
) -> bool {
    let StatementKind::Assign(assignment) = &statement.kind else { return false; };
    let (destination_place, value) = &**assignment;
    let Rvalue::Use(Operand::Move(source)) = value else { return false; };
    let [ProjectionElem::Downcast(_, actual_variant), ProjectionElem::Field(field, ty)] = source.projection.as_slice()
    else { return false; };
    plain_local_v1(destination_place, destination)
        && source.local.as_usize() == 1
        && actual_variant.as_usize() == variant
        && field.as_usize() == 0
        && normalize_type_v1(tcx, instance, *ty) == Some(field_type)
}

fn aggregate_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, statement: &Statement<'tcx>,
    destination: usize, aggregate_type: Ty<'tcx>, variant_def: DefId, source: usize, copied: bool,
) -> bool {
    let StatementKind::Assign(assignment) = &statement.kind else { return false; };
    let (place, value) = &**assignment;
    let Rvalue::Aggregate(kind, operands) = value else { return false; };
    let AggregateKind::Adt(def_id, variant, raw_arguments, None, None) = &**kind else { return false; };
    let TyKind::Adt(expected, expected_arguments) = *aggregate_type.kind() else { return false; };
    let mut operands = operands.iter();
    let (Some(operand), None) = (operands.next(), operands.next()) else { return false; };
    let operand_matches = match (copied, operand) {
        (true, Operand::Copy(place)) | (false, Operand::Move(place)) => plain_local_v1(place, source),
        _ => false,
    };
    plain_local_v1(place, destination)
        && *def_id == expected.did()
        && expected.variants().get(*variant).map(|row| row.def_id) == Some(variant_def)
        && instance.try_instantiate_mir_and_normalize_erasing_regions(
            tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(*raw_arguments),
        ).ok() == Some(expected_arguments)
        && operand_matches
}

fn branch_body_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let Some(types) = branch_signature_v1(tcx, instance) else { return false; };
    if body.arg_count != 1 || body.local_decls.len() != 6
        || body.basic_blocks.len() != 5 || body.source_scopes.len() != 3
    {
        return false;
    }
    for (local, expected) in body.local_decls.iter().zip([
        types.output, types.input, tcx.types.isize, types.payload, types.error, types.residual,
    ]) {
        if normalize_type_v1(tcx, instance, local.ty) != Some(expected)
            || local.source_info.scope != SourceScope::from_u32(0)
        {
            return false;
        }
    }
    for (index, scope) in body.source_scopes.iter().enumerate() {
        if scope.parent_scope != (index != 0).then_some(SourceScope::from_u32(0))
            || scope.inlined.is_some() || scope.inlined_parent_scope.is_some()
            || !matches!(&scope.local_data, ClearCrossCrate::Clear)
        {
            return false;
        }
    }
    let statement_scopes: [&[u32]; 5] = [&[0], &[], &[0, 2, 2, 2, 2], &[0, 1], &[]];
    for (block, scopes) in body.basic_blocks.iter().zip(statement_scopes) {
        if block.is_cleanup || block.statements.len() != scopes.len()
            || block.statements.iter().zip(scopes).any(|(statement, scope)| statement.source_info.scope != SourceScope::from_u32(*scope))
            || block.terminator.as_ref().is_none_or(|term| term.source_info.scope != SourceScope::from_u32(0))
        {
            return false;
        }
    }
    let block = |index| &body.basic_blocks[BasicBlock::from_u32(index)];
    let StatementKind::Assign(discriminant) = &block(0).statements[0].kind else { return false; };
    if !plain_local_v1(&discriminant.0, 2)
        || !matches!(&discriminant.1, Rvalue::Discriminant(input) if plain_local_v1(input, 1))
    {
        return false;
    }
    let TerminatorKind::SwitchInt { discr, targets } = &block(0).terminator().kind else { return false; };
    if !matches!(discr, Operand::Move(place) if plain_local_v1(place, 2))
        || !targets.iter().eq([(0, BasicBlock::from_u32(3)), (1, BasicBlock::from_u32(2))])
        || targets.otherwise() != BasicBlock::from_u32(1)
        || !matches!(block(1).terminator().kind, TerminatorKind::Unreachable)
        || !matches!(block(4).terminator().kind, TerminatorKind::Return)
        || !matches!(block(2).terminator().kind, TerminatorKind::Goto { target } if target == BasicBlock::from_u32(4))
        || !matches!(block(3).terminator().kind, TerminatorKind::Goto { target } if target == BasicBlock::from_u32(4))
    {
        return false;
    }
    let Some(err) = tcx.lang_items().result_err_variant() else { return false; };
    let Some(break_) = tcx.lang_items().cf_break_variant() else { return false; };
    let Some(continue_) = tcx.lang_items().cf_continue_variant() else { return false; };
    let error = &block(2).statements;
    let ok = &block(3).statements;
    move_variant_field_v1(tcx, instance, &error[0], 4, 1, types.error)
        && matches!(error[1].kind, StatementKind::StorageLive(local) if local.as_usize() == 5)
        && aggregate_v1(tcx, instance, &error[2], 5, types.residual, err, 4, true)
        && aggregate_v1(tcx, instance, &error[3], 0, types.output, break_, 5, false)
        && matches!(error[4].kind, StatementKind::StorageDead(local) if local.as_usize() == 5)
        && move_variant_field_v1(tcx, instance, &ok[0], 3, 0, types.payload)
        && aggregate_v1(tcx, instance, &ok[1], 0, types.output, continue_, 3, true)
}

/// Only discharges the external-HIR source-origin check. The same real MIR,
/// all nested calls and the independent inline-origin audit remain mandatory.
pub(crate) fn authenticate_reviewed_safe_core_result_branch_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>,
) -> bool {
    let Some(types) = branch_signature_v1(tcx, instance) else { return false; };
    result_family_v1(tcx, types.input).is_some()
        && branch_body_v1(tcx, instance, tcx.instance_mir(instance.def))
}

struct ResidualTypesV1<'tcx> {
    input: Ty<'tcx>,
    output: Ty<'tcx>,
    error: Ty<'tcx>,
    converted_error: Ty<'tcx>,
}

fn residual_signature_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>,
) -> Option<ResidualTypesV1<'tcx>> {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !crate::production_rustc_intrinsic_v1::is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_kind(instance.def_id()) != DefKind::AssocFn
        || !tcx.is_mir_available(instance.def_id())
        || instance.args.len() != 3
        || instance.args.iter().any(|arg| arg.as_type().is_none() || arg.has_param() || arg.has_escaping_bound_vars())
        || tcx.associated_item(instance.def_id()).trait_item_def_id() != tcx.lang_items().from_residual_fn()
        || tcx.lang_items().from_residual_fn().is_none()
    {
        return None;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id()).instantiate(tcx, instance.args),
    );
    let signature = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature).ok()?;
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic {
        return None;
    }
    let [input] = signature.inputs() else { return None; };
    let (empty, error) = result_arguments_v1(tcx, *input)?;
    let output = signature.output();
    let (payload, converted_error) = result_arguments_v1(tcx, output)?;
    if instance.args[0].as_type()? != payload || instance.args[1].as_type()? != error
        || instance.args[2].as_type()? != converted_error
    {
        return None;
    }
    let TyKind::Adt(infallible, empty_arguments) = *empty.kind() else { return None; };
    if infallible.did().krate != instance.def_id().krate
        || tcx.def_path(infallible.did()).to_string_no_crate_verbose() != "::convert::Infallible"
        || !infallible.is_enum() || !infallible.variants().is_empty() || !empty_arguments.is_empty()
    {
        return None;
    }
    Some(ResidualTypesV1 { input: *input, output, error, converted_error })
}

fn conversion_call_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>, types: &ResidualTypesV1<'tcx>,
) -> bool {
    let TerminatorKind::Call { func, args, destination, target, unwind, .. } =
        &body.basic_blocks[BasicBlock::from_u32(0)].terminator().kind
    else { return false; };
    let Operand::Constant(function) = func else { return false; };
    let TyKind::FnDef(def_id, raw_arguments) = *function.const_.ty().kind() else { return false; };
    if def_id.krate != instance.def_id().krate
        || tcx.get_diagnostic_item(Symbol::intern("from_fn")) != Some(def_id)
        || tcx.get_diagnostic_item(Symbol::intern("From")).is_none()
        || tcx.trait_of_assoc(def_id) != tcx.get_diagnostic_item(Symbol::intern("From"))
        || !matches!(&args[..], [argument] if matches!(&argument.node, Operand::Move(place) if plain_local_v1(place, 3)))
        || !plain_local_v1(destination, 4)
        || *target != Some(BasicBlock::from_u32(1))
        || !matches!(unwind, UnwindAction::Unreachable)
    {
        return false;
    }
    let Ok(arguments) = instance.try_instantiate_mir_and_normalize_erasing_regions(
        tcx, TypingEnv::fully_monomorphized(), EarlyBinder::bind(raw_arguments),
    ) else { return false; };
    if arguments.len() != 2 || arguments[0].as_type() != Some(types.converted_error)
        || arguments[1].as_type() != Some(types.error)
    {
        return false;
    }
    let signature = tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(def_id).instantiate(tcx, arguments));
    let Ok(signature) = tcx.try_normalize_erasing_regions(TypingEnv::fully_monomorphized(), signature)
    else { return false; };
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic
        || signature.inputs() != [types.error] || signature.output() != types.converted_error
    {
        return false;
    }
    // Resolve the actual conversion, never replace it. Its body and source
    // origin remain independently visited by the existing collector.
    matches!(Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), def_id, arguments),
        Ok(Some(resolved)) if matches!(resolved.def, InstanceKind::Item(_))
            && resolved.args.iter().all(|arg| !arg.has_param() && !arg.has_escaping_bound_vars()))
}

fn residual_body_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>, body: &Body<'tcx>) -> bool {
    let Some(types) = residual_signature_v1(tcx, instance) else { return false; };
    if body.arg_count != 1 || body.local_decls.len() != 6
        || body.basic_blocks.len() != 2 || body.source_scopes.len() != 2
    {
        return false;
    }
    for (local, expected) in body.local_decls.iter().zip([
        types.output, types.input, tcx.types.isize, types.error, types.converted_error, tcx.types.bool,
    ]) {
        if normalize_type_v1(tcx, instance, local.ty) != Some(expected)
            || local.source_info.scope != SourceScope::from_u32(0)
        {
            return false;
        }
    }
    for (index, scope) in body.source_scopes.iter().enumerate() {
        if scope.parent_scope != (index != 0).then_some(SourceScope::from_u32(0))
            || scope.inlined.is_some() || scope.inlined_parent_scope.is_some()
            || !matches!(&scope.local_data, ClearCrossCrate::Clear)
        {
            return false;
        }
    }
    for (block_index, (block, scopes)) in body.basic_blocks.iter().zip([&[0, 0, 0, 0, 1][..], &[1, 1][..]]).enumerate() {
        if block.is_cleanup || block.statements.len() != scopes.len()
            || block.statements.iter().zip(scopes).any(|(statement, scope)| statement.source_info.scope != SourceScope::from_u32(*scope))
            || block.terminator.as_ref().is_none_or(|term| term.source_info.scope != SourceScope::from_u32(if block_index == 0 { 1 } else { 0 }))
        {
            return false;
        }
    }
    let entry = &body.basic_blocks[BasicBlock::from_u32(0)];
    let exit = &body.basic_blocks[BasicBlock::from_u32(1)];
    let StatementKind::Assign(discriminant) = &entry.statements[0].kind else { return false; };
    if !plain_local_v1(&discriminant.0, 2)
        || !matches!(&discriminant.1, Rvalue::Discriminant(input) if plain_local_v1(input, 1))
    {
        return false;
    }
    let StatementKind::Assign(comparison) = &entry.statements[1].kind else { return false; };
    let Rvalue::BinaryOp(BinOp::Eq, operands) = &comparison.1 else { return false; };
    let (Operand::Copy(discriminant), Operand::Constant(one)) = &**operands else { return false; };
    if !plain_local_v1(&comparison.0, 5) || !plain_local_v1(discriminant, 2)
        || one.const_.ty() != tcx.types.isize
        || one.const_.try_eval_bits(tcx, TypingEnv::fully_monomorphized()) != Some(1)
    {
        return false;
    }
    let StatementKind::Intrinsic(intrinsic) = &entry.statements[2].kind else { return false; };
    let Some(err) = tcx.lang_items().result_err_variant() else { return false; };
    matches!(&**intrinsic, NonDivergingIntrinsic::Assume(Operand::Move(place)) if plain_local_v1(place, 5))
        && move_variant_field_v1(tcx, instance, &entry.statements[3], 3, 1, types.error)
        && matches!(entry.statements[4].kind, StatementKind::StorageLive(local) if local.as_usize() == 4)
        && conversion_call_v1(tcx, instance, body, &types)
        && aggregate_v1(tcx, instance, &exit.statements[0], 0, types.output, err, 4, false)
        && matches!(exit.statements[1].kind, StatementKind::StorageDead(local) if local.as_usize() == 4)
        && matches!(exit.terminator().kind, TerminatorKind::Return)
}

pub(crate) fn authenticate_reviewed_safe_core_result_residual_v1<'tcx>(
    tcx: TyCtxt<'tcx>, instance: Instance<'tcx>,
) -> bool {
    let Some(types) = residual_signature_v1(tcx, instance) else { return false; };
    let Some(output) = result_family_v1(tcx, types.output) else { return false; };
    let Some(input_error) = error_family_v1(tcx, types.error) else { return false; };
    (input_error == output.error
        || matches!((input_error, output.error),
            (ResultErrorFamilyV1::Matrix | ResultErrorFamilyV1::Strided, ResultErrorFamilyV1::Kernel)))
        && residual_body_v1(tcx, instance, tcx.instance_mir(instance.def))
}

#[cfg(test)]
#[path = "core_result_control_v1_tests.rs"]
mod tests;

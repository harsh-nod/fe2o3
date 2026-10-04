//! Exact live physical-entry marker observations. Const bytes alone are inert.
use crate::trusted_device_items::{self, TrustedDeviceItem};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCompilerIntrinsicOperationV1 as Operation, SemanticPhysicalEntryInstructionV37,
};
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Body, Operand};
use rustc_middle::ty::{
    ConstKind, EarlyBinder, FnSig, GenericArgKind, GenericParamDefKind, Instance, TyCtxt, TyKind,
    TypingEnv, UintTy,
};

#[derive(Clone, Copy)]
pub(crate) struct ActualPhysicalEntryCallV37<'tcx> {
    instance: Instance<'tcx>,
    operation: Operation,
    argument_locals: [u32; 5],
    moved_arguments: u8,
}
impl<'tcx> ActualPhysicalEntryCallV37<'tcx> {
    pub(crate) fn instance(self) -> Instance<'tcx> {
        self.instance
    }
    pub(crate) fn operation(self) -> Operation {
        self.operation
    }
    pub(crate) fn argument_locals(self) -> [u32; 5] {
        self.argument_locals
    }
    pub(crate) fn moved_arguments(self) -> u8 {
        self.moved_arguments
    }
}

pub(crate) const fn is_physical(
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
) -> bool {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as E;
    matches!(
        expansion,
        E::Gfx942PhysicalEntryBegin | E::Gfx942PhysicalEntryLabel | E::Gfx942PhysicalEntryStep
    )
}

/// Exact trusted definition plus declared/actual const kind, order and width.
pub(crate) fn parse_operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Operation, &'static str> {
    let item = trusted_device_items::classify(tcx, instance.def_id())
        .ok_or("physical-entry callee has no reviewed provider identity")?;
    let count = match item {
        TrustedDeviceItem::AmdGpuPhysicalEntryBeginGfx942 => 0,
        TrustedDeviceItem::AmdGpuPhysicalEntryLabelGfx942 => 1,
        TrustedDeviceItem::AmdGpuPhysicalEntryStepGfx942 => 5,
        _ => return Err("physical-entry callee is not a physical-entry provider"),
    };
    let declaration = tcx.generics_of(instance.def_id());
    if declaration.parent.is_some()
        || declaration.parent_count != 0
        || declaration.own_params.len() != count
        || instance.args.len() != count
    {
        return Err("physical-entry marker has unexpected inherited or actual generic arguments");
    }
    let mut values = [0u32; 5];
    for (index, (parameter, argument)) in declaration
        .own_params
        .iter()
        .zip(instance.args.iter())
        .enumerate()
    {
        if parameter.index != index as u32
            || !matches!(parameter.kind, GenericParamDefKind::Const { .. })
        {
            return Err("physical-entry marker const declaration order differs");
        }
        let expected = if index == 4 { UintTy::U32 } else { UintTy::U8 };
        let declared = tcx.type_of(parameter.def_id).instantiate_identity();
        if !matches!(declared.kind(),TyKind::Uint(kind) if *kind==expected) {
            return Err("physical-entry marker declared const type differs");
        }
        let GenericArgKind::Const(value) = argument.kind() else {
            return Err("physical-entry marker actual generic is not a const");
        };
        let ConstKind::Value(evaluated) = value.kind() else {
            return Err("physical-entry marker const is unresolved");
        };
        if !matches!(evaluated.ty.kind(),TyKind::Uint(kind) if *kind==expected) {
            return Err("physical-entry marker actual const type differs");
        }
        let leaf = evaluated
            .valtree
            .try_to_leaf()
            .ok_or("physical-entry marker const is not a primitive leaf")?;
        if leaf.size().bytes() != if index == 4 { 4 } else { 1 } {
            return Err("physical-entry marker const width differs");
        }
        values[index] = u32::try_from(leaf.to_bits(leaf.size()))
            .map_err(|_| "physical-entry marker const exceeds exact width")?;
    }
    match count {
        0 => Ok(Operation::Gfx942PhysicalEntryBegin),
        1 => Ok(Operation::Gfx942PhysicalEntryLabel(values[0] as u8)),
        5 => Ok(Operation::Gfx942PhysicalEntryStep(
            SemanticPhysicalEntryInstructionV37::new(
                values[0] as u8,
                values[1] as u8,
                values[2] as u8,
                values[3] as u8,
                values[4],
            )
            .map_err(|_| "physical-entry instruction descriptor is outside the closed profile")?,
        )),
        _ => unreachable!("closed marker generic counts"),
    }
}

pub(crate) fn valid_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    signature: &FnSig<'tcx>,
    operation: Operation,
) -> bool {
    if matches!(operation, Operation::Gfx942PhysicalEntryBegin) {
        return crate::production_complete_body_call_vnext::valid_root_signature(tcx, signature);
    }
    matches!(
        operation,
        Operation::Gfx942PhysicalEntryLabel(_) | Operation::Gfx942PhysicalEntryStep(_)
    ) && signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().is_empty()
        && signature.output().is_unit()
}

pub(crate) fn observe<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    func: &Operand<'tcx>,
    operands: &[&Operand<'tcx>],
) -> Result<ActualPhysicalEntryCallV37<'tcx>, &'static str> {
    if !matches!(func, Operand::Constant(_)) {
        return Err("physical-entry callee must be an actual constant function");
    }
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(func.ty(body, tcx)),
        )
        .map_err(|_| "physical-entry callee normalization failed")?;
    let TyKind::FnDef(definition, args) = callable.kind() else {
        return Err("physical-entry callee is not a direct function");
    };
    let instance = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
        .map_err(|_| "physical-entry callee resolution failed")?
        .ok_or("physical-entry callee Instance is absent")?;
    let operation = parse_operation(tcx, instance)?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if !valid_signature(tcx, &signature, operation) || operands.len() != signature.inputs().len() {
        return Err("physical-entry marker source signature or runtime arity differs");
    }
    let mut argument_locals = [0; 5];
    let mut moved_arguments = 0;
    for (index, operand) in operands.iter().enumerate() {
        if operand.ty(body, tcx) != signature.inputs()[index] {
            return Err("physical-entry marker operand type differs");
        }
        let (place, moved) = match operand {
            Operand::Copy(place) => (place, false),
            Operand::Move(place) => (place, true),
            _ => return Err("physical-entry begin requires exact root argument transport"),
        };
        if !place.projection.is_empty() || (index == 0 && !moved) {
            return Err("physical-entry begin cannot project or copy its output owner");
        }
        argument_locals[index] = place.local.as_u32();
        moved_arguments |= u8::from(moved) << index;
    }
    Ok(ActualPhysicalEntryCallV37 {
        instance,
        operation,
        argument_locals,
        moved_arguments,
    })
}

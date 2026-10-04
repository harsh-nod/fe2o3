//! Exact live physical-global-copy marker observations. Const bytes alone are inert.
use crate::trusted_device_items::{self, TrustedDeviceItem};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCompilerIntrinsicOperationV1 as Operation, SemanticPhysicalGlobalCopyInstructionV38,
};
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Body, Operand};
use rustc_middle::ty::{
    ConstKind, EarlyBinder, FnSig, GenericArgKind, GenericParamDefKind, Instance, TyCtxt, TyKind,
    TypingEnv, UintTy,
};

#[derive(Clone, Copy)]
pub(crate) struct ActualPhysicalGlobalCopyCallV38<'tcx> {
    instance: Instance<'tcx>,
    operation: Operation,
    argument_locals: [u32; 2],
    moved_arguments: u8,
}
impl<'tcx> ActualPhysicalGlobalCopyCallV38<'tcx> {
    pub(crate) fn instance(self) -> Instance<'tcx> {
        self.instance
    }
    pub(crate) fn operation(self) -> Operation {
        self.operation
    }
    pub(crate) fn argument_locals(self) -> [u32; 2] {
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
        E::Gfx942PhysicalGlobalCopyBegin
            | E::Gfx942PhysicalGlobalCopyLabel
            | E::Gfx942PhysicalGlobalCopyStep
    )
}

/// Exact trusted definition plus declared/actual const kind, order and width.
pub(crate) fn parse_operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Operation, &'static str> {
    let item = trusted_device_items::classify(tcx, instance.def_id())
        .ok_or("physical-global-copy callee has no reviewed provider identity")?;
    let count = match item {
        TrustedDeviceItem::AmdGpuPhysicalGlobalCopyBeginGfx942 => 0,
        TrustedDeviceItem::AmdGpuPhysicalGlobalCopyLabelGfx942 => 1,
        TrustedDeviceItem::AmdGpuPhysicalGlobalCopyStepGfx942 => 5,
        _ => return Err("physical-global-copy callee is not a physical-global-copy provider"),
    };
    let declaration = tcx.generics_of(instance.def_id());
    if declaration.parent.is_some()
        || declaration.parent_count != 0
        || declaration.own_params.len() != count
        || instance.args.len() != count
    {
        return Err(
            "physical-global-copy marker has unexpected inherited or actual generic arguments",
        );
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
            return Err("physical-global-copy marker const declaration order differs");
        }
        let expected = if index == 4 { UintTy::U32 } else { UintTy::U8 };
        let declared = tcx.type_of(parameter.def_id).instantiate_identity();
        if !matches!(declared.kind(),TyKind::Uint(kind) if *kind==expected) {
            return Err("physical-global-copy marker declared const type differs");
        }
        let GenericArgKind::Const(value) = argument.kind() else {
            return Err("physical-global-copy marker actual generic is not a const");
        };
        let ConstKind::Value(evaluated) = value.kind() else {
            return Err("physical-global-copy marker const is unresolved");
        };
        if !matches!(evaluated.ty.kind(),TyKind::Uint(kind) if *kind==expected) {
            return Err("physical-global-copy marker actual const type differs");
        }
        let leaf = evaluated
            .valtree
            .try_to_leaf()
            .ok_or("physical-global-copy marker const is not a primitive leaf")?;
        if leaf.size().bytes() != if index == 4 { 4 } else { 1 } {
            return Err("physical-global-copy marker const width differs");
        }
        values[index] = u32::try_from(leaf.to_bits(leaf.size()))
            .map_err(|_| "physical-global-copy marker const exceeds exact width")?;
    }
    match count {
        0 => Ok(Operation::Gfx942PhysicalGlobalCopyBegin),
        1 if values[0] == 0 => Ok(Operation::Gfx942PhysicalGlobalCopyLabel(0)),
        1 => Err("physical-global-copy supports only label zero"),
        5 => Ok(Operation::Gfx942PhysicalGlobalCopyStep(
            SemanticPhysicalGlobalCopyInstructionV38::new(
                values[0] as u8,
                values[1] as u8,
                values[2] as u8,
                values[3] as u8,
                values[4],
            )
            .map_err(
                |_| "physical-global-copy instruction descriptor is outside the closed profile",
            )?,
        )),
        _ => unreachable!("closed marker generic counts"),
    }
}

/// Both source types are checked from the actual rustc root/callee signature.
/// The shared input's element and the trusted nominal output are exact u32.
pub(crate) fn valid_root_signature<'tcx>(tcx: TyCtxt<'tcx>, signature: &FnSig<'tcx>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().len() == 2
        && signature.output().is_unit()
        && matches!(signature.inputs()[0].kind(), TyKind::Ref(_, pointee, rustc_hir::Mutability::Not)
            if matches!(pointee.kind(), TyKind::Slice(element)
                if matches!(element.kind(), TyKind::Uint(UintTy::U32))))
        && crate::production_complete_body_call_vnext::exact_output_type(tcx, signature.inputs()[1])
}

pub(crate) fn valid_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    signature: &FnSig<'tcx>,
    operation: Operation,
) -> bool {
    if matches!(operation, Operation::Gfx942PhysicalGlobalCopyBegin) {
        return valid_root_signature(tcx, signature);
    }
    matches!(
        operation,
        Operation::Gfx942PhysicalGlobalCopyLabel(_) | Operation::Gfx942PhysicalGlobalCopyStep(_)
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
) -> Result<ActualPhysicalGlobalCopyCallV38<'tcx>, &'static str> {
    if !matches!(func, Operand::Constant(_)) {
        return Err("physical-global-copy callee must be an actual constant function");
    }
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(func.ty(body, tcx)),
        )
        .map_err(|_| "physical-global-copy callee normalization failed")?;
    let TyKind::FnDef(definition, args) = callable.kind() else {
        return Err("physical-global-copy callee is not a direct function");
    };
    let instance = Instance::try_resolve(tcx, TypingEnv::fully_monomorphized(), *definition, args)
        .map_err(|_| "physical-global-copy callee resolution failed")?
        .ok_or("physical-global-copy callee Instance is absent")?;
    let operation = parse_operation(tcx, instance)?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if !valid_signature(tcx, &signature, operation) || operands.len() != signature.inputs().len() {
        return Err("physical-global-copy marker source signature or runtime arity differs");
    }
    let mut argument_locals = [0; 2];
    let mut moved_arguments = 0;
    for (index, operand) in operands.iter().enumerate() {
        if operand.ty(body, tcx) != signature.inputs()[index] {
            return Err("physical-global-copy marker operand type differs");
        }
        let (place, moved) = match operand {
            Operand::Copy(place) => (place, false),
            Operand::Move(place) => (place, true),
            _ => return Err("physical-global-copy begin requires exact root argument transport"),
        };
        if !place.projection.is_empty() || (index == 1 && !moved) {
            return Err("physical-global-copy begin cannot project or copy its output owner");
        }
        argument_locals[index] = place.local.as_u32();
        moved_arguments |= u8::from(moved) << index;
    }
    Ok(ActualPhysicalGlobalCopyCallV38 {
        instance,
        operation,
        argument_locals,
        moved_arguments,
    })
}

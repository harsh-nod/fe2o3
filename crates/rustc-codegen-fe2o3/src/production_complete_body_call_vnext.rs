//! Exact live complete-body source call observer for registered terminal144.
//! Actual retained calls select the independent MIR36/CombinedV6 profile.
//! Parsing alone creates no semantic/canonical/proof/publication owner.

use crate::trusted_device_items::{self, TrustedDeviceItem};
use fe2o3_kernel_ir::Gfx942CompleteBodyPackedV1;
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Body, Operand};
use rustc_middle::ty::{
    ConstKind, EarlyBinder, FnSig, GenericArgKind, GenericParamDefKind, Instance, Ty, TyCtxt,
    TyKind, TypingEnv, UintTy,
};

/// Real normalized call observation. Private fields; no transport constructor.
pub(crate) struct ActualCompleteBodyCallVNext<'tcx> {
    instance: Instance<'tcx>,
    packed: Gfx942CompleteBodyPackedV1,
    registers: [u8; 5],
    argument_locals: [u32; 5],
    moved_arguments: u8,
}
impl<'tcx> ActualCompleteBodyCallVNext<'tcx> {
    pub(crate) fn instance(&self) -> Instance<'tcx> {
        self.instance
    }
    pub(crate) fn packed(&self) -> Gfx942CompleteBodyPackedV1 {
        self.packed
    }
    pub(crate) fn registers(&self) -> [u8; 5] {
        self.registers
    }
    pub(crate) fn argument_locals(&self) -> [u32; 5] {
        self.argument_locals
    }
    pub(crate) fn moved_arguments(&self) -> u8 {
        self.moved_arguments
    }
}

/// EXACT declared and actual generic kind/order/type/width; no ignored entries.
/// The rustc query machinery is outside this helper's fixed local data bound.
pub(crate) fn parse_complete_body_consts<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Gfx942CompleteBodyPackedV1, &'static str> {
    let declaration = tcx.generics_of(instance.def_id());
    if declaration.parent.is_some()
        || declaration.parent_count != 0
        || declaration.own_params.len() != 10
        || instance.args.len() != 10
    {
        return Err("complete body requires exactly ten noninherited const generics");
    }
    let mut values = [0_u64; 10];
    for (index, (parameter, argument)) in declaration
        .own_params
        .iter()
        .zip(instance.args.iter())
        .enumerate()
    {
        if parameter.index != index as u32
            || !matches!(parameter.kind, GenericParamDefKind::Const { .. })
        {
            return Err("complete body declaration is not ten ordered consts");
        }
        let expected = if index < 2 { UintTy::U8 } else { UintTy::U64 };
        let declared = tcx.type_of(parameter.def_id).instantiate_identity();
        if !matches!(declared.kind(), TyKind::Uint(kind) if *kind == expected) {
            return Err("complete body declared const types must be two u8 then eight u64");
        }
        let GenericArgKind::Const(value) = argument.kind() else {
            return Err("complete body actual generic is not a const");
        };
        let ConstKind::Value(evaluated) = value.kind() else {
            return Err("complete body actual const is unresolved");
        };
        if !matches!(evaluated.ty.kind(), TyKind::Uint(kind) if *kind == expected) {
            return Err("complete body actual const has wrong exact type");
        }
        let leaf = evaluated
            .valtree
            .try_to_leaf()
            .ok_or("complete body actual const is not a primitive leaf")?;
        if leaf.size().bytes() != if index < 2 { 1 } else { 8 } {
            return Err("complete body actual const has wrong primitive width");
        }
        values[index] = u64::try_from(leaf.to_bits(leaf.size()))
            .map_err(|_| "complete body actual const exceeds primitive width")?;
    }
    packed_from_values(values)
}

fn packed_from_values(values: [u64; 10]) -> Result<Gfx942CompleteBodyPackedV1, &'static str> {
    let blocks = u8::try_from(values[0]).map_err(|_| "complete body block count is not u8")?;
    let steps = u8::try_from(values[1]).map_err(|_| "complete body step count is not u8")?;
    Gfx942CompleteBodyPackedV1::from_words(
        blocks,
        steps,
        [values[2], values[3], values[4], values[5]],
        [values[6], values[7], values[8], values[9]],
    )
    .map_err(|_| "complete body packed grammar, counts or padding differ")
}

pub(crate) fn valid_marker_signature<'tcx>(tcx: TyCtxt<'tcx>, signature: &FnSig<'tcx>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().len() == 10
        && signature.output().is_unit()
        && signature.inputs().iter().enumerate().all(|(index, ty)| {
            if index == 0 {
                exact_output_type(tcx, *ty)
            } else {
                matches!(ty.kind(), TyKind::Uint(kind)
                    if *kind == if index < 5 { UintTy::U32 } else { UintTy::U8 })
            }
        })
}

pub(crate) fn valid_root_signature<'tcx>(tcx: TyCtxt<'tcx>, signature: &FnSig<'tcx>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().len() == 5
        && signature.output().is_unit()
        && exact_output_type(tcx, signature.inputs()[0])
        && signature.inputs()[1..]
            .iter()
            .all(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
}

fn exact_output_type<'tcx>(tcx: TyCtxt<'tcx>, ty: Ty<'tcx>) -> bool {
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return false;
    };
    if trusted_device_items::classify(tcx, definition.did())
        != Some(TrustedDeviceItem::DisjointSlice)
        || arguments.len() != 2
        || !arguments[0]
            .as_type()
            .is_some_and(|ty| matches!(ty.kind(), TyKind::Uint(UintTy::U32)))
    {
        return false;
    }
    // Same definition-based Index1D derivation as the normal importer. Index1D
    // itself has no diagnostic item; do not authenticate it by display path.
    let Some(function) = trusted_device_items::definition(tcx, TrustedDeviceItem::ThreadIndex1d)
    else {
        return false;
    };
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(function).instantiate_identity());
    let TyKind::Adt(index_definition, index_arguments) = *signature.output().kind() else {
        return false;
    };
    trusted_device_items::classify(tcx, index_definition.did())
        == Some(TrustedDeviceItem::ThreadIndex)
        && index_arguments.len() == 1
        && index_arguments[0].as_type().is_some()
        && arguments[1].as_type() == index_arguments[0].as_type()
}

pub(crate) fn observe_complete_body_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    func: &Operand<'tcx>,
    operands: [&Operand<'tcx>; 10],
) -> Result<ActualCompleteBodyCallVNext<'tcx>, &'static str> {
    if !matches!(func, Operand::Constant(_)) {
        return Err("complete body callee is not an actual constant function");
    }
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(func.ty(body, tcx)),
        )
        .map_err(|_| "complete body callee normalization failed")?;
    let TyKind::FnDef(definition, generic_args) = callable.kind() else {
        return Err("complete body callee is not a direct function");
    };
    let instance = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        generic_args,
    )
    .map_err(|_| "complete body callee resolution failed")?
    .ok_or("complete body callee Instance is absent")?;
    // Symbolic new registry variant, deliberately not added to shared registry.
    if trusted_device_items::classify(tcx, instance.def_id())
        != Some(TrustedDeviceItem::AmdGpuCompleteBodyE32)
    {
        return Err("complete body callee is not the reviewed new provider");
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if !valid_marker_signature(tcx, &signature) {
        return Err("complete body callee signature differs");
    }
    let packed = parse_complete_body_consts(tcx, instance)?;
    let mut argument_locals = [0; 5];
    let mut moved_arguments = 0_u8;
    for (index, operand) in operands[..5].iter().enumerate() {
        let ty = operand.ty(body, tcx);
        if ty != signature.inputs()[index] {
            return Err("complete body runtime operand type differs");
        }
        let (place, moved) = match operand {
            Operand::Copy(place) => (place, false),
            Operand::Move(place) => (place, true),
            _ => return Err("complete body data must be direct root-argument transport"),
        };
        if !place.projection.is_empty() {
            return Err("complete body runtime argument is projected");
        }
        if index == 0 && !moved {
            return Err("complete body output owner must be moved into the marker");
        }
        argument_locals[index] = place.local.as_u32();
        moved_arguments |= u8::from(moved) << index;
    }
    let mut literals = [None; 5];
    for (index, operand) in operands[5..].iter().enumerate() {
        let Operand::Constant(value) = operand else {
            return Err("complete body physical role is not an actual MIR literal");
        };
        if !matches!(value.const_.ty().kind(), TyKind::Uint(UintTy::U8)) {
            return Err("complete body physical literal is not exact u8");
        }
        literals[index] = value
            .const_
            .try_eval_bits(tcx, TypingEnv::fully_monomorphized());
    }
    let registers = registers_from_literals(literals)?;
    Ok(ActualCompleteBodyCallVNext {
        instance,
        packed,
        registers,
        argument_locals,
        moved_arguments,
    })
}

fn registers_from_literals(literals: [Option<u128>; 5]) -> Result<[u8; 5], &'static str> {
    let mut registers = [0; 5];
    let mut used = 0_u64;
    for (slot, literal) in literals.into_iter().enumerate() {
        let register = literal
            .and_then(|value| u8::try_from(value).ok())
            .ok_or("complete body physical role cannot be evaluated as u8")?;
        if !(8..=63).contains(&register) || used & (1_u64 << register) != 0 {
            return Err("complete body roles require distinct v8..v63 outside the reserved prefix");
        }
        used |= 1_u64 << register;
        registers[slot] = register;
    }
    Ok(registers)
}

#[cfg(test)]
#[path = "production_complete_body_call_vnext_tests.rs"]
mod tests;

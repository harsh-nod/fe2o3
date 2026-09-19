//! Exact typed-constant source contract for CombinedV5 terminal134.
//!
//! Parsing a program is not marker authentication. Callers first retain the
//! trusted terminal and independently replay its actual caller MIR/Instance.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCanonAbiV1, SemanticExternAbiV1, SemanticFunctionAbiV1,
    SemanticGfx942OrderedProgramRegistersV32, SemanticGfx942U32ProgramV32, SemanticScalarTypeV1,
    SemanticTypeDeclV1, SemanticTypeIdV1, SemanticTypeShapeV1,
};
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Body, Operand};
use rustc_middle::ty::{
    ConstKind, EarlyBinder, FnSig, GenericArgKind, GenericParamDefKind, Instance, TyCtxt, TyKind,
    TypingEnv, UintTy,
};

pub(crate) use crate::production_ordered_region_v31::MAX_PROFILE_BLOCKS;

/// Strictly parses all five arguments, without dropping non-const or unresolved
/// entries. The caller provides a fully normalized actual Instance. Rustc query
/// allocations are not covered by this helper's fixed local array bound.
pub(crate) fn parse_program_consts<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<SemanticGfx942U32ProgramV32, &'static str> {
    let declaration = tcx.generics_of(instance.def_id());
    if declaration.parent.is_some()
        || declaration.parent_count != 0
        || declaration.own_params.len() != 5
        || instance.args.len() != 5
    {
        return Err("ordered program requires exactly five noninherited const generics");
    }
    let mut values = [0_u64; 5];
    for (index, (parameter, argument)) in declaration
        .own_params
        .iter()
        .zip(instance.args.iter())
        .enumerate()
    {
        if parameter.index != index as u32
            || !matches!(parameter.kind, GenericParamDefKind::Const { .. })
        {
            return Err("ordered program generic declaration is not five ordered consts");
        }
        let expected = if index == 0 { UintTy::U8 } else { UintTy::U64 };
        let declared_ty = tcx.type_of(parameter.def_id).instantiate_identity();
        if !matches!(declared_ty.kind(), TyKind::Uint(kind) if *kind == expected) {
            return Err("ordered program declared const types must be u8 then four u64");
        }
        let GenericArgKind::Const(value) = argument.kind() else {
            return Err("ordered program actual generic argument is not a const");
        };
        let ConstKind::Value(evaluated) = value.kind() else {
            return Err("ordered program actual const did not normalize to a value");
        };
        if !matches!(evaluated.ty.kind(), TyKind::Uint(kind) if *kind == expected) {
            return Err("ordered program actual const has the wrong exact primitive type");
        }
        let leaf = evaluated
            .valtree
            .try_to_leaf()
            .ok_or("ordered program actual const is not a primitive leaf")?;
        if leaf.size().bytes() != if index == 0 { 1 } else { 8 } {
            return Err("ordered program actual const has the wrong primitive width");
        }
        values[index] = u64::try_from(leaf.to_bits(leaf.size()))
            .map_err(|_| "ordered program actual const exceeds its primitive width")?;
    }
    program_from_values(values)
}

fn program_from_values(values: [u64; 5]) -> Result<SemanticGfx942U32ProgramV32, &'static str> {
    let count =
        u8::try_from(values[0]).map_err(|_| "ordered program instruction count is not exact u8")?;
    SemanticGfx942U32ProgramV32::from_packed(count, [values[1], values[2], values[3], values[4]])
        .map_err(|_| "ordered program descriptors, padding or definite initialization are invalid")
}

pub(crate) fn valid_signature<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    signature: &FnSig<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> bool {
    valid_rust_signature(signature)
        && parse_program_consts(tcx, instance).is_ok()
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.c_variadic()
        && !abi.can_unwind()
        && abi.source_input_types().len() == 8
        && abi
            .source_input_types()
            .iter()
            .enumerate()
            .all(|(index, ty)| unsigned_type(types, *ty, if index < 3 { 32 } else { 8 }))
        && unsigned_type(types, abi.source_output_type(), 32)
}

fn valid_rust_signature(signature: &FnSig<'_>) -> bool {
    signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && signature.inputs().len() == 8
        && signature.inputs().iter().enumerate().all(|(index, ty)| {
            matches!(ty.kind(), TyKind::Uint(kind)
                if *kind == if index < 3 { UintTy::U32 } else { UintTy::U8 })
        })
        && matches!(signature.output().kind(), TyKind::Uint(UintTy::U32))
}

fn unsigned_type(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1, bits: u16) -> bool {
    types.get(ty.index() as usize).is_some_and(|declaration| {
        matches!(declaration.shape(), SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Integer { signed: false, bits: actual }) if *actual == bits)
    })
}

/// An inert observation made from a real MIR call, not a public constructor or
/// a transport-reconstructed source owner. The full Instance is kept for replay.
pub(crate) struct ActualOrderedProgramCallV32<'tcx> {
    instance: Instance<'tcx>,
    program: SemanticGfx942U32ProgramV32,
    registers: SemanticGfx942OrderedProgramRegistersV32,
}

impl<'tcx> ActualOrderedProgramCallV32<'tcx> {
    pub(crate) fn instance(&self) -> Instance<'tcx> {
        self.instance
    }

    pub(crate) fn program(&self) -> SemanticGfx942U32ProgramV32 {
        self.program
    }

    pub(crate) fn registers(&self) -> SemanticGfx942OrderedProgramRegistersV32 {
        self.registers
    }
}

pub(crate) fn observe_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    caller: Instance<'tcx>,
    body: &Body<'tcx>,
    func: &Operand<'tcx>,
    operands: [&Operand<'tcx>; 8],
) -> Result<ActualOrderedProgramCallV32<'tcx>, &'static str> {
    if !matches!(func, Operand::Constant(_)) {
        return Err("ordered program callee is not an actual constant function");
    }
    let callable = caller
        .try_instantiate_mir_and_normalize_erasing_regions(
            tcx,
            TypingEnv::fully_monomorphized(),
            EarlyBinder::bind(func.ty(body, tcx)),
        )
        .map_err(|_| "ordered program actual callee normalization failed")?;
    let TyKind::FnDef(definition, generic_args) = callable.kind() else {
        return Err("ordered program callee is not a direct function");
    };
    let instance = Instance::try_resolve(
        tcx,
        TypingEnv::fully_monomorphized(),
        *definition,
        generic_args,
    )
    .map_err(|_| "ordered program actual callee resolution failed")?
    .ok_or("ordered program actual callee Instance is absent")?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if !valid_rust_signature(&signature) {
        return Err("ordered program actual callee signature is not admitted");
    }
    let program = parse_program_consts(tcx, instance)?;
    for (index, operand) in operands.iter().enumerate() {
        let expected = if index < 3 { UintTy::U32 } else { UintTy::U8 };
        if !matches!(operand.ty(body, tcx).kind(), TyKind::Uint(kind) if *kind == expected) {
            return Err("ordered program actual runtime operand has the wrong exact type");
        }
        if index < 3
            && matches!(operand, Operand::Copy(place) | Operand::Move(place) if !place.projection.is_empty())
        {
            return Err("ordered program data inputs must be direct scalar locals or constants");
        }
    }
    let mut literals = [None; 5];
    for (index, operand) in operands[3..].iter().enumerate() {
        let Operand::Constant(constant) = operand else {
            return Err("ordered program physical role is not an actual MIR constant");
        };
        literals[index] = constant
            .const_
            .try_eval_bits(tcx, TypingEnv::fully_monomorphized());
    }
    let registers = registers_from_literals(literals)?;
    Ok(ActualOrderedProgramCallV32 {
        instance,
        program,
        registers,
    })
}

fn registers_from_literals(
    literals: [Option<u128>; 5],
) -> Result<SemanticGfx942OrderedProgramRegistersV32, &'static str> {
    let mut values = [0; 5];
    for (slot, value) in literals.into_iter().enumerate() {
        values[slot] = value
            .and_then(|value| u8::try_from(value).ok())
            .ok_or("ordered program physical constant cannot be evaluated as u8")?;
    }
    SemanticGfx942OrderedProgramRegistersV32::new(
        values[0],
        values[1],
        [values[2], values[3], values[4]],
    )
    .map_err(|_| "ordered program physical roles must be distinct v0..v63")
}

pub(crate) fn require_unconditional_single_execution(
    body: &Body<'_>,
    block: u32,
) -> Result<(), &'static str> {
    crate::production_ordered_region_v31::require_unconditional_single_execution(body, block)
        .map_err(|_| "ordered program requires bounded unconditional acyclic source placement")
}

#[cfg(test)]
#[path = "production_ordered_program_v32_tests.rs"]
mod tests;

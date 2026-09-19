//! Closed typed scalar saturation; this is not normalized atomic MIR.

use super::{ProductionRustcIntrinsicClassificationV1, ProductionRustcIntrinsicOperationV1};
use fe2o3_mir_model::semantic_mir_v1::SemanticSaturatingIntegerOpV1;
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::middle::codegen_fn_attrs::CodegenFnAttrFlags;
use rustc_middle::ty::{Instance, IntTy, Ty, TyCtxt, TyKind, UintTy};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SaturatingIntrinsicErrorV1 {
    GenericArity,
    ElementTypeArgument,
    UnsupportedIntegerType,
    Signature,
    Unwind,
    CallArity,
    InputType,
    ResultType,
}

impl fmt::Display for SaturatingIntrinsicErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::GenericArity => "saturating integer intrinsic with unexpected generic arity",
            Self::ElementTypeArgument => "saturating integer intrinsic without a type argument",
            Self::UnsupportedIntegerType => "saturating integer intrinsic outside the supported signed/unsigned 8/16/32/64-bit subset",
            Self::Signature => "saturating integer intrinsic without an exact safe nonvariadic Rust signature",
            Self::Unwind => "saturating integer intrinsic without rustc nounwind metadata",
            Self::CallArity => "saturating integer intrinsic with unexpected call arity",
            Self::InputType => "saturating integer intrinsic whose inputs differ from its type argument",
            Self::ResultType => "saturating integer intrinsic whose result differs from its type argument",
        })
    }
}

pub(crate) fn operation_v1(name: &str) -> Option<SemanticSaturatingIntegerOpV1> {
    match name {
        "saturating_add" => Some(SemanticSaturatingIntegerOpV1::Add),
        "saturating_sub" => Some(SemanticSaturatingIntegerOpV1::Subtract),
        _ => None,
    }
}

/// Exact scalar identity, with pointer-sized integers resolved by the target.
pub(crate) fn integer_shape_v1(ty: Ty<'_>, pointer_bits: u64) -> Option<(bool, u16)> {
    let (signed, bits) = match ty.kind() {
        TyKind::Int(integer) => (
            true,
            match integer {
                IntTy::I8 => 8,
                IntTy::I16 => 16,
                IntTy::I32 => 32,
                IntTy::I64 => 64,
                IntTy::Isize => pointer_bits,
                IntTy::I128 => return None,
            },
        ),
        TyKind::Uint(integer) => (
            false,
            match integer {
                UintTy::U8 => 8,
                UintTy::U16 => 16,
                UintTy::U32 => 32,
                UintTy::U64 => 64,
                UintTy::Usize => pointer_bits,
                UintTy::U128 => return None,
            },
        ),
        _ => return None,
    };
    supported_bits_v1(bits).then_some((signed, bits as u16))
}

fn supported_bits_v1(bits: u64) -> bool {
    matches!(bits, 8 | 16 | 32 | 64)
}

#[derive(Clone, Copy, Debug)]
struct SaturatingContractV1 {
    supported_integer: bool,
    safe: bool,
    rust_abi: bool,
    variadic: bool,
    nounwind: bool,
    input_count: usize,
    inputs_match: bool,
    result_matches: bool,
}

fn validate_contract_v1(contract: SaturatingContractV1) -> Result<(), SaturatingIntrinsicErrorV1> {
    if !contract.supported_integer {
        return Err(SaturatingIntrinsicErrorV1::UnsupportedIntegerType);
    }
    if !contract.safe || !contract.rust_abi || contract.variadic {
        return Err(SaturatingIntrinsicErrorV1::Signature);
    }
    if !contract.nounwind {
        return Err(SaturatingIntrinsicErrorV1::Unwind);
    }
    if contract.input_count != 2 {
        return Err(SaturatingIntrinsicErrorV1::CallArity);
    }
    if !contract.inputs_match {
        return Err(SaturatingIntrinsicErrorV1::InputType);
    }
    if !contract.result_matches {
        return Err(SaturatingIntrinsicErrorV1::ResultType);
    }
    Ok(())
}

/// The caller has already required `InstanceKind::Intrinsic` and rustc's
/// intrinsic metadata, then matched its exact name. Ordinary Items never enter.
pub(super) fn classify_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    operation: SemanticSaturatingIntegerOpV1,
) -> Result<ProductionRustcIntrinsicClassificationV1<'tcx>, SaturatingIntrinsicErrorV1> {
    let [argument] = instance.args.as_slice() else {
        return Err(SaturatingIntrinsicErrorV1::GenericArity);
    };
    let element_type = argument
        .as_type()
        .ok_or(SaturatingIntrinsicErrorV1::ElementTypeArgument)?;
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    validate_contract_v1(SaturatingContractV1 {
        supported_integer: integer_shape_v1(element_type, tcx.data_layout.pointer_size().bits())
            .is_some(),
        safe: signature.safety == Safety::Safe,
        rust_abi: signature.abi == ExternAbi::Rust,
        variadic: signature.c_variadic,
        nounwind: tcx
            .codegen_fn_attrs(instance.def_id())
            .flags
            .contains(CodegenFnAttrFlags::NEVER_UNWIND),
        input_count: signature.inputs().len(),
        inputs_match: signature.inputs().iter().all(|ty| *ty == element_type),
        result_matches: signature.output() == element_type,
    })?;
    Ok(ProductionRustcIntrinsicClassificationV1 {
        operation: ProductionRustcIntrinsicOperationV1::SaturatingInteger(operation),
        element_type,
    })
}

#[cfg(test)]
#[path = "production_rustc_saturating_integer_v1_tests.rs"]
mod tests;

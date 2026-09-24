//! Exact source provider ABI adapter for CombinedV7 terminals145..147.
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::ty::{FnSig, Instance, TyCtxt};

pub(crate) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    signature: &FnSig<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, &'static str> {
    let operation = crate::production_physical_entry_call_v37::parse_operation(tcx, instance)?;
    let count = if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin
    ) {
        5
    } else {
        0
    };
    if !crate::production_physical_entry_call_v37::valid_signature(tcx, signature, operation)
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || abi.can_unwind()
        || abi.fixed_count() as usize != count
        || abi.source_input_types().len() != count
        || abi.arguments().len() != count
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        || !types
            .get(abi.source_output_type().index() as usize)
            .is_some_and(|ty| matches!(ty.shape(), SemanticTypeShapeV1::Unit))
    {
        return Err("physical-entry marker source/ABI signature differs");
    }
    if count == 5
        && abi.source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ]
    {
        return Err("physical-entry begin ownership differs");
    }
    for (index, ty) in abi.source_input_types().iter().enumerate() {
        let declaration = types
            .get(ty.index() as usize)
            .ok_or("physical-entry marker ABI type absent")?;
        let argument = abi.arguments()[index].value();
        if argument.adjusted().is_some() || argument.pointee_override().is_some() {
            return Err("physical-entry adjusted marker ABI is unsupported");
        }
        if index == 0 {
            if !matches!(argument.mode(), SemanticAbiPassModeV1::Pair { .. })
                || declaration.layout().size_bytes() != Some(16)
                || declaration.layout().alignment_bytes() != 8
            {
                return Err("physical-entry output is not the exact pointer/length pair ABI");
            }
        } else if !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
            || !matches!(
                declaration.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            )
            || declaration.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
            || declaration.layout().size_bytes() != Some(4)
            || declaration.layout().alignment_bytes() != 4
        {
            return Err("physical-entry scalar marker input is not ordinary direct u32");
        }
    }
    Ok(operation)
}

//! Exact source provider ABI adapter for CombinedV8 terminals148..150.
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::ty::{FnSig, Instance, TyCtxt};

pub(crate) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    signature: &FnSig<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, &'static str> {
    let operation =
        crate::production_physical_global_copy_call_v38::parse_operation(tcx, instance)?;
    let count = if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalGlobalCopyBegin
    ) {
        2
    } else {
        0
    };
    if !crate::production_physical_global_copy_call_v38::valid_signature(tcx, signature, operation)
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
        return Err("physical-global-copy marker source/ABI signature differs");
    }
    if count == 2
        && abi.source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ]
    {
        return Err("physical-global-copy begin ownership differs");
    }
    for (index, ty) in abi.source_input_types().iter().enumerate() {
        let declaration = types
            .get(ty.index() as usize)
            .ok_or("physical-global-copy marker ABI type absent")?;
        let argument = abi.arguments()[index].value();
        if argument.adjusted().is_some() || argument.pointee_override().is_some() {
            return Err("physical-global-copy adjusted marker ABI is unsupported");
        }
        if !matches!(argument.mode(), SemanticAbiPassModeV1::Pair { .. })
            || declaration.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
            || declaration.layout().size_bytes() != Some(16)
            || declaration.layout().alignment_bytes() != 8
            || declaration.layout().is_uninhabited()
        {
            return Err("physical-global-copy input/output are not exact pointer/length pair ABIs");
        }
    }
    Ok(operation)
}

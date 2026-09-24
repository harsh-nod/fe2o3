//! Exact source provider ABI adapter for CombinedV9 terminals151..153.
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
        crate::production_physical_lds_exchange_call_v39::parse_operation(tcx, instance)?;
    let count = if matches!(
        operation,
        SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeBegin(_)
    ) {
        2
    } else {
        0
    };
    if !crate::production_physical_lds_exchange_call_v39::valid_signature(tcx, signature, operation)
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
        return Err("physical-lds-exchange marker source/ABI signature differs");
    }
    if count == 2
        && abi.source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ]
    {
        return Err("physical-lds-exchange begin ownership differs");
    }
    for (index, ty) in abi.source_input_types().iter().enumerate() {
        let declaration = types
            .get(ty.index() as usize)
            .ok_or("physical-lds-exchange marker ABI type absent")?;
        let argument = abi.arguments()[index].value();
        if argument.adjusted().is_some() || argument.pointee_override().is_some() {
            return Err("physical-lds-exchange adjusted marker ABI is unsupported");
        }
        if !matches!(argument.mode(), SemanticAbiPassModeV1::Pair { .. })
            || declaration.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
            || declaration.layout().size_bytes() != Some(16)
            || declaration.layout().alignment_bytes() != 8
            || declaration.layout().is_uninhabited()
        {
            return Err(
                "physical-lds-exchange input/output are not exact pointer/length pair ABIs",
            );
        }
    }
    Ok(operation)
}

//! Exact trusted-terminal adapter for CombinedV6 terminal144 / MIR36 intrinsic92.
//! The caller must already retain the authenticated terminal Instance. This
//! helper rechecks complete declared/actual const grammar and source/ABI shape;
//! the occurrence producer separately validates actual caller transport/custody.
use fe2o3_mir_model::semantic_mir_v1::*;
use rustc_middle::ty::{FnSig, Instance, TyCtxt};

pub(crate) fn operation<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    signature: &FnSig<'tcx>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, &'static str> {
    if !crate::production_complete_body_call_vnext::valid_marker_signature(tcx, signature)
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || abi.can_unwind()
        || abi.fixed_count() != 10
        || abi.source_input_types().len() != 10
        || abi.arguments().len() != 10
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        || !types
            .get(abi.source_output_type().index() as usize)
            .is_some_and(|ty| matches!(ty.shape(), SemanticTypeShapeV1::Unit))
    {
        return Err("complete-body marker source/ABI signature differs");
    }
    for (index, ty) in abi.source_input_types().iter().enumerate() {
        let declaration = types
            .get(ty.index() as usize)
            .ok_or("complete-body marker ABI type missing")?;
        let argument = abi.arguments()[index].value();
        if argument.adjusted().is_some() || argument.pointee_override().is_some() {
            return Err("complete-body marker adjusted ABI input is unsupported");
        }
        if index == 0 {
            if !matches!(argument.mode(), SemanticAbiPassModeV1::Pair { .. })
                || declaration.layout().size_bytes() != Some(16)
                || declaration.layout().alignment_bytes() != 8
            {
                return Err("complete-body output slice ABI is not the exact pointer/length pair");
            }
        } else if !matches!(argument.mode(), SemanticAbiPassModeV1::Direct(_))
            || !matches!(declaration.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits })
                if *bits == if index < 5 {32} else {8})
        {
            return Err("complete-body scalar ABI width differs");
        }
    }
    let packed =
        crate::production_complete_body_call_vnext::parse_complete_body_consts(tcx, instance)?;
    Ok(SemanticCompilerIntrinsicOperationV1::Gfx942CompleteBody(
        SemanticCompleteBodyPackingVNext {
            block_count: packed.block_count(),
            instruction_count: packed.instruction_count(),
            block_words: packed.block_words(),
            instruction_words: packed.instruction_words(),
        },
    ))
}

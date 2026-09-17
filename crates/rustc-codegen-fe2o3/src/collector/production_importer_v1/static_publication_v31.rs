fn import_static_publication_v31<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
    let reject = || body_owner_table_mismatch_v1("static publication ABI or nominal type");
    let publish = match expansion {
        Expansion::StaticPublication128PublishF32 => true,
        Expansion::StaticPublication128TryReadF32 => false,
        _ => return Err(reject()),
    };
    if inputs.len() != rust_inputs.len() || inputs.len() != (if publish { 4 } else { 3 }) {
        return Err(reject());
    }
    let (element, space) = rust_disjoint_slice_v1(tcx, rust_inputs[0]).ok_or_else(reject)?;
    if space != SemanticDisjointIndexSpaceV1::Index1d
        || !matches!(element.kind(), TyKind::Float(FloatTy::F32))
        || !matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
        || (publish && rust_inputs[3] != element)
    {
        return Err(reject());
    }
    let atomic = rust_shared_slice_element_v1(rust_inputs[1]).ok_or_else(reject)?;
    let layout_cx = rustc_middle::ty::layout::LayoutCx::new(
        tcx,
        rustc_middle::ty::TypingEnv::fully_monomorphized(),
    );
    if !crate::rust_type_layout_v3::is_genuine_core_atomic_u32_v1(tcx, &layout_cx, atomic)
        || !rust_trusted_adt_type_arguments_v1(
            tcx,
            rust_output,
            TrustedDeviceItem::PublicationAttemptF32,
        )
        .is_some_and(|arguments| arguments.is_empty())
    {
        return Err(reject());
    }
    let pointer = aggregate_field_v1(types, inputs[0], 0)?;
    let payload_element = pointer_pointee_v1(types, pointer)?;
    let result_element = aggregate_field_v1(types, output, 1)?;
    if payload_element != result_element || (publish && inputs[3] != result_element) {
        return Err(reject());
    }
    Ok(if publish {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128PublishF32 {
            payload: inputs[0],
            flags: inputs[1],
            result: output,
        }
    } else {
        SemanticCompilerIntrinsicOperationV1::StaticPublication128TryReadF32 {
            payload: inputs[0],
            flags: inputs[1],
            result: output,
        }
    })
}

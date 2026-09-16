fn rust_read_only_allocation_element_v30<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
) -> Option<Ty<'tcx>> {
    let arguments =
        rust_trusted_adt_type_arguments_v1(tcx, ty, TrustedDeviceItem::ReadOnlyAllocation)?;
    let [element] = arguments.as_slice() else {
        return None;
    };
    matches!(
        element.kind(),
        TyKind::Uint(UintTy::U16) | TyKind::Float(FloatTy::F32)
    )
    .then_some(*element)
}

fn import_read_only_allocation_v30<'tcx>(
    tcx: TyCtxt<'tcx>,
    expansion: crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1,
    rust_inputs: &[Ty<'tcx>],
    rust_output: Ty<'tcx>,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    types: &[SemanticTypeDeclV1],
) -> Result<SemanticCompilerIntrinsicOperationV1, ProductionSemanticImportErrorV1> {
    use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
    let reject =
        || body_owner_table_mismatch_v1("consuming read-only allocation ABI or nominal type");
    if inputs.len() != rust_inputs.len() || inputs.is_empty() {
        return Err(reject());
    }
    match expansion {
        Expansion::DisjointSliceIntoReadOnly if inputs.len() == 1 => {
            let (rust_element, space) =
                rust_disjoint_slice_v1(tcx, rust_inputs[0]).ok_or_else(reject)?;
            if space != SemanticDisjointIndexSpaceV1::Index1d
                || rust_read_only_allocation_element_v30(tcx, rust_output) != Some(rust_element)
            {
                return Err(reject());
            }
            let source_pointer = aggregate_field_v1(types, inputs[0], 0)?;
            let element = pointer_pointee_v1(types, source_pointer)?;
            let view_pointer = aggregate_field_v1(types, output, 0)?;
            if pointer_pointee_v1(types, view_pointer)? != element {
                return Err(reject());
            }
            Ok(
                SemanticCompilerIntrinsicOperationV1::DisjointSliceIntoReadOnly {
                    slice: inputs[0],
                    view: output,
                    element,
                },
            )
        }
        Expansion::ReadOnlyAllocationLen | Expansion::ReadOnlyAllocationLoadOr => {
            let TyKind::Ref(_, rust_view, rustc_hir::Mutability::Not) = *rust_inputs[0].kind()
            else {
                return Err(reject());
            };
            let rust_element =
                rust_read_only_allocation_element_v30(tcx, rust_view).ok_or_else(reject)?;
            let view = pointer_pointee_v1(types, inputs[0])?;
            let pointer = aggregate_field_v1(types, view, 0)?;
            let element = pointer_pointee_v1(types, pointer)?;
            if expansion == Expansion::ReadOnlyAllocationLen {
                if inputs.len() != 1 || !matches!(rust_output.kind(), TyKind::Uint(UintTy::Usize)) {
                    return Err(reject());
                }
                Ok(SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLen { view })
            } else {
                if inputs.len() != 3
                    || !matches!(rust_inputs[1].kind(), TyKind::Uint(UintTy::Usize))
                    || rust_inputs[2] != rust_element
                    || rust_output != rust_element
                    || inputs[2] != element
                    || output != element
                {
                    return Err(reject());
                }
                Ok(
                    SemanticCompilerIntrinsicOperationV1::ReadOnlyAllocationLoadOr {
                        view,
                        element,
                    },
                )
            }
        }
        _ => Err(reject()),
    }
}

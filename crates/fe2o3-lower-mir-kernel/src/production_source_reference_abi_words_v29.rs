// Exact original reference argument/return word replay over shared Pliron leaf identities.
fn check_by_value_abi_components_v29(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueV1,
    output: &[ByValueKernelParameterComponentV1],
    mut budget: Option<&mut dyn SemanticEmissionBudgetV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let ty = abi.ty();
    if abi.adjusted().is_some() {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate argument has an adjusted ABI type",
        ));
    }
    let source_size = types[ty.index() as usize]
        .layout()
        .size_bytes()
        .ok_or_else(|| unsupported(0, None, None, "by-value argument layout is unsized"))?;
    let mut words = Vec::new();
    let requested = argument_product_v1(output.len(), 2)?;
    if let Some(budget) = budget.as_deref_mut() {
        budget.charge_work(argument_sum_v1(&[
            64,
            argument_product_v1(
                requested,
                requested.checked_ilog2().unwrap_or(0) as usize + 20,
            )?,
        ])?)?;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Vec<(u64, u64, SemanticBackendScalarV1)>>(),
            argument_product_v1(
                requested,
                std::mem::size_of::<(u64, u64, SemanticBackendScalarV1)>(),
            )?,
        ])?)?;
    }
    words.try_reserve_exact(requested).map_err(|_| {
        ProductionSemanticKirErrorV1::AllocationFailure {
            resource: ProductionSemanticKirResourceV1::DebugBindings,
        }
    })?;
    if let Some(budget) = budget.as_deref_mut() {
        budget.reserve_storage(argument_product_v1(
            words
                .capacity()
                .checked_sub(requested)
                .ok_or(ArgumentResourceV1::Accounting)?,
            std::mem::size_of::<(u64, u64, SemanticBackendScalarV1)>(),
        )?)?;
    }
    for (_, _, _, offset, leaf) in output {
        for (relative, scalar) in leaf.words() {
            let offset = offset
                .checked_add(relative)
                .ok_or_else(|| unsupported(0, None, None, "by-value ABI word offset overflows"))?;
            let width = scalar.primitive().size_bytes().ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "by-value scalar component has no exact byte width",
                )
            })?;
            let end = offset.checked_add(width).ok_or_else(|| {
                unsupported(0, None, None, "by-value scalar component range overflows")
            })?;
            if end > source_size {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "by-value scalar component exceeds its source layout",
                ));
            }
            words.push((offset, end, scalar));
        }
    }
    words.sort_unstable_by_key(|word| word.0);
    if words.windows(2).any(|pair| pair[0].1 > pair[1].0) {
        return Err(unsupported(
            0,
            None,
            None,
            "by-value scalar component ranges overlap",
        ));
    }
    let exact_mode = match abi.mode() {
        SemanticAbiPassModeV1::Ignore => {
            output.is_empty()
                && types[ty.index() as usize].layout().size_bytes() == Some(0)
                && matches!(
                    types[ty.index() as usize].layout().backend_repr(),
                    SemanticBackendReprV1::Memory { sized: true }
                )
        }
        SemanticAbiPassModeV1::Direct(attributes) => {
            match types[ty.index() as usize].layout().backend_repr() {
                SemanticBackendReprV1::Scalar(root_scalar) => matches!(
                    words.as_slice(),
                    [(0, _, leaf_scalar)] if leaf_scalar == root_scalar
                ),
                SemanticBackendReprV1::Memory { sized: true } => {
                    function.abi().spec_abi_unadjusted()
                        && *attributes
                            == fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueAttributesV1::plain(
                            )
                        && !output.is_empty()
                }
                _ => false,
            }
        }
        SemanticAbiPassModeV1::Pair { .. } => {
            let SemanticBackendReprV1::ScalarPair { first, second } =
                types[ty.index() as usize].layout().backend_repr()
            else {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "pair by-value aggregate lacks scalar-pair ABI representation",
                ));
            };
            let first_size = first.primitive().size_bytes().ok_or_else(|| {
                unsupported(
                    0,
                    None,
                    None,
                    "first aggregate ABI scalar has invalid width",
                )
            })?;
            let alignment = second.primitive().alignment_bytes();
            let second_offset = first_size
                .checked_add(alignment.saturating_sub(1))
                .map(|value| value & !alignment.saturating_sub(1))
                .ok_or_else(|| {
                    unsupported(0, None, None, "aggregate ABI scalar offset overflows")
                })?;
            match words.as_slice() {
                [(0, _, a), (b_offset, _, b)] => {
                    a == first && *b_offset == second_offset && b == second
                }
                _ => false,
            }
        }
        SemanticAbiPassModeV1::Cast { pad_i32, cast } => {
            let layout = types[ty.index() as usize].layout();
            let regular = cast.attributes().regular();
            let exact = matches!(
                function.abi().canon_abi(),
                SemanticCanonAbiV1::Rust
                    | SemanticCanonAbiV1::RustCold
                    | SemanticCanonAbiV1::RustPreserveNone
            ) && matches!(
                layout.backend_repr(),
                SemanticBackendReprV1::Memory { sized: true }
            ) && layout.size_bytes().is_some_and(|size| size != 0)
                && !pad_i32
                && cast.prefix().iter().all(Option::is_none)
                && cast.rest_offset_bytes().is_none()
                && cast.rest().unit().kind() == SemanticAbiRegisterKindV1::Integer
                && layout.size_bytes() == Some(cast.rest().unit().size_bytes())
                && layout.size_bytes() == Some(cast.rest_total_bytes())
                && !cast.rest_consecutive()
                && !regular.no_alias()
                && regular.pointer_capture().is_none()
                && !regular.non_null()
                && !regular.read_only()
                && !regular.in_register()
                && cast.attributes().extension() == SemanticAbiExtensionV1::None
                && cast.attributes().pointee_size_bytes() == 0
                && cast.attributes().pointee_alignment_bytes().is_none()
                && !output.is_empty();
            if !exact {
                return Err(unsupported(
                    0,
                    None,
                    None,
                    "aggregate cast ABI is not an exact simple Rust integer transport",
                ));
            }
            true
        }
        SemanticAbiPassModeV1::Indirect {
            attributes,
            metadata_attributes,
            on_stack,
        } => {
            let layout = types[ty.index() as usize].layout();
            let regular = attributes.regular();
            let exact = matches!(
                layout.backend_repr(),
                SemanticBackendReprV1::Memory { sized: true }
            ) && layout.size_bytes().is_some_and(|size| size != 0)
                && metadata_attributes.is_none()
                && !on_stack
                && regular.no_alias()
                && matches!(
                    regular.pointer_capture(),
                    Some(
                        SemanticAbiPointerCaptureV1::CapturesAddress
                            | SemanticAbiPointerCaptureV1::CapturesNone
                    )
                )
                && regular.non_null()
                && regular.no_undef()
                && attributes.extension() == SemanticAbiExtensionV1::None
                && attributes.pointee_size_bytes() == layout.rustc_size_bytes()
                && attributes.pointee_alignment_bytes() == Some(layout.alignment_bytes())
                && !output.is_empty();
            if !exact {
                let reason = if metadata_attributes.is_some() {
                    "metadata-bearing indirect aggregate ABI is unsupported"
                } else if *on_stack {
                    "on-stack indirect aggregate ABI is unsupported"
                } else {
                    "indirect aggregate carrier does not exactly match its sized source layout"
                };
                return Err(unsupported(0, None, None, reason));
            }
            true
        }
    };
    if !exact_mode {
        return Err(unsupported(
            0,
            None,
            None,
            "aggregate kernel argument ABI mode does not match its scalar components",
        ));
    }
    Ok(())
}

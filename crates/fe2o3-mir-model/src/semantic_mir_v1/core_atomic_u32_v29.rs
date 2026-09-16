// The marker retains nominal compiler classification; these checks establish
// its exact pinned-core storage consequences, not authority from shape alone.
fn validate_core_atomic_u32_v29(
    context: &mut ValidationContextV1<'_>,
    root: SemanticTypeIdV1,
) -> Result<(), SemanticMirErrorV1> {
    let mut current = root;
    // Atomic<u32> -> UnsafeCell<Align4<u32>> -> Align4<u32> -> u32.
    for depth in 0..4 {
        charge_validation_work(context, 1)?;
        let ty = context
            .request
            .types
            .get(current.index() as usize)
            .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
        let layout = ty.layout();
        if layout.size_bytes() != Some(4)
            || layout.rustc_size_bytes() != 4
            || layout.alignment_bytes() != 4
            || layout.unadjusted_abi_alignment_bytes() != 4
            || layout.is_uninhabited()
            || layout.largest_niche().is_some()
            || !matches!(
                layout.variants(),
                SemanticRustcVariantsV1::Single { index: 0 }
            )
            || (depth != 0 && ty.rust_type_kind() != SemanticRustTypeKindV1::Ordinary)
            || ty.abi_properties().first_pointee().is_some()
            || ty.abi_properties().second_pointee().is_some()
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        if depth == 3 {
            return if matches!(
                ty.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32
                })
            ) {
                Ok(())
            } else {
                Err(SemanticMirErrorV1::InvalidTypeLayout)
            };
        }
        let SemanticTypeShapeV1::Aggregate(fields) = ty.shape() else {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        };
        let [field] = fields.fields() else {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        };
        if !matches!(layout.fields(), SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes, memory_order_source_indices,
        } if source_order_offsets_bytes.as_ref() == [0]
            && memory_order_source_indices.as_ref() == [0])
            || !matches!(layout.details(), SemanticTypeLayoutDetailsV1::Aggregate(fields)
                if fields.field_offsets() == [0])
        {
            return Err(SemanticMirErrorV1::InvalidTypeLayout);
        }
        current = *field;
    }
    unreachable!("the final pinned storage layer returns")
}

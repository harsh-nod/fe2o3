use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn owner(
    inspect: impl FnOnce(&SourceSlots<'_, '_>, &[Type], TypeId, &mut Writer<'_, '_>) -> Result<()>,
) {
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            super::super::super::paired::aggregate_tests::checked_transform(
                types,
                functions,
                SemanticCheckedBinaryOpV1::Add,
                false,
                false,
            )
        },
        |plan, out| {
            super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let source = plan.source(out)?.source_semantic(out.budget)?;
                let pair = source.functions().last().unwrap().locals()[4].ty();
                inspect(slots, source.types(), pair, out)
            })
        },
    )
    .0
    .unwrap();
}

fn inspect_layout(
    types: &[Type],
    abi: &source_abi::SourceAbi,
    ty: TypeId,
    work: usize,
    storage: usize,
) -> (Result<Option<(u64, u64, bool)>>, usize, usize, usize) {
    let mut work_budget = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work_budget,
        storage,
    );
    let result = (|| {
        budget.reserve_storage(crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT)?;
        let mut writer = Writer::new(&mut budget)?;
        let before = writer.budget.storage();
        let schema = SourceMemoryTypesV51::derive(types, abi, &mut writer)?;
        // Only retained row capacities survive the iterative postorder. Its
        // color/stack buffers affect peak, not the retained owner floor.
        assert_eq!(
            writer.budget.storage() - before,
            headers()
                + schema.layouts.capacity() * size_of::<Layout>()
                + schema.fields.capacity() * size_of::<Field>()
        );
        schema.layout(ty, &mut writer)
    })();
    (
        result,
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
    )
}

#[test]
fn original_typed_memory_schema_preserves_padding_and_rejects_recursive_or_zero_sized_fields() {
    owner(|slots, types, pair, _| {
        let original = &types[pair.index() as usize];
        let Shape::Tuple(fields) = original.shape() else {
            panic!("original checked pair")
        };
        let word = fields.fields()[0];
        let unit = TypeId::from_index(
            types
                .iter()
                .position(|ty| matches!(ty.shape(), Shape::Unit))
                .unwrap() as u32,
        );
        assert_eq!(
            inspect_layout(types, &slots.abi, pair, LIMIT, LIMIT).0?,
            Some((8, 4, true))
        );
        for (bytes, alignment, offsets, children) in [
            (8, 4, vec![1, 4], fields.fields().to_vec()),
            (8, 4, vec![0, 0], fields.fields().to_vec()),
            (8, 4, vec![0, 4], vec![word, unit]),
        ] {
            let mut changed = types.to_vec();
            changed[pair.index() as usize] = Type::new(
                original.identity(),
                original.layout_identity(),
                SemanticTypeLayoutV1::aggregate(
                    Some(bytes),
                    alignment,
                    SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
                )
                .unwrap(),
                Shape::Tuple(SemanticAggregateTypeV1::new(children).unwrap()),
            );
            assert_eq!(
                inspect_layout(&changed, &slots.abi, pair, LIMIT, LIMIT).0?,
                None
            );
        }
        let mut recursive = types.to_vec();
        recursive[pair.index() as usize] = Type::new(
            original.identity(),
            original.layout_identity(),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                4,
                SemanticFieldsShapeV1::array(8, 1),
                SemanticRustcVariantsV1::Single { index: 0 },
                Repr::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            Shape::Array {
                element: pair,
                length: 1,
            },
        );
        assert_eq!(
            inspect_layout(&recursive, &slots.abi, pair, LIMIT, LIMIT).0?,
            None
        );
        Ok(())
    });
}

#[test]
fn original_typed_memory_schema_tracks_reference_copy_permission_and_exact_pointer_encoding() {
    owner(|slots, types, pair, _| {
        let original = &types[pair.index() as usize];
        let Shape::Tuple(fields) = original.shape() else {
            panic!("original checked pair")
        };
        let word = fields.fields()[0];
        for (kind, mutable, space, pointee, expected) in [
            (
                PointerKind::Reference,
                Mutability::Immutable,
                0,
                word,
                Some((8, 8, true)),
            ),
            (
                PointerKind::Reference,
                Mutability::Mutable,
                0,
                word,
                Some((8, 8, false)),
            ),
            (
                PointerKind::Raw,
                Mutability::Mutable,
                0,
                word,
                Some((8, 8, true)),
            ),
            (PointerKind::Reference, Mutability::Immutable, 0, pair, None),
            (PointerKind::Raw, Mutability::Mutable, 1, word, None),
        ] {
            let mut changed = types.to_vec();
            changed[pair.index() as usize] = Type::new(
                original.identity(),
                original.layout_identity(),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    Repr::scalar(BackendScalar::initialized(
                        Primitive::pointer(space, 8, 8),
                        SemanticScalarValidityRangeV1::new(
                            if kind == PointerKind::Reference { 1 } else { 0 },
                            u64::MAX.into(),
                        ),
                    )),
                    false,
                )
                .unwrap(),
                Shape::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        kind,
                        mutable,
                        space,
                        64,
                        Metadata::None,
                    )
                    .unwrap(),
                ),
            );
            assert_eq!(
                inspect_layout(&changed, &slots.abi, pair, LIMIT, LIMIT).0?,
                expected
            );
        }
        Ok(())
    });
}

#[test]
fn original_typed_memory_schema_has_exact_resource_cuts_and_retained_vs_peak_accounting() {
    owner(|slots, types, pair, _| {
        let measured = inspect_layout(types, &slots.abi, pair, LIMIT, LIMIT);
        assert_eq!(measured.0?, Some((8, 4, true)));
        assert!(measured.3 > measured.2);
        let exact = inspect_layout(types, &slots.abi, pair, measured.1, measured.3);
        assert_eq!(exact.0?, Some((8, 4, true)));
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
        assert!(
            matches!(inspect_layout(types, &slots.abi, pair, measured.1 - 1, measured.3).0,
            Err(Error::Resource(Resource::Work(error))) if error.actual() == measured.1 && error.limit() == measured.1 - 1)
        );
        assert!(
            matches!(inspect_layout(types, &slots.abi, pair, measured.1, measured.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error))) if error.actual() == measured.3 && error.limit() == measured.3 - 1)
        );
        Ok(())
    });
}

#[test]
fn original_typed_memory_schema_header_covers_independent_retained_and_scratch_fields() {
    struct FieldsMirror {
        layouts: Vec<Layout>,
        fields: Vec<Field>,
    }
    assert_eq!(size_of::<SourceMemoryTypesV51>(), size_of::<FieldsMirror>());
    assert_eq!(
        headers(),
        size_of::<FieldsMirror>()
            + 2 * size_of::<Vec<Layout>>()
            + 2 * size_of::<Vec<Field>>()
            + size_of::<Vec<u8>>()
            + size_of::<Vec<Frame>>()
            + 2 * size_of::<Frame>()
            + size_of::<Layout>()
            + size_of::<Option<TypeId>>()
            + size_of::<Result<Option<(u64, u64, bool)>>>()
            + 23 * size_of::<usize>()
            + 14 * size_of::<&()>()
    );
}

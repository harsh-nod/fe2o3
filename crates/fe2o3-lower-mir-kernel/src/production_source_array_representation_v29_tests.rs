use super::*;

fn declarations(
    space: u32,
    metadata: SemanticPointerMetadataV1,
    size: u64,
) -> Vec<SemanticTypeDeclV1> {
    vec![
        fact_scalar(false, false),
        fact_pointer(
            SemanticTypeIdV1::from_index(0),
            SemanticPointerKindV1::Raw,
            SemanticMutabilityV1::Immutable,
            space,
            64,
            metadata,
            SemanticTypeLayoutV1::new(Some(size), 8).unwrap(),
        ),
        fact_declaration(
            901,
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                16,
                8,
                SemanticFieldsShapeV1::array(8, 2),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(1),
                length: 2,
            },
        ),
    ]
}

#[test]
fn original_and_legacy_retained_pointer_facts_preserve_exact_shapes_and_work() {
    for (space, original, legacy) in [
        (0, AddressSpace::Generic, AddressSpace::Global),
        (1, AddressSpace::Global, AddressSpace::Global),
        (3, AddressSpace::Workgroup, AddressSpace::Workgroup),
        (4, AddressSpace::Constant, AddressSpace::Constant),
        (5, AddressSpace::Private, AddressSpace::Private),
    ] {
        let types = declarations(space, SemanticPointerMetadataV1::None, 8);
        for (representation, expected) in [
            (ExecutionCfgRepresentationV29::LegacyAbi, legacy),
            (ExecutionCfgRepresentationV29::OriginalSource, original),
        ] {
            let mut work = PrivateArrayRecorderBudgetV1::new(1, 29).unwrap();
            let facts = private_retained_array_facts_with_representation_v29(
                &types,
                SemanticTypeIdV1::from_index(2),
                2,
                representation,
                &mut work,
            )
            .unwrap()
            .unwrap();
            assert_eq!(work.work.work(), 29);
            assert_eq!(
                (facts.length, facts.element.size, facts.element.alignment),
                (2, 8, 8)
            );
            assert_eq!(
                facts.element.element.into_owned_type(),
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    expected,
                    AccessMode::ReadOnly
                )
            );
            let plan = retained_array_slot_plan_with_representation_v29(
                &types,
                SemanticTypeIdV1::from_index(2),
                2,
                representation,
            )
            .unwrap();
            let (actual, alignment, array) = plan.storage.scalar_array().unwrap();
            assert_eq!(*actual, facts.element.element.into_owned_type());
            assert_eq!(alignment, facts.element.alignment);
            assert_eq!(array.unwrap().length, facts.length);
        }
        let legacy_plan =
            retained_array_slot_plan_v1(&types, SemanticTypeIdV1::from_index(2), 2).unwrap();
        assert_eq!(
            *legacy_plan.storage.scalar_array().unwrap().0,
            Type::pointer(Type::Scalar(ScalarType::U32), legacy, AccessMode::ReadOnly)
        );
    }
}

#[test]
fn original_retained_pointer_facts_keep_unsupported_space_metadata_and_layout_refusals() {
    for (space, metadata, size) in [
        (2, SemanticPointerMetadataV1::None, 8),
        (6, SemanticPointerMetadataV1::None, 8),
        (u32::MAX, SemanticPointerMetadataV1::None, 8),
        (0, SemanticPointerMetadataV1::SliceLength, 8),
        (0, SemanticPointerMetadataV1::VTable, 8),
        (0, SemanticPointerMetadataV1::None, 16),
    ] {
        let types = declarations(space, metadata, size);
        for representation in [
            ExecutionCfgRepresentationV29::LegacyAbi,
            ExecutionCfgRepresentationV29::OriginalSource,
        ] {
            let mut work = PrivateArrayRecorderBudgetV1::new(1, 100).unwrap();
            assert!(
                private_retained_array_facts_with_representation_v29(
                    &types,
                    SemanticTypeIdV1::from_index(2),
                    2,
                    representation,
                    &mut work
                )
                .unwrap()
                .is_none()
            );
        }
    }
}

#[test]
fn original_and_legacy_retained_array_facts_keep_exact_work_and_sticky_one_short() {
    let types = declarations(0, SemanticPointerMetadataV1::None, 8);
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        for limit in [29, 28] {
            let mut work = PrivateArrayRecorderBudgetV1::new(1, limit).unwrap();
            let result = private_retained_array_facts_with_representation_v29(
                &types,
                SemanticTypeIdV1::from_index(2),
                2,
                representation,
                &mut work,
            );
            if limit == 29 {
                assert!(result.unwrap().is_some());
                assert_eq!(work.work.work(), 29);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: 29,
                        limit: 28
                    })
                ));
                assert_eq!(work.work.work(), 28);
                assert!(matches!(
                    work.charge_private_array_work(0),
                    Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual: 29,
                        limit: 28
                    })
                ));
            }
        }
    }
}

#[test]
fn retained_array_recorder_work_replacement_preserves_explicit_representation() {
    for representation in [
        ExecutionCfgRepresentationV29::LegacyAbi,
        ExecutionCfgRepresentationV29::OriginalSource,
    ] {
        let recorder = PrivateArrayFunctionRecorderV1::new_with_representation_v29(
            PrivateArrayRecorderWorkV1::Owned(PrivateArrayLazyBudgetV1::new(1, 100)),
            false,
            100,
            PrivateArrayPayloadV1::default(),
            SemanticEmissionPlacementV1::default(),
            representation,
        );
        let (recorder, old) = recorder.replace_work(PrivateArrayRecorderWorkV1::Owned(
            PrivateArrayLazyBudgetV1::new(1, 100),
        ));
        assert_eq!(recorder.representation, representation);
        drop(old);
    }
}

// Isolated representation tests confer no original-source admission authority.
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLayoutIdentityV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

fn fixture(
    count: usize,
) -> (
    Vec<SemanticTypeDeclV1>,
    SemanticValueBindingV1,
    SemanticPlaceV1,
) {
    let scalar = SemanticTypeIdV1::from_index(0);
    let types = vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([30; 32]),
            SemanticLayoutIdentityV1::from_sha256([30; 32]),
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(
                fe2o3_mir_model::semantic_mir_v1::SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([31; 32]),
            SemanticLayoutIdentityV1::from_sha256([31; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(count as u64 * 4),
                4,
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateLayoutV1::new(
                    (0..count).map(|index| index as u64 * 4).collect(),
                    vec![],
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1::new(vec![scalar; count])
                    .unwrap(),
            ),
        ),
    ];
    let original = SemanticValueBindingV1::Aggregate(
        (0..count)
            .map(|index| SemanticValueBindingV1::Value {
                id: ValueId(index as u32),
                ty: Type::Scalar(ScalarType::U32),
            })
            .collect(),
    );
    let destination = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(0),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field((count / 2) as u32), scalar)
                .unwrap(),
        ],
        scalar,
    )
    .unwrap();
    (types, original, destination)
}

#[test]
fn static_field_update_copy_is_linear_exactly_budgeted_and_preserves_original() {
    for count in [2, 16, 64, 127] {
        let (types, original, destination) = fixture(count);
        // One root, one binding/type pair per scalar, one vector allocation,
        // and the fixed wrapper plus one typed projection.
        let required_work = 12 + 2 * count;
        let required_storage = (count + 1) * std::mem::size_of::<SemanticValueBindingV1>()
            + std::mem::size_of::<Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1>>();
        for (work_short, storage_short) in [(false, false), (true, false), (false, true)] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(required_work - usize::from(work_short));
            let mut budget = ArgumentBudgetV1::new(
                &mut work,
                37 + required_storage - usize::from(storage_short),
            );
            budget.reserve_storage(37).unwrap();
            let result = rebuild_static_field_update_v29(
                &types,
                SemanticTypeIdV1::from_index(1),
                &original,
                &destination,
                SemanticValueBindingV1::Value {
                    id: ValueId(999),
                    ty: Type::Scalar(ScalarType::U32),
                },
                &mut budget,
            );
            if work_short {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
            } else if storage_short {
                assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
            } else {
                let updated = result.as_ref().unwrap();
                let SemanticValueBindingV1::Aggregate(fields) = updated else {
                    panic!("missing updated holder")
                };
                assert_eq!(fields.capacity(), count);
                for (index, field) in fields.iter().enumerate() {
                    assert!(matches!(field, SemanticValueBindingV1::Value { id, ty }
                        if *id == ValueId(if index == count / 2 { 999 } else { index as u32 })
                            && *ty == Type::Scalar(ScalarType::U32)));
                }
                assert_eq!(budget.work(), required_work);
                assert_eq!(budget.storage(), 37 + required_storage);
            }
            drop(result);
            let SemanticValueBindingV1::Aggregate(fields) = &original else {
                unreachable!()
            };
            assert!(
                matches!(fields[count / 2], SemanticValueBindingV1::Value { id, .. }
                if id == ValueId((count / 2) as u32))
            );
            budget.release_storage(budget.storage() - 37).unwrap();
            assert_eq!(budget.storage(), 37);
        }
    }
}

#[test]
fn static_field_update_rejects_wrong_shape_path_type_and_component_overflow() {
    for fault in 0..5 {
        let (types, original, mut destination) = fixture(if fault == 4 { 128 } else { 2 });
        if fault == 0 {
            destination =
                SemanticPlaceV1::new(destination.local(), vec![], destination.ty()).unwrap();
        } else if fault == 1 {
            destination = SemanticPlaceV1::new(
                destination.local(),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(99),
                        destination.ty(),
                    )
                    .unwrap(),
                ],
                destination.ty(),
            )
            .unwrap();
        } else if fault == 2 {
            destination = SemanticPlaceV1::new(
                destination.local(),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Field(0),
                        SemanticTypeIdV1::from_index(1),
                    )
                    .unwrap(),
                ],
                SemanticTypeIdV1::from_index(1),
            )
            .unwrap();
        }
        let original = if fault == 3 {
            SemanticValueBindingV1::Unit
        } else {
            original
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        let result = rebuild_static_field_update_v29(
            &types,
            SemanticTypeIdV1::from_index(1),
            &original,
            &destination,
            SemanticValueBindingV1::Value {
                id: ValueId(999),
                ty: Type::Scalar(ScalarType::U32),
            },
            &mut budget,
        );
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
            if detail == if fault == 4 { "execution CFG transport differs from its captured SSA state" }
            else { "static field update differs from its original SSA holder" })
        );
        drop(result);
        budget.release_storage(budget.storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

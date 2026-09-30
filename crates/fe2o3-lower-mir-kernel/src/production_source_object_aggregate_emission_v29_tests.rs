use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const FIELD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const RECORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);

// An inert shape fixture for the allocation-free preflight only. It cannot
// stand in for original source admission or selected schema/currentness proof.
fn types(count: usize, nominal: bool, nested: bool) -> Vec<SemanticTypeDeclV1> {
    let field = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([81; 32]),
        SemanticLayoutIdentityV1::from_sha256([82; 32]),
        SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 32,
        }),
    );
    let fields =
        SemanticAggregateTypeV1::new(vec![if nested { RECORD } else { FIELD }; count]).unwrap();
    let record = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([83; 32]),
        SemanticLayoutIdentityV1::from_sha256([84; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(count as u64 * 4),
            4,
            SemanticAggregateLayoutV1::new(
                (0..count).map(|index| index as u64 * 4).collect(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        if nominal {
            SemanticTypeShapeV1::Aggregate(fields)
        } else {
            SemanticTypeShapeV1::Tuple(fields)
        },
    );
    vec![field, record]
}

#[test]
fn scalar_aggregate_shape_preflight_has_exact_linear_work_and_no_storage() {
    for nominal in [false, true] {
        for count in [1, 2, 31, 32, 64, 128, MAX_SSA_VALUE_COMPONENTS_V1] {
            let types = types(count, nominal, false);
            let kind = if nominal {
                SemanticAggregateKindV1::Aggregate
            } else {
                SemanticAggregateKindV1::Tuple
            };
            let expected = 6 + 2 * count;
            for short in [0, 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(expected - short);
                let mut budget = ArgumentBudgetV1::new(&mut work, 47);
                budget.reserve_storage(47).unwrap();
                let result = source_object_aggregate_field_types_v29(
                    &types,
                    RECORD,
                    &kind,
                    count,
                    &mut budget,
                );
                if short == 0 {
                    assert_eq!(result.unwrap(), vec![FIELD; count]);
                    assert_eq!(budget.work(), expected);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (47, 47));
            }
        }
    }
}

#[test]
fn scalar_aggregate_shape_preflight_rejects_unsupported_and_mismatched_shapes() {
    for (count, supplied, nominal, nested, kind) in [
        (0, 0, false, false, SemanticAggregateKindV1::Tuple),
        (
            MAX_SSA_VALUE_COMPONENTS_V1 + 1,
            MAX_SSA_VALUE_COMPONENTS_V1 + 1,
            false,
            false,
            SemanticAggregateKindV1::Tuple,
        ),
        (2, 1, false, false, SemanticAggregateKindV1::Tuple),
        (2, 2, false, false, SemanticAggregateKindV1::Aggregate),
        (2, 2, true, false, SemanticAggregateKindV1::Tuple),
        (2, 2, false, false, SemanticAggregateKindV1::Array),
        (2, 2, false, false, SemanticAggregateKindV1::EnumVariant(0)),
        (2, 2, false, true, SemanticAggregateKindV1::Tuple),
    ] {
        let types = types(count, nominal, nested);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(matches!(
            source_object_aggregate_field_types_v29(&types, RECORD, &kind, supplied, &mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "typed allocation identity or representation requires its exact source contract",
                ..
            })
        ));
        assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
    }
}

fn retained_read(index: u32) -> ScopedMemoryReadV29 {
    ScopedMemoryReadV29 {
        site: execution_site_v29(SemanticBlockIdV1::from_index(2), Some(3)),
        role: ExecutionOperandV29::RvalueOperand(index),
        prefix: 0,
        ty: FIELD,
        occurrence: ScopedMemoryOccurrenceV29::Retained {
            event: index as usize + 7,
        },
    }
}

#[test]
fn retained_aggregate_read_index_has_independent_exact_linear_work_and_no_storage() {
    for count in [1usize, 2, 31, 64, 128, MAX_SSA_VALUE_COMPONENTS_V1] {
        for short in [0, 1] {
            let mut reads = vec![None; count];
            let mut work = CanonicalKernelIrWorkBudgetV1::new(5 * count - short);
            let mut budget = ArgumentBudgetV1::new(&mut work, 47);
            budget.reserve_storage(47).unwrap();
            let result = (|| {
                for index in 0..count {
                    let read = retained_read(index as u32);
                    source_object_aggregate_insert_read_v29(
                        &mut reads,
                        read.site,
                        ValueId(index as u32),
                        read,
                        &mut budget,
                    )?;
                }
                Ok::<_, ProductionSemanticKirErrorV1>(())
            })();
            if short == 0 {
                result.unwrap();
                assert_eq!(budget.work(), 5 * count);
                for (index, read) in reads.iter().enumerate() {
                    assert_eq!(
                        *read,
                        Some((ValueId(index as u32), retained_read(index as u32)))
                    );
                }
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == 5 * count)
                );
                assert_eq!(reads[count - 1], None, "denial precedes publication");
            }
            assert_eq!((budget.storage(), budget.peak_storage()), (47, 47));
        }
    }
}

#[test]
fn retained_aggregate_read_index_rejects_changed_site_role_range_and_duplicate() {
    for fault in 0..4 {
        let mut reads = vec![None; 2];
        let original = retained_read(0);
        let mut read = original;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        match fault {
            0 => read.site = execution_site_v29(SemanticBlockIdV1::from_index(3), Some(3)),
            1 => read.role = ExecutionOperandV29::CallArgument(0),
            2 => read.role = ExecutionOperandV29::RvalueOperand(2),
            3 => source_object_aggregate_insert_read_v29(
                &mut reads,
                read.site,
                ValueId(1),
                read,
                &mut budget,
            )
            .unwrap(),
            _ => unreachable!(),
        }
        let saved = reads.clone();
        assert!(matches!(
            source_object_aggregate_insert_read_v29(
                &mut reads,
                original.site,
                ValueId(2),
                read,
                &mut budget
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "typed object source payload differs from its actual operation",
                ..
            })
        ));
        assert_eq!(reads, saved);
        assert_eq!(budget.storage(), 0);
    }
}

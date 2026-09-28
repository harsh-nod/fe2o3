use super::*;

mod carrier_tests {
    include!("production_compiler_carrier_representation_v18_tests.rs");
}
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticLayoutIdentityV1,
    SemanticPointerTypeV1, SemanticTypeIdentityV1, SemanticTypeLayoutV1,
};

const WORD: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);

fn types(space: u32) -> Vec<SemanticTypeDeclV1> {
    let declaration = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
            layout,
            shape,
        )
    };
    vec![
        declaration(
            1,
            SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        declaration(
            3,
            SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    WORD,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    space,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ),
        declaration(
            5,
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![POINTER, WORD]).unwrap()),
        ),
    ]
}

fn pointer(space: AddressSpace) -> Type {
    Type::pointer(Type::Scalar(ScalarType::U32), space, AccessMode::ReadOnly)
}

fn values(space: AddressSpace) -> [ValueDef; 2] {
    [
        ValueDef::new(ValueId(7), pointer(space)),
        ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
    ]
}

fn binding(space: AddressSpace) -> SemanticValueBindingV1 {
    SemanticValueBindingV1::Aggregate(vec![
        SemanticValueBindingV1::Value {
            id: ValueId(7),
            ty: pointer(space),
        },
        SemanticValueBindingV1::Value {
            id: ValueId(8),
            ty: Type::Scalar(ScalarType::U32),
        },
    ])
}

fn assert_cfg_mismatch(error: ProductionSemanticKirErrorV1) {
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::Unsupported {
            function: 0,
            block: None,
            statement: None,
            detail: "execution CFG transport differs from its captured SSA state",
        }
    ));
}

#[test]
fn original_source_cfg_shapes_change_only_as_zero_and_preserve_legacy_costs() {
    for (space, source_space, legacy_space) in [
        (0, AddressSpace::Generic, AddressSpace::Global),
        (1, AddressSpace::Global, AddressSpace::Global),
        (3, AddressSpace::Workgroup, AddressSpace::Workgroup),
        (4, AddressSpace::Constant, AddressSpace::Constant),
        (5, AddressSpace::Private, AddressSpace::Private),
    ] {
        let types = types(space);
        let mut legacy_work = Work::new(usize::MAX);
        let mut source_work = Work::new(usize::MAX);
        let mut legacy_budget = ArgumentBudgetV1::new(&mut legacy_work, usize::MAX);
        let mut source_budget = ArgumentBudgetV1::new(&mut source_work, usize::MAX);
        let legacy = execution_cfg_types_v29(&types, PAIR, &mut legacy_budget).unwrap();
        let source = source_execution_cfg_types_v29(&types, PAIR, &mut source_budget).unwrap();
        assert_eq!(
            legacy,
            [pointer(legacy_space), Type::Scalar(ScalarType::U32)]
        );
        assert_eq!(
            source,
            [pointer(source_space), Type::Scalar(ScalarType::U32)]
        );
        assert_eq!(
            (
                source_budget.work(),
                source_budget.storage(),
                source_budget.peak_storage()
            ),
            (
                legacy_budget.work(),
                legacy_budget.storage(),
                legacy_budget.peak_storage()
            )
        );
    }
}

#[test]
fn source_cfg_representation_is_independent_of_canonical_rebuilding() {
    let types = types(0);
    for source in [false, true] {
        for canonical in [false, true] {
            for actual in [AddressSpace::Generic, AddressSpace::Global] {
                let values = values(actual);
                let mut work = Work::new(usize::MAX);
                let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
                let mut leaves = [].iter();
                let mut physical = values.iter();
                let mut nodes = 0;
                let result = if source {
                    rebuild_source_execution_cfg_binding_v29(
                        &types,
                        PAIR,
                        canonical,
                        &mut leaves,
                        &mut physical,
                        &mut nodes,
                        &mut budget,
                    )
                } else {
                    rebuild_execution_cfg_binding_v29(
                        &types,
                        PAIR,
                        canonical,
                        &mut leaves,
                        &mut physical,
                        &mut nodes,
                        &mut budget,
                    )
                };
                let expected = if source {
                    AddressSpace::Generic
                } else {
                    AddressSpace::Global
                };
                if canonical && actual != expected {
                    assert_cfg_mismatch(result.unwrap_err());
                } else {
                    let SemanticValueBindingV1::Aggregate(fields) = result.unwrap() else {
                        panic!("expected the original aggregate binding");
                    };
                    assert_eq!(fields.len(), 2);
                    assert!(
                        matches!(&fields[0], SemanticValueBindingV1::Value { id, ty }
                        if *id == ValueId(7) && *ty == pointer(actual))
                    );
                    assert!(
                        matches!(&fields[1], SemanticValueBindingV1::Value { id, ty }
                        if *id == ValueId(8) && *ty == Type::Scalar(ScalarType::U32))
                    );
                    assert_eq!(nodes, 3);
                    assert!(leaves.next().is_none());
                    assert!(physical.next().is_none());
                }
            }
        }
    }
}

#[test]
fn source_call_shape_fallback_requires_generic_without_descriptor_authority() {
    let types = types(0);
    for source in [false, true] {
        for actual in [AddressSpace::Generic, AddressSpace::Global] {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let binding = binding(actual);
            let result = if source {
                execution_call_shape_with_representation_v29(
                    &types,
                    PAIR,
                    &binding,
                    ExecutionCfgRepresentationV29::OriginalSource,
                    &mut budget,
                )
            } else {
                execution_call_shape_v29(&types, PAIR, &binding, &mut budget)
            };
            if source == (actual == AddressSpace::Generic) {
                assert!(result.unwrap().is_empty());
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "execution call parameters differ from their source instance",
                    })
                ));
            }
        }
    }
}

#[test]
fn source_cfg_pointer_keeps_exact_work_and_storage_refusal_boundaries() {
    let types = types(0);
    let floor = 19;
    let bytes = std::mem::size_of::<Type>();
    for source in [false, true] {
        for (work_limit, storage_limit, succeeds) in [
            (3, floor + bytes, true),
            (2, floor + bytes, false),
            (3, floor + bytes - 1, false),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = if source {
                source_execution_cfg_ordinary_type_v29(&types, POINTER, &mut budget)
            } else {
                execution_cfg_ordinary_type_v29(&types, POINTER, &mut budget)
            };
            if succeeds {
                assert_eq!(
                    result.unwrap(),
                    Some(pointer(if source {
                        AddressSpace::Generic
                    } else {
                        AddressSpace::Global
                    }))
                );
                assert_eq!(budget.storage(), floor + bytes);
            } else if work_limit == 2 {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.actual() == 3 && error.limit() == 2)
                );
                assert_eq!(budget.storage(), floor);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error))) if error.actual() == floor + bytes && error.limit() == storage_limit)
                );
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(budget.work(), if work_limit == 2 { 2 } else { 3 });
        }
    }
}

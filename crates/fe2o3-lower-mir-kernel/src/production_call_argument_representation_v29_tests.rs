use super::*;

mod reference_transport_v26 {
    include!("production_source_reference_call_transport_v26_tests.rs");
}

fn projections() -> [HelperCallArgumentV1; 2] {
    [0, 1].map(|component| HelperCallArgumentV1 {
        source_argument: 0,
        tuple_field: None,
        component: Some(component),
    })
}

fn check(
    source: bool,
    actual: &SemanticValueBindingV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let physical = [
        pointer(if source {
            AddressSpace::Generic
        } else {
            AddressSpace::Global
        }),
        Type::Scalar(ScalarType::U32),
    ];
    if source {
        execution_call_argument_shape_with_representation_v29(
            &types(0),
            PAIR,
            0,
            actual,
            &projections(),
            &physical,
            ExecutionCfgRepresentationV29::OriginalSource,
            budget,
        )
    } else {
        execution_call_argument_shape_v29(
            &types(0),
            PAIR,
            0,
            actual,
            &projections(),
            &physical,
            budget,
        )
    }
}

#[test]
fn ordinary_call_argument_fallback_preserves_original_and_legacy_shapes_and_costs() {
    let run = |source| {
        let mut work = Work::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(37).unwrap();
        let actual = binding(if source {
            AddressSpace::Generic
        } else {
            AddressSpace::Global
        });
        check(source, &actual, &mut budget).unwrap();
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn ordinary_call_argument_fallback_rejects_foreign_space_access_and_pointee() {
    for source in [false, true] {
        let selected = if source {
            AddressSpace::Generic
        } else {
            AddressSpace::Global
        };
        let foreign = if source {
            AddressSpace::Global
        } else {
            AddressSpace::Generic
        };
        for wrong in [
            pointer(foreign),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                selected,
                AccessMode::ReadWrite,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U64),
                selected,
                AccessMode::ReadOnly,
            ),
        ] {
            let actual = SemanticValueBindingV1::Aggregate(vec![
                SemanticValueBindingV1::Value {
                    id: ValueId(7),
                    ty: wrong,
                },
                SemanticValueBindingV1::Value {
                    id: ValueId(8),
                    ty: Type::Scalar(ScalarType::U32),
                },
            ]);
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(37).unwrap();
            assert!(matches!(
                check(source, &actual, &mut budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    function: 0,
                    block: None,
                    statement: None,
                    detail: "execution call parameters differ from their source instance",
                })
            ));
            assert!(budget.storage() >= 37);
        }
    }
}

#[test]
fn ordinary_call_argument_fallback_preserves_exact_and_one_short_work() {
    for source in [false, true] {
        let actual = binding(if source {
            AddressSpace::Generic
        } else {
            AddressSpace::Global
        });
        let (cost, storage, peak) = {
            let mut work = Work::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(37).unwrap();
            check(source, &actual, &mut budget).unwrap();
            (budget.work(), budget.storage(), budget.peak_storage())
        };
        assert!(cost > 2);
        for limit in [cost, cost - 1] {
            let mut work = Work::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(37).unwrap();
            let result = check(source, &actual, &mut budget);
            if limit == cost {
                result.unwrap();
                assert_eq!(budget.work(), cost);
            } else {
                assert!(matches!(result,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(error)))
                        if error.actual() == cost && error.limit() == limit));
                // The final check atomically charges the two physical values.
                assert_eq!(budget.work(), cost - 2);
            }
            assert_eq!(budget.storage(), storage);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.failed_storage(), None);
        }
    }
}

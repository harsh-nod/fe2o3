use super::*;

type O = ProductionRankedOperationV1;
type T = ProductionRankedTerminatorV1;

fn id(value: u32) -> ProductionRankedValueIdV1 {
    ProductionRankedValueIdV1::new(value)
}

fn local(value: u32) -> ProductionRankedValueV1 {
    ProductionRankedValueV1::Local(id(value))
}

fn raw(blocks: Vec<ProductionRankedBlockV1>) -> ProductionRankedKernelV1 {
    ProductionRankedKernelV1 {
        function_name: "schedule".into(),
        argument_count: 0,
        blocks,
        tree_work: 0,
    }
}

fn cast(result: u32, source: u32) -> O {
    O::IndexUnsignedCast {
        result: id(result),
        source: local(source),
        bit_width: 32,
    }
}

#[test]
fn slots_bind_out_of_order_exactly_once_and_require_total_coverage() {
    let mut slots = RankedValueSlotsV1::new(2).unwrap();
    slots.bind(id(1), 7u8).unwrap();
    assert_eq!(slots.get(0), None);
    assert_eq!(slots.get(1), Some(7));
    assert!(slots.finish().is_err());
    assert!(slots.bind(id(1), 8).is_err());
    assert!(slots.bind(id(2), 9).is_err());
    slots.bind(id(0), 6).unwrap();
    assert_eq!(slots.finish(), Ok(()));
    assert_eq!(slots.get(1), Some(7));
}

#[test]
fn construction_work_accepts_exact_limit_and_rejects_one_less() {
    let kernel = raw(vec![ProductionRankedBlockV1::new(
        vec![O::IndexConstant {
            result: id(0),
            value: 1,
        }],
        T::Return,
    )]);
    let reads = RankedSemanticReadsV1::new(&kernel).unwrap();
    let exact = 17 + 1 + 64 + 2;
    assert!(RankedOperationScheduleV1::with_limit(&kernel, &reads, exact).is_ok());
    assert!(
        matches!(RankedOperationScheduleV1::with_limit(&kernel, &reads, exact - 1),
        Err(E::ResourceLimit { resource: "construction dependency work", limit, actual })
        if limit == exact - 1 && actual == exact)
    );
}

#[test]
fn repeated_dependencies_and_terminator_operands_are_charged_per_occurrence() {
    let mut kernel = raw(vec![ProductionRankedBlockV1::new(
        vec![
            O::IndexConstant {
                result: id(0),
                value: 1,
            },
            O::IndexBinary {
                result: id(1),
                kind: IndexBinaryKindAttr::Add,
                lhs: local(0),
                rhs: local(0),
            },
        ],
        T::Return,
    )]);
    for extra in [0, 8] {
        if extra != 0 {
            kernel.blocks[0].terminator = T::IndexEqual {
                lhs: local(1),
                rhs: local(1),
                true_block: 0,
                false_block: 0,
            };
        }
        let reads = RankedSemanticReadsV1::new(&kernel).unwrap();
        let exact = 263 + extra;
        assert!(RankedOperationScheduleV1::with_limit(&kernel, &reads, exact).is_ok());
        assert!(
            matches!(RankedOperationScheduleV1::with_limit(&kernel, &reads, exact - 1),
            Err(E::ResourceLimit { resource: "construction dependency work", limit, actual })
            if limit == exact - 1 && actual == exact)
        );
    }
}

#[test]
fn canonical_ids_are_checked_before_allocating_numbered_slots() {
    for result in [1, u32::MAX] {
        let kernel = raw(vec![ProductionRankedBlockV1::new(
            vec![O::IndexConstant {
                result: id(result),
                value: 0,
            }],
            T::Return,
        )]);
        assert!(
            matches!(kernel.validate(), Err(E::NonCanonicalValueId { expected: 0, actual }) if actual == result)
        );
    }
}

#[test]
fn repeated_load_occurrences_are_charged_before_dependency_deduplication() {
    use super::super::super::{ProductionSemanticLoadV2, ProductionSemanticReadModeV2};
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let mut expression = ProductionSemanticExpressionV2::Load(ProductionSemanticLoadV2 {
        block: 0,
        operation: 2,
        scalar,
        read_mode: ProductionSemanticReadModeV2::UnorderedNonVolatile,
        allocation_origin: 7,
        view: local(0),
        indices: vec![local(1)].into_boxed_slice(),
    });
    for occurrences in [1, 2, 4] {
        if occurrences > 1 {
            expression = ProductionSemanticExpressionV2::Binary {
                operation: ProductionSemanticBinaryOpV2::Add,
                scalar,
                overflow: ProductionOverflowContractV2::Wrapping,
                lhs: Box::new(expression.clone()),
                rhs: Box::new(expression.clone()),
            };
        }
        let kernel = raw(vec![ProductionRankedBlockV1::new(
            vec![
                O::View {
                    result: id(0),
                    element_width: 32,
                    writable: false,
                    shape: vec![64],
                    dynamic_extents: vec![],
                    allocation_origin: 7,
                    noalias_class: 1,
                },
                O::IndexConstant {
                    result: id(1),
                    value: 0,
                },
                O::Access {
                    kind: AccessKindAttr::Read,
                    view: local(0),
                    indices: vec![local(1)],
                },
                O::SemanticExpression {
                    result: id(2),
                    expression: expression.clone(),
                    numerical_contract: ProductionNumericalContractV2::exact_for_expression(
                        &expression,
                    ),
                },
            ],
            T::Return,
        )]);
        let reads = RankedSemanticReadsV1::new(&kernel).unwrap();
        assert_eq!(reads.work(), 4 + 2 * occurrences - 1);
        assert_eq!(reads.loads().count(), 1);
        assert_eq!(reads.dependencies().count(), occurrences);
        // One block, four nodes, three results/predecessors, two address operands,
        // one checked load, and a separately charged edge for every load occurrence.
        let exact =
            17 * reads.work() + 1 + 4 * 64 + 3 * 2 + 3 * 32 + 2 * 32 + 33 + 32 * occurrences;
        assert!(RankedOperationScheduleV1::with_limit(&kernel, &reads, exact).is_ok());
        assert!(
            matches!(RankedOperationScheduleV1::with_limit(&kernel, &reads, exact - 1),
            Err(E::ResourceLimit { resource: "construction dependency work", limit, actual })
                if limit == exact - 1 && actual == exact)
        );
    }
}

#[test]
fn self_forward_and_cross_block_construction_cycles_reject() {
    for kernel in [
        raw(vec![ProductionRankedBlockV1::new(
            vec![cast(0, 0)],
            T::Return,
        )]),
        raw(vec![ProductionRankedBlockV1::new(
            vec![
                cast(0, 1),
                O::IndexConstant {
                    result: id(1),
                    value: 0,
                },
            ],
            T::Return,
        )]),
        raw(vec![
            ProductionRankedBlockV1::new(vec![cast(0, 1)], T::Branch { target: 1 }),
            ProductionRankedBlockV1::new(vec![cast(1, 0)], T::Return),
        ]),
    ] {
        assert_eq!(
            kernel.validate(),
            Err(E::Materialization(
                "cyclic operation construction dependencies"
            ))
        );
    }
}

#[test]
fn aggregate_operation_limit_precedes_operation_contents() {
    let kernel = raw(vec![ProductionRankedBlockV1::new(
        vec![cast(0, u32::MAX); MAX_RANKED_BOUNDS_OPERATIONS],
        T::Return,
    )]);
    assert!(
        matches!(kernel.validate(), Err(E::ResourceLimit { resource: "operation", limit, actual })
        if limit == MAX_RANKED_BOUNDS_OPERATIONS && actual == limit + 1)
    );
}

#[test]
fn expanded_operation_limit_precedes_producer_validation() {
    let scalar = ProductionSemanticScalarTypeV2::Float { bits: 32 };
    let mut expression = ProductionSemanticExpressionV2::Constant { scalar, bits: 0 };
    for _ in 0..12 {
        expression = ProductionSemanticExpressionV2::Binary {
            operation: ProductionSemanticBinaryOpV2::Add,
            scalar,
            overflow: ProductionOverflowContractV2::Wrapping,
            lhs: Box::new(expression.clone()),
            rhs: Box::new(expression),
        };
    }
    assert_eq!(expression.validate().unwrap().nodes, 8191);
    let operations = (0..8)
        .map(|index| O::SemanticExpression {
            result: id(index),
            numerical_contract: ProductionNumericalContractV2::exact_for_expression(&expression),
            expression: expression.clone(),
        })
        .collect::<Vec<_>>();
    let admitted = raw(vec![ProductionRankedBlockV1::new(
        operations[..7].to_vec(),
        T::Return,
    )]);
    assert_eq!(admitted.validate(), Ok(ranked_tree_work(1, 57345).unwrap()));
    let mut kernel = raw(vec![ProductionRankedBlockV1::new(operations, T::Return)]);
    let expected = Err(E::ResourceLimit {
        resource: "operation",
        limit: MAX_RANKED_BOUNDS_OPERATIONS,
        actual: 65537,
    });
    assert_eq!(kernel.validate(), expected);
    let malformed = ProductionSemanticExpressionV2::Constant {
        scalar,
        bits: 1 << 32,
    };
    assert!(malformed.validate().is_err());
    kernel.blocks[0].operations.push(O::SemanticExpression {
        result: id(8),
        numerical_contract: ProductionNumericalContractV2::exact_for_expression(&malformed),
        expression: malformed,
    });
    assert_eq!(kernel.validate(), expected);
    let O::SemanticExpression { result, .. } = &mut kernel.blocks[0].operations[0] else {
        unreachable!()
    };
    *result = id(u32::MAX);
    assert_eq!(kernel.validate(), expected);
}

#[test]
fn cfg_backedges_are_not_operation_construction_dependencies() {
    let kernel = raw(vec![
        ProductionRankedBlockV1::new(
            vec![O::IndexConstant {
                result: id(0),
                value: 0,
            }],
            T::BranchArgs {
                target: 1,
                arguments: vec![local(0)],
            },
        ),
        ProductionRankedBlockV1::with_index_arguments(
            1,
            vec![O::IndexUnsignedCast {
                result: id(1),
                source: ProductionRankedValueV1::BlockArgument {
                    block: 1,
                    argument: 0,
                },
                bit_width: 32,
            }],
            T::BranchArgs {
                target: 1,
                arguments: vec![local(1)],
            },
        ),
    ]);
    assert!(kernel.validate().is_ok());
    let reads = RankedSemanticReadsV1::new(&kernel).unwrap();
    let schedule = RankedOperationScheduleV1::new(&kernel, &reads).unwrap();
    assert_eq!(schedule.operations, vec![(0, 0), (1, 0)]);
    assert_eq!(schedule.local_count, 2);
}

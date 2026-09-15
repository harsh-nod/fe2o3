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
fn terminator_only_nondominating_use_rejects_at_public_construction() {
    let kernel = ProductionRankedKernelV1::new(
        "terminator_dominance",
        1,
        vec![
            ProductionRankedBlockV1::new(
                vec![O::IndexConstant {
                    result: id(0),
                    value: 32,
                }],
                T::IndexLessThan {
                    lhs: ProductionRankedValueV1::Argument(0),
                    rhs: local(0),
                    true_block: 2,
                    false_block: 3,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![],
                T::IndexEqual {
                    lhs: local(1),
                    rhs: local(0),
                    true_block: 4,
                    false_block: 4,
                },
            ),
            ProductionRankedBlockV1::new(
                vec![O::IndexConstant {
                    result: id(1),
                    value: 7,
                }],
                T::Branch { target: 1 },
            ),
            ProductionRankedBlockV1::new(vec![], T::Branch { target: 1 }),
            ProductionRankedBlockV1::new(vec![], T::Return),
        ],
    )
    .unwrap();
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [dialect_kernel::dialect_registration().unwrap()],
    )
    .unwrap();
    let registered = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("terminator_dominance", kernel).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        session.construct_registered(registered),
        Err(ProductionSessionErrorV1::Operation(
            OperationHandleError::OperationVerificationRejected
        ))
    ));
    assert!(session.is_poisoned());
}

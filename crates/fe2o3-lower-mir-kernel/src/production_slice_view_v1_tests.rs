use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition,
};

#[path = "production_slice_call_composition_v1_tests.rs"]
mod call_composition;

fn site(block: u32, statement: u32, access: u32, assertion: u32) -> ProductionSliceAccessSiteV1 {
    let root = SemanticFunctionIdV1::from_index(0);
    ProductionSliceAccessSiteV1::new(
        root,
        root,
        SemanticBlockIdV1::from_index(block),
        Some(statement),
        access,
        SemanticBlockIdV1::from_index(assertion),
    )
}

fn read_slice() -> SemanticStatementV1 {
    let source = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    assignment(
        5,
        U32,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
    )
}

fn bounds_assertion(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: value(4, BOOL),
        expected: true,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: value(2, U64),
            index: value(3, U64),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

fn compare_index() -> Vec<SemanticStatementV1> {
    vec![
        assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
        assignment(
            4,
            BOOL,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left: value(3, U64),
                right: value(2, U64),
            },
        ),
    ]
}

fn slice_owner(
    changed_index: bool,
    elided: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = slice_source(changed_index, elided);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

fn slice_source(
    changed_index: bool,
    elided: bool,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let metadata = assignment(
                2,
                U64,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: value(1, SLICE_REF),
                },
            );
            if elided {
                vec![
                    block(
                        31,
                        vec![metadata],
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: value(2, U64),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    8,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                )],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                            )
                            .unwrap(),
                        },
                    ),
                    block(32, compare_index(), bounds_assertion(3)),
                    block(33, vec![], SemanticTerminatorKindV1::Return),
                    block(34, vec![read_slice()], SemanticTerminatorKindV1::Return),
                ]
            } else {
                let mut statements = vec![metadata];
                statements.extend(compare_index());
                let mut reads = Vec::new();
                if changed_index {
                    reads.push(assignment(
                        3,
                        U64,
                        SemanticRvalueKindV1::Use(constant(U64, 1, 8)),
                    ));
                }
                reads.push(read_slice());
                vec![
                    block(31, statements, bounds_assertion(1)),
                    block(32, reads, SemanticTerminatorKindV1::Return),
                ]
            }
        },
        |_| "slice_view_root".into(),
        &[U32],
    )
}

fn with_slice_owner(
    changed_index: bool,
    elided: bool,
    inspect: impl FnOnce(&ProductionPreRankedKirOwnerV1, &Inventory<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    with_owner(|budget| slice_owner(changed_index, elided, budget), inspect);
}

fn with_owner(
    build: impl FnOnce(&mut AssertOriginBudgetV1<'_>) -> ProductionPreRankedKirOwnerV1,
    inspect: impl FnOnce(&ProductionPreRankedKirOwnerV1, &Inventory<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = build(&mut budget);
    let payload = retained(&owner);
    budget.reserve_storage(payload).unwrap();
    let (inventory, storage) = Inventory::derive(owner.executable(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let incoming = budget.storage();
    inspect(&owner, &inventory, &mut budget);
    assert_eq!(budget.storage(), incoming);
    drop(inventory);
    budget.release_storage(storage.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(payload).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn read_binds_exact_entry_slice_extent_index_and_source_argument() {
    with_slice_owner(false, false, |owner, inventory, budget| {
        let (data, length, input) = owner
            .with_checked_slice_access_v1(inventory, site(1, 0, 0, 0), budget, |view| {
                assert_eq!(view.source().source_argument(), 0);
                assert!(view.source().source_path().is_empty());
                assert!(matches!(
                    view.input(),
                    Definition::FunctionArgument { argument: 0, .. }
                ));
                assert_eq!(view.memory().address_space, AddressSpace::Global);
                assert!(!view.memory().volatile);
                assert_eq!(view.loaded_type(), &Type::Scalar(ScalarType::U32));
                assert_eq!(view.access().effect, 0);
                assert_ne!(view.input(), view.index());
                Ok((view.data_carrier(), view.length_carrier(), view.input()))
            })
            .unwrap();
        let Definition::FunctionArgument { function, .. } = input else {
            unreachable!()
        };
        for carrier in [data, length] {
            let value = inventory
                .definitions()
                .iter()
                .find(|row| row.coordinate == carrier)
                .unwrap()
                .value
                .unwrap();
            assert_eq!(
                value_origin_v1::resolve_whole_value_origin_v1(
                    inventory,
                    owner.executable(),
                    function,
                    value,
                    budget,
                )
                .unwrap(),
                Some(input)
            );
        }
    });
}

fn control_owner(
    bypass: bool,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let mut statements = vec![assignment(
                2,
                U64,
                SemanticRvalueKindV1::Unary {
                    operation: SemanticUnaryOpV1::PointerMetadata,
                    operand: value(1, SLICE_REF),
                },
            )];
            statements.extend(compare_index());
            if bypass {
                vec![
                    block(
                        31,
                        statements,
                        SemanticTerminatorKindV1::SwitchInt {
                            discriminant: value(2, U64),
                            targets: SemanticSwitchTargetsV1::new(
                                vec![SemanticSwitchTargetV1::new(
                                    0,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                                )],
                                edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                            )
                            .unwrap(),
                        },
                    ),
                    block(32, vec![], bounds_assertion(2)),
                    block(33, vec![read_slice()], SemanticTerminatorKindV1::Return),
                ]
            } else {
                let mut assertion = bounds_assertion(1);
                let SemanticTerminatorKindV1::Assert { expected, .. } = &mut assertion else {
                    unreachable!()
                };
                *expected = false;
                vec![
                    block(31, statements, assertion),
                    block(32, vec![read_slice()], SemanticTerminatorKindV1::Return),
                ]
            }
        },
        |_| "slice_control_root".into(),
        &[U32],
    );
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

#[test]
fn bypass_predecessor_and_negative_assertion_polarity_do_not_authorize_reads() {
    for bypass in [false, true] {
        with_owner(
            |budget| control_owner(bypass, budget),
            |owner, inventory, budget| {
                let request = if bypass {
                    site(2, 0, 0, 1)
                } else {
                    site(1, 0, 0, 0)
                };
                let result: Result<(), _> =
                    owner.with_checked_slice_access_v1(inventory, request, budget, |_| {
                        panic!("the exact positive guard is required")
                    });
                let expected = if bypass {
                    "slice read requires the unique assertion-success predecessor"
                } else {
                    "slice read is not the selected positive assertion successor"
                };
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == expected),
                    "{result:?}"
                );
            },
        );
    }
}

fn two_slice_owner(
    data: u32,
    length: u32,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_symbol_and_slices(
        Fixture::ElidedBounds,
        false,
        |_, _| {
            let read = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(data),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE)
                        .unwrap(),
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(4)),
                        U32,
                    )
                    .unwrap(),
                ],
                U32,
            )
            .unwrap();
            vec![
                block(
                    31,
                    vec![
                        assignment(
                            3,
                            U64,
                            SemanticRvalueKindV1::Unary {
                                operation: SemanticUnaryOpV1::PointerMetadata,
                                operand: value(length, SLICE_REF),
                            },
                        ),
                        assignment(4, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                        assignment(
                            5,
                            BOOL,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::LessThan,
                                left: value(4, U64),
                                right: value(3, U64),
                            },
                        ),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: value(5, BOOL),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: value(3, U64),
                            index: value(4, U64),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    32,
                    vec![assignment(
                        6,
                        U32,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
                    )],
                    SemanticTerminatorKindV1::Return,
                ),
            ]
        },
        |_| "two_slice_view_root".into(),
        &[U32],
        2,
    );
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        budget,
    )
    .unwrap()
}

#[test]
fn same_typed_slice_inputs_cannot_exchange_data_and_extent() {
    for (data, length) in [(1, 1), (2, 2), (1, 2), (2, 1)] {
        with_owner(
            |budget| two_slice_owner(data, length, budget),
            |owner, inventory, budget| {
                let mut called = false;
                let result = owner.with_checked_slice_access_v1(
                    inventory,
                    site(1, 0, 0, 0),
                    budget,
                    |view| {
                        called = true;
                        assert_eq!(data, length);
                        assert_eq!(view.source().source_argument(), data - 1);
                        assert!(
                            matches!(view.input(), Definition::FunctionArgument { argument, .. }
                        if argument == data - 1)
                        );
                        Ok(())
                    },
                );
                assert_eq!(called, data == length);
                if data == length {
                    result.unwrap();
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                    ));
                }
            },
        );
    }
}

#[test]
fn reassigned_index_cannot_reuse_the_prior_assertion() {
    with_slice_owner(true, false, |owner, inventory, budget| {
        let result: Result<(), _> =
            owner.with_checked_slice_access_v1(inventory, site(1, 1, 0, 0), budget, |_| {
                panic!("a different index must not receive a checked view")
            });
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    });
}

#[test]
fn source_access_assertion_and_function_substitutions_are_rejected() {
    with_slice_owner(false, false, |owner, inventory, budget| {
        let root = SemanticFunctionIdV1::from_index(0);
        let other = SemanticFunctionIdV1::from_index(1);
        for request in [
            site(1, 0, 1, 0),
            site(1, 1, 0, 0),
            site(2, 0, 0, 0),
            site(1, 0, 0, 1),
            ProductionSliceAccessSiteV1::new(
                other,
                root,
                SemanticBlockIdV1::from_index(1),
                Some(0),
                0,
                SemanticBlockIdV1::from_index(0),
            ),
            ProductionSliceAccessSiteV1::new(
                root,
                other,
                SemanticBlockIdV1::from_index(1),
                Some(0),
                0,
                SemanticBlockIdV1::from_index(0),
            ),
        ] {
            let result: Result<(), _> =
                owner.with_checked_slice_access_v1(inventory, request, budget, |_| {
                    panic!("a substituted locator must not receive a checked view")
                });
            if request == site(1, 0, 0, 1) {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::AssertOrigin(
                    SemanticKirAssertOriginErrorV1::MissingBinding { site: actual },
                )) if actual == SemanticKirAssertSiteV1::new(root, root, SemanticBlockIdV1::from_index(1)))
                );
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                ));
            }
        }
    });
}

#[test]
fn identical_but_foreign_inventory_is_rejected() {
    with_slice_owner(false, false, |owner, _, budget| {
        let other = slice_owner(false, false, budget);
        let payload = retained(&other);
        budget.reserve_storage(payload).unwrap();
        let (inventory, storage) = Inventory::derive(other.executable(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let result: Result<(), _> =
            owner.with_checked_slice_access_v1(&inventory, site(1, 0, 0, 0), budget, |_| {
                panic!("another owner's inventory must not receive a checked view")
            });
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        drop(inventory);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(other);
        budget.release_storage(payload).unwrap();
    });
}

#[test]
fn elided_assertion_does_not_fabricate_an_emitted_comparison() {
    with_slice_owner(false, true, |owner, inventory, budget| {
        let root = SemanticFunctionIdV1::from_index(0);
        assert!(matches!(
            owner
                .assert_origins()
                .assert_condition(root, root, SemanticBlockIdV1::from_index(1), budget,)
                .unwrap()
                .outcome(),
            SemanticKirAssertConditionOutcomeV1::ElidedByExistingRule { .. }
        ));
        let result: Result<(), _> =
            owner.with_checked_slice_access_v1(inventory, site(3, 0, 0, 1), budget, |_| {
                panic!("elided assertions are not supported by this query")
            });
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "slice assertion was elided by an existing rule",
                ..
            })
        ));
    });
}

#[test]
fn callback_failure_restores_temporary_storage() {
    with_slice_owner(false, false, |owner, inventory, budget| {
        let mut called = false;
        let result =
            owner.with_checked_slice_access_v1(inventory, site(1, 0, 0, 0), budget, |_| {
                called = true;
                Err::<(), _>(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            });
        assert!(called);
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    });
}

#[test]
fn exact_and_one_short_query_budgets_preserve_the_incoming_storage_floor() {
    with_slice_owner(false, false, |owner, inventory, outer| {
        let floor = outer.storage();
        let measure = |work_limit, storage_limit| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = owner.with_checked_slice_access_v1(
                inventory,
                site(1, 0, 0, 0),
                &mut budget,
                |_| Ok(()),
            );
            assert_eq!(budget.storage(), floor);
            (
                result,
                budget.work(),
                budget.peak_storage(),
                budget.failed_storage(),
            )
        };
        let (result, work, peak, _) = measure(WORK, STORAGE);
        result.unwrap();
        assert!(work > 0 && peak > floor);
        assert!(measure(work, peak).0.is_ok());
        for limit in [0, work - 1] {
            assert!(matches!(
                measure(limit, peak).0,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Work(_))
                )
            ));
        }
        let (result, _, _, failed) = measure(work, peak - 1);
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(Resource::Storage(_)))
        ));
        assert_eq!(failed, Some(peak));
    });
}

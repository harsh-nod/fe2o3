use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

fn place(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![],
        SemanticTypeIdV1::from_index(local),
    )
    .unwrap()
}
fn statement(kind: SemanticStatementKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(SemanticSourceProvenanceV1::unavailable(), kind)
}
fn borrow(kind: SemanticBorrowKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(2),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(2),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: place(1),
            },
        ),
    )))
}
fn fixture(
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([1; 32]),
        SemanticLayoutIdentityV1::from_sha256([2; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        0,
        vec![],
        SemanticTypeIdV1::from_index(0),
        vec![],
        SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticAbiPassModeV1::Ignore,
        ),
    )
    .unwrap();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([3; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([4; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([5; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([6; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([7; 32]),
        source,
        abi,
        (0..3)
            .map(|i| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([10 + i as u8; 32]),
                    SemanticTypeIdV1::from_index(i),
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([20; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, term),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}
fn observe(
    function: &SemanticFunctionDeclV1,
    remaining: &mut usize,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    only_shared(
        function,
        SemanticLocalIdV1::from_index(1),
        &mut |work, words| {
            assert_eq!(words, 0, "allocation-free original-use inventory");
            *remaining = remaining
                .checked_sub(work)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            Ok(())
        },
    )
}
#[test]
fn guarded_grid_erased_owner_shared_borrows_do_not_consume_or_recreate_a_move() {
    let f = fixture(
        vec![
            borrow(SemanticBorrowKindV1::Shared),
            borrow(SemanticBorrowKindV1::Shared),
        ],
        SemanticTerminatorKindV1::Return,
    );
    assert_eq!(observe(&f, &mut 1000), Ok(true));
    let moved = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(0),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1))),
        ),
    )));
    for first in [true, false] {
        let statements = if first {
            vec![moved.clone(), borrow(SemanticBorrowKindV1::Shared)]
        } else {
            vec![
                borrow(SemanticBorrowKindV1::Shared),
                moved.clone(),
                borrow(SemanticBorrowKindV1::Shared),
            ]
        };
        assert_eq!(
            observe(
                &fixture(statements, SemanticTerminatorKindV1::Return),
                &mut 1000
            ),
            Ok(false)
        );
    }
}
#[test]
fn guarded_grid_erased_owner_lifetime_projected_write_and_mutable_borrow_reject() {
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(0),
                SemanticTypeIdV1::from_index(0),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap();
    for bad in [
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::Deinitialize(place(1))),
        borrow(SemanticBorrowKindV1::Mutable),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            projected,
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1::from_index(0),
                SemanticConstantValueV1::ZeroSized,
            )),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
    ] {
        let f = fixture(
            vec![
                borrow(SemanticBorrowKindV1::Shared),
                bad,
                borrow(SemanticBorrowKindV1::Shared),
            ],
            SemanticTerminatorKindV1::Return,
        );
        assert_eq!(observe(&f, &mut 1000), Ok(false));
    }
}
#[test]
fn guarded_grid_erased_owner_drop_and_call_escape_reject() {
    let edge = SemanticControlFlowEdgeV1::new(
        SemanticEdgeRoleV1::DropReturn,
        SemanticBlockIdV1::from_index(0),
    );
    let drop = SemanticTerminatorKindV1::Drop {
        place: place(1),
        drop_glue: SemanticFunctionIdV1::from_index(1),
        target: edge,
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(1),
            vec![SemanticOperandV1::Move(place(1))],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    for term in [drop, call] {
        assert_eq!(
            observe(
                &fixture(vec![borrow(SemanticBorrowKindV1::Shared)], term),
                &mut 1000
            ),
            Ok(false)
        );
    }
}
#[test]
fn guarded_grid_erased_owner_work_is_shared_and_exactly_bounded() {
    let f = fixture(
        vec![borrow(SemanticBorrowKindV1::Shared)],
        SemanticTerminatorKindV1::Return,
    );
    let mut remaining = 1000;
    assert_eq!(observe(&f, &mut remaining), Ok(true));
    let spent = 1000 - remaining;
    assert!(spent > 0);
    let mut exact = spent;
    assert_eq!(observe(&f, &mut exact), Ok(true));
    assert_eq!(exact, 0);
    assert_eq!(
        observe(&f, &mut exact),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
    assert_eq!(
        observe(&f, &mut (spent - 1)),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
}
fn initialize_owner() -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(1),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, vec![]).unwrap(),
            ),
        ),
    )))
}
fn initialized_observe(
    function: &SemanticFunctionDeclV1,
    remaining: &mut usize,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    initialized_shared(
        function,
        SemanticLocalIdV1::from_index(1),
        &mut |work, words| {
            assert_eq!(words, 0);
            *remaining = remaining
                .checked_sub(work)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            Ok(())
        },
    )
}
#[test]
fn guarded_grid_defined_owner_does_not_relax_erased_owner_custody() {
    let f = fixture(
        vec![initialize_owner(), borrow(SemanticBorrowKindV1::Shared)],
        SemanticTerminatorKindV1::Return,
    );
    assert_eq!(initialized_observe(&f, &mut 1000), Ok(true));
    assert_eq!(observe(&f, &mut 1000), Ok(false));
    let missing = fixture(
        vec![borrow(SemanticBorrowKindV1::Shared)],
        SemanticTerminatorKindV1::Return,
    );
    assert_eq!(initialized_observe(&missing, &mut 1000), Ok(false));
    assert_eq!(observe(&missing, &mut 1000), Ok(true));
}
#[test]
fn guarded_grid_defined_owner_reassignment_deinit_and_escape_reject() {
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(0),
                SemanticTypeIdV1::from_index(0),
            )
            .unwrap(),
        ],
        SemanticTypeIdV1::from_index(0),
    )
    .unwrap();
    for bad in [
        initialize_owner(),
        statement(SemanticStatementKindV1::Deinitialize(place(1))),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        borrow(SemanticBorrowKindV1::Mutable),
        statement(SemanticStatementKindV1::SetDiscriminant {
            place: place(1),
            variant_index: 0,
        }),
        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            projected,
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                SemanticTypeIdV1::from_index(0),
                SemanticConstantValueV1::ZeroSized,
            )),
            SemanticVolatilityV1::NonVolatile,
            None,
        ))),
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1))),
            ),
        ))),
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0),
            SemanticRvalueV1::new(
                SemanticTypeIdV1::from_index(1),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1))),
            ),
        ))),
    ] {
        let f = fixture(
            vec![
                initialize_owner(),
                borrow(SemanticBorrowKindV1::Shared),
                bad,
                borrow(SemanticBorrowKindV1::Shared),
            ],
            SemanticTerminatorKindV1::Return,
        );
        assert_eq!(initialized_observe(&f, &mut 1000), Ok(false));
    }
}
#[test]
fn guarded_grid_defined_owner_self_initialization_is_not_a_definition() {
    let self_copy = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(1),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1))),
        ),
    )));
    let f = fixture(
        vec![self_copy, borrow(SemanticBorrowKindV1::Shared)],
        SemanticTerminatorKindV1::Return,
    );
    assert_eq!(initialized_observe(&f, &mut 1000), Ok(false));
}
#[test]
fn guarded_grid_defined_owner_keeps_shared_budget_exact() {
    let f = fixture(
        vec![initialize_owner(), borrow(SemanticBorrowKindV1::Shared)],
        SemanticTerminatorKindV1::Return,
    );
    let mut remaining = 1000;
    assert_eq!(initialized_observe(&f, &mut remaining), Ok(true));
    let spent = 1000 - remaining;
    assert_eq!(
        initialized_observe(&f, &mut (spent - 1)),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
    let mut exact = spent;
    assert_eq!(initialized_observe(&f, &mut exact), Ok(true));
    assert_eq!(exact, 0);
    assert_eq!(
        initialized_observe(&f, &mut exact),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
}

#[test]
fn guarded_grid_defined_owner_requires_exact_normal_call_definition() {
    for mutation in 0..5 {
        let destination = if mutation == 4 {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![],
                SemanticTypeIdV1::from_index(0),
            )
            .unwrap()
        } else {
            place(1)
        };
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                destination,
                SemanticControlFlowEdgeV1::new(
                    if mutation == 2 {
                        SemanticEdgeRoleV1::Goto
                    } else {
                        SemanticEdgeRoleV1::CallReturn
                    },
                    SemanticBlockIdV1::from_index(0),
                ),
            )),
            if mutation == 1 {
                SemanticUnwindActionV1::Continue
            } else {
                SemanticUnwindActionV1::Unreachable
            },
        )
        .unwrap();
        let statements = if mutation == 3 {
            vec![initialize_owner()]
        } else {
            vec![]
        };
        let f = fixture(statements, SemanticTerminatorKindV1::Call(call));
        assert_eq!(
            initialized_observe(&f, &mut 1000),
            Ok(mutation == 0),
            "mutation {mutation}"
        );
        assert_eq!(observe(&f, &mut 1000), Ok(false));
    }
}

#[test]
fn guarded_grid_defined_owner_charges_each_aggregate_operand() {
    let constants = vec![
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            SemanticTypeIdV1::from_index(0),
            SemanticConstantValueV1::ZeroSized
        ));
        257
    ];
    let assign = statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(1),
        SemanticRvalueV1::new(
            SemanticTypeIdV1::from_index(1),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(SemanticAggregateKindV1::Aggregate, constants)
                    .unwrap(),
            ),
        ),
    )));
    let f = fixture(
        vec![assign, borrow(SemanticBorrowKindV1::Shared)],
        SemanticTerminatorKindV1::Return,
    );
    let mut left = 1000;
    assert_eq!(initialized_observe(&f, &mut left), Ok(true));
    let spent = 1000 - left;
    assert!(spent > 257);
    assert_eq!(
        initialized_observe(&f, &mut (spent - 1)),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    );
}

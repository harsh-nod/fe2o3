fn reuse_invalidation_rows() -> Vec<(SemanticStatementKindV1, bool)> {
    use SemanticRvalueKindV1 as R;
    use SemanticStatementKindV1 as S;
    let moved = || SemanticOperandV1::Move(place(1));
    let copied = || SemanticOperandV1::Copy(place(1));
    let assign_kind = |kind| {
        S::Assign(SemanticAssignmentV1::new(
            place(3),
            SemanticRvalueV1::new(SemanticTypeIdV1::from_index(1), kind),
        ))
    };
    let mut rows = vec![
        (S::Nop, false),
        (S::StorageLive(SemanticLocalIdV1::from_index(1)), true),
        (S::StorageDead(SemanticLocalIdV1::from_index(1)), true),
        (S::Deinitialize(place(1)), true),
        (
            S::SetDiscriminant {
                place: place(1),
                variant_index: 0,
            },
            true,
        ),
        (S::Assume(moved()), true),
        (S::Assume(copied()), false),
        (assign(1, None).kind().clone(), true),
        (assign_kind(R::Use(moved())), true),
        (assign_kind(R::Use(copied())), false),
        (
            assign_kind(R::Unary {
                operation: SemanticUnaryOpV1::Not,
                operand: moved(),
            }),
            true,
        ),
        (
            assign_kind(R::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: moved(),
            }),
            true,
        ),
        (
            assign_kind(R::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(1),
            }),
            false,
        ),
        (
            assign_kind(R::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(1),
            }),
            true,
        ),
        (
            assign_kind(R::Borrow {
                kind: SemanticBorrowKindV1::Fake,
                place: place(1),
            }),
            true,
        ),
        (
            assign_kind(R::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: place(1),
            }),
            true,
        ),
        (
            assign_kind(R::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(1),
            }),
            true,
        ),
        (assign_kind(R::Length(place(1))), false),
        (assign_kind(R::Discriminant(place(1))), false),
        (
            assign_kind(R::Load(SemanticMemoryLoadV1::new(
                place(1),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
            false,
        ),
        (
            assign_kind(
                R::aggregate(SemanticAggregateKindV1::Tuple, vec![copied(), moved()]).unwrap(),
            ),
            true,
        ),
    ];
    for (left, right) in [
        (moved(), copied()),
        (copied(), moved()),
        (copied(), copied()),
    ] {
        let invalid = matches!(left, SemanticOperandV1::Move(_))
            || matches!(right, SemanticOperandV1::Move(_));
        rows.push((
            assign_kind(R::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: left.clone(),
                right: right.clone(),
            }),
            invalid,
        ));
        rows.push((
            assign_kind(R::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                SemanticCheckedBinaryOpV1::Add,
                left.clone(),
                right.clone(),
            ))),
            invalid,
        ));
        rows.push((
            assign_kind(R::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                SemanticUncheckedBinaryOpV1::Add,
                left,
                right,
            ))),
            invalid,
        ));
    }
    let access = SemanticAtomicAccessV1::new(
        SemanticAtomicOrderingV1::Relaxed,
        SemanticAtomicScopeV1::Workgroup,
    );
    for target in [1, 3] {
        rows.push((
            S::Store(SemanticMemoryStoreV1::new(
                place(target),
                copied(),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
            target == 1,
        ));
    }
    rows.push((
        S::Store(SemanticMemoryStoreV1::new(
            place(3),
            moved(),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
        true,
    ));
    for (destination, address, operand, invalid) in [
        (1, 3, copied(), true),
        (3, 1, copied(), true),
        (3, 3, moved(), true),
        (3, 3, copied(), false),
    ] {
        rows.push((
            S::AtomicRmw(SemanticAtomicRmwV1::new(
                place(destination),
                place(address),
                operand,
                SemanticAtomicRmwOpV1::Add,
                access,
            )),
            invalid,
        ));
    }
    for (destination, address, expected, replacement, invalid) in [
        (1, 3, copied(), copied(), true),
        (3, 1, copied(), copied(), true),
        (3, 3, moved(), copied(), true),
        (3, 3, copied(), moved(), true),
        (3, 3, copied(), copied(), false),
    ] {
        rows.push((
            S::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                place(destination),
                place(address),
                expected,
                replacement,
                access,
                SemanticAtomicOrderingV1::Relaxed,
                false,
            )),
            invalid,
        ));
    }
    rows
}

#[test]
fn capability_reuse_cold_oracle_covers_all_statement_invalidation_forms() {
    // Perturb the independent source scanner against a fixed scalar SSA-use
    // scaffold. These borrow/store forms are not scalar MIR admission claims.
    let scaffold = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            statement(SemanticStatementKindV1::Nop),
        ],
        None,
    )]);
    let plan = plan(&scaffold);
    for (kind, expected) in reuse_invalidation_rows() {
        assert_eq!(invalidates(&kind, 1), expected, "{kind:?}");
        assert_eq!(reference_invalidates(&kind, 1), expected, "{kind:?}");
        let body = body(vec![block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
                statement(kind),
            ],
            None,
        )]);
        let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let value = cached.use_value(0, 1).unwrap();
        for end in [Some(2), None] {
            let consumer = reuse_consumer(0, end);
            let actual = cached.loan_live(loan(value, 1), consumer);
            assert_eq!(actual.is_err(), expected && end.is_none());
            reuse_assert_same(
                actual,
                reference.loan_live_reference(loan(value, 1), consumer),
            );
        }
    }
}

#[test]
fn capability_reuse_terminal_call_and_drop_checks_remain_ordered() {
    let scaffold = reuse_body(&[vec![1], vec![2], vec![]]);
    let plan = plan(&scaffold);
    let edge = |role| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(2));
    let call = |operand, destination| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![operand],
                Some(SemanticCallDestinationV1::new(
                    place(destination),
                    edge(SemanticEdgeRoleV1::CallReturn),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    for (terminator, expected) in [
        (call(SemanticOperandV1::Move(place(1)), 3), true),
        (call(SemanticOperandV1::Copy(place(1)), 1), true),
        (call(SemanticOperandV1::Copy(place(1)), 3), false),
        (
            SemanticTerminatorKindV1::Drop {
                place: place(1),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: edge(SemanticEdgeRoleV1::DropReturn),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            true,
        ),
        (
            SemanticTerminatorKindV1::Drop {
                place: place(3),
                drop_glue: SemanticFunctionIdV1::from_index(0),
                target: edge(SemanticEdgeRoleV1::DropReturn),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
            false,
        ),
    ] {
        let mut blocks = scaffold.blocks().to_vec();
        blocks[1] = SemanticBasicBlockV1::new(
            blocks[1].identity(),
            blocks[1].source(),
            blocks[1].statements().to_vec(),
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap();
        let body = body(blocks);
        let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let value = cached.use_value(0, 1).unwrap();
        for consumer in [
            reuse_consumer(1, Some(0)),
            reuse_consumer(1, None),
            reuse_consumer(2, Some(0)),
        ] {
            let actual = cached.loan_live(loan(value, 1), consumer);
            assert_eq!(
                actual.is_err(),
                expected && consumer != reuse_consumer(1, Some(0))
            );
            reuse_assert_same(
                actual,
                reference.loan_live_reference(loan(value, 1), consumer),
            );
        }
    }
}

#[test]
fn capability_reuse_same_local_different_block_versions_do_not_alias() {
    let body = body(vec![
        block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            ],
            Some(1),
        ),
        block(
            1,
            vec![
                assign(1, None),
                assign(3, Some(SemanticOperandV1::Copy(place(1)))),
            ],
            None,
        ),
    ]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let first = graph.use_value(0, 1).unwrap();
    let second = graph.use_value(1, 1).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        graph.definition(first).unwrap(),
        CapabilityDefinitionSiteV1 {
            block: 0,
            statement: Some(0),
            local: 1,
        }
    );
    assert_eq!(
        graph.definition(second).unwrap(),
        CapabilityDefinitionSiteV1 {
            block: 1,
            statement: Some(0),
            local: 1,
        }
    );
    assert_eq!(graph.use_value(0, 1).unwrap(), first);
    assert_eq!(graph.use_value(1, 1).unwrap(), second);
    assert_eq!(graph.reuse.definitions.len(), 2);
}

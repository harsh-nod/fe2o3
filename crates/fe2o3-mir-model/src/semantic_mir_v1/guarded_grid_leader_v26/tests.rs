use super::*;

#[path = "tests/variable_work.rs"]
mod variable_work;

fn t(i: u32) -> SemanticTypeIdV1 {
    SemanticTypeIdV1::from_index(i)
}
fn l(i: u32) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(i)
}
fn b(i: u32) -> SemanticBlockIdV1 {
    SemanticBlockIdV1::from_index(i)
}
fn types() -> SemanticGuardedGridLeaderTypesV1 {
    SemanticGuardedGridLeaderTypesV1::new([t(0), t(1), t(2), t(3), t(4), t(5), t(6), t(7)])
}
fn p(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(l(local), vec![], ty).unwrap()
}
fn edge(role: SemanticEdgeRoleV1, to: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, b(to))
}
fn goto(to: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, to))
}
fn constant(ty: SemanticTypeIdV1, value: u128, size: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, size).unwrap()),
    ))
}
fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            p(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    term: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), term),
    )
    .unwrap()
}
fn attributes() -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap()
}
fn abi(
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
    ignore: bool,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc_with_source_signature(
        SemanticAbiIdentityV1::from_sha256([12; 32]),
        SemanticLayoutIdentityV1::from_sha256([13; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        inputs.len() as u32,
        inputs.to_vec(),
        output,
        inputs
            .iter()
            .map(|ty| {
                SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    *ty,
                    SemanticAbiPassModeV1::Direct(attributes()),
                ))
            })
            .collect(),
        SemanticAbiValueV1::new(
            output,
            if ignore {
                SemanticAbiPassModeV1::Ignore
            } else {
                SemanticAbiPassModeV1::Direct(attributes())
            },
        ),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::SharedBorrow;
        inputs.len()
    ])
    .unwrap()
}
fn function(
    id: u8,
    abi: SemanticFunctionAbiV1,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    entry: u32,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let s = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([id; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([id; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([id; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([id; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([id; 32]),
        s,
        abi,
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([i as u8 + 1; 32]),
                    *ty,
                    *role,
                    s,
                )
            })
            .collect(),
        b(entry),
        blocks,
    )
    .unwrap()
}
fn switch(local: u32, ty: SemanticTypeIdV1, zero: u32, otherwise: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Move(p(local, ty)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                edge(SemanticEdgeRoleV1::SwitchValue, zero),
            )],
            edge(SemanticEdgeRoleV1::SwitchOtherwise, otherwise),
        )
        .unwrap(),
    }
}
fn call(
    callee: u32,
    args: Vec<SemanticOperandV1>,
    destination: u32,
    ty: SemanticTypeIdV1,
    to: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            args,
            Some(SemanticCallDestinationV1::new(
                p(destination, ty),
                edge(SemanticEdgeRoleV1::CallReturn, to),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}
fn getter(order: [u32; 5], mutation: u8) -> SemanticFunctionDeclV1 {
    let t = types();
    let [guard, issuer, some, none, exit] = order;
    let read = SemanticPlaceV1::new(
        l(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, t.grid).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Field(if mutation == 1 { 0 } else { 1 }),
                t.rank,
            )
            .unwrap(),
        ],
        t.rank,
    )
    .unwrap();
    let mut statements = vec![
        assign(
            3,
            t.rank,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(read)),
        ),
        assign(
            2,
            t.boolean,
            SemanticRvalueKindV1::Binary {
                operation: if mutation == 2 {
                    SemanticBinaryOpV1::NotEqual
                } else {
                    SemanticBinaryOpV1::Equal
                },
                left: SemanticOperandV1::Move(p(3, t.rank)),
                right: constant(
                    if mutation == 3 { t.boolean } else { t.rank },
                    if mutation == 4 { 1 } else { 0 },
                    if mutation == 15 { 4 } else { 8 },
                ),
            },
        ),
    ];
    if mutation == 5 {
        statements.pop();
    }
    if mutation == 6 {
        statements.push(assign(
            3,
            t.rank,
            SemanticRvalueKindV1::Use(constant(t.rank, 1, 8)),
        ));
    }
    let variant = |tag: u32| {
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::EnumVariant(tag),
                if tag == 1 {
                    vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                        if mutation == 7 { t.grid } else { t.leader },
                        SemanticConstantValueV1::ZeroSized,
                    ))]
                } else {
                    vec![]
                },
            )
            .unwrap(),
        )
    };
    let mut blocks = vec![None; 5];
    blocks[guard as usize] = Some(block(
        1,
        statements,
        if mutation == 8 {
            goto(issuer)
        } else {
            switch(
                2,
                t.boolean,
                if mutation == 9 { issuer } else { none },
                if mutation == 9 { none } else { issuer },
            )
        },
    ));
    blocks[issuer as usize] = Some(block(
        2,
        vec![],
        call(
            1,
            if mutation == 10 {
                vec![constant(t.rank, 0, 8)]
            } else {
                vec![]
            },
            4,
            t.leader,
            if mutation == 11 { none } else { some },
        ),
    ));
    blocks[some as usize] = Some(block(
        3,
        vec![assign(0, t.leader_option, variant(1))],
        goto(if mutation == 12 { guard } else { exit }),
    ));
    blocks[none as usize] = Some(block(
        4,
        vec![assign(
            0,
            t.leader_option,
            variant(if mutation == 13 { 1 } else { 0 }),
        )],
        goto(exit),
    ));
    blocks[exit as usize] = Some(block(
        5,
        vec![],
        if mutation == 14 {
            goto(some)
        } else {
            SemanticTerminatorKindV1::Return
        },
    ));
    function(
        1,
        abi(&[t.grid_reference], t.leader_option, false),
        &[
            (t.leader_option, SemanticLocalRoleV1::Return),
            (t.grid_reference, SemanticLocalRoleV1::Argument(0)),
            (t.boolean, SemanticLocalRoleV1::Temporary),
            (t.rank, SemanticLocalRoleV1::Temporary),
            (t.leader, SemanticLocalRoleV1::Temporary),
        ],
        guard,
        blocks.into_iter().map(Option::unwrap).collect(),
    )
}

#[test]
fn guarded_grid_original_five_block_recipe_and_role_permutations() {
    for order in [[0, 1, 2, 3, 4], [3, 0, 4, 1, 2], [4, 3, 2, 1, 0]] {
        let r = body::observe(&getter(order, 0), types()).expect("all original roles retained");
        assert_eq!(
            [
                r.guard.index(),
                r.issuer.index(),
                r.some.index(),
                r.none.index(),
                r.exit.index()
            ],
            order
        );
    }
}
#[test]
fn guarded_grid_guard_payload_issuer_and_bypass_mutations_reject() {
    for mutation in 1..=15 {
        assert_eq!(
            body::observe(&getter([0, 1, 2, 3, 4], mutation), types()),
            None,
            "mutation {mutation}"
        );
    }
}
#[test]
fn guarded_grid_empty_issuer_is_not_generic_zst_initialization() {
    let t = types();
    let correct = function(
        2,
        abi(&[], t.leader, true),
        &[(t.leader, SemanticLocalRoleV1::Return)],
        0,
        vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
    );
    assert!(body::issuer_matches(&correct, t.leader));
    assert!(!body::issuer_matches(&correct, t.grid));
    let wrong = function(
        2,
        abi(&[], t.leader, true),
        &[(t.leader, SemanticLocalRoleV1::Return)],
        0,
        vec![block(
            1,
            vec![SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Nop,
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    assert!(!body::issuer_matches(&wrong, t.leader));
    let wrong_abi = function(
        2,
        abi(&[], t.leader, false),
        &[(t.leader, SemanticLocalRoleV1::Return)],
        0,
        vec![block(1, vec![], SemanticTerminatorKindV1::Return)],
    );
    assert!(!body::issuer_matches(&wrong_abi, t.leader));
}

fn caller(mutation: u8) -> SemanticFunctionDeclV1 {
    let t = types();
    let payload = SemanticPlaceV1::new(
        l(2),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Downcast(if mutation == 1 { 0 } else { 1 }),
                t.grid_option,
            )
            .unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), t.grid).unwrap(),
        ],
        t.grid,
    )
    .unwrap();
    let mut transfer = vec![
        assign(
            4,
            t.grid,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(payload)),
        ),
        assign(
            5,
            t.grid_reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: p(if mutation == 2 { 7 } else { 4 }, t.grid),
            },
        ),
    ];
    if mutation == 3 {
        transfer.insert(
            1,
            SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Deinitialize(p(4, t.grid)),
            ),
        );
    }
    if mutation == 4 {
        transfer.push(assign(
            5,
            t.grid_reference,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: p(4, t.grid),
            },
        ));
    }
    if mutation == 5 {
        transfer.push(assign(
            7,
            t.grid,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(p(4, t.grid))),
        ));
    }
    if mutation == 6 {
        transfer.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageDead(l(4)),
        ));
    }
    if mutation == 8 {
        transfer.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                p(7, t.grid),
                SemanticOperandV1::Move(p(4, t.grid)),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ));
    }
    if mutation == 9 {
        transfer.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(p(5, t.grid_reference))),
        ));
    }
    let exit = match mutation {
        10 => SemanticTerminatorKindV1::Assert {
            condition: constant(t.boolean, 1, 1),
            expected: true,
            message: SemanticAssertMessageV1::NullPointerDereference,
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        11 => SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(p(5, t.grid_reference)),
            targets: SemanticSwitchTargetsV1::new(
                vec![],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
            )
            .unwrap(),
        },
        12 => SemanticTerminatorKindV1::Drop {
            place: p(4, t.grid),
            drop_glue: SemanticFunctionIdV1::from_index(4),
            target: edge(SemanticEdgeRoleV1::DropReturn, 3),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        13 => SemanticTerminatorKindV1::Assert {
            condition: constant(t.boolean, 1, 1),
            expected: true,
            message: SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Copy(p(
                5,
                t.grid_reference,
            ))),
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 3),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        _ => SemanticTerminatorKindV1::Return,
    };
    let blocks = vec![
        block(
            1,
            vec![],
            call(
                3,
                vec![SemanticOperandV1::Copy(p(1, t.context_reference))],
                2,
                t.grid_option,
                1,
            ),
        ),
        block(
            2,
            vec![assign(
                3,
                t.rank,
                SemanticRvalueKindV1::Discriminant(p(2, t.grid_option)),
            )],
            switch(
                3,
                t.rank,
                if mutation == 7 { 2 } else { 3 },
                if mutation == 7 { 3 } else { 2 },
            ),
        ),
        block(
            3,
            transfer,
            call(
                0,
                vec![SemanticOperandV1::Move(p(5, t.grid_reference))],
                6,
                t.leader_option,
                3,
            ),
        ),
        block(4, vec![], exit),
    ];
    function(
        3,
        abi(&[t.context_reference], t.leader_option, false),
        &[
            (t.leader_option, SemanticLocalRoleV1::Return),
            (t.context_reference, SemanticLocalRoleV1::Argument(0)),
            (t.grid_option, SemanticLocalRoleV1::Temporary),
            (t.rank, SemanticLocalRoleV1::Temporary),
            (t.grid, SemanticLocalRoleV1::Temporary),
            (t.grid_reference, SemanticLocalRoleV1::Temporary),
            (t.leader_option, SemanticLocalRoleV1::Temporary),
            (t.grid, SemanticLocalRoleV1::Temporary),
        ],
        0,
        blocks,
    )
}
fn observe_receiver(
    f: &SemanticFunctionDeclV1,
    work: &mut u64,
) -> Result<receiver::Receiver, SemanticMirErrorV1> {
    receiver::observe(
        f,
        SemanticFunctionIdV1::from_index(0),
        SemanticGuardedGridLeaderSourceV1 {
            caller: SemanticFunctionIdV1::from_index(2),
            call_block: b(2),
            grid_getter: SemanticFunctionIdV1::from_index(3),
            grid_current: SemanticFunctionIdV1::from_index(4),
            grid_call_block: b(0),
        },
        types(),
        &(0..5)
            .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i)))
            .collect::<Vec<_>>(),
        work,
    )
}
#[test]
fn guarded_grid_receiver_uses_actual_option_and_shared_borrow() {
    let mut work = 1_000_000;
    let r = observe_receiver(&caller(0), &mut work).unwrap();
    assert_eq!((r.grid_option, r.grid, r.receiver), (l(2), l(4), l(5)));
    assert!(work < 1_000_000);
}
#[test]
fn guarded_grid_receiver_kills_alternate_receiver_and_inverted_option_reject() {
    for mutation in 1..=7 {
        assert_eq!(
            observe_receiver(&caller(mutation), &mut 1_000_000),
            Err(SemanticMirErrorV1::InvalidFunctionAbi),
            "mutation {mutation}"
        );
    }
}
#[test]
fn guarded_grid_receiver_charges_existing_work() {
    let mut work = 0;
    assert_eq!(
        observe_receiver(&caller(0), &mut work),
        Err(SemanticMirErrorV1::InvalidFunctionAbi)
    );
    assert_eq!(work, 0);
}

#[test]
fn guarded_grid_receiver_rejects_store_and_noncall_escapes() {
    for mutation in [8, 9, 11, 12, 13] {
        assert_eq!(
            observe_receiver(&caller(mutation), &mut 1_000_000),
            Err(SemanticMirErrorV1::InvalidFunctionAbi),
            "mutation {mutation}"
        );
    }
    assert!(
        observe_receiver(&caller(10), &mut 1_000_000).is_ok(),
        "unrelated retained assert is not a source authority premise"
    );
}

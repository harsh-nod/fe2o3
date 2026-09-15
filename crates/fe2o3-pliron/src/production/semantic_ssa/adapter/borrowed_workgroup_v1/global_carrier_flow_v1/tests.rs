use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;
#[path = "../global_statement_index_v1/fixture.rs"]
mod fixture;
use fixture::{assignment, callable, place, projection, ty};

#[path = "relevance_tests.rs"]
mod relevance_tests;
#[path = "coverage118_tests.rs"]
mod coverage118_tests;
#[path = "mixed_siblings118_tests.rs"]
mod mixed_siblings118_tests;

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn types() -> Vec<SemanticTypeDeclV1> {
    let mut types = fixture::types();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([90; 32]),
        SemanticLayoutIdentityV1::from_sha256([90; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![ty(7), ty(7), ty(2)]).unwrap(),
        ),
    ));
    types
}

fn projected(local: u32, field: u32, result: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![projection(
            SemanticProjectionKindV1::Field(field),
            ty(result),
        )],
        ty(result),
    )
    .unwrap()
}

fn borrow(reference: u32, owner: u32) -> SemanticStatementKindV1 {
    assignment(
        place(reference, ty(7)),
        ty(7),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: place(owner, ty(6)),
        },
    )
}

fn copy(destination: u32, result: u32, source: SemanticPlaceV1) -> SemanticStatementKindV1 {
    assignment(
        place(destination, ty(result)),
        ty(result),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)),
    )
}

fn body(statements: Vec<SemanticStatementKindV1>) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let abi_value = |ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore);
    let abi = SemanticFunctionAbiV1::new(
        SemanticAbiIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
        SemanticCanonAbiV1::Rust,
        false,
        false,
        vec![abi_value(ty(6)), abi_value(ty(6)), abi_value(ty(2))],
        abi_value(ty(0)),
    )
    .unwrap();
    // Two actual non-ZST Global owner types, not scalar stand-ins. This is a
    // classifier component fixture, not canonical kernel or issuer authority.
    let locals = [0, 6, 6, 2, 7, 7, 30, 30, 7, 7, 8, 7, 5, 7]
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([100 + i as u8; 32]),
                ty(t),
                match i {
                    0 => SemanticLocalRoleV1::Return,
                    1..=3 => SemanticLocalRoleV1::Argument(i as u32 - 1),
                    _ => SemanticLocalRoleV1::Temporary,
                },
                source,
            )
        })
        .collect();
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([92; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([92; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([92; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([92; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([92; 32]),
        source,
        abi,
        locals,
        SemanticBlockIdV1::from_index(0),
        vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([93; 32]),
                source,
                statements
                    .into_iter()
                    .map(|s| SemanticStatementV1::new(source, s))
                    .collect(),
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

fn source_statements() -> Vec<SemanticStatementKindV1> {
    let capture = assignment(
        place(6, ty(30)),
        ty(30),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Tuple,
                vec![
                    SemanticOperandV1::Copy(place(4, ty(7))),
                    SemanticOperandV1::Copy(place(5, ty(7))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                ],
            )
            .unwrap(),
        ),
    );
    let matrix = assignment(
        place(10, ty(8)),
        ty(8),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    SemanticOperandV1::Copy(place(8, ty(7))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        ty(0),
                        SemanticConstantValueV1::ZeroSized,
                    )),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        ty(0),
                        SemanticConstantValueV1::ZeroSized,
                    )),
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        ty(0),
                        SemanticConstantValueV1::ZeroSized,
                    )),
                ],
            )
            .unwrap(),
        ),
    );
    vec![
        borrow(4, 1),
        borrow(5, 2),
        capture,
        assignment(
            place(7, ty(30)),
            ty(30),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(6, ty(30)))),
        ),
        copy(8, 7, projected(7, 0, 7)),
        copy(9, 7, projected(7, 1, 7)),
        matrix,
        assignment(
            place(11, ty(7)),
            ty(7),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(9),
                    vec![projection(SemanticProjectionKindV1::Dereference, ty(6))],
                    ty(6),
                )
                .unwrap(),
            },
        ),
        copy(
            12,
            5,
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(11),
                vec![
                    projection(SemanticProjectionKindV1::Dereference, ty(6)),
                    projection(SemanticProjectionKindV1::Field(0), ty(5)),
                ],
                ty(5),
            )
            .unwrap(),
        ),
    ]
}

fn audit(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    limit: usize,
) -> Result<(BTreeSet<SemanticTransparentBorrowSiteV1>, usize), ProductionSemanticSsaErrorV1> {
    let fact = GlobalBf16BorrowV1::for_callable(types, &callable(0, 62));
    let facts = fact.into_iter().collect::<Vec<_>>();
    let mut work = budget(limit);
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work)?;
    let explicit = direct_definition_or_lifetime_locals_v1(function);
    let mut flow = Audit::new(function, Some(types), &facts, &index, &explicit, &mut work)?;
    for (block, b) in function.blocks().iter().enumerate() {
        for (statement, s) in b.statements().iter().enumerate() {
            flow.statement(
                SemanticTransparentBorrowSiteV1 {
                    block: block as u32,
                    statement: statement as u32,
                },
                s.kind(),
                &mut work,
            )?;
        }
        flow.terminator(b.terminator().kind(), &mut work)?;
    }
    let sites = flow.finish(&mut work)?;
    Ok((sites, limit - work.remaining))
}

fn root_sites() -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    [0, 1, 7]
        .into_iter()
        .map(|statement| SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement,
        })
        .collect()
}

#[test]
fn global_carrier_two_fields_move_forwarding_and_exact_shared_reborrow() {
    let body = body(source_statements());
    let (sites, _) = audit(&body, &types(), MAX_FLOW_WORK).unwrap();
    assert_eq!(sites, root_sites());
    // The public production classifier hook must consume the new audit too.
    assert!(root_sites().is_subset(&typed_direct_sites(&body, &types(), &[callable(0, 62)])));
}

#[test]
fn global_carrier_bad_sibling_escape_poisons_both_original_owners() {
    for source in [place(9, ty(7)), place(7, ty(30))] {
        let mut statements = source_statements();
        statements.push(assignment(
            place(13, ty(7)),
            ty(7),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: source,
            },
        ));
        assert!(
            audit(&body(statements), &types(), MAX_FLOW_WORK)
                .unwrap()
                .0
                .is_empty()
        );
    }
}

#[test]
fn global_carrier_unknown_receiver_call_is_not_a_terminal_sink() {
    let b = body(source_statements());
    let call = SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(0),
        vec![SemanticOperandV1::Copy(place(9, ty(7)))],
        None,
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap();
    let mut work = budget(MAX_FLOW_WORK);
    let types = types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work).unwrap();
    let explicit = direct_definition_or_lifetime_locals_v1(&b);
    let mut flow = Audit::new(&b, Some(&types), &facts, &index, &explicit, &mut work).unwrap();
    for (statement, s) in b.blocks()[0].statements().iter().enumerate() {
        flow.statement(
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: statement as u32,
            },
            s.kind(),
            &mut work,
        )
        .unwrap();
    }
    flow.terminator(&SemanticTerminatorKindV1::Call(call), &mut work)
        .unwrap();
    assert!(flow.finish(&mut work).unwrap().is_empty());
}

#[test]
fn global_carrier_duplicate_definition_self_cycle_and_unknown_incoming_reject() {
    for change in 0..3 {
        let mut statements = source_statements();
        match change {
            0 => statements.push(copy(9, 7, place(8, ty(7)))),
            1 => statements.push(copy(7, 30, place(7, ty(30)))),
            2 => statements[0] = SemanticStatementKindV1::Nop,
            _ => unreachable!(),
        }
        assert!(
            audit(&body(statements), &types(), MAX_FLOW_WORK)
                .unwrap()
                .0
                .is_empty(),
            "change {change}"
        );
    }
}

#[test]
fn global_carrier_disconnected_self_cycle_does_not_poison_valid_borrows() {
    let mut statements = source_statements();
    // Replacing the carrier forwarding disconnects the malformed component
    // from both original Global borrows; its own reborrow must still reject.
    statements[3] = copy(7, 30, place(7, ty(30)));
    let (sites, _) = audit(&body(statements), &types(), MAX_FLOW_WORK).unwrap();
    let expected = [0, 1]
        .into_iter()
        .map(|statement| SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement,
        })
        .collect();
    assert_eq!(sites, expected);
}

#[test]
fn global_carrier_wrong_field_type_dereference_and_mutable_reborrow_reject() {
    for change in 0..3 {
        let mut statements = source_statements();
        statements[5] = match change {
            0 => copy(9, 7, projected(7, 2, 7)),
            1 => copy(
                9,
                7,
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(7),
                    vec![projection(SemanticProjectionKindV1::Dereference, ty(7))],
                    ty(7),
                )
                .unwrap(),
            ),
            2 => assignment(
                place(9, ty(7)),
                ty(7),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(2, ty(6)),
                },
            ),
            _ => unreachable!(),
        };
        // A malformed route must not make the other copied field authoritative.
        let sites = audit(&body(statements), &types(), MAX_FLOW_WORK).unwrap().0;
        assert!(!sites.contains(&SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement: 7
        }));
        if change < 2 {
            assert!(sites.is_empty());
        }
    }
}

#[test]
fn global_carrier_unauthenticated_fact_and_matrix_barrier_remain_closed() {
    let types = types();
    let mut work = budget(MAX_FLOW_WORK);
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    assert!(shapes.contains(ty(30), &mut work).unwrap());
    assert!(!shapes.contains(ty(8), &mut work).unwrap());
    assert_eq!(shapes.pointee(ty(7), &mut work).unwrap(), Some(ty(6)));
    assert_eq!(shapes.pointee(ty(22), &mut work).unwrap(), None);
    let mut foreign = types.clone();
    foreign[7] = types[22].clone();
    assert!(GlobalBf16BorrowV1::for_callable(&foreign, &callable(0, 62)).is_none());
    assert!(
        audit(&body(source_statements()), &foreign, MAX_FLOW_WORK)
            .unwrap()
            .0
            .is_empty()
    );
}

#[test]
fn global_carrier_shared_work_exact_boundary_and_one_short_fail_closed() {
    let body = body(source_statements());
    let (expected, spent) = audit(&body, &types(), MAX_FLOW_WORK).unwrap();
    assert_eq!(audit(&body, &types(), spent).unwrap(), (expected, spent));
    assert!(matches!(
        flow_work_profile_v1::original_error_for_test(
            audit(&body, &types(), spent - 1).unwrap_err()
        ),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        }
    ));
}

#[test]
fn global_carrier_growth_accounts_retained_capacity_and_relocation() {
    let mut values = Vec::new();
    let mut work = budget(MAX_FLOW_WORK);
    let mut required = 0;
    for i in 0..65usize {
        let old_len = values.len();
        let old_capacity = values.capacity();
        push(&mut values, i, &mut work).unwrap();
        required += 1;
        if old_len == old_capacity {
            required += values.capacity() + old_len;
        }
        assert_eq!(MAX_FLOW_WORK - work.remaining, required);
    }
    assert_eq!(values, (0..65).collect::<Vec<_>>());
}

#[test]
fn indexed_shape_setup_preserves_original_source_local_census_and_node_fields() {
    let function = body(source_statements());
    let types = types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let mut cold_work = budget(MAX_FLOW_WORK);
    let mut cold = super::shapes::Shapes::new(&types, &facts, &mut cold_work).unwrap();
    let mut expected = BTreeMap::new();
    for (local, declaration) in function.locals().iter().enumerate() {
        if cold.contains(declaration.ty(), &mut cold_work).unwrap() {
            expected.insert(local as u32, expected.len());
        }
    }
    let mut work = budget(MAX_FLOW_WORK);
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work).unwrap();
    let explicit = direct_definition_or_lifetime_locals_v1(&function);
    let audit = Audit::new(&function, Some(&types), &facts, &index, &explicit, &mut work).unwrap();
    let state = audit.0.as_ref().unwrap();
    assert!(std::ptr::eq(state.function, &function));
    assert_eq!(state.flow.by_local, expected);
    assert_eq!(state.flow.nodes.len(), expected.len());
    assert_eq!(state.flow.definitions.len(), expected.len());
    assert_eq!(state.flow.edges.len(), expected.len());
    assert!(state.flow.roots.is_empty());
    for (local, index) in expected {
        let declaration = &function.locals()[local as usize];
        let node = &state.flow.nodes[index];
        assert_eq!(node.site, SemanticTransparentBorrowSiteV1 { block: u32::MAX, statement: u32::MAX });
        assert_eq!(node.source_local, local);
        assert_eq!(node.source_type, declaration.ty());
        assert_eq!(node.source_reference, None);
        assert!(node.value_alias);
        assert!(matches!(node.source_kind, SemanticBorrowCandidateSourceV1::Direct));
        assert_eq!(node.valid, declaration.role() != SemanticLocalRoleV1::Return);
        assert_eq!(node.consumers, 0);
        assert!(!node.intrinsic_consumer);
        assert_eq!(
            state.flow.definitions[index],
            matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
        );
        assert!(state.flow.edges[index].is_empty());
    }
}

#[path = "declaration_batch_tests.rs"]
mod declaration_batch_tests;

#[path = "shared_carrier120_tests.rs"]
mod shared_carrier120_tests;

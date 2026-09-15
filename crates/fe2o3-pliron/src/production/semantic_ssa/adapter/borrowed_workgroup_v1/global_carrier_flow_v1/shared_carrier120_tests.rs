//! Classifier component fixtures only, not GPU, Global issuer, origin, or
//! admitted source/kernel proof. Source SSA still owns field/value identity.
//! Shared-wrapper publication belongs exclusively to the common custody graph;
//! these Global audit fixtures do not establish wrapper source/SSA promotion.
use super::*;

const CARRIER: u32 = 30;
const SHARED: u32 = 31;
const CAPTURES: [usize; 2] = [2, 14];
const WRAPPER_LOCALS: [u32; 6] = [14, 15, 16, 17, 18, 20];
const EXTRA_LOCALS: [u32; 9] = [
    SHARED, SHARED, SHARED, SHARED, SHARED, CARRIER, SHARED, 7, 7,
];

fn add_type(
    types: &mut Vec<SemanticTypeDeclV1>,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> u32 {
    let index = types.len() as u32;
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([150 + index as u8; 32]),
        SemanticLayoutIdentityV1::from_sha256([150 + index as u8; 32]),
        layout,
        shape,
    ));
    index
}

fn add_reference(types: &mut Vec<SemanticTypeDeclV1>, pointee: u32) -> u32 {
    add_type(
        types,
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(pointee),
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
}

fn shared_types() -> Vec<SemanticTypeDeclV1> {
    let mut result = types();
    assert_eq!(add_reference(&mut result, CARRIER), SHARED);
    result
}

fn rebuild(
    original: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
}

fn extended_body(
    statements: Vec<SemanticStatementKindV1>,
    additional: &[u32],
) -> SemanticFunctionDeclV1 {
    let original = body(statements);
    let mut locals = original.locals().to_vec();
    for &t in EXTRA_LOCALS.iter().chain(additional) {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([150 + locals.len() as u8; 32]),
            ty(t),
            SemanticLocalRoleV1::Temporary,
            original.source(),
        ));
    }
    rebuild(&original, locals, original.blocks().to_vec())
}

fn shared_body(statements: Vec<SemanticStatementKindV1>) -> SemanticFunctionDeclV1 {
    extended_body(statements, &[])
}

fn shared_borrow(
    destination: u32,
    reference: u32,
    source: SemanticPlaceV1,
) -> SemanticStatementKindV1 {
    assignment(
        place(destination, ty(reference)),
        ty(reference),
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Shared,
            place: source,
        },
    )
}

fn moved(destination: u32, result: u32, source: SemanticPlaceV1) -> SemanticStatementKindV1 {
    assignment(
        place(destination, ty(result)),
        ty(result),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(source)),
    )
}

fn path(local: u32, projections: &[(SemanticProjectionKindV1, u32)]) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        projections
            .iter()
            .map(|&(kind, t)| projection(kind, ty(t)))
            .collect(),
        ty(projections.last().unwrap().1),
    )
    .unwrap()
}

fn field(local: u32, index: u32) -> SemanticPlaceV1 {
    path(
        local,
        &[
            (SemanticProjectionKindV1::Dereference, CARRIER),
            (SemanticProjectionKindV1::Field(index), 7),
        ],
    )
}

fn capture(
    destination: u32,
    first: SemanticPlaceV1,
    second: SemanticPlaceV1,
) -> SemanticStatementKindV1 {
    assignment(
        place(destination, ty(CARRIER)),
        ty(CARRIER),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![
                SemanticOperandV1::Copy(first),
                SemanticOperandV1::Copy(second),
                SemanticOperandV1::Copy(place(3, ty(2))),
            ],
        )
        .unwrap(),
    )
}

fn shared_statements() -> Vec<SemanticStatementKindV1> {
    let mut statements = source_statements();
    statements.splice(
        4..4,
        [
            shared_borrow(14, SHARED, place(7, ty(CARRIER))),
            copy(15, SHARED, place(14, ty(SHARED))),
            moved(16, SHARED, place(15, ty(SHARED))),
            shared_borrow(17, SHARED, place(7, ty(CARRIER))),
            copy(18, SHARED, place(17, ty(SHARED))),
        ],
    );
    statements[9] = copy(8, 7, field(16, 0));
    statements[10] = copy(9, 7, field(18, 1));
    statements.extend([
        capture(19, field(16, 0), field(18, 1)),
        shared_borrow(20, SHARED, place(19, ty(CARRIER))),
        copy(21, 7, field(20, 0)),
        copy(22, 7, field(20, 1)),
    ]);
    statements
}

fn expected_sites() -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    // Only original Global borrows publish here; wrapper borrows add graph edges.
    [0, 1, 12]
        .into_iter()
        .map(|statement| SemanticTransparentBorrowSiteV1 {
            block: 0,
            statement,
        })
        .collect()
}

fn source_assignment(function: &SemanticFunctionDeclV1, statement: usize) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(a) = function.blocks()[0].statements()[statement].kind()
    else {
        panic!("expected fixture assignment {statement}");
    };
    a
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    pending: Vec<(usize, usize, bool)>,
    fields: Vec<(usize, usize, bool)>,
    sites: BTreeSet<SemanticTransparentBorrowSiteV1>,
    spent: usize,
}

// coverage118's observer is private and fixes its own type table. This variant
// accepts the added reference type and requests both original capture sites.
fn observe(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    limit: usize,
) -> Result<Observation, ProductionSemanticSsaErrorV1> {
    let facts = [GlobalBf16BorrowV1::for_callable(types, &callable(0, 62)).unwrap()];
    assert_eq!(facts[0].pairs()[2], (ty(7), ty(6)));
    let mut work = budget(limit);
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work)?;
    let explicit = direct_definition_or_lifetime_locals_v1(function);
    let mut flow = Audit::new(function, Some(types), &facts, &index, &explicit, &mut work)?;
    let mut pending = Vec::new();
    for (statement, s) in function.blocks()[0].statements().iter().enumerate() {
        let transport = flow.statement_with_transport(
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: statement as u32,
            },
            s.kind(),
            &mut work,
        )?;
        if CAPTURES.contains(&statement) {
            let mut transport = transport;
            for field in 0..2 {
                let recorded = match transport.as_mut() {
                    Some(transport) => transport.defer_leaf(
                        source_assignment(function, statement),
                        field,
                        ty(7),
                        ty(6),
                        &mut work,
                    )?,
                    None => false,
                };
                pending.push((statement, field, recorded));
            }
        }
    }
    flow.terminator(function.blocks()[0].terminator().kind(), &mut work)?;
    let completed = flow.finish_with_fields(&mut work)?;
    let mut fields = completed
        .fields
        .iter()
        .map(|coverage| {
            let statement = CAPTURES
                .into_iter()
                .find(|&statement| {
                    std::ptr::eq(
                        coverage.assignment(),
                        source_assignment(function, statement),
                    )
                })
                .expect("coverage must retain the exact original assignment, not a clone or alias");
            (statement, coverage.field(), coverage.closed())
        })
        .collect::<Vec<_>>();
    fields.sort_unstable();
    Ok(Observation {
        pending,
        fields,
        sites: completed.sites,
        spent: limit - work.remaining,
    })
}

fn expected_fields(closed: bool) -> Vec<(usize, usize, bool)> {
    CAPTURES
        .into_iter()
        .flat_map(|statement| [(statement, 0, closed), (statement, 1, closed)])
        .collect()
}

fn assert_closed(function: &SemanticFunctionDeclV1, types: &[SemanticTypeDeclV1]) {
    let observed = observe(function, types, MAX_FLOW_WORK).unwrap();
    assert_eq!(observed.pending, expected_fields(true));
    assert_eq!(observed.fields, expected_fields(true));
    assert_eq!(observed.sites, expected_sites());
    assert_eq!(
        audit(function, types, MAX_FLOW_WORK).unwrap().0,
        observed.sites
    );
}

fn assert_poisoned(function: &SemanticFunctionDeclV1, types: &[SemanticTypeDeclV1]) {
    let observed = observe(function, types, MAX_FLOW_WORK).unwrap();
    assert_eq!(observed.pending, expected_fields(true));
    assert_eq!(observed.fields, expected_fields(false));
    assert!(observed.sites.is_empty(), "{observed:?}");
    assert_eq!(
        audit(function, types, MAX_FLOW_WORK).unwrap().0,
        observed.sites
    );
}

#[test]
fn shared_carrier120_two_globals_repeated_wrappers_copy_move_and_pending_fields_close() {
    let types = shared_types();
    let function = shared_body(shared_statements());
    let original = function.clone();
    assert_closed(&function, &types);
    assert!(expected_sites().is_subset(&typed_direct_sites(&function, &types, &[callable(0, 62)])));
    assert_eq!(function, original);
    for (statement, expected) in [
        (9, field(16, 0)),
        (10, field(18, 1)),
        (16, field(20, 0)),
        (17, field(20, 1)),
    ] {
        assert_eq!(
            source_assignment(&function, statement).value().kind(),
            &SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(expected))
        );
    }
    assert_ne!(field(16, 0), field(16, 1));
    let SemanticRvalueKindV1::Aggregate(a) = source_assignment(&function, 14).value().kind() else {
        unreachable!()
    };
    assert_eq!(a.operands()[0], SemanticOperandV1::Copy(field(16, 0)));
    assert_eq!(a.operands()[1], SemanticOperandV1::Copy(field(18, 1)));
}

#[test]
fn shared_carrier120_global_audit_never_publishes_shared_wrapper_borrows() {
    for (types, function, wrappers) in [
        (
            shared_types(),
            shared_body(shared_statements()),
            vec![4, 7, 15],
        ),
        (
            nested_types(),
            extended_body(nested_statements(), &[32, 33]),
            vec![4, 7, 15, 19],
        ),
    ] {
        let observed = observe(&function, &types, MAX_FLOW_WORK).unwrap();
        assert_eq!(observed.pending, expected_fields(true));
        assert_eq!(observed.fields, expected_fields(true));
        assert_eq!(observed.sites, expected_sites());
        let (sites, _) = audit(&function, &types, MAX_FLOW_WORK).unwrap();
        assert_eq!(sites, observed.sites);
        for statement in wrappers {
            assert!(matches!(
                source_assignment(&function, statement).value().kind(),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    ..
                }
            ));
            assert!(
                !sites.contains(&SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: statement as u32,
                }),
                "Global audit must not bypass a common-custody veto at wrapper borrow {statement}",
            );
        }
    }
}

#[test]
fn shared_carrier120_aggregate_and_tuple_kinds_both_close_exactly() {
    let mut types = shared_types();
    let old = &types[CARRIER as usize];
    let SemanticTypeShapeV1::Tuple(fields) = old.shape() else {
        unreachable!()
    };
    types[CARRIER as usize] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        old.layout().clone(),
        SemanticTypeShapeV1::Aggregate(fields.clone()),
    );
    let mut statements = shared_statements();
    for statement in CAPTURES {
        let SemanticStatementKindV1::Assign(a) = &statements[statement] else {
            unreachable!()
        };
        let SemanticRvalueKindV1::Aggregate(value) = a.value().kind() else {
            unreachable!()
        };
        statements[statement] = assignment(
            a.destination().clone(),
            ty(CARRIER),
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                value.operands().to_vec(),
            )
            .unwrap(),
        );
    }
    assert_closed(&shared_body(statements), &types);
}

#[test]
fn shared_carrier120_borrow_sites_keep_original_block_and_statement_coordinates() {
    let original = shared_body(shared_statements());
    let source = original.source();
    let mut statements = vec![SemanticStatementV1::new(source, SemanticStatementKindV1::Nop); 3];
    statements.extend_from_slice(original.blocks()[0].statements());
    let blocks = vec![
        SemanticBasicBlockV1::new(
            original.blocks()[0].identity(),
            source,
            vec![],
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(1),
                )),
            ),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([229; 32]),
            source,
            statements,
            original.blocks()[0].terminator().clone(),
        )
        .unwrap(),
    ];
    let function = rebuild(&original, original.locals().to_vec(), blocks);
    let retained = function.clone();
    let expected = expected_sites()
        .into_iter()
        .map(|site| SemanticTransparentBorrowSiteV1 {
            block: 1,
            statement: site.statement + 3,
        })
        .collect();
    assert_eq!(
        audit(&function, &shared_types(), MAX_FLOW_WORK).unwrap().0,
        expected
    );
    assert_eq!(function, retained);
}

#[test]
fn shared_carrier120_late_escape_of_every_wrapper_carrier_or_global_poisons_all_fields() {
    let types = shared_types();
    for (local, t) in WRAPPER_LOCALS
        .into_iter()
        .map(|local| (local, SHARED))
        .chain([6, 7, 19].into_iter().map(|local| (local, CARRIER)))
        .chain([4, 5, 8, 9, 11, 21, 22].into_iter().map(|local| (local, 7)))
    {
        let mut statements = shared_statements();
        statements.push(assignment(
            place(13, ty(7)),
            ty(7),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(local, ty(t)),
            },
        ));
        assert_poisoned(&shared_body(statements), &types);
    }
}

#[test]
fn shared_carrier120_non_assignment_and_unknown_call_uses_poison_all_fields() {
    let types = shared_types();
    for (local, t) in [(14, SHARED), (20, SHARED), (4, 7), (22, 7)] {
        let mut statements = shared_statements();
        statements.push(SemanticStatementKindV1::Deinitialize(place(local, ty(t))));
        assert_poisoned(&shared_body(statements), &types);

        let original = shared_body(shared_statements());
        let block = &original.blocks()[0];
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![SemanticOperandV1::Copy(place(local, ty(t)))],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let blocks = vec![
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                block.statements().to_vec(),
                SemanticTerminatorV1::new(block.source(), SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
        ];
        assert_poisoned(
            &rebuild(&original, original.locals().to_vec(), blocks),
            &types,
        );
    }
}

#[test]
fn shared_carrier120_duplicate_undefined_and_self_definitions_poison_connected_group() {
    let types = shared_types();
    for statement in [4, 5, 6, 7, 8, 15] {
        let mut statements = shared_statements();
        statements.push(statements[statement].clone());
        assert_poisoned(&shared_body(statements), &types);
    }
    for statement in [0, 1, 4, 5, 6, 7, 8, 15] {
        let mut statements = shared_statements();
        statements[statement] = SemanticStatementKindV1::Nop;
        // The last wrapper otherwise disconnects when its definition is absent.
        statements.push(capture(23, field(20, 0), field(18, 1)));
        assert_poisoned(&extended_body(statements, &[CARRIER]), &types);
    }
    for (statement, local) in [(4, 14), (5, 15), (6, 16), (7, 17), (8, 18), (15, 20)] {
        for move_alias in [false, true] {
            let mut statements = shared_statements();
            statements[statement] = if move_alias {
                moved(local, SHARED, place(local, ty(SHARED)))
            } else {
                copy(local, SHARED, place(local, ty(SHARED)))
            };
            statements.push(capture(23, field(20, 0), field(18, 1)));
            assert_poisoned(&extended_body(statements, &[CARRIER]), &types);
        }
    }
}

#[test]
fn shared_carrier120_disconnected_bad_wrapper_does_not_poison_closed_group() {
    let types = shared_types();
    let mut statements = shared_statements();
    statements.push(copy(23, SHARED, place(23, ty(SHARED))));
    assert_closed(&extended_body(statements, &[SHARED]), &types);
}

#[test]
fn shared_carrier120_pointer_kind_mutability_address_width_metadata_and_layout_are_exact() {
    for change in 0..10 {
        let mut types = shared_types();
        let old = &types[SHARED as usize];
        let shape = SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty(CARRIER),
                if change == 0 {
                    SemanticPointerKindV1::Raw
                } else {
                    SemanticPointerKindV1::Reference
                },
                if change == 1 {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                if change == 2 { 1 } else { 0 },
                if change == 3 { 32 } else { 64 },
                match change {
                    4 => SemanticPointerMetadataV1::SliceLength,
                    5 => SemanticPointerMetadataV1::VTable,
                    _ => SemanticPointerMetadataV1::None,
                },
            )
            .unwrap(),
        );
        let layout = match change {
            3 => SemanticTypeLayoutV1::new(Some(4), 4).unwrap(),
            4 | 5 => SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            6 => SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
            7 => SemanticTypeLayoutV1::new(Some(8), 4).unwrap(),
            8 => SemanticTypeLayoutV1::new(None, 8).unwrap(),
            9 => SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            _ => old.layout().clone(),
        };
        let malformed = add_type(&mut types, layout, shape);
        let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
        let mut setup = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
        assert!(!shapes.contains(ty(malformed), &mut setup).unwrap());
        let w = std::mem::size_of::<u64>().div_ceil(std::mem::size_of::<usize>());
        // Owner + guard + lookup + unchanged validation + completed insertion.
        for cost in [15 + 3 * w, 2 + w, 2 + w] {
            let mut work = budget(cost);
            assert_eq!(
                shapes
                    .shared_carrier_pointee(ty(malformed), &mut work)
                    .unwrap(),
                None
            );
            assert_eq!(work.remaining, 0, "change={change}");
        }
        let mut work = budget(2 + 2 * w);
        assert!(!shapes.transport_contains(ty(malformed), &mut work).unwrap());
        assert_eq!(work.remaining, 0);
        let mut statements = shared_statements();
        // Request both captures before the malformed borrow escapes their group.
        statements.push(shared_borrow(23, malformed, place(7, ty(CARRIER))));
        assert_poisoned(&extended_body(statements, &[malformed]), &types);
    }
}

#[test]
fn shared_carrier120_alias_requires_exact_wrapper_type_and_local_declaration() {
    let mut types = shared_types();
    let lookalike = add_reference(&mut types, CARRIER);
    assert_ne!(ty(lookalike), ty(SHARED));
    for move_alias in [false, true] {
        let mut statements = shared_statements();
        statements.push(if move_alias {
            moved(23, lookalike, place(14, ty(SHARED)))
        } else {
            copy(23, lookalike, place(14, ty(SHARED)))
        });
        assert_poisoned(&extended_body(statements, &[lookalike]), &types);
    }
    for local in [14, 15, 16] {
        let original = shared_body(shared_statements());
        let mut locals = original.locals().to_vec();
        let old = &locals[local];
        locals[local] =
            SemanticLocalDeclV1::new(old.identity(), ty(lookalike), old.role(), old.source());
        assert_poisoned(
            &rebuild(&original, locals, original.blocks().to_vec()),
            &types,
        );
    }
}

#[test]
fn shared_carrier120_projected_destination_and_wrong_result_type_reject() {
    for malformed in [
        assignment(
            field(14, 0),
            ty(7),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(8, ty(7)))),
        ),
        assignment(
            place(23, ty(SHARED)),
            ty(7),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(14, ty(SHARED)))),
        ),
    ] {
        let mut statements = shared_statements();
        statements.push(malformed);
        assert_poisoned(&extended_body(statements, &[SHARED]), &shared_types());
    }
}

#[test]
fn shared_carrier120_malformed_projection_paths_poison_all_pending_fields() {
    use SemanticProjectionKindV1::*;
    for projections in [
        vec![(Field(0), 7)],
        vec![(Dereference, 7)],
        vec![
            (Dereference, CARRIER),
            (Dereference, CARRIER),
            (Field(0), 7),
        ],
        vec![(Dereference, CARRIER), (Field(0), 7), (Dereference, 6)],
        vec![(Dereference, CARRIER), (Field(2), 7)],
        vec![(Dereference, CARRIER), (Field(3), 7)],
        vec![(Dereference, CARRIER), (Field(u32::MAX), 7)],
        vec![(Dereference, CARRIER), (Field(0), 2)],
        vec![(Field(0), CARRIER), (Dereference, CARRIER), (Field(0), 7)],
        vec![
            (Dereference, CARRIER),
            (Index(SemanticLocalIdV1::from_index(3)), 7),
        ],
        vec![
            (Dereference, CARRIER),
            (
                ConstantIndex {
                    offset: 0,
                    minimum_length: 1,
                    from_end: false,
                },
                7,
            ),
        ],
        vec![
            (Dereference, CARRIER),
            (Downcast(0), CARRIER),
            (Field(0), 7),
        ],
        vec![(Dereference, CARRIER), (OpaqueCast, CARRIER), (Field(0), 7)],
        vec![(Dereference, CARRIER), (Subtype, CARRIER), (Field(0), 7)],
        vec![
            (Dereference, CARRIER),
            (
                Subslice {
                    from: 0,
                    to: 0,
                    from_end: false,
                },
                7,
            ),
        ],
    ] {
        let source = path(14, &projections);
        let mut statements = shared_statements();
        statements.push(copy(23, source.ty().index(), source.clone()));
        assert_poisoned(
            &extended_body(statements, &[source.ty().index()]),
            &shared_types(),
        );
    }
}

#[test]
fn shared_carrier120_borrow_requires_whole_by_value_carrier_and_never_reborrows_wrapper() {
    use SemanticProjectionKindV1::*;
    let mut types = shared_types();
    let chain = add_reference(&mut types, SHARED);
    for (result, source) in [
        (SHARED, path(14, &[(Dereference, CARRIER)])),
        (chain, place(14, ty(SHARED))),
        (SHARED, place(7, ty(SHARED))),
        (7, field(14, 0)),
        (
            7,
            path(
                14,
                &[(Dereference, CARRIER), (Field(0), 7), (Dereference, 6)],
            ),
        ),
    ] {
        let mut statements = shared_statements();
        statements.push(shared_borrow(23, result, source));
        assert_poisoned(&extended_body(statements, &[result]), &types);
    }
    for kind in [SemanticBorrowKindV1::Mutable, SemanticBorrowKindV1::Fake] {
        let mut statements = shared_statements();
        statements.push(assignment(
            place(23, ty(SHARED)),
            ty(SHARED),
            SemanticRvalueKindV1::Borrow {
                kind,
                place: place(7, ty(CARRIER)),
            },
        ));
        assert_poisoned(&extended_body(statements, &[SHARED]), &types);
    }
}

fn nested_types() -> Vec<SemanticTypeDeclV1> {
    let mut types = shared_types();
    assert_eq!(
        add_type(
            &mut types,
            SemanticTypeLayoutV1::aggregate(
                Some(32),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 24], vec![]).unwrap()
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![ty(CARRIER), ty(2)]).unwrap()
            ),
        ),
        32
    );
    assert_eq!(add_reference(&mut types, 32), 33);
    types
}

fn nested_statements() -> Vec<SemanticStatementKindV1> {
    let mut statements = shared_statements();
    statements.extend([
        assignment(
            place(23, ty(32)),
            ty(32),
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    SemanticOperandV1::Copy(place(19, ty(CARRIER))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                ],
            )
            .unwrap(),
        ),
        shared_borrow(24, 33, place(23, ty(32))),
    ]);
    statements
}

#[test]
fn shared_carrier120_one_dereference_then_nested_fields_and_copied_subaggregate_close() {
    use SemanticProjectionKindV1::*;
    let mut statements = nested_statements();
    statements.extend([
        copy(
            25,
            CARRIER,
            path(24, &[(Dereference, 32), (Field(0), CARRIER)]),
        ),
        copy(
            26,
            7,
            path(24, &[(Dereference, 32), (Field(0), CARRIER), (Field(1), 7)]),
        ),
        copy(27, 2, path(24, &[(Dereference, 32), (Field(1), 2)])),
    ]);
    let function = extended_body(statements, &[32, 33, CARRIER, 7, 2]);
    let types = nested_types();
    let observed = observe(&function, &types, MAX_FLOW_WORK).unwrap();
    assert_eq!(observed.pending, expected_fields(true));
    assert_eq!(observed.fields, expected_fields(true));
    let expected = expected_sites();
    assert_eq!(observed.sites, expected);
    assert_eq!(audit(&function, &types, MAX_FLOW_WORK).unwrap().0, expected);
}

#[test]
fn shared_carrier120_moved_selected_aggregate_through_shared_reference_rejects() {
    use SemanticProjectionKindV1::*;
    for move_value in [false, true] {
        let mut statements = shared_statements();
        let source = path(14, &[(Dereference, CARRIER)]);
        statements.push(if move_value {
            moved(23, CARRIER, source)
        } else {
            copy(23, CARRIER, source)
        });
        assert_poisoned(&extended_body(statements, &[CARRIER]), &shared_types());
    }
    let mut statements = nested_statements();
    statements.push(moved(
        25,
        CARRIER,
        path(24, &[(Dereference, 32), (Field(0), CARRIER)]),
    ));
    assert_poisoned(
        &extended_body(statements, &[32, 33, CARRIER]),
        &nested_types(),
    );
}

#[test]
fn shared_carrier120_moved_global_leaf_retains_existing_classifier_transport_rule() {
    let mut statements = shared_statements();
    statements.push(moved(23, 7, field(14, 0)));
    // Admission and partial-move certificates remain outside this audit fixture.
    assert_closed(&extended_body(statements, &[7]), &shared_types());
}

#[test]
fn shared_carrier120_projected_borrow_of_nested_carrier_rejects() {
    use SemanticProjectionKindV1::*;
    let mut statements = nested_statements();
    statements.push(shared_borrow(25, SHARED, path(23, &[(Field(0), CARRIER)])));
    assert_poisoned(
        &extended_body(statements, &[32, 33, SHARED]),
        &nested_types(),
    );
}

#[test]
fn shared_carrier120_matrix_and_reference_chains_are_not_carrier_shapes() {
    let mut types = shared_types();
    let chain = add_reference(&mut types, SHARED);
    let raw = add_type(
        &mut types,
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new(
                ty(CARRIER),
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    );
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let mut work = budget(MAX_FLOW_WORK);
    let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    assert!(shapes.contains(ty(CARRIER), &mut work).unwrap());
    assert!(!shapes.contains(ty(SHARED), &mut work).unwrap());
    for t in [CARRIER, SHARED] {
        assert!(shapes.transport_contains(ty(t), &mut work).unwrap());
    }
    for t in [8, 9, chain, raw] {
        assert!(
            !shapes.transport_contains(ty(t), &mut work).unwrap(),
            "type {t}"
        );
    }
    assert_eq!(
        shapes
            .shared_carrier_pointee(ty(SHARED), &mut work)
            .unwrap(),
        Some(ty(CARRIER))
    );
    // Only the authenticated Global leaf is a defer_leaf reference/owned pair.
    assert_eq!(shapes.pointee(ty(SHARED), &mut work).unwrap(), None);
    assert_eq!(shapes.pointee(ty(7), &mut work).unwrap(), Some(ty(6)));
    let function = shared_body(shared_statements());
    let mut raw = BTreeSet::new();
    shapes
        .for_each_declaration(function.locals(), &mut work, |local, _, selected, _| {
            if selected {
                raw.insert(local as u32);
            }
            Ok(())
        })
        .unwrap();
    let mut transport = BTreeSet::new();
    shapes
        .for_each_transport_declaration(function.locals(), &mut work, |local, _, selected, _| {
            if selected {
                transport.insert(local as u32);
            }
            Ok(())
        })
        .unwrap();
    raw.extend(WRAPPER_LOCALS);
    assert_eq!(transport, raw);
}

#[test]
fn shared_cache121_transport_and_raw_query_order_never_admits_reference_only_carriers() {
    let mut types = shared_types();
    let chain = add_reference(&mut types, SHARED);
    let aggregate = add_type(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            8,
            SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![ty(SHARED)]).unwrap()),
    );
    let shared_aggregate = add_reference(&mut types, aggregate);
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let function = extended_body(vec![], &[chain, aggregate, shared_aggregate]);
    for transport_first in [false, true] {
        let mut work = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
        if transport_first {
            assert!(shapes.transport_contains(ty(SHARED), &mut work).unwrap());
        } else {
            assert!(!shapes.contains(ty(SHARED), &mut work).unwrap());
        }
        for _ in 0..2 {
            for (index, raw, transport, pointee) in [
                (SHARED, false, true, Some(CARRIER)),
                (chain, false, false, None),
                (aggregate, false, false, None),
                (shared_aggregate, false, false, None),
                (CARRIER, true, true, None),
                (7, true, true, None),
                (8, false, false, None),
                (9, false, false, None),
            ] {
                assert_eq!(
                    shapes.transport_contains(ty(index), &mut work).unwrap(),
                    transport
                );
                assert_eq!(shapes.contains(ty(index), &mut work).unwrap(), raw);
                assert_eq!(
                    shapes.shared_carrier_pointee(ty(index), &mut work).unwrap(),
                    pointee.map(ty)
                );
                assert_eq!(shapes.contains(ty(index), &mut work).unwrap(), raw);
            }
            shapes
                .for_each_transport_declaration(
                    function.locals(),
                    &mut work,
                    |_, d, selected, _| {
                        assert_eq!(selected, [7, CARRIER, SHARED].contains(&d.ty().index()));
                        Ok(())
                    },
                )
                .unwrap();
            shapes
                .for_each_declaration(function.locals(), &mut work, |_, d, selected, _| {
                    assert_eq!(selected, [7, CARRIER].contains(&d.ty().index()));
                    Ok(())
                })
                .unwrap();
        }
    }
}

fn assert_budget_error<T: std::fmt::Debug>(
    result: Result<T, ProductionSemanticSsaErrorV1>,
    limit: usize,
) {
    let error = result.expect_err("every shorter total budget must return an error");
    assert!(
        matches!(
            flow_work_profile_v1::original_error_for_test(error),
            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                ..
            }
        ),
        "limit {limit}"
    );
}

#[test]
fn shared_carrier120_exact_total_and_every_shorter_budget_return_error() {
    let types = shared_types();
    let function = shared_body(shared_statements());
    let full = observe(&function, &types, MAX_FLOW_WORK).unwrap();
    assert_eq!(full.pending, expected_fields(true));
    assert_eq!(full.fields, expected_fields(true));
    assert_eq!(full.sites, expected_sites());
    assert!(full.spent > 0 && full.spent <= MAX_FLOW_WORK);
    assert_eq!(observe(&function, &types, full.spent).unwrap(), full);
    for limit in 0..full.spent {
        assert_budget_error(observe(&function, &types, limit), limit);
    }

    let (sites, spent) = audit(&function, &types, MAX_FLOW_WORK).unwrap();
    assert_eq!(sites, expected_sites());
    assert_eq!(audit(&function, &types, spent).unwrap(), (sites, spent));
    for limit in 0..spent {
        assert_budget_error(audit(&function, &types, limit), limit);
    }
}

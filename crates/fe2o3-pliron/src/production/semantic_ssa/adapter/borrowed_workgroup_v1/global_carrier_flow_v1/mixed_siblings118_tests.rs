//! Classifier/candidate integration, not source admission or issuer evidence.
use super::*;
#[path = "mixed_publication118_tests.rs"]
mod publication;

const CAPTURE: usize = 4;

fn mixed(escape: Option<u32>) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
    let mut types = types();
    let old = &types[30];
    types[30] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        old.layout().clone(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(vec![ty(7), ty(7), ty(11)]).unwrap(),
        ),
    );
    let original = body(source_statements());
    let source = original.source();
    let mut locals = original.locals().to_vec();
    for (index, t) in [10, 11].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([230 + index as u8; 32]),
            ty(t),
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let zero = |t, value| SemanticOperandV1::Constant(SemanticConstantV1::new(ty(t), value));
    let mut statements = vec![
        assignment(
            place(14, ty(10)),
            ty(10),
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    zero(
                        3,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(0, 4).unwrap()),
                    ),
                    zero(0, SemanticConstantValueV1::ZeroSized),
                    zero(0, SemanticConstantValueV1::ZeroSized),
                    zero(0, SemanticConstantValueV1::ZeroSized),
                ],
            )
            .unwrap(),
        ),
        assignment(
            place(15, ty(11)),
            ty(11),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(14, ty(10)),
            },
        ),
    ];
    statements.extend(source_statements());
    statements[CAPTURE] = assignment(
        place(6, ty(30)),
        ty(30),
        SemanticRvalueKindV1::aggregate(
            SemanticAggregateKindV1::Tuple,
            vec![
                SemanticOperandV1::Copy(place(4, ty(7))),
                SemanticOperandV1::Copy(place(5, ty(7))),
                SemanticOperandV1::Copy(place(15, ty(11))),
            ],
        )
        .unwrap(),
    );
    if let Some(local) = escape {
        statements.push(assignment(
            place(13, ty(7)),
            ty(7),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(local, ty(7)),
            },
        ));
    }
    let block = &original.blocks()[0];
    let blocks = vec![
        SemanticBasicBlockV1::new(
            block.identity(),
            source,
            statements
                .into_iter()
                .map(|kind| SemanticStatementV1::new(source, kind))
                .collect(),
            block.terminator().clone(),
        )
        .unwrap(),
    ];
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        source,
        original.abi().clone(),
        locals,
        original.entry(),
        blocks,
    )
    .unwrap();
    (types, function)
}

fn candidates() -> Vec<SemanticBorrowCandidateV1> {
    [(2, 1, 6), (3, 2, 6), (1, 14, 10), (4, 15, 10)]
        .into_iter()
        .enumerate()
        .map(
            |(index, (statement, owner, owned))| SemanticBorrowCandidateV1 {
                site: SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement,
                },
                source_local: owner,
                source_type: ty(owned),
                source_reference: (index == 3).then_some(15),
                value_alias: index == 3,
                source_kind: if index == 3 {
                    SemanticBorrowCandidateSourceV1::TypedCarrier
                } else {
                    SemanticBorrowCandidateSourceV1::Direct
                },
                valid: true,
                consumers: 0,
                intrinsic_consumer: false,
            },
        )
        .collect()
}

fn run(
    escape: Option<u32>,
    foreign_handle: bool,
    wrong_role: bool,
    use_handle: bool,
) -> ([bool; 4], [bool; 4], Vec<(usize, bool)>) {
    let (types, function) = mixed(escape);
    let (_, foreign) = mixed(escape);
    let audited = if foreign_handle { &foreign } else { &function };
    let mut work = budget(MAX_FLOW_WORK);
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    assert_eq!(facts[0].pairs()[1], (ty(11), ty(10)));
    let routes = math_capture_flow_v1::Routes::from_pairs(
        &function,
        &types,
        &BTreeMap::from([facts[0].pairs()[1]]),
        &BTreeSet::new(),
        &mut work,
    )
    .unwrap();
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work).unwrap();
    let explicit = direct_definition_or_lifetime_locals_v1(audited);
    let mut audit =
        Audit::new(audited, Some(&types), &facts, &index, &explicit, &mut work).unwrap();
    let by_reference = BTreeMap::from([(4, 0), (5, 1), (15, 2), (6, 3)]);
    let mut candidates = candidates();
    if wrong_role {
        candidates[0].source_type = ty(10);
    }
    let mut before_finish = [false; 4];
    for (statement, s) in audited.blocks()[0].statements().iter().enumerate() {
        let mut transport = audit
            .statement_with_transport(
                SemanticTransparentBorrowSiteV1 {
                    block: 0,
                    statement: statement as u32,
                },
                s.kind(),
                &mut work,
            )
            .unwrap();
        if statement == CAPTURE {
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[0].statements()[statement].kind()
            else {
                unreachable!()
            };
            assert_eq!(
                routes
                    .source(assignment, &mut work)
                    .unwrap()
                    .unwrap()
                    .local()
                    .index(),
                15
            );
            assert!(transport.is_some());
            routes
                .invalidate_siblings_with_global(
                    assignment,
                    &by_reference,
                    &mut candidates,
                    [None; 2],
                    if use_handle { transport.as_mut() } else { None },
                    &mut work,
                )
                .unwrap();
            before_finish = std::array::from_fn(|i| candidates[i].valid);
        }
    }
    audit
        .terminator(audited.blocks()[0].terminator().kind(), &mut work)
        .unwrap();
    let closed = audit.finish_with_fields(&mut work).unwrap();
    // The ordinary Global result remains independent of main-candidate state.
    assert_eq!(
        closed.sites,
        super::audit(audited, &types, MAX_FLOW_WORK).unwrap().0
    );
    math_capture_flow_v1::Routes::resolve_global_siblings(
        &closed.fields,
        &by_reference,
        &mut candidates,
        &mut work,
    )
    .unwrap();
    (
        before_finish,
        std::array::from_fn(|i| candidates[i].valid),
        closed
            .fields
            .iter()
            .map(|field| (field.field(), field.closed()))
            .collect(),
    )
}

#[test]
fn mixed_global_siblings118_complete_coverage_preserves_only_the_checked_leaves() {
    assert_eq!(
        run(None, false, false, true),
        ([true; 4], [true; 4], vec![(0, true), (1, true)])
    );
    assert_eq!(
        run(None, false, false, false),
        (
            [false, false, true, false],
            [false, false, true, false],
            vec![]
        )
    );
    assert_eq!(
        run(None, false, true, true),
        (
            [false, true, true, false],
            [false, true, true, false],
            vec![(1, true)]
        )
    );
}

#[test]
fn mixed_global_siblings118_late_poison_invalidates_destination_before_components() {
    for escape in [4, 5, 8, 9] {
        assert_eq!(
            run(Some(escape), false, false, true),
            (
                [true; 4],
                [false, false, true, false],
                vec![(0, false), (1, false)]
            )
        );
    }
}

#[test]
fn mixed_global_siblings118_foreign_valid_transport_cannot_exempt_original_operands() {
    assert_eq!(
        run(None, true, false, true),
        (
            [false, false, true, false],
            [false, false, true, false],
            vec![]
        )
    );
}

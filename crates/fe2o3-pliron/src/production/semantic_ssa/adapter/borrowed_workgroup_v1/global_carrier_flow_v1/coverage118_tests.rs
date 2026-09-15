use super::*;

// Classifier component fixtures only: pending transport coverage is neither
// admitted source/kernel authority nor Global issuer, origin, or GPU proof.
const CAPTURE: usize = 2;
const LEAVES: [(usize, u32, u32); 2] = [(0, 7, 6), (1, 7, 6)];

#[derive(Clone, Copy, Debug)]
enum WorkPhase {
    Whole,
    Pending,
    Finish,
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    transport: bool,
    pending: Vec<bool>,
    sites: BTreeSet<SemanticTransparentBorrowSiteV1>,
    fields: Vec<(usize, bool)>,
    spent: usize,
}

fn source_assignment(function: &SemanticFunctionDeclV1, index: usize) -> &SemanticAssignmentV1 {
    let SemanticStatementKindV1::Assign(assignment) =
        function.blocks()[0].statements()[index].kind()
    else {
        panic!("expected fixture assignment {index}");
    };
    assignment
}

fn visit(
    flow: &mut Audit<'_>,
    function: &SemanticFunctionDeclV1,
    range: std::ops::Range<usize>,
    work: &mut Budget,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    for statement in range {
        flow.statement(
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: statement as u32,
            },
            function.blocks()[0].statements()[statement].kind(),
            work,
        )?;
    }
    Ok(())
}

fn observe(
    function: &SemanticFunctionDeclV1,
    statement: usize,
    requests: &[(usize, u32, u32)],
    phase: WorkPhase,
    limit: usize,
) -> Result<Observation, ProductionSemanticSsaErrorV1> {
    let types = types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    assert_eq!(facts[0].pairs()[2], (ty(7), ty(6)));
    let mut work = budget(if matches!(phase, WorkPhase::Whole) {
        limit
    } else {
        MAX_FLOW_WORK
    });
    let index = global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work)?;
    let explicit = direct_definition_or_lifetime_locals_v1(function);
    let mut flow = Audit::new(function, Some(&types), &facts, &index, &explicit, &mut work)?;
    visit(&mut flow, function, 0..statement, &mut work)?;

    let mut pending = Vec::new();
    let mut pending_cost = 0;
    let had_transport;
    {
        let transport = flow.statement_with_transport(
            SemanticTransparentBorrowSiteV1 {
                block: 0,
                statement: statement as u32,
            },
            function.blocks()[0].statements()[statement].kind(),
            &mut work,
        )?;
        had_transport = transport.is_some();
        // Isolated phase measurements get a fresh cap; Whole keeps one budget
        // from index construction through the completed audit.
        if matches!(phase, WorkPhase::Pending) {
            work = budget(limit);
        }
        if let Some(mut transport) = transport {
            for &(field, reference, owned) in requests {
                pending.push(transport.defer_leaf(
                    source_assignment(function, statement),
                    field,
                    ty(reference),
                    ty(owned),
                    &mut work,
                )?);
            }
        }
        if matches!(phase, WorkPhase::Pending) {
            pending_cost = limit - work.remaining;
            work = budget(MAX_FLOW_WORK);
        }
    }
    // Ending the handle's scope releases its exclusive audit borrow before
    // another statement, terminator, or finish can be called.
    visit(
        &mut flow,
        function,
        statement + 1..function.blocks()[0].statements().len(),
        &mut work,
    )?;
    flow.terminator(function.blocks()[0].terminator().kind(), &mut work)?;
    if matches!(phase, WorkPhase::Finish) {
        work = budget(limit);
    }
    let completed = flow.finish_with_fields(&mut work)?;
    let mut fields = completed
        .fields
        .iter()
        .map(|coverage| {
            assert!(std::ptr::eq(
                coverage.assignment(),
                source_assignment(function, statement),
            ));
            (coverage.field(), coverage.closed())
        })
        .collect::<Vec<_>>();
    fields.sort_unstable();
    Ok(Observation {
        transport: had_transport,
        pending,
        sites: completed.sites,
        fields,
        spent: match phase {
            WorkPhase::Pending => pending_cost,
            WorkPhase::Whole | WorkPhase::Finish => limit - work.remaining,
        },
    })
}

fn capture_coverage(function: &SemanticFunctionDeclV1) -> Observation {
    observe(function, CAPTURE, &LEAVES, WorkPhase::Whole, MAX_FLOW_WORK).unwrap()
}

fn assert_poisoned(function: &SemanticFunctionDeclV1) {
    let observed = capture_coverage(function);
    assert!(observed.transport);
    assert_eq!(observed.pending, [true, true]);
    assert_eq!(observed.fields, [(0, false), (1, false)]);
    assert!(observed.sites.is_empty());
    assert_eq!(
        observed.sites,
        audit(function, &types(), MAX_FLOW_WORK).unwrap().0,
    );
}

#[test]
fn coverage118_original_capture_records_two_pending_leaves_then_closes() {
    let function = body(source_statements());
    let observed = capture_coverage(&function);
    assert!(observed.transport);
    assert_eq!(observed.pending, [true, true]);
    assert_eq!(observed.fields, [(0, true), (1, true)]);
    assert_eq!(observed.sites, root_sites());
    assert_eq!(
        observed.sites,
        audit(&function, &types(), MAX_FLOW_WORK).unwrap().0,
    );

    let unrequested = observe(&function, CAPTURE, &[], WorkPhase::Whole, MAX_FLOW_WORK).unwrap();
    assert!(unrequested.transport);
    assert!(unrequested.pending.is_empty());
    assert!(unrequested.fields.is_empty());
    assert_eq!(unrequested.sites, root_sites());
}

#[test]
fn coverage118_wrong_field_or_reference_owned_pair_cannot_defer() {
    let function = body(source_statements());
    for request in [
        (3, 7, 6),
        (usize::MAX, 7, 6),
        (2, 7, 6),
        (2, 2, 2),
        (0, 6, 7),
        (0, 7, 21),
        (0, 22, 21),
        (1, 22, 6),
        (1, 7, 2),
        (1, 2, 6),
    ] {
        let observed = observe(
            &function,
            CAPTURE,
            &[request],
            WorkPhase::Whole,
            MAX_FLOW_WORK,
        )
        .unwrap();
        assert!(observed.transport);
        assert_eq!(observed.pending, [false], "request {request:?}");
        assert!(observed.fields.is_empty(), "request {request:?}");
        assert_eq!(observed.sites, root_sites());
    }
}

#[test]
fn coverage118_only_successful_aggregate_transport_can_defer_leaves() {
    let function = body(source_statements());
    // A direct borrow, a successful whole-carrier Move, and the terminal matrix
    // fixture must not grant leaf coverage for the tuple's field numbers.
    for statement in [0, 3, 6] {
        let observed = observe(
            &function,
            statement,
            &LEAVES,
            WorkPhase::Whole,
            MAX_FLOW_WORK,
        )
        .unwrap();
        if statement == 3 {
            assert!(observed.transport);
            assert_eq!(observed.pending, [false, false]);
        } else {
            assert!(!observed.transport);
        }
        assert!(observed.fields.is_empty());
        assert_eq!(observed.sites, root_sites());
    }

    let mut statements = source_statements();
    statements[CAPTURE] = assignment(
        place(6, ty(30)),
        ty(30),
        SemanticRvalueKindV1::Aggregate(
            SemanticAggregateRvalueV1::new(
                SemanticAggregateKindV1::Aggregate,
                vec![
                    SemanticOperandV1::Copy(place(4, ty(7))),
                    SemanticOperandV1::Copy(place(5, ty(7))),
                    SemanticOperandV1::Copy(place(3, ty(2))),
                ],
            )
            .unwrap(),
        ),
    );
    let rejected = capture_coverage(&body(statements));
    assert!(!rejected.transport, "wrong aggregate kind is not transport");
    assert!(rejected.pending.is_empty());
    assert!(rejected.fields.is_empty());
    assert!(rejected.sites.is_empty());
}

#[test]
fn coverage118_cloned_foreign_or_wrong_site_statement_cannot_defer() {
    let function = body(source_statements());
    let original = function.blocks()[0].statements()[CAPTURE].kind();
    let cloned = original.clone();
    let foreign = body(source_statements());
    let foreign_statement = foreign.blocks()[0].statements()[CAPTURE].kind();
    assert_eq!(&cloned, original);
    assert_eq!(foreign_statement, original);
    assert!(!std::ptr::eq(&cloned, original));
    assert!(!std::ptr::eq(foreign_statement, original));

    for (label, source, block, statement) in [
        ("clone", &cloned, 0, CAPTURE as u32),
        ("foreign body", foreign_statement, 0, CAPTURE as u32),
        ("wrong statement index", original, 0, CAPTURE as u32 + 1),
        ("wrong block index", original, 1, CAPTURE as u32),
    ] {
        let types = types();
        let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
        let mut work = budget(MAX_FLOW_WORK);
        let index =
            global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut work).unwrap();
        let explicit = direct_definition_or_lifetime_locals_v1(&function);
        let mut flow = Audit::new(
            &function,
            Some(&types),
            &facts,
            &index,
            &explicit,
            &mut work,
        )
        .unwrap();
        visit(&mut flow, &function, 0..CAPTURE, &mut work).unwrap();
        {
            let transport = flow
                .statement_with_transport(
                    SemanticTransparentBorrowSiteV1 { block, statement },
                    source,
                    &mut work,
                )
                .unwrap();
            if let Some(mut transport) = transport {
                for field in 0..2 {
                    assert!(
                        !transport
                            .defer_leaf(
                                source_assignment(&function, CAPTURE),
                                field,
                                ty(7),
                                ty(6),
                                &mut work
                            )
                            .unwrap(),
                        "{label}, field {field}",
                    );
                }
            }
        }
        visit(
            &mut flow,
            &function,
            CAPTURE + 1..function.blocks()[0].statements().len(),
            &mut work,
        )
        .unwrap();
        flow.terminator(function.blocks()[0].terminator().kind(), &mut work)
            .unwrap();
        assert!(
            flow.finish_with_fields(&mut work)
                .unwrap()
                .fields
                .is_empty()
        );
    }
}

#[test]
fn coverage118_later_escape_of_either_global_or_destination_closes_neither_leaf() {
    for (local, source_type) in [(4, 7), (5, 7), (6, 30), (7, 30), (8, 7), (9, 7)] {
        let mut statements = source_statements();
        statements.push(assignment(
            place(13, ty(7)),
            ty(7),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(local, ty(source_type)),
            },
        ));
        assert_poisoned(&body(statements));
    }
}

#[test]
fn coverage118_later_duplicate_definition_revokes_pending_closure() {
    for duplicate in [borrow(4, 1), borrow(5, 2), copy(6, 30, place(7, ty(30)))] {
        let mut statements = source_statements();
        statements.push(duplicate);
        assert_poisoned(&body(statements));
    }
}

#[test]
fn coverage118_undefined_parent_is_pending_until_whole_audit_rejects() {
    for missing in 0..2 {
        let mut statements = source_statements();
        statements[missing] = SemanticStatementKindV1::Nop;
        assert_poisoned(&body(statements));
    }
}

fn assert_no_completed_positive(
    result: Result<Observation, ProductionSemanticSsaErrorV1>,
    phase: WorkPhase,
    limit: usize,
) {
    match result {
        Err(error) => assert!(
            matches!(
                flow_work_profile_v1::original_error_for_test(error),
                ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                    resource: SsaPlannerResourceV1::WorkUnits,
                    ..
                }
            ),
            "{phase:?}, limit {limit}",
        ),
        Ok(observed) => assert!(
            observed.fields.iter().all(|&(_, closed)| !closed),
            "{phase:?}, limit {limit}: {observed:?}",
        ),
    }
}

#[test]
fn coverage118_pending_allocation_and_finish_exact_and_all_shorter_caps() {
    let function = body(source_statements());
    for phase in [WorkPhase::Pending, WorkPhase::Finish] {
        let full = observe(&function, CAPTURE, &LEAVES, phase, MAX_FLOW_WORK).unwrap();
        assert_eq!(full.pending, [true, true]);
        assert_eq!(full.fields, [(0, true), (1, true)]);
        assert_eq!(full.sites, root_sites());
        let fullcost = full.spent;
        assert!(fullcost > 0 && fullcost <= MAX_FLOW_WORK);
        assert_eq!(
            observe(&function, CAPTURE, &LEAVES, phase, fullcost).unwrap(),
            full,
        );
        assert_no_completed_positive(
            observe(&function, CAPTURE, &LEAVES, phase, fullcost - 1),
            phase,
            fullcost - 1,
        );
        // The measured phase cost bounds this sweep; no search to MAX_FLOW_WORK.
        for limit in 0..fullcost - 1 {
            assert_no_completed_positive(
                observe(&function, CAPTURE, &LEAVES, phase, limit),
                phase,
                limit,
            );
        }
    }
}

#[test]
fn coverage118_pending_and_finish_share_the_whole_audit_budget() {
    let function = body(source_statements());
    let full = capture_coverage(&function);
    assert_eq!(full.pending, [true, true]);
    assert_eq!(full.fields, [(0, true), (1, true)]);
    assert_eq!(full.sites, root_sites());
    let fullcost = full.spent;
    assert!(fullcost > 0 && fullcost <= MAX_FLOW_WORK);
    assert_eq!(
        observe(&function, CAPTURE, &LEAVES, WorkPhase::Whole, fullcost).unwrap(),
        full,
    );
    assert_no_completed_positive(
        observe(&function, CAPTURE, &LEAVES, WorkPhase::Whole, fullcost - 1),
        WorkPhase::Whole,
        fullcost - 1,
    );
}

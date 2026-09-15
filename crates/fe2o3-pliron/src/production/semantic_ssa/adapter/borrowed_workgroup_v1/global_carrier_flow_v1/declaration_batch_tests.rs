use super::*;

// Exact90 setup preimage, including scalar owner checks and all flow writes.
fn scalar_setup<'a>(
    function: &'a SemanticFunctionDeclV1,
    types: Option<&'a [SemanticTypeDeclV1]>,
    facts: &'a [GlobalBf16BorrowV1],
    statements: &'a global_statement_index_v1::GlobalStatementIndex<'a>,
    explicit: &'a BTreeSet<SemanticLocalIdV1>,
    budget: &mut Budget,
) -> Result<Audit<'a>, ProductionSemanticSsaErrorV1> {
    let Some(types) = types else {
        return Ok(Audit(None));
    };
    if facts.is_empty() {
        return Ok(Audit(None));
    }
    let mut shapes = Shapes::new(types, facts, budget)?;
    let mut flow = Flow {
        by_local: BTreeMap::new(),
        nodes: Vec::new(),
        definitions: Vec::new(),
        edges: Vec::new(),
        roots: Vec::new(),
    };
    for (local, declaration) in function.locals().iter().enumerate() {
        budget.charge(1)?;
        if !shapes.contains(declaration.ty(), budget)? {
            continue;
        }
        budget.charge(3 + map_work(flow.by_local.len()))?;
        flow.by_local.insert(local as u32, flow.nodes.len());
        push(
            &mut flow.nodes,
            SemanticBorrowCandidateV1 {
                site: SemanticTransparentBorrowSiteV1 {
                    block: u32::MAX,
                    statement: u32::MAX,
                },
                source_local: local as u32,
                source_type: declaration.ty(),
                source_reference: None,
                value_alias: true,
                source_kind: SemanticBorrowCandidateSourceV1::Direct,
                valid: declaration.role() != SemanticLocalRoleV1::Return,
                consumers: 0,
                intrinsic_consumer: false,
            },
            budget,
        )?;
        push(
            &mut flow.definitions,
            matches!(declaration.role(), SemanticLocalRoleV1::Argument(_)),
            budget,
        )?;
        push(&mut flow.edges, Vec::new(), budget)?;
    }
    if flow.nodes.is_empty() {
        return Ok(Audit(None));
    }
    let relevance = Relevance::new(
        function.locals().len(),
        flow.by_local.keys().copied(),
        budget,
    )?;
    Ok(Audit(Some(State {
        pending_fields: Vec::new(),
        function,
        shapes,
        flow,
        statements,
        explicit,
        facts: facts.len(),
        relevance,
    })))
}

fn same_setup(left: &Audit<'_>, right: &Audit<'_>, function: &SemanticFunctionDeclV1) {
    let left = left.0.as_ref().unwrap();
    let right = right.0.as_ref().unwrap();
    assert!(std::ptr::eq(left.function, function));
    assert!(std::ptr::eq(right.function, function));
    assert_eq!(left.flow.by_local, right.flow.by_local);
    assert_eq!(left.flow.definitions, right.flow.definitions);
    assert_eq!(left.flow.edges, right.flow.edges);
    assert_eq!(left.flow.roots, right.flow.roots);
    assert_eq!(left.flow.nodes.len(), right.flow.nodes.len());
    for (a, b) in left.flow.nodes.iter().zip(&right.flow.nodes) {
        assert_eq!(
            (
                a.site,
                a.source_local,
                a.source_type,
                a.source_reference,
                a.value_alias,
                a.source_kind,
                a.valid,
                a.consumers,
                a.intrinsic_consumer
            ),
            (
                b.site,
                b.source_local,
                b.source_type,
                b.source_reference,
                b.value_alias,
                b.source_kind,
                b.valid,
                b.consumers,
                b.intrinsic_consumer
            )
        );
    }
}

fn finish_source(
    mut audit: Audit<'_>,
    function: &SemanticFunctionDeclV1,
    work: &mut Budget,
) -> Result<BTreeSet<SemanticTransparentBorrowSiteV1>, ProductionSemanticSsaErrorV1> {
    for (block, body) in function.blocks().iter().enumerate() {
        for (statement, source) in body.statements().iter().enumerate() {
            audit.statement(
                SemanticTransparentBorrowSiteV1 {
                    block: block as u32,
                    statement: statement as u32,
                },
                source.kind(),
                work,
            )?;
        }
        audit.terminator(body.terminator().kind(), work)?;
    }
    audit.finish(work)
}

fn original_fixture_shared_probe_work(function: &SemanticFunctionDeclV1) -> usize {
    assert_eq!(
        function
            .locals()
            .iter()
            .map(|local| local.ty().index())
            .collect::<Vec<_>>(),
        [0, 6, 6, 2, 7, 7, 30, 30, 7, 7, 8, 7, 5, 7]
    );
    // Eight selected references/carriers skip the new probe. Five unselected
    // non-pointers cost one each. The slice pointer's cold shared lookup and
    // publication read/write three packed words. Metadata rejects in the
    // thirteen prepaid canonical checks, before a pointee walk.
    let word_cells = std::mem::size_of::<u64>().div_ceil(std::mem::size_of::<usize>());
    5 + 1 + 13 + 3 * word_cells
}

#[test]
fn declaration_batch_setup_and_full_flow_match_scalar_preimage_on_original_mutations() {
    assert_eq!(MAX_FLOW_WORK, 262_144);
    let types = types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let index =
        global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut budget(MAX_FLOW_WORK))
            .unwrap();
    for mutation in 0..7 {
        let mut statements = source_statements();
        match mutation {
            0 => {}
            1 => statements.push(copy(7, 30, place(7, ty(30)))),
            2 => statements[0] = SemanticStatementKindV1::Nop,
            3 => statements.push(SemanticStatementKindV1::Assume(SemanticOperandV1::Copy(
                place(9, ty(7)),
            ))),
            4 => {
                statements.push(SemanticStatementKindV1::StorageDead(
                    SemanticLocalIdV1::from_index(9),
                ));
                statements.push(SemanticStatementKindV1::StorageLive(
                    SemanticLocalIdV1::from_index(9),
                ));
            }
            5 => statements.push(assignment(
                place(4, ty(7)),
                ty(2),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, ty(2)))),
            )),
            6 => statements.push(borrow(4, 1)),
            _ => unreachable!(),
        }
        let function = body(statements);
        let explicit = direct_definition_or_lifetime_locals_v1(&function);
        let mut old_work = budget(MAX_FLOW_WORK);
        let mut new_work = budget(MAX_FLOW_WORK);
        let old = scalar_setup(
            &function,
            Some(&types),
            &facts,
            &index,
            &explicit,
            &mut old_work,
        )
        .unwrap();
        let new = Audit::new(
            &function,
            Some(&types),
            &facts,
            &index,
            &explicit,
            &mut new_work,
        )
        .unwrap();
        same_setup(&old, &new, &function);
        assert_eq!(
            new_work.remaining + original_fixture_shared_probe_work(&function),
            old_work.remaining + function.locals().len() - 2
        );
        let old_sites = finish_source(old, &function, &mut old_work).unwrap();
        let new_sites = finish_source(new, &function, &mut new_work).unwrap();
        assert_eq!(new_sites, old_sites, "mutation={mutation}");
        // Statement 8 first queries the slice pointer during transport. The
        // batch warmed its negative verdict during declarations; the scalar
        // control still pays the 13 checks and completed-cache write once.
        assert!(matches!(function.blocks()[0].statements()[8].kind(),
            SemanticStatementKindV1::Assign(a)
                if a.destination().local().index() == 12 && a.destination().ty() == ty(5)));
        let warm_saving =
            13 + 2 * std::mem::size_of::<u64>().div_ceil(std::mem::size_of::<usize>());
        assert_eq!(
            new_work.remaining + original_fixture_shared_probe_work(&function),
            old_work.remaining + function.locals().len() - 2 + warm_saving
        );
        if mutation == 0 {
            assert_eq!(new_sites, root_sites());
        }
    }
}

#[test]
fn declaration_batch_real_flow_storage_exact_and_every_short_setup_budget_reject() {
    let function = body(source_statements());
    let types = types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let index =
        global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut budget(MAX_FLOW_WORK))
            .unwrap();
    let explicit = direct_definition_or_lifetime_locals_v1(&function);
    let mut original = budget(MAX_FLOW_WORK);
    let old = scalar_setup(
        &function,
        Some(&types),
        &facts,
        &index,
        &explicit,
        &mut original,
    )
    .unwrap();
    // Retain the unchanged scalar preimage, plus independently enumerated
    // shared probes, including real node/definition/edge allocation.
    let required = MAX_FLOW_WORK - original.remaining - function.locals().len()
        + 2
        + original_fixture_shared_probe_work(&function);
    for limit in 0..=required {
        let mut work = budget(limit);
        match Audit::new(
            &function,
            Some(&types),
            &facts,
            &index,
            &explicit,
            &mut work,
        ) {
            Ok(new) => {
                assert_eq!(limit, required);
                assert_eq!(work.remaining, 0);
                same_setup(&old, &new, &function);
            }
            Err(error) => {
                assert!(limit < required);
                assert!(matches!(
                    flow_work_profile_v1::original_error_for_test(error),
                    ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                        resource: SsaPlannerResourceV1::WorkUnits,
                        required,
                        limit: actual,
                    } if required == limit + 1 && actual == limit
                ));
            }
        }
    }
}

#[test]
fn declaration_batch_same_sized_foreign_body_keeps_exact_local_owner_roles_and_census() {
    let first = body(source_statements());
    let mut locals = first.locals().to_vec();
    for (local, t) in [(0usize, 7), (3, 7), (4, 2)] {
        let old = &locals[local];
        locals[local] = SemanticLocalDeclV1::new(old.identity(), ty(t), old.role(), old.source());
    }
    let foreign = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([213; 32]),
        first.role(),
        first.item_definition_identity(),
        first.monomorphization_identity(),
        first.generic_type_arguments_identity(),
        first.const_generic_arguments_identity(),
        first.source(),
        first.abi().clone(),
        locals,
        first.entry(),
        first.blocks().to_vec(),
    )
    .unwrap();
    assert_eq!(first.locals().len(), foreign.locals().len());
    assert_ne!(first.locals().as_ptr(), foreign.locals().as_ptr());
    assert_ne!(first.identity(), foreign.identity());
    let types = types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let index =
        global_statement_index_v1::GlobalStatementIndex::new(&facts, &mut budget(MAX_FLOW_WORK))
            .unwrap();
    let mut rosters = Vec::new();
    for function in [&first, &foreign, &first] {
        let explicit = direct_definition_or_lifetime_locals_v1(function);
        let old = scalar_setup(
            function,
            Some(&types),
            &facts,
            &index,
            &explicit,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        let new = Audit::new(
            function,
            Some(&types),
            &facts,
            &index,
            &explicit,
            &mut budget(MAX_FLOW_WORK),
        )
        .unwrap();
        same_setup(&old, &new, function);
        let state = new.0.as_ref().unwrap();
        let mut cold_work = budget(MAX_FLOW_WORK);
        let mut cold = super::super::shapes::Shapes::new(&types, &facts, &mut cold_work).unwrap();
        let mut expected = BTreeMap::new();
        for (local, d) in function.locals().iter().enumerate() {
            if cold.contains(d.ty(), &mut cold_work).unwrap() {
                expected.insert(local as u32, expected.len());
            }
        }
        assert_eq!(state.flow.by_local, expected);
        if std::ptr::eq(function, &foreign) {
            let ret = state.flow.by_local[&0];
            let arg = state.flow.by_local[&3];
            assert!(!state.flow.nodes[ret].valid);
            assert!(!state.flow.definitions[ret]);
            assert!(state.flow.nodes[arg].valid);
            assert!(state.flow.definitions[arg]);
            assert!(!state.flow.by_local.contains_key(&4));
        }
        rosters.push(expected);
    }
    assert_ne!(rosters[0], rosters[1]);
    assert_eq!(rosters[0], rosters[2]);
}

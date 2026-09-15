mod definition_memo_tests {
    use super::*;
    use std::mem::size_of;

    const WORK: usize = 1_000_000;

    fn graph<'a>(
        source: &fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'a>,
        indexed: bool,
    ) -> CapabilitySsaGraphV1<'a> {
        let graph =
            CapabilitySsaGraphV1::new(source.function(), source.plan().plan(), WORK).unwrap();
        if indexed {
            graph.with_definition_source(source).unwrap()
        } else {
            graph
        }
    }

    fn layout() -> (usize, usize) {
        let word = size_of::<usize>();
        let header = size_of::<&SemanticFunctionDeclV1>()
            + size_of::<&SsaConstructionPlanV1>()
            + size_of::<Vec<Option<CapabilityDefinitionSiteV1>>>()
            + word;
        let site_fields = 2 * size_of::<u32>() + size_of::<Option<u32>>();
        assert_eq!(
            size_of::<Option<(&SemanticFunctionDeclV1, &SsaConstructionPlanV1)>>(),
            2 * word
        );
        assert_eq!(
            size_of::<Vec<Option<CapabilityDefinitionSiteV1>>>(),
            3 * word
        );
        assert_eq!(size_of::<CapabilityDefinitionMemoV1<'_>>(), header);
        assert_eq!(size_of::<CapabilityDefinitionSiteV1>(), site_fields);
        assert_eq!(size_of::<Option<CapabilityDefinitionSiteV1>>(), site_fields);
        let cells = (header.div_ceil(word), site_fields.div_ceil(word));
        if word == 8 {
            assert_eq!(cells, (6, 2));
        }
        cells
    }

    fn values(ssa: &SsaConstructionPlanV1) -> Vec<SsaValueV1> {
        ssa.reverse_postorder()
            .iter()
            .flat_map(|block| ssa.resolved_events(*block).unwrap())
            .filter_map(|(_, event)| match event {
                SsaResolvedEventV1::Define { value, .. } => Some(*value),
                _ => None,
            })
            .collect()
    }

    fn index(value: SsaValueV1) -> usize {
        let SsaValueV1::Definition(id) = value else {
            panic!("expected a dense definition ID");
        };
        id.get() as usize
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Snapshot {
        owner: Option<(usize, usize)>,
        rows: Vec<Option<CapabilityDefinitionSiteV1>>,
        populated: usize,
        capacity: usize,
        allocation: usize,
    }

    fn snapshot(graph: &CapabilitySsaGraphV1<'_>) -> Snapshot {
        let memo = &graph.reuse.definitions;
        assert_eq!(memo.len(), memo.populated);
        assert_eq!(memo.is_empty(), memo.populated == 0);
        Snapshot {
            owner: memo
                .owner
                .map(|(body, ssa)| (body as *const _ as usize, ssa as *const _ as usize)),
            rows: memo.rows.clone(),
            populated: memo.populated,
            capacity: memo.rows.capacity(),
            allocation: memo.rows.as_ptr() as usize,
        }
    }

    fn unallocated(graph: &CapabilitySsaGraphV1<'_>) {
        let state = snapshot(graph);
        assert_eq!(state.owner, None);
        assert!(state.rows.is_empty());
        assert_eq!((state.populated, state.capacity), (0, 0));
    }

    fn no_loan_authority(graph: &CapabilitySsaGraphV1<'_>) {
        assert!(graph.reuse.uses.is_empty());
        assert!(graph.reuse.loans.is_empty());
        assert!(graph.reuse.loan_regions.is_empty());
        assert!(graph.reuse.reachability.is_empty());
        assert!(graph.reuse.owner_invalidations.is_empty());
        assert!(graph.reuse.endpoint_scc.is_none());
        assert!(graph.reuse.path_region_scratch.is_none());
        assert!(graph.reuse.region_acyclic_scratch.is_none());
    }

    fn work_failure<T: std::fmt::Debug>(result: &Result<T, ProductionSemanticKirErrorV1>) {
        assert!(
            matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual, limit,
        }) if *actual == WORK + 1 && *limit == WORK),
            "{result:?}"
        );
    }

    fn original_rejection(graph: &mut CapabilitySsaGraphV1<'_>, value: SsaValueV1) {
        let prefix = match value {
            SsaValueV1::BlockArgument { .. } => 1,
            SsaValueV1::Definition(id) if id.get() as usize >= graph.ssa.definition_count() => 2,
            SsaValueV1::Definition(_) => 5 + layout().1,
        };
        let state = snapshot(graph);
        for _ in 0..2 {
            let mut original = CapabilitySsaGraphV1::new(graph.body, graph.ssa, WORK).unwrap();
            original.reuse.definition_source = graph.reuse.definition_source;
            let before = (graph.remaining, original.remaining);
            let expected = original.definition_uncached(value);
            let actual = graph.definition(value);
            assert!(matches!(
                expected,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. }
                    | ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
            reuse_assert_same(actual, expected);
            assert_eq!(
                before.0 - graph.remaining,
                prefix + before.1 - original.remaining
            );
            assert_eq!(snapshot(graph), state);
        }
    }

    #[test]
    fn selected_success_alone_populates_dense_storage() {
        let owner = indexed_owner(vec![
            block(
                0,
                vec![
                    assign(1, None),
                    assign(2, None),
                    assign(3, None),
                    assign(3, None),
                ],
                None,
            ),
            block(1, vec![assign(1, None), assign(1, None)], None),
        ]);
        let source = endpoint_query(&owner);
        let ssa = source.plan().plan();
        let definitions = values(ssa);
        assert_eq!(definitions.len(), 4);
        assert_eq!(ssa.reverse_postorder().len(), 1);
        let selected = definitions[1];
        let expected = CapabilityDefinitionSiteV1 {
            block: 0,
            statement: Some(1),
            local: 2,
        };
        for indexed in [false, true] {
            let mut memo = graph(&source, indexed);
            unallocated(&memo);
            for &ambiguous in &definitions[2..] {
                original_rejection(&mut memo, ambiguous);
            }
            unallocated(&memo);
            for _ in 0..2 {
                assert_eq!(memo.definition(selected).unwrap(), expected);
                let table = &memo.reuse.definitions;
                assert_eq!(table.rows.len(), ssa.definition_count());
                assert_eq!(table.len(), 1);
                assert!(!table.is_empty());
                let (body, plan) = table.owner.unwrap();
                assert!(std::ptr::eq(body, source.function()));
                assert!(std::ptr::eq(plan, ssa));
                for (id, row) in table.rows.iter().enumerate() {
                    assert_eq!(*row, (id == index(selected)).then_some(expected));
                }
            }
            for &ambiguous in &definitions[2..] {
                original_rejection(&mut memo, ambiguous);
            }
            no_loan_authority(&memo);
        }
    }

    #[test]
    fn warm_equal_hash_body_and_ssa_clones_fail_the_pointer_guard() {
        let owner = indexed_owner(indexed_blocks(2));
        let source = endpoint_query(&owner);
        let body_copy = source.function().clone();
        let ssa_copy = source.plan().plan().clone();
        assert_eq!(&body_copy, source.function());
        assert_eq!(&ssa_copy, source.plan().plan());
        assert_eq!(body_copy.identity(), source.function().identity());
        assert_eq!(ssa_copy.identity(), source.plan().plan().identity());
        let value = values(&ssa_copy)[0];
        for indexed in [false, true] {
            for (body, ssa) in [
                (&body_copy, source.plan().plan()),
                (source.function(), &ssa_copy),
                (&body_copy, &ssa_copy),
            ] {
                assert_eq!(
                    ssa.definition_count(),
                    source.plan().plan().definition_count()
                );
                let mut memo = graph(&source, indexed);
                let site = memo.definition(value).unwrap();
                let state = snapshot(&memo);
                memo.body = body;
                memo.ssa = ssa;
                for allowance in 0..=5 {
                    memo.remaining = allowance;
                    let result = memo.definition(value);
                    if allowance < 5 {
                        work_failure(&result);
                    } else {
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
                        ));
                    }
                    let mut expected = allowance;
                    debit(&mut expected, &[1, 1, 3]);
                    assert_eq!(memo.remaining, expected);
                    assert_eq!(snapshot(&memo), state);
                }
                memo.remaining = WORK;
                for id in [u32::try_from(ssa.definition_count()).unwrap(), u32::MAX] {
                    original_rejection(
                        &mut memo,
                        SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(id)),
                    );
                }
                assert_eq!(snapshot(&memo), state);
                memo.body = source.function();
                memo.ssa = source.plan().plan();
                memo.remaining = 5 + layout().1;
                assert_eq!(memo.definition(value).unwrap(), site);
                assert_eq!(memo.remaining, 0);
                assert_eq!(snapshot(&memo), state);
            }
        }
    }

    #[test]
    fn missing_dense_ids_keep_original_rejection_without_allocating() {
        for blocks in [graph_body(&[vec![]]).blocks().to_vec(), indexed_blocks(2)] {
            let owner = indexed_owner(blocks);
            let source = endpoint_query(&owner);
            let ssa = source.plan().plan();
            for indexed in [false, true] {
                let mut memo = graph(&source, indexed);
                for warm in [false, true] {
                    if warm && let Some(value) = values(ssa).first() {
                        memo.definition(*value).unwrap();
                    }
                    for id in [u32::try_from(ssa.definition_count()).unwrap(), u32::MAX] {
                        original_rejection(
                            &mut memo,
                            SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(id)),
                        );
                    }
                    if memo.reuse.definitions.is_empty() {
                        unallocated(&memo);
                    }
                }
            }
        }
    }

    #[test]
    fn actual_phi_and_entry_definitions_are_never_published() {
        let scaffold = graph_body(&[vec![1, 2], vec![3], vec![3], vec![]]);
        let owner = indexed_owner(
            scaffold
                .blocks()
                .iter()
                .enumerate()
                .map(|(id, block)| {
                    SemanticBasicBlockV1::new(
                        block.identity(),
                        block.source(),
                        match id {
                            1 | 2 => vec![assign(1, None)],
                            3 => vec![assign(2, Some(SemanticOperandV1::Copy(place(1))))],
                            _ => vec![],
                        },
                        block.terminator().clone(),
                    )
                    .unwrap()
                })
                .collect(),
        );
        let source = endpoint_query(&owner);
        let phi = graph(&source, false).use_value_uncached(3, 1).unwrap();
        assert!(matches!(phi, SsaValueV1::BlockArgument { .. }));
        let parameter_owner =
            indexed_owner_with_parameter(vec![indexed_call(0, 1, 2), block(1, vec![], None)], true);
        let parameter = endpoint_query(&parameter_owner);
        let entry = parameter.plan().plan().entry_definitions()[0].value();
        for (source, value) in [(&source, phi), (&parameter, entry)] {
            for indexed in [false, true] {
                let mut memo = graph(source, indexed);
                original_rejection(&mut memo, value);
                unallocated(&memo);
                if let Some(warm) = values(source.plan().plan()).first() {
                    memo.definition(*warm).unwrap();
                    original_rejection(&mut memo, value);
                }
                no_loan_authority(&memo);
            }
        }
    }

    #[test]
    fn projected_missing_and_duplicate_source_assignments_keep_original_checks() {
        let owner = indexed_owner(indexed_blocks(1));
        let source = endpoint_query(&owner);
        let definitions = values(source.plan().plan());
        let statements = source.function().blocks()[0].statements();
        let SemanticStatementKindV1::Assign(first) = statements[0].kind() else {
            panic!("assignment");
        };
        let projected = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(1),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place(1).ty())
                    .unwrap(),
            ],
            place(1).ty(),
        )
        .unwrap();
        for changed in [
            vec![
                statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    projected,
                    first.value().clone(),
                ))),
                statements[1].clone(),
            ],
            vec![
                statement(SemanticStatementKindV1::Nop),
                statements[1].clone(),
            ],
            vec![
                statements[0].clone(),
                statements[0].clone(),
                statements[1].clone(),
            ],
        ] {
            let changed = body(vec![block(0, changed, None)]);
            for indexed in [false, true] {
                let mut memo =
                    CapabilitySsaGraphV1::new(&changed, source.plan().plan(), WORK).unwrap();
                // Bypass attachment only to exercise the unchanged source checker on malformed input.
                memo.reuse.definition_source = indexed.then_some(source);
                original_rejection(&mut memo, definitions[0]);
                unallocated(&memo);
                memo.definition(definitions[1]).unwrap();
                original_rejection(&mut memo, definitions[0]);
                assert_eq!(memo.reuse.definitions.len(), 1);
            }
        }
    }

    #[test]
    fn source_calls_preserve_exact_sites_and_destination_rejections() {
        let owner = indexed_owner_with_parameter(
            vec![
                indexed_call(0, 1, 2),
                indexed_call(1, 2, 3),
                block(2, vec![], None),
            ],
            true,
        );
        let source = endpoint_query(&owner);
        let ssa = source.plan().plan();
        for indexed in [false, true] {
            let mut memo = graph(&source, indexed);
            for block in 0..2 {
                let row = ssa
                    .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(block), 0))
                    .unwrap()[0];
                let site = CapabilityDefinitionSiteV1 {
                    block,
                    statement: None,
                    local: row.variable().get(),
                };
                for _ in 0..2 {
                    assert_eq!(memo.definition(row.value()).unwrap(), site);
                    reuse_assert_same(Ok(site), memo.definition_uncached(row.value()));
                }
            }
            original_rejection(&mut memo, ssa.entry_definitions()[0].value());
            assert_eq!(memo.reuse.definitions.len(), 2);
            no_loan_authority(&memo);
        }
        let value = ssa
            .edge_definitions(SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0))
            .unwrap()[0]
            .value();
        let SemanticTerminatorKindV1::Call(call) =
            source.function().blocks()[0].terminator().kind()
        else {
            panic!("call");
        };
        let projected = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), place(2).ty())
                    .unwrap(),
            ],
            place(2).ty(),
        )
        .unwrap();
        let changed_call = |destination| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    call.callee(),
                    call.arguments().to_vec(),
                    Some(SemanticCallDestinationV1::new(
                        destination,
                        call.destination().unwrap().edge(),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        for terminator in [
            SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::Goto,
                SemanticBlockIdV1::from_index(1),
            )),
            changed_call(place(3)),
            changed_call(projected),
        ] {
            let mut blocks = source.function().blocks().to_vec();
            blocks[0] = indexed_call_block(0, terminator);
            let changed = body(blocks);
            for indexed in [false, true] {
                let mut memo = CapabilitySsaGraphV1::new(&changed, ssa, WORK).unwrap();
                // As above, test the private source-checking boundary below attachment.
                memo.reuse.definition_source = indexed.then_some(source);
                original_rejection(&mut memo, value);
                unallocated(&memo);
            }
        }
    }

    fn debit(remaining: &mut usize, charges: &[usize]) -> bool {
        for &charge in charges {
            let Some(next) = remaining.checked_sub(charge) else {
                return false;
            };
            *remaining = next;
        }
        true
    }

    #[test]
    fn every_short_cold_warm_miss_and_hit_budget_preserves_exact_publication() {
        let owner = indexed_owner(indexed_blocks(2));
        let source = endpoint_query(&owner);
        let definitions = values(source.plan().plan());
        let count = source.plan().plan().definition_count();
        let (header, slot) = layout();
        for indexed in [false, true] {
            // The allocation shape is observed; every debit is independently specified.
            let mut complete = graph(&source, indexed);
            let before = complete.remaining;
            complete.definition(definitions[0]).unwrap();
            let excess = complete.reuse.definitions.rows.capacity() - count;
            let mut uncached = graph(&source, indexed);
            let original_before = uncached.remaining;
            uncached.definition_uncached(definitions[0]).unwrap();
            let first_publication = [header, count * slot, excess * slot, count * slot, slot + 3];
            assert_eq!(
                before - complete.remaining,
                5 + slot + original_before - uncached.remaining
                    + first_publication.iter().sum::<usize>()
            );
            for (warm, hit) in [(false, false), (true, false), (true, true)] {
                let value = definitions[usize::from(warm && !hit)];
                let mut original = graph(&source, indexed);
                let before = original.remaining;
                let site = original.definition_uncached(value).unwrap();
                let original_cost = before - original.remaining;
                let publication = if hit {
                    vec![]
                } else if warm {
                    vec![slot + 1]
                } else {
                    first_publication.to_vec()
                };
                let needed = 5
                    + slot
                    + if hit {
                        0
                    } else {
                        original_cost + publication.iter().sum::<usize>()
                    };
                for allowance in 0..=needed {
                    let mut memo = graph(&source, indexed);
                    if warm {
                        memo.definition(definitions[0]).unwrap();
                    }
                    let state = snapshot(&memo);
                    memo.remaining = allowance;
                    let actual = memo.definition(value);
                    let mut remaining = allowance;
                    let mut success = debit(&mut remaining, &[1, 1, 3, slot]);
                    if success && !hit {
                        let mut reference = graph(&source, indexed);
                        reference.remaining = remaining;
                        let result = reference.definition_uncached(value);
                        remaining = reference.remaining;
                        success = result.is_ok();
                        if !success {
                            work_failure(&result);
                        }
                        if success {
                            success = debit(&mut remaining, &publication);
                        }
                    }
                    assert_eq!(
                        memo.remaining, remaining,
                        "indexed={indexed}, warm={warm}, hit={hit}, budget={allowance}"
                    );
                    assert_eq!(success, allowance == needed);
                    if allowance < needed {
                        work_failure(&actual);
                        assert_eq!(snapshot(&memo), state);
                        if !warm {
                            unallocated(&memo);
                        }
                        memo.remaining = needed;
                        assert_eq!(memo.definition(value).unwrap(), site);
                        assert_eq!(memo.remaining, 0);
                    } else {
                        assert_eq!(actual.unwrap(), site);
                    }
                    let table = &memo.reuse.definitions;
                    assert_eq!(table.rows.len(), count);
                    assert_eq!(table.len(), if warm && !hit { 2 } else { 1 });
                    assert_eq!(table.rows[index(value)], Some(site));
                    if warm {
                        assert_eq!(table.rows.as_ptr() as usize, state.allocation);
                        assert_eq!(table.rows.capacity(), state.capacity);
                        assert_eq!(
                            table.rows[index(definitions[0])],
                            state.rows[index(definitions[0])]
                        );
                    }
                    no_loan_authority(&memo);
                }
            }
        }
    }

    #[test]
    fn many_definitions_have_constant_warm_cost_without_loan_authority() {
        let owner = indexed_owner(indexed_blocks(24));
        let source = endpoint_query(&owner);
        let definitions = values(source.plan().plan());
        assert_eq!(definitions.len(), 48);
        for indexed in [false, true] {
            let mut memo = graph(&source, indexed);
            let mut original = graph(&source, indexed);
            let total_before = (memo.remaining, original.remaining);
            let mut sites = Vec::new();
            for &value in &definitions {
                let site = memo.definition(value).unwrap();
                reuse_assert_same(Ok(site), original.definition_uncached(value));
                sites.push(site);
            }
            let state = snapshot(&memo);
            let before = (memo.remaining, original.remaining);
            for _ in 0..3 {
                for (&value, &site) in definitions.iter().zip(&sites).rev() {
                    let available = memo.remaining;
                    assert_eq!(memo.definition(value).unwrap(), site);
                    assert_eq!(available - memo.remaining, 5 + layout().1);
                    reuse_assert_same(Ok(site), original.definition_uncached(value));
                }
            }
            assert_eq!(snapshot(&memo), state);
            assert_eq!(memo.reuse.definitions.len(), definitions.len());
            assert!(before.0 - memo.remaining < before.1 - original.remaining);
            assert!(total_before.0 - memo.remaining < total_before.1 - original.remaining);
            no_loan_authority(&memo);
            let loan = CapabilityLoanV1 {
                borrow: sites[1],
                owner_local: 1,
                owner_value: definitions[0],
            };
            let consumer = *sites.last().unwrap();
            for _ in 0..2 {
                let actual = memo.loan_live(loan, consumer);
                assert!(matches!(
                    actual,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ));
                reuse_assert_same(actual, original.loan_live_reference(loan, consumer));
                assert!(memo.reuse.loans.is_empty());
                assert_eq!(snapshot(&memo), state);
            }
        }
    }
}

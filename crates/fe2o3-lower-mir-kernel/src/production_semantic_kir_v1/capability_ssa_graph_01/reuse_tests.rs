// Scalar graph components only; none of these fixtures issues a capability.
fn reuse_assert_same<T: Eq + std::fmt::Debug>(
    actual: Result<T, ProductionSemanticKirErrorV1>,
    reference: Result<T, ProductionSemanticKirErrorV1>,
) {
    use ProductionSemanticKirErrorV1 as Error;
    match (actual, reference) {
        (Ok(actual), Ok(reference)) => assert_eq!(actual, reference),
        (Err(Error::CorrespondenceMismatch), Err(Error::CorrespondenceMismatch)) => {}
        (
            Err(Error::Unsupported {
                function,
                block,
                statement,
                detail,
            }),
            Err(Error::Unsupported {
                function: other_function,
                block: other_block,
                statement: other_statement,
                detail: other_detail,
            }),
        ) => assert_eq!(
            (function, block, statement, detail),
            (other_function, other_block, other_statement, other_detail),
        ),
        (actual, reference) => panic!("actual={actual:?}; reference={reference:?}"),
    }
}

fn reuse_body(edges: &[Vec<u32>]) -> SemanticFunctionDeclV1 {
    let scaffold = graph_body(edges);
    body(
        scaffold
            .blocks()
            .iter()
            .enumerate()
            .map(|(index, block)| {
                SemanticBasicBlockV1::new(
                    block.identity(),
                    block.source(),
                    if index == 0 {
                        vec![
                            assign(1, None),
                            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
                        ]
                    } else {
                        vec![assign(3, None)]
                    },
                    block.terminator().clone(),
                )
                .unwrap()
            })
            .collect(),
    )
}

fn reuse_consumer(block: u32, statement: Option<u32>) -> CapabilityDefinitionSiteV1 {
    CapabilityDefinitionSiteV1 {
        block,
        statement,
        local: 3,
    }
}

fn reuse_definition_slot_words() -> usize {
    std::mem::size_of::<Option<CapabilityDefinitionSiteV1>>().div_ceil(std::mem::size_of::<usize>())
}

fn reuse_definition_lookup_work() -> usize {
    // Value tag, dense bound, owner presence and pointer identities, slot read.
    1 + 1 + 3 + reuse_definition_slot_words()
}

fn reuse_definition_first_publication_charges(count: usize) -> [usize; 5] {
    let slot = reuse_definition_slot_words();
    let capacity = Vec::<Option<CapabilityDefinitionSiteV1>>::with_capacity(count).capacity();
    // Temporary header, allocation and surplus, initialization, atomic commit.
    [
        std::mem::size_of::<CapabilityDefinitionMemoV1<'_>>()
            .div_ceil(std::mem::size_of::<usize>()),
        count * slot,
        (capacity - count) * slot,
        count * slot,
        slot + 3,
    ]
}

#[test]
fn capability_reuse_matches_reference_on_seeded_loans_and_cycles() {
    let mut seed = 0xc478_823f_u32;
    for _ in 0..48 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 4).map(|_| next() % 8).collect())
            .collect();
        let body = reuse_body(&edges);
        let plan = plan(&body);
        let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let owner = cached.use_value(0, 1).unwrap();
        for target in 0..8 {
            for statement in [Some(0), None] {
                let consumer = reuse_consumer(target, statement);
                for _ in 0..2 {
                    reuse_assert_same(
                        cached.loan_live(loan(owner, 1), consumer),
                        reference.loan_live_reference(loan(owner, 1), consumer),
                    );
                }
            }
        }
    }
}

#[test]
fn capability_reuse_preserves_dead_paths_and_cycle_rejections() {
    for edges in [
        vec![vec![1, 4], vec![2], vec![3], vec![], vec![4], vec![3]],
        vec![vec![1, 4], vec![2], vec![1, 3], vec![], vec![4], vec![3]],
        vec![vec![1, 1], vec![2], vec![]],
        vec![vec![0]],
    ] {
        let body = reuse_body(&edges);
        let plan = plan(&body);
        let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let owner = cached.use_value(0, 1).unwrap();
        for target in 0..edges.len() as u32 {
            reuse_assert_same(
                cached.loan_live(loan(owner, 1), reuse_consumer(target, None)),
                reference.loan_live_reference(loan(owner, 1), reuse_consumer(target, None)),
            );
        }
    }
}

#[test]
fn capability_reuse_preserves_statement_endpoints_and_invalidations() {
    for invalidation in [
        assign(3, Some(SemanticOperandV1::Move(place(1)))),
        assign(1, None),
        statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(1),
        )),
        statement(SemanticStatementKindV1::StorageDead(
            SemanticLocalIdV1::from_index(1),
        )),
    ] {
        let body = body(vec![block(
            0,
            vec![
                assign(1, None),
                assign(2, Some(SemanticOperandV1::Copy(place(1)))),
                statement(SemanticStatementKindV1::Nop),
                invalidation,
            ],
            None,
        )]);
        let plan = plan(&body);
        let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let owner = cached.use_value(0, 1).unwrap();
        cached
            .loan_live(loan(owner, 1), reuse_consumer(0, Some(2)))
            .unwrap();
        for endpoint in [Some(2), Some(3), None] {
            reuse_assert_same(
                cached.loan_live(loan(owner, 1), reuse_consumer(0, endpoint)),
                reference.loan_live_reference(loan(owner, 1), reuse_consumer(0, endpoint)),
            );
        }
        assert!(matches!(
            cached.loan_live(loan(owner, 1), reuse_consumer(0, None)),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "capability loan crosses a move, overwrite, deinitialization or storage death",
                ..
            })
        ));
        assert!(
            !cached
                .reuse
                .loans
                .contains_key(&(loan(owner, 1), reuse_consumer(0, None)))
        );
    }
}

#[test]
fn capability_reuse_deinitialization_retains_storage_instead_of_inventing_ssa_use() {
    // Deinitialization makes the real scalar owner non-promotable. Ordered
    // source-scanner coverage remains in the independent invalidation oracle.
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            statement(SemanticStatementKindV1::Nop),
            statement(SemanticStatementKindV1::Deinitialize(place(1))),
        ],
        None,
    )]);
    let plan = plan(&body);
    assert!(
        !plan
            .plan()
            .promoted_variables()
            .contains(&fe2o3_mir_model::SsaVariableIdV1::new(1))
    );
    let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    for _ in 0..2 {
        let actual = cached.use_value(0, 1);
        assert!(matches!(
            &actual,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "capability reference has no exact SSA use",
            })
        ));
        reuse_assert_same(actual, reference.use_value_uncached(0, 1));
    }
    assert!(cached.reuse.uses.is_empty());
    assert!(cached.reuse.loans.is_empty());
}

#[test]
fn capability_reuse_loan_key_includes_every_owner_borrow_and_consumer_axis() {
    let body = reuse_body(&[vec![1], vec![]]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    let original = (loan(owner, 1), reuse_consumer(1, Some(0)));
    graph.loan_live(original.0, original.1).unwrap();
    let mut changed = Vec::new();
    let mut key = original;
    key.0.borrow.block = 1;
    changed.push(key);
    let mut key = original;
    key.0.borrow.statement = None;
    changed.push(key);
    let mut key = original;
    key.0.borrow.statement = Some(0);
    changed.push(key);
    let mut key = original;
    key.0.borrow.local = 3;
    changed.push(key);
    let mut key = original;
    key.0.owner_local = 2;
    changed.push(key);
    let mut key = original;
    key.0.owner_value = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(999));
    changed.push(key);
    let mut key = original;
    key.1.block = 0;
    changed.push(key);
    let mut key = original;
    key.1.statement = None;
    changed.push(key);
    let mut key = original;
    key.1.statement = Some(1);
    changed.push(key);
    let mut key = original;
    key.1.local = 2;
    changed.push(key);
    for key in changed {
        // Enough for exactly the outer lookup, not even a nested cold query.
        graph.remaining = lookup_work(graph.reuse.loans.len());
        assert!(matches!(
            graph.loan_live(key.0, key.1),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                ..
            })
        ));
        assert_eq!(graph.reuse.loans.len(), 1);
    }
    graph.remaining = lookup_work(graph.reuse.loans.len());
    graph.loan_live(original.0, original.1).unwrap();
    assert_eq!(graph.remaining, 0);
}

#[test]
fn capability_reuse_use_and_definition_failures_are_not_cached() {
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            assign(1, None),
            assign(3, Some(SemanticOperandV1::Copy(place(1)))),
        ],
        None,
    )]);
    let plan = plan(&body);
    let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    for _ in 0..2 {
        reuse_assert_same(cached.use_value(0, 1), reference.use_value_uncached(0, 1));
        reuse_assert_same(cached.use_value(0, 0), reference.use_value_uncached(0, 0));
        let definitions: Vec<_> = plan
            .plan()
            .resolved_events(SsaBlockIdV1::new(0))
            .unwrap()
            .iter()
            .filter_map(|(_, event)| match event {
                SsaResolvedEventV1::Define { variable, value } if variable.get() == 1 => {
                    Some(*value)
                }
                _ => None,
            })
            .collect();
        assert_eq!(definitions.len(), 2);
        for value in definitions {
            reuse_assert_same(
                cached.definition(value),
                reference.definition_uncached(value),
            );
        }
        let missing = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(999));
        reuse_assert_same(
            cached.definition(missing),
            reference.definition_uncached(missing),
        );
    }
    assert!(cached.reuse.uses.is_empty());
    assert!(cached.reuse.definitions.is_empty());
    assert!(cached.reuse.definitions.owner.is_none());
    assert!(cached.reuse.definitions.rows.is_empty());
    assert_eq!(cached.reuse.definitions.rows.capacity(), 0);
}

#[test]
fn capability_reuse_definitions_keep_exact_call_return_edge() {
    let call = SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(1),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(1),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    );
    let first = SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([60; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        vec![],
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), call),
    )
    .unwrap();
    let body = body(vec![
        first,
        block(
            1,
            vec![assign(2, Some(SemanticOperandV1::Copy(place(1))))],
            None,
        ),
    ]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let value = graph.use_value(1, 1).unwrap();
    for _ in 0..2 {
        assert_eq!(
            graph.definition(value).unwrap(),
            CapabilityDefinitionSiteV1 {
                block: 0,
                statement: None,
                local: 1,
            }
        );
    }
    let SemanticTerminatorKindV1::Call(original_call) = body.blocks()[0].terminator().kind() else {
        unreachable!()
    };
    for terminator in [
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(1),
        )),
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                original_call.callee(),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(3),
                    original_call.destination().unwrap().edge(),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    ] {
        let mut blocks = body.blocks().to_vec();
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            vec![],
            SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
        )
        .unwrap();
        let changed = self::body(blocks);
        let mut cached = CapabilitySsaGraphV1::new(&changed, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&changed, plan.plan(), 100_000).unwrap();
        let actual = cached.definition(value);
        assert!(actual.is_err());
        reuse_assert_same(actual, reference.definition_uncached(value));
        assert!(cached.reuse.definitions.is_empty());
    }
}

#[test]
fn capability_reuse_projected_or_missing_source_assignments_still_reject() {
    let scaffold = reuse_body(&[vec![]]);
    let plan = plan(&scaffold);
    let mut probe = CapabilitySsaGraphV1::new(&scaffold, plan.plan(), 100_000).unwrap();
    let value = probe.use_value(0, 1).unwrap();
    let ty = SemanticTypeIdV1::from_index(1);
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty).unwrap()],
        ty,
    )
    .unwrap();
    let SemanticStatementKindV1::Assign(original) = scaffold.blocks()[0].statements()[0].kind()
    else {
        unreachable!()
    };
    for changed in [
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            projected,
            original.value().clone(),
        ))),
        statement(SemanticStatementKindV1::Nop),
    ] {
        let body = body(vec![block(
            0,
            vec![changed, scaffold.blocks()[0].statements()[1].clone()],
            None,
        )]);
        let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let actual = cached.definition(value);
        assert!(actual.is_err());
        reuse_assert_same(actual, reference.definition_uncached(value));
        assert!(cached.reuse.definitions.is_empty());
    }
}

#[test]
fn capability_reuse_failed_precharge_leaves_query_tables_unchanged() {
    let body = reuse_body(&[vec![1], vec![]]);
    let plan = plan(&body);
    let mut probe = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let value = probe.use_value_uncached(0, 1).unwrap();
    for query in 0..3 {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        let before = graph.remaining;
        match query {
            0 => {
                graph.use_value_uncached(0, 1).unwrap();
            }
            1 => {
                graph.definition_uncached(value).unwrap();
            }
            _ => {
                graph.reaches_uncached(0, 1).unwrap();
            }
        }
        let cold = before - graph.remaining;
        let (lookup, publication, final_charge) = match query {
            0 => {
                let insert = insertion_work::<(u32, u32), SsaValueV1>(0);
                (lookup_work(0), insert, insert)
            }
            1 => {
                let charges =
                    reuse_definition_first_publication_charges(plan.plan().definition_count());
                (
                    reuse_definition_lookup_work(),
                    charges.iter().sum(),
                    charges[4],
                )
            }
            _ => {
                let insert = insertion_work::<(u32, u32), bool>(0);
                (lookup_work(0), insert, insert)
            }
        };
        let required = lookup + cold + publication;
        graph.remaining = required - 1;
        let result = match query {
            0 => graph.use_value(0, 1).map(|_| ()),
            1 => graph.definition(value).map(|_| ()),
            _ => graph.reaches(0, 1).map(|_| ()),
        };
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
        ));
        assert_eq!(graph.remaining, final_charge - 1);
        assert!(graph.reuse.uses.is_empty());
        assert!(graph.reuse.definitions.is_empty());
        assert!(graph.reuse.definitions.owner.is_none());
        assert!(graph.reuse.definitions.rows.is_empty());
        assert_eq!(graph.reuse.definitions.rows.capacity(), 0);
        assert!(graph.reuse.reachability.is_empty());
        graph.remaining = required;
        match query {
            0 => {
                graph.use_value(0, 1).unwrap();
            }
            1 => {
                graph.definition(value).unwrap();
                assert_eq!(graph.reuse.definitions.len(), 1);
                assert_eq!(
                    graph.reuse.definitions.rows.len(),
                    plan.plan().definition_count()
                );
                let (bound_body, bound_ssa) = graph.reuse.definitions.owner.unwrap();
                assert!(std::ptr::eq(bound_body, &body));
                assert!(std::ptr::eq(bound_ssa, plan.plan()));
            }
            _ => {
                graph.reaches(0, 1).unwrap();
            }
        }
        assert_eq!(graph.remaining, 0);
    }
}

#[test]
fn capability_reuse_failed_loan_commit_does_not_cache_success() {
    let body = reuse_body(&[vec![1], vec![]]);
    let plan = plan(&body);
    let mut probe = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let owner = probe.use_value(0, 1).unwrap();
    let consumer = reuse_consumer(1, Some(0));
    probe.loan_live(loan(owner, 1), consumer).unwrap();
    let used = probe.limit - probe.remaining;
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), used - 1).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    assert!(matches!(graph.loan_live(loan(owner, 1), consumer),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit,
        }) if actual == used && limit == used - 1));
    assert!(graph.reuse.loans.is_empty());
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), used).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    graph.loan_live(loan(owner, 1), consumer).unwrap();
    assert_eq!(graph.remaining, 0);
    assert_eq!(graph.reuse.loans.len(), 1);
}

#[test]
fn capability_reuse_reachability_keys_keep_direction_and_false_results() {
    let body = graph_body(&[vec![1], vec![], vec![]]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    for (from, to, expected) in [(0, 1, true), (1, 0, false), (2, 2, false), (1, 1, true)] {
        assert_eq!(graph.reaches(from, to).unwrap(), expected);
        graph.remaining = lookup_work(graph.reuse.reachability.len());
        assert_eq!(graph.reaches(from, to).unwrap(), expected);
        assert_eq!(graph.remaining, 0);
        graph.remaining = 100_000;
    }
    assert_eq!(graph.reuse.reachability.len(), 4);
}

#[test]
fn capability_reuse_cache_lifetime_is_one_immutable_body_and_ssa_plan() {
    for next in [Some(1), None] {
        let body = body(vec![block(0, vec![], next), block(1, vec![], None)]);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1000).unwrap();
        assert!(graph.reuse.reachability.is_empty());
        assert_eq!(graph.reaches(0, 1).unwrap(), next.is_some());
    }
}

#[test]
fn capability_reuse_sparse_growth_is_charged_in_logical_words() {
    let body = graph_body(&[vec![1], vec![2], vec![]]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    for to in 0..3 {
        assert!(graph.reaches(0, to).unwrap());
        assert_eq!(graph.reuse.reachability.len(), to as usize + 1);
        assert!(graph.reuse.uses.is_empty());
        assert!(graph.reuse.definitions.is_empty());
        assert!(graph.reuse.loans.is_empty());
    }
    let logical_words = query_reuse::header_words()
        + graph.reuse.reachability.len() * query_reuse::entry_words::<(u32, u32), bool>();
    assert!(graph.limit - graph.remaining >= logical_words);
}

#[test]
fn capability_reuse_repeated_thirteen_consumer_graph_reduces_charged_work() {
    let count = 96u32;
    let edges: Vec<_> = (0..count)
        .map(|i| if i + 1 < count { vec![i + 1] } else { vec![] })
        .collect();
    let body = reuse_body(&edges);
    let plan = plan(&body);
    let mut cached = CapabilitySsaGraphV1::new(&body, plan.plan(), 10_000_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 10_000_000).unwrap();
    for target in count - 13..count {
        let value = cached.use_value(0, 1).unwrap();
        assert_eq!(value, reference.use_value_uncached(0, 1).unwrap());
        reuse_assert_same(
            cached.definition(value),
            reference.definition_uncached(value),
        );
        reuse_assert_same(
            cached.loan_live(loan(value, 1), reuse_consumer(target, Some(0))),
            reference.loan_live_reference(loan(value, 1), reuse_consumer(target, Some(0))),
        );
    }
    let cached_work = cached.limit - cached.remaining;
    let reference_work = reference.limit - reference.remaining;
    assert!(
        cached_work < reference_work,
        "cached={cached_work}; reference={reference_work}"
    );
    assert_eq!(cached.reuse.definitions.len(), 1);
    assert_eq!(cached.reuse.loans.len(), 13);
    let consumer = reuse_consumer(count - 1, Some(0));
    let value = cached.use_value(0, 1).unwrap();
    let before = cached.remaining;
    cached.loan_live(loan(value, 1), consumer).unwrap();
    assert_eq!(before - cached.remaining, lookup_work(13));
}

#[test]
fn capability_loan_region_reuses_geometry_without_reusing_owner_authority() {
    let body = body(vec![block(
        0,
        vec![
            assign(1, None),
            assign(2, Some(SemanticOperandV1::Copy(place(1)))),
            assign(3, Some(SemanticOperandV1::Copy(place(2)))),
            statement(SemanticStatementKindV1::Nop),
            statement(SemanticStatementKindV1::StorageDead(
                SemanticLocalIdV1::from_index(2),
            )),
        ],
        None,
    )]);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let first = graph.use_value(0, 1).unwrap();
    let second = graph.use_value(0, 2).unwrap();
    graph
        .loan_live(loan(first, 1), reuse_consumer(0, None))
        .unwrap();
    assert_eq!(graph.reuse.loan_regions.len(), 1);
    let before = graph.loan_region(0, 0).unwrap();
    let other = CapabilityLoanV1 {
        borrow: CapabilityDefinitionSiteV1 {
            block: 0,
            statement: Some(2),
            local: 3,
        },
        owner_local: 2,
        owner_value: second,
    };
    for end in [Some(3), Some(4), None] {
        let actual = graph.loan_live(other, reuse_consumer(0, end));
        assert_eq!(actual.is_ok(), end.is_some());
        reuse_assert_same(
            actual,
            reference.loan_live_before_region_reuse(other, reuse_consumer(0, end)),
        );
    }
    assert!(Arc::ptr_eq(&before, &graph.loan_region(0, 0).unwrap()));
    assert!(
        !graph
            .reuse
            .loans
            .contains_key(&(other, reuse_consumer(0, None)))
    );
    let wrong_owner = CapabilityLoanV1 {
        owner_value: first,
        ..other
    };
    assert!(matches!(
        graph.loan_live(wrong_owner, reuse_consumer(0, Some(3))),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(graph.reuse.loan_regions.len(), 1);
}

#[test]
fn capability_loan_region_matches_preimage_on_seeded_loans_and_cycles() {
    let mut seed = 0x8591_d743_u32;
    for _ in 0..64 {
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            seed >> 16
        };
        let edges: Vec<Vec<u32>> = (0..8)
            .map(|_| (0..next() % 5).map(|_| next() % 8).collect())
            .collect();
        let body = reuse_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
        let owner = graph.use_value(0, 1).unwrap();
        for block in 0..8 {
            for statement in [Some(0), None] {
                let consumer = reuse_consumer(block, statement);
                for _ in 0..2 {
                    reuse_assert_same(
                        graph.loan_live(loan(owner, 1), consumer),
                        reference.loan_live_before_region_reuse(loan(owner, 1), consumer),
                    );
                }
            }
        }
    }
}

#[test]
fn capability_loan_region_keeps_direction_dead_paths_and_duplicate_edges() {
    for edges in [
        vec![vec![1, 1], vec![2], vec![]],
        vec![vec![1, 3], vec![2], vec![], vec![3], vec![2]],
        vec![vec![1], vec![1, 2], vec![]],
        vec![vec![1], vec![2], vec![0]],
    ] {
        let body = graph_body(&edges);
        let plan = plan(&body);
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_000_000).unwrap();
        let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 1_000_000).unwrap();
        for from in 0..edges.len() as u32 {
            for to in 0..edges.len() as u32 {
                let actual = graph.loan_region(from, to).unwrap();
                let expected = reference.path_region(from, to).unwrap();
                assert_eq!(actual.blocks, expected);
                assert_eq!(
                    actual.acyclic,
                    reference
                        .region_is_acyclic_cold_reference(&expected)
                        .unwrap()
                );
                assert!(Arc::ptr_eq(&actual, &graph.loan_region(from, to).unwrap()));
            }
        }
        assert!(matches!(
            graph.loan_region(0, edges.len() as u32),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(graph.reuse.loan_regions.len(), edges.len() * edges.len());
    }
}

#[test]
fn capability_loan_region_exact_cold_and_warm_debits_never_publish_partial_rows() {
    let body = graph_body(&[vec![1, 1], vec![2], vec![]]);
    let plan = plan(&body);
    let mut measured = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let prefix = measured.remaining;
    let original = measured.loan_region(0, 2).unwrap();
    let cost = prefix - measured.remaining;
    for available in 0..cost {
        let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
        graph.remaining = available;
        for _ in 0..2 {
            let before = graph.remaining;
            assert!(matches!(
                graph.loan_region(0, 2),
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 100_001,
                    limit: 100_000,
                })
            ));
            assert!(graph.remaining <= before);
            assert!(graph.reuse.loan_regions.is_empty());
            assert!(graph.reuse.loans.is_empty());
        }
    }
    let mut exact = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    exact.remaining = cost;
    assert_eq!(exact.loan_region(0, 2).unwrap().blocks, original.blocks);
    assert_eq!(exact.remaining, 0);
    let warm = lookup_work(exact.reuse.loan_regions.len()) + 2;
    for available in 0..warm {
        exact.remaining = available;
        assert!(matches!(
            exact.loan_region(0, 2),
            Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
        ));
        assert_eq!(exact.remaining, available);
        assert_eq!(exact.reuse.loan_regions.len(), 1);
    }
    exact.remaining = warm;
    let cached = exact.loan_region(0, 2).unwrap();
    assert_eq!(cached.blocks, original.blocks);
    assert_eq!(exact.remaining, 0);
}

#[test]
fn capability_loan_region_cache_is_scoped_to_one_body_and_ssa_plan() {
    let first = graph_body(&[vec![1], vec![], vec![]]);
    let second = graph_body(&[vec![2], vec![], vec![]]);
    assert_eq!(first.identity(), second.identity());
    let first_plan = plan(&first);
    let second_plan = plan(&second);
    let mut a = CapabilitySsaGraphV1::new(&first, first_plan.plan(), 100_000).unwrap();
    let mut b = CapabilitySsaGraphV1::new(&second, second_plan.plan(), 100_000).unwrap();
    let region_a = a.loan_region(0, 1).unwrap();
    assert!(b.reuse.loan_regions.is_empty());
    let region_b = b.loan_region(0, 1).unwrap();
    assert_eq!(region_a.blocks, [true, true, false]);
    assert_eq!(region_b.blocks, [false, false, false]);
    assert!(!Arc::ptr_eq(&region_a, &region_b));
}

#[test]
fn capability_loan_region_failed_loan_commit_retains_geometry_not_a_proof() {
    let body = reuse_body(&[vec![1], vec![]]);
    let plan = plan(&body);
    let mut measured = CapabilitySsaGraphV1::new(&body, plan.plan(), 100_000).unwrap();
    let owner = measured.use_value(0, 1).unwrap();
    let consumer = reuse_consumer(1, Some(0));
    measured.loan_live(loan(owner, 1), consumer).unwrap();
    let required = measured.limit - measured.remaining;
    let mut short = CapabilitySsaGraphV1::new(&body, plan.plan(), required - 1).unwrap();
    let owner = short.use_value(0, 1).unwrap();
    assert!(matches!(short.loan_live(loan(owner, 1), consumer),
        Err(ProductionSemanticKirErrorV1::ResourceLimit { actual, limit, .. })
            if actual == required && limit == required - 1));
    assert!(short.reuse.loans.is_empty());
    assert_eq!(short.reuse.loan_regions.len(), 1);
    let mut exact = CapabilitySsaGraphV1::new(&body, plan.plan(), required).unwrap();
    let owner = exact.use_value(0, 1).unwrap();
    exact.loan_live(loan(owner, 1), consumer).unwrap();
    assert_eq!(exact.remaining, 0);
}

#[cfg(target_pointer_width = "64")]
#[test]
fn capability_loan_region_reported_prefix_layout_assumptions() {
    use std::collections::BTreeMap;
    use std::mem::size_of;

    type Key = (u32, u32);
    type Value = Arc<CapabilityLoanRegionV1>;
    let word = size_of::<usize>();
    assert_eq!(query_reuse::entry_words::<Key, Value>(), 3);
    assert_eq!(size_of::<CapabilityLoanRegionV1>().div_ceil(word) + 2, 6);
    assert_eq!(size_of::<BTreeMap<Key, Value>>().div_ceil(word), 3);
    assert_eq!(lookup_work(45) + 1, 74);
    assert_eq!(insertion_work::<Key, Value>(45) + 6, 301);
    assert_eq!((66 - 45) * (9 * 618) - 66 * 74 - 45 * 301 - 3, 98_370);
}

#[test]
fn capability_loan_region_many_distinct_consumers_avoid_repeated_cfg_work() {
    let count = 618_u32;
    let edges: Vec<_> = (0..count)
        .map(|i| if i + 1 < count { vec![i + 1] } else { vec![] })
        .collect();
    let original = reuse_body(&edges);
    let mut blocks = original.blocks().to_vec();
    let last = blocks.last_mut().unwrap();
    *last = SemanticBasicBlockV1::new(
        last.identity(),
        last.source(),
        (0..24).map(|_| assign(3, None)).collect(),
        last.terminator().clone(),
    )
    .unwrap();
    let body = body(blocks);
    let plan = plan(&body);
    let mut graph = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    let mut reference = CapabilitySsaGraphV1::new(&body, plan.plan(), 2_000_000).unwrap();
    let owner = graph.use_value(0, 1).unwrap();
    assert_eq!(reference.use_value(0, 1).unwrap(), owner);
    for statement in 0..24 {
        let consumer = reuse_consumer(count - 1, Some(statement));
        graph.loan_live(loan(owner, 1), consumer).unwrap();
        reference
            .loan_live_before_region_reuse(loan(owner, 1), consumer)
            .unwrap();
    }
    let actual_work = graph.limit - graph.remaining;
    let reference_work = reference.limit - reference.remaining;
    assert_eq!(graph.reuse.loan_regions.len(), 1);
    assert_eq!(graph.reuse.loans.len(), 24);
    assert_eq!(reference.reuse.loans.len(), 24);
    assert!(reference.reuse.loan_regions.is_empty());
    assert!(
        actual_work < reference_work / 2,
        "new={actual_work}, previous={reference_work}"
    );
}

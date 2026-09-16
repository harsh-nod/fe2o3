// These graph fixtures synthesize private witnesses only for traversal tests.
// Source admission and exact executable ownership are tested separately.
fn guarded_flow_site(index: u32) -> SemanticAccessSiteV1 {
    SemanticAccessSiteV1 {
        block: 100 + index,
        statement: Some(0),
        ordinal: 0,
    }
}

fn guarded_flow_key(operation: u32) -> u64 {
    u64::from(operation) << 32
}

fn guarded_flow_witness(block: u32, operation: u32, site: u32) -> AuthenticatedConditionalReadV1 {
    AuthenticatedConditionalReadV1 {
        location: FunctionOperationLocation::new(
            BlockId(block),
            usize::try_from(operation).unwrap(),
        ),
        operation_access_ordinal: 0,
        site: guarded_flow_site(site),
    }
}

fn guarded_flow_events(sites: &[u32]) -> BTreeMap<u32, Vec<(u64, SemanticAccessSiteV1)>> {
    BTreeMap::from([(
        0,
        sites
            .iter()
            .enumerate()
            .map(|(operation, site)| {
                (
                    guarded_flow_key(u32::try_from(operation).unwrap()),
                    guarded_flow_site(*site),
                )
            })
            .collect(),
    )])
}

fn guarded_flow_edges(
    pairs: &[(u32, u32)],
) -> BTreeSet<(SemanticAccessSiteV1, SemanticAccessSiteV1)> {
    pairs
        .iter()
        .map(|(first, second)| (guarded_flow_site(*first), guarded_flow_site(*second)))
        .collect()
}

#[test]
fn guarded_effect_flow_matches_explicit_true_read_and_empty_false_continuation() {
    let events = guarded_flow_events(&[0, 1, 2]);
    let successors = BTreeMap::from([(0, vec![])]);
    let conditional = BTreeMap::from([((0, guarded_flow_key(1)), guarded_flow_witness(0, 1, 1))]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 1_000 };
    let actual = effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget)
        .expect("exact conditional read has two finite continuations");

    // Independently spell the same operation as two CFG edges. The false
    // edge contains no event and rejoins immediately before the final load.
    let expanded_events = BTreeMap::from([
        (0, vec![(0, guarded_flow_site(0))]),
        (1, vec![(0, guarded_flow_site(1))]),
        (3, vec![(0, guarded_flow_site(2))]),
    ]);
    let expanded_successors =
        BTreeMap::from([(0, vec![1, 2]), (1, vec![3]), (2, vec![3]), (3, vec![])]);
    let expected = effect_flow_signature_v1(
        0,
        &expanded_events,
        &expanded_successors,
        &BTreeMap::new(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(actual.entry_effects, BTreeSet::from([guarded_flow_site(0)]));
    assert_eq!(
        actual.next_effects,
        guarded_flow_edges(&[(0, 1), (0, 2), (1, 2)])
    );
}

#[test]
fn guarded_effect_flow_consecutive_optional_reads_retain_each_event_and_skip_path() {
    let events = guarded_flow_events(&[0, 1, 2, 3]);
    let successors = BTreeMap::from([(0, vec![])]);
    let conditional = BTreeMap::from([
        ((0, guarded_flow_key(1)), guarded_flow_witness(0, 1, 1)),
        ((0, guarded_flow_key(2)), guarded_flow_witness(0, 2, 2)),
    ]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 1_000 };
    let flow =
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).unwrap();
    assert_eq!(flow.entry_effects, BTreeSet::from([guarded_flow_site(0)]));
    assert_eq!(
        flow.next_effects,
        guarded_flow_edges(&[(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)])
    );
}

#[test]
fn guarded_effect_flow_never_skips_an_unrelated_mandatory_neighbor() {
    let events = guarded_flow_events(&[0, 1, 2, 3, 4]);
    let successors = BTreeMap::from([(0, vec![])]);
    let conditional = BTreeMap::from([
        ((0, guarded_flow_key(1)), guarded_flow_witness(0, 1, 1)),
        ((0, guarded_flow_key(3)), guarded_flow_witness(0, 3, 3)),
    ]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 1_000 };
    let flow =
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).unwrap();
    assert_eq!(
        flow.next_effects,
        guarded_flow_edges(&[(0, 1), (0, 2), (1, 2), (2, 3), (2, 4), (3, 4)])
    );
    assert!(
        !flow
            .next_effects
            .contains(&(guarded_flow_site(0), guarded_flow_site(3)))
    );
    assert!(
        !flow
            .next_effects
            .contains(&(guarded_flow_site(1), guarded_flow_site(4)))
    );
}

#[test]
fn guarded_effect_flow_absent_or_mismatched_witness_does_not_make_a_load_optional() {
    let events = guarded_flow_events(&[0, 1, 2]);
    let successors = BTreeMap::from([(0, vec![])]);
    let expected_edges = guarded_flow_edges(&[(0, 1), (1, 2)]);
    let mut witnesses = Vec::new();
    witnesses.push(BTreeMap::new());
    // Exactness includes the physical block, operation index, source site,
    // access ordinal, and the event-map key. None is advisory evidence.
    for mutation in 0..5 {
        let mut witness = guarded_flow_witness(0, 1, 1);
        let mut key = (0, guarded_flow_key(1));
        match mutation {
            0 => witness.location = FunctionOperationLocation::new(BlockId(1), 1),
            1 => witness.location = FunctionOperationLocation::new(BlockId(0), 2),
            2 => witness.site = guarded_flow_site(9),
            3 => witness.operation_access_ordinal = 1,
            4 => key = (0, guarded_flow_key(2)),
            _ => unreachable!(),
        }
        witnesses.push(BTreeMap::from([(key, witness)]));
    }
    for conditional in witnesses {
        let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 1_000 };
        let flow = effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget)
            .expect("invalid witness grants no skip path");
        assert_eq!(flow.entry_effects, BTreeSet::from([guarded_flow_site(0)]));
        assert_eq!(flow.next_effects, expected_edges);
    }
}

#[test]
fn guarded_effect_flow_optional_entry_uses_only_its_real_successor() {
    let events = BTreeMap::from([
        (0, vec![(0, guarded_flow_site(0))]),
        (1, vec![(0, guarded_flow_site(1))]),
        (2, vec![(0, guarded_flow_site(2))]),
    ]);
    let successors = BTreeMap::from([(0, vec![1]), (1, vec![]), (2, vec![])]);
    let conditional = BTreeMap::from([((0, 0), guarded_flow_witness(0, 0, 0))]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 1_000 };
    let flow =
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).unwrap();
    assert_eq!(
        flow.entry_effects,
        BTreeSet::from([guarded_flow_site(0), guarded_flow_site(1)])
    );
    assert_eq!(flow.next_effects, guarded_flow_edges(&[(0, 1)]));
    assert!(!flow.entry_effects.contains(&guarded_flow_site(2)));
}

#[test]
fn guarded_effect_flow_optional_final_read_does_not_fabricate_a_successor() {
    let events = guarded_flow_events(&[0]);
    let successors = BTreeMap::from([(0, vec![])]);
    let conditional = BTreeMap::from([((0, 0), guarded_flow_witness(0, 0, 0))]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 100 };
    let flow =
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).unwrap();
    assert_eq!(flow.entry_effects, BTreeSet::from([guarded_flow_site(0)]));
    assert!(flow.next_effects.is_empty());
}

#[test]
fn guarded_effect_flow_cyclic_skip_paths_are_bounded_and_preserve_mandatory_events() {
    let events = guarded_flow_events(&[0, 1]);
    let successors = BTreeMap::from([(0, vec![0])]);
    let conditional = BTreeMap::from([((0, 0), guarded_flow_witness(0, 0, 0))]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 1_000 };
    let flow =
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).unwrap();
    assert_eq!(
        flow.entry_effects,
        BTreeSet::from([guarded_flow_site(0), guarded_flow_site(1)])
    );
    assert_eq!(
        flow.next_effects,
        guarded_flow_edges(&[(0, 1), (1, 0), (1, 1)])
    );
    let mut exhausted = UnsupportedIndexCorrelationBudgetV1 { remaining: 1 };
    assert!(
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut exhausted).is_none()
    );
}

#[test]
fn guarded_effect_flow_charges_optional_chains_and_rejects_missing_successor_nodes() {
    let sites = (0..32).collect::<Vec<_>>();
    let events = guarded_flow_events(&sites);
    let conditional = (0..32)
        .map(|operation| {
            (
                (0, guarded_flow_key(operation)),
                guarded_flow_witness(0, operation, operation),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let successors = BTreeMap::from([(0, vec![])]);
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 8 };
    assert!(effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).is_none());
    let mut budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 10_000 };
    let flow =
        effect_flow_signature_v1(0, &events, &successors, &conditional, &mut budget).unwrap();
    assert_eq!(flow.entry_effects.len(), 32);
    assert_eq!(flow.next_effects.len(), 32 * 31 / 2);
    assert!(budget.remaining < 10_000);
    let malformed_successors = BTreeMap::from([(0, vec![99])]);
    assert!(
        effect_flow_signature_v1(0, &events, &malformed_successors, &conditional, &mut budget,)
            .is_none()
    );
}

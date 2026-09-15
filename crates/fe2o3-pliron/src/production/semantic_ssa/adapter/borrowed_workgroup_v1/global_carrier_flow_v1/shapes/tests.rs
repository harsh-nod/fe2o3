use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

#[path = "../../global_statement_index_v1/fixture.rs"]
mod fixture;
use fixture::{callable, ty};

fn budget(limit: usize) -> Budget {
    Budget {
        remaining: limit,
        limit,
        profile: FlowWorkProfile::default(),
    }
}

fn facts(types: &[SemanticTypeDeclV1]) -> Vec<GlobalBf16BorrowV1> {
    [callable(0, 62), callable(15, 63), callable(0, 64)]
        .iter()
        .map(|callable| GlobalBf16BorrowV1::for_callable(types, callable).unwrap())
        .collect()
}

fn aggregate(index: u8, fields: &[u32]) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([index; 32]),
        SemanticLayoutIdentityV1::from_sha256([index; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(fields.len() as u64 * 8),
            8,
            SemanticAggregateLayoutV1::new(
                (0..fields.len()).map(|i| i as u64 * 8).collect(),
                vec![],
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(
            SemanticAggregateTypeV1::new(fields.iter().copied().map(ty).collect()).unwrap(),
        ),
    )
}

fn graph() -> Vec<SemanticTypeDeclV1> {
    let mut types = fixture::types();
    for (index, fields) in [
        (30, vec![7, 7, 2]),
        (31, vec![2, 3]),
        (32, vec![30, 31, 30]),
        (33, vec![8]),
        (34, vec![34, 7]),
        (35, vec![7, u32::MAX]),
        (36, vec![37]),
        (37, vec![36]),
        (38, vec![7, 34]),
        (39, vec![]),
    ] {
        types.push(aggregate(index, &fields));
    }
    types
}

// Independent recursive semantics: no production memo, front slots or pending
// stack. Every field is visited even after an earlier selected field.
fn oracle(
    types: &[SemanticTypeDeclV1],
    facts: &[GlobalBf16BorrowV1],
    ty: SemanticTypeIdV1,
    path: &mut BTreeSet<SemanticTypeIdV1>,
) -> Result<bool, ()> {
    let declaration = types.get(ty.index() as usize).ok_or(())?;
    if facts.iter().any(|fact| fact.pairs()[2].0 == ty) {
        return Ok(true);
    }
    if facts.iter().any(|fact| fact.pairs()[0].1 == ty) {
        return Ok(false);
    }
    let fields = match declaration.shape() {
        SemanticTypeShapeV1::Aggregate(a) | SemanticTypeShapeV1::Tuple(a) => a.fields(),
        _ => return Ok(false),
    };
    if !path.insert(ty) {
        return Err(());
    }
    let mut selected = false;
    for &field in fields {
        selected |= oracle(types, facts, field, path)?;
    }
    path.remove(&ty);
    Ok(selected)
}

fn tree_work(mut len: usize) -> usize {
    let mut work = 1;
    while len != 0 {
        work += 1;
        len /= 2;
    }
    work
}

fn storage_work() -> usize {
    // Sum independent fixed fields and alignment rather than copying the
    // production size_of::<Completed> debit into the expected cost.
    let word = std::mem::size_of::<usize>();
    let payload = 64 * std::mem::size_of::<Option<(SemanticTypeIdV1, bool)>>()
        + std::mem::size_of::<&[SemanticTypeDeclV1]>()
        + std::mem::size_of::<bool>();
    let bytes = payload.div_ceil(word) * word;
    assert_eq!(std::mem::size_of::<Completed<'_>>(), bytes);
    2 * bytes / word
}

fn slot_work() -> usize {
    std::mem::size_of::<Option<(SemanticTypeIdV1, bool)>>().div_ceil(std::mem::size_of::<usize>())
}

fn assert_work_failure(error: ProductionSemanticSsaErrorV1) {
    assert!(matches!(
        flow_work_profile_v1::original_error_for_test(error),
        ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        }
    ));
}

#[test]
fn completed_shapes_agree_with_original_and_independent_exhaustive_graph_oracles() {
    let types = graph();
    let facts = facts(&types);
    for index in (0..types.len() as u32).chain([u32::MAX]) {
        let expected = oracle(&types, &facts, ty(index), &mut BTreeSet::new());
        let mut hot_budget = budget(MAX_FLOW_WORK);
        let mut cold_budget = budget(MAX_FLOW_WORK);
        let mut hot = Shapes::new(&types, &facts, &mut hot_budget).unwrap();
        let mut cold = Shapes::new(&types, &facts, &mut cold_budget).unwrap();
        let observed = hot.contains(ty(index), &mut hot_budget).map_err(|_| ());
        let original = cold
            .contains_cold(ty(index), &mut cold_budget)
            .map_err(|_| ());
        assert_eq!(observed, expected, "type={index}");
        assert_eq!(observed, original, "type={index}");
        if expected.is_ok() {
            assert_eq!(
                hot.contains(ty(index), &mut hot_budget).map_err(|_| ()),
                expected,
                "warm type={index}"
            );
        }
    }
}

#[test]
fn constructor_charges_fixed_storage_initialization_and_original_duplicate_facts() {
    assert_eq!(MAX_FLOW_WORK, 262_144);
    assert_eq!(COMPLETED_SLOTS, 64);
    let types = fixture::types();
    let facts = facts(&types);
    let mut work = budget(MAX_FLOW_WORK);
    let shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    // Distinct pair/barrier cardinalities before insertion are 0, 1 and 2.
    let original = (6 + 1 + 1) + (6 + 2 + 2) + (6 + 3 + 3);
    assert_eq!(MAX_FLOW_WORK - work.remaining, storage_work() + original);
    assert_eq!(shapes.pairs.len(), 2);
    assert_eq!(shapes.barriers.len(), 2);
    assert_eq!(shapes.completed.slots, [None; 64]);
    assert!(std::ptr::eq(shapes.types, shapes.completed.types));
    assert!(!shapes.completed.failed);
    for limit in [0, storage_work() - 1, storage_work() + original - 1] {
        let mut work = budget(limit);
        match Shapes::new(&types, &facts, &mut work) {
            Ok(_) => panic!("uncharged constructor at {limit}"),
            Err(error) => assert_work_failure(error),
        }
    }
    let mut exact = budget(storage_work() + original);
    assert!(Shapes::new(&types, &facts, &mut exact).is_ok());
    assert_eq!(exact.remaining, 0);
}

#[test]
fn cold_fill_complete_boundary_and_each_short_budget_do_not_publish_or_resume() {
    let types = fixture::types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let pending_cells =
        std::mem::size_of::<(SemanticTypeIdV1, bool)>().div_ceil(std::mem::size_of::<usize>());
    // Original empty memo: initial lookup, stack allocation/write, pop lookup,
    // exact pair/barrier/type classification and final memo read.
    let original = 1 + 3 * pending_cells + 1 + (4 + 2 + 2 + 1) + 2;
    let expected = original + 1 + 2 * slot_work();
    for limit in 0..=expected {
        let mut setup = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
        let mut work = budget(limit);
        match shapes.contains(ty(7), &mut work) {
            Ok(value) => {
                assert_eq!(limit, expected);
                assert!(value);
                assert_eq!(work.remaining, 0);
                assert_eq!(shapes.completed.slots[7], Some((ty(7), true)));
            }
            Err(error) => {
                assert_work_failure(error);
                assert!(shapes.completed.failed);
                assert!(shapes.completed.slots.iter().all(Option::is_none));
                let mut fresh = budget(MAX_FLOW_WORK);
                assert!(matches!(
                    shapes.contains(ty(7), &mut fresh),
                    Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
                ));
                if limit == expected - 1 {
                    // The cold traversal completed, but its unpaid publication
                    // must not become a resumable front-cache capability.
                    assert_eq!(shapes.cache.get(&ty(7)), Some(&Some(true)));
                }
            }
        }
    }
}

#[test]
fn warm_read_exact_boundary_and_one_short_preserve_original_limit_errors() {
    let types = fixture::types();
    let facts = facts(&types);
    let expected = 1 + slot_work();
    for limit in 0..=expected {
        let mut setup = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut setup).unwrap();
        assert!(shapes.contains(ty(7), &mut setup).unwrap());
        let mut work = budget(limit);
        match shapes.contains(ty(7), &mut work) {
            Ok(value) => {
                assert!(value);
                assert_eq!(limit, expected);
                assert_eq!(work.remaining, 0);
            }
            Err(error) => {
                assert_work_failure(error);
                assert!(shapes.completed.failed);
                assert!(shapes.contains(ty(7), &mut budget(MAX_FLOW_WORK)).is_err());
            }
        }
    }
}

#[test]
fn repeated_local_type_work_has_independent_net_cost_including_retention() {
    let types = fixture::types();
    let facts = [GlobalBf16BorrowV1::for_callable(&types, &callable(0, 62)).unwrap()];
    let mut hot_setup = budget(MAX_FLOW_WORK);
    let mut cold_setup = budget(MAX_FLOW_WORK);
    let mut hot = Shapes::new(&types, &facts, &mut hot_setup).unwrap();
    let mut cold = Shapes::new(&types, &facts, &mut cold_setup).unwrap();
    let mut hot_work = budget(MAX_FLOW_WORK);
    let mut cold_work = budget(MAX_FLOW_WORK);
    for index in 0..30 {
        assert_eq!(
            hot.contains(ty(index), &mut hot_work).unwrap(),
            cold.contains_cold(ty(index), &mut cold_work).unwrap()
        );
    }
    assert_eq!(cold.cache.len(), 30);
    let before_hot = hot_work.remaining;
    let before_cold = cold_work.remaining;
    for _ in 0..4096 {
        assert!(!hot.contains(ty(2), &mut hot_work).unwrap());
        assert!(!cold.contains_cold(ty(2), &mut cold_work).unwrap());
    }
    let lookup = 1 + slot_work();
    assert_eq!(before_hot - hot_work.remaining, 4096 * lookup);
    assert_eq!(before_cold - cold_work.remaining, 4096 * tree_work(30));
    let original_total = 8 + (MAX_FLOW_WORK - cold_work.remaining);
    let retained_total = MAX_FLOW_WORK - hot_setup.remaining + MAX_FLOW_WORK - hot_work.remaining;
    let expected_saving =
        4096 * (tree_work(30) - lookup) - storage_work() - 30 * (lookup + slot_work());
    assert_eq!(original_total - retained_total, expected_saving);
    assert!(expected_saving > 0);
    assert_eq!(hot.completed.slots.len(), 64);
}

#[test]
fn collisions_keep_exact_keys_and_preserve_original_tree_memo() {
    let mut types = fixture::types();
    types.resize(72, types[2].clone());
    let facts = facts(&types);
    let mut work = budget(MAX_FLOW_WORK);
    let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    let mut original_work = budget(MAX_FLOW_WORK);
    let mut original = Shapes::new(&types, &facts, &mut original_work).unwrap();
    for (index, expected) in [(7, true), (71, false), (7, true), (71, false)] {
        let before = work.remaining;
        let original_before = original_work.remaining;
        assert_eq!(shapes.contains(ty(index), &mut work).unwrap(), expected);
        assert_eq!(
            original
                .contains_cold(ty(index), &mut original_work)
                .unwrap(),
            expected
        );
        assert_eq!(
            before - work.remaining,
            original_before - original_work.remaining + 1 + 2 * slot_work()
        );
        assert_eq!(shapes.completed.slots[7], Some((ty(index), expected)));
    }
    assert_eq!(shapes.cache.get(&ty(7)), Some(&Some(true)));
    assert_eq!(shapes.cache.get(&ty(71)), Some(&Some(false)));
    assert!(shapes.contains(ty(u32::MAX), &mut work).is_err());
    assert!(shapes.completed.failed);
}

#[test]
fn exact_type_owner_and_separate_fact_instances_cannot_reuse_foreign_verdicts() {
    let types = fixture::types();
    let foreign = types.clone();
    let facts = facts(&types);
    let mut work = budget(MAX_FLOW_WORK);
    let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
    assert!(shapes.contains(ty(7), &mut work).unwrap());
    shapes.types = &foreign;
    let before = work.remaining;
    assert!(matches!(
        shapes.contains(ty(7), &mut work),
        Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
    ));
    assert_eq!(before - work.remaining, 1);
    shapes.types = &types;
    assert!(shapes.contains(ty(7), &mut work).is_err());
    let mut other = Shapes::new(&types, &[], &mut work).unwrap();
    assert!(!other.contains(ty(7), &mut work).unwrap());
    let mut authenticated = Shapes::new(&foreign, &facts, &mut work).unwrap();
    assert!(authenticated.contains(ty(7), &mut work).unwrap());
}

#[test]
fn selected_sibling_does_not_hide_cycles_malformed_fields_or_matrix_barriers() {
    let types = graph();
    let facts = facts(&types);
    for index in [34, 35, 36, 37, 38] {
        let mut work = budget(MAX_FLOW_WORK);
        let mut shapes = Shapes::new(&types, &facts, &mut work).unwrap();
        assert!(shapes.contains(ty(7), &mut work).unwrap());
        assert!(!shapes.contains(ty(8), &mut work).unwrap());
        assert!(!shapes.contains(ty(33), &mut work).unwrap());
        assert!(matches!(
            shapes.contains(ty(index), &mut work),
            Err(ProductionSemanticSsaErrorV1::ReplayMismatch)
        ));
        assert!(shapes.completed.failed);
        assert_ne!(
            shapes.completed.slots[index as usize],
            Some((ty(index), true))
        );
        assert!(shapes.contains(ty(7), &mut work).is_err());
    }
}

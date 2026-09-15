use super::*;
use std::cell::RefCell;
use crate::production::{ProductionSemanticMirLimitsV1, ProductionSemanticSsaSourceQueryErrorV1,
    ProductionSemanticSsaSourceSiteV1, SemanticPartialMoveViolationV1};

#[path = "shared_carrier_fixture.rs"]
mod fixture;
use fixture::{CARRIER, CARRIER_LOCAL};

type Node = (SemanticTransparentBorrowSiteV1, u32, SemanticTypeIdV1, Option<u32>, bool,
    SemanticBorrowCandidateSourceV1, bool, u32, bool);

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    nodes: Vec<Node>,
    by_reference: BTreeMap<u32, usize>,
    direct: Vec<Vec<SemanticTransparentBorrowSiteV1>>,
    lanes: Vec<Vec<SemanticTransparentBorrowSiteV1>>,
    mutable: BTreeSet<usize>,
    shared: BTreeSet<usize>,
    phase: BTreeSet<usize>,
    grid: BTreeSet<usize>,
}

#[derive(Default, Debug)]
struct Trace {
    cold: bool,
    records: usize,
    queries: usize,
    first_calls: usize,
    second_calls: usize,
    removed_work: usize,
    snapshots: Vec<Snapshot>,
    work: Vec<usize>,
}

thread_local! {
    static TRACE: RefCell<Option<Trace>> = const { RefCell::new(None) };
}

struct Reset;
impl Drop for Reset {
    fn drop(&mut self) { TRACE.with(|slot| { slot.borrow_mut().take(); }); }
}

fn mode<T>(cold: bool, action: impl FnOnce() -> T) -> (T, Trace) {
    TRACE.with(|slot| {
        let mut slot = slot.borrow_mut();
        assert!(slot.is_none(), "no nested or reused observation owner");
        *slot = Some(Trace { cold, ..Trace::default() });
    });
    let _reset = Reset;
    let result = action();
    let trace = TRACE.with(|slot| slot.borrow_mut().take().unwrap());
    (result, trace)
}

pub(in super::super) fn record_is_cold() -> bool {
    TRACE.with(|slot| slot.borrow_mut().as_mut().is_some_and(|trace| {
        trace.records += 1;
        trace.cold
    }))
}

pub(in super::super) fn query_is_cold() -> bool {
    TRACE.with(|slot| slot.borrow_mut().as_mut().is_some_and(|trace| {
        trace.queries += 1;
        trace.cold
    }))
}

pub(in super::super) fn cold_query_work(work: usize) {
    TRACE.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() { trace.removed_work += work; }
    });
}

pub(in super::super) fn source_query(stage: FlowWorkStage) {
    TRACE.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() {
            match stage {
                FlowWorkStage::Candidates => trace.first_calls += 1,
                FlowWorkStage::Uses => trace.second_calls += 1,
                _ => {}
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn uses_finished(
    candidates: &[SemanticBorrowCandidateV1], by_reference: &BTreeMap<u32, usize>,
    direct: &[Vec<SemanticTransparentBorrowSiteV1>], lanes: &[Vec<SemanticTransparentBorrowSiteV1>],
    mutable: &BTreeSet<usize>, shared: &BTreeSet<usize>, phase: &BTreeSet<usize>, grid: &BTreeSet<usize>,
) {
    TRACE.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() {
            trace.snapshots.push(Snapshot {
                nodes: candidates.iter().map(|c| (c.site, c.source_local, c.source_type,
                    c.source_reference, c.value_alias, c.source_kind, c.valid, c.consumers,
                    c.intrinsic_consumer)).collect(),
                by_reference: by_reference.clone(), direct: direct.to_vec(), lanes: lanes.to_vec(),
                mutable: mutable.clone(), shared: shared.clone(), phase: phase.clone(), grid: grid.clone(),
            });
        }
    });
}

pub(in super::super) fn finished(work: usize) {
    TRACE.with(|slot| {
        if let Some(trace) = slot.borrow_mut().as_mut() { trace.work.push(work); }
    });
}

fn equivalent<T: std::fmt::Debug + PartialEq>((cold, a): &(T, Trace), (hot, b): &(T, Trace)) {
    assert_eq!(cold, hot, "cold exact result");
    assert_eq!(a.snapshots, b.snapshots, "all candidate/use/loan inputs");
    assert_eq!(a.first_calls, b.first_calls);
    assert_eq!(a.records, b.records);
    assert_eq!(a.queries, b.queries);
    assert_eq!(a.second_calls, a.queries, "cold executes the original source classifier");
    assert_eq!(b.second_calls, 0, "hot removes actual classifier invocations");
    assert_eq!(b.work.iter().sum::<usize>() + a.removed_work,
        a.work.iter().sum::<usize>() + b.records + b.queries,
        "only actual removed classification work minus retained read/write work");
}

fn expanded(source: &AdmittedInertSemanticMirV1) -> SemanticCallExpansionV1 {
    let expansion = SemanticCallExpansionV1::try_new(source, Default::default()).unwrap();
    expansion.verify_replay(source).unwrap();
    expansion
}

#[test]
fn retained_source_kind_uses_existing_candidate_padding_and_alias_semantics() {
    #[allow(dead_code)]
    struct OldCandidate {
        site: SemanticTransparentBorrowSiteV1,
        source_local: u32,
        source_type: SemanticTypeIdV1,
        source_reference: Option<u32>,
        value_alias: bool,
        valid: bool,
        consumers: u32,
        intrinsic_consumer: bool,
    }
    assert_eq!(std::mem::size_of::<SemanticBorrowCandidateV1>(), std::mem::size_of::<OldCandidate>());
    assert_eq!(std::mem::align_of::<SemanticBorrowCandidateV1>(), std::mem::align_of::<OldCandidate>());
    for alias in [false, true] {
        for source_kind in [SemanticBorrowCandidateSourceV1::Direct, SemanticBorrowCandidateSourceV1::TypedCarrier] {
            let candidate = SemanticBorrowCandidateV1 {
                site: SemanticTransparentBorrowSiteV1 { block: 0, statement: 0 },
                source_local: 0, source_type: ty(0), source_reference: None,
                value_alias: alias, source_kind, valid: false, consumers: 0, intrinsic_consumer: false,
            };
            assert_eq!(candidate.value_alias, alias);
            assert!(!candidate.valid, "classification never marks a candidate valid");
        }
    }
}

#[test]
fn same_assignment_reuse_removes_real_calls_and_work_with_exact_all_use_results() {
    let source = fixture::admitted();
    let expansion = expanded(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let run = || execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
    let cold = mode(true, run);
    let hot = mode(false, run);
    equivalent(&cold, &hot);
    assert!(!hot.0.is_empty());
    assert!(hot.1.queries > 0);
    assert!(hot.1.snapshots.iter().flat_map(|s| &s.nodes)
        .any(|n| n.5 == SemanticBorrowCandidateSourceV1::TypedCarrier));
    assert!(hot.1.work.iter().sum::<usize>() < cold.1.work.iter().sum::<usize>());
    eprintln!("candidate-source84 cold_calls={} hot_calls={} cold_work={} hot_work={} removed={} writes={} reads={}",
        cold.1.first_calls + cold.1.second_calls, hot.1.first_calls + hot.1.second_calls,
        cold.1.work.iter().sum::<usize>(), hot.1.work.iter().sum::<usize>(),
        cold.1.removed_work, hot.1.records, hot.1.queries);
}

#[test]
fn noncarrier_candidates_pay_reads_writes_without_reinterpreting_their_kind() {
    let body = direct_function(direct_statements(), false);
    let callables = [borrowed_callable(5, true)];
    let run = || sites(&body, &callables, &[], MAX_FLOW_WORK, None).unwrap();
    let cold = mode(true, run);
    let hot = mode(false, run);
    equivalent(&cold, &hot);
    assert!(hot.1.records > 0);
    assert!(!hot.0.is_empty(), "existing direct-reference positive");
    assert!(hot.1.snapshots.iter().flat_map(|s| &s.nodes)
        .all(|n| n.5 == SemanticBorrowCandidateSourceV1::Direct));
    // A pure direct-reference query can be free in the old classifier. Do not
    // conceal the new read/write cost or claim a gain for every input.
    assert!(hot.1.work.iter().sum::<usize>() >= cold.1.work.iter().sum::<usize>());
}

#[test]
fn no_candidate_pass_has_no_retained_source_state_or_extra_queries() {
    let source = epoch_source(true);
    let run = || sites(&source.functions()[0], &[], &[], MAX_FLOW_WORK, None).unwrap();
    let cold = mode(true, run);
    let hot = mode(false, run);
    equivalent(&cold, &hot);
    assert!(hot.0.is_empty());
    assert_eq!((hot.1.records, hot.1.queries, hot.1.first_calls), (0, 0, 0));
}

#[test]
fn exact_cumulative_budget_and_one_short_remain_fail_closed_in_both_modes() {
    let source = fixture::admitted();
    let expansion = expanded(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    for cold in [true, false] {
        let run = |limit| sites(view.body(), source.callables(), &[], limit, Some(source.types()));
        let (expected, trace) = mode(cold, || run(MAX_FLOW_WORK));
        let expected = expected.unwrap();
        assert_eq!(trace.work.len(), 1);
        let exact = trace.work[0];
        assert_eq!(mode(cold, || run(exact)).0.unwrap(), expected);
        let error = mode(cold, || run(exact - 1)).0.unwrap_err();
        assert!(matches!(flow_work_profile_v1::original_error_for_test(error),
            ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits, required, limit,
            } if required == exact && limit == exact - 1));
    }
}

fn readmit(source: &AdmittedInertSemanticMirV1, root: SemanticFunctionDeclV1) -> AdmittedInertSemanticMirV1 {
    let mut functions = source.functions().to_vec();
    functions[0] = root;
    InertSemanticMirRequestV1::new_with_callables(source.target(), source.types().to_vec(),
        vec![], vec![], vec![], functions, source.callables().to_vec(), source.roots().to_vec())
        .unwrap().admit_exact_v20(Default::default()).unwrap()
}

fn changed_first_block(source: &AdmittedInertSemanticMirV1, statements: Vec<SemanticStatementV1>) -> AdmittedInertSemanticMirV1 {
    let root = &source.functions()[0];
    let mut blocks = root.blocks().to_vec();
    let old = &blocks[0];
    blocks[0] = SemanticBasicBlockV1::new(old.identity(), old.source(), statements, old.terminator().clone()).unwrap();
    readmit(source, fixture::rebuild(root, root.locals().to_vec(), blocks))
}

#[test]
fn duplicate_and_missing_definitions_do_not_reuse_a_kind_at_another_site() {
    for duplicate in [false, true] {
        let source = fixture::admitted();
        let mut statements = source.functions()[0].blocks()[0].statements().to_vec();
        let index = statements.iter().position(|s| matches!(s.kind(), SemanticStatementKindV1::Assign(a)
            if a.destination().local().index() == CARRIER_LOCAL)).unwrap();
        if duplicate { statements.insert(index + 1, statements[index].clone()); }
        else { statements.remove(index); }
        let source = changed_first_block(&source, statements);
        let expansion = expanded(&source);
        let view = expansion.root(source.roots()[0]).unwrap();
        let run = || execution_sites(&source, &expansion, view, MAX_FLOW_WORK).unwrap();
        let cold = mode(true, run);
        let hot = mode(false, run);
        equivalent(&cold, &hot);
        for (site, _) in fixture::original_borrows(view.body()) { assert!(!hot.0.contains(&site)); }
    }
}

fn owner(source: AdmittedInertSemanticMirV1) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(source,
        ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default())
}

#[test]
fn retained_kind_preserves_live_source_query_original_place_and_replay() {
    let run = || {
        let source = fixture::admitted();
        let identity = *source.semantic_sha256().as_bytes();
        let root = source.roots()[0];
        let original = source.functions()[0].clone();
        let owner = owner(source).unwrap();
        owner.verify_replay().unwrap();
        assert_eq!(owner.source_semantic_sha256(), &identity);
        assert_eq!(owner.source_semantic().functions()[0], original);
        let view = owner.execution_view_for_root(root).unwrap();
        let query = owner.source_query_for_root(root, view.body()).unwrap();
        let borrows = fixture::original_borrows(view.body());
        assert_eq!(borrows.len(), 2);
        let mut result = Vec::new();
        for (site, place) in borrows {
            let site = ProductionSemanticSsaSourceSiteV1::new(SemanticBlockIdV1::from_index(site.block), Some(site.statement));
            result.push(query.borrow_place_use(site, place, &mut || true).unwrap());
            assert!(matches!(query.borrow_place_use(site, &place.clone(), &mut || true),
                Err(ProductionSemanticSsaSourceQueryErrorV1::OperandOutsideSite)));
            assert!(query.borrow_place_use(site, place, &mut || false).is_err());
        }
        assert_eq!(result[0], result[1]);
        result
    };
    let cold = mode(true, run);
    let hot = mode(false, run);
    equivalent(&cold, &hot);
}

#[test]
fn storage_death_and_move_still_fail_original_ssa_lifetime_checks() {
    for moved in [false, true] {
        let run = || {
            let source = fixture::admitted();
            let mut statements = source.functions()[0].blocks()[0].statements().to_vec();
            statements.insert(statements.len() - 1, if moved {
                assign(11, CARRIER, SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(CARRIER_LOCAL, CARRIER))))
            } else { statement(SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(CARRIER_LOCAL))) });
            owner(changed_first_block(&source, statements)).unwrap_err()
        };
        let cold = mode(true, run);
        let hot = mode(false, run);
        equivalent(&cold, &hot);
        let cause = match &hot.0 {
            ProductionSemanticSsaErrorV1::ExpandedExecution { error, .. } => error.as_ref(),
            error => error,
        };
        assert!(matches!(cause,
            ProductionSemanticSsaErrorV1::PartialMove { local: CARRIER_LOCAL,
                violation: SemanticPartialMoveViolationV1::MaybeMovedValueUsed, .. }
            | ProductionSemanticSsaErrorV1::Planner { error: SsaPlannerErrorV1::UndefinedAtUse { variable: _, .. }, .. }),
            "moved={moved}: {:?}", hot.0);
        if let ProductionSemanticSsaErrorV1::Planner { error: SsaPlannerErrorV1::UndefinedAtUse { variable, .. }, .. } = cause {
            assert_eq!(variable.get(), CARRIER_LOCAL);
        }
    }
}

#[path = "math_capture_flow_v1/policy_carrier_v1/fixture.rs"]
mod policy_fixture;

#[test]
fn retained_kind_still_audits_tracked_siblings_even_when_candidate_already_invalid() {
    let source = policy_fixture::source(policy_fixture::Mutation::None);
    let expansion = expanded(&source);
    let view = expansion.root(source.roots()[0]).unwrap();
    let facts = MatrixBorrowSitesV1::new(&source, &expansion, view,
        &expansion.defined_capability_bindings(&source).unwrap(), MAX_FLOW_WORK).unwrap();
    let mut setup = Budget { remaining: MAX_FLOW_WORK, limit: MAX_FLOW_WORK, profile: FlowWorkProfile::default() };
    let routes = math_capture_flow_v1::Routes::new_with_matrix(view.body(), Some(source.types()),
        source.callables(), &facts, &mut setup).unwrap();
    for moved in [false, true] {
        for tracked in [false, true] {
            let run = || {
                let sibling = if moved { SemanticOperandV1::Move(place(6, 6)) }
                    else { SemanticOperandV1::Copy(place(6, 6)) };
                // Exact typed classifier input, not a new source-body recipe or
                // proof of this synthetic statement's canonical source identity.
                let assignment = SemanticAssignmentV1::new(place(16, 14),
                    SemanticRvalueV1::new(ty(14), SemanticRvalueKindV1::aggregate(
                        SemanticAggregateKindV1::Aggregate,
                        vec![SemanticOperandV1::Copy(place(5, 5)), sibling]).unwrap()));
                let mut budget = Budget { remaining: MAX_FLOW_WORK, limit: MAX_FLOW_WORK,
                    profile: FlowWorkProfile::default() };
                budget.profile.stage = FlowWorkStage::Candidates;
                let original = routes.source(&assignment, &mut budget).unwrap().unwrap();
                let site = SemanticTransparentBorrowSiteV1 { block: 0, statement: 0 };
                let definition = SemanticBorrowCandidateV1 {
                    site, source_local: original.local().index(), source_type: ty(2),
                    source_reference: Some(original.local().index()), value_alias: true,
                    source_kind: candidate_source_v1::record(SemanticBorrowCandidateSourceV1::TypedCarrier, &mut budget).unwrap(),
                    valid: false, consumers: 0, intrinsic_consumer: false,
                };
                let mut siblings = vec![SemanticBorrowCandidateV1 {
                    site, source_local: 3, source_type: ty(3), source_reference: None,
                    value_alias: false, source_kind: SemanticBorrowCandidateSourceV1::Direct,
                    valid: true, consumers: 0, intrinsic_consumer: false,
                }];
                budget.profile.stage = FlowWorkStage::Uses;
                assert!(candidate_source_v1::is_carrier(&definition, &routes, &assignment, &mut budget).unwrap());
                let references = if tracked { BTreeMap::from([(6, 0)]) } else { BTreeMap::new() };
                routes.invalidate_siblings(&assignment, &references, &mut siblings, &mut budget).unwrap();
                (siblings[0].valid, siblings[0].consumers, definition.valid)
            };
            let cold = mode(true, run);
            let hot = mode(false, run);
            assert_eq!(cold.0, hot.0);
            assert_eq!(hot.0, (!tracked, 0, false));
            assert_eq!((cold.1.first_calls, cold.1.second_calls), (1, 1));
            assert_eq!((hot.1.first_calls, hot.1.second_calls), (1, 0));
        }
    }
}

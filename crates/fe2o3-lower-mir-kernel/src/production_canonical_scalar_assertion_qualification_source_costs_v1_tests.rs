// Source-reader arithmetic remains distinct from graph inventory and native
// pipeline units. Private layouts are named from this existing test scope;
// no production visibility is widened for the oracle.
use super::*;
use fe2o3_mir_model::semantic_assertion_v1::{ScalarAssignmentSiteV1, UnsignedRangeProofV1};

// ABI premise, not a Rust-language guarantee: exact variant/field/type mirrors
// of semantic_assertion_v1.rs on the pinned nightly x86_64 test target. These
// are never constructed as evidence, never passed to the source interpreter,
// and never sized by reading a successful interpreter receipt.
#[allow(dead_code)]
enum RangeOperandLayout {
    Constant {
        ty: SemanticTypeIdV1,
        bits: Option<u128>,
    },
    Local(usize),
    CheckedResult {
        local: usize,
    },
    ProjectedPlace(SemanticPlaceV1),
}
#[allow(dead_code)]
struct StrictUpperBoundLayout {
    local: usize,
    use_block: usize,
    next_switch_block: usize,
    range: Option<UnsignedRangeProofV1>,
    can_reach_use: Vec<bool>,
    stability_visited: Vec<usize>,
    stability_pending: std::collections::VecDeque<usize>,
    stability_generation: usize,
    proven_upper_bound: Option<u128>,
}
#[allow(dead_code)]
enum RangeFrameLayout {
    Operand {
        task: RangeOperandLayout,
        use_site: ScalarAssignmentSiteV1,
    },
    FinishBinary {
        operation: SemanticBinaryOpV1,
        destination_maximum: Option<u128>,
        left_source: SemanticOperandV1,
        right_source: SemanticOperandV1,
        use_site: ScalarAssignmentSiteV1,
    },
    FinishCheckedResult {
        local: usize,
        relational_range: Option<UnsignedRangeProofV1>,
    },
    FinishProjectedPlace {
        local: usize,
    },
    FinishLocalDefinition {
        local: usize,
        use_block: usize,
        maximum: u128,
    },
    ContinueStrictUpperBound(StrictUpperBoundLayout),
    ApplyStrictUpperBoundCandidate {
        state: StrictUpperBoundLayout,
        switch_block: usize,
        success_target: usize,
    },
}

fn exact_vector<T>(count: usize, trace: &mut Trace) -> usize {
    let bytes = count.checked_mul(size_of::<T>()).unwrap();
    trace.reserve(bytes);
    // Pinned alloc::raw_vec::grow_exact retains the requested non-ZST count.
    // The production API still performs this separate capacity reconciliation.
    trace.reserve(0);
    bytes
}

pub(super) fn source_analysis_construction(graph: Graph, trace: &mut Trace) -> usize {
    use fe2o3_mir_model::semantic_assertion_v1::{
        ScalarAssignmentSiteV1, SemanticAssertionAnalysisV1, SemanticAssertionCfgV1,
    };
    assert_ne!(graph, Graph::LiteralFinal);
    let dynamic = usize::from(graph == Graph::DynamicRetained);
    let locals = 1 + 2 * dynamic;
    let header = size_of::<SemanticAssertionAnalysisV1<'_>>();
    trace.reserve(header);
    trace.work(1);
    trace.work(4);
    let cfg_header = size_of::<SemanticAssertionCfgV1>() + 3 * size_of::<Vec<usize>>();
    trace.reserve(cfg_header);
    exact_vector::<Vec<usize>>(2, trace);
    exact_vector::<Vec<usize>>(2, trace);
    exact_vector::<usize>(2, trace);
    trace.work(2);
    for edges in [1, 0] {
        trace.work(2);
        for _ in 0..edges {
            trace.work(2);
        }
        exact_vector::<usize>(edges, trace);
        for _ in 0..edges {
            trace.work(2);
        }
        trace.work(2 * edges);
        for _ in 0..edges {
            trace.work(2);
        }
    }
    for indegree in [0, 1] {
        trace.work(1);
        exact_vector::<usize>(indegree, trace);
    }
    for edges in [1, 0] {
        trace.work(1);
        for _ in 0..edges {
            trace.work(2);
        }
    }
    exact_vector::<bool>(2, trace);
    exact_vector::<usize>(2, trace);
    for amount in [3, 2, 3, 2] {
        trace.work(amount);
    }
    // Definition inventory: the entry comparison writes one temporary only
    // in the dynamic source. The literal fixture has no assignment at all.
    trace.work(locals);
    exact_vector::<u8>(locals, trace);
    trace.work(locals);
    exact_vector::<Option<ScalarAssignmentSiteV1>>(locals, trace);
    trace.work(locals);
    exact_vector::<bool>(locals, trace);
    trace.work(0);
    exact_vector::<Vec<usize>>(2, trace);
    for statements in [dynamic, 0] {
        trace.work(3);
        trace.work(0);
        exact_vector::<usize>(2 * statements + 1, trace);
        for _ in 0..statements {
            trace.work(4);
        }
        trace.work(2 * statements);
        trace.work(statements);
    }
    trace.work(locals);
    trace.work(locals);
    exact_vector::<Vec<usize>>(locals, trace);
    trace.work(2);
    trace.work(2);
    header
        + cfg_header
        + 4 * size_of::<Vec<usize>>()
        + 6 * size_of::<usize>()
        + 2 * size_of::<bool>()
        + locals
            * (size_of::<u8>()
                + size_of::<Option<ScalarAssignmentSiteV1>>()
                + size_of::<bool>()
                + size_of::<Vec<usize>>())
        + 2 * size_of::<Vec<usize>>()
        + (2 * dynamic + 2) * size_of::<usize>()
}

pub(super) fn literal_source_query(trace: &mut Trace) -> usize {
    trace.point(Point::SourceProof);
    trace.work(1);
    trace.mark("source assertion query entered");
    trace.work(0);
    let frames = exact_vector::<RangeFrameLayout>(1, trace);
    trace.work(1);
    trace.mark("source literal range visited");
    trace.work(0);
    let values = exact_vector::<Option<UnsignedRangeProofV1>>(1, trace);
    trace.mark("source fact proved");
    frames + values
}

pub(super) fn source_call_index(graph: Graph, trace: &mut Trace) -> usize {
    assert_ne!(graph, Graph::LiteralFinal);
    let dynamic = usize::from(graph == Graph::DynamicRetained);
    let definitions = graph.counts().definitions;
    // One source association, one Return anchor, two source terminator spans,
    // and (only for dynamic) the genuine scalar entry-argument binding.
    trace.work(6 + 1 + 2 + dynamic);
    let retained = size_of::<ProductionCanonicalCallsV1<'_>>()
        + size_of::<CanonicalCallGroupV1<'_>>()
        + size_of::<CanonicalCallBindingV1<'_>>();
    trace.reserve(retained);
    let targets = trace.enter(&[]);
    trace.work(2 + 196);
    trace.reserve(
        2 * size_of::<&Function>() + size_of::<(usize, &SemanticKirFunctionCorrespondenceV1)>(),
    );
    // assert_origin_sort adds a charged root-comparison step beyond the
    // inventory's independent sort. Do not reuse the latter's 17-work sum.
    for amount in [1, 1, 14, 1, 1, 14] {
        trace.work(amount);
    }
    for amount in [1, 14, 72, 1, 14] {
        trace.work(amount);
    }
    let validation = trace.enter(&[]);
    trace.work(1 + 2);
    trace.work(1 + 2 * dynamic);
    trace.work(3);
    for operations in [1 + dynamic, 0, 1] {
        trace.work(operations);
    }
    trace.work((definitions + 3) * 100);
    trace.work(3);
    exact_vector::<(ValueId, &Type)>(definitions, trace);
    trace.work(3);
    for operations in [1 + dynamic, 0, 1] {
        trace.work(operations);
    }
    trace.work(3);
    exact_vector::<&BasicBlock>(3, trace);
    for operations in [1 + dynamic, 0] {
        trace.work(40);
        trace.work(operations);
    }
    trace.work(0); // Zero returned components are visited by the actual reader.
    trace.leave(validation);
    trace.work(1); // Actual canonical trap call occurrence.
    for amount in [1, 2 * (44 + 1), (44 + 2) * 8] {
        trace.work(amount);
    }
    trace.work(1); // Actual source Return anchor, not an empty call roster.
    trace.work(0);
    trace.leave(targets);
    trace.work(3);
    retained
}

#[test]
fn qualification_source_call_index_has_nonempty_return_and_reserved_call_debits() {
    for (graph, expected) in [
        (Graph::LiteralOriginal, 1_306),
        (Graph::DynamicRetained, 1_512),
    ] {
        let mut trace = Trace::new(29);
        let outer = trace.enter(&[]);
        let storage = source_call_index(graph, &mut trace);
        trace.mark("actual call index complete");
        trace.leave(outer);
        let predicted = trace.success();
        assert_eq!(predicted.work, expected);
        assert_eq!(predicted.storage, 29);
        assert!(predicted.peak > 29 + storage);
    }
}

#[test]
fn qualification_live_source_analysis_construction_census_is_independent() {
    for (graph, expected) in [(Graph::LiteralOriginal, 50), (Graph::DynamicRetained, 67)] {
        let mut trace = Trace::new(29);
        let scope = trace.enter(&[]);
        let bytes = source_analysis_construction(graph, &mut trace);
        trace.leave(scope);
        let predicted = trace.success();
        assert_eq!(predicted.work, expected);
        assert_eq!((predicted.storage, predicted.peak), (29, 29 + bytes));
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceAttempt {
    Work(usize),
    Storage(usize),
}

// This records observations only. Expected attempts and cut limits are made
// above from source arithmetic before this adapter ever invokes the engine.
// Its Vec is test instrumentation, not part of the source budget's payload.
struct SourceMeter<'a, 'w> {
    budget: &'a mut ArgumentBudgetV1<'w>,
    attempts: Vec<SourceAttempt>,
}
impl fe2o3_mir_model::SemanticAssertionMeterV1 for SourceMeter<'_, '_> {
    type Error = ArgumentResourceV1;
    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.attempts.push(SourceAttempt::Work(amount));
        self.budget.charge_work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Self::Error> {
        self.attempts.push(SourceAttempt::Storage(amount));
        self.budget.reserve_storage(amount)
    }
}

fn source_attempts(trace: &Trace, work_limit: usize, storage_limit: usize) -> Vec<SourceAttempt> {
    let mut work = 0usize;
    let mut storage = trace.floor;
    let mut result = Vec::new();
    for event in &trace.events {
        let denied = match event.debit {
            Debit::Work {
                amount,
                guard: None,
            } => {
                result.push(SourceAttempt::Work(amount));
                let attempted = work.checked_add(amount).unwrap();
                if attempted > work_limit {
                    true
                } else {
                    work = attempted;
                    false
                }
            }
            Debit::Reserve(amount) => {
                result.push(SourceAttempt::Storage(amount));
                let attempted = storage.checked_add(amount).unwrap();
                if attempted > storage_limit {
                    true
                } else {
                    storage = attempted;
                    false
                }
            }
            Debit::Mark(_) => false,
            _ => panic!("direct source transcript has no wrapper, refund or guard"),
        };
        if denied {
            break;
        }
    }
    result
}

fn check_source_attempt(
    owner: &ProductionPreRankedKirOwnerV1,
    graph: Graph,
    query: bool,
    work_limit: usize,
    storage_limit: usize,
) {
    use fe2o3_mir_model::{
        SemanticAssertionAnalysisV1, SemanticAssertionLimitsV1, SemanticAssertionMeteredErrorV1,
        SemanticAssertionOutcomeV1, SemanticAssertionProofKindV1,
    };
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + 29;
    let mut trace = Trace::new(floor);
    source_analysis_construction(graph, &mut trace);
    if query {
        literal_source_query(&mut trace);
    }
    let predicted = trace.run(work_limit, storage_limit);
    let expected_attempts = source_attempts(&trace, work_limit, storage_limit);
    let semantic = owner.semantic_ssa().source_semantic();
    assert_eq!(semantic.functions().len(), 1);
    let function = &semantic.functions()[0];
    assert_eq!(function.blocks().len(), 2);
    assert_eq!(
        function.locals().len(),
        1 + 2 * usize::from(graph == Graph::DynamicRetained)
    );
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
    {
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let mut meter = SourceMeter {
            budget: &mut budget,
            attempts: Vec::new(),
        };
        let actual = SemanticAssertionAnalysisV1::new_metered(
            semantic.types(),
            function,
            SemanticAssertionLimitsV1::new(usize::MAX, usize::MAX),
            &mut meter,
        );
        let result = match actual {
            Ok(mut analysis) => {
                let result = if query {
                    analysis
                        .assertion_at_v1(SemanticBlockIdV1::from_index(0), &mut meter)
                        .map(|outcome| {
                            let SemanticAssertionOutcomeV1::Proved(fact) = outcome else {
                                panic!("literal source must produce a borrowed fact")
                            };
                            assert_eq!(fact.proof_kind(), SemanticAssertionProofKindV1::ExactRange);
                        })
                } else {
                    Ok(())
                };
                drop(analysis);
                result
            }
            Err(error) => Err(error),
        };
        match (predicted.denied_at, result) {
            (None, Ok(())) => {}
            (
                Some((_, Denial::Work { .. })),
                Err(SemanticAssertionMeteredErrorV1::Meter(ArgumentResourceV1::Work(_))),
            ) => {}
            (
                Some((_, Denial::Storage { .. })),
                Err(SemanticAssertionMeteredErrorV1::Meter(ArgumentResourceV1::Storage(_))),
            ) => {}
            (_, result) => panic!("source cost prediction differs: {predicted:?}, {result:?}"),
        }
        assert_eq!(meter.attempts, expected_attempts);
        drop(meter);
        assert_eq!(budget.work(), predicted.work);
        assert_eq!(budget.storage(), predicted.storage);
        assert_eq!(budget.peak_storage(), predicted.peak);
        assert_eq!(budget.failed_storage(), predicted.first_storage);
        // Every analysis, fact and failed constructor has been destroyed first.
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(work.failed_work(), predicted.first_work);
}

pub(super) fn qualify_source_components(owner: &ProductionPreRankedKirOwnerV1, dynamic: bool) {
    let graph = if dynamic {
        Graph::DynamicRetained
    } else {
        Graph::LiteralOriginal
    };
    let floor = owner.unit_local_source_storage_floor_v1().unwrap() + 29;
    let mut construction = Trace::new(floor);
    let retained = source_analysis_construction(graph, &mut construction);
    let work = if dynamic { 67 } else { 50 };
    check_source_attempt(owner, graph, false, work, floor + retained);
    check_source_attempt(owner, graph, false, work - 1, usize::MAX);
    check_source_attempt(owner, graph, false, usize::MAX, floor + retained - 1);
    if !dynamic {
        let mut query = construction.clone();
        let scratch = literal_source_query(&mut query);
        // Work 51 denies the evaluator visit after the frame has been prepaid;
        // work 52 proves the predicate. Neither cut comes from a measured run.
        check_source_attempt(owner, graph, true, 51, usize::MAX);
        check_source_attempt(owner, graph, true, 52, floor + retained + scratch);
        check_source_attempt(owner, graph, true, 52, floor + retained + scratch - 1);
        // A separate record-peak cut isolates the private frame allocation.
        check_source_attempt(
            owner,
            graph,
            true,
            52,
            floor + retained + size_of::<RangeFrameLayout>() - 1,
        );
    }
}

pub(super) fn qualify_call_index(owner: &ProductionPreRankedKirOwnerV1, dynamic: bool) {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let graph = if dynamic {
        Graph::DynamicRetained
    } else {
        Graph::LiteralOriginal
    };
    let owner_floor = owner.unit_local_source_storage_floor_v1().unwrap() + 29;
    let mut setup_work = Work::new(usize::MAX);
    let mut setup = ArgumentBudgetV1::new(&mut setup_work, usize::MAX);
    setup.reserve_storage(owner_floor).unwrap();
    let (inventory, receipt) =
        CanonicalKirInventoryV1::derive(owner.executable(), &mut setup).unwrap();
    assert_eq!(receipt.retained_storage(), graph.inventory_storage());
    setup.reserve_storage(graph.inventory_storage()).unwrap();
    let floor = owner_floor + graph.inventory_storage();
    let mut expected = Trace::new(floor);
    source_call_index(graph, &mut expected);
    let expected = expected.success();
    // Direct reader observation, not a successful public-entry preflight.
    // The inventory and owner are prepaid in this fresh reader's ledger.
    let mut work = Work::new(expected.work);
    {
        let mut budget = ArgumentBudgetV1::new(&mut work, expected.peak);
        budget.reserve_storage(floor).unwrap();
        let calls = ProductionCanonicalCallsV1::build(owner, &inventory, &mut budget).unwrap();
        assert_eq!(calls.groups.len(), 1);
        assert_eq!(
            calls.groups[0].calls.len(),
            1,
            "actual source Return anchor"
        );
        assert!(
            calls.calls.is_empty(),
            "the reserved Trap is not an ordinary source call"
        );
        assert_eq!(inventory.calls().len(), 1);
        assert_eq!(budget.work(), expected.work);
        assert_eq!(budget.storage(), expected.storage);
        assert_eq!(budget.peak_storage(), expected.peak);
        assert_eq!(budget.failed_storage(), None);
        drop(calls);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    }
    assert_eq!(work.failed_work(), None);
    drop(inventory);
    setup.release_storage(graph.inventory_storage()).unwrap();
    assert_eq!(setup.storage(), owner_floor);
}

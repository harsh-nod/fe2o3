//! Inert source/destination controls only; not genuine canonical observations.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::SemanticBorrowKindV1;
#[path = "whole_root_inert_fixtures_v1_tests.rs"]
mod fixture;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 19;
struct Model {
    function: SemanticFunctionDeclV1,
    types: Vec<SemanticTypeDeclV1>,
    counts: Vec<u8>,
    options: SemanticOptionDominanceV1,
    enumeration: SemanticEnumPayloadDominanceV1,
}
impl Model {
    fn source<'a>(&'a self, resources: &Prep<'_, '_>) -> Source<'a> {
        Source {
            function: &self.function,
            types: &self.types,
            callables: &[],
            ledger: resources.original_ledger_v1().unwrap(),
        }
    }
}
fn model(with_edges: bool) -> Model {
    let statements = if with_edges {
        vec![
            fixture::assign(
                1,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(fixture::place(0))),
            ),
            fixture::assign(
                2,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: fixture::place(1),
                },
            ),
            fixture::assign(
                3,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(fixture::place(2))),
            ),
        ]
    } else {
        Vec::new()
    };
    let function = fixture::fixture(statements, 4, None);
    let types = fixture::projection_types();
    let counts = assertion_definition_inventory(&function).unwrap().counts;
    let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
    let enumeration = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    Model {
        function,
        types,
        counts,
        options,
        enumeration,
    }
}
struct State {
    initial: InitialDestinations,
    arguments: RetainedBeforeArgumentWritersV1,
    prefix: RootEntryPrefixV1,
    stage: RetainedInitialCapabilityGraphV1,
}
impl State {
    fn new(resources: &mut Prep<'_, '_>) -> Self {
        let mut initial = InitialDestinations::new();
        initial.prepare(4, resources).unwrap();
        let mut arguments = RetainedBeforeArgumentWritersV1::new();
        arguments.prepare_into(4, resources).unwrap();
        Self {
            initial,
            arguments,
            prefix: RootEntryPrefixV1::empty(),
            stage: RetainedInitialCapabilityGraphV1::new(),
        }
    }
    fn prepare(
        &mut self,
        model: &Model,
        launch: Option<u64>,
        resources: &mut Prep<'_, '_>,
    ) -> BResult<()> {
        let source = model.source(resources);
        self.stage.prepare_into(
            &source,
            &model.counts,
            &model.options,
            &model.enumeration,
            launch,
            &mut self.initial,
            &mut self.arguments,
            &self.prefix,
            resources,
        )
    }
    fn completed(
        &self,
        model: &Model,
        launch: Option<u64>,
        resources: &Prep<'_, '_>,
    ) -> BResult<()> {
        let source = model.source(resources);
        self.stage.completed_for(
            &source,
            &model.counts,
            &model.options,
            &model.enumeration,
            launch,
            &self.initial,
            &self.arguments,
            &self.prefix,
            resources,
        )
    }
}
fn initial_bytes() -> usize {
    4 * (size_of::<Option<ProjectedDisjointIndexV1>>()
        + size_of::<Option<ProjectedGridLeaderV1>>()
        + size_of::<Option<GuardPredicateV1>>()
        + size_of::<Vec<CapabilityEdgeV1>>()
        + 2 * size_of::<Option<u32>>())
}
fn accounting_error(result: BResult<()>) {
    assert!(matches!(
        result,
        Err(Backend::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Accounting)
        ))
    ));
}
fn same_error(left: &Backend, right: &Backend) -> bool {
    match (left, right) {
        (
            Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(a)),
            Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(b)),
        ) => a == b,
        (Backend::Unsupported(a), Backend::Unsupported(b)) => a == b,
        (Backend::Incomplete(a), Backend::Incomplete(b)) => a == b,
        _ => false,
    }
}
#[test]
fn retained_initial_graph_matches_original_alias_borrow_order() {
    for with_edges in [false, true] {
        let model = model(with_edges);
        let mut expected = InitialDestinations::new();
        expected.prepare(4, &mut Prep::unmetered()).unwrap();
        let mut count = 0;
        let mut borrowed = Vec::new();
        root_initial_capability_graph_v1::populate_initial_graph_v1(
            &[],
            &model.function,
            None,
            &model.counts,
            &model.options,
            &model.enumeration,
            &mut expected.edges,
            &mut count,
            &mut expected.stores,
            &mut expected.loads,
            &mut borrowed,
            &mut Prep::unmetered(),
        )
        .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
        state
            .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        state
            .completed(&model, None, &Prep::new(&mut budget, &mut owned))
            .unwrap();
        assert_eq!(state.stage.local_definitions, model.counts);
        assert_eq!(state.stage.launch_extent, Some(0));
        assert_eq!(state.initial.edges, expected.edges);
        assert_eq!(state.initial.stores, expected.stores);
        assert_eq!(state.initial.loads, expected.loads);
        assert_eq!(state.arguments.edge_count, count);
        assert_eq!(state.arguments.borrowed_locals, borrowed);
        assert_eq!(count, if with_edges { 3 } else { 0 });
        if with_edges {
            assert_eq!(state.arguments.borrowed_locals, vec![(1, 0)]);
            for source in 0..3 {
                assert_eq!(state.initial.edges[source][0].destination, source + 1);
            }
        }
        assert!(state.initial.indices.iter().all(Option::is_none));
        assert!(state.initial.leaders.iter().all(Option::is_none));
        assert!(state.initial.predicates.iter().all(Option::is_none));
        assert!(state.prefix.entry_operations.is_empty());
        assert_eq!(state.prefix.next_value, 0);
        assert_eq!(budget.storage(), owned);
        drop(state);
        budget.release_storage(owned).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn retained_initial_graph_preserves_prior_writer_values_without_reinitializing() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    // Inert representation of a nonempty prior writer, not genuine read coverage.
    state.arguments.runtime_index_arguments[1] = Some(9);
    state.arguments.next_runtime_argument = 10;
    state.prefix.next_value = 7;
    state
        .prepare(&model, Some(64), &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    state
        .completed(&model, Some(64), &Prep::new(&mut budget, &mut owned))
        .unwrap();
    assert_eq!(
        state.arguments.runtime_index_arguments,
        vec![None, Some(9), None, None]
    );
    assert_eq!(state.arguments.next_runtime_argument, 10);
    assert_eq!(state.prefix.next_value, 7);
    assert_eq!(state.stage.launch_extent, Some(0));
    drop(state);
    budget.release_storage(owned).unwrap();
}
fn trial(work_extra: usize, storage_extra: usize) -> (bool, usize, usize, bool, bool) {
    let model = model(true);
    let mut work = Work::new(32 + work_extra);
    let mut budget = Budget::new(&mut work, FLOOR + initial_bytes() + storage_extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    assert_eq!(budget.work(), 32);
    assert_eq!(owned, initial_bytes());
    let result = state.prepare(&model, None, &mut Prep::new(&mut budget, &mut owned));
    let observation = (
        result.is_ok(),
        budget.work() - 32,
        owned - initial_bytes(),
        !state.stage.local_definitions.is_empty(),
        state.initial.edges.iter().any(|row| !row.is_empty()),
    );
    assert_eq!(budget.storage(), FLOOR + owned);
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        state.arguments.edge_count,
        state.arguments.borrowed_locals.len(),
        state.stage.local_definitions.capacity(),
    );
    let repeated = state.prepare(&model, None, &mut Prep::new(&mut budget, &mut owned));
    assert!(repeated.is_err());
    assert_eq!(state.stage.phase, GraphPhase::Terminal);
    if let Err(error) = &result {
        assert!(same_error(error, state.stage.failure.as_ref().unwrap()));
        assert!(same_error(error, repeated.as_ref().unwrap_err()));
    }
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            state.arguments.edge_count,
            state.arguments.borrowed_locals.len(),
            state.stage.local_definitions.capacity()
        )
    );
    assert!(
        state
            .completed(&model, None, &Prep::new(&mut budget, &mut owned))
            .is_err()
    );
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    observation
}
#[test]
fn retained_initial_graph_every_work_cut_keeps_partial_destinations_and_first_error() {
    let good = trial(LIMIT, LIMIT);
    assert!(good.0);
    let mut copied = false;
    let mut graph = false;
    for cut in 0..good.1 {
        let denied = trial(cut, LIMIT);
        assert!(!denied.0);
        copied |= denied.3;
        graph |= denied.4;
    }
    assert!(copied && graph);
    assert!(trial(good.1, LIMIT).0);
}
#[test]
fn retained_initial_graph_every_storage_cut_keeps_partial_destinations_and_first_error() {
    let good = trial(LIMIT, LIMIT);
    let mut copied = false;
    let mut graph = false;
    for cut in 0..good.2 {
        let denied = trial(LIMIT, cut);
        assert!(!denied.0);
        copied |= denied.3;
        graph |= denied.4;
    }
    assert!(copied && graph);
    assert!(trial(LIMIT, good.2).0);
}
#[test]
fn retained_initial_graph_equal_counts_copy_is_not_original_source_identity() {
    let mut model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state
        .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let copied_counts = model.counts.clone();
    let old = std::mem::replace(&mut model.counts, copied_counts);
    let before = (budget.work(), budget.storage(), owned);
    accounting_error(state.completed(&model, None, &Prep::new(&mut budget, &mut owned)));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    model.counts = old;
    state
        .completed(&model, None, &Prep::new(&mut budget, &mut owned))
        .unwrap();
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_foreign_source_and_launch_do_not_spend() {
    let model = model(true);
    let foreign = self::model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state
        .prepare(&model, Some(64), &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let before = (budget.work(), budget.storage(), owned);
    accounting_error(state.completed(&foreign, Some(64), &Prep::new(&mut budget, &mut owned)));
    accounting_error(state.completed(&model, None, &Prep::new(&mut budget, &mut owned)));
    accounting_error(state.completed(&model, Some(65), &Prep::new(&mut budget, &mut owned)));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_foreign_equal_counter_does_not_spend() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state
        .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let before = (budget.work(), budget.storage(), owned);
    let mut foreign = owned;
    accounting_error(state.completed(&model, None, &Prep::new(&mut budget, &mut foreign)));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    assert_eq!(foreign, owned);
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_foreign_budget_does_not_spend() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state
        .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    other.reserve_storage(owned).unwrap();
    let before = (other.work(), other.storage(), owned);
    accounting_error(state.completed(&model, None, &Prep::new(&mut other, &mut owned)));
    assert_eq!(before, (other.work(), other.storage(), owned));
    other.release_storage(owned).unwrap();
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_runtime_cursor_change_refuses_completion() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state
        .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    state.arguments.next_runtime_argument += 1;
    accounting_error(state.completed(&model, None, &Prep::new(&mut budget, &mut owned)));
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_replaced_destination_refuses_completion() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state
        .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let other_initial = InitialDestinations::new();
    let resources = Prep::new(&mut budget, &mut owned);
    let source = model.source(&resources);
    accounting_error(state.stage.completed_for(
        &source,
        &model.counts,
        &model.options,
        &model.enumeration,
        None,
        &other_initial,
        &state.arguments,
        &state.prefix,
        &resources,
    ));
    drop(resources);
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_nonpristine_destination_and_count_shape_are_terminal() {
    for bad_counts in [false, true] {
        let mut model = model(true);
        if bad_counts {
            model.counts.pop();
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
        if !bad_counts {
            state.arguments.edge_count = 1;
        }
        accounting_error(state.prepare(&model, None, &mut Prep::new(&mut budget, &mut owned)));
        assert_eq!(state.stage.phase, GraphPhase::Terminal);
        assert!(state.stage.local_definitions.is_empty());
        assert!(state.initial.edges.iter().all(Vec::is_empty));
        drop(state);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_initial_graph_unmetered_component_refuses_before_allocating() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    let source = model.source(&Prep::new(&mut budget, &mut owned));
    let before = (budget.work(), budget.storage(), owned);
    accounting_error(state.stage.prepare_into(
        &source,
        &model.counts,
        &model.options,
        &model.enumeration,
        None,
        &mut state.initial,
        &mut state.arguments,
        &state.prefix,
        &mut Prep::unmetered(),
    ));
    assert!(state.stage.local_definitions.is_empty());
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_prior_denial_cannot_publish_completion() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    assert!(budget.charge_work(LIMIT).is_err());
    accounting_error(state.prepare(&model, None, &mut Prep::new(&mut budget, &mut owned)));
    assert_eq!(state.stage.phase, GraphPhase::Terminal);
    assert!(state.stage.local_definitions.is_empty());
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_initial_graph_callback_unwind_keeps_outer_payload_until_drop() {
    let model = model(true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state
            .prepare(&model, None, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        std::panic::panic_any(());
    }));
    assert!(caught.is_err());
    assert_eq!(state.arguments.edge_count, 3);
    assert_eq!(state.stage.local_definitions, model.counts);
    assert_eq!(budget.storage(), owned);
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_initial_graph_whole_transition_revokes_every_old_phase() {
    for phase in [
        WholePhase::Fresh,
        WholePhase::Terminal,
        WholePhase::BeforeArgumentWriters,
        WholePhase::InitialStridedReadsTerminal,
        WholePhase::AfterInitialStridedReads,
        WholePhase::InitialCapabilityGraphTerminal,
        WholePhase::AfterInitialCapabilityGraph,
    ] {
        let mut value = phase;
        assert_eq!(
            whole::begin_initial_graph_transition(&mut value),
            phase == WholePhase::AfterInitialStridedReads
        );
        assert_eq!(value, WholePhase::InitialCapabilityGraphTerminal);
        assert!(!whole::begin_initial_graph_transition(&mut value));
        assert_eq!(value, WholePhase::InitialCapabilityGraphTerminal);
    }
}

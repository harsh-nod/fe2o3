//! Inert model DATA only. No genuine source, ordinary route, or native execution.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
#[path = "whole_root_inert_fixtures_v1_tests.rs"]
mod fixture;
const LIMIT: usize = 1_000_000;
const FLOOR: usize = 19;
const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
struct Model {
    function: SemanticFunctionDeclV1,
    types: Vec<SemanticTypeDeclV1>,
    calls: Vec<SemanticCallableDeclV1>,
    counts: Vec<u8>,
    escaped: Vec<bool>,
    options: SemanticOptionDominanceV1,
    enumeration: SemanticEnumPayloadDominanceV1,
}
fn callable(operation: SemanticCompilerIntrinsicOperationV1) -> SemanticCallableDeclV1 {
    SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256(fixture::bytes(116)),
            SemanticItemDefinitionIdentityV1::from_sha256(fixture::bytes(117)),
            SemanticMonomorphizationIdentityV1::from_sha256(fixture::bytes(118)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(fixture::bytes(119)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(fixture::bytes(120)),
            SemanticSourceProvenanceV1::unavailable(),
            fixture::fixture(Vec::new(), 4, None).abi().clone(),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(fixture::bytes(121)),
    }
}
fn model(destinations: &[u32], grid: bool) -> Model {
    let mut blocks = Vec::new();
    for (ordinal, destination) in destinations.iter().copied().enumerate() {
        blocks.push(fixture::block(
            50 + ordinal as u8,
            Vec::new(),
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(0),
                    Vec::new(),
                    Some(SemanticCallDestinationV1::new(
                        fixture::place(destination),
                        fixture::cfg_edge(SemanticEdgeRoleV1::CallReturn, ordinal as u32 + 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ));
    }
    // A real inert graph edge must remain unpropagated after seed-only completion.
    blocks.push(fixture::block(
        50 + destinations.len() as u8,
        vec![fixture::assign(
            3,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(fixture::place(1))),
        )],
        SemanticTerminatorKindV1::Return,
    ));
    let function = fixture::projection_function_with_locals(
        blocks,
        (0..4)
            .map(|index| {
                fixture::local(
                    100 + index,
                    SCALAR,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                )
            })
            .collect(),
    );
    let inventory = assertion_definition_inventory(&function).unwrap();
    let types = fixture::projection_types();
    let options = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
    let enumeration = SemanticEnumPayloadDominanceV1::analyze(&function, &types).unwrap();
    Model {
        function,
        types,
        counts: inventory.counts,
        escaped: inventory.address_escaped,
        calls: vec![callable(if grid {
            SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
                grid_leader: SCALAR,
            }
        } else {
            SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: SCALAR,
                raw_index: SCALAR,
            }
        })],
        options,
        enumeration,
    }
}
impl Model {
    fn source<'a>(&'a self, resources: &Prep<'_, '_>) -> Source<'a> {
        Source {
            function: &self.function,
            types: &self.types,
            callables: &self.calls,
            ledger: resources.original_ledger_v1().unwrap(),
        }
    }
}
struct State {
    initial: InitialDestinations,
    arguments: RetainedBeforeArgumentWritersV1,
    prefix: RootEntryPrefixV1,
    graph: RetainedInitialCapabilityGraphV1,
    seed: RetainedInvocationSeedV1,
}
impl State {
    fn new(resources: &mut Prep<'_, '_>) -> Self {
        let mut initial = InitialDestinations::new();
        initial.prepare(4, resources).unwrap();
        let mut arguments = RetainedBeforeArgumentWritersV1::new();
        arguments.prepare_into(4, resources).unwrap();
        let mut prefix = RootEntryPrefixV1::empty();
        resources.reserve(&mut prefix.entry_operations, 1).unwrap();
        prefix
            .entry_operations
            .push(ProductionRankedOperationV1::IndexConstant {
                result: ProductionRankedValueIdV1::new(80),
                value: 17,
            });
        prefix.next_value = 81;
        arguments.runtime_index_arguments[2] = Some(9);
        arguments.next_runtime_argument = 10;
        Self {
            initial,
            arguments,
            prefix,
            graph: RetainedInitialCapabilityGraphV1::new(),
            seed: RetainedInvocationSeedV1::new(),
        }
    }
    // Called only after State is in its final outer slot: source keys bind its addresses.
    fn graph(&mut self, model: &Model, resources: &mut Prep<'_, '_>) {
        let source = model.source(resources);
        self.graph
            .prepare_into(
                &source,
                &model.counts,
                &model.options,
                &model.enumeration,
                Some(64),
                &mut self.initial,
                &mut self.arguments,
                &self.prefix,
                resources,
            )
            .unwrap();
    }
    fn seed(&mut self, model: &Model, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let source = model.source(resources);
        self.seed.prepare_into(
            &source,
            &model.counts,
            &model.escaped,
            &model.options,
            &model.enumeration,
            Some(64),
            &self.graph,
            &mut self.initial,
            &self.arguments,
            &mut self.prefix,
            resources,
        )
    }
    fn completed(&self, model: &Model, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let source = model.source(resources);
        self.seed.completed_for(
            &source,
            &model.counts,
            &model.escaped,
            &model.options,
            &model.enumeration,
            Some(64),
            &self.graph,
            &self.initial,
            &self.arguments,
            &self.prefix,
            resources,
        )
    }
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
fn retained_invocation_seed_preserves_prefix_namespace_and_stops_before_propagation() {
    let model = model(&[2, 1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    let indices = state.initial.indices.as_ptr();
    let edges = state.initial.edges.clone(); // Inert oracle, not production accounting.
    state
        .seed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    state
        .completed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    assert_eq!(state.initial.indices.as_ptr(), indices);
    assert_eq!(state.seed.index_fifo, vec![2, 1]);
    assert_eq!((state.seed.index_cursor, state.seed.grid_cursor), (0, 0));
    assert_eq!(state.initial.edges, edges);
    assert!(state.initial.indices[3].is_none());
    assert_eq!(state.prefix.next_value, 83);
    assert_eq!(
        state.prefix.entry_operations[0],
        ProductionRankedOperationV1::IndexConstant {
            result: ProductionRankedValueIdV1::new(80),
            value: 17
        }
    );
    assert_eq!(
        state.arguments.runtime_index_arguments,
        vec![None, None, Some(9), None]
    );
    assert_eq!(state.arguments.next_runtime_argument, 10);
    let source = model.source(&Prep::new(&mut budget, &mut owned));
    assert!(
        state
            .graph
            .completed_for(
                &source,
                &model.counts,
                &model.options,
                &model.enumeration,
                Some(64),
                &state.initial,
                &state.arguments,
                &state.prefix,
                &Prep::new(&mut budget, &mut owned)
            )
            .is_err()
    ); // Obsolete writer floor.
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_invocation_seed_empty_source_keeps_both_fifos_empty() {
    let model = model(&[], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    state
        .seed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    state
        .completed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    assert_eq!(
        (
            state.seed.index_fifo.capacity(),
            state.seed.grid_fifo.capacity()
        ),
        (0, 0)
    );
    assert_eq!(
        (state.prefix.entry_operations.len(), state.prefix.next_value),
        (1, 81)
    );
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_scalar_refusal_retains_emission_before_custody() {
    for escaped in [false, true] {
        let mut model = model(&[1], false);
        if escaped {
            model.escaped[1] = true;
        } else {
            model.counts[1] = 2;
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
        state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
        let error = state
            .seed(&model, &mut Prep::new(&mut budget, &mut owned))
            .unwrap_err();
        assert!(matches!(&error, Backend::Incomplete(_)));
        assert_eq!(
            (state.prefix.entry_operations.len(), state.prefix.next_value),
            (2, 82)
        );
        assert!(state.initial.indices[1].is_none());
        assert!(state.seed.index_fifo.is_empty());
        assert_eq!(state.seed.phase, SeedPhase::Terminal);
        let before = (budget.work(), budget.storage(), owned);
        let repeated = state
            .seed(&model, &mut Prep::new(&mut budget, &mut owned))
            .unwrap_err();
        assert!(same_error(&error, &repeated));
        assert_eq!(before, (budget.work(), budget.storage(), owned));
        drop(state);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_invocation_seed_duplicate_refusal_keeps_first_seed() {
    let mut model = model(&[1, 1], false);
    // Deliberate DATA-only custody override isolates the duplicate donor branch.
    model.counts[1] = 1;
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    assert!(matches!(
        state.seed(&model, &mut Prep::new(&mut budget, &mut owned)),
        Err(Backend::Unsupported(
            "multiple invocation capabilities for one semantic local"
        ))
    ));
    assert_eq!(state.seed.index_fifo, vec![1]);
    assert!(state.initial.indices[1].is_some());
    assert_eq!(
        (state.prefix.entry_operations.len(), state.prefix.next_value),
        (2, 82)
    );
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_grid_refusal_precedes_seed_allocation() {
    let model = model(&[1], true);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    assert!(matches!(
        state.seed(&model, &mut Prep::new(&mut budget, &mut owned)),
        Err(Backend::Incomplete(
            "unjoined index data does not support grid-leader seeds"
        ))
    ));
    assert_eq!(
        (state.prefix.entry_operations.len(), state.prefix.next_value),
        (1, 81)
    );
    assert!(state.initial.predicates.iter().all(Option::is_none));
    assert_eq!(
        (
            state.seed.index_fifo.capacity(),
            state.seed.grid_fifo.capacity()
        ),
        (0, 0)
    );
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_overflow_retains_original_counter_and_prefix() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.prefix.next_value = u32::MAX;
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    assert!(matches!(
        state.seed(&model, &mut Prep::new(&mut budget, &mut owned)),
        Err(Backend::Unsupported("too many ranked SSA values"))
    ));
    assert_eq!(
        (state.prefix.entry_operations.len(), state.prefix.next_value),
        (1, u32::MAX)
    );
    assert!(state.initial.indices[1].is_none());
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_foreign_counters_and_budgets_refuse_without_spend() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    state
        .seed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let before = (budget.work(), budget.storage(), owned);
    let mut foreign = owned;
    accounting_error(state.completed(&model, &mut Prep::new(&mut budget, &mut foreign)));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    let mut other_work = Work::new(LIMIT);
    let mut other = Budget::new(&mut other_work, LIMIT);
    other.reserve_storage(owned).unwrap();
    accounting_error(state.completed(&model, &mut Prep::new(&mut other, &mut owned)));
    assert_eq!(other.work(), 0);
    other.release_storage(owned).unwrap();
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_equal_counts_or_escaped_copy_is_foreign_identity() {
    for counts in [false, true] {
        let mut model = model(&[1], false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
        state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
        state
            .seed(&model, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        let old_counts = model.counts.clone();
        let old_escaped = model.escaped.clone();
        let held_counts;
        let held_escaped;
        if counts {
            held_counts = Some(std::mem::replace(&mut model.counts, old_counts));
            held_escaped = None;
        } else {
            held_escaped = Some(std::mem::replace(&mut model.escaped, old_escaped));
            held_counts = None;
        }
        let before = (budget.work(), budget.storage(), owned);
        accounting_error(state.completed(&model, &mut Prep::new(&mut budget, &mut owned)));
        assert_eq!(before, (budget.work(), budget.storage(), owned));
        drop((held_counts, held_escaped));
        drop(state);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_invocation_seed_current_count_or_escape_change_refuses_completion() {
    for escaped in [false, true] {
        let mut model = model(&[1], false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
        state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
        state
            .seed(&model, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        if escaped {
            model.escaped[1] = true;
        } else {
            model.counts[1] = 2;
        }
        accounting_error(state.completed(&model, &mut Prep::new(&mut budget, &mut owned)));
        drop(state);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_invocation_seed_cursor_prefix_and_index_changes_refuse_completion() {
    for which in 0..4 {
        let model = model(&[1], false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut owned = 0;
        let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
        state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
        state
            .seed(&model, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        match which {
            0 => state.seed.index_cursor = 1,
            1 => state.prefix.next_value += 1,
            2 => state.initial.indices[1] = None,
            _ => state.arguments.next_runtime_argument += 1,
        }
        accounting_error(state.completed(&model, &mut Prep::new(&mut budget, &mut owned)));
        drop(state);
        budget.release_storage(owned).unwrap();
    }
}
#[test]
fn retained_invocation_seed_reentry_revokes_component_completion() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    state
        .seed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap();
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        state.prefix.next_value,
    );
    accounting_error(state.seed(&model, &mut Prep::new(&mut budget, &mut owned)));
    assert_eq!(state.seed.phase, SeedPhase::Terminal);
    accounting_error(state.completed(&model, &mut Prep::new(&mut budget, &mut owned)));
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            state.prefix.next_value
        )
    );
    drop(state);
    budget.release_storage(owned).unwrap();
}
fn baseline() -> (usize, usize) {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    let base = (budget.work(), budget.storage());
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    base
}
fn trial(
    base: (usize, usize),
    work_extra: usize,
    storage_extra: usize,
) -> (bool, usize, usize, bool, bool, bool) {
    let model = model(&[1], false);
    let mut work = Work::new(base.0 + work_extra);
    let mut budget = Budget::new(&mut work, base.1 + storage_extra);
    budget.reserve_storage(FLOOR).unwrap();
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    assert_eq!((budget.work(), budget.storage()), base);
    let result = state.seed(&model, &mut Prep::new(&mut budget, &mut owned));
    let observation = (
        result.is_ok(),
        budget.work() - base.0,
        budget.storage() - base.1,
        state.prefix.entry_operations.len() > 1,
        state.initial.indices[1].is_some(),
        !state.seed.index_fifo.is_empty(),
    );
    assert_eq!(budget.storage(), FLOOR + owned);
    let before = (
        budget.work(),
        budget.storage(),
        owned,
        state.prefix.entry_operations.len(),
        state.prefix.next_value,
        state.seed.index_fifo.len(),
        state.seed.index_fifo.capacity(),
    );
    let repeated = state
        .seed(&model, &mut Prep::new(&mut budget, &mut owned))
        .unwrap_err();
    if let Err(error) = result {
        assert!(same_error(&error, &repeated));
    }
    assert_eq!(state.seed.phase, SeedPhase::Terminal);
    assert_eq!(
        before,
        (
            budget.work(),
            budget.storage(),
            owned,
            state.prefix.entry_operations.len(),
            state.prefix.next_value,
            state.seed.index_fifo.len(),
            state.seed.index_fifo.capacity()
        )
    );
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    observation
}
#[test]
fn retained_invocation_seed_selected_work_cuts_retain_partial_fifo_and_exact_first_error() {
    let base = baseline();
    let good = trial(base, LIMIT, LIMIT);
    assert!(good.0);
    // This is selected-cut coverage, not all-cut or genuine postflight coverage.
    let mut between_index_and_queue = false;
    for cut in [0, 31, 32, 47, 200, 255, 272, 273, 289, good.1 - 1] {
        if cut >= good.1 {
            continue;
        }
        let denied = trial(base, cut, LIMIT);
        assert!(!denied.0);
        between_index_and_queue |= denied.4 && !denied.5;
    }
    assert!(between_index_and_queue);
    assert!(trial(base, good.1, LIMIT).0);
}
#[test]
fn retained_invocation_seed_selected_storage_cuts_keep_original_attached_payloads() {
    let base = baseline();
    let good = trial(base, LIMIT, LIMIT);
    let frame = frame::bytes().unwrap();
    let operation = size_of::<ProductionRankedOperationV1>();
    let mut operation_before_fifo = false;
    for cut in [0, frame - 1, frame, frame + operation, good.2 - 1] {
        if cut >= good.2 {
            continue;
        }
        let denied = trial(base, LIMIT, cut);
        assert!(!denied.0);
        operation_before_fifo |= denied.4 && !denied.5;
    }
    assert!(operation_before_fifo);
    assert!(trial(base, LIMIT, good.2).0);
}
#[test]
fn retained_invocation_seed_outer_callback_unwind_keeps_payload_until_drop() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        state
            .seed(&model, &mut Prep::new(&mut budget, &mut owned))
            .unwrap();
        std::panic::panic_any(());
    }));
    assert!(caught.is_err());
    assert_eq!(state.seed.index_fifo, vec![1]);
    assert_eq!(
        (state.prefix.entry_operations.len(), state.prefix.next_value),
        (2, 82)
    );
    assert_eq!(budget.storage(), owned);
    drop(state);
    budget.release_storage(owned).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn retained_invocation_seed_whole_transition_invalidates_all_earlier_phases() {
    for phase in [
        WholePhase::Fresh,
        WholePhase::Terminal,
        WholePhase::BeforeArgumentWriters,
        WholePhase::InitialStridedReadsTerminal,
        WholePhase::AfterInitialStridedReads,
        WholePhase::InitialCapabilityGraphTerminal,
        WholePhase::AfterInitialCapabilityGraph,
        WholePhase::InvocationSeedsTerminal,
        WholePhase::AfterInvocationSeeds,
    ] {
        let mut current = phase;
        assert_eq!(
            whole::begin_invocation_seed_transition(&mut current),
            phase == WholePhase::AfterInitialCapabilityGraph
        );
        assert_eq!(current, WholePhase::InvocationSeedsTerminal);
        assert!(!whole::begin_invocation_seed_transition(&mut current));
        assert_eq!(current, WholePhase::InvocationSeedsTerminal);
    }
}

#[test]
fn retained_invocation_seed_foreign_counter_entry_is_terminal_without_spend() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    let before = (budget.work(), budget.storage(), owned);
    let mut foreign = owned;
    accounting_error(state.seed(&model, &mut Prep::new(&mut budget, &mut foreign)));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    assert_eq!(foreign, owned);
    assert_eq!(state.seed.phase, SeedPhase::Terminal);
    assert_eq!(state.prefix.next_value, 81);
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_unmetered_entry_refuses_without_allocation() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    let source = model.source(&Prep::new(&mut budget, &mut owned));
    let before = (budget.work(), budget.storage(), owned);
    accounting_error(state.seed.prepare_into(
        &source,
        &model.counts,
        &model.escaped,
        &model.options,
        &model.enumeration,
        Some(64),
        &state.graph,
        &mut state.initial,
        &state.arguments,
        &mut state.prefix,
        &mut Prep::unmetered(),
    ));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    assert_eq!(
        (
            state.seed.index_fifo.capacity(),
            state.seed.grid_fifo.capacity()
        ),
        (0, 0)
    );
    assert_eq!(state.prefix.next_value, 81);
    drop(state);
    budget.release_storage(owned).unwrap();
}
#[test]
fn retained_invocation_seed_prior_denial_is_terminal_without_seed_mutation() {
    let model = model(&[1], false);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let mut owned = 0;
    let mut state = State::new(&mut Prep::new(&mut budget, &mut owned));
    state.graph(&model, &mut Prep::new(&mut budget, &mut owned));
    assert!(budget.charge_work(LIMIT).is_err());
    let before = (budget.work(), budget.storage(), owned);
    accounting_error(state.seed(&model, &mut Prep::new(&mut budget, &mut owned)));
    assert_eq!(before, (budget.work(), budget.storage(), owned));
    assert_eq!(state.seed.phase, SeedPhase::Terminal);
    assert_eq!(state.prefix.next_value, 81);
    drop(state);
    budget.release_storage(owned).unwrap();
}

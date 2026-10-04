//! Genuine-source retained initial graph DATA oracle; no invocation or route activation.
use super::*;
type OriginalGraph = root_initial_capability_graph_v1::InitialGraphStorageV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct GraphObservation {
    pub(in super::super) locals: usize,
    pub(in super::super) blocks: usize,
    pub(in super::super) edges: usize,
    pub(in super::super) stores: usize,
    pub(in super::super) loads: usize,
    pub(in super::super) borrowed: usize,
    pub(in super::super) prefix_operations: usize,
    pub(in super::super) next_value: u32,
    pub(in super::super) next_argument: usize,
}
pub(in super::super) struct GraphOracle {
    graph: OriginalGraph,
    counts: Vec<u8>,
    indices: Vec<Option<u32>>,
    slices: Vec<Option<u32>>,
    execution: Option<ProductionRankedOperationV1>,
    writers: Option<WriterFloor>,
    source: Option<GraphSourceKey>,
    ledger: Option<Snapshot>,
    failure: Option<Backend>,
    entered: bool,
    populated: bool,
    postflight_paid: bool,
    observation: Option<GraphObservation>,
}
impl GraphOracle {
    pub(in super::super) fn new() -> Self {
        Self {
            graph: OriginalGraph::empty(),
            counts: Vec::new(),
            indices: Vec::new(),
            slices: Vec::new(),
            execution: None,
            writers: None,
            source: None,
            ledger: None,
            failure: None,
            entered: false,
            populated: false,
            postflight_paid: false,
            observation: None,
        }
    }
    fn saved(&self) -> Backend {
        match &self.failure {
            Some(Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))) => {
                resource(*error)
            }
            Some(Backend::Unsupported(detail)) => Backend::Unsupported(*detail),
            Some(Backend::Incomplete(detail)) => Backend::Incomplete(*detail),
            _ => accounting(),
        }
    }
    pub(in super::super) fn observation(&self) -> Option<GraphObservation> {
        self.observation
    }
}
pub(in super::super) fn comparison_work(
    count: usize,
    edges: usize,
    stores: usize,
    loads: usize,
    borrowed: usize,
) -> BResult<usize> {
    // Per local: four None/row scans, copied and current counts, both argument slots plus fixed
    // writer/source comparisons. Each Copy graph row has fewer than 64 scalar
    // fields, including the largest capability kind; nested row headers count
    // in the local term. This prepays one complete independent DATA comparison.
    count
        .checked_mul(33)
        .and_then(|n| {
            edges
                .checked_add(stores)
                .and_then(|m| m.checked_add(loads))
                .and_then(|m| m.checked_add(borrowed))
                .and_then(|m| m.checked_mul(64))
                .and_then(|m| n.checked_add(m))
        })
        .and_then(|n| n.checked_add(128))
        .ok_or_else(arithmetic)
}
fn current_counts_match(key: (usize, usize), current: &[u8], expected: &[u8]) -> bool {
    key == (current.as_ptr() as usize, current.len()) && current == expected
}
fn compare_rows(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &GraphOracle,
    source: &Source<'_>,
) -> BResult<GraphObservation> {
    let scalar_stage = &pending.earlier.earlier.earlier;
    let enumeration_stage = &scalar_stage.earlier;
    let initial = &enumeration_stage.options.initial;
    let stage = &pending.initial_graph;
    let count = source.function.locals().len();
    let expected = oracle.observation.ok_or_else(accounting)?;
    // Expected dimensions bound every scan before any element comparison.
    if expected.locals != count
        || expected.blocks != source.function.blocks().len()
        || !oracle.populated
        || stage.phase != GraphPhase::Complete
        || stage.failure.is_some()
        || stage.source != oracle.source
        || stage.launch_extent != Some(0)
        || stage.local_definitions.len() != count
        || oracle.counts.len() != count
        || initial.indices.len() != count
        || initial.leaders.len() != count
        || initial.predicates.len() != count
        || initial.edges.len() != count
        || oracle.graph.edges.len() != count
        || pending.arguments.runtime_index_arguments.len() != count
        || pending.arguments.runtime_slice_extent_arguments.len() != count
        || oracle.indices.len() != count
        || oracle.slices.len() != count
        || pending.arguments.edge_count != expected.edges
        || initial.stores.len() != expected.stores
        || initial.loads.len() != expected.loads
        || pending.arguments.borrowed_locals.len() != expected.borrowed
        || oracle.graph.edge_count != expected.edges
        || oracle.graph.stores.len() != expected.stores
        || oracle.graph.loads.len() != expected.loads
        || oracle.graph.borrowed.len() != expected.borrowed
        || pending.prefix.entry_operations.len() != 1
        || oracle.writers != Some(WriterFloor::take(&pending.arguments, &pending.prefix))
    {
        return Err(accounting());
    }
    // Sum row lengths under the paid local scan before nested equality.
    let actual_edges = initial.edges.iter().try_fold(0usize, |n, row| {
        n.checked_add(row.len()).ok_or_else(arithmetic)
    })?;
    let oracle_edges = oracle.graph.edges.iter().try_fold(0usize, |n, row| {
        n.checked_add(row.len()).ok_or_else(arithmetic)
    })?;
    if actual_edges != expected.edges
        || oracle_edges != expected.edges
        || initial.indices.iter().any(Option::is_some)
        || initial.leaders.iter().any(Option::is_some)
        || initial.predicates.iter().any(Option::is_some)
        || stage.local_definitions != oracle.counts
        || initial.edges != oracle.graph.edges
        || initial.stores != oracle.graph.stores
        || initial.loads != oracle.graph.loads
        || pending.arguments.borrowed_locals != oracle.graph.borrowed
        || pending.arguments.runtime_index_arguments != oracle.indices
        || pending.arguments.runtime_slice_extent_arguments != oracle.slices
        || Some(&pending.prefix.entry_operations[0]) != oracle.execution.as_ref()
    {
        return Err(accounting());
    }
    Ok(expected)
}
// Seed-specific DATA comparison only. Old compare_rows remains byte-exact.
pub(in super::super) fn compare_seed_successor_graph_rows(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &GraphOracle,
    source: &Source<'_>,
) -> BResult<GraphObservation> {
    let scalar_stage = &pending.earlier.earlier.earlier;
    let enumeration_stage = &scalar_stage.earlier;
    let initial = &enumeration_stage.options.initial;
    let stage = &pending.initial_graph;
    let count = source.function.locals().len();
    let expected = oracle.observation.ok_or_else(accounting)?;
    // Expected dimensions bound every scan before any element comparison.
    if expected.locals != count
        || expected.blocks != source.function.blocks().len()
        || !oracle.populated
        || stage.phase != GraphPhase::Complete
        || stage.failure.is_some()
        || stage.source != oracle.source
        || stage.launch_extent != Some(0)
        || stage.local_definitions.len() != count
        || oracle.counts.len() != count
        || initial.indices.len() != count
        || initial.leaders.len() != count
        || initial.predicates.len() != count
        || initial.edges.len() != count
        || oracle.graph.edges.len() != count
        || pending.arguments.runtime_index_arguments.len() != count
        || pending.arguments.runtime_slice_extent_arguments.len() != count
        || oracle.indices.len() != count
        || oracle.slices.len() != count
        || pending.arguments.edge_count != expected.edges
        || initial.stores.len() != expected.stores
        || initial.loads.len() != expected.loads
        || pending.arguments.borrowed_locals.len() != expected.borrowed
        || oracle.graph.edge_count != expected.edges
        || oracle.graph.stores.len() != expected.stores
        || oracle.graph.loads.len() != expected.loads
        || oracle.graph.borrowed.len() != expected.borrowed
        || pending.prefix.entry_operations.is_empty()
    {
        return Err(accounting());
    }
    // Sum row lengths under the paid local scan before nested equality.
    let actual_edges = initial.edges.iter().try_fold(0usize, |n, row| {
        n.checked_add(row.len()).ok_or_else(arithmetic)
    })?;
    let oracle_edges = oracle.graph.edges.iter().try_fold(0usize, |n, row| {
        n.checked_add(row.len()).ok_or_else(arithmetic)
    })?;
    if actual_edges != expected.edges
        || oracle_edges != expected.edges
        || initial.leaders.iter().any(Option::is_some)
        || initial.predicates.iter().any(Option::is_some)
        || stage.local_definitions != oracle.counts
        || initial.edges != oracle.graph.edges
        || initial.stores != oracle.graph.stores
        || initial.loads != oracle.graph.loads
        || pending.arguments.borrowed_locals != oracle.graph.borrowed
        || pending.arguments.runtime_index_arguments != oracle.indices
        || pending.arguments.runtime_slice_extent_arguments != oracle.slices
        || Some(&pending.prefix.entry_operations[0]) != oracle.execution.as_ref()
    {
        return Err(accounting());
    }
    Ok(expected)
}
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn observe_before_reentry_in_scope<'s>(
    pending: &mut PendingWholeRootBeforeArgumentWritersV1<'s>,
    oracle: &mut GraphOracle,
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<GraphObservation> {
    pending.completed_after_initial_strided_reads(owner, function_id, facts, owned)?;
    let entry = pending.entry.ok_or_else(accounting)?;
    let semantic = owner.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(function_id.index() as usize)
        .ok_or_else(accounting)?;
    let source = Source {
        function,
        types: semantic.types(),
        callables: semantic.callables(),
        ledger: (entry.budget_slot, entry.work_ledger),
    };
    let donor =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            let result = prepare_original(pending, oracle, &source, resources);
            match result {
                Ok(()) => Ok(()),
                Err(error) => {
                    oracle.failure = Some(error);
                    Err(oracle.saved())
                }
            }
        });
    if let Err(error) = donor {
        return Err(if oracle.failure.is_some() {
            oracle.saved()
        } else {
            error
        });
    }
    // A real invocation of the new retained bridge, not a reachability marker.
    pending
        .prepare_initial_capability_graph(owner, function_id, facts, owned)
        .map_err(nominal_error)?;
    pending.completed_after_initial_capability_graph(owner, function_id, facts, owned)?;
    let observed =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            let expected = oracle.observation.ok_or_else(accounting)?;
            let work = comparison_work(
                expected.locals,
                expected.edges,
                expected.stores,
                expected.loads,
                expected.borrowed,
            )?;
            // One pass now and one after actual checked/canonical postflight.
            resources.work(work.checked_mul(2).ok_or_else(arithmetic)?)?;
            let observed = compare_rows(pending, oracle, &source)?;
            oracle.postflight_paid = true;
            oracle.ledger = resources.retained_custody_snapshot_v1();
            Ok(observed)
        })?;
    Ok(observed)
}
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn observe_in_scope<'s>(
    pending: &mut PendingWholeRootBeforeArgumentWritersV1<'s>,
    oracle: &mut GraphOracle,
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<GraphObservation> {
    let observed =
        observe_before_reentry_in_scope(pending, oracle, owner, function_id, facts, owned)?;
    let held = facts.retained_whole_root_snapshot_v1(owner, function_id, owned, None)?;
    if pending
        .completed_for(owner, function_id, facts, owned)
        .is_ok()
        || pending
            .completed_after_initial_strided_reads(owner, function_id, facts, owned)
            .is_ok()
    {
        return Err(accounting());
    }
    let mut foreign = *owned;
    if pending
        .completed_after_initial_capability_graph(owner, function_id, facts, &mut foreign)
        .is_ok()
    {
        return Err(accounting());
    }
    if pending.prepare_initial_capability_graph(owner, function_id, facts, owned)
        != Err(QueryError::Resource(Resource::Accounting))
        || pending.phase != WholePhase::InitialCapabilityGraphTerminal
        || pending
            .completed_after_initial_capability_graph(owner, function_id, facts, owned)
            .is_ok()
    {
        return Err(accounting());
    }
    let after = facts.retained_whole_root_snapshot_v1(owner, function_id, owned, Some(held))?;
    if after != held || foreign != *owned {
        return Err(accounting());
    }
    Ok(observed)
}
fn prepare_original(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &mut GraphOracle,
    source: &Source<'_>,
    resources: &mut Prep<'_, '_>,
) -> BResult<()> {
    if oracle.entered
        || resources.original_ledger_v1() != Some(source.ledger)
        || resources.has_denial()
    {
        return Err(accounting());
    }
    oracle.entered = true;
    let view = pending.earlier.view(source, resources)?;
    let counts = &view.scalar_inventory().counts;
    let count = source.function.locals().len();
    let selected = pending.selected.as_ref().ok_or_else(accounting)?;
    let launch = bounded_linear_launch_extent_v1(&selected.input().source_launch);
    let execution = ranked_execution_layout_v1(selected.source_root().layout());
    if count > 4096
        || counts.len() != count
        || source.function.blocks().len() > 32
        || !selected.references().is_empty()
        || pending.prefix.reserved_reference_values.is_some()
        || pending.prefix.entry_operations.len() != 1
        || pending.prefix.entry_operations[0] != execution
        || pending.prefix.next_value != 0
        || pending.arguments.next_runtime_argument != 1
        || pending.arguments.runtime_index_arguments.len() != count
        || pending.arguments.runtime_slice_extent_arguments.len() != count
    {
        return Err(Backend::Incomplete(
            "genuine graph observer requires the checked empty-read execution-only predecessor",
        ));
    }
    resources.work(
        count
            .checked_mul(2)
            .and_then(|n| n.checked_add(32))
            .ok_or_else(arithmetic)?,
    )?;
    if pending
        .arguments
        .runtime_index_arguments
        .iter()
        .any(Option::is_some)
        || pending
            .arguments
            .runtime_slice_extent_arguments
            .iter()
            .any(Option::is_some)
    {
        return Err(accounting());
    }
    let scalar_stage = &pending.earlier.earlier.earlier;
    let enumeration_stage = &scalar_stage.earlier;
    oracle.source = Some(GraphSourceKey::new(
        source,
        counts,
        view.option_dominance(),
        view.enum_dominance(),
        launch,
        &enumeration_stage.options.initial,
        &pending.arguments,
        &pending.prefix,
    ));
    oracle.writers = Some(WriterFloor::take(&pending.arguments, &pending.prefix));
    oracle.execution = Some(execution); // ExecutionLayout has no nested heap payload.
    resources.work(count)?;
    resources.reserve(&mut oracle.counts, count)?;
    oracle.counts.extend_from_slice(counts);
    resources.work(count)?;
    resources.reserve(&mut oracle.indices, count)?;
    oracle
        .indices
        .extend_from_slice(&pending.arguments.runtime_index_arguments);
    resources.work(count)?;
    resources.reserve(&mut oracle.slices, count)?;
    oracle
        .slices
        .extend_from_slice(&pending.arguments.runtime_slice_extent_arguments);
    resources.work(count)?;
    resources.reserve(&mut oracle.graph.edges, count)?;
    oracle.graph.edges.resize_with(count, Vec::new);
    // Exact current donor, same original meter. Every destination was attached
    // to oracle before this callback and remains held through parent postflight.
    root_initial_capability_graph_v1::populate_initial_graph_v1(
        source.callables,
        source.function,
        launch,
        &oracle.counts,
        view.option_dominance(),
        view.enum_dominance(),
        &mut oracle.graph.edges,
        &mut oracle.graph.edge_count,
        &mut oracle.graph.stores,
        &mut oracle.graph.loads,
        &mut oracle.graph.borrowed,
        resources,
    )?;
    oracle.populated = true;
    oracle.observation = Some(GraphObservation {
        locals: count,
        blocks: source.function.blocks().len(),
        edges: oracle.graph.edge_count,
        stores: oracle.graph.stores.len(),
        loads: oracle.graph.loads.len(),
        borrowed: oracle.graph.borrowed.len(),
        prefix_operations: pending.prefix.entry_operations.len(),
        next_value: pending.prefix.next_value,
        next_argument: pending.arguments.next_runtime_argument,
    });
    Ok(())
}
/// DATA-only second comparison after the real canonical callback/postflight.
/// Canonical scratch can legitimately have been refunded; the enclosing original
/// Custody floor, not an in-scope equality, checks retained storage lifetime.
pub(in super::super) fn postflight(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &mut GraphOracle,
    owner: &ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    resources: &Prep<'_, '_>,
) -> BResult<GraphObservation> {
    let paid = oracle.postflight_paid;
    oracle.postflight_paid = false;
    let held = oracle.ledger.ok_or_else(accounting)?;
    let now = resources
        .retained_custody_snapshot_v1()
        .ok_or_else(accounting)?;
    if !paid
        || oracle.failure.is_some()
        || pending.failure.is_some()
        || pending.phase != WholePhase::InitialCapabilityGraphTerminal
        || !pending
            .owner
            .is_some_and(|bound| std::ptr::eq(bound, owner))
        || pending.function != Some(function_id)
        || now.budget_slot != held.budget_slot
        || now.work_ledger != held.work_ledger
        || now.owned_slot != held.owned_slot
        || now.owned != held.owned
        || now.work < held.work
        || now.peak < held.peak
        || now.denied_work
        || now.denied_storage
    {
        return Err(accounting());
    }
    let semantic = owner.semantic_ssa().source_semantic();
    let function = semantic
        .functions()
        .get(function_id.index() as usize)
        .ok_or_else(accounting)?;
    let source = Source {
        function,
        types: semantic.types(),
        callables: semantic.callables(),
        ledger: (now.budget_slot, now.work_ledger),
    };
    let key = oracle.source.ok_or_else(accounting)?;
    let scalar_stage = &pending.earlier.earlier.earlier;
    let current_inventory = scalar_stage.scalar.completed_for(function, resources)?;
    let current_counts = &current_inventory.counts;
    let enumeration_stage = &scalar_stage.earlier;
    let options = enumeration_stage
        .options
        .dominance
        .completed()
        .ok_or_else(accounting)?;
    let enumeration = enumeration_stage
        .enumeration
        .completed()
        .ok_or_else(accounting)?;
    let selected = pending.selected.as_ref().ok_or_else(accounting)?;
    if key.source != source.identity()
        || current_counts.len() != function.locals().len()
        || !current_counts_match(key.counts, current_counts, &oracle.counts)
        || key.options != options as *const _ as usize
        || key.enumeration != enumeration as *const _ as usize
        || key.initial != &enumeration_stage.options.initial as *const _ as usize
        || key.arguments != &pending.arguments as *const _ as usize
        || key.prefix != &pending.prefix as *const _ as usize
        || key.launch_upper_bound
            != bounded_linear_launch_extent_v1(&selected.input().source_launch)
    {
        return Err(accounting());
    }
    compare_rows(pending, oracle, &source)
}
pub(in super::super) fn frame() -> BResult<usize> {
    const ROWS: usize = 9;
    let rows = [
        super::frame::bytes()?,
        size_of::<(
            GraphOracle,
            GraphObservation,
            Option<GraphObservation>,
            OriginalGraph,
            Vec<u8>,
            Vec<Option<u32>>,
            Vec<Option<u32>>,
            Option<ProductionRankedOperationV1>,
            Option<WriterFloor>,
            Option<GraphSourceKey>,
            Option<Snapshot>,
            Option<Backend>,
        )>(),
        size_of::<(
            &mut GraphOracle,
            &GraphOracle,
            &mut PendingWholeRootBeforeArgumentWritersV1<'static>,
            &PendingWholeRootBeforeArgumentWritersV1<'static>,
            &ProductionPreRankedKirOwnerV1,
            SemanticFunctionIdV1,
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &mut usize,
            Source<'static>,
            Snapshot,
            Snapshot,
            Option<Snapshot>,
            BResult<Snapshot>,
        )>(),
        size_of::<(
            BeforeCapabilitiesV1<'static>,
            &AssertionDefinitionInventoryV1,
            &AssertionDefinitionInventoryV1,
            BResult<&AssertionDefinitionInventoryV1>,
            &[u8],
            &ActualSelectedInputsV1<'static>,
            &LaunchContract,
            Option<u64>,
            ProductionRankedOperationV1,
            &mut Prep<'static, 'static>,
            &Prep<'static, 'static>,
            &Vec<u8>,
            &Vec<Option<u32>>,
            BResult<()>,
            BResult<GraphObservation>,
            Result<()>,
            QueryError,
            Backend,
            Resource,
        )>(),
        size_of::<(
            std::slice::Iter<'static, Vec<CapabilityEdgeV1>>,
            std::slice::Iter<'static, Option<ProjectedDisjointIndexV1>>,
            std::slice::Iter<'static, Option<ProjectedGridLeaderV1>>,
            std::slice::Iter<'static, Option<GuardPredicateV1>>,
            std::slice::Iter<'static, Option<u32>>,
            usize,
            usize,
            usize,
            usize,
            bool,
        )>(),
        size_of::<(
            &[CapabilityEdgeV1],
            &[PendingEnumPayloadStoreV1],
            &[PendingEnumPayloadLoadV1],
            &[(usize, usize)],
            &[u8],
            &[Option<u32>],
            &ProductionRankedOperationV1,
            &[u8],
            &[u8],
            (usize, usize),
            bool,
            &CapabilityEdgeV1,
            &CapabilityEdgeKindV1,
            &PendingEnumPayloadStoreV1,
            &PendingEnumPayloadLoadV1,
            &(usize, usize),
            Option<usize>,
            Option<&Backend>,
        )>(),
        // Nonterminal factoring and seed-only DATA comparator add source-carrier
        // frames without changing the old graph marker/reentry semantics.
        size_of::<(
            &mut PendingWholeRootBeforeArgumentWritersV1<'static>,
            &mut GraphOracle,
            &ProductionPreRankedKirOwnerV1,
            SemanticFunctionIdV1,
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &mut usize,
            GraphObservation,
            BResult<GraphObservation>,
        )>(),
        size_of::<(
            &PendingWholeRootBeforeArgumentWritersV1<'static>,
            &GraphOracle,
            &Source<'static>,
            GraphObservation,
            usize,
            usize,
            usize,
            BResult<GraphObservation>,
            bool,
        )>(),
        size_of::<(
            [usize; ROWS],
            std::array::IntoIter<usize, ROWS>,
            usize,
            usize,
            Option<usize>,
            BResult<usize>,
        )>(),
    ];
    rows.into_iter()
        .try_fold(0usize, |n, row| n.checked_add(row).ok_or_else(arithmetic))
}
#[test]
fn genuine_graph_comparison_work_charges_each_local_and_copy_row() {
    assert_eq!(
        comparison_work(4, 3, 2, 1, 1).unwrap(),
        4 * 33 + 7 * 64 + 128
    );
    assert_eq!(comparison_work(0, 0, 0, 0, 0).unwrap(), 128);
}
#[test]
fn genuine_graph_comparison_work_overflow_refuses() {
    assert!(matches!(
        comparison_work(usize::MAX, 0, 0, 0, 0),
        Err(Backend::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
        ))
    ));
    assert!(comparison_work(0, usize::MAX, 1, 0, 0).is_err());
}
#[test]
fn genuine_graph_current_counts_refuses_foreign_pointer_length_or_content() {
    let retained = [1_u8, 0, 2];
    let equal = [1_u8, 0, 2];
    let changed = [1_u8, 0, 3];
    let key = (retained.as_ptr() as usize, retained.len());
    assert!(current_counts_match(key, &retained, &equal));
    assert!(!current_counts_match(key, &equal, &retained));
    assert!(!current_counts_match((key.0, 2), &retained, &equal));
    assert!(!current_counts_match(key, &retained[..2], &equal[..2]));
    assert!(!current_counts_match(key, &retained, &changed));
}

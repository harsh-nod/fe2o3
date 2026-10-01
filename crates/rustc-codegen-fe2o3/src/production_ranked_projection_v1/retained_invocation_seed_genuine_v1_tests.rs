//! Genuine-source seed-only DATA oracle; no recovery, propagation, or route activation.
use super::super::genuine as graph;
use super::*;
type Observation = SeedObservation;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::production_ranked_projection_v1) struct SeedObservation {
    pub(in crate::production_ranked_projection_v1) locals: usize,
    pub(in crate::production_ranked_projection_v1) blocks: usize,
    pub(in crate::production_ranked_projection_v1) seeds: usize,
    pub(in crate::production_ranked_projection_v1) prefix_operations: usize,
    pub(in crate::production_ranked_projection_v1) next_value: u32,
    pub(in crate::production_ranked_projection_v1) next_argument: usize,
}
pub(in crate::production_ranked_projection_v1) struct SeedOracle {
    counts: Vec<u8>,
    escaped: Vec<bool>,
    indices: Vec<Option<ProjectedDisjointIndexV1>>,
    leaders: Vec<Option<ProjectedGridLeaderV1>>,
    predicates: Vec<Option<GuardPredicateV1>>,
    index_fifo: Vec<usize>,
    index_cursor: usize,
    grid_fifo: Vec<usize>,
    grid_cursor: usize,
    operations: Vec<ProductionRankedOperationV1>,
    next_value: u32,
    key: Option<SeedKey>,
    before: Option<WriterFloor>,
    after: Option<WriterFloor>,
    ledger: Option<Snapshot>,
    failure: Option<Backend>,
    entered: bool,
    populated: bool,
    postflight_paid: bool,
    observation: Option<Observation>,
}
impl SeedOracle {
    pub(in crate::production_ranked_projection_v1) fn new() -> Self {
        Self {
            counts: Vec::new(),
            escaped: Vec::new(),
            indices: Vec::new(),
            leaders: Vec::new(),
            predicates: Vec::new(),
            index_fifo: Vec::new(),
            index_cursor: 0,
            grid_fifo: Vec::new(),
            grid_cursor: 0,
            operations: Vec::new(),
            next_value: 0,
            key: None,
            before: None,
            after: None,
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
}
fn comparison_work(locals: usize, seeds: usize) -> BResult<usize> {
    locals
        .checked_mul(12)
        .and_then(|n| seeds.checked_mul(32).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(128))
        .ok_or_else(arithmetic)
}
fn counts_and_escape_match(
    count_key: (usize, usize),
    escaped_key: (usize, usize),
    counts: &[u8],
    escaped: &[bool],
    expected_counts: &[u8],
    expected_escaped: &[bool],
) -> bool {
    count_key == (counts.as_ptr() as usize, counts.len())
        && escaped_key == (escaped.as_ptr() as usize, escaped.len())
        && counts == expected_counts
        && escaped == expected_escaped
}
fn namespace_matches(before: WriterFloor, after: WriterFloor, seeds: usize) -> bool {
    u32::try_from(seeds).ok().is_some_and(|n| {
        before.operations.checked_add(seeds) == Some(after.operations)
            && before.next_value.checked_add(n) == Some(after.next_value)
            && before.next_argument == after.next_argument
            && before.index_rows == after.index_rows
            && before.slice_rows == after.slice_rows
    })
}
fn current_key(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    source: &Source<'_>,
    resources: &Prep<'_, '_>,
) -> BResult<SeedKey> {
    let scalar_stage = &pending.earlier.earlier.earlier;
    let scalar = scalar_stage
        .scalar
        .completed_for(source.function, resources)?;
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
    Ok(SeedKey::new(
        source,
        &scalar.counts,
        &scalar.address_escaped,
        options,
        enumeration,
        bounded_linear_launch_extent_v1(&selected.input().source_launch),
        &pending.initial_graph,
        &enumeration_stage.options.initial,
        &pending.arguments,
        &pending.prefix,
    ))
}
fn compare_rows(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &SeedOracle,
    graph_oracle: &graph::GraphOracle,
    source: &Source<'_>,
    resources: &Prep<'_, '_>,
) -> BResult<Observation> {
    let expected = oracle.observation.ok_or_else(accounting)?;
    let before = oracle.before.ok_or_else(accounting)?;
    let after = oracle.after.ok_or_else(accounting)?;
    let key = oracle.key.ok_or_else(accounting)?;
    let count = source.function.locals().len();
    let scalar_stage = &pending.earlier.earlier.earlier;
    let scalar = scalar_stage
        .scalar
        .completed_for(source.function, resources)?;
    let initial = &scalar_stage.earlier.options.initial;
    let stage = &pending.invocation_seeds;
    // Dimensions precede all row equality; seed count may genuinely be zero.
    if expected.locals != count
        || expected.blocks != source.function.blocks().len()
        || expected.seeds > count
        || !oracle.populated
        || oracle.failure.is_some()
        || stage.phase != SeedPhase::Complete
        || stage.failure.is_some()
        || stage.key != Some(key)
        || current_key(pending, source, resources)? != key
        || oracle.counts.len() != count
        || oracle.escaped.len() != count
        || oracle.indices.len() != count
        || oracle.leaders.len() != count
        || oracle.predicates.len() != count
        || initial.indices.len() != count
        || initial.leaders.len() != count
        || initial.predicates.len() != count
        || stage.index_fifo.len() != expected.seeds
        || oracle.index_fifo.len() != expected.seeds
        || stage.index_cursor != 0
        || oracle.index_cursor != 0
        || stage.grid_cursor != 0
        || oracle.grid_cursor != 0
        || !stage.grid_fifo.is_empty()
        || !oracle.grid_fifo.is_empty()
        || stage.grid_fifo.capacity() != 0
        || oracle.grid_fifo.capacity() != 0
        || stage.before != Some(before)
        || stage.after != Some(after)
        || after != WriterFloor::take(&pending.arguments, &pending.prefix)
        || !namespace_matches(before, after, expected.seeds)
        || oracle.operations.len() != after.operations
        || oracle.next_value != after.next_value
        || expected.prefix_operations != after.operations
        || expected.next_value != after.next_value
        || expected.next_argument != after.next_argument
    {
        return Err(accounting());
    }
    if !counts_and_escape_match(
        key.graph.counts,
        key.address_escaped,
        &scalar.counts,
        &scalar.address_escaped,
        &oracle.counts,
        &oracle.escaped,
    ) || initial.indices != oracle.indices
        || initial.leaders != oracle.leaders
        || initial.predicates != oracle.predicates
        || stage.index_fifo != oracle.index_fifo
        || stage.grid_fifo != oracle.grid_fifo
        || pending.prefix.entry_operations != oracle.operations
    {
        return Err(accounting());
    }
    // Original metered graph oracle remains live. Only seed-owned index rows and
    // the appended operation/value namespace differ from its old writer floor.
    graph::compare_seed_successor_graph_rows(pending, graph_oracle, source)?;
    Ok(expected)
}
fn prepare_original(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &mut SeedOracle,
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
    let scalar_stage = &pending.earlier.earlier.earlier;
    let scalar = scalar_stage
        .scalar
        .completed_for(source.function, resources)?;
    let enumeration_stage = &scalar_stage.earlier;
    let options = enumeration_stage
        .options
        .dominance
        .completed()
        .ok_or_else(accounting)?;
    let initial = &enumeration_stage.options.initial;
    let count = source.function.locals().len();
    if count > 4096
        || source.function.blocks().len() > 32
        || scalar.counts.len() != count
        || scalar.address_escaped.len() != count
        || initial.indices.len() != count
        || initial.leaders.len() != count
        || initial.predicates.len() != count
        || pending.prefix.entry_operations.len() != 1
        || pending.prefix.next_value != 0
        || pending.arguments.next_runtime_argument != 1
    {
        return Err(Backend::Incomplete(
            "genuine seed observer requires checked graph predecessor",
        ));
    }
    resources.work(
        count
            .checked_mul(3)
            .and_then(|n| n.checked_add(32))
            .ok_or_else(arithmetic)?,
    )?;
    if initial.indices.iter().any(Option::is_some)
        || initial.leaders.iter().any(Option::is_some)
        || initial.predicates.iter().any(Option::is_some)
    {
        return Err(accounting());
    }
    let selected = pending.selected.as_ref().ok_or_else(accounting)?;
    let execution = ranked_execution_layout_v1(selected.source_root().layout());
    if pending.prefix.entry_operations[0] != execution {
        return Err(accounting());
    }
    oracle.key = Some(current_key(pending, source, resources)?);
    oracle.before = Some(WriterFloor::take(&pending.arguments, &pending.prefix));
    // All independently retained destinations are attached to this outer oracle
    // before any fallible paid allocation, including both queue Vec headers.
    resources.work(count)?;
    resources.reserve(&mut oracle.counts, count)?;
    oracle.counts.extend_from_slice(&scalar.counts);
    resources.work(count)?;
    resources.reserve(&mut oracle.escaped, count)?;
    oracle.escaped.extend_from_slice(&scalar.address_escaped);
    resources.work(count)?;
    resources.reserve(&mut oracle.indices, count)?;
    oracle.indices.extend_from_slice(&initial.indices);
    resources.work(count)?;
    resources.reserve(&mut oracle.leaders, count)?;
    oracle.leaders.extend_from_slice(&initial.leaders);
    resources.work(count)?;
    resources.reserve(&mut oracle.predicates, count)?;
    oracle.predicates.extend_from_slice(&initial.predicates);
    resources.work(1)?;
    resources.reserve(&mut oracle.operations, 1)?;
    oracle.operations.push(execution);
    oracle.next_value = pending.prefix.next_value;
    donor::seed_retained_root_invocation_values_v1(
        source.callables,
        source.function,
        &oracle.counts,
        &oracle.escaped,
        options,
        &mut oracle.indices,
        &mut oracle.leaders,
        &mut oracle.predicates,
        &mut oracle.index_fifo,
        &mut oracle.index_cursor,
        &mut oracle.grid_fifo,
        &mut oracle.grid_cursor,
        &mut oracle.operations,
        &mut oracle.next_value,
        resources,
    )?;
    let before = oracle.before.ok_or_else(accounting)?;
    let after = WriterFloor {
        operations: oracle.operations.len(),
        next_value: oracle.next_value,
        ..before
    };
    oracle.after = Some(after);
    oracle.populated = true;
    oracle.observation = Some(Observation {
        locals: count,
        blocks: source.function.blocks().len(),
        seeds: oracle.index_fifo.len(),
        prefix_operations: after.operations,
        next_value: after.next_value,
        next_argument: after.next_argument,
    });
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn observe_in_scope<'s>(
    pending: &mut PendingWholeRootBeforeArgumentWritersV1<'s>,
    oracle: &mut SeedOracle,
    graph_oracle: &graph::GraphOracle,
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<Observation> {
    pending.completed_after_initial_capability_graph(owner, function_id, facts, owned)?;
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
    let donor_result =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            match prepare_original(pending, oracle, &source, resources) {
                Ok(()) => Ok(()),
                Err(error) => {
                    oracle.failure = Some(error);
                    Err(oracle.saved())
                }
            }
        });
    if let Err(error) = donor_result {
        return Err(if oracle.failure.is_some() {
            oracle.saved()
        } else {
            error
        });
    }
    pending
        .prepare_invocation_seeds(owner, function_id, facts, owned)
        .map_err(nominal_error)?;
    pending.completed_after_invocation_seeds(owner, function_id, facts, owned)?;
    let observed =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            let expected = oracle.observation.ok_or_else(accounting)?;
            let graph_expected = graph_oracle.observation().ok_or_else(accounting)?;
            let work = comparison_work(expected.locals, expected.seeds)?
                .checked_add(graph::comparison_work(
                    graph_expected.locals,
                    graph_expected.edges,
                    graph_expected.stores,
                    graph_expected.loads,
                    graph_expected.borrowed,
                )?)
                .ok_or_else(arithmetic)?;
            resources.work(work.checked_mul(2).ok_or_else(arithmetic)?)?;
            let observed = compare_rows(pending, oracle, graph_oracle, &source, resources)?;
            oracle.postflight_paid = true;
            oracle.ledger = resources.retained_custody_snapshot_v1();
            Ok(observed)
        })?;
    let held = facts.retained_whole_root_snapshot_v1(owner, function_id, owned, None)?;
    if pending
        .completed_for(owner, function_id, facts, owned)
        .is_ok()
        || pending
            .completed_after_initial_strided_reads(owner, function_id, facts, owned)
            .is_ok()
        || pending
            .completed_after_initial_capability_graph(owner, function_id, facts, owned)
            .is_ok()
    {
        return Err(accounting());
    }
    let mut foreign = *owned;
    if pending
        .completed_after_invocation_seeds(owner, function_id, facts, &mut foreign)
        .is_ok()
    {
        return Err(accounting());
    }
    if pending.prepare_invocation_seeds(owner, function_id, facts, owned)
        != Err(QueryError::Resource(Resource::Accounting))
        || pending.phase != WholePhase::InvocationSeedsTerminal
        || pending
            .completed_after_invocation_seeds(owner, function_id, facts, owned)
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
/// Paid DATA comparison after actual canonical callback/postflight. Canonical
/// scratch refunds do not invalidate the enclosing retained-owner custody floor.
pub(in crate::production_ranked_projection_v1) fn postflight(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &mut SeedOracle,
    graph_oracle: &graph::GraphOracle,
    owner: &ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    resources: &Prep<'_, '_>,
) -> BResult<Observation> {
    let paid = oracle.postflight_paid;
    oracle.postflight_paid = false;
    let held = oracle.ledger.ok_or_else(accounting)?;
    let now = resources
        .retained_custody_snapshot_v1()
        .ok_or_else(accounting)?;
    if !paid
        || oracle.failure.is_some()
        || pending.failure.is_some()
        || pending.phase != WholePhase::InvocationSeedsTerminal
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
    compare_rows(pending, oracle, graph_oracle, &source, resources)
}
pub(in crate::production_ranked_projection_v1) fn frame() -> BResult<usize> {
    const ROWS: usize = 7;
    let rows = [
        super::frame::bytes()?,
        size_of::<(
            SeedOracle,
            Observation,
            Option<Observation>,
            Option<SeedKey>,
            Option<WriterFloor>,
            Option<WriterFloor>,
            Option<Snapshot>,
            Option<Backend>,
        )>(),
        size_of::<(
            &mut SeedOracle,
            &SeedOracle,
            &graph::GraphOracle,
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
            &AssertionDefinitionInventoryV1,
            BResult<&AssertionDefinitionInventoryV1>,
            &[u8],
            &[bool],
            &SemanticOptionDominanceV1,
            &SemanticEnumPayloadDominanceV1,
            &InitialDestinations,
            &ActualSelectedInputsV1<'static>,
            &LaunchContract,
            Option<u64>,
            ProductionRankedOperationV1,
            &mut Prep<'static, 'static>,
            &Prep<'static, 'static>,
            BResult<()>,
            BResult<Observation>,
            Result<()>,
            QueryError,
            Backend,
            Resource,
            SeedKey,
            WriterFloor,
            WriterFloor,
            graph::GraphObservation,
        )>(),
        size_of::<(
            std::slice::Iter<'static, Option<ProjectedDisjointIndexV1>>,
            std::slice::Iter<'static, Option<ProjectedGridLeaderV1>>,
            std::slice::Iter<'static, Option<GuardPredicateV1>>,
            &[Option<ProjectedDisjointIndexV1>],
            &[Option<ProjectedGridLeaderV1>],
            &[Option<GuardPredicateV1>],
            &[usize],
            &[ProductionRankedOperationV1],
            usize,
            usize,
            usize,
            usize,
            u32,
            bool,
        )>(),
        size_of::<(
            (usize, usize),
            (usize, usize),
            &[u8],
            &[u8],
            &[bool],
            &[bool],
            Option<usize>,
            Option<u32>,
            Option<&Backend>,
            &WriterFloor,
            &WriterFloor,
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
fn genuine_seed_comparison_work_accepts_zero_and_charges_each_row() {
    assert_eq!(comparison_work(0, 0).unwrap(), 128);
    assert_eq!(comparison_work(7, 3).unwrap(), 7 * 12 + 3 * 32 + 128);
}
#[test]
fn genuine_seed_comparison_work_overflow_refuses() {
    assert!(comparison_work(usize::MAX, 0).is_err());
    assert!(comparison_work(0, usize::MAX).is_err());
}
#[test]
fn genuine_seed_namespace_preserves_nonzero_prefix_and_zero_seeds() {
    let before = WriterFloor {
        operations: 9,
        next_value: 17,
        next_argument: 5,
        index_rows: 10,
        slice_rows: 10,
    };
    assert!(namespace_matches(before, before, 0));
    let after = WriterFloor {
        operations: 11,
        next_value: 19,
        ..before
    };
    assert!(namespace_matches(before, after, 2));
    assert!(!namespace_matches(before, after, 0));
    assert!(!namespace_matches(
        before,
        WriterFloor {
            next_value: 2,
            ..after
        },
        2
    ));
    assert!(!namespace_matches(
        before,
        WriterFloor {
            next_argument: 6,
            ..after
        },
        2
    ));
    assert!(!namespace_matches(
        before,
        WriterFloor {
            index_rows: 9,
            ..after
        },
        2
    ));
}
#[test]
fn genuine_seed_namespace_overflow_refuses() {
    let before = WriterFloor {
        operations: usize::MAX,
        next_value: u32::MAX,
        next_argument: 1,
        index_rows: 1,
        slice_rows: 1,
    };
    assert!(!namespace_matches(before, before, 1));
    assert!(namespace_matches(before, before, 0));
}
#[test]
fn genuine_seed_current_scalar_identity_and_content_both_matter() {
    let counts = [0_u8, 1, 2];
    let escaped = [false, true, false];
    let equal_counts = [0_u8, 1, 2];
    let equal_escaped = [false, true, false];
    let ck = (counts.as_ptr() as usize, counts.len());
    let ek = (escaped.as_ptr() as usize, escaped.len());
    assert!(counts_and_escape_match(
        ck,
        ek,
        &counts,
        &escaped,
        &equal_counts,
        &equal_escaped
    ));
    assert!(!counts_and_escape_match(
        ck,
        ek,
        &equal_counts,
        &escaped,
        &counts,
        &escaped
    ));
    assert!(!counts_and_escape_match(
        ck,
        ek,
        &counts,
        &equal_escaped,
        &counts,
        &escaped
    ));
    assert!(!counts_and_escape_match(
        ck,
        ek,
        &counts[..2],
        &escaped,
        &counts[..2],
        &escaped
    ));
    assert!(!counts_and_escape_match(
        ck,
        ek,
        &counts,
        &escaped,
        &[0, 1, 3],
        &escaped
    ));
    assert!(!counts_and_escape_match(
        ck,
        ek,
        &counts,
        &escaped,
        &counts,
        &[true, true, false]
    ));
}

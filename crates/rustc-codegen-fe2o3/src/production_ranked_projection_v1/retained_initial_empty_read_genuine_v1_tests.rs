//! Genuine-source empty-read writer observation only; never a nonempty oracle.
use super::*;
#[derive(Clone, Copy, Debug)]
pub(in super::super) struct EmptyReadObservation {
    pub(in super::super) blocks: usize,
    pub(in super::super) read_views: usize,
    pub(in super::super) projected_rows: usize,
    pub(in super::super) prefix_operations: usize,
    pub(in super::super) next_value: u32,
    pub(in super::super) next_argument: usize,
    pub(in super::super) lookup_visits: usize,
}
pub(in super::super) struct EmptyReadOracle {
    projected: Vec<Option<GuardedRankedAccessV1>>,
    operations: Vec<ProductionRankedOperationV1>,
    next_value: u32,
    next_argument: usize,
    classified: bool,
}
impl EmptyReadOracle {
    pub(in super::super) fn new() -> Self {
        Self {
            projected: Vec::new(),
            operations: Vec::new(),
            next_value: 0,
            next_argument: 1,
            classified: false,
        }
    }
}
fn require_empty(
    effects: &[Option<ProjectedReadViewAccessV1>],
    resources: &mut Prep<'_, '_>,
) -> BResult<usize> {
    resources.work(effects.len())?;
    if effects.iter().any(Option::is_some) {
        return Err(Backend::Incomplete(
            "genuine initial-read observer requires an all-None read-view profile",
        ));
    }
    Ok(effects.len())
}
/// Both pending and oracle are physically held by the enclosing run, outside
/// with_checked_nominal_facts_observation_v1 and its error/panic postflight.
#[allow(clippy::too_many_arguments)]
pub(in super::super) fn observe_in_scope<'s>(
    pending: &mut PendingWholeRootBeforeArgumentWritersV1<'s>,
    oracle: &mut EmptyReadOracle,
    original_indices: &mut [Option<u32>],
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<EmptyReadObservation> {
    pending.completed_for(owner, function_id, facts, owned)?;
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
    let (constants, preparation) =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            Ok((
                pending.constants.completed_for(function, resources)?,
                pending.earlier.view(&source, resources)?,
            ))
        })?;
    let inputs = NominalCapabilityInputsV1::from_borrowed_source_v1(
        function,
        preparation.enum_dominance(),
        preparation.allocation_contracts(),
        constants,
    );
    let driver = &pending.driver;
    let mut borrowed = None;
    with_nominal_capability_consumer_v1(facts, |site, consumer| {
        borrowed = Some(driver.completed_for(site, &inputs, consumer)?.read_views);
        Ok(())
    })
    .map_err(nominal_error)?;
    let effects = borrowed.ok_or_else(accounting)?;
    // This lexical block ends every driver/provenance borrow before mutation.
    with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
        let count = require_empty(effects, resources)?;
        if count != function.blocks().len()
            || count > 32
            || oracle.classified
            || !oracle.projected.is_empty()
            || oracle.projected.capacity() != 0
            || !oracle.operations.is_empty()
            || oracle.operations.capacity() != 0
            || original_indices.len() != function.locals().len()
        {
            return Err(accounting());
        }
        oracle.classified = true;
        // Current genuine fixture has no reference-prefix outputs. Verify the
        // actual selected source prefix rather than assuming that from source text.
        resources.work(
            function
                .locals()
                .len()
                .checked_add(2)
                .ok_or_else(arithmetic)?,
        )?;
        let selected = pending.selected.as_ref().ok_or_else(accounting)?;
        let expected = ranked_execution_layout_v1(selected.source_root().layout());
        if !selected.references().is_empty()
            || pending.prefix.reserved_reference_values.is_some()
            || pending.prefix.entry_operations.len() != 1
            || pending.prefix.entry_operations[0] != expected
            || pending.prefix.next_value != 0
            || original_indices.iter().any(Option::is_some)
        {
            return Err(Backend::Incomplete(
                "genuine empty-read observer requires the unchanged execution-only prefix",
            ));
        }
        // ExecutionLayout has no nested allocations. Its independent original
        // operation belongs to oracle before the donor runs.
        resources.push(&mut oracle.operations, expected)?;
        // The unchanged donor sees only None rows, so neither its HashMap nor
        // any view/access nested Vec can allocate. Prepay requested outer rows;
        // attach the actual donor result immediately (not a preallocated stand-in).
        resources.work(count)?;
        resources.reserve_storage(
            count
                .checked_mul(size_of::<Option<GuardedRankedAccessV1>>())
                .ok_or_else(arithmetic)?,
        )?;
        oracle.projected = project_strided_read_effects_v1(
            semantic.types(),
            function,
            effects,
            &preparation.provenance().stable_argument_origins,
            original_indices,
            &mut oracle.next_argument,
            &mut oracle.operations,
            &mut oracle.next_value,
        )?;
        // Any allocator excess is inspected only after attaching the owner and
        // remains held on a failing excess debit, matching the existing oracle policy.
        let excess = oracle
            .projected
            .capacity()
            .checked_sub(count)
            .ok_or_else(accounting)?;
        resources.reserve_storage(
            excess
                .checked_mul(size_of::<Option<GuardedRankedAccessV1>>())
                .ok_or_else(arithmetic)?,
        )?;
        Ok(())
    })?;
    pending
        .prepare_initial_strided_reads(owner, function_id, facts, owned)
        .map_err(nominal_error)?;
    pending.completed_after_initial_strided_reads(owner, function_id, facts, owned)?;
    let observation =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            let rows = pending.initial_reads.projected.len();
            let work = rows
                .checked_add(function.locals().len())
                .and_then(|n| n.checked_add(8))
                .ok_or_else(arithmetic)?;
            resources.work(work)?;
            if !oracle.classified
                || pending.initial_reads.projected != oracle.projected
                || pending.prefix.entry_operations != oracle.operations
                || pending.prefix.next_value != oracle.next_value
                || pending.arguments.runtime_index_arguments.as_slice() != &*original_indices
                || pending.arguments.next_runtime_argument != oracle.next_argument
                || !pending.initial_reads.views.is_empty()
                || pending.initial_reads.pending_view.is_some()
                || pending.initial_reads.lookup_visits != 0
            {
                return Err(accounting());
            }
            Ok(EmptyReadObservation {
                blocks: function.blocks().len(),
                read_views: 0,
                projected_rows: rows,
                prefix_operations: pending.prefix.entry_operations.len(),
                next_value: pending.prefix.next_value,
                next_argument: pending.arguments.next_runtime_argument,
                lookup_visits: pending.initial_reads.lookup_visits,
            })
        })?;
    // Old completion, foreign counter and repeated advance must all refuse.
    // Snapshot before and after proves those negative checks spend nothing.
    let held = facts.retained_whole_root_snapshot_v1(owner, function_id, owned, None)?;
    if pending
        .completed_for(owner, function_id, facts, owned)
        .is_ok()
    {
        return Err(accounting());
    }
    let mut foreign_counter = *owned;
    if pending
        .completed_after_initial_strided_reads(owner, function_id, facts, &mut foreign_counter)
        .is_ok()
    {
        return Err(accounting());
    }
    if pending.prepare_initial_strided_reads(owner, function_id, facts, owned)
        != Err(QueryError::Resource(Resource::Accounting))
    {
        return Err(accounting());
    }
    if pending.phase != WholePhase::InitialStridedReadsTerminal
        || pending
            .completed_after_initial_strided_reads(owner, function_id, facts, owned)
            .is_ok()
    {
        return Err(accounting());
    }
    let after = facts.retained_whole_root_snapshot_v1(owner, function_id, owned, Some(held))?;
    if after != held || foreign_counter != *owned {
        return Err(accounting());
    }
    Ok(observation)
}
pub(in super::super) fn frame() -> BResult<usize> {
    const ROWS: usize = 6;
    let rows = [
        size_of::<(
            EmptyReadOracle,
            EmptyReadObservation,
            Option<EmptyReadObservation>,
            &mut EmptyReadOracle,
            &mut PendingWholeRootBeforeArgumentWritersV1<'static>,
            &mut [Option<u32>],
            &ProductionPreRankedKirOwnerV1,
            SemanticFunctionIdV1,
        )>(),
        size_of::<(
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &mut usize,
            Snapshot,
            Snapshot,
            usize,
            bool,
            Source<'static>,
            NominalCapabilityInputsV1<'static>,
            BeforeCapabilitiesV1<'static>,
        )>(),
        size_of::<(
            &RetainedNominalCapabilityDriverV1,
            Option<&[Option<ProjectedReadViewAccessV1>]>,
            &[Option<ProjectedReadViewAccessV1>],
            &[Option<u32>],
            &[Option<u64>],
            BResult<()>,
            BResult<EmptyReadObservation>,
            Result<()>,
        )>(),
        // Exact empty-donor carriers, including its zero-capacity unallocated map.
        size_of::<(
            HashMap<u64, (ProjectedReadViewV1, ProjectedViewV1)>,
            Vec<Option<GuardedRankedAccessV1>>,
            ProductionRankedOperationV1,
            &mut Vec<ProductionRankedOperationV1>,
            &mut u32,
            &mut usize,
            std::iter::Zip<
                std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
                std::iter::Copied<std::slice::Iter<'static, Option<ProjectedReadViewAccessV1>>>,
            >,
        )>(),
        size_of::<(
            std::slice::Iter<'static, Option<ProjectedReadViewAccessV1>>,
            std::slice::Iter<'static, Option<u32>>,
            usize,
            Option<usize>,
            Backend,
            Resource,
            QueryError,
            std::collections::TryReserveError,
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
    rows.into_iter().try_fold(0usize, |sum, row| {
        sum.checked_add(row).ok_or_else(arithmetic)
    })
}
#[test]
fn genuine_initial_read_empty_classifier_charges_all_rows_before_acceptance() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let effects = [None, None, None];
    for cut in 0..=3 {
        let mut work = Work::new(cut);
        let mut budget = Budget::new(&mut work, 100);
        let mut owned = 0;
        let outcome = require_empty(&effects, &mut Prep::new(&mut budget, &mut owned));
        assert_eq!(outcome.is_ok(), cut == 3);
        assert_eq!(owned, 0);
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn genuine_initial_read_empty_classifier_refuses_nonempty_without_donor_execution() {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    let view = ProjectedReadViewV1 {
        root: 1,
        element: SemanticTypeIdV1::from_index(0),
        allocation: AllocationContractV1 {
            allocation_origin: 1,
            noalias_class: 1,
            writable: false,
            singleton_object: false,
        },
        rows: ProjectedReadValueV1::Constant(1),
        columns: ProjectedReadValueV1::Constant(1),
    };
    let effect = ProjectedReadViewAccessV1 {
        view,
        row: ProjectedReadValueV1::Constant(0),
        column: ProjectedReadValueV1::Constant(0),
    };
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    let mut owned = 0;
    assert!(matches!(
        require_empty(
            &[None, Some(effect), None],
            &mut Prep::new(&mut budget, &mut owned)
        ),
        Err(Backend::Incomplete(_))
    ));
    assert_eq!(budget.work(), 3);
    assert_eq!(owned, 0);
}

//! Separate genuine nonempty-read observation. The historical empty observer
//! remains fail-closed; this entry has no graph/seed or ordinary-route authority.
use super::*;
use retained_original::RetainedOriginalReadsV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) struct NonemptyReadObservation {
    pub(in super::super) locals: usize,
    pub(in super::super) blocks: usize,
    pub(in super::super) effect_block: usize,
    pub(in super::super) read_views: usize,
    pub(in super::super) projected_rows: usize,
    pub(in super::super) prefix_operations: usize,
    pub(in super::super) next_value: u32,
    pub(in super::super) next_argument: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SingleReadProfile {
    block: usize,
    effect: ProjectedReadViewAccessV1,
}
pub(in super::super) struct NonemptyReadOracle {
    original: RetainedOriginalReadsV1,
    entered: bool,
    source: Option<ReadSourceKey>,
    profile: Option<SingleReadProfile>,
    ledger: Option<Snapshot>,
    postflight_paid: bool,
    observation: Option<NonemptyReadObservation>,
}
impl NonemptyReadOracle {
    pub(in super::super) fn new() -> Self {
        Self {
            original: RetainedOriginalReadsV1::new(),
            entered: false,
            source: None,
            profile: None,
            ledger: None,
            postflight_paid: false,
            observation: None,
        }
    }
}
fn require_single(
    effects: &[Option<ProjectedReadViewAccessV1>],
    blocks: usize,
    resources: &mut Prep<'_, '_>,
) -> BResult<SingleReadProfile> {
    if !(1..=32).contains(&blocks) || effects.len() != blocks {
        return Err(Backend::Incomplete(
            "genuine nonempty-read source block profile differs",
        ));
    }
    resources.work(blocks.checked_mul(16).ok_or_else(arithmetic)?)?;
    let mut selected = None;
    for (block, effect) in effects.iter().copied().enumerate() {
        let Some(effect) = effect else {
            continue;
        };
        if selected.is_some()
            || effect.view.allocation.writable
            || effect.view.allocation.noalias_class != 1
            || effect.view.rows != ProjectedReadValueV1::Constant(1)
            || effect.view.columns != ProjectedReadValueV1::Constant(1)
            || effect.row != ProjectedReadValueV1::Constant(0)
            || effect.column != ProjectedReadValueV1::Constant(0)
        {
            return Err(Backend::Incomplete(
                "genuine nonempty-read observer requires one shared constant 1x1 read",
            ));
        }
        selected = Some(SingleReadProfile { block, effect });
    }
    selected.ok_or(Backend::Incomplete(
        "genuine nonempty-read observer found no read-view effect",
    ))
}
fn comparison_work(locals: usize, blocks: usize) -> BResult<usize> {
    // There are THREE complete local-row scans: Option<u32> array equality
    // (including both discriminants and any values), index is_some, and slice
    // extent is_some. Sixteen units per local conservatively cover all three;
    // the fixed debit is reserved for the bounded six-op/single-view payloads,
    // not used to subsidize unbounded local rows.
    locals
        .checked_mul(16)
        .and_then(|n| blocks.checked_mul(16).and_then(|rows| n.checked_add(rows)))
        .and_then(|n| n.checked_add(256))
        .ok_or_else(arithmetic)
}
fn comparison_work_for_two_passes(locals: usize, blocks: usize) -> BResult<usize> {
    comparison_work(locals, blocks)?
        .checked_mul(2)
        .ok_or_else(arithmetic)
}
fn bounded_payload_shapes(
    operations: &[ProductionRankedOperationV1],
    projected: &[Option<GuardedRankedAccessV1>],
    blocks: usize,
    effect_block: usize,
) -> bool {
    if operations.len() != 6 || projected.len() != blocks || effect_block >= blocks {
        return false;
    }
    for (index, operation) in operations.iter().enumerate() {
        let valid = match (index, operation) {
            (0, ProductionRankedOperationV1::ExecutionLayout { .. }) => true,
            (1 | 2 | 4 | 5, ProductionRankedOperationV1::IndexConstant { .. }) => true,
            (
                3,
                ProductionRankedOperationV1::ViewInSpace {
                    shape,
                    dynamic_extents,
                    ..
                },
            ) => shape.len() == 2 && dynamic_extents.len() == 2,
            _ => false,
        };
        if !valid {
            return false;
        }
    }
    for (block, access) in projected.iter().enumerate() {
        if block == effect_block {
            let Some(access) = access else {
                return false;
            };
            if access.indices.len() != 2 || access.comparisons.len() != 2 {
                return false;
            }
        } else if access.is_some() {
            return false;
        }
    }
    true
}
fn compare_rows(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &NonemptyReadOracle,
    function: &SemanticFunctionDeclV1,
) -> BResult<NonemptyReadObservation> {
    let profile = oracle.profile.ok_or_else(accounting)?;
    let expected = oracle.observation.ok_or_else(accounting)?;
    let actual = &pending.initial_reads;
    let original = &oracle.original;
    let count = function.locals().len();
    if !oracle.entered
        || count > 4096
        || function.blocks().len() > 32
        || expected.locals != count
        || expected.blocks != function.blocks().len()
        || expected.effect_block != profile.block
        || expected.read_views != 1
        || expected.projected_rows != expected.blocks
        || expected.prefix_operations != 6
        || expected.next_value != 5
        || expected.next_argument != 1
        || actual.phase != ReadPhase::Complete
        || actual.failure.is_some()
        || actual.source != oracle.source
        || actual.pending_view.is_some()
        || actual.completed_blocks != expected.blocks
        || actual.views.len() != 1
        || original.views.len() != 1
        || actual.lookup_visits != 0
        || actual.final_next_value != 5
        || actual.final_next_argument != 1
        || actual.final_operations != 6
        || pending.prefix.next_value != 5
        || pending.arguments.next_runtime_argument != 1
        || original.next_value != 5
        || original.next_argument != 1
        || pending.prefix.reserved_reference_values.is_some()
        || pending.arguments.runtime_index_arguments.len() != count
        || pending.arguments.runtime_slice_extent_arguments.len() != count
        || original.arguments.len() != count
        || !bounded_payload_shapes(
            &pending.prefix.entry_operations,
            &actual.projected,
            expected.blocks,
            profile.block,
        )
        || !bounded_payload_shapes(
            &original.operations,
            &original.projected,
            expected.blocks,
            profile.block,
        )
    {
        return Err(accounting());
    }
    let row = &actual.views[0];
    let (original_source, original_view) = original
        .views
        .get(&profile.effect.view.root)
        .ok_or_else(accounting)?;
    if row.source != profile.effect.view
        || *original_source != profile.effect.view
        || row.view.shape.len() != 2
        || row.view.dynamic_extents.len() != 2
        || original_view.shape.len() != 2
        || original_view.dynamic_extents.len() != 2
        || row.view != *original_view
        || actual.projected != original.projected
        || pending.prefix.entry_operations != original.operations
        || pending.arguments.runtime_index_arguments != original.arguments
        || pending
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
    Ok(expected)
}

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn observe_in_scope<'s>(
    pending: &mut PendingWholeRootBeforeArgumentWritersV1<'s>,
    oracle: &mut NonemptyReadOracle,
    owner: &'s ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
    owned: &mut usize,
) -> BResult<NonemptyReadObservation> {
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
    let mut borrowed = None;
    let driver = &pending.driver;
    with_nominal_capability_consumer_v1(facts, |site, consumer| {
        borrowed = Some(driver.completed_for(site, &inputs, consumer)?.read_views);
        Ok(())
    })
    .map_err(nominal_error)?;
    let effects = borrowed.ok_or_else(accounting)?;
    with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
        if oracle.entered
            || resources.original_ledger_v1() != Some(source.ledger)
            || resources.has_denial()
        {
            return Err(accounting());
        }
        oracle.entered = true;
        let profile = require_single(effects, function.blocks().len(), resources)?;
        resources.work(
            function
                .locals()
                .len()
                .checked_mul(2)
                .and_then(|n| n.checked_add(32))
                .ok_or_else(arithmetic)?,
        )?;
        if type_width(semantic.types(), profile.effect.view.element)? != 16
            || function.locals().len() > 4096
        {
            return Err(Backend::Incomplete(
                "genuine nonempty-read fixture element or locals differ",
            ));
        }
        let selected = pending.selected.as_ref().ok_or_else(accounting)?;
        let execution = ranked_execution_layout_v1(selected.source_root().layout());
        if !selected.references().is_empty()
            || pending.prefix.reserved_reference_values.is_some()
            || pending.prefix.entry_operations.len() != 1
            || pending.prefix.entry_operations[0] != execution
            || pending.prefix.next_value != 0
            || pending.arguments.next_runtime_argument != 1
            || pending.arguments.runtime_index_arguments.len() != function.locals().len()
            || pending.arguments.runtime_slice_extent_arguments.len() != function.locals().len()
            || pending
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
            return Err(Backend::Incomplete(
                "genuine nonempty-read observer requires the actual execution-only predecessor",
            ));
        }
        let origins = &preparation.provenance().stable_argument_origins;
        oracle.source = Some(ReadSourceKey::new(
            semantic.types(),
            function,
            effects,
            origins,
        ));
        oracle.profile = Some(profile);
        // Original output owners are attached to oracle outside this callback.
        // This is independent legacy-algorithm transcription, not the candidate.
        oracle.original.prepare_into(
            semantic.types(),
            function,
            effects,
            origins,
            Some(&execution),
            resources,
        )?;
        oracle
            .original
            .completed_for(semantic.types(), function, effects, origins, resources)?;
        oracle.observation = Some(NonemptyReadObservation {
            locals: function.locals().len(),
            blocks: function.blocks().len(),
            effect_block: profile.block,
            read_views: 1,
            projected_rows: effects.len(),
            prefix_operations: oracle.original.operations.len(),
            next_value: oracle.original.next_value,
            next_argument: oracle.original.next_argument,
        });
        Ok(())
    })?;
    pending
        .prepare_initial_strided_reads(owner, function_id, facts, owned)
        .map_err(nominal_error)?;
    pending.completed_after_initial_strided_reads(owner, function_id, facts, owned)?;
    let observed =
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            // Both complete bounded DATA comparisons are paid on the actual ledger.
            resources.work(comparison_work_for_two_passes(
                function.locals().len(),
                function.blocks().len(),
            )?)?;
            let observed = compare_rows(pending, oracle, function)?;
            oracle.postflight_paid = true;
            oracle.ledger = resources.retained_custody_snapshot_v1();
            Ok(observed)
        })?;
    let held = facts.retained_whole_root_snapshot_v1(owner, function_id, owned, None)?;
    if pending
        .completed_for(owner, function_id, facts, owned)
        .is_ok()
    {
        return Err(accounting());
    }
    let mut foreign = *owned;
    if pending
        .completed_after_initial_strided_reads(owner, function_id, facts, &mut foreign)
        .is_ok()
        || pending.prepare_initial_strided_reads(owner, function_id, facts, owned)
            != Err(QueryError::Resource(Resource::Accounting))
        || pending.phase != WholePhase::InitialStridedReadsTerminal
        || pending
            .completed_after_initial_strided_reads(owner, function_id, facts, owned)
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

/// Canonical scratch may have been refunded. The enclosing original Custody
/// check owns the storage floor; this consumes only the prepaid DATA comparison.
pub(in super::super) fn postflight(
    pending: &PendingWholeRootBeforeArgumentWritersV1<'_>,
    oracle: &mut NonemptyReadOracle,
    owner: &ProductionPreRankedKirOwnerV1,
    function_id: SemanticFunctionIdV1,
    resources: &Prep<'_, '_>,
) -> BResult<NonemptyReadObservation> {
    let paid = oracle.postflight_paid;
    oracle.postflight_paid = false;
    let held = oracle.ledger.ok_or_else(accounting)?;
    let now = resources
        .retained_custody_snapshot_v1()
        .ok_or_else(accounting)?;
    if !paid
        || pending.failure.is_some()
        || pending.phase != WholePhase::InitialStridedReadsTerminal
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
    let key = oracle.source.ok_or_else(accounting)?;
    if key.function != function as *const _ as usize
        || key.types != (semantic.types().as_ptr() as usize, semantic.types().len())
    {
        return Err(accounting());
    }
    compare_rows(pending, oracle, function)
}

pub(in super::super) fn frame() -> BResult<usize> {
    const ROWS: usize = 7;
    let rows = [
        size_of::<(
            NonemptyReadOracle,
            NonemptyReadObservation,
            Option<NonemptyReadObservation>,
            SingleReadProfile,
            Option<SingleReadProfile>,
            ReadSourceKey,
            Option<ReadSourceKey>,
        )>(),
        size_of::<(
            &mut PendingWholeRootBeforeArgumentWritersV1<'static>,
            &mut NonemptyReadOracle,
            &ProductionPreRankedKirOwnerV1,
            SemanticFunctionIdV1,
            &mut CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>,
            &mut usize,
            Source<'static>,
            Snapshot,
            Snapshot,
            usize,
            bool,
        )>(),
        size_of::<(
            NominalCapabilityInputsV1<'static>,
            BeforeCapabilitiesV1<'static>,
            &RetainedNominalCapabilityDriverV1,
            Option<&[Option<ProjectedReadViewAccessV1>]>,
            &[Option<ProjectedReadViewAccessV1>],
            &[Option<u32>],
            &[Option<u64>],
        )>(),
        size_of::<(
            std::iter::Enumerate<
                std::iter::Copied<std::slice::Iter<'static, Option<ProjectedReadViewAccessV1>>>,
            >,
            std::iter::Enumerate<std::slice::Iter<'static, ProductionRankedOperationV1>>,
            std::iter::Enumerate<std::slice::Iter<'static, Option<GuardedRankedAccessV1>>>,
            std::slice::Iter<'static, Option<u32>>,
            ProjectedReadViewAccessV1,
            ProductionRankedOperationV1,
            &ProductionRankedOperationV1,
            &ReadViewRow,
            Option<&(ProjectedReadViewV1, ProjectedViewV1)>,
        )>(),
        size_of::<(
            &[ProductionRankedOperationV1],
            &[Option<GuardedRankedAccessV1>],
            &SemanticFunctionDeclV1,
            &NonemptyReadOracle,
            &RetainedOriginalReadsV1,
            &RetainedInitialStridedReadV1,
            &ProjectedReadViewV1,
            &ProjectedViewV1,
            &Prep<'static, 'static>,
            &mut Prep<'static, 'static>,
            usize,
            usize,
            usize,
        )>(),
        size_of::<(
            BResult<()>,
            BResult<NonemptyReadObservation>,
            BResult<SingleReadProfile>,
            BResult<usize>,
            Result<()>,
            Backend,
            QueryError,
            Resource,
            Option<usize>,
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

#[cfg(test)]
mod controls {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    fn effect() -> ProjectedReadViewAccessV1 {
        ProjectedReadViewAccessV1 {
            view: ProjectedReadViewV1 {
                root: 11,
                element: SemanticTypeIdV1::from_index(0),
                allocation: AllocationContractV1 {
                    allocation_origin: 7,
                    noalias_class: 1,
                    writable: false,
                    singleton_object: false,
                },
                rows: ProjectedReadValueV1::Constant(1),
                columns: ProjectedReadValueV1::Constant(1),
            },
            row: ProjectedReadValueV1::Constant(0),
            column: ProjectedReadValueV1::Constant(0),
        }
    }
    #[test]
    fn genuine_nonempty_classifier_retains_actual_effect_block_without_invented_ids() {
        for count in [1, 2, 32] {
            for block in 0..count {
                let mut effects = vec![None; count];
                effects[block] = Some(effect());
                let mut work = Work::new(1000);
                let mut budget = Budget::new(&mut work, 1000);
                let mut owned = 0;
                let observed =
                    require_single(&effects, count, &mut Prep::new(&mut budget, &mut owned))
                        .unwrap();
                assert_eq!(observed.block, block);
                assert_eq!(observed.effect, effect());
                assert_eq!(budget.work(), count * 16);
                assert_eq!(owned, 0);
            }
        }
    }
    #[test]
    fn genuine_nonempty_classifier_charges_full_bounded_scan_before_profile() {
        let effects = [None, Some(effect()), None];
        for cut in 0..=48 {
            let mut work = Work::new(cut);
            let mut budget = Budget::new(&mut work, 1000);
            let mut owned = 0;
            let result = require_single(&effects, 3, &mut Prep::new(&mut budget, &mut owned));
            assert_eq!(result.is_ok(), cut == 48);
            assert_eq!(owned, 0);
            assert_eq!(budget.storage(), 0);
        }
    }
    #[test]
    fn genuine_nonempty_classifier_refuses_empty_multiple_and_unclosed_contracts() {
        for mode in 0..10 {
            let mut effects = [None, Some(effect()), None];
            let mut blocks = 3;
            match mode {
                0 => effects[1] = None,
                1 => effects[2] = Some(effect()),
                2 => effects[1].as_mut().unwrap().view.allocation.writable = true,
                3 => effects[1].as_mut().unwrap().view.allocation.noalias_class = 2,
                4 => effects[1].as_mut().unwrap().view.rows = ProjectedReadValueV1::Constant(2),
                5 => effects[1].as_mut().unwrap().view.columns = ProjectedReadValueV1::Constant(0),
                6 => {
                    effects[1].as_mut().unwrap().row =
                        ProjectedReadValueV1::Local(SemanticLocalIdV1::from_index(1))
                }
                7 => effects[1].as_mut().unwrap().column = ProjectedReadValueV1::Constant(1),
                8 => blocks = 2,
                _ => blocks = 33,
            }
            let mut work = Work::new(1000);
            let mut budget = Budget::new(&mut work, 1000);
            let mut owned = 0;
            assert!(
                require_single(&effects, blocks, &mut Prep::new(&mut budget, &mut owned)).is_err()
            );
            assert_eq!(owned, 0);
            assert_eq!(budget.storage(), 0);
        }
    }

    #[test]
    fn genuine_nonempty_comparison_debit_covers_three_local_scans_at_profile_boundaries() {
        for locals in [0usize, 1, 4096] {
            for blocks in [0usize, 1, 32] {
                let single = comparison_work(locals, blocks).unwrap();
                assert_eq!(single, locals * 16 + blocks * 16 + 256);
                // Separate from fixed/payload overhead: conservatively allow
                // eight units for two-sided Option equality and four for each
                // is_some scan, including iteration and the Option test.
                assert!(single - blocks * 16 - 256 >= locals * (8 + 4 + 4));
                assert_eq!(
                    comparison_work_for_two_passes(locals, blocks).unwrap(),
                    single * 2
                );
            }
        }
    }
    #[test]
    fn genuine_nonempty_comparison_debit_checks_each_arithmetic_boundary() {
        let largest_linear = (usize::MAX - 256) / 16;
        for (locals, blocks) in [(largest_linear, 0), (0, largest_linear)] {
            assert_eq!(
                comparison_work(locals, blocks).unwrap(),
                (locals + blocks) * 16 + 256
            );
        }
        // Multiplication, row sum, and final fixed-debit addition each fail
        // closed, without relying on the admitted source-profile maximum.
        for (locals, blocks) in [
            (usize::MAX / 16 + 1, 0),
            (0, usize::MAX / 16 + 1),
            (usize::MAX / 16, 1),
            (largest_linear + 1, 0),
            (0, largest_linear + 1),
        ] {
            assert!(matches!(
                comparison_work(locals, blocks),
                Err(Backend::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
                ))
            ));
            assert!(matches!(
                comparison_work_for_two_passes(locals, blocks),
                Err(Backend::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
                ))
            ));
        }
        let largest_paired = (usize::MAX / 2 - 256) / 16;
        assert_eq!(
            comparison_work_for_two_passes(largest_paired, 0).unwrap(),
            (largest_paired * 16 + 256) * 2
        );
        assert!(comparison_work(largest_paired + 1, 0).is_ok());
        assert!(matches!(
            comparison_work_for_two_passes(largest_paired + 1, 0),
            Err(Backend::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Arithmetic)
            ))
        ));
    }
    #[test]
    fn genuine_nonempty_comparison_prepays_both_passes_before_one_is_allowed() {
        let required = comparison_work_for_two_passes(4096, 1).unwrap();
        for cut in [0usize, required - 1, required, required + 1] {
            let mut work = Work::new(cut);
            let mut budget = Budget::new(&mut work, 1000);
            let mut owned = 0;
            let paid = Prep::new(&mut budget, &mut owned).work(required);
            assert_eq!(paid.is_ok(), cut >= required);
            assert_eq!(owned, 0);
            assert_eq!(budget.storage(), 0);
        }
    }
}

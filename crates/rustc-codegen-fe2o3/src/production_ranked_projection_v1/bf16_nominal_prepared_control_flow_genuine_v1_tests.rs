//! Genuine original-owner CFG checkpoint. No synthetic admission, new frontend,
//! normal route, or replacement positive ledger. The small oracle independently
//! spells ordinary source terminator rows; it is not a full-wave/access proof.
use super::*;
use crate::production_ranked_projection_v1::{
    ProductionRankedProjectionErrorV1, ProjectedCfgTerminatorV1, SemanticAssertMessageV1,
    SemanticFunctionDeclV1, SemanticUnwindActionV1,
    canonical_assertion_facts_v1::{
        CanonicalAssertionErrorV1, NominalPreparedControlFlowV1,
        ProjectedAssertionConditionV1 as Condition, ProjectedAssertionFactsV1,
        with_nominal_canonical_facts_observation_v1, with_nominal_prepared_control_flow_v1,
    },
};

const CFG_HEADERS: usize = 16 * 1024;
const MAX_ROWS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ExpectedRow {
    kind: &'static str,
    count: usize,
    targets: [usize; MAX_ROWS],
}
impl ExpectedRow {
    fn empty(kind: &'static str) -> Self {
        Self {
            kind,
            count: 0,
            targets: [0; MAX_ROWS],
        }
    }
    fn append(&mut self, target: usize, blocks: usize) {
        assert!(target < blocks && blocks <= MAX_ROWS);
        if self.targets[..self.count].contains(&target) {
            return;
        }
        assert!(self.count < MAX_ROWS);
        self.targets[self.count] = target;
        self.count += 1;
    }
    fn branch(target: usize, blocks: usize) -> Self {
        let mut row = Self::empty("branch");
        row.append(target, blocks);
        row
    }
}
fn projection_error(error: ProductionRankedProjectionErrorV1) -> Error {
    match error {
        ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(error),
        ) => Error::Resource(error),
        _ => Error::Unavailable("genuine CFG oracle source query refused"),
    }
}
fn scan_work(edges: usize) -> Result<usize> {
    add(1024, times(edges, MAX_ROWS + 8)?)
}
fn row_matches(actual: &ProjectedCfgTerminatorV1, expected: &ExpectedRow) -> bool {
    let targets = &expected.targets[..expected.count];
    match actual {
        ProjectedCfgTerminatorV1::AbsentMaterialized => {
            expected.kind == "absent" && targets.is_empty()
        }
        ProjectedCfgTerminatorV1::Branch(target) => {
            expected.kind == "branch" && targets == [*target]
        }
        ProjectedCfgTerminatorV1::AnalysisSplit {
            first_block,
            second_block,
        } => expected.kind == "split" && targets == [*first_block, *second_block],
        ProjectedCfgTerminatorV1::AnalysisMultiSplit { blocks } => {
            expected.kind == "multi" && targets == blocks.as_slice()
        }
        ProjectedCfgTerminatorV1::Return => expected.kind == "return" && targets.is_empty(),
        ProjectedCfgTerminatorV1::Trap => expected.kind == "trap" && targets.is_empty(),
        ProjectedCfgTerminatorV1::Predicate { .. } | ProjectedCfgTerminatorV1::ExactSwitch(_) => {
            false
        }
    }
}
// This oracle does not call either shared terminator projector or its switch
// helper. Fixed scratch has no dynamic backing and all source walks are prepaid.
fn expected_row(
    function: &SemanticFunctionDeclV1,
    index: usize,
    callables: &[SemanticCallableDeclV1],
    proved: bool,
    facts: &mut impl ProjectedAssertionFactsV1,
) -> Result<ExpectedRow> {
    use SemanticTerminatorKindV1 as T;
    let blocks = function.blocks().len();
    assert!((1..=MAX_ROWS).contains(&blocks) && index < blocks);
    facts
        .charge_private_array_work(1024)
        .map_err(projection_error)?;
    if !facts
        .is_materialized_block(index)
        .map_err(projection_error)?
    {
        return Ok(ExpectedRow::empty("absent"));
    }
    let block = &function.blocks()[index];
    match block.terminator().kind() {
        T::Goto(edge) | T::Drop { target: edge, .. } => {
            Ok(ExpectedRow::branch(edge.target().index() as usize, blocks))
        }
        T::Call(call) => {
            assert!(!matches!(call.unwind(), SemanticUnwindActionV1::Cleanup(_)));
            if let Some(destination) = call.destination() {
                Ok(ExpectedRow::branch(
                    destination.edge().target().index() as usize,
                    blocks,
                ))
            } else if matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Trap,
                    ..
                })
            ) {
                Ok(ExpectedRow::empty("trap"))
            } else {
                Ok(ExpectedRow::empty("return"))
            }
        }
        T::Assert {
            expected,
            message,
            target,
            ..
        } => {
            if !matches!(message, SemanticAssertMessageV1::BoundsCheck { .. }) {
                let condition = facts
                    .condition(index, *expected, target.target())
                    .map_err(projection_error)?;
                match condition {
                    Condition::Bool(value) => assert_eq!(value, *expected),
                    Condition::Dormant
                    | Condition::Unknown
                    | Condition::Dynamic
                    | Condition::ElidedByExistingRule => assert!(proved),
                }
            }
            Ok(ExpectedRow::branch(
                target.target().index() as usize,
                blocks,
            ))
        }
        T::SwitchInt { targets, .. } => {
            let capacity = targets
                .values()
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?;
            assert!(capacity <= crate::production_ranked_projection_v1::MAX_RANKED_BOUNDS_EDGES);
            facts
                .charge_private_array_work(scan_work(capacity)?)
                .map_err(projection_error)?;
            let otherwise = targets.otherwise().target().index() as usize;
            assert!(otherwise < blocks);
            let fallback = &function.blocks()[otherwise];
            let elided = targets.values().len() == 2
                && targets.values()[0].value() == 0
                && targets.values()[1].value() == 1
                && fallback.statements().is_empty()
                && matches!(fallback.terminator().kind(), T::Unreachable);
            let mut row = ExpectedRow::empty("pending");
            for target in targets.values() {
                row.append(target.edge().target().index() as usize, blocks);
            }
            if !elided {
                row.append(otherwise, blocks);
            }
            row.kind = match row.count {
                0 => panic!("genuine source switch has no successor"),
                1 => "branch",
                2 => "split",
                _ => "multi",
            };
            Ok(row)
        }
        T::FalseEdge { .. } => panic!("genuine source retains an unsupported false edge"),
        T::Return
        | T::TailCall(_)
        | T::UnwindResume
        | T::UnwindTerminate
        | T::Abort
        | T::Unreachable => Ok(ExpectedRow::empty("return")),
    }
}

fn compare_effects(
    view: &NominalPreparedControlFlowV1<'_, '_>,
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let effects = view.effects();
    let original = effects.original();
    // Reuse the old full original-row/Call/Matrix/Return checker, which emits no
    // marker. Do not emit duplicate old prepared-effects markers.
    assert_eq!(
        observe_view(original, owner, source, inventory, budget)?,
        [0, 2, 0, 0]
    );
    budget.charge_work(add(256, times(effects.effects().len(), 128)?)?)?;
    assert!(std::ptr::eq(
        view.assertions().cfg().function(),
        original.function()
    ));
    assert_eq!(effects.source_block(), source.call_block());
    assert_eq!(effects.effects().len(), original.effects().len());
    assert_eq!(view.terminators().len(), original.effects().len());
    assert_ne!(effects.effects().as_ptr(), original.effects().as_ptr());
    let mut counts = [0usize; 4];
    for (index, (actual, baseline)) in effects.effects().iter().zip(original.effects()).enumerate()
    {
        if index == source.call_block().index() as usize {
            assert_eq!(*baseline, ProjectedCapabilityTerminatorEffectsV1::default());
            assert_eq!(
                actual.layout.as_ref(),
                Some(original.candidate().operation())
            );
            assert!(
                actual.global_read.is_none()
                    && actual.transpose_workgroup.is_none()
                    && actual.read_view.is_none()
            );
        } else {
            assert_eq!(actual, baseline);
        }
        for (count, present) in counts.iter_mut().zip([
            actual.layout.is_some(),
            actual.global_read.is_some(),
            actual.transpose_workgroup.is_some(),
            actual.read_view.is_some(),
        ]) {
            *count += usize::from(present);
        }
    }
    assert_eq!(counts, [1, 2, 0, 0]);
    assert_eq!(
        original.candidate().permutation(),
        source.return_permutation()
    );
    Ok(())
}

pub(super) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    original: &mut Budget<'_>,
) -> Result<()> {
    let before = Checkpoint::take(original);
    let entered = Cell::new(0usize);
    let rows_seen = Cell::new(0usize);
    with_headers(original, CFG_HEADERS, |original| {
        // Includes finite marker formatting work; physical stderr machinery is
        // not a whole-action retained-memory measurement.
        original.charge_work(CFG_HEADERS)?;
        with_nominal_prepared_control_flow_v1(
            owner,
            inventory,
            source.root(),
            source.call_block(),
            source.source_call(),
            original,
            |view, budget| {
                entered.set(entered.get() + 1);
                assert!(budget.work_ledger_identity_v1() == before.ledger);
                compare_effects(view, owner, source, inventory, budget)?;
                let function = view.effects().original().function();
                let semantic = owner.semantic_ssa().source_semantic();
                assert!((1..=MAX_ROWS).contains(&function.blocks().len()));
                // Separate original canonical query supplies materialization and
                // actual graph conditions. It uses the SAME positive ledger and
                // original owners; the oracle never invokes the new projector.
                with_nominal_canonical_facts_observation_v1(
                    owner,
                    inventory,
                    source.root(),
                    source.root(),
                    source.call_block(),
                    source.source_call(),
                    budget,
                    |facts| {
                        for (index, actual) in view.terminators().iter().enumerate() {
                            let expected = expected_row(
                                function,
                                index,
                                semantic.callables(),
                                view.assertions().decisions()[index],
                                facts,
                            )?;
                            assert!(
                                row_matches(actual, &expected),
                                "source-indexed CFG row differs"
                            );
                            rows_seen.set(rows_seen.get() + 1);
                            eprintln!(
                                "fe2o3-prepared-nominal-control-flow-row-v1 phase=callback root={} source_call_block={} permutation={:?} source_block={} kind={} successors={:?}",
                                source.root().index(),
                                source.call_block().index(),
                                source.return_permutation(),
                                index,
                                expected.kind,
                                &expected.targets[..expected.count],
                            );
                        }
                        Ok(())
                    },
                )?;
                Ok(())
            },
        )
    })?;
    assert_eq!(entered.get(), 1);
    assert_eq!(
        rows_seen.get(),
        owner.semantic_ssa().source_semantic().functions()[source.root().index() as usize]
            .blocks()
            .len()
    );
    before.require_completed(original)?;
    assert_eq!(
        (original.failed_work(), original.failed_storage()),
        (None, None)
    );
    eprintln!(
        "fe2o3-prepared-nominal-control-flow-complete-v1 root={} source_call_block={} permutation={:?} rows={} callbacks={} layouts=1 global_reads=2 transpose_workgroups=0 read_views=0 work_delta={} storage_restored=true same_ledger=true normal_admission=false",
        source.root().index(),
        source.call_block().index(),
        source.return_permutation(),
        rows_seen.get(),
        entered.get(),
        original
            .work()
            .checked_sub(before.work)
            .ok_or(Resource::Accounting)?,
    );
    Ok(())
}

pub(super) fn controls(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    let before = Checkpoint::take(original);
    let occurrence = owner
        .semantic_ssa()
        .occurrence_storage()
        .ok_or(Error::Unavailable("genuine CFG occurrence storage absent"))?
        .retained_storage();
    let floor = add(
        add(owner.retained_analysis_storage_v1(), occurrence)?,
        inventory_storage,
    )?;
    // Follow the old negative-only probe pattern. Original cumulative work
    // prepays every finite attempt and its scratch. No positive view is ever
    // accepted from a fresh probe and ORIGINAL is never reset or replaced.
    with_headers(original, CFG_HEADERS + PROBE_SCRATCH, |original| {
        for mode in 0..2 {
            original.charge_work(PROBE_WORK + CFG_HEADERS)?;
            let mut work = Work::new(if mode == 0 { 0 } else { PROBE_WORK });
            let mut probe = Budget::new(
                &mut work,
                if mode == 0 {
                    add(floor, PROBE_SCRATCH)?
                } else {
                    floor
                },
            );
            probe.reserve_storage(floor)?;
            let entered = Cell::new(false);
            let result = with_nominal_prepared_control_flow_v1(
                owner,
                inventory,
                source.root(),
                source.call_block(),
                source.source_call(),
                &mut probe,
                |_, _| {
                    entered.set(true);
                    Ok(())
                },
            );
            assert!(!entered.get());
            assert_eq!(probe.storage(), floor);
            if mode == 0 {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                assert!(probe.failed_work().is_some());
                assert!(probe.failed_storage().is_none());
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                assert!(probe.failed_storage().is_some());
                assert!(probe.failed_work().is_none());
            }
            eprintln!(
                "fe2o3-prepared-nominal-control-flow-denial-v1 root={} source_call_block={} permutation={:?} kind={} entered=false storage_restored=true positive_authority=false reason={:?}",
                source.root().index(),
                source.call_block().index(),
                source.return_permutation(),
                if mode == 0 { "work" } else { "storage" },
                result,
            );
        }
        Ok(())
    })?;
    before.require_completed(original)?;
    assert_eq!(
        (original.failed_work(), original.failed_storage()),
        (None, None)
    );
    Ok(())
}

#[test]
fn cfg_oracle_preserves_order_and_only_deduplicates_exact_successors() {
    let mut row = ExpectedRow::empty("multi");
    for target in [3, 1, 3, 2, 1] {
        row.append(target, 4);
    }
    assert_eq!(&row.targets[..row.count], &[3, 1, 2]);
    assert!(row_matches(
        &ProjectedCfgTerminatorV1::AnalysisMultiSplit {
            blocks: vec![3, 1, 2]
        },
        &row
    ));
    assert!(!row_matches(
        &ProjectedCfgTerminatorV1::AnalysisMultiSplit {
            blocks: vec![1, 3, 2]
        },
        &row
    ));
}
#[test]
fn cfg_oracle_rejects_wrong_kind_or_target_not_just_scalar_counts() {
    let row = ExpectedRow::branch(3, 4);
    assert!(row_matches(&ProjectedCfgTerminatorV1::Branch(3), &row));
    assert!(!row_matches(&ProjectedCfgTerminatorV1::Branch(2), &row));
    assert!(!row_matches(&ProjectedCfgTerminatorV1::Return, &row));
    assert!(!row_matches(
        &ProjectedCfgTerminatorV1::Trap,
        &ExpectedRow::empty("return")
    ));
}
#[test]
fn cfg_oracle_fixed_scratch_and_checked_work_fit_named_header() {
    assert!(
        4 * size_of::<ExpectedRow>()
            + 4 * size_of::<Budget<'static>>()
            + 4 * size_of::<Work>()
            + 32 * size_of::<Cell<usize>>()
            + 12 * size_of::<Result<()>>()
            + 8 * size_of::<Checkpoint>()
            + 4096
            <= CFG_HEADERS
    );
    assert_eq!(scan_work(0), Ok(1024));
    assert_eq!(scan_work(3), Ok(1144));
    assert!(matches!(
        scan_work(usize::MAX),
        Err(Error::Resource(Resource::Arithmetic))
    ));
}

use super::super::scalar_emission_fixture_v1_tests as fixture;
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use fe2o3_mir_model::semantic_mir_v1::*;
use std::panic::{AssertUnwindSafe, catch_unwind, panic_any};

#[path = "guarded_source_progress_cast_v1_tests.rs"]
mod cast_tests;
#[path = "multi_entry_guarded_refusal_v1_tests.rs"]
mod multi_entry_tests;
#[path = "guarded_source_progress_resources_v1_tests.rs"]
mod resource_tests;

const WORK: usize = 1_000_000_000;
const STORAGE: usize = 512 * 1024 * 1024;
const FLOOR: usize = 23;
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

#[test]
fn source_bearing_stages_cannot_be_cloned() {
    trait AmbiguousIfClone<A> {
        fn check() {}
    }
    impl<T: ?Sized> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    // Adding Clone makes either inference ambiguous. These are real visible
    // types, not compile-fail examples that could pass through privacy errors.
    let _ = <GuardedRankedSourceV1 as AmbiguousIfClone<_>>::check;
    let _ = <crate::production_pipeline::guarded_loop_source_v1::ProductionGuardedRankedSourceV1
        as AmbiguousIfClone<_>>::check;
}

fn value(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap(),
    )
}
fn constant(bits: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
    ))
}
fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap(),
            SemanticRvalueV1::new(ty, kind),
        )),
    )
}

// Every transform precedes semantic admission/SSA/N. This is component source,
// not a substitute for the separate rustc-driven source qualifier.
fn transform(
    function: &SemanticFunctionDeclV1,
    seed: u8,
    symbol: &[u8],
    snapshot: bool,
    step: u128,
) -> SemanticFunctionDeclV1 {
    let mut locals = function.locals().to_vec();
    let mut blocks = function.blocks().to_vec();
    if snapshot {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([75; 32]),
            U32,
            SemanticLocalRoleV1::Temporary,
            function.source(),
        ));
        let header = &blocks[1];
        blocks[1] = SemanticBasicBlockV1::new(
            header.identity(),
            header.source(),
            vec![
                assign(5, U32, SemanticRvalueKindV1::Use(value(1, U32))),
                assign(
                    3,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: value(2, U32),
                        right: value(5, U32),
                    },
                ),
            ],
            header.terminator().clone(),
        )
        .unwrap();
    }
    if step != 1 {
        let body = &blocks[2];
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            target,
            unwind,
            ..
        } = body.terminator().kind()
        else {
            unreachable!()
        };
        blocks[2] = SemanticBasicBlockV1::new(
            body.identity(),
            body.source(),
            vec![assign(
                4,
                PAIR,
                SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                    SemanticCheckedBinaryOpV1::Add,
                    value(2, U32),
                    constant(step),
                )),
            )],
            SemanticTerminatorV1::new(
                body.terminator().source(),
                SemanticTerminatorKindV1::Assert {
                    condition: condition.clone(),
                    expected: *expected,
                    message: SemanticAssertMessageV1::Overflow {
                        operation: SemanticBinaryOpV1::Add,
                        left: value(2, U32),
                        right: constant(step),
                    },
                    target: *target,
                    unwind: *unwind,
                },
            ),
        )
        .unwrap();
    }
    SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([seed; 32]),
        function.role(),
        SemanticItemDefinitionIdentityV1::from_sha256([seed; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([seed; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([seed; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([seed; 32]),
        function.source(),
        function.abi().clone(),
        locals,
        function.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(symbol.to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([seed; 32]),
        function.kernel_entry().unwrap().source_contract(),
    ))
}

fn target(alternate_layout_identity: bool) -> SemanticTargetDataLayoutV1 {
    // The frozen model has one architecture variant. These are distinct
    // component layout identities, not real gfx942/gfx950 profile coverage.
    let tag = if alternate_layout_identity { 251 } else { 250 };
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([tag; 32]))
}
fn capture_with(
    budget: &mut Budget<'_>,
    alternate_layout_identity: bool,
    snapshot: bool,
    count: usize,
    step: u128,
) -> Capture {
    let (ssa, launch) = fixture::source_with(30, target(alternate_layout_identity), |function| {
        (0..count)
            .map(|index| {
                transform(
                    &function,
                    30 + index as u8,
                    format!("renamed_generic_instance_{index}").as_bytes(),
                    snapshot,
                    step,
                )
            })
            .collect()
    });
    Capture::try_materialize_with_budget_v1(ssa, launch, Default::default(), budget).unwrap()
}
fn inputs(count: usize) -> Vec<ProductionRankedRootInputV1> {
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    (0..count)
        .map(|index| {
            ProductionRankedRootInputV1::new(
                &if index == 0 {
                    "component".to_owned()
                } else {
                    format!("component_{index}")
                },
                [30 + index as u8; 32],
                &launch,
            )
        })
        .collect()
}
fn project(
    budget: &mut Budget<'_>,
    alternate_layout_identity: bool,
    snapshot: bool,
    count: usize,
) -> GuardedRankedSourceV1 {
    let capture = capture_with(budget, alternate_layout_identity, snapshot, count, 1);
    budget
        .reserve_storage(capture.retained_analysis_storage_v1())
        .unwrap();
    GuardedRankedSourceV1::try_project_v1(
        capture,
        &inputs(count),
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
        budget,
    )
    .unwrap()
}
fn release(stage: GuardedRankedSourceV1, budget: &mut Budget<'_>) {
    let receipt = stage.retained_storage();
    drop(stage);
    budget.release_storage(receipt).unwrap();
}

#[test]
fn original_fixture_identity_wrapper_preserves_source_ssa_and_n() {
    let (old, old_launch) = fixture::source(30);
    let (new, new_launch) = fixture::source_with(30, target(false), |function| vec![function]);
    assert_eq!(
        old.source_semantic().semantic_sha256(),
        new.source_semantic().semantic_sha256()
    );
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let a =
        Capture::try_materialize_with_budget_v1(old, old_launch, Default::default(), &mut budget)
            .unwrap();
    budget
        .reserve_storage(a.retained_analysis_storage_v1())
        .unwrap();
    let b =
        Capture::try_materialize_with_budget_v1(new, new_launch, Default::default(), &mut budget)
            .unwrap();
    assert_eq!(
        a.original().executable().canonical().identity(),
        b.original().executable().canonical().identity()
    );
    assert_eq!(
        a.original().executable().canonical().canonical_bytes(),
        b.original().executable().canonical().canonical_bytes()
    );
}

#[test]
fn strict_owning_projection_consumes_every_actual_site_and_replays_both_layout_identities() {
    assert_ne!(target(false).identity(), target(true).identity());
    for alternate_layout_identity in [false, true] {
        for snapshot in [false, true] {
            for count in [1, 2] {
                let mut work = Work::new(WORK);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.reserve_storage(FLOOR).unwrap();
                let stage = project(&mut budget, alternate_layout_identity, snapshot, count);
                let source = stage.capture().original().semantic_ssa().source_semantic();
                assert_eq!(source.target(), target(alternate_layout_identity));
                for function in source.functions() {
                    assert_eq!(
                        function.abi().layout_identity(),
                        source.target_layout_identity()
                    );
                }
                assert_eq!(stage.roots().len(), count);
                assert_eq!(stage.sites().len(), count);
                assert_eq!(budget.storage(), FLOOR + stage.retained_storage());
                for (index, site) in stage.sites().iter().enumerate() {
                    assert!(site.consumed);
                    assert_eq!(
                        (site.root(), site.function(), site.block(), site.statement()),
                        (index as u32, index as u32, 2, 0)
                    );
                    assert_eq!(stage.certificate_count(index), Some(1));
                    assert_eq!(site.certificate_ordinal(), 0);
                }
                assert!(!stage.grants_authority());
                let before = budget.storage();
                stage.replay_consistency_v1(&mut budget).unwrap();
                assert_eq!(budget.storage(), before);
                release(stage, &mut budget);
                assert_eq!(budget.storage(), FLOOR);
            }
        }
    }
}

#[test]
fn root_binding_substitution_consumes_only_its_owned_receipt() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let captured = capture_with(&mut budget, false, false, 1, 1);
    budget
        .reserve_storage(captured.retained_analysis_storage_v1())
        .unwrap();
    let mut roots = inputs(1);
    roots[0].kernel_binding = [99; 32];
    assert!(
        GuardedRankedSourceV1::try_project_v1(
            captured,
            &roots,
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn unsupported_dynamic_u32_step_is_not_counted_as_guarded_progress() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let captured = capture_with(&mut budget, false, false, 1, 2);
    let (reports, _) = derive_reports(&captured, &mut budget).unwrap();
    assert!(reports[0].report.certificates().is_empty());
    drop(reports);
    budget.release_storage(budget.storage() - FLOOR).unwrap();
    budget
        .reserve_storage(captured.retained_analysis_storage_v1())
        .unwrap();
    assert!(
        GuardedRankedSourceV1::try_project_v1(
            captured,
            &inputs(1),
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn replay_refuses_changed_consumption_root_report_and_extra_or_missing_rows() {
    for mode in 0..7 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let mut stage = project(&mut budget, false, true, 1);
        // Test-only hostile rows are separately prepaid before mutation.
        let mut extra = 0;
        match mode {
            0 => stage.sites[0].consumed = false,
            1 => stage.sites[0].key.statement += 1,
            2 => stage.sites[0].ordinal += 1,
            3 => stage.reports[0].root = SemanticFunctionIdV1::from_index(1),
            4 => stage.roots[0].semantic_root = SemanticFunctionIdV1::from_index(1),
            5 => {
                stage.sites.clear();
            }
            _ => {
                let mut replacement =
                    resources::table::<ConsumedProgressSiteV1>(2, &mut budget).unwrap();
                replacement.extend([stage.sites[0], stage.sites[0]]);
                extra = resources::bytes::<ConsumedProgressSiteV1>(replacement.capacity()).unwrap();
                let old = std::mem::replace(&mut stage.sites, replacement);
                // Keep original vector reservation until the stage drop too.
                drop(old);
            }
        }
        let before = budget.storage();
        assert!(stage.replay_consistency_v1(&mut budget).is_err());
        assert_eq!(budget.storage(), before);
        release(stage, &mut budget);
        budget.release_storage(extra).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn foreign_equal_bytes_owner_cannot_supply_a_guard_report() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let a = capture_with(&mut budget, false, false, 1, 1);
    budget
        .reserve_storage(a.retained_analysis_storage_v1())
        .unwrap();
    let b = capture_with(&mut budget, false, false, 1, 1);
    budget
        .reserve_storage(b.retained_analysis_storage_v1())
        .unwrap();
    assert_eq!(
        a.original().executable().canonical().canonical_bytes(),
        b.original().executable().canonical().canonical_bytes()
    );
    let (reports, _) = derive_reports(&a, &mut budget).unwrap();
    let (guard, _) = derive_guard(&a, &reports, &mut budget).unwrap();
    assert!(matches!(
        derive_sites(&b, &reports, &guard, &mut budget),
        Err(ProductionRankedProjectionErrorV1::Incomplete(
            "guarded progress has a foreign original C owner"
        ))
    ));
}

#[test]
fn replay_rejects_budget_move_replacement_and_missing_receipt_before_work() {
    let mut work = Work::new(WORK);
    let mut foreign_work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let stage = project(&mut budget, false, false, 1);
    let receipt = stage.retained_storage();
    budget.release_storage(1).unwrap();
    assert!(budget.storage() >= receipt);
    let before = (budget.work(), budget.storage());
    assert!(matches!(
        stage.replay_consistency_v1(&mut budget),
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Accounting)
        ))
    ));
    assert_eq!((budget.work(), budget.storage()), before);
    budget.reserve_storage(1).unwrap();
    let mut foreign = Budget::new(&mut foreign_work, STORAGE);
    foreign.reserve_storage(FLOOR + receipt).unwrap();
    std::mem::swap(&mut budget, &mut foreign);
    let before = (budget.work(), budget.storage());
    assert!(stage.replay_consistency_v1(&mut budget).is_err());
    assert_eq!((budget.work(), budget.storage()), before);
    // The original ledger is also refused in a different Budget slot.
    let before = (foreign.work(), foreign.storage());
    assert!(stage.replay_consistency_v1(&mut foreign).is_err());
    assert_eq!((foreign.work(), foreign.storage()), before);
    drop(stage);
    foreign.release_storage(receipt).unwrap();
    assert_eq!(foreign.storage(), FLOOR);
}

fn resource_error(error: &(dyn std::error::Error + 'static)) -> Option<Resource> {
    if let Some(value) = error.downcast_ref::<Resource>() {
        return Some(*value);
    }
    if let Some(value) = error.downcast_ref::<CaptureError>() {
        return match value {
            CaptureError::Resource(value)
            | CaptureError::Inventory(
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1::Resource(value),
            )
            | CaptureError::Loops(fe2o3_kernel_analysis::CanonicalKirLoopErrorV1::Resource(
                value,
            )) => Some(*value),
            _ => None,
        };
    }
    error.source().and_then(resource_error)
}
fn require_short<T>(result: Result<T>, storage: bool) {
    let error = match result {
        Err(error) => error,
        Ok(_) => panic!("one-short limit succeeded"),
    };
    assert!(
        matches!(
            (resource_error(&error), storage),
            (Some(Resource::Storage(_)), true) | (Some(Resource::Work(_)), false)
        ),
        "{error:?}"
    );
}

#[test]
fn projection_has_exact_and_one_short_cumulative_live_resources() {
    let run = |limit, capacity| {
        let mut setup_work = Work::new(WORK);
        let mut setup = Budget::new(&mut setup_work, STORAGE);
        let capture = capture_with(&mut setup, false, true, 2, 1);
        let incoming = capture.retained_analysis_storage_v1();
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, capacity);
        budget.reserve_storage(FLOOR + incoming).unwrap();
        let result = GuardedRankedSourceV1::try_project_v1(
            capture,
            &inputs(2),
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            &mut budget,
        );
        let spent = budget.work();
        let peak = budget.peak_storage();
        let result = result.map(|stage| release(stage, &mut budget));
        assert_eq!(budget.storage(), FLOOR);
        (result, spent, peak)
    };
    let (result, spent, peak) = run(WORK, STORAGE);
    result.unwrap();
    run(spent, peak).0.unwrap();
    require_short(run(spent - 1, peak).0, false);
    require_short(run(spent, peak - 1).0, true);
}

#[test]
#[allow(clippy::drop_non_drop)] // Ends the old borrow before reinitializing the same Budget slot.
fn replay_has_exact_and_one_short_shared_ledger_resources() {
    let run = |limit, capacity| {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let stage = project(&mut budget, false, true, 1);
        let before = budget.storage();
        // Test-only ceiling change: preserve the SAME Work and Budget place,
        // reconstruct only the storage ceiling with the entire live receipt.
        drop(budget);
        budget = Budget::new(&mut work, capacity);
        budget.reserve_storage(before).unwrap();
        let result = stage.replay_consistency_v1(&mut budget);
        assert_eq!(budget.storage(), before);
        let spent = budget.work();
        let peak = budget.peak_storage();
        release(stage, &mut budget);
        assert_eq!(budget.storage(), FLOOR);
        (result, spent, peak)
    };
    let (result, spent, peak) = run(WORK, STORAGE);
    result.unwrap();
    run(spent, peak).0.unwrap();
    require_short(run(spent - 1, peak).0, false);
    require_short(run(spent, peak - 1).0, true);
}

#[test]
fn temporary_input_floor_transfer_is_exact_and_preserves_unrelated_storage() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let temporary = 17;
    budget.reserve_storage(temporary).unwrap();
    let mut stage = project(&mut budget, false, true, 1);
    let old = stage.required_floor;
    assert!(stage.release_input_floor_v1(&budget, temporary).is_err());
    assert_eq!(stage.required_floor, old);
    budget.release_storage(temporary).unwrap();
    assert!(
        stage
            .release_input_floor_v1(&budget, temporary - 1)
            .is_err()
    );
    assert_eq!(stage.required_floor, old);
    stage.release_input_floor_v1(&budget, temporary).unwrap();
    assert_eq!(stage.required_floor, FLOOR + stage.retained_storage());
    stage.replay_consistency_v1(&mut budget).unwrap();
    budget.release_storage(1).unwrap();
    assert!(budget.storage() >= stage.retained_storage());
    let before = (budget.work(), budget.storage());
    assert!(stage.replay_consistency_v1(&mut budget).is_err());
    assert_eq!((budget.work(), budget.storage()), before);
    budget.reserve_storage(1).unwrap();
    release(stage, &mut budget);
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn contradictory_actual_bool_remains_stronger_than_source_progress() {
    use super::super::canonical_assertion_facts_v1::projected_assertion_is_proved_v1;
    assert!(!projected_assertion_is_proved_v1(
        ProjectedAssertionConditionV1::Bool(true),
        false,
        true
    ));
    assert!(projected_assertion_is_proved_v1(
        ProjectedAssertionConditionV1::Bool(false),
        false,
        true
    ));
}

// This isolated meter deliberately supplies no graph proof. The tests below
// exercise only the new candidate join with independently constructed genuine
// source/C/guard rows. Full projector/arithmetic precedence is tested above.
struct Facts<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0.charge_work(amount).map_err(resource)
    }
    fn helper_value_ledger_v1(
        &self,
    ) -> Result<(
        usize,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    )> {
        Ok((
            self.0 as *const _ as usize,
            self.0.work_ledger_identity_v1(),
        ))
    }
    fn scalar_private_storage_v1(&self) -> Result<usize> {
        Ok(self.0.storage())
    }
    fn reserve_scalar_private_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.0.reserve_storage(amount).map_err(resource)
    }
    fn release_scalar_private_storage_v1(&mut self, amount: usize) -> Result<()> {
        self.0.release_storage(amount).map_err(resource)
    }
    fn private_array_initializer_count(&mut self, _: usize, _: usize) -> Result<Option<u64>> {
        Ok(None)
    }
    fn is_materialized_block(&mut self, _: usize) -> Result<bool> {
        Ok(true)
    }
    fn condition(
        &mut self,
        _: usize,
        _: bool,
        _: SemanticBlockIdV1,
    ) -> Result<ProjectedAssertionConditionV1> {
        Ok(ProjectedAssertionConditionV1::Dynamic)
    }
}

fn candidate(snapshot: bool) -> ProjectedUniformInductionV1 {
    let bound = ProductionRankedValueV1::Argument(0);
    let step = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(1));
    ProjectedUniformInductionV1 {
        initializer_block: 0,
        preheader_control: ProjectedInductionPreheaderControlV1::Direct,
        header: 1,
        body_entry: 2,
        latch: 3,
        exit: 4,
        loop_blocks: vec![1, 2, 3],
        initial: ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0)),
        bound,
        step,
        source_progress: ProjectedSourceInductionCandidateV1 {
            induction: SemanticLocalIdV1::from_index(2),
            induction_type: U32,
            header_statement: usize::from(snapshot),
            bound_operand: value(if snapshot { 5 } else { 1 }, U32),
            latch_statement: 0,
            update: ProjectedSourceInductionUpdateV1::Checked {
                producer_block: 2,
                producer_statement: 0,
                result_local: SemanticLocalIdV1::from_index(4),
            },
            step_operand: constant(1),
            step_value: 1,
            ranked_bound: bound,
            ranked_step: step,
        },
        bound_cast: None,
        body_predicates: vec![],
    }
}

fn with_candidate(
    action: impl FnOnce(
        &mut GuardedSourceProgressV1<'_, '_>,
        &mut Facts<'_, '_>,
        &[SemanticTypeDeclV1],
        &SemanticFunctionDeclV1,
    ),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = capture_with(&mut budget, false, true, 1, 1);
    budget
        .reserve_storage(capture.retained_analysis_storage_v1())
        .unwrap();
    let (reports, _) = derive_reports(&capture, &mut budget).unwrap();
    let (guards, _) = derive_guard(&capture, &reports, &mut budget).unwrap();
    let mut sites = derive_sites(&capture, &reports, &guards, &mut budget).unwrap();
    {
        let source = capture.original().semantic_ssa().source_semantic();
        let mut progress = GuardedSourceProgressV1 {
            capture: &capture,
            reports: &reports,
            guards: &guards,
            sites: &mut sites,
            slot: &mut budget as *mut _ as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
        };
        action(
            &mut progress,
            &mut Facts(&mut budget),
            source.types(),
            &source.functions()[0],
        );
    }
    drop(sites);
    drop(guards);
    drop(reports);
    drop(capture);
    budget.release_storage(budget.storage() - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn exact_candidate_is_consumed_once_and_omission_is_not_success() {
    with_candidate(|progress, facts, types, function| {
        assert!(progress.finish(facts.0).is_err());
        progress
            .with_source(
                0,
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                facts,
                |facts| {
                    facts.require_guarded_source_progress_v1(types, function, &candidate(true), &[])
                },
            )
            .unwrap();
        progress.finish(facts.0).unwrap();
        assert!(
            progress
                .with_source(
                    0,
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(0),
                    facts,
                    |facts| facts.require_guarded_source_progress_v1(
                        types,
                        function,
                        &candidate(true),
                        &[],
                    )
                )
                .is_err()
        );
    });
}

#[test]
fn substituted_actual_candidate_coordinates_operands_and_ranked_values_are_refused() {
    for mode in 0..16 {
        with_candidate(|progress, facts, types, function| {
            let mut candidate = candidate(true);
            match mode {
                0 => candidate.source_progress.induction = SemanticLocalIdV1::from_index(1),
                1 => candidate.source_progress.header_statement = 0,
                2 => candidate.source_progress.bound_operand = value(1, U32),
                3 => candidate.source_progress.latch_statement = 1,
                4 => candidate.source_progress.update = ProjectedSourceInductionUpdateV1::Ordinary,
                5 => {
                    candidate.source_progress.update = ProjectedSourceInductionUpdateV1::Checked {
                        producer_block: 2,
                        producer_statement: 1,
                        result_local: SemanticLocalIdV1::from_index(4),
                    }
                }
                6 => {
                    candidate.source_progress.update = ProjectedSourceInductionUpdateV1::Checked {
                        producer_block: 2,
                        producer_statement: 0,
                        result_local: SemanticLocalIdV1::from_index(3),
                    }
                }
                7 => candidate.source_progress.step_operand = constant(2),
                8 => candidate.source_progress.step_value = 2,
                9 => candidate.bound = ProductionRankedValueV1::Argument(9),
                10 => candidate.step = ProductionRankedValueV1::Argument(9),
                11 => candidate.initializer_block = 4,
                12 => candidate.body_entry = 3,
                13 => candidate.exit = 3,
                14 => {
                    candidate.source_progress.bound_operand = SemanticOperandV1::Move(
                        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(5), vec![], U32)
                            .unwrap(),
                    )
                }
                _ => candidate.source_progress.update = ProjectedSourceInductionUpdateV1::Unchecked,
            }
            let result = progress.with_source(
                0,
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                facts,
                |facts| facts.require_guarded_source_progress_v1(types, function, &candidate, &[]),
            );
            assert!(result.is_err(), "candidate mutation {mode}");
            assert!(!progress.sites[0].consumed);
            assert!(progress.finish(facts.0).is_err());
        });
    }
}

#[test]
fn equal_value_foreign_function_or_types_and_changed_root_selection_are_refused() {
    with_candidate(|progress, facts, types, function| {
        let cloned = function.clone();
        let cloned_types = types.to_vec();
        for (actual_types, actual_function) in
            [(types, &cloned), (cloned_types.as_slice(), function)]
        {
            assert!(
                progress
                    .with_source(
                        0,
                        SemanticFunctionIdV1::from_index(0),
                        SemanticFunctionIdV1::from_index(0),
                        facts,
                        |facts| facts.require_guarded_source_progress_v1(
                            actual_types,
                            actual_function,
                            &candidate(true),
                            &[],
                        )
                    )
                    .is_err()
            );
        }
        assert!(
            progress
                .with_source(
                    0,
                    SemanticFunctionIdV1::from_index(1),
                    SemanticFunctionIdV1::from_index(0),
                    facts,
                    |_| -> Result<()> { panic!("foreign root reached candidate") }
                )
                .is_err()
        );
        assert!(!progress.sites[0].consumed);
    });
}

#[test]
fn absent_u32_join_and_non_u32_claim_never_consume_a_certificate() {
    with_candidate(|progress, facts, types, function| {
        let mut changed = candidate(true);
        changed.source_progress.induction_type = BOOL;
        progress
            .with_source(
                0,
                SemanticFunctionIdV1::from_index(0),
                SemanticFunctionIdV1::from_index(0),
                facts,
                |facts| facts.require_guarded_source_progress_v1(types, function, &changed, &[]),
            )
            .unwrap();
        assert!(!progress.sites[0].consumed);
        assert!(progress.finish(facts.0).is_err());
        let mut empty = GuardedSourceProgressV1 {
            capture: progress.capture,
            reports: progress.reports,
            guards: progress.guards,
            sites: &mut [],
            slot: progress.slot,
            ledger: progress.ledger,
            floor: progress.floor,
        };
        assert!(
            empty
                .with_source(
                    0,
                    SemanticFunctionIdV1::from_index(0),
                    SemanticFunctionIdV1::from_index(0),
                    facts,
                    |facts| facts.require_guarded_source_progress_v1(
                        types,
                        function,
                        &candidate(true),
                        &[],
                    )
                )
                .is_err()
        );
        assert!(!progress.sites[0].consumed);
    });
}

#[test]
fn legacy_none_route_keeps_exact_ranked_bytes_work_and_source_report() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let capture = capture_with(&mut budget, false, true, 1, 1);
    budget
        .reserve_storage(capture.retained_analysis_storage_v1())
        .unwrap();
    let source = RankedProjectionSourceV1::from_materialized_checked(capture.original()).unwrap();
    let roots = inputs(1);
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    let floor = budget.storage();
    let start = budget.work();
    let historical = project_ranked_roots_v1(&source, &roots, &references, &mut budget).unwrap();
    let old_work = budget.work() - start;
    let start = budget.work();
    let explicit_none =
        project_ranked_roots_with_progress_v1(&source, &roots, &references, &mut budget, None)
            .unwrap()
            .finish(&source, &mut budget)
            .unwrap();
    assert_eq!(budget.work() - start, old_work);
    assert_eq!(budget.storage(), floor);
    assert_eq!(historical[0].ranked_ir, explicit_none[0].ranked_ir);
    assert_eq!(
        historical[0].semantic_u32_induction,
        explicit_none[0].semantic_u32_induction
    );
    // Two projections exist only in this parity test, never in the new stage.
    let strict =
        GuardedRankedSourceV1::try_project_v1(capture, &roots, &references, &mut budget).unwrap();
    assert_eq!(historical[0].ranked_ir, strict.roots[0].ranked_ir);
    assert_eq!(
        historical[0].semantic_u32_induction,
        strict.roots[0].semantic_u32_induction
    );
    release(strict, &mut budget);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn foreign_retained_report_is_not_equivalent_source_custody() {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let mut stage = project(&mut budget, false, true, 1);
    let (ssa, launch) = fixture::source(31);
    let foreign =
        Capture::try_materialize_with_budget_v1(ssa, launch, Default::default(), &mut budget)
            .unwrap();
    budget
        .reserve_storage(foreign.retained_analysis_storage_v1())
        .unwrap();
    let (mut reports, _) = derive_reports(&foreign, &mut budget).unwrap();
    std::mem::swap(&mut stage.reports[0].report, &mut reports[0].report);
    let floor = budget.storage();
    assert!(stage.replay_consistency_v1(&mut budget).is_err());
    assert_eq!(budget.storage(), floor);
    drop((reports, foreign, stage));
    budget.release_storage(budget.storage() - FLOOR).unwrap();
}

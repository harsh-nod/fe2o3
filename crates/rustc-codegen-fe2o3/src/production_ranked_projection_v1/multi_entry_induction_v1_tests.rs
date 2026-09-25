use super::*;
use canonical_assertion_facts_v1::ProjectedAssertionConditionV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticBasicBlockV1, SemanticStatementV1,
};

struct Facts<'a, 'w>(&'a mut Budget<'w>);
impl ProjectedAssertionFactsV1 for Facts<'_, '_> {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0.charge_work(amount).map_err(resource)
    }
    fn helper_value_ledger_v1(&self) -> Result<(usize, Ledger)> {
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

// Deliberately retain the trait-default missing storage/identity custody.
struct NoCustodyFacts(usize);
impl ProjectedAssertionFactsV1 for NoCustodyFacts {
    fn charge_private_array_work(&mut self, amount: usize) -> Result<()> {
        self.0 += amount;
        Ok(())
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

fn fixture() -> SemanticFunctionDeclV1 {
    super::super::tests::multi_entry_source_fixture_v1()
}

fn rebuilt(
    function: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    SemanticFunctionDeclV1::new(
        function.identity(),
        function.role(),
        function.item_definition_identity(),
        function.monomorphization_identity(),
        function.generic_type_arguments_identity(),
        function.const_generic_arguments_identity(),
        function.source(),
        function.abi().clone(),
        function.locals().to_vec(),
        function.entry(),
        blocks,
    )
    .unwrap()
}

fn graph(function: &SemanticFunctionDeclV1) -> ProjectedLoopCfgV1 {
    projected_loop_cfg_graph_v1(function).unwrap()
}

fn make(
    function: &SemanticFunctionDeclV1,
    scope: &mut Scope,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<Box<Entries>> {
    build(
        function,
        &graph(function),
        &[true, true, true, false, false, false],
        3,
        4,
        SemanticLocalIdV1::from_index(1),
        &[1, 2],
        &mut Context { scope, facts },
    )
}

fn checked_component(
    function: &SemanticFunctionDeclV1,
    scope: &mut Scope,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<()> {
    let proof = make(function, scope, facts)?;
    assert_eq!(
        proof.initialization,
        ScalarAssignmentSiteV1 {
            block: 0,
            statement: 0
        }
    );
    assert_eq!(
        proof.rows,
        vec![
            Entry {
                block: 1,
                successor: 0,
                role: SemanticEdgeRoleV1::Goto,
                target: 3
            },
            Entry {
                block: 2,
                successor: 0,
                role: SemanticEdgeRoleV1::Goto,
                target: 3
            },
        ]
    );
    replay(
        &proof,
        function,
        &graph(function),
        3,
        4,
        SemanticLocalIdV1::from_index(1),
        &mut Context { scope, facts },
    )
}

const DONE: &str = "component complete; no source owner returned";

fn run(
    work_limit: usize,
    storage_limit: usize,
) -> (
    Result<ProductionRankedRootProgramV1>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    run_component(
        work_limit,
        storage_limit,
        fixture(),
        |function, scope, facts| checked_component(function, scope, facts),
    )
}

fn run_component(
    work_limit: usize,
    storage_limit: usize,
    function: SemanticFunctionDeclV1,
    component: impl FnOnce(&SemanticFunctionDeclV1, &mut Scope, &mut Facts<'_, '_>) -> Result<()>,
) -> (
    Result<ProductionRankedRootProgramV1>,
    usize,
    usize,
    Option<usize>,
    Option<usize>,
) {
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            component(&function, scope, facts)?;
            Err(reject(DONE))
        });
        assert_eq!(budget.storage(), 17);
        assert!(budget.work_ledger_identity_v1() == ledger);
        (
            result,
            budget.work(),
            budget.peak_storage(),
            budget.failed_storage(),
        )
    };
    (result, accepted, peak, failed_storage, work.failed_work())
}

#[test]
fn multi_entry_source_exact_initializer_entries_replay_and_budget_boundaries() {
    let (result, work, peak, storage_denial, work_denial) = run(usize::MAX, usize::MAX);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((storage_denial, work_denial), (None, None));
    let (result, exact_work, exact_peak, a, b) = run(work, peak);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((exact_work, exact_peak, a, b), (work, peak, None, None));
    let (result, accepted, _, storage_denial, work_denial) = run(work - 1, peak);
    let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error)),
    )) = result
    else {
        panic!("exact Work refusal required")
    };
    assert_eq!((error.actual(), error.limit()), (work, work - 1));
    assert_eq!(
        (accepted, storage_denial, work_denial),
        (work - 5, None, Some(work))
    );
    let (result, accepted_work, prior_peak, storage_denial, work_denial) = run(work, peak - 1);
    let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Storage(error)),
    )) = result
    else {
        panic!("exact Storage refusal required")
    };
    assert_eq!((error.actual(), error.limit()), (peak, peak - 1));
    assert_eq!((storage_denial, work_denial), (Some(peak), None));
    // Replay's nested Scratch::new refuses its six-usize pending buffer.
    // The unexecuted suffix includes the 35-unit reinitialization walk.
    assert_eq!(fixture().blocks().len(), 6);
    assert_eq!(
        (accepted_work, prior_peak),
        (
            work.checked_sub(301).unwrap(),
            peak.checked_sub(bytes::<usize>(6).unwrap()).unwrap(),
        )
    );
}

#[test]
fn multi_entry_source_preserves_prior_denial_history_and_deterministic_measurements() {
    let (_, work, peak, _, _) = run(usize::MAX, usize::MAX);
    for _ in 0..4 {
        let (result, repeated_work, repeated_peak, a, b) = run(work, peak);
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
        ));
        assert_eq!(
            (repeated_work, repeated_peak, a, b),
            (work, peak, None, None)
        );
    }
    let mut meter = Work::new(work + 17);
    {
        let mut budget = Budget::new(&mut meter, peak);
        budget.charge_work(17).unwrap();
        budget.reserve_storage(17).unwrap();
        assert!(matches!(
            budget.charge_work(work + 1),
            Err(Resource::Work(_))
        ));
        assert!(matches!(
            budget.reserve_storage(peak),
            Err(Resource::Storage(_))
        ));
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            checked_component(&fixture(), scope, facts)?;
            Err(reject(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (work + 17, 17, peak)
        );
        assert_eq!(budget.failed_storage(), Some(peak + 17));
    }
    assert_eq!(meter.failed_work(), Some(work + 18));
}

#[test]
fn multi_entry_scope_is_lazy_for_historical_paths() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    let result = with_scope(&mut Facts(&mut budget), |_, _| Err(reject(DONE)));
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        (0, 0, 0, None)
    );
}

#[test]
fn multi_entry_scope_keeps_trait_default_custody_optional_without_induction() {
    let mut facts = NoCustodyFacts(0);
    let mut called = false;
    let result = with_scope(&mut facts, |_, _| {
        called = true;
        Err(ProductionRankedProjectionErrorV1::Unsupported(DONE))
    });
    assert!(called);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
    assert_eq!(facts.0, 0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut facts, |_, _| panic!("historical callback unwind"))
    }));
    assert!(panic.is_err());
    assert_eq!(facts.0, 0);
}

#[test]
fn multi_entry_scope_requires_entry_custody_on_use_and_cannot_retry_with_a_new_floor() {
    let mut facts = NoCustodyFacts(0);
    let result = with_scope(&mut facts, |scope, facts| {
        let result = make(&fixture(), scope, facts);
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(_))
        ));
        assert_eq!(facts.0, 0);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(17).unwrap();
        for _ in 0..2 {
            let result = make(&fixture(), scope, &mut Facts(&mut budget));
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!((budget.storage(), budget.work()), (17, 0));
        }
        Err(ProductionRankedProjectionErrorV1::Unsupported(DONE))
    });
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Unsupported(_))
    ));
    assert_eq!(facts.0, 0);
}

#[test]
fn multi_entry_scope_refuses_foreign_account_before_first_induction_use() {
    let mut work = Work::new(usize::MAX);
    let mut other_work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let mut other = Budget::new(&mut other_work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    other.reserve_storage(17).unwrap();
    let account = budget.work_ledger_identity_v1();
    for moved in [false, true] {
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            std::mem::swap(facts.0, &mut other);
            // Same address/foreign ledger, then same ledger/foreign address.
            let result = if moved {
                make(&fixture(), scope, &mut Facts(&mut other))
            } else {
                make(&fixture(), scope, facts)
            };
            std::mem::swap(facts.0, &mut other);
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(Resource::Accounting)
                ))
            ));
            assert_eq!((other.storage(), other.work()), (17, 0));
            Err(ProductionRankedProjectionErrorV1::Unsupported(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Unsupported(_))
        ));
        assert_eq!((budget.storage(), budget.work()), (17, 0));
        assert!(budget.work_ledger_identity_v1() == account);
    }
}

fn extent_then_induction_replay(
    function: &SemanticFunctionDeclV1,
    scope: &mut Scope,
    facts: &mut Facts<'_, '_>,
) -> Result<()> {
    let floor = facts.0.storage();
    let count = function.locals().len();
    let proof = slice_extent_projection_v1::with_scope(count, facts, |extent| {
        let scratch = slice_extent_projection_v1::Scratch {
            origins: vec![None; count],
            arguments: vec![None; count],
            definitions: vec![0; count],
            escaped: vec![false; count],
        };
        extent.retain(&scratch)?;
        make(function, scope, extent.facts())
    })?;
    // The actual extent scope is dead; only the induction's own rows/header
    // remain. Its outer floor must never include the released inner scratch.
    assert_eq!(facts.0.storage(), floor + scope.retained);
    replay(
        &proof,
        function,
        &graph(function),
        3,
        4,
        SemanticLocalIdV1::from_index(1),
        &mut Context { scope, facts },
    )
}

#[test]
fn multi_entry_outer_account_survives_extent_scratch_with_exact_and_short_budgets() {
    let run = |work, storage| run_component(work, storage, fixture(), extent_then_induction_replay);
    let (result, work, peak, a, b) = run(usize::MAX, usize::MAX);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((a, b), (None, None));
    let (result, w, p, a, b) = run(work, peak);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((w, p, a, b), (work, peak, None, None));
    let (result, _, _, a, b) = run(work - 1, peak);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Work(_))
        ))
    ));
    assert_eq!((a, b), (None, Some(work)));
    let (result, _, _, a, b) = run(work, peak - 1);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
            CanonicalAssertionErrorV1::Resource(Resource::Storage(_))
        ))
    ));
    assert_eq!((a, b), (Some(peak), None));
}

#[test]
fn multi_entry_outer_account_refuses_foreign_after_extent_and_initial_floor_loss() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    for foreign in [true, false] {
        let before = budget.work();
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            if foreign {
                extent_then_induction_replay(&fixture(), scope, facts)?;
                let mut other_work = Work::new(usize::MAX);
                let mut other = Budget::new(&mut other_work, usize::MAX);
                let foreign_floor = facts.0.storage();
                other.reserve_storage(foreign_floor).unwrap();
                let result = make(&fixture(), scope, &mut Facts(&mut other));
                assert_eq!((other.storage(), other.work()), (foreign_floor, 0));
                result?;
            } else {
                facts.0.release_storage(1).unwrap();
                make(&fixture(), scope, facts)?;
            }
            Err(reject(DONE))
        });
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                CanonicalAssertionErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.storage(), if foreign { 17 } else { 16 });
        if foreign {
            assert!(budget.work() > before);
        } else {
            assert_eq!(budget.work(), before);
        }
    }
}

#[test]
fn multi_entry_outer_account_unwind_after_extent_release_keeps_original_floor() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    budget.charge_work(13).unwrap();
    let account = budget.work_ledger_identity_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut Facts(&mut budget), |scope, facts| {
            extent_then_induction_replay(&fixture(), scope, facts)?;
            panic!("injected after shorter-lived extent scope");
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 17);
    assert!(budget.work_ledger_identity_v1() == account);
    assert!(budget.work() > 13);
}

#[test]
fn multi_entry_source_replay_rejects_missing_reordered_foreign_and_mutated_entries() {
    let function = fixture();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
        let proof = make(&function, scope, facts)?;
        for mutation in 0..5 {
            let mut bad = (*proof).clone();
            match mutation {
                0 => {
                    bad.rows.pop();
                }
                1 => bad.rows.swap(0, 1),
                2 => bad.rows[0].target = 4,
                3 => bad.initialization.statement = 1,
                4 => bad.function = SemanticFunctionIdentityV1::from_sha256([199; 32]),
                _ => unreachable!(),
            }
            let error = replay(
                &bad,
                &function,
                &graph(&function),
                3,
                4,
                SemanticLocalIdV1::from_index(1),
                &mut Context { scope, facts },
            );
            assert!(matches!(
                error,
                Err(ProductionRankedProjectionErrorV1::Incomplete(_))
            ));
        }
        let foreign = function.clone();
        let error = replay(
            &proof,
            &foreign,
            &graph(&foreign),
            3,
            4,
            SemanticLocalIdV1::from_index(1),
            &mut Context { scope, facts },
        );
        assert!(matches!(
            error,
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "multi-entry proof is attached to a different source recurrence"
            ))
        ));
        Err(reject(DONE))
    });
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn multi_entry_source_rejects_kills_and_bypassed_initializer() {
    let function = fixture();
    for mutation in 0..6 {
        let mut blocks = function.blocks().to_vec();
        let original = &blocks[1];
        let local = SemanticLocalIdV1::from_index(1);
        let kind = match mutation {
            0 => SemanticStatementKindV1::StorageDead(local),
            1 => SemanticStatementKindV1::StorageLive(local),
            2 => SemanticStatementKindV1::Assume(SemanticOperandV1::Move(
                SemanticPlaceV1::new(local, vec![], function.locals()[1].ty()).unwrap(),
            )),
            4 => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(0),
                    vec![],
                    function.locals()[0].ty(),
                )
                .unwrap(),
                SemanticRvalueV1::new(
                    function.locals()[1].ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(
                        SemanticPlaceV1::new(local, vec![], function.locals()[1].ty()).unwrap(),
                    )),
                ),
            )),
            _ => function.blocks()[0].statements()[0].kind().clone(),
        };
        let statement = SemanticStatementV1::new(original.terminator().source(), kind);
        blocks[1] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            vec![statement],
            original.terminator().clone(),
        )
        .unwrap();
        if mutation == 5 {
            let entry = &function.blocks()[0];
            blocks[0] = SemanticBasicBlockV1::new(
                entry.identity(),
                entry.source(),
                vec![],
                entry.terminator().clone(),
            )
            .unwrap();
        }
        let changed = rebuilt(&function, blocks);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            make(&changed, scope, facts)?;
            Err(reject(DONE))
        });
        assert!(
            matches!(result, Err(ProductionRankedProjectionErrorV1::Incomplete(reason)) if reason != DONE)
        );
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn multi_entry_scope_preserves_sibling_on_panic_and_refuses_foreign_ledger() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut Facts(&mut budget), |scope, facts| {
            let function = fixture();
            let proof = make(&function, scope, facts)?;
            assert_eq!(proof.rows.len(), 2);
            facts.0.reserve_storage(23).unwrap();
            let mut other_work = Work::new(usize::MAX);
            let mut other = Budget::new(&mut other_work, usize::MAX);
            let error = make(&fixture(), scope, &mut Facts(&mut other));
            assert!(matches!(
                error,
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                        Resource::Accounting
                    )
                ))
            ));
            assert_eq!(other.storage(), 0);
            panic!("injected source proof unwind");
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn multi_entry_box_keeps_historical_control_and_induction_row_layouts() {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    enum OldControl {
        Direct,
        Optional {
            discriminant: SemanticOperandV1,
            explicit_value: u128,
            explicit_target: usize,
            otherwise: usize,
        },
    }
    #[allow(dead_code)]
    struct OldInduction {
        initializer_block: usize,
        preheader_control: OldControl,
        header: usize,
        body_entry: usize,
        latch: usize,
        exit: usize,
        loop_blocks: Vec<usize>,
        initial: ProductionRankedValueV1,
        bound: ProductionRankedValueV1,
        step: ProductionRankedValueV1,
        source_progress: ProjectedSourceInductionCandidateV1,
        bound_cast: Option<ProjectedUnsignedCastCandidateV1>,
        body_predicates: Vec<ProjectedInductionBodyPredicateV1>,
    }
    assert_eq!(
        size_of::<OldControl>(),
        size_of::<ProjectedInductionPreheaderControlV1>()
    );
    assert_eq!(
        align_of::<OldControl>(),
        align_of::<ProjectedInductionPreheaderControlV1>()
    );
    assert_eq!(
        size_of::<OldInduction>(),
        size_of::<ProjectedUniformInductionV1>()
    );
    assert_eq!(
        align_of::<OldInduction>(),
        align_of::<ProjectedUniformInductionV1>()
    );
}

fn distant_fixture() -> SemanticFunctionDeclV1 {
    super::super::tests::distant_initializer_source_fixture_v1()
}

fn distant_row(
    function: &SemanticFunctionDeclV1,
    context: &mut Context<'_, '_>,
) -> Result<ProjectedUniformInductionV1> {
    let mut row = super::super::tests::distant_initializer_induction_fixture_v1();
    let proof = single::build(
        function,
        &graph(function),
        6,
        3,
        4,
        SemanticLocalIdV1::from_index(1),
        context,
    )?;
    assert_eq!(
        proof.initialization(),
        ScalarAssignmentSiteV1 {
            block: 0,
            statement: 0
        }
    );
    row.preheader_control = ProjectedInductionPreheaderControlV1::DistantDirect(proof);
    Ok(row)
}

fn distant_proof(row: &ProjectedUniformInductionV1) -> &single::Proof {
    let ProjectedInductionPreheaderControlV1::DistantDirect(proof) = &row.preheader_control else {
        unreachable!()
    };
    proof
}

fn bind_distant(
    row: &mut ProjectedUniformInductionV1,
    context: &mut Context<'_, '_>,
) -> Result<()> {
    let ProjectedInductionPreheaderControlV1::DistantDirect(proof) = &mut row.preheader_control
    else {
        unreachable!()
    };
    single::bind_initial(proof, row.initial, context)
}

fn distant_component(
    function: &SemanticFunctionDeclV1,
    scope: &mut Scope,
    facts: &mut dyn ProjectedAssertionFactsV1,
) -> Result<()> {
    let mut context = Context { scope, facts };
    let mut row = distant_row(function, &mut context)?;
    bind_distant(&mut row, &mut context)?;
    single::replay(
        distant_proof(&row),
        function,
        &graph(function),
        &row,
        &mut context,
    )?;
    single::before_emission(distant_proof(&row), function, &row, &mut context)
}

#[test]
fn distant_initializer_exact_replay_and_resource_boundaries() {
    let run = |work, storage| {
        run_component(
            work,
            storage,
            distant_fixture(),
            |function, scope, facts| distant_component(function, scope, facts),
        )
    };
    let (result, work, peak, a, b) = run(usize::MAX, usize::MAX);
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!((a, b), (None, None));
    for _ in 0..3 {
        let (result, w, p, a, b) = run(work, peak);
        assert!(matches!(
            result,
            Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
        ));
        assert_eq!((w, p, a, b), (work, peak, None, None));
    }
    let (result, accepted, prior_peak, a, b) = run(work - 1, peak);
    let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Work(error)),
    )) = result
    else {
        panic!("exact work refusal required")
    };
    assert_eq!(
        (error.actual(), error.limit(), a, b),
        (work, work - 1, None, Some(work))
    );
    // The final exact-Goto replay refuses its five-unit charge.
    assert_eq!((accepted, prior_peak), (work - 5, peak));
    let (result, accepted, prior_peak, a, b) = run(work, peak - 1);
    let Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
        canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(Resource::Storage(error)),
    )) = result
    else {
        panic!("exact storage refusal required")
    };
    assert_eq!(
        (error.actual(), error.limit(), a, b),
        (peak, peak - 1, Some(peak), None)
    );
    // Replay refuses the seven-element outside buffer before the remaining scan.
    assert_eq!(
        (accepted, prior_peak),
        (work - 352, peak - bytes::<bool>(7).unwrap())
    );
}

#[test]
fn distant_initializer_rejects_kills_bypass_and_multiple_entries() {
    let function = distant_fixture();
    for mutation in 0..8 {
        let mut blocks = function.blocks().to_vec();
        let local = SemanticLocalIdV1::from_index(1);
        let place = SemanticPlaceV1::new(local, vec![], function.locals()[1].ty()).unwrap();
        let selected = if mutation == 2 || mutation == 7 {
            2
        } else if mutation == 4 {
            6
        } else {
            1
        };
        let original = &blocks[selected];
        let kind = match mutation {
            1 => SemanticStatementKindV1::StorageDead(local),
            2 => SemanticStatementKindV1::StorageLive(local),
            3 => SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(0),
                    vec![],
                    function.locals()[0].ty(),
                )
                .unwrap(),
                SemanticRvalueV1::new(
                    function.locals()[1].ty(),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)),
                ),
            )),
            4 => SemanticStatementKindV1::Deinitialize(place),
            _ => function.blocks()[0].statements()[0].kind().clone(),
        };
        let statements = if mutation == 5 || mutation == 7 {
            vec![]
        } else {
            vec![SemanticStatementV1::new(
                original.terminator().source(),
                kind,
            )]
        };
        let terminator = if mutation == 7 {
            function.blocks()[6].terminator().clone()
        } else {
            original.terminator().clone()
        };
        blocks[selected] = SemanticBasicBlockV1::new(
            original.identity(),
            original.source(),
            statements,
            terminator,
        )
        .unwrap();
        if mutation == 5 || mutation == 6 {
            let entry = &function.blocks()[0];
            blocks[0] = SemanticBasicBlockV1::new(
                entry.identity(),
                entry.source(),
                vec![],
                entry.terminator().clone(),
            )
            .unwrap();
        }
        let changed = rebuilt(&function, blocks);
        let (result, _, _, _, _) =
            run_component(usize::MAX, usize::MAX, changed, |function, scope, facts| {
                distant_component(function, scope, facts)
            });
        assert!(
            matches!(result, Err(ProductionRankedProjectionErrorV1::Incomplete(reason)) if reason != DONE),
            "mutation {mutation}"
        );
    }
}

#[test]
fn distant_initializer_rejects_unbound_duplicate_and_substituted_custody() {
    let function = distant_fixture();
    let foreign = function.clone();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
        let mut context = Context { scope, facts };
        let mut row = distant_row(&function, &mut context)?;
        assert!(matches!(
            single::before_emission(distant_proof(&row), &function, &row, &mut context),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "distant initializer lost source/row custody"
            ))
        ));
        bind_distant(&mut row, &mut context)?;
        assert!(matches!(
            bind_distant(&mut row, &mut context),
            Err(ProductionRankedProjectionErrorV1::Incomplete(
                "distant initializer was bound twice"
            ))
        ));
        single::replay(
            distant_proof(&row),
            &function,
            &graph(&function),
            &row,
            &mut context,
        )?;
        for mutation in 0..5 {
            let mut changed = row.clone();
            match mutation {
                1 => {
                    assert_ne!(changed.initial, changed.step);
                    changed.initial = changed.step;
                }
                2 => changed.initializer_block = 1,
                3 => changed.source_progress.induction = SemanticLocalIdV1::from_index(3),
                _ => {}
            }
            let source = if mutation == 0 { &foreign } else { &function };
            // A separately cloned proof is not the proof retained by this row.
            let proof = if mutation == 4 {
                distant_proof(&row)
            } else {
                distant_proof(&changed)
            };
            assert!(matches!(
                single::replay(proof, source, &graph(source), &changed, &mut context),
                Err(ProductionRankedProjectionErrorV1::Incomplete(_))
            ));
            assert!(matches!(
                single::before_emission(proof, source, &changed, &mut context),
                Err(ProductionRankedProjectionErrorV1::Incomplete(_))
            ));
        }
        Err(reject(DONE))
    });
    assert!(matches!(
        result,
        Err(ProductionRankedProjectionErrorV1::Incomplete(DONE))
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn distant_initializer_unwind_preserves_sibling_floor_and_refuses_foreign_ledger() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(17).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        with_scope(&mut Facts(&mut budget), |scope, facts| {
            let function = distant_fixture();
            let row = distant_row(&function, &mut Context { scope, facts })?;
            facts.0.reserve_storage(23).unwrap();
            let mut other_work = Work::new(usize::MAX);
            let mut other = Budget::new(&mut other_work, usize::MAX);
            assert!(matches!(
                distant_row(
                    &function,
                    &mut Context {
                        scope,
                        facts: &mut Facts(&mut other)
                    }
                ),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(
                        Resource::Accounting
                    )
                ))
            ));
            assert_eq!(other.storage(), 0);
            drop(row);
            panic!("injected distant initializer unwind");
        })
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 40);
    assert_eq!(budget.failed_storage(), None);
}

fn reentry_fixture(function: &SemanticFunctionDeclV1, target: u32) -> SemanticFunctionDeclV1 {
    let mut blocks = function.blocks().to_vec();
    let original = &blocks[5];
    blocks[5] = SemanticBasicBlockV1::new(
        original.identity(),
        original.source(),
        vec![],
        fe2o3_mir_model::semantic_mir_v1::SemanticTerminatorV1::new(
            original.source(),
            SemanticTerminatorKindV1::Goto(
                fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::Goto,
                    SemanticBlockIdV1::from_index(target),
                ),
            ),
        ),
    )
    .unwrap();
    rebuilt(function, blocks)
}

#[test]
fn distant_initializer_requires_reinitialization_on_every_reentry() {
    let function = distant_fixture();
    for reinitializes in [false, true] {
        let (result, _, _, _, _) = run_component(
            usize::MAX,
            usize::MAX,
            reentry_fixture(&function, if reinitializes { 0 } else { 6 }),
            |function, scope, facts| distant_component(function, scope, facts),
        );
        let expected = if reinitializes {
            DONE
        } else {
            "induction initializer is bypassed on loop re-entry"
        };
        assert!(
            matches!(result, Err(ProductionRankedProjectionErrorV1::Incomplete(reason)) if reason == expected)
        );
    }
}

#[test]
fn multi_entry_initializer_requires_reinitialization_on_every_reentry() {
    for target in [0, 1, 2] {
        let (result, _, _, _, _) = run_component(
            usize::MAX,
            usize::MAX,
            reentry_fixture(&fixture(), target),
            |function, scope, facts| checked_component(function, scope, facts),
        );
        let expected = if target == 0 {
            DONE
        } else {
            "induction initializer is bypassed on loop re-entry"
        };
        assert!(
            matches!(result, Err(ProductionRankedProjectionErrorV1::Incomplete(reason)) if reason == expected)
        );
    }
}

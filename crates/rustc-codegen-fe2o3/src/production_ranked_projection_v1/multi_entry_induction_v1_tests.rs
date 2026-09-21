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
    let mut work = Work::new(work_limit);
    let (result, accepted, peak, failed_storage) = {
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = with_scope(&mut Facts(&mut budget), |scope, facts| {
            checked_component(&fixture(), scope, facts)?;
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
    // The unexecuted suffix costs 12 + 158 + 10 + 64 + 12 + 10 work units.
    assert_eq!(fixture().blocks().len(), 6);
    assert_eq!(
        (accepted_work, prior_peak),
        (
            work.checked_sub(266).unwrap(),
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

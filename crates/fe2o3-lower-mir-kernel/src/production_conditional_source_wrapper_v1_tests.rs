include!("production_conditional_continuation_accounting_v1_tests.rs");

fn conditional_wrapper_pending(
    source: &ProductionPreRankedKirOwnerV1,
) -> fe2o3_pliron::ProductionConditionalRankedAnalysisV1 {
    use fe2o3_pliron::{
        ProductionConditionalOwnershipSiteV1, ProductionConstructionV1, ProductionPlironSessionV1,
        ProductionRankedBlockV1, ProductionRankedKernelV1, ProductionRankedTerminatorV1,
        ProductionRankedValueIdV1, ProductionSessionLimitsV1,
    };
    let layout = source.source_launch().roots()[0].layout();
    let view = ProductionRankedValueV1::Local(ProductionRankedValueIdV1::new(0));
    // No global effects: the pending contract exists only to exercise custody
    // and wrapper association. No output coverage is claimed by this fixture.
    let recipe = ProductionRankedKernelV1::new(
        "selected_wrapper_0",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: layout.grid_identity(),
                    global_extents: layout.global_extents(),
                    workgroup_extents: layout.workgroup_extents(),
                    subgroup_size: layout.subgroup_size(),
                    full_physical_workgroups: layout.full_physical_workgroups(),
                },
                ProductionRankedOperationV1::View {
                    result: ProductionRankedValueIdV1::new(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 1,
                },
                ProductionRankedOperationV1::OwnershipContract {
                    view,
                    coverage: dialect_kernel::OwnershipCoverageAttr::TotalView,
                    partition: dialect_kernel::OwnershipPartitionAttr::ExactSets,
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let mut session = ProductionPlironSessionV1::new(
        ProductionSessionLimitsV1::default(),
        [
            dialect_gpu::dialect_registration().unwrap(),
            dialect_kernel::dialect_registration().unwrap(),
        ],
    )
    .unwrap();
    let registered = session
        .register_construction(
            ProductionConstructionV1::ranked_kernel("selected_body_pending", recipe).unwrap(),
        )
        .unwrap();
    let (stage, ranked_root) = session.construct_registered(registered).unwrap();
    session
        .prepare_conditional_ranked_analysis_v1(
            stage,
            ranked_root,
            &[ProductionConditionalOwnershipSiteV1 {
                block: 0,
                operation: 2,
                view,
            }],
        )
        .unwrap()
}

#[test]
fn conditional_translation_distinguishes_the_root_from_its_selected_result_body() {
    let mut source = materialize(Fixture::default());
    let selected = source
        .semantic_ssa()
        .source_semantic()
        .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    assert_ne!(selected.root(), selected.body());
    let root = selected.root();
    let body = selected.body();
    let pending = conditional_wrapper_pending(&source);
    let run = |source: &ProductionPreRankedKirOwnerV1, candidate_root: u32| {
        let candidate = crate::NativeRankedSourceCandidateV1::from_untrusted_parts(
            candidate_root,
            1,
            pending.kernel().unwrap(),
            &[],
            &[],
            "",
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        let floor = source.retained_analysis_storage_v1() + FLOOR;
        budget.reserve_storage(floor).unwrap();
        let result = source
            .check_conditional_source_translation_v1(&pending, candidate, &mut budget)
            .map(|result| result.memory_effects());
        assert_eq!(budget.storage(), floor);
        result
    };
    assert_eq!(run(&source, root.index()).unwrap(), 0);
    assert!(matches!(
        run(&source, body.index()),
        Err(ProductionConditionalSourceTranslationErrorV1::SourceAssociation)
    ));
    let mut rows = source.correspondence.lowered_functions.to_vec();
    let entry = rows
        .iter_mut()
        .find(|row| row.role == SemanticKirFunctionRoleV1::KernelEntry)
        .unwrap();
    entry.semantic_function = root;
    source.correspondence.lowered_functions = rows.into_boxed_slice();
    assert!(matches!(
        run(&source, root.index()),
        Err(ProductionConditionalSourceTranslationErrorV1::SourceAssociation)
    ));
}

fn continuation_input(
    source: &ProductionPreRankedKirOwnerV1,
    root: u32,
) -> ProductionConditionalRootInputV1 {
    use fe2o3_proof_contracts::DigestV1;
    let digest = |byte| DigestV1::from_untrusted_bytes([byte; 32]);
    ProductionConditionalRootInputV1 {
        pending: conditional_wrapper_pending(source),
        semantic_root: root,
        launch_rank: 1,
        access_sources: vec![],
        executable_effect_sources: vec![],
        ranked_ir: String::new(),
        reference_subjects: fe2o3_pliron::ProductionConditionalReferenceSubjectsV1::new(
            fe2o3_functional_proof::SafeReferenceKindV2::Mir,
            digest(1),
            DigestV1::ZERO,
            digest(2),
            digest(3),
            digest(4),
        )
        .unwrap(),
    }
}

#[test]
fn consuming_continuation_checks_source_root_before_exposing_an_aggregate_request() {
    use fe2o3_kernel_ir::CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned;
    let source = materialize(Fixture::default());
    let selected = source
        .semantic_ssa()
        .source_semantic()
        .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let mut ledger = Owned::new(CanonicalKernelIrWorkBudgetV1::new(WORK), STORAGE);
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    ledger
        .with_budget(|budget| budget.reserve_storage(floor))
        .unwrap();
    let error = continue_conditional_root_v1(
        &source,
        continuation_input(&source, selected.body().index()),
        &mut ledger,
    )
    .err()
    .expect("selected body is not the source root");
    assert!(matches!(
        error,
        ProductionConditionalContinuationErrorV1::Source(
            ProductionConditionalSourceTranslationErrorV1::SourceAssociation
        )
    ));
    assert_eq!(ledger.storage(), floor);
    let accepted = ledger.work();
    // Source authentication succeeds, but the no-output fixture must not gain
    // an aggregate subject from its otherwise well-formed pending ownership row.
    let error = continue_conditional_root_v1(
        &source,
        continuation_input(&source, selected.root().index()),
        &mut ledger,
    )
    .err()
    .expect("no output cannot supply total-output continuation");
    assert!(matches!(
        error,
        ProductionConditionalContinuationErrorV1::Subject(
            "unsupported canonical conditional output"
        )
    ));
    assert!(ledger.work() > accepted);
    assert_eq!(ledger.storage(), floor);
}

#[test]
fn consuming_continuation_denials_preserve_the_original_account_and_source_floor() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    };
    let source = materialize(Fixture::default());
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    for deny_work in [false, true] {
        let mut ledger = Owned::new(
            CanonicalKernelIrWorkBudgetV1::new(if deny_work { 7 } else { WORK }),
            if deny_work { STORAGE } else { floor },
        );
        ledger
            .with_budget(|budget| {
                budget.reserve_storage(floor)?;
                budget.charge_work(7)
            })
            .unwrap();
        let error =
            continue_conditional_root_v1(&source, continuation_input(&source, 0), &mut ledger)
                .err()
                .expect("original ledger must deny continuation");
        if deny_work {
            assert!(matches!(
                error,
                ProductionConditionalContinuationErrorV1::Resource(Resource::Work(_))
            ));
            assert_eq!(ledger.work(), 7);
        } else {
            assert!(matches!(
                error,
                ProductionConditionalContinuationErrorV1::Resource(Resource::Storage(_))
            ));
            assert_eq!(ledger.work(), 15);
        }
        assert_eq!(ledger.storage(), floor);
    }
}

#[test]
fn consuming_continuation_adopts_spare_input_capacity_before_source_replay() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    };
    let source = materialize(Fixture::default());
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    for buffer in 0..3 {
        // Empty buffers keep the same correspondence but still occupy storage.
        let mut input = continuation_input(&source, 0);
        match buffer {
            0 => input.access_sources.reserve_exact(64),
            1 => input.executable_effect_sources.reserve_exact(64),
            _ => input.ranked_ir.reserve_exact(4096),
        }
        let prior_allowance = input.pending.retained_analysis_storage_v1()
            + std::mem::size_of::<ProductionConditionalFinalRootV1<'_, '_>>();
        let mut ledger = Owned::new(
            CanonicalKernelIrWorkBudgetV1::new(WORK),
            floor + prior_allowance,
        );
        ledger
            .with_budget(|budget| budget.reserve_storage(floor))
            .unwrap();
        let error = continue_conditional_root_v1(&source, input, &mut ledger)
            .err()
            .expect("spare capacity must be charged before source replay");
        assert!(matches!(
            error,
            ProductionConditionalContinuationErrorV1::Resource(Resource::Storage(_))
        ));
        assert_eq!(ledger.storage(), floor);
        assert_eq!(ledger.work(), 8);
    }
}

#[test]
fn borrowed_continuation_replays_source_on_the_original_account() {
    let source = materialize(Fixture::default());
    let selected = source
        .semantic_ssa()
        .source_semantic()
        .select_kernel_body_for_root_v1(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    budget.charge_work(7).unwrap();
    let account = budget.work_ledger_identity_v1();
    for (root, wrong_body) in [(selected.body(), true), (selected.root(), false)] {
        let error = with_conditional_root_request_v1(
            &source,
            continuation_input(&source, root.index()),
            &mut budget,
            |_, _| panic!("a source mismatch or missing output cannot expose proof subjects"),
        )
        .err()
        .expect("unsupported source must remain closed");
        if wrong_body {
            assert!(matches!(
                error,
                ProductionConditionalContinuationErrorV1::Source(
                    ProductionConditionalSourceTranslationErrorV1::SourceAssociation
                )
            ));
        } else {
            assert!(matches!(
                error,
                ProductionConditionalContinuationErrorV1::Subject(
                    "unsupported canonical conditional output"
                )
            ));
        }
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), floor);
    }
    drop(budget);
    assert!(work.work() > 7);
}

#[test]
fn borrowed_continuation_cannot_restart_exhausted_work_or_skip_input_capacity() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let source = materialize(Fixture::default());
    let floor = source.retained_analysis_storage_v1() + FLOOR;
    for denial in 0..5 {
        let mut input = continuation_input(&source, 0);
        let old_allowance = input.pending.retained_analysis_storage_v1()
            + std::mem::size_of::<ProductionConditionalFinalRootV1<'_, '_>>();
        match denial {
            2 => input.access_sources.reserve_exact(64),
            3 => input.executable_effect_sources.reserve_exact(64),
            4 => input.ranked_ir.reserve_exact(4096),
            _ => (),
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(if denial == 0 { 7 } else { WORK });
        let storage = if denial == 0 {
            STORAGE
        } else if denial == 1 {
            floor
        } else {
            floor + old_allowance
        };
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
        budget.reserve_storage(floor).unwrap();
        budget.charge_work(7).unwrap();
        let account = budget.work_ledger_identity_v1();
        let error = with_conditional_root_request_v1(&source, input, &mut budget, |_, _| {
            panic!("exhausted inherited account cannot expose a request")
        })
        .err()
        .expect("the original account must deny continuation");
        if denial == 0 {
            assert!(matches!(
                error,
                ProductionConditionalContinuationErrorV1::Resource(Resource::Work(_))
            ));
        } else {
            assert!(matches!(
                error,
                ProductionConditionalContinuationErrorV1::Resource(Resource::Storage(_))
            ));
        }
        assert!(budget.work_ledger_identity_v1() == account);
        assert_eq!(budget.storage(), floor);
        drop(budget);
        assert_eq!(work.work(), if denial == 0 { 7 } else { 15 });
    }
}

use super::*;

const INVOCATION_INDEX_WORK_LIMIT_V1: usize = 100_000_000;

// A real captured source/N/B/O view, but only the private numeric index builder
// is queried. The inert empty claims below do not complete control coverage.
fn with_invocation_source_index_fixture_v1(
    body: impl FnOnce(
        &ProductionSourceOutputOccurrencesV1<'_, '_>,
        &ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        usize,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let root = SemanticFunctionIdV1::from_index(0);
    let export = "invocation_index_resource";
    let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
        noop_semantic_owner(&[export]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work =
        fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(INVOCATION_INDEX_WORK_LIMIT_V1);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 16 * 1024 * 1024);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(19).unwrap();
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let entry = ssa.source_semantic().functions()[0].kernel_entry().unwrap();
    let required = entry
        .source_contract()
        .launch()
        .unwrap()
        .required()
        .unwrap()
        .as_array();
    let input = crate::ProductionSourceLaunchRootInputV1::new(
        export,
        *entry.kernel_binding_identity().as_bytes(),
        crate::ProductionSourceLaunchInputV1::new(1, Some(required), [1, 1, 1]),
    );
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &[input]).unwrap();
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    let source_bytes = source.executable_storage().retained_storage()
        + source.assert_origin_storage().payload_storage();
    budget.reserve_storage(source_bytes).unwrap();
    let observed = fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(
        source.executable(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
    let output_bytes = checked.storage().retained_storage();
    budget.reserve_storage(output_bytes).unwrap();
    let coordinate_bytes;
    {
        let (coordinates, coordinate_storage) =
            fe2o3_kernel_analysis::check_canonical_kir_coordinate_preservation_v1(
                source.executable(),
                source.executable(),
                &mut budget,
            )
            .unwrap();
        coordinate_bytes = coordinate_storage.retained_storage();
        budget.reserve_storage(coordinate_bytes).unwrap();
        let (view, storage) = crate::derive_source_output_occurrences_policy3_v1(
            &source,
            &coordinates,
            &checked,
            &mut budget,
        )
        .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let ranked = noop_ranked_root(root, export);
        let claims = ProductionProjectionControlCandidateV1::default();
        let candidate = ProductionCanonicalMemoryAnalysisCandidateV1 {
            selected_root: root,
            selected_function: root,
            lowering: &ranked.lowering,
            access_sources: &[],
            executable_effect_sources: &[],
            control: &claims,
        };
        let canonical =
            source_output_ordinary_function_alias_v1(&source, root, root, &mut budget).unwrap();
        let captured = source
            .semantic_ssa()
            .occurrences_v1()
            .unwrap()
            .function(root)
            .unwrap();
        assert!(std::ptr::eq(captured.owner(), source.semantic_ssa()));
        assert!(captured.events().iter().all(|event| {
            source_output_invocation_event_key_v1(event).is_none()
                && !matches!(event.resolved(), Some(SsaResolvedEventV1::Define { .. }))
        }));
        assert!(captured.edge_definitions().is_empty());
        assert!(captured.successors().is_empty());
        assert_eq!(source.correspondence.statement_operation_spans().len(), 1);
        assert_eq!(source.correspondence.terminator_operation_spans().len(), 1);
        let function = &source.executable().module().functions[canonical.0 as usize];
        assert_eq!(function.body.as_ref().unwrap().blocks.len(), 1);
        assert!(
            function.body.as_ref().unwrap().blocks[0]
                .operations
                .is_empty()
        );

        // Capture query 8; every ignored event 8; one Nop span 5;
        // first Vec reserve/push 1+1; one terminator span scan 5. The next
        // reserve costs one work unit and fails AFTER a statement Vec is live.
        let index_prefix = 8 + 8 * captured.events().len() + 5 + 1 + 1 + 5;
        let floor = budget.storage();
        body(&view, &candidate, canonical, index_prefix, &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(view);
        budget.release_storage(storage.retained_storage()).unwrap();
    }
    budget.release_storage(coordinate_bytes).unwrap();
    drop(checked);
    budget.release_storage(output_bytes).unwrap();
    drop(source);
    budget.release_storage(source_bytes).unwrap();
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn invocation_source_index_first_vector_storage_denial_restores_the_real_view_floor() {
    with_invocation_source_index_fixture_v1(|view, candidate, canonical, _, budget| {
        let floor = budget.storage();
        let header = std::mem::size_of::<SourceOutputInvocationSourceIndexV1>();
        let first_capacity_request = 4 * std::mem::size_of::<((u32, u32), usize)>();
        let headroom = header + first_capacity_request - 1;
        let held = budget.storage_limit() - floor - headroom;
        budget.reserve_storage(held).unwrap();
        let held_floor = budget.storage();
        let before = budget.work();
        let result = source_output_global_scratch_scope_v1(budget, |budget| {
            let index =
                source_output_invocation_source_index_v1(view, candidate, canonical, budget)?;
            drop(index);
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOutputErrorV1::SourceOrigin(
            SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Storage(error))
        )) if error.actual() == budget.storage_limit() + 1)
        );
        assert!(budget.work() > before);
        assert_eq!(budget.storage(), held_floor);
        assert_eq!(budget.failed_storage(), Some(budget.storage_limit() + 1));
        budget.release_storage(held).unwrap();
        source_output_global_scratch_scope_v1(budget, |budget| {
            let index =
                source_output_invocation_source_index_v1(view, candidate, canonical, budget)?;
            assert!(index.events.is_empty());
            assert_eq!(index.statements.len(), 1);
            assert_eq!(index.terminators.len(), 1);
            assert_eq!(index.blocks.len(), 1);
            drop(index);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(budget.storage_limit() + 1));
    });
}

#[test]
fn invocation_source_index_later_work_denial_drops_its_allocated_prefix() {
    with_invocation_source_index_fixture_v1(|view, candidate, canonical, index_prefix, budget| {
        let floor = budget.storage();
        let held_work = INVOCATION_INDEX_WORK_LIMIT_V1 - budget.work() - index_prefix;
        budget.charge_work(held_work).unwrap();
        let before = budget.work();
        let result = source_output_global_scratch_scope_v1(budget, |budget| {
            let index =
                source_output_invocation_source_index_v1(view, candidate, canonical, budget)?;
            drop(index);
            Ok(())
        });
        assert!(
            matches!(result, Err(ProductionSourceOutputErrorV1::SourceOrigin(
            SemanticKirAssertOriginErrorV1::Resource(AssertOriginResourceV1::Work(error))
        )) if error.actual() == INVOCATION_INDEX_WORK_LIMIT_V1 + 1
            && error.limit() == INVOCATION_INDEX_WORK_LIMIT_V1)
        );
        assert_eq!(budget.work(), before + index_prefix);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
    });
}

#[derive(Clone, Copy)]
enum Expected {
    Accepted,
    CorrespondenceMismatch,
}

fn source_and_output() -> (
    ProductionSemanticSsaOwnerV1,
    Module,
    SemanticKirCorrespondenceV1,
) {
    let source = ProductionSemanticSsaOwnerV1::try_new(
        helper_closure_semantic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let (module, correspondence) =
        lower_module(&source, ProductionSemanticKirLimitsV1::default(), None).unwrap();
    assert_eq!(source.source_semantic().functions().len(), 2);
    assert_eq!(
        source.source_semantic().roots(),
        [SemanticFunctionIdV1::from_index(0)]
    );
    assert_eq!((module.functions.len(), module.kernels.len()), (2, 1));
    assert_eq!(correspondence.lowered_functions.len(), 2);
    assert_eq!(correspondence.blocks.len(), 3);
    assert_eq!(correspondence.terminator_operation_spans.len(), 3);
    assert!(correspondence.parameter_bindings.is_empty());
    (source, module, correspondence)
}

fn check_both(
    source: &ProductionSemanticSsaOwnerV1,
    module: &Module,
    roots: &[SemanticFunctionIdV1],
    max_blocks: usize,
    correspondence: &SemanticKirCorrespondenceV1,
    expected: Expected,
) {
    // The private core's precondition is established by a real replay on the
    // same immutable owner. No fabricated proof token or skip mode is exposed.
    source.verify_replay().unwrap();
    let core = validate_semantic_kir_correspondence_after_source_replay_v1(
        source,
        module,
        roots,
        max_blocks,
        correspondence,
    );
    let wrapper =
        validate_semantic_kir_correspondence(source, module, roots, max_blocks, correspondence);
    for result in [core, wrapper] {
        match expected {
            Expected::Accepted => result.unwrap(),
            Expected::CorrespondenceMismatch => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            )),
        }
    }
}

#[test]
fn correspondence_replay_and_core_accept_the_same_actual_helper_output() {
    let (source, module, correspondence) = source_and_output();
    assert!(source.occurrences_v1().is_none());
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &correspondence,
        Expected::Accepted,
    );
}

#[test]
fn captured_source_does_not_change_the_legacy_correspondence_wrapper() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
    };
    let (mut source, module, correspondence) = source_and_output();
    let identity = source.identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget =
        CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 16 * 1024 * 1024);
    let receipt = source
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 0);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(source.identity(), identity);
    let capture = source.occurrences_v1().unwrap();
    assert_eq!(capture.function_count(), 2);
    assert!(std::ptr::eq(
        capture
            .function(SemanticFunctionIdV1::from_index(0))
            .unwrap()
            .owner(),
        &source
    ));
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &correspondence,
        Expected::Accepted,
    );
    let mut changed = correspondence.clone();
    changed.semantic_sha256[0] ^= 1;
    check_both(
        &source,
        &module,
        source.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &changed,
        Expected::CorrespondenceMismatch,
    );
    drop(source);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn correspondence_core_preserves_exact_structural_rejections() {
    let (source, module, correspondence) = source_and_output();
    let roots = source.source_semantic().roots();
    let limits = ProductionSemanticKirLimitsV1::default();
    for mutation in 0..8 {
        let mut changed = correspondence.clone();
        let mut graph = module.clone();
        let foreign = [SemanticFunctionIdV1::from_index(1)];
        let expected_roots = match mutation {
            0 => &[][..],
            1 => &foreign,
            _ => roots,
        };
        match mutation {
            0 | 1 => {}
            2 => changed.function_count += 1,
            3 => graph.kernels[0].entry = FunctionId::new("foreign_entry"),
            4 => {
                changed.lowered_functions =
                    changed.lowered_functions[..1].to_vec().into_boxed_slice()
            }
            5 => changed.blocks[0].semantic_block = SemanticBlockIdV1::from_index(99),
            6 => changed.terminator_operation_spans[0].operation_count += 1,
            7 => {
                changed.parameter_bindings = vec![SemanticKirParameterBindingV1 {
                    correspondence_owner: SemanticFunctionIdV1::from_index(0),
                    semantic_function: SemanticFunctionIdV1::from_index(0),
                    semantic_local: SemanticLocalIdV1::from_index(0),
                    kernel_ir_value: ValueId(0),
                }]
                .into_boxed_slice()
            }
            _ => unreachable!(),
        }
        check_both(
            &source,
            &graph,
            expected_roots,
            limits.max_blocks,
            &changed,
            Expected::CorrespondenceMismatch,
        );
    }
    // The closure rejects three required blocks against a limit of two.
    // Both correspondence entries preserve their existing mismatch mapping.
    let semantic = source.source_semantic();
    let selected_body = semantic
        .select_kernel_body_for_root_v1(roots[0])
        .unwrap()
        .body();
    let mut closure_budget = ReachableClosureBlockBudgetV1::new(2);
    assert!(matches!(
        reachable_defined_closure_v1(
            semantic,
            selected_body,
            correspondence.function_count,
            &mut closure_budget,
        ),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::Blocks,
            actual: 3,
            limit: 2,
        })
    ));
    check_both(
        &source,
        &module,
        roots,
        2,
        &correspondence,
        Expected::CorrespondenceMismatch,
    );
    check_both(
        &source,
        &module,
        roots,
        3,
        &correspondence,
        Expected::Accepted,
    );
}

#[test]
fn correspondence_core_rejects_a_different_genuinely_admitted_source_owner() {
    let (_source, module, correspondence) = source_and_output();
    let foreign = ProductionSemanticSsaOwnerV1::try_new(
        noop_semantic_owner(&["foreign_root"]),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    check_both(
        &foreign,
        &module,
        foreign.source_semantic().roots(),
        ProductionSemanticKirLimitsV1::default().max_blocks,
        &correspondence,
        Expected::CorrespondenceMismatch,
    );
}

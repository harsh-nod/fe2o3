mod source_output_pipeline_catalog_tests {
    use super::*;
    use fe2o3_kernel_analysis::{
        CanonicalKirInventoryV1 as Inventory,
        KernelIrContractCatalogBindingErrorV1 as BindingError, check_kernel_ir_contract_catalog_v1,
    };
    use fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1 as Catalog;
    use fe2o3_lower_mir_kernel::ProductionSourceOutputOccurrencesV1;

    include!("source_output_pipeline_fixture_v1_tests.rs");
    include!("source_output_pipeline_marker_component_v1_tests.rs");
    include!("source_output_receipt_v1_tests.rs");

    fn ordinary_pipeline_census(source: &ProductionPreRankedKirOwnerV1, owner: &Owner) {
        let semantic = source.semantic_ssa().source_semantic();
        let source_events = semantic
            .functions()
            .iter()
            .flat_map(|function| function.blocks())
            .filter_map(|block| {
                let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                    return None;
                };
                match &semantic.callables()[call.callee().index() as usize] {
                    SemanticCallableDeclV1::CompilerIntrinsic {
                        operation:
                            SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent {
                                event, ..
                            },
                        ..
                    } => Some(*event),
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            source_events,
            vec![
                SemanticWorkgroupPipelineEventV1::Stage,
                SemanticWorkgroupPipelineEventV1::Commit,
                SemanticWorkgroupPipelineEventV1::Wait,
                SemanticWorkgroupPipelineEventV1::Consume,
                SemanticWorkgroupPipelineEventV1::Discard,
                SemanticWorkgroupPipelineEventV1::Release,
            ]
        );
        let mut allocation = 0;
        let mut barrier = 0;
        let mut marker = 0;
        for operation in owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
        {
            match operation.kind {
                fe2o3_kernel_ir::OperationKind::WorkgroupMemory(_) => allocation += 1,
                fe2o3_kernel_ir::OperationKind::WorkgroupBarrier(_) => barrier += 1,
                fe2o3_kernel_ir::OperationKind::VerificationContract(_) => marker += 1,
                _ => {}
            }
        }
        // Ordinary emission keeps the Wait barrier, but does not emit the
        // NativeV12 event markers. This is not six-marker source coverage.
        assert_eq!((allocation, barrier, marker), (1, 1, 0));
    }

    fn pipeline_source(loop_carried: bool) -> ProductionPreRankedKirOwnerV1 {
        let (ssa, launch) = pipeline_fixture(loop_carried);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap()
    }

    fn with_catalog_view(
        source: &ProductionPreRankedKirOwnerV1,
        profile: Profile,
        inspect: impl FnOnce(&ProductionSourceOutputOccurrencesV1<'_, '_>, &mut Budget<'_>),
    ) {
        with_actual(source, profile, |bound, checked, budget| {
            let (coordinates, coordinate_storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    bound,
                    profile,
                    budget,
                )
                .unwrap();
            budget
                .reserve_storage(coordinate_storage.retained_storage())
                .unwrap();
            let (view, view_storage) =
                derive_source_output_occurrences_v1(source, &coordinates, checked, budget).unwrap();
            budget
                .reserve_storage(view_storage.retained_storage())
                .unwrap();
            let floor = budget.storage();
            inspect(&view, budget);
            assert_eq!(budget.storage(), floor);
            drop(view);
            budget
                .release_storage(view_storage.retained_storage())
                .unwrap();
            drop(coordinates);
            budget
                .release_storage(coordinate_storage.retained_storage())
                .unwrap();
        });
    }

    fn check_actual_catalog(
        owner: &Owner,
        catalog: &Catalog,
        marker_count: usize,
        budget: &mut Budget<'_>,
    ) {
        let floor = budget.storage();
        let (inventory, storage) = Inventory::derive(owner, budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (checked, receipt) =
            check_kernel_ir_contract_catalog_v1(&inventory, catalog, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        assert_eq!(checked.marker_count(), marker_count);
        assert!(!checked.grants_authority());
        drop(checked);
        budget.release_storage(receipt.retained_storage()).unwrap();
        drop(inventory);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    }

    #[test]
    fn actual_pipeline_source_catalog_tracks_real_binder_and_checked_output_on_both_targets() {
        for loop_carried in [false, true] {
            let source = pipeline_source(loop_carried);
            ordinary_pipeline_census(&source, source.executable());
            for profile in [Profile::Gfx942, Profile::Gfx950] {
                with_catalog_view(&source, profile, |view, budget| {
                    ordinary_pipeline_census(&source, view.bound());
                    ordinary_pipeline_census(&source, view.output());
                    let input = view.input_pipeline_catalog(budget).unwrap();
                    let output = view.output_pipeline_catalog(budget).unwrap();
                    assert_eq!(
                        input.semantic_source(),
                        source
                            .semantic_ssa()
                            .source_semantic()
                            .semantic_sha256()
                            .as_bytes()
                    );
                    assert_eq!(output.semantic_source(), input.semantic_source());
                    assert_eq!(input.definitions(), output.definitions());
                    assert_eq!(
                        (
                            input.definitions().len(),
                            input.bindings().len(),
                            output.bindings().len()
                        ),
                        (1, 1, 1)
                    );
                    let definition = input.definitions()[0];
                    assert_eq!(
                        (
                            definition.semantic_pipeline_type,
                            definition.semantic_payload_type
                        ),
                        (6, 2)
                    );
                    assert_eq!(
                        (
                            definition.buffers,
                            definition.elements,
                            definition.prefetch_distance
                        ),
                        (2, 64, 1)
                    );
                    assert_eq!(
                        (
                            definition.packed_bits,
                            definition.source_size_bytes,
                            definition.source_alignment_bytes
                        ),
                        (32, 4, 4)
                    );
                    assert!(!input.grants_authority());
                    assert!(!output.grants_authority());
                    assert!(!view.grants_authority());
                    assert_ne!(
                        view.bound().canonical().canonical_bytes(),
                        view.output().canonical().canonical_bytes()
                    );
                    let placements = view.pipeline_allocation_placements(budget).unwrap();
                    assert_eq!(placements.len(), 1);
                    assert_eq!(placements[0].input, input.bindings()[0]);
                    assert_eq!(placements[0].output, Some(output.bindings()[0]));
                    assert_ne!(placements[0].input, placements[0].output.unwrap());
                    let markers = view.pipeline_marker_placements(budget).unwrap();
                    assert!(markers.is_empty());
                    check_actual_catalog(view.bound(), input, 0, budget);
                    check_actual_catalog(view.output(), output, 0, budget);
                });
            }
        }
    }

    #[test]
    fn empty_pipeline_catalog_does_not_claim_ordinary_private_memory_coverage() {
        let source = assertion_materialized(literal_assertion(true, true, false));
        with_catalog_view(&source, Profile::Gfx942, |view, budget| {
            for catalog in [
                view.input_pipeline_catalog(budget).unwrap(),
                view.output_pipeline_catalog(budget).unwrap(),
            ] {
                assert!(catalog.definitions().is_empty());
                assert!(catalog.bindings().is_empty());
                assert!(!catalog.grants_authority());
            }
            assert!(
                view.pipeline_allocation_placements(budget)
                    .unwrap()
                    .is_empty()
            );
            assert!(view.pipeline_marker_placements(budget).unwrap().is_empty());
            assert!(
                view.output()
                    .module()
                    .functions
                    .iter()
                    .filter_map(|f| f.body.as_ref())
                    .flat_map(|body| &body.blocks)
                    .flat_map(|block| &block.operations)
                    .any(|operation| matches!(
                        operation.kind,
                        fe2o3_kernel_ir::OperationKind::Store { .. }
                    ))
            );
        });
    }

    #[test]
    fn actual_output_graph_rejects_shifted_and_wrong_geometry_catalog_candidates() {
        let source = pipeline_source(false);
        with_catalog_view(&source, Profile::Gfx942, |view, budget| {
            let output = view.output_pipeline_catalog(budget).unwrap();
            let (inventory, inventory_storage) = Inventory::derive(view.output(), budget).unwrap();
            budget
                .reserve_storage(inventory_storage.retained_storage())
                .unwrap();
            // Test-owned structured candidate rows, not source-authentication inputs.
            for case in 0..2 {
                let mut definitions = output.definitions().to_vec();
                let mut bindings = output.bindings().to_vec();
                let expected = match case {
                    0 => {
                        bindings[0].operation += 1;
                        "allocation occurrence"
                    }
                    _ => {
                        definitions[0].elements += 1;
                        "allocation geometry or layout"
                    }
                };
                let (candidate, storage) = Catalog::from_rows_with_budget(
                    *output.semantic_source(),
                    &definitions,
                    &bindings,
                    budget,
                )
                .unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                let floor = budget.storage();
                let result = check_kernel_ir_contract_catalog_v1(&inventory, &candidate, budget);
                assert!(
                    matches!(&result,
                    Err(BindingError::Invalid(reason)) if *reason == expected),
                    "case {case}: {result:?}"
                );
                assert_eq!(budget.storage(), floor);
                drop(result);
                drop(candidate);
                budget.release_storage(storage.retained_storage()).unwrap();
            }
            drop(inventory);
            budget
                .release_storage(inventory_storage.retained_storage())
                .unwrap();
        });
    }

    #[test]
    fn graph_catalog_digest_is_not_a_source_authentication_or_lifecycle_proof() {
        let source = pipeline_source(false);
        with_catalog_view(&source, Profile::Gfx942, |view, budget| {
            let actual = view.output_pipeline_catalog(budget).unwrap();
            let mut foreign_source = *actual.semantic_source();
            foreign_source[0] ^= 1;
            let (candidate, storage) = Catalog::from_rows_with_budget(
                foreign_source,
                actual.definitions(),
                actual.bindings(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            assert_ne!(candidate.canonical_bytes(), actual.canonical_bytes());
            // The generic graph checker deliberately cannot authenticate source
            // metadata. The sealed B1 constructor accepts no such caller catalog.
            check_actual_catalog(view.output(), &candidate, 0, budget);
            assert!(!candidate.grants_authority());
            assert_eq!(
                actual.semantic_source(),
                source
                    .semantic_ssa()
                    .source_semantic()
                    .semantic_sha256()
                    .as_bytes()
            );
            drop(candidate);
            budget.release_storage(storage.retained_storage()).unwrap();
        });
        missing_binding_on_marker_free_output_is_not_source_coverage();
    }

    #[test]
    fn retained_catalog_accessors_have_an_independent_four_one_unit_query_boundary() {
        let source = pipeline_source(false);
        with_catalog_view(&source, Profile::Gfx942, |view, budget| {
            let floor = budget.storage();
            for limit in [4, 3] {
                let mut work = Work::new(7 + limit);
                {
                    let mut query = Budget::new(&mut work, floor);
                    query.charge_work(7).unwrap();
                    query.reserve_storage(floor).unwrap();
                    view.input_pipeline_catalog(&mut query).unwrap();
                    view.output_pipeline_catalog(&mut query).unwrap();
                    view.pipeline_allocation_placements(&mut query).unwrap();
                    assert_eq!(query.work(), 10);
                    let last = view.pipeline_marker_placements(&mut query);
                    if limit == 4 {
                        assert!(last.is_ok());
                        assert_eq!(query.work(), 11);
                    } else {
                        assert!(matches!(
                            last,
                            Err(ProductionSourceOutputErrorV1::Resource(
                                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(
                                    _
                                )
                            ))
                        ));
                        assert_eq!(query.work(), 10);
                    }
                    assert_eq!(query.storage(), floor);
                }
                assert_eq!(work.failed_work(), (limit == 3).then_some(11));
            }
        });
    }

    fn missing_binding_on_marker_free_output_is_not_source_coverage() {
        let source = pipeline_source(false);
        with_catalog_view(&source, Profile::Gfx942, |view, budget| {
            ordinary_pipeline_census(&source, view.output());
            let actual = view.output_pipeline_catalog(budget).unwrap();
            assert_eq!(actual.bindings().len(), 1);
            let (candidate, storage) = Catalog::from_rows_with_budget(
                *actual.semantic_source(),
                actual.definitions(),
                &[],
                budget,
            )
            .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            // Graph validation checks supplied allocations and actual markers;
            // absent ordinary markers cannot prove completeness of this catalog.
            // The real B1 source constructor still produced its required binding.
            check_actual_catalog(view.output(), &candidate, 0, budget);
            assert!(!candidate.grants_authority());
            assert_ne!(candidate.canonical_bytes(), actual.canonical_bytes());
            drop(candidate);
            budget.release_storage(storage.retained_storage()).unwrap();
        });
    }
}

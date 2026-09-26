fn current_ordinary_module_owner() -> ProductionSemanticSsaOwnerV1 {
    let original = module_fixture_owner(ModuleFixture::Ordinary);
    let source = original.source_semantic();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        source.functions().to_vec(),
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    assert_ne!(admitted.wire_version(), SemanticMirWireVersionV1::V29);
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn original_ordinary_profile_reaches_source_owned_v18_and_replays_without_conversion() {
    let projection_owner = current_ordinary_module_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let pending = with_module_fixture_view(
        &projection_owner,
        ModuleFixture::Ordinary,
        &mut budget,
        |source, budget| {
            let owner = current_ordinary_module_owner();
            let version = owner.source_semantic().wire_version();
            let sha = *owner.source_semantic_sha256();
            let (_, launch) =
                with_module_fixture_view(&owner, ModuleFixture::Ordinary, budget, |_, _| ())
                    .unwrap();
            let pending = ProductionPendingScopedSourceOwnerV29::try_materialize_with_budget(
                owner,
                launch,
                source.input,
                ProductionSemanticKirLimitsV1::default(),
                budget,
            )
            .unwrap();
            let _: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV18 =
                pending.pending_identity();
            assert_eq!(pending.source_semantic_sha256(), &sha);
            assert_eq!(
                pending.inner.source.owner.source_semantic().wire_version(),
                version
            );
            assert_eq!(pending.pending_module().kernels.len(), 2);
            assert_eq!(pending.assertion_attachment_count(), 2);
            assert!(
                pending
                    .inner
                    .pending
                    .roots
                    .iter()
                    .all(|root| !root.requires_context_issue)
            );
            pending
        },
    )
    .unwrap()
    .0;
    drop(projection_owner);
    let floor = budget.storage();
    let module = pending.pending_module() as *const Module;
    pending.replay_with_budget(&mut budget).unwrap();
    assert_eq!(pending.pending_module() as *const Module, module);
    assert_eq!(budget.storage(), floor);
    let retained = pending.adopted_storage();
    drop(pending);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

#[test]
fn all_roots_share_the_original_source_table_and_full_v18_reconstruction() {
    for kind in [
        ModuleFixture::Ordinary,
        ModuleFixture::Mixed,
        ModuleFixture::Array,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        with_module_fixture(kind, &mut budget, |source, budget| {
            let floor = budget.storage();
            let limits = ProductionSemanticKirLimitsV1::default();
            let pending = admit_pending_scoped_module_v29(source, limits, budget).unwrap();
            with_scoped_source_test_layouts_v29(
                source,
                limits,
                budget,
                |demands, layouts, budget| {
                    let types = demands.types(source.owner, budget)?;
                    let expected = layouts.rows(source.owner, budget)?;
                    assert_eq!(pending.graph.module().storage_layouts.as_slice(), &*expected);
                    for ty in types {
                        let id = layouts.row_for(source.owner, *ty, budget)?;
                        assert!((id.0 as usize) < pending.graph.module().storage_layouts.len());
                    }
                    for ordinal in 0..source.launch.roots().len() {
                        let (requests, _) = demands.root_requests(source.owner, ordinal, budget)?;
                        assert!(
                            requests
                                .iter()
                                .all(|row| row.root
                                    == source.launch.roots()[ordinal].selected_root())
                        );
                    }
                    Ok(())
                },
            )
            .unwrap();
            let before = budget.storage();
            let rows = with_scoped_source_cleanup_v29(budget, before, |cleanup, budget| {
                reconstruct_scoped_source_v29(&pending, source, limits, cleanup, budget)
            })
            .unwrap();
            assert_origin_drop_v1(rows, budget).unwrap();
            drop(pending);
            budget.release_storage(budget.storage() - floor).unwrap();
        })
        .unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn source_reconstruction_rejects_a_structurally_valid_unused_table_extension() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    with_module_fixture(ModuleFixture::Array, &mut budget, |source, budget| {
        let floor = budget.storage();
        let limits = ProductionSemanticKirLimitsV1::default();
        let mut pending = admit_pending_scoped_module_v29(source, limits, budget).unwrap();
        let original_identity = *pending.graph.identity();
        let (mut candidate, candidate_storage) = pending.graph.copy_module_for_transformation_v18(budget).unwrap();
        budget.reserve_storage(candidate_storage.retained_storage()).unwrap();
        assert!(!candidate.storage_layouts.is_empty());
        let old_capacity = candidate.storage_layouts.capacity();
        let width = size_of::<fe2o3_kernel_ir::StorageLayoutV1>();
        budget.reserve_storage(width).unwrap();
        candidate.storage_layouts.try_reserve_exact(1).unwrap();
        let growth = (candidate.storage_layouts.capacity() - old_capacity) * width;
        if growth < width { budget.release_storage(width - growth).unwrap(); }
        else { budget.reserve_storage(growth - width).unwrap(); }
        candidate.storage_layouts.push(fe2o3_kernel_ir::StorageLayoutV1 {
            size: 4, alignment: 4,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::F32),
        });
        let (changed, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::
            from_module_ref_with_verification_budget_v18(&candidate, limits.storage_layout_limits(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(candidate);
        budget.release_storage(candidate_storage.retained_storage() + growth).unwrap();
        assert_ne!(changed.identity(), &original_identity);
        pending.retained_storage = pending.retained_storage - pending.graph_storage.retained_storage()
            + receipt.retained_storage();
        let old = std::mem::replace(&mut pending.graph, changed);
        drop(old);
        budget.release_storage(pending.graph_storage.retained_storage()).unwrap();
        pending.graph_storage = receipt;
        let before = budget.storage();
        assert!(matches!(with_scoped_source_cleanup_v29(budget, before, |cleanup, budget| {
            reconstruct_scoped_source_v29(&pending, source, limits, cleanup, budget)
        }),
            Err(ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { .. }))));
        assert_eq!(budget.storage(), before);
        drop(pending);
        budget.release_storage(budget.storage() - floor).unwrap();
    }).unwrap();
    assert_eq!(budget.storage(), 0);
}

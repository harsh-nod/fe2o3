use super::*;
use fe2o3_amdgcn_model::{
    ProductionTargetCoordinateErrorV1 as CoordinateError,
    check_production_target_coordinate_preservation_v1 as check,
};

fn admit(module: &Module, budget: &mut Budget<'_>) -> (Owner, usize) {
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(module, budget).unwrap();
    let retained = storage.retained_storage();
    budget.reserve_storage(retained).unwrap();
    (owner, retained)
}

#[test]
fn actual_binder_delta_is_exact_for_both_targets_and_non_dense_executables() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for changed in [false, true] {
            let module = neutral(changed);
            let actual = bind_production_target_v1(&module, profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(PREFIX).unwrap();
            let (input, a) = admit(&module, &mut budget);
            let (bound, b) = admit(actual.module(), &mut budget);
            let floor = budget.storage();
            let (coordinates, storage) = check(&input, &bound, profile, &mut budget).unwrap();
            assert!(std::ptr::eq(coordinates.input(), &input));
            assert!(std::ptr::eq(coordinates.output(), &bound));
            assert!(!coordinates.grants_authority());
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let checked = optimize_checked_canonical_kernel_ir_v1(&bound, &mut budget).unwrap();
            assert_eq!(checked.owner().module().kernels, bound.module().kernels);
            assert_eq!(
                checked.owner().module().required_capabilities,
                bound.module().required_capabilities
            );
            assert_eq!(
                checked.report().passes().iter().any(|pass| pass.changed()),
                changed
            );
            drop(checked);
            #[allow(
                clippy::drop_non_drop,
                reason = "End the borrowed witness before releasing its ledger reservation"
            )]
            drop(coordinates);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
            drop(bound);
            budget.release_storage(b).unwrap();
            drop(input);
            budget.release_storage(a).unwrap();
            assert_eq!(budget.storage(), PREFIX);
        }
    }
}

#[test]
fn exact_binder_checker_refuses_wrong_profile_removed_added_and_body_substitutions() {
    let module = neutral(true);
    let actual = bind_production_target_v1(&module, Profile::Gfx942).unwrap();
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    let (input, a) = admit(&module, &mut budget);
    let (bound, b) = admit(actual.module(), &mut budget);
    let floor = budget.storage();
    assert!(matches!(
        check(&input, &bound, Profile::Gfx950, &mut budget),
        Err(CoordinateError::Metadata(_))
    ));
    assert_eq!(budget.storage(), floor);
    let mut candidates = Vec::new();
    let mut missing = actual.module().clone();
    missing.kernels[0]
        .required_capabilities
        .remove(&TargetCapability::WaveWidth(WaveWidth::Wave64));
    candidates.push((missing, false));
    let mut extra = actual.module().clone();
    extra
        .required_capabilities
        .insert(TargetCapability::Extension {
            namespace: "test".into(),
            name: "unissued-capability".into(),
        });
    candidates.push((extra, false));
    let mut wrong_body = actual.module().clone();
    wrong_body.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Constant(Constant::U32(8));
    candidates.push((wrong_body, true));
    for (candidate, body_mismatch) in candidates {
        let (other, retained) = admit(&candidate, &mut budget);
        let before = budget.storage();
        let error = check(&input, &other, Profile::Gfx942, &mut budget).unwrap_err();
        if body_mismatch {
            assert!(matches!(error, CoordinateError::Coordinates(_)));
        } else {
            assert!(matches!(error, CoordinateError::Metadata(_)));
        }
        assert_eq!(budget.storage(), before);
        drop(other);
        budget.release_storage(retained).unwrap();
    }
    drop(bound);
    budget.release_storage(b).unwrap();
    drop(input);
    budget.release_storage(a).unwrap();
    assert_eq!(budget.storage(), 0);
}

mod supplied_native_relation {
    use super::*;
    use fe2o3_amdgcn_model::{
        CheckedNativeV12TargetBindingRelationV1 as Relation,
        NativeV12TargetBindingRelationErrorV1 as RelationError,
        check_native_v12_target_binding_relation_v1 as relation,
    };
    use fe2o3_kernel_analysis::{
        CheckedCanonicalKirCoordinatePreservationV1 as Coordinates,
        CheckedKernelIrContractCatalogV1 as CheckedCatalog,
        check_kernel_ir_contract_catalog_v1 as check_catalog,
    };
    use fe2o3_kernel_ir::{
        AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE as TARGET_NAMESPACE,
        AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME as TARGET_942,
        AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME as TARGET_950, AccessMode, AddressSpace,
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        InertCanonicalKernelIrContractCatalogV1 as Catalog,
        KernelIrPipelineContractDefinitionV1 as Definition,
        KernelIrPipelineStorageBindingV1 as Binding, VerificationContractKeyV12,
        VerificationContractOperationV12, WorkgroupMemory, WorkgroupMemoryExtent,
        WorkgroupPipelineEventKindV12 as Event,
    };

    struct Pair {
        n: Owner,
        b: Owner,
        cn: Catalog,
        cb: Catalog,
        retained: usize,
        cn_retained: usize,
        cb_retained: usize,
    }

    impl Pair {
        fn new(
            n: &Module,
            b: &Module,
            definitions: &[Definition],
            bindings: &[Binding],
            budget: &mut Budget<'_>,
        ) -> Self {
            let (n, ns) = admit(n, budget);
            let (b, bs) = admit(b, budget);
            let (cn, cns) =
                Catalog::from_rows_with_budget([1; 32], definitions, bindings, budget).unwrap();
            budget.reserve_storage(cns.retained_storage()).unwrap();
            // A separately decoded catalog, not a digest-equality test oracle.
            let (cb, cbs) = Catalog::decode_with_budget(cn.canonical_bytes(), budget).unwrap();
            budget.reserve_storage(cbs.retained_storage()).unwrap();
            Self {
                n,
                b,
                cn,
                cb,
                retained: ns + bs + cns.retained_storage() + cbs.retained_storage(),
                cn_retained: cns.retained_storage(),
                cb_retained: cbs.retained_storage(),
            }
        }

        fn replace_bound_catalog(
            &mut self,
            source: [u8; 32],
            definitions: &[Definition],
            bindings: &[Binding],
            budget: &mut Budget<'_>,
        ) {
            let (new, receipt) =
                Catalog::from_rows_with_budget(source, definitions, bindings, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            drop(std::mem::replace(&mut self.cb, new));
            budget.release_storage(self.cb_retained).unwrap();
            self.retained = self.retained - self.cb_retained + receipt.retained_storage();
            self.cb_retained = receipt.retained_storage();
        }

        fn dispose(self, budget: &mut Budget<'_>) {
            let retained = self.retained;
            drop(self);
            budget.release_storage(retained).unwrap();
        }
    }

    fn with_views(
        pair: &Pair,
        budget: &mut Budget<'_>,
        next: impl FnOnce(&CheckedCatalog<'_, '_>, &CheckedCatalog<'_, '_>, &mut Budget<'_>),
    ) {
        let floor = budget.storage();
        let (ni, ns) = CanonicalKirInventoryV1::derive(&pair.n, budget).unwrap();
        budget.reserve_storage(ns.retained_storage()).unwrap();
        let (bi, bs) = CanonicalKirInventoryV1::derive(&pair.b, budget).unwrap();
        budget.reserve_storage(bs.retained_storage()).unwrap();
        let (nc, ncs) = check_catalog(&ni, &pair.cn, budget).unwrap();
        budget.reserve_storage(ncs.retained_storage()).unwrap();
        let (bc, bcs) = check_catalog(&bi, &pair.cb, budget).unwrap();
        budget.reserve_storage(bcs.retained_storage()).unwrap();
        next(&nc, &bc, budget);
        #[allow(
            clippy::drop_non_drop,
            reason = "End both borrowed checked views before releasing their receipts"
        )]
        drop((nc, bc));
        budget
            .release_storage(ncs.retained_storage() + bcs.retained_storage())
            .unwrap();
        drop((ni, bi));
        budget
            .release_storage(ns.retained_storage() + bs.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    }

    fn check_positive(pair: &Pair, profile: Profile, budget: &mut Budget<'_>) {
        with_views(pair, budget, |nc, bc, budget| {
            let floor = budget.storage();
            let (result, receipt) = relation(&pair.n, nc, &pair.b, bc, profile, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert!(std::ptr::eq(result.neutral(), &pair.n));
            assert!(std::ptr::eq(result.bound(), &pair.b));
            assert!(std::ptr::eq(result.coordinates().input(), &pair.n));
            assert!(std::ptr::eq(result.coordinates().output(), &pair.b));
            assert!(std::ptr::eq(result.neutral_catalog(), &pair.cn));
            assert!(std::ptr::eq(result.bound_catalog(), &pair.cb));
            assert_eq!(result.profile(), profile);
            assert!(!result.grants_authority());
            assert!(!result.coordinates().grants_authority());
            assert_eq!(
                receipt.retained_storage(),
                std::mem::size_of::<Relation<'_, '_, '_, '_>>()
            );
            #[allow(
                clippy::drop_non_drop,
                reason = "End the borrowed relation before releasing its receipt"
            )]
            drop(result);
            budget.release_storage(receipt.retained_storage()).unwrap();
        });
    }

    fn multiple_roots() -> Module {
        let mut module = neutral(true);
        let mut helper = BasicBlock::new(BlockId(812));
        helper.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::internal_helper(
            "a_helper",
            Signature::new(vec![], vec![]),
            vec![],
            vec![helper],
        ));
        let mut second = BasicBlock::new(BlockId(73));
        second.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            "z_entry",
            Signature::new(vec![], vec![]),
            vec![],
            vec![second],
        ));
        let mut kernel = Kernel::new(
            "alpha",
            "z_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(32),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
        module.kernels.push(kernel);
        module
    }

    fn contract() -> Definition {
        Definition {
            key: 0,
            semantic_pipeline_type: 9,
            semantic_payload_type: 3,
            buffers: 2,
            elements: 32,
            prefetch_distance: 1,
            packed_bits: 32,
            source_size_bytes: 4,
            source_alignment_bytes: 4,
        }
    }

    // Actual direct allocation rows stay distinct from the block-parameter
    // aliases consumed by all six markers. ValueIds are reused by each function.
    fn pipeline_module(markers: bool) -> (Module, Vec<Binding>) {
        let mut module = Module::new("supplied-pipeline-target");
        module
            .required_capabilities
            .insert(TargetCapability::WorkgroupMemory);
        let mut bindings = Vec::new();
        for (ordinal, name) in ["z_entry", "a_helper"].into_iter().enumerate() {
            let pointer = Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Workgroup,
                AccessMode::ReadWrite,
            );
            let mut entry = BasicBlock::new(BlockId(812));
            entry.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(900), pointer.clone()),
                OperationKind::WorkgroupMemory(WorkgroupMemory {
                    element: Type::Scalar(ScalarType::U32),
                    extent: WorkgroupMemoryExtent::Static(64),
                    alignment: 4,
                }),
            ));
            entry.terminator = Some(Terminator::Branch {
                target: BlockId(7),
                arguments: vec![ValueId(900)],
            });
            let mut carried = BasicBlock::new(BlockId(7));
            carried
                .parameters
                .push(ValueDef::new(ValueId(800), pointer));
            if markers {
                for kind in [
                    Event::Stage,
                    Event::Commit,
                    Event::Wait,
                    Event::Consume,
                    Event::Discard,
                    Event::Release,
                ] {
                    carried.operations.push(Operation::new(
                        vec![],
                        OperationKind::VerificationContract(
                            VerificationContractOperationV12::WorkgroupPipelineEvent {
                                contract: VerificationContractKeyV12::new(0),
                                kind,
                                storage: ValueId(800),
                                epoch: ValueId(444),
                            },
                        ),
                    ));
                }
            }
            carried.terminator = Some(Terminator::Return { values: vec![] });
            let signature = Signature::new(vec![Type::INDEX], vec![]);
            let mut function = if ordinal == 0 {
                Function::kernel_entry(name, signature, vec![ValueId(444)], vec![entry, carried])
            } else {
                Function::internal_helper(name, signature, vec![ValueId(444)], vec![entry, carried])
            };
            function
                .required_capabilities
                .insert(TargetCapability::WorkgroupMemory);
            module.functions.push(function);
            bindings.push(Binding {
                function: u32::try_from(ordinal).unwrap(),
                storage: 900,
                key: 0,
                block: 0,
                operation: 0,
            });
        }
        let mut kernel = Kernel::new(
            "pipeline",
            "z_entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        );
        kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
        kernel
            .required_capabilities
            .insert(TargetCapability::WorkgroupMemory);
        module.kernels.push(kernel);
        (module, bindings)
    }

    #[test]
    fn supplied_relation_checks_both_targets_sparse_helpers_and_nonlexical_roots() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let source = multiple_roots();
            let actual = bind_production_target_v1(&source, profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(PRIOR_WORK).unwrap();
            budget.reserve_storage(PREFIX).unwrap();
            let pair = Pair::new(&source, actual.module(), &[], &[], &mut budget);
            assert_eq!(pair.n.module().kernels.len(), 2);
            assert_eq!(pair.n.module().functions.len(), 3);
            check_positive(&pair, profile, &mut budget);
            pair.dispose(&mut budget);
            assert_eq!(budget.storage(), PREFIX);
        }
    }

    #[test]
    fn supplied_relation_accepts_full_idempotent_binder_without_identity_shortcut() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let first = bind_production_target_v1(&multiple_roots(), profile).unwrap();
            let second = bind_production_target_v1(first.module(), profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            let pair = Pair::new(first.module(), second.module(), &[], &[], &mut budget);
            assert_eq!(
                pair.n.canonical().canonical_bytes(),
                pair.b.canonical().canonical_bytes()
            );
            assert_eq!(pair.n.canonical().identity(), pair.b.canonical().identity());
            assert!(!std::ptr::eq(&pair.n, &pair.b));
            check_positive(&pair, profile, &mut budget);
            // A lookalike identity is not permission to exchange graph-bound views.
            with_views(&pair, &mut budget, |nc, bc, budget| {
                let floor = budget.storage();
                assert!(matches!(
                    relation(&pair.n, bc, &pair.b, nc, profile, budget),
                    Err(RelationError::Invalid("catalog graph owner"))
                ));
                assert!(matches!(
                    relation(&pair.n, bc, &pair.b, bc, profile, budget),
                    Err(RelationError::Invalid("catalog graph owner"))
                ));
                assert!(matches!(
                    relation(&pair.n, nc, &pair.b, nc, profile, budget),
                    Err(RelationError::Invalid("catalog graph owner"))
                ));
                assert_eq!(budget.storage(), floor);
            });
            pair.dispose(&mut budget);
        }
    }

    #[test]
    fn supplied_relation_accepts_nonempty_catalogs_and_exact_alias_uses() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let (source, bindings) = pipeline_module(true);
            let actual = bind_production_target_v1(&source, profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            let pair = Pair::new(
                &source,
                actual.module(),
                &[contract()],
                &bindings,
                &mut budget,
            );
            with_views(&pair, &mut budget, |nc, bc, budget| {
                assert_eq!(nc.marker_count(), 12);
                assert_eq!(bc.marker_count(), 12);
                assert_eq!(nc.catalog().bindings(), bindings);
                assert!(!std::ptr::eq(nc.catalog(), bc.catalog()));
                let (result, receipt) =
                    relation(&pair.n, nc, &pair.b, bc, profile, budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                #[allow(
                    clippy::drop_non_drop,
                    reason = "End the relation before releasing its receipt"
                )]
                drop(result);
                budget.release_storage(receipt.retained_storage()).unwrap();
                // Independently bind the very same catalog object on B.
                let (same, same_receipt) = check_catalog(bc.inventory(), &pair.cn, budget).unwrap();
                budget
                    .reserve_storage(same_receipt.retained_storage())
                    .unwrap();
                let (result, receipt) =
                    relation(&pair.n, nc, &pair.b, &same, profile, budget).unwrap();
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                assert!(std::ptr::eq(
                    result.neutral_catalog(),
                    result.bound_catalog()
                ));
                #[allow(
                    clippy::drop_non_drop,
                    reason = "End the relation and checked view before releasing receipts"
                )]
                drop((result, same));
                budget
                    .release_storage(receipt.retained_storage() + same_receipt.retained_storage())
                    .unwrap();
            });
            pair.dispose(&mut budget);
        }
    }

    #[test]
    fn supplied_relation_refuses_graph_valid_source_metadata_and_binding_drift() {
        for mode in 0..3 {
            let (source, bindings) = pipeline_module(mode != 2);
            let actual = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            let mut pair = Pair::new(
                &source,
                actual.module(),
                &[contract()],
                &bindings,
                &mut budget,
            );
            let mut definition = contract();
            if mode == 1 {
                definition.semantic_pipeline_type += 1;
            }
            let changed_bindings = if mode == 2 { &bindings[..1] } else { &bindings };
            pair.replace_bound_catalog(
                if mode == 0 { [2; 32] } else { [1; 32] },
                &[definition],
                changed_bindings,
                &mut budget,
            );
            // Both independently checked catalogs reach the new byte comparer.
            with_views(&pair, &mut budget, |nc, bc, budget| {
                let floor = budget.storage();
                assert!(matches!(
                    relation(&pair.n, nc, &pair.b, bc, Profile::Gfx942, budget),
                    Err(RelationError::Invalid("complete catalog content"))
                ));
                assert_eq!(budget.storage(), floor);
            });
            pair.dispose(&mut budget);
        }
    }

    #[test]
    fn supplied_relation_does_not_use_graph_identity_as_catalog_source_identity() {
        let source = neutral(false);
        let actual = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let mut pair = Pair::new(&source, actual.module(), &[], &[], &mut budget);
        let n_hash = *pair.n.canonical().identity().digest();
        let b_hash = *pair.b.canonical().identity().digest();
        assert_ne!(n_hash, b_hash);
        let (new, receipt) = Catalog::from_rows_with_budget(n_hash, &[], &[], &mut budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        drop(std::mem::replace(&mut pair.cn, new));
        budget.release_storage(pair.cn_retained).unwrap();
        pair.retained = pair.retained - pair.cn_retained + receipt.retained_storage();
        pair.cn_retained = receipt.retained_storage();
        pair.replace_bound_catalog(b_hash, &[], &[], &mut budget);
        with_views(&pair, &mut budget, |nc, bc, budget| {
            assert!(matches!(
                relation(&pair.n, nc, &pair.b, bc, Profile::Gfx942, budget),
                Err(RelationError::Invalid("complete catalog content"))
            ));
        });
        // Equal arbitrary source claims are only an inert conjunction.
        pair.replace_bound_catalog(n_hash, &[], &[], &mut budget);
        check_positive(&pair, Profile::Gfx942, &mut budget);
        pair.dispose(&mut budget);
    }

    #[test]
    fn supplied_relation_refuses_admitted_body_metadata_and_profile_mutations() {
        let source = multiple_roots();
        let actual = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
        for mode in 0..9 {
            let mut candidate = actual.module().clone();
            match mode {
                0 => {
                    candidate.kernels[0]
                        .required_capabilities
                        .remove(&TargetCapability::WaveWidth(WaveWidth::Wave64));
                }
                1 => {
                    candidate.functions[1]
                        .required_capabilities
                        .insert(gfx942_xnack_minus_target_capability());
                }
                2 => {
                    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
                        OperationKind::Constant(Constant::U32(8));
                }
                3 => {
                    candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results
                        [0]
                    .id = ValueId(18);
                }
                4 => {
                    candidate.kernels[0].workgroup_size = Some(WorkgroupSize::new(32, 1, 1));
                }
                5 => {
                    candidate.kernels.swap(0, 1);
                }
                7 => {
                    let body = candidate.functions[0].body.as_mut().unwrap();
                    let mut exit = BasicBlock::new(BlockId(991));
                    exit.terminator = Some(Terminator::Return { values: vec![] });
                    body.blocks.push(exit);
                    body.blocks[0].terminator = Some(Terminator::Branch {
                        target: BlockId(991),
                        arguments: vec![],
                    });
                }
                8 => {
                    let operation =
                        &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].operations[0];
                    operation.results[0].ty = Type::Scalar(ScalarType::U64);
                    operation.kind = OperationKind::Constant(Constant::U64(7));
                }
                _ => {}
            }
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            // Admission and both catalog checks must succeed before rejection.
            let pair = Pair::new(&source, &candidate, &[], &[], &mut budget);
            with_views(&pair, &mut budget, |nc, bc, budget| {
                let floor = budget.storage();
                assert!(matches!(
                    relation(
                        &pair.n,
                        nc,
                        &pair.b,
                        bc,
                        if mode == 6 {
                            Profile::Gfx950
                        } else {
                            Profile::Gfx942
                        },
                        budget
                    ),
                    Err(RelationError::Target(_))
                ));
                assert_eq!(budget.storage(), floor);
            });
            pair.dispose(&mut budget);
        }
    }

    #[test]
    fn malformed_graphs_and_unbound_markers_are_rejected_before_relation() {
        let mut source = neutral(false);
        source.functions[0].body.as_mut().unwrap().blocks[0].terminator = None;
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        let floor = budget.storage();
        assert!(Owner::from_module_ref_with_verification_budget_v12(&source, &mut budget).is_err());
        assert_eq!(budget.storage(), floor);
        let (source, _) = pipeline_module(true);
        let actual = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
        let pair = Pair::new(&source, actual.module(), &[contract()], &[], &mut budget);
        let (inventory, storage) = CanonicalKirInventoryV1::derive(&pair.n, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        assert!(check_catalog(&inventory, &pair.cn, &mut budget).is_err());
        assert_eq!(budget.storage(), floor);
        drop(inventory);
        budget.release_storage(storage.retained_storage()).unwrap();
        pair.dispose(&mut budget);
    }

    #[test]
    fn supplied_relation_refuses_admitted_edge_operand_and_entry_mapping_changes() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            let mut source = multiple_roots();
            let body = source.functions[0].body.as_mut().unwrap();
            let mut exit = BasicBlock::new(BlockId(991));
            exit.parameters
                .push(ValueDef::new(ValueId(314), Type::Scalar(ScalarType::U32)));
            exit.terminator = Some(Terminator::Return { values: vec![] });
            body.blocks.push(exit);
            body.blocks[0].terminator = Some(Terminator::Branch {
                target: BlockId(991),
                arguments: vec![ValueId(17)],
            });
            let actual = bind_production_target_v1(&source, profile).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(PRIOR_WORK).unwrap();
            budget.reserve_storage(PREFIX).unwrap();
            let pair = Pair::new(&source, actual.module(), &[], &[], &mut budget);
            check_positive(&pair, profile, &mut budget);
            pair.dispose(&mut budget);
            for change_entry in [false, true] {
                let mut candidate = actual.module().clone();
                if change_entry {
                    let first = candidate.kernels[0].entry.clone();
                    candidate.kernels[0].entry = candidate.kernels[1].entry.clone();
                    candidate.kernels[1].entry = first;
                } else {
                    let Some(Terminator::Branch { arguments, .. }) =
                        &mut candidate.functions[0].body.as_mut().unwrap().blocks[0].terminator
                    else {
                        panic!("the actual binder retained the source edge");
                    };
                    arguments[0] = ValueId(93);
                }
                // Each graph is independently admitted and both catalogs bind;
                // only the supplied exact-coordinate comparison must reject it.
                let pair = Pair::new(&source, &candidate, &[], &[], &mut budget);
                with_views(&pair, &mut budget, |nc, bc, budget| {
                    let floor = budget.storage();
                    let error = relation(&pair.n, nc, &pair.b, bc, profile, budget).unwrap_err();
                    let expected = if change_entry {
                        "kernel declaration"
                    } else {
                        "function declaration or executable body"
                    };
                    assert!(matches!(error,
                        RelationError::Target(CoordinateError::Coordinates(
                            fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1::Mismatch(reason)
                        )) if reason == expected
                    ));
                    assert_eq!(budget.storage(), floor);
                });
                pair.dispose(&mut budget);
                assert_eq!(budget.storage(), PREFIX);
            }
        }
    }

    // Independent fixed-fixture derivation: coordinate checker = payload+24;
    // target layer = 38+3*T+2*entry_len. T is the exact extension comparison
    // byte charge. New wrapper adds7 and both56-byte empty catalog payloads.
    fn empty_relation_work(pair: &Pair, profile: Profile) -> usize {
        assert_eq!(pair.n.module().functions.len(), 1);
        assert_eq!(pair.n.module().kernels.len(), 1);
        assert!(pair.n.module().required_capabilities.is_empty());
        assert!(
            pair.n.module().functions[0]
                .required_capabilities
                .is_empty()
        );
        assert!(pair.n.module().kernels[0].required_capabilities.is_empty());
        assert_eq!(pair.b.module().required_capabilities.len(), 2);
        assert_eq!(pair.b.module().functions[0].required_capabilities.len(), 2);
        assert_eq!(pair.b.module().kernels[0].required_capabilities.len(), 2);
        assert_eq!(pair.cn.canonical_bytes().len(), 56);
        assert_eq!(pair.cb.canonical_bytes().len(), 56);
        let target = match profile {
            Profile::Gfx942 => TARGET_942,
            Profile::Gfx950 => TARGET_950,
        };
        let extension_bytes = 2 * (TARGET_NAMESPACE.len() + target.len());
        62 + pair.n.canonical().canonical_bytes().len()
            + pair.b.canonical().canonical_bytes().len()
            + 3 * extension_bytes
            + 2 * pair.n.module().kernels[0].entry.as_str().len()
            + 7
            + 112
    }

    #[test]
    fn supplied_relation_complete_work_exact_under_and_repeated_prefix_are_derived() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for one_short in [false, true] {
                let source = neutral(false);
                let actual = bind_production_target_v1(&source, profile).unwrap();
                let mut work = Work::new(WORK);
                let mut budget = Budget::new(&mut work, STORAGE);
                budget.charge_work(PRIOR_WORK).unwrap();
                budget.reserve_storage(PREFIX).unwrap();
                let pair = Pair::new(&source, actual.module(), &[], &[], &mut budget);
                let exact = empty_relation_work(&pair, profile);
                with_views(&pair, &mut budget, |nc, bc, budget| {
                    let before = budget.work();
                    for _ in 0..2 {
                        let (checked, receipt) =
                            relation(&pair.n, nc, &pair.b, bc, profile, budget).unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        #[allow(
                            clippy::drop_non_drop,
                            reason = "End the relation before releasing its receipt"
                        )]
                        drop(checked);
                        budget.release_storage(receipt.retained_storage()).unwrap();
                    }
                    assert_eq!(budget.work(), before + 2 * exact);
                    // Fill only unused allowance on this original ledger. The
                    // independent operation formula, not measured query work,
                    // determines the exact remaining boundary.
                    let remaining = exact - usize::from(one_short);
                    budget
                        .charge_work(WORK - budget.work() - remaining)
                        .unwrap();
                    let floor = budget.storage();
                    let result = relation(&pair.n, nc, &pair.b, bc, profile, budget);
                    if one_short {
                        assert!(
                            matches!(result, Err(RelationError::Resource(Resource::Work(error))) if error.actual() == WORK + 1 && error.limit() == WORK)
                        );
                        assert_eq!(budget.work(), WORK + 1 - 114);
                    } else {
                        let (checked, receipt) = result.unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        #[allow(
                            clippy::drop_non_drop,
                            reason = "End the relation before releasing its receipt"
                        )]
                        drop(checked);
                        budget.release_storage(receipt.retained_storage()).unwrap();
                        assert_eq!(budget.work(), WORK);
                    }
                    assert_eq!(budget.storage(), floor);
                });
                pair.dispose(&mut budget);
                assert_eq!(budget.storage(), PREFIX);
                assert_eq!(work.failed_work(), one_short.then_some(WORK + 1));
            }
        }
    }

    #[test]
    fn supplied_relation_prepays_wrapper_header_before_owner_or_target_checks() {
        let source = neutral(false);
        let actual = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(PRIOR_WORK).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let pair = Pair::new(&source, actual.module(), &[], &[], &mut budget);
        with_views(&pair, &mut budget, |nc, bc, budget| {
            let header = std::mem::size_of::<Relation<'_, '_, '_, '_>>();
            let coordinate_header = std::mem::size_of::<Coordinates<'_, '_>>();
            let wrapper = header.checked_sub(coordinate_header).unwrap();
            assert!(wrapper > 0);
            let extra = STORAGE - budget.storage() - (wrapper - 1);
            budget.reserve_storage(extra).unwrap();
            let floor = budget.storage();
            let prior = budget.work();
            assert!(matches!(
                relation(&pair.n, bc, &pair.b, nc, Profile::Gfx942, budget),
                Err(RelationError::Resource(Resource::Storage(_)))
            ));
            assert_eq!(budget.work(), prior + 4);
            assert_eq!(budget.failed_storage(), Some(STORAGE + 1));
            assert_eq!(budget.storage(), floor);
            budget.release_storage(extra).unwrap();
        });
        pair.dispose(&mut budget);
        assert_eq!(budget.storage(), PREFIX);
        assert_eq!(work.failed_work(), None);
    }

    #[test]
    fn supplied_relation_inline_storage_exact_under_and_owner_gate_work_boundaries() {
        for one_short in [false, true] {
            let source = neutral(false);
            let actual = bind_production_target_v1(&source, Profile::Gfx942).unwrap();
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(PREFIX).unwrap();
            let pair = Pair::new(&source, actual.module(), &[], &[], &mut budget);
            with_views(&pair, &mut budget, |nc, bc, budget| {
                let header = std::mem::size_of::<Relation<'_, '_, '_, '_>>();
                assert!(header >= std::mem::size_of::<Coordinates<'_, '_>>());
                let extra = STORAGE - budget.storage() - (header - usize::from(one_short));
                budget.reserve_storage(extra).unwrap();
                let floor = budget.storage();
                let result = relation(&pair.n, nc, &pair.b, bc, Profile::Gfx942, budget);
                if one_short {
                    assert!(
                        matches!(result, Err(RelationError::Target(CoordinateError::Coordinates(error)))
                        if matches!(error, fe2o3_kernel_analysis::CanonicalKirCoordinatePreservationErrorV1::Resource(Resource::Storage(_))))
                    );
                    assert_eq!(budget.failed_storage(), Some(STORAGE + 1));
                } else {
                    let (checked, receipt) = result.unwrap();
                    assert_eq!(receipt.retained_storage(), header);
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_eq!(budget.storage(), STORAGE);
                    #[allow(
                        clippy::drop_non_drop,
                        reason = "End the relation before releasing its receipt"
                    )]
                    drop(checked);
                    budget.release_storage(receipt.retained_storage()).unwrap();
                }
                assert_eq!(budget.storage(), floor);
                budget.release_storage(extra).unwrap();
                // Same graph bytes do not repair a catalog whose inventory is
                // bound to the other endpoint; this gate costs exactly4.
                let remaining = 4 - usize::from(one_short);
                budget
                    .charge_work(WORK - budget.work() - remaining)
                    .unwrap();
                let result = relation(&pair.n, bc, &pair.b, nc, Profile::Gfx942, budget);
                if one_short {
                    assert!(
                        matches!(result, Err(RelationError::Resource(Resource::Work(error))) if error.actual() == WORK + 1)
                    );
                } else {
                    assert!(matches!(
                        result,
                        Err(RelationError::Invalid("catalog graph owner"))
                    ));
                    assert_eq!(budget.work(), WORK);
                }
            });
            pair.dispose(&mut budget);
            assert_eq!(budget.storage(), PREFIX);
            assert_eq!(work.failed_work(), one_short.then_some(WORK + 1));
        }
    }
}

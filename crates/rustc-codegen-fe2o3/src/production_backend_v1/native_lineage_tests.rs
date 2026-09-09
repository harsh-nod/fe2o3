struct NativeLineageFixture {
    kernel_ir: fe2o3_compiler_lineage::InertKernelIrReceiptV3,
    final_graph: fe2o3_compiler_lineage::TargetLineageIdentityV3,
    target: ProductionBackendTargetContractV1,
    replay: ProductionBackendLineageReplayV1,
}

fn native_lineage_fixture(target: &str, constant: u32) -> NativeLineageFixture {
    use fe2o3_kernel_ir::{
        Constant, Operation, OperationKind, ScalarType, Type, ValueDef, ValueId,
        VerifiedCanonicalKernelIrV13, WorkgroupSize,
    };

    let (_, mut module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        return_only_v13().into_canonical_bytes(),
    )
    .unwrap();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(Operation::effect_free(
            ValueDef::new(ValueId(0), Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(constant)),
        ));
    let neutral = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let optimized = fe2o3_kernel_opt::optimize_production_kernel_ir_module_v6(&module).unwrap();
    let final_owner =
        VerifiedCanonicalKernelIrV13::from_module(optimized.module().clone()).unwrap();
    let epoch = optimized.report().final_epoch();
    let backend = ProductionBackendTargetV1::from_configured_target(target).unwrap();
    let closure = backend
        .close_semantic_capabilities_v1(&final_owner, epoch)
        .unwrap();
    let llvm = backend
        .lower_v13_module_v1(&final_owner, epoch, &closure)
        .unwrap();
    let llvm = backend.bind_worker_layout_v1(&llvm).unwrap();
    let replay = backend
        .prepare_lineage_replay_v1(
            neutral.canonical_bytes(),
            optimized.module(),
            optimized.report(),
            &llvm,
        )
        .unwrap();
    NativeLineageFixture {
        kernel_ir: fe2o3_compiler_lineage::InertKernelIrReceiptV3::from_canonical_preimage(
            neutral.canonical_bytes(),
        )
        .unwrap(),
        final_graph: fe2o3_compiler_lineage::TargetLineageIdentityV3::new(
            *final_owner.identity().digest(),
            final_owner.identity().canonical_length(),
        )
        .unwrap(),
        target: backend.contract(),
        replay,
    }
}

#[test]
fn native_v13_backend_receipt_crosses_independent_verifier_for_both_targets() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let fixture = native_lineage_fixture(target, 7);
        let receipt = fixture
            .replay
            .validate_frozen_v3(&fixture.kernel_ir, fixture.final_graph, fixture.target)
            .unwrap();
        let identity = (receipt.identity_sha256(), receipt.identity_byte_len());
        let receipt = receipt.into_frozen_v3_receipt();
        assert_eq!(identity.0, *receipt.identity().sha256());
        assert_eq!(identity.1, receipt.identity().byte_len());
        let validated =
            fe2o3_verifier::validate_compiler_kir_to_llvm_replay_v1(&fixture.kernel_ir, &receipt)
                .unwrap();
        assert_eq!(
            validated.replay().llvm_mode(),
            dialect_amdgcn::ProductionKirToLlvmReplayModeV1::CapabilityClosedV13,
        );
        assert_eq!(
            validated.replay().evidence().profile().device_target(),
            target
        );
        assert!(validated.replay().has_exact_target_optimization_replay());
        assert!(validated.has_exact_kir_to_llvm_replay());
        assert!(!validated.has_exact_target_binding_replay());
        assert!(!validated.authenticates_compiler_origin());
        assert!(!validated.establishes_llvm_to_machine_refinement());
        assert!(!validated.grants_runtime_authority());
    }
}

#[test]
fn native_v13_backend_receipt_rejects_source_substitution() {
    let original = native_lineage_fixture("gfx942:xnack-", 7);
    let substituted = native_lineage_fixture("gfx942:xnack-", 8);
    assert_ne!(
        original.kernel_ir.identity(),
        substituted.kernel_ir.identity()
    );
    assert!(matches!(
        original.replay.validate_frozen_v3(
            &substituted.kernel_ir,
            original.final_graph,
            original.target,
        ),
        Err(ProductionBackendErrorV1::LineageReplayValidation(_)),
    ));
}

#[test]
fn native_v13_backend_receipt_rejects_target_and_final_graph_substitution() {
    let fixture = native_lineage_fixture("gfx942:xnack-", 7);
    let other_target = ProductionBackendTargetV1::from_configured_target("gfx950:xnack-")
        .unwrap()
        .contract();
    assert!(matches!(
        fixture
            .replay
            .validate_frozen_v3(&fixture.kernel_ir, fixture.final_graph, other_target,),
        Err(ProductionBackendErrorV1::LineageReplayChanged),
    ));
    let fixture = native_lineage_fixture("gfx942:xnack-", 7);
    let wrong_graph = fe2o3_compiler_lineage::TargetLineageIdentityV3::new(
        [7; 32],
        fixture.final_graph.byte_len(),
    )
    .unwrap();
    assert!(matches!(
        fixture
            .replay
            .validate_frozen_v3(&fixture.kernel_ir, wrong_graph, fixture.target),
        Err(ProductionBackendErrorV1::LineageReplayChanged),
    ));
}

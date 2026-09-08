use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    ProductionTargetCapabilityDiagnosticCodeV1, ProductionTargetCapabilityErrorV1,
    ProductionTargetLaunchEvidenceV13, ProductionV13AmdLoweringErrorV1,
    lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1,
};
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BlockId, ExecutionCapabilityOpV1,
    ExecutionCapabilityOperationV1, ExecutionCapabilityProvenanceV1,
    ExecutionCapabilityRequirementV1, ExecutionCapabilityRoleV1, ExecutionCapabilitySignatureV1,
    ExecutionCapabilitySourceV1, ExecutionCapabilityTypeV1, ExecutionSafetyObligationsV1,
    ExecutionTypeIdentityV1, Function, FunctionId, Kernel, KernelContextSourceIdentityV1,
    KernelContextTypeV1, LaunchDomain, LaunchExtent, Module, Operation, OperationKind, Signature,
    TargetCapability, Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13,
    WorkgroupSize, required_execution_obligations_v1,
};

fn identity(byte: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([byte; 32])
}

fn provenance() -> ExecutionCapabilityProvenanceV1 {
    ExecutionCapabilityProvenanceV1 {
        root: FunctionId::new("entry"),
        kernel_binding: [1; 32],
        frontend_unit: [2; 32],
        kernel_marker: [3; 32],
        target_brand: [4; 32],
        launch_brand: [5; 32],
        issuance: [6; 32],
    }
}

fn capability_operation(width: Option<u32>) -> (ExecutionCapabilityOperationV1, Type) {
    let operation = match width {
        None => ExecutionCapabilityOperationV1::WorkgroupDerive {
            context: identity(10),
            workgroup: identity(11),
        },
        Some(width) => ExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: identity(11),
            subgroup: identity(12),
            width,
        },
    };
    let (source_type, role) = match width {
        None => (identity(11), ExecutionCapabilityRoleV1::Workgroup),
        Some(width) => (identity(12), ExecutionCapabilityRoleV1::Subgroup { width }),
    };
    let ty = Type::ExecutionCapability(ExecutionCapabilityTypeV1 {
        source_type,
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch: Some([8; 32]),
        role,
    });
    (operation, ty)
}

fn module_with_capability(width: Option<u32>, module_name: &str) -> Module {
    let (workgroup_operation, workgroup_type) = capability_operation(None);
    let workgroup_contract = ExecutionCapabilityOpV1 {
        operands: vec![ValueId(0)],
        signature: ExecutionCapabilitySignatureV1::new(&[identity(10)], identity(11)).unwrap(),
        provenance: provenance(),
        workgroup_brand: Some([7; 32]),
        epoch_before: Some([8; 32]),
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &workgroup_operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [9; 32],
            operation: [10; 32],
            block: 0,
        },
        operation: workgroup_operation,
    };
    let mut requirements = workgroup_contract.operation.required_capabilities();
    let mut block = BasicBlock::new(BlockId(0));
    let context = KernelContextTypeV1::new("entry", [3; 32], [4; 32], [5; 32]);
    block.operations.push(Operation::kernel_context_issue(
        ValueId(0),
        context,
        KernelContextSourceIdentityV1::new([21; 32], [22; 32], [23; 32], [24; 32]),
    ));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), workgroup_type),
        OperationKind::ExecutionCapability(workgroup_contract),
    ));
    if let Some(width) = width {
        let (subgroup_operation, subgroup_type) = capability_operation(Some(width));
        requirements.extend(subgroup_operation.required_capabilities());
        let subgroup_contract = ExecutionCapabilityOpV1 {
            operands: vec![ValueId(1)],
            signature: ExecutionCapabilitySignatureV1::new(&[identity(11)], identity(12)).unwrap(),
            provenance: provenance(),
            workgroup_brand: Some([7; 32]),
            epoch_before: Some([8; 32]),
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(
                required_execution_obligations_v1(&subgroup_operation),
            ),
            source: ExecutionCapabilitySourceV1 {
                function: [9; 32],
                operation: [11; 32],
                block: 0,
            },
            operation: subgroup_operation,
        };
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(2), subgroup_type),
            OperationKind::ExecutionCapability(subgroup_contract),
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut entry =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    entry.required_capabilities = requirements.clone();

    let mut kernel = Kernel::new(
        "entry_kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new(module_name);
    module.required_capabilities = requirements;
    module.functions.push(entry);
    module.kernels.push(kernel);
    module
}

fn owner(module: Module) -> VerifiedCanonicalKernelIrV13 {
    VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

#[test]
fn checked_v13_hierarchy_lowers_for_both_exact_amd_profiles() {
    for (profile, cpu) in [
        (ProductionAmdTargetProfileV1::Gfx942, "gfx942"),
        (ProductionAmdTargetProfileV1::Gfx950, "gfx950"),
    ] {
        let owner = owner(module_with_capability(Some(64), cpu));
        let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 3).unwrap();
        let lowered =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 3, &evidence, profile)
                .unwrap();
        assert!(
            lowered
                .llvm_ir()
                .contains(&format!("\"target-cpu\"=\"{cpu}\""))
        );
        assert!(lowered.llvm_ir().contains("+wavefrontsize64"));
        assert_ne!(lowered.capability_closure_identity(), [0; 32]);
        assert!(!lowered.grants_load_authority());
        assert!(!lowered.grants_launch_authority());
    }
}

#[test]
fn v13_wave32_substitution_is_rejected_before_lowering() {
    let owner = owner(module_with_capability(Some(32), "wave32"));
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 4).unwrap();
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let error =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 4, &evidence, profile)
                .unwrap_err();
        let ProductionV13AmdLoweringErrorV1::Capability(error) = error else {
            panic!("Wave32 substitution reached physical lowering for {profile:?}")
        };
        assert_eq!(
            error.diagnostic_code(),
            ProductionTargetCapabilityDiagnosticCodeV1::Unsupported
        );
    }
}

#[test]
fn v13_address_space_axis_substitution_is_rejected_before_lowering() {
    let mut module = module_with_capability(None, "constant-address-substitution");
    let substituted = TargetCapability::Execution(ExecutionCapabilityRequirementV1::AddressSpace {
        address_space: AddressSpace::Constant,
        access: AccessMode::ReadOnly,
    });
    module.required_capabilities.insert(substituted.clone());
    module.functions[0]
        .required_capabilities
        .insert(substituted);
    let owner = owner(module);
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 4).unwrap();
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        let error =
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&owner, 4, &evidence, profile)
                .unwrap_err();
        let ProductionV13AmdLoweringErrorV1::Capability(error) = error else {
            panic!("address-space substitution reached physical lowering for {profile:?}")
        };
        assert_eq!(
            error.diagnostic_code(),
            ProductionTargetCapabilityDiagnosticCodeV1::Unsupported
        );
    }
}

#[test]
fn v13_launch_evidence_is_bound_to_graph_identity_and_epoch() {
    let expected = owner(module_with_capability(None, "expected"));
    let substituted = owner(module_with_capability(None, "substituted"));
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&substituted, 5).unwrap();
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        assert!(matches!(
            lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(&expected, 5, &evidence, profile,),
            Err(ProductionV13AmdLoweringErrorV1::Capability(
                ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch
            ))
        ));
    }
}

#[test]
fn amd_profile_is_a_typed_boundary_not_a_neutral_profile_string() {
    let profiles = [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ];
    assert_eq!(
        profiles.map(ProductionAmdTargetProfileV1::cpu),
        ["gfx942", "gfx950"]
    );
    assert!(
        profiles
            .into_iter()
            .all(|profile| !profile.cpu().contains("synthetic"))
    );
}

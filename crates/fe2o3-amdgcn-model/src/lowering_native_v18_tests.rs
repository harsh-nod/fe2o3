use fe2o3_kernel_ir::{
    AddressSpace, BasicBlock, BlockId, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, LaunchDomain, LaunchExtent, Signature,
    StorageLayoutLimitsV1, UnaryOp, ValueDef, ValueId, VerifiedCanonicalKernelIrModuleV18 as Owner,
    WorkgroupSize,
};

fn module_v18() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
        OperationKind::Unary {
            op: UnaryOp::Not,
            operand: ValueId(0),
        },
    ));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("native_v18_owner");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    module.kernels.push(kernel);
    module
}

fn admit_v18(module: &Module, budget: &mut Budget<'_>) -> (Owner, usize) {
    let (owner, receipt) = Owner::from_module_ref_with_verification_budget_v18(
        module,
        StorageLayoutLimitsV1 {
            rows: 64,
            edges: 256,
            containment_depth: 32,
            object_bytes: 4096,
        },
        budget,
    )
    .unwrap();
    let retained = receipt.retained_storage();
    budget.reserve_storage(retained).unwrap();
    (owner, retained)
}

#[test]
fn native_v18_anchors_reject_same_bytes_foreign_owner_and_historical_identity_pair() {
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    budget.reserve_storage(29).unwrap();
    let module = module_v18();
    let (first, a) = admit_v18(&module, &mut budget);
    let (second, b) = admit_v18(&module, &mut budget);
    assert_eq!(first.canonical_bytes(), second.canonical_bytes());
    let before = (
        budget.work(),
        budget.storage(),
        budget.peak_storage(),
        budget.failed_storage(),
    );
    let identity = ProductionSemanticAnchorKirIdentityV1::from_v18(&first);
    assert_eq!(identity.version(), 18);
    assert_eq!(identity.sha256(), *first.identity().digest());
    assert_eq!(identity.byte_len(), first.identity().canonical_length());
    assert_eq!(
        SemanticAnchorInputV1::NativeV18(&first)
            .validate(first.module())
            .unwrap(),
        identity
    );
    for rejected in [
        SemanticAnchorInputV1::NativeV18(&first).validate(second.module()),
        SemanticAnchorInputV1::NativeV18(&first).validate(&module),
        SemanticAnchorInputV1::Historical(identity).validate(first.module()),
    ] {
        assert!(
            rejected
                .unwrap_err()
                .contains(LoweringDiagnosticCode::SemanticAnchorIdentityMismatch)
        );
    }
    assert_eq!(
        (
            budget.work(),
            budget.storage(),
            budget.peak_storage(),
            budget.failed_storage()
        ),
        before
    );
    drop(second);
    budget.release_storage(b).unwrap();
    drop(first);
    budget.release_storage(a).unwrap();
    assert_eq!(budget.storage(), 29);
}

#[test]
fn native_v18_target_llvm_ir_preserves_exact_owner_geometry_and_anchor_version_on_both_targets() {
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    budget.reserve_storage(29).unwrap();
    let (owner, retained) = admit_v18(&module_v18(), &mut budget);
    let before = (budget.work(), budget.storage());
    for (cpu, llvm) in [
        ("gfx942", lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap()),
        ("gfx950", lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap()),
    ] {
        assert!(llvm.contains("define amdgpu_kernel void @kernel("));
        assert!(llvm.contains(&format!("\"target-cpu\"=\"{cpu}\"")));
        assert!(llvm.contains("\"amdgpu-flat-work-group-size\"=\"64,64\""));
        assert!(llvm.contains("!fe2o3.semantic_anchor.v1 ="));
        assert!(llvm.contains("kir-version:18"));
        assert!(!llvm.contains("kir-version:12"));
        assert!(llvm.contains(&format!("sha256:{}", lower_hex(owner.identity().digest()))));
        assert!(llvm.contains(&format!("target:{cpu}:xnack-")));
        assert!(llvm.contains("i64 1, i64 1}"));
    }
    // Target emission has a separate bounded resource policy.
    assert_eq!((budget.work(), budget.storage()), before);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 29);
}

#[test]
fn native_v18_target_llvm_ir_keeps_geometry_refusal_on_both_targets() {
    let mut module = module_v18();
    module.kernels[0].workgroup_size = None;
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    let (owner, retained) = admit_v18(&module, &mut budget);
    for error in [
        lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &owner,
        )
        .unwrap_err(),
        lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &owner,
        )
        .unwrap_err(),
    ] {
        assert!(error.contains(LoweringDiagnosticCode::MissingWorkgroupSize));
    }
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

fn scalar_metadata_v1763(module: &mut Module) {
    module.storage_layouts = vec![
        fe2o3_kernel_ir::StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        fe2o3_kernel_ir::StorageLayoutV1 {
            size: 0,
            alignment: 1,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Record(Box::new([])),
        },
    ];
}

#[test]
fn native_v18_inert_scalar_unit_metadata_uses_actual_owner_without_legacy_widening() {
    let mut module = module_v18();
    scalar_metadata_v1763(&mut module);
    let mut work = Work::new(1_000_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000_000);
    let (owner, retained) = admit_v18(&module, &mut budget);
    assert_eq!(owner.module().storage_layouts, module.storage_layouts);
    for llvm in [
        lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &owner,
        )
        .unwrap(),
        lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            &owner,
        )
        .unwrap(),
    ] {
        assert!(llvm.contains("kir-version:18"));
        assert!(llvm.contains(&format!("sha256:{}", lower_hex(owner.identity().digest()))));
    }
    assert!(
        reject_unsupported_v12_module(owner.module())
            .unwrap_err()
            .contains(LoweringDiagnosticCode::UnsupportedType)
    );
    assert!(
        fe2o3_kernel_ir::verify_module_ref(owner.module()).is_err(),
        "the new owner route must not widen the legacy raw verifier"
    );
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn native_v18_unused_record_array_and_pointer_metadata_keep_exact_owner_anchors() {
    use fe2o3_kernel_ir::{
        StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Kind, StorageLayoutV1,
        StoragePointerV1,
    };
    for row in [
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Kind::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: Id(0),
                }]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: Kind::Array {
                element: Id(0),
                length: 2,
                stride: 4,
            },
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: Kind::Pointer(StoragePointerV1 {
                pointee: Id(0),
                value_space: AddressSpace::Global,
                encoded_space: AddressSpace::Global,
                access: AccessMode::ReadOnly,
                stored_bits: 64,
            }),
        },
    ] {
        let mut module = module_v18();
        scalar_metadata_v1763(&mut module);
        module.storage_layouts.push(row);
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        let (owner, retained) = admit_v18(&module, &mut budget);
        for llvm in [
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap(),
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap(),
        ] {
            assert!(llvm.contains("kir-version:18"));
            assert!(llvm.contains(&format!("sha256:{}", lower_hex(owner.identity().digest()))));
        }
        assert_eq!(owner.module().storage_layouts, module.storage_layouts);
        assert!(reject_unsupported_v12_module(owner.module()).is_err());
        drop(owner);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[path = "lowering_native_inert_layout_v18_tests.rs"]
mod inert_layout_tests;

#[test]
fn native_v18_dead_storage_type_and_read_are_not_authorized_by_scalar_metadata() {
    use fe2o3_kernel_ir::{MemoryAccess, StorageLayoutIdV1, StorageOperationV1};
    for read in [false, true] {
        let mut module = module_v18();
        scalar_metadata_v1763(&mut module);
        let entry = &mut module.functions[0];
        entry.signature.parameters[0] = Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(0)),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let block = &mut entry.body.as_mut().unwrap().blocks[0];
        block.operations.clear();
        if read {
            block.operations.push(Operation::new(
                vec![ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32))],
                OperationKind::Storage(StorageOperationV1::ReadValue {
                    address: ValueId(0),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                }),
            ));
        }
        let mut work = Work::new(1_000_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000_000);
        let (owner, retained) = admit_v18(&module, &mut budget);
        for error in [
            lower_canonical_v18_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap_err(),
            lower_canonical_v18_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&owner).unwrap_err(),
        ] {
            assert!(error.contains(LoweringDiagnosticCode::UnsupportedType));
        }
        drop(owner);
        budget.release_storage(retained).unwrap();
    }
}

#[test]
fn native_v18_neutral_target_selection_rejects_each_declared_conflict_and_keeps_legacy_rows_required()
 {
    for (target, conflicting) in [
        (
            LoweringTarget::Gfx942XnackMinusV1,
            AMDGPU_GFX950_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        ),
        (
            LoweringTarget::Gfx950XnackMinusV1,
            AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME,
        ),
    ] {
        for site in 0..3 {
            let mut module = module_v18();
            let capability = TargetCapability::Extension {
                namespace: AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE.to_owned(),
                name: conflicting.to_owned(),
            };
            match site {
                0 => {
                    module.required_capabilities.insert(capability);
                }
                1 => {
                    module.kernels[0].required_capabilities.insert(capability);
                }
                _ => {
                    module.functions[0].required_capabilities.insert(capability);
                }
            }
            let mut work = Work::new(1_000_000_000);
            let mut budget = Budget::new(&mut work, 1_000_000_000);
            let (owner, retained) = admit_v18(&module, &mut budget);
            let error = lower_compiler_module_to_llvm_ir_for_target(
                owner.module(),
                target,
                None,
                Some(SemanticAnchorInputV1::NativeV18(&owner)),
                true,
            )
            .unwrap_err();
            assert!(error.contains(LoweringDiagnosticCode::UnsupportedCapability));
            drop(owner);
            budget.release_storage(retained).unwrap();
        }
        let module = module_v18();
        let error = lower_compiler_module_to_llvm_ir_for_target(&module, target, None, None, true)
            .unwrap_err();
        assert!(error.contains(LoweringDiagnosticCode::UnsupportedCapability));
        assert!(error.to_string().contains("lowering requires"));
    }
}

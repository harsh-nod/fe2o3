use fe2o3_amdgcn_model::{
    LoweringDiagnosticCode, LoweringErrors, ProductionSemanticAnchorKirIdentityV1,
    lower_compiler_module_to_gfx942_llvm_ir,
    lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies,
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_compiler_module_to_llvm_ir, lower_device_module_to_gfx942_llvm_ir,
    lower_device_module_to_gfx942_xnack_minus_llvm_ir, lower_kernel_to_gfx942_llvm_ir,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir,
    lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1,
    lower_kernel_to_gfx950_xnack_minus_llvm_ir,
    lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1, lower_kernel_to_llvm_ir,
};
use fe2o3_kernel_ir::*;

fn vector() -> FixedVectorTypeV12 {
    FixedVectorTypeV12::new(ScalarType::F32, 4, VectorLayoutV12::Contiguous)
}

fn scalar_module() -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("v12_consumer");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    ));
    let mut kernel = Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    module.kernels.push(kernel);
    module
}

fn helper(parameters: Vec<Type>, operations: Vec<Operation>) -> Function {
    let values = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect();
    let mut block = BasicBlock::new(BlockId(7));
    block.operations = operations;
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::internal_helper(
        "uncalled",
        Signature::new(parameters, vec![]),
        values,
        vec![block],
    )
}

fn assert_all_public_paths_reject(module: &Module, expected: LoweringDiagnosticCode) {
    verify_module(module).expect("the backend must reject semantically valid V12 input");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000_000);
    let (native, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    for result in [
        fe2o3_amdgcn_model::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(&native),
        fe2o3_amdgcn_model::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(&native),
    ] {
        let errors = result.unwrap_err();
        assert!(errors.contains(expected), "{errors}");
    }
    drop(native);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
    let kernel = KernelId::new("kernel");
    let scalar_owner = VerifiedCanonicalKernelIrV8::from_module(scalar_module()).unwrap();
    let anchor = ProductionSemanticAnchorKirIdentityV1::from_v8(&scalar_owner);
    type KernelLowerer = fn(&Module, &KernelId) -> Result<String, LoweringErrors>;
    type ModuleLowerer = fn(&Module) -> Result<String, LoweringErrors>;
    let kernel_paths: [KernelLowerer; 4] = [
        lower_kernel_to_llvm_ir,
        lower_kernel_to_gfx942_llvm_ir,
        lower_kernel_to_gfx942_xnack_minus_llvm_ir,
        lower_kernel_to_gfx950_xnack_minus_llvm_ir,
    ];
    let module_paths: [ModuleLowerer; 6] = [
        lower_compiler_module_to_llvm_ir,
        lower_compiler_module_to_gfx942_llvm_ir,
        lower_compiler_module_to_gfx942_xnack_minus_llvm_ir,
        lower_compiler_module_to_gfx950_xnack_minus_llvm_ir,
        lower_device_module_to_gfx942_llvm_ir,
        lower_device_module_to_gfx942_xnack_minus_llvm_ir,
    ];
    for lower in kernel_paths {
        let errors = lower(module, &kernel).unwrap_err();
        assert!(errors.contains(expected), "{errors}");
        assert_eq!(
            errors.diagnostics()[0]
                .location
                .function
                .as_ref()
                .unwrap()
                .as_str(),
            "uncalled"
        );
    }
    for lower in module_paths {
        let errors = lower(module).unwrap_err();
        assert!(errors.contains(expected), "{errors}");
    }
    for result in [
        lower_kernel_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            module, &kernel, anchor,
        ),
        lower_kernel_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            module, &kernel, anchor,
        ),
        lower_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            module, anchor,
        ),
        lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
            module, anchor,
        ),
        lower_compiler_module_to_gfx942_llvm_ir_with_launch_policies(module, &[]),
    ] {
        let errors = result.unwrap_err();
        assert!(errors.contains(expected), "{errors}");
    }
}

#[test]
fn uncalled_declarations_reject_vector_parameters_results_and_nested_types() {
    for ty in [
        Type::vector(vector()),
        Type::pointer(
            Type::vector(vector()),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::vector(vector()),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
        Type::slice(
            Type::pointer(
                Type::vector(vector()),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ),
    ] {
        for result in [false, true] {
            let mut module = scalar_module();
            let signature = if result {
                Signature::new(vec![], vec![ty.clone()])
            } else {
                Signature::new(vec![ty.clone()], vec![])
            };
            module
                .functions
                .push(Function::external_import("uncalled", signature));
            assert_all_public_paths_reject(&module, LoweringDiagnosticCode::UnsupportedType);
        }
    }
}

#[test]
fn uncalled_valid_vector_load_is_rejected_as_an_unsupported_operation() {
    let mut module = scalar_module();
    module.functions.push(helper(
        vec![Type::pointer(
            Type::F32,
            AddressSpace::Global,
            AccessMode::ReadOnly,
        )],
        vec![Operation::new(
            vec![ValueDef::new(ValueId(1), Type::vector(vector()))],
            OperationKind::VectorLoad(VectorLoadOperationV12::new(
                ValueId(0),
                VectorMemoryAccessV12::new(vector(), MemoryAccess::new(AddressSpace::Global, 16)),
            )),
        )],
    ));
    assert_all_public_paths_reject(&module, LoweringDiagnosticCode::UnsupportedOperation);
}

#[test]
fn valid_store_and_layout_conversion_cannot_hide_behind_dead_vector_inputs() {
    let access = VectorMemoryAccessV12::new(vector(), MemoryAccess::new(AddressSpace::Global, 16));
    for operations in [
        vec![Operation::new(
            vec![],
            OperationKind::VectorStore(VectorStoreOperationV12::new(
                ValueId(0),
                ValueId(1),
                access,
            )),
        )],
        vec![Operation::new(
            vec![ValueDef::new(
                ValueId(2),
                Type::vector(vector().with_layout(VectorLayoutV12::Interleaved { factor: 2 })),
            )],
            OperationKind::VectorLayoutConvert(VectorLayoutConversionV12::new(
                ValueId(1),
                VectorLayoutV12::Interleaved { factor: 2 },
            )),
        )],
    ] {
        let mut module = scalar_module();
        module.functions.push(helper(
            vec![
                Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite),
                Type::vector(vector()),
            ],
            operations,
        ));
        assert_all_public_paths_reject(&module, LoweringDiagnosticCode::UnsupportedType);
    }
}

#[test]
fn dead_vector_alloca_and_unreachable_block_arguments_are_rejected() {
    let mut module = scalar_module();
    module.functions.push(helper(
        vec![],
        vec![Operation::new(
            vec![ValueDef::new(
                ValueId(0),
                Type::pointer(
                    Type::vector(vector()),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            )],
            OperationKind::Alloca {
                element: Type::vector(vector()),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 16,
            },
        )],
    ));
    assert_all_public_paths_reject(&module, LoweringDiagnosticCode::UnsupportedType);

    let mut module = scalar_module();
    let mut function = helper(vec![], vec![]);
    let mut dead = BasicBlock::new(BlockId(99));
    dead.parameters
        .push(ValueDef::new(ValueId(99), Type::vector(vector())));
    dead.terminator = Some(Terminator::Return { values: vec![] });
    function.body.as_mut().unwrap().blocks.push(dead);
    module.functions.push(function);
    assert_all_public_paths_reject(&module, LoweringDiagnosticCode::UnsupportedType);
}

#[test]
fn every_valid_marker_kind_is_explicitly_unsupported_even_in_an_uncalled_helper() {
    for kind in [
        WorkgroupPipelineEventKindV12::Stage,
        WorkgroupPipelineEventKindV12::Commit,
        WorkgroupPipelineEventKindV12::Wait,
        WorkgroupPipelineEventKindV12::Consume,
        WorkgroupPipelineEventKindV12::Discard,
        WorkgroupPipelineEventKindV12::Release,
    ] {
        let mut module = scalar_module();
        module.functions.push(helper(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::I32),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
                Type::INDEX,
            ],
            vec![Operation::new(
                vec![],
                OperationKind::VerificationContract(
                    VerificationContractOperationV12::WorkgroupPipelineEvent {
                        contract: VerificationContractKeyV12::new(0),
                        kind,
                        storage: ValueId(0),
                        epoch: ValueId(1),
                    },
                ),
            )],
        ));
        assert_all_public_paths_reject(&module, LoweringDiagnosticCode::UnsupportedOperation);
    }
}

#[test]
fn malformed_vector_input_still_reports_verification_errors_without_emission() {
    let mut module = scalar_module();
    module.functions.push(helper(
        vec![],
        vec![Operation::new(
            vec![],
            OperationKind::VectorLoad(VectorLoadOperationV12::new(
                ValueId(99),
                VectorMemoryAccessV12::new(vector(), MemoryAccess::new(AddressSpace::Global, 16)),
            )),
        )],
    ));
    let errors = lower_kernel_to_llvm_ir(&module, &KernelId::new("kernel")).unwrap_err();
    assert!(errors.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic.code,
        LoweringDiagnosticCode::InputVerification(_)
    )));
}

#[test]
fn existing_scalar_lowering_still_emits_llvm() {
    let llvm = lower_kernel_to_llvm_ir(&scalar_module(), &KernelId::new("kernel")).unwrap();
    assert!(llvm.contains("define amdgpu_kernel void @kernel"));
}

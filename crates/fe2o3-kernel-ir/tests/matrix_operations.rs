use std::collections::BTreeSet;

use fe2o3_kernel_ir::*;

fn pointer(element: Type, access: AccessMode) -> Type {
    Type::pointer(element, AddressSpace::Workgroup, access)
}

fn matrix_module() -> Module {
    let parameters = vec![Type::F32, Type::F32, Type::F32, Type::F32];
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();

    let allocation = |id: u32, element: Type| {
        Operation::new(
            vec![ValueDef::new(
                ValueId(id),
                pointer(element.clone(), AccessMode::ReadWrite),
            )],
            OperationKind::WorkgroupMemory(WorkgroupMemory {
                element,
                extent: WorkgroupMemoryExtent::Static(256),
                alignment: 16,
            }),
        )
    };
    let load_a = MatrixOperation::lds_load(ValueId(4), MatrixElement::Bf16);
    let load_b = MatrixOperation::lds_load(ValueId(5), MatrixElement::Bf16);
    let mma = MatrixOperation::multiply_accumulate(
        [ValueId(7), ValueId(8), ValueId(9), ValueId(10)],
        [ValueId(11), ValueId(12), ValueId(13), ValueId(14)],
        [ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
    );
    let store = MatrixOperation::lds_store(
        ValueId(6),
        [ValueId(15), ValueId(16), ValueId(17), ValueId(18)],
        MatrixElement::F32,
    );

    let mut next = 7;
    let mut matrix_op = |matrix: MatrixOperation| {
        let results = matrix
            .result_types()
            .into_iter()
            .map(|ty| {
                let result = ValueDef::new(ValueId(next), ty);
                next += 1;
                result
            })
            .collect();
        Operation::new(results, OperationKind::Matrix(matrix))
    };
    let operations = vec![
        allocation(4, Type::Scalar(ScalarType::Bf16)),
        allocation(5, Type::Scalar(ScalarType::Bf16)),
        allocation(6, Type::F32),
        matrix_op(load_a),
        matrix_op(load_b),
        matrix_op(mma),
        matrix_op(store),
    ];

    let mut function = Function::kernel_entry(
        "matrix_impl",
        Signature::new(parameters, vec![]),
        parameter_ids,
        vec![BasicBlock {
            id: BlockId(0),
            parameters: vec![],
            operations,
            terminator: Some(Terminator::Return { values: vec![] }),
        }],
    );
    function.required_capabilities = function.derived_capabilities();

    let mut kernel = Kernel::new(
        "matrix_kernel",
        "matrix_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));

    let mut module = Module::new("tests::matrix");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn operation_mut(module: &mut Module, index: usize) -> &mut MatrixOperation {
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .iter_mut()
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix) => Some(matrix),
            _ => None,
        })
        .nth(index)
        .expect("expected matrix operation")
}

fn allocation_mut(module: &mut Module, index: usize) -> &mut WorkgroupMemory {
    let operation = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[index];
    let OperationKind::WorkgroupMemory(memory) = &mut operation.kind else {
        panic!("expected workgroup-memory allocation")
    };
    memory
}

#[test]
fn exact_matrix_and_lds_profiles_verify_with_explicit_effects() {
    let module = matrix_module();
    verify_module(&module).unwrap();

    let matrix_operations = module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Matrix(_)))
        .collect::<Vec<_>>();
    assert_eq!(
        matrix_operations[0].memory_effects(),
        vec![MemoryEffect::Read(AddressSpace::Workgroup)]
    );
    assert!(matrix_operations[2].effect_summary().is_pure());
    assert_eq!(
        matrix_operations[3].memory_effects(),
        vec![MemoryEffect::Write(AddressSpace::Workgroup)]
    );

    let capabilities = module.functions[0].derived_capabilities();
    assert!(capabilities.contains(&TargetCapability::BFloat16));
    assert!(capabilities.contains(&TargetCapability::WorkgroupMemory));
    assert!(capabilities.contains(&TargetCapability::WaveWidth(WaveWidth::Wave64)));
    assert!(capabilities.contains(&TargetCapability::Extension {
        namespace: MATRIX_CAPABILITY_NAMESPACE.to_string(),
        name: BF16_F32_M16N16K16_CAPABILITY.to_string(),
    }));
    assert!(capabilities.contains(&TargetCapability::Extension {
        namespace: MATRIX_CAPABILITY_NAMESPACE.to_string(),
        name: LDS_TILE_16X16_XOR4_CAPABILITY.to_string(),
    }));
}

#[test]
fn profile_shape_wave_and_convergence_mutations_fail_closed() {
    let mut shape = matrix_module();
    let MatrixOperationKind::MultiplyAccumulate { profile, .. } =
        &mut operation_mut(&mut shape, 2).kind
    else {
        panic!()
    };
    profile.m = 32;
    let errors = verify_module(&shape).unwrap_err();
    assert!(errors.contains(DiagnosticCode::InvalidSemanticOperation));

    let mut partial = matrix_module();
    operation_mut(&mut partial, 2).active_lanes = 32;
    assert!(
        verify_module(&partial)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidSemanticOperation)
    );

    let mut wrong_scope = matrix_module();
    operation_mut(&mut wrong_scope, 0).convergence =
        Convergence::uniform(SynchronizationScope::Workgroup);
    assert!(
        verify_module(&wrong_scope)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidSemanticOperation)
    );

    let mut wrong_tile = matrix_module();
    let MatrixOperationKind::LdsLoad { profile, .. } = &mut operation_mut(&mut wrong_tile, 0).kind
    else {
        panic!()
    };
    profile.fragment_elements = 8;
    assert!(
        verify_module(&wrong_tile)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidSemanticOperation)
    );
}

#[test]
fn pointer_value_and_result_type_mutations_fail_closed() {
    let mut global = matrix_module();
    global.functions[0].body.as_mut().unwrap().blocks[0].operations[0].results[0].ty =
        Type::pointer(
            Type::Scalar(ScalarType::Bf16),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
    assert!(
        verify_module(&global)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );

    let mut read_only_store = matrix_module();
    read_only_store.functions[0].body.as_mut().unwrap().blocks[0].operations[2].results[0].ty =
        pointer(Type::F32, AccessMode::ReadOnly);
    assert!(
        verify_module(&read_only_store)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );

    let mut wrong_accumulator = matrix_module();
    wrong_accumulator.functions[0].signature.parameters[0] = Type::Scalar(ScalarType::Bf16);
    assert!(
        verify_module(&wrong_accumulator)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidOperandType)
    );

    let mut wrong_result = matrix_module();
    wrong_result.functions[0].body.as_mut().unwrap().blocks[0].operations[5].results[0].ty =
        Type::Scalar(ScalarType::Bf16);
    assert!(
        verify_module(&wrong_result)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );
}

#[test]
fn matrix_lds_requires_authenticated_extent_alignment_and_provenance() {
    for extent in [
        WorkgroupMemoryExtent::Static(255),
        WorkgroupMemoryExtent::Dynamic,
        WorkgroupMemoryExtent::DynamicAtLeast(255),
    ] {
        let mut short = matrix_module();
        allocation_mut(&mut short, 0).extent = extent;
        assert!(
            verify_module(&short)
                .unwrap_err()
                .contains(DiagnosticCode::InvalidMemoryAccess)
        );
    }

    let mut under_aligned = matrix_module();
    allocation_mut(&mut under_aligned, 0).alignment = 1;
    assert!(
        verify_module(&under_aligned)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidAlignment)
    );

    let mut wrong_provenance = matrix_module();
    wrong_provenance.functions[0]
        .signature
        .parameters
        .push(pointer(
            Type::Scalar(ScalarType::Bf16),
            AccessMode::ReadOnly,
        ));
    wrong_provenance.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(19));
    let MatrixOperationKind::LdsLoad { base, .. } =
        &mut operation_mut(&mut wrong_provenance, 0).kind
    else {
        panic!()
    };
    *base = ValueId(19);
    assert!(
        verify_module(&wrong_provenance)
            .unwrap_err()
            .contains(DiagnosticCode::InvalidMemoryAccess)
    );

    let mut bounded_dynamic = matrix_module();
    allocation_mut(&mut bounded_dynamic, 0).extent = WorkgroupMemoryExtent::DynamicAtLeast(256);
    bounded_dynamic.functions[0].required_capabilities =
        bounded_dynamic.functions[0].derived_capabilities();
    verify_module(&bounded_dynamic).unwrap();
}

#[test]
fn target_capability_admission_requires_both_matrix_extensions() {
    let module = matrix_module();
    let all = module.functions[0].derived_capabilities();
    verify_module_with_capabilities(&module, &all).unwrap();

    for extension in [
        BF16_F32_M16N16K16_CAPABILITY,
        LDS_TILE_16X16_XOR4_CAPABILITY,
    ] {
        let mut missing = all.clone();
        missing.remove(&TargetCapability::Extension {
            namespace: MATRIX_CAPABILITY_NAMESPACE.to_string(),
            name: extension.to_string(),
        });
        assert!(
            verify_module_with_capabilities(&module, &missing)
                .unwrap_err()
                .contains(DiagnosticCode::UnsupportedCapability)
        );
    }

    let baseline = BTreeSet::from([
        TargetCapability::BFloat16,
        TargetCapability::Subgroups,
        TargetCapability::SubgroupSize(64),
        TargetCapability::WaveWidth(WaveWidth::Wave64),
        TargetCapability::WorkgroupMemory,
    ]);
    assert!(
        verify_module_with_capabilities(&module, &baseline)
            .unwrap_err()
            .contains(DiagnosticCode::UnsupportedCapability)
    );
}

#[test]
fn frozen_module_wire_versions_reject_matrix_authority() {
    assert!(matches!(
        encode_module_v1(&matrix_module()),
        Err(KernelIrEncodeError::UnsupportedInVersion {
            feature: "explicit workgroup memory",
            ..
        })
    ));
    for encoded in [
        encode_module_v2(&matrix_module()),
        encode_module_v3(&matrix_module()),
    ] {
        assert!(matches!(
            encoded,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                feature: "matrix operation",
                ..
            })
        ));
    }
}

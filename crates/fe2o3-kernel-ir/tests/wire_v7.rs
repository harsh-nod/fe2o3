use fe2o3_kernel_ir::*;

const GUARDED_POINTER: ValueId = ValueId(0x1020_3040);
const GUARDED_PREDICATE: ValueId = ValueId(0x5060_7080);
const GUARDED_FALLBACK: ValueId = ValueId(0x90a0_b0c0);
const GUARDED_RESULT: ValueId = ValueId(0xd0e0_f001);

fn tensor_layout_fixture() -> Module {
    let parameters = vec![Type::F32, Type::F32, Type::F32, Type::F32];
    let parameter_ids = (0..parameters.len())
        .map(|index| ValueId(index as u32))
        .collect::<Vec<_>>();
    let allocation = |id: u32, element: Type| {
        Operation::new(
            vec![ValueDef::new(
                ValueId(id),
                Type::pointer(
                    element.clone(),
                    AddressSpace::Workgroup,
                    AccessMode::ReadWrite,
                ),
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
    )
    .with_declared_tensor_layout(TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64());
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
        "layout_impl",
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
        "layout",
        "layout_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new("tensor-layout-wire-v7");
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn guarded_load_fixture(predicate_type: Type, fallback_type: Type) -> Module {
    let access = MemoryAccess::new(AddressSpace::Global, 2);
    let guarded = Operation::new(
        vec![ValueDef::new(
            GUARDED_RESULT,
            Type::Scalar(ScalarType::Bf16),
        )],
        OperationKind::GuardedLoad {
            pointer: GUARDED_POINTER,
            predicate: GUARDED_PREDICATE,
            fallback: GUARDED_FALLBACK,
            access,
        },
    );
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(guarded);
    block.terminator = Some(Terminator::Return { values: vec![] });

    let mut module = Module::new("guarded-load-wire-v7");
    module.functions.push(Function::definition(
        "guarded_load",
        Signature::new(
            vec![
                Type::pointer(
                    Type::Scalar(ScalarType::Bf16),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                predicate_type,
                fallback_type,
            ],
            vec![],
        ),
        vec![GUARDED_POINTER, GUARDED_PREDICATE, GUARDED_FALLBACK],
        vec![block],
    ));
    module
}

fn matrix_mut(module: &mut Module) -> &mut MatrixOperation {
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.kind {
            OperationKind::Matrix(matrix)
                if matches!(matrix.kind, MatrixOperationKind::MultiplyAccumulate { .. }) =>
            {
                Some(matrix)
            }
            _ => None,
        })
        .expect("fixture has one matrix multiply")
}

#[test]
fn v7_round_trips_the_complete_checked_tensor_contract() {
    assert_eq!(KERNEL_IR_VERSION_V7, 7);
    assert_eq!(KERNEL_IR_DOMAIN_V7, b"FE2O3/KERNEL-IR/V7\0");
    let module = tensor_layout_fixture();
    verify_module(&module).unwrap();

    let first = encode_module_v7(&module).unwrap();
    let second = encode_module_v7(&module).unwrap();
    assert_eq!(first, second);
    assert_eq!(first[8..10], KERNEL_IR_VERSION_V7.to_le_bytes());
    assert_eq!(decode_module_v7(&first).unwrap(), module);

    let owner = VerifiedCanonicalKernelIrV7::from_module(module).unwrap();
    owner.revalidate().unwrap();
    assert_eq!(owner.canonical_bytes(), first);
}

#[test]
fn v7_preserves_independent_storage_transforms_tail_and_opaque_ids() {
    let mut module = tensor_layout_fixture();
    let contract = matrix_mut(&mut module).tensor_layout.as_mut().unwrap();
    contract.a.lds_swizzle = TensorLdsSwizzleV1::Xor4;
    contract.b.lds_swizzle = TensorLdsSwizzleV1::None;
    contract.tail_mask = TensorTailMaskV1::ZeroFilledPredicateInputs;
    contract.a.mapping = TensorSymbolicMapV1::Opaque(u32::MAX);

    let bytes = encode_module_v7(&module).unwrap();
    assert_eq!(decode_module_v7(&bytes).unwrap(), module);
}

#[test]
fn frozen_v5_v6_reject_layout_and_legacy_matrix_decode_is_unverified() {
    let module = tensor_layout_fixture();
    for (version, result) in [
        (KERNEL_IR_VERSION_V5, encode_module_v5(&module)),
        (KERNEL_IR_VERSION_V6, encode_module_v6(&module)),
    ] {
        assert_eq!(
            result,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "tensor layout contract",
            })
        );
    }

    let mut legacy = module;
    matrix_mut(&mut legacy).tensor_layout = None;
    let v6 = encode_module_v6(&legacy).unwrap();
    let decoded = decode_module_v7(&v6).unwrap();
    assert_eq!(decoded, legacy);
    assert!(
        verify_module(&decoded)
            .unwrap_err()
            .to_string()
            .contains("explicit tensor layout contract")
    );

    let v7 = encode_module_v7(&tensor_layout_fixture()).unwrap();
    assert_eq!(
        decode_module_v6(&v7),
        Err(KernelIrDecodeError::UnknownVersion(KERNEL_IR_VERSION_V7))
    );
}

#[test]
fn guarded_load_is_v7_only_deterministic_and_retains_its_conditional_effect() {
    let module = guarded_load_fixture(Type::BOOL, Type::Scalar(ScalarType::Bf16));
    verify_module(&module).unwrap();
    let operation = &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0];
    assert_eq!(
        operation.kind.operands(),
        vec![GUARDED_POINTER, GUARDED_PREDICATE, GUARDED_FALLBACK]
    );
    assert_eq!(
        operation.memory_effects(),
        vec![MemoryEffect::Read(AddressSpace::Global)]
    );

    let first = encode_module_v7(&module).unwrap();
    let second = encode_module_v7(&module).unwrap();
    assert_eq!(first, second);
    assert_eq!(decode_module_v7(&first).unwrap(), module);

    for (version, result) in [
        (KERNEL_IR_VERSION_V5, encode_module_v5(&module)),
        (KERNEL_IR_VERSION_V6, encode_module_v6(&module)),
    ] {
        assert_eq!(
            result,
            Err(KernelIrEncodeError::UnsupportedInVersion {
                version,
                feature: "guarded load",
            })
        );
    }
}

#[test]
fn guarded_load_rejects_non_boolean_predicates_and_wrong_fallback_types() {
    let bad_predicate = guarded_load_fixture(Type::F32, Type::Scalar(ScalarType::Bf16));
    assert!(
        verify_module(&bad_predicate)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );

    let bad_fallback = guarded_load_fixture(Type::BOOL, Type::F32);
    assert!(
        verify_module(&bad_fallback)
            .unwrap_err()
            .contains(DiagnosticCode::TypeMismatch)
    );
}

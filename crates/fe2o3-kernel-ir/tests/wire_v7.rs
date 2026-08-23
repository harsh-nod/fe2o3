use fe2o3_kernel_ir::*;

const GUARDED_POINTER: ValueId = ValueId(0x1020_3040);
const GUARDED_PREDICATE: ValueId = ValueId(0x5060_7080);
const GUARDED_FALLBACK: ValueId = ValueId(0x90a0_b0c0);
const GUARDED_RESULT: ValueId = ValueId(0xd0e0_f001);

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
    let module = tiled_gemm_v1_module();
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
    let mut module = tiled_gemm_v1_module();
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
    let module = tiled_gemm_v1_module();
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

    let v7 = encode_module_v7(&tiled_gemm_v1_module()).unwrap();
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

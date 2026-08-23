use fe2o3_kernel_ir::*;

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

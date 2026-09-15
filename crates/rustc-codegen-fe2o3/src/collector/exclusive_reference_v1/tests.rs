use super::*;

fn root() -> ReferenceFunctionIdentityV1 {
    ReferenceFunctionIdentityV1 {
        def_path_hash: [8; 16],
        function_sha256: [9; 32],
        item_definition_sha256: [10; 32],
        monomorphization_sha256: [11; 32],
        generic_type_arguments_sha256: [12; 32],
        const_generic_arguments_sha256: [13; 32],
        rustc_mir_body_sha256: [10; 32],
    }
}

#[test]
fn exclusive_reference_witness_requires_exact_root_and_body() {
    let source = ExclusiveOutputSourceV1::test_only_v1(9, 1, ReferenceScalarTypeV1::F32);
    let root = root();
    assert!(source.matches_binding_v1(&root, 1, ReferenceScalarTypeV1::F32));
    for mutation in 0..2 {
        let mut changed = root;
        if mutation == 0 {
            changed.function_sha256[0] ^= 1;
        } else {
            changed.rustc_mir_body_sha256[0] ^= 1;
        }
        assert!(!source.matches_binding_v1(&changed, 1, ReferenceScalarTypeV1::F32));
    }
}

#[test]
fn exclusive_reference_witness_cannot_transfer_to_another_argument_or_primitive() {
    let source = ExclusiveOutputSourceV1::test_only_v1(9, 1, ReferenceScalarTypeV1::U32);
    let root = root();
    for (argument, scalar) in [
        (0, ReferenceScalarTypeV1::U32),
        (2, ReferenceScalarTypeV1::U32),
        (1, ReferenceScalarTypeV1::I32),
        (1, ReferenceScalarTypeV1::U64),
        (1, ReferenceScalarTypeV1::F32),
    ] {
        assert!(!source.matches_binding_v1(&root, argument, scalar));
    }
}

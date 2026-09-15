use super::*;

#[allow(dead_code)]
mod fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../fe2o3-amdgcn-model/src/lowering/v13/reusable_phase_tests/fixture.rs"
    ));
}

fn phase() -> KirOperation {
    fixture::memory(2)
        .functions
        .into_iter()
        .filter_map(|f| f.body)
        .flat_map(|b| b.blocks)
        .flat_map(|b| b.operations)
        .find(|o| matches!(o.kind, OperationKind::ReusablePhase(_)))
        .unwrap()
}

fn token_type() -> KirType {
    fixture::memory(2)
        .functions
        .into_iter()
        .filter_map(|f| f.body)
        .flat_map(|b| b.blocks)
        .flat_map(|b| b.operations)
        .flat_map(|o| o.results)
        .find_map(|r| matches!(r.ty, KirType::ReusablePhaseToken(_)).then_some(r.ty))
        .unwrap()
}

fn raw_type(bytes: &[u8]) -> PhaseValueTypeV14 {
    PhaseValueTypeV14(StringAttr::new(encode_hex(bytes)))
}

#[test]
fn declared_v14_phase_fragment_has_exact_header_and_rejects_v13_relabel() {
    let original = phase();
    let bytes = encode_operation_declared(&original, CanonicalKernelIrVersionV1::V14).unwrap();
    assert_eq!(&bytes[..8], &fe2o3_kernel_ir::KERNEL_IR_MAGIC_V1);
    assert_eq!(&bytes[8..10], &14u16.to_le_bytes());
    assert_eq!(
        decode_operation_declared(&bytes, CanonicalKernelIrVersionV1::V14),
        Some(original.clone())
    );
    assert!(encode_operation(&original).is_none());
    let mut relabeled = bytes.clone();
    relabeled[8..10].copy_from_slice(&13u16.to_le_bytes());
    assert!(decode_operation(&relabeled).is_none());
    assert!(decode_operation_declared(&relabeled, CanonicalKernelIrVersionV1::V14).is_none());
    assert!(decode_operation(&bytes).is_none());
}

#[test]
fn declared_v14_all_token_and_extended_capability_types_roundtrip() {
    let mut seen_tokens = 0;
    let mut seen_caps = 0;
    for ty in fixture::memory(2)
        .functions
        .into_iter()
        .filter_map(|f| f.body)
        .flat_map(|b| b.blocks)
        .flat_map(|b| b.operations)
        .flat_map(|o| o.results)
        .map(|r| r.ty)
    {
        if !PhaseValueTypeV14::supports(&ty) {
            continue;
        }
        let envelope = type_envelope(ty.clone());
        let bytes = encode_module_v14(&envelope).unwrap();
        assert_eq!(raw_type(&bytes).kir_type(), Some(ty.clone()));
        assert!(encode_module_v13(&envelope).is_err());
        match ty {
            KirType::ReusablePhaseToken(_) => seen_tokens += 1,
            KirType::ExecutionCapability(_) => seen_caps += 1,
            _ => panic!("only explicitly closed logical roles"),
        }
    }
    assert!(seen_tokens > 5 && seen_caps >= 3);
}

#[test]
fn declared_v14_type_rejects_version_suffix_and_envelope_substitution() {
    let ty = token_type();
    let bytes = encode_module_v14(&type_envelope(ty)).unwrap();
    assert!(raw_type(&bytes).kir_type().is_some());
    for tag in [0u16, 13, 15] {
        let mut changed = bytes.clone();
        changed[8..10].copy_from_slice(&tag.to_le_bytes());
        assert!(raw_type(&changed).kir_type().is_none());
    }
    let mut suffix = bytes.clone();
    suffix.push(0);
    assert!(raw_type(&suffix).kir_type().is_none());
    let mut wrong_shape = decode_module_v14(&bytes).unwrap();
    wrong_shape.id = "other.module".into();
    assert!(
        raw_type(&encode_module_v14(&wrong_shape).unwrap())
            .kir_type()
            .is_none()
    );
    assert!(
        raw_type(&encode_module_v14(&type_envelope(KirType::F32)).unwrap())
            .kir_type()
            .is_none()
    );
}

#[test]
fn declared_v14_type_rejects_incomplete_or_oversized_custody() {
    let mut ty = token_type();
    let KirType::ReusablePhaseToken(token) = &mut ty else {
        panic!()
    };
    token.owner_anchor_epoch = [0; 32];
    assert!(!PhaseValueTypeV14::supports(&ty));
    let oversized = PhaseValueTypeV14(StringAttr::new(
        "00".repeat(MAX_CANONICAL_KIR_OPERATION_BYTES_V1 + 1),
    ));
    assert!(oversized.kir_type().is_none());
}

#[test]
fn declared_v13_old_operation_fragment_bytes_are_unchanged() {
    let old = fixture::legacy_modules()[0]
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|b| &b.blocks)
        .flat_map(|b| &b.operations)
        .next()
        .unwrap()
        .clone();
    let strict = CanonicalKirOperationAttr::new(&old).unwrap();
    let common =
        CanonicalKirOperationAttr::new_declared(&old, CanonicalKernelIrVersionV1::V13).unwrap();
    assert_eq!(strict, common);
    let bytes = strict.bytes().unwrap();
    assert_eq!(&bytes[8..10], &13u16.to_le_bytes());
    assert_eq!(decode_operation(&bytes), Some(old));
}

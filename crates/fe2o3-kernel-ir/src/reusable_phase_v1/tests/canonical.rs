use super::*;
use crate::execution_capability_v1::reusable_phase_wire_v14 as codec;
use sha2::{Digest, Sha256};

#[test]
fn phase_v14_roundtrip_retains_complete_two_phase_memory_graph() {
    let m = memory::memory_fixture(2);
    let bytes = encode_module_v14(&m).unwrap();
    let pointer = bytes.as_ptr();
    let (owner, decoded) = VerifiedCanonicalKernelIrV1::from_canonical_bytes_with_module(bytes, CanonicalKernelIrVersionV1::V14).unwrap();
    assert_eq!(owner.canonical_bytes().as_ptr(), pointer);
    assert_eq!(decoded, m);
    assert_eq!(owner.version(), CanonicalKernelIrVersionV1::V14);
    assert_eq!(encode_module_v14(&decoded).unwrap(), owner.canonical_bytes());
    owner.revalidate().unwrap();
    let bytes = owner.into_canonical_bytes();
    assert_eq!(bytes.as_ptr(), pointer);
}

#[test]
fn phase_v14_general_owner_keeps_v13_facade_bytes_identity_and_move_ownership() {
    let old = module();
    let bytes = encode_module_v13(&old).unwrap();
    let pointer = bytes.as_ptr();
    let legacy = VerifiedCanonicalKernelIrV13::from_canonical_bytes(bytes).unwrap();
    let general = VerifiedCanonicalKernelIrV1::from_module(old, CanonicalKernelIrVersionV1::V13).unwrap();
    assert_eq!(legacy.canonical_bytes().as_ptr(), pointer);
    assert_eq!(legacy.canonical_bytes(), general.canonical_bytes());
    assert_eq!(legacy.identity().digest(), general.identity().digest());
    let domain = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V13\0";
    let mut digest = Sha256::new();
    digest.update((domain.len() as u32).to_le_bytes());
    digest.update(domain); digest.update(1u16.to_le_bytes());
    digest.update((legacy.canonical_bytes().len() as u64).to_le_bytes());
    digest.update(legacy.canonical_bytes());
    let expected: [u8; 32] = digest.finalize().into();
    assert_eq!(*legacy.identity().digest(), expected);
    legacy.revalidate().unwrap(); general.revalidate().unwrap();
    let bytes = legacy.into_canonical_bytes();
    assert_eq!(bytes.as_ptr(), pointer);
}

#[test]
fn phase_v14_declared_version_cannot_fall_back_to_v13_or_reverse() {
    let old = module();
    let bytes = encode_module_v14(&old).unwrap();
    assert_eq!(decode_module_v13(&bytes), Err(KernelIrDecodeError::UnknownVersion(14)));
    assert!(matches!(VerifiedCanonicalKernelIrV13::from_canonical_bytes(bytes.clone()),
        Err(VerifiedCanonicalKernelIrErrorV13::NotExactV13 { version: 14 })));
    assert!(matches!(VerifiedCanonicalKernelIrV1::from_canonical_bytes(bytes, CanonicalKernelIrVersionV1::V13),
        Err(VerifiedCanonicalKernelIrErrorV1::NotDeclaredVersion { expected: CanonicalKernelIrVersionV1::V13, actual: 14 })));
    let bytes = encode_module_v13(&old).unwrap();
    assert_eq!(decode_module_v14(&bytes), Err(KernelIrDecodeError::UnknownVersion(13)));
    assert!(matches!(VerifiedCanonicalKernelIrV1::from_canonical_bytes(bytes, CanonicalKernelIrVersionV1::V14),
        Err(VerifiedCanonicalKernelIrErrorV1::NotDeclaredVersion { expected: CanonicalKernelIrVersionV1::V14, actual: 13 })));
}

#[test]
fn phase_v14_new_family_rejects_downgrade_and_old_type_codec() {
    let m = memory::memory_fixture(1);
    let mut bytes = encode_module_v14(&m).unwrap();
    bytes[8..10].copy_from_slice(&13u16.to_le_bytes());
    assert_eq!(decode_module_v13(&bytes), Err(KernelIrDecodeError::NonCanonical));
    for result in operations(&m).iter().flat_map(|o| &o.results) {
        if let Type::ExecutionCapability(c) = &result.ty {
            if matches!(c.role, ExecutionCapabilityRoleV1::ReusableWorkgroup | ExecutionCapabilityRoleV1::ReusablePhaseCompletion) {
                let bytes = codec::encode_capability(c, 14).unwrap();
                assert_eq!(bytes.last().copied(), Some(if c.role == ExecutionCapabilityRoleV1::ReusableWorkgroup { 18 } else { 19 }));
                assert_eq!(codec::decode_capability(&bytes, 14).as_ref(), Some(c));
                assert!(codec::encode_capability(c, 13).is_none());
                assert!(codec::decode_capability(&bytes, 13).is_none());
                assert!(encode_execution_capability_type_v1(c).is_none());
                assert!(decode_execution_capability_type_v1(&bytes).is_none());
            }
        }
    }
}

#[test]
fn phase_v14_every_operation_payload_is_closed_bounded_and_nontruncatable() {
    let m = memory::memory_fixture(1);
    let mut kinds = std::collections::BTreeSet::new();
    for op in operations(&m) {
        let OperationKind::ReusablePhase(p) = &op.kind else { continue; };
        let bytes = codec::encode_contract(p).unwrap(); kinds.insert(bytes[1]);
        assert!(bytes.len() <= MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1);
        assert_eq!(codec::decode_contract(&bytes, p.operands.clone()).as_ref(), Some(p));
        for end in 0..bytes.len() { assert!(codec::decode_contract(&bytes[..end], p.operands.clone()).is_none()); }
        let mut trailing = bytes.clone(); trailing.push(0);
        assert!(codec::decode_contract(&trailing, p.operands.clone()).is_none());
        let mut bad = bytes.clone(); bad[0] = 1;
        assert!(codec::decode_contract(&bad, p.operands.clone()).is_none());
        bad[0] = 2; bad[1] = 255;
        assert!(codec::decode_contract(&bad, p.operands.clone()).is_none());
    }
    assert_eq!(kinds.into_iter().collect::<Vec<_>>(), (0..8).collect::<Vec<_>>());
}

#[test]
fn phase_v14_every_token_role_roundtrips_with_all_source_and_epoch_fields() {
    let m = memory::memory_fixture(1);
    for result in operations(&m).iter().flat_map(|o| &o.results) {
        let Type::ReusablePhaseToken(t) = &result.ty else { continue; };
        let bytes = codec::encode_token(t).unwrap();
        assert_eq!(codec::decode_token(&bytes).as_ref(), Some(t));
        for end in 0..bytes.len() { assert!(codec::decode_token(&bytes[..end]).is_none()); }
        let mut trailing = bytes.clone(); trailing.push(0);
        assert!(codec::decode_token(&trailing).is_none());
        let mut unknown = bytes; unknown[0] = 255;
        assert!(codec::decode_token(&unknown).is_none());
    }
}

#[test]
fn phase_v14_maximum_root_and_closed_payload_sizes_preserve_old_ceilings() {
    let m = memory::memory_fixture(1);
    let mut largest = 0;
    for result in operations(&m).iter().flat_map(|o| &o.results) {
        let Type::ReusablePhaseToken(t) = &result.ty else { continue; };
        let mut token = t.clone(); token.provenance.root = FunctionId::new("r".repeat(256));
        let bytes = codec::encode_token(&token).unwrap(); largest = largest.max(bytes.len());
        assert_eq!(codec::decode_token(&bytes), Some(token.clone()));
        token.provenance.root = FunctionId::new("r".repeat(257));
        assert!(codec::encode_token(&token).is_none());
    }
    assert_eq!(largest, 754);
    let mut begin = operations(&m).iter().find_map(|o| match &o.kind {
        OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::Begin { .. }) => Some(p.clone()), _ => None,
    }).unwrap();
    begin.provenance.root = FunctionId::new("r".repeat(256));
    assert_eq!(codec::encode_contract(&begin).unwrap().len(), 1888);
    let ReusablePhaseOperationV1::Begin { wrapper, invoke, .. } = &mut begin.operation else { panic!() };
    let args = vec![identity(90); MAX_EXECUTION_CAPABILITY_ARGUMENTS_V1];
    wrapper.signature = ExecutionCapabilitySignatureV1::new(&args, identity(93)).unwrap();
    invoke.signature = ExecutionCapabilitySignatureV1::new(&args, identity(95)).unwrap();
    assert_eq!(codec::encode_contract(&begin).unwrap().len(), 2016);
    let PhaseOperationSourceV1::Defined(source) = &mut begin.source else { panic!() };
    source.signature = ExecutionCapabilitySignatureV1::new(&args, identity(96)).unwrap();
    assert!(codec::encode_contract(&begin).is_none());
}

#[test]
fn phase_v14_wire_well_formedness_does_not_replace_lifecycle_verification() {
    let mut m = memory::memory_fixture(1);
    let begin = operations_mut(&mut m).iter_mut().find_map(|o| match &mut o.kind {
        OperationKind::ReusablePhase(p) if matches!(p.operation, ReusablePhaseOperationV1::Begin { .. }) => Some(p), _ => None,
    }).unwrap();
    let PhaseOperationSourceV1::Defined(d) = &mut begin.source else { panic!() };
    let old = d.call.source.occurrence.unwrap();
    d.call.source.occurrence = ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
        old.root_source_identity(), old.expansion_identity(), [222; 32], old.caller_instance(), old.expanded_block());
    let bytes = encode_module_v14(&m).unwrap();
    assert_eq!(decode_module_v14(&bytes).unwrap(), m);
    assert!(matches!(VerifiedCanonicalKernelIrV1::from_canonical_bytes(bytes, CanonicalKernelIrVersionV1::V14),
        Err(VerifiedCanonicalKernelIrErrorV1::Verification(_))));
}

use super::*;
use crate::*;
include!("../../execution_capability_v1/subgroup_partition/fixture.rs");

fn digest(m: &Module, version: CanonicalKernelIrVersionV1) -> CanonicalModuleDigestV1 {
    let mut work = usize::MAX;
    canonical_module_digest_v1(m, version, MAX_MODULE_BYTES_V1, &mut work).unwrap()
}
fn phase_type_module() -> Module {
    let mut m = module();
    m.functions[0].signature.parameters.push(Type::ReusablePhaseToken(ReusablePhaseTokenTypeV1 {
        provenance: provenance(), phase: PhaseKeyV1::from_untrusted_bytes([52; 32]),
        owner_source: identity(90), owner_anchor_epoch: [8; 32], outer_brand: [7; 32],
        phase_brand: [53; 32], initial_epoch: [54; 32],
        role: ReusablePhaseTokenRoleV1::OwnerLoan(PhaseLoanStateV1::Active),
    }));
    m
}

#[test]
fn streaming_digest_matches_collecting_codec_and_declared_headers() {
    let m = module();
    let old = encode_module_v13(&m).unwrap();
    let new = encode_module_v14(&m).unwrap();
    for (version, bytes) in [(CanonicalKernelIrVersionV1::V13, &old), (CanonicalKernelIrVersionV1::V14, &new)] {
        let actual = digest(&m, version);
        assert_eq!(*actual.digest(), <[u8; 32]>::from(Sha256::digest(bytes)));
        assert_eq!(actual.canonical_length(), bytes.len() as u64);
        assert_eq!(u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize, bytes.len());
    }
    assert_ne!(digest(&m, CanonicalKernelIrVersionV1::V13), digest(&m, CanonicalKernelIrVersionV1::V14));
}

#[test]
fn streaming_digest_exact_byte_boundary_and_one_short() {
    let m = module();
    let bytes = encode_module_v13(&m).unwrap();
    let version = CanonicalKernelIrVersionV1::V13;
    let mut work = usize::MAX;
    assert_eq!(canonical_module_digest_v1(&m, version, bytes.len(), &mut work).unwrap(), digest(&m, version));
    for max in [0, HEADER_BYTES - 1, bytes.len() - 1] {
        let mut work = usize::MAX;
        assert_eq!(canonical_module_digest_v1(&m, version, max, &mut work),
            Err(KernelIrEncodeError::TooLarge { max }));
    }
}

#[test]
fn streaming_digest_charges_both_passes_without_reset_on_failure() {
    let m = module();
    let mut work = usize::MAX;
    let expected = canonical_module_digest_v1(&m, CanonicalKernelIrVersionV1::V13, MAX_MODULE_BYTES_V1, &mut work).unwrap();
    let charged = usize::MAX - work;
    let mut one = Writer::streaming(13, MAX_MODULE_BYTES_V1, usize::MAX, Output::Count);
    encode_module_into(&mut one, &m, 0).unwrap();
    let first = usize::MAX - one.work.as_ref().unwrap().remaining;
    assert_eq!(charged, 2 * first);
    let mut exact = charged;
    assert_eq!(canonical_module_digest_v1(&m, CanonicalKernelIrVersionV1::V13, MAX_MODULE_BYTES_V1, &mut exact).unwrap(), expected);
    assert_eq!(exact, 0);
    for mut short in [0, first, charged - 1] {
        assert!(matches!(canonical_module_digest_v1(&m, CanonicalKernelIrVersionV1::V13, MAX_MODULE_BYTES_V1, &mut short),
            Err(KernelIrEncodeError::LimitExceeded { field: "canonical digest work", .. })));
        assert_eq!(short, 0);
    }
}

#[test]
fn streaming_digest_keeps_phase_v14_only_and_rejects_bad_roles() {
    let m = phase_type_module();
    let mut phase_work = usize::MAX;
    assert_eq!(canonical_module_digest_v1(&m, CanonicalKernelIrVersionV1::V13, MAX_MODULE_BYTES_V1, &mut phase_work).unwrap_err(),
        encode_module_v13(&m).unwrap_err());
    let bytes = encode_module_v14(&m).unwrap();
    assert_eq!(*digest(&m, CanonicalKernelIrVersionV1::V14).digest(), <[u8; 32]>::from(Sha256::digest(bytes)));
    let mut wrong = module();
    wrong.functions[0].role = FunctionRole::ExternalImport;
    let mut role_work = usize::MAX;
    assert_eq!(canonical_module_digest_v1(&wrong, CanonicalKernelIrVersionV1::V13, MAX_MODULE_BYTES_V1, &mut role_work).unwrap_err(),
        encode_module_v13(&wrong).unwrap_err());
}

#[test]
fn streaming_digest_keeps_fixed_codec_ceiling_without_wire_storage() {
    let mut writer = Writer::streaming(13, MAX_MODULE_BYTES_V1, usize::MAX, Output::Count);
    let chunk = [0; 4096];
    for _ in 0..MAX_MODULE_BYTES_V1 / chunk.len() { writer.bytes(&chunk).unwrap(); }
    assert_eq!(writer.length, MAX_MODULE_BYTES_V1);
    assert!(matches!(writer.output, Output::Count));
    assert_eq!(writer.bytes(&[0]), Err(KernelIrEncodeError::TooLarge { max: MAX_MODULE_BYTES_V1 }));
    let m = module();
    let mut work = usize::MAX;
    assert_eq!(canonical_module_digest_v1(&m, CanonicalKernelIrVersionV1::V13, usize::MAX, &mut work).unwrap(),
        digest(&m, CanonicalKernelIrVersionV1::V13));
}

#[test]
fn streaming_digest_rejects_role_work_before_shared_roster_allocation() {
    let m = module();
    let mut writer = Writer::streaming(13, MAX_MODULE_BYTES_V1, 0, Output::Count);
    assert!(matches!(writer.charge_role_validation(&m), Err(KernelIrEncodeError::LimitExceeded {
        field: "canonical digest work", ..
    })));
    assert_eq!(writer.length, 0);
}

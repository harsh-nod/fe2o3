//! Coordinate checks only: no recovered owner or protected origin is fabricated.
use super::*;
use fe2o3_compiler_ffi::CompilerDescriptorSourceV1;
use fe2o3_compiler_lineage::InertAbiReceiptV3;
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CompilerIdentityV1,
    DeviceDescriptorTableV1, DeviceTargetV1, DimensionsV1, EvidenceDigest, EvidenceIdentity,
    KernelAbiLayoutV1, KernelDescriptorV1, KernelId, LaunchConstraintsV1, ProducerIdentityV1, Text,
    ValidName,
};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1, Module, VerifiedCanonicalKernelIrV12};

type Failure = ProtectedCompilerNativeHandoffBindingErrorV1;

fn closure(pins: [u8; 6]) -> CompilerClosureV2 {
    CompilerClosureV2::new(
        [pins[0]; 32],
        [pins[1]; 32],
        [pins[2]; 32],
        [pins[3]; 32],
        [pins[4]; 32],
        [pins[5]; 32],
    )
    .unwrap()
}

#[test]
fn every_compiler_pin_is_checked_against_both_invocation_and_expectation() {
    let pins = [1, 2, 3, 4, 5, 6];
    let expected = closure(pins);
    assert_eq!(check_compiler_closure(expected, expected, expected), Ok(()));
    for index in 0..pins.len() {
        let mut changed = pins;
        changed[index] += 10;
        let changed = closure(changed);
        assert_eq!(
            check_compiler_closure(expected, changed, expected),
            Err(mismatch("invocation compiler closure"))
        );
        assert_eq!(
            check_compiler_closure(changed, expected, expected),
            Err(mismatch("invocation compiler closure"))
        );
        assert_eq!(
            check_compiler_closure(expected, expected, changed),
            Err(mismatch("expected compiler closure"))
        );
    }
}

#[test]
fn actual_verified_f_digest_and_length_must_match_the_native_subject() {
    // Real V12 semantic admission mints these identities. Neither graph is a
    // recovered compiler owner and this check grants no source/F provenance.
    let actual = VerifiedCanonicalKernelIrV12::from_module(Module::new("final_a")).unwrap();
    let other = VerifiedCanonicalKernelIrV12::from_module(Module::new("final_b")).unwrap();
    let identity = actual.identity();
    let subject = InertNativeNeutralSubjectV1::new(
        *identity.digest(),
        identity.canonical_length(),
        [3; 32],
        32,
    )
    .unwrap();
    assert_eq!(check_actual_f(identity, &subject), Ok(()));
    assert_eq!(
        identity.canonical_length(),
        other.identity().canonical_length()
    );
    assert_eq!(
        check_actual_f(other.identity(), &subject),
        Err(mismatch("recovered actual F"))
    );
    let wrong_length = InertNativeNeutralSubjectV1::new(
        *identity.digest(),
        identity.canonical_length() + 1,
        [3; 32],
        32,
    )
    .unwrap();
    assert_eq!(
        check_actual_f(identity, &wrong_length),
        Err(mismatch("recovered actual F"))
    );
}

// Uses the same public inert descriptor constructors as compiler-ffi's
// descriptor_source fixture, with a zero-argument kernel for this hash check.
fn descriptor() -> CompilerDescriptorSourceV1 {
    let evidence = |byte| {
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([byte; 32]),
            EvidenceDigest::from_sha256_bytes([byte; 32]),
        )
    };
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([1; 32]),
        ValidName::new("binding_kernel").unwrap(),
        ValidName::new("binding_kernel").unwrap(),
        ValidName::new("binding_kernel.kd").unwrap(),
        evidence(2),
        evidence(3),
        vec![],
        KernelAbiLayoutV1::new(0, 0, 1).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            64,
            0,
            0,
        )
        .unwrap(),
        vec![],
    )
    .unwrap();
    CompilerDescriptorSourceV1::new(
        DeviceDescriptorTableV1::new(
            CanonicalCodeObjectDigest::from_bytes([0; 32]),
            CodeObjectVersion::V6,
            CompilerIdentityV1::new(
                Text::new("binding-fixture").unwrap(),
                Text::new("test").unwrap(),
                [4; 20],
            ),
            ProducerIdentityV1::new(
                Text::new("binding-fixture").unwrap(),
                Text::new("test").unwrap(),
            ),
            DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
            vec![],
            vec![],
            vec![kernel],
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn descriptor_uses_raw_identity_and_rejects_abi_receipt_or_length_substitution() {
    let descriptor = descriptor();
    let id = descriptor.identity();
    let raw = TargetLineageIdentityV3::new(*id.sha256(), id.byte_len()).unwrap();
    let receipt = InertAbiReceiptV3::from_canonical_preimage(descriptor.canonical_bytes()).unwrap();
    let wrapped =
        TargetLineageIdentityV3::new(*receipt.identity().sha256(), receipt.identity().byte_len())
            .unwrap();
    let wrong_length = TargetLineageIdentityV3::new(*id.sha256(), id.byte_len() + 1).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, MAX_INERT_REFINED_FORWARDING_STORAGE_V1);
    let bytes = descriptor.canonical_bytes();
    budget.reserve_storage(bytes.len()).unwrap();
    assert_eq!(check_descriptor(bytes, raw, &mut budget), Ok(()));
    assert_eq!(
        check_descriptor(bytes, wrapped, &mut budget),
        Err(mismatch("native descriptor identity"))
    );
    assert_eq!(
        check_descriptor(bytes, wrong_length, &mut budget),
        Err(mismatch("native descriptor byte length"))
    );
    assert_eq!(budget.work(), 2 * (bytes.len() + 64));
    assert_eq!(budget.storage(), bytes.len());
    assert!(matches!(
        check_descriptor(&vec![0; MAX_DESCRIPTOR_TABLE_BYTES + 1], raw, &mut budget),
        Err(Failure::RelationshipMismatch {
            field: "native descriptor byte length"
        })
    ));
}

#[test]
fn descriptor_work_refusal_precedes_hash_comparison_and_preserves_the_ledger() {
    let descriptor = descriptor();
    let bytes = descriptor.canonical_bytes();
    let wrong = TargetLineageIdentityV3::new([9; 32], bytes.len() as u64).unwrap();
    let prefix = 17;
    let needed = bytes.len() + 64;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(prefix + needed - 1);
    {
        let mut budget = Budget::new(&mut work, MAX_INERT_REFINED_FORWARDING_STORAGE_V1);
        budget.reserve_storage(bytes.len()).unwrap();
        budget.charge_work(prefix).unwrap();
        let error = check_descriptor(bytes, wrong, &mut budget).unwrap_err();
        assert!(matches!(error, Failure::Resource(Resource::Work(_))));
        assert!(error.source().is_some());
        assert_eq!(budget.work(), prefix);
        assert_eq!(budget.storage(), bytes.len());
        assert_eq!(budget.peak_storage(), bytes.len());
    }
    assert_eq!(work.failed_work(), Some(prefix + needed));
}

#[test]
fn focused_errors_preserve_resource_and_native_decode_diagnostics() {
    let error = Failure::from(Resource::Accounting);
    assert!(error.source().is_some());
    assert!(error.to_string().contains("accounting"));
    let decode = InertNativeLoweringAssociationV1::decode(&[]).unwrap_err();
    let error = Failure::from(decode);
    assert!(matches!(
        error,
        Failure::NativeLowering(NativeLoweringAssociationErrorV1::Length)
    ));
    assert!(error.to_string().contains("Length"));
    assert!(mismatch("test axis").source().is_none());
}

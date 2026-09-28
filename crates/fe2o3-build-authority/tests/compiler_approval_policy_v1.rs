//! Synthetic public policy bytes cannot approve a compiler or enforce a runtime.
use fe2o3_build_authority::*;
use sha2::{Digest, Sha256};
use std::panic::{AssertUnwindSafe, catch_unwind};

type Error = CompilerApprovalPolicyErrorV1<u8>;
const PINS: [[u8; 32]; 6] = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]];
const PROFILE: [u8; 32] = [7; 32];
const MANIFEST: [u8; 32] = [8; 32];
const END: usize = COMPILER_APPROVAL_POLICY_BYTES_V1 - 32;

fn closure(pins: [[u8; 32]; 6]) -> CompilerClosureV2 {
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
}
fn policy() -> CompilerApprovalPolicyV1 {
    CompilerApprovalPolicyV1::new(closure(PINS), PROFILE, MANIFEST, 1, |_| Ok::<_, u8>(())).unwrap()
}
fn decode(bytes: &[u8]) -> Result<CompilerApprovalPolicyV1, Error> {
    CompilerApprovalPolicyV1::decode(bytes, |_| Ok(()))
}
fn digest(bytes: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-compiler-approval-policy-v1\0");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    hash.finalize().into()
}
fn reseal(bytes: &mut [u8]) {
    let sha256 = digest(&bytes[..END]);
    bytes[END..].copy_from_slice(&sha256);
}

#[test]
fn fixed_schema_roundtrip_exact_fields_and_domain_are_inert() {
    let value = policy();
    let bytes = value.canonical_bytes();
    assert_eq!(COMPILER_APPROVAL_POLICY_BYTES_V1, 352);
    assert_eq!(COMPILER_APPROVAL_POLICY_HEADER_LEN_V1, 32);
    assert_eq!(COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V1, 1);
    assert_eq!(
        COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V1,
        b"fe2o3-compiler-approval-policy-v1\0"
    );
    assert_eq!(
        &bytes[..20],
        b"F2CAP1\0\0\x01\0\x20\0\x60\x01\0\0\x01\0\x01\0"
    );
    assert_eq!(&bytes[20..32], &[0; 12]);
    for (slot, expected) in bytes[32..224].chunks_exact(32).zip(PINS) {
        assert_eq!(slot, expected);
    }
    assert_eq!(&bytes[224..256], &closure(PINS).identity_sha256());
    assert_eq!(&bytes[256..288], &PROFILE);
    assert_eq!(&bytes[288..320], &MANIFEST);
    assert_eq!(value.compiler_closure(), closure(PINS));
    assert_eq!(value.client_profile_identity(), &PROFILE);
    assert_eq!(value.runtime_manifest_identity(), &MANIFEST);
    assert_eq!(value.required_runtime_enforcement_version(), 1);
    assert_eq!(*value.identity(), digest(&bytes[..END]));
    assert_ne!(
        *value.identity(),
        <[u8; 32]>::from(Sha256::digest(&bytes[..END]))
    );
    assert_eq!(decode(bytes).unwrap(), value);
    assert_eq!(policy().canonical_bytes(), bytes);
    assert!(!value.grants_authority());
    assert!(!decode(bytes).unwrap().grants_authority());
    assert!(COMPILER_APPROVAL_POLICY_STORAGE_V1 > std::mem::size_of::<CompilerApprovalPolicyV1>());
    assert!(COMPILER_APPROVAL_POLICY_WORK_V1 >= bytes.len());
}

#[test]
fn every_byte_is_bound_and_each_valid_field_change_changes_identity() {
    let original = policy();
    for index in 0..COMPILER_APPROVAL_POLICY_BYTES_V1 {
        let mut bytes = *original.canonical_bytes();
        bytes[index] ^= 1;
        assert!(decode(&bytes).is_err(), "byte {index}");
    }
    for index in 0..PINS.len() {
        let mut pins = PINS;
        pins[index][index] ^= 128;
        let changed =
            CompilerApprovalPolicyV1::new(closure(pins), PROFILE, MANIFEST, 1, |_| Ok::<_, u8>(()))
                .unwrap();
        assert_ne!(changed.identity(), original.identity());
        assert_eq!(decode(changed.canonical_bytes()).unwrap(), changed);
    }
    for offset in [256, 288] {
        let mut bytes = *original.canonical_bytes();
        bytes[offset] ^= 1;
        reseal(&mut bytes);
        let changed = decode(&bytes).unwrap();
        assert_ne!(changed.identity(), original.identity());
        assert!(!changed.grants_authority());
    }
}

#[test]
fn zero_identities_are_refused_even_after_coherent_resealing() {
    for (profile, manifest, expected) in [
        ([0; 32], MANIFEST, Error::ZeroClientProfileIdentity),
        (PROFILE, [0; 32], Error::ZeroRuntimeManifestIdentity),
    ] {
        assert_eq!(
            CompilerApprovalPolicyV1::new(closure(PINS), profile, manifest, 1, |_| Ok::<_, u8>(())),
            Err(expected)
        );
    }
    let fields = [
        CompilerClosureDigestFieldV2::CargoExecutable,
        CompilerClosureDigestFieldV2::CargoBindingTrampoline,
        CompilerClosureDigestFieldV2::CargoFe2o3BindingWrapper,
        CompilerClosureDigestFieldV2::RustcExecutable,
        CompilerClosureDigestFieldV2::RustcRuntimeTree,
        CompilerClosureDigestFieldV2::CodegenBackend,
        CompilerClosureDigestFieldV2::CompilerClosure,
    ];
    for (index, field) in fields.into_iter().enumerate() {
        let mut bytes = *policy().canonical_bytes();
        bytes[32 + index * 32..64 + index * 32].fill(0);
        reseal(&mut bytes);
        assert_eq!(
            decode(&bytes),
            Err(Error::CompilerClosure(CompilerClosureErrorV2::ZeroDigest {
                field
            }))
        );
    }
    for (offset, expected) in [
        (256, Error::ZeroClientProfileIdentity),
        (288, Error::ZeroRuntimeManifestIdentity),
    ] {
        let mut bytes = *policy().canonical_bytes();
        bytes[offset..offset + 32].fill(0);
        reseal(&mut bytes);
        assert_eq!(decode(&bytes), Err(expected));
    }
}

#[test]
fn closure_aggregate_is_validated_by_the_existing_codec() {
    for offset in (32..256).step_by(32) {
        let mut bytes = *policy().canonical_bytes();
        bytes[offset] ^= 128;
        reseal(&mut bytes);
        assert_eq!(
            decode(&bytes),
            Err(Error::CompilerClosure(
                CompilerClosureErrorV2::IdentityMismatch
            ))
        );
    }
}

#[test]
fn unsupported_enforcement_transition_and_schema_versions_fail_when_resealed() {
    for version in [0_u16, 2, u16::MAX] {
        assert_eq!(
            CompilerApprovalPolicyV1::new(
                closure(PINS),
                PROFILE,
                MANIFEST,
                version,
                |_| Ok::<_, u8>(())
            ),
            Err(Error::UnsupportedRuntimeEnforcementVersion { version })
        );
        let mut bytes = *policy().canonical_bytes();
        bytes[18..20].copy_from_slice(&version.to_le_bytes());
        reseal(&mut bytes);
        assert_eq!(
            decode(&bytes),
            Err(Error::UnsupportedRuntimeEnforcementVersion { version })
        );
        let mut bytes = *policy().canonical_bytes();
        bytes[16..18].copy_from_slice(&version.to_le_bytes());
        let aggregate = derive_compiler_closure_identity_v2(
            PINS[0], PINS[1], PINS[2], PINS[3], PINS[4], PINS[5], version,
        );
        bytes[224..256].copy_from_slice(&aggregate);
        reseal(&mut bytes);
        assert_eq!(
            decode(&bytes),
            Err(Error::CompilerClosure(
                CompilerClosureErrorV2::UnsupportedTransitionProtocolVersion { version }
            ))
        );
        let mut bytes = *policy().canonical_bytes();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        reseal(&mut bytes);
        assert_eq!(decode(&bytes), Err(Error::Header));
    }
}

#[test]
fn strict_magic_extent_reserved_and_terminal_digest_have_no_fallback() {
    for index in (0..8).chain(20..32) {
        let mut bytes = *policy().canonical_bytes();
        bytes[index] ^= 1;
        reseal(&mut bytes);
        assert_eq!(
            decode(&bytes),
            Err(if index < 8 {
                Error::Header
            } else {
                Error::Reserved
            })
        );
    }
    for header_len in [0_u16, 31, 33, u16::MAX] {
        let mut bytes = *policy().canonical_bytes();
        bytes[10..12].copy_from_slice(&header_len.to_le_bytes());
        reseal(&mut bytes);
        assert_eq!(decode(&bytes), Err(Error::Header));
    }
    for length in [0_u32, 351, 353, u32::MAX] {
        let mut bytes = *policy().canonical_bytes();
        bytes[12..16].copy_from_slice(&length.to_le_bytes());
        reseal(&mut bytes);
        assert_eq!(decode(&bytes), Err(Error::Length));
    }
    let mut bytes = *policy().canonical_bytes();
    bytes[END..].fill(0);
    assert_eq!(decode(&bytes), Err(Error::Identity));
}

#[test]
fn every_truncation_and_trailing_byte_is_refused_before_payload_work() {
    let value = policy();
    for length in 0..COMPILER_APPROVAL_POLICY_BYTES_V1 {
        assert_eq!(
            CompilerApprovalPolicyV1::decode(
                &value.canonical_bytes()[..length],
                |_| -> Result<(), u8> { panic!("length rejection must not walk") }
            ),
            Err(Error::Length)
        );
    }
    let mut extended = [0; COMPILER_APPROVAL_POLICY_BYTES_V1 + 1];
    extended[..COMPILER_APPROVAL_POLICY_BYTES_V1].copy_from_slice(value.canonical_bytes());
    assert_eq!(decode(&extended), Err(Error::Length));
}

#[test]
fn exact_budget_passes_one_short_and_refusal_precede_all_walks() {
    let original = policy();
    for limit in [
        0,
        COMPILER_APPROVAL_POLICY_WORK_V1 - 1,
        COMPILER_APPROVAL_POLICY_WORK_V1,
    ] {
        let mut calls = 0;
        let result = CompilerApprovalPolicyV1::decode(original.canonical_bytes(), |work| {
            calls += 1;
            assert_eq!(work, COMPILER_APPROVAL_POLICY_WORK_V1);
            if work > limit { Err(37_u8) } else { Ok(()) }
        });
        assert_eq!(calls, 1);
        assert_eq!(
            result,
            if limit < COMPILER_APPROVAL_POLICY_WORK_V1 {
                Err(Error::Charge(37))
            } else {
                Ok(original)
            }
        );
        let mut calls = 0;
        let result = CompilerApprovalPolicyV1::new(closure(PINS), PROFILE, MANIFEST, 1, |work| {
            calls += 1;
            assert_eq!(work, COMPILER_APPROVAL_POLICY_WORK_V1);
            if work > limit { Err(37_u8) } else { Ok(()) }
        });
        assert_eq!(calls, 1);
        assert_eq!(
            result,
            if limit < COMPILER_APPROVAL_POLICY_WORK_V1 {
                Err(Error::Charge(37))
            } else {
                Ok(original)
            }
        );
    }
    let invalid = [0; COMPILER_APPROVAL_POLICY_BYTES_V1];
    assert_eq!(
        CompilerApprovalPolicyV1::decode(&invalid, |_| Err(37_u8)),
        Err(Error::Charge(37))
    );
    assert_eq!(
        CompilerApprovalPolicyV1::new(closure(PINS), [0; 32], [0; 32], 0, |_| Err(37_u8)),
        Err(Error::Charge(37))
    );
    assert_eq!(policy(), original);
}

#[test]
fn charge_and_callback_drop_panics_preserve_inputs_and_return_no_record() {
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("callback drop");
        }
    }
    let original = policy();
    let bytes = *original.canonical_bytes();
    for destructor in [false, true] {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if destructor {
                let bomb = Bomb;
                CompilerApprovalPolicyV1::decode(&bytes, move |_| {
                    let _capture = &bomb;
                    Ok::<_, u8>(())
                })
                .unwrap()
            } else {
                CompilerApprovalPolicyV1::decode(&bytes, |_| -> Result<(), u8> {
                    panic!("work refusal")
                })
                .unwrap()
            }
        }));
        assert!(result.is_err());
        assert_eq!(&bytes, original.canonical_bytes());
        let result = catch_unwind(AssertUnwindSafe(|| {
            if destructor {
                let bomb = Bomb;
                CompilerApprovalPolicyV1::new(closure(PINS), PROFILE, MANIFEST, 1, move |_| {
                    let _capture = &bomb;
                    Ok::<_, u8>(())
                })
                .unwrap()
            } else {
                CompilerApprovalPolicyV1::new(
                    closure(PINS),
                    PROFILE,
                    MANIFEST,
                    1,
                    |_| -> Result<(), u8> { panic!("work refusal") },
                )
                .unwrap()
            }
        }));
        assert!(result.is_err());
    }
    assert_eq!(policy(), original);
}

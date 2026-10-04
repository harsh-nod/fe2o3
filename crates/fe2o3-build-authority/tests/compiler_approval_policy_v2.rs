//! Independent V2 transcripts remain inert, including coherently resealed roles.
use fe2o3_build_authority::*;
use sha2::{Digest, Sha256};
use std::{
    cell::Cell,
    panic::{AssertUnwindSafe, catch_unwind},
};

type Error = CompilerApprovalPolicyErrorV2<u8>;
type Framing = CompilerApprovalPolicyErrorV1<u8>;
const UID: u32 = 7001;
const GID: u32 = 8001;
const END: usize = 320;

fn closure() -> CompilerClosureV2 {
    CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap()
}
fn policy(uid: u32, gid: u32) -> Result<CompilerApprovalPolicyV2, Error> {
    CompilerApprovalPolicyV2::new(closure(), [7; 32], [8; 32], 1, uid, gid, |_| Ok(()))
}
fn decode(bytes: &[u8]) -> Result<CompilerApprovalPolicyV2, Error> {
    CompilerApprovalPolicyV2::decode(bytes, |_| Ok(()))
}
fn reseal(bytes: &mut [u8], domain: &[u8]) {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(320_u64.to_le_bytes());
    hash.update(&bytes[..END]);
    bytes[END..].copy_from_slice(&hash.finalize());
}
const DOMAIN: &[u8] = b"fe2o3-compiler-approval-policy-v2\0";

#[test]
fn exact_v2_transcript_retains_existing_closure_offsets_and_binds_helper_credentials() {
    let mut expected = [0; 352];
    expected[..20].copy_from_slice(b"F2CAP2\0\0\x02\0\x20\0\x60\x01\0\0\x01\0\x01\0");
    expected[20..24].copy_from_slice(&UID.to_le_bytes());
    expected[24..28].copy_from_slice(&GID.to_le_bytes());
    for (index, slot) in expected[32..224].chunks_exact_mut(32).enumerate() {
        slot.fill(index as u8 + 1);
    }
    expected[224..256].copy_from_slice(&closure().identity_sha256());
    expected[256..288].fill(7);
    expected[288..320].fill(8);
    reseal(&mut expected, DOMAIN);
    let value = policy(UID, GID).unwrap();
    assert_eq!(value.canonical_bytes(), &expected);
    assert_eq!(value.proof_helper_uid(), UID);
    assert_eq!(value.proof_helper_gid(), GID);
    assert_eq!(value.compiler_closure(), closure());
    assert_eq!(value.client_profile_identity(), &[7; 32]);
    assert_eq!(value.runtime_manifest_identity(), &[8; 32]);
    assert_eq!(value.required_runtime_enforcement_version(), 1);
    assert_eq!(value.identity(), &expected[320..]);
    assert_eq!(decode(&expected).unwrap(), value);
    assert!(!value.grants_authority());
    assert!(!decode(&expected).unwrap().grants_authority());
    assert_eq!(COMPILER_APPROVAL_POLICY_HEADER_LEN_V2, 32);
    assert_eq!(COMPILER_APPROVAL_POLICY_BYTES_V2, 352);
    assert_eq!(COMPILER_APPROVAL_POLICY_MAGIC_V2, *b"F2CAP2\0\0");
    assert_eq!(COMPILER_APPROVAL_POLICY_VERSION_V2, 2);
    assert_eq!(COMPILER_APPROVAL_POLICY_IDENTITY_DOMAIN_V2, DOMAIN);
    assert_eq!(COMPILER_APPROVAL_POLICY_RUNTIME_ENFORCEMENT_VERSION_V2, 1);
    assert!(COMPILER_APPROVAL_POLICY_STORAGE_V2 > std::mem::size_of_val(&value));
    for index in 0..expected.len() {
        let mut changed = expected;
        changed[index] ^= 1;
        assert!(decode(&changed).is_err(), "unsealed byte {index}");
    }
    for (uid, gid) in [(UID + 1, GID), (UID, GID + 1)] {
        let changed = policy(uid, gid).unwrap();
        assert_ne!(changed.identity(), value.identity());
        assert_eq!(decode(changed.canonical_bytes()).unwrap(), changed);
    }
}

#[test]
fn invalid_helper_credentials_refuse_construction_and_coherently_resealed_records() {
    for (uid, gid, expected) in [
        (0, GID, Error::InvalidProofHelperUid),
        (u32::MAX, GID, Error::InvalidProofHelperUid),
        (UID, 0, Error::InvalidProofHelperGid),
        (UID, u32::MAX, Error::InvalidProofHelperGid),
    ] {
        assert_eq!(policy(uid, gid), Err(expected));
        let mut bytes = *policy(UID, GID).unwrap().canonical_bytes();
        bytes[20..24].copy_from_slice(&uid.to_le_bytes());
        bytes[24..28].copy_from_slice(&gid.to_le_bytes());
        reseal(&mut bytes, DOMAIN);
        assert_eq!(decode(&bytes), Err(expected));
    }
    for (uid, gid) in [(1, 1), (u32::MAX - 1, u32::MAX - 1)] {
        assert!(policy(uid, gid).is_ok());
    }
}

#[test]
fn versions_domains_reserved_bytes_and_lengths_never_fall_back() {
    let legacy =
        CompilerApprovalPolicyV1::new(closure(), [7; 32], [8; 32], 1, |_| Ok::<_, u8>(())).unwrap();
    let value = policy(UID, GID).unwrap();
    assert_eq!(
        decode(legacy.canonical_bytes()),
        Err(Error::Framing(Framing::Header))
    );
    assert_eq!(
        CompilerApprovalPolicyV1::decode(value.canonical_bytes(), |_| Ok::<_, u8>(())),
        Err(Framing::Header)
    );
    for version in [0_u16, 1, 3, u16::MAX] {
        let mut bytes = *value.canonical_bytes();
        bytes[8..10].copy_from_slice(&version.to_le_bytes());
        reseal(&mut bytes, DOMAIN);
        assert_eq!(decode(&bytes), Err(Error::Framing(Framing::Header)));
    }
    for index in 28..32 {
        let mut bytes = *value.canonical_bytes();
        bytes[index] = 1;
        reseal(&mut bytes, DOMAIN);
        assert_eq!(decode(&bytes), Err(Error::Framing(Framing::Reserved)));
    }
    let mut bytes = *value.canonical_bytes();
    reseal(&mut bytes, b"fe2o3-compiler-approval-policy-v1\0");
    assert_eq!(decode(&bytes), Err(Error::Framing(Framing::Identity)));
    for len in 0..352 {
        assert_eq!(
            CompilerApprovalPolicyV2::decode(&bytes[..len], |_| -> Result<(), u8> {
                panic!("length refusal must precede charge")
            }),
            Err(Error::Framing(Framing::Length))
        );
    }
    let mut extended = bytes.to_vec();
    extended.push(0);
    assert_eq!(decode(&extended), Err(Error::Framing(Framing::Length)));
    for (offset, expected) in [
        (256, Framing::ZeroClientProfileIdentity),
        (288, Framing::ZeroRuntimeManifestIdentity),
    ] {
        let mut bytes = *value.canonical_bytes();
        bytes[offset..offset + 32].fill(0);
        reseal(&mut bytes, DOMAIN);
        assert_eq!(decode(&bytes), Err(Error::Framing(expected)));
    }
    let mut bytes = *value.canonical_bytes();
    bytes[32] ^= 1;
    reseal(&mut bytes, DOMAIN);
    assert_eq!(
        decode(&bytes),
        Err(Error::Framing(Framing::CompilerClosure(
            CompilerClosureErrorV2::IdentityMismatch
        )))
    );
    bytes = *value.canonical_bytes();
    bytes[18..20].copy_from_slice(&2_u16.to_le_bytes());
    reseal(&mut bytes, DOMAIN);
    assert_eq!(
        decode(&bytes),
        Err(Error::Framing(
            Framing::UnsupportedRuntimeEnforcementVersion { version: 2 }
        ))
    );
}

#[test]
fn work_refusal_and_callback_destruction_precede_credential_and_payload_reads() {
    let value = policy(UID, GID).unwrap();
    for limit in [
        0,
        COMPILER_APPROVAL_POLICY_WORK_V2 - 1,
        COMPILER_APPROVAL_POLICY_WORK_V2,
    ] {
        for construct in [false, true] {
            let calls = Cell::new(0);
            let charge = |amount| {
                calls.set(calls.get() + 1);
                assert_eq!(amount, COMPILER_APPROVAL_POLICY_WORK_V2);
                if amount > limit { Err(37_u8) } else { Ok(()) }
            };
            let result = if construct {
                CompilerApprovalPolicyV2::new(closure(), [7; 32], [8; 32], 1, UID, GID, charge)
            } else {
                CompilerApprovalPolicyV2::decode(value.canonical_bytes(), charge)
            };
            assert_eq!(calls.get(), 1);
            assert_eq!(
                result,
                if limit < COMPILER_APPROVAL_POLICY_WORK_V2 {
                    Err(Error::Framing(Framing::Charge(37)))
                } else {
                    Ok(value)
                }
            );
        }
    }
    assert_eq!(
        CompilerApprovalPolicyV2::new(closure(), [0; 32], [0; 32], 0, 0, 0, |_| Err(37_u8)),
        Err(Error::Framing(Framing::Charge(37)))
    );
    assert_eq!(
        CompilerApprovalPolicyV2::decode(&[0; 352], |_| Err(37_u8)),
        Err(Error::Framing(Framing::Charge(37)))
    );
    struct Bomb;
    impl Drop for Bomb {
        fn drop(&mut self) {
            panic!("callback destruction");
        }
    }
    let bytes = *value.canonical_bytes();
    for construct in [false, true] {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let bomb = Bomb;
            let charge = move |_| {
                let _borrow = &bomb;
                Ok::<_, u8>(())
            };
            if construct {
                CompilerApprovalPolicyV2::new(closure(), [0; 32], [0; 32], 0, 0, 0, charge)
            } else {
                CompilerApprovalPolicyV2::decode(&bytes, charge)
            }
        }));
        assert!(result.is_err());
        assert_eq!(&bytes, value.canonical_bytes());
    }
}

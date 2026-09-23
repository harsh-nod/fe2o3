//! Synthetic signed protocol records, not protected compiler occurrences.
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_DECODE_WORK_V2 as DW,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_IDENTITY_WORK_V2 as MW,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_ISSUE_WORK_V2 as IW,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_STORAGE_V2 as RS,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_VERIFY_WORK_V2 as VW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2 as QW,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as PW, CompilerExecutionAttestationErrorV1 as Framing,
    CompilerExecutionAttestationErrorV2 as Error,
    CompilerExecutionAttestationReceiptV1 as LegacyReceipt,
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV1 as LegacyRequest,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionIssuerPolicyV1 as LegacyPolicy, CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;
#[path = "support/native_attestation_fixture.rs"]
mod fixture;
use fixture::*;
const LIMIT: usize = 1_000_000;
macro_rules! assert_error_eq {
    ($actual:expr, $expected:expr $(,)?) => {
        assert_eq!(format!("{:?}", $actual), format!("{:?}", $expected))
    };
}

fn signature_message(bytes: &[u8], version: u16) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(format!(
        "FE2O3/COMPILER-EXECUTION-RECEIPT-SIGNATURE/V{version}\0"
    ));
    hash.update(304u64.to_le_bytes());
    hash.update(&bytes[..304]);
    hash.finalize().into()
}
fn anchor(bytes: &[u8], version: u16) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(format!(
        "FE2O3/COMPILER-EXECUTION-ROLLBACK-ANCHOR/V{version}\0"
    ));
    for range in [
        200..208,
        208..240,
        24..56,
        96..128,
        128..136,
        168..200,
        64..96,
    ] {
        hash.update(&bytes[range]);
    }
    hash.finalize().into()
}
fn sign(bytes: &mut [u8], version: u16, key: &SigningKey) {
    let signature = key.sign(&signature_message(bytes, version)).to_bytes();
    bytes[304..368].copy_from_slice(&signature);
    seal(bytes, "COMPILER-EXECUTION-RECEIPT", version);
}
fn rebuild(bytes: &mut [u8], version: u16) {
    let next = anchor(bytes, version);
    bytes[240..272].copy_from_slice(&next);
    sign(bytes, version, &SigningKey::from_bytes(&[0x51; 32]));
}
fn receipt_wire(version: u16) -> [u8; 400] {
    let mut bytes = [0; 400];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CER1"
        } else {
            b"F2O3CER2"
        },
        version,
    );
    bytes[24..56].copy_from_slice(&request_wire(version)[914..]);
    bytes[56..64].copy_from_slice(&946u64.to_le_bytes());
    bytes[64..96].copy_from_slice(&policy_wire(version)[184..]);
    bytes[96..136].copy_from_slice(&challenge_wire(version)[56..96]);
    bytes[136..168].copy_from_slice(&challenge_wire(version)[168..]);
    bytes[168..200].fill(0x71);
    bytes[200..208].copy_from_slice(&1u64.to_le_bytes());
    bytes[272..304].copy_from_slice(&policy_wire(version)[112..144]);
    rebuild(&mut bytes, version);
    bytes
}
fn policy(bytes: &[u8]) -> Policy {
    let mut work = Work::new(PW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    Policy::decode(bytes, &mut budget).unwrap().0
}
fn request(bytes: &[u8]) -> Request {
    let mut work = Work::new(QW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    Request::decode(bytes, &mut budget).unwrap().0
}
fn decode(bytes: &[u8]) -> Result<Receipt, Error> {
    let mut work = Work::new(DW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    let result = Receipt::decode(bytes, &mut budget);
    assert_eq!(budget.storage(), bytes.len());
    result.map(|(r, s)| {
        assert_eq!(r.retained_storage(), s.additional_storage());
        r
    })
}
fn decode_error(version: u16, bytes: &[u8]) -> Framing {
    if version == 1 {
        LegacyReceipt::decode(bytes).unwrap_err()
    } else {
        match decode(bytes).unwrap_err() {
            Error::Framing(e) => e,
            other => panic!("{other:?}"),
        }
    }
}
fn verify_error(version: u16, bytes: &[u8]) -> Framing {
    if version == 1 {
        LegacyReceipt::decode(bytes)
            .unwrap()
            .verify(
                &LegacyPolicy::decode(&policy_wire(1)).unwrap(),
                &LegacyRequest::decode(&request_wire(1)).unwrap(),
                [0; 32],
            )
            .unwrap_err()
    } else {
        let p = policy(&policy_wire(2));
        let q = request(&request_wire(2));
        let r = decode(bytes).unwrap();
        let mut work = Work::new(VW);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(p.retained_storage() + q.retained_storage() + r.retained_storage())
            .unwrap();
        match r.verify(&p, &q, [0; 32], &mut budget).unwrap_err() {
            Error::Framing(e) => e,
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn independent_signed_transcripts_freeze_both_families() {
    for (version, next, message, identity, wire_hash) in [
        (
            1,
            "ed60c217db168c50b69bb0648993313d9b37c099f7e64111d04c48d47b2f6946",
            "4ba02fda74627c4901329cbf195ecf306a48a377e5bb8c8b75bf8d0cf15911d9",
            "430fb35ad2b3a0ee2095eaa3702b66c3fc0fdab90986686ae585a2b8d1f5072b",
            "e1afce82f8f05d4ce4362ef7313320b3167b94a4884e008d851f29a8532cd3c7",
        ),
        (
            2,
            "1e8e4a5445c37f216e24102fef015438523a88bf427d694954f6333467d047f7",
            "6503ada215e01b6f2f3019bdd3d15a20ce8b16eaa819036a5c4b7f2d36e7b2bf",
            "4a886460d3a1f6f1f643166366f8c35ff3fc87694bd1272a1ae81b3c63c5193c",
            "7783073c13c574699b897e5498a9f8f1c32e87e2e5c738e2ca77a9572c8d3e2d",
        ),
    ] {
        let bytes = receipt_wire(version);
        assert_eq!(hex(&bytes[240..272]), next);
        assert_eq!(hex(&signature_message(&bytes, version)), message);
        assert_eq!(hex(&bytes[368..]), identity);
        assert_eq!(hex(&Sha256::digest(bytes)), wire_hash);
        if version == 1 {
            let r = LegacyReceipt::issue(
                &LegacyPolicy::decode(&policy_wire(1)).unwrap(),
                &LegacyRequest::decode(&request_wire(1)).unwrap(),
                &SigningKey::from_bytes(&[0x51; 32]),
            )
            .unwrap();
            assert_eq!(r.canonical_bytes(), &bytes);
            assert_eq!(LegacyReceipt::decode(&bytes).unwrap(), r);
        } else {
            assert_eq!(decode(&bytes).unwrap().canonical_bytes(), &bytes);
        }
    }
}

#[test]
fn issue_verify_and_downgrade_preserve_exact_reservations_and_no_authority() {
    let p = policy(&policy_wire(2));
    let q = request(&request_wire(2));
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let floor = p.retained_storage() + q.retained_storage() + size_of::<SigningKey>();
    let mut work = Work::new(IW + VW + MW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(floor).unwrap();
    let (r, charge) = Receipt::issue(&p, &q, &key, &mut budget).unwrap();
    assert_eq!(r.canonical_bytes(), &receipt_wire(2));
    assert_eq!(budget.storage(), floor);
    assert_eq!(charge.additional_storage(), r.retained_storage());
    budget.reserve_storage(charge.additional_storage()).unwrap();
    let inherited = budget.storage();
    assert!(
        r.identity()
            .matches_canonical_bytes(r.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert!(!r.grants_compiler_authority());
    assert!(!r.grants_load_authority());
    assert!(!r.grants_launch_authority());
    let (verified, delta) = r.verify(&p, &q, [0; 32], &mut budget).unwrap();
    assert_eq!(delta.additional_storage(), 0);
    assert_eq!(verified.retained_storage(), charge.additional_storage());
    assert_eq!(budget.storage(), inherited);
    assert_eq!(budget.work(), IW + VW + MW);
    assert!(verified.authenticates_pinned_signing_key());
    assert!(!verified.authenticates_protected_compiler_execution());
    assert!(!verified.grants_compiler_authority());
    assert!(!verified.grants_load_authority());
    assert!(!verified.grants_launch_authority());
    assert_eq!(verified.receipt().request_sha256(), q.identity().as_bytes());
    assert_eq!(
        verified.receipt().challenge_identity(),
        q.challenge().identity()
    );
    assert_eq!(verified.receipt().challenge_nonce(), q.challenge().nonce());
    assert_eq!(verified.receipt().policy_identity(), p.identity());
    assert_eq!(
        verified.into_receipt().retained_storage(),
        charge.additional_storage()
    );
}

#[test]
fn every_receipt_byte_mutation_and_wrong_length_rejects() {
    for version in [1, 2] {
        let original = receipt_wire(version);
        for i in 0..400 {
            let mut bytes = original;
            bytes[i] ^= 0x80;
            let _ = decode_error(version, &bytes);
        }
        for len in [0, 399, 401, 100_000] {
            let _ = decode_error(version, &vec![0; len]);
        }
    }
}

#[test]
fn family_signature_identity_and_rollback_domains_are_independent() {
    assert!(matches!(
        decode(&receipt_wire(1)),
        Err(Error::Framing(Framing::InvalidMagic("receipt")))
    ));
    assert!(matches!(
        LegacyReceipt::decode(&receipt_wire(2)),
        Err(Framing::InvalidMagic("receipt"))
    ));
    for version in [1, 2] {
        let other = 3 - version;
        let mut bytes = receipt_wire(version);
        let wrong = anchor(&bytes, other);
        bytes[240..272].copy_from_slice(&wrong);
        sign(&mut bytes, version, &SigningKey::from_bytes(&[0x51; 32]));
        assert_error_eq!(
            decode_error(version, &bytes),
            Framing::RollbackTransitionMismatch
        );
        bytes = receipt_wire(version);
        sign(&mut bytes, other, &SigningKey::from_bytes(&[0x51; 32]));
        seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT", version);
        assert_error_eq!(decode_error(version, &bytes), Framing::SignatureRejected);
        bytes = receipt_wire(version);
        seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT", other);
        assert_error_eq!(
            decode_error(version, &bytes),
            Framing::IdentityMismatch("receipt")
        );
    }
}

#[test]
fn resealed_signature_negatives_are_not_identity_failures() {
    for version in [1, 2] {
        for mode in 0..4 {
            let mut bytes = receipt_wire(version);
            match mode {
                0 => bytes[304] ^= 1,
                1 => bytes[336..368].fill(0xff),
                2 => {
                    bytes[304..336].fill(0);
                    bytes[304] = 1;
                }
                _ => {
                    let sig = SigningKey::from_bytes(&[0x51; 32]).sign(&bytes[..304]);
                    bytes[304..368].copy_from_slice(&sig.to_bytes());
                }
            }
            seal(&mut bytes, "COMPILER-EXECUTION-RECEIPT", version);
            assert_error_eq!(decode_error(version, &bytes), Framing::SignatureRejected);
        }
        let mut bytes = receipt_wire(version);
        bytes[272..304].fill(0);
        bytes[272] = 1;
        bytes[304] ^= 1;
        assert_error_eq!(decode_error(version, &bytes), Framing::WeakVerifyingKey);
    }
}

#[test]
fn decoding_keeps_v1_zero_field_and_signature_before_footer_contracts() {
    for version in [1, 2] {
        for (range, expected) in [
            (64..96, Framing::PolicyMismatch),
            (136..168, Framing::ChallengeMismatch),
            (168..200, Framing::ChallengeMismatch),
        ] {
            let mut bytes = receipt_wire(version);
            bytes[range].fill(0);
            rebuild(&mut bytes, version);
            assert_error_eq!(verify_error(version, &bytes), expected);
        }
        let mut bytes = receipt_wire(version);
        bytes[368..].fill(0);
        assert_error_eq!(
            decode_error(version, &bytes),
            Framing::IdentityMismatch("receipt")
        );
        bytes[304] ^= 1;
        assert_error_eq!(decode_error(version, &bytes), Framing::SignatureRejected);
    }
}

#[test]
fn paired_context_disagreements_freeze_verification_precedence() {
    for version in [1, 2] {
        for (first, second, expected) in [
            (64, 96, Framing::PolicyMismatch),
            (96, 200, Framing::SubjectMismatch),
            (200, 136, Framing::SequenceMismatch),
            (136, 24, Framing::ChallengeMismatch),
        ] {
            let mut bytes = receipt_wire(version);
            bytes[first] ^= 2;
            bytes[second] ^= 2;
            if bytes[200] != 1 {
                bytes[208..240].fill(0x77);
            }
            rebuild(&mut bytes, version);
            assert_error_eq!(verify_error(version, &bytes), expected);
        }
        let mut bytes = receipt_wire(version);
        bytes[24] ^= 1;
        rebuild(&mut bytes, version);
        assert_error_eq!(
            verify_error(version, &bytes),
            Framing::RollbackTransitionMismatch
        );
        let mut bytes = receipt_wire(version);
        let other_key = SigningKey::from_bytes(&[0x53; 32]);
        bytes[272..304].copy_from_slice(&other_key.verifying_key().to_bytes());
        sign(&mut bytes, version, &other_key);
        assert_error_eq!(verify_error(version, &bytes), Framing::PolicyMismatch);
    }
}

#[test]
fn exact_one_short_and_missing_input_budget_cases_precede_work() {
    let p = policy(&policy_wire(2));
    let q = request(&request_wire(2));
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let base = p.retained_storage() + q.retained_storage();
    for mode in 0..4 {
        let r = decode(&receipt_wire(2)).unwrap();
        let (cost, floor) = match mode {
            0 => (IW, base + size_of::<SigningKey>()),
            1 => (DW, 400),
            2 => (VW, base + r.retained_storage()),
            _ => (MW, 400),
        };
        for case in 0..4 {
            let r = decode(&receipt_wire(2)).unwrap();
            let mut work = Work::new(cost - usize::from(case == 1));
            let mut budget = Budget::new(&mut work, floor + RS - usize::from(case == 2));
            let entered = floor - usize::from(case == 3);
            budget.reserve_storage(entered).unwrap();
            let result = match mode {
                0 => Receipt::issue(&p, &q, &key, &mut budget).map(|_| ()),
                1 => Receipt::decode(&receipt_wire(2), &mut budget).map(|_| ()),
                2 => r.verify(&p, &q, [0; 32], &mut budget).map(|_| ()),
                _ => r
                    .identity()
                    .matches_canonical_bytes(&receipt_wire(2), &mut budget)
                    .map(|_| ()),
            };
            assert_eq!(budget.storage(), entered);
            match case {
                0 => {
                    result.unwrap();
                    assert_eq!(budget.work(), cost);
                    assert_eq!(budget.peak_storage(), floor + RS);
                }
                1 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
                    assert_eq!(budget.work(), 8);
                }
                2 => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
                    assert_eq!(budget.work(), cost);
                    assert_eq!(budget.failed_storage(), Some(floor + RS));
                }
                _ => {
                    assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
                    assert_eq!(budget.work(), 8);
                }
            }
        }
    }
}

#[test]
fn issuance_key_mismatch_precedes_request_policy_mismatch() {
    for version in [1, 2] {
        let mut changed = policy_wire(version);
        changed[24] ^= 1;
        seal(&mut changed, "COMPILER-EXECUTION-ISSUER-POLICY", version);
        for seed in [0x51, 0x53] {
            let expected = if seed == 0x53 {
                Framing::SigningKeyMismatch
            } else {
                Framing::PolicyMismatch
            };
            let key = SigningKey::from_bytes(&[seed; 32]);
            if version == 1 {
                assert_error_eq!(
                    LegacyReceipt::issue(
                        &LegacyPolicy::decode(&changed).unwrap(),
                        &LegacyRequest::decode(&request_wire(1)).unwrap(),
                        &key
                    )
                    .unwrap_err(),
                    expected
                );
            } else {
                let p = policy(&changed);
                let q = request(&request_wire(2));
                let mut work = Work::new(IW);
                let mut budget = Budget::new(&mut work, LIMIT);
                budget
                    .reserve_storage(
                        p.retained_storage() + q.retained_storage() + size_of::<SigningKey>(),
                    )
                    .unwrap();
                assert_error_eq!(
                    Receipt::issue(&p, &q, &key, &mut budget).unwrap_err(),
                    Error::Framing(expected)
                );
            }
        }
    }
}

#[test]
fn native_two_step_chain_rejects_stale_anchor_without_advancing_it() {
    let p = policy(&policy_wire(2));
    let first = decode(&receipt_wire(2)).unwrap();
    let prior = first.next_rollback_anchor();
    let mut wire = request_wire(2);
    wire[24 + 96..24 + 128].fill(0x72);
    wire[24 + 128..24 + 136].copy_from_slice(&2u64.to_le_bytes());
    wire[24 + 136..24 + 168].copy_from_slice(&prior);
    seal(&mut wire[24..224], "COMPILER-EXECUTION-CHALLENGE", 2);
    seal(&mut wire, "COMPILER-EXECUTION-REQUEST", 2);
    let q = request(&wire);
    let key = SigningKey::from_bytes(&[0x51; 32]);
    let mut work = Work::new(2 * (IW + VW));
    let mut budget = Budget::new(&mut work, LIMIT);
    let floor = p.retained_storage() + q.retained_storage() + size_of::<SigningKey>();
    budget.reserve_storage(floor).unwrap();
    for current in [[0; 32], prior] {
        let (r, charge) = Receipt::issue(&p, &q, &key, &mut budget).unwrap();
        assert_eq!(r.sequence(), 2);
        assert_eq!(r.prior_rollback_anchor(), prior);
        assert_ne!(r.next_rollback_anchor(), prior);
        assert_eq!(
            hex(&r.next_rollback_anchor()),
            "06bf2ab07aeda6ceba5e085ef5109cb87a2e634581a4ab0d7911db8ac43ab7ea"
        );
        assert_eq!(
            hex(r.identity().as_bytes()),
            "63c2b571dcbd63ad21b9a38ba19ffd34aa5ff47de3919cbbc90306a49c5fc8ff"
        );
        assert_eq!(
            hex(&Sha256::digest(r.canonical_bytes())),
            "e8f1f0feff702d277dfd16ddce6bc61d92197e61c9a7ad6c0e19305902d5be23"
        );
        budget.reserve_storage(charge.additional_storage()).unwrap();
        let result = r.verify(&p, &q, current, &mut budget);
        assert_eq!(budget.storage(), floor + charge.additional_storage());
        if current == prior {
            assert!(result.is_ok());
        } else {
            assert!(matches!(
                result,
                Err(Error::Framing(Framing::RollbackAnchorMismatch))
            ));
        }
        drop(result);
        budget.release_storage(charge.additional_storage()).unwrap();
    }
}

#[test]
fn adjacent_decode_faults_freeze_validation_order() {
    for version in [1, 2] {
        for (first, second, expected) in [
            (0, 24, Framing::InvalidMagic("receipt")),
            (24, 96, Framing::ZeroValue("attestation request")),
            (56, 96, Framing::RequestLengthMismatch),
            (96, 272, Framing::ZeroValue("compiler-execution subject")),
            (128, 272, Framing::SubjectLengthMismatch),
            (272, 200, Framing::WeakVerifyingKey),
            (200, 240, Framing::ZeroValue("attestation sequence")),
            (240, 304, Framing::RollbackTransitionMismatch),
            (304, 368, Framing::SignatureRejected),
        ] {
            let mut bytes = receipt_wire(version);
            for offset in [first, second] {
                match offset {
                    24 | 96 => bytes[offset..offset + 32].fill(0),
                    272 => {
                        bytes[272..304].fill(0);
                        bytes[272] = 1;
                    }
                    200 => bytes[200..208].fill(0),
                    _ => bytes[offset] ^= 1,
                }
            }
            assert_error_eq!(decode_error(version, &bytes), expected);
        }
    }
}

#[test]
fn every_pinned_policy_axis_and_valid_alternate_request_are_checked() {
    for offset in [24, 32, 64, 72, 104, 112, 144] {
        let mut bytes = policy_wire(2);
        if offset >= 112 {
            bytes[offset..offset + 32].copy_from_slice(
                &SigningKey::from_bytes(&[0x53; 32])
                    .verifying_key()
                    .to_bytes(),
            );
        } else {
            bytes[offset] ^= 1;
        }
        seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", 2);
        let p = policy(&bytes);
        let q = request(&request_wire(2));
        let r = decode(&receipt_wire(2)).unwrap();
        let mut work = Work::new(VW);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(p.retained_storage() + q.retained_storage() + r.retained_storage())
            .unwrap();
        assert!(matches!(
            r.verify(&p, &q, [0; 32], &mut budget),
            Err(Error::Framing(Framing::PolicyMismatch))
        ));
    }
    for subject_change in [false, true] {
        let mut bytes = request_wire(2);
        if subject_change {
            bytes[224 + 378] ^= 1;
            seal(&mut bytes[224..914], "INERT-COMPILER-EXECUTION-SUBJECT", 2);
            let identity: [u8; 32] = bytes[882..914].try_into().unwrap();
            bytes[24 + 56..24 + 88].copy_from_slice(&identity);
        } else {
            bytes[24 + 96] ^= 1;
        }
        seal(&mut bytes[24..224], "COMPILER-EXECUTION-CHALLENGE", 2);
        seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", 2);
        let p = policy(&policy_wire(2));
        let q = request(&bytes);
        let r = decode(&receipt_wire(2)).unwrap();
        let mut work = Work::new(VW);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(p.retained_storage() + q.retained_storage() + r.retained_storage())
            .unwrap();
        assert_error_eq!(
            r.verify(&p, &q, [0; 32], &mut budget).unwrap_err(),
            Error::Framing(if subject_change {
                Framing::SubjectMismatch
            } else {
                Framing::ChallengeMismatch
            })
        );
    }
}

#[test]
fn request_prior_is_checked_even_when_receipt_matches_current_anchor() {
    let p = policy(&policy_wire(2));
    let mut qwire = request_wire(2);
    qwire[152..160].copy_from_slice(&2u64.to_le_bytes());
    qwire[160..192].fill(0x81);
    seal(&mut qwire[24..224], "COMPILER-EXECUTION-CHALLENGE", 2);
    seal(&mut qwire, "COMPILER-EXECUTION-REQUEST", 2);
    let q = request(&qwire);
    for change_challenge in [false, true] {
        let mut bytes = receipt_wire(2);
        bytes[24..56].copy_from_slice(q.identity().as_bytes());
        bytes[136..168].copy_from_slice(q.challenge().identity().as_bytes());
        bytes[200..208].copy_from_slice(&2u64.to_le_bytes());
        bytes[208..240].fill(0x82);
        if change_challenge {
            bytes[136] ^= 1;
        }
        rebuild(&mut bytes, 2);
        let r = decode(&bytes).unwrap();
        let mut work = Work::new(VW);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget
            .reserve_storage(p.retained_storage() + q.retained_storage() + r.retained_storage())
            .unwrap();
        assert_error_eq!(
            r.verify(&p, &q, [0x82; 32], &mut budget).unwrap_err(),
            Error::Framing(if change_challenge {
                Framing::ChallengeMismatch
            } else {
                Framing::RollbackAnchorMismatch
            })
        );
    }
}

#[test]
fn full_key_owner_floor_and_prior_denial_history_are_preserved() {
    let p = policy(&policy_wire(2));
    let q = request(&request_wire(2));
    let key = SigningKey::from_bytes(&[0x51; 32]);
    assert!(size_of::<SigningKey>() > 64);
    for short in [32, 64] {
        let mut work = Work::new(IW);
        let mut budget = Budget::new(&mut work, LIMIT);
        let floor = p.retained_storage() + q.retained_storage() + short;
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            Receipt::issue(&p, &q, &key, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), 8);
        assert_eq!(budget.storage(), floor);
    }
    let r = decode(&receipt_wire(2)).unwrap();
    let floor = p.retained_storage() + q.retained_storage() + r.retained_storage() + 37;
    let mut work = Work::new(VW);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(budget.reserve_storage(LIMIT + 1).is_err());
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        r.verify(&p, &q, [1; 32], &mut budget),
        Err(Error::Framing(Framing::RollbackAnchorMismatch))
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.work(), VW);
    assert_eq!(budget.peak_storage(), floor + RS);
    assert_eq!(budget.failed_storage(), Some(LIMIT + 1));
}

//! Synthetic protocol fixtures, not protected compiler provenance.
use ed25519_dalek::SigningKey;
use fe2o3_artifact_transaction::{
    CompilerExecutionSubjectErrorV2 as SubjectError,
    INERT_COMPILER_EXECUTION_SUBJECT_WORK_V2 as SUBJECT_WORK,
    InertCompilerExecutionSubjectV2 as Subject,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_STORAGE_V2 as CS,
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_WORK_V2 as CW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_STORAGE_V2 as DS,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_DECODE_WORK_V2 as DW,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_STORAGE_V2 as RS,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_WORK_V2 as RW,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as PW,
    CompilerExecutionAttestationChallengeV1 as LegacyChallenge,
    CompilerExecutionAttestationChallengeV2 as Challenge,
    CompilerExecutionAttestationErrorV1 as Framing, CompilerExecutionAttestationErrorV2 as Error,
    CompilerExecutionAttestationRequestV1 as LegacyRequest,
    CompilerExecutionAttestationRequestV2 as Request, CompilerExecutionIssuerPolicyV2 as Policy,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use sha2::{Digest, Sha256};

const CHALLENGE_BYTES: usize = 200;
const SUBJECT_BYTES: usize = 690;
const REQUEST_BYTES: usize = 946;
const LIMIT: usize = 1_000_000;

fn seal(bytes: &mut [u8], kind: &str, version: u16) {
    let prefix = bytes.len() - 32;
    let mut hash = Sha256::new();
    hash.update(format!("FE2O3/{kind}/V{version}\0").as_bytes());
    hash.update((prefix as u64).to_le_bytes());
    hash.update(&bytes[..prefix]);
    bytes[prefix..].copy_from_slice(&hash.finalize());
}
fn header(bytes: &mut [u8], magic: &[u8; 8], version: u16) {
    let len = bytes.len();
    bytes[..8].copy_from_slice(magic);
    bytes[8..10].copy_from_slice(&version.to_le_bytes());
    bytes[12..20].copy_from_slice(&(len as u64).to_le_bytes());
}
fn policy_wire(version: u16) -> [u8; 216] {
    let mut bytes = [0; 216];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEP1"
        } else {
            b"F2O3CEP2"
        },
        version,
    );
    bytes[24..32].copy_from_slice(&7u64.to_le_bytes());
    bytes[32..64].fill(0x61);
    bytes[64..72].copy_from_slice(&12345u64.to_le_bytes());
    bytes[72..104].fill(0x62);
    bytes[104..112].copy_from_slice(&67890u64.to_le_bytes());
    bytes[112..144].copy_from_slice(
        &SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
    );
    bytes[144..176].copy_from_slice(
        &SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
    );
    bytes[176..178].copy_from_slice(&version.to_le_bytes());
    seal(&mut bytes, "COMPILER-EXECUTION-ISSUER-POLICY", version);
    bytes
}
fn subject_wire(version: u16) -> [u8; SUBJECT_BYTES] {
    let seed = 0x20;
    let mut bytes = [0; SUBJECT_BYTES];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CES1"
        } else {
            b"F2O3CES2"
        },
        version,
    );
    bytes[24..32].copy_from_slice(&9u64.to_le_bytes());
    bytes[32..48].fill(seed + 6);
    bytes[48..80].fill(seed + 7);
    bytes[88..120].fill(seed + 8);
    bytes[120..152].fill(seed + 9);
    let mut closure = Sha256::new();
    closure.update(b"fe2o3-compiler-closure-identity-v2\0");
    closure.update(1u16.to_le_bytes());
    for axis in 0..6 {
        let pin = [seed + axis as u8; 32];
        bytes[152 + axis * 32..184 + axis * 32].copy_from_slice(&pin);
        closure.update(pin);
    }
    bytes[344..346].copy_from_slice(&1u16.to_le_bytes());
    bytes[346..378].copy_from_slice(&closure.finalize());
    for axis in 0..7 {
        let offset = 378 + axis * 40;
        bytes[offset..offset + 32].fill(seed + 10 + axis as u8);
        bytes[offset + 32..offset + 40].copy_from_slice(&(1000 + axis as u64).to_le_bytes());
    }
    seal(&mut bytes, "INERT-COMPILER-EXECUTION-SUBJECT", version);
    bytes
}
fn challenge_wire(version: u16) -> [u8; CHALLENGE_BYTES] {
    let mut bytes = [0; CHALLENGE_BYTES];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEC1"
        } else {
            b"F2O3CEC2"
        },
        version,
    );
    bytes[24..56].copy_from_slice(&policy_wire(version)[184..]);
    bytes[56..88].copy_from_slice(&subject_wire(version)[658..]);
    bytes[88..96].copy_from_slice(&690u64.to_le_bytes());
    bytes[96..128].fill(0x71);
    bytes[128..136].copy_from_slice(&1u64.to_le_bytes());
    seal(&mut bytes, "COMPILER-EXECUTION-CHALLENGE", version);
    bytes
}
fn request_wire(version: u16) -> [u8; REQUEST_BYTES] {
    let mut bytes = [0; REQUEST_BYTES];
    header(
        &mut bytes,
        if version == 1 {
            b"F2O3CEQ1"
        } else {
            b"F2O3CEQ2"
        },
        version,
    );
    bytes[24..224].copy_from_slice(&challenge_wire(version));
    bytes[224..914].copy_from_slice(&subject_wire(version));
    seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", version);
    bytes
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn decode(bytes: &[u8]) -> Result<Request, Error> {
    let mut work = Work::new(DW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(bytes.len()).unwrap();
    let result = Request::decode(bytes, &mut budget);
    assert_eq!(budget.storage(), bytes.len());
    result.map(|(owner, storage)| {
        assert_eq!(storage.additional_storage(), owner.retained_storage());
        budget
            .reserve_storage(storage.additional_storage())
            .unwrap();
        owner
    })
}

#[test]
fn independent_v1_v2_transcripts_and_golden_identities() {
    for (version, challenge_id, request_id, challenge_sha, request_sha) in [
        (
            1,
            "614cd53f6fcbb98894083bb264ab54c90dc2da1400af7dd889aa294c1af3f51c",
            "0df597af35ab48bc4acca873997eabd24ed5e48f49677e4820343f65f05a8920",
            "0fdf6ffb206ebbf2460972abb41b865e0506924a098b725fdbe6ba66b59908a6",
            "1dba4449947c57f4d384ca9cc18864114ab1f0ad8dc4281c8d14b5d287e0ce13",
        ),
        (
            2,
            "3e4ee2483ae70aec4401962d305cc3a6c57d83467dd0f53f6266905dbe375186",
            "57ab801ce387808ea18af8e493c46229109c15a8c71f09ce7027f158f06dba79",
            "bf0a47ae7e01e14e3edc7cdfe410fe1aff6ebc0c80f557b697ffcf02f60180be",
            "1912b46a66a7953ef6f7ec9646a9e738c41db6e5598d69401a713db5f76bf094",
        ),
    ] {
        let challenge = challenge_wire(version);
        let request = request_wire(version);
        assert_eq!(hex(&challenge[168..]), challenge_id);
        assert_eq!(hex(&request[914..]), request_id);
        assert_eq!(hex(&Sha256::digest(challenge)), challenge_sha);
        assert_eq!(hex(&Sha256::digest(request)), request_sha);
        if version == 1 {
            assert_eq!(
                LegacyChallenge::decode(&challenge)
                    .unwrap()
                    .canonical_bytes(),
                &challenge
            );
            assert_eq!(
                LegacyRequest::decode(&request).unwrap().canonical_bytes(),
                &request
            );
        } else {
            let decoded = decode(&request).unwrap();
            assert_eq!(decoded.canonical_bytes(), &request);
            assert_eq!(decoded.challenge().canonical_bytes(), &challenge);
            assert_eq!(decoded.subject().canonical_bytes(), &subject_wire(2));
        }
    }
}

#[test]
fn constructors_transfer_exact_subject_and_challenge_reservations() {
    let mut work = Work::new(PW + SUBJECT_WORK + 3 * CW + 2 * RW);
    let mut budget = Budget::new(&mut work, LIMIT);
    let wires = 216 + SUBJECT_BYTES;
    budget.reserve_storage(wires).unwrap();
    let (policy, p) = Policy::decode(&policy_wire(2), &mut budget).unwrap();
    budget.reserve_storage(p.additional_storage()).unwrap();
    let (subject, s) = Subject::decode(&subject_wire(2), &mut budget).unwrap();
    budget.reserve_storage(s.retained_storage()).unwrap();
    let (challenge, c) =
        Challenge::new(&policy, &subject, [0x71; 32], 1, [0; 32], &mut budget).unwrap();
    budget.reserve_storage(c.additional_storage()).unwrap();
    assert_eq!(challenge.policy_identity(), policy.identity());
    assert_eq!(challenge.nonce(), [0x71; 32]);
    assert_eq!(challenge.sequence(), 1);
    assert_eq!(challenge.prior_rollback_anchor(), [0; 32]);
    assert!(
        challenge
            .subject()
            .matches_subject(&subject, &mut budget)
            .unwrap()
    );
    assert_eq!(challenge.subject().byte_len(), 690);
    assert_eq!(challenge.subject().sha256(), *subject.identity().sha256());
    assert!(
        challenge
            .identity()
            .matches_canonical_bytes(challenge.canonical_bytes(), &mut budget)
            .unwrap()
    );
    let inherited = c.additional_storage() + s.retained_storage();
    let floor = budget.storage();
    let (request, extra) = Request::new(challenge, subject, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        request.retained_storage(),
        inherited + extra.additional_storage()
    );
    budget.reserve_storage(extra.additional_storage()).unwrap();
    assert_eq!(request.canonical_bytes(), &request_wire(2));
    assert!(
        request
            .identity()
            .matches_canonical_bytes(request.canonical_bytes(), &mut budget)
            .unwrap()
    );
    assert_eq!(budget.work(), PW + SUBJECT_WORK + 3 * CW + 2 * RW);
}

#[test]
fn exact_and_one_short_nested_decode_limits_restore_input_floor() {
    for (work_limit, storage_limit) in [
        (DW, REQUEST_BYTES + DS),
        (DW - 1, REQUEST_BYTES + DS),
        (DW, REQUEST_BYTES + DS - 1),
        (RW - 1, REQUEST_BYTES + DS),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(REQUEST_BYTES).unwrap();
        let result = Request::decode(&request_wire(2), &mut budget);
        assert_eq!(budget.storage(), REQUEST_BYTES);
        if work_limit < RW {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(budget.work(), 8);
        } else if work_limit < DW {
            assert!(matches!(
                result,
                Err(Error::Subject(SubjectError::Resource(Resource::Work(_))))
            ));
            assert_eq!(budget.work(), RW + 8);
        } else if storage_limit < REQUEST_BYTES + DS {
            assert!(matches!(
                result,
                Err(Error::Subject(SubjectError::Resource(Resource::Storage(_))))
            ));
            assert_eq!(budget.failed_storage(), Some(REQUEST_BYTES + DS));
        } else {
            let (request, storage) = result.unwrap();
            assert_eq!(budget.work(), DW);
            assert_eq!(budget.peak_storage(), REQUEST_BYTES + DS);
            assert_eq!(storage.additional_storage(), request.retained_storage());
        }
    }
}

#[test]
fn challenge_exact_and_one_short_work_storage_limits() {
    for (work_limit, storage_limit) in [(CW, 200 + CS), (CW - 1, 200 + CS), (CW, 199 + CS)] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(200).unwrap();
        let result = Challenge::decode(&challenge_wire(2), &mut budget);
        assert_eq!(budget.storage(), 200);
        if work_limit < CW {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
        } else if storage_limit < 200 + CS {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
        } else {
            let (challenge, storage) = result.unwrap();
            assert_eq!(challenge.retained_storage(), storage.additional_storage());
            assert_eq!(budget.peak_storage(), 200 + CS);
        }
    }
}

#[test]
fn all_single_byte_mutations_and_wrong_lengths_reject() {
    for i in 0..REQUEST_BYTES {
        let mut bytes = request_wire(2);
        bytes[i] ^= 0x80;
        assert!(decode(&bytes).is_err(), "request byte {i}");
    }
    let mut work = Work::new((CHALLENGE_BYTES + 4) * CW);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(200).unwrap();
    for i in 0..CHALLENGE_BYTES {
        let mut bytes = challenge_wire(2);
        bytes[i] ^= 0x80;
        assert!(
            Challenge::decode(&bytes, &mut budget).is_err(),
            "challenge byte {i}"
        );
        assert_eq!(budget.storage(), 200);
    }
    for len in [0, 945, 947, 100_000] {
        assert!(decode(&vec![0; len]).is_err());
    }
    for len in [0, 199, 201, 100_000] {
        assert!(matches!(
            Challenge::decode(&vec![0; len], &mut budget),
            Err(Error::Framing(Framing::InvalidLength { .. }))
        ));
    }
}

#[test]
fn outer_and_resealed_nested_family_substitutions_reject_both_directions() {
    assert!(matches!(
        decode(&request_wire(1)),
        Err(Error::Framing(Framing::InvalidMagic("request")))
    ));
    assert!(matches!(
        LegacyRequest::decode(&request_wire(2)),
        Err(Framing::InvalidMagic("request"))
    ));
    for version in [1, 2] {
        for subject in [false, true] {
            let mut bytes = request_wire(version);
            if subject {
                bytes[224..914].copy_from_slice(&subject_wire(3 - version));
            } else {
                bytes[24..224].copy_from_slice(&challenge_wire(3 - version));
            }
            seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", version);
            if version == 1 {
                assert!(LegacyRequest::decode(&bytes).is_err());
            } else if subject {
                assert!(matches!(decode(&bytes), Err(Error::Subject(_))));
            } else {
                assert!(matches!(
                    decode(&bytes),
                    Err(Error::Framing(Framing::InvalidMagic("challenge")))
                ));
            }
        }
    }
}

#[test]
fn challenge_paired_error_precedence_and_rollback_positions() {
    for version in [1, 2] {
        let mut work = Work::new(20 * CW);
        let mut budget = Budget::new(&mut work, LIMIT);
        budget.reserve_storage(200).unwrap();
        let mut check = |bytes: &[u8]| -> Result<(), Framing> {
            if version == 1 {
                LegacyChallenge::decode(bytes).map(|_| ())
            } else {
                Challenge::decode(bytes, &mut budget)
                    .map(|_| ())
                    .map_err(|e| match e {
                        Error::Framing(e) => e,
                        _ => panic!("unexpected resource denial"),
                    })
            }
        };
        let mut bytes = challenge_wire(version);
        bytes[56..96].fill(0);
        bytes[168..].fill(0);
        bytes[24..56].fill(0);
        assert!(matches!(
            check(&bytes),
            Err(Framing::ZeroValue("compiler-execution subject"))
        ));
        bytes[56..96].copy_from_slice(&challenge_wire(version)[56..96]);
        assert!(matches!(
            check(&bytes),
            Err(Framing::ZeroValue("challenge"))
        ));
        seal(&mut bytes, "COMPILER-EXECUTION-CHALLENGE", version);
        assert!(matches!(
            check(&bytes),
            Err(Framing::ZeroValue("issuer policy"))
        ));
        for (sequence, prior, valid) in [
            (0u64, [0; 32], false),
            (1, [1; 32], false),
            (2, [0; 32], false),
            (2, [1; 32], true),
        ] {
            let mut bytes = challenge_wire(version);
            bytes[128..136].copy_from_slice(&sequence.to_le_bytes());
            bytes[136..168].copy_from_slice(&prior);
            seal(&mut bytes, "COMPILER-EXECUTION-CHALLENGE", version);
            assert_eq!(check(&bytes).is_ok(), valid);
        }
    }
}

#[test]
fn nested_subject_error_precedes_footer_and_footer_precedes_subject_mismatch() {
    for version in [1, 2] {
        let mut bytes = request_wire(version);
        bytes[224] ^= 1;
        bytes[914..].fill(0);
        if version == 1 {
            assert!(matches!(
                LegacyRequest::decode(&bytes),
                Err(Framing::Subject(_))
            ));
        } else {
            assert!(matches!(decode(&bytes), Err(Error::Subject(_))));
        }
        bytes[224..914].copy_from_slice(&subject_wire(version));
        bytes[224 + 120] ^= 1;
        seal(
            &mut bytes[224..914],
            "INERT-COMPILER-EXECUTION-SUBJECT",
            version,
        );
        if version == 1 {
            assert!(matches!(
                LegacyRequest::decode(&bytes),
                Err(Framing::ZeroValue("request"))
            ));
        } else {
            assert!(matches!(
                decode(&bytes),
                Err(Error::Framing(Framing::ZeroValue("request")))
            ));
        }
        seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", version);
        if version == 1 {
            assert!(matches!(
                LegacyRequest::decode(&bytes),
                Err(Framing::SubjectMismatch)
            ));
        } else {
            assert!(matches!(
                decode(&bytes),
                Err(Error::Framing(Framing::SubjectMismatch))
            ));
        }
    }
}

#[test]
fn independently_resealed_subject_axes_cannot_substitute_under_the_original_challenge() {
    for offset in [24, 32, 48, 88, 120, 378, 410, 618, 650] {
        let mut bytes = request_wire(2);
        bytes[224 + offset] ^= 1;
        seal(&mut bytes[224..914], "INERT-COMPILER-EXECUTION-SUBJECT", 2);
        seal(&mut bytes, "COMPILER-EXECUTION-REQUEST", 2);
        assert!(
            matches!(
                decode(&bytes),
                Err(Error::Framing(Framing::SubjectMismatch))
            ),
            "axis {offset}"
        );
    }
}

#[test]
fn consuming_mismatch_and_missing_input_floor_leave_only_the_inherited_reservation() {
    let mut work = Work::new(CW + SUBJECT_WORK + RW + DW);
    let mut budget = Budget::new(&mut work, LIMIT);
    assert!(matches!(
        Request::decode(&request_wire(2), &mut budget),
        Err(Error::Resource(Resource::Accounting))
    ));
    budget.reserve_storage(890).unwrap();
    let (challenge, c) = Challenge::decode(&challenge_wire(2), &mut budget).unwrap();
    budget.reserve_storage(c.additional_storage()).unwrap();
    let mut bytes = subject_wire(2);
    bytes[120] ^= 1;
    seal(&mut bytes, "INERT-COMPILER-EXECUTION-SUBJECT", 2);
    let (subject, s) = Subject::decode(&bytes, &mut budget).unwrap();
    budget.reserve_storage(s.retained_storage()).unwrap();
    let floor = budget.storage();
    assert!(matches!(
        Request::new(challenge, subject, &mut budget),
        Err(Error::Framing(Framing::SubjectMismatch))
    ));
    assert_eq!(budget.storage(), floor);
    budget
        .release_storage(c.additional_storage() + s.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 890);
}

#[test]
fn subject_storage_ceiling_is_not_bypassed_by_request_decode() {
    let mut work = Work::new(DW);
    let mut budget = Budget::new(
        &mut work,
        fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4 + 1,
    );
    budget.reserve_storage(REQUEST_BYTES).unwrap();
    assert!(matches!(
        Request::decode(&request_wire(2), &mut budget),
        Err(Error::Subject(SubjectError::Resource(Resource::Accounting)))
    ));
    assert_eq!(budget.storage(), REQUEST_BYTES);
    assert_eq!(budget.work(), RW + 8);
    assert_eq!(budget.peak_storage(), REQUEST_BYTES + RS);
}

#[test]
fn constructor_exact_and_one_short_limits_preserve_consumed_input_charges() {
    use std::mem::size_of;
    let inherited = size_of::<Challenge>()
        + size_of::<Subject>()
        + size_of::<fe2o3_compiler_execution_protocol::CompilerExecutionAttestationStorageV2>()
        + size_of::<fe2o3_artifact_transaction::InertCompilerExecutionSubjectStorageV2>();
    for (short_work, short_storage) in [(false, false), (true, false), (false, true)] {
        let mut work = Work::new(CW + SUBJECT_WORK + RW - usize::from(short_work));
        let mut budget = Budget::new(&mut work, 890 + inherited + RS - usize::from(short_storage));
        budget.reserve_storage(890).unwrap();
        let (challenge, c) = Challenge::decode(&challenge_wire(2), &mut budget).unwrap();
        budget.reserve_storage(c.additional_storage()).unwrap();
        let (subject, s) = Subject::decode(&subject_wire(2), &mut budget).unwrap();
        budget.reserve_storage(s.retained_storage()).unwrap();
        assert_eq!(c.additional_storage() + s.retained_storage(), inherited);
        let result = Request::new(challenge, subject, &mut budget);
        assert_eq!(budget.storage(), 890 + inherited);
        if short_work {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(budget.work(), CW + SUBJECT_WORK + 8);
        } else if short_storage {
            assert!(matches!(result, Err(Error::Resource(Resource::Storage(_)))));
            assert_eq!(budget.failed_storage(), Some(890 + inherited + RS));
        } else {
            let (request, delta) = result.unwrap();
            assert_eq!(
                request.retained_storage(),
                inherited + delta.additional_storage()
            );
            assert_eq!(budget.work(), CW + SUBJECT_WORK + RW);
            assert_eq!(budget.peak_storage(), 890 + inherited + RS);
        }
    }
}

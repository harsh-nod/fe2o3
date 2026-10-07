//! Codec/accounting tests with inert records, not execution or protected custody.
use super::*;
use crate as protocol;
use crate::NativeApplicationSessionTranscriptV1 as Transcript;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use message::{NativeApplicationProofKindV1 as Kind, NativeApplicationProofMessageV1 as Message};
#[allow(dead_code)]
#[path = "../../tests/support/native_application_registration_fixture.rs"]
mod fixture;

fn parts() -> Parts {
    Parts {
        native_handoff: ([11; 32], 100),
        final_kernel_ir: ([12; 32], 101),
        analysis_request: ([13; 32], 102),
        analysis_bundle: ([14; 32], 103),
        analysis_receipt: ([15; 32], 104),
        generated_source: ([16; 32], 105),
        obligation: ([17; 32], 106),
        signed_receipt: ([18; 32], 107),
        analysis_execution_identity: [19; 32],
        analysis_challenge: [20; 32],
        receipt_verifying_key: [21; 32],
        carriage_identity: [22; 32],
        subject_identity: [23; 32],
        policy_identity: [24; 32],
    }
}
fn retain<T>(result: Result<(T, Storage)>, b: &mut Budget<'_>) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.retained_storage()).unwrap();
    value
}
fn session(binding: &Registration, seed: u8, b: &mut Budget<'_>) -> Session {
    let (transcript, charge) = Transcript::from_untrusted_parts(
        [seed; 32],
        [seed + 1; 32],
        *binding.identity().as_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (session, charge) =
        Session::new(transcript, [3; 32], [seed + 2; 32], (123, 1001, 1001), b).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    session
}
fn setup() -> (Owned, Registration, Session, Inputs, Evidence) {
    let mut account = Owned::new(Work::new(1_000_000_000), 16_000_000);
    let (registration, session, inputs, evidence) = account.with_budget(|b| {
        let registration = fixture::binding(1201, 1200, 1001, 1001, b);
        let session = session(&registration, 4, b);
        let inputs = retain(Inputs::new(&session, &registration, ([9; 32], 260), b), b);
        let evidence = retain(Evidence::new(&inputs, parts(), b), b);
        (registration, session, inputs, evidence)
    });
    (account, registration, session, inputs, evidence)
}
fn exact<T>(q: Quote, run: impl FnOnce(&mut Budget<'_>) -> Result<(T, Storage)>) -> (T, Storage) {
    let mut work = Work::new(q.work());
    let mut b = Budget::new(&mut work, q.input_floor() + q.scratch());
    b.reserve_storage(q.input_floor()).unwrap();
    let (value, storage) = run(&mut b).unwrap();
    assert_eq!(b.storage(), q.input_floor());
    assert_eq!(b.work(), q.work());
    assert_eq!(storage.retained_storage(), q.retained_storage());
    (value, storage)
}

#[test]
fn native_proof_records_bind_inputs_and_remain_inert() {
    let (_, registration, session, inputs, evidence) = setup();
    assert_eq!(inputs.session_identity(), session.identity());
    assert_eq!(
        inputs.registration_identity(),
        *registration.identity().as_bytes()
    );
    assert_eq!(inputs.deployment_identity(), session.deployment());
    assert_eq!(
        inputs.readiness(),
        (
            registration.inputs().readiness_sha256(),
            registration.inputs().readiness_byte_len()
        )
    );
    assert_eq!(inputs.payload(), ([9; 32], 260));
    assert!(!inputs.authenticates_input_custody());
    assert_eq!(evidence.inputs_identity(), inputs.identity());
    assert_eq!(evidence.parts(), parts());
    assert_eq!(evidence.boundary(), 6);
    assert!(!evidence.authenticates_proof_execution());
    assert!(!evidence.authenticates_currentness());
    assert!(!evidence.grants_launch_authority());
    let (decoded, _) = exact(Inputs::decoding_quote(), |b| {
        Inputs::decode(inputs.canonical_bytes(), b)
    });
    assert_eq!(decoded, inputs);
    let (decoded, _) = exact(Evidence::decoding_quote(), |b| {
        Evidence::decode(evidence.canonical_bytes(), b)
    });
    assert_eq!(decoded, evidence);
    let (decoded, _) = exact(Inputs::decoding_quote(), |b| evidence.decode_inputs(b));
    assert_eq!(decoded, inputs);
}

#[test]
fn native_proof_frames_round_trip_exact_phase_and_rights_shapes() {
    let (mut account, _, session, inputs, evidence) = setup();
    let messages = account.with_budget(|b| {
        [
            retain(Message::active(&session, b), b),
            retain(Message::request(&session, &inputs, b), b),
            retain(Message::proved(&session, &evidence, b), b),
            retain(Message::probe(&session, 2, b), b),
            retain(Message::retained(&session, 2, &evidence, b), b),
            retain(
                Message::rejected(&session, 3, "retained custody unavailable", b),
                b,
            ),
        ]
    });
    for message in &messages {
        assert_eq!(
            message.required_rights(),
            usize::from(message.kind() == Kind::Request) * 2
        );
        assert!(!message.authenticates_sender());
        assert!(!message.grants_launch_authority());
        let q = Message::decoding_quote(message.kind(), message.canonical_bytes().len(), &session)
            .unwrap();
        let (decoded, _) = exact(q, |b| {
            Message::decode(message.canonical_bytes(), &session, b)
        });
        assert_eq!(&decoded, message);
    }
    account.with_budget(|b| {
        assert_eq!(messages[1].decode_inputs(b).unwrap().0, inputs);
        assert_eq!(messages[2].decode_evidence(b).unwrap().0, evidence);
        assert!(messages[0].decode_inputs(b).is_err());
        assert!(messages[1].decode_evidence(b).is_err());
        for seq in [0, 1, u64::MAX] {
            assert!(Message::probe(&session, seq, b).is_err());
            assert!(Message::retained(&session, seq, &evidence, b).is_err());
        }
        for text in [String::new(), "x".repeat(513)] {
            assert!(Message::rejected(&session, 2, &text, b).is_err());
        }
        assert!(Message::rejected(&session, 0, "bad", b).is_err());
    });
}

#[test]
fn native_proof_codecs_reject_every_byte_tamper_trailing_and_legacy_families() {
    let (mut account, _, session, inputs, evidence) = setup();
    let message = account.with_budget(|b| retain(Message::proved(&session, &evidence, b), b));
    account.with_budget(|b| {
        for (bytes, kind) in [
            (inputs.canonical_bytes().as_slice(), 0),
            (evidence.canonical_bytes().as_slice(), 1),
            (message.canonical_bytes(), 2),
        ] {
            for index in 0..bytes.len() {
                let mut changed = bytes.to_vec();
                changed[index] ^= 1;
                let accepted = match kind {
                    0 => Inputs::decode(&changed, b).is_ok(),
                    1 => Evidence::decode(&changed, b).is_ok(),
                    _ => Message::decode(&changed, &session, b).is_ok(),
                };
                assert!(!accepted, "kind={kind} index={index}");
            }
            let mut legacy = bytes.to_vec();
            legacy[..8].copy_from_slice(b"F3APMS1\0");
            seal(&mut legacy);
            assert!(Inputs::decode(&legacy, b).is_err());
            assert!(Evidence::decode(&legacy, b).is_err());
            assert!(Message::decode(&legacy, &session, b).is_err());
            assert!(Message::decode(&bytes[..bytes.len() - 1], &session, b).is_err());
            let mut trailing = bytes.to_vec();
            trailing.push(0);
            assert!(Message::decode(&trailing, &session, b).is_err());
        }
    });
}

#[test]
fn native_proof_resealed_wrong_profiles_and_nested_associations_are_refused() {
    let (mut account, registration, session, inputs, evidence) = setup();
    let message = account.with_budget(|b| retain(Message::proved(&session, &evidence, b), b));
    account.with_budget(|b| {
        inputs.check_registration(&registration, b).unwrap();
        evidence.check_inputs(&inputs, b).unwrap();
        let mut changed_readiness = *inputs.canonical_bytes();
        changed_readiness[120] ^= 1;
        seal(&mut changed_readiness);
        let (changed, _) = Inputs::decode(&changed_readiness, b).unwrap();
        assert!(changed.check_registration(&registration, b).is_err());
        assert!(evidence.check_inputs(&changed, b).is_err());
        let mut changed_payload = *inputs.canonical_bytes();
        changed_payload[160] ^= 1;
        seal(&mut changed_payload);
        let (changed, _) = Inputs::decode(&changed_payload, b).unwrap();
        assert!(evidence.check_inputs(&changed, b).is_err());
        let other = self::session(&registration, 30, b);
        assert!(Message::request(&other, &inputs, b).is_err());
        assert!(Message::proved(&other, &evidence, b).is_err());
        assert!(Message::decode(message.canonical_bytes(), &other, b).is_err());
        for (offset, replacement) in [(10, 1), (16, 1), (24, 0)] {
            let mut changed = *inputs.canonical_bytes();
            if offset == 24 {
                changed[24..56].fill(0);
            } else {
                changed[offset] = replacement;
            }
            seal(&mut changed);
            assert!(Inputs::decode(&changed, b).is_err());
        }
        for maximum in [
            0,
            NATIVE_APPLICATION_PROOF_MAX_PAYLOAD_BYTES_V1 as u64 + 1,
            u64::MAX,
        ] {
            assert!(Inputs::new(&session, &registration, ([9; 32], maximum), b).is_err());
        }
        for offset in [
            HEADER + INPUT_BYTES,
            HEADER + INPUT_BYTES + 2,
            Evidence::IDS,
        ] {
            let mut changed = *evidence.canonical_bytes();
            if offset == Evidence::IDS {
                changed[offset..offset + 32].fill(0);
            } else {
                changed[offset] ^= 1;
            }
            seal(&mut changed);
            assert!(Evidence::decode(&changed, b).is_err());
        }
        for offset in [11, 56] {
            let mut changed = message.canonical_bytes().to_vec();
            changed[offset] = 1;
            seal(&mut changed);
            assert!(Message::decode(&changed, &session, b).is_err());
        }
        let mut unknown_kind = message.canonical_bytes().to_vec();
        unknown_kind[10] = 7;
        seal(&mut unknown_kind);
        assert!(Message::decode(&unknown_kind, &session, b).is_err());
        let mut wrong_nested = message.canonical_bytes().to_vec();
        let nested = &mut wrong_nested[64..64 + EVIDENCE_BYTES];
        let input = &mut nested[HEADER..HEADER + INPUT_BYTES];
        input[24] ^= 1;
        seal(input);
        seal(nested);
        seal(&mut wrong_nested);
        assert!(Message::decode(&wrong_nested, &session, b).is_err());
        for sequence in [0u64, 2, u64::MAX] {
            let mut changed = message.canonical_bytes().to_vec();
            changed[48..56].copy_from_slice(&sequence.to_le_bytes());
            seal(&mut changed);
            assert!(Message::decode(&changed, &session, b).is_err());
        }
    });
}

#[test]
fn every_evidence_axis_changes_its_exact_identity_but_never_authenticates_it() {
    let (mut account, _, _, _, evidence) = setup();
    account.with_budget(|b| {
        for offset in (Evidence::PARTS..Evidence::IDS)
            .step_by(40)
            .chain((Evidence::IDS..EVIDENCE_BYTES - 32).step_by(32))
        {
            let mut changed = *evidence.canonical_bytes();
            changed[offset] ^= 1;
            seal(&mut changed);
            let (other, _) = Evidence::decode(&changed, b).unwrap();
            assert_ne!(other.identity(), evidence.identity());
            assert!(!other.authenticates_proof_execution());
        }
        for offset in (Evidence::PARTS..Evidence::IDS).step_by(40) {
            let mut changed = *evidence.canonical_bytes();
            changed[offset + 32..offset + 40].fill(0);
            seal(&mut changed);
            assert!(Evidence::decode(&changed, b).is_err());
            changed[offset + 32..offset + 40].copy_from_slice(&u64::MAX.to_le_bytes());
            seal(&mut changed);
            assert!(Evidence::decode(&changed, b).is_err());
        }
    });
}

#[test]
fn native_proof_exact_decode_quotes_and_one_short_denials_share_original_ledger() {
    let (mut account, _, session, inputs, evidence) = setup();
    let message = account.with_budget(|b| retain(Message::proved(&session, &evidence, b), b));
    for kind in 0..3 {
        let q = match kind {
            0 => Inputs::decoding_quote(),
            1 => Evidence::decoding_quote(),
            _ => Message::decoding_quote(Kind::Proved, message.canonical_bytes().len(), &session)
                .unwrap(),
        };
        for short in 0..4 {
            let mut work = Work::new(q.work() - usize::from(short == 1));
            let floor = q.input_floor() - usize::from(short == 3);
            let mut b = Budget::new(&mut work, floor + q.scratch() - usize::from(short == 2));
            b.reserve_storage(floor).unwrap();
            let ledger = b.work_ledger_identity_v1();
            let before_work = b.work();
            let accepted = match kind {
                0 => Inputs::decode(inputs.canonical_bytes(), &mut b).is_ok(),
                1 => Evidence::decode(evidence.canonical_bytes(), &mut b).is_ok(),
                _ => Message::decode(message.canonical_bytes(), &session, &mut b).is_ok(),
            };
            assert_eq!(accepted, short == 0, "kind={kind} short={short}");
            assert!(b.work_ledger_identity_v1() == ledger);
            assert_eq!(b.storage(), floor);
            assert!(b.work() > before_work);
            if short == 0 {
                assert_eq!(b.work(), q.work());
                assert_eq!(b.peak_storage(), floor + q.scratch());
            }
            if short == 1 {
                assert!(b.failed_work().is_some());
            }
            if short == 2 {
                assert!(b.failed_storage().is_some());
            }
        }
    }
}

#[test]
fn native_transcript_public_codec_is_metered_inert_and_rejects_invalid_coordinates() {
    let mut account = Owned::new(Work::new(1_000_000), 1_000_000);
    account.with_budget(|b| {
        let (value, charge) =
            Transcript::from_untrusted_parts([1; 32], [2; 32], [3; 32], b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert!(!value.authenticates_sender());
        let bytes = value.canonical_bytes();
        b.reserve_storage(bytes.len()).unwrap();
        assert_eq!(Transcript::decode(&bytes, b).unwrap().0, value);
        for index in 0..3 {
            let mut changed = bytes;
            changed[index * 32..(index + 1) * 32].fill(0);
            assert!(Transcript::decode(&changed, b).is_err());
        }
        assert!(Transcript::decode(&bytes[..95], b).is_err());
        assert!(Transcript::from_untrusted_parts([1; 32], [1; 32], [3; 32], b).is_err());
    });
}

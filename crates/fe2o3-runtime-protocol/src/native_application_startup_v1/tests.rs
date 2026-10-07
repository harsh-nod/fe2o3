use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

fn session(budget: &mut Budget<'_>) -> Session {
    let (transcript, s) =
        Transcript::from_untrusted_parts([1; 32], [2; 32], [3; 32], budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    let (session, s) =
        Session::new(transcript, [4; 32], [5; 32], (1234, 1001, 1002), budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    session
}

#[test]
fn native_ready_ack_roundtrip_retains_original_account_without_authority() {
    let mut owned = Owned::new(Work::new(usize::MAX), 1_000_000);
    owned.with_budget(|budget| {
        let session = session(budget);
        let original = (
            budget.work_ledger_identity_v1(),
            budget.storage_account_identity_v1(),
        );
        let floor = budget.storage();
        let (ready, s) = Record::ready(&session, [6; 32], budget).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(s.additional_storage()).unwrap();
        assert_eq!(ready.kind(), Kind::CurrentnessReady);
        assert_eq!(ready.transcript(), session.transcript());
        assert_eq!(ready.proof_session_identity(), session.identity());
        assert_eq!(ready.currentness_gate_identity(), [6; 32]);
        let (ack, s) = Record::acknowledge(&ready, budget).unwrap();
        budget.reserve_storage(s.additional_storage()).unwrap();
        ack.check_acknowledgment(&ready, budget).unwrap();
        assert_ne!(ready.identity(), ack.identity());
        assert_ne!(ready.canonical_bytes(), ack.canonical_bytes());
        assert!(!ready.grants_launch_authority() && !ack.grants_launch_authority());
        assert!(Record::acknowledge(&ack, budget).is_err());
        assert!(ready.check_acknowledgment(&ack, budget).is_err());
        for record in [&ready, &ack] {
            budget.reserve_storage(BYTES).unwrap();
            let before = budget.storage();
            let (decoded, s) = Record::decode(record.canonical_bytes(), budget).unwrap();
            assert_eq!(&decoded, record);
            assert_eq!(s.additional_storage(), Record::STORAGE);
            assert_eq!(budget.storage(), before);
            budget.release_storage(BYTES).unwrap();
        }
        assert!(budget.work_ledger_identity_v1() == original.0);
        assert_eq!(budget.storage_account_identity_v1(), original.1);
    });
}

#[test]
fn native_startup_rejects_each_wire_byte_and_cross_session_or_gate_substitution() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let session = session(&mut budget);
    let (ready, s) = Record::ready(&session, [6; 32], &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    let (ack, s) = Record::acknowledge(&ready, &mut budget).unwrap();
    budget.reserve_storage(s.additional_storage()).unwrap();
    budget.reserve_storage(BYTES).unwrap();
    for index in 0..BYTES {
        let mut bytes = *ready.canonical_bytes();
        bytes[index] ^= 1;
        assert!(
            Record::decode(&bytes, &mut budget).is_err(),
            "changed byte {index}"
        );
    }
    assert!(Record::decode(&ready.canonical_bytes()[..BYTES - 1], &mut budget).is_err());
    let mut trailing = ready.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(Record::decode(&trailing, &mut budget).is_err());
    for (transcript, session_id, gate) in [
        (
            Transcript::from_untrusted_parts([7; 32], [2; 32], [3; 32], &mut budget)
                .unwrap()
                .0,
            session.identity(),
            [6; 32],
        ),
        (session.transcript(), [8; 32], [6; 32]),
        (session.transcript(), session.identity(), [9; 32]),
    ] {
        let other = Record::encode(Kind::CurrentnessReady, transcript, session_id, gate).unwrap();
        assert!(ack.check_acknowledgment(&other, &mut budget).is_err());
    }
    assert!(Record::ready(&session, [0; 32], &mut budget).is_err());
    for offset in [16, 48, 80, 112, 144] {
        let mut bytes = *ready.canonical_bytes();
        bytes[offset..offset + 32].fill(0);
        let mut hash = Sha256::new();
        hash.update(READY_DOMAIN);
        hash.update(&bytes[..176]);
        bytes[176..].copy_from_slice(&hash.finalize());
        assert!(Record::decode(&bytes, &mut budget).is_err());
    }
}

#[test]
fn native_startup_exact_and_one_short_work_storage_gates() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let session = session(&mut budget);
    let (ready, _) = Record::ready(&session, [6; 32], &mut budget).unwrap();
    let bytes = *ready.canonical_bytes();
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(BYTES).unwrap();
    Record::decode(&bytes, &mut budget).unwrap();
    let limits = (budget.work(), budget.peak_storage());
    for (w, s, ok) in [
        (limits.0, limits.1, true),
        (limits.0 - 1, limits.1, false),
        (limits.0, limits.1 - 1, false),
    ] {
        let mut work = Work::new(w);
        let mut budget = Budget::new(&mut work, s);
        budget.reserve_storage(BYTES).unwrap();
        assert_eq!(Record::decode(&bytes, &mut budget).is_ok(), ok);
        assert_eq!(budget.storage(), BYTES);
    }
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 1_000_000);
    assert!(
        Record::decode(&bytes, &mut budget).is_err(),
        "unpaid bytes refused"
    );
    assert_eq!(budget.storage(), 0);
}

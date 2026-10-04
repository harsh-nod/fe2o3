//! Shared V2/V3 session predicates over actual signed journal recovery.
//! Inert subjects and matching journal bytes are not live compiler authority.
//! These tests construct neither Admission nor NativeOccurrence and do not
//! establish live observation, publication-lock retention or successful retirement.
use super::super::tests::{custody, directory};
use super::*;
use fe2o3_artifact_transaction::MAX_COMPILER_MODULE_HANDOFF_STORAGE_V4 as STORAGE_LIMIT;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::os::fd::AsFd;

#[allow(dead_code)]
#[path = "../../fe2o3-compiler-execution-protocol/tests/support/native_attestation_fixture.rs"]
mod fixture;

#[derive(Clone, Copy, Debug)]
enum Stage {
    Ready,
    Prepared,
    Issued,
}

fn subject(seed: u8, b: &mut Budget<'_>) -> Subject {
    let (subject, charge) = b
        .with_prepaid_scope(b.storage(), 8, 8, fixture::SUBJECT_BYTES, |b| {
            let mut bytes = fixture::subject_wire(SUBJECT_VERSION);
            bytes[32..48].fill(seed);
            fixture::seal(
                &mut bytes,
                "INERT-COMPILER-EXECUTION-SUBJECT",
                SUBJECT_VERSION,
            );
            Subject::decode(&bytes, b)
        })
        .unwrap();
    b.reserve_storage(charge.retained_storage()).unwrap();
    subject
}

fn recovered(stage: Stage, b: &mut Budget<'_>) -> (tempfile::TempDir, Ledger) {
    let (policy, key) = custody(b);
    let (directory, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &policy, &key, b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    if matches!(stage, Stage::Prepared | Stage::Issued) {
        let subject = subject(1, b);
        let next = ledger
            .record
            .fixture_prepared(subject, &policy, &key, b)
            .unwrap();
        b.reserve_storage(Record::STORAGE).unwrap();
        ledger.commit(next, b).unwrap();
    }
    if matches!(stage, Stage::Issued) {
        let next = ledger.record.fixture_issued(&policy, &key, b).unwrap();
        b.reserve_storage(Record::STORAGE).unwrap();
        ledger.commit(next, b).unwrap();
    }
    b.reserve_storage(record::BYTES).unwrap();
    let committed = *ledger.record.bytes();
    drop(ledger);
    b.release_storage(Ledger::STORAGE).unwrap();

    let recovered = Ledger::recover(root.as_fd(), &policy, &key, b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    assert_eq!(recovered.record.bytes(), &committed);
    assert!(matches!(
        (stage, &recovered.record.body),
        (Stage::Ready, Body::Ready)
            | (Stage::Prepared, Body::Prepared { .. })
            | (Stage::Issued, Body::Issued { .. })
    ));
    recovered.validate(b).unwrap();
    (directory, recovered)
}

fn pending_subject(record: &Record) -> &Subject {
    match &record.body {
        Body::Prepared { subject, .. } => subject,
        Body::Issued { request, .. } => request.subject(),
        Body::Ready => panic!("fixture must contain a pending signed record"),
    }
}

fn assert_missing_owner(stage: Stage) {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (_directory, ledger) = recovered(stage, &mut b);
    let record = &ledger.record;

    // Even exact inert joins cannot populate the session's missing live owner.
    matches_pending(record, pending_subject(record), &record.occurrence).unwrap();
    let session = Session::default();
    assert!(session.occurrence().is_err());
    assert!(session.check_record(record).is_err(), "stage={stage:?}");
    assert_eq!(session.retained_storage(), 0);
    ledger.validate(&mut b).unwrap();
}

#[test]
fn recovered_prepared_signed_record_cannot_authorize_missing_live_owner() {
    assert_missing_owner(Stage::Prepared);
}

#[test]
fn recovered_issued_signed_record_cannot_authorize_missing_live_owner() {
    assert_missing_owner(Stage::Issued);
}

#[test]
fn pending_record_rejects_identical_subject_with_wrong_occurrence_identity() {
    for stage in [Stage::Prepared, Stage::Issued] {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        let (_directory, ledger) = recovered(stage, &mut b);
        let record = &ledger.record;
        let subject = pending_subject(record);
        matches_pending(record, subject, &record.occurrence).unwrap();
        let mut changed = record.occurrence;
        changed[0] ^= 1;
        assert_ne!(changed, record.occurrence);
        assert!(
            matches_pending(record, subject, &changed).is_err(),
            "stage={stage:?}"
        );
        ledger.validate(&mut b).unwrap();
    }
}

#[test]
fn pending_record_rejects_changed_subject_with_exact_occurrence_identity() {
    for stage in [Stage::Prepared, Stage::Issued] {
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        let (_directory, ledger) = recovered(stage, &mut b);
        let record = &ledger.record;
        let changed = subject(2, &mut b);
        assert_ne!(
            changed.canonical_bytes(),
            pending_subject(record).canonical_bytes()
        );
        matches_pending(record, pending_subject(record), &record.occurrence).unwrap();
        assert!(
            matches_pending(record, &changed, &record.occurrence).is_err(),
            "stage={stage:?}"
        );
        ledger.validate(&mut b).unwrap();
    }
}

#[test]
fn recovered_ready_record_has_no_pending_occurrence() {
    let mut work = Work::new(usize::MAX);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let (_directory, ledger) = recovered(Stage::Ready, &mut b);
    let subject = subject(1, &mut b);
    let session = Session::default();
    session.check_record(&ledger.record).unwrap();
    assert!(session.occurrence().is_err());
    assert_eq!(session.retained_storage(), 0);
    assert_eq!(ledger.record.occurrence, [0; 32]);
    for identity in [ledger.record.occurrence, [43; 32]] {
        assert!(matches_pending(&ledger.record, &subject, &identity).is_err());
    }
    ledger.validate(&mut b).unwrap();
}

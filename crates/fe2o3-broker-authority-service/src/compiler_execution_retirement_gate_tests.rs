//! Shared V2/V3 durable-retirement predicates over ordinary signed journals.
//! No Admission or NativeOccurrence is constructed; these fixtures establish
//! neither live-owner lock retirement nor protected execution or GPU credit.
use super::*;

const WORK_LIMIT: usize = 64 * 1024 * 1024 * 1024;

fn gate_without_mutation(
    ledger: &Ledger,
    publication: &Publication,
    ack: &Ack,
    root: &std::path::Path,
    b: &mut Budget<'_>,
) -> Result<Carriage> {
    let before = snapshot(root);
    let record = *ledger.record.bytes();
    let poisoned = ledger.poisoned;
    let floor = b.storage();
    let meter = b.work_ledger_identity_v1();
    let result = ledger.retirement_carriage(publication, ack, b);
    assert_eq!(b.storage(), floor, "gate returns full unreserved storage");
    assert!(b.work_ledger_identity_v1() == meter);
    assert_eq!(ledger.record.bytes(), &record);
    assert_eq!(ledger.poisoned, poisoned);
    assert_eq!(snapshot(root), before, "gate must not mutate durable state");
    result
}

fn completed_carriage(
    ledger: &Ledger,
    publication: &Publication,
    ack: &Ack,
    root: &std::path::Path,
    b: &mut Budget<'_>,
) -> Vec<u8> {
    let floor = b.storage();
    let work = b.work();
    let carriage = gate_without_mutation(ledger, publication, ack, root, b).unwrap();
    assert!(b.work() > work, "each replay must debit the original meter");
    assert_eq!(
        carriage.publication().canonical_bytes(),
        publication.canonical_bytes()
    );
    assert_eq!(
        carriage.acknowledgment().canonical_bytes(),
        ack.canonical_bytes()
    );
    let charge = carriage.retained_storage();
    b.reserve_storage(charge).unwrap();
    assert_eq!(b.storage(), floor + charge);
    let bytes = carriage.canonical_bytes().to_vec();
    drop(carriage);
    b.release_storage(charge).unwrap();
    assert_eq!(b.storage(), floor);
    bytes
}

fn inert_carriage(
    policy: &Policy,
    request: &Request,
    publication: &Publication,
    ack: &Ack,
    b: &mut Budget<'_>,
) -> Carriage {
    let p = retain(Policy::decode(policy.canonical_bytes(), b).unwrap(), b).unwrap();
    let q = retain(Request::decode(request.canonical_bytes(), b).unwrap(), b).unwrap();
    let u = retain(
        Publication::decode(publication.canonical_bytes(), b).unwrap(),
        b,
    )
    .unwrap();
    let a = retain(Ack::decode(ack.canonical_bytes(), b).unwrap(), b).unwrap();
    retain(Carriage::new(p, q, u, a, b).unwrap(), b).unwrap()
}

#[test]
fn retirement_requires_all_five_durable_commits_and_replays_a_lost_completion_reply() {
    for committed in 0..=5 {
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(65536).unwrap();
        let (p, key) = custody(&mut b);
        let (dir, root) = directory();
        let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        b.reserve_storage(Ledger::STORAGE).unwrap();
        let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
        let mut fault = NthFault {
            boundary: if committed == 0 {
                Boundary::CreateTemp
            } else {
                Boundary::SyncCanonicalName
            },
            timing: if committed == 0 {
                Timing::Before
            } else {
                Timing::After
            },
            remaining: committed.max(1),
            fired: false,
        };
        assert!(
            ledger
                .publish_with_hooks(&p, &key, &q, &u, &mut proposed, &mut fault, &mut b)
                .is_err()
        );
        assert!(fault.fired, "durable position {committed}");

        // ACKs are inert claims. At positions 3/4 this is the actual persisted
        // Worker identity, but the issuer has not committed its advance yet.
        let identity = if committed >= 3 {
            let wire = std::fs::read(dir.path().join(WORKER.canonical)).unwrap();
            wire[wire.len() - 32..].try_into().unwrap()
        } else {
            [0x55; 32]
        };
        let claim = retain(Ack::new(&u, identity, &mut b).unwrap(), &mut b).unwrap();
        assert!(gate_without_mutation(&ledger, &u, &claim, dir.path(), &mut b).is_err());
        drop(ledger);
        let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        ledger.validate(&mut b).unwrap();
        assert_eq!(ledger.record.sequence, if committed == 5 { 2 } else { 1 });
        assert_eq!(ledger.worker.is_some(), committed >= 3);
        assert_eq!(ledger.record.last_ack.is_some(), committed == 5);
        assert_eq!(matches!(ledger.record.body, Body::Ready), committed == 5);

        let inert = inert_carriage(&p, &q, &u, &claim, &mut b);
        let result = gate_without_mutation(
            &ledger,
            inert.publication(),
            inert.acknowledgment(),
            dir.path(),
            &mut b,
        );
        if committed < 5 {
            let error = result
                .err()
                .expect("valid carriage bytes cannot complete publication");
            assert_eq!(error.resource(), None, "durable position {committed}");
        } else {
            let carriage = result.unwrap();
            assert_eq!(carriage.canonical_bytes(), inert.canonical_bytes());
        }

        let (ack, advanced) = ledger
            .publish_fixture(
                &p,
                &key,
                &q,
                &u,
                &mut |challenge, b| {
                    assert!(committed < 2, "committed anchor must not exchange again");
                    proposed(challenge, b)
                },
                &mut b,
            )
            .unwrap();
        assert_eq!(advanced, committed < 5);
        b.reserve_storage(ack.retained_storage()).unwrap();
        let first = completed_carriage(&ledger, &u, &ack, dir.path(), &mut b);
        if committed >= 3 {
            assert_eq!(first.as_slice(), inert.canonical_bytes());
        }
        // Discard the first response and recover the signed journals. Replaying
        // the predicate is read-only and does not create a live occurrence.
        drop(ledger);
        let ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        assert_eq!(
            completed_carriage(&ledger, &u, &ack, dir.path(), &mut b),
            first
        );
        assert_eq!(
            completed_carriage(&ledger, &u, &ack, dir.path(), &mut b),
            first
        );
        assert_eq!(b.failed_work(), None);
        assert_eq!(b.failed_storage(), None);
    }
}

#[test]
fn retirement_rejects_altered_ack_publication_and_ready_without_completed_worker() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    let (ack, _) = ledger
        .publish_fixture(&p, &key, &q, &u, &mut proposed, &mut b)
        .unwrap();
    b.reserve_storage(ack.retained_storage()).unwrap();
    let expected = completed_carriage(&ledger, &u, &ack, dir.path(), &mut b);

    let mut wrong_worker = ack.worker_ledger_record_identity();
    wrong_worker[0] ^= 1;
    let altered_ack = retain(Ack::new(&u, wrong_worker, &mut b).unwrap(), &mut b).unwrap();
    altered_ack.matches_publication(&u, &mut b).unwrap();
    assert_ne!(altered_ack.canonical_bytes(), ack.canonical_bytes());
    assert!(gate_without_mutation(&ledger, &u, &altered_ack, dir.path(), &mut b).is_err());
    for change_occurrence in [false, true] {
        let mut journal = u.issuer_journal_identity();
        let mut occurrence = u.compiler_occurrence_identity();
        if change_occurrence {
            occurrence[0] ^= 1;
        } else {
            journal[0] ^= 1;
        }
        let receipt = retain(
            Receipt::decode(u.receipt().canonical_bytes(), &mut b).unwrap(),
            &mut b,
        )
        .unwrap();
        let altered = retain(
            Publication::new(journal, occurrence, receipt, &mut b).unwrap(),
            &mut b,
        )
        .unwrap();
        let matching = retain(Ack::new(&altered, wrong_worker, &mut b).unwrap(), &mut b).unwrap();
        matching.matches_publication(&altered, &mut b).unwrap();
        assert_ne!(altered.canonical_bytes(), u.canonical_bytes());
        for candidate in [&ack, &matching] {
            let error = gate_without_mutation(&ledger, &altered, candidate, dir.path(), &mut b)
                .unwrap_err();
            assert_eq!(error.resource(), None);
        }
    }
    assert_eq!(
        completed_carriage(&ledger, &u, &ack, dir.path(), &mut b),
        expected
    );

    let (empty_dir, empty_root) = directory();
    let empty = Ledger::recover(empty_root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    assert!(matches!(empty.record.body, Body::Ready));
    assert!(empty.worker.is_none());
    let error = gate_without_mutation(&empty, &u, &ack, empty_dir.path(), &mut b).unwrap_err();
    assert_eq!(error.resource(), None);
}

#[test]
fn retirement_refuses_old_completed_carriage_while_next_prepared_or_issued_is_pending() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    let (ack, _) = ledger
        .publish_fixture(&p, &key, &q, &u, &mut proposed, &mut b)
        .unwrap();
    b.reserve_storage(ack.retained_storage()).unwrap();
    let completed = completed_carriage(&ledger, &u, &ack, dir.path(), &mut b);
    for issue_next in [false, true] {
        let next = if issue_next {
            ledger.record.fixture_issued(&p, &key, &mut b).unwrap()
        } else {
            let next_subject = subject(2, &mut b);
            ledger
                .record
                .fixture_prepared(next_subject, &p, &key, &mut b)
                .unwrap()
        };
        b.reserve_storage(Record::STORAGE).unwrap();
        ledger.commit(next, &mut b).unwrap();
        drop(ledger);
        ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
        assert!(matches!(
            (&ledger.record.body, issue_next),
            (Body::Prepared { .. }, false) | (Body::Issued { .. }, true)
        ));
        let recovered = ledger
            .recover_carriage(q.subject(), &mut b)
            .unwrap()
            .unwrap();
        assert_eq!(recovered.canonical_bytes().as_slice(), completed.as_slice());
        let charge = recovered.retained_storage();
        b.reserve_storage(charge).unwrap();
        let error = gate_without_mutation(
            &ledger,
            recovered.publication(),
            recovered.acknowledgment(),
            dir.path(),
            &mut b,
        )
        .unwrap_err();
        assert_eq!(error.resource(), None);
        drop(recovered);
        b.release_storage(charge).unwrap();
        ledger.validate(&mut b).unwrap();
    }
}

#[test]
fn retirement_revalidates_durable_worker_instead_of_trusting_retained_carriage() {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut b);
    let (ack, _) = ledger
        .publish_fixture(&p, &key, &q, &u, &mut proposed, &mut b)
        .unwrap();
    b.reserve_storage(ack.retained_storage()).unwrap();
    completed_carriage(&ledger, &u, &ack, dir.path(), &mut b);
    let path = dir.path().join(WORKER.canonical);
    let mut changed = std::fs::read(&path).unwrap();
    changed[24] ^= 1;
    std::fs::write(path, changed).unwrap();
    let error = gate_without_mutation(&ledger, &u, &ack, dir.path(), &mut b).unwrap_err();
    assert_eq!(error.resource(), None);
}

#[test]
fn retirement_exact_quota_and_work_or_storage_denials_preserve_durable_state() {
    let mut fixture_work = Work::new(WORK_LIMIT);
    let mut fixture_budget = Budget::new(&mut fixture_work, STORAGE_LIMIT);
    fixture_budget.reserve_storage(65536).unwrap();
    let (p, key) = custody(&mut fixture_budget);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut fixture_budget).unwrap();
    fixture_budget.reserve_storage(Ledger::STORAGE).unwrap();
    let (q, u) = issued(&mut ledger, &p, &key, 1, &mut fixture_budget);
    let (ack, _) = ledger
        .publish_fixture(&p, &key, &q, &u, &mut proposed, &mut fixture_budget)
        .unwrap();
    fixture_budget
        .reserve_storage(ack.retained_storage())
        .unwrap();

    const PREFIX: usize = 19;
    let floor = Ledger::STORAGE + u.retained_storage() + ack.retained_storage() + 23;
    // Each independent fixture invocation borrows its supplied meter. Measure
    // the full nested gate, including both durable validations, without a quota
    // formula duplicated from the implementation.
    let (exact_work, peak) = {
        let mut work = Work::new(WORK_LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(floor).unwrap();
        b.charge_work(PREFIX).unwrap();
        let carriage = gate_without_mutation(&ledger, &u, &ack, dir.path(), &mut b).unwrap();
        assert_eq!(
            carriage.acknowledgment().canonical_bytes(),
            ack.canonical_bytes()
        );
        drop(carriage);
        (b.work(), b.peak_storage())
    };
    assert!(exact_work > PREFIX && peak > floor);
    for (work_limit, storage_limit, expected) in [
        (exact_work, peak, "exact"),
        (exact_work - 1, peak, "work"),
        (WORK_LIMIT, peak - 1, "storage"),
    ] {
        let mut work = Work::new(work_limit);
        let mut b = Budget::new(&mut work, storage_limit);
        b.reserve_storage(floor).unwrap();
        b.charge_work(PREFIX).unwrap();
        let result = gate_without_mutation(&ledger, &u, &ack, dir.path(), &mut b);
        match expected {
            "exact" => {
                let carriage = result.unwrap();
                assert_eq!(
                    carriage.publication().canonical_bytes(),
                    u.canonical_bytes()
                );
                assert_eq!(b.work(), exact_work);
                assert_eq!(b.peak_storage(), peak);
                assert_eq!(b.failed_work(), None);
                assert_eq!(b.failed_storage(), None);
            }
            "work" | "storage" => {
                let error = result.unwrap_err();
                if expected == "work" {
                    assert!(matches!(error.resource(), Some(Resource::Work(_))));
                    assert_eq!(b.failed_work(), Some(exact_work));
                } else {
                    assert!(matches!(error.resource(), Some(Resource::Storage(_))));
                    assert_eq!(b.failed_storage(), Some(peak));
                }
                let failures = (b.failed_work(), b.failed_storage());
                let accepted_work = b.work();
                assert!(gate_without_mutation(&ledger, &u, &ack, dir.path(), &mut b).is_err());
                assert!(b.work() >= accepted_work);
                assert_eq!((b.failed_work(), b.failed_storage()), failures);
            }
            _ => unreachable!(),
        }
        assert_eq!(b.storage(), floor);
    }
}

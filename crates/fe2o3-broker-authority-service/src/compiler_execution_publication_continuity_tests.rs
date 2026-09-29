//! Shared native issuer V2/V3 tests of the actual durable Ledger consumer.
//! These are bounded fixtures, not actual Admission/NativeOccurrence coverage,
//! production qualification, or GPU qualification.
use super::*;
use fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalStageV1 as Stage;
use std::cell::Cell;

// Logical work units, not allocated bytes; each fresh fixture gets its own cap.
const WORK_LIMIT: usize = 64 * 1024 * 1024 * 1024;
const REFUSAL: &str = "fixture publication continuity refused";
const MIN_CHECKPOINTS: usize = 12;
const MAX_CHECKPOINTS: usize = 32;

fn assert_checkpoint_coverage(phases: &[(usize, usize)]) {
    assert!((MIN_CHECKPOINTS..=MAX_CHECKPOINTS).contains(&phases.len()));
    assert!(
        phases.starts_with(&[(0, 0), (0, 0)]),
        "entry/pre-prepare checks"
    );
    // Each pair brackets a named boundary with checks on both sides. Repeated
    // checks at the same position are allowed; every runtime check is swept.
    for (name, before, after) in [
        ("PreparedAnchor commit", (0, 0), (1, 0)),
        ("anchor exchange", (1, 0), (1, 1)),
        ("AnchorCommitted commit", (1, 1), (2, 1)),
        ("Worker commit", (2, 1), (3, 1)),
        ("Published commit", (3, 1), (4, 1)),
        ("issuer commit", (4, 1), (5, 1)),
    ] {
        assert!(
            phases.windows(2).any(|pair| pair == [before, after]),
            "missing checks bracketing {name}: {phases:?}"
        );
    }
    assert!(
        phases.ends_with(&[(5, 1), (5, 1)]),
        "final validation checks"
    );
}

struct PublicationCommits<'a> {
    completed: &'a Cell<usize>,
    refused: &'a Cell<bool>,
}
impl RetainedDurableDirectoryHooksV1 for PublicationCommits<'_> {
    fn record(&mut self, boundary: Boundary, timing: Timing) -> std::io::Result<()> {
        assert!(!self.refused.get(), "durable I/O after continuity refusal");
        if (boundary, timing) == (Boundary::SyncCanonicalName, Timing::After) {
            self.completed.set(self.completed.get() + 1);
            assert!(self.completed.get() <= 5);
        }
        Ok(())
    }
}

fn durable_bytes(root: &std::path::Path) -> Vec<(std::ffi::OsString, Vec<u8>)> {
    // Recovery stabilizes canonical names via rename, so ctime can change.
    snapshot(root)
        .into_iter()
        .map(|(name, bytes, ..)| (name, bytes))
        .collect()
}

fn assert_publication_position(ledger: &Ledger, commits: usize) {
    assert!(commits <= 5);
    assert_eq!(ledger.record.sequence, if commits == 5 { 2 } else { 1 });
    assert_eq!(ledger.worker.is_some(), commits >= 3);
    assert_eq!(ledger.record.last_ack.is_some(), commits == 5);
    if commits == 5 {
        assert!(matches!(ledger.record.body, Body::Ready));
    } else {
        assert!(matches!(ledger.record.body, Body::Issued { .. }));
    }
    let expected_stage = match commits {
        0 => None,
        1 => Some(Stage::PreparedAnchor),
        2 | 3 => Some(Stage::AnchorCommitted),
        4 | 5 => Some(Stage::Published),
        _ => unreachable!(),
    };
    assert_eq!(ledger.anchor.as_ref().map(|a| a.stage()), expected_stage);
}

fn publication_continuity_case(refuse_at: Option<usize>) -> Vec<(usize, usize)> {
    let mut work = Work::new(WORK_LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    b.reserve_storage(65536).unwrap();
    let original_meter = b.work_ledger_identity_v1();
    let (p, key) = custody(&mut b);
    let (dir, root) = directory();
    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    b.reserve_storage(Ledger::STORAGE).unwrap();
    let (request, publication) = issued(&mut ledger, &p, &key, 1, &mut b);
    let initial = snapshot(dir.path());
    let storage_floor = b.storage();
    let work_before = b.work();
    let completed = Cell::new(0);
    let exchanges = Cell::new(0);
    let refused = Cell::new(false);
    let mut hooks = PublicationCommits {
        completed: &completed,
        refused: &refused,
    };
    let mut phases = Vec::new();
    let mut last_check_work = work_before;
    let mut refusal_snapshot = None;
    let mut exchanged_challenge = None;
    let result = ledger.publish_with_continuity_checks_fixture(
        &p,
        &key,
        &request,
        &publication,
        &mut |challenge, b| {
            assert!(!refused.get(), "anchor exchange after continuity refusal");
            assert!(b.work_ledger_identity_v1() == original_meter);
            assert_eq!(exchanges.get(), 0, "only one fresh anchor exchange");
            assert!(dir.path().join(ANCHOR.canonical).exists());
            exchanged_challenge = Some(challenge.clone());
            let receipt = proposed(challenge, b)?;
            exchanges.set(exchanges.get() + 1);
            Ok(receipt)
        },
        &mut |b| {
            assert!(!refused.get(), "checkpoint after continuity refusal");
            assert!(b.work_ledger_identity_v1() == original_meter);
            assert_eq!(b.storage_limit(), STORAGE_LIMIT);
            // Make each checker consume the original meter and scratch frame.
            b.charge_work(1)?;
            b.reserve_storage(1)?;
            assert!(b.work() > last_check_work);
            last_check_work = b.work();
            phases.push((completed.get(), exchanges.get()));
            assert!(phases.len() <= MAX_CHECKPOINTS);
            if refuse_at == Some(phases.len()) {
                refusal_snapshot = Some(snapshot(dir.path()));
                refused.set(true);
                return Err(Error::rejected(REFUSAL));
            }
            Ok(())
        },
        &mut hooks,
        &mut b,
    );
    assert_eq!(b.storage(), storage_floor, "case {refuse_at:?}");
    assert!(b.work_ledger_identity_v1() == original_meter);
    assert_eq!(b.storage_limit(), STORAGE_LIMIT);
    assert!(b.work() >= last_check_work && b.work() > work_before);
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
    let first_ack = if let Some(checkpoint) = refuse_at {
        assert!(refused.get(), "checkpoint {checkpoint} was not reached");
        assert_eq!(phases.len(), checkpoint);
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("checkpoint {checkpoint} released a successful ACK"),
        };
        assert_eq!(error.to_string(), Error::rejected(REFUSAL).to_string());
        assert_eq!(snapshot(dir.path()), refusal_snapshot.unwrap());
        None
    } else {
        assert!(!refused.get());
        let (ack, advanced) = result.unwrap();
        assert!(advanced);
        Some(ack.canonical_bytes().to_vec())
    };
    let commits = completed.get();
    assert_publication_position(&ledger, commits);
    if commits == 0 {
        assert_eq!(snapshot(dir.path()), initial);
    }
    let persisted_challenge = ledger.anchor.as_ref().map(|a| a.challenge().clone());
    if let Some(challenge) = &exchanged_challenge {
        assert_eq!(persisted_challenge.as_ref(), Some(challenge));
    }
    let refused_position = durable_bytes(dir.path());
    let issuer_bytes = *ledger.record.bytes();
    drop(ledger);

    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    assert_eq!(b.storage(), storage_floor);
    assert_eq!(durable_bytes(dir.path()), refused_position);
    assert_eq!(ledger.record.bytes(), &issuer_bytes);
    assert_publication_position(&ledger, commits);
    ledger.validate(&mut b).unwrap();
    let mut retry_exchanges = 0;
    let (ack, advanced) = ledger
        .publish_fixture(
            &p,
            &key,
            &request,
            &publication,
            &mut |challenge, b| {
                assert!(commits < 2, "committed anchor must not exchange again");
                assert!(b.work_ledger_identity_v1() == original_meter);
                assert_eq!(retry_exchanges, 0);
                if let Some(persisted) = &persisted_challenge {
                    assert_eq!(challenge, persisted, "retry changed durable challenge");
                }
                retry_exchanges += 1;
                proposed(challenge, b)
            },
            &mut b,
        )
        .unwrap();
    assert_eq!(retry_exchanges, usize::from(commits < 2));
    assert_eq!(advanced, commits < 5);
    assert_eq!(b.storage(), storage_floor);
    assert_publication_position(&ledger, 5);
    assert_eq!(
        ledger.record.last_ack.as_ref().unwrap().canonical_bytes(),
        ack.canonical_bytes()
    );
    if let Some(first_ack) = first_ack {
        assert_eq!(ack.canonical_bytes().as_slice(), first_ack.as_slice());
    }
    b.reserve_storage(ack.retained_storage()).unwrap();
    let replay_floor = b.storage();
    let committed_position = durable_bytes(dir.path());
    drop(ledger);

    let mut ledger = Ledger::recover(root.as_fd(), &p, &key, &mut b).unwrap();
    assert_eq!(b.storage(), replay_floor);
    assert_eq!(durable_bytes(dir.path()), committed_position);
    assert_publication_position(&ledger, 5);
    let before_replay = snapshot(dir.path());
    let replay_work = b.work();
    let (replay, advanced) = ledger
        .publish_fixture(
            &p,
            &key,
            &request,
            &publication,
            &mut |_, _| panic!("ACK replay must not exchange"),
            &mut b,
        )
        .unwrap();
    assert!(!advanced);
    assert_eq!(replay.canonical_bytes(), ack.canonical_bytes());
    assert_eq!(snapshot(dir.path()), before_replay);
    assert_eq!(b.storage(), replay_floor);
    assert!(b.work_ledger_identity_v1() == original_meter);
    assert!(b.work() > replay_work && b.work() <= WORK_LIMIT);
    assert!(b.peak_storage() <= STORAGE_LIMIT);
    assert_eq!(b.failed_work(), None);
    assert_eq!(b.failed_storage(), None);
    phases
}

#[test]
fn native_worker_continuity_refusal_at_every_publication_checkpoint_recovers_exactly_once() {
    let successful_phases = publication_continuity_case(None);
    assert_checkpoint_coverage(&successful_phases);
    for checkpoint in 1..=successful_phases.len() {
        let phases = publication_continuity_case(Some(checkpoint));
        assert_eq!(phases, successful_phases[..checkpoint]);
    }
}

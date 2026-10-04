//! Inert ordering/accounting tests only, without source or harness authority.
use super::*;
use ed25519_dalek::SigningKey;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3 as CODEC_FRAME,
    COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3 as CODEC_WORK,
    CompilerExecutionClientProcessIdentityV1 as Client,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerMeasurementV1 as Measurement, CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionRootControlKindV3 as Kind,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
};
use sha2::{Digest, Sha256};
use std::io::Write;

type Window<'work> = RootControlReplayWindowV3<'work>;
const LIMIT: usize = 200_000_000;
const STORAGE_LIMIT: usize = 2_000_000;
const RECORD_STORAGE: usize = size_of::<(Record, ProtocolStorage)>();
const KINDS: [Kind; 4] = [Kind::Reconcile, Kind::Observe, Kind::Validate, Kind::Retire];

fn retain<T>(
    result: std::result::Result<(T, ProtocolStorage), ProtocolError>,
    b: &mut Budget<'_>,
) -> T {
    let (value, charge) = result.unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    value
}

fn binding(b: &mut Budget<'_>) -> Binding {
    let (policy, charge) = Policy::new(
        7,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        SigningKey::from_bytes(&[0x51; 32])
            .verifying_key()
            .to_bytes(),
        SigningKey::from_bytes(&[0x52; 32])
            .verifying_key()
            .to_bytes(),
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let (manifest, charge) = Manifest::new(
        Client::new(1234, 5678, 9012).unwrap(),
        Service::new(6001, 7001).unwrap(),
        &policy,
        b,
    )
    .unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    let result = retain(
        Binding::new(&policy, &manifest, [0x71; 32], [0x72; 32], b),
        b,
    );
    let released = policy.retained_storage() + manifest.retained_storage();
    drop((policy, manifest));
    b.release_storage(released).unwrap();
    result
}

fn window<'work>(b: &mut Budget<'work>) -> Window<'work> {
    let binding = binding(b);
    let consumed = binding.retained_storage();
    let floor = b.storage();
    let prefix = b.work();
    let (window, charge) = Window::new(binding, b).unwrap();
    assert_eq!((b.storage(), b.work()), (floor, prefix + WORK));
    assert_eq!(charge.additional_storage(), Window::STORAGE);
    assert!(Window::STORAGE >= consumed + 2 * RECORD_STORAGE + size_of::<Window<'_>>());
    b.release_storage(consumed).unwrap();
    b.reserve_storage(charge.additional_storage()).unwrap();
    window
}

fn request(w: &Window<'_>, seq: u64, kind: Kind, payload: &[u8], b: &mut Budget<'_>) -> Record {
    b.reserve_storage(payload.len()).unwrap();
    let result = retain(Record::request(&w.binding, seq, kind, payload, b), b);
    b.release_storage(payload.len()).unwrap();
    result
}

fn reply(request: &Record, b: &mut Budget<'_>) -> Record {
    retain(Record::reply(request, &[], b), b)
}

fn decode(bytes: &[u8; RECORD_BYTES], b: &mut Budget<'_>) -> Record {
    b.reserve_storage(RECORD_BYTES).unwrap();
    let result = retain(Record::decode(bytes, b), b);
    b.release_storage(RECORD_BYTES).unwrap();
    result
}

fn changed_association(record: &Record, offset: usize, b: &mut Budget<'_>) -> Record {
    let mut bytes = *record.canonical_bytes();
    bytes[offset] ^= 1;
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/ROOT-ISSUER-CONTROL/V3\0");
    hash.update(&bytes[..RECORD_BYTES - 32]);
    bytes[RECORD_BYTES - 32..].copy_from_slice(&hash.finalize());
    decode(&bytes, b)
}

fn snapshot(w: &Window<'_>) -> (u8, Option<[u8; RECORD_BYTES]>, Option<[u8; RECORD_BYTES]>) {
    match &w.state {
        State::Empty => (0, None, None),
        State::Exchange {
            request,
            reply: None,
        } => (1, Some(*request.canonical_bytes()), None),
        State::Exchange {
            request,
            reply: Some(reply),
        } => (
            2,
            Some(*request.canonical_bytes()),
            Some(*reply.canonical_bytes()),
        ),
    }
}

fn accept(w: &mut Window<'_>, record: Record, b: &mut Budget<'_>) -> Result<Disposition> {
    let before = snapshot(w);
    let floor = b.storage();
    let charge = record.retained_storage();
    let result = w.accept(record, b);
    assert_eq!(b.storage(), floor);
    assert_eq!(w.retained_storage(), Window::STORAGE);
    if !matches!(&result, Ok(Disposition::Accepted)) {
        assert_eq!(snapshot(w), before);
    }
    b.release_storage(charge).unwrap();
    result
}

fn complete(w: &mut Window<'_>, record: Record, b: &mut Budget<'_>) -> Result<()> {
    let before = snapshot(w);
    let floor = b.storage();
    let charge = record.retained_storage();
    let result = w.complete(record, b);
    assert_eq!(b.storage(), floor);
    assert_eq!(w.retained_storage(), Window::STORAGE);
    if result.is_err() {
        assert_eq!(snapshot(w), before);
    }
    b.release_storage(charge).unwrap();
    result
}

fn refuse<T>(result: Result<T>) {
    assert!(matches!(result, Err(Error::Refused(_))));
}

fn prepare_and_drop(w: &mut Window<'_>, record: Record, b: &mut Budget<'_>) -> Result<()> {
    let before = snapshot(w);
    let floor = b.storage();
    let charge = record.retained_storage();
    let result = w.prepare_complete(record, b).map(drop);
    assert_eq!(snapshot(w), before);
    assert_eq!(b.storage(), floor);
    assert_eq!(w.retained_storage(), Window::STORAGE);
    b.release_storage(charge).unwrap();
    result
}

#[test]
fn prepared_reply_drop_preserves_pending_and_commit_installs_exact_replay() {
    for kind in KINDS {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(23).unwrap();
        let mut w = window(&mut b);
        let first = request(&w, 1, kind, b"opaque", &mut b);
        let first_bytes = *first.canonical_bytes();
        let response = reply(&first, &mut b);
        let response_bytes = *response.canonical_bytes();
        accept(&mut w, first, &mut b).unwrap();
        assert!(b.charge_work(LIMIT + 1).is_err());
        assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let prefix = b.work();
        prepare_and_drop(&mut w, response, &mut b).unwrap();
        assert_eq!(b.work(), prefix + Window::WORK);
        assert_eq!(b.storage(), 23 + Window::STORAGE);
        let duplicate = decode(&first_bytes, &mut b);
        assert_eq!(
            accept(&mut w, duplicate, &mut b).unwrap(),
            Disposition::Pending
        );

        let response = decode(&response_bytes, &mut b);
        let floor = b.storage();
        let prefix = b.work();
        let prepared = w.prepare_complete(response, &mut b).unwrap();
        assert_eq!((b.storage(), b.work()), (floor, prefix + Window::WORK));
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        let peak = b.peak_storage();
        prepared.commit();
        assert_eq!((b.storage(), b.work()), (floor, prefix + Window::WORK));
        assert_eq!(b.peak_storage(), peak);
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        b.release_storage(RECORD_STORAGE).unwrap();
        let cached = snapshot(&w);
        assert_eq!(cached, (2, Some(first_bytes), Some(response_bytes)));
        for _ in 0..2 {
            let duplicate = decode(&first_bytes, &mut b);
            assert_eq!(
                accept(&mut w, duplicate, &mut b).unwrap(),
                Disposition::Replay
            );
            assert_eq!(snapshot(&w), cached);
        }
        let response = decode(&response_bytes, &mut b);
        refuse(prepare_and_drop(&mut w, response, &mut b));
        assert_eq!(b.storage(), 23 + Window::STORAGE);
    }
}

#[test]
fn prepared_reply_refuses_nonpending_direction_and_every_request_association() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut w = window(&mut b);
    let first = request(&w, 1, Kind::Observe, b"one", &mut b);
    let bytes = *first.canonical_bytes();
    let valid = reply(&first, &mut b);
    let premature = reply(&first, &mut b);
    refuse(prepare_and_drop(&mut w, premature, &mut b));
    accept(&mut w, first, &mut b).unwrap();
    let wrong_direction = decode(&bytes, &mut b);
    refuse(prepare_and_drop(&mut w, wrong_direction, &mut b));
    for (seq, kind, payload) in [
        (1, Kind::Observe, b"two"),
        (1, Kind::Validate, b"one"),
        (2, Kind::Observe, b"one"),
    ] {
        let wrong_request = request(&w, seq, kind, payload, &mut b);
        let wrong_reply = reply(&wrong_request, &mut b);
        refuse(prepare_and_drop(&mut w, wrong_reply, &mut b));
        drop(wrong_request);
        b.release_storage(RECORD_STORAGE).unwrap();
    }
    for offset in [24, 56, 88, 120, 160] {
        let wrong = changed_association(&valid, offset, &mut b);
        refuse(prepare_and_drop(&mut w, wrong, &mut b));
    }
    prepare_and_drop(&mut w, valid, &mut b).unwrap();
    assert_eq!(snapshot(&w), (1, Some(bytes), None));
}

#[test]
fn enclosing_scope_failure_or_unwind_drops_prepared_reply_without_mutation() {
    for unwind in [false, true] {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        let mut w = window(&mut b);
        let first = request(&w, 1, Kind::Retire, &[], &mut b);
        let response = reply(&first, &mut b);
        accept(&mut w, first, &mut b).unwrap();
        let before = snapshot(&w);
        assert!(b.charge_work(LIMIT + 1).is_err());
        assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
        let history = (b.failed_work(), b.failed_storage());
        let floor = b.storage();
        let prefix = b.work();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<()> {
            let prepared = b.with_prepaid_scope(floor, ENTRY, ENTRY, 53, |b| {
                let prepared = w.prepare_complete(response, b)?;
                if unwind {
                    panic!("simulated caller unwind with an uncommitted inert reply");
                }
                // Fail the enclosing scope's final frame check after preparation.
                b.release_storage(1)?;
                Ok::<_, Error>(prepared)
            })?;
            prepared.commit();
            Ok(())
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            assert!(matches!(
                outcome,
                Ok(Err(Error::Resource(Resource::Accounting)))
            ));
        }
        assert_eq!(snapshot(&w), before);
        assert_eq!(
            (b.storage(), b.work()),
            (floor, prefix + ENTRY + Window::WORK)
        );
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        b.release_storage(RECORD_STORAGE).unwrap();
    }
}

#[test]
fn pending_duplicates_never_restart_and_cached_replies_survive_send_failure() {
    for kind in KINDS {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(23).unwrap();
        let mut w = window(&mut b);
        let fixed = 23 + Window::STORAGE;
        assert!(w.pending(&mut b).unwrap().is_none());
        assert!(w.request(&mut b).unwrap().is_none());
        assert!(w.reply(&mut b).unwrap().is_none());
        let first = request(&w, 1, kind, b"opaque", &mut b);
        let first_bytes = *first.canonical_bytes();
        let response = reply(&first, &mut b);
        let response_bytes = *response.canonical_bytes();
        assert_eq!(
            accept(&mut w, first, &mut b).unwrap(),
            Disposition::Accepted
        );
        for _ in 0..2 {
            let duplicate = decode(&first_bytes, &mut b);
            assert_eq!(
                accept(&mut w, duplicate, &mut b).unwrap(),
                Disposition::Pending
            );
        }
        assert_eq!(
            w.pending(&mut b).unwrap().unwrap().canonical_bytes(),
            &first_bytes
        );
        let next = request(&w, 2, kind, &[], &mut b);
        refuse(accept(&mut w, next, &mut b));
        complete(&mut w, response, &mut b).unwrap();
        assert_eq!(b.storage(), fixed);
        assert!(w.pending(&mut b).unwrap().is_none());
        assert_eq!(
            w.request(&mut b).unwrap().unwrap().canonical_bytes(),
            &first_bytes
        );

        let cached = snapshot(&w);
        let response = w.reply(&mut b).unwrap().unwrap();
        let mut failed_send = &mut [0u8; 0][..];
        assert!(failed_send.write_all(response.canonical_bytes()).is_err());
        assert_eq!(snapshot(&w), cached);
        for _ in 0..2 {
            let duplicate = decode(&first_bytes, &mut b);
            assert_eq!(
                accept(&mut w, duplicate, &mut b).unwrap(),
                Disposition::Replay
            );
            let mut sent = [0; RECORD_BYTES];
            sent.as_mut_slice()
                .write_all(w.reply(&mut b).unwrap().unwrap().canonical_bytes())
                .unwrap();
            assert_eq!(sent, response_bytes);
            assert_eq!(snapshot(&w), cached);
        }
        let duplicate_reply = decode(&response_bytes, &mut b);
        refuse(complete(&mut w, duplicate_reply, &mut b));
        let skipped = request(&w, 3, kind, &[], &mut b);
        refuse(accept(&mut w, skipped, &mut b));
        let next = request(&w, 2, kind, &[], &mut b);
        let next_reply = reply(&next, &mut b);
        assert_eq!(accept(&mut w, next, &mut b).unwrap(), Disposition::Accepted);
        assert!(w.reply(&mut b).unwrap().is_none());
        let stale = decode(&first_bytes, &mut b);
        refuse(accept(&mut w, stale, &mut b));
        complete(&mut w, next_reply, &mut b).unwrap();
        let stale = decode(&first_bytes, &mut b);
        refuse(accept(&mut w, stale, &mut b));
        let next = request(&w, 3, kind, &[], &mut b);
        assert_eq!(accept(&mut w, next, &mut b).unwrap(), Disposition::Accepted);
        assert_eq!(b.storage(), fixed);
        drop(w);
        b.release_storage(Window::STORAGE).unwrap();
        assert_eq!(b.storage(), 23);
    }
}

#[test]
fn request_direction_sequence_content_kind_and_all_binding_axes_refuse_unchanged() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut w = window(&mut b);
    for stage in 0..3 {
        let seq = if stage == 2 { 2 } else { 1 };
        let baseline = request(&w, seq, Kind::Observe, b"one", &mut b);
        for offset in [24, 56, 88, 120] {
            let wrong = changed_association(&baseline, offset, &mut b);
            refuse(accept(&mut w, wrong, &mut b));
        }
        let wrong = reply(&baseline, &mut b);
        refuse(accept(&mut w, wrong, &mut b));
        for seq in [if stage == 2 { 3 } else { 2 }, u64::MAX] {
            let wrong = request(&w, seq, Kind::Observe, b"one", &mut b);
            refuse(accept(&mut w, wrong, &mut b));
        }
        if stage > 0 {
            for (kind, payload) in [(Kind::Observe, b"two"), (Kind::Validate, b"one")] {
                let wrong = request(&w, 1, kind, payload, &mut b);
                refuse(accept(&mut w, wrong, &mut b));
            }
        }
        if stage == 0 {
            let premature = reply(&baseline, &mut b);
            refuse(complete(&mut w, premature, &mut b));
            assert_eq!(
                accept(&mut w, baseline, &mut b).unwrap(),
                Disposition::Accepted
            );
        } else {
            if stage == 1 {
                let response = reply(&baseline, &mut b);
                complete(&mut w, response, &mut b).unwrap();
            }
            drop(baseline);
            b.release_storage(RECORD_STORAGE).unwrap();
        }
    }
}

#[test]
fn complete_requires_the_exact_pending_request_digest_before_mutation() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut w = window(&mut b);
    let first = request(&w, 1, Kind::Observe, b"one", &mut b);
    let bytes = *first.canonical_bytes();
    let valid = reply(&first, &mut b);
    accept(&mut w, first, &mut b).unwrap();
    let wrong_direction = decode(&bytes, &mut b);
    refuse(complete(&mut w, wrong_direction, &mut b));
    for (seq, kind, payload) in [
        (1, Kind::Observe, b"two"),
        (1, Kind::Validate, b"one"),
        (2, Kind::Observe, b"one"),
    ] {
        let wrong_request = request(&w, seq, kind, payload, &mut b);
        let wrong_reply = reply(&wrong_request, &mut b);
        refuse(complete(&mut w, wrong_reply, &mut b));
        drop(wrong_request);
        b.release_storage(RECORD_STORAGE).unwrap();
    }
    for offset in [24, 56, 88, 120, 160] {
        let wrong = changed_association(&valid, offset, &mut b);
        refuse(complete(&mut w, wrong, &mut b));
    }
    complete(&mut w, valid, &mut b).unwrap();
}

#[test]
fn sequence_exhaustion_fault_injection_is_inert_and_preserves_final_replay() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut w = window(&mut b);
    let request = request(&w, u64::MAX - 1, Kind::Retire, &[], &mut b);
    let response = reply(&request, &mut b);
    // Test-only inert state placement, not retirement or recovery evidence.
    w.state = State::Exchange {
        request,
        reply: Some(response),
    };
    b.release_storage(2 * RECORD_STORAGE).unwrap();
    let last = self::request(&w, u64::MAX, Kind::Observe, &[], &mut b);
    let last_bytes = *last.canonical_bytes();
    let response = reply(&last, &mut b);
    assert_eq!(accept(&mut w, last, &mut b).unwrap(), Disposition::Accepted);
    complete(&mut w, response, &mut b).unwrap();
    let duplicate = decode(&last_bytes, &mut b);
    assert_eq!(
        accept(&mut w, duplicate, &mut b).unwrap(),
        Disposition::Replay
    );
    for seq in [1, u64::MAX - 1, u64::MAX] {
        let wrong = self::request(&w, seq, Kind::Observe, b"different", &mut b);
        assert!(matches!(
            accept(&mut w, wrong, &mut b),
            Err(Error::Refused("root control sequence exhausted"))
        ));
    }
}

#[test]
fn constructor_returns_full_unreserved_charge_and_preserves_resource_history() {
    for (remaining_work, remaining_scratch, spent, success) in [
        (ENTRY - 1, Window::SCRATCH, 0, false),
        (WORK - 1, Window::SCRATCH, ENTRY, false),
        (Window::WORK, FRAME - 1, WORK, false),
        (Window::WORK, Window::SCRATCH, WORK, true),
    ] {
        let mut work = Work::new(LIMIT);
        let mut b = Budget::new(&mut work, STORAGE_LIMIT);
        b.reserve_storage(23).unwrap();
        let binding = binding(&mut b);
        assert!(b.charge_work(LIMIT + 1).is_err());
        assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
        let history = (b.failed_work(), b.failed_storage());
        b.charge_work(LIMIT - b.work() - remaining_work).unwrap();
        b.reserve_storage(STORAGE_LIMIT - b.storage() - remaining_scratch)
            .unwrap();
        let floor = b.storage();
        let prefix = b.work();
        let result = Window::new(binding, &mut b);
        assert_eq!((b.storage(), b.work()), (floor, prefix + spent));
        assert_eq!((b.failed_work(), b.failed_storage()), history);
        match result {
            Ok((w, charge)) => {
                assert!(success);
                assert_eq!(charge.additional_storage(), Window::STORAGE);
                assert_eq!(w.retained_storage(), Window::STORAGE);
                assert!(matches!(w.state, State::Empty));
            }
            Err(Error::Resource(_)) => assert!(!success),
            Err(e) => panic!("unexpected refusal: {e}"),
        }
    }
}

#[test]
fn exact_quotes_and_one_short_nested_funding_preserve_state_floor_and_history() {
    assert_eq!(Window::WORK, WORK + CODEC_WORK);
    assert_eq!(Window::SCRATCH, FRAME + CODEC_FRAME);
    // Accept, complete, prepare/drop, and prepare/commit share the same quotes.
    for operation in 0..4 {
        for (remaining_work, remaining_scratch, spent, success) in [
            (ENTRY - 1, Window::SCRATCH, 0, false),
            (WORK - 1, Window::SCRATCH, ENTRY, false),
            (WORK + ENTRY - 1, Window::SCRATCH, WORK, false),
            (Window::WORK - 1, Window::SCRATCH, WORK + ENTRY, false),
            (Window::WORK, FRAME - 1, WORK, false),
            (Window::WORK, Window::SCRATCH - 1, Window::WORK, false),
            (Window::WORK, Window::SCRATCH, Window::WORK, true),
        ] {
            let mut work = Work::new(LIMIT);
            let mut b = Budget::new(&mut work, STORAGE_LIMIT);
            b.reserve_storage(23).unwrap();
            let mut w = window(&mut b);
            let first = request(&w, 1, Kind::Observe, &[], &mut b);
            let response = reply(&first, &mut b);
            accept(&mut w, first, &mut b).unwrap();
            let input = if operation != 0 {
                response
            } else {
                complete(&mut w, response, &mut b).unwrap();
                request(&w, 2, Kind::Observe, &[], &mut b)
            };
            assert!(b.charge_work(LIMIT + 1).is_err());
            assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
            let history = (b.failed_work(), b.failed_storage());
            b.charge_work(LIMIT - b.work() - remaining_work).unwrap();
            b.reserve_storage(STORAGE_LIMIT - b.storage() - remaining_scratch)
                .unwrap();
            let floor = b.storage();
            let prefix = b.work();
            let before = snapshot(&w);
            let result = match operation {
                0 => w.accept(input, &mut b).map(|disposition| {
                    assert_eq!(disposition, Disposition::Accepted);
                }),
                1 => w.complete(input, &mut b),
                _ => w.prepare_complete(input, &mut b).map(|prepared| {
                    if operation == 3 {
                        prepared.commit();
                    }
                }),
            };
            if success {
                result.unwrap();
                if operation == 2 {
                    assert_eq!(snapshot(&w), before);
                } else {
                    assert_ne!(snapshot(&w), before);
                }
                assert_eq!(b.peak_storage(), floor + Window::SCRATCH);
            } else {
                match result {
                    Err(Error::Resource(Resource::Work(e))) => {
                        let attempted = if remaining_work < ENTRY {
                            ENTRY
                        } else if remaining_work < WORK {
                            WORK
                        } else if remaining_work < WORK + ENTRY {
                            WORK + ENTRY
                        } else {
                            Window::WORK
                        };
                        assert_eq!((e.actual(), e.limit()), (prefix + attempted, LIMIT));
                    }
                    Err(Error::Resource(Resource::Storage(e))) => {
                        let scratch = if remaining_scratch < FRAME {
                            FRAME
                        } else {
                            Window::SCRATCH
                        };
                        assert_eq!((e.actual(), e.limit()), (floor + scratch, STORAGE_LIMIT));
                    }
                    _ => panic!("expected funding refusal"),
                }
                assert_eq!(snapshot(&w), before);
            }
            assert_eq!((b.storage(), b.work()), (floor, prefix + spent));
            assert_eq!((b.failed_work(), b.failed_storage()), history);
            assert_eq!(w.retained_storage(), Window::STORAGE);
        }
    }
}

#[test]
fn unprepaid_binding_window_and_record_floors_refuse_before_mutation() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let binding = binding(&mut b);
    b.release_storage(1).unwrap();
    let floor = b.storage();
    let prefix = b.work();
    assert!(matches!(
        Window::new(binding, &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!((b.storage(), b.work()), (floor, prefix + ENTRY));
    b.release_storage(floor).unwrap();

    let mut w = window(&mut b);
    let first = request(&w, 1, Kind::Observe, &[], &mut b);
    let response = reply(&first, &mut b);
    let response_bytes = *response.canonical_bytes();
    accept(&mut w, first, &mut b).unwrap();
    b.release_storage(1).unwrap();
    let before = snapshot(&w);
    let floor = b.storage();
    let prefix = b.work();
    assert!(matches!(
        w.complete(response, &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(snapshot(&w), before);
    assert_eq!((b.storage(), b.work()), (floor, prefix + ENTRY));
    b.release_storage(RECORD_STORAGE - 1).unwrap();
    let response = decode(&response_bytes, &mut b);
    b.release_storage(1).unwrap();
    let floor = b.storage();
    let prefix = b.work();
    assert!(matches!(
        w.prepare_complete(response, &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(snapshot(&w), before);
    assert_eq!((b.storage(), b.work()), (floor, prefix + ENTRY));
    b.release_storage(RECORD_STORAGE - 1).unwrap();
    let next = request(&w, 2, Kind::Observe, &[], &mut b);
    b.release_storage(1).unwrap();
    assert!(matches!(
        w.accept(next, &mut b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert_eq!(snapshot(&w), before);
    b.release_storage(RECORD_STORAGE).unwrap();
    let floor = b.storage();
    let prefix = b.work();
    for result in [w.pending(&mut b), w.request(&mut b), w.reply(&mut b)] {
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    }
    assert_eq!(snapshot(&w), before);
    assert_eq!((b.storage(), b.work()), (floor, prefix + 3 * ENTRY));
}

fn accounting_refusal(w: &mut Window<'_>, b: &mut Budget<'_>, bytes: &[u8; RECORD_BYTES]) {
    let first = decode(bytes, b);
    let response = reply(&first, b);
    let prepared_response = reply(&first, b);
    let floor = b.storage();
    let prefix = b.work();
    let before = snapshot(w);
    assert!(matches!(
        accept(w, first, b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        complete(w, response, b),
        Err(Error::Resource(Resource::Accounting))
    ));
    assert!(matches!(
        prepare_and_drop(w, prepared_response, b),
        Err(Error::Resource(Resource::Accounting))
    ));
    for result in [w.pending(b), w.request(b), w.reply(b)] {
        assert!(matches!(result, Err(Error::Resource(Resource::Accounting))));
    }
    assert_eq!(snapshot(w), before);
    assert_eq!(
        (b.storage(), b.work()),
        (floor - 3 * RECORD_STORAGE, prefix + 6 * WORK)
    );
}

#[test]
fn foreign_ledger_original_slot_moved_budget_and_identity_faults_refuse() {
    let mut work = Work::new(LIMIT);
    let mut foreign_work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut foreign = Budget::new(&mut foreign_work, STORAGE_LIMIT);
    foreign.reserve_storage(Window::STORAGE).unwrap();
    let mut w = window(&mut b);
    let first = request(&w, 1, Kind::Observe, &[], &mut b);
    let bytes = *first.canonical_bytes();
    accept(&mut w, first, &mut b).unwrap();
    assert!(b.charge_work(LIMIT + 1).is_err());
    assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
    let history = (b.failed_work(), b.failed_storage());
    let account = b.work_ledger_identity_v1();
    let prefix = b.work();
    accounting_refusal(&mut w, &mut foreign, &bytes);
    assert_eq!(b.work(), prefix);
    std::mem::swap(&mut b, &mut foreign);
    assert!(b.work_ledger_identity_v1() != account);
    accounting_refusal(&mut w, &mut b, &bytes);
    assert!(foreign.work_ledger_identity_v1() == account);
    accounting_refusal(&mut w, &mut foreign, &bytes);
    std::mem::swap(&mut b, &mut foreign);

    // Inert owner-field fault injection only, not post-fork or thread-transfer qualification.
    let pid = w.process;
    w.process = process::Pid::from_raw(if pid.as_raw_nonzero().get() == 1 {
        2
    } else {
        1
    })
    .unwrap();
    accounting_refusal(&mut w, &mut b, &bytes);
    w.process = pid;
    let tid = w.creator;
    w.creator = process::Pid::from_raw(if tid.as_raw_nonzero().get() == 1 {
        2
    } else {
        1
    })
    .unwrap();
    accounting_refusal(&mut w, &mut b, &bytes);
    w.creator = tid;
    let duplicate = decode(&bytes, &mut b);
    assert_eq!(
        accept(&mut w, duplicate, &mut b).unwrap(),
        Disposition::Pending
    );
    assert_eq!((b.failed_work(), b.failed_storage()), history);
}

#[test]
fn caller_unwind_preserves_committed_cache_entry_storage_and_denial_history() {
    let mut work = Work::new(LIMIT);
    let mut b = Budget::new(&mut work, STORAGE_LIMIT);
    let mut w = window(&mut b);
    let first = request(&w, 1, Kind::Retire, &[], &mut b);
    let bytes = *first.canonical_bytes();
    let response = reply(&first, &mut b);
    accept(&mut w, first, &mut b).unwrap();
    assert!(b.charge_work(LIMIT + 1).is_err());
    assert!(b.reserve_storage(STORAGE_LIMIT + 1).is_err());
    let history = (b.failed_work(), b.failed_storage());
    let floor = b.storage();
    let prefix = b.work();
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        b.with_prepaid_scope(floor, ENTRY, ENTRY, 53, |b| -> Result<()> {
            w.complete(response, b)?;
            panic!("simulated sender unwind after inert completion");
        })
    }));
    assert!(panic.is_err());
    assert_eq!(
        (b.storage(), b.work()),
        (floor, prefix + ENTRY + Window::WORK)
    );
    assert_eq!((b.failed_work(), b.failed_storage()), history);
    b.release_storage(RECORD_STORAGE).unwrap();
    let duplicate = decode(&bytes, &mut b);
    assert_eq!(
        accept(&mut w, duplicate, &mut b).unwrap(),
        Disposition::Replay
    );
    assert!(w.reply(&mut b).unwrap().unwrap().is_reply());
}

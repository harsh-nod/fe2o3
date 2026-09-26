//! Guard sequencing only; the pipe-backed accounting client never exchanges.
use super::*;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerMeasurementV1 as Measurement;

fn policy(budget: &mut Budget<'_>) -> Policy {
    let key = |seed| {
        ed25519_dalek::SigningKey::from_bytes(&[seed; 32])
            .verifying_key()
            .to_bytes()
    };
    let (policy, storage) = Policy::new(
        7,
        Measurement::new([0x61; 32], 12345).unwrap(),
        Measurement::new([0x62; 32], 67890).unwrap(),
        key(0x51),
        key(0x52),
        budget,
    )
    .unwrap();
    budget
        .reserve_storage(storage.additional_storage())
        .unwrap();
    policy
}

#[test]
fn completion_checks_preparation_before_publication_on_error_unwind_and_floor_loss() {
    for fault in ["error", "panic", "floor"] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 256 * 1024);
        let policy = policy(&mut budget);
        let before = budget.storage();
        let prefix = budget.work();
        let (mut client, reader) = accounting_client(&mut budget);
        client.deadline += Duration::from_secs(1);
        let published = std::cell::Cell::new(false);
        let result = catch_unwind(AssertUnwindSafe(|| {
            client.prepare_and_acquire::<(), (), (), ClientError>(
                &policy,
                |b| {
                    b.charge_work(7)?;
                    b.reserve_storage(11)?;
                    match fault {
                        "error" => Err(ClientError::Mismatch("preparation refused")),
                        "panic" => panic!("preparation unwind"),
                        _ => {
                            b.release_storage(12)?;
                            Ok(())
                        }
                    }
                },
                |(), _| {
                    published.set(true);
                    panic!("publication must not run")
                },
                |_, (), _| panic!("finish must not run"),
            )
        }));
        match fault {
            "error" => assert!(matches!(
                result,
                Ok(Err(ClientError::Mismatch("preparation refused")))
            )),
            "panic" => assert!(result.is_err()),
            _ => assert!(matches!(
                result,
                Ok(Err(ClientError::Resource(Resource::Accounting)))
            )),
        }
        drop(result);
        assert!(!published.get());
        closed(reader);
        assert_eq!(budget.work(), prefix + 8 + 7);
        let retained = if fault == "floor" {
            TestClient::RETAINED + FLOOR - 1
        } else {
            FLOOR + 11
        };
        assert_eq!(budget.storage(), before + retained);
    }
}

#[test]
fn completion_rejects_foreign_preparation_account_before_publication() {
    let mut work = Work::new(1_000_000);
    let mut other_work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024);
    let mut other = Budget::new(&mut other_work, 256 * 1024);
    other.reserve_storage(64 * 1024).unwrap();
    let policy = policy(&mut budget);
    let before = budget.storage();
    let prefix = budget.work();
    let (client, reader) = accounting_client(&mut budget);
    let mut displaced = None;
    let result = client.prepare_and_acquire::<(), (), (), ClientError>(
        &policy,
        |b| {
            displaced = Some(std::mem::replace(b, other));
            Ok(())
        },
        |(), _| panic!("foreign account cannot publish"),
        |_, (), _| panic!("finish must not run"),
    );
    assert!(matches!(
        result,
        Err(ClientError::Resource(Resource::Accounting))
    ));
    closed(reader);
    assert_eq!(budget.storage(), 64 * 1024);
    assert_eq!(budget.work(), 0);
    let original = displaced.unwrap();
    assert_eq!(original.work(), prefix + 8);
    assert_eq!(original.storage(), before + TestClient::RETAINED + FLOOR);
}

#[test]
fn completion_expired_deadline_stops_before_publication() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024);
    let policy = policy(&mut budget);
    let before = budget.storage();
    let (client, reader) = accounting_client(&mut budget);
    let result = client.prepare_and_acquire::<(), (), (), ClientError>(
        &policy,
        |_| Ok(()),
        |(), _| panic!("expired session cannot publish"),
        |_, (), _| panic!("finish must not run"),
    );
    assert!(matches!(
        result,
        Err(ClientError::Transport(TransportError::Timeout))
    ));
    closed(reader);
    assert_eq!(budget.storage(), before + FLOOR);
}

#[test]
fn completion_publication_refusal_or_unwind_never_acquires_or_finishes() {
    for panic in [false, true] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 256 * 1024);
        let policy = policy(&mut budget);
        let before = budget.storage();
        let prefix = budget.work();
        let (mut client, reader) = accounting_client(&mut budget);
        client.deadline += Duration::from_secs(1);
        let result = catch_unwind(AssertUnwindSafe(|| {
            client.prepare_and_acquire::<(), (), (), ClientError>(
                &policy,
                |_| Ok(()),
                |(), b| {
                    b.charge_work(7)?;
                    b.reserve_storage(11)?;
                    if panic {
                        panic!("publication unwind");
                    }
                    Err(ClientError::Mismatch("publication refused"))
                },
                |_, (), _| panic!("finish must not run"),
            )
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(matches!(
                result,
                Ok(Err(ClientError::Mismatch("publication refused")))
            ));
        }
        drop(result);
        closed(reader);
        assert_eq!(budget.work(), prefix + 16 + 7);
        assert_eq!(budget.storage(), before + FLOOR + 11);
    }
}

#[test]
fn completion_preparation_quota_denial_never_calls_out() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024);
    let policy = policy(&mut budget);
    budget.charge_work(1_000_000 - budget.work()).unwrap();
    let before = budget.storage();
    let (client, reader) = accounting_client(&mut budget);
    let result = client.prepare_and_acquire::<(), (), (), ClientError>(
        &policy,
        |_| panic!("quota precedes preparation"),
        |(), _| panic!("quota precedes publication"),
        |_, (), _| panic!("finish must not run"),
    );
    assert!(matches!(
        result,
        Err(ClientError::Resource(Resource::Work(_)))
    ));
    closed(reader);
    assert_eq!(budget.storage(), before + FLOOR);
    assert_eq!(budget.failed_work(), Some(1_000_000 + 8));
}

#[test]
fn completion_publication_quota_is_checked_after_preparation() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024);
    let policy = policy(&mut budget);
    let before = budget.storage();
    let (client, reader) = accounting_client(&mut budget);
    let result = client.prepare_and_acquire::<(), (), (), ClientError>(
        &policy,
        |b| {
            b.charge_work(1_000_000 - b.work() - 7)?;
            Ok(())
        },
        |(), _| panic!("publication requires its own prepaid entry work"),
        |_, (), _| panic!("finish must not run"),
    );
    assert!(matches!(
        result,
        Err(ClientError::Resource(Resource::Work(_)))
    ));
    closed(reader);
    assert_eq!(budget.storage(), before + FLOOR);
    assert_eq!(budget.work(), 1_000_000 - 7);
    assert_eq!(budget.failed_work(), Some(1_000_000 + 1));
}

#[test]
fn completion_publication_floor_failure_wins_over_inner_error() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 256 * 1024);
    let policy = policy(&mut budget);
    let before = budget.storage();
    let (mut client, reader) = accounting_client(&mut budget);
    client.deadline += Duration::from_secs(1);
    let result = client.prepare_and_acquire::<(), (), (), ClientError>(
        &policy,
        |_| Ok(()),
        |(), b| {
            b.release_storage(1)?;
            Err(ClientError::Mismatch("publication refused"))
        },
        |_, (), _| panic!("acquisition cannot follow a damaged account"),
    );
    assert!(matches!(
        result,
        Err(ClientError::Resource(Resource::Accounting))
    ));
    closed(reader);
    assert_eq!(budget.storage(), before + TestClient::RETAINED + FLOOR - 1);
}

use super::*;

#[test]
fn process_and_currentness_loss_never_issue_or_retry_lifecycle_calls() {
    let mut first_fixture = fixture();
    let authority = first_fixture.authority(10);
    let key = authority.0.plan.queue;
    let backend = FakeBackend::new(
        first_fixture.foundation,
        vec![success(Mutation::CreateId(5))],
    );
    let calls = backend.calls.clone();
    let opener = backend.opener_pid.clone();
    let mut engine = NativeQueueEngineV1::new(backend).unwrap();
    engine.admit(authority).unwrap();
    opener.set(std::process::id().wrapping_add(1));
    assert_eq!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::ProcessChanged)
    );
    assert!(calls.borrow().is_empty());
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));
    assert!(engine.journal_summary().authority_poisoned);
    drop(engine);
    assert!(calls.borrow().is_empty());

    let mut precheck_fixture = fixture();
    let authority = precheck_fixture.authority(12);
    let key = authority.0.plan.queue;
    let mut backend = FakeBackend::new(
        precheck_fixture.foundation,
        vec![success(Mutation::CreateId(7))],
    );
    backend.fail_currentness_at = Some(1);
    let calls = backend.calls.clone();
    let mut engine = NativeQueueEngineV1::new(backend).unwrap();
    engine.admit(authority).unwrap();
    assert!(matches!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::Currentness(_))
    ));
    assert!(calls.borrow().is_empty());
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));

    let mut second_fixture = fixture();
    let authority = second_fixture.authority(11);
    let key = authority.0.plan.queue;
    let mut backend = FakeBackend::new(
        second_fixture.foundation,
        vec![success(Mutation::CreateId(6))],
    );
    backend.fail_currentness_at = Some(2);
    let calls = backend.calls.clone();
    let mut engine = NativeQueueEngineV1::new(backend).unwrap();
    engine.admit(authority).unwrap();
    assert_eq!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::Currentness(
            "scripted currentness loss"
        ))
    );
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));
    assert_eq!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
    );
    assert_eq!(calls.borrow().len(), 1);
}

#[test]
fn native_create_boundary_callback_runs_once_and_only_after_preflight() {
    let mut rejected_fixture = fixture();
    let authority = rejected_fixture.authority(10);
    let key = authority.0.plan.queue;
    let mut backend = FakeBackend::new(
        rejected_fixture.foundation,
        vec![success(Mutation::CreateId(5))],
    );
    backend.fail_currentness_at = Some(1);
    let mut rejected = NativeQueueEngineV1::new(backend).unwrap();
    rejected.admit(authority).unwrap();
    let rejected_callback = Cell::new(false);
    assert!(matches!(
        rejected.create_at_native_boundary(key, || rejected_callback.set(true)),
        Err(NativeQueueAdapterErrorV1::Currentness(_))
    ));
    assert!(!rejected_callback.get());

    let mut accepted_fixture = fixture();
    let authority = accepted_fixture.authority(11);
    let key = authority.0.plan.queue;
    let backend = FakeBackend::new(
        accepted_fixture.foundation,
        vec![success(Mutation::CreateId(6))],
    );
    let calls = backend.calls.clone();
    let mut accepted = NativeQueueEngineV1::new(backend).unwrap();
    accepted.admit(authority).unwrap();
    let accepted_callback = Cell::new(false);
    accepted
        .create_at_native_boundary(key, || {
            assert!(calls.borrow().is_empty());
            accepted_callback.set(true);
        })
        .unwrap();
    assert!(accepted_callback.get());
    assert!(matches!(calls.borrow().as_slice(), [LoggedCall::Create(_)]));
}

#[test]
fn ambiguous_unknown_id_globally_poisons_create_and_known_id_collision_is_retained() {
    let mut first_fixture = fixture();
    let first = first_fixture.authority(10);
    let second = first_fixture.authority(20);
    let first_key = first.0.plan.queue;
    let second_key = second.0.plan.queue;
    let mut engine = NativeQueueEngineV1::new(FakeBackend::new(
        first_fixture.foundation,
        vec![outcome(QueueSyscallStatusV1::Indeterminate, Mutation::None)],
    ))
    .unwrap();
    engine.admit(first).unwrap();
    engine.admit(second).unwrap();
    assert!(engine.create(first_key).is_err());
    let call_count = engine.backend.calls.borrow().len();
    assert_eq!(
        engine.create(second_key),
        Err(NativeQueueAdapterErrorV1::InvalidPhase)
    );
    assert_eq!(engine.backend.calls.borrow().len(), call_count);

    let mut second_fixture = fixture();
    let first = second_fixture.authority(10);
    let second = second_fixture.authority(20);
    let first_key = first.0.plan.queue;
    let second_key = second.0.plan.queue;
    let mut engine = NativeQueueEngineV1::new(FakeBackend::new(
        second_fixture.foundation,
        vec![
            outcome(QueueSyscallStatusV1::Indeterminate, Mutation::CreateId(31)),
            success(Mutation::CreateId(31)),
        ],
    ))
    .unwrap();
    engine.admit(first).unwrap();
    engine.admit(second).unwrap();
    assert!(engine.create(first_key).is_err());
    assert!(engine.create(second_key).is_err());
    assert_eq!(
        engine.phase(second_key),
        Some(ComputeAqlQueuePhaseV1::Ambiguous)
    );
    assert_eq!(engine.native_queue_id(second_key), None);
    assert_eq!(engine.journal_summary().live_publications, 8);
}

#[test]
fn manifest_digest_is_exact() {
    assert!(
        NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1.contains(&format!(
            "compute_session_sha256={GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1}\n"
        ))
    );
    let actual = Sha256::digest(NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_SHA256_V1);
}

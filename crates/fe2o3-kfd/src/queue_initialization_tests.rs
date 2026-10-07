use super::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

#[test]
fn initialization_failure_retains_exact_backend_and_returned_foundation() {
    use BootstrapCallV1::*;
    for (fault, wrong_pid) in [
        (None, true),
        (Some((Opener, true)), false),
        (Some((Take, false)), false),
        (Some((Take, true)), false),
        (Some((Authenticate, false)), false),
        (Some((Authenticate, true)), false),
    ] {
        let fixture = fixture();
        let issuer = fixture.foundation.issuer().unwrap();
        let memory = fixture.foundation.memory().clone();
        let mut backend = FakeBackend::new(fixture.foundation, Vec::new());
        backend.bootstrap_fault = fault;
        if wrong_pid {
            backend.opener_pid.set(std::process::id() ^ 1);
        }
        let calls = backend.bootstrap_calls.clone();
        let native = backend.calls.clone();
        let drops = backend.drops.clone();
        let mut initialization = NativeQueueEngineInitializationV1::new(backend);
        let result = catch_unwind(AssertUnwindSafe(|| initialization.initialize()));
        if let Some((stage, true)) = fault {
            let payload = match result {
                Err(payload) => payload,
                Ok(_) => panic!("expected panic"),
            };
            assert_eq!(*payload.downcast::<BootstrapCallV1>().unwrap(), stage);
        } else {
            let result = result.unwrap();
            assert!(matches!(result, Err(NativeQueueAdapterErrorV1::ProcessChanged)) == wrong_pid);
            assert!(result.is_err());
        }
        assert_eq!(drops.get(), 0);
        let backend = initialization.backend.as_ref().unwrap();
        let foundation = if matches!(fault, Some((Authenticate, _))) {
            assert!(backend.foundation.is_none());
            initialization.foundation.as_ref().unwrap()
        } else {
            assert!(initialization.foundation.is_none());
            backend.foundation.as_ref().unwrap()
        };
        assert_eq!(foundation.issuer().unwrap(), issuer);
        assert_eq!(foundation.memory(), &memory);
        foundation.authenticate_origin().unwrap();
        let before = calls.borrow().clone();
        assert!(matches!(
            initialization.initialize(),
            Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
        ));
        assert_eq!(*calls.borrow(), before);
        assert!(native.borrow().is_empty());
        assert_eq!(drops.get(), 0);
        drop(initialization);
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn initialization_success_transfers_once_and_rejects_reentry() {
    let fixture = fixture();
    let issuer = fixture.foundation.issuer().unwrap();
    let memory = fixture.foundation.memory().clone();
    let backend = FakeBackend::new(fixture.foundation, Vec::new());
    let calls = backend.bootstrap_calls.clone();
    let drops = backend.drops.clone();
    let mut initialization = NativeQueueEngineInitializationV1::new(backend);
    let engine = initialization.initialize().unwrap();
    assert!(initialization.backend.is_none());
    assert!(initialization.foundation.is_none());
    assert!(engine.backend.foundation.is_none());
    assert!(Rc::ptr_eq(&engine.backend.drops, &drops));
    assert_eq!(engine.foundation.issuer().unwrap(), issuer);
    assert_eq!(engine.foundation.memory(), &memory);
    assert_eq!(
        *calls.borrow(),
        [
            BootstrapCallV1::Opener,
            BootstrapCallV1::Take,
            BootstrapCallV1::Authenticate
        ]
    );
    assert!(matches!(
        initialization.initialize(),
        Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
    ));
    assert_eq!(calls.borrow().len(), 3);
    drop(initialization);
    assert_eq!(drops.get(), 0);
    drop(engine);
    assert_eq!(drops.get(), 1);
}

#[test]
fn admission_precommit_failures_keep_exact_caller_authority() {
    for fault in 0..5 {
        let mut fixture = fixture();
        let mut authority = fixture.authority(10);
        if fault == 2 {
            authority.0.buffers.ring_base_address = 0;
        }
        if fault == 3 {
            authority.0.plan.schema_version += 1;
        }
        if fault == 4 {
            authority.0.plan.resources.context_save.mapping.id.0 += 1;
        }
        let expected = authority.0;
        let mut authority = Some(authority);
        let mut engine =
            NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, Vec::new())).unwrap();
        if fault < 2 {
            engine.backend.bootstrap_fault = Some((BootstrapCallV1::ResourceView, fault == 1));
        }
        let memory = engine.foundation.memory().clone();
        let model = engine.model.clone();
        let result = catch_unwind(AssertUnwindSafe(|| engine.admit_in_place(&mut authority)));
        if fault == 1 {
            assert_eq!(
                *result.unwrap_err().downcast::<BootstrapCallV1>().unwrap(),
                BootstrapCallV1::ResourceView
            );
        } else {
            assert!(result.unwrap().is_err());
        }
        let retained = authority.as_ref().unwrap().0;
        assert_eq!(retained.plan, expected.plan);
        assert_eq!(retained.buffers, expected.buffers);
        assert_eq!(engine.foundation.memory(), &memory);
        assert_eq!(engine.model, model);
        assert!(engine.resources.is_empty());
        assert!(!engine.authority_poisoned);
        assert!(engine.backend.calls.borrow().is_empty());
        assert_eq!(engine.backend.drops.get(), 0);
    }
}

#[test]
fn admission_success_and_revision_exhaustion_transfer_only_into_engine() {
    for exhausted in [false, true] {
        let mut fixture = fixture();
        let first = fixture.authority(10);
        let second = fixture.authority(20);
        let expected = first.0;
        if exhausted {
            fixture
                .foundation
                .set_certificate_revision_for_test(u64::MAX)
                .unwrap();
        }
        let before = fixture.foundation.memory().clone();
        let mut engine =
            NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, Vec::new())).unwrap();
        let mut authority = Some(first);
        let result = engine.admit_in_place(&mut authority);
        assert_eq!(result.is_err(), exhausted);
        assert!(authority.is_none());
        assert_eq!(engine.resources.len(), 1);
        let retained = engine.resources[0].authority.as_ref().unwrap().0;
        assert_eq!(retained.plan, expected.plan);
        assert_eq!(retained.buffers, expected.buffers);
        assert_eq!(engine.authority_poisoned, exhausted);
        if exhausted {
            assert_eq!(engine.foundation.memory(), &before);
            assert!(engine.model.queues().is_empty());
            let mut second = Some(second);
            let calls = engine.backend.bootstrap_calls.borrow().len();
            assert_eq!(
                engine.admit_in_place(&mut second),
                Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
            );
            assert!(second.is_some());
            assert_eq!(engine.backend.bootstrap_calls.borrow().len(), calls);
        } else {
            assert_eq!(result.unwrap(), expected.plan.queue);
            assert_eq!(engine.model.queues().len(), 1);
            assert!(engine.admit_in_place(&mut authority).is_err());
            assert_eq!(engine.resources.len(), 1);
        }
        assert!(engine.backend.calls.borrow().is_empty());
    }
}

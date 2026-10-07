use super::*;
use RuntimeRequestAllocationResultV1::{Outcome, Unsupported};

fn fixture() -> KfdMultiDeviceRuntimeBackendV1 {
    let mut right = KfdRuntimeBackendV1::mock();
    right.description.backend_device = 8;
    KfdMultiDeviceRuntimeBackendV1::from_backends(vec![KfdRuntimeBackendV1::mock(), right]).unwrap()
}

fn dispose_synthetic(mut backend: KfdMultiDeviceRuntimeBackendV1) {
    // Only resource-free mock native state is disarmed, never a native owner.
    for child in &mut backend.children {
        assert!(
            !child.native_available && child.queue.is_none() && child.admitted_device.is_none()
        );
        child.terminal = false;
        let ids: Vec<_> = child.allocations.keys().copied().collect();
        for id in ids {
            child.release_allocation_v1(id).unwrap();
        }
    }
}

#[test]
fn multi_allocation_non_owner_outcomes_preserve_error_and_route_state() {
    for kind in 0..5 {
        let mut backend = fixture();
        let next = backend.next_handle;
        let error = KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "exact diagnostic".to_owned(),
        );
        let pointer = error.detail().as_ptr();
        let result = backend.route_allocation_v1(1, |_| match kind {
            0 => Unsupported,
            1 => Outcome(Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error))),
            2 => Outcome(Err(RuntimeBackendFailureV1::Rejected(error))),
            3 => Outcome(Err(RuntimeBackendFailureV1::Quiescent(error))),
            4 => Outcome(Err(RuntimeBackendFailureV1::Terminal(error))),
            _ => unreachable!(),
        });
        match (kind, result) {
            (0, Unsupported) => {}
            (1, Outcome(Ok(RuntimeBackendAllocationOutcomeV1::SettledNoOwner(error))))
            | (2, Outcome(Err(RuntimeBackendFailureV1::Rejected(error))))
            | (3, Outcome(Err(RuntimeBackendFailureV1::Quiescent(error))))
            | (4, Outcome(Err(RuntimeBackendFailureV1::Terminal(error)))) => {
                assert_eq!(error.detail().as_ptr(), pointer);
                assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
            }
            _ => panic!("allocation outcome changed"),
        }
        assert_eq!(backend.next_handle, next);
        assert!(backend.allocations.is_empty());
        assert_eq!(backend.terminal, kind == 4);
        assert!(
            backend
                .children
                .iter()
                .all(|child| child.next_handle == 1 && child.allocations.is_empty())
        );
        if kind != 4 {
            let retry = backend
                .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap();
            assert_eq!(retry, next);
            assert_eq!(backend.allocations[&retry].child, 1);
            backend.release_allocation_v1(retry).unwrap();
        }
        dispose_synthetic(backend);
    }
}

#[test]
fn multi_allocation_success_routes_equal_child_handles_independently() {
    let mut backend = fixture();
    let left = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let right = backend
        .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    assert_ne!(left, right);
    assert_eq!(
        backend.allocations[&left].local,
        backend.allocations[&right].local
    );
    backend.write_allocation_v1(left, 0, &[1; 8]).unwrap();
    backend.write_allocation_v1(right, 0, &[2; 8]).unwrap();
    let mut output = [0; 8];
    backend.read_allocation_v1(left, 0, &mut output).unwrap();
    assert_eq!(output, [1; 8]);
    backend.read_allocation_v1(right, 0, &mut output).unwrap();
    assert_eq!(output, [2; 8]);
    backend.release_allocation_v1(left).unwrap();
    assert!(backend.children[0].allocations.is_empty());
    assert_eq!(backend.children[1].allocations.len(), 1);
    backend.release_allocation_v1(right).unwrap();
    assert!(backend.allocations.is_empty());
}

#[test]
fn multi_allocation_preflight_never_invokes_child_or_burns_id() {
    for case in 0..4 {
        let mut backend = fixture();
        let child = if case == 3 { 2 } else { 0 };
        match case {
            0 => backend.next_handle = 0,
            1 => backend.next_handle = u64::MAX,
            2 => {
                backend.allocations.insert(
                    1,
                    RoutedHandleV1 {
                        child: 0,
                        local: 99,
                    },
                );
            }
            _ => {}
        }
        let next = backend.next_handle;
        let count = backend.allocations.len();
        let result = backend.route_allocation_v1(child, |_| panic!("preflight entered child"));
        assert!(matches!(result, Outcome(Err(_))));
        assert_eq!(backend.next_handle, next);
        assert_eq!(backend.allocations.len(), count);
        assert_eq!(backend.terminal, case != 1);
        dispose_synthetic(backend);
    }
}

#[test]
fn multi_allocation_zero_child_handle_seals_both_owners() {
    let mut backend = std::mem::ManuallyDrop::new(fixture());
    assert!(matches!(
        backend.route_allocation_v1(1, |_| Outcome(Ok(
            RuntimeBackendAllocationOutcomeV1::Allocated(0),
        ))),
        Outcome(Err(RuntimeBackendFailureV1::Terminal(_)))
    ));
    assert!(backend.terminal && backend.children[1].terminal);
    assert!(!backend.children[0].terminal);
    assert_eq!(backend.next_handle, 1);
    assert!(backend.allocations.is_empty());
    dispose_synthetic(std::mem::ManuallyDrop::into_inner(backend));
}

#[test]
fn multi_allocation_post_owner_panic_preserves_payload_and_hidden_custody() {
    let mut backend = std::mem::ManuallyDrop::new(fixture());
    let payload = Box::new(927_u64);
    let pointer = core::ptr::from_ref(payload.as_ref());
    let result = catch_unwind(AssertUnwindSafe(|| {
        backend.route_allocation_v1(1, |child| {
            child
                .allocate_v1(8, RuntimeMemoryKindV1::HostVisible, 8, 8)
                .unwrap();
            std::panic::resume_unwind(payload);
        })
    }));
    let payload = match result {
        Err(payload) => payload,
        _ => panic!("panic swallowed"),
    };
    let recovered = payload.downcast::<u64>().unwrap();
    assert_eq!(core::ptr::from_ref(recovered.as_ref()), pointer);
    assert!(backend.terminal && backend.children[1].terminal);
    assert!(!backend.children[0].terminal);
    assert_eq!(backend.children[0].next_handle, 1);
    assert!(backend.children[0].allocations.is_empty());
    assert_eq!(backend.children[1].allocations.len(), 1);
    assert_eq!(backend.children[1].staged_context_bytes, 8);
    assert!(backend.allocations.is_empty());
    assert_eq!(backend.next_handle, 1);
    assert!(matches!(
        backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    dispose_synthetic(std::mem::ManuallyDrop::into_inner(backend));
}

#[test]
fn multi_allocation_duplicate_child_insertion_cannot_create_outer_alias() {
    let mut backend = std::mem::ManuallyDrop::new(fixture());
    let first = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        .unwrap();
    let local = backend.allocations[&first].local;
    backend.children[0].next_handle = local;
    let next = backend.next_handle;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            backend.allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 8, 8)
        }))
        .is_err()
    );
    assert!(backend.terminal && backend.children[0].terminal);
    assert!(!backend.children[1].terminal);
    assert_eq!(backend.allocations.len(), 1);
    assert_eq!(backend.allocations[&first].local, local);
    assert_eq!(backend.children[0].allocations.len(), 1);
    assert_eq!(backend.next_handle, next);
    dispose_synthetic(std::mem::ManuallyDrop::into_inner(backend));
}

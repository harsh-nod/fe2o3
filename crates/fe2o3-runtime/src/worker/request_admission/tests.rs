use super::*;
use crate::{
    KfdRuntimeBackendV1, RuntimeBackendProtocolErrorV1, RuntimeBackendResourceKindV1,
    RuntimeValidationErrorV1,
};
use std::cell::RefCell;

#[test]
fn request_owner_scope_preserves_unscoped_dispatch_and_checks_every_owned_handle() {
    let never = std::iter::from_fn(|| -> Option<u64> { panic!("legacy dispatch scanned scope") });
    assert!(reject_foreign_allocations_v1(None, never).is_none());
    let checked = RefCell::new(Vec::new());
    let owns = |handle| {
        checked.borrow_mut().push(handle);
        handle == 41
    };
    assert!(reject_foreign_allocations_v1(Some(&owns), [41, 41]).is_none());
    let response = reject_foreign_allocations_v1(Some(&owns), [41, 99, 41]).unwrap();
    assert_eq!(
        response,
        encode_backend_failure_v1(RuntimeBackendFailureV1::Rejected(()))
    );
    assert_eq!(*checked.borrow(), [41, 41, 41, 99]);
}

#[test]
fn request_owner_open_rejects_legacy_and_returns_backend() {
    let failure = RuntimeWorkerRequestOwnerV1::open(KfdRuntimeBackendV1::mock()).unwrap_err();
    assert!(matches!(
        failure.error(),
        RuntimeErrorV1::Validation(RuntimeValidationErrorV1::Unsupported)
    ));
    let (mut backend, _) = failure.into_parts();
    assert_eq!(backend.enumerate_devices_v1().unwrap()[0].backend_device, 7);
}

#[test]
fn request_owner_classification_never_downgrades_sealed_validation() {
    for terminal in [false, true] {
        for (kind, error) in [
            (
                0,
                RuntimeErrorV1::Validation(RuntimeValidationErrorV1::InvalidBackendDescription),
            ),
            (0, RuntimeErrorV1::BackendRejected("rejected")),
            (1, RuntimeErrorV1::BackendQuiescent("quiescent")),
            (2, RuntimeErrorV1::BackendTerminal("terminal")),
            (
                2,
                RuntimeErrorV1::BackendProtocol(RuntimeBackendProtocolErrorV1::ZeroHandle(
                    RuntimeBackendResourceKindV1::Allocation,
                )),
            ),
        ] {
            let result = classify::<(), _>(terminal, Err(error));
            let tag = match result.unwrap_err() {
                RuntimeBackendFailureV1::Rejected(_) => 0,
                RuntimeBackendFailureV1::Quiescent(_) => 1,
                RuntimeBackendFailureV1::Terminal(_) => 2,
            };
            assert_eq!(tag, if terminal { 2 } else { kind });
        }
    }
}

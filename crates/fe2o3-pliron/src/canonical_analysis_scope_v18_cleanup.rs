use std::panic::{AssertUnwindSafe, catch_unwind};

pub(crate) const CLEANUP_ATTEMPTS: usize = 4;
type PanicPayload = Box<dyn std::any::Any + Send>;

// This bounds destructor calls, not arbitrary Drop work or elapsed time.
// Exhaustion terminates the worker; it cannot promise a typed return or that
// every Rust/external-resource destructor completed during process reclamation.
pub(crate) fn discard_caught_payload(mut payload: PanicPayload) {
    for _ in 0..CLEANUP_ATTEMPTS {
        match catch_unwind(AssertUnwindSafe(|| drop(payload))) {
            Ok(()) => return,
            Err(next) => payload = next,
        }
    }
    std::process::abort();
}

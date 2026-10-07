//! Shared descriptive decisions for issue #182's owner-thread integration.
//!
//! These functions do not authenticate their inputs or grant native authority.
//! The separate Verus abstraction does not prove the threads, channels, adapter
//! observations, or complete executable refinement of the runtime.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R61ShutdownFactsV1 {
    pub worker_returned_normally: bool,
    pub context_cleanup_complete: bool,
    pub native_shutdown_attempted: bool,
    pub native_shutdown_succeeded: bool,
}

pub const fn r61_owner_may_release_v1(facts: R61ShutdownFactsV1) -> bool {
    facts.worker_returned_normally
        && facts.context_cleanup_complete
        && facts.native_shutdown_attempted
        && facts.native_shutdown_succeeded
}

pub const fn r61_reply_may_resolve_v1(already_resolved: bool) -> bool {
    !already_resolved
}

pub const fn r61_operation_registry_accepts_v1(retained: usize, capacity: usize) -> bool {
    capacity > 0 && capacity <= 65_536 && retained < capacity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shutdown_checks_every_boolean_combination() {
        for bits in 0..16 {
            let facts = R61ShutdownFactsV1 {
                worker_returned_normally: bits & 1 != 0,
                context_cleanup_complete: bits & 2 != 0,
                native_shutdown_attempted: bits & 4 != 0,
                native_shutdown_succeeded: bits & 8 != 0,
            };
            assert_eq!(r61_owner_may_release_v1(facts), bits == 15);
        }
    }

    #[test]
    fn reply_resolution_is_consumed_once() {
        assert!(r61_reply_may_resolve_v1(false));
        assert!(!r61_reply_may_resolve_v1(true));
    }

    #[test]
    fn registry_admission_checks_boundary_and_overflow_inputs() {
        for capacity in [0, 1, 1024, 65_536, 65_537, usize::MAX] {
            for retained in [0, 1, capacity.saturating_sub(1), capacity, usize::MAX] {
                let admitted = r61_operation_registry_accepts_v1(retained, capacity);
                assert_eq!(
                    admitted,
                    (1..=65_536).contains(&capacity) && retained < capacity
                );
                if admitted {
                    assert!(retained.checked_add(1).unwrap() <= capacity);
                }
            }
        }
    }
}

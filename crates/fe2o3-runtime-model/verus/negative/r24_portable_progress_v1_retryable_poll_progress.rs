use vstd::prelude::*;
verus! {
pub struct ProgressRegistrationV1 {
    pub observing: bool,
    pub registration_active: bool,
    pub custody_retained: bool,
}

// Mutation: retryable poll keeps custody but retires progress registration.
pub open spec fn mutated_retryable_poll_v1(before: ProgressRegistrationV1)
    -> ProgressRegistrationV1 {
    ProgressRegistrationV1 {
        observing: before.observing,
        registration_active: false,
        custody_retained: before.custody_retained,
    }
}

pub proof fn mutated_retryable_poll_retires_progress_registration_v1()
    ensures {
        let before = ProgressRegistrationV1 {
            observing: true,
            registration_active: true,
            custody_retained: true,
        };
        let after = mutated_retryable_poll_v1(before);
        after.registration_active && after.custody_retained
    }, {}
}

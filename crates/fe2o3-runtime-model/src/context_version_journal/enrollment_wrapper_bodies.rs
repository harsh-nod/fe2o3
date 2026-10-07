// Public enrollment forwarding, shared with the actual-owner proof harness.
macro_rules! journal_enrollment_wrapper_body {
    ($journal:ident, $entries:ident, $output:ident, $execute:path) => {
        $execute($journal, $entries, $output)
    };
}

macro_rules! stable_enrollment_wrapper_body {
    ($owner:ident, $entries:ident, $output:ident) => {
        $owner.journal.enroll_allocations($entries, $output)
    };
}

macro_rules! producer_enrollment_wrapper_body {
    ($owner:ident, $entries:ident, $output:ident) => {
        $owner.stable.enroll_allocations($entries, $output)
    };
}

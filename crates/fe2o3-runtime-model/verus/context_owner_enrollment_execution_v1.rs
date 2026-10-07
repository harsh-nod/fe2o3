// One actual declaration universe, including the public owner adapters.
include!("context_owner_unknown_execution_v1.rs");
use vstd::prelude::verus as enrollment_declarations_v1;
include!("../src/context_version_journal/enrollment_declarations.rs");
use self::ContextAllocationEnrollmentV1 as EnrollmentV1;
use self::ContextVersionJournalErrorV1 as EnrollmentErrorV1;
include!("../src/context_version_journal/enrollment_ordering_bodies.rs");
include!("context_journal_enrollment_ordering_bodies_v1.rs");
include!("../src/context_version_journal/enrollment_adaptive_bodies.rs");
include!("context_journal_enrollment_adaptive_bodies_v1.rs");
include!("../src/context_version_journal/enrollment_bodies.rs");
include!("context_journal_enrollment_execution_bodies_v1.rs");
include!("../src/context_version_journal/enrollment_wrapper_bodies.rs");
include!("context_owner_enrollment_bodies_v1.rs");

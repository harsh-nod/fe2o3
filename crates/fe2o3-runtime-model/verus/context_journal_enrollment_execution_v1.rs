// Full batch execution on actual declarations; no historical Vec witness is assumed.
include!("context_journal_enrollment_adaptive_v1.rs");
use self::ContextVersionJournalV1 as JournalContentsV1;
use self::ContextVersionJournalErrorV1 as EnrollmentErrorV1;
include!("../src/context_version_journal/enrollment_bodies.rs");
include!("context_journal_enrollment_execution_bodies_v1.rs");

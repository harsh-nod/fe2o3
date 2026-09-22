// Standalone actual-type ordering proof. No historical Vec witness is assumed.
use vstd::prelude::*;
use vstd::prelude::verus as context_journal_declarations_v1;
use vstd::prelude::verus as enrollment_declarations_v1;
include!("../src/context_version_journal/declarations.rs");
include!("../src/context_version_journal/enrollment_declarations.rs");
include!("../src/context_version_journal/enrollment_ordering_bodies.rs");

include!("context_journal_enrollment_ordering_bodies_v1.rs");

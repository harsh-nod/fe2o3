// Actual-type Begin execution; normal contents, not physical Vec storage or unwind refinement.
use vstd::prelude::*;
use vstd::prelude::verus as context_journal_declarations_v1;
include!("../src/context_version_journal/declarations.rs");
// This standalone root instantiates only the three retained lookup/comparison templates.
#[allow(unused_macros)]
#[macro_use]
mod retained_templates {
    include!("../src/context_version_journal/retained_bodies.rs");
}
include!("../src/context_version_journal/begin_bodies.rs");

use self::ContextAllocationKeyV1 as AllocationKeyV1;
use self::ContextAllocationReferenceV1 as AllocationReferenceV1;
use self::ContextAllocationWriteV1 as AllocationWriteV1;
use self::ContextVersionJournalErrorV1 as ReadErrorV1;
use self::ContextVersionJournalV1 as JournalContentsV1;
use self::ContextWriterKeyV1 as WriterKeyV1;
use self::ContextWriterKindV1 as WriterKindV1;
use self::ContextWriterReferenceV1 as WriterReferenceV1;

include!("context_journal_begin_decisions_v1.rs");
include!("context_journal_begin_bodies_v1.rs");
include!("context_journal_begin_witnesses_v1.rs");

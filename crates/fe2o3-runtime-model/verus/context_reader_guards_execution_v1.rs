// Actual owner types and shared Begin guards; normal contents, not allocator/unwind refinement.
include!("context_journal_begin_execution_v1.rs");
use vstd::prelude::verus as context_read_declarations_v1;
use vstd::prelude::verus as context_producer_read_declarations_v1;
include!("../src/context_read_leases/declarations.rs");
include!("../src/context_producer_reads/declarations.rs");
include!("../src/context_version_journal/inspection_bodies.rs");
include!("context_owner_inspection_bodies_v1.rs");
include!("../src/context_version_journal/lookup_bodies.rs");
include!("../src/context_read_leases/guard_bodies.rs");
include!("context_reader_guards_decisions_v1.rs");
include!("context_reader_guards_bodies_v1.rs");

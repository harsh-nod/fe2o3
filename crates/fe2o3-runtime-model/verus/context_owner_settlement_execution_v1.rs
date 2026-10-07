// Extend the public writer universe; no substitute owner declarations.
include!("context_owner_writer_execution_v1.rs");
include!("../src/context_version_journal/settlement_return_body.rs");
include!("../src/context_version_journal/settlement_scratch_bodies.rs");
include!("../src/context_version_journal/settlement_commit_body.rs");
include!("../src/context_version_journal/settlement_wrapper_bodies.rs");

mod settlement_storage {
    use vstd::prelude::verus as settlement_storage_declarations_v1;
    include!("../src/context_version_journal/settlement_storage_declarations.rs");
}
use settlement_storage::SettlementReturnStorageV1;

include!("context_owner_settlement_decisions_v1.rs");
include!("context_owner_settlement_bodies_v1.rs");
include!("context_owner_settlement_witnesses_v1.rs");

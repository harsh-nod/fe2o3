// Expected-negative R45 mutation: an out-of-range fault can reach success.
use vstd::prelude::*;
verus! {
pub enum PublicationResultV1 { Rejected, Succeeded }
pub open spec fn barrier_count_v1() -> nat { 1 }
pub open spec fn fault_index_v1() -> nat { 1 }
pub open spec fn mutated_publication_result_v1() -> PublicationResultV1 {
    PublicationResultV1::Succeeded
}
pub proof fn mutated_malformed_fault_fails_closed_v1()
    ensures fault_index_v1() >= barrier_count_v1()
        ==> mutated_publication_result_v1() == PublicationResultV1::Rejected,
{}
}

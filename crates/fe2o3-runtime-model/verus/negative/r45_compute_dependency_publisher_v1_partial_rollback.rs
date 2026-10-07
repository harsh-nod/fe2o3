// Expected-negative R45 mutation: failed preflight releases a reader prefix.
use vstd::prelude::*;
verus! {
pub enum PreflightResultV1 { Rejected, Passed }
pub open spec fn mutated_preflight_result_v1() -> PreflightResultV1 {
    PreflightResultV1::Rejected
}
pub open spec fn released_readers_v1() -> nat { 1 }
pub proof fn mutated_failed_preflight_releases_nothing_v1()
    ensures mutated_preflight_result_v1() == PreflightResultV1::Rejected
        ==> released_readers_v1() == 0,
{}
}

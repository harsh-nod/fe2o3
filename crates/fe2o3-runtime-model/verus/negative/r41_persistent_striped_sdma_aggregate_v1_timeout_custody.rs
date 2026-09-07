// Expected-negative R41 mutation: timeout drops the whole published submission owner.
use vstd::prelude::*;
verus! {
pub open spec fn request_count_v1() -> nat { 8 }
pub open spec fn mutated_timeout_custody_len_v1() -> nat { 0 }
pub proof fn mutated_timeout_retains_whole_custody_v1()
    ensures mutated_timeout_custody_len_v1() == request_count_v1(),
{}
}

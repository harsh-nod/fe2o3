// Expected negative: only cross-engine checks are kept; compute uniqueness is lost.
use vstd::prelude::*;
verus! {
pub open spec fn cross_engine_only_v1(first: u64, second: u64, copy: u64) -> bool {
    first != 0 && second != 0 && copy != 0 && first != copy && second != copy
}
pub proof fn mutated_duplicate_compute_v1(first: u64, second: u64, copy: u64)
    requires cross_engine_only_v1(first, second, copy),
    ensures first != second,
{}
}

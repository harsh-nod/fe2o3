use vstd::prelude::*;
verus! {
pub open spec fn mutated_aggregate_bytes_v1(window_bytes: nat) -> nat {
    (window_bytes - 1) as nat
}
pub proof fn mutated_d2d_completion_aggregate_is_exact_v1()
    ensures mutated_aggregate_bytes_v1(4096) == 4096, {}
}

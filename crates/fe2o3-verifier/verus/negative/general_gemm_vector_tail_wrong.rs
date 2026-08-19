use vstd::prelude::*;

#[path = "../general_gemm_schedule_model_v1.rs"]
mod model;

verus! {

// Removing the full-vector predicate permits a four-element transfer at the
// final K-tail even though component three is outside the logical allocation.
pub proof fn mutated_unguarded_vector_tail_is_bounded_v1(
    group_y: nat,
    phase: nat,
    lane: nat,
    m: nat,
    k: nat,
    lda: nat,
)
    requires
        lane < 64,
        lda >= k,
        model::a_row_v1(group_y, lane) < m,
        model::phase_depth_v1(phase, lane, 0) < k,
    ensures
        model::phase_depth_v1(phase, lane, 3) < k,
{
}

fn main() {}

}

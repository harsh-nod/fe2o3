use vstd::prelude::*;

verus! {

/// Target-neutral model of the identity write mapping used by the Rust example.
pub open spec fn output_index(thread: nat) -> nat {
    thread
}

pub open spec fn vecadd_value(a: Seq<int>, b: Seq<int>, thread: nat) -> int
    recommends
        thread < a.len(),
        thread < b.len(),
{
    a[thread as int] + b[thread as int]
}

pub open spec fn vecadd_write(
    old_output: Seq<int>,
    a: Seq<int>,
    b: Seq<int>,
    thread: nat,
) -> Seq<int>
    recommends
        thread < old_output.len(),
        thread < a.len(),
        thread < b.len(),
{
    old_output.update(output_index(thread) as int, vecadd_value(a, b, thread))
}

pub proof fn per_thread_vecadd_is_in_bounds(
    a: Seq<int>,
    b: Seq<int>,
    output: Seq<int>,
    thread: nat,
)
    requires
        a.len() == b.len(),
        a.len() == output.len(),
        thread < output.len(),
    ensures
        output_index(thread) < a.len(),
        output_index(thread) < b.len(),
        output_index(thread) < output.len(),
        vecadd_value(a, b, thread) == a[thread as int] + b[thread as int],
{
}

/// Since each thread's write set is the singleton `{output_index(thread)}`,
/// unequal indices establish pairwise-disjoint writes.
pub proof fn distinct_threads_have_disjoint_outputs(
    left: nat,
    right: nat,
    thread_count: nat,
)
    requires
        left < thread_count,
        right < thread_count,
        left != right,
    ensures
        output_index(left) != output_index(right),
{
}

pub proof fn vecadd_changes_only_the_owned_output(
    old_output: Seq<int>,
    a: Seq<int>,
    b: Seq<int>,
    thread: nat,
    other: nat,
)
    requires
        old_output.len() == a.len(),
        old_output.len() == b.len(),
        thread < old_output.len(),
        other < old_output.len(),
        other != output_index(thread),
    ensures
        vecadd_write(old_output, a, b, thread)[other as int] == old_output[other as int],
{
}

/// Trusted hardware/backend boundary. The backend must refine this contract,
/// and launch composition must separately guarantee distinct IDs for distinct
/// active threads.
#[verifier::external_body]
pub fn hardware_thread_id(thread_count: usize) -> (thread: usize)
    requires
        thread_count > 0,
    ensures
        thread < thread_count,
{
    unimplemented!()
}

} // verus!

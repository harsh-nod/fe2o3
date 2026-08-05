use vstd::prelude::*;

verus! {

pub open spec fn fill_write(old_output: Seq<int>, value: int, thread: nat) -> Seq<int>
    recommends
        thread < old_output.len(),
{
    old_output.update(thread as int, value)
}

/// Expected failure: one thread's write cannot establish a full-buffer fill.
pub proof fn mutated_one_write_fills_every_element(
    old_output: Seq<int>,
    value: int,
    thread: nat,
)
    requires
        thread < old_output.len(),
    ensures
        forall |index: nat| index < old_output.len() ==>
            fill_write(old_output, value, thread)[index as int] == value,
{
}

} // verus!

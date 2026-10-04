use vstd::prelude::*;

verus! {

/// Target-neutral model of the identity write mapping used by the Rust fill.
pub open spec fn output_index(thread: nat) -> nat {
    thread
}

pub open spec fn fill_write(old_output: Seq<int>, value: int, thread: nat) -> Seq<int>
    recommends
        thread < old_output.len(),
{
    old_output.update(output_index(thread) as int, value)
}

pub open spec fn fill_postcondition(output: Seq<int>, value: int) -> bool {
    forall |index: nat| index < output.len() ==> output[index as int] == value
}

/// Address arithmetic is modeled in mathematical naturals. `address_space_size`
/// is an exclusive upper bound supplied by the target environment identity.
pub open spec fn fill_byte_address(
    base_address: nat,
    thread: nat,
    element_size: nat,
) -> nat {
    base_address + output_index(thread) * element_size
}

pub open spec fn fill_byte_end(
    base_address: nat,
    thread: nat,
    element_size: nat,
) -> nat {
    fill_byte_address(base_address, thread, element_size) + element_size
}

pub proof fn per_thread_fill_is_in_bounds_and_address_representable(
    output: Seq<int>,
    thread: nat,
    base_address: nat,
    element_size: nat,
    address_space_size: nat,
)
    requires
        thread < output.len(),
        element_size > 0,
        base_address + output.len() * element_size <= address_space_size,
    ensures
        output_index(thread) < output.len(),
        fill_byte_address(base_address, thread, element_size)
            < fill_byte_end(base_address, thread, element_size),
        fill_byte_end(base_address, thread, element_size) <= address_space_size,
{
    assert(thread + 1 <= output.len());
    assert((thread + 1) * element_size <= output.len() * element_size) by (nonlinear_arith)
        requires
            thread + 1 <= output.len(),
            element_size > 0,
    ;
    assert(fill_byte_end(base_address, thread, element_size)
        == base_address + (thread + 1) * element_size) by (nonlinear_arith)
        requires
            output_index(thread) == thread,
    ;
}

/// Since each thread's write set is the singleton `{output_index(thread)}`,
/// unequal logical IDs establish pairwise-disjoint writes.
pub proof fn distinct_threads_have_disjoint_fill_outputs(
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

pub proof fn fill_write_sets_only_the_owned_output(
    old_output: Seq<int>,
    value: int,
    thread: nat,
    other: nat,
)
    requires
        thread < old_output.len(),
        other < old_output.len(),
        other != output_index(thread),
    ensures
        fill_write(old_output, value, thread).len() == old_output.len(),
        fill_write(old_output, value, thread)[output_index(thread) as int] == value,
        fill_write(old_output, value, thread)[other as int] == old_output[other as int],
{
}

/// Explicit contract the backend must refine for a one-dimensional launch.
/// `active_slot` is the launch slot and `observed_id` is the hardware global ID.
pub open spec fn hardware_thread_id_contract(
    active_slot: nat,
    observed_id: nat,
    thread_count: nat,
) -> bool {
    active_slot < thread_count && observed_id == active_slot
}

/// Trusted hardware/backend boundary. Passing `active_slot` is ghost modeling
/// for this spike; it is not a claim that the GPU intrinsic takes that argument.
#[verifier::external_body]
pub fn hardware_thread_id(active_slot: usize, thread_count: usize) -> (thread: usize)
    requires
        active_slot < thread_count,
    ensures
        hardware_thread_id_contract(
            active_slot as nat,
            thread as nat,
            thread_count as nat,
        ),
{
    unimplemented!()
}

pub proof fn hardware_ids_produce_disjoint_fill_writes(
    left_slot: nat,
    right_slot: nat,
    left_id: nat,
    right_id: nat,
    thread_count: nat,
)
    requires
        hardware_thread_id_contract(left_slot, left_id, thread_count),
        hardware_thread_id_contract(right_slot, right_id, thread_count),
        left_slot != right_slot,
    ensures
        output_index(left_id) != output_index(right_id),
{
}

/// If every active slot satisfies the hardware-ID contract and its observed ID
/// performs its fill write, every output element has the requested value.
pub proof fn completed_hardware_fill_satisfies_postcondition(
    output: Seq<int>,
    value: int,
    observed_ids: Seq<nat>,
    thread_count: nat,
)
    requires
        output.len() == thread_count,
        observed_ids.len() == thread_count,
        forall |slot: nat| slot < thread_count ==>
            hardware_thread_id_contract(
                slot,
                observed_ids[slot as int],
                thread_count,
            ),
        forall |slot: nat| slot < thread_count ==>
            output[output_index(observed_ids[slot as int]) as int] == value,
    ensures
        fill_postcondition(output, value),
{
    assert forall |index: nat| index < output.len() implies
        output[index as int] == value by {
        assert(index < thread_count);
        assert(hardware_thread_id_contract(
            index,
            observed_ids[index as int],
            thread_count,
        ));
        assert(observed_ids[index as int] == index);
        assert(output[output_index(observed_ids[index as int]) as int] == value);
    }
}

} // verus!

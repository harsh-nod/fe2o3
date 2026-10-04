use vstd::prelude::*;

verus! {

pub open spec fn address_space_writable(address_space: nat) -> bool {
    (address_space == 0 || address_space == 1 || address_space == 3
        || address_space == 4 || address_space == 5)
        && address_space != 4
}

pub proof fn mutated_constant_memory_is_writable()
    ensures
        address_space_writable(4),
{
}

} // verus!

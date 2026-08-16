use vstd::prelude::*;
verus! {
pub proof fn mutated_extent_contains_element_v1(element: nat)
    requires element < 129,
    ensures element < 128,
{
}
}

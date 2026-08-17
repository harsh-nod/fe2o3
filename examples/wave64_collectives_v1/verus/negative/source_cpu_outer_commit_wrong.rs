use vstd::prelude::*;

verus! {

pub proof fn mutated_outer_public_base_commit_is_exact_v2()
    ensures 0xb8daeb2bint == 0xc8daeb2bint,
{
}

}

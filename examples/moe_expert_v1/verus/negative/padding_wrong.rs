use vstd::prelude::*;
verus! {
pub proof fn mutated_padding_is_active_v1(row: nat)
    requires 4 <= row < 16,
    ensures row < 4,
{
}
}

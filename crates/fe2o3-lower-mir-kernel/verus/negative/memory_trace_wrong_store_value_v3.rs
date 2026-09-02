use vstd::prelude::*;

verus! {

pub open spec fn source_diamond_helper_v3(left: int, right: int, fallback: int) -> int {
    // Fixed witness values make XOR nonzero, so the exact helper returns fallback.
    if left == 0xf0 && right == 0x0f { fallback } else { 0 }
}

pub proof fn wrong_call_result_store_cannot_refine_v3()
    ensures source_diamond_helper_v3(0xf0, 0x0f, 17) == 0,
{
}

}

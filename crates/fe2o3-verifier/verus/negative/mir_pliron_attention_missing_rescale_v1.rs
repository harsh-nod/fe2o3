use vstd::prelude::*;

verus! {

proof fn missing_attention_rescale(previous: int, term: int, rescale: int)
    requires rescale != 1, previous != 0,
    ensures
        previous + term == previous * rescale + term,
{
}

}

fn main() {}

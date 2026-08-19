use vstd::prelude::*;

verus! {

pub proof fn mutated_bf16_widening_drops_the_sign_bit_v1(bits: nat)
    requires bits < 65536,
    ensures ((bits % 32768) * 65536) / 65536 == bits,
{
}

fn main() {}

}

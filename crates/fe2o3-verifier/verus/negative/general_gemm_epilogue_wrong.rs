use vstd::prelude::*;

verus! {

pub proof fn mutated_epilogue_omits_beta_v1(
    alpha: real,
    accumulator: real,
    beta: real,
    c: real,
)
    ensures alpha * accumulator + c == alpha * accumulator + beta * c,
{
}

fn main() {}

}

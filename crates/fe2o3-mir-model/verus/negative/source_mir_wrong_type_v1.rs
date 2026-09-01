use vstd::prelude::*;
verus! {
proof fn wrong_type(source_bits: nat)
    requires source_bits == 32,
    ensures source_bits == 64,
{}
}
fn main() {}

use vstd::prelude::*;
verus! {
spec fn effects(left: int, right: int, destination: int) -> Seq<int> {
    seq![left, right, destination, (left + right) % 4294967296]
}
proof fn wrong_destination_is_not_a_composition(left: int, right: int, destination: int)
    ensures effects(left, right, destination) == effects(left, right, destination + 1),
{}
}
fn main() {}

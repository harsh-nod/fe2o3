use vstd::prelude::*;
verus! {
proof fn wrong_destination_is_not_a_composition(
    values: Map<int, int>,
    destination: int,
    wrong_destination: int,
    result: int,
)
    requires destination != wrong_destination,
    ensures
        values.insert(destination, result)[destination]
            == values.insert(wrong_destination, result)[destination],
{}
}
fn main() {}

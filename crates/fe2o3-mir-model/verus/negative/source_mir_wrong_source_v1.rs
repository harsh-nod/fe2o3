use vstd::prelude::*;
verus! {
proof fn wrong_source_binding(source_binding: int)
    ensures source_binding == source_binding + 1,
{}
}
fn main() {}

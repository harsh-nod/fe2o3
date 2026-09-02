use vstd::prelude::*;
verus! {
proof fn cross_owner_splice_is_not_an_exact_join(source_owner: int, kir_owner: int)
    requires source_owner != kir_owner,
    ensures source_owner == kir_owner,
{}
}
fn main() {}

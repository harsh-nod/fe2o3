use vstd::prelude::*;
verus! {
spec fn norm(v: int) -> int { v % 4294967296 }
proof fn wrong_operator(left: int, right: int)
    ensures norm(left + right) == norm(left - right),
{}
}
fn main() {}

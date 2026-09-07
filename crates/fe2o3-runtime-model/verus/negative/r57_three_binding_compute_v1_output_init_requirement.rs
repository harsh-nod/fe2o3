// Expected-negative R57 mutation: write-only C is required to open initialized.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_output_requires_initialization_v1() -> bool { true }
pub proof fn mutated_output_init_requirement_is_rejected_v1()
    ensures !mutated_output_requires_initialization_v1(),
{}
}
fn main() {}

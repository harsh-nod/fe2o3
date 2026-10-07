// Expected-negative R57 mutation: admission requires write-only C to be initialized.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_output_admitted_v1(
    role_is_write: bool, effect_is_write_only: bool, initialized: bool,
) -> bool {
    role_is_write && effect_is_write_only && initialized
}
pub proof fn mutated_output_init_requirement_is_rejected_v1()
    ensures mutated_output_admitted_v1(true, true, false),
{}
}
fn main() {}

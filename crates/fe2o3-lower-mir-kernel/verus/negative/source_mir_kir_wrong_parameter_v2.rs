use vstd::prelude::*;
verus! {
proof fn wrong_parameter_ordinal_is_not_an_exact_join(parameter: int)
    ensures parameter == parameter + 1,
{}
}
fn main() {}

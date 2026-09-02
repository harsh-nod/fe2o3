use vstd::prelude::*;
verus! {
proof fn wrong_parameter_ordinal_is_not_an_exact_join(
    local_to_ssa: Map<int, int>,
    semantic_local: int,
    parameter: int,
    wrong_parameter: int,
)
    requires
        local_to_ssa.dom().contains(semantic_local),
        local_to_ssa[semantic_local] == parameter,
        parameter != wrong_parameter,
    ensures local_to_ssa[semantic_local] == wrong_parameter,
{}
}
fn main() {}

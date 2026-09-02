use vstd::prelude::*;
verus! {
proof fn wrong_result_ssa_is_not_an_exact_state_update(
    values: Map<int, int>,
    result_ssa: int,
    wrong_result_ssa: int,
    result: int,
)
    requires result_ssa != wrong_result_ssa,
    ensures
        values.insert(result_ssa, result)[result_ssa]
            == values.insert(wrong_result_ssa, result)[result_ssa],
{}
}
fn main() {}

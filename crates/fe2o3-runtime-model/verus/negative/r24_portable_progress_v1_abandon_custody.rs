use vstd::prelude::*;
verus! {
pub open spec fn mutated_custody_before_drop_v1() -> bool { true }
pub open spec fn mutated_custody_after_drop_v1() -> bool { false }
pub proof fn mutated_drop_preserves_custody_v1()
    ensures mutated_custody_before_drop_v1() == mutated_custody_after_drop_v1(), {}
}

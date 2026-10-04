use vstd::prelude::*;
verus! {
pub enum PhaseV1 { Prepared, Quarantined }
pub open spec fn mutated_retained_failure_v1() -> PhaseV1 { PhaseV1::Prepared }
pub proof fn mutated_retained_indeterminate_is_quarantined_v1()
    ensures mutated_retained_failure_v1() == PhaseV1::Quarantined,
{}
}

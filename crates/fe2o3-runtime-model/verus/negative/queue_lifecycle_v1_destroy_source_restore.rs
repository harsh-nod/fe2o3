use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum QueuePhaseV1 {
    Active,
    Disabled,
    DestroyPending,
}

pub open spec fn mutated_failed_destroy_source_v1(source: QueuePhaseV1) -> QueuePhaseV1 {
    if source == QueuePhaseV1::Active || source == QueuePhaseV1::Disabled {
        QueuePhaseV1::Disabled
    } else {
        source
    }
}

pub proof fn mutated_active_destroy_failure_restores_exact_source_v1()
    ensures
        mutated_failed_destroy_source_v1(QueuePhaseV1::Active) == QueuePhaseV1::Active,
{
}

} // verus!

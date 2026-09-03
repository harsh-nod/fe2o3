use vstd::prelude::*;

verus! {

pub struct StateV1 {
    pub submission_id: nat,
    pub dependencies: Set<nat>,
    pub terminal: bool,
    pub reserved: bool,
    pub owns_slot: bool,
    pub owns_resource: bool,
}

pub open spec fn mutated_release_v1(
    state: StateV1,
    _submissions: Seq<StateV1>,
) -> StateV1 {
    if state.terminal {
        StateV1 { owns_slot: false, owns_resource: false, ..state }
    } else {
        state
    }
}

pub proof fn mutated_reserved_dependent_blocks_terminal_release_v1(
    producer: StateV1,
    dependent: StateV1,
)
    requires
        producer.terminal,
        producer.owns_slot,
        producer.owns_resource,
        dependent.reserved,
        dependent.dependencies.contains(producer.submission_id),
    ensures mutated_release_v1(producer, Seq::empty().push(dependent)) == producer,
{
}

}

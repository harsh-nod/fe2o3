use vstd::prelude::*;

verus! {

pub struct StateV1 {
    pub submission_id: nat,
    pub dependencies: Set<nat>,
    pub published: bool,
    pub terminal_succeeded: bool,
}

pub open spec fn mutated_publish_v1(
    state: StateV1,
    _submissions: Seq<StateV1>,
) -> StateV1 {
    StateV1 { published: true, ..state }
}

pub proof fn mutated_unready_dependency_blocks_publication_v1(
    consumer: StateV1,
    failed_producer: StateV1,
)
    requires
        !consumer.published,
        consumer.dependencies.contains(failed_producer.submission_id),
        !failed_producer.terminal_succeeded,
    ensures mutated_publish_v1(consumer, Seq::empty().push(failed_producer)) == consumer,
{
}

}

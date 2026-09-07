// Expected-negative R56 mutation: a first-native indeterminate result falsely
// duplicates that first shard as untouched instead of retaining the complete
// second-native request-index/ticket partition.
use vstd::prelude::*;
verus! {
pub struct ShardV1 {
    pub native: nat,
    pub request_indices: Seq<nat>,
    pub ticket_request_indices: Seq<nat>,
}

pub struct FirstIndeterminateCustodyV1 {
    pub confirmed_prefix: nat,
    pub indeterminate: ShardV1,
    pub untouched: ShardV1,
    pub cursor_committed: bool,
}

pub open spec fn mutated_first_indeterminate_custody_v1()
    -> FirstIndeterminateCustodyV1
{
    let first = ShardV1 {
        native: 1,
        request_indices: seq![0nat, 2nat],
        ticket_request_indices: seq![0nat, 2nat],
    };
    FirstIndeterminateCustodyV1 {
        confirmed_prefix: 0,
        indeterminate: first,
        untouched: first,
        cursor_committed: false,
    }
}

pub proof fn mutated_first_indeterminate_custody_is_rejected_v1()
    ensures {
        let out = mutated_first_indeterminate_custody_v1();
        out.confirmed_prefix == 0
            && out.indeterminate.native == 1
            && out.indeterminate.request_indices == seq![0nat, 2nat]
            && out.indeterminate.ticket_request_indices == seq![0nat, 2nat]
            && out.untouched.native == 0
            && out.untouched.request_indices == seq![1nat, 3nat]
            && out.untouched.ticket_request_indices == seq![1nat, 3nat]
            && !out.cursor_committed
    },
{}
}
fn main() {}

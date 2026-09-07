// Expected-negative R56 mutation: wait timeout loses completed publications
// and rolls back the already committed cursor.
use vstd::prelude::*;
verus! {
pub struct PublishedV1 {
    pub requests: nat,
    pub publications: nat,
    pub tails: nat,
    pub cursor_committed: bool,
}

pub open spec fn mutated_wait_timeout_v1(published: PublishedV1) -> PublishedV1 {
    PublishedV1 { requests: published.requests, publications: 0, tails: 0,
        cursor_committed: false }
}

pub proof fn mutated_timeout_retains_exact_custody_v1(published: PublishedV1)
    requires published.requests >= 2, published.publications == 2,
        published.tails == 2, published.cursor_committed,
    ensures {
        let pending = mutated_wait_timeout_v1(published);
        pending.requests == published.requests
            && pending.publications == published.publications
            && pending.tails == published.tails
            && pending.cursor_committed == published.cursor_committed
    },
{}
}
fn main() {}

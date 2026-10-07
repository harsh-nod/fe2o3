// Expected-negative R48 mutation: same-queue packet occurrences may alias.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_same_queue_packets_are_distinct_v1(
    _left_queue: nat, _left_packet: nat, _right_queue: nat, _right_packet: nat,
) -> bool { true }
pub proof fn same_queue_duplicate_packet_is_rejected_v1(queue: nat, packet: nat)
    ensures !mutated_same_queue_packets_are_distinct_v1(queue, packet, queue, packet),
{}
}

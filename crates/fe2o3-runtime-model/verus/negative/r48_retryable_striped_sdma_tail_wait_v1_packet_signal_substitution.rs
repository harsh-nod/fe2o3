// Expected-negative R48 mutation: packet completion signal differs from its ticket.
use vstd::prelude::*;
verus! {
pub open spec fn mutated_packet_signal_v1(ticket_signal: nat) -> nat { ticket_signal + 1 }
pub proof fn packet_signal_is_ticket_signal_v1(ticket_signal: nat)
    ensures mutated_packet_signal_v1(ticket_signal) == ticket_signal,
{}
}

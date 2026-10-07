// Expected-negative R48 mutation: publication omits the complete packet body premise.
use vstd::prelude::*;
verus! {
pub open spec fn required_packet_body_words_v1() -> nat { 15 }
pub open spec fn mutated_packet_body_words_written_v1() -> nat { 14 }
pub proof fn complete_packet_body_precedes_publication_v1()
    ensures mutated_packet_body_words_written_v1() == required_packet_body_words_v1(),
{}
}

use vstd::prelude::*;

verus! {

pub open spec fn mutated_vendor_body_v1() -> Seq<u8> {
    Seq::new(64, |index: int|
        if index == 0 {
            0u8
        } else if index == 2 {
            1u8
        } else {
            0u8
        }
    )
}

pub open spec fn invalid_body_v1(packet: Seq<u8>) -> bool {
    packet.len() == 64 && packet[0] == 1u8
}

pub proof fn mutated_vendor_body_is_invalid_v1()
    ensures
        invalid_body_v1(mutated_vendor_body_v1()),
{
}

} // verus!

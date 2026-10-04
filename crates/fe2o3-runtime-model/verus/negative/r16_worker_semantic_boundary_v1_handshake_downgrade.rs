use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum HandshakeV1 { RuntimeV4, ExactRuntimeV5 }

#[derive(PartialEq, Eq)]
pub enum PhaseV1 { ReadyV5, Terminal }

pub open spec fn mutated_negotiate_v1(handshake: HandshakeV1) -> PhaseV1 {
    if handshake == HandshakeV1::RuntimeV4 { PhaseV1::ReadyV5 } else { PhaseV1::ReadyV5 }
}

pub proof fn mutated_v4_handshake_is_rejected_v1()
    ensures mutated_negotiate_v1(HandshakeV1::RuntimeV4) == PhaseV1::Terminal,
{
}

}

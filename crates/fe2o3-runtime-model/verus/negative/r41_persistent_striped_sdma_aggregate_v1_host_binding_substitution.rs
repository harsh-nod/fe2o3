// Expected-negative R41 mutation: completion substitutes host generation and coherent kind.
use vstd::prelude::*;
verus! {
pub enum HostStorageKindV1 { Coherent, NonCoherent }
pub struct HostBindingV1 {
    pub generation: nat,
    pub kind: HostStorageKindV1,
}
pub open spec fn captured_host_binding_v1() -> HostBindingV1 {
    HostBindingV1 { generation: 17, kind: HostStorageKindV1::Coherent }
}
pub open spec fn mutated_host_binding_v1() -> HostBindingV1 {
    HostBindingV1 { generation: 18, kind: HostStorageKindV1::NonCoherent }
}
pub proof fn mutated_host_binding_is_exact_v1()
    ensures mutated_host_binding_v1() == captured_host_binding_v1(),
{}
}

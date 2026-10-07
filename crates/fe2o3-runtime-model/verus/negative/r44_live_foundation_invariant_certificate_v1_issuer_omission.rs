// Expected-negative R44 mutation: certificate matching omits issuer identity.
use vstd::prelude::*;
verus! {
#[derive(PartialEq, Eq)]
pub struct RegistryV1 {
    pub issuer_occurrence: nat,
    pub foundation_id: nat,
}
#[derive(PartialEq, Eq)]
pub struct CertificateV1 {
    pub issuer_occurrence: nat,
    pub foundation_id: nat,
}
pub open spec fn mutated_certificate_matches_v1(
    registry: RegistryV1,
    certificate: CertificateV1,
) -> bool {
    certificate.foundation_id == registry.foundation_id
}
pub proof fn mutated_issuer_omission_rejects_cross_registry_swap_v1(
    registry: RegistryV1,
    certificate: CertificateV1,
)
    requires
        registry.issuer_occurrence > 0,
        certificate.issuer_occurrence > 0,
        certificate.issuer_occurrence != registry.issuer_occurrence,
        certificate.foundation_id == registry.foundation_id,
    ensures !mutated_certificate_matches_v1(registry, certificate),
{}
}

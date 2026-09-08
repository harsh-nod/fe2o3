// Expected-negative R57 mutation: admission checks the full output range and
// launch/storage coordinates but omits the certificate-authority binding.

use vstd::prelude::*;

verus! {

#[derive(PartialEq, Eq)]
pub enum WriteCoverageV1 { FullExtent, Partial }

pub struct OutputBindingV1 {
    pub allocation: nat,
    pub storage: nat,
    pub allocation_generation: nat,
    pub byte_len: nat,
}

pub struct FullWriteCertificateV1 {
    pub issuer_authority: nat,
    pub binder_authority: nat,
    pub device: nat,
    pub vm: nat,
    pub queue: nat,
    pub queue_generation: nat,
    pub kernel: nat,
    pub dispatch: nat,
    pub transaction_generation: nat,
    pub allocation: nat,
    pub storage: nat,
    pub allocation_generation: nat,
    pub byte_offset: nat,
    pub byte_len: nat,
    pub coverage: WriteCoverageV1,
}

pub struct PlanV1 {
    pub fixed_binder_authority: nat,
    pub full_write_issuer_authority: nat,
    pub device: nat,
    pub vm: nat,
    pub queue: nat,
    pub queue_generation: nat,
    pub kernel: nat,
    pub dispatch: nat,
    pub transaction_generation: nat,
    pub c: OutputBindingV1,
    pub full_write: FullWriteCertificateV1,
}

pub open spec fn mutated_admitted_v1(plan: PlanV1) -> bool {
    plan.fixed_binder_authority > 0
        && plan.full_write_issuer_authority > 0
        && plan.full_write.issuer_authority > 0
        // Mutations: omitted exact issuer recognition and issuer/binder
        // distinctness. All certificate subject coordinates remain exact.
        && plan.full_write.binder_authority == plan.fixed_binder_authority
        && plan.full_write.device == plan.device
        && plan.full_write.vm == plan.vm
        && plan.full_write.queue == plan.queue
        && plan.full_write.queue_generation == plan.queue_generation
        && plan.full_write.kernel == plan.kernel
        && plan.full_write.dispatch == plan.dispatch
        && plan.full_write.transaction_generation == plan.transaction_generation
        && plan.full_write.allocation == plan.c.allocation
        && plan.full_write.storage == plan.c.storage
        && plan.full_write.allocation_generation == plan.c.allocation_generation
        && plan.full_write.byte_offset == 0
        && plan.full_write.byte_len == plan.c.byte_len
        && plan.full_write.coverage == WriteCoverageV1::FullExtent
}

pub proof fn mutated_full_write_certificate_is_rejected_v1(plan: PlanV1)
    requires mutated_admitted_v1(plan),
    ensures plan.full_write.issuer_authority == plan.full_write_issuer_authority
        && plan.full_write.issuer_authority != plan.full_write.binder_authority,
{}

} // verus!

fn main() {}

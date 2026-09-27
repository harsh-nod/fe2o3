use vstd::prelude::*;

include!("../../fe2o3-runtime/src/kfd_backend/compute_peer_gate_body.rs");

verus! {

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerComputeResultV1 { Pending, Succeeded, Failed }

impl vstd::std_specs::cmp::PartialEqSpecImpl for PeerComputeResultV1 {
    open spec fn obeys_eq_spec() -> bool { true }
    open spec fn eq_spec(&self, other: &Self) -> bool { *self == *other }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerComputeActionV1 { Invalid, Wait, FailUnpublished, ContinueNativeChecks }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PeerComputeGateV1 {
    pub owner: u64,
    pub consumer: u64,
    pub result: PeerComputeResultV1,
    pub order_complete: bool,
    pub native_failed: bool,
}

pub open spec fn exact_owner(gate: PeerComputeGateV1, owner: u64, consumer: u64) -> bool {
    owner != 0 && consumer != 0 && gate.owner == owner && gate.consumer == consumer
}

pub open spec fn resolution_valid(gate: PeerComputeGateV1, owner: u64, consumer: u64,
    result: PeerComputeResultV1, order_complete: bool) -> bool {
    exact_owner(gate, owner, consumer)
        && (gate.order_complete ==> order_complete)
        && (gate.result != PeerComputeResultV1::Pending ==> gate.result == result)
}

pub open spec fn predecessor_access(gate: PeerComputeGateV1, owner: u64, consumer: u64) -> bool {
    exact_owner(gate, owner, consumer)
        && (gate.result != PeerComputeResultV1::Succeeded || !gate.order_complete)
}

pub open spec fn continuation_ready(gate: PeerComputeGateV1, consumer: u64, native_success: bool) -> bool {
    exact_owner(gate, gate.owner, consumer) && gate.order_complete && !gate.native_failed
        && gate.result == PeerComputeResultV1::Succeeded && native_success
}

pub open spec fn failure_ready(gate: PeerComputeGateV1, consumer: u64, native_order: bool) -> bool {
    exact_owner(gate, gate.owner, consumer) && gate.order_complete && native_order
        && (gate.native_failed || gate.result == PeerComputeResultV1::Failed)
}

impl PeerComputeGateV1 {
    pub fn owns(self, owner: u64, consumer: u64) -> (out: bool)
        ensures out == exact_owner(self, owner, consumer),
    {
        let gate = self;
        peer_compute_gate_owns_body!(verus_exec_expr, gate, owner, consumer)
    }

    pub fn permits_predecessor_access(self, owner: u64, consumer: u64) -> (out: bool)
        ensures out == predecessor_access(self, owner, consumer),
    {
        let gate = self;
        peer_compute_gate_access_body!(verus_exec_expr, gate, owner, consumer)
    }

    pub fn resolve(self, owner: u64, consumer: u64, result: PeerComputeResultV1,
        order_complete: bool) -> (out: Result<Self, ()>)
        ensures out == if resolution_valid(self, owner, consumer, result, order_complete) {
            Ok(PeerComputeGateV1 { result, order_complete, ..self })
        } else { Err(()) },
    {
        let gate = self;
        peer_compute_gate_resolve_body!(verus_exec_expr, gate, owner, consumer, result, order_complete)
    }

    pub fn action(self, consumer: u64, native_success: bool, native_order: bool) -> (out: PeerComputeActionV1)
        ensures (out == PeerComputeActionV1::Invalid) <==> !exact_owner(self, self.owner, consumer),
            (out == PeerComputeActionV1::FailUnpublished) <==> failure_ready(self, consumer, native_order),
            (out == PeerComputeActionV1::ContinueNativeChecks) <==> continuation_ready(self, consumer, native_success),
            (out == PeerComputeActionV1::Wait) <==> exact_owner(self, self.owner, consumer)
                && !failure_ready(self, consumer, native_order) && !continuation_ready(self, consumer, native_success),
    {
        let gate = self;
        peer_compute_gate_action_body!(verus_exec_expr, gate, consumer, native_success, native_order)
    }
}

pub fn access_excludes_publication_witness(gate: PeerComputeGateV1, owner: u64, consumer: u64,
    native_success: bool, native_order: bool)
{
    let allowed = gate.permits_predecessor_access(owner, consumer);
    let action = gate.action(consumer, native_success, native_order);
    assert(allowed ==> action != PeerComputeActionV1::ContinueNativeChecks);
}

pub fn settled_access_stays_revoked_witness(native_failed: bool, result: PeerComputeResultV1,
    order_complete: bool) -> (out: bool)
    ensures !out,
{
    let gate = PeerComputeGateV1 { owner: 7, consumer: 19, result: PeerComputeResultV1::Succeeded,
        order_complete: true, native_failed };
    match gate.resolve(7, 19, result, order_complete) {
        Ok(updated) => updated.permits_predecessor_access(7, 19),
        Err(()) => gate.permits_predecessor_access(7, 19),
    }
}

pub fn unresolved_access_survives_native_failure_witness(result: PeerComputeResultV1,
    order_complete: bool) -> (out: bool)
    requires result != PeerComputeResultV1::Succeeded || !order_complete,
    ensures out,
{
    let gate = PeerComputeGateV1 { owner: 7, consumer: 19, result, order_complete, native_failed: true };
    gate.permits_predecessor_access(7, 19)
}

pub fn waiting_witness(native_success: bool, native_order: bool) -> (out: PeerComputeActionV1)
    ensures out == PeerComputeActionV1::Wait,
{
    let gate = PeerComputeGateV1 { owner: 7, consumer: 19, result: PeerComputeResultV1::Pending,
        order_complete: true, native_failed: false };
    gate.action(19, native_success, native_order)
}

pub fn failed_ordering_witness(native_order: bool, external_order: bool) -> (out: PeerComputeActionV1)
    ensures out == if native_order && external_order { PeerComputeActionV1::FailUnpublished }
        else { PeerComputeActionV1::Wait },
{
    let gate = PeerComputeGateV1 { owner: 7, consumer: 19, result: PeerComputeResultV1::Failed,
        order_complete: external_order, native_failed: false };
    gate.action(19, true, native_order)
}

pub fn failed_cannot_reopen_witness(native_failed: bool) -> (out: PeerComputeActionV1)
    ensures out == PeerComputeActionV1::FailUnpublished,
{
    let gate = PeerComputeGateV1 { owner: 7, consumer: 19,
        result: if native_failed { PeerComputeResultV1::Pending } else { PeerComputeResultV1::Failed },
        order_complete: true, native_failed };
    let updated = gate.resolve(7, 19, PeerComputeResultV1::Succeeded, true);
    match updated {
        Ok(gate) => gate.action(19, true, true),
        Err(()) => gate.action(19, true, true),
    }
}

pub fn success_continues_native_checks_witness(native_success: bool, external_order: bool) -> (out: PeerComputeActionV1)
    ensures out == if native_success && external_order { PeerComputeActionV1::ContinueNativeChecks }
        else { PeerComputeActionV1::Wait },
{
    let waiting = PeerComputeGateV1 { owner: 7, consumer: 19, result: PeerComputeResultV1::Pending,
        order_complete: false, native_failed: false };
    let resolved = waiting.resolve(7, 19, PeerComputeResultV1::Succeeded, external_order);
    match resolved {
        Ok(gate) => gate.action(19, native_success, false),
        Err(()) => { assert(false); PeerComputeActionV1::Invalid },
    }
}

pub fn rejected_resolution_preserves_waiting_witness(owner: u64, consumer: u64) -> (out: PeerComputeActionV1)
    requires owner != 7 || consumer != 19,
    ensures out == PeerComputeActionV1::Wait,
{
    let gate = PeerComputeGateV1 { owner: 7, consumer: 19, result: PeerComputeResultV1::Pending,
        order_complete: false, native_failed: false };
    let rejected = gate.resolve(owner, consumer, PeerComputeResultV1::Succeeded, true);
    assert(rejected.is_err());
    gate.action(19, true, true)
}

}

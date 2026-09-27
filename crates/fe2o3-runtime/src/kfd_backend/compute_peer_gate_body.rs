// Shared executable gate decisions; native ownership and backing checks remain in the adapter.
macro_rules! peer_compute_gate_owns_body {
    ($syntax:ident, $gate:ident, $query_owner:ident, $query_consumer:ident) => {
        $syntax!({
            $query_owner != 0 && $query_consumer != 0
                && $gate.owner == $query_owner && $gate.consumer == $query_consumer
        })
    };
}

macro_rules! peer_compute_gate_access_body {
    ($syntax:ident, $gate:ident, $owner:ident, $consumer:ident) => {
        $syntax!({
            $gate.owns($owner, $consumer)
                && ($gate.result != PeerComputeResultV1::Succeeded || !$gate.order_complete)
        })
    };
}

macro_rules! peer_compute_gate_resolve_body {
    ($syntax:ident, $gate:ident, $owner:ident, $consumer:ident, $result:ident, $ordered:ident) => {
        $syntax!({
            if $owner == 0 || $consumer == 0 || $gate.owner != $owner || $gate.consumer != $consumer
                || $gate.order_complete && !$ordered
                || $gate.result != PeerComputeResultV1::Pending && $gate.result != $result
            {
                return Err(());
            }
            Ok(PeerComputeGateV1 { result: $result, order_complete: $ordered, ..$gate })
        })
    };
}

macro_rules! peer_compute_gate_action_body {
    ($syntax:ident, $gate:ident, $consumer:ident, $native_success:ident, $native_order:ident) => {
        $syntax!({
            if $gate.owner == 0 || $consumer == 0 || $gate.consumer != $consumer {
                return PeerComputeActionV1::Invalid;
            }
            if !$gate.order_complete {
                return PeerComputeActionV1::Wait;
            }
            if $gate.native_failed || $gate.result == PeerComputeResultV1::Failed {
                return if $native_order { PeerComputeActionV1::FailUnpublished }
                    else { PeerComputeActionV1::Wait };
            }
            if $gate.result == PeerComputeResultV1::Succeeded && $native_success {
                PeerComputeActionV1::ContinueNativeChecks
            } else {
                PeerComputeActionV1::Wait
            }
        })
    };
}

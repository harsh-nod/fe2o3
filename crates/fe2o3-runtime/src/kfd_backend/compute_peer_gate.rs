//! Private external dependency gate on the genuine child compute owner.

use super::*;

include!("compute_peer_gate_body.rs");
macro_rules! gate_rust_expr {
    ($body:expr) => {
        $body
    };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PeerComputeResultV1 {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PeerComputeActionV1 {
    Invalid,
    Wait,
    FailUnpublished,
    ContinueNativeChecks,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PeerComputeGateV1 {
    owner: u64,
    consumer: u64,
    result: PeerComputeResultV1,
    order_complete: bool,
    native_failed: bool,
}

impl PeerComputeGateV1 {
    pub(super) fn owns(self, owner: u64, consumer: u64) -> bool {
        let gate = self;
        peer_compute_gate_owns_body!(gate_rust_expr, gate, owner, consumer)
    }

    pub(super) fn permits_predecessor_access(self, owner: u64, consumer: u64) -> bool {
        let gate = self;
        peer_compute_gate_access_body!(gate_rust_expr, gate, owner, consumer)
    }

    pub(super) fn waiting(owner: u64, consumer: u64, order_complete: bool) -> Self {
        Self {
            owner,
            consumer,
            result: PeerComputeResultV1::Pending,
            order_complete,
            native_failed: false,
        }
    }

    pub(super) fn resolve(
        self,
        owner: u64,
        consumer: u64,
        result: PeerComputeResultV1,
        order_complete: bool,
    ) -> Result<Self, ()> {
        let gate = self;
        peer_compute_gate_resolve_body!(
            gate_rust_expr,
            gate,
            owner,
            consumer,
            result,
            order_complete
        )
    }

    pub(super) fn action(
        self,
        consumer: u64,
        native_success: bool,
        native_order: bool,
    ) -> PeerComputeActionV1 {
        let gate = self;
        peer_compute_gate_action_body!(gate_rust_expr, gate, consumer, native_success, native_order)
    }

    fn failed(self) -> bool {
        self.native_failed || self.result == PeerComputeResultV1::Failed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_compute_access_gate_exhaustive_identity_and_lifetime() {
        for result in [
            PeerComputeResultV1::Pending,
            PeerComputeResultV1::Succeeded,
            PeerComputeResultV1::Failed,
        ] {
            for order_complete in [false, true] {
                for native_failed in [false, true] {
                    let gate = PeerComputeGateV1 {
                        owner: 7,
                        consumer: 19,
                        result,
                        order_complete,
                        native_failed,
                    };
                    for owner in [0, 7, 8] {
                        for consumer in [0, 19, 20] {
                            let exact = owner == 7 && consumer == 19;
                            let allowed = exact
                                && (result != PeerComputeResultV1::Succeeded || !order_complete);
                            assert_eq!(gate.owns(owner, consumer), exact);
                            assert_eq!(gate.permits_predecessor_access(owner, consumer), allowed);
                            for native_success in [false, true] {
                                for native_order in [false, true] {
                                    if allowed {
                                        assert_ne!(
                                            gate.action(consumer, native_success, native_order),
                                            PeerComputeActionV1::ContinueNativeChecks
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) enum PeerComputeStepV1 {
    Continue(PendingComputeSubmissionV1),
    Observed(BackendPollV1),
}

impl PendingComputeSubmissionV1 {
    pub(super) fn peer_gate_allows_native_checks_v1(&self) -> bool {
        self.peer_gate.is_none_or(|gate| {
            gate.action(
                self.id,
                self.explicit_dependency_cursor == self.explicit_success_dependencies.len(),
                true,
            ) == PeerComputeActionV1::ContinueNativeChecks
        })
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn exact_submission_quiescent_v1(&self, id: u64) -> bool {
        self.submissions
            .get(&id)
            .is_some_and(|record| record.status != BackendPollV1::Pending)
            && !self.pending_compute.contains_key(&id)
            && !self.active_sdma.contains_key(&id)
            && self.active_compute_lane_v1(id).is_none()
    }

    pub(super) fn observe_peer_compute_gate_v1(
        &mut self,
        pending: PendingComputeSubmissionV1,
    ) -> Result<PeerComputeStepV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.observe_peer_compute_gate_with_v1(pending, Self::observe_peer_compute_gate_inner_v1)
    }

    pub(super) fn observe_peer_compute_gate_with_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
        observe: impl FnOnce(
            &mut Self,
            &mut PendingComputeSubmissionV1,
        ) -> Result<
            PeerComputeActionV1,
            RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>,
        >,
    ) -> Result<PeerComputeStepV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if pending.peer_gate.is_none() {
            return Ok(PeerComputeStepV1::Continue(pending));
        }
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(self, &mut pending)));
        let action = match result {
            Ok(Ok(action)) => action,
            Ok(Err(failure)) => {
                self.pending_compute.insert(pending.id, pending);
                if matches!(failure, RuntimeBackendFailureV1::Terminal(_)) {
                    self.poison_terminal_v1();
                }
                return Err(failure);
            }
            Err(payload) => {
                self.pending_compute.insert(pending.id, pending);
                let _ = self
                    .terminal_error("KFD peer gate observation unwound with compute custody live");
                std::panic::resume_unwind(payload);
            }
        };
        match action {
            PeerComputeActionV1::ContinueNativeChecks => Ok(PeerComputeStepV1::Continue(pending)),
            PeerComputeActionV1::FailUnpublished => Ok(PeerComputeStepV1::Observed(
                self.settle_unpublished_compute_v1(pending, BackendPollV1::Failed { code: -1 }),
            )),
            PeerComputeActionV1::Wait => {
                self.pending_compute.insert(pending.id, pending);
                Ok(PeerComputeStepV1::Observed(BackendPollV1::Pending))
            }
            PeerComputeActionV1::Invalid => {
                self.pending_compute.insert(pending.id, pending);
                Err(self.terminal_error("KFD compute peer gate lost its exact consumer identity"))
            }
        }
    }

    fn observe_peer_compute_gate_inner_v1(
        &mut self,
        pending: &mut PendingComputeSubmissionV1,
    ) -> Result<PeerComputeActionV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut gate = pending
            .peer_gate
            .expect("peer observation retains its gate");
        if gate.action(pending.id, false, false) == PeerComputeActionV1::Invalid {
            return Ok(PeerComputeActionV1::Invalid);
        }
        // A failed consumer need not wait for unrelated successful inputs, but
        // still owns its position behind both external and native stream prefixes.
        while !gate.failed() {
            let Some(dependency) = pending
                .explicit_success_dependencies
                .get(pending.explicit_dependency_cursor)
                .copied()
            else {
                break;
            };
            match self.poll_v1(dependency) {
                Ok(BackendPollV1::Succeeded) => pending.explicit_dependency_cursor += 1,
                Ok(BackendPollV1::Failed { .. }) => gate.native_failed = true,
                Ok(BackendPollV1::Pending) => break,
                Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    if self.exact_submission_quiescent_v1(dependency) {
                        gate.native_failed = true;
                    } else {
                        break;
                    }
                }
                Err(RuntimeBackendFailureV1::Rejected(error)) => {
                    return Err(self.terminal_error(format!(
                        "KFD peer-gated compute retained a rejected dependency: {error}"
                    )));
                }
                Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    return Err(failure);
                }
            }
            pending.peer_gate = Some(gate);
        }
        pending.peer_gate = Some(gate);
        if gate.action(
            pending.id,
            pending.explicit_dependency_cursor == pending.explicit_success_dependencies.len(),
            false,
        ) == PeerComputeActionV1::ContinueNativeChecks
        {
            return Ok(PeerComputeActionV1::ContinueNativeChecks);
        }
        let mut ordered = pending
            .ordered_predecessor
            .is_none_or(|id| self.exact_submission_quiescent_v1(id));
        if !ordered {
            let predecessor = pending
                .ordered_predecessor
                .expect("incomplete native prefix has an identity");
            match self.poll_v1(predecessor) {
                Ok(_) | Err(RuntimeBackendFailureV1::Quiescent(_)) => {
                    ordered = self.exact_submission_quiescent_v1(predecessor);
                }
                Err(RuntimeBackendFailureV1::Rejected(error)) => {
                    return Err(self.terminal_error(format!(
                        "KFD peer-gated compute retained a rejected prefix: {error}"
                    )));
                }
                Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => {
                    return Err(failure);
                }
            }
        }
        Ok(gate.action(
            pending.id,
            pending.explicit_dependency_cursor == pending.explicit_success_dependencies.len(),
            ordered,
        ))
    }
}

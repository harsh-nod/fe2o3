//! Domain-separated state-transition event contracts.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, HostEventIdentityV1, HostRequestIdentityV1, HostStateIdentityV1,
    check_preimage_bound,
};
use crate::state::can_transition;
use crate::{
    AdmitEventIdV1, CompileEventIdV1, DispatchEventIdV1, FlowScopeIdV1, HostContractErrorV1,
    HostOperationStateV1, LoadEventIdV1, OperationIdV1, OperationPhaseV1, WaitEventIdV1,
};

/// Hard maximum event count in one complete V1 event batch.
pub const MAX_EVENT_BATCH_V1: usize = 512;

/// One immutable event binding an exact operation-state revision.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostEventV1 {
    identity: HostEventIdentityV1,
    scope_id: FlowScopeIdV1,
    operation_id: OperationIdV1,
    request_identity: HostRequestIdentityV1,
    revision: u32,
    predecessor_state_identity: Option<HostStateIdentityV1>,
    state_identity: HostStateIdentityV1,
    phase: OperationPhaseV1,
}

impl HostEventV1 {
    /// Creates an event from one exact state revision.
    pub fn new(
        identity: HostEventIdentityV1,
        state: &HostOperationStateV1,
    ) -> Result<Self, HostContractErrorV1> {
        if identity.flow() != state.identity().flow() {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::Flow,
            });
        }
        Ok(Self {
            identity,
            scope_id: state.scope_id(),
            operation_id: state.operation_id(),
            request_identity: state.request_identity(),
            revision: state.revision(),
            predecessor_state_identity: state.predecessor_identity(),
            state_identity: state.identity(),
            phase: state.phase(),
        })
    }

    /// Returns the caller-supplied event commitment.
    pub const fn identity(self) -> HostEventIdentityV1 {
        self.identity
    }

    /// Returns the parallel-operation namespace.
    pub const fn scope_id(self) -> FlowScopeIdV1 {
        self.scope_id
    }

    /// Returns the logical operation identity.
    pub const fn operation_id(self) -> OperationIdV1 {
        self.operation_id
    }

    /// Returns the exact request commitment.
    pub const fn request_identity(self) -> HostRequestIdentityV1 {
        self.request_identity
    }

    /// Returns the zero-based state revision.
    pub const fn revision(self) -> u32 {
        self.revision
    }

    /// Returns the exact predecessor state commitment.
    pub const fn predecessor_state_identity(self) -> Option<HostStateIdentityV1> {
        self.predecessor_state_identity
    }

    /// Returns the exact state commitment observed by this event.
    pub const fn state_identity(self) -> HostStateIdentityV1 {
        self.state_identity
    }

    /// Returns the observed descriptive phase.
    pub const fn phase(self) -> OperationPhaseV1 {
        self.phase
    }

    /// Checks that this event is an exact projection of the supplied state.
    pub fn validate_state(&self, state: &HostOperationStateV1) -> Result<(), HostContractErrorV1> {
        if self.scope_id != state.scope_id()
            || self.operation_id != state.operation_id()
            || self.request_identity != state.request_identity()
            || self.revision != state.revision()
            || self.predecessor_state_identity != state.predecessor_identity()
            || self.state_identity != state.identity()
            || self.phase != state.phase()
        {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::StatePredecessor,
            });
        }
        Ok(())
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(event_domain(self.identity));
        encoder.digest(self.scope_id.digest());
        encoder.digest(self.operation_id.digest());
        encoder.u8(self.request_identity.flow() as u8);
        encoder.digest(self.request_identity.digest());
        encoder.u32(self.revision);
        encoder.optional_digest(
            self.predecessor_state_identity
                .map(HostStateIdentityV1::digest),
        );
        encoder.digest(self.state_identity.digest());
        encode_phase(self.phase, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}

/// Bounded complete traces for one scope, with arbitrary interleaving.
///
/// Each operation must begin at revision zero and remain contiguous within the
/// batch. Different operations may be interleaved in any order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostEventBatchV1 {
    scope_id: FlowScopeIdV1,
    events: Vec<HostEventV1>,
}

impl HostEventBatchV1 {
    /// Creates a nonempty batch and validates every per-operation chain.
    pub fn new(
        scope_id: FlowScopeIdV1,
        events: Vec<HostEventV1>,
    ) -> Result<Self, HostContractErrorV1> {
        if events.is_empty() {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::EventBatch,
            });
        }
        if events.len() > MAX_EVENT_BATCH_V1 {
            return Err(HostContractErrorV1::TooManyItems {
                field: ContractFieldV1::EventBatch,
                actual: events.len(),
                maximum: MAX_EVENT_BATCH_V1,
            });
        }
        for (index, event) in events.iter().enumerate() {
            if event.scope_id != scope_id {
                return Err(HostContractErrorV1::Mismatch {
                    field: ContractFieldV1::Scope,
                });
            }
            if events[..index]
                .iter()
                .any(|earlier| earlier.identity == event.identity)
            {
                return Err(HostContractErrorV1::Duplicate {
                    field: ContractFieldV1::EventBatch,
                });
            }
            if events[..index]
                .iter()
                .any(|earlier| earlier.state_identity == event.state_identity)
            {
                return Err(HostContractErrorV1::Duplicate {
                    field: ContractFieldV1::StateIdentity,
                });
            }
            let previous = events[..index]
                .iter()
                .rev()
                .find(|earlier| earlier.operation_id == event.operation_id);
            validate_batch_link(previous, event)?;
        }
        Ok(Self { scope_id, events })
    }

    /// Returns the common parallel-operation namespace.
    pub const fn scope_id(&self) -> FlowScopeIdV1 {
        self.scope_id
    }

    /// Returns events in observation order.
    pub fn events(&self) -> &[HostEventV1] {
        &self.events
    }
}

fn validate_batch_link(
    previous: Option<&HostEventV1>,
    current: &HostEventV1,
) -> Result<(), HostContractErrorV1> {
    match previous {
        None => {
            if current.revision != 0
                || current.predecessor_state_identity.is_some()
                || current.phase != OperationPhaseV1::Requested
            {
                return Err(HostContractErrorV1::InvalidInitialState);
            }
        }
        Some(previous) => {
            if previous.request_identity != current.request_identity {
                return Err(HostContractErrorV1::Mismatch {
                    field: ContractFieldV1::RequestIdentity,
                });
            }
            if current.revision
                != previous.revision.checked_add(1).ok_or(
                    HostContractErrorV1::ArithmeticOverflow {
                        field: ContractFieldV1::StateRevision,
                    },
                )?
            {
                return Err(HostContractErrorV1::Mismatch {
                    field: ContractFieldV1::StateRevision,
                });
            }
            if current.predecessor_state_identity != Some(previous.state_identity) {
                return Err(HostContractErrorV1::Mismatch {
                    field: ContractFieldV1::StatePredecessor,
                });
            }
            if previous.phase.is_terminal() {
                return Err(HostContractErrorV1::TerminalStateTransition);
            }
            if !can_transition(previous.phase, current.phase) {
                return Err(HostContractErrorV1::InvalidStateTransition);
            }
        }
    }
    Ok(())
}

fn encode_phase(phase: OperationPhaseV1, encoder: &mut EncoderV1) {
    match phase {
        OperationPhaseV1::Requested => encoder.u8(1),
        OperationPhaseV1::Pending => encoder.u8(2),
        OperationPhaseV1::Active => encoder.u8(3),
        OperationPhaseV1::Succeeded(result) => {
            encoder.u8(4);
            encoder.digest(result.digest());
        }
        OperationPhaseV1::Rejected(result) => {
            encoder.u8(5);
            encoder.digest(result.digest());
        }
        OperationPhaseV1::Failed(result) => {
            encoder.u8(6);
            encoder.digest(result.digest());
        }
        OperationPhaseV1::Cancelled => encoder.u8(7),
    }
}

const fn event_domain(identity: HostEventIdentityV1) -> &'static [u8] {
    match identity {
        HostEventIdentityV1::Compile(_) => CompileEventIdV1::DOMAIN_V1,
        HostEventIdentityV1::Admit(_) => AdmitEventIdV1::DOMAIN_V1,
        HostEventIdentityV1::Load(_) => LoadEventIdV1::DOMAIN_V1,
        HostEventIdentityV1::Dispatch(_) => DispatchEventIdV1::DOMAIN_V1,
        HostEventIdentityV1::Wait(_) => WaitEventIdV1::DOMAIN_V1,
    }
}

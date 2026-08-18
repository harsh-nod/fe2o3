//! Per-operation state contracts without a global serial cursor.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, HostRequestIdentityV1, HostResultIdentityV1, HostResultReferenceV1,
    HostStateIdentityV1, OperationResultClassV1, check_preimage_bound,
};
use crate::{
    AdmitStateIdV1, CompileStateIdV1, DispatchStateIdV1, FlowScopeIdV1, LoadStateIdV1,
    WaitStateIdV1,
};
use crate::{HostContractErrorV1, OperationContextV1, OperationIdV1};

/// Descriptive phase of one host operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationPhaseV1 {
    /// Unique revision-zero phase before implementation acceptance.
    Requested,
    /// Accepted for later processing but not observed active.
    Pending,
    /// Observed active by an implementation-specific adapter.
    Active,
    /// The host operation produced a successful result description.
    Succeeded(HostResultIdentityV1),
    /// The host operation produced a rejection result.
    Rejected(HostResultIdentityV1),
    /// The host operation produced a failure result.
    Failed(HostResultIdentityV1),
    /// The host operation was cancelled before producing a result.
    Cancelled,
}

impl OperationPhaseV1 {
    /// Reports whether no later V1 state may follow this phase.
    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded(_) | Self::Rejected(_) | Self::Failed(_) | Self::Cancelled
        )
    }

    const fn result_class(self) -> Option<OperationResultClassV1> {
        match self {
            Self::Succeeded(_) => Some(OperationResultClassV1::Succeeded),
            Self::Rejected(_) => Some(OperationResultClassV1::Rejected),
            Self::Failed(_) => Some(OperationResultClassV1::Failed),
            Self::Requested | Self::Pending | Self::Active | Self::Cancelled => None,
        }
    }

    const fn result_identity(self) -> Option<HostResultIdentityV1> {
        match self {
            Self::Succeeded(identity) | Self::Rejected(identity) | Self::Failed(identity) => {
                Some(identity)
            }
            Self::Requested | Self::Pending | Self::Active | Self::Cancelled => None,
        }
    }
}

/// One immutable revision in a per-operation V1 state chain.
///
/// Different operation identities in one scope have independent revision
/// chains and may progress in any interleaving.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostOperationStateV1 {
    identity: HostStateIdentityV1,
    scope_id: FlowScopeIdV1,
    operation_id: OperationIdV1,
    request_identity: HostRequestIdentityV1,
    revision: u32,
    predecessor_identity: Option<HostStateIdentityV1>,
    phase: OperationPhaseV1,
}

impl HostOperationStateV1 {
    /// Creates the unique revision-zero `Requested` state for an operation.
    pub fn initial(
        identity: HostStateIdentityV1,
        context: &OperationContextV1,
        request_identity: HostRequestIdentityV1,
    ) -> Result<Self, HostContractErrorV1> {
        if identity.flow() != request_identity.flow() {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::Flow,
            });
        }
        Ok(Self {
            identity,
            scope_id: context.scope_id(),
            operation_id: context.operation_id(),
            request_identity,
            revision: 0,
            predecessor_identity: None,
            phase: OperationPhaseV1::Requested,
        })
    }

    /// Creates the next state and checks exact predecessor and flow binding.
    pub fn transition(
        identity: HostStateIdentityV1,
        previous: &Self,
        phase: OperationPhaseV1,
    ) -> Result<Self, HostContractErrorV1> {
        if previous.phase.is_terminal() {
            return Err(HostContractErrorV1::TerminalStateTransition);
        }
        if identity.flow() != previous.identity.flow() {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::Flow,
            });
        }
        if identity == previous.identity {
            return Err(HostContractErrorV1::Duplicate {
                field: ContractFieldV1::StateIdentity,
            });
        }
        if let Some(result_identity) = phase.result_identity()
            && result_identity.flow() != previous.request_identity.flow()
        {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::TerminalResult,
            });
        }
        if !can_transition(previous.phase, phase) {
            return Err(HostContractErrorV1::InvalidStateTransition);
        }
        let revision =
            previous
                .revision
                .checked_add(1)
                .ok_or(HostContractErrorV1::ArithmeticOverflow {
                    field: ContractFieldV1::StateRevision,
                })?;
        Ok(Self {
            identity,
            scope_id: previous.scope_id,
            operation_id: previous.operation_id,
            request_identity: previous.request_identity,
            revision,
            predecessor_identity: Some(previous.identity),
            phase,
        })
    }

    /// Returns the caller-supplied state commitment.
    pub const fn identity(self) -> HostStateIdentityV1 {
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

    /// Returns the exact predecessor state commitment when revision is nonzero.
    pub const fn predecessor_identity(self) -> Option<HostStateIdentityV1> {
        self.predecessor_identity
    }

    /// Returns the descriptive operation phase.
    pub const fn phase(self) -> OperationPhaseV1 {
        self.phase
    }

    /// Checks that a terminal state names the exact result and disposition.
    pub fn validate_terminal_result(
        &self,
        result: HostResultReferenceV1,
    ) -> Result<(), HostContractErrorV1> {
        if result.request_identity() != self.request_identity {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::RequestIdentity,
            });
        }
        if self.phase.result_identity() != Some(result.identity())
            || self.phase.result_class() != Some(result.class())
        {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::TerminalResult,
            });
        }
        Ok(())
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(state_domain(self.identity));
        encoder.digest(self.scope_id.digest());
        encoder.digest(self.operation_id.digest());
        encoder.u8(self.request_identity.flow() as u8);
        encoder.digest(self.request_identity.digest());
        encoder.u32(self.revision);
        encoder.optional_digest(self.predecessor_identity.map(HostStateIdentityV1::digest));
        encode_phase(self.phase, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}

pub(crate) const fn can_transition(current: OperationPhaseV1, next: OperationPhaseV1) -> bool {
    matches!(
        (current, next),
        (
            OperationPhaseV1::Requested,
            OperationPhaseV1::Pending
                | OperationPhaseV1::Active
                | OperationPhaseV1::Succeeded(_)
                | OperationPhaseV1::Rejected(_)
                | OperationPhaseV1::Failed(_)
                | OperationPhaseV1::Cancelled
        ) | (
            OperationPhaseV1::Pending,
            OperationPhaseV1::Active
                | OperationPhaseV1::Succeeded(_)
                | OperationPhaseV1::Rejected(_)
                | OperationPhaseV1::Failed(_)
                | OperationPhaseV1::Cancelled
        ) | (
            OperationPhaseV1::Active,
            OperationPhaseV1::Succeeded(_)
                | OperationPhaseV1::Rejected(_)
                | OperationPhaseV1::Failed(_)
                | OperationPhaseV1::Cancelled
        )
    )
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

const fn state_domain(identity: HostStateIdentityV1) -> &'static [u8] {
    match identity {
        HostStateIdentityV1::Compile(_) => CompileStateIdV1::DOMAIN_V1,
        HostStateIdentityV1::Admit(_) => AdmitStateIdV1::DOMAIN_V1,
        HostStateIdentityV1::Load(_) => LoadStateIdV1::DOMAIN_V1,
        HostStateIdentityV1::Dispatch(_) => DispatchStateIdV1::DOMAIN_V1,
        HostStateIdentityV1::Wait(_) => WaitStateIdV1::DOMAIN_V1,
    }
}

//! Private dynamic-identity bookkeeping, independent of serialized protocols.
//!
//! Runtime supplies existing invocation/site owners and actual call-stack
//! transitions for opt-in in-process observations. This does not identify a run
//! across transcript owners, authenticate source, or enable resource/CLI facts.
//! No serialized capability or wire/domain allocation is made here.
//!
//! All state is fixed-size. Activation allocation is monotone per invocation;
//! attempts are monotone per activation, independent of static site and stack
//! depth. A retained operation token survives suspension/resumption unchanged.
//! Every fallible transition validates completely before changing state.

use std::num::NonZeroU64;

use crate::{
    SimulationDebugSiteV1, SimulationInvocationV1, SimulationLimitsErrorV1, SimulationLimitsV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameActivationId(NonZeroU64);

impl FrameActivationId {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationAttemptId(NonZeroU64);

impl OperationAttemptId {
    pub(crate) const fn get(self) -> u64 {
        self.0.get()
    }
}

/// A run-local activation key. Full existing invocation coordinates participate
/// in equality; the numeric activation alone is deliberately not a global key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FrameIdentity {
    invocation: SimulationInvocationV1,
    activation: FrameActivationId,
}

impl FrameIdentity {
    pub(crate) const fn invocation(self) -> SimulationInvocationV1 {
        self.invocation
    }

    pub(crate) const fn activation(self) -> FrameActivationId {
        self.activation
    }
}

/// One attempted operation, not a completion claim or a per-site iteration rank.
/// Static-site equality is checked as well as the dynamic token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationIdentity {
    frame: FrameIdentity,
    attempt: OperationAttemptId,
    site: SimulationDebugSiteV1,
}

impl OperationIdentity {
    pub(crate) const fn frame(self) -> FrameIdentity {
        self.frame
    }

    pub(crate) const fn attempt(self) -> OperationAttemptId {
        self.attempt
    }

    pub(crate) const fn site(self) -> SimulationDebugSiteV1 {
        self.site
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IdentityStateError {
    InvalidSimulationLimits(SimulationLimitsErrorV1),
    CounterOverflow,
    ActivationLimit { limit: u64 },
    AttemptLimit { limit: u64 },
    WrongInvocation,
    WrongActivation,
    WrongFunction,
    OperationMismatch,
    NoPendingOperation,
    OperationAlreadyPending,
    OperationSuspended,
    OperationNotSuspended,
    AlreadySuspended,
    FrameRetired,
    FrameNotRetired,
    CannotResetRoot,
}

/// No Clone/Copy: copying a live allocator would reuse issued activations.
#[derive(Debug)]
pub(crate) struct InvocationIdentityState {
    invocation: SimulationInvocationV1,
    last_activation: FrameActivationId,
    activation_limit: u64,
    attempt_limit: u64,
}

impl InvocationIdentityState {
    /// Mint the only root activation together with its move-only allocator.
    ///
    /// Both counter ceilings derive from existing validated simulation limits,
    /// not a new debugger resource policy. One extra activation accounts for
    /// the root; one extra attempt permits the pre-operation observation that
    /// precedes a final step-limit refusal. Execution must still charge steps.
    pub(crate) fn new(
        invocation: SimulationInvocationV1,
        function_ordinal: usize,
        limits: SimulationLimitsV1,
    ) -> Result<(Self, FrameIdentityState), IdentityStateError> {
        let limits = limits
            .validate()
            .map_err(IdentityStateError::InvalidSimulationLimits)?;
        let ceiling = limits
            .max_steps
            .checked_add(1)
            .ok_or(IdentityStateError::CounterOverflow)?;
        let root = FrameActivationId(NonZeroU64::MIN);
        let state = Self {
            invocation,
            last_activation: root,
            activation_limit: ceiling,
            attempt_limit: ceiling,
        };
        let frame = state.frame(root, function_ordinal);
        Ok((state, frame))
    }

    #[cfg(test)]
    pub(crate) const fn last_activation(&self) -> FrameActivationId {
        self.last_activation
    }

    /// Allocate a fresh helper activation. The runtime owns whether an actual
    /// helper entry is valid; this leaf neither stores nor reconstructs a stack.
    pub(crate) fn enter_frame(
        &mut self,
        function_ordinal: usize,
    ) -> Result<FrameIdentityState, IdentityStateError> {
        let next = self.next_activation()?;
        let frame = self.frame(next, function_ordinal);
        self.last_activation = next;
        Ok(frame)
    }

    /// Reuse storage only after its old activation was retired. The root cannot
    /// be replaced within an invocation. Fresh runs create fresh owners instead.
    pub(crate) fn reset_frame(
        &mut self,
        slot: &mut FrameIdentityState,
        function_ordinal: usize,
    ) -> Result<(), IdentityStateError> {
        if slot.identity.invocation != self.invocation {
            return Err(IdentityStateError::WrongInvocation);
        }
        if slot.identity.activation.get() == 1 {
            return Err(IdentityStateError::CannotResetRoot);
        }
        if slot.phase != FramePhase::Retired {
            return Err(IdentityStateError::FrameNotRetired);
        }
        let next = self.next_activation()?;
        let replacement = self.frame(next, function_ordinal);
        // No fallible work remains: commit allocator and slot together.
        self.last_activation = next;
        *slot = replacement;
        Ok(())
    }

    fn next_activation(&self) -> Result<FrameActivationId, IdentityStateError> {
        let next = self
            .last_activation
            .get()
            .checked_add(1)
            .ok_or(IdentityStateError::CounterOverflow)?;
        if next > self.activation_limit {
            return Err(IdentityStateError::ActivationLimit {
                limit: self.activation_limit,
            });
        }
        NonZeroU64::new(next)
            .map(FrameActivationId)
            .ok_or(IdentityStateError::CounterOverflow)
    }

    fn frame(&self, activation: FrameActivationId, function_ordinal: usize) -> FrameIdentityState {
        FrameIdentityState {
            identity: FrameIdentity {
                invocation: self.invocation,
                activation,
            },
            function_ordinal,
            attempt_limit: self.attempt_limit,
            last_attempt: 0,
            phase: FramePhase::Ready,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingOperation {
    attempt: OperationAttemptId,
    site: SimulationDebugSiteV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FramePhase {
    Ready,
    Running(PendingOperation),
    Suspended(PendingOperation),
    Retired,
}

/// Move-only state for one reusable runtime frame slot. Immutable issued keys
/// may be copied, but live state cannot be cloned to mint duplicate attempts.
#[derive(Debug)]
pub(crate) struct FrameIdentityState {
    identity: FrameIdentity,
    function_ordinal: usize,
    attempt_limit: u64,
    last_attempt: u64,
    phase: FramePhase,
}

impl FrameIdentityState {
    #[cfg(test)]
    pub(crate) const fn identity(&self) -> FrameIdentity {
        self.identity
    }

    #[cfg(test)]
    pub(crate) const fn last_attempt(&self) -> u64 {
        self.last_attempt
    }

    #[cfg(test)]
    pub(crate) const fn is_suspended(&self) -> bool {
        matches!(self.phase, FramePhase::Suspended(_))
    }

    #[cfg(test)]
    pub(crate) const fn is_retired(&self) -> bool {
        matches!(self.phase, FramePhase::Retired)
    }

    pub(crate) fn pending(&self) -> Option<OperationIdentity> {
        match self.phase {
            FramePhase::Running(pending) | FramePhase::Suspended(pending) => {
                Some(self.operation_identity(pending))
            }
            FramePhase::Ready | FramePhase::Retired => None,
        }
    }

    pub(crate) fn begin(
        &mut self,
        site: SimulationDebugSiteV1,
    ) -> Result<OperationIdentity, IdentityStateError> {
        match self.phase {
            FramePhase::Ready => {}
            FramePhase::Running(_) | FramePhase::Suspended(_) => {
                return Err(IdentityStateError::OperationAlreadyPending);
            }
            FramePhase::Retired => return Err(IdentityStateError::FrameRetired),
        }
        if site.function_ordinal != self.function_ordinal {
            return Err(IdentityStateError::WrongFunction);
        }
        let next = self
            .last_attempt
            .checked_add(1)
            .ok_or(IdentityStateError::CounterOverflow)?;
        if next > self.attempt_limit {
            return Err(IdentityStateError::AttemptLimit {
                limit: self.attempt_limit,
            });
        }
        let attempt = NonZeroU64::new(next)
            .map(OperationAttemptId)
            .ok_or(IdentityStateError::CounterOverflow)?;
        let pending = PendingOperation { attempt, site };
        let identity = self.operation_identity(pending);
        self.last_attempt = next;
        self.phase = FramePhase::Running(pending);
        Ok(identity)
    }

    /// Suspend an operation, such as a call or cooperative wait, without
    /// completing it or incrementing its occurrence counter.
    pub(crate) fn suspend(
        &mut self,
        expected: OperationIdentity,
    ) -> Result<(), IdentityStateError> {
        let pending = self.checked_pending(expected)?;
        if matches!(self.phase, FramePhase::Suspended(_)) {
            return Err(IdentityStateError::AlreadySuspended);
        }
        self.phase = FramePhase::Suspended(pending);
        Ok(())
    }

    /// Resume only that exact pending operation. For calls the runtime must
    /// separately establish that the matching child really returned.
    pub(crate) fn resume(
        &mut self,
        expected: OperationIdentity,
    ) -> Result<OperationIdentity, IdentityStateError> {
        let pending = self.checked_pending(expected)?;
        if !matches!(self.phase, FramePhase::Suspended(_)) {
            return Err(IdentityStateError::OperationNotSuspended);
        }
        self.phase = FramePhase::Running(pending);
        Ok(self.operation_identity(pending))
    }

    /// Close bookkeeping after the caller retained its AfterOperation context.
    /// This method does not itself establish successful semantic execution.
    pub(crate) fn complete(
        &mut self,
        expected: OperationIdentity,
    ) -> Result<(), IdentityStateError> {
        self.checked_pending(expected)?;
        if matches!(self.phase, FramePhase::Suspended(_)) {
            return Err(IdentityStateError::OperationSuspended);
        }
        self.phase = FramePhase::Ready;
        Ok(())
    }

    pub(crate) fn retire(&mut self) -> Result<(), IdentityStateError> {
        match self.phase {
            FramePhase::Ready => {
                self.phase = FramePhase::Retired;
                Ok(())
            }
            FramePhase::Running(_) | FramePhase::Suspended(_) => {
                Err(IdentityStateError::OperationAlreadyPending)
            }
            FramePhase::Retired => Err(IdentityStateError::FrameRetired),
        }
    }

    fn checked_pending(
        &self,
        expected: OperationIdentity,
    ) -> Result<PendingOperation, IdentityStateError> {
        if expected.frame.invocation != self.identity.invocation {
            return Err(IdentityStateError::WrongInvocation);
        }
        if expected.frame.activation != self.identity.activation {
            return Err(IdentityStateError::WrongActivation);
        }
        let pending = match self.phase {
            FramePhase::Ready => return Err(IdentityStateError::NoPendingOperation),
            FramePhase::Retired => return Err(IdentityStateError::FrameRetired),
            FramePhase::Running(pending) | FramePhase::Suspended(pending) => pending,
        };
        if self.operation_identity(pending) != expected {
            return Err(IdentityStateError::OperationMismatch);
        }
        Ok(pending)
    }

    const fn operation_identity(&self, pending: PendingOperation) -> OperationIdentity {
        OperationIdentity {
            frame: self.identity,
            attempt: pending.attempt,
            site: pending.site,
        }
    }
}

#[cfg(test)]
#[path = "debug_identity_state_tests.rs"]
mod tests;

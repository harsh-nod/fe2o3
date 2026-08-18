use core::marker::PhantomData;

use fe2o3_service_model::{FailureDispositionV1, LifecycleStateV1, ServiceStateV1};

use crate::{BindingFieldV1, ServiceContractV1, ServiceHostErrorV1, ServiceKeyV1};

/// Adapter lifecycle phases, including the host-only prepared boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecyclePhaseV1 {
    /// Caller storage is borrowed but no start description was accepted.
    Prepared,
    /// A canonical starting model state was observed.
    Starting,
    /// A canonical running model state was observed.
    Running,
    /// New submissions are cut off while accepted work drains.
    Draining,
    /// A stop description follows drain.
    Stopping,
    /// A structurally quiescent stopped model state was observed.
    Stopped,
    /// Failure leaves device access possible, so storage stays retained.
    FailedMayAccess,
    /// Failure is structurally described as quiesced.
    FailedQuiesced,
}

impl LifecyclePhaseV1 {
    /// Reports whether this phase admits no later terminal transition.
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Stopped | Self::FailedQuiesced)
    }

    /// Reports the structural storage-retention disposition.
    pub const fn storage_disposition(self) -> StorageDispositionV1 {
        match self {
            Self::Stopped | Self::FailedQuiesced => StorageDispositionV1::Releasable,
            Self::Prepared
            | Self::Starting
            | Self::Running
            | Self::Draining
            | Self::Stopping
            | Self::FailedMayAccess => StorageDispositionV1::Retained,
        }
    }
}

/// Whether the adapter still retains caller storage borrows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageDispositionV1 {
    /// Storage must remain represented by the live service value.
    Retained,
    /// The structural lifecycle permits conversion to released borrows.
    Releasable,
}

/// Dynamic lifecycle cursor for validating hostile or decoded descriptions.
///
/// Normal callers should prefer the typestate aliases in this module.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCursorV1 {
    key: ServiceKeyV1,
    phase: LifecyclePhaseV1,
    revision: u8,
}

impl LifecycleCursorV1 {
    /// Creates revision zero at the prepared boundary.
    pub const fn prepared(key: ServiceKeyV1) -> Self {
        Self {
            key,
            phase: LifecyclePhaseV1::Prepared,
            revision: 0,
        }
    }

    /// Returns the exact service key.
    pub const fn key(self) -> ServiceKeyV1 {
        self.key
    }

    /// Returns the current lifecycle phase.
    pub const fn phase(self) -> LifecyclePhaseV1 {
        self.phase
    }

    /// Returns the zero-based lifecycle revision.
    pub const fn revision(self) -> u8 {
        self.revision
    }

    /// Reports whether this dynamic phase structurally retains storage.
    pub const fn storage_disposition(self) -> StorageDispositionV1 {
        self.phase.storage_disposition()
    }

    /// Checks an exact service-bound lifecycle edge.
    pub fn transition(
        self,
        key: ServiceKeyV1,
        next: LifecyclePhaseV1,
    ) -> Result<Self, ServiceHostErrorV1> {
        if key != self.key {
            return Err(ServiceHostErrorV1::BindingMismatch {
                field: BindingFieldV1::Lifecycle,
            });
        }
        if self.phase.is_terminal() {
            return Err(ServiceHostErrorV1::TerminalLifecycleTransition { phase: self.phase });
        }
        if !can_transition(self.phase, next) {
            return Err(ServiceHostErrorV1::InvalidLifecycleTransition {
                from: self.phase,
                to: next,
            });
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(ServiceHostErrorV1::LifecycleRevisionExhausted)?;
        Ok(Self {
            key: self.key,
            phase: next,
            revision,
        })
    }

    /// Checks whether a terminal model state structurally permits release.
    ///
    /// Success is not runtime quiescence evidence or release authority.
    pub fn validate_release(
        self,
        contract: &ServiceContractV1<'_>,
        model_state: &ServiceStateV1,
    ) -> Result<(), ServiceHostErrorV1> {
        if self.key != contract.key() {
            return Err(ServiceHostErrorV1::BindingMismatch {
                field: BindingFieldV1::Lifecycle,
            });
        }
        let expected = match self.phase {
            LifecyclePhaseV1::Stopped => LifecycleStateV1::Stopped,
            LifecyclePhaseV1::FailedQuiesced => {
                LifecycleStateV1::Failed(FailureDispositionV1::DeviceQuiesced)
            }
            LifecyclePhaseV1::Prepared
            | LifecyclePhaseV1::Starting
            | LifecyclePhaseV1::Running
            | LifecyclePhaseV1::Draining
            | LifecyclePhaseV1::Stopping
            | LifecyclePhaseV1::FailedMayAccess => {
                return Err(ServiceHostErrorV1::EarlyStorageRelease);
            }
        };
        contract.validate_quiescent_model_state(model_state, expected)
    }

    fn advance_known(self, next: LifecyclePhaseV1) -> Self {
        debug_assert!(can_transition(self.phase, next));
        Self {
            key: self.key,
            phase: next,
            revision: self.revision + 1,
        }
    }
}

const fn can_transition(current: LifecyclePhaseV1, next: LifecyclePhaseV1) -> bool {
    matches!(
        (current, next),
        (LifecyclePhaseV1::Prepared, LifecyclePhaseV1::Starting)
            | (LifecyclePhaseV1::Starting, LifecyclePhaseV1::Running)
            | (
                LifecyclePhaseV1::Starting
                    | LifecyclePhaseV1::Running
                    | LifecyclePhaseV1::Draining
                    | LifecyclePhaseV1::Stopping,
                LifecyclePhaseV1::FailedMayAccess | LifecyclePhaseV1::FailedQuiesced
            )
            | (LifecyclePhaseV1::Running, LifecyclePhaseV1::Draining)
            | (LifecyclePhaseV1::Draining, LifecyclePhaseV1::Stopping)
            | (LifecyclePhaseV1::Stopping, LifecyclePhaseV1::Stopped)
            | (
                LifecyclePhaseV1::FailedMayAccess,
                LifecyclePhaseV1::FailedQuiesced
            )
    )
}

/// Borrowed queue, state, input, and output storage retained by a service.
#[derive(Debug)]
pub struct ServiceResourcesV1<'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    queue: &'resource mut Queue,
    state: &'resource mut State,
    inputs: &'resource Inputs,
    outputs: &'resource mut Outputs,
}

impl<'resource, Queue, State, Inputs, Outputs>
    ServiceResourcesV1<'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Collects caller storage borrows without allocating or operating on it.
    pub const fn new(
        queue: &'resource mut Queue,
        state: &'resource mut State,
        inputs: &'resource Inputs,
        outputs: &'resource mut Outputs,
    ) -> Self {
        Self {
            queue,
            state,
            inputs,
            outputs,
        }
    }

    /// Borrows the retained queue description immutably.
    pub const fn queue(&self) -> &Queue {
        self.queue
    }

    /// Borrows the retained state description immutably.
    pub const fn state(&self) -> &State {
        self.state
    }

    /// Borrows the retained inputs.
    pub const fn inputs(&self) -> &Inputs {
        self.inputs
    }

    /// Borrows the retained outputs immutably.
    pub const fn outputs(&self) -> &Outputs {
        self.outputs
    }
}

/// Caller storage borrows returned after a structural terminal transition.
///
/// This type records only that the adapter stopped retaining the borrows. It
/// is not proof that a runtime or device stopped accessing storage.
#[derive(Debug)]
pub struct ReleasedResourcesV1<'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    resources: ServiceResourcesV1<'resource, Queue, State, Inputs, Outputs>,
}

impl<'resource, Queue, State, Inputs, Outputs>
    ReleasedResourcesV1<'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Returns the queue, state, input, and output borrows in that order.
    #[allow(clippy::type_complexity)]
    pub fn into_parts(
        self,
    ) -> (
        &'resource mut Queue,
        &'resource mut State,
        &'resource Inputs,
        &'resource mut Outputs,
    ) {
        let ServiceResourcesV1 {
            queue,
            state,
            inputs,
            outputs,
        } = self.resources;
        (queue, state, inputs, outputs)
    }
}

mod sealed {
    pub trait Sealed {}
}

#[doc(hidden)]
pub trait PhaseMarker: sealed::Sealed {
    const PHASE: LifecyclePhaseV1;
}

macro_rules! phase_marker {
    ($name:ident, $phase:ident) => {
        #[doc(hidden)]
        #[derive(Debug)]
        pub struct $name;

        impl sealed::Sealed for $name {}

        impl PhaseMarker for $name {
            const PHASE: LifecyclePhaseV1 = LifecyclePhaseV1::$phase;
        }
    };
}

phase_marker!(PreparedPhaseV1, Prepared);
phase_marker!(StartingPhaseV1, Starting);
phase_marker!(RunningPhaseV1, Running);
phase_marker!(DrainingPhaseV1, Draining);
phase_marker!(StoppingPhaseV1, Stopping);
phase_marker!(StoppedPhaseV1, Stopped);
phase_marker!(FailedMayAccessPhaseV1, FailedMayAccess);
phase_marker!(FailedQuiescedPhaseV1, FailedQuiesced);

/// Borrow-retaining service value parameterized by an adapter lifecycle phase.
#[derive(Debug)]
#[must_use = "a live service contract must reach a classified terminal representation"]
pub struct ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, Phase>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    pub(crate) contract: &'contract ServiceContractV1<'contract>,
    resources: ServiceResourcesV1<'resource, Queue, State, Inputs, Outputs>,
    cursor: LifecycleCursorV1,
    pub(crate) ticket_brand: TicketBrandV1,
    phase: PhantomData<Phase>,
}

#[derive(Debug)]
pub(crate) struct TicketBrandV1;

impl<'contract, 'resource, Queue, State, Inputs, Outputs, Phase>
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, Phase>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
    Phase: PhaseMarker,
{
    /// Returns the exact service contract.
    pub const fn contract(&self) -> &'contract ServiceContractV1<'contract> {
        self.contract
    }

    /// Returns the current typestate phase.
    pub const fn phase(&self) -> LifecyclePhaseV1 {
        Phase::PHASE
    }

    /// Returns the exact dynamic cursor corresponding to the typestate.
    pub const fn cursor(&self) -> LifecycleCursorV1 {
        self.cursor
    }

    /// Reports whether the service value still structurally retains storage.
    pub const fn storage_disposition(&self) -> StorageDispositionV1 {
        Phase::PHASE.storage_disposition()
    }

    /// Borrows the retained storage descriptions.
    pub const fn resources(&self) -> &ServiceResourcesV1<'resource, Queue, State, Inputs, Outputs> {
        &self.resources
    }

    fn into_phase<Next>(
        self,
    ) -> ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, Next>
    where
        Next: PhaseMarker,
    {
        ServiceHostV1 {
            contract: self.contract,
            resources: self.resources,
            cursor: self.cursor.advance_known(Next::PHASE),
            ticket_brand: self.ticket_brand,
            phase: PhantomData,
        }
    }

    fn observe<Next>(
        self,
        state: &ServiceStateV1,
        expected: LifecycleStateV1,
    ) -> Result<
        ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, Next>,
        TransitionRejectedV1<Self>,
    >
    where
        Next: PhaseMarker,
    {
        if let Err(error) = self.contract.validate_model_state(state, expected) {
            return Err(TransitionRejectedV1 {
                error,
                service: self,
            });
        }
        Ok(self.into_phase())
    }

    fn observe_failure(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        FailedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        let LifecycleStateV1::Failed(disposition) = state.lifecycle else {
            return Err(TransitionRejectedV1 {
                error: ServiceHostErrorV1::BindingMismatch {
                    field: BindingFieldV1::ModelState,
                },
                service: self,
            });
        };
        let validation = match disposition {
            FailureDispositionV1::DeviceMayStillAccess => {
                self.contract.validate_model_state(state, state.lifecycle)
            }
            FailureDispositionV1::DeviceQuiesced => self
                .contract
                .validate_quiescent_model_state(state, state.lifecycle),
        };
        if let Err(error) = validation {
            return Err(TransitionRejectedV1 {
                error,
                service: self,
            });
        }
        Ok(match disposition {
            FailureDispositionV1::DeviceMayStillAccess => {
                FailedServiceV1::MayStillAccess(self.into_phase())
            }
            FailureDispositionV1::DeviceQuiesced => FailedServiceV1::Quiesced(self.into_phase()),
        })
    }
}

/// Rejected transition paired with the still-borrow-retaining service value.
#[derive(Debug)]
pub struct TransitionRejectedV1<Service> {
    error: ServiceHostErrorV1,
    service: Service,
}

impl<Service> TransitionRejectedV1<Service> {
    /// Returns the structural rejection.
    pub const fn error(&self) -> ServiceHostErrorV1 {
        self.error
    }

    /// Recovers the service value so retained borrows are not lost on error.
    pub fn into_service(self) -> Service {
        self.service
    }

    /// Splits the rejection and retained service value.
    pub fn into_parts(self) -> (ServiceHostErrorV1, Service) {
        (self.error, self.service)
    }
}

/// Prepared service typestate.
pub type PreparedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, PreparedPhaseV1>;
/// Starting service typestate.
pub type StartingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, StartingPhaseV1>;
/// Running service typestate.
pub type RunningServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, RunningPhaseV1>;
/// Draining service typestate.
pub type DrainingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, DrainingPhaseV1>;
/// Stopping service typestate.
pub type StoppingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, StoppingPhaseV1>;
/// Stopped service typestate.
pub type StoppedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, StoppedPhaseV1>;
/// Failed service typestate that may still access caller storage.
pub type FailedMayAccessServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, FailedMayAccessPhaseV1>;
/// Failed service typestate with a structurally quiescent model state.
pub type FailedQuiescedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs> =
    ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, FailedQuiescedPhaseV1>;

/// Creates a prepared, borrow-retaining service description.
pub fn prepare<'contract, 'resource, Queue, State, Inputs, Outputs>(
    contract: &'contract ServiceContractV1<'contract>,
    resources: ServiceResourcesV1<'resource, Queue, State, Inputs, Outputs>,
) -> PreparedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    ServiceHostV1 {
        contract,
        resources,
        cursor: LifecycleCursorV1::prepared(contract.key()),
        ticket_brand: TicketBrandV1,
        phase: PhantomData,
    }
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    PreparedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Observes the canonical starting model state; it launches nothing.
    pub fn start(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        StartingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        self.observe(state, LifecycleStateV1::Starting)
    }
}

macro_rules! failure_method {
    () => {
        /// Observes a canonical classified failure; it performs no recovery.
        pub fn fail(
            self,
            state: &ServiceStateV1,
        ) -> Result<
            FailedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
            TransitionRejectedV1<Self>,
        > {
            self.observe_failure(state)
        }
    };
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    StartingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Observes the canonical running model state; it starts no execution.
    pub fn running(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        RunningServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        self.observe(state, LifecycleStateV1::Running)
    }

    failure_method!();
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    RunningServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Observes drain admission cutoff; it submits no runtime operation.
    pub fn drain(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        DrainingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        self.observe(state, LifecycleStateV1::Draining)
    }

    failure_method!();
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    DrainingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Observes the stopping model state after drain.
    pub fn stop(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        StoppingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        self.observe(state, LifecycleStateV1::Stopping)
    }

    failure_method!();
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    StoppingServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Observes a stopped state, including canonical quiescence invariants.
    pub fn stopped(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        StoppedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        if let Err(error) = self
            .contract
            .validate_quiescent_model_state(state, LifecycleStateV1::Stopped)
        {
            return Err(TransitionRejectedV1 {
                error,
                service: self,
            });
        }
        Ok(self.into_phase())
    }

    failure_method!();
}

/// Classified failure typestate preserving whether storage may be accessed.
#[derive(Debug)]
pub enum FailedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// The failure model says device access may remain live.
    MayStillAccess(FailedMayAccessServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>),
    /// The failure model and canonical state are structurally quiescent.
    Quiesced(FailedQuiescedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>),
}

impl<'contract, 'resource, Queue, State, Inputs, Outputs>
    FailedMayAccessServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>
where
    Queue: ?Sized,
    State: ?Sized,
    Inputs: ?Sized,
    Outputs: ?Sized,
{
    /// Observes the canonical failure-disposition refinement to quiesced.
    pub fn quiesced(
        self,
        state: &ServiceStateV1,
    ) -> Result<
        FailedQuiescedServiceV1<'contract, 'resource, Queue, State, Inputs, Outputs>,
        TransitionRejectedV1<Self>,
    > {
        let expected = LifecycleStateV1::Failed(FailureDispositionV1::DeviceQuiesced);
        if let Err(error) = self
            .contract
            .validate_quiescent_model_state(state, expected)
        {
            return Err(TransitionRejectedV1 {
                error,
                service: self,
            });
        }
        Ok(self.into_phase())
    }
}

macro_rules! release_method {
    ($phase:ident) => {
        impl<'contract, 'resource, Queue, State, Inputs, Outputs>
            ServiceHostV1<'contract, 'resource, Queue, State, Inputs, Outputs, $phase>
        where
            Queue: ?Sized,
            State: ?Sized,
            Inputs: ?Sized,
            Outputs: ?Sized,
        {
            /// Stops retaining storage borrows after a structural terminal state.
            ///
            /// This conversion performs no runtime release and grants no
            /// storage-release or quiescence authority.
            pub fn release(self) -> ReleasedResourcesV1<'resource, Queue, State, Inputs, Outputs> {
                ReleasedResourcesV1 {
                    resources: self.resources,
                }
            }
        }
    };
}

release_method!(StoppedPhaseV1);
release_method!(FailedQuiescedPhaseV1);

use core::fmt;

/// Exact relation rejected by a service-host structural check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BindingFieldV1 {
    /// Canonical model run identity.
    ServiceRun,
    /// Host service-instance identity.
    ServiceInstance,
    /// Canonical service epoch.
    ServiceEpoch,
    /// Canonical queue identity.
    QueueIdentity,
    /// Queue allocation epoch.
    QueueEpoch,
    /// Queue allocation ordinal.
    QueueOrdinal,
    /// Closed task-schema identity.
    TaskSchema,
    /// Scheduler-model identity.
    SchedulerModel,
    /// Host runtime-context identity.
    RuntimeContext,
    /// Host loaded-object identity.
    LoadedObject,
    /// Host load generation.
    LoadGeneration,
    /// Queue slot identity.
    Slot,
    /// Logical or encoded queue generation.
    Generation,
    /// Canonical task tag.
    TaskTag,
    /// Exact dispatch request.
    DispatchRequest,
    /// Exact dispatch result.
    DispatchResult,
    /// Dispatch submission identity.
    DispatchSubmission,
    /// Completion-signal identity.
    CompletionSignal,
    /// Exact wait request.
    WaitRequest,
    /// Wait completion observation.
    CompletionObservation,
    /// Canonical service-model state.
    ModelState,
    /// Lifecycle revision or service key.
    Lifecycle,
}

/// Structural rejection produced by this authority-free adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ServiceHostErrorV1 {
    /// The canonical service-model configuration was invalid.
    InvalidModelConfiguration,
    /// The canonical run identity input was invalid.
    InvalidRunContract,
    /// The supplied host load result was not a successful load.
    LoadNotSuccessful,
    /// A required service or queue epoch was zero.
    ZeroEpoch {
        /// Rejected epoch relation.
        field: BindingFieldV1,
    },
    /// Two exact identities, epochs, generations, or records disagreed.
    BindingMismatch {
        /// Rejected relation.
        field: BindingFieldV1,
    },
    /// The canonical service state failed its global invariants.
    InvalidModelState,
    /// The requested lifecycle edge skipped or reversed a phase.
    InvalidLifecycleTransition {
        /// Current phase.
        from: crate::LifecyclePhaseV1,
        /// Requested next phase.
        to: crate::LifecyclePhaseV1,
    },
    /// A transition attempted to leave or repeat a terminal phase.
    TerminalLifecycleTransition {
        /// Terminal phase that was asked to transition.
        phase: crate::LifecyclePhaseV1,
    },
    /// The lifecycle revision counter was exhausted.
    LifecycleRevisionExhausted,
    /// A persistent-task dispatch was required.
    PersistentTaskRequired,
    /// The dispatch contract did not describe a successful submission.
    DispatchNotSubmitted,
    /// The wait contract did not describe a satisfied terminal observation.
    WaitNotSatisfied,
    /// A ticket no longer names the current queue epoch or generation.
    StaleTicket,
    /// Storage release was represented before a quiesced terminal phase.
    EarlyStorageRelease,
    /// A failed or stopped model state was not structurally quiescent.
    ModelNotQuiescent,
}

impl fmt::Display for ServiceHostErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid V1 service-host contract: {self:?}")
    }
}

impl core::error::Error for ServiceHostErrorV1 {}

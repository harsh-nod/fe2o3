//! Executable finite R51 model for the public R48 compute-dependency lifecycle.
//!
//! The model is deliberately addressless. Identity, completion, native-effect,
//! and currentness facts are mathematical inputs. Named trace steps mirror R48
//! production transition names only so fixtures can detect drift in the modeled
//! story. They do not establish Rust-to-Verus or production-Rust refinement.
//! This module performs no I/O and grants no KFD, HSA, HIP, signal, event,
//! dispatch, teardown, hardware, progress, parity, or performance authority.

use alloc::{boxed::Box, vec::Vec};

pub const R51_MAX_ACTIVE_DEPENDENCY_TARGETS_V1: usize = 128;
pub const R51_MAX_DEPENDENCY_EVENTS_V1: usize = 256;

pub const R51_SUCCESS_TRACE_V1: &[&str] = &[
    "ensure_target_capacity",
    "reserve_acceptance_epoch",
    "retain_dependency_readers_for_target_v1",
    "begin_target_use",
    "publish_native",
    "bind_published_target",
    "poll_compute_dependency_dispatch_v1",
    "observe_published_target_once",
    "release_dependency_reader_event_batch_v1",
    "release_after_dependent_completion",
];

pub const R51_RETRY_ROLLBACK_TRACE_V1: &[&str] = &[
    "ensure_target_capacity",
    "reserve_acceptance_epoch",
    "retain_dependency_readers_for_target_v1",
    "begin_target_use",
    "publish_native",
    "rollback_retryable_before_side_effect",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R51DependencyTargetPhaseV1 {
    Prepared,
    NativePublished,
    Published,
    Completed,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R51ProductionTransitionV1 {
    SubmitFixedDispatchWithDependencyEvents,
    EnsureTargetCapacity,
    ReserveAcceptanceEpoch,
    RetainDependencyReadersForTarget,
    BeginTargetUse,
    PublishNative,
    BindPublishedTarget,
    PollComputeDependencyDispatch,
    ObservePublishedTargetOnce,
    ReleaseDependencyReaderEventBatch,
    ReleaseAfterDependentCompletion,
    RollbackRetryableBeforeSideEffect,
    RecycleFixedDispatch,
    ReleaseComputeDependencyEvent,
    DestroyAuxiliaryComputeLane,
}

impl R51ProductionTransitionV1 {
    pub const fn production_name_model_only(self) -> &'static str {
        match self {
            Self::SubmitFixedDispatchWithDependencyEvents => {
                "submit_fixed_dispatch_with_dependency_events_v1"
            }
            Self::EnsureTargetCapacity => "ensure_target_capacity",
            Self::ReserveAcceptanceEpoch => "reserve_acceptance_epoch",
            Self::RetainDependencyReadersForTarget => "retain_dependency_readers_for_target_v1",
            Self::BeginTargetUse => "begin_target_use",
            Self::PublishNative => "publish_native",
            Self::BindPublishedTarget => "bind_published_target",
            Self::PollComputeDependencyDispatch => "poll_compute_dependency_dispatch_v1",
            Self::ObservePublishedTargetOnce => "observe_published_target_once",
            Self::ReleaseDependencyReaderEventBatch => "release_dependency_reader_event_batch_v1",
            Self::ReleaseAfterDependentCompletion => "release_after_dependent_completion",
            Self::RollbackRetryableBeforeSideEffect => "rollback_retryable_before_side_effect",
            Self::RecycleFixedDispatch => "recycle_fixed_dispatch",
            Self::ReleaseComputeDependencyEvent => "release_compute_dependency_event_v1",
            Self::DestroyAuxiliaryComputeLane => "destroy_auxiliary_compute_lane_v1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R51SignalIdentityV1 {
    pub mapping: u64,
    pub slot: u16,
    pub generation: u64,
}

impl R51SignalIdentityV1 {
    pub const fn valid_model_only(self) -> bool {
        self.mapping != 0 && self.generation != 0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51SourceEventV1 {
    owner_occurrence: u64,
    session_occurrence: u64,
    source_lane: u64,
    source_arena: u64,
    source_epoch: u64,
    public_custody: u64,
    signal: R51SignalIdentityV1,
}

impl R51SourceEventV1 {
    pub const fn source_epoch_model_only(&self) -> u64 {
        self.source_epoch
    }

    pub const fn source_arena_model_only(&self) -> u64 {
        self.source_arena
    }

    pub const fn source_lane_model_only(&self) -> u64 {
        self.source_lane
    }

    pub const fn public_custody_model_only(&self) -> u64 {
        self.public_custody
    }

    pub const fn signal_model_only(&self) -> R51SignalIdentityV1 {
        self.signal
    }

    pub fn substitute_arena_model_only(mut self, source_arena: u64) -> Self {
        self.source_arena = source_arena;
        self
    }

    pub fn substitute_epoch_model_only(mut self, source_epoch: u64) -> Self {
        self.source_epoch = source_epoch;
        self
    }

    pub fn substitute_owner_model_only(mut self, owner_occurrence: u64) -> Self {
        self.owner_occurrence = owner_occurrence;
        self
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51TargetEventV1 {
    owner_occurrence: u64,
    session_occurrence: u64,
    target_epoch: u64,
    public_custody: u64,
    signal: R51SignalIdentityV1,
}

impl R51TargetEventV1 {
    pub const fn target_epoch_model_only(&self) -> u64 {
        self.target_epoch
    }

    pub const fn public_custody_model_only(&self) -> u64 {
        self.public_custody
    }

    pub const fn signal_model_only(&self) -> R51SignalIdentityV1 {
        self.signal
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51CompletedSignalV1 {
    public_custody: u64,
    signal: R51SignalIdentityV1,
}

impl R51CompletedSignalV1 {
    pub const fn public_custody_model_only(&self) -> u64 {
        self.public_custody
    }

    pub const fn signal_model_only(&self) -> R51SignalIdentityV1 {
        self.signal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R51NativePublicationFaultV1 {
    None,
    RingFullNoEffect,
    AtOrAfterFirstClaim,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R51CompletionObservationV1 {
    Pending,
    ExactCompleted,
    Substituted {
        target_epoch: u64,
        dispatch_custody: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R51DependencyLifecycleErrorV1 {
    InvalidIdentity,
    Terminal,
    EpochExhausted,
    ActiveTargetCapacity,
    EmptyDependencies,
    TooManyDependencies,
    CrossSession,
    SameLane,
    MultipleSourceArenas,
    MultipleSourceLanes,
    DuplicateEvent,
    DuplicateSignal,
    StaleOrCyclicEpoch,
    InvalidCustody,
    InvalidPhase,
    ExactCompletionMismatch,
    SignalPinned,
    TeardownBlocked,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51BeginFailureV1 {
    pub error: R51DependencyLifecycleErrorV1,
    pub events: Vec<R51SourceEventV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51PreparedTargetV1 {
    owner_occurrence: u64,
    epoch: u64,
    target_lane: u64,
    source_lane: u64,
    source_arena: u64,
    dispatch_custody: u64,
    target_event: R51TargetEventV1,
    events: Vec<R51SourceEventV1>,
}

impl R51PreparedTargetV1 {
    pub const fn target_epoch_model_only(&self) -> u64 {
        self.epoch
    }

    pub const fn source_arena_model_only(&self) -> u64 {
        self.source_arena
    }

    pub fn substitute_epoch_model_only(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51NativePublishedTargetV1 {
    prepared: R51PreparedTargetV1,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51PublishedTargetV1 {
    owner_occurrence: u64,
    epoch: u64,
    target_lane: u64,
    source_lane: u64,
    source_arena: u64,
    dispatch_custody: u64,
    target_signal: R51SignalIdentityV1,
    events: Vec<R51SourceEventV1>,
}

impl R51PublishedTargetV1 {
    pub const fn target_epoch_model_only(&self) -> u64 {
        self.epoch
    }

    pub const fn dispatch_custody_model_only(&self) -> u64 {
        self.dispatch_custody
    }

    pub const fn source_arena_model_only(&self) -> u64 {
        self.source_arena
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51CompletedTargetV1 {
    owner_occurrence: u64,
    epoch: u64,
    target_lane: u64,
    dispatch_custody: u64,
    target_signal: R51SignalIdentityV1,
    dependency_count: u16,
}

impl R51CompletedTargetV1 {
    pub const fn target_epoch_model_only(&self) -> u64 {
        self.epoch
    }

    pub const fn dispatch_custody_model_only(&self) -> u64 {
        self.dispatch_custody
    }

    pub const fn dependency_count_model_only(&self) -> u16 {
        self.dependency_count
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51ReleasedTargetV1 {
    completed: R51CompletedTargetV1,
}

impl R51ReleasedTargetV1 {
    pub const fn phase_model_only(&self) -> R51DependencyTargetPhaseV1 {
        R51DependencyTargetPhaseV1::Released
    }

    pub const fn target_epoch_model_only(&self) -> u64 {
        self.completed.epoch
    }

    pub const fn dispatch_custody_model_only(&self) -> u64 {
        self.completed.dispatch_custody
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51RetryableTargetV1 {
    prepared: R51PreparedTargetV1,
}

#[derive(Debug, Eq, PartialEq)]
pub enum R51NativePublicationV1 {
    Published(R51NativePublishedTargetV1),
    Retryable(R51RetryableTargetV1),
}

#[derive(Debug, Eq, PartialEq)]
pub enum R51PollV1 {
    Pending(Box<R51PublishedTargetV1>),
    Ready(R51CompletedTargetV1),
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51SignalPinnedFailureV1 {
    pub error: R51DependencyLifecycleErrorV1,
    pub completed: R51CompletedSignalV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R51TeardownReceiptV1 {
    pub owner_occurrence: u64,
    pub active_targets: usize,
    pub event_pins: usize,
    pub reader_pins: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R51PinRecordV1 {
    signal: R51SignalIdentityV1,
    public_custody: u64,
    event_pins: u16,
    reader_pins: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R51ActiveTargetV1 {
    epoch: u64,
    target_lane: u64,
    source_lane: u64,
    source_arena: u64,
    dispatch_custody: u64,
    target_event_custody: u64,
    target_signal: R51SignalIdentityV1,
    dependency_count: u16,
    phase: R51DependencyTargetPhaseV1,
    source_pins_released: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R51LifecycleSnapshotV1 {
    pub next_acceptance_epoch: Option<u64>,
    pub active_targets: usize,
    pub event_pins: usize,
    pub reader_pins: usize,
    pub native_effects: u64,
    pub recycle_mutations: u64,
    pub terminal: bool,
    pub last_released_epoch: Option<u64>,
    pub active_phases: Vec<(u64, R51DependencyTargetPhaseV1)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct R51DependencyLifecycleV1 {
    owner_occurrence: u64,
    session_occurrence: u64,
    next_acceptance_epoch: Option<u64>,
    active: Vec<R51ActiveTargetV1>,
    pins: Vec<R51PinRecordV1>,
    trace: Vec<R51ProductionTransitionV1>,
    native_effects: u64,
    recycle_mutations: u64,
    terminal: bool,
    last_released_epoch: Option<u64>,
}

impl R51DependencyLifecycleV1 {
    pub fn new_model_only(
        owner_occurrence: u64,
        session_occurrence: u64,
    ) -> Result<Self, R51DependencyLifecycleErrorV1> {
        if owner_occurrence == 0 || session_occurrence == 0 {
            return Err(R51DependencyLifecycleErrorV1::InvalidIdentity);
        }
        Ok(Self {
            owner_occurrence,
            session_occurrence,
            next_acceptance_epoch: Some(1),
            active: Vec::with_capacity(R51_MAX_ACTIVE_DEPENDENCY_TARGETS_V1),
            pins: Vec::new(),
            trace: Vec::new(),
            native_effects: 0,
            recycle_mutations: 0,
            terminal: false,
            last_released_epoch: None,
        })
    }

    pub fn snapshot_model_only(&self) -> R51LifecycleSnapshotV1 {
        R51LifecycleSnapshotV1 {
            next_acceptance_epoch: self.next_acceptance_epoch,
            active_targets: self.active.len(),
            event_pins: self
                .pins
                .iter()
                .map(|pin| usize::from(pin.event_pins))
                .sum(),
            reader_pins: self
                .pins
                .iter()
                .map(|pin| usize::from(pin.reader_pins))
                .sum(),
            native_effects: self.native_effects,
            recycle_mutations: self.recycle_mutations,
            terminal: self.terminal,
            last_released_epoch: self.last_released_epoch,
            active_phases: self
                .active
                .iter()
                .map(|target| (target.epoch, target.phase))
                .collect(),
        }
    }

    pub fn trace_names_model_only(&self) -> Vec<&'static str> {
        self.trace
            .iter()
            .map(|step| step.production_name_model_only())
            .collect()
    }

    pub fn clear_trace_model_only(&mut self) {
        self.trace.clear();
    }

    pub const fn next_acceptance_epoch_model_only(&self) -> Option<u64> {
        self.next_acceptance_epoch
    }

    pub fn substitute_next_acceptance_epoch_model_only(&mut self, epoch: Option<u64>) {
        self.next_acceptance_epoch = epoch;
    }

    pub const fn terminal_model_only(&self) -> bool {
        self.terminal
    }

    pub fn phase_model_only(&self, epoch: u64) -> Option<R51DependencyTargetPhaseV1> {
        self.active
            .iter()
            .find(|target| target.epoch == epoch)
            .map(|target| target.phase)
            .or_else(|| {
                (self.last_released_epoch == Some(epoch))
                    .then_some(R51DependencyTargetPhaseV1::Released)
            })
    }

    pub fn publish_source_event_model_only(
        &mut self,
        source_lane: u64,
        source_arena: u64,
        public_custody: u64,
        signal: R51SignalIdentityV1,
    ) -> Result<R51SourceEventV1, R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        if source_lane == 0
            || source_arena == 0
            || public_custody == 0
            || !signal.valid_model_only()
        {
            return Err(R51DependencyLifecycleErrorV1::InvalidIdentity);
        }
        if self
            .pins
            .iter()
            .any(|pin| pin.public_custody == public_custody)
        {
            return Err(R51DependencyLifecycleErrorV1::DuplicateEvent);
        }
        if self.pins.iter().any(|pin| pin.signal == signal) {
            return Err(R51DependencyLifecycleErrorV1::DuplicateSignal);
        }
        let epoch = self.burn_acceptance_epoch_model_only()?;
        self.pins.push(R51PinRecordV1 {
            signal,
            public_custody,
            event_pins: 1,
            reader_pins: 0,
        });
        self.trace
            .push(R51ProductionTransitionV1::SubmitFixedDispatchWithDependencyEvents);
        Ok(R51SourceEventV1 {
            owner_occurrence: self.owner_occurrence,
            session_occurrence: self.session_occurrence,
            source_lane,
            source_arena,
            source_epoch: epoch,
            public_custody,
            signal,
        })
    }

    pub fn completed_source_signal_model_only(
        &self,
        event: &R51SourceEventV1,
        completed_custody: u64,
    ) -> Result<R51CompletedSignalV1, R51DependencyLifecycleErrorV1> {
        if completed_custody == 0 || !self.event_is_exact_model_only(event) {
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        Ok(R51CompletedSignalV1 {
            public_custody: completed_custody,
            signal: event.signal,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_target_model_only(
        &mut self,
        target_lane: u64,
        dispatch_custody: u64,
        target_event_custody: u64,
        target_signal: R51SignalIdentityV1,
        events: Vec<R51SourceEventV1>,
    ) -> Result<R51PreparedTargetV1, R51BeginFailureV1> {
        let reject = |error, events| R51BeginFailureV1 { error, events };
        if self.terminal {
            return Err(reject(R51DependencyLifecycleErrorV1::Terminal, events));
        }
        if events.is_empty() {
            return Err(reject(
                R51DependencyLifecycleErrorV1::EmptyDependencies,
                events,
            ));
        }
        if events.len() > R51_MAX_DEPENDENCY_EVENTS_V1 {
            return Err(reject(
                R51DependencyLifecycleErrorV1::TooManyDependencies,
                events,
            ));
        }
        if self.active.len() >= R51_MAX_ACTIVE_DEPENDENCY_TARGETS_V1 {
            return Err(reject(
                R51DependencyLifecycleErrorV1::ActiveTargetCapacity,
                events,
            ));
        }
        if target_lane == 0
            || dispatch_custody == 0
            || target_event_custody == 0
            || !target_signal.valid_model_only()
            || self.pins.iter().any(|pin| pin.signal == target_signal)
            || self
                .pins
                .iter()
                .any(|pin| pin.public_custody == target_event_custody)
        {
            return Err(reject(
                R51DependencyLifecycleErrorV1::InvalidIdentity,
                events,
            ));
        }
        let source_lane = events[0].source_lane;
        let source_arena = events[0].source_arena;
        let target_epoch = match self.next_acceptance_epoch {
            Some(epoch) => epoch,
            None => {
                self.terminal = true;
                return Err(reject(
                    R51DependencyLifecycleErrorV1::EpochExhausted,
                    events,
                ));
            }
        };
        for (index, event) in events.iter().enumerate() {
            if event.owner_occurrence != self.owner_occurrence
                || event.session_occurrence != self.session_occurrence
            {
                return Err(reject(R51DependencyLifecycleErrorV1::CrossSession, events));
            }
            if event.source_lane != source_lane {
                return Err(reject(
                    R51DependencyLifecycleErrorV1::MultipleSourceLanes,
                    events,
                ));
            }
            if event.source_arena != source_arena {
                return Err(reject(
                    R51DependencyLifecycleErrorV1::MultipleSourceArenas,
                    events,
                ));
            }
            if event.source_lane == target_lane {
                return Err(reject(R51DependencyLifecycleErrorV1::SameLane, events));
            }
            if event.source_epoch == 0 || event.source_epoch >= target_epoch {
                return Err(reject(
                    R51DependencyLifecycleErrorV1::StaleOrCyclicEpoch,
                    events,
                ));
            }
            if !self.event_is_exact_model_only(event) {
                return Err(reject(
                    R51DependencyLifecycleErrorV1::InvalidCustody,
                    events,
                ));
            }
            if events[..index]
                .iter()
                .any(|prior| prior.public_custody == event.public_custody)
            {
                return Err(reject(
                    R51DependencyLifecycleErrorV1::DuplicateEvent,
                    events,
                ));
            }
            if events[..index]
                .iter()
                .any(|prior| prior.signal == event.signal)
            {
                return Err(reject(
                    R51DependencyLifecycleErrorV1::DuplicateSignal,
                    events,
                ));
            }
        }

        self.trace
            .push(R51ProductionTransitionV1::EnsureTargetCapacity);
        let epoch = match self.burn_acceptance_epoch_model_only() {
            Ok(epoch) => epoch,
            Err(error) => return Err(reject(error, events)),
        };
        self.trace
            .push(R51ProductionTransitionV1::ReserveAcceptanceEpoch);
        for event in &events {
            let pin = self
                .pins
                .iter_mut()
                .find(|pin| pin.public_custody == event.public_custody)
                .expect("event custody was preflighted");
            pin.reader_pins = 1;
        }
        self.trace
            .push(R51ProductionTransitionV1::RetainDependencyReadersForTarget);
        self.pins.push(R51PinRecordV1 {
            signal: target_signal,
            public_custody: target_event_custody,
            event_pins: 1,
            reader_pins: 0,
        });
        self.active.push(R51ActiveTargetV1 {
            epoch,
            target_lane,
            source_lane,
            source_arena,
            dispatch_custody,
            target_event_custody,
            target_signal,
            dependency_count: events.len() as u16,
            phase: R51DependencyTargetPhaseV1::Prepared,
            source_pins_released: false,
        });
        self.trace.push(R51ProductionTransitionV1::BeginTargetUse);
        Ok(R51PreparedTargetV1 {
            owner_occurrence: self.owner_occurrence,
            epoch,
            target_lane,
            source_lane,
            source_arena,
            dispatch_custody,
            target_event: R51TargetEventV1 {
                owner_occurrence: self.owner_occurrence,
                session_occurrence: self.session_occurrence,
                target_epoch: epoch,
                public_custody: target_event_custody,
                signal: target_signal,
            },
            events,
        })
    }

    pub fn publish_native_model_only(
        &mut self,
        prepared: R51PreparedTargetV1,
        fault: R51NativePublicationFaultV1,
    ) -> Result<R51NativePublicationV1, R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        if !self.prepared_is_exact_model_only(&prepared) {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.trace.push(R51ProductionTransitionV1::PublishNative);
        match fault {
            R51NativePublicationFaultV1::RingFullNoEffect => {
                Ok(R51NativePublicationV1::Retryable(R51RetryableTargetV1 {
                    prepared,
                }))
            }
            R51NativePublicationFaultV1::AtOrAfterFirstClaim => {
                self.native_effects += 1;
                self.target_mut_model_only(prepared.epoch).phase =
                    R51DependencyTargetPhaseV1::NativePublished;
                self.terminal = true;
                Err(R51DependencyLifecycleErrorV1::Terminal)
            }
            R51NativePublicationFaultV1::None => {
                self.native_effects += 1;
                self.target_mut_model_only(prepared.epoch).phase =
                    R51DependencyTargetPhaseV1::NativePublished;
                Ok(R51NativePublicationV1::Published(
                    R51NativePublishedTargetV1 { prepared },
                ))
            }
        }
    }

    pub fn rollback_retryable_model_only(
        &mut self,
        retryable: R51RetryableTargetV1,
    ) -> Result<Vec<R51SourceEventV1>, R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        let prepared = retryable.prepared;
        if !self.prepared_is_exact_model_only(&prepared)
            || prepared.events.iter().any(|event| {
                self.pins
                    .iter()
                    .find(|pin| pin.public_custody == event.public_custody)
                    .is_none_or(|pin| pin.event_pins != 1 || pin.reader_pins != 1)
            })
        {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.trace
            .push(R51ProductionTransitionV1::RollbackRetryableBeforeSideEffect);
        for event in &prepared.events {
            self.pins
                .iter_mut()
                .find(|pin| pin.public_custody == event.public_custody)
                .expect("rollback preflight authenticated every event")
                .reader_pins = 0;
        }
        self.remove_pin_model_only(prepared.target_event.public_custody);
        self.remove_active_model_only(prepared.epoch);
        Ok(prepared.events)
    }

    pub fn bind_published_target_model_only(
        &mut self,
        native: R51NativePublishedTargetV1,
    ) -> Result<(Box<R51PublishedTargetV1>, R51TargetEventV1), R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        let prepared = native.prepared;
        if !self.prepared_is_exact_phase_model_only(
            &prepared,
            R51DependencyTargetPhaseV1::NativePublished,
        ) {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.target_mut_model_only(prepared.epoch).phase = R51DependencyTargetPhaseV1::Published;
        self.trace
            .push(R51ProductionTransitionV1::BindPublishedTarget);
        Ok((
            Box::new(R51PublishedTargetV1 {
                owner_occurrence: prepared.owner_occurrence,
                epoch: prepared.epoch,
                target_lane: prepared.target_lane,
                source_lane: prepared.source_lane,
                source_arena: prepared.source_arena,
                dispatch_custody: prepared.dispatch_custody,
                target_signal: prepared.target_event.signal,
                events: prepared.events,
            }),
            prepared.target_event,
        ))
    }

    pub fn poll_model_only(
        &mut self,
        published: Box<R51PublishedTargetV1>,
        observation: R51CompletionObservationV1,
    ) -> Result<R51PollV1, R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        if !self.published_is_exact_model_only(&published) {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.trace
            .push(R51ProductionTransitionV1::PollComputeDependencyDispatch);
        self.trace
            .push(R51ProductionTransitionV1::ObservePublishedTargetOnce);
        match observation {
            R51CompletionObservationV1::Pending => Ok(R51PollV1::Pending(published)),
            R51CompletionObservationV1::Substituted {
                target_epoch,
                dispatch_custody,
            } => {
                let _ = (target_epoch, dispatch_custody);
                self.terminal = true;
                Err(R51DependencyLifecycleErrorV1::ExactCompletionMismatch)
            }
            R51CompletionObservationV1::ExactCompleted => {
                if published.events.iter().any(|event| {
                    self.pins
                        .iter()
                        .find(|pin| pin.public_custody == event.public_custody)
                        .is_none_or(|pin| pin.event_pins != 1 || pin.reader_pins != 1)
                }) {
                    self.terminal = true;
                    return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
                }
                self.target_mut_model_only(published.epoch).phase =
                    R51DependencyTargetPhaseV1::Completed;
                for event in &published.events {
                    self.remove_pin_model_only(event.public_custody);
                }
                self.target_mut_model_only(published.epoch)
                    .source_pins_released = true;
                self.trace
                    .push(R51ProductionTransitionV1::ReleaseDependencyReaderEventBatch);
                Ok(R51PollV1::Ready(R51CompletedTargetV1 {
                    owner_occurrence: published.owner_occurrence,
                    epoch: published.epoch,
                    target_lane: published.target_lane,
                    dispatch_custody: published.dispatch_custody,
                    target_signal: published.target_signal,
                    dependency_count: published.events.len() as u16,
                }))
            }
        }
    }

    pub fn release_after_dependent_completion_model_only(
        &mut self,
        completed: R51CompletedTargetV1,
    ) -> Result<R51ReleasedTargetV1, R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        let Some(target) = self
            .active
            .iter()
            .find(|target| target.epoch == completed.epoch)
        else {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        };
        if target.phase != R51DependencyTargetPhaseV1::Completed
            || !target.source_pins_released
            || target.dispatch_custody != completed.dispatch_custody
            || target.target_lane != completed.target_lane
            || target.target_signal != completed.target_signal
            || target.dependency_count != completed.dependency_count
            || completed.owner_occurrence != self.owner_occurrence
        {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.remove_active_model_only(completed.epoch);
        self.last_released_epoch = Some(completed.epoch);
        self.trace
            .push(R51ProductionTransitionV1::ReleaseAfterDependentCompletion);
        Ok(R51ReleasedTargetV1 { completed })
    }

    pub fn completed_target_signal_model_only(
        &self,
        released: &R51ReleasedTargetV1,
        completed_custody: u64,
    ) -> Result<R51CompletedSignalV1, R51DependencyLifecycleErrorV1> {
        if completed_custody == 0
            || self.last_released_epoch != Some(released.completed.epoch)
            || released.completed.owner_occurrence != self.owner_occurrence
        {
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        Ok(R51CompletedSignalV1 {
            public_custody: completed_custody,
            signal: released.completed.target_signal,
        })
    }

    pub fn release_target_event_model_only(
        &mut self,
        event: R51TargetEventV1,
    ) -> Result<(), R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        let Some(pin) = self
            .pins
            .iter()
            .find(|pin| pin.public_custody == event.public_custody)
        else {
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        };
        if event.owner_occurrence != self.owner_occurrence
            || event.session_occurrence != self.session_occurrence
            || pin.signal != event.signal
            || pin.event_pins != 1
            || pin.reader_pins != 0
        {
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.remove_pin_model_only(event.public_custody);
        self.trace
            .push(R51ProductionTransitionV1::ReleaseComputeDependencyEvent);
        Ok(())
    }

    pub fn release_source_event_model_only(
        &mut self,
        event: R51SourceEventV1,
    ) -> Result<(), R51DependencyLifecycleErrorV1> {
        if self.terminal {
            return Err(R51DependencyLifecycleErrorV1::Terminal);
        }
        if !self.event_is_exact_model_only(&event) {
            return Err(R51DependencyLifecycleErrorV1::InvalidCustody);
        }
        self.remove_pin_model_only(event.public_custody);
        self.trace
            .push(R51ProductionTransitionV1::ReleaseComputeDependencyEvent);
        Ok(())
    }

    pub fn recycle_completed_signal_model_only(
        &mut self,
        completed: R51CompletedSignalV1,
    ) -> Result<(), R51SignalPinnedFailureV1> {
        if self.pins.iter().any(|pin| {
            pin.signal == completed.signal && (pin.event_pins != 0 || pin.reader_pins != 0)
        }) {
            return Err(R51SignalPinnedFailureV1 {
                error: R51DependencyLifecycleErrorV1::SignalPinned,
                completed,
            });
        }
        if self.terminal {
            return Err(R51SignalPinnedFailureV1 {
                error: R51DependencyLifecycleErrorV1::Terminal,
                completed,
            });
        }
        self.recycle_mutations += 1;
        self.trace
            .push(R51ProductionTransitionV1::RecycleFixedDispatch);
        Ok(())
    }

    pub fn teardown_allowed_model_only(&self) -> bool {
        self.active.is_empty()
            && self
                .pins
                .iter()
                .all(|pin| pin.event_pins == 0 && pin.reader_pins == 0)
    }

    pub fn teardown_model_only(
        &mut self,
    ) -> Result<R51TeardownReceiptV1, R51DependencyLifecycleErrorV1> {
        if !self.teardown_allowed_model_only() {
            return Err(R51DependencyLifecycleErrorV1::TeardownBlocked);
        }
        self.trace
            .push(R51ProductionTransitionV1::DestroyAuxiliaryComputeLane);
        Ok(R51TeardownReceiptV1 {
            owner_occurrence: self.owner_occurrence,
            active_targets: 0,
            event_pins: 0,
            reader_pins: 0,
        })
    }

    fn burn_acceptance_epoch_model_only(&mut self) -> Result<u64, R51DependencyLifecycleErrorV1> {
        let Some(epoch) = self.next_acceptance_epoch else {
            self.terminal = true;
            return Err(R51DependencyLifecycleErrorV1::EpochExhausted);
        };
        if epoch == 0 {
            self.terminal = true;
            self.next_acceptance_epoch = None;
            return Err(R51DependencyLifecycleErrorV1::EpochExhausted);
        }
        self.next_acceptance_epoch = epoch.checked_add(1);
        Ok(epoch)
    }

    fn event_is_exact_model_only(&self, event: &R51SourceEventV1) -> bool {
        event.owner_occurrence == self.owner_occurrence
            && event.session_occurrence == self.session_occurrence
            && event.source_lane != 0
            && event.source_arena != 0
            && event.source_epoch != 0
            && event.public_custody != 0
            && event.signal.valid_model_only()
            && self.pins.iter().any(|pin| {
                pin.public_custody == event.public_custody
                    && pin.signal == event.signal
                    && pin.event_pins == 1
                    && pin.reader_pins == 0
            })
    }

    fn prepared_is_exact_model_only(&self, prepared: &R51PreparedTargetV1) -> bool {
        self.prepared_is_exact_phase_model_only(prepared, R51DependencyTargetPhaseV1::Prepared)
    }

    fn prepared_is_exact_phase_model_only(
        &self,
        prepared: &R51PreparedTargetV1,
        phase: R51DependencyTargetPhaseV1,
    ) -> bool {
        prepared.owner_occurrence == self.owner_occurrence
            && prepared.target_event.owner_occurrence == self.owner_occurrence
            && prepared.target_event.session_occurrence == self.session_occurrence
            && prepared.target_event.target_epoch == prepared.epoch
            && self.active.iter().any(|target| {
                target.epoch == prepared.epoch
                    && target.target_lane == prepared.target_lane
                    && target.source_lane == prepared.source_lane
                    && target.source_arena == prepared.source_arena
                    && target.dispatch_custody == prepared.dispatch_custody
                    && target.target_event_custody == prepared.target_event.public_custody
                    && target.target_signal == prepared.target_event.signal
                    && target.dependency_count == prepared.events.len() as u16
                    && target.phase == phase
                    && !target.source_pins_released
            })
            && prepared.events.iter().all(|event| {
                event.owner_occurrence == self.owner_occurrence
                    && event.session_occurrence == self.session_occurrence
                    && event.source_lane == prepared.source_lane
                    && event.source_arena == prepared.source_arena
                    && event.source_epoch < prepared.epoch
                    && self.pins.iter().any(|pin| {
                        pin.public_custody == event.public_custody
                            && pin.signal == event.signal
                            && pin.event_pins == 1
                            && pin.reader_pins == 1
                    })
            })
    }

    fn published_is_exact_model_only(&self, published: &R51PublishedTargetV1) -> bool {
        published.owner_occurrence == self.owner_occurrence
            && self.active.iter().any(|target| {
                target.epoch == published.epoch
                    && target.target_lane == published.target_lane
                    && target.source_lane == published.source_lane
                    && target.source_arena == published.source_arena
                    && target.dispatch_custody == published.dispatch_custody
                    && target.target_signal == published.target_signal
                    && target.dependency_count == published.events.len() as u16
                    && target.phase == R51DependencyTargetPhaseV1::Published
                    && !target.source_pins_released
            })
            && published.events.iter().all(|event| {
                event.source_lane == published.source_lane
                    && event.source_arena == published.source_arena
                    && event.source_epoch < published.epoch
            })
    }

    fn target_mut_model_only(&mut self, epoch: u64) -> &mut R51ActiveTargetV1 {
        self.active
            .iter_mut()
            .find(|target| target.epoch == epoch)
            .expect("authenticated target epoch remains active")
    }

    fn remove_active_model_only(&mut self, epoch: u64) {
        let index = self
            .active
            .iter()
            .position(|target| target.epoch == epoch)
            .expect("authenticated target epoch remains active");
        self.active.remove(index);
    }

    fn remove_pin_model_only(&mut self, public_custody: u64) {
        let index = self
            .pins
            .iter()
            .position(|pin| pin.public_custody == public_custody)
            .expect("authenticated pin remains live");
        self.pins.remove(index);
    }
}

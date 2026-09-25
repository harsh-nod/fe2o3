//! Independent executable R37 model for typed native SDMA wait activation.
//!
//! The model starts from an abstract published directional or same-device
//! submission. It records caller-supplied native observations and models the
//! custody transition performed by a bounded wait. It also models only the
//! route selected for explicit poll calls and non-published waits.
//!
//! All identities, counts, storage tokens, deadlines, and native outcomes are
//! finite contracted inputs. This model performs no I/O and does not refine
//! production Rust, KFD, HSA, HIP, a driver, firmware, a native queue, a clock,
//! hardware completion or coherence, progress, liveness, or performance.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37CopyKindV1 {
    Directional,
    SameDevice,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37DeadlineClassV1 {
    Zero,
    Positive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37CompletionDispositionV1 {
    Settle,
    ContinuationReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37IdentityChangeStageV1 {
    Pending,
    Completed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R37NativeIdentityV1 {
    pub owner_id: u64,
    pub request_id: u64,
}

impl R37NativeIdentityV1 {
    pub const fn is_valid(self) -> bool {
        self.owner_id != 0 && self.request_id != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R37OrderedFrameV1 {
    pub predecessor: u64,
    pub current: u64,
    pub successor: u64,
}

impl R37OrderedFrameV1 {
    pub const fn is_valid_for(self, current: u64) -> bool {
        self.predecessor < self.current && self.current < self.successor && self.current == current
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R37WaitBindingV1 {
    pub kind: R37CopyKindV1,
    pub submission: u64,
    pub stream: u64,
    pub source_allocation: u64,
    pub destination_allocation: u64,
    pub source_storage_generation: u64,
    pub destination_storage_generation: u64,
    pub restored_source_storage: u64,
    pub restored_destination_storage: u64,
    pub dependency_submission: u64,
    pub dependency_retain_count: u8,
    pub source_custody_count: u8,
    pub destination_custody_count: u8,
    pub stream_owner_count: u8,
    pub published_index_frame: R37OrderedFrameV1,
    pub stream_frame: R37OrderedFrameV1,
    pub native_identity: R37NativeIdentityV1,
}

impl R37WaitBindingV1 {
    pub const fn is_valid(self) -> bool {
        self.submission != 0
            && self.stream != 0
            && self.source_allocation != 0
            && self.destination_allocation != 0
            && self.source_allocation != self.destination_allocation
            && self.source_storage_generation != 0
            && self.destination_storage_generation != 0
            && self.restored_source_storage != 0
            && self.restored_destination_storage != 0
            && self.dependency_submission != 0
            && self.dependency_retain_count != 0
            && self.source_custody_count != 0
            && self.destination_custody_count != 0
            && self.stream_owner_count != 0
            && self.published_index_frame.is_valid_for(self.submission)
            && self.stream_frame.is_valid_for(self.submission)
            && self.native_identity.is_valid()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37NativeWaitObservationV1 {
    Complete,
    ExactTypedTimeout(R37NativeIdentityV1),
    NonTimeoutRetryable(R37NativeIdentityV1),
    IdentityChange {
        stage: R37IdentityChangeStageV1,
        returned: R37NativeIdentityV1,
    },
    Teardown {
        terminal_token: u64,
    },
}

impl R37NativeWaitObservationV1 {
    /// Input-only observation contract. It invokes no transition and compares
    /// no output state.
    pub const fn is_valid_for(self, binding: R37WaitBindingV1) -> bool {
        match self {
            Self::Complete => true,
            Self::ExactTypedTimeout(returned) | Self::NonTimeoutRetryable(returned) => {
                returned.owner_id == binding.native_identity.owner_id
                    && returned.request_id == binding.native_identity.request_id
            }
            Self::IdentityChange { returned, .. } => {
                returned.is_valid()
                    && (returned.owner_id != binding.native_identity.owner_id
                        || returned.request_id != binding.native_identity.request_id)
            }
            Self::Teardown { terminal_token } => terminal_token != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37CallV1 {
    Poll,
    Wait,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37EntryPhaseV1 {
    PublishedDirectional,
    PublishedSameDevice,
    Ready,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37RouteV1 {
    Poll,
    LegacyWaitPoll,
    NativeDirectionalWait,
    NativeSameDeviceWait,
}

pub const fn r37_route_model_only(call: R37CallV1, phase: R37EntryPhaseV1) -> R37RouteV1 {
    match (call, phase) {
        (R37CallV1::Poll, _) => R37RouteV1::Poll,
        (R37CallV1::Wait, R37EntryPhaseV1::PublishedDirectional) => {
            R37RouteV1::NativeDirectionalWait
        }
        (R37CallV1::Wait, R37EntryPhaseV1::PublishedSameDevice) => R37RouteV1::NativeSameDeviceWait,
        (R37CallV1::Wait, R37EntryPhaseV1::Ready | R37EntryPhaseV1::Other) => {
            R37RouteV1::LegacyWaitPoll
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37OutcomeV1 {
    Pending,
    Succeeded,
    Terminal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37ActivePhaseV1 {
    Published(R37CopyKindV1),
    Ready,
    Absent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37StorageV1 {
    InFlight { submission: u64, generation: u64 },
    Restored { storage_token: u64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37NativeCustodyV1 {
    ActivePublished(R37NativeIdentityV1),
    RestoredPair,
    TerminalPending(R37NativeIdentityV1),
    TerminalCompleted(R37NativeIdentityV1),
    TerminalTeardown(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R37WaitSnapshotV1 {
    pub binding: R37WaitBindingV1,
    pub route: R37RouteV1,
    pub outcome: R37OutcomeV1,
    pub active_present: bool,
    pub active_phase: R37ActivePhaseV1,
    pub published_index_retained: bool,
    pub published_index_frame: R37OrderedFrameV1,
    pub source_storage: R37StorageV1,
    pub destination_storage: R37StorageV1,
    pub dependency_retain_count: u8,
    pub source_custody_count: u8,
    pub destination_custody_count: u8,
    pub stream_owner_count: u8,
    pub stream_current_retained: bool,
    pub stream_frame: R37OrderedFrameV1,
    pub native_custody: R37NativeCustodyV1,
    pub terminal_poisoned: bool,
    pub native_observation_count: u8,
    pub settled: bool,
    pub completion_recorded: bool,
    pub continuation_ready: bool,
    pub continuation_publication_count: u8,
}

impl R37WaitSnapshotV1 {
    /// Exact modeled operational custody, excluding result and observation
    /// fields that necessarily change when the wait is performed.
    pub fn same_operational_custody(&self, other: &Self) -> bool {
        self.binding == other.binding
            && self.active_present == other.active_present
            && self.active_phase == other.active_phase
            && self.published_index_retained == other.published_index_retained
            && self.published_index_frame == other.published_index_frame
            && self.source_storage == other.source_storage
            && self.destination_storage == other.destination_storage
            && self.dependency_retain_count == other.dependency_retain_count
            && self.source_custody_count == other.source_custody_count
            && self.destination_custody_count == other.destination_custody_count
            && self.stream_owner_count == other.stream_owner_count
            && self.stream_current_retained == other.stream_current_retained
            && self.stream_frame == other.stream_frame
            && self.native_custody == other.native_custody
    }

    pub fn terminal_preserves_in_flight_retains(&self) -> bool {
        self.source_storage
            == (R37StorageV1::InFlight {
                submission: self.binding.submission,
                generation: self.binding.source_storage_generation,
            })
            && self.destination_storage
                == (R37StorageV1::InFlight {
                    submission: self.binding.submission,
                    generation: self.binding.destination_storage_generation,
                })
            && self.dependency_retain_count == self.binding.dependency_retain_count
            && self.source_custody_count == self.binding.source_custody_count
            && self.destination_custody_count == self.binding.destination_custody_count
            && self.stream_owner_count == self.binding.stream_owner_count
            && self.stream_current_retained
            && self.stream_frame.predecessor == self.binding.stream_frame.predecessor
            && self.stream_frame.current == self.binding.stream_frame.current
            && self.stream_frame.successor == self.binding.stream_frame.successor
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R37ModelErrorV1 {
    InvalidBinding,
    InvalidObservation,
}

struct R37PublishedAuthorityV1 {
    binding: R37WaitBindingV1,
}

/// Move-only owner of one model-only Published native wait authority.
///
/// ```compile_fail
/// use fe2o3_runtime_model::{R37CopyKindV1, R37NativeIdentityV1,
///     R37OrderedFrameV1, R37TypedNativeSdmaWaitModelV1, R37WaitBindingV1};
/// let binding = R37WaitBindingV1 {
///     kind: R37CopyKindV1::Directional, submission: 13, stream: 17,
///     source_allocation: 19, destination_allocation: 23,
///     source_storage_generation: 29, destination_storage_generation: 31,
///     restored_source_storage: 37, restored_destination_storage: 41,
///     dependency_submission: 43, dependency_retain_count: 1,
///     source_custody_count: 1, destination_custody_count: 1,
///     stream_owner_count: 3,
///     published_index_frame: R37OrderedFrameV1 {
///         predecessor: 11, current: 13, successor: 47,
///     },
///     stream_frame: R37OrderedFrameV1 {
///         predecessor: 7, current: 13, successor: 53,
///     },
///     native_identity: R37NativeIdentityV1 { owner_id: 59, request_id: 61 },
/// };
/// let owner = R37TypedNativeSdmaWaitModelV1::new_model_only(binding).unwrap();
/// let duplicated = owner.clone();
/// # let _ = duplicated;
/// ```
pub struct R37TypedNativeSdmaWaitModelV1 {
    published: R37PublishedAuthorityV1,
}

impl R37TypedNativeSdmaWaitModelV1 {
    pub fn new_model_only(binding: R37WaitBindingV1) -> Result<Self, R37ModelErrorV1> {
        if !binding.is_valid() {
            return Err(R37ModelErrorV1::InvalidBinding);
        }
        Ok(Self {
            published: R37PublishedAuthorityV1 { binding },
        })
    }

    pub fn initial_snapshot_model_only(&self) -> R37WaitSnapshotV1 {
        initial_snapshot(self.published.binding)
    }

    pub fn run_model_only(
        self,
        deadline: R37DeadlineClassV1,
        observation: R37NativeWaitObservationV1,
        completion: R37CompletionDispositionV1,
    ) -> Result<R37WaitSnapshotV1, R37ModelErrorV1> {
        let binding = self.published.binding;
        if !observation.is_valid_for(binding) {
            return Err(R37ModelErrorV1::InvalidObservation);
        }
        Ok(execute_wait(binding, deadline, observation, completion))
    }
}

fn initial_snapshot(binding: R37WaitBindingV1) -> R37WaitSnapshotV1 {
    R37WaitSnapshotV1 {
        binding,
        route: match binding.kind {
            R37CopyKindV1::Directional => R37RouteV1::NativeDirectionalWait,
            R37CopyKindV1::SameDevice => R37RouteV1::NativeSameDeviceWait,
        },
        outcome: R37OutcomeV1::Pending,
        active_present: true,
        active_phase: R37ActivePhaseV1::Published(binding.kind),
        published_index_retained: true,
        published_index_frame: binding.published_index_frame,
        source_storage: R37StorageV1::InFlight {
            submission: binding.submission,
            generation: binding.source_storage_generation,
        },
        destination_storage: R37StorageV1::InFlight {
            submission: binding.submission,
            generation: binding.destination_storage_generation,
        },
        dependency_retain_count: binding.dependency_retain_count,
        source_custody_count: binding.source_custody_count,
        destination_custody_count: binding.destination_custody_count,
        stream_owner_count: binding.stream_owner_count,
        stream_current_retained: true,
        stream_frame: binding.stream_frame,
        native_custody: R37NativeCustodyV1::ActivePublished(binding.native_identity),
        terminal_poisoned: false,
        native_observation_count: 0,
        settled: false,
        completion_recorded: false,
        continuation_ready: false,
        continuation_publication_count: 0,
    }
}

fn terminal_snapshot(
    mut state: R37WaitSnapshotV1,
    custody: R37NativeCustodyV1,
) -> R37WaitSnapshotV1 {
    state.outcome = R37OutcomeV1::Terminal;
    state.active_present = false;
    state.active_phase = R37ActivePhaseV1::Absent;
    state.published_index_retained = false;
    state.native_custody = custody;
    state.terminal_poisoned = true;
    state
}

fn restored_snapshot(mut state: R37WaitSnapshotV1) -> R37WaitSnapshotV1 {
    state.source_storage = R37StorageV1::Restored {
        storage_token: state.binding.restored_source_storage,
    };
    state.destination_storage = R37StorageV1::Restored {
        storage_token: state.binding.restored_destination_storage,
    };
    state.native_custody = R37NativeCustodyV1::RestoredPair;
    state.published_index_retained = false;
    state
}

fn execute_wait(
    binding: R37WaitBindingV1,
    _deadline: R37DeadlineClassV1,
    observation: R37NativeWaitObservationV1,
    completion: R37CompletionDispositionV1,
) -> R37WaitSnapshotV1 {
    let mut state = initial_snapshot(binding);
    state.native_observation_count = 1;
    match observation {
        R37NativeWaitObservationV1::Complete => {
            state = restored_snapshot(state);
            match completion {
                R37CompletionDispositionV1::Settle => {
                    state.outcome = R37OutcomeV1::Succeeded;
                    state.active_present = false;
                    state.active_phase = R37ActivePhaseV1::Absent;
                    state.dependency_retain_count -= 1;
                    state.source_custody_count -= 1;
                    state.destination_custody_count -= 1;
                    state.stream_owner_count -= 1;
                    state.stream_current_retained = false;
                    state.settled = true;
                    state.completion_recorded = true;
                }
                R37CompletionDispositionV1::ContinuationReady => {
                    state.outcome = R37OutcomeV1::Pending;
                    state.active_present = true;
                    state.active_phase = R37ActivePhaseV1::Ready;
                    state.continuation_ready = true;
                }
            }
            state
        }
        R37NativeWaitObservationV1::ExactTypedTimeout(_) => state,
        R37NativeWaitObservationV1::NonTimeoutRetryable(returned) => {
            terminal_snapshot(state, R37NativeCustodyV1::TerminalPending(returned))
        }
        R37NativeWaitObservationV1::IdentityChange { stage, returned } => {
            let custody = match stage {
                R37IdentityChangeStageV1::Pending => R37NativeCustodyV1::TerminalPending(returned),
                R37IdentityChangeStageV1::Completed => {
                    R37NativeCustodyV1::TerminalCompleted(returned)
                }
            };
            terminal_snapshot(state, custody)
        }
        R37NativeWaitObservationV1::Teardown { terminal_token } => {
            terminal_snapshot(state, R37NativeCustodyV1::TerminalTeardown(terminal_token))
        }
    }
}

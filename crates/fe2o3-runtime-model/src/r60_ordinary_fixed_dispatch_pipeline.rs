//! Executable bounded R60 model for ordinary fixed-dispatch pipelining.
//!
//! This model supersedes the R13 scheduler for this narrow pipeline surface.
//! It separates completion-only lane ordering from explicit success
//! dependencies and carries exact recipe, storage, epoch, slot, generation,
//! and custody identities through a fixed 64-entry roster. It performs no I/O
//! and grants no runtime, KFD, AQL, HSA, HIP, hardware, refinement, parity, or
//! performance authority.

use alloc::{boxed::Box, collections::VecDeque, vec::Vec};

pub const R60_MAX_EPOCHS_PER_LANE_V1: usize = 64;
pub const R60_MAX_EXPLICIT_SUCCESS_DEPENDENCIES_V1: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60PipelineModelErrorV1 {
    InvalidIdentity,
    CapacityExceeded,
    DuplicateSubmission,
    UnknownEpoch,
    StaleEpoch,
    NotLaneTail,
    UnsupportedExecutionClass,
    ExplicitDependencyNotSucceeded,
    OrderedPredecessorNotPublicationCapable,
    OrderedWaitMismatch,
    RecipeOrStorageMismatch,
    IllegalTransition,
    TooLate,
    NotCurrent,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R60LaneIdentityV1 {
    pub device_id: u64,
    pub device_generation: u64,
    pub lane_id: u64,
    pub lane_generation: u64,
}

impl R60LaneIdentityV1 {
    pub const fn valid_model_only(self) -> bool {
        self.device_id != 0
            && self.device_generation != 0
            && self.lane_id != 0
            && self.lane_generation != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R60EpochIdentityV1 {
    lane: R60LaneIdentityV1,
    slot: u8,
    slot_generation: u64,
    logical_epoch: u64,
    submission_id: u64,
}

impl R60EpochIdentityV1 {
    pub const fn lane(self) -> R60LaneIdentityV1 {
        self.lane
    }

    pub const fn slot(self) -> u8 {
        self.slot
    }

    pub const fn slot_generation(self) -> u64 {
        self.slot_generation
    }

    pub const fn logical_epoch(self) -> u64 {
        self.logical_epoch
    }

    pub const fn submission_id(self) -> u64 {
        self.submission_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60ExecutionClassV1 {
    OrdinaryFixedDispatch,
    PersistentN1,
    ThreeBindingPersistentN3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R60RecipeStorageFingerprintV1 {
    pub kernel_id: u64,
    pub module_sha256: [u8; 32],
    pub dispatch_shape_sha256: [u8; 32],
    pub storage_sha256: [u8; 32],
    pub bindings_sha256: [u8; 32],
}

impl R60RecipeStorageFingerprintV1 {
    pub const fn valid_model_only(self) -> bool {
        self.kernel_id != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60TerminalStatusV1 {
    Succeeded,
    Failed { code: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60PipelinePhaseV1 {
    Queued,
    Prepared,
    Published,
    Completed,
    PhysicallyRetired,
    HostCommitted,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60PublicationScriptV1 {
    Complete,
    RetryableNoEffect,
    Indeterminate,
}

/// Exact ordering authority presented when a prepared epoch is published.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60PublicationOrderingWitnessV1 {
    NoPredecessor,
    CompletionObserved(R60EpochIdentityV1),
    WaitForPrior(R60EpochIdentityV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60PublicationOutcomeV1 {
    Published,
    Retryable,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60CompletionObservationV1 {
    Pending,
    Exact(R60TerminalStatusV1),
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60CompletionOutcomeV1 {
    Pending,
    Completed,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60RetirementObservationV1 {
    Exact,
    RetryableRestoredExact,
    RestoreFailed,
    Indeterminate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60RetirementOutcomeV1 {
    PhysicallyRetired,
    Retryable,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R60CancellationV1 {
    Cancelled,
    TooLate,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R60HostObservationV1 {
    pub status: R60TerminalStatusV1,
    pub committed_effects: u64,
    pub profile_visible: bool,
    pub custody_released: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R60PipelineSnapshotV1 {
    pub live_epochs: usize,
    pub next_logical_epoch: Option<u64>,
    pub native_publications: u64,
    pub physical_retirements: u64,
    pub host_commits: u64,
    pub cancellations: u64,
    pub deferred_predecessor_retains: usize,
    pub quarantined: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct R60EpochRecordV1 {
    identity: R60EpochIdentityV1,
    class: R60ExecutionClassV1,
    ordered_predecessor: Option<R60EpochIdentityV1>,
    publication_ordering_witness: Option<R60PublicationOrderingWitnessV1>,
    explicit_success_dependencies: Vec<R60EpochIdentityV1>,
    recipe_storage: R60RecipeStorageFingerprintV1,
    phase: R60PipelinePhaseV1,
    terminal_status: Option<R60TerminalStatusV1>,
    owns_custody: bool,
    pending_ordered_predecessor_retain: bool,
    deferred_ordered_predecessor_retain: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct R60PipelineSlotV1 {
    generation: u64,
    entry: Option<R60EpochRecordV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct R60HostReceiptV1 {
    identity: R60EpochIdentityV1,
    recipe_storage: R60RecipeStorageFingerprintV1,
    ordered_predecessor: Option<R60EpochIdentityV1>,
    publication_ordering_witness: R60PublicationOrderingWitnessV1,
    status: R60TerminalStatusV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R60DeferredPredecessorRetainV1 {
    predecessor: R60EpochIdentityV1,
    successor: R60EpochIdentityV1,
}

/// One caller-constructible physical-lane model with a fixed epoch roster.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R60OrdinaryFixedDispatchPipelineModelV1 {
    lane: R60LaneIdentityV1,
    slots: Box<[R60PipelineSlotV1; R60_MAX_EPOCHS_PER_LANE_V1]>,
    order: VecDeque<R60EpochIdentityV1>,
    receipts: Vec<R60HostReceiptV1>,
    predecessor_retains: Vec<R60DeferredPredecessorRetainV1>,
    stream_tail: Option<R60EpochIdentityV1>,
    live: usize,
    next_logical_epoch: Option<u64>,
    native_publications: u64,
    physical_retirements: u64,
    host_commits: u64,
    cancellations: u64,
    current: bool,
    quarantined: bool,
}

impl R60OrdinaryFixedDispatchPipelineModelV1 {
    pub fn new_model_only(lane: R60LaneIdentityV1) -> Result<Self, R60PipelineModelErrorV1> {
        if !lane.valid_model_only() {
            return Err(R60PipelineModelErrorV1::InvalidIdentity);
        }
        let slots = Box::new(core::array::from_fn(|_| R60PipelineSlotV1 {
            generation: 0,
            entry: None,
        }));
        Ok(Self {
            lane,
            slots,
            order: VecDeque::new(),
            receipts: Vec::new(),
            predecessor_retains: Vec::new(),
            stream_tail: None,
            live: 0,
            next_logical_epoch: Some(1),
            native_publications: 0,
            physical_retirements: 0,
            host_commits: 0,
            cancellations: 0,
            current: true,
            quarantined: false,
        })
    }

    pub const fn lane_model_only(&self) -> R60LaneIdentityV1 {
        self.lane
    }

    pub const fn live_epoch_count_model_only(&self) -> usize {
        self.live
    }

    pub const fn current_model_only(&self) -> bool {
        self.current
    }

    pub const fn quarantined_model_only(&self) -> bool {
        self.quarantined
    }

    pub fn snapshot_model_only(&self) -> R60PipelineSnapshotV1 {
        R60PipelineSnapshotV1 {
            live_epochs: self.live,
            next_logical_epoch: self.next_logical_epoch,
            native_publications: self.native_publications,
            physical_retirements: self.physical_retirements,
            host_commits: self.host_commits,
            cancellations: self.cancellations,
            deferred_predecessor_retains: self.predecessor_retains.len(),
            quarantined: self.quarantined,
        }
    }

    pub fn phase_model_only(&self, identity: R60EpochIdentityV1) -> Option<R60PipelinePhaseV1> {
        if self.receipt(identity).is_some() {
            return Some(R60PipelinePhaseV1::HostCommitted);
        }
        self.entry(identity).map(|entry| entry.phase)
    }

    pub fn recipe_storage_model_only(
        &self,
        identity: R60EpochIdentityV1,
    ) -> Option<R60RecipeStorageFingerprintV1> {
        self.entry(identity)
            .map(|entry| entry.recipe_storage)
            .or_else(|| self.receipt(identity).map(|receipt| receipt.recipe_storage))
    }

    pub fn ordered_predecessor_model_only(
        &self,
        identity: R60EpochIdentityV1,
    ) -> Option<Option<R60EpochIdentityV1>> {
        self.entry(identity)
            .map(|entry| entry.ordered_predecessor)
            .or_else(|| {
                self.receipt(identity)
                    .map(|receipt| receipt.ordered_predecessor)
            })
    }

    pub fn publication_ordering_witness_model_only(
        &self,
        identity: R60EpochIdentityV1,
    ) -> Option<Option<R60PublicationOrderingWitnessV1>> {
        self.entry(identity)
            .map(|entry| entry.publication_ordering_witness)
            .or_else(|| {
                self.receipt(identity)
                    .map(|receipt| Some(receipt.publication_ordering_witness))
            })
    }

    pub fn explicit_success_dependencies_model_only(
        &self,
        identity: R60EpochIdentityV1,
    ) -> Option<&[R60EpochIdentityV1]> {
        self.entry(identity)
            .map(|entry| entry.explicit_success_dependencies.as_slice())
    }

    pub fn owns_custody_model_only(&self, identity: R60EpochIdentityV1) -> Option<bool> {
        self.entry(identity).map(|entry| entry.owns_custody)
    }

    pub fn predecessor_retain_count_model_only(&self, predecessor: R60EpochIdentityV1) -> usize {
        self.predecessor_retains
            .iter()
            .filter(|retain| retain.predecessor == predecessor)
            .count()
    }

    pub fn host_observation_model_only(
        &self,
        identity: R60EpochIdentityV1,
    ) -> Option<R60HostObservationV1> {
        self.receipt(identity).map(|receipt| R60HostObservationV1 {
            status: receipt.status,
            committed_effects: 1,
            profile_visible: true,
            custody_released: true,
        })
    }

    pub fn enqueue_model_only(
        &mut self,
        submission_id: u64,
        class: R60ExecutionClassV1,
        ordered_predecessor: Option<R60EpochIdentityV1>,
        explicit_success_dependencies: &[R60EpochIdentityV1],
        recipe_storage: R60RecipeStorageFingerprintV1,
    ) -> Result<R60EpochIdentityV1, R60PipelineModelErrorV1> {
        if !self.current || self.quarantined {
            return Err(R60PipelineModelErrorV1::NotCurrent);
        }
        if submission_id == 0 || !recipe_storage.valid_model_only() {
            return Err(R60PipelineModelErrorV1::InvalidIdentity);
        }
        if class != R60ExecutionClassV1::OrdinaryFixedDispatch {
            return Err(R60PipelineModelErrorV1::UnsupportedExecutionClass);
        }
        if self.live >= R60_MAX_EPOCHS_PER_LANE_V1
            || explicit_success_dependencies.len() > R60_MAX_EXPLICIT_SUCCESS_DEPENDENCIES_V1
        {
            return Err(R60PipelineModelErrorV1::CapacityExceeded);
        }
        if self
            .slots
            .iter()
            .filter_map(|slot| slot.entry.as_ref())
            .any(|entry| entry.identity.submission_id == submission_id)
            || self
                .receipts
                .iter()
                .any(|receipt| receipt.identity.submission_id == submission_id)
        {
            return Err(R60PipelineModelErrorV1::DuplicateSubmission);
        }
        if explicit_success_dependencies
            .iter()
            .enumerate()
            .any(|(index, dependency)| {
                dependency.lane != self.lane
                    || explicit_success_dependencies[..index].contains(dependency)
                    || !self.known_identity(*dependency)
            })
        {
            return Err(R60PipelineModelErrorV1::InvalidIdentity);
        }
        match (self.stream_tail, ordered_predecessor) {
            (None, None) => {}
            (Some(tail), Some(predecessor))
                if tail == predecessor && self.known_identity(predecessor) => {}
            _ => return Err(R60PipelineModelErrorV1::NotLaneTail),
        }

        let logical_epoch = self
            .next_logical_epoch
            .ok_or(R60PipelineModelErrorV1::CapacityExceeded)?;
        let Some((slot_index, slot_generation)) =
            self.slots.iter().enumerate().find_map(|(index, slot)| {
                if slot.entry.is_some() {
                    return None;
                }
                slot.generation
                    .checked_add(1)
                    .filter(|generation| *generation != 0)
                    .map(|generation| (index, generation))
            })
        else {
            return Err(R60PipelineModelErrorV1::CapacityExceeded);
        };
        let identity = R60EpochIdentityV1 {
            lane: self.lane,
            slot: u8::try_from(slot_index).expect("R60 fixed roster contains exactly 64 entries"),
            slot_generation,
            logical_epoch,
            submission_id,
        };
        let pending_ordered_predecessor_retain = ordered_predecessor.is_some();
        self.slots[slot_index].generation = slot_generation;
        self.slots[slot_index].entry = Some(R60EpochRecordV1 {
            identity,
            class,
            ordered_predecessor,
            publication_ordering_witness: None,
            explicit_success_dependencies: explicit_success_dependencies.to_vec(),
            recipe_storage,
            phase: R60PipelinePhaseV1::Queued,
            terminal_status: None,
            owns_custody: true,
            pending_ordered_predecessor_retain,
            deferred_ordered_predecessor_retain: false,
        });
        self.order.push_back(identity);
        self.stream_tail = Some(identity);
        if let Some(predecessor) = ordered_predecessor {
            self.predecessor_retains
                .push(R60DeferredPredecessorRetainV1 {
                    predecessor,
                    successor: identity,
                });
        }
        self.live += 1;
        self.next_logical_epoch = logical_epoch.checked_add(1);
        Ok(identity)
    }

    pub fn prepare_model_only(
        &mut self,
        identity: R60EpochIdentityV1,
        observed_recipe_storage: R60RecipeStorageFingerprintV1,
    ) -> Result<(), R60PipelineModelErrorV1> {
        self.require_exact_identity(identity)?;
        let entry = self
            .entry_mut(identity)
            .ok_or(R60PipelineModelErrorV1::UnknownEpoch)?;
        if entry.phase != R60PipelinePhaseV1::Queued {
            return Err(R60PipelineModelErrorV1::IllegalTransition);
        }
        if entry.recipe_storage != observed_recipe_storage {
            return Err(R60PipelineModelErrorV1::RecipeOrStorageMismatch);
        }
        entry.phase = R60PipelinePhaseV1::Prepared;
        Ok(())
    }

    pub fn publish_model_only(
        &mut self,
        identity: R60EpochIdentityV1,
        observed_recipe_storage: R60RecipeStorageFingerprintV1,
        ordering_witness: R60PublicationOrderingWitnessV1,
        script: R60PublicationScriptV1,
    ) -> Result<R60PublicationOutcomeV1, R60PipelineModelErrorV1> {
        self.require_exact_identity(identity)?;
        if !self.current || self.quarantined {
            return Err(R60PipelineModelErrorV1::NotCurrent);
        }
        let entry = self
            .entry(identity)
            .ok_or(R60PipelineModelErrorV1::UnknownEpoch)?;
        if entry.phase != R60PipelinePhaseV1::Prepared {
            return Err(R60PipelineModelErrorV1::IllegalTransition);
        }
        if entry.class != R60ExecutionClassV1::OrdinaryFixedDispatch {
            return Err(R60PipelineModelErrorV1::UnsupportedExecutionClass);
        }
        if entry.recipe_storage != observed_recipe_storage {
            return Err(R60PipelineModelErrorV1::RecipeOrStorageMismatch);
        }
        match (entry.ordered_predecessor, ordering_witness) {
            (None, R60PublicationOrderingWitnessV1::NoPredecessor) => {}
            (Some(expected), R60PublicationOrderingWitnessV1::CompletionObserved(observed))
                if expected == observed && self.completed_internal(expected) => {}
            (Some(expected), R60PublicationOrderingWitnessV1::WaitForPrior(observed))
                if expected == observed
                    && self.publication_chain_predecessor(expected, entry.recipe_storage) => {}
            (Some(expected), R60PublicationOrderingWitnessV1::CompletionObserved(observed))
            | (Some(expected), R60PublicationOrderingWitnessV1::WaitForPrior(observed))
                if expected != observed =>
            {
                return Err(R60PipelineModelErrorV1::OrderedWaitMismatch);
            }
            (Some(_), R60PublicationOrderingWitnessV1::NoPredecessor)
            | (None, R60PublicationOrderingWitnessV1::CompletionObserved(_))
            | (None, R60PublicationOrderingWitnessV1::WaitForPrior(_)) => {
                return Err(R60PipelineModelErrorV1::OrderedWaitMismatch);
            }
            _ => {
                return Err(R60PipelineModelErrorV1::OrderedPredecessorNotPublicationCapable);
            }
        }
        if entry
            .explicit_success_dependencies
            .iter()
            .any(|dependency| !self.succeeded_internal(*dependency))
        {
            return Err(R60PipelineModelErrorV1::ExplicitDependencyNotSucceeded);
        }

        match script {
            R60PublicationScriptV1::RetryableNoEffect => Ok(R60PublicationOutcomeV1::Retryable),
            R60PublicationScriptV1::Complete => {
                let next_native_publications = self
                    .native_publications
                    .checked_add(1)
                    .ok_or(R60PipelineModelErrorV1::CapacityExceeded)?;
                let release_pending_retain = matches!(
                    ordering_witness,
                    R60PublicationOrderingWitnessV1::CompletionObserved(_)
                );
                {
                    let entry = self.entry_mut(identity).expect("exact live identity");
                    entry.phase = R60PipelinePhaseV1::Published;
                    entry.publication_ordering_witness = Some(ordering_witness);
                    match ordering_witness {
                        R60PublicationOrderingWitnessV1::WaitForPrior(_) => {
                            entry.pending_ordered_predecessor_retain = false;
                            entry.deferred_ordered_predecessor_retain = true;
                        }
                        R60PublicationOrderingWitnessV1::CompletionObserved(_) => {
                            entry.pending_ordered_predecessor_retain = false;
                            entry.deferred_ordered_predecessor_retain = false;
                        }
                        R60PublicationOrderingWitnessV1::NoPredecessor => {}
                    }
                }
                if release_pending_retain {
                    self.release_exact_predecessor_retain(identity)?;
                }
                self.native_publications = next_native_publications;
                Ok(R60PublicationOutcomeV1::Published)
            }
            R60PublicationScriptV1::Indeterminate => {
                self.quarantine_all_model_only();
                Ok(R60PublicationOutcomeV1::Quarantined)
            }
        }
    }

    pub fn observe_completion_model_only(
        &mut self,
        identity: R60EpochIdentityV1,
        observed_identity: R60EpochIdentityV1,
        observed_recipe_storage: R60RecipeStorageFingerprintV1,
        observation: R60CompletionObservationV1,
    ) -> Result<R60CompletionOutcomeV1, R60PipelineModelErrorV1> {
        self.require_exact_identity(identity)?;
        let entry = self
            .entry(identity)
            .ok_or(R60PipelineModelErrorV1::UnknownEpoch)?;
        if entry.phase != R60PipelinePhaseV1::Published {
            return Err(R60PipelineModelErrorV1::IllegalTransition);
        }
        if observed_identity != identity || observed_recipe_storage != entry.recipe_storage {
            self.quarantine_all_model_only();
            return Ok(R60CompletionOutcomeV1::Quarantined);
        }
        match observation {
            R60CompletionObservationV1::Pending => Ok(R60CompletionOutcomeV1::Pending),
            R60CompletionObservationV1::Exact(status) => {
                let entry = self.entry_mut(identity).expect("exact live identity");
                entry.phase = R60PipelinePhaseV1::Completed;
                entry.terminal_status = Some(status);
                Ok(R60CompletionOutcomeV1::Completed)
            }
            R60CompletionObservationV1::Indeterminate => {
                self.quarantine_all_model_only();
                Ok(R60CompletionOutcomeV1::Quarantined)
            }
        }
    }

    pub fn retire_physical_model_only(
        &mut self,
        identity: R60EpochIdentityV1,
        observed_identity: R60EpochIdentityV1,
        observed_recipe_storage: R60RecipeStorageFingerprintV1,
        observation: R60RetirementObservationV1,
    ) -> Result<R60RetirementOutcomeV1, R60PipelineModelErrorV1> {
        self.require_exact_identity(identity)?;
        let entry = self
            .entry(identity)
            .ok_or(R60PipelineModelErrorV1::UnknownEpoch)?;
        if entry.phase != R60PipelinePhaseV1::Completed {
            return Err(R60PipelineModelErrorV1::IllegalTransition);
        }
        if observed_identity != identity || observed_recipe_storage != entry.recipe_storage {
            self.quarantine_all_model_only();
            return Ok(R60RetirementOutcomeV1::Quarantined);
        }
        match observation {
            R60RetirementObservationV1::Exact => {
                let next_physical_retirements = self
                    .physical_retirements
                    .checked_add(1)
                    .ok_or(R60PipelineModelErrorV1::CapacityExceeded)?;
                self.entry_mut(identity).expect("exact live identity").phase =
                    R60PipelinePhaseV1::PhysicallyRetired;
                self.physical_retirements = next_physical_retirements;
                Ok(R60RetirementOutcomeV1::PhysicallyRetired)
            }
            R60RetirementObservationV1::RetryableRestoredExact => {
                Ok(R60RetirementOutcomeV1::Retryable)
            }
            R60RetirementObservationV1::RestoreFailed
            | R60RetirementObservationV1::Indeterminate => {
                self.quarantine_all_model_only();
                Ok(R60RetirementOutcomeV1::Quarantined)
            }
        }
    }

    /// Publishes only the maximal physically-retired prefix to the host.
    pub fn commit_contiguous_model_only(
        &mut self,
    ) -> Result<Vec<R60EpochIdentityV1>, R60PipelineModelErrorV1> {
        if self.quarantined {
            return Err(R60PipelineModelErrorV1::NotCurrent);
        }
        let prefix_len = self
            .order
            .iter()
            .take_while(|identity| {
                self.phase_model_only(**identity) == Some(R60PipelinePhaseV1::PhysicallyRetired)
            })
            .count();
        let next_host_commits = self
            .host_commits
            .checked_add(
                u64::try_from(prefix_len).map_err(|_| R60PipelineModelErrorV1::CapacityExceeded)?,
            )
            .ok_or(R60PipelineModelErrorV1::CapacityExceeded)?;
        let mut committed = Vec::new();
        committed
            .try_reserve_exact(prefix_len)
            .map_err(|_| R60PipelineModelErrorV1::CapacityExceeded)?;
        self.receipts
            .try_reserve(prefix_len)
            .map_err(|_| R60PipelineModelErrorV1::CapacityExceeded)?;
        while let Some(frontier) = self.order.front().copied() {
            if self.phase_model_only(frontier) != Some(R60PipelinePhaseV1::PhysicallyRetired) {
                break;
            }
            let slot = &mut self.slots[usize::from(frontier.slot)];
            let entry = slot
                .entry
                .take()
                .ok_or(R60PipelineModelErrorV1::InvariantViolation)?;
            if entry.identity != frontier || !entry.owns_custody {
                self.quarantine_all_model_only();
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
            let status = entry
                .terminal_status
                .ok_or(R60PipelineModelErrorV1::InvariantViolation)?;
            self.order.pop_front();
            self.live = self
                .live
                .checked_sub(1)
                .ok_or(R60PipelineModelErrorV1::InvariantViolation)?;
            if entry.deferred_ordered_predecessor_retain {
                self.release_exact_predecessor_retain(frontier)?;
            }
            self.receipts.push(R60HostReceiptV1 {
                identity: frontier,
                recipe_storage: entry.recipe_storage,
                ordered_predecessor: entry.ordered_predecessor,
                publication_ordering_witness: entry
                    .publication_ordering_witness
                    .ok_or(R60PipelineModelErrorV1::InvariantViolation)?,
                status,
            });
            committed.push(frontier);
        }
        self.host_commits = next_host_commits;
        Ok(committed)
    }

    pub fn cancel_tail_model_only(
        &mut self,
        identity: R60EpochIdentityV1,
    ) -> Result<R60CancellationV1, R60PipelineModelErrorV1> {
        self.require_exact_identity(identity)?;
        if self.order.back().copied() != Some(identity) {
            return Err(R60PipelineModelErrorV1::NotLaneTail);
        }
        let phase = self
            .phase_model_only(identity)
            .ok_or(R60PipelineModelErrorV1::UnknownEpoch)?;
        if !matches!(
            phase,
            R60PipelinePhaseV1::Queued | R60PipelinePhaseV1::Prepared
        ) {
            return Ok(R60CancellationV1::TooLate);
        }
        let next_cancellations = self
            .cancellations
            .checked_add(1)
            .ok_or(R60PipelineModelErrorV1::CapacityExceeded)?;
        let slot = &mut self.slots[usize::from(identity.slot)];
        let entry = slot
            .entry
            .take()
            .ok_or(R60PipelineModelErrorV1::InvariantViolation)?;
        if entry.identity != identity || !entry.owns_custody {
            self.quarantine_all_model_only();
            return Err(R60PipelineModelErrorV1::InvariantViolation);
        }
        self.order.pop_back();
        self.stream_tail = entry.ordered_predecessor;
        self.live = self
            .live
            .checked_sub(1)
            .ok_or(R60PipelineModelErrorV1::InvariantViolation)?;
        if entry.pending_ordered_predecessor_retain {
            self.release_exact_predecessor_retain(identity)?;
        }
        self.cancellations = next_cancellations;
        Ok(R60CancellationV1::Cancelled)
    }

    pub fn lose_currentness_model_only(&mut self) {
        self.current = false;
        self.quarantine_all_model_only();
    }

    pub fn validate_global_invariants(&self) -> Result<(), R60PipelineModelErrorV1> {
        if !self.lane.valid_model_only()
            || self.live != self.order.len()
            || self.live > R60_MAX_EPOCHS_PER_LANE_V1
            || self.host_commits != self.receipts.len() as u64
            || self.quarantined && self.current
        {
            return Err(R60PipelineModelErrorV1::InvariantViolation);
        }
        match self.stream_tail {
            Some(tail) if !self.known_identity(tail) => {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
            None if !self.order.is_empty() || !self.receipts.is_empty() => {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
            _ => {}
        }
        let expected_tail = self
            .order
            .back()
            .copied()
            .or_else(|| self.receipts.last().map(|receipt| receipt.identity));
        if self.stream_tail != expected_tail {
            return Err(R60PipelineModelErrorV1::InvariantViolation);
        }
        let occupied = self
            .slots
            .iter()
            .filter(|slot| slot.entry.is_some())
            .count();
        if occupied != self.live {
            return Err(R60PipelineModelErrorV1::InvariantViolation);
        }
        for (order_index, identity) in self.order.iter().enumerate() {
            if identity.lane != self.lane
                || usize::from(identity.slot) >= R60_MAX_EPOCHS_PER_LANE_V1
                || self
                    .order
                    .iter()
                    .take(order_index)
                    .any(|prior| prior == identity)
                || order_index > 0
                    && self.order[order_index - 1].logical_epoch >= identity.logical_epoch
            {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
            let slot = &self.slots[usize::from(identity.slot)];
            let Some(entry) = slot.entry.as_ref() else {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            };
            if entry.identity != *identity
                || slot.generation != identity.slot_generation
                || entry.class != R60ExecutionClassV1::OrdinaryFixedDispatch
                || !entry.recipe_storage.valid_model_only()
                || !entry.owns_custody
                || matches!(entry.phase, R60PipelinePhaseV1::HostCommitted)
                || matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Queued
                        | R60PipelinePhaseV1::Prepared
                        | R60PipelinePhaseV1::Published
                ) && entry.terminal_status.is_some()
                || matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Completed | R60PipelinePhaseV1::PhysicallyRetired
                ) && entry.terminal_status.is_none()
                || self.quarantined && entry.phase != R60PipelinePhaseV1::Quarantined
                || entry.explicit_success_dependencies.len()
                    > R60_MAX_EXPLICIT_SUCCESS_DEPENDENCIES_V1
                || matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Queued | R60PipelinePhaseV1::Prepared
                ) && entry.publication_ordering_witness.is_some()
                || matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Published
                        | R60PipelinePhaseV1::Completed
                        | R60PipelinePhaseV1::PhysicallyRetired
                ) && !entry.publication_ordering_witness.is_some_and(|witness| {
                    Self::ordering_witness_binds(entry.ordered_predecessor, witness)
                })
                || entry.pending_ordered_predecessor_retain
                    && entry.deferred_ordered_predecessor_retain
                || matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Queued | R60PipelinePhaseV1::Prepared
                ) && entry.pending_ordered_predecessor_retain
                    != entry.ordered_predecessor.is_some()
                || matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Published
                        | R60PipelinePhaseV1::Completed
                        | R60PipelinePhaseV1::PhysicallyRetired
                ) && entry.pending_ordered_predecessor_retain
            {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
            if (entry.pending_ordered_predecessor_retain
                || entry.deferred_ordered_predecessor_retain)
                != self.predecessor_retains.iter().any(|retain| {
                    retain.predecessor == entry.ordered_predecessor.unwrap_or(*identity)
                        && retain.successor == *identity
                })
            {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
        }
        for (index, receipt) in self.receipts.iter().enumerate() {
            if receipt.identity.lane != self.lane
                || !Self::ordering_witness_binds(
                    receipt.ordered_predecessor,
                    receipt.publication_ordering_witness,
                )
                || self.receipts[..index]
                    .iter()
                    .any(|prior| prior.identity == receipt.identity)
                || self.entry(receipt.identity).is_some()
                || index > 0
                    && self.receipts[index - 1].identity.logical_epoch
                        >= receipt.identity.logical_epoch
            {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
        }
        for retain in &self.predecessor_retains {
            let Some(successor) = self.entry(retain.successor) else {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            };
            if successor.ordered_predecessor != Some(retain.predecessor)
                || !(successor.pending_ordered_predecessor_retain
                    || successor.deferred_ordered_predecessor_retain)
                || !self.known_identity(retain.predecessor)
                || self
                    .predecessor_retains
                    .iter()
                    .filter(|candidate| {
                        candidate.predecessor == retain.predecessor
                            && candidate.successor == retain.successor
                    })
                    .count()
                    != 1
            {
                return Err(R60PipelineModelErrorV1::InvariantViolation);
            }
        }
        Ok(())
    }

    fn entry(&self, identity: R60EpochIdentityV1) -> Option<&R60EpochRecordV1> {
        self.slots.get(usize::from(identity.slot)).and_then(|slot| {
            slot.entry
                .as_ref()
                .filter(|entry| entry.identity == identity)
        })
    }

    fn entry_mut(&mut self, identity: R60EpochIdentityV1) -> Option<&mut R60EpochRecordV1> {
        self.slots
            .get_mut(usize::from(identity.slot))
            .and_then(|slot| {
                slot.entry
                    .as_mut()
                    .filter(|entry| entry.identity == identity)
            })
    }

    fn receipt(&self, identity: R60EpochIdentityV1) -> Option<&R60HostReceiptV1> {
        self.receipts
            .iter()
            .find(|receipt| receipt.identity == identity)
    }

    fn known_identity(&self, identity: R60EpochIdentityV1) -> bool {
        identity.lane == self.lane
            && (self.entry(identity).is_some() || self.receipt(identity).is_some())
    }

    fn require_exact_identity(
        &self,
        identity: R60EpochIdentityV1,
    ) -> Result<(), R60PipelineModelErrorV1> {
        if identity.lane != self.lane || usize::from(identity.slot) >= R60_MAX_EPOCHS_PER_LANE_V1 {
            return Err(R60PipelineModelErrorV1::InvalidIdentity);
        }
        let slot = &self.slots[usize::from(identity.slot)];
        if slot
            .entry
            .as_ref()
            .is_some_and(|entry| entry.identity == identity)
        {
            return Ok(());
        }
        if slot.generation >= identity.slot_generation || self.receipt(identity).is_some() {
            Err(R60PipelineModelErrorV1::StaleEpoch)
        } else {
            Err(R60PipelineModelErrorV1::UnknownEpoch)
        }
    }

    fn publication_chain_predecessor(
        &self,
        identity: R60EpochIdentityV1,
        recipe_storage: R60RecipeStorageFingerprintV1,
    ) -> bool {
        self.entry(identity).is_some_and(|entry| {
            entry.recipe_storage == recipe_storage
                && entry.owns_custody
                && matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Published
                        | R60PipelinePhaseV1::Completed
                        | R60PipelinePhaseV1::PhysicallyRetired
                )
        })
    }

    fn completed_internal(&self, identity: R60EpochIdentityV1) -> bool {
        self.receipt(identity).is_some()
    }

    fn ordering_witness_binds(
        predecessor: Option<R60EpochIdentityV1>,
        witness: R60PublicationOrderingWitnessV1,
    ) -> bool {
        match (predecessor, witness) {
            (None, R60PublicationOrderingWitnessV1::NoPredecessor) => true,
            (
                Some(expected),
                R60PublicationOrderingWitnessV1::CompletionObserved(observed)
                | R60PublicationOrderingWitnessV1::WaitForPrior(observed),
            ) => expected == observed,
            _ => false,
        }
    }

    fn succeeded_internal(&self, identity: R60EpochIdentityV1) -> bool {
        self.receipt(identity)
            .is_some_and(|receipt| receipt.status == R60TerminalStatusV1::Succeeded)
            || self.entry(identity).is_some_and(|entry| {
                matches!(
                    entry.phase,
                    R60PipelinePhaseV1::Completed | R60PipelinePhaseV1::PhysicallyRetired
                ) && entry.terminal_status == Some(R60TerminalStatusV1::Succeeded)
            })
    }

    fn release_exact_predecessor_retain(
        &mut self,
        successor: R60EpochIdentityV1,
    ) -> Result<(), R60PipelineModelErrorV1> {
        let Some(index) = self
            .predecessor_retains
            .iter()
            .position(|retain| retain.successor == successor)
        else {
            return Err(R60PipelineModelErrorV1::InvariantViolation);
        };
        self.predecessor_retains.remove(index);
        Ok(())
    }

    fn quarantine_all_model_only(&mut self) {
        self.current = false;
        self.quarantined = true;
        for entry in self.slots.iter_mut().filter_map(|slot| slot.entry.as_mut()) {
            entry.phase = R60PipelinePhaseV1::Quarantined;
        }
    }

    #[cfg(test)]
    pub(crate) fn exhaust_vacant_slot_generations_for_test_v1(&mut self) {
        assert_eq!(self.live, 0);
        for slot in self.slots.iter_mut() {
            slot.generation = u64::MAX;
        }
    }

    #[cfg(test)]
    pub(crate) fn exhaust_logical_epochs_for_test_v1(&mut self) {
        assert_eq!(self.live, 0);
        self.next_logical_epoch = None;
    }

    #[cfg(test)]
    pub(crate) fn exhaust_host_commits_for_test_v1(&mut self) {
        self.host_commits = u64::MAX;
    }
}

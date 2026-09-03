//! Bounded model of native multi-queue compute concurrency.
//!
//! The model binds queue occurrences and slot generations to submission
//! custody. It admits publication only after dependencies succeed, accepts
//! terminal observations only for the exact queue occurrence and slot
//! generation, and quarantines published custody when device currentness is
//! lost. All values are caller-constructible abstract state: this module makes
//! no Rust-to-Verus refinement claim and grants no KFD, HSA, HIP, or hardware
//! execution authority.

use alloc::vec::Vec;

pub const MAX_R12_COMPUTE_QUEUES_V1: usize = 16;
pub const MAX_R12_SLOTS_PER_QUEUE_V1: usize = 64;
pub const MAX_R12_SUBMISSIONS_V1: usize = 4_096;
pub const MAX_R12_DEPENDENCIES_V1: usize = 256;
pub const MAX_R12_RESOURCES_PER_SUBMISSION_V1: usize = 64;
pub const MAX_R12_RESOURCES_V1: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R12ConcurrencyModelErrorV1 {
    InvalidIdentity,
    InvalidCapability,
    Unsupported,
    CapacityExceeded,
    DuplicateIdentity,
    UnknownQueue,
    UnknownSubmission,
    UnknownResource,
    ResourceBusy,
    DependencyNotReady,
    StaleIdentity,
    TooLate,
    NotCurrent,
    NotDrained,
    IllegalTransition,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R12DeviceKeyV1 {
    pub device_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R12MultiQueueCapabilityV1 {
    pub device: R12DeviceKeyV1,
    pub stable: bool,
    pub multi_queue_compute: bool,
    pub max_compute_queues: usize,
    pub max_slots_per_queue: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R12QueueOccurrenceV1 {
    pub device: R12DeviceKeyV1,
    pub queue_id: u64,
    pub occurrence: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R12SlotKeyV1 {
    pub queue: R12QueueOccurrenceV1,
    pub slot_index: usize,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R12StreamKeyV1 {
    pub device: R12DeviceKeyV1,
    pub stream_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R12SubmissionKeyV1 {
    pub stream: R12StreamKeyV1,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R12ResourceKeyV1 {
    pub device: R12DeviceKeyV1,
    pub resource_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R12TerminalStatusV1 {
    Succeeded,
    Failed { code: i64 },
    QuiescentWithoutResult,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R12SubmissionPhaseV1 {
    Reserved,
    Published,
    Terminal(R12TerminalStatusV1),
    CancelledBeforePublication,
    Indeterminate,
    Released,
}

impl R12SubmissionPhaseV1 {
    pub const fn retains_custody(self) -> bool {
        matches!(
            self,
            Self::Reserved | Self::Published | Self::Terminal(_) | Self::Indeterminate
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R12SlotPhaseV1 {
    Free,
    Reserved(R12SubmissionKeyV1),
    Published(R12SubmissionKeyV1),
    Terminal(R12SubmissionKeyV1),
    Quarantined(R12SubmissionKeyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R12SlotRecordV1 {
    key: R12SlotKeyV1,
    phase: R12SlotPhaseV1,
}

impl R12SlotRecordV1 {
    pub const fn key(self) -> R12SlotKeyV1 {
        self.key
    }

    pub const fn phase(self) -> R12SlotPhaseV1 {
        self.phase
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R12SubmissionRecordV1 {
    key: R12SubmissionKeyV1,
    queue: R12QueueOccurrenceV1,
    slot: R12SlotKeyV1,
    dependencies: Vec<R12SubmissionKeyV1>,
    resources: Vec<R12ResourceKeyV1>,
    phase: R12SubmissionPhaseV1,
}

impl R12SubmissionRecordV1 {
    pub const fn key(&self) -> R12SubmissionKeyV1 {
        self.key
    }

    pub const fn queue(&self) -> R12QueueOccurrenceV1 {
        self.queue
    }

    pub const fn slot(&self) -> R12SlotKeyV1 {
        self.slot
    }

    pub fn dependencies(&self) -> &[R12SubmissionKeyV1] {
        &self.dependencies
    }

    pub fn resources(&self) -> &[R12ResourceKeyV1] {
        &self.resources
    }

    pub const fn phase(&self) -> R12SubmissionPhaseV1 {
        self.phase
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R12QueueRecordV1 {
    key: R12QueueOccurrenceV1,
    drained: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R12ResourceRecordV1 {
    key: R12ResourceKeyV1,
    owner: Option<R12SubmissionKeyV1>,
    quarantined: bool,
}

/// A finite state machine for an admitted set of compute queue occurrences.
pub struct R12NativeConcurrencyModelV1 {
    capability: R12MultiQueueCapabilityV1,
    current: bool,
    slots_per_queue: usize,
    queues: Vec<R12QueueRecordV1>,
    slots: Vec<R12SlotRecordV1>,
    resources: Vec<R12ResourceRecordV1>,
    submissions: Vec<R12SubmissionRecordV1>,
}

impl R12NativeConcurrencyModelV1 {
    /// Admits an exact capability/device binding and creates fresh occurrences.
    pub fn new_model_only(
        device: R12DeviceKeyV1,
        capability: R12MultiQueueCapabilityV1,
        queue_count: usize,
        slots_per_queue: usize,
    ) -> Result<Self, R12ConcurrencyModelErrorV1> {
        if device.device_id == 0 || device.generation == 0 {
            return Err(R12ConcurrencyModelErrorV1::InvalidIdentity);
        }
        if capability.device != device || !capability.stable {
            return Err(R12ConcurrencyModelErrorV1::InvalidCapability);
        }
        if !capability.multi_queue_compute {
            return Err(R12ConcurrencyModelErrorV1::Unsupported);
        }
        if queue_count < 2
            || queue_count > capability.max_compute_queues
            || queue_count > MAX_R12_COMPUTE_QUEUES_V1
            || slots_per_queue == 0
            || slots_per_queue > capability.max_slots_per_queue
            || slots_per_queue > MAX_R12_SLOTS_PER_QUEUE_V1
        {
            return Err(R12ConcurrencyModelErrorV1::CapacityExceeded);
        }

        let mut queues = Vec::with_capacity(queue_count);
        let mut slots = Vec::with_capacity(queue_count * slots_per_queue);
        for queue_index in 0..queue_count {
            let queue = R12QueueOccurrenceV1 {
                device,
                queue_id: u64::try_from(queue_index + 1)
                    .map_err(|_| R12ConcurrencyModelErrorV1::CapacityExceeded)?,
                occurrence: 1,
            };
            queues.push(R12QueueRecordV1 {
                key: queue,
                drained: false,
            });
            for slot_index in 0..slots_per_queue {
                slots.push(R12SlotRecordV1 {
                    key: R12SlotKeyV1 {
                        queue,
                        slot_index,
                        generation: 1,
                    },
                    phase: R12SlotPhaseV1::Free,
                });
            }
        }
        Ok(Self {
            capability,
            current: true,
            slots_per_queue,
            queues,
            slots,
            resources: Vec::new(),
            submissions: Vec::new(),
        })
    }

    pub const fn capability(&self) -> R12MultiQueueCapabilityV1 {
        self.capability
    }

    pub const fn current(&self) -> bool {
        self.current
    }

    pub fn queues(&self) -> impl Iterator<Item = R12QueueOccurrenceV1> + '_ {
        self.queues.iter().map(|queue| queue.key)
    }

    pub fn slot(&self, key: R12SlotKeyV1) -> Option<R12SlotRecordV1> {
        self.slots.iter().find(|slot| slot.key == key).copied()
    }

    pub fn submission(&self, key: R12SubmissionKeyV1) -> Option<&R12SubmissionRecordV1> {
        self.submissions
            .iter()
            .find(|submission| submission.key == key)
    }

    pub fn resource_owner(&self, key: R12ResourceKeyV1) -> Option<Option<R12SubmissionKeyV1>> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .map(|resource| resource.owner)
    }

    pub fn resource_quarantined(&self, key: R12ResourceKeyV1) -> Option<bool> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .map(|resource| resource.quarantined)
    }

    pub fn queue_drained(&self, key: R12QueueOccurrenceV1) -> Option<bool> {
        self.queues
            .iter()
            .find(|queue| queue.key == key)
            .map(|queue| queue.drained)
    }

    pub fn register_resource_model_only(
        &mut self,
        key: R12ResourceKeyV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        if !self.current {
            return Err(R12ConcurrencyModelErrorV1::NotCurrent);
        }
        if key.device != self.capability.device || key.resource_id == 0 || key.generation == 0 {
            return Err(R12ConcurrencyModelErrorV1::InvalidIdentity);
        }
        if self.resources.len() >= MAX_R12_RESOURCES_V1 {
            return Err(R12ConcurrencyModelErrorV1::CapacityExceeded);
        }
        if self.resources.iter().any(|resource| resource.key == key) {
            return Err(R12ConcurrencyModelErrorV1::DuplicateIdentity);
        }
        self.resources.push(R12ResourceRecordV1 {
            key,
            owner: None,
            quarantined: false,
        });
        Ok(())
    }

    /// Reserves one free exact-generation slot and takes resource custody.
    pub fn reserve_model_only(
        &mut self,
        key: R12SubmissionKeyV1,
        queue: R12QueueOccurrenceV1,
        dependencies: &[R12SubmissionKeyV1],
        resources: &[R12ResourceKeyV1],
    ) -> Result<R12SlotKeyV1, R12ConcurrencyModelErrorV1> {
        if !self.current {
            return Err(R12ConcurrencyModelErrorV1::NotCurrent);
        }
        if key.stream.device != self.capability.device
            || key.stream.stream_id == 0
            || key.stream.generation == 0
            || key.sequence == 0
        {
            return Err(R12ConcurrencyModelErrorV1::InvalidIdentity);
        }
        let queue_index = self.queue_index(queue)?;
        if self.submissions.len() >= MAX_R12_SUBMISSIONS_V1
            || dependencies.len() > MAX_R12_DEPENDENCIES_V1
            || resources.len() > MAX_R12_RESOURCES_PER_SUBMISSION_V1
        {
            return Err(R12ConcurrencyModelErrorV1::CapacityExceeded);
        }
        if self
            .submissions
            .iter()
            .any(|submission| submission.key == key)
        {
            return Err(R12ConcurrencyModelErrorV1::DuplicateIdentity);
        }
        for (index, dependency) in dependencies.iter().enumerate() {
            if dependency == &key
                || dependencies[..index].contains(dependency)
                || self.submission(*dependency).is_none()
            {
                return Err(R12ConcurrencyModelErrorV1::InvalidIdentity);
            }
        }

        let mut resource_indices = Vec::with_capacity(resources.len());
        for (index, resource) in resources.iter().enumerate() {
            if resources[..index].contains(resource) {
                return Err(R12ConcurrencyModelErrorV1::DuplicateIdentity);
            }
            let resource_index = self
                .resources
                .iter()
                .position(|record| record.key == *resource)
                .ok_or(R12ConcurrencyModelErrorV1::UnknownResource)?;
            let record = self.resources[resource_index];
            if record.owner.is_some() || record.quarantined {
                return Err(R12ConcurrencyModelErrorV1::ResourceBusy);
            }
            resource_indices.push(resource_index);
        }
        let slot_index = self
            .slots
            .iter()
            .position(|slot| slot.key.queue == queue && slot.phase == R12SlotPhaseV1::Free)
            .ok_or(R12ConcurrencyModelErrorV1::CapacityExceeded)?;
        let slot = self.slots[slot_index].key;

        for resource_index in resource_indices {
            self.resources[resource_index].owner = Some(key);
        }
        self.queues[queue_index].drained = false;
        self.slots[slot_index].phase = R12SlotPhaseV1::Reserved(key);
        self.submissions.push(R12SubmissionRecordV1 {
            key,
            queue,
            slot,
            dependencies: dependencies.to_vec(),
            resources: resources.to_vec(),
            phase: R12SubmissionPhaseV1::Reserved,
        });
        Ok(slot)
    }

    /// Publishes only a current reservation whose dependencies succeeded.
    pub fn publish_model_only(
        &mut self,
        key: R12SubmissionKeyV1,
        slot: R12SlotKeyV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        if !self.current {
            return Err(R12ConcurrencyModelErrorV1::NotCurrent);
        }
        let submission_index = self.submission_index(key)?;
        if self.submissions[submission_index].phase != R12SubmissionPhaseV1::Reserved {
            return Err(R12ConcurrencyModelErrorV1::IllegalTransition);
        }
        if self.submissions[submission_index].slot != slot {
            return Err(R12ConcurrencyModelErrorV1::StaleIdentity);
        }
        if !self.submissions[submission_index]
            .dependencies
            .iter()
            .all(|dependency| {
                self.submission(*dependency).is_some_and(|record| {
                    record.phase == R12SubmissionPhaseV1::Terminal(R12TerminalStatusV1::Succeeded)
                })
            })
        {
            return Err(R12ConcurrencyModelErrorV1::DependencyNotReady);
        }
        let slot_index = self.slot_index(slot)?;
        if self.slots[slot_index].phase != R12SlotPhaseV1::Reserved(key) {
            return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
        }
        self.slots[slot_index].phase = R12SlotPhaseV1::Published(key);
        self.submissions[submission_index].phase = R12SubmissionPhaseV1::Published;
        Ok(())
    }

    /// Applies a terminal event only to its exact occurrence/generation owner.
    pub fn observe_terminal_model_only(
        &mut self,
        key: R12SubmissionKeyV1,
        slot: R12SlotKeyV1,
        status: R12TerminalStatusV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        let submission_index = self.submission_index(key)?;
        if self.submissions[submission_index].slot != slot {
            return Err(R12ConcurrencyModelErrorV1::StaleIdentity);
        }
        if self.submissions[submission_index].phase != R12SubmissionPhaseV1::Published {
            return Err(R12ConcurrencyModelErrorV1::IllegalTransition);
        }
        let slot_index = self.slot_index(slot)?;
        if self.slots[slot_index].phase != R12SlotPhaseV1::Published(key) {
            return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
        }
        self.slots[slot_index].phase = R12SlotPhaseV1::Terminal(key);
        self.submissions[submission_index].phase = R12SubmissionPhaseV1::Terminal(status);
        Ok(())
    }

    /// Cancels only before publication and immediately relinquishes custody.
    pub fn cancel_model_only(
        &mut self,
        key: R12SubmissionKeyV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        let submission_index = self.submission_index(key)?;
        match self.submissions[submission_index].phase {
            R12SubmissionPhaseV1::Reserved => self.release_custody(
                submission_index,
                R12SubmissionPhaseV1::CancelledBeforePublication,
            ),
            R12SubmissionPhaseV1::Published | R12SubmissionPhaseV1::Indeterminate => {
                Err(R12ConcurrencyModelErrorV1::TooLate)
            }
            _ => Err(R12ConcurrencyModelErrorV1::IllegalTransition),
        }
    }

    /// Releases terminal custody after no reserved consumer retains its result.
    pub fn release_terminal_model_only(
        &mut self,
        key: R12SubmissionKeyV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        let submission_index = self.submission_index(key)?;
        if !matches!(
            self.submissions[submission_index].phase,
            R12SubmissionPhaseV1::Terminal(_)
        ) {
            return Err(R12ConcurrencyModelErrorV1::IllegalTransition);
        }
        if self.submissions.iter().any(|submission| {
            submission.phase == R12SubmissionPhaseV1::Reserved
                && submission.dependencies.contains(&key)
        }) {
            return Err(R12ConcurrencyModelErrorV1::ResourceBusy);
        }
        self.release_custody(submission_index, R12SubmissionPhaseV1::Released)
    }

    /// Cancels reservations and quarantines published work on currentness loss.
    pub fn lose_currentness_model_only(&mut self) -> Result<(), R12ConcurrencyModelErrorV1> {
        if !self.current {
            return Err(R12ConcurrencyModelErrorV1::IllegalTransition);
        }
        self.validate_global_invariants()?;
        for submission in &self.submissions {
            if submission.phase == R12SubmissionPhaseV1::Reserved
                && submission.slot.generation == u64::MAX
            {
                return Err(R12ConcurrencyModelErrorV1::CapacityExceeded);
            }
        }
        self.current = false;

        for submission_index in 0..self.submissions.len() {
            match self.submissions[submission_index].phase {
                R12SubmissionPhaseV1::Reserved => self.release_custody(
                    submission_index,
                    R12SubmissionPhaseV1::CancelledBeforePublication,
                )?,
                R12SubmissionPhaseV1::Published => {
                    let key = self.submissions[submission_index].key;
                    let slot = self.submissions[submission_index].slot;
                    let resources = self.submissions[submission_index].resources.clone();
                    let slot_index = self.slot_index(slot)?;
                    self.slots[slot_index].phase = R12SlotPhaseV1::Quarantined(key);
                    for resource in resources {
                        let resource_index = self.resource_index(resource)?;
                        self.resources[resource_index].quarantined = true;
                    }
                    self.submissions[submission_index].phase = R12SubmissionPhaseV1::Indeterminate;
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Records quiescence only for an exact occurrence with no retained custody.
    pub fn drain_queue_model_only(
        &mut self,
        queue: R12QueueOccurrenceV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        let queue_index = self.queue_index(queue)?;
        if self
            .submissions
            .iter()
            .any(|submission| submission.queue == queue && submission.phase.retains_custody())
            || self
                .slots
                .iter()
                .any(|slot| slot.key.queue == queue && slot.phase != R12SlotPhaseV1::Free)
        {
            return Err(R12ConcurrencyModelErrorV1::NotDrained);
        }
        self.queues[queue_index].drained = true;
        Ok(())
    }

    /// Recreates an exactly drained queue under the next occurrence identity.
    pub fn recreate_drained_queue_model_only(
        &mut self,
        queue: R12QueueOccurrenceV1,
    ) -> Result<R12QueueOccurrenceV1, R12ConcurrencyModelErrorV1> {
        if !self.current {
            return Err(R12ConcurrencyModelErrorV1::NotCurrent);
        }
        let queue_index = self.queue_index(queue)?;
        if !self.queues[queue_index].drained {
            return Err(R12ConcurrencyModelErrorV1::NotDrained);
        }
        let occurrence = queue
            .occurrence
            .checked_add(1)
            .ok_or(R12ConcurrencyModelErrorV1::CapacityExceeded)?;
        let next_queue = R12QueueOccurrenceV1 {
            occurrence,
            ..queue
        };
        for slot in &mut self.slots {
            if slot.key.queue == queue {
                if slot.phase != R12SlotPhaseV1::Free {
                    return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
                }
                slot.key.queue = next_queue;
                slot.key.generation = 1;
            }
        }
        self.queues[queue_index] = R12QueueRecordV1 {
            key: next_queue,
            drained: false,
        };
        Ok(next_queue)
    }

    /// Checks bijective custody, identity binding, and quarantine invariants.
    pub fn validate_global_invariants(&self) -> Result<(), R12ConcurrencyModelErrorV1> {
        if self.queues.len() < 2
            || self.queues.len() > self.capability.max_compute_queues
            || self.slots_per_queue == 0
            || self.slots_per_queue > self.capability.max_slots_per_queue
            || self.capability.device.device_id == 0
            || !self.capability.stable
            || !self.capability.multi_queue_compute
        {
            return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
        }
        for (index, queue) in self.queues.iter().enumerate() {
            if queue.key.device != self.capability.device
                || queue.key.queue_id == 0
                || queue.key.occurrence == 0
                || self.queues[..index]
                    .iter()
                    .any(|prior| prior.key == queue.key || prior.key.queue_id == queue.key.queue_id)
                || self
                    .slots
                    .iter()
                    .filter(|slot| slot.key.queue == queue.key)
                    .count()
                    != self.slots_per_queue
            {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
            if queue.drained
                && (self.submissions.iter().any(|submission| {
                    submission.queue == queue.key && submission.phase.retains_custody()
                }) || self
                    .slots
                    .iter()
                    .any(|slot| slot.key.queue == queue.key && slot.phase != R12SlotPhaseV1::Free))
            {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
        }
        for (index, resource) in self.resources.iter().enumerate() {
            if self.resources[..index]
                .iter()
                .any(|prior| prior.key == resource.key)
                || resource.quarantined && resource.owner.is_none()
            {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
            if let Some(owner) = resource.owner {
                let submission = self
                    .submission(owner)
                    .ok_or(R12ConcurrencyModelErrorV1::InvariantViolation)?;
                if !submission.phase.retains_custody()
                    || !submission.resources.contains(&resource.key)
                    || resource.quarantined
                        != (submission.phase == R12SubmissionPhaseV1::Indeterminate)
                {
                    return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
                }
            }
        }
        for (index, submission) in self.submissions.iter().enumerate() {
            if self.submissions[..index]
                .iter()
                .any(|prior| prior.key == submission.key)
                || submission.queue != submission.slot.queue
            {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
            let expected_slot = match submission.phase {
                R12SubmissionPhaseV1::Reserved => Some(R12SlotPhaseV1::Reserved(submission.key)),
                R12SubmissionPhaseV1::Published => Some(R12SlotPhaseV1::Published(submission.key)),
                R12SubmissionPhaseV1::Terminal(_) => Some(R12SlotPhaseV1::Terminal(submission.key)),
                R12SubmissionPhaseV1::Indeterminate => {
                    Some(R12SlotPhaseV1::Quarantined(submission.key))
                }
                R12SubmissionPhaseV1::CancelledBeforePublication
                | R12SubmissionPhaseV1::Released => None,
            };
            if let Some(expected) = expected_slot {
                if self.slot(submission.slot).map(|slot| slot.phase) != Some(expected)
                    || submission.resources.iter().any(|resource| {
                        self.resource_owner(*resource) != Some(Some(submission.key))
                    })
                {
                    return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
                }
            } else if submission
                .resources
                .iter()
                .any(|resource| self.resource_owner(*resource) == Some(Some(submission.key)))
            {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
        }
        for slot in &self.slots {
            let valid = match slot.phase {
                R12SlotPhaseV1::Free => true,
                R12SlotPhaseV1::Reserved(owner) => self.submission(owner).is_some_and(|record| {
                    record.slot == slot.key && record.phase == R12SubmissionPhaseV1::Reserved
                }),
                R12SlotPhaseV1::Published(owner) => self.submission(owner).is_some_and(|record| {
                    record.slot == slot.key && record.phase == R12SubmissionPhaseV1::Published
                }),
                R12SlotPhaseV1::Terminal(owner) => self.submission(owner).is_some_and(|record| {
                    record.slot == slot.key
                        && matches!(record.phase, R12SubmissionPhaseV1::Terminal(_))
                }),
                R12SlotPhaseV1::Quarantined(owner) => {
                    self.submission(owner).is_some_and(|record| {
                        record.slot == slot.key
                            && record.phase == R12SubmissionPhaseV1::Indeterminate
                    })
                }
            };
            if !valid {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
        }
        Ok(())
    }

    fn queue_index(&self, key: R12QueueOccurrenceV1) -> Result<usize, R12ConcurrencyModelErrorV1> {
        self.queues
            .iter()
            .position(|queue| queue.key == key)
            .ok_or(R12ConcurrencyModelErrorV1::UnknownQueue)
    }

    fn slot_index(&self, key: R12SlotKeyV1) -> Result<usize, R12ConcurrencyModelErrorV1> {
        self.slots
            .iter()
            .position(|slot| slot.key == key)
            .ok_or(R12ConcurrencyModelErrorV1::StaleIdentity)
    }

    fn resource_index(&self, key: R12ResourceKeyV1) -> Result<usize, R12ConcurrencyModelErrorV1> {
        self.resources
            .iter()
            .position(|resource| resource.key == key)
            .ok_or(R12ConcurrencyModelErrorV1::UnknownResource)
    }

    fn submission_index(
        &self,
        key: R12SubmissionKeyV1,
    ) -> Result<usize, R12ConcurrencyModelErrorV1> {
        self.submissions
            .iter()
            .position(|submission| submission.key == key)
            .ok_or(R12ConcurrencyModelErrorV1::UnknownSubmission)
    }

    fn release_custody(
        &mut self,
        submission_index: usize,
        next_phase: R12SubmissionPhaseV1,
    ) -> Result<(), R12ConcurrencyModelErrorV1> {
        let key = self.submissions[submission_index].key;
        let slot = self.submissions[submission_index].slot;
        let resources = self.submissions[submission_index].resources.clone();
        let slot_index = self.slot_index(slot)?;
        let next_generation = slot
            .generation
            .checked_add(1)
            .ok_or(R12ConcurrencyModelErrorV1::CapacityExceeded)?;
        let expected_slot_phase = match self.submissions[submission_index].phase {
            R12SubmissionPhaseV1::Reserved => R12SlotPhaseV1::Reserved(key),
            R12SubmissionPhaseV1::Terminal(_) => R12SlotPhaseV1::Terminal(key),
            _ => return Err(R12ConcurrencyModelErrorV1::IllegalTransition),
        };
        if self.slots[slot_index].phase != expected_slot_phase {
            return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
        }
        for resource in &resources {
            let resource_index = self.resource_index(*resource)?;
            if self.resources[resource_index].owner != Some(key)
                || self.resources[resource_index].quarantined
            {
                return Err(R12ConcurrencyModelErrorV1::InvariantViolation);
            }
        }
        for resource in resources {
            let resource_index = self.resource_index(resource)?;
            self.resources[resource_index].owner = None;
        }
        self.slots[slot_index].key.generation = next_generation;
        self.slots[slot_index].phase = R12SlotPhaseV1::Free;
        self.submissions[submission_index].phase = next_phase;
        Ok(())
    }
}

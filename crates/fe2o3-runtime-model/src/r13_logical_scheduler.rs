//! Bounded logical-stream scheduling over two abstract physical lanes.
//!
//! Logical streams retain FIFO order across compute and copy operations. A
//! physical lane is leased only when a stream head is publishable: every
//! dependency succeeded, no referenced resource has an active owner, and the
//! selected lane has no owner. Completion returns the lane
//! while lifetime custody remains retained until an explicit release. An
//! ordered successor may take active ownership without erasing its terminal
//! producer's retention. Currentness loss cancels unpublished work and
//! quarantines published or terminal custody.
//!
//! This is a caller-constructible, pure state machine. It does not refine the
//! Rust runtime, KFD, AQL packets, firmware, HSA, HIP, or hardware execution.

use alloc::vec::Vec;

pub const R13_PHYSICAL_LANE_COUNT_V1: usize = 2;
pub const MAX_R13_LOGICAL_STREAMS_V1: usize = 64;
pub const MAX_R13_SUBMISSIONS_V1: usize = 4_096;
pub const MAX_R13_DEPENDENCIES_V1: usize = 32;
pub const MAX_R13_DEPENDENCY_DEPTH_V1: usize = 32;
pub const MAX_R13_RESOURCES_PER_SUBMISSION_V1: usize = 64;
pub const MAX_R13_RESOURCES_V1: usize = 8_192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R13SchedulerModelErrorV1 {
    InvalidIdentity,
    CapacityExceeded,
    DuplicateIdentity,
    UnknownStream,
    UnknownSubmission,
    UnknownResource,
    DependencyNotReady,
    ResourceBusy,
    NoPhysicalLane,
    NotStreamHead,
    TooLate,
    NotCurrent,
    IllegalTransition,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R13SchedulerDeviceKeyV1 {
    pub device_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R13LogicalStreamKeyV1 {
    pub device: R13SchedulerDeviceKeyV1,
    pub stream_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R13ScheduledSubmissionKeyV1 {
    pub stream: R13LogicalStreamKeyV1,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct R13ScheduledResourceKeyV1 {
    pub device: R13SchedulerDeviceKeyV1,
    pub resource_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R13OperationClassV1 {
    Compute,
    Copy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R13TerminalStatusV1 {
    Succeeded,
    Failed { code: i64 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R13ScheduledSubmissionPhaseV1 {
    Queued,
    Published {
        lane: usize,
    },
    Terminal(R13TerminalStatusV1),
    CancelledBeforePublication,
    Indeterminate {
        lane: Option<usize>,
        terminal: Option<R13TerminalStatusV1>,
    },
    Released(R13TerminalStatusV1),
}

impl R13ScheduledSubmissionPhaseV1 {
    pub const fn retains_resources(self) -> bool {
        matches!(
            self,
            Self::Published { .. } | Self::Terminal(_) | Self::Indeterminate { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R13LogicalStreamRecordV1 {
    key: R13LogicalStreamKeyV1,
    head: Option<R13ScheduledSubmissionKeyV1>,
    tail: Option<R13ScheduledSubmissionKeyV1>,
    next_sequence: u64,
}

impl R13LogicalStreamRecordV1 {
    pub const fn key(self) -> R13LogicalStreamKeyV1 {
        self.key
    }

    pub const fn head(self) -> Option<R13ScheduledSubmissionKeyV1> {
        self.head
    }

    pub const fn tail(self) -> Option<R13ScheduledSubmissionKeyV1> {
        self.tail
    }

    pub const fn next_sequence(self) -> u64 {
        self.next_sequence
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct R13ScheduledSubmissionRecordV1 {
    key: R13ScheduledSubmissionKeyV1,
    class: R13OperationClassV1,
    predecessor: Option<R13ScheduledSubmissionKeyV1>,
    successor: Option<R13ScheduledSubmissionKeyV1>,
    dependencies: Vec<R13ScheduledSubmissionKeyV1>,
    dependency_depth: usize,
    resources: Vec<R13ScheduledResourceKeyV1>,
    phase: R13ScheduledSubmissionPhaseV1,
}

impl R13ScheduledSubmissionRecordV1 {
    pub const fn key(&self) -> R13ScheduledSubmissionKeyV1 {
        self.key
    }

    pub const fn class(&self) -> R13OperationClassV1 {
        self.class
    }

    pub const fn predecessor(&self) -> Option<R13ScheduledSubmissionKeyV1> {
        self.predecessor
    }

    pub const fn successor(&self) -> Option<R13ScheduledSubmissionKeyV1> {
        self.successor
    }

    pub fn dependencies(&self) -> &[R13ScheduledSubmissionKeyV1] {
        &self.dependencies
    }

    pub const fn dependency_depth(&self) -> usize {
        self.dependency_depth
    }

    pub fn resources(&self) -> &[R13ScheduledResourceKeyV1] {
        &self.resources
    }

    pub const fn phase(&self) -> R13ScheduledSubmissionPhaseV1 {
        self.phase
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct R13ResourceRecordV1 {
    key: R13ScheduledResourceKeyV1,
    active_owner: Option<R13ScheduledSubmissionKeyV1>,
    retainers: Vec<R13ScheduledSubmissionKeyV1>,
    quarantined: bool,
}

/// A bounded scheduler with many logical FIFOs and exactly two physical lanes.
pub struct R13LogicalSchedulerModelV1 {
    device: R13SchedulerDeviceKeyV1,
    current: bool,
    streams: Vec<R13LogicalStreamRecordV1>,
    submissions: Vec<R13ScheduledSubmissionRecordV1>,
    resources: Vec<R13ResourceRecordV1>,
    lanes: [Option<R13ScheduledSubmissionKeyV1>; R13_PHYSICAL_LANE_COUNT_V1],
}

impl R13LogicalSchedulerModelV1 {
    pub fn new_model_only(
        device: R13SchedulerDeviceKeyV1,
    ) -> Result<Self, R13SchedulerModelErrorV1> {
        if device.device_id == 0 || device.generation == 0 {
            return Err(R13SchedulerModelErrorV1::InvalidIdentity);
        }
        Ok(Self {
            device,
            current: true,
            streams: Vec::new(),
            submissions: Vec::new(),
            resources: Vec::new(),
            lanes: [None; R13_PHYSICAL_LANE_COUNT_V1],
        })
    }

    pub const fn device(&self) -> R13SchedulerDeviceKeyV1 {
        self.device
    }

    pub const fn current(&self) -> bool {
        self.current
    }

    pub const fn physical_lane_count(&self) -> usize {
        R13_PHYSICAL_LANE_COUNT_V1
    }

    pub fn lane_owner(&self, lane: usize) -> Option<Option<R13ScheduledSubmissionKeyV1>> {
        self.lanes.get(lane).copied()
    }

    pub fn stream(&self, key: R13LogicalStreamKeyV1) -> Option<R13LogicalStreamRecordV1> {
        self.streams
            .iter()
            .find(|stream| stream.key == key)
            .copied()
    }

    pub fn submission(
        &self,
        key: R13ScheduledSubmissionKeyV1,
    ) -> Option<&R13ScheduledSubmissionRecordV1> {
        self.submissions
            .iter()
            .find(|submission| submission.key == key)
    }

    pub fn resource_active_owner(
        &self,
        key: R13ScheduledResourceKeyV1,
    ) -> Option<Option<R13ScheduledSubmissionKeyV1>> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .map(|resource| resource.active_owner)
    }

    pub fn resource_retainers(
        &self,
        key: R13ScheduledResourceKeyV1,
    ) -> Option<&[R13ScheduledSubmissionKeyV1]> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .map(|resource| resource.retainers.as_slice())
    }

    pub fn resource_quarantined(&self, key: R13ScheduledResourceKeyV1) -> Option<bool> {
        self.resources
            .iter()
            .find(|resource| resource.key == key)
            .map(|resource| resource.quarantined)
    }

    pub fn register_stream_model_only(
        &mut self,
        key: R13LogicalStreamKeyV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        if !self.current {
            return Err(R13SchedulerModelErrorV1::NotCurrent);
        }
        if key.device != self.device || key.stream_id == 0 || key.generation == 0 {
            return Err(R13SchedulerModelErrorV1::InvalidIdentity);
        }
        if self.streams.len() >= MAX_R13_LOGICAL_STREAMS_V1 {
            return Err(R13SchedulerModelErrorV1::CapacityExceeded);
        }
        if self.streams.iter().any(|stream| stream.key == key) {
            return Err(R13SchedulerModelErrorV1::DuplicateIdentity);
        }
        self.streams.push(R13LogicalStreamRecordV1 {
            key,
            head: None,
            tail: None,
            next_sequence: 1,
        });
        Ok(())
    }

    pub fn register_resource_model_only(
        &mut self,
        key: R13ScheduledResourceKeyV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        if !self.current {
            return Err(R13SchedulerModelErrorV1::NotCurrent);
        }
        if key.device != self.device || key.resource_id == 0 || key.generation == 0 {
            return Err(R13SchedulerModelErrorV1::InvalidIdentity);
        }
        if self.resources.len() >= MAX_R13_RESOURCES_V1 {
            return Err(R13SchedulerModelErrorV1::CapacityExceeded);
        }
        if self.resources.iter().any(|resource| resource.key == key) {
            return Err(R13SchedulerModelErrorV1::DuplicateIdentity);
        }
        self.resources.push(R13ResourceRecordV1 {
            key,
            active_owner: None,
            retainers: Vec::new(),
            quarantined: false,
        });
        Ok(())
    }

    /// Appends an operation to one logical FIFO without taking a physical lane.
    pub fn enqueue_model_only(
        &mut self,
        key: R13ScheduledSubmissionKeyV1,
        class: R13OperationClassV1,
        dependencies: &[R13ScheduledSubmissionKeyV1],
        resources: &[R13ScheduledResourceKeyV1],
    ) -> Result<(), R13SchedulerModelErrorV1> {
        if !self.current {
            return Err(R13SchedulerModelErrorV1::NotCurrent);
        }
        if key.stream.device != self.device || key.sequence == 0 {
            return Err(R13SchedulerModelErrorV1::InvalidIdentity);
        }
        let stream_index = self.stream_index(key.stream)?;
        if self.streams[stream_index].next_sequence != key.sequence {
            return Err(R13SchedulerModelErrorV1::InvalidIdentity);
        }
        if self.submissions.len() >= MAX_R13_SUBMISSIONS_V1
            || dependencies.len() > MAX_R13_DEPENDENCIES_V1
            || resources.len() > MAX_R13_RESOURCES_PER_SUBMISSION_V1
        {
            return Err(R13SchedulerModelErrorV1::CapacityExceeded);
        }
        if self.submission(key).is_some() {
            return Err(R13SchedulerModelErrorV1::DuplicateIdentity);
        }

        let predecessor = self.streams[stream_index].tail;
        let mut effective_dependencies = dependencies.to_vec();
        if let Some(predecessor) = predecessor
            && !effective_dependencies.contains(&predecessor)
        {
            if effective_dependencies.len() == MAX_R13_DEPENDENCIES_V1 {
                return Err(R13SchedulerModelErrorV1::CapacityExceeded);
            }
            effective_dependencies.push(predecessor);
        }

        let mut dependency_depth = 1usize;
        for (index, dependency) in effective_dependencies.iter().enumerate() {
            if *dependency == key || effective_dependencies[..index].contains(dependency) {
                return Err(R13SchedulerModelErrorV1::InvalidIdentity);
            }
            let dependency_record = self
                .submission(*dependency)
                .ok_or(R13SchedulerModelErrorV1::UnknownSubmission)?;
            if matches!(
                dependency_record.phase,
                R13ScheduledSubmissionPhaseV1::CancelledBeforePublication
                    | R13ScheduledSubmissionPhaseV1::Indeterminate { .. }
                    | R13ScheduledSubmissionPhaseV1::Terminal(R13TerminalStatusV1::Failed { .. })
                    | R13ScheduledSubmissionPhaseV1::Released(_)
            ) {
                return Err(R13SchedulerModelErrorV1::DependencyNotReady);
            }
            dependency_depth = dependency_depth.max(
                dependency_record
                    .dependency_depth
                    .checked_add(1)
                    .ok_or(R13SchedulerModelErrorV1::CapacityExceeded)?,
            );
        }
        if dependency_depth > MAX_R13_DEPENDENCY_DEPTH_V1 {
            return Err(R13SchedulerModelErrorV1::CapacityExceeded);
        }

        for (index, resource) in resources.iter().enumerate() {
            if resources[..index].contains(resource) {
                return Err(R13SchedulerModelErrorV1::DuplicateIdentity);
            }
            let record = self
                .resources
                .iter()
                .find(|record| record.key == *resource)
                .ok_or(R13SchedulerModelErrorV1::UnknownResource)?;
            if record.quarantined {
                return Err(R13SchedulerModelErrorV1::ResourceBusy);
            }
        }

        let next_sequence = key
            .sequence
            .checked_add(1)
            .ok_or(R13SchedulerModelErrorV1::CapacityExceeded)?;
        if let Some(predecessor) = predecessor {
            let predecessor_index = self.submission_index(predecessor)?;
            if self.submissions[predecessor_index].successor.is_some() {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            self.submissions[predecessor_index].successor = Some(key);
        } else {
            self.streams[stream_index].head = Some(key);
        }
        self.streams[stream_index].tail = Some(key);
        self.streams[stream_index].next_sequence = next_sequence;
        self.submissions.push(R13ScheduledSubmissionRecordV1 {
            key,
            class,
            predecessor,
            successor: None,
            dependencies: effective_dependencies,
            dependency_depth,
            resources: resources.to_vec(),
            phase: R13ScheduledSubmissionPhaseV1::Queued,
        });
        Ok(())
    }

    /// Publishes a ready logical-stream head on the lowest-numbered free lane.
    pub fn publish_head_model_only(
        &mut self,
        key: R13ScheduledSubmissionKeyV1,
    ) -> Result<usize, R13SchedulerModelErrorV1> {
        if !self.current {
            return Err(R13SchedulerModelErrorV1::NotCurrent);
        }
        let stream_index = self.stream_index(key.stream)?;
        if self.streams[stream_index].head != Some(key) {
            return Err(R13SchedulerModelErrorV1::NotStreamHead);
        }
        let submission_index = self.submission_index(key)?;
        if self.submissions[submission_index].phase != R13ScheduledSubmissionPhaseV1::Queued {
            return Err(R13SchedulerModelErrorV1::IllegalTransition);
        }
        if !self.submissions[submission_index]
            .dependencies
            .iter()
            .all(|dependency| {
                self.submission(*dependency).is_some_and(|record| {
                    record.phase
                        == R13ScheduledSubmissionPhaseV1::Terminal(R13TerminalStatusV1::Succeeded)
                })
            })
        {
            return Err(R13SchedulerModelErrorV1::DependencyNotReady);
        }
        let lane = self
            .lanes
            .iter()
            .position(Option::is_none)
            .ok_or(R13SchedulerModelErrorV1::NoPhysicalLane)?;
        let mut resource_indices =
            Vec::with_capacity(self.submissions[submission_index].resources.len());
        for resource in &self.submissions[submission_index].resources {
            let resource_index = self.resource_index(*resource)?;
            let record = &self.resources[resource_index];
            if record.active_owner.is_some() || record.quarantined {
                return Err(R13SchedulerModelErrorV1::ResourceBusy);
            }
            resource_indices.push(resource_index);
        }

        for resource_index in resource_indices {
            self.resources[resource_index].active_owner = Some(key);
            self.resources[resource_index].retainers.push(key);
        }
        self.lanes[lane] = Some(key);
        self.submissions[submission_index].phase =
            R13ScheduledSubmissionPhaseV1::Published { lane };
        Ok(lane)
    }

    /// Accepts completion only from the exact currently leased lane.
    pub fn observe_terminal_model_only(
        &mut self,
        key: R13ScheduledSubmissionKeyV1,
        observed_lane: usize,
        status: R13TerminalStatusV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        let submission_index = self.submission_index(key)?;
        if self.submissions[submission_index].phase
            != (R13ScheduledSubmissionPhaseV1::Published {
                lane: observed_lane,
            })
        {
            return Err(R13SchedulerModelErrorV1::IllegalTransition);
        }
        if self.lanes.get(observed_lane).copied().flatten() != Some(key) {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        let stream_index = self.stream_index(key.stream)?;
        if self.streams[stream_index].head != Some(key) {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        let successor = self.submissions[submission_index].successor;
        let resource_indices = self.submissions[submission_index]
            .resources
            .iter()
            .map(|resource| self.resource_index(*resource))
            .collect::<Result<Vec<_>, _>>()?;
        if resource_indices
            .iter()
            .any(|resource_index| self.resources[*resource_index].active_owner != Some(key))
        {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        for resource_index in resource_indices {
            self.resources[resource_index].active_owner = None;
        }
        self.lanes[observed_lane] = None;
        self.submissions[submission_index].phase = R13ScheduledSubmissionPhaseV1::Terminal(status);
        if let Some(successor) = successor {
            let successor_index = self.submission_index(successor)?;
            self.submissions[successor_index].predecessor = None;
        }
        self.streams[stream_index].head = successor;
        if successor.is_none() {
            self.streams[stream_index].tail = None;
        }
        Ok(())
    }

    /// Cancels only the unpublished tail and restores the prior FIFO tail.
    pub fn cancel_tail_model_only(
        &mut self,
        key: R13ScheduledSubmissionKeyV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        if !self.current {
            return Err(R13SchedulerModelErrorV1::NotCurrent);
        }
        let submission_index = self.submission_index(key)?;
        if self.submissions[submission_index].phase != R13ScheduledSubmissionPhaseV1::Queued {
            return Err(R13SchedulerModelErrorV1::TooLate);
        }
        let stream_index = self.stream_index(key.stream)?;
        if self.streams[stream_index].tail != Some(key) {
            return Err(R13SchedulerModelErrorV1::TooLate);
        }
        let predecessor = self.submissions[submission_index].predecessor;
        if let Some(predecessor) = predecessor {
            let predecessor_index = self.submission_index(predecessor)?;
            if self.submissions[predecessor_index].successor != Some(key) {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            self.submissions[predecessor_index].successor = None;
        } else if self.streams[stream_index].head != Some(key) {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        self.streams[stream_index].tail = predecessor;
        if self.streams[stream_index].head == Some(key) {
            self.streams[stream_index].head = None;
        }
        self.submissions[submission_index].phase =
            R13ScheduledSubmissionPhaseV1::CancelledBeforePublication;
        Ok(())
    }

    /// Releases terminal resources only after queued dependents consumed them.
    pub fn release_terminal_model_only(
        &mut self,
        key: R13ScheduledSubmissionKeyV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        let submission_index = self.submission_index(key)?;
        let status = match self.submissions[submission_index].phase {
            R13ScheduledSubmissionPhaseV1::Terminal(status) => status,
            _ => return Err(R13SchedulerModelErrorV1::IllegalTransition),
        };
        if self.submissions.iter().any(|submission| {
            submission.phase == R13ScheduledSubmissionPhaseV1::Queued
                && submission.dependencies.contains(&key)
        }) {
            return Err(R13SchedulerModelErrorV1::ResourceBusy);
        }
        let resources = self.submissions[submission_index].resources.clone();
        let resource_indices = resources
            .iter()
            .map(|resource| self.resource_index(*resource))
            .collect::<Result<Vec<_>, _>>()?;
        if resource_indices.iter().any(|resource_index| {
            let resource = &self.resources[*resource_index];
            resource.quarantined
                || resource.active_owner == Some(key)
                || !resource.retainers.contains(&key)
        }) {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        for resource_index in resource_indices {
            let resource = &mut self.resources[resource_index];
            let Some(retainer_index) = resource
                .retainers
                .iter()
                .position(|retainer| *retainer == key)
            else {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            };
            resource.retainers.remove(retainer_index);
        }
        self.submissions[submission_index].phase = R13ScheduledSubmissionPhaseV1::Released(status);
        Ok(())
    }

    /// Fails closed: no unpublished operation survives and custody is retained.
    pub fn lose_currentness_model_only(&mut self) -> Result<(), R13SchedulerModelErrorV1> {
        if !self.current {
            return Err(R13SchedulerModelErrorV1::IllegalTransition);
        }
        self.validate_global_invariants()?;
        self.current = false;
        for stream in &mut self.streams {
            stream.head = None;
            stream.tail = None;
        }
        for submission_index in 0..self.submissions.len() {
            match self.submissions[submission_index].phase {
                R13ScheduledSubmissionPhaseV1::Queued => {
                    self.submissions[submission_index].phase =
                        R13ScheduledSubmissionPhaseV1::CancelledBeforePublication;
                }
                R13ScheduledSubmissionPhaseV1::Published { lane } => {
                    let key = self.submissions[submission_index].key;
                    let resources = self.submissions[submission_index].resources.clone();
                    for resource in resources {
                        let resource_index = self.resource_index(resource)?;
                        self.resources[resource_index].quarantined = true;
                    }
                    self.submissions[submission_index].phase =
                        R13ScheduledSubmissionPhaseV1::Indeterminate {
                            lane: Some(lane),
                            terminal: None,
                        };
                    if self.lanes[lane] != Some(key) {
                        return Err(R13SchedulerModelErrorV1::InvariantViolation);
                    }
                }
                R13ScheduledSubmissionPhaseV1::Terminal(status) => {
                    let resources = self.submissions[submission_index].resources.clone();
                    for resource in resources {
                        let resource_index = self.resource_index(resource)?;
                        self.resources[resource_index].quarantined = true;
                    }
                    self.submissions[submission_index].phase =
                        R13ScheduledSubmissionPhaseV1::Indeterminate {
                            lane: None,
                            terminal: Some(status),
                        };
                }
                _ => {}
            }
        }
        self.validate_global_invariants()
    }

    /// Checks bounded FIFO, unique lease, dependency, and custody invariants.
    pub fn validate_global_invariants(&self) -> Result<(), R13SchedulerModelErrorV1> {
        if self.device.device_id == 0
            || self.device.generation == 0
            || self.streams.len() > MAX_R13_LOGICAL_STREAMS_V1
            || self.submissions.len() > MAX_R13_SUBMISSIONS_V1
            || self.resources.len() > MAX_R13_RESOURCES_V1
        {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        for (index, stream) in self.streams.iter().enumerate() {
            if stream.key.device != self.device
                || stream.key.stream_id == 0
                || stream.key.generation == 0
                || stream.next_sequence == 0
                || self.streams[..index]
                    .iter()
                    .any(|prior| prior.key == stream.key)
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            if !self.current {
                if stream.head.is_some() || stream.tail.is_some() {
                    return Err(R13SchedulerModelErrorV1::InvariantViolation);
                }
                continue;
            }
            self.validate_stream_fifo(stream)?;
        }
        for (index, submission) in self.submissions.iter().enumerate() {
            if submission.key.stream.device != self.device
                || submission.key.sequence == 0
                || submission.dependencies.len() > MAX_R13_DEPENDENCIES_V1
                || submission.dependency_depth > MAX_R13_DEPENDENCY_DEPTH_V1
                || submission.resources.len() > MAX_R13_RESOURCES_PER_SUBMISSION_V1
                || self.submissions[..index]
                    .iter()
                    .any(|prior| prior.key == submission.key)
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            self.validate_submission_dependencies(submission)?;
            self.validate_submission_custody(submission)?;
        }
        for (index, resource) in self.resources.iter().enumerate() {
            if resource.key.device != self.device
                || resource.key.resource_id == 0
                || resource.key.generation == 0
                || self.resources[..index]
                    .iter()
                    .any(|prior| prior.key == resource.key)
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            if resource.retainers.len() > self.submissions.len()
                || resource
                    .retainers
                    .iter()
                    .enumerate()
                    .any(|(retainer_index, retainer)| {
                        resource.retainers[..retainer_index].contains(retainer)
                    })
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            if let Some(owner) = resource.active_owner {
                let submission = self
                    .submission(owner)
                    .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?;
                if !resource.retainers.contains(&owner)
                    || !matches!(
                        submission.phase,
                        R13ScheduledSubmissionPhaseV1::Published { .. }
                            | R13ScheduledSubmissionPhaseV1::Indeterminate {
                                lane: Some(_),
                                terminal: None,
                            }
                    )
                {
                    return Err(R13SchedulerModelErrorV1::InvariantViolation);
                }
            }
            for retainer in &resource.retainers {
                let submission = self
                    .submission(*retainer)
                    .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?;
                if submission
                    .resources
                    .iter()
                    .filter(|key| **key == resource.key)
                    .count()
                    != 1
                    || !submission.phase.retains_resources()
                    || (resource.quarantined
                        != matches!(
                            submission.phase,
                            R13ScheduledSubmissionPhaseV1::Indeterminate { .. }
                        ))
                {
                    return Err(R13SchedulerModelErrorV1::InvariantViolation);
                }
            }
            if resource.quarantined && resource.retainers.is_empty() {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
        }
        for (lane, owner) in self.lanes.iter().enumerate() {
            if let Some(owner) = owner {
                let submission = self
                    .submission(*owner)
                    .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?;
                if submission.phase != (R13ScheduledSubmissionPhaseV1::Published { lane })
                    && submission.phase
                        != (R13ScheduledSubmissionPhaseV1::Indeterminate {
                            lane: Some(lane),
                            terminal: None,
                        })
                {
                    return Err(R13SchedulerModelErrorV1::InvariantViolation);
                }
            }
        }
        Ok(())
    }

    fn validate_stream_fifo(
        &self,
        stream: &R13LogicalStreamRecordV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        if stream.head.is_none() != stream.tail.is_none() {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        let mut cursor = stream.head;
        let mut predecessor = None;
        let mut visited = 0usize;
        while let Some(key) = cursor {
            visited = visited
                .checked_add(1)
                .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?;
            if visited > self.submissions.len() {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            let record = self
                .submission(key)
                .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?;
            if key.stream != stream.key
                || record.predecessor != predecessor
                || !matches!(
                    record.phase,
                    R13ScheduledSubmissionPhaseV1::Queued
                        | R13ScheduledSubmissionPhaseV1::Published { .. }
                )
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            predecessor = Some(key);
            cursor = record.successor;
        }
        if predecessor != stream.tail {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        let outstanding = self
            .submissions
            .iter()
            .filter(|submission| {
                submission.key.stream == stream.key
                    && matches!(
                        submission.phase,
                        R13ScheduledSubmissionPhaseV1::Queued
                            | R13ScheduledSubmissionPhaseV1::Published { .. }
                    )
            })
            .count();
        if outstanding != visited {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        Ok(())
    }

    fn validate_submission_dependencies(
        &self,
        submission: &R13ScheduledSubmissionRecordV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        let mut expected_depth = 1usize;
        if let Some(predecessor) = submission.predecessor
            && !submission.dependencies.contains(&predecessor)
        {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        for (index, dependency) in submission.dependencies.iter().enumerate() {
            if *dependency == submission.key
                || submission.dependencies[..index].contains(dependency)
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
            let record = self
                .submission(*dependency)
                .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?;
            expected_depth = expected_depth.max(
                record
                    .dependency_depth
                    .checked_add(1)
                    .ok_or(R13SchedulerModelErrorV1::InvariantViolation)?,
            );
        }
        if expected_depth != submission.dependency_depth {
            return Err(R13SchedulerModelErrorV1::InvariantViolation);
        }
        for (index, resource) in submission.resources.iter().enumerate() {
            if submission.resources[..index].contains(resource)
                || self.resources.iter().all(|record| record.key != *resource)
            {
                return Err(R13SchedulerModelErrorV1::InvariantViolation);
            }
        }
        Ok(())
    }

    fn validate_submission_custody(
        &self,
        submission: &R13ScheduledSubmissionRecordV1,
    ) -> Result<(), R13SchedulerModelErrorV1> {
        let resource_state_matches = |retained: bool, active: bool, quarantined: bool| {
            submission.resources.iter().all(|key| {
                self.resources
                    .iter()
                    .find(|resource| resource.key == *key)
                    .is_some_and(|resource| {
                        resource.retainers.contains(&submission.key) == retained
                            && (resource.active_owner == Some(submission.key)) == active
                            && (!retained || resource.quarantined == quarantined)
                    })
            })
        };
        let valid = match submission.phase {
            R13ScheduledSubmissionPhaseV1::Queued => {
                self.current && resource_state_matches(false, false, false)
            }
            R13ScheduledSubmissionPhaseV1::Published { lane } => {
                self.current
                    && self.lanes.get(lane).copied().flatten() == Some(submission.key)
                    && resource_state_matches(true, true, false)
            }
            R13ScheduledSubmissionPhaseV1::Terminal(_) => {
                self.current && resource_state_matches(true, false, false)
            }
            R13ScheduledSubmissionPhaseV1::CancelledBeforePublication => {
                resource_state_matches(false, false, false)
            }
            R13ScheduledSubmissionPhaseV1::Indeterminate { lane, .. } => {
                !self.current
                    && match lane {
                        Some(lane) => {
                            self.lanes.get(lane).copied().flatten() == Some(submission.key)
                        }
                        None => true,
                    }
                    && resource_state_matches(true, lane.is_some(), true)
            }
            R13ScheduledSubmissionPhaseV1::Released(_) => {
                resource_state_matches(false, false, false)
            }
        };
        if valid {
            Ok(())
        } else {
            Err(R13SchedulerModelErrorV1::InvariantViolation)
        }
    }

    fn stream_index(&self, key: R13LogicalStreamKeyV1) -> Result<usize, R13SchedulerModelErrorV1> {
        self.streams
            .iter()
            .position(|stream| stream.key == key)
            .ok_or(R13SchedulerModelErrorV1::UnknownStream)
    }

    fn submission_index(
        &self,
        key: R13ScheduledSubmissionKeyV1,
    ) -> Result<usize, R13SchedulerModelErrorV1> {
        self.submissions
            .iter()
            .position(|submission| submission.key == key)
            .ok_or(R13SchedulerModelErrorV1::UnknownSubmission)
    }

    fn resource_index(
        &self,
        key: R13ScheduledResourceKeyV1,
    ) -> Result<usize, R13SchedulerModelErrorV1> {
        self.resources
            .iter()
            .position(|resource| resource.key == key)
            .ok_or(R13SchedulerModelErrorV1::UnknownResource)
    }
}

//! R11 backend-neutral completion, launch-contract, and batch-custody model.
//!
//! Every value in this module is caller-constructible mathematical state. The
//! model performs no backend call and does not refine the executable runtime,
//! KFD, HSA, HIP, compiler output, or machine code.

use alloc::vec::Vec;

pub const MAX_R11_SUBMISSIONS_V1: usize = 4_096;
pub const MAX_R11_EVENTS_V1: usize = 4_096;
pub const MAX_R11_CALLBACKS_V1: usize = 4_096;
pub const MAX_R11_MAPPINGS_V1: usize = 256;
pub const MAX_R11_BATCHES_V1: usize = 256;
pub const MAX_R11_BATCH_MAPPINGS_V1: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11RuntimeModelErrorV1 {
    CapacityExceeded,
    InvalidIdentity,
    DuplicateIdentity,
    UnknownSubmission,
    UnknownEvent,
    UnknownCallback,
    UnknownMapping,
    UnknownBatch,
    InvalidContract,
    Unsupported,
    RetainedByEvent,
    IllegalTransition,
    InvariantViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11CompletionStatusV1 {
    Pending,
    Succeeded,
    FailedBackend(i64),
    Cancelled,
    QuiescentWithoutResult,
}

impl R11CompletionStatusV1 {
    pub const fn is_terminal(self) -> bool {
        !matches!(self, Self::Pending)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R11CallbackRecordV1 {
    callback_id: u64,
    submission_id: u64,
    discharged: bool,
    observed_status: Option<R11CompletionStatusV1>,
}

impl R11CallbackRecordV1 {
    pub const fn callback_id(self) -> u64 {
        self.callback_id
    }

    pub const fn submission_id(self) -> u64 {
        self.submission_id
    }

    pub const fn discharged(self) -> bool {
        self.discharged
    }

    pub const fn observed_status(self) -> Option<R11CompletionStatusV1> {
        self.observed_status
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R11SubmissionRecordV1 {
    submission_id: u64,
    status: R11CompletionStatusV1,
    released: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct R11EventRecordV1 {
    event_id: u64,
    submission_id: u64,
}

/// Shared submission/event status with exact-once callback discharge history.
#[derive(Debug, Default)]
pub struct R11CompletionModelV1 {
    submissions: Vec<R11SubmissionRecordV1>,
    events: Vec<R11EventRecordV1>,
    callbacks: Vec<R11CallbackRecordV1>,
}

impl R11CompletionModelV1 {
    pub fn register_submission_model_only(
        &mut self,
        submission_id: u64,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if submission_id == 0 {
            return Err(R11RuntimeModelErrorV1::InvalidIdentity);
        }
        if self.submissions.len() >= MAX_R11_SUBMISSIONS_V1 {
            return Err(R11RuntimeModelErrorV1::CapacityExceeded);
        }
        if self
            .submissions
            .iter()
            .any(|submission| submission.submission_id == submission_id)
        {
            return Err(R11RuntimeModelErrorV1::DuplicateIdentity);
        }
        self.submissions.push(R11SubmissionRecordV1 {
            submission_id,
            status: R11CompletionStatusV1::Pending,
            released: false,
        });
        Ok(())
    }

    pub fn record_event_model_only(
        &mut self,
        event_id: u64,
        submission_id: u64,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if event_id == 0 {
            return Err(R11RuntimeModelErrorV1::InvalidIdentity);
        }
        if self.events.len() >= MAX_R11_EVENTS_V1 {
            return Err(R11RuntimeModelErrorV1::CapacityExceeded);
        }
        if self.events.iter().any(|event| event.event_id == event_id) {
            return Err(R11RuntimeModelErrorV1::DuplicateIdentity);
        }
        self.live_submission(submission_id)?;
        self.events.push(R11EventRecordV1 {
            event_id,
            submission_id,
        });
        Ok(())
    }

    pub fn register_callback_model_only(
        &mut self,
        callback_id: u64,
        submission_id: u64,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if callback_id == 0 {
            return Err(R11RuntimeModelErrorV1::InvalidIdentity);
        }
        if self.callbacks.len() >= MAX_R11_CALLBACKS_V1 {
            return Err(R11RuntimeModelErrorV1::CapacityExceeded);
        }
        if self
            .callbacks
            .iter()
            .any(|callback| callback.callback_id == callback_id)
        {
            return Err(R11RuntimeModelErrorV1::DuplicateIdentity);
        }
        let status = self.live_submission(submission_id)?.status;
        self.callbacks.push(R11CallbackRecordV1 {
            callback_id,
            submission_id,
            discharged: status.is_terminal(),
            observed_status: status.is_terminal().then_some(status),
        });
        Ok(())
    }

    pub fn observe_completion_model_only(
        &mut self,
        submission_id: u64,
        status: R11CompletionStatusV1,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if !status.is_terminal() {
            return Err(R11RuntimeModelErrorV1::InvalidContract);
        }
        let submission = self.live_submission_mut(submission_id)?;
        if submission.status.is_terminal() {
            return Err(R11RuntimeModelErrorV1::IllegalTransition);
        }
        submission.status = status;
        for callback in &mut self.callbacks {
            if callback.submission_id == submission_id && !callback.discharged {
                callback.discharged = true;
                callback.observed_status = Some(status);
            }
        }
        Ok(())
    }

    pub fn query_submission_model_only(
        &self,
        submission_id: u64,
    ) -> Result<R11CompletionStatusV1, R11RuntimeModelErrorV1> {
        Ok(self.live_submission(submission_id)?.status)
    }

    pub fn query_event_model_only(
        &self,
        event_id: u64,
    ) -> Result<R11CompletionStatusV1, R11RuntimeModelErrorV1> {
        let event = self
            .events
            .iter()
            .find(|event| event.event_id == event_id)
            .ok_or(R11RuntimeModelErrorV1::UnknownEvent)?;
        self.query_submission_model_only(event.submission_id)
    }

    pub fn release_event_model_only(
        &mut self,
        event_id: u64,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        let index = self
            .events
            .iter()
            .position(|event| event.event_id == event_id)
            .ok_or(R11RuntimeModelErrorV1::UnknownEvent)?;
        self.events.remove(index);
        Ok(())
    }

    pub fn release_submission_model_only(
        &mut self,
        submission_id: u64,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if self
            .events
            .iter()
            .any(|event| event.submission_id == submission_id)
        {
            return Err(R11RuntimeModelErrorV1::RetainedByEvent);
        }
        let callbacks_complete = self
            .callbacks
            .iter()
            .filter(|callback| callback.submission_id == submission_id)
            .all(|callback| callback.discharged);
        let submission = self.live_submission_mut(submission_id)?;
        if !submission.status.is_terminal() || !callbacks_complete {
            return Err(R11RuntimeModelErrorV1::IllegalTransition);
        }
        submission.released = true;
        Ok(())
    }

    pub fn callback(
        &self,
        callback_id: u64,
    ) -> Result<R11CallbackRecordV1, R11RuntimeModelErrorV1> {
        self.callbacks
            .iter()
            .find(|callback| callback.callback_id == callback_id)
            .copied()
            .ok_or(R11RuntimeModelErrorV1::UnknownCallback)
    }

    pub fn validate_global_invariants(&self) -> Result<(), R11RuntimeModelErrorV1> {
        for (index, submission) in self.submissions.iter().enumerate() {
            if submission.submission_id == 0
                || self.submissions[..index]
                    .iter()
                    .any(|prior| prior.submission_id == submission.submission_id)
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
            if submission.released
                && (!submission.status.is_terminal()
                    || self
                        .events
                        .iter()
                        .any(|event| event.submission_id == submission.submission_id))
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
        }
        for (index, event) in self.events.iter().enumerate() {
            if event.event_id == 0
                || self.events[..index]
                    .iter()
                    .any(|prior| prior.event_id == event.event_id)
                || self.live_submission(event.submission_id).is_err()
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
        }
        for (index, callback) in self.callbacks.iter().enumerate() {
            let submission = self
                .submissions
                .iter()
                .find(|submission| submission.submission_id == callback.submission_id)
                .ok_or(R11RuntimeModelErrorV1::InvariantViolation)?;
            if callback.callback_id == 0
                || self.callbacks[..index]
                    .iter()
                    .any(|prior| prior.callback_id == callback.callback_id)
                || callback.discharged != callback.observed_status.is_some()
                || callback.discharged != submission.status.is_terminal()
                || callback
                    .observed_status
                    .is_some_and(|status| status != submission.status)
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
        }
        Ok(())
    }

    fn live_submission(
        &self,
        submission_id: u64,
    ) -> Result<&R11SubmissionRecordV1, R11RuntimeModelErrorV1> {
        self.submissions
            .iter()
            .find(|submission| submission.submission_id == submission_id && !submission.released)
            .ok_or(R11RuntimeModelErrorV1::UnknownSubmission)
    }

    fn live_submission_mut(
        &mut self,
        submission_id: u64,
    ) -> Result<&mut R11SubmissionRecordV1, R11RuntimeModelErrorV1> {
        self.submissions
            .iter_mut()
            .find(|submission| submission.submission_id == submission_id && !submission.released)
            .ok_or(R11RuntimeModelErrorV1::UnknownSubmission)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11MemoryScopeV1 {
    Workgroup,
    Device,
    System,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11MemoryOrderV1 {
    Relaxed,
    Acquire,
    Release,
    AcquireRelease,
    SequentiallyConsistent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11AtomicOperationV1 {
    Add,
    Minimum,
    Maximum,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Exchange,
    CompareExchange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R11LaunchGeometryV1 {
    pub grid: [u32; 3],
    pub workgroup: [u32; 3],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R11AtomicLaunchContractV1 {
    pub operation: R11AtomicOperationV1,
    pub scope: R11MemoryScopeV1,
    pub order: R11MemoryOrderV1,
    pub failure_order: Option<R11MemoryOrderV1>,
    pub weak: bool,
    pub geometry: R11LaunchGeometryV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R11ExecutionCapabilitiesV1 {
    pub stable: bool,
    pub execution_detail: bool,
}

pub fn admit_atomic_launch_model_only(
    declared: R11AtomicLaunchContractV1,
    requested: R11AtomicLaunchContractV1,
    capabilities: R11ExecutionCapabilitiesV1,
) -> Result<R11AtomicLaunchContractV1, R11RuntimeModelErrorV1> {
    if !capabilities.stable || !capabilities.execution_detail {
        return Err(R11RuntimeModelErrorV1::Unsupported);
    }
    if declared != requested
        || !geometry_valid(requested.geometry)
        || !atomic_contract_is_legal(requested)
    {
        return Err(R11RuntimeModelErrorV1::InvalidContract);
    }
    Ok(requested)
}

fn atomic_contract_is_legal(contract: R11AtomicLaunchContractV1) -> bool {
    match (contract.operation, contract.failure_order) {
        (R11AtomicOperationV1::CompareExchange, Some(failure)) => {
            valid_compare_exchange_order_pair(contract.order, failure)
        }
        (R11AtomicOperationV1::CompareExchange, None) => false,
        (_, None) => !contract.weak,
        (_, Some(_)) => false,
    }
}

fn valid_compare_exchange_order_pair(success: R11MemoryOrderV1, failure: R11MemoryOrderV1) -> bool {
    match success {
        R11MemoryOrderV1::Relaxed => matches!(failure, R11MemoryOrderV1::Relaxed),
        R11MemoryOrderV1::Acquire => {
            matches!(
                failure,
                R11MemoryOrderV1::Relaxed | R11MemoryOrderV1::Acquire
            )
        }
        R11MemoryOrderV1::Release => matches!(failure, R11MemoryOrderV1::Relaxed),
        R11MemoryOrderV1::AcquireRelease => {
            matches!(
                failure,
                R11MemoryOrderV1::Relaxed | R11MemoryOrderV1::Acquire
            )
        }
        R11MemoryOrderV1::SequentiallyConsistent => matches!(
            failure,
            R11MemoryOrderV1::Relaxed
                | R11MemoryOrderV1::Acquire
                | R11MemoryOrderV1::SequentiallyConsistent
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11CollectiveOperationV1 {
    Barrier,
    Broadcast,
    ReduceSum,
    ReduceMinimum,
    ReduceMaximum,
    AllReduceSum,
    InclusiveScanSum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R11CollectiveLaunchContractV1 {
    pub operation: R11CollectiveOperationV1,
    pub scope: R11MemoryScopeV1,
    pub order: R11MemoryOrderV1,
    pub participants: u64,
    pub geometry: R11LaunchGeometryV1,
}

pub fn admit_collective_launch_model_only(
    declared: R11CollectiveLaunchContractV1,
    requested: R11CollectiveLaunchContractV1,
    capabilities: R11ExecutionCapabilitiesV1,
) -> Result<R11CollectiveLaunchContractV1, R11RuntimeModelErrorV1> {
    if !capabilities.stable || !capabilities.execution_detail {
        return Err(R11RuntimeModelErrorV1::Unsupported);
    }
    if declared != requested || !complete_workgroup_geometry_valid(requested.geometry) {
        return Err(R11RuntimeModelErrorV1::InvalidContract);
    }
    let participants = match requested.scope {
        R11MemoryScopeV1::Workgroup => product(requested.geometry.workgroup),
        R11MemoryScopeV1::Device => product(requested.geometry.grid),
        R11MemoryScopeV1::System => None,
    };
    if participants != Some(requested.participants) || requested.participants == 0 {
        return Err(R11RuntimeModelErrorV1::InvalidContract);
    }
    Ok(requested)
}

fn geometry_valid(geometry: R11LaunchGeometryV1) -> bool {
    !geometry.grid.contains(&0)
        && !geometry.workgroup.contains(&0)
        && geometry
            .workgroup
            .into_iter()
            .try_fold(1_u32, u32::checked_mul)
            .is_some()
}

fn complete_workgroup_geometry_valid(geometry: R11LaunchGeometryV1) -> bool {
    geometry_valid(geometry)
        && geometry
            .grid
            .into_iter()
            .zip(geometry.workgroup)
            .all(|(grid, workgroup)| grid >= workgroup && grid.is_multiple_of(workgroup))
}

fn product(extent: [u32; 3]) -> Option<u64> {
    extent.into_iter().try_fold(1_u64, |product, value| {
        product.checked_mul(u64::from(value))
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct R11PersistentMappingKeyV1 {
    pub mapping_id: u64,
    pub generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum R11PersistentMappingPhaseV1 {
    Active,
    RetainedByBatch(u64),
    Quarantined,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct R11PersistentMappingRecordV1 {
    key: R11PersistentMappingKeyV1,
    phase: R11PersistentMappingPhaseV1,
}

impl R11PersistentMappingRecordV1 {
    pub const fn key(self) -> R11PersistentMappingKeyV1 {
        self.key
    }

    pub const fn phase(self) -> R11PersistentMappingPhaseV1 {
        self.phase
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct R11PersistentBatchRecordV1 {
    batch_id: u64,
    mappings: Vec<R11PersistentMappingKeyV1>,
    active: bool,
    indeterminate: bool,
}

/// Persistent mapping custody across all packets in an abstract batch.
#[derive(Debug, Default)]
pub struct R11PersistentBatchModelV1 {
    mappings: Vec<R11PersistentMappingRecordV1>,
    batches: Vec<R11PersistentBatchRecordV1>,
}

impl R11PersistentBatchModelV1 {
    pub fn register_mapping_model_only(
        &mut self,
        key: R11PersistentMappingKeyV1,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if key.mapping_id == 0 || key.generation == 0 {
            return Err(R11RuntimeModelErrorV1::InvalidIdentity);
        }
        if self.mappings.len() >= MAX_R11_MAPPINGS_V1 {
            return Err(R11RuntimeModelErrorV1::CapacityExceeded);
        }
        if self
            .mappings
            .iter()
            .any(|mapping| mapping.key.mapping_id == key.mapping_id)
        {
            return Err(R11RuntimeModelErrorV1::DuplicateIdentity);
        }
        self.mappings.push(R11PersistentMappingRecordV1 {
            key,
            phase: R11PersistentMappingPhaseV1::Active,
        });
        Ok(())
    }

    pub fn begin_batch_model_only(
        &mut self,
        batch_id: u64,
        mappings: Vec<R11PersistentMappingKeyV1>,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        if batch_id == 0 || mappings.is_empty() {
            return Err(R11RuntimeModelErrorV1::InvalidIdentity);
        }
        if self.batches.len() >= MAX_R11_BATCHES_V1 || mappings.len() > MAX_R11_BATCH_MAPPINGS_V1 {
            return Err(R11RuntimeModelErrorV1::CapacityExceeded);
        }
        if self.batches.iter().any(|batch| batch.batch_id == batch_id)
            || mappings
                .iter()
                .enumerate()
                .any(|(index, mapping)| mappings[..index].contains(mapping))
        {
            return Err(R11RuntimeModelErrorV1::DuplicateIdentity);
        }
        for key in &mappings {
            let mapping = self
                .mappings
                .iter()
                .find(|mapping| mapping.key == *key)
                .ok_or(R11RuntimeModelErrorV1::UnknownMapping)?;
            if mapping.phase != R11PersistentMappingPhaseV1::Active {
                return Err(R11RuntimeModelErrorV1::IllegalTransition);
            }
        }
        for key in &mappings {
            self.mapping_mut(*key)?.phase = R11PersistentMappingPhaseV1::RetainedByBatch(batch_id);
        }
        self.batches.push(R11PersistentBatchRecordV1 {
            batch_id,
            mappings,
            active: true,
            indeterminate: false,
        });
        Ok(())
    }

    pub fn complete_batch_model_only(
        &mut self,
        batch_id: u64,
        conclusive: bool,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        let index = self
            .batches
            .iter()
            .position(|batch| batch.batch_id == batch_id)
            .ok_or(R11RuntimeModelErrorV1::UnknownBatch)?;
        if !self.batches[index].active {
            return Err(R11RuntimeModelErrorV1::IllegalTransition);
        }
        let mappings = self.batches[index].mappings.clone();
        for key in mappings {
            let mapping = self.mapping_mut(key)?;
            if mapping.phase != R11PersistentMappingPhaseV1::RetainedByBatch(batch_id) {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
            mapping.phase = if conclusive {
                R11PersistentMappingPhaseV1::Active
            } else {
                R11PersistentMappingPhaseV1::Quarantined
            };
        }
        self.batches[index].active = false;
        self.batches[index].indeterminate = !conclusive;
        Ok(())
    }

    pub fn release_mapping_model_only(
        &mut self,
        key: R11PersistentMappingKeyV1,
    ) -> Result<(), R11RuntimeModelErrorV1> {
        let mapping = self.mapping_mut(key)?;
        if mapping.phase != R11PersistentMappingPhaseV1::Active {
            return Err(R11RuntimeModelErrorV1::IllegalTransition);
        }
        mapping.phase = R11PersistentMappingPhaseV1::Released;
        Ok(())
    }

    pub fn mapping(
        &self,
        key: R11PersistentMappingKeyV1,
    ) -> Result<R11PersistentMappingRecordV1, R11RuntimeModelErrorV1> {
        self.mappings
            .iter()
            .find(|mapping| mapping.key == key)
            .copied()
            .ok_or(R11RuntimeModelErrorV1::UnknownMapping)
    }

    pub fn validate_global_invariants(&self) -> Result<(), R11RuntimeModelErrorV1> {
        for (index, mapping) in self.mappings.iter().enumerate() {
            if mapping.key.mapping_id == 0
                || mapping.key.generation == 0
                || self.mappings[..index]
                    .iter()
                    .any(|prior| prior.key.mapping_id == mapping.key.mapping_id)
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
            if let R11PersistentMappingPhaseV1::RetainedByBatch(batch_id) = mapping.phase
                && !self.batches.iter().any(|batch| {
                    batch.batch_id == batch_id
                        && batch.active
                        && batch.mappings.contains(&mapping.key)
                })
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
        }
        for (index, batch) in self.batches.iter().enumerate() {
            if batch.batch_id == 0
                || self.batches[..index]
                    .iter()
                    .any(|prior| prior.batch_id == batch.batch_id)
                || batch.mappings.is_empty()
                || batch
                    .mappings
                    .iter()
                    .enumerate()
                    .any(|(mapping_index, key)| batch.mappings[..mapping_index].contains(key))
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
            if batch.active
                && batch.mappings.iter().any(|key| {
                    self.mapping(*key).map_or(true, |mapping| {
                        mapping.phase
                            != R11PersistentMappingPhaseV1::RetainedByBatch(batch.batch_id)
                    })
                })
            {
                return Err(R11RuntimeModelErrorV1::InvariantViolation);
            }
        }
        Ok(())
    }

    fn mapping_mut(
        &mut self,
        key: R11PersistentMappingKeyV1,
    ) -> Result<&mut R11PersistentMappingRecordV1, R11RuntimeModelErrorV1> {
        self.mappings
            .iter_mut()
            .find(|mapping| mapping.key == key)
            .ok_or(R11RuntimeModelErrorV1::UnknownMapping)
    }
}

//! Bounded carrier for the issue #135 abstract service transition system.
//!
//! The specification's logical generation is a natural number. This finite
//! executable-free carrier uses `u64` and rejects exhaustion instead of
//! treating wraparound as a semantic transition. Concrete bounded generation
//! words still require a separately named refinement and ABA proof.

use alloc::vec::Vec;
use core::fmt;

use crate::{
    DeliveryPolicyV1, IdentityDigestV1, MAX_QUEUE_CAPACITY_V1, SchedulerModelIdV1, ServiceRunIdV1,
    TaskSchemaIdV1,
};

pub const SERVICE_STATE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_MODEL_TASKS_V1: usize = 32_768;
pub const MAX_MODEL_LEASES_V1: usize = 32_768;
pub const MAX_MODEL_WORKERS_V1: usize = 4_096;
pub const MAX_MODEL_DEPENDENCIES_V1: usize = 65_536;
pub const MAX_MODEL_PHASE_REGIONS_V1: usize = 8_192;
pub const MAX_MODEL_COMPLETIONS_V1: usize = 32_768;
pub const MAX_INVARIANT_VIOLATIONS_V1: usize = 256;

macro_rules! numeric_id {
    ($name:ident, $inner:ty) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(pub $inner);
    };
}

numeric_id!(SlotIdV1, u16);
numeric_id!(TaskIdV1, u64);
numeric_id!(LeaseIdV1, u64);
numeric_id!(WorkerIdV1, u16);
numeric_id!(WorkgroupIdV1, u16);
numeric_id!(RegionIdV1, u16);
numeric_id!(PhaseIdV1, u16);
numeric_id!(DependencyEpochV1, u64);
numeric_id!(AcquisitionEventIdV1, u64);
numeric_id!(CompletionRecordIdV1, u64);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SlotKeyV1 {
    pub run_id: ServiceRunIdV1,
    pub service_epoch: u64,
    pub queue_identity: IdentityDigestV1,
    pub slot_id: SlotIdV1,
    pub logical_generation: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FailureDispositionV1 {
    DeviceMayStillAccess = 1,
    DeviceQuiesced = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleStateV1 {
    Starting,
    Running,
    Draining,
    Stopping,
    Stopped,
    Failed(FailureDispositionV1),
}

impl LifecycleStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        matches!(
            (self, next),
            (Self::Starting, Self::Running)
                | (Self::Starting, Self::Failed(_))
                | (Self::Running, Self::Draining)
                | (Self::Running, Self::Stopping)
                | (Self::Running, Self::Failed(_))
                | (Self::Draining, Self::Stopping)
                | (Self::Draining, Self::Failed(_))
                | (Self::Stopping, Self::Stopped)
                | (Self::Stopping, Self::Failed(_))
                | (
                    Self::Failed(FailureDispositionV1::DeviceMayStillAccess),
                    Self::Failed(FailureDispositionV1::DeviceQuiesced)
                )
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TaskOutcomeV1 {
    Succeeded = 1,
    Cancelled = 2,
    Failed = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CancellationStageV1 {
    Reserved = 1,
    Initialized = 2,
    Published = 3,
    Acquired = 4,
    Executing = 5,
    CompletionPending = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskStateV1 {
    Accepted,
    Reserved(SlotKeyV1),
    Initialized(SlotKeyV1),
    Published(SlotKeyV1),
    Acquired {
        slot: SlotKeyV1,
        lease_id: LeaseIdV1,
    },
    Executing {
        slot: SlotKeyV1,
        lease_id: LeaseIdV1,
        phase_id: PhaseIdV1,
    },
    CompletionPending {
        slot: SlotKeyV1,
        lease_id: LeaseIdV1,
        outcome: TaskOutcomeV1,
    },
    Completed {
        slot: SlotKeyV1,
        record_id: CompletionRecordIdV1,
    },
    Cancelled {
        slot: SlotKeyV1,
        stage: CancellationStageV1,
        record_id: CompletionRecordIdV1,
    },
    Failed {
        slot: Option<SlotKeyV1>,
        record_id: Option<CompletionRecordIdV1>,
    },
}

impl TaskStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (Self::Accepted, Self::Reserved(_)) => true,
            (Self::Accepted, Self::Failed { slot: None, .. }) => true,
            (Self::Reserved(left), Self::Initialized(right)) => left == right,
            (Self::Initialized(left), Self::Published(right)) => left == right,
            (Self::Published(left), Self::Acquired { slot: right, .. }) => left == right,
            (
                Self::Acquired {
                    slot: left,
                    lease_id: left_lease,
                },
                Self::Executing {
                    slot: right,
                    lease_id: right_lease,
                    ..
                },
            ) => left == right && left_lease == right_lease,
            (
                Self::Executing {
                    slot: left,
                    lease_id: left_lease,
                    ..
                },
                Self::CompletionPending {
                    slot: right,
                    lease_id: right_lease,
                    ..
                },
            ) => left == right && left_lease == right_lease,
            (
                Self::CompletionPending {
                    slot: left,
                    outcome: TaskOutcomeV1::Succeeded,
                    ..
                },
                Self::Completed { slot: right, .. },
            ) => left == right,
            (current, Self::Cancelled { slot, stage, .. }) => {
                current.slot_key() == Some(slot) && current.cancellation_stage() == Some(stage)
            }
            (current, Self::Failed { slot, .. }) => {
                !current.is_terminal() && slot == current.slot_key()
            }
            _ => false,
        }
    }

    pub const fn slot_key(self) -> Option<SlotKeyV1> {
        match self {
            Self::Accepted => None,
            Self::Reserved(slot)
            | Self::Initialized(slot)
            | Self::Published(slot)
            | Self::Acquired { slot, .. }
            | Self::Executing { slot, .. }
            | Self::CompletionPending { slot, .. }
            | Self::Completed { slot, .. }
            | Self::Cancelled { slot, .. } => Some(slot),
            Self::Failed { slot, .. } => slot,
        }
    }

    pub const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Completed { .. } | Self::Cancelled { .. } | Self::Failed { .. }
        )
    }

    pub const fn live_lease(self) -> Option<LeaseIdV1> {
        match self {
            Self::Acquired { lease_id, .. }
            | Self::Executing { lease_id, .. }
            | Self::CompletionPending { lease_id, .. } => Some(lease_id),
            _ => None,
        }
    }

    const fn cancellation_stage(self) -> Option<CancellationStageV1> {
        match self {
            Self::Reserved(_) => Some(CancellationStageV1::Reserved),
            Self::Initialized(_) => Some(CancellationStageV1::Initialized),
            Self::Published(_) => Some(CancellationStageV1::Published),
            Self::Acquired { .. } => Some(CancellationStageV1::Acquired),
            Self::Executing { .. } => Some(CancellationStageV1::Executing),
            Self::CompletionPending { .. } => Some(CancellationStageV1::CompletionPending),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueSlotStateV1 {
    Empty {
        generation: u64,
    },
    Reserved {
        generation: u64,
        task_id: TaskIdV1,
    },
    Initialized {
        generation: u64,
        task_id: TaskIdV1,
    },
    Published {
        generation: u64,
        task_id: TaskIdV1,
    },
    Acquired {
        generation: u64,
        task_id: TaskIdV1,
        lease_id: LeaseIdV1,
    },
    Executing {
        generation: u64,
        task_id: TaskIdV1,
        lease_id: LeaseIdV1,
    },
    Completed {
        generation: u64,
        task_id: TaskIdV1,
        outcome: TaskOutcomeV1,
        record_id: CompletionRecordIdV1,
    },
    Reclaimable {
        generation: u64,
        task_id: TaskIdV1,
        outcome: TaskOutcomeV1,
        record_id: CompletionRecordIdV1,
    },
}

impl QueueSlotStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (
                Self::Empty { generation: left },
                Self::Reserved {
                    generation: right, ..
                },
            ) => left == right,
            (
                Self::Reserved {
                    generation: left,
                    task_id: left_task,
                },
                Self::Initialized {
                    generation: right,
                    task_id: right_task,
                },
            )
            | (
                Self::Initialized {
                    generation: left,
                    task_id: left_task,
                },
                Self::Published {
                    generation: right,
                    task_id: right_task,
                },
            ) => left == right && left_task == right_task,
            (
                Self::Published {
                    generation: left,
                    task_id: left_task,
                },
                Self::Acquired {
                    generation: right,
                    task_id: right_task,
                    ..
                },
            ) => left == right && left_task == right_task,
            (
                Self::Acquired {
                    generation: left,
                    task_id: left_task,
                    lease_id: left_lease,
                },
                Self::Executing {
                    generation: right,
                    task_id: right_task,
                    lease_id: right_lease,
                },
            ) => left == right && left_task == right_task && left_lease == right_lease,
            (
                Self::Executing {
                    generation: left,
                    task_id: left_task,
                    lease_id: _,
                },
                Self::Completed {
                    generation: right,
                    task_id: right_task,
                    ..
                },
            ) => left == right && left_task == right_task,
            (
                Self::Completed {
                    generation: left,
                    task_id: left_task,
                    outcome: left_outcome,
                    record_id: left_record,
                },
                Self::Reclaimable {
                    generation: right,
                    task_id: right_task,
                    outcome: right_outcome,
                    record_id: right_record,
                },
            ) => {
                left == right
                    && left_task == right_task
                    && left_outcome == right_outcome
                    && left_record == right_record
            }
            (
                Self::Reclaimable {
                    generation: left, ..
                },
                Self::Empty { generation: right },
            ) => left.checked_add(1) == Some(right),
            (
                current,
                Self::Completed {
                    generation,
                    task_id,
                    outcome,
                    ..
                },
            ) => {
                current.generation() == generation
                    && current.task_id() == Some(task_id)
                    && outcome != TaskOutcomeV1::Succeeded
                    && matches!(
                        current,
                        Self::Reserved { .. }
                            | Self::Initialized { .. }
                            | Self::Published { .. }
                            | Self::Acquired { .. }
                            | Self::Executing { .. }
                    )
            }
            _ => false,
        }
    }

    pub const fn generation(self) -> u64 {
        match self {
            Self::Empty { generation }
            | Self::Reserved { generation, .. }
            | Self::Initialized { generation, .. }
            | Self::Published { generation, .. }
            | Self::Acquired { generation, .. }
            | Self::Executing { generation, .. }
            | Self::Completed { generation, .. }
            | Self::Reclaimable { generation, .. } => generation,
        }
    }

    pub const fn task_id(self) -> Option<TaskIdV1> {
        match self {
            Self::Empty { .. } => None,
            Self::Reserved { task_id, .. }
            | Self::Initialized { task_id, .. }
            | Self::Published { task_id, .. }
            | Self::Acquired { task_id, .. }
            | Self::Executing { task_id, .. }
            | Self::Completed { task_id, .. }
            | Self::Reclaimable { task_id, .. } => Some(task_id),
        }
    }

    pub const fn live_lease(self) -> Option<LeaseIdV1> {
        match self {
            Self::Acquired { lease_id, .. } | Self::Executing { lease_id, .. } => Some(lease_id),
            _ => None,
        }
    }

    pub const fn is_empty(self) -> bool {
        matches!(self, Self::Empty { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseKeyV1 {
    pub slot: SlotKeyV1,
    pub task_id: TaskIdV1,
    pub acquisition_event: AcquisitionEventIdV1,
    pub worker_id: WorkerIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LeaseStateV1 {
    Issued(LeaseKeyV1),
    Executing(LeaseKeyV1),
    Consumed {
        key: LeaseKeyV1,
        outcome: TaskOutcomeV1,
        record_id: CompletionRecordIdV1,
    },
}

impl LeaseStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (Self::Issued(left), Self::Executing(right)) => left == right,
            (Self::Issued(left) | Self::Executing(left), Self::Consumed { key: right, .. }) => {
                left == right
            }
            _ => false,
        }
    }

    pub const fn key(self) -> LeaseKeyV1 {
        match self {
            Self::Issued(key) | Self::Executing(key) | Self::Consumed { key, .. } => key,
        }
    }

    pub const fn is_live(self) -> bool {
        !matches!(self, Self::Consumed { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenerationCounterV1 {
    pub logical: u64,
    pub encoded: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationStateV1 {
    Current(GenerationCounterV1),
    Exhausted(GenerationCounterV1),
}

impl GenerationStateV1 {
    pub const fn counter(self) -> GenerationCounterV1 {
        match self {
            Self::Current(counter) | Self::Exhausted(counter) => counter,
        }
    }

    /// Checks either an unchanged observation or the one-step reclaim edge.
    pub fn can_transition_to(self, next: Self, reclaim: bool, generation_modulus: u64) -> bool {
        if generation_modulus < 2 {
            return false;
        }
        if !reclaim {
            return self == next;
        }
        let (Self::Current(left), Self::Current(right)) = (self, next) else {
            return false;
        };
        left.logical.checked_add(1) == Some(right.logical)
            && right.encoded == right.logical % generation_modulus
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AbaStateV1 {
    Protected { oldest_live_generation: Option<u64> },
    WrapRisk { oldest_live_generation: u64 },
}

impl AbaStateV1 {
    pub fn is_admissible(self, logical_generation: u64, maximum_live_span: u64) -> bool {
        match self {
            Self::Protected {
                oldest_live_generation: None,
            } => true,
            Self::Protected {
                oldest_live_generation: Some(oldest),
            } => oldest <= logical_generation && logical_generation - oldest <= maximum_live_span,
            Self::WrapRisk { .. } => false,
        }
    }

    /// Allows references to be introduced at the current generation or
    /// discharged monotonically. The recorded oldest generation cannot move
    /// backward.
    pub fn can_transition_to(
        self,
        next: Self,
        logical_generation: u64,
        maximum_live_span: u64,
    ) -> bool {
        if !next.is_admissible(logical_generation, maximum_live_span) {
            return false;
        }
        match (self, next) {
            (Self::WrapRisk { .. }, _) => false,
            (
                Self::Protected {
                    oldest_live_generation: Some(left),
                },
                Self::Protected {
                    oldest_live_generation: Some(right),
                },
            ) => right >= left,
            (Self::Protected { .. }, Self::Protected { .. }) => true,
            (_, Self::WrapRisk { .. }) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerStateV1 {
    Starting,
    Idle,
    Acquiring,
    Running {
        task_id: TaskIdV1,
        lease_id: LeaseIdV1,
        phase_id: PhaseIdV1,
    },
    Publishing {
        task_id: TaskIdV1,
        lease_id: LeaseIdV1,
    },
    Exiting,
    Exited,
    Failed,
}

impl WorkerStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (Self::Starting, Self::Idle)
            | (Self::Idle, Self::Acquiring)
            | (Self::Acquiring, Self::Idle)
            | (Self::Acquiring, Self::Running { .. })
            | (Self::Publishing { .. }, Self::Acquiring)
            | (Self::Publishing { .. }, Self::Idle)
            | (Self::Idle, Self::Exiting)
            | (Self::Acquiring, Self::Exiting)
            | (Self::Exiting, Self::Exited)
            | (Self::Starting, Self::Failed)
            | (Self::Idle, Self::Failed)
            | (Self::Acquiring, Self::Failed)
            | (Self::Running { .. }, Self::Failed)
            | (Self::Publishing { .. }, Self::Failed)
            | (Self::Exiting, Self::Failed) => true,
            (
                Self::Running {
                    task_id: left_task,
                    lease_id: left_lease,
                    ..
                },
                Self::Publishing {
                    task_id: right_task,
                    lease_id: right_lease,
                },
            ) => left_task == right_task && left_lease == right_lease,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyStateV1 {
    Pending,
    CompletionPublished { producer_task: TaskIdV1 },
    VisibleSatisfied { producer_task: TaskIdV1 },
    Failed { producer_task: TaskIdV1 },
}

impl DependencyStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (Self::Pending, Self::CompletionPublished { .. } | Self::Failed { .. }) => true,
            (
                Self::CompletionPublished {
                    producer_task: left,
                },
                Self::VisibleSatisfied {
                    producer_task: right,
                },
            ) => left == right,
            _ => false,
        }
    }

    pub const fn is_visible_satisfied(self) -> bool {
        matches!(self, Self::VisibleSatisfied { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseOwnerV1 {
    Worker(WorkerIdV1),
    Workgroup(WorkgroupIdV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhaseStateV1 {
    Inactive {
        epoch: u64,
    },
    Active {
        epoch: u64,
        phase_id: PhaseIdV1,
        owner: PhaseOwnerV1,
    },
    Completed {
        epoch: u64,
        phase_id: PhaseIdV1,
        owner: PhaseOwnerV1,
    },
    Retired {
        epoch: u64,
    },
}

impl PhaseStateV1 {
    pub fn can_transition_to(self, next: Self) -> bool {
        if self == next {
            return true;
        }
        match (self, next) {
            (Self::Inactive { epoch: left }, Self::Active { epoch: right, .. }) => left == right,
            (
                Self::Active {
                    epoch: left,
                    phase_id: left_phase,
                    owner: left_owner,
                },
                Self::Completed {
                    epoch: right,
                    phase_id: right_phase,
                    owner: right_owner,
                },
            ) => left == right && left_phase == right_phase && left_owner == right_owner,
            (Self::Completed { epoch: left, .. }, Self::Inactive { epoch: right }) => {
                left.checked_add(1) == Some(right)
            }
            (Self::Inactive { epoch: left }, Self::Retired { epoch: right }) => left == right,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceModelConfigV1 {
    pub run_id: ServiceRunIdV1,
    pub service_epoch: u64,
    pub queue_identity: IdentityDigestV1,
    pub task_schema_id: TaskSchemaIdV1,
    pub scheduler_model_id: SchedulerModelIdV1,
    pub admitted_task_tags: Vec<u32>,
    pub queue_capacity: u16,
    pub generation_modulus: u64,
    pub maximum_live_generation_span: u64,
    pub delivery_policy: DeliveryPolicyV1,
    pub failure_model_id: IdentityDigestV1,
}

impl ServiceModelConfigV1 {
    pub fn validate(&self) -> Result<(), InvariantViolationV1> {
        if self.queue_capacity == 0 || self.queue_capacity > MAX_QUEUE_CAPACITY_V1 {
            return Err(InvariantViolationV1::InvalidConfiguration);
        }
        if self.generation_modulus < 2
            || self.maximum_live_generation_span >= self.generation_modulus
        {
            return Err(InvariantViolationV1::InvalidConfiguration);
        }
        if self.admitted_task_tags.is_empty()
            || self.admitted_task_tags.len() > crate::MAX_TASK_VARIANTS_V1
            || self
                .admitted_task_tags
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(InvariantViolationV1::InvalidConfiguration);
        }
        Ok(())
    }

    pub fn slot_key(&self, slot_id: SlotIdV1, logical_generation: u64) -> SlotKeyV1 {
        SlotKeyV1 {
            run_id: self.run_id,
            service_epoch: self.service_epoch,
            queue_identity: self.queue_identity,
            slot_id,
            logical_generation,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueueSlotRecordV1 {
    pub slot_id: SlotIdV1,
    pub generation: GenerationStateV1,
    pub aba: AbaStateV1,
    pub state: QueueSlotStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TaskRecordV1 {
    pub task_id: TaskIdV1,
    pub canonical_tag: u32,
    pub payload_identity: IdentityDigestV1,
    pub submission_sequence: u64,
    pub dependencies: Vec<DependencyEpochV1>,
    pub state: TaskStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LeaseRecordV1 {
    pub lease_id: LeaseIdV1,
    pub state: LeaseStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerRecordV1 {
    pub worker_id: WorkerIdV1,
    pub state: WorkerStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DependencyRecordV1 {
    pub epoch: DependencyEpochV1,
    pub state: DependencyStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhaseRegionRecordV1 {
    pub region_id: RegionIdV1,
    pub state: PhaseStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionRecordV1 {
    pub record_id: CompletionRecordIdV1,
    pub task_id: TaskIdV1,
    pub slot: SlotKeyV1,
    pub outcome: TaskOutcomeV1,
    pub visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FailureRecordV1 {
    pub failure_model_id: IdentityDigestV1,
    pub failure_event_id: IdentityDigestV1,
    pub disposition: FailureDispositionV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceStateV1 {
    pub config: ServiceModelConfigV1,
    pub lifecycle: LifecycleStateV1,
    pub admission_cutoff: Option<u64>,
    pub slots: Vec<QueueSlotRecordV1>,
    pub tasks: Vec<TaskRecordV1>,
    pub leases: Vec<LeaseRecordV1>,
    pub workers: Vec<WorkerRecordV1>,
    pub dependencies: Vec<DependencyRecordV1>,
    pub phase_regions: Vec<PhaseRegionRecordV1>,
    pub completion_records: Vec<CompletionRecordV1>,
    pub failure: Option<FailureRecordV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvariantViolationV1 {
    InvalidConfiguration,
    ModelBoundExceeded(&'static str),
    DuplicateSlot(SlotIdV1),
    MissingSlot(SlotIdV1),
    SlotOutOfBounds(SlotIdV1),
    GenerationEncodingMismatch(SlotIdV1),
    GenerationStateMismatch(SlotIdV1),
    GenerationExhausted(SlotIdV1),
    AbaWrapRisk(SlotIdV1),
    DuplicateTask(TaskIdV1),
    UnknownTaskTag(TaskIdV1),
    DuplicateLease(LeaseIdV1),
    DuplicateWorker(WorkerIdV1),
    DuplicateDependency(DependencyEpochV1),
    DuplicatePhaseRegion(RegionIdV1),
    DuplicateCompletion(CompletionRecordIdV1),
    InvalidSlotBrand(SlotIdV1),
    SlotTaskMismatch(SlotIdV1),
    TaskSlotMismatch(TaskIdV1),
    LeaseMismatch(LeaseIdV1),
    DuplicateLiveLease(TaskIdV1),
    WorkerLeaseMismatch(WorkerIdV1),
    DependencyUnsatisfied(TaskIdV1),
    DuplicateTaskDependency(TaskIdV1),
    PhaseOwnershipMismatch(RegionIdV1),
    CompletionMismatch(CompletionRecordIdV1),
    AdmissionCutoffMismatch,
    FailureRecordMismatch,
    StoppedButNotQuiescent,
    ViolationLimitReached,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvariantReportV1 {
    violations: Vec<InvariantViolationV1>,
}

impl InvariantReportV1 {
    pub fn violations(&self) -> &[InvariantViolationV1] {
        &self.violations
    }
}

impl fmt::Display for InvariantReportV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} service-model invariant violation(s)",
            self.violations.len()
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransitionErrorV1 {
    CurrentStateInvalid(InvariantReportV1),
    NextStateInvalid(InvariantReportV1),
    ImmutableConfigurationChanged,
    RemovedTask(TaskIdV1),
    RemovedLease(LeaseIdV1),
    AddedTaskInInvalidState(TaskIdV1),
    AddedLeaseInInvalidState(LeaseIdV1),
    IllegalLifecycleTransition,
    IllegalSlotTransition(SlotIdV1),
    IllegalTaskTransition(TaskIdV1),
    IllegalLeaseTransition(LeaseIdV1),
    IllegalWorkerTransition(WorkerIdV1),
    IllegalDependencyTransition(DependencyEpochV1),
    IllegalPhaseTransition(RegionIdV1),
    IllegalCompletionTransition(CompletionRecordIdV1),
}

impl ServiceStateV1 {
    pub fn validate_global_invariants(&self) -> Result<(), InvariantReportV1> {
        let mut violations = Vec::new();
        if let Err(violation) = self.config.validate() {
            push_violation(&mut violations, violation);
        }
        check_bound(
            &mut violations,
            "slots",
            self.slots.len(),
            usize::from(self.config.queue_capacity),
        );
        if self.slots.len() != usize::from(self.config.queue_capacity) {
            for raw_id in 0..self.config.queue_capacity {
                let id = SlotIdV1(raw_id);
                if !self.slots.iter().any(|slot| slot.slot_id == id) {
                    push_violation(&mut violations, InvariantViolationV1::MissingSlot(id));
                }
            }
        }
        check_bound(
            &mut violations,
            "tasks",
            self.tasks.len(),
            MAX_MODEL_TASKS_V1,
        );
        check_bound(
            &mut violations,
            "leases",
            self.leases.len(),
            MAX_MODEL_LEASES_V1,
        );
        check_bound(
            &mut violations,
            "workers",
            self.workers.len(),
            MAX_MODEL_WORKERS_V1,
        );
        check_bound(
            &mut violations,
            "dependencies",
            self.dependencies.len(),
            MAX_MODEL_DEPENDENCIES_V1,
        );
        check_bound(
            &mut violations,
            "phase regions",
            self.phase_regions.len(),
            MAX_MODEL_PHASE_REGIONS_V1,
        );
        check_bound(
            &mut violations,
            "completions",
            self.completion_records.len(),
            MAX_MODEL_COMPLETIONS_V1,
        );

        for (index, slot) in self.slots.iter().enumerate() {
            if self.slots[..index]
                .iter()
                .any(|other| other.slot_id == slot.slot_id)
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::DuplicateSlot(slot.slot_id),
                );
            }
            if slot.slot_id.0 >= self.config.queue_capacity {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::SlotOutOfBounds(slot.slot_id),
                );
            }
            let counter = slot.generation.counter();
            if counter.encoded != counter.logical % self.config.generation_modulus {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::GenerationEncodingMismatch(slot.slot_id),
                );
            }
            if slot.state.generation() != counter.logical {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::GenerationStateMismatch(slot.slot_id),
                );
            }
            if matches!(slot.generation, GenerationStateV1::Exhausted(_)) {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::GenerationExhausted(slot.slot_id),
                );
            }
            if !slot
                .aba
                .is_admissible(counter.logical, self.config.maximum_live_generation_span)
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::AbaWrapRisk(slot.slot_id),
                );
            }
            if let Some(task_id) = slot.state.task_id() {
                self.check_slot_task(slot, task_id, &mut violations);
            }
        }

        check_unique_records(self, &mut violations);

        for task in &self.tasks {
            if self
                .config
                .admitted_task_tags
                .binary_search(&task.canonical_tag)
                .is_err()
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::UnknownTaskTag(task.task_id),
                );
            }
            if task
                .dependencies
                .iter()
                .enumerate()
                .any(|(index, epoch)| task.dependencies[..index].contains(epoch))
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::DuplicateTaskDependency(task.task_id),
                );
            }
            if let Some(slot_key) = task.state.slot_key() {
                if !self.valid_slot_brand(slot_key) {
                    push_violation(
                        &mut violations,
                        InvariantViolationV1::TaskSlotMismatch(task.task_id),
                    );
                }
                if !task.state.is_terminal() {
                    match self.slot(slot_key.slot_id) {
                        Some(slot)
                            if slot.state.task_id() == Some(task.task_id)
                                && slot.state.generation() == slot_key.logical_generation => {}
                        _ => push_violation(
                            &mut violations,
                            InvariantViolationV1::TaskSlotMismatch(task.task_id),
                        ),
                    }
                }
            }
            if matches!(
                task.state,
                TaskStateV1::Acquired { .. }
                    | TaskStateV1::Executing { .. }
                    | TaskStateV1::CompletionPending { .. }
            ) {
                for epoch in &task.dependencies {
                    if !self
                        .dependency(*epoch)
                        .is_some_and(|dependency| dependency.state.is_visible_satisfied())
                    {
                        push_violation(
                            &mut violations,
                            InvariantViolationV1::DependencyUnsatisfied(task.task_id),
                        );
                    }
                }
            }
            if let Some(lease_id) = task.state.live_lease()
                && !self.lease(lease_id).is_some_and(|lease| {
                    lease.state.is_live() && lease.state.key().task_id == task.task_id
                })
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::LeaseMismatch(lease_id),
                );
            }
        }

        for (index, lease) in self.leases.iter().enumerate() {
            let key = lease.state.key();
            if !self.valid_slot_brand(key.slot)
                || self.task(key.task_id).is_none()
                || self.worker(key.worker_id).is_none()
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::LeaseMismatch(lease.lease_id),
                );
            }
            if lease.state.is_live()
                && self.leases[..index].iter().any(|other| {
                    other.state.is_live()
                        && (other.state.key().task_id == key.task_id
                            || other.state.key().slot == key.slot)
                })
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::DuplicateLiveLease(key.task_id),
                );
            }
        }

        for worker in &self.workers {
            if let WorkerStateV1::Running {
                task_id,
                lease_id,
                phase_id,
            } = worker.state
            {
                let lease_matches = self.lease(lease_id).is_some_and(|lease| {
                    matches!(lease.state, LeaseStateV1::Executing(key) if key.worker_id == worker.worker_id && key.task_id == task_id)
                });
                let task_matches = self.task(task_id).is_some_and(|task| {
                    matches!(task.state,
                        TaskStateV1::Executing { lease_id: current, phase_id: current_phase, .. }
                            if current == lease_id && current_phase == phase_id
                    ) || matches!(task.state,
                        TaskStateV1::CompletionPending { lease_id: current, .. }
                            if current == lease_id
                    )
                });
                if !lease_matches || !task_matches {
                    push_violation(
                        &mut violations,
                        InvariantViolationV1::WorkerLeaseMismatch(worker.worker_id),
                    );
                }
                if !self.phase_regions.iter().any(|region| {
                    matches!(region.state, PhaseStateV1::Active { phase_id: current, owner: PhaseOwnerV1::Worker(owner), .. } if current == phase_id && owner == worker.worker_id)
                }) {
                    push_violation(&mut violations, InvariantViolationV1::WorkerLeaseMismatch(worker.worker_id));
                }
            }
        }

        for phase in &self.phase_regions {
            if let PhaseStateV1::Active {
                owner: PhaseOwnerV1::Worker(worker),
                ..
            } = phase.state
                && !self
                    .worker(worker)
                    .is_some_and(|record| matches!(record.state, WorkerStateV1::Running { .. }))
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::PhaseOwnershipMismatch(phase.region_id),
                );
            }
        }

        for completion in &self.completion_records {
            let task_matches = self
                .task(completion.task_id)
                .is_some_and(|task| match task.state {
                    TaskStateV1::Completed { slot, record_id } => {
                        completion.outcome == TaskOutcomeV1::Succeeded
                            && slot == completion.slot
                            && record_id == completion.record_id
                    }
                    TaskStateV1::Cancelled {
                        slot, record_id, ..
                    } => {
                        completion.outcome == TaskOutcomeV1::Cancelled
                            && slot == completion.slot
                            && record_id == completion.record_id
                    }
                    TaskStateV1::Failed {
                        slot: Some(slot),
                        record_id: Some(record_id),
                    } => {
                        completion.outcome == TaskOutcomeV1::Failed
                            && slot == completion.slot
                            && record_id == completion.record_id
                    }
                    _ => false,
                });
            if !task_matches || !self.valid_slot_brand(completion.slot) {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::CompletionMismatch(completion.record_id),
                );
            }
        }
        for task in &self.tasks {
            let referenced = match task.state {
                TaskStateV1::Completed { record_id, .. }
                | TaskStateV1::Cancelled { record_id, .. } => Some(record_id),
                TaskStateV1::Failed {
                    record_id: Some(record_id),
                    ..
                } => Some(record_id),
                _ => None,
            };
            if let Some(record_id) = referenced
                && !self
                    .completion_records
                    .iter()
                    .any(|record| record.record_id == record_id && record.task_id == task.task_id)
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::CompletionMismatch(record_id),
                );
            }
        }
        for slot in &self.slots {
            if let QueueSlotStateV1::Reclaimable { record_id, .. } = slot.state
                && !self
                    .completion_records
                    .iter()
                    .any(|record| record.record_id == record_id && record.visible)
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::CompletionMismatch(record_id),
                );
            }
        }
        for lease in &self.leases {
            if let LeaseStateV1::Consumed {
                key,
                outcome,
                record_id,
            } = lease.state
                && !self.completion_records.iter().any(|record| {
                    record.record_id == record_id
                        && record.task_id == key.task_id
                        && record.slot == key.slot
                        && record.outcome == outcome
                })
            {
                push_violation(
                    &mut violations,
                    InvariantViolationV1::CompletionMismatch(record_id),
                );
            }
        }

        match self.lifecycle {
            LifecycleStateV1::Draining | LifecycleStateV1::Stopping | LifecycleStateV1::Stopped => {
                if self.admission_cutoff.is_none()
                    || self.tasks.iter().any(|task| {
                        self.admission_cutoff
                            .is_some_and(|cutoff| task.submission_sequence > cutoff)
                    })
                {
                    push_violation(
                        &mut violations,
                        InvariantViolationV1::AdmissionCutoffMismatch,
                    );
                }
            }
            LifecycleStateV1::Starting | LifecycleStateV1::Running => {
                if self.admission_cutoff.is_some() {
                    push_violation(
                        &mut violations,
                        InvariantViolationV1::AdmissionCutoffMismatch,
                    );
                }
            }
            LifecycleStateV1::Failed(_) => {}
        }
        match (self.lifecycle, self.failure) {
            (LifecycleStateV1::Failed(disposition), Some(record))
                if record.disposition == disposition
                    && record.failure_model_id == self.config.failure_model_id => {}
            (LifecycleStateV1::Failed(_), _) | (_, Some(_)) => {
                push_violation(&mut violations, InvariantViolationV1::FailureRecordMismatch);
            }
            _ => {}
        }
        if self.lifecycle == LifecycleStateV1::Stopped && !self.is_quiescent() {
            push_violation(
                &mut violations,
                InvariantViolationV1::StoppedButNotQuiescent,
            );
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(InvariantReportV1 { violations })
        }
    }

    pub fn validate_transition_to(&self, next: &Self) -> Result<(), TransitionErrorV1> {
        self.validate_global_invariants()
            .map_err(TransitionErrorV1::CurrentStateInvalid)?;
        next.validate_global_invariants()
            .map_err(TransitionErrorV1::NextStateInvalid)?;
        if self.config != next.config {
            return Err(TransitionErrorV1::ImmutableConfigurationChanged);
        }
        if !self.lifecycle.can_transition_to(next.lifecycle) {
            return Err(TransitionErrorV1::IllegalLifecycleTransition);
        }
        for slot in &self.slots {
            let next_slot = next
                .slot(slot.slot_id)
                .ok_or(TransitionErrorV1::IllegalSlotTransition(slot.slot_id))?;
            if !slot.state.can_transition_to(next_slot.state)
                || !generation_transition_valid(slot, next_slot, self.config.generation_modulus)
                || !slot.aba.can_transition_to(
                    next_slot.aba,
                    next_slot.generation.counter().logical,
                    self.config.maximum_live_generation_span,
                )
            {
                return Err(TransitionErrorV1::IllegalSlotTransition(slot.slot_id));
            }
            if matches!(slot.state, QueueSlotStateV1::Empty { .. })
                && matches!(next_slot.state, QueueSlotStateV1::Reserved { .. })
                && (self.lifecycle != LifecycleStateV1::Running
                    || next.lifecycle != LifecycleStateV1::Running)
            {
                return Err(TransitionErrorV1::IllegalSlotTransition(slot.slot_id));
            }
            if matches!(slot.state, QueueSlotStateV1::Published { .. })
                && matches!(next_slot.state, QueueSlotStateV1::Acquired { .. })
                && !matches!(
                    self.lifecycle,
                    LifecycleStateV1::Running | LifecycleStateV1::Draining
                )
            {
                return Err(TransitionErrorV1::IllegalSlotTransition(slot.slot_id));
            }
        }
        for task in &self.tasks {
            let next_task = next
                .task(task.task_id)
                .ok_or(TransitionErrorV1::RemovedTask(task.task_id))?;
            if task.task_id != next_task.task_id
                || task.canonical_tag != next_task.canonical_tag
                || task.payload_identity != next_task.payload_identity
                || task.submission_sequence != next_task.submission_sequence
                || task.dependencies != next_task.dependencies
                || !task.state.can_transition_to(next_task.state)
            {
                return Err(TransitionErrorV1::IllegalTaskTransition(task.task_id));
            }
        }
        for task in &next.tasks {
            if self.task(task.task_id).is_none() && !matches!(task.state, TaskStateV1::Reserved(_))
            {
                return Err(TransitionErrorV1::AddedTaskInInvalidState(task.task_id));
            }
        }
        for lease in &self.leases {
            let next_lease = next
                .lease(lease.lease_id)
                .ok_or(TransitionErrorV1::RemovedLease(lease.lease_id))?;
            if !lease.state.can_transition_to(next_lease.state) {
                return Err(TransitionErrorV1::IllegalLeaseTransition(lease.lease_id));
            }
        }
        for lease in &next.leases {
            if self.lease(lease.lease_id).is_none()
                && !matches!(lease.state, LeaseStateV1::Issued(_))
            {
                return Err(TransitionErrorV1::AddedLeaseInInvalidState(lease.lease_id));
            }
        }
        for worker in &self.workers {
            let next_worker = next
                .worker(worker.worker_id)
                .ok_or(TransitionErrorV1::IllegalWorkerTransition(worker.worker_id))?;
            if !worker.state.can_transition_to(next_worker.state) {
                return Err(TransitionErrorV1::IllegalWorkerTransition(worker.worker_id));
            }
        }
        for dependency in &self.dependencies {
            let next_dependency = next.dependency(dependency.epoch).ok_or(
                TransitionErrorV1::IllegalDependencyTransition(dependency.epoch),
            )?;
            if !dependency.state.can_transition_to(next_dependency.state) {
                return Err(TransitionErrorV1::IllegalDependencyTransition(
                    dependency.epoch,
                ));
            }
        }
        for dependency in &next.dependencies {
            if self.dependency(dependency.epoch).is_none()
                && dependency.state != DependencyStateV1::Pending
            {
                return Err(TransitionErrorV1::IllegalDependencyTransition(
                    dependency.epoch,
                ));
            }
        }
        for phase in &self.phase_regions {
            let next_phase = next
                .phase_region(phase.region_id)
                .ok_or(TransitionErrorV1::IllegalPhaseTransition(phase.region_id))?;
            if !phase.state.can_transition_to(next_phase.state) {
                return Err(TransitionErrorV1::IllegalPhaseTransition(phase.region_id));
            }
        }
        if self.slots.len() != next.slots.len() {
            return Err(TransitionErrorV1::IllegalSlotTransition(SlotIdV1(u16::MAX)));
        }
        if self.workers.len() != next.workers.len() {
            return Err(TransitionErrorV1::IllegalWorkerTransition(WorkerIdV1(
                u16::MAX,
            )));
        }
        if self.phase_regions.len() != next.phase_regions.len() {
            return Err(TransitionErrorV1::IllegalPhaseTransition(RegionIdV1(
                u16::MAX,
            )));
        }
        for completion in &self.completion_records {
            let Some(next_completion) = next
                .completion_records
                .iter()
                .find(|candidate| candidate.record_id == completion.record_id)
            else {
                return Err(TransitionErrorV1::IllegalCompletionTransition(
                    completion.record_id,
                ));
            };
            if completion.record_id != next_completion.record_id
                || completion.task_id != next_completion.task_id
                || completion.slot != next_completion.slot
                || completion.outcome != next_completion.outcome
                || (completion.visible && !next_completion.visible)
            {
                return Err(TransitionErrorV1::IllegalCompletionTransition(
                    completion.record_id,
                ));
            }
        }
        Ok(())
    }

    pub fn is_quiescent(&self) -> bool {
        self.slots.iter().all(|slot| slot.state.is_empty())
            && self.tasks.iter().all(|task| task.state.is_terminal())
            && self.leases.iter().all(|lease| !lease.state.is_live())
            && self
                .workers
                .iter()
                .all(|worker| matches!(worker.state, WorkerStateV1::Exited | WorkerStateV1::Failed))
            && self.completion_records.iter().all(|record| record.visible)
    }

    fn valid_slot_brand(&self, key: SlotKeyV1) -> bool {
        key.run_id == self.config.run_id
            && key.service_epoch == self.config.service_epoch
            && key.queue_identity == self.config.queue_identity
            && key.slot_id.0 < self.config.queue_capacity
    }

    fn check_slot_task(
        &self,
        slot: &QueueSlotRecordV1,
        task_id: TaskIdV1,
        violations: &mut Vec<InvariantViolationV1>,
    ) {
        let Some(task) = self.task(task_id) else {
            push_violation(
                violations,
                InvariantViolationV1::SlotTaskMismatch(slot.slot_id),
            );
            return;
        };
        let expected_key = self.config.slot_key(slot.slot_id, slot.state.generation());
        if task.state.slot_key() != Some(expected_key)
            || !slot_task_phase_matches(slot.state, task.state)
        {
            push_violation(
                violations,
                InvariantViolationV1::SlotTaskMismatch(slot.slot_id),
            );
        }
        if let Some(lease_id) = slot.state.live_lease()
            && task.state.live_lease() != Some(lease_id)
        {
            push_violation(violations, InvariantViolationV1::LeaseMismatch(lease_id));
        }
    }

    fn slot(&self, id: SlotIdV1) -> Option<&QueueSlotRecordV1> {
        self.slots.iter().find(|record| record.slot_id == id)
    }

    fn task(&self, id: TaskIdV1) -> Option<&TaskRecordV1> {
        self.tasks.iter().find(|record| record.task_id == id)
    }

    fn lease(&self, id: LeaseIdV1) -> Option<&LeaseRecordV1> {
        self.leases.iter().find(|record| record.lease_id == id)
    }

    fn worker(&self, id: WorkerIdV1) -> Option<&WorkerRecordV1> {
        self.workers.iter().find(|record| record.worker_id == id)
    }

    fn dependency(&self, epoch: DependencyEpochV1) -> Option<&DependencyRecordV1> {
        self.dependencies
            .iter()
            .find(|record| record.epoch == epoch)
    }

    fn phase_region(&self, id: RegionIdV1) -> Option<&PhaseRegionRecordV1> {
        self.phase_regions
            .iter()
            .find(|record| record.region_id == id)
    }
}

fn generation_transition_valid(
    current: &QueueSlotRecordV1,
    next: &QueueSlotRecordV1,
    generation_modulus: u64,
) -> bool {
    let reclaim = matches!(current.state, QueueSlotStateV1::Reclaimable { .. })
        && matches!(next.state, QueueSlotStateV1::Empty { .. });
    current
        .generation
        .can_transition_to(next.generation, reclaim, generation_modulus)
}

fn slot_task_phase_matches(slot: QueueSlotStateV1, task: TaskStateV1) -> bool {
    matches!(
        (slot, task),
        (QueueSlotStateV1::Reserved { .. }, TaskStateV1::Reserved(_))
            | (
                QueueSlotStateV1::Initialized { .. },
                TaskStateV1::Initialized(_)
            )
            | (
                QueueSlotStateV1::Published { .. },
                TaskStateV1::Published(_)
            )
            | (
                QueueSlotStateV1::Acquired { .. },
                TaskStateV1::Acquired { .. }
            )
            | (
                QueueSlotStateV1::Executing { .. },
                TaskStateV1::Executing { .. }
            )
            | (
                QueueSlotStateV1::Executing { .. },
                TaskStateV1::CompletionPending { .. }
            )
            | (
                QueueSlotStateV1::Completed {
                    outcome: TaskOutcomeV1::Succeeded,
                    ..
                },
                TaskStateV1::Completed { .. }
            )
            | (
                QueueSlotStateV1::Completed {
                    outcome: TaskOutcomeV1::Cancelled,
                    ..
                },
                TaskStateV1::Cancelled { .. }
            )
            | (
                QueueSlotStateV1::Completed {
                    outcome: TaskOutcomeV1::Failed,
                    ..
                },
                TaskStateV1::Failed { .. }
            )
            | (
                QueueSlotStateV1::Reclaimable {
                    outcome: TaskOutcomeV1::Succeeded,
                    ..
                },
                TaskStateV1::Completed { .. }
            )
            | (
                QueueSlotStateV1::Reclaimable {
                    outcome: TaskOutcomeV1::Cancelled,
                    ..
                },
                TaskStateV1::Cancelled { .. }
            )
            | (
                QueueSlotStateV1::Reclaimable {
                    outcome: TaskOutcomeV1::Failed,
                    ..
                },
                TaskStateV1::Failed { .. }
            )
    )
}

fn check_unique_records(state: &ServiceStateV1, violations: &mut Vec<InvariantViolationV1>) {
    for (index, task) in state.tasks.iter().enumerate() {
        if state.tasks[..index]
            .iter()
            .any(|other| other.task_id == task.task_id)
        {
            push_violation(
                violations,
                InvariantViolationV1::DuplicateTask(task.task_id),
            );
        }
    }
    for (index, lease) in state.leases.iter().enumerate() {
        if state.leases[..index]
            .iter()
            .any(|other| other.lease_id == lease.lease_id)
        {
            push_violation(
                violations,
                InvariantViolationV1::DuplicateLease(lease.lease_id),
            );
        }
    }
    for (index, worker) in state.workers.iter().enumerate() {
        if state.workers[..index]
            .iter()
            .any(|other| other.worker_id == worker.worker_id)
        {
            push_violation(
                violations,
                InvariantViolationV1::DuplicateWorker(worker.worker_id),
            );
        }
    }
    for (index, dependency) in state.dependencies.iter().enumerate() {
        if state.dependencies[..index]
            .iter()
            .any(|other| other.epoch == dependency.epoch)
        {
            push_violation(
                violations,
                InvariantViolationV1::DuplicateDependency(dependency.epoch),
            );
        }
    }
    for (index, phase) in state.phase_regions.iter().enumerate() {
        if state.phase_regions[..index]
            .iter()
            .any(|other| other.region_id == phase.region_id)
        {
            push_violation(
                violations,
                InvariantViolationV1::DuplicatePhaseRegion(phase.region_id),
            );
        }
    }
    for (index, completion) in state.completion_records.iter().enumerate() {
        if state.completion_records[..index].iter().any(|other| {
            other.record_id == completion.record_id || other.task_id == completion.task_id
        }) {
            push_violation(
                violations,
                InvariantViolationV1::DuplicateCompletion(completion.record_id),
            );
        }
    }
}

fn check_bound(
    violations: &mut Vec<InvariantViolationV1>,
    field: &'static str,
    actual: usize,
    maximum: usize,
) {
    if actual > maximum {
        push_violation(violations, InvariantViolationV1::ModelBoundExceeded(field));
    }
}

fn push_violation(violations: &mut Vec<InvariantViolationV1>, violation: InvariantViolationV1) {
    if violations.len() < MAX_INVARIANT_VIOLATIONS_V1 {
        violations.push(violation);
    } else if violations.last() != Some(&InvariantViolationV1::ViolationLimitReached) {
        violations[MAX_INVARIANT_VIOLATIONS_V1 - 1] = InvariantViolationV1::ViolationLimitReached;
    }
}

//! Bounded, syscall-free runtime transition system.

use alloc::vec::Vec;

use crate::*;

pub const RUNTIME_STATE_SCHEMA_VERSION_V1: u16 = 1;
pub const MAX_DEVICES_V1: usize = 16;
pub const MAX_VMS_V1: usize = 64;
pub const MAX_ALLOCATIONS_V1: usize = 4_096;
pub const MAX_MAPPINGS_V1: usize = 4_096;
pub const MAX_LOADED_CODE_V1: usize = 256;
pub const MAX_QUEUES_V1: usize = 256;
pub const MAX_DISPATCHES_V1: usize = 32_768;
pub const MAX_COMPLETIONS_V1: usize = MAX_DISPATCHES_V1;
pub const MAX_DISPATCH_RESOURCES_V1: usize = 256;
pub const MAX_QUEUE_CAPACITY_V1: u32 = 16_384;
pub const AQL_PACKET_BYTES_V1: u64 = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceStateV1 {
    Ready,
    MayStillAccess,
    Quiescent,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VmStateV1 {
    Active,
    MayStillAccess,
    Quiescent,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceStateV1 {
    Live,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueStateV1 {
    Ready,
    MayStillAccess,
    Quiescent,
    Released,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DispatchStateV1 {
    Prepared,
    Published,
    Ambiguous,
    Completed,
    FailedQuiescent,
    AbortedBeforePublication,
}

impl DispatchStateV1 {
    pub const fn retains_resources(self) -> bool {
        matches!(self, Self::Prepared | Self::Published | Self::Ambiguous)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionStateV1 {
    Armed,
    Ambiguous,
    Observed,
    QuiescedFailure,
    CancelledBeforePublication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryAccessV1 {
    Read,
    ReadWrite,
    ReadExecute,
}

impl MemoryAccessV1 {
    pub const fn permits(self, required: Self) -> bool {
        matches!(
            (self, required),
            (Self::Read, Self::Read)
                | (Self::ReadWrite, Self::Read)
                | (Self::ReadWrite, Self::ReadWrite)
                | (Self::ReadExecute, Self::Read)
                | (Self::ReadExecute, Self::ReadExecute)
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DispatchResourceV1 {
    pub mapping: MappingKeyV1,
    pub required_access: MemoryAccessV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceRecordV1 {
    pub key: DeviceKeyV1,
    pub state: DeviceStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VmRecordV1 {
    pub key: VmKeyV1,
    pub state: VmStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AllocationRecordV1 {
    pub key: AllocationKeyV1,
    pub byte_len: u64,
    pub state: ResourceStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappingRecordV1 {
    pub key: MappingKeyV1,
    pub allocation_offset: u64,
    pub gpu_va: u64,
    pub byte_len: u64,
    pub access: MemoryAccessV1,
    pub state: ResourceStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadedCodeRecordV1 {
    pub key: LoadedCodeKeyV1,
    pub load_plan_id: CodeLoadPlanIdV1,
    pub artifact_id: RuntimeArtifactIdV1,
    pub executable_mapping: MappingKeyV1,
    pub entry_offset: u64,
    pub state: ResourceStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueRecordV1 {
    pub key: QueueKeyV1,
    pub plan_id: QueuePlanIdV1,
    pub ring_mapping: MappingKeyV1,
    pub capacity: u32,
    pub state: QueueStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DispatchRecordV1 {
    pub key: DispatchKeyV1,
    pub code: LoadedCodeKeyV1,
    pub completion: CompletionKeyV1,
    pub resources: Vec<DispatchResourceV1>,
    pub state: DispatchStateV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletionRecordV1 {
    pub key: CompletionKeyV1,
    pub state: CompletionStateV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordKindV1 {
    Device,
    Vm,
    Allocation,
    Mapping,
    LoadedCode,
    Queue,
    Dispatch,
    Completion,
    DispatchResource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordRefV1 {
    Device(DeviceKeyV1),
    Vm(VmKeyV1),
    Allocation(AllocationKeyV1),
    Mapping(MappingKeyV1),
    LoadedCode(LoadedCodeKeyV1),
    Queue(QueueKeyV1),
    Dispatch(DispatchKeyV1),
    Completion(CompletionKeyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InvariantViolationV1 {
    CapacityExceeded(RecordKindV1),
    Duplicate(RecordRefV1),
    MissingParent(RecordRefV1),
    BindingMismatch(RecordRefV1),
    InvalidRange(RecordRefV1),
    InvalidAccess(RecordRefV1),
    InvalidGeneration(DeviceKeyV1),
    InvalidState(RecordRefV1),
    AddressOverlap(MappingKeyV1, MappingKeyV1),
    NonCanonicalResources(DispatchKeyV1),
    CompletionMismatch(DispatchKeyV1),
    EarlyRelease(RecordRefV1),
    QueueOversubscribed(QueueKeyV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransitionErrorV1 {
    SourceInvariant(InvariantViolationV1),
    NextInvariant(InvariantViolationV1),
    CapacityExceeded { kind: RecordKindV1, maximum: usize },
    NotFound(RecordRefV1),
    AlreadyExists(RecordRefV1),
    IllegalState(RecordRefV1),
    BindingMismatch(RecordRefV1),
    GenerationNotMonotonic(DeviceKeyV1),
    InvalidRange(RecordRefV1),
    InvalidAccess(RecordRefV1),
    AddressConflict(MappingKeyV1),
    NonCanonicalResources(DispatchKeyV1),
    ResourceInUse(RecordRefV1),
    QueueFull(QueueKeyV1),
    CompletionMismatch(DispatchKeyV1),
    NotQuiescent(RecordRefV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeTransitionV1 {
    AddDevice {
        key: DeviceKeyV1,
    },
    BeginDeviceFailure {
        key: DeviceKeyV1,
    },
    EstablishDeviceQuiescence {
        key: DeviceKeyV1,
    },
    ReleaseDevice {
        key: DeviceKeyV1,
    },
    CreateVm {
        key: VmKeyV1,
    },
    BeginVmFailure {
        key: VmKeyV1,
    },
    EstablishVmQuiescence {
        key: VmKeyV1,
    },
    ReleaseVm {
        key: VmKeyV1,
    },
    Allocate {
        key: AllocationKeyV1,
        byte_len: u64,
    },
    ReleaseAllocation {
        key: AllocationKeyV1,
    },
    Map {
        key: MappingKeyV1,
        allocation_offset: u64,
        gpu_va: u64,
        byte_len: u64,
        access: MemoryAccessV1,
    },
    Unmap {
        key: MappingKeyV1,
    },
    LoadCode {
        key: LoadedCodeKeyV1,
        load_plan_id: CodeLoadPlanIdV1,
        artifact_id: RuntimeArtifactIdV1,
        executable_mapping: MappingKeyV1,
        entry_offset: u64,
    },
    UnloadCode {
        key: LoadedCodeKeyV1,
    },
    CreateQueue {
        key: QueueKeyV1,
        plan_id: QueuePlanIdV1,
        ring_mapping: MappingKeyV1,
        capacity: u32,
    },
    BeginQueueFailure {
        key: QueueKeyV1,
    },
    EstablishQueueQuiescence {
        key: QueueKeyV1,
    },
    ReleaseQueue {
        key: QueueKeyV1,
    },
    PrepareDispatch {
        key: DispatchKeyV1,
        code: LoadedCodeKeyV1,
        completion: CompletionKeyV1,
        resources: Vec<DispatchResourceV1>,
    },
    AbortPrepared {
        completion: CompletionKeyV1,
    },
    PublishDispatch {
        completion: CompletionKeyV1,
    },
    MarkDispatchAmbiguous {
        completion: CompletionKeyV1,
    },
    ObserveCompletion {
        completion: CompletionKeyV1,
    },
    SettleAfterQuiescence {
        completion: CompletionKeyV1,
    },
}

/// Complete finite carrier for the runtime lifecycle model.
///
/// `next` is immutable: a rejected transition cannot partially mutate the
/// source state. All collections are capped by public, versioned bounds.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RuntimeStateV1 {
    devices: Vec<DeviceRecordV1>,
    vms: Vec<VmRecordV1>,
    allocations: Vec<AllocationRecordV1>,
    pub(crate) mappings: Vec<MappingRecordV1>,
    loaded_code: Vec<LoadedCodeRecordV1>,
    queues: Vec<QueueRecordV1>,
    dispatches: Vec<DispatchRecordV1>,
    completions: Vec<CompletionRecordV1>,
}

impl RuntimeStateV1 {
    pub const fn new() -> Self {
        Self {
            devices: Vec::new(),
            vms: Vec::new(),
            allocations: Vec::new(),
            mappings: Vec::new(),
            loaded_code: Vec::new(),
            queues: Vec::new(),
            dispatches: Vec::new(),
            completions: Vec::new(),
        }
    }

    pub fn devices(&self) -> &[DeviceRecordV1] {
        &self.devices
    }
    pub fn vms(&self) -> &[VmRecordV1] {
        &self.vms
    }
    pub fn allocations(&self) -> &[AllocationRecordV1] {
        &self.allocations
    }
    pub fn mappings(&self) -> &[MappingRecordV1] {
        &self.mappings
    }
    pub fn loaded_code(&self) -> &[LoadedCodeRecordV1] {
        &self.loaded_code
    }
    pub fn queues(&self) -> &[QueueRecordV1] {
        &self.queues
    }
    pub fn dispatches(&self) -> &[DispatchRecordV1] {
        &self.dispatches
    }
    pub fn completions(&self) -> &[CompletionRecordV1] {
        &self.completions
    }

    pub fn next(&self, transition: RuntimeTransitionV1) -> Result<Self, TransitionErrorV1> {
        self.validate_global_invariants()
            .map_err(TransitionErrorV1::SourceInvariant)?;
        let mut next = self.clone();
        next.apply(transition)?;
        next.validate_global_invariants()
            .map_err(TransitionErrorV1::NextInvariant)?;
        Ok(next)
    }

    pub fn validate_global_invariants(&self) -> Result<(), InvariantViolationV1> {
        self.validate_capacities()?;
        self.validate_devices()?;
        self.validate_vms_and_memory()?;
        self.validate_code_and_queues()?;
        self.validate_dispatches_and_completions()?;
        Ok(())
    }

    fn apply(&mut self, transition: RuntimeTransitionV1) -> Result<(), TransitionErrorV1> {
        match transition {
            RuntimeTransitionV1::AddDevice { key } => self.add_device(key),
            RuntimeTransitionV1::BeginDeviceFailure { key } => {
                let record = self.device_mut(key)?;
                require_state(
                    record.state == DeviceStateV1::Ready,
                    RecordRefV1::Device(key),
                )?;
                record.state = DeviceStateV1::MayStillAccess;
                Ok(())
            }
            RuntimeTransitionV1::EstablishDeviceQuiescence { key } => {
                let record = self.device_mut(key)?;
                require_state(
                    record.state == DeviceStateV1::MayStillAccess,
                    RecordRefV1::Device(key),
                )?;
                record.state = DeviceStateV1::Quiescent;
                Ok(())
            }
            RuntimeTransitionV1::ReleaseDevice { key } => self.release_device(key),
            RuntimeTransitionV1::CreateVm { key } => self.create_vm(key),
            RuntimeTransitionV1::BeginVmFailure { key } => {
                let record = self.vm_mut(key)?;
                require_state(record.state == VmStateV1::Active, RecordRefV1::Vm(key))?;
                record.state = VmStateV1::MayStillAccess;
                Ok(())
            }
            RuntimeTransitionV1::EstablishVmQuiescence { key } => {
                let record = self.vm_mut(key)?;
                require_state(
                    record.state == VmStateV1::MayStillAccess,
                    RecordRefV1::Vm(key),
                )?;
                record.state = VmStateV1::Quiescent;
                Ok(())
            }
            RuntimeTransitionV1::ReleaseVm { key } => self.release_vm(key),
            RuntimeTransitionV1::Allocate { key, byte_len } => self.allocate(key, byte_len),
            RuntimeTransitionV1::ReleaseAllocation { key } => self.release_allocation(key),
            RuntimeTransitionV1::Map {
                key,
                allocation_offset,
                gpu_va,
                byte_len,
                access,
            } => self.map(key, allocation_offset, gpu_va, byte_len, access),
            RuntimeTransitionV1::Unmap { key } => self.unmap(key),
            RuntimeTransitionV1::LoadCode {
                key,
                load_plan_id,
                artifact_id,
                executable_mapping,
                entry_offset,
            } => self.load_code(
                key,
                load_plan_id,
                artifact_id,
                executable_mapping,
                entry_offset,
            ),
            RuntimeTransitionV1::UnloadCode { key } => self.unload_code(key),
            RuntimeTransitionV1::CreateQueue {
                key,
                plan_id,
                ring_mapping,
                capacity,
            } => self.create_queue(key, plan_id, ring_mapping, capacity),
            RuntimeTransitionV1::BeginQueueFailure { key } => {
                let record = self.queue_mut(key)?;
                require_state(record.state == QueueStateV1::Ready, RecordRefV1::Queue(key))?;
                record.state = QueueStateV1::MayStillAccess;
                Ok(())
            }
            RuntimeTransitionV1::EstablishQueueQuiescence { key } => {
                let record = self.queue_mut(key)?;
                require_state(
                    record.state == QueueStateV1::MayStillAccess,
                    RecordRefV1::Queue(key),
                )?;
                record.state = QueueStateV1::Quiescent;
                Ok(())
            }
            RuntimeTransitionV1::ReleaseQueue { key } => self.release_queue(key),
            RuntimeTransitionV1::PrepareDispatch {
                key,
                code,
                completion,
                resources,
            } => self.prepare_dispatch(key, code, completion, resources),
            RuntimeTransitionV1::AbortPrepared { completion } => self.transition_dispatch(
                completion,
                DispatchStateV1::Prepared,
                DispatchStateV1::AbortedBeforePublication,
                CompletionStateV1::Armed,
                CompletionStateV1::CancelledBeforePublication,
            ),
            RuntimeTransitionV1::PublishDispatch { completion } => {
                self.publish_dispatch(completion)
            }
            RuntimeTransitionV1::MarkDispatchAmbiguous { completion } => self.transition_dispatch(
                completion,
                DispatchStateV1::Published,
                DispatchStateV1::Ambiguous,
                CompletionStateV1::Armed,
                CompletionStateV1::Ambiguous,
            ),
            RuntimeTransitionV1::ObserveCompletion { completion } => {
                self.observe_completion(completion)
            }
            RuntimeTransitionV1::SettleAfterQuiescence { completion } => {
                self.settle_after_quiescence(completion)
            }
        }
    }

    fn add_device(&mut self, key: DeviceKeyV1) -> Result<(), TransitionErrorV1> {
        ensure_room(self.devices.len(), MAX_DEVICES_V1, RecordKindV1::Device)?;
        if self.devices.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Device(key)));
        }
        for record in self
            .devices
            .iter()
            .filter(|record| record.key.physical == key.physical)
        {
            if record.state != DeviceStateV1::Released {
                return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Device(
                    record.key,
                )));
            }
            if record.key.generation >= key.generation {
                return Err(TransitionErrorV1::GenerationNotMonotonic(key));
            }
        }
        self.devices.push(DeviceRecordV1 {
            key,
            state: DeviceStateV1::Ready,
        });
        Ok(())
    }

    fn release_device(&mut self, key: DeviceKeyV1) -> Result<(), TransitionErrorV1> {
        let state = self.device(key)?.state;
        require_state(
            matches!(state, DeviceStateV1::Ready | DeviceStateV1::Quiescent),
            RecordRefV1::Device(key),
        )?;
        if self
            .vms
            .iter()
            .any(|record| record.key.device == key && record.state != VmStateV1::Released)
        {
            return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Device(key)));
        }
        self.device_mut(key)?.state = DeviceStateV1::Released;
        Ok(())
    }

    fn create_vm(&mut self, key: VmKeyV1) -> Result<(), TransitionErrorV1> {
        ensure_room(self.vms.len(), MAX_VMS_V1, RecordKindV1::Vm)?;
        if self.vms.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Vm(key)));
        }
        require_state(
            self.device(key.device)?.state == DeviceStateV1::Ready,
            RecordRefV1::Device(key.device),
        )?;
        self.vms.push(VmRecordV1 {
            key,
            state: VmStateV1::Active,
        });
        Ok(())
    }

    fn release_vm(&mut self, key: VmKeyV1) -> Result<(), TransitionErrorV1> {
        let state = self.vm(key)?.state;
        require_state(
            matches!(state, VmStateV1::Active | VmStateV1::Quiescent),
            RecordRefV1::Vm(key),
        )?;
        let resources_live = self
            .allocations
            .iter()
            .any(|r| r.key.vm == key && r.state != ResourceStateV1::Released)
            || self
                .loaded_code
                .iter()
                .any(|r| r.key.vm == key && r.state != ResourceStateV1::Released)
            || self
                .queues
                .iter()
                .any(|r| r.key.vm == key && r.state != QueueStateV1::Released);
        if resources_live {
            return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Vm(key)));
        }
        self.vm_mut(key)?.state = VmStateV1::Released;
        Ok(())
    }

    fn allocate(&mut self, key: AllocationKeyV1, byte_len: u64) -> Result<(), TransitionErrorV1> {
        ensure_room(
            self.allocations.len(),
            MAX_ALLOCATIONS_V1,
            RecordKindV1::Allocation,
        )?;
        if self.allocations.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Allocation(
                key,
            )));
        }
        if byte_len == 0 {
            return Err(TransitionErrorV1::InvalidRange(RecordRefV1::Allocation(
                key,
            )));
        }
        self.require_vm_active(key.vm)?;
        self.allocations.push(AllocationRecordV1 {
            key,
            byte_len,
            state: ResourceStateV1::Live,
        });
        Ok(())
    }

    fn release_allocation(&mut self, key: AllocationKeyV1) -> Result<(), TransitionErrorV1> {
        require_state(
            self.allocation(key)?.state == ResourceStateV1::Live,
            RecordRefV1::Allocation(key),
        )?;
        if self
            .mappings
            .iter()
            .any(|r| r.key.allocation == key && r.state == ResourceStateV1::Live)
        {
            return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Allocation(
                key,
            )));
        }
        self.allocation_mut(key)?.state = ResourceStateV1::Released;
        Ok(())
    }

    fn map(
        &mut self,
        key: MappingKeyV1,
        allocation_offset: u64,
        gpu_va: u64,
        byte_len: u64,
        access: MemoryAccessV1,
    ) -> Result<(), TransitionErrorV1> {
        ensure_room(self.mappings.len(), MAX_MAPPINGS_V1, RecordKindV1::Mapping)?;
        if self.mappings.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Mapping(key)));
        }
        self.require_vm_active(key.allocation.vm)?;
        let allocation = self.allocation(key.allocation)?;
        require_state(
            allocation.state == ResourceStateV1::Live,
            RecordRefV1::Allocation(key.allocation),
        )?;
        if byte_len == 0
            || allocation_offset
                .checked_add(byte_len)
                .is_none_or(|end| end > allocation.byte_len)
            || gpu_va.checked_add(byte_len).is_none()
        {
            return Err(TransitionErrorV1::InvalidRange(RecordRefV1::Mapping(key)));
        }
        if self
            .mappings
            .iter()
            .filter(|r| {
                r.state == ResourceStateV1::Live && r.key.allocation.vm == key.allocation.vm
            })
            .any(|r| ranges_overlap(gpu_va, byte_len, r.gpu_va, r.byte_len))
        {
            return Err(TransitionErrorV1::AddressConflict(key));
        }
        self.mappings.push(MappingRecordV1 {
            key,
            allocation_offset,
            gpu_va,
            byte_len,
            access,
            state: ResourceStateV1::Live,
        });
        Ok(())
    }

    fn unmap(&mut self, key: MappingKeyV1) -> Result<(), TransitionErrorV1> {
        require_state(
            self.mapping(key)?.state == ResourceStateV1::Live,
            RecordRefV1::Mapping(key),
        )?;
        if self
            .loaded_code
            .iter()
            .any(|r| r.executable_mapping == key && r.state == ResourceStateV1::Live)
            || self
                .queues
                .iter()
                .any(|r| r.ring_mapping == key && r.state != QueueStateV1::Released)
            || self.dispatches.iter().any(|r| {
                r.state.retains_resources()
                    && r.resources.iter().any(|resource| resource.mapping == key)
            })
        {
            return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Mapping(key)));
        }
        self.mapping_mut(key)?.state = ResourceStateV1::Released;
        Ok(())
    }

    fn load_code(
        &mut self,
        key: LoadedCodeKeyV1,
        load_plan_id: CodeLoadPlanIdV1,
        artifact_id: RuntimeArtifactIdV1,
        executable_mapping: MappingKeyV1,
        entry_offset: u64,
    ) -> Result<(), TransitionErrorV1> {
        ensure_room(
            self.loaded_code.len(),
            MAX_LOADED_CODE_V1,
            RecordKindV1::LoadedCode,
        )?;
        if self.loaded_code.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::LoadedCode(
                key,
            )));
        }
        self.require_vm_active(key.vm)?;
        let mapping = self.mapping(executable_mapping)?;
        if executable_mapping.allocation.vm != key.vm {
            return Err(TransitionErrorV1::BindingMismatch(RecordRefV1::LoadedCode(
                key,
            )));
        }
        if mapping.state != ResourceStateV1::Live || mapping.access != MemoryAccessV1::ReadExecute {
            return Err(TransitionErrorV1::InvalidAccess(RecordRefV1::Mapping(
                executable_mapping,
            )));
        }
        if entry_offset >= mapping.byte_len {
            return Err(TransitionErrorV1::InvalidRange(RecordRefV1::LoadedCode(
                key,
            )));
        }
        self.loaded_code.push(LoadedCodeRecordV1 {
            key,
            load_plan_id,
            artifact_id,
            executable_mapping,
            entry_offset,
            state: ResourceStateV1::Live,
        });
        Ok(())
    }

    fn unload_code(&mut self, key: LoadedCodeKeyV1) -> Result<(), TransitionErrorV1> {
        require_state(
            self.code(key)?.state == ResourceStateV1::Live,
            RecordRefV1::LoadedCode(key),
        )?;
        if self
            .dispatches
            .iter()
            .any(|r| r.code == key && r.state.retains_resources())
        {
            return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::LoadedCode(
                key,
            )));
        }
        self.code_mut(key)?.state = ResourceStateV1::Released;
        Ok(())
    }

    fn create_queue(
        &mut self,
        key: QueueKeyV1,
        plan_id: QueuePlanIdV1,
        ring_mapping: MappingKeyV1,
        capacity: u32,
    ) -> Result<(), TransitionErrorV1> {
        ensure_room(self.queues.len(), MAX_QUEUES_V1, RecordKindV1::Queue)?;
        if self.queues.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Queue(key)));
        }
        self.require_vm_active(key.vm)?;
        if capacity == 0 || capacity > MAX_QUEUE_CAPACITY_V1 || !capacity.is_power_of_two() {
            return Err(TransitionErrorV1::InvalidRange(RecordRefV1::Queue(key)));
        }
        let mapping = self.mapping(ring_mapping)?;
        let required_bytes = u64::from(capacity)
            .checked_mul(AQL_PACKET_BYTES_V1)
            .ok_or(TransitionErrorV1::InvalidRange(RecordRefV1::Queue(key)))?;
        if ring_mapping.allocation.vm != key.vm {
            return Err(TransitionErrorV1::BindingMismatch(RecordRefV1::Queue(key)));
        }
        if mapping.state != ResourceStateV1::Live
            || mapping.access != MemoryAccessV1::ReadWrite
            || mapping.byte_len < required_bytes
        {
            return Err(TransitionErrorV1::InvalidAccess(RecordRefV1::Mapping(
                ring_mapping,
            )));
        }
        for record in self
            .queues
            .iter()
            .filter(|record| record.key.vm == key.vm && record.key.id == key.id)
        {
            if record.state != QueueStateV1::Released {
                return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Queue(
                    record.key,
                )));
            }
            if record.key.generation >= key.generation {
                return Err(TransitionErrorV1::IllegalState(RecordRefV1::Queue(key)));
            }
        }
        self.queues.push(QueueRecordV1 {
            key,
            plan_id,
            ring_mapping,
            capacity,
            state: QueueStateV1::Ready,
        });
        Ok(())
    }

    fn release_queue(&mut self, key: QueueKeyV1) -> Result<(), TransitionErrorV1> {
        let state = self.queue(key)?.state;
        if state == QueueStateV1::MayStillAccess && !self.queue_is_effectively_quiescent(key)? {
            return Err(TransitionErrorV1::NotQuiescent(RecordRefV1::Queue(key)));
        }
        require_state(state != QueueStateV1::Released, RecordRefV1::Queue(key))?;
        if self
            .dispatches
            .iter()
            .any(|r| r.key.queue == key && r.state.retains_resources())
        {
            return Err(TransitionErrorV1::ResourceInUse(RecordRefV1::Queue(key)));
        }
        self.queue_mut(key)?.state = QueueStateV1::Released;
        Ok(())
    }

    fn prepare_dispatch(
        &mut self,
        key: DispatchKeyV1,
        code: LoadedCodeKeyV1,
        completion: CompletionKeyV1,
        resources: Vec<DispatchResourceV1>,
    ) -> Result<(), TransitionErrorV1> {
        ensure_room(
            self.dispatches.len(),
            MAX_DISPATCHES_V1,
            RecordKindV1::Dispatch,
        )?;
        ensure_room(
            self.completions.len(),
            MAX_COMPLETIONS_V1,
            RecordKindV1::Completion,
        )?;
        if self.dispatches.iter().any(|record| record.key == key) {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Dispatch(key)));
        }
        if self
            .completions
            .iter()
            .any(|record| record.key == completion)
        {
            return Err(TransitionErrorV1::AlreadyExists(RecordRefV1::Completion(
                completion,
            )));
        }
        if completion.dispatch != key {
            return Err(TransitionErrorV1::CompletionMismatch(key));
        }
        if resources.len() > MAX_DISPATCH_RESOURCES_V1 {
            return Err(TransitionErrorV1::CapacityExceeded {
                kind: RecordKindV1::DispatchResource,
                maximum: MAX_DISPATCH_RESOURCES_V1,
            });
        }
        if resources
            .windows(2)
            .any(|pair| pair[0].mapping >= pair[1].mapping)
        {
            return Err(TransitionErrorV1::NonCanonicalResources(key));
        }
        self.require_vm_active(key.queue.vm)?;
        let queue = self.queue(key.queue)?;
        require_state(
            queue.state == QueueStateV1::Ready,
            RecordRefV1::Queue(key.queue),
        )?;
        let active = self
            .dispatches
            .iter()
            .filter(|record| record.key.queue == key.queue && record.state.retains_resources())
            .count();
        if active >= queue.capacity as usize {
            return Err(TransitionErrorV1::QueueFull(key.queue));
        }
        let loaded = self.code(code)?;
        if code.vm != key.queue.vm {
            return Err(TransitionErrorV1::BindingMismatch(RecordRefV1::Dispatch(
                key,
            )));
        }
        require_state(
            loaded.state == ResourceStateV1::Live,
            RecordRefV1::LoadedCode(code),
        )?;
        for resource in &resources {
            let mapping = self.mapping(resource.mapping)?;
            if resource.mapping.allocation.vm != key.queue.vm {
                return Err(TransitionErrorV1::BindingMismatch(RecordRefV1::Mapping(
                    resource.mapping,
                )));
            }
            if mapping.state != ResourceStateV1::Live
                || !mapping.access.permits(resource.required_access)
            {
                return Err(TransitionErrorV1::InvalidAccess(RecordRefV1::Mapping(
                    resource.mapping,
                )));
            }
        }
        self.dispatches.push(DispatchRecordV1 {
            key,
            code,
            completion,
            resources,
            state: DispatchStateV1::Prepared,
        });
        self.completions.push(CompletionRecordV1 {
            key: completion,
            state: CompletionStateV1::Armed,
        });
        Ok(())
    }

    fn publish_dispatch(&mut self, completion: CompletionKeyV1) -> Result<(), TransitionErrorV1> {
        let dispatch = self.dispatch(completion.dispatch)?;
        if dispatch.completion != completion {
            return Err(TransitionErrorV1::CompletionMismatch(completion.dispatch));
        }
        require_state(
            dispatch.state == DispatchStateV1::Prepared,
            RecordRefV1::Dispatch(completion.dispatch),
        )?;
        require_state(
            self.completion(completion)?.state == CompletionStateV1::Armed,
            RecordRefV1::Completion(completion),
        )?;
        self.require_vm_active(completion.dispatch.queue.vm)?;
        require_state(
            self.queue(completion.dispatch.queue)?.state == QueueStateV1::Ready,
            RecordRefV1::Queue(completion.dispatch.queue),
        )?;
        self.dispatch_mut(completion.dispatch)?.state = DispatchStateV1::Published;
        Ok(())
    }

    fn observe_completion(&mut self, completion: CompletionKeyV1) -> Result<(), TransitionErrorV1> {
        let dispatch = self.dispatch(completion.dispatch)?;
        if dispatch.completion != completion {
            return Err(TransitionErrorV1::CompletionMismatch(completion.dispatch));
        }
        require_state(
            dispatch.state == DispatchStateV1::Published,
            RecordRefV1::Dispatch(completion.dispatch),
        )?;
        require_state(
            self.completion(completion)?.state == CompletionStateV1::Armed,
            RecordRefV1::Completion(completion),
        )?;
        self.require_vm_active(completion.dispatch.queue.vm)?;
        require_state(
            self.queue(completion.dispatch.queue)?.state == QueueStateV1::Ready,
            RecordRefV1::Queue(completion.dispatch.queue),
        )?;
        self.dispatch_mut(completion.dispatch)?.state = DispatchStateV1::Completed;
        self.completion_mut(completion)?.state = CompletionStateV1::Observed;
        Ok(())
    }

    fn settle_after_quiescence(
        &mut self,
        completion: CompletionKeyV1,
    ) -> Result<(), TransitionErrorV1> {
        let dispatch = self.dispatch(completion.dispatch)?;
        if dispatch.completion != completion {
            return Err(TransitionErrorV1::CompletionMismatch(completion.dispatch));
        }
        require_state(
            dispatch.state == DispatchStateV1::Ambiguous,
            RecordRefV1::Dispatch(completion.dispatch),
        )?;
        require_state(
            self.completion(completion)?.state == CompletionStateV1::Ambiguous,
            RecordRefV1::Completion(completion),
        )?;
        if !self.queue_is_effectively_quiescent(completion.dispatch.queue)? {
            return Err(TransitionErrorV1::NotQuiescent(RecordRefV1::Queue(
                completion.dispatch.queue,
            )));
        }
        self.dispatch_mut(completion.dispatch)?.state = DispatchStateV1::FailedQuiescent;
        self.completion_mut(completion)?.state = CompletionStateV1::QuiescedFailure;
        Ok(())
    }

    fn transition_dispatch(
        &mut self,
        completion: CompletionKeyV1,
        from_dispatch: DispatchStateV1,
        to_dispatch: DispatchStateV1,
        from_completion: CompletionStateV1,
        to_completion: CompletionStateV1,
    ) -> Result<(), TransitionErrorV1> {
        let dispatch = self.dispatch(completion.dispatch)?;
        if dispatch.completion != completion {
            return Err(TransitionErrorV1::CompletionMismatch(completion.dispatch));
        }
        require_state(
            dispatch.state == from_dispatch,
            RecordRefV1::Dispatch(completion.dispatch),
        )?;
        require_state(
            self.completion(completion)?.state == from_completion,
            RecordRefV1::Completion(completion),
        )?;
        self.dispatch_mut(completion.dispatch)?.state = to_dispatch;
        self.completion_mut(completion)?.state = to_completion;
        Ok(())
    }

    fn require_vm_active(&self, key: VmKeyV1) -> Result<(), TransitionErrorV1> {
        require_state(
            self.vm(key)?.state == VmStateV1::Active,
            RecordRefV1::Vm(key),
        )?;
        require_state(
            self.device(key.device)?.state == DeviceStateV1::Ready,
            RecordRefV1::Device(key.device),
        )
    }

    fn queue_is_effectively_quiescent(&self, key: QueueKeyV1) -> Result<bool, TransitionErrorV1> {
        Ok(self.queue(key)?.state == QueueStateV1::Quiescent
            || self.vm(key.vm)?.state == VmStateV1::Quiescent
            || self.device(key.vm.device)?.state == DeviceStateV1::Quiescent)
    }

    fn validate_capacities(&self) -> Result<(), InvariantViolationV1> {
        for (actual, maximum, kind) in [
            (self.devices.len(), MAX_DEVICES_V1, RecordKindV1::Device),
            (self.vms.len(), MAX_VMS_V1, RecordKindV1::Vm),
            (
                self.allocations.len(),
                MAX_ALLOCATIONS_V1,
                RecordKindV1::Allocation,
            ),
            (self.mappings.len(), MAX_MAPPINGS_V1, RecordKindV1::Mapping),
            (
                self.loaded_code.len(),
                MAX_LOADED_CODE_V1,
                RecordKindV1::LoadedCode,
            ),
            (self.queues.len(), MAX_QUEUES_V1, RecordKindV1::Queue),
            (
                self.dispatches.len(),
                MAX_DISPATCHES_V1,
                RecordKindV1::Dispatch,
            ),
            (
                self.completions.len(),
                MAX_COMPLETIONS_V1,
                RecordKindV1::Completion,
            ),
        ] {
            if actual > maximum {
                return Err(InvariantViolationV1::CapacityExceeded(kind));
            }
        }
        Ok(())
    }

    fn validate_devices(&self) -> Result<(), InvariantViolationV1> {
        for (index, device) in self.devices.iter().enumerate() {
            if self.devices[..index]
                .iter()
                .any(|other| other.key == device.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Device(
                    device.key,
                )));
            }
            for other in self.devices.iter().filter(|other| {
                other.key.physical == device.key.physical
                    && other.key.generation < device.key.generation
            }) {
                if other.state != DeviceStateV1::Released {
                    return Err(InvariantViolationV1::InvalidGeneration(device.key));
                }
            }
        }
        Ok(())
    }

    fn validate_vms_and_memory(&self) -> Result<(), InvariantViolationV1> {
        for (index, vm) in self.vms.iter().enumerate() {
            if self.vms[..index].iter().any(|other| other.key == vm.key) {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Vm(vm.key)));
            }
            let device = self
                .devices
                .iter()
                .find(|r| r.key == vm.key.device)
                .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Vm(vm.key)))?;
            if device.state == DeviceStateV1::Released && vm.state != VmStateV1::Released {
                return Err(InvariantViolationV1::EarlyRelease(RecordRefV1::Device(
                    device.key,
                )));
            }
        }
        for (index, allocation) in self.allocations.iter().enumerate() {
            if self.allocations[..index]
                .iter()
                .any(|other| other.key == allocation.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Allocation(
                    allocation.key,
                )));
            }
            let vm = self.vms.iter().find(|r| r.key == allocation.key.vm).ok_or(
                InvariantViolationV1::MissingParent(RecordRefV1::Allocation(allocation.key)),
            )?;
            if allocation.byte_len == 0 {
                return Err(InvariantViolationV1::InvalidRange(RecordRefV1::Allocation(
                    allocation.key,
                )));
            }
            if vm.state == VmStateV1::Released && allocation.state != ResourceStateV1::Released {
                return Err(InvariantViolationV1::EarlyRelease(RecordRefV1::Vm(vm.key)));
            }
        }
        for (index, mapping) in self.mappings.iter().enumerate() {
            if self.mappings[..index]
                .iter()
                .any(|other| other.key == mapping.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Mapping(
                    mapping.key,
                )));
            }
            let allocation = self
                .allocations
                .iter()
                .find(|r| r.key == mapping.key.allocation)
                .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Mapping(
                    mapping.key,
                )))?;
            if mapping.byte_len == 0
                || mapping
                    .allocation_offset
                    .checked_add(mapping.byte_len)
                    .is_none_or(|end| end > allocation.byte_len)
                || mapping.gpu_va.checked_add(mapping.byte_len).is_none()
            {
                return Err(InvariantViolationV1::InvalidRange(RecordRefV1::Mapping(
                    mapping.key,
                )));
            }
            if allocation.state == ResourceStateV1::Released
                && mapping.state != ResourceStateV1::Released
            {
                return Err(InvariantViolationV1::EarlyRelease(RecordRefV1::Allocation(
                    allocation.key,
                )));
            }
            if mapping.state == ResourceStateV1::Live {
                for other in self.mappings[..index].iter().filter(|other| {
                    other.state == ResourceStateV1::Live
                        && other.key.allocation.vm == mapping.key.allocation.vm
                }) {
                    if ranges_overlap(
                        mapping.gpu_va,
                        mapping.byte_len,
                        other.gpu_va,
                        other.byte_len,
                    ) {
                        return Err(InvariantViolationV1::AddressOverlap(other.key, mapping.key));
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_code_and_queues(&self) -> Result<(), InvariantViolationV1> {
        for (index, code) in self.loaded_code.iter().enumerate() {
            if self.loaded_code[..index]
                .iter()
                .any(|other| other.key == code.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::LoadedCode(
                    code.key,
                )));
            }
            let mapping = self
                .mappings
                .iter()
                .find(|r| r.key == code.executable_mapping)
                .ok_or(InvariantViolationV1::MissingParent(
                    RecordRefV1::LoadedCode(code.key),
                ))?;
            if code.executable_mapping.allocation.vm != code.key.vm {
                return Err(InvariantViolationV1::BindingMismatch(
                    RecordRefV1::LoadedCode(code.key),
                ));
            }
            if code.entry_offset >= mapping.byte_len {
                return Err(InvariantViolationV1::InvalidRange(RecordRefV1::LoadedCode(
                    code.key,
                )));
            }
            if code.state == ResourceStateV1::Live
                && (mapping.state != ResourceStateV1::Live
                    || mapping.access != MemoryAccessV1::ReadExecute)
            {
                return Err(InvariantViolationV1::InvalidAccess(
                    RecordRefV1::LoadedCode(code.key),
                ));
            }
        }
        for (index, queue) in self.queues.iter().enumerate() {
            if self.queues[..index]
                .iter()
                .any(|other| other.key == queue.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Queue(
                    queue.key,
                )));
            }
            let mapping = self
                .mappings
                .iter()
                .find(|r| r.key == queue.ring_mapping)
                .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Queue(
                    queue.key,
                )))?;
            let required = u64::from(queue.capacity)
                .checked_mul(AQL_PACKET_BYTES_V1)
                .ok_or(InvariantViolationV1::InvalidRange(RecordRefV1::Queue(
                    queue.key,
                )))?;
            if queue.ring_mapping.allocation.vm != queue.key.vm {
                return Err(InvariantViolationV1::BindingMismatch(RecordRefV1::Queue(
                    queue.key,
                )));
            }
            if queue.capacity == 0
                || queue.capacity > MAX_QUEUE_CAPACITY_V1
                || !queue.capacity.is_power_of_two()
                || mapping.byte_len < required
            {
                return Err(InvariantViolationV1::InvalidRange(RecordRefV1::Queue(
                    queue.key,
                )));
            }
            if queue.state != QueueStateV1::Released
                && (mapping.state != ResourceStateV1::Live
                    || mapping.access != MemoryAccessV1::ReadWrite)
            {
                return Err(InvariantViolationV1::InvalidAccess(RecordRefV1::Queue(
                    queue.key,
                )));
            }
        }
        Ok(())
    }

    fn validate_dispatches_and_completions(&self) -> Result<(), InvariantViolationV1> {
        for (index, dispatch) in self.dispatches.iter().enumerate() {
            if self.dispatches[..index]
                .iter()
                .any(|other| other.key == dispatch.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Dispatch(
                    dispatch.key,
                )));
            }
            let queue = self
                .queues
                .iter()
                .find(|r| r.key == dispatch.key.queue)
                .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Dispatch(
                    dispatch.key,
                )))?;
            let code = self
                .loaded_code
                .iter()
                .find(|r| r.key == dispatch.code)
                .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Dispatch(
                    dispatch.key,
                )))?;
            let completion = self
                .completions
                .iter()
                .find(|r| r.key == dispatch.completion)
                .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Dispatch(
                    dispatch.key,
                )))?;
            if dispatch.code.vm != dispatch.key.queue.vm {
                return Err(InvariantViolationV1::BindingMismatch(
                    RecordRefV1::Dispatch(dispatch.key),
                ));
            }
            if dispatch.completion.dispatch != dispatch.key
                || !completion_matches(dispatch.state, completion.state)
            {
                return Err(InvariantViolationV1::CompletionMismatch(dispatch.key));
            }
            if dispatch.resources.len() > MAX_DISPATCH_RESOURCES_V1
                || dispatch
                    .resources
                    .windows(2)
                    .any(|pair| pair[0].mapping >= pair[1].mapping)
            {
                return Err(InvariantViolationV1::NonCanonicalResources(dispatch.key));
            }
            if dispatch.state.retains_resources() {
                if queue.state == QueueStateV1::Released || code.state != ResourceStateV1::Live {
                    return Err(InvariantViolationV1::EarlyRelease(RecordRefV1::Dispatch(
                        dispatch.key,
                    )));
                }
                for resource in &dispatch.resources {
                    let mapping = self
                        .mappings
                        .iter()
                        .find(|r| r.key == resource.mapping)
                        .ok_or(InvariantViolationV1::MissingParent(RecordRefV1::Mapping(
                            resource.mapping,
                        )))?;
                    if resource.mapping.allocation.vm != dispatch.key.queue.vm {
                        return Err(InvariantViolationV1::BindingMismatch(RecordRefV1::Mapping(
                            resource.mapping,
                        )));
                    }
                    if mapping.state != ResourceStateV1::Live
                        || !mapping.access.permits(resource.required_access)
                    {
                        return Err(InvariantViolationV1::EarlyRelease(RecordRefV1::Mapping(
                            resource.mapping,
                        )));
                    }
                }
            }
        }
        for (index, completion) in self.completions.iter().enumerate() {
            if self.completions[..index]
                .iter()
                .any(|other| other.key == completion.key)
            {
                return Err(InvariantViolationV1::Duplicate(RecordRefV1::Completion(
                    completion.key,
                )));
            }
            if !self
                .dispatches
                .iter()
                .any(|r| r.key == completion.key.dispatch && r.completion == completion.key)
            {
                return Err(InvariantViolationV1::MissingParent(
                    RecordRefV1::Completion(completion.key),
                ));
            }
        }
        for queue in &self.queues {
            let active = self
                .dispatches
                .iter()
                .filter(|r| r.key.queue == queue.key && r.state.retains_resources())
                .count();
            if active > queue.capacity as usize {
                return Err(InvariantViolationV1::QueueOversubscribed(queue.key));
            }
        }
        Ok(())
    }

    fn device(&self, key: DeviceKeyV1) -> Result<&DeviceRecordV1, TransitionErrorV1> {
        self.devices
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Device(key)))
    }
    fn device_mut(&mut self, key: DeviceKeyV1) -> Result<&mut DeviceRecordV1, TransitionErrorV1> {
        self.devices
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Device(key)))
    }
    fn vm(&self, key: VmKeyV1) -> Result<&VmRecordV1, TransitionErrorV1> {
        self.vms
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Vm(key)))
    }
    fn vm_mut(&mut self, key: VmKeyV1) -> Result<&mut VmRecordV1, TransitionErrorV1> {
        self.vms
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Vm(key)))
    }
    fn allocation(&self, key: AllocationKeyV1) -> Result<&AllocationRecordV1, TransitionErrorV1> {
        self.allocations
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Allocation(key)))
    }
    fn allocation_mut(
        &mut self,
        key: AllocationKeyV1,
    ) -> Result<&mut AllocationRecordV1, TransitionErrorV1> {
        self.allocations
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Allocation(key)))
    }
    fn mapping(&self, key: MappingKeyV1) -> Result<&MappingRecordV1, TransitionErrorV1> {
        self.mappings
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Mapping(key)))
    }
    fn mapping_mut(
        &mut self,
        key: MappingKeyV1,
    ) -> Result<&mut MappingRecordV1, TransitionErrorV1> {
        self.mappings
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Mapping(key)))
    }
    fn code(&self, key: LoadedCodeKeyV1) -> Result<&LoadedCodeRecordV1, TransitionErrorV1> {
        self.loaded_code
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::LoadedCode(key)))
    }
    fn code_mut(
        &mut self,
        key: LoadedCodeKeyV1,
    ) -> Result<&mut LoadedCodeRecordV1, TransitionErrorV1> {
        self.loaded_code
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::LoadedCode(key)))
    }
    fn queue(&self, key: QueueKeyV1) -> Result<&QueueRecordV1, TransitionErrorV1> {
        self.queues
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Queue(key)))
    }
    fn queue_mut(&mut self, key: QueueKeyV1) -> Result<&mut QueueRecordV1, TransitionErrorV1> {
        self.queues
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Queue(key)))
    }
    fn dispatch(&self, key: DispatchKeyV1) -> Result<&DispatchRecordV1, TransitionErrorV1> {
        self.dispatches
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Dispatch(key)))
    }
    fn dispatch_mut(
        &mut self,
        key: DispatchKeyV1,
    ) -> Result<&mut DispatchRecordV1, TransitionErrorV1> {
        self.dispatches
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Dispatch(key)))
    }
    fn completion(&self, key: CompletionKeyV1) -> Result<&CompletionRecordV1, TransitionErrorV1> {
        self.completions
            .iter()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Completion(key)))
    }
    fn completion_mut(
        &mut self,
        key: CompletionKeyV1,
    ) -> Result<&mut CompletionRecordV1, TransitionErrorV1> {
        self.completions
            .iter_mut()
            .find(|r| r.key == key)
            .ok_or(TransitionErrorV1::NotFound(RecordRefV1::Completion(key)))
    }
}

fn completion_matches(dispatch: DispatchStateV1, completion: CompletionStateV1) -> bool {
    matches!(
        (dispatch, completion),
        (DispatchStateV1::Prepared, CompletionStateV1::Armed)
            | (DispatchStateV1::Published, CompletionStateV1::Armed)
            | (DispatchStateV1::Ambiguous, CompletionStateV1::Ambiguous)
            | (DispatchStateV1::Completed, CompletionStateV1::Observed)
            | (
                DispatchStateV1::FailedQuiescent,
                CompletionStateV1::QuiescedFailure
            )
            | (
                DispatchStateV1::AbortedBeforePublication,
                CompletionStateV1::CancelledBeforePublication
            )
    )
}

fn require_state(condition: bool, record: RecordRefV1) -> Result<(), TransitionErrorV1> {
    if condition {
        Ok(())
    } else {
        Err(TransitionErrorV1::IllegalState(record))
    }
}

fn ensure_room(actual: usize, maximum: usize, kind: RecordKindV1) -> Result<(), TransitionErrorV1> {
    if actual < maximum {
        Ok(())
    } else {
        Err(TransitionErrorV1::CapacityExceeded { kind, maximum })
    }
}

fn ranges_overlap(left_start: u64, left_len: u64, right_start: u64, right_len: u64) -> bool {
    let left_end = left_start + left_len;
    let right_end = right_start + right_len;
    left_start < right_end && right_start < left_end
}

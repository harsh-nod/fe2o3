use super::*;
use crate::queue::dispatch_binding::preparation::{
    PreparationStageV1, PrimaryPreparationSnapshotV1,
};
use crate::queue::dispatch_binding::{
    actual_persistent_control_test_program, prepare_public_fixed_dispatch_resources_in_place,
};
use crate::queue_resources::queue_resource_plan_for_test_v1;
use crate::shared_memory::{
    Gfx942DeviceMemoryDispatchAuthorityV1, PreparationMemoryFixtureV1 as Memory,
};
use fe2o3_amdhsa_loader::ValidatedKernelEnvelope;
use fe2o3_aql::AqlDispatchGeometryV1;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

#[path = "integration_preparation_tests.rs"]
mod preparation_cases;

#[path = "integration_projection_tests.rs"]
mod projection_cases;

#[path = "../construction_auxiliary/integration_tests.rs"]
mod auxiliary_cases;

#[path = "integration_platform.rs"]
mod platform;
use crate::queue_linux::primary_fixture::{LocalGateV1, LocalResourcesV1};
use platform::{Fixture, Owner, OwnerIdentity, Role, assert_platform};

#[derive(Default)]
struct Trace {
    calls: Vec<&'static str>,
    fault: Option<(&'static str, usize, bool)>,
    native_fault: Option<(&'static str, usize, bool)>,
    create: u8,
    poison: bool,
    cleanup: usize,
    cleanup_panic: Option<bool>,
    drops: usize,
    session: u64,
    initial_data: Option<crate::shared_memory::PreparationMemoryObservationV1>,
    initial_preparation: Option<PrimaryPreparationSnapshotV1>,
    minted: Vec<OwnerIdentity>,
    create_return: Option<(u32, u64)>,
    create_returns: Vec<(u32, u64)>,
    create_collision: bool,
    dependency_invalid: bool,
    local_gate: Option<LocalGateV1>,
    local_resources: LocalResourcesV1,
    local_finish_poison: bool,
    projection_fault: Option<(
        &'static str,
        usize,
        crate::shared_memory::PrimaryProjectionCaseV1,
    )>,
}

thread_local! { static ACTIVE: RefCell<Option<Rc<RefCell<Trace>>>> = const { RefCell::new(None) }; }

fn trace() -> Rc<RefCell<Trace>> {
    ACTIVE.with(|a| a.borrow().as_ref().unwrap().clone())
}

fn record(name: &'static str) -> usize {
    let trace = trace();
    let mut t = trace.borrow_mut();
    t.calls.push(name);
    t.calls.iter().filter(|&&call| call == name).count()
}

fn step(name: &'static str) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    let occurrence = record(name);
    let fault = trace().borrow().fault;
    if let Some((at, nth, panic)) = fault
        && (at, nth) == (name, occurrence)
    {
        if panic {
            std::panic::panic_any((name, occurrence));
        }
        return Err(ComputeAqlQueueSessionErrorV1::Contract(name));
    }
    Ok(())
}

fn memory_step(name: &'static str) -> Result<(), MemorySessionError> {
    step(name).map_err(|_| MemorySessionError::Model(name))
}

fn native_step(memory: &mut Memory, name: &'static str, operation: &'static str) {
    let occurrence = record(name);
    projection_step(memory, name, occurrence);
    let fault = trace().borrow().native_fault;
    if let Some((at, nth, panic)) = fault
        && (at, nth) == (name, occurrence)
    {
        memory.primary_arm_native(operation, panic);
    }
}

fn projection_step(memory: &mut Memory, name: &'static str, occurrence: usize) {
    if let Some((at, nth, case)) = trace().borrow().projection_fault
        && (at, nth) == (name, occurrence)
    {
        memory.primary_arm_projection_v1(case);
    }
}

impl construction::RingMemoryV1 for Memory {
    fn preflight_cpu(&self, ring: &CpuRingAuthorityV1) -> Result<(), MemorySessionError> {
        memory_step("ring-preflight-cpu")?;
        match ring {
            CpuRingAuthorityV1::AqlSpecial(t) => self.primary_preflight_cpu(t),
            CpuRingAuthorityV1::ExecutableProbe(t) => self.primary_preflight_cpu(t),
            CpuRingAuthorityV1::UserptrProbe(t) => self.primary_preflight_cpu(t),
        }
    }
    fn preflight_mapped(
        &self,
        ring: &construction::MappedRingV1,
    ) -> Result<(), MemorySessionError> {
        memory_step("ring-preflight-mapped")?;
        match ring {
            construction::MappedRingV1::AqlSpecial(t) => self.primary_preflight_mapped(t),
            construction::MappedRingV1::ExecutableProbe(t) => self.primary_preflight_mapped(t),
            construction::MappedRingV1::UserptrProbe(t) => self.primary_preflight_mapped(t),
        }
    }
    fn map(
        &mut self,
        ring: CpuRingAuthorityV1,
    ) -> Result<construction::MappedRingV1, MemorySessionError> {
        native_step(self, "ring-map", "map_gpu");
        match ring {
            CpuRingAuthorityV1::AqlSpecial(t) => {
                Memory::map(self, t).map(construction::MappedRingV1::AqlSpecial)
            }
            CpuRingAuthorityV1::ExecutableProbe(t) => {
                Memory::map(self, t).map(construction::MappedRingV1::ExecutableProbe)
            }
            CpuRingAuthorityV1::UserptrProbe(t) => {
                Memory::map(self, t).map(construction::MappedRingV1::UserptrProbe)
            }
        }
    }
    fn retain(
        &mut self,
        ring: construction::MappedRingV1,
    ) -> Result<RingAuthority, MemorySessionError> {
        record("ring-retain");
        match ring {
            construction::MappedRingV1::AqlSpecial(t) => {
                self.primary_retain(t).map(RingAuthority::AqlSpecial)
            }
            construction::MappedRingV1::ExecutableProbe(t) => {
                self.primary_retain(t).map(RingAuthority::ExecutableProbe)
            }
            construction::MappedRingV1::UserptrProbe(t) => {
                self.primary_retain(t).map(RingAuthority::UserptrProbe)
            }
        }
    }
}

impl PrimaryMemoryV1 for Memory {
    fn plan_aql_queue_resources(
        &self,
        ring_bytes: u32,
    ) -> Result<Gfx942AqlQueueResourcePlanV1, ComputeAqlQueueSessionErrorV1> {
        step("plan-auxiliary-resources")?;
        Ok(queue_resource_plan_for_test_v1(ring_bytes))
    }
    fn allocate_ring(
        &mut self,
        backing: QueueRingBackingV1,
        bytes: usize,
    ) -> Result<CpuRingAuthorityV1, MemorySessionError> {
        memory_step("allocate-ring")?;
        let occurrence = trace()
            .borrow()
            .calls
            .iter()
            .filter(|&&c| c == "allocate-ring")
            .count();
        projection_step(self, "allocate-ring", occurrence);
        match backing {
            QueueRingBackingV1::AqlSpecial => {
                self.allocate(bytes).map(CpuRingAuthorityV1::AqlSpecial)
            }
            QueueRingBackingV1::ExecutableProbe => self
                .allocate(bytes)
                .map(CpuRingAuthorityV1::ExecutableProbe),
            QueueRingBackingV1::UserptrProbe => {
                self.allocate(bytes).map(CpuRingAuthorityV1::UserptrProbe)
            }
        }
    }
    fn initialize_ring(
        &mut self,
        ring: &mut CpuRingAuthorityV1,
    ) -> Result<Result<(), NativeAqlSubmissionErrorV1>, MemorySessionError> {
        memory_step("initialize-ring")?;
        match ring {
            CpuRingAuthorityV1::AqlSpecial(t) => self.primary_write(t, initialize_invalid_ring),
            CpuRingAuthorityV1::ExecutableProbe(t) => {
                self.primary_write(t, initialize_invalid_ring)
            }
            CpuRingAuthorityV1::UserptrProbe(t) => self.primary_write(t, initialize_invalid_ring),
        }
    }
    fn allocate_userptr_aql_control(
        &mut self,
    ) -> Result<Cpu<UserptrAqlControlGttV1>, MemorySessionError> {
        memory_step("allocate-control")?;
        native_step(self, "native-control", "alloc_userptr");
        self.allocate(4096)
    }
    fn allocate_host_visible_coherent(
        &mut self,
        bytes: usize,
    ) -> Result<Cpu<HostVisibleCoherentGttV1>, MemorySessionError> {
        memory_step("allocate-completion")?;
        native_step(self, "native-completion", "alloc");
        let token = self.allocate(bytes)?;
        self.primary_align_completion(&token);
        Ok(token)
    }
    fn allocate_executable(
        &mut self,
        bytes: usize,
    ) -> Result<Cpu<ExecutableGttV1>, MemorySessionError> {
        memory_step("allocate-executable")?;
        native_step(self, "native-executable", "alloc");
        self.allocate(bytes)
    }
    fn with_bytes_mut<P: GttProfileV1, R>(
        &mut self,
        token: &mut Cpu<P>,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        memory_step("initialize-bytes")?;
        self.primary_write(token, f)
    }
    fn preflight_cpu_queue_token_v1<P: GttProfileV1>(
        &self,
        token: &Cpu<P>,
    ) -> Result<(), MemorySessionError> {
        memory_step("preflight-cpu")?;
        self.primary_preflight_cpu(token)
    }
    fn preflight_mapped_queue_token_v1<P: crate::shared_memory::MutableGpuGttProfileV1>(
        &self,
        token: &Mapped<P>,
    ) -> Result<(), MemorySessionError> {
        memory_step("preflight-mapped")?;
        self.primary_preflight_mapped(token)
    }
    fn preflight_immutable_queue_token_v1(&self, token: &Sealed) -> Result<(), MemorySessionError> {
        memory_step("preflight-immutable")?;
        self.primary_preflight_immutable(token)
    }
    fn preflight_executable_queue_token_v1(
        &self,
        token: &Executable,
    ) -> Result<(), MemorySessionError> {
        memory_step("preflight-executable")?;
        self.primary_preflight_executable(token)
    }
    fn map_to_gpu<P: crate::shared_memory::MutableGpuGttProfileV1>(
        &mut self,
        token: Cpu<P>,
    ) -> Result<Mapped<P>, MemorySessionError> {
        native_step(self, "map-mutable", "map_gpu");
        Memory::map(self, token)
    }
    fn seal_executable(
        &mut self,
        token: Cpu<ExecutableGttV1>,
    ) -> Result<Sealed, MemorySessionError> {
        native_step(self, "seal", "protect_cpu_read_only");
        self.primary_seal(token)
    }
    fn map_executable_to_gpu(&mut self, token: Sealed) -> Result<Executable, MemorySessionError> {
        native_step(self, "map-executable", "map_gpu");
        self.primary_map_executable(token)
    }
    fn retain_aql_control_resource(
        &mut self,
        token: Mapped<UserptrAqlControlGttV1>,
    ) -> Result<ControlAuthority, MemorySessionError> {
        record("retain-control");
        self.primary_retain(token)
    }
    fn retain_aql_completion_signal_resource(
        &mut self,
        token: Mapped<HostVisibleCoherentGttV1>,
    ) -> Result<CompletionSignalAuthority, MemorySessionError> {
        record("retain-completion");
        self.primary_retain(token)
    }
    fn retain_aql_eop_resource(
        &mut self,
        token: Executable,
    ) -> Result<EopAuthority, MemorySessionError> {
        record("retain-eop");
        self.primary_retain(token)
    }
    fn retain_aql_context_save_resource(
        &mut self,
        token: Executable,
    ) -> Result<ContextSaveAuthority, MemorySessionError> {
        record("retain-context");
        self.primary_retain(token)
    }
    fn check_queue_currentness(&mut self) -> Result<(), MemorySessionError> {
        memory_step("currentness")?;
        self.primary_currentness()
    }
    fn queue_model_device(&self) -> fe2o3_runtime_model::ModelDeviceAdmissionV1 {
        self.primary_device()
    }
    fn take_queue_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, MemorySessionError> {
        memory_step("foundation")?;
        self.primary_transfer(&[])
    }
    fn take_queue_model_foundation_with_dispatch_memory(
        &mut self,
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<QueueModelFoundationV1, MemorySessionError> {
        memory_step("foundation")?;
        self.primary_transfer(authorities)
    }
    fn authenticate_queue_model_foundation(
        &self,
        foundation: &QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError> {
        memory_step("authenticate")?;
        self.primary_authenticate(foundation)
    }
    fn opener_pid(&self) -> u32 {
        std::process::id()
    }
    fn create_queue(
        &mut self,
        mut args: fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlCreateQueueArgs> {
        let occurrence = record("create");
        let mode = trace().borrow().create;
        if mode == 5 {
            std::panic::panic_any("CREATE panic");
        }
        if !matches!(mode, 1 | 2) {
            args.queue_id = if trace().borrow().create_collision {
                7
            } else {
                6 + occurrence as u32
            };
            args.doorbell_offset = (fe2o3_kfd_uapi::KFD_MMAP_TYPE_DOORBELL
                << fe2o3_kfd_uapi::KFD_MMAP_TYPE_SHIFT)
                | ((u64::from(args.gpu_id) & 0xffff) << fe2o3_kfd_uapi::KFD_MMAP_GPU_ID_HASH_SHIFT)
                | (8 * occurrence as u64);
        }
        if mode == 4 {
            args.ring_size *= 2;
        }
        trace().borrow_mut().create_return = Some((args.queue_id, args.doorbell_offset));
        trace()
            .borrow_mut()
            .create_returns
            .push((args.queue_id, args.doorbell_offset));
        let status = match mode {
            1 => fe2o3_runtime_model::QueueSyscallStatusV1::FailedNoEffect,
            2 | 3 => fe2o3_runtime_model::QueueSyscallStatusV1::Indeterminate,
            _ => fe2o3_runtime_model::QueueSyscallStatusV1::Succeeded,
        };
        QueueKernelOutcomeV1 {
            value: args,
            status,
        }
    }
    fn update_queue(
        &mut self,
        _: fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlUpdateQueueArgs> {
        panic!("constructor must not UPDATE")
    }
    fn destroy_queue(
        &mut self,
        _: fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<fe2o3_kfd_uapi::KfdIoctlDestroyQueueArgs> {
        panic!("constructor must not DESTROY")
    }
}

type Preparation = (
    Vec<ValidatedKernelEnvelope<'static>>,
    FixedDispatchPreparationCustodyV1<3>,
);
type Root<P = Preparation> = PrimaryQueueConstructionV1<P, Fixture>;
type RunResult<P = Preparation> = (Box<Root<P>>, Result<(), Box<dyn std::any::Any + Send>>);

#[derive(Default)]
struct ExternalSlots {
    runtime: Option<Owner>,
    control: Option<Owner>,
}

fn setup_memory() -> (Memory, Rc<RefCell<Trace>>) {
    setup_memory_with_host_budget(1 << 20)
}

fn setup_memory_with_host_budget(bytes: u64) -> (Memory, Rc<RefCell<Trace>>) {
    let memory = Memory::with_host_budget(true, 1 << 30, bytes);
    let trace = Rc::new(RefCell::new(Trace {
        session: memory.primary_session_id(),
        initial_data: Some(memory.observation()),
        ..Trace::default()
    }));
    ACTIVE.with(|a| *a.borrow_mut() = Some(trace.clone()));
    (memory, trace)
}

fn recipe() -> (
    Vec<ValidatedKernelEnvelope<'static>>,
    [Gfx942FixedDispatchPacketV1; 3],
) {
    const IMAGE: &[u8] = include_bytes!(
        "../../../../fe2o3-runtime/fixtures/trusted-gfx942-inplace-transform-v1/inplace_transform.hsaco"
    );
    let programs = (1..=3)
        .map(|i| actual_persistent_control_test_program(IMAGE, [i; 32]))
        .collect();
    let packets = std::array::from_fn(|i| {
        let mut bytes = [0; 16];
        bytes[8..].copy_from_slice(&1024_u64.to_le_bytes());
        Gfx942FixedDispatchPacketV1::new(
            i,
            AqlDispatchGeometryV1::new([256, 1, 1], [256, 1, 1]).unwrap(),
            0,
            bytes.into(),
            vec![Gfx942DispatchBufferBindingV1::new(0, 0, 0, 4096)].into_boxed_slice(),
        )
    });
    (programs, packets)
}

fn setup() -> (Box<Root>, Rc<RefCell<Trace>>) {
    let (memory, trace) = setup_memory();
    setup_with_memory(memory, trace)
}

fn setup_with_memory(
    mut memory: Memory,
    trace: Rc<RefCell<Trace>>,
) -> (Box<Root>, Rc<RefCell<Trace>>) {
    let (programs, packets) = recipe();
    let custody = FixedDispatchPreparationCustodyV1::new(packets, memory.roster());
    trace.borrow_mut().initial_data = Some(memory.observation());
    trace.borrow_mut().initial_preparation = Some(custody.primary_snapshot_v1());
    (Root::new_with(memory, (programs, custody)), trace)
}

fn run(root: Box<Root>, backing: QueueRingBackingV1, external: bool) -> RunResult {
    let mut slots = external.then(|| ExternalSlots {
        runtime: Some(Owner::new(Role::Runtime)),
        control: Some(Owner::new(Role::Control)),
    });
    let result = run_with(root, backing, slots.as_mut(), prepare_fixed);
    if let Some(slots) = slots
        && trace().borrow().calls.contains(&"event")
    {
        assert!(slots.runtime.is_none() && slots.control.is_none());
    }
    result
}

fn prepare_fixed(root: &mut Root) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    prepare_public_fixed_dispatch_resources_in_place(
        root.memory.as_mut().unwrap(),
        &root.preparation.0,
        &mut root.preparation.1,
    )?;
    root.dispatch = Some(root.preparation.1.take_completed()?);
    Ok(())
}

fn run_with<P>(
    root: Box<Root<P>>,
    backing: QueueRingBackingV1,
    mut external: Option<&mut ExternalSlots>,
    prepare: impl FnOnce(&mut Root<P>) -> Result<(), ComputeAqlQueueSessionErrorV1>,
) -> RunResult<P> {
    let mut retained = None;
    let mut success = None;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let result = run_rooted_construction_with_v1(
            root,
            |root, entry| {
                prepare(root)?;
                root.construct(
                    entry,
                    queue_resource_plan_for_test_v1(4096),
                    4096,
                    backing,
                    external.as_mut().map(|s| (&mut s.runtime, &mut s.control)),
                )
            },
            |root| {
                if let Some(shadows) = root.unpublished.as_mut() {
                    Fixture::cleanup_unpublished(shadows);
                }
            },
            &Fixture::poison,
            |root| {
                record("retain-root");
                retained = Some(root);
            },
        );
        match result {
            Ok(root) => success = Some(root),
            Err(error) => std::panic::panic_any(error),
        }
    }));
    let root = retained
        .or(success)
        .expect("original constructor root retained or returned");
    (root, result)
}

fn memory<P>(root: &Root<P>) -> &Memory {
    if let Some(memory) = &root.memory {
        return memory;
    }
    if let Some(initialization) = &root.initialization
        && let Some(backend) = &initialization.backend
    {
        return &backend.session;
    }
    if let Some(engine) = &root.engine {
        return &engine.backend.session;
    }
    &root.completed.as_ref().unwrap().engine.backend.session
}

fn assert_common<P>(root: &Root<P>, expected: usize, trace: &Rc<RefCell<Trace>>) {
    assert_eq!(
        root as *const Root<P> as usize, expected,
        "exact original root allocation"
    );
    let t = trace.borrow();
    memory(root).primary_assert_accounts_and_records(t.session);
    memory(root).assert_original_data_unchanged(t.initial_data.as_ref().unwrap());
    assert_eq!(
        t.drops, 0,
        "returned platform owner dropped before settlement"
    );
    assert!(t.calls.iter().filter(|&&s| s == "foundation").count() <= 1);
    assert!(t.calls.iter().filter(|&&s| s == "create").count() <= 1);
    if t.calls.contains(&"publish") {
        assert_eq!(t.cleanup, 0);
    } else if root.unpublished.is_some() {
        assert_eq!(t.cleanup, 1);
    }
}

fn assert_root(root: &Root, expected: usize, trace: &Rc<RefCell<Trace>>) {
    assert_root_with_external(root, expected, trace, None);
}

fn assert_root_with_external(
    root: &Root,
    expected: usize,
    trace: &Rc<RefCell<Trace>>,
    external: Option<&ExternalSlots>,
) {
    assert_common(root, expected, trace);
    assert_platform(root, external);
    let original_dispatch = root
        .dispatch
        .as_ref()
        .or_else(|| root.completed.as_ref().and_then(|c| c.dispatch.as_ref()));
    root.preparation.1.primary_assert_snapshot_v1(
        memory(root),
        trace.borrow().initial_preparation.as_ref().unwrap(),
        original_dispatch,
    );
    let Some(original_dispatch) = original_dispatch else {
        assert!(
            root.ring.is_none(),
            "failed preparation must not enter queue construction"
        );
        return;
    };
    assert_eq!(original_dispatch.primary_fixture_identities_v1().len(), 6);
    memory(root).primary_assert_device_owners(&original_dispatch.device_authorities_inline_v1());
    assert_partition(root, original_dispatch.primary_fixture_identities_v1());
}

fn ring_identity(ring: &RingAuthority) -> SharedGttAllocationIdentityV1 {
    match ring {
        RingAuthority::AqlSpecial(t) => Memory::primary_token_identity(t),
        RingAuthority::ExecutableProbe(t) => Memory::primary_token_identity(t),
        RingAuthority::UserptrProbe(t) => Memory::primary_token_identity(t),
    }
}

fn authority_ids(a: &QueueResourceAuthorityV1) -> [SharedGttAllocationIdentityV1; 4] {
    [
        ring_identity(&a.ring),
        Memory::primary_token_identity(&a.control),
        Memory::primary_token_identity(&a.eop),
        Memory::primary_token_identity(&a.context_save),
    ]
}

fn mutable_ids<P: GttProfileV1, R>(
    prefix: &MutablePrefixV1<P, R>,
    owners: &mut Vec<SharedGttAllocationIdentityV1>,
    markers: &mut Vec<SharedGttAllocationIdentityV1>,
    identify: fn(&R) -> SharedGttAllocationIdentityV1,
) {
    owners.extend(prefix.cpu.iter().map(|t| t.storage_identity()));
    owners.extend(prefix.mapped.iter().map(|t| t.storage_identity()));
    owners.extend(prefix.retained.iter().map(identify));
    markers.extend(prefix.in_session);
}

fn executable_ids<R>(
    prefix: &ExecutablePrefixV1<R>,
    owners: &mut Vec<SharedGttAllocationIdentityV1>,
    markers: &mut Vec<SharedGttAllocationIdentityV1>,
    identify: fn(&R) -> SharedGttAllocationIdentityV1,
) {
    owners.extend(prefix.cpu.iter().map(|t| t.storage_identity()));
    owners.extend(prefix.sealed.iter().map(|t| t.storage_identity()));
    owners.extend(prefix.mapped.iter().map(|t| t.storage_identity()));
    owners.extend(prefix.retained.iter().map(identify));
    markers.extend(prefix.in_session);
}

fn ring_ids(
    ring: &Option<RingConstructionV1>,
    owners: &mut Vec<SharedGttAllocationIdentityV1>,
    markers: &mut Vec<SharedGttAllocationIdentityV1>,
) {
    if let Some(ring) = ring {
        match ring {
            RingConstructionV1::Cpu(CpuRingAuthorityV1::AqlSpecial(t)) => {
                owners.push(t.storage_identity())
            }
            RingConstructionV1::Cpu(CpuRingAuthorityV1::ExecutableProbe(t)) => {
                owners.push(t.storage_identity())
            }
            RingConstructionV1::Cpu(CpuRingAuthorityV1::UserptrProbe(t)) => {
                owners.push(t.storage_identity())
            }
            RingConstructionV1::Mapped(construction::MappedRingV1::AqlSpecial(t)) => {
                owners.push(t.storage_identity())
            }
            RingConstructionV1::Mapped(construction::MappedRingV1::ExecutableProbe(t)) => {
                owners.push(t.storage_identity())
            }
            RingConstructionV1::Mapped(construction::MappedRingV1::UserptrProbe(t)) => {
                owners.push(t.storage_identity())
            }
            RingConstructionV1::Retained(t) => owners.push(ring_identity(t)),
            RingConstructionV1::InSession(id) => markers.push(*id),
            RingConstructionV1::Transferred => (),
        }
    }
}

fn assert_partition<P>(root: &Root<P>, owners: Vec<SharedGttAllocationIdentityV1>) {
    assert_partition_with_markers(root, owners, Vec::new());
}

fn assert_partition_with_markers<P>(
    root: &Root<P>,
    mut owners: Vec<SharedGttAllocationIdentityV1>,
    mut markers: Vec<SharedGttAllocationIdentityV1>,
) {
    let memory = memory(root);
    ring_ids(&root.ring, &mut owners, &mut markers);
    mutable_ids(
        &root.control,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    mutable_ids(
        &root.completion,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    executable_ids(
        &root.eop,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    executable_ids(
        &root.context,
        &mut owners,
        &mut markers,
        Memory::primary_token_identity,
    );
    if let Some(prefix) = &root.resource_prefix {
        owners.extend(prefix.primary_fixture_identities_v1());
    }
    if let Some(a) = &root.authority {
        owners.extend(authority_ids(a));
    }
    let engine = root
        .engine
        .as_ref()
        .or_else(|| root.completed.as_ref().map(|c| &c.engine));
    if let Some(engine) = engine {
        for record in &engine.resources {
            owners.extend(authority_ids(
                record
                    .authority
                    .as_ref()
                    .expect("actual queue tokens retained in engine"),
            ));
        }
    }
    if let Some(c) = &root.completed {
        owners.push(Memory::primary_token_identity(&c.completion_signals));
    }
    let terminal = memory.primary_terminal_identities();
    if memory.primary_terminal_allocation_v1() {
        assert!(
            markers.is_empty(),
            "allocation output has no consumed-input marker"
        );
    } else {
        assert_eq!(
            markers, terminal,
            "markers name exactly the R88-owned token, not another owner"
        );
    }
    owners.extend(terminal);
    let expected = memory.primary_identities();
    assert_eq!(owners.len(), expected.len(), "one owner per native record");
    let owners: std::collections::HashSet<_> = owners.into_iter().collect();
    assert_eq!(owners.len(), expected.len(), "no duplicate token owner");
    let expected = expected.into_iter().collect();
    assert_eq!(
        owners, expected,
        "complete exact native token partition; no dropped or duplicated owner"
    );
}

#[test]
fn same_session_primary_success_uses_actual_preparation_resources_foundation_and_create() {
    for backing in [
        QueueRingBackingV1::AqlSpecial,
        QueueRingBackingV1::ExecutableProbe,
        QueueRingBackingV1::UserptrProbe,
    ] {
        for external in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            let (root, result) = run(root, backing, external);
            assert!(
                result.is_ok(),
                "construction should succeed: {:?}",
                result
                    .as_ref()
                    .err()
                    .and_then(|p| p.downcast_ref::<ComputeAqlQueueSessionErrorV1>())
            );
            assert_root(&root, address, &trace);
            let complete = root.completed.as_ref().unwrap();
            memory(&root)
                .primary_authenticate(&complete.engine.foundation)
                .unwrap();
            assert_eq!(complete.observation.queue_id, 7);
            assert_eq!(
                (
                    complete.observation.doorbell_slice_bytes,
                    complete.observation.doorbell_byte_offset
                ),
                (
                    fe2o3_kfd_uapi::KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES as usize,
                    8
                )
            );
            let t = trace.borrow();
            assert!(!t.poison);
            assert_eq!(t.calls.iter().filter(|&&s| s == "currentness").count(), 8);
            assert_eq!(t.calls.iter().filter(|&&s| s == "create").count(), 1);
            let publish = t.calls.iter().position(|&s| s == "publish").unwrap();
            assert_eq!(t.calls[publish + 1], "create");
            let doorbell = t.calls.iter().position(|&s| s == "doorbell").unwrap();
            assert_eq!(
                &t.calls[doorbell - 1..],
                &[
                    "currentness",
                    "doorbell",
                    "doorbell-observe",
                    "currentness",
                    "gate-finish"
                ]
            );
        }
    }
}

#[test]
fn same_session_primary_borrowed_failures_retain_original_owner_at_every_observed_boundary() {
    let cases = [
        ("allocate-ring", 1),
        ("allocate-control", 1),
        ("allocate-completion", 1),
        ("allocate-executable", 1),
        ("allocate-executable", 2),
        ("initialize-ring", 1),
        ("initialize-bytes", 1),
        ("initialize-bytes", 2),
        ("initialize-bytes", 3),
        ("initialize-bytes", 4),
        ("runtime-enable", 1),
        ("runtime-validate", 1),
        ("runtime-validate", 2),
        ("gate-arm", 1),
        ("event", 1),
        ("shadow-install", 1),
        ("shadow-init", 1),
        ("event-validate", 1),
        ("shadow-restore", 1),
        ("ring-preflight-cpu", 1),
        ("ring-preflight-mapped", 1),
        ("preflight-cpu", 1),
        ("preflight-cpu", 2),
        ("preflight-cpu", 3),
        ("preflight-cpu", 4),
        ("preflight-immutable", 1),
        ("preflight-immutable", 2),
        ("preflight-mapped", 1),
        ("preflight-mapped", 2),
        ("preflight-executable", 1),
        ("preflight-executable", 2),
        ("foundation", 1),
        ("authenticate", 1),
        ("runtime-created", 1),
        ("recover-outputs", 1),
        ("recover-id", 1),
        ("dependency", 1),
        ("doorbell", 1),
        ("gate-finish", 1),
    ];
    for (name, occurrence) in cases {
        for panic in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            trace.borrow_mut().fault = Some((name, occurrence, panic));
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
            let payload = result.expect_err("injected failure must escape");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&(name, occurrence))
                );
            } else {
                assert!(payload.is::<ComputeAqlQueueSessionErrorV1>());
            }
            assert_root(&root, address, &trace);
            assert_eq!(trace.borrow().poison, name != "allocate-ring");
        }
    }
}

#[test]
fn same_session_primary_real_native_failures_preserve_r88_and_original_accounts() {
    for (name, occurrence) in [
        ("native-control", 1),
        ("native-completion", 1),
        ("native-executable", 1),
        ("native-executable", 2),
        ("seal", 1),
        ("seal", 2),
        ("ring-map", 1),
        ("map-mutable", 1),
        ("map-mutable", 2),
        ("map-executable", 1),
        ("map-executable", 2),
    ] {
        for panic in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            trace.borrow_mut().native_fault = Some((name, occurrence, panic));
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
            let payload = result.expect_err("native fault must be exercised");
            let operation = match name {
                "native-control" => "alloc_userptr",
                "native-completion" | "native-executable" => "alloc",
                "seal" => "protect_cpu_read_only",
                _ => "map_gpu",
            };
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", operation))
                );
            } else {
                let error = payload
                    .downcast_ref::<ComputeAqlQueueSessionErrorV1>()
                    .expect("injected native error");
                let ComputeAqlQueueSessionErrorV1::TerminalCreation { source, .. } = error else {
                    panic!("terminal native error");
                };
                assert!(
                    matches!(&**source, ComputeAqlQueueSessionErrorV1::Memory(MemorySessionError::Injected(at)) if *at == operation),
                    "wrong error: {error:?}"
                );
            }
            assert_root(&root, address, &trace);
            assert!(trace.borrow().poison);
            assert!(!trace.borrow().calls.contains(&"create"));
        }
    }
}

#[test]
fn same_session_primary_create_uncertainty_retains_published_owners_without_cleanup() {
    for mode in 1..=5 {
        let (root, trace) = setup();
        let address = &*root as *const Root as usize;
        trace.borrow_mut().create = mode;
        let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
        assert!(result.is_err());
        assert_root(&root, address, &trace);
        let t = trace.borrow();
        assert!(t.poison);
        assert_eq!(t.cleanup, 0);
        assert!(!t.calls.contains(&"doorbell"));
        if mode != 5 {
            let engine = root.engine.as_ref().unwrap();
            assert_eq!(
                engine.phase(engine.resources[0].key),
                Some(if mode == 1 {
                    fe2o3_runtime_model::ComputeAqlQueuePhaseV1::Planned
                } else {
                    fe2o3_runtime_model::ComputeAqlQueuePhaseV1::Ambiguous
                })
            );
        }
        assert!(
            root.engine.as_ref().unwrap().resources[0]
                .authority
                .is_some()
        );
    }
}

#[test]
fn same_session_primary_currentness_faults_cover_create_and_both_doorbell_checks() {
    for occurrence in 1..=8 {
        for panic in [false, true] {
            let (root, trace) = setup();
            let address = &*root as *const Root as usize;
            trace.borrow_mut().fault = Some(("currentness", occurrence, panic));
            let (root, result) = run(root, QueueRingBackingV1::AqlSpecial, false);
            let payload = result.expect_err("each required currentness boundary must be reached");
            if panic {
                assert_eq!(
                    payload.downcast_ref::<(&str, usize)>(),
                    Some(&("currentness", occurrence))
                );
            } else {
                assert!(payload.is::<ComputeAqlQueueSessionErrorV1>());
            }
            assert_root(&root, address, &trace);
            assert!(!trace.borrow().calls.contains(&"gate-finish"));
            if occurrence == 5 {
                let t = trace.borrow();
                assert!(!t.calls.contains(&"publish"));
                assert!(!t.calls.contains(&"create"));
                assert_eq!(t.cleanup, 1);
            }
            if occurrence >= 7 {
                let completed = root
                    .completed
                    .as_ref()
                    .expect("completed bundle retained across closing checks");
                assert_eq!(completed.doorbell.is_some(), occurrence == 8);
            }
        }
    }
}

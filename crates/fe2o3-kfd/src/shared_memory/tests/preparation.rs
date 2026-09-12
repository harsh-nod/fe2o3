//! Forward the real R86/R88 memory primitives into the production preparation sequencer.

use super::*;
use crate::queue::dispatch_binding::preparation::PreparationMemoryV1;
use crate::queue::dispatch_binding::{DispatchDataAuthorityV1, Gfx942FixedDispatchDataV1};
use crate::shared_memory::dispatch_retention as replay_retention;
use crate::shared_memory::transitions::{self as adapter, ProjectionV1};

type CodeCpu = SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>;
type CodeImmutable = SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>;
type CodeMapped = SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>;
type CodeAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchCodeResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type KernargCpu = SharedGttAllocationV1<KernargGttV1, GttCpuWritableV1>;
type KernargMapped = SharedGttAllocationV1<KernargGttV1, GttGpuAccessibleMutableV1>;
type KernargAuthority = SharedGttQueueResourceAuthorityV1<
    AqlDispatchKernargResourceRoleV1,
    KernargGttV1,
    GttGpuAccessibleMutableV1,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PreparationMemoryCallV1 {
    AllocateCode(usize),
    WriteCode(usize),
    SealCode(usize),
    MapCode(usize),
    AllocateKernarg,
    WriteKernarg,
    MapKernarg,
}

#[derive(Clone, Copy)]
pub(crate) enum PreparationNativeFaultV1 {
    Error(&'static str),
    Panic(&'static str),
    CurrentnessError(usize),
    CurrentnessPanic(usize),
    AccessPanic,
    PartialMap(u32, bool),
    ProjectionRejection,
    Projection(super::primary_projection::PrimaryProjectionCaseV1),
}

pub(crate) struct PreparationMemoryFixtureV1 {
    pub(super) fixture: BackingConstructorFixture,
    pub(super) disposed_controls: Vec<SharedGttAllocationIdentityV1>,
    code_count: usize,
    projection_rejection_va: u64,
    pub(crate) fault: Option<(PreparationMemoryCallV1, PreparationNativeFaultV1)>,
    pub(super) projection_fault: Option<super::primary_projection::PrimaryProjectionCaseV1>,
    pub(super) projection_observation: Option<(
        super::primary_projection::PrimaryProjectionCaseV1,
        MemoryLifecycleStateV1,
    )>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct PreparationMemoryObservationV1 {
    pub(crate) calls: [usize; 8],
    pub(crate) host: Option<Gfx942HostVisibleBackingUsageV1>,
    pub(crate) device: Option<Gfx942DeviceBackingUsageV1>,
    pub(crate) phase: SharedMemorySessionPhaseV1,
    pub(crate) controls: usize,
    pub(crate) pending: bool,
    pub(crate) terminal: usize,
    data: Vec<DataRecordObservation>,
}

#[derive(Debug, Eq, PartialEq)]
struct DataMappingObservation {
    address: u64,
    bytes: Vec<u8>,
    active: bool,
    writable: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct DataRecordObservation {
    identity: crate::sdma::Gfx942SdmaBufferStorageIdentityV1,
    gpu_va: u64,
    reservation: Option<(u64, usize)>,
    handle: Option<u64>,
    mapping: Option<DataMappingObservation>,
    free_attempted: bool,
}

impl PreparationMemoryFixtureV1 {
    pub(crate) fn retain_replay(
        &self,
        data: &mut Option<Gfx942FixedDispatchDataV1>,
        before_validation: impl FnOnce(),
    ) -> Result<crate::shared_memory::RetainedDispatchDataV1, MemorySessionError> {
        let f = &self.fixture;
        replay_retention::retain_replay_with_v1(
            &f.engine,
            f.device.model_key(),
            f.vm,
            data,
            before_validation,
        )
    }
    pub(crate) fn new(configured: bool) -> Self {
        Self::with_aperture(configured, 0x20_0000)
    }

    pub(crate) fn with_aperture(configured: bool, bytes: u64) -> Self {
        Self::with_host_budget(configured, bytes, 1 << 20)
    }

    pub(crate) fn with_host_budget(configured: bool, bytes: u64, host_bytes: u64) -> Self {
        let mut fixture = BackingConstructorFixture::with_aperture(
            configured.then(|| Gfx942DeviceBackingBudgetV1::new(1 << 20, 64).unwrap()),
            bytes,
        );
        if configured {
            fixture
                .engine
                .configure_host_visible_backing_budget_v1(
                    fixture.device.model_key(),
                    fixture.vm,
                    Gfx942HostVisibleBackingBudgetV1::new(host_bytes, 64).unwrap(),
                )
                .unwrap();
        }
        Self {
            fixture,
            disposed_controls: Vec::new(),
            code_count: 0,
            projection_rejection_va: 0x1_0000_u64.checked_add(bytes).unwrap(),
            fault: None,
            projection_fault: None,
            projection_observation: None,
        }
    }

    pub(crate) fn allocate<P: GttProfileV1>(
        &mut self,
        bytes: usize,
    ) -> Result<SharedGttAllocationV1<P, GttCpuWritableV1>, MemorySessionError> {
        let fault = self.take_primary_projection_v1();
        let f = &mut self.fixture;
        let mut projection = ProjectionV1::new(&mut f.foundation, f.device, f.vm);
        projection.fault = fault;
        adapter::allocate_v1(&mut f.engine, &mut projection, bytes, || {
            panic!("unexpected preparation revision exhaustion")
        })
    }

    pub(crate) fn map<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>, MemorySessionError> {
        let fault = self.take_primary_projection_v1();
        let f = &mut self.fixture;
        let mut projection = ProjectionV1::new(&mut f.foundation, f.device, f.vm);
        projection.fault = fault;
        adapter::map_mutable_v1(&mut f.engine, &mut projection, token, || {
            panic!("unexpected preparation revision exhaustion")
        })
    }

    pub(crate) fn host(&mut self, initialized: bool) -> Gfx942FixedDispatchDataV1 {
        let mut token = self.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
        if initialized {
            self.fixture
                .engine
                .with_bytes_mut(&mut token, |bytes| bytes.fill(0x5a))
                .unwrap();
        }
        let token = self.map(token).unwrap();
        if initialized {
            Gfx942FixedDispatchDataV1::host_visible_initialized(
                Gfx942InitializedHostVisibleMemoryV1 { token },
            )
        } else {
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(token)
        }
    }

    pub(crate) fn device(&mut self, initialized: bool) -> Gfx942FixedDispatchDataV1 {
        let f = &mut self.fixture;
        if initialized {
            let source = vec![0x6b; 4096].into_boxed_slice();
            let descriptor = content(&source);
            let source = validate_initialization_source(source, descriptor).unwrap();
            let lease = f
                .engine
                .allocate_device_memory_with_flags(
                    f.device.model_key(),
                    f.vm,
                    4096,
                    4096,
                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap();
            Gfx942FixedDispatchDataV1::initialized(
                f.engine
                    .initialize_public_device_memory(lease, source)
                    .unwrap(),
            )
        } else {
            let lease = f
                .engine
                .allocate_device_memory(f.device.model_key(), f.vm, 4096, 4096)
                .unwrap();
            Gfx942FixedDispatchDataV1::uninitialized(f.engine.map_device_memory(lease).unwrap())
        }
    }

    pub(crate) fn roster(&mut self) -> Vec<Gfx942FixedDispatchDataV1> {
        vec![
            self.device(true),
            self.host(true),
            self.device(false),
            self.host(false),
        ]
    }

    fn arm(&mut self, call: PreparationMemoryCallV1) {
        let Some((at, fault)) = self.fault else {
            return;
        };
        if at != call {
            return;
        }
        let f = &mut self.fixture;
        match fault {
            PreparationNativeFaultV1::Error(operation) => {
                f.engine.backend.fail_operation = Some(operation)
            }
            PreparationNativeFaultV1::Panic(operation) => {
                f.engine.backend.panic_operation = Some(operation)
            }
            PreparationNativeFaultV1::CurrentnessError(delta) => {
                f.engine.backend.fail_currentness_at =
                    Some(f.engine.backend.currentness_calls + delta)
            }
            PreparationNativeFaultV1::CurrentnessPanic(delta) => {
                f.engine.backend.panic_currentness_at =
                    Some(f.engine.backend.currentness_calls + delta)
            }
            PreparationNativeFaultV1::AccessPanic => {
                f.engine
                    .allocations
                    .last_mut()
                    .unwrap()
                    .mapping
                    .as_mut()
                    .unwrap()
                    .panic_access = Some("with_bytes_mut")
            }
            PreparationNativeFaultV1::PartialMap(prefix, errno) => {
                f.engine.backend.map_progress = prefix;
                f.engine.backend.map_errno = errno;
            }
            PreparationNativeFaultV1::ProjectionRejection => {
                f.engine.backend.fixed_va = Some(self.projection_rejection_va)
            }
            PreparationNativeFaultV1::Projection(case) => self.primary_arm_projection_v1(case),
        }
    }

    pub(crate) fn observation(&self) -> PreparationMemoryObservationV1 {
        self.assert_disposed_controls_v1();
        let e = &self.fixture.engine;
        let b = &e.backend;
        let mapping = |m: &FakeMapping| DataMappingObservation {
            address: m.address,
            bytes: m.bytes.clone(),
            active: m.active,
            writable: m.writable,
        };
        let mut data = e
            .allocations
            .iter()
            .filter(|r| r.profile == SharedGttProfileV1::HostVisibleCoherent)
            .map(|r| DataRecordObservation {
                identity: crate::sdma::Gfx942SdmaBufferStorageIdentityV1::Host(
                    SharedGttAllocationIdentityV1 {
                        session_id: e.session_id,
                        id: r.id,
                        generation: r.generation,
                    },
                ),
                gpu_va: r.gpu_va,
                reservation: r.reservation,
                handle: r.handle,
                mapping: r.mapping.as_ref().map(mapping),
                free_attempted: r.free_attempted,
            })
            .collect::<Vec<_>>();
        data.extend(e.device_memory.iter().map(|r| DataRecordObservation {
            identity: crate::sdma::Gfx942SdmaBufferStorageIdentityV1::Device(
                Gfx942DeviceMemoryIdentityV1 {
                    id: r.id,
                    generation: r.generation,
                    device: r.device,
                    vm: r.vm,
                },
            ),
            gpu_va: r.gpu_va,
            reservation: r.reservation,
            handle: r.handle,
            mapping: r.mapping.as_ref().map(mapping),
            free_attempted: r.free_attempted,
        }));
        PreparationMemoryObservationV1 {
            calls: [
                b.currentness_calls,
                b.reserve_va_calls,
                b.alloc_calls,
                b.map_cpu_calls,
                b.map_gpu_calls,
                b.unmap_gpu_calls,
                b.free_calls,
                b.release_va_calls,
            ],
            host: e.host_backing_account.as_ref().map(|a| a.usage()),
            device: e.device_backing_account.as_ref().map(|a| a.usage()),
            phase: e.phase(),
            controls: e
                .allocations
                .iter()
                .filter(|r| {
                    matches!(
                        r.profile,
                        SharedGttProfileV1::Executable | SharedGttProfileV1::Kernarg
                    ) && !self.is_disposed_control_v1(r.id, r.generation)
                })
                .count(),
            pending: e.pending_allocation.is_some(),
            terminal: e.terminal_transition.as_ref().map_or(0, |t| {
                usize::from(t.input.is_some()) + usize::from(t.output.is_some())
            }),
            data,
        }
    }

    pub(crate) fn assert_data_unchanged(&self, before: &PreparationMemoryObservationV1) {
        let after = self.observation();
        assert_eq!(
            after.data, before.data,
            "original native data bytes and record owners changed"
        );
    }

    pub(crate) fn assert_original_data_unchanged(&self, before: &PreparationMemoryObservationV1) {
        self.assert_original_records_unchanged(before);
        assert_eq!(
            self.observation().device,
            before.device,
            "original N2 usage unchanged"
        );
    }

    pub(crate) fn assert_original_records_unchanged(
        &self,
        before: &PreparationMemoryObservationV1,
    ) {
        let after = self.observation();
        for expected in &before.data {
            assert_eq!(
                after
                    .data
                    .iter()
                    .filter(|r| r.identity == expected.identity)
                    .collect::<Vec<_>>(),
                vec![expected],
                "original data bytes, mapping and native record"
            );
        }
    }

    pub(crate) fn assert_control_custody(
        &self,
        owners: &[SharedGttAllocationIdentityV1],
        markers: &[SharedGttAllocationIdentityV1],
        allocating: Option<SharedGttProfileV1>,
    ) {
        let e = &self.fixture.engine;
        let mut actual = owners.to_vec();
        let mut terminal = Vec::new();
        if let Some(t) = &e.terminal_transition {
            for token in t.input.iter().chain(t.output.iter()) {
                terminal.push(SharedGttAllocationIdentityV1 {
                    session_id: token.session_id,
                    id: token.id,
                    generation: token.generation,
                });
            }
        }
        if let Some(profile) = allocating {
            assert!(markers.is_empty());
            if let Some(t) = &e.terminal_transition {
                assert!(t.input.is_none());
                assert_eq!(t.output.as_ref().unwrap().profile, profile);
                assert!(matches!(
                    t.stage,
                    adapter::TransitionStageV1::AllocationEvidence
                        | adapter::TransitionStageV1::AllocationProjection
                        | adapter::TransitionStageV1::AllocationCommit
                ));
                assert!(e.pending_allocation.is_none());
            }
        } else {
            assert_eq!(
                markers, terminal,
                "every terminal transition must have its exact preparation handoff marker"
            );
            assert!(e.pending_allocation.is_none());
        }
        actual.extend(terminal);
        assert_eq!(
            actual
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            actual.len(),
            "duplicate token ownership"
        );
        let controls: Vec<_> = e
            .allocations
            .iter()
            .filter(|r| {
                matches!(
                    r.profile,
                    SharedGttProfileV1::Executable | SharedGttProfileV1::Kernarg
                ) && !self.is_disposed_control_v1(r.id, r.generation)
            })
            .collect();
        assert_eq!(actual.len(), controls.len(), "missing actual control owner");
        for record in controls {
            assert!(actual.contains(&SharedGttAllocationIdentityV1 {
                session_id: e.session_id,
                id: record.id,
                generation: record.generation
            }));
            assert!(record.reservation.is_some() && record.handle.is_some());
            assert!(record.mapping.as_ref().unwrap().active);
            assert!(!record.free_attempted);
        }
        if let Some(pending) = &e.pending_allocation {
            assert_eq!(
                Some(pending.profile),
                allocating,
                "R87 pending allocation must remain linked to its exact allocation profile"
            );
            assert_eq!(pending.id + 1, e.next_id);
            assert!(e.allocations.iter().all(|r| r.id != pending.id));
        }
    }

    pub(super) fn is_disposed_control_v1(&self, id: u64, generation: u64) -> bool {
        self.disposed_controls
            .contains(&SharedGttAllocationIdentityV1 {
                session_id: self.fixture.engine.session_id,
                id,
                generation,
            })
    }

    pub(super) fn assert_disposed_controls_v1(&self) {
        let e = &self.fixture.engine;
        assert_eq!(
            self.disposed_controls.len(),
            self.disposed_controls
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
        );
        for id in &self.disposed_controls {
            assert_eq!(id.session_id, e.session_id);
            let record = e.allocations.iter().find(|r| r.id == id.id).unwrap();
            assert_eq!(record.generation, id.generation);
            assert!(matches!(
                record.profile,
                SharedGttProfileV1::Executable | SharedGttProfileV1::Kernarg
            ));
            assert_eq!(record.phase, SharedAllocationPhaseV1::Released);
            assert!(record.free_attempted);
            assert!(
                record.reservation.is_none() && record.handle.is_none() && record.mapping.is_none()
            );
            assert!(record.host_backing_charge.is_none());
        }
        assert_eq!(
            e.allocations
                .iter()
                .filter(|r| r.phase == SharedAllocationPhaseV1::Released)
                .count(),
            self.disposed_controls.len()
        );
    }

    pub(crate) fn code_identity(authority: &CodeAuthority) -> SharedGttAllocationIdentityV1 {
        authority.token.storage_identity()
    }
    pub(crate) fn kernarg_identity(authority: &KernargAuthority) -> SharedGttAllocationIdentityV1 {
        authority.token.storage_identity()
    }

    pub(crate) fn data_storage(
        authority: &DispatchDataAuthorityV1,
    ) -> crate::sdma::Gfx942SdmaBufferStorageIdentityV1 {
        match authority {
            DispatchDataAuthorityV1::Device(a) => {
                crate::sdma::Gfx942SdmaBufferStorageIdentityV1::Device(a.lease.storage_identity())
            }
            DispatchDataAuthorityV1::HostVisible(a) => {
                crate::sdma::Gfx942SdmaBufferStorageIdentityV1::Host(a.token.storage_identity())
            }
        }
    }

    pub(crate) fn mapped_bytes(&self, mapping: MemoryMappingKeyV1) -> &[u8] {
        let e = &self.fixture.engine;
        let record = e
            .allocations
            .iter()
            .find(|r| model_keys(self.fixture.vm, r.id, r.generation).2 == mapping)
            .unwrap();
        let mapping = record.mapping.as_ref().unwrap();
        &mapping.bytes[mapping.byte_offset..mapping.byte_offset + record.layout.requested_bytes]
    }
}

#[test]
fn replay_retention_borrows_original_data_through_validation_panic() {
    let mut memory = PreparationMemoryFixtureV1::new(true);
    let mut data = Some(memory.device(true));
    let identity = data.as_ref().unwrap().sdma_storage_identity();
    let content = data.as_ref().unwrap().initialized_content();
    let before = memory.observation();
    let f = &memory.fixture;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        replay_retention::retain_replay_with_v1(
            &f.engine,
            f.device.model_key(),
            f.vm,
            &mut data,
            || std::panic::panic_any("replay validation"),
        )
    }));
    let Err(payload) = caught else {
        panic!("injected panic must escape")
    };
    assert_eq!(payload.downcast_ref::<&str>(), Some(&"replay validation"));
    let data = data.as_ref().expect("original replay token remains rooted");
    assert_eq!(data.sdma_storage_identity(), identity);
    assert_eq!(data.initialized_content(), content);
    assert_eq!(memory.observation(), before);
}

#[test]
fn replay_retention_rejects_foreign_or_wrong_storage_without_moving_it() {
    for host in [false, true] {
        let memory = PreparationMemoryFixtureV1::new(true);
        let mut source = PreparationMemoryFixtureV1::new(true);
        let mut data = Some(if host {
            source.host(true)
        } else {
            source.device(true)
        });
        let identity = data.as_ref().unwrap().sdma_storage_identity();
        let content = data.as_ref().unwrap().initialized_content();
        let before = source.observation();
        let target_before = memory.observation();
        let f = &memory.fixture;
        assert!(
            replay_retention::retain_replay_with_v1(
                &f.engine,
                f.device.model_key(),
                f.vm,
                &mut data,
                || {},
            )
            .is_err()
        );
        assert_eq!(data.as_ref().unwrap().sdma_storage_identity(), identity);
        assert_eq!(data.as_ref().unwrap().initialized_content(), content);
        assert_eq!(source.observation(), before);
        assert_eq!(memory.observation(), target_before);
    }
}

#[test]
fn replay_retention_commits_exact_descriptor_and_authority_without_native_effects() {
    for initialized in [false, true] {
        let mut memory = PreparationMemoryFixtureV1::new(true);
        let mut data = Some(memory.device(initialized));
        let identity = data.as_ref().unwrap().sdma_storage_identity();
        let content = data.as_ref().unwrap().initialized_content();
        let layout = data.as_ref().unwrap().layout();
        let before = memory.observation();
        let f = &memory.fixture;
        let retained = replay_retention::retain_replay_with_v1(
            &f.engine,
            f.device.model_key(),
            f.vm,
            &mut data,
            || {},
        )
        .unwrap();
        assert!(data.is_none());
        assert_eq!(
            PreparationMemoryFixtureV1::data_storage(&retained.authority),
            identity
        );
        assert_eq!(retained.initialized_content, content);
        assert_eq!(retained.fully_initialized, initialized);
        assert_eq!(retained.layout, layout);
        assert_eq!(memory.observation(), before);
        assert!(
            replay_retention::retain_replay_with_v1(
                &f.engine,
                f.device.model_key(),
                f.vm,
                &mut data,
                || {},
            )
            .is_err()
        );
    }
}

impl PreparationMemoryV1 for PreparationMemoryFixtureV1 {
    fn retain_data(
        &mut self,
        data: &mut Vec<Gfx942FixedDispatchDataV1>,
    ) -> Result<RetainedDispatchDataRosterV1, MemorySessionError> {
        let f = &self.fixture;
        crate::shared_memory::dispatch_retention::retain_v1(
            &f.engine,
            f.device.model_key(),
            f.vm,
            data,
        )
    }
    fn allocate_code(&mut self, bytes: usize) -> Result<CodeCpu, MemorySessionError> {
        let index = self.code_count;
        self.code_count += 1;
        self.arm(PreparationMemoryCallV1::AllocateCode(index));
        self.allocate(bytes)
    }
    fn write_code<R>(
        &mut self,
        token: &mut CodeCpu,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.arm(PreparationMemoryCallV1::WriteCode(self.code_count - 1));
        self.fixture.engine.with_bytes_mut(token, f)
    }
    fn seal_code(&mut self, token: CodeCpu) -> Result<CodeImmutable, MemorySessionError> {
        self.arm(PreparationMemoryCallV1::SealCode(self.code_count - 1));
        adapter::seal_v1(&mut self.fixture.engine, token)
    }
    fn map_code(&mut self, token: CodeImmutable) -> Result<CodeMapped, MemorySessionError> {
        self.arm(PreparationMemoryCallV1::MapCode(self.code_count - 1));
        let fault = self.take_primary_projection_v1();
        let f = &mut self.fixture;
        let mut projection = ProjectionV1::new(&mut f.foundation, f.device, f.vm);
        projection.fault = fault;
        adapter::map_executable_v1(&mut f.engine, &mut projection, token, || {
            panic!("unexpected preparation revision exhaustion")
        })
    }
    fn retain_code(&mut self, token: CodeMapped) -> Result<CodeAuthority, MemorySessionError> {
        adapter::retain_v1(&mut self.fixture.engine, self.fixture.vm, token)
    }
    fn allocate_kernarg(&mut self, bytes: usize) -> Result<KernargCpu, MemorySessionError> {
        self.arm(PreparationMemoryCallV1::AllocateKernarg);
        self.allocate(bytes)
    }
    fn write_kernarg<R>(
        &mut self,
        token: &mut KernargCpu,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.arm(PreparationMemoryCallV1::WriteKernarg);
        self.fixture.engine.with_bytes_mut(token, f)
    }
    fn map_kernarg(&mut self, token: KernargCpu) -> Result<KernargMapped, MemorySessionError> {
        self.arm(PreparationMemoryCallV1::MapKernarg);
        self.map(token)
    }
    fn retain_kernarg(
        &mut self,
        token: KernargMapped,
    ) -> Result<KernargAuthority, MemorySessionError> {
        adapter::retain_v1(&mut self.fixture.engine, self.fixture.vm, token)
    }
    fn quarantine(&mut self) {
        self.fixture.engine.phase = SharedMemorySessionPhaseV1::Quarantined;
    }
}

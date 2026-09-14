//! Private tokens over actual fake-native records; not Linux authority issuance.

use super::*;
use crate::queue::dispatch_binding::pristine_abort::PristineControlReleaseV1;
use crate::shared_memory::{control_cleanup, transitions};

mod cleanup_tests;

type Code = SharedGttQueueResourceAuthorityV1<
    AqlDispatchCodeResourceRoleV1,
    ExecutableGttV1,
    GttGpuAccessibleExecutableV1,
>;
type Kernarg = SharedGttQueueResourceAuthorityV1<
    AqlDispatchKernargResourceRoleV1,
    KernargGttV1,
    GttGpuAccessibleMutableV1,
>;
type Host = SharedGttQueueResourceAuthorityV1<
    AqlDispatchHostDataResourceRoleV1,
    HostVisibleCoherentGttV1,
    GttGpuAccessibleMutableV1,
>;

pub(crate) struct PristineAbortMemoryFixtureV1 {
    fixture: BackingConstructorFixture,
    control_ordinal: usize,
    control_failure: Option<(usize, &'static str, bool)>,
    control_unmap: Option<(usize, u32, bool)>,
    projection_fault: Option<(
        usize,
        control_cleanup::CleanupStageV1,
        transitions::ProjectionFaultV1,
    )>,
    process_poisoned: usize,
}

impl PristineAbortMemoryFixtureV1 {
    pub(crate) fn new() -> Self {
        Self::new_configured(true)
    }

    fn new_configured(configured: bool) -> Self {
        let mut fixture = BackingConstructorFixture::new(
            configured.then(|| Gfx942DeviceBackingBudgetV1::new(65536, 16).unwrap()),
        );
        if configured {
            fixture
                .engine
                .configure_host_visible_backing_budget_v1(
                    fixture.device.model_key(),
                    fixture.vm,
                    Gfx942HostVisibleBackingBudgetV1::new(65536, 16).unwrap(),
                )
                .unwrap();
        }
        Self {
            fixture,
            control_ordinal: 0,
            control_failure: None,
            control_unmap: None,
            projection_fault: None,
            process_poisoned: 0,
        }
    }

    fn retain<R: SharedGttQueueResourceRoleV1, P: GttProfileV1, S: GpuMappedGttStateV1>(
        &mut self,
        token: SharedGttAllocationV1<P, S>,
    ) -> SharedGttQueueResourceAuthorityV1<R, P, S> {
        transitions::retain_v1(&mut self.fixture.engine, self.fixture.vm, token).unwrap()
    }

    fn allocate<P: GttProfileV1>(
        &mut self,
        bytes: usize,
    ) -> SharedGttAllocationV1<P, GttCpuWritableV1> {
        let f = &mut self.fixture;
        transitions::allocate_v1(
            &mut f.engine,
            &mut transitions::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            bytes,
            || panic!("unexpected construction preflight failure"),
        )
        .unwrap()
    }

    fn map<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> SharedGttAllocationV1<P, GttGpuAccessibleMutableV1> {
        let f = &mut self.fixture;
        transitions::map_mutable_v1(
            &mut f.engine,
            &mut transitions::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            token,
            || panic!("unexpected construction preflight failure"),
        )
        .unwrap()
    }

    pub(crate) fn code(&mut self) -> Code {
        let token = self.allocate::<ExecutableGttV1>(8192);
        let f = &mut self.fixture;
        let token = transitions::seal_v1(&mut f.engine, token).unwrap();
        let token = transitions::map_executable_v1(
            &mut f.engine,
            &mut transitions::ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            token,
            || panic!("unexpected construction preflight failure"),
        )
        .unwrap();
        self.retain(token)
    }

    pub(crate) fn kernarg(&mut self) -> Kernarg {
        let token = self.allocate::<KernargGttV1>(256);
        let token = self.map(token);
        self.retain(token)
    }

    pub(crate) fn host(&mut self) -> Host {
        let mut token = self.allocate::<HostVisibleCoherentGttV1>(17);
        self.fixture
            .engine
            .with_bytes_mut(&mut token, |bytes| bytes.fill(0x5a))
            .unwrap();
        let token = self.map(token);
        self.retain(token)
    }

    pub(crate) fn device(&mut self) -> Gfx942DeviceMemoryDispatchAuthorityV1 {
        let lease = self
            .fixture
            .engine
            .allocate_device_memory(self.fixture.device.model_key(), self.fixture.vm, 17, 4)
            .unwrap();
        let lease = self.fixture.engine.map_device_memory(lease).unwrap();
        let record = self.fixture.engine.device_memory.last().unwrap();
        assert_eq!(record.id, lease.id);
        Gfx942DeviceMemoryDispatchAuthorityV1 {
            lease,
            facts: Gfx942DeviceMemoryDispatchFactsV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
                gpu_va: record.gpu_va,
                layout: record.layout,
            },
        }
    }

    pub(crate) fn usage(&self) -> (Gfx942HostVisibleBackingUsageV1, Gfx942DeviceBackingUsageV1) {
        (
            self.fixture
                .engine
                .host_backing_account
                .as_ref()
                .unwrap()
                .usage(),
            self.fixture.usage().unwrap(),
        )
    }

    pub(crate) fn retain_data(
        &self,
        data: &mut Vec<crate::queue::dispatch_binding::Gfx942FixedDispatchDataV1>,
    ) -> Result<
        crate::shared_memory::dispatch_retention::RetainedDispatchDataRosterV1,
        MemorySessionError,
    > {
        crate::shared_memory::dispatch_retention::retain_v1(
            &self.fixture.engine,
            self.fixture.device.model_key(),
            self.fixture.vm,
            data,
        )
    }

    pub(crate) fn fail(&mut self, operation: &'static str, panic: bool) {
        if panic {
            self.fixture.engine.backend.panic_operation = Some(operation);
        } else {
            self.fixture.engine.backend.fail_operation = Some(operation);
        }
    }

    pub(crate) fn fail_currentness(&mut self, offset: usize, panic: bool) {
        let call = self.currentness_calls() + offset;
        if panic {
            self.fixture.engine.backend.panic_currentness_at = Some(call);
        } else {
            self.fixture.engine.backend.fail_currentness_at = Some(call);
        }
    }

    pub(crate) fn fail_control(&mut self, ordinal: usize, operation: &'static str, panic: bool) {
        self.control_failure = Some((ordinal, operation, panic));
    }

    pub(crate) fn unmap_control(&mut self, ordinal: usize, progress: u32, errno: bool) {
        self.control_unmap = Some((ordinal, progress, errno));
    }

    pub(crate) fn fail_control_commit(&mut self, ordinal: usize, release: bool, panic: bool) {
        self.projection_fault = Some((
            ordinal,
            if release {
                control_cleanup::CleanupStageV1::ReleaseCommit
            } else {
                control_cleanup::CleanupStageV1::UnmapCommit
            },
            if panic {
                transitions::ProjectionFaultV1::Panic
            } else {
                transitions::ProjectionFaultV1::Error
            },
        ));
    }

    pub(crate) fn exhaust_control_commit(&mut self, ordinal: usize, release: bool) {
        let f = &mut self.fixture;
        f.foundation
            .mint_invariant_certificate(f.engine.session_id, f.device, f.vm)
            .unwrap();
        self.projection_fault = Some((
            ordinal,
            if release {
                control_cleanup::CleanupStageV1::ReleaseCommit
            } else {
                control_cleanup::CleanupStageV1::UnmapCommit
            },
            transitions::ProjectionFaultV1::ExhaustRevision,
        ));
    }

    pub(crate) fn is_quarantined(&self) -> bool {
        self.fixture.engine.phase == SharedMemorySessionPhaseV1::Quarantined
    }

    pub(crate) fn cleanup_call_count(&self) -> usize {
        self.fixture.engine.backend.cleanup_calls.len()
    }

    fn start_control(&mut self) {
        self.control_ordinal += 1;
        if let Some((ordinal, operation, panic)) = self.control_failure
            && ordinal == self.control_ordinal
        {
            self.fail(operation, panic);
        }
        if let Some((ordinal, progress, errno)) = self.control_unmap
            && ordinal == self.control_ordinal
        {
            self.fixture.engine.backend.unmap_progress = progress;
            self.fixture.engine.backend.unmap_errno = errno;
        }
    }

    pub(crate) fn currentness_calls(&self) -> usize {
        self.fixture.engine.backend.currentness_calls
    }
    pub(crate) fn freed(&self) -> usize {
        self.fixture.engine.backend.free_calls
    }
    pub(crate) fn disposed_controls(&self) -> usize {
        self.fixture
            .engine
            .allocations
            .iter()
            .filter(|record| {
                matches!(
                    record.profile,
                    SharedGttProfileV1::Kernarg | SharedGttProfileV1::Executable
                ) && record.phase == SharedAllocationPhaseV1::Released
                    && record.mapping.is_none()
                    && record.handle.is_none()
                    && record.reservation.is_none()
            })
            .count()
    }
    pub(crate) fn native_calls(&self) -> Vec<&'static str> {
        self.fixture.engine.backend.operations.clone()
    }

    pub(crate) fn data_is_retained(&self) -> bool {
        self.fixture
            .engine
            .allocations
            .iter()
            .filter(|record| record.layout.profile == SharedGttProfileV1::HostVisibleCoherent)
            .all(|record| {
                record.phase == SharedAllocationPhaseV1::GpuAccessibleMutable
                    && record.handle.is_some()
                    && record
                        .mapping
                        .as_ref()
                        .is_some_and(|mapping| mapping.bytes[..17] == [0x5a; 17])
            })
            && self.fixture.engine.device_memory.iter().all(|record| {
                record.phase == DeviceMemoryPhaseV1::Mapped && record.handle.is_some()
            })
    }
}

impl PristineControlReleaseV1 for PristineAbortMemoryFixtureV1 {
    fn release_control(
        &mut self,
        custody: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        self.start_control();
        let f = &mut self.fixture;
        let mut projection = control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm);
        projection.fault = self
            .projection_fault
            .filter(|(ordinal, _, _)| *ordinal == self.control_ordinal)
            .map(|(_, stage, fault)| (stage, fault));
        control_cleanup::release_v1(&mut f.engine, &mut projection, custody, || {
            self.process_poisoned += 1
        })
    }
}

impl PristineControlReleaseV1 for super::preparation::PreparationMemoryFixtureV1 {
    fn release_control(
        &mut self,
        custody: &mut ControlCleanupCustodyV1,
    ) -> Result<(), MemorySessionError> {
        let identity = custody.observation().identity;
        let f = &mut self.fixture;
        control_cleanup::release_v1(
            &mut f.engine,
            &mut control_cleanup::ProjectionV1::new(&mut f.foundation, f.vm),
            custody,
            || panic!("unexpected cleanup preflight failure"),
        )?;
        self.disposed_controls.push(identity);
        Ok(())
    }
}

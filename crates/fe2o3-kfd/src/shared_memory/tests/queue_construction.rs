//! Real R88 token/record fixtures for the queue construction handoffs.

use super::*;
use crate::shared_memory::transitions::{self as adapter, ProjectionV1};
use std::any::TypeId;

#[derive(Clone, Copy)]
pub(crate) enum QueueConstructionFaultV1 {
    MapError,
    MapPanic,
    CurrentnessError(usize),
    CurrentnessPanic(usize),
    ProjectionError,
    ProjectionPanic,
    CommitError,
    CommitPanic,
}

pub(crate) struct QueueConstructionMemoryFixtureV1 {
    fixture: BackingConstructorFixture,
    projection_fault: Option<(adapter::TransitionStageV1, adapter::ProjectionFaultV1)>,
}

impl QueueConstructionMemoryFixtureV1 {
    pub(crate) fn new() -> Self {
        let mut fixture = BackingConstructorFixture::with_aperture(
            Some(Gfx942DeviceBackingBudgetV1::new(1 << 20, 16).unwrap()),
            1 << 30,
        );
        fixture
            .engine
            .configure_host_visible_backing_budget_v1(
                fixture.device.model_key(),
                fixture.vm,
                Gfx942HostVisibleBackingBudgetV1::new(1 << 20, 16).unwrap(),
            )
            .unwrap();
        Self {
            fixture,
            projection_fault: None,
        }
    }

    pub(crate) fn device(&self) -> ModelDeviceAdmissionV1 {
        self.fixture.device
    }

    pub(crate) fn charged_device(&mut self) -> Gfx942DeviceMemoryDispatchAuthorityV1 {
        self.fixture.mapped_device()
    }

    pub(crate) fn assert_panic(
        fault: QueueConstructionFaultV1,
        payload: &(dyn std::any::Any + Send),
    ) {
        use QueueConstructionFaultV1::*;
        match fault {
            MapPanic => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "map_gpu"))
            ),
            CurrentnessPanic(_) => assert_eq!(
                payload.downcast_ref::<(&str, &str)>(),
                Some(&("N2 native panic", "currentness"))
            ),
            ProjectionPanic | CommitPanic => {
                let stage = if matches!(fault, ProjectionPanic) {
                    adapter::TransitionStageV1::MapProjection
                } else {
                    adapter::TransitionStageV1::MapCommit
                };
                assert_eq!(
                    payload.downcast_ref::<(&str, adapter::TransitionStageV1)>(),
                    Some(&("session projection", stage))
                );
            }
            _ => panic!("unexpected panic"),
        }
    }

    pub(crate) fn allocate<P: GttProfileV1>(
        &mut self,
        bytes: usize,
    ) -> SharedGttAllocationV1<P, GttCpuWritableV1> {
        let f = &mut self.fixture;
        adapter::allocate_v1(
            &mut f.engine,
            &mut ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            bytes,
            || panic!("unexpected fixture revision exhaustion"),
        )
        .unwrap()
    }

    pub(crate) fn preflight_cpu<P: MutableGpuGttProfileV1>(
        &self,
        token: &SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<(), MemorySessionError> {
        adapter::preflight_borrowed_v1(
            &self.fixture.engine,
            token,
            SharedAllocationPhaseV1::CpuWritable,
        )
    }

    pub(crate) fn preflight_mapped<P: MutableGpuGttProfileV1>(
        &self,
        token: &SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Result<(), MemorySessionError> {
        adapter::preflight_borrowed_v1(
            &self.fixture.engine,
            token,
            SharedAllocationPhaseV1::GpuAccessibleMutable,
        )
    }

    pub(crate) fn map<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>, MemorySessionError> {
        let f = &mut self.fixture;
        let mut projection = ProjectionV1::new(&mut f.foundation, f.device, f.vm);
        projection.fault = self.projection_fault;
        adapter::map_mutable_v1(&mut f.engine, &mut projection, token, || {
            panic!("unexpected fixture revision exhaustion")
        })
    }

    pub(crate) fn retain_ring<P: MutableGpuGttProfileV1>(
        &mut self,
        token: SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Result<
        SharedGttQueueResourceAuthorityV1<AqlRingResourceRoleV1, P, GttGpuAccessibleMutableV1>,
        MemorySessionError,
    > {
        adapter::retain_v1(&mut self.fixture.engine, self.fixture.vm, token)
    }

    pub(crate) fn control(
        &mut self,
        bytes: usize,
        foreign_vm: bool,
    ) -> SharedGttQueueResourceAuthorityV1<
        AqlControlResourceRoleV1,
        UserptrAqlControlGttV1,
        GttGpuAccessibleMutableV1,
    > {
        let token = self.allocate(bytes);
        let token = self.map(token).unwrap();
        let mut authority =
            adapter::retain_v1(&mut self.fixture.engine, self.fixture.vm, token).unwrap();
        if foreign_vm {
            authority.facts.mapping.allocation.vm.id.0 += 1;
        }
        authority
    }

    fn executable<R: SharedGttQueueResourceRoleV1>(
        &mut self,
        bytes: usize,
    ) -> SharedGttQueueResourceAuthorityV1<R, ExecutableGttV1, GttGpuAccessibleExecutableV1> {
        let token = self.allocate(bytes);
        let f = &mut self.fixture;
        let token = adapter::seal_v1(&mut f.engine, token).unwrap();
        let token = adapter::map_executable_v1(
            &mut f.engine,
            &mut ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            token,
            || panic!("unexpected fixture revision exhaustion"),
        )
        .unwrap();
        adapter::retain_v1(&mut f.engine, f.vm, token).unwrap()
    }

    pub(crate) fn eop(
        &mut self,
        bytes: usize,
    ) -> SharedGttQueueResourceAuthorityV1<
        AqlEndOfPipeResourceRoleV1,
        ExecutableGttV1,
        GttGpuAccessibleExecutableV1,
    > {
        self.executable(bytes)
    }

    pub(crate) fn context_save(
        &mut self,
        bytes: usize,
    ) -> SharedGttQueueResourceAuthorityV1<
        AqlContextSaveResourceRoleV1,
        ExecutableGttV1,
        GttGpuAccessibleExecutableV1,
    > {
        self.executable(bytes)
    }

    pub(crate) fn calls(&self) -> [usize; 8] {
        let b = &self.fixture.engine.backend;
        [
            b.currentness_calls,
            b.reserve_va_calls,
            b.alloc_calls,
            b.map_cpu_calls,
            b.map_gpu_calls,
            b.unmap_gpu_calls,
            b.free_calls,
            b.release_va_calls,
        ]
    }

    pub(crate) fn host_usage(&self) -> Gfx942HostVisibleBackingUsageV1 {
        self.fixture
            .engine
            .host_backing_account
            .as_ref()
            .unwrap()
            .usage()
    }

    pub(crate) fn device_usage(&self) -> Gfx942DeviceBackingUsageV1 {
        self.fixture.usage().unwrap()
    }

    pub(crate) fn close(&mut self) {
        self.fixture.engine.phase = SharedMemorySessionPhaseV1::Quarantined;
    }

    pub(crate) fn arm(&mut self, fault: QueueConstructionFaultV1) {
        use QueueConstructionFaultV1::*;
        use adapter::{ProjectionFaultV1 as F, TransitionStageV1 as S};
        let b = &mut self.fixture.engine.backend;
        match fault {
            MapError => {
                b.map_progress = 1;
                b.map_errno = true;
            }
            MapPanic => b.panic_operation = Some("map_gpu"),
            CurrentnessError(offset) => b.fail_currentness_at = Some(b.currentness_calls + offset),
            CurrentnessPanic(offset) => b.panic_currentness_at = Some(b.currentness_calls + offset),
            ProjectionError => self.projection_fault = Some((S::MapProjection, F::Error)),
            ProjectionPanic => self.projection_fault = Some((S::MapProjection, F::Panic)),
            CommitError => self.projection_fault = Some((S::MapCommit, F::Error)),
            CommitPanic => self.projection_fault = Some((S::MapCommit, F::Panic)),
        }
    }

    pub(crate) fn assert_terminal<P: GttProfileV1>(
        &self,
        identity: SharedGttAllocationIdentityV1,
        layout: SharedGttAllocationLayoutV1,
        mapped: bool,
    ) {
        let f = &self.fixture;
        assert_eq!(f.engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        let terminal = f
            .engine
            .terminal_transition
            .as_ref()
            .expect("R88 retained token");
        let (token, absent) = if mapped {
            (terminal.output.as_ref().unwrap(), &terminal.input)
        } else {
            (terminal.input.as_ref().unwrap(), &terminal.output)
        };
        assert!(absent.is_none());
        assert_eq!(
            (token.session_id, token.id, token.generation),
            (identity.session_id, identity.id, identity.generation)
        );
        assert_eq!(token.layout, layout);
        assert_eq!(token.profile_type, TypeId::of::<P>());
        assert_eq!(
            token.state_type,
            if mapped {
                TypeId::of::<GttGpuAccessibleMutableV1>()
            } else {
                TypeId::of::<GttCpuWritableV1>()
            }
        );
        let record = f
            .engine
            .allocations
            .iter()
            .find(|r| r.id == identity.id)
            .unwrap();
        assert!(record.reservation.is_some());
        assert!(record.handle.is_some());
        assert!(record.mapping.as_ref().unwrap().active);
        assert!(!record.free_attempted);
        assert_eq!(&self.calls()[5..], &[0, 0, 0]);
    }

    pub(crate) fn terminal_native_progress(&self) -> (bool, Option<bool>, Option<u32>) {
        let progress = &self
            .fixture
            .engine
            .terminal_transition
            .as_ref()
            .unwrap()
            .progress;
        (
            progress.attempted,
            progress.returned_success,
            progress.returned_map_prefix,
        )
    }
}

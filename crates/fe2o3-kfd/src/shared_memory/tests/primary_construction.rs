//! Extend the original preparation fixture; never replace its session or accounts.

use super::preparation::PreparationMemoryFixtureV1;
use super::*;
use crate::shared_memory::allocation::PendingAllocationStageV1;
use crate::shared_memory::transitions::{self as adapter, ProjectionV1};

impl PreparationMemoryFixtureV1 {
    pub(crate) fn primary_assert_preparation_fault_v1(
        &self,
        call: super::preparation::PreparationMemoryCallV1,
        fault: super::preparation::PreparationNativeFaultV1,
    ) {
        use super::preparation::{
            PreparationMemoryCallV1 as Call, PreparationNativeFaultV1 as Fault,
        };
        use PendingAllocationStageV1 as Stage;
        let e = &self.fixture.engine;
        assert_eq!(e.phase, SharedMemorySessionPhaseV1::Quarantined);
        if matches!(fault, Fault::Projection(_)) {
            match call {
                Call::AllocateCode(_) | Call::MapCode(_) => {
                    self.primary_assert_projection_v1::<ExecutableGttV1>()
                }
                Call::AllocateKernarg | Call::MapKernarg => {
                    self.primary_assert_projection_v1::<KernargGttV1>()
                }
                _ => panic!("projection must select allocation or mapping"),
            }
            return;
        }
        if matches!(call, Call::AllocateCode(_) | Call::AllocateKernarg) {
            if matches!(fault, Fault::ProjectionRejection) {
                assert!(e.pending_allocation.is_none());
                let terminal = e.terminal_transition.as_ref().unwrap();
                assert_eq!(
                    terminal.stage,
                    adapter::TransitionStageV1::AllocationProjection
                );
                assert!(terminal.input.is_none());
                let token = terminal.output.as_ref().unwrap();
                assert_eq!(token.state_type, std::any::TypeId::of::<GttCpuWritableV1>());
                let record = e.allocations.iter().find(|r| r.id == token.id).unwrap();
                assert_eq!(record.generation, token.generation);
                assert_eq!(record.layout, token.layout);
                assert_eq!(record.gpu_va, e.backend.fixed_va.unwrap());
                return;
            }
            if matches!(
                fault,
                Fault::CurrentnessError(1) | Fault::CurrentnessPanic(1)
            ) {
                assert!(e.pending_allocation.is_none());
                assert!(self.primary_terminal_identities().is_empty());
                return;
            }
            let (stage, reservation, output, mapping) = match fault {
                Fault::Error("reserve_va") | Fault::Panic("reserve_va") => {
                    (Stage::ReserveVa, false, false, false)
                }
                Fault::Error("alloc") => (Stage::Allocate, true, true, false),
                Fault::Panic("alloc") => (Stage::Allocate, true, false, false),
                Fault::Error("map_cpu") | Fault::Panic("map_cpu") => {
                    (Stage::MapCpu, true, true, false)
                }
                Fault::Error("prepare_cpu_mapping") | Fault::Panic("prepare_cpu_mapping") => {
                    (Stage::PrepareCpuMapping, true, true, true)
                }
                Fault::CurrentnessError(2) | Fault::CurrentnessPanic(2) => {
                    (Stage::CheckAllocation, true, true, false)
                }
                Fault::CurrentnessError(3) | Fault::CurrentnessPanic(3) => {
                    (Stage::CheckMapping, true, true, true)
                }
                _ => panic!("unexpected allocation fault"),
            };
            let pending = e.pending_allocation.as_ref().unwrap();
            assert_eq!(pending.stage, stage);
            assert_eq!(pending.reservation.is_some(), reservation);
            assert_eq!(pending.allocation_output.is_some(), output);
            assert_eq!(pending.mapping.is_some(), mapping);
            if let Some(raw) = &pending.allocation_output {
                assert_eq!(Some(*raw), e.backend.last_allocation_output);
                assert_eq!(raw.va_addr, pending.reservation.unwrap().0);
                assert_eq!(raw.size, pending.layout.gpu_va_bytes);
            }
            if let Some(mapping) = &pending.mapping {
                assert!(mapping.active);
                assert_eq!(mapping.address, pending.reservation.unwrap().0);
                assert_eq!(mapping.bytes.len(), pending.layout.cpu_mapping_bytes);
            }
            assert!(self.primary_terminal_identities().is_empty());
        } else if matches!(call, Call::MapCode(_) | Call::MapKernarg) {
            let terminal = e.terminal_transition.as_ref().unwrap();
            assert!(terminal.input.is_some() && terminal.output.is_none());
            let token = terminal.input.as_ref().unwrap();
            assert_eq!(
                token.state_type,
                if matches!(call, Call::MapCode(_)) {
                    std::any::TypeId::of::<GttExecutableImmutableV1>()
                } else {
                    std::any::TypeId::of::<GttCpuWritableV1>()
                }
            );
            assert_eq!(
                token.profile_type,
                if matches!(call, Call::MapCode(_)) {
                    std::any::TypeId::of::<ExecutableGttV1>()
                } else {
                    std::any::TypeId::of::<KernargGttV1>()
                }
            );
            let record = e.allocations.iter().find(|r| r.id == token.id).unwrap();
            assert_eq!(
                (token.session_id, token.generation, token.layout),
                (e.session_id, record.generation, record.layout)
            );
            assert_eq!(terminal.stage, adapter::TransitionStageV1::Map);
            let expected = match fault {
                Fault::CurrentnessError(1) | Fault::CurrentnessPanic(1) => (false, None, None),
                Fault::Panic("map_gpu") => (true, None, None),
                Fault::Error("map_gpu") => (true, Some(false), Some(1)),
                Fault::PartialMap(prefix, errno) => (true, Some(!errno), Some(prefix)),
                Fault::CurrentnessError(2) | Fault::CurrentnessPanic(2) => {
                    (true, Some(true), Some(1))
                }
                _ => panic!("unexpected map fault"),
            };
            assert_eq!(
                (
                    terminal.progress.attempted,
                    terminal.progress.returned_success,
                    terminal.progress.returned_map_prefix
                ),
                expected
            );
        } else if matches!(call, Call::SealCode(_)) {
            let terminal = e.terminal_transition.as_ref().unwrap();
            assert!(terminal.input.is_some() && terminal.output.is_none());
            assert_eq!(
                terminal.input.as_ref().unwrap().state_type,
                std::any::TypeId::of::<GttCpuWritableV1>()
            );
        } else {
            assert!(
                e.terminal_transition.is_none(),
                "materialization borrows the rooted CPU token"
            );
        }
    }

    pub(crate) fn primary_align_completion(
        &mut self,
        token: &SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>,
    ) {
        let mapping = self
            .fixture
            .engine
            .allocations
            .iter_mut()
            .find(|r| r.id == token.id)
            .unwrap()
            .mapping
            .as_mut()
            .unwrap();
        assert_eq!(mapping.byte_offset, 0);
        let bytes = mapping.bytes.len();
        mapping
            .bytes
            .resize(bytes + fe2o3_aql::AMD_SIGNAL_ALIGNMENT_V1 - 1, 0);
        let offset = mapping
            .bytes
            .as_ptr()
            .align_offset(fe2o3_aql::AMD_SIGNAL_ALIGNMENT_V1);
        mapping.bytes.copy_within(..bytes, offset);
        mapping.byte_offset = offset;
    }

    pub(crate) fn primary_device(&self) -> ModelDeviceAdmissionV1 {
        self.fixture.device
    }
    pub(crate) fn primary_session_id(&self) -> u64 {
        self.fixture.engine.session_id
    }
    pub(crate) fn primary_currentness(&mut self) -> Result<(), MemorySessionError> {
        self.fixture.engine.require_active()?;
        self.fixture.engine.check_currentness()
    }
    pub(crate) fn primary_write<P: GttProfileV1, R>(
        &mut self,
        token: &mut SharedGttAllocationV1<P, GttCpuWritableV1>,
        f: impl FnOnce(&mut [u8]) -> R,
    ) -> Result<R, MemorySessionError> {
        self.fixture.engine.with_bytes_mut(token, f)
    }
    pub(crate) fn primary_preflight_cpu<P: GttProfileV1>(
        &self,
        token: &SharedGttAllocationV1<P, GttCpuWritableV1>,
    ) -> Result<(), MemorySessionError> {
        adapter::preflight_borrowed_v1(
            &self.fixture.engine,
            token,
            SharedAllocationPhaseV1::CpuWritable,
        )
    }
    pub(crate) fn primary_preflight_mapped<P: MutableGpuGttProfileV1>(
        &self,
        token: &SharedGttAllocationV1<P, GttGpuAccessibleMutableV1>,
    ) -> Result<(), MemorySessionError> {
        adapter::preflight_borrowed_v1(
            &self.fixture.engine,
            token,
            SharedAllocationPhaseV1::GpuAccessibleMutable,
        )
    }
    pub(crate) fn primary_preflight_immutable(
        &self,
        token: &SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>,
    ) -> Result<(), MemorySessionError> {
        adapter::preflight_borrowed_v1(
            &self.fixture.engine,
            token,
            SharedAllocationPhaseV1::ExecutableImmutable,
        )
    }
    pub(crate) fn primary_preflight_executable(
        &self,
        token: &SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
    ) -> Result<(), MemorySessionError> {
        adapter::preflight_borrowed_v1(
            &self.fixture.engine,
            token,
            SharedAllocationPhaseV1::GpuAccessibleExecutable,
        )
    }
    pub(crate) fn primary_seal(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttCpuWritableV1>,
    ) -> Result<SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>, MemorySessionError>
    {
        adapter::seal_v1(&mut self.fixture.engine, token)
    }
    pub(crate) fn primary_map_executable(
        &mut self,
        token: SharedGttAllocationV1<ExecutableGttV1, GttExecutableImmutableV1>,
    ) -> Result<
        SharedGttAllocationV1<ExecutableGttV1, GttGpuAccessibleExecutableV1>,
        MemorySessionError,
    > {
        let fault = self.take_primary_projection_v1();
        let f = &mut self.fixture;
        let mut projection = ProjectionV1::new(&mut f.foundation, f.device, f.vm);
        projection.fault = fault;
        adapter::map_executable_v1(&mut f.engine, &mut projection, token, || {
            panic!("unexpected primary fixture revision exhaustion")
        })
    }
    #[allow(private_bounds)]
    pub(crate) fn primary_retain<R, P, S>(
        &mut self,
        token: SharedGttAllocationV1<P, S>,
    ) -> Result<SharedGttQueueResourceAuthorityV1<R, P, S>, MemorySessionError>
    where
        R: SharedGttQueueResourceRoleV1,
        P: GttProfileV1,
        S: GpuMappedGttStateV1,
    {
        adapter::retain_v1(&mut self.fixture.engine, self.fixture.vm, token)
    }
    pub(crate) fn primary_transfer(
        &mut self,
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
    ) -> Result<QueueModelFoundationV1, MemorySessionError> {
        self.primary_currentness()?;
        self.fixture.transfer(authorities)
    }
    pub(crate) fn primary_authenticate(
        &self,
        foundation: &QueueModelFoundationV1,
    ) -> Result<(), MemorySessionError> {
        let f = &self.fixture;
        let issuer = f
            .ownership
            .queue_owned_issuer()
            .ok_or(MemorySessionError::Model("fixture queue ownership"))?;
        foundation
            .authenticate(f.engine.session_id, f.device, f.vm, issuer)
            .map_err(MemorySessionError::Model)
    }
    pub(crate) fn primary_loan(
        &mut self,
        queue: &mut QueueModelFoundationV1,
    ) -> Result<LiveQueueModelFoundationLoanV1, MemorySessionError> {
        let f = &mut self.fixture;
        f.ownership
            .loan_foundation(
                f.engine.session_id,
                &mut f.foundation,
                queue,
                f.device,
                f.vm,
            )
            .map_err(|()| MemorySessionError::Model("fixture live foundation loan"))
    }
    pub(crate) fn primary_reclaim(
        &mut self,
        queue: &mut QueueModelFoundationV1,
        loan: LiveQueueModelFoundationLoanV1,
    ) -> Result<(), MemorySessionError> {
        let f = &mut self.fixture;
        f.ownership
            .reclaim_foundation(
                f.engine.session_id,
                &mut f.foundation,
                queue,
                f.device,
                f.vm,
                loan,
            )
            .map_err(|()| MemorySessionError::Model("fixture live foundation reclaim"))
    }
    pub(crate) fn primary_arm_native(&mut self, operation: &'static str, panic: bool) {
        if panic {
            self.fixture.engine.backend.panic_operation = Some(operation);
        } else {
            self.fixture.engine.backend.fail_operation = Some(operation);
        }
    }
    pub(crate) fn primary_assert_accounts_and_records(&self, session_id: u64) {
        let f = &self.fixture;
        let e = &f.engine;
        assert_eq!(e.session_id, session_id, "original memory session");
        for record in &e.allocations {
            assert!(
                e.shared_host_backing_charge_matches(record),
                "original Host account and exact charge"
            );
            assert!(!record.free_attempted);
            assert!(record.reservation.is_some() && record.handle.is_some());
            assert!(record.mapping.as_ref().unwrap().active);
        }
        for record in &e.device_memory {
            if let Some(account) = &e.device_backing_account {
                assert!(record.backing_charge.as_ref().unwrap().matches(
                    account,
                    session_id,
                    f.device.model_key(),
                    f.vm,
                    record.id,
                    record.generation,
                    record.layout
                ));
            }
            assert!(!record.free_attempted);
        }
        let b = &e.backend;
        if let Some(pending) = &e.pending_allocation {
            assert_eq!(e.phase, SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(pending.id + 1, e.next_id);
            assert_eq!(
                pending.reservation.is_none(),
                pending.stage == PendingAllocationStageV1::ReserveVa
            );
            assert!(e.allocations.iter().all(|r| r.id != pending.id));
        }
        if let Some(account) = &e.host_backing_account {
            let records: Vec<_> = e
                .allocations
                .iter()
                .filter(|r| r.host_backing_charge.is_some())
                .collect();
            let pending = e.pending_allocation.as_ref().filter(|p| {
                p.profile == SharedGttProfileV1::HostVisibleCoherent
                    && profile_layout::<HostVisibleCoherentGttV1>(p.layout.requested_bytes()).ok()
                        == Some(p.layout)
            });
            let usage = account.usage();
            let backing: u64 = records
                .iter()
                .map(|r| r.layout.cpu_mapping_bytes() as u64)
                .sum();
            assert_eq!(
                usage.used_backing_bytes,
                backing + pending.map_or(0, |p| p.layout.cpu_mapping_bytes() as u64)
            );
            assert_eq!(
                usage.used_allocation_records,
                (records.len() + usize::from(pending.is_some())) as u64
            );
            assert_eq!(usage.reserved_records, 0);
            assert_eq!(usage.retained_records, records.len());
            assert_eq!(usage.quarantined_records, usize::from(pending.is_some()));
        }
        assert_eq!(
            (b.unmap_gpu_calls, b.free_calls, b.release_va_calls),
            (0, 0, 0)
        );
    }
    pub(crate) fn primary_identities(&self) -> Vec<SharedGttAllocationIdentityV1> {
        let e = &self.fixture.engine;
        e.allocations
            .iter()
            .map(|r| SharedGttAllocationIdentityV1 {
                session_id: e.session_id,
                id: r.id,
                generation: r.generation,
            })
            .collect()
    }

    pub(crate) fn primary_assert_device_owners(
        &self,
        authorities: &[&Gfx942DeviceMemoryDispatchAuthorityV1],
    ) {
        let e = &self.fixture.engine;
        assert_eq!(authorities.len(), e.device_memory.len());
        for record in &e.device_memory {
            let expected = Gfx942DeviceMemoryIdentityV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
            };
            let owners: Vec<_> = authorities
                .iter()
                .filter(|a| a.lease.storage_identity() == expected)
                .collect();
            assert_eq!(owners.len(), 1, "exact Device token retained once");
            assert_eq!(owners[0].lease.layout(), record.layout);
            assert_eq!(owners[0].facts.id, record.id);
            assert_eq!(owners[0].facts.generation, record.generation);
            assert_eq!(owners[0].facts.device, record.device);
            assert_eq!(owners[0].facts.vm, record.vm);
            assert_eq!(owners[0].facts.gpu_va, record.gpu_va);
            assert_eq!(owners[0].facts.layout, record.layout);
        }
    }
    pub(crate) fn primary_terminal_identities(&self) -> Vec<SharedGttAllocationIdentityV1> {
        self.fixture
            .engine
            .terminal_transition
            .iter()
            .flat_map(|t| t.input.iter().chain(t.output.iter()))
            .map(|t| SharedGttAllocationIdentityV1 {
                session_id: t.session_id,
                id: t.id,
                generation: t.generation,
            })
            .collect()
    }
    pub(crate) fn primary_token_identity<R, P, S>(
        token: &SharedGttQueueResourceAuthorityV1<R, P, S>,
    ) -> SharedGttAllocationIdentityV1
    where
        R: SharedGttQueueResourceRoleV1,
        P: GttProfileV1,
        S: GttAllocationStateV1,
    {
        token.token.storage_identity()
    }
}

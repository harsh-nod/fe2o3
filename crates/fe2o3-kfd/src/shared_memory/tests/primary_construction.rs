//! Extend the original preparation fixture; never replace its session or accounts.

use super::preparation::PreparationMemoryFixtureV1;
use super::*;
use crate::shared_memory::transitions::{self as adapter, ProjectionV1};

impl PreparationMemoryFixtureV1 {
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
        let f = &mut self.fixture;
        adapter::map_executable_v1(
            &mut f.engine,
            &mut ProjectionV1::new(&mut f.foundation, f.device, f.vm),
            token,
            || panic!("unexpected primary fixture revision exhaustion"),
        )
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
            assert!(pending.reservation.is_some());
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

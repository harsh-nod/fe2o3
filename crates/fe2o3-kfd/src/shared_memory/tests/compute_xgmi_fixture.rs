//! Real engine transitions over two model-only devices; no Linux route admission.

use super::preparation::PreparationMemoryFixtureV1;
use super::*;

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ComputeXgmiMemorySnapshotV1 {
    pub(crate) phase: SharedMemorySessionPhaseV1,
    pub(crate) identities: Vec<Gfx942DeviceMemoryIdentityV1>,
    pub(crate) states: Vec<&'static str>,
    pub(crate) addresses: Vec<u64>,
    pub(crate) physical_bytes: Vec<u64>,
    pub(crate) maps: Vec<(Vec<u32>, u32)>,
    pub(crate) unmaps: Vec<(Vec<u32>, u32)>,
    pub(crate) currentness_calls: usize,
    pub(crate) device_usage: Option<Gfx942DeviceBackingUsageV1>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ComputeXgmiBufferSnapshotV1 {
    pub(crate) identities: Vec<Gfx942DeviceMemoryIdentityV1>,
    pub(crate) slots: [bool; 3],
    pub(crate) peer_prefix: Option<(u32, u32, bool, bool)>,
    pub(crate) progress: [(bool, Option<bool>, Option<u32>); 4],
}

impl ComputeXgmiBufferV1 {
    pub(crate) fn compute_xgmi_identities_v1(&self) -> Vec<Gfx942DeviceMemoryIdentityV1> {
        self.local
            .iter()
            .map(|lease| lease.storage_identity())
            .chain(self.unmapped.iter().map(|lease| lease.storage_identity()))
            .chain(self.peer.iter().map(|peer| peer.lease().storage_identity()))
            .collect()
    }

    pub(crate) fn compute_xgmi_snapshot_v1(&self) -> ComputeXgmiBufferSnapshotV1 {
        ComputeXgmiBufferSnapshotV1 {
            identities: self.compute_xgmi_identities_v1(),
            slots: [
                self.local.is_some(),
                self.unmapped.is_some(),
                self.peer.is_some(),
            ],
            peer_prefix: self.peer.as_ref().map(|peer| {
                (
                    peer.mapped_prefix,
                    peer.unmapped_prefix,
                    peer.map_succeeded,
                    peer.unmap_indeterminate,
                )
            }),
            progress: self.progress.map(|progress| {
                (
                    progress.attempted,
                    progress.returned_success,
                    progress.returned_map_prefix,
                )
            }),
        }
    }
}

impl PreparationMemoryFixtureV1 {
    pub(crate) fn compute_xgmi_v1(gpu_id: u32, va_base: u64, configured: bool) -> Self {
        assert!(matches!(gpu_id, 1001 | 1002));
        assert!(matches!(
            (gpu_id, va_base),
            (1001, 0x1_0000) | (1002, 0x41_0000)
        ));
        let (identity, memory, device, vm) = transferred_model_foundation_with_correlation(
            0x80_0000,
            model_correlation_for_gpu(gpu_id),
        );
        let mut backend = FakeBackend::good();
        backend.gpu_id = gpu_id;
        backend.next_va = va_base;
        let mut fixture = BackingConstructorFixture {
            engine: SharedMemoryEngine::acquire(backend).unwrap(),
            ownership: QueueModelOwnershipV1::new(),
            foundation: QueueModelFoundationV1::uncertified(identity, memory),
            device,
            vm,
        };
        fixture
            .configure(configured.then(|| Gfx942DeviceBackingBudgetV1::new(1 << 20, 64).unwrap()))
            .unwrap();
        if configured {
            fixture
                .engine
                .configure_host_visible_backing_budget_v1(
                    device.model_key(),
                    vm,
                    Gfx942HostVisibleBackingBudgetV1::new(1 << 20, 64).unwrap(),
                )
                .unwrap();
        }
        let mut result = Self::new(configured);
        result.fixture = fixture;
        result
    }

    pub(crate) fn compute_xgmi_lease_v1(
        &mut self,
        physical_bytes: usize,
    ) -> Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1> {
        let f = &mut self.fixture;
        let source = vec![0x6b; physical_bytes].into_boxed_slice();
        let descriptor = content(&source);
        let source = validate_initialization_source(source, descriptor).unwrap();
        let lease = f
            .engine
            .allocate_device_memory_with_flags(
                f.device.model_key(),
                f.vm,
                physical_bytes as u64,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        f.engine
            .initialize_public_device_memory(lease, source)
            .unwrap()
            .lease
    }

    pub(crate) fn compute_xgmi_transition_v1(
        &mut self,
        root: &mut ComputeXgmiBufferV1,
        stage: usize,
    ) -> Result<(), MemorySessionError> {
        if matches!(stage, 1 | 2) {
            root.require_exact_roster([1001, 1002], stage == 1)?;
        }
        crate::shared_memory::compute_xgmi_transition::with_compute_transition(
            &mut self.fixture.engine,
            |engine| match stage {
                0 => root.local_unmap(engine),
                1 => root.peer_map(engine),
                2 => root.peer_unmap(engine),
                3 => root.local_map(engine),
                _ => panic!("unknown compute-XGMI transition"),
            },
        )
    }

    pub(crate) fn compute_xgmi_facts_v1(
        &self,
        mapping: &Gfx942XgmiMappedDeviceMemoryV1,
    ) -> Result<Gfx942DeviceMemoryDispatchFactsV1, MemorySessionError> {
        if !mapping.is_fully_mapped() || mapping.gpu_ids() != [1001, 1002] {
            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);
        }
        let f = &self.fixture;
        f.engine
            .mapped_device_memory_facts_v1(mapping.lease(), f.device.model_key(), f.vm)
    }

    pub(crate) fn compute_xgmi_currentness_fault_v1(&mut self, ordinal: usize, panic: bool) {
        let b = &mut self.fixture.engine.backend;
        let at = b.currentness_calls + ordinal;
        if panic {
            b.panic_currentness_at = Some(at);
        } else {
            b.fail_currentness_at = Some(at);
        }
    }

    pub(crate) fn compute_xgmi_fault_v1(
        &mut self,
        stage: usize,
        prefix: u32,
        errno: bool,
        panic: bool,
    ) {
        let b = &mut self.fixture.engine.backend;
        match stage {
            0 | 3 if panic => {
                b.panic_operation = Some(if stage == 0 { "unmap_gpu" } else { "map_gpu" })
            }
            0 => {
                b.unmap_progress = prefix;
                b.unmap_errno = errno;
            }
            3 => {
                b.map_progress = prefix;
                b.map_errno = errno;
            }
            1 if panic => b.panic_multi_map_at = Some(b.multi_map_inputs.len() + 1),
            2 if panic => b.panic_multi_unmap_at = Some(b.multi_unmap_inputs.len() + 1),
            1 => {
                b.multi_map_script
                    .resize(b.multi_map_inputs.len() + 1, (2, false));
                *b.multi_map_script.last_mut().unwrap() = (prefix, errno);
            }
            2 => {
                b.multi_unmap_script
                    .resize(b.multi_unmap_inputs.len() + 1, (2, false));
                *b.multi_unmap_script.last_mut().unwrap() = (prefix, errno);
            }
            _ => panic!("unknown compute-XGMI transition fault"),
        }
    }

    pub(crate) fn compute_xgmi_snapshot_v1(&self) -> ComputeXgmiMemorySnapshotV1 {
        let e = &self.fixture.engine;
        ComputeXgmiMemorySnapshotV1 {
            phase: e.phase,
            identities: e
                .device_memory
                .iter()
                .map(|r| Gfx942DeviceMemoryIdentityV1 {
                    id: r.id,
                    generation: r.generation,
                    device: r.device,
                    vm: r.vm,
                })
                .collect(),
            states: e
                .device_memory
                .iter()
                .map(|r| match r.phase {
                    DeviceMemoryPhaseV1::Mapped => "mapped",
                    DeviceMemoryPhaseV1::Unmapped => "unmapped",
                    DeviceMemoryPhaseV1::Ambiguous => "ambiguous",
                    DeviceMemoryPhaseV1::Released => "released",
                })
                .collect(),
            addresses: e.device_memory.iter().map(|r| r.gpu_va).collect(),
            physical_bytes: e
                .device_memory
                .iter()
                .map(|r| r.layout.requested_bytes())
                .collect(),
            maps: e.backend.multi_map_inputs.clone(),
            unmaps: e.backend.multi_unmap_inputs.clone(),
            currentness_calls: e.backend.currentness_calls,
            device_usage: e
                .device_backing_account
                .as_ref()
                .map(DeviceBackingAccountV1::usage),
        }
    }
}

#[test]
fn compute_xgmi_composed_fixture_has_distinct_real_identities_and_disjoint_extents() {
    for configured in [false, true] {
        let mut source = PreparationMemoryFixtureV1::compute_xgmi_v1(1001, 0x1_0000, configured);
        let mut destination =
            PreparationMemoryFixtureV1::compute_xgmi_v1(1002, 0x41_0000, configured);
        let a = source.compute_xgmi_lease_v1(4096);
        let b = destination.compute_xgmi_lease_v1(8192);
        assert_ne!(a.storage_identity(), b.storage_identity());
        assert_ne!(
            source.primary_session_id(),
            destination.primary_session_id()
        );
        assert_ne!(source.primary_vm(), destination.primary_vm());
        let source_device = source.primary_device().correlation();
        let destination_device = destination.primary_device().correlation();
        assert_eq!(source_device.kfd_gpu_id(), 1001);
        assert_eq!(destination_device.kfd_gpu_id(), 1002);
        assert_ne!(
            source_device.identity().pci,
            destination_device.identity().pci
        );
        assert_ne!(
            source_device.render_node(),
            destination_device.render_node()
        );
        let sa = source.compute_xgmi_snapshot_v1();
        let sb = destination.compute_xgmi_snapshot_v1();
        assert!(sa.addresses[0] + sa.physical_bytes[0] < sb.addresses[0]);
        assert_eq!([sa.physical_bytes[0], sb.physical_bytes[0]], [4096, 8192]);
        assert_eq!(sa.device_usage.is_some(), configured);
        assert_eq!(sb.device_usage.is_some(), configured);
    }
}

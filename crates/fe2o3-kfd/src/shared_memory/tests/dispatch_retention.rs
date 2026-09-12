//! Actual fake-native records and backing charges, not Linux execution evidence.

use super::*;
use crate::queue::dispatch_binding::{
    DispatchDataAuthorityV1, DispatchDataStorageRefV1, GFX942_MAX_FIXED_DISPATCH_DATA_V1,
    Gfx942FixedDispatchDataLayoutV1, Gfx942FixedDispatchDataV1,
    Gfx942FixedDispatchStorageIdentityV1,
};
use crate::shared_memory::coherent_initialization::{
    CoherentInitializationV1, CpuAllocation, MappedAllocation, initialize_v1,
};
use crate::shared_memory::dispatch_retention::{retain_v1, retain_with_v1};

struct Fixture {
    engine: SharedMemoryEngine<FakeBackend>,
    device: DeviceKeyV1,
    vm: VmKeyV1,
}

impl Fixture {
    fn new(configured: bool) -> Self {
        let mut engine = acquired();
        let (device, vm) = device_vm(1);
        if configured {
            engine
                .configure_device_backing_budget_v1(
                    device,
                    vm,
                    Gfx942DeviceBackingBudgetV1::new(1 << 20, 64).unwrap(),
                )
                .unwrap();
            engine
                .configure_host_visible_backing_budget_v1(
                    device,
                    vm,
                    Gfx942HostVisibleBackingBudgetV1::new(1 << 20, 64).unwrap(),
                )
                .unwrap();
        }
        Self { engine, device, vm }
    }

    fn host(&mut self, initialized: bool) -> Gfx942FixedDispatchDataV1 {
        if initialized {
            Gfx942FixedDispatchDataV1::host_visible_initialized(
                initialize_v1(self, &[0x5a; 17]).unwrap(),
            )
        } else {
            let token = self
                .engine
                .allocate::<HostVisibleCoherentGttV1>(17)
                .unwrap();
            Gfx942FixedDispatchDataV1::host_visible_uninitialized(
                self.engine.map_mutable(token).unwrap(),
            )
        }
    }

    fn device(&mut self, initialized: bool) -> Gfx942FixedDispatchDataV1 {
        if initialized {
            let source = vec![0x6b; 17].into_boxed_slice();
            let descriptor = content(&source);
            let source = validate_initialization_source(source, descriptor).unwrap();
            let lease = self
                .engine
                .allocate_device_memory_with_flags(
                    self.device,
                    self.vm,
                    17,
                    4,
                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
                )
                .unwrap();
            Gfx942FixedDispatchDataV1::initialized(
                self.engine
                    .initialize_public_device_memory(lease, source)
                    .unwrap(),
            )
        } else {
            let lease = self
                .engine
                .allocate_device_memory(self.device, self.vm, 17, 4)
                .unwrap();
            Gfx942FixedDispatchDataV1::uninitialized(self.engine.map_device_memory(lease).unwrap())
        }
    }

    fn roster(&mut self) -> Vec<Gfx942FixedDispatchDataV1> {
        vec![
            self.device(false),
            self.host(true),
            self.device(true),
            self.host(false),
        ]
    }

    fn observation(&self) -> Observation {
        let b = &self.engine.backend;
        Observation {
            host: self.engine.host_backing_account.as_ref().map(|a| a.usage()),
            device: self
                .engine
                .device_backing_account
                .as_ref()
                .map(|a| a.usage()),
            calls: [
                b.currentness_calls,
                b.operational_currentness_calls,
                b.reserve_va_calls,
                b.alloc_calls,
                b.map_cpu_calls,
                b.map_gpu_calls,
                b.unmap_gpu_calls,
                b.free_calls,
                b.release_va_calls,
            ],
            operations: b.operations.clone(),
            records: (
                self.engine.allocations.len(),
                self.engine.device_memory.len(),
            ),
        }
    }
}

impl CoherentInitializationV1 for Fixture {
    fn allocate(&mut self, length: usize) -> Result<CpuAllocation, MemorySessionError> {
        self.engine.allocate(length)
    }
    fn copy(
        &mut self,
        token: CpuAllocation,
        source: &[u8],
    ) -> Result<CpuAllocation, MemorySessionError> {
        crate::shared_memory::transitions::copy_coherent_v1(&mut self.engine, token, source)
    }
    fn map(&mut self, token: CpuAllocation) -> Result<MappedAllocation, MemorySessionError> {
        self.engine.map_mutable(token)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Observation {
    host: Option<Gfx942HostVisibleBackingUsageV1>,
    device: Option<Gfx942DeviceBackingUsageV1>,
    calls: [usize; 9],
    operations: Vec<&'static str>,
    records: (usize, usize),
}

#[derive(Debug, PartialEq, Eq)]
struct InputIdentity {
    storage: Gfx942FixedDispatchStorageIdentityV1,
    layout: Gfx942FixedDispatchDataLayoutV1,
    initialized: bool,
    content: Option<Gfx942DeviceContentDescriptorV1>,
}

fn identities(data: &[Gfx942FixedDispatchDataV1]) -> Vec<InputIdentity> {
    data.iter()
        .map(|input| InputIdentity {
            storage: input.storage_identity(),
            layout: input.layout(),
            initialized: input.is_fully_initialized(),
            content: input.initialized_content(),
        })
        .collect()
}

fn reject_unchanged(f: &Fixture, data: &mut Vec<Gfx942FixedDispatchDataV1>) {
    let original = identities(data);
    let backing = (data.as_ptr(), data.capacity());
    let before = f.observation();
    assert!(retain_v1(&f.engine, f.device, f.vm, data).is_err());
    assert_eq!(identities(data), original);
    assert_eq!((data.as_ptr(), data.capacity()), backing);
    assert_eq!(f.observation(), before);
}

#[test]
fn dispatch_retention_preserves_complete_mixed_roster_and_exact_charges() {
    for configured in [false, true] {
        let mut f = Fixture::new(configured);
        let mut data = f.roster();
        let expected = identities(&data);
        let backing = (data.as_ptr(), data.capacity());
        let before = f.observation();
        let retained = retain_v1(&f.engine, f.device, f.vm, &mut data).unwrap();
        assert!(data.is_empty());
        assert_eq!((data.as_ptr(), data.capacity()), backing);
        assert_eq!(retained.len(), expected.len());
        for (retained, expected) in retained.into_iter().zip(expected) {
            assert_eq!(retained.layout, expected.layout);
            assert_eq!(retained.fully_initialized, expected.initialized);
            assert_eq!(retained.initialized_content, expected.content);
            match (retained.authority, expected.storage) {
                (
                    DispatchDataAuthorityV1::Device(authority),
                    Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(id)
                    | Gfx942FixedDispatchStorageIdentityV1::DeviceInitializedContent(id),
                ) => {
                    assert_eq!(authority.lease.storage_identity(), id);
                    assert_eq!(authority.facts.layout, authority.lease.layout());
                    assert_eq!(authority.facts.vm, f.vm);
                    let index = f
                        .engine
                        .device_memory_index(&authority.lease, DeviceMemoryPhaseV1::Mapped)
                        .unwrap();
                    let record = &f.engine.device_memory[index];
                    assert_eq!(authority.facts.id, record.id);
                    assert_eq!(authority.facts.generation, record.generation);
                    assert_eq!(authority.facts.device, record.device);
                    assert_eq!(authority.facts.gpu_va, record.gpu_va);
                }
                (
                    DispatchDataAuthorityV1::HostVisible(authority),
                    Gfx942FixedDispatchStorageIdentityV1::HostVisibleUninitialized(id)
                    | Gfx942FixedDispatchStorageIdentityV1::HostVisibleInitialized(id),
                ) => {
                    assert_eq!(authority.token.storage_identity(), id);
                    assert_eq!(authority.facts.logical_bytes, 17);
                    assert_eq!(authority.facts.mapping.allocation.vm, f.vm);
                    let index = f
                        .engine
                        .index(
                            &authority.token,
                            SharedAllocationPhaseV1::GpuAccessibleMutable,
                        )
                        .unwrap();
                    let record = &f.engine.allocations[index];
                    assert_eq!(
                        authority.facts.mapping,
                        model_keys(f.vm, record.id, record.generation).2
                    );
                    assert_eq!(authority.facts.publication.mapping, authority.facts.mapping);
                    assert_eq!(
                        authority.facts.publication.id,
                        MemoryPublicationIdV1(record.id)
                    );
                    assert_eq!(authority.facts.gpu_va, record.gpu_va);
                    assert_eq!(
                        authority.facts.cpu_mapping_bytes,
                        record.layout.cpu_mapping_bytes()
                    );
                    assert_eq!(authority.facts.gpu_va_bytes, record.layout.gpu_va_bytes());
                }
                _ => panic!("retention changed storage kind"),
            }
        }
        assert_eq!(f.observation(), before);
        if configured {
            let usage = f.observation();
            assert_eq!(usage.host.unwrap().used_allocation_records, 2);
            assert_eq!(usage.device.unwrap().used_allocation_records, 2);
            assert_eq!(usage.host.unwrap().used_backing_bytes, 8192);
            assert_eq!(usage.device.unwrap().used_backing_bytes, 8192);
        }
    }
}

#[test]
fn dispatch_retention_late_generation_rejection_keeps_original_prefix_and_suffix() {
    for ordinal in 0..4 {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        match data[ordinal].storage_ref() {
            DispatchDataStorageRefV1::Device(lease) => {
                let index = f
                    .engine
                    .device_memory_index(lease, DeviceMemoryPhaseV1::Mapped)
                    .unwrap();
                f.engine.device_memory[index].generation += 1;
            }
            DispatchDataStorageRefV1::HostVisible(token) => {
                let index = f
                    .engine
                    .index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)
                    .unwrap();
                f.engine.allocations[index].generation += 1;
            }
        }
        reject_unchanged(&f, &mut data);
    }
}

#[test]
fn dispatch_retention_wrong_phase_at_every_ordinal_keeps_custody() {
    for ordinal in 0..4 {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        match data[ordinal].storage_ref() {
            DispatchDataStorageRefV1::Device(lease) => {
                let index = f
                    .engine
                    .device_memory_index(lease, DeviceMemoryPhaseV1::Mapped)
                    .unwrap();
                f.engine.device_memory[index].phase = DeviceMemoryPhaseV1::Ambiguous;
            }
            DispatchDataStorageRefV1::HostVisible(token) => {
                let index = f
                    .engine
                    .index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)
                    .unwrap();
                f.engine.allocations[index].phase = SharedAllocationPhaseV1::CpuWritable;
            }
        }
        reject_unchanged(&f, &mut data);
    }
}

#[test]
fn dispatch_retention_missing_native_backing_at_every_ordinal_keeps_custody() {
    for ordinal in 0..4 {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        match data[ordinal].storage_ref() {
            DispatchDataStorageRefV1::Device(lease) => {
                let index = f
                    .engine
                    .device_memory_index(lease, DeviceMemoryPhaseV1::Mapped)
                    .unwrap();
                let handle = f.engine.device_memory[index].handle.take();
                reject_unchanged(&f, &mut data);
                f.engine.device_memory[index].handle = handle;
            }
            DispatchDataStorageRefV1::HostVisible(token) => {
                let index = f
                    .engine
                    .index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)
                    .unwrap();
                let handle = f.engine.allocations[index].handle.take();
                reject_unchanged(&f, &mut data);
                f.engine.allocations[index].handle = handle;
            }
        }
    }
}

#[test]
fn dispatch_retention_missing_charge_at_every_ordinal_keeps_full_debit() {
    for ordinal in 0..4 {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        match data[ordinal].storage_ref() {
            DispatchDataStorageRefV1::Device(lease) => {
                let index = f
                    .engine
                    .device_memory_index(lease, DeviceMemoryPhaseV1::Mapped)
                    .unwrap();
                let charge = f.engine.device_memory[index].backing_charge.take().unwrap();
                reject_unchanged(&f, &mut data);
                f.engine.device_memory[index].backing_charge = Some(charge);
            }
            DispatchDataStorageRefV1::HostVisible(token) => {
                let index = f
                    .engine
                    .index(token, SharedAllocationPhaseV1::GpuAccessibleMutable)
                    .unwrap();
                let charge = f.engine.allocations[index]
                    .host_backing_charge
                    .take()
                    .unwrap();
                reject_unchanged(&f, &mut data);
                f.engine.allocations[index].host_backing_charge = Some(charge);
            }
        }
    }
}

#[test]
fn dispatch_retention_foreign_host_session_and_device_vm_reject_without_moves() {
    for host in [false, true] {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        let mut foreign = Fixture::new(false);
        foreign.vm.id.0 += 1;
        data[2] = if host {
            foreign.host(true)
        } else {
            foreign.device(true)
        };
        let foreign_before = foreign.observation();
        reject_unchanged(&f, &mut data);
        assert_eq!(foreign.observation(), foreign_before);
    }
}

#[test]
fn dispatch_retention_preflight_panic_leaves_complete_original_owner() {
    for ordinal in 0..4 {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        let expected = identities(&data);
        let before = f.observation();
        let backing = (data.as_ptr(), data.capacity());
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = retain_with_v1(&f.engine, f.device, f.vm, &mut data, |index| {
                if index == ordinal {
                    std::panic::panic_any(("data preflight", ordinal));
                }
            });
        }))
        .unwrap_err();
        assert_eq!(
            failure.downcast_ref::<(&str, usize)>(),
            Some(&("data preflight", ordinal))
        );
        assert_eq!(identities(&data), expected);
        assert_eq!((data.as_ptr(), data.capacity()), backing);
        assert_eq!(f.observation(), before);
        assert_eq!(
            retain_v1(&f.engine, f.device, f.vm, &mut data)
                .unwrap()
                .len(),
            4
        );
        assert_eq!(f.observation(), before);
    }
}

#[test]
fn dispatch_retention_wrong_requested_domain_rejects_valid_local_tokens() {
    for host in [false, true] {
        for coordinate in 0..3 {
            let mut f = Fixture::new(true);
            let mut data = vec![if host { f.host(true) } else { f.device(true) }];
            let expected = identities(&data);
            let before = f.observation();
            let mut device = f.device;
            let mut vm = f.vm;
            match coordinate {
                0 => device.generation.0 += 1,
                1 => vm.id.0 += 1,
                _ => {
                    device.generation.0 += 1;
                    vm.device = device;
                }
            }
            assert!(retain_v1(&f.engine, device, vm, &mut data).is_err());
            assert_eq!(identities(&data), expected);
            assert_eq!(f.observation(), before);
            assert_eq!(
                retain_v1(&f.engine, f.device, f.vm, &mut data)
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(f.observation(), before);
        }
    }
}

#[test]
fn dispatch_retention_duplicate_native_storage_ignores_initialization_labels() {
    for host in [false, true] {
        let mut f = Fixture::new(true);
        let mut data = f.roster();
        // Deliberately malformed, test-private aliases over one actual record.
        // Safe production callers cannot clone these linear tokens.
        let duplicate = match data[if host { 1 } else { 2 }].storage_ref() {
            DispatchDataStorageRefV1::HostVisible(token) => {
                Gfx942FixedDispatchDataV1::host_visible_uninitialized(SharedGttAllocationV1 {
                    session_id: token.session_id,
                    id: token.id,
                    generation: token.generation,
                    layout: token.layout,
                    marker: PhantomData,
                })
            }
            DispatchDataStorageRefV1::Device(lease) => {
                Gfx942FixedDispatchDataV1::uninitialized(Gfx942DeviceMemoryLeaseV1 {
                    id: lease.id,
                    generation: lease.generation,
                    device: lease.device,
                    vm: lease.vm,
                    layout: lease.layout,
                    marker: PhantomData,
                })
            }
        };
        assert_ne!(
            duplicate.storage_identity(),
            data[if host { 1 } else { 2 }].storage_identity()
        );
        assert_eq!(
            duplicate.sdma_storage_identity(),
            data[if host { 1 } else { 2 }].sdma_storage_identity()
        );
        data.push(duplicate);
        reject_unchanged(&f, &mut data);
    }
}

#[test]
fn dispatch_retention_capacity_is_bounded_before_validation_or_consumption() {
    let mut f = Fixture::new(true);
    let mut data: Vec<_> = (0..=GFX942_MAX_FIXED_DISPATCH_DATA_V1)
        .map(|_| f.host(false))
        .collect();
    reject_unchanged(&f, &mut data);
    assert!(matches!(
        retain_with_v1(&f.engine, f.device, f.vm, &mut data, |_| panic!(
            "oversized roster reached validation"
        )),
        Err(MemorySessionError::SharedAllocationCapacity {
            maximum: GFX942_MAX_FIXED_DISPATCH_DATA_V1
        })
    ));
    let _overflow = data.pop().expect("one input exceeds the bounded roster");
    let before = f.observation();
    assert_eq!(
        retain_v1(&f.engine, f.device, f.vm, &mut data)
            .unwrap()
            .len(),
        GFX942_MAX_FIXED_DISPATCH_DATA_V1
    );
    assert_eq!(f.observation(), before);
    assert!(
        retain_v1(&f.engine, f.device, f.vm, &mut data)
            .unwrap()
            .is_empty()
    );
    assert_eq!(f.observation(), before);
}

#[test]
fn dispatch_retention_malformed_content_never_downgrades_initialization() {
    let mut f = Fixture::new(true);
    let mut data = f.roster();
    let lease = f
        .engine
        .allocate_device_memory(f.device, f.vm, 17, 4)
        .unwrap();
    let lease = f.engine.map_device_memory(lease).unwrap();
    data[2] = Gfx942FixedDispatchDataV1::initialized(Gfx942InitializedDeviceMemoryV1 {
        lease,
        content: content(&[0x6b; 16]),
    });
    reject_unchanged(&f, &mut data);
    assert!(matches!(
        retain_v1(&f.engine, f.device, f.vm, &mut data),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
}

#[test]
fn dispatch_retention_refunds_only_after_actual_fake_native_disposal() {
    let mut f = Fixture::new(true);
    let mut data = f.roster();
    let retained = retain_v1(&f.engine, f.device, f.vm, &mut data).unwrap();
    for input in retained {
        match input.authority {
            DispatchDataAuthorityV1::Device(authority) => {
                let lease = f.engine.unmap_device_memory(authority.lease).unwrap();
                f.engine.release_device_memory(lease).unwrap();
            }
            DispatchDataAuthorityV1::HostVisible(authority) => {
                let token = f.engine.unmap_mutable(authority.token).unwrap();
                f.engine
                    .release(token, SharedAllocationPhaseV1::CpuWritable)
                    .unwrap();
            }
        }
    }
    let observed = f.observation();
    assert_eq!(observed.host.unwrap().used_allocation_records, 0);
    assert_eq!(observed.host.unwrap().used_backing_bytes, 0);
    assert_eq!(observed.device.unwrap().used_allocation_records, 0);
    assert_eq!(observed.device.unwrap().used_backing_bytes, 0);
    assert_eq!(f.engine.backend.free_calls, 4);
}

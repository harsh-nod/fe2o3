use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use fe2o3_kfd_uapi::{
    KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES, KFD_MAX_QUEUE_SLOTS_PER_PROCESS,
    KFD_MMAP_GPU_ID_HASH_SHIFT, KFD_MMAP_TYPE_DOORBELL, KFD_MMAP_TYPE_SHIFT,
    admit_kfd_aql_queue_ring_size, admit_kfd_queue_percentage, admit_kfd_queue_priority,
};
use fe2o3_runtime_model::*;
use sha2::{Digest, Sha256};

use super::*;

#[path = "queue_initialization_tests.rs"]
mod initialization;

const TEST_KFD_DYNAMIC_MAJOR: u32 = 511;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn domain() -> DeviceObservationDomainIdV1 {
    DeviceObservationDomainIdV1::from_untrusted_digest(digest(1))
}

fn profile() -> DeviceAdmissionProfileV1 {
    DeviceAdmissionProfileV1::gfx942_xnack_minus_spx_nps1_kfd_1_18_drm_3_64_0(
        DeviceAdmissionProfileIdV1::from_untrusted_digest(digest(2)),
        digest(3),
        digest(4),
    )
}

fn correlation() -> ModelCorrelatedDeviceV1 {
    correlation_for(0x6ced_1647_a296_545c, 5)
}

fn correlation_for(gpu_unique_id: u64, bus: u8) -> ModelCorrelatedDeviceV1 {
    let epoch = ObservationEpochV1(9);
    let device_ordinal = u32::from(bus.checked_sub(5).expect("test PCI bus is at least 5"));
    let pci = PciAddressV1 {
        domain: 0,
        bus,
        device: 0,
        function: 0,
    };
    UntrustedDeviceInventoryV1::from_untrusted_observations(
        UntrustedKfdObservationV1 {
            domain_id: domain(),
            epoch,
            node: DeviceNodeV1 {
                major: TEST_KFD_DYNAMIC_MAJOR,
                minor: KFD_DEVICE_MINOR_V1,
            },
            uapi_major: KFD_UAPI_MAJOR_V1,
            uapi_minor: KFD_UAPI_MINOR_V1,
            schema_identity: digest(3),
            xnack: XnackObservationV1::Disabled,
        },
        vec![UntrustedTopologyObservationV1 {
            domain_id: domain(),
            epoch,
            topology_node_id: 2 + device_ordinal,
            kfd_gpu_id: 28_851 + device_ordinal,
            gpu_unique_id,
            drm_render_minor: DRM_RENDER_MIN_MINOR_V1 + device_ordinal,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            target: GpuTargetObservationV1::Gfx942,
            compute_partition: ComputePartitionObservationV1::Spx,
            memory_partition: MemoryPartitionObservationV1::Nps1,
        }],
        vec![UntrustedRenderObservationV1 {
            domain_id: domain(),
            epoch,
            node: DeviceNodeV1 {
                major: DRM_DEVICE_MAJOR_V1,
                minor: DRM_RENDER_MIN_MINOR_V1 + device_ordinal,
            },
            gpu_unique_id,
            pci,
            vendor_id: AMD_PCI_VENDOR_ID_V1,
            device_id: MI300X_PCI_DEVICE_ID_V1,
            pci_revision_id: 0,
            drm_schema_identity: digest(4),
            driver_name: DrmDriverNameObservationV1::Amdgpu,
            drm_major: DRM_DRIVER_MAJOR_V1,
            drm_minor: DRM_DRIVER_MINOR_V1,
            drm_patch: DRM_DRIVER_PATCH_V1,
            acceleration_working: true,
            family: DrmFamilyObservationV1::AmdgpuFamilyAi,
        }],
    )
    .unwrap()
    .correlate_model_only(&profile())
    .unwrap()
}

struct Fixture {
    foundation: QueueModelFoundationV1,
    device: ModelDeviceAdmissionV1,
    vm: ModelVmAdmissionV1,
    next_identity: u64,
}

fn fixture() -> Fixture {
    let identity = DeviceIdentityStateV1::new(domain());
    let (identity, device) = identity
        .register_device_model_only(correlation(), DeviceGenerationV1(1))
        .unwrap();
    let correlated = device.correlation();
    let (identity, vm) = identity
        .register_vm_model_only(
            device,
            UntrustedVmObservationV1 {
                domain_id: domain(),
                device: device.model_key(),
                vm_id: VmIdV1(10),
                kfd_gpu_id: correlated.kfd_gpu_id(),
                render_node: correlated.render_node(),
                pci: correlated.identity().pci,
            },
        )
        .unwrap();
    let memory = MemoryLifecycleStateV1::new_monotonic_non_reusable(domain())
        .next(MemoryTransitionV1::AcquireVm {
            admission: vm,
            mapping_devices: vec![device],
            handle: UntrustedVmHandleObservationV1(100),
            aperture: GpuVaRangeV1 {
                base: 0x1_0000,
                byte_len: 0x20_0000,
            },
        })
        .unwrap();
    let mut foundation = QueueModelFoundationV1::uncertified(identity, memory);
    foundation
        .mint_invariant_certificate(1, device, vm.model_key())
        .unwrap();
    Fixture {
        foundation,
        device,
        vm,
        next_identity: 1_000,
    }
}

impl Fixture {
    fn authority(&mut self, seed: u8) -> FakeAuthority {
        let base = self.next_identity;
        self.next_identity += 1_000;
        let mut memory = self.foundation.memory().clone();
        let mut bindings = Vec::new();
        for index in 0_u64..COMPUTE_AQL_RESOURCE_COUNT_V1 as u64 {
            let reservation = VaReservationKeyV1 {
                vm: self.vm.model_key(),
                id: VaReservationIdV1(base + index),
            };
            let allocation = MemoryAllocationKeyV1 {
                vm: self.vm.model_key(),
                id: AllocationIdV1(base + 100 + index),
                generation: AllocationGenerationV1(1),
            };
            let mapping = MemoryMappingKeyV1 {
                allocation,
                id: MappingIdV1(base + 200 + index),
            };
            memory = memory
                .next(MemoryTransitionV1::ReserveVa {
                    key: reservation,
                    range: GpuVaRangeV1 {
                        base: 0x2_0000 + (base / 1_000) * 0x10_000 + index * MEMORY_PAGE_BYTES_V1,
                        byte_len: MEMORY_PAGE_BYTES_V1,
                    },
                    alignment: MEMORY_PAGE_BYTES_V1,
                })
                .unwrap();
            memory = memory
                .next(MemoryTransitionV1::Allocate {
                    key: allocation,
                    reservation,
                    handle: UntrustedAllocationHandleObservationV1(base + 300 + index),
                    spec: MemoryAllocationSpecV1 {
                        byte_len: MEMORY_PAGE_BYTES_V1,
                        alignment: MEMORY_PAGE_BYTES_V1,
                        kind: MemoryKindV1::QueueStorage,
                        coherence: MemoryCoherenceV1::HostCoherent,
                    },
                })
                .unwrap();
            memory = memory
                .next(MemoryTransitionV1::BeginMap {
                    key: mapping,
                    target_devices: vec![self.device.model_key()],
                    access: MemoryAccessV1::ReadWrite,
                })
                .unwrap();
            memory = memory
                .next(MemoryTransitionV1::ObserveMap {
                    key: mapping,
                    progress: PartialProgressObservationV1 {
                        n_success: 1,
                        status: PartialOperationStatusV1::Succeeded,
                    },
                })
                .unwrap();
            bindings.push(ComputeAqlResourceBindingV1 {
                mapping,
                publication: MemoryPublicationKeyV1 {
                    mapping,
                    id: MemoryPublicationIdV1(base + 400 + index),
                },
                expected_kind: MemoryKindV1::QueueStorage,
                expected_coherence: MemoryCoherenceV1::HostCoherent,
                expected_access: MemoryAccessV1::ReadWrite,
            });
        }
        self.foundation
            .replace_memory_after_sealed_transition(memory)
            .unwrap();
        let queue = QueueKeyV1 {
            vm: self.vm.model_key(),
            id: QueueInstanceIdV1(base + 500),
            generation: QueueGenerationV1(1),
        };
        FakeAuthority(NativeQueueResourceViewV1 {
            plan: ComputeAqlQueuePlanV1 {
                schema_version: QUEUE_LIFECYCLE_SCHEMA_VERSION_V1,
                target: ComputeAqlTargetProfileV1::Gfx942XnackMinusSpxNps1Kfd1_18,
                domain_id: domain(),
                plan_id: QueuePlanIdV1::from_untrusted_digest(digest(seed)),
                current_device: self.device,
                queue,
                initial_configuration: QueueConfigurationIdV1::from_untrusted_digest(digest(
                    seed + 1,
                )),
                resources: ComputeAqlQueueResourcesV1 {
                    ring: bindings[0],
                    control: bindings[1],
                    eop: bindings[2],
                    context_save: bindings[3],
                    private_scratch: None,
                },
            },
            buffers: KfdAqlComputeQueueBuffers {
                ring_base_address: 0x10_0000 + base * 0x100,
                write_pointer_address: 0x20_0000 + base * 0x100,
                read_pointer_address: 0x30_0000 + base * 0x100,
                eop_buffer_address: 0x40_0000 + base * 0x100,
                eop_buffer_size: 4096,
                ctx_save_restore_address: 0x50_0000 + base * 0x100,
                ctx_save_restore_size: 0xb167000,
                ctl_stack_size: 0x18000,
            },
            ring_size: admit_kfd_aql_queue_ring_size(4096).unwrap(),
            initial_percentage: admit_kfd_queue_percentage(100).unwrap(),
            priority: admit_kfd_queue_priority(7).unwrap(),
        })
    }
}

struct FakeAuthority(NativeQueueResourceViewV1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LoggedCall {
    Create(KfdIoctlCreateQueueArgs),
    Update(KfdIoctlUpdateQueueArgs),
    Destroy(KfdIoctlDestroyQueueArgs),
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    CreateZero,
    CreateId(u32),
    CreateDoorbell(u64),
    CreateRingSize,
    UpdateQueueId,
    DestroyQueueId,
}

#[derive(Clone, Copy)]
struct ScriptedOutcome {
    status: QueueSyscallStatusV1,
    mutation: Mutation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BootstrapCallV1 {
    Opener,
    Take,
    Authenticate,
    ResourceView,
}

struct FakeBackend {
    foundation: Option<QueueModelFoundationV1>,
    opener_pid: Rc<Cell<u32>>,
    currentness_calls: usize,
    fail_currentness_at: Option<usize>,
    panic_destroy: bool,
    outcomes: VecDeque<ScriptedOutcome>,
    calls: Rc<RefCell<Vec<LoggedCall>>>,
    bootstrap_calls: Rc<RefCell<Vec<BootstrapCallV1>>>,
    bootstrap_fault: Option<(BootstrapCallV1, bool)>,
    drops: Rc<Cell<usize>>,
}

impl FakeBackend {
    fn new(foundation: QueueModelFoundationV1, outcomes: Vec<ScriptedOutcome>) -> Self {
        Self {
            foundation: Some(foundation),
            opener_pid: Rc::new(Cell::new(std::process::id())),
            currentness_calls: 0,
            fail_currentness_at: None,
            panic_destroy: false,
            outcomes: outcomes.into(),
            calls: Rc::new(RefCell::new(Vec::new())),
            bootstrap_calls: Rc::new(RefCell::new(Vec::new())),
            bootstrap_fault: None,
            drops: Rc::new(Cell::new(0)),
        }
    }

    fn outcome(&mut self) -> ScriptedOutcome {
        self.outcomes.pop_front().expect("missing scripted outcome")
    }

    fn bootstrap(&self, call: BootstrapCallV1) -> Result<(), NativeQueueAdapterErrorV1> {
        self.bootstrap_calls.borrow_mut().push(call);
        if let Some((at, panic)) = self.bootstrap_fault
            && at == call
        {
            if panic {
                std::panic::panic_any(call);
            }
            return Err(NativeQueueAdapterErrorV1::ModelProjection);
        }
        Ok(())
    }
}

impl Drop for FakeBackend {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

impl NativeQueueBackendV1 for FakeBackend {
    type ResourceAuthority = FakeAuthority;

    fn opener_pid(&self) -> u32 {
        self.bootstrap(BootstrapCallV1::Opener)
            .expect("injected opener error");
        self.opener_pid.get()
    }

    fn take_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, NativeQueueAdapterErrorV1> {
        self.bootstrap(BootstrapCallV1::Take)?;
        self.foundation
            .take()
            .ok_or(NativeQueueAdapterErrorV1::ModelProjection)
    }

    fn authenticate_model_foundation(
        &self,
        foundation: &QueueModelFoundationV1,
    ) -> Result<(), NativeQueueAdapterErrorV1> {
        self.bootstrap(BootstrapCallV1::Authenticate)?;
        foundation
            .authenticate_origin()
            .map_err(|_| NativeQueueAdapterErrorV1::ModelProjection)
    }

    fn resource_view(
        &self,
        authority: &Self::ResourceAuthority,
    ) -> Result<NativeQueueResourceViewV1, NativeQueueAdapterErrorV1> {
        self.bootstrap(BootstrapCallV1::ResourceView)?;
        Ok(authority.0)
    }

    fn check_currentness(&mut self) -> Result<(), &'static str> {
        self.currentness_calls += 1;
        if self.fail_currentness_at == Some(self.currentness_calls) {
            Err("scripted currentness loss")
        } else {
            Ok(())
        }
    }

    fn create(
        &mut self,
        args: KfdIoctlCreateQueueArgs,
    ) -> QueueKernelOutcomeV1<KfdIoctlCreateQueueArgs> {
        self.calls.borrow_mut().push(LoggedCall::Create(args));
        let outcome = self.outcome();
        let mut value = args;
        match outcome.mutation {
            Mutation::None => {}
            Mutation::CreateZero => {
                value.queue_id = 0;
                value.doorbell_offset = encoded_doorbell(value.gpu_id, 0);
            }
            Mutation::CreateId(queue_id) => {
                value.queue_id = queue_id;
                value.doorbell_offset = encoded_doorbell(value.gpu_id, 8);
            }
            Mutation::CreateDoorbell(raw) => {
                value.queue_id = 3;
                value.doorbell_offset = raw;
            }
            Mutation::CreateRingSize => {
                value.queue_id = 3;
                value.doorbell_offset = encoded_doorbell(value.gpu_id, 8);
                value.ring_size *= 2;
            }
            _ => panic!("wrong CREATE mutation"),
        }
        QueueKernelOutcomeV1 {
            value,
            status: outcome.status,
        }
    }

    fn update(
        &mut self,
        args: KfdIoctlUpdateQueueArgs,
    ) -> QueueKernelOutcomeV1<KfdIoctlUpdateQueueArgs> {
        self.calls.borrow_mut().push(LoggedCall::Update(args));
        let outcome = self.outcome();
        let mut value = args;
        match outcome.mutation {
            Mutation::None => {}
            Mutation::UpdateQueueId => value.queue_id ^= 1,
            _ => panic!("wrong UPDATE mutation"),
        }
        QueueKernelOutcomeV1 {
            value,
            status: outcome.status,
        }
    }

    fn destroy(
        &mut self,
        args: KfdIoctlDestroyQueueArgs,
    ) -> QueueKernelOutcomeV1<KfdIoctlDestroyQueueArgs> {
        self.calls.borrow_mut().push(LoggedCall::Destroy(args));
        if self.panic_destroy {
            std::panic::panic_any("retained DESTROY panic");
        }
        let outcome = self.outcome();
        let mut value = args;
        match outcome.mutation {
            Mutation::None => {}
            Mutation::DestroyQueueId => value.queue_id ^= 1,
            _ => panic!("wrong DESTROY mutation"),
        }
        QueueKernelOutcomeV1 {
            value,
            status: outcome.status,
        }
    }
}

fn outcome(status: QueueSyscallStatusV1, mutation: Mutation) -> ScriptedOutcome {
    ScriptedOutcome { status, mutation }
}

fn success(mutation: Mutation) -> ScriptedOutcome {
    outcome(QueueSyscallStatusV1::Succeeded, mutation)
}

fn encoded_doorbell(gpu_id: u32, offset: u64) -> u64 {
    (KFD_MMAP_TYPE_DOORBELL << KFD_MMAP_TYPE_SHIFT)
        | ((gpu_id as u64 & 0xffff) << KFD_MMAP_GPU_ID_HASH_SHIFT)
        | offset
}

fn active_engine(tail: Vec<ScriptedOutcome>) -> (NativeQueueEngineV1<FakeBackend>, QueueKeyV1) {
    let mut first_fixture = fixture();
    let authority = first_fixture.authority(10);
    let key = authority.0.plan.queue;
    let mut script = vec![success(Mutation::CreateId(23))];
    script.extend(tail);
    let mut engine =
        NativeQueueEngineV1::new(FakeBackend::new(first_fixture.foundation, script)).unwrap();
    engine.admit(authority).unwrap();
    engine.create(key).unwrap();
    (engine, key)
}

#[path = "queue_tests/certificate_tests.rs"]
mod certificate_tests;
#[path = "queue_tests/currentness_tests.rs"]
mod currentness_tests;

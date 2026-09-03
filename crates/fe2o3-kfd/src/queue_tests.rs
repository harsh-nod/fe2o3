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
    let epoch = ObservationEpochV1(9);
    let pci = PciAddressV1 {
        domain: 0,
        bus: 5,
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
            topology_node_id: 2,
            kfd_gpu_id: 28_851,
            gpu_unique_id: 0x6ced_1647_a296_545c,
            drm_render_minor: DRM_RENDER_MIN_MINOR_V1,
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
                minor: DRM_RENDER_MIN_MINOR_V1,
            },
            gpu_unique_id: 0x6ced_1647_a296_545c,
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
    let memory = MemoryLifecycleStateV1::new(domain())
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
    Fixture {
        foundation: QueueModelFoundationV1 { identity, memory },
        device,
        vm,
        next_identity: 1_000,
    }
}

impl Fixture {
    fn authority(&mut self, seed: u8) -> FakeAuthority {
        let base = self.next_identity;
        self.next_identity += 1_000;
        let mut memory = self.foundation.memory.clone();
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
        self.foundation.memory = memory;
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

struct FakeBackend {
    foundation: Option<QueueModelFoundationV1>,
    opener_pid: Rc<Cell<u32>>,
    currentness_calls: usize,
    fail_currentness_at: Option<usize>,
    outcomes: VecDeque<ScriptedOutcome>,
    calls: Rc<RefCell<Vec<LoggedCall>>>,
}

impl FakeBackend {
    fn new(foundation: QueueModelFoundationV1, outcomes: Vec<ScriptedOutcome>) -> Self {
        Self {
            foundation: Some(foundation),
            opener_pid: Rc::new(Cell::new(std::process::id())),
            currentness_calls: 0,
            fail_currentness_at: None,
            outcomes: outcomes.into(),
            calls: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn outcome(&mut self) -> ScriptedOutcome {
        self.outcomes.pop_front().expect("missing scripted outcome")
    }
}

impl NativeQueueBackendV1 for FakeBackend {
    type ResourceAuthority = FakeAuthority;

    fn opener_pid(&self) -> u32 {
        self.opener_pid.get()
    }

    fn take_model_foundation(
        &mut self,
    ) -> Result<QueueModelFoundationV1, NativeQueueAdapterErrorV1> {
        self.foundation
            .take()
            .ok_or(NativeQueueAdapterErrorV1::ModelProjection)
    }

    fn resource_view(
        &self,
        authority: &Self::ResourceAuthority,
    ) -> Result<NativeQueueResourceViewV1, NativeQueueAdapterErrorV1> {
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

#[test]
fn complete_lifecycle_projects_exact_history_and_releases_only_explicitly() {
    let (mut engine, key) = active_engine(vec![
        success(Mutation::None),
        success(Mutation::None),
        success(Mutation::None),
    ]);
    let configuration = QueueConfigurationIdV1::from_untrusted_digest(digest(40));
    engine
        .update(
            key,
            configuration,
            admit_kfd_queue_percentage(75).unwrap(),
            admit_kfd_queue_priority(9).unwrap(),
        )
        .unwrap();
    engine.disable(key).unwrap();
    engine.destroy(key).unwrap();
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Destroyed));
    assert_eq!(engine.native_queue_id(key), Some(23));
    assert_eq!(engine.model.queues()[0].configuration, configuration);
    let summary = engine.journal_summary();
    assert_eq!(summary.queues, 1);
    assert_eq!(summary.history, 9);
    assert_eq!(summary.live_publications, 4);
    assert_eq!(summary.ambiguous, 0);
    assert!(!summary.authority_poisoned);
    let calls = engine.backend.calls.borrow();
    assert_eq!(calls.len(), 4);
    assert!(matches!(calls[0], LoggedCall::Create(args) if args.queue_id == u32::MAX));
    assert!(
        matches!(calls[1], LoggedCall::Update(args) if args.queue_id == 23 && args.queue_percentage == 75)
    );
    assert!(
        matches!(calls[2], LoggedCall::Update(args) if args.queue_id == 23 && args.ring_base_address == 0 && args.queue_percentage == 0)
    );
    assert!(matches!(calls[3], LoggedCall::Destroy(args) if args.queue_id == 23));
    drop(calls);
    let _authority = engine.release_destroyed_resources(key).unwrap();
    assert_eq!(engine.journal_summary().live_publications, 0);
    let backend = engine.into_backend().unwrap();
    assert_eq!(backend.calls.borrow().len(), 4);
}

#[test]
fn two_queue_keys_remain_independently_active_and_destroyable() {
    let mut fixture = fixture();
    let first = fixture.authority(10);
    let second = fixture.authority(20);
    let first_key = first.0.plan.queue;
    let second_key = second.0.plan.queue;
    let mut engine = NativeQueueEngineV1::new(FakeBackend::new(
        fixture.foundation,
        vec![
            success(Mutation::CreateId(31)),
            success(Mutation::CreateId(32)),
            success(Mutation::None),
            success(Mutation::None),
        ],
    ))
    .unwrap();
    engine.admit(first).unwrap();
    engine.admit(second).unwrap();
    engine.create(first_key).unwrap();
    engine.create(second_key).unwrap();

    assert_eq!(
        engine.phase(first_key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    assert_eq!(
        engine.phase(second_key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    assert_eq!(engine.native_queue_id(first_key), Some(31));
    assert_eq!(engine.native_queue_id(second_key), Some(32));
    assert_eq!(engine.journal_summary().live_publications, 8);

    engine.destroy(second_key).unwrap();
    let _second = engine.release_destroyed_resources(second_key).unwrap();
    assert_eq!(
        engine.phase(first_key),
        Some(ComputeAqlQueuePhaseV1::Active)
    );
    assert_eq!(engine.journal_summary().live_publications, 4);
    engine.destroy(first_key).unwrap();
    let _first = engine.release_destroyed_resources(first_key).unwrap();
    assert_eq!(engine.journal_summary().live_publications, 0);
    engine.into_backend().unwrap();
}

#[test]
fn backend_return_rejects_live_or_unreleased_queue_resources() {
    let (engine, _) = active_engine(Vec::new());
    assert_eq!(
        engine.into_backend().err().unwrap(),
        NativeQueueAdapterErrorV1::InvalidPhase
    );

    let fixture = fixture();
    let engine =
        NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, Vec::new())).unwrap();
    assert_eq!(
        engine.into_backend().err().unwrap(),
        NativeQueueAdapterErrorV1::InvalidPhase
    );
}

#[test]
fn queue_id_zero_and_positive_max_profile_id_are_both_admitted() {
    for (queue_id, mutation) in [
        (0, Mutation::CreateZero),
        (
            KFD_MAX_QUEUE_SLOTS_PER_PROCESS - 1,
            Mutation::CreateId(KFD_MAX_QUEUE_SLOTS_PER_PROCESS - 1),
        ),
    ] {
        let mut fixture = fixture();
        let authority = fixture.authority(10);
        let key = authority.0.plan.queue;
        let backend = FakeBackend::new(fixture.foundation, vec![success(mutation)]);
        let mut engine = NativeQueueEngineV1::new(backend).unwrap();
        engine.admit(authority).unwrap();
        engine.create(key).unwrap();
        assert_eq!(engine.native_queue_id(key), Some(queue_id));
        assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
    }
}

#[test]
fn create_errno_semantics_and_malformed_outputs_fail_closed() {
    let cases = [
        (
            outcome(QueueSyscallStatusV1::Indeterminate, Mutation::None),
            NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Create),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            outcome(QueueSyscallStatusV1::FailedNoEffect, Mutation::None),
            NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Create),
            ComputeAqlQueuePhaseV1::Planned,
        ),
        (
            success(Mutation::None),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateId(KFD_MAX_QUEUE_SLOTS_PER_PROCESS)),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(0)),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(encoded_doorbell(
                28_851,
                KFD_GFX942_PROCESS_DOORBELL_SLICE_BYTES,
            ))),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(encoded_doorbell(28_852, 8))),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateDoorbell(encoded_doorbell(28_851, 1))),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            success(Mutation::CreateRingSize),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE immutable inputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
        (
            outcome(QueueSyscallStatusV1::FailedNoEffect, Mutation::CreateId(11)),
            NativeQueueAdapterErrorV1::MalformedKernelResult(
                NativeQueueOperationV1::Create,
                "CREATE_QUEUE failed-no-effect outputs",
            ),
            ComputeAqlQueuePhaseV1::Ambiguous,
        ),
    ];
    for (script, expected_error, expected_phase) in cases {
        let must_poison = matches!(
            &expected_error,
            NativeQueueAdapterErrorV1::MalformedKernelResult(_, _)
        );
        let mut fixture = fixture();
        let authority = fixture.authority(10);
        let key = authority.0.plan.queue;
        let mut engine =
            NativeQueueEngineV1::new(FakeBackend::new(fixture.foundation, vec![script])).unwrap();
        engine.admit(authority).unwrap();
        assert_eq!(engine.create(key), Err(expected_error));
        assert_eq!(engine.phase(key), Some(expected_phase));
        assert_eq!(engine.journal_summary().live_publications, 4);
        assert_eq!(engine.journal_summary().authority_poisoned, must_poison);
        assert!(engine.release_destroyed_resources(key).is_err());
    }
}

#[test]
fn every_noncreate_operation_classifies_failure_and_mutation_conservatively() {
    for status in [
        QueueSyscallStatusV1::FailedNoEffect,
        QueueSyscallStatusV1::Indeterminate,
    ] {
        let (mut engine, key) = active_engine(vec![outcome(status, Mutation::None)]);
        let error = engine
            .update(
                key,
                QueueConfigurationIdV1::from_untrusted_digest(digest(42)),
                admit_kfd_queue_percentage(50).unwrap(),
                admit_kfd_queue_priority(4).unwrap(),
            )
            .unwrap_err();
        assert_eq!(
            error,
            if status == QueueSyscallStatusV1::FailedNoEffect {
                NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Update)
            } else {
                NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Update)
            }
        );
        assert_eq!(
            engine.phase(key),
            Some(if status == QueueSyscallStatusV1::FailedNoEffect {
                ComputeAqlQueuePhaseV1::Active
            } else {
                ComputeAqlQueuePhaseV1::Ambiguous
            })
        );

        let (mut engine, key) = active_engine(vec![outcome(status, Mutation::None)]);
        let error = engine.disable(key).unwrap_err();
        assert_eq!(
            error,
            if status == QueueSyscallStatusV1::FailedNoEffect {
                NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Disable)
            } else {
                NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Disable)
            }
        );

        let (mut engine, key) = active_engine(vec![
            success(Mutation::None),
            outcome(status, Mutation::None),
        ]);
        engine.disable(key).unwrap();
        let error = engine.destroy(key).unwrap_err();
        assert_eq!(
            error,
            if status == QueueSyscallStatusV1::FailedNoEffect {
                NativeQueueAdapterErrorV1::BackendFailedNoEffect(NativeQueueOperationV1::Destroy)
            } else {
                NativeQueueAdapterErrorV1::BackendIndeterminate(NativeQueueOperationV1::Destroy)
            }
        );
        assert_eq!(engine.journal_summary().live_publications, 4);
    }

    let (mut engine, key) = active_engine(vec![success(Mutation::UpdateQueueId)]);
    assert!(matches!(
        engine.update(
            key,
            QueueConfigurationIdV1::from_untrusted_digest(digest(44)),
            admit_kfd_queue_percentage(50).unwrap(),
            admit_kfd_queue_priority(4).unwrap(),
        ),
        Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
            NativeQueueOperationV1::Update,
            _
        ))
    ));
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));

    let (mut engine, key) = active_engine(vec![
        success(Mutation::None),
        success(Mutation::DestroyQueueId),
    ]);
    engine.disable(key).unwrap();
    assert!(matches!(
        engine.destroy(key),
        Err(NativeQueueAdapterErrorV1::MalformedKernelResult(
            NativeQueueOperationV1::Destroy,
            _
        ))
    ));
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));
}

#[test]
fn update_from_disabled_preserves_exact_resume_phase_on_no_effect() {
    let (mut engine, key) = active_engine(vec![
        success(Mutation::None),
        outcome(QueueSyscallStatusV1::FailedNoEffect, Mutation::None),
    ]);
    engine.disable(key).unwrap();
    let original = engine.model.queues()[0].configuration;
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Disabled));
    assert_eq!(
        engine.update(
            key,
            QueueConfigurationIdV1::from_untrusted_digest(digest(61)),
            admit_kfd_queue_percentage(90).unwrap(),
            admit_kfd_queue_priority(6).unwrap(),
        ),
        Err(NativeQueueAdapterErrorV1::BackendFailedNoEffect(
            NativeQueueOperationV1::Update
        ))
    );
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Disabled));
    assert_eq!(engine.model.queues()[0].configuration, original);

    let (mut engine, key) = active_engine(vec![success(Mutation::None), success(Mutation::None)]);
    engine.disable(key).unwrap();
    let next = QueueConfigurationIdV1::from_untrusted_digest(digest(62));
    engine
        .update(
            key,
            next,
            admit_kfd_queue_percentage(90).unwrap(),
            admit_kfd_queue_priority(6).unwrap(),
        )
        .unwrap();
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
    assert_eq!(engine.model.queues()[0].configuration, next);
}

#[test]
fn cumulative_history_capacity_rejects_before_currentness_or_ioctl() {
    const COMPLETED_UPDATES: usize = 126;
    let tail = vec![success(Mutation::None); COMPLETED_UPDATES];
    let (mut engine, key) = active_engine(tail);
    for index in 0..COMPLETED_UPDATES {
        engine
            .update(
                key,
                QueueConfigurationIdV1::from_untrusted_digest(digest(80 + index as u8)),
                admit_kfd_queue_percentage(50).unwrap(),
                admit_kfd_queue_priority(4).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(engine.model.history().len(), 255);
    let calls = engine.backend.calls.borrow().len();
    let currentness_calls = engine.backend.currentness_calls;
    assert_eq!(
        engine.update(
            key,
            QueueConfigurationIdV1::from_untrusted_digest(digest(79)),
            admit_kfd_queue_percentage(50).unwrap(),
            admit_kfd_queue_priority(4).unwrap(),
        ),
        Err(NativeQueueAdapterErrorV1::JournalCapacity)
    );
    assert_eq!(engine.backend.calls.borrow().len(), calls);
    assert_eq!(engine.backend.currentness_calls, currentness_calls);
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Active));
}

#[test]
fn process_and_currentness_loss_never_issue_or_retry_lifecycle_calls() {
    let mut first_fixture = fixture();
    let authority = first_fixture.authority(10);
    let key = authority.0.plan.queue;
    let backend = FakeBackend::new(
        first_fixture.foundation,
        vec![success(Mutation::CreateId(5))],
    );
    let calls = backend.calls.clone();
    let opener = backend.opener_pid.clone();
    let mut engine = NativeQueueEngineV1::new(backend).unwrap();
    engine.admit(authority).unwrap();
    opener.set(std::process::id().wrapping_add(1));
    assert_eq!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::ProcessChanged)
    );
    assert!(calls.borrow().is_empty());
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));
    assert!(engine.journal_summary().authority_poisoned);
    drop(engine);
    assert!(calls.borrow().is_empty());

    let mut precheck_fixture = fixture();
    let authority = precheck_fixture.authority(12);
    let key = authority.0.plan.queue;
    let mut backend = FakeBackend::new(
        precheck_fixture.foundation,
        vec![success(Mutation::CreateId(7))],
    );
    backend.fail_currentness_at = Some(1);
    let calls = backend.calls.clone();
    let mut engine = NativeQueueEngineV1::new(backend).unwrap();
    engine.admit(authority).unwrap();
    assert!(matches!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::Currentness(_))
    ));
    assert!(calls.borrow().is_empty());
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));

    let mut second_fixture = fixture();
    let authority = second_fixture.authority(11);
    let key = authority.0.plan.queue;
    let mut backend = FakeBackend::new(
        second_fixture.foundation,
        vec![success(Mutation::CreateId(6))],
    );
    backend.fail_currentness_at = Some(2);
    let calls = backend.calls.clone();
    let mut engine = NativeQueueEngineV1::new(backend).unwrap();
    engine.admit(authority).unwrap();
    assert_eq!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::Currentness(
            "scripted currentness loss"
        ))
    );
    assert_eq!(calls.borrow().len(), 1);
    assert_eq!(engine.phase(key), Some(ComputeAqlQueuePhaseV1::Ambiguous));
    assert_eq!(
        engine.create(key),
        Err(NativeQueueAdapterErrorV1::AuthorityPoisoned)
    );
    assert_eq!(calls.borrow().len(), 1);
}

#[test]
fn ambiguous_unknown_id_globally_poisons_create_and_known_id_collision_is_retained() {
    let mut first_fixture = fixture();
    let first = first_fixture.authority(10);
    let second = first_fixture.authority(20);
    let first_key = first.0.plan.queue;
    let second_key = second.0.plan.queue;
    let mut engine = NativeQueueEngineV1::new(FakeBackend::new(
        first_fixture.foundation,
        vec![outcome(QueueSyscallStatusV1::Indeterminate, Mutation::None)],
    ))
    .unwrap();
    engine.admit(first).unwrap();
    engine.admit(second).unwrap();
    assert!(engine.create(first_key).is_err());
    let call_count = engine.backend.calls.borrow().len();
    assert_eq!(
        engine.create(second_key),
        Err(NativeQueueAdapterErrorV1::InvalidPhase)
    );
    assert_eq!(engine.backend.calls.borrow().len(), call_count);

    let mut second_fixture = fixture();
    let first = second_fixture.authority(10);
    let second = second_fixture.authority(20);
    let first_key = first.0.plan.queue;
    let second_key = second.0.plan.queue;
    let mut engine = NativeQueueEngineV1::new(FakeBackend::new(
        second_fixture.foundation,
        vec![
            outcome(QueueSyscallStatusV1::Indeterminate, Mutation::CreateId(31)),
            success(Mutation::CreateId(31)),
        ],
    ))
    .unwrap();
    engine.admit(first).unwrap();
    engine.admit(second).unwrap();
    assert!(engine.create(first_key).is_err());
    assert!(engine.create(second_key).is_err());
    assert_eq!(
        engine.phase(second_key),
        Some(ComputeAqlQueuePhaseV1::Ambiguous)
    );
    assert_eq!(engine.native_queue_id(second_key), None);
    assert_eq!(engine.journal_summary().live_publications, 8);
}

#[test]
fn manifest_digest_is_exact() {
    assert!(
        NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1.contains(&format!(
            "compute_session_sha256={GFX942_COMPUTE_AQL_SESSION_MANIFEST_SHA256_V1}\n"
        ))
    );
    let actual = Sha256::digest(NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_V1.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(actual, NATIVE_QUEUE_ADAPTER_FOUNDATION_MANIFEST_SHA256_V1);
}

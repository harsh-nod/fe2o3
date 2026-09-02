use alloc::{vec, vec::Vec};

use super::*;

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn mapping_observation(ids: Vec<u32>) -> UntrustedNativeMultiDeviceMappingV1 {
    UntrustedNativeMultiDeviceMappingV1 {
        schema_version: R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1,
        operation_identity: digest(1),
        allocation_identity: digest(2),
        kfd_gpu_ids: ids,
    }
}

fn active_mapping() -> ModelNativeMultiDeviceMappingV1 {
    begin_native_multi_device_mapping_model_only_v1(mapping_observation(vec![41, 73]))
        .unwrap()
        .observe_map_cumulative_prefix_model_only_v1(0, 2, NativeProgressStatusV1::Succeeded)
        .unwrap()
}

#[test]
fn canonical_map_and_full_teardown_retain_exact_prefixes() {
    let active = active_mapping();
    assert_eq!(active.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(active.kfd_gpu_ids(), [41, 73]);
    assert_eq!(active.mapped_prefix(), 2);
    assert_eq!(active.unmapped_prefix(), 0);
    assert_eq!(active.phase(), NativeMappingPhaseV1::Active);
    assert!(!active.is_releasable());

    let released = active
        .begin_unmap_model_only_v1()
        .unwrap()
        .observe_unmap_cumulative_prefix_model_only_v1(0, 2, NativeProgressStatusV1::Succeeded)
        .unwrap();
    assert_eq!(released.mapped_prefix(), 2);
    assert_eq!(released.unmapped_prefix(), 2);
    assert_eq!(released.phase(), NativeMappingPhaseV1::Compensated);
    assert!(released.is_releasable());
}

#[test]
fn partial_map_failure_compensates_only_the_exact_mapped_prefix() {
    let partial =
        begin_native_multi_device_mapping_model_only_v1(mapping_observation(vec![41, 73, 91]))
            .unwrap()
            .observe_map_cumulative_prefix_model_only_v1(0, 2, NativeProgressStatusV1::Failed)
            .unwrap();
    assert_eq!(partial.mapped_prefix(), 2);
    assert_eq!(partial.phase(), NativeMappingPhaseV1::Compensating);
    assert!(!partial.is_releasable());

    let still_partial = partial
        .observe_unmap_cumulative_prefix_model_only_v1(0, 1, NativeProgressStatusV1::Failed)
        .unwrap();
    assert_eq!(still_partial.unmapped_prefix(), 1);
    assert!(!still_partial.is_releasable());

    assert_eq!(
        still_partial
            .clone()
            .observe_unmap_cumulative_prefix_model_only_v1(1, 0, NativeProgressStatusV1::Failed,),
        Err(NativeMappingAdmissionErrorV1::NonCumulativePrefix)
    );

    let compensated = still_partial
        .observe_unmap_cumulative_prefix_model_only_v1(1, 2, NativeProgressStatusV1::Succeeded)
        .unwrap();
    assert_eq!(compensated.unmapped_prefix(), 2);
    assert!(compensated.is_releasable());
}

#[test]
fn mapping_rejects_noncanonical_devices_and_inexact_progress() {
    for (ids, expected) in [
        (vec![], NativeMappingAdmissionErrorV1::EmptyDeviceSet),
        (vec![0], NativeMappingAdmissionErrorV1::InvalidGpuId),
        (
            vec![73, 41],
            NativeMappingAdmissionErrorV1::NonCanonicalDeviceSet,
        ),
        (
            vec![41, 41],
            NativeMappingAdmissionErrorV1::NonCanonicalDeviceSet,
        ),
    ] {
        assert_eq!(
            begin_native_multi_device_mapping_model_only_v1(mapping_observation(ids)),
            Err(expected)
        );
    }

    let mapping =
        begin_native_multi_device_mapping_model_only_v1(mapping_observation(vec![41, 73])).unwrap();
    assert_eq!(
        mapping.clone().observe_map_cumulative_prefix_model_only_v1(
            1,
            1,
            NativeProgressStatusV1::Failed,
        ),
        Err(NativeMappingAdmissionErrorV1::NonCumulativePrefix)
    );
    assert_eq!(
        mapping.clone().observe_map_cumulative_prefix_model_only_v1(
            0,
            3,
            NativeProgressStatusV1::Failed,
        ),
        Err(NativeMappingAdmissionErrorV1::ProgressOutOfRange)
    );
    assert_eq!(
        mapping.observe_map_cumulative_prefix_model_only_v1(
            0,
            1,
            NativeProgressStatusV1::Succeeded,
        ),
        Err(NativeMappingAdmissionErrorV1::IncompleteSuccess)
    );
}

#[test]
fn indeterminate_native_progress_is_quarantined() {
    let quarantined =
        begin_native_multi_device_mapping_model_only_v1(mapping_observation(vec![41, 73]))
            .unwrap()
            .observe_map_cumulative_prefix_model_only_v1(
                0,
                1,
                NativeProgressStatusV1::Indeterminate,
            )
            .unwrap();
    assert_eq!(quarantined.mapped_prefix(), 1);
    assert_eq!(quarantined.phase(), NativeMappingPhaseV1::Quarantined);
    assert!(!quarantined.is_releasable());
}

#[test]
fn failed_full_unmap_progress_remains_quarantined_and_unreleasable() {
    let quarantined = active_mapping()
        .begin_unmap_model_only_v1()
        .unwrap()
        .observe_unmap_cumulative_prefix_model_only_v1(0, 2, NativeProgressStatusV1::Failed)
        .unwrap();
    assert_eq!(quarantined.mapped_prefix(), 2);
    assert_eq!(quarantined.unmapped_prefix(), 2);
    assert_eq!(quarantined.phase(), NativeMappingPhaseV1::Quarantined);
    assert!(!quarantined.is_releasable());
}

fn device(physical: u64, generation: u64) -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(physical),
        generation: DeviceGenerationV1(generation),
    }
}

fn route_observation() -> UntrustedNativeXgmiRouteObservationV1 {
    UntrustedNativeXgmiRouteObservationV1 {
        schema_version: R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1,
        route_identity: digest(10),
        topology_identity: digest(11),
        topology_generation: 12,
        observation_epoch: ObservationEpochV1(13),
        source_device: device(0x100, 3),
        destination_device: device(0x200, 4),
        source_kfd_gpu_id: 73,
        destination_kfd_gpu_id: 41,
        source_node_id: 5,
        destination_node_id: 6,
        hive_id: 7,
        io_link_index: 0,
        link_type: KFD_XGMI_LINK_TYPE_V1,
        min_bandwidth: 32_000,
        max_bandwidth: 64_000,
        recommended_transfer_size: 4 * 1_024 * 1_024,
        recommended_sdma_engine_id_mask: 1 << 3,
        selected_sdma_engine_id: 3,
        link_flags: KFD_XGMI_LINK_ENABLED_FLAG_V1,
        peer_access_supported: true,
        sdma_xgmi_queue_supported: true,
    }
}

fn route_currentness(
    route: UntrustedNativeXgmiRouteObservationV1,
) -> UntrustedNativeXgmiCurrentnessV1 {
    UntrustedNativeXgmiCurrentnessV1 {
        route_identity: route.route_identity,
        topology_identity: route.topology_identity,
        topology_generation: route.topology_generation,
        observation_epoch: route.observation_epoch,
        source_device: route.source_device,
        destination_device: route.destination_device,
        source_kfd_gpu_id: route.source_kfd_gpu_id,
        destination_kfd_gpu_id: route.destination_kfd_gpu_id,
        source_node_id: route.source_node_id,
        destination_node_id: route.destination_node_id,
        hive_id: route.hive_id,
        io_link_index: route.io_link_index,
        link_type: route.link_type,
        min_bandwidth: route.min_bandwidth,
        max_bandwidth: route.max_bandwidth,
        recommended_transfer_size: route.recommended_transfer_size,
        recommended_sdma_engine_id_mask: route.recommended_sdma_engine_id_mask,
        selected_sdma_engine_id: route.selected_sdma_engine_id,
        link_flags: route.link_flags,
        reset_fence_current: true,
    }
}

#[test]
fn directional_xgmi_route_requires_exact_current_topology_and_mapping() {
    let observation = route_observation();
    let admitted = admit_native_xgmi_route_model_only_v1(
        &active_mapping(),
        observation,
        route_currentness(observation),
    )
    .unwrap();
    assert_eq!(admitted.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(admitted.source_device(), observation.source_device);
    assert_eq!(
        admitted.destination_device(),
        observation.destination_device
    );
    assert_eq!(admitted.observation(), observation);

    let mut reversed = route_currentness(observation);
    core::mem::swap(
        &mut reversed.source_device,
        &mut reversed.destination_device,
    );
    core::mem::swap(
        &mut reversed.source_kfd_gpu_id,
        &mut reversed.destination_kfd_gpu_id,
    );
    assert_eq!(
        admit_native_xgmi_route_model_only_v1(&active_mapping(), observation, reversed),
        Err(NativeXgmiRouteAdmissionErrorV1::CurrentnessMismatch)
    );
}

#[test]
fn xgmi_route_rejects_every_stale_or_unavailable_surface() {
    let valid = route_observation();
    let mapping = active_mapping();

    let mut stale = route_currentness(valid);
    stale.topology_generation += 1;
    assert_eq!(
        admit_native_xgmi_route_model_only_v1(&mapping, valid, stale),
        Err(NativeXgmiRouteAdmissionErrorV1::CurrentnessMismatch)
    );

    let mut reset_lost = route_currentness(valid);
    reset_lost.reset_fence_current = false;
    assert_eq!(
        admit_native_xgmi_route_model_only_v1(&mapping, valid, reset_lost),
        Err(NativeXgmiRouteAdmissionErrorV1::ResetFenceNotCurrent)
    );

    let mut engine_changed = route_currentness(valid);
    engine_changed.selected_sdma_engine_id = 4;
    assert_eq!(
        admit_native_xgmi_route_model_only_v1(&mapping, valid, engine_changed),
        Err(NativeXgmiRouteAdmissionErrorV1::CurrentnessMismatch)
    );

    let unsupported = UntrustedNativeXgmiRouteObservationV1 {
        sdma_xgmi_queue_supported: false,
        ..valid
    };
    assert_eq!(
        admit_native_xgmi_route_model_only_v1(
            &mapping,
            unsupported,
            route_currentness(unsupported),
        ),
        Err(NativeXgmiRouteAdmissionErrorV1::XgmiQueueUnavailable)
    );

    let invalid_engine = UntrustedNativeXgmiRouteObservationV1 {
        recommended_sdma_engine_id_mask: 1 << 3,
        selected_sdma_engine_id: 4,
        ..valid
    };
    assert_eq!(
        admit_native_xgmi_route_model_only_v1(
            &mapping,
            invalid_engine,
            route_currentness(invalid_engine),
        ),
        Err(NativeXgmiRouteAdmissionErrorV1::InvalidEngineSelection)
    );
}

#[derive(Clone)]
struct EvidenceFixture {
    semantics: ModelGfx942KernelSemanticsV1,
    attestation: UntrustedMachineCodeEvidenceAttestationV1,
    loaded: UntrustedLoadedMachineCodeObservationV1,
}

fn advance(state: RuntimeStateV1, transition: RuntimeTransitionV1) -> RuntimeStateV1 {
    state.next(transition).unwrap()
}

fn evidence_fixture() -> EvidenceFixture {
    let target_device = device(0x942, 3);
    let vm = VmKeyV1 {
        device: target_device,
        id: VmIdV1(7),
    };
    let mut runtime = advance(
        RuntimeStateV1::new(),
        RuntimeTransitionV1::AddDevice { key: target_device },
    );
    runtime = advance(runtime, RuntimeTransitionV1::CreateVm { key: vm });
    let mut mappings = Vec::new();
    for index in 0_u64..4 {
        let allocation = AllocationKeyV1 {
            vm,
            id: AllocationIdV1(index + 1),
        };
        let mapping = MappingKeyV1 {
            allocation,
            id: MappingIdV1(index + 1),
        };
        runtime = advance(
            runtime,
            RuntimeTransitionV1::Allocate {
                key: allocation,
                byte_len: 4_096,
            },
        );
        runtime = advance(
            runtime,
            RuntimeTransitionV1::Map {
                key: mapping,
                allocation_offset: 0,
                gpu_va: 0x1000_0000 + index * 0x1_0000,
                byte_len: 4_096,
                access: if index == 0 {
                    MemoryAccessV1::ReadExecute
                } else {
                    MemoryAccessV1::ReadWrite
                },
            },
        );
        mappings.push(mapping);
    }
    let artifact = RuntimeArtifactIdV1::from_untrusted_digest(digest(20));
    let loaded_code = LoadedCodeKeyV1 {
        vm,
        id: LoadedCodeIdV1(9),
    };
    runtime = advance(
        runtime,
        RuntimeTransitionV1::LoadCode {
            key: loaded_code,
            load_plan_id: CodeLoadPlanIdV1::from_untrusted_digest(digest(21)),
            artifact_id: artifact,
            executable_mapping: mappings[0],
            entry_offset: 64,
        },
    );
    let resources = AsyncOperationResourcesV1::new(
        loaded_code,
        mappings[1],
        mappings[2],
        vec![DispatchResourceV1 {
            mapping: mappings[3],
            required_access: MemoryAccessV1::ReadWrite,
        }],
    )
    .unwrap();
    let contract_identity = digest(22);
    let kernel_identity = digest(23);
    let semantics = admit_gfx942_kernel_semantics_model_only_v1(
        &runtime,
        &resources,
        UntrustedGfx942KernelSemanticContractV1 {
            schema_version: GFX942_KERNEL_SEMANTICS_SCHEMA_VERSION_V1,
            contract_identity,
            device: target_device,
            artifact,
            kernel_identity,
            operations: vec![Gfx942KernelSemanticOperationV1::Atomic(
                Gfx942AtomicSemanticV1 {
                    operation_id: 1,
                    operation: Gfx942AtomicOperationV1::FetchAdd,
                    width: Gfx942AtomicWidthV1::Bits32,
                    storage: Gfx942AtomicStorageV1::Global {
                        mapping: mappings[3],
                        byte_offset: 0,
                        coherence: DeclaredAtomicCoherenceV1::SystemCoherent,
                    },
                    scope: Gfx942AtomicScopeV1::System,
                    success_ordering: Gfx942MemoryOrderingV1::AcquireRelease,
                    failure_ordering: None,
                },
            )],
        },
    )
    .unwrap();
    let target = Gfx942Cov6MachineTargetV1::exact_v1();
    let attestation = UntrustedMachineCodeEvidenceAttestationV1 {
        schema_version: R9_NATIVE_EVIDENCE_SCHEMA_VERSION_V1,
        attestation_identity: digest(24),
        artifact,
        target,
        kernel_symbol_identity: digest(25),
        kernel_descriptor_digest: digest(26),
        machine_code_digest: digest(27),
        checked_instruction_class_receipt_digest: digest(28),
        semantic_contract_identity: contract_identity,
        kernel_identity,
        toolchain_identity: digest(29),
    };
    let loaded = UntrustedLoadedMachineCodeObservationV1 {
        loaded_code,
        device: target_device,
        artifact,
        target,
        kernel_symbol_identity: attestation.kernel_symbol_identity,
        kernel_descriptor_digest: attestation.kernel_descriptor_digest,
        machine_code_digest: attestation.machine_code_digest,
        checked_instruction_class_receipt_digest: attestation
            .checked_instruction_class_receipt_digest,
    };
    EvidenceFixture {
        semantics,
        attestation,
        loaded,
    }
}

#[test]
fn exact_machine_code_evidence_binds_to_the_admitted_semantics() {
    let fixture = evidence_fixture();
    let binding = bind_machine_code_evidence_model_only_v1(
        &fixture.semantics,
        fixture.attestation,
        fixture.loaded,
    )
    .unwrap();
    assert_eq!(binding.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(binding.attestation(), fixture.attestation);
    assert_eq!(binding.loaded(), fixture.loaded);
}

#[test]
fn machine_code_binding_rejects_each_substituted_coordinate() {
    let fixture = evidence_fixture();
    let cases = [
        (
            UntrustedLoadedMachineCodeObservationV1 {
                loaded_code: LoadedCodeKeyV1 {
                    id: LoadedCodeIdV1(fixture.loaded.loaded_code.id.0 + 1),
                    ..fixture.loaded.loaded_code
                },
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::LoadedCodeMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                device: DeviceKeyV1 {
                    generation: DeviceGenerationV1(fixture.loaded.device.generation.0 + 1),
                    ..fixture.loaded.device
                },
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::DeviceMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(90)),
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::ArtifactMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                target: Gfx942Cov6MachineTargetV1 {
                    code_object_version: 5,
                    ..fixture.loaded.target
                },
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::TargetMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                kernel_symbol_identity: digest(90),
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::SymbolMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                kernel_descriptor_digest: digest(90),
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::DescriptorMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                machine_code_digest: digest(90),
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::MachineCodeMismatch,
        ),
        (
            UntrustedLoadedMachineCodeObservationV1 {
                checked_instruction_class_receipt_digest: digest(90),
                ..fixture.loaded
            },
            MachineCodeEvidenceBindingErrorV1::InstructionClassReceiptMismatch,
        ),
    ];
    for (loaded, expected) in cases {
        assert_eq!(
            bind_machine_code_evidence_model_only_v1(
                &fixture.semantics,
                fixture.attestation,
                loaded,
            ),
            Err(expected)
        );
    }

    let wrong_target = UntrustedMachineCodeEvidenceAttestationV1 {
        target: Gfx942Cov6MachineTargetV1 {
            wavefront_size: 32,
            ..fixture.attestation.target
        },
        ..fixture.attestation
    };
    assert_eq!(
        bind_machine_code_evidence_model_only_v1(&fixture.semantics, wrong_target, fixture.loaded,),
        Err(MachineCodeEvidenceBindingErrorV1::UnsupportedTarget)
    );

    for (attestation, expected) in [
        (
            UntrustedMachineCodeEvidenceAttestationV1 {
                schema_version: 0,
                ..fixture.attestation
            },
            MachineCodeEvidenceBindingErrorV1::InvalidSchema,
        ),
        (
            UntrustedMachineCodeEvidenceAttestationV1 {
                attestation_identity: digest(0),
                ..fixture.attestation
            },
            MachineCodeEvidenceBindingErrorV1::InvalidIdentity,
        ),
        (
            UntrustedMachineCodeEvidenceAttestationV1 {
                semantic_contract_identity: digest(90),
                ..fixture.attestation
            },
            MachineCodeEvidenceBindingErrorV1::SemanticContractMismatch,
        ),
        (
            UntrustedMachineCodeEvidenceAttestationV1 {
                kernel_identity: digest(90),
                ..fixture.attestation
            },
            MachineCodeEvidenceBindingErrorV1::KernelIdentityMismatch,
        ),
        (
            UntrustedMachineCodeEvidenceAttestationV1 {
                artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(90)),
                ..fixture.attestation
            },
            MachineCodeEvidenceBindingErrorV1::ArtifactMismatch,
        ),
    ] {
        assert_eq!(
            bind_machine_code_evidence_model_only_v1(
                &fixture.semantics,
                attestation,
                fixture.loaded,
            ),
            Err(expected)
        );
    }
}

fn dispatch_currentness(
    binding: ModelMachineCodeEvidenceBindingV1,
) -> UntrustedMachineCodeDispatchCurrentnessV1 {
    let attestation = binding.attestation();
    let loaded = binding.loaded();
    UntrustedMachineCodeDispatchCurrentnessV1 {
        dispatch_identity: digest(40),
        loaded_code: loaded.loaded_code,
        device: loaded.device,
        artifact: loaded.artifact,
        target: loaded.target,
        attestation_identity: attestation.attestation_identity,
        kernel_symbol_identity: loaded.kernel_symbol_identity,
        kernel_descriptor_digest: loaded.kernel_descriptor_digest,
        machine_code_digest: loaded.machine_code_digest,
        checked_instruction_class_receipt_digest: loaded.checked_instruction_class_receipt_digest,
        semantic_contract_identity: attestation.semantic_contract_identity,
        kernel_identity: attestation.kernel_identity,
        toolchain_identity: attestation.toolchain_identity,
        device_current: true,
        code_current: true,
        mappings_current: true,
        queue_current: true,
        reset_fence_current: true,
        dependency_frontier: 7,
        completed_frontier: 7,
    }
}

#[test]
fn dispatch_requires_exact_current_binding_and_complete_dependencies() {
    let fixture = evidence_fixture();
    let binding = bind_machine_code_evidence_model_only_v1(
        &fixture.semantics,
        fixture.attestation,
        fixture.loaded,
    )
    .unwrap();
    let current = dispatch_currentness(binding);
    let dispatch = admit_machine_code_dispatch_model_only_v1(binding, current).unwrap();
    assert_eq!(dispatch.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(dispatch.dispatch_identity(), current.dispatch_identity);
    assert_eq!(dispatch.binding(), binding);
    assert_eq!(dispatch.dependency_frontier(), current.dependency_frontier);

    for rejected in [
        UntrustedMachineCodeDispatchCurrentnessV1 {
            device_current: false,
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            code_current: false,
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            mappings_current: false,
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            queue_current: false,
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            reset_fence_current: false,
            ..current
        },
    ] {
        assert_eq!(
            admit_machine_code_dispatch_model_only_v1(binding, rejected),
            Err(MachineCodeDispatchAdmissionErrorV1::EvidenceNotCurrent)
        );
    }

    let incomplete = UntrustedMachineCodeDispatchCurrentnessV1 {
        dependency_frontier: 8,
        completed_frontier: 7,
        ..current
    };
    assert_eq!(
        admit_machine_code_dispatch_model_only_v1(binding, incomplete),
        Err(MachineCodeDispatchAdmissionErrorV1::DependencyIncomplete)
    );

    for substituted in [
        UntrustedMachineCodeDispatchCurrentnessV1 {
            loaded_code: LoadedCodeKeyV1 {
                id: LoadedCodeIdV1(current.loaded_code.id.0 + 1),
                ..current.loaded_code
            },
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            device: DeviceKeyV1 {
                generation: DeviceGenerationV1(current.device.generation.0 + 1),
                ..current.device
            },
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(99)),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            target: Gfx942Cov6MachineTargetV1 {
                wavefront_size: 32,
                ..current.target
            },
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            attestation_identity: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            kernel_symbol_identity: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            kernel_descriptor_digest: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            machine_code_digest: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            checked_instruction_class_receipt_digest: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            semantic_contract_identity: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            kernel_identity: digest(99),
            ..current
        },
        UntrustedMachineCodeDispatchCurrentnessV1 {
            toolchain_identity: digest(99),
            ..current
        },
    ] {
        assert_eq!(
            admit_machine_code_dispatch_model_only_v1(binding, substituted),
            Err(MachineCodeDispatchAdmissionErrorV1::BindingMismatch)
        );
    }
}

use alloc::{vec, vec::Vec};

use super::*;

#[derive(Clone, Copy)]
struct Fixture {
    device: DeviceKeyV1,
    artifact: RuntimeArtifactIdV1,
    data: MappingKeyV1,
}

fn digest(seed: u8) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes([seed; IDENTITY_DIGEST_BYTES_V1])
}

fn advance(state: RuntimeStateV1, transition: RuntimeTransitionV1) -> RuntimeStateV1 {
    state.next(transition).unwrap()
}

fn fixture() -> (RuntimeStateV1, AsyncOperationResourcesV1, Fixture) {
    fixture_with_data_gpu_va(0x1003_0000)
}

fn fixture_with_data_gpu_va(
    data_gpu_va: u64,
) -> (RuntimeStateV1, AsyncOperationResourcesV1, Fixture) {
    let device = DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(0x942),
        generation: DeviceGenerationV1(3),
    };
    let vm = VmKeyV1 {
        device,
        id: VmIdV1(7),
    };
    let mut runtime = advance(
        RuntimeStateV1::new(),
        RuntimeTransitionV1::AddDevice { key: device },
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
                gpu_va: if index == 3 {
                    data_gpu_va
                } else {
                    0x1000_0000 + index * 0x1_0000
                },
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
    let artifact = RuntimeArtifactIdV1::from_untrusted_digest(digest(0x30));
    let code = LoadedCodeKeyV1 {
        vm,
        id: LoadedCodeIdV1(9),
    };
    runtime = advance(
        runtime,
        RuntimeTransitionV1::LoadCode {
            key: code,
            load_plan_id: CodeLoadPlanIdV1::from_untrusted_digest(digest(0x31)),
            artifact_id: artifact,
            executable_mapping: mappings[0],
            entry_offset: 64,
        },
    );
    let resources = AsyncOperationResourcesV1::new(
        code,
        mappings[1],
        mappings[2],
        vec![DispatchResourceV1 {
            mapping: mappings[3],
            required_access: MemoryAccessV1::ReadWrite,
        }],
    )
    .unwrap();
    (
        runtime,
        resources,
        Fixture {
            device,
            artifact,
            data: mappings[3],
        },
    )
}

fn contract(
    fixture: Fixture,
    operations: Vec<Gfx942KernelSemanticOperationV1>,
) -> UntrustedGfx942KernelSemanticContractV1 {
    UntrustedGfx942KernelSemanticContractV1 {
        schema_version: GFX942_KERNEL_SEMANTICS_SCHEMA_VERSION_V1,
        contract_identity: digest(0x40),
        device: fixture.device,
        artifact: fixture.artifact,
        kernel_identity: digest(0x41),
        operations,
    }
}

fn global_atomic(
    fixture: Fixture,
    operation_id: u16,
    operation: Gfx942AtomicOperationV1,
) -> Gfx942KernelSemanticOperationV1 {
    let (success, failure) = match operation {
        Gfx942AtomicOperationV1::Load => (Gfx942MemoryOrderingV1::Acquire, None),
        Gfx942AtomicOperationV1::Store => (Gfx942MemoryOrderingV1::Release, None),
        Gfx942AtomicOperationV1::CompareExchange => (
            Gfx942MemoryOrderingV1::AcquireRelease,
            Some(Gfx942MemoryOrderingV1::Acquire),
        ),
        _ => (Gfx942MemoryOrderingV1::Relaxed, None),
    };
    Gfx942KernelSemanticOperationV1::Atomic(Gfx942AtomicSemanticV1 {
        operation_id,
        operation,
        width: if operation_id.is_multiple_of(2) {
            Gfx942AtomicWidthV1::Bits64
        } else {
            Gfx942AtomicWidthV1::Bits32
        },
        storage: Gfx942AtomicStorageV1::Global {
            mapping: fixture.data,
            byte_offset: u64::from(operation_id) * 16,
            coherence: DeclaredAtomicCoherenceV1::SystemCoherent,
        },
        scope: Gfx942AtomicScopeV1::System,
        success_ordering: success,
        failure_ordering: failure,
    })
}

fn collective(
    operation_id: u16,
    operation: Gfx942CollectiveOperationV1,
    element: Gfx942CollectiveElementV1,
    operation_width_or_workgroup_size: u32,
    lds_slots: u32,
) -> Gfx942KernelSemanticOperationV1 {
    let subgroup_operation = matches!(
        operation,
        Gfx942CollectiveOperationV1::SubgroupReduceSumF32
            | Gfx942CollectiveOperationV1::SubgroupReduceMaxF32
    );
    Gfx942KernelSemanticOperationV1::Collective(Gfx942CollectiveSemanticV1 {
        operation_id,
        operation,
        element,
        physical_participant_count: if subgroup_operation {
            GFX942_WAVE64_LANES_V1
        } else {
            operation_width_or_workgroup_size
        },
        subgroup_width: subgroup_operation.then_some(operation_width_or_workgroup_size),
        lds_slots,
        convergent: true,
        exact_one_dimensional_launch: true,
    })
}

#[test]
fn complete_reviewed_atomic_and_collective_roster_is_admitted_exactly() {
    let (runtime, resources, fixture) = fixture();
    let atomic_operations = [
        Gfx942AtomicOperationV1::Load,
        Gfx942AtomicOperationV1::Store,
        Gfx942AtomicOperationV1::Swap,
        Gfx942AtomicOperationV1::CompareExchange,
        Gfx942AtomicOperationV1::FetchAdd,
        Gfx942AtomicOperationV1::FetchSub,
        Gfx942AtomicOperationV1::FetchAnd,
        Gfx942AtomicOperationV1::FetchNand,
        Gfx942AtomicOperationV1::FetchOr,
        Gfx942AtomicOperationV1::FetchXor,
        Gfx942AtomicOperationV1::FetchMinSigned,
        Gfx942AtomicOperationV1::FetchMinUnsigned,
        Gfx942AtomicOperationV1::FetchMaxSigned,
        Gfx942AtomicOperationV1::FetchMaxUnsigned,
    ];
    let mut operations = atomic_operations
        .into_iter()
        .enumerate()
        .map(|(index, operation)| global_atomic(fixture, index as u16 + 1, operation))
        .collect::<Vec<_>>();
    operations.extend([
        collective(
            15,
            Gfx942CollectiveOperationV1::Wave64ReduceSum,
            Gfx942CollectiveElementV1::U32,
            64,
            0,
        ),
        collective(
            16,
            Gfx942CollectiveOperationV1::Wave64InclusiveScanSum,
            Gfx942CollectiveElementV1::I32,
            64,
            0,
        ),
        collective(
            17,
            Gfx942CollectiveOperationV1::Wave64ExclusiveScanSum,
            Gfx942CollectiveElementV1::F32,
            64,
            0,
        ),
        collective(
            18,
            Gfx942CollectiveOperationV1::Wave64ReduceActiveU32,
            Gfx942CollectiveElementV1::U32,
            64,
            0,
        ),
        collective(
            19,
            Gfx942CollectiveOperationV1::SubgroupReduceSumF32,
            Gfx942CollectiveElementV1::F32,
            32,
            0,
        ),
        collective(
            20,
            Gfx942CollectiveOperationV1::SubgroupReduceMaxF32,
            Gfx942CollectiveElementV1::F32,
            16,
            0,
        ),
        collective(
            21,
            Gfx942CollectiveOperationV1::WorkgroupReduceSum,
            Gfx942CollectiveElementV1::U32,
            256,
            256,
        ),
        collective(
            22,
            Gfx942CollectiveOperationV1::WorkgroupInclusiveScanSum,
            Gfx942CollectiveElementV1::I32,
            128,
            128,
        ),
        collective(
            23,
            Gfx942CollectiveOperationV1::WorkgroupExclusiveScanSum,
            Gfx942CollectiveElementV1::F32,
            64,
            64,
        ),
        collective(
            24,
            Gfx942CollectiveOperationV1::Workgroup256ReduceActiveU32,
            Gfx942CollectiveElementV1::U32,
            256,
            256,
        ),
        Gfx942KernelSemanticOperationV1::Atomic(Gfx942AtomicSemanticV1 {
            operation_id: 25,
            operation: Gfx942AtomicOperationV1::FetchAdd,
            width: Gfx942AtomicWidthV1::Bits32,
            storage: Gfx942AtomicStorageV1::Workgroup {
                byte_offset: 16,
                lds_byte_len: 1_024,
            },
            scope: Gfx942AtomicScopeV1::Workgroup,
            success_ordering: Gfx942MemoryOrderingV1::AcquireRelease,
            failure_ordering: None,
        }),
    ]);

    let admitted = admit_gfx942_kernel_semantics_model_only_v1(
        &runtime,
        &resources,
        contract(fixture, operations.clone()),
    )
    .unwrap();
    assert_eq!(admitted.authority_domain(), AuthorityDomainV1::ModelOnly);
    assert_eq!(admitted.device(), fixture.device);
    assert_eq!(admitted.code(), resources.code());
    assert_eq!(admitted.artifact(), fixture.artifact);
    assert_eq!(admitted.resources(), &resources);
    assert_eq!(admitted.operations(), operations);
}

#[test]
fn admitted_value_retains_the_exact_validated_resource_set() {
    let (runtime, resources, fixture) = fixture();
    let operation = global_atomic(fixture, 1, Gfx942AtomicOperationV1::FetchAdd);
    let admitted = admit_gfx942_kernel_semantics_model_only_v1(
        &runtime,
        &resources,
        contract(fixture, vec![operation]),
    )
    .unwrap();

    let alternate_resources = AsyncOperationResourcesV1::new(
        resources.code(),
        resources.completion_signal(),
        resources.kernarg(),
        resources.data().to_vec(),
    )
    .unwrap();
    let alternate = admit_gfx942_kernel_semantics_model_only_v1(
        &runtime,
        &alternate_resources,
        contract(fixture, vec![operation]),
    )
    .unwrap();

    assert_eq!(admitted.resources(), &resources);
    assert_eq!(alternate.resources(), &alternate_resources);
    assert_ne!(admitted, alternate);
}

#[test]
fn identity_artifact_and_canonical_roster_are_required() {
    let (runtime, resources, fixture) = fixture();
    let valid = global_atomic(fixture, 1, Gfx942AtomicOperationV1::FetchAdd);
    for (mut rejected, expected) in [
        (
            UntrustedGfx942KernelSemanticContractV1 {
                schema_version: 0,
                ..contract(fixture, vec![valid])
            },
            Gfx942KernelSemanticAdmissionErrorV1::InvalidSchema,
        ),
        (
            UntrustedGfx942KernelSemanticContractV1 {
                artifact: RuntimeArtifactIdV1::from_untrusted_digest(digest(0x77)),
                ..contract(fixture, vec![valid])
            },
            Gfx942KernelSemanticAdmissionErrorV1::ArtifactMismatch,
        ),
        (
            contract(fixture, Vec::new()),
            Gfx942KernelSemanticAdmissionErrorV1::EmptyOperationRoster,
        ),
        (
            contract(fixture, vec![valid, valid]),
            Gfx942KernelSemanticAdmissionErrorV1::NonCanonicalOperationRoster,
        ),
    ] {
        assert_eq!(
            admit_gfx942_kernel_semantics_model_only_v1(&runtime, &resources, rejected.clone()),
            Err(expected)
        );
        rejected.operations.clear();
    }
}

#[test]
fn atomic_ordering_scope_coherence_resource_and_range_fail_closed() {
    let (runtime, resources, fixture) = fixture();
    let base = match global_atomic(fixture, 1, Gfx942AtomicOperationV1::FetchAdd) {
        Gfx942KernelSemanticOperationV1::Atomic(atomic) => atomic,
        _ => unreachable!(),
    };
    let cases = [
        (
            Gfx942AtomicSemanticV1 {
                operation: Gfx942AtomicOperationV1::Load,
                success_ordering: Gfx942MemoryOrderingV1::Release,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::AtomicOrderingInvalid,
        ),
        (
            Gfx942AtomicSemanticV1 {
                operation: Gfx942AtomicOperationV1::CompareExchange,
                success_ordering: Gfx942MemoryOrderingV1::Release,
                failure_ordering: Some(Gfx942MemoryOrderingV1::Acquire),
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::AtomicFailureOrderingInvalid,
        ),
        (
            Gfx942AtomicSemanticV1 {
                storage: Gfx942AtomicStorageV1::Global {
                    mapping: fixture.data,
                    byte_offset: 16,
                    coherence: DeclaredAtomicCoherenceV1::DeviceOnly,
                },
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::SystemCoherenceRequired,
        ),
        (
            Gfx942AtomicSemanticV1 {
                storage: Gfx942AtomicStorageV1::Global {
                    mapping: fixture.data,
                    byte_offset: 3,
                    coherence: DeclaredAtomicCoherenceV1::SystemCoherent,
                },
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid,
        ),
        (
            Gfx942AtomicSemanticV1 {
                storage: Gfx942AtomicStorageV1::Workgroup {
                    byte_offset: 0,
                    lds_byte_len: 64,
                },
                scope: Gfx942AtomicScopeV1::Device,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::AtomicStorageScopeMismatch,
        ),
    ];
    for (atomic, expected) in cases {
        assert_eq!(
            admit_gfx942_kernel_semantics_model_only_v1(
                &runtime,
                &resources,
                contract(
                    fixture,
                    vec![Gfx942KernelSemanticOperationV1::Atomic(atomic)]
                ),
            ),
            Err(expected)
        );
    }

    let absent_mapping = MappingKeyV1 {
        allocation: fixture.data.allocation,
        id: MappingIdV1(99),
    };
    let absent = Gfx942AtomicSemanticV1 {
        storage: Gfx942AtomicStorageV1::Global {
            mapping: absent_mapping,
            byte_offset: 0,
            coherence: DeclaredAtomicCoherenceV1::SystemCoherent,
        },
        ..base
    };
    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &resources,
            contract(
                fixture,
                vec![Gfx942KernelSemanticOperationV1::Atomic(absent)]
            ),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::MappingNotLive)
    );
}

#[test]
fn global_atomic_alignment_is_checked_at_the_actual_gpu_address() {
    let (runtime, resources, fixture) = fixture_with_data_gpu_va(0x1003_0002);
    let atomic = global_atomic(fixture, 1, Gfx942AtomicOperationV1::FetchAdd);

    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &resources,
            contract(fixture, vec![atomic]),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::AtomicRangeInvalid)
    );
}

#[test]
fn overlapping_atomic_declarations_must_name_the_same_exact_object() {
    let (runtime, resources, fixture) = fixture();
    let first = match global_atomic(fixture, 1, Gfx942AtomicOperationV1::FetchAdd) {
        Gfx942KernelSemanticOperationV1::Atomic(atomic) => atomic,
        _ => unreachable!(),
    };
    let exact_reuse = Gfx942AtomicSemanticV1 {
        operation_id: 2,
        operation: Gfx942AtomicOperationV1::FetchXor,
        ..first
    };
    assert!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &resources,
            contract(
                fixture,
                vec![
                    Gfx942KernelSemanticOperationV1::Atomic(first),
                    Gfx942KernelSemanticOperationV1::Atomic(exact_reuse),
                ],
            ),
        )
        .is_ok()
    );

    let mixed_width = Gfx942AtomicSemanticV1 {
        operation_id: 2,
        width: Gfx942AtomicWidthV1::Bits64,
        ..first
    };
    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &resources,
            contract(
                fixture,
                vec![
                    Gfx942KernelSemanticOperationV1::Atomic(first),
                    Gfx942KernelSemanticOperationV1::Atomic(mixed_width),
                ],
            ),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::IncompatibleAtomicObject)
    );

    let lds_wide = Gfx942AtomicSemanticV1 {
        operation_id: 1,
        operation: Gfx942AtomicOperationV1::FetchAdd,
        width: Gfx942AtomicWidthV1::Bits64,
        storage: Gfx942AtomicStorageV1::Workgroup {
            byte_offset: 0,
            lds_byte_len: 64,
        },
        scope: Gfx942AtomicScopeV1::Workgroup,
        success_ordering: Gfx942MemoryOrderingV1::Relaxed,
        failure_ordering: None,
    };
    let lds_partial_overlap = Gfx942AtomicSemanticV1 {
        operation_id: 2,
        width: Gfx942AtomicWidthV1::Bits32,
        storage: Gfx942AtomicStorageV1::Workgroup {
            byte_offset: 4,
            lds_byte_len: 64,
        },
        ..lds_wide
    };
    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &resources,
            contract(
                fixture,
                vec![
                    Gfx942KernelSemanticOperationV1::Atomic(lds_wide),
                    Gfx942KernelSemanticOperationV1::Atomic(lds_partial_overlap),
                ],
            ),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::IncompatibleAtomicObject)
    );
}

#[test]
fn different_virtual_mappings_cannot_hide_an_incompatible_physical_atomic_object() {
    let (mut runtime, resources, fixture) = fixture();
    let alias = MappingKeyV1 {
        allocation: fixture.data.allocation,
        id: MappingIdV1(99),
    };
    runtime = advance(
        runtime,
        RuntimeTransitionV1::Map {
            key: alias,
            allocation_offset: 0,
            gpu_va: 0x1004_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let aliased_resources = AsyncOperationResourcesV1::new(
        resources.code(),
        resources.kernarg(),
        resources.completion_signal(),
        vec![
            DispatchResourceV1 {
                mapping: fixture.data,
                required_access: MemoryAccessV1::ReadWrite,
            },
            DispatchResourceV1 {
                mapping: alias,
                required_access: MemoryAccessV1::ReadWrite,
            },
        ],
    )
    .unwrap();
    let first = match global_atomic(fixture, 1, Gfx942AtomicOperationV1::FetchAdd) {
        Gfx942KernelSemanticOperationV1::Atomic(atomic) => atomic,
        _ => unreachable!(),
    };
    let incompatible_alias = Gfx942AtomicSemanticV1 {
        operation_id: 2,
        width: Gfx942AtomicWidthV1::Bits64,
        storage: Gfx942AtomicStorageV1::Global {
            mapping: alias,
            byte_offset: 16,
            coherence: DeclaredAtomicCoherenceV1::SystemCoherent,
        },
        ..first
    };

    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &aliased_resources,
            contract(
                fixture,
                vec![
                    Gfx942KernelSemanticOperationV1::Atomic(first),
                    Gfx942KernelSemanticOperationV1::Atomic(incompatible_alias),
                ],
            ),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::IncompatibleAtomicObject)
    );
}

#[test]
fn different_virtual_mappings_cannot_alias_resource_roles() {
    let (mut runtime, resources, fixture) = fixture();
    let kernarg_alias = MappingKeyV1 {
        allocation: resources.kernarg().allocation,
        id: MappingIdV1(99),
    };
    runtime = advance(
        runtime,
        RuntimeTransitionV1::Map {
            key: kernarg_alias,
            allocation_offset: 0,
            gpu_va: 0x1004_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let aliased_resources = AsyncOperationResourcesV1::new(
        resources.code(),
        resources.kernarg(),
        resources.completion_signal(),
        vec![DispatchResourceV1 {
            mapping: kernarg_alias,
            required_access: MemoryAccessV1::ReadWrite,
        }],
    )
    .unwrap();

    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &aliased_resources,
            contract(
                fixture,
                vec![collective(
                    1,
                    Gfx942CollectiveOperationV1::Wave64ReduceSum,
                    Gfx942CollectiveElementV1::U32,
                    GFX942_WAVE64_LANES_V1,
                    0,
                )],
            ),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::ResourceRoleCollision)
    );
}

#[test]
fn writable_different_va_alias_of_live_executable_storage_is_rejected() {
    let (mut runtime, resources, fixture) = fixture();
    let executable = runtime
        .loaded_code()
        .iter()
        .find(|code| code.key == resources.code())
        .unwrap()
        .executable_mapping;
    let writable_alias = MappingKeyV1 {
        allocation: executable.allocation,
        id: MappingIdV1(98),
    };
    runtime = advance(
        runtime,
        RuntimeTransitionV1::Map {
            key: writable_alias,
            allocation_offset: 0,
            gpu_va: 0x1004_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::ReadWrite,
        },
    );
    let writable_resources = AsyncOperationResourcesV1::new(
        resources.code(),
        resources.kernarg(),
        resources.completion_signal(),
        vec![DispatchResourceV1 {
            mapping: writable_alias,
            required_access: MemoryAccessV1::ReadWrite,
        }],
    )
    .unwrap();
    let operation = collective(
        1,
        Gfx942CollectiveOperationV1::Wave64ReduceSum,
        Gfx942CollectiveElementV1::U32,
        GFX942_WAVE64_LANES_V1,
        0,
    );

    assert_eq!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &writable_resources,
            contract(fixture, vec![operation]),
        ),
        Err(Gfx942KernelSemanticAdmissionErrorV1::ExecutableStorageCollision)
    );
}

#[test]
fn read_only_different_va_alias_of_live_executable_storage_remains_admissible() {
    let (mut runtime, resources, fixture) = fixture();
    let executable = runtime
        .loaded_code()
        .iter()
        .find(|code| code.key == resources.code())
        .unwrap()
        .executable_mapping;
    let read_alias = MappingKeyV1 {
        allocation: executable.allocation,
        id: MappingIdV1(97),
    };
    runtime = advance(
        runtime,
        RuntimeTransitionV1::Map {
            key: read_alias,
            allocation_offset: 0,
            gpu_va: 0x1004_0000,
            byte_len: 4_096,
            access: MemoryAccessV1::Read,
        },
    );
    let read_resources = AsyncOperationResourcesV1::new(
        resources.code(),
        resources.kernarg(),
        resources.completion_signal(),
        vec![DispatchResourceV1 {
            mapping: read_alias,
            required_access: MemoryAccessV1::Read,
        }],
    )
    .unwrap();
    let operation = collective(
        1,
        Gfx942CollectiveOperationV1::Wave64ReduceSum,
        Gfx942CollectiveElementV1::U32,
        GFX942_WAVE64_LANES_V1,
        0,
    );

    assert!(
        admit_gfx942_kernel_semantics_model_only_v1(
            &runtime,
            &read_resources,
            contract(fixture, vec![operation]),
        )
        .is_ok()
    );
}

#[test]
fn collective_convergence_geometry_elements_and_scratch_are_exact() {
    let (runtime, resources, fixture) = fixture();
    let base = Gfx942CollectiveSemanticV1 {
        operation_id: 1,
        operation: Gfx942CollectiveOperationV1::WorkgroupReduceSum,
        element: Gfx942CollectiveElementV1::U32,
        physical_participant_count: 256,
        subgroup_width: None,
        lds_slots: 256,
        convergent: true,
        exact_one_dimensional_launch: true,
    };
    let cases = [
        (
            Gfx942CollectiveSemanticV1 {
                convergent: false,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveConvergenceRequired,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                physical_participant_count: 192,
                lds_slots: 192,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                lds_slots: 255,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveScratchInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                operation: Gfx942CollectiveOperationV1::SubgroupReduceMaxF32,
                element: Gfx942CollectiveElementV1::U32,
                physical_participant_count: GFX942_WAVE64_LANES_V1,
                subgroup_width: Some(32),
                lds_slots: 0,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveElementInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                operation: Gfx942CollectiveOperationV1::Wave64ReduceSum,
                physical_participant_count: 32,
                lds_slots: 0,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                operation: Gfx942CollectiveOperationV1::SubgroupReduceSumF32,
                element: Gfx942CollectiveElementV1::F32,
                physical_participant_count: 32,
                subgroup_width: Some(32),
                lds_slots: 0,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                operation: Gfx942CollectiveOperationV1::SubgroupReduceSumF32,
                element: Gfx942CollectiveElementV1::F32,
                physical_participant_count: GFX942_WAVE64_LANES_V1,
                subgroup_width: None,
                lds_slots: 0,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                operation: Gfx942CollectiveOperationV1::SubgroupReduceSumF32,
                element: Gfx942CollectiveElementV1::F32,
                physical_participant_count: GFX942_WAVE64_LANES_V1,
                subgroup_width: Some(3),
                lds_slots: 0,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                operation: Gfx942CollectiveOperationV1::Wave64ReduceSum,
                physical_participant_count: GFX942_WAVE64_LANES_V1,
                subgroup_width: Some(32),
                lds_slots: 0,
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
        (
            Gfx942CollectiveSemanticV1 {
                subgroup_width: Some(32),
                ..base
            },
            Gfx942KernelSemanticAdmissionErrorV1::CollectiveGeometryInvalid,
        ),
    ];
    for (collective, expected) in cases {
        assert_eq!(
            admit_gfx942_kernel_semantics_model_only_v1(
                &runtime,
                &resources,
                contract(
                    fixture,
                    vec![Gfx942KernelSemanticOperationV1::Collective(collective)]
                ),
            ),
            Err(expected)
        );
    }
}

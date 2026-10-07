use super::*;

#[test]
fn mapped_coherent_subrange_copy_is_exact_and_generation_checked() {
    let mut engine = acquired();
    let mut token = engine.allocate::<HostVisibleCoherentGttV1>(256).unwrap();
    engine
        .with_bytes_mut(&mut token, |bytes| {
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = index as u8;
            }
        })
        .unwrap();
    let mut token = engine.map_mutable(token).unwrap();
    let copied = engine
        .copy_mapped_host_visible_subrange(&token, 64, 32)
        .unwrap();
    assert_eq!(copied.as_ref(), &(64_u8..96).collect::<Vec<_>>());
    let mut copied_into = [0_u8; 32];
    engine
        .copy_mapped_host_visible_subrange_into(&token, 64, &mut copied_into)
        .unwrap();
    assert_eq!(copied_into, (64_u8..96).collect::<Vec<_>>().as_slice());
    engine
        .overwrite_mapped_host_visible_subrange(&mut token, 80, &[9, 8, 7, 6])
        .unwrap();
    let overwritten = engine
        .copy_mapped_host_visible_subrange(&token, 78, 8)
        .unwrap();
    assert_eq!(overwritten.as_ref(), &[78, 79, 9, 8, 7, 6, 84, 85]);
    assert!(
        engine
            .copy_mapped_host_visible_subrange(&token, 0, 0)
            .is_err()
    );
    assert!(
        engine
            .copy_mapped_host_visible_subrange(&token, 250, 16)
            .is_err()
    );
    assert!(
        engine
            .copy_mapped_host_visible_subrange_into(&token, 250, &mut copied_into)
            .is_err()
    );
    assert!(
        engine
            .overwrite_mapped_host_visible_subrange(&mut token, 250, &[0; 16])
            .is_err()
    );

    let stale = SharedGttAllocationV1 {
        session_id: token.session_id,
        id: token.id,
        generation: token.generation + 1,
        layout: token.layout,
        marker: PhantomData,
    };
    assert!(
        engine
            .copy_mapped_host_visible_subrange(&stale, 64, 32)
            .is_err()
    );
}

#[test]
fn coherent_capture_into_rejects_each_stale_coordinate_without_writing() {
    for mutation in 0..7 {
        let mut engine = acquired();
        let token = engine.allocate::<HostVisibleCoherentGttV1>(64).unwrap();
        let token = engine.map_mutable(token).unwrap();
        let mut candidate = SharedGttAllocationV1 {
            session_id: token.session_id,
            id: token.id,
            generation: token.generation,
            layout: token.layout,
            marker: PhantomData::<(HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1)>,
        };
        match mutation {
            0 => candidate.session_id += 1,
            1 => candidate.id += 1,
            2 => candidate.generation += 1,
            3 => candidate.layout.requested_bytes += 1,
            4 => engine.allocations[0].phase = SharedAllocationPhaseV1::Released,
            5 => engine.allocations[0].mapping = None,
            _ => engine.phase = SharedMemorySessionPhaseV1::Quarantined,
        }
        let calls = (
            engine.backend.reserve_va_calls,
            engine.backend.alloc_calls,
            engine.backend.map_cpu_calls,
            engine.backend.map_gpu_calls,
            engine.backend.unmap_gpu_calls,
            engine.backend.free_calls,
        );
        let mut destination = [0xa5; 8];
        assert!(
            engine
                .copy_mapped_host_visible_subrange_into(&candidate, 8, &mut destination)
                .is_err()
        );
        assert_eq!(destination, [0xa5; 8]);
        assert_eq!(
            calls,
            (
                engine.backend.reserve_va_calls,
                engine.backend.alloc_calls,
                engine.backend.map_cpu_calls,
                engine.backend.map_gpu_calls,
                engine.backend.unmap_gpu_calls,
                engine.backend.free_calls
            )
        );
    }
}

#[test]
fn coherent_capture_into_has_exact_currentness_and_no_native_mutation() {
    for failing_fence in [None, Some(1), Some(2)] {
        let mut engine = acquired();
        let mut token = engine.allocate::<HostVisibleCoherentGttV1>(64).unwrap();
        engine
            .with_bytes_mut(&mut token, |bytes| {
                for (index, byte) in bytes.iter_mut().enumerate() {
                    *byte = index as u8;
                }
            })
            .unwrap();
        let token = engine.map_mutable(token).unwrap();
        let before = engine.backend.operational_currentness_calls;
        engine.backend.fail_operational_currentness_at = failing_fence.map(|delta| before + delta);
        let operations = engine.backend.operations.len();
        let currentness = engine.backend.currentness_calls;
        let mut destination = [0xa5; 12];
        let result =
            engine.copy_mapped_host_visible_subrange_into(&token, 16, &mut destination[2..10]);
        assert_eq!(destination[..2], [0xa5; 2]);
        assert_eq!(destination[10..], [0xa5; 2]);
        assert_eq!(engine.backend.currentness_calls, currentness);
        assert_eq!(engine.backend.operations.len(), operations);
        if failing_fence == Some(1) {
            assert_eq!(destination, [0xa5; 12]);
            assert_eq!(engine.backend.operational_currentness_calls - before, 1);
        } else {
            assert_eq!(destination[2..10], [16, 17, 18, 19, 20, 21, 22, 23]);
            assert_eq!(engine.backend.operational_currentness_calls - before, 2);
        }
        if failing_fence.is_some() {
            assert!(result.is_err());
            assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        } else {
            assert!(result.is_ok());
        }
    }
}

#[test]
fn coherent_capture_into_panic_still_checks_closing_currentness() {
    let mut engine = acquired();
    let token = engine.allocate::<HostVisibleCoherentGttV1>(64).unwrap();
    let token = engine.map_mutable(token).unwrap();
    engine.allocations[0].mapping.as_mut().unwrap().panic_access = Some("with_bytes");
    let before = engine.backend.operational_currentness_calls;
    let mut destination = [0xa5; 8];
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        engine.copy_mapped_host_visible_subrange_into(&token, 8, &mut destination)
    }))
    .unwrap_err();
    assert_eq!(
        payload.downcast_ref::<(&'static str, &'static str)>(),
        Some(&("N2 native panic", "with_bytes"))
    );
    assert_eq!(engine.backend.operational_currentness_calls - before, 2);
    assert_eq!(destination, [0xa5; 8]);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

#[test]
fn authenticated_full_mapped_write_hashes_while_preserving_chunk_currentness() {
    let mut engine = acquired();
    let token = engine.allocate::<HostVisibleCoherentGttV1>(256).unwrap();
    let mut token = engine.map_mutable(token).unwrap();
    let source: Vec<u8> = (0..=255).collect();
    let before = engine.backend.operational_currentness_calls;
    let observed = engine
        .overwrite_full_mapped_host_visible_and_sha256(&mut token, &source, 100)
        .unwrap();
    assert_eq!(observed, <[u8; 32]>::from(Sha256::digest(&source)));
    assert_eq!(engine.backend.operational_currentness_calls - before, 6);
    assert_eq!(
        engine
            .copy_mapped_host_visible_subrange(&token, 0, 256)
            .unwrap()
            .as_ref(),
        source
    );
}

#[test]
fn authenticated_mapped_write_currentness_failure_quarantines_session() {
    for currentness_delta in 1..=6 {
        let mut engine = acquired();
        let token = engine.allocate::<HostVisibleCoherentGttV1>(256).unwrap();
        let mut token = engine.map_mutable(token).unwrap();
        let source = vec![0xa5; 256];
        engine.backend.fail_operational_currentness_at = Some(
            engine
                .backend
                .operational_currentness_calls
                .checked_add(currentness_delta)
                .unwrap(),
        );

        assert!(matches!(
            engine.overwrite_full_mapped_host_visible_and_sha256(&mut token, &source, 100),
            Err(MemorySessionError::Injected("operational_currentness"))
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        let written_bytes = match currentness_delta {
            1 => 0,
            2 | 3 => 100,
            4 | 5 => 200,
            6 => 256,
            _ => unreachable!(),
        };
        let mapping = engine.allocations[0].mapping.as_ref().unwrap();
        assert!(
            mapping.bytes[..written_bytes]
                .iter()
                .all(|byte| *byte == 0xa5)
        );
        assert!(mapping.bytes[written_bytes..].iter().all(|byte| *byte == 0));
        assert!(matches!(
            engine.overwrite_full_mapped_host_visible_and_sha256(&mut token, &source, 100),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
    }
}

#[test]
fn shared_allocation_identity_rejects_cross_session_substitution() {
    let mut first = acquired();
    let mut second = acquired();
    let first_token = first.allocate::<HostVisibleCoherentGttV1>(256).unwrap();
    let second_token = second.allocate::<HostVisibleCoherentGttV1>(256).unwrap();

    assert_eq!(first_token.id, second_token.id);
    assert_eq!(first_token.generation, second_token.generation);
    assert_ne!(
        first_token.storage_identity(),
        second_token.storage_identity()
    );
    assert!(
        second
            .index(&first_token, SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
}

#[test]
fn shared_record_slot_index_is_non_authoritative_and_fully_authenticated() {
    let mut engine = acquired();
    let first = engine.allocate::<HostVisibleCoherentGttV1>(256).unwrap();
    let second = engine.allocate::<HostVisibleCoherentGttV1>(512).unwrap();
    let first_slot = engine.allocation_record_slots[&first.id];
    let second_slot = engine.allocation_record_slots[&second.id];
    let forge = |generation, layout| SharedGttAllocationV1 {
        session_id: first.session_id,
        id: first.id,
        generation,
        layout,
        marker: PhantomData::<(HostVisibleCoherentGttV1, GttCpuWritableV1)>,
    };

    engine.allocation_record_slots.insert(first.id, second_slot);
    let wrong_in_range = forge(first.generation, first.layout);
    assert_eq!(
        engine
            .index(&wrong_in_range, SharedAllocationPhaseV1::CpuWritable)
            .unwrap(),
        first_slot
    );
    engine.allocation_record_slots.insert(first.id, usize::MAX);
    let out_of_range = forge(first.generation, first.layout);
    assert_eq!(
        engine
            .index(&out_of_range, SharedAllocationPhaseV1::CpuWritable)
            .unwrap(),
        first_slot
    );
    engine.allocation_record_slots.insert(first.id, first_slot);
    let stale = forge(first.generation + 1, first.layout);
    assert!(matches!(
        engine.index(&stale, SharedAllocationPhaseV1::CpuWritable),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    let wrong_profile = SharedGttAllocationV1 {
        session_id: first.session_id,
        id: first.id,
        generation: first.generation,
        layout: first.layout,
        marker: PhantomData::<(KernargGttV1, GttCpuWritableV1)>,
    };
    assert!(matches!(
        engine.index(&wrong_profile, SharedAllocationPhaseV1::CpuWritable),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    assert!(matches!(
        engine.index(&first, SharedAllocationPhaseV1::GpuAccessibleMutable),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    let mut forged_layout = first.layout;
    forged_layout.requested_bytes += 1;
    let forged = forge(first.generation, forged_layout);
    assert!(matches!(
        engine.index(&forged, SharedAllocationPhaseV1::CpuWritable),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn shared_record_slot_index_lookup_work_is_constant_across_bounded_depths() {
    for count in [1, 32, MAX_SHARED_GTT_ALLOCATIONS_V1] {
        let mut engine = acquired();
        let mut tokens = Vec::new();
        for _ in 0..count {
            tokens.push(engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap());
        }
        for token in &tokens {
            engine.shared_lookup_comparisons.set(0);
            assert_eq!(
                engine
                    .index(token, SharedAllocationPhaseV1::CpuWritable)
                    .unwrap(),
                engine.allocation_record_slots[&token.id]
            );
            assert_eq!(engine.shared_lookup_comparisons.get(), 1);
        }
    }
}

#[test]
fn private_role_subranges_reject_overlap_misalignment_and_overflow() {
    let vm = VmKeyV1 {
        device: fe2o3_runtime_model::DeviceKeyV1 {
            physical: fe2o3_runtime_model::PhysicalDeviceIdV1(1),
            generation: fe2o3_runtime_model::DeviceGenerationV1(1),
        },
        id: VmIdV1(1),
    };
    let (_, _, mapping) = model_keys(vm, 7, 1);
    let facts = SharedGttMappedResourceFactsV1 {
        gpu_va: 0x20_000,
        logical_bytes: 8192,
        cpu_mapping_bytes: 8192,
        gpu_va_bytes: 8192,
        mapping,
        publication: MemoryPublicationKeyV1 {
            mapping,
            id: MemoryPublicationIdV1(7),
        },
    };
    assert_eq!(
        facts.checked_disjoint_gpu_subranges((0, 8, 8), (4096, 8, 8)),
        Some((0x20_000, 0x21_000))
    );
    assert_eq!(
        facts.checked_disjoint_gpu_subranges((0, 8, 8), (0, 8, 8)),
        None
    );
    assert_eq!(facts.checked_gpu_subrange(1, 8, 8), None);
    assert_eq!(facts.checked_gpu_subrange(8188, 8, 4), None);
    assert_eq!(facts.checked_gpu_subrange(0, 0, 8), None);

    let overflowing = SharedGttMappedResourceFactsV1 {
        gpu_va: u64::MAX - 4095,
        ..facts
    };
    assert_eq!(overflowing.checked_gpu_subrange(4096, 8, 8), None);
}

#[test]
fn four_profiles_coexist_with_exact_flags_and_gfx942_aql_geometry() {
    let mut engine = acquired();
    let mut ordinary = engine.allocate::<HostVisibleCoherentGttV1>(4097).unwrap();
    let mut kernarg = engine.allocate::<KernargGttV1>(256).unwrap();
    let mut aql = engine.allocate::<AqlQueueGttV1>(4096).unwrap();
    let mut executable = engine.allocate::<ExecutableGttV1>(8192).unwrap();
    assert_eq!(
        engine.backend.flags,
        vec![0x8400_0002, 0x8600_0002, 0xc400_0002, 0xc400_0002]
    );
    assert_eq!(ordinary.layout().cpu_mapping_bytes(), 8192);
    assert_eq!(aql.layout().cpu_mapping_bytes(), 4096);
    assert_eq!(aql.layout().gpu_va_bytes(), 4096);
    engine
        .with_bytes_mut(&mut ordinary, |bytes| bytes[0] = 11)
        .unwrap();
    engine
        .with_bytes_mut(&mut kernarg, |bytes| bytes[0] = 22)
        .unwrap();
    engine
        .with_bytes_mut(&mut aql, |bytes| bytes[0] = 33)
        .unwrap();
    engine
        .with_bytes_mut(&mut executable, |bytes| bytes[0] = 44)
        .unwrap();
    assert_eq!(
        engine
            .with_bytes(&ordinary, SharedAllocationPhaseV1::CpuWritable, |b| b[0])
            .unwrap(),
        11
    );
    let executable = engine.seal_executable(executable).unwrap();
    assert_eq!(
        engine
            .with_bytes(
                &executable,
                SharedAllocationPhaseV1::ExecutableImmutable,
                |b| b[0]
            )
            .unwrap(),
        44
    );
    let ordinary = engine.map_mutable(ordinary).unwrap();
    let ordinary = engine.unmap_mutable(ordinary).unwrap();
    let executable = engine.map_executable(executable).unwrap();
    let executable = engine.unmap_executable(executable).unwrap();
    engine
        .release(ordinary, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    engine
        .release(kernarg, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    engine
        .release(aql, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    engine
        .release(executable, SharedAllocationPhaseV1::ExecutableImmutable)
        .unwrap();
    assert_eq!(engine.retained_gpu_va_bytes, 0);
    assert_eq!(engine.backend.free_calls, 4);
    assert_eq!(engine.backend.release_va_calls, 4);
}

#[test]
fn dropped_partial_dispatch_code_and_kernarg_tokens_remain_registry_owned() {
    let mut engine = acquired();
    let executable = engine.allocate::<ExecutableGttV1>(8192).unwrap();
    let executable = engine.seal_executable(executable).unwrap();
    let executable = engine.map_executable(executable).unwrap();
    let kernarg = engine.allocate::<KernargGttV1>(256).unwrap();
    let kernarg = engine.map_mutable(kernarg).unwrap();

    drop(executable);
    drop(kernarg);

    assert_eq!(engine.allocations.len(), 2);
    assert!(engine.allocations.iter().all(|record| {
        record.phase != SharedAllocationPhaseV1::Released
            && record.handle.is_some()
            && record.mapping.is_some()
    }));
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);

    assert!(matches!(
        engine
            .quarantine::<()>(MemorySessionError::KernelResultMalformed(
                "partial fixed dispatch preparation",
            ))
            .unwrap_err(),
        MemorySessionError::KernelResultMalformed("partial fixed dispatch preparation")
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.allocations.len(), 2);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

#[test]
fn gfx942_production_and_executable_probe_rings_are_exact_one_span() {
    let special = profile_layout::<AqlQueueGttV1>(4096).unwrap();
    let probe = profile_layout::<ExecutableAqlQueueProbeGttV1>(4096).unwrap();

    assert_eq!(special.requested_bytes(), 4096);
    assert_eq!(special.cpu_mapping_bytes(), 4096);
    assert_eq!(special.gpu_va_bytes(), 4096);
    assert_eq!(special.uapi_flags(), KfdAllocMemoryFlags::EXECUTABLE.bits());
    assert_eq!(probe.profile(), SharedGttProfileV1::AqlQueue);
    assert_eq!(probe.requested_bytes(), 4096);
    assert_eq!(probe.cpu_mapping_bytes(), 4096);
    assert_eq!(probe.gpu_va_bytes(), 4096);
    assert_eq!(probe.uapi_flags(), KfdAllocMemoryFlags::EXECUTABLE.bits());
    assert_eq!(
        probe.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_AQL_QUEUE_MEM,
        0
    );
    assert_eq!(
        probe.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_UNCACHED,
        0
    );

    for invalid in [0, 1024, 4095, 4097, 8193] {
        assert!(profile_layout::<ExecutableAqlQueueProbeGttV1>(invalid).is_err());
    }
    let oversized = usize::try_from(MAX_AQL_QUEUE_BYTES_V1 + 1).unwrap();
    assert!(profile_layout::<ExecutableAqlQueueProbeGttV1>(oversized).is_err());
}

#[test]
fn executable_probe_ring_uses_exact_allocation_and_release_span() {
    let mut engine = acquired();
    let mut ring = engine
        .allocate::<ExecutableAqlQueueProbeGttV1>(4096)
        .unwrap();
    assert_eq!(engine.backend.flags, vec![0xc400_0002]);
    assert_eq!(engine.allocations[0].reservation, Some((0x2_0000, 4096)));
    engine
        .with_bytes_mut(&mut ring, |bytes| bytes.fill(0xa5))
        .unwrap();
    let ring = engine.map_mutable(ring).unwrap();
    let ring = engine.unmap_mutable(ring).unwrap();
    engine
        .release(ring, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(engine.retained_gpu_va_bytes, 0);
    assert_eq!(engine.backend.unmap_gpu_calls, 1);
    assert_eq!(engine.backend.free_calls, 1);
    assert_eq!(engine.backend.release_va_calls, 1);
}

#[test]
fn userptr_probe_ring_is_exact_one_x_registration() {
    let special = profile_layout::<AqlQueueGttV1>(4096).unwrap();
    let userptr = profile_layout::<UserptrAqlQueueProbeGttV1>(4096).unwrap();

    assert_eq!(special.gpu_va_bytes(), 4096);
    assert_eq!(userptr.profile(), SharedGttProfileV1::AqlQueue);
    assert_eq!(userptr.requested_bytes(), 4096);
    assert_eq!(userptr.cpu_mapping_bytes(), 4096);
    assert_eq!(userptr.gpu_va_bytes(), 4096);
    assert_eq!(userptr.uapi_flags(), 0xd600_0004);
    assert_eq!(
        userptr.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_GTT,
        0
    );
    assert_eq!(
        userptr.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_AQL_QUEUE_MEM,
        0
    );
    assert_ne!(
        userptr.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_USERPTR,
        0
    );
    assert_ne!(
        userptr.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_NO_SUBSTITUTE,
        0
    );
    for invalid in [0, 1024, 4095, 4097, 8193] {
        assert!(profile_layout::<UserptrAqlQueueProbeGttV1>(invalid).is_err());
    }
}

#[test]
fn userptr_aql_control_is_exact_same_va_coherent_page() {
    let control = profile_layout::<UserptrAqlControlGttV1>(4096).unwrap();
    assert_eq!(control.profile(), SharedGttProfileV1::HostVisibleCoherent);
    assert_eq!(control.requested_bytes(), 4096);
    assert_eq!(control.cpu_mapping_bytes(), 4096);
    assert_eq!(control.gpu_va_bytes(), 4096);
    assert_eq!(control.uapi_flags(), 0x8400_0004);
    assert_ne!(
        control.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_USERPTR,
        0
    );
    assert_ne!(
        control.uapi_flags() & fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_COHERENT,
        0
    );
    assert_eq!(
        control.uapi_flags()
            & (fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_EXECUTABLE
                | fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_UNCACHED
                | fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_NO_SUBSTITUTE
                | fe2o3_kfd_uapi::KFD_IOC_ALLOC_MEM_FLAGS_AQL_QUEUE_MEM),
        0
    );

    let mut engine = acquired();
    let control = engine.allocate::<UserptrAqlControlGttV1>(4096).unwrap();
    assert_eq!(engine.backend.flags, vec![0x8400_0004]);
    assert_eq!(
        engine.backend.last_userptr_input,
        Some((0x2_0000, 0x2_0000, 4096))
    );
    assert_eq!(engine.allocations[0].mmap_offset, 0x91_000);
    let control = engine.map_mutable(control).unwrap();
    let control = engine.unmap_mutable(control).unwrap();
    engine
        .release(control, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(
        engine.backend.operations,
        [
            "prepare_userptr",
            "prepare_cpu_mapping",
            "alloc_userptr",
            "map_gpu",
            "unmap_gpu",
            "free",
            "unmap_cpu",
        ]
    );
}

#[test]
fn userptr_aql_control_malformed_result_quarantines_without_cleanup() {
    let mut engine = acquired();
    engine.backend.corrupt_flags = true;
    assert!(engine.allocate::<UserptrAqlControlGttV1>(4096).is_err());
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

#[test]
fn userptr_probe_frees_bo_before_unmapping_original_vma() {
    let mut engine = acquired();
    let mut ring = engine.allocate::<UserptrAqlQueueProbeGttV1>(4096).unwrap();
    assert_eq!(engine.backend.flags, vec![0xd600_0004]);
    assert_eq!(
        engine.backend.last_userptr_input,
        Some((0x2_0000, 0x2_0000, 4096))
    );
    assert_eq!(engine.allocations[0].mmap_offset, 0x91_000);
    assert_ne!(
        engine.allocations[0].mmap_offset,
        engine.allocations[0].gpu_va
    );
    engine
        .with_bytes_mut(&mut ring, |bytes| bytes.fill(0xa5))
        .unwrap();
    let ring = engine.map_mutable(ring).unwrap();
    let ring = engine.unmap_mutable(ring).unwrap();
    engine
        .release(ring, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();

    assert_eq!(
        engine.backend.operations,
        [
            "prepare_userptr",
            "prepare_cpu_mapping",
            "alloc_userptr",
            "map_gpu",
            "unmap_gpu",
            "free",
            "unmap_cpu",
        ]
    );
    assert_eq!(engine.backend.release_va_calls, 0);
    assert_eq!(engine.retained_gpu_va_bytes, 0);
    assert_eq!(
        engine.allocations[0].phase,
        SharedAllocationPhaseV1::Released
    );
}

#[test]
fn userptr_probe_native_failures_quarantine_without_reordered_cleanup() {
    for operation in ["prepare_userptr", "prepare_cpu_mapping", "alloc_userptr"] {
        let mut engine = acquired();
        engine.backend.fail_operation = Some(operation);
        assert!(engine.allocate::<UserptrAqlQueueProbeGttV1>(4096).is_err());
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }

    let mut map = acquired();
    let ring = map.allocate::<UserptrAqlQueueProbeGttV1>(4096).unwrap();
    map.backend.map_errno = true;
    assert!(map.map_mutable(ring).is_err());
    assert_eq!(map.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(map.backend.free_calls, 0);

    let mut unmap = acquired();
    let ring = unmap
        .allocate::<UserptrAqlQueueProbeGttV1>(4096)
        .and_then(|ring| unmap.map_mutable(ring))
        .unwrap();
    unmap.backend.unmap_errno = true;
    assert!(unmap.unmap_mutable(ring).is_err());
    assert_eq!(unmap.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(unmap.backend.free_calls, 0);

    let mut free = acquired();
    let ring = free.allocate::<UserptrAqlQueueProbeGttV1>(4096).unwrap();
    free.backend.fail_operation = Some("free");
    assert!(
        free.release(ring, SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
    assert_eq!(free.backend.free_calls, 1);
    assert!(free.allocations[0].handle.is_some());
    assert!(free.allocations[0].mapping.is_some());
    assert_eq!(free.backend.release_va_calls, 0);

    let mut cpu_unmap = acquired();
    let ring = cpu_unmap
        .allocate::<UserptrAqlQueueProbeGttV1>(4096)
        .unwrap();
    cpu_unmap.backend.fail_operation = Some("unmap_cpu");
    assert!(
        cpu_unmap
            .release(ring, SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
    assert!(cpu_unmap.allocations[0].handle.is_none());
    assert!(cpu_unmap.allocations[0].mapping.is_some());
    assert_eq!(cpu_unmap.backend.release_va_calls, 0);
    assert_eq!(
        &cpu_unmap.backend.operations[cpu_unmap.backend.operations.len() - 2..],
        ["free", "unmap_cpu"]
    );
}

#[test]
fn userptr_output_mmap_offset_is_opaque_and_not_collision_authority() {
    let mut zero = acquired();
    zero.backend.userptr_mmap_offset = Some(0);
    let ring = zero.allocate::<UserptrAqlQueueProbeGttV1>(4096).unwrap();
    assert_eq!(zero.allocations[0].mmap_offset, 0);
    zero.release(ring, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();

    let mut collide_with_ordinary = acquired();
    let ordinary = collide_with_ordinary
        .allocate::<HostVisibleCoherentGttV1>(4096)
        .unwrap();
    let ordinary_offset = collide_with_ordinary.allocations[0].mmap_offset;
    collide_with_ordinary.backend.userptr_mmap_offset = Some(ordinary_offset);
    let userptr = collide_with_ordinary
        .allocate::<UserptrAqlQueueProbeGttV1>(4096)
        .unwrap();
    collide_with_ordinary
        .release(userptr, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    collide_with_ordinary
        .release(ordinary, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();

    let mut collide_with_later_gtt = acquired();
    collide_with_later_gtt.backend.userptr_mmap_offset = Some(0x42_000);
    let userptr = collide_with_later_gtt
        .allocate::<UserptrAqlQueueProbeGttV1>(4096)
        .unwrap();
    let ordinary = collide_with_later_gtt
        .allocate::<HostVisibleCoherentGttV1>(4096)
        .unwrap();
    collide_with_later_gtt
        .release(userptr, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    collide_with_later_gtt
        .release(ordinary, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
}

#[test]
fn userptr_allocation_currentness_failures_require_process_terminal_custody() {
    for currentness_call in [3, 4, 5, 6] {
        let mut engine = acquired();
        engine.backend.fail_currentness_at = Some(currentness_call);
        assert!(engine.allocate::<UserptrAqlQueueProbeGttV1>(4096).is_err());
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.free_calls, 0);
        assert_eq!(engine.backend.release_va_calls, 0);
    }

    let mut malformed = acquired();
    malformed.backend.corrupt_flags = true;
    assert!(
        malformed
            .allocate::<UserptrAqlQueueProbeGttV1>(4096)
            .is_err()
    );
    assert_eq!(malformed.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(malformed.backend.free_calls, 0);
    assert_eq!(malformed.backend.release_va_calls, 0);
}

#[test]
fn userptr_release_currentness_failures_retain_exact_terminal_custody() {
    for (currentness_call, mapping_retained, reservation_retained) in
        [(8, true, true), (9, false, true), (10, false, false)]
    {
        let mut engine = acquired();
        let ring = engine.allocate::<UserptrAqlQueueProbeGttV1>(4096).unwrap();
        engine.backend.fail_currentness_at = Some(currentness_call);

        assert!(matches!(
            engine.release(ring, SharedAllocationPhaseV1::CpuWritable),
            Err(MemorySessionError::Injected("currentness"))
        ));
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert!(engine.allocations[0].handle.is_none());
        assert_eq!(engine.allocations[0].mapping.is_some(), mapping_retained);
        assert_eq!(
            engine.allocations[0].reservation.is_some(),
            reservation_retained
        );
        assert_eq!(
            engine.allocations[0].phase,
            SharedAllocationPhaseV1::CpuWritable
        );
        assert_eq!(engine.retained_gpu_va_bytes, 4096);
        assert_eq!(engine.backend.free_calls, 1);
        assert_eq!(engine.backend.release_va_calls, 0);
        assert!(engine.backend.operations.ends_with(if mapping_retained {
            &["free"]
        } else {
            &["free", "unmap_cpu"]
        }));
        assert!(matches!(
            engine.allocate::<HostVisibleCoherentGttV1>(4096),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
    }
}

#[test]
fn bounds_and_aql_shape_fail_before_native_mutation() {
    let mut engine = acquired();
    assert!(engine.allocations.capacity() >= MAX_SHARED_GTT_ALLOCATIONS_V1);
    assert!(engine.device_memory.capacity() >= MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1);
    assert!(matches!(
        engine.allocate::<AqlQueueGttV1>(8193),
        Err(MemorySessionError::InvalidProfileSize(_))
    ));
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(usize::MAX),
        Err(MemorySessionError::SizeOverflow) | Err(MemorySessionError::InvalidProfileSize(_))
    ));
    assert!(engine.backend.flags.is_empty());
    let mut tokens = Vec::new();
    for _ in 0..MAX_SHARED_GTT_ALLOCATIONS_V1 {
        tokens.push(engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap());
    }
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(1),
        Err(MemorySessionError::SharedAllocationCapacity { .. })
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(tokens.len(), MAX_SHARED_GTT_ALLOCATIONS_V1);
}

#[test]
fn released_shared_allocations_do_not_consume_record_capacity() {
    let mut engine = acquired();
    let mut anchor = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
    engine
        .with_bytes_mut(&mut anchor, |bytes| bytes[0] = 0x5a)
        .unwrap();
    let cycles = MAX_SHARED_GTT_ALLOCATIONS_V1 * 4;
    let mut stale = None;
    let mut stale_slot = None;

    for cycle in 0..cycles {
        let token = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
        if cycle == 0 {
            stale = Some(SharedGttAllocationV1 {
                session_id: token.session_id,
                id: token.id,
                generation: token.generation,
                layout: token.layout,
                marker: PhantomData::<(HostVisibleCoherentGttV1, GttCpuWritableV1)>,
            });
            stale_slot = Some(engine.allocation_record_slots[&token.id]);
        }
        engine
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        assert!(engine.allocations.len() <= MAX_SHARED_GTT_ALLOCATIONS_V1);
    }

    let stale = stale.unwrap();
    assert!(
        !engine
            .allocations
            .iter()
            .any(|record| record.id == stale.id)
    );
    assert_eq!(
        engine
            .allocations
            .iter()
            .filter(|record| record.phase != SharedAllocationPhaseV1::Released)
            .count(),
        1
    );
    assert_eq!(engine.next_id, u64::try_from(cycles).unwrap() + 2);
    assert_eq!(engine.retained_gpu_va_bytes, 4096);
    assert_eq!(engine.backend.free_calls, cycles);
    assert_eq!(engine.backend.release_va_calls, cycles);

    engine.backend.next_handle = 2;
    let replacement = engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap();
    assert_eq!(
        engine.allocation_record_slots[&replacement.id],
        stale_slot.unwrap()
    );
    engine.shared_lookup_comparisons.set(0);
    let replacement_index = engine
        .index(&replacement, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(engine.shared_lookup_comparisons.get(), 1);
    assert_eq!(engine.allocations[replacement_index].handle, Some(2));
    assert!(matches!(
        engine.release(stale, SharedAllocationPhaseV1::CpuWritable),
        Err(MemorySessionError::InvalidAllocationAuthority)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(engine.backend.free_calls, cycles);
    engine
        .release(replacement, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(
        engine
            .with_bytes(&anchor, SharedAllocationPhaseV1::CpuWritable, |bytes| bytes
                [0])
            .unwrap(),
        0x5a
    );
    engine
        .release(anchor, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    assert_eq!(engine.retained_gpu_va_bytes, 0);
    assert_eq!(engine.backend.free_calls, cycles + 2);
    assert_eq!(engine.backend.release_va_calls, cycles + 2);
    assert!(engine.allocations.len() <= MAX_SHARED_GTT_ALLOCATIONS_V1);
}

#[test]
fn shared_slot_reuse_still_rejects_collision_with_a_later_live_record() {
    let mut engine = acquired();
    let mut tokens = Vec::new();
    for _ in 0..MAX_SHARED_GTT_ALLOCATIONS_V1 {
        tokens.push(engine.allocate::<HostVisibleCoherentGttV1>(1).unwrap());
    }
    let released = tokens.remove(0);
    engine
        .release(released, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    let later_live_va = engine.allocations[engine.allocation_record_slots[&tokens[0].id]].gpu_va;
    engine.backend.fixed_va = Some(later_live_va);

    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(1),
        Err(MemorySessionError::KernelResultMalformed(
            "overlapping GPU VA reservation"
        ))
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
}

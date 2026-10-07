use super::*;

#[test]
fn overlap_kernel_output_and_cpu_address_substitution_quarantine_globally() {
    let mut overlap = acquired();
    overlap.backend.fixed_va = Some(0x2_0000);
    let _first = overlap.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
    assert!(overlap.allocate::<KernargGttV1>(4096).is_err());
    assert_eq!(overlap.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(overlap.backend.flags.len(), 1);

    let mut malformed = acquired();
    malformed.backend.corrupt_flags = true;
    assert!(malformed.allocate::<ExecutableGttV1>(4096).is_err());
    assert_eq!(malformed.phase(), SharedMemorySessionPhaseV1::Quarantined);

    let mut non_identity = acquired();
    non_identity.backend.corrupt_mapping_address = true;
    assert!(matches!(
        non_identity.allocate::<HostVisibleCoherentGttV1>(4096),
        Err(MemorySessionError::KernelResultMalformed(
            "shared identity CPU/GPU VA mapping"
        ))
    ));
    assert_eq!(
        non_identity.phase(),
        SharedMemorySessionPhaseV1::Quarantined
    );
}

#[test]
fn later_allocation_failure_revokes_use_of_prior_tokens() {
    let mut engine = acquired();
    let first = engine.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
    engine.backend.alloc_oom = true;
    assert!(engine.allocate::<KernargGttV1>(4096).is_err());
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert!(matches!(
        engine.with_bytes(&first, SharedAllocationPhaseV1::CpuWritable, |_| ()),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
}

#[test]
fn completion_arena_allocation_oom_requires_quarantined_teardown() {
    let mut engine = acquired();
    engine.backend.alloc_oom = true;
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(
            crate::queue::completion::COMPLETION_SIGNAL_ARENA_BYTES_V1,
        ),
        Err(MemorySessionError::Syscall {
            operation: "AMDKFD_IOC_ALLOC_MEMORY_OF_GPU",
            ..
        })
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.alloc_calls, 1);
    let pending = engine.pending_allocation.as_ref().unwrap();
    assert_eq!(
        pending.stage,
        crate::shared_memory::allocation::PendingAllocationStageV1::Allocate
    );
    assert_eq!(pending.id, 1);
    assert_eq!(pending.record_slot, 0);
    assert_eq!(
        pending.layout.requested_bytes(),
        crate::queue::completion::COMPLETION_SIGNAL_ARENA_BYTES_V1
    );
    assert_eq!(engine.retained_gpu_va_bytes, pending.layout.gpu_va_bytes());
    assert_eq!(
        pending.reservation,
        Some((0x2_0000, pending.layout.gpu_va_bytes() as usize))
    );
    assert_eq!(
        pending.allocation_output,
        engine.backend.last_allocation_output
    );
    assert!(pending.allocation_output.unwrap().handle != 0);
    assert!(pending.mapping.is_none());
    assert!(engine.allocations.is_empty());
    assert_eq!(engine.next_id, 2);
}

#[test]
fn ambiguous_map_seal_and_free_are_terminal_without_retry() {
    let mut map = acquired();
    let token = map.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
    map.backend.map_errno = true;
    assert!(map.map_mutable(token).is_err());
    assert_eq!(map.phase(), SharedMemorySessionPhaseV1::Quarantined);

    let mut seal = acquired();
    let token = seal.allocate::<ExecutableGttV1>(4096).unwrap();
    seal.backend.fail_operation = Some("protect_cpu_read_only");
    assert!(seal.seal_executable(token).is_err());
    assert_eq!(seal.phase(), SharedMemorySessionPhaseV1::Quarantined);

    let mut free = acquired();
    let token = free.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
    free.backend.fail_operation = Some("free");
    assert!(
        free.release(token, SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
    assert_eq!(free.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(free.backend.free_calls, 1);

    let mut va_guard = acquired();
    let token = va_guard.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
    va_guard.backend.fail_operation = Some("release_va_reservation");
    assert!(
        va_guard
            .release(token, SharedAllocationPhaseV1::CpuWritable)
            .is_err()
    );
    assert_eq!(va_guard.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(va_guard.backend.free_calls, 1);
    assert_eq!(va_guard.backend.release_va_calls, 1);
}

#[test]
fn device_memory_profile_manifest_and_layout_are_frozen() {
    let digest = Sha256::digest(GFX942_DEVICE_MEMORY_LEASE_MANIFEST_V1);
    assert_eq!(
        digest.as_slice(),
        GFX942_DEVICE_MEMORY_LEASE_MANIFEST_SHA256_BYTES_V1
    );
    let mut digest_hex = String::with_capacity(64);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.iter().copied() {
        digest_hex.push(char::from(HEX[usize::from(byte >> 4)]));
        digest_hex.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    assert_eq!(digest_hex, GFX942_DEVICE_MEMORY_LEASE_MANIFEST_SHA256_V1);
    assert!(
        GFX942_DEVICE_MEMORY_LEASE_MANIFEST_V1
            .contains(fe2o3_kfd_uapi::KFD_DEVICE_MEMORY_LIFECYCLE_SCHEMA_MANIFEST_SHA256)
    );
    let initialization_digest = Sha256::digest(GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_V1);
    let initialization_hex: String = initialization_digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        initialization_hex,
        GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_SHA256_V1
    );
    assert!(
        GFX942_DEVICE_MEMORY_INITIALIZATION_MANIFEST_V1
            .contains(fe2o3_kfd_uapi::KFD_PUBLIC_DEVICE_MEMORY_SCHEMA_MANIFEST_SHA256)
    );

    assert!(matches!(
        device_memory_layout(0, 4096, KfdAllocMemoryFlags::DEVICE_LOCAL),
        Err(MemorySessionError::InvalidDeviceMemorySize)
    ));
    assert!(matches!(
        device_memory_layout(
            MAX_GFX942_DEVICE_MEMORY_BYTES_V1 + 1,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL,
        ),
        Err(MemorySessionError::InvalidDeviceMemorySize)
    ));
    for alignment in [0, 3, 8192] {
        assert!(matches!(
            device_memory_layout(1, alignment, KfdAllocMemoryFlags::DEVICE_LOCAL),
            Err(MemorySessionError::InvalidDeviceMemoryAlignment)
        ));
    }
    let layout = device_memory_layout(4097, 256, KfdAllocMemoryFlags::DEVICE_LOCAL).unwrap();
    assert_eq!(layout.requested_bytes(), 4097);
    assert_eq!(layout.backing_bytes(), 8192);
    assert_eq!(layout.alignment(), 256);
    assert_eq!(layout.uapi_flags(), 0x8000_0001);
}

#[test]
fn device_memory_lifecycle_is_linear_redacted_and_single_device() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let lease = engine
        .allocate_device_memory(device, vm, 4097, 256)
        .unwrap();
    assert_eq!(engine.backend.flags, vec![0x8000_0001]);
    assert_eq!(engine.backend.reserve_va_calls, 1);
    assert_eq!(engine.backend.alloc_calls, 1);
    assert_eq!(engine.backend.map_cpu_calls, 0);
    assert_eq!(engine.retained_device_memory_bytes, 8192);
    assert_eq!(engine.device_memory.len(), 1);
    assert_eq!(engine.device_memory[0].device, device);
    assert_eq!(engine.device_memory[0].vm, vm);

    let lease = engine.map_device_memory(lease).unwrap();
    assert_eq!(engine.backend.map_gpu_calls, 1);
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Mapped);
    let lease = engine.unmap_device_memory(lease).unwrap();
    assert_eq!(engine.backend.unmap_gpu_calls, 1);
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Unmapped);
    engine.release_device_memory(lease).unwrap();
    assert_eq!(engine.backend.free_calls, 1);
    assert_eq!(engine.backend.release_va_calls, 1);
    assert_eq!(engine.retained_device_memory_bytes, 0);
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Released);
}

#[test]
fn rounded_sdma_device_backing_preserves_custody_and_logical_copy_bounds() {
    use crate::persistent_directional_sdma::*;
    use crate::sdma::{Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1};
    use fe2o3_runtime_model::{QueueGenerationV1, QueueInstanceIdV1, QueueKeyV1};

    for bytes in [1, 4097, 1_048_832] {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let queue = QueueKeyV1 {
            vm,
            id: QueueInstanceIdV1(17),
            generation: QueueGenerationV1(3),
        };
        let (logical, physical) =
            crate::sdma::device_buffer_allocation_extents_v1(bytes, 4096).unwrap();
        let lease = engine
            .allocate_device_memory(device, vm, physical, 4096)
            .unwrap();
        let lease = engine.map_device_memory(lease).unwrap();
        let identity = lease.storage_identity();
        assert_eq!(lease.layout().requested_bytes(), physical);
        assert_eq!(lease.layout().backing_bytes(), physical);
        let buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Device(lease),
            queue,
            1,
            logical,
        );
        assert!(directional_persistent_sdma_extents_are_admitted_v1(
            buffer.requested_bytes(),
            buffer.physical_bytes(),
            buffer.pool_generation(),
        ));
        let pair = Gfx942PersistentDirectionalSdmaPairV1 {
            host_to_device_queue_id: 21,
            device_to_host_queue_id: 22,
        };
        let (allocation, debit) =
            promote_directional_persistent_sdma_custody_v1(buffer, pair, 1).unwrap();
        assert_eq!(allocation.byte_len(), bytes);
        assert_eq!(allocation.physical_byte_len(), physical);
        assert_eq!(debit, 1);
        let host = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Host(mapped_host_for_persistent_sdma_test(
                100,
                physical as usize,
            )),
            queue,
            1,
            physical,
        );
        let (allocation, host) = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
            queue,
            false,
            allocation,
            crate::Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host,
            0,
            bytes - 1,
            1,
        )
        .unwrap_or_else(|_| panic!("last logical byte must remain copyable"));
        let failure = crate::queue::admit_directional_persistent_sdma_copy_input_v1(
            queue,
            false,
            allocation,
            crate::Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host,
            0,
            bytes,
            1,
        )
        .expect_err("padding is outside the logical copy extent");
        let Gfx942DirectionalPersistentSdmaSubmissionCustodyV1::Retryable { allocation, host } =
            failure.into_parts().1
        else {
            panic!("range rejection must return exact custody");
        };
        let (allocation, host, packets) =
            crate::queue::admit_directional_persistent_sdma_window_input_v1(
                queue,
                false,
                allocation,
                crate::Gfx942PersistentSdmaDirectionV1::HostToDevice,
                host,
                0,
                bytes - 1,
                1,
            )
            .unwrap_or_else(|_| panic!("last logical byte must remain window-copyable"));
        assert_eq!(packets, 1);
        let failure = crate::queue::admit_directional_persistent_sdma_window_input_v1(
            queue,
            false,
            allocation,
            crate::Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host,
            0,
            bytes,
            1,
        )
        .expect_err("window admission cannot copy padding either");
        let Gfx942DirectionalPersistentSdmaWindowSubmissionCustodyV1::Retryable {
            allocation,
            host: _,
        } = failure.into_parts().1
        else {
            panic!("window range rejection must return exact custody");
        };
        let (buffer, debit) =
            demote_directional_persistent_sdma_custody_v1(allocation, debit).unwrap();
        assert_eq!(debit, 1);
        assert_eq!(buffer.requested_bytes(), bytes);
        assert_eq!(buffer.physical_bytes(), physical);
        assert_eq!(buffer.pool_generation(), 2);
        let (Gfx942SdmaBufferStorageV1::Device(lease), _, _, _) = buffer.into_bridge_parts() else {
            panic!("device custody changed kind");
        };
        assert_eq!(lease.storage_identity(), identity);
        let lease = engine.unmap_device_memory(lease).unwrap();
        engine.release_device_memory(lease).unwrap();
        assert_eq!(engine.backend.unmap_gpu_calls, 1);
        assert_eq!(engine.backend.free_calls, 1);
        assert_eq!(engine.backend.release_va_calls, 1);
        assert_eq!(engine.retained_device_memory_bytes, 0);
    }
}

#[test]
fn xgmi_mapping_retries_only_advanced_prefixes_and_retains_canonical_roster() {
    let mut engine = acquired();
    engine.backend.multi_map_script = vec![(1, true), (2, false)];
    engine.backend.multi_unmap_script = vec![(1, true), (2, false)];
    let lease = allocate_public_device_memory(&mut engine);
    let mapping = match engine.map_device_memory_to_gpus(lease, [7, 9].into()) {
        Ok(mapping) => mapping,
        Err(_) => panic!("advanced cumulative map prefix must be retried"),
    };
    assert!(mapping.is_fully_mapped());
    assert_eq!(mapping.gpu_ids(), [7, 9]);
    assert_eq!(
        engine.backend.multi_map_inputs,
        [(vec![7, 9], 0), (vec![7, 9], 1),]
    );
    let lease = match engine.unmap_device_memory_from_gpus(mapping) {
        Ok(lease) => lease,
        Err(_) => panic!("advanced cumulative unmap prefix must be retried"),
    };
    assert_eq!(
        engine.backend.multi_unmap_inputs,
        [(vec![7, 9], 0), (vec![7, 9], 1),]
    );
    engine.release_device_memory(lease).unwrap();
}

#[test]
fn xgmi_map_errno_at_full_prefix_never_grants_copy_authority() {
    let mut engine = acquired();
    engine.backend.multi_map_script = vec![(2, true)];
    let lease = allocate_public_device_memory(&mut engine);
    let failure = match engine.map_device_memory_to_gpus(lease, [7, 9].into()) {
        Err(failure) => failure,
        Ok(_) => panic!("errored full map prefix must not be admitted"),
    };
    let (_, recovery) = failure.into_parts();
    let mapping = match recovery {
        Gfx942XgmiMapRecoveryV1::PartiallyMapped(mapping) => mapping,
        Gfx942XgmiMapRecoveryV1::Unmapped(_) => panic!("full prefix may retain mappings"),
    };
    assert_eq!(mapping.mapped_prefix(), 2);
    assert!(!mapping.is_fully_mapped());
    let lease = match engine.unmap_device_memory_from_gpus(mapping) {
        Ok(lease) => lease,
        Err(_) => panic!("cleanup of the known mapped prefix must remain possible"),
    };
    engine.release_device_memory(lease).unwrap();
}

#[test]
fn xgmi_peer_post_currentness_failure_retains_mapped_cleanup_authority() {
    let failure = finish_xgmi_map_with_peer_post(
        Ok(xgmi_mapping_for_sdma_test(17)),
        Err(MemorySessionError::Injected("peer post currentness")),
    )
    .err()
    .unwrap();
    assert!(matches!(
        failure.error(),
        MemorySessionError::Injected("peer post currentness")
    ));
    let (_, recovery) = failure.into_parts();
    let mapping = match recovery {
        Gfx942XgmiMapRecoveryV1::PartiallyMapped(mapping) => mapping,
        Gfx942XgmiMapRecoveryV1::Unmapped(_) => {
            panic!("successful native map must retain mapped cleanup authority")
        }
    };
    assert_eq!(mapping.gpu_ids(), [7, 9]);
    assert!(mapping.is_fully_mapped());
}

#[test]
fn xgmi_peer_post_unmap_failure_retains_unmapped_free_authority() {
    let mapping = xgmi_mapping_for_sdma_test(18);
    let lease = mapping.lease.retag();
    let failure = finish_xgmi_unmap_with_peer_post(
        Ok(lease),
        Err(MemorySessionError::Injected("peer post unmap currentness")),
    )
    .err()
    .unwrap();
    assert!(matches!(
        failure.error(),
        MemorySessionError::Injected("peer post unmap currentness")
    ));
    assert!(matches!(
        failure.into_parts().1,
        Gfx942XgmiUnmapRecoveryV1::Unmapped(_)
    ));
}

#[test]
fn xgmi_unmap_errno_at_full_prefix_quarantines_without_release() {
    let mut engine = acquired();
    let lease = allocate_public_device_memory(&mut engine);
    let mapping = match engine.map_device_memory_to_gpus(lease, [7, 9].into()) {
        Ok(mapping) => mapping,
        Err(_) => panic!("full successful mapping expected"),
    };
    engine.backend.multi_unmap_script = vec![(2, true)];
    let failure = match engine.unmap_device_memory_from_gpus(mapping) {
        Err(failure) => failure,
        Ok(_) => panic!("errored full unmap prefix must remain indeterminate"),
    };
    let (_, recovery) = failure.into_parts();
    let mapping = match recovery {
        Gfx942XgmiUnmapRecoveryV1::PartiallyUnmapped(mapping) => mapping,
        Gfx942XgmiUnmapRecoveryV1::Unmapped(_) => {
            panic!("errored unmap must not mint unmapped authority")
        }
    };
    assert_eq!(mapping.unmapped_prefix(), 2);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(
        engine.device_memory[0].phase,
        DeviceMemoryPhaseV1::Ambiguous
    );
}

#[test]
fn xgmi_unmap_regressed_prefix_is_indeterminate_and_quarantined() {
    let mut engine = acquired();
    let lease = allocate_public_device_memory(&mut engine);
    let mapping = engine
        .map_device_memory_to_gpus(lease, [7, 9].into())
        .ok()
        .unwrap();
    engine.backend.multi_unmap_script = vec![(1, true), (0, true)];
    let failure = engine.unmap_device_memory_from_gpus(mapping).err().unwrap();
    let mapping = match failure.into_parts().1 {
        Gfx942XgmiUnmapRecoveryV1::PartiallyUnmapped(mapping) => mapping,
        Gfx942XgmiUnmapRecoveryV1::Unmapped(_) => panic!("regression cannot release"),
    };
    assert_eq!(mapping.unmapped_prefix, 1);
    assert!(mapping.unmap_indeterminate);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(
        engine.device_memory[0].phase,
        DeviceMemoryPhaseV1::Ambiguous
    );
    assert_eq!(engine.backend.free_calls, 0);
}

#[test]
fn xgmi_unmap_overshoot_is_indeterminate_and_quarantined() {
    let mut engine = acquired();
    let lease = allocate_public_device_memory(&mut engine);
    let mapping = engine
        .map_device_memory_to_gpus(lease, [7, 9].into())
        .ok()
        .unwrap();
    engine.backend.multi_unmap_script = vec![(3, true)];
    let failure = engine.unmap_device_memory_from_gpus(mapping).err().unwrap();
    let mapping = match failure.into_parts().1 {
        Gfx942XgmiUnmapRecoveryV1::PartiallyUnmapped(mapping) => mapping,
        Gfx942XgmiUnmapRecoveryV1::Unmapped(_) => panic!("overshoot cannot release"),
    };
    assert_eq!(mapping.unmapped_prefix, 0);
    assert!(mapping.unmap_indeterminate);
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(
        engine.device_memory[0].phase,
        DeviceMemoryPhaseV1::Ambiguous
    );
    assert_eq!(engine.backend.free_calls, 0);
}

#[test]
fn public_device_memory_initialization_checks_mapped_bytes_before_gpu_map() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let bytes = vec![0x5a; 4097];
    let descriptor = content(&bytes);
    let byte_len = bytes.len();
    let source = validate_initialization_source(bytes.into_boxed_slice(), descriptor).unwrap();
    let lease = engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            byte_len as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
    let initialized = engine
        .initialize_public_device_memory(lease, source)
        .unwrap();
    assert_eq!(engine.backend.flags, vec![0xa000_0001]);
    assert_eq!(engine.backend.map_cpu_calls, 1);
    assert_eq!(engine.backend.map_gpu_calls, 1);
    assert_eq!(engine.backend.last_unmapped_readback_calls, 1);
    assert!(engine.device_memory[0].mapping.is_none());
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Mapped);
    assert_eq!(initialized.content(), descriptor);
    assert_eq!(initialized.layout().requested_bytes(), byte_len as u64);

    let (lease, retained) = initialized.into_parts();
    assert_eq!(retained, descriptor);
    let lease = engine.unmap_device_memory(lease).unwrap();
    engine.release_device_memory(lease).unwrap();
    assert_eq!(engine.retained_device_memory_bytes, 0);
}

#[test]
fn public_device_initialization_plan_is_bounded_and_covers_uneven_extents() {
    assert_eq!(public_device_initialization_plan(0, 0), None);
    assert_eq!(
        public_device_initialization_plan(67, 4),
        Some(PublicDeviceInitializationPlanV1 {
            chunk_bytes: 17,
            chunk_count: 4,
        })
    );
    assert_eq!(
        public_device_initialization_plan(3, usize::MAX),
        Some(PublicDeviceInitializationPlanV1 {
            chunk_bytes: 1,
            chunk_count: 3,
        })
    );

    let mut bytes = (0_u8..67).collect::<Vec<_>>();
    fill_repeated_byte_with_workers(&mut bytes, 0xa5, 4).unwrap();
    assert_eq!(bytes, vec![0xa5; 67]);
}

#[test]
fn repeated_byte_fill_threshold_keeps_small_inputs_serial() {
    assert_eq!(
        public_device_initialization_worker_count(
            PUBLIC_DEVICE_PARALLEL_FILL_THRESHOLD_BYTES_V1 - 1,
            usize::MAX,
        ),
        1
    );
    assert_eq!(
        public_device_initialization_worker_count(
            PUBLIC_DEVICE_PARALLEL_FILL_THRESHOLD_BYTES_V1,
            usize::MAX,
        ),
        4
    );
    assert_eq!(
        public_device_initialization_worker_count(
            PUBLIC_DEVICE_PARALLEL_FILL_THRESHOLD_BYTES_V1,
            0,
        ),
        1
    );
    assert_eq!(
        public_device_initialization_worker_count(usize::MAX, usize::MAX),
        MAX_PUBLIC_DEVICE_PARALLEL_FILL_WORKERS_V1
    );
}

#[test]
fn arbitrary_source_copy_and_distinct_verification_cover_partition_edges() {
    for (byte_len, workers) in [(1, 1), (3, usize::MAX), (17, 4), (67, 4)] {
        let source = (0..byte_len)
            .map(|index| (index as u8).wrapping_mul(37))
            .collect::<Vec<_>>();
        let mut mapped = vec![0xff; byte_len];
        write_disjoint_source_partitions(
            &mut mapped,
            &source,
            workers,
            |_| true,
            |destination, source| destination.copy_from_slice(source),
        )
        .unwrap();
        assert_eq!(mapped, source);
        verify_disjoint_source_partitions(
            &mapped,
            &source,
            workers,
            |_| true,
            |left, right| left == right,
        )
        .unwrap();
    }
}

#[test]
fn arbitrary_source_verification_rejects_partial_wrong_and_length_substitution() {
    let source = (0_u8..67).collect::<Vec<_>>();
    let mut mapped = vec![0xff; source.len()];
    write_disjoint_source_partitions(
        &mut mapped,
        &source,
        4,
        |_| true,
        |destination, source| {
            let copied = destination.len().saturating_sub(1);
            destination[..copied].copy_from_slice(&source[..copied]);
        },
    )
    .unwrap();
    assert!(matches!(
        verify_disjoint_source_partitions(
            &mapped,
            &source,
            4,
            |_| true,
            |left, right| left == right,
        ),
        Err(MemorySessionError::DeviceContentMismatch)
    ));

    mapped.copy_from_slice(&source);
    mapped[34] ^= 1;
    assert!(matches!(
        verify_disjoint_source_partitions(
            &mapped,
            &source,
            4,
            |_| true,
            |left, right| left == right,
        ),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
    assert!(matches!(
        verify_disjoint_source_partitions(
            &mapped[..66],
            &source,
            4,
            |_| true,
            |left, right| left == right,
        ),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
}

#[test]
fn arbitrary_source_worker_spawn_and_panic_fail_closed_by_phase() {
    let source = (0_u8..67).collect::<Vec<_>>();
    let mut mapped = vec![0xff; source.len()];
    assert!(matches!(
        write_disjoint_source_partitions(
            &mut mapped,
            &source,
            4,
            |index| index != 2,
            |destination, source| destination.copy_from_slice(source),
        ),
        Err(MemorySessionError::DeviceInitializationWriteFailed)
    ));
    assert!(matches!(
        write_disjoint_source_partitions(
            &mut mapped,
            &source,
            4,
            |_| true,
            |_, _| panic!("injected arbitrary-source writer panic"),
        ),
        Err(MemorySessionError::DeviceInitializationWriteFailed)
    ));

    mapped.copy_from_slice(&source);
    assert!(matches!(
        verify_disjoint_source_partitions(
            &mapped,
            &source,
            4,
            |index| index != 2,
            |left, right| left == right,
        ),
        Err(MemorySessionError::DeviceInitializationVerificationFailed)
    ));
    assert!(matches!(
        verify_disjoint_source_partitions(
            &mapped,
            &source,
            4,
            |_| true,
            |_, _| panic!("injected arbitrary-source verifier panic"),
        ),
        Err(MemorySessionError::DeviceInitializationVerificationFailed)
    ));
}

#[test]
fn repeated_byte_fill_handles_empty_and_hostile_worker_bounds_without_panicking() {
    let mut empty = [];
    fill_repeated_byte_with_workers(&mut empty, 0x11, 0).unwrap();
    fill_repeated_byte_with_workers(&mut empty, 0x22, usize::MAX).unwrap();

    let mut single = [0_u8];
    fill_repeated_byte_with_workers(&mut single, 0x33, 0).unwrap();
    assert_eq!(single, [0x33]);
    fill_repeated_byte_with_workers(&mut single, 0x44, usize::MAX).unwrap();
    assert_eq!(single, [0x44]);
}

#[test]
fn repeated_byte_worker_panic_becomes_a_typed_failure() {
    let mut bytes = [0_u8; 17];
    let result = write_disjoint_repeated_byte_partitions(&mut bytes, 4, |_| {
        panic!("injected repeated-byte writer panic");
    });
    assert!(matches!(
        result,
        Err(MemorySessionError::DeviceInitializationWriteFailed)
    ));
}

#[test]
fn repeated_byte_initialization_fills_without_readback_and_releases_exact_extent() {
    for (byte_len, repeated_byte) in [(1_u64, 0_u8), (4096, 0x5a), (4097, 0xff)] {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let initialization = repeated_content(byte_len, repeated_byte);
        let lease = engine
            .allocate_device_memory_with_flags(
                device,
                vm,
                byte_len,
                4096,
                KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
            )
            .unwrap();
        engine.backend.corrupt_readback = true;
        let initialized = engine
            .initialize_public_device_memory_repeated_byte(lease, initialization)
            .unwrap();

        assert_eq!(initialized.content(), initialization.content());
        assert_eq!(initialized.layout().requested_bytes(), byte_len);
        assert_eq!(engine.backend.flags, vec![0xa000_0001]);
        assert_eq!(engine.backend.map_cpu_calls, 1);
        assert_eq!(engine.backend.map_gpu_calls, 1);
        assert_eq!(engine.backend.last_unmapped_readback_calls, 0);
        assert_eq!(
            engine.backend.operations,
            ["map_cpu", "prepare_cpu_mapping", "unmap_cpu", "map_gpu"]
        );
        assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Mapped);
        assert!(engine.device_memory[0].mapping.is_none());
        let mapped = engine.backend.last_unmapped_bytes.as_ref().unwrap();
        let logical_len = byte_len as usize;
        assert!(
            mapped[..logical_len]
                .iter()
                .all(|byte| *byte == repeated_byte)
        );
        assert!(mapped[logical_len..].iter().all(|byte| *byte == 0));

        let (lease, content) = initialized.into_parts();
        assert_eq!(content, initialization.content());
        let lease = engine.unmap_device_memory(lease).unwrap();
        engine.release_device_memory(lease).unwrap();
        assert_eq!(engine.backend.free_calls, 1);
        assert_eq!(engine.backend.release_va_calls, 1);
        assert_eq!(engine.retained_device_memory_bytes, 0);
        assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Released);
    }
}

#[test]
fn repeated_byte_write_failure_quarantines_before_unmap_or_gpu_map() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let byte_len = 4097_u64;
    let lease = engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            byte_len,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();

    let result = engine.initialize_public_device_memory_after_preflight(
        &lease,
        byte_len as usize,
        |_| Err(MemorySessionError::DeviceInitializationWriteFailed),
        None,
        &mut crate::shared_memory::device_initialization::DeviceInitializationStageV1::Admission,
    );

    assert!(matches!(
        result,
        Err(MemorySessionError::DeviceInitializationWriteFailed)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.map_gpu_calls, 0);
    assert!(engine.device_memory[0].mapping.is_some());
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Unmapped);
}

#[test]
fn arbitrary_partial_write_quarantines_before_cpu_unmap_or_gpu_map() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let bytes = vec![0x5a; 4097];
    let lease = engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            bytes.len() as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();

    let result = engine.initialize_public_device_memory_after_preflight(
        &lease,
        bytes.len(),
        |mapped| {
            mapped[..bytes.len() - 1].copy_from_slice(&bytes[..bytes.len() - 1]);
            Ok(())
        },
        Some(&bytes),
        &mut crate::shared_memory::device_initialization::DeviceInitializationStageV1::Admission,
    );

    assert!(matches!(
        result,
        Err(MemorySessionError::DeviceContentMismatch)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.map_gpu_calls, 0);
    assert_eq!(
        engine.backend.operations,
        ["map_cpu", "prepare_cpu_mapping"]
    );
    assert!(engine.device_memory[0].mapping.is_some());
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Unmapped);
}

#[test]
fn arbitrary_owned_bytes_still_reject_a_readback_mismatch() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let bytes = vec![0x3c; 4097];
    let role = crate::Gfx942DeviceContentRoleV1::new([0x62; 32], 8).unwrap();
    let content = Gfx942DeviceContentDescriptorV1::from_bytes(role, &bytes).unwrap();
    let byte_len = bytes.len();
    let source = validate_initialization_source(bytes.into_boxed_slice(), content).unwrap();
    let lease = engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            byte_len as u64,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
    engine.backend.corrupt_readback = true;

    assert!(matches!(
        engine.initialize_public_device_memory(lease, source),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.map_cpu_calls, 1);
    assert_eq!(engine.backend.map_gpu_calls, 0);
    assert_eq!(engine.retained_device_memory_bytes, 8192);
    assert_eq!(engine.device_memory[0].phase, DeviceMemoryPhaseV1::Unmapped);
    assert!(engine.device_memory[0].mapping.is_some());
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

#[test]
fn initialization_source_preflight_rejects_digest_length_and_empty_substitution() {
    let bytes = vec![0x5a; 4096].into_boxed_slice();
    let descriptor = content(&bytes);
    let wrong_digest = content(&vec![0xa5; 4096]);
    let wrong_length = Gfx942DeviceContentDescriptorV1::new(
        descriptor.role(),
        descriptor.byte_len() - 1,
        descriptor.sha256(),
    )
    .unwrap();
    assert!(matches!(
        validate_initialization_source(bytes.clone(), wrong_digest),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
    assert!(matches!(
        validate_initialization_source(bytes, wrong_length),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
    assert!(matches!(
        validate_initialization_source(Vec::new().into_boxed_slice(), descriptor),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
}

#[test]
fn initialization_source_preflight_rejects_mutation_before_ownership_transfer() {
    let mut bytes = vec![0x5a; 4096];
    let descriptor = content(&bytes);
    bytes[2048] ^= 1;
    assert!(matches!(
        validate_initialization_source(bytes.into_boxed_slice(), descriptor),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
}

#[test]
fn validated_initialization_source_takes_the_exact_box_without_copying() {
    let bytes = vec![0x5a; 4096].into_boxed_slice();
    let descriptor = content(&bytes);
    let pointer = bytes.as_ptr();
    let source = validate_initialization_source(bytes, descriptor).unwrap();
    assert_eq!(source.bytes().as_ptr(), pointer);
    assert_eq!(source.byte_len(), 4096);
    assert_eq!(source.content(), descriptor);
}

#[test]
fn internal_post_allocation_length_substitution_quarantines() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let bytes = vec![0x5a; 4096];
    let descriptor = content(&bytes);
    let source = validate_initialization_source(bytes.into_boxed_slice(), descriptor).unwrap();
    let lease = engine
        .allocate_device_memory_with_flags(
            device,
            vm,
            source.byte_len() + 1,
            4096,
            KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,
        )
        .unwrap();
    assert!(matches!(
        engine.initialize_public_device_memory(lease, source),
        Err(MemorySessionError::DeviceContentMismatch)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.backend.map_cpu_calls, 0);
    assert_eq!(engine.backend.map_gpu_calls, 0);
}

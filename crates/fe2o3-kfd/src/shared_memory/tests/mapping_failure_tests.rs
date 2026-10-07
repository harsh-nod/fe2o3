use super::*;

#[test]
fn public_device_memory_mapping_failures_quarantine_without_gpu_publication() {
    for operation in ["map_cpu", "prepare_cpu_mapping", "unmap_cpu"] {
        let mut engine = acquired();
        let (device, vm) = device_vm(7);
        let bytes = vec![0x3c; 4096];
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
        engine.backend.fail_operation = Some(operation);
        assert!(
            engine
                .initialize_public_device_memory(lease, source)
                .is_err()
        );
        assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(engine.backend.map_gpu_calls, 0);
    }
}

#[test]
fn arbitrary_source_has_one_preflight_hash_and_parallel_exact_readback() {
    let source = include_str!("../../shared_memory.rs");
    let validation = source
        .split("fn validate_initialization_source_bytes(")
        .nth(1)
        .unwrap()
        .split("fn device_memory_layout(")
        .next()
        .unwrap();
    assert!(validation.contains("bytes: &[u8]"));
    assert_eq!(validation.matches("Sha256::digest(bytes)").count(), 1);

    let custody = include_str!("../../shared_memory/device_initialization.rs");
    let metadata = custody
        .split("fn source_metadata(")
        .nth(1)
        .unwrap()
        .split("fn admitted(")
        .next()
        .unwrap();
    assert!(
        metadata
            .find("validate_initialization_source_bytes(bytes, *content)?")
            .unwrap()
            < metadata.find("self.source.take()").unwrap()
    );
    assert!(metadata.contains("ValidatedInitializationSourceV1 {"));
    let initialization = custody
        .split("fn prepare_inner<")
        .nth(1)
        .unwrap()
        .split("fn take_complete(")
        .next()
        .unwrap();
    assert!(
        initialization.find("self.source_metadata()?").unwrap()
            < initialization
                .find("engine.allocate_device_memory_with_flags_inner(")
                .unwrap()
    );
    assert!(!initialization.contains("Sha256::digest"));
    assert!(initialization.contains("copy_public_device_mapping(mapped, source.bytes())"));
    assert!(initialization.contains("Some(source.bytes())"));
    assert!(
        initialization
            .find(".initialize_public_device_memory_after_preflight(")
            .unwrap()
            < initialization
                .find("engine.map_device_memory_borrowed(lease, &mut self.progress)?")
                .unwrap()
    );

    let mapped_preflight = source
        .split("fn initialize_public_device_memory_after_preflight(")
        .nth(1)
        .unwrap()
        .split("fn with_unmapped_public_device_memory<R>(")
        .next()
        .unwrap();
    assert!(!mapped_preflight.contains("Sha256::digest(mapped)"));
    assert!(mapped_preflight.contains("verify_public_device_mapping(mapped, source)"));

    let verification = source
        .split("fn verify_public_device_mapping(")
        .nth(1)
        .unwrap()
        .split("fn write_disjoint_source_partitions(")
        .next()
        .unwrap();
    assert!(verification.contains("verify_disjoint_source_partitions"));

    let workers = source
        .split("fn write_disjoint_source_partitions(")
        .nth(1)
        .unwrap()
        .split("fn verify_disjoint_source_partitions(")
        .next()
        .unwrap();
    assert!(workers.contains("chunks_mut(plan.chunk_bytes)"));
    assert!(workers.contains("source.chunks(plan.chunk_bytes)"));
    assert!(workers.contains("spawn_scoped"));

    let public_entry = source
        .split("pub fn initialize_gfx942_device_memory(")
        .nth(1)
        .unwrap()
        .split("pub fn initialize_gfx942_device_memory_repeated_byte(")
        .next()
        .unwrap();
    assert!(public_entry.contains("device_initialization::initialize_bytes_v1("));
    let bytes_entry = custody
        .split("fn initialize_bytes_v1<")
        .nth(1)
        .unwrap()
        .split("fn initialize_repeated_v1<")
        .next()
        .unwrap();
    assert!(bytes_entry.contains("finish_v1("));
    assert!(bytes_entry.contains("InitializationSourceV1::Unvalidated(bytes, content)"));
    let finish = custody
        .split("fn finish_v1<")
        .nth(1)
        .unwrap()
        .split("fn initialize_bytes_v1<")
        .next()
        .unwrap();
    assert!(finish.contains("custody.prepare_in_place(engine, request)?"));
}

#[test]
fn dispatch_transfer_requires_exact_complete_distinct_mapped_lease_set() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let first = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .and_then(|lease| engine.map_device_memory(lease))
        .unwrap();
    let second = engine
        .allocate_device_memory(device, vm, 8192, 4096)
        .and_then(|lease| engine.map_device_memory(lease))
        .unwrap();
    let authority = |lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>| {
        let record = engine
            .device_memory
            .iter()
            .find(|record| record.id == lease.id)
            .unwrap();
        Gfx942DeviceMemoryDispatchAuthorityV1 {
            facts: Gfx942DeviceMemoryDispatchFactsV1 {
                id: record.id,
                generation: record.generation,
                device: record.device,
                vm: record.vm,
                gpu_va: record.gpu_va,
                layout: record.layout,
            },
            lease,
        }
    };
    let mut exact = [authority(first), authority(second)];
    let exact_refs = [&exact[0], &exact[1]];
    assert!(
        engine
            .validate_complete_dispatch_device_memory_set(&exact_refs, device, vm)
            .is_ok()
    );
    assert!(matches!(
        engine.validate_complete_dispatch_device_memory_set(&exact_refs[..1], device, vm),
        Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
    ));

    exact[1].facts = exact[0].facts;
    let exact_refs = [&exact[0], &exact[1]];
    assert!(matches!(
        engine.validate_complete_dispatch_device_memory_set(&exact_refs, device, vm),
        Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
    ));
    exact[1].facts.generation += 1;
    let exact_refs = [&exact[0], &exact[1]];
    assert!(matches!(
        engine.validate_complete_dispatch_device_memory_set(&exact_refs, device, vm),
        Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn live_dispatch_validation_accepts_an_exact_bound_subset() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let bound = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .and_then(|lease| engine.map_device_memory(lease))
        .unwrap();
    let _unrelated = engine
        .allocate_device_memory(device, vm, 8192, 4096)
        .and_then(|lease| engine.map_device_memory(lease))
        .unwrap();
    let record = engine
        .device_memory
        .iter()
        .find(|record| record.id == bound.id)
        .unwrap();
    let authority = Gfx942DeviceMemoryDispatchAuthorityV1 {
        facts: Gfx942DeviceMemoryDispatchFactsV1 {
            id: record.id,
            generation: record.generation,
            device: record.device,
            vm: record.vm,
            gpu_va: record.gpu_va,
            layout: record.layout,
        },
        lease: bound,
    };

    assert!(matches!(
        engine.validate_complete_dispatch_device_memory_set(&[&authority], device, vm),
        Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
    ));
    engine
        .validate_live_queue_dispatch_memory(&[&authority], device, vm)
        .unwrap();
    engine
        .validate_persistent_replay_dispatch_memory(&[&authority], device, vm)
        .unwrap();

    let mut foreign = authority.facts;
    foreign.generation += 1;
    let foreign = Gfx942DeviceMemoryDispatchAuthorityV1 {
        facts: foreign,
        lease: authority.lease,
    };
    assert!(matches!(
        engine.validate_live_queue_dispatch_memory(&[&foreign], device, vm),
        Err(MemorySessionError::DeviceMemoryQueueBindingRequired)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn persistent_replay_uses_only_operational_currentness_before_bound_subset_validation() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let full_before = engine.backend.currentness_calls;
    assert_eq!(engine.backend.operational_currentness_calls, 0);

    engine
        .validate_live_queue_dispatch_memory(&[], device, vm)
        .unwrap();
    assert_eq!(engine.backend.currentness_calls, full_before + 1);
    assert_eq!(engine.backend.operational_currentness_calls, 0);

    engine
        .validate_persistent_replay_dispatch_memory(&[], device, vm)
        .unwrap();
    assert_eq!(engine.backend.currentness_calls, full_before + 1);
    assert_eq!(engine.backend.operational_currentness_calls, 1);

    engine.backend.fail_operational_currentness_at = Some(2);
    assert!(matches!(
        engine.validate_persistent_replay_dispatch_memory(&[], device, vm),
        Err(MemorySessionError::Injected("operational_currentness"))
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
}

#[test]
fn returned_dispatch_session_preserves_and_releases_the_exact_mapped_c3_lease() {
    let mut engine = acquired();
    let (device, vm) = device_vm(7);
    let lease = engine
        .allocate_device_memory(device, vm, 8192, 4096)
        .and_then(|lease| engine.map_device_memory(lease))
        .unwrap();
    let record = engine
        .device_memory
        .iter()
        .find(|record| record.id == lease.id)
        .unwrap();
    let expected = (
        record.id,
        record.generation,
        record.device,
        record.vm,
        record.layout,
    );
    let authority = Gfx942DeviceMemoryDispatchAuthorityV1 {
        facts: Gfx942DeviceMemoryDispatchFactsV1 {
            id: record.id,
            generation: record.generation,
            device: record.device,
            vm: record.vm,
            gpu_va: record.gpu_va,
            layout: record.layout,
        },
        lease,
    };

    assert_eq!(
        (
            authority.facts.id,
            authority.facts.generation,
            authority.facts.device,
            authority.facts.vm,
            authority.facts.layout,
        ),
        expected
    );
    let returned_session = (engine, authority.into_lease());
    let (mut engine, returned) = returned_session;
    assert_eq!(
        (
            returned.id,
            returned.generation,
            returned.device,
            returned.vm,
            returned.layout,
        ),
        expected
    );
    let returned = engine.unmap_device_memory(returned).unwrap();
    engine.release_device_memory(returned).unwrap();
    assert_eq!(engine.backend.unmap_gpu_calls, 1);
    assert_eq!(engine.backend.free_calls, 1);
    assert_eq!(engine.backend.release_va_calls, 1);
    assert_eq!(engine.retained_device_memory_bytes, 0);
}

#[test]
fn device_memory_oom_retains_possible_native_authority_and_poison() {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine.backend.alloc_oom = true;
    assert!(matches!(
        engine.allocate_device_memory(device, vm, 4096, 4096),
        Err(MemorySessionError::Syscall {
            source: rustix::io::Errno::NOMEM,
            ..
        })
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(engine.device_memory.len(), 1);
    assert!(engine.device_memory[0].reservation.is_some());
    assert!(engine.device_memory[0].handle.is_some());
    assert_eq!(
        engine.device_memory[0].phase,
        DeviceMemoryPhaseV1::Ambiguous
    );
    assert_eq!(engine.retained_device_memory_bytes, 4096);
    assert_eq!(engine.backend.free_calls, 0);
    assert_eq!(engine.backend.release_va_calls, 0);
}

#[test]
fn device_memory_rejects_wrong_device_generation_and_address_overflow() {
    let mut mismatch = acquired();
    let (device, vm) = device_vm(1);
    let (other_device, _) = device_vm(2);
    assert!(matches!(
        mismatch.allocate_device_memory(other_device, vm, 4096, 4096),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(mismatch.backend.reserve_va_calls, 0);

    let lease = mismatch
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let substituted = Gfx942DeviceMemoryLeaseV1 {
        id: lease.id,
        generation: lease.generation,
        device: other_device,
        vm: VmKeyV1 {
            device: other_device,
            id: lease.vm.id,
        },
        layout: lease.layout,
        marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
    };
    let stale_generation = Gfx942DeviceMemoryLeaseV1 {
        id: lease.id,
        generation: lease.generation + 1,
        device: lease.device,
        vm: lease.vm,
        layout: lease.layout,
        marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
    };
    assert!(matches!(
        mismatch.map_device_memory(substituted),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert!(matches!(
        mismatch.map_device_memory(stale_generation),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(mismatch.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(mismatch.backend.map_gpu_calls, 0);

    let mut overflow = acquired();
    overflow.backend.fixed_va = Some(u64::MAX - 2047);
    assert!(
        overflow
            .allocate_device_memory(device, vm, 4096, 4096)
            .is_err()
    );
    assert_eq!(overflow.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(overflow.backend.alloc_calls, 0);
    assert!(overflow.device_memory[0].reservation.is_some());
}

#[test]
fn device_record_slot_index_is_non_authoritative_and_fully_authenticated() {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    let first = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let second = engine
        .allocate_device_memory(device, vm, 8192, 4096)
        .unwrap();
    let first_slot = engine.device_memory_record_slots[&first.id];
    let second_slot = engine.device_memory_record_slots[&second.id];
    let forge = |generation, layout| Gfx942DeviceMemoryLeaseV1 {
        id: first.id,
        generation,
        device: first.device,
        vm: first.vm,
        layout,
        marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
    };

    engine
        .device_memory_record_slots
        .insert(first.id, second_slot);
    let wrong_in_range = forge(first.generation, first.layout);
    assert_eq!(
        engine
            .device_memory_index(&wrong_in_range, DeviceMemoryPhaseV1::Unmapped)
            .unwrap(),
        first_slot
    );
    engine
        .device_memory_record_slots
        .insert(first.id, usize::MAX);
    let out_of_range = forge(first.generation, first.layout);
    assert_eq!(
        engine
            .device_memory_index(&out_of_range, DeviceMemoryPhaseV1::Unmapped)
            .unwrap(),
        first_slot
    );
    engine
        .device_memory_record_slots
        .insert(first.id, first_slot);
    let stale = forge(first.generation + 1, first.layout);
    assert!(matches!(
        engine.device_memory_index(&stale, DeviceMemoryPhaseV1::Unmapped),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert!(matches!(
        engine.device_memory_index(&first, DeviceMemoryPhaseV1::Mapped),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    let mut forged_layout = first.layout;
    forged_layout.requested_bytes += 1;
    let forged = forge(first.generation, forged_layout);
    assert!(matches!(
        engine.device_memory_index(&forged, DeviceMemoryPhaseV1::Unmapped),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn device_record_slot_index_lookup_work_is_constant_across_bounded_depths() {
    let (device, vm) = device_vm(1);
    for count in [1, 32, MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1] {
        let mut engine = acquired();
        let mut leases = Vec::new();
        for _ in 0..count {
            leases.push(engine.allocate_device_memory(device, vm, 1, 1).unwrap());
        }
        for lease in &leases {
            engine.device_lookup_comparisons.set(0);
            assert_eq!(
                engine
                    .device_memory_index(lease, DeviceMemoryPhaseV1::Unmapped)
                    .unwrap(),
                engine.device_memory_record_slots[&lease.id]
            );
            assert_eq!(engine.device_lookup_comparisons.get(), 1);
        }
    }
}

#[test]
fn device_memory_map_and_unmap_ambiguity_retain_and_poison() {
    let (device, vm) = device_vm(1);

    let mut map_zero = acquired();
    let lease = map_zero
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    map_zero.backend.map_progress = 0;
    assert!(map_zero.map_device_memory(lease).is_err());
    assert_eq!(map_zero.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert!(map_zero.device_memory[0].handle.is_some());
    assert!(map_zero.device_memory[0].reservation.is_some());

    let mut map_errno = acquired();
    let lease = map_errno
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    map_errno.backend.map_errno = true;
    assert!(map_errno.map_device_memory(lease).is_err());
    assert_eq!(map_errno.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(map_errno.backend.free_calls, 0);

    let mut unmap_errno = acquired();
    let lease = unmap_errno
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let lease = unmap_errno.map_device_memory(lease).unwrap();
    unmap_errno.backend.unmap_errno = true;
    assert!(unmap_errno.unmap_device_memory(lease).is_err());
    assert_eq!(unmap_errno.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(
        unmap_errno.device_memory[0].phase,
        DeviceMemoryPhaseV1::Ambiguous
    );
    assert_eq!(unmap_errno.backend.free_calls, 0);
}

#[test]
fn device_memory_free_and_va_release_ambiguity_are_never_retried() {
    let (device, vm) = device_vm(1);

    let mut free = acquired();
    let lease = free.allocate_device_memory(device, vm, 4096, 4096).unwrap();
    free.backend.fail_operation = Some("free");
    assert!(free.release_device_memory(lease).is_err());
    assert_eq!(free.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(free.backend.free_calls, 1);
    assert_eq!(free.backend.release_va_calls, 0);
    assert!(free.device_memory[0].handle.is_some());
    assert!(free.device_memory[0].reservation.is_some());

    let mut va = acquired();
    let lease = va.allocate_device_memory(device, vm, 4096, 4096).unwrap();
    va.backend.fail_operation = Some("release_va_reservation");
    assert!(va.release_device_memory(lease).is_err());
    assert_eq!(va.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(va.backend.free_calls, 1);
    assert_eq!(va.backend.release_va_calls, 1);
    assert!(va.device_memory[0].handle.is_none());
    assert!(va.device_memory[0].reservation.is_some());
}

#[test]
fn device_memory_post_side_effect_currentness_failures_retain_and_poison() {
    let (device, vm) = device_vm(1);

    let mut allocation = acquired();
    allocation.backend.fail_currentness_at = Some(4);
    assert!(
        allocation
            .allocate_device_memory(device, vm, 4096, 4096)
            .is_err()
    );
    assert_eq!(allocation.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert!(allocation.device_memory[0].handle.is_some());
    assert!(allocation.device_memory[0].reservation.is_some());

    let mut map = acquired();
    let lease = map.allocate_device_memory(device, vm, 4096, 4096).unwrap();
    map.backend.fail_currentness_at = Some(6);
    assert!(map.map_device_memory(lease).is_err());
    assert_eq!(map.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(map.device_memory[0].phase, DeviceMemoryPhaseV1::Ambiguous);

    let mut unmap = acquired();
    let lease = unmap
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let lease = unmap.map_device_memory(lease).unwrap();
    unmap.backend.fail_currentness_at = Some(8);
    assert!(unmap.unmap_device_memory(lease).is_err());
    assert_eq!(unmap.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert_eq!(unmap.device_memory[0].phase, DeviceMemoryPhaseV1::Ambiguous);

    let mut free = acquired();
    let lease = free.allocate_device_memory(device, vm, 4096, 4096).unwrap();
    free.backend.fail_currentness_at = Some(6);
    assert!(free.release_device_memory(lease).is_err());
    assert_eq!(free.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert!(free.device_memory[0].handle.is_none());
    assert!(free.device_memory[0].reservation.is_some());

    let mut va_release = acquired();
    let lease = va_release
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    va_release.backend.fail_currentness_at = Some(7);
    assert!(va_release.release_device_memory(lease).is_err());
    assert_eq!(va_release.phase(), SharedMemorySessionPhaseV1::Quarantined);
    assert!(va_release.device_memory[0].handle.is_none());
    assert!(va_release.device_memory[0].reservation.is_none());
    assert_eq!(va_release.retained_device_memory_bytes, 4096);
}

#[test]
fn released_device_memory_rejects_forged_double_release_and_use() {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    let lease = engine
        .allocate_device_memory(device, vm, 4096, 4096)
        .unwrap();
    let forge = || Gfx942DeviceMemoryLeaseV1 {
        id: lease.id,
        generation: lease.generation,
        device: lease.device,
        vm: lease.vm,
        layout: lease.layout,
        marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
    };
    let double_release = forge();
    let use_after_release = forge();
    engine.release_device_memory(lease).unwrap();
    assert!(matches!(
        engine.release_device_memory(double_release),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert!(matches!(
        engine.map_device_memory(use_after_release),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(engine.backend.free_calls, 1);
    assert_eq!(engine.backend.release_va_calls, 1);
}

#[test]
fn device_memory_capacity_is_preflighted_and_success_reclaims_bytes() {
    let mut bytes = acquired();
    let (device, vm) = device_vm(1);
    let lease = bytes
        .allocate_device_memory(device, vm, MAX_GFX942_DEVICE_MEMORY_BYTES_V1, 4096)
        .unwrap();
    assert!(matches!(
        bytes.allocate_device_memory(device, vm, 1, 1),
        Err(MemorySessionError::DeviceMemoryByteCapacity { .. })
    ));
    assert_eq!(bytes.backend.alloc_calls, 1);
    bytes.release_device_memory(lease).unwrap();
    assert_eq!(bytes.retained_device_memory_bytes, 0);
    assert!(bytes.allocate_device_memory(device, vm, 1, 1).is_ok());

    let mut records = acquired();
    let mut leases = Vec::new();
    for _ in 0..MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 {
        leases.push(records.allocate_device_memory(device, vm, 1, 1).unwrap());
    }
    assert!(matches!(
        records.allocate_device_memory(device, vm, 1, 1),
        Err(MemorySessionError::DeviceMemoryAllocationCapacity { .. })
    ));
    assert_eq!(
        records.backend.alloc_calls,
        MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1
    );
    assert_eq!(leases.len(), MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1);
}

#[test]
fn released_device_memory_does_not_consume_record_capacity() {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    let anchor = engine.allocate_device_memory(device, vm, 1, 1).unwrap();
    let cycles = MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 * 4;
    let mut stale = None;
    let mut stale_slot = None;

    for cycle in 0..cycles {
        let lease = engine.allocate_device_memory(device, vm, 1, 1).unwrap();
        if cycle == 0 {
            stale = Some(Gfx942DeviceMemoryLeaseV1 {
                id: lease.id,
                generation: lease.generation,
                device: lease.device,
                vm: lease.vm,
                layout: lease.layout,
                marker: PhantomData::<Gfx942DeviceMemoryUnmappedV1>,
            });
            stale_slot = Some(engine.device_memory_record_slots[&lease.id]);
        }
        engine.release_device_memory(lease).unwrap();
        assert!(engine.device_memory.len() <= MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1);
    }

    let stale = stale.unwrap();
    assert!(
        !engine
            .device_memory
            .iter()
            .any(|record| record.id == stale.id)
    );
    assert_eq!(
        engine
            .device_memory
            .iter()
            .filter(|record| record.phase != DeviceMemoryPhaseV1::Released)
            .count(),
        1
    );
    assert_eq!(
        engine.next_device_memory_id,
        u64::try_from(cycles).unwrap() + 2
    );
    assert_eq!(engine.retained_device_memory_bytes, 4096);
    assert_eq!(engine.backend.free_calls, cycles);
    assert_eq!(engine.backend.release_va_calls, cycles);

    engine.backend.next_handle = 2;
    let replacement = engine.allocate_device_memory(device, vm, 1, 1).unwrap();
    assert_eq!(
        engine.device_memory_record_slots[&replacement.id],
        stale_slot.unwrap()
    );
    engine.device_lookup_comparisons.set(0);
    let replacement_index = engine
        .device_memory_index(&replacement, DeviceMemoryPhaseV1::Unmapped)
        .unwrap();
    assert_eq!(engine.device_lookup_comparisons.get(), 1);
    assert_eq!(engine.device_memory[replacement_index].handle, Some(2));
    assert!(matches!(
        engine.release_device_memory(stale),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(engine.backend.free_calls, cycles);
    engine.release_device_memory(replacement).unwrap();
    let anchor = engine.map_device_memory(anchor).unwrap();
    let anchor = engine.unmap_device_memory(anchor).unwrap();
    engine.release_device_memory(anchor).unwrap();
    assert_eq!(engine.retained_device_memory_bytes, 0);
    assert_eq!(engine.backend.free_calls, cycles + 2);
    assert_eq!(engine.backend.release_va_calls, cycles + 2);
    assert!(engine.device_memory.len() <= MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1);
}

#[test]
fn device_slot_reuse_checks_all_later_live_records_for_native_collisions() {
    let setup = || {
        let mut engine = acquired();
        let (device, vm) = device_vm(1);
        let mut leases = Vec::new();
        for _ in 0..MAX_GFX942_DEVICE_MEMORY_ALLOCATION_RECORDS_V1 {
            leases.push(engine.allocate_device_memory(device, vm, 1, 1).unwrap());
        }
        let released = leases.remove(0);
        engine.release_device_memory(released).unwrap();
        (engine, device, vm, leases)
    };

    let (mut va, device, vm, leases) = setup();
    va.backend.fixed_va =
        Some(va.device_memory[va.device_memory_record_slots[&leases[0].id]].gpu_va);
    assert!(matches!(
        va.allocate_device_memory(device, vm, 1, 1),
        Err(MemorySessionError::KernelResultMalformed(
            "overlapping device-memory GPU VA reservation"
        ))
    ));
    assert_eq!(va.phase(), SharedMemorySessionPhaseV1::Quarantined);

    let (mut handle, device, vm, leases) = setup();
    handle.backend.next_handle = handle.device_memory
        [handle.device_memory_record_slots[&leases[0].id]]
        .handle
        .unwrap();
    assert!(matches!(
        handle.allocate_device_memory(device, vm, 1, 1),
        Err(MemorySessionError::KernelResultMalformed(
            "device-memory handle or mmap-offset collision"
        ))
    ));
    assert_eq!(handle.phase(), SharedMemorySessionPhaseV1::Quarantined);
}

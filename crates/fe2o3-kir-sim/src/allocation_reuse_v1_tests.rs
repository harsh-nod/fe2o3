//! Allocator component tests. These do not substitute for ordinary-source,
//! sealed-capture, transport, or browser qualification.
use super::*;

fn limits() -> SimulationLimitsV1 {
    SimulationLimitsV1 {
        max_allocations: 32,
        max_allocation_bytes: 4_096,
        max_total_bytes: 8_192,
        ..SimulationLimitsV1::default()
    }
}

fn memory(cap: Option<usize>) -> Memory {
    Memory::new(
        0,
        0,
        limits(),
        cap.map(|bytes| SimulationAllocationReuseV1::exact_private_and_workgroup(bytes).unwrap()),
    )
    .unwrap()
}

fn invocation() -> SimulationInvocationV1 {
    SimulationInvocationV1 {
        global: [7, 0, 0],
        workgroup: [1, 0, 0],
        local: [3, 0, 0],
        workgroup_size: [4, 1, 1],
        workgroup_count: [2, 1, 1],
        launch_extent: [8, 1, 1],
    }
}

fn creation(address_space: AddressSpace) -> AllocationCreationV1 {
    let invocation = invocation();
    let scope = if address_space == AddressSpace::Workgroup {
        SimulationAllocationScopeV1::Workgroup {
            coordinate: invocation.workgroup,
            size: invocation.workgroup_size,
            count: invocation.workgroup_count,
            launch: invocation.launch_extent,
        }
    } else {
        SimulationAllocationScopeV1::Invocation(invocation)
    };
    AllocationCreationV1 {
        scope,
        site: Some(SimulationEventSiteV1 {
            function_ordinal: 2,
            block: BlockId(3),
            operation: Some(4),
        }),
    }
}

fn allocate(
    memory: &mut Memory,
    shape: (AddressSpace, AccessMode, u32),
    bytes: &[u8],
    initialized: &[bool],
) -> AllocationCommit {
    memory
        .allocate(
            shape,
            (
                try_clone_slice(bytes).unwrap(),
                try_clone_slice(initialized).unwrap(),
            ),
            creation(shape.0),
            limits(),
        )
        .unwrap()
}

fn private(memory: &mut Memory, len: usize) -> AllocationCommit {
    allocate(
        memory,
        (AddressSpace::Private, AccessMode::ReadWrite, 4),
        &vec![0; len],
        &vec![false; len],
    )
}

fn identity(commit: &AllocationCommit) -> SimulationAllocationStorageIdentityV1 {
    commit.transition.unwrap().descriptor().identity()
}

fn pointer(allocation: u64) -> PointerValue {
    PointerValue {
        allocation,
        byte_offset: 0,
        element: ScalarType::U32,
        address_space: AddressSpace::Private,
        access: AccessMode::ReadWrite,
        lower_bound: 0,
        upper_bound: 4,
        abi_argument_ordinal: NO_ABI_ARGUMENT_V1,
    }
}

#[test]
fn policy_is_explicit_and_hard_bounded() {
    assert_eq!(
        SimulationAllocationReuseV1::exact_private_and_workgroup(0)
            .unwrap()
            .max_cached_payload_bytes(),
        0
    );
    assert!(
        SimulationAllocationReuseV1::exact_private_and_workgroup(
            MAX_ALLOCATION_REUSE_CACHED_PAYLOAD_BYTES_V1,
        )
        .is_ok()
    );
    assert!(
        SimulationAllocationReuseV1::exact_private_and_workgroup(
            MAX_ALLOCATION_REUSE_CACHED_PAYLOAD_BYTES_V1 + 1,
        )
        .is_err()
    );
}

#[test]
fn legacy_storage_retains_no_pool_or_new_descriptor() {
    let mut memory = memory(None);
    let first = private(&mut memory, 4);
    assert_eq!(first.id, 1);
    assert_eq!(first.transition, None);
    assert!(
        memory.allocations[&first.id]
            .observation_descriptor
            .is_empty()
    );
    assert_eq!(
        memory.allocations[&first.id]
            .observation_descriptor
            .capacity(),
        0,
    );
    assert!(memory.release_one(first.id).unwrap().removed);
    let second = private(&mut memory, 4);
    assert_eq!(second.id, 2);
    assert_eq!(second.transition, None);
    assert!(memory.reuse.is_none());
}

#[test]
fn released_private_storage_reuses_actual_vectors_but_never_semantic_pointer() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    let original = &memory.allocations[&first.id];
    let bytes_pointer = original.bytes.as_ptr();
    let mask_pointer = original.initialized.as_ptr();
    let released = memory.release_one(first.id).unwrap();
    assert!(released.removed);
    assert_eq!(released.transition.unwrap().sequence(), 2);
    assert_eq!(
        released.transition.unwrap().kind(),
        SimulationAllocationTransitionKindV1::Release
    );
    let second = allocate(
        &mut memory,
        (AddressSpace::Private, AccessMode::ReadWrite, 4),
        &[1, 2, 3, 4],
        &[true, false, true, false],
    );
    assert_ne!(first.id, second.id);
    assert_eq!(
        identity(&first).storage_slot(),
        identity(&second).storage_slot()
    );
    assert_eq!(identity(&first).generation(), 1);
    assert_eq!(identity(&second).generation(), 2);
    assert_eq!(
        second.transition.unwrap().kind(),
        SimulationAllocationTransitionKindV1::Create {
            previous_allocation: Some(first.id)
        }
    );
    let reused = &memory.allocations[&second.id];
    assert_eq!(reused.bytes.as_ptr(), bytes_pointer);
    assert_eq!(reused.initialized.as_ptr(), mask_pointer);
    assert_eq!(reused.bytes, [1, 2, 3, 4]);
    assert_eq!(reused.initialized, [true, false, true, false]);
    assert!(matches!(memory.allocation(&pointer(first.id)),
        Err(SimulationExecutionErrorKindV1::DanglingPointer { allocation }) if allocation == first.id));
    assert!(memory.allocation(&pointer(second.id)).is_ok());
    assert_eq!(memory.reuse.as_ref().unwrap().cached_payload_bytes, 0);
}

#[test]
fn workgroup_hit_moves_all_four_vectors_and_resets_publication_ownership() {
    let mut memory = memory(Some(128));
    let shape = (AddressSpace::Workgroup, AccessMode::ReadWrite, 4);
    let first = allocate(&mut memory, shape, &[9; 4], &[true; 4]);
    let allocation = memory.allocations.get_mut(&first.id).unwrap();
    allocation.workgroup_published.fill(true);
    allocation.workgroup_writer.fill(23);
    let pointers = (
        allocation.bytes.as_ptr(),
        allocation.initialized.as_ptr(),
        allocation.workgroup_published.as_ptr(),
        allocation.workgroup_writer.as_ptr(),
    );
    memory.release_one(first.id).unwrap();
    let second = allocate(&mut memory, shape, &[0; 4], &[false; 4]);
    let allocation = &memory.allocations[&second.id];
    assert_eq!(
        pointers,
        (
            allocation.bytes.as_ptr(),
            allocation.initialized.as_ptr(),
            allocation.workgroup_published.as_ptr(),
            allocation.workgroup_writer.as_ptr(),
        )
    );
    assert_eq!(allocation.bytes, [0; 4]);
    assert_eq!(allocation.initialized, [false; 4]);
    assert_eq!(allocation.workgroup_published, [false; 4]);
    assert_eq!(allocation.workgroup_writer, [0; 4]);
    assert_eq!(identity(&second).generation(), 2);
    assert_eq!(
        second.transition.unwrap().descriptor().scope(),
        creation(shape.0).scope
    );
}

#[test]
fn active_allocations_never_share_a_storage_slot() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    let second = private(&mut memory, 4);
    assert_ne!(
        identity(&first).storage_slot(),
        identity(&second).storage_slot()
    );
    assert_eq!(identity(&second).generation(), 1);
    assert_eq!(
        second.transition.unwrap().kind(),
        SimulationAllocationTransitionKindV1::Create {
            previous_allocation: None
        }
    );
}

#[test]
fn every_exact_shape_mismatch_misses_without_relabeling_cached_storage() {
    for (shape, len) in [
        ((AddressSpace::Private, AccessMode::ReadWrite, 8), 4),
        ((AddressSpace::Private, AccessMode::ReadOnly, 4), 4),
        ((AddressSpace::Private, AccessMode::ReadWrite, 4), 8),
        ((AddressSpace::Workgroup, AccessMode::ReadWrite, 4), 4),
    ] {
        let mut memory = memory(Some(256));
        let first = private(&mut memory, 4);
        memory.release_one(first.id).unwrap();
        let second = allocate(&mut memory, shape, &vec![0; len], &vec![false; len]);
        assert_ne!(
            identity(&first).storage_slot(),
            identity(&second).storage_slot()
        );
        assert_eq!(identity(&second).generation(), 1);
        assert_eq!(
            memory
                .reuse
                .as_ref()
                .unwrap()
                .private
                .as_ref()
                .unwrap()
                .observation_descriptor
                .first()
                .unwrap()
                .identity(),
            identity(&first)
        );
    }
}

#[test]
fn both_cells_charge_actual_capacity_and_exact_combined_cap() {
    let mut exact = memory(Some(52));
    let private = private(&mut exact, 4);
    let workgroup = allocate(
        &mut exact,
        (AddressSpace::Workgroup, AccessMode::ReadWrite, 4),
        &[0; 4],
        &[false; 4],
    );
    assert_eq!(
        exact.allocations[&private.id].retained_payload_capacity_bytes(),
        Some(8)
    );
    assert_eq!(
        exact.allocations[&workgroup.id].retained_payload_capacity_bytes(),
        Some(44)
    );
    exact.release_one(private.id).unwrap();
    exact.release_one(workgroup.id).unwrap();
    let pool = exact.reuse.as_ref().unwrap();
    assert_eq!(pool.cached_payload_bytes, 52);
    assert!(pool.private.is_some() && pool.workgroup.is_some());

    let mut short = memory(Some(51));
    let first = self::private(&mut short, 4);
    let second = allocate(
        &mut short,
        (AddressSpace::Workgroup, AccessMode::ReadWrite, 4),
        &[0; 4],
        &[false; 4],
    );
    short.release_one(first.id).unwrap();
    short.release_one(second.id).unwrap();
    let pool = short.reuse.as_ref().unwrap();
    assert_eq!(pool.cached_payload_bytes, 8);
    assert!(pool.private.is_some() && pool.workgroup.is_none());
}

#[test]
fn replacing_a_cell_retires_the_old_slot_and_charges_new_capacity() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    let second = private(&mut memory, 8);
    memory.release_one(first.id).unwrap();
    memory.release_one(second.id).unwrap();
    assert_eq!(memory.reuse.as_ref().unwrap().cached_payload_bytes, 16);
    let third = private(&mut memory, 4);
    assert_ne!(
        identity(&third).storage_slot(),
        identity(&first).storage_slot()
    );
    assert_ne!(
        identity(&third).storage_slot(),
        identity(&second).storage_slot()
    );
}

#[test]
fn zero_length_allocations_have_real_distinct_incarnations_without_payload() {
    let mut memory = memory(Some(0));
    let first = private(&mut memory, 0);
    memory.release_one(first.id).unwrap();
    let second = private(&mut memory, 0);
    assert_ne!(first.id, second.id);
    assert_eq!(
        identity(&first).storage_slot(),
        identity(&second).storage_slot()
    );
    assert_eq!(identity(&second).generation(), 2);
    assert_eq!(memory.reuse.as_ref().unwrap().cached_payload_bytes, 0);
}

#[test]
fn cumulative_creation_limit_does_not_become_live_count() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    memory.release_one(first.id).unwrap();
    let mut bounded = limits();
    bounded.max_allocations = 1;
    let result = memory.allocate(
        (AddressSpace::Private, AccessMode::ReadWrite, 4),
        (vec![0; 4], vec![false; 4]),
        creation(AddressSpace::Private),
        bounded,
    );
    assert!(matches!(
        result,
        Err(SimulationExecutionErrorKindV1::AllocationLimit { limit: 1 })
    ));
    assert_eq!(memory.allocations_created, 1);
    assert_eq!(memory.live_bytes, 0);
    assert_eq!(memory.reuse.as_ref().unwrap().sequence, 2);
    assert!(memory.reuse.as_ref().unwrap().private.is_some());
}

#[test]
fn semantic_and_slot_overflow_refuse_before_cached_storage_mutates() {
    for overflow_semantic in [false, true] {
        let mut memory = memory(Some(128));
        let first = private(&mut memory, 4);
        memory.release_one(first.id).unwrap();
        if overflow_semantic {
            memory.next_allocation = u64::MAX;
        } else {
            memory.reuse.as_mut().unwrap().next_storage_slot = u64::MAX;
        }
        // Different length requires a fresh slot rather than a reuse hit.
        let result = memory.allocate(
            (AddressSpace::Private, AccessMode::ReadWrite, 4),
            (vec![0; 8], vec![false; 8]),
            creation(AddressSpace::Private),
            limits(),
        );
        assert!(result.is_err());
        assert_eq!(memory.allocations_created, 1);
        assert_eq!(memory.live_bytes, 0);
        let pool = memory.reuse.as_ref().unwrap();
        assert_eq!(pool.sequence, 2);
        assert_eq!(pool.cached_payload_bytes, 8);
        assert_eq!(
            pool.private
                .as_ref()
                .unwrap()
                .observation_descriptor
                .first()
                .unwrap()
                .identity(),
            identity(&first)
        );
    }
}

#[test]
fn exhausted_generation_retires_old_buffer_instead_of_wrapping() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    memory.release_one(first.id).unwrap();
    let cached = memory.reuse.as_mut().unwrap().private.as_mut().unwrap();
    let old = cached.observation_descriptor[0];
    cached.observation_descriptor[0] = SimulationAllocationDescriptorV1::new(
        SimulationAllocationStorageIdentityV1::new(
            first.id,
            identity(&first).storage_slot(),
            u64::MAX,
        ),
        (
            old.address_space(),
            old.access(),
            old.alignment(),
            old.byte_len(),
        ),
        old.scope(),
        old.creation_site(),
    );
    let second = private(&mut memory, 4);
    assert_ne!(
        identity(&first).storage_slot(),
        identity(&second).storage_slot()
    );
    assert_eq!(identity(&second).generation(), 1);
    assert!(memory.reuse.as_ref().unwrap().private.is_none());
    assert_eq!(memory.reuse.as_ref().unwrap().cached_payload_bytes, 0);
}

#[test]
fn malformed_input_and_release_accounting_errors_are_transactional() {
    let mut memory = memory(Some(128));
    let result = memory.allocate(
        (AddressSpace::Private, AccessMode::ReadWrite, 4),
        (vec![0; 4], vec![false; 3]),
        creation(AddressSpace::Private),
        limits(),
    );
    assert!(result.is_err());
    assert_eq!(memory.next_allocation, 1);
    assert_eq!(memory.reuse.as_ref().unwrap().sequence, 0);
    let first = private(&mut memory, 4);
    memory.live_bytes = 0;
    assert!(memory.release_one(first.id).is_err());
    assert!(memory.allocations.contains_key(&first.id));
    assert_eq!(memory.reuse.as_ref().unwrap().sequence, 1);
}

#[test]
fn sequence_exhaustion_marks_unavailable_without_changing_execution() {
    let mut memory = memory(Some(128));
    memory.reuse.as_mut().unwrap().sequence = u64::MAX;
    let first = private(&mut memory, 4);
    assert!(first.transition.is_none());
    assert!(memory.allocations.contains_key(&first.id));
    assert_eq!(
        memory.reuse.as_ref().unwrap().watermark(),
        SimulationAllocationWatermarkV1::Unavailable {
            reason: SimulationAllocationObservationUnavailableV1::SequenceOverflow,
        }
    );
    let released = memory.release_one(first.id).unwrap();
    assert!(released.removed);
    assert!(released.transition.is_none());
}

#[test]
fn duplicate_release_adds_no_transition_or_cache_credit() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    let release = memory.release_one(first.id).unwrap();
    assert!(release.removed);
    let duplicate = memory.release_one(first.id).unwrap();
    assert!(!duplicate.removed);
    assert!(duplicate.transition.is_none());
    assert_eq!(memory.reuse.as_ref().unwrap().sequence, 2);
    assert_eq!(memory.reuse.as_ref().unwrap().cached_payload_bytes, 8);
}

#[test]
fn preexisting_dispatch_storage_is_identified_but_never_cached() {
    let mut memory = memory(Some(128));
    let commit = memory
        .allocate(
            (AddressSpace::Global, AccessMode::ReadWrite, 4),
            (vec![1; 4], vec![true; 4]),
            AllocationCreationV1::dispatch(),
            limits(),
        )
        .unwrap();
    let birth = commit.transition.unwrap();
    assert_eq!(
        birth.kind(),
        SimulationAllocationTransitionKindV1::Preexisting
    );
    assert_eq!(
        birth.descriptor().scope(),
        SimulationAllocationScopeV1::Dispatch
    );
    assert_eq!(birth.descriptor().creation_site(), None);
    let release = memory.release_one(commit.id).unwrap().transition.unwrap();
    assert_eq!(release.descriptor(), birth.descriptor());
    assert_eq!(memory.reuse.as_ref().unwrap().cached_payload_bytes, 0);
    assert!(memory.reuse.as_ref().unwrap().private.is_none());
    assert!(memory.reuse.as_ref().unwrap().workgroup.is_none());
}

#[test]
fn release_retains_creation_facts_and_sequence_is_strictly_contiguous() {
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    let birth = first.transition.unwrap();
    let released = memory.release_one(first.id).unwrap().transition.unwrap();
    let second = private(&mut memory, 4);
    assert_eq!(
        [
            birth.sequence(),
            released.sequence(),
            second.transition.unwrap().sequence()
        ],
        [1, 2, 3]
    );
    assert_eq!(birth.descriptor(), released.descriptor());
    assert_eq!(
        birth.descriptor().creation_site(),
        creation(AddressSpace::Private).site
    );
    assert_eq!(
        memory.reuse.as_ref().unwrap().watermark(),
        SimulationAllocationWatermarkV1::Available {
            through_sequence: 3
        }
    );
}

#[test]
fn observed_descriptor_storage_moves_with_reused_backing_and_stays_out_of_line() {
    // The default allocation-map bucket must not embed the full invocation DTO.
    assert!(size_of::<Allocation>() <= 5 * size_of::<Vec<u8>>() + 2 * size_of::<u64>());
    let mut memory = memory(Some(128));
    let first = private(&mut memory, 4);
    let descriptor = &memory.allocations[&first.id].observation_descriptor;
    assert_eq!(descriptor.len(), 1);
    let pointer = descriptor.as_ptr();
    let capacity = descriptor.capacity();
    assert_eq!(descriptor[0].identity(), identity(&first));
    memory.release_one(first.id).unwrap();
    let cached = &memory
        .reuse
        .as_ref()
        .unwrap()
        .private
        .as_ref()
        .unwrap()
        .observation_descriptor;
    assert_eq!(cached.as_ptr(), pointer);
    assert_eq!(cached.capacity(), capacity);
    let second = memory
        .allocate_with_descriptor_reservation(
            (AddressSpace::Private, AccessMode::ReadWrite, 4),
            (vec![0; 4], vec![false; 4]),
            creation(AddressSpace::Private),
            limits(),
            |_| panic!("cache hit must not reserve descriptor backing"),
        )
        .unwrap();
    let current = &memory.allocations[&second.id].observation_descriptor;
    assert_eq!(current.len(), 1);
    assert_eq!(current.as_ptr(), pointer);
    assert_eq!(current.capacity(), capacity);
    assert_eq!(current[0].identity(), identity(&second));
    assert_ne!(first.id, second.id);
    assert_eq!(
        identity(&second).storage_slot(),
        identity(&first).storage_slot()
    );
    assert_eq!(identity(&second).generation(), 2);
}

#[test]
fn descriptor_reservation_failure_precedes_retirement_and_identity_commit() {
    for exhausted in [false, true] {
        let mut memory = memory(Some(128));
        let first = private(&mut memory, 4);
        memory.release_one(first.id).unwrap();
        if exhausted {
            let cached = memory.reuse.as_mut().unwrap().private.as_mut().unwrap();
            let old = cached.observation_descriptor[0];
            cached.observation_descriptor[0] = SimulationAllocationDescriptorV1::new(
                SimulationAllocationStorageIdentityV1::new(
                    first.id,
                    old.identity().storage_slot(),
                    u64::MAX,
                ),
                (
                    old.address_space(),
                    old.access(),
                    old.alignment(),
                    old.byte_len(),
                ),
                old.scope(),
                old.creation_site(),
            );
        }
        let (descriptor, descriptor_pointer, payload_pointer, sequence, slot, cache_bytes) = {
            let pool = memory.reuse.as_ref().unwrap();
            let cached = pool.private.as_ref().unwrap();
            (
                cached.observation_descriptor[0],
                cached.observation_descriptor.as_ptr(),
                cached.bytes.as_ptr(),
                pool.sequence,
                pool.next_storage_slot,
                pool.cached_payload_bytes,
            )
        };
        let next_allocation = memory.next_allocation;
        let len = if exhausted { 4 } else { 8 };
        let result = memory.allocate_with_descriptor_reservation(
            (AddressSpace::Private, AccessMode::ReadWrite, 4),
            (vec![0; len], vec![false; len]),
            creation(AddressSpace::Private),
            limits(),
            |_| Err(SimulationExecutionErrorKindV1::AllocationFailure),
        );
        assert!(matches!(
            result,
            Err(SimulationExecutionErrorKindV1::AllocationFailure)
        ));
        assert_eq!(memory.next_allocation, next_allocation);
        assert_eq!(memory.allocations_created, 1);
        assert_eq!(memory.live_bytes, 0);
        assert!(memory.allocations.is_empty());
        let pool = memory.reuse.as_ref().unwrap();
        assert_eq!(pool.sequence, sequence);
        assert_eq!(pool.next_storage_slot, slot);
        assert_eq!(pool.cached_payload_bytes, cache_bytes);
        let cached = pool.private.as_ref().unwrap();
        assert_eq!(cached.observation_descriptor.len(), 1);
        assert_eq!(cached.observation_descriptor[0], descriptor);
        assert_eq!(cached.observation_descriptor.as_ptr(), descriptor_pointer);
        assert_eq!(cached.bytes.as_ptr(), payload_pointer);
    }
}

use super::*;

#[test]
fn device_buffer_allocation_extents_preserve_logical_bounds() {
    for (logical, physical) in [
        (1, 4096),
        (4095, 4096),
        (4096, 4096),
        (4097, 8192),
        (1_048_832, 1_052_672),
        (256 << 20, 256 << 20),
    ] {
        assert_eq!(
            device_buffer_allocation_extents_v1(logical, 4096).unwrap(),
            (logical, physical)
        );
        assert!(
            crate::persistent_directional_sdma::directional_persistent_sdma_extents_are_admitted_v1(
                logical, physical, 1
            )
        );
    }
    let (logical, physical) = device_buffer_allocation_extents_v1((256 << 20) + 1, 4096).unwrap();
    assert!(
        !crate::persistent_directional_sdma::directional_persistent_sdma_extents_are_admitted_v1(
            logical, physical, 1
        )
    );
    for bytes in [
        0,
        crate::shared_memory::MAX_GFX942_DEVICE_MEMORY_BYTES_V1 + 1,
        u64::MAX,
    ] {
        assert!(device_buffer_allocation_extents_v1(bytes, 4096).is_err());
    }
    for alignment in [0, 3, 8192, u64::MAX] {
        assert!(device_buffer_allocation_extents_v1(4097, alignment).is_err());
    }
}

#[test]
fn device_buffer_allocation_wires_rounded_backing_and_original_logical_extent() {
    let body = include_str!("../../queue_live/sdma_allocation.rs");
    assert!(body.contains("device_buffer_allocation_extents_v1("));
    assert!(body.contains("device_buffer_allocation_extents_v1(*logical_bytes, *alignment)?"));
    assert!(body.contains("prepare_device(allocation, physical_bytes, *alignment)"));
    assert!(body.contains("logical_bytes,"));
    assert!(!body.contains("logical_bytes: physical_bytes"));
}

#[test]
fn shared_allocation_record_shape_covers_ring_control_and_completions() {
    assert_eq!(
        GFX942_SDMA_SHARED_ALLOCATION_RECORDS_PER_QUEUE_V1,
        1 + 1 + 1
    );
}

#[test]
fn compute_coexistence_scans_both_directions_and_settled_retained_records() {
    for engine in [
        GFX942_SDMA_D2H_ENGINE_INDEX_V1,
        GFX942_SDMA_H2D_ENGINE_INDEX_V1,
    ] {
        let mut owner = compute_coexistence_owner_for_test(engine);
        compute_coexistence_add_single(&mut owner, 41);
        compute_coexistence_add_window(&mut owner, 42);
        owner.records[7].as_mut().unwrap().completion_observed = true;
        let endpoints = compute_coexistence_collect_for_test(&owner).unwrap();
        assert_eq!(endpoints.len(), 2);
        assert!(endpoints.iter().any(|entry| entry.allocation_id == 41));
        assert!(endpoints.iter().any(|entry| entry.allocation_id == 42));
        let domain = fe2o3_runtime_model::R66DeviceDomainV1 {
            physical_device: 7,
            device_generation: 1,
            vm_id: 1,
        };
        let mut compute = endpoints[0];
        compute.allocation_id = 43;
        assert!(fe2o3_runtime_model::r66_device_storage_rosters_disjoint_v1(
            domain,
            &[compute],
            &endpoints
        ));
        compute.allocation_id = 41;
        compute.generation += 1;
        assert!(
            !fe2o3_runtime_model::r66_device_storage_rosters_disjoint_v1(
                domain,
                &[compute],
                &endpoints
            )
        );
        assert!(owner.records[7].is_some());
        assert!(owner.persistent_window_records[63].is_some());
    }
}

#[test]
fn compute_coexistence_rejects_incomplete_stale_and_ordinary_ledgers() {
    for mutation in 0..13 {
        let mut owner = compute_coexistence_owner_for_test(GFX942_SDMA_H2D_ENGINE_INDEX_V1);
        compute_coexistence_add_single(&mut owner, 41);
        compute_coexistence_add_window(&mut owner, 42);
        match mutation {
            0 => {
                owner.records.pop();
            }
            1 => {
                owner.persistent_window_slots[0] = None;
            }
            2 => {
                owner.persistent_window_records[63] = None;
            }
            3 => {
                owner.persistent_window_slots[0]
                    .as_mut()
                    .unwrap()
                    .generation += 1;
            }
            4 => {
                owner.persistent_window_slots[0]
                    .as_mut()
                    .unwrap()
                    .anchor_slot = 64;
            }
            5 => {
                owner.persistent_window_slots[0]
                    .as_mut()
                    .unwrap()
                    .completion_value = 0;
            }
            6 => {
                owner.persistent_window_records[63]
                    .as_mut()
                    .unwrap()
                    .packet_count = 1;
            }
            7 => {
                owner.records[7].as_mut().unwrap().directional_persistent = false;
            }
            8 => {
                owner.records[7].as_mut().unwrap().generation += 1;
            }
            9 => {
                owner.records[7]
                    .as_mut()
                    .unwrap()
                    .destination
                    .owner
                    .generation
                    .0 += 1;
            }
            10 => {
                owner.records[7].as_mut().unwrap().source.pool_generation = 0;
            }
            11 => {
                owner.records[7].as_mut().unwrap().destination_offset = u64::MAX;
            }
            12 => {
                owner.persistent_window_slots[7] = owner.persistent_window_slots[0];
            }
            _ => unreachable!(),
        }
        assert!(
            compute_coexistence_collect_for_test(&owner).is_none(),
            "mutation {mutation}"
        );
        assert!(
            owner.records[7].is_some(),
            "rejection must retain exact custody"
        );
    }
}

#[test]
fn compute_coexistence_missing_native_authority_never_admits() {
    let owner = queue_key(7, 11, 13);
    let queues = Gfx942SdmaQueueSetV1::Directional(vec![
        compute_coexistence_owner_for_test(GFX942_SDMA_D2H_ENGINE_INDEX_V1),
        compute_coexistence_owner_for_test(GFX942_SDMA_H2D_ENGINE_INDEX_V1),
    ]);
    assert!(queues.compute_coexistence_endpoints_v1(owner).is_none());
    assert!(
        Gfx942SdmaQueueSetV1::Generic(Vec::new())
            .compute_coexistence_endpoints_v1(owner)
            .is_none()
    );
    assert!(
        Gfx942SdmaQueueSetV1::Directional(Vec::new())
            .compute_coexistence_endpoints_v1(owner)
            .is_none()
    );
}

#[test]
fn full_host_content_certificate_is_bound_preserved_and_invalidated() {
    let owner = queue_key(7, 11, 13);
    let digest = [0x5a; 32];
    let (device, mut host) = persistent_sdma_buffers_for_test(owner, 100);
    assert_eq!(host.certified_full_host_content_sha256(4096), None);
    host.certify_full_host_content(digest);
    assert_eq!(host.certified_full_host_content_sha256(4096), Some(digest));
    assert_eq!(host.certified_full_host_content_sha256(4095), None);

    let request = Gfx942SdmaCopyRequestV1::new(host, 0, device, 0, 4096);
    let (mut host, _device) = request.into_buffers();
    assert_eq!(
        host.certified_full_host_content_sha256(4096),
        Some(digest),
        "H2D source construction must preserve exact host evidence"
    );
    host.set_logical_bytes(2048);
    assert_eq!(host.certified_full_host_content_sha256(2048), None);

    let (device, mut host) = persistent_sdma_buffers_for_test(owner, 200);
    host.certify_full_host_content(digest);
    let request = Gfx942SdmaCopyRequestV1::new(device, 0, host, 0, 4096);
    let (_device, host) = request.into_buffers();
    assert_eq!(
        host.certified_full_host_content_sha256(4096),
        None,
        "D2H destination construction must invalidate before publication"
    );

    let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 300);
    host.certify_full_host_content(digest);
    host.advance_pool_generation().unwrap();
    assert_eq!(host.certified_full_host_content_sha256(4096), None);

    let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 400);
    host.certify_full_host_content(digest);
    let (storage, owner, generation, logical_bytes) = host.into_bridge_parts();
    let host = Gfx942SdmaBufferV1::from_bridge_parts(storage, owner, generation, logical_bytes);
    assert_eq!(host.certified_full_host_content_sha256(4096), None);
}

#[test]
fn sdma_recycle_generation_overflow_preserves_original_certificate() {
    let (_device, mut host) = persistent_sdma_buffers_for_test(queue_key(7, 11, 13), 500);
    host.pool_generation = u64::MAX;
    host.certify_full_host_content([0x5a; 32]);
    let metadata = host.cleanup_metadata();
    let address = core::ptr::from_ref(host.host_content_certificate.as_deref().unwrap());
    assert!(host.advance_pool_generation().is_err());
    assert_eq!(host.cleanup_metadata(), metadata);
    assert_eq!(
        core::ptr::from_ref(host.host_content_certificate.as_deref().unwrap()),
        address
    );
    assert_eq!(
        host.certified_full_host_content_sha256(4096),
        Some([0x5a; 32])
    );
}

#[test]
fn sdma_cleanup_preserves_original_certificate_box_through_native_disposal() {
    use crate::shared_memory::{
        DataCleanupCustodyV1, DispatchDataReleaseV1, PristineAbortMemoryFixtureV1,
    };
    for configured in [false, true] {
        for fault in [None, Some(false), Some(true)] {
            let mut memory = PristineAbortMemoryFixtureV1::new_configured(configured);
            let token = memory.host().into_token();
            let mut buffer = Gfx942SdmaBufferV1::from_bridge_parts(
                Gfx942SdmaBufferStorageV1::Host(token),
                queue_key(7, 11, 13),
                3,
                17,
            );
            buffer.certify_full_host_content(Sha256::digest([0x5a; 17]).into());
            let certificate =
                core::ptr::from_ref(buffer.host_content_certificate.as_deref().unwrap());
            let metadata = buffer.cleanup_metadata();
            let identity = buffer.storage_identity();
            let before = memory.memory_snapshot();
            let mut root = DataCleanupCustodyV1::from_sdma(buffer);
            if let Some(panic) = fault {
                memory.fail_data(1, "free", panic);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                memory.release_data(&mut root)
            }));
            match fault {
                None => result.unwrap().unwrap(),
                Some(false) => assert!(result.unwrap().is_err()),
                Some(true) => assert_eq!(
                    result.unwrap_err().downcast_ref::<(&str, &str)>(),
                    Some(&("N2 native panic", "free"))
                ),
            }
            assert_eq!(root.sdma_metadata(), Some(&metadata));
            assert_eq!(
                core::ptr::from_ref(
                    root.sdma_metadata()
                        .unwrap()
                        .host_content_certificate
                        .as_deref()
                        .unwrap()
                ),
                certificate
            );
            let observed = root.observation();
            before.assert_data_prefix(
                &memory,
                &[],
                &[identity],
                usize::from(fault.is_none()),
                fault.map(|_| &observed),
            );
            let stable = memory.memory_snapshot();
            assert!(memory.release_data(&mut root).is_err());
            assert_eq!(memory.memory_snapshot(), stable);
            assert_eq!(root.observation(), observed);
        }
    }
}

#[test]
fn host_content_certificate_rejects_owner_and_storage_substitution() {
    let owner = queue_key(7, 21, 23);
    let digest = [0xa5; 32];
    let (_device, mut first) = persistent_sdma_buffers_for_test(owner, 500);
    let (_device, mut second) = persistent_sdma_buffers_for_test(owner, 600);
    first.certify_full_host_content(digest);
    core::mem::swap(&mut first.storage, &mut second.storage);
    assert_eq!(first.certified_full_host_content_sha256(4096), None);

    second.certify_full_host_content(digest);
    second.owner = queue_key(7, 22, 23);
    assert_eq!(second.certified_full_host_content_sha256(4096), None);
}

#[test]
fn attempted_full_write_clears_certificate_before_lower_failure() {
    let owner = queue_key(7, 31, 33);
    let digest = [0x3c; 32];
    let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 700);
    host.certify_full_host_content(digest);
    let result = host.replace_full_host_content_certificate(|_| {
        Err(Gfx942SdmaErrorV1::Contract(
            "injected opening or closing currentness failure",
        ))
    });
    assert!(result.is_err());
    assert_eq!(host.certified_full_host_content_sha256(4096), None);
}

#[test]
fn padded_full_logical_write_is_valid_but_not_authenticatable() {
    let owner = queue_key(7, 41, 43);
    let (_device, mut host) = persistent_sdma_buffers_for_test(owner, 800);
    assert!(matches!(
        exact_full_host_write_is_authenticatable(&host, 4096),
        Ok(true)
    ));
    host.set_logical_bytes(2048);
    assert!(matches!(
        exact_full_host_write_is_authenticatable(&host, 2048),
        Ok(false)
    ));
    assert!(exact_full_host_write_is_authenticatable(&host, 2049).is_err());
    assert!(exact_full_host_write_is_authenticatable(&host, 0).is_err());
}

#[test]
fn single_copy_prepare_and_publication_are_stack_sized() {
    let source = include_str!("../../sdma.rs");
    let prepare = source
        .split("fn prepare_single_recoverable")
        .nth(1)
        .unwrap()
        .split("fn submit_prepared_single_with_custody")
        .next()
        .unwrap();
    let publish = source
        .split("fn submit_prepared_single_with_custody")
        .nth(1)
        .unwrap()
        .split("fn prepare_persistent_window_recoverable")
        .next()
        .unwrap();
    assert!(!prepare.contains("Vec<"));
    assert!(!prepare.contains("vec!["));
    assert!(!prepare.contains("preallocate_doorbell_failure_message"));
    assert!(!publish.contains("Vec<"));
    assert!(!publish.contains("preallocate_doorbell_failure_message"));
    assert!(publish.contains("PreparedSingleSdmaPublicationFailureV1::Retained"));
}

#[test]
fn persistent_window_packet_limits_and_ring_wrap_are_exact() {
    let maximum = GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1;
    assert!(persistent_sdma_window_packet_count(0).is_err());
    assert_eq!(persistent_sdma_window_packet_count(1).unwrap(), 1);
    assert_eq!(persistent_sdma_window_packet_count(maximum).unwrap(), 1);
    assert_eq!(persistent_sdma_window_packet_count(maximum + 1).unwrap(), 2);
    let sixty_three =
        u32::try_from(u64::from(maximum) * GFX942_SDMA_MAX_IN_FLIGHT_V1 as u64).unwrap();
    assert_eq!(
        persistent_sdma_window_packet_count(sixty_three).unwrap(),
        GFX942_SDMA_MAX_IN_FLIGHT_V1
    );
    assert!(persistent_sdma_window_packet_count(sixty_three + 1).is_err());

    let write = u64::from(GFX942_SDMA_RING_BYTES_V1) - 2 * GFX942_SDMA_SUBMISSION_BYTES_V1 as u64;
    assert_eq!(batch_ring_slot(write, 0).unwrap(), 62);
    assert_eq!(batch_ring_slot(write, 1).unwrap(), 63);
    assert_eq!(batch_ring_slot(write, 2).unwrap(), 0);
}

#[test]
fn persistent_window_publication_is_one_pointer_and_one_doorbell() {
    let source = include_str!("../../sdma.rs");
    let publication = source
        .split("fn submit_prepared_persistent_window_with_custody")
        .nth(1)
        .unwrap()
        .split("fn validate_persistent_window_tickets")
        .next()
        .unwrap();
    assert_eq!(
        publication
            .matches("publish_sdma_control_write_release_in_current_scope")
            .count(),
        1
    );
    assert_eq!(publication.matches("store_packet_id_release").count(), 1);
    let records = publication
        .find("persistent_window_records[anchor_slot]")
        .unwrap();
    let first_mapped_write = publication
        .find("overwrite_mapped_host_visible_subrange_in_current_scope")
        .unwrap();
    assert!(records < first_mapped_write);
    assert!(publication.contains("for copy in &copies"));
    assert!(publication.contains("PreparedPersistentSdmaWindowPublicationFailureV1::Retained"));
}

#[test]
fn persistent_window_has_exclusive_occupancy_and_whole_window_retirement() {
    let source = include_str!("../../sdma.rs");
    let owner = source
        .split("pub(crate) struct Gfx942SdmaQueueOwnerV1")
        .nth(1)
        .unwrap()
        .split("impl Gfx942SdmaQueueOwnerV1")
        .next()
        .unwrap();
    assert!(owner.contains("persistent_window_slots"));
    assert!(owner.contains("persistent_window_records"));

    let batch_start = source
        .split("fn observe_batch_start")
        .nth(1)
        .unwrap()
        .split("fn prepare_xgmi_batch")
        .next()
        .unwrap();
    assert!(batch_start.contains("persistent_window_slots"));
    let destroy = source.split("pub(crate) fn destroy_queue").nth(1).unwrap();
    assert!(destroy.contains("persistent_window_slots"));
    assert!(destroy.contains("persistent_window_records"));

    let generic_validation = source
        .split("fn validate_ticket")
        .nth(1)
        .unwrap()
        .split("fn validate_xgmi_ticket")
        .next()
        .unwrap();
    assert!(generic_validation.contains("self.records"));
    assert!(!generic_validation.contains("persistent_window_slots"));

    let completion = source
        .split("fn complete_persistent_window")
        .nth(1)
        .unwrap()
        .split("fn poll_persistent_window")
        .next()
        .unwrap();
    assert!(completion.contains("for ticket in tickets"));
    assert!(completion.contains("persistent_window_records[anchor_slot]"));
}

#[test]
fn pool_owner_and_generation_coordinates_are_exact() {
    let owner = queue_key(7, 3, 1);
    assert!(exact_queue_owner(owner, owner));
    assert!(!exact_queue_owner(owner, queue_key(8, 3, 1)));
    assert!(!exact_queue_owner(owner, queue_key(7, 4, 1)));
    assert!(!exact_queue_owner(owner, queue_key(7, 3, 2)));
    assert_eq!(next_pool_generation(1).unwrap(), 2);
    assert!(next_pool_generation(u64::MAX).is_err());
}

#[test]
fn persistent_compute_requires_both_directional_sdma_ledgers_quiescent() {
    assert!(directional_sdma_pair_quiescence_is_admitted(true, true));
    assert!(!directional_sdma_pair_quiescence_is_admitted(false, true));
    assert!(!directional_sdma_pair_quiescence_is_admitted(true, false));
    assert!(!directional_sdma_pair_quiescence_is_admitted(false, false));
}

#[test]
fn ticket_rejects_native_queue_id_reuse_across_queue_occurrences() {
    let owner = queue_key(7, 3, 1);
    let ticket = Gfx942SdmaCopyTicketV1 {
        owner,
        queue_id: 11,
        slot: 0,
        generation: 1,
    };
    assert!(ticket_matches_queue_occurrence(ticket, owner, 11));
    assert!(!ticket_matches_queue_occurrence(
        ticket,
        queue_key(7, 3, 2),
        11
    ));
    assert!(!ticket_matches_queue_occurrence(ticket, owner, 12));
}

#[test]
fn xgmi_timeout_failure_retains_the_exact_ticket() {
    let ticket = Gfx942SdmaCopyTicketV1 {
        owner: queue_key(7, 4, 1),
        queue_id: 17,
        slot: 3,
        generation: 9,
    };
    let failure = classify_xgmi_wait_result(Err(Gfx942SdmaErrorV1::Timeout), Ok(()), ticket)
        .err()
        .unwrap();
    assert!(matches!(failure.error(), Gfx942SdmaErrorV1::Timeout));
    assert_eq!(failure.retained_ticket(), Some(ticket));
    assert!(failure.into_indeterminate_completion().is_none());
}

#[test]
fn xgmi_post_completion_currentness_failure_retains_both_mappings() {
    let ticket = Gfx942SdmaCopyTicketV1 {
        owner: queue_key(7, 4, 1),
        queue_id: 17,
        slot: 3,
        generation: 9,
    };
    let completed = Gfx942XgmiCompletedCopyV1 {
        source: crate::shared_memory::xgmi_mapping_for_sdma_test(11),
        destination: crate::shared_memory::xgmi_mapping_for_sdma_test(12),
        copy_bytes: 4096,
    };
    let failure = classify_xgmi_wait_result(
        Ok(completed),
        Err(Gfx942SdmaErrorV1::Contract("injected post currentness")),
        ticket,
    )
    .err()
    .unwrap();
    assert!(failure.retained_ticket().is_none());
    let completed = failure.into_indeterminate_completion().unwrap();
    assert_eq!(completed.copy_bytes(), 4096);
    let (source, destination) = completed.into_mappings();
    assert_eq!(source.gpu_ids(), [7, 9]);
    assert_eq!(destination.gpu_ids(), [7, 9]);
    assert!(source.is_fully_mapped());
    assert!(destination.is_fully_mapped());
}

#[test]
fn gfx942_linear_copy_and_fence_match_the_pinned_packet_layout() {
    let packet = Gfx942SdmaCopySubmissionV1::new(
        0x1234_5678_9abc_def0,
        0xfedc_ba98_7654_3210,
        4096,
        0x1111_2222_3333_4448,
        7,
    )
    .unwrap();
    assert_eq!(word(&packet, 0), 1);
    assert_eq!(word(&packet, 1), 4095);
    assert_eq!(word(&packet, 2), 0);
    assert_eq!(word(&packet, 3), 0x9abc_def0);
    assert_eq!(word(&packet, 4), 0x1234_5678);
    assert_eq!(word(&packet, 5), 0x7654_3210);
    assert_eq!(word(&packet, 6), 0xfedc_ba98);
    assert_eq!(word(&packet, 7), 0x0053_0005);
    assert_eq!(word(&packet, 8), 0x3333_4448);
    assert_eq!(word(&packet, 9), 0x1111_2222);
    assert_eq!(word(&packet, 10), 7);
    assert!(packet.bytes[44..].iter().all(|byte| *byte == 0));
}

#[test]
fn invalid_sizes_addresses_and_completion_values_fail_closed() {
    assert_eq!(
        Gfx942SdmaCopySubmissionV1::new(0, 1, 1, 1, 1),
        Err(Gfx942SdmaPacketErrorV1::ZeroAddress)
    );
    assert_eq!(
        Gfx942SdmaCopySubmissionV1::new(1, 1, 0, 1, 1),
        Err(Gfx942SdmaPacketErrorV1::EmptyCopy)
    );
    assert_eq!(
        Gfx942SdmaCopySubmissionV1::new(1, 1, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1, 1, 1,),
        Err(Gfx942SdmaPacketErrorV1::CopyTooLarge)
    );
    assert_eq!(
        Gfx942SdmaCopySubmissionV1::new(1, 1, 1, 1, 0),
        Err(Gfx942SdmaPacketErrorV1::ZeroCompletionValue)
    );
    assert_eq!(
        Gfx942SdmaCopySubmissionV1::new(u64::MAX, 1, 2, 1, 1),
        Err(Gfx942SdmaPacketErrorV1::AddressOverflow)
    );
}

#[test]
fn overlap_check_is_half_open_and_overflow_fail_closed() {
    assert!(!ranges_overlap(0x1000, 16, 0x1010, 16));
    assert!(ranges_overlap(0x1000, 17, 0x1010, 16));
    assert!(ranges_overlap(u64::MAX, 2, 0, 1));
}

#[test]
fn fixed_batch_geometry_has_unique_slots_across_wrap() {
    assert_eq!(submission_batch_bytes(1).unwrap(), 64);
    assert_eq!(submission_batch_bytes(63).unwrap(), 4032);
    assert!(submission_batch_bytes(0).is_err());
    assert!(submission_batch_bytes(64).is_err());
    assert!(batch_ring_slot(1, 0).is_err());

    let slots = (0..GFX942_SDMA_RING_SLOT_COUNT_V1)
        .map(|index| batch_ring_slot(4032, index).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(slots[0], 63);
    assert_eq!(slots[1], 0);
    let mut unique = slots.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), GFX942_SDMA_RING_SLOT_COUNT_V1);

    assert!(sdma_ring_delta_is_below_capacity(4032, 0));
    assert!(!sdma_ring_delta_is_below_capacity(4096, 0));
    assert!(!sdma_ring_delta_is_below_capacity(63, 64));
}

#[test]
fn batch_publication_plan_has_one_exact_tail_for_fake_mmio() {
    #[derive(Default)]
    struct FakePublication {
        packet_writes: usize,
        write_publications: Vec<(u64, u64)>,
        doorbells: Vec<u64>,
    }

    let plan = admit_sdma_batch_publication_plan(4032, 4032 + 4 * 64, 4).unwrap();
    let mut fake = FakePublication::default();
    fake.packet_writes += plan.packet_count;
    fake.write_publications.push((plan.write, plan.write_end));
    fake.doorbells.push(plan.write_end);
    assert_eq!(fake.packet_writes, 4);
    assert_eq!(fake.write_publications, [(4032, 4288)]);
    assert_eq!(fake.doorbells, [4288]);

    assert!(admit_sdma_batch_publication_plan(1, 65, 1).is_err());
    assert!(admit_sdma_batch_publication_plan(0, 64, 0).is_err());
    assert!(admit_sdma_batch_publication_plan(0, 192, 2).is_err());
    assert!(admit_sdma_batch_publication_plan(0, 64 * 64, GFX942_SDMA_RING_SLOT_COUNT_V1).is_err());
}

#[test]
fn striped_queue_count_is_closed_to_balanced_gfx942_inventory() {
    for admitted in [2, 4, 6, 8, 10, 12, 14, 16] {
        assert!(striped_sdma_queue_count_is_admitted(admitted));
    }
    for rejected in [0, 1, 3, 15, 17, u32::MAX] {
        assert!(!striped_sdma_queue_count_is_admitted(rejected));
    }
}

#[test]
fn combined_queue_count_reserves_one_directional_queue_per_engine() {
    assert_eq!(GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_PER_ENGINE_V1, 7);
    assert_eq!(GFX942_SDMA_MAX_COMBINED_STRIPED_QUEUES_V1, 14);
    for admitted in [2, 4, 6, 8, 10, 12, 14] {
        assert!(combined_striped_sdma_queue_count_is_admitted(admitted));
    }
    for rejected in [0, 1, 3, 15, 16, 17, u32::MAX] {
        assert!(!combined_striped_sdma_queue_count_is_admitted(rejected));
    }
    assert!(
        striped_sdma_queue_count_is_admitted(16),
        "standalone striping retains the full eight queues per engine"
    );
}

#[test]
fn queue_set_creation_disposition_is_independent_of_retained_roster() {
    for earlier_boundary_crossed in [false, true] {
        for confirmed_owner_retained in [false, true] {
            for owner_failure_terminal in [false, true] {
                let expected = if earlier_boundary_crossed
                    || confirmed_owner_retained
                    || owner_failure_terminal
                {
                    Gfx942SdmaQueueSetCreationDispositionV1::Terminal
                } else {
                    Gfx942SdmaQueueSetCreationDispositionV1::Retryable
                };
                assert_eq!(
                    classify_sdma_queue_set_creation_failure(
                        earlier_boundary_crossed,
                        confirmed_owner_retained,
                        owner_failure_terminal,
                    ),
                    expected,
                );
            }
        }
    }

    assert_eq!(
        classify_sdma_queue_set_creation_failure(true, false, false),
        Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
        "a lower preflight failure is terminal after an earlier memory boundary",
    );
}

#[test]
fn xgmi_creation_failure_marks_terminal_without_requiring_queue_roster() {
    let retryable = Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
        error: Gfx942SdmaErrorV1::Contract("injected pure preflight"),
        disposition: Gfx942SdmaQueueSetCreationDispositionV1::Retryable,
        stage: None,
        host_preparation: false,
    };
    assert!(!retryable.is_terminal());

    let terminal_without_queue = Gfx942NativeXgmiSdmaQueueCreationFailureV1 {
        error: Gfx942SdmaErrorV1::Contract("injected post-route failure"),
        disposition: Gfx942SdmaQueueSetCreationDispositionV1::Terminal,
        stage: Some("memory-terminal-no-queue-custody"),
        host_preparation: false,
    };
    assert!(terminal_without_queue.is_terminal());
    assert_eq!(
        terminal_without_queue.terminal_stage(),
        Some("memory-terminal-no-queue-custody")
    );
}

#[test]
fn creation_guards_cover_the_first_memory_operation_and_xgmi_route_scope() {
    let source = include_str!("../../sdma.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let host_preparation = source
        .split("fn prepare_sdma_queue_host_resources()")
        .nth(1)
        .unwrap()
        .split("fn prepare_sdma_queue_host_resource_roster")
        .next()
        .unwrap();
    assert!(host_preparation.contains("preallocate_doorbell_failure_message"));
    let owner_create = source
        .split("fn create_with_engine_in_armed_scope(")
        .nth(1)
        .unwrap()
        .split("pub(crate) const fn observation")
        .next()
        .unwrap();
    assert!(owner_create.contains("&ProcessGlobalKfdRuntimeCreationArmV1"));
    assert!(!owner_create.contains("arm_process_global_kfd_runtime_gate_for_creation_v1"));
    assert!(!owner_create.contains("creation_arm.disarm()"));
    let opening_currentness = owner_create
        .find("memory.check_queue_currentness()")
        .unwrap();
    let promotion = owner_create
        .find("creation::finish_native_attempt")
        .unwrap();
    assert!(opening_currentness < promotion);

    let generic_create = include_str!("../../sdma/creation.rs")
        .split("pub(super) fn create_set(")
        .nth(1)
        .unwrap();
    let host = generic_create
        .find("prepare_sdma_queue_host_resource_roster")
        .unwrap();
    let arm = generic_create
        .find("arm_process_global_kfd_runtime_gate_for_creation_v1")
        .unwrap();
    let scoped_owner = generic_create
        .find("create_with_engine_in_armed_scope")
        .unwrap();
    let disarm = generic_create.find("creation_arm.disarm()").unwrap();
    assert!(host < arm && arm < scoped_owner && scoped_owner < disarm);

    let xgmi_create = source
        .split("impl Gfx942NativeXgmiSdmaQueueV1 {")
        .nth(1)
        .unwrap()
        .split("pub const fn route")
        .next()
        .unwrap();
    assert!(xgmi_create.contains("xgmi_creation::create(source, destination, route, root)"));
    let driver = include_str!("../../sdma/xgmi_creation.rs")
        .split("fn create_with(")
        .nth(1)
        .unwrap()
        .split("struct Sessions")
        .next()
        .unwrap();
    let host = driver.find(".prepare(route)").unwrap();
    let arm = driver.find(".arm()").unwrap();
    let begin = driver.find("root.begin(route)").unwrap();
    let opening = driver.find("context.validate(route)").unwrap();
    let lower = driver.find("context.create_owner").unwrap();
    let confirm = driver.find("root.confirm(owner)").unwrap();
    let closing = driver.rfind("context.validate(route)").unwrap();
    let disarm = driver.find("context.disarm").unwrap();
    let finish = driver.find("root.finish()").unwrap();
    assert!(host < arm && arm < begin && begin < opening && opening < lower);
    assert!(lower < confirm && confirm < closing && closing < disarm && disarm < finish);
    let raw = source
        .split("fn create_with_borrowed_attempt_in_armed_scope(")
        .nth(1)
        .unwrap()
        .split("pub(crate) const fn observation")
        .next()
        .unwrap();
    assert!(!raw.contains("attempted.take()"));
    assert!(!raw.contains("Gfx942SdmaQueueOwnerCreationFailureV1::terminal"));
    let xgmi_lower = source
        .split("fn create_on_xgmi_engine_in_armed_scope(")
        .nth(1)
        .unwrap()
        .split("fn create_with_engine_in_armed_scope(")
        .next()
        .unwrap();
    assert!(!xgmi_lower.contains("&mut None"));
    assert!(xgmi_lower.contains("create_with_borrowed_attempt_in_armed_scope"));
}

#[test]
fn progress_counts_pending_without_device_clock_claim() {
    let observed_at = Instant::now();
    let progress = Gfx942SdmaQueueProgressObservationV1 {
        queue_id: 17,
        submitted_count: 7,
        completed_count: 3,
        queue_write_bytes: 448,
        queue_read_bytes: 192,
        host_observed_at: observed_at,
    };
    assert_eq!(progress.queue_id(), 17);
    assert_eq!(progress.submitted_count(), 7);
    assert_eq!(progress.completed_count(), 3);
    assert_eq!(progress.pending_count(), 4);
    assert_eq!(progress.queue_write_bytes(), 448);
    assert_eq!(progress.queue_read_bytes(), 192);
    assert_eq!(progress.host_observed_at(), observed_at);
}

#[test]
fn directional_queue_ids_must_be_distinct() {
    assert!(directional_queue_ids_are_distinct(7, 8));
    assert!(!directional_queue_ids_are_distinct(7, 7));
}

#[test]
fn xgmi_destroy_borrows_queue_until_native_destroy_succeeds() {
    type DestroyXgmiQueueV1 = fn(
        &mut Gfx942NativeXgmiSdmaQueueV1,
        &mut SharedGttMemorySessionV1,
        &mut SharedGttMemorySessionV1,
    ) -> Result<(), Gfx942SdmaErrorV1>;

    let _: DestroyXgmiQueueV1 = Gfx942NativeXgmiSdmaQueueV1::destroy_and_release;
    type CreateXgmiQueueV1 =
        fn(
            &mut SharedGttMemorySessionV1,
            &mut SharedGttMemorySessionV1,
            crate::topology::Gfx942XgmiRouteV1,
            &mut Gfx942NativeXgmiSdmaQueueCreationRootV1,
        )
            -> Result<Gfx942NativeXgmiSdmaQueueV1, Gfx942NativeXgmiSdmaQueueCreationFailureV1>;
    let _: CreateXgmiQueueV1 = Gfx942NativeXgmiSdmaQueueV1::create;
}

#[test]
fn counter_and_generation_invariant_failures_are_terminal() {
    let mut poisoned = false;
    assert!(validate_sdma_write_counter_or_poison(1, &mut poisoned).is_err());
    assert!(poisoned);

    let mut poisoned = false;
    assert_eq!(next_sdma_ticket_generation(7, &mut poisoned).unwrap(), 8);
    assert!(!poisoned);
    assert!(next_sdma_ticket_generation(u32::MAX, &mut poisoned).is_err());
    assert!(poisoned);

    let mut poisoned = false;
    assert!(checked_sdma_write_end(u64::MAX - 63, 64, &mut poisoned).is_err());
    assert!(poisoned);
}

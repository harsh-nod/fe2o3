//! Real mapped-owner/packet algorithms with synthetic fences, not GPU payload execution.

use super::*;
use fe2o3_runtime_model::OrderedPeerCopySegmentV1;

fn segment(source_offset: u64, destination_offset: u64, byte_len: u64) -> OrderedPeerCopySegmentV1 {
    OrderedPeerCopySegmentV1 {
        source_offset,
        destination_offset,
        byte_len,
    }
}

fn segments_fixture(
    configured: bool,
    plan: Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    physical: [usize; 2],
    packetized: bool,
) -> (Pair, TransferRoot, Before) {
    let (pair, mut root, before) =
        fixture_window(configured, plan.windows()[0], physical, packetized);
    root.core = CopyPlan::Segments(plan)
        .admit(GPU_IDS, before.logical_bytes[0], before.logical_bytes[1])
        .unwrap();
    (pair, root, before)
}

fn small_fixture() -> (Pair, TransferRoot, Before) {
    let plan = Arc::new(
        Gfx942ComputeXgmiSegmentsPlanV1::new(
            2048,
            4097,
            31,
            1900,
            127,
            3900,
            &[segment(17, 11, 333), segment(71, 41, 127)],
        )
        .unwrap(),
    );
    segments_fixture(true, plan, PHYSICAL_BYTES, false)
}

fn complete_current(pair: &mut Pair, root: &mut TransferRoot) {
    pair.queue
        .complete(&mut pair.memory[0], root.core.copy_custody_for_test());
}

fn first_segment_completed(pair: &mut Pair, root: &mut TransferRoot) {
    begin(pair, root);
    complete_current(pair, root);
    assert_eq!(progress(pair, root), Gfx942ComputeXgmiProgressV1::Changed);
    assert_eq!(root.phase, Phase::Published);
    assert_eq!(pair.packets.len(), 1);
}

fn assert_single_mapping_pair(pair: &Pair) {
    for memory in &pair.memory {
        let state = memory.compute_xgmi_snapshot_v1();
        assert_eq!(state.maps, [(GPU_IDS.to_vec(), 0)]);
        assert_eq!(state.unmaps, [(GPU_IDS.to_vec(), 0)]);
    }
}

#[test]
fn compute_xgmi_segments_composed_nested_packets_keep_order_and_final_only_restoration() {
    let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let plan = Arc::new(
        Gfx942ComputeXgmiSegmentsPlanV1::new(
            cap + 2048,
            cap + 4097,
            19,
            cap + 1024,
            127,
            cap + 2000,
            &[
                segment(0, 31, cap + 37),
                segment(47, 41, 129),
                segment(0, 31, cap + 37),
            ],
        )
        .unwrap(),
    );
    let expected = [
        (19, 158, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        (19 + cap, 158 + cap, 37),
        (66, 168, 129),
        (19, 158, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        (19 + cap, 158 + cap, 37),
    ];
    for configured in [false, true] {
        let (mut pair, mut root, before) =
            segments_fixture(configured, Arc::clone(&plan), [0x402000, 0x404000], true);
        pair.queue.start_at_ring_tail(&mut pair.memory[0]);
        begin(&mut pair, &mut root);
        let mut tickets = Vec::new();
        for (index, (source_offset, destination_offset, bytes)) in expected.into_iter().enumerate()
        {
            let packet = Gfx942ComputeXgmiCopyPacketV1 {
                source_offset,
                destination_offset,
                bytes,
            };
            assert_eq!(pair.packets[index], packet);
            let snapshot = pair.queue.snapshot(&pair.memory[0]);
            let slot = ((snapshot.doorbell / 64 - 1) % 64) as usize;
            assert_eq!(slot, (63 + index) % 64);
            let start = slot * 64;
            let fence =
                u64::from_le_bytes(snapshot.ring[start + 32..start + 40].try_into().unwrap());
            let encoded = crate::sdma::Gfx942SdmaCopySubmissionV1::new(
                before.addresses[0] + source_offset,
                before.addresses[1] + destination_offset,
                bytes,
                fence,
                snapshot.generations[slot],
            )
            .unwrap();
            assert_eq!(&snapshot.ring[start..start + 64], encoded.bytes());
            let ticket = root.core.copy_custody_for_test().ticket.unwrap();
            assert!(!tickets.contains(&ticket));
            tickets.push(ticket);
            assert_eq!(
                progress(&mut pair, &mut root),
                Gfx942ComputeXgmiProgressV1::Pending
            );
            assert_eq!(pair.queue.snapshot(&pair.memory[0]), snapshot);
            assert!(
                root.finish(&mut pair, |pair, core| core.finish_with(pair))
                    .is_err()
            );
            complete_current(&mut pair, &mut root);
            let last = index + 1 == expected.len();
            assert_eq!(
                root.poll(&mut pair, |pair, core| core.poll_with(pair))
                    .unwrap(),
                last
            );
            let completed = pair.queue.snapshot(&pair.memory[0]);
            for _ in 0..2 {
                assert_eq!(
                    root.poll(&mut pair, |pair, core| core.poll_with(pair))
                        .unwrap(),
                    last
                );
                assert_eq!(pair.queue.snapshot(&pair.memory[0]), completed);
                assert_eq!(pair.packets.len(), index + 1);
                assert_eq!(root.core.copy_custody_for_test().ticket, Some(ticket));
                assert!(root.core.copy_custody_for_test().completed.is_some());
                assert_original_owners(&root, &before, false);
                assert_conserved(&pair, &root, &before);
            }
            if !last {
                assert_eq!(root.phase, Phase::Published);
                assert_eq!(
                    progress(&mut pair, &mut root),
                    Gfx942ComputeXgmiProgressV1::Changed
                );
                assert_eq!(pair.packets.len(), index + 2);
            }
        }
        assert_eq!(root.phase, Phase::Ready);
        let retakes = pair.retakes;
        assert_eq!(
            progress(&mut pair, &mut root),
            Gfx942ComputeXgmiProgressV1::Ready
        );
        assert_eq!(pair.retakes, retakes);
        root.finish(&mut pair, |pair, core| core.finish_with(pair))
            .unwrap();
        assert_eq!(root.phase, Phase::Finished);
        assert_original_owners(&root, &before, true);
        assert_conserved(&pair, &root, &before);
        assert_models_retaken(&pair, &before, pair.retakes[0] as u64);
        assert_single_mapping_pair(&pair);
        assert_eq!(pair.waits, 0);
    }
}

#[test]
fn compute_xgmi_segments_composed_sync_crosses_ring_capacity_with_one_mapping_pair() {
    let descriptors: Vec<_> = (0..65)
        .map(|index| segment(index * 3, (index % 3) * 7, 17))
        .collect();
    let plan = Arc::new(
        Gfx942ComputeXgmiSegmentsPlanV1::new(2048, 4097, 31, 1900, 127, 3900, &descriptors)
            .unwrap(),
    );
    let (mut pair, mut root, before) = segments_fixture(true, plan, PHYSICAL_BYTES, false);
    pair.complete_on_submit = true;
    root.execute(&mut pair, |pair, core| {
        core.run_with(pair, Duration::from_secs(10))
    })
    .unwrap();
    assert_eq!(pair.packets.len(), 65);
    for (packet, descriptor) in pair.packets.iter().zip(descriptors) {
        assert_eq!(
            *packet,
            Gfx942ComputeXgmiCopyPacketV1 {
                source_offset: 31 + descriptor.source_offset,
                destination_offset: 127 + descriptor.destination_offset,
                bytes: 17,
            }
        );
    }
    assert_eq!(pair.waits, 65);
    assert_eq!(pair.retakes, [1; 2]);
    assert_original_owners(&root, &before, true);
    assert_conserved(&pair, &root, &before);
    assert_models_retaken(&pair, &before, 1);
    assert_single_mapping_pair(&pair);
}

#[test]
fn compute_xgmi_segments_admission_rejects_changed_logical_extents_before_extraction() {
    let (pair, root, before) = small_fixture();
    for (source, destination) in [(4096, 4097), (2048, 8192)] {
        let plan = Arc::new(
            Gfx942ComputeXgmiSegmentsPlanV1::new(
                source,
                destination,
                0,
                source,
                0,
                destination,
                &[segment(0, 0, 1)],
            )
            .unwrap(),
        );
        assert!(CopyPlan::Segments(plan).admit(GPU_IDS, 2048, 4097).is_err());
        assert_original_owners(&root, &before, true);
        assert_conserved(&pair, &root, &before);
        assert_eq!(pair.retakes, [0; 2]);
        assert!(pair.packets.is_empty());
    }
}

#[test]
fn compute_xgmi_segments_later_publication_errors_and_unwinds_retain_exact_pair() {
    for operation in ["reset", "ring", "control", "doorbell"] {
        for panic in [false, true] {
            let (mut pair, mut root, before) = small_fixture();
            first_segment_completed(&mut pair, &mut root);
            pair.queue.fault(&mut pair.memory[0], operation, panic);
            let result = catch_unwind(AssertUnwindSafe(|| {
                root.progress(&mut pair, |pair, core| core.progress_with(pair))
            }));
            assert_eq!(result.is_err(), panic);
            if !panic {
                assert!(result.unwrap().is_err());
            }
            assert_terminal(&pair, &root, &before);
            let snapshot = pair.queue.snapshot(&pair.memory[0]);
            assert_eq!(snapshot.retained, before.identities);
            assert!(snapshot.uncertain.is_some());
            assert_eq!(pair.packets.len(), 2);
            assert_eq!(pair.retakes, [3; 2]);
        }
    }
}

#[test]
fn compute_xgmi_segments_later_poll_and_closing_errors_preserve_completed_custody() {
    for closing in [false, true] {
        for panic in [false, true] {
            let (mut pair, mut root, before) = small_fixture();
            first_segment_completed(&mut pair, &mut root);
            assert_eq!(
                progress(&mut pair, &mut root),
                Gfx942ComputeXgmiProgressV1::Changed
            );
            if closing {
                complete_current(&mut pair, &mut root);
                pair.memory[0].sdma_fail_operational_currentness_v1(2, panic);
            } else {
                pair.queue.fault(&mut pair.memory[0], "observe", panic);
            }
            let result = catch_unwind(AssertUnwindSafe(|| {
                root.progress(&mut pair, |pair, core| core.progress_with(pair))
            }));
            assert_eq!(result.is_err(), panic);
            if !panic {
                assert!(result.unwrap().is_err());
            }
            assert_terminal(&pair, &root, &before);
            assert_eq!(
                root.core.copy_custody_for_test().completed.is_some(),
                closing
            );
            assert_eq!(pair.retakes, [4; 2]);
        }
    }
}

#[test]
fn compute_xgmi_segments_boundary_retake_failures_never_restore_partial_outputs() {
    for phase in 0..4 {
        for endpoint in 0..2 {
            for after in [false, true] {
                for panic in [false, true] {
                    let (mut pair, mut root, before) = small_fixture();
                    if phase == 0 {
                        begin(&mut pair, &mut root);
                        complete_current(&mut pair, &mut root);
                    } else {
                        first_segment_completed(&mut pair, &mut root);
                    }
                    if phase >= 2 {
                        assert_eq!(
                            progress(&mut pair, &mut root),
                            Gfx942ComputeXgmiProgressV1::Changed
                        );
                        complete_current(&mut pair, &mut root);
                    }
                    if phase == 3 {
                        assert_eq!(
                            progress(&mut pair, &mut root),
                            Gfx942ComputeXgmiProgressV1::Ready
                        );
                    }
                    let retakes = pair.retakes;
                    pair.retake_fault = Some((endpoint, after, panic));
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if phase == 3 {
                            root.finish(&mut pair, |pair, core| core.finish_with(pair))
                        } else {
                            root.progress(&mut pair, |pair, core| core.progress_with(pair))
                                .map(|_| ())
                        }
                    }));
                    assert_eq!(result.is_err(), panic);
                    if !panic {
                        assert!(result.unwrap().is_err());
                    }
                    assert_terminal(&pair, &root, &before);
                    assert_eq!(pair.retakes, retakes.map(|n| n + 1));
                    let retakes = pair.retakes;
                    assert!(
                        root.progress(&mut pair, |pair, core| core.progress_with(pair))
                            .is_err()
                    );
                    assert_eq!(pair.retakes, retakes);
                }
            }
        }
    }
}

//! Real owner transitions and SDMA packets; completion injection is not GPU data execution.

use super::*;

fn subrange_fixture(configured: bool) -> (Pair, TransferRoot, Before) {
    fixture_window(
        configured,
        Gfx942ComputeXgmiCopyWindowV1::new(2048, 4097, 31, 127, 333).unwrap(),
        PHYSICAL_BYTES,
        false,
    )
}

#[test]
fn compute_xgmi_subrange_composed_tail_wrap_keeps_complete_unequal_owners() {
    let cap = u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1);
    let bytes = 2 * cap + 37;
    let window =
        Gfx942ComputeXgmiCopyWindowV1::new(bytes + 777, bytes + 1777, 91, 1033, bytes).unwrap();
    for configured in [false, true] {
        let (mut pair, mut root, before) =
            fixture_window(configured, window, [0x802000, 0x804000], true);
        assert_ne!(before.logical_bytes[0], before.logical_bytes[1]);
        assert_ne!(before.identities[0], before.identities[1]);
        assert_ne!(before.addresses[0], before.addresses[1]);
        pair.queue.start_at_ring_tail(&mut pair.memory[0]);
        begin(&mut pair, &mut root);
        let mut tickets = Vec::new();
        for index in 0..3 {
            let relative = index as u64 * cap;
            let packet = Gfx942ComputeXgmiCopyPacketV1 {
                source_offset: 91 + relative,
                destination_offset: 1033 + relative,
                bytes: if index == 2 {
                    37
                } else {
                    GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1
                },
            };
            assert_eq!(pair.packets[index], packet);
            let snapshot = pair.queue.snapshot(&pair.memory[0]);
            let slot = ((snapshot.doorbell / 64 - 1) % 64) as usize;
            assert_eq!(slot, [63, 0, 1][index]);
            let start = slot * 64;
            let fence =
                u64::from_le_bytes(snapshot.ring[start + 32..start + 40].try_into().unwrap());
            let expected = crate::sdma::Gfx942SdmaCopySubmissionV1::new(
                before.addresses[0] + packet.source_offset,
                before.addresses[1] + packet.destination_offset,
                packet.bytes,
                fence,
                snapshot.generations[slot],
            )
            .unwrap();
            assert_eq!(&snapshot.ring[start..start + 64], expected.bytes());
            let ticket = root.core.copy_custody_for_test().ticket.unwrap();
            assert!(!tickets.contains(&ticket));
            tickets.push(ticket);
            for _ in 0..2 {
                assert!(
                    !root
                        .poll(&mut pair, |pair, core| core.poll_with(pair))
                        .unwrap()
                );
                assert_eq!(pair.queue.snapshot(&pair.memory[0]), snapshot);
                assert_eq!(root.core.copy_custody_for_test().ticket, Some(ticket));
                assert_original_owners(&root, &before, false);
                assert_conserved(&pair, &root, &before);
            }
            pair.queue
                .complete(&mut pair.memory[0], root.core.copy_custody_for_test());
            assert_eq!(
                root.poll(&mut pair, |pair, core| core.poll_with(pair))
                    .unwrap(),
                index == 2
            );
            assert_original_owners(&root, &before, false);
            assert_conserved(&pair, &root, &before);
            if index != 2 {
                assert_eq!(
                    progress(&mut pair, &mut root),
                    Gfx942ComputeXgmiProgressV1::Changed
                );
            }
        }
        assert_eq!(root.phase, Phase::Ready);
        root.finish(&mut pair, |pair, core| core.finish_with(pair))
            .unwrap();
        assert_eq!(root.phase, Phase::Finished);
        assert_original_owners(&root, &before, true);
        assert_conserved(&pair, &root, &before);
        assert_models_retaken(&pair, &before, 13);
        assert_eq!(pair.retakes, [13; 2]);
        assert_eq!(pair.waits, 0);
        for memory in &pair.memory {
            let state = memory.compute_xgmi_snapshot_v1();
            assert_eq!(state.maps, [(GPU_IDS.to_vec(), 0)]);
            assert_eq!(state.unmaps, [(GPU_IDS.to_vec(), 0)]);
        }
    }
}

#[test]
fn compute_xgmi_subrange_sync_uses_original_owners_and_single_mapping_sequence() {
    let (mut pair, mut root, before) = subrange_fixture(true);
    pair.complete_on_submit = true;
    root.execute(&mut pair, |pair, core| {
        core.run_with(pair, Duration::from_secs(1))
    })
    .unwrap();
    assert_eq!(
        pair.packets,
        [Gfx942ComputeXgmiCopyPacketV1 {
            source_offset: 31,
            destination_offset: 127,
            bytes: 333
        }]
    );
    assert_eq!(pair.waits, 1);
    assert_eq!(pair.retakes, [1; 2]);
    assert_original_owners(&root, &before, true);
    assert_conserved(&pair, &root, &before);
    assert_models_retaken(&pair, &before, 1);
}

#[test]
fn compute_xgmi_subrange_admission_rejects_padding_or_changed_extent_before_take() {
    let (pair, root, before) = subrange_fixture(true);
    let logical = root.allocations.each_ref().map(|slot| {
        let allocation = slot.as_ref().unwrap();
        admit_allocation(allocation, allocation.attachment.queue, true).unwrap()
    });
    assert_eq!(logical, [2048, 4097]);
    assert!(
        admit_window(logical[0], logical[1], None).is_err(),
        "old full API still rejects unequal logical extents"
    );
    for window in [
        Gfx942ComputeXgmiCopyWindowV1::new(4096, 4097, 2048, 0, 1).unwrap(),
        Gfx942ComputeXgmiCopyWindowV1::new(2048, 8192, 0, 4097, 1).unwrap(),
        Gfx942ComputeXgmiCopyWindowV1::new(2049, 4097, 31, 127, 333).unwrap(),
        Gfx942ComputeXgmiCopyWindowV1::new(2048, 4098, 31, 127, 333).unwrap(),
    ] {
        assert!(admit_window(logical[0], logical[1], Some(window)).is_err());
        assert_original_owners(&root, &before, true);
        assert_conserved(&pair, &root, &before);
        assert_eq!(pair.retakes, [0; 2]);
        assert!(pair.packets.is_empty());
    }
    assert_eq!(
        admit_window(2048, 2048, None).unwrap(),
        Gfx942ComputeXgmiCopyWindowV1::new(2048, 2048, 0, 0, 2048).unwrap()
    );
}

#[test]
fn compute_xgmi_subrange_mapping_prefix_errors_and_unwinds_retain_both_owners() {
    for endpoint in 0..2 {
        for stage in 0..4 {
            for panic in [false, true] {
                let (mut pair, mut root, before) = subrange_fixture(true);
                if stage >= 2 {
                    begin(&mut pair, &mut root);
                    ready(&mut pair, &mut root);
                }
                let retakes = pair.retakes;
                pair.memory[endpoint].compute_xgmi_fault_v1(
                    stage,
                    u32::from(stage == 1 || stage == 2),
                    false,
                    panic,
                );
                let result = catch_unwind(AssertUnwindSafe(|| {
                    if stage < 2 {
                        root.begin(&mut pair, |pair, core| core.begin_with(pair))
                    } else {
                        root.finish(&mut pair, |pair, core| core.finish_with(pair))
                    }
                }));
                assert_eq!(result.is_err(), panic);
                if !panic {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(pair.retakes, retakes.map(|n| n + 1));
                assert_terminal(&pair, &root, &before);
            }
        }
    }
}

#[test]
fn compute_xgmi_subrange_publication_and_observation_faults_preserve_exact_custody() {
    for operation in ["reset", "ring", "control", "doorbell", "observe"] {
        for panic in [false, true] {
            let (mut pair, mut root, before) = subrange_fixture(true);
            if operation == "observe" {
                begin(&mut pair, &mut root);
            }
            let ticket = root.core.copy_custody_for_test().ticket;
            pair.queue.fault(&mut pair.memory[0], operation, panic);
            let result = catch_unwind(AssertUnwindSafe(|| {
                if operation == "observe" {
                    root.poll(&mut pair, |pair, core| core.poll_with(pair))
                        .map(|_| ())
                } else {
                    root.begin(&mut pair, |pair, core| core.begin_with(pair))
                }
            }));
            assert_eq!(result.is_err(), panic);
            if !panic {
                assert!(result.unwrap().is_err());
            }
            assert_terminal(&pair, &root, &before);
            assert_eq!(pair.queue.retained_identities(), before.identities);
            if operation == "observe" {
                assert_eq!(root.core.copy_custody_for_test().ticket, ticket);
            } else {
                assert!(pair.queue.snapshot(&pair.memory[0]).uncertain.is_some());
            }
        }
    }
}

#[test]
fn compute_xgmi_subrange_all_model_retake_boundaries_conserve_native_authority() {
    for phase in 0..4 {
        for endpoint in 0..2 {
            for after in [false, true] {
                for panic in [false, true] {
                    let (mut pair, mut root, before) = subrange_fixture(true);
                    if phase > 0 {
                        begin(&mut pair, &mut root);
                    }
                    if phase >= 2 {
                        pair.queue
                            .complete(&mut pair.memory[0], root.core.copy_custody_for_test());
                    }
                    if phase == 3 {
                        assert!(
                            root.poll(&mut pair, |pair, core| core.poll_with(pair))
                                .unwrap()
                        );
                    }
                    let retakes = pair.retakes;
                    pair.retake_fault = Some((endpoint, after, panic));
                    let result = catch_unwind(AssertUnwindSafe(|| match phase {
                        0 => root.begin(&mut pair, |pair, core| core.begin_with(pair)),
                        1 | 2 => root
                            .poll(&mut pair, |pair, core| core.poll_with(pair))
                            .map(|_| ()),
                        _ => root.finish(&mut pair, |pair, core| core.finish_with(pair)),
                    }));
                    assert_eq!(result.is_err(), panic);
                    if !panic {
                        assert!(result.unwrap().is_err());
                    }
                    assert_eq!(pair.retakes, retakes.map(|n| n + 1));
                    assert_terminal(&pair, &root, &before);
                }
            }
        }
    }
}

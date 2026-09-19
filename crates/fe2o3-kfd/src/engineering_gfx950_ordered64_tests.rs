use super::*;
use std::io::Cursor;

fn command64(count: usize, timeout_ms: u32) -> CommandV1 {
    let CommandV1::DispatchOrderedBatch { dispatches, .. } = command(count, timeout_ms) else {
        unreachable!()
    };
    CommandV1::DispatchOrderedBatch64 {
        dispatches,
        timeout_ms,
    }
}

#[test]
fn ordered64_wire_is_distinct_and_preserves_all_legacy_limits() {
    assert_eq!(MAX_ORDERED_BATCH_DISPATCHES_V1, 16);
    assert_eq!(MAX_SEQUENCE_DISPATCHES_V1, 16);
    assert_eq!(MAX_HEADER_BYTES_V1, 65_536);
    assert_eq!(MAX_TRANSFER_BYTES_V1, 4 * 1024 * 1024);
    for count in 1..=64 {
        let command = command64(count, 600_000);
        assert_eq!(command.payload_bytes().unwrap(), count * 65_536);
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &command).unwrap();
        bytes.push(0xa5);
        let mut cursor = Cursor::new(bytes);
        assert_eq!(
            read_header_v1::<CommandV1>(&mut cursor).unwrap(),
            Some(command)
        );
        assert_eq!(cursor.get_ref()[cursor.position() as usize], 0xa5);
    }
    assert_eq!(
        command64(64, 1).payload_bytes().unwrap(),
        MAX_TRANSFER_BYTES_V1 as usize
    );
    for (count, timeout) in [(0, 1), (65, 1), (64, 0), (64, 600_001)] {
        assert!(command64(count, timeout).payload_bytes().is_err());
    }
    assert!(command(17, 1).payload_bytes().is_err());
    assert!(require_sequence_capacity(0, 0, 17).is_err());
    let response = ResponseV1::DispatchOrderedBatch64Completed {
        completed_dispatches: 64,
        elapsed_ns: 123,
    };
    let json = serde_json::to_value(&response).unwrap();
    assert_eq!(json["op"], "dispatch_ordered_batch64_completed");
    assert!(json.get("kernel_elapsed_ns").is_none());
    assert_eq!(
        serde_json::from_value::<ResponseV1>(json).unwrap(),
        response
    );
    let mut json = serde_json::to_value(command64(1, 1)).unwrap();
    json["dispatches"][0]["timeout_ms"] = 1.into();
    assert!(serde_json::from_value::<CommandV1>(json).is_err());
    let mut json = serde_json::to_value(command64(1, 1)).unwrap();
    json["op"] = "dispatch_ordered_batch128".into();
    assert!(serde_json::from_value::<CommandV1>(json).is_err());
}

#[test]
fn ordered64_last_payload_and_pointer_limit_remain_bounded() {
    let pointer = PointerFixupV1 {
        kernarg_offset: 0,
        buffer: 1,
        buffer_offset: 0,
        extent_bytes: 8,
        access: BufferAccessV1::Read,
    };
    for too_many_pointers in [false, true] {
        let mut command = command64(64, 1);
        let CommandV1::DispatchOrderedBatch64 { dispatches, .. } = &mut command else {
            unreachable!()
        };
        if too_many_pointers {
            dispatches[63].pointers = vec![pointer.clone(); MAX_POINTER_FIXUPS_V1 + 1];
        } else {
            dispatches[63].payload_bytes = MAX_KERNARG_BYTES_V1 + 1;
        }
        assert!(command.payload_bytes().is_err());
    }
}

#[test]
fn ordered64_pointer_rich_headers_fail_before_any_frame_bytes() {
    let pointer = PointerFixupV1 {
        kernarg_offset: u32::MAX,
        buffer: u64::MAX,
        buffer_offset: u64::MAX,
        extent_bytes: u64::MAX,
        access: BufferAccessV1::ReadWrite,
    };
    for (pointers, fits) in [(3, true), (13, false)] {
        let mut command = command64(64, 1);
        let CommandV1::DispatchOrderedBatch64 { dispatches, .. } = &mut command else {
            unreachable!()
        };
        for dispatch in dispatches {
            dispatch.pointers = vec![pointer.clone(); pointers];
        }
        assert!(command.payload_bytes().is_ok());
        let raw = serde_json::to_vec(&command).unwrap();
        assert_eq!(raw.len() <= MAX_HEADER_BYTES_V1, fits);
        let mut frame = Vec::new();
        assert_eq!(write_header_v1(&mut frame, &command).is_ok(), fits);
        if !fits {
            assert!(frame.is_empty());
            let mut incoming = (raw.len() as u32).to_le_bytes().to_vec();
            incoming.extend_from_slice(&raw);
            let mut cursor = Cursor::new(incoming);
            assert!(read_header_v1::<CommandV1>(&mut cursor).is_err());
            assert_eq!(cursor.position(), 4);
        }
    }
}

#[test]
fn ordered64_all_storage_slots_are_bounded_and_modes_do_not_alias() {
    let mode = OrderedMode::Batch64;
    assert_eq!(mode.arena_bytes(), 4 * 1024 * 1024);
    assert_eq!(
        MAX_ORDERED_BATCH64_DISPATCHES_V1 * AMD_SIGNAL_BYTES_V1,
        PAGE_BYTES
    );
    for index in 0..64 {
        assert_eq!(
            mode.slot_offsets(index, 65_536).unwrap(),
            (index * 65_536, index * 64)
        );
    }
    for (index, bytes) in [(64, 0), (usize::MAX, 0), (63, 65_537)] {
        assert!(mode.slot_offsets(index, bytes).is_err());
    }
    for mode in [OrderedMode::V1, OrderedMode::Batch64] {
        mode.require_arena(mode.arena_bytes()).unwrap();
        assert!(mode.require_arena(mode.arena_bytes() - 1).is_err());
        assert!(mode.require_arena(mode.arena_bytes() + 1).is_err());
    }
    assert!(
        OrderedMode::V1
            .require_arena(OrderedMode::Batch64.arena_bytes())
            .is_err()
    );
    assert!(
        OrderedMode::Batch64
            .require_arena(OrderedMode::V1.arena_bytes())
            .is_err()
    );
    assert!(OrderedMode::V1.slot_offsets(16, 0).is_err());
    assert!(batch64_count::<0>().is_err());
    assert_eq!(batch64_count::<64>().unwrap(), 64);
    assert!(batch64_count::<65>().is_err());
    assert!(batch_count::<17>().is_err());
}

#[test]
fn ordered64_ring_wrap_capacity_overflow_frontiers_and_identity_stay_exact() {
    let limit = MAX_UNRETIRED_RING_PACKETS_V1;
    require_ordered64_capacity(limit - 64, 0, 64).unwrap();
    for (write, read, count) in [
        (0, 0, 0),
        (0, 0, 65),
        (limit - 63, 0, 64),
        (limit, 0, 1),
        (0, 1, 1),
        (u64::MAX, u64::MAX, 1),
    ] {
        assert!(require_ordered64_capacity(write, read, count).is_err());
    }
    require_ordered64_capacity(limit, 64, 64).unwrap();
    let mut ring = AqlSingleProducerRingModelV1::new(
        AqlRingCapacityV1::from_ring_bytes(8192).unwrap(),
        127,
        127,
    )
    .unwrap();
    let reservation = ring.reserve_fixed_batch_v2(127, 64).unwrap();
    assert_eq!(reservation.next_write(), 191);
    for index in 0..64 {
        assert_eq!(
            reservation.entry(index).unwrap().slot_index(),
            (127 + index) % 128
        );
    }
    assert!(reservation.entry(64).is_none());
    assert!(require_completed_frontier(127, 191).is_err());
    for axis in 0..3 {
        let mut changed = [42, 7, 191];
        changed[axis] += 1;
        assert!(require_pending_dispatch_identity([42, 7, 191], changed, false).is_err());
    }
    for (counters, exception) in [
        ((190, 127), 0),
        ((191, 126), 0),
        ((191, 192), 0),
        ((191, 127), 1),
    ] {
        assert!(
            dispatch_completed(
                191,
                127,
                counters,
                AqlCompletionObservationV1::Completed,
                exception
            )
            .is_err()
        );
    }
    assert!(
        dispatch_completed(
            191,
            127,
            (191, 127),
            AqlCompletionObservationV1::Completed,
            0
        )
        .unwrap()
    );
}

#[test]
fn ordered64_every_stage_failure_poison_preserves_the_exact_published_prefix() {
    for count in [1, 16, 17, 40, 63, 64] {
        let mut success = Fake::default();
        run_ordered_batch_mode(&mut success, count, 600_000, OrderedMode::Batch64).unwrap();
        assert!(success.completed && !success.poisoned);
        assert_eq!(success.retained, count);
        assert_eq!(success.events.first().unwrap(), "dispatch_fence");
        assert_eq!(success.events.last().unwrap(), "dispatch_fence");
        for event in [
            "preparation_fence",
            "write_reservation",
            "doorbell",
            "poll_final_identity_counters_exception",
        ] {
            assert_eq!(
                success
                    .events
                    .iter()
                    .filter(|entry| entry.as_str() == event)
                    .count(),
                1
            );
        }
        assert_eq!(
            success
                .events
                .iter()
                .filter(|event| event.starts_with("validate_signal:"))
                .count(),
            count
        );
        for fail_at in 0..success.events.len() {
            let mut fake = Fake {
                fail_at: Some(fail_at),
                ..Fake::default()
            };
            assert!(
                run_ordered_batch_mode(&mut fake, count, 600_000, OrderedMode::Batch64).is_err()
            );
            assert!(fake.poisoned);
            assert_eq!(fake.events, success.events[..=fail_at]);
            if fake.events.iter().any(|event| event == "write_reservation") {
                assert_eq!(fake.retained, count);
            }
            if !fake.events.iter().any(|event| event == "complete_frontier") {
                assert!(!fake.completed);
            }
            assert!(
                run_ordered_batch_mode(&mut fake, count, 600_000, OrderedMode::Batch64).is_err()
            );
            assert_eq!(fake.events, success.events[..=fail_at]);
        }
    }
}

#[test]
fn ordered64_each_preparation_fault_is_rejected_before_storage_or_publication() {
    for after in 0..64 {
        for fault in [
            PreparationFault::Reset,
            PreparationFault::Poison,
            PreparationFault::Exception,
            PreparationFault::Frontier,
        ] {
            let mut fake = Fake {
                fault_after_prepare: Some((after, fault)),
                ..Fake::default()
            };
            assert!(run_ordered_batch_mode(&mut fake, 64, 600_000, OrderedMode::Batch64).is_err());
            assert!(fake.poisoned && !fake.completed);
            assert_eq!(fake.retained, 0);
            assert_eq!(fake.events.len(), 66);
            assert_eq!(fake.events.last().unwrap(), "preparation_fence");
        }
    }
}

#[test]
fn ordered64_each_signal_and_aggregate_deadline_remain_required() {
    for index in 0..64 {
        for fault in [
            AqlCompletionObservationV1::Pending,
            AqlCompletionObservationV1::Unexpected(-1),
        ] {
            let mut signals = [AqlCompletionObservationV1::Completed; 64];
            signals[index] = fault;
            assert!(require_signals_complete(signals).is_err());
        }
    }
    for (count, timeout) in [(0, 1), (65, 1), (64, 0), (64, 600_001)] {
        let mut fake = Fake::default();
        assert!(run_ordered_batch_mode(&mut fake, count, timeout, OrderedMode::Batch64).is_err());
        assert!(fake.poisoned && fake.events.is_empty());
    }
    let mut fake = Fake {
        pending_polls: usize::MAX,
        pause_millis: 2,
        ..Fake::default()
    };
    assert!(run_ordered_batch_mode(&mut fake, 64, 1, OrderedMode::Batch64).is_err());
    assert!(fake.poisoned && !fake.completed);
    assert_eq!(fake.retained, 64);
    assert!(
        !fake
            .events
            .iter()
            .any(|event| event.starts_with("validate_signal:"))
    );
}

#[test]
fn ordered64_queue_epoch_mode_change_rejects_before_staging_and_poison_is_terminal() {
    for (requested, retained) in [
        (OrderedMode::V1, OrderedMode::Batch64),
        (OrderedMode::Batch64, OrderedMode::V1),
    ] {
        let mut fake = Fake {
            retained_arena: Some((requested, retained.arena_bytes())),
            ..Fake::default()
        };
        assert!(run_ordered_batch_mode(&mut fake, 1, 600_000, requested).is_err());
        assert!(fake.poisoned && !fake.completed);
        assert_eq!(fake.retained, 0);
        assert_eq!(
            fake.events,
            ["dispatch_fence", "prepare:0", "preparation_fence"]
        );
        let before = fake.events.clone();
        assert!(run_ordered_batch_mode(&mut fake, 1, 600_000, requested).is_err());
        assert_eq!(fake.events, before);
    }
}

#[test]
fn ordered64_same_mode_reuses_completed_storage_and_keeps_every_signal_observation() {
    let mode = OrderedMode::Batch64;
    let mut fake = Fake {
        retained_arena: Some((mode, mode.arena_bytes())),
        ..Fake::default()
    };
    for _ in 0..2 {
        run_ordered_batch_mode(&mut fake, 64, 600_000, mode).unwrap();
        assert!(fake.completed && !fake.poisoned);
    }
    assert_eq!(fake.retained_arena, Some((mode, 4 * 1024 * 1024)));
    assert_eq!(
        fake.events
            .iter()
            .filter(|event| event.starts_with("validate_signal:"))
            .count(),
        128
    );
    assert_eq!(
        fake.events
            .iter()
            .filter(|event| event.as_str() == "doorbell")
            .count(),
        2
    );
}

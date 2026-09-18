use super::*;
use std::io::Cursor;

fn dispatches() -> Vec<OrderedBatchDispatchV1> {
    vec![
        OrderedBatchDispatchV1 {
            kernel: u64::MAX,
            payload_bytes: 8,
            workgroup: [64, 1, 1],
            grid: [64, 1, 1],
            pointers: vec![PointerFixupV1 {
                kernarg_offset: 0,
                buffer: u64::MAX,
                buffer_offset: u64::MAX,
                extent_bytes: u64::MAX,
                access: BufferAccessV1::Read,
            }],
        };
        FULL_FORWARD_DISPATCHES_V1
    ]
}

#[test]
fn full_forward_round_trip_uses_separate_plan_and_no_response_payload() {
    let dispatches = dispatches();
    let kernargs = vec![0xa5; 8 * FULL_FORWARD_DISPATCHES_V1];
    let payload = encode_full_forward_payload_v1(&dispatches, &kernargs).unwrap();
    assert!(payload.plan_bytes as usize > MAX_HEADER_BYTES_V1);
    let command = CommandV1::DispatchFullForward {
        dispatch_count: FULL_FORWARD_DISPATCHES_V1 as u32,
        plan_bytes: payload.plan_bytes,
        kernarg_bytes: payload.kernarg_bytes,
        timeout_ms: 600_000,
    };
    assert_eq!(command.payload_bytes().unwrap(), payload.bytes.len());
    let mut frame = Vec::new();
    write_header_v1(&mut frame, &command).unwrap();
    assert!(frame.len() < MAX_HEADER_BYTES_V1);
    frame.extend_from_slice(&payload.bytes);
    let mut stream = Cursor::new(frame);
    assert_eq!(
        read_header_v1::<CommandV1>(&mut stream).unwrap(),
        Some(command)
    );
    let mut received = vec![0; payload.bytes.len()];
    stream.read_exact(&mut received).unwrap();
    let (decoded, args) = decode_full_forward_payload_v1(
        FULL_FORWARD_DISPATCHES_V1 as u32,
        payload.plan_bytes,
        payload.kernarg_bytes,
        received,
    )
    .unwrap();
    assert_eq!(decoded, dispatches);
    assert_eq!(args, kernargs);
    assert_eq!(read_header_v1::<CommandV1>(&mut stream).unwrap(), None);
    let receipt = ResponseV1::DispatchFullForwardCompleted {
        completed_dispatches: 616,
        elapsed_ns: 123,
    };
    let mut frame = Vec::new();
    write_header_v1(&mut frame, &receipt).unwrap();
    let mut stream = Cursor::new(frame);
    assert_eq!(
        read_header_v1::<ResponseV1>(&mut stream).unwrap(),
        Some(receipt)
    );
    assert_eq!(read_header_v1::<ResponseV1>(&mut stream).unwrap(), None);
}

#[test]
fn full_forward_frame_bounds_precede_payload_allocation() {
    assert_eq!(MAX_HEADER_BYTES_V1, 65_536);
    assert_eq!(MAX_SEQUENCE_DISPATCHES_V1, 16);
    assert_eq!(MAX_ORDERED_BATCH_DISPATCHES_V1, 16);
    assert_eq!(MAX_TRANSFER_BYTES_V1, 4_194_304);
    assert_eq!(
        full_forward_lengths(
            616,
            MAX_FULL_FORWARD_PLAN_BYTES_V1,
            MAX_FULL_FORWARD_KERNARG_BYTES_V1
        )
        .unwrap(),
        5_242_880
    );
    for (count, plan, args, timeout) in [
        (0, 1, 0, 1),
        (16, 1, 0, 1),
        (615, 1, 0, 1),
        (617, 1, 0, 1),
        (u32::MAX, 1, 0, 1),
        (616, 0, 0, 1),
        (616, MAX_FULL_FORWARD_PLAN_BYTES_V1 + 1, 0, 1),
        (616, 1, MAX_FULL_FORWARD_KERNARG_BYTES_V1 + 1, 1),
        (616, u32::MAX, u32::MAX, 1),
        (616, 1, 0, 0),
        (616, 1, 0, 600_001),
    ] {
        assert!(
            CommandV1::DispatchFullForward {
                dispatch_count: count,
                plan_bytes: plan,
                kernarg_bytes: args,
                timeout_ms: timeout,
            }
            .payload_bytes()
            .is_err()
        );
    }
    let mut writer = BoundedPlanWriter(vec![0; MAX_FULL_FORWARD_PLAN_BYTES_V1 as usize - 1]);
    writer.write_all(&[1]).unwrap();
    assert!(writer.write_all(&[2]).is_err());
    assert_eq!(writer.0.len(), MAX_FULL_FORWARD_PLAN_BYTES_V1 as usize);
}

#[test]
fn full_forward_rejects_bad_plan_fields_counts_and_payloads() {
    let dispatches = dispatches();
    let kernargs = vec![0; 8 * FULL_FORWARD_DISPATCHES_V1];
    let good = encode_full_forward_payload_v1(&dispatches, &kernargs).unwrap();
    assert!(
        decode_full_forward_payload_v1(
            616,
            good.plan_bytes,
            good.kernarg_bytes,
            good.bytes[..good.bytes.len() - 1].to_vec()
        )
        .is_err()
    );
    let mut extra = good.bytes.clone();
    extra.push(0);
    assert!(
        decode_full_forward_payload_v1(616, good.plan_bytes, good.kernarg_bytes, extra).is_err()
    );
    let plan: serde_json::Value =
        serde_json::from_slice(&good.bytes[..good.plan_bytes as usize]).unwrap();
    for change in 0..7 {
        let mut plan = plan.clone();
        match change {
            0 => {
                plan["extra"] = true.into();
            }
            1 => {
                plan["dispatches"][0]["timeout_ms"] = 1.into();
            }
            2 => {
                plan["dispatches"][0]["pointers"][0]["address"] = 1.into();
            }
            3 => {
                plan["dispatches"].as_array_mut().unwrap().pop();
            }
            4 => {
                plan["dispatches"][0]["payload_bytes"] = 9.into();
            }
            5 => {
                plan["dispatches"][0]["payload_bytes"] = (MAX_KERNARG_BYTES_V1 + 1).into();
            }
            6 => {
                plan["dispatches"][0]["pointers"] =
                    serde_json::to_value(vec![dispatches[0].pointers[0].clone(); 257]).unwrap();
            }
            _ => unreachable!(),
        }
        let mut bytes = serde_json::to_vec(&plan).unwrap();
        let plan_bytes = bytes.len() as u32;
        bytes.extend_from_slice(&kernargs);
        assert!(
            decode_full_forward_payload_v1(616, plan_bytes, kernargs.len() as u32, bytes).is_err(),
            "change {change}"
        );
    }
    for text in [
        b"{\"dispatches\":[],\"dispatches\":[]}".as_slice(),
        b"[]",
        b"{",
    ] {
        assert!(decode_full_forward_payload_v1(616, text.len() as u32, 0, text.to_vec()).is_err());
    }
}

#[test]
fn full_forward_encoder_bounds_each_member_and_total_before_payload_copy() {
    let mut items = dispatches();
    assert!(encode_full_forward_payload_v1(&items[..615], &[]).is_err());
    assert!(encode_full_forward_payload_v1(&items, &[]).is_err());
    items[0].payload_bytes = MAX_KERNARG_BYTES_V1 + 1;
    assert!(full_forward_kernarg_bytes(&items).is_err());
    items[0].payload_bytes = 8;
    let pointer = items[0].pointers[0].clone();
    items[0].pointers.resize(257, pointer);
    assert!(full_forward_kernarg_bytes(&items).is_err());
    let mut items = dispatches();
    for item in &mut items {
        item.payload_bytes = MAX_KERNARG_BYTES_V1;
    }
    assert!(full_forward_kernarg_bytes(&items).is_err());
    let mut items = dispatches();
    for item in &mut items {
        item.payload_bytes = 0;
        item.pointers.resize(256, item.pointers[0].clone());
    }
    assert!(encode_full_forward_payload_v1(&items, &[]).is_err());
}

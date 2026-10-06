use super::*;
use std::io::Cursor;

fn definition() -> TokenProgramDefinitionV1 {
    TokenProgramDefinitionV1 {
        dispatches: vec![OrderedBatchDispatchV1 {
            kernel: 1,
            payload_bytes: 16,
            workgroup: [64, 1, 1],
            grid: [64, 1, 1],
            pointers: vec![PointerFixupV1 {
                kernarg_offset: 8,
                buffer: 1,
                buffer_offset: 0,
                extent_bytes: 32,
                access: BufferAccessV1::Read,
            }],
        }],
        slots: vec![
            TokenProgramSlotV1::ScalarU32 {
                dispatch: 0,
                offset: 0,
                minimum: 1,
                maximum: 4096,
            },
            TokenProgramSlotV1::Pointer {
                dispatch: 0,
                pointer: 0,
                buffers: vec![1, 2],
                maximum_offset: 128,
            },
        ],
    }
}

fn updates() -> Vec<TokenProgramUpdateV1> {
    vec![
        TokenProgramUpdateV1::ScalarU32 { value: 13 },
        TokenProgramUpdateV1::Pointer {
            buffer: 2,
            offset: 64,
        },
    ]
}

#[test]
fn registration_payload_round_trips_without_changing_header_framing() {
    let definition = definition();
    let kernargs = vec![0; 16];
    let (command, payload) = encode_token_program_v1(&definition, &kernargs).unwrap();
    assert_eq!(command.payload_bytes().unwrap(), payload.len());
    let mut framed = Vec::new();
    write_header_v1(&mut framed, &command).unwrap();
    framed.extend_from_slice(&payload);
    let mut cursor = Cursor::new(framed);
    assert_eq!(
        read_header_v1::<CommandV1>(&mut cursor).unwrap(),
        Some(command.clone())
    );
    let mut actual = Vec::new();
    cursor.read_to_end(&mut actual).unwrap();
    assert_eq!(actual, payload);
    let CommandV1::RegisterTokenProgram {
        definition_bytes,
        kernarg_bytes,
    } = command
    else {
        panic!()
    };
    let template = ProgramTemplate::decode(definition_bytes, kernarg_bytes, &payload).unwrap();
    assert_eq!(template.definition, definition);
    assert_eq!(template.initial(), (definition.dispatches, kernargs));
}

#[test]
fn registration_framing_is_bounded_before_payload_allocation() {
    for (definition, kernarg) in [
        (0, 0),
        (MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1 + 1, 0),
        (1, MAX_TRANSFER_BYTES_V1),
        (1, u32::MAX),
    ] {
        assert!(
            CommandV1::RegisterTokenProgram {
                definition_bytes: definition,
                kernarg_bytes: kernarg
            }
            .payload_bytes()
            .is_err()
        );
    }
    assert_eq!(
        token_program_payload_bytes(
            MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1,
            MAX_TRANSFER_BYTES_V1 - MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1
        )
        .unwrap(),
        MAX_TRANSFER_BYTES_V1 as usize
    );
    assert!(ProgramTemplate::decode(2, 0, b"{}").is_err());
    let (command, mut payload) = encode_token_program_v1(&definition(), &[0; 16]).unwrap();
    let CommandV1::RegisterTokenProgram {
        definition_bytes,
        kernarg_bytes,
    } = command
    else {
        panic!()
    };
    payload.push(0);
    assert!(ProgramTemplate::decode(definition_bytes, kernarg_bytes, &payload).is_err());
}

#[test]
fn definition_unknown_and_duplicate_fields_are_rejected() {
    let valid = serde_json::to_string(&definition()).unwrap();
    for json in [
        valid.replacen('{', "{\"extra\":1,", 1),
        valid.replacen("\"dispatches\":", "\"dispatches\":[],\"dispatches\":", 1),
        valid.replacen("\"minimum\":1", "\"minimum\":1,\"minimum\":2", 1),
        valid.replacen(
            "\"maximum_offset\":128",
            "\"maximum_offset\":128,\"extra\":1",
            1,
        ),
    ] {
        let mut bytes = json.as_bytes().to_vec();
        bytes.extend_from_slice(&[0; 16]);
        assert!(
            ProgramTemplate::decode(json.len() as u32, 16, &bytes).is_err(),
            "{json}"
        );
    }
}

#[test]
fn program_dispatch_and_slot_counts_have_closed_bounds() {
    let mut value = definition();
    value.slots.clear();
    value.dispatches = vec![value.dispatches[0].clone(); MAX_TOKEN_PROGRAM_DISPATCHES_V1];
    assert!(encode_token_program_v1(&value, &[0; 16 * MAX_TOKEN_PROGRAM_DISPATCHES_V1]).is_ok());
    value.dispatches.push(value.dispatches[0].clone());
    assert!(
        ProgramTemplate::new(value, vec![0; 16 * (MAX_TOKEN_PROGRAM_DISPATCHES_V1 + 1)]).is_err()
    );
    let mut value = definition();
    value.dispatches.clear();
    assert!(ProgramTemplate::new(value, Vec::new()).is_err());
    let mut value = definition();
    value.slots = vec![value.slots[0].clone(); MAX_TOKEN_PROGRAM_SLOTS_V1 + 1];
    assert!(ProgramTemplate::new(value, vec![0; 16]).is_err());
}

#[test]
fn slot_duplicates_indices_and_pointer_allowlists_are_rejected() {
    let valid = definition();
    let invalid = [
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 1,
            offset: 0,
            minimum: 0,
            maximum: 1,
        },
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: u32::MAX,
            minimum: 0,
            maximum: 1,
        },
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: 13,
            minimum: 0,
            maximum: 1,
        },
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: 0,
            minimum: 2,
            maximum: 1,
        },
        TokenProgramSlotV1::Pointer {
            dispatch: 0,
            pointer: 1,
            buffers: vec![1],
            maximum_offset: 0,
        },
        TokenProgramSlotV1::Pointer {
            dispatch: 0,
            pointer: 0,
            buffers: vec![],
            maximum_offset: 0,
        },
        TokenProgramSlotV1::Pointer {
            dispatch: 0,
            pointer: 0,
            buffers: vec![1, 1],
            maximum_offset: 0,
        },
        TokenProgramSlotV1::Pointer {
            dispatch: 0,
            pointer: 0,
            buffers: vec![0],
            maximum_offset: 0,
        },
        TokenProgramSlotV1::Pointer {
            dispatch: 0,
            pointer: 0,
            buffers: (1..=9).collect(),
            maximum_offset: 0,
        },
    ];
    for slot in invalid {
        let mut value = valid.clone();
        value.slots = vec![slot];
        assert!(ProgramTemplate::new(value, vec![0; 16]).is_err());
    }
    for slot in &valid.slots {
        let mut value = valid.clone();
        value.slots.push(slot.clone());
        assert!(ProgramTemplate::new(value, vec![0; 16]).is_err());
    }
    let mut overlapping = valid;
    overlapping.slots.push(TokenProgramSlotV1::ScalarU32 {
        dispatch: 0,
        offset: 2,
        minimum: 0,
        maximum: 1,
    });
    assert!(ProgramTemplate::new(overlapping, vec![0; 16]).is_err());
}

#[test]
fn exact_updates_change_only_declared_values_and_preserve_template() {
    let template = ProgramTemplate::new(definition(), vec![0; 16]).unwrap();
    let original = template.initial();
    let (commands, bytes) = template.materialize(&updates()).unwrap();
    assert_eq!(&bytes[..4], &13u32.to_le_bytes());
    assert_eq!(&bytes[4..], &[0; 12]);
    let fixup = &commands[0].pointers[0];
    assert_eq!(
        (
            fixup.buffer,
            fixup.buffer_offset,
            fixup.extent_bytes,
            fixup.access
        ),
        (2, 64, 32, BufferAccessV1::Read)
    );
    assert_eq!(commands[0].grid, original.0[0].grid);
    assert_eq!(template.initial(), original);
}

#[test]
fn bad_or_incomplete_updates_are_transactional_rejections() {
    let template = ProgramTemplate::new(definition(), vec![0; 16]).unwrap();
    let original = template.initial();
    let mut cases = vec![
        vec![],
        updates()[..1].to_vec(),
        vec![updates()[0].clone(); 3],
    ];
    for value in [0, 4097] {
        cases.push(vec![
            TokenProgramUpdateV1::ScalarU32 { value },
            updates()[1].clone(),
        ]);
    }
    cases.push(vec![
        updates()[0].clone(),
        TokenProgramUpdateV1::Pointer {
            buffer: 3,
            offset: 0,
        },
    ]);
    cases.push(vec![
        updates()[0].clone(),
        TokenProgramUpdateV1::Pointer {
            buffer: 1,
            offset: 129,
        },
    ]);
    cases.push(vec![updates()[1].clone(), updates()[0].clone()]);
    for case in cases {
        assert!(template.materialize(&case).is_err());
        assert_eq!(template.initial(), original);
    }
}

#[test]
fn execute_and_release_headers_are_exact_and_bounded() {
    for command in [
        CommandV1::ExecuteTokenProgram {
            program: 1,
            expected_epoch: 2,
            expected_completed_packets: 3,
            timeout_ms: 600_000,
            updates: updates(),
        },
        CommandV1::ReleaseTokenProgram {
            program: 1,
            expected_epoch: 2,
        },
    ] {
        assert_eq!(command.payload_bytes().unwrap(), 0);
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &command).unwrap();
        assert_eq!(
            read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).unwrap(),
            Some(command)
        );
    }
    for (timeout_ms, count) in [(0, 0), (600_001, 0), (1, MAX_TOKEN_PROGRAM_SLOTS_V1 + 1)] {
        assert!(
            CommandV1::ExecuteTokenProgram {
                program: 1,
                expected_epoch: 0,
                expected_completed_packets: 0,
                timeout_ms,
                updates: vec![TokenProgramUpdateV1::ScalarU32 { value: 1 }; count]
            }
            .payload_bytes()
            .is_err()
        );
    }
    let mut bytes = Vec::new();
    write_header_v1(&mut bytes, &serde_json::json!({"op":"release_token_program","program":1,"expected_epoch":0,"extra":true})).unwrap();
    assert!(read_header_v1::<CommandV1>(&mut Cursor::new(bytes)).is_err());
}

#[test]
fn program_responses_round_trip_and_do_not_define_partial_success() {
    for response in [
        ResponseV1::TokenProgramRegistered {
            program: 1,
            device_unique_id: 7,
            queue_epoch: 2,
            dispatches: 65,
            slots: 2,
        },
        ResponseV1::TokenProgramReleased {
            program: 1,
            queue_epoch: 2,
        },
        ResponseV1::TokenProgramCompleted {
            program: 1,
            device_unique_id: 7,
            queue_epoch: 2,
            completed_dispatches: 65,
            completed_packets: 129,
            elapsed_ns: 100,
        },
    ] {
        let mut bytes = Vec::new();
        write_header_v1(&mut bytes, &response).unwrap();
        assert_eq!(
            read_header_v1::<ResponseV1>(&mut Cursor::new(bytes)).unwrap(),
            Some(response)
        );
    }
}

fn assert_counted_encoding_equivalent(
    value: &TokenProgramDefinitionV1,
    kernargs: &[u8],
    expected: Option<&str>,
) {
    let before = (value.clone(), kernargs.to_vec());
    let owned = encode_token_program_v1(value, kernargs)
        .err()
        .map(|error| (error.kind(), error.to_string()));
    let counted = validate_token_program_encoding_v1(value, kernargs)
        .err()
        .map(|error| (error.kind(), error.to_string()));
    assert_eq!(counted, owned);
    assert_eq!(
        counted,
        expected.map(|message| (io::ErrorKind::InvalidData, message.to_string()))
    );
    assert_eq!(value, &before.0);
    assert_eq!(kernargs, before.1);
}

#[test]
fn counted_encoding_preserves_valid_empty_maximum_and_numeric_templates() {
    assert_counted_encoding_equivalent(&definition(), &[0; 16], None);
    let mut value = definition();
    value.slots.clear();
    value.dispatches[0].payload_bytes = 0;
    value.dispatches[0].pointers.clear();
    assert_counted_encoding_equivalent(&value, &[], None);
    value.dispatches = vec![value.dispatches[0].clone(); MAX_TOKEN_PROGRAM_DISPATCHES_V1];
    assert_counted_encoding_equivalent(&value, &[], None);

    let mut value = definition();
    value.slots.clear();
    value.dispatches[0].payload_bytes = MAX_KERNARG_BYTES_V1;
    value.dispatches[0].kernel = u64::MAX;
    value.dispatches[0].workgroup = [u16::MAX, 0, 1];
    value.dispatches[0].grid = [0, u32::MAX, 1];
    value.dispatches[0].pointers[0].buffer_offset = u64::MAX;
    value.dispatches[0].pointers[0].extent_bytes = u64::MAX;
    value.dispatches[0].pointers =
        vec![value.dispatches[0].pointers[0].clone(); MAX_POINTER_FIXUPS_V1];
    assert_counted_encoding_equivalent(&value, &vec![0xa5; MAX_KERNARG_BYTES_V1 as usize], None);

    let mut value = definition();
    value.dispatches[0].payload_bytes = (MAX_TOKEN_PROGRAM_SLOTS_V1 * 4) as u32;
    value.slots = (0..MAX_TOKEN_PROGRAM_SLOTS_V1)
        .map(|index| TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: (index * 4) as u32,
            minimum: 0,
            maximum: u32::MAX,
        })
        .collect();
    assert_counted_encoding_equivalent(&value, &vec![0; MAX_TOKEN_PROGRAM_SLOTS_V1 * 4], None);
}

#[test]
fn counted_encoding_preserves_structural_rejection_order_and_messages() {
    let mut value = definition();
    value.dispatches.clear();
    assert_counted_encoding_equivalent(&value, &[], Some("token program count limits"));
    let mut value = definition();
    value.dispatches = vec![value.dispatches[0].clone(); MAX_TOKEN_PROGRAM_DISPATCHES_V1 + 1];
    assert_counted_encoding_equivalent(&value, &[], Some("token program count limits"));
    let mut value = definition();
    value.slots = vec![value.slots[0].clone(); MAX_TOKEN_PROGRAM_SLOTS_V1 + 1];
    assert_counted_encoding_equivalent(&value, &[0; 16], Some("token program count limits"));
    assert_counted_encoding_equivalent(
        &definition(),
        &vec![0; MAX_TRANSFER_BYTES_V1 as usize + 1],
        Some("token program count limits"),
    );
    let mut value = definition();
    value.dispatches[0].payload_bytes = MAX_KERNARG_BYTES_V1 + 1;
    assert_counted_encoding_equivalent(&value, &[], Some("token program dispatch limits"));
    let mut value = definition();
    value.dispatches[0].pointers =
        vec![value.dispatches[0].pointers[0].clone(); MAX_POINTER_FIXUPS_V1 + 1];
    assert_counted_encoding_equivalent(&value, &[], Some("token program dispatch limits"));
    let mut value = definition();
    value.dispatches[0].payload_bytes = MAX_KERNARG_BYTES_V1;
    value.dispatches = vec![value.dispatches[0].clone(); 65];
    assert_counted_encoding_equivalent(&value, &[], Some("token program payload overflow"));
    for size in [15, 17] {
        assert_counted_encoding_equivalent(
            &definition(),
            &vec![0; size],
            Some("token program kernarg length"),
        );
    }
    for (slot, message) in [
        (
            TokenProgramSlotV1::ScalarU32 {
                dispatch: u16::MAX,
                offset: 0,
                minimum: 0,
                maximum: 1,
            },
            "token program dispatch index",
        ),
        (
            TokenProgramSlotV1::ScalarU32 {
                dispatch: 0,
                offset: u32::MAX,
                minimum: 0,
                maximum: 1,
            },
            "token program scalar extent",
        ),
        (
            TokenProgramSlotV1::ScalarU32 {
                dispatch: 0,
                offset: 13,
                minimum: 0,
                maximum: 1,
            },
            "token program scalar extent",
        ),
        (
            TokenProgramSlotV1::ScalarU32 {
                dispatch: 0,
                offset: 0,
                minimum: 2,
                maximum: 1,
            },
            "token program scalar slot",
        ),
        (
            TokenProgramSlotV1::Pointer {
                dispatch: u16::MAX,
                pointer: 0,
                buffers: vec![1],
                maximum_offset: 0,
            },
            "token program dispatch index",
        ),
        (
            TokenProgramSlotV1::Pointer {
                dispatch: 0,
                pointer: 1,
                buffers: vec![1],
                maximum_offset: 0,
            },
            "token program pointer slot",
        ),
    ] {
        let mut value = definition();
        value.slots = vec![slot];
        assert_counted_encoding_equivalent(&value, &[0; 16], Some(message));
    }
    for buffers in [vec![], vec![0], vec![1, 1], (1..=9).collect()] {
        let mut value = definition();
        value.slots = vec![TokenProgramSlotV1::Pointer {
            dispatch: 0,
            pointer: 0,
            buffers,
            maximum_offset: 0,
        }];
        assert_counted_encoding_equivalent(&value, &[0; 16], Some("token program pointer slot"));
    }
    for (index, message) in [
        (0, "token program scalar slot"),
        (1, "token program pointer slot"),
    ] {
        let mut value = definition();
        value.slots.push(value.slots[index].clone());
        assert_counted_encoding_equivalent(&value, &[0; 16], Some(message));
    }
    let mut value = definition();
    value.slots.push(TokenProgramSlotV1::ScalarU32 {
        dispatch: 0,
        offset: 2,
        minimum: 0,
        maximum: 1,
    });
    assert_counted_encoding_equivalent(&value, &[0; 16], Some("token program scalar slot"));
}

pub(super) fn definition_at_json_length(target: usize) -> TokenProgramDefinitionV1 {
    let mut value = definition();
    value.slots.clear();
    value.dispatches[0].payload_bytes = 0;
    value.dispatches[0].pointers.clear();
    let empty = value.dispatches[0].clone();
    let pointer = PointerFixupV1 {
        kernarg_offset: 0,
        buffer: 1,
        buffer_offset: 1,
        extent_bytes: 1,
        access: BufferAccessV1::Read,
    };
    let pointer_bytes = serde_json::to_vec(&pointer).unwrap().len();
    let dispatch_bytes = serde_json::to_vec(&empty).unwrap().len();
    let mut bytes = serde_json::to_vec(&value).unwrap().len();
    loop {
        let last = value.dispatches.last_mut().unwrap();
        if last.pointers.len() == MAX_POINTER_FIXUPS_V1 {
            if bytes + dispatch_bytes + 1 > target {
                break;
            }
            value.dispatches.push(empty.clone());
            bytes += dispatch_bytes + 1;
        } else {
            let extra = pointer_bytes + usize::from(!last.pointers.is_empty());
            if bytes + extra > target {
                break;
            }
            last.pointers.push(pointer.clone());
            bytes += extra;
        }
    }
    let mut remaining = target - bytes;
    for dispatch in &mut value.dispatches {
        for pointer in &mut dispatch.pointers {
            for field in [
                &mut pointer.buffer,
                &mut pointer.buffer_offset,
                &mut pointer.extent_bytes,
            ] {
                let digits = remaining.min(19);
                *field = 10u64.pow(digits as u32);
                remaining -= digits;
            }
        }
    }
    assert_eq!(remaining, 0);
    assert!(value.dispatches.len() <= MAX_TOKEN_PROGRAM_DISPATCHES_V1);
    assert_eq!(serde_json::to_vec(&value).unwrap().len(), target);
    value
}

#[test]
fn counted_encoding_preserves_exact_serialized_definition_limit() {
    let limit = MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1 as usize;
    for bytes in [limit - 1, limit, limit + 1] {
        let value = definition_at_json_length(bytes);
        assert_counted_encoding_equivalent(
            &value,
            &[],
            (bytes > limit).then_some("token program definition limit"),
        );
        let mut count = JsonByteCount::default();
        serde_json::to_writer(&mut count, &value).unwrap();
        assert_eq!(count.bytes, bytes);
    }
}

#[test]
fn counted_encoding_preserves_exact_combined_transfer_limit() {
    let mut value = definition();
    value.slots.clear();
    value.dispatches[0].pointers.clear();
    value.dispatches[0].payload_bytes = MAX_KERNARG_BYTES_V1;
    value.dispatches = vec![value.dispatches[0].clone(); 64];
    let definition_bytes = serde_json::to_vec(&value).unwrap().len();
    for total in [
        MAX_TRANSFER_BYTES_V1 - 1,
        MAX_TRANSFER_BYTES_V1,
        MAX_TRANSFER_BYTES_V1 + 1,
    ] {
        let payload_bytes = total as usize - definition_bytes;
        value.dispatches.last_mut().unwrap().payload_bytes =
            (payload_bytes - 63 * MAX_KERNARG_BYTES_V1 as usize) as u32;
        assert_eq!(serde_json::to_vec(&value).unwrap().len(), definition_bytes);
        assert_counted_encoding_equivalent(
            &value,
            &vec![0; payload_bytes],
            (total > MAX_TRANSFER_BYTES_V1).then_some("token program transfer limit"),
        );
    }
}

#[test]
fn counted_encoding_keeps_definition_limit_before_transfer_limit() {
    let mut value = definition_at_json_length(MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1 as usize + 1);
    let mut empty = value.dispatches[0].clone();
    empty.pointers.clear();
    value.dispatches.resize(64, empty);
    for dispatch in &mut value.dispatches {
        dispatch.payload_bytes = MAX_KERNARG_BYTES_V1;
    }
    assert!(
        serde_json::to_vec(&value).unwrap().len() > MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1 as usize
    );
    assert_counted_encoding_equivalent(
        &value,
        &vec![0; MAX_TRANSFER_BYTES_V1 as usize],
        Some("token program definition limit"),
    );
}

#[test]
fn counted_json_writer_counts_exact_bytes_and_propagates_overflow() {
    let value = definition();
    let expected = serde_json::to_vec(&value).unwrap();
    let mut count = JsonByteCount::default();
    serde_json::to_writer(&mut count, &value).unwrap();
    assert_eq!(count.bytes, expected.len());
    count.flush().unwrap();
    let mut full = JsonByteCount { bytes: usize::MAX };
    assert!(serde_json::to_writer(&mut full, &value).is_err());
    assert_eq!(full.bytes, usize::MAX);
}

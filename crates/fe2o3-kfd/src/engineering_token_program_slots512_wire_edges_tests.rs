use super::*;
use std::io::Cursor;

fn dispatch(payload_bytes: u32) -> OrderedBatchDispatchV1 {
    OrderedBatchDispatchV1 {
        kernel: 1,
        payload_bytes,
        workgroup: [64, 1, 1],
        grid: [64, 1, 1],
        pointers: Vec::new(),
    }
}

// This exercises the real command/slot cardinality, not Ferric's kernel ABI.
fn two_page_shape() -> (TokenProgramDefinitionV1, Vec<u8>, Vec<TokenProgramUpdateV1>) {
    let mut definition = TokenProgramDefinitionV1 {
        dispatches: vec![dispatch(32); 649],
        slots: Vec::new(),
    };
    let mut updates = Vec::new();
    for layer in 0..36usize {
        let base = 1 + layer * 18;
        for (copy, page) in [(0usize, 7u32), (1, 3)] {
            let index = base + 7 + copy;
            let command = &mut definition.dispatches[index];
            for pointer in 0..2u16 {
                let buffer = u64::try_from(layer * 2).unwrap() + u64::from(pointer) + 1;
                command.pointers.push(PointerFixupV1 {
                    kernarg_offset: u32::from(pointer) * 8,
                    buffer,
                    buffer_offset: 0,
                    extent_bytes: 32_768,
                    access: BufferAccessV1::Write,
                });
                definition.slots.push(TokenProgramSlotV1::Pointer {
                    dispatch: u16::try_from(index).unwrap(),
                    pointer,
                    buffers: vec![buffer],
                    maximum_offset: 511 * 32_768,
                });
                updates.push(TokenProgramUpdateV1::Pointer {
                    buffer,
                    offset: u64::from(page) * 32_768,
                });
            }
            for (offset, minimum, maximum, value) in [
                (16, 0, 8160, 96),
                (20, 0, 511, page),
                (24, 0, 1, u32::try_from(1 - copy).unwrap()),
            ] {
                definition.slots.push(TokenProgramSlotV1::ScalarU32 {
                    dispatch: u16::try_from(index).unwrap(),
                    offset,
                    minimum,
                    maximum,
                });
                updates.push(TokenProgramUpdateV1::ScalarU32 { value });
            }
        }
        definition.slots.push(TokenProgramSlotV1::ScalarU32 {
            dispatch: u16::try_from(base + 9).unwrap(),
            offset: 0,
            minimum: 32,
            maximum: 8192,
        });
        updates.push(TokenProgramUpdateV1::ScalarU32 { value: 128 });
    }
    (definition, vec![0xa5; 649 * 32], updates)
}

#[test]
fn synthetic_649_dispatch_396_slot_frame_round_trips_and_materializes_all_layers() {
    let (definition, kernargs, updates) = two_page_shape();
    assert_eq!(
        (
            definition.dispatches.len(),
            definition.slots.len(),
            updates.len()
        ),
        (649, 396, 396)
    );
    assert!(encode_token_program_v1(&definition, &kernargs).is_err());
    assert!(validate_token_program_encoding_v1(&definition, &kernargs).is_err());
    validate_token_program_slots512_encoding_v1(&definition, &kernargs).unwrap();
    let (command, payload) = encode_token_program_slots512_v1(&definition, &kernargs).unwrap();
    let CommandV1::RegisterTokenProgramSlots512V1 {
        definition_bytes,
        kernarg_bytes,
    } = command.clone()
    else {
        panic!()
    };
    assert_eq!(
        usize::try_from(definition_bytes).unwrap(),
        serde_json::to_vec(&definition).unwrap().len()
    );
    assert_eq!(usize::try_from(kernarg_bytes).unwrap(), 649 * 32);
    assert_eq!(command.payload_bytes().unwrap(), payload.len());
    let mut framed = Vec::new();
    write_header_v1(&mut framed, &command).unwrap();
    framed.extend_from_slice(&payload);
    let mut cursor = Cursor::new(framed);
    assert_eq!(
        read_header_v1::<CommandV1>(&mut cursor).unwrap(),
        Some(command)
    );
    let mut decoded_payload = Vec::new();
    cursor.read_to_end(&mut decoded_payload).unwrap();
    assert_eq!(decoded_payload, payload);
    let template = ProgramTemplate::decode_with_policy(
        definition_bytes,
        kernarg_bytes,
        &decoded_payload,
        TokenProgramSlotPolicyV1::Extended512,
    )
    .unwrap();
    let original = template.initial();
    assert!(ProgramTemplate::decode(definition_bytes, kernarg_bytes, &decoded_payload).is_err());
    let (actual_commands, actual_bytes) = template.materialize(&updates).unwrap();
    let mut expected_commands = definition.dispatches.clone();
    let mut expected_bytes = kernargs.clone();
    for layer in 0..36usize {
        let base = 1 + layer * 18;
        for (copy, page) in [(0usize, 7u32), (1, 3)] {
            let index = base + 7 + copy;
            for pointer in &mut expected_commands[index].pointers {
                pointer.buffer_offset = u64::from(page) * 32_768;
            }
            for (offset, value) in [
                (16, 96u32),
                (20, page),
                (24, u32::try_from(1 - copy).unwrap()),
            ] {
                let start = index * 32 + offset;
                expected_bytes[start..start + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        let start = (base + 9) * 32;
        expected_bytes[start..start + 4].copy_from_slice(&128u32.to_le_bytes());
    }
    assert_eq!(
        (actual_commands, actual_bytes),
        (expected_commands, expected_bytes)
    );
    assert_eq!(template.initial(), (definition.dispatches, kernargs));
    let mut invalid = updates;
    invalid[395] = TokenProgramUpdateV1::ScalarU32 { value: 8193 };
    assert!(template.materialize(&invalid).is_err());
    assert_eq!(template.initial(), original);
}

#[test]
fn worst_case_512_pointer_execute_header_has_exact_serialized_size() {
    let update = TokenProgramUpdateV1::Pointer {
        buffer: u64::MAX,
        offset: u64::MAX,
    };
    let command = CommandV1::ExecuteTokenProgramSlots512V1 {
        program: u64::MAX,
        expected_epoch: u64::MAX,
        expected_completed_packets: u64::MAX,
        timeout_ms: 600_000,
        updates: vec![update; 512],
    };
    let pointer =
        "{\"kind\":\"pointer\",\"buffer\":18446744073709551615,\"offset\":18446744073709551615}";
    assert_eq!(pointer.len(), 78);
    let expected = format!(
        "{{\"op\":\"execute_token_program_slots512_v1\",\"program\":18446744073709551615,\"expected_epoch\":18446744073709551615,\"expected_completed_packets\":18446744073709551615,\"timeout_ms\":600000,\"updates\":[{}]}}",
        vec![pointer; 512].join(","),
    );
    let actual = serde_json::to_vec(&command).unwrap();
    assert_eq!(actual, expected.as_bytes());
    assert_eq!(actual.len(), 40_641);
    assert!(actual.len() < MAX_HEADER_BYTES_V1);
    assert_eq!(command.payload_bytes().unwrap(), 0);
    let mut framed = Vec::new();
    write_header_v1(&mut framed, &command).unwrap();
    assert_eq!(&framed[..4], &40_641u32.to_le_bytes());
    assert_eq!(framed.len(), 40_645);
    assert_eq!(
        read_header_v1::<CommandV1>(&mut Cursor::new(framed)).unwrap(),
        Some(command)
    );
}

#[test]
fn exact_existing_header_ceiling_is_enforced_before_reading_body() {
    let command = CommandV1::Close;
    let mut json = serde_json::to_vec(&command).unwrap();
    json.resize(MAX_HEADER_BYTES_V1, b' ');
    let mut framed = u32::try_from(json.len()).unwrap().to_le_bytes().to_vec();
    framed.extend_from_slice(&json);
    assert_eq!(
        read_header_v1::<CommandV1>(&mut Cursor::new(framed)).unwrap(),
        Some(command)
    );
    for length in [
        0u32,
        u32::try_from(MAX_HEADER_BYTES_V1 + 1).unwrap(),
        u32::MAX,
    ] {
        let mut framed = length.to_le_bytes().to_vec();
        framed.extend_from_slice(b"body must remain unread");
        let mut cursor = Cursor::new(framed);
        assert!(read_header_v1::<CommandV1>(&mut cursor).is_err());
        assert_eq!(cursor.position(), 4);
    }
}

#[test]
fn raw_malformed_extended_discriminants_and_duplicate_tags_are_rejected() {
    for json in [
        r#"{"op":"register_token_program_slots_512_v1","definition_bytes":1,"kernarg_bytes":0}"#,
        r#"{"op":"RegisterTokenProgramSlots512V1","definition_bytes":1,"kernarg_bytes":0}"#,
        r#"{"op":"register_token_program_slots512_v2","definition_bytes":1,"kernarg_bytes":0}"#,
        r#"{"op":"execute_token_program_slots_512_v1","program":1,"expected_epoch":1,"expected_completed_packets":0,"timeout_ms":1,"updates":[]}"#,
        r#"{"op":"register_token_program_slots512_v1","op":"register_token_program","definition_bytes":1,"kernarg_bytes":0}"#,
        r#"{"op":"register_token_program_slots512_v1","definition_bytes":1,"kernarg_bytes":0,"slot_policy":"extended512"}"#,
    ] {
        let mut frame = u32::try_from(json.len()).unwrap().to_le_bytes().to_vec();
        frame.extend_from_slice(json.as_bytes());
        assert!(
            read_header_v1::<CommandV1>(&mut Cursor::new(frame)).is_err(),
            "{json}"
        );
    }
}

#[test]
fn legacy_registration_execution_and_release_bytes_remain_exact() {
    let cases = [
        (
            CommandV1::RegisterTokenProgram {
                definition_bytes: 1234,
                kernarg_bytes: 5678,
            },
            r#"{"op":"register_token_program","definition_bytes":1234,"kernarg_bytes":5678}"#,
        ),
        (
            CommandV1::ExecuteTokenProgram {
                program: 17,
                expected_epoch: 2,
                expected_completed_packets: 652,
                timeout_ms: 15_000,
                updates: vec![
                    TokenProgramUpdateV1::ScalarU32 { value: 128 },
                    TokenProgramUpdateV1::Pointer {
                        buffer: 9,
                        offset: 32_768,
                    },
                ],
            },
            r#"{"op":"execute_token_program","program":17,"expected_epoch":2,"expected_completed_packets":652,"timeout_ms":15000,"updates":[{"kind":"scalar_u32","value":128},{"kind":"pointer","buffer":9,"offset":32768}]}"#,
        ),
        (
            CommandV1::ReleaseTokenProgram {
                program: 17,
                expected_epoch: 2,
            },
            r#"{"op":"release_token_program","program":17,"expected_epoch":2}"#,
        ),
    ];
    for (command, json) in cases {
        let mut expected = u32::try_from(json.len()).unwrap().to_le_bytes().to_vec();
        expected.extend_from_slice(json.as_bytes());
        let mut actual = Vec::new();
        write_header_v1(&mut actual, &command).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            read_header_v1::<CommandV1>(&mut Cursor::new(actual)).unwrap(),
            Some(command)
        );
    }
}

#[test]
fn extended_slots_do_not_widen_per_dispatch_pointer_fixups() {
    let mut definition = TokenProgramDefinitionV1 {
        dispatches: vec![dispatch(2048); 2],
        slots: Vec::new(),
    };
    for index in 0..2u16 {
        for pointer in 0..256u16 {
            definition.dispatches[usize::from(index)]
                .pointers
                .push(PointerFixupV1 {
                    kernarg_offset: u32::from(pointer) * 8,
                    buffer: u64::from(pointer) + 1,
                    buffer_offset: 0,
                    extent_bytes: 8,
                    access: BufferAccessV1::Read,
                });
            definition.slots.push(TokenProgramSlotV1::Pointer {
                dispatch: index,
                pointer,
                buffers: vec![u64::from(pointer) + 1],
                maximum_offset: 0,
            });
        }
    }
    let bytes = vec![0; 4096];
    assert_eq!(definition.slots.len(), 512);
    validate_token_program_slots512_encoding_v1(&definition, &bytes).unwrap();
    encode_token_program_slots512_v1(&definition, &bytes).unwrap();
    let extra = definition.dispatches[1].pointers[0].clone();
    definition.dispatches[1].pointers.push(extra);
    assert!(validate_token_program_slots512_encoding_v1(&definition, &bytes).is_err());
    assert!(encode_token_program_slots512_v1(&definition, &bytes).is_err());
    definition.slots.clear();
    assert!(validate_token_program_encoding_v1(&definition, &bytes).is_err());
    assert!(encode_token_program_v1(&definition, &bytes).is_err());
    definition.dispatches[1].pointers.pop();
    validate_token_program_encoding_v1(&definition, &bytes).unwrap();
    encode_token_program_v1(&definition, &bytes).unwrap();
}

fn exact_transfer_definition(delta: i32) -> (TokenProgramDefinitionV1, Vec<u8>) {
    let mut definition = TokenProgramDefinitionV1 {
        dispatches: vec![dispatch(MAX_KERNARG_BYTES_V1); 64],
        slots: Vec::new(),
    };
    let original_json_bytes = serde_json::to_vec(&definition).unwrap().len();
    let last = i64::from(MAX_KERNARG_BYTES_V1) - i64::try_from(original_json_bytes).unwrap()
        + i64::from(delta);
    definition.dispatches[63].payload_bytes = u32::try_from(last).unwrap();
    assert_eq!(
        serde_json::to_vec(&definition).unwrap().len(),
        original_json_bytes
    );
    let bytes = definition
        .dispatches
        .iter()
        .map(|entry| usize::try_from(entry.payload_bytes).unwrap())
        .sum::<usize>();
    (definition, vec![0; bytes])
}

#[test]
fn actual_encoded_definition_and_kernargs_share_exact_four_mib_ceiling() {
    for delta in [-1, 0, 1] {
        let (definition, kernargs) = exact_transfer_definition(delta);
        let json_bytes = serde_json::to_vec(&definition).unwrap().len();
        let expected_total = i64::from(MAX_TRANSFER_BYTES_V1) + i64::from(delta);
        assert_eq!(
            i64::try_from(json_bytes + kernargs.len()).unwrap(),
            expected_total
        );
        assert!(json_bytes < usize::try_from(MAX_TOKEN_PROGRAM_DEFINITION_BYTES_V1).unwrap());
        assert_eq!(
            validate_token_program_encoding_v1(&definition, &kernargs).is_ok(),
            delta <= 0
        );
        assert_eq!(
            validate_token_program_slots512_encoding_v1(&definition, &kernargs).is_ok(),
            delta <= 0
        );
        let legacy = encode_token_program_v1(&definition, &kernargs);
        let extended = encode_token_program_slots512_v1(&definition, &kernargs);
        if delta > 0 {
            assert_eq!(
                legacy.unwrap_err().to_string(),
                extended.unwrap_err().to_string()
            );
            continue;
        }
        let (_, legacy_payload) = legacy.unwrap();
        let (command, payload) = extended.unwrap();
        assert_eq!(legacy_payload, payload);
        assert_eq!(
            command.payload_bytes().unwrap(),
            usize::try_from(expected_total).unwrap()
        );
        let CommandV1::RegisterTokenProgramSlots512V1 {
            definition_bytes,
            kernarg_bytes,
        } = command
        else {
            panic!()
        };
        let template = ProgramTemplate::decode_with_policy(
            definition_bytes,
            kernarg_bytes,
            &payload,
            TokenProgramSlotPolicyV1::Extended512,
        )
        .unwrap();
        assert_eq!(template.initial(), (definition.dispatches, kernargs));
        let mut extra = payload.clone();
        extra.push(0);
        assert!(
            ProgramTemplate::decode_with_policy(
                definition_bytes,
                kernarg_bytes,
                &extra,
                TokenProgramSlotPolicyV1::Extended512,
            )
            .is_err()
        );
        assert!(
            ProgramTemplate::decode_with_policy(
                definition_bytes,
                kernarg_bytes,
                &payload[..payload.len() - 1],
                TokenProgramSlotPolicyV1::Extended512,
            )
            .is_err()
        );
    }
}

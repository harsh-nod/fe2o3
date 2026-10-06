use super::*;
use std::io::Cursor;

fn scalar_definition(count: usize) -> (TokenProgramDefinitionV1, Vec<u8>) {
    let definition = TokenProgramDefinitionV1 {
        dispatches: vec![OrderedBatchDispatchV1 {
            kernel: 1,
            payload_bytes: u32::try_from(count * 4).unwrap(),
            workgroup: [64, 1, 1],
            grid: [64, 1, 1],
            pointers: Vec::new(),
        }],
        slots: (0..count)
            .map(|index| TokenProgramSlotV1::ScalarU32 {
                dispatch: 0,
                offset: u32::try_from(index * 4).unwrap(),
                minimum: 1,
                maximum: 4096,
            })
            .collect(),
    };
    (definition, vec![0; count * 4])
}

fn execute(count: usize) -> CommandV1 {
    CommandV1::ExecuteTokenProgramSlots512V1 {
        program: u64::MAX,
        expected_epoch: u64::MAX,
        expected_completed_packets: u64::MAX,
        timeout_ms: 600_000,
        updates: vec![
            TokenProgramUpdateV1::Pointer {
                buffer: u64::MAX,
                offset: u64::MAX
            };
            count
        ],
    }
}

#[test]
fn closed_slot_limits_leave_legacy_helpers_unchanged() {
    assert_eq!(TokenProgramSlotPolicyV1::Standard256.max_slots(), 256);
    assert_eq!(TokenProgramSlotPolicyV1::Extended512.max_slots(), 512);
    for count in [0, 256, 257, 396, 512, 513] {
        let (definition, bytes) = scalar_definition(count);
        assert_eq!(
            encode_token_program_v1(&definition, &bytes).is_ok(),
            count <= 256
        );
        assert_eq!(
            validate_token_program_encoding_v1(&definition, &bytes).is_ok(),
            count <= 256
        );
        assert_eq!(
            encode_token_program_slots512_v1(&definition, &bytes).is_ok(),
            count <= 512
        );
        assert_eq!(
            validate_token_program_slots512_encoding_v1(&definition, &bytes).is_ok(),
            count <= 512
        );
    }
}

#[test]
fn extended_registration_round_trip_does_not_bypass_legacy_decode() {
    let (definition, bytes) = scalar_definition(512);
    let (command, payload) = encode_token_program_slots512_v1(&definition, &bytes).unwrap();
    assert_eq!(command.payload_bytes().unwrap(), payload.len());
    let mut frame = Vec::new();
    write_header_v1(&mut frame, &command).unwrap();
    assert_eq!(
        read_header_v1::<CommandV1>(&mut Cursor::new(frame)).unwrap(),
        Some(command.clone())
    );
    let CommandV1::RegisterTokenProgramSlots512V1 {
        definition_bytes,
        kernarg_bytes,
    } = command
    else {
        panic!()
    };
    assert!(ProgramTemplate::decode(definition_bytes, kernarg_bytes, &payload).is_err());
    let template = ProgramTemplate::decode_with_policy(
        definition_bytes,
        kernarg_bytes,
        &payload,
        TokenProgramSlotPolicyV1::Extended512,
    )
    .unwrap();
    assert_eq!(template.definition, definition);
    assert_eq!(template.initial().1, bytes);
    assert!(
        ProgramTemplate::decode_with_policy(
            definition_bytes,
            kernarg_bytes,
            &payload[..payload.len() - 1],
            TokenProgramSlotPolicyV1::Extended512
        )
        .is_err()
    );
}

#[test]
fn extended_last_scalar_and_exact_update_count_remain_checked() {
    let (definition, bytes) = scalar_definition(512);
    let template =
        ProgramTemplate::new_with_policy(definition, bytes, TokenProgramSlotPolicyV1::Extended512)
            .unwrap();
    let updates = vec![TokenProgramUpdateV1::ScalarU32 { value: 4096 }; 512];
    let (_, payload) = template.materialize(&updates).unwrap();
    assert_eq!(&payload[2044..], &4096u32.to_le_bytes());
    assert!(template.materialize(&updates[..511]).is_err());
    let mut invalid = updates.clone();
    invalid.push(updates[0].clone());
    assert!(template.materialize(&invalid).is_err());
    for update in [
        TokenProgramUpdateV1::ScalarU32 { value: 0 },
        TokenProgramUpdateV1::ScalarU32 { value: 4097 },
        TokenProgramUpdateV1::Pointer {
            buffer: 1,
            offset: 0,
        },
    ] {
        let mut invalid = updates.clone();
        invalid[511] = update;
        assert!(template.materialize(&invalid).is_err());
    }
    assert_eq!(template.initial().1, vec![0; 2048]);
}

#[test]
fn extended_definition_rejects_duplicate_overlap_and_last_invalid_slot() {
    let (valid, bytes) = scalar_definition(512);
    for slot in [
        valid.slots[0].clone(),
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: 1,
            minimum: 1,
            maximum: 9,
        },
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 1,
            offset: 0,
            minimum: 1,
            maximum: 9,
        },
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: 2048,
            minimum: 1,
            maximum: 9,
        },
        TokenProgramSlotV1::ScalarU32 {
            dispatch: 0,
            offset: 2044,
            minimum: 9,
            maximum: 1,
        },
    ] {
        let mut definition = valid.clone();
        definition.slots[511] = slot;
        let encoded = encode_token_program_slots512_v1(&definition, &bytes).unwrap_err();
        let counted = validate_token_program_slots512_encoding_v1(&definition, &bytes).unwrap_err();
        assert_eq!(encoded.to_string(), counted.to_string());
    }
}

#[test]
fn extended_pointer_allowlist_and_last_update_are_not_widened() {
    let mut definition = TokenProgramDefinitionV1 {
        dispatches: Vec::new(),
        slots: Vec::new(),
    };
    for index in 0..512u16 {
        definition.dispatches.push(OrderedBatchDispatchV1 {
            kernel: 1,
            payload_bytes: 8,
            workgroup: [64, 1, 1],
            grid: [64, 1, 1],
            pointers: vec![PointerFixupV1 {
                kernarg_offset: 0,
                buffer: 1,
                buffer_offset: 0,
                extent_bytes: 8,
                access: BufferAccessV1::Read,
            }],
        });
        definition.slots.push(TokenProgramSlotV1::Pointer {
            dispatch: index,
            pointer: 0,
            buffers: vec![1, 2],
            maximum_offset: 16,
        });
    }
    let template = ProgramTemplate::new_with_policy(
        definition.clone(),
        vec![0; 4096],
        TokenProgramSlotPolicyV1::Extended512,
    )
    .unwrap();
    let mut updates = vec![
        TokenProgramUpdateV1::Pointer {
            buffer: 2,
            offset: 16
        };
        512
    ];
    assert_eq!(
        template.materialize(&updates).unwrap().0[511].pointers[0].buffer,
        2
    );
    for update in [
        TokenProgramUpdateV1::Pointer {
            buffer: 3,
            offset: 16,
        },
        TokenProgramUpdateV1::Pointer {
            buffer: 2,
            offset: 17,
        },
        TokenProgramUpdateV1::ScalarU32 { value: 1 },
    ] {
        updates[511] = update;
        assert!(template.materialize(&updates).is_err());
    }
    for buffers in [vec![], vec![0], vec![1, 1], (1..=9).collect()] {
        let mut invalid = definition.clone();
        let TokenProgramSlotV1::Pointer {
            buffers: allowed, ..
        } = &mut invalid.slots[511]
        else {
            panic!()
        };
        *allowed = buffers;
        assert!(
            ProgramTemplate::new_with_policy(
                invalid,
                vec![0; 4096],
                TokenProgramSlotPolicyV1::Extended512
            )
            .is_err()
        );
    }
}

#[test]
fn extended_execute_uses_existing_bounded_header_and_separate_schema() {
    let command = execute(512);
    assert_eq!(command.payload_bytes().unwrap(), 0);
    let json = serde_json::to_vec(&command).unwrap();
    assert!(json.len() < MAX_HEADER_BYTES_V1);
    assert!(json.len() < 41_000);
    let mut frame = Vec::new();
    write_header_v1(&mut frame, &command).unwrap();
    assert_eq!(
        read_header_v1::<CommandV1>(&mut Cursor::new(frame)).unwrap(),
        Some(command.clone())
    );
    assert!(execute(513).payload_bytes().is_err());
    let mut value = serde_json::to_value(&command).unwrap();
    value["op"] = "execute_token_program".into();
    let legacy: CommandV1 = serde_json::from_value(value).unwrap();
    assert!(legacy.payload_bytes().is_err());
    for timeout in [0, 600_001, u32::MAX] {
        let mut invalid = command.clone();
        let CommandV1::ExecuteTokenProgramSlots512V1 { timeout_ms, .. } = &mut invalid else {
            panic!()
        };
        *timeout_ms = timeout;
        assert!(invalid.payload_bytes().is_err());
    }
    let mut value = serde_json::to_value(command).unwrap();
    value["extra"] = true.into();
    assert!(serde_json::from_value::<CommandV1>(value).is_err());
    assert!(write_header_v1(&mut Vec::new(), &"x".repeat(MAX_HEADER_BYTES_V1)).is_err());
}

#[test]
fn extended_payload_framing_preserves_definition_and_transfer_ceilings() {
    for (definition_bytes, kernarg_bytes, accepted) in [
        (0, 0, false),
        (1, 0, true),
        (512 * 1024, 4 * 1024 * 1024 - 512 * 1024, true),
        (512 * 1024 + 1, 0, false),
        (1, 4 * 1024 * 1024, false),
        (1, u32::MAX, false),
    ] {
        assert_eq!(
            CommandV1::RegisterTokenProgramSlots512V1 {
                definition_bytes,
                kernarg_bytes
            }
            .payload_bytes()
            .is_ok(),
            accepted
        );
    }
    for bytes in [512 * 1024 - 1, 512 * 1024, 512 * 1024 + 1] {
        let definition = super::tests::definition_at_json_length(bytes);
        assert_eq!(
            encode_token_program_slots512_v1(&definition, &[]).is_ok(),
            bytes <= 512 * 1024
        );
        assert_eq!(
            validate_token_program_slots512_encoding_v1(&definition, &[]).is_ok(),
            bytes <= 512 * 1024
        );
    }
}

#[test]
fn extended_template_still_rejects_unknown_json_and_dispatch_limits() {
    let (mut definition, bytes) = scalar_definition(396);
    let mut value = serde_json::to_value(&definition).unwrap();
    value["slots"][395]["extra"] = true.into();
    let mut payload = serde_json::to_vec(&value).unwrap();
    let definition_bytes = u32::try_from(payload.len()).unwrap();
    payload.extend_from_slice(&bytes);
    assert!(
        ProgramTemplate::decode_with_policy(
            definition_bytes,
            u32::try_from(bytes.len()).unwrap(),
            &payload,
            TokenProgramSlotPolicyV1::Extended512
        )
        .is_err()
    );
    definition.dispatches = vec![definition.dispatches[0].clone(); 1025];
    assert!(encode_token_program_slots512_v1(&definition, &bytes).is_err());
    let (mut definition, bytes) = scalar_definition(396);
    definition.dispatches[0].payload_bytes = MAX_KERNARG_BYTES_V1 + 1;
    assert!(encode_token_program_slots512_v1(&definition, &bytes).is_err());
}

//! Synthetic fixed-payload controls only; no source/native/verified-owner claim.
use super::super::{CanonicalKernelIrWorkBudgetV1, DecodeBudgetV12, WriterModeV1};
use super::*;
use crate::{
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramDestinationV1 as Destination,
    Gfx942ProgramRoleV1 as Role,
};

fn declaration() -> Declaration {
    Declaration {
        origin: Origin {
            root_axes: [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]],
            mir_body: [6; 32],
            semantic_block: [7; 32],
            source_signature: [8; 32],
            rustc_fn_abi: [9; 32],
            frontend_bytes_sha256: [10; 32],
            raw_block: 0x0102_0304,
        },
        registers: Gfx942OrderedProgramRegistersV1::new(32, 33, [34, 35, 36]).unwrap(),
        parameters: [
            ValueId(0),
            ValueId(1),
            ValueId(2),
            ValueId(3),
            ValueId(u32::MAX),
        ],
        labels: [0, 7, 255, 9, 0, 0, 0, 0],
        block_count: 4,
        instruction_count: 16,
    }
}
fn step() -> Step {
    Step {
        authored_block: 3,
        authored_instruction: 15,
        instruction: Gfx942ProgramInstructionV1::Binary {
            opcode: Opcode::Add,
            destination: Destination::Output,
            left: Role::Input0,
            right: Role::Scratch,
        },
        operands: [Some(ValueId(0x1122_3344)), Some(ValueId(u32::MAX))],
    }
}
fn declaration_bytes() -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V19, None);
    encode_declaration(&mut writer, &declaration()).unwrap();
    writer.bytes
}
fn step_bytes(value: &Step) -> Vec<u8> {
    let mut writer = Writer::new(KERNEL_IR_VERSION_V19, None);
    encode_step(&mut writer, value).unwrap();
    writer.bytes
}
fn read_declaration(bytes: &[u8]) -> Result<Declaration, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V19;
    let value = decode_declaration(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(value)
}
fn read_step(bytes: &[u8]) -> Result<Step, KernelIrDecodeError> {
    let mut reader = Reader::new(bytes, None);
    reader.version = KERNEL_IR_VERSION_V19;
    let value = decode_step(&mut reader)?;
    if !reader.is_finished() {
        return Err(KernelIrDecodeError::TrailingBytes);
    }
    Ok(value)
}

#[test]
fn declaration_is_literal_360_bytes_with_all_ten_axes_and_full_u32_identities() {
    let bytes = declaration_bytes();
    let mut expected = vec![0];
    for value in 1..=10 {
        expected.extend_from_slice(&[value; 32]);
    }
    expected.extend_from_slice(&[4, 3, 2, 1, 32, 33, 34, 35, 36]);
    for value in [0_u32, 1, 2, 3, u32::MAX] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    expected.extend_from_slice(&[0, 7, 255, 9, 0, 0, 0, 0, 4, 16]);
    assert_eq!(expected.len(), 360);
    assert_eq!(bytes, expected);
    assert_eq!(read_declaration(&bytes), Ok(declaration()));
    for axis in 0..10 {
        let mut changed = bytes.clone();
        changed[1 + axis * 32 + 31] ^= 0x40;
        let value = read_declaration(&changed).unwrap();
        assert_ne!(value.origin, declaration().origin);
        let mut writer = Writer::new(KERNEL_IR_VERSION_V19, None);
        encode_declaration(&mut writer, &value).unwrap();
        assert_eq!(writer.bytes, changed);
    }
}

#[test]
fn step_has_literal_15_byte_layout_and_zero_none_padding() {
    let expected = vec![
        0, 3, 15, 0x89, 1, 1, 0x44, 0x33, 0x22, 0x11, 1, 0xff, 0xff, 0xff, 0xff,
    ];
    assert_eq!(step_bytes(&step()), expected);
    assert_eq!(read_step(&expected), Ok(step()));
    let mut moved = step();
    moved.instruction = Gfx942ProgramInstructionV1::Move {
        destination: Destination::Output,
        source: Role::Input0,
    };
    moved.operands[1] = None;
    let bytes = step_bytes(&moved);
    assert_eq!(bytes.len(), 15);
    assert_eq!(&bytes[10..15], &[0; 5]);
    assert_eq!(read_step(&bytes), Ok(moved));
    let mut bad = bytes;
    bad[14] = 1;
    assert_eq!(read_step(&bad), Err(KernelIrDecodeError::NonCanonical));
}

#[test]
fn every_truncated_prefix_and_extra_byte_refuses() {
    let declaration = declaration_bytes();
    let step = step_bytes(&step());
    for end in 0..declaration.len() {
        assert!(read_declaration(&declaration[..end]).is_err());
    }
    for end in 0..step.len() {
        assert!(read_step(&step[..end]).is_err());
    }
    assert_eq!(
        read_declaration(&[declaration, vec![0]].concat()),
        Err(KernelIrDecodeError::TrailingBytes)
    );
    assert_eq!(
        read_step(&[step, vec![0]].concat()),
        Err(KernelIrDecodeError::TrailingBytes)
    );
}

#[test]
fn unknown_revision_empty_axes_register_aliases_and_reserved_abi_registers_refuse() {
    let bytes = declaration_bytes();
    for tag in 1..=u8::MAX {
        let mut bad = bytes.clone();
        bad[0] = tag;
        assert!(matches!(
            read_declaration(&bad),
            Err(KernelIrDecodeError::UnknownTag { .. })
        ));
    }
    for axis in 0..10 {
        let mut bad = bytes.clone();
        bad[1 + axis * 32..1 + (axis + 1) * 32].fill(0);
        assert_eq!(
            read_declaration(&bad),
            Err(KernelIrDecodeError::NonCanonical)
        );
    }
    for index in 325..330 {
        for register in [0, 7, 64, 255] {
            let mut bad = bytes.clone();
            bad[index] = register;
            assert_eq!(
                read_declaration(&bad),
                Err(KernelIrDecodeError::NonCanonical)
            );
        }
    }
    let mut bad = bytes;
    bad[326] = bad[325];
    assert_eq!(
        read_declaration(&bad),
        Err(KernelIrDecodeError::NonCanonical)
    );
}

#[test]
fn parameter_aliases_duplicate_labels_padding_and_count_bounds_refuse() {
    let bytes = declaration_bytes();
    let mut bad = bytes.clone();
    bad[334..338].copy_from_slice(&bytes[330..334]);
    assert_eq!(
        read_declaration(&bad),
        Err(KernelIrDecodeError::NonCanonical)
    );
    for (offset, value) in [(351, 0), (357, 1), (358, 0), (358, 9), (359, 0), (359, 17)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert_eq!(
            read_declaration(&bad),
            Err(KernelIrDecodeError::NonCanonical)
        );
    }
}

#[test]
fn declaration_wire_bound_includes_eight_blocks_without_claiming_semantic_admission() {
    let mut value = declaration();
    value.block_count = 8;
    value.labels = [255, 1, 2, 3, 4, 5, 6, 0];
    value.registers = Gfx942OrderedProgramRegistersV1::new(8, 63, [9, 62, 10]).unwrap();
    let mut writer = Writer::new(KERNEL_IR_VERSION_V19, None);
    encode_declaration(&mut writer, &value).unwrap();
    assert_eq!(read_declaration(&writer.bytes), Ok(value));
}

#[test]
fn descriptor_reserved_bits_ordinals_presence_tags_and_wrong_arities_refuse() {
    let bytes = step_bytes(&step());
    for revision in 1..=u8::MAX {
        let mut bad = bytes.clone();
        bad[0] = revision;
        assert!(matches!(
            read_step(&bad),
            Err(KernelIrDecodeError::UnknownTag { .. })
        ));
    }
    for (offset, value) in [(1, 8), (2, 16), (4, 0xfc), (5, 2), (10, 2)] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(read_step(&bad).is_err());
    }
    let mut missing = bytes.clone();
    missing[10..15].fill(0);
    assert_eq!(read_step(&missing), Err(KernelIrDecodeError::NonCanonical));
    let mut missing_first = bytes;
    missing_first[5..10].fill(0);
    assert_eq!(
        read_step(&missing_first),
        Err(KernelIrDecodeError::NonCanonical)
    );
    let mut wrong_move = step();
    wrong_move.instruction = Gfx942ProgramInstructionV1::Move {
        destination: Destination::Scratch,
        source: Role::Output,
    };
    let mut writer = Writer::new(KERNEL_IR_VERSION_V19, None);
    assert!(encode_step(&mut writer, &wrong_move).is_err());
    assert!(writer.bytes.is_empty());
}

#[test]
fn repeated_self_read_ssa_operands_are_preserved() {
    let mut value = step();
    value.instruction = Gfx942ProgramInstructionV1::Binary {
        opcode: Opcode::Xor,
        destination: Destination::Output,
        left: Role::Output,
        right: Role::Output,
    };
    value.operands = [Some(ValueId(u32::MAX)); 2];
    assert_eq!(read_step(&step_bytes(&value)), Ok(value));
}

#[test]
fn leaf_version_is_exact_and_never_advances_foreign_reader_or_writer() {
    let declaration = declaration_bytes();
    let step = step_bytes(&step());
    for version in [0, 1, 12, 13, 14, 15, 16, 17, 18, 20, u16::MAX] {
        let mut writer = Writer::new(version, None);
        assert!(encode_declaration(&mut writer, &self::declaration()).is_err());
        assert!(encode_step(&mut writer, &self::step()).is_err());
        assert!(writer.bytes.is_empty());
        let mut reader = Reader::new(&declaration, None);
        reader.version = version;
        assert_eq!(
            decode_declaration(&mut reader),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
        assert_eq!(reader.offset, 0);
        let mut reader = Reader::new(&step, None);
        reader.version = version;
        assert_eq!(
            decode_step(&mut reader),
            Err(KernelIrDecodeError::UnknownVersion(version))
        );
        assert_eq!(reader.offset, 0);
    }
}

#[test]
fn literal_work_budgets_cover_count_materialize_compare_and_decode() {
    for (kind, length, precharge, fields) in [(0, 360, 1024, 25), (1, 15, 64, 8)] {
        for short in [false, true] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(precharge + fields - usize::from(short));
            let mut writer = Writer::counter(KERNEL_IR_VERSION_V19, &mut work);
            let result = if kind == 0 {
                encode_declaration(&mut writer, &declaration())
            } else {
                encode_step(&mut writer, &step())
            };
            assert_eq!(result.is_ok(), !short);
            if !short {
                assert_eq!(writer.length(), length);
            }
            assert!(writer.bytes.is_empty());
        }
        let bytes = if kind == 0 {
            declaration_bytes()
        } else {
            step_bytes(&step())
        };
        for short in [false, true] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(precharge + length - usize::from(short));
            let mut writer = Writer::new(KERNEL_IR_VERSION_V19, Some(&mut work));
            let result = if kind == 0 {
                encode_declaration(&mut writer, &declaration())
            } else {
                encode_step(&mut writer, &step())
            };
            assert_eq!(result.is_ok(), !short);
            if !short {
                assert_eq!(writer.bytes, bytes);
            }
        }
        for short in [false, true] {
            let mut work =
                CanonicalKernelIrWorkBudgetV1::new(precharge + length - usize::from(short));
            let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Work(&mut work)));
            reader.version = KERNEL_IR_VERSION_V19;
            let result = if kind == 0 {
                decode_declaration(&mut reader).map(|_| ())
            } else {
                decode_step(&mut reader).map(|_| ())
            };
            assert_eq!(result.is_ok(), !short);
        }
        for short in [false, true] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(
                precharge + length + fields - usize::from(short),
            );
            let mut writer = Writer::comparing(KERNEL_IR_VERSION_V19, &bytes, Some(&mut work));
            let result = if kind == 0 {
                encode_declaration(&mut writer, &declaration())
            } else {
                encode_step(&mut writer, &step())
            };
            assert_eq!(result.is_ok(), !short);
            assert!(writer.bytes.is_empty());
            if !short {
                assert!(matches!(
                    writer.mode,
                    WriterModeV1::Compare { matches: true, .. }
                ));
                assert_eq!(writer.length(), length);
            }
        }
    }
}

#[test]
fn declaration_and_step_share_cumulative_work_without_reset() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1024 + 360 + 64 + 14);
    let mut writer = Writer::new(KERNEL_IR_VERSION_V19, Some(&mut work));
    encode_declaration(&mut writer, &declaration()).unwrap();
    assert!(matches!(
        encode_step(&mut writer, &step()),
        Err(KernelIrEncodeError::WorkLimit(_))
    ));
}

#[test]
fn payload_decode_cannot_allocate_or_release_heap_storage() {
    struct Fixed(CanonicalKernelIrWorkBudgetV1);
    impl super::super::DecodeResourceBudgetV12 for Fixed {
        fn work_budget(&mut self) -> &mut CanonicalKernelIrWorkBudgetV1 {
            &mut self.0
        }
        fn work_limit(&self) -> usize {
            self.0.limit()
        }
        fn reserve(
            &mut self,
            _: usize,
        ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1> {
            panic!("fixed payload must not reserve storage")
        }
        fn release(
            &mut self,
            _: usize,
        ) -> Result<(), crate::CanonicalKernelIrVerificationResourceErrorV1> {
            panic!("fixed payload must not release storage")
        }
    }
    let bytes = [declaration_bytes(), step_bytes(&step())].concat();
    let mut budget = Fixed(CanonicalKernelIrWorkBudgetV1::new(1024 + 360 + 64 + 15));
    let mut reader = Reader::new(&bytes, Some(DecodeBudgetV12::Resources(&mut budget)));
    reader.version = KERNEL_IR_VERSION_V19;
    assert_eq!(decode_declaration(&mut reader), Ok(declaration()));
    assert_eq!(decode_step(&mut reader), Ok(step()));
    assert!(reader.is_finished());
}

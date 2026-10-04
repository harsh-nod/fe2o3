//! Pure packing tests; no source, executable schema or native qualification.
use super::*;
use crate::{
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramDestinationV1 as Destination,
    Gfx942ProgramInstructionV1 as Instruction, Gfx942ProgramRoleV1 as Role,
};
type Error = Gfx942CompleteBodyPackingErrorV1;
type Packed = Gfx942CompleteBodyPackedV1;
type Builder = Gfx942CompleteBodyBuilderV1;
type Terminator = Gfx942CompleteBodyTerminatorV1;
const OUTPUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Input0,
};

fn block(
    label: u8,
    instructions: &[Instruction],
    terminator: Terminator,
) -> Gfx942CompleteBodyBlockV1<'_> {
    Gfx942CompleteBodyBlockV1 {
        label: Gfx942CompleteBodyLabelV1(label),
        instructions,
        terminator,
    }
}
fn one() -> Packed {
    let mut builder = Builder::new();
    builder
        .push_block(block(7, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd))
        .unwrap();
    builder.finish().unwrap()
}
fn changed_blocks(words: [u64; 4]) -> Result<Packed, Error> {
    Packed::from_words(1, 1, words, one().instruction_words())
}
fn changed_instructions(words: [u64; 4]) -> Result<Packed, Error> {
    Packed::from_words(1, 1, one().block_words(), words)
}

#[test]
fn one_block_has_literal_numeric_words_and_borrowed_decoded_steps() {
    let packed = one();
    assert_eq!(packed.block_words(), [0x4107, 0, 0, 0]);
    assert_eq!(packed.instruction_words(), [8, 0, 0, 0]);
    let decoded = packed.decode().unwrap();
    assert_eq!((decoded.block_count(), decoded.instruction_count()), (1, 1));
    let mut blocks = decoded.blocks();
    assert_eq!(blocks.len(), 1);
    let value = blocks.next().unwrap();
    assert_eq!(value.label, Gfx942CompleteBodyLabelV1(7));
    assert_eq!(value.instructions, &[OUTPUT]);
    assert_eq!(value.terminator, Terminator::GuardedStoreOutputAndEnd);
    assert_eq!(blocks.len(), 0);
    assert!(blocks.next().is_none());
}

#[test]
fn literal_little_endian_word_transport_preserves_numeric_packing() {
    let packed = one();
    assert_eq!(
        packed.block_words()[0].to_le_bytes(),
        [7, 65, 0, 0, 0, 0, 0, 0]
    );
    assert_eq!(
        packed.instruction_words()[0].to_le_bytes(),
        [8, 0, 0, 0, 0, 0, 0, 0]
    );
    let blocks = packed
        .block_words()
        .map(|word| u64::from_le_bytes(word.to_le_bytes()));
    let instructions = packed
        .instruction_words()
        .map(|word| u64::from_le_bytes(word.to_le_bytes()));
    assert_eq!(
        Packed::from_words(1, 1, blocks, instructions).unwrap(),
        packed
    );
}

#[test]
fn literal_big_endian_word_transport_preserves_numeric_packing() {
    let packed = one();
    assert_eq!(
        packed.block_words()[0].to_be_bytes(),
        [0, 0, 0, 0, 0, 0, 65, 7]
    );
    assert_eq!(
        packed.instruction_words()[0].to_be_bytes(),
        [0, 0, 0, 0, 0, 0, 0, 8]
    );
    let blocks = packed
        .block_words()
        .map(|word| u64::from_be_bytes(word.to_be_bytes()));
    let instructions = packed
        .instruction_words()
        .map(|word| u64::from_be_bytes(word.to_be_bytes()));
    assert_eq!(
        Packed::from_words(1, 1, blocks, instructions).unwrap(),
        packed
    );
    // Interpreting the bytes in the wrong order is not an alternative codec.
    assert!(
        Packed::from_words(
            1,
            1,
            packed.block_words().map(u64::swap_bytes),
            instructions
        )
        .is_err()
    );
}

#[test]
fn all_typed_instruction_words_reuse_the_existing_descriptor_codec() {
    let instructions = [
        OUTPUT,
        Instruction::Move {
            destination: Destination::Scratch,
            source: Role::Output,
        },
        Instruction::Binary {
            opcode: Opcode::Add,
            destination: Destination::Output,
            left: Role::Input0,
            right: Role::Input1,
        },
        Instruction::Binary {
            opcode: Opcode::Subtract,
            destination: Destination::Scratch,
            left: Role::Input1,
            right: Role::Input2,
        },
        Instruction::Binary {
            opcode: Opcode::And,
            destination: Destination::Output,
            left: Role::Scratch,
            right: Role::Input0,
        },
        Instruction::Binary {
            opcode: Opcode::Or,
            destination: Destination::Output,
            left: Role::Output,
            right: Role::Input1,
        },
        Instruction::Binary {
            opcode: Opcode::Xor,
            destination: Destination::Output,
            left: Role::Output,
            right: Role::Input2,
        },
    ];
    let mut builder = Builder::new();
    builder
        .push_block(block(
            255,
            &instructions,
            Terminator::GuardedStoreOutputAndEnd,
        ))
        .unwrap();
    let packed = builder.finish().unwrap();
    for (index, instruction) in instructions.iter().enumerate() {
        assert_eq!(
            (packed.instruction_words()[index / 4] >> (16 * (index % 4))) as u16,
            instruction.descriptor()
        );
    }
    let decoded = packed.decode().unwrap();
    assert_eq!(decoded.blocks().next().unwrap().instructions, instructions);
}

#[test]
fn empty_blocks_and_all_terminators_have_distinct_zero_padded_fields() {
    let mut builder = Builder::new();
    let expected = [
        block(
            250,
            &[],
            Terminator::BranchSelectorZero {
                zero: Gfx942CompleteBodyLabelV1(0),
                nonzero: Gfx942CompleteBodyLabelV1(255),
            },
        ),
        block(
            0,
            &[OUTPUT],
            Terminator::Jump(Gfx942CompleteBodyLabelV1(255)),
        ),
        block(255, &[], Terminator::GuardedStoreOutputAndEnd),
    ];
    for value in expected {
        builder.push_block(value).unwrap();
    }
    let packed = builder.finish().unwrap();
    let decoded = packed.decode().unwrap();
    for (actual, expected) in decoded.blocks().zip(expected) {
        assert_eq!(actual.label, expected.label);
        assert_eq!(actual.instructions, expected.instructions);
        assert_eq!(actual.terminator, expected.terminator);
    }
    assert_eq!(packed.block_words()[1] >> 32, 0);
    assert_eq!(&packed.block_words()[2..], &[0, 0]);
    assert_eq!(packed.instruction_words()[0], 8);
}

#[test]
fn maximum_eight_blocks_sixteen_steps_is_exact_and_inputs_are_copied() {
    let mut builder = Builder::default();
    let mut steps = [OUTPUT; 2];
    for index in 0..8 {
        builder
            .push_block(block(index, &steps, Terminator::GuardedStoreOutputAndEnd))
            .unwrap();
    }
    assert_eq!(
        (builder.block_count(), builder.instruction_count()),
        (8, 16)
    );
    let packed = builder.finish().unwrap();
    steps[0] = EMPTY_INSTRUCTION;
    let decoded = packed.decode().unwrap();
    assert_eq!(
        (decoded.block_count(), decoded.instruction_count()),
        (8, 16)
    );
    for block in decoded.blocks() {
        assert_eq!(block.instructions, &[OUTPUT; 2]);
    }
    assert_eq!(steps[0], EMPTY_INSTRUCTION);
    assert!(!std::mem::needs_drop::<Builder>());
    assert!(!std::mem::needs_drop::<Packed>());
    assert!(!std::mem::needs_drop::<Gfx942CompleteBodyDecodedV1>());
    assert!(std::mem::size_of::<Packed>() <= 80);
    assert!(std::mem::size_of::<Gfx942CompleteBodyDecodedV1>() <= 512);
}

#[test]
fn grammar_builder_refusals_are_transactional() {
    let mut builder = Builder::new();
    for index in 0..8 {
        builder
            .push_block(block(
                index,
                &[OUTPUT],
                Terminator::GuardedStoreOutputAndEnd,
            ))
            .unwrap();
    }
    let saved = builder.clone();
    assert_eq!(
        builder.push_block(block(9, &[], Terminator::GuardedStoreOutputAndEnd)),
        Err(Error::BlockCount { count: 9 })
    );
    assert_eq!(builder, saved);
    let mut builder = Builder::new();
    let saved = builder.clone();
    assert_eq!(
        builder.push_block(block(
            0,
            &[OUTPUT; 17],
            Terminator::GuardedStoreOutputAndEnd
        )),
        Err(Error::BlockStepCount {
            block: 0,
            count: 17
        })
    );
    assert_eq!(builder, saved);
    builder
        .push_block(block(
            0,
            &[OUTPUT; 16],
            Terminator::GuardedStoreOutputAndEnd,
        ))
        .unwrap();
    let saved = builder.clone();
    assert_eq!(
        builder.push_block(block(1, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd)),
        Err(Error::StepCount { count: 17 })
    );
    assert_eq!(builder, saved);
    assert_eq!(builder.finish().unwrap().instruction_count(), 16);
}

#[test]
fn exact_count_and_declared_sum_failures_are_explicit() {
    let value = one();
    for count in [0, 9, 255] {
        assert_eq!(
            Packed::from_words(count, 1, value.block_words(), value.instruction_words()),
            Err(Error::BlockCount {
                count: usize::from(count)
            })
        );
    }
    for count in [0, 17, 255] {
        assert_eq!(
            Packed::from_words(1, count, value.block_words(), value.instruction_words()),
            Err(Error::StepCount {
                count: usize::from(count)
            })
        );
    }
    assert_eq!(Builder::new().finish(), Err(Error::BlockCount { count: 0 }));
    let mut empty = Builder::new();
    empty
        .push_block(block(7, &[], Terminator::GuardedStoreOutputAndEnd))
        .unwrap();
    assert_eq!(empty.finish(), Err(Error::StepCount { count: 0 }));
    let mut words = value.block_words();
    words[0] = 0x4007; // zero block steps, but declared total remains one
    assert_eq!(
        changed_blocks(words),
        Err(Error::StepCountMismatch {
            declared: 1,
            actual: 0
        })
    );
    words[0] = 0x4007 | (17 << 8);
    assert_eq!(
        changed_blocks(words),
        Err(Error::BlockStepCount {
            block: 0,
            count: 17
        })
    );
    let words = [
        (0x4000 | (16 << 8)) | ((0x4001_u64 | (16 << 8)) << 32),
        0,
        0,
        0,
    ];
    assert_eq!(
        Packed::from_words(2, 16, words, [0; 4]),
        Err(Error::StepCount { count: 32 })
    );
}

#[test]
fn every_inactive_block_and_step_slot_must_be_zero() {
    for block in 1..8 {
        let mut words = one().block_words();
        words[block / 2] |= 1 << (32 * (block % 2));
        assert_eq!(
            changed_blocks(words),
            Err(Error::NonZeroBlockPadding { block })
        );
    }
    for instruction in 1..16 {
        let mut words = one().instruction_words();
        words[instruction / 4] |= 1 << (16 * (instruction % 4));
        assert_eq!(
            changed_instructions(words),
            Err(Error::NonZeroInstructionPadding { instruction })
        );
    }
}

#[test]
fn reserved_block_bit_unknown_tag_and_unused_targets_are_refused() {
    let mut words = one().block_words();
    words[0] |= 1 << 31;
    assert_eq!(
        changed_blocks(words),
        Err(Error::ReservedBlockBit { block: 0 })
    );
    words = one().block_words();
    words[0] |= 1 << 13; // terminator 2 -> forbidden 3
    assert_eq!(
        changed_blocks(words),
        Err(Error::TerminatorTag { block: 0, tag: 3 })
    );
    for unused in [1 << 15, 1 << 23] {
        words = one().block_words();
        words[0] |= unused;
        assert_eq!(
            changed_blocks(words),
            Err(Error::NonZeroTargetPadding { block: 0 })
        );
    }
    words = one().block_words();
    words[0] = 7 | (1 << 8) | (1 << 23); // jump cannot retain target1
    assert_eq!(
        changed_blocks(words),
        Err(Error::NonZeroTargetPadding { block: 0 })
    );
}

#[test]
fn closed_instruction_errors_are_preserved_with_exact_ordinal() {
    for descriptor in [6_u16, 0xfc00, 7 << 4, 1 << 7] {
        let mut words = one().instruction_words();
        words[0] = u64::from(descriptor);
        let expected = Instruction::from_descriptor(descriptor).unwrap_err();
        assert_eq!(
            changed_instructions(words),
            Err(Error::Instruction {
                instruction: 0,
                error: expected
            })
        );
    }
    let mut builder = Builder::new();
    builder
        .push_block(block(
            0,
            &[OUTPUT; 16],
            Terminator::GuardedStoreOutputAndEnd,
        ))
        .unwrap();
    let packed = builder.finish().unwrap();
    let mut words = packed.instruction_words();
    words[3] = (words[3] & !(0xffff << 48)) | (6 << 48);
    assert!(matches!(
        Packed::from_words(1, 16, packed.block_words(), words),
        Err(Error::Instruction {
            instruction: 15,
            ..
        })
    ));
}

#[test]
fn packing_deliberately_does_not_admit_cfg_or_initialization() {
    let uninitialized = Instruction::Move {
        destination: Destination::Output,
        source: Role::Scratch,
    };
    let mut builder = Builder::new();
    // Duplicated label, self/backward edge and undefined Scratch are inert data.
    builder
        .push_block(block(
            3,
            &[uninitialized],
            Terminator::Jump(Gfx942CompleteBodyLabelV1(3)),
        ))
        .unwrap();
    builder
        .push_block(block(3, &[], Terminator::GuardedStoreOutputAndEnd))
        .unwrap();
    let packed = builder.finish().unwrap();
    assert_eq!(packed.decode().unwrap().blocks().len(), 2);
}

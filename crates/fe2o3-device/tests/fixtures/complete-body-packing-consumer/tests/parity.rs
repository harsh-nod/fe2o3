use device::{
    Gfx942CompleteBodyBlockV1 as DBlock, Gfx942CompleteBodyBuilderV1 as DBuilder,
    Gfx942CompleteBodyInstructionV1 as DInstruction, Gfx942CompleteBodyLabelV1 as DLabel,
    Gfx942CompleteBodyPackedV1 as DPacked, Gfx942CompleteBodyPackingErrorV1 as DError,
    Gfx942CompleteBodyTerminatorV1 as DTerm,
};
use fe2o3_device::complete_body_packing_v1 as device;
use fe2o3_kernel_ir::{
    Gfx942CompleteBodyBlockV1 as KBlock, Gfx942CompleteBodyBuilderV1 as KBuilder,
    Gfx942CompleteBodyLabelV1 as KLabel, Gfx942CompleteBodyPackedV1 as KPacked,
    Gfx942CompleteBodyTerminatorV1 as KTerm, Gfx942ProgramInstructionV1 as KInstruction,
};

fn block_word(words: &mut [u64; 4], slot: usize, word: u32) {
    let shift = 32 * (slot % 2);
    words[slot / 2] = (words[slot / 2] & !(0xffff_ffff_u64 << shift)) | ((word as u64) << shift);
}
fn instruction_word(words: &mut [u64; 4], slot: usize, word: u16) {
    let shift = 16 * (slot % 4);
    words[slot / 4] = (words[slot / 4] & !(0xffff_u64 << shift)) | ((word as u64) << shift);
}
fn compare(blocks: u8, steps: u8, block_words: [u64; 4], instruction_words: [u64; 4]) -> bool {
    let device = DPacked::from_words(blocks, steps, block_words, instruction_words);
    let compiler = KPacked::from_words(blocks, steps, block_words, instruction_words);
    match (device, compiler) {
        (Ok(device), Ok(compiler)) => {
            assert_eq!(device.block_count(), compiler.block_count());
            assert_eq!(device.instruction_count(), compiler.instruction_count());
            assert_eq!(device.block_words(), compiler.block_words());
            assert_eq!(device.instruction_words(), compiler.instruction_words());
            let decoded = compiler.decode().unwrap();
            assert_eq!(decoded.block_count(), blocks as usize);
            assert_eq!(decoded.instruction_count(), steps as usize);
            true
        }
        (Err(device), Err(compiler)) => {
            // Exact variant + fields + precedence, not merely two refusals.
            assert_eq!(format!("{device:?}"), format!("{compiler:?}"));
            false
        }
        pair => panic!("device/compiler packing mismatch: {pair:?}"),
    }
}
fn one_block(count: u8) -> [u64; 4] {
    [((count as u64) << 8) | (2 << 13), 0, 0, 0]
}

const fn const_example() -> Result<DPacked, DError> {
    let first = match DInstruction::from_descriptor(fe2o3_device::__fe2o3_ordered_program_step_v1!(
        mov(scratch, input0)
    )) {
        Ok(value) => value,
        Err(error) => {
            return Err(DError::Instruction {
                instruction: 0,
                error,
            });
        }
    };
    let second = match DInstruction::from_descriptor(
        fe2o3_device::__fe2o3_ordered_program_step_v1!(xor(out, scratch, input1)),
    ) {
        Ok(value) => value,
        Err(error) => {
            return Err(DError::Instruction {
                instruction: 1,
                error,
            });
        }
    };
    let mut builder = DBuilder::new();
    if let Err(error) = builder.push_block(DBlock {
        label: DLabel(3),
        instructions: &[first],
        terminator: DTerm::BranchSelectorZero {
            zero: DLabel(8),
            nonzero: DLabel(9),
        },
    }) {
        return Err(error);
    }
    if let Err(error) = builder.push_block(DBlock {
        label: DLabel(8),
        instructions: &[],
        terminator: DTerm::Jump(DLabel(9)),
    }) {
        return Err(error);
    }
    if let Err(error) = builder.push_block(DBlock {
        label: DLabel(9),
        instructions: &[second],
        terminator: DTerm::GuardedStoreOutputAndEnd,
    }) {
        return Err(error);
    }
    builder.finish()
}
const CONST_EXAMPLE: Result<DPacked, DError> = const_example();
const CONST_BAD_DESCRIPTOR: Result<DInstruction, device::Gfx942CompleteBodyDescriptorErrorV1> =
    DInstruction::from_descriptor(0xfc00);
const CONST_BAD_PACKING: Result<DPacked, DError> =
    DPacked::from_words(1, 1, [0x4100, 0, 0, 0], [0, 0, 0, 1]);

#[test]
fn genuine_const_builder_uses_existing_instruction_meanings() {
    let packed = CONST_EXAMPLE.unwrap();
    assert_eq!(packed.block_count(), 3);
    assert_eq!(packed.instruction_count(), 2);
    assert!(compare(
        3,
        2,
        packed.block_words(),
        packed.instruction_words()
    ));
    let mut expected = KBuilder::new();
    expected
        .push_block(KBlock {
            label: KLabel(3),
            instructions: &[KInstruction::from_descriptor(0).unwrap()],
            terminator: KTerm::BranchSelectorZero {
                zero: KLabel(8),
                nonzero: KLabel(9),
            },
        })
        .unwrap();
    expected
        .push_block(KBlock {
            label: KLabel(8),
            instructions: &[],
            terminator: KTerm::Jump(KLabel(9)),
        })
        .unwrap();
    expected
        .push_block(KBlock {
            label: KLabel(9),
            instructions: &[KInstruction::from_descriptor(189).unwrap()],
            terminator: KTerm::GuardedStoreOutputAndEnd,
        })
        .unwrap();
    let expected = expected.finish().unwrap();
    assert_eq!(packed.block_words(), expected.block_words());
    assert_eq!(packed.instruction_words(), expected.instruction_words());
    assert!(CONST_BAD_DESCRIPTOR.is_err());
    assert_eq!(
        CONST_BAD_PACKING,
        Err(DError::NonZeroInstructionPadding { instruction: 12 })
    );
}

#[test]
fn every_u16_descriptor_has_identical_grammar_and_error_precedence() {
    let mut accepted = 0;
    for word in 0..=u16::MAX {
        match (
            DInstruction::from_descriptor(word),
            KInstruction::from_descriptor(word),
        ) {
            (Ok(device), Ok(compiler)) => {
                accepted += 1;
                assert_eq!(device.descriptor(), compiler.descriptor());
                assert!(compare(1, 1, one_block(1), [word as u64, 0, 0, 0]));
            }
            (Err(device), Err(compiler)) => {
                assert_eq!(format!("{device:?}"), format!("{compiler:?}"));
                assert!(!compare(1, 1, one_block(1), [word as u64, 0, 0, 0]));
            }
            pair => panic!("descriptor {word:#06x}: {pair:?}"),
        }
    }
    assert_eq!(accepted, 260); // 2*5 moves + 5*2*5*5 binary combinations
}

#[test]
fn every_accepted_descriptor_round_trips_at_all_sixteen_slots() {
    for word in 0..=u16::MAX {
        if DInstruction::from_descriptor(word).is_err() {
            continue;
        }
        for slot in 0..16 {
            let mut words = [0; 4];
            instruction_word(&mut words, slot, word);
            assert!(compare(1, 16, one_block(16), words));
            let decoded = KPacked::from_words(1, 16, one_block(16), words)
                .unwrap()
                .decode()
                .unwrap();
            let block = decoded.blocks().next().unwrap();
            assert_eq!(block.instructions[slot].descriptor(), word);
        }
    }
}

#[test]
fn every_instruction_slot_rejects_each_closed_error_family() {
    for slot in 0..16 {
        for word in [0x0400, 6, 7, 5 << 4, (5 << 7) | 1, 1 << 7] {
            let mut words = [0; 4];
            instruction_word(&mut words, slot, word);
            assert!(!compare(1, 16, one_block(16), words));
        }
    }
}

#[test]
fn all_top_level_u8_counts_match_without_truncation() {
    for blocks in 0..=u8::MAX {
        for steps in 0..=u8::MAX {
            // One step in block 0; later declared blocks are empty jump-to-zero
            // intent. It is grammar-valid but intentionally not CFG-validated.
            assert_eq!(
                compare(blocks, steps, one_block(1), [0; 4]),
                (1..=8).contains(&blocks) && steps == 1,
            );
        }
    }
}

#[test]
fn block_step_counts_and_accumulated_count_match_in_all_eight_slots() {
    for slot in 0..8 {
        for count in 0_u32..=31 {
            let mut words = [0; 4];
            block_word(&mut words, slot, count << 8 | 2 << 13);
            let declared = count.clamp(1, 16) as u8;
            assert_eq!(
                compare(8, declared, words, [0; 4]),
                (1..=16).contains(&count)
            );
        }
    }
    for count in 1..=16 {
        let mut words = [0; 4];
        block_word(&mut words, 0, count << 8);
        block_word(&mut words, 7, (17 - count) << 8 | 2 << 13);
        assert!(!compare(8, 16, words, [0; 4]));
    }
}

#[test]
fn all_block_slots_labels_targets_and_terminator_tags_match() {
    for slot in 0..8 {
        for label in 0_u32..=255 {
            let mut words = [0; 4];
            block_word(&mut words, slot, label | 1 << 8 | 2 << 13);
            assert!(compare(8, 1, words, [0; 4]));
        }
        for target in 0..=255_u32 {
            let mut words = [0; 4];
            // Exercise all 256 values in each target field, independently of
            // label meanings; the codec must not attempt CFG admission.
            let target1 = (target + 97) & 255;
            block_word(
                &mut words,
                slot,
                1 << 8 | 1 << 13 | target << 15 | target1 << 23,
            );
            assert!(compare(8, 1, words, [0; 4]));
            block_word(&mut words, slot, 1 << 8 | target << 15);
            assert!(compare(8, 1, words, [0; 4]));
        }
        let mut words = [0; 4];
        block_word(&mut words, slot, 1 << 8 | 3 << 13);
        assert!(!compare(8, 1, words, [0; 4]));
        for target in 1..=255_u32 {
            for bad in [target << 23, 2 << 13 | target << 15, 2 << 13 | target << 23] {
                block_word(&mut words, slot, 1 << 8 | bad);
                assert!(!compare(8, 1, words, [0; 4]));
            }
        }
    }
}

#[test]
fn active_reserved_bits_and_every_unused_padding_bit_are_refused() {
    for slot in 0..8 {
        let mut words = [0; 4];
        block_word(&mut words, slot, 1 << 8 | 2 << 13 | 1 << 31);
        assert!(!compare(8, 1, words, [0; 4]));
    }
    for active in 1..=8 {
        for slot in active..8 {
            for bit in 0..32 {
                let mut words = one_block(1);
                block_word(&mut words, slot, 1 << bit);
                assert!(!compare(active as u8, 1, words, [0; 4]));
            }
        }
    }
    for active in 1..=16 {
        for slot in active..16 {
            for bit in 0..16 {
                let mut words = [0; 4];
                instruction_word(&mut words, slot, 1 << bit);
                assert!(!compare(1, active as u8, one_block(active as u8), words));
            }
        }
    }
}

#[test]
fn const_builder_refusals_are_transactional_and_maximal_body_matches() {
    let instruction = DInstruction::from_descriptor(0).unwrap();
    let mut builder = DBuilder::new();
    assert_eq!(
        builder.clone().finish(),
        Err(DError::BlockCount { count: 0 })
    );
    builder
        .push_block(DBlock {
            label: DLabel(0),
            instructions: &[],
            terminator: DTerm::Jump(DLabel(0)),
        })
        .unwrap();
    assert_eq!(
        builder.clone().finish(),
        Err(DError::StepCount { count: 0 })
    );
    let before = builder.clone();
    assert_eq!(
        builder.push_block(DBlock {
            label: DLabel(0),
            instructions: &[instruction; 17],
            terminator: DTerm::Jump(DLabel(0)),
        }),
        Err(DError::BlockStepCount {
            block: 1,
            count: 17
        })
    );
    assert_eq!(builder, before);
    builder
        .push_block(DBlock {
            label: DLabel(0),
            instructions: &[instruction; 16],
            terminator: DTerm::Jump(DLabel(0)),
        })
        .unwrap();
    let before = builder.clone();
    assert_eq!(
        builder.push_block(DBlock {
            label: DLabel(0),
            instructions: &[instruction],
            terminator: DTerm::Jump(DLabel(0)),
        }),
        Err(DError::StepCount { count: 17 })
    );
    assert_eq!(builder, before);
    for _ in 2..8 {
        builder
            .push_block(DBlock {
                label: DLabel(0),
                instructions: &[],
                terminator: DTerm::GuardedStoreOutputAndEnd,
            })
            .unwrap();
    }
    let before = builder.clone();
    assert_eq!(
        builder.push_block(DBlock {
            label: DLabel(0),
            instructions: &[],
            terminator: DTerm::Jump(DLabel(0)),
        }),
        Err(DError::BlockCount { count: 9 })
    );
    assert_eq!(builder, before);
    assert_eq!(builder.block_count(), 8);
    assert_eq!(builder.instruction_count(), 16);
    let packed = builder.finish().unwrap();
    assert!(compare(
        8,
        16,
        packed.block_words(),
        packed.instruction_words()
    ));
    // Duplicate labels, backward edges and undefined output still pack:
    // neither public crate codec is a semantic admission constructor.
}

#[test]
fn numeric_words_survive_explicit_both_endian_byte_round_trips() {
    let packed = CONST_EXAMPLE.unwrap();
    for little_endian in [false, true] {
        let round_trip = |words: [u64; 4]| {
            words.map(|word| {
                let bytes = if little_endian {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                };
                if little_endian {
                    u64::from_le_bytes(bytes)
                } else {
                    u64::from_be_bytes(bytes)
                }
            })
        };
        assert!(compare(
            packed.block_count(),
            packed.instruction_count(),
            round_trip(packed.block_words()),
            round_trip(packed.instruction_words()),
        ));
    }
}

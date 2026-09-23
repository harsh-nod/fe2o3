//! Synthetic packing/checker composition only; no source/native authority.
use super::*;
use fe2o3_kernel_ir::Gfx942CompleteBodyBuilderV1 as Builder;
type Block<'a> = Gfx942CompleteBodyBlockV1<'a>;
type Terminator = Gfx942CompleteBodyTerminatorV1;
type BodyError = Gfx942CompleteBodyErrorV1;
type Error = Gfx942CompleteBodyPackedCheckErrorV1;
const OUTPUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Input0,
};
const SCRATCH: Instruction = Instruction::Move {
    destination: Destination::Scratch,
    source: Role::Input1,
};
const SCRATCH_OUTPUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Scratch,
};

fn label(value: u8) -> Gfx942CompleteBodyLabelV1 {
    Gfx942CompleteBodyLabelV1(value)
}
fn block(value: u8, instructions: &[Instruction], terminator: Terminator) -> Block<'_> {
    Block {
        label: label(value),
        instructions,
        terminator,
    }
}
fn registers() -> Registers {
    Registers::new(32, 33, [34, 35, 36]).unwrap()
}
fn packed(blocks: &[Block<'_>]) -> Gfx942CompleteBodyPackedV1 {
    let mut builder = Builder::new();
    for &block in blocks {
        builder.push_block(block).unwrap();
    }
    builder.finish().unwrap()
}
fn direct(blocks: &[Block<'_>]) -> Result<Gfx942CompleteBodyPlanV1, BodyError> {
    Gfx942CompleteBodyPlanV1::check(
        Gfx942CompleteBodyBoundaryV1::PROFILE,
        registers(),
        Gfx942CompleteBodyResourcesV1::required(registers()),
        blocks,
        &mut CanonicalKernelIrWorkBudgetV1::new(512),
    )
}
fn decoded(blocks: &[Block<'_>]) -> Result<Gfx942CompleteBodyPlanV1, Error> {
    Gfx942CompleteBodyPlanV1::check_packed(
        Gfx942CompleteBodyBoundaryV1::PROFILE,
        registers(),
        Gfx942CompleteBodyResourcesV1::required(registers()),
        packed(blocks),
        &mut CanonicalKernelIrWorkBudgetV1::new(576),
    )
}

#[test]
fn old_and_neutral_public_primitive_names_are_the_same_types() {
    let neutral = fe2o3_kernel_ir::Gfx942CompleteBodyBlockV1 {
        label: fe2o3_kernel_ir::Gfx942CompleteBodyLabelV1(255),
        instructions: &[OUTPUT],
        terminator: fe2o3_kernel_ir::Gfx942CompleteBodyTerminatorV1::GuardedStoreOutputAndEnd,
    };
    let old: Gfx942CompleteBodyBlockV1<'_> = neutral;
    assert_eq!(direct(&[old]).unwrap(), decoded(&[old]).unwrap());
    assert_eq!(
        GFX942_COMPLETE_BODY_MAX_BLOCKS_V1,
        fe2o3_kernel_ir::GFX942_COMPLETE_BODY_MAX_BLOCKS_V1
    );
    assert_eq!(
        GFX942_COMPLETE_BODY_MAX_STEPS_V1,
        fe2o3_kernel_ir::GFX942_COMPLETE_BODY_MAX_STEPS_V1
    );
}

#[test]
fn every_merge_definition_combination_reuses_the_existing_checker() {
    for mask in 0..4 {
        let left: &[Instruction] = if mask & 1 != 0 { &[SCRATCH] } else { &[] };
        let right: &[Instruction] = if mask & 2 != 0 { &[SCRATCH] } else { &[] };
        let blocks = [
            block(
                7,
                &[OUTPUT],
                Terminator::BranchSelectorZero {
                    zero: label(91),
                    nonzero: label(3),
                },
            ),
            block(91, left, Terminator::Jump(label(250))),
            block(3, right, Terminator::Jump(label(250))),
            block(250, &[SCRATCH_OUTPUT], Terminator::GuardedStoreOutputAndEnd),
        ];
        assert_eq!(decoded(&blocks), direct(&blocks).map_err(Error::Body));
        assert_eq!(decoded(&blocks).is_ok(), mask == 3);
    }
}

#[test]
fn grammar_acceptance_never_replaces_cfg_or_definition_admission() {
    let cases: &[&[Block<'_>]] = &[
        &[block(0, &[OUTPUT], Terminator::Jump(label(0)))],
        &[
            block(0, &[OUTPUT], Terminator::Jump(label(9))),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ],
        &[
            block(0, &[OUTPUT], Terminator::Jump(label(1))),
            block(0, &[], Terminator::GuardedStoreOutputAndEnd),
        ],
        &[
            block(0, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ],
        &[block(
            0,
            &[SCRATCH_OUTPUT],
            Terminator::GuardedStoreOutputAndEnd,
        )],
        &[block(0, &[SCRATCH], Terminator::GuardedStoreOutputAndEnd)],
        &[
            block(
                0,
                &[OUTPUT],
                Terminator::BranchSelectorZero {
                    zero: label(1),
                    nonzero: label(1),
                },
            ),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ],
    ];
    for blocks in cases {
        assert!(packed(blocks).decode().is_ok());
        assert!(direct(blocks).is_err());
        assert_eq!(decoded(blocks), direct(blocks).map_err(Error::Body));
    }
}

#[test]
fn all_eight_blocks_and_sixteen_steps_roundtrip_without_reordering() {
    let steps = [OUTPUT; 2];
    let mut blocks = [block(0, &steps, Terminator::GuardedStoreOutputAndEnd); 8];
    for (index, value) in blocks.iter_mut().enumerate() {
        value.label = label((255 - index) as u8);
        if index < 7 {
            value.terminator = Terminator::Jump(label((254 - index) as u8));
        }
    }
    let actual = decoded(&blocks).unwrap();
    assert_eq!(actual, direct(&blocks).unwrap());
    assert_eq!((actual.block_count(), actual.instruction_count()), (8, 16));
}

#[test]
fn boundary_resource_and_reserved_register_refusals_are_unchanged() {
    let input = packed(&[block(0, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd)]);
    let mut boundary = Gfx942CompleteBodyBoundaryV1::PROFILE;
    boundary.wave_width = 32;
    let mut resources = Gfx942CompleteBodyResourcesV1::required(registers());
    resources.restores_exec = false;
    for (boundary, registers, resources, expected) in [
        (
            boundary,
            registers(),
            Gfx942CompleteBodyResourcesV1::required(registers()),
            BodyError::Boundary,
        ),
        (
            Gfx942CompleteBodyBoundaryV1::PROFILE,
            registers(),
            resources,
            BodyError::Resources,
        ),
        (
            Gfx942CompleteBodyBoundaryV1::PROFILE,
            Registers::new(7, 33, [34, 35, 36]).unwrap(),
            Gfx942CompleteBodyResourcesV1::required(Registers::new(7, 33, [34, 35, 36]).unwrap()),
            BodyError::ReservedVgpr { register: 7 },
        ),
    ] {
        assert_eq!(
            Gfx942CompleteBodyPlanV1::check_packed(
                boundary,
                registers,
                resources,
                input,
                &mut CanonicalKernelIrWorkBudgetV1::new(576)
            ),
            Err(Error::Body(expected))
        );
    }
}

#[test]
fn decode_and_checker_work_are_distinct_prepaid_debits_with_incoming_floor() {
    let input = packed(&[block(0, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd)]);
    for (available, accepted, failed) in [
        (63, 0, Some(75)),
        (64, 64, Some(587)),
        (575, 64, Some(587)),
        (576, 576, None),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(11 + available);
        work.charge_work(11).unwrap();
        let result = Gfx942CompleteBodyPlanV1::check_packed(
            Gfx942CompleteBodyBoundaryV1::PROFILE,
            registers(),
            Gfx942CompleteBodyResourcesV1::required(registers()),
            input,
            &mut work,
        );
        assert_eq!(result.is_ok(), available == 576);
        assert_eq!(work.work(), 11 + accepted);
        assert_eq!(work.failed_work(), failed);
    }
    // Direct Layer A still consumes exactly its original 512 units.
    let mut work = CanonicalKernelIrWorkBudgetV1::new(523);
    work.charge_work(11).unwrap();
    Gfx942CompleteBodyPlanV1::check(
        Gfx942CompleteBodyBoundaryV1::PROFILE,
        registers(),
        Gfx942CompleteBodyResourcesV1::required(registers()),
        &[block(0, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd)],
        &mut work,
    )
    .unwrap();
    assert_eq!(work.work(), 523);
}

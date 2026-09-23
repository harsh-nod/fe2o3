//! Synthetic structural checks only; no source/native execution or authority.
use super::*;
use fe2o3_kernel_ir::Gfx942ProgramBinaryOpcodeV1 as Opcode;
type Label = Gfx942CompleteBodyLabelV1;
type Terminator = Gfx942CompleteBodyTerminatorV1;
type Error = Gfx942CompleteBodyErrorV1;
type Plan = Gfx942CompleteBodyPlanV1;
type Boundary = Gfx942CompleteBodyBoundaryV1;
type Resources = Gfx942CompleteBodyResourcesV1;

const OUTPUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Input0,
};
const SCRATCH: Instruction = Instruction::Move {
    destination: Destination::Scratch,
    source: Role::Input1,
};
const SCRATCH_TO_OUTPUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Scratch,
};

fn label(value: u8) -> Label {
    Gfx942CompleteBodyLabelV1(value)
}
fn registers() -> Registers {
    Registers::new(32, 33, [34, 35, 36]).unwrap()
}
fn block(
    id: u8,
    instructions: &[Instruction],
    terminator: Terminator,
) -> Gfx942CompleteBodyBlockV1<'_> {
    Gfx942CompleteBodyBlockV1 {
        label: label(id),
        instructions,
        terminator,
    }
}
fn check(blocks: &[Gfx942CompleteBodyBlockV1<'_>]) -> Result<Plan, Error> {
    let registers = registers();
    Plan::check(
        Boundary::PROFILE,
        registers,
        Resources::required(registers),
        blocks,
        &mut CanonicalKernelIrWorkBudgetV1::new(512),
    )
}
fn terminal(instructions: &[Instruction]) -> Gfx942CompleteBodyBlockV1<'_> {
    block(240, instructions, Terminator::GuardedStoreOutputAndEnd)
}

#[test]
fn one_body_copies_intent_and_exposes_only_active_fixed_storage() {
    let mut instructions = [
        OUTPUT,
        Instruction::Move {
            destination: Destination::Output,
            source: Role::Output,
        },
    ];
    let plan = check(&[terminal(&instructions)]).unwrap();
    instructions[0] = SCRATCH;
    assert_eq!(plan.block_count(), 1);
    assert_eq!(plan.instruction_count(), 2);
    assert_eq!(plan.registers(), registers());
    assert_eq!(plan.boundary(), Boundary::PROFILE);
    assert_eq!(plan.resources(), Resources::required(registers()));
    let projected = plan.block(0).unwrap();
    assert_eq!(projected.label, label(240));
    assert_eq!(
        projected.instructions,
        &[
            OUTPUT,
            Instruction::Move {
                destination: Destination::Output,
                source: Role::Output,
            }
        ]
    );
    assert_eq!(projected.defined_in, 0b00111);
    assert_eq!(projected.defined_out, 0b10111);
    assert!(plan.block(1).is_none());
    assert!(plan.block(8).is_none());
    assert!(plan.block(usize::MAX).is_none());
    assert!(std::mem::size_of::<Plan>() <= 512);
    assert!(!std::mem::needs_drop::<Plan>());
    assert_eq!(instructions[0], SCRATCH); // input mutation was real, not aliased
}

#[test]
fn preserves_dead_and_self_writes_without_an_optimization_pass() {
    let steps = [OUTPUT; 16];
    let plan = check(&[terminal(&steps)]).unwrap();
    assert_eq!(plan.instruction_count(), 16);
    assert_eq!(plan.block(0).unwrap().instructions, &steps);
}

#[test]
fn two_branch_outputs_must_both_reach_the_merge() {
    for mask in 0..4 {
        let left: &[Instruction] = if mask & 1 != 0 { &[OUTPUT] } else { &[] };
        let right: &[Instruction] = if mask & 2 != 0 { &[OUTPUT] } else { &[] };
        let blocks = [
            block(
                7,
                &[SCRATCH],
                Terminator::BranchSelectorZero {
                    zero: label(91),
                    nonzero: label(3),
                },
            ),
            block(91, left, Terminator::Jump(label(250))),
            block(3, right, Terminator::Jump(label(250))),
            block(250, &[], Terminator::GuardedStoreOutputAndEnd),
        ];
        let actual = check(&blocks);
        assert_eq!(actual.is_ok(), mask == 3, "arm definitions {mask}");
        if let Ok(plan) = actual {
            assert_eq!(plan.block(3).unwrap().defined_in, 0b11111);
            // Labels are not ordinals and may numerically decrease.
            assert_eq!(plan.block(2).unwrap().label, label(3));
        } else {
            assert_eq!(actual, Err(Error::OutputNotDefined { label: label(250) }));
        }
    }
}

#[test]
fn every_three_predecessor_combination_uses_intersection_not_union() {
    for mask in 0..8 {
        let a: &[Instruction] = if mask & 1 != 0 { &[SCRATCH] } else { &[] };
        let b: &[Instruction] = if mask & 2 != 0 { &[SCRATCH] } else { &[] };
        let c: &[Instruction] = if mask & 4 != 0 { &[SCRATCH] } else { &[] };
        let blocks = [
            block(
                0,
                &[],
                Terminator::BranchSelectorZero {
                    zero: label(1),
                    nonzero: label(2),
                },
            ),
            block(
                1,
                &[],
                Terminator::BranchSelectorZero {
                    zero: label(3),
                    nonzero: label(4),
                },
            ),
            block(2, a, Terminator::Jump(label(5))),
            block(3, b, Terminator::Jump(label(5))),
            block(4, c, Terminator::Jump(label(5))),
            block(
                5,
                &[SCRATCH_TO_OUTPUT],
                Terminator::GuardedStoreOutputAndEnd,
            ),
        ];
        let actual = check(&blocks);
        assert_eq!(actual.is_ok(), mask == 7, "predecessor definitions {mask}");
        if mask != 7 {
            assert_eq!(
                actual,
                Err(Error::ReadBeforeDefinition {
                    label: label(5),
                    instruction: 0,
                    roles: 0b01000,
                })
            );
        }
    }
}

#[test]
fn independent_terminal_paths_each_require_a_defined_output() {
    let blocks = [
        block(
            0,
            &[],
            Terminator::BranchSelectorZero {
                zero: label(1),
                nonzero: label(2),
            },
        ),
        block(1, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd),
        block(2, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd),
    ];
    assert!(check(&blocks).is_ok());
    let bad = [
        blocks[0],
        block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        blocks[2],
    ];
    assert_eq!(
        check(&bad),
        Err(Error::OutputNotDefined { label: label(1) })
    );
}

#[test]
fn reads_pre_step_state_and_rejects_uninitialized_scratch_or_output() {
    for (source, roles) in [(Role::Scratch, 0b01000), (Role::Output, 0b10000)] {
        let step = Instruction::Move {
            destination: Destination::Output,
            source,
        };
        assert_eq!(
            check(&[terminal(&[step])]),
            Err(Error::ReadBeforeDefinition {
                label: label(240),
                instruction: 0,
                roles,
            })
        );
    }
    let step = Instruction::Binary {
        opcode: Opcode::Xor,
        destination: Destination::Scratch,
        left: Role::Scratch,
        right: Role::Output,
    };
    assert_eq!(
        check(&[terminal(&[step, OUTPUT])]),
        Err(Error::ReadBeforeDefinition {
            label: label(240),
            instruction: 0,
            roles: 0b11000,
        })
    );
}

#[test]
fn exact_eight_block_and_sixteen_step_boundaries() {
    let steps = [OUTPUT, OUTPUT];
    let blocks = [
        block(0, &steps, Terminator::Jump(label(1))),
        block(1, &steps, Terminator::Jump(label(2))),
        block(2, &steps, Terminator::Jump(label(3))),
        block(3, &steps, Terminator::Jump(label(4))),
        block(4, &steps, Terminator::Jump(label(5))),
        block(5, &steps, Terminator::Jump(label(6))),
        block(6, &steps, Terminator::Jump(label(7))),
        block(7, &steps, Terminator::GuardedStoreOutputAndEnd),
    ];
    let plan = check(&blocks).unwrap();
    assert_eq!((plan.block_count(), plan.instruction_count()), (8, 16));
    assert_eq!(check(&[]), Err(Error::BlockCount { count: 0 }));
    assert_eq!(
        check(&[terminal(&[OUTPUT]); 9]),
        Err(Error::BlockCount { count: 9 })
    );
    assert_eq!(check(&[terminal(&[])]), Err(Error::StepCount { count: 0 }));
    assert_eq!(
        check(&[terminal(&[OUTPUT; 17])]),
        Err(Error::StepCount { count: 17 })
    );
    let distributed = [
        block(0, &[OUTPUT; 9], Terminator::Jump(label(1))),
        block(1, &[OUTPUT; 8], Terminator::GuardedStoreOutputAndEnd),
    ];
    assert_eq!(check(&distributed), Err(Error::StepCount { count: 17 }));
}

#[test]
fn duplicate_missing_self_and_backward_labels_refuse() {
    assert_eq!(
        check(&[
            block(1, &[OUTPUT], Terminator::Jump(label(1))),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::DuplicateLabel { label: label(1) })
    );
    assert_eq!(
        check(&[
            block(0, &[OUTPUT], Terminator::Jump(label(99))),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::MissingLabel { label: label(99) })
    );
    assert_eq!(
        check(&[
            block(0, &[OUTPUT], Terminator::Jump(label(0))),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::NonForwardEdge {
            from: label(0),
            to: label(0)
        })
    );
    assert_eq!(
        check(&[
            block(0, &[OUTPUT], Terminator::Jump(label(1))),
            block(1, &[], Terminator::Jump(label(0))),
            block(2, &[], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::NonForwardEdge {
            from: label(1),
            to: label(0)
        })
    );
}

#[test]
fn alias_targets_unreachable_blocks_and_nonterminal_last_block_refuse() {
    assert_eq!(
        check(&[
            block(
                0,
                &[OUTPUT],
                Terminator::BranchSelectorZero {
                    zero: label(1),
                    nonzero: label(1)
                }
            ),
            block(1, &[], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::AliasedBranchTargets { label: label(1) })
    );
    assert_eq!(
        check(&[
            block(0, &[OUTPUT], Terminator::Jump(label(2))),
            block(1, &[OUTPUT], Terminator::Jump(label(2))),
            block(2, &[], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::UnreachableBlock { label: label(1) })
    );
    assert_eq!(
        check(&[
            block(0, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd),
            block(1, &[OUTPUT], Terminator::GuardedStoreOutputAndEnd),
        ]),
        Err(Error::UnreachableBlock { label: label(1) })
    );
    assert_eq!(
        check(&[block(0, &[OUTPUT], Terminator::Jump(label(0)))]),
        Err(Error::LastBlockMustTerminate)
    );
}

#[test]
fn every_reserved_vgpr_role_refuses_while_v8_and_v63_are_in_range() {
    for reserved in 0..8 {
        for role in 0..5 {
            let mut roles = [32, 33, 34, 35, 36];
            roles[role] = reserved;
            let registers =
                Registers::new(roles[0], roles[1], [roles[2], roles[3], roles[4]]).unwrap();
            let actual = Plan::check(
                Boundary::PROFILE,
                registers,
                Resources::required(registers),
                &[terminal(&[OUTPUT])],
                &mut CanonicalKernelIrWorkBudgetV1::new(512),
            );
            assert_eq!(actual, Err(Error::ReservedVgpr { register: reserved }));
        }
    }
    let registers = Registers::new(8, 63, [9, 10, 11]).unwrap();
    let plan = Plan::check(
        Boundary::PROFILE,
        registers,
        Resources::required(registers),
        &[terminal(&[OUTPUT])],
        &mut CanonicalKernelIrWorkBudgetV1::new(512),
    )
    .unwrap();
    assert_eq!(plan.resources().vgpr_binding_extent, 64);
    assert!(Registers::new(32, 32, [34, 35, 36]).is_err());
    assert!(Registers::new(32, 64, [34, 35, 36]).is_err());
}

#[test]
fn every_boundary_axis_refuses_independently() {
    let mut variants = [Boundary::PROFILE; 7];
    variants[0].pointer_bits = 32;
    variants[1].index_bits = 32;
    variants[2].wave_width = 32;
    variants[3].xnack_enabled = true;
    variants[4].required_workgroup = [32, 1, 1];
    variants[5].maximum_workgroup = [128, 1, 1];
    variants[6].required_workgroup = [1, 64, 1];
    for boundary in variants {
        let actual = Plan::check(
            boundary,
            registers(),
            Resources::required(registers()),
            &[terminal(&[OUTPUT])],
            &mut CanonicalKernelIrWorkBudgetV1::new(512),
        );
        assert_eq!(actual, Err(Error::Boundary));
    }
    assert_eq!(Boundary::EXPLICIT_PARAMETER_OFFSETS, [0, 8, 16, 20, 24, 28]);
    assert_eq!(Boundary::EXPLICIT_PARAMETER_BYTES, 32);
    assert_eq!(Boundary::EXPLICIT_PARAMETER_ALIGNMENT, 8);
}

#[test]
fn each_implicit_state_resource_and_helper_declaration_is_exact() {
    let expected = Resources::required(registers());
    let mut variants = [expected; 13];
    variants[0].vgpr_binding_extent -= 1;
    variants[1].vgpr_binding_extent += 1;
    variants[2].fixed_sgpr_binding_extent -= 1;
    variants[3].fixed_sgpr_binding_extent += 1;
    variants[4].reads_exec = false;
    variants[5].restores_exec = false;
    variants[6].clobbers_vcc = false;
    variants[7].clobbers_scc = false;
    variants[8].guarded_global_u32_store = false;
    variants[9].global_loads = true;
    variants[10].group_segment_bytes = 4;
    variants[11].private_segment_bytes = 4;
    variants[12].runtime_helper_calls = 1;
    for resources in variants {
        let actual = Plan::check(
            Boundary::PROFILE,
            registers(),
            resources,
            &[terminal(&[OUTPUT])],
            &mut CanonicalKernelIrWorkBudgetV1::new(512),
        );
        assert_eq!(actual, Err(Error::Resources));
    }
    assert_eq!(expected.fixed_sgpr_binding_extent, 23);
}

#[test]
fn prepaid_work_respects_existing_floor_and_failure_semantics() {
    let blocks = [terminal(&[OUTPUT])];
    let mut short = CanonicalKernelIrWorkBudgetV1::new(562);
    short.charge_work(51).unwrap();
    let actual = Plan::check(
        Boundary::PROFILE,
        registers(),
        Resources::required(registers()),
        &blocks,
        &mut short,
    );
    let Err(Error::Work(error)) = actual else {
        panic!("expected bounded work failure");
    };
    assert_eq!(error.actual(), 563);
    assert_eq!(error.limit(), 562);
    assert_eq!(short.work(), 51);
    assert_eq!(short.failed_work(), Some(563));

    let mut exact = CanonicalKernelIrWorkBudgetV1::new(563);
    exact.charge_work(51).unwrap();
    assert!(
        Plan::check(
            Boundary::PROFILE,
            registers(),
            Resources::required(registers()),
            &blocks,
            &mut exact
        )
        .is_ok()
    );
    assert_eq!(exact.work(), 563);

    let mut invalid = CanonicalKernelIrWorkBudgetV1::new(512);
    assert_eq!(
        Plan::check(
            Boundary::PROFILE,
            registers(),
            Resources::required(registers()),
            &[],
            &mut invalid
        ),
        Err(Error::BlockCount { count: 0 })
    );
    assert_eq!(invalid.work(), 512);
}

#[test]
fn all_existing_binary_meanings_and_input_roles_are_structurally_reused() {
    for opcode in [
        Opcode::Add,
        Opcode::Subtract,
        Opcode::And,
        Opcode::Or,
        Opcode::Xor,
    ] {
        for left in [Role::Input0, Role::Input1, Role::Input2] {
            for right in [Role::Input0, Role::Input1, Role::Input2] {
                let instruction = Instruction::Binary {
                    opcode,
                    destination: Destination::Output,
                    left,
                    right,
                };
                let plan = check(&[terminal(&[instruction])]).unwrap();
                assert_eq!(plan.block(0).unwrap().instructions, &[instruction]);
            }
        }
    }
}

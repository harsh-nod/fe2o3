//! Synthetic pure mechanism checks, not source or LLVM/native qualification.
use super::*;
use crate::{
    Gfx942CompleteBodyBlockV1 as Block, Gfx942CompleteBodyBoundaryV1 as Boundary,
    Gfx942CompleteBodyErrorV1 as BodyError, Gfx942CompleteBodyResourcesV1 as Resources,
};
use fe2o3_kernel_ir::{
    Gfx942CompleteBodyLabelV1 as Label, Gfx942CompleteBodyTerminatorV1 as Term,
    Gfx942ProgramBinaryOpcodeV1 as Opcode, Gfx942ProgramDestinationV1 as Destination,
    Gfx942ProgramInstructionV1 as Instruction, Gfx942ProgramRoleV1 as Role,
};
type Plan = Gfx942CompleteBodyPlanV1;
type Registers = Gfx942OrderedProgramRegistersV1;
const OUT: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Input0,
};
const SCRATCH: Instruction = Instruction::Move {
    destination: Destination::Scratch,
    source: Role::Input1,
};
const USE_SCRATCH: Instruction = Instruction::Move {
    destination: Destination::Output,
    source: Role::Scratch,
};

fn registers() -> Registers {
    Registers::new(32, 33, [34, 35, 36]).unwrap()
}
fn block(label: u8, instructions: &[Instruction], terminator: Term) -> Block<'_> {
    Block {
        label: Label(label),
        instructions,
        terminator,
    }
}
fn check(blocks: &[Block<'_>], registers: Registers) -> Result<Plan, BodyError> {
    Plan::check(
        Boundary::PROFILE,
        registers,
        Resources::required(registers),
        blocks,
        &mut CanonicalKernelIrWorkBudgetV1::new(512),
    )
}
fn render(plan: &Plan) -> Gfx942CompleteBodyEmissionV1 {
    render_gfx942_complete_body_llvm_v1(
        plan,
        Gfx942CompleteBodySymbolV1::new("checked_body").unwrap(),
        &mut CanonicalKernelIrWorkBudgetV1::new(GFX942_COMPLETE_BODY_RENDER_WORK_V1),
    )
    .unwrap()
}
fn one() -> Plan {
    check(
        &[block(255, &[OUT], Term::GuardedStoreOutputAndEnd)],
        registers(),
    )
    .unwrap()
}
fn output_diamond() -> Plan {
    check(
        &[
            block(
                240,
                &[],
                Term::BranchSelectorZero {
                    zero: Label(17),
                    nonzero: Label(2),
                },
            ),
            block(17, &[OUT], Term::Jump(Label(4))),
            block(
                2,
                &[Instruction::Move {
                    destination: Destination::Output,
                    source: Role::Input1,
                }],
                Term::Jump(Label(4)),
            ),
            block(4, &[], Term::GuardedStoreOutputAndEnd),
        ],
        registers(),
    )
    .unwrap()
}
fn scratch_diamond() -> Plan {
    check(
        &[
            block(
                0,
                &[],
                Term::BranchSelectorZero {
                    zero: Label(93),
                    nonzero: Label(7),
                },
            ),
            block(93, &[SCRATCH], Term::Jump(Label(250))),
            block(
                7,
                &[Instruction::Move {
                    destination: Destination::Scratch,
                    source: Role::Input0,
                }],
                Term::Jump(Label(250)),
            ),
            block(250, &[USE_SCRATCH], Term::GuardedStoreOutputAndEnd),
        ],
        registers(),
    )
    .unwrap()
}
fn two_terminals() -> Plan {
    check(
        &[
            block(
                41,
                &[],
                Term::BranchSelectorZero {
                    zero: Label(10),
                    nonzero: Label(3),
                },
            ),
            block(10, &[OUT], Term::GuardedStoreOutputAndEnd),
            block(3, &[OUT], Term::GuardedStoreOutputAndEnd),
        ],
        registers(),
    )
    .unwrap()
}
fn maximum() -> Plan {
    let repeated = [Instruction::Move {
        destination: Destination::Output,
        source: Role::Output,
    }; 14];
    check(
        &[
            block(200, &[OUT], Term::Jump(Label(0))),
            block(0, &[], Term::Jump(Label(255))),
            block(255, &repeated, Term::Jump(Label(18))),
            block(18, &[], Term::Jump(Label(1))),
            block(1, &[], Term::Jump(Label(99))),
            block(99, &[], Term::Jump(Label(44))),
            block(44, &[], Term::Jump(Label(2))),
            block(2, &[OUT], Term::GuardedStoreOutputAndEnd),
        ],
        Registers::new(60, 63, [8, 9, 10]).unwrap(),
    )
    .unwrap()
}

#[test]
fn five_checked_profiles_stay_bounded_and_retain_every_step() {
    for plan in [
        one(),
        output_diamond(),
        scratch_diamond(),
        two_terminals(),
        maximum(),
    ] {
        let emitted = render(&plan);
        assert!(emitted.llvm_ir().len() <= GFX942_COMPLETE_BODY_LLVM_BYTES_V1);
        assert!(emitted.assembly_template().len() <= GFX942_COMPLETE_BODY_ASSEMBLY_BYTES_V1);
        assert_eq!(emitted.registers(), plan.registers());
        assert_eq!(emitted.blocks().count(), plan.block_count());
        assert_eq!(emitted.instructions().count(), plan.instruction_count());
        let source: Vec<_> = (0..plan.block_count())
            .flat_map(|i| {
                plan.block(i)
                    .unwrap()
                    .instructions
                    .iter()
                    .map(|i| i.descriptor())
            })
            .collect();
        assert_eq!(
            emitted
                .instructions()
                .map(|row| row.descriptor)
                .collect::<Vec<_>>(),
            source
        );
        assert_eq!(
            emitted
                .assembly_template()
                .matches("global_store_dword ")
                .count(),
            1
        );
        assert_eq!(emitted.assembly_template().matches("s_endpgm").count(), 1);
    }
}

#[test]
fn one_block_has_exact_generated_label_arithmetic_jump_and_guarded_tail() {
    let emitted = render(&one());
    let expected = concat!(
        ".Lfe2o3_cb_${:uid}_b0:\n",
        "v_mov_b32_e32 v33, v34\n",
        "s_branch .Lfe2o3_cb_${:uid}_tail\n",
        ".Lfe2o3_cb_${:uid}_tail:\n",
        "v_lshlrev_b64 v[2:3], 2, v[0:1]\n",
        "v_add_co_u32_e32 v2, vcc, s16, v2\n",
        "v_addc_co_u32_e32 v3, vcc, v4, v3, vcc\n",
        "v_cmp_gt_u64_e32 vcc, s[18:19], v[0:1]\n",
        "s_and_saveexec_b64 s[20:21], vcc\n",
        "global_store_dword v[2:3], v33, off\n",
        "s_waitcnt vmcnt(0)\n",
        "s_mov_b64 exec, s[20:21]\n",
        "s_endpgm\n",
    );
    assert_eq!(emitted.assembly_template(), expected);
    assert_eq!(
        emitted.compiler_tail(),
        Gfx942CompleteBodyEmittedTailV1 {
            label_line: 3,
            first_instruction_line: 4,
            instruction_count: 9,
        }
    );
    let block = *emitted.blocks().next().unwrap();
    assert_eq!(block.ordinal, 0);
    assert_eq!(block.label, Label(255)); // source label is not native/template ordinal
    assert_eq!(
        (
            block.label_line,
            block.terminator_line,
            block.terminator_line_count
        ),
        (0, 2, 1)
    );
    assert_eq!(
        block.terminator,
        Gfx942CompleteBodyLoweredTerminatorV1::CompilerTail
    );
    assert_eq!(emitted.instructions().next().unwrap().assembly_line, 1);
}

#[test]
fn six_machine_arguments_and_exact_selector_input_are_fixed() {
    let emitted = render(&one());
    let text = emitted.llvm_ir();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("define "))
            .collect::<Vec<_>>(),
        [
            "define amdgpu_kernel void @checked_body(ptr addrspace(1) %data, i64 %length, i32 %a, i32 %b, i32 %c, i32 %selector) #0 !reqd_work_group_size !0 {"
        ]
    );
    assert_eq!(Boundary::EXPLICIT_PARAMETER_OFFSETS, [0, 8, 16, 20, 24, 28]);
    assert_eq!(Boundary::EXPLICIT_PARAMETER_BYTES, 32);
    assert_eq!(Boundary::EXPLICIT_PARAMETER_ALIGNMENT, 8);
    let calls: Vec<_> = text.lines().filter(|line| line.contains(" asm ")).collect();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].ends_with(concat!(
        "\", \"{s16},{v4},{s18},{s19},{v34},{v35},{v36},{v0},{v1},{s22},",
        "~{v2},~{v3},~{v32},~{v33},~{s20},~{s21},~{vcc},~{scc},~{memory}\"",
        "(i32 %ptr_lo, i32 %ptr_hi, i32 %len_lo, i32 %len_hi, i32 %a, i32 %b, i32 %c, i32 %idx_lo, i32 %idx_hi, i32 %selector)"
    )));
    assert_eq!(calls[0].matches("{s22}").count(), 1);
    assert_eq!(calls[0].matches("i32 %selector").count(), 1);
    for absent in [
        "~{exec}",
        "~{s22}",
        "{s23}",
        "naked",
        "module asm",
        "callbr",
        "asm goto",
    ] {
        assert!(!text.contains(absent), "{absent}");
    }
}

#[test]
fn ordinary_shell_retains_unsigned_index_pointer_length_and_fixed_target() {
    let emitted = render(&one());
    let text = emitted.llvm_ir();
    for line in [
        "target triple = \"amdgcn-amd-amdhsa\"",
        "  %ptr = ptrtoint ptr addrspace(1) %data to i64",
        "  %ptr_lo = trunc i64 %ptr to i32",
        "  %ptr_shift = lshr i64 %ptr, 32",
        "  %ptr_hi = trunc i64 %ptr_shift to i32",
        "  %len_lo = trunc i64 %length to i32",
        "  %len_shift = lshr i64 %length, 32",
        "  %len_hi = trunc i64 %len_shift to i32",
        "  %local32 = call i32 @llvm.amdgcn.workitem.id.x()",
        "  %group32 = call i32 @llvm.amdgcn.workgroup.id.x()",
        "  %local = zext i32 %local32 to i64",
        "  %group = zext i32 %group32 to i64",
        "  %base = mul i64 %group, 64",
        "  %index = add i64 %base, %local",
        "  %idx_lo = trunc i64 %index to i32",
        "  %idx_shift = lshr i64 %index, 32",
        "  %idx_hi = trunc i64 %idx_shift to i32",
        "  unreachable",
        "!0 = !{i32 64, i32 1, i32 1}",
    ] {
        assert_eq!(
            text.lines().filter(|actual| *actual == line).count(),
            1,
            "{line}"
        );
    }
    assert_eq!(text.matches("\"target-cpu\"=\"gfx942\"").count(), 1);
    assert_eq!(
        text.matches("\"target-features\"=\"-xnack,-wavefrontsize32,+wavefrontsize64\"")
            .count(),
        1
    );
    assert_eq!(
        text.matches("\"amdgpu-flat-work-group-size\"=\"64,64\"")
            .count(),
        1
    );
    assert!(text.contains(fe2o3_amd_target::PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1));
    assert!(!text.contains("getelementptr inbounds"));
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("declare "))
            .count(),
        2
    );
}

#[test]
fn zero_and_nonzero_edges_use_exact_s22_comparison_and_authored_order() {
    let emitted = render(&output_diamond());
    let lines: Vec<_> = emitted.assembly_template().lines().collect();
    let blocks: Vec<_> = emitted.blocks().collect();
    assert_eq!(
        blocks.iter().map(|row| row.label).collect::<Vec<_>>(),
        [Label(240), Label(17), Label(2), Label(4)]
    );
    let first = blocks[0];
    assert_eq!(
        first.terminator,
        Gfx942CompleteBodyLoweredTerminatorV1::BranchSelectorZero {
            zero_ordinal: 1,
            nonzero_ordinal: 2,
        }
    );
    assert_eq!(first.terminator_line_count, 3);
    let start = usize::from(first.terminator_line);
    assert_eq!(
        &lines[start..start + 3],
        [
            "s_cmp_eq_u32 s22, 0",
            "s_cbranch_scc1 .Lfe2o3_cb_${:uid}_b1",
            "s_branch .Lfe2o3_cb_${:uid}_b2",
        ]
    );
    for row in &blocks[1..3] {
        assert_eq!(
            row.terminator,
            Gfx942CompleteBodyLoweredTerminatorV1::Jump { target_ordinal: 3 }
        );
        assert_eq!(
            lines[usize::from(row.terminator_line)],
            "s_branch .Lfe2o3_cb_${:uid}_b3"
        );
    }
    assert_eq!(
        emitted
            .assembly_template()
            .matches("s_cmp_eq_u32 s22, 0")
            .count(),
        1
    );
}

#[test]
fn both_authored_terminals_go_to_one_separately_named_tail() {
    let emitted = render(&two_terminals());
    assert_eq!(
        emitted
            .blocks()
            .filter(|row| row.terminator == Gfx942CompleteBodyLoweredTerminatorV1::CompilerTail)
            .count(),
        2
    );
    assert_eq!(
        emitted
            .assembly_template()
            .matches("s_branch .Lfe2o3_cb_${:uid}_tail\n")
            .count(),
        2
    );
    assert_eq!(
        emitted
            .assembly_template()
            .matches(".Lfe2o3_cb_${:uid}_tail:\n")
            .count(),
        1
    );
    assert_eq!(emitted.blocks().count(), 3); // compiler tail is not a fourth authored block
    assert_eq!(emitted.compiler_tail().instruction_count, 9);
}

#[test]
fn exact_maximum_preserves_empty_blocks_v63_and_all_self_writes() {
    let plan = maximum();
    let emitted = render(&plan);
    assert_eq!(emitted.blocks().count(), 8);
    assert_eq!(emitted.instructions().count(), 16);
    assert_eq!(
        emitted
            .blocks()
            .filter(|row| row.instruction_count == 0)
            .count(),
        5
    );
    assert_eq!(
        emitted
            .assembly_template()
            .matches("v_mov_b32_e32 v63, v63\n")
            .count(),
        14
    );
    assert!(
        emitted
            .assembly_template()
            .contains("global_store_dword v[2:3], v63, off")
    );
    assert!(emitted.llvm_ir().contains("{v8},{v9},{v10}"));
    assert!(emitted.llvm_ir().contains("~{v60},~{v63}"));
}

#[test]
fn all_six_opcodes_and_unused_final_scratch_write_are_retained() {
    let steps = [
        OUT,
        SCRATCH,
        Instruction::Binary {
            opcode: Opcode::Add,
            destination: Destination::Output,
            left: Role::Output,
            right: Role::Scratch,
        },
        Instruction::Binary {
            opcode: Opcode::Subtract,
            destination: Destination::Scratch,
            left: Role::Input2,
            right: Role::Output,
        },
        Instruction::Binary {
            opcode: Opcode::And,
            destination: Destination::Output,
            left: Role::Scratch,
            right: Role::Input1,
        },
        Instruction::Binary {
            opcode: Opcode::Or,
            destination: Destination::Scratch,
            left: Role::Output,
            right: Role::Output,
        },
        Instruction::Binary {
            opcode: Opcode::Xor,
            destination: Destination::Output,
            left: Role::Input0,
            right: Role::Scratch,
        },
        Instruction::Move {
            destination: Destination::Output,
            source: Role::Output,
        },
        SCRATCH,
    ];
    let plan = check(
        &[block(0, &steps, Term::GuardedStoreOutputAndEnd)],
        registers(),
    )
    .unwrap();
    let emitted = render(&plan);
    let lines: Vec<_> = emitted.assembly_template().lines().collect();
    let observed: Vec<_> = emitted
        .instructions()
        .map(|row| lines[usize::from(row.assembly_line)])
        .collect();
    assert_eq!(
        observed,
        [
            "v_mov_b32_e32 v33, v34",
            "v_mov_b32_e32 v32, v35",
            "v_add_u32_e32 v33, v33, v32",
            "v_sub_u32_e32 v32, v36, v33",
            "v_and_b32_e32 v33, v32, v35",
            "v_or_b32_e32 v32, v33, v33",
            "v_xor_b32_e32 v33, v34, v32",
            "v_mov_b32_e32 v33, v33",
            "v_mov_b32_e32 v32, v35",
        ]
    );
}

#[test]
fn correspondence_partitions_all_assembly_lines_without_hidden_authored_work() {
    for plan in [
        one(),
        output_diamond(),
        scratch_diamond(),
        two_terminals(),
        maximum(),
    ] {
        let emitted = render(&plan);
        let lines: Vec<_> = emitted.assembly_template().lines().collect();
        let mut visited = vec![false; lines.len()];
        let mut mark = |line: usize| {
            assert!(!visited[line], "overlap at {line}");
            visited[line] = true;
        };
        for row in emitted.blocks() {
            mark(usize::from(row.label_line));
            assert_eq!(
                lines[usize::from(row.label_line)],
                format!(".Lfe2o3_cb_${{:uid}}_b{}:", row.ordinal)
            );
            for line in
                row.terminator_line..row.terminator_line + u16::from(row.terminator_line_count)
            {
                mark(usize::from(line));
            }
        }
        for row in emitted.instructions() {
            mark(usize::from(row.assembly_line));
            let original = plan.block(usize::from(row.block_ordinal)).unwrap();
            assert_eq!(
                row.descriptor,
                original.instructions[usize::from(row.instruction_in_block)].descriptor()
            );
        }
        let tail = emitted.compiler_tail();
        mark(usize::from(tail.label_line));
        for line in tail.first_instruction_line
            ..tail.first_instruction_line + u16::from(tail.instruction_count)
        {
            mark(usize::from(line));
        }
        assert!(visited.into_iter().all(|value| value));
    }
}

#[test]
fn assembly_template_is_exactly_newline_escaped_in_one_llvm_opaque_unit() {
    let emitted = render(&scratch_diamond());
    let escaped = emitted.assembly_template().replace('\n', "\\0A\\09");
    let call = emitted
        .llvm_ir()
        .lines()
        .find(|line| line.contains(" asm "))
        .unwrap();
    assert!(call.starts_with(&format!("  call void asm sideeffect \"{escaped}\", ")));
    assert!(emitted.assembly_template().contains("${:uid}"));
    assert!(!emitted.assembly_template().contains("checked_body"));
    assert!(!emitted.assembly_template().contains('"'));
}

#[test]
fn work_is_prepaid_and_refusal_preserves_accepted_prefix() {
    let plan = one();
    let symbol = Gfx942CompleteBodySymbolV1::new("k").unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(GFX942_COMPLETE_BODY_RENDER_WORK_V1);
    work.charge_work(1).unwrap();
    assert!(matches!(
        render_gfx942_complete_body_llvm_v1(&plan, symbol, &mut work),
        Err(Gfx942CompleteBodyEmissionErrorV1::Work(_))
    ));
    assert_eq!(work.work(), 1);
    assert_eq!(
        work.failed_work(),
        Some(GFX942_COMPLETE_BODY_RENDER_WORK_V1 + 1)
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(GFX942_COMPLETE_BODY_RENDER_WORK_V1 * 2);
    let first = render_gfx942_complete_body_llvm_v1(&plan, symbol, &mut work).unwrap();
    let second = render_gfx942_complete_body_llvm_v1(&plan, symbol, &mut work).unwrap();
    assert_eq!(first, second); // pure repeat rendering, not artifact/source replay authority
    assert_eq!(work.work(), GFX942_COMPLETE_BODY_RENDER_WORK_V1 * 2);
}

#[test]
fn symbols_are_bounded_identifiers_not_an_assembly_escape_hatch() {
    for invalid in [
        "", "0kernel", "a-b", "a.b", "a b", "a\nb", "a\0b", "é", "a\"b", "a\\b", "a$0", "a;ret",
        "@k", "\u{202e}",
    ] {
        assert_eq!(
            Gfx942CompleteBodySymbolV1::new(invalid),
            Err(Gfx942CompleteBodyEmissionErrorV1::Symbol)
        );
    }
    let too_long = "a".repeat(129);
    assert_eq!(
        Gfx942CompleteBodySymbolV1::new(&too_long),
        Err(Gfx942CompleteBodyEmissionErrorV1::Symbol)
    );
    for accepted in ["k", "_k0", "Kernel_123"] {
        assert_eq!(
            Gfx942CompleteBodySymbolV1::new(accepted).unwrap().as_str(),
            accepted
        );
    }
    let max_symbol = "k".repeat(128);
    let symbol = Gfx942CompleteBodySymbolV1::new(&max_symbol).unwrap();
    let emitted = render_gfx942_complete_body_llvm_v1(
        &maximum(),
        symbol,
        &mut CanonicalKernelIrWorkBudgetV1::new(GFX942_COMPLETE_BODY_RENDER_WORK_V1),
    )
    .unwrap();
    assert!(emitted.llvm_ir().contains(&format!("void @{max_symbol}(")));
}

#[test]
fn undefined_output_and_incomplete_merge_refuse_before_any_renderable_plan() {
    assert!(matches!(
        check(
            &[block(0, &[SCRATCH], Term::GuardedStoreOutputAndEnd)],
            registers()
        ),
        Err(BodyError::OutputNotDefined { .. })
    ));
    let undefined = check(
        &[
            block(
                0,
                &[],
                Term::BranchSelectorZero {
                    zero: Label(1),
                    nonzero: Label(2),
                },
            ),
            block(1, &[SCRATCH], Term::Jump(Label(3))),
            block(2, &[], Term::Jump(Label(3))),
            block(3, &[USE_SCRATCH], Term::GuardedStoreOutputAndEnd),
        ],
        registers(),
    );
    assert!(matches!(
        undefined,
        Err(BodyError::ReadBeforeDefinition { .. })
    ));
}

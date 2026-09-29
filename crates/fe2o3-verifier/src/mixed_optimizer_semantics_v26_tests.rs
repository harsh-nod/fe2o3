use super::*;
use crate::mixed_optimizer_refinement_v26::{Budget, SOURCE_LIMIT};
use fe2o3_kernel_analysis::check_canonical_kir_transition_v18;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Module,
    Operation as Instruction, Signature, StorageLayoutLimitsV1, Terminator, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_pliron::optimize_neutral_kernel_ir_integer_worklist_v18;

include!("mixed_optimizer_congruence_v27_tests.rs");

const LIMIT: usize = 100_000_000;
const LAYOUTS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};

#[test]
fn mixed_cfg_composition_is_emitted_with_explicit_local_and_initial_obligations() {
    let text = source(scalar_case(Constant::U32(0), BinaryOp::Add, false));
    assert_eq!(text.matches(cfg_trace::PRELUDE).count(), 1);
    assert!(text.contains("proof fn cfg_finite_trace_refinement_v26<N, O, E>"));
    assert!(text.contains("cfg_step_simulates_v26(step_n, step_o, related),"));
    assert!(text.contains("        related(n, o),"));
    assert!(text.contains("step_n(n).events == step_o(o).events"));
    assert!(text.contains("step_n(n).halted == step_o(o).halted"));
    assert!(text.contains("related(step_n(n).state, step_o(o).state)"));
    assert!(text.contains("proof fn cfg_trace_bound_v26<S, E>"));
    assert!(text.contains("proof fn block_simulation_0"));
    assert!(!text.contains("assume("));
    assert!(!text.contains("external_body"));
    assert!(text.contains("proof fn cfg_initial_relation_0_v26"));
    assert!(text.contains("proof fn cfg_block_step_refinement_0_v26"));
    assert!(text.contains("proof fn cfg_all_steps_refine_v26"));
    assert!(text.contains("proof fn cfg_function_trace_refinement_0_v26"));
    assert_eq!(text.matches("cfg_finite_trace_refinement_v26(").count(), 2);
    let theorem = text
        .split("proof fn cfg_function_trace_refinement_0_v26")
        .nth(1)
        .unwrap();
    let requirements = theorem.split(" ensures ").next().unwrap();
    assert!(requirements.contains("cfg_entry_n_0_v26(n, arguments)"));
    assert!(requirements.contains("cfg_entry_o_0_v26(o, arguments)"));
    assert!(!requirements.contains("cfg_related_v26"));
    assert!(!requirements.contains("cfg_step_simulates_v26"));
}

#[test]
fn mixed_cfg_graph_uses_independent_actual_definition_environments() {
    let text = source(scalar_case(Constant::U32(0), BinaryOp::Add, false));
    for (side, count) in [("n", 3), ("o", 1)] {
        let block = text
            .split(&format!("open spec fn cfg_block_{side}_0("))
            .nth(1)
            .unwrap();
        assert!(block.starts_with(&format!(
            "{PARAMETERS}) -> CfgStepV26<CfgStateV26, int>\n recommends base.len() == {count},"
        )));
        assert!(text.contains(&format!("proof fn cfg_step_frame_{side}_v26")));
        assert!(text.contains(&format!("open spec fn cfg_entry_{side}_0_v26")));
    }
    let output = text
        .split("open spec fn cfg_block_o_0(")
        .nth(1)
        .unwrap()
        .split("open spec fn cfg_frame_n_v26")
        .next()
        .unwrap();
    assert!(output.contains("events: seq![1int, o_final,base[0],]"));
    assert!(!output.contains("base[1]"));
    assert!(!output.contains("base[2]"));
}

fn swapping_loop() -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(0), ValueId(1)],
    });
    let mut body = BasicBlock::new(BlockId(1));
    body.parameters = vec![
        ValueDef::new(ValueId(3), ty.clone()),
        ValueDef::new(ValueId(4), ty.clone()),
    ];
    body.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(2),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(4), ValueId(3)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(3), ValueId(4)],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.parameters = vec![
        ValueDef::new(ValueId(5), ty.clone()),
        ValueDef::new(ValueId(6), ty.clone()),
    ];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(5), ValueId(6)],
    });
    let mut module = Module::new("mixed-cfg-swapping-loop");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(
            vec![ty.clone(), ty.clone(), Type::BOOL],
            vec![ty.clone(), ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, body, exit],
    ));
    module
}

#[test]
fn mixed_cfg_graph_installs_backedge_block_arguments_simultaneously() {
    let text = source(swapping_loop());
    assert_eq!(
        text.matches("let next = next.update(3, base[4]);\n let next = next.update(4, base[3]);")
            .count(),
        2
    );
    assert!(text.contains("if base[2] == 1int"));
    assert!(text.contains("pc: 1, values: next"));
    assert!(text.contains("pc: 2, values: next"));
    assert!(!text.contains("next.update(4, next[3])"));
}

#[test]
fn mixed_cfg_loop_relation_is_derived_from_uses_without_stale_parameter_premises() {
    let text = source(swapping_loop());
    let entry = text
        .split("open spec fn cfg_live_0_v26")
        .nth(1)
        .unwrap()
        .split("open spec fn cfg_live_1_v26")
        .next()
        .unwrap();
    for argument in 0..3 {
        assert!(entry.contains(&format!("base[{argument}] == optimized[{argument}]")));
    }
    for stale in 3..7 {
        assert!(!entry.contains(&format!("base[{stale}]")));
    }
    let body = text
        .split("open spec fn cfg_live_1_v26")
        .nth(1)
        .unwrap()
        .split("open spec fn cfg_live_2_v26")
        .next()
        .unwrap();
    for incoming in [2, 3, 4] {
        assert!(body.contains(&format!("base[{incoming}] == optimized[{incoming}]")));
    }
    for block in 0..3 {
        let proof = text
            .split(&format!("proof fn cfg_block_step_refinement_{block}_v26"))
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert!(proof.contains(&format!("block_simulation_{block}(base, initial, op);")));
        assert!(proof.contains("let base = n.values;"));
        assert!(proof.contains("let base = o.values;"));
        let requirements = proof.split(" ensures ").next().unwrap();
        assert!(!requirements.contains("cfg_step_n_v26"));
        assert!(!requirements.contains("cfg_step_o_v26"));
    }
}

#[test]
fn mixed_cfg_omitting_any_loop_or_external_equation_dependency_is_rejected() {
    for module in [swapping_loop(), reverse_blocks()] {
        with_optimized(module, |input, output, rows, budget| {
            budget.reserve_storage(SOURCE_LIMIT).unwrap();
            let floor = budget.storage();
            let mut out = Writer::new(budget).unwrap();
            let count = cfg_relation::check_each_omission(input, output, rows, &mut out).unwrap();
            assert!(count >= 3);
            drop(out);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn mixed_cfg_generation_has_exact_and_one_short_work_and_storage_limits() {
    for module in [swapping_loop(), reverse_blocks()] {
        with_optimized(module, |input, output, rows, _| {
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(SOURCE_LIMIT + 17).unwrap();
                let floor = budget.storage();
                let mut out = Writer::new(&mut budget).unwrap();
                let writer_retained = out.budget.storage() - floor;
                let result = generate(input, output, rows, &mut out);
                drop(out);
                assert_eq!(budget.storage(), floor + writer_retained);
                budget.release_storage(writer_retained).unwrap();
                assert_eq!(budget.storage(), floor);
                (result, budget.work(), budget.peak_storage())
            };
            let (baseline, work, storage) = run(LIMIT, LIMIT);
            assert_eq!(baseline.unwrap(), output.blocks().len());
            let (exact, exact_work, exact_storage) = run(work, storage);
            assert_eq!(exact.unwrap(), output.blocks().len());
            assert_eq!((exact_work, exact_storage), (work, storage));
            assert!(matches!(
                run(work - 1, storage).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(work, storage - 1).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
        });
    }
}

#[test]
fn mixed_cfg_edge_argument_rebinding_at_equal_count_is_rejected_before_generation() {
    with_optimized(swapping_loop(), |input, output, rows, budget| {
        let mut arguments = rows.edge_arguments.to_vec();
        let first = arguments[0].input;
        arguments[0].input = arguments[1].input;
        arguments[1].input = first;
        let changed = Rows {
            edge_arguments: &arguments,
            ..rows
        };
        assert!(check_canonical_kir_transition_v18(input, output, changed, budget).is_err());
    });
}

fn switching_case(typed: bool) -> Module {
    let ty = Type::Scalar(if typed {
        ScalarType::I8
    } else {
        ScalarType::U64
    });
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(if typed {
        Terminator::IntegerSwitch {
            selector: ValueId(0),
            cases: vec![
                fe2o3_kernel_ir::IntegerSwitchCase {
                    value: Constant::I8(-1),
                    target: BlockId(1),
                    arguments: vec![],
                },
                fe2o3_kernel_ir::IntegerSwitchCase {
                    value: Constant::I8(3),
                    target: BlockId(2),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(3),
            default_arguments: vec![],
        }
    } else {
        Terminator::Switch {
            selector: ValueId(0),
            cases: vec![
                fe2o3_kernel_ir::SwitchCase {
                    value: 3,
                    target: BlockId(1),
                    arguments: vec![],
                },
                fe2o3_kernel_ir::SwitchCase {
                    value: u64::MAX,
                    target: BlockId(2),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(3),
            default_arguments: vec![],
        }
    });
    let mut blocks = vec![entry];
    for ordinal in 1..=3 {
        let mut block = BasicBlock::new(BlockId(ordinal));
        block.terminator = Some(if ordinal == 3 {
            Terminator::Unreachable
        } else {
            Terminator::Return {
                values: vec![ValueId(0)],
            }
        });
        blocks.push(block);
    }
    let mut module = Module::new("mixed-cfg-switch");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(0)],
        blocks,
    ));
    module
}

#[test]
fn mixed_cfg_graph_retains_typed_switch_bits_default_edges_and_distinct_termination() {
    for typed in [false, true] {
        let text = source(switching_case(typed));
        assert!(text.contains(if typed {
            "if base[0] == 255int"
        } else {
            "if base[0] == 18446744073709551615int"
        }));
        assert!(text.contains("if base[0] == 3int"));
        assert!(text.contains("pc: 3, values: next"));
        assert!(text.contains("events: seq![1int, n_final,base[0],]"));
        assert!(text.contains("events: seq![2int, n_final]"));
        assert!(text.contains("events: seq![3int]"));
    }
}

fn constant(id: u32, value: Constant) -> Instruction {
    Instruction::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}
fn binary(id: u32, ty: Type, op: BinaryOp, lhs: u32, rhs: u32) -> Instruction {
    let mut results = vec![ValueDef::new(ValueId(id), ty)];
    if matches!(op, BinaryOp::Checked(_)) {
        results.push(ValueDef::new(
            ValueId(id + 1),
            Type::Scalar(ScalarType::Bool),
        ));
    }
    Instruction::new(
        results,
        OperationKind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}
fn scalar_case(neutral: Constant, operator: BinaryOp, left: bool) -> Module {
    let ty = neutral.ty();
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.push(constant(1, neutral));
    block.operations.push(binary(
        2,
        ty.clone(),
        operator,
        if left { 1 } else { 0 },
        if left { 0 } else { 1 },
    ));
    let returns = if matches!(operator, BinaryOp::Checked(_)) {
        vec![ty.clone(), Type::Scalar(ScalarType::Bool)]
    } else {
        vec![ty.clone()]
    };
    block.terminator = Some(Terminator::Return {
        values: if returns.len() == 2 {
            vec![ValueId(2), ValueId(3)]
        } else {
            vec![ValueId(2)]
        },
    });
    let mut module = Module::new("mixed-policy9-proof");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(vec![ty], returns),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

fn with_optimized<R>(
    module: Module,
    consume: impl FnOnce(&Inventory<'_>, &Inventory<'_>, Rows<'_>, &mut Budget<'_>) -> R,
) -> R {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.reserve_storage(17).unwrap();
    let (input_owner, input_storage) =
        Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget).unwrap();
    budget
        .reserve_storage(input_storage.retained_storage())
        .unwrap();
    let optimized =
        optimize_neutral_kernel_ir_integer_worklist_v18(&input_owner, LAYOUTS, &mut budget)
            .unwrap();
    budget
        .reserve_storage(optimized.storage().retained_storage())
        .unwrap();
    let (input, ai) = Inventory::derive_v18(&input_owner, &mut budget).unwrap();
    budget.reserve_storage(ai.retained_storage()).unwrap();
    let (output, bi) = Inventory::derive_v18(optimized.owner(), &mut budget).unwrap();
    budget.reserve_storage(bi.retained_storage()).unwrap();
    let (checked, _) = check_canonical_kir_transition_v18(
        &input,
        &output,
        optimized.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    assert!(!checked.grants_authority());
    drop(checked);
    consume(
        &input,
        &output,
        optimized.occurrences().candidate(),
        &mut budget,
    )
}

fn source(module: Module) -> String {
    with_optimized(module, |input, output, rows, budget| {
        budget.reserve_storage(SOURCE_LIMIT).unwrap();
        let mut out = Writer::new(budget).unwrap();
        let floor = out.budget.storage();
        let blocks = generate(input, output, rows, &mut out).unwrap();
        let text = out.finish().unwrap();
        assert_eq!(blocks, output.blocks().len());
        assert_eq!(budget.storage(), floor);
        crate::CanonicalGeneratedVerusProofInputV3::new(text.as_bytes().to_vec()).unwrap();
        text
    })
}

#[test]
fn actual_neutral_integer_outputs_emit_both_semantics_not_a_report_assertion() {
    for (zero, one, all) in [
        (Constant::U8(0), Constant::U8(1), Constant::U8(u8::MAX)),
        (Constant::I8(0), Constant::I8(1), Constant::I8(-1)),
        (Constant::U16(0), Constant::U16(1), Constant::U16(u16::MAX)),
        (Constant::I16(0), Constant::I16(1), Constant::I16(-1)),
        (Constant::U32(0), Constant::U32(1), Constant::U32(u32::MAX)),
        (Constant::I32(0), Constant::I32(1), Constant::I32(-1)),
        (Constant::U64(0), Constant::U64(1), Constant::U64(u64::MAX)),
        (Constant::I64(0), Constant::I64(1), Constant::I64(-1)),
    ] {
        for (operator, neutral) in [
            (BinaryOp::Add, zero.clone()),
            (BinaryOp::Subtract, zero.clone()),
            (BinaryOp::Multiply, one.clone()),
            (BinaryOp::BitAnd, all),
            (BinaryOp::BitOr, zero.clone()),
            (BinaryOp::BitXor, zero.clone()),
            (BinaryOp::Checked(CheckedBinaryOperator::Add), zero.clone()),
            (BinaryOp::Checked(CheckedBinaryOperator::Subtract), zero),
            (BinaryOp::Checked(CheckedBinaryOperator::Multiply), one),
        ] {
            for left in [false, true] {
                if left
                    && matches!(
                        operator,
                        BinaryOp::Subtract | BinaryOp::Checked(CheckedBinaryOperator::Subtract)
                    )
                {
                    continue;
                }
                let text = source(scalar_case(neutral.clone(), operator, left));
                assert!(text.contains("open spec fn block_0_n"));
                assert!(text.contains("open spec fn block_0_o"));
                assert!(text.contains("proof fn block_simulation_0"));
                assert!(text.contains(
                    "ensures block_0_n(base, initial, op) == block_0_o(base, initial, op)"
                ));
                assert!(!text.contains("assume("));
                assert!(!text.contains("external_body"));
                assert!(!text.contains("requires false"));
                if matches!(operator, BinaryOp::Checked(_)) {
                    assert!(text.contains("if n_trap1 { 1 } else { 0 }"));
                }
                if matches!(
                    operator,
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply
                ) {
                    assert!(text.contains("if n_trap1 { 1int } else { 0int }"));
                }
            }
        }
    }
}

fn reverse_blocks() -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(constant(1, Constant::U32(0)));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(1));
    exit.operations
        .push(binary(2, ty.clone(), BinaryOp::Add, 0, 3));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut middle = BasicBlock::new(BlockId(2));
    middle
        .operations
        .push(binary(3, ty.clone(), BinaryOp::Add, 1, 1));
    middle.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut module = Module::new("reverse-block-neutral-exposure");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(0)],
        vec![entry, exit, middle],
    ));
    module
}

#[test]
fn original_dominating_equations_are_retained_across_reverse_block_order() {
    let text = source(reverse_blocks());
    let middle = text
        .split("proof fn block_simulation_1")
        .nth(1)
        .unwrap()
        .split("ensures")
        .next()
        .unwrap();
    assert!(middle.contains("base[3] == (base[1] + base[1]) % 4294967296"));
    assert!(middle.contains("base[1] == 0"));
    assert!(text.contains("proof fn block_simulation_2"));
}

#[test]
fn original_occurrence_cannot_be_rebound_at_equal_row_count() {
    with_optimized(
        scalar_case(Constant::U32(0), BinaryOp::Add, false),
        |input, output, rows, budget| {
            let mut uses = rows.uses.to_vec();
            let original = uses[0].input;
            uses[0].input = fe2o3_kernel_ir::CanonicalKirUseCoordinateV1::OperationOperand {
                operation: input.operations()[1].coordinate,
                operand: 1,
            };
            assert_ne!(uses[0].input, original);
            let changed = Rows {
                uses: &uses,
                ..rows
            };
            assert!(check_canonical_kir_transition_v18(input, output, changed, budget).is_err());
        },
    );
}

#[test]
fn statement_source_work_refusal_is_typed_and_scratch_is_settled() {
    with_optimized(
        scalar_case(Constant::U32(0), BinaryOp::Add, false),
        |input, output, rows, _| {
            let mut work = Work::new(1);
            let mut budget = Budget::new(&mut work, LIMIT);
            budget.reserve_storage(SOURCE_LIMIT + 17).unwrap();
            let mut out = Writer::new(&mut budget).unwrap();
            let floor = out.budget.storage();
            assert!(matches!(
                generate(input, output, rows, &mut out),
                Err(Error::Resource(Resource::Work(_)))
            ));
            drop(out);
            assert_eq!(budget.storage(), floor);
        },
    );
}

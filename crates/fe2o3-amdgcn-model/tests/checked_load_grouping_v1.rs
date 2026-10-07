use std::collections::BTreeMap;
use std::fs;
use std::process::Command;

use fe2o3_amdgcn_model::{
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir as ordinary,
    lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_checked_load_grouping_v1 as grouped,
};
use fe2o3_kernel_ir::*;

const U32: Type = Type::Scalar(ScalarType::U32);

fn fixture(count: u32) -> Module {
    let mut parameters = Vec::new();
    let mut block = BasicBlock::new(BlockId(0));
    for i in 0..count {
        parameters.extend([
            Type::pointer(U32, AddressSpace::Global, AccessMode::ReadOnly),
            Type::BOOL,
            U32,
        ]);
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(100 + i), U32),
            OperationKind::GuardedLoad {
                pointer: ValueId(i * 3),
                predicate: ValueId(i * 3 + 1),
                fallback: ValueId(i * 3 + 2),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut function = Function::kernel_entry(
        "checked_loads",
        Signature::new(parameters, vec![]),
        (0..count * 3).map(ValueId).collect(),
        vec![block],
    );
    function
        .required_capabilities
        .insert(gfx950_xnack_minus_target_capability());
    function
        .required_capabilities
        .insert(TargetCapability::WaveWidth(WaveWidth::Wave64));
    let mut kernel = Kernel::new(
        "checked_loads",
        "checked_loads",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    kernel.required_capabilities = function.required_capabilities.clone();
    let mut module = Module::new("checked-load-grouping-v1");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn operations(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn constant(id: u32, value: Constant) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), value.ty()),
        OperationKind::Constant(value),
    )
}

fn execute_load_cfg(llvm: &str, count: u32, mask: u32) -> (Vec<u64>, Vec<u64>) {
    execute_load_cfg_with_inputs(llvm, count, mask, &[], &[])
}

// Execute only the fixture's emitted branch/load/phi and checked-address subset,
// refusing every unexpected instruction. Invalid addresses have no memory entry.
fn execute_load_cfg_with_inputs(
    llvm: &str,
    count: u32,
    mask: u32,
    input_overrides: &[(&str, u64)],
    extra_memory: &[(u64, u64)],
) -> (Vec<u64>, Vec<u64>) {
    let body = llvm
        .split_once("bb0:\n")
        .unwrap()
        .1
        .split_once("\n}")
        .unwrap()
        .0;
    let lines: Vec<_> = body.lines().map(str::trim).collect();
    let labels: BTreeMap<_, _> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, line)| line.strip_suffix(':').map(|label| (label, i + 1)))
        .collect();
    let mut values = BTreeMap::new();
    let mut memory = BTreeMap::new();
    let mut overflow_pairs = BTreeMap::new();
    for i in 0..count {
        let valid = mask & (1 << i) != 0;
        values.insert(format!("%arg{}", i * 3), u64::from(i));
        values.insert(format!("%arg{}.data", i * 3), u64::from(i));
        values.insert(format!("%arg{}.len", i * 3), u64::from(valid));
        values.insert(format!("%arg{}", i * 3 + 1), u64::from(valid));
        values.insert(format!("%arg{}", i * 3 + 2), 1_000 + u64::from(i));
        if valid {
            memory.insert(u64::from(i), 2_000 + u64::from(i));
        }
    }
    for &(name, value) in input_overrides {
        *values
            .get_mut(name)
            .expect("override names an actual input") = value;
    }
    for &(address, value) in extra_memory {
        assert!(
            memory.insert(address, value).is_none(),
            "duplicate memory address"
        );
    }
    let read = |token: &str, values: &BTreeMap<String, u64>| -> u64 {
        match token {
            "true" => 1,
            "false" => 0,
            _ if token.starts_with('%') => values[token],
            _ => token.parse().unwrap(),
        }
    };
    let mut pc = 0;
    let mut current = "bb0";
    let mut predecessor = "";
    let mut reads = Vec::new();
    let mut returned = false;
    for _ in 0..1_000 {
        let line = lines[pc];
        pc += 1;
        if line == "ret void" {
            returned = true;
            break;
        }
        if line.is_empty() {
            continue;
        }
        let target = if let Some(branch) = line.strip_prefix("br i1 ") {
            let (condition, destinations) = branch.split_once(", label %").unwrap();
            let (yes, no) = destinations.split_once(", label %").unwrap();
            Some(if read(condition, &values) != 0 {
                yes
            } else {
                no
            })
        } else {
            line.strip_prefix("br label %")
        };
        if let Some(target) = target {
            predecessor = current;
            current = target;
            pc = labels[target];
            continue;
        }
        let (result, expression) = line.split_once(" = ").expect("closed emitted CFG subset");
        if let Some(operands) =
            expression.strip_prefix("call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 ")
        {
            let (left, right) = operands
                .strip_suffix(')')
                .unwrap()
                .split_once(", i32 ")
                .unwrap();
            let (sum, overflow) = u32::try_from(read(left, &values))
                .unwrap()
                .overflowing_add(u32::try_from(read(right, &values)).unwrap());
            assert!(!values.contains_key(result), "duplicate SSA definition");
            assert!(
                overflow_pairs
                    .insert(result.to_owned(), (u64::from(sum), u64::from(overflow)))
                    .is_none(),
                "duplicate SSA definition"
            );
            continue;
        }
        let value = if let Some(operands) = expression.strip_prefix("and i1 ") {
            let (left, right) = operands.split_once(", ").unwrap();
            read(left, &values) & read(right, &values)
        } else if let Some(operands) = expression.strip_prefix("xor i1 ") {
            let (left, right) = operands.split_once(", ").unwrap();
            read(left, &values) ^ read(right, &values)
        } else if let Some(operands) = expression.strip_prefix("extractvalue { i32, i1 } ") {
            let (pair, index) = operands.split_once(", ").unwrap();
            let pair = overflow_pairs[pair];
            match index {
                "0" => pair.0,
                "1" => pair.1,
                _ => panic!("unknown overflow aggregate index"),
            }
        } else if let Some(operands) = expression.strip_prefix("add i64 ") {
            let (left, right) = operands.split_once(", ").unwrap();
            read(left, &values).wrapping_add(read(right, &values))
        } else if let Some(operands) = expression.strip_prefix("icmp ugt i64 ") {
            let (left, right) = operands.split_once(", ").unwrap();
            u64::from(read(left, &values) > read(right, &values))
        } else if let Some(operands) =
            expression.strip_prefix("getelementptr i8, ptr addrspace(1) ")
        {
            let (base, offset) = operands.split_once(", i64 ").unwrap();
            read(base, &values).wrapping_add(read(offset, &values))
        } else if let Some(operands) =
            expression.strip_prefix("getelementptr i32, ptr addrspace(1) ")
        {
            let (base, offset) = operands.split_once(", i32 ").unwrap();
            // LLVM sign-extends an i32 GEP index before scaling by sizeof(i32).
            let offset = i64::from(u32::try_from(read(offset, &values)).unwrap() as i32);
            read(base, &values).wrapping_add((offset as u64).wrapping_mul(4))
        } else if let Some(load) = expression.strip_prefix("load i32, ptr addrspace(1) ") {
            let (pointer, _) = load.split_once(", align ").unwrap();
            let address = read(pointer, &values);
            reads.push(address);
            *memory
                .get(&address)
                .expect("false guard accessed an invalid pointer")
        } else if let Some(phi) = expression.strip_prefix("phi i32 ") {
            phi.split(" ], [ ")
                .find_map(|incoming| {
                    let incoming = incoming.trim_start_matches("[ ").trim_end_matches(" ]");
                    let (value, label) = incoming.split_once(", %").unwrap();
                    (label == predecessor).then(|| read(value, &values))
                })
                .expect("phi has actual predecessor")
        } else {
            panic!("unexpected emitted instruction: {line}");
        };
        assert!(
            !overflow_pairs.contains_key(result),
            "duplicate SSA definition"
        );
        assert!(
            values.insert(result.to_owned(), value).is_none(),
            "duplicate SSA definition"
        );
    }
    assert!(
        returned,
        "emitted CFG exceeded the step limit without returning"
    );
    (
        (0..count)
            .map(|i| values[&format!("%v{}", 100 + i)])
            .collect(),
        reads,
    )
}

#[test]
#[should_panic(expected = "emitted CFG exceeded the step limit without returning")]
fn emitted_cfg_interpreter_rejects_nontermination_after_results_are_ready() {
    let llvm = grouped(&fixture(2)).unwrap();
    assert_eq!(llvm.matches("  ret void\n").count(), 1);
    let looping = llvm.replacen(
        "  ret void\n",
        "  br label %nonterminating\nnonterminating:\n  br label %nonterminating\n",
        1,
    );
    let _ = execute_load_cfg(&looping, 2, 3);
}

#[test]
fn all_256_masks_preserve_distinct_fallbacks_and_never_access_invalid_pointers() {
    let module = fixture(8);
    let original = ordinary(&module).unwrap();
    let optimized = grouped(&module).unwrap();
    for mask in 0..256 {
        assert_eq!(
            execute_load_cfg(&original, 8, mask),
            execute_load_cfg(&optimized, 8, mask)
        );
    }
}

#[test]
fn empty_inputs_take_fallbacks_without_any_load() {
    let result = execute_load_cfg(&grouped(&fixture(8)).unwrap(), 8, 0);
    assert_eq!(result.0, (1_000..1_008).collect::<Vec<_>>());
    assert!(result.1.is_empty());
}

fn slice_fixture() -> Module {
    let mut module = fixture(2);
    let loads = operations(&mut module).clone();
    operations(&mut module).clear();
    for (i, mut load) in loads.into_iter().enumerate() {
        let i = i as u32;
        module.functions[0].signature.parameters[i as usize * 3] =
            Type::slice(U32, AddressSpace::Global, AccessMode::ReadOnly);
        let id = 200 + i * 10;
        operations(&mut module).extend([
            constant(id, Constant::Index(0)),
            Operation::effect_free(
                ValueDef::new(
                    ValueId(id + 1),
                    Type::pointer(U32, AddressSpace::Global, AccessMode::ReadOnly),
                ),
                OperationKind::SliceData {
                    slice: ValueId(i * 3),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(id + 2), Type::INDEX),
                OperationKind::SliceLength {
                    slice: ValueId(i * 3),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(id + 3), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::GreaterThan,
                    lhs: ValueId(id + 2),
                    rhs: ValueId(id),
                },
            ),
        ]);
        let OperationKind::GuardedLoad {
            pointer, predicate, ..
        } = &mut load.kind
        else {
            unreachable!()
        };
        *pointer = ValueId(id + 1);
        *predicate = ValueId(id + 3);
        operations(&mut module).push(load);
    }
    module
}

#[test]
fn empty_and_mixed_length_slices_preserve_non_access() {
    let module = slice_fixture();
    let original = ordinary(&module).unwrap();
    let optimized = grouped(&module).unwrap();
    assert!(optimized.contains(".group_fast = load"));
    for mask in 0..4 {
        assert_eq!(
            execute_load_cfg(&original, 2, mask),
            execute_load_cfg(&optimized, 2, mask)
        );
    }
    assert!(execute_load_cfg(&optimized, 2, 0).1.is_empty());
}

#[test]
fn all_valid_fast_block_has_eight_loads_before_reconvergence() {
    let llvm = grouped(&fixture(8)).unwrap();
    let fast = llvm
        .split_once("checked_load_bb0_op0_fast:\n")
        .unwrap()
        .1
        .split_once("checked_load_bb0_op0_slow:\n")
        .unwrap()
        .0;
    assert_eq!(fast.matches(" = load ").count(), 8);
    assert_eq!(fast.matches("br ").count(), 1);
    assert_eq!(
        execute_load_cfg(&llvm, 8, 255).1,
        (0..8).collect::<Vec<_>>()
    );
}

#[test]
fn grouping_is_default_off_and_singletons_are_unchanged() {
    let module = fixture(2);
    let old = ordinary(&module).unwrap();
    assert!(!old.contains("checked_load_bb"));
    assert!(grouped(&module).unwrap().contains("checked_load_bb"));
    assert_eq!(ordinary(&module).unwrap(), old);
    for count in [0, 1] {
        assert_eq!(
            ordinary(&fixture(count)).unwrap(),
            grouped(&fixture(count)).unwrap()
        );
    }
}

#[test]
fn ninth_load_starts_a_separate_scalar_region() {
    let llvm = grouped(&fixture(9)).unwrap();
    assert_eq!(llvm.matches(".group_fast = load").count(), 8);
    assert!(llvm.contains("%v108.loaded = load"));
    assert!(llvm.contains("guarded_load_bb0_op8_merge:"));
}

fn checked_address_fixture() -> Module {
    let mut module = fixture(2);
    let ops = operations(&mut module);
    ops.splice(
        1..1,
        [
            constant(200, Constant::U32(1)),
            Operation::checked_binary(
                ValueDef::new(ValueId(201), U32),
                ValueDef::new(ValueId(202), Type::BOOL),
                CheckedBinaryOperator::Add,
                ValueId(200),
                ValueId(5),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(203), Type::BOOL),
                OperationKind::Unary {
                    op: UnaryOp::Not,
                    operand: ValueId(202),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(204), Type::BOOL),
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: ValueId(4),
                    rhs: ValueId(203),
                },
            ),
            Operation::effect_free(
                ValueDef::new(
                    ValueId(205),
                    Type::pointer(U32, AddressSpace::Global, AccessMode::ReadOnly),
                ),
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(201),
                },
            ),
        ],
    );
    let OperationKind::GuardedLoad {
        pointer, predicate, ..
    } = &mut ops[6].kind
    else {
        unreachable!()
    };
    *pointer = ValueId(205);
    *predicate = ValueId(204);
    module
}

#[test]
fn independent_checked_address_and_bounds_operations_can_be_crossed() {
    let llvm = grouped(&checked_address_fixture()).unwrap();
    assert!(llvm.contains(".group_fast = load"));
    let branch = llvm.find("br i1 %checked_load_bb0_op0.all1").unwrap();
    assert!(llvm.find("%v205 = getelementptr").unwrap() < branch);
    assert!(llvm.find("%v204 = and i1").unwrap() < branch);
    assert!(!llvm.contains("getelementptr inbounds"));
}

#[test]
fn checked_overflow_and_nonzero_gep_preserve_loads_and_fallbacks() {
    let module = checked_address_fixture();
    let original = ordinary(&module).unwrap();
    let optimized = grouped(&module).unwrap();
    for mask in 0..4 {
        for addend in [0_u32, 3, u32::MAX - 1, u32::MAX] {
            let overflow = addend == u32::MAX;
            let base = if overflow { u64::MAX } else { 0x1000 };
            let index = i64::from(addend.wrapping_add(1) as i32);
            let address = base.wrapping_add((index as u64).wrapping_mul(4));
            let second_valid = mask & 2 != 0 && !overflow;
            let extra_memory = if second_valid {
                vec![(address, 0x4321)]
            } else {
                vec![]
            };
            let overrides = [("%arg3", base), ("%arg5", u64::from(addend))];
            let a = execute_load_cfg_with_inputs(&original, 2, mask, &overrides, &extra_memory);
            let b = execute_load_cfg_with_inputs(&optimized, 2, mask, &overrides, &extra_memory);
            assert_eq!(a, b, "mask={mask}, addend={addend}");
            assert_eq!(
                b.0,
                vec![
                    if mask & 1 != 0 { 2_000 } else { 1_000 },
                    if second_valid {
                        0x4321
                    } else {
                        u64::from(addend)
                    }
                ]
            );
            let mut expected_reads = Vec::new();
            if mask & 1 != 0 {
                expected_reads.push(0);
            }
            if second_valid {
                expected_reads.push(address);
            }
            assert_eq!(b.1, expected_reads);
        }
    }
}

#[test]
fn scan_bound_does_not_hoist_unbounded_regions() {
    let mut module = fixture(2);
    operations(&mut module).splice(1..1, (200..456).map(|id| constant(id, Constant::U32(id))));
    assert_eq!(ordinary(&module).unwrap(), grouped(&module).unwrap());
}

#[test]
fn volatile_or_read_write_pointers_are_not_grouped() {
    let mut volatile = fixture(2);
    let OperationKind::GuardedLoad { access, .. } = &mut operations(&mut volatile)[1].kind else {
        unreachable!()
    };
    access.volatile = true;
    assert_eq!(ordinary(&volatile).unwrap(), grouped(&volatile).unwrap());
    let mut writable = fixture(2);
    writable.functions[0].signature.parameters[3] =
        Type::pointer(U32, AddressSpace::Global, AccessMode::ReadWrite);
    assert_eq!(ordinary(&writable).unwrap(), grouped(&writable).unwrap());
}

#[test]
fn loaded_value_dependencies_and_trapping_arithmetic_stop_grouping() {
    let mut fallback = fixture(2);
    let OperationKind::GuardedLoad {
        fallback: value, ..
    } = &mut operations(&mut fallback)[1].kind
    else {
        unreachable!()
    };
    *value = ValueId(100);
    assert_eq!(ordinary(&fallback).unwrap(), grouped(&fallback).unwrap());
    for (op, lhs) in [
        (BinaryOp::Add, ValueId(100)),
        (BinaryOp::Divide, ValueId(2)),
        (BinaryOp::Remainder, ValueId(2)),
        (BinaryOp::ShiftLeft, ValueId(2)),
    ] {
        let mut module = fixture(2);
        operations(&mut module).insert(
            1,
            Operation::effect_free(
                ValueDef::new(ValueId(200), U32),
                OperationKind::Binary {
                    op,
                    lhs,
                    rhs: ValueId(5),
                },
            ),
        );
        assert_eq!(ordinary(&module).unwrap(), grouped(&module).unwrap());
    }
}

#[test]
fn stores_and_unconditional_loads_stop_grouping() {
    let mut module = fixture(2);
    operations(&mut module).insert(
        1,
        Operation::effect_free(
            ValueDef::new(ValueId(200), U32),
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    );
    assert_eq!(ordinary(&module).unwrap(), grouped(&module).unwrap());
    module.functions[0].signature.parameters[0] =
        Type::pointer(U32, AddressSpace::Global, AccessMode::ReadWrite);
    operations(&mut module)[1] = Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(0),
            value: ValueId(2),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    );
    assert_eq!(ordinary(&module).unwrap(), grouped(&module).unwrap());
}

fn refresh_capabilities(module: &mut Module) {
    let derived = module.functions[0].derived_capabilities();
    module.functions[0].required_capabilities.extend(derived);
    module.kernels[0].required_capabilities = module.functions[0].required_capabilities.clone();
    module.required_capabilities = module.functions[0].required_capabilities.clone();
}

fn collective_fixture(at_end: bool) -> Module {
    let mut module = fixture(2);
    operations(&mut module).insert(
        if at_end { 2 } else { 1 },
        Operation::effect_free(
            ValueDef::new(ValueId(200), Type::Scalar(ScalarType::U64)),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::Ballot {
                    predicate: ValueId(1),
                },
                WaveWidth::Wave64,
            )),
        ),
    );
    refresh_capabilities(&mut module);
    module
}

#[test]
fn collectives_are_never_crossed_and_follow_full_reconvergence() {
    let between = collective_fixture(false);
    assert_eq!(ordinary(&between).unwrap(), grouped(&between).unwrap());
    let llvm = grouped(&collective_fixture(true)).unwrap();
    assert!(
        llvm.find("%v101 = phi i32").unwrap()
            < llvm.find("call i64 @llvm.amdgcn.ballot.i64").unwrap()
    );
}

#[test]
fn workgroup_barrier_is_a_hard_boundary() {
    let mut module = fixture(2);
    operations(&mut module).insert(
        1,
        Operation::new(
            vec![],
            OperationKind::WorkgroupBarrier(WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            }),
        ),
    );
    refresh_capabilities(&mut module);
    assert_eq!(ordinary(&module).unwrap(), grouped(&module).unwrap());
}

fn successor_fixture() -> Module {
    let mut module = fixture(2);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(101)],
    });
    let mut next = BasicBlock::new(BlockId(1));
    next.parameters.push(ValueDef::new(ValueId(200), U32));
    next.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(next);
    module
}

#[test]
fn successor_phi_names_the_actual_final_group_merge() {
    let llvm = grouped(&successor_fixture()).unwrap();
    assert!(llvm.contains("%v200 = phi i32 [ %v101, %guarded_load_bb0_op1_merge ]"));
    assert_eq!(llvm.matches("guarded_load_bb0_op1_merge:\n").count(), 1);
}

fn loop_fixture() -> Module {
    let mut module = fixture(2);
    let loads = operations(&mut module).clone();
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.clear();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(2)],
    });
    let mut body_block = BasicBlock::new(BlockId(1));
    body_block.parameters.push(ValueDef::new(ValueId(200), U32));
    body_block.operations = loads;
    body_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(1),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(101)],
        else_target: BlockId(2),
        else_arguments: vec![ValueId(100)],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.parameters.push(ValueDef::new(ValueId(201), U32));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([body_block, exit]);
    module
}

#[test]
fn grouped_loop_exit_and_backedge_retain_phi_edges() {
    let llvm = grouped(&loop_fixture()).unwrap();
    assert_eq!(llvm.matches("guarded_load_bb1_op1_merge:\n").count(), 1);
    assert!(llvm.contains("%v200 = phi i32"));
    assert!(llvm.contains("%v201 = phi i32 [ %v100, %guarded_load_bb1_op1_merge ]"));
}

#[test]
fn anchored_entry_keeps_original_per_operation_markers() {
    let module = fixture(2);
    let owner = VerifiedCanonicalKernelIrV11::from_module(module.clone()).unwrap();
    let llvm = fe2o3_amdgcn_model::lower_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(
        &module, fe2o3_amdgcn_model::ProductionSemanticAnchorKirIdentityV1::from_v11(&owner),
    ).unwrap();
    assert!(!llvm.contains("checked_load_bb"));
    assert_eq!(llvm.matches("call void @llvm.pseudoprobe(").count(), 2);
}

#[test]
fn llvm_assembler_validates_grouped_and_successor_cfgs() {
    let directory = std::env::temp_dir().join(format!(
        "fe2o3-checked-load-grouping-{}",
        std::process::id()
    ));
    fs::create_dir(&directory).unwrap();
    let assembler =
        std::env::var_os("FE2O3_CHECKED_LOAD_LLVM_AS").unwrap_or_else(|| "llvm-as".into());
    for (index, module) in [
        fixture(8),
        fixture(9),
        successor_fixture(),
        loop_fixture(),
        slice_fixture(),
        collective_fixture(true),
        checked_address_fixture(),
    ]
    .iter()
    .enumerate()
    {
        let source = directory.join(format!("{index}.ll"));
        fs::write(&source, grouped(module).unwrap()).unwrap();
        let output = Command::new(&assembler)
            .arg(source)
            .arg("-o")
            .arg(directory.join(format!("{index}.bc")))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

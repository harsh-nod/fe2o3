use super::*;
use fe2o3_kernel_ir::{
    Atomic, AtomicKind, Barrier, BarrierSemantics, MemoryOrdering, SynchronizationScope,
};

fn excluded(input: &Module, allocation: Coordinate) {
    let (output, rows) = candidate(input, &[], &[]);
    accept(input, &output, &[], &rows);
    refuse(
        input,
        &output,
        &[allocation],
        &rows,
        "eligible selected allocation",
    );
}
fn global_argument(input: &mut Module) {
    input.functions[0].signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    input.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(ValueId(4));
}

#[test]
fn trapping_stored_computation_and_external_suffix_effect_are_exactly_retained() {
    let mut input = fixture(ScalarType::U32, None);
    global_argument(&mut input);
    ops(&mut input).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::Scalar(ScalarType::U32)),
            Kind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
    );
    if let Kind::Store { value, .. } = &mut ops(&mut input)[5].kind {
        *value = ValueId(21);
    }
    ops(&mut input).push(Operation::new(
        vec![],
        Kind::Store {
            pointer: ValueId(4),
            value: ValueId(14),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let (output, rows) = candidate(
        &input,
        &[coord(1), coord(3), coord(5), coord(6)],
        &[(coord(7), coord(1), coord(6), ValueId(1))],
    );
    accept(&input, &output, &[coord(1)], &rows);
    assert_eq!(
        output.functions[0].body.as_ref().unwrap().blocks[0].operations[2],
        input.functions[0].body.as_ref().unwrap().blocks[0].operations[4]
    );
    let (bad, bad_rows) = candidate(
        &input,
        &[coord(1), coord(3), coord(4), coord(5), coord(6)],
        &[(coord(7), coord(1), coord(6), ValueId(1))],
    );
    refuse(
        &input,
        &bad,
        &[coord(1)],
        &bad_rows,
        "retained operation payload",
    );
    let mut bad = output.clone();
    if let Kind::Store { value, .. } = &mut ops(&mut bad)[4].kind {
        *value = ValueId(0);
    }
    refuse(
        &input,
        &bad,
        &[coord(1)],
        &rows,
        "retained operation payload",
    );
    let mut bad = output;
    ops(&mut bad).swap(2, 3);
    let mut bad_rows = rows;
    bad_rows.swap(2, 3);
    refuse(
        &input,
        &bad,
        &[coord(1)],
        &bad_rows,
        "retained operation payload",
    );
}

#[test]
fn known_distinct_allocation_can_interleave_without_aliasing_cells() {
    let mut input = fixture(ScalarType::U32, None);
    let ty = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(ty.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    // Allocate before either interval, then interleave the second cell's Store/Load.
    ops(&mut input).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(20), pointer),
            Kind::Alloca {
                element: ty.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
    );
    ops(&mut input).insert(
        6,
        Operation::new(
            vec![],
            Kind::Store {
                pointer: ValueId(20),
                value: ValueId(0),
                access,
            },
        ),
    );
    ops(&mut input).insert(
        8,
        Operation::effect_free(
            ValueDef::new(ValueId(21), ty),
            Kind::Load {
                pointer: ValueId(20),
                access,
            },
        ),
    );
    let (output, rows) = candidate(
        &input,
        &[coord(1), coord(3), coord(4), coord(5), coord(6), coord(7)],
        &[
            (coord(8), coord(4), coord(6), ValueId(0)),
            (coord(9), coord(1), coord(7), ValueId(1)),
        ],
    );
    accept(&input, &output, &[coord(1), coord(4)], &rows);
    let (subset, subset_rows) = candidate(
        &input,
        &[coord(1), coord(3), coord(5), coord(7)],
        &[(coord(9), coord(1), coord(7), ValueId(1))],
    );
    accept(&input, &subset, &[coord(1)], &subset_rows);
}

#[test]
fn guarded_nested_and_address_escape_uses_refuse_whole_selection() {
    let mut input = fixture(ScalarType::U32, None);
    let Kind::Load { pointer, access } = ops(&mut input)[6].kind else {
        unreachable!()
    };
    ops(&mut input)[6].kind = Kind::GuardedLoad {
        pointer,
        predicate: ValueId(3),
        fallback: ValueId(0),
        access,
    };
    excluded(&input, coord(1));
    let mut input = fixture(ScalarType::U32, None);
    let pointer = ops(&mut input)[1].results[0].ty.clone();
    ops(&mut input).insert(
        4,
        Operation::effect_free(
            ValueDef::new(ValueId(15), pointer.clone()),
            Kind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(12),
            },
        ),
    );
    excluded(&input, coord(1));
    let mut input = fixture(ScalarType::U32, None);
    ops(&mut input).push(Operation::new(
        vec![],
        Kind::Call {
            callee: "escape".into(),
            arguments: vec![ValueId(13)],
        },
    ));
    input.functions.push(Function::declaration(
        "escape",
        Signature::new(vec![pointer.clone()], vec![]),
    ));
    excluded(&input, coord(1));
    let mut input = fixture(ScalarType::U32, None);
    input.functions[0].signature.results = vec![pointer.clone()];
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Return {
        values: vec![ValueId(11)],
    });
    excluded(&input, coord(1));
    let mut input = fixture(ScalarType::U32, None);
    let mut second = BasicBlock::new(BlockId(72));
    second.parameters = vec![ValueDef::new(ValueId(20), pointer)];
    second.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(72),
        arguments: vec![ValueId(13)],
    });
    input.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(second);
    excluded(&input, coord(1));
}

#[test]
fn unknown_effects_and_ordering_inside_interval_are_not_transparent() {
    for mode in 0..4 {
        let mut input = fixture(ScalarType::U32, None);
        global_argument(&mut input);
        let operation = match mode {
            0 => Operation::new(
                vec![],
                Kind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                }),
            ),
            1 => Operation::new(
                vec![],
                Kind::Atomic(Atomic {
                    kind: AtomicKind::Store,
                    pointer: ValueId(4),
                    value: Some(ValueId(0)),
                    compare: None,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                    scope: SynchronizationScope::Device,
                    ordering: MemoryOrdering::Relaxed,
                    failure_ordering: None,
                }),
            ),
            2 => Operation::effect_free(
                ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32)),
                Kind::Load {
                    pointer: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            _ => {
                let mut block = BasicBlock::new(BlockId(0));
                block.terminator = Some(Terminator::Return { values: vec![] });
                input.functions.push(Function::internal_helper(
                    "other",
                    Signature::new(vec![], vec![]),
                    vec![],
                    vec![block],
                ));
                Operation::new(
                    vec![],
                    Kind::Call {
                        callee: "other".into(),
                        arguments: vec![],
                    },
                )
            }
        };
        ops(&mut input).insert(5, operation);
        excluded(&input, coord(1));
    }
}

#[test]
fn unreachable_uses_and_multiblock_accesses_are_not_silently_filtered() {
    let mut input = fixture(ScalarType::U32, None);
    let load = ops(&mut input).pop().unwrap();
    let mut next = BasicBlock::new(BlockId(72));
    next.operations.push(load);
    next.terminator = Some(Terminator::Return {
        values: vec![ValueId(14)],
    });
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(72),
        arguments: vec![],
    });
    input.functions[0].body.as_mut().unwrap().blocks.push(next);
    excluded(&input, coord(1));
    let mut input = fixture(ScalarType::U32, None);
    let pointer = ops(&mut input)[1].results[0].ty.clone();
    let mut dead = input.functions[0].body.as_mut().unwrap().blocks.remove(0);
    dead.id = BlockId(72);
    dead.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(20), pointer),
        Kind::GetElementPointer {
            base: ValueId(11),
            offset: ValueId(2),
        },
    ));
    dead.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    let mut entry = BasicBlock::new(BlockId(91));
    entry.terminator = Some(Terminator::Return {
        values: vec![ValueId(0)],
    });
    input.functions[0].body.as_mut().unwrap().blocks = vec![entry, dead];
    excluded(&input, location(0, 1, 1));
}

#[test]
fn duplicate_cfg_edges_and_their_values_are_preserved_exactly() {
    let mut input = fixture(ScalarType::U32, None);
    let mut exit = BasicBlock::new(BlockId(72));
    exit.parameters = vec![ValueDef::new(ValueId(20), Type::Scalar(ScalarType::U32))];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });
    input.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::ConditionalBranch {
            condition: ValueId(3),
            then_target: BlockId(72),
            then_arguments: vec![ValueId(14)],
            else_target: BlockId(72),
            else_arguments: vec![ValueId(0)],
        });
    input.functions[0].body.as_mut().unwrap().blocks.push(exit);
    let (output, rows) = promoted(&input);
    accept(&input, &output, &[coord(1)], &rows);
    let mut bad = output;
    if let Some(Terminator::ConditionalBranch { else_arguments, .. }) =
        &mut bad.functions[0].body.as_mut().unwrap().blocks[0].terminator
    {
        *else_arguments = vec![ValueId(14)];
    }
    refuse(&input, &bad, &[coord(1)], &rows, "block payload");
}

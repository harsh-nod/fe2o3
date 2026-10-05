use super::*;
use fe2o3_kernel_ir::{
    Barrier, BarrierSemantics, CastKind, MemoryOrdering, StorageCopyOverlapV1, StorageFieldV1,
    StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1, StorageOperationV1 as Storage,
    StorageProjectionV1 as Projection, SynchronizationScope,
};

fn call(result: u32, callee: &str, argument: u32) -> Operation {
    value(
        result,
        Type::INDEX,
        Kind::Call {
            callee: callee.into(),
            arguments: vec![ValueId(argument)],
        },
    )
}

fn helper(name: &str, operations: Vec<Operation>, returned: u32) -> IrFunction {
    let mut block = BasicBlock::new(BlockId(900));
    block.operations = operations;
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(returned)],
    });
    IrFunction::internal_helper(
        name,
        Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
        vec![ValueId(100)],
        vec![block],
    )
}

fn helper_module(varying_argument: bool, varying_helper: bool) -> Module {
    let mut root = tile_block(7, 4);
    root.operations
        .insert(0, call(4, "outer", if varying_argument { 3 } else { 1 }));
    root.operations.insert(0, index(3, IndexKind::Local));
    let mut module = module(vec![root]);
    // Outer precedes inner in physical order and calls it twice.
    module.functions.push(helper(
        "outer",
        vec![call(101, "inner", 100), call(102, "inner", 101)],
        102,
    ));
    module.functions.push(helper(
        "inner",
        if varying_helper {
            vec![index(101, IndexKind::Local)]
        } else {
            vec![value(
                101,
                Type::INDEX,
                Kind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(100),
                    rhs: ValueId(100),
                },
            )]
        },
        101,
    ));
    module
}

#[test]
fn tile_convergence_summarizes_defined_calls_without_assuming_uniform_helper_arguments() {
    check(&helper_module(false, false)).unwrap();
    for input in [helper_module(true, false), helper_module(false, true)] {
        assert!(matches!(check(&input), Err(Error::VaryingTileInput(_))));
    }
}

#[test]
fn tile_convergence_refuses_recursive_cyclic_and_collective_defined_calls() {
    for kind in 0..5 {
        let mut input = helper_module(false, false);
        let body = input.functions[2].body.as_mut().unwrap();
        match kind {
            0 => body.blocks[0].operations = vec![call(101, "inner", 100)],
            1 => body.blocks[0].operations = vec![call(101, "outer", 100)],
            2 => {
                body.blocks[0].terminator = Some(Terminator::Branch {
                    target: BlockId(900),
                    arguments: vec![],
                })
            }
            3 => body.blocks[0].terminator = Some(Terminator::Unreachable),
            _ => body.blocks[0].operations.push(Operation::new(
                vec![],
                Kind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Workgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                }),
            )),
        }
        assert!(
            matches!(check(&input), Err(Error::UnsupportedArrival(_))),
            "case {kind}"
        );
    }
}

#[test]
fn tile_convergence_transitive_opaque_calls_refuse_but_dead_calls_do_not() {
    let mut input = helper_module(false, false);
    input.functions.push(IrFunction::external_import(
        "opaque",
        Signature::new(vec![Type::INDEX], vec![Type::INDEX]),
    ));
    let mut dead = BasicBlock::new(BlockId(801));
    dead.operations.push(call(150, "opaque", 100));
    dead.terminator = Some(Terminator::Return {
        values: vec![ValueId(150)],
    });
    input.functions[2].body.as_mut().unwrap().blocks.push(dead);
    check(&input).unwrap();
    input.functions[2].body.as_mut().unwrap().blocks[0].operations = vec![call(101, "opaque", 100)];
    assert!(matches!(check(&input), Err(Error::UnsupportedArrival(_))));
}

fn storage_module(read_base: bool) -> Module {
    let pointer = |layout| {
        Type::pointer(
            Type::StorageObject(StorageLayoutIdV1(layout)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        )
    };
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let mut root = tile_block(7, if read_base { 34 } else { 1 });
    root.operations.splice(
        0..0,
        [
            value(
                30,
                pointer(1),
                Kind::Alloca {
                    element: Type::StorageObject(StorageLayoutIdV1(1)),
                    count: None,
                    address_space: AddressSpace::Private,
                    alignment: 4,
                },
            ),
            value(
                31,
                pointer(0),
                Kind::Storage(Storage::Project {
                    base: ValueId(30),
                    step: Projection::Field(0),
                }),
            ),
            value(
                32,
                Type::Scalar(ScalarType::U32),
                Kind::Constant(Constant::U32(0)),
            ),
            Operation::new(
                vec![],
                Kind::Storage(Storage::WriteValue {
                    address: ValueId(31),
                    value: ValueId(32),
                    access,
                }),
            ),
            Operation::new(
                vec![],
                Kind::Storage(Storage::CopyObject {
                    source: ValueId(31),
                    destination: ValueId(31),
                    source_access: access,
                    destination_access: access,
                    overlap: StorageCopyOverlapV1::MayOverlap,
                }),
            ),
            value(
                33,
                Type::Scalar(ScalarType::U32),
                Kind::Storage(Storage::ReadValue {
                    address: ValueId(31),
                    access,
                }),
            ),
            value(
                34,
                Type::INDEX,
                Kind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(33),
                    to: Type::INDEX,
                },
            ),
        ],
    );
    let mut input = module(vec![root]);
    input.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
    ];
    input
}

#[test]
fn tile_convergence_storage_transfers_preserve_arrival_and_taint_reads() {
    check(&storage_module(false)).unwrap();
    assert!(matches!(
        check(&storage_module(true)),
        Err(Error::VaryingTileInput(_))
    ));
}

#[test]
fn tile_convergence_summarized_calls_have_exact_and_one_short_resources() {
    for input in [helper_module(false, false), storage_module(false)] {
        with_inventory(&input, |inventory| {
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(31).unwrap();
                let result =
                    check_canonical_tile_convergence_v160(inventory, Function(0), &mut budget);
                assert_eq!(budget.storage(), 31);
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, storage) = run(usize::MAX, usize::MAX);
            result.unwrap();
            assert_eq!(run(work, storage), (Ok(()), work, storage));
            assert!(matches!(run(work - 1, storage).0,
                Err(Error::Resource(Resource::Work(error))) if error.actual() == work && error.limit() == work - 1));
            assert!(matches!(run(work, storage - 1).0,
                Err(Error::Resource(Resource::Storage(error))) if error.actual() == storage && error.limit() == storage - 1));
        });
    }
}

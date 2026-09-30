use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    Constant, ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role, Function, Kernel,
    LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind as Op, ScalarType,
    Signature, StorageFieldV1, StorageLayoutIdV1 as Id, StorageLayoutKindV1 as Layout,
    StorageLayoutV1, StorageOperationV1 as Storage, StorageProjectionV1 as Projection, Terminator,
    Type, ValueDef, ValueId,
};

pub(crate) const WORK: usize = 10_000_000_000_000;
pub(crate) const SPACE: usize = 2_000_000_000;
pub(crate) const LIMITS: StorageLayoutLimitsV1 = StorageLayoutLimitsV1 {
    rows: 64,
    edges: 256,
    containment_depth: 32,
    object_bytes: 4096,
};
const U32: Type = Type::Scalar(ScalarType::U32);
pub(crate) fn constant(id: u32, n: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), U32),
        Op::Constant(Constant::U32(n)),
    )
}
pub(crate) fn add(id: u32, a: u32, b: u32) -> Operation {
    binary(id, a, b, BinaryOp::Add)
}
fn xor(id: u32, a: u32, b: u32) -> Operation {
    binary(id, a, b, BinaryOp::BitXor)
}
fn binary(id: u32, a: u32, b: u32, op: BinaryOp) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), U32),
        Op::Binary {
            op,
            lhs: ValueId(a),
            rhs: ValueId(b),
        },
    )
}
pub(crate) fn fixture() -> Module {
    let mut module = Module::new("v18-observed-scalar");
    module.storage_layouts = vec![
        StorageLayoutV1 {
            size: 4,
            alignment: 4,
            kind: Layout::Scalar(ScalarType::U32),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 4,
            kind: Layout::Record(
                vec![
                    StorageFieldV1 {
                        offset: 0,
                        layout: Id(0),
                    },
                    StorageFieldV1 {
                        offset: 4,
                        layout: Id(0),
                    },
                ]
                .into_boxed_slice(),
            ),
        },
    ];
    let pointer = |row| {
        Type::pointer(
            Type::StorageObject(Id(row)),
            AddressSpace::Private,
            AccessMode::ReadWrite,
        )
    };
    let access = MemoryAccess::new(AddressSpace::Private, 4);
    let store = |value| {
        Operation::new(
            vec![],
            Op::Storage(Storage::WriteValue {
                address: ValueId(11),
                value: ValueId(value),
                access,
            }),
        )
    };
    let read = |id| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), U32),
            Op::Storage(Storage::ReadValue {
                address: ValueId(11),
                access,
            }),
        )
    };
    let mut block = BasicBlock::new(BlockId(91));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(20), Type::Execution(Role::Context)),
            Op::Execution(Execution::ContextIssue),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(21), Type::Execution(Role::Workgroup)),
            Op::Execution(Execution::WorkgroupDerive {
                context: ValueId(20),
            }),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(10), pointer(1)),
            Op::Alloca {
                element: Type::StorageObject(Id(1)),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(11), pointer(0)),
            Op::Storage(Storage::Project {
                base: ValueId(10),
                step: Projection::Field(0),
            }),
        ),
        constant(3, 7),
        constant(4, 9),
        add(5, 3, 4),
        xor(6, 0, 1),
        xor(7, 0, 1),
        Operation::effect_free(
            ValueDef::new(ValueId(8), U32),
            Op::Select {
                condition: ValueId(2),
                true_value: ValueId(6),
                false_value: ValueId(6),
            },
        ),
        store(5),
        store(8),
        read(12),
        read(13),
        add(14, 12, 7),
        store(14),
        constant(15, 99),
        Operation::new(
            vec![],
            Op::Execution(Execution::ScopeEnd {
                workgroup: ValueId(21),
                discarded: vec![],
            }),
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![U32, U32, Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "entry",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    module
}

pub(crate) struct Input {
    pub(crate) owner: Owner,
    pub(crate) storage: usize,
}
pub(crate) fn input(module: &Module) -> Input {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v18(module, LIMITS, &mut budget).unwrap();
    assert_eq!(budget.storage(), 0);
    Input {
        owner,
        storage: storage.retained_storage(),
    }
}
pub(crate) fn observe<'a>(
    input: &'a Input,
    budget: &mut Budget<'_>,
) -> KirNeutralOptimizationOutputV18<'a> {
    let floor = budget.storage();
    let output = optimize_neutral_kernel_ir_v18(&input.owner, LIMITS, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(output.storage().retained_storage())
        .unwrap();
    output
}

#[test]
fn actual_fixed_execution_preserves_storage_and_lifecycle_while_rewriting_scalars() {
    let input = input(&fixture());
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(37 + input.storage).unwrap();
    let observed = observe(&input, &mut budget);
    assert!(std::ptr::eq(observed.input(), &input.owner));
    assert_ne!(
        observed.owner().canonical_bytes(),
        input.owner.canonical_bytes()
    );
    assert_eq!(
        observed.owner().module().storage_layouts,
        input.owner.module().storage_layouts
    );
    assert_eq!(
        observed
            .report()
            .passes()
            .iter()
            .map(|p| p.pass())
            .collect::<Vec<_>>(),
        crate::fixed_policy_v3::POLICY3_PASSES
    );
    assert!(observed.map().matches_execution(observed.report()));
    let before = &input.owner.module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations;
    let after = &observed.owner().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations;
    assert!(after.len() < before.len());
    assert!(
        after
            .iter()
            .any(|op| matches!(op.kind, Op::Constant(Constant::U32(16))))
    );
    // An ordinary Add can synthesize a folded result, but is deliberately not
    // in the total-operation DCE/CSE allowance.
    let adds = |operations: &[Operation]| {
        operations
            .iter()
            .filter(|op| {
                matches!(
                    op.kind,
                    Op::Binary {
                        op: BinaryOp::Add,
                        ..
                    }
                )
            })
            .count()
    };
    assert_eq!(adds(before), 2);
    assert_eq!(adds(after), 2);
    assert_eq!(
        after
            .iter()
            .filter(|op| matches!(
                op.kind,
                Op::Binary {
                    op: BinaryOp::BitXor,
                    ..
                }
            ))
            .count(),
        1
    );
    for predicate in [
        (|op: &Operation| matches!(op.kind, Op::Storage(_))) as fn(&Operation) -> bool,
        |op: &Operation| matches!(op.kind, Op::Execution(_)),
    ] {
        assert_eq!(
            before.iter().filter(|op| predicate(op)).count(),
            after.iter().filter(|op| predicate(op)).count()
        );
    }
    let checked = observed.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), 37 + input.storage);
    assert_eq!(checked.input_audit_bytes(), input.owner.canonical_bytes());
    assert_eq!(checked.execution().graph_schema(), 18);
    assert_eq!(checked.execution().policy_version(), 3);
    assert!(!checked.grants_authority());
}

pub(crate) fn diamond(looping: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![xor(3, 1, 2), constant(4, 0)];
    let mut left = BasicBlock::new(BlockId(1));
    let mut right = BasicBlock::new(BlockId(2));
    if looping {
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(3)],
        });
        left.parameters = vec![ValueDef::new(ValueId(10), U32)];
        left.operations = vec![xor(11, 10, 4), xor(12, 10, 4)];
        left.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_arguments: vec![ValueId(11)],
            else_target: BlockId(2),
            else_arguments: vec![ValueId(12)],
        });
        right.parameters = vec![ValueDef::new(ValueId(20), U32)];
        right.terminator = Some(Terminator::Return {
            values: vec![ValueId(20)],
        });
    } else {
        entry.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_arguments: vec![],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        left.operations = vec![xor(11, 1, 2)];
        left.terminator = Some(Terminator::Return {
            values: vec![ValueId(11)],
        });
        right.terminator = Some(Terminator::Return {
            values: vec![ValueId(3)],
        });
    }
    let mut module = Module::new("v18-dominance-loop");
    module.storage_layouts = fixture().storage_layouts;
    module.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![Type::BOOL, U32, U32], vec![U32]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, left, right],
    ));
    module
}

#[test]
fn observed_dominance_cse_and_loop_edges_pass_the_independent_v18_checker() {
    for looping in [false, true] {
        let input = input(&diamond(looping));
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(23 + input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        assert!(observed.occurrences().candidate().edges.len() >= 2);
        let checked = observed.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), 23 + input.storage);
        assert!(!checked.occurrences().candidate().definitions.is_empty());
    }
}

#[test]
fn separate_sessions_produce_identical_canonical_output_and_observed_relations() {
    let input = input(&fixture());
    let mut outputs = Vec::new();
    for _ in 0..2 {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(input.storage).unwrap();
        let output = observe(&input, &mut budget);
        outputs.push((
            output.owner().canonical_bytes().to_vec(),
            *output.map().digest(),
            output.occurrences().candidate().uses.to_vec(),
        ));
    }
    assert_eq!(outputs[0], outputs[1]);
}

#[test]
fn empty_and_multiple_function_modules_take_the_same_fixed_route() {
    let mut many = fixture();
    many.functions.extend(diamond(false).functions);
    for module in [Module::new("empty"), many] {
        let input = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        assert_eq!(observed.report().passes().len(), 8);
        assert_eq!(
            observed.owner().module().functions.len(),
            module.functions.len()
        );
        observed.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), input.storage);
    }
}

#[test]
fn actual_selected_branch_and_duplicate_destination_edges_have_complete_checked_rows() {
    for selected in [false, true] {
        let mut module = diamond(false);
        let body = module.functions[0].body.as_mut().unwrap();
        if selected {
            body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(9), Type::BOOL),
                Op::Constant(Constant::Bool(true)),
            ));
            let Some(Terminator::ConditionalBranch { condition, .. }) =
                &mut body.blocks[0].terminator
            else {
                unreachable!()
            };
            *condition = ValueId(9);
        } else {
            body.blocks.truncate(2);
            body.blocks[1].parameters = vec![ValueDef::new(ValueId(20), U32)];
            body.blocks[1].operations.clear();
            body.blocks[1].terminator = Some(Terminator::Return {
                values: vec![ValueId(20)],
            });
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(0),
                then_target: BlockId(1),
                then_arguments: vec![ValueId(1)],
                else_target: BlockId(1),
                else_arguments: vec![ValueId(2)],
            });
        }
        let input = input(&module);
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, SPACE);
        budget.reserve_storage(17 + input.storage).unwrap();
        let observed = observe(&input, &mut budget);
        if selected {
            assert!(
                observed.owner().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap()
                    .blocks
                    .len()
                    < 3
            );
        }
        observed.try_check_and_finish_v18(&mut budget).unwrap();
        assert_eq!(budget.storage(), 17 + input.storage);
    }
}

#[test]
fn whole_slice_and_pointer_exposure_use_the_real_fixed_cse_and_checked_transition() {
    use fe2o3_kernel_ir::CastKind;
    let concrete = Type::slice(U32, AddressSpace::Global, AccessMode::ReadOnly);
    let generic = Type::slice(U32, AddressSpace::Generic, AccessMode::ReadOnly);
    let pointer = Type::pointer(U32, AddressSpace::Global, AccessMode::ReadOnly);
    let generic_pointer = Type::pointer(U32, AddressSpace::Generic, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    for (id, from, to, kind) in [
        (2, 0, generic.clone(), CastKind::SliceToGeneric),
        (3, 0, generic.clone(), CastKind::SliceToGeneric),
        (4, 1, generic_pointer.clone(), CastKind::PointerToGeneric),
        (5, 1, generic_pointer.clone(), CastKind::PointerToGeneric),
    ] {
        block.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(id), to.clone()),
            Op::Cast {
                kind,
                value: ValueId(from),
                to,
            },
        ));
    }
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2), ValueId(3), ValueId(4), ValueId(5)],
    });
    let mut module = Module::new("v18-descriptor-cse");
    module.storage_layouts = fixture().storage_layouts;
    module.functions.push(Function::internal_helper(
        "expose",
        Signature::new(
            vec![concrete, pointer],
            vec![
                generic.clone(),
                generic,
                generic_pointer.clone(),
                generic_pointer,
            ],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    let input = input(&module);
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SPACE);
    budget.reserve_storage(13 + input.storage).unwrap();
    let observed = observe(&input, &mut budget);
    let operations = &observed.owner().module().functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks[0]
        .operations;
    assert_eq!(
        operations
            .iter()
            .filter(|op| matches!(op.kind, Op::Cast { .. }))
            .count(),
        2
    );
    observed.try_check_and_finish_v18(&mut budget).unwrap();
    assert_eq!(budget.storage(), 13 + input.storage);
}

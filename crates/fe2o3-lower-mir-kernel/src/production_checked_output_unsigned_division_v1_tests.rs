use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1 as Inventory;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as Coordinate, CastKind, ComparePredicate, Constant,
    Function, IntegerSwitchCase, MemoryAccess, Module, Operation, OperationKind as Kind,
    ScalarType, Signature, SwitchCase, Terminator, Type, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};

const WORK: usize = 100_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const FLOOR: usize = 37;

fn ty(scalar: ScalarType) -> Type {
    Type::Scalar(scalar)
}

fn constant(id: u32, scalar: ScalarType, value: u64) -> Operation {
    let constant = match scalar {
        ScalarType::U8 => Constant::U8(value.try_into().unwrap()),
        ScalarType::U32 => Constant::U32(value.try_into().unwrap()),
        ScalarType::U64 => Constant::U64(value),
        ScalarType::Index => Constant::Index(value),
        _ => panic!("fixture constant type"),
    };
    Operation::effect_free(
        ValueDef::new(ValueId(id), ty(scalar)),
        Kind::Constant(constant),
    )
}

fn compare(id: u32, predicate: ComparePredicate, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::BOOL),
        Kind::Compare {
            predicate,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn binary(id: u32, scalar: ScalarType, op: BinaryOp, lhs: u32, rhs: u32) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), ty(scalar)),
        Kind::Binary {
            op,
            lhs: ValueId(lhs),
            rhs: ValueId(rhs),
        },
    )
}

fn block(id: u32, operations: Vec<Operation>, terminator: Terminator) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.operations = operations;
    block.terminator = Some(terminator);
    block
}

fn ret() -> Terminator {
    Terminator::Return { values: vec![] }
}

fn branch(target: u32, arguments: &[u32]) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: arguments.iter().copied().map(ValueId).collect(),
    }
}

fn conditional(condition: u32, yes: u32, yes_args: &[u32], no: u32, no_args: &[u32]) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: yes_args.iter().copied().map(ValueId).collect(),
        else_target: BlockId(no),
        else_arguments: no_args.iter().copied().map(ValueId).collect(),
    }
}

fn function(name: &str, scalar: ScalarType, blocks: Vec<BasicBlock>) -> Function {
    Function::internal_helper(
        name,
        Signature::new(vec![ty(scalar), ty(scalar), Type::BOOL], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        blocks,
    )
}

fn module(scalar: ScalarType, blocks: Vec<BasicBlock>) -> Module {
    let mut module = Module::new("unsigned-division-fixture");
    module.functions.push(function("f", scalar, blocks));
    module
}

fn coordinate(function: u32, block: u32, operation: u32) -> Coordinate {
    Coordinate {
        block: Block {
            function: FunctionCoordinate(function),
            block,
        },
        operation,
    }
}

fn guarded(
    scalar: ScalarType,
    op: BinaryOp,
    predicate: ComparePredicate,
    reversed: bool,
    nonzero_edge: bool,
) -> Module {
    let (lhs, rhs) = if reversed { (3, 1) } else { (1, 3) };
    let nonzero_is_then = predicate == ComparePredicate::NotEqual;
    let enter_on_then = nonzero_is_then == nonzero_edge;
    module(
        scalar,
        vec![
            block(
                91,
                vec![constant(3, scalar, 0), compare(4, predicate, lhs, rhs)],
                conditional(
                    4,
                    if enter_on_then { 5 } else { 400 },
                    &[],
                    if enter_on_then { 400 } else { 5 },
                    &[],
                ),
            ),
            block(5, vec![binary(5, scalar, op, 0, 1)], ret()),
            block(400, vec![], ret()),
        ],
    )
}

fn with_inventory(module: Module, next: impl FnOnce(&Inventory<'_>, &mut Budget<'_>)) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, owner_storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
            .expect("all component fixtures must pass actual connected-owner verification");
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    drop(module);
    let (inventory, inventory_storage) = Inventory::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    next(&inventory, &mut budget);
    assert_eq!(
        budget.storage(),
        floor,
        "analysis/query must restore its incoming floor"
    );
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

fn target() -> SemanticTargetDataLayoutV1 {
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32]))
}

fn check_module(input: Module, expected: &[(Coordinate, bool)]) {
    with_inventory(input, |inventory, budget| {
        let floor = budget.storage();
        {
            let proof = check(inventory, target(), budget).unwrap();
            for &(coordinate, expected) in expected {
                let ordinal = inventory
                    .operations()
                    .iter()
                    .position(|row| row.coordinate == coordinate)
                    .unwrap();
                assert_eq!(
                    proof.operation(inventory, ordinal, budget).unwrap(),
                    expected,
                    "actual verified operation {coordinate:?}"
                );
            }
            assert!(
                !proof
                    .operation(inventory, inventory.operations().len(), budget)
                    .unwrap()
            );
            assert!(!proof.operation(inventory, usize::MAX, budget).unwrap());
        }
        // This internal worker leaves scratch owned by the enclosing general
        // admission transaction. Release only after the borrowed proof drops.
        budget.release_storage(budget.storage() - floor).unwrap();
    });
}

#[test]
fn unsigned_division_exact_eq_ne_polarities_and_reversed_zero_operands() {
    for scalar in [ScalarType::U32, ScalarType::U64, ScalarType::Index] {
        for op in [BinaryOp::Divide, BinaryOp::Remainder] {
            for predicate in [ComparePredicate::Equal, ComparePredicate::NotEqual] {
                for reversed in [false, true] {
                    for safe in [false, true] {
                        check_module(
                            guarded(scalar, op, predicate, reversed, safe),
                            &[(coordinate(0, 1, 0), safe)],
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn unsigned_division_constant_zero_and_nonzero_are_distinguished_without_a_guard() {
    for scalar in [ScalarType::U32, ScalarType::U64, ScalarType::Index] {
        for op in [BinaryOp::Divide, BinaryOp::Remainder] {
            for value in [0, 1, 7] {
                check_module(
                    module(
                        scalar,
                        vec![block(
                            91,
                            vec![constant(3, scalar, value), binary(4, scalar, op, 0, 3)],
                            ret(),
                        )],
                    ),
                    &[(coordinate(0, 0, 1), value != 0)],
                );
            }
        }
    }
}

#[test]
fn unsigned_division_an_unrelated_nonzero_comparison_does_not_prove_the_divisor() {
    let mut input = guarded(
        ScalarType::U32,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        true,
    );
    let entry = &mut input.functions[0].body.as_mut().unwrap().blocks[0];
    entry.operations[1] = compare(4, ComparePredicate::NotEqual, 0, 3);
    check_module(input, &[(coordinate(0, 1, 0), false)]);
}

#[test]
fn unsigned_division_must_reject_an_alternate_unguarded_predecessor() {
    let scalar = ScalarType::U32;
    let input = module(
        scalar,
        vec![
            block(91, vec![], conditional(2, 8, &[], 5, &[])),
            block(
                8,
                vec![
                    constant(3, scalar, 0),
                    compare(4, ComparePredicate::NotEqual, 1, 3),
                ],
                conditional(4, 5, &[], 400, &[]),
            ),
            block(5, vec![binary(5, scalar, BinaryOp::Divide, 0, 1)], ret()),
            block(400, vec![], ret()),
        ],
    );
    check_module(input, &[(coordinate(0, 2, 0), false)]);
}

#[test]
fn unsigned_division_duplicate_successor_edges_keep_both_polarities() {
    let scalar = ScalarType::U32;
    let input = module(
        scalar,
        vec![
            block(
                91,
                vec![
                    constant(3, scalar, 0),
                    compare(4, ComparePredicate::NotEqual, 1, 3),
                ],
                conditional(4, 5, &[], 5, &[]),
            ),
            block(5, vec![binary(5, scalar, BinaryOp::Divide, 0, 1)], ret()),
        ],
    );
    check_module(input, &[(coordinate(0, 1, 0), false)]);
}

#[test]
fn unsigned_division_same_block_operation_before_the_guard_is_not_proven() {
    let scalar = ScalarType::U32;
    let input = module(
        scalar,
        vec![
            block(
                91,
                vec![
                    constant(3, scalar, 0),
                    binary(5, scalar, BinaryOp::Divide, 0, 1),
                    compare(4, ComparePredicate::NotEqual, 1, 3),
                ],
                conditional(4, 5, &[], 400, &[]),
            ),
            block(5, vec![binary(6, scalar, BinaryOp::Remainder, 0, 1)], ret()),
            block(400, vec![], ret()),
        ],
    );
    check_module(
        input,
        &[(coordinate(0, 0, 1), false), (coordinate(0, 1, 0), true)],
    );
}

#[test]
fn unsigned_division_block_parameter_proof_uses_the_actual_incoming_definition() {
    let scalar = ScalarType::U64;
    for transported in [1, 3] {
        let mut use_block = block(5, vec![binary(11, scalar, BinaryOp::Divide, 0, 10)], ret());
        use_block
            .parameters
            .push(ValueDef::new(ValueId(10), ty(scalar)));
        let input = module(
            scalar,
            vec![
                block(
                    91,
                    vec![
                        constant(3, scalar, 0),
                        compare(4, ComparePredicate::NotEqual, 1, 3),
                    ],
                    conditional(4, 5, &[transported], 400, &[]),
                ),
                use_block,
                block(400, vec![], ret()),
            ],
        );
        check_module(input, &[(coordinate(0, 1, 0), transported == 1)]);
    }
}

#[test]
fn unsigned_division_duplicate_edges_bind_each_actual_parameter_argument() {
    let scalar = ScalarType::U32;
    for bad_argument in [false, true] {
        let mut use_block = block(
            5,
            vec![binary(11, scalar, BinaryOp::Remainder, 0, 10)],
            ret(),
        );
        use_block
            .parameters
            .push(ValueDef::new(ValueId(10), ty(scalar)));
        let input = module(
            scalar,
            vec![
                block(
                    91,
                    vec![constant(3, scalar, 0), constant(4, scalar, 7)],
                    conditional(2, 5, &[4], 5, &[if bad_argument { 3 } else { 4 }]),
                ),
                use_block,
            ],
        );
        check_module(input, &[(coordinate(0, 1, 0), !bad_argument)]);
    }
}

#[test]
fn unsigned_division_independently_guarded_predecessors_join_exact_parameter_values() {
    let scalar = ScalarType::U32;
    let mut use_block = block(5, vec![binary(11, scalar, BinaryOp::Divide, 0, 10)], ret());
    use_block
        .parameters
        .push(ValueDef::new(ValueId(10), ty(scalar)));
    check_module(
        module(
            scalar,
            vec![
                block(
                    91,
                    vec![constant(3, scalar, 0)],
                    conditional(2, 8, &[], 9, &[]),
                ),
                block(
                    8,
                    vec![compare(4, ComparePredicate::NotEqual, 1, 3)],
                    conditional(4, 5, &[1], 400, &[]),
                ),
                block(
                    9,
                    vec![compare(5, ComparePredicate::Equal, 0, 3)],
                    conditional(5, 400, &[], 5, &[0]),
                ),
                use_block,
                block(400, vec![], ret()),
            ],
        ),
        &[(coordinate(0, 3, 0), true)],
    );
}

#[test]
fn unsigned_division_loop_parameter_bindings_are_simultaneous_not_sequential() {
    let scalar = ScalarType::U32;
    for swap in [false, true] {
        let mut body = block(
            5,
            vec![
                binary(12, scalar, BinaryOp::Divide, 0, 10),
                binary(13, scalar, BinaryOp::Divide, 0, 11),
            ],
            conditional(2, 5, if swap { &[11, 10] } else { &[10, 11] }, 400, &[]),
        );
        body.parameters = vec![
            ValueDef::new(ValueId(10), ty(scalar)),
            ValueDef::new(ValueId(11), ty(scalar)),
        ];
        check_module(
            module(
                scalar,
                vec![
                    block(
                        91,
                        vec![
                            constant(3, scalar, 0),
                            compare(4, ComparePredicate::NotEqual, 1, 3),
                        ],
                        conditional(4, 5, &[1, 0], 400, &[]),
                    ),
                    body,
                    block(400, vec![], ret()),
                ],
            ),
            &[(coordinate(0, 1, 0), !swap), (coordinate(0, 1, 1), false)],
        );
    }
}

#[test]
fn unsigned_division_loop_requires_both_safe_entry_and_safe_backedge_arguments() {
    let scalar = ScalarType::U32;
    for safe_entry in [false, true] {
        for safe_backedge in [false, true] {
            let entry = block(
                91,
                vec![
                    constant(3, scalar, 0),
                    compare(4, ComparePredicate::NotEqual, 1, 3),
                ],
                if safe_entry {
                    conditional(4, 5, &[1], 400, &[])
                } else {
                    branch(5, &[1])
                },
            );
            let mut body = block(
                5,
                vec![binary(11, scalar, BinaryOp::Divide, 0, 10)],
                conditional(2, 5, &[if safe_backedge { 10 } else { 3 }], 400, &[]),
            );
            body.parameters.push(ValueDef::new(ValueId(10), ty(scalar)));
            check_module(
                module(scalar, vec![entry, body, block(400, vec![], ret())]),
                &[(coordinate(0, 1, 0), safe_entry && safe_backedge)],
            );
        }
    }
}

#[test]
fn unsigned_division_entry_backedge_cannot_erase_the_unguarded_function_entry() {
    let scalar = ScalarType::U32;
    check_module(
        module(
            scalar,
            vec![
                block(
                    91,
                    vec![
                        constant(3, scalar, 0),
                        binary(5, scalar, BinaryOp::Divide, 0, 1),
                        compare(4, ComparePredicate::NotEqual, 1, 3),
                    ],
                    conditional(4, 91, &[], 400, &[]),
                ),
                block(400, vec![], ret()),
            ],
        ),
        &[(coordinate(0, 0, 1), false)],
    );
}

#[test]
fn unsigned_division_guarded_loop_rechecks_each_new_backedge_definition() {
    let scalar = ScalarType::U32;
    let entry = block(
        91,
        vec![
            constant(3, scalar, 0),
            constant(6, scalar, 1),
            compare(4, ComparePredicate::NotEqual, 1, 3),
        ],
        conditional(4, 5, &[1], 400, &[]),
    );
    let mut body = block(
        5,
        vec![
            binary(11, scalar, BinaryOp::Divide, 0, 10),
            binary(12, scalar, BinaryOp::Subtract, 10, 6),
            compare(13, ComparePredicate::NotEqual, 12, 3),
        ],
        conditional(13, 5, &[12], 400, &[]),
    );
    body.parameters.push(ValueDef::new(ValueId(10), ty(scalar)));
    check_module(
        module(scalar, vec![entry, body, block(400, vec![], ret())]),
        &[(coordinate(0, 1, 0), true)],
    );
}

#[test]
fn unsigned_division_nonzero_fact_does_not_survive_narrowing_or_changed_arithmetic() {
    let scalar = ScalarType::U64;
    let mut narrowed = guarded(
        scalar,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        true,
    );
    narrowed.functions[0].body.as_mut().unwrap().blocks[1].operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(5), ty(ScalarType::U32)),
            Kind::Cast {
                kind: CastKind::Truncate,
                value: ValueId(1),
                to: ty(ScalarType::U32),
            },
        ),
        constant(6, ScalarType::U32, 42),
        binary(7, ScalarType::U32, BinaryOp::Divide, 6, 5),
    ];
    check_module(narrowed, &[(coordinate(0, 1, 2), false)]);
    let mut changed = guarded(
        scalar,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        true,
    );
    changed.functions[0].body.as_mut().unwrap().blocks[1].operations = vec![
        binary(5, scalar, BinaryOp::Subtract, 1, 1),
        binary(6, scalar, BinaryOp::Divide, 0, 5),
    ];
    check_module(changed, &[(coordinate(0, 1, 1), false)]);
}

#[test]
fn unsigned_division_nonzero_fact_survives_only_supported_nonzero_preserving_casts() {
    for (from, to, kind) in [
        (ScalarType::U32, ScalarType::U64, CastKind::ZeroExtend),
        (ScalarType::U32, ScalarType::Index, CastKind::ZeroExtend),
        (ScalarType::U64, ScalarType::Index, CastKind::Bitcast),
        (ScalarType::Index, ScalarType::U64, CastKind::Bitcast),
    ] {
        let mut input = guarded(
            from,
            BinaryOp::Divide,
            ComparePredicate::NotEqual,
            false,
            true,
        );
        input.functions[0].body.as_mut().unwrap().blocks[1].operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(5), ty(to)),
                Kind::Cast {
                    kind,
                    value: ValueId(1),
                    to: ty(to),
                },
            ),
            constant(6, to, 42),
            binary(7, to, BinaryOp::Divide, 6, 5),
        ];
        check_module(input, &[(coordinate(0, 1, 2), true)]);
    }
}

#[test]
fn unsigned_division_loop_backedge_alias_retains_only_its_exact_parameter_proof() {
    let mut body = block(
        5,
        vec![
            binary(11, ScalarType::U64, BinaryOp::Divide, 6, 10),
            Operation::effect_free(
                ValueDef::new(ValueId(12), ty(ScalarType::Index)),
                Kind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(10),
                    to: ty(ScalarType::Index),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(13), ty(ScalarType::U64)),
                Kind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(12),
                    to: ty(ScalarType::U64),
                },
            ),
        ],
        conditional(2, 5, &[13], 400, &[]),
    );
    body.parameters
        .push(ValueDef::new(ValueId(10), ty(ScalarType::U64)));
    check_module(
        module(
            ScalarType::U32,
            vec![
                block(
                    91,
                    vec![
                        constant(3, ScalarType::U32, 0),
                        constant(6, ScalarType::U64, 42),
                        Operation::effect_free(
                            ValueDef::new(ValueId(7), ty(ScalarType::U64)),
                            Kind::Cast {
                                kind: CastKind::ZeroExtend,
                                value: ValueId(1),
                                to: ty(ScalarType::U64),
                            },
                        ),
                        compare(4, ComparePredicate::NotEqual, 1, 3),
                    ],
                    conditional(4, 5, &[7], 400, &[]),
                ),
                body,
                block(400, vec![], ret()),
            ],
        ),
        &[(coordinate(0, 1, 0), true)],
    );
}

#[test]
fn unsigned_division_switch_cases_default_and_duplicate_targets_keep_exact_edge_facts() {
    for scalar in [ScalarType::U32, ScalarType::U64, ScalarType::Index] {
        for typed in [false, true] {
            for (case_value, case_target, default_target, expected) in [
                (0, 400, 5, true),
                (7, 5, 400, true),
                (0, 5, 400, false),
                (7, 400, 5, false),
                (0, 5, 5, false),
            ] {
                let terminator = if typed {
                    let value = match scalar {
                        ScalarType::U32 => Constant::U32(case_value as u32),
                        ScalarType::U64 => Constant::U64(case_value),
                        ScalarType::Index => Constant::Index(case_value),
                        _ => unreachable!(),
                    };
                    Terminator::IntegerSwitch {
                        selector: ValueId(1),
                        cases: vec![IntegerSwitchCase {
                            value,
                            target: BlockId(case_target),
                            arguments: vec![],
                        }],
                        default_target: BlockId(default_target),
                        default_arguments: vec![],
                    }
                } else {
                    Terminator::Switch {
                        selector: ValueId(1),
                        cases: vec![SwitchCase {
                            value: case_value,
                            target: BlockId(case_target),
                            arguments: vec![],
                        }],
                        default_target: BlockId(default_target),
                        default_arguments: vec![],
                    }
                };
                check_module(
                    module(
                        scalar,
                        vec![
                            block(91, vec![], terminator),
                            block(5, vec![binary(5, scalar, BinaryOp::Divide, 0, 1)], ret()),
                            block(400, vec![], ret()),
                        ],
                    ),
                    &[(coordinate(0, 1, 0), expected)],
                );
            }
        }
    }
}

#[test]
fn unsigned_division_a_new_load_cannot_borrow_the_previous_loads_guard() {
    let scalar = ScalarType::U32;
    let load = |result| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), ty(scalar)),
            Kind::Load {
                pointer: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        )
    };
    let mut input = module(
        scalar,
        vec![
            block(
                91,
                vec![
                    load(3),
                    constant(4, scalar, 0),
                    compare(5, ComparePredicate::NotEqual, 3, 4),
                ],
                conditional(5, 5, &[], 400, &[]),
            ),
            block(
                5,
                vec![
                    binary(8, scalar, BinaryOp::Divide, 0, 3),
                    load(6),
                    binary(7, scalar, BinaryOp::Divide, 0, 6),
                ],
                ret(),
            ),
            block(400, vec![], ret()),
        ],
    );
    input.functions[0].signature.parameters[1] =
        Type::pointer(ty(scalar), AddressSpace::Global, AccessMode::ReadOnly);
    check_module(
        input,
        &[(coordinate(0, 1, 0), true), (coordinate(0, 1, 2), false)],
    );
}

#[test]
fn unsigned_division_same_value_ids_in_another_function_do_not_share_facts() {
    let scalar = ScalarType::U32;
    let mut input = guarded(
        scalar,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        true,
    );
    let mut other = guarded(
        scalar,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        false,
    );
    let mut other_function = other.functions.remove(0);
    other_function.id = "other".into();
    input.functions.push(other_function);
    check_module(
        input,
        &[(coordinate(0, 1, 0), true), (coordinate(1, 1, 0), false)],
    );
}

#[test]
fn unsigned_division_unreachable_self_cycle_cannot_invent_a_nonzero_seed() {
    let scalar = ScalarType::U32;
    let mut detached = block(
        5,
        vec![binary(11, scalar, BinaryOp::Divide, 0, 10)],
        branch(5, &[10]),
    );
    detached
        .parameters
        .push(ValueDef::new(ValueId(10), ty(scalar)));
    check_module(
        module(scalar, vec![block(91, vec![], ret()), detached]),
        &[(coordinate(0, 1, 0), false)],
    );
}

#[test]
fn unsigned_division_narrow_unsigned_signed_and_float_operations_remain_out_of_scope() {
    for value in [
        Constant::U8(7),
        Constant::I32(7),
        Constant::F32Bits(7f32.to_bits()),
    ] {
        let scalar = match &value {
            Constant::U8(_) => ScalarType::U8,
            Constant::I32(_) => ScalarType::I32,
            Constant::F32Bits(_) => ScalarType::F32,
            _ => unreachable!(),
        };
        check_module(
            module(
                scalar,
                vec![block(
                    91,
                    vec![
                        Operation::effect_free(
                            ValueDef::new(ValueId(3), ty(scalar)),
                            Kind::Constant(value),
                        ),
                        binary(4, scalar, BinaryOp::Divide, 0, 3),
                    ],
                    ret(),
                )],
            ),
            &[(coordinate(0, 0, 1), false)],
        );
    }
}

#[test]
fn unsigned_division_foreign_inventory_is_rejected_even_for_identical_verified_bytes() {
    let input = guarded(
        ScalarType::U32,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        true,
    );
    with_inventory(input, |inventory, budget| {
        let outer_floor = budget.storage();
        let (other_owner, owner_storage) =
            Owner::from_module_ref_with_verification_budget_v12(inventory.owner().module(), budget)
                .unwrap();
        budget
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (other_inventory, inventory_storage) = Inventory::derive(&other_owner, budget).unwrap();
        budget
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        assert_eq!(inventory.identity(), other_inventory.identity());
        assert!(!std::ptr::eq(inventory, &other_inventory));
        let proof_floor = budget.storage();
        {
            let proof = check(inventory, target(), budget).unwrap();
            assert!(matches!(
                proof.operation(&other_inventory, 2, budget),
                Err(
                    ProductionCheckedOutputAdmissionErrorPolicy3V1::Unsupported {
                        phase: "unsigned division",
                        detail: "same borrowed inventory",
                    }
                )
            ));
            assert!(proof.operation(inventory, 2, budget).unwrap());
        }
        budget
            .release_storage(budget.storage() - proof_floor)
            .unwrap();
        drop(other_inventory);
        budget
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        drop(other_owner);
        budget
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), outer_floor);
    });
}

#[test]
fn unsigned_division_repeated_derivation_is_deterministic_on_the_actual_inventory() {
    let input = guarded(
        ScalarType::U32,
        BinaryOp::Divide,
        ComparePredicate::NotEqual,
        false,
        true,
    );
    with_inventory(input, |inventory, setup_budget| {
        let floor = setup_budget.storage();
        let run = || {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let observed = {
                let proof = check(inventory, target(), &mut budget).unwrap();
                [0, 1, 2].map(|ordinal| proof.operation(inventory, ordinal, &mut budget).unwrap())
            };
            let snapshot = (
                observed,
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
            );
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            snapshot
        };
        let first = run();
        assert_eq!(first.0, [false, false, true]);
        assert_eq!(first, run());
    });
}

fn fixed_header() -> usize {
    std::mem::size_of::<&Inventory<'_>>() + std::mem::size_of::<SemanticTargetDataLayoutV1>()
}

fn bitmap_header() -> usize {
    std::mem::size_of::<Vec<u8>>()
}

#[test]
fn unsigned_division_empty_check_and_query_have_exact_and_one_short_work_limits() {
    with_inventory(Module::new("empty"), |inventory, setup_budget| {
        let floor = setup_budget.storage();
        for limit in [1, 7, 8] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            let expected_peak = match limit {
                1 => floor,
                7 => floor + fixed_header(),
                _ => floor + fixed_header() + bitmap_header(),
            };
            {
                let result = check(inventory, target(), &mut budget);
                if limit == 8 {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                            Resource::Work(_)
                        ))
                    ));
                }
            }
            assert_eq!(
                budget.work(),
                if limit == 1 {
                    0
                } else if limit == 7 {
                    2
                } else {
                    8
                }
            );
            assert_eq!(budget.storage(), expected_peak);
            assert_eq!(budget.peak_storage(), expected_peak);
            assert_eq!(budget.failed_storage(), None);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                work.failed_work(),
                if limit == 1 {
                    Some(2)
                } else if limit == 7 {
                    Some(8)
                } else {
                    None
                }
            );
        }
        for limit in [10, 11] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            {
                let proof = check(inventory, target(), &mut budget).unwrap();
                let result = proof.operation(inventory, 0, &mut budget);
                if limit == 11 {
                    assert!(matches!(result, Ok(false)));
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                            Resource::Work(_)
                        ))
                    ));
                }
            }
            assert_eq!(budget.work(), if limit == 10 { 8 } else { 11 });
            assert_eq!(budget.storage(), floor + fixed_header() + bitmap_header());
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                work.failed_work(),
                if limit == 10 { Some(11) } else { None }
            );
        }
    });
}

#[test]
fn unsigned_division_empty_header_storage_boundaries_preserve_the_accepted_prefix() {
    with_inventory(Module::new("empty"), |inventory, setup_budget| {
        let floor = setup_budget.storage();
        let retained = fixed_header() + bitmap_header();
        for limit in [
            floor + fixed_header() - 1,
            floor + retained - 1,
            floor + retained,
        ] {
            let mut work = Work::new(WORK);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            {
                let result = check(inventory, target(), &mut budget);
                if limit == floor + retained {
                    assert!(result.is_ok());
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                            Resource::Storage(_)
                        ))
                    ));
                }
            }
            let first = limit < floor + fixed_header();
            assert_eq!(budget.work(), if first { 2 } else { 8 });
            assert_eq!(
                budget.storage(),
                if first {
                    floor
                } else if limit < floor + retained {
                    floor + fixed_header()
                } else {
                    floor + retained
                }
            );
            assert_eq!(budget.peak_storage(), budget.storage());
            assert_eq!(
                budget.failed_storage(),
                if first {
                    Some(floor + fixed_header())
                } else if limit < floor + retained {
                    Some(floor + retained)
                } else {
                    None
                }
            );
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(work.failed_work(), None);
        }
    });
}

#[test]
fn unsigned_division_work_failure_after_bitmap_allocation_remains_caller_releasable() {
    let input = module(
        ScalarType::U32,
        vec![block(91, vec![constant(3, ScalarType::U32, 7)], ret())],
    );
    with_inventory(input, |inventory, setup_budget| {
        let floor = setup_budget.storage();
        let mut work = Work::new(16);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            check(inventory, target(), &mut budget),
            Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                Resource::Work(_)
            ))
        ));
        assert_eq!(budget.work(), 9);
        assert!(budget.storage() > floor + fixed_header() + bitmap_header());
        assert_eq!(budget.peak_storage(), budget.storage());
        assert_eq!(budget.failed_storage(), None);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
        assert_eq!(work.failed_work(), Some(17));
    });
}

#[test]
fn unsigned_division_constant_candidate_has_exact_late_97_96_work_boundary() {
    let input = module(
        ScalarType::U32,
        vec![block(
            91,
            vec![
                constant(3, ScalarType::U32, 7),
                binary(4, ScalarType::U32, BinaryOp::Divide, 0, 3),
            ],
            ret(),
        )],
    );
    with_inventory(input, |inventory, setup_budget| {
        let floor = setup_budget.storage();
        assert_eq!(inventory.operations().len(), 2);
        assert_eq!(inventory.definitions().len(), 5);
        assert_eq!(inventory.blocks().len(), 1);
        assert!(inventory.edges().is_empty());
        // 26 candidate census + 21 facts + 30 five buffers + 4 initialization
        // + 3 offsets + 2 entry + 2 reachability + 4 flags + 2 normalize + 3 literal.
        let minimum_scratch = fixed_header()
            + 2 * bitmap_header()
            + std::mem::size_of::<Vec<Fact>>()
            + 4 * std::mem::size_of::<Vec<usize>>()
            + 3
            + 5 * std::mem::size_of::<Fact>()
            + 4 * std::mem::size_of::<usize>();
        for limit in [96, 97] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.reserve_storage(floor).unwrap();
            {
                let result = check(inventory, target(), &mut budget);
                if limit == 97 {
                    assert_eq!(result.unwrap().operations, [0, 1]);
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionCheckedOutputAdmissionErrorPolicy3V1::Resource(
                            Resource::Work(_)
                        ))
                    ));
                }
            }
            assert_eq!(budget.work(), if limit == 96 { 94 } else { 97 });
            assert!(budget.storage() >= floor + minimum_scratch);
            assert_eq!(budget.peak_storage(), budget.storage());
            assert_eq!(budget.failed_storage(), None);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(
                work.failed_work(),
                if limit == 96 { Some(97) } else { None }
            );
        }
    });
}

#[test]
fn unsigned_division_large_unrelated_cfg_uses_only_the_no_candidate_bitmap_budget() {
    const BLOCKS: usize = 128;
    let mut blocks = Vec::with_capacity(BLOCKS);
    for ordinal in 0..BLOCKS {
        let id = 10_000 - ordinal as u32;
        blocks.push(block(
            id,
            vec![constant(ordinal as u32 + 3, ScalarType::U32, 7)],
            if ordinal + 1 == BLOCKS {
                ret()
            } else {
                branch(id - 1, &[])
            },
        ));
    }
    with_inventory(
        module(ScalarType::U32, blocks),
        |inventory, setup_budget| {
            let floor = setup_budget.storage();
            assert_eq!(inventory.operations().len(), BLOCKS);
            let mut work = Work::new(8 + 9 * BLOCKS + 3);
            let mut budget = Budget::new(
                &mut work,
                floor + fixed_header() + bitmap_header() + 2 * BLOCKS,
            );
            budget.reserve_storage(floor).unwrap();
            {
                let proof = check(inventory, target(), &mut budget).unwrap();
                assert_eq!(budget.work(), 8 + 9 * BLOCKS);
                assert_eq!(
                    budget.storage(),
                    floor + fixed_header() + bitmap_header() + proof.operations.capacity()
                );
                assert!(!proof.operation(inventory, BLOCKS - 1, &mut budget).unwrap());
            }
            assert_eq!(budget.work(), 8 + 9 * BLOCKS + 3);
            assert_eq!(budget.peak_storage(), budget.storage());
            assert_eq!(budget.failed_storage(), None);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            assert_eq!(work.failed_work(), None);
        },
    );
}

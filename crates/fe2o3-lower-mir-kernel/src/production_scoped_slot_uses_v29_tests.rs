use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1;

const LIMIT: usize = 10_000_000;
const FLOOR: usize = 31;
const P: ValueId = ValueId(4_000_000_000);
const Q: ValueId = ValueId(4_000_000_001);

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn pointer(access: AccessMode) -> Type {
    Type::pointer(scalar(), AddressSpace::Private, access)
}
fn access() -> MemoryAccess {
    MemoryAccess::new(AddressSpace::Private, 4)
}
fn result(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}
fn allocation(id: ValueId, element: Type) -> Operation {
    Operation::effect_free(
        ValueDef::new(
            id,
            Type::pointer(
                element.clone(),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        ),
        OperationKind::Alloca {
            element,
            count: None,
            address_space: AddressSpace::Private,
            alignment: 8,
        },
    )
}
fn store(pointer: ValueId, value: ValueId) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer,
            value,
            access: access(),
        },
    )
}
fn load(id: u32, pointer: ValueId, ty: Type) -> Operation {
    result(
        id,
        ty,
        OperationKind::Load {
            pointer,
            access: access(),
        },
    )
}
fn block(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}
fn fixture() -> Function {
    let mut entry = block(77);
    entry.operations = vec![
        allocation(P, scalar()),
        allocation(Q, scalar()),
        result(3, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        store(P, ValueId(1)),
    ];
    Function::internal_helper(
        "slot-use",
        Signature::new(
            vec![
                Type::BOOL,
                scalar(),
                pointer(AccessMode::ReadWrite),
                Type::pointer(
                    pointer(AccessMode::ReadWrite),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(4)],
        vec![entry],
    )
}

// Inert candidate labels for analysis of raw KIR, never source-census authority.
fn candidates(function: &Function) -> Vec<ScopedSourceSlotV29> {
    let mut slots = Vec::new();
    for (block_ordinal, block) in function.body.as_ref().unwrap().blocks.iter().enumerate() {
        for (operation, row) in block.operations.iter().enumerate() {
            let OperationKind::Alloca { element, .. } = &row.kind else {
                continue;
            };
            let (element, size) = match element {
                Type::Scalar(scalar) => (PrivateRetainedElementFactsV1::Scalar(*scalar), 4),
                Type::Pointer(pointer) => (
                    PrivateRetainedElementFactsV1::ThinPointer {
                        element: pointer.pointee.as_scalar().unwrap(),
                        space: pointer.address_space,
                        access: pointer.access,
                    },
                    8,
                ),
                _ => unreachable!(),
            };
            slots.push(ScopedSourceSlotV29 {
                instance: ProductionCallInstanceIdV1(0),
                origin: ScopedSlotOriginV29 {
                    local: slots.len() as u32,
                    semantic_type: SemanticTypeIdV1::from_index(0),
                    pointer: row.results[0].id,
                },
                element_type: SemanticTypeIdV1::from_index(0),
                element: PrivateRetainedSlotFactsV1 {
                    element,
                    size,
                    alignment: 8,
                },
                length: 1,
                bytes: size,
                count: None,
                allocation: PrivateArrayPhysicalLocationV1 {
                    block_ordinal,
                    block: block.id,
                    operation,
                },
            });
        }
    }
    slots
}

fn verified(function: &Function) {
    let mut module = Module::new("slot-use-analysis-only");
    module.functions.push(function.clone());
    module.functions.push(Function::external_import(
        "sink",
        Signature::new(vec![pointer(AccessMode::ReadWrite)], vec![]),
    ));
    verify_module(&module).unwrap();
}
fn run(
    function: &Function,
    work_limit: usize,
    storage_limit: usize,
) -> (UseResult<()>, usize, usize) {
    let slots = candidates(function);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        check_function(function, &slots, 19, budget)
    });
    assert_eq!(budget.storage(), FLOOR);
    (result, budget.work(), budget.peak_storage())
}
fn detail(result: &UseResult<()>) -> &'static str {
    match result {
        Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. }) => detail,
        other => panic!("expected precise slot-use refusal, got {other:?}"),
    }
}

#[test]
fn direct_gep_restricted_selected_and_guarded_accesses_keep_one_slot_origin() {
    let mut function = fixture();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .extend([
            result(
                10,
                pointer(AccessMode::ReadWrite),
                OperationKind::GetElementPointer {
                    base: P,
                    offset: ValueId(3),
                },
            ),
            result(
                11,
                pointer(AccessMode::ReadOnly),
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: P,
                    to: pointer(AccessMode::ReadOnly),
                },
            ),
            load(12, ValueId(11), scalar()),
            result(
                13,
                scalar(),
                OperationKind::GuardedLoad {
                    pointer: P,
                    predicate: ValueId(0),
                    fallback: ValueId(1),
                    access: access(),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer: P,
                    predicate: ValueId(0),
                    value: ValueId(1),
                    access: access(),
                },
            ),
            result(
                14,
                pointer(AccessMode::ReadWrite),
                OperationKind::Select {
                    condition: ValueId(0),
                    true_value: P,
                    false_value: ValueId(10),
                },
            ),
            load(15, ValueId(14), scalar()),
        ]);
    verified(&function);
    run(&function, LIMIT, LIMIT).0.unwrap();
}

fn loop_fixture(other: ValueId) -> Function {
    let mut function = fixture();
    let body = function.body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(900),
        then_arguments: vec![P],
        else_target: BlockId(900),
        else_arguments: vec![other],
    });
    let mut header = block(900);
    header
        .parameters
        .push(ValueDef::new(ValueId(100), pointer(AccessMode::ReadWrite)));
    header.operations.push(load(101, ValueId(100), scalar()));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(900),
        then_arguments: vec![ValueId(100)],
        else_target: BlockId(901),
        else_arguments: vec![],
    });
    body.blocks.extend([header, block(901)]);
    function
}

#[test]
fn grounded_cfg_loops_and_duplicate_edges_accept_only_the_same_origin() {
    let function = loop_fixture(P);
    verified(&function);
    run(&function, LIMIT, LIMIT).0.unwrap();
    for other in [Q, ValueId(2)] {
        let function = loop_fixture(other);
        verified(&function);
        assert_eq!(
            detail(&run(&function, LIMIT, LIMIT).0),
            "scoped source-slot pointer transport is ambiguous"
        );
    }
}

#[test]
fn two_slot_and_slot_external_selects_cannot_hide_conflicting_origins() {
    for other in [Q, ValueId(2)] {
        let mut function = fixture();
        function.body.as_mut().unwrap().blocks[0]
            .operations
            .push(result(
                10,
                pointer(AccessMode::ReadWrite),
                OperationKind::Select {
                    condition: ValueId(0),
                    true_value: P,
                    false_value: other,
                },
            ));
        verified(&function);
        assert!(matches!(
            detail(&run(&function, LIMIT, LIMIT).0),
            "scoped source-slot selection mixes allocation origins"
                | "scoped source-slot pointer transport is ambiguous"
        ));
    }
}

#[test]
fn address_escape_and_already_invalid_identity_observation_reject() {
    for case in 0..4 {
        let mut function = fixture();
        let body = function.body.as_mut().unwrap();
        match case {
            0 => {
                function
                    .signature
                    .results
                    .push(pointer(AccessMode::ReadWrite));
                body.blocks[0].terminator = Some(Terminator::Return { values: vec![P] });
            }
            1 => body.blocks[0].operations.push(Operation::new(
                vec![],
                OperationKind::Call {
                    callee: FunctionId::new("sink"),
                    arguments: vec![P],
                },
            )),
            2 => body.blocks[0].operations.push(store(ValueId(4), P)),
            3 => body.blocks[0].operations.push(result(
                10,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: P,
                    rhs: Q,
                },
            )),
            _ => unreachable!(),
        }
        if case == 3 {
            // Pointer comparison is already invalid KIR; preserve that boundary
            // while also testing the pre-verification operand-role rejection.
            let mut module = Module::new("invalid-pointer-comparison");
            module.functions.push(function.clone());
            let error = verify_module(&module).unwrap_err();
            assert_eq!(error.diagnostics().len(), 1);
            assert_eq!(
                error.diagnostics()[0].code,
                fe2o3_kernel_ir::DiagnosticCode::InvalidOperandType
            );
        } else {
            verified(&function);
        }
        assert_eq!(
            detail(&run(&function, LIMIT, LIMIT).0),
            "scoped source-slot address escapes through an unsupported operand"
        );
    }
}

#[test]
fn loaded_pointer_data_is_unrelated_but_storing_a_slot_address_is_not() {
    let mut function = fixture();
    let storage = ValueId(4_000_000_002);
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations.extend([
        allocation(storage, pointer(AccessMode::ReadWrite)),
        store(storage, ValueId(2)),
        load(10, storage, pointer(AccessMode::ReadWrite)),
        load(11, ValueId(10), scalar()),
    ]);
    verified(&function);
    run(&function, LIMIT, LIMIT).0.unwrap();
    let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
    operations[5] = store(storage, P);
    verified(&function);
    assert_eq!(
        detail(&run(&function, LIMIT, LIMIT).0),
        "scoped source-slot address escapes through an unsupported operand"
    );
}

#[test]
fn guarded_fallback_and_guarded_store_value_are_not_address_transport_roles() {
    for store_case in [false, true] {
        let mut function = fixture();
        let storage = ValueId(4_000_000_002);
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        operations.push(allocation(storage, pointer(AccessMode::ReadWrite)));
        operations.push(if store_case {
            Operation::new(
                vec![],
                OperationKind::GuardedStore {
                    pointer: storage,
                    predicate: ValueId(0),
                    value: P,
                    access: access(),
                },
            )
        } else {
            result(
                10,
                pointer(AccessMode::ReadWrite),
                OperationKind::GuardedLoad {
                    pointer: storage,
                    predicate: ValueId(0),
                    fallback: P,
                    access: access(),
                },
            )
        });
        verified(&function);
        assert_eq!(
            detail(&run(&function, LIMIT, LIMIT).0),
            "scoped source-slot address escapes through an unsupported operand"
        );
    }
}

#[test]
fn one_value_in_allowed_and_forbidden_operand_positions_still_rejects() {
    let mut function = fixture();
    function.body.as_mut().unwrap().blocks[0]
        .operations
        .push(store(P, P));
    // Deliberately malformed: role validation must not exempt a ValueId globally.
    assert_eq!(
        detail(&run(&function, LIMIT, LIMIT).0),
        "scoped source-slot address escapes through an unsupported operand"
    );
}

#[test]
fn malformed_cfg_arity_types_and_ungrounded_pointer_cycles_refuse() {
    for case in 0..3 {
        let mut function = loop_fixture(P);
        let body = function.body.as_mut().unwrap();
        match case {
            0 => {
                if let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                    &mut body.blocks[0].terminator
                {
                    then_arguments.clear();
                }
            }
            1 => {
                if let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
                    &mut body.blocks[0].terminator
                {
                    then_arguments[0] = ValueId(1);
                }
            }
            2 => {
                body.blocks[0].terminator = Some(Terminator::Return { values: vec![] });
                // Unreachable, ungrounded incoming cycle is still checked.
                body.blocks[1].terminator = Some(Terminator::Branch {
                    target: BlockId(900),
                    arguments: vec![ValueId(100)],
                });
            }
            _ => unreachable!(),
        }
        let result = run(&function, LIMIT, LIMIT).0;
        assert_eq!(
            detail(&result),
            match case {
                0 => "scoped source-slot CFG transport has an incompatible signature",
                1 => "scoped source-slot pointer access has an incompatible type",
                2 => "scoped source-slot pointer transport is ambiguous",
                _ => unreachable!(),
            }
        );
    }
}

#[test]
fn candidate_access_type_space_and_restriction_mutations_refuse() {
    for case in 0..4 {
        let mut function = fixture();
        let bad = match case {
            0 => result(
                10,
                pointer(AccessMode::ReadWrite),
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: P,
                    to: pointer(AccessMode::ReadWrite),
                },
            ),
            1 => result(
                10,
                scalar(),
                OperationKind::Load {
                    pointer: P,
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            2 => load(10, P, Type::F32),
            3 => result(
                10,
                pointer(AccessMode::ReadWrite),
                OperationKind::GetElementPointer {
                    base: P,
                    offset: ValueId(0),
                },
            ),
            _ => unreachable!(),
        };
        function.body.as_mut().unwrap().blocks[0]
            .operations
            .push(bad);
        assert_eq!(
            detail(&run(&function, LIMIT, LIMIT).0),
            match case {
                0 => "scoped source-slot restriction changes address space or widens access",
                1 => "scoped source-slot memory access is incompatible",
                2 => "scoped source-slot pointer access has an incompatible type",
                3 => "scoped source-slot offset is not an integer",
                _ => unreachable!(),
            }
        );
    }
}

#[test]
fn a_grounded_loop_alias_cannot_escape_through_a_return() {
    let mut function = loop_fixture(P);
    function
        .signature
        .results
        .push(pointer(AccessMode::ReadWrite));
    function.body.as_mut().unwrap().blocks[2].terminator = Some(Terminator::Return {
        values: vec![ValueId(100)],
    });
    verified(&function);
    assert_eq!(
        detail(&run(&function, LIMIT, LIMIT).0),
        "scoped source-slot address escapes through an unsupported operand"
    );
}

#[test]
fn simple_slot_use_fixture_has_independently_counted_resource_limits() {
    let function = fixture();
    verified(&function);
    // Scratch wrapper, index, body, remaining construction, solve and use checks.
    let work = 2 + 157 + 13 + 31 + 78 + 22;
    assert_eq!(work, 303);
    let peak = FLOOR
        + std::mem::size_of::<BlockId>()
        + 7 * std::mem::size_of::<(ValueId, &Type)>()
        + std::mem::size_of::<(BlockId, &BasicBlock)>()
        + 14 * std::mem::size_of::<Origin>()
        + 14 * std::mem::size_of::<usize>()
        + 7 * std::mem::size_of::<bool>();
    let (result, spent, actual_peak) = run(&function, work, peak);
    result.unwrap();
    assert_eq!((spent, actual_peak), (work, peak));
    assert!(matches!(
        run(&function, work - 1, peak).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
    assert!(matches!(
        run(&function, work, peak - 1).0,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn assembly_operands_without_values_are_charged_for_each_scan() {
    use fe2o3_kernel_ir::{
        AssemblyConstraint, AssemblyOperand, AssemblyOperandKind, AssemblySourceIdentity,
        InlineAssembly, InlineAssemblyTarget,
    };
    let mut previous = None;
    for count in [0, 17] {
        let mut function = fixture();
        // Inert metering-only payload, not an admitted assembly/source fixture.
        function.body.as_mut().unwrap().blocks[0]
            .operations
            .push(Operation::new(
                vec![],
                OperationKind::InlineAssembly(InlineAssembly {
                    target: InlineAssemblyTarget::AmdGpuGfx942,
                    source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                    mnemonic: "metering-only".into(),
                    operands: vec![
                        AssemblyOperand {
                            kind: AssemblyOperandKind::ImmediateI32(7),
                            constraint: AssemblyConstraint::ImmediateI32,
                        };
                        count
                    ],
                    options: BTreeSet::new(),
                    declared_effects: BTreeSet::new(),
                }),
            ));
        let (result, work, peak) = run(&function, LIMIT, LIMIT);
        result.unwrap();
        if let Some((old_work, old_peak)) = previous {
            assert_eq!(work - old_work, 2 * count);
            assert_eq!(peak, old_peak);
            run(&function, work, peak).0.unwrap();
            assert!(matches!(
                run(&function, work - 1, peak).0,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        }
        previous = Some((work, peak));
    }
}

#[test]
fn address_use_analysis_restores_storage_at_exact_resource_boundaries() {
    let function = loop_fixture(P);
    verified(&function);
    let (result, work, peak) = run(&function, LIMIT, LIMIT);
    result.unwrap();
    run(&function, work, peak).0.unwrap();
    for (work, storage, expect_work) in [(work - 1, peak, true), (work, peak - 1, false)] {
        let result = run(&function, work, storage).0;
        assert!(
            matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )) if expect_work
            ) || matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )) if !expect_work
            ),
            "{result:?}"
        );
    }
}

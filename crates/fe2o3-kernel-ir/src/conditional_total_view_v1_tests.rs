use super::*;
use crate::{
    BasicBlock, BinaryOp, CanonicalKernelIrWorkBudgetV1, FunctionBody, IntrinsicOperation,
    Signature, SwitchCase, ValueDef, verify_module_ref,
};

fn scalar() -> Type {
    Type::Scalar(ScalarType::U32)
}
fn pointer() -> Type {
    Type::pointer(scalar(), AddressSpace::Global, AccessMode::ReadWrite)
}
fn op(value: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(value), ty), kind)
}
fn ret() -> Terminator {
    Terminator::Return { values: vec![] }
}
fn branch(target: u32) -> Terminator {
    Terminator::Branch {
        target: BlockId(target),
        arguments: vec![],
    }
}
fn conditional(condition: u32, yes: u32, no: u32) -> Terminator {
    Terminator::ConditionalBranch {
        condition: ValueId(condition),
        then_target: BlockId(yes),
        then_arguments: vec![],
        else_target: BlockId(no),
        else_arguments: vec![],
    }
}
fn store(pointer: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(pointer),
            value: ValueId(1),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}
fn body(module: &mut Module) -> &mut FunctionBody {
    module.functions[0].body.as_mut().unwrap()
}

fn fixture() -> Module {
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        op(
            10,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            11,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            12,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
        op(13, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        op(
            14,
            Type::INDEX,
            OperationKind::Select {
                condition: ValueId(12),
                true_value: ValueId(10),
                false_value: ValueId(13),
            },
        ),
        op(
            15,
            pointer(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            16,
            pointer(),
            OperationKind::GetElementPointer {
                base: ValueId(15),
                offset: ValueId(14),
            },
        ),
    ];
    entry.terminator = Some(conditional(12, 20, 30));
    let mut yes = BasicBlock::new(BlockId(20));
    yes.operations.push(store(16));
    yes.terminator = Some(ret());
    let mut no = BasicBlock::new(BlockId(30));
    no.terminator = Some(ret());
    let slice = Type::slice(scalar(), AddressSpace::Global, AccessMode::ReadWrite);
    let mut module = Module::new("conditional-total-view");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(vec![slice.clone(), scalar(), Type::BOOL, slice], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![entry, yes, no],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn analyze(module: &Module) -> ConditionalTotalViewAnalysisV1<'_> {
    let verified = verify_module_ref(module).expect("fixture is genuine verified canonical KIR");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(37).unwrap();
    let result = derive_conditional_total_view_from_verified_v1(
        verified,
        &KernelId::new("kernel"),
        &mut budget,
    )
    .unwrap();
    assert_eq!(
        budget.storage(),
        37,
        "scratch must not release caller storage"
    );
    assert!(budget.work() > 0);
    result
}

fn refused(module: &Module) -> Unsupported {
    let result = analyze(module);
    assert!(result.facts().is_none(), "{result:?}");
    result.unsupported_reason().expect("unsupported fragment")
}

#[test]
fn dynamic_identity_reports_exact_borrowed_subject_without_a_launch_sample() {
    let module = fixture();
    let result = analyze(&module);
    assert_eq!(result.unsupported_reason(), None);
    let facts = result.facts().expect("conditional identity theorem");
    assert!(std::ptr::eq(facts.module(), &module));
    assert!(std::ptr::eq(facts.function(), &module.functions[0]));
    assert!(std::ptr::eq(facts.kernel(), &module.kernels[0]));
    assert_eq!((facts.kernel_ordinal(), facts.function_ordinal()), (0, 0));
    assert_eq!(
        (facts.output_parameter_index(), facts.output_value()),
        (0, ValueId(0))
    );
    assert_eq!(
        (facts.index(), facts.length(), facts.predicate()),
        (ValueId(10), ValueId(11), ValueId(12))
    );
    assert_eq!(
        (facts.pointer(), facts.offset(), facts.store_value()),
        (ValueId(16), ValueId(14), ValueId(1))
    );
    assert_eq!(facts.store_predicate(), None);
    assert_eq!(
        facts.address_domain(),
        ConditionalTotalViewAddressDomainV1::GuardedOutput
    );
    assert_eq!(
        facts.store_location(),
        FunctionOperationLocation::new(BlockId(20), 0)
    );
    assert_eq!((facts.element_bytes(), facts.alignment()), (4, 4));
    let debug = format!("{result:?}");
    assert!(debug.len() < 1024);
    assert!(!debug.contains("FunctionBody"));
    assert!(!debug.contains("conditional-total-view"));
    // The theorem has no sampled length, nonempty-domain claim, or fake grid.
    assert_eq!(
        facts.kernel().domain,
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic
        }
    );
}

#[test]
fn write_only_slice_and_pointer_support_the_same_ordinary_store_theorem() {
    let mut module = fixture();
    module.functions[0].signature.parameters[0] =
        Type::slice(scalar(), AddressSpace::Global, AccessMode::WriteOnly);
    for ordinal in [5, 6] {
        body(&mut module).blocks[0].operations[ordinal].results[0].ty =
            Type::pointer(scalar(), AddressSpace::Global, AccessMode::WriteOnly);
    }
    let result = analyze(&module);
    let facts = result.facts().expect("verified write-only output store");
    assert_eq!(facts.output_parameter_index(), 0);
    assert_eq!(facts.output_value(), ValueId(0));
    assert_eq!(facts.store_predicate(), None);
    assert_eq!(
        facts.store_location(),
        FunctionOperationLocation::new(BlockId(20), 0)
    );
}

#[test]
fn direct_address_retains_the_domain_required_by_its_evaluation_paths() {
    let mut module = fixture();
    let mut gep = body(&mut module).blocks[0].operations.pop().unwrap();
    let OperationKind::GetElementPointer { offset, .. } = &mut gep.kind else {
        panic!("GEP");
    };
    *offset = ValueId(10);
    body(&mut module).blocks[1].operations.insert(0, gep);
    let result = analyze(&module);
    let facts = result.facts().expect("direct guarded address");
    assert_eq!(facts.offset(), ValueId(10));
    assert_eq!(
        facts.address_domain(),
        ConditionalTotalViewAddressDomainV1::GuardedOutput
    );
    assert_eq!(
        facts.store_location(),
        FunctionOperationLocation::new(BlockId(20), 1)
    );

    let gep = body(&mut module).blocks[1].operations.remove(0);
    body(&mut module).blocks[0].operations.push(gep);
    let result = analyze(&module);
    let facts = result
        .facts()
        .expect("caller must admit whole-launch arithmetic");
    assert_eq!(facts.offset(), ValueId(10));
    assert_eq!(
        facts.address_domain(),
        ConditionalTotalViewAddressDomainV1::GlobalLaunch
    );
    assert_eq!(
        facts.store_location(),
        FunctionOperationLocation::new(BlockId(20), 0)
    );

    body(&mut module).blocks[0].terminator = Some(conditional(12, 30, 20));
    assert!(matches!(refused(&module), Unsupported::WriteCount { .. }));
}

#[test]
fn explicit_guarded_store_has_separate_must_execute_analysis() {
    let mut module = fixture();
    body(&mut module).blocks[1].operations.clear();
    body(&mut module).blocks[0].operations.push(Operation::new(
        vec![],
        OperationKind::GuardedStore {
            pointer: ValueId(16),
            value: ValueId(1),
            predicate: ValueId(12),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    assert_eq!(
        analyze(&module).facts().unwrap().store_predicate(),
        Some(ValueId(12))
    );

    let guarded = body(&mut module).blocks[0].operations.pop().unwrap();
    body(&mut module).blocks[1].operations.push(guarded);
    body(&mut module).blocks[0].terminator = Some(conditional(2, 20, 30));
    assert!(matches!(
        refused(&module),
        Unsupported::WriteCount {
            predicate_true: true,
            ..
        }
    ));
}

#[test]
fn bool_switch_prunes_default_only_from_the_exact_selector() {
    let mut module = fixture();
    body(&mut module).blocks[0].operations.push(op(
        17,
        Type::Scalar(ScalarType::U64),
        OperationKind::Cast {
            kind: CastKind::ZeroExtend,
            value: ValueId(12),
            to: Type::Scalar(ScalarType::U64),
        },
    ));
    body(&mut module).blocks[0].terminator = Some(Terminator::Switch {
        selector: ValueId(17),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(30),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(20),
                arguments: vec![],
            },
        ],
        default_target: BlockId(40),
        default_arguments: vec![],
    });
    let mut impossible = BasicBlock::new(BlockId(40));
    impossible.terminator = Some(Terminator::Unreachable);
    body(&mut module).blocks.push(impossible);
    assert!(analyze(&module).facts().is_some());

    let Some(Terminator::Switch { cases, .. }) = &mut body(&mut module).blocks[0].terminator else {
        panic!("switch");
    };
    cases.swap(0, 1);
    assert!(
        analyze(&module).facts().is_some(),
        "reversed complete cases still exclude the default"
    );

    let OperationKind::Cast { value, .. } = &mut body(&mut module).blocks[0]
        .operations
        .last_mut()
        .unwrap()
        .kind
    else {
        panic!("cast");
    };
    *value = ValueId(2);
    assert!(matches!(refused(&module), Unsupported::Terminator { .. }));
}

#[test]
fn sparse_nonmonotonic_ids_and_non_topological_block_order_are_supported() {
    let mut module = fixture();
    let entry_id = 900_000_001;
    let write_id = 17;
    let tail_id = 4_000_000_001;
    let index_id = u32::MAX - 9;
    let blocks = &mut body(&mut module).blocks;
    blocks[0].id = BlockId(entry_id);
    blocks[0].terminator = Some(conditional(12, write_id, tail_id));
    blocks[0].operations[0].results[0].id = ValueId(index_id);
    blocks[0].operations[1].results[0].id = ValueId(4000);
    blocks[0].operations[2].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(index_id),
        rhs: ValueId(4000),
    };
    blocks[0].operations[4].kind = OperationKind::Select {
        condition: ValueId(12),
        true_value: ValueId(index_id),
        false_value: ValueId(13),
    };
    blocks[1].id = BlockId(write_id);
    blocks[1].terminator = Some(branch(5));
    blocks[2].id = BlockId(tail_id);
    let mut joined = BasicBlock::new(BlockId(5));
    joined.terminator = Some(ret());
    blocks.insert(1, joined);
    let result = analyze(&module);
    let facts = result
        .facts()
        .expect("IDs and physical order are not CFG ordinals");
    assert_eq!(facts.index(), ValueId(index_id));
    assert_eq!(facts.length(), ValueId(4000));
    assert_eq!(
        facts.store_location(),
        FunctionOperationLocation::new(BlockId(write_id), 0)
    );
}

#[test]
fn duplicate_branch_targets_preserve_each_edge_occurrence() {
    let mut module = fixture();
    let store = body(&mut module).blocks[1].operations.pop().unwrap();
    body(&mut module).blocks[1].terminator = Some(conditional(2, 40, 40));
    let mut joined = BasicBlock::new(BlockId(40));
    joined.operations.push(store);
    joined.terminator = Some(ret());
    body(&mut module).blocks.push(joined);
    assert!(analyze(&module).facts().is_some());
}

#[test]
fn switch_default_sharing_a_case_target_does_not_change_the_selected_edge() {
    for (default_target, both_cases_share_target) in [(20, false), (30, false), (20, true)] {
        let mut module = fixture();
        body(&mut module).blocks[0].operations.push(op(
            17,
            Type::Scalar(ScalarType::U64),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(12),
                to: Type::Scalar(ScalarType::U64),
            },
        ));
        body(&mut module).blocks[0].terminator = Some(Terminator::Switch {
            selector: ValueId(17),
            cases: vec![
                SwitchCase {
                    value: 1,
                    target: BlockId(20),
                    arguments: vec![],
                },
                SwitchCase {
                    value: 0,
                    target: BlockId(if both_cases_share_target { 20 } else { 30 }),
                    arguments: vec![],
                },
            ],
            default_target: BlockId(default_target),
            default_arguments: vec![],
        });
        if both_cases_share_target {
            body(&mut module).blocks.pop();
            body(&mut module).blocks[1].operations[0].kind = OperationKind::GuardedStore {
                pointer: ValueId(16),
                value: ValueId(1),
                predicate: ValueId(12),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            };
        }
        assert!(
            analyze(&module).facts().is_some(),
            "default {default_target}, shared cases {both_cases_share_target}"
        );
        if both_cases_share_target {
            body(&mut module).blocks[1].operations[0] = store(16);
            assert!(matches!(
                refused(&module),
                Unsupported::WriteCount {
                    predicate_true: false,
                    ..
                }
            ));
        }
    }
}

#[test]
fn branch_joins_union_counts_without_summing_alternative_paths() {
    let mut module = fixture();
    let store = body(&mut module).blocks[1].operations.pop().unwrap();
    body(&mut module).blocks[1].terminator = Some(conditional(2, 40, 50));
    for id in [40, 50] {
        let mut block = BasicBlock::new(BlockId(id));
        block.terminator = Some(branch(60));
        body(&mut module).blocks.push(block);
    }
    let mut joined = BasicBlock::new(BlockId(60));
    joined.operations.push(store);
    joined.terminator = Some(ret());
    body(&mut module).blocks.push(joined);
    assert!(analyze(&module).facts().is_some());

    body(&mut module).blocks[4].terminator = Some(ret());
    assert!(matches!(
        refused(&module),
        Unsupported::WriteCount {
            predicate_true: true,
            ..
        }
    ));
}

#[test]
fn bypass_extra_predicate_and_reversed_polarity_do_not_establish_coverage() {
    let mut bypass = fixture();
    body(&mut bypass).blocks[0].terminator = Some(conditional(2, 40, 30));
    let mut guarded_path = BasicBlock::new(BlockId(40));
    guarded_path.terminator = Some(conditional(12, 20, 30));
    body(&mut bypass).blocks.push(guarded_path);
    assert!(matches!(
        refused(&bypass),
        Unsupported::WriteCount {
            predicate_true: true,
            ..
        }
    ));
    for terminator in [conditional(2, 20, 30), conditional(12, 30, 20)] {
        let mut module = fixture();
        body(&mut module).blocks[0].terminator = Some(terminator);
        assert!(matches!(refused(&module), Unsupported::WriteCount { .. }));
    }
    let mut module = fixture();
    let store = body(&mut module).blocks[1].operations.pop().unwrap();
    body(&mut module).blocks[1].terminator = Some(conditional(2, 40, 30));
    let mut extra_guard = BasicBlock::new(BlockId(40));
    extra_guard.operations.push(store);
    extra_guard.terminator = Some(ret());
    body(&mut module).blocks.push(extra_guard);
    assert!(matches!(
        refused(&module),
        Unsupported::WriteCount {
            predicate_true: true,
            ..
        }
    ));
}

#[test]
fn wrong_length_index_guard_and_selected_offset_are_unsupported() {
    for mutation in 0..5 {
        let mut module = fixture();
        let entry = &mut body(&mut module).blocks[0];
        match mutation {
            0 => entry.operations[1].kind = OperationKind::SliceLength { slice: ValueId(3) },
            1 => {
                entry.operations[0].kind = OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Local,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                ))
            }
            2 => {
                entry.operations[2].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(10),
                    rhs: ValueId(11),
                }
            }
            3 => {
                entry.operations[4].kind = OperationKind::Select {
                    condition: ValueId(12),
                    true_value: ValueId(13),
                    false_value: ValueId(10),
                }
            }
            4 => entry.operations[3].kind = OperationKind::Constant(Constant::Index(1)),
            _ => unreachable!(),
        }
        assert!(analyze(&module).facts().is_none(), "mutation {mutation}");
    }
    let mut module = fixture();
    body(&mut module).blocks[1].operations[0].kind = OperationKind::GuardedStore {
        pointer: ValueId(16),
        value: ValueId(1),
        predicate: ValueId(2),
        access: MemoryAccess::new(AddressSpace::Global, 4),
    };
    assert_eq!(
        refused(&module),
        Unsupported::Predicate { value: ValueId(2) }
    );
}

#[test]
fn duplicate_and_additional_output_writes_are_explicitly_outside_the_fragment() {
    for other_output in [false, true] {
        let mut module = fixture();
        if other_output {
            body(&mut module).blocks[1].operations.extend([
                op(
                    24,
                    pointer(),
                    OperationKind::SliceData { slice: ValueId(3) },
                ),
                op(
                    25,
                    pointer(),
                    OperationKind::GetElementPointer {
                        base: ValueId(24),
                        offset: ValueId(14),
                    },
                ),
                store(25),
            ]);
        } else {
            body(&mut module).blocks[1].operations.push(store(16));
        }
        assert_eq!(refused(&module), Unsupported::StoreCount { actual: 2 });
    }
}

#[test]
fn abnormal_completion_before_after_and_outside_the_write_is_rejected() {
    let mut after = fixture();
    body(&mut after).blocks[1].terminator = Some(Terminator::Unreachable);
    assert_eq!(
        refused(&after),
        Unsupported::AbnormalExit { block: BlockId(20) }
    );
    let mut tail = fixture();
    body(&mut tail).blocks[2].terminator = Some(Terminator::Unreachable);
    assert_eq!(
        refused(&tail),
        Unsupported::AbnormalExit { block: BlockId(30) }
    );
    let mut before = fixture();
    body(&mut before).blocks[1].terminator = Some(conditional(2, 40, 50));
    let store = body(&mut before).blocks[1].operations.pop().unwrap();
    let mut write = BasicBlock::new(BlockId(40));
    write.operations.push(store);
    write.terminator = Some(ret());
    let mut trap = BasicBlock::new(BlockId(50));
    trap.terminator = Some(Terminator::Unreachable);
    body(&mut before).blocks.extend([write, trap]);
    assert!(matches!(refused(&before), Unsupported::AbnormalExit { .. }));
}

#[test]
fn cycles_block_arguments_and_multidimensional_launches_fail_closed() {
    let mut cycle = fixture();
    body(&mut cycle).blocks[1].terminator = Some(branch(20));
    assert_eq!(refused(&cycle), Unsupported::Cycle);
    let mut arguments = fixture();
    body(&mut arguments).blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(30), Type::INDEX));
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
        &mut body(&mut arguments).blocks[0].terminator
    else {
        panic!("branch");
    };
    then_arguments.push(ValueId(10));
    assert_eq!(
        refused(&arguments),
        Unsupported::BlockArguments { block: BlockId(20) }
    );
    for domain in [
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
        LaunchDomain::D2 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Static(2),
        },
    ] {
        let mut module = fixture();
        module.kernels[0].domain = domain;
        assert_eq!(refused(&module), Unsupported::LaunchDomain);
    }
}

#[test]
fn calls_reads_volatile_stores_and_unproved_arithmetic_are_not_completion_facts() {
    let mut calls = fixture();
    let mut helper = BasicBlock::new(BlockId(0));
    helper.terminator = Some(ret());
    calls.functions.push(Function::internal_helper(
        "helper",
        Signature::new(vec![], vec![]),
        vec![],
        vec![helper],
    ));
    body(&mut calls).blocks[0].operations.push(Operation::new(
        vec![],
        OperationKind::Call {
            callee: "helper".into(),
            arguments: vec![],
        },
    ));
    assert!(matches!(refused(&calls), Unsupported::Call { .. }));

    let mut reads = fixture();
    body(&mut reads).blocks[1].operations.push(op(
        30,
        scalar(),
        OperationKind::Load {
            pointer: ValueId(16),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    assert!(matches!(refused(&reads), Unsupported::Operation { .. }));
    let mut volatile = fixture();
    let OperationKind::Store { access, .. } = &mut body(&mut volatile).blocks[1].operations[0].kind
    else {
        panic!("store");
    };
    access.volatile = true;
    assert!(matches!(
        refused(&volatile),
        Unsupported::StoreAccess { .. }
    ));

    for operation in [BinaryOp::Add, BinaryOp::Divide, BinaryOp::Multiply] {
        let mut arithmetic = fixture();
        body(&mut arithmetic).blocks[0].operations.extend([
            op(
                30,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(u64::MAX)),
            ),
            op(
                31,
                Type::INDEX,
                OperationKind::Binary {
                    op: operation,
                    lhs: ValueId(30),
                    rhs: ValueId(10),
                },
            ),
        ]);
        assert!(matches!(
            refused(&arithmetic),
            Unsupported::Operation { .. }
        ));
    }
}

#[test]
fn floating_arithmetic_and_structurally_unreachable_blocks_fail_closed() {
    let mut floating = fixture();
    body(&mut floating).blocks[0].operations.extend([
        op(30, Type::F32, OperationKind::Constant(Constant::F32Bits(0))),
        op(
            31,
            Type::F32,
            OperationKind::Binary {
                op: BinaryOp::Divide,
                lhs: ValueId(30),
                rhs: ValueId(30),
            },
        ),
    ]);
    assert!(matches!(refused(&floating), Unsupported::Operation { .. }));
    let mut unreachable = fixture();
    let mut dead = BasicBlock::new(BlockId(40));
    dead.terminator = Some(Terminator::Unreachable);
    body(&mut unreachable).blocks.push(dead);
    assert_eq!(
        refused(&unreachable),
        Unsupported::UnreachableBlock { block: BlockId(40) }
    );
}

#[test]
fn exact_work_and_storage_boundaries_preserve_the_callers_floor() {
    let module = fixture();
    let verified = verify_module_ref(&module).unwrap();
    let kernel = KernelId::new("kernel");
    let floor = 123;
    let prefix = 17;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.charge_work(prefix).unwrap();
    budget.reserve_storage(floor).unwrap();
    assert!(
        derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget)
            .unwrap()
            .facts()
            .is_some()
    );
    let exact_work = budget.work();
    let exact_storage = budget.peak_storage();
    assert_eq!(budget.storage(), floor);

    for (work_limit, storage_limit, success) in [
        (exact_work, exact_storage, true),
        (exact_work - 1, exact_storage, false),
        (exact_work, exact_storage - 1, false),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.charge_work(prefix).unwrap();
        budget.reserve_storage(floor).unwrap();
        let identity = budget.work_ledger_identity_v1();
        let result = derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget);
        assert_eq!(result.is_ok(), success, "{result:?}");
        if success {
            assert_eq!(budget.work(), exact_work);
            assert_eq!(budget.peak_storage(), exact_storage);
        } else if work_limit < exact_work {
            assert!(matches!(
                result,
                Err(ConditionalTotalViewErrorV1::Resource(ResourceError::Work(
                    _
                )))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ConditionalTotalViewErrorV1::Resource(
                    ResourceError::Storage(_)
                ))
            ));
        }
        assert!(budget.work() >= prefix);
        assert_eq!(budget.storage(), floor);
        assert!(identity == budget.work_ledger_identity_v1());
    }
}

#[test]
fn every_work_failure_and_partial_storage_failure_cleans_up() {
    let module = fixture();
    let verified = verify_module_ref(&module).unwrap();
    let kernel = KernelId::new("kernel");
    let floor = 11;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget).unwrap();
    let required_work = budget.work();
    let peak = budget.peak_storage();
    for limit in 0..required_work {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget),
            Err(ConditionalTotalViewErrorV1::Resource(ResourceError::Work(
                _
            )))
        ));
        assert_eq!(budget.storage(), floor, "work limit {limit}");
    }
    for limit in [floor, floor + (peak - floor) / 2, peak - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = Budget::new(&mut work, limit);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            derive_conditional_total_view_from_verified_v1(verified, &kernel, &mut budget),
            Err(ConditionalTotalViewErrorV1::Resource(
                ResourceError::Storage(_)
            ))
        ));
        assert_eq!(budget.storage(), floor, "storage limit {limit}");
    }
}

#[test]
fn output_coverage_does_not_establish_reference_value_equivalence() {
    let mut module = fixture();
    let original_value = analyze(&module).facts().unwrap().store_value();
    body(&mut module).blocks[0].operations.push(op(
        31,
        scalar(),
        OperationKind::Constant(Constant::U32(99)),
    ));
    let OperationKind::Store { value, .. } = &mut body(&mut module).blocks[1].operations[0].kind
    else {
        panic!("store");
    };
    *value = ValueId(31);
    let result = analyze(&module);
    let facts = result.facts().expect("unchanged coverage, different value");
    assert_eq!(facts.store_value(), ValueId(31));
    assert_ne!(facts.store_value(), original_value);
}

#[test]
fn repeated_calls_accumulate_work_and_keep_subjects_distinct() {
    let first = fixture();
    let second = fixture();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(91).unwrap();
    let first_result = derive_conditional_total_view_from_verified_v1(
        verify_module_ref(&first).unwrap(),
        &KernelId::new("kernel"),
        &mut budget,
    )
    .unwrap();
    let first_work = budget.work();
    let second_result = derive_conditional_total_view_from_verified_v1(
        verify_module_ref(&second).unwrap(),
        &KernelId::new("kernel"),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.work(), first_work * 2);
    assert_eq!(budget.storage(), 91);
    assert!(!std::ptr::eq(
        first_result.facts().unwrap().module(),
        second_result.facts().unwrap().module()
    ));
    let missing = derive_conditional_total_view_from_verified_v1(
        verify_module_ref(&first).unwrap(),
        &KernelId::new("absent"),
        &mut budget,
    )
    .unwrap();
    assert_eq!(missing.unsupported_reason(), Some(Unsupported::Kernel));
    assert_eq!(budget.storage(), 91);
}

#[test]
fn overflowing_allocation_formula_is_a_typed_resource_failure() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(5).unwrap();
    assert!(matches!(
        allocate::<u64>(usize::MAX, &mut budget),
        Err(Failure::Error(ConditionalTotalViewErrorV1::Resource(
            ResourceError::Arithmetic
        )))
    ));
    assert_eq!(budget.storage(), 5);
}

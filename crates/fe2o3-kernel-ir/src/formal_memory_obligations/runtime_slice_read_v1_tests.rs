use super::*;

#[path = "runtime_slice_read_integration_hostile_v1_tests.rs"]
mod integration_hostile_tests;
#[path = "runtime_slice_read_representation_v1_tests.rs"]
mod representation_tests;
use crate::{BasicBlock, Kernel, Signature, ValueDef};

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture(scalar: ScalarType, mode: AccessMode) -> Module {
    let element = Type::Scalar(scalar);
    let width = scalar_byte_width(scalar).unwrap() as u32;
    let slice = Type::slice(element.clone(), AddressSpace::Global, mode);
    let pointer = Type::pointer(element.clone(), AddressSpace::Global, mode);
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        op(
            6,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            7,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(6),
            },
        ),
        op(
            8,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut read = BasicBlock::new(BlockId(20));
    read.operations = vec![
        op(
            9,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(8),
                offset: ValueId(1),
            },
        ),
        op(
            10,
            element.clone(),
            OperationKind::Load {
                pointer: ValueId(9),
                access: MemoryAccess::new(AddressSpace::Global, width),
            },
        ),
    ];
    read.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("runtime-read");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                slice.clone(),
                Type::INDEX,
                Type::INDEX,
                slice,
                Type::BOOL,
                element,
            ],
            vec![],
        ),
        (0..6).map(ValueId).collect(),
        vec![entry, read, exit],
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

fn analyze(module: &Module) -> FormalMemoryObligationAnalysis {
    let verified = verify_module_ref(module).expect("genuine independently verified input");
    crate::derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits64,
    )
    .unwrap()
}

fn domain(analysis: &FormalMemoryObligationAnalysis) -> FormalRuntimeSliceReadDomainV1 {
    assert!(analysis.is_complete(), "{analysis:?}");
    let [access] = analysis.obligations().accesses() else {
        panic!("one actual read")
    };
    let FormalAccessDomainV1::RuntimeSliceReadBounded(domain) = access.domain() else {
        panic!("runtime read, not an affine address")
    };
    domain
}

fn unsupported(module: &Module) {
    let analysis = analyze(module);
    assert!(!analysis.is_complete(), "{analysis:?}");
    assert!(
        analysis.incomplete_reasons().iter().any(|reason| matches!(
            reason,
            FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }
        )),
        "{analysis:?}"
    );
}

#[test]
fn opaque_scalar_reads_keep_exact_symbolic_bounds_without_affine_injectivity() {
    for scalar in [
        ScalarType::U8,
        ScalarType::I8,
        ScalarType::U16,
        ScalarType::I16,
        ScalarType::U32,
        ScalarType::I32,
        ScalarType::U64,
        ScalarType::I64,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        for mode in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            let analysis = analyze(&fixture(scalar, mode));
            let domain = domain(&analysis);
            assert_eq!(domain.allocation().parameter_index(), 0);
            assert_eq!(domain.slice(), ValueId(0));
            assert_eq!(domain.index(), ValueId(1));
            assert_eq!(domain.guard_index(), ValueId(1));
            assert_eq!(domain.length(), ValueId(6));
            assert_eq!(domain.predicate(), ValueId(7));
            assert_eq!(domain.pointer(), ValueId(9));
            assert_eq!(domain.element_bytes(), scalar_byte_width(scalar).unwrap());
            assert_eq!(
                domain.path(),
                FormalGuardedPathV1::TrueEdge {
                    source: BlockId(10),
                    ordinal: 0,
                    target: BlockId(20)
                }
            );
            let report = analysis.obligations();
            assert_eq!(
                report.accesses()[0].byte_offset(),
                ByteExpression::Unbounded
            );
            assert_eq!(report.accesses()[0].kind(), FormalMemoryAccessKind::Read);
            assert_eq!(
                report.accesses()[0].location(),
                FunctionOperationLocation::new(BlockId(20), 1)
            );
            assert_eq!(
                report.bounds_requirements()[0].kind(),
                FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(domain)
            );
            assert_eq!(report.bounds_requirements()[0].minimum_byte_len(), None);
            assert!(!report.bounds_requirements()[0].is_met_by_untrusted_byte_len(u64::MAX));
            assert!(report.inter_invocation_conflicts().is_empty());
        }
    }
}

#[test]
fn a_checked_index_producer_is_opaque_and_only_its_actual_result_is_guarded() {
    let mut module = fixture(ScalarType::F32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        0,
        Operation::checked_binary(
            ValueDef::new(ValueId(40), Type::INDEX),
            ValueDef::new(ValueId(41), Type::BOOL),
            crate::CheckedBinaryOperator::Multiply,
            ValueId(1),
            ValueId(2),
        ),
    );
    let OperationKind::Compare { lhs, .. } = &mut body.blocks[0].operations[2].kind else {
        unreachable!()
    };
    *lhs = ValueId(40);
    let OperationKind::GetElementPointer { offset, .. } = &mut body.blocks[1].operations[0].kind
    else {
        unreachable!()
    };
    *offset = ValueId(40);
    assert_eq!(domain(&analyze(&module)).index(), ValueId(40));
}

fn wrong_index(module: &mut Module) {
    let OperationKind::Compare { lhs, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *lhs = ValueId(2);
}
fn wrong_slice(module: &mut Module) {
    let OperationKind::SliceLength { slice } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    *slice = ValueId(3);
}
fn non_strict(module: &mut Module) {
    let OperationKind::Compare { predicate, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[1].kind
    else {
        unreachable!()
    };
    *predicate = ComparePredicate::LessThanOrEqual;
}
fn false_edge(module: &mut Module) {
    let Some(Terminator::ConditionalBranch {
        then_target,
        else_target,
        ..
    }) = &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    std::mem::swap(then_target, else_target);
}
fn duplicate_edge_bypass(module: &mut Module) {
    let Some(Terminator::ConditionalBranch { else_target, .. }) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].terminator
    else {
        unreachable!()
    };
    *else_target = BlockId(20);
}
fn before_guard(module: &mut Module) {
    let body = module.functions[0].body.as_mut().unwrap();
    let operations = std::mem::take(&mut body.blocks[1].operations);
    body.blocks[0].operations.extend(operations);
}

#[test]
fn exact_pair_polarity_and_success_edge_are_mandatory() {
    for mutate in [
        wrong_index as fn(&mut Module),
        wrong_slice,
        non_strict,
        false_edge,
        duplicate_edge_bypass,
        before_guard,
    ] {
        let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
        mutate(&mut module);
        unsupported(&module);
    }
}

#[test]
fn a_new_post_guard_index_cannot_reuse_the_old_guard() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let read = &mut module.functions[0].body.as_mut().unwrap().blocks[1];
    read.operations.insert(
        0,
        Operation::checked_binary(
            ValueDef::new(ValueId(40), Type::INDEX),
            ValueDef::new(ValueId(41), Type::BOOL),
            crate::CheckedBinaryOperator::Add,
            ValueId(1),
            ValueId(2),
        ),
    );
    let OperationKind::GetElementPointer { offset, .. } = &mut read.operations[1].kind else {
        unreachable!()
    };
    *offset = ValueId(40);
    unsupported(&module);
}

#[test]
fn every_actual_consumer_requires_its_own_success_path() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    let gep = body.blocks[1].operations.remove(0);
    body.blocks[0].operations.push(gep);
    body.blocks[2].operations.push(op(
        11,
        Type::Scalar(ScalarType::U32),
        OperationKind::Load {
            pointer: ValueId(9),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    let analysis = analyze(&module);
    assert!(!analysis.is_complete());
    assert_eq!(analysis.obligations().accesses().len(), 1);
    assert_eq!(
        analysis.obligations().accesses()[0].location(),
        FunctionOperationLocation::new(BlockId(20), 0)
    );
    assert!(matches!(
        analysis.obligations().accesses()[0].domain(),
        FormalAccessDomainV1::RuntimeSliceReadBounded(_)
    ));
    unsupported(&module);
}

fn write(pointer: ValueId) -> Operation {
    Operation::new(
        vec![],
        OperationKind::Store {
            pointer,
            value: ValueId(5),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    )
}

#[test]
fn the_read_proof_does_not_exempt_a_write_through_the_same_opaque_pointer() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadWrite);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(write(ValueId(9)));
    let analysis = analyze(&module);
    assert_eq!(analysis.obligations().accesses().len(), 1);
    assert_eq!(
        analysis.obligations().accesses()[0].kind(),
        FormalMemoryAccessKind::Read
    );
    unsupported(&module);
}

#[test]
fn same_allocation_read_write_conflicts_and_external_aliases_remain_conservative() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadWrite);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(write(ValueId(8)));
    let analysis = analyze(&module);
    assert!(analysis.is_complete(), "{analysis:?}");
    assert!(
        analysis
            .obligations()
            .inter_invocation_conflicts()
            .iter()
            .any(
                |conflict| conflict.left() == FunctionOperationLocation::new(BlockId(20), 1)
                    && conflict.right() == FunctionOperationLocation::new(BlockId(20), 2)
            )
    );

    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    module.functions[0].signature.parameters.push(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = module.functions[0].body.as_mut().unwrap();
    body.parameters.push(ValueId(50));
    body.blocks[1].operations.push(write(ValueId(50)));
    let analysis = analyze(&module);
    assert!(analysis.is_complete(), "{analysis:?}");
    let [alias] = analysis.obligations().runtime_alias_requirements() else {
        panic!("one conservative independent-allocation pair")
    };
    assert_eq!(
        alias.left_region(),
        FormalAliasRegionV1::WholeFormalAllocation
    );
    assert_eq!(
        alias.right_accessed_bytes(),
        Some(FormalByteRange {
            start: 0,
            end_exclusive: 4
        })
    );
}

#[test]
fn invalid_width_overalignment_and_volatile_loads_do_not_get_runtime_rows() {
    unsupported(&fixture(ScalarType::U128, AccessMode::ReadOnly));
    for (alignment, volatile) in [(8, false), (4, true)] {
        let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
        let OperationKind::Load { access, .. } =
            &mut module.functions[0].body.as_mut().unwrap().blocks[1].operations[1].kind
        else {
            unreachable!()
        };
        access.alignment = alignment;
        access.volatile = volatile;
        unsupported(&module);
    }
}

#[test]
fn old_affine_reads_keep_the_existing_domain_and_fixed_byte_bound() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.insert(
        0,
        op(
            40,
            Type::INDEX,
            OperationKind::Intrinsic(crate::IntrinsicOperation::global_id_1d()),
        ),
    );
    let OperationKind::Compare { lhs, .. } = &mut body.blocks[0].operations[2].kind else {
        unreachable!()
    };
    *lhs = ValueId(40);
    let OperationKind::GetElementPointer { offset, .. } = &mut body.blocks[1].operations[0].kind
    else {
        unreachable!()
    };
    *offset = ValueId(40);
    let analysis = analyze(&module);
    assert!(analysis.is_complete());
    let [read] = analysis.obligations().accesses() else {
        panic!("one affine read")
    };
    assert_eq!(read.domain(), FormalAccessDomainV1::LaunchEnvelope);
    assert_eq!(read.byte_offset(), ByteExpression::invocation_affine(0, 4));
    assert_eq!(
        analysis.obligations().bounds_requirements()[0].minimum_byte_len(),
        Some(256)
    );
    InertCanonicalFormalMemoryObligationReceiptV1::from_obligations(analysis.obligations())
        .unwrap();
}

#[test]
fn exact_block_argument_transport_keeps_original_and_compared_index_coordinates() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let slice = module.functions[0].signature.parameters[0].clone();
    let body = module.functions[0].body.as_mut().unwrap();
    let compare = body.blocks[0].operations.remove(1);
    let data = body.blocks[0].operations.pop().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments: vec![ValueId(1), ValueId(6), ValueId(0)],
    });
    let mut guard = BasicBlock::new(BlockId(15));
    guard.parameters = vec![
        ValueDef::new(ValueId(40), Type::INDEX),
        ValueDef::new(ValueId(41), Type::INDEX),
        ValueDef::new(ValueId(42), slice.clone()),
    ];
    guard.operations = vec![compare];
    guard.operations[0].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(40),
        rhs: ValueId(41),
    };
    guard.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![ValueId(40), ValueId(42)],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let read = &mut body.blocks[1];
    read.parameters = vec![
        ValueDef::new(ValueId(60), Type::INDEX),
        ValueDef::new(ValueId(62), slice),
    ];
    read.operations.insert(0, data);
    read.operations[0].kind = OperationKind::SliceData { slice: ValueId(62) };
    let OperationKind::GetElementPointer { offset, .. } = &mut read.operations[1].kind else {
        unreachable!()
    };
    *offset = ValueId(60);
    body.blocks.push(guard);
    let domain = domain(&analyze(&module));
    assert_eq!(domain.slice(), ValueId(0));
    assert_eq!(domain.index(), ValueId(60));
    assert_eq!(domain.guard_index(), ValueId(40));
    assert_eq!(domain.length(), ValueId(41));
}

fn loop_carrier(changing: bool) -> Module {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    let Some(Terminator::ConditionalBranch { then_target, .. }) = &mut body.blocks[0].terminator
    else {
        unreachable!()
    };
    *then_target = BlockId(15);
    let mut landing = BasicBlock::new(BlockId(15));
    landing.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![ValueId(1)],
    });
    let read = &mut body.blocks[1];
    read.parameters
        .push(ValueDef::new(ValueId(40), Type::INDEX));
    let OperationKind::GetElementPointer { offset, .. } = &mut read.operations[0].kind else {
        unreachable!()
    };
    *offset = ValueId(40);
    read.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(20),
        then_arguments: vec![if changing { ValueId(2) } else { ValueId(40) }],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    body.blocks.push(landing);
    module
}

#[test]
fn seeded_invariant_loop_carriers_are_distinct_from_changing_recurrences() {
    let analysis = analyze(&loop_carrier(false));
    assert_eq!(domain(&analysis).index(), ValueId(40));
    unsupported(&loop_carrier(true));
}

#[test]
fn duplicate_successor_argument_rows_cannot_hide_a_different_index() {
    for different in [false, true] {
        let mut module = loop_carrier(false);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[1].terminator = Some(Terminator::Return { values: vec![] });
        body.blocks[3].terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(4),
            then_target: BlockId(20),
            then_arguments: vec![ValueId(1)],
            else_target: BlockId(20),
            else_arguments: vec![if different { ValueId(2) } else { ValueId(1) }],
        });
        if different {
            unsupported(&module);
        } else {
            domain(&analyze(&module));
        }
    }
}

#[test]
fn an_enclosing_guard_remains_available_after_a_non_dominating_sibling_guard() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    let Some(Terminator::ConditionalBranch { then_target, .. }) = &mut body.blocks[0].terminator
    else {
        unreachable!()
    };
    *then_target = BlockId(11);
    let mut outer = BasicBlock::new(BlockId(11));
    outer.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(12),
        then_arguments: vec![],
        else_target: BlockId(20),
        else_arguments: vec![],
    });
    let mut inner_guard = BasicBlock::new(BlockId(12));
    inner_guard.operations.push(op(
        17,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(1),
            rhs: ValueId(6),
        },
    ));
    inner_guard.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(17),
        then_target: BlockId(13),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut inner = BasicBlock::new(BlockId(13));
    inner.terminator = Some(Terminator::Branch {
        target: BlockId(20),
        arguments: vec![],
    });
    body.blocks.extend([outer, inner_guard, inner]);
    let domain = domain(&analyze(&module));
    assert_eq!(domain.predicate(), ValueId(7));
    assert_eq!(
        domain.path(),
        FormalGuardedPathV1::TrueEdge {
            source: BlockId(10),
            ordinal: 0,
            target: BlockId(11)
        }
    );
}

#[test]
fn explicit_guarded_load_and_additive_pointer_chains_are_not_new_domain_shortcuts() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    module.functions[0].body.as_mut().unwrap().blocks[1].operations[1].kind =
        OperationKind::GuardedLoad {
            pointer: ValueId(9),
            predicate: ValueId(7),
            fallback: ValueId(5),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        };
    assert!(
        analyze(&module)
            .obligations()
            .accesses()
            .iter()
            .all(|row| !matches!(
                row.domain(),
                FormalAccessDomainV1::RuntimeSliceReadBounded(_)
            ))
    );
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let read = &mut module.functions[0].body.as_mut().unwrap().blocks[1];
    let pointer = read.operations[0].results[0].ty.clone();
    read.operations.insert(
        1,
        op(40, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
    );
    read.operations.insert(
        2,
        op(
            41,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(9),
                offset: ValueId(40),
            },
        ),
    );
    let OperationKind::Load { pointer, .. } = &mut read.operations[3].kind else {
        unreachable!()
    };
    *pointer = ValueId(41);
    unsupported(&module);
}

#[test]
fn sparse_block_ids_and_physical_block_order_do_not_change_exact_guard_identity() {
    let mut module = fixture(ScalarType::F32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].id = BlockId(u32::MAX - 2);
    body.blocks[1].id = BlockId(u32::MAX - 1);
    body.blocks[2].id = BlockId(u32::MAX);
    let Some(Terminator::ConditionalBranch {
        then_target,
        else_target,
        ..
    }) = &mut body.blocks[0].terminator
    else {
        unreachable!()
    };
    *then_target = BlockId(u32::MAX - 1);
    *else_target = BlockId(u32::MAX);
    body.blocks.swap(1, 2);
    assert_eq!(
        domain(&analyze(&module)).path(),
        FormalGuardedPathV1::TrueEdge {
            source: BlockId(u32::MAX - 2),
            ordinal: 0,
            target: BlockId(u32::MAX - 1)
        }
    );
}

#[test]
fn thirty_two_bit_analysis_cannot_create_the_sixty_four_bit_runtime_domain() {
    let module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let verified = verify_module_ref(&module).unwrap();
    let analysis = crate::derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits32,
    )
    .unwrap();
    assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
        reason,
        FormalMemoryIncompleteReason::UnsupportedIndexWidth {
            width: FormalIndexWidth::Bits32
        }
    )));
    assert!(analysis.obligations().accesses().is_empty());
}

#[test]
fn new_domain_work_storage_and_no_candidate_paths_are_bounded() {
    let module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let _verified = verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let (definitions, control) = collect_definitions(function).unwrap();
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    let query = |guarded: &mut GuardedAnalysisV1<'_>| {
        guarded.runtime_slice_read(
            FunctionOperationLocation::new(BlockId(20), 1),
            ValueId(9),
            FormalMemoryAccessKind::Read,
            MemoryAccess::new(AddressSpace::Global, 4),
            InvocationRange1d::new(0, 64).unwrap(),
            None,
        )
    };
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let bytes = guarded.ledger.bytes;
    assert!(query(&mut guarded).unwrap().is_some());
    let exact = guarded.ledger.work.work();
    assert!(exact > 24);
    for limit in [exact - 1, exact] {
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let result = query(&mut guarded);
        if limit == exact {
            assert!(result.unwrap().is_some());
        } else {
            assert!(matches!(result, Err(ResourceError::Work(_))));
        }
        assert_eq!(guarded.ledger.bytes, bytes);
        assert_eq!(guarded.ledger.work.failed_work().is_some(), limit != exact);
    }
    guarded.runtime_reads = RuntimeReadState::default();
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    guarded.ledger.bytes = MAX_NEW_BYTES;
    assert!(matches!(
        guarded.collect_runtime_reads(&definitions, function),
        Err(ResourceError::Storage { .. })
    ));
    assert_eq!(guarded.runtime_reads.guards.capacity(), 0);

    let mut no_candidate = fixture(ScalarType::U32, AccessMode::ReadOnly);
    no_candidate.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .pop();
    let function = &no_candidate.functions[0];
    verify_module_ref(&no_candidate).unwrap();
    assert!(collect_definitions(function).unwrap().1.is_none());
}

#[test]
fn origin_lookup_charge_covers_pinned_btree_node_search_comparisons() {
    use std::{cell::Cell, cmp::Ordering, collections::BTreeMap};

    thread_local! {
        static COMPARISONS: Cell<usize> = const { Cell::new(0) };
    }
    struct Key(usize);
    impl PartialEq for Key {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for Key {}
    impl PartialOrd for Key {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for Key {
        fn cmp(&self, other: &Self) -> Ordering {
            COMPARISONS.set(COMPARISONS.get() + 1);
            self.0.cmp(&other.0)
        }
    }

    for count in [0usize, 1, 11, 12, 100, 1_000] {
        for reverse in [false, true] {
            let mut map = BTreeMap::new();
            for ordinal in 0..count {
                let key = if reverse {
                    count - ordinal - 1
                } else {
                    ordinal
                };
                map.insert(Key(key), ());
            }
            let lookup_charge = origin_lookup_work_v1(count).unwrap();
            for key in 0..=count + 1 {
                COMPARISONS.set(0);
                let _ = map.get(&Key(key));
                assert!(COMPARISONS.get() <= lookup_charge - 4);
                if count == 11 && key == 10 {
                    // One full pinned root node already exceeds log2(n) + 4.
                    assert_eq!(COMPARISONS.get(), 11);
                    assert!(
                        COMPARISONS.get()
                            > crate::verification_index_v1::verification_ceil_log2_v1(count) + 4
                    );
                }
            }
        }
    }
    assert_eq!(origin_lookup_work_v1(0).unwrap(), 16);
    assert_eq!(origin_lookup_work_v1(11).unwrap(), 64);
    assert!(origin_lookup_work_v1(usize::MAX).is_ok());
}

#[path = "runtime_slice_read_switch_v1_tests.rs"]
mod switch_tests;

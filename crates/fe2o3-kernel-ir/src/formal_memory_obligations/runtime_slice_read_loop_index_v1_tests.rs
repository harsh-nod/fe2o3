use super::*;
use crate::{FormalMemoryReceiptEncodingV4, InertFormalMemoryReceiptFormatV4};

fn changing_loop(scalar: ScalarType, mode: AccessMode, increment: bool) -> Module {
    let mut module = fixture(scalar, mode);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.remove(1);
    body.blocks[0].operations.push(op(
        21,
        Type::INDEX,
        OperationKind::Constant(Constant::Index(1)),
    ));
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments: vec![ValueId(1)],
    });
    let mut header = BasicBlock::new(BlockId(15));
    header
        .parameters
        .push(ValueDef::new(ValueId(40), Type::INDEX));
    header.operations.push(op(
        7,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(40),
            rhs: ValueId(6),
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let read = &mut body.blocks[1];
    let OperationKind::GetElementPointer { offset, .. } = &mut read.operations[0].kind else {
        unreachable!()
    };
    *offset = ValueId(40);
    read.operations.push(Operation::checked_binary(
        ValueDef::new(ValueId(41), Type::INDEX),
        ValueDef::new(ValueId(42), Type::BOOL),
        crate::CheckedBinaryOperator::Add,
        ValueId(40),
        ValueId(21),
    ));
    read.terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments: vec![if increment { ValueId(41) } else { ValueId(2) }],
    });
    body.blocks.push(header);
    module
}

fn refused(module: &Module, index: ValueId) {
    let analysis = analyze(module);
    assert!(!analysis.is_complete(), "{analysis:?}");
    assert!(analysis.obligations().accesses().is_empty(), "{analysis:?}");
    assert!(
        analysis.incomplete_reasons().iter().any(|reason| matches!(
            reason,
            FormalMemoryIncompleteReason::UnsupportedIndexExpression { index: actual, .. }
                if *actual == index
        )),
        "{analysis:?}"
    );
}

#[test]
fn a_fresh_guard_accepts_only_the_current_changing_phi_read() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        for mode in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            for increment in [false, true] {
                let module = changing_loop(scalar, mode, increment);
                let before = module.clone();
                let analysis = analyze(&module);
                let row = domain(&analysis);
                assert_eq!(row.allocation().parameter_index(), 0);
                assert_eq!(
                    (row.slice(), row.index(), row.guard_index()),
                    (ValueId(0), ValueId(40), ValueId(40))
                );
                assert_eq!(
                    (row.length(), row.predicate(), row.pointer()),
                    (ValueId(6), ValueId(7), ValueId(9))
                );
                assert_eq!(row.element_bytes(), scalar_byte_width(scalar).unwrap());
                assert_eq!(
                    row.path(),
                    FormalGuardedPathV1::TrueEdge {
                        source: BlockId(15),
                        ordinal: 0,
                        target: BlockId(20),
                    }
                );
                let access = &analysis.obligations().accesses()[0];
                assert_eq!(
                    access.location(),
                    FunctionOperationLocation::new(BlockId(20), 1)
                );
                assert_eq!(access.kind(), FormalMemoryAccessKind::Read);
                assert_eq!(access.byte_offset(), ByteExpression::Unbounded);
                let bounds = &analysis.obligations().bounds_requirements()[0];
                assert_eq!(
                    bounds.kind(),
                    FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(row)
                );
                assert_eq!(bounds.minimum_byte_len(), None);
                assert!(!bounds.is_met_by_untrusted_byte_len(u64::MAX));
                assert!(
                    analysis
                        .obligations()
                        .inter_invocation_conflicts()
                        .is_empty()
                );
                assert_eq!(analyze(&module), analysis);
                assert_eq!(module, before);
            }
        }
    }
}

#[test]
fn an_exact_phi_read_witness_does_not_solve_its_origin_or_affine_recurrence() {
    let module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let (definitions, control) = collect_definitions(function).unwrap();
    assert_eq!(
        definitions.block_parameter_origins.get(&ValueId(40)),
        Some(&None)
    );
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    assert_eq!(guarded.runtime_index_origin(ValueId(40)).unwrap(), None);
    assert!(matches!(
        guarded.runtime_read_index(ValueId(40)).unwrap(),
        Some(ReadIndex::ExactBlockParameter(ValueId(40)))
    ));
    assert_eq!(guarded.runtime_index_origin(ValueId(40)).unwrap(), None);
    assert_eq!(
        guarded.runtime_index_origin(ValueId(1)).unwrap(),
        Some(ValueId(1))
    );
    assert!(matches!(
        guarded.runtime_read_index(ValueId(1)).unwrap(),
        Some(ReadIndex::ProvenOrigin(ValueId(1)))
    ));
    assert!(guarded.runtime_read_index(ValueId(42)).unwrap().is_none());
}

#[test]
fn changing_phi_runtime_rows_roundtrip_inertly_without_mutating_the_graph() {
    let module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    let before = module.clone();
    let analysis = analyze(&module);
    domain(&analysis);
    let receipt =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(analysis.obligations()).unwrap();
    assert_eq!(
        receipt.metadata().encoding(),
        FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
    );
    assert!(!receipt.grants_authority());
    let bytes = receipt.into_canonical_bytes();
    let decoded = InertFormalMemoryReceiptFormatV4::decode_current(bytes.clone()).unwrap();
    decoded.revalidate().unwrap();
    assert_eq!(decoded.canonical_bytes(), bytes);
    assert!(!decoded.grants_authority());
    assert_eq!(module, before);
}

#[test]
fn a_stale_external_guard_still_does_not_cover_a_changing_phi() {
    refused(&loop_carrier(true), ValueId(40));
    let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    let OperationKind::Compare { lhs, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[3].operations[0].kind
    else {
        unreachable!()
    };
    *lhs = ValueId(1);
    refused(&module, ValueId(40));
}

#[test]
fn swapping_phis_or_using_the_post_guard_update_cannot_reuse_a_read_witness() {
    for update in [false, true] {
        let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
        let body = module.functions[0].body.as_mut().unwrap();
        let index = if update {
            let next = body.blocks[1].operations.pop().unwrap();
            body.blocks[1].operations.insert(0, next);
            let OperationKind::GetElementPointer { offset, .. } =
                &mut body.blocks[1].operations[1].kind
            else {
                unreachable!()
            };
            *offset = ValueId(41);
            ValueId(41)
        } else {
            body.blocks[3]
                .parameters
                .push(ValueDef::new(ValueId(43), Type::INDEX));
            let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[0].terminator else {
                unreachable!()
            };
            arguments.push(ValueId(2));
            let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[1].terminator else {
                unreachable!()
            };
            arguments.push(ValueId(40));
            let OperationKind::GetElementPointer { offset, .. } =
                &mut body.blocks[1].operations[0].kind
            else {
                unreachable!()
            };
            *offset = ValueId(43);
            ValueId(43)
        };
        refused(&module, index);
    }
}

#[test]
fn a_changing_index_still_requires_the_exact_slice_and_strict_comparison() {
    for case in 0..3 {
        let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
        let body = module.functions[0].body.as_mut().unwrap();
        match case {
            0 => {
                body.blocks[0].operations[0].kind = OperationKind::SliceLength { slice: ValueId(3) }
            }
            1 => body.blocks[0].operations[1].kind = OperationKind::SliceData { slice: ValueId(3) },
            _ => {
                body.blocks[3].operations[0].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(40),
                    rhs: ValueId(6),
                }
            }
        }
        refused(&module, ValueId(40));
    }
}

#[test]
fn duplicate_false_or_nondominating_edges_cannot_supply_a_phi_read_guard() {
    for case in 0..3 {
        let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
        let body = module.functions[0].body.as_mut().unwrap();
        match case {
            0 => {
                let Some(Terminator::ConditionalBranch { else_target, .. }) =
                    &mut body.blocks[3].terminator
                else {
                    unreachable!()
                };
                *else_target = BlockId(20);
            }
            1 => {
                let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut body.blocks[3].terminator
                else {
                    unreachable!()
                };
                std::mem::swap(then_target, else_target);
            }
            _ => {
                let mut landing = BasicBlock::new(BlockId(16));
                landing.terminator = Some(Terminator::Branch {
                    target: BlockId(20),
                    arguments: vec![],
                });
                let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut body.blocks[3].terminator
                else {
                    unreachable!()
                };
                *then_target = BlockId(16);
                *else_target = BlockId(20);
                body.blocks.push(landing);
            }
        }
        refused(&module, ValueId(40));
    }
}

#[test]
fn a_read_before_its_loop_guard_is_not_covered_by_the_later_true_edge() {
    let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    let body = module.functions[0].body.as_mut().unwrap();
    let load = body.blocks[1].operations.remove(1);
    let gep = body.blocks[1].operations.remove(0);
    body.blocks[3].operations.insert(0, gep);
    body.blocks[3].operations.insert(1, load);
    refused(&module, ValueId(40));
}

#[test]
fn an_exact_phi_read_witness_never_exempts_a_write_through_that_pointer() {
    let mut module = changing_loop(ScalarType::U32, AccessMode::ReadWrite, true);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(write(ValueId(9)));
    let analysis = analyze(&module);
    assert!(!analysis.is_complete());
    assert_eq!(analysis.obligations().accesses().len(), 1);
    assert_eq!(
        analysis.obligations().accesses()[0].kind(),
        FormalMemoryAccessKind::Read
    );
    assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
        reason,
        FormalMemoryIncompleteReason::UnsupportedIndexExpression {
            index: ValueId(40),
            ..
        }
    )));
}

#[test]
fn changing_phi_reads_keep_whole_allocation_alias_and_race_obligations() {
    let mut module = changing_loop(ScalarType::U32, AccessMode::ReadWrite, true);
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
                    && conflict.right() == FunctionOperationLocation::new(BlockId(20), 3)
            )
    );

    let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
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
        panic!("one allocation pair")
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
fn exact_phi_query_work_and_initial_storage_refusals_use_the_real_guard_ledger() {
    let module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    verify_module_ref(&module).unwrap();
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
    guarded.ledger.charge(7).unwrap();
    let bytes = guarded.ledger.bytes;
    let records = guarded.ledger.records;
    assert!(query(&mut guarded).unwrap().is_some());
    let exact = guarded.ledger.work.work();
    for limit in [exact - 1, exact] {
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
        guarded.ledger.charge(7).unwrap();
        let result = query(&mut guarded);
        if limit == exact {
            assert!(result.unwrap().is_some());
            assert_eq!(guarded.ledger.work.work(), exact);
            assert_eq!(guarded.ledger.work.failed_work(), None);
        } else {
            let Err(ResourceError::Work(error)) = result else {
                panic!("exact query work refusal")
            };
            assert_eq!(error.limit(), limit);
            assert_eq!(error.actual(), exact);
            assert_eq!(guarded.ledger.work.failed_work(), Some(exact));
            assert_eq!(guarded.ledger.work.work(), exact - 24);
        }
        assert_eq!(guarded.ledger.bytes, bytes);
        assert_eq!(guarded.ledger.records, records);
    }
    guarded.runtime_reads = RuntimeReadState::default();
    guarded.ledger.bytes = MAX_NEW_BYTES;
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    guarded.ledger.charge(7).unwrap();
    let expected =
        MAX_NEW_BYTES + definitions.block_parameter_origins.len() * size_of::<Origin<'_>>();
    assert!(
        matches!(guarded.collect_runtime_reads(&definitions, function),
        Err(ResourceError::Storage { actual, limit }) if actual == expected && limit == MAX_NEW_BYTES)
    );
    assert_eq!(guarded.ledger.work.work(), 9);
    assert_eq!(guarded.ledger.bytes, MAX_NEW_BYTES);
    assert_eq!(guarded.ledger.records, records);
    assert_eq!(guarded.runtime_reads.origins.capacity(), 0);
    assert_eq!(guarded.runtime_reads.guards.capacity(), 0);
}

#[test]
fn old_scalar_and_invariant_reads_and_the_no_read_path_keep_their_contracts() {
    let module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let before = module.clone();
    assert_eq!(domain(&analyze(&module)).index(), ValueId(1));
    assert_eq!(module, before);
    assert_eq!(domain(&analyze(&loop_carrier(false))).index(), ValueId(40));
    let mut module = changing_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .remove(1);
    verify_module_ref(&module).unwrap();
    assert!(
        collect_definitions(&module.functions[0])
            .unwrap()
            .1
            .is_none()
    );
    let analysis = analyze(&module);
    assert!(analysis.is_complete(), "{analysis:?}");
    assert!(analysis.obligations().accesses().is_empty());
}

fn bridged_loop(scalar: ScalarType, mode: AccessMode, increment: bool) -> Module {
    let mut module = changing_loop(scalar, mode, increment);
    let function = &mut module.functions[0];
    function.signature.parameters[1] = Type::Scalar(ScalarType::U64);
    function.signature.parameters[2] = Type::Scalar(ScalarType::U64);
    let body = function.body.as_mut().unwrap();
    body.blocks[0].operations[2] = op(
        21,
        Type::Scalar(ScalarType::U64),
        OperationKind::Constant(Constant::U64(1)),
    );
    body.blocks[3].parameters[0].ty = Type::Scalar(ScalarType::U64);
    body.blocks[3].operations[0].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: ValueId(60),
        rhs: ValueId(6),
    };
    body.blocks[3].operations.insert(
        0,
        op(
            60,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(40),
                to: Type::INDEX,
            },
        ),
    );
    body.blocks[1].operations[0].kind = OperationKind::GetElementPointer {
        base: ValueId(8),
        offset: ValueId(61),
    };
    body.blocks[1].operations[2].results[0].ty = Type::Scalar(ScalarType::U64);
    body.blocks[1].operations.insert(
        0,
        op(
            61,
            Type::INDEX,
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(40),
                to: Type::INDEX,
            },
        ),
    );
    module
}

#[test]
fn duplicate_bridges_of_the_current_u64_phi_retain_exact_raw_read_sites() {
    for scalar in [
        ScalarType::U8,
        ScalarType::U16,
        ScalarType::U32,
        ScalarType::U64,
        ScalarType::I8,
        ScalarType::I16,
        ScalarType::I32,
        ScalarType::I64,
        ScalarType::F16,
        ScalarType::Bf16,
        ScalarType::F32,
        ScalarType::F64,
    ] {
        for mode in [AccessMode::ReadOnly, AccessMode::ReadWrite] {
            for increment in [false, true] {
                let module = bridged_loop(scalar, mode, increment);
                let before = module.clone();
                let analysis = analyze(&module);
                let row = domain(&analysis);
                assert_eq!(
                    (row.index(), row.guard_index(), row.length()),
                    (ValueId(61), ValueId(60), ValueId(6))
                );
                assert_eq!(
                    (row.slice(), row.pointer(), row.predicate()),
                    (ValueId(0), ValueId(9), ValueId(7))
                );
                assert_eq!(
                    row.path(),
                    FormalGuardedPathV1::TrueEdge {
                        source: BlockId(15),
                        ordinal: 0,
                        target: BlockId(20)
                    }
                );
                assert_eq!(row.allocation().parameter_index(), 0);
                assert_eq!(row.element_bytes(), scalar_byte_width(scalar).unwrap());
                let access = &analysis.obligations().accesses()[0];
                assert_eq!(
                    access.location(),
                    FunctionOperationLocation::new(BlockId(20), 2)
                );
                assert_eq!(access.kind(), FormalMemoryAccessKind::Read);
                assert_eq!(access.byte_offset(), ByteExpression::Unbounded);
                let bounds = &analysis.obligations().bounds_requirements()[0];
                assert_eq!(
                    bounds.kind(),
                    FormalBoundsKindV1::RuntimeSliceElementAtGuardedIndex(row)
                );
                assert_eq!(bounds.minimum_byte_len(), None);
                assert!(!bounds.is_met_by_untrusted_byte_len(u64::MAX));
                assert!(
                    analysis
                        .obligations()
                        .inter_invocation_conflicts()
                        .is_empty()
                );
                assert_eq!(analyze(&module), analysis);
                assert_eq!(module, before);
            }
        }
    }
}

#[test]
fn bridged_phi_reads_do_not_change_unique_origins_or_admit_raw_u64_guards() {
    let module = bridged_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    let function = &module.functions[0];
    verify_module_ref(&module).unwrap();
    let (definitions, control) = collect_definitions(function).unwrap();
    assert_eq!(
        definitions.block_parameter_origins.get(&ValueId(40)),
        Some(&None)
    );
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    for value in [ValueId(60), ValueId(61)] {
        assert!(matches!(
            guarded.runtime_read_index(value).unwrap(),
            Some(ReadIndex::ExactBlockParameter(ValueId(40)))
        ));
        assert_eq!(guarded.runtime_index_origin(value).unwrap(), Some(value));
    }
    assert!(guarded.runtime_read_index(ValueId(40)).unwrap().is_none());
    assert_eq!(guarded.runtime_origin(ValueId(40)).unwrap(), None);
}

#[test]
fn stale_distinct_and_updated_u64_values_cannot_share_a_bridged_phi_guard() {
    for case in 0..3 {
        let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadOnly, true);
        let body = module.functions[0].body.as_mut().unwrap();
        match case {
            0 => {
                body.blocks[3].operations[0].kind = OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(1),
                    to: Type::INDEX,
                }
            }
            1 => {
                body.blocks[3]
                    .parameters
                    .push(ValueDef::new(ValueId(43), Type::Scalar(ScalarType::U64)));
                let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[0].terminator
                else {
                    unreachable!();
                };
                arguments.push(ValueId(2));
                let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[1].terminator
                else {
                    unreachable!();
                };
                arguments.push(ValueId(40));
                body.blocks[1].operations[0].kind = OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(43),
                    to: Type::INDEX,
                };
            }
            _ => {
                let next = body.blocks[1].operations.pop().unwrap();
                body.blocks[1].operations.insert(0, next);
                body.blocks[1].operations[1].kind = OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(41),
                    to: Type::INDEX,
                };
            }
        }
        refused(&module, ValueId(61));
    }
}

#[test]
fn bridged_reads_preserve_slice_strictness_and_dynamic_true_edge_refusals() {
    for case in 0..7 {
        let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadOnly, true);
        let body = module.functions[0].body.as_mut().unwrap();
        match case {
            0 => {
                body.blocks[0].operations[0].kind = OperationKind::SliceLength { slice: ValueId(3) }
            }
            1 => body.blocks[0].operations[1].kind = OperationKind::SliceData { slice: ValueId(3) },
            2 => {
                body.blocks[3].operations[1].kind = OperationKind::Compare {
                    predicate: ComparePredicate::LessThanOrEqual,
                    lhs: ValueId(60),
                    rhs: ValueId(6),
                }
            }
            3 | 4 | 5 => {
                let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut body.blocks[3].terminator
                else {
                    unreachable!();
                };
                match case {
                    3 => *else_target = BlockId(20),
                    4 => std::mem::swap(then_target, else_target),
                    _ => {
                        *then_target = BlockId(16);
                        *else_target = BlockId(20);
                    }
                }
                if case == 5 {
                    let mut landing = BasicBlock::new(BlockId(16));
                    landing.terminator = Some(Terminator::Branch {
                        target: BlockId(20),
                        arguments: vec![],
                    });
                    body.blocks.push(landing);
                }
            }
            _ => {
                let read = body.blocks[1].operations.drain(..3).collect::<Vec<_>>();
                body.blocks[3].operations.splice(0..0, read);
            }
        }
        refused(&module, ValueId(61));
    }
    let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    let OperationKind::Load { access, .. } =
        &mut module.functions[0].body.as_mut().unwrap().blocks[1].operations[2].kind
    else {
        unreachable!();
    };
    access.volatile = true;
    refused(&module, ValueId(61));
}

#[test]
fn bridged_reads_never_supply_write_bounds_alias_or_race_exemptions() {
    let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadWrite, true);
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(write(ValueId(9)));
    let analysis = analyze(&module);
    assert!(!analysis.is_complete());
    assert_eq!(analysis.obligations().accesses().len(), 1);
    assert_eq!(
        analysis.obligations().accesses()[0].kind(),
        FormalMemoryAccessKind::Read
    );
    assert!(analysis.incomplete_reasons().iter().any(|r| matches!(
        r,
        FormalMemoryIncompleteReason::UnsupportedIndexExpression {
            index: ValueId(61),
            ..
        }
    )));
    let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadWrite, true);
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
                |c| c.left() == FunctionOperationLocation::new(BlockId(20), 2)
                    && c.right() == FunctionOperationLocation::new(BlockId(20), 4)
            )
    );
    let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadOnly, true);
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
        panic!("one allocation pair");
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
fn bridged_read_query_work_is_exact_and_the_no_read_path_still_allocates_nothing() {
    let mut module = bridged_loop(ScalarType::U32, AccessMode::ReadOnly, true);
    {
        let function = &module.functions[0];
        let (definitions, control) = collect_definitions(function).unwrap();
        let mut guarded =
            GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
        let query = |g: &mut GuardedAnalysisV1<'_>| {
            g.runtime_slice_read(
                FunctionOperationLocation::new(BlockId(20), 2),
                ValueId(9),
                FormalMemoryAccessKind::Read,
                MemoryAccess::new(AddressSpace::Global, 4),
                InvocationRange1d::new(0, 64).unwrap(),
                None,
            )
        };
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        guarded.ledger.charge(7).unwrap();
        let (bytes, records) = (guarded.ledger.bytes, guarded.ledger.records);
        assert!(query(&mut guarded).unwrap().is_some());
        let exact = guarded.ledger.work.work();
        for limit in [exact - 1, exact] {
            guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
            guarded.ledger.charge(7).unwrap();
            let result = query(&mut guarded);
            if limit == exact {
                assert!(result.unwrap().is_some());
                assert_eq!(guarded.ledger.work.work(), exact);
                assert_eq!(guarded.ledger.work.failed_work(), None);
            } else {
                assert!(
                    matches!(result, Err(ResourceError::Work(e)) if e.actual() == exact && e.limit() == limit)
                );
                assert_eq!(guarded.ledger.work.work(), exact - 24);
                assert_eq!(guarded.ledger.work.failed_work(), Some(exact));
            }
            assert_eq!(
                (guarded.ledger.bytes, guarded.ledger.records),
                (bytes, records)
            );
        }
    }
    module.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .remove(2);
    verify_module_ref(&module).unwrap();
    assert!(
        collect_definitions(&module.functions[0])
            .unwrap()
            .1
            .is_none()
    );
    let analysis = analyze(&module);
    assert!(analysis.is_complete(), "{analysis:?}");
    assert!(analysis.obligations().accesses().is_empty());
}

fn nested_shared_predicate(mode: AccessMode) -> Module {
    let mut module = bridged_loop(ScalarType::U32, mode, true);
    let body = module.functions[0].body.as_mut().unwrap();
    let Some(Terminator::ConditionalBranch { then_target, .. }) = &mut body.blocks[3].terminator
    else {
        unreachable!();
    };
    *then_target = BlockId(16);
    let mut repeated = BasicBlock::new(BlockId(16));
    repeated.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    body.blocks.push(repeated);
    module
}

fn disjoint_shared_predicate() -> Module {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(11),
        then_arguments: vec![],
        else_target: BlockId(12),
        else_arguments: vec![],
    });
    let mut second_read = body.blocks[1].clone();
    second_read.id = BlockId(21);
    second_read.operations[0].results[0].id = ValueId(19);
    second_read.operations[1].results[0].id = ValueId(20);
    let OperationKind::Load { pointer, .. } = &mut second_read.operations[1].kind else {
        unreachable!();
    };
    *pointer = ValueId(19);
    for (source, target) in [(11, 20), (12, 21)] {
        let mut guard = BasicBlock::new(BlockId(source));
        guard.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(7),
            then_target: BlockId(target),
            then_arguments: vec![],
            else_target: BlockId(30),
            else_arguments: vec![],
        });
        body.blocks.push(guard);
    }
    body.blocks.push(second_read);
    module
}

#[test]
fn repeated_true_predicate_keeps_a_fresh_changing_phi_witness_after_cse() {
    let mut module = nested_shared_predicate(AccessMode::ReadOnly);
    for merged in [false, true] {
        if !merged {
            let repeated = &mut module.functions[0].body.as_mut().unwrap().blocks[4];
            repeated.operations.push(op(
                70,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    lhs: ValueId(60),
                    rhs: ValueId(6),
                },
            ));
            let Some(Terminator::ConditionalBranch { condition, .. }) = &mut repeated.terminator
            else {
                unreachable!();
            };
            *condition = ValueId(70);
        } else {
            let repeated = &mut module.functions[0].body.as_mut().unwrap().blocks[4];
            repeated.operations.clear();
            let Some(Terminator::ConditionalBranch { condition, .. }) = &mut repeated.terminator
            else {
                unreachable!();
            };
            *condition = ValueId(7);
        }
        let before = module.clone();
        let analysis = analyze(&module);
        let row = domain(&analysis);
        assert_eq!(
            (row.index(), row.guard_index(), row.predicate()),
            (ValueId(61), ValueId(60), ValueId(7))
        );
        assert_eq!(
            row.path(),
            FormalGuardedPathV1::TrueEdge {
                source: BlockId(15),
                ordinal: 0,
                target: BlockId(16),
            }
        );
        assert_eq!(
            analysis.obligations().accesses()[0].byte_offset(),
            ByteExpression::Unbounded
        );
        let function = &module.functions[0];
        let (definitions, control) = collect_definitions(function).unwrap();
        assert_eq!(
            definitions.block_parameter_origins.get(&ValueId(40)),
            Some(&None)
        );
        let mut guarded =
            GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
        assert_eq!(guarded.truths.len(), 2);
        assert!(guarded.truths.iter().all(|row| row.ambiguous == merged));
        assert_eq!(guarded.runtime_reads.guards.len(), 2);
        assert_eq!(guarded.runtime_index_origin(ValueId(40)).unwrap(), None);
        assert_eq!(module, before);
        assert_eq!(analyze(&module), analysis);
    }
}

#[test]
fn repeated_predicate_disjoint_true_edges_cover_only_their_own_reads() {
    let module = disjoint_shared_predicate();
    let analysis = analyze(&module);
    assert!(analysis.is_complete(), "{analysis:?}");
    assert_eq!(analysis.obligations().accesses().len(), 2);
    for access in analysis.obligations().accesses() {
        let FormalAccessDomainV1::RuntimeSliceReadBounded(row) = access.domain() else {
            panic!("actual runtime read");
        };
        let (source, target) = if access.location().block == BlockId(20) {
            (11, 20)
        } else {
            (12, 21)
        };
        assert_eq!(row.predicate(), ValueId(7));
        assert_eq!(
            row.path(),
            FormalGuardedPathV1::TrueEdge {
                source: BlockId(source),
                ordinal: 0,
                target: BlockId(target),
            }
        );
        assert_eq!(access.byte_offset(), ByteExpression::Unbounded);
    }
    let mut joined = module.clone();
    let body = joined.functions[0].body.as_mut().unwrap();
    body.blocks[1].operations.clear();
    body.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(21),
        arguments: vec![],
    });
    unsupported(&joined);
}

#[test]
fn repeated_predicate_false_bypass_and_duplicate_incoming_edges_do_not_cover_reads() {
    for mutation in 0..3 {
        let mut module = disjoint_shared_predicate();
        let body = module.functions[0].body.as_mut().unwrap();
        if mutation == 2 {
            let Some(Terminator::ConditionalBranch { then_target, .. }) =
                &mut body.blocks[0].terminator
            else {
                unreachable!();
            };
            *then_target = BlockId(13);
            let mut bypass = BasicBlock::new(BlockId(13));
            bypass.terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(7),
                then_target: BlockId(11),
                then_arguments: vec![],
                else_target: BlockId(20),
                else_arguments: vec![],
            });
            body.blocks.push(bypass);
        } else {
            for block in &mut body.blocks[3..5] {
                let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut block.terminator
                else {
                    unreachable!();
                };
                if mutation == 0 {
                    std::mem::swap(then_target, else_target);
                } else {
                    *else_target = *then_target;
                }
            }
        }
        unsupported(&module);
    }
}

#[test]
fn repeated_predicate_never_proves_a_distinct_phi_index_or_write() {
    let mut wrong = nested_shared_predicate(AccessMode::ReadOnly);
    let OperationKind::Cast { value, .. } =
        &mut wrong.functions[0].body.as_mut().unwrap().blocks[1].operations[0].kind
    else {
        unreachable!();
    };
    *value = ValueId(2);
    refused(&wrong, ValueId(61));
    let mut write_case = nested_shared_predicate(AccessMode::ReadWrite);
    write_case.functions[0].body.as_mut().unwrap().blocks[1]
        .operations
        .push(write(ValueId(9)));
    let analysis = analyze(&write_case);
    assert!(!analysis.is_complete());
    assert!(
        analysis
            .obligations()
            .accesses()
            .iter()
            .all(|row| row.kind() == FormalMemoryAccessKind::Read)
    );
    assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(reason,
        FormalMemoryIncompleteReason::UnsupportedIndexExpression { index, .. } if *index == ValueId(61))));
}

#[test]
fn repeated_predicate_rows_keep_prepaid_capacity_and_exact_query_work() {
    let module = nested_shared_predicate(AccessMode::ReadOnly);
    verify_module_ref(&module).unwrap();
    let function = &module.functions[0];
    let (definitions, control) = collect_definitions(function).unwrap();
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    assert_eq!(guarded.truths.len(), 2);
    assert_eq!(guarded.runtime_reads.guards.len(), 2);
    assert!(guarded.runtime_reads.guards.capacity() >= guarded.truths.len());
    assert!(
        guarded.ledger.bytes >= guarded.runtime_reads.guards.capacity() * size_of::<ReadGuard>()
    );
    let query = |g: &mut GuardedAnalysisV1<'_>| {
        g.runtime_slice_read(
            FunctionOperationLocation::new(BlockId(20), 2),
            ValueId(9),
            FormalMemoryAccessKind::Read,
            MemoryAccess::new(AddressSpace::Global, 4),
            InvocationRange1d::new(0, 64).unwrap(),
            None,
        )
    };
    let (bytes, records) = (guarded.ledger.bytes, guarded.ledger.records);
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    guarded.ledger.charge(7).unwrap();
    assert!(query(&mut guarded).unwrap().is_some());
    let exact = guarded.ledger.work.work();
    for limit in [exact - 1, exact] {
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
        guarded.ledger.charge(7).unwrap();
        let result = query(&mut guarded);
        if limit == exact {
            assert!(result.unwrap().is_some());
        } else {
            assert!(
                matches!(result, Err(ResourceError::Work(e)) if e.actual() == exact && e.limit() == limit)
            );
            assert_eq!(guarded.ledger.work.work(), exact - 24);
            assert_eq!(guarded.ledger.work.failed_work(), Some(exact));
        }
        assert_eq!(
            (guarded.ledger.bytes, guarded.ledger.records),
            (bytes, records)
        );
    }
    guarded.runtime_reads = RuntimeReadState::default();
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    guarded.ledger.bytes = MAX_NEW_BYTES;
    assert_eq!(definitions.block_parameter_origins.len(), 1);
    assert!(
        matches!(guarded.collect_runtime_reads(&definitions, function),
        Err(ResourceError::Storage { actual, limit })
            if actual == MAX_NEW_BYTES + size_of::<Origin<'_>>() && limit == MAX_NEW_BYTES)
    );
    assert_eq!(guarded.ledger.bytes, MAX_NEW_BYTES);
    assert_eq!(guarded.runtime_reads.guards.capacity(), 0);
}

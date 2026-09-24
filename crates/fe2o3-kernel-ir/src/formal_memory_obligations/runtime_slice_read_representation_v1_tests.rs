use super::*;
use crate::{FormalMemoryReceiptEncodingV4, InertFormalMemoryReceiptFormatV4};

fn cast_fixture(guard: (ValueId, ValueId), read_index: ValueId) -> Module {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let function = &mut module.functions[0];
    function.signature.parameters[1] = Type::Scalar(ScalarType::U64);
    let body = function.body.as_mut().unwrap();
    let entry = &mut body.blocks[0];
    entry.operations.splice(
        1..1,
        [
            op(
                40,
                Type::INDEX,
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(1),
                    to: Type::INDEX,
                },
            ),
            op(
                41,
                Type::Scalar(ScalarType::U64),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(6),
                    to: Type::Scalar(ScalarType::U64),
                },
            ),
            op(
                42,
                Type::INDEX,
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(1),
                    to: Type::INDEX,
                },
            ),
        ],
    );
    entry.operations[4].kind = OperationKind::Compare {
        predicate: ComparePredicate::LessThan,
        lhs: guard.0,
        rhs: guard.1,
    };
    body.blocks[1].operations[0].kind = OperationKind::GetElementPointer {
        base: ValueId(8),
        offset: read_index,
    };
    module
}

fn assert_unsupported_cast_index(module: &Module, index: ValueId) {
    let analysis = analyze(module);
    assert!(!analysis.is_complete(), "{analysis:?}");
    assert!(analysis.obligations().accesses().is_empty());
    assert!(analysis.obligations().bounds_requirements().is_empty());
    assert!(
        analysis.incomplete_reasons().iter().any(|reason| matches!(
            reason,
            FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                location,
                index: actual,
                allocation,
            } if *location == FunctionOperationLocation::new(BlockId(20), 0)
                && *actual == index
                && allocation.parameter_index() == 0
        )),
        "{analysis:?}"
    );
}

fn assert_exact_cast_index(module: &Module, index: ValueId) {
    let analysis = analyze(module);
    let actual = domain(&analysis);
    assert_eq!((actual.index(), actual.guard_index()), (index, index));
    assert_eq!(actual.length(), ValueId(6));
    assert_eq!(actual.slice(), ValueId(0));
    assert_eq!(actual.pointer(), ValueId(9));
    let receipt =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(analysis.obligations()).unwrap();
    assert_eq!(
        receipt.metadata().encoding(),
        FormalMemoryReceiptEncodingV4::RuntimeBoundedV4
    );
    assert!(!receipt.grants_authority());
    let decoded =
        InertFormalMemoryReceiptFormatV4::decode_current(receipt.canonical_bytes().to_vec())
            .unwrap();
    assert_eq!(decoded, receipt);
}

#[test]
fn raw_u64_comparison_does_not_replace_the_exact_post_cast_index_guard() {
    // Both graphs verify. Equal 64-bit representations do not add a new guard
    // recipe: the accepted graph compares the actual INDEX used by the GEP.
    assert_unsupported_cast_index(
        &cast_fixture((ValueId(1), ValueId(41)), ValueId(40)),
        ValueId(40),
    );
    assert_exact_cast_index(
        &cast_fixture((ValueId(40), ValueId(6)), ValueId(40)),
        ValueId(40),
    );
}

#[test]
fn separate_exact_representation_casts_share_only_the_same_scalar() {
    let module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
    assert_bridge(&module, 42, 40, 6);
    let mut changed = module.clone();
    changed.functions[0].signature.parameters[2] = Type::Scalar(ScalarType::U64);
    let OperationKind::Cast { value, .. } =
        &mut changed.functions[0].body.as_mut().unwrap().blocks[0].operations[3].kind
    else {
        unreachable!();
    };
    *value = ValueId(2);
    assert_unsupported_cast_index(&changed, ValueId(42));
    assert_exact_cast_index(
        &cast_fixture((ValueId(42), ValueId(6)), ValueId(42)),
        ValueId(42),
    );
}

fn assert_bridge(module: &Module, index: u32, guard: u32, length: u32) {
    let before = module.clone();
    let analysis = analyze(module);
    let row = domain(&analysis);
    assert_eq!(
        (row.index(), row.guard_index(), row.length()),
        (ValueId(index), ValueId(guard), ValueId(length))
    );
    assert_eq!((row.slice(), row.pointer()), (ValueId(0), ValueId(9)));
    assert_eq!(
        analysis.obligations().accesses()[0].byte_offset(),
        ByteExpression::Unbounded
    );
    assert_eq!(
        analysis.obligations().bounds_requirements()[0].minimum_byte_len(),
        None
    );
    let receipt =
        InertFormalMemoryReceiptFormatV4::from_current_obligations(analysis.obligations()).unwrap();
    assert!(!receipt.grants_authority());
    assert_eq!(
        InertFormalMemoryReceiptFormatV4::decode_current(receipt.canonical_bytes().to_vec())
            .unwrap(),
        receipt
    );
    assert_eq!(*module, before);
}

#[test]
fn inverse_length_bridge_preserves_the_actual_slice_and_raw_witness_ids() {
    let mut module = cast_fixture((ValueId(40), ValueId(43)), ValueId(42));
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            4,
            op(
                43,
                Type::INDEX,
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(41),
                    to: Type::INDEX,
                },
            ),
        );
    assert_bridge(&module, 42, 40, 43);
    let mut wrong = module.clone();
    wrong.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::SliceLength { slice: ValueId(3) };
    assert_unsupported_cast_index(&wrong, ValueId(42));
    let verified = verify_module_ref(&module).unwrap();
    let analysis = crate::derive_kernel_memory_obligations_from_verified(
        verified,
        &KernelId::new("kernel"),
        ExplicitLaunchExtent1d::Exact(64),
        FormalIndexWidth::Bits32,
    )
    .unwrap();
    assert!(!analysis.is_complete());
    assert!(analysis.obligations().accesses().is_empty());
    assert!(analysis.incomplete_reasons().iter().any(|r| matches!(
        r,
        FormalMemoryIncompleteReason::UnsupportedIndexWidth {
            width: FormalIndexWidth::Bits32
        }
    )));
}

#[test]
fn exact_same_type_origin_transport_preserves_inverse_length_and_index_bridges() {
    let mut module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
    let body = module.functions[0].body.as_mut().unwrap();
    let cast = body.blocks[0].operations.remove(3);
    let Some(Terminator::ConditionalBranch { then_arguments, .. }) = &mut body.blocks[0].terminator
    else {
        unreachable!();
    };
    then_arguments.push(ValueId(1));
    body.blocks[1]
        .parameters
        .push(ValueDef::new(ValueId(50), Type::Scalar(ScalarType::U64)));
    body.blocks[1].operations.insert(0, cast);
    let OperationKind::Cast { value, .. } = &mut body.blocks[1].operations[0].kind else {
        unreachable!();
    };
    *value = ValueId(50);
    assert_bridge(&module, 42, 40, 6);
    let (definitions, _) = collect_definitions(&module.functions[0]).unwrap();
    assert_eq!(
        definitions.block_parameter_origins.get(&ValueId(50)),
        Some(&Some(ValueId(1)))
    );

    let mut module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
    let body = module.functions[0].body.as_mut().unwrap();
    let compare = body.blocks[0].operations.remove(4);
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(15),
        arguments: vec![ValueId(41)],
    });
    let mut guard = BasicBlock::new(BlockId(15));
    guard
        .parameters
        .push(ValueDef::new(ValueId(50), Type::Scalar(ScalarType::U64)));
    guard.operations.push(op(
        51,
        Type::INDEX,
        OperationKind::Cast {
            kind: CastKind::Bitcast,
            value: ValueId(50),
            to: Type::INDEX,
        },
    ));
    guard.operations.push(compare);
    let OperationKind::Compare { rhs, .. } = &mut guard.operations[1].kind else {
        unreachable!();
    };
    *rhs = ValueId(51);
    guard.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(7),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    body.blocks.push(guard);
    assert_bridge(&module, 42, 40, 51);
    module.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::ConditionalBranch {
            condition: ValueId(4),
            then_target: BlockId(15),
            then_arguments: vec![ValueId(41)],
            else_target: BlockId(15),
            else_arguments: vec![ValueId(1)],
        });
    assert_unsupported_cast_index(&module, ValueId(42));
}

#[test]
fn numeric_and_signed_casts_are_not_representation_equivalence() {
    for (source, middle, kind) in [
        (ScalarType::U32, ScalarType::U64, CastKind::ZeroExtend),
        (ScalarType::I64, ScalarType::U64, CastKind::Bitcast),
        (ScalarType::I32, ScalarType::U64, CastKind::SignExtend),
        (ScalarType::U128, ScalarType::U64, CastKind::Truncate),
    ] {
        let mut module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
        let function = &mut module.functions[0];
        function.signature.parameters[1] = Type::Scalar(source);
        let operations = &mut function.body.as_mut().unwrap().blocks[0].operations;
        for (at, id) in [(3, 52), (1, 51)] {
            let OperationKind::Cast { value, .. } = &mut operations[at].kind else {
                unreachable!();
            };
            *value = ValueId(id);
        }
        operations.splice(
            1..1,
            [51, 52].map(|id| {
                op(
                    id,
                    Type::Scalar(middle),
                    OperationKind::Cast {
                        kind,
                        value: ValueId(1),
                        to: Type::Scalar(middle),
                    },
                )
            }),
        );
        assert_unsupported_cast_index(&module, ValueId(42));
    }
}

#[test]
fn invalid_direct_wrong_width_bridges_fail_ir_admission_not_formal_analysis() {
    for scalar in [
        ScalarType::I64,
        ScalarType::U32,
        ScalarType::U128,
        ScalarType::F64,
    ] {
        let mut module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
        module.functions[0].signature.parameters[1] = Type::Scalar(scalar);
        assert!(verify_module_ref(&module).is_err(), "{scalar:?}");
    }
}

#[test]
fn read_representation_cache_has_bounded_deep_paths_and_rejects_cycles() {
    let module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
    let function = &module.functions[0];
    verify_module_ref(&module).unwrap();
    let (definitions, control) = collect_definitions(function).unwrap();
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    for cyclic in [false, true] {
        let count = 4096;
        guarded.runtime_reads.representations = (0..count)
            .map(|i| ReadRepresentation {
                value: ValueId(i as u32),
                scalar: ScalarType::Index,
                link: ReadLink::Terminal,
                next: if i + 1 < count {
                    Some(i + 1)
                } else if cyclic {
                    Some(0)
                } else {
                    None
                },
                resolution: if i + 1 == count && !cyclic {
                    ReadResolution::Resolved(ReadIndex::ProvenOrigin(ValueId(9000)))
                } else {
                    ReadResolution::Pending
                },
            })
            .collect();
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(100_000);
        guarded.resolve_read_representations().unwrap();
        assert!(guarded.ledger.work.work() < 10 * count);
        for row in &guarded.runtime_reads.representations {
            assert!(if cyclic {
                matches!(row.resolution, ReadResolution::Unsupported)
            } else {
                matches!(
                    row.resolution,
                    ReadResolution::Resolved(ReadIndex::ProvenOrigin(ValueId(9000)))
                )
            });
        }
    }
}

#[test]
fn read_representation_cache_prepays_headers_capacity_and_typed_work() {
    let module = cast_fixture((ValueId(40), ValueId(6)), ValueId(42));
    let function = &module.functions[0];
    verify_module_ref(&module).unwrap();
    let (definitions, control) = collect_definitions(function).unwrap();
    let mut guarded =
        GuardedAnalysisV1::new(control.unwrap(), &definitions, function, true).unwrap();
    guarded.runtime_reads.representations.clear();
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(0);
    let before = guarded.ledger.bytes;
    let result = guarded.collect_read_representations(function);
    assert!(matches!(result, Err(ResourceError::Work(e)) if e.actual() == 2 && e.limit() == 0));
    assert_eq!(guarded.ledger.bytes, before);
    guarded.runtime_reads.representations = Vec::new();
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    guarded.ledger.bytes = MAX_NEW_BYTES;
    let rows =
        guarded.parameters.len() + guarded.runtime_reads.origins.len() + guarded.definitions.len();
    let result = guarded.collect_read_representations(function);
    assert!(
        matches!(result, Err(ResourceError::Storage { actual, limit })
        if actual == MAX_NEW_BYTES + rows * size_of::<ReadRepresentation>() && limit == MAX_NEW_BYTES)
    );
    assert_eq!(guarded.runtime_reads.representations.capacity(), 0);
    assert_eq!(guarded.ledger.work.work(), 2);
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let result = guarded.resolve_read_representations();
    assert!(
        matches!(result, Err(ResourceError::Storage { actual, limit })
        if actual == MAX_NEW_BYTES + size_of::<Vec<usize>>() && limit == MAX_NEW_BYTES)
    );
    assert_eq!(guarded.ledger.work.work(), 0);
    guarded.ledger.bytes = 0;
    guarded.collect_read_representations(function).unwrap();
    let count = guarded.runtime_reads.representations.len();
    let capacity = guarded.runtime_reads.representations.capacity();
    assert!(capacity >= rows && count <= rows);
    assert!(
        guarded.ledger.bytes
            >= capacity * size_of::<ReadRepresentation>()
                + size_of::<Vec<usize>>()
                + count * size_of::<usize>()
    );
    assert_eq!(
        guarded.runtime_reads.representations.last().unwrap().value,
        ValueId(42)
    );
    let exact_work = guarded.ledger.work.work();
    let exact_bytes = guarded.ledger.bytes;
    for limit in [exact_work - 1, exact_work] {
        guarded.runtime_reads.representations = Vec::new();
        guarded.ledger.bytes = 0;
        guarded.ledger.records = 0;
        guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let result = guarded.collect_read_representations(function);
        if limit == exact_work {
            result.unwrap();
            assert_eq!(guarded.ledger.work.work(), exact_work);
            assert_eq!(guarded.ledger.work.failed_work(), None);
        } else {
            assert!(
                matches!(result, Err(ResourceError::Work(e)) if e.actual() == exact_work && e.limit() == limit)
            );
            // The last row is bridge 42; its final resolved-cache write pays 2.
            assert_eq!(guarded.ledger.work.work(), exact_work - 2);
            assert_eq!(guarded.ledger.work.failed_work(), Some(exact_work));
        }
        assert_eq!(guarded.ledger.bytes, exact_bytes);
    }
    guarded.ledger.bytes = 0;
    guarded.ledger.records = 0;
    guarded.ledger.work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut actual_backing = Vec::<usize>::new();
    guarded.ledger.reserve(&mut actual_backing, count).unwrap();
    assert_eq!(
        guarded.ledger.bytes,
        actual_backing.capacity() * size_of::<usize>()
    );
    assert_eq!(guarded.ledger.records, actual_backing.capacity());
}

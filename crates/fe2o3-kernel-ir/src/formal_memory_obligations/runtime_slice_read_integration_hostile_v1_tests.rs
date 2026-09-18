use super::*;
use crate::{FormalMemoryReceiptEncodingV4, InertFormalMemoryReceiptFormatV4};

fn runtime_roundtrip(analysis: &FormalMemoryObligationAnalysis) {
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
}

#[test]
fn two_opaque_read_indices_on_one_slice_keep_the_pre_guard_site_incomplete_after_codec_roundtrip() {
    let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
    let body = module.functions[0].body.as_mut().unwrap();
    let pointer = body.blocks[1].operations[0].results[0].ty.clone();
    // SliceData %8 and length %6 are real definitions. The later true edge
    // proves %1 < %6, not this earlier read through distinct opaque index %2.
    body.blocks[0].operations.extend([
        op(
            20,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(8),
                offset: ValueId(2),
            },
        ),
        op(
            21,
            Type::Scalar(ScalarType::U32),
            OperationKind::Load {
                pointer: ValueId(20),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    assert_eq!(
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter(|operation| matches!(operation.kind, OperationKind::Load { .. }))
            .count(),
        2
    );
    let analysis = analyze(&module);
    assert!(!analysis.is_complete());
    // Existing pointer derivation locates this failure at the GEP, not its load.
    assert!(
        analysis.incomplete_reasons().iter().any(|reason| matches!(
            reason,
            FormalMemoryIncompleteReason::UnsupportedIndexExpression { location, index, allocation }
                if *location == FunctionOperationLocation::new(BlockId(10), 3)
                    && *index == ValueId(2)
                    && allocation.parameter_index() == 0
        )),
        "{:?}",
        analysis.incomplete_reasons()
    );
    let [valid] = analysis.obligations().accesses() else {
        panic!("only the independently guarded actual read is retained")
    };
    assert_eq!(
        valid.location(),
        FunctionOperationLocation::new(BlockId(20), 1)
    );
    let FormalAccessDomainV1::RuntimeSliceReadBounded(domain) = valid.domain() else {
        panic!("actual runtime read domain")
    };
    assert_eq!(
        (domain.slice(), domain.index(), domain.guard_index()),
        (ValueId(0), ValueId(1), ValueId(1))
    );
    assert_eq!(analysis.obligations().bounds_requirements().len(), 1);

    // V4 encodes retained rows, not the separate extraction completeness result.
    // Successful inert roundtrip must not be mistaken for accounting for both reads.
    runtime_roundtrip(&analysis);
    assert!(!analysis.is_complete());
    assert_eq!(analysis.obligations().accesses().len(), 1);
}

#[test]
fn mutually_recursive_loop_carriers_require_one_origin_across_both_seeds() {
    for conflicting_seed in [false, true] {
        let mut module = loop_carrier(false);
        let body = module.functions[0].body.as_mut().unwrap();
        body.blocks[1]
            .parameters
            .push(ValueDef::new(ValueId(41), Type::INDEX));
        let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[3].terminator else {
            unreachable!()
        };
        arguments.push(if conflicting_seed {
            ValueId(2)
        } else {
            ValueId(1)
        });
        let Some(Terminator::ConditionalBranch { then_arguments, .. }) =
            &mut body.blocks[1].terminator
        else {
            unreachable!()
        };
        // %40 <- %41 and %41 <- %40 form a two-node SCC. It is valid only
        // when the two real entry seeds identify the same guarded SSA value.
        *then_arguments = vec![ValueId(41), ValueId(40)];
        let analysis = analyze(&module);
        if conflicting_seed {
            assert!(!analysis.is_complete());
            assert!(analysis.obligations().accesses().is_empty());
            assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
                reason,
                FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                    index: ValueId(40),
                    ..
                }
            )));
        } else {
            let domain = domain(&analysis);
            assert_eq!(domain.index(), ValueId(40));
            assert_eq!(domain.guard_index(), ValueId(1));
            assert_eq!(domain.slice(), ValueId(0));
            runtime_roundtrip(&analysis);
        }
    }
}

#[test]
fn wrapped_u64_index_requires_its_exact_slice_bound_not_an_old_or_nonzero_guard() {
    for guard in 0..3 {
        let mut module = fixture(ScalarType::U32, AccessMode::ReadOnly);
        let body = module.functions[0].body.as_mut().unwrap();
        let mut operations = vec![
            op(
                40,
                Type::Scalar(ScalarType::U64),
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(1),
                    to: Type::Scalar(ScalarType::U64),
                },
            ),
            op(
                41,
                Type::Scalar(ScalarType::U64),
                OperationKind::Constant(Constant::U64(u64::MAX)),
            ),
            op(
                42,
                Type::Scalar(ScalarType::U64),
                OperationKind::Binary {
                    op: crate::BinaryOp::Add,
                    lhs: ValueId(40),
                    rhs: ValueId(41),
                },
            ),
            op(
                43,
                Type::INDEX,
                OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value: ValueId(42),
                    to: Type::INDEX,
                },
            ),
            op(44, Type::INDEX, OperationKind::Constant(Constant::Index(0))),
        ];
        operations.append(&mut body.blocks[0].operations);
        body.blocks[0].operations = operations;
        body.blocks[0].operations[6].kind = match guard {
            0 => OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(43),
                rhs: ValueId(6),
            },
            1 => OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(1),
                rhs: ValueId(6),
            },
            _ => OperationKind::Compare {
                predicate: ComparePredicate::NotEqual,
                lhs: ValueId(43),
                rhs: ValueId(44),
            },
        };
        let OperationKind::GetElementPointer { offset, .. } =
            &mut body.blocks[1].operations[0].kind
        else {
            unreachable!()
        };
        *offset = ValueId(43);
        let analysis = analyze(&module);
        if guard == 0 {
            let domain = domain(&analysis);
            assert_eq!(
                (domain.index(), domain.guard_index()),
                (ValueId(43), ValueId(43))
            );
            runtime_roundtrip(&analysis);
        } else {
            // For original index zero and positive length, the old bound is
            // true while the wrapped result is MAX. MAX != 0 proves no bound.
            assert!(!analysis.is_complete());
            assert!(analysis.obligations().accesses().is_empty());
            assert!(analysis.incomplete_reasons().iter().any(|reason| matches!(
                reason,
                FormalMemoryIncompleteReason::UnsupportedIndexExpression {
                    index: ValueId(43),
                    ..
                }
            )));
        }
    }
}

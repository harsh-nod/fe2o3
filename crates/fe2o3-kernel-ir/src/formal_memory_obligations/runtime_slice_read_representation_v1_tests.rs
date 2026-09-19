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
fn separate_equal_input_cast_results_do_not_share_a_runtime_guard() {
    // Casts 40 and 42 read the same U64 input but remain distinct SSA results.
    // Only block-argument origin transport, not cast-expression equivalence,
    // is part of the current closed runtime-read rule.
    assert_unsupported_cast_index(
        &cast_fixture((ValueId(40), ValueId(6)), ValueId(42)),
        ValueId(42),
    );
    assert_exact_cast_index(
        &cast_fixture((ValueId(42), ValueId(6)), ValueId(42)),
        ValueId(42),
    );
}

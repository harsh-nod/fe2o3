use super::*;
use crate::{IndexConstantOp, RankedViewType};
use pliron::dialect::DialectName;

fn scalar(kind: SemanticScalarKindAttr, bits: u16) -> SemanticTypedScalarV1 {
    SemanticTypedScalarV1::new(kind, bits).unwrap()
}

struct Fixture {
    context: Context,
    view: Value,
    index: Value,
    guard: Value,
    fallback: Value,
}

fn fixture() -> Fixture {
    let mut context = Context::new();
    crate::register_dialect(
        &mut context,
        &DialectName::try_new(crate::DIALECT_NAME).unwrap(),
    )
    .unwrap();
    let ty = RankedViewType::new(&mut context, 32, false, vec![8]).unwrap();
    let view = RankedViewOp::new_in_space_with_allocation_contract(
        &mut context,
        ty,
        vec![],
        MemorySpaceAttr::Global,
        1,
        0,
    )
    .unwrap()
    .result(&context);
    let index = IndexConstantOp::new(&mut context, 2).result(&context);
    let guard =
        SemanticTypedConstantOp::new(&mut context, 1, scalar(SemanticScalarKindAttr::Bool, 1))
            .result(&context);
    let fallback =
        SemanticTypedConstantOp::new(&mut context, 0, scalar(SemanticScalarKindAttr::Float, 32))
            .result(&context);
    Fixture {
        context,
        view,
        index,
        guard,
        fallback,
    }
}

fn read(f: &mut Fixture, guarded: bool) -> SemanticTypedReadOp {
    SemanticTypedReadOp::new(
        &mut f.context,
        SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
        scalar(SemanticScalarKindAttr::Float, 32),
        MemorySpaceAttr::Global,
        SemanticReadVolatilityAttr::NonVolatile,
        SemanticReadOrderingAttr::Unordered,
        f.view,
        vec![f.index],
        guarded.then_some((f.guard, f.fallback)),
    )
    .unwrap()
}

#[test]
fn read_has_real_operands_and_one_exact_result() {
    let mut f = fixture();
    let op = read(&mut f, true);
    op.verify(&f.context).unwrap();
    assert_eq!(op.view(&f.context), f.view);
    assert_eq!(op.indices(&f.context), Some(vec![f.index]));
    assert_eq!(op.guarded(&f.context), Some((f.guard, f.fallback)));
    assert_eq!(
        op.result(&f.context).defining_op(),
        Some(op.get_operation())
    );
    assert_eq!(op.get_operation().deref(&f.context).get_num_operands(), 4);
    assert_eq!(op.get_operation().deref(&f.context).get_num_results(), 1);
    assert_eq!(
        op.scalar(&f.context),
        Some(scalar(SemanticScalarKindAttr::Float, 32))
    );
}

#[test]
fn unguarded_read_has_no_invented_fallback() {
    let mut f = fixture();
    let op = read(&mut f, false);
    op.verify(&f.context).unwrap();
    assert_eq!(op.guarded(&f.context), None);
    assert_eq!(op.get_operation().deref(&f.context).get_num_operands(), 2);
}

#[test]
fn volatility_is_preserved_in_the_graph_not_erased() {
    let mut f = fixture();
    let op = read(&mut f, true);
    op.set_attr_kernel_semantic_read_volatility(
        &mut f.context,
        SemanticReadVolatilityAttr::Volatile,
    );
    op.verify(&f.context).unwrap();
    assert_eq!(
        op.volatility(&f.context),
        Some(SemanticReadVolatilityAttr::Volatile)
    );
}

#[test]
fn ordered_read_has_no_unsupported_constructor_path() {
    for ordering in [
        SemanticReadOrderingAttr::Acquire,
        SemanticReadOrderingAttr::SequentiallyConsistent,
    ] {
        let mut f = fixture();
        assert_eq!(
            SemanticTypedReadOp::new(
                &mut f.context,
                SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
                scalar(SemanticScalarKindAttr::Float, 32),
                MemorySpaceAttr::Global,
                SemanticReadVolatilityAttr::NonVolatile,
                ordering,
                f.view,
                vec![f.index],
                None,
            )
            .err(),
            Some(SemanticTypedReadErrorV1::UnsupportedOrdering)
        );
        let op = read(&mut f, false);
        op.set_attr_kernel_semantic_read_ordering(&mut f.context, ordering);
        assert!(op.verify(&f.context).is_err());
    }
}

#[test]
fn verifier_rejects_scalar_width_space_and_symbol_substitution() {
    for case in 0..3 {
        let mut f = fixture();
        let op = read(&mut f, true);
        match case {
            0 => op.set_attr_kernel_semantic_read_bit_width(&mut f.context, DimensionAttr(64)),
            1 => op.set_attr_kernel_semantic_read_space(&mut f.context, MemorySpaceAttr::Workgroup),
            _ => op.set_attr_kernel_semantic_read_id(&mut f.context, SemanticSymbolAttr(7)),
        }
        assert!(op.verify(&f.context).is_err());
    }
}

#[test]
fn guard_must_be_bool_and_fallback_must_match_kind_not_just_width() {
    for wrong_guard in [true, false] {
        let mut f = fixture();
        let int = SemanticTypedConstantOp::new(
            &mut f.context,
            0,
            scalar(SemanticScalarKindAttr::SignedInteger, 32),
        )
        .result(&f.context);
        let guarded = if wrong_guard {
            (f.fallback, f.fallback)
        } else {
            (f.guard, int)
        };
        assert_eq!(
            SemanticTypedReadOp::new(
                &mut f.context,
                SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
                scalar(SemanticScalarKindAttr::Float, 32),
                MemorySpaceAttr::Global,
                SemanticReadVolatilityAttr::NonVolatile,
                SemanticReadOrderingAttr::Unordered,
                f.view,
                vec![f.index],
                Some(guarded),
            )
            .err(),
            Some(SemanticTypedReadErrorV1::InvalidGuardOrFallback)
        );
    }
}

#[test]
fn rank_and_index_type_are_checked() {
    for wrong_type in [false, true] {
        let mut f = fixture();
        let indices = if wrong_type { vec![f.fallback] } else { vec![] };
        assert_eq!(
            SemanticTypedReadOp::new(
                &mut f.context,
                SEMANTIC_TYPED_READ_SYMBOL_BASE_V1,
                scalar(SemanticScalarKindAttr::Float, 32),
                MemorySpaceAttr::Global,
                SemanticReadVolatilityAttr::NonVolatile,
                SemanticReadOrderingAttr::Unordered,
                f.view,
                indices,
                None,
            )
            .err(),
            Some(SemanticTypedReadErrorV1::InvalidIndex)
        );
    }
}

#[test]
fn unknown_and_missing_attributes_are_rejected() {
    for missing in [false, true] {
        let mut f = fixture();
        let op = read(&mut f, true);
        if missing {
            op.get_operation()
                .deref_mut(&f.context)
                .attributes
                .0
                .remove(
                    &pliron::identifier::Identifier::try_from("kernel_semantic_read_volatility")
                        .unwrap(),
                );
        } else {
            op.get_operation()
                .deref_mut(&f.context)
                .attributes
                .set("invented_proof".try_into().unwrap(), SemanticSymbolAttr(1));
        }
        assert!(op.verify(&f.context).is_err());
    }
}

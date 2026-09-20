use super::*;
use dialect_kernel::{DIALECT_NAME, register_dialect};
use pliron::dialect::DialectName;

fn context() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    context
}

fn equivalent(context: &Context, left: Value, right: Value) -> bool {
    let mut meter = EquivalenceResourceMeterV1::new(1, 32).unwrap();
    let result = evaluate_index_equivalence_v1(context, left, right, 32, &mut meter).unwrap();
    assert!(!meter.exhausted());
    result
}

#[test]
fn singleton_dependencies_are_not_numeric_identity_in_either_direction() {
    let context = &mut context();
    let value = IndexConstantOp::new(context, 7).result(context);
    let join = DeterministicJoinOp::new(context, vec![value]).result(context);
    assert!(!equivalent(context, value, join));
    assert!(!equivalent(context, join, value));
    assert!(equivalent(context, join, join));
}

#[test]
fn distinct_joins_with_equal_dependencies_remain_numerically_opaque() {
    let context = &mut context();
    let left_value = IndexConstantOp::new(context, 7).result(context);
    let right_value = IndexConstantOp::new(context, 7).result(context);
    let left = DeterministicJoinOp::new(context, vec![left_value]).result(context);
    for dependency in [left_value, right_value] {
        let right = DeterministicJoinOp::new(context, vec![dependency]).result(context);
        assert!(!equivalent(context, left, right));
        assert!(!equivalent(context, right, left));
    }
    assert!(equivalent(context, left_value, right_value));
}

#[test]
fn nested_joins_do_not_become_offsets_of_their_dependencies() {
    let context = &mut context();
    let value = IndexConstantOp::new(context, 7).result(context);
    let one = IndexConstantOp::new(context, 1).result(context);
    let mut join = value;
    for _ in 0..8 {
        join = DeterministicJoinOp::new(context, vec![join]).result(context);
        let add = IndexBinaryOp::new(context, IndexBinaryKindAttr::Add, join, one).result(context);
        let mut meter = EquivalenceResourceMeterV1::new(8, 32).unwrap();
        assert_eq!(index_offset(context, join, value, &mut meter), None);
        assert_eq!(index_offset(context, add, value, &mut meter), None);
        assert_eq!(index_offset(context, add, join, &mut meter), Some(1));
        assert!(!meter.exhausted());
    }
}

#[test]
fn cast_and_binary_recursion_preserve_opaque_operand_identity() {
    let context = &mut context();
    let value = IndexConstantOp::new(context, 7).result(context);
    let one = IndexConstantOp::new(context, 1).result(context);
    let left = DeterministicJoinOp::new(context, vec![value]).result(context);
    let right = DeterministicJoinOp::new(context, vec![value]).result(context);
    let left_cast = IndexUnsignedCastOp::new(context, left, 32).result(context);
    let right_cast = IndexUnsignedCastOp::new(context, right, 32).result(context);
    let same_cast = IndexUnsignedCastOp::new(context, left, 32).result(context);
    assert!(!equivalent(context, left_cast, right_cast));
    assert!(equivalent(context, left_cast, same_cast));
    for kind in [
        IndexBinaryKindAttr::Add,
        IndexBinaryKindAttr::Multiply,
        IndexBinaryKindAttr::Divide,
        IndexBinaryKindAttr::Remainder,
    ] {
        let lhs = IndexBinaryOp::new(context, kind, left, one).result(context);
        let rhs = IndexBinaryOp::new(context, kind, right, one).result(context);
        let same = IndexBinaryOp::new(context, kind, left, one).result(context);
        assert!(!equivalent(context, lhs, rhs));
        assert!(equivalent(context, lhs, same));
        if matches!(
            kind,
            IndexBinaryKindAttr::Add | IndexBinaryKindAttr::Multiply
        ) {
            let reversed = IndexBinaryOp::new(context, kind, one, left).result(context);
            assert!(equivalent(context, lhs, reversed));
        }
    }
}

#[test]
fn uniformity_still_follows_explicit_dependencies_without_numeric_authority() {
    let context = &mut context();
    let constant = IndexConstantOp::new(context, 7).result(context);
    let join = DeterministicJoinOp::new(context, vec![constant]).result(context);
    assert_eq!(
        is_uniform_value(context, join, &HashSet::new(), 2),
        Ok(true)
    );
    assert_eq!(
        is_uniform_value(context, join, &HashSet::new(), 1),
        Err(UniformityVisitLimitV1)
    );
    assert!(!equivalent(context, join, constant));
}

#[test]
fn opaque_rejection_is_memoized_and_keeps_exact_resource_refusals() {
    let context = &mut context();
    let constant = IndexConstantOp::new(context, 7).result(context);
    let join = DeterministicJoinOp::new(context, vec![constant]).result(context);
    let mut meter = EquivalenceResourceMeterV1::new(2, 1).unwrap();
    assert_eq!(
        evaluate_index_equivalence_v1(context, join, constant, 1, &mut meter),
        Ok(false)
    );
    let expanded = meter.expanded_pairs;
    let steps = meter.cursor_steps;
    assert_eq!(
        evaluate_index_equivalence_v1(context, join, constant, 1, &mut meter),
        Ok(false)
    );
    assert_eq!(meter.expanded_pairs, expanded);
    assert_eq!(meter.cursor_steps, steps + 1);
    assert!(!meter.exhausted());
    let mut one_under = EquivalenceResourceMeterV1::new(1, 0).unwrap();
    assert_eq!(
        evaluate_index_equivalence_v1(context, join, constant, 1, &mut one_under),
        Err(EquivalenceVisitLimitV1)
    );
    assert!(one_under.exhausted());
}

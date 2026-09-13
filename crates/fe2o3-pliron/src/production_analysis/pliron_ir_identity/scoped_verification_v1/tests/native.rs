use super::*;
use dialect_gpu::{
    optimization_v1::{BinaryKindAttr, BinaryOp},
    switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3, SwitchOpV3},
};
use pliron::builtin::types::Signedness;

#[test]
fn checked_result_and_overflow_are_independent_dominance_uses() {
    let context = &mut setup();
    let integer = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let function = function(context, vec![integer, integer]);
    let entry = function.get_entry_block(context);
    let join = block(context, function, vec![integer, boolean]);
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let checked = BinaryOp::new(context, BinaryKindAttr::CheckedAdd, lhs, rhs);
    append(context, entry, checked);
    let branch = BranchArgsOp::new(
        context,
        vec![checked.result(context), checked.overflow(context).unwrap()],
        join,
    );
    append(context, entry, branch);
    let argument = join.deref(context).get_argument(0);
    let late = BinaryOp::new(context, BinaryKindAttr::CheckedAdd, argument, argument);
    append(context, join, late);
    ret(context, join);
    compare(context, &function, true);
    assert_eq!(TRACE.get().operand_visits, 6);
    for (ordinal, value, original) in [
        (0, late.result(context), checked.result(context)),
        (
            1,
            late.overflow(context).unwrap(),
            checked.overflow(context).unwrap(),
        ),
    ] {
        Operation::replace_operand(branch.get_operation(), context, ordinal, value);
        compare(context, &function, false);
        assert!(matches!(
            observe(context, &function),
            Err(Failure::Dominance { block: 0, operation: 1, operand }) if operand == ordinal
        ));
        Operation::replace_operand(branch.get_operation(), context, ordinal, original);
    }
}

#[test]
fn native_switch_checks_every_payload_occurrence_without_admitting_the_dialect() {
    for cases in [0, 1, 16, 17] {
        let context = &mut setup();
        let integer = IntegerType::get(context, 128, Signedness::Unsigned).into();
        let function = function(context, vec![integer]);
        let entry = function.get_entry_block(context);
        let join = block(context, function, vec![integer, integer]);
        let selector = entry.deref(context).get_argument(0);
        let edges = (0..=cases)
            .map(|_| SwitchEdgeV3::new(join, vec![selector, selector]))
            .collect();
        let switch = SwitchOpV3::try_new(
            context,
            selector,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..cases as u64).collect(),
            edges,
        )
        .unwrap();
        append(context, entry, switch);
        ret(context, join);
        compare(context, &function, true);
        assert_eq!(TRACE.get().tree_requests, 1);
        assert_eq!(TRACE.get().operand_visits, 1 + 2 * (cases + 1));
        assert!(matches!(
            prescan(context, &function),
            Err(PlironIrIdentityErrorV1::UnsupportedOperation { .. })
        ));

        // Check each case and the default independently. A block argument of
        // the destination is not available in the branching predecessor.
        for operand in 1..1 + 2 * (cases + 1) {
            let foreign_order = join.deref(context).get_argument(operand % 2);
            Operation::replace_operand(switch.get_operation(), context, operand, foreign_order);
            compare(context, &function, false);
            assert!(matches!(
                observe(context, &function),
                Err(Failure::Dominance { block: 0, operation: 0, operand: failed }) if failed == operand
            ));
            Operation::replace_operand(switch.get_operation(), context, operand, selector);
        }
    }
}

use super::*;
use dialect_kernel::{AtomicOrderingAttr, AtomicScopeAttr, SemanticSymbolOp};
use pliron::common_traits::Verify;

#[test]
fn stored_rhs_is_charged_at_the_existing_operation_item_boundary() {
    let context = &mut Context::new();
    dialect_kernel::register_dialect(
        context,
        &pliron::dialect::DialectName::try_new("kernel").unwrap(),
    )
    .unwrap();
    for rank in [1, dialect_kernel::MAX_RANKED_MEMORY_RANK] {
        let ty = dialect_kernel::RankedViewType::new(context, 32, true, vec![8; rank]).unwrap();
        let view = RankedViewOp::new(context, ty, vec![]).unwrap();
        let index = IndexConstantOp::new(context, 0).result(context);
        let value = SemanticSymbolOp::new(context, 1).result(context);
        let plain = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            view.result(context),
            vec![index; rank],
        )
        .unwrap();
        let atomic = RankedAccessOp::new_atomic(
            context,
            AccessKindAttr::AtomicWrite,
            AtomicOrderingAttr::Release,
            AtomicScopeAttr::Device,
            view.result(context),
            vec![index; rank],
        )
        .unwrap();
        for access in [plain, atomic] {
            let cost = || {
                let raw = access.get_operation().deref(context);
                raw.get_num_operands()
                    + raw.get_num_results()
                    + raw.get_num_successors()
                    + raw.attributes.0.len()
            };
            let legacy_cost = cost();
            Operation::insert_operand(access.get_operation(), context, rank + 1, value);
            let valued_cost = cost();
            assert_eq!(valued_cost, legacy_cost + 1);
            access.verify(context).unwrap();
            for (cost, succeeds) in [(legacy_cost, true), (valued_cost, false)] {
                let mut budget = RankedBoundsBudget::default();
                budget
                    .reserve(
                        RankedBoundsResource::OperationItems,
                        MAX_RANKED_BOUNDS_OPERATION_ITEMS - legacy_cost,
                    )
                    .unwrap();
                let result = budget.reserve(RankedBoundsResource::OperationItems, cost);
                if succeeds {
                    result.unwrap();
                } else {
                    assert!(
                        matches!(result, Err(RankedBoundsFindingV1::ResourceLimitExceeded {
                        resource: "operation component", limit: MAX_RANKED_BOUNDS_OPERATION_ITEMS, actual,
                    }) if actual == MAX_RANKED_BOUNDS_OPERATION_ITEMS + 1)
                    );
                }
            }
        }
    }
}

use super::*;
use dialect_gpu::{
    optimization_v1::{BinaryKindAttr, BinaryOp},
    switch_v3::{SwitchEdgeV3, SwitchKeyKindAttrV3, SwitchOpV3},
};
use pliron::builtin::types::Signedness;

// Structural closure alone is independent of the production operation
// allowlist. This census is not a positive identity or analysis admission.
fn native_census(context: &Context, function: &FuncOp) -> PrescanV1 {
    let blocks: Vec<_> = function
        .get_region(context)
        .deref(context)
        .iter(context)
        .collect();
    let operations: Vec<Vec<_>> = blocks
        .iter()
        .map(|block| block.deref(context).iter(context).collect())
        .collect();
    let mut scan = PrescanV1 {
        blocks,
        operations,
        values: 0,
        operands: 0,
        successors: 0,
        block_arguments: 0,
        attributes: 0,
        type_nodes: 0,
        max_operation_arity: 0,
        max_successor_arity: 0,
    };
    for block in &scan.blocks {
        scan.block_arguments += block.deref(context).get_num_arguments();
    }
    scan.values = scan.block_arguments;
    for operation in scan.operations.iter().flatten() {
        let raw = operation.deref(context);
        scan.values += raw.get_num_results();
        scan.operands += raw.get_num_operands();
        scan.successors += raw.get_num_successors();
    }
    scan
}

#[test]
fn checked_second_result_pays_both_defining_roster_scans() {
    let context = &mut setup();
    let integer = IntegerType::get(context, 32, Signedness::Unsigned).into();
    let local = function(context, vec![integer, integer]);
    let entry = local.get_entry_block(context);
    let lhs = entry.deref(context).get_argument(0);
    let rhs = entry.deref(context).get_argument(1);
    let binary = BinaryOp::new(context, BinaryKindAttr::CheckedAdd, lhs, rhs);
    append(context, entry, binary);
    let overflow = binary.overflow(context).unwrap();
    let boolean = IntegerType::get(context, 1, Signedness::Signless).into();
    let join = BasicBlock::new(context, None, vec![boolean; 5]);
    join.insert_at_back(local.get_region(context), context);
    let branch = BranchArgsOp::new(context, vec![overflow; 5], join);
    append(context, entry, branch);
    let ret = ReturnOp::new(context);
    append(context, join, ret);
    verify_operation(local.get_operation(), context).unwrap();
    let scan = native_census(context, &local);
    let mut budget = Budget::new(hard()).unwrap();
    let owners = Owners::new(context, &local, &scan, &mut budget).unwrap();
    let before = budget.work;
    let mut remaining = 5;
    owners
        .scalar_uses(context, overflow, 1, &mut remaining, &mut budget)
        .unwrap();
    // num_uses:8+2, uses:2+4*5, membership:5*408,
    // exact user slot searches:1+2+3+4+5.
    assert_eq!(budget.work - before, 2087);
    assert_eq!(remaining, 0);
    assert!(check(context, &local, &scan, hard()).is_ok());
}

#[test]
fn native_switch_preserves_every_repeated_successor_use() {
    for count in [0, 1, 3, 4, 5, 17] {
        let context = &mut setup();
        let integer = IntegerType::get(context, 128, Signedness::Unsigned).into();
        let local = function(context, vec![integer]);
        let entry = local.get_entry_block(context);
        let selector = entry.deref(context).get_argument(0);
        let join = BasicBlock::new(context, None, vec![]);
        join.insert_at_back(local.get_region(context), context);
        let edges = (0..=count)
            .map(|_| SwitchEdgeV3::new(join, vec![]))
            .collect();
        let switch = SwitchOpV3::try_new(
            context,
            selector,
            SwitchKeyKindAttrV3::LegacyU64,
            (0..count as u64).collect(),
            edges,
        )
        .unwrap();
        append(context, entry, switch);
        let ret = ReturnOp::new(context);
        append(context, join, ret);
        verify_operation(local.get_operation(), context).unwrap();
        let scan = native_census(context, &local);
        assert_eq!(scan.successors, count + 1);
        assert_eq!(join.num_preds(context), count + 1);
        reset_trace();
        let bound = check(context, &local, &scan, hard()).unwrap();
        assert_eq!(observed().user_rosters, count + 2);
        assert_eq!(observed().successor_use_vectors, 1);
        assert_eq!(observed().full_verifications, 0);
        assert_eq!(
            check(
                context,
                &local,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound()
                )
            )
            .unwrap(),
            bound
        );
        assert_resource(
            check(
                context,
                &local,
                &scan,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
            ),
            "work upper bound",
        );
        assert!(matches!(
            prescan(context, &local),
            Err(PlironIrIdentityErrorV1::UnsupportedOperation { .. })
        ));
    }
}

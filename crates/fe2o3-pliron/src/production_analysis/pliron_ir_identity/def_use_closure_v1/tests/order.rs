use super::*;

#[test]
fn local_rosters_must_match_physical_heads_order_and_tails() {
    for malformed in 0..4 {
        let context = &mut setup();
        let function = function(context, vec![]);
        let entry = function.get_entry_block(context);
        for value in 0..3 {
            let constant = IndexConstantOp::new(context, value);
            append(context, entry, constant);
        }
        let ret = ReturnOp::new(context);
        append(context, entry, ret);
        let mut scan = prescan(context, &function).unwrap();
        match malformed {
            0 => {
                scan.operations[0].remove(0);
            }
            1 => scan.operations[0].swap(1, 2),
            2 => {
                scan.operations[0].pop();
            }
            _ => scan.operations[0].clear(),
        }
        reset_trace();
        assert!(matches!(
            check(context, &function, &scan, hard()),
            Err(Failure::Invalid(
                "identity operation roster differs from physical order"
            ))
        ));
        assert_eq!(observed().order_indexes, 1);
        assert_eq!(observed().live_order_indexes, 0);
        assert_eq!(observed().retired_order_indexes, 1);
        assert_eq!(observed().full_verifications, 0);
    }
}

#[test]
fn matched_block_and_operation_permutation_is_not_physical_region_order() {
    let context = &mut setup();
    let function = fanout(context, 1);
    let mut scan = prescan(context, &function).unwrap();
    scan.blocks.swap(0, 1);
    scan.operations.swap(0, 1);
    reset_trace();
    assert!(matches!(
        check(context, &function, &scan, hard()),
        Err(Failure::Invalid(
            "identity block roster differs from physical order"
        ))
    ));
    assert_eq!(observed().live_order_indexes, 0);
    assert_eq!(observed().retired_order_indexes, 1);
}

#[test]
fn region_middle_order_and_tail_must_match_the_complete_roster() {
    for omit_tail in [false, true] {
        let context = &mut setup();
        let function = function(context, vec![]);
        for _ in 0..3 {
            let block = BasicBlock::new(context, None, vec![]);
            block.insert_at_back(function.get_region(context), context);
        }
        let blocks: Vec<_> = function
            .get_region(context)
            .deref(context)
            .iter(context)
            .collect();
        for block in blocks {
            let ret = ReturnOp::new(context);
            append(context, block, ret);
        }
        let mut scan = prescan(context, &function).unwrap();
        if omit_tail {
            scan.blocks.pop();
            scan.operations.pop();
        } else {
            scan.blocks.swap(1, 2);
            scan.operations.swap(1, 2);
        }
        reset_trace();
        assert!(matches!(
            check(context, &function, &scan, hard()),
            Err(Failure::Invalid(
                "identity block roster differs from physical order"
            ))
        ));
        assert_eq!(observed().live_order_indexes, 0);
        assert_eq!(observed().retired_order_indexes, 1);
    }
}

#[test]
fn admitted_index_counts_repeated_queries_and_cannot_exceed_prepayment() {
    let context = &mut setup();
    let function = fanout(context, 2);
    let scan = prescan(context, &function).unwrap();
    reset_trace();
    let mut order = super::super::check(context, &function, &scan, hard()).unwrap();
    assert_eq!(observed().live_order_indexes, 1);
    assert_eq!(order.remaining_queries, 2);
    assert!(order.strictly_precedes(scan.operations[0][0], 1));
    assert!(order.strictly_precedes(scan.operations[0][0], 1));
    assert!(!order.strictly_precedes(scan.operations[0][0], 1));
    drop(order);
    assert_eq!(observed().live_order_indexes, 0);
    assert_eq!(observed().retired_order_indexes, 1);
}

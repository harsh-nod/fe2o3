use super::*;

#[test]
fn distant_definition_fanout_uses_one_index_and_one_lookup_per_operand() {
    for count in [1, 16, 128] {
        let context = &mut setup();
        let function = function(context, vec![]);
        let entry = function.get_entry_block(context);
        let early = IndexConstantOp::new(context, 7);
        append(context, entry, early);
        for _ in 0..count {
            binary(context, entry, early.result(context), early.result(context));
        }
        ret(context, entry);
        let before = def_use_closure_v1::order_index_counts_for_tests();
        TRACE.set(Trace::default());
        assert!(captures(context, &function));
        assert_eq!(
            TRACE.get(),
            Trace {
                structural_verifications: 1,
                tree_requests: 1,
                operand_visits: 2 * count,
                cross_block_queries: 0,
                order_queries: 2 * count,
            }
        );
        let after = def_use_closure_v1::order_index_counts_for_tests();
        assert_eq!(after, (before.0 + 1, 0, before.2 + 1, before.3 + 1));
    }
}

#[test]
fn checked_order_rejects_another_root_or_context_before_verification() {
    let context = &mut setup();
    let first = function(context, vec![]);
    ret(context, first.get_entry_block(context));
    let second = function(context, vec![]);
    ret(context, second.get_entry_block(context));
    let other_context = &mut setup();
    let other = function(other_context, vec![]);
    ret(other_context, other.get_entry_block(other_context));
    for (target_context, target) in [
        (&*context, second),
        (&*other_context, other),
        (&*other_context, first),
    ] {
        let scan = def_use_closure_v1::native_census(context, &first);
        let order = def_use_closure_v1::check(
            context,
            &first,
            &scan,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        TRACE.set(Trace::default());
        assert!(matches!(
            verify(target_context, &target, order),
            Err(Failure::OwnerMismatch)
        ));
        assert_eq!(TRACE.get(), Trace::default());
        assert_eq!(def_use_closure_v1::order_index_counts_for_tests().1, 0);
    }
}

#[test]
fn actual_capture_retires_index_before_final_diagnostics_and_canonical_maps() {
    for outcome in 0..5 {
        let context = &mut setup();
        let function = function(context, vec![]);
        let entry = function.get_entry_block(context);
        let constant = IndexConstantOp::new(context, 1);
        append(context, entry, constant);
        let user = binary(
            context,
            entry,
            constant.result(context),
            constant.result(context),
        );
        match outcome {
            1 => Operation::replace_operand(user.get_operation(), context, 0, user.result(context)),
            2 => {} // Missing terminator: raw structural rejection.
            3 => PANIC_NEXT.set(true),
            4 => {
                let external = IndexConstantOp::new(context, 2);
                Operation::replace_operand(
                    user.get_operation(),
                    context,
                    0,
                    external.result(context),
                );
            }
            _ => {}
        }
        if outcome != 2 {
            ret(context, entry);
        }
        let before = def_use_closure_v1::order_index_counts_for_tests();
        assert_eq!(captures(context, &function), outcome == 0);
        let after = def_use_closure_v1::order_index_counts_for_tests();
        assert_eq!(
            after,
            (before.0 + 1, 0, before.2 + 1, before.3 + 1),
            "outcome {outcome}"
        );
        assert!(!PANIC_NEXT.get());
    }
}

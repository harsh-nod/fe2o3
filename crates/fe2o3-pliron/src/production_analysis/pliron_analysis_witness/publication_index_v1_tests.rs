#[cfg(test)]
mod publication_index_tests_v1 {
    use super::*;
    use crate::production_analysis::{
        pliron_function_inventory::BoundedPlironFunctionInventoryV1,
        pliron_ranked_bounds::run_pliron_ranked_bounds_check_v1,
        pliron_sparse_index::analyze_pliron_sparse_indices_v1,
    };
    use pliron::{dialect::DialectName, parsable::parse_from_str};

    fn fixture(
        predicated: bool,
        wrong_extent: bool,
    ) -> (Context, FuncOp, dialect_kernel::PublicationReadGuardOp) {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        let extent = if wrong_extent {
            "flags_len"
        } else {
            "payload_len"
        };
        let access = if predicated {
            "kernel.access (payload, checked, success) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,true,[0]>, kernel.index, kernel.checked_access_capability) -> ()>;"
        } else {
            "kernel.access (payload, checked) [] [kernel_access_kind: kernel.access_kind Read]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> ()>;"
        };
        let text = format!(
            r#"
builtin.func @publication_index: builtin.function <() -> ()>
{{
  ^entry():
    payload_len = kernel.index_constant () [] [kernel_index_value: kernel.index_value 128]: <() -> (kernel.index)>;
    flags_len = kernel.index_constant () [] [kernel_index_value: kernel.index_value 256]: <() -> (kernel.index)>;
    cell = kernel.index_constant () [] [kernel_index_value: kernel.index_value 200]: <() -> (kernel.index)>;
    payload = kernel.ranked_view (payload_len) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;
    flags = kernel.ranked_view (flags_len) [] [kernel_memory_space: kernel.memory_space Global]: <(kernel.index) -> (kernel.ranked_view <32,true,[0]>)>;
    acquired = kernel.access (flags, cell) [] [kernel_access_kind: kernel.access_kind AtomicRead, kernel_atomic_ordering: kernel.atomic_ordering Acquire, kernel_atomic_scope: kernel.atomic_scope System, kernel_publication_atomic: kernel.publication_atomic AcquireU32]: <(kernel.ranked_view <32,true,[0]>, kernel.index) -> (kernel.index)>;
    checked, success = kernel.publication_read_guard (cell, {extent}, acquired) [] []: <(kernel.index, kernel.index, kernel.index) -> (kernel.index, kernel.checked_access_capability)>;
    {access}
    kernel.return () [] []: <() -> ()>
}}
"#
        );
        let function = FuncOp::from_operation(
            parse_from_str(Operation::top_level_parser(), &mut context, &text).unwrap(),
        );
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let guard = inventory
            .operations()
            .iter()
            .find_map(|site| {
                Operation::get_op::<dialect_kernel::PublicationReadGuardOp>(
                    site.pointer(),
                    &context,
                )
            })
            .unwrap();
        (context, function, guard)
    }

    #[test]
    fn publication_guard_copies_index_but_never_acquire_or_success_facts() {
        let (context, function, guard) = fixture(true, false);
        function.verify(&context).unwrap();
        let sparse = analyze_pliron_sparse_indices_v1(&context, &function).unwrap();
        assert_eq!(
            sparse.fact(guard.result(&context)).constant_value(),
            Some(200)
        );
        assert_eq!(sparse.fact(guard.acquired(&context)).constant_value(), None);
        assert_eq!(sparse.fact(guard.success(&context)).constant_value(), None);
        assert_eq!(
            evaluate_raw_index_at_invocation_v1(&context, guard.result(&context), &[], &mut 0),
            Some(200)
        );
        assert_eq!(
            evaluate_raw_index_at_invocation_v1(&context, guard.acquired(&context), &[], &mut 0),
            None
        );
        assert_eq!(
            evaluate_raw_index_at_invocation_v1(&context, guard.success(&context), &[], &mut 0),
            None
        );
        assert!(run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
    }

    #[test]
    fn publication_bound_requires_exact_predicate_and_actual_extent() {
        let (context, function, _) = fixture(false, false);
        function.verify(&context).unwrap();
        assert!(!run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
        let (context, function, _) = fixture(true, true);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let access = inventory
            .operations()
            .iter()
            .filter_map(|site| Operation::get_op::<RankedAccessOp>(site.pointer(), &context))
            .find(|access| access.checked_success(&context).is_some())
            .unwrap();
        assert!(access.verify(&context).is_err());
        assert!(!run_pliron_ranked_bounds_check_v1(&context, &function).is_clean());
    }

    #[test]
    fn publication_raw_copy_preserves_exact_work_limit() {
        let (context, _, guard) = fixture(true, false);
        let mut exact = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 - 2;
        assert_eq!(
            evaluate_raw_index_at_invocation_v1(&context, guard.result(&context), &[], &mut exact),
            Some(200)
        );
        assert_eq!(exact, MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1);
        let mut insufficient = MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 - 1;
        assert_eq!(
            evaluate_raw_index_at_invocation_v1(
                &context,
                guard.result(&context),
                &[],
                &mut insufficient
            ),
            None
        );
    }
}

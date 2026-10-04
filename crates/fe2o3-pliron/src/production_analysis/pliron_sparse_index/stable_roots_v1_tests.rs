mod stable_root_tests {
    use super::*;
    use dialect_kernel::{DIALECT_NAME, IndexType, ReturnOp, register_dialect};
    use pliron::{builtin::types::FunctionType, dialect::DialectName, r#type::TypeHandle};

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        context
    }

    fn function(context: &mut Context, name: &str, arity: usize) -> (FuncOp, Vec<Value>) {
        let index: TypeHandle = IndexType::get(context).into();
        let function_type = FunctionType::get(context, vec![index; arity], vec![]);
        let function = FuncOp::new(context, name.try_into().unwrap(), function_type);
        let entry = function.get_entry_block(context);
        let values = (0..arity)
            .map(|i| entry.deref(context).get_argument(i))
            .collect();
        (function, values)
    }

    fn block(context: &mut Context, function: &FuncOp, name: &str) -> (Ptr<BasicBlock>, Value) {
        let index: TypeHandle = IndexType::get(context).into();
        let block = BasicBlock::new(context, Some(name.try_into().unwrap()), vec![index]);
        block.insert_at_back(function.get_region(context), context);
        let value = block.deref(context).get_argument(0);
        (block, value)
    }

    fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, op: &O) {
        op.get_operation().insert_at_back(block, context);
    }

    fn finish(context: &mut Context, block: Ptr<BasicBlock>) {
        let ret = ReturnOp::new(context);
        append(context, block, &ret);
    }

    #[test]
    fn stable_roots_preserve_parallel_edges_and_reject_divergent_distinct_roots() {
        for same in [true, false] {
            let context = &mut setup();
            let (function, args) = function(context, "parallel_roots", 2);
            let entry = function.get_entry_block(context);
            let (join, value) = block(context, &function, "join");
            let invocation = InvocationIndexOp::new(context, 0, 64);
            let one = IndexConstantOp::new(context, 1);
            let branch = IndexLessThanBranchArgsOp::new(
                context,
                invocation.result(context),
                one.result(context),
                vec![args[0]],
                vec![args[usize::from(!same)]],
                join,
                join,
            );
            append(context, entry, &invocation);
            append(context, entry, &one);
            append(context, entry, &branch);
            finish(context, join);
            let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
            assert_eq!(analysis.fact(value), SparseIndexFactV1::Unknown);
            assert_eq!(
                analysis.stable_root(context, &function, value),
                same.then_some(SparseStableRootV1::EntryArgument(args[0]))
            );
        }
    }

    #[test]
    fn stable_roots_wake_numeric_unknown_users_for_late_poison() {
        for same in [true, false] {
            let context = &mut setup();
            let (function, args) = function(context, "late_root_poison", 2);
            let entry = function.get_entry_block(context);
            let (join, joined) = block(context, &function, "join");
            let (sink, result) = block(context, &function, "sink");
            let delays = (0..4)
                .map(|i| block(context, &function, &format!("delay_{i}")))
                .collect::<Vec<_>>();
            let invocation = InvocationIndexOp::new(context, 0, 64);
            let one = IndexConstantOp::new(context, 1);
            let enter = IndexLessThanBranchArgsOp::new(
                context,
                invocation.result(context),
                one.result(context),
                vec![args[0]],
                vec![args[usize::from(!same)]],
                join,
                delays[3].0,
            );
            append(context, entry, &invocation);
            append(context, entry, &one);
            append(context, entry, &enter);
            let to_sink = BranchArgsOp::new(context, vec![joined], sink);
            append(context, join, &to_sink);
            finish(context, sink);
            for i in 0..4 {
                let target = if i == 0 { join } else { delays[i - 1].0 };
                let edge = BranchArgsOp::new(context, vec![delays[i].1], target);
                append(context, delays[i].0, &edge);
            }
            let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
            assert_eq!(analysis.fact(result), SparseIndexFactV1::Unknown);
            assert_eq!(
                analysis.stable_root(context, &function, result),
                same.then_some(SparseStableRootV1::EntryArgument(args[0]))
            );
        }
    }

    #[test]
    fn stable_roots_anchor_identity_cycles_but_not_changed_iteration_values() {
        for unchanged in [true, false] {
            let context = &mut setup();
            let (function, args) = function(context, "root_cycle", 2);
            let entry = function.get_entry_block(context);
            let (cycle, carried) = block(context, &function, "cycle");
            let (exit, result) = block(context, &function, "exit");
            let invocation = InvocationIndexOp::new(context, 0, 64);
            let one = IndexConstantOp::new(context, 1);
            let enter = BranchArgsOp::new(context, vec![args[0]], cycle);
            append(context, entry, &invocation);
            append(context, entry, &one);
            append(context, entry, &enter);
            let back = IndexLessThanBranchArgsOp::new(
                context,
                invocation.result(context),
                one.result(context),
                vec![if unchanged { carried } else { args[1] }],
                vec![carried],
                cycle,
                exit,
            );
            append(context, cycle, &back);
            finish(context, exit);
            let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
            assert_eq!(analysis.fact(result), SparseIndexFactV1::Unknown);
            assert_eq!(
                analysis.stable_root(context, &function, result),
                unchanged.then_some(SparseStableRootV1::EntryArgument(args[0]))
            );
        }
    }

    #[test]
    fn stable_roots_shared_queue_poison_unanchored_cycle_inputs() {
        let context = &mut setup();
        let (function, args) = function(context, "raw_pending_cycle", 1);
        let entry = function.get_entry_block(context);
        let (join, joined) = block(context, &function, "join");
        let (sink, result) = block(context, &function, "sink");
        let (cycle, carried) = block(context, &function, "cycle");
        let invocation = InvocationIndexOp::new(context, 0, 64);
        let one = IndexConstantOp::new(context, 1);
        // Defensive raw-CFG negative: this self-seeded entry edge does not
        // dominate its payload, so structural verification rejects it too.
        // The sparse queue must still never export its provisional join root.
        let enter = IndexLessThanBranchArgsOp::new(
            context,
            invocation.result(context),
            one.result(context),
            vec![args[0]],
            vec![carried],
            join,
            cycle,
        );
        append(context, entry, &invocation);
        append(context, entry, &one);
        append(context, entry, &enter);
        let back = IndexLessThanBranchArgsOp::new(
            context,
            invocation.result(context),
            one.result(context),
            vec![carried],
            vec![carried],
            cycle,
            join,
        );
        append(context, cycle, &back);
        let to_sink = BranchArgsOp::new(context, vec![joined], sink);
        append(context, join, &to_sink);
        finish(context, sink);
        let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
        assert_eq!(analysis.fact(result), SparseIndexFactV1::Unknown);
        for value in [carried, joined, result] {
            assert_eq!(analysis.stable_root(context, &function, value), None);
        }
    }

    #[test]
    fn stable_roots_close_unanchored_pending_inputs_before_export() {
        let context = &mut setup();
        let (function, args) = function(context, "pending_roots", 3);
        let definitions = [
            SparseDefinitionV1 {
                kind: SparseDefinitionKindV1::Merge(vec![args[0]]),
                result: args[0],
            },
            SparseDefinitionV1 {
                kind: SparseDefinitionKindV1::Merge(vec![args[0], args[2]]),
                result: args[1],
            },
            SparseDefinitionV1 {
                kind: SparseDefinitionKindV1::EntryArgument { ordinal: 2 },
                result: args[2],
            },
        ];
        let indices = args
            .iter()
            .copied()
            .enumerate()
            .map(|(i, value)| (value, i))
            .collect();
        let mut roots = vec![SparseStableRootLatticeV1::Pending; 3];
        roots[2] = SparseStableRootLatticeV1::Known(SparseStableRootV1::EntryArgument(args[2]));
        let mut work = 0;
        let provisional =
            derive_sparse_stable_root_v1(&definitions[1], roots[1], &roots, &indices, &mut work)
                .unwrap();
        assert_eq!(provisional, roots[2]);
        publish_sparse_stable_root_v1(&mut roots[1], provisional, &mut work).unwrap();
        publish_sparse_stable_root_v1(&mut roots[0], SparseStableRootLatticeV1::Unknown, &mut work)
            .unwrap();
        let closed =
            derive_sparse_stable_root_v1(&definitions[1], roots[1], &roots, &indices, &mut work)
                .unwrap();
        publish_sparse_stable_root_v1(&mut roots[1], closed, &mut work).unwrap();
        assert_eq!(roots[1], SparseStableRootLatticeV1::Unknown);
        let _ = function;
    }

    #[test]
    fn stable_roots_authenticate_owner_ordinal_and_context_before_publication() {
        let context = &mut setup();
        let (function, args) = function(context, "owner", 2);
        let entry = function.get_entry_block(context);
        let (foreign, foreign_args) = self::function(context, "foreign", 1);
        let constant = IndexConstantOp::new(context, 7);
        let foreign_entry = foreign.get_entry_block(context);
        append(context, foreign_entry, &constant);
        finish(context, foreign_entry);
        finish(context, entry);
        for definition in [
            SparseDefinitionV1 {
                kind: SparseDefinitionKindV1::EntryArgument { ordinal: 0 },
                result: foreign_args[0],
            },
            SparseDefinitionV1 {
                kind: SparseDefinitionKindV1::EntryArgument { ordinal: 1 },
                result: args[0],
            },
            SparseDefinitionV1 {
                kind: SparseDefinitionKindV1::Operation(constant.get_operation()),
                result: constant.result(context),
            },
        ] {
            let mut work = 0;
            assert!(matches!(
                seed_sparse_stable_root_v1(context, entry, &definition, &mut work),
                Err(SparseIndexFailureV1::MalformedControlFlow { .. })
            ));
            assert_eq!(work, 16);
        }
        let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
        assert!(analysis.stable_root(context, &function, args[0]).is_some());
        assert_eq!(analysis.stable_root(context, &foreign, args[0]), None);
        assert_eq!(
            analysis.stable_root(context, &function, foreign_args[0]),
            None
        );
        let other_context = setup();
        assert_eq!(
            analysis.stable_root(&other_context, &function, args[0]),
            None
        );
    }

    #[test]
    fn stable_roots_seed_only_literals_and_copy_identity_not_arithmetic() {
        let context = &mut setup();
        let (function, _) = function(context, "literal_roots", 0);
        let entry = function.get_entry_block(context);
        let one = IndexConstantOp::new(context, 1);
        let sum = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            one.result(context),
            one.result(context),
        );
        append(context, entry, &one);
        append(context, entry, &sum);
        finish(context, entry);
        let analysis = analyze_pliron_sparse_indices_v1(context, &function).unwrap();
        assert_eq!(
            analysis.stable_root(context, &function, one.result(context)),
            Some(SparseStableRootV1::Constant(1))
        );
        assert_eq!(
            analysis.stable_root(context, &function, sum.result(context)),
            None
        );
        assert_eq!(analysis.fact(sum.result(context)).constant_value(), Some(2));
    }

    #[test]
    fn stable_roots_have_prepaid_exact_and_one_under_boundaries() {
        let context = &mut setup();
        let (function, _) = function(context, "root_work", 0);
        let entry = function.get_entry_block(context);
        let constant = IndexConstantOp::new(context, 7);
        append(context, entry, &constant);
        let definition = SparseDefinitionV1 {
            kind: SparseDefinitionKindV1::Operation(constant.get_operation()),
            result: constant.result(context),
        };
        // One root: vector/owner lifecycle48 + authenticated literal seed24.
        let mut exact = MAX_SPARSE_INDEX_WORK_UNITS_V1 - 72;
        assert_eq!(
            initialize_sparse_stable_roots_v1(
                context,
                entry,
                std::slice::from_ref(&definition),
                &mut exact
            )
            .unwrap()
            .len(),
            1
        );
        assert_eq!(exact, MAX_SPARSE_INDEX_WORK_UNITS_V1);
        let mut under = MAX_SPARSE_INDEX_WORK_UNITS_V1 - 71;
        assert!(
            initialize_sparse_stable_roots_v1(
                context,
                entry,
                std::slice::from_ref(&definition),
                &mut under
            )
            .is_err()
        );
        assert_eq!(under, MAX_SPARSE_INDEX_WORK_UNITS_V1 + 1);
        let mut before_allocation = MAX_SPARSE_INDEX_WORK_UNITS_V1 - 47;
        assert!(
            initialize_sparse_stable_roots_v1(
                context,
                entry,
                &[definition],
                &mut before_allocation
            )
            .is_err()
        );
        assert_eq!(before_allocation, MAX_SPARSE_INDEX_WORK_UNITS_V1 + 1);
        let mut admitted_root = SparseStableRootLatticeV1::Pending;
        let mut admitted_work = MAX_SPARSE_INDEX_WORK_UNITS_V1 - 1;
        assert!(
            publish_sparse_stable_root_v1(
                &mut admitted_root,
                SparseStableRootLatticeV1::Known(SparseStableRootV1::Constant(7)),
                &mut admitted_work
            )
            .unwrap()
        );
        assert_eq!(admitted_work, MAX_SPARSE_INDEX_WORK_UNITS_V1);
        let mut root = SparseStableRootLatticeV1::Pending;
        let mut work = MAX_SPARSE_INDEX_WORK_UNITS_V1;
        assert!(
            publish_sparse_stable_root_v1(&mut root, SparseStableRootLatticeV1::Unknown, &mut work)
                .is_err()
        );
        assert_eq!(root, SparseStableRootLatticeV1::Pending);
        assert_eq!(work, MAX_SPARSE_INDEX_WORK_UNITS_V1 + 1);
        assert!(
            std::mem::size_of::<SparseStableRootLatticeV1>() <= 8 * std::mem::size_of::<usize>()
        );
        assert!(
            std::mem::size_of::<Option<SparseStableRootV1>>() <= 8 * std::mem::size_of::<usize>()
        );
        assert!(
            std::mem::size_of::<SparseValueFactsV1>()
                <= std::mem::size_of::<SparseIndexFactV1>() + 8 * std::mem::size_of::<usize>()
        );
        assert!(std::mem::size_of::<SparseDefinitionKindV1>() <= 4 * std::mem::size_of::<usize>());
    }

    #[test]
    fn stable_roots_reuse_recorded_ordinals_for_many_entry_arguments() {
        let context = &mut setup();
        let (function, args) = function(context, "many_entry_roots", 1_024);
        let entry = function.get_entry_block(context);
        finish(context, entry);
        let inventory = BoundedPlironFunctionInventoryV1::collect(context, &function).unwrap();
        let (analysis, work) =
            analyze_pliron_sparse_indices_with_work_v1(context, &function, &inventory).unwrap();
        // Reachability1 + setup(32A+16) + seed16A + countersA + loop4A
        // + final closureA. Roots reuse ordinals; the exact type sidecar adds
        // one prepaid argument-type scan per value, Q=A*(A+1)/2.
        assert_eq!(work, 54 * 1_024 + 17 + 1_024 * 1_025 / 2);
        for arg in args {
            assert_eq!(analysis.fact(arg), SparseIndexFactV1::Unknown);
            assert_eq!(
                analysis.stable_root(context, &function, arg),
                Some(SparseStableRootV1::EntryArgument(arg))
            );
        }
    }
}

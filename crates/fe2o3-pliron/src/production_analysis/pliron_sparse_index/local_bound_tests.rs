#[cfg(test)]
mod local_bound_tests {
    use super::*;
    use crate::KernelCheckStatusV1;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_ranked_bounds::{
        RankedBoundsFindingV1, run_pliron_ranked_bounds_check_v1,
    };
    use dialect_kernel::{
        BranchOp, DIALECT_NAME, IndexType, IndexUnknownOp, ReturnOp, register_dialect,
    };
    use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op, r#type::TypeHandle};

    fn setup() -> Context {
        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        context
    }

    fn function(context: &mut Context, name: &str) -> FuncOp {
        FuncOp::new(
            context,
            name.try_into().unwrap(),
            FunctionType::get(context, vec![], vec![]),
        )
    }

    fn block(
        context: &mut Context,
        function: &FuncOp,
        name: &str,
        indexed: bool,
    ) -> Ptr<BasicBlock> {
        let types: Vec<TypeHandle> = if indexed {
            vec![IndexType::get(context).into()]
        } else {
            vec![]
        };
        let result = BasicBlock::new(context, Some(name.try_into().unwrap()), types);
        result.insert_at_back(function.get_region(context), context);
        result
    }

    fn argument(context: &Context, block: Ptr<BasicBlock>) -> Value {
        block.deref(context).get_argument(0)
    }

    fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
        operation.get_operation().insert_at_back(block, context);
    }

    fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
    }

    fn authenticated_census(
        context: &Context,
        function: &FuncOp,
    ) -> ProductionAnalysisInputCensusV1 {
        LivePlironStructuralIdentityProviderV1::new(context, function)
            .capture_with_resource_limits_v1(unlimited())
            .ok()
            .expect("the genuine function passes structural identity verification")
            .input_census
    }

    fn analyze_with_work(context: &Context, function: &FuncOp) -> (SparseIndexAnalysisV1, usize) {
        let inventory = BoundedPlironFunctionInventoryV1::collect(context, function).unwrap();
        analyze_pliron_sparse_indices_with_work_v1(context, function, &inventory).unwrap()
    }

    fn overflow(rhs: u64) -> SparseIndexFactV1 {
        SparseIndexFactV1::MachineOverflow(SparseMachineOverflowV1 {
            operation: IndexBinaryKindAttr::Add,
            invocation: vec![0],
            lhs: u64::MAX,
            rhs,
        })
    }

    #[test]
    fn differing_overflow_witnesses_are_not_replaced_by_unknown_at_the_ceiling() {
        let first = derive_binary(
            Some(IndexBinaryKindAttr::Add),
            overflow(1),
            overflow(2),
            &[1],
        );
        let second = derive_binary(
            Some(IndexBinaryKindAttr::Add),
            SparseIndexFactV1::Unknown,
            overflow(2),
            &[1],
        );
        let third = derive_binary(
            Some(IndexBinaryKindAttr::Add),
            SparseIndexFactV1::Unknown,
            SparseIndexFactV1::Unknown,
            &[1],
        );
        assert_eq!(first, overflow(1));
        assert_eq!(second, overflow(2));
        assert_eq!(third, SparseIndexFactV1::Unknown);
        let mut current = SparseIndexLatticeV1::Pending;
        let mut publications = 0;
        let mut work = 0;
        assert!(
            publish_sparse_fact_v1(
                &mut current,
                SparseIndexLatticeV1::Known(first),
                &mut publications,
                &mut work
            )
            .unwrap()
        );
        assert!(
            publish_sparse_fact_v1(
                &mut current,
                SparseIndexLatticeV1::Known(second.clone()),
                &mut publications,
                &mut work
            )
            .unwrap()
        );
        assert_eq!(
            publish_sparse_fact_v1(
                &mut current,
                SparseIndexLatticeV1::Known(third),
                &mut publications,
                &mut work
            ),
            Err(limit("sparse fact publications", 2, 3))
        );
        assert_eq!(current, SparseIndexLatticeV1::Known(second));
        assert_eq!(publications, 2);
        assert_eq!(work, 3);
    }

    #[test]
    fn equal_facts_are_paid_but_do_not_consume_publications() {
        let known = SparseIndexLatticeV1::Known(overflow(1));
        let unknown = SparseIndexLatticeV1::Known(SparseIndexFactV1::Unknown);
        let mut current = SparseIndexLatticeV1::Pending;
        let mut publications = 0;
        let mut work = 0;
        for (next, changed) in [
            (SparseIndexLatticeV1::Pending, false),
            (known.clone(), true),
            (known, false),
            (unknown.clone(), true),
            (unknown.clone(), false),
        ] {
            assert_eq!(
                publish_sparse_fact_v1(&mut current, next, &mut publications, &mut work),
                Ok(changed)
            );
        }
        assert_eq!(current, unknown);
        assert_eq!(publications, 2);
        assert_eq!(work, 5);
    }

    #[test]
    fn publication_admission_is_exact_and_denial_precedes_state_mutation() {
        let next = SparseIndexLatticeV1::Known(overflow(1));
        let mut current = SparseIndexLatticeV1::Pending;
        let mut publications = 0;
        let mut work = MAX_SPARSE_INDEX_WORK_UNITS_V1 - 1;
        assert_eq!(
            publish_sparse_fact_v1(&mut current, next.clone(), &mut publications, &mut work),
            Ok(true)
        );
        assert_eq!(work, MAX_SPARSE_INDEX_WORK_UNITS_V1);
        let mut blocked = next.clone();
        let mut at_ceiling = 2;
        assert_eq!(
            publish_sparse_fact_v1(
                &mut blocked,
                SparseIndexLatticeV1::Known(overflow(2)),
                &mut at_ceiling,
                &mut work
            ),
            Err(limit(
                "sparse propagation work",
                MAX_SPARSE_INDEX_WORK_UNITS_V1,
                MAX_SPARSE_INDEX_WORK_UNITS_V1 + 1
            ))
        );
        assert_eq!(blocked, next);
        assert_eq!(at_ceiling, 2);
        assert_eq!(publications, 1);
    }

    fn duplicate_join(context: &mut Context, dead_predecessors: usize) -> (FuncOp, Value) {
        let function = function(context, "duplicate_join");
        let entry = function.get_entry_block(context);
        let join = block(context, &function, "join", true);
        let seven = IndexConstantOp::new(context, 7);
        append(context, entry, &seven);
        let value = seven.result(context);
        let branch = IndexEqualBranchArgsOp::new(
            context,
            value,
            value,
            vec![value],
            vec![value],
            join,
            join,
        );
        append(context, entry, &branch);
        let ret = ReturnOp::new(context);
        append(context, join, &ret);
        for index in 0..dead_predecessors {
            let dead = block(context, &function, &format!("dead_{index}"), false);
            let nine = IndexConstantOp::new(context, 9);
            append(context, dead, &nine);
            let edge = BranchArgsOp::new(context, vec![nine.result(context)], join);
            append(context, dead, &edge);
        }
        (function, argument(context, join))
    }

    #[test]
    fn authenticated_duplicate_edges_have_literal_work_and_storage_boundaries() {
        let context = &mut setup();
        let (function, joined) = duplicate_join(context, 0);
        let census = authenticated_census(context, &function);
        assert_eq!(
            (
                census.blocks,
                census.operations,
                census.operands,
                census.results,
                census.successors,
                census.block_arguments
            ),
            (2, 3, 4, 1, 2, 1)
        );
        let shape =
            collect_sparse_index_merge_census_v1(context, &function, census, unlimited()).unwrap();
        assert_eq!(
            shape,
            SparseIndexMergeCensusV1 {
                inputs: 2,
                squared_inputs: 4,
                argument_type_work: 1,
            }
        );
        // V=2,D=12,structural=13; propagation=4+124+240+4+32+16=420.
        // Work=104+3+420+1=528; retained=80 and temporary=170.
        let bound = preflight_sparse_index_resource_upper_bound_v1(
            context,
            &function,
            census,
            ProductionAnalysisResourceLimitsV1::new(528, 250),
        )
        .unwrap();
        assert_eq!(
            (
                bound.work_upper_bound(),
                bound.retained_storage_upper_bound(),
                bound.peak_storage_upper_bound()
            ),
            (528, 80, 250)
        );
        for limits in [
            ProductionAnalysisResourceLimitsV1::new(527, 250),
            ProductionAnalysisResourceLimitsV1::new(528, 249),
        ] {
            assert!(
                preflight_sparse_index_resource_upper_bound_v1(context, &function, census, limits)
                    .is_err()
            );
        }
        let (analysis, work) = analyze_with_work(context, &function);
        assert_eq!(analysis.fact(joined).constant_value(), Some(7));
        // Numeric14 + root setup80 + seeds40 + dispatch/publication4 +
        // root merge2 + final closure2 =142; no extra queue pop is needed.
        assert_eq!(work, 143);
    }

    #[test]
    fn unreachable_high_fan_in_is_counted_but_does_not_change_reachable_facts() {
        for dead in [1, 32, 128] {
            let context = &mut setup();
            let (function, joined) = duplicate_join(context, dead);
            let census = authenticated_census(context, &function);
            let shape =
                collect_sparse_index_merge_census_v1(context, &function, census, unlimited())
                    .unwrap();
            let degree = dead + 2;
            assert_eq!(
                shape,
                SparseIndexMergeCensusV1 {
                    inputs: degree,
                    squared_inputs: degree * degree,
                    argument_type_work: 1,
                }
            );
            let bound = preflight_sparse_index_resource_upper_bound_v1(
                context,
                &function,
                census,
                unlimited(),
            )
            .unwrap();
            let (analysis, work) = analyze_with_work(context, &function);
            assert_eq!(analysis.fact(joined).constant_value(), Some(7));
            assert!(work <= bound.work_upper_bound());
        }
    }

    #[test]
    fn self_loop_revisits_preserve_equality_and_have_literal_runtime_work() {
        let context = &mut setup();
        let function = function(context, "self_loop");
        let entry = function.get_entry_block(context);
        let cycle = block(context, &function, "cycle", true);
        let value = argument(context, cycle);
        let seven = IndexConstantOp::new(context, 7);
        append(context, entry, &seven);
        let enter = BranchArgsOp::new(context, vec![seven.result(context)], cycle);
        append(context, entry, &enter);
        let back = BranchArgsOp::new(context, vec![value], cycle);
        append(context, cycle, &back);
        let census = authenticated_census(context, &function);
        assert_eq!(
            collect_sparse_index_merge_census_v1(context, &function, census, unlimited()).unwrap(),
            SparseIndexMergeCensusV1 {
                inputs: 2,
                squared_inputs: 4,
                argument_type_work: 1,
            }
        );
        let (analysis, work) = analyze_with_work(context, &function);
        assert_eq!(analysis.fact(value).constant_value(), Some(7));
        // Numeric18 + setup80 + seeds40 + root dispatch/publication6 +
        // root merge4 + final closure2 =150.
        assert_eq!(work, 151);
    }

    #[test]
    fn repeated_operation_operands_are_charged_even_when_queueing_is_deduplicated() {
        let context = &mut setup();
        let function = function(context, "repeated_operands");
        let entry = function.get_entry_block(context);
        let seven = IndexConstantOp::new(context, 7);
        append(context, entry, &seven);
        let add = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            seven.result(context),
            seven.result(context),
        );
        append(context, entry, &add);
        let ret = ReturnOp::new(context);
        append(context, entry, &ret);
        let (analysis, work) = analyze_with_work(context, &function);
        assert_eq!(
            analysis.fact(add.result(context)).constant_value(),
            Some(14)
        );
        // Numeric9 + setup80 + seeds40 + dispatch/publication4 + closure2.
        assert_eq!(work, 135);
    }

    fn staggered_overflow_function(context: &mut Context) -> FuncOp {
        let function = function(context, "staggered_overflows");
        let entry = function.get_entry_block(context);
        let left = block(context, &function, "left", true);
        let right = block(context, &function, "right", true);
        let late_left = block(context, &function, "late_left", true);
        // Reverse roster order delays the right Unknown beyond the left update
        // and the binary's second publication. Every edge and SSA owner is real.
        let late_right = (0..4)
            .map(|index| block(context, &function, &format!("late_right_{index}"), true))
            .collect::<Vec<_>>();
        let maximum = IndexConstantOp::new(context, u64::MAX);
        let one = IndexConstantOp::new(context, 1);
        let two = IndexConstantOp::new(context, 2);
        let unknown = IndexUnknownOp::new(context);
        for op in [
            maximum.get_operation(),
            one.get_operation(),
            two.get_operation(),
            unknown.get_operation(),
        ] {
            op.insert_at_back(entry, context);
        }
        let overflow_left = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            maximum.result(context),
            one.result(context),
        );
        let overflow_right = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            maximum.result(context),
            two.result(context),
        );
        append(context, entry, &overflow_left);
        append(context, entry, &overflow_right);
        let enter = IndexEqualBranchArgsOp::new(
            context,
            one.result(context),
            two.result(context),
            vec![overflow_left.result(context)],
            vec![unknown.result(context)],
            left,
            late_left,
        );
        append(context, entry, &enter);
        let to_left = BranchArgsOp::new(context, vec![argument(context, late_left)], left);
        append(context, late_left, &to_left);
        let to_right = IndexEqualBranchArgsOp::new(
            context,
            one.result(context),
            two.result(context),
            vec![overflow_right.result(context)],
            vec![unknown.result(context)],
            right,
            late_right[3],
        );
        append(context, left, &to_right);
        for index in 0..4 {
            let target = if index == 0 {
                right
            } else {
                late_right[index - 1]
            };
            let edge =
                BranchArgsOp::new(context, vec![argument(context, late_right[index])], target);
            append(context, late_right[index], &edge);
        }
        let result = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Add,
            argument(context, left),
            argument(context, right),
        );
        append(context, right, &result);
        let ret = ReturnOp::new(context);
        append(context, right, &ret);
        function
    }

    #[test]
    fn genuine_staggered_overflow_cfg_rejects_a_third_publication() {
        let context = &mut setup();
        let function = staggered_overflow_function(context);
        let census = authenticated_census(context, &function);
        preflight_sparse_index_resource_upper_bound_v1(context, &function, census, unlimited())
            .unwrap();
        assert_eq!(
            analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
            limit("sparse fact publications", 2, 3)
        );
    }

    #[test]
    fn production_ranked_checks_keep_the_publication_error_terminal() {
        let context = &mut setup();
        let function = staggered_overflow_function(context);
        let report = run_pliron_ranked_bounds_check_v1(context, &function);
        assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
        assert_eq!(
            report.findings(),
            &[RankedBoundsFindingV1::SparseIndexAnalysisFailed {
                detail: "sparse fact publications count 3 exceeds 2".to_owned()
            }]
        );
    }

    #[test]
    fn merge_census_scan_is_prepaid_and_rejects_count_mismatches() {
        let context = &mut setup();
        let (function, _) = duplicate_join(context, 0);
        let census = authenticated_census(context, &function);
        assert_eq!(sparse_index_merge_census_work_v1(census).unwrap(), 3);
        assert!(
            collect_sparse_index_merge_census_v1(
                context,
                &function,
                census,
                ProductionAnalysisResourceLimitsV1::new(3, 0)
            )
            .is_ok()
        );
        let under = collect_sparse_index_merge_census_v1(
            context,
            &function,
            census,
            ProductionAnalysisResourceLimitsV1::new(2, 0),
        )
        .unwrap_err();
        assert_eq!(under.resource, "work upper bound");
        for changed in [
            ProductionAnalysisInputCensusV1 {
                blocks: 1,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                blocks: 3,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                block_arguments: 0,
                ..census
            },
        ] {
            assert_eq!(
                collect_sparse_index_merge_census_v1(context, &function, changed, unlimited()),
                Err(sparse_index_resource_error_v1(
                    "sparse-index merge census mismatch"
                ))
            );
        }
        let invalid = ProductionAnalysisInputCensusV1 {
            blocks: usize::MAX,
            ..census
        };
        assert!(
            collect_sparse_index_merge_census_v1(context, &function, invalid, unlimited()).is_err()
        );
    }

    #[test]
    fn checked_merge_moments_and_existing_hard_caps_cannot_wrap() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 1,
            operations: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        for shape in [
            SparseIndexMergeCensusV1 {
                inputs: usize::MAX,
                squared_inputs: 0,
                argument_type_work: 0,
            },
            SparseIndexMergeCensusV1 {
                inputs: 0,
                squared_inputs: usize::MAX,
                argument_type_work: 0,
            },
            SparseIndexMergeCensusV1 {
                inputs: 1,
                squared_inputs: MAX_SPARSE_INDEX_WORK_UNITS_V1,
                argument_type_work: 0,
            },
        ] {
            assert!(sparse_index_resource_upper_bound_v1(census, shape, unlimited()).is_err());
        }
        assert!(checked_sparse_index_product_v1(usize::MAX, 2, "merge product").is_err());
        for oversized in [
            ProductionAnalysisInputCensusV1 {
                results: MAX_SPARSE_INDEX_VALUES_V1 + 1,
                ..census
            },
            ProductionAnalysisInputCensusV1 {
                operands: MAX_SPARSE_INDEX_USES_V1,
                successors: 1,
                ..census
            },
        ] {
            assert!(checked_sparse_index_population_v1(oversized).is_err());
        }
        assert_eq!(MAX_SPARSE_INDEX_VALUES_V1, 65_536);
        assert_eq!(MAX_SPARSE_INDEX_USES_V1, 262_144);
        assert_eq!(MAX_SPARSE_INDEX_WORK_UNITS_V1, 1_048_576);
    }

    #[test]
    fn malformed_typed_edges_still_fail_before_propagation() {
        let context = &mut setup();
        let function = function(context, "untyped_phi_edge");
        let entry = function.get_entry_block(context);
        let join = block(context, &function, "join", true);
        let branch = BranchOp::new(context, join);
        append(context, entry, &branch);
        let ret = ReturnOp::new(context);
        append(context, join, &ret);
        assert_eq!(
            analyze_pliron_sparse_indices_v1(context, &function).unwrap_err(),
            malformed("a block argument has a predecessor without typed edge operands")
        );
    }

    #[test]
    fn local_merge_bound_does_not_square_unrelated_operations() {
        let context = &mut setup();
        let function = function(context, "independent_constants");
        let entry = function.get_entry_block(context);
        for index in 0..1_100 {
            let constant = IndexConstantOp::new(context, index);
            append(context, entry, &constant);
        }
        let ret = ReturnOp::new(context);
        append(context, entry, &ret);
        let census = authenticated_census(context, &function);
        assert_eq!(
            (census.blocks, census.operations, census.results),
            (1, 1_101, 1_100)
        );
        assert_eq!(
            collect_sparse_index_merge_census_v1(context, &function, census, unlimited()).unwrap(),
            SparseIndexMergeCensusV1::default()
        );
        // The old D^2 term alone was 1101^2=1212201. The new bound is
        // 8*2202 + scan2 + (1+62*1100+20*1101+16) = 107855.
        // Retained30824 + temporary67116 = peak97940; no merge rescan term.
        let old_square = census.operations * census.operations;
        assert_eq!(old_square, 1_212_201);
        assert!(old_square > MAX_SPARSE_INDEX_WORK_UNITS_V1);
        let bound = preflight_sparse_index_resource_upper_bound_v1(
            context,
            &function,
            census,
            ProductionAnalysisResourceLimitsV1::new(107_855, 97_940),
        )
        .unwrap();
        assert_eq!(
            (bound.work_upper_bound(), bound.peak_storage_upper_bound()),
            (107_855, 97_940)
        );
        let (analysis, work) = analyze_with_work(context, &function);
        assert_eq!(analysis.facts.len(), 1_100);
        // Reachability1 + root setup(32V+16) + literal seeds24V + countersV
        // + four units per pop + final closureV = 62V+17.
        assert_eq!(work, 68_217);
    }
}

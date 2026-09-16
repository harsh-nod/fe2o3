#[cfg(test)]
fn component_race_names_v1() -> RaceNameCensusV1 {
    // Inert formula inputs only; production obtains these from the live graph.
    RaceNameCensusV1 {
        name_storage: 64,
        scan_work: 0,
        execution_lookup_work: 0,
    }
}

#[cfg(test)]
mod name_census_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::{
        LivePlironStructuralIdentityProviderV1, derive_pliron_ir_structural_identity_v1,
    };
    use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
    use dialect_kernel::{IndexType, RankedViewType, ReturnOp};
    use pliron::{
        builtin::types::FunctionType,
        debug_info::{set_block_arg_name, set_operation_result_name},
        dialect::DialectName,
    };
    use std::{
        cell::Cell,
        panic::{AssertUnwindSafe, catch_unwind},
    };

    fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
    }

    fn context() -> Context {
        let mut context = Context::new();
        dialect_kernel::register_dialect(
            &mut context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(&mut context).unwrap();
        context
    }

    fn fixture(
        context: &mut Context,
        name: &str,
        arguments: usize,
        accesses: usize,
    ) -> (FuncOp, Value, Value) {
        assert!(arguments > 0);
        let ty = IndexType::get(context).into();
        let function_type = FunctionType::get(context, vec![ty; arguments], vec![]);
        let function = FuncOp::new(context, name.try_into().unwrap(), function_type);
        let entry = function.get_entry_block(context);
        let index = entry.deref(context).get_argument(arguments - 1);
        let view_type = RankedViewType::new(context, 32, true, vec![8]).unwrap();
        let view = RankedViewOp::new_in_space(context, view_type, vec![], MemorySpaceAttr::Global)
            .unwrap();
        view.get_operation().insert_at_back(entry, context);
        for _ in 0..accesses {
            // Read-only effects still construct names before later race filtering.
            let access = RankedAccessOp::new(
                context,
                AccessKindAttr::Read,
                view.result(context),
                vec![index],
            )
            .unwrap();
            access.get_operation().insert_at_back(entry, context);
        }
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(entry, context);
        (function, view.result(context), index)
    }

    fn census(context: &Context, function: &FuncOp) -> ProductionAnalysisInputCensusV1 {
        LivePlironStructuralIdentityProviderV1::new(context, function)
            .capture_with_resource_limits_v1(unlimited())
            .ok()
            .expect("actual fixture passes structural identity admission")
            .input_census
    }

    fn rename(context: &Context, value: Value, name: &str) {
        let index = value.find_index(context);
        let name = Some(name.try_into().unwrap());
        match value.defining_entity() {
            DefiningEntity::Op(operation) => {
                set_operation_result_name(context, operation, index, name)
            }
            DefiningEntity::Block(block) => set_block_arg_name(context, block, index, name),
        }
    }

    #[test]
    fn race_name_census_matches_actual_names_and_counts_repeated_read_operands() {
        for arguments in [1, 37] {
            let mut context = context();
            let (function, view, index) = fixture(&mut context, "name_population", arguments, 2);
            rename(&context, view, &"v".repeat(8_192));
            rename(&context, index, &"i".repeat(16_384));
            let census = census(&context, &function);
            let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
            let mut queried = Vec::new();
            let names = collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                census,
                unlimited(),
                |value| {
                    queried.push(value);
                    value.unique_name_byte_len(&context)
                },
            )
            .unwrap();
            assert_eq!(queried, [view, index, view, index]);
            assert_eq!(
                names.name_storage,
                index.unique_name(&context).as_ref().len()
            );
            assert!(names.name_storage > view.unique_name(&context).as_ref().len());
            let lookup =
                census.max_operation_arity.max(census.block_arguments) + 20 * census.attributes;
            assert_eq!(names.execution_lookup_work, lookup);
            assert_eq!(
                names.scan_work,
                32 + 8 * census.blocks
                    + 12 * census.operations
                    + 4 * census.operands
                    + (census.operands + census.operations) * (lookup + 80)
            );
            let mut analyses = PlironAnalysisManagerV1::new(&function);
            analyses.prepare_sparse_indices(&context, &function);
            let sparse = analyses.sparse_indices().unwrap();
            let expected = race_resource_upper_bound_for_shape_v1(
                census,
                names,
                static_invocation_shape_for_resource_v1(sparse, None),
                presburger_invocation_shape_for_resource_v1(sparse.launch_extents()),
                unlimited(),
            )
            .unwrap();
            assert_eq!(
                preflight_race_resource_upper_bound_v1(
                    &context,
                    &function,
                    analyses.function_inventory().ok(),
                    census,
                    sparse,
                    None,
                    unlimited(),
                ),
                Ok(expected)
            );
            assert_eq!(
                preflight_race_resource_upper_bound_v1(
                    &context,
                    &function,
                    None,
                    census,
                    sparse,
                    None,
                    unlimited(),
                ),
                Err(race_name_census_error_v1())
            );
        }
    }

    #[test]
    fn race_name_census_admits_scratch_and_work_before_queries_and_charges_scan_once() {
        let mut context = context();
        let (function, _, _) = fixture(&mut context, "name_scan_limits", 1, 1);
        let census = census(&context, &function);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let work = race_name_scan_work_v1(census).unwrap();
        for (work_limit, peak_limit, expected) in [
            (work - 1, 16, "work upper bound"),
            (work, 15, "peak storage upper bound"),
        ] {
            let queries = Cell::new(0);
            let error = collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                census,
                ProductionAnalysisResourceLimitsV1::new(work_limit, peak_limit),
                |value| {
                    queries.set(queries.get() + 1);
                    value.unique_name_byte_len(&context)
                },
            )
            .unwrap_err();
            assert_eq!(queries.get(), 0);
            assert_eq!(
                error,
                ProductionAnalysisResourceLimitV1 {
                    phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                    resource: expected
                }
            );
        }
        let names = collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            census,
            ProductionAnalysisResourceLimitsV1::new(work, 16),
            |value| value.unique_name_byte_len(&context),
        )
        .unwrap();
        assert_eq!(names.scan_work, work);
        assert_eq!(names.name_storage, 38);
        assert_eq!(RACE_NAME_CENSUS_SCRATCH_V1, 8 + 3 + 2 + 1 + 2);
        assert_eq!(
            std::mem::size_of::<RaceNameScanV1>(),
            8 * std::mem::size_of::<usize>()
        );
        assert_eq!(
            std::mem::size_of::<RaceNameCensusV1>(),
            3 * std::mem::size_of::<usize>()
        );
        let with_scan =
            calculate_race_resource_upper_bound_for_shape_v1(census, names, Some((2, 1)), None)
                .unwrap();
        let without_scan = calculate_race_resource_upper_bound_for_shape_v1(
            census,
            RaceNameCensusV1 {
                scan_work: 0,
                ..names
            },
            Some((2, 1)),
            None,
        )
        .unwrap();
        assert_eq!(
            with_scan.bound.work_upper_bound() - without_scan.bound.work_upper_bound(),
            work
        );
        assert_eq!(
            with_scan.bound.peak_storage_upper_bound(),
            without_scan.bound.peak_storage_upper_bound()
        );
    }

    #[test]
    fn race_name_census_checks_count_and_arity_prefixes_before_affected_queries() {
        let mut context = context();
        let (function, _, _) = fixture(&mut context, "name_prefix", 37, 1);
        let actual = census(&context, &function);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        assert!(actual.operands > 0 && actual.results > 0 && actual.attributes > 0);
        let mut cases = Vec::new();
        let mut changed = actual;
        changed.blocks -= 1;
        cases.push(changed);
        let mut changed = actual;
        changed.operations -= 1;
        cases.push(changed);
        let mut changed = actual;
        changed.operands -= 1;
        cases.push(changed);
        let mut changed = actual;
        changed.results -= 1;
        cases.push(changed);
        let mut changed = actual;
        changed.block_arguments -= 1;
        cases.push(changed);
        let mut changed = actual;
        changed.attributes = 0;
        cases.push(changed);
        let mut changed = actual;
        changed.max_operation_arity = 1;
        cases.push(changed);
        let mut changed = actual;
        changed.ranked_accesses = 0;
        cases.push(changed);
        for supplied in cases {
            let queries = Cell::new(0);
            let error = collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                supplied,
                unlimited(),
                |value| {
                    queries.set(queries.get() + 1);
                    value.unique_name_byte_len(&context)
                },
            )
            .unwrap_err();
            assert_eq!(error, race_name_census_error_v1());
            assert_eq!(queries.get(), 0);
        }
        // A count surplus is an inert hostile input, not authenticated custody.
        let mut surplus = actual;
        surplus.operands += 1;
        let queries = Cell::new(0);
        assert_eq!(
            collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                surplus,
                unlimited(),
                |value| {
                    queries.set(queries.get() + 1);
                    value.unique_name_byte_len(&context)
                }
            )
            .unwrap_err(),
            race_name_census_error_v1()
        );
        assert_eq!(queries.get(), 2);
    }

    #[test]
    fn race_name_census_rejects_foreign_inventory_and_value_before_lookup() {
        let mut context = context();
        let (function, _, _) = fixture(&mut context, "name_owner", 1, 1);
        let (foreign, view, _) = fixture(&mut context, "name_foreign", 1, 1);
        let actual = census(&context, &function);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &foreign).unwrap();
        assert_eq!(
            collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                actual,
                unlimited(),
                |_| { panic!("foreign inventory reached name lookup") }
            )
            .unwrap_err(),
            race_name_census_error_v1()
        );
        assert_eq!(
            require_race_name_value_v1(&context, &function, view, actual),
            Err(race_name_census_error_v1())
        );
    }

    #[test]
    fn race_name_census_refreshes_debug_only_names_and_rejects_mid_scan_mutation() {
        let mut context = context();
        let (function, view, index) = fixture(&mut context, "name_epoch", 1, 1);
        rename(&context, view, "short_view");
        rename(&context, index, "short_index");
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let before = derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        let old_census = census(&context, &function);
        let old_epoch = context.ir_mutation_attempt_epoch().unwrap();
        let old = collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            old_census,
            unlimited(),
            |value| value.unique_name_byte_len(&context),
        )
        .unwrap();
        rename(&context, view, &"x".repeat(32_768));
        let after = derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        assert!(before.exactly_matches(&after));
        assert_ne!(old_epoch, context.ir_mutation_attempt_epoch().unwrap());
        let fresh_census = census(&context, &function);
        assert_eq!(old_census.identifier_bytes, fresh_census.identifier_bytes);
        let fresh = collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            fresh_census,
            unlimited(),
            |value| value.unique_name_byte_len(&context),
        )
        .unwrap();
        assert!(fresh.name_storage > old.name_storage);
        assert_eq!(
            fresh.name_storage,
            view.unique_name(&context).as_ref().len()
        );
        let queries = Cell::new(0);
        assert_eq!(
            collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                fresh_census,
                unlimited(),
                |value| {
                    if queries.get() == 0 {
                        rename(&context, index, "changed_during_scan");
                    }
                    queries.set(queries.get() + 1);
                    value.unique_name_byte_len(&context)
                }
            )
            .unwrap_err(),
            race_name_census_error_v1()
        );
        assert_eq!(queries.get(), 2);
    }

    #[test]
    fn race_name_census_query_error_and_panic_do_not_publish_a_partial_maximum() {
        let mut context = context();
        let (function, _, _) = fixture(&mut context, "name_failure", 1, 1);
        let census = census(&context, &function);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let epoch = context.ir_mutation_attempt_epoch().unwrap();
        let mut queries = 0;
        assert_eq!(
            collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                census,
                unlimited(),
                |_| {
                    queries += 1;
                    None
                }
            )
            .unwrap_err(),
            race_resource_overflow_v1()
        );
        assert_eq!(queries, 1);
        let entered = Cell::new(false);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                census,
                unlimited(),
                |_| {
                    entered.set(true);
                    panic!("name census query panic")
                },
            );
        }))
        .unwrap_err();
        assert!(entered.get());
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"name census query panic")
        );
        assert_eq!(context.ir_mutation_attempt_epoch().unwrap(), epoch);
        let retry = collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            census,
            unlimited(),
            |value| value.unique_name_byte_len(&context),
        )
        .unwrap();
        assert_eq!(retry.name_storage, 38);
    }

    #[test]
    fn race_name_formula_uses_explicit_names_not_unrelated_identifier_bytes() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..Default::default()
        };
        let names = RaceNameCensusV1 {
            name_storage: 91,
            scan_work: 123,
            execution_lookup_work: 57,
        };
        let base =
            calculate_race_resource_upper_bound_for_shape_v1(census, names, Some((2, 1)), None)
                .unwrap();
        let changed = calculate_race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                identifier_bytes: usize::MAX,
                ..census
            },
            names,
            Some((2, 1)),
            None,
        )
        .unwrap();
        assert_eq!(base.bound, changed.bound);
        let no_lookup = calculate_race_resource_upper_bound_for_shape_v1(
            census,
            RaceNameCensusV1 {
                execution_lookup_work: 0,
                ..names
            },
            Some((2, 1)),
            None,
        )
        .unwrap();
        assert_eq!(
            base.bound.work_upper_bound() - no_lookup.bound.work_upper_bound(),
            2 * 57
        );
        for invalid in [
            RaceNameCensusV1 {
                name_storage: usize::MAX,
                ..names
            },
            RaceNameCensusV1 {
                scan_work: usize::MAX,
                ..names
            },
            RaceNameCensusV1 {
                execution_lookup_work: usize::MAX,
                ..names
            },
        ] {
            assert!(
                matches!(calculate_race_resource_upper_bound_for_shape_v1(census, invalid, Some((2,1)), None), Err(error) if error == race_resource_overflow_v1())
            );
        }
        assert_eq!(MAX_PLIRON_RACE_INVOCATIONS_V1, 65_536);
        assert_eq!(MAX_PLIRON_RACE_EFFECT_INSTANCES_V1, 1_048_576);
        assert_eq!(MAX_PLIRON_RACE_FINDINGS_V1, 4_096);
    }

    #[test]
    fn race_name_census_excludes_the_actual_checked_success_operand() {
        let mut context = context();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "checked_name_population".try_into().unwrap(),
            function_type,
        );
        let entry = function.get_entry_block(&context);
        let invocation = InvocationIndexOp::new(&mut context, 0, 64);
        let zero = IndexConstantOp::new(&mut context, 0);
        let one = IndexConstantOp::new(&mut context, 1);
        let extent = IndexConstantOp::new(&mut context, 4_096);
        let invocation_value = invocation.result(&context);
        let zero_value = zero.result(&context);
        let one_value = one.result(&context);
        let extent_value = extent.result(&context);
        let view_type = RankedViewType::new(&mut context, 32, true, vec![DYNAMIC_EXTENT]).unwrap();
        let view = RankedViewOp::new_in_space(
            &mut context,
            view_type,
            vec![extent_value],
            MemorySpaceAttr::Global,
        )
        .unwrap();
        let checked = CheckedRowStripedIndex2DOp::new_predicated(
            &mut context,
            invocation_value,
            zero_value,
            one_value,
            extent_value,
            extent_value,
            extent_value,
            [64, 64],
        );
        let index = checked.result(&context);
        let success = checked.success(&context).unwrap();
        let view_value = view.result(&context);
        let access = RankedAccessOp::new_predicated(
            &mut context,
            AccessKindAttr::Read,
            view_value,
            index,
            success,
        )
        .unwrap();
        let ret = ReturnOp::new(&mut context);
        for operation in [
            invocation.get_operation(),
            zero.get_operation(),
            one.get_operation(),
            extent.get_operation(),
            view.get_operation(),
            checked.get_operation(),
            access.get_operation(),
            ret.get_operation(),
        ] {
            operation.insert_at_back(entry, &context);
        }
        rename(&context, view.result(&context), &"v".repeat(95));
        rename(&context, index, &"i".repeat(129));
        rename(&context, success, &"s".repeat(65_536));
        assert_eq!(access.get_operation().deref(&context).get_num_operands(), 3);
        assert_eq!(access.checked_success(&context), Some(success));
        assert_eq!(access.indices(&context), [index]);
        let census = census(&context, &function);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let mut queried = Vec::new();
        let names = collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            census,
            unlimited(),
            |value| {
                queried.push(value);
                value.unique_name_byte_len(&context)
            },
        )
        .unwrap();
        assert_eq!(queried, [view.result(&context), index]);
        assert_eq!(
            names.name_storage,
            index.unique_name(&context).as_ref().len()
        );
        assert!(success.unique_name_byte_len(&context).unwrap() > names.name_storage);
    }

    #[test]
    fn race_name_census_bounds_real_retained_view_and_unresolved_index_payloads() {
        for unresolved in [false, true] {
            let (report, expected_view, expected_index, maximum) = {
                let mut context = context();
                let function_type = FunctionType::get(&context, vec![], vec![]);
                let function = FuncOp::new(
                    &mut context,
                    "retained_name_population".try_into().unwrap(),
                    function_type,
                );
                let entry = function.get_entry_block(&context);
                let access_block = pliron::basic_block::BasicBlock::new(
                    &mut context,
                    Some("access".try_into().unwrap()),
                    vec![],
                );
                let exit = pliron::basic_block::BasicBlock::new(
                    &mut context,
                    Some("exit".try_into().unwrap()),
                    vec![],
                );
                access_block.insert_at_back(function.get_region(&context), &context);
                exit.insert_at_back(function.get_region(&context), &context);
                let view_type = RankedViewType::new(&mut context, 32, true, vec![1]).unwrap();
                let view = RankedViewOp::new_in_space(
                    &mut context,
                    view_type,
                    vec![],
                    MemorySpaceAttr::Global,
                )
                .unwrap();
                let invocation = InvocationIndexOp::new(&mut context, 0, 2);
                let (index, index_operation) = if unresolved {
                    let index = dialect_kernel::IndexUnknownOp::new(&mut context);
                    (index.result(&context), index.get_operation())
                } else {
                    let index = IndexConstantOp::new(&mut context, 0);
                    (index.result(&context), index.get_operation())
                };
                let extent = IndexConstantOp::new(&mut context, 1);
                let extent_value = extent.result(&context);
                let view_value = view.result(&context);
                let guard = IndexLessThanBranchOp::new(
                    &mut context,
                    index,
                    extent_value,
                    access_block,
                    exit,
                );
                let write = RankedAccessOp::new(
                    &mut context,
                    AccessKindAttr::Write,
                    view_value,
                    vec![index],
                )
                .unwrap();
                let to_exit = dialect_kernel::BranchOp::new(&mut context, exit);
                let ret = ReturnOp::new(&mut context);
                for operation in [
                    view.get_operation(),
                    invocation.get_operation(),
                    index_operation,
                    extent.get_operation(),
                    guard.get_operation(),
                ] {
                    operation.insert_at_back(entry, &context);
                }
                write.get_operation().insert_at_back(access_block, &context);
                to_exit
                    .get_operation()
                    .insert_at_back(access_block, &context);
                ret.get_operation().insert_at_back(exit, &context);
                rename(&context, view.result(&context), &"v".repeat(8_192));
                rename(&context, index, &"i".repeat(16_384));
                let census = census(&context, &function);
                let inventory =
                    BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
                let names = collect_race_name_census_v1(
                    &context,
                    &function,
                    &inventory,
                    census,
                    unlimited(),
                    |value| value.unique_name_byte_len(&context),
                )
                .unwrap();
                let expected_view = view.result(&context).unique_name(&context).to_string();
                let expected_index = index.unique_name(&context).to_string();
                assert_eq!(names.name_storage, expected_index.len());
                // This entrypoint runs the real bounds prerequisite before race.
                let report = run_pliron_ranked_race_check_v1(&context, &function);
                (report, expected_view, expected_index, names.name_storage)
            };
            // The report owns its diagnostic after the original Context is gone.
            if unresolved {
                assert_eq!(report.status(), KernelCheckStatusV1::Incomplete);
                let [
                    RankedRaceFindingV1::UnresolvedIndex {
                        value, dimension, ..
                    },
                ] = report.findings()
                else {
                    panic!(
                        "expected actual unresolved-index report: {:?}",
                        report.findings()
                    );
                };
                assert_eq!(*dimension, 0);
                assert_eq!(value, &expected_index);
                assert_eq!(value.len(), maximum);
            } else {
                assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
                let [
                    RankedRaceFindingV1::ConflictingEffects {
                        view,
                        indices,
                        first,
                        second,
                    },
                ] = report.findings()
                else {
                    panic!(
                        "expected actual conflicting-effects report: {:?}",
                        report.findings()
                    );
                };
                assert_eq!(view, &expected_view);
                assert!(view.len() <= maximum);
                assert_eq!(indices, &[0]);
                assert_eq!(first.invocation(), &[0]);
                assert_eq!(second.invocation(), &[1]);
            }
        }
    }

    #[test]
    fn race_name_census_allocation_only_uses_the_full_u64_origin_bound() {
        let mut context = context();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "allocation_name_population".try_into().unwrap(),
            function_type,
        );
        let entry = function.get_entry_block(&context);
        let effect = AllocationEffectOp::new(
            &mut context,
            AccessKindAttr::Read,
            MemorySpaceAttr::Global,
            u64::MAX,
            1,
        )
        .unwrap();
        let ret = ReturnOp::new(&mut context);
        effect.get_operation().insert_at_back(entry, &context);
        ret.get_operation().insert_at_back(entry, &context);
        let actual = census(&context, &function);
        assert_eq!(actual.ranked_accesses, 0);
        assert_eq!(actual.allocation_effects, 1);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        let names = collect_race_name_census_v1(
            &context,
            &function,
            &inventory,
            actual,
            unlimited(),
            |_| panic!("allocation-only census queried an SSA name"),
        )
        .unwrap();
        assert_eq!(
            names.name_storage,
            format!("allocation origin {}", u64::MAX).len()
        );
        assert_eq!(names.name_storage, 38);
        let mut missing_effect = actual;
        missing_effect.allocation_effects = 0;
        assert_eq!(
            collect_race_name_census_v1(
                &context,
                &function,
                &inventory,
                missing_effect,
                unlimited(),
                |_| { panic!("allocation-only deficit queried an SSA name") }
            )
            .unwrap_err(),
            race_name_census_error_v1()
        );
        assert!(run_pliron_ranked_race_check_v1(&context, &function).is_clean());
    }

    #[test]
    fn race_name_scan_arithmetic_overflow_refuses_before_query() {
        let mut context = context();
        let (function, _, _) = fixture(&mut context, "name_overflow", 1, 1);
        let actual = census(&context, &function);
        let inventory = BoundedPlironFunctionInventoryV1::collect(&context, &function).unwrap();
        for supplied in [
            ProductionAnalysisInputCensusV1 {
                blocks: usize::MAX,
                ..actual
            },
            ProductionAnalysisInputCensusV1 {
                operations: usize::MAX,
                ..actual
            },
            ProductionAnalysisInputCensusV1 {
                operands: usize::MAX,
                ..actual
            },
            ProductionAnalysisInputCensusV1 {
                attributes: usize::MAX,
                ..actual
            },
            ProductionAnalysisInputCensusV1 {
                max_operation_arity: usize::MAX,
                ..actual
            },
        ] {
            assert_eq!(
                collect_race_name_census_v1(
                    &context,
                    &function,
                    &inventory,
                    supplied,
                    unlimited(),
                    |_| { panic!("overflow reached query") }
                )
                .unwrap_err(),
                race_resource_overflow_v1()
            );
        }
    }
}

#[cfg(test)]
mod status_tests {
    use super::*;

    fn witness(
        access: AccessKindAttr,
        atomic_scope: Option<AtomicScopeAttr>,
    ) -> RankedRaceWitnessV1 {
        RankedRaceWitnessV1 {
            location: RankedRaceLocationV1 {
                block: 0,
                operation: 0,
            },
            access,
            invocation: vec![0],
            grid: 0,
            workgroup: Some(0),
            subgroup: Some(0),
            lane: Some(0),
            atomic_scope,
        }
    }

    fn conflict() -> RankedRaceFindingV1 {
        RankedRaceFindingV1::ConflictingEffects {
            view: "v0".to_owned(),
            indices: vec![0],
            first: witness(AccessKindAttr::Write, None),
            second: witness(AccessKindAttr::Read, None),
        }
    }

    #[test]
    fn every_race_finding_has_the_shared_status() {
        let incomplete = [
            RankedRaceFindingV1::BoundsPrerequisiteRejected,
            RankedRaceFindingV1::SparseIndexAnalysisFailed {
                detail: "unresolved".to_owned(),
            },
            RankedRaceFindingV1::DynamicLaunchExtent { dimension: 0 },
            RankedRaceFindingV1::LaunchDomainTooLarge {
                invocations: 2,
                limit: 1,
            },
            RankedRaceFindingV1::UnresolvedIndex {
                block: 0,
                operation: 0,
                dimension: 0,
                value: "i".to_owned(),
            },
            RankedRaceFindingV1::EffectInstanceLimitExceeded {
                actual: 2,
                limit: 1,
            },
            RankedRaceFindingV1::FindingLimitExceeded {
                actual: 2,
                limit: 1,
            },
            RankedRaceFindingV1::ExecutionLayoutUnavailable {
                detail: "missing".to_owned(),
            },
            RankedRaceFindingV1::AllocationContractUnavailable {
                detail: "missing".to_owned(),
            },
            RankedRaceFindingV1::HappensBeforeIncomplete {
                view: "v0".to_owned(),
                detail: "missing".to_owned(),
            },
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }

        let rejected = [
            conflict(),
            RankedRaceFindingV1::InsufficientAtomicScope {
                view: "v0".to_owned(),
                indices: vec![0],
                first: witness(
                    AccessKindAttr::AtomicWrite,
                    Some(AtomicScopeAttr::Workgroup),
                ),
                second: witness(AccessKindAttr::AtomicRead, Some(AtomicScopeAttr::Workgroup)),
            },
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }
    }

    #[test]
    fn rejected_race_finding_dominates_an_incomplete_finding() {
        let report = RankedRaceReportV1 {
            findings: vec![RankedRaceFindingV1::BoundsPrerequisiteRejected, conflict()],
        };
        assert_eq!(report.status(), KernelCheckStatusV1::Rejected);
        assert!(!report.is_clean());
        assert_eq!(clean().status(), KernelCheckStatusV1::Clean);
    }

    #[test]
    fn effect_pair_inventory_is_charged_before_enumeration() {
        assert!(effect_pair_inventory_fits_budget(1_447));
        assert!(!effect_pair_inventory_fits_budget(1_448));
        assert!(!effect_pair_inventory_fits_budget(usize::MAX));
    }

    #[test]
    fn race_resource_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // One effect and two invocations make 24 raw-evaluator queries. Each
        // query visits one definition: 24*8 stack/map operations plus 24*3
        // query lifecycles. The single-query peak has six map-capacity units,
        // three eight-unit frames, and eight invocation decode items.
        // The single symbolic pair also prepays twelve root queries and
        // three comparisons: 12*4+3=51. These queries allocate nothing.
        // Two possible name constructions each pay 4*64+64 work; the scan's
        // sixteen logical slots and two 64-byte copies add to temporary space.
        const EXACT_WORK: usize = 3_707;
        const EXACT_RETAINED: usize = 1_272;
        const EXACT_PEAK: usize = 68_646;
        let exact = race_resource_upper_bound_for_shape_v1(
            census,
            component_race_names_v1(),
            Some((2, 1)),
            None,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                Some((2, 1)),
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                Some((2, 1)),
                None,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn race_resource_bound_uses_effect_census_and_ordered_finding_pairs() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 4_096,
            ranked_accesses: 2,
            allocation_effects: 20,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let bound = race_resource_upper_bound_for_shape_v1(
            census,
            component_race_names_v1(),
            Some((64, 1)),
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 4_000_000),
        )
        .unwrap();
        // Witness coordinates have rank eight, names reserve 64 units, and
        // each finding also reserves 1024 diagnostic bytes and 160 fields.
        const PER_FINDING: usize = 3 * 8 + 64 + 1_024 + 160;
        assert_eq!(bound.retained_storage_upper_bound(), 22 * 22 * PER_FINDING);
        let scalar_only = race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: 0,
                allocation_effects: 0,
                ..census
            },
            component_race_names_v1(),
            Some((64, 1)),
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, 4_000_000),
        )
        .unwrap();
        assert_eq!(scalar_only.retained_storage_upper_bound(), PER_FINDING);
        assert!(bound.peak_storage_upper_bound() > scalar_only.peak_storage_upper_bound());
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                Some((64, 1)),
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound()
                ),
            )
            .is_ok()
        );
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                Some((64, 1)),
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn race_early_paths_charge_one_finding_and_its_construction() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        for (shape, work) in [
            (None, 1_051_843),
            (Some((0, 1)), 1_051_843),
            (Some((1, 1)), 3_347),
        ] {
            const RETAINED: usize = 1_272;
            // Effect collection (88), four signal/class sets (8), one retained
            // diagnostic and one construction temporary. No exact map/query.
            const PEAK: usize = 88 + 8 + 2 * RETAINED + 2 * 64 + 16;
            let bound = race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                shape,
                None,
                ProductionAnalysisResourceLimitsV1::new(work, PEAK),
            )
            .unwrap();
            assert_eq!(bound.work_upper_bound(), work);
            assert_eq!(bound.retained_storage_upper_bound(), RETAINED);
            assert_eq!(bound.peak_storage_upper_bound(), PEAK);
            for limits in [
                ProductionAnalysisResourceLimitsV1::new(work - 1, PEAK),
                ProductionAnalysisResourceLimitsV1::new(work, PEAK - 1),
            ] {
                assert!(
                    race_resource_upper_bound_for_shape_v1(
                        census,
                        component_race_names_v1(),
                        shape,
                        None,
                        limits
                    )
                    .is_err()
                );
            }
        }
    }

    #[test]
    fn race_unreachable_fallback_does_not_precharge_ordered_finding_vectors() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 4_096,
            blocks: 385,
            ranked_accesses: 64,
            identifier_bytes: 32_768,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let names = RaceNameCensusV1 {
            name_storage: 32_832,
            ..component_race_names_v1()
        };
        let bound =
            race_resource_upper_bound_for_shape_v1(census, names, None, None, limits).unwrap();
        const PER_FINDING: usize = 3 * 8 + 32_768 + 64 + 1_024 + 160;
        assert_eq!(bound.retained_storage_upper_bound(), PER_FINDING);
        assert_eq!(
            bound.peak_storage_upper_bound(),
            64 * (8 + 16 + 32_768 + 64) + 385 * 11 + 64 * 8 + 2 * PER_FINDING + 2 * 32_832 + 16
        );
        assert!(
            race_resource_upper_bound_for_shape_v1(census, names, Some((2, 1)), None, limits)
                .is_err()
        );
    }

    #[test]
    fn race_relation_shape_matches_domain_and_minimum_pair_work_gates() {
        for extents in [
            &[][..],
            &[0],
            &[1],
            &[65_536],
            &[77_791_232],
            &[u64::MAX, 2],
            &[u64::MAX, u64::MAX, u64::MAX],
        ] {
            assert_eq!(
                presburger_invocation_shape_for_resource_v1(extents),
                None,
                "{extents:?}"
            );
        }
        for (extents, expected) in [
            (&[65_537][..], (65_537, 1)),
            (&[262_144][..], (262_144, 1)),
            (&[131_072, 1, 1][..], (131_072, 3)),
        ] {
            assert_eq!(
                presburger_invocation_shape_for_resource_v1(extents),
                Some(expected)
            );
        }
        for extents in [&[262_145][..], &[131_073, 1, 1][..]] {
            assert_eq!(presburger_invocation_shape_for_resource_v1(extents), None);
        }
    }

    #[test]
    fn race_relation_maps_are_temporary_and_pairs_bound_four_walks() {
        let shape = Some((65_537, 1));
        let (work, storage) = presburger_relation_resource_upper_bound_v1(shape, 1).unwrap();
        assert_eq!(work, 65_537 * (16 * 8 * 8 + 128 * 8 + 256));
        assert_eq!(storage, 65_537 * (9 * 8 + 64) + 8 * 8 * 8 + 64 * 8 + 128);
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(shape, 0),
            Ok((0, 0))
        );
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(shape, usize::MAX),
            Ok((work * 3, storage))
        );
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(Some((usize::MAX, 1)), 1),
            Err(race_resource_overflow_v1())
        );
        assert_eq!(
            presburger_relation_resource_upper_bound_v1(Some((1, usize::MAX)), 1),
            Err(race_resource_overflow_v1())
        );
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            ranked_accesses: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let bound = race_resource_upper_bound_for_shape_v1(
            census,
            component_race_names_v1(),
            None,
            shape,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        assert_eq!(bound.retained_storage_upper_bound(), 1_272);
        assert_eq!(bound.work_upper_bound(), 1_051_843 + work);
        assert_eq!(bound.peak_storage_upper_bound(), 2_784 + storage);
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                None,
                shape,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Ok(bound)
        );
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                None,
                shape,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound() - 1,
                    bound.peak_storage_upper_bound(),
                ),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                resource: "work upper bound",
            })
        );
        assert!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                None,
                shape,
                ProductionAnalysisResourceLimitsV1::new(
                    bound.work_upper_bound(),
                    bound.peak_storage_upper_bound() - 1
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn race_remainder_relation_enumerates_beyond_exact_trace_limit() {
        use dialect_kernel::{IndexBinaryKindAttr, IndexBinaryOp, RankedViewType, ReturnOp};
        use pliron::{builtin::types::FunctionType, dialect::DialectName, op::Op};

        let context = &mut Context::new();
        dialect_kernel::register_dialect(
            context,
            &DialectName::try_new(dialect_kernel::DIALECT_NAME).unwrap(),
        )
        .unwrap();
        dialect_gpu::register_dialect(context).unwrap();
        let function_type = FunctionType::get(context, vec![], vec![]);
        let function = FuncOp::new(
            context,
            "remainder_relation_resource".try_into().unwrap(),
            function_type,
        );
        let entry = function.get_entry_block(context);
        let invocation = InvocationIndexOp::new(context, 0, 65_537);
        let modulus = IndexConstantOp::new(context, 65_537);
        let index = IndexBinaryOp::new(
            context,
            IndexBinaryKindAttr::Remainder,
            invocation.result(context),
            modulus.result(context),
        );
        let memory_type = RankedViewType::new(context, 32, true, vec![65_537]).unwrap();
        let memory =
            RankedViewOp::new_in_space(context, memory_type, vec![], MemorySpaceAttr::Global)
                .unwrap();
        let write = RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            memory.result(context),
            vec![index.result(context)],
        )
        .unwrap();
        let ret = ReturnOp::new(context);
        for operation in [
            invocation.get_operation(),
            modulus.get_operation(),
            index.get_operation(),
            memory.get_operation(),
            write.get_operation(),
            ret.get_operation(),
        ] {
            operation.insert_at_back(entry, context);
        }
        let mut analyses = PlironAnalysisManagerV1::new(&function);
        analyses.prepare_sparse_indices(context, &function);
        let sparse = analyses.sparse_indices().unwrap();
        assert_eq!(static_invocation_shape_for_resource_v1(sparse, None), None);
        assert!(sparse.fact(index.result(context)).affine().is_none());
        assert_eq!(
            presburger_invocation_shape_for_resource_v1(sparse.launch_extents()),
            Some((65_537, 1))
        );
        let report = run_pliron_ranked_race_check_v1(context, &function);
        assert!(report.is_clean(), "{:#?}", report.findings());
    }

    #[test]
    fn race_resource_bound_rejects_effect_census_sum_overflow() {
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                ProductionAnalysisInputCensusV1 {
                    ranked_accesses: usize::MAX,
                    allocation_effects: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                component_race_names_v1(),
                Some((1, 1)),
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(race_resource_overflow_v1())
        );
    }

    #[test]
    fn raw_index_evaluator_bound_covers_a_deep_definition_dag() {
        const OPERATIONS: usize = 20_001;
        let (work, temporary) =
            raw_index_evaluation_resource_upper_bound_v1(OPERATIONS, 1, 3).unwrap();
        assert_eq!(work, OPERATIONS * 8 + 3);
        assert_eq!(temporary, OPERATIONS * 6 + (2 * OPERATIONS + 1) * 8 + 3);

        let capped = raw_index_evaluation_resource_upper_bound_v1(
            OPERATIONS,
            MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1,
            3,
        )
        .unwrap();
        assert_eq!(
            capped.0,
            (MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 + 1) * 8
                + MAX_BOUNDS_WITNESS_EVALUATION_STEPS_V1 * 3
        );
    }

    #[test]
    fn race_resource_bound_rejects_pair_overflow() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: usize::MAX,
            ranked_accesses: usize::MAX,
            ..ProductionAnalysisInputCensusV1::default()
        };
        assert_eq!(
            race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                Some((1, 1)),
                None,
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(race_resource_overflow_v1())
        );
    }
}

#[cfg(test)]
mod numeric_preflight_tests {
    use super::*;
    use std::{collections::BTreeMap, io};

    fn small_numbers() -> RaceResourcePreflightNumbersV1 {
        calculate_race_resource_upper_bound_for_shape_v1(
            ProductionAnalysisInputCensusV1 {
                operations: 1,
                ranked_accesses: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            component_race_names_v1(),
            Some((2, 1)),
            None,
        )
        .unwrap()
    }

    fn fields(bytes: &[u8]) -> BTreeMap<&str, usize> {
        let text = std::str::from_utf8(bytes).unwrap();
        assert_eq!(text.lines().count(), 1);
        assert!(text.ends_with('\n'));
        let mut words = text.split_whitespace();
        assert_eq!(words.next(), Some("RACE_RESOURCE_PREFLIGHT_V1"));
        assert_eq!(words.next(), Some("stage=local_require"));
        assert_eq!(words.next(), Some("units=logical"));
        let mut result = BTreeMap::new();
        for word in words {
            let (key, value) = word.split_once('=').unwrap();
            assert!(result.insert(key, value.parse().unwrap()).is_none());
        }
        assert_eq!(result.len(), 40);
        result
    }

    #[test]
    fn race_numeric_preflight_exposes_existing_small_calculation_terms() {
        let numbers = small_numbers();
        let per_finding = 3 * 8 + 64 + 1_024 + 160;
        assert_eq!(numbers.census.operations, 1);
        assert_eq!(numbers.effects, 1);
        assert_eq!(numbers.effect_pairs, 1);
        assert_eq!(numbers.pairs, 1);
        assert_eq!(numbers.rank, 8);
        assert_eq!(numbers.potential_effect_instances, 2);
        assert_eq!(numbers.charged_effect_instances, 2);
        assert_eq!(numbers.retained_effect_instances, 2);
        assert_eq!(numbers.retained_finding_count, 1);
        assert_eq!(numbers.name_storage, 64);
        assert_eq!(numbers.per_finding_storage, per_finding);
        assert_eq!(numbers.raw_evaluation_work, 24 * 8 + 24 * 3);
        assert_eq!(numbers.raw_evaluation_temporary, 6 + 3 * 8 + 8);
        assert_eq!(numbers.symbolic_work, 8 * 8 + 16);
        assert_eq!(numbers.presburger_work, 0);
        assert_eq!(numbers.presburger_temporary, 0);
        assert_eq!(
            numbers.work,
            32 + 80 + 51 + 2 * 48 + 264 + 2 * (4 * 64 + 64)
        );
        assert_eq!(numbers.effect_state, 8 + 16 + 64);
        assert_eq!(numbers.address_state, 2 * (8 * 9 + 64));
        assert_eq!(numbers.attempted_finding, per_finding);
        assert_eq!(numbers.conflict_class_storage, 4_097 * 16);
        assert_eq!(
            numbers.temporary,
            numbers.effect_state + 272 + per_finding + 4_097 * 16 + 8 + 38 + 2 * 64 + 16
        );
        assert_eq!(
            numbers.bound.work_upper_bound(),
            numbers.work + 2 * per_finding
        );
        assert_eq!(numbers.bound.retained_storage_upper_bound(), per_finding);
        assert_eq!(
            numbers.bound.peak_storage_upper_bound(),
            per_finding + numbers.temporary
        );

        let mut output = Vec::new();
        let limits = ProductionAnalysisResourceLimitsV1::new(19, 23);
        write_race_resource_preflight_v1(true, &mut output, &numbers, limits);
        let fields = fields(&output);
        let core_work = 32 + 80 + 51 + 2 * 48 + 264 + 2 * (4 * 64 + 64);
        let temporary = (8 + 16 + 64)
            + 2 * (8 * 9 + 64)
            + per_finding
            + 4_097 * 16
            + 8
            + (6 + 3 * 8 + 8)
            + 2 * 64
            + 16;
        assert_eq!(
            fields,
            BTreeMap::from([
                ("blocks", 0),
                ("operations", 1),
                ("successors", 0),
                ("ranked_accesses", 1),
                ("allocation_effects", 0),
                ("identifier_bytes", 0),
                ("canonical_bytes", 0),
                ("static_present", 1),
                ("static_invocations", 2),
                ("static_rank", 1),
                ("presburger_present", 0),
                ("presburger_invocations", 0),
                ("presburger_rank", 0),
                ("effects", 1),
                ("effect_pairs", 1),
                ("pairs", 1),
                ("rank", 8),
                ("potential_instances", 2),
                ("charged_instances", 2),
                ("retained_instances", 2),
                ("finding_count", 1),
                ("name_storage", 64),
                ("per_finding_storage", per_finding),
                ("core_work", core_work),
                ("raw_work", 24 * 8 + 24 * 3),
                ("presburger_work", 0),
                ("symbolic_work", 8 * 8 + 16),
                ("effect_state", 8 + 16 + 64),
                ("address_state", 2 * (8 * 9 + 64)),
                ("attempted_finding", per_finding),
                ("conflict_class_storage", 4_097 * 16),
                ("raw_temporary", 6 + 3 * 8 + 8),
                ("presburger_temporary", 0),
                ("temporary", temporary),
                ("work", core_work + 2 * per_finding),
                ("retained", per_finding),
                ("peak", per_finding + temporary),
                ("remaining_work", 19),
                ("remaining_peak", 23),
                ("word_bits", usize::BITS as usize),
            ])
        );
        assert_eq!(fields["core_work"], numbers.work);
        assert_eq!(fields["work"], numbers.bound.work_upper_bound());
        assert_eq!(fields["retained"], per_finding);
        assert_eq!(fields["peak"], per_finding + numbers.temporary);
        assert_eq!(fields["remaining_work"], 19);
        assert_eq!(fields["remaining_peak"], 23);
        assert_eq!(fields["word_bits"], usize::BITS as usize);
    }

    #[test]
    fn race_numeric_preflight_observes_exact_limits_before_unchanged_admission() {
        let numbers = small_numbers();
        let work = numbers.bound.work_upper_bound();
        let peak = numbers.bound.peak_storage_upper_bound();
        for (work_limit, peak_limit, expected_resource) in [
            (work, peak, None),
            (work - 1, peak, Some("work upper bound")),
            (work, peak - 1, Some("peak storage upper bound")),
            (work - 1, peak - 1, Some("work upper bound")),
        ] {
            let limits = ProductionAnalysisResourceLimitsV1::new(work_limit, peak_limit);
            let mut calls = 0;
            let mut output = Vec::new();
            let result = finish_race_resource_preflight_v1(Ok(numbers), limits, |seen, actual| {
                calls += 1;
                assert_eq!(actual, limits);
                assert_eq!(seen.bound, numbers.bound);
                write_race_resource_preflight_v1(true, &mut output, seen, actual);
            });
            assert_eq!(calls, 1);
            let fields = fields(&output);
            assert_eq!(fields["remaining_work"], work_limit);
            assert_eq!(fields["remaining_peak"], peak_limit);
            assert_eq!(fields["work"], work);
            assert_eq!(fields["peak"], peak);
            match expected_resource {
                None => assert_eq!(result, Ok(numbers.bound)),
                Some(resource) => assert_eq!(
                    result,
                    Err(ProductionAnalysisResourceLimitV1 {
                        phase: ProductionAnalysisResourcePhaseV1::RaceFreedom,
                        resource,
                    })
                ),
            }
        }
    }

    struct RefusingWriter {
        writes: usize,
    }

    impl io::Write for RefusingWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            self.writes += 1;
            Err(io::Error::other("component diagnostic sink failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            panic!("numeric diagnostic must not flush the writer")
        }
    }

    struct UntouchedWriter;

    impl io::Write for UntouchedWriter {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            panic!("disabled numeric diagnostic touched the writer")
        }

        fn flush(&mut self) -> io::Result<()> {
            panic!("disabled numeric diagnostic flushed the writer")
        }
    }

    #[test]
    fn race_numeric_preflight_writer_failure_and_disabled_output_preserve_results() {
        let numbers = small_numbers();
        for limits in [
            ProductionAnalysisResourceLimitsV1::new(
                numbers.bound.work_upper_bound(),
                numbers.bound.peak_storage_upper_bound(),
            ),
            ProductionAnalysisResourceLimitsV1::new(0, 0),
            ProductionAnalysisResourceLimitsV1::new(numbers.bound.work_upper_bound(), 0),
        ] {
            let expected = limits.require(
                ProductionAnalysisResourcePhaseV1::RaceFreedom,
                numbers.bound,
            );
            let mut writer = RefusingWriter { writes: 0 };
            let result = finish_race_resource_preflight_v1(Ok(numbers), limits, |seen, actual| {
                write_race_resource_preflight_v1(true, &mut writer, seen, actual);
            });
            assert_eq!(writer.writes, 1);
            assert_eq!(result, expected);
            assert_eq!(
                finish_race_resource_preflight_v1(Ok(numbers), limits, |seen, actual| {
                    write_race_resource_preflight_v1(false, &mut UntouchedWriter, seen, actual);
                }),
                expected
            );
        }
    }

    #[test]
    fn race_numeric_preflight_shapes_and_whole_effect_census_stay_distinct() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 3,
            blocks: 2,
            ranked_accesses: 1,
            allocation_effects: 2,
            identifier_bytes: 31,
            canonical_bytes: 47,
            ..ProductionAnalysisInputCensusV1::default()
        };
        for shape in [None, Some((0, 1)), Some((1, 1)), Some((2, 1))] {
            let numbers = calculate_race_resource_upper_bound_for_shape_v1(
                census,
                component_race_names_v1(),
                shape,
                Some((65_537, 1)),
            )
            .unwrap();
            let mut output = Vec::new();
            write_race_resource_preflight_v1(
                true,
                &mut output,
                &numbers,
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            );
            let fields = fields(&output);
            assert_eq!(fields["static_present"], usize::from(shape.is_some()));
            assert_eq!(
                fields["static_invocations"],
                shape.map_or(0, |value| value.0)
            );
            assert_eq!(fields["static_rank"], shape.map_or(0, |value| value.1));
            assert_eq!(fields["presburger_present"], 1);
            assert_eq!(fields["presburger_invocations"], 65_537);
            assert_eq!(fields["presburger_rank"], 1);
            assert_eq!(fields["ranked_accesses"], 1);
            assert_eq!(fields["allocation_effects"], 2);
            assert_eq!(fields["effects"], 3);
            assert_eq!(fields["effect_pairs"], 6);
            assert_eq!(fields["identifier_bytes"], 31);
            assert_eq!(fields["canonical_bytes"], 47);
        }
    }

    #[test]
    fn race_numeric_preflight_overflow_has_no_complete_observation() {
        for census in [
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: usize::MAX,
                allocation_effects: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            ProductionAnalysisInputCensusV1 {
                ranked_accesses: usize::MAX,
                ..ProductionAnalysisInputCensusV1::default()
            },
            ProductionAnalysisInputCensusV1 {
                operations: usize::MAX,
                ..ProductionAnalysisInputCensusV1::default()
            },
        ] {
            let result = finish_race_resource_preflight_v1(
                calculate_race_resource_upper_bound_for_shape_v1(
                    census,
                    component_race_names_v1(),
                    Some((1, 1)),
                    None,
                ),
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
                |_, _| panic!("overflow emitted a complete numeric record"),
            );
            assert_eq!(result, Err(race_resource_overflow_v1()));
        }
    }

    #[test]
    fn race_numeric_preflight_fixed_roster_has_bounded_maximum_width_output() {
        // Deliberately synthetic writer values, not an admitted bound or census.
        let maximum = usize::MAX;
        let numbers = RaceResourcePreflightNumbersV1 {
            census: ProductionAnalysisInputCensusV1 {
                blocks: maximum,
                operations: maximum,
                successors: maximum,
                ranked_accesses: maximum,
                allocation_effects: maximum,
                identifier_bytes: maximum,
                canonical_bytes: maximum,
                ..ProductionAnalysisInputCensusV1::default()
            },
            invocation_shape: Some((maximum, maximum)),
            presburger_shape: Some((maximum, maximum)),
            effects: maximum,
            effect_pairs: maximum,
            pairs: maximum,
            rank: maximum,
            potential_effect_instances: maximum,
            charged_effect_instances: maximum,
            retained_effect_instances: maximum,
            retained_finding_count: maximum,
            name_storage: maximum,
            per_finding_storage: maximum,
            work: maximum,
            raw_evaluation_work: maximum,
            presburger_work: maximum,
            symbolic_work: maximum,
            effect_state: maximum,
            address_state: maximum,
            attempted_finding: maximum,
            conflict_class_storage: maximum,
            raw_evaluation_temporary: maximum,
            presburger_temporary: maximum,
            temporary: maximum,
            bound: ProductionAnalysisResourceUpperBoundV1::checked_phase(
                ProductionAnalysisResourcePhaseV1::RaceFreedom,
                maximum,
                maximum,
                0,
            )
            .unwrap(),
        };
        let mut output = Vec::new();
        write_race_resource_preflight_v1(
            true,
            &mut output,
            &numbers,
            ProductionAnalysisResourceLimitsV1::new(maximum, maximum),
        );
        let fields = fields(&output);
        let numeric_bytes: usize = fields.values().map(|value| value.to_string().len()).sum();
        const FIXED_BYTES: usize = 659;
        assert_eq!(output.len() - numeric_bytes, FIXED_BYTES);
        assert!(output.len() <= FIXED_BYTES + 40 * usize::BITS as usize);
        assert_eq!(fields["remaining_work"], maximum);
        assert_eq!(fields["remaining_peak"], maximum);
        assert_eq!(fields["word_bits"], usize::BITS as usize);
    }
}

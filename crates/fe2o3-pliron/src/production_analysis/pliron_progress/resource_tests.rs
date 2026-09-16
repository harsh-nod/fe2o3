#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;
    use dialect_kernel::{DIALECT_NAME, IndexConstantOp, ReturnOp, register_dialect};
    use pliron::{builtin::types::FunctionType, dialect::DialectName};

    fn census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            blocks: 7,
            operations: 32,
            operands: 58,
            results: 24,
            successors: 11,
            block_arguments: 8,
            attributes: 15,
            type_nodes: 42,
            identifier_bytes: 190,
            canonical_bytes: 1_300,
            max_operation_arity: 8,
            max_successor_arity: 2,
            pipeline_creates: 0,
            pipeline_events: 0,
            ranked_accesses: 0,
            workgroup_ranked_accesses: 0,
            allocation_effects: 0,
            collective_transpose_candidates: 0,
            ownership_contracts: 0,
            effect_refinement_contracts: 0,
            index_lt_branch_candidates: 0,
            memory_bounds_guard_candidates: 0,
            semantic_definitions: 0,
            semantic_refinement_contracts: 0,
            native_switch_verification_work: 0,
            native_switch_verification_scratch: 0,
        }
    }

    #[test]
    fn progress_bound_accepts_exact_limits_and_rejects_one_under() {
        let census = census();
        let exact = preflight_progress_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(
            preflight_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Ok(exact)
        );
        assert!(
            preflight_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            )
            .is_err()
        );
        assert!(
            preflight_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn progress_bound_rejects_arithmetic_overflow_and_hard_caps() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        assert_eq!(
            preflight_progress_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    identifier_bytes: usize::MAX,
                    successors: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                unlimited,
            ),
            preflight_progress_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    successors: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                unlimited,
            )
        );
        assert_eq!(
            checked_progress_sum_v1(&[usize::MAX, 1], "test sum"),
            Err(progress_resource_error_v1("test sum"))
        );
        assert_eq!(
            checked_progress_product_v1(usize::MAX, 2, "test product"),
            Err(progress_resource_error_v1("test product"))
        );
        assert!(
            preflight_progress_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    blocks: MAX_PLIRON_PROGRESS_BLOCKS_V1 + 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                unlimited,
            )
            .is_err()
        );
    }

    #[test]
    fn late_same_block_use_has_an_exact_precharged_verifier_boundary() {
        const CONSTANTS: usize = 64;
        const OPERATIONS: usize = CONSTANTS + 2;
        const OPERANDS: usize = 2;
        const RESULTS: usize = CONSTANTS + 1;
        const ATTRIBUTES: usize = CONSTANTS + 3;

        let mut context = Context::new();
        register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        let function_type = FunctionType::get(&context, vec![], vec![]);
        let function = FuncOp::new(
            &mut context,
            "late_same_block_use".try_into().unwrap(),
            function_type,
        );
        let entry = function.get_entry_block(&context);
        let mut first_two = Vec::with_capacity(2);
        for value in 0..CONSTANTS {
            let constant = IndexConstantOp::new(&mut context, value as u64);
            if first_two.len() < 2 {
                first_two.push(constant.result(&context));
            }
            constant.get_operation().insert_at_back(entry, &context);
        }
        let late_use = IndexBinaryOp::new(
            &mut context,
            IndexBinaryKindAttr::Add,
            first_two[0],
            first_two[1],
        );
        late_use.get_operation().insert_at_back(entry, &context);
        ReturnOp::new(&mut context)
            .get_operation()
            .insert_at_back(entry, &context);
        verify_operation(function.get_operation(), &context).unwrap();

        let inventory = bounded_structural_inventory(&context, &function).unwrap();
        assert_eq!(inventory.regions, 1);
        assert_eq!(inventory.blocks, 1);
        assert_eq!(inventory.operations, OPERATIONS);
        assert_eq!(inventory.operands, OPERANDS);
        assert_eq!(inventory.results, RESULTS);
        assert_eq!(inventory.attributes, ATTRIBUTES);
        assert_eq!(inventory.block_arguments, 0);
        assert_eq!(inventory.edges, 0);

        // The final binary use is physically after all 64 definitions. The
        // runtime verifier precharge is the 202 structural visits plus the
        // defensible 2 operands * 66 operations same-block scan bound.
        assert_eq!(inventory.verification_work(), 334);

        let census = ProductionAnalysisInputCensusV1 {
            blocks: 1,
            operations: OPERATIONS,
            operands: OPERANDS,
            results: RESULTS,
            attributes: ATTRIBUTES,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // Preflight conservatively admits one possible region per operation
        // plus the function region: structural=268, verifier scan=132,
        // charged=412, dominators=2, and loop analysis=69.
        const EXACT_WORK: usize = 1_555;
        // One 1,040-item possible diagnostic, one retained block, and the
        // explicitly separated inventory/verifier/CFG temporary owners.
        const EXACT_PEAK_STORAGE: usize = 1_862;
        let exact = preflight_progress_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK_STORAGE),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK_STORAGE);
        assert!(
            preflight_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK_STORAGE),
            )
            .is_err()
        );
        assert!(
            preflight_progress_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK_STORAGE - 1),
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod resource_budget_tests {
    use super::*;

    #[test]
    fn work_budget_accepts_the_boundary_and_rejects_one_more_unit() {
        let mut budget = ProgressWorkBudgetV1::default();
        budget.charge(MAX_PLIRON_PROGRESS_WORK_UNITS_V1).unwrap();
        assert_eq!(
            budget.charge(1),
            Err(PlironProgressFindingV1::ResourceLimitExceeded {
                resource: "work units",
                actual: MAX_PLIRON_PROGRESS_WORK_UNITS_V1 + 1,
                limit: MAX_PLIRON_PROGRESS_WORK_UNITS_V1,
            })
        );
    }
}

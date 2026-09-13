#[cfg(test)]
mod resource_tests {
    use super::*;
    use crate::ProductionAnalysisResourceLimitV1;
    use dialect_kernel::{DIALECT_NAME, IndexConstantOp, ReturnOp, register_dialect};
    use pliron::{builtin::attributes::UnitAttr, dialect::DialectName, identifier::Identifier};

    include!("native_switch_resource_tests.rs");

    fn debug_named_function_v1(context: &mut Context) -> (FuncOp, Value) {
        register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        dialect_gpu::register_dialect(context).unwrap();
        dialect_proof::register_dialect(context).unwrap();
        let signature = FunctionType::get(context, vec![], vec![]);
        let function = FuncOp::new(
            context,
            Identifier::try_from("identity_debug_name").unwrap(),
            signature,
        );
        let entry = function.get_entry_block(context);
        let constant = IndexConstantOp::new(context, 7);
        let result = constant.result(context);
        constant.get_operation().insert_at_back(entry, context);
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(entry, context);
        (function, result)
    }

    fn nested_type_function_v1(context: &mut Context, depth: usize, width: usize) -> FuncOp {
        register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        dialect_gpu::register_dialect(context).unwrap();
        dialect_proof::register_dialect(context).unwrap();
        let unit: TypeHandle = UnitType::get(context).into();
        let mut nested = unit;
        for _ in 0..depth {
            let mut arguments = Vec::new();
            arguments.try_reserve_exact(width).unwrap();
            arguments.push(nested);
            arguments.extend(std::iter::repeat_n(unit, width.saturating_sub(1)));
            nested = FunctionType::get(context, arguments, vec![unit]).into();
        }
        let function = FuncOp::new(
            context,
            Identifier::try_from("identity_nested_type").unwrap(),
            FunctionType::get(context, vec![nested], vec![]),
        );
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(function.get_entry_block(context), context);
        function
    }

    fn capture_bound_v1(
        context: &Context,
        function: &FuncOp,
        limits: ProductionAnalysisResourceLimitsV1,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, IdentityCaptureFailureV1> {
        LivePlironStructuralIdentityProviderV1::new(context, function)
            .capture_with_resource_limits_v1(limits)
            .map(|capture| capture.resource_upper_bound)
    }

    fn assert_real_capture_exact_and_one_under_v1(context: &Context, function: &FuncOp) {
        let Ok(bound) = capture_bound_v1(
            context,
            function,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        ) else {
            panic!("hard-ceiling identity capture failed")
        };
        let work = bound.work_upper_bound();
        let storage = bound.peak_storage_upper_bound();
        let Ok(exact) = capture_bound_v1(
            context,
            function,
            ProductionAnalysisResourceLimitsV1::new(work, storage),
        ) else {
            panic!("exact identity capture limit rejected")
        };
        assert_eq!(exact, bound);
        assert!(matches!(
            capture_bound_v1(
                context,
                function,
                ProductionAnalysisResourceLimitsV1::new(work - 1, storage),
            ),
            Err(IdentityCaptureFailureV1::ResourceLimit(_))
        ));
        assert!(matches!(
            capture_bound_v1(
                context,
                function,
                ProductionAnalysisResourceLimitsV1::new(work, storage - 1),
            ),
            Err(IdentityCaptureFailureV1::ResourceLimit(_))
        ));
    }

    #[test]
    fn capture_bound_has_an_independent_exact_sum_and_one_under_rejects() {
        let census = ProductionAnalysisInputCensusV1 {
            blocks: 2,
            operations: 3,
            operands: 5,
            results: 7,
            successors: 11,
            block_arguments: 13,
            attributes: 17,
            type_nodes: 19,
            identifier_bytes: 23,
            canonical_bytes: 29,
            max_operation_arity: 12,
            max_successor_arity: 6,
            pipeline_creates: 1,
            pipeline_events: 2,
            ranked_accesses: 3,
            workgroup_ranked_accesses: 0,
            allocation_effects: 0,
            collective_transpose_candidates: 0,
            ownership_contracts: 0,
            effect_refinement_contracts: 0,
            index_lt_branch_candidates: 1,
            semantic_definitions: 0,
            semantic_refinement_contracts: 0,
            native_switch_verification_work: 43,
            native_switch_verification_scratch: 47,
        };
        let bound = identity_capture_resource_upper_bound_v1(census, 31, 37, 41).unwrap();
        let graph_items = 2 + 3 + 5 + 7 + 11 + 13 + 17 + 19;
        let attribute_sort_height = usize::BITS as usize - 17_usize.leading_zeros() as usize;
        let expected_work = graph_items * 7
            + 23 * 4
            + 29 * 2
            + 31 * 2
            + 23 * attribute_sort_height
            + 37 * 4
            + 41 * 3
            + 1
            + 43;
        let expected_retained = 29 + 31 + 37 + 41 + 9;
        let expected_temporary = 2 * 3 + 3 * 3 + (7 + 13) + 23 + 37 + 47;
        assert_eq!(bound.work_upper_bound(), expected_work);
        assert_eq!(bound.retained_storage_upper_bound(), expected_retained);
        assert_eq!(
            bound.peak_storage_upper_bound(),
            expected_retained + expected_temporary
        );
        assert!(
            ProductionAnalysisResourceLimitsV1::new(
                expected_work,
                expected_retained + expected_temporary
            )
            .admits(bound)
        );
        assert!(
            !ProductionAnalysisResourceLimitsV1::new(
                expected_work - 1,
                expected_retained + expected_temporary
            )
            .admits(bound)
        );
        assert!(
            !ProductionAnalysisResourceLimitsV1::new(
                expected_work,
                expected_retained + expected_temporary - 1
            )
            .admits(bound)
        );
    }

    #[test]
    fn capture_bound_arithmetic_overflow_is_typed() {
        let error = identity_capture_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                canonical_bytes: usize::MAX,
                ..ProductionAnalysisInputCensusV1::default()
            },
            1,
            0,
            0,
        )
        .unwrap_err();
        assert_eq!(
            error.phase,
            ProductionAnalysisResourcePhaseV1::StructuralIdentity
        );
        assert_eq!(error.resource, "identity capture work upper bound");
    }

    #[test]
    fn textual_preflight_bound_has_an_independent_exact_formula() {
        let census = IdentityPreflightCensusV1 {
            structural_work: 7,
            structural_storage: 11,
            rendered_entities: 13,
            type_roots: 23,
            records: 17,
            max_semantic_attributes_per_dictionary: 19,
            ..Default::default()
        };
        let bound = identity_textual_preflight_resource_upper_bound_v1(census).unwrap();
        let max_summary_bytes = MAX_DIAGNOSTIC_DETAIL_CHARS_V1 * 4 + 3;
        let expected_work = 13 * MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1
            + 17 * max_summary_bytes * 4
            + 23 * MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 * 4
            + 7;
        let expected_peak =
            5 * MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 + max_summary_bytes + 11 + 19;
        assert_eq!(bound.work_upper_bound(), expected_work);
        assert_eq!(bound.retained_storage_upper_bound(), 0);
        assert_eq!(bound.peak_storage_upper_bound(), expected_peak);
        assert!(
            ProductionAnalysisResourceLimitsV1::new(expected_work, expected_peak).admits(bound)
        );
        assert!(
            !ProductionAnalysisResourceLimitsV1::new(expected_work - 1, expected_peak)
                .admits(bound)
        );
        assert!(
            !ProductionAnalysisResourceLimitsV1::new(expected_work, expected_peak - 1)
                .admits(bound)
        );
    }

    #[test]
    fn textual_preflight_overflow_is_typed() {
        let error = identity_textual_preflight_resource_upper_bound_v1(IdentityPreflightCensusV1 {
            rendered_entities: usize::MAX,
            ..IdentityPreflightCensusV1::default()
        })
        .unwrap_err();
        assert_eq!(
            error.phase,
            ProductionAnalysisResourcePhaseV1::StructuralIdentity
        );
        assert_eq!(
            error.resource,
            "identity textual preflight work upper bound"
        );
    }

    #[test]
    fn preflight_and_capture_composition_sums_work_and_uses_lifetime_peak() {
        let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        let capture =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 7, 11, 13).unwrap();
        let preflight =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 17, 0, 31).unwrap();
        let combined = dominate_identity_preflight_bound_v1(capture, preflight).unwrap();
        assert_eq!(combined.work_upper_bound(), 24);
        assert_eq!(combined.retained_storage_upper_bound(), 11);
        assert_eq!(combined.peak_storage_upper_bound(), 31);
        assert!(ProductionAnalysisResourceLimitsV1::new(24, 31).admits(combined));
        assert!(!ProductionAnalysisResourceLimitsV1::new(23, 31).admits(combined));
        assert!(!ProductionAnalysisResourceLimitsV1::new(24, 30).admits(combined));

        let capture_dominates = dominate_identity_preflight_bound_v1(
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 3, 19, 23).unwrap(),
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 5, 0, 29).unwrap(),
        )
        .unwrap();
        assert_eq!(capture_dominates.work_upper_bound(), 8);
        assert_eq!(capture_dominates.peak_storage_upper_bound(), 42);
    }

    #[test]
    fn preflight_and_capture_composition_rejects_work_overflow() {
        let phase = ProductionAnalysisResourcePhaseV1::StructuralIdentity;
        let capture =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, usize::MAX, 0, 0).unwrap();
        let preflight =
            ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 1, 0, 0).unwrap();
        assert_eq!(
            dominate_identity_preflight_bound_v1(capture, preflight),
            Err(ProductionAnalysisResourceLimitV1 {
                phase,
                resource: "identity preflight and capture work upper bound",
            })
        );
    }

    #[test]
    fn real_capture_accepts_exact_limits_and_rejects_each_one_under() {
        let mut context = Context::new();
        let (function, _) = debug_named_function_v1(&mut context);
        assert_real_capture_exact_and_one_under_v1(&context, &function);
    }

    #[test]
    fn wide_deep_function_types_are_preflighted_before_owned_roster_traversal() {
        let mut context = Context::new();
        let function = nested_type_function_v1(&mut context, 24, 24);
        assert_real_capture_exact_and_one_under_v1(&context, &function);
        let identity = derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        assert_eq!(identity.block_count(), 1);
    }

    #[test]
    fn hostile_root_attribute_scan_obeys_exact_preflight_limits() {
        let mut context = Context::new();
        let (function, _) = debug_named_function_v1(&mut context);
        for ordinal in 0..128 {
            function.get_operation().deref_mut(&context).attributes.set(
                Identifier::try_from(format!("identity_root_attribute_{ordinal:03}")).unwrap(),
                UnitAttr::new(),
            );
        }
        let census = preflight_identity_structure_v1(
            &context,
            &function,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        assert_eq!(
            preflight_identity_structure_v1(
                &context,
                &function,
                ProductionAnalysisResourceLimitsV1::new(
                    census.structural_work,
                    census.structural_storage,
                ),
            )
            .unwrap(),
            census
        );
        assert!(
            preflight_identity_structure_v1(
                &context,
                &function,
                ProductionAnalysisResourceLimitsV1::new(
                    census.structural_work - 1,
                    census.structural_storage,
                ),
            )
            .is_err()
        );
        assert!(
            preflight_identity_structure_v1(
                &context,
                &function,
                ProductionAnalysisResourceLimitsV1::new(
                    census.structural_work,
                    census.structural_storage - 1,
                ),
            )
            .is_err()
        );
        assert_real_capture_exact_and_one_under_v1(&context, &function);
    }

    #[test]
    fn debug_ssa_names_are_excluded_from_structural_identity() {
        let mut context = Context::new();
        let (function, result) = debug_named_function_v1(&mut context);
        let unnamed = derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        result.set_name(
            &context,
            Some(
                Identifier::try_new("x".repeat(MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 * 32))
                    .unwrap(),
            ),
        );
        let named = derive_pliron_ir_structural_identity_v1(&context, &function).unwrap();
        assert!(unnamed.exactly_matches(&named));
    }

    #[test]
    fn huge_debug_name_is_rejected_by_textual_preflight_before_capture() {
        let mut context = Context::new();
        let (function, result) = debug_named_function_v1(&mut context);
        result.set_name(
            &context,
            Some(
                Identifier::try_new("x".repeat(MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 * 32))
                    .unwrap(),
            ),
        );
        let census = preflight_identity_structure_v1(
            &context,
            &function,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        let textual = identity_textual_preflight_resource_upper_bound_v1(census).unwrap();
        assert!(textual.peak_storage_upper_bound() > census.structural_storage);

        let limits = ProductionAnalysisResourceLimitsV1::new(
            textual.work_upper_bound(),
            textual.peak_storage_upper_bound() - 1,
        );
        let mut provider = LivePlironStructuralIdentityProviderV1::new(&context, &function);
        let error = provider
            .capture_with_resource_limits_v1(limits)
            .err()
            .expect("one-under textual workspace must reject capture");
        let IdentityCaptureFailureV1::ResourceLimit(error) = error else {
            panic!("textual preflight returned a non-resource failure")
        };
        assert_eq!(
            error.phase,
            ProductionAnalysisResourcePhaseV1::StructuralIdentity
        );
        assert_eq!(error.resource, "peak storage upper bound");
    }
}

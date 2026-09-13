#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    #[test]
    fn effect_bound_has_exact_and_one_under_admission() {
        let census = ProductionAnalysisInputCensusV1 {
            operations: 1,
            effect_refinement_contracts: 1,
            ..ProductionAnalysisInputCensusV1::default()
        };
        // One operation yields eleven possible contract obligations at rank 8.
        // Each finding retains 3*4096 text bytes, 16 witness words, and 32
        // fixed-field units. Work additionally covers the semantic table,
        // correlation, finding materialization, and one source-name render.
        const EXACT_WORK: usize = 172_613;
        const EXACT_RETAINED: usize = 135_696;
        const EXACT_PEAK: usize = 156_225;
        let exact = preflight_effect_refinement_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), EXACT_RETAINED);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_effect_refinement_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK,),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::EffectRefinement,
                resource: "work upper bound",
            })
        );
        assert_eq!(
            preflight_effect_refinement_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK - 1,),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::EffectRefinement,
                resource: "peak storage upper bound",
            })
        );
    }

    #[test]
    fn effect_bound_rejects_overflow_before_analysis() {
        assert_eq!(
            preflight_effect_refinement_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    operations: usize::MAX,
                    effect_refinement_contracts: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
            ),
            Err(effect_resource_overflow_v1())
        );
    }

    #[test]
    fn effect_bound_covers_every_full_rank_contract_obligation() {
        let exact = preflight_effect_refinement_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                operations: 1,
                effect_refinement_contracts: 1,
                ..ProductionAnalysisInputCensusV1::default()
            },
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let obligations = dialect_kernel::MAX_RANKED_MEMORY_RANK + 3;
        assert_eq!(
            exact.retained_storage_upper_bound(),
            obligations
                * (3 * MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1
                    + 2 * dialect_kernel::MAX_RANKED_MEMORY_RANK
                    + EFFECT_FINDING_FIXED_STORAGE_V1)
        );
    }

    #[test]
    fn contract_free_inventory_has_exact_allocation_free_boundaries() {
        const OPERATIONS: usize = 103;
        const MAX_OPERATION_ARITY: usize = 10;
        const EXACT_WORK: usize = OPERATIONS;
        const EXACT_PEAK: usize = 0;

        let census = ProductionAnalysisInputCensusV1 {
            operations: OPERATIONS,
            max_operation_arity: MAX_OPERATION_ARITY,
            effect_refinement_contracts: 0,
            ..ProductionAnalysisInputCensusV1::default()
        };
        let exact = preflight_effect_refinement_resource_upper_bound_v1(
            census,
            ProductionAnalysisResourceLimitsV1::new(EXACT_WORK, EXACT_PEAK),
        )
        .unwrap();
        assert_eq!(exact.work_upper_bound(), EXACT_WORK);
        assert_eq!(exact.retained_storage_upper_bound(), 0);
        assert_eq!(exact.peak_storage_upper_bound(), EXACT_PEAK);
        assert_eq!(
            preflight_effect_refinement_resource_upper_bound_v1(
                census,
                ProductionAnalysisResourceLimitsV1::new(EXACT_WORK - 1, EXACT_PEAK),
            ),
            Err(ProductionAnalysisResourceLimitV1 {
                phase: ProductionAnalysisResourcePhaseV1::EffectRefinement,
                resource: "work upper bound",
            })
        );
        let with_contract = preflight_effect_refinement_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                effect_refinement_contracts: 1,
                ..census
            },
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        assert_eq!(
            with_contract.retained_storage_upper_bound(),
            OPERATIONS * effect_finding_retained_storage_v1().unwrap()
        );
        assert!(with_contract.peak_storage_upper_bound() > EXACT_PEAK);
    }

    #[test]
    fn effect_mismatch_descriptions_are_independently_bounded() {
        let actual = bounded_effect_owned_diagnostic_v1("a".repeat(8_192));
        let expected = bounded_effect_owned_diagnostic_v1("界".repeat(4_096));
        let finding = PlironEffectRefinementFindingV1::ValueMismatch {
            view: bounded_effect_owned_diagnostic_v1("view".repeat(2_048)),
            location: EffectRefinementLocationV1 {
                block: 0,
                operation: 0,
            },
            actual,
            expected,
            witness: None,
        };
        let PlironEffectRefinementFindingV1::ValueMismatch {
            view,
            actual,
            expected,
            ..
        } = finding
        else {
            unreachable!()
        };
        for text in [&view, &actual, &expected] {
            assert!(text.len() <= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1);
            assert!(text.len() >= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3);
            assert!(text.ends_with("..."));
        }
    }

    #[test]
    fn hierarchy_projection_bounds_names_and_outer_formatting() {
        let dynamic = HierarchicalOwnershipFindingV1::DynamicExtentIncomplete {
            view: "界".repeat(4_096),
            dimension: 7,
        };
        let projected = project_hierarchy_finding_v1(&dynamic, None);
        let PlironEffectRefinementFindingV1::DynamicOwnershipIncomplete { view, detail, .. } =
            projected
        else {
            unreachable!()
        };
        assert!(view.len() <= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1);
        assert!(view.len() >= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3);
        assert!(detail.len() <= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1);
        assert!(detail.len() >= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3);
        assert!(view.ends_with("...") && detail.ends_with("..."));

        let rejected = HierarchicalOwnershipFindingV1::CoverageHole {
            view: "view".to_owned(),
            coordinate: vec![u64::MAX; 4_096],
            extents: vec![u64::MAX; 4_096],
        };
        let projected = project_hierarchy_finding_v1(&rejected, None);
        let PlironEffectRefinementFindingV1::OwnershipRejected { detail } = projected else {
            unreachable!()
        };
        assert!(detail.len() <= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1);
        assert!(detail.len() >= MAX_PLIRON_SEMANTIC_DIAGNOSTIC_BYTES_V1 - 3);
        assert!(detail.ends_with("..."));
    }
}

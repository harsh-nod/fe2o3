#[cfg(test)]
mod ownership_resource_tests {
    use super::*;
    use crate::ProductionAnalysisResourcePhaseV1 as Phase;
    use crate::production_analysis::pliron_pass_contract::PlironStructuralIdentityProviderV1;
    use dialect_kernel::{
        AccessKindAttr, DIALECT_NAME, IndexConstantOp, InvocationIndexOp, RankedAccessOp,
        RankedViewOp, RankedViewType, ReturnOp, register_dialect,
    };
    use pliron::{
        builtin::types::FunctionType, dialect::DialectName, op::Op, operation::verify_operation,
    };

    fn bound(
        work: usize,
        retained: usize,
        temporary: usize,
    ) -> ProductionAnalysisResourceUpperBoundV1 {
        ProductionAnalysisResourceUpperBoundV1::checked_phase(
            Phase::HierarchicalOwnership,
            work,
            retained,
            temporary,
        )
        .unwrap()
    }

    fn compose(
        contracts: usize,
        local: ProductionAnalysisResourceUpperBoundV1,
        bounds: ProductionAnalysisResourceUpperBoundV1,
        race: ProductionAnalysisResourceUpperBoundV1,
    ) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
        compose_hierarchical_ownership_resource_upper_bound_v1(
            ProductionAnalysisInputCensusV1 {
                ownership_contracts: contracts,
                ..ProductionAnalysisInputCensusV1::default()
            },
            local,
            bounds,
            race,
        )
    }

    fn assert_boundary(
        actual: ProductionAnalysisResourceUpperBoundV1,
        work: usize,
        retained: usize,
        peak: usize,
    ) {
        assert_eq!(actual.work_upper_bound(), work);
        assert_eq!(actual.retained_storage_upper_bound(), retained);
        assert_eq!(actual.peak_storage_upper_bound(), peak);
        assert_eq!(
            ProductionAnalysisResourceLimitsV1::new(work, peak)
                .require(Phase::HierarchicalOwnership, actual),
            Ok(actual),
        );
        for (limits, resource) in [
            (
                ProductionAnalysisResourceLimitsV1::new(work - 1, peak),
                "work upper bound",
            ),
            (
                ProductionAnalysisResourceLimitsV1::new(work, peak - 1),
                "peak storage upper bound",
            ),
        ] {
            let error = limits
                .require(Phase::HierarchicalOwnership, actual)
                .unwrap_err();
            assert_eq!(
                error,
                ProductionAnalysisResourceLimitV1 {
                    phase: Phase::HierarchicalOwnership,
                    resource,
                },
            );
            assert_eq!(
                resource_upper_bound_error_v1(error),
                ProductionPlironPreloweringErrorV2::ResourceLimit {
                    phase: Phase::HierarchicalOwnership,
                    producing_pass: None,
                    resource,
                },
            );
        }
    }

    #[test]
    fn ownership_empty_composition_keeps_the_exact_local_bound() {
        let local = bound(11, 3, 5);
        let actual = compose(0, local, bound(17, 7, 13), bound(23, 11, 19)).unwrap();
        assert_eq!(actual, local);
        assert_boundary(actual, 11, 3, 8);
    }

    #[test]
    fn ownership_nonempty_composition_preserves_nested_order_and_boundaries() {
        let local = bound(11, 3, 5);
        let bounds = bound(17, 7, 13);
        let race = bound(23, 11, 19);
        let legacy = local
            .checked_with_nested_sequence_discard(&[bounds, race], Phase::HierarchicalOwnership)
            .unwrap();
        for contracts in [1, 17, usize::MAX] {
            let actual = compose(contracts, local, bounds, race).unwrap();
            assert_eq!(actual, legacy);
            // Work 11+17+23; retained 3; peak (3+5)+max(7+13,11+19).
            assert_boundary(actual, 51, 3, 38);
        }
    }

    #[test]
    fn ownership_empty_composition_does_not_reserve_unreachable_overflows() {
        let local = bound(11, 3, 5);
        for (bounds, race) in [
            (bound(usize::MAX, 0, 0), bound(1, 0, 0)),
            (bound(0, usize::MAX, 0), bound(0, 0, 0)),
            (bound(0, 0, 0), bound(0, 0, usize::MAX)),
        ] {
            assert_eq!(compose(0, local, bounds, race), Ok(local));
        }
        let largest = bound(usize::MAX, usize::MAX, 0);
        assert_eq!(
            compose(0, largest, bound(1, 0, 1), bound(1, 0, 1)),
            Ok(largest)
        );
    }

    #[test]
    fn ownership_nonempty_overflow_preserves_phase_and_error_precedence() {
        let local = bound(11, 3, 5);
        for (bounds, race, resource) in [
            (
                bound(usize::MAX, usize::MAX, 0),
                bound(0, 0, 0),
                "nested work upper bound",
            ),
            (
                bound(0, 0, 0),
                bound(usize::MAX, usize::MAX, 0),
                "nested work upper bound",
            ),
            (
                bound(0, usize::MAX, 0),
                bound(0, 0, 0),
                "nested peak storage upper bound",
            ),
            (
                bound(0, 0, 0),
                bound(0, 0, usize::MAX),
                "nested peak storage upper bound",
            ),
        ] {
            let error = ProductionAnalysisResourceLimitV1 {
                phase: Phase::HierarchicalOwnership,
                resource,
            };
            assert_eq!(compose(1, local, bounds, race), Err(error));
            assert_eq!(
                local.checked_with_nested_sequence_discard(
                    &[bounds, race],
                    Phase::HierarchicalOwnership,
                ),
                Err(error),
            );
        }
    }

    fn read_function(context: &mut Context, name: &str, accesses: usize, index: u64) -> FuncOp {
        register_dialect(context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
        dialect_gpu::register_dialect(context).unwrap();
        let function = FuncOp::new(
            context,
            name.try_into().unwrap(),
            FunctionType::get(context, vec![], vec![]),
        );
        let entry = function.get_entry_block(context);
        let index = IndexConstantOp::new(context, index);
        index.get_operation().insert_at_back(entry, context);
        let ty = RankedViewType::new(context, 32, false, vec![1]).unwrap();
        let view = RankedViewOp::new(context, ty, vec![]).unwrap();
        view.get_operation().insert_at_back(entry, context);
        for _ in 0..accesses {
            let access = RankedAccessOp::new(
                context,
                AccessKindAttr::Read,
                view.result(context),
                vec![index.result(context)],
            )
            .unwrap();
            access.get_operation().insert_at_back(entry, context);
        }
        ReturnOp::new(context)
            .get_operation()
            .insert_at_back(entry, context);
        verify_operation(function.get_operation(), context).unwrap();
        function
    }

    fn census(context: &Context, function: &FuncOp) -> ProductionAnalysisInputCensusV1 {
        let mut provider = LivePlironStructuralIdentityProviderV1::new(context, function);
        provider
            .capture_with_resource_limits_v1(
                ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
            )
            .ok()
            .unwrap()
            .input_census
    }

    #[test]
    fn ownership_empty_authenticated_read_pipeline_completes_all_stages() {
        let name = "ownership_empty_reads";
        let context = &mut Context::new();
        let function = read_function(context, name, 64, 0);
        let input = census(context, &function);
        assert_eq!(input.ownership_contracts, 0);
        assert_eq!(input.ranked_accesses, 64);
        assert_eq!(input.operations, 67);
        let local = preflight_hierarchical_ownership_resource_upper_bound_v1(
            input,
            None,
            ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
        )
        .unwrap();
        assert_eq!(local, bound(67, 0, 0));

        let report = require_production_pliron_checks_before_lowering_v2(context, &function)
            .unwrap_or_else(|error| {
                panic!(
                    "short no-contract fixture (name bytes={}, reads=64) failed: {error:?}",
                    name.len(),
                )
            });
        assert!(report.is_clean());
        assert!(report.bounds().is_clean());
        assert!(report.race().is_clean());
        assert!(report.ownership().is_clean());
        assert!(report.preservation().is_exact_identity());
        assert_eq!(report.preservation().certificates().len(), 9);
        assert!(!report.grants_compiler_refinement_authority());
        assert!(!report.grants_artifact_or_launch_authority());
    }

    #[test]
    fn ownership_empty_long_name_does_not_inflate_numeric_race_diagnostics() {
        const FINDING_ROWS: usize = 4_096;
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        // This name used to be charged once per possible race finding even
        // though the analysis now retains only bounded numeric value names.
        let name = "n".repeat(limits.max_peak_storage() / FINDING_ROWS + 1);
        let context = &mut Context::new();
        let function = read_function(context, &name, 64, 0);
        let input = census(context, &function);
        assert_eq!(input.ownership_contracts, 0);
        assert_eq!(input.ranked_accesses, 64);
        assert_eq!(input.allocation_effects, 0);
        assert_eq!(input.operations, 67);
        assert_eq!(input.ranked_accesses * input.ranked_accesses, FINDING_ROWS);
        assert!(input.identifier_bytes >= name.len());
        assert!(
            input.identifier_bytes.checked_mul(FINDING_ROWS).unwrap() > limits.max_peak_storage()
        );
        let singleton = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("singleton retains one diagnostic, not an exact-fallback finding vector");
        assert!(singleton.is_clean());
        assert!(singleton.race().is_clean());
        assert!(singleton.preservation().is_exact_identity());
        assert_eq!(singleton.preservation().certificates().len(), 9);
        assert!(!singleton.grants_compiler_refinement_authority());
        assert!(!singleton.grants_artifact_or_launch_authority());

        InvocationIndexOp::new(context, 0, 2)
            .get_operation()
            .insert_at_front(function.get_entry_block(context), context);
        assert_eq!(census(context, &function).operations, 68);
        let concurrent = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("unrelated function text is not retained once per numeric race finding");
        assert!(concurrent.is_clean());
        assert!(concurrent.race().is_clean());
        assert!(concurrent.ownership().is_clean());
        assert!(concurrent.preservation().is_exact_identity());
        assert_eq!(concurrent.preservation().certificates().len(), 9);
        assert!(!concurrent.grants_compiler_refinement_authority());
        assert!(!concurrent.grants_artifact_or_launch_authority());
    }

    #[test]
    fn ownership_empty_does_not_skip_primary_race_storage_admission() {
        const READS: usize = 64;
        const INVOCATIONS: u64 = 16_384;
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        let context = &mut Context::new();
        let function = read_function(context, "ownership_empty_actual_race_budget", READS, 0);
        let singleton = require_production_pliron_checks_before_lowering_v2(context, &function)
            .expect("the singleton fixture must pass every earlier stage");
        assert!(singleton.is_clean());
        InvocationIndexOp::new(context, 0, INVOCATIONS)
            .get_operation()
            .insert_at_front(function.get_entry_block(context), context);
        let input = census(context, &function);
        assert_eq!(input.ownership_contracts, 0);
        assert_eq!(input.ranked_accesses, READS);
        assert_eq!(input.allocation_effects, 0);
        assert_eq!(input.operations, READS + 4);
        // One million possible effect instances need at least 136 address-state
        // units each at rank8. This genuine shape exceeds the unchanged ceiling.
        let instances = READS * usize::try_from(INVOCATIONS).unwrap();
        assert_eq!(instances, crate::MAX_PLIRON_RACE_EFFECT_INSTANCES_V1);
        assert!(
            instances * (dialect_kernel::MAX_RANKED_MEMORY_RANK * 9 + 64)
                > limits.max_peak_storage()
        );
        let Err(error) = require_production_pliron_checks_before_lowering_v2(context, &function)
        else {
            panic!("empty ownership bypassed the primary race storage admission")
        };
        assert!(
            matches!(
                error,
                ProductionPlironPreloweringErrorV2::ResourceLimit {
                    phase: Phase::RaceFreedom,
                    producing_pass: None,
                    resource: "peak storage upper bound",
                }
            ),
            "unexpected primary gate: {error:?}"
        );
    }

    #[test]
    fn ownership_empty_does_not_skip_the_primary_bounds_gate() {
        let context = &mut Context::new();
        let function = read_function(context, "ownership_empty_bad_read", 1, 1);
        assert_eq!(census(context, &function).ownership_contracts, 0);
        assert!(matches!(
            require_production_pliron_checks_before_lowering_v2(context, &function),
            Err(ProductionPlironPreloweringErrorV2::Bounds(_)),
        ));
    }
}

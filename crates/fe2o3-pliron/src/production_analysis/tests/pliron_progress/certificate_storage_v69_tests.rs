mod certificate_storage_v69_tests {
    use super::*;
    use crate::production_analysis::pliron_ir_identity::LivePlironStructuralIdentityProviderV1;
    use crate::production_analysis::pliron_pass_contract::{
        PRODUCTION_PLIRON_PASS_CONTRACTS_V1, PlironStructuralIdentityProviderV1,
        begin_production_pliron_pass_contract_session_v1,
    };
    use crate::production_analysis::pliron_progress::{
        preflight_progress_resource_upper_bound_v1,
        preflight_scoped_progress_resource_upper_bound_v1,
        run_pliron_progress_with_scoped_input_v1,
    };
    use crate::production_analysis::pliron_resource_envelope::{
        ProductionAnalysisInputCensusV1, ProductionAnalysisResourceLimitV1,
        ProductionAnalysisResourceLimitsV1, ProductionAnalysisResourcePhaseV1,
    };
    use pliron::{common_traits::Named, linked_list::ContainsLinkedList};

    fn unlimited() -> ProductionAnalysisResourceLimitsV1 {
        ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX)
    }

    fn values(context: &Context, function: &FuncOp) -> Vec<Value> {
        let mut values = Vec::new();
        for block in function.get_region(context).deref(context).iter(context) {
            values.extend(block.deref(context).arguments());
            for operation in block.deref(context).iter(context) {
                values.extend(operation.deref(context).results());
            }
        }
        values
    }

    fn assert_certificate_alias_independence(nested: bool) {
        let context = &mut setup();
        let function = if nested {
            nested_loop(context, NestedLoopCase::Canonical)
        } else {
            constant_loop(context, 0, 7, 1)
        };
        let values = values(context, &function);
        // Keep debug-attribute owner counts identical in both censuses.
        for value in &values {
            value.set_name(context, Some("initial_alias".try_into().unwrap()));
        }
        verify_operation(function.get_operation(), context).unwrap();
        let expected = run_pliron_progress_check_v1(context, &function);
        assert!(expected.is_clean(), "{expected:?}");
        assert_eq!(expected.certificates().len(), if nested { 2 } else { 1 });
        let mut provider = LivePlironStructuralIdentityProviderV1::new(context, &function);
        let before = provider
            .capture_with_resource_limits_v1(unlimited())
            .ok()
            .unwrap();
        let bound =
            preflight_scoped_progress_resource_upper_bound_v1(before.input_census, unlimited())
                .unwrap();
        for alias in ["short_alias".to_owned(), "d".repeat(262_144)] {
            for value in &values {
                value.set_name(context, Some(alias.clone().try_into().unwrap()));
            }
            verify_operation(function.get_operation(), context).unwrap();
            let observed = provider
                .capture_with_resource_limits_v1(unlimited())
                .ok()
                .unwrap();
            assert!(
                provider
                    .require_exact_identity(&before.snapshot, &observed.snapshot)
                    .is_ok()
            );
            assert_eq!(observed.input_census, before.input_census);
            assert_eq!(
                preflight_scoped_progress_resource_upper_bound_v1(
                    observed.input_census,
                    unlimited()
                ),
                Ok(bound)
            );
            let report = run_pliron_progress_check_v1(context, &function);
            assert_eq!(
                report,
                expected,
                "nested={nested}, alias length={}",
                alias.len()
            );
            for certificate in report.certificates() {
                for label in [certificate.induction(), certificate.bound()] {
                    assert!(label.starts_with('v'));
                    assert!(label[1..].bytes().all(|byte| byte.is_ascii_digit()));
                    assert!((2..=21).contains(&label.len()));
                }
            }
            let (owned, copied) = report.validation_text_storage_v1().unwrap();
            assert!(owned <= 128 * report.certificates().len());
            assert!(copied <= 42 * report.certificates().len());
            assert_eq!(report.try_clone_validation_payload_v1().unwrap(), report);
        }
    }

    #[test]
    fn canonical_certificate_labels_ignore_debug_aliases() {
        assert_certificate_alias_independence(false);
    }

    #[test]
    fn nested_certificate_labels_ignore_debug_aliases() {
        assert_certificate_alias_independence(true);
    }

    #[test]
    fn rejected_loop_proofs_keep_their_findings_with_long_aliases() {
        for nested in [false, true] {
            let context = &mut setup();
            let function = if nested {
                nested_loop(context, NestedLoopCase::ZeroInnerStep)
            } else {
                constant_loop(context, 0, 7, 0)
            };
            let expected = run_pliron_progress_check_v1(context, &function);
            assert!(!expected.is_clean(), "{expected:?}");
            assert!(expected.certificates().is_empty());
            for value in values(context, &function) {
                value.set_name(context, Some("d".repeat(262_144).try_into().unwrap()));
            }
            verify_operation(function.get_operation(), context).unwrap();
            assert_eq!(run_pliron_progress_check_v1(context, &function), expected);
        }
    }

    #[test]
    fn pinned_numeric_value_label_fits_the_fixed_requested_payload() {
        // Exercise the pinned Value::id rendering maximum without forging a UID.
        let largest = format!("v{}", u64::MAX);
        assert_eq!(largest, "v18446744073709551615");
        assert_eq!(largest.len(), 21);
        assert!(largest.capacity() <= 64);
        let context = &mut setup();
        let (function, arguments) = make_function(context, "numeric_labels", 1);
        let ret = ReturnOp::new(context);
        append(context, function.get_entry_block(context), &ret);
        arguments[0].set_name(context, Some("d".repeat(262_144).try_into().unwrap()));
        let label: String = arguments[0].id(context).into();
        assert!(label.starts_with('v'));
        assert!(label[1..].bytes().all(|byte| byte.is_ascii_digit()));
        assert!(label.len() <= 21);
        assert!(label.capacity() <= 64);
    }

    #[test]
    fn fixed_certificate_payload_has_literal_exact_and_one_under_limits() {
        for identifier_bytes in [0, 190, usize::MAX] {
            let census = ProductionAnalysisInputCensusV1 {
                blocks: 2,
                operations: 3,
                operands: 4,
                block_arguments: 2,
                attributes: 1,
                successors: 3,
                identifier_bytes,
                ..ProductionAnalysisInputCensusV1::default()
            };
            // S=19; charged=67; dominators=50; loops=120; parallel payloads=2976.
            // Retained=5586; scoped base temporary=120, plus payload owner19.
            // Standalone adds12 verifier work and omits3 scoped-owner cells.
            for (scoped, work, peak) in [(true, 3_289, 5_725), (false, 3_301, 5_722)] {
                let preflight = if scoped {
                    preflight_scoped_progress_resource_upper_bound_v1
                } else {
                    preflight_progress_resource_upper_bound_v1
                };
                let bound =
                    preflight(census, ProductionAnalysisResourceLimitsV1::new(work, peak)).unwrap();
                assert_eq!(bound.work_upper_bound(), work);
                assert_eq!(bound.retained_storage_upper_bound(), 5_586);
                assert_eq!(bound.peak_storage_upper_bound(), peak);
                for (work_limit, storage_limit, resource) in [
                    (work - 1, peak, "work upper bound"),
                    (work, peak - 1, "peak storage upper bound"),
                ] {
                    assert_eq!(
                        preflight(
                            census,
                            ProductionAnalysisResourceLimitsV1::new(work_limit, storage_limit)
                        ),
                        Err(ProductionAnalysisResourceLimitV1 {
                            phase: ProductionAnalysisResourcePhaseV1::Progress,
                            resource,
                        })
                    );
                }
            }
        }
    }

    #[test]
    fn authenticated_long_name_chain_admits_progress_without_phantom_certificate_text() {
        const BLOCKS: usize = 768;
        const NAME_BYTES: usize = 64_000;
        let context = &mut setup();
        let (function, _) = make_function(context, &"f".repeat(NAME_BYTES), 0);
        let mut previous = function.get_entry_block(context);
        for _ in 1..BLOCKS {
            let next = BasicBlock::new(context, None, vec![]);
            next.insert_at_back(function.get_region(context), context);
            let branch = BranchOp::new(context, next);
            append(context, previous, &branch);
            previous = next;
        }
        let ret = ReturnOp::new(context);
        append(context, previous, &ret);
        verify_operation(function.get_operation(), context).unwrap();
        let mut session = begin_production_pliron_pass_contract_session_v1(
            LivePlironStructuralIdentityProviderV1::new(context, &function),
        )
        .unwrap();
        let census = session.input_census_v1();
        assert_eq!(census.blocks, BLOCKS);
        assert_eq!(census.operations, BLOCKS);
        assert_eq!(census.successors, BLOCKS - 1);
        assert_eq!(census.operands, 0);
        assert!(census.identifier_bytes >= NAME_BYTES);
        let limits = ProductionAnalysisResourceLimitsV1::production_hard_ceiling();
        // The function name alone made the former certificate term exceed
        // the complete production ceiling, even though this graph is acyclic.
        const FORMER_TEXT_LOWER_BOUND: usize = 98_176_000;
        assert_eq!(2 * (BLOCKS - 1) * NAME_BYTES, FORMER_TEXT_LOWER_BOUND);
        assert!(FORMER_TEXT_LOWER_BOUND > limits.max_peak_storage());
        let bound = preflight_scoped_progress_resource_upper_bound_v1(census, limits).unwrap();
        assert!(bound.peak_storage_upper_bound() < limits.max_peak_storage());

        // These are preservation-only transitions, not eight analysis runs.
        // The real live provider authenticates the same graph at every step.
        for contract in &PRODUCTION_PLIRON_PASS_CONTRACTS_V1[..8] {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        let report = session
            .run_scoped_semantic_refinement_with_resource_limits_v1(limits, |input| {
                let scoped = run_pliron_progress_with_scoped_input_v1(input)?;
                assert!(std::ptr::eq(scoped.context, &*context));
                assert_eq!(scoped.function.get_operation(), function.get_operation());
                Ok(Ok::<_, ()>(scoped.report))
            })
            .unwrap()
            .unwrap();
        assert!(report.is_clean(), "{report:?}");
        assert!(report.certificates().is_empty());
        assert!(report.findings().is_empty());
    }
}

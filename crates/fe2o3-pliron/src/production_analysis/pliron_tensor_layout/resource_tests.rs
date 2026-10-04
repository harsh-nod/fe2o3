#[cfg(test)]
mod resource_upper_bound_tests {
    use super::*;

    fn census() -> ProductionAnalysisInputCensusV1 {
        ProductionAnalysisInputCensusV1 {
            blocks: 6,
            operations: 30,
            operands: 54,
            results: 21,
            successors: 9,
            block_arguments: 6,
            attributes: 18,
            type_nodes: 37,
            identifier_bytes: 180,
            canonical_bytes: 1_100,
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
            semantic_definitions: 0,
            semantic_refinement_contracts: 0,
            native_switch_verification_work: 0,
            native_switch_verification_scratch: 0,
        }
    }

    #[test]
    fn tensor_layout_bound_accepts_exact_limits_and_rejects_one_under() {
        let census = census();
        let exact = preflight_tensor_layout_resource_upper_bound_v1(
            census,
            None,
            ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX),
        )
        .unwrap();
        let findings = (30 * MAX_TENSOR_CONTRACT_FINDINGS_PER_SITE_V1 + 1)
            .min(MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1 + 1);
        let retained = findings * (96 + dialect_kernel::MAX_RANKED_MEMORY_RANK * 4)
            + 180
            + 30 * 24
            + findings * 24;
        let ordinary_symbolic = 27 * 16 + 54 * 3 + 6 * 16 + 9 * 6 + 6 * 3;
        let phi_rows = 2 * 6;
        let selector_headers_and_facts = (3 + 1 + 1) * 6;
        let forwarding_row_words = std::mem::size_of::<SubgroupForwardingRowV1<'static>>()
            .div_ceil(std::mem::size_of::<usize>());
        let forwarding_headers = std::mem::size_of::<SubgroupForwardingIndexV1<'static>>()
            .div_ceil(std::mem::size_of::<usize>());
        // Six borrowed rows, six path slots, at most six complete tree nodes.
        let forwarding_storage = 6 * (forwarding_row_words + 1 + 64) + forwarding_headers;
        let temporary = 30 * 32
            + MAX_TENSOR_FRAGMENT_COORDINATES_V1 * TENSOR_COORDINATE_MAP_ITEMS_PER_ENTRY_V1
            + 23
            + ordinary_symbolic
            + phi_rows
            + forwarding_storage.max(selector_headers_and_facts);
        assert_eq!(exact.retained_storage_upper_bound(), retained);
        assert_eq!(exact.peak_storage_upper_bound(), retained + temporary);
        assert_eq!(
            preflight_tensor_layout_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound(),
                ),
            ),
            Ok(exact)
        );
        assert!(
            preflight_tensor_layout_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound() - 1,
                    exact.peak_storage_upper_bound(),
                ),
            )
            .is_err()
        );
        assert!(
            preflight_tensor_layout_resource_upper_bound_v1(
                census,
                None,
                ProductionAnalysisResourceLimitsV1::new(
                    exact.work_upper_bound(),
                    exact.peak_storage_upper_bound() - 1,
                ),
            )
            .is_err()
        );
    }

    #[test]
    fn tensor_layout_bound_rejects_overflow_and_internal_caps() {
        let unlimited = ProductionAnalysisResourceLimitsV1::new(usize::MAX, usize::MAX);
        assert!(
            preflight_tensor_layout_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    results: usize::MAX,
                    block_arguments: 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                None,
                unlimited,
            )
            .is_err()
        );
        assert!(
            preflight_tensor_layout_resource_upper_bound_v1(
                ProductionAnalysisInputCensusV1 {
                    operations: MAX_PLIRON_TENSOR_LAYOUT_OPERATIONS_V1 + 1,
                    ..ProductionAnalysisInputCensusV1::default()
                },
                None,
                unlimited,
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rejected_contract() -> PlironTensorLayoutFindingV1 {
        PlironTensorLayoutFindingV1::Contract {
            block: 0,
            operation: 0,
            finding: TensorLayoutFindingV1::TailMaskMismatch,
        }
    }

    #[test]
    fn every_tensor_layout_finding_has_the_shared_status() {
        let incomplete = [
            PlironTensorLayoutFindingV1::Contract {
                block: 0,
                operation: 0,
                finding: TensorLayoutFindingV1::UnsupportedProfile,
            },
            PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete {
                detail: "unresolved".to_owned(),
            },
            PlironTensorLayoutFindingV1::ResourceLimitExceeded,
        ];
        for finding in incomplete {
            assert_eq!(finding.status(), KernelCheckStatusV1::Incomplete);
        }

        let rejected = [
            rejected_contract(),
            PlironTensorLayoutFindingV1::ActiveLaneMismatch {
                block: 0,
                operation: 0,
                expected: 64,
                actual: 32,
            },
            PlironTensorLayoutFindingV1::ExecutionLayoutMismatch {
                block: 0,
                operation: 0,
                declared: 32,
                required: 64,
            },
            PlironTensorLayoutFindingV1::ConvergenceMismatch {
                block: 0,
                operation: 0,
                actual: TensorConvergenceAttr::Divergent,
            },
            PlironTensorLayoutFindingV1::MalformedContract {
                block: 0,
                operation: 0,
            },
            PlironTensorLayoutFindingV1::DivergentInstructionTrace {
                first_invocation: vec![0],
                first_trace: vec![(0, 0)],
                second_invocation: vec![1],
                second_trace: vec![],
            },
            PlironTensorLayoutFindingV1::PartialSubgroupParticipation {
                grid: 0,
                workgroup: 0,
                subgroup: 0,
                expected: 64,
                actual: 63,
            },
            PlironTensorLayoutFindingV1::DivergentSubgroupControl {
                block: 0,
                operation: 0,
                controller: 1,
            },
        ];
        for finding in rejected {
            assert_eq!(finding.status(), KernelCheckStatusV1::Rejected);
        }
    }

    #[test]
    fn rejected_tensor_finding_dominates_an_incomplete_finding() {
        let mixed = report(vec![
            PlironTensorLayoutFindingV1::ResourceLimitExceeded,
            rejected_contract(),
        ]);
        assert_eq!(mixed.status(), KernelCheckStatusV1::Rejected);
        assert!(!mixed.is_clean());
        assert_eq!(report(vec![]).status(), KernelCheckStatusV1::Clean);
    }

    #[test]
    fn shared_tensor_reachability_condenses_cycles_and_multiple_sites() {
        let successors = vec![vec![1], vec![2], vec![1, 3], vec![4], vec![]];
        let mut work = 0;
        let predecessors = bounded_predecessors(&successors, &mut work).unwrap();
        let reachability =
            bounded_tensor_reachability(&successors, &predecessors, &[2, 4], &mut work).unwrap();

        for block in [0, 1, 2] {
            assert!(reachability.block_reaches(block, 0).unwrap());
            assert!(reachability.block_reaches(block, 1).unwrap());
        }
        for block in [3, 4] {
            assert!(!reachability.block_reaches(block, 0).unwrap());
            assert!(reachability.block_reaches(block, 1).unwrap());
        }
        assert!(work < MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1);
    }

    #[test]
    fn shared_tensor_reachability_handles_the_site_limit_in_four_words() {
        let block_count = MAX_PLIRON_TENSOR_LAYOUT_FINDINGS_V1;
        let successors = (0..block_count)
            .map(|block| {
                (block + 1 < block_count)
                    .then_some(block + 1)
                    .into_iter()
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let tensor_blocks = (0..block_count).collect::<Vec<_>>();
        let mut work = 0;
        let predecessors = bounded_predecessors(&successors, &mut work).unwrap();
        let reachability =
            bounded_tensor_reachability(&successors, &predecessors, &tensor_blocks, &mut work)
                .unwrap();

        assert!(reachability.block_reaches(0, block_count - 1).unwrap());
        assert!(!reachability.block_reaches(block_count - 1, 0).unwrap());
        assert_eq!(reachability.tensors_by_component[0].len(), 4);
        assert!(work < MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1);
    }

    #[test]
    fn shared_tensor_reachability_rejects_malformed_inputs_without_panicking() {
        let mut work = 0;
        assert!(matches!(
            bounded_tensor_reachability(&[vec![]], &[], &[0], &mut work),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("do not match")
        ));

        let mut work = 0;
        assert!(matches!(
            bounded_tensor_reachability(&[vec![1]], &[vec![]], &[0], &mut work),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("edge is outside")
        ));

        let mut work = 0;
        assert!(matches!(
            bounded_tensor_reachability(&[vec![]], &[vec![]], &[1], &mut work),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("tensor block is outside")
        ));
    }

    #[test]
    fn shared_tensor_reachability_obeys_the_convergence_budget() {
        let mut work = MAX_PLIRON_TENSOR_UNIFORMITY_WORK_UNITS_V1;
        assert!(matches!(
            bounded_tensor_reachability(&[vec![]], &[vec![]], &[0], &mut work),
            Err(PlironTensorLayoutFindingV1::ResourceLimitExceeded)
        ));
    }

    #[test]
    fn tensor_reachability_queries_fail_typed_at_both_boundaries() {
        let reachability = TensorReachabilityV1 {
            component_of: vec![0],
            tensors_by_component: vec![vec![1]],
        };
        assert!(matches!(
            reachability.block_reaches(1, 0),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("block outside")
        ));
        assert!(matches!(
            reachability.block_reaches(0, 64),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("unknown tensor site")
        ));
    }

    #[test]
    fn postdominators_reject_malformed_inputs_without_panicking() {
        let mut work = 0;
        assert!(matches!(
            bounded_postdominators(&[vec![]], &[], &[vec![]], &mut work),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("do not match")
        ));

        let mut work = 0;
        assert!(matches!(
            bounded_postdominators(&[vec![]], &[true], &[vec![1]], &mut work),
            Err(PlironTensorLayoutFindingV1::ConvergenceAnalysisIncomplete { detail })
                if detail.contains("predecessor is outside")
        ));
    }
}

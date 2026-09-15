mod output_operation_contract_actual_tests_v1 {
    use super::*;
    include!("source_output_pipeline_fixture_v1_tests.rs");

    fn source() -> ProductionPreRankedKirOwnerV1 {
        let (ssa, launch) = pipeline_fixture(false);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap()
    }

    fn candidate(
        name: &str,
        count: usize,
        fence: bool,
    ) -> fe2o3_pliron::ProductionRankedKernelLoweringInputV1 {
        let mut operations = vec![ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [64, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        }];
        for _ in 0..count {
            operations.push(if fence {
                ProductionRankedOperationV1::Fence {
                    memory_scope: MemoryScopeAttr::Device,
                    address_space: AddressSpaceAttr::Global,
                    order: MemoryOrderAttr::Release,
                }
            } else {
                ProductionRankedOperationV1::Barrier {
                    execution_scope: HierarchyAttr::Workgroup,
                    memory_scope: MemoryScopeAttr::Workgroup,
                    address_space: AddressSpaceAttr::Workgroup,
                    order: MemoryOrderAttr::AcquireRelease,
                }
            });
        }
        let kernel = ProductionRankedKernelV1::new(
            name,
            2,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("output_contract_diagnostic_candidate", kernel)
                .unwrap(),
            ProductionSessionLimitsV1::default(),
        )
        .unwrap()
    }

    #[test]
    fn genuine_pipeline_source_checked_output_matches_only_exact_synchronization_contracts() {
        let source = source();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_global_view(&source, profile, |view, budget| {
                assert_ne!(
                    view.bound().canonical().canonical_bytes(),
                    view.output().canonical().canonical_bytes()
                );
                let name = source.semantic_ssa().source_semantic().functions()[0]
                    .kernel_entry()
                    .unwrap()
                    .export_symbol();
                let name = std::str::from_utf8(name.as_bytes()).unwrap();
                let exact = candidate(name, 1, false);
                // This is a verified diagnostic candidate, not a projected O
                // root. Pipeline generated effects still require their own
                // complete transport, ordering and mandatory census checks.
                view.check_ranked_synchronization_tensor_contracts(ROOT, ROOT, &exact, budget)
                    .unwrap();
                for (count, fence) in [(0, false), (2, false), (1, true)] {
                    let wrong = candidate(name, count, fence);
                    assert!(matches!(
                        view.check_ranked_synchronization_tensor_contracts(
                            ROOT, ROOT, &wrong, budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "output synchronization contract mismatch"
                        ))
                    ));
                }
                let wrong = candidate("foreign_contract_root", 1, false);
                assert!(matches!(
                    view.check_ranked_synchronization_tensor_contracts(ROOT, ROOT, &wrong, budget),
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "output contract source name changed"
                    ))
                ));
                assert!(matches!(
                    view.check_ranked_synchronization_tensor_contracts(
                        ROOT,
                        SemanticFunctionIdV1::from_index(1),
                        &exact,
                        budget
                    ),
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "output contract source alias absent"
                    ))
                ));
                assert!(!view.grants_authority());
            });
        }
    }

    fn target_scan_work(
        capabilities: &std::collections::BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    ) -> usize {
        use fe2o3_kernel_ir::{AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE as N, TargetCapability};
        let mut work = 1;
        for capability in capabilities {
            work += 2;
            if let TargetCapability::Extension { namespace, name } = capability {
                // Fixture shape facts, independent of the query's measured work.
                // Other namespaces have unequal lengths and stop before bytes.
                if namespace == N {
                    work += 1 + N.len() + 2 * (name.len() + 1);
                } else {
                    assert_ne!(namespace.len(), N.len());
                    work += 1;
                }
            }
        }
        work
    }

    #[test]
    fn genuine_output_contract_query_has_shape_derived_exact_under_and_partial_storage_cleanup() {
        let source = source();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_global_view(&source, profile, |view, inherited| {
                let name = source.semantic_ssa().source_semantic().functions()[0]
                    .kernel_entry()
                    .unwrap()
                    .export_symbol();
                let name = std::str::from_utf8(name.as_bytes()).unwrap();
                let exact = candidate(name, 1, false);
                let mut entries =
                    view.output().module().functions.iter().filter(|function| {
                        function.role == fe2o3_kernel_ir::FunctionRole::KernelEntry
                    });
                let output = entries.next().expect("source kernel entry");
                assert!(entries.next().is_none());
                let body = output.body.as_ref().unwrap();
                let mut operations = 0;
                let mut effects = 0;
                let mut synchronizations = 0;
                for block in &body.blocks {
                    for operation in &block.operations {
                        operations += 1;
                        assert!(!matches!(
                            &operation.kind,
                            fe2o3_kernel_ir::OperationKind::Matrix(_)
                        ));
                        operation
                            .try_visit_local_memory_effects_v1(|effect| {
                                effects += 1;
                                if matches!(
                                    effect,
                                    fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Synchronize { .. }
                                ) {
                                    synchronizations += 1;
                                }
                                Ok::<(), std::convert::Infallible>(())
                            })
                            .unwrap();
                    }
                }
                assert_eq!(synchronizations, 1);
                let ranked_blocks = exact.kernel().blocks().len();
                let ranked_operations: usize = exact
                    .kernel()
                    .blocks()
                    .iter()
                    .map(|b| b.operations().len())
                    .sum();
                let sync_record = std::mem::size_of::<Option<u8>>() + 3;
                let tensor_alignment =
                    std::mem::align_of::<fe2o3_kernel_ir::TensorLayoutContractV1>()
                        .max(std::mem::align_of::<u32>());
                let tensor_record =
                    (std::mem::size_of::<fe2o3_kernel_ir::TensorLayoutContractV1>() + 4 + 1)
                        .next_multiple_of(tensor_alignment);
                // Entry8, two names, alias3/fetch5, two target scans, target1,
                // headers3, two first-insert2, sync length/record, tensor length.
                let fixed = 8
                    + (name.len() + 1)
                    + 3
                    + 5
                    + (output.id.as_str().len() + 1)
                    + target_scan_work(&view.output().module().required_capabilities)
                    + target_scan_work(&output.required_capabilities)
                    + 1
                    + 3
                    + 4
                    + 1
                    + (1 + sync_record)
                    + 1;
                let required = fixed
                    + 3 * body.blocks.len()
                    + 2 * ranked_blocks
                    + (26 + tensor_record) * operations
                    + (10 + tensor_record) * ranked_operations
                    + effects;
                let headers = 4 * std::mem::size_of::<Vec<()>>();
                let scratch = headers + 8 * sync_record;
                let floor = inherited.storage();
                for (headroom, storage) in [
                    (required, scratch),
                    (required - 1, scratch),
                    (required, scratch - 1),
                ] {
                    let mut work = Work::new(7 + headroom);
                    let mut query = Budget::new(&mut work, floor + storage);
                    query.charge_work(7).unwrap();
                    query.reserve_storage(floor).unwrap();
                    let result = view.check_ranked_synchronization_tensor_contracts(
                        ROOT, ROOT, &exact, &mut query,
                    );
                    assert_eq!(result.is_ok(), headroom == required && storage == scratch);
                    assert_eq!(query.storage(), floor);
                    if storage == scratch {
                        assert_eq!(query.work(), 7 + headroom);
                        assert_eq!(query.peak_storage(), floor + scratch);
                        drop(query);
                        assert_eq!(
                            work.failed_work(),
                            (headroom < required).then_some(7 + required)
                        );
                    } else {
                        assert!(query.peak_storage() >= floor + headers + 4 * sync_record);
                    }
                }
                let mut work = Work::new(100);
                let mut missing = Budget::new(&mut work, 0);
                assert!(matches!(
                    view.check_ranked_synchronization_tensor_contracts(
                        ROOT,
                        ROOT,
                        &exact,
                        &mut missing
                    ),
                    Err(ProductionSourceOutputErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                    ))
                ));
                assert_eq!(missing.work(), 4);
                assert_eq!(missing.storage(), 0);
            });
        }
    }

    include!("source_output_genuine_tensor_v1_tests.rs");
}

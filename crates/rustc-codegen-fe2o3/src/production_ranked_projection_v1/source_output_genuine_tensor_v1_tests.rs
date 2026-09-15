mod genuine_tensor_contract_tests_v1 {
    use super::*;
    include!("source_output_genuine_tensor_fixture_v1_tests.rs");

    fn tensor_candidate(
        name: &str,
        contracts: &[fe2o3_kernel_ir::TensorLayoutContractV1],
        convergence: dialect_kernel::TensorConvergenceAttr,
        active_lanes: u32,
    ) -> Result<
        fe2o3_pliron::ProductionRankedKernelLoweringInputV1,
        fe2o3_pliron::ProductionRankedCompileErrorV1,
    > {
        let mut operations = vec![ProductionRankedOperationV1::ExecutionLayout {
            grid_identity: 1,
            global_extents: [64, 1, 1],
            workgroup_extents: [64, 1, 1],
            subgroup_size: 64,
            full_physical_workgroups: true,
        }];
        operations.extend(contracts.iter().map(|contract| {
            ProductionRankedOperationV1::TensorLayout {
                contract: *contract,
                convergence,
                active_lanes,
                binding: None,
            }
        }));
        let kernel = ProductionRankedKernelV1::new(
            name,
            3,
            vec![ProductionRankedBlockV1::new(
                operations,
                ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        compile_ranked_kernel_for_lowering_v1(
            ProductionConstructionV1::ranked_kernel("genuine_tensor_contract_diagnostic", kernel)
                .unwrap(),
            ProductionSessionLimitsV1::default(),
        )
    }

    #[test]
    fn genuine_checked_view_load_mfma_store_source_retains_exact_output_tensor_contract() {
        let source = genuine_tensor_source_v1(TensorSourceFaultV1::None).unwrap();
        let function = &source.semantic_ssa().source_semantic().functions()[0];
        for (block, callable, operand) in [(4, 4, 1), (7, 6, 1), (8, 7, 0)] {
            let block = &function.blocks()[block];
            let SemanticStatementKindV1::Assign(borrow) = block.statements().last().unwrap().kind()
            else {
                panic!("lane consumer must have a fresh source borrow");
            };
            assert_eq!(borrow.destination(), &place(10, T_LANE_REF));
            assert!(matches!(
                borrow.value().kind(),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: source,
                } if source == &place(5, T_LANE)
            ));
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                panic!("lane borrow must feed its same-block intrinsic");
            };
            assert_eq!(call.callee().index(), callable);
            assert_eq!(call.arguments()[operand], value(10, T_LANE_REF));
        }
        let expected =
            fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64()
                .with_zero_filled_predicate_inputs();
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_global_view(&source, profile, |view, budget| {
                // O is exclusively the real checked optimizer owner. A rewrite
                // is not required for this operation-contract query fixture.
                for module in [source.executable().module(), view.output().module()] {
                    let mut matrices = 0;
                    let mut reads = 0;
                    let mut writes = 0;
                    for operation in module
                        .functions
                        .iter()
                        .filter(|f| f.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
                        .filter_map(|f| f.body.as_ref())
                        .flat_map(|body| &body.blocks)
                        .flat_map(|block| &block.operations)
                    {
                        if let fe2o3_kernel_ir::OperationKind::Matrix(matrix) = &operation.kind {
                            assert!(matches!(
                                &matrix.kind,
                                fe2o3_kernel_ir::MatrixOperationKind::MultiplyAccumulate { .. }
                            ));
                            assert_eq!(matrix.tensor_layout, Some(expected));
                            assert_eq!(matrix.active_lanes, 64);
                            assert_eq!(
                                matrix.convergence.scope(),
                                fe2o3_kernel_ir::SynchronizationScope::Subgroup
                            );
                            assert_eq!(operation.results.len(), 4);
                            matrices += 1;
                        }
                        operation
                            .try_visit_local_memory_effects_v1(|effect| {
                                match effect {
                                    fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Read(
                                        fe2o3_kernel_ir::AddressSpace::Global,
                                    ) => reads += 1,
                                    fe2o3_kernel_ir::KirLocalMemoryEffectRefV1::Write(
                                        fe2o3_kernel_ir::AddressSpace::Global,
                                    ) => writes += 1,
                                    _ => {}
                                }
                                Ok::<(), std::convert::Infallible>(())
                            })
                            .unwrap();
                    }
                    assert_eq!((matrices, reads, writes), (1, 8, 1));
                }
                let name = source.semantic_ssa().source_semantic().functions()[0]
                    .kernel_entry()
                    .unwrap()
                    .export_symbol();
                let name = std::str::from_utf8(name.as_bytes()).unwrap();
                let subgroup = dialect_kernel::TensorConvergenceAttr::UniformSubgroup;
                let exact = tensor_candidate(name, &[expected], subgroup, 64).unwrap();
                let floor = budget.storage();
                // Contract-only candidate. Actual generated operand transport,
                // ordered effects and complete root attachment are independent.
                view.check_ranked_synchronization_tensor_contracts(ROOT, ROOT, &exact, budget)
                    .unwrap();
                let substituted = [
                    expected.with_a_lds_xor4(),
                    fe2o3_kernel_ir::TensorLayoutContractV1::gfx942_mfma_bf16_f32_m16n16k16_wave64(),
                    fe2o3_kernel_ir::TensorLayoutContractV1::gfx950_scaled_mfma_fp4_e2m1_f32_m16n16k128_wave64()
                        .with_zero_filled_predicate_inputs(),
                ];
                for changed in substituted {
                    let wrong = tensor_candidate(name, &[changed], subgroup, 64).unwrap();
                    assert!(matches!(
                        view.check_ranked_synchronization_tensor_contracts(
                            ROOT, ROOT, &wrong, budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "output tensor contract mismatch"
                        ))
                    ));
                }
                for (convergence, active_lanes) in [
                    (dialect_kernel::TensorConvergenceAttr::UniformWorkgroup, 64),
                    (dialect_kernel::TensorConvergenceAttr::Divergent, 64),
                    (dialect_kernel::TensorConvergenceAttr::Opaque, 64),
                    (subgroup, 32),
                ] {
                    // Invalid convergence cannot enter the checked lowering
                    // input. Preserve that earlier proof gate, not a forged
                    // candidate that bypasses it to reach the O query.
                    let error =
                        tensor_candidate(name, &[expected], convergence, active_lanes).unwrap_err();
                    let fe2o3_pliron::ProductionRankedCompileErrorV1::Session(
                        fe2o3_pliron::ProductionSessionErrorV1::RankedTensorLayout(error),
                    ) = error
                    else {
                        panic!("wrong tensor input reached an unrelated gate: {error:?}");
                    };
                    assert!(error.report().findings().iter().any(|finding| {
                        if active_lanes == 32 {
                            matches!(
                                finding,
                                fe2o3_pliron::PlironTensorLayoutFindingV1::ActiveLaneMismatch {
                                    block: 0,
                                    operation: 1,
                                    expected: 64,
                                    actual: 32,
                                }
                            )
                        } else {
                            matches!(finding,
                                fe2o3_pliron::PlironTensorLayoutFindingV1::ConvergenceMismatch {
                                    block: 0, operation: 1, actual,
                                } if *actual == convergence)
                        }
                    }));
                    assert_eq!(budget.storage(), floor);
                }
                for contracts in [vec![], vec![expected, expected]] {
                    let wrong = tensor_candidate(name, &contracts, subgroup, 64).unwrap();
                    assert!(matches!(
                        view.check_ranked_synchronization_tensor_contracts(
                            ROOT, ROOT, &wrong, budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "output tensor contract mismatch"
                        ))
                    ));
                }
                assert_eq!(budget.storage(), floor);
                assert!(!view.grants_authority());
                view.check_ranked_synchronization_tensor_contracts(ROOT, ROOT, &exact, budget)
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }

    #[test]
    fn tensor_source_refuses_removed_issuers_at_exact_root_closure_gate() {
        use fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1;

        for (fault, expected) in [
            (TensorSourceFaultV1::UnissuedLane, 2),
            (TensorSourceFaultV1::UnissuedContext, 1),
            (TensorSourceFaultV1::MissingRhsProducer, 6),
        ] {
            let error = genuine_tensor_source_v1(fault).unwrap_err();
            // Removing the source issuer leaves its authenticated declaration
            // outside the root closure. Do not bypass this earlier gate.
            assert!(
                matches!(error.downcast_ref::<SemanticMirErrorV1>(),
                    Some(SemanticMirErrorV1::CallableOutsideRootClosure { callable })
                        if callable.index() == expected),
                "{fault:?} reached the wrong source gate: {error:?}"
            );
        }
    }

    #[test]
    fn tensor_source_refuses_forked_lane_borrow_and_non_dominating_result() {
        use fe2o3_lower_mir_kernel::{
            ProductionPreRankedKirErrorV1 as Pre, ProductionSemanticKirErrorV1 as Kir,
        };
        let error = genuine_tensor_source_v1(TensorSourceFaultV1::ReusedLaneBorrow).unwrap_err();
        assert!(
            matches!(
                error.downcast_ref::<Pre>(),
                Some(Pre::Lowering(Kir::Unsupported {
                    function: 0,
                    block: Some(4),
                    statement: None,
                    detail: "typed matrix load lane",
                }))
            ),
            "forked lane borrow reached the wrong source gate: {error:?}"
        );
        for (fault, expected_statement) in [
            (TensorSourceFaultV1::MissingResultSuccess, 0),
            (TensorSourceFaultV1::WrongResultSuccess, 0),
            (TensorSourceFaultV1::SharedResultSuccessTarget, 0),
            (TensorSourceFaultV1::OverwrittenResultAfterSuccess, 0),
            (TensorSourceFaultV1::AliasedResultAfterSuccess, 1),
        ] {
            let error = genuine_tensor_source_v1(fault).unwrap_err();
            assert!(
                matches!(
                    error.downcast_ref::<Pre>(),
                    Some(Pre::Lowering(Kir::Unsupported {
                        function: 0,
                        block: Some(4),
                        statement: Some(statement),
                        detail: "enum downcast lacks an authenticated variant",
                    })) if *statement == expected_statement
                ),
                "{fault:?} reached the wrong source gate: {error:?}"
            );
        }
    }
}

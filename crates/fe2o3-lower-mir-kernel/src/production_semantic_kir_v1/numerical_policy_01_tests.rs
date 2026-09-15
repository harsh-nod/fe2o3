mod numerical_policy_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn fixture() -> (
        ProductionSemanticMirOwnerV1,
        SemanticExecutionCapabilityContractV1,
    ) {
        fixture_with_policy_move(true)
    }

    pub(super) fn fixture_with_policy_move(
        transport_policy: bool,
    ) -> (
        ProductionSemanticMirOwnerV1,
        SemanticExecutionCapabilityContractV1,
    ) {
        let original_owner = noop_semantic_owner(&["numerical_entry"]);
        let original = &original_owner.semantic().functions()[0];
        let source = SemanticSourceProvenanceV1::unavailable();
        let unit = SemanticTypeIdV1::from_index(0);
        let context = SemanticTypeIdV1::from_index(1);
        let reference = SemanticTypeIdV1::from_index(2);
        let capability = SemanticTypeIdV1::from_index(3);
        let id = |tag| SemanticTypeIdentityV1::from_sha256([tag; 32]);
        let zst = |tag| {
            SemanticTypeDeclV1::new(
                id(tag),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(0),
                    1,
                    SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
            )
        };
        let types = vec![
            unit_type(),
            zst(10),
            SemanticTypeDeclV1::new(
                id(11),
                SemanticLayoutIdentityV1::from_sha256([11; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(8),
                    8,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        context,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
            .with_rustc_abi_properties(
                SemanticTypeAbiPropertiesV1::new(false, false)
                    .with_rustc_layout_is_noundef(true)
                    .with_scalar_pointee_info(
                        Some(
                            SemanticAbiPointeeInfoV1::new(
                                SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                                0,
                                1,
                            )
                            .unwrap(),
                        ),
                        None,
                    ),
            ),
            zst(12),
        ];
        let provenance = SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(0),
            original.kernel_entry().unwrap().kernel_binding_identity(),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256([4; 32]),
            id(1),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256([2; 32]),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256([3; 32]),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256([7; 32]),
        )
        .unwrap();
        let contract = SemanticExecutionCapabilityContractV1::new_kernel_scoped(
            SemanticExecutionCapabilityOperationV1::NumericalPolicyIssue {
                context: reference,
                capability,
                policy: id(13),
            },
            SemanticExecutionCapabilitySignatureV1::new(&[reference], capability).unwrap(),
            provenance,
            SemanticFunctionIdentityV1::from_sha256([51; 32]),
        )
        .unwrap();
        let intrinsic = |tag, inputs: Vec<SemanticTypeIdV1>, output, operation| {
            let arguments = inputs
                .iter()
                .map(|ty| {
                    SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                        *ty,
                        SemanticAbiPassModeV1::Direct(
                            SemanticAbiValueAttributesV1::new(
                                SemanticAbiRegularAttributesV1::new(
                                    true,
                                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                                    true,
                                    true,
                                    false,
                                    true,
                                ),
                                SemanticAbiExtensionV1::None,
                                0,
                                None,
                            )
                            .unwrap(),
                        ),
                    ))
                })
                .collect();
            let count = inputs.len();
            let abi = SemanticFunctionAbiV1::from_rustc_with_source_signature(
                SemanticAbiIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([250; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                count as u32,
                inputs,
                output,
                arguments,
                SemanticAbiValueV1::new(output, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap()
            .with_source_argument_ownership(vec![
                SemanticSourceArgumentOwnershipV1::SharedBorrow;
                count
            ])
            .unwrap();
            SemanticCallableDeclV1::CompilerIntrinsic {
                binding: SemanticNonBodyCallableBindingV1::new(
                    SemanticFunctionIdentityV1::from_sha256([tag; 32]),
                    SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
                    SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
                    SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
                    SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
                    source,
                    abi,
                ),
                operation,
                operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
            }
        };
        let callables = vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            intrinsic(
                50,
                vec![],
                context,
                SemanticCompilerIntrinsicOperationV1::KernelContextIssue { context },
            ),
            intrinsic(
                51,
                vec![reference],
                capability,
                SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
            ),
        ];
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let call = |callee, arguments, local, ty, target| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        place(local, ty),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(target),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let blocks = vec![
            block(60, vec![], call(1, vec![], 1, context, 1)),
            block(
                61,
                vec![SemanticStatementV1::new(
                    source,
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(2, reference),
                        SemanticRvalueV1::new(
                            reference,
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: place(1, context),
                            },
                        ),
                    )),
                )],
                call(
                    2,
                    vec![SemanticOperandV1::Copy(place(2, reference))],
                    3,
                    capability,
                    2,
                ),
            ),
            block(
                62,
                if transport_policy {
                    vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            place(4, capability),
                            SemanticRvalueV1::new(
                                capability,
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(
                                    3, capability,
                                ))),
                            ),
                        )),
                    )]
                } else {
                    vec![]
                },
                SemanticTerminatorKindV1::Return,
            ),
        ];
        let function = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source,
            original.abi().clone(),
            [unit, context, reference, capability, capability]
                .into_iter()
                .enumerate()
                .map(|(i, ty)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([70 + i as u8; 32]),
                        ty,
                        if i == 0 {
                            SemanticLocalRoleV1::Return
                        } else {
                            SemanticLocalRoleV1::Temporary
                        },
                        source,
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            types,
            vec![],
            vec![],
            vec![],
            vec![function],
            callables,
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_exact_v18(SemanticMirLimitsV1::default())
        .unwrap();
        (
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            contract,
        )
    }

    #[test]
    fn numerical_policy_lowers_through_semantic_ssa_to_a_retained_typed_kir_result() {
        let (source, source_contract) = fixture();
        let owner = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            source,
            ProductionSemanticKirLimitsV1::default(),
            vec![ProductionKernelContextLoweringInputV1::new(
                SemanticFunctionIdV1::from_index(0),
                [4; 32],
                [1; 32],
                [2; 32],
                [3; 32],
                [7; 32],
            )],
        )
        .unwrap();
        let operations = owner.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .filter_map(|op| match &op.kind {
                OperationKind::ExecutionCapability(contract) => Some((op, contract)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [(operation, contract)] = operations.as_slice() else {
            panic!("one retained policy issuance")
        };
        assert!(matches!(
            contract.operation,
            ExecutionCapabilityOperationV1::NumericalPolicyIssue {
                mode: fe2o3_kernel_ir::NumericalModeV1::StrictIeee,
                ..
            }
        ));
        let [
            ValueDef {
                ty: Type::ExecutionCapability(authority),
                ..
            },
        ] = operation.results.as_slice()
        else {
            panic!("typed authority")
        };
        assert!(numerical_policy_transport_matches_v1(
            source_contract,
            authority
        ));
        assert_eq!(
            contract.obligations.bits(),
            source_contract.obligations().bits()
        );
        assert_eq!(
            contract.source.operation,
            *source_contract.source_identity().as_bytes()
        );
        owner.verify_equivalence().unwrap();
    }

    #[test]
    fn unused_source_numerical_policy_reaches_analyzer_simulator_and_physical_lowering() {
        use fe2o3_amdgcn_model::{
            ProductionTargetLaunchEvidenceV13, lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1,
        };
        use fe2o3_kernel_analysis::{
            KernelCheckStatusV1, analyze_execution_capability_final_graph_v1,
        };
        use fe2o3_kir_sim::{
            AdmittedSimulationModuleV1, SimulationExecutionCapabilityFamilyV13, SimulationLimitsV1,
            SimulationRequestV1, SimulationTargetV1,
        };

        let (source, _) = fixture_with_policy_move(false);
        let owner = ProductionSemanticKirOwnerV1::try_lower_with_kernel_contexts(
            source,
            ProductionSemanticKirLimitsV1::default(),
            vec![ProductionKernelContextLoweringInputV1::new(
                SemanticFunctionIdV1::from_index(0),
                [4; 32],
                [1; 32],
                [2; 32],
                [3; 32],
                [7; 32],
            )],
        )
        .unwrap();
        owner.verify_equivalence().unwrap();
        let module = owner.module();
        let canonical =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let analysis = analyze_execution_capability_final_graph_v1(&canonical, module, 7).unwrap();
        assert_eq!(analysis.status(), KernelCheckStatusV1::Clean);
        assert!(!analysis.grants_proof_machine_artifact_or_launch_authority());

        let evidence =
            ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 7).unwrap();
        let mut baseline = module.clone();
        for block in &mut baseline.functions[0].body.as_mut().unwrap().blocks {
            block.operations.retain(|op| !matches!(&op.kind,
                OperationKind::ExecutionCapability(contract)
                    if matches!(contract.operation, ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. })));
        }
        let baseline =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV13::from_module(baseline).unwrap();
        let baseline_evidence =
            ProductionTargetLaunchEvidenceV13::for_static_launches(&baseline, 7).unwrap();
        for profile in [
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
            fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
        ] {
            let lowered = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
                &canonical, 7, &evidence, profile,
            )
            .unwrap();
            let expected = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
                &baseline,
                7,
                &baseline_evidence,
                profile,
            )
            .unwrap();
            assert_eq!(lowered.llvm_ir(), expected.llvm_ir());
            assert!(!lowered.has_complete_operational_translation_derivation());
            assert!(!lowered.grants_load_authority());
            assert!(!lowered.grants_launch_authority());
        }

        let expected_identity = *canonical.identity().digest();
        let admitted =
            AdmittedSimulationModuleV1::admit_v13(canonical, SimulationLimitsV1::default())
                .unwrap();
        assert_eq!(admitted.identity().digest(), &expected_identity);
        assert!(!admitted.grants_execution_authority());
        assert!(
            admitted
                .capability_projection_receipt_v13()
                .unwrap()
                .coordinates()
                .iter()
                .any(|coordinate| coordinate.execution_family()
                    == Some(SimulationExecutionCapabilityFamilyV13::NumericalPolicyIssue))
        );
        let kernel = &module.kernels[0];
        let execution = admitted
            .simulate(
                &SimulationRequestV1::new(kernel.id.as_str(), [64, 1, 1], [64, 1, 1], vec![]),
                SimulationTargetV1::amdgpu_64(),
                SimulationLimitsV1::default(),
            )
            .unwrap();
        assert_eq!(execution.invocations_executed(), 64);
    }

    #[test]
    fn numerical_policy_ssa_rejects_empty_aggregate_wrong_nominal_policy_and_target() {
        let (owner, contract) = fixture();
        let types = owner.semantic().types();
        let context = KernelContextTypeV1::new("numerical_entry", [1; 32], [2; 32], [3; 32]);
        let source_type = execution_type_identity_v1(types, contract.signature().output()).unwrap();
        let binding = SemanticPromotedBindingV1::NumericalPolicy {
            contract,
            source_type,
        };
        assert!(
            binding
                .transport_values(&SemanticValueBindingV1::Aggregate(vec![]))
                .is_err()
        );
        let ty = numerical_policy_transport_type_v1(
            types,
            contract.signature().output(),
            contract,
            Some(&context),
        )
        .unwrap();
        let Type::ExecutionCapability(authority) = ty else {
            unreachable!()
        };
        for mutate in [
            (|ty: &mut ExecutionCapabilityTypeV1| {
                ty.source_type = ExecutionTypeIdentityV1::new([99; 32])
            }) as fn(&mut ExecutionCapabilityTypeV1),
            |ty| ty.provenance.target_brand = [99; 32],
            |ty| {
                ty.role = ExecutionCapabilityRoleV1::NumericalPolicy {
                    policy: ExecutionTypeIdentityV1::new([99; 32]),
                    mode: fe2o3_kernel_ir::NumericalModeV1::StrictIeee,
                }
            },
        ] {
            let mut bad = authority.clone();
            mutate(&mut bad);
            assert!(
                binding
                    .transport_values(&SemanticValueBindingV1::Value {
                        id: ValueId(0),
                        ty: Type::ExecutionCapability(bad)
                    })
                    .is_err()
            );
        }
    }
}

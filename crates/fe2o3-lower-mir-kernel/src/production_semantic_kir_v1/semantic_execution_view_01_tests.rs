pub(super) mod checked_execution_view_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiCastV1, SemanticAbiRegisterV1, SemanticAbiUniformV1,
        SemanticAbiValueAttributesV1, SemanticDirectEnumEncodingV1, SemanticEnumLayoutV1,
        SemanticEnumVariantLayoutV1,
    };

    fn lower_helper_calls(calls: u8) -> ProductionSemanticKirOwnerV1 {
        ProductionSemanticKirOwnerV1::try_lower(
            helper_closure_semantic_owner_with_calls(calls),
            ProductionSemanticKirLimitsV1::default(),
        )
        .expect("checked unit helper calls lower without a second semantic document")
    }

    #[test]
    fn checked_expansion_lowers_one_physical_root_and_retains_original_source() {
        let source = helper_closure_semantic_owner();
        let source_bytes = source.semantic().canonical_encoding().to_vec();
        let source_hash = *source.semantic().semantic_sha256().as_bytes();
        let original_root = source.semantic().functions()[0].clone();
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(
            lowered.semantic().semantic().canonical_encoding(),
            source_bytes
        );
        assert_eq!(lowered.correspondence().semantic_sha256(), &source_hash);
        assert!(lowered.has_expanded_calls());
        assert_eq!(lowered.correspondence().function_count(), 2);
        assert_eq!(lowered.correspondence().lowered_functions().len(), 1);
        assert_eq!(lowered.module().functions.len(), 1);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = lowered.semantic_ssa.execution_view_for_root(root).unwrap();
        assert_eq!(view.body().abi(), original_root.abi());
        assert_eq!(view.body().kernel_entry(), original_root.kernel_entry());
        assert_eq!(view.instances().len(), 2);
        assert_eq!(
            view.instances()[0].function_identity(),
            original_root.identity()
        );
        assert_eq!(
            lowered.execution_expansion_identity(),
            Some(lowered.semantic_ssa.execution_expansion().identity()),
        );
        assert!(lowered.module().functions.iter().all(|function| {
            function.body.as_ref().is_some_and(|body| {
                body.blocks.iter().all(|block| {
                    block
                        .operations
                        .iter()
                        .all(|operation| !matches!(operation.kind, OperationKind::Call { .. }))
                })
            })
        }));
        lowered.verify_equivalence().unwrap();
    }

    #[test]
    fn repeated_calls_keep_distinct_execution_blocks_and_original_call_instances() {
        let lowered = lower_helper_calls(2);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = lowered.semantic_ssa.execution_view_for_root(root).unwrap();
        assert_eq!(view.instances().len(), 3);
        assert_eq!(
            view.instances()[1].function(),
            view.instances()[2].function()
        );
        assert_ne!(
            view.instances()[1].call_block(),
            view.instances()[2].call_block()
        );
        let helper_blocks = view
            .block_origins()
            .iter()
            .enumerate()
            .filter(|(_, origin)| origin.function() != root)
            .collect::<Vec<_>>();
        assert_eq!(helper_blocks.len(), 2);
        assert_eq!(helper_blocks[0].1.block(), helper_blocks[1].1.block());
        assert_ne!(helper_blocks[0].1.instance(), helper_blocks[1].1.instance());
        for (execution_block, _) in helper_blocks {
            assert!(lowered.correspondence().blocks().iter().any(|record| {
                record.correspondence_owner() == root
                    && record.semantic_function() == root
                    && record.semantic_block().index() as usize == execution_block
            }));
        }
        lowered.verify_equivalence().unwrap();
    }

    #[test]
    fn original_coordinate_induction_cannot_export_expanded_correspondence_v4() {
        let lowered = lower_helper_calls(1);
        let original_report = fe2o3_mir_model::analyze_semantic_u32_induction_no_overflow_v1(
            lowered.semantic().semantic(),
            SemanticFunctionIdV1::from_index(0),
        )
        .unwrap();
        assert!(original_report.execution_view_identity().is_none());
        assert!(matches!(
            crate::InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(
                &lowered,
                &original_report,
            ),
            Err(crate::ProductionCorrespondenceEvidenceErrorV4::ExecutionViewUnsupported),
        ));
    }

    #[test]
    fn replay_rejects_stripped_or_substituted_expansion_identity() {
        let mut lowered = lower_helper_calls(1);
        let actual = lowered.correspondence.execution_expansion_identity;
        for substituted in [None, Some([0xa3; 32])] {
            assert_ne!(actual, substituted);
            lowered.correspondence.execution_expansion_identity = substituted;
            assert!(matches!(
                lowered.verify_equivalence(),
                Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
            ));
        }
        lowered.correspondence.execution_expansion_identity = actual;
        lowered.verify_equivalence().unwrap();
    }

    #[test]
    fn replay_rejects_relabeling_an_expanded_block_as_an_original_block() {
        let mut lowered = lower_helper_calls(1);
        let root = SemanticFunctionIdV1::from_index(0);
        let view = lowered.semantic_ssa.execution_view_for_root(root).unwrap();
        let (expanded_block, origin) = view
            .block_origins()
            .iter()
            .enumerate()
            .find(|(_, origin)| origin.function() != root)
            .unwrap();
        let original_block = origin.block();
        assert_ne!(expanded_block as u32, original_block.index());
        lowered
            .correspondence
            .blocks
            .iter_mut()
            .find(|record| record.semantic_block().index() as usize == expanded_block)
            .unwrap()
            .semantic_block = original_block;
        assert!(matches!(
            lowered.verify_equivalence(),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }

    #[test]
    fn identity_view_preserves_the_original_coordinate_space() {
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            noop_semantic_owner(&["identity_root"]),
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        assert!(!lowered.has_expanded_calls());
        assert!(lowered.execution_expansion_identity().is_none());
        assert_eq!(lowered.correspondence().function_count(), 1);
        lowered.verify_equivalence().unwrap();
    }

    #[test]
    fn exact_transparent_result_wrapper_keeps_its_selected_body_and_root_geometry() {
        let source = transparent_result_wrapper_owner();
        let root = SemanticFunctionIdV1::from_index(0);
        let selected = SemanticFunctionIdV1::from_index(1);
        let selection = source
            .semantic()
            .select_kernel_body_for_root_v1(root)
            .unwrap();
        assert!(selection.has_transparent_result_wrapper());
        assert_eq!(selection.body(), selected);
        let lowered = ProductionSemanticKirOwnerV1::try_lower(
            source,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        assert!(!lowered.has_expanded_calls());
        assert_eq!(lowered.correspondence().function_count(), 2);
        assert_eq!(
            lowered.correspondence().lowered_functions()[0].correspondence_owner(),
            root
        );
        assert_eq!(
            lowered.correspondence().lowered_functions()[0].semantic_function(),
            selected
        );
        assert_eq!(
            lowered
                .semantic_ssa
                .execution_plan_for_root(root)
                .unwrap()
                .function(),
            selected
        );
        assert_eq!(
            lowered.module().kernels[0].workgroup_size,
            Some(WorkgroupSize::new(64, 1, 1))
        );
        lowered.verify_equivalence().unwrap();
    }

    pub(in crate::production_semantic_kir_v1) fn transparent_result_wrapper_owner()
    -> ProductionSemanticMirOwnerV1 {
        let baseline = helper_closure_semantic_owner();
        let source = baseline.semantic();
        let provenance = SemanticSourceProvenanceV1::unavailable();
        let unit = SemanticTypeIdV1::from_index(0);
        let discriminant = SemanticTypeIdV1::from_index(1);
        let result = SemanticTypeIdV1::from_index(2);
        let variants = (0..2)
            .map(|index| {
                SemanticEnumVariantLayoutV1::from_rustc(
                    index,
                    8,
                    4,
                    SemanticFieldsShapeV1::arbitrary(vec![4], vec![0]).unwrap(),
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    4,
                    u64::from(index) + 100,
                    SemanticAggregateLayoutV1::new(vec![4], vec![]).unwrap(),
                )
                .unwrap()
            })
            .collect();
        let result_type = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([91; 32]),
            SemanticLayoutIdentityV1::from_sha256([92; 32]),
            SemanticTypeLayoutV1::enum_layout(
                8,
                4,
                SemanticEnumLayoutV1::new(
                    variants,
                    SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                        0,
                        0,
                        SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::integer(false, 32, 4),
                            SemanticScalarValidityRangeV1::new(0, 1),
                        ),
                    )),
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::enum_type(
                discriminant,
                (0..2)
                    .map(|index| {
                        SemanticEnumVariantV1::new(
                            index,
                            SemanticAggregateTypeV1::new(vec![if index == 0 {
                                unit
                            } else {
                                discriminant
                            }])
                            .unwrap(),
                        )
                    })
                    .collect(),
            )
            .unwrap(),
        );
        let result_mode = SemanticAbiPassModeV1::Cast {
            pad_i32: false,
            cast: SemanticAbiCastV1::new(
                [None; 8],
                None,
                SemanticAbiUniformV1::new(
                    SemanticAbiRegisterV1::new(SemanticAbiRegisterKindV1::Integer, 8).unwrap(),
                    8,
                )
                .unwrap(),
                SemanticAbiValueAttributesV1::plain(),
            ),
        };
        let result_local = |role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([208; 32]),
                result,
                role,
                provenance,
            )
        };
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                provenance,
                statements,
                SemanticTerminatorV1::new(provenance, terminator),
            )
            .unwrap()
        };
        let mut functions = Vec::new();
        for (index, original) in source.functions().iter().enumerate() {
            let (abi, locals, blocks) = if index == 0 {
                (
                    original.abi().clone(),
                    vec![
                        original.locals()[0].clone(),
                        result_local(SemanticLocalRoleV1::Temporary),
                    ],
                    vec![
                        block(
                            94,
                            vec![],
                            SemanticTerminatorKindV1::Call(
                                SemanticDirectCallV1::new(
                                    SemanticFunctionIdV1::from_index(1),
                                    vec![],
                                    Some(SemanticCallDestinationV1::new(
                                        SemanticPlaceV1::new(
                                            SemanticLocalIdV1::from_index(1),
                                            vec![],
                                            result,
                                        )
                                        .unwrap(),
                                        SemanticControlFlowEdgeV1::new(
                                            SemanticEdgeRoleV1::CallReturn,
                                            SemanticBlockIdV1::from_index(1),
                                        ),
                                    )),
                                    SemanticUnwindActionV1::Unreachable,
                                )
                                .unwrap(),
                            ),
                        ),
                        block(95, vec![], SemanticTerminatorKindV1::Return),
                    ],
                )
            } else {
                (
                    SemanticFunctionAbiV1::from_rustc(
                        SemanticAbiIdentityV1::from_sha256([96; 32]),
                        source.target().identity(),
                        SemanticCanonAbiV1::Rust,
                        SemanticExternAbiV1::Rust,
                        false,
                        false,
                        0,
                        vec![],
                        SemanticAbiValueV1::new(result, result_mode.clone()),
                    )
                    .unwrap(),
                    vec![result_local(SemanticLocalRoleV1::Return)],
                    vec![block(
                        97,
                        vec![SemanticStatementV1::new(
                            provenance,
                            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                                SemanticPlaceV1::new(
                                    SemanticLocalIdV1::from_index(0),
                                    vec![],
                                    result,
                                )
                                .unwrap(),
                                SemanticRvalueV1::new(
                                    result,
                                    SemanticRvalueKindV1::aggregate(
                                        SemanticAggregateKindV1::EnumVariant(0),
                                        vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                                            unit,
                                            SemanticConstantValueV1::ZeroSized,
                                        ))],
                                    )
                                    .unwrap(),
                                ),
                            )),
                        )],
                        SemanticTerminatorKindV1::Return,
                    )],
                )
            };
            let mut function = SemanticFunctionDeclV1::new(
                original.identity(),
                original.role(),
                original.item_definition_identity(),
                original.monomorphization_identity(),
                original.generic_type_arguments_identity(),
                original.const_generic_arguments_identity(),
                original.source(),
                abi,
                locals,
                original.entry(),
                blocks,
            )
            .unwrap();
            if let Some(entry) = original.kernel_entry() {
                function = function.with_kernel_entry(entry.clone());
            }
            functions.push(function);
        }
        let admitted = InertSemanticMirRequestV1::new(
            source.target().clone(),
            vec![
                unit_type(),
                plain_bit_scalar_type(
                    90,
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    },
                ),
                result_type,
            ],
            vec![],
            vec![],
            vec![],
            functions,
            source.roots().to_vec(),
        )
        .unwrap()
        .admit(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap()
    }

    #[test]
    fn ranked_and_kir_retain_the_same_checked_execution_owner() {
        let root = SemanticFunctionIdV1::from_index(0);
        let ssa = ProductionSemanticSsaOwnerV1::try_new(
            helper_closure_semantic_owner(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let identity = ssa.identity();
        let candidate = noop_ranked_root(root, "helper_root");
        validate_borrowed_ssa_ranked_semantic_projection_candidate_with_generated_effects_v1(
            &ssa,
            root,
            &candidate.lowering,
            &candidate.ranked_ir,
            &candidate.access_sources,
            &candidate.executable_effect_sources,
        )
        .unwrap();
        let roster = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_ssa_projection_roster_candidate(
            ssa, vec![candidate],
        ).unwrap();
        let lowered = ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
            roster,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap();
        assert_eq!(lowered.semantic_ssa_identity(), identity);
        assert!(lowered.has_expanded_calls());
        assert!(lowered.retains_mandatory_generic_checks());
        lowered.verify_equivalence().unwrap();
    }
}

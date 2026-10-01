#[cfg(test)]
mod scalar_enum_result_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiExtensionV1, SemanticAbiPointerCaptureV1, SemanticAbiRegularAttributesV1,
        SemanticAbiValueAttributesV1, SemanticDirectEnumEncodingV1, SemanticEnumEncodingV1,
        SemanticEnumLayoutV1, SemanticEnumVariantLayoutV1,
    };

    const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
    const U8: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
    const ENUM: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

    fn types() -> Vec<SemanticTypeDeclV1> {
        let declaration = |tag, layout, shape| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag + 1; 32]),
                layout,
                shape,
            )
        };
        let variant = |index, size, offsets: Vec<u64>| {
            let order = (0..offsets.len() as u32).collect();
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                size,
                4,
                SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        };
        let tag = SemanticBackendScalarV1::initialized(
            SemanticBackendPrimitiveV1::integer(false, 32, 4),
            SemanticScalarValidityRangeV1::new(3, 29),
        );
        vec![
            unit_type(),
            plain_bit_scalar_type(
                91,
                SemanticBackendPrimitiveV1::integer(false, 32, 4),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            plain_bit_scalar_type(
                93,
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 8,
                },
            ),
            declaration(
                95,
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    4,
                    SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U8]).unwrap()),
            ),
            declaration(
                97,
                SemanticTypeLayoutV1::with_exact_rustc_layout(
                    2,
                    1,
                    SemanticFieldsShapeV1::array(1, 2),
                    SemanticRustcVariantsV1::Single { index: 0 },
                    SemanticBackendReprV1::memory(true),
                    None,
                    false,
                    None,
                    1,
                    0,
                    SemanticTypeLayoutDetailsV1::None,
                )
                .unwrap(),
                SemanticTypeShapeV1::Array {
                    element: U8,
                    length: 2,
                },
            ),
            declaration(
                99,
                SemanticTypeLayoutV1::enum_layout(
                    16,
                    4,
                    SemanticEnumLayoutV1::new(
                        vec![
                            variant(0, 4, vec![]),
                            variant(1, 8, vec![4]),
                            variant(2, 16, vec![4, 12]),
                        ],
                        SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                            0, 0, tag,
                        )),
                    )
                    .unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Enum {
                    discriminant: U32,
                    variants: vec![
                        SemanticEnumVariantV1::new(
                            3,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            11,
                            SemanticAggregateTypeV1::new(vec![U32]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            29,
                            SemanticAggregateTypeV1::new(vec![
                                SemanticTypeIdV1::from_index(3),
                                SemanticTypeIdV1::from_index(4),
                            ])
                            .unwrap(),
                        ),
                    ]
                    .into_boxed_slice(),
                },
            ),
        ]
    }

    fn whole(local: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ENUM).unwrap()
    }

    fn scalar(bits: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits, 4).unwrap()),
        ))
    }

    fn fixture(mode: u8) -> SemanticFunctionDeclV1 {
        let source = SemanticSourceProvenanceV1::unavailable();
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let assign = |local, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    whole(local),
                    SemanticRvalueV1::new(ENUM, value),
                )),
            )
        };
        let constructor = |variant, operands| {
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(variant), operands)
                .unwrap()
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
        let mut last = vec![
            assign(
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(whole(1))),
            ),
            assign(
                0,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(whole(2))),
            ),
        ];
        if mode == 2 {
            last.push(assign(
                0,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(whole(2))),
            ));
        }
        if mode == 3 {
            last.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::SetDiscriminant {
                    place: whole(0),
                    variant_index: 0,
                },
            ));
        }
        let blocks = vec![
            block(
                101,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: scalar(0),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 1),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(
                102,
                vec![assign(1, constructor(0, vec![]))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                103,
                if mode == 1 {
                    vec![]
                } else {
                    vec![assign(1, constructor(1, vec![scalar(7)]))]
                },
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                104,
                last,
                if mode == 4 {
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3))
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ),
        ];
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([105; 32]),
            SemanticLayoutIdentityV1::from_sha256([106; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(
                ENUM,
                SemanticAbiPassModeV1::Indirect {
                    attributes: SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(
                            true,
                            Some(SemanticAbiPointerCaptureV1::CapturesNone),
                            true,
                            false,
                            false,
                            true,
                        ),
                        SemanticAbiExtensionV1::None,
                        16,
                        Some(4),
                    )
                    .unwrap(),
                    metadata_attributes: None,
                    on_stack: false,
                },
            ),
        )
        .unwrap();
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([165; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([108; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([109; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([110; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([111; 32]),
            source,
            abi,
            (0..3)
                .map(|local| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([112 + local; 32]),
                        ENUM,
                        if local == 0 {
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
    }

    fn ssa(
        function: &SemanticFunctionDeclV1,
        types: &[SemanticTypeDeclV1],
    ) -> ProductionSemanticSsaFunctionPlanV1 {
        plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            function,
            types,
            &[],
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    fn values(types: &[SemanticTypeDeclV1]) -> Vec<ValueDef> {
        scalar_enum_result_shape_v1(types, ENUM)
            .unwrap()
            .components
            .iter()
            .enumerate()
            .map(|(index, component)| {
                ValueDef::new(ValueId(index as u32 + 1), component.kernel_type.clone())
            })
            .collect()
    }

    fn owner() -> ProductionSemanticSsaOwnerV1 {
        let seed = scalar_transmute_semantic_owner();
        let semantic = seed.semantic();
        let root = &semantic.functions()[0];
        let source = root.source();
        let statement = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(2), vec![], U32).unwrap(),
                SemanticRvalueV1::new(U32, SemanticRvalueKindV1::Discriminant(whole(1))),
            )),
        );
        let blocks = vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([161; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(
                    source,
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(1),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                whole(1),
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
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([162; 32]),
                source,
                vec![statement],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ];
        let root = SemanticFunctionDeclV1::new(
            root.identity(),
            root.role(),
            root.item_definition_identity(),
            root.monomorphization_identity(),
            root.generic_type_arguments_identity(),
            root.const_generic_arguments_identity(),
            source,
            root.abi().clone(),
            vec![
                root.locals()[0].clone(),
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([163; 32]),
                    ENUM,
                    SemanticLocalRoleV1::Temporary,
                    source,
                ),
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([164; 32]),
                    U32,
                    SemanticLocalRoleV1::Temporary,
                    source,
                ),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
        let semantic = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            types(),
            vec![],
            vec![],
            vec![],
            vec![root, fixture(0)],
            vec![
                SemanticCallableDeclV1::Defined {
                    function: SemanticFunctionIdV1::from_index(0),
                },
                SemanticCallableDeclV1::Defined {
                    function: SemanticFunctionIdV1::from_index(1),
                },
            ],
            vec![SemanticFunctionIdV1::from_index(0)],
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                semantic,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    fn roster(source: &ProductionSemanticSsaOwnerV1) -> crate::ProductionSourceLaunchRosterV1 {
        let semantic = source.source_semantic();
        let entry = semantic.functions()[0].kernel_entry().unwrap();
        crate::ProductionSourceLaunchRosterV1::try_new(
            semantic,
            &[crate::ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap()
    }

    #[test]
    fn ordinary_enum_transport_remains_tag_only() {
        assert!(
            SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary)
                .uses_structural_enum_transport()
        );
        assert!(!SemanticPromotedTransportV1::ScalarEnumResult.uses_structural_enum_transport());
        assert!(SemanticPromotedTransportV1::ScalarEnumResult.tracks_enum_variant_v1());
        let types = types();
        let defs = values(&types);
        let binding = scalar_enum_result_binding_v1(&types, ENUM, &defs).unwrap();
        assert_eq!(
            binding.values().unwrap(),
            vec![(defs[0].id, defs[0].ty.clone())]
        );
    }

    #[test]
    fn logical_slots_preserve_semantic_tag_and_variant_field_leaf_order() {
        let shape = scalar_enum_result_shape_v1(&types(), ENUM).unwrap();
        assert_eq!(shape.components.len(), 6);
        assert_eq!(shape.components[0].slot, ScalarEnumResultSlotV1::Tag);
        assert_eq!(
            shape.components[0].kernel_type,
            Type::Scalar(ScalarType::U32)
        );
        assert_eq!(
            shape.components[1].slot,
            ScalarEnumResultSlotV1::Payload {
                variant: 1,
                field: 0,
                leaf: 0
            }
        );
        assert_eq!(
            shape.components[2].slot,
            ScalarEnumResultSlotV1::Payload {
                variant: 2,
                field: 0,
                leaf: 0
            }
        );
        assert_eq!(
            shape.components[3].slot,
            ScalarEnumResultSlotV1::Payload {
                variant: 2,
                field: 0,
                leaf: 1
            }
        );
        assert_eq!(
            shape.components[4].slot,
            ScalarEnumResultSlotV1::Payload {
                variant: 2,
                field: 1,
                leaf: 0
            }
        );
        assert_eq!(
            shape.components[5].slot,
            ScalarEnumResultSlotV1::Payload {
                variant: 2,
                field: 1,
                leaf: 1
            }
        );
    }

    #[test]
    fn full_result_vectors_round_trip_and_reject_type_arity_or_roster_changes() {
        let types = types();
        let defs = values(&types);
        let expected = defs.iter().map(|def| def.ty.clone()).collect::<Vec<_>>();
        let binding = scalar_enum_result_binding_v1(&types, ENUM, &defs).unwrap();
        assert_eq!(
            scalar_enum_result_values_v1(&types, ENUM, &binding, &expected).unwrap(),
            defs.iter()
                .map(|def| (def.id, def.ty.clone()))
                .collect::<Vec<_>>()
        );
        assert!(scalar_enum_result_binding_v1(&types, ENUM, &defs[..5]).is_err());
        let mut bad = defs.clone();
        bad[3].ty = Type::Scalar(ScalarType::U32);
        assert!(scalar_enum_result_binding_v1(&types, ENUM, &bad).is_err());
        let mut bad = binding.clone();
        let SemanticValueBindingV1::Enum { payloads, .. } = &mut bad else {
            unreachable!()
        };
        let fields = payloads.remove(&2).unwrap();
        payloads.insert(3, fields);
        assert!(scalar_enum_result_values_v1(&types, ENUM, &bad, &expected).is_err());
        let mut bad = scalar_enum_result_binding_v1(&types, ENUM, &defs).unwrap();
        let SemanticValueBindingV1::Enum { payloads, .. } = &mut bad else {
            unreachable!()
        };
        payloads.insert(3, vec![]);
        assert!(scalar_enum_result_values_v1(&types, ENUM, &bad, &expected).is_err());
        let mut bad = scalar_enum_result_binding_v1(&types, ENUM, &defs).unwrap();
        let SemanticValueBindingV1::Enum { payloads, .. } = &mut bad else {
            unreachable!()
        };
        let one = payloads.remove(&1).unwrap();
        payloads.insert(0, one);
        payloads.insert(1, vec![]);
        assert!(scalar_enum_result_values_v1(&types, ENUM, &bad, &expected).is_err());
        let mut bad = scalar_enum_result_binding_v1(&types, ENUM, &defs).unwrap();
        let SemanticValueBindingV1::Enum { payloads, .. } = &mut bad else {
            unreachable!()
        };
        payloads.get_mut(&1).unwrap()[0] = SemanticValueBindingV1::WaveLane {
            value: defs[1].id,
            wave: SemanticCurrentWaveV1::new(64),
        };
        assert!(scalar_enum_result_values_v1(&types, ENUM, &bad, &expected).is_err());
        let mut bad = binding;
        let SemanticValueBindingV1::Enum { payloads, .. } = &mut bad else {
            unreachable!()
        };
        payloads.remove(&1);
        assert!(scalar_enum_result_values_v1(&types, ENUM, &bad, &expected).is_err());
    }

    #[test]
    fn nested_enum_and_opaque_payloads_do_not_become_scalar_words() {
        for replacement in [
            SemanticTypeShapeV1::Opaque,
            types()[ENUM.index() as usize].shape().clone(),
        ] {
            let mut types = types();
            let old = &types[U8.index() as usize];
            types[U8.index() as usize] = SemanticTypeDeclV1::new(
                old.identity(),
                old.layout_identity(),
                old.layout().clone(),
                replacement,
            );
            assert_eq!(
                lower_scalar_type(&types, U32).unwrap(),
                Type::Scalar(ScalarType::U32)
            );
            assert!(scalar_enum_result_shape_v1(&types, ENUM).is_err());
        }
    }

    #[test]
    fn zero_sized_and_scalar_arrays_obey_structural_bound() {
        let mut types = types();
        let array = &types[4];
        types[4] = SemanticTypeDeclV1::new(
            array.identity(),
            array.layout_identity(),
            array.layout().clone(),
            SemanticTypeShapeV1::Array {
                element: SemanticTypeIdV1::from_index(0),
                length: MAX_SSA_VALUE_COMPONENTS_V1 as u64 + 1,
            },
        );
        assert!(scalar_enum_result_shape_v1(&types, ENUM).is_err());
    }

    #[test]
    fn constructor_join_and_copy_move_closure_selects_complete_result_transport() {
        let types = types();
        let function = fixture(0);
        let ssa = ssa(&function, &types);
        let locals = scalar_enum_result_locals_v1(
            &types,
            &[],
            &function,
            &ssa,
            usize::MAX,
            usize::MAX,
            None,
        )
        .unwrap();
        assert_eq!(locals, BTreeSet::from([0, 1, 2]));
        let option = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let plan = SemanticControlFlowSsaPlanV1::analyze(
            SemanticSsaTransportInputV1 {
                types: &types,
                callables: &[],
                function: &function,
                semantic_function: SemanticFunctionIdV1::from_index(0),
            },
            &ssa,
            &option,
            &BTreeMap::new(),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        for local in locals {
            assert_eq!(
                plan.promoted[&local].transport,
                SemanticPromotedTransportV1::ScalarEnumResult
            );
            assert_eq!(plan.promoted[&local].kernel_types.len(), 6);
        }
        assert_eq!(plan.live_in(3), [1]);
    }

    #[test]
    fn missing_join_input_and_move_reuse_fail_shared_ssa_initialization() {
        for mode in [1, 2] {
            assert!(
                plan_semantic_function_ssa_with_module_v1(
                    SemanticFunctionIdV1::from_index(0),
                    &fixture(mode),
                    &types(),
                    &[],
                    ProductionSemanticSsaLimitsV1::default(),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn tag_only_writes_and_cycles_do_not_opt_into_complete_transport() {
        for mode in [3, 4] {
            let function = fixture(mode);
            let types = types();
            let ssa = ssa(&function, &types);
            assert!(
                scalar_enum_result_locals_v1(
                    &types,
                    &[],
                    &function,
                    &ssa,
                    usize::MAX,
                    usize::MAX,
                    None
                )
                .is_err()
            );
        }
    }

    #[test]
    fn producer_analysis_enforces_both_work_and_storage_before_growth() {
        let types = types();
        let function = fixture(0);
        let ssa = ssa(&function, &types);
        for (work, storage) in [(0, usize::MAX), (usize::MAX, 0)] {
            assert!(matches!(
                scalar_enum_result_locals_v1(&types, &[], &function, &ssa, work, storage, None),
                Err(ProductionSemanticKirErrorV1::ResourceLimit { .. })
            ));
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        assert!(
            scalar_enum_result_locals_v1(
                &types,
                &[],
                &function,
                &ssa,
                usize::MAX,
                usize::MAX,
                Some(&mut budget)
            )
            .is_err()
        );
    }

    #[test]
    fn actual_owner_call_result_view_preserves_all_slots_without_private_memory() {
        let source = owner();
        let roster = roster(&source);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let lowered = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            source,
            roster,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let root = SemanticFunctionIdV1::from_index(0);
        lowered
            .with_checked_call_v1(
                root,
                root,
                SemanticBlockIdV1::from_index(0),
                &mut budget,
                |view| {
                    assert_eq!(view.result_count(), 6);
                    for slot in 0..6 {
                        assert!(view.result_component(slot).is_none());
                        let component = view.enum_result_component(slot).unwrap();
                        assert_eq!(component.value(), &view.operation().results[slot]);
                        assert_eq!(
                            view.result_transport(slot).unwrap().value(),
                            component.value().id
                        );
                    }
                    assert_eq!(
                        view.enum_result_component(0).unwrap().slot(),
                        ProductionCallEnumResultSlotV1::Tag
                    );
                    assert!(view.enum_result_component(6).is_none());
                    assert!(view.visit_result_nodes(|_| Ok(())).is_err());
                    Ok(())
                },
            )
            .unwrap();
        let module = lowered.executable().module();
        for function in &module.functions {
            for block in &function.body.as_ref().unwrap().blocks {
                assert!(!block.operations.iter().any(|operation| matches!(operation.kind,
                    OperationKind::Load { ref access, .. } | OperationKind::Store { ref access, .. }
                    if access.address_space == AddressSpace::Private
                )));
            }
        }
        let helper = module
            .functions
            .iter()
            .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .unwrap();
        assert_eq!(helper.signature.results.len(), 6);
        assert!(
            helper
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| matches!(
                    operation.kind,
                    OperationKind::Constant(Constant::U8(0))
                ))
        );
    }

    #[test]
    fn scoped_instance_enum_result_signature_matches_nonempty_common_plan() {
        let mut source = owner();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        let receipt = source
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        production_call_instances_v1::with_production_call_instances_v1(
            &source,
            SemanticFunctionIdV1::from_index(0),
            &mut budget,
            |instances, budget| {
                let child = instances.calls(instances.root()).unwrap()[0]
                    .child()
                    .unwrap();
                let signature = execution_function_signature_v29(instances, child, budget).unwrap();
                let plan = execution_instance_plan_v29(
                    instances,
                    child,
                    FunctionId::new("enum_instance"),
                    SemanticEmissionPlacementV1 {
                        first_block: 7,
                        first_value: 101,
                    },
                    budget,
                )
                .unwrap();
                assert_eq!(signature.result_types.len(), 6);
                assert_eq!(signature.result_types, plan.result_types);
                assert_eq!(signature.result_semantic_type, ENUM);
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
            },
        )
        .unwrap();
    }

    #[test]
    fn same_typed_return_slot_swap_fails_actual_owner_equivalence_replay() {
        let source = owner();
        let roster = roster(&source);
        let roots = materialization_launch_roots_v1(&source, &roster).unwrap();
        let limits = ProductionSemanticKirLimitsV1::default();
        let (mut module, mut correspondence) = lower_module(&source, limits, Some(&roots)).unwrap();
        let helper = module
            .functions
            .iter_mut()
            .find(|function| function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .unwrap();
        for block in &mut helper.body.as_mut().unwrap().blocks {
            if let Some(Terminator::Return { values }) = &mut block.terminator {
                assert_eq!(values.len(), 6);
                values.swap(0, 1);
            }
        }
        verify_module(&module).unwrap();
        for row in &correspondence.call_returns {
            if row.semantic_function.index() == 1
                && matches!(row.kind, SemanticKirCallReturnKindV1::Return { .. })
            {
                let range = row.components().range().unwrap();
                correspondence
                    .call_result_components
                    .swap(range.start, range.start + 1);
            }
        }
        let owner = ProductionSemanticKirOwnerV1 {
            canonical_kernel_ir: ProductionCanonicalKernelIrV1::from_module(module.clone())
                .unwrap(),
            semantic_ssa: source,
            module: RetainedProductionKirModuleV1::Legacy(module),
            correspondence,
            limits,
            launch_roots: Some(roots),
            generic_checks: Box::new([]),
        };
        assert!(matches!(
            owner.verify_equivalence(),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
}

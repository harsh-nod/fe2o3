    #[test]
    fn component_witness_transport_contract_requires_the_exact_checked_producer() {
        let input = SemanticTypeIdV1::from_index(1);
        let witness = SemanticTypeIdV1::from_index(2);
        let raw = SemanticTypeIdV1::from_index(3);
        let blocked = SemanticDisjointIndexSpaceV1::BlockedIndex1d {
            lanes_per_block: 64,
            elements_per_lane: 4,
        };
        let tiled = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
            lanes_per_tile: 64,
            tile_rows: 16,
            tile_columns: 16,
            elements_per_lane: 4,
        };
        let striped = SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
            lanes_per_row: 64,
            elements_per_lane: 4,
        };
        let producers = [
            (
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedBlock {
                    input_witness: input,
                    output_block: witness,
                    raw_index: raw,
                    input_space: SemanticDisjointIndexSpaceV1::Index1d,
                    output_space: blocked,
                    lanes_per_block: 64,
                    elements_per_lane: 4,
                },
                blocked,
            ),
            (
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedTiled2d {
                    input_witness: input,
                    output_tile: witness,
                    raw_index: raw,
                    input_space: SemanticDisjointIndexSpaceV1::Index1d,
                    output_space: tiled,
                    lanes_per_tile: 64,
                    tile_rows: 16,
                    tile_columns: 16,
                    elements_per_lane: 4,
                },
                tiled,
            ),
            (
                SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                    input_witness: input,
                    output_stripe: witness,
                    raw_index: raw,
                    input_space: SemanticDisjointIndexSpaceV1::Index1d,
                    output_space: striped,
                    lanes_per_row: 64,
                    elements_per_lane: 4,
                },
                striped,
            ),
        ];
        for (operation, expected) in producers {
            assert_eq!(
                option_component_witness_contract_v1(&operation, witness),
                Some(expected)
            );
            assert_eq!(
                option_component_witness_contract_v1(&operation, SemanticTypeIdV1::from_index(9)),
                None
            );
        }
        assert_eq!(
            option_component_witness_contract_v1(
                &SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
                    disjoint_slice: input,
                    block_witness: witness,
                    element: raw,
                    raw_index: raw,
                    index_space: blocked,
                    lanes_per_block: 64,
                    elements_per_lane: 4,
                },
                witness,
            ),
            None
        );
    }

    #[test]
    fn checked_shift_contract_mints_disjoint_output_from_thread_or_disjoint_input() {
        let input = SemanticTypeIdV1::from_index(1);
        let output = SemanticTypeIdV1::from_index(2);
        let raw = SemanticTypeIdV1::from_index(3);
        let shifted = SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 4 };
        let availability = test_option_availability_v1();
        let thread = SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
            input_witness: input,
            output_witness: output,
            raw_index: raw,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space: shifted,
            offset: 4,
        };
        let disjoint = SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
            input_witness: input,
            output_witness: output,
            raw_index: raw,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space: shifted,
            offset: 4,
        };
        let expected = Some(SemanticPromotedBindingV1::IndexWitness {
            index_space: shifted,
            disjoint: true,
            availability: Some(SemanticCapabilityAvailabilityV1::Option(availability)),
        });

        assert_eq!(
            option_capability_contract_v1(&thread, output, availability),
            expected
        );
        assert_eq!(
            option_capability_contract_v1(&disjoint, output, availability),
            expected
        );
        assert_eq!(
            option_capability_contract_v1(&thread, input, availability),
            None
        );
        let wrong_output = SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedShift {
            input_witness: input,
            output_witness: output,
            raw_index: raw,
            input_space: SemanticDisjointIndexSpaceV1::Index1d,
            output_space: SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 5 },
            offset: 4,
        };
        let wrong_input = SemanticCompilerIntrinsicOperationV1::DisjointIndexCheckedShift {
            input_witness: input,
            output_witness: output,
            raw_index: raw,
            input_space: shifted,
            output_space: shifted,
            offset: 4,
        };
        assert_eq!(
            option_capability_contract_v1(&wrong_output, output, availability),
            None
        );
        assert_eq!(
            option_capability_contract_v1(&wrong_input, output, availability),
            None
        );
    }
    fn nested_component_capability_fixture_v1(
        selected_result_variant: u32,
        projected_write: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        Vec<SemanticCallableDeclV1>,
        SemanticFunctionDeclV1,
        SemanticOptionDominanceV1,
    ) {
        let unit = SemanticTypeIdV1::from_index(0);
        let u32_ty = SemanticTypeIdV1::from_index(1);
        let witness = SemanticTypeIdV1::from_index(2);
        let option = SemanticTypeIdV1::from_index(3);
        let option_some = SemanticTypeIdV1::from_index(4);
        let control = SemanticTypeIdV1::from_index(5);
        let control_continue = SemanticTypeIdV1::from_index(6);
        let result = SemanticTypeIdV1::from_index(7);
        let result_ok = SemanticTypeIdV1::from_index(8);
        let source = SemanticSourceProvenanceV1::unavailable();
        let aggregate_type = |tag, fields: Vec<SemanticTypeIdV1>| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag.wrapping_add(1); 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
            )
        };
        let enum_type = |tag, variants: Vec<Vec<SemanticTypeIdV1>>| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag.wrapping_add(1); 32]),
                SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
                SemanticTypeShapeV1::Enum {
                    discriminant: u32_ty,
                    variants: variants
                        .into_iter()
                        .enumerate()
                        .map(|(variant, fields)| {
                            SemanticEnumVariantV1::new(
                                variant as u128,
                                SemanticAggregateTypeV1::new(fields).unwrap(),
                            )
                        })
                        .collect::<Vec<_>>()
                        .into_boxed_slice(),
                },
            )
        };
        let types = vec![
            unit_type(),
            unsigned_scalar_type(11, 32),
            aggregate_type(13, vec![u32_ty]),
            enum_type(15, vec![vec![], vec![witness]]),
            aggregate_type(17, vec![witness]),
            enum_type(19, vec![vec![witness], vec![unit]]),
            aggregate_type(21, vec![witness]),
            enum_type(23, vec![vec![witness], vec![unit]]),
            aggregate_type(25, vec![witness]),
        ];
        let direct = |ty| {
            SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(
                    fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueAttributesV1::plain(),
                ),
            )
        };
        let intrinsic_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([27; 32]),
            SemanticLayoutIdentityV1::from_sha256([28; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            direct(option),
        )
        .unwrap();
        let striped = SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
            lanes_per_row: 64,
            elements_per_lane: 64,
        };
        let callables = vec![SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([29; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([31; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([32; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([33; 32]),
                source,
                intrinsic_abi,
            ),
            operation: SemanticCompilerIntrinsicOperationV1::ThreadIndexCheckedRowStriped2d {
                input_witness: u32_ty,
                output_stripe: witness,
                raw_index: u32_ty,
                input_space: SemanticDisjointIndexSpaceV1::Index1d,
                output_space: striped,
                lanes_per_row: 64,
                elements_per_lane: 64,
            },
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([34; 32]),
        }];
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let payload_place = |local, variant, variant_type, payload_type| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(local),
                vec![
                    SemanticProjectionV1::new(
                        SemanticProjectionKindV1::Downcast(variant),
                        variant_type,
                    )
                    .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), payload_type)
                        .unwrap(),
                ],
                payload_type,
            )
            .unwrap()
        };
        let assign_use = |destination, operand_place| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(
                        witness,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(operand_place)),
                    ),
                )),
            )
        };
        let assign_variant = |local, ty, variant, operand| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
                    SemanticRvalueV1::new(
                        ty,
                        SemanticRvalueKindV1::aggregate(
                            SemanticAggregateKindV1::EnumVariant(variant),
                            vec![SemanticOperandV1::Move(operand)],
                        )
                        .unwrap(),
                    ),
                )),
            )
        };
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let producer_call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(1, option),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let discriminant = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(2, u32_ty),
                SemanticRvalueV1::new(u32_ty, SemanticRvalueKindV1::Discriminant(place(1, option))),
            )),
        );
        let mut nested = vec![
            assign_use(place(3, witness), payload_place(1, 1, option_some, witness)),
            assign_variant(4, control, 0, place(3, witness)),
            assign_use(
                place(5, witness),
                payload_place(4, 0, control_continue, witness),
            ),
            assign_variant(6, result, 0, place(5, witness)),
            assign_use(
                place(7, witness),
                payload_place(6, selected_result_variant, result_ok, witness),
            ),
            assign_use(place(8, witness), place(7, witness)),
        ];
        if projected_write {
            let projected = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(8),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap(),
                ],
                u32_ty,
            )
            .unwrap();
            nested.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    projected,
                    SemanticRvalueV1::new(
                        u32_ty,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(
                                u32_ty,
                                SemanticConstantValueV1::Scalar(
                                    SemanticScalarValueV1::new(0, 4).unwrap(),
                                ),
                            ),
                        )),
                    ),
                )),
            ));
        }
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
            block(35, vec![], SemanticTerminatorKindV1::Call(producer_call)),
            block(
                36,
                vec![discriminant],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, u32_ty)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                    )
                    .unwrap(),
                },
            ),
            block(37, nested, SemanticTerminatorKindV1::Return),
            block(38, vec![], SemanticTerminatorKindV1::Return),
        ];
        let function_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([39; 32]),
            SemanticLayoutIdentityV1::from_sha256([40; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let local = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                role,
                source,
            )
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([41; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([42; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([43; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([44; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([45; 32]),
            source,
            function_abi,
            vec![
                local(46, unit, SemanticLocalRoleV1::Return),
                local(47, option, SemanticLocalRoleV1::Temporary),
                local(48, u32_ty, SemanticLocalRoleV1::Temporary),
                local(49, witness, SemanticLocalRoleV1::Temporary),
                local(50, control, SemanticLocalRoleV1::Temporary),
                local(51, witness, SemanticLocalRoleV1::Temporary),
                local(52, result, SemanticLocalRoleV1::Temporary),
                local(53, witness, SemanticLocalRoleV1::Temporary),
                local(54, witness, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        let option_dominance = SemanticOptionDominanceV1::analyze(
            &function,
            &[fe2o3_mir_model::SemanticOptionProducerV1::new(
                SemanticLocalIdV1::from_index(1),
                SemanticBlockIdV1::from_index(1),
            )],
        )
        .unwrap();
        (types, callables, function, option_dominance)
    }

    #[test]
    fn capability_origin_traverses_nested_try_enums_and_aliases() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let availability = option_dominance
            .availability(SemanticLocalIdV1::from_index(1))
            .unwrap();
        let certified = (0..function.locals().len() as u32).collect();
        assert_eq!(
            promoted_capability_binding_v1(
                &types,
                &callables,
                &function,
                &option_dominance,
                &certified,
                8,
            )
            .unwrap(),
            Some(SemanticPromotedBindingV1::ComponentWitness {
                index_space: SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
                    lanes_per_row: 64,
                    elements_per_lane: 64,
                },
                availability: SemanticCapabilityAvailabilityV1::Option(availability),
            })
        );
    }

    #[test]
    fn original_compiler_carriers_define_plain_cfg_transport_contracts() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect();
        let availability = option_dominance
            .availability(SemanticLocalIdV1::from_index(1))
            .unwrap();
        let index_space = SemanticDisjointIndexSpaceV1::RowStriped2dIndex1d {
            lanes_per_row: 64,
            elements_per_lane: 64,
        };
        let expected = [
            (
                1,
                SemanticPromotedBindingV1::OptionComponentWitness {
                    index_space,
                    availability,
                },
                vec![
                    ValueDef::new(ValueId(91), Type::BOOL),
                    ValueDef::new(ValueId(92), Type::INDEX),
                ],
            ),
            (
                8,
                SemanticPromotedBindingV1::ComponentWitness {
                    index_space,
                    availability: SemanticCapabilityAvailabilityV1::Option(availability),
                },
                vec![ValueDef::new(ValueId(92), Type::INDEX)],
            ),
        ];
        for (local, expected_descriptor, values) in expected {
            let descriptor = promoted_capability_binding_v1(
                &types,
                &callables,
                &function,
                &option_dominance,
                &certified,
                local,
            )
            .unwrap()
            .expect("the original intrinsic and guarded aliases identify this carrier");
            assert_eq!(descriptor, expected_descriptor);
            let ty = function.locals()[local as usize].ty();

            // This is the intrinsic-result representation contract, not a source
            // plan admission or a test of the not-yet-integrated CFG dispatcher.
            let plain = SourceReferenceNodeV29 {
                ty,
                kind: SourceReferenceNodeKindV29::Plain(None),
                value_origin: None,
                storage: None,
                inactive: None,
                descriptor: None,
            };
            let expected_types: Vec<_> = values.iter().map(|value| value.ty.clone()).collect();
            assert_eq!(descriptor.transport_types(&types, plain.ty).unwrap(), expected_types);
            check_paid_compiler_carrier_construction(descriptor, &types, plain.ty, &values);
            let binding = descriptor
                .binding_from_transport(&types, plain.ty, &values)
                .unwrap();
            assert_eq!(
                descriptor.transport_values(&binding).unwrap(),
                values.iter().map(|value| (value.id, value.ty.clone())).collect::<Vec<_>>(),
            );
            check_transport_visitors(
                SemanticPromotedTransportV1::Semantic(descriptor),
                &binding,
                &expected_types,
            );
            match (local, &binding) {
                (
                    1,
                    SemanticValueBindingV1::OptionComponentWitness {
                        present,
                        raw,
                        index_space: actual_space,
                        availability: actual_availability,
                    },
                ) => {
                    assert_eq!((*present, *raw), (ValueId(91), ValueId(92)));
                    assert_eq!(*actual_space, index_space);
                    assert_eq!(*actual_availability, availability);
                }
                (
                    8,
                    SemanticValueBindingV1::ComponentWitness {
                        raw,
                        index_space: actual_space,
                        availability: actual_availability,
                    },
                ) => {
                    assert_eq!(*raw, ValueId(92));
                    assert_eq!(*actual_space, index_space);
                    assert_eq!(
                        *actual_availability,
                        SemanticCapabilityAvailabilityV1::Option(availability),
                    );
                }
                _ => panic!("source carrier was rebuilt as a different binding: {binding:?}"),
            }
        }
    }

    #[test]
    fn original_carrier_resolution_prepays_shared_ledger_before_queries() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect();
        let local = SemanticLocalIdV1::from_index(8);
        let make = || SemanticCapabilityOriginResolverV1::new(
            &types, &callables, &function, &option_dominance, &certified,
            usize::MAX, usize::MAX,
        ).unwrap();
        let expected = make().resolve(local).unwrap();
        assert!(expected.is_some());
        let run = |work_limit, storage_limit| {
            let mut resolver = make();
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
            budget.reserve_storage(37).unwrap();
            let result = scoped_slot_attempt_v29(&mut budget, |budget| {
                resolver.resolve_with_budget_v29(local, &mut Some(budget))
            });
            // All recursive visiting rows unwind even after a paid operation
            // refuses. Retained memo and conservative credits die with owner.
            assert!(resolver.visiting.is_empty());
            let observed = (budget.work(), budget.peak_storage());
            if result.is_ok() {
                let retained = budget.storage() - 37;
                budget.release_storage(retained).unwrap();
            }
            assert_eq!(budget.storage(), 37);
            (result, observed)
        };
        let (result, measured) = run(1_000_000, 1_000_000);
        assert_eq!(result.unwrap(), expected);
        assert!(measured.0 > 0 && measured.1 > 37);
        // These are calibrated recursive-query boundaries, distinct from the
        // independent zero-component construction equation.
        assert_eq!(run(measured.0, measured.1).0.unwrap(), expected);
        assert!(matches!(run(measured.0 - 1, measured.1).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(_)))));
        assert!(matches!(run(measured.0, measured.1 - 1).0,
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(_)))));

        let mut resolver = make();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        assert!(resolver.resolve_with_budget_v29(local, &mut Some(&mut budget)).is_err());
        assert!(resolver.memo.is_empty() && resolver.visiting.is_empty());
        assert_eq!(budget.storage(), 0, "initial work refusal precedes allocation");
    }

    #[test]
    fn original_carrier_capture_reuses_exact_intrinsic_and_guarded_alias_contracts() {
        let (types, callables, function, dominance) = nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect();
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types, &callables, &function, &dominance, &certified, usize::MAX, usize::MAX,
        ).unwrap();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        for local in [1, 8] {
            let expected = promoted_capability_binding_v1(&types, &callables, &function,
                &dominance, &certified, local).unwrap().unwrap();
            assert_eq!(original_compiler_carrier_v29(SemanticLocalIdV1::from_index(local),
                &BTreeMap::new(), &mut resolver, &mut budget).unwrap(),
                Some((function.locals()[local as usize].ty(), expected)));
        }
        assert!(resolver.visiting.is_empty());
    }

    #[test]
    fn original_carrier_capture_does_not_rewalk_ordinary_alias_chains() {
        let (types, _, base, _) = nested_component_capability_fixture_v1(0, false);
        let ty = SemanticTypeIdV1::from_index(1);
        let source = base.source();
        for count in [16_u32, 64, 256, 1024] {
            let mut locals = vec![base.locals()[0].clone()];
            let mut statements = Vec::new();
            let place = |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
            for local in 1..=count {
                let mut identity = [0; 32];
                identity[..4].copy_from_slice(&local.to_le_bytes());
                locals.push(SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256(identity),
                    ty, SemanticLocalRoleV1::Temporary, source));
                let operand = if local == 1 {
                    SemanticOperandV1::Constant(SemanticConstantV1::new(ty,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(9, 4).unwrap())))
                } else { SemanticOperandV1::Copy(place(local - 1)) };
                statements.push(SemanticStatementV1::new(source, SemanticStatementKindV1::Assign(
                    SemanticAssignmentV1::new(place(local), SemanticRvalueV1::new(ty, SemanticRvalueKindV1::Use(operand))))));
            }
            let function = SemanticFunctionDeclV1::new(base.identity(), base.role(), base.item_definition_identity(),
                base.monomorphization_identity(), base.generic_type_arguments_identity(), base.const_generic_arguments_identity(),
                source, base.abi().clone(), locals, SemanticBlockIdV1::from_index(0), vec![
                    SemanticBasicBlockV1::new(SemanticBlockIdentityV1::from_sha256([81; 32]), source, statements,
                        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return)).unwrap(),
                ]).unwrap();
            let dominance = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
            let certified = (0..=count).collect();
            let mut resolver = SemanticCapabilityOriginResolverV1::new(
                &types, &[], &function, &dominance, &certified, usize::MAX, usize::MAX,
            ).unwrap();
            // This bounds only the newly expanded query roster, not the
            // inherited constructor's independently metered source indexing.
            let bound = 120 * count as usize * (count.ilog2() as usize + 2);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(bound);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            for local in 1..=count {
                assert_eq!(original_compiler_carrier_v29(SemanticLocalIdV1::from_index(local),
                    &BTreeMap::new(), &mut resolver, &mut budget).unwrap(), None);
            }
            assert_eq!(resolver.memo.len(), count as usize);
            assert!(resolver.visiting.is_empty());
            assert!(budget.work() <= bound);
        }
    }

    #[test]
    fn original_compiler_carrier_transport_rejects_changed_payload_and_availability() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect();
        let descriptor = |local| {
            promoted_capability_binding_v1(
                &types,
                &callables,
                &function,
                &option_dominance,
                &certified,
                local,
            )
            .unwrap()
            .unwrap()
        };
        let optional = descriptor(1);
        let ty = function.locals()[1].ty();
        let values = [
            ValueDef::new(ValueId(91), Type::BOOL),
            ValueDef::new(ValueId(92), Type::INDEX),
        ];
        let valid = optional.binding_from_transport(&types, ty, &values).unwrap();
        assert_eq!(optional.transport_values(&valid).unwrap().len(), 2);
        for invalid in [
            vec![],
            vec![values[0].clone()],
            vec![values[1].clone(), values[0].clone()],
            vec![values[0].clone(), values[1].clone(), values[1].clone()],
        ] {
            assert!(matches!(
                optional.binding_from_transport(&types, ty, &invalid),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "typed fragment SSA component types changed",
                    ..
                }),
            ));
        }
        let SemanticValueBindingV1::OptionComponentWitness {
            present,
            raw,
            availability,
            ..
        } = valid else { unreachable!() };
        let changed_contract = SemanticValueBindingV1::OptionComponentWitness {
            present,
            raw,
            index_space: SemanticDisjointIndexSpaceV1::Index1d,
            availability,
        };
        assert_eq!(
            optional.transport_values(&changed_contract),
            Err("promoted optional component witness lacks its authenticated producer metadata"),
        );
        let witness = descriptor(8);
        let valid = witness
            .binding_from_transport(&types, function.locals()[8].ty(), &values[1..])
            .unwrap();
        assert_eq!(witness.transport_values(&valid).unwrap().len(), 1);
        let SemanticValueBindingV1::ComponentWitness { raw, index_space, .. } = valid
        else { unreachable!() };
        let changed_availability = SemanticValueBindingV1::ComponentWitness {
            raw,
            index_space,
            availability: SemanticCapabilityAvailabilityV1::EnumPayload {
                local: SemanticLocalIdV1::from_index(6),
                variant: 0,
            },
        };
        assert_eq!(
            witness.transport_values(&changed_availability),
            Err("promoted component witness lacks its authenticated availability"),
        );
    }

    #[test]
    fn capability_origin_rejects_wrong_variant_and_projected_writes() {
        for (selected_variant, projected_write) in [(1, false), (0, true)] {
            let (types, callables, function, option_dominance) =
                nested_component_capability_fixture_v1(selected_variant, projected_write);
            let certified = (0..function.locals().len() as u32).collect();
            assert_eq!(
                promoted_capability_binding_v1(
                    &types,
                    &callables,
                    &function,
                    &option_dominance,
                    &certified,
                    8,
                )
                .unwrap(),
                None
            );
        }
    }

    #[test]
    fn capability_origin_requires_every_alias_and_enum_carrier_to_be_certified() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let all = (0..function.locals().len() as u32).collect::<BTreeSet<_>>();
        for traversed in [1, 3, 4, 5, 6, 7, 8] {
            let mut certified = all.clone();
            certified.remove(&traversed);
            assert_eq!(
                promoted_capability_binding_v1(
                    &types,
                    &callables,
                    &function,
                    &option_dominance,
                    &certified,
                    8,
                )
                .unwrap(),
                None,
                "uncertified local {traversed} was traversed",
            );
        }
    }

    #[test]
    fn capability_definition_index_invalidates_every_storage_observation() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect::<BTreeSet<_>>();
        let source = SemanticSourceProvenanceV1::unavailable();
        let witness = SemanticTypeIdV1::from_index(2);
        let control = SemanticTypeIdV1::from_index(5);
        let u32_ty = SemanticTypeIdV1::from_index(1);
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let statement = |kind| SemanticStatementV1::new(source, kind);
        let observations = vec![
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0, SemanticTypeIdV1::from_index(0)),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(0),
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Mutable,
                        place: place(7, witness),
                    },
                ),
            ))),
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(0, SemanticTypeIdV1::from_index(0)),
                SemanticRvalueV1::new(
                    SemanticTypeIdV1::from_index(0),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(6, SemanticTypeIdV1::from_index(7)),
                    },
                ),
            ))),
            statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(5, witness),
                SemanticOperandV1::Copy(place(3, witness)),
                SemanticVolatilityV1::NonVolatile,
                None,
            ))),
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: place(4, control),
                variant_index: 0,
            }),
            statement(SemanticStatementKindV1::Deinitialize(place(3, witness))),
            statement(SemanticStatementKindV1::AtomicRmw(
                SemanticAtomicRmwV1::new(
                    place(8, witness),
                    place(1, SemanticTypeIdV1::from_index(3)),
                    SemanticOperandV1::Copy(place(3, witness)),
                    SemanticAtomicRmwOpV1::Exchange,
                    SemanticAtomicAccessV1::new(
                        SemanticAtomicOrderingV1::Relaxed,
                        SemanticAtomicScopeV1::Workgroup,
                    ),
                ),
            )),
        ];
        let projected_destination = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap()],
            u32_ty,
        )
        .unwrap();
        let projected_call = SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    projected_destination,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(0),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        );
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &option_dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        for observation in &observations {
            resolver.index_statement(observation.kind()).unwrap();
        }
        resolver.index_terminator(&projected_call).unwrap();
        assert_eq!(
            resolver.invalidated_locals,
            BTreeSet::from([1, 2, 3, 4, 5, 6, 7, 8]),
        );
    }

    #[test]
    fn capability_origin_cycles_fail_closed() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect::<BTreeSet<_>>();
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &option_dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        let self_alias = match function.blocks()[2].statements()[5].kind() {
            SemanticStatementKindV1::Assign(assignment) => assignment.value().kind(),
            _ => unreachable!(),
        };
        resolver.definitions.insert(
            7,
            vec![SemanticCapabilityDefinitionV1::Assignment(self_alias)],
        );
        assert_eq!(
            resolver.resolve(SemanticLocalIdV1::from_index(7)).unwrap(),
            None
        );
    }

    #[test]
    fn capability_origin_resource_limits_have_exact_inclusive_boundaries() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect::<BTreeSet<_>>();
        let target = SemanticLocalIdV1::from_index(8);
        let resolve_with_limits = |work_limit, storage_limit| {
            let mut resolver = SemanticCapabilityOriginResolverV1::new(
                &types,
                &callables,
                &function,
                &option_dominance,
                &certified,
                work_limit,
                storage_limit,
            )?;
            let result = resolver.resolve(target)?;
            Ok::<_, ProductionSemanticKirErrorV1>((result, resolver.work, resolver.peak_storage))
        };

        let (expected, exact_work, exact_storage) =
            resolve_with_limits(usize::MAX, usize::MAX).unwrap();
        let (actual, charged_work, peak_storage) =
            resolve_with_limits(exact_work, exact_storage).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(charged_work, exact_work);
        assert_eq!(peak_storage, exact_storage);

        assert!(matches!(
            resolve_with_limits(exact_work - 1, usize::MAX),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual,
                limit,
            }) if actual == exact_work && limit == exact_work - 1
        ));
        assert!(matches!(
            resolve_with_limits(usize::MAX, exact_storage - 1),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual,
                limit,
            }) if actual == exact_storage && limit == exact_storage - 1
        ));
    }

    #[test]
    fn capability_origin_reports_typed_work_exhaustion_after_indexing() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect::<BTreeSet<_>>();
        let mut resolver = SemanticCapabilityOriginResolverV1::new(
            &types,
            &callables,
            &function,
            &option_dominance,
            &certified,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        resolver.work_limit = resolver.work;
        let error = resolver
            .resolve(SemanticLocalIdV1::from_index(8))
            .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                ..
            }
        ));
    }

    #[test]
    fn capability_origin_rejects_malformed_enum_aggregate_arity_and_type() {
        let (types, callables, function, option_dominance) =
            nested_component_capability_fixture_v1(0, false);
        let certified = (0..function.locals().len() as u32).collect::<BTreeSet<_>>();
        let result = SemanticTypeIdV1::from_index(7);
        let witness = SemanticTypeIdV1::from_index(2);
        let malformed_arity = SemanticRvalueV1::new(
            result,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                .unwrap(),
        );
        let wrong_type = SemanticRvalueV1::new(
            result,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(0),
                vec![SemanticOperandV1::Copy(
                    SemanticPlaceV1::new(
                        SemanticLocalIdV1::from_index(2),
                        vec![],
                        SemanticTypeIdV1::from_index(1),
                    )
                    .unwrap(),
                )],
            )
            .unwrap(),
        );
        for malformed in [malformed_arity.kind(), wrong_type.kind()] {
            let mut resolver = SemanticCapabilityOriginResolverV1::new(
                &types,
                &callables,
                &function,
                &option_dominance,
                &certified,
                usize::MAX,
                usize::MAX,
            )
            .unwrap();
            resolver.definitions.insert(
                6,
                vec![SemanticCapabilityDefinitionV1::Assignment(malformed)],
            );
            assert_eq!(
                resolver
                    .resolve_enum_payload(SemanticLocalIdV1::from_index(6), 0, 0, witness,)
                    .unwrap(),
                None,
            );
        }
    }

    #[test]
    fn specialized_option_projection_accepts_rustc_exact_some_payload_only() {
        let (types, _, _, _) = nested_component_capability_fixture_v1(0, false);
        let option = SemanticTypeIdV1::from_index(3);
        let some = SemanticTypeIdV1::from_index(4);
        let witness = SemanticTypeIdV1::from_index(2);
        let u32_ty = SemanticTypeIdV1::from_index(1);
        let projection = |kind, result_type| SemanticProjectionV1::new(kind, result_type).unwrap();
        let valid = vec![
            projection(SemanticProjectionKindV1::Downcast(1), option),
            projection(SemanticProjectionKindV1::Field(0), witness),
        ];
        assert_eq!(
            exact_option_payload_projection_v1(&types, option, &valid, witness),
            Some(witness),
        );

        let malformed = [
            vec![
                projection(SemanticProjectionKindV1::Downcast(0), some),
                valid[1],
            ],
            vec![
                valid[0],
                projection(SemanticProjectionKindV1::Field(1), witness),
            ],
            vec![
                valid[0],
                projection(SemanticProjectionKindV1::Field(0), u32_ty),
            ],
            vec![
                projection(SemanticProjectionKindV1::Downcast(1), some),
                valid[1],
            ],
            vec![
                projection(SemanticProjectionKindV1::Downcast(1), u32_ty),
                valid[1],
            ],
            vec![valid[0]],
        ];
        for projections in malformed {
            assert_eq!(
                exact_option_payload_projection_v1(&types, option, &projections, witness),
                None,
            );
        }
        assert_eq!(
            exact_option_payload_projection_v1(&types, option, &valid, u32_ty),
            None,
        );
    }

    #[test]
    fn specialized_option_projection_remains_confined_to_the_some_edge() {
        let (_, _, _, option_dominance) = nested_component_capability_fixture_v1(0, false);
        let option = SemanticLocalIdV1::from_index(1);
        let availability = option_dominance.availability(option).unwrap();

        assert!(option_dominance.allows(availability, SemanticBlockIdV1::from_index(2),));
        assert!(!option_dominance.allows(availability, SemanticBlockIdV1::from_index(3),));
    }

    #[test]
    fn capability_origin_rejects_conflicting_option_availability() {
        let unit = SemanticTypeIdV1::from_index(0);
        let option = SemanticTypeIdV1::from_index(1);
        let discriminant = SemanticTypeIdV1::from_index(2);
        let source = SemanticSourceProvenanceV1::unavailable();
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
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
        let option_call = |local, target| {
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    place(local, option),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap()
        };
        let bind_discriminant = |option_local, discriminant_local| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(discriminant_local, discriminant),
                    SemanticRvalueV1::new(
                        discriminant,
                        SemanticRvalueKindV1::Discriminant(place(option_local, option)),
                    ),
                )),
            )
        };
        let switch = |discriminant_local, some, none| SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(discriminant_local, discriminant)),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    0,
                    edge(SemanticEdgeRoleV1::SwitchValue, none),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, some),
            )
            .unwrap(),
        };
        let blocks = vec![
            block(
                170,
                vec![],
                SemanticTerminatorKindV1::Call(option_call(1, 1)),
            ),
            block(171, vec![bind_discriminant(1, 2)], switch(2, 2, 6)),
            block(
                172,
                vec![],
                SemanticTerminatorKindV1::Call(option_call(3, 3)),
            ),
            block(173, vec![bind_discriminant(3, 4)], switch(4, 4, 5)),
            block(174, vec![], SemanticTerminatorKindV1::Return),
            block(175, vec![], SemanticTerminatorKindV1::Return),
            block(176, vec![], SemanticTerminatorKindV1::Return),
        ];
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([177; 32]),
            SemanticLayoutIdentityV1::from_sha256([178; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let local = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                role,
                source,
            )
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([179; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([180; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([181; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([182; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([183; 32]),
            source,
            abi,
            vec![
                local(184, unit, SemanticLocalRoleV1::Return),
                local(185, option, SemanticLocalRoleV1::Temporary),
                local(186, discriminant, SemanticLocalRoleV1::Temporary),
                local(187, option, SemanticLocalRoleV1::Temporary),
                local(188, discriminant, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        let first_local = SemanticLocalIdV1::from_index(1);
        let second_local = SemanticLocalIdV1::from_index(3);
        let dominance = SemanticOptionDominanceV1::analyze(
            &function,
            &[
                fe2o3_mir_model::SemanticOptionProducerV1::new(
                    first_local,
                    SemanticBlockIdV1::from_index(1),
                ),
                fe2o3_mir_model::SemanticOptionProducerV1::new(
                    second_local,
                    SemanticBlockIdV1::from_index(3),
                ),
            ],
        )
        .unwrap();
        let first = dominance.availability(first_local).unwrap();
        let second = dominance.availability(second_local).unwrap();
        assert_ne!(first, second);

        let mut merged = None;
        assert!(merge_capability_origin_v1(
            &mut merged,
            SemanticPromotedBindingV1::OptionGridLeader {
                availability: first,
            },
        ));
        assert!(!merge_capability_origin_v1(
            &mut merged,
            SemanticPromotedBindingV1::OptionGridLeader {
                availability: second,
            },
        ));
    }

    #[test]
    fn retained_memory_gate_distinguishes_local_storage_from_dereferenced_memory() {
        let root = SemanticTypeIdV1::from_index(0);
        let projected = SemanticTypeIdV1::from_index(1);
        let local = SemanticLocalIdV1::from_index(0);
        let place = |projections| SemanticPlaceV1::new(local, projections, projected).unwrap();
        let projection = |kind, result_type| SemanticProjectionV1::new(kind, result_type).unwrap();

        assert!(place_requires_local_storage_v1(
            &SemanticPlaceV1::new(local, vec![], root).unwrap()
        ));
        assert!(place_requires_local_storage_v1(&place(vec![projection(
            SemanticProjectionKindV1::Field(0),
            projected,
        )])));
        assert!(!place_requires_local_storage_v1(&place(vec![projection(
            SemanticProjectionKindV1::Dereference,
            projected,
        )])));
        assert!(!place_requires_local_storage_v1(&place(vec![
            projection(SemanticProjectionKindV1::Field(0), root),
            projection(SemanticProjectionKindV1::Dereference, projected),
        ])));
    }

    #[test]
    fn retained_slot_types_are_exactly_scalars_and_metadata_free_pointers() {
        let scalar = SemanticTypeIdV1::from_index(0);
        let thin_pointer = SemanticTypeIdV1::from_index(1);
        let fat_pointer = SemanticTypeIdV1::from_index(2);
        let aggregate = SemanticTypeIdV1::from_index(3);
        let pointer = |tag, metadata, size| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag.wrapping_add(1); 32]),
                SemanticTypeLayoutV1::new(size, 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        scalar,
                        SemanticPointerKindV1::Raw,
                        SemanticMutabilityV1::Mutable,
                        1,
                        64,
                        metadata,
                    )
                    .unwrap(),
                ),
            )
        };
        let types = vec![
            u64_type(),
            pointer(210, SemanticPointerMetadataV1::None, Some(8)),
            pointer(212, SemanticPointerMetadataV1::SliceLength, Some(16)),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([214; 32]),
                SemanticLayoutIdentityV1::from_sha256([215; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![scalar]).unwrap()),
            ),
        ];

        assert_eq!(
            retained_local_slot_type_v1(&types, scalar),
            Some((Type::Scalar(ScalarType::U64), 8))
        );
        assert_eq!(
            retained_local_slot_type_v1(&types, thin_pointer),
            Some((
                Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                8,
            ))
        );
        assert_eq!(retained_local_slot_type_v1(&types, fat_pointer), None);
        assert_eq!(retained_local_slot_type_v1(&types, aggregate), None);
    }

    #[test]
    fn retained_initialization_analysis_has_exact_work_and_storage_boundaries() {
        let unit = SemanticTypeIdV1::from_index(0);
        let scalar = SemanticTypeIdV1::from_index(1);
        let source = SemanticSourceProvenanceV1::unavailable();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([216; 32]),
            SemanticLayoutIdentityV1::from_sha256([217; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let local = |tag, ty, role| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag; 32]),
                ty,
                role,
                source,
            )
        };
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        let scalar_constant = || {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                scalar,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(1, 8).unwrap()),
            ))
        };
        let assign_retained = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], scalar).unwrap(),
                SemanticRvalueV1::new(
                    scalar,
                    SemanticRvalueKindV1::Use(scalar_constant()),
                ),
            )),
        );
        let block = |tag, statements, terminator| {
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([tag; 32]),
                source,
                statements,
                SemanticTerminatorV1::new(source, terminator),
            )
            .unwrap()
        };
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([218; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([219; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([220; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([221; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([222; 32]),
            source,
            abi,
            vec![
                local(223, unit, SemanticLocalRoleV1::Return),
                local(224, scalar, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![
                block(
                    225,
                    vec![assign_retained],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: scalar_constant(),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    226,
                    vec![],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(
                    227,
                    vec![],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
                ),
                block(228, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap();
        let slots = BTreeMap::from([(
            ScopedAllocationIdentityV29::LegacyLocal(1),
            SemanticRetainedLocalSlotPlanV1 {
                semantic_type: scalar,
                storage: SemanticRetainedStorageV29::ScalarArray {
                    kernel_type: Type::Scalar(ScalarType::U64),
                    alignment: 8,
                    array: None,
                },
            },
        )]);
        let reachable = BTreeSet::from([0, 1, 2, 3]);

        let mut observed = SemanticRetainedInitializationBudgetV1::new(usize::MAX, usize::MAX);
        let entries = retained_local_initialization_entries_with_budget_v1(
            &function,
            &slots,
            &reachable,
            &mut observed,
        )
        .unwrap();
        assert_eq!(entries.get(&0), Some(&BTreeSet::new()));
        for block in [1, 2, 3] {
            assert_eq!(entries.get(&block), Some(&BTreeSet::from([1])));
        }
        assert!(observed.work > 0);
        assert_eq!(observed.storage, 7, "returned map remains live");
        assert_eq!(observed.peak_storage, 13);

        retained_local_initialization_entries_v1(
            &function,
            &slots,
            &reachable,
            observed.work,
            observed.peak_storage,
        )
        .unwrap();

        let work_limit = observed.work - 1;
        let error = retained_local_initialization_entries_v1(
            &function,
            &slots,
            &reachable,
            work_limit,
            usize::MAX,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual,
                limit,
            } if actual == observed.work && limit == work_limit
        ));

        let storage_limit = observed.peak_storage - 1;
        let error = retained_local_initialization_entries_v1(
            &function,
            &slots,
            &reachable,
            usize::MAX,
            storage_limit,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual,
                limit,
            } if actual == observed.peak_storage && limit == storage_limit
        ));
    }
    include!("semantic_ssa_capability_storage_v29_tests.rs");

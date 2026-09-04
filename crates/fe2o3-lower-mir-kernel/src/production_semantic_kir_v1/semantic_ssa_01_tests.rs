mod semantic_ssa_transport_tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAtomicAccessV1, SemanticBorrowKindV1, SemanticMemoryStoreV1, SemanticPointerTypeV1,
    };

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
                valid[1].clone(),
            ],
            vec![
                valid[0].clone(),
                projection(SemanticProjectionKindV1::Field(1), witness),
            ],
            vec![
                valid[0].clone(),
                projection(SemanticProjectionKindV1::Field(0), u32_ty),
            ],
            vec![
                projection(SemanticProjectionKindV1::Downcast(1), some),
                valid[1].clone(),
            ],
            vec![
                projection(SemanticProjectionKindV1::Downcast(1), u32_ty),
                valid[1].clone(),
            ],
            vec![valid[0].clone()],
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
            1,
            SemanticRetainedLocalSlotPlanV1 {
                semantic_type: scalar,
                kernel_type: Type::Scalar(ScalarType::U64),
                alignment: 8,
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

    #[test]
    fn direct_parameter_transport_preserves_only_its_authenticated_carrier() {
        let semantic_type = SemanticTypeIdV1::from_index(0);
        let slice = Type::slice(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let transport = SemanticPromotedTransportV1::DirectParameter { parameter_local: 3 };
        let direct_parameters = BTreeMap::from([(3, slice.clone())]);

        assert_eq!(
            transport
                .transport_types(&[], semantic_type, &direct_parameters)
                .unwrap(),
            vec![slice.clone()]
        );
        assert_eq!(
            transport
                .transport_values(
                    &SemanticValueBindingV1::Value {
                        id: ValueId(7),
                        ty: slice.clone(),
                    },
                    std::slice::from_ref(&slice),
                )
                .unwrap(),
            vec![(ValueId(7), slice.clone())]
        );
        assert!(
            transport
                .transport_values(
                    &SemanticValueBindingV1::Aggregate(vec![SemanticValueBindingV1::Value {
                        id: ValueId(7),
                        ty: slice.clone(),
                    }]),
                    std::slice::from_ref(&slice),
                )
                .is_err()
        );

        let parameter = ValueDef::new(ValueId(11), slice.clone());
        assert!(matches!(
            transport
                .binding_from_transport(
                    &[],
                    semantic_type,
                    std::slice::from_ref(&parameter),
                    std::slice::from_ref(&slice),
                )
                .unwrap(),
            SemanticValueBindingV1::Value { id: ValueId(11), ty } if ty == slice
        ));
        assert!(
            transport
                .binding_from_transport(
                    &[],
                    semantic_type,
                    &[ValueDef::new(ValueId(12), Type::Scalar(ScalarType::U64))],
                    std::slice::from_ref(&slice),
                )
                .is_err()
        );
    }

    fn test_option_availability_v1() -> SemanticOptionAvailabilityV1 {
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
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(1, option),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let bind_discriminant = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(2, discriminant),
                SemanticRvalueV1::new(
                    discriminant,
                    SemanticRvalueKindV1::Discriminant(place(1, option)),
                ),
            )),
        );
        let blocks = vec![
            block(220, vec![], SemanticTerminatorKindV1::Call(call)),
            block(
                221,
                vec![bind_discriminant],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, discriminant)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 2),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                    )
                    .unwrap(),
                },
            ),
            block(222, vec![], SemanticTerminatorKindV1::Return),
            block(223, vec![], SemanticTerminatorKindV1::Return),
        ];
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([224; 32]),
            SemanticLayoutIdentityV1::from_sha256([225; 32]),
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
            SemanticFunctionIdentityV1::from_sha256([226; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([227; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([228; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([229; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([230; 32]),
            source,
            abi,
            vec![
                local(231, unit, SemanticLocalRoleV1::Return),
                local(232, option, SemanticLocalRoleV1::Temporary),
                local(233, discriminant, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        let option_local = SemanticLocalIdV1::from_index(1);
        SemanticOptionDominanceV1::analyze(
            &function,
            &[fe2o3_mir_model::SemanticOptionProducerV1::new(
                option_local,
                SemanticBlockIdV1::from_index(1),
            )],
        )
        .unwrap()
        .availability(option_local)
        .unwrap()
    }

    #[test]
    fn optional_capability_transport_round_trips_metadata_without_strengthening() {
        let availability = test_option_availability_v1();
        let semantic_type = SemanticTypeIdV1::from_index(0);
        let tiled = SemanticDisjointIndexSpaceV1::Tiled2dIndex1d {
            lanes_per_tile: 64,
            tile_rows: 16,
            tile_columns: 16,
            elements_per_lane: 4,
        };
        let component = SemanticPromotedBindingV1::OptionComponentWitness {
            index_space: tiled,
            availability,
        };
        let component_values = [
            ValueDef::new(ValueId(20), Type::BOOL),
            ValueDef::new(ValueId(21), Type::INDEX),
        ];
        assert_eq!(
            component.transport_types(&[], semantic_type).unwrap(),
            vec![Type::BOOL, Type::INDEX]
        );
        assert!(matches!(
            component
                .binding_from_transport(&[], semantic_type, &component_values)
                .unwrap(),
            SemanticValueBindingV1::OptionComponentWitness {
                present: ValueId(20),
                raw: ValueId(21),
                index_space,
                availability: actual,
            } if index_space == tiled && actual == availability
        ));
        assert!(
            component
                .transport_values(&SemanticValueBindingV1::OptionComponentWitness {
                    present: ValueId(20),
                    raw: ValueId(21),
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                    availability,
                },)
                .is_err()
        );

        for disjoint in [false, true] {
            let index = SemanticPromotedBindingV1::OptionIndexWitness {
                index_space: SemanticDisjointIndexSpaceV1::ShiftedIndex1d { offset: 4 },
                disjoint,
                availability,
            };
            assert!(matches!(
                index
                    .binding_from_transport(&[], semantic_type, &component_values)
                    .unwrap(),
                SemanticValueBindingV1::OptionIndexWitness {
                    disjoint: actual,
                    availability: actual_availability,
                    ..
                } if actual == disjoint && actual_availability == availability
            ));
        }

        let leader = SemanticPromotedBindingV1::OptionGridLeader { availability };
        assert!(matches!(
            leader
                .binding_from_transport(
                    &[],
                    semantic_type,
                    &[ValueDef::new(ValueId(22), Type::BOOL)],
                )
                .unwrap(),
            SemanticValueBindingV1::OptionGridLeader {
                present: ValueId(22),
                availability: actual,
            } if actual == availability
        ));

        let pointer = SemanticPromotedBindingV1::OptionPointer {
            element: ScalarType::F32,
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            availability,
        };
        let pointer_type = Type::pointer(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let pointer_values = [
            ValueDef::new(ValueId(23), Type::BOOL),
            ValueDef::new(ValueId(24), pointer_type.clone()),
        ];
        let pointer_binding = pointer
            .binding_from_transport(&[], semantic_type, &pointer_values)
            .unwrap();
        assert_eq!(
            pointer.transport_values(&pointer_binding).unwrap(),
            vec![
                (ValueId(23), Type::BOOL),
                (ValueId(24), pointer_type.clone()),
            ]
        );
        assert!(
            SemanticPromotedBindingV1::Ordinary
                .transport_values(&pointer_binding)
                .is_err()
        );
    }

    #[test]
    fn promoted_transparent_borrow_uses_the_referent_aggregate_transport() {
        let unit = SemanticTypeIdV1::from_index(0);
        let u64_ty = SemanticTypeIdV1::from_index(1);
        let aggregate_ty = SemanticTypeIdV1::from_index(2);
        let reference_ty = SemanticTypeIdV1::from_index(3);
        let source = SemanticSourceProvenanceV1::unavailable();
        let types = vec![
            unit_type(),
            u64_type(),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([201; 32]),
                SemanticLayoutIdentityV1::from_sha256([202; 32]),
                SemanticTypeLayoutV1::aggregate(
                    Some(16),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![u64_ty, u64_ty]).unwrap(),
                ),
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([203; 32]),
                SemanticLayoutIdentityV1::from_sha256([204; 32]),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1::new_with_kind(
                        aggregate_ty,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            ),
        ];
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let alias = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(3, aggregate_ty),
                SemanticRvalueV1::new(
                    aggregate_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, aggregate_ty))),
                ),
            )),
        );
        let borrow = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(2, reference_ty),
                SemanticRvalueV1::new(
                    reference_ty,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: place(3, aggregate_ty),
                    },
                ),
            )),
        );
        let block = SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([205; 32]),
            source,
            vec![alias, borrow],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap();
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([206; 32]),
            SemanticLayoutIdentityV1::from_sha256([207; 32]),
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
            SemanticFunctionIdentityV1::from_sha256([208; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([209; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([210; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([211; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([212; 32]),
            source,
            abi,
            vec![
                local(213, unit, SemanticLocalRoleV1::Return),
                local(214, aggregate_ty, SemanticLocalRoleV1::Temporary),
                local(215, reference_ty, SemanticLocalRoleV1::Temporary),
                local(216, aggregate_ty, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![block],
        )
        .unwrap();

        let option_dominance = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let promoted = BTreeSet::from([1, 2, 3]);
        let mut capability_origins = SemanticCapabilityOriginResolverV1::new(
            &types,
            &[],
            &function,
            &option_dominance,
            &promoted,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        let (transport_type, binding) = promoted_transport_descriptor_v1(
            &types,
            &function,
            2,
            &BTreeMap::new(),
            &promoted,
            &mut capability_origins,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(transport_type, aggregate_ty);
        assert_eq!(
            binding,
            SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary)
        );
        assert_eq!(
            binding
                .transport_types(&types, transport_type, &BTreeMap::new())
                .unwrap(),
            vec![Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U64),],
        );
        let expected = [Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U64)];
        assert!(matches!(
            binding
                .binding_from_transport(
                    &types,
                    transport_type,
                    &[
                        ValueDef::new(ValueId(40), Type::Scalar(ScalarType::U64)),
                        ValueDef::new(ValueId(41), Type::Scalar(ScalarType::U64)),
                    ],
                    &expected,
                )
                .unwrap(),
            SemanticValueBindingV1::Aggregate(fields) if fields.len() == 2
        ));

        let promoted = BTreeSet::from([2]);
        let mut capability_origins = SemanticCapabilityOriginResolverV1::new(
            &types,
            &[],
            &function,
            &option_dominance,
            &promoted,
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        let (transport_type, binding) = promoted_transport_descriptor_v1(
            &types,
            &function,
            2,
            &BTreeMap::new(),
            &promoted,
            &mut capability_origins,
            &BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(transport_type, reference_ty);
        assert_eq!(
            binding,
            SemanticPromotedTransportV1::Semantic(SemanticPromotedBindingV1::Ordinary)
        );
    }

    #[test]
    fn entry_seeds_only_the_certified_implicit_workgroup_lds_scope() {
        let scope_ty = SemanticTypeIdV1::from_index(0);
        let same_shape_ty = SemanticTypeIdV1::from_index(1);
        let reference_ty = SemanticTypeIdV1::from_index(2);
        let dynamic_lds_ty = SemanticTypeIdV1::from_index(3);
        let storage_ty = SemanticTypeIdV1::from_index(4);
        let source = SemanticSourceProvenanceV1::unavailable();
        let zst = |tag| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag.wrapping_add(1); 32]),
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
            zst(150),
            zst(152),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([154; 32]),
                SemanticLayoutIdentityV1::from_sha256([155; 32]),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Opaque,
            ),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([156; 32]),
                SemanticLayoutIdentityV1::from_sha256([157; 32]),
                SemanticTypeLayoutV1::new(Some(0), 1).unwrap(),
                SemanticTypeShapeV1::Opaque,
            ),
            integer_type(158, false, 32),
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
            SemanticAbiIdentityV1::from_sha256([160; 32]),
            SemanticLayoutIdentityV1::from_sha256([161; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(direct(reference_ty))],
            direct(dynamic_lds_ty),
        )
        .unwrap();
        let callable = SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256([162; 32]),
                SemanticItemDefinitionIdentityV1::from_sha256([163; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([164; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([165; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([166; 32]),
                source,
                intrinsic_abi,
            ),
            operation: SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent {
                scope: scope_ty,
                dynamic_lds: dynamic_lds_ty,
                element_storage: storage_ty,
                elements: 1,
            },
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([167; 32]),
        };
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let borrow = SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(2, reference_ty),
                SemanticRvalueV1::new(
                    reference_ty,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(1, scope_ty),
                    },
                ),
            )),
        );
        let return_edge = SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(1),
        );
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![SemanticOperandV1::Copy(place(2, reference_ty))],
            Some(SemanticCallDestinationV1::new(
                place(4, dynamic_lds_ty),
                return_edge,
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let blocks = vec![
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([168; 32]),
                source,
                vec![borrow],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Call(call)),
            )
            .unwrap(),
            SemanticBasicBlockV1::new(
                SemanticBlockIdentityV1::from_sha256([169; 32]),
                source,
                vec![],
                SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
            )
            .unwrap(),
        ];
        let function_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([170; 32]),
            SemanticLayoutIdentityV1::from_sha256([171; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(same_shape_ty, SemanticAbiPassModeV1::Ignore),
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
            SemanticFunctionIdentityV1::from_sha256([172; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([173; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([174; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([175; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([176; 32]),
            source,
            function_abi,
            vec![
                local(177, same_shape_ty, SemanticLocalRoleV1::Return),
                local(178, scope_ty, SemanticLocalRoleV1::Temporary),
                local(179, reference_ty, SemanticLocalRoleV1::Temporary),
                local(180, same_shape_ty, SemanticLocalRoleV1::Temporary),
                local(181, dynamic_lds_ty, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        let callables = [callable];
        let mut lowering = SemanticFunctionLoweringV1::new(
            &types,
            &callables,
            &function,
            SemanticParameterBindingsV1 {
                declarations: &[],
                values: &[],
                types: &[],
                local_bindings: None,
            },
            None,
            None,
            BTreeSet::new(),
            1,
            false,
            16,
        )
        .unwrap();

        assert_eq!(
            lowering.control_flow_ssa.implicit_entry_locals,
            BTreeSet::from([1])
        );
        assert!(lowering.locals[1].is_none());
        assert!(lowering.locals[3].is_none());
        let mut entry = BasicBlock::new(BlockId(0));
        lowering
            .begin_block(SemanticBlockIdV1::from_index(0), &mut entry)
            .unwrap();
        assert!(matches!(
            lowering.locals[1],
            Some(SemanticValueBindingV1::WorkgroupLdsScope)
        ));
        assert!(lowering.locals[3].is_none());
    }

    fn allocation_provenance_fixture_v1(
        entry_terminator: Terminator,
        merge_terminator: Terminator,
    ) -> Function {
        let slice = Type::slice(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let mut entry = BasicBlock::new(BlockId(0));
        entry.terminator = Some(entry_terminator);
        let mut merge = BasicBlock::new(BlockId(1));
        merge
            .parameters
            .push(ValueDef::new(ValueId(2), slice.clone()));
        merge.terminator = Some(merge_terminator);
        Function::kernel_entry(
            "ssa_allocation_provenance",
            Signature::new(vec![slice.clone(), slice], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![entry, merge],
        )
    }

    fn allocation_origin_v1(function: &Function, value: ValueId) -> Option<u32> {
        let body = function.body.as_ref().unwrap();
        let mut index_budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 256 };
        let index = build_kir_correlation_index(body, 16, &mut index_budget).unwrap();
        let mut provenance_budget = UnsupportedIndexCorrelationBudgetV1 { remaining: 256 };
        external_allocation_parameter_v1(
            function,
            &index,
            value,
            &mut BTreeSet::new(),
            &mut provenance_budget,
        )
    }

    #[test]
    fn allocation_provenance_traverses_ssa_edges_and_loop_backedges() {
        let direct = allocation_provenance_fixture_v1(
            Terminator::Branch {
                target: BlockId(1),
                arguments: vec![ValueId(0)],
            },
            Terminator::Return { values: vec![] },
        );
        assert_eq!(allocation_origin_v1(&direct, ValueId(2)), Some(0));

        let loop_carried = allocation_provenance_fixture_v1(
            Terminator::Branch {
                target: BlockId(1),
                arguments: vec![ValueId(0)],
            },
            Terminator::Branch {
                target: BlockId(1),
                arguments: vec![ValueId(2)],
            },
        );
        assert_eq!(allocation_origin_v1(&loop_carried, ValueId(2)), Some(0));
    }

    #[test]
    fn allocation_provenance_rejects_conflicting_parallel_ssa_edges() {
        let conflicting = allocation_provenance_fixture_v1(
            Terminator::ConditionalBranch {
                condition: ValueId(3),
                then_target: BlockId(1),
                then_arguments: vec![ValueId(0)],
                else_target: BlockId(1),
                else_arguments: vec![ValueId(1)],
            },
            Terminator::Return { values: vec![] },
        );
        assert_eq!(allocation_origin_v1(&conflicting, ValueId(2)), None);
    }

    fn exact_enum_ssa_fixture_v1(
        parallel_variant_edges: bool,
    ) -> (
        Vec<SemanticTypeDeclV1>,
        SemanticFunctionDeclV1,
        SemanticControlFlowSsaPlanV1,
    ) {
        let unit = SemanticTypeIdV1::from_index(0);
        let u32_ty = SemanticTypeIdV1::from_index(1);
        let enum_ty = SemanticTypeIdV1::from_index(2);
        let source = SemanticSourceProvenanceV1::unavailable();
        let types = vec![
            unit_type(),
            unsigned_scalar_type(61, 32),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([62; 32]),
                SemanticLayoutIdentityV1::from_sha256([63; 32]),
                SemanticTypeLayoutV1::new(Some(8), 4).unwrap(),
                SemanticTypeShapeV1::Enum {
                    discriminant: u32_ty,
                    variants: vec![
                        SemanticEnumVariantV1::new(
                            0,
                            SemanticAggregateTypeV1::new(vec![u32_ty]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            1,
                            SemanticAggregateTypeV1::new(vec![u32_ty]).unwrap(),
                        ),
                    ]
                    .into_boxed_slice(),
                },
            ),
        ];
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let field = |variant| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(variant), enum_ty)
                        .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap(),
                ],
                u32_ty,
            )
            .unwrap()
        };
        let scalar = |value| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                u32_ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
            ))
        };
        let assign = |destination, ty, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(ty, value),
                )),
            )
        };
        let enum_variant = |variant, payload| {
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(variant),
                vec![scalar(payload)],
            )
            .unwrap()
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
        let initial_switch = SemanticTerminatorKindV1::SwitchInt {
            discriminant: scalar(1),
            targets: SemanticSwitchTargetsV1::new(
                vec![SemanticSwitchTargetV1::new(
                    1,
                    edge(SemanticEdgeRoleV1::SwitchValue, 1),
                )],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        };
        let variant_one_target = if parallel_variant_edges { 5 } else { 6 };
        let variant_switch = SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Copy(place(3, u32_ty)),
            targets: SemanticSwitchTargetsV1::new(
                vec![
                    SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 5)),
                    SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, variant_one_target),
                    ),
                ],
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 7),
            )
            .unwrap(),
        };
        let blocks = vec![
            block(64, vec![], initial_switch),
            block(
                65,
                vec![assign(place(1, enum_ty), enum_ty, enum_variant(0, 11))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                66,
                vec![assign(place(1, enum_ty), enum_ty, enum_variant(1, 13))],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
            ),
            block(
                67,
                vec![assign(
                    place(2, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Discriminant(place(1, enum_ty)),
                )],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
            ),
            block(
                68,
                vec![assign(
                    place(3, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(2, u32_ty))),
                )],
                variant_switch,
            ),
            block(
                69,
                vec![assign(
                    place(4, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(0))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(
                70,
                vec![assign(
                    place(4, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(71, vec![], SemanticTerminatorKindV1::Unreachable),
        ];
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([72; 32]),
            SemanticLayoutIdentityV1::from_sha256([73; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let locals = [unit, enum_ty, u32_ty, u32_ty, u32_ty]
            .into_iter()
            .enumerate()
            .map(|(local, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([74 + local as u8; 32]),
                    ty,
                    if local == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([79; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([80; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([81; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([82; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([83; 32]),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        let semantic_function = SemanticFunctionIdV1::from_index(0);
        let semantic_ssa = plan_semantic_function_ssa_with_module_v1(
            semantic_function,
            &function,
            &types,
            &[],
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let option_dominance = SemanticOptionDominanceV1::analyze(&function, &[]).unwrap();
        let plan = SemanticControlFlowSsaPlanV1::analyze(
            &types,
            &[],
            &function,
            semantic_function,
            &semantic_ssa,
            &option_dominance,
            &BTreeMap::new(),
            usize::MAX,
            usize::MAX,
        )
        .unwrap();
        (types, function, plan)
    }

    #[test]
    fn enum_variant_facts_follow_exact_ssa_through_split_discriminant_blocks() {
        let (types, function, plan) = exact_enum_ssa_fixture_v1(false);
        let facts =
            analyze_promoted_enum_variants_v1(&types, &function, &plan, usize::MAX, usize::MAX)
                .unwrap();
        let enum_at_zero = plan.entry_value(&function, 5, 1).unwrap();
        let enum_at_one = plan.entry_value(&function, 6, 1).unwrap();

        assert_eq!(enum_at_zero, enum_at_one);
        assert_eq!(facts.get(&(5, enum_at_zero)), Some(&0));
        assert_eq!(facts.get(&(6, enum_at_one)), Some(&1));
        assert_ne!(facts.get(&(5, enum_at_zero)), Some(&1));
        assert_ne!(facts.get(&(6, enum_at_one)), Some(&0));
    }

    #[test]
    fn enum_variant_facts_meet_conflicting_parallel_edges_by_exact_value() {
        let (types, function, plan) = exact_enum_ssa_fixture_v1(true);
        let facts =
            analyze_promoted_enum_variants_v1(&types, &function, &plan, usize::MAX, usize::MAX)
                .unwrap();
        let enum_at_join = plan.entry_value(&function, 5, 1).unwrap();

        assert_eq!(facts.get(&(5, enum_at_join)), None);
    }

    #[test]
    fn edge_defined_enum_value_does_not_inherit_the_overwritten_ssa_sibling() {
        let (types, template_function, template_plan) = exact_enum_ssa_fixture_v1(false);
        let unit = SemanticTypeIdV1::from_index(0);
        let u32_ty = SemanticTypeIdV1::from_index(1);
        let enum_ty = SemanticTypeIdV1::from_index(2);
        let source = SemanticSourceProvenanceV1::unavailable();
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let scalar = |value| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                u32_ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
            ))
        };
        let assign = |destination, ty, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(ty, value),
                )),
            )
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
        let initial = assign(
            place(1, enum_ty),
            enum_ty,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(0),
                vec![scalar(19)],
            )
            .unwrap(),
        );
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![],
            Some(SemanticCallDestinationV1::new(
                place(1, enum_ty),
                edge(SemanticEdgeRoleV1::CallReturn, 1),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let discriminant = assign(
            place(2, u32_ty),
            u32_ty,
            SemanticRvalueKindV1::Discriminant(place(1, enum_ty)),
        );
        let field = |variant| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(variant), enum_ty)
                        .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap(),
                ],
                u32_ty,
            )
            .unwrap()
        };
        let blocks = vec![
            block(84, vec![initial], SemanticTerminatorKindV1::Call(call)),
            block(
                85,
                vec![discriminant],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
            block(
                86,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(2, u32_ty)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 4),
                            ),
                        ],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                    )
                    .unwrap(),
                },
            ),
            block(
                87,
                vec![assign(
                    place(3, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(0))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(
                88,
                vec![assign(
                    place(3, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field(1))),
                )],
                SemanticTerminatorKindV1::Return,
            ),
            block(89, vec![], SemanticTerminatorKindV1::Unreachable),
        ];
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([90; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([91; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([92; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([93; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([94; 32]),
            source,
            template_function.abi().clone(),
            template_function.locals().to_vec(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        assert_eq!(function.abi().return_value().ty(), unit);

        let old_value = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(100));
        let edge_value = SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(101));
        let discriminant_value =
            SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(102));
        let enum_variable = fe2o3_mir_model::SsaVariableIdV1::new(1);
        let mut plan = SemanticControlFlowSsaPlanV1 {
            compiler_issued_bindings: BTreeMap::new(),
            implicit_entry_locals: BTreeSet::new(),
            ssa_value_locals: BTreeSet::from([1, 2]),
            retained_local_slots: BTreeMap::new(),
            retained_initialized_at_entry: BTreeMap::new(),
            promoted: BTreeMap::from([(1, template_plan.promoted[&1].clone())]),
            live_in: (0..6).map(|block| (block, Vec::new())).collect(),
            block_entry_values: BTreeMap::from([
                ((1, 1), edge_value),
                ((2, 2), discriminant_value),
                ((3, 1), edge_value),
                ((4, 1), edge_value),
            ]),
            entry_definitions: BTreeMap::new(),
            definition_values: BTreeMap::from([
                ((0, 1), vec![old_value]),
                ((1, 2), vec![discriminant_value]),
            ]),
            edge_definitions: BTreeMap::new(),
            edge_arguments: BTreeMap::new(),
        };
        plan.edge_definitions
            .insert((0, 0), vec![SsaArgumentV1::new(enum_variable, edge_value)]);
        for edge_id in [(1, 0), (2, 0), (2, 1), (2, 2)] {
            plan.edge_definitions.entry(edge_id).or_default();
        }
        for edge_id in [(0, 0), (1, 0), (2, 0), (2, 1), (2, 2)] {
            plan.edge_arguments.insert(edge_id, Vec::new());
        }

        let facts =
            analyze_promoted_enum_variants_v1(&types, &function, &plan, usize::MAX, usize::MAX)
                .unwrap();
        assert_eq!(facts.get(&(1, old_value)), Some(&0));
        assert_eq!(facts.get(&(1, edge_value)), None);
        assert_eq!(facts.get(&(3, edge_value)), Some(&0));
        assert_eq!(facts.get(&(4, edge_value)), Some(&1));
    }

    #[test]
    fn enum_variant_analysis_enforces_exact_work_and_storage_boundaries() {
        let (types, function, plan) = exact_enum_ssa_fixture_v1(false);
        let succeeds = |work, storage| {
            analyze_promoted_enum_variants_v1(&types, &function, &plan, work, storage)
        };
        let minimum_work = (0..1024)
            .find(|work| succeeds(*work, usize::MAX).is_ok())
            .expect("fixture analysis work must be small and bounded");
        let minimum_storage = (0..1024)
            .find(|storage| succeeds(usize::MAX, *storage).is_ok())
            .expect("fixture analysis storage must be small and bounded");

        assert!(succeeds(minimum_work, minimum_storage).is_ok());
        assert!(matches!(
            succeeds(minimum_work - 1, usize::MAX),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisWork,
                actual,
                limit,
            }) if actual == minimum_work && limit + 1 == minimum_work
        ));
        assert!(matches!(
            succeeds(usize::MAX, minimum_storage - 1),
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::AnalysisStorage,
                actual,
                limit,
            }) if actual == minimum_storage && limit + 1 == minimum_storage
        ));
    }
}

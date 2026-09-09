    #[test]
    fn gfx950_ranked_tensor_authentication_accepts_exact_mixed_profile_only() {
        let fp4 = SemanticMfmaProfileV1::Fp4E2M1F32M16N16K128;
        let fp8 = SemanticMfmaProfileV1::Fp8E4M3F32M16N16K128;
        let lhs_contract = gfx950_mfma_operand_contract(fp4, SemanticMfmaOperandRoleV1::A);
        let rhs_contract = gfx950_mfma_operand_contract(fp8, SemanticMfmaOperandRoleV1::B);
        let accumulator_contract = gfx950_mfma_accumulator_contract(fp4);
        let state_for = |lhs_contract, rhs_contract, accumulator_contract| {
            HashMap::from([
                (
                    0,
                    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::MatrixContext {
                        root: 10,
                    }),
                ),
                (
                    1,
                    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(
                        ProjectedMfmaOperandV1 {
                            contract: lhs_contract,
                            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                            lane_root: 20,
                            allocation: tensor_test_allocation(),
                        },
                    )),
                ),
                (
                    2,
                    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(
                        ProjectedMfmaOperandV1 {
                            contract: rhs_contract,
                            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                            lane_root: 20,
                            allocation: tensor_test_allocation(),
                        },
                    )),
                ),
                (
                    3,
                    ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Accumulator(
                        ProjectedMfmaAccumulatorV1 {
                            contract: accumulator_contract,
                            lane_root: 20,
                            value_root: 30,
                            flow_root: 30,
                        },
                    )),
                ),
            ])
        };

        let authenticated = authenticate_tensor_instruction_v1(
            &tensor_test_call(),
            &state_for(lhs_contract, rhs_contract, accumulator_contract),
            lhs_contract,
            rhs_contract,
            accumulator_contract,
        )
        .unwrap();
        assert_eq!(
                authenticated.contract,
                fe2o3_kernel_ir::TensorLayoutContractV1::
                    gfx950_scaled_mfma_fp4_e2m1_fp8_e4m3_f32_m16n16k128_wave64(),
            );

        let reversed_lhs = gfx950_mfma_operand_contract(fp8, SemanticMfmaOperandRoleV1::A);
        let reversed_rhs = gfx950_mfma_operand_contract(fp4, SemanticMfmaOperandRoleV1::B);
        assert_eq!(
            authenticate_tensor_instruction_v1(
                &tensor_test_call(),
                &state_for(reversed_lhs, reversed_rhs, accumulator_contract),
                reversed_lhs,
                reversed_rhs,
                accumulator_contract,
            ),
            Err("an MFMA call with incompatible instruction profiles"),
        );

        let wrong_accumulator = gfx950_mfma_accumulator_contract(fp8);
        assert_eq!(
            authenticate_tensor_instruction_v1(
                &tensor_test_call(),
                &state_for(lhs_contract, rhs_contract, wrong_accumulator),
                lhs_contract,
                rhs_contract,
                wrong_accumulator,
            ),
            Err("an MFMA call with incompatible instruction profiles"),
        );
    }

    #[test]
    fn gfx950_transpose_zst_recovery_rejects_substitution_and_ambiguity() {
        let exact = ProjectedGfx950TransposeTileV1 {
            state: ProjectedGfx950TransposeStateV1::Published,
            format: SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            lane_root: 11,
            token_root: 12,
            source_allocation: Some(tensor_test_allocation()),
        };
        let zst = SemanticOperandV1::Constant(SemanticConstantV1::new(
            SCALAR_TYPE,
            SemanticConstantValueV1::ZeroSized,
        ));
        let mut state = HashMap::from([(
            0,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Gfx950TransposeTile(exact)),
        )]);

        assert_eq!(
            resolve_gfx950_transpose_tile_v1(
                &zst,
                &state,
                SCALAR_TYPE,
                ProjectedGfx950TransposeStateV1::Published,
                SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            ),
            Some(exact),
        );
        assert!(
            resolve_gfx950_transpose_tile_v1(
                &zst,
                &state,
                SCALAR_TYPE,
                ProjectedGfx950TransposeStateV1::Staged,
                SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            )
            .is_none()
        );
        assert!(
            resolve_gfx950_transpose_tile_v1(
                &zst,
                &state,
                SCALAR_TYPE,
                ProjectedGfx950TransposeStateV1::Published,
                SemanticGfx950LdsTransposeFormatV1::Fp4E2M1,
            )
            .is_none()
        );
        assert!(
            resolve_gfx950_transpose_tile_v1(
                &zst,
                &state,
                ENUM_TYPE,
                ProjectedGfx950TransposeStateV1::Published,
                SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            )
            .is_none()
        );

        state.insert(
            1,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Gfx950TransposeTile(
                ProjectedGfx950TransposeTileV1 {
                    token_root: 13,
                    ..exact
                },
            )),
        );
        assert!(
            resolve_gfx950_transpose_tile_v1(
                &zst,
                &state,
                SCALAR_TYPE,
                ProjectedGfx950TransposeStateV1::Published,
                SemanticGfx950LdsTransposeFormatV1::Fp8E4M3,
            )
            .is_none(),
            "a removed-ZST receiver must recover exactly one live state token",
        );
    }

    #[test]
    fn accumulator_join_preserves_one_authenticated_loop_carried_producer() {
        let accumulator = |value_root, flow_root| {
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Accumulator(
                ProjectedMfmaAccumulatorV1 {
                    contract: mfma_accumulator_contract(),
                    lane_root: 20,
                    value_root,
                    flow_root,
                },
            ))
        };
        let initialized = accumulator(30, 30);
        let loop_result = accumulator(30, 40);
        for (current, incoming) in [(initialized, loop_result), (loop_result, initialized)] {
            assert_eq!(
                merge_capability_values_v1(current, incoming),
                loop_result,
                "the join must be deterministic regardless of predecessor order",
            );
        }

        assert_eq!(
            merge_capability_values_v1(accumulator(30, 40), accumulator(30, 50)),
            ProjectedCapabilityValueV1::Invalid,
            "two competing non-initial producers are not one loop recurrence",
        );
        assert_eq!(
            merge_capability_values_v1(initialized, accumulator(31, 40)),
            ProjectedCapabilityValueV1::Invalid,
            "a changed stable semantic value root must fail closed",
        );
    }

    #[test]
    fn swapped_missing_and_cross_lane_mfma_producers_fail_closed() {
        let call = tensor_test_call();
        let mut state = authenticated_tensor_state(
            SemanticMfmaStorageLayoutV1::RowMajor,
            SemanticMfmaStorageLayoutV1::RowMajor,
        );
        assert!(
            authenticate_tensor_instruction_v1(
                &call,
                &state,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                mfma_accumulator_contract(),
            )
            .unwrap_err()
            .contains("metadata")
        );

        state.remove(&1);
        assert!(
            authenticate_tensor_instruction_v1(
                &call,
                &state,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                mfma_accumulator_contract(),
            )
            .unwrap_err()
            .contains("lhs")
        );

        let mut state = authenticated_tensor_state(
            SemanticMfmaStorageLayoutV1::RowMajor,
            SemanticMfmaStorageLayoutV1::RowMajor,
        );
        let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(rhs)) = state[&2]
        else {
            unreachable!()
        };
        state.insert(
            2,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(
                ProjectedMfmaOperandV1 {
                    lane_root: 21,
                    ..rhs
                },
            )),
        );
        assert!(
            authenticate_tensor_instruction_v1(
                &call,
                &state,
                mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
                mfma_operand_contract(SemanticMfmaOperandRoleV1::B),
                mfma_accumulator_contract(),
            )
            .unwrap_err()
            .contains("authenticated wave64 lane")
        );
    }

    #[test]
    fn result_ok_payloads_require_their_exact_dominating_edges() {
        let carrier = SemanticLocalIdV1::from_index(1);
        let discriminator = SemanticLocalIdV1::from_index(2);
        let discriminator_place = SemanticPlaceV1::new(discriminator, vec![], SCALAR_TYPE).unwrap();
        let result_function = projection_function_with_locals(
            vec![
                block(
                    90,
                    vec![
                        enum_definition(carrier, 0),
                        enum_discriminant(carrier, discriminator),
                    ],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(discriminator_place),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1),
                            )],
                            cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(91, vec![], SemanticTerminatorKindV1::Return),
                block(92, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(90, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(91, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                local(92, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let result_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&result_function, &projection_types_with_enum())
                .unwrap();
        let result_state = HashMap::from([(
            1,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::ViewResult(
                ProjectedMfmaViewV1 {
                    role: SemanticMfmaOperandRoleV1::A,
                    profile: SemanticMfmaProfileV1::Bf16F32M16N16K16,
                    storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
                    allocation: tensor_test_allocation(),
                },
            )),
        )]);
        assert!(matches!(
            capability_origin_from_assignment_operand_v1(
                &tensor_payload(1, 0),
                &result_state,
                &result_dominance,
                SemanticBlockIdV1::from_index(1),
            ),
            Some(ProjectedCapabilityValueV1::Known(
                ProjectedCapabilityOriginV1::View(_)
            ))
        ));
        assert!(
            capability_origin_from_assignment_operand_v1(
                &tensor_payload(1, 0),
                &result_state,
                &result_dominance,
                SemanticBlockIdV1::from_index(2),
            )
            .is_none()
        );
    }

    #[test]
    fn exact_enum_transport_preserves_capability_through_nested_wrappers() {
        let function = projection_function(vec![block(96, vec![], SemanticTerminatorKindV1::Return)]);
        let enum_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let origin = ProjectedCapabilityOriginV1::Operand(ProjectedMfmaOperandV1 {
            contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            lane_root: 20,
            allocation: tensor_test_allocation(),
        });
        let mut state = HashMap::from([(0, ProjectedCapabilityValueV1::Known(origin))]);
        let first = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(0),
            vec![tensor_operand(0)],
        )
        .unwrap();
        let first = capability_origin_from_enum_aggregate_v1(
            &first,
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap()
        .unwrap();
        state.insert(1, first);
        let second = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(1),
            vec![tensor_operand(1)],
        )
        .unwrap();
        let second = capability_origin_from_enum_aggregate_v1(
            &second,
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap()
        .unwrap();
        state.insert(2, second);

        let first_again = capability_origin_from_assignment_operand_v1(
            &tensor_payload(2, 1),
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap();
        assert_eq!(first_again, first);
        state.insert(3, first_again);
        assert_eq!(
            capability_origin_from_assignment_operand_v1(
                &tensor_payload(3, 0),
                &state,
                &enum_dominance,
                SemanticBlockIdV1::from_index(0),
            ),
            Some(ProjectedCapabilityValueV1::Known(origin))
        );
    }

    #[test]
    fn enum_transport_rejects_wrong_variant_extra_fields_and_bypass_join() {
        let function = projection_function(vec![block(97, vec![], SemanticTerminatorKindV1::Return)]);
        let enum_dominance =
            SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
        let origin = ProjectedCapabilityOriginV1::Operand(ProjectedMfmaOperandV1 {
            contract: mfma_operand_contract(SemanticMfmaOperandRoleV1::A),
            storage_layout: SemanticMfmaStorageLayoutV1::RowMajor,
            lane_root: 20,
            allocation: tensor_test_allocation(),
        });
        let state = HashMap::from([(0, ProjectedCapabilityValueV1::Known(origin))]);
        let aggregate = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(4),
            vec![tensor_operand(0)],
        )
        .unwrap();
        let wrapped = capability_origin_from_enum_aggregate_v1(
            &aggregate,
            &state,
            &enum_dominance,
            SemanticBlockIdV1::from_index(0),
        )
        .unwrap()
        .unwrap();
        let wrapped_state = HashMap::from([(1, wrapped)]);
        assert!(
            capability_origin_from_assignment_operand_v1(
                &tensor_payload(1, 3),
                &wrapped_state,
                &enum_dominance,
                SemanticBlockIdV1::from_index(0),
            )
            .is_none()
        );

        let extra_fields = SemanticAggregateRvalueV1::new(
            SemanticAggregateKindV1::EnumVariant(4),
            vec![tensor_operand(0), tensor_operand(0)],
        )
        .unwrap();
        assert!(
            capability_origin_from_enum_aggregate_v1(
                &extra_fields,
                &state,
                &enum_dominance,
                SemanticBlockIdV1::from_index(0),
            )
            .unwrap()
            .is_none()
        );

        let mut joined = wrapped_state;
        assert!(merge_capability_states_v1(&mut joined, &HashMap::new()).unwrap());
        assert_eq!(joined[&1], ProjectedCapabilityValueV1::Invalid);
    }

    #[test]
    fn enum_transport_nesting_has_an_explicit_resource_bound() {
        let origin = ProjectedCapabilityOriginV1::Lane {
            root: 1,
            wave_width: 64,
        };
        let mut value = ProjectedCapabilityValueV1::Known(origin);
        for variant in 0..MAX_PROJECTED_CAPABILITY_ENUM_DEPTH_V1 {
            value = wrap_capability_enum_value_v1(value, variant as u32).unwrap();
        }
        assert!(matches!(
            wrap_capability_enum_value_v1(value, 99),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability enum transport exceeds the charged nesting limit"
            ))
        ));
    }

    fn move_local_operand(local: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Move(
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE).unwrap(),
        )
    }

    fn capability_state_origin() -> ProjectedCapabilityValueV1 {
        ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Lane {
            root: 31,
            wave_width: 64,
        })
    }

    #[test]
    fn copy_and_move_transfer_capability_exactly_once() {
        let place = |local| SemanticPlaceV1::new(local, vec![], SCALAR_TYPE).unwrap();
        let assignment = |destination, operand| {
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(destination),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(operand)),
            )))
        };
        for first in [tensor_operand(1), move_local_operand(1)] {
            let function = projection_function_with_locals(
                vec![block(
                    102,
                    vec![
                        assignment(SemanticLocalIdV1::from_index(2), first),
                        assignment(SemanticLocalIdV1::from_index(3), tensor_operand(1)),
                    ],
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(102, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(103, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(104, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                    local(105, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let payload =
                SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types()).unwrap();
            let mut state = HashMap::from([(1, capability_state_origin())]);
            transfer_capability_statements_v1(&[], &function, 0, &mut state, &payload, None).unwrap();
            assert_eq!(state[&1], ProjectedCapabilityValueV1::Invalid);
            assert_eq!(state[&2], capability_state_origin());
            assert_eq!(state[&3], ProjectedCapabilityValueV1::Invalid);
        }
    }

    #[test]
    fn partial_assignment_discriminant_and_deinitialize_invalidate_enum_transport() {
        let payload_place = match tensor_payload(1, 4) {
            SemanticOperandV1::Move(place) => place,
            _ => unreachable!(),
        };
        let statements = [
            statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                payload_place.clone(),
                SemanticRvalueV1::new(SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0))),
            ))),
            statement(SemanticStatementKindV1::SetDiscriminant {
                place: SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], ENUM_TYPE)
                    .unwrap(),
                variant_index: 5,
            }),
            statement(SemanticStatementKindV1::Deinitialize(payload_place)),
        ];
        for invalidating_statement in statements {
            let function = projection_function_with_locals(
                vec![block(
                    106,
                    vec![invalidating_statement],
                    SemanticTerminatorKindV1::Return,
                )],
                vec![
                    local(106, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                    local(107, ENUM_TYPE, SemanticLocalRoleV1::Temporary),
                ],
            );
            let payload =
                SemanticEnumPayloadDominanceV1::analyze(&function, &projection_types_with_enum())
                    .unwrap();
            let wrapped = wrap_capability_enum_value_v1(capability_state_origin(), 4).unwrap();
            let mut state = HashMap::from([(1, wrapped)]);
            transfer_capability_statements_v1(&[], &function, 0, &mut state, &payload, None).unwrap();
            assert_eq!(state[&1], ProjectedCapabilityValueV1::Invalid);
            assert_eq!(
                capability_origin_from_assignment_operand_v1(
                    &tensor_payload(1, 4),
                    &state,
                    &payload,
                    SemanticBlockIdV1::from_index(0),
                ),
                Some(ProjectedCapabilityValueV1::Invalid)
            );
        }
    }

    #[test]
    fn call_operands_consume_capabilities_even_without_a_known_producer() {
        let call = SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(0),
            vec![move_local_operand(1)],
            None,
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        let function = projection_function_with_locals(
            vec![block(108, vec![], SemanticTerminatorKindV1::Call(call))],
            vec![
                local(108, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(109, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let mut state = HashMap::from([(1, capability_state_origin())]);
        assert_eq!(
            transfer_capability_terminator_v1(
                &[],
                &function,
                0,
                &mut state,
                &[None; 4],
                &[],
                &[],
                &HashMap::new(),
                false
            )
            .unwrap(),
            ProjectedCapabilityTerminatorEffectsV1::default()
        );
        assert_eq!(state[&1], ProjectedCapabilityValueV1::Invalid);
    }

    #[test]
    fn capability_on_only_one_predecessor_becomes_invalid_at_the_join() {
        let mut current = HashMap::from([(
            7,
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Lane {
                root: 1,
                wave_width: 64,
            }),
        )]);
        assert!(merge_capability_states_v1(&mut current, &HashMap::new()).unwrap());
        assert_eq!(current[&7], ProjectedCapabilityValueV1::Invalid);
    }

    #[test]
    fn capability_state_storage_accepts_the_exact_bound_before_fallible_clone() {
        assert_eq!(
            checked_capability_stored_entries_v1(MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1 - 1, 1,)
                .unwrap(),
            MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1,
        );
        assert!(matches!(
            checked_capability_stored_entries_v1(MAX_PROJECTED_CAPABILITY_STATE_ENTRIES_V1, 1,),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability states exceed the charged storage limit"
            ))
        ));
        assert!(matches!(
            checked_capability_stored_entries_v1(usize::MAX, 1),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability stored-state accounting overflow"
            ))
        ));

        let state = HashMap::from([(1, capability_state_origin())]);
        assert_eq!(try_clone_capability_state_v1(&state).unwrap(), state);
    }

    #[test]
    fn duplicate_capability_cfg_successors_are_charged_once_and_merged_once() {
        let targets = (0..65_536_u128)
            .map(|value| {
                SemanticSwitchTargetV1::new(value, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1))
            })
            .collect();
        let terminator = SemanticTerminatorKindV1::SwitchInt {
            discriminant: constant(0),
            targets: SemanticSwitchTargetsV1::new(
                targets,
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        };
        let mut work = 0;
        assert_eq!(
            charged_unique_capability_successors_v1(&terminator, 7, &mut work).unwrap(),
            vec![1, 2]
        );
        assert_eq!(work, 65_537 + 2 * (7 + 1));
    }

    #[test]
    fn capability_cfg_successor_deduplication_is_deterministic_and_resource_bounded() {
        let terminator = SemanticTerminatorKindV1::SwitchInt {
            discriminant: constant(0),
            targets: SemanticSwitchTargetsV1::new(
                vec![
                    SemanticSwitchTargetV1::new(0, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                    SemanticSwitchTargetV1::new(1, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 1)),
                    SemanticSwitchTargetV1::new(2, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                    SemanticSwitchTargetV1::new(3, cfg_edge(SemanticEdgeRoleV1::SwitchValue, 2)),
                ],
                cfg_edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
            )
            .unwrap(),
        };
        for _ in 0..4 {
            let mut work = 0;
            assert_eq!(
                charged_unique_capability_successors_v1(&terminator, 0, &mut work).unwrap(),
                vec![1, 2, 3]
            );
            assert_eq!(work, 8);
        }

        let mut exhausted_work = MAX_PROJECTED_CAPABILITY_DATAFLOW_WORK_V1 - 1;
        assert!(matches!(
            charged_unique_capability_successors_v1(&terminator, 0, &mut exhausted_work),
            Err(ProductionRankedProjectionErrorV1::Unsupported(
                "capability dataflow exceeds the charged projection limit"
            ))
        ));
    }

    #[test]
    fn uniform_switch_projection_accepts_only_immutable_arguments_or_constants() {
        let function = projection_function_with_locals(
            vec![block(93, vec![], SemanticTerminatorKindV1::Return)],
            vec![
                local(93, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(94, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(95, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        );
        let origins = local_stable_argument_origins(&projection_types(), &function).unwrap();
        let mut arguments = vec![None; function.locals().len()];
        let mut next_argument = 1;
        let mut operations = Vec::new();
        let mut next_value = 0;
        assert!(matches!(
            project_uniform_switch_operand_v1(
                &tensor_operand(1),
                &[None; 3],
                &origins,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            )
            .unwrap(),
            Some(ProductionRankedValueV1::Argument(1))
        ));
        assert!(
            project_uniform_switch_operand_v1(
                &tensor_operand(2),
                &[None; 3],
                &origins,
                &mut arguments,
                &mut next_argument,
                &mut operations,
                &mut next_value,
            )
            .unwrap()
            .is_none()
        );
    }

    fn project_intrinsic_contracts_for_test(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Result<IntrinsicProjectionV1, ProductionRankedProjectionErrorV1> {
        let constants = constant_locals(function)?;
        let mut operations = Vec::new();
        let mut next_value = 0;
        let mut ranked_ir = String::new();
        let callable_effects = DefinedCallableEmptyEffectSummariesV1 {
            decisions: Box::new([]),
        };
        project_intrinsic_contracts(
            callables,
            &callable_effects,
            types,
            function,
            None,
            Some(64),
            &constants,
            &mut operations,
            &mut next_value,
            &mut ranked_ir,
        )
    }

    // assertion_proof_types ends at U128; optional_selector_types has four more entries.
    const CAP_INDEX_CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(U128_TYPE.index() + 1);
    const CAP_INDEX_INVOCATION: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 1);
    const CAP_INDEX_RECEIVER: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 2);
    const CAP_INDEX_WITNESS: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 3);
    const CAP_INDEX_BORROW: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 4);
    const CAP_INDEX_DISJOINT: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 5);
    const CAP_INDEX_DISJOINT_BORROW: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 6);
    const CAP_INDEX_CONTEXT_BORROW: SemanticTypeIdV1 =
        SemanticTypeIdV1::from_index(CAP_INDEX_CONTEXT.index() + 7);

    fn capability_index_provenance(mutation: Option<usize>) -> SemanticKernelCapabilityProvenanceV1 {
        let identity = |axis| {
            bytes(if mutation == Some(axis) {
                180
            } else {
                160 + axis as u8
            })
        };
        SemanticKernelCapabilityProvenanceV1::new(
            SemanticFunctionIdV1::from_index(u32::from(mutation == Some(0))),
            SemanticKernelBindingIdentityV1::from_sha256(identity(1)),
            SemanticKernelCapabilityFrontendUnitIdentityV1::from_sha256(identity(2)),
            SemanticTypeIdentityV1::from_sha256(identity(3)),
            SemanticKernelCapabilityTargetBrandIdentityV1::from_sha256(identity(4)),
            SemanticKernelCapabilityLaunchBrandIdentityV1::from_sha256(identity(5)),
            SemanticKernelCapabilityIssuanceIdentityV1::from_sha256(identity(6)),
        )
        .unwrap()
    }

    fn capability_index_types() -> Vec<SemanticTypeDeclV1> {
        let mut types = assertion_proof_types();
        assert_eq!(types.len(), CAP_INDEX_CONTEXT.index() as usize);
        types.push(exact_transparent_u16_marker_decl_v1(170));
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256(bytes(171)),
            SemanticLayoutIdentityV1::from_sha256(bytes(171)),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                8,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U64_TYPE]).unwrap()),
        ));
        let reference = |tag, pointee| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        SemanticPointerKindV1::Reference,
                        SemanticMutabilityV1::Immutable,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            )
        };
        let witness = |tag| {
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(tag)),
                SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
                SemanticTypeLayoutV1::aggregate(
                    Some(8),
                    8,
                    SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
                )
                .unwrap(),
                SemanticTypeShapeV1::Aggregate(
                    SemanticAggregateTypeV1::new(vec![U64_TYPE, CAP_INDEX_CONTEXT]).unwrap(),
                ),
            )
        };
        types.push(reference(172, CAP_INDEX_INVOCATION));
        types.push(witness(173));
        types.push(reference(174, CAP_INDEX_WITNESS));
        types.push(witness(175));
        types.push(reference(176, CAP_INDEX_DISJOINT));
        types.push(reference(177, CAP_INDEX_CONTEXT));
        assert_eq!(types.len(), CAP_INDEX_CONTEXT_BORROW.index() as usize + 1);
        types
    }

    fn capability_index_callable(
        operation: SemanticCompilerIntrinsicOperationV1,
        inputs: &[(SemanticTypeIdV1, SemanticSourceArgumentOwnershipV1)],
        output: SemanticTypeIdV1,
    ) -> SemanticCallableDeclV1 {
        let direct = |ty| {
            SemanticAbiValueV1::new(
                ty,
                SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
            )
        };
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256(bytes(178)),
            SemanticLayoutIdentityV1::from_sha256(bytes(178)),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            inputs.iter().map(|(ty, _)| direct(*ty)).collect(),
            direct(output),
        )
        .unwrap()
        .with_source_argument_ownership(inputs.iter().map(|(_, ownership)| *ownership).collect())
        .unwrap();
        SemanticCallableDeclV1::CompilerIntrinsic {
            binding: SemanticNonBodyCallableBindingV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(116)),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(117)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(118)),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(119)),
                SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(120)),
                SemanticSourceProvenanceV1::unavailable(),
                abi,
            ),
            operation,
            operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256(bytes(121)),
        }
    }

    fn capability_index_fixture() -> (
        Vec<SemanticTypeDeclV1>,
        Vec<SemanticCallableDeclV1>,
        SemanticFunctionDeclV1,
    ) {
        use SemanticCompilerIntrinsicOperationV1 as Op;
        use SemanticSourceArgumentOwnershipV1::{ByValue, SharedBorrow};
        let source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(116));
        let provenance = capability_index_provenance(None);
        let callables = vec![
            compiler_intrinsic_callable(Op::KernelContextIssue {
                context: CAP_INDEX_CONTEXT,
            }),
            compiler_intrinsic_callable(Op::CapabilityGlobalBindReadOnly {
                context: CAP_INDEX_CONTEXT,
                physical: U64_POINTER_TYPE,
                view: ARRAY_TYPE,
                element: U64_TYPE,
                contract: SemanticCapabilityMemoryContractV1::global_read_only(),
                provenance,
                source_identity,
            }),
            capability_index_callable(
                Op::CapabilityInvocationIndex1d {
                    invocation: CAP_INDEX_INVOCATION,
                    index_witness: CAP_INDEX_WITNESS,
                    raw_index: U64_TYPE,
                    provenance,
                    source_identity,
                },
                &[(CAP_INDEX_RECEIVER, SharedBorrow)],
                CAP_INDEX_WITNESS,
            ),
            capability_index_callable(
                Op::ThreadIndexGet {
                    index_witness: CAP_INDEX_WITNESS,
                    raw_index: U64_TYPE,
                },
                &[(CAP_INDEX_BORROW, SharedBorrow)],
                U64_TYPE,
            ),
            capability_index_callable(
                Op::ThreadIndexIntoDisjoint {
                    input_witness: CAP_INDEX_WITNESS,
                    output_witness: CAP_INDEX_DISJOINT,
                    raw_index: U64_TYPE,
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                },
                &[(CAP_INDEX_WITNESS, ByValue)],
                CAP_INDEX_DISJOINT,
            ),
            capability_index_callable(
                Op::DisjointIndexGet {
                    index_witness: CAP_INDEX_DISJOINT,
                    raw_index: U64_TYPE,
                    index_space: SemanticDisjointIndexSpaceV1::Index1d,
                },
                &[(CAP_INDEX_DISJOINT_BORROW, SharedBorrow)],
                U64_TYPE,
            ),
        ];
        let call = |callee, arguments, destination, ty, target| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        typed_place(destination, ty),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let borrow = |destination, ty, source, source_ty| {
            typed_assignment(
                destination,
                ty,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: typed_place(source, source_ty),
                },
            )
        };
        let function = projection_function_with_locals(
            vec![
                block(160, vec![], call(0, vec![], 1, CAP_INDEX_CONTEXT, 1)),
                block(
                    161,
                    vec![borrow(10, CAP_INDEX_CONTEXT_BORROW, 1, CAP_INDEX_CONTEXT)],
                    call(
                        1,
                        vec![typed_operand(10, CAP_INDEX_CONTEXT_BORROW)],
                        11,
                        ARRAY_TYPE,
                        2,
                    ),
                ),
                block(
                    162,
                    vec![
                        typed_assignment(
                            2,
                            CAP_INDEX_INVOCATION,
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Aggregate,
                                    vec![typed_constant(U64_TYPE, 0, 8)],
                                )
                                .unwrap(),
                            ),
                        ),
                        borrow(3, CAP_INDEX_RECEIVER, 2, CAP_INDEX_INVOCATION),
                    ],
                    call(
                        2,
                        vec![typed_operand(3, CAP_INDEX_RECEIVER)],
                        4,
                        CAP_INDEX_WITNESS,
                        3,
                    ),
                ),
                block(
                    163,
                    vec![borrow(5, CAP_INDEX_BORROW, 4, CAP_INDEX_WITNESS)],
                    call(3, vec![typed_operand(5, CAP_INDEX_BORROW)], 6, U64_TYPE, 4),
                ),
                block(
                    164,
                    vec![],
                    call(
                        4,
                        vec![SemanticOperandV1::Move(typed_place(4, CAP_INDEX_WITNESS))],
                        7,
                        CAP_INDEX_DISJOINT,
                        5,
                    ),
                ),
                block(
                    165,
                    vec![borrow(8, CAP_INDEX_DISJOINT_BORROW, 7, CAP_INDEX_DISJOINT)],
                    call(
                        5,
                        vec![typed_operand(8, CAP_INDEX_DISJOINT_BORROW)],
                        9,
                        U64_TYPE,
                        6,
                    ),
                ),
                block(166, vec![], SemanticTerminatorKindV1::Return),
            ],
            [
                SCALAR_TYPE,
                CAP_INDEX_CONTEXT,
                CAP_INDEX_INVOCATION,
                CAP_INDEX_RECEIVER,
                CAP_INDEX_WITNESS,
                CAP_INDEX_BORROW,
                U64_TYPE,
                CAP_INDEX_DISJOINT,
                CAP_INDEX_DISJOINT_BORROW,
                U64_TYPE,
                CAP_INDEX_CONTEXT_BORROW,
                ARRAY_TYPE,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                local(
                    180 + index as u8,
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                )
            })
            .collect(),
        );
        (capability_index_types(), callables, function)
    }

    fn project_capability_index_fixture(
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
    ) -> Result<
        (IntrinsicProjectionV1, Vec<ProductionRankedOperationV1>),
        ProductionRankedProjectionErrorV1,
    > {
        let root = capability_index_provenance(None);
        let provenance = root_invocation_provenance_v1(
            types,
            callables,
            function,
            root.root(),
            root.kernel_binding(),
        )?;
        let mut operations = Vec::new();
        let projected = project_intrinsic_contracts(
            callables,
            &DefinedCallableEmptyEffectSummariesV1 {
                decisions: Box::new([]),
            },
            types,
            function,
            provenance,
            Some(64),
            &constant_locals(function)?,
            &mut operations,
            &mut 0,
            &mut String::new(),
        )?;
        Ok((projected, operations))
    }

    #[test]
    fn capability_invocation_index_preserves_borrow_get_and_into_disjoint_mapping() {
        let (types, callables, function) = capability_index_fixture();
        let (projection, operations) =
            project_capability_index_fixture(&types, &callables, &function).unwrap();
        let witness = projection.index_values[4].expect("authenticated invocation witness");
        assert_eq!(witness.mapping, SemanticDisjointIndexSpaceV1::Index1d);
        assert_eq!(witness.precondition, None);
        assert_eq!(witness.availability, None);
        for local in [5, 6, 7, 8, 9] {
            assert_eq!(
                projection.index_values[local],
                Some(witness),
                "local {local}"
            );
        }
        assert!(
            projection.index_values[3].is_none(),
            "receiver borrow is not itself an index"
        );
        assert_eq!(
            operations
                .iter()
                .filter(|op| matches!(
                    op,
                    ProductionRankedOperationV1::InvocationIndex {
                        dimension: 0,
                        launch_extent: 0,
                        ..
                    }
                ))
                .count(),
            1
        );
    }

    #[test]
    fn capability_invocation_index_rejects_each_provenance_axis_and_source_substitution() {
        let (types, callables, function) = capability_index_fixture();
        for axis in 0..8 {
            let mut changed = callables.clone();
            let SemanticCallableDeclV1::CompilerIntrinsic {
                operation:
                    SemanticCompilerIntrinsicOperationV1::CapabilityInvocationIndex1d {
                        provenance,
                        source_identity,
                        ..
                    },
                ..
            } = &mut changed[2]
            else {
                unreachable!()
            };
            if axis < 7 {
                *provenance = capability_index_provenance(Some(axis));
            } else {
                *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(200));
            }
            assert_incomplete(
                project_capability_index_fixture(&types, &changed, &function),
                "an invocation index changed its root provenance or source binding",
            );
        }
    }

    #[test]
    fn capability_invocation_index_rejects_missing_or_conflicting_root_anchor() {
        let (types, callables, function) = capability_index_fixture();
        for axis in 0..2 {
            let mut changed = callables.clone();
            for index in [1, 2] {
                let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut changed[index]
                else {
                    unreachable!()
                };
                match operation {
                    SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                        provenance,
                        ..
                    }
                    | SemanticCompilerIntrinsicOperationV1::CapabilityInvocationIndex1d {
                        provenance,
                        ..
                    } => {
                        *provenance = capability_index_provenance(Some(axis));
                    }
                    _ => unreachable!(),
                }
            }
            // Root and binding are independently selected, not taken from either terminal.
            assert_incomplete(
                project_capability_index_fixture(&types, &changed, &function),
                "an invocation index has conflicting root context provenance",
            );
        }
        for index in [0, 1] {
            let mut changed = callables.clone();
            changed[index] = compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::FabsF32);
            assert_incomplete(
                project_capability_index_fixture(&types, &changed, &function),
                if index == 0 {
                    "an invocation index has conflicting root context provenance"
                } else {
                    "an invocation index lacks an independent root context bind"
                },
            );
        }
        let mut changed = callables.clone();
        let SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                SemanticCompilerIntrinsicOperationV1::CapabilityGlobalBindReadOnly {
                    source_identity, ..
                },
            ..
        } = &mut changed[1]
        else {
            unreachable!()
        };
        *source_identity = SemanticFunctionIdentityV1::from_sha256(bytes(201));
        assert_incomplete(
            project_capability_index_fixture(&types, &changed, &function),
            "an invocation index has conflicting root context provenance",
        );
        assert_incomplete(
            project_intrinsic_contracts_for_test(&types, &callables, &function),
            "an invocation index changed its root provenance or source binding",
        );
    }

    #[test]
    fn capability_invocation_index_rejects_receiver_abi_and_mapping_changes() {
        let (types, callables, function) = capability_index_fixture();
        let SemanticTerminatorKindV1::Call(call) = function.blocks()[2].terminator().kind() else {
            unreachable!()
        };
        let provenance = Some(capability_index_provenance(None));
        for (ty, ownership, output) in [
            (
                CAP_INDEX_INVOCATION,
                SemanticSourceArgumentOwnershipV1::ByValue,
                CAP_INDEX_WITNESS,
            ),
            (
                CAP_INDEX_RECEIVER,
                SemanticSourceArgumentOwnershipV1::ByValue,
                CAP_INDEX_WITNESS,
            ),
            (
                CAP_INDEX_RECEIVER,
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                U64_TYPE,
            ),
        ] {
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callables[2] else {
                unreachable!()
            };
            let changed = capability_index_callable(operation, &[(ty, ownership)], output);
            assert_incomplete(
                validate_capability_invocation_index_v1(&types, &changed, call, provenance),
                "an invocation index changed its borrowed receiver or witness ABI",
            );
        }
        for index in [4, 5] {
            let mut changed = callables.clone();
            let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = &mut changed[index]
            else {
                unreachable!()
            };
            let (SemanticCompilerIntrinsicOperationV1::ThreadIndexIntoDisjoint { index_space, .. }
            | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { index_space, .. }) = operation
            else {
                unreachable!()
            };
            *index_space = SemanticDisjointIndexSpaceV1::GridExclusive;
            assert_incomplete(
                project_capability_index_fixture(&types, &changed, &function),
                "an index identity transform changed its mapping",
            );
        }
    }

    #[test]
    fn capability_invocation_index_rejects_missing_mutable_raw_and_cross_brand_receivers() {
        let (types, callables, function) = capability_index_fixture();
        let SemanticTerminatorKindV1::Call(call) = function.blocks()[2].terminator().kind() else {
            unreachable!()
        };
        let provenance = Some(capability_index_provenance(None));
        let missing = SemanticDirectCallV1::new_callable(
            call.callee(),
            vec![],
            call.destination().cloned(),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap();
        assert_incomplete(
            validate_capability_invocation_index_v1(&types, &callables[2], &missing, provenance),
            "an invocation index requires one shared borrowed receiver",
        );
        for (kind, mutability, pointee) in [
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
                CAP_INDEX_INVOCATION,
            ),
            (
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                CAP_INDEX_INVOCATION,
            ),
            (
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                CAP_INDEX_CONTEXT,
            ),
        ] {
            let mut changed = types.clone();
            changed[CAP_INDEX_RECEIVER.index() as usize] = SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256(bytes(202)),
                SemanticLayoutIdentityV1::from_sha256(bytes(202)),
                SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointee,
                        kind,
                        mutability,
                        0,
                        64,
                        SemanticPointerMetadataV1::None,
                    )
                    .unwrap(),
                ),
            );
            assert_incomplete(
                project_capability_index_fixture(&changed, &callables, &function),
                "an invocation index changed its borrowed receiver or witness ABI",
            );
        }
    }

    #[derive(Clone, Copy)]
    enum CachedIndexMutationV1 {
        SharedBorrow,
        Redefined,
        AddressEscaped,
    }

    fn cached_index_mutation_function(shape: CachedIndexMutationV1) -> SemanticFunctionDeclV1 {
        let call = |callee, arguments, destination, target| {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(callee),
                    arguments,
                    Some(SemanticCallDestinationV1::new(
                        typed_place(destination, SCALAR_TYPE),
                        cfg_edge(SemanticEdgeRoleV1::CallReturn, target),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        };
        let mutation = match shape {
            CachedIndexMutationV1::SharedBorrow => typed_assignment(
                3,
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: typed_place(2, SCALAR_TYPE),
                },
            ),
            CachedIndexMutationV1::Redefined => {
                typed_assignment(2, SCALAR_TYPE, SemanticRvalueKindV1::Use(constant(0)))
            }
            CachedIndexMutationV1::AddressEscaped => typed_assignment(
                3,
                POINTER_TYPE,
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: typed_place(2, SCALAR_TYPE),
                },
            ),
        };
        projection_function_with_locals(
            vec![
                block(130, vec![], call(0, vec![], 1, 1)),
                block(
                    131,
                    vec![],
                    call(1, vec![typed_operand(1, SCALAR_TYPE)], 2, 2),
                ),
                block(132, vec![mutation], zero_switch(2, SCALAR_TYPE, 3, 4)),
                block(133, vec![], SemanticTerminatorKindV1::Return),
                block(134, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(130, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(131, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(132, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(133, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    #[test]
    fn mutated_cached_indices_fail_closed_in_production_intrinsic_projection() {
        let callables = [
            compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                index_witness: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            }),
            compiler_intrinsic_callable(SemanticCompilerIntrinsicOperationV1::ThreadIndexGet {
                index_witness: SCALAR_TYPE,
                raw_index: SCALAR_TYPE,
            }),
        ];
        project_intrinsic_contracts_for_test(
            &projection_types(),
            &callables,
            &cached_index_mutation_function(CachedIndexMutationV1::SharedBorrow),
        )
        .unwrap();
        for shape in [
            CachedIndexMutationV1::Redefined,
            CachedIndexMutationV1::AddressEscaped,
        ] {
            assert_incomplete(
                project_intrinsic_contracts_for_test(
                    &projection_types(),
                    &callables,
                    &cached_index_mutation_function(shape),
                ),
                "an index capability local has mutable or address-escaped value semantics",
            );
        }
    }

    #[derive(Clone, Copy)]
    enum DirectPredicateMutationV1 {
        Stable,
        DestinationOverwrite,
        ReassignedArgument,
        EscapedAlias,
    }

    fn mutated_direct_predicate_function(shape: DirectPredicateMutationV1) -> SemanticFunctionDeclV1 {
        let mut statements = Vec::new();
        let left = match shape {
            DirectPredicateMutationV1::Stable | DirectPredicateMutationV1::DestinationOverwrite => {
                typed_operand(2, SCALAR_TYPE)
            }
            DirectPredicateMutationV1::ReassignedArgument => {
                statements.push(typed_assignment(
                    2,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Add,
                        left: typed_operand(2, SCALAR_TYPE),
                        right: constant(1),
                    },
                ));
                typed_operand(2, SCALAR_TYPE)
            }
            DirectPredicateMutationV1::EscapedAlias => {
                statements.push(typed_assignment(
                    4,
                    SCALAR_TYPE,
                    SemanticRvalueKindV1::Use(typed_operand(2, SCALAR_TYPE)),
                ));
                statements.push(typed_assignment(
                    5,
                    POINTER_TYPE,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: typed_place(4, SCALAR_TYPE),
                    },
                ));
                typed_operand(4, SCALAR_TYPE)
            }
        };
        statements.push(typed_assignment(
            1,
            BOOL_TYPE,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::LessThan,
                left,
                right: typed_operand(3, SCALAR_TYPE),
            },
        ));
        if matches!(shape, DirectPredicateMutationV1::DestinationOverwrite) {
            statements.push(typed_assignment(
                1,
                BOOL_TYPE,
                SemanticRvalueKindV1::Use(typed_constant(BOOL_TYPE, 1, 1)),
            ));
        }
        projection_function_with_locals(
            vec![
                block(135, statements, zero_switch(1, BOOL_TYPE, 1, 2)),
                block(136, vec![], SemanticTerminatorKindV1::Return),
                block(137, vec![], SemanticTerminatorKindV1::Return),
            ],
            vec![
                local(135, SCALAR_TYPE, SemanticLocalRoleV1::Return),
                local(136, BOOL_TYPE, SemanticLocalRoleV1::Temporary),
                local(137, SCALAR_TYPE, SemanticLocalRoleV1::Argument(0)),
                local(138, SCALAR_TYPE, SemanticLocalRoleV1::Argument(1)),
                local(139, SCALAR_TYPE, SemanticLocalRoleV1::Temporary),
                local(140, POINTER_TYPE, SemanticLocalRoleV1::Temporary),
            ],
        )
    }

    #[test]
    fn mutated_uniform_comparisons_do_not_mint_direct_switch_predicates() {
        let stable = mutated_direct_predicate_function(DirectPredicateMutationV1::Stable);
        assert!(
            project_intrinsic_contracts_for_test(&assertion_proof_types(), &[], &stable)
                .unwrap()
                .direct_switch_predicates[1]
                .is_some()
        );
        assert_incomplete(
            project_intrinsic_contracts_for_test(
                &assertion_proof_types(),
                &[],
                &mutated_direct_predicate_function(DirectPredicateMutationV1::DestinationOverwrite),
            ),
            "a uniform induction comparison with multiple header definitions",
        );
        let reassigned =
            mutated_direct_predicate_function(DirectPredicateMutationV1::ReassignedArgument);
        let projected =
            project_intrinsic_contracts_for_test(&assertion_proof_types(), &[], &reassigned).unwrap();
        assert!(projected.direct_switch_predicates[1].is_none());
        assert_incomplete(
            project_intrinsic_contracts_for_test(
                &assertion_proof_types(),
                &[],
                &mutated_direct_predicate_function(DirectPredicateMutationV1::EscapedAlias),
            ),
            "a uniform induction alias has address-escaped value semantics",
        );
    }

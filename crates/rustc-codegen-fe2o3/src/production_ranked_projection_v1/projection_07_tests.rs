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
            ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Gfx950TransposeTile(
                exact,
            )),
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
        let ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::Operand(rhs)) =
            state[&2]
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
        let result_dominance = SemanticEnumPayloadDominanceV1::analyze(
            &result_function,
            &projection_types_with_enum(),
        )
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
        let function =
            projection_function(vec![block(96, vec![], SemanticTerminatorKindV1::Return)]);
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
        let function =
            projection_function(vec![block(97, vec![], SemanticTerminatorKindV1::Return)]);
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
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], SCALAR_TYPE)
                .unwrap(),
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
            transfer_capability_statements_v1(&function, 0, &mut state, &payload).unwrap();
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
            transfer_capability_statements_v1(&function, 0, &mut state, &payload).unwrap();
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
            Some(64),
            &constants,
            &mut operations,
            &mut next_value,
            &mut ranked_ir,
        )
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

    fn mutated_direct_predicate_function(
        shape: DirectPredicateMutationV1,
    ) -> SemanticFunctionDeclV1 {
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
            project_intrinsic_contracts_for_test(&assertion_proof_types(), &[], &reassigned)
                .unwrap();
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

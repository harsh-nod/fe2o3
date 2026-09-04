    use super::*;
    use dialect_kernel::AccessKindAttr;
    use fe2o3_mir_model::semantic_mir_v1::{InertSemanticMirRequestV1, SemanticMirLimitsV1};
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticAbiArgumentV1, SemanticAbiIdentityV1, SemanticAbiPassModeV1, SemanticAbiValueV1,
        SemanticAggregateLayoutV1, SemanticAggregateTypeV1, SemanticAssignmentV1,
        SemanticBackendReprV1, SemanticBackendScalarV1, SemanticBasicBlockV1,
        SemanticBlockIdentityV1, SemanticCallDestinationV1, SemanticCallableIdV1,
        SemanticCanonAbiV1, SemanticCompilerIntrinsicIdentityV1,
        SemanticConstGenericArgumentsIdentityV1, SemanticConstantV1, SemanticControlFlowEdgeV1,
        SemanticDirectCallV1, SemanticEdgeRoleV1, SemanticExternAbiV1, SemanticFieldsShapeV1,
        SemanticFunctionAbiV1, SemanticFunctionIdentityV1, SemanticFunctionRoleV1,
        SemanticGenericTypeArgumentsIdentityV1, SemanticItemDefinitionIdentityV1,
        SemanticKernelBindingIdentityV1, SemanticKernelEntryV1, SemanticKernelLaunchBoundsV1,
        SemanticKernelSourceContractV1, SemanticLayoutIdentityV1, SemanticLinkSymbolV1,
        SemanticLocalDeclV1, SemanticLocalIdV1, SemanticLocalIdentityV1,
        SemanticMfmaAccumulatorDistributionV1, SemanticMfmaOperandRoleV1,
        SemanticMfmaRegisterDistributionV1, SemanticMonomorphizationIdentityV1,
        SemanticNonBodyCallableBindingV1, SemanticProjectionV1, SemanticRustcVariantsV1,
        SemanticRvalueV1, SemanticScalarValidityRangeV1, SemanticSourceProvenanceV1,
        SemanticStatementV1, SemanticSwitchTargetV1, SemanticSwitchTargetsV1,
        SemanticTargetDataLayoutV1, SemanticTerminatorV1, SemanticTypeIdentityV1,
        SemanticTypeLayoutDetailsV1, SemanticTypeLayoutV1, SemanticUnwindActionV1,
        SemanticWorkgroupDimensionsV1,
    };
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionRankedValueIdV1, ProductionSemanticMirLimitsV1,
        ProductionSessionLimitsV1, compile_ranked_kernel_for_gfx942_lowering_v1,
        compile_ranked_kernel_for_lowering_v1,
    };

    #[test]
    fn workgroup_reduction_accepts_only_the_closed_scalar_and_geometry_contract() {
        assert_eq!(truncate_unsigned_constant_v1(64, &Type::INDEX), Some(64),);
        assert_eq!(
            truncate_unsigned_constant_v1(64, &Type::Scalar(ScalarType::U32)),
            Some(64),
        );
        assert_eq!(
            truncate_unsigned_constant_v1(u64::from(u32::MAX) + 2, &Type::Scalar(ScalarType::U32),),
            Some(1),
        );
        assert_eq!(
            truncate_unsigned_constant_v1(64, &Type::Scalar(ScalarType::I32)),
            None,
        );
        for scalar in [ScalarType::U32, ScalarType::I32, ScalarType::F32] {
            assert!(workgroup_reduction_scalar_supported_v1(&Type::Scalar(
                scalar
            )));
        }
        for scalar in [
            ScalarType::U8,
            ScalarType::U64,
            ScalarType::I64,
            ScalarType::F16,
            ScalarType::F64,
        ] {
            assert!(!workgroup_reduction_scalar_supported_v1(&Type::Scalar(
                scalar
            )));
        }

        for width in [1, 2, 4, 64, 128, 256] {
            assert_eq!(
                validate_workgroup_reduction_geometry_v1(
                    Some([width, 1, 1]),
                    Some(width),
                    Some(width),
                ),
                Ok(width),
            );
            assert_eq!(
                validate_workgroup_reduction_geometry_v1(Some([width, 1, 1]), Some(width), None,),
                Ok(width),
            );
        }
        for geometry in [
            None,
            Some([0, 1, 1]),
            Some([3, 1, 1]),
            Some([257, 1, 1]),
            Some([64, 2, 1]),
            Some([64, 1, 2]),
        ] {
            assert!(
                validate_workgroup_reduction_geometry_v1(geometry, Some(64), Some(64)).is_err()
            );
        }
        for compiler_slots in [None, Some(0), Some(63), Some(65), Some(256)] {
            assert!(
                validate_workgroup_reduction_geometry_v1(
                    Some([64, 1, 1]),
                    compiler_slots,
                    Some(64),
                )
                .is_err()
            );
        }
        for scratch_slots in [Some(0), Some(63), Some(65), Some(256)] {
            assert!(
                validate_workgroup_reduction_geometry_v1(
                    Some([64, 1, 1]),
                    Some(64),
                    scratch_slots,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn subgroup_broadcast_accepts_local_u32_mask_below_wave64_width() {
        let unknown = ValueId(0);
        let mask = ValueId(1);
        let masked = ValueId(2);
        let constant = Operation::effect_free(
            ValueDef::new(mask, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(63)),
        );
        for kind in [
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: unknown,
                rhs: mask,
            },
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: mask,
                rhs: unknown,
            },
        ] {
            let operations = [
                constant.clone(),
                Operation::effect_free(ValueDef::new(masked, Type::Scalar(ScalarType::U32)), kind),
            ];
            assert!(subgroup_broadcast_source_is_statically_bounded(
                &operations,
                masked,
                64,
            ));
        }
        assert!(subgroup_broadcast_source_is_statically_bounded(
            &[constant],
            mask,
            64,
        ));
    }

    #[test]
    fn subgroup_broadcast_rejects_mask_at_width_and_missing_constant_mask() {
        let unknown = ValueId(0);
        let mask = ValueId(1);
        let masked = ValueId(2);
        let mask64 = Operation::effect_free(
            ValueDef::new(mask, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(64)),
        );
        let bitand = |rhs| {
            Operation::effect_free(
                ValueDef::new(masked, Type::Scalar(ScalarType::U32)),
                OperationKind::Binary {
                    op: BinaryOp::BitAnd,
                    lhs: unknown,
                    rhs,
                },
            )
        };
        assert!(!subgroup_broadcast_source_is_statically_bounded(
            &[mask64, bitand(mask)],
            masked,
            64,
        ));
        assert!(!subgroup_broadcast_source_is_statically_bounded(
            &[bitand(ValueId(3))],
            masked,
            64,
        ));
    }

    #[derive(Clone, Copy)]
    struct AuthenticatedInductionFixtureV1 {
        bits: u16,
        bound: u128,
        step: u128,
        extra_write: bool,
        bypass_guard: bool,
    }

    impl Default for AuthenticatedInductionFixtureV1 {
        fn default() -> Self {
            Self {
                bits: 64,
                bound: 64,
                step: 1,
                extra_write: false,
                bypass_guard: false,
            }
        }
    }

    fn authenticated_induction_fixture_v1(
        options: AuthenticatedInductionFixtureV1,
    ) -> (Vec<SemanticTypeDeclV1>, SemanticFunctionDeclV1) {
        let unit = SemanticTypeIdV1::from_index(0);
        let induction_ty = SemanticTypeIdV1::from_index(1);
        let bool_ty = SemanticTypeIdV1::from_index(2);
        let u32_ty = SemanticTypeIdV1::from_index(3);
        let source = SemanticSourceProvenanceV1::unavailable();
        let size = u8::try_from(options.bits / 8).unwrap();
        let place = |local, ty| {
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
        };
        let operand = |local, ty| SemanticOperandV1::Copy(place(local, ty));
        let constant = |ty, value| {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                ty,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, size).unwrap()),
            ))
        };
        let assign = |local, ty, value| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    place(local, ty),
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
        let entry = block(
            210,
            vec![assign(
                1,
                induction_ty,
                SemanticRvalueKindV1::Use(constant(induction_ty, 0)),
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        );
        let header = block(
            211,
            vec![assign(
                2,
                bool_ty,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::LessThan,
                    left: operand(1, induction_ty),
                    right: constant(induction_ty, options.bound),
                },
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand(2, bool_ty),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                )
                .unwrap(),
            },
        );
        let mut body_statements = vec![assign(
            3,
            u32_ty,
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::Integer,
                operand: operand(1, induction_ty),
            },
        )];
        if options.extra_write {
            body_statements.push(assign(
                1,
                induction_ty,
                SemanticRvalueKindV1::Use(constant(induction_ty, 0)),
            ));
        }
        let body = block(
            212,
            body_statements,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 3)),
        );
        let latch = block(
            213,
            vec![assign(
                1,
                induction_ty,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Add,
                    left: operand(1, induction_ty),
                    right: constant(induction_ty, options.step),
                },
            )],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        );
        let exit = block(
            214,
            vec![],
            if options.bypass_guard {
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2))
            } else {
                SemanticTerminatorKindV1::Return
            },
        );
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([215; 32]),
            SemanticLayoutIdentityV1::from_sha256([216; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(unit, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap();
        let locals: Vec<SemanticLocalDeclV1> = [
            (unit, SemanticLocalRoleV1::Return),
            (induction_ty, SemanticLocalRoleV1::Temporary),
            (bool_ty, SemanticLocalRoleV1::Temporary),
            (u32_ty, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(local, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([217 + local as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([221; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([222; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([223; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([224; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([225; 32]),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![entry, header, body, latch, exit],
        )
        .unwrap();
        (
            vec![
                unit_type(),
                unsigned_scalar_type(226, options.bits),
                bool_type(),
                unsigned_scalar_type(228, 32),
            ],
            function,
        )
    }

    #[test]
    fn ranked_canonical_induction_bound_survives_exact_u64_to_u32_cast() {
        let (types, function) =
            authenticated_induction_fixture_v1(AuthenticatedInductionFixtureV1::default());
        let bounds = authenticated_loop_induction_bounds_v1(&types, &function).unwrap();
        assert_eq!(bounds.get(&(2, 1)), Some(&64));
        let alias = SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![],
                SemanticTypeIdV1::from_index(3),
            )
            .unwrap(),
        );
        assert_eq!(
            authenticated_unsigned_operand_exclusive_bound_v1(
                &types,
                &function,
                &bounds,
                SemanticBlockIdV1::from_index(2),
                &alias,
            ),
            Some(64),
        );
    }

    #[test]
    fn authenticated_induction_bound_rejects_width_overrun_and_hostile_loops() {
        let (types, function) =
            authenticated_induction_fixture_v1(AuthenticatedInductionFixtureV1 {
                bound: 65,
                ..AuthenticatedInductionFixtureV1::default()
            });
        let bounds = authenticated_loop_induction_bounds_v1(&types, &function).unwrap();
        assert_eq!(bounds.get(&(2, 1)), Some(&65));
        assert!(!authenticated_subgroup_broadcast_source_is_bounded(
            *bounds.get(&(2, 1)).unwrap(),
            64,
        ));
        assert!(!authenticated_subgroup_broadcast_source_is_bounded(0, 0));

        for options in [
            AuthenticatedInductionFixtureV1 {
                extra_write: true,
                ..AuthenticatedInductionFixtureV1::default()
            },
            AuthenticatedInductionFixtureV1 {
                step: 0,
                ..AuthenticatedInductionFixtureV1::default()
            },
            AuthenticatedInductionFixtureV1 {
                bits: 8,
                bound: 255,
                step: 2,
                ..AuthenticatedInductionFixtureV1::default()
            },
            AuthenticatedInductionFixtureV1 {
                bypass_guard: true,
                ..AuthenticatedInductionFixtureV1::default()
            },
        ] {
            let (types, function) = authenticated_induction_fixture_v1(options);
            assert!(
                authenticated_loop_induction_bounds_v1(&types, &function)
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[test]
    fn inactive_launch_axes_use_canonical_identity_and_extent_constants() {
        for rank in 1..=3 {
            for (axis, axis_rank) in [(Axis::X, 1), (Axis::Y, 2), (Axis::Z, 3)] {
                for kind in [IndexKind::Global, IndexKind::Workgroup, IndexKind::Local] {
                    assert_eq!(
                        inactive_launch_axis_value_v1(rank, kind, axis),
                        (axis_rank > rank).then_some(0),
                    );
                }
                for kind in [IndexKind::WorkgroupSize, IndexKind::WorkgroupCount] {
                    assert_eq!(
                        inactive_launch_axis_value_v1(rank, kind, axis),
                        (axis_rank > rank).then_some(1),
                    );
                }
            }
        }
    }

    #[test]
    fn semantic_casts_use_the_shared_bounded_kernel_ir_index_paths() {
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::Scalar(ScalarType::U32),
                &Type::INDEX,
            ),
            Some([Some((CastKind::ZeroExtend, ScalarType::Index)), None])
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::Scalar(ScalarType::U64),
                &Type::INDEX,
            ),
            Some([Some((CastKind::Bitcast, ScalarType::Index)), None])
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::INDEX,
                &Type::Scalar(ScalarType::U64),
            ),
            Some([Some((CastKind::Bitcast, ScalarType::U64)), None])
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::Scalar(ScalarType::I32),
                &Type::INDEX,
            ),
            Some([
                Some((CastKind::SignExtend, ScalarType::U64)),
                Some((CastKind::Bitcast, ScalarType::Index)),
            ])
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::INDEX,
                &Type::Scalar(ScalarType::U32),
            ),
            Some([
                Some((CastKind::Bitcast, ScalarType::U64)),
                Some((CastKind::Truncate, ScalarType::U32)),
            ])
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::INDEX,
                &Type::Scalar(ScalarType::F64),
            ),
            None
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Float,
                &Type::Scalar(ScalarType::U64),
                &Type::INDEX,
            ),
            None
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Integer,
                &Type::Scalar(ScalarType::U32),
                &Type::Scalar(ScalarType::U64),
            ),
            Some([Some((CastKind::ZeroExtend, ScalarType::U64)), None])
        );
        assert_eq!(
            lower_cast_path(
                SemanticCastKindV1::Transmute,
                &Type::Scalar(ScalarType::U32),
                &Type::Scalar(ScalarType::F32),
            ),
            Some([Some((CastKind::Bitcast, ScalarType::F32)), None])
        );
        for (from, to) in [
            (Type::Scalar(ScalarType::U32), Type::Scalar(ScalarType::U64)),
            (Type::Scalar(ScalarType::U32), Type::Unit),
        ] {
            assert_eq!(
                lower_cast_path(SemanticCastKindV1::Transmute, &from, &to),
                None
            );
        }
    }

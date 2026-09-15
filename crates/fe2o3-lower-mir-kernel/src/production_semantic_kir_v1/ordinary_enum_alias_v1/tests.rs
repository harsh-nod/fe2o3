mod ordinary_enum_alias_tests {
    include!("lifetime_tests.rs");
    include!("mixed_lineage_tests.rs");
    include!("inactive_residual_tests.rs");
    include!("aggregate_custody_tests.rs");
    include!("../ordinary_enum_alias_observation_v1/tests.rs");
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::*;

    fn ty(n: u32) -> SemanticTypeIdV1 {
        SemanticTypeIdV1::from_index(n)
    }
    fn local(n: u32) -> SemanticLocalIdV1 {
        SemanticLocalIdV1::from_index(n)
    }
    fn place(n: u32, t: u32) -> SemanticPlaceV1 {
        SemanticPlaceV1::new(local(n), vec![], ty(t)).unwrap()
    }
    fn source() -> SemanticSourceProvenanceV1 {
        SemanticSourceProvenanceV1::unavailable()
    }
    fn copy(n: u32, t: u32) -> SemanticOperandV1 {
        SemanticOperandV1::Copy(place(n, t))
    }
    fn constant(n: u128) -> SemanticOperandV1 {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(1),
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(n, 8).unwrap()),
        ))
    }
    fn assign(n: u32, t: u32, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(n, t),
                SemanticRvalueV1::new(ty(t), value),
            )),
        )
    }
    fn edge(role: SemanticEdgeRoleV1, b: u32) -> SemanticControlFlowEdgeV1 {
        SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(b))
    }
    fn goto(b: u32) -> SemanticTerminatorKindV1 {
        SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, b))
    }
    fn block(
        b: u8,
        statements: Vec<SemanticStatementV1>,
        term: SemanticTerminatorKindV1,
    ) -> SemanticBasicBlockV1 {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([b + 20; 32]),
            source(),
            statements,
            SemanticTerminatorV1::new(source(), term),
        )
        .unwrap()
    }
    fn field(n: u32, t: u32, field: u32, result: u32) -> SemanticOperandV1 {
        let _ = t;
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                local(n),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(field), ty(result))
                        .unwrap(),
                ],
                ty(result),
            )
            .unwrap(),
        )
    }
    fn some(n: u32, value: SemanticOperandV1) -> SemanticStatementV1 {
        assign(
            n,
            2,
            SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(1), vec![value])
                .unwrap(),
        )
    }
    fn selected() -> SemanticOperandV1 {
        SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                local(3),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), ty(2))
                        .unwrap(),
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(1)).unwrap(),
                ],
                ty(1),
            )
            .unwrap(),
        )
    }

    struct Fixture {
        types: Vec<SemanticTypeDeclV1>,
        function: SemanticFunctionDeclV1,
    }
    impl Fixture {
        fn new(moved: bool) -> Self {
            let types = vec![
                unit_type(),
                unsigned_scalar_type(2, 64),
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([3; 32]),
                    SemanticLayoutIdentityV1::from_sha256([3; 32]),
                    SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
                    SemanticTypeShapeV1::Enum {
                        discriminant: ty(1),
                        variants: vec![
                            SemanticEnumVariantV1::new(
                                0,
                                SemanticAggregateTypeV1::new(vec![]).unwrap(),
                            ),
                            SemanticEnumVariantV1::new(
                                1,
                                SemanticAggregateTypeV1::new(vec![ty(1)]).unwrap(),
                            ),
                        ]
                        .into_boxed_slice(),
                    },
                ),
                bool_type(),
                SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([5; 32]),
                    SemanticLayoutIdentityV1::from_sha256([5; 32]),
                    SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
                    SemanticTypeShapeV1::Tuple(
                        SemanticAggregateTypeV1::new(vec![ty(1), ty(3)]).unwrap(),
                    ),
                ),
            ];
            let mut blocks = vec![
                block(
                    0,
                    vec![
                        assign(
                            6,
                            4,
                            SemanticRvalueKindV1::CheckedBinary(
                                SemanticCheckedBinaryRvalueV1::new(
                                    SemanticCheckedBinaryOpV1::Add,
                                    constant(41),
                                    constant(1),
                                ),
                            ),
                        ),
                        assign(
                            7,
                            4,
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(6, 4))),
                        ),
                        assign(8, 3, SemanticRvalueKindV1::Use(field(7, 4, 1, 3))),
                    ],
                    SemanticTerminatorKindV1::Assert {
                        condition: copy(8, 3),
                        expected: false,
                        message: SemanticAssertMessageV1::Overflow {
                            operation: SemanticBinaryOpV1::Add,
                            left: constant(41),
                            right: constant(1),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 8),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(
                    1,
                    vec![
                        assign(9, 1, SemanticRvalueKindV1::Use(field(7, 4, 0, 1))),
                        some(2, copy(9, 1)),
                    ],
                    goto(3),
                ),
                block(
                    2,
                    vec![assign(
                        2,
                        2,
                        SemanticRvalueKindV1::aggregate(
                            SemanticAggregateKindV1::EnumVariant(0),
                            vec![],
                        )
                        .unwrap(),
                    )],
                    goto(3),
                ),
                block(
                    3,
                    vec![assign(
                        3,
                        2,
                        SemanticRvalueKindV1::Use(if moved {
                            SemanticOperandV1::Move(place(2, 2))
                        } else {
                            copy(2, 2)
                        }),
                    )],
                    goto(4),
                ),
                block(
                    4,
                    vec![assign(
                        4,
                        1,
                        SemanticRvalueKindV1::Discriminant(place(3, 2)),
                    )],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: copy(4, 1),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![
                                SemanticSwitchTargetV1::new(
                                    0,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 5),
                                ),
                                SemanticSwitchTargetV1::new(
                                    1,
                                    edge(SemanticEdgeRoleV1::SwitchValue, 6),
                                ),
                            ],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 7),
                        )
                        .unwrap(),
                    },
                ),
                block(5, vec![], SemanticTerminatorKindV1::Return),
                block(
                    6,
                    vec![assign(5, 1, SemanticRvalueKindV1::Use(selected()))],
                    SemanticTerminatorKindV1::Return,
                ),
                block(7, vec![], SemanticTerminatorKindV1::Unreachable),
                block(
                    8,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: copy(1, 3),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 2),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                        )
                        .unwrap(),
                    },
                ),
            ];
            let abi = SemanticFunctionAbiV1::from_rustc(
                SemanticAbiIdentityV1::from_sha256([10; 32]),
                SemanticLayoutIdentityV1::from_sha256([11; 32]),
                SemanticCanonAbiV1::Rust,
                SemanticExternAbiV1::Rust,
                false,
                false,
                1,
                vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                    ty(3),
                    SemanticAbiPassModeV1::Direct(SemanticAbiValueAttributesV1::plain()),
                ))],
                SemanticAbiValueV1::new(ty(0), SemanticAbiPassModeV1::Ignore),
            )
            .unwrap();
            let locals = [0, 3, 2, 2, 1, 1, 4, 4, 3, 1]
                .into_iter()
                .enumerate()
                .map(|(i, t)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([60 + i as u8; 32]),
                        ty(t),
                        match i {
                            0 => SemanticLocalRoleV1::Return,
                            1 => SemanticLocalRoleV1::Argument(0),
                            _ => SemanticLocalRoleV1::Temporary,
                        },
                        source(),
                    )
                })
                .collect();
            let function = SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256([80; 32]),
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([81; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([82; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([83; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([84; 32]),
                source(),
                abi,
                locals,
                SemanticBlockIdV1::from_index(0),
                std::mem::take(&mut blocks),
            )
            .unwrap();
            Self { types, function }
        }
        fn replace_block(
            &mut self,
            b: usize,
            statements: Vec<SemanticStatementV1>,
            term: SemanticTerminatorKindV1,
        ) {
            let f = &self.function;
            let mut blocks = f.blocks().to_vec();
            blocks[b] = block(b as u8, statements, term);
            self.function = SemanticFunctionDeclV1::new(
                f.identity(),
                f.role(),
                f.item_definition_identity(),
                f.monomorphization_identity(),
                f.generic_type_arguments_identity(),
                f.const_generic_arguments_identity(),
                f.source(),
                f.abi().clone(),
                f.locals().to_vec(),
                f.entry(),
                blocks,
            )
            .unwrap();
        }
        fn lowering(&self) -> SemanticFunctionLoweringV1<'_> {
            SemanticFunctionLoweringV1::new(
                &self.types,
                &[],
                &self.function,
                SemanticParameterBindingsV1 {
                    declarations: &[(0, 1, ty(3))],
                    values: &[ValueId(0)],
                    types: &[Type::Scalar(ScalarType::Bool)],
                    local_bindings: None,
                },
                Some(BlockId(9)),
                None,
                BTreeSet::new(),
                1,
                false,
                1_000_000,
            )
            .unwrap()
        }
        fn emit(
            &self,
        ) -> Result<(Vec<BasicBlock>, SemanticFunctionLoweringV1<'_>), ProductionSemanticKirErrorV1>
        {
            let owner = plan_semantic_function_ssa_with_module_v1(
                SemanticFunctionIdV1::from_index(0),
                &self.function,
                &self.types,
                &[],
                ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap();
            let mut lowering = self.lowering();
            let mut result = vec![];
            for block_id in owner.plan().reverse_postorder() {
                let b = block_id.get();
                let source = &self.function.blocks()[b as usize];
                let mut target = BasicBlock::new(BlockId(b));
                lowering.begin_block(SemanticBlockIdV1::from_index(b), &mut target)?;
                for (i, statement) in source.statements().iter().enumerate() {
                    lowering.lower_statement(
                        SemanticBlockIdV1::from_index(b),
                        Some(i as u32),
                        statement.kind(),
                        &mut target.operations,
                    )?;
                }
                target.terminator = Some(lowering.lower_terminator(
                    SemanticBlockIdV1::from_index(b),
                    source.terminator().kind(),
                    &mut target.operations,
                )?);
                result.push(target);
            }
            require_semantic_ssa_definitions_consumed_v1(
                0,
                &lowering.pending_semantic_ssa_definitions,
            )?;
            Ok((result, lowering))
        }
    }

    #[test]
    fn ordinary_enum_alias_move_and_copy_restore_only_original_some_slot() {
        for moved in [false, true] {
            let fixture = Fixture::new(moved);
            let before = fixture.function.clone();
            let (blocks, lowering) = fixture.emit().unwrap();
            assert!(!lowering.control_flow_ssa.promoted.contains_key(&3));
            assert!(lowering.control_flow_ssa.promoted.contains_key(&2));
            assert_eq!(
                lowering.enum_payload_aliases.as_ref().map(|proofs| proofs.iter().map(|(local, proof)| (*local, proof.source())).collect::<BTreeMap<_, _>>()),
                Some(BTreeMap::from([(3, 2)]))
            );
            let slot = lowering.enum_payload_storage[&(2, 1, 0)].components[0].pointer;
            let reads = blocks
                .iter()
                .flat_map(|b| b.operations.iter().map(move |op| (b.id.0, op)))
                .filter_map(|(b, op)| {
                    matches!(op.kind,OperationKind::Load {pointer,..} if pointer==slot).then_some(b)
                })
                .collect::<Vec<_>>();
            assert_eq!(reads, vec![6], "no None/unselected payload load");
            assert!(
                blocks
                    .iter()
                    .find(|b| b.id.0 == 1)
                    .unwrap()
                    .operations
                    .iter()
                    .any(|op| matches!(op.kind,OperationKind::Store {pointer,..} if pointer==slot))
            );
            assert_eq!(
                fixture.function, before,
                "original checked arithmetic, moves, assert and CFG remain exact"
            );
        }
    }

    fn require_missing_payload(fixture: &Fixture) {
        let error = match fixture.emit() {
            Ok(_) => panic!("unproved alias must not lower"),
            Err(e) => e,
        };
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::EnumPayloadUnavailable {
                    block: 6,
                    local: 3,
                    variant: 1,
                    field: 0,
                    available_fields: 0,
                    ..
                }
            ),
            "{error:?}"
        );
    }

    #[test]
    fn ordinary_enum_alias_missing_or_inverted_variant_guard_rejects() {
        for inverted in [false, true] {
            let mut f = Fixture::new(true);
            let term = if inverted {
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: copy(4, 1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 6),
                            ),
                            SemanticSwitchTargetV1::new(
                                1,
                                edge(SemanticEdgeRoleV1::SwitchValue, 5),
                            ),
                        ],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 7),
                    )
                    .unwrap(),
                }
            } else {
                goto(6)
            };
            f.replace_block(4, f.function.blocks()[4].statements().to_vec(), term);
            require_missing_payload(&f);
        }
    }

    #[test]
    fn ordinary_enum_alias_source_overwrite_after_capture_rejects() {
        let mut f = Fixture::new(true);
        let mut statements = f.function.blocks()[3].statements().to_vec();
        statements.push(some(2, constant(99)));
        f.replace_block(3, statements, goto(4));
        require_missing_payload(&f);
    }

    #[test]
    fn ordinary_enum_alias_bypass_into_selected_branch_rejects() {
        let mut f = Fixture::new(true);
        f.replace_block(5, vec![], goto(6));
        require_missing_payload(&f);
    }

    #[test]
    fn ordinary_enum_alias_discriminant_only_use_spends_no_new_precision_work() {
        let mut f = Fixture::new(true);
        f.replace_block(6, vec![], SemanticTerminatorKindV1::Return);
        let initial = f.lowering().enum_analysis_budget;
        let (_, lowering) = f.emit().unwrap();
        assert!(lowering.enum_payload_aliases.is_none());
        assert_eq!(lowering.enum_analysis_budget.work, initial.work);
        assert_eq!(lowering.enum_analysis_budget.storage, initial.storage);
    }

    #[test]
    fn ordinary_enum_alias_types_and_compiler_issued_payload_are_not_interchangeable() {
        let base = Fixture::new(true);
        let plan = base.lowering().control_flow_ssa;
        for mutation in 0..4 {
            let mut f = Fixture::new(true);
            let mut transport = plan.clone();
            match mutation {
                0 => f.replace_block(
                    3,
                    vec![assign(3, 2, SemanticRvalueKindV1::Use(copy(2, 1)))],
                    goto(4),
                ),
                1 => f.replace_block(
                    1,
                    vec![some(
                        2,
                        SemanticOperandV1::Constant(SemanticConstantV1::new(
                            ty(3),
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(1, 1).unwrap(),
                            ),
                        )),
                    )],
                    goto(3),
                ),
                2 => {
                    transport
                        .compiler_issued_bindings
                        .insert(ty(1), SemanticPromotedBindingV1::KernelContext);
                }
                _ => {
                    transport.ssa_value_locals.remove(&3);
                }
            }
            let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
            assert!(
                ordinary_enum_alias_v1::plan(&f.types, &f.function, &transport, &mut budget)
                    .unwrap()
                    .is_empty(),
                "mutation {mutation}"
            );
        }
    }

    // Mutation tests deliberately retain the authentic pre-mutation transport
    // roster to test the independent complete-source audit, not SSA admission.
    #[test]
    fn ordinary_enum_alias_lifetime_writes_escape_and_backedge_revoke_storage_alias() {
        let base = Fixture::new(true);
        let plan = base.lowering().control_flow_ssa;
        for mutation in 0..7 {
            let mut f = Fixture::new(true);
            let mut statements = f.function.blocks()[3].statements().to_vec();
            let mut term = goto(4);
            match mutation {
                0 => statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Deinitialize(place(2, 2)),
                )),
                1 => statements.insert(0, SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(local(2)),
                )),
                2 => statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::SetDiscriminant {
                        place: place(2, 2),
                        variant_index: 0,
                    },
                )),
                3 => statements.push(some(3, constant(99))),
                4 => term = goto(1),
                5 => statements.push(assign(
                    5,
                    1,
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Mutable,
                        place: place(2, 2),
                    },
                )),
                _ => {
                    term = SemanticTerminatorKindV1::TailCall(
                        SemanticDirectTailCallV1::new_callable(
                            SemanticCallableIdV1::from_index(0),
                            vec![copy(2, 2)],
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    )
                }
            }
            f.replace_block(3, statements, term);
            let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
            assert!(
                ordinary_enum_alias_v1::plan(&f.types, &f.function, &plan, &mut budget)
                    .unwrap()
                    .is_empty(),
                "mutation {mutation}"
            );
        }
    }

    #[test]
    fn ordinary_enum_alias_cleanup_edge_to_source_write_is_not_ignored() {
        let base = Fixture::new(true);
        let plan = base.lowering().control_flow_ssa;
        let mut f = Fixture::new(true);
        f.replace_block(
            3,
            f.function.blocks()[3].statements().to_vec(),
            SemanticTerminatorKindV1::Assert {
                condition: copy(1, 3),
                expected: true,
                message: SemanticAssertMessageV1::ResumedAfterPanic,
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                unwind: SemanticUnwindActionV1::Cleanup(edge(SemanticEdgeRoleV1::AssertUnwind, 1)),
            },
        );
        let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        assert!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &plan, &mut budget)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn ordinary_enum_alias_exact_shared_work_and_storage_limits() {
        let f = Fixture::new(true);
        let plan = f.lowering().control_flow_ssa;
        let mut measured = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        assert_eq!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &plan, &mut measured).unwrap(),
            BTreeMap::from([(3, 2)])
        );
        let mut exact = SemanticEnumAnalysisBudgetV1::new(measured.work, measured.storage);
        assert_eq!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &plan, &mut exact).unwrap(),
            BTreeMap::from([(3, 2)])
        );
        for (work, storage, resource) in [
            (
                measured.work - 1,
                measured.storage,
                ProductionSemanticKirResourceV1::AnalysisWork,
            ),
            (
                measured.work,
                measured.storage - 1,
                ProductionSemanticKirResourceV1::AnalysisStorage,
            ),
        ] {
            let mut budget = SemanticEnumAnalysisBudgetV1::new(work, storage);
            let error = ordinary_enum_alias_v1::plan(&f.types, &f.function, &plan, &mut budget)
                .unwrap_err();
            assert!(
                matches!(error,ProductionSemanticKirErrorV1::ResourceLimit {resource:actual,..} if actual==resource)
            );
        }
        let mut shared = SemanticEnumAnalysisBudgetV1::new(measured.work, measured.storage);
        shared.charge_work(1).unwrap();
        assert!(
            matches!(ordinary_enum_alias_v1::plan(&f.types,&f.function,&plan,&mut shared),Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource:ProductionSemanticKirResourceV1::AnalysisWork,actual,limit}) if actual==measured.work+1&&limit==measured.work)
        );
    }
}

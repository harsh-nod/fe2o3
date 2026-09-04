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

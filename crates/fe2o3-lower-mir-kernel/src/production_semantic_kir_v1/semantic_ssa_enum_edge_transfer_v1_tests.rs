fn enum_edge_definition_v1(index: u32) -> SsaValueV1 {
    SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(index))
}

fn enum_edge_phi_v1(block: u32, local: u32) -> SsaValueV1 {
    SsaValueV1::BlockArgument {
        block: SsaBlockIdV1::new(block),
        variable: fe2o3_mir_model::SsaVariableIdV1::new(local),
    }
}

fn enum_edge_argument_v1(local: u32, value: SsaValueV1) -> SsaArgumentV1 {
    SsaArgumentV1::new(fe2o3_mir_model::SsaVariableIdV1::new(local), value)
}

fn enum_edge_transfer_v1(
    facts: &SemanticEnumSsaFactsV1,
    arguments: &[SsaArgumentV1],
) -> SemanticEnumSsaFactsV1 {
    let mut budget = SemanticEnumAnalysisBudgetV1::new(usize::MAX, usize::MAX);
    let next = facts.renamed_for_edge(arguments, 1, &mut budget).unwrap();
    assert_eq!(budget.storage, next.retained_entries());
    next
}

#[test]
fn enum_edge_overwrite_uses_the_original_incoming_definition() {
    let source = enum_edge_definition_v1(7);
    let target = enum_edge_phi_v1(1, 1);
    let unrelated = enum_edge_definition_v1(8);
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(source, 1), (target, 0), (unrelated, 2)]),
        discriminants: BTreeMap::new(),
    };
    let next = enum_edge_transfer_v1(&facts, &[enum_edge_argument_v1(1, source)]);
    assert_eq!(next.variants, BTreeMap::from([(target, 1), (unrelated, 2)]));
    assert_eq!(facts.variants.get(&target), Some(&0));
}

#[test]
fn enum_edge_overwrite_drops_stale_discriminator_keys_and_references() {
    let enum_target = enum_edge_phi_v1(1, 1);
    let discriminator_target = enum_edge_phi_v1(1, 2);
    let old_discriminator = enum_edge_definition_v1(10);
    let untouched_discriminator = enum_edge_definition_v1(11);
    let untouched_enum = enum_edge_definition_v1(12);
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(enum_target, 0)]),
        discriminants: BTreeMap::from([
            (discriminator_target, untouched_enum),
            (old_discriminator, enum_target),
            (untouched_discriminator, untouched_enum),
        ]),
    };
    let next = enum_edge_transfer_v1(
        &facts,
        &[
            enum_edge_argument_v1(1, enum_edge_definition_v1(20)),
            enum_edge_argument_v1(2, enum_edge_definition_v1(21)),
            enum_edge_argument_v1(3, old_discriminator),
        ],
    );
    assert!(next.variants.is_empty());
    assert_eq!(
        next.discriminants,
        BTreeMap::from([(untouched_discriminator, untouched_enum)])
    );
    let carried = enum_edge_transfer_v1(
        &facts,
        &[enum_edge_argument_v1(1, enum_edge_definition_v1(20))],
    );
    assert!(!carried.discriminants.contains_key(&old_discriminator));
}

#[test]
fn enum_edge_identity_and_permutation_are_simultaneous() {
    let a = enum_edge_phi_v1(1, 1);
    let b = enum_edge_phi_v1(1, 2);
    let da = enum_edge_phi_v1(1, 3);
    let db = enum_edge_phi_v1(1, 4);
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(a, 0), (b, 1)]),
        discriminants: BTreeMap::from([(da, a), (db, b)]),
    };
    let identity = [
        enum_edge_argument_v1(1, a),
        enum_edge_argument_v1(2, b),
        enum_edge_argument_v1(3, da),
        enum_edge_argument_v1(4, db),
    ];
    assert_eq!(enum_edge_transfer_v1(&facts, &identity), facts);
    let permutation = [
        enum_edge_argument_v1(1, b),
        enum_edge_argument_v1(2, a),
        enum_edge_argument_v1(3, db),
        enum_edge_argument_v1(4, da),
    ];
    let next = enum_edge_transfer_v1(&facts, &permutation);
    assert_eq!(next.variants, BTreeMap::from([(a, 1), (b, 0)]));
    assert_eq!(next.discriminants, BTreeMap::from([(da, a), (db, b)]));
    let mut reversed = permutation;
    reversed.reverse();
    assert_eq!(enum_edge_transfer_v1(&facts, &reversed), next);
}

#[test]
fn enum_edge_source_fanout_keeps_variants_but_not_ambiguous_discriminator_relations() {
    let source = enum_edge_definition_v1(1);
    let discriminator = enum_edge_definition_v1(2);
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(source, 1)]),
        discriminants: BTreeMap::from([(discriminator, source)]),
    };
    let arguments = [
        enum_edge_argument_v1(1, source),
        enum_edge_argument_v1(2, source),
        enum_edge_argument_v1(3, discriminator),
    ];
    let next = enum_edge_transfer_v1(&facts, &arguments);
    assert_eq!(
        next.variants,
        BTreeMap::from([(enum_edge_phi_v1(1, 1), 1), (enum_edge_phi_v1(1, 2), 1)])
    );
    assert!(next.discriminants.is_empty());
    let carried = enum_edge_transfer_v1(&facts, &arguments[..2]);
    assert!(carried.discriminants.is_empty());
}

#[test]
fn enum_edge_duplicate_destinations_refuse_and_restore_temporary_entries() {
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(enum_edge_definition_v1(1), 0)]),
        discriminants: BTreeMap::new(),
    };
    let arguments = [
        enum_edge_argument_v1(1, enum_edge_definition_v1(1)),
        enum_edge_argument_v1(1, enum_edge_definition_v1(2)),
    ];
    let mut budget = SemanticEnumAnalysisBudgetV1::new(usize::MAX, 7 + 1 + 4 * 2);
    budget.charge_storage(7).unwrap();
    assert!(matches!(
        facts.renamed_for_edge(&arguments, 1, &mut budget),
        Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    ));
    assert_eq!(budget.storage, 7);
    assert_eq!(budget.work, 1 + 2 * 2 + 1);
}

#[test]
fn enum_edge_transfer_has_source_derived_exact_and_one_short_component_limits() {
    const PREFIX_WORK: usize = 5;
    const PREFIX_STORAGE: usize = 7;
    // Two argument passes, one pass over two variants and one discriminator, entry unit.
    const WORK: usize = 1 + 2 * 2 + 2 + 1;
    // Two indexes of at most A rows, carried V+D rows, and at most 2A copied rows.
    const TEMPORARY: usize = 2 * 2 + (2 + 1) + 2 * 2;
    let source = enum_edge_definition_v1(1);
    let discriminator = enum_edge_definition_v1(2);
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(source, 1), (enum_edge_phi_v1(1, 1), 0)]),
        discriminants: BTreeMap::from([(discriminator, source)]),
    };
    let arguments = [
        enum_edge_argument_v1(1, source),
        enum_edge_argument_v1(2, discriminator),
    ];
    let budget = |work, storage| {
        let mut budget = SemanticEnumAnalysisBudgetV1::new(work, storage);
        budget.charge_work(PREFIX_WORK).unwrap();
        budget.charge_storage(PREFIX_STORAGE).unwrap();
        budget
    };
    let mut exact = budget(PREFIX_WORK + WORK, PREFIX_STORAGE + TEMPORARY);
    let next = facts.renamed_for_edge(&arguments, 1, &mut exact).unwrap();
    assert_eq!(next.retained_entries(), 2);
    assert_eq!(exact.work, PREFIX_WORK + WORK);
    assert_eq!(exact.storage, PREFIX_STORAGE + 2);
    drop(next);
    exact.replace_storage(2, 0).unwrap();
    assert_eq!(exact.storage, PREFIX_STORAGE);

    let mut work_short = budget(PREFIX_WORK + WORK - 1, usize::MAX);
    assert!(matches!(
        facts.renamed_for_edge(&arguments, 1, &mut work_short),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisWork,
            actual,
            limit,
        }) if actual == PREFIX_WORK + WORK && limit + 1 == actual
    ));
    assert_eq!(work_short.storage, PREFIX_STORAGE);
    assert_eq!(work_short.work, PREFIX_WORK + WORK);
    let mut storage_short = budget(usize::MAX, PREFIX_STORAGE + TEMPORARY - 1);
    assert!(matches!(
        facts.renamed_for_edge(&arguments, 1, &mut storage_short),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            actual,
            limit,
        }) if actual == PREFIX_STORAGE + TEMPORARY && limit + 1 == actual
    ));
    assert_eq!(storage_short.storage, PREFIX_STORAGE);
    assert_eq!(storage_short.work, PREFIX_WORK + WORK);

    let mut cumulative = budget(PREFIX_WORK + 2 * WORK, PREFIX_STORAGE + 2 + TEMPORARY);
    let first = facts
        .renamed_for_edge(&arguments, 1, &mut cumulative)
        .unwrap();
    let second = facts
        .renamed_for_edge(&arguments, 1, &mut cumulative)
        .unwrap();
    assert_eq!(cumulative.work, PREFIX_WORK + 2 * WORK);
    assert_eq!(cumulative.storage, PREFIX_STORAGE + 4);
    drop((first, second));
    cumulative.replace_storage(4, 0).unwrap();
    assert_eq!(cumulative.storage, PREFIX_STORAGE);
}

fn enum_edge_loop_source_v1(
    backedge_variant: u32,
) -> (
    Vec<SemanticTypeDeclV1>,
    SemanticFunctionDeclV1,
    SemanticControlFlowSsaPlanV1,
) {
    let (types, template, _) = exact_enum_ssa_fixture_v1(false);
    let u32_ty = SemanticTypeIdV1::from_index(1);
    let enum_ty = SemanticTypeIdV1::from_index(2);
    let source = SemanticSourceProvenanceV1::unavailable();
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |destination, ty, value| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                destination,
                SemanticRvalueV1::new(ty, value),
            )),
        )
    };
    let constructor = |variant, payload| {
        assign(
            place(1, enum_ty),
            enum_ty,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(variant),
                vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                    u32_ty,
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(payload, 4).unwrap(),
                    ),
                ))],
            )
            .unwrap(),
        )
    };
    let edge =
        |role, block| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block));
    let block = |tag, statements, terminator| {
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([tag; 32]),
            source,
            statements,
            SemanticTerminatorV1::new(source, terminator),
        )
        .unwrap()
    };
    let payload = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), enum_ty).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), u32_ty).unwrap(),
        ],
        u32_ty,
    )
    .unwrap();
    let blocks = vec![
        block(
            101,
            vec![constructor(0, 10)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(
            102,
            vec![
                assign(
                    place(4, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(payload)),
                ),
                assign(
                    place(2, u32_ty),
                    u32_ty,
                    SemanticRvalueKindV1::Discriminant(place(1, enum_ty)),
                ),
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(2, u32_ty)),
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
        block(
            103,
            vec![constructor(backedge_variant, 20)],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
        ),
        block(104, vec![], SemanticTerminatorKindV1::Return),
    ];
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([105; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([106; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([107; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([108; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([109; 32]),
        source,
        template.abi().clone(),
        template.locals().to_vec(),
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
        SemanticSsaTransportInputV1 {
            types: &types,
            callables: &[],
            function: &function,
            semantic_function,
        },
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
fn enum_edge_actual_loop_source_does_not_forge_the_header_variant() {
    let (types, function, plan) = enum_edge_loop_source_v1(1);
    let header = plan.entry_value(&function, 1, 1).unwrap();
    assert_eq!(header, enum_edge_phi_v1(1, 1));
    let backedge_definition = plan.definition_values[&(2, 1)][0];
    assert!(matches!(backedge_definition, SsaValueV1::Definition(_)));
    assert!(plan.edge_arguments[&(2, 0)].iter().any(|argument| {
        argument.variable().get() == 1 && argument.value() == backedge_definition
    }));
    let facts = analyze_promoted_enum_variants_v1(&types, &function, &plan, usize::MAX, usize::MAX)
        .unwrap();
    assert_eq!(facts.get(&(1, header)), None);
    // Fresh static definitions cannot be part of their own block's first incoming facts;
    // subsequent meet only removes facts. No additional Assign/call kill is needed here.
    for ((block, _), definitions) in &plan.definition_values {
        for definition in definitions {
            assert!(!facts.contains_key(&(*block, *definition)));
        }
    }
}

#[test]
fn enum_edge_actual_loop_preserves_a_genuinely_invariant_variant() {
    let (types, function, plan) = enum_edge_loop_source_v1(0);
    let header = plan.entry_value(&function, 1, 1).unwrap();
    let facts = analyze_promoted_enum_variants_v1(&types, &function, &plan, usize::MAX, usize::MAX)
        .unwrap();
    assert_eq!(facts.get(&(1, header)), Some(&0));
}

#[test]
fn enum_edge_merge_transfers_entries_and_cleans_up_late_budget_denials() {
    const PREFIX_WORK: usize = 3;
    const PREFIX_STORAGE: usize = 7;
    const PREVIOUS: usize = 10;
    const EDGE_WORK: usize = 1 + 2 + 2;
    const MERGE_WORK: usize = 1 + 1 + 2 * PREVIOUS;
    const WORK: usize = PREFIX_WORK + EDGE_WORK + MERGE_WORK;
    // Edge construction needs 2 + 4A = 6 temporary entries. The later merge peak
    // is larger: retained old rows + one edge fact + PREVIOUS result capacity.
    const STORAGE: usize = PREFIX_STORAGE + PREVIOUS + 1 + PREVIOUS;
    let (_, _, plan) = enum_edge_loop_source_v1(1);
    let definition = plan.definition_values[&(2, 1)][0];
    let header = enum_edge_phi_v1(1, 1);
    assert_eq!(plan.edge_arguments[&(2, 0)].len(), 1);
    for (work_limit, storage_limit, old_variant, expected_retained) in [
        (WORK, STORAGE, 0, Some(0)),
        (WORK, STORAGE, 1, Some(1)),
        (WORK - 1, STORAGE, 0, None),
        (WORK, STORAGE - 1, 0, None),
    ] {
        let facts = SemanticEnumSsaFactsV1 {
            variants: BTreeMap::from([(definition, 1), (header, 0)]),
            discriminants: BTreeMap::new(),
        };
        let mut previous = SemanticEnumSsaFactsV1 {
            variants: BTreeMap::from([(header, old_variant)]),
            discriminants: BTreeMap::new(),
        };
        for index in 0..9 {
            previous
                .variants
                .insert(enum_edge_definition_v1(100 + index), 0);
        }
        assert_eq!(previous.retained_entries(), PREVIOUS);
        let mut incoming = vec![None, Some(previous.clone()), None, None];
        let mut queued = BTreeSet::new();
        let mut worklist = VecDeque::new();
        let mut budget = SemanticEnumAnalysisBudgetV1::new(work_limit, storage_limit);
        budget.charge_work(PREFIX_WORK).unwrap();
        budget.charge_storage(PREFIX_STORAGE + PREVIOUS).unwrap();
        let result = propagate_promoted_enum_facts_v1(
            2,
            0,
            1,
            facts,
            &plan,
            &mut incoming,
            &mut queued,
            &mut worklist,
            &mut budget,
        );
        assert_eq!(budget.work, WORK);
        if let Some(retained) = expected_retained {
            result.unwrap();
            assert_eq!(incoming[1].as_ref().unwrap().retained_entries(), retained);
            assert_eq!(budget.storage, PREFIX_STORAGE + retained);
            assert_eq!(queued, BTreeSet::from([1]));
            assert_eq!(worklist, VecDeque::from([1]));
            drop(incoming);
            budget.replace_storage(retained, 0).unwrap();
            assert_eq!(budget.storage, PREFIX_STORAGE);
        } else {
            let expected_resource = if work_limit < WORK {
                ProductionSemanticKirResourceV1::AnalysisWork
            } else {
                ProductionSemanticKirResourceV1::AnalysisStorage
            };
            let expected_actual = if work_limit < WORK { WORK } else { STORAGE };
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit { resource, actual, limit })
                    if resource == expected_resource && actual == expected_actual && limit + 1 == actual
            ));
            assert_eq!(incoming[1].as_ref(), Some(&previous));
            assert!(queued.is_empty());
            assert!(worklist.is_empty());
            assert_eq!(budget.storage, PREFIX_STORAGE + PREVIOUS);
        }
    }
}

#[test]
fn enum_edge_unchanged_merge_releases_both_temporary_maps_without_queueing() {
    let (_, _, plan) = enum_edge_loop_source_v1(1);
    let definition = plan.definition_values[&(2, 1)][0];
    let header = enum_edge_phi_v1(1, 1);
    assert_eq!(plan.edge_arguments[&(2, 0)].len(), 1);
    let facts = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(definition, 1), (header, 0)]),
        discriminants: BTreeMap::new(),
    };
    let previous = SemanticEnumSsaFactsV1 {
        variants: BTreeMap::from([(header, 1)]),
        discriminants: BTreeMap::new(),
    };
    let mut incoming = vec![None, Some(previous.clone()), None, None];
    let mut queued = BTreeSet::new();
    let mut worklist = VecDeque::new();
    // A=1, F=2, E=P=1: edge work 5, merge work 4, maximum temporary rows 6.
    let mut budget = SemanticEnumAnalysisBudgetV1::new(5 + 4, 7 + 1 + 6);
    budget.charge_storage(7 + 1).unwrap();
    propagate_promoted_enum_facts_v1(
        2,
        0,
        1,
        facts,
        &plan,
        &mut incoming,
        &mut queued,
        &mut worklist,
        &mut budget,
    )
    .unwrap();
    assert_eq!(incoming[1].as_ref(), Some(&previous));
    assert_eq!(budget.work, 5 + 4);
    assert_eq!(budget.storage, 7 + 1);
    assert!(queued.is_empty());
    assert!(worklist.is_empty());
}

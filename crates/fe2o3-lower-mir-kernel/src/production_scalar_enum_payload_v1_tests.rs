use super::*;

const SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const ENUM: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const FUNCTION: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy)]
enum Case {
    Ordinary,
    SameProducer,
    Conflict,
    RedefinedParameter,
    Backedge,
    Single,
    Reordered,
}

struct Fixture {
    types: Vec<SemanticTypeDeclV1>,
    function: SemanticFunctionDeclV1,
}

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn operand(local: u32) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, SCALAR))
}

fn constant(value: u64) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        SCALAR,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 8).unwrap()),
    ))
}

fn assign(local: u32, ty: SemanticTypeIdV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(local, ty),
            SemanticRvalueV1::new(ty, value),
        )),
    )
}

fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}

fn goto(target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target))
}

fn block(
    index: u8,
    statements: Vec<SemanticStatementV1>,
    end: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([index + 40; 32]),
        source,
        statements,
        SemanticTerminatorV1::new(source, end),
    )
    .unwrap()
}

impl Fixture {
    fn new(case: Case) -> Self {
        let source = SemanticSourceProvenanceV1::unavailable();
        let types = vec![
            unit_type(),
            u64_type(),
            SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([90; 32]),
                SemanticLayoutIdentityV1::from_sha256([91; 32]),
                SemanticTypeLayoutV1::new(Some(16), 8).unwrap(),
                SemanticTypeShapeV1::Enum {
                    discriminant: SCALAR,
                    variants: vec![
                        SemanticEnumVariantV1::new(
                            3,
                            SemanticAggregateTypeV1::new(vec![SCALAR]).unwrap(),
                        ),
                        SemanticEnumVariantV1::new(
                            11,
                            SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        ),
                    ]
                    .into_boxed_slice(),
                },
            ),
        ];
        let scalar_abi = || {
            SemanticAbiValueV1::new(
                SCALAR,
                SemanticAbiPassModeV1::Direct(
                    fe2o3_mir_model::semantic_mir_v1::SemanticAbiValueAttributesV1::plain(),
                ),
            )
        };
        let abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([92; 32]),
            SemanticLayoutIdentityV1::from_sha256([93; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            3,
            (0..3)
                .map(|_| SemanticAbiArgumentV1::source(scalar_abi()))
                .collect(),
            scalar_abi(),
        )
        .unwrap();
        let construct = |variant, fields| {
            assign(
                4,
                ENUM,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::EnumVariant(variant),
                    fields,
                )
                .unwrap(),
            )
        };
        let mut first = Vec::new();
        if matches!(case, Case::RedefinedParameter) {
            first.push(assign(2, SCALAR, SemanticRvalueKindV1::Use(constant(99))));
        }
        first.push(construct(0, vec![operand(2)]));
        let second = match case {
            Case::Single => SemanticStatementV1::new(source, SemanticStatementKindV1::Nop),
            Case::SameProducer => construct(0, vec![operand(2)]),
            Case::Conflict => construct(0, vec![operand(3)]),
            _ => construct(1, vec![]),
        };
        let field = SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![
                SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), ENUM).unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR).unwrap(),
            ],
            SCALAR,
        )
        .unwrap();
        let mut blocks = vec![
            block(
                0,
                vec![],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: operand(1),
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
            block(1, first, goto(3)),
            block(2, vec![second], goto(3)),
            block(
                3,
                vec![assign(
                    5,
                    SCALAR,
                    SemanticRvalueKindV1::Discriminant(place(4, ENUM)),
                )],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: operand(5),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![
                            SemanticSwitchTargetV1::new(
                                3,
                                edge(SemanticEdgeRoleV1::SwitchValue, 4),
                            ),
                            SemanticSwitchTargetV1::new(
                                11,
                                edge(SemanticEdgeRoleV1::SwitchValue, 5),
                            ),
                        ],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 6),
                    )
                    .unwrap(),
                },
            ),
            block(
                4,
                vec![assign(
                    0,
                    SCALAR,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(field)),
                )],
                if matches!(case, Case::Backedge) {
                    goto(3)
                } else {
                    SemanticTerminatorKindV1::Return
                },
            ),
            block(
                5,
                vec![assign(0, SCALAR, SemanticRvalueKindV1::Use(constant(0)))],
                SemanticTerminatorKindV1::Return,
            ),
            block(6, vec![], SemanticTerminatorKindV1::Unreachable),
        ];
        if matches!(case, Case::Single) {
            blocks[0] = block(0, vec![], goto(1));
        }
        if matches!(case, Case::Reordered) {
            blocks.swap(1, 2);
        }
        let locals = [
            (SCALAR, SemanticLocalRoleV1::Return),
            (SCALAR, SemanticLocalRoleV1::Argument(0)),
            (SCALAR, SemanticLocalRoleV1::Argument(1)),
            (SCALAR, SemanticLocalRoleV1::Argument(2)),
            (ENUM, SemanticLocalRoleV1::Temporary),
            (SCALAR, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([100 + index as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([110; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([111; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([112; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([113; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([114; 32]),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap();
        Self { types, function }
    }

    fn plans(
        &self,
    ) -> (
        ProductionSemanticSsaFunctionPlanV1,
        SemanticControlFlowSsaPlanV1,
    ) {
        let source = plan_semantic_function_ssa_with_module_v1(
            FUNCTION,
            &self.function,
            &self.types,
            &[],
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        let producers = semantic_option_producers_v1(&self.function, &[]).unwrap();
        let dominance = SemanticOptionDominanceV1::analyze(&self.function, &producers).unwrap();
        let lowered = SemanticControlFlowSsaPlanV1::analyze(
            SemanticSsaTransportInputV1 {
                types: &self.types,
                callables: &[],
                function: &self.function,
                semantic_function: FUNCTION,
            },
            &source,
            &dominance,
            &BTreeMap::new(),
            100_000,
            100_000,
        )
        .unwrap();
        (source, lowered)
    }

    fn bindings(&self) -> Vec<Option<SemanticValueBindingV1>> {
        let mut bindings = vec![None; self.function.locals().len()];
        for (local, slot) in bindings.iter_mut().enumerate().take(4).skip(1) {
            *slot = Some(SemanticValueBindingV1::Value {
                id: ValueId(10 + local as u32),
                ty: Type::Scalar(ScalarType::U64),
            });
        }
        bindings
    }

    fn with_lowering<T>(
        &self,
        enabled: bool,
        body: impl FnOnce(
            &mut SemanticFunctionLoweringV1<'_>,
            &ProductionSemanticSsaFunctionPlanV1,
        ) -> T,
    ) -> T {
        let (plan, _) = self.plans();
        let declarations = [(0, 1, SCALAR), (1, 2, SCALAR), (2, 3, SCALAR)];
        let values = [ValueId(11), ValueId(12), ValueId(13)];
        let types = [
            Type::Scalar(ScalarType::U64),
            Type::Scalar(ScalarType::U64),
            Type::Scalar(ScalarType::U64),
        ];
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        let returns =
            CallReturnBufferV1::for_function(&self.function, &[], &BTreeMap::new(), 1, &mut budget)
                .unwrap();
        let mut lowering = SemanticFunctionLoweringV1::new_interprocedural(
            &self.types,
            &[],
            &self.function,
            &plan,
            FUNCTION,
            FUNCTION,
            BTreeMap::new(),
            BTreeMap::new(),
            vec![Type::Scalar(ScalarType::U64)],
            SemanticParameterBindingsV1 {
                declarations: &declarations,
                values: &values,
                types: &types,
                local_bindings: None,
            },
            None,
            None,
            BTreeSet::new().into(),
            1,
            false,
            100_000,
            PrivateArrayRecorderWorkV1::Owned(PrivateArrayLazyBudgetV1::new(1, 100_000)),
            None,
            returns,
            enabled.then_some(&mut budget as &mut dyn SemanticEmissionBudgetV1),
            SemanticEmissionPlacementV1::default(),
            None,
            None,
        )
        .unwrap();
        body(&mut lowering, &plan)
    }
}

fn plan_with_limits(
    fixture: &Fixture,
    work_limit: usize,
    storage_limit: usize,
) -> Result<(Vec<ScalarEnumPayloadV1>, usize, usize), ProductionSemanticKirErrorV1> {
    let (source, ssa) = fixture.plans();
    let (dominance, variants) = restoration_facts(fixture, &ssa);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let rows = plan_scalar_enum_payloads_v1(
        &fixture.types,
        &fixture.function,
        &source,
        &ssa,
        &fixture.bindings(),
        ScalarEnumRestorationFactsV1 {
            dominance: &dominance,
            variants: &variants,
        },
        &mut budget,
    )?;
    let storage = budget.storage();
    drop(budget);
    Ok((rows, work.work(), storage))
}

fn restoration_facts(
    fixture: &Fixture,
    ssa: &SemanticControlFlowSsaPlanV1,
) -> (
    SemanticEnumPayloadDominanceV1,
    BTreeMap<(u32, SsaValueV1), u32>,
) {
    (
        SemanticEnumPayloadDominanceV1::analyze(&fixture.function, &fixture.types).unwrap(),
        analyze_promoted_enum_variants_v1(&fixture.types, &fixture.function, ssa, 100_000, 100_000)
            .unwrap(),
    )
}

#[test]
fn immutable_entry_payload_uses_exact_ssa_across_enum_phi() {
    for case in [Case::Ordinary, Case::SameProducer, Case::Reordered] {
        let fixture = Fixture::new(case);
        let (rows, _, _) = plan_with_limits(&fixture, 1_000_000, 1_000_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, (4, 0, 0));
        let ScalarEnumCandidateV1::Fixed(source) = rows[0].source else {
            panic!("exact producer required");
        };
        assert_eq!(source.value, ValueId(12));
        let (_, ssa) = fixture.plans();
        assert_eq!(Some(&source.ssa), ssa.entry_definitions.get(&2));
        let facts = analyze_promoted_enum_variants_v1(
            &fixture.types,
            &fixture.function,
            &ssa,
            100_000,
            100_000,
        )
        .unwrap();
        let current = ssa.entry_value(&fixture.function, 4, 4).unwrap();
        assert_eq!(facts.get(&(4, current)), Some(&0));
        assert!(
            !rows.iter().any(|row| row.key.1 == 1),
            "absent variant has no fabricated payload"
        );
        // The shared local-only guard cannot prove these multiple definitions.
        let old =
            SemanticEnumPayloadDominanceV1::analyze(&fixture.function, &fixture.types).unwrap();
        assert!(
            old.availability(SemanticLocalIdV1::from_index(4), 0)
                .is_none()
        );
    }
}

#[test]
fn conflicting_redefined_and_looping_payloads_retain_storage() {
    for case in [Case::Conflict, Case::RedefinedParameter, Case::Backedge] {
        let fixture = Fixture::new(case);
        assert!(
            plan_with_limits(&fixture, 1_000_000, 1_000_000)
                .unwrap()
                .0
                .is_empty()
        );
    }
}

#[test]
fn scalar_plan_does_not_reconstruct_pointers_or_compiler_issued_values() {
    let fixture = Fixture::new(Case::Ordinary);
    let (source, mut ssa) = fixture.plans();
    let (dominance, variants) = restoration_facts(&fixture, &ssa);
    for binding in [
        SemanticValueBindingV1::Value {
            id: ValueId(12),
            ty: Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            ),
        },
        SemanticValueBindingV1::Value {
            id: ValueId(12),
            ty: Type::Scalar(ScalarType::U32),
        },
        SemanticValueBindingV1::MathContext,
        SemanticValueBindingV1::Unmaterialized,
    ] {
        let mut bindings = fixture.bindings();
        bindings[2] = Some(binding);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        assert!(
            plan_scalar_enum_payloads_v1(
                &fixture.types,
                &fixture.function,
                &source,
                &ssa,
                &bindings,
                ScalarEnumRestorationFactsV1 {
                    dominance: &dominance,
                    variants: &variants
                },
                &mut budget
            )
            .unwrap()
            .is_empty()
        );
    }
    ssa.compiler_issued_bindings
        .insert(SCALAR, SemanticPromotedBindingV1::MathContext);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    assert!(
        plan_scalar_enum_payloads_v1(
            &fixture.types,
            &fixture.function,
            &source,
            &ssa,
            &fixture.bindings(),
            ScalarEnumRestorationFactsV1 {
                dominance: &dominance,
                variants: &variants
            },
            &mut budget
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn scalar_payload_analysis_uses_original_exact_work_and_storage_limits() {
    let fixture = Fixture::new(Case::Ordinary);
    let (_, work, storage) = plan_with_limits(&fixture, 1_000_000, 1_000_000).unwrap();
    assert!(work > 1 && storage > 1);
    assert!(plan_with_limits(&fixture, work, storage).is_ok());
    assert!(plan_with_limits(&fixture, work - 1, storage).is_err());
    assert!(plan_with_limits(&fixture, work, storage - 1).is_err());
}

#[test]
fn only_proved_payload_memory_operations_are_omitted() {
    let fixture = Fixture::new(Case::Ordinary);
    for enabled in [false, true] {
        fixture.with_lowering(enabled, |lowering, plan| {
            assert_eq!(lowering.enum_payload_storage.is_empty(), enabled);
            let mut operations = Vec::new();
            for source_block in plan.plan().reverse_postorder() {
                let block = SemanticBlockIdV1::from_index(source_block.get());
                let mut target = BasicBlock::new(BlockId(source_block.get()));
                let prologue = lowering.begin_block(block, &mut target).unwrap();
                if enabled {
                    assert_eq!(prologue.enum_payload_storage, 0);
                }
                for (ordinal, statement) in fixture.function.blocks()[source_block.get() as usize]
                    .statements()
                    .iter()
                    .enumerate()
                {
                    lowering
                        .lower_statement(
                            block,
                            Some(ordinal as u32),
                            statement.kind(),
                            &mut target.operations,
                        )
                        .unwrap();
                }
                lowering
                    .lower_terminator(
                        block,
                        fixture.function.blocks()[source_block.get() as usize]
                            .terminator()
                            .kind(),
                        &mut target.operations,
                    )
                    .unwrap();
                operations.extend(target.operations);
            }
            for is_kind in [
                (|kind: &OperationKind| matches!(kind, OperationKind::Alloca { .. }))
                    as fn(&OperationKind) -> bool,
                |kind| matches!(kind, OperationKind::Store { .. }),
                |kind| matches!(kind, OperationKind::Load { .. }),
            ] {
                assert_eq!(
                    operations.iter().any(|operation| is_kind(&operation.kind)),
                    !enabled
                );
            }
            require_semantic_ssa_definitions_consumed_v1(
                0,
                &lowering.pending_semantic_ssa_definitions,
            )
            .unwrap();
        });
    }
}

#[test]
fn omitted_payload_requires_actual_current_variant_and_original_binding() {
    let fixture = Fixture::new(Case::Ordinary);
    fixture.with_lowering(true, |lowering, _| {
        let mut entry = BasicBlock::new(BlockId(0));
        lowering
            .begin_block(SemanticBlockIdV1::from_index(0), &mut entry)
            .unwrap();
        assert!(
            lowering
                .scalar_enum_payload_v1((4, 0, 0), Some(SemanticBlockIdV1::from_index(5)))
                .is_err()
        );
        let source = match lowering.scalar_enum_payloads[0].source {
            ScalarEnumCandidateV1::Fixed(source) => source,
            _ => unreachable!(),
        };
        lowering.semantic_ssa_bindings.insert(
            source.ssa,
            SemanticValueBindingV1::Value {
                id: ValueId(999),
                ty: Type::Scalar(ScalarType::U64),
            },
        );
        assert!(lowering.scalar_enum_payload_v1((4, 0, 0), None).is_err());
        lowering.semantic_ssa_bindings.remove(&source.ssa);
        assert!(lowering.scalar_enum_payload_v1((4, 0, 0), None).is_err());
    });
}

#[test]
fn source_definition_census_never_treats_projected_or_other_writes_as_constructors() {
    let mut census = vec![ScalarEnumLocalCensusV1::default(); 6];
    let projected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(0), ENUM).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), SCALAR).unwrap(),
        ],
        SCALAR,
    )
    .unwrap();
    scalar_enum_record_definition_v1(&mut census, &projected, true).unwrap();
    scalar_enum_record_definition_v1(&mut census, &place(4, ENUM), false).unwrap();
    assert_eq!(census[4].definitions, 2);
    assert_eq!(census[4].constructors, 0);
}

#[test]
fn legacy_only_restoration_retains_storage_before_any_emission() {
    let fixture = Fixture::new(Case::Single);
    let (_, ssa) = fixture.plans();
    let (dominance, _) = restoration_facts(&fixture, &ssa);
    let guard = dominance
        .availability(SemanticLocalIdV1::from_index(4), 0)
        .unwrap();
    assert!(dominance.allows(guard, SemanticBlockIdV1::from_index(4)));
    // Remove optional SSA evidence deliberately: a legacy fact cannot replace
    // the new route's exact evidence, but must keep the original storage route.
    let variants = BTreeMap::new();
    let source = ScalarEnumProducerV1 {
        semantic_type: SCALAR,
        ssa: ssa.entry_definitions[&2],
        value: ValueId(12),
        scalar: ScalarType::U64,
    };
    let mut rows = vec![ScalarEnumPayloadV1 {
        key: (4, 0, 0),
        semantic_type: SCALAR,
        source: ScalarEnumCandidateV1::Fixed(source),
    }];
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    retain_scalar_enum_restorations_v1(
        &mut rows,
        &fixture.types,
        &fixture.function,
        &ssa,
        ScalarEnumRestorationFactsV1 {
            dominance: &dominance,
            variants: &variants,
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(rows[0].source, ScalarEnumCandidateV1::Rejected);
}

#[test]
fn omitted_store_rejects_same_physical_value_with_capability_custody() {
    let scalar = SemanticValueBindingV1::Value {
        id: ValueId(17),
        ty: Type::Scalar(ScalarType::U32),
    };
    let capability = SemanticValueBindingV1::WaveLane {
        value: ValueId(17),
        wave: SemanticCurrentWaveV1::new(64),
    };
    assert_eq!(
        scalar.value(),
        capability.value(),
        "the broad value projection is not enough"
    );
    assert!(!scalar_enum_values_match_v1(&scalar, &capability));
    assert!(!scalar_enum_values_match_v1(&capability, &scalar));
    assert!(scalar_enum_values_match_v1(&scalar, &scalar));
}

#[test]
fn payload_plan_consumes_the_callers_existing_work_and_storage_prefix() {
    let fixture = Fixture::new(Case::Ordinary);
    let (_, used_work, used_storage) = plan_with_limits(&fixture, 1_000_000, 1_000_000).unwrap();
    let (source, ssa) = fixture.plans();
    let (dominance, variants) = restoration_facts(&fixture, &ssa);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(used_work + 7);
    work.charge_work(7).unwrap();
    let mut budget = ArgumentBudgetV1::new(&mut work, used_storage + 32);
    budget.reserve_storage(32).unwrap();
    let rows = plan_scalar_enum_payloads_v1(
        &fixture.types,
        &fixture.function,
        &source,
        &ssa,
        &fixture.bindings(),
        ScalarEnumRestorationFactsV1 {
            dominance: &dominance,
            variants: &variants,
        },
        &mut budget,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(budget.storage(), used_storage + 32);
    drop(rows);
    // The caller releases the retained reservations only after all results
    // drop, matching the existing module-level storage floor contract.
    budget.release_storage(used_storage).unwrap();
    assert_eq!(budget.storage(), 32);
    drop(budget);
    assert_eq!(work.work(), used_work + 7);
}

fn aggregate_fixture(moved: bool) -> Fixture {
    let mut f = mixed_fixture(moved);
    f.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8, 16, 16], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(
            SemanticAggregateTypeV1::new(vec![ty(1), ty(1), ty(0), ty(6)]).unwrap(),
        ),
    ));
    f.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([92; 32]),
        SemanticLayoutIdentityV1::from_sha256([92; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    let variants = (0..2)
        .map(|v| {
            let offsets = if v == 0 { vec![] } else { vec![8] };
            let order = if v == 0 { vec![] } else { vec![0] };
            SemanticEnumVariantLayoutV1::from_rustc(
                v,
                24,
                8,
                SemanticFieldsShapeV1::arbitrary(offsets.clone(), order).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    f.types[2] = SemanticTypeDeclV1::new(
        f.types[2].identity(),
        SemanticLayoutIdentityV1::from_sha256([93; 32]),
        SemanticTypeLayoutV1::enum_layout(
            24,
            8,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(
                    0,
                    0,
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(false, 64, 8),
                        SemanticScalarValidityRangeV1::new(0, 1),
                    ),
                )),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: ty(1),
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![ty(5)]).unwrap()),
            ]
            .into_boxed_slice(),
        },
    );
    let mut locals = f.function.locals().to_vec();
    for (n, t) in [(12, 5), (13, 5), (14, 2)] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([60 + n; 32]),
            ty(t),
            SemanticLocalRoleV1::Temporary,
            source(),
        ));
    }
    let original = &f.function;
    f.function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        locals,
        original.entry(),
        original.blocks().to_vec(),
    )
    .unwrap();
    let zero = |t| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty(t),
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    f.replace_block(
        1,
        vec![
            f.function.blocks()[1].statements()[0].clone(),
            assign(
                12,
                5,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::Aggregate,
                    vec![copy(9, 1), constant(64), zero(0), zero(6)],
                )
                .unwrap(),
            ),
            some(2, SemanticOperandV1::Move(place(12, 5))),
        ],
        goto(3),
    );
    f.replace_block(
        2,
        vec![
            assign(
                14,
                2,
                SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                    .unwrap(),
            ),
            assign(
                2,
                2,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(14, 2))),
            ),
        ],
        goto(3),
    );
    let selected = SemanticPlaceV1::new(
        local(3),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), ty(2)).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(5)).unwrap(),
        ],
        ty(5),
    )
    .unwrap();
    f.replace_block(
        6,
        vec![
            assign(
                13,
                5,
                SemanticRvalueKindV1::Use(if moved {
                    SemanticOperandV1::Move(selected)
                } else {
                    SemanticOperandV1::Copy(selected)
                }),
            ),
            assign(5, 1, SemanticRvalueKindV1::Use(field(13, 5, 0, 1))),
        ],
        SemanticTerminatorKindV1::Return,
    );
    f
}

fn aggregate_missing(f: &Fixture) {
    let owner = mixed_owner(f);
    let error = match mixed_emit(&owner) {
        Ok(_) => panic!("unproved aggregate restored"),
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
fn ordinary_enum_aggregate_mixed_original_constructor_and_marker_custody() {
    for moved in [false, true] {
        let owner = mixed_owner(&aggregate_fixture(moved));
        let original = owner.source_semantic().functions().to_vec();
        let (blocks, lower) = mixed_emit(&owner).unwrap();
        assert_eq!(owner.source_semantic().functions(), original);
        assert!(!lower.control_flow_ssa.promoted.contains_key(&3));
        assert!(lower.control_flow_ssa.promoted.contains_key(&2));
        assert!(!lower.enum_payload_sources.contains_key(&(2, 1, 0)));
        assert!(lower.enum_aggregate_custody.is_some());
        assert!(lower.enum_mixed_aliases.is_none());
        let slot = &lower.enum_payload_storage[&(2, 1, 0)];
        assert_eq!(slot.semantic_type, ty(5));
        assert_eq!(slot.components.len(), 2);
        for component in &slot.components {
            let stores = blocks.iter().flat_map(|b| b.operations.iter().filter_map(move |op|
                matches!(op.kind, OperationKind::Store { pointer, .. } if pointer == component.pointer).then_some(b.id.0))).collect::<Vec<_>>();
            let loads = blocks.iter().flat_map(|b| b.operations.iter().filter_map(move |op|
                matches!(op.kind, OperationKind::Load { pointer, .. } if pointer == component.pointer).then_some(b.id.0))).collect::<Vec<_>>();
            assert_eq!(stores, [1]);
            assert_eq!(loads, [6]);
        }
        for site in [(1, 12), (6, 13)] {
            let [value] = lower.control_flow_ssa.definition_values[&site].as_slice() else {
                panic!("one original SSA definition");
            };
            let SemanticValueBindingV1::Aggregate(fields) = &lower.semantic_ssa_bindings[value]
            else {
                panic!("original aggregate binding");
            };
            assert_eq!(fields.len(), 4);
            assert!(matches!(
                &fields[0],
                SemanticValueBindingV1::Value {
                    ty: Type::Scalar(ScalarType::U64),
                    ..
                }
            ));
            assert!(matches!(
                &fields[1],
                SemanticValueBindingV1::Value {
                    ty: Type::Scalar(ScalarType::U64),
                    ..
                }
            ));
            assert!(matches!(fields[2], SemanticValueBindingV1::Unit));
            assert!(matches!(&fields[3], SemanticValueBindingV1::Aggregate(v) if v.is_empty()));
        }
    }
}

#[test]
fn ordinary_enum_aggregate_original_owner_reaches_canonical_kir() {
    for moved in [false, true] {
        let owner = mixed_owner(&aggregate_fixture(moved));
        let original = owner.source_semantic().semantic_sha256();
        let (module, correspondence) =
            lower_module(&owner, ProductionSemanticKirLimitsV1::default(), None, &[]).unwrap();
        assert_eq!(correspondence.semantic_sha256(), original.as_bytes());
        let canonical = ProductionCanonicalKernelIrV1::from_module(module).unwrap();
        assert!(!canonical.canonical_bytes().is_empty());
    }
}

#[test]
fn ordinary_enum_aggregate_scalar_alias_keeps_existing_proof_owner() {
    for moved in [false, true] {
        let owner = mixed_owner(&mixed_fixture(moved));
        let (_, lower) = mixed_emit(&owner).unwrap();
        assert!(lower.enum_aggregate_custody.is_none());
        assert!(lower.enum_mixed_aliases.is_some());
        assert!(lower.enum_payload_aliases.is_some());
    }
}

#[test]
fn ordinary_enum_aggregate_payload_alias_is_not_original_constructor_custody() {
    let mut f = aggregate_fixture(true);
    let mut statements = f.function.blocks()[1].statements()[..2].to_vec();
    statements.push(assign(13, 5, SemanticRvalueKindV1::Use(copy(12, 5))));
    statements.push(some(2, SemanticOperandV1::Move(place(13, 5))));
    f.replace_block(1, statements, goto(3));
    aggregate_missing(&f);
}

#[test]
fn ordinary_enum_aggregate_missing_payload_slot_never_restores() {
    let owner = mixed_owner(&aggregate_fixture(true));
    let mut lower = mixed_lowering(&owner);
    assert!(lower.enum_payload_storage.remove(&(2, 1, 0)).is_some());
    let error = match aggregate_run_with(&owner, lower) {
        Ok(_) => panic!("missing constructor storage"),
        Err(error) => error,
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
fn ordinary_enum_aggregate_cached_proof_rechecks_bound_types_and_storage() {
    let owner = mixed_owner(&aggregate_fixture(true));
    for mutation in 0..7 {
        let (_, mut lower) = mixed_emit(&owner).unwrap();
        let [value] = lower.control_flow_ssa.definition_values[&(11, 3)].as_slice() else {
            panic!("one final alias definition");
        };
        lower.locals[3] = Some(lower.semantic_ssa_bindings[value].clone());
        assert!(matches!(lower.locals[3].as_ref(),
            Some(SemanticValueBindingV1::Enum { payloads, .. }) if payloads.is_empty()));
        if mutation < 5 {
            let bound = [2, 5, 1, 0, 6][mutation];
            lower
                .control_flow_ssa
                .compiler_issued_bindings
                .insert(ty(bound), SemanticPromotedBindingV1::KernelContext);
        } else if mutation == 5 {
            lower.enum_payload_storage.remove(&(2, 1, 0));
        } else {
            lower
                .enum_payload_storage
                .get_mut(&(2, 1, 0))
                .unwrap()
                .components[0]
                .kernel_type = Type::Scalar(ScalarType::Bool);
        }
        let mut operations = Vec::new();
        lower
            .refine_plain_enum_alias(
                SemanticBlockIdV1::from_index(6),
                Some(0),
                3,
                &mut operations,
            )
            .unwrap();
        assert!(operations.is_empty(), "mutation {mutation}");
        assert!(
            matches!(lower.locals[3].as_ref(),
            Some(SemanticValueBindingV1::Enum { payloads, .. }) if payloads.is_empty()),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_enum_aggregate_missing_inverted_and_bypass_guards_reject() {
    for mutation in 0..3 {
        let mut f = aggregate_fixture(true);
        match mutation {
            0 => f.replace_block(4, vec![], goto(6)),
            1 => f.replace_block(
                4,
                f.function.blocks()[4].statements().to_vec(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: copy(4, 1),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 6),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                    )
                    .unwrap(),
                },
            ),
            _ => f.replace_block(5, vec![], goto(6)),
        }
        aggregate_missing(&f);
    }
}

#[test]
fn ordinary_enum_aggregate_competing_selected_constructor_rejects() {
    let mut f = aggregate_fixture(true);
    let source = f.function.blocks()[1].statements().to_vec();
    f.replace_block(2, source, goto(3));
    aggregate_missing(&f);
}

#[test]
fn ordinary_enum_aggregate_none_alias_substituted_with_selected_source_rejects() {
    let mut f = aggregate_fixture(true);
    let mut source = f.function.blocks()[1].statements()[..2].to_vec();
    source.push(some(14, SemanticOperandV1::Move(place(12, 5))));
    source.push(assign(
        2,
        2,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(14, 2))),
    ));
    f.replace_block(2, source, goto(3));
    aggregate_missing(&f);
}

#[test]
fn ordinary_enum_aggregate_slot_overwrite_after_capture_rejects() {
    let mut f = aggregate_fixture(false);
    let mut source = f.function.blocks()[3].statements()[..1].to_vec();
    source.push(assign(
        2,
        2,
        SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![]).unwrap(),
    ));
    f.replace_block(3, source, goto(10));
    aggregate_missing(&f);
}

#[test]
fn ordinary_enum_aggregate_wrong_lifetime_and_backedge_reject() {
    for mutation in 0..2 {
        let mut f = aggregate_fixture(true);
        if mutation == 0 {
            f.replace_block(
                3,
                f.function.blocks()[3].statements()[..1].to_vec(),
                goto(10),
            );
            let mut statements = f.function.blocks()[10].statements().to_vec();
            statements.push(lifetime_marker(true, 2));
            f.replace_block(10, statements, goto(11));
        } else {
            // Refill the moved local on the backedge without breaking cyclic lineage.
            f.replace_block(
                5,
                vec![assign(
                    10,
                    2,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, 2))),
                )],
                goto(10),
            );
        }
        aggregate_missing(&f);
    }
}

#[test]
fn ordinary_enum_aggregate_copy_backedge_rejects_cyclic_lineage() {
    let mut f = aggregate_fixture(false);
    f.replace_block(5, vec![], goto(10));
    aggregate_missing(&f);
}

#[test]
fn ordinary_enum_aggregate_unreinitialized_move_backedge_rejects_in_planner() {
    let mut f = aggregate_fixture(true);
    f.replace_block(5, vec![], goto(10));
    let result = plan_semantic_function_ssa_with_module_v1(
        SemanticFunctionIdV1::from_index(0),
        &f.function,
        &f.types,
        &[],
        ProductionSemanticSsaLimitsV1::default(),
    );
    assert!(
        matches!(
            &result,
            Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
                function,
                error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtEdge {
                    edge,
                    target,
                    variable,
                },
            }) if function.index() == 0 && edge.source().get() == 5 && edge.ordinal() == 0
                && target.get() == 10 && variable.get() == 10
        ),
        "{result:?}"
    );
}

#[test]
fn ordinary_enum_aggregate_final_alias_reassignment_rejects() {
    let mut f = aggregate_fixture(false);
    let mut statements = f.function.blocks()[11].statements().to_vec();
    statements.push(assign(3, 2, SemanticRvalueKindV1::Use(copy(11, 2))));
    f.replace_block(11, statements, goto(4));
    aggregate_missing(&f);
}

fn aggregate_run_with<'a>(
    owner: &'a fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    mut lower: SemanticFunctionLoweringV1<'a>,
) -> Result<SemanticFunctionLoweringV1<'a>, ProductionSemanticKirErrorV1> {
    for b in owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
        .unwrap()
        .plan()
        .reverse_postorder()
    {
        let id = SemanticBlockIdV1::from_index(b.get());
        let body = &lower.function.blocks()[b.get() as usize];
        let mut target = BasicBlock::new(BlockId(b.get()));
        lower.begin_block(id, &mut target)?;
        for (s, item) in body.statements().iter().enumerate() {
            lower.lower_statement(id, Some(s as u32), item.kind(), &mut target.operations)?;
        }
        lower.lower_terminator(id, body.terminator().kind(), &mut target.operations)?;
    }
    require_semantic_ssa_definitions_consumed_v1(0, &lower.pending_semantic_ssa_definitions)?;
    Ok(lower)
}

#[test]
fn ordinary_enum_aggregate_missing_owner_never_fabricates_markers() {
    let owner = mixed_owner(&aggregate_fixture(true));
    let mut lower = mixed_lowering(&owner);
    lower.enum_alias_source = None;
    let error = match aggregate_run_with(&owner, lower) {
        Ok(_) => panic!("missing owner"),
        Err(e) => e,
    };
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::EnumPayloadUnavailable {
                block: 6,
                local: 3,
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn ordinary_enum_aggregate_identical_bytes_foreign_owner_rejects() {
    let f = aggregate_fixture(true);
    let a = mixed_owner(&f);
    let b = mixed_owner(&f);
    let mut lower = mixed_lowering(&a);
    let root = SemanticFunctionIdV1::from_index(0);
    lower.enum_alias_source = Some(
        b.source_query_for_root(root, b.execution_view_for_root(root).unwrap().body())
            .unwrap(),
    );
    let error = match aggregate_run_with(&a, lower) {
        Ok(_) => panic!("foreign owner"),
        Err(e) => e,
    };
    assert!(
        matches!(error, ProductionSemanticKirErrorV1::CorrespondenceMismatch),
        "{error:?}"
    );
}

#[test]
fn ordinary_enum_aggregate_compiler_bound_outer_payload_or_marker_never_initializes() {
    let owner = mixed_owner(&aggregate_fixture(true));
    for bound in [2, 5, 1, 0, 6] {
        let mut lower = mixed_lowering(&owner);
        lower
            .control_flow_ssa
            .compiler_issued_bindings
            .insert(ty(bound), SemanticPromotedBindingV1::KernelContext);
        lower
            .retain_plain_enum_constructor(
                SemanticBlockIdV1::from_index(1),
                Some(2),
                local(2),
                1,
                0,
                &SemanticValueBindingV1::Aggregate(vec![]),
            )
            .unwrap();
        assert!(
            lower.enum_aggregate_custody.is_none(),
            "compiler bound type {bound}"
        );
    }
}

#[test]
fn ordinary_enum_aggregate_pointer_wide_and_nested_data_shapes_never_initialize() {
    let owner = mixed_owner(&aggregate_fixture(true));
    for mutation in 0..3 {
        let mut types = owner.source_semantic().types().to_vec();
        let shape = match mutation {
            0 => SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    ty(0),
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
            1 => SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 128,
            }),
            _ => SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![ty(0)]).unwrap()),
        };
        types[1] = SemanticTypeDeclV1::new(
            types[1].identity(),
            types[1].layout_identity(),
            types[1].layout().clone(),
            shape,
        );
        let mut lower = mixed_lowering(&owner);
        lower.types = &types;
        lower
            .retain_plain_enum_constructor(
                SemanticBlockIdV1::from_index(1),
                Some(2),
                local(2),
                1,
                0,
                &SemanticValueBindingV1::Aggregate(vec![]),
            )
            .unwrap();
        assert!(
            lower.enum_aggregate_custody.is_none(),
            "unsupported data shape {mutation}"
        );
    }
}

#[test]
fn ordinary_enum_aggregate_constructor_operand_binding_substitution_rejects() {
    let owner = mixed_owner(&aggregate_fixture(true));
    let mut lower = mixed_lowering(&owner);
    let root = SemanticFunctionIdV1::from_index(0);
    let function = owner.execution_view_for_root(root).unwrap().body();
    // Prepare the genuine original aggregate and query its original Move operand.
    for b in owner
        .execution_plan_for_root(root)
        .unwrap()
        .plan()
        .reverse_postorder()
    {
        let id = SemanticBlockIdV1::from_index(b.get());
        let mut target = BasicBlock::new(BlockId(b.get()));
        lower.begin_block(id, &mut target).unwrap();
        for (s, item) in function.blocks()[b.get() as usize]
            .statements()
            .iter()
            .enumerate()
        {
            if b.get() == 1 && s == 2 {
                let original = lower.locals[12].clone().unwrap();
                let SemanticValueBindingV1::Aggregate(fields) = original else {
                    panic!("original data");
                };
                for mutation in 0..3 {
                    let mut changed = fields.clone();
                    match mutation {
                        0 => changed.swap(0, 1),
                        1 => changed[2] = SemanticValueBindingV1::Aggregate(vec![]),
                        _ => changed[3] = SemanticValueBindingV1::Unit,
                    }
                    lower
                        .retain_plain_enum_constructor(
                            id,
                            Some(2),
                            local(2),
                            1,
                            0,
                            &SemanticValueBindingV1::Aggregate(changed),
                        )
                        .unwrap();
                }
                // A subsequent original constructor must not report duplicate
                // custody. The substituted attempt retained nothing.
                lower
                    .lower_statement(id, Some(2), item.kind(), &mut target.operations)
                    .unwrap();
                return;
            }
            lower
                .lower_statement(id, Some(s as u32), item.kind(), &mut target.operations)
                .unwrap();
        }
        lower
            .lower_terminator(
                id,
                function.blocks()[b.get() as usize].terminator().kind(),
                &mut target.operations,
            )
            .unwrap();
    }
    panic!("constructor not visited");
}

fn aggregate_guard_tables(
    lower: &mut SemanticFunctionLoweringV1<'_>,
    block: u32,
    live_count: usize,
) {
    lower.control_flow_ssa.live_in.clear();
    lower.control_flow_ssa.live_in.insert(
        block,
        (0..15).filter(|l| *l != 3).take(live_count).collect(),
    );
    lower.control_flow_ssa.entry_definitions.clear();
    lower.control_flow_ssa.block_entry_values.clear();
    lower.promoted_enum_variant_by_value.clear();
}

#[test]
fn ordinary_enum_aggregate_guard_live_in_and_fallback_have_independent_boundaries() {
    let owner = mixed_owner(&aggregate_fixture(true));
    for live_count in [0, 4, 8] {
        for variant in [0, 1, 2] {
            for short in [false, true] {
                let mut lower = mixed_lowering(&owner);
                aggregate_guard_tables(&mut lower, 6, live_count);
                for v in [0, 1] {
                    assert!(
                        lower
                            .enum_payload_dominance
                            .availability(local(3), v)
                            .is_some()
                    );
                }
                assert_eq!(
                    lower
                        .enum_payload_dominance
                        .availability_lookup_work_units(local(3)),
                    3
                );
                assert_eq!(
                    lower
                        .enum_payload_dominance
                        .availability_lookup_work_units(local(1)),
                    1
                );
                assert_eq!(
                    lower
                        .enum_payload_dominance
                        .availability_lookup_work_units(local(u32::MAX)),
                    1
                );
                // One live-in map row costs 2 per lookup; other maps are empty.
                // 1 + 2 + (2 + n + 1) + (1 + 1) + (1 + 2 + 1).
                let expected = 12 + live_count;
                let prefix = 17;
                let limit = prefix + expected - usize::from(short);
                lower.enum_analysis_budget = SemanticEnumAnalysisBudgetV1::new(limit, 19);
                lower.enum_analysis_budget.charge_work(prefix).unwrap();
                lower.enum_analysis_budget.charge_storage(19).unwrap();
                let result = lower.metered_enum_variant_is_available_v1(
                    local(3),
                    variant,
                    SemanticBlockIdV1::from_index(6),
                );
                if short {
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual,
                        limit: actual_limit,
                    }) if actual == prefix + expected && actual_limit == limit)
                    );
                } else {
                    assert_eq!(result.unwrap(), variant == 1);
                }
                assert_eq!(lower.enum_analysis_budget.work, prefix + expected);
                assert_eq!(lower.enum_analysis_budget.storage, 19);
            }
        }
    }
}

#[test]
fn ordinary_enum_aggregate_guard_entry_lookup_growth_ignores_live_in_scan() {
    let owner = mixed_owner(&aggregate_fixture(true));
    for (rows, lookup_cost) in [(0, 1), (1, 2), (4, 4), (8, 5)] {
        for live_count in [0, 8] {
            for short in [false, true] {
                let mut lower = mixed_lowering(&owner);
                let entry = lower.function.entry();
                aggregate_guard_tables(&mut lower, entry.index(), live_count);
                for l in 0..rows {
                    let value = SsaValueV1::BlockArgument {
                        block: SsaBlockIdV1::new(entry.index()),
                        variable: fe2o3_mir_model::SsaVariableIdV1::new(l),
                    };
                    lower.control_flow_ssa.entry_definitions.insert(l, value);
                    lower
                        .promoted_enum_variant_by_value
                        .insert((entry.index(), value), 1);
                }
                // No live-in or block-entry map lookup is taken at function entry.
                // Fixed work is 1 + 1 metadata read + (1 + 2) fallback + 1 allows.
                let expected = 6 + 2 * lookup_cost;
                let prefix = 9;
                let limit = prefix + expected - usize::from(short);
                lower.enum_analysis_budget = SemanticEnumAnalysisBudgetV1::new(limit, 7);
                lower.enum_analysis_budget.charge_work(prefix).unwrap();
                lower.enum_analysis_budget.charge_storage(7).unwrap();
                let result = lower.metered_enum_variant_is_available_v1(local(3), 1, entry);
                if short {
                    assert!(
                        matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit {
                        resource: ProductionSemanticKirResourceV1::AnalysisWork,
                        actual,
                        limit: actual_limit,
                    }) if actual == prefix + expected && actual_limit == limit)
                    );
                } else {
                    assert_eq!(result.unwrap(), rows >= 4);
                }
                assert_eq!(lower.enum_analysis_budget.work, prefix + expected);
                assert_eq!(lower.enum_analysis_budget.storage, 7);
            }
        }
    }
}

#[test]
fn ordinary_enum_aggregate_rejected_guard_charges_delta_before_any_reload() {
    let owner = mixed_owner(&aggregate_fixture(true));
    for live_count in [0, 4, 8] {
        for boundary in 0..3 {
            let (_, mut lower) = mixed_emit(&owner).unwrap();
            let [value] = lower.control_flow_ssa.definition_values[&(11, 3)].as_slice() else {
                panic!("one final alias definition");
            };
            lower.locals[3] = Some(lower.semantic_ssa_bindings[value].clone());
            assert!(lower.control_flow_ssa.compiler_issued_bindings.is_empty());
            aggregate_guard_tables(&mut lower, 5, live_count);
            // Eligibility is 2 + 2 enum variants + 2 aggregate + 4 * 2 fields.
            // The rejected guard costs 12 + n, independent of a measured run.
            let prefix = 17;
            let complete = prefix + 14 + 12 + live_count;
            let before_scan = prefix + 14 + 6 + live_count;
            let (limit, spent) = match boundary {
                0 => (complete, complete),
                1 => (complete - 1, complete),
                _ => (before_scan - 1, before_scan),
            };
            lower.enum_analysis_budget = SemanticEnumAnalysisBudgetV1::new(limit, 19);
            lower.enum_analysis_budget.charge_work(prefix).unwrap();
            lower.enum_analysis_budget.charge_storage(19).unwrap();
            let mut operations = Vec::new();
            let result = lower.refine_plain_enum_alias(
                SemanticBlockIdV1::from_index(5),
                Some(0),
                3,
                &mut operations,
            );
            if boundary == 0 {
                result.unwrap();
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual,
                    limit: actual_limit,
                }) if actual == spent && actual_limit == limit)
                );
            }
            assert!(operations.is_empty());
            assert!(matches!(lower.locals[3].as_ref(),
                Some(SemanticValueBindingV1::Enum { payloads, .. }) if payloads.is_empty()));
            assert_eq!(lower.enum_analysis_budget.work, spent);
            assert_eq!(lower.enum_analysis_budget.storage, 19);
        }
    }
}

#[test]
fn ordinary_enum_aggregate_shared_work_and_storage_exact_boundaries() {
    let owner = mixed_owner(&aggregate_fixture(true));
    let initial = mixed_lowering(&owner);
    let prefix = (
        initial.enum_analysis_budget.work,
        initial.enum_analysis_budget.storage,
    );
    let measured = aggregate_run_with(&owner, initial).unwrap();
    let total = (
        measured.enum_analysis_budget.work,
        measured.enum_analysis_budget.storage,
    );
    assert!(total.0 > prefix.0 && total.1 > prefix.1);
    for resource in 0..3 {
        let mut lower = mixed_lowering(&owner);
        lower.enum_analysis_budget = SemanticEnumAnalysisBudgetV1::new(
            total.0 - usize::from(resource == 1),
            total.1 - usize::from(resource == 2),
        );
        lower.enum_analysis_budget.charge_work(prefix.0).unwrap();
        lower.enum_analysis_budget.charge_storage(prefix.1).unwrap();
        match aggregate_run_with(&owner, lower) {
            Ok(lower) => {
                assert_eq!(resource, 0);
                assert_eq!(
                    (
                        lower.enum_analysis_budget.work,
                        lower.enum_analysis_budget.storage
                    ),
                    total
                );
            }
            Err(ProductionSemanticKirErrorV1::ResourceLimit {
                resource: actual,
                actual: spent,
                limit,
            }) => {
                assert_ne!(resource, 0);
                assert_eq!(
                    actual,
                    if resource == 1 {
                        ProductionSemanticKirResourceV1::AnalysisWork
                    } else {
                        ProductionSemanticKirResourceV1::AnalysisStorage
                    }
                );
                assert_eq!(
                    limit,
                    if resource == 1 {
                        total.0 - 1
                    } else {
                        total.1 - 1
                    }
                );
                assert!(spent > limit);
            }
            Err(e) => panic!("unexpected failure: {e:?}"),
        }
    }
}

fn mixed_fixture(moved: bool) -> Fixture {
    let mut f = Fixture::new(moved);
    f.types[1] = plain_bit_scalar_type(
        2,
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        },
    );
    f.types[3] = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([4; 32]),
        SemanticLayoutIdentityV1::from_sha256([4; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        f.types[3].shape().clone(),
    );
    let variants = (0..2)
        .map(|index| {
            let offsets = if index == 0 { vec![] } else { vec![8] };
            let order = if index == 0 { vec![] } else { vec![0] };
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                16,
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
        SemanticLayoutIdentityV1::from_sha256([13; 32]),
        SemanticTypeLayoutV1::enum_layout(
            16,
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
        f.types[2].shape().clone(),
    );
    f.types[4] = SemanticTypeDeclV1::new(
        f.types[4].identity(),
        f.types[4].layout_identity(),
        SemanticTypeLayoutV1::aggregate(
            Some(16),
            8,
            SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
        )
        .unwrap(),
        f.types[4].shape().clone(),
    );
    let transfer = |from| {
        if moved {
            SemanticOperandV1::Move(place(from, 2))
        } else {
            copy(from, 2)
        }
    };
    let mut blocks = f.function.blocks().to_vec();
    let old_assert = blocks[0].terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert {
        condition,
        expected,
        message,
        unwind,
        ..
    } = old_assert
    else {
        unreachable!()
    };
    blocks[0] = block(
        0,
        blocks[0].statements().to_vec(),
        SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            unwind,
            target: edge(SemanticEdgeRoleV1::AssertSuccess, 12),
        },
    );
    blocks[3] = block(
        3,
        vec![
            assign(10, 2, SemanticRvalueKindV1::Use(transfer(2))),
            lifetime_marker(true, 2),
        ],
        goto(10),
    );
    blocks.extend([
        block(
            9,
            vec![assign(
                10,
                2,
                SemanticRvalueKindV1::aggregate(SemanticAggregateKindV1::EnumVariant(0), vec![])
                    .unwrap(),
            )],
            goto(10),
        ),
        block(
            10,
            vec![assign(11, 2, SemanticRvalueKindV1::Use(transfer(10)))],
            goto(11),
        ),
        block(
            11,
            vec![assign(3, 2, SemanticRvalueKindV1::Use(transfer(11)))],
            goto(4),
        ),
        block(
            12,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(1, 3),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        0,
                        edge(SemanticEdgeRoleV1::SwitchValue, 9),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 8),
                )
                .unwrap(),
            },
        ),
    ]);
    let mut locals = f.function.locals().to_vec();
    for n in [10, 11] {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([60 + n; 32]),
            ty(2),
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
        blocks,
    )
    .unwrap();
    f
}

fn mixed_owner(f: &Fixture) -> fe2o3_pliron::ProductionSemanticSsaOwnerV1 {
    let seed = super::resource_tests::noop_semantic_owner(&["enum_mixed_lineage"]);
    let semantic = seed.semantic();
    let original = &f.function;
    let abi = SemanticFunctionAbiV1::new(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        false,
        false,
        vec![SemanticAbiValueV1::new(
            ty(3),
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    SemanticAbiExtensionV1::ZeroExtend,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )],
        original.abi().return_value().clone(),
    )
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        semantic.functions()[0].role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        original.locals().to_vec(),
        original.entry(),
        original.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(semantic.functions()[0].kernel_entry().unwrap().clone());
    let root = SemanticFunctionIdV1::from_index(0);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target().clone(),
        f.types.clone(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(root)],
        vec![root],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    owner.verify_replay().unwrap();
    owner
}

fn mixed_lowering(
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) -> SemanticFunctionLoweringV1<'_> {
    let root = SemanticFunctionIdV1::from_index(0);
    let function = owner.execution_view_for_root(root).unwrap().body();
    let mut lower = SemanticFunctionLoweringV1::new(
        owner.source_semantic().types(),
        owner.source_semantic().callables(),
        function,
        SemanticParameterBindingsV1 {
            declarations: &[(0, 1, ty(3))],
            values: &[ValueId(0)],
            types: &[Type::Scalar(ScalarType::Bool)],
            local_bindings: None,
        },
        Some(BlockId(13)),
        None,
        BTreeSet::new(),
        1,
        false,
        1_000_000,
    )
    .unwrap();
    lower.enum_alias_source = Some(owner.source_query_for_root(root, function).unwrap());
    lower
}

fn mixed_emit(
    owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
) -> Result<(Vec<BasicBlock>, SemanticFunctionLoweringV1<'_>), ProductionSemanticKirErrorV1> {
    let root = SemanticFunctionIdV1::from_index(0);
    let function = owner.execution_view_for_root(root).unwrap().body();
    let mut lower = mixed_lowering(owner);
    let mut result = Vec::new();
    for b in owner
        .execution_plan_for_root(root)
        .unwrap()
        .plan()
        .reverse_postorder()
    {
        let id = SemanticBlockIdV1::from_index(b.get());
        let body = &function.blocks()[b.get() as usize];
        let mut target = BasicBlock::new(BlockId(b.get()));
        lower.begin_block(id, &mut target)?;
        for (s, item) in body.statements().iter().enumerate() {
            lower.lower_statement(id, Some(s as u32), item.kind(), &mut target.operations)?;
        }
        target.terminator =
            Some(lower.lower_terminator(id, body.terminator().kind(), &mut target.operations)?);
        result.push(target);
    }
    require_semantic_ssa_definitions_consumed_v1(0, &lower.pending_semantic_ssa_definitions)?;
    Ok((result, lower))
}

#[test]
fn ordinary_enum_alias_mixed_constructor_chain_restores_exact_slot_in_consumer() {
    for moved in [false, true] {
        let owner = mixed_owner(&mixed_fixture(moved));
        let original = owner.source_semantic().functions().to_vec();
        let (blocks, lower) = mixed_emit(&owner).unwrap();
        assert!(!lower.control_flow_ssa.promoted.contains_key(&3));
        assert!(!lower.control_flow_ssa.promoted.contains_key(&11));
        assert!(lower.control_flow_ssa.promoted.contains_key(&2));
        assert!(
            !lower
                .enum_payload_aliases
                .as_ref()
                .unwrap()
                .contains_key(&3)
        );
        assert!(lower.enum_mixed_aliases.is_some());
        let slot = lower.enum_payload_storage[&(2, 1, 0)].components[0].pointer;
        let loads = blocks
            .iter()
            .flat_map(|b| {
                b.operations.iter().filter_map(move |op| {
                    matches!(op.kind, OperationKind::Load { pointer, .. } if pointer == slot)
                        .then_some(b.id.0)
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(loads, [6]);
        let stores = blocks
            .iter()
            .flat_map(|b| {
                b.operations.iter().filter_map(move |op| {
                    matches!(op.kind, OperationKind::Store { pointer, .. } if pointer == slot)
                        .then_some(b.id.0)
                })
            })
            .collect::<Vec<_>>();
        assert_eq!(stores, [1]);
        assert_eq!(owner.source_semantic().functions(), original);
    }
}

#[test]
fn ordinary_enum_alias_mixed_production_owner_hook_reaches_canonical_kir() {
    let owner = mixed_owner(&mixed_fixture(true));
    let (module, correspondence) =
        lower_module(&owner, ProductionSemanticKirLimitsV1::default(), None, &[]).unwrap();
    assert_eq!(
        correspondence.semantic_sha256(),
        owner.source_semantic().semantic_sha256().as_bytes()
    );
    let canonical = ProductionCanonicalKernelIrV1::from_module(module).unwrap();
    assert!(!canonical.canonical_bytes().is_empty());
}

fn mixed_missing(f: &Fixture) {
    let owner = mixed_owner(f);
    let error = match mixed_emit(&owner) {
        Ok(_) => panic!("unproved mixed payload"),
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
fn ordinary_enum_alias_mixed_guard_missing_inverted_and_bypass_remain_required() {
    for mutation in 0..3 {
        let mut f = mixed_fixture(true);
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
        mixed_missing(&f);
    }
}

#[test]
fn ordinary_enum_alias_mixed_other_some_constructor_is_not_an_excluded_incoming() {
    let mut f = mixed_fixture(true);
    f.replace_block(9, vec![some(10, constant(19))], goto(10));
    mixed_missing(&f);
}

#[test]
fn ordinary_enum_alias_mixed_payload_slot_rewrite_after_capture_rejects() {
    let mut f = mixed_fixture(false);
    let mut statements = f.function.blocks()[3].statements().to_vec();
    statements.pop(); // No source lifetime marker is needed to test a live overwrite.
    statements.push(some(2, constant(9)));
    f.replace_block(3, statements, goto(10));
    mixed_missing(&f);
}

#[test]
fn ordinary_enum_alias_mixed_cross_block_source_death_gets_no_new_exception() {
    let mut f = mixed_fixture(true);
    let mut statements = f.function.blocks()[3].statements().to_vec();
    statements.pop();
    f.replace_block(3, statements, goto(10));
    let mut later = f.function.blocks()[10].statements().to_vec();
    later.push(lifetime_marker(true, 2));
    f.replace_block(10, later, goto(11));
    mixed_missing(&f);
}

#[test]
fn ordinary_enum_alias_mixed_killed_intermediate_still_rejects_original_ssa() {
    for dead in [false, true] {
        let mut f = mixed_fixture(true);
        let mut statements = f.function.blocks()[11].statements().to_vec();
        statements.insert(0, lifetime_marker(dead, 11));
        f.replace_block(11, statements, goto(4));
        let result = plan_semantic_function_ssa_with_module_v1(
            SemanticFunctionIdV1::from_index(0),
            &f.function,
            &f.types,
            &[],
            ProductionSemanticSsaLimitsV1::default(),
        );
        assert!(
            matches!(result, Err(fe2o3_pliron::ProductionSemanticSsaErrorV1::Planner {
            error: fe2o3_mir_model::SsaPlannerErrorV1::UndefinedAtUse { block, variable, .. }, ..
        }) if block.get() == 11 && variable.get() == 11),
            "{result:?}"
        );
    }
}

#[test]
fn ordinary_enum_alias_mixed_exact_shared_budget_and_storage_boundary() {
    let owner = mixed_owner(&mixed_fixture(true));
    let transport = mixed_lowering(&owner).control_flow_ssa;
    let root = SemanticFunctionIdV1::from_index(0);
    let function = owner.execution_view_for_root(root).unwrap().body();
    let run = |budget: &mut SemanticEnumAnalysisBudgetV1| {
        let query = owner.source_query_for_root(root, function).unwrap();
        let mut plan = ordinary_enum_alias_v1::MixedAliasPlan::new(
            query,
            owner.source_semantic().types(),
            &transport,
            budget,
        )?;
        plan.storage_owner(
            owner.source_semantic().types(),
            &transport,
            SemanticBlockIdV1::from_index(6),
            Some(0),
            3,
            1,
            budget,
        )
    };
    let mut measured = SemanticEnumAnalysisBudgetV1::new(1_000_000, 1_000_000);
    assert_eq!(run(&mut measured).unwrap(), Some(2));
    let mut exact = SemanticEnumAnalysisBudgetV1::new(measured.work + 17, measured.storage + 19);
    exact.charge_work(17).unwrap();
    exact.charge_storage(19).unwrap();
    assert_eq!(run(&mut exact).unwrap(), Some(2));
    assert_eq!(
        (exact.work, exact.storage),
        (measured.work + 17, measured.storage + 19)
    );
    let mut short = SemanticEnumAnalysisBudgetV1::new(measured.work - 1, measured.storage);
    assert!(
        matches!(run(&mut short), Err(ProductionSemanticKirErrorV1::ResourceLimit {
        resource: ProductionSemanticKirResourceV1::AnalysisWork, actual, limit,
    }) if actual > limit && limit == measured.work - 1)
    );
    let mut short = SemanticEnumAnalysisBudgetV1::new(measured.work, measured.storage - 1);
    assert!(
        matches!(run(&mut short), Err(ProductionSemanticKirErrorV1::ResourceLimit {
        resource: ProductionSemanticKirResourceV1::AnalysisStorage, actual, limit,
    }) if actual > limit && limit == measured.storage - 1)
    );
}

#[test]
fn ordinary_enum_alias_mixed_loop_incoming_cannot_be_optimistically_cached() {
    let mut f = mixed_fixture(false);
    f.replace_block(5, vec![], goto(10));
    mixed_missing(&f);
}

#[test]
fn ordinary_enum_alias_mixed_final_alias_reassignment_does_not_reuse_old_certificate() {
    let mut f = mixed_fixture(false);
    let mut statements = f.function.blocks()[11].statements().to_vec();
    statements.push(assign(3, 2, SemanticRvalueKindV1::Use(copy(11, 2))));
    f.replace_block(11, statements, goto(4));
    mixed_missing(&f);
}

#[test]
fn ordinary_enum_alias_mixed_discriminant_only_does_not_build_lineage() {
    let mut f = mixed_fixture(true);
    f.replace_block(6, vec![], SemanticTerminatorKindV1::Return);
    let owner = mixed_owner(&f);
    let (_, lower) = mixed_emit(&owner).unwrap();
    assert!(lower.enum_alias_source.is_some());
    assert!(lower.enum_mixed_aliases.is_none());
    assert!(lower.enum_payload_aliases.is_none());
}

#[test]
fn ordinary_enum_alias_mixed_missing_owner_keeps_original_rejection() {
    let owner = mixed_owner(&mixed_fixture(true));
    let mut lower = mixed_lowering(&owner);
    lower.enum_alias_source = None;
    let function = lower.function;
    let mut failure = None;
    for b in owner
        .execution_plan_for_root(SemanticFunctionIdV1::from_index(0))
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
            if let Err(e) =
                lower.lower_statement(id, Some(s as u32), item.kind(), &mut target.operations)
            {
                failure = Some(e);
                break;
            }
        }
        if failure.is_some() {
            break;
        }
        lower
            .lower_terminator(
                id,
                function.blocks()[b.get() as usize].terminator().kind(),
                &mut target.operations,
            )
            .unwrap();
    }
    assert!(
        matches!(
            failure,
            Some(ProductionSemanticKirErrorV1::EnumPayloadUnavailable {
                block: 6,
                local: 3,
                variant: 1,
                field: 0,
                available_fields: 0,
                ..
            })
        ),
        "{failure:?}"
    );
    assert!(lower.enum_mixed_aliases.is_none());
}

#[test]
fn ordinary_enum_alias_mixed_source_query_rejects_equal_foreign_body_and_other_root() {
    let a = mixed_owner(&mixed_fixture(true));
    let b = mixed_owner(&mixed_fixture(true));
    let root = SemanticFunctionIdV1::from_index(0);
    assert!(matches!(
        a.source_query_for_root(root, b.execution_view_for_root(root).unwrap().body()),
        Err(fe2o3_pliron::ProductionSemanticSsaSourceQueryErrorV1::WrongOwner)
    ));
    assert!(matches!(
        a.source_query_for_root(
            SemanticFunctionIdV1::from_index(1),
            a.execution_view_for_root(root).unwrap().body()
        ),
        Err(fe2o3_pliron::ProductionSemanticSsaSourceQueryErrorV1::WrongOwner)
    ));
}

#[test]
fn ordinary_enum_alias_mixed_compiler_issued_field_or_changed_width_do_not_get_scalar_slots() {
    let owner = mixed_owner(&mixed_fixture(true));
    let root = SemanticFunctionIdV1::from_index(0);
    let function = owner.execution_view_for_root(root).unwrap().body();
    for mutation in 0..3 {
        let mut transport = mixed_lowering(&owner).control_flow_ssa;
        let mut types = owner.source_semantic().types().to_vec();
        match mutation {
            0 => {
                transport
                    .compiler_issued_bindings
                    .insert(ty(1), SemanticPromotedBindingV1::KernelContext);
            }
            1 => {
                transport
                    .compiler_issued_bindings
                    .insert(ty(2), SemanticPromotedBindingV1::KernelContext);
            }
            _ => {
                types[1] = unsigned_scalar_type(2, 128);
            }
        }
        let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        let mut plan = ordinary_enum_alias_v1::MixedAliasPlan::new(
            a_query(&owner),
            &types,
            &transport,
            &mut budget,
        )
        .unwrap();
        assert_eq!(
            plan.storage_owner(
                &types,
                &transport,
                SemanticBlockIdV1::from_index(6),
                Some(0),
                3,
                1,
                &mut budget
            )
            .unwrap(),
            None
        );
    }
    fn a_query(
        owner: &fe2o3_pliron::ProductionSemanticSsaOwnerV1,
    ) -> fe2o3_pliron::ProductionSemanticSsaSourceQueryV1<'_> {
        let root = SemanticFunctionIdV1::from_index(0);
        owner
            .source_query_for_root(root, owner.execution_view_for_root(root).unwrap().body())
            .unwrap()
    }
    assert_eq!(function.locals()[3].ty(), ty(2));
}

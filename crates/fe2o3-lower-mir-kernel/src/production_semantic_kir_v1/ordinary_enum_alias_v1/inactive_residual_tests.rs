// ControlFlow<Option<Never>, integer>: preserve the original residual and
// checked arithmetic while reading only the selected primitive slot.
fn inactive_residual_type() -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([91; 32]),
        SemanticLayoutIdentityV1::from_sha256([91; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: ty(1),
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new_with_inhabitedness(
                    1,
                    SemanticAggregateTypeV1::new(vec![ty(6)]).unwrap(),
                    true,
                ),
            ]
            .into_boxed_slice(),
        },
    )
}

fn residual_fixture(moved: bool, scalar_variant: u32) -> Fixture {
    let mut fixture = mixed_fixture(moved);
    fixture.function = Fixture::new(moved).function;
    fixture.types.push(inactive_residual_type());
    fixture.types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([92; 32]),
        SemanticLayoutIdentityV1::from_sha256([92; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            1,
            SemanticFieldsShapeV1::Primitive,
            SemanticRustcVariantsV1::Empty,
            SemanticBackendReprV1::memory(true),
            None,
            true,
            None,
            1,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Never,
    ));
    let variants = (0..2)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                16,
                8,
                SemanticFieldsShapeV1::arbitrary(vec![8], vec![0]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticAggregateLayoutV1::new(vec![8], vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    let layout = SemanticTypeLayoutV1::enum_layout(
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
    .unwrap();
    fixture.types[2] = SemanticTypeDeclV1::new(
        fixture.types[2].identity(),
        fixture.types[2].layout_identity(),
        layout,
        SemanticTypeShapeV1::Enum {
            discriminant: ty(1),
            variants: (0..2)
                .map(|index| {
                    SemanticEnumVariantV1::new(
                        u128::from(index),
                        SemanticAggregateTypeV1::new(vec![ty(if index == scalar_variant {
                            1
                        } else {
                            5
                        })])
                        .unwrap(),
                    )
                })
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        },
    );
    fixture.replace_block(
        1,
        vec![
            assign(9, 1, SemanticRvalueKindV1::Use(field(7, 4, 0, 1))),
            assign(
                2,
                2,
                SemanticRvalueKindV1::aggregate(
                    SemanticAggregateKindV1::EnumVariant(scalar_variant),
                    vec![copy(9, 1)],
                )
                .unwrap(),
            ),
        ],
        goto(3),
    );
    fixture.replace_block(
        2,
        vec![assign(
            2,
            2,
            SemanticRvalueKindV1::aggregate(
                SemanticAggregateKindV1::EnumVariant(1 - scalar_variant),
                vec![SemanticOperandV1::Constant(SemanticConstantV1::new(
                    ty(5),
                    SemanticConstantValueV1::ZeroSized,
                ))],
            )
            .unwrap(),
        )],
        goto(3),
    );
    let mut capture = fixture.function.blocks()[3].statements().to_vec();
    capture.extend([
        SemanticStatementV1::new(source(), SemanticStatementKindV1::Nop),
        lifetime_marker(true, 2),
    ]);
    fixture.replace_block(3, capture, goto(4));
    fixture.replace_block(
        4,
        fixture.function.blocks()[4].statements().to_vec(),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: SemanticOperandV1::Move(place(4, 1)),
            targets: SemanticSwitchTargetsV1::new(
                (0..2)
                    .map(|index| {
                        SemanticSwitchTargetV1::new(
                            u128::from(index),
                            edge(
                                SemanticEdgeRoleV1::SwitchValue,
                                if index == scalar_variant { 6 } else { 5 },
                            ),
                        )
                    })
                    .collect(),
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 7),
            )
            .unwrap(),
        },
    );
    let selected = SemanticOperandV1::Move(
        SemanticPlaceV1::new(
            local(3),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::Downcast(scalar_variant),
                    ty(2),
                )
                .unwrap(),
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ty(1)).unwrap(),
            ],
            ty(1),
        )
        .unwrap(),
    );
    fixture.replace_block(
        6,
        vec![assign(5, 1, SemanticRvalueKindV1::Use(selected))],
        goto(5),
    );
    fixture.replace_block(
        5,
        vec![lifetime_marker(true, 3)],
        SemanticTerminatorKindV1::Return,
    );
    fixture
}

#[test]
fn ordinary_enum_alias_residual_original_scalar_consumer_both_variants_move_and_copy() {
    for moved in [false, true] {
        for variant in 0..2 {
            let owner = mixed_owner(&residual_fixture(moved, variant));
            let original = owner.source_semantic().functions().to_vec();
            let (blocks, lower) = mixed_emit(&owner).unwrap();
            assert_eq!(owner.source_semantic().functions(), original);
            let proof = lower.enum_payload_aliases.as_ref().unwrap()[&3];
            assert_eq!(proof.source(), 2);
            assert!(
                lower.enum_mixed_aliases.is_none(),
                "direct proof still takes precedence"
            );
            let slot = &lower.enum_payload_storage[&(2, variant, 0)];
            assert_eq!(slot.components.len(), 1);
            let pointer = slot.components[0].pointer;
            let accesses = blocks
                .iter()
                .flat_map(|b| {
                    b.operations.iter().filter_map(move |op| match op.kind {
                        OperationKind::Store { pointer: p, .. } if p == pointer => {
                            Some((b.id.0, false))
                        }
                        OperationKind::Load { pointer: p, .. } if p == pointer => {
                            Some((b.id.0, true))
                        }
                        _ => None,
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(accesses, [(1, false), (6, true)]);
            // Rust zero size does not erase the existing KIR logical enum tag.
            let residual = &lower.enum_payload_storage[&(2, 1 - variant, 0)];
            assert_eq!(residual.semantic_type, ty(5));
            assert_eq!(residual.exact_enum_variant, None);
            assert!(residual.compiler_issued_binding.is_none());
            let [tag] = residual.components.as_ref() else {
                panic!("one existing logical residual tag component");
            };
            assert_eq!(tag.kernel_type, Type::Scalar(ScalarType::U64));
            assert_eq!(tag.alignment, 8);
            let mut tag_stores = Vec::new();
            for b in &blocks {
                for (index, operation) in b.operations.iter().enumerate() {
                    match operation.kind {
                        OperationKind::Store { pointer, value, .. } if pointer == tag.pointer => {
                            tag_stores.push(b.id.0);
                            let definitions = b.operations[..index]
                                .iter()
                                .filter(|op| op.results.iter().any(|result| result.id == value))
                                .collect::<Vec<_>>();
                            let [definition] = definitions.as_slice() else {
                                panic!("tag store must retain one preceding original constant");
                            };
                            assert!(matches!(
                                definition.kind,
                                OperationKind::Constant(Constant::U64(0))
                            ));
                            assert_eq!(definition.results[0].ty, Type::Scalar(ScalarType::U64));
                        }
                        OperationKind::Load { pointer, .. } if pointer == tag.pointer => {
                            panic!("scalar alias must not load the inactive residual tag");
                        }
                        _ => {}
                    }
                }
            }
            assert_eq!(tag_stores, [2]);
            assert!(lower.locals[2].is_none());
            assert!(lower.locals[3].is_none());
        }
    }
}

#[test]
fn ordinary_enum_alias_residual_checked_owner_reaches_canonical_kir_unchanged_source() {
    let owner = mixed_owner(&residual_fixture(true, 0));
    let original = *owner.source_semantic().semantic_sha256().as_bytes();
    let (module, correspondence) =
        lower_module(&owner, ProductionSemanticKirLimitsV1::default(), None, &[]).unwrap();
    assert_eq!(correspondence.semantic_sha256(), &original);
    assert!(
        !ProductionCanonicalKernelIrV1::from_module(module)
            .unwrap()
            .canonical_bytes()
            .is_empty()
    );
    owner.verify_replay().unwrap();
}

#[test]
fn ordinary_enum_alias_residual_missing_inverted_or_bypassed_guard_rejects() {
    for mutation in 0..3 {
        let mut f = residual_fixture(true, 0);
        match mutation {
            0 => f.replace_block(4, f.function.blocks()[4].statements().to_vec(), goto(6)),
            1 => f.replace_block(
                4,
                f.function.blocks()[4].statements().to_vec(),
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
            _ => f.replace_block(
                3,
                f.function.blocks()[3].statements().to_vec(),
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: copy(1, 3),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 6),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 4),
                    )
                    .unwrap(),
                },
            ),
        }
        let owner = mixed_owner(&f);
        let error = mixed_emit(&owner).err().expect("no missing-guard payload");
        assert!(
            matches!(
                error,
                ProductionSemanticKirErrorV1::EnumPayloadUnavailable {
                    block: 6,
                    local: 3,
                    variant: 0,
                    field: 0,
                    available_fields: 0,
                    ..
                }
            ),
            "mutation {mutation}: {error:?}"
        );
    }
}

#[test]
fn ordinary_enum_alias_residual_erased_variant_is_not_restored_by_cached_scalar_proof() {
    let mut fixture = residual_fixture(true, 0);
    // Both arms reach one lifetime end. The residual arm consumes the original
    // enum into a fresh temporary, retaining a live source use without adding
    // a second discriminator that would invalidate the original guard proof.
    let f = &fixture.function;
    let mut locals = f.locals().to_vec();
    let sink = u32::try_from(locals.len()).unwrap();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([94; 32]),
        ty(2),
        SemanticLocalRoleV1::Temporary,
        source(),
    ));
    let mut blocks = f.blocks().to_vec();
    assert_eq!(blocks.len(), 9);
    blocks[6] = block(6, blocks[6].statements().to_vec(), goto(9));
    blocks[5] = block(
        5,
        vec![assign(
            sink,
            2,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(3, 2))),
        )],
        goto(9),
    );
    blocks.push(block(
        9,
        vec![lifetime_marker(true, 3)],
        SemanticTerminatorKindV1::Return,
    ));
    fixture.function = SemanticFunctionDeclV1::new(
        f.identity(),
        f.role(),
        f.item_definition_identity(),
        f.monomorphization_identity(),
        f.generic_type_arguments_identity(),
        f.const_generic_arguments_identity(),
        f.source(),
        f.abi().clone(),
        locals,
        f.entry(),
        blocks,
    )
    .unwrap();
    assert_eq!(
        fixture.function.blocks().iter().flat_map(|b| b.statements()).filter(|s| {
            matches!(s.kind(), SemanticStatementKindV1::Assign(a)
                if matches!(a.value().kind(), SemanticRvalueKindV1::Discriminant(p) if p.local() == local(3)))
        }).count(),
        1,
        "keep the original unique discriminator and its switch"
    );
    assert_eq!(
        fixture
            .function
            .blocks()
            .iter()
            .flat_map(|b| b.statements())
            .filter(|s| {
                matches!(s.kind(), SemanticStatementKindV1::StorageDead(l) if *l == local(3))
            })
            .count(),
        1
    );
    let owner = mixed_owner(&fixture);
    let (_, mut lower) = mixed_emit(&owner).unwrap();
    assert!(
        lower
            .enum_payload_aliases
            .as_ref()
            .unwrap()
            .contains_key(&3)
    );
    // Re-enter the original other branch through the same retained SSA entry.
    let mut target = BasicBlock::new(BlockId(5));
    lower
        .begin_block(SemanticBlockIdV1::from_index(5), &mut target)
        .unwrap();
    assert!(lower.enum_variant_is_available_v1(local(3), 1, SemanticBlockIdV1::from_index(5)));
    assert!(
        matches!(&lower.locals[3], Some(SemanticValueBindingV1::Enum {
        semantic_type, payloads, ..
    }) if *semantic_type == ty(2) && payloads.is_empty())
    );
    let proof = lower.enum_payload_aliases.as_ref().unwrap()[&3];
    assert!(
        proof
            .allows_use(
                lower.function,
                &lower.control_flow_ssa,
                SemanticBlockIdV1::from_index(5),
                Some(0),
                3,
                &mut lower.enum_analysis_budget,
            )
            .unwrap(),
        "the nonprimitive gate, not a stale or absent SSA use, must deny restoration"
    );
    let before = format!("{:?}", lower.locals[3]);
    assert_eq!(
        lower
            .untransported_scalar_enum_storage_owner(SemanticBlockIdV1::from_index(5), Some(0), 3,)
            .unwrap(),
        None
    );
    assert_eq!(
        format!("{:?}", lower.locals[3]),
        before,
        "eligibility must not construct an erased residual"
    );
}

#[test]
fn ordinary_enum_alias_residual_shape_and_compiler_binding_substitutions_reject() {
    let base = residual_fixture(true, 0);
    let transport = base.lowering().control_flow_ssa;
    for mutation in 0..8 {
        let mut f = residual_fixture(true, 0);
        let original = f.types[5].clone();
        let mut shape = original.shape().clone();
        let mut layout = original.layout().clone();
        match mutation {
            0 => layout = SemanticTypeLayoutV1::new(Some(1), 1).unwrap(),
            1 => {
                shape =
                    SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap())
            }
            2 => shape = SemanticTypeShapeV1::Unit,
            3 => {
                layout = SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(0),
                    1,
                    SemanticBackendReprV1::memory(true),
                    true,
                )
                .unwrap()
            }
            4..=6 => {
                let SemanticTypeShapeV1::Enum { variants, .. } = &mut shape else {
                    unreachable!()
                };
                variants[0] = match mutation {
                    4 => SemanticEnumVariantV1::new(
                        0,
                        SemanticAggregateTypeV1::new(vec![ty(0)]).unwrap(),
                    ),
                    5 => SemanticEnumVariantV1::new_with_inhabitedness(
                        0,
                        SemanticAggregateTypeV1::new(vec![]).unwrap(),
                        true,
                    ),
                    _ => variants[0].clone(),
                };
                if mutation == 6 {
                    variants[1] = SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![]).unwrap(),
                    );
                }
            }
            _ => {
                let mut bindings = transport.clone();
                bindings
                    .compiler_issued_bindings
                    .insert(ty(5), SemanticPromotedBindingV1::KernelContext);
                let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
                assert!(
                    ordinary_enum_alias_v1::plan(&f.types, &f.function, &bindings, &mut budget)
                        .unwrap()
                        .is_empty()
                );
                continue;
            }
        }
        f.types[5] = SemanticTypeDeclV1::new(
            original.identity(),
            original.layout_identity(),
            layout,
            shape,
        );
        let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        assert!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &transport, &mut budget)
                .unwrap()
                .is_empty(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_enum_alias_residual_death_writes_and_complete_edges_keep_old_rules() {
    let base = residual_fixture(true, 0);
    let transport = base.lowering().control_flow_ssa;
    for mutation in 0..7 {
        let mut f = residual_fixture(true, 0);
        let mut capture = f.function.blocks()[3].statements().to_vec();
        let mut term = goto(4);
        match mutation {
            0 => capture.insert(0, lifetime_marker(true, 2)),
            1 => {
                capture.pop();
                f.replace_block(
                    5,
                    vec![lifetime_marker(true, 2)],
                    SemanticTerminatorKindV1::Return,
                );
            }
            2 => capture.push(SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Deinitialize(place(2, 2)),
            )),
            3 => capture.push(SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::SetDiscriminant {
                    place: place(2, 2),
                    variant_index: 1,
                },
            )),
            4 => capture.push(f.function.blocks()[1].statements()[1].clone()),
            5 => term = goto(1),
            _ => {
                term = SemanticTerminatorKindV1::Assert {
                    condition: copy(1, 3),
                    expected: true,
                    message: SemanticAssertMessageV1::ResumedAfterPanic,
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                    unwind: SemanticUnwindActionV1::Cleanup(edge(
                        SemanticEdgeRoleV1::AssertUnwind,
                        1,
                    )),
                }
            }
        }
        f.replace_block(3, capture, term);
        let mut budget = SemanticEnumAnalysisBudgetV1::new(100_000, 100_000);
        assert!(
            ordinary_enum_alias_v1::plan(&f.types, &f.function, &transport, &mut budget)
                .unwrap()
                .is_empty(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_enum_alias_residual_exact_existing_work_and_storage_owner() {
    let fixture = residual_fixture(true, 0);
    let transport = fixture.lowering().control_flow_ssa;
    let run = |budget: &mut SemanticEnumAnalysisBudgetV1| {
        ordinary_enum_alias_v1::plan(&fixture.types, &fixture.function, &transport, budget)
    };
    let mut measured = SemanticEnumAnalysisBudgetV1::new(1_000_000, 1_000_000);
    measured.charge_work(17).unwrap();
    measured.charge_storage(19).unwrap();
    assert_eq!(run(&mut measured).unwrap().get(&3), Some(&2));
    let mut exact = SemanticEnumAnalysisBudgetV1::new(measured.work, measured.storage);
    exact.charge_work(17).unwrap();
    exact.charge_storage(19).unwrap();
    assert_eq!(run(&mut exact).unwrap().get(&3), Some(&2));
    for short_work in [true, false] {
        let mut short = SemanticEnumAnalysisBudgetV1::new(
            measured.work - usize::from(short_work),
            measured.storage - usize::from(!short_work),
        );
        short.charge_work(17).unwrap();
        short.charge_storage(19).unwrap();
        let error = run(&mut short).unwrap_err();
        let expected = if short_work {
            ProductionSemanticKirResourceV1::AnalysisWork
        } else {
            ProductionSemanticKirResourceV1::AnalysisStorage
        };
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::ResourceLimit {
            resource, actual, limit,
        } if resource == expected && actual == limit + 1),
            "{error:?}"
        );
        assert!(short.work >= 17 && short.storage >= 19);
    }
}

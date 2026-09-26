use super::*;

#[test]
fn hostile_refusal_prefixes_are_charged_before_each_attempted_step() {
    for (fixture, path, work) in [
        (Fixture::direct(), vec![], 3 + 1),
        (
            Fixture::direct(),
            vec![SemanticMovePathElementV1::Field(99)],
            3 + 1 + 1 + 1,
        ),
        (Fixture::niche(false), payload(1, 0), 3 + 3 + 2),
    ] {
        let state = state(&[path]);
        let original = state.clone();
        let mut exact = budget(work);
        moved(validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut exact,
        ));
        assert_eq!(exact.work_units, work);
        let mut short = budget(work - 1);
        assert!(matches!(
            validate_partial_move_discriminant_read_v1(
                &fixture.function,
                Some(&fixture.types),
                &fixture.query(),
                location(),
                &state,
                &mut short
            ),
            Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                ..
            })
        ));
        assert_eq!(short.work_units, work);
        assert_eq!((short.base_storage_words, short.state_entries), (19, 7));
        assert_eq!(state, original);
    }
}

#[test]
fn exact_and_every_short_work_boundary_preserve_move_state_and_storage() {
    let fixture = Fixture::direct();
    let state = state(&[payload(0, 0)]);
    let original = state.clone();
    // Dispatch + query root + tag + (visit + move root + downcast + field + compare).
    const WORK: usize = 3 + 3 + 2;
    for available in 0..=WORK {
        let mut budget = budget(available);
        let base = (
            budget.base_storage_words,
            budget.base_work_units,
            budget.state_entries,
        );
        let result = validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut budget,
        );
        if available == WORK {
            result.unwrap();
            assert_eq!(budget.work_units, WORK);
        } else {
            assert!(
                matches!(result, Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                function, resource: SsaPlannerResourceV1::WorkUnits, required, limit
            }) if function == location().function && required == limit + 1 && limit == budget.limits.max_work_units())
            );
            assert_eq!(budget.work_units, available + 1);
        }
        assert_eq!(
            base,
            (
                budget.base_storage_words,
                budget.base_work_units,
                budget.state_entries
            )
        );
        assert_eq!(state, original);
    }
    let mut twice = budget(2 * WORK);
    for _ in 0..2 {
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut twice,
        )
        .unwrap();
    }
    assert_eq!(twice.work_units, 2 * WORK);
    assert_eq!(state, original);
}

#[test]
fn every_retained_move_is_metered_and_the_budget_overflow_is_checked() {
    let fixture = Fixture::direct();
    let state = state(&[payload(0, 0), payload(1, 1)]);
    const WORK: usize = 3 + 2 * (3 + 2);
    let mut exact = budget(WORK);
    validate_partial_move_discriminant_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &fixture.query(),
        location(),
        &state,
        &mut exact,
    )
    .unwrap();
    assert_eq!(exact.work_units, WORK);
    let mut overflow = budget(100);
    overflow.work_units = usize::MAX;
    assert!(matches!(
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut overflow
        ),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    ));
    assert_eq!(overflow.state_entries, 7);
    assert_eq!(overflow.work_units, usize::MAX);
    let mut overflow = budget(100);
    overflow.base_work_units = usize::MAX;
    assert!(matches!(
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &fixture.query(),
            location(),
            &state,
            &mut overflow
        ),
        Err(ProductionSemanticSsaErrorV1::ResourceOverflow)
    ));
    assert_eq!(overflow.work_units, 1);
    assert_eq!(overflow.state_entries, 7);
}

#[test]
fn large_fixed_arrays_do_not_expand_and_opposite_end_aliases_remain_exact() {
    for length in [2, 1_000_000_000] {
        let mut types = base_types();
        let enumeration = add_direct(&mut types, vec![WORD, WORD], vec![8, 16], 24);
        let root = add_array(&mut types, enumeration, length);
        let fixture = Fixture::new(types, root);
        let query = place(
            1,
            enumeration,
            vec![projection(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: length,
                    from_end: false,
                },
                enumeration,
            )],
        );
        let disjoint = state(&[vec![SemanticMovePathElementV1::ConstantIndex {
            offset: 1,
            from_end: true,
        }]]);
        let mut exact = budget(3 + 1 + 3 + 1);
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &query,
            location(),
            &disjoint,
            &mut exact,
        )
        .unwrap();
        assert_eq!(exact.work_units, 8);
        assert_eq!(exact.state_entries, 7);
        let alias = state(&[vec![SemanticMovePathElementV1::ConstantIndex {
            offset: length,
            from_end: true,
        }]]);
        moved(validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &query,
            location(),
            &alias,
            &mut budget(8),
        ));
        let invalid = state(&[vec![SemanticMovePathElementV1::ConstantIndex {
            offset: 0,
            from_end: true,
        }]]);
        moved(validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &query,
            location(),
            &invalid,
            &mut budget(8),
        ));
        for (offset, minimum_length) in [(length, length + 1), (0, length + 1)] {
            let invalid = place(
                1,
                enumeration,
                vec![projection(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset,
                        minimum_length,
                        from_end: false,
                    },
                    enumeration,
                )],
            );
            assert!(
                partial_move_query_geometry_v1(
                    &fixture.function,
                    &fixture.types,
                    &invalid,
                    &mut budget(2)
                )
                .unwrap()
                .is_none()
            );
        }
    }
}

#[test]
fn deep_borrowed_paths_use_linear_explicit_work_and_no_new_move_entries() {
    const DEPTH: usize = 32;
    let mut types = base_types();
    let enumeration = add_direct(&mut types, vec![WORD, WORD], vec![8, 16], 24);
    let mut root = enumeration;
    let mut children = Vec::new();
    for _ in 0..DEPTH {
        children.push(root);
        root = add_record(&mut types, vec![root], vec![0], 24);
    }
    let fixture = Fixture::new(types, root);
    let query = place(
        1,
        enumeration,
        children
            .into_iter()
            .rev()
            .map(|ty| projection(SemanticProjectionKindV1::Field(0), ty))
            .collect(),
    );
    let mut path = vec![SemanticMovePathElementV1::Field(0); DEPTH];
    path.extend(payload(0, 0));
    let state = state(&[path]);
    const WORK: usize = 3 + DEPTH + 3 + DEPTH + 2;
    let mut exact = budget(WORK);
    validate_partial_move_discriminant_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &query,
        location(),
        &state,
        &mut exact,
    )
    .unwrap();
    assert_eq!(exact.work_units, WORK);
    assert_eq!((exact.base_storage_words, exact.state_entries), (19, 7));
    let mut short = budget(WORK - 1);
    assert!(matches!(
        validate_partial_move_discriminant_read_v1(
            &fixture.function,
            Some(&fixture.types),
            &query,
            location(),
            &state,
            &mut short
        ),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits,
            ..
        })
    ));
    assert_eq!(short.work_units, WORK);
}

fn planner_work_limit(work: usize) -> SsaPlannerLimitsV1 {
    let old = SsaPlannerLimitsV1::default();
    SsaPlannerLimitsV1::try_new(
        old.max_variables(),
        old.max_blocks(),
        old.max_edges(),
        old.max_events(),
        old.max_edge_definitions(),
        old.max_output_items(),
        old.max_storage_words(),
        work,
    )
    .unwrap()
}

fn module_work_limit(work: usize) -> ProductionSemanticSsaModuleLimitsV1 {
    let old = ProductionSemanticSsaModuleLimitsV1::default();
    ProductionSemanticSsaModuleLimitsV1::try_new(
        old.max_variables(),
        old.max_blocks(),
        old.max_edges(),
        old.max_events(),
        old.max_edge_definitions(),
        old.max_output_items(),
        old.max_storage_words(),
        work,
    )
    .unwrap()
}

#[test]
fn original_tag_work_reaches_function_and_module_limits_with_independent_increment() {
    let fixture = Fixture::direct();
    let blocks = || {
        vec![block(
            100,
            vec![
                construct(&fixture, 0),
                move_statement(&fixture, 0, 0),
                tag_statement(&fixture),
            ],
            SemanticTerminatorKindV1::Return,
        )]
    };
    let unrestricted = owner(&fixture, blocks(), ProductionSemanticSsaLimitsV1::default()).unwrap();
    let plan = unrestricted
        .plan_for_function(SemanticFunctionIdV1::from_index(0))
        .unwrap();
    // One visited block, one depth-two move (no old read paths), one exact tag
    // query. The return unit has no moved paths. This is not a measured oracle.
    const PARTIAL_WORK: usize = 1 + 3 + 3 + 2;
    assert_eq!(plan.partial_move_certificate().projected_moves(), 1);
    assert_eq!(plan.partial_move_certificate().state_entries(), 1);
    assert_eq!(plan.partial_move_certificate().work_units(), PARTIAL_WORK);
    let base = plan.resources().work_units() + plan.auxiliary_resources.work_units;
    let function_exact = base + PARTIAL_WORK;
    // This argument-free root has one reference-summary roster visit and one
    // fixed-point visit. Those module-only charges do not consume its planner.
    assert_eq!(fixture.function.role(), SemanticFunctionRoleV1::KernelRoot);
    assert!(fixture.function.abi().source_input_types().is_empty());
    const MODULE_REFERENCE_WORK: usize = 1 + 1;
    let module_exact = function_exact + MODULE_REFERENCE_WORK;
    assert_eq!(unrestricted.summary().work_units(), module_exact);
    owner(
        &fixture,
        blocks(),
        ProductionSemanticSsaLimitsV1::new(planner_work_limit(function_exact)),
    )
    .unwrap()
    .verify_replay()
    .unwrap();
    assert!(
        matches!(owner(&fixture, blocks(), ProductionSemanticSsaLimitsV1::new(planner_work_limit(function_exact - 1))),
        Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits, required, limit, ..
        }) if required == function_exact && limit == function_exact - 1)
    );
    owner(
        &fixture,
        blocks(),
        ProductionSemanticSsaLimitsV1::with_module_limits(
            SsaPlannerLimitsV1::default(),
            module_work_limit(module_exact),
        ),
    )
    .unwrap()
    .verify_replay()
    .unwrap();
    assert!(
        matches!(owner(&fixture, blocks(), ProductionSemanticSsaLimitsV1::with_module_limits(SsaPlannerLimitsV1::default(), module_work_limit(module_exact - 1))),
        Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
            resource: SsaPlannerResourceV1::WorkUnits, required, limit
        }) if required == module_exact && limit == module_exact - 1)
    );
}

#[test]
fn missing_or_malformed_geometry_never_supplies_disjointness() {
    let fixture = Fixture::direct();
    let state = state(&[payload(0, 0)]);
    moved(validate_partial_move_discriminant_read_v1(
        &fixture.function,
        None,
        &fixture.query(),
        location(),
        &state,
        &mut budget(100),
    ));
    let mut no_layout = fixture.types.clone();
    no_layout[fixture.root.index() as usize] = declaration(
        fixture.root.index() as usize,
        SemanticTypeLayoutV1::new(Some(24), 8).unwrap(),
        no_layout[fixture.root.index() as usize].shape().clone(),
    );
    moved(validate_partial_move_discriminant_read_v1(
        &fixture.function,
        Some(&no_layout),
        &fixture.query(),
        location(),
        &state,
        &mut budget(100),
    ));
    let wrong = place(1, WORD, vec![]);
    moved(validate_partial_move_discriminant_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &wrong,
        location(),
        &state,
        &mut budget(100),
    ));
    for path in [
        vec![
            SemanticMovePathElementV1::Downcast(99),
            SemanticMovePathElementV1::Field(0),
        ],
        payload(0, 99),
        vec![SemanticMovePathElementV1::Downcast(0)],
        vec![SemanticMovePathElementV1::ConstantIndex {
            offset: 0,
            from_end: false,
        }],
    ] {
        moved(fixture.check(&fixture.query(), &[path]));
    }
    let mut types = base_types();
    let inner = add_direct(&mut types, vec![WORD, WORD], vec![8, 16], 24);
    let root = add_record(&mut types, vec![inner], vec![0], 24);
    let nested = Fixture::new(types, root);
    let mut malformed = nested.types.clone();
    malformed[root.index() as usize] = declaration(
        root.index() as usize,
        SemanticTypeLayoutV1::aggregate(
            Some(24),
            8,
            SemanticAggregateLayoutV1::new(vec![16], vec![]).unwrap(),
        )
        .unwrap(),
        malformed[root.index() as usize].shape().clone(),
    );
    let query = place(
        1,
        inner,
        vec![projection(SemanticProjectionKindV1::Field(0), inner)],
    );
    assert!(
        partial_move_query_geometry_v1(&nested.function, &malformed, &query, &mut budget(100))
            .unwrap()
            .is_none()
    );
    let overflowing = PartialMoveGeometryV1 {
        ty: root,
        start: u64::MAX - 1,
        size: 24,
        root_size: u64::MAX,
        variant: None,
    };
    assert!(overflowing.child(&nested.types, inner, 0).is_none());
    let mut union = nested.types.clone();
    union[root.index() as usize] = declaration(
        root.index() as usize,
        union[root.index() as usize].layout().clone(),
        SemanticTypeShapeV1::Union(SemanticAggregateTypeV1::new(vec![inner]).unwrap()),
    );
    assert!(
        partial_move_query_geometry_v1(&nested.function, &union, &query, &mut budget(100))
            .unwrap()
            .is_none()
    );
    for kind in [
        SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
        SemanticProjectionKindV1::Dereference,
        SemanticProjectionKindV1::OpaqueCast,
        SemanticProjectionKindV1::Subtype,
        SemanticProjectionKindV1::Subslice {
            from: 0,
            to: 0,
            from_end: false,
        },
    ] {
        let query = place(1, inner, vec![projection(kind, inner)]);
        assert!(
            partial_move_query_geometry_v1(
                &nested.function,
                &nested.types,
                &query,
                &mut budget(100)
            )
            .unwrap()
            .is_none()
        );
        moved(nested.check(&query, &[vec![SemanticMovePathElementV1::Field(0)]]));
    }
}

#[test]
fn original_niche_offset_and_tag_geometry_are_not_guessed() {
    let mut fixture = Fixture::niche(false);
    let original = fixture.types[fixture.root.index() as usize].clone();
    let SemanticRustcVariantsV1::Multiple(layout) = original.layout().variants() else {
        panic!();
    };
    let SemanticEnumEncodingV1::Niche(niche) = layout.encoding() else {
        panic!();
    };
    for offset in [8, u64::MAX - 7] {
        let changed = SemanticNicheEnumEncodingV1::new(
            niche.tag_field(),
            SemanticNicheSourceV1::new(niche.source().path().to_vec(), offset).unwrap(),
            niche.source_niche(),
            niche.tag(),
            niche.untagged_variant(),
            niche.niche_variant_range().0,
            niche.niche_variant_range().1,
            niche.niche_start(),
        )
        .unwrap();
        let changed_layout = SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                layout.variants().to_vec(),
                SemanticEnumEncodingV1::Niche(changed),
            )
            .unwrap(),
        );
        if let Ok(changed_layout) = changed_layout {
            fixture.types[fixture.root.index() as usize] = declaration(
                fixture.root.index() as usize,
                changed_layout,
                original.shape().clone(),
            );
            moved(fixture.check(&fixture.query(), &[payload(1, 1)]));
        } else {
            assert!(
                offset > 16,
                "ordinary in-range malformed encoding reaches the geometry refusal"
            );
        }
    }
    let mut direct = Fixture::direct();
    let original = direct.types[direct.root.index() as usize].clone();
    let SemanticRustcVariantsV1::Multiple(layout) = original.layout().variants() else {
        panic!();
    };
    let SemanticEnumEncodingV1::Direct(tag) = layout.encoding() else {
        panic!();
    };
    direct.types[direct.root.index() as usize] = declaration(
        direct.root.index() as usize,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            24,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                layout.variants().to_vec(),
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 1, tag.tag())),
            )
            .unwrap(),
        )
        .unwrap(),
        original.shape().clone(),
    );
    moved(direct.check(&direct.query(), &[payload(0, 0)]));
}

#[test]
fn exact_nested_niche_and_single_wrapper_geometry_is_not_a_constant_tag_shortcut() {
    let mut types = base_types();
    let inner = add_niche(&mut types, false);
    let array = add_array(&mut types, inner, 2);
    let root = id(types.len());
    types.push(declaration(
        types.len(),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            32,
            8,
            SemanticFieldsShapeV1::arbitrary(vec![0], vec![0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            8,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            WORD,
            vec![SemanticEnumVariantV1::new(
                0,
                SemanticAggregateTypeV1::new(vec![array]).unwrap(),
            )],
        )
        .unwrap(),
    ));
    let fixture = Fixture::new(types, root);
    let query = place(
        1,
        inner,
        vec![
            projection(SemanticProjectionKindV1::Downcast(0), root),
            projection(SemanticProjectionKindV1::Field(0), array),
            projection(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 1,
                    minimum_length: 2,
                    from_end: false,
                },
                inner,
            ),
        ],
    );
    let mut disjoint = payload(0, 0);
    disjoint.push(SemanticMovePathElementV1::ConstantIndex {
        offset: 1,
        from_end: true,
    });
    disjoint.extend(payload(1, 1));
    fixture.check(&query, &[disjoint.clone()]).unwrap();
    moved(fixture.check(&fixture.query(), &[disjoint]));
    let mut overlapping = payload(0, 0);
    overlapping.push(SemanticMovePathElementV1::ConstantIndex {
        offset: 1,
        from_end: true,
    });
    overlapping.extend(payload(1, 0));
    moved(fixture.check(&query, &[overlapping]));
    let mut wrong_result = query.projections().to_vec();
    wrong_result[1] = projection(SemanticProjectionKindV1::Field(0), WORD);
    let wrong_result = place(1, inner, wrong_result);
    assert!(
        partial_move_query_geometry_v1(
            &fixture.function,
            &fixture.types,
            &wrong_result,
            &mut budget(100)
        )
        .unwrap()
        .is_none()
    );
    for path in [
        vec![SemanticMovePathElementV1::Downcast(0)],
        vec![SemanticMovePathElementV1::Field(99)],
    ] {
        moved(fixture.check(&query, &[path]));
    }
}

#[test]
fn admitted_zero_byte_field_inside_niche_tag_does_not_read_or_restore_payload_bytes() {
    let fixture = Fixture::niche(false);
    let mut types = fixture.types.clone();
    let original = &fixture.types[fixture.root.index() as usize];
    let SemanticRustcVariantsV1::Multiple(layout) = original.layout().variants() else {
        panic!();
    };
    let SemanticEnumEncodingV1::Niche(niche) = layout.encoding() else {
        panic!();
    };
    let SemanticTypeShapeV1::Enum { variants, .. } = original.shape() else {
        panic!();
    };
    let payload_type = variants[1].fields().fields()[0];
    let payload_layout = SemanticEnumVariantLayoutV1::from_rustc(
        1,
        16,
        8,
        SemanticFieldsShapeV1::arbitrary(vec![0, 4, 8], vec![0, 1, 2]).unwrap(),
        SemanticBackendReprV1::memory(true),
        Some(niche.source_niche()),
        false,
        None,
        8,
        0,
        SemanticAggregateLayoutV1::new(vec![0, 4, 8], vec![]).unwrap(),
    )
    .unwrap();
    types[fixture.root.index() as usize] = declaration(
        fixture.root.index() as usize,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            16,
            8,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                vec![layout.variants()[0].clone(), payload_layout],
                layout.encoding().clone(),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            WORD,
            vec![
                variants[0].clone(),
                SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![payload_type, UNIT, WORD]).unwrap(),
                ),
            ],
        )
        .unwrap(),
    );
    let fixture = Fixture::new(types, fixture.root);
    let state = state(&[payload(1, 1)]);
    let mut exact = budget(8);
    validate_partial_move_discriminant_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &fixture.query(),
        location(),
        &state,
        &mut exact,
    )
    .unwrap();
    assert_eq!(exact.work_units, 8);
    moved(validate_partial_move_place_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &field_place(&fixture, 1, 1, UNIT),
        location(),
        &state,
        &mut budget(100),
    ));
    moved(validate_partial_move_place_read_v1(
        &fixture.function,
        Some(&fixture.types),
        &fixture.query(),
        location(),
        &state,
        &mut budget(100),
    ));
}

use super::*;

fn assertion_owner(expected: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = comparison_loop();
    let source = base.source_semantic();
    let old = &source.functions()[0];
    let mut blocks = old.blocks().to_vec();
    let SemanticTerminatorKindV1::SwitchInt { discriminant, .. } = blocks[1].terminator().kind()
    else {
        unreachable!()
    };
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[1].source(),
            SemanticTerminatorKindV1::Assert {
                condition: discriminant.clone(),
                expected,
                message: SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Subtract,
                    left: SemanticOperandV1::Copy(place(2, U32)),
                    right: literal(1),
                },
                target: SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::AssertSuccess,
                    SemanticBlockIdV1::from_index(2),
                ),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
    )
    .unwrap();
    let mut functions = source.functions().to_vec();
    functions[0] = rebuild_root(old, old.locals().to_vec(), blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        source.callables().to_vec(),
        source.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn positive() -> ProductionSemanticSsaOwnerV1 {
    assertion_owner(true)
}
fn negative() -> ProductionSemanticSsaOwnerV1 {
    assertion_owner(false)
}

#[test]
fn source_scalar_assert_boundaries_check_original_and_optimized_both_polarities() {
    for factory in [positive as fn() -> _, negative] {
        let reached = std::cell::Cell::new(false);
        with_policy11(factory, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    let input = leaves.original.leaves;
                    assert!(!input.boundaries.rows.is_empty());
                    source_scalar_normalization_scratch_v18(
                        original.source.cleanup,
                        budget,
                        0,
                        |budget| {
                            let function = original.source.root_row(0)?.function_ordinal;
                            let assertions =
                                source_boundary_assertions_v40(original, function, budget)?;
                            assert_eq!(assertions.len(), 1);
                            assert!(matches!(
                                assertions[0].binding.outcome(),
                                SemanticKirAssertConditionOutcomeV1::Emitted { .. }
                            ));
                            Ok(())
                        },
                    )?;
                    input.check_boundary_equations_v31(budget)?;
                    input.check_boundary_actual_v31(Some(leaves), budget)?;
                    reached.set(true);
                    Ok(())
                },
            )
        })
        .unwrap();
        assert!(reached.get());
    }
}

#[test]
fn source_scalar_assert_boundaries_reject_missing_attachment_and_changed_polarity() {
    for absent in [false, true] {
        let result = with_policy11(positive, |original, optimized, budget| {
            original.with_optimized_scalar_leaf_namespace_v18(
                optimized,
                0,
                &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                budget,
                |leaves, budget| {
                    with_check(leaves, budget, |check, budget| {
                        let rows = source_boundary_assertions_v40(
                            original,
                            original.source.root_row(0)?.function_ordinal,
                            budget,
                        )?;
                        assert_eq!(rows.len(), 1);
                        let row = rows[0];
                        let input = original
                            .inventory
                            .blocks()
                            .iter()
                            .find(|block| block.coordinate == row.binding.block())
                            .unwrap();
                        let actual = check.block(input.coordinate, budget)?;
                        let semantic = original.source.source_semantic(budget)?;
                        let source = semantic.functions()
                            [row.site.semantic_function.index() as usize]
                            .blocks()[row.site.semantic_block.index() as usize]
                            .terminator()
                            .kind();
                        let mut altered = source.clone();
                        if let SemanticTerminatorKindV1::Assert { expected, .. } = &mut altered {
                            if !absent {
                                *expected = !*expected;
                            }
                        } else {
                            unreachable!();
                        }
                        check.assertion_v40(
                            if absent { &[] } else { &rows },
                            row.instance.index(),
                            row.site.semantic_function,
                            row.site.semantic_block,
                            &altered,
                            input,
                            actual,
                            budget,
                        )
                    })
                },
            )
        });
        assert_binding(
            result,
            if absent {
                "source assertion attachment is absent"
            } else {
                "source assertion coordinate or polarity differs"
            },
        );
    }
}

#[test]
fn source_scalar_assert_index_keeps_other_roots_empty_and_repeated_lookup_bounded() {
    with_policy11(positive, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                with_check(leaves, budget, |check, budget| {
                    let root = original.source.root_row(0)?;
                    let rows =
                        source_boundary_assertions_v40(original, root.function_ordinal, budget)?;
                    assert_eq!(rows.len(), 1);
                    let other = source_boundary_assertions_v40(
                        original,
                        original.source.root_row(1)?.function_ordinal,
                        budget,
                    )?;
                    assert!(other.is_empty());
                    let row = rows[0];
                    let key = (row.instance.index(), row.site);
                    let storage = budget.storage();
                    let work = budget.work();
                    for _ in 0..64 {
                        let found = assert_origin_find_v1(&rows, budget, |candidate, budget| {
                            budget.charge_work(2)?;
                            Ok((candidate.instance.index(), candidate.site).cmp(&key))
                        })
                        .map_err(|error| source_emission_error_v18(error.into()))?;
                        assert_eq!(found, Some(0));
                    }
                    assert_eq!(budget.storage(), storage);
                    // One-row binary lookup pays one comparison and its fixed loop
                    // charge. It does not rebuild or rescan the source archive.
                    assert_eq!(budget.work() - work, 64 * 3);
                    assert!(std::ptr::eq(check.leaves.relation.source, original.source));
                    Ok(())
                })
            },
        )
    })
    .unwrap();
}

fn cut(cut: Option<(bool, usize)>) -> (usize, usize, bool) {
    let observed = std::cell::Cell::new(None);
    let result = with_policy11(negative, |original, optimized, budget| {
        let floor = budget.storage();
        let result =
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                let remaining = match cut {
                    Some((false, amount)) => amount,
                    _ => MODULE_LIMIT / 2,
                };
                budget.reserve_storage(MODULE_LIMIT - budget.storage() - remaining)?;
                if let Some((true, amount)) = cut {
                    budget.charge_work(OPTIMIZED_SOURCE_WORK_LIMIT_V18 - budget.work() - amount)?;
                }
                let before = (budget.work(), budget.storage());
                let entered = std::cell::Cell::new(false);
                let result = original.with_optimized_scalar_leaf_namespace_v18(
                    optimized,
                    0,
                    &SourceScalarNamespaceV18::PrivateSourceWritesV22,
                    budget,
                    |_, _| {
                        entered.set(true);
                        Err::<(), _>(ProductionSourceOwnedViewErrorV18::Binding(
                            "assertion boundary test stop",
                        ))
                    },
                );
                observed.set(Some((
                    budget.work() - before.0,
                    budget.peak_storage() - before.1,
                    entered.get(),
                )));
                match (&result, entered.get(), cut) {
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "assertion boundary test stop",
                        )),
                        true,
                        _,
                    ) => {}
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work(
                            error,
                        ))),
                        false,
                        Some((true, _)),
                    ) => assert!(error.actual() > error.limit()),
                    (
                        Err(ProductionSourceOwnedViewErrorV18::Resource(
                            ArgumentResourceV1::Storage(error),
                        )),
                        false,
                        Some((false, _)),
                    ) => assert!(error.actual() > error.limit()),
                    _ => panic!("unexpected assertion resource result: {result:?}"),
                }
                result
            });
        assert_eq!(budget.storage(), floor);
        result
    });
    assert!(result.is_err());
    observed
        .get()
        .expect("the assertion namespace was attempted")
}

#[test]
fn source_scalar_assert_boundary_exact_and_one_short_resource_cuts_preserve_cleanup() {
    let (work, storage, reached) = cut(None);
    assert!(reached && work > 0 && storage > 0);
    for (kind, exact) in [(true, work), (false, storage)] {
        assert!(cut(Some((kind, exact))).2);
        assert!(!cut(Some((kind, exact - 1))).2);
    }
}

pub(super) fn independent_assert_headers() -> usize {
    type Frames<'a> = (
        Vec<&'a ReplayedInstanceAssertV1>,
        SourceOwnedResultV18<Vec<&'a ReplayedInstanceAssertV1>>,
        [&'a ReplayedInstanceAssertV1; 3],
        &'a [&'a ReplayedInstanceAssertV1],
        SemanticKirAssertConditionBindingV1,
        SemanticKirAssertConditionOutcomeV1,
        SemanticKirAssertSiteV1,
        (usize, SemanticKirAssertSiteV1),
        std::slice::Iter<'a, ReplayedInstanceAssertV1>,
        std::slice::Windows<'a, &'a ReplayedInstanceAssertV1>,
        &'a fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirEdgeRefV1<'a>,
        [SourceOwnedResultV18<usize>; 3],
        [SourceOwnedResultV18<()>; 2],
        [usize; 12],
        [&'a (); 16],
    );
    size_of::<Frames<'_>>() + std::mem::align_of::<Frames<'_>>()
}

#[test]
fn source_scalar_assert_boundary_headers_match_independent_frames() {
    assert_eq!(
        source_boundary_assert_headers_v40().unwrap(),
        independent_assert_headers()
    );
}

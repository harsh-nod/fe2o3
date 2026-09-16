// Included after the genuine canonical memory/control source fixtures.
// No test constructs a completed Store/control analysis directly.

fn with_formal_completed_roots_v1<T>(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    policy3: bool,
    inputs: &[ProductionRankedRootInputV1],
    body: impl for<'scope, 'source, 'output> FnOnce(
        &[fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<
            'scope,
            'source,
            'output,
        >],
        &mut Budget<'_>,
    ) -> Result<T, ProductionSourceOutputErrorV1>,
) -> Result<T, ProductionRankedProjectionErrorV1> {
    let mut result = None;
    if policy3 {
        with_actual_policy3_canonical_view_v1(source, profile, |checked, view, budget| {
            assert!(std::ptr::eq(view.output(), checked.owner()));
            let floor = budget.storage();
            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            result =
                Some(with_ranked_root_preparation_v1(
                    &projection,
                    inputs,
                    &references,
                    |effects, references| {
                        checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                            view,
                            budget,
                            |session| {
                                with_prepared_canonical_memory_session_v1(
                                    &projection,
                                    inputs,
                                    effects,
                                    references,
                                    session,
                                    |roots, budget| {
                                        assert!(roots.iter().all(|root| std::ptr::eq(
                                            root.output(),
                                            checked.owner()
                                        )));
                                        body(roots, budget)
                                    },
                                )
                            },
                        )
                    },
                ));
            assert_eq!(budget.storage(), floor);
        });
    } else {
        with_actual(source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            result = Some(with_projected_canonical_memory_analysis_v1(
                source,
                bound,
                checked,
                profile,
                inputs,
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |roots, budget| {
                    assert!(
                        roots
                            .iter()
                            .all(|root| std::ptr::eq(root.output(), checked.owner()))
                    );
                    body(roots, budget)
                },
            ));
            assert_eq!(budget.storage(), floor);
        });
    }
    result.expect("the real checked owner callback must run")
}

#[test]
fn scoped_formal_complete_borrows_same_actual_o_on_both_policies_and_targets() {
    let source = canonical_private_constant_store_source_v1();
    for policy3 in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_formal_completed_roots_v1(
                &source,
                profile,
                policy3,
                &[ranked_root_input_1d(A_NAME, 247, 64)],
                |roots, budget| {
                    let [root] = roots else {
                        panic!("one source root required")
                    };
                    let floor = budget.storage();
                    let before = budget.work();
                    let kernel = &root.output().module().kernels[0];
                    // Live-check/getter20, source+entry4, each row visit2 and
                    // comparison2 plus all four identifier bytes, finish1,
                    // final live-check19. Formal engine/callback are separate.
                    let expected = 44 + 4 + 2 * A_NAME.len() + 2 * kernel.entry.as_str().len();
                    let mut called = false;
                    root.with_complete_formal_memory_v1(budget, |formal, budget| {
                        called = true;
                        assert!(std::ptr::eq(formal.output(), root.output()));
                        assert!(std::ptr::eq(formal.kernel(), kernel));
                        assert_eq!(formal.selected_root(), root.selected_root());
                        assert_eq!(formal.selected_function(), root.selected_function());
                        assert_eq!(formal.obligations().kernel(), &kernel.id);
                        assert!(formal.obligations().inter_invocation_conflicts().is_empty());
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    })
                    .unwrap();
                    assert!(called);
                    assert_eq!(budget.work() - before, expected);
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}

#[test]
fn scoped_formal_complete_does_not_discharge_dynamic_global_indices() {
    use fe2o3_lower_mir_kernel::{
        ProductionFormalMemoryErrorV1 as Formal, ProductionScopedFormalMemoryErrorV1 as Error,
    };
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for policy3 in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_formal_completed_roots_v1(
                &source, profile, policy3, &[ranked_root_input_1d(A_NAME, 247, 1)],
                |roots, budget| {
                    let [root] = roots else { panic!("one source root required") };
                    let floor = budget.storage();
                    let mut called = false;
                    let error = root.with_complete_formal_memory_v1(budget, |_, _| {
                        called = true;
                        Ok(())
                    }).unwrap_err();
                    assert!(!called);
                    assert!(matches!(error, Error::Formal(Formal::Incomplete { ref reasons })
                        if reasons.iter().any(|reason| matches!(reason,
                            fe2o3_kernel_ir::FormalMemoryIncompleteReason::UnsupportedIndexExpression { .. }))));
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            ).unwrap();
        }
    }
}

#[test]
fn scoped_formal_complete_rejects_foreign_ledger_and_preserves_original_callback_error() {
    use fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1 as Error;
    let source = canonical_private_constant_store_source_v1();
    for policy3 in [false, true] {
        with_formal_completed_roots_v1(
            &source,
            Profile::Gfx942,
            policy3,
            &[ranked_root_input_1d(A_NAME, 247, 64)],
            |roots, budget| {
                let root = &roots[0];
                let floor = budget.storage();
                let mut foreign_work = Work::new(LIMIT);
                let mut foreign = Budget::new(&mut foreign_work, STORAGE_LIMIT);
                foreign.reserve_storage(floor).unwrap();
                let before = budget.work();
                let mut called = false;
                assert!(matches!(
                    root.with_complete_formal_memory_v1(&mut foreign, |_, _| {
                        called = true;
                        Ok(())
                    }),
                    Err(Error::SourceOutput(_))
                ));
                assert!(!called);
                assert_eq!(budget.work(), before);
                assert_eq!(budget.storage(), floor);
                assert_eq!(foreign.storage(), floor);
                let failure = root.with_complete_formal_memory_v1(budget, |_, _| {
                    Err::<(), _>(Error::SourceOutput(ProductionSourceOutputErrorV1::Invalid(
                        "formal callback sentinel",
                    )))
                });
                assert!(matches!(
                    failure,
                    Err(Error::SourceOutput(ProductionSourceOutputErrorV1::Invalid(
                        "formal callback sentinel"
                    )))
                ));
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn scoped_formal_complete_denies_binding_work_before_extraction_or_callback() {
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    use fe2o3_lower_mir_kernel::ProductionScopedFormalMemoryErrorV1 as Error;
    let source = canonical_private_constant_store_source_v1();
    // This consumes the existing live meter, never swaps in a fresh budget.
    // Exactly24 admits validation20 + source2 + entry2; row visit2 is denied.
    with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
        let floor = budget.storage();
        let mut called = false;
        let error = with_projected_canonical_memory_analysis_v1(
            &source,
            bound,
            checked,
            Profile::Gfx942,
            &[ranked_root_input_1d(A_NAME, 247, 64)],
            &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
            budget,
            |roots, budget| {
                budget
                    .charge_work(LIMIT - budget.work() - 24)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let error = roots[0]
                    .with_complete_formal_memory_v1(budget, |_, _| {
                        called = true;
                        Ok(())
                    })
                    .unwrap_err();
                assert_eq!(budget.work(), LIMIT);
                let Error::SourceOutput(
                    error @ ProductionSourceOutputErrorV1::Resource(Resource::Work(_)),
                ) = error
                else {
                    panic!("binding work denial required");
                };
                Err::<(), _>(error)
            },
        )
        .unwrap_err();
        assert!(!called);
        assert!(matches!(
            error,
            ProductionRankedProjectionErrorV1::CanonicalAssertions(
                canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Output(
                    ProductionSourceOutputErrorV1::Resource(Resource::Work(_))
                )
            )
        ));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), None);
    });
}

fn scoped_formal_helper_before_entry_source_v1() -> ProductionPreRankedKirOwnerV1 {
    let seed = canonical_private_constant_store_source_v1();
    let semantic = seed.semantic_ssa().source_semantic();
    let root = &semantic.functions()[0];
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(90)),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(91)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(92)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(93)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(94)),
        SemanticSourceProvenanceV1::unavailable(),
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(95)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(A_UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![])
        .unwrap(),
        vec![local(96, A_UNIT, SemanticLocalRoleV1::Return)],
        SemanticBlockIdV1::from_index(0),
        vec![block(97, vec![], SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let mut blocks = root.blocks().to_vec();
    blocks[0] = block(
        201,
        vec![],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(0),
                vec![],
                Some(SemanticCallDestinationV1::new(
                    whole(0, A_UNIT),
                    cfg_edge(SemanticEdgeRoleV1::CallReturn, 2),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    let root = ordinary_rebuild_v1(root, root.abi().clone(), root.locals().to_vec(), blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![helper, root],
        (0..2)
            .map(|index| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index)))
            .collect(),
        vec![SemanticFunctionIdV1::from_index(1)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    materialize_ranked_fixture_v1(ssa, &[ranked_root_input_1d(A_NAME, 247, 64)]).unwrap()
}

#[test]
fn scoped_formal_source_helper_precedes_entry_without_becoming_kernel_index() {
    let source = scoped_formal_helper_before_entry_source_v1();
    for policy3 in [false, true] {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_formal_completed_roots_v1(
                &source,
                profile,
                policy3,
                &[ranked_root_input_1d(A_NAME, 247, 64)],
                |roots, budget| {
                    let [root] = roots else {
                        panic!("one source root required")
                    };
                    let module = root.output().module();
                    assert_eq!(module.kernels.len(), 1);
                    assert_eq!(
                        module.functions[0].role,
                        fe2o3_kernel_ir::FunctionRole::InternalHelper
                    );
                    assert_eq!(
                        module.functions[1].role,
                        fe2o3_kernel_ir::FunctionRole::KernelEntry
                    );
                    assert_eq!(module.kernels[0].entry, module.functions[1].id);
                    assert!(
                        module.functions[1]
                            .body
                            .as_ref()
                            .unwrap()
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .any(|operation| matches!(&operation.kind,
                            fe2o3_kernel_ir::OperationKind::Call { callee, .. }
                            if callee == &module.functions[0].id))
                    );
                    assert_eq!(root.selected_root(), SemanticFunctionIdV1::from_index(1));
                    root.with_complete_formal_memory_v1(budget, |formal, _| {
                        assert!(std::ptr::eq(formal.output(), root.output()));
                        assert!(std::ptr::eq(formal.kernel(), &module.kernels[0]));
                        assert_eq!(formal.obligations().kernel(), &module.kernels[0].id);
                        Ok(())
                    })
                    .unwrap();
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}

#[test]
fn scoped_formal_callback_unwind_releases_completed_scope_not_checked_o() {
    let source = canonical_private_constant_store_source_v1();
    with_actual(&source, Profile::Gfx942, |bound, checked, budget| {
        let floor = budget.storage();
        let before = budget.work();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_projected_canonical_memory_analysis_v1(
                &source,
                bound,
                checked,
                Profile::Gfx942,
                &[ranked_root_input_1d(A_NAME, 247, 64)],
                &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default(),
                budget,
                |roots, budget| {
                    roots[0]
                        .with_complete_formal_memory_v1(budget, |formal, _| -> Result<(), _> {
                            assert!(std::ptr::eq(formal.output(), checked.owner()));
                            panic!("formal scoped unwind sentinel");
                        })
                        .unwrap();
                    Ok(())
                },
            )
            .unwrap();
        }));
        let payload = panic.unwrap_err();
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"formal scoped unwind sentinel")
        );
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > before);
        assert_eq!(budget.failed_storage(), None);
        assert!(!checked.owner().module().kernels.is_empty());
    });
}

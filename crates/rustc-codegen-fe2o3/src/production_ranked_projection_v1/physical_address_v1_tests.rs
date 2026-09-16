// Genuine source/N/B/O tests. Component-only arithmetic/accounting cases live
// in the lowerer child; no completed analysis is constructed by these tests.
fn with_physical_address_source_v1(
    source: &ProductionPreRankedKirOwnerV1,
    profile: Profile,
    policy3: bool,
    body: impl for<'scope, 'source, 'output> FnOnce(
        &fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output>,
        &mut Budget<'_>,
    ) -> Result<(), ProductionSourceOutputErrorV1>,
) {
    let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    if policy3 {
        with_actual_policy3_canonical_view_v1(source, profile, |checked, view, budget| {
            let projection = RankedProjectionSourceV1::from_legacy(source).unwrap();
            with_ranked_root_preparation_v1(
                &projection,
                &inputs,
                &references,
                |effects, references| {
                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                        view,
                        budget,
                        |session| {
                            with_prepared_canonical_memory_session_v1(
                                &projection,
                                &inputs,
                                effects,
                                references,
                                session,
                                |roots, budget| {
                                    assert_eq!(roots.len(), 1);
                                    assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                                    body(&roots[0], budget)
                                },
                            )
                        },
                    )
                },
            )
            .unwrap();
        });
    } else {
        with_actual(source, profile, |bound, checked, budget| {
            with_projected_canonical_memory_analysis_v1(
                source,
                bound,
                checked,
                profile,
                &inputs,
                &references,
                budget,
                |roots, budget| {
                    assert_eq!(roots.len(), 1);
                    assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                    body(&roots[0], budget)
                },
            )
            .unwrap();
        });
    }
}

fn physical_address_two_root_source_v1() -> ProductionPreRankedKirOwnerV1 {
    let seed = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    let source = seed.semantic_ssa().source_semantic();
    let root = &source.functions()[0];
    let second = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(253)),
        root.role(),
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(254)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(255)),
        root.generic_type_arguments_identity(),
        root.const_generic_arguments_identity(),
        root.source(),
        root.abi().clone(),
        root.locals().to_vec(),
        root.entry(),
        root.blocks().to_vec(),
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"address_second".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(248)),
        root.kernel_entry().unwrap().source_contract().clone(),
    ));
    let mut functions = source.functions().to_vec();
    let second_id = SemanticFunctionIdV1::from_index(functions.len() as u32);
    functions.push(second);
    assert_eq!(source.callables().len() + 1, functions.len());
    let callables = (0..functions.len())
        .map(|i| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(i as u32)))
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        source.allocations().to_vec(),
        source.statics().to_vec(),
        source.vtables().to_vec(),
        functions,
        callables,
        vec![SemanticFunctionIdV1::from_index(0), second_id],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    materialize_ranked_fixture_v1(
        ssa,
        &[
            ranked_root_input_1d(A_NAME, 247, 1),
            ranked_root_input_1d("address_second", 248, 1),
        ],
    )
    .unwrap()
}

#[test]
fn physical_address_two_roots_reuse_one_ledger_and_keep_failure_history() {
    let source = physical_address_two_root_source_v1();
    let inputs = [
        ranked_root_input_1d(A_NAME, 247, 1),
        ranked_root_input_1d("address_second", 248, 1),
    ];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            with_projected_canonical_memory_analysis_v1(
                &source,
                bound,
                checked,
                profile,
                &inputs,
                &references,
                budget,
                |roots, budget| {
                    assert_eq!(roots.len(), 2);
                    let floor = budget.storage();
                    let prefix = budget.work();
                    let mut first_operation = None;
                    roots[0].with_physical_address_relation_v1(budget, |relation, budget| {
                        first_operation = Some(relation.access(0, budget)?.unwrap().operation());
                        Ok(())
                    })?;
                    assert_eq!(budget.storage(), floor);
                    let after_first = budget.work();
                    assert!(after_first > prefix);
                    let refused =
                        roots[1].with_physical_address_relation_v1(budget, |_, budget| {
                            // A storage denial leaves work available for continuation.
                            budget
                                .reserve_storage(STORAGE_LIMIT)
                                .map_err(ProductionSourceOutputErrorV1::Resource)
                        });
                    assert!(refused.is_err());
                    let failed_storage = budget.failed_storage();
                    assert!(failed_storage.is_some());
                    assert_eq!(budget.storage(), floor);
                    let after_failure = budget.work();
                    assert!(after_failure > after_first);
                    roots[1].with_physical_address_relation_v1(budget, |relation, budget| {
                        let second = relation.access(0, budget)?.unwrap().operation();
                        assert_ne!(
                            first_operation.unwrap().block.function,
                            second.block.function
                        );
                        assert!(std::ptr::eq(relation.output(), checked.owner()));
                        Ok(())
                    })?;
                    assert!(budget.work() > after_failure);
                    assert_eq!(budget.failed_storage(), failed_storage);
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                },
            )
            .unwrap();
        });
    }
}

#[test]
fn physical_address_actual_formal_index_and_tuple_slots_match_own_gep_on_both_targets() {
    use fe2o3_kernel_ir::{CanonicalKirUseCoordinateV1 as Use, OperationKind};
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            for field in 0..2 {
                for shape in [BodyShape::Straight, BodyShape::Join, BodyShape::Loop] {
                    let source = canonical_same_typed_tuple_store_source_v1(field, shape);
                    with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                        let floor = budget.storage();
                        let mut completed = false;
                        root.with_physical_address_relation_v1(budget, |relation, budget| {
                            completed = true;
                            assert!(std::ptr::eq(root.output(), relation.output()));
                            assert_eq!(
                                (
                                    relation.global_access_count(),
                                    relation.private_access_count()
                                ),
                                (1, 0)
                            );
                            let access = relation.access(0, budget)?.unwrap();
                            assert_eq!(access.access_ordinal(), 0);
                            assert_eq!(
                                access.operation(),
                                root.access(0, budget)?.unwrap().operation()
                            );
                            assert_eq!(access.element_bytes(), 4);
                            assert_eq!(access.alignment(), 4);
                            let coordinate = access.gep();
                            let function = &relation.output().module().functions
                                [coordinate.block.function.0 as usize];
                            let gep = &function.body.as_ref().unwrap().blocks
                                [coordinate.block.block as usize]
                                .operations[coordinate.operation as usize];
                            assert!(matches!(gep.kind, OperationKind::GetElementPointer { .. }));
                            assert_eq!(
                                access.offset_use(),
                                Use::OperationOperand {
                                    operation: coordinate,
                                    operand: 1
                                }
                            );
                            assert!(relation.access(1, budget)?.is_none());
                            // This is correspondence only. Dynamic formal extraction
                            // remains UnsupportedIndexExpression in the F tests.
                            Ok(())
                        })?;
                        assert!(completed);
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    });
                }
            }
        }
    }
}

#[test]
fn physical_address_actual_nonzero_literal_and_distinct_slice_extents_are_checked() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            for bits in [3, 17] {
                let (source, _) = conditional_literal_source_v1(bits, None, false);
                with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                    root.with_physical_address_relation_v1(budget, |relation, budget| {
                        assert_eq!(relation.global_access_count(), 1);
                        assert!(relation.access(0, budget)?.is_some());
                        Ok(())
                    })
                });
            }
            let source = conditional_literal_two_guards_v1();
            with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                root.with_physical_address_relation_v1(budget, |relation, budget| {
                    assert_eq!(relation.global_access_count(), 2);
                    let first = relation.access(0, budget)?.unwrap();
                    let second = relation.access(1, budget)?.unwrap();
                    assert_ne!(first.allocation(), second.allocation());
                    assert_ne!(first.offset_use(), second.offset_use());
                    assert_ne!(first.ranked_extent(), second.ranked_extent());
                    Ok(())
                })
            });
        }
    }
}

#[test]
fn physical_address_scope_error_panic_and_distinct_ledger_preserve_live_parent() {
    use ProductionSourceOutputErrorV1 as Error;
    use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                let floor = budget.storage();
                for outcome in 0..3 {
                    let before = budget.work();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        root.with_physical_address_relation_v1(budget, |relation, budget| {
                            let held = budget.storage();
                            let before = budget.work();
                            assert!(relation.access(0, budget)?.is_some());
                            // Parent liveness19 + local floor2 + binary iteration(1+1).
                            assert_eq!(budget.work(), before + 23);
                            budget.release_storage(1).unwrap();
                            let denied = relation.access(0, budget);
                            budget.reserve_storage(1).unwrap();
                            assert!(matches!(denied, Err(Error::Resource(Resource::Accounting))));
                            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
                            let mut other = Budget::new(&mut work, held);
                            other.reserve_storage(held).unwrap();
                            assert!(matches!(
                                relation.access(0, &mut other),
                                Err(Error::Resource(Resource::Accounting))
                            ));
                            assert_eq!(other.storage(), held);
                            assert!(relation.access(0, budget)?.is_some());
                            match outcome {
                                0 => Ok(()),
                                1 => Err(Error::Invalid("physical callback refusal")),
                                _ => panic!("physical callback unwind"),
                            }
                        })
                    }));
                    match outcome {
                        0 => result.unwrap().unwrap(),
                        1 => assert!(result.unwrap().is_err()),
                        _ => assert!(result.is_err()),
                    }
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > before);
                }
                Ok(())
            });
        }
    }
}

#[test]
fn physical_address_scoped_access_query_has_an_independently_declared_final_work_boundary() {
    // Admission/construction is excluded. Both fixture pipelines explicitly use
    // LIMIT; spend unused capacity, leaving the derived getter cost. This is
    // not calibration of a successful whole-query run.
    const ACCESS: usize = 23;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            for exact in [true, false] {
                with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                    root.with_physical_address_relation_v1(budget, |relation, budget| {
                        let remaining = ACCESS - usize::from(!exact);
                        budget
                            .charge_work(LIMIT - budget.work() - remaining)
                            .unwrap();
                        let floor = budget.storage();
                        let result = relation.access(0, budget);
                        assert_eq!(result.is_ok(), exact);
                        // Under: the binary-search visit consumes the final
                        // unit; its following comparison is the denied unit.
                        assert_eq!(budget.work(), LIMIT);
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    })
                });
            }
        }
    }
}

fn physical_address_width_mutation_v1(
    root: &fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1<'_>,
) -> ProductionRankedKernelLoweringInputV1 {
    let mut changed = 0;
    let blocks = root
        .lowering
        .kernel()
        .blocks()
        .iter()
        .map(|block| {
            let mut operations = block.operations().to_vec();
            for operation in &mut operations {
                if let ProductionRankedOperationV1::ViewInSpace {
                    element_width,
                    memory_space: dialect_kernel::MemorySpaceAttr::Global,
                    ..
                } = operation
                {
                    assert_eq!(*element_width, 32);
                    *element_width = 16;
                    changed += 1;
                }
            }
            fe2o3_pliron::ProductionRankedBlockV1::new(operations, block.terminator().clone())
        })
        .collect();
    assert!(changed > 0);
    let kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
        root.lowering.kernel().function_name(),
        root.lowering.kernel().argument_count(),
        blocks,
    )
    .unwrap();
    fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
        fe2o3_pliron::ProductionConstructionV1::ranked_kernel("physical_width_mutation", kernel)
            .unwrap(),
        fe2o3_pliron::ProductionSessionLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn physical_address_rejects_wrong_element_units_after_real_completed_store_scope() {
    use fe2o3_lower_mir_kernel::ProductionCanonicalMemoryAnalysisCandidateV1 as Candidate;
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_canonical_control_candidate_test_v1(
            &source,
            profile,
            |_| {},
            |view, roots, budget| {
                let root = &roots[0];
                let changed = physical_address_width_mutation_v1(root);
                let candidates = [Candidate {
                    lowering: &changed,
                    selected_root: root.selected_root,
                    selected_function: root.selected_function,
                    access_sources: root.access_sources,
                    executable_effect_sources: root.executable_effect_sources,
                    control: root.control,
                }];
                let floor = budget.storage();
                let mut completed_store = false;
                let result: Result<(), ProductionSourceOutputErrorV1> = view
                    .with_conditional_memory_control_coverage_v1(
                        &candidates,
                        budget,
                        |control, budget| {
                            view.with_canonical_store_analysis_v1(
                                &candidates,
                                control,
                                budget,
                                |roots, budget| {
                                    completed_store = true;
                                    roots[0].with_physical_address_relation_v1(budget, |_, _| {
                                        panic!("wrong element width must not complete")
                                    })
                                },
                            )
                        },
                    );
                assert!(completed_store, "mutation must reach the address check");
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "physical address requires one exact scalar slice extent and width"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn physical_address_changed_actual_offset_is_refused_by_checked_transition_before_scope() {
    use fe2o3_kernel_ir::OperationKind;
    let source = canonical_same_typed_tuple_store_source_v1(1, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        with_actual(&source, profile, |bound, checked, budget| {
            let floor = budget.storage();
            let mut changed = checked.owner().module().clone();
            let body = changed.functions[0].body.as_mut().unwrap();
            let length = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::SliceLength { .. }))
                .unwrap()
                .results[0]
                .id;
            let mut count = 0;
            for operation in body
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
            {
                if let OperationKind::GetElementPointer { offset, .. } = &mut operation.kind {
                    assert_ne!(*offset, length);
                    *offset = length;
                    count += 1;
                }
            }
            assert_eq!(count, 1);
            let (changed, storage) =
                Owner::from_module_ref_with_verification_budget_v12(&changed, budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (input, input_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(bound, budget).unwrap();
            budget
                .reserve_storage(input_storage.retained_storage())
                .unwrap();
            let (output, output_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&changed, budget).unwrap();
            budget
                .reserve_storage(output_storage.retained_storage())
                .unwrap();
            assert!(
                fe2o3_kernel_analysis::check_canonical_kir_transition_v1(
                    &input,
                    &output,
                    checked.occurrences().candidate(),
                    budget
                )
                .is_err()
            );
            drop(output);
            budget
                .release_storage(output_storage.retained_storage())
                .unwrap();
            drop(input);
            budget
                .release_storage(input_storage.retained_storage())
                .unwrap();
            drop(changed);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn physical_address_actual_query_storage_denial_drops_scratch_before_reentry() {
    let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            with_physical_address_source_v1(&source, profile, policy3, |root, budget| {
                let floor = budget.storage();
                let prefix = budget.work();
                // The nonzero context/workspace header cannot fit in one byte.
                // This is a constructor refusal test, not an inferred exact cap.
                let held = STORAGE_LIMIT - floor - 1;
                budget.reserve_storage(held).unwrap();
                let denied_floor = budget.storage();
                let mut called = false;
                let result = root.with_physical_address_relation_v1(budget, |_, _| {
                    called = true;
                    Ok(())
                });
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(_))
                ));
                assert!(!called);
                assert_eq!(budget.storage(), denied_floor);
                assert!(budget.work() > prefix);
                let failed = budget.failed_storage();
                assert!(failed.is_some_and(|requested| requested > STORAGE_LIMIT));
                budget.release_storage(held).unwrap();
                assert_eq!(budget.storage(), floor);
                root.with_physical_address_relation_v1(budget, |relation, budget| {
                    assert!(relation.access(0, budget)?.is_some());
                    Ok(())
                })?;
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_storage(), failed);
                Ok(())
            });
        }
    }
}

#[test]
fn physical_address_genuine_private_c_keeps_separate_partition_on_both_endpoints() {
    let source = canonical_private_constant_store_source_v1();
    let inputs = [ranked_root_input_1d(A_NAME, 247, 64)];
    let references = crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for policy3 in [false, true] {
            let mut completed_store = false;
            let mut completed_address = false;
            let mut inspect =
                |roots: &[fe2o3_lower_mir_kernel::ProductionScopedCanonicalStoreAnalysisV1<
                    '_,
                    '_,
                    '_,
                >],
                 budget: &mut Budget<'_>| {
                    let [root] = roots else {
                        panic!("one actual Private C root")
                    };
                    completed_store = true;
                    let floor = budget.storage();
                    let access = root.access(0, budget)?.unwrap();
                    assert_eq!(root.access_count(), 1);
                    assert!(access.store_value().is_some());
                    let coordinate = access.operation();
                    let operation = &root.output().module().functions
                        [coordinate.block.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[coordinate.block.block as usize]
                        .operations[coordinate.operation as usize];
                    assert!(
                        matches!(&operation.kind, fe2o3_kernel_ir::OperationKind::Store { access, .. }
                    if access.address_space == fe2o3_kernel_ir::AddressSpace::Private)
                    );
                    root.with_physical_address_relation_v1(budget, |relation, budget| {
                        completed_address = true;
                        assert!(std::ptr::eq(relation.output(), root.output()));
                        assert_eq!(
                            (
                                relation.global_access_count(),
                                relation.private_access_count()
                            ),
                            (0, 1)
                        );
                        assert!(relation.access(0, budget)?.is_none());
                        Ok(())
                    })?;
                    assert_eq!(budget.storage(), floor);
                    Ok::<(), ProductionSourceOutputErrorV1>(())
                };
            if policy3 {
                with_actual_policy3_canonical_view_v1(&source, profile, |checked, view, budget| {
                    let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
                    with_ranked_root_preparation_v1(
                        &projection, &inputs, &references, |effects, references| {
                            checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                                view, budget, |session| {
                                    with_prepared_canonical_memory_session_v1(
                                        &projection, &inputs, effects, references, session,
                                        |roots, budget| {
                                            assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                                            inspect(roots, budget)
                                        },
                                    )
                                },
                            )
                        },
                    ).unwrap();
                });
            } else {
                with_actual(&source, profile, |bound, checked, budget| {
                    with_projected_canonical_memory_analysis_v1(
                        &source,
                        bound,
                        checked,
                        profile,
                        &inputs,
                        &references,
                        budget,
                        |roots, budget| {
                            assert!(std::ptr::eq(roots[0].output(), checked.owner()));
                            inspect(roots, budget)
                        },
                    )
                    .unwrap();
                });
            }
            assert!(completed_store && completed_address);
        }
    }
}

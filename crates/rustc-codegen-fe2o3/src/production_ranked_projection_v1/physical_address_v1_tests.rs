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
mod borrowed_full_address_tests {
    use super::*;
    use fe2o3_lower_mir_kernel::{
        ProductionBorrowedRankedCorrespondenceV1, ProductionProjectionArgumentComponentV1,
        ProductionScopedCanonicalStoreAnalysisV1,
    };

    #[test]
    fn full_expression_recording_preserves_actual_call_refusal() {
        let source = canonical_same_typed_tuple_store_source_v1(0, BodyShape::Straight);
        with_actual_policy3_canonical_view_v1(&source, Profile::Gfx942, |_, view, budget| {
            let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
            let inputs = [ranked_root_input_1d(A_NAME, 247, 1)];
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            with_ranked_root_preparation_v1(&projection, &inputs, &references, |effects, partition| {
                checked_output_session_v1::with_checked_output_assertions_view_budget_v1(view, budget, |session| {
                    session.with_canonical_memory_scope_v1(|session| {
                        let source_root = projection.source_launch().roots()[0];
                        let selection = projection.semantic_ssa().source_semantic().select_kernel_body_for_root_v1(source_root.selected_root()).unwrap();
                        let mut facts = session.for_source(source_root.selected_root(), selection.body());
                        let old = project_and_verify_ranked_root_control_inner_v1(
                            projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                            &partition[0], &mut facts,
                        ).err().expect("the historical tuple-result expression path refuses");
                        let mut recorder = canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(&mut facts)?;
                        let recorded = project_and_verify_ranked_root_control_with_address_claims_v1(
                            projection.semantic_ssa(), effects, selection, &inputs[0], source_root,
                            &partition[0], &mut facts, Some(&mut recorder),
                        ).err().expect("descriptive recording must not enable recorded-Call control");
                        assert_eq!(format!("{old:?}"), format!("{recorded:?}"));
                        Ok(())
                    })
                })
            }).unwrap();
        });
    }

    fn with_fixture(
        profile: Profile,
        capture: bool,
        multiple: bool,
        mutate: impl FnOnce(&mut [canonical_memory_control_v1::CanonicalMemoryControlRecorderV1]),
        body: impl FnOnce(
            &[ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>],
            &ProductionBorrowedRankedCorrespondenceV1<'_>,
            &[canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) {
        with_index_fixture(
            profile,
            capture,
            multiple,
            false,
            |recorders, _| mutate(recorders),
            body,
        );
    }

    fn with_index_fixture(
        profile: Profile,
        capture: bool,
        multiple: bool,
        formal_index: bool,
        mutate: impl FnOnce(
            &mut [canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
            Option<SemanticLocalIdV1>,
        ),
        body: impl FnOnce(
            &[ProductionScopedCanonicalStoreAnalysisV1<'_, '_, '_>],
            &ProductionBorrowedRankedCorrespondenceV1<'_>,
            &[canonical_memory_control_v1::CanonicalMemoryControlRecorderV1],
            &mut Budget<'_>,
        ) -> Result<(), ProductionSourceOutputErrorV1>,
    ) {
        // The seed provides admitted syntax only. This fresh owner is captured
        // BEFORE its own materialization; no capture is grafted onto borrowed N.
        let seed = genuine_dynamic_global_source_v1(GlobalWriteExpressionShapeV1::Parameter, 1);
        let semantic = seed.semantic_ssa().source_semantic();
        let mut functions = semantic.functions().to_vec();
        let extra_index =
            formal_index.then(|| {
                let root = &functions[0];
                let mut blocks = root.blocks().to_vec();
                let mut removed = 0;
                for block in &mut blocks {
                    let statements = block.statements().iter().filter(|statement| {
                    if matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                        if assignment.destination().local().index() == 8
                            && assignment.destination().projections().is_empty())
                    {
                        removed += 1;
                        false
                    } else {
                        true
                    }
                }).cloned().collect();
                    *block = SemanticBasicBlockV1::new(
                        block.identity(),
                        block.source(),
                        statements,
                        block.terminator().clone(),
                    )
                    .unwrap();
                }
                assert_eq!(removed, 1);
                let mut locals = root.locals().to_vec();
                assert_eq!(locals[8].ty(), A_U64);
                locals[8] = SemanticLocalDeclV1::new(
                    locals[8].identity(),
                    A_U64,
                    SemanticLocalRoleV1::Argument(3),
                    locals[8].source(),
                );
                let extra = SemanticLocalIdV1::from_index(locals.len() as u32);
                locals.push(local(250, A_U64, SemanticLocalRoleV1::Argument(4)));
                let old = root.abi();
                assert_eq!(old.fixed_count(), 3);
                let mut arguments = old.arguments().to_vec();
                let mut ownership = old.source_argument_ownership().to_vec();
                for _ in 0..2 {
                    arguments.push(SemanticAbiArgumentV1::source(
                        neutral_plain_direct_abi_value_v1(A_U64),
                    ));
                    ownership.push(SemanticSourceArgumentOwnershipV1::ByValue);
                }
                let abi = SemanticFunctionAbiV1::from_rustc(
                    old.identity(),
                    old.layout_identity(),
                    old.canon_abi(),
                    old.extern_abi(),
                    old.can_unwind(),
                    old.c_variadic(),
                    5,
                    arguments,
                    old.return_value().clone(),
                )
                .unwrap()
                .with_source_argument_ownership(ownership)
                .unwrap();
                functions[0] = ordinary_rebuild_v1(root, abi, locals, blocks);
                extra
            });
        let mut inputs = vec![ranked_root_input_1d(A_NAME, 247, 1)];
        if multiple {
            let first = &functions[0];
            let second = SemanticFunctionDeclV1::new(
                SemanticFunctionIdentityV1::from_sha256(bytes(251)),
                first.role(),
                SemanticItemDefinitionIdentityV1::from_sha256(bytes(252)),
                SemanticMonomorphizationIdentityV1::from_sha256(bytes(253)),
                first.generic_type_arguments_identity(),
                first.const_generic_arguments_identity(),
                first.source(),
                first.abi().clone(),
                first.locals().to_vec(),
                first.entry(),
                first.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(SemanticKernelEntryV1::new(
                SemanticLinkSymbolV1::new(b"full_address_second".to_vec()).unwrap(),
                SemanticKernelBindingIdentityV1::from_sha256(bytes(248)),
                first.kernel_entry().unwrap().source_contract(),
            ));
            functions.push(second);
            inputs.push(ranked_root_input_1d("full_address_second", 248, 1));
        }
        let roots = (0..functions.len())
            .map(|i| SemanticFunctionIdV1::from_index(i as u32))
            .collect::<Vec<_>>();
        let callables = roots
            .iter()
            .map(|root| SemanticCallableDeclV1::defined(*root))
            .collect();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            semantic.target(),
            semantic.types().to_vec(),
            semantic.allocations().to_vec(),
            semantic.statics().to_vec(),
            semantic.vtables().to_vec(),
            functions,
            callables,
            roots,
        )
        .unwrap()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
        let semantic_owner = ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap();
        let mut ssa = ProductionSemanticSsaOwnerV1::try_new(
            semantic_owner,
            fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap();
        drop(seed);
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, STORAGE_LIMIT);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(PREFIX).unwrap();
        let capture_bytes = if capture {
            let receipt = ssa
                .try_capture_occurrences_with_budget_v1(&mut budget)
                .unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(ssa.occurrence_storage(), Some(receipt));
            let view = ssa.occurrences_v1().unwrap();
            for index in 0..inputs.len() {
                let rows = view
                    .function(SemanticFunctionIdV1::from_index(index as u32))
                    .unwrap();
                assert!(std::ptr::eq(rows.owner(), &ssa));
                let source_use = rows.events().iter().find(|row| {
                    row.site() == fe2o3_pliron::ProductionSemanticSsaOccurrenceSiteV1::Statement {
                        block: fe2o3_mir_model::SsaBlockIdV1::new(1), statement: 0,
                    } && row.operand() == fe2o3_pliron::ProductionSemanticSsaOperandRoleV1::Destination
                        && row.role() == fe2o3_pliron::ProductionSemanticSsaEventRoleV1::ProjectionIndexUse(1)
                }).expect("the exact Store index event must be captured");
                assert!(source_use.is_promoted() && source_use.is_reachable());
                assert!(
                    matches!(source_use.resolved(), Some(fe2o3_mir_model::SsaResolvedEventV1::Use { variable, .. }) if variable.get() == 8)
                );
                if let Some(extra) = extra_index {
                    use fe2o3_pliron::ProductionSemanticSsaEntryOriginV1 as Origin;
                    let entry = rows
                        .entry_definitions()
                        .iter()
                        .find(|entry| entry.variable().get() == 8)
                        .unwrap();
                    let other = rows
                        .entry_definitions()
                        .iter()
                        .find(|entry| entry.variable().get() == extra.index())
                        .unwrap();
                    assert_eq!(entry.origin(), Origin::Argument(3));
                    assert_eq!(other.origin(), Origin::Argument(4));
                    assert!(entry.value().is_some() && other.value().is_some());
                    assert_ne!(entry.value(), other.value());
                    let Some(fe2o3_mir_model::SsaResolvedEventV1::Use { value, .. }) =
                        source_use.resolved()
                    else {
                        unreachable!()
                    };
                    assert_eq!(entry.value(), Some(value));
                    assert_ne!(other.value(), Some(value));
                }
            }
            receipt.retained_storage()
        } else {
            assert!(ssa.occurrences_v1().is_none());
            0
        };
        let launch = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs).unwrap();
        let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            fe2o3_lower_mir_kernel::ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        let source_bytes = source.executable_storage().retained_storage()
            + source.assert_origin_storage().payload_storage();
        budget.reserve_storage(source_bytes).unwrap();
        let bound =
            dialect_amdgcn::bind_production_target_v1(source.executable().module(), profile)
                .unwrap();
        let (input, input_storage) =
            Owner::from_module_ref_with_verification_budget_v12(bound.module(), &mut budget)
                .unwrap();
        budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let observed =
            fe2o3_pliron::optimize_native_neutral_kernel_ir_policy3_v1(&input, &mut budget)
                .unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let checked = observed.try_check_and_finish_v1(&mut budget).unwrap();
        let output_bytes = checked.storage().retained_storage();
        budget.reserve_storage(output_bytes).unwrap();
        let coordinate_bytes;
        {
            let (coordinates, coordinates_storage) =
                dialect_amdgcn::check_production_target_coordinate_preservation_v1(
                    source.executable(),
                    &input,
                    profile,
                    &mut budget,
                )
                .unwrap();
            budget
                .reserve_storage(coordinates_storage.retained_storage())
                .unwrap();
            coordinate_bytes = coordinates_storage.retained_storage();
            let (view, view_storage) =
                fe2o3_lower_mir_kernel::derive_source_output_occurrences_policy3_v1(
                    &source,
                    &coordinates,
                    &checked,
                    &mut budget,
                )
                .unwrap();
            budget
                .reserve_storage(view_storage.retained_storage())
                .unwrap();
            let projection = RankedProjectionSourceV1::from_legacy(&source).unwrap();
            let references =
                crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::default();
            let floor = budget.storage();
            with_ranked_root_preparation_v1(
                &projection,
                &inputs,
                &references,
                |effects, partition| {
                    checked_output_session_v1::with_checked_output_assertions_view_budget_v1(
                        &view,
                        &mut budget,
                        |session| {
                            session.with_canonical_memory_scope_v1(|session| {
                                let mut full = Vec::new();
                                let mut recorders = Vec::new();
                                for (ordinal, source_root) in
                                    projection.source_launch().roots().iter().enumerate()
                                {
                                    let selection = projection
                                        .semantic_ssa()
                                        .source_semantic()
                                        .select_kernel_body_for_root_v1(source_root.selected_root())
                                        .unwrap();
                                    let mut facts = session
                                        .for_source(source_root.selected_root(), selection.body());
                                    let legacy = project_and_verify_ranked_root_control_inner_v1(
                                        projection.semantic_ssa(),
                                        effects,
                                        selection,
                                        &inputs[ordinal],
                                        *source_root,
                                        &partition[ordinal],
                                        &mut facts,
                                    )?;
                                    let mut recorder =
                                canonical_memory_control_v1::CanonicalMemoryControlRecorderV1::new(
                                    &mut facts,
                                )?;
                                    let captured =
                                project_and_verify_ranked_root_control_with_address_claims_v1(
                                    projection.semantic_ssa(),
                                    effects,
                                    selection,
                                    &inputs[ordinal],
                                    *source_root,
                                    &partition[ordinal],
                                    &mut facts,
                                    Some(&mut recorder),
                                )?;
                                    assert_eq!(captured.ranked_ir, legacy.ranked_ir);
                                    assert_eq!(captured.access_sources, legacy.access_sources);
                                    assert_eq!(
                                        captured.all_kernel_checks_are_clean(),
                                        legacy.all_kernel_checks_are_clean()
                                    );
                                    assert!(captured.all_kernel_checks_are_clean());
                                    full.push(captured);
                                    recorders.push(recorder);
                                }
                                mutate(&mut recorders, extra_index);
                                with_prepared_canonical_memory_session_v1(
                                    &projection,
                                    &inputs,
                                    effects,
                                    partition,
                                    session,
                                    |analyses, budget| {
                                        assert_eq!(analyses.len(), inputs.len());
                                        for analysis in analyses {
                                            assert!(std::ptr::eq(
                                                analysis.output(),
                                                checked.owner()
                                            ));
                                        }
                                        with_authenticated_borrowed_ranked_source_roster_v1(
                                            &source,
                                            full.into_boxed_slice(),
                                            budget,
                                            |original, verification, budget| {
                                                assert!(std::ptr::eq(
                                                    original.materialized(),
                                                    &source
                                                ));
                                                for root in verification.roots() {
                                                    assert!(
                                                !root
                                                    .verification()
                                                    .has_authenticated_functional_verification()
                                            );
                                                    assert!(
                                                        root.verification()
                                                            .aggregate_verus_execution()
                                                            .is_none()
                                                    );
                                                }
                                                Ok(body(analyses, original, &recorders, budget))
                                            },
                                        )
                                        .expect("actual full-ranked R1 correspondence must succeed")
                                    },
                                )
                            })
                        },
                    )
                },
            )
            .unwrap();
            assert!(references.as_slice().is_empty());
            assert_eq!(budget.storage(), floor);
            drop(view);
            budget
                .release_storage(view_storage.retained_storage())
                .unwrap();
        }
        budget.release_storage(coordinate_bytes).unwrap();
        drop(checked);
        budget.release_storage(output_bytes).unwrap();
        drop(input);
        budget
            .release_storage(input_storage.retained_storage())
            .unwrap();
        drop(source);
        budget.release_storage(source_bytes).unwrap();
        budget.release_storage(capture_bytes).unwrap();
        assert_eq!(budget.storage(), PREFIX);
        assert!(budget.work() > 7);
    }

    #[test]
    fn captured_scalar_formal_index_matches_exact_own_o_use_on_both_targets() {
        use fe2o3_kernel_ir::{
            CanonicalKirDefinitionCoordinateV1 as Def, CastKind, OperationKind, ScalarType, Type,
        };
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_index_fixture(
                profile,
                true,
                true,
                true,
                |_, extra| {
                    assert!(extra.is_some());
                },
                |analyses, original, recorders, budget| {
                    assert_eq!(analyses.len(), 2);
                    let floor = budget.storage();
                    let mut prior_work = budget.work();
                    for (ordinal, analysis) in analyses.iter().enumerate() {
                        analysis.with_physical_address_relation_v1(budget, |relation, budget| {
                            let access = relation.access(0, budget)?.unwrap();
                            let function = &relation.output().module().functions[access.gep().block.function.0 as usize];
                            assert!(matches!(function.signature.parameters.as_slice(), [
                                Type::Slice(_), Type::Slice(_), Type::Scalar(ScalarType::U32),
                                Type::Scalar(ScalarType::U64), Type::Scalar(ScalarType::U64),
                            ]));
                            let body = function.body.as_ref().unwrap();
                            let Def::Result { operation, result: 0 } = access.offset_definition() else {
                                panic!("this source U64 index must reach its own INDEX cast result");
                            };
                            assert_eq!(operation.block.function, access.gep().block.function);
                            let cast = &body.blocks[operation.block.block as usize].operations[operation.operation as usize];
                            assert!(matches!(cast.kind, OperationKind::Cast {
                                kind: CastKind::Bitcast, value, to: Type::INDEX,
                            } if value == body.parameters[3]));
                            assert_ne!(body.parameters[3], body.parameters[4]);
                            let gep = &body.blocks[access.gep().block.block as usize].operations[access.gep().operation as usize];
                            assert!(matches!(gep.kind, OperationKind::GetElementPointer { offset, .. }
                                if offset == cast.results[0].id));
                            for _ in 0..2 {
                                let live = budget.storage();
                                relation.check_borrowed_ranked_addresses_v1(
                                    original, ordinal, recorders[ordinal].candidate(), budget,
                                )?;
                                assert_eq!(budget.storage(), live);
                                assert!(budget.work() > prior_work);
                                prior_work = budget.work();
                            }
                            Ok(())
                        })?;
                        assert_eq!(budget.storage(), floor);
                    }
                    Ok(())
                },
            );
        }
    }

    #[test]
    fn captured_scalar_formal_index_rejects_same_typed_claimed_source_use_swap() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            with_index_fixture(
                profile,
                true,
                false,
                true,
                |recorders, extra| {
                    let extra = extra.unwrap();
                    let claims = &mut recorders[0].candidate_mut().arguments;
                    let index = claims
                        .iter_mut()
                        .find(|claim| {
                            claim.source_local.index() == 8
                                && claim.component
                                    == ProductionProjectionArgumentComponentV1::Scalar
                        })
                        .unwrap();
                    assert_ne!(index.source_local, extra);
                    // Only an inert full claim is hostile. The exact captured
                    // Store use still resolves to formal 3, never formal 4.
                    index.source_local = extra;
                },
                |analyses, original, recorders, budget| {
                    analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                        let floor = budget.storage();
                        let before = budget.work();
                        assert!(matches!(
                            relation.check_borrowed_ranked_addresses_v1(
                                original,
                                0,
                                recorders[0].candidate(),
                                budget,
                            ),
                            Err(ProductionSourceOutputErrorV1::Invalid(
                                "full address leaf source mapping differs"
                            ))
                        ));
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work() > before);
                        Ok(())
                    })
                },
            );
        }
    }

    #[test]
    fn captured_full_addresses_match_same_actual_o_without_functional_authority() {
        for profile in [Profile::Gfx942, Profile::Gfx950] {
            for multiple in [false, true] {
                with_fixture(
                    profile,
                    true,
                    multiple,
                    |_| {},
                    |analyses, original, recorders, budget| {
                        let floor = budget.storage();
                        let mut history = budget.work();
                        for (ordinal, analysis) in analyses.iter().enumerate() {
                            analysis.with_physical_address_relation_v1(
                                budget,
                                |relation, budget| {
                                    assert_eq!(relation.global_access_count(), 1);
                                    for _ in 0..2 {
                                        let live = budget.storage();
                                        relation.check_borrowed_ranked_addresses_v1(
                                            original,
                                            ordinal,
                                            recorders[ordinal].candidate(),
                                            budget,
                                        )?;
                                        assert_eq!(budget.storage(), live);
                                        assert!(budget.work() > history);
                                        history = budget.work();
                                    }
                                    Ok(())
                                },
                            )?;
                            assert_eq!(budget.storage(), floor);
                        }
                        Ok(())
                    },
                );
            }
        }
    }

    #[test]
    fn full_address_join_refuses_uncaptured_source_after_real_r1_and_d() {
        with_fixture(
            Profile::Gfx942,
            false,
            false,
            |_| {},
            |analyses, original, recorders, budget| {
                analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                    let floor = budget.storage();
                    assert!(matches!(
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorders[0].candidate(),
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address source SSA capture absent"
                        ))
                    ));
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                })
            },
        );
    }

    #[test]
    fn full_address_claim_mutations_reach_exact_join_after_r1_and_d() {
        for mutation in 0..5 {
            with_fixture(
                Profile::Gfx942,
                true,
                false,
                |recorders| {
                    let claims = &mut recorders[0].candidate_mut().arguments;
                    let index = claims
                        .iter()
                        .position(|row| {
                            row.source_local.index() == 8
                                && row.component == ProductionProjectionArgumentComponentV1::Scalar
                        })
                        .unwrap();
                    let extent = claims
                        .iter()
                        .position(|row| {
                            row.component == ProductionProjectionArgumentComponentV1::SliceLength
                                && row.source_local.index() == 1
                        })
                        .unwrap();
                    match mutation {
                        0 => claims[index].source_local = SemanticLocalIdV1::from_index(7),
                        1 => claims[extent].source_local = SemanticLocalIdV1::from_index(2),
                        2 => {
                            claims.remove(index);
                        }
                        3 => claims[extent] = claims[index],
                        4 => {
                            let value = claims[index].ranked_value;
                            claims[index].ranked_value = claims[extent].ranked_value;
                            claims[extent].ranked_value = value;
                        }
                        _ => unreachable!(),
                    }
                },
                |analyses, original, recorders, budget| {
                    analyses[0].with_physical_address_relation_v1(budget, |relation, budget| {
                        let floor = budget.storage();
                        let error = relation
                            .check_borrowed_ranked_addresses_v1(
                                original,
                                0,
                                recorders[0].candidate(),
                                budget,
                            )
                            .unwrap_err();
                        match mutation {
                            0 | 1 | 4 => assert!(matches!(
                                error,
                                ProductionSourceOutputErrorV1::Invalid(
                                    "full address leaf source mapping differs"
                                )
                            )),
                            2 => assert!(matches!(
                                error,
                                ProductionSourceOutputErrorV1::Invalid(
                                    "full address source claim absent"
                                )
                            )),
                            3 => {
                                assert!(matches!(error, ProductionSourceOutputErrorV1::Invalid(_)))
                            }
                            _ => unreachable!(),
                        }
                        assert_eq!(budget.storage(), floor);
                        Ok(())
                    })
                },
            );
        }
    }

    #[test]
    fn full_address_scopes_keep_receipts_on_error_unwind_storage_denial_and_reentry() {
        use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1 as Resource;
        with_fixture(
            Profile::Gfx950,
            true,
            true,
            |_| {},
            |analyses, original, recorders, budget| {
                let floor = budget.storage();
                let result: Result<(), ProductionSourceOutputErrorV1> = analyses[0]
                    .with_physical_address_relation_v1(budget, |relation, budget| {
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorders[0].candidate(),
                            budget,
                        )?;
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full-address callback marker",
                        ))
                    });
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Invalid(
                        "full-address callback marker"
                    ))
                ));
                assert_eq!(budget.storage(), floor);
                let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: Result<(), ProductionSourceOutputErrorV1> = analyses[0]
                        .with_physical_address_relation_v1(budget, |relation, budget| {
                            relation.check_borrowed_ranked_addresses_v1(
                                original,
                                0,
                                recorders[0].candidate(),
                                budget,
                            )?;
                            std::panic::panic_any(0x333_u32);
                        });
                }))
                .unwrap_err();
                assert_eq!(panic.downcast_ref::<u32>(), Some(&0x333));
                assert_eq!(budget.storage(), floor);
                let before_second = budget.work();
                analyses[1].with_physical_address_relation_v1(budget, |relation, budget| {
                    let live = budget.storage();
                    assert!(matches!(
                        relation.check_borrowed_ranked_addresses_v1(
                            original,
                            0,
                            recorders[1].candidate(),
                            budget
                        ),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "full address selected root differs"
                        ))
                    ));
                    let padding = budget.storage_limit() - budget.storage();
                    budget.reserve_storage(padding).unwrap();
                    let denied = relation.check_borrowed_ranked_addresses_v1(
                        original,
                        1,
                        recorders[1].candidate(),
                        budget,
                    );
                    assert!(matches!(
                        denied,
                        Err(ProductionSourceOutputErrorV1::Resource(Resource::Storage(
                            _
                        )))
                    ));
                    assert_eq!(budget.storage(), live + padding);
                    budget.release_storage(padding).unwrap();
                    relation.check_borrowed_ranked_addresses_v1(
                        original,
                        1,
                        recorders[1].candidate(),
                        budget,
                    )?;
                    assert_eq!(budget.storage(), live);
                    Ok(())
                })?;
                assert!(budget.work() > before_second);
                assert_eq!(budget.storage(), floor);
                Ok(())
            },
        );
    }
}

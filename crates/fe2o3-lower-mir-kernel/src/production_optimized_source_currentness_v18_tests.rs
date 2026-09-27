#[test]
fn actual_scalar_output_currentness_uses_exact_fresh_memory_versions() {
    for factory in [
        scalar_payload_owner_v18 as fn() -> ProductionSemanticSsaOwnerV1,
        folding_source_owner_v18,
        scalar_read_store_owner_v18,
    ] {
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                analyses.with_memory_versions(budget, |input_memory, output_memory, budget| {
                    let mut visited = 0;
                    for root in 0..original.source.root_count(budget)? {
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original, optimized, root, input_memory, output_memory, budget,
                            |memory, budget| {
                                memory.visit_accesses(budget, |instance, anchor, input, output, budget| {
                                    assert!(original.source.instance(root, instance, budget).is_ok());
                                    let source = original.source.sidecar(root, instance, budget)?;
                                    assert!(source.scoped_memory_anchors.as_ref().unwrap().rows.get(anchor).is_some());
                                    let (actual, pointer) = output.expect("live fixture retains each physical access");
                                    assert!(matches!(optimized.operation(input, budget)?,
                                        ProductionOptimizedSourceOperationV18::Retained { output, .. } if output == actual));
                                    let output = optimized.output_inventory(budget)?;
                                    assert!(matches!(optimized_source_operation_row_v18(output, actual, budget)?.operation.kind,
                                        OperationKind::Load { pointer: value, .. } | OperationKind::Store { pointer: value, .. }
                                        if value == pointer));
                                    assert!(output_memory.operation(actual, budget).unwrap().is_some());
                                    visited += 1;
                                    Ok(())
                                })
                            },
                        )?;
                    }
                    assert!(visited > 0);
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })
            })
        });
    }
}

#[test]
fn optimized_source_currentness_entry_rechecks_every_genuine_scalar_root() {
    for factory in [
        scalar_payload_owner_v18 as fn() -> ProductionSemanticSsaOwnerV1,
        folding_source_owner_v18,
        scalar_read_store_owner_v18,
    ] {
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            let expected = original.source(budget)?.root_count(budget)?;
            assert_eq!(
                expected, 2,
                "distinct original roots must both reach output currentness"
            );
            let floor = budget.storage();
            assert_eq!(
                original.check_optimized_source_currentness_v18(optimized, budget)?,
                expected
            );
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    }
}

#[test]
fn optimized_source_currentness_entry_retains_genuine_stored_and_fresh_restart_successes() {
    for factory in [
        stored_physical_owner_v29 as fn() -> ProductionSemanticSsaOwnerV1,
        || physical_address_owner(PhysicalAddressCase::FreshRestart),
    ] {
        run_production_optimized_consumer_v18(factory, |original, optimized, budget| {
            let expected = original.source(budget)?.root_count(budget)?;
            let floor = budget.storage();
            assert_eq!(
                original.check_optimized_source_currentness_v18(optimized, budget)?,
                expected
            );
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    }
}

#[test]
fn ordinary_scalar_memory_retains_original_activation_alternatives() {
    run_production_optimized_consumer_v18(
        scalar_payload_owner_v18,
        |original, optimized, budget| {
            let expected =
                scoped_raw_admission_v29::test_ordinary_activation_census_v29(original, 0, budget)?;
            original.with_optimized_analysis_v18(optimized, budget, |analysis, budget| {
                analysis.with_memory_versions(budget, |input, output, budget| {
                    scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                        original,
                        optimized,
                        0,
                        input,
                        output,
                        budget,
                        |checked, budget| {
                            let mut count = 0;
                            checked.visit_accesses(budget, |_, _, _, output, _| {
                                assert!(output.is_some());
                                count += 1;
                                Ok(())
                            })?;
                            assert_eq!(count, expected);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )
                })
            })
        },
    );
}

#[test]
fn descriptor_effects_remain_outside_a_checked_empty_local_currentness_domain() {
    run_production_optimized_consumer_v18(
        two_descriptor_reads_at_one_source_site_v18,
        |original, optimized, budget| {
            let owner = original.source.root_row(0)?;
            assert!(owner.source_slots.slots.is_empty());
            assert!(
                owner.source_slots.pending_memory.is_some(),
                "original source census must execute"
            );
            let mut failure_reads = 0;
            for sidecar in &owner.sidecars.rows {
                let anchors = sidecar.scoped_memory_anchors.as_ref().unwrap();
                budget.charge_work(anchors.rows.len())?;
                failure_reads += anchors
                    .rows
                    .iter()
                    .filter(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::FailureRead { .. }))
                    .count();
            }
            assert!(
                failure_reads > 0,
                "descriptor source must exercise real failure-message reads"
            );
            let input_function =
                &original.inventory.functions()[original.source.root(0, budget)?.1];
            budget.charge_work(input_function.operations.len())?;
            assert!(
                !original.inventory.operations()[input_function.operations.clone()]
                    .iter()
                    .any(|row| matches!(row.operation.kind, OperationKind::Alloca { .. })),
                "empty source roster must agree with the actual input graph"
            );
            let output = optimized.output_inventory(budget)?;
            let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
            assert!(output.effects()[function.effects.clone()].len() >= 2);
            original.with_optimized_analysis_v18(optimized, budget, |analysis, budget| {
                analysis.with_memory_versions(budget, |input, output, budget| {
                    scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                        original,
                        optimized,
                        0,
                        input,
                        output,
                        budget,
                        |physical, budget| {
                            let mut local = 0;
                            physical.visit_accesses(budget, |_, _, _, _, _| {
                                local += 1;
                                Ok(())
                            })?;
                            assert_eq!(local, 0, "descriptor effects are not private-cell proofs");
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )
                })
            })
        },
    );
}

fn repeated_descriptor_helper_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    use fe2o3_mir_model::semantic_mir_v1::*;
    let base = two_descriptor_reads_at_one_source_site_v18();
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    let abi = original.abi();
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([222; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        3,
        abi.arguments().to_vec(),
        abi.return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(abi.source_argument_ownership().to_vec())
    .unwrap();
    let edge = |target| {
        SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::CallReturn,
            SemanticBlockIdV1::from_index(target),
        )
    };
    let call = |target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                (1..=3)
                    .map(|local| {
                        SemanticOperandV1::Copy(place(
                            local,
                            original.locals()[local as usize].ty(),
                        ))
                    })
                    .collect(),
                Some(SemanticCallDestinationV1::new(place(0, UNIT), edge(target))),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let root = function(
        200,
        SemanticFunctionRoleV1::KernelRoot,
        abi.clone(),
        original.locals()[..4].to_vec(),
        vec![
            block(220, vec![], call(1)),
            block(221, vec![], call(2)),
            block(222, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mut blocks = original.blocks().to_vec();
    let mut tail = blocks[1].statements().to_vec();
    tail.push(assign(
        place(0, UNIT),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
            UNIT,
            SemanticConstantValueV1::ZeroSized,
        ))),
    ));
    blocks[1] = block(223, tail, SemanticTerminatorKindV1::Return);
    let helper = function(
        201,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        original.locals().to_vec(),
        blocks,
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
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

#[test]
fn repeated_descriptor_helpers_keep_global_value_ids_and_distinct_source_invocations() {
    run_production_optimized_consumer_v18(
        repeated_descriptor_helper_owner_v18,
        |original, optimized, budget| {
            let owner = original.source.root_row(0)?;
            assert!(owner.source_slots.slots.is_empty());
            assert!(owner.source_slots.pending_memory.is_some());
            let mut helpers = [usize::MAX; 2];
            let mut helper_count = 0;
            for instance in 0..original.source.instance_count(0, budget)? {
                if original.source.instance(0, instance, budget)?.0.index() == 1 {
                    assert!(original.source.instance_active(0, instance, budget)?);
                    assert!(helper_count < helpers.len());
                    helpers[helper_count] = instance;
                    helper_count += 1;
                }
            }
            assert_eq!(helper_count, 2);
            assert_ne!(helpers[0], helpers[1]);
            let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
            let actual = optimized.output_inventory(budget)?;
            assert_eq!(
                actual.effects()[function.effects.clone()]
                    .iter()
                    .filter(|row| matches!(
                        optimized_source_operation_row_v18(
                            actual,
                            row.coordinate.operation,
                            budget
                        )
                        .unwrap()
                        .operation
                        .kind,
                        OperationKind::Load { .. }
                    ))
                    .count(),
                4
            );
            original.with_optimized_analysis_v18(optimized, budget, |analysis, budget| {
                analysis.with_memory_versions(budget, |input, output, budget| {
                    scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                        original,
                        optimized,
                        0,
                        input,
                        output,
                        budget,
                        |physical, budget| {
                            physical.visit_accesses(budget, |_, _, _, _, _| {
                                panic!("descriptor effects must not become private-slot accesses")
                            })
                        },
                    )
                })
            })
        },
    );
}

fn descriptor_with_private_scalar_owner_v18() -> ProductionSemanticSsaOwnerV1 {
    let base = two_descriptor_reads_at_one_source_site_v18();
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    let mut locals = original.locals().to_vec();
    let temporary = locals.len() as u32;
    locals.push(local(224, U32, SemanticLocalRoleV1::Temporary));
    let mut statements = original.blocks()[1].statements().to_vec();
    statements.push(SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(temporary, U32),
            literal(17),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    let mut blocks = original.blocks().to_vec();
    blocks[1] = block(225, statements, SemanticTerminatorKindV1::Return);
    let root = function(200, original.role(), original.abi().clone(), locals, blocks)
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![root],
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

#[test]
fn descriptor_with_a_real_local_cell_does_not_acquire_empty_domain_currentness() {
    // Without the captured kernel ABI, the external slice remains Generic.
    // Its unknown origin must not be confused with the empty-domain shortcut.
    let (result, _, _, completed) = run_descriptor_local_v18(
        false,
        false,
        false,
        DescriptorLocalFaultV18::None,
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
    );
    assert!(!completed);
    assert!(
        DESCRIPTOR_LOCAL_OBSERVED_V18.get() > 0,
        "the genuine private slot was emitted"
    );
    assert!(
        matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source raw address differs from its actual formation or memory history",
                        ..
                    }
                )
            ))
        ),
        "{result:?}"
    );
}

include!("production_optimized_source_descriptor_local_v18_tests.rs");

#[test]
fn original_memory_report_cannot_certify_the_optimized_endpoint() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(folding_source_owner_v18, &mut budget);
    let rejected = std::cell::Cell::new(false);
    let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let attempted =
            source.with_checked_optimization_v18(budget, |original, optimized, budget| {
                original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                    analyses.with_memory_versions(budget, |input_memory, _, budget| {
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original,
                            optimized,
                            0,
                            input_memory,
                            input_memory,
                            budget,
                            |_, _| -> SourceOwnedResultV18<()> {
                                panic!("foreign output report admitted")
                            },
                        )
                    })
                })?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            });
        rejected.set(attempted.is_err());
        Ok::<_, ProductionSourceOwnedViewErrorV18>(())
    });
    assert!(rejected.get());
    assert!(matches!(
        result,
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized currentness foreign output memory versions"
        ))
    ));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
#[test]
fn actual_optimized_retained_indices_recheck_fresh_output_history_and_guard_edges() {
    for case in [
        RetainedIndexCaseV29::Constant,
        RetainedIndexCaseV29::Mask,
        RetainedIndexCaseV29::Guard,
        RetainedIndexCaseV29::GuardLoop,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = retained_index_prepared_v29(case, &mut budget)
            .unwrap_or_else(|error| panic!("{case:?} preparation: {error:?}"));
        let entered = std::cell::Cell::new(false);
        let completed = std::cell::Cell::new(false);
        with_production_optimized_consumer_v18(prepared, &mut budget, |original, optimized, budget| {
            entered.set(true);
            let floor = budget.storage();
            source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
            let owner = original.source.root_row(1)?;
            let mut relocated = 0;
            for (ordinal, slot) in owner.source_slots.slots.iter().enumerate() {
                let input = scoped_raw_admission_v29::optimized_source_slot_input_v18(original, 1, ordinal, budget)?;
                let backing = original.retained_allocation(1, input, budget)?.unwrap();
                assert_eq!(backing.row, ordinal);
                assert!(std::ptr::eq(backing.slot, slot));
                let allocation = optimized.allocation(1, input, budget)?.unwrap();
                assert_eq!(allocation.instance(), slot.instance.index());
                assert!(allocation.output().is_some());
                let block = optimized_source_block_row_v18(original.inventory, input.block, budget)?.block;
                relocated += usize::from(block.id != slot.allocation.block
                    || input.operation as usize != slot.allocation.operation);
            }
            assert!(relocated > 0, "fixture must transport an actual invocation-local allocation");
            original.with_optimized_analysis_v18(optimized, budget, |analysis, budget| {
                analysis.with_memory_versions(budget, |input, output, budget| {
                    scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                        original, optimized, 1, input, output, budget, |physical, budget| {
                            let actual = optimized.output_inventory(budget)?;
                            let root = optimized_source_root_function_v18(original, optimized, 1, budget)?;
                            assert!(actual.operations()[root.operations.clone()].iter()
                                .any(|row| matches!(row.operation.kind, OperationKind::Execution(_))));
                            let mut retained = 0;
                            physical.visit_accesses(budget, |_, _, before, after, budget| {
                                if let Some((after, _)) = after {
                                    assert!(input.operation(before, budget)
                                        .map_err(|error| ProductionSourceOwnedViewErrorV18::from(
                                            fe2o3_pliron::CanonicalAnalysisScopeErrorV1::MemorySsa(error)))?.is_some());
                                    assert!(output.operation(after, budget)
                                        .map_err(|error| ProductionSourceOwnedViewErrorV18::from(
                                            fe2o3_pliron::CanonicalAnalysisScopeErrorV1::MemorySsa(error)))?.is_some());
                                    retained += 1;
                                }
                                Ok(())
                            })?;
                            assert!(retained >= 2, "{case:?}");
                            completed.set(true);
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        },
                    )
                })
            })
            })?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        }).unwrap_or_else(|error| panic!("{case:?}, entered={}, completed={}: {error:?}",
            entered.get(), completed.get()));
        assert!(completed.get(), "{case:?}");
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn original_allocation_locator_rejects_substituted_attachment_roles_and_instances() {
    for fault in 0..7 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget).unwrap();
        let reached = std::cell::Cell::new(false);
        let result = prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                    let owner = source.root_row(1)?;
                    let (ordinal, slot) = owner.source_slots.slots.iter().enumerate().find(|(_, slot)| {
                        owner.source_slots.slots.iter().any(|other|
                            other.instance == slot.instance && other.origin.pointer != slot.origin.pointer)
                    }).expect("source fixture must have distinct allocations in one invocation");
                    let input = scoped_raw_admission_v29::optimized_source_slot_input_v18(original, 1, ordinal, budget)?;
                    let backing = original.retained_allocation(1, input, budget)?.unwrap();
                    assert_eq!(backing.row, ordinal);
                    let floor = budget.storage();
                    budget.reserve_storage(std::mem::size_of::<Vec<SourceAttachmentV18>>()
                        + std::mem::size_of::<ProductionSourceCorrespondenceV18<'_>>())?;
                    let mut rows = source_attachments_v18(source, inventory, budget)?;
                    let target = rows.iter().position(|row|
                        row.key.root == 1 && row.key.family == TileAttachmentFamilyV29::SourceSlot
                        && row.key.instance == slot.instance.index()
                        && row.key.field == TileAttachmentFieldV29::SlotAllocation
                        && matches!(row.location, TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point))
                            if point.function == input.block.function.0 as usize
                            && point.block == input.block.block as usize && point.operation == input.operation as usize)
                    ).unwrap();
                    let target_key = rows[target].key;
                    match fault {
                        0 | 1 => {
                            let replacement = rows.iter().find(|row| row.key.root == 1
                                && row.key.family == TileAttachmentFamilyV29::SourceSlot
                                && row.key.field == TileAttachmentFieldV29::SlotAllocation
                                && if fault == 0 {
                                    row.key.instance == target_key.instance && row.key.row != target_key.row
                                } else { row.key.instance != target_key.instance }
                            ).expect("source fixture must distinguish allocation ownership").location;
                            assert_ne!(replacement, rows[target].location);
                            rows[target].location = replacement;
                        }
                        2 => rows[target].key.field = TileAttachmentFieldV29::SlotPointer,
                        3 => {
                            let extra = (target + 1) % rows.len();
                            rows[extra] = rows[target];
                            rows[extra].key.part = 1;
                        }
                        4 => rows[target].key.part = 1,
                        5 => {
                            let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(mut point)) = rows[target].location
                                else { unreachable!() };
                            point.function = source.root(0, budget)?.1;
                            rows[target].location = TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point));
                        }
                        6 => rows[target].location = TileAttachmentLocationV29::NoOutput,
                        _ => unreachable!(),
                    }
                    private_array_heapsort_v1(&mut rows, |row| source_attachment_key_v18(row.key),
                        &mut SourceCorrespondenceWorkV18(budget), || ArgumentResourceV1::Arithmetic.into())?;
                    let tampered = ProductionSourceCorrespondenceV18 {
                        source, inventory, attachments: &rows, slot: std::ptr::from_ref(budget) as usize,
                        ledger: budget.work_ledger_identity_v1(), floor: budget.storage(),
                    };
                    let error = scoped_raw_admission_v29::optimized_source_slot_input_v18(&tampered, 1, ordinal, budget)
                        .expect_err("original allocation substitution must refuse");
                    assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)), "fault {fault}");
                    let stopped = budget.work();
                    assert!(original.retained_allocation(1, input, budget).is_err());
                    assert_eq!(budget.work(), stopped, "first locator refusal remains owned");
                    reached.set(true);
                    drop(tampered);
                    drop(rows);
                    budget.release_storage(budget.storage() - floor)?;
                    Err(error)
                })
            }))
        });
        assert!(
            reached.get(),
            "hostile control must follow the exact positive locator"
        );
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Binding(_))
        ));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn original_allocation_locator_resource_run_v18(
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut count = 0;
    let result = (|| {
        let prepared = retained_index_prepared_v29(RetainedIndexCaseV29::Constant, &mut budget)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget| {
                source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                    for root in 0..source.root_count(budget)? {
                        for ordinal in 0..source.root_row(root)?.source_slots.slots.len() {
                            let input = scoped_raw_admission_v29::optimized_source_slot_input_v18(original, root, ordinal, budget)?;
                            assert_eq!(original.retained_allocation(root, input, budget)?.unwrap().row, ordinal);
                            count += 1;
                        }
                    }
                    Ok(())
                })
            }))
        })
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), count)
}

#[test]
fn original_allocation_locator_has_exact_and_one_short_owned_resource_boundaries() {
    let (result, work, storage, count) =
        original_allocation_locator_resource_run_v18(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    assert!(count > 0);
    let (exact, actual_work, actual_storage, actual_count) =
        original_allocation_locator_resource_run_v18(work, storage);
    exact.unwrap();
    assert_eq!(
        (actual_work, actual_storage, actual_count),
        (work, storage, count)
    );
    for (work_limit, storage_limit, expect_work) in
        [(work - 1, storage, true), (work, storage - 1, false)]
    {
        let (short, _, _, _) =
            original_allocation_locator_resource_run_v18(work_limit, storage_limit);
        let error = match short {
            Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => error,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                ),
            )) => error,
            other => panic!("exact original resource denial required: {other:?}"),
        };
        if expect_work {
            assert!(matches!(error, ArgumentResourceV1::Work(_)));
        } else {
            assert!(matches!(error, ArgumentResourceV1::Storage(_)));
        }
    }
}

include!("production_source_allocation_slot_v18_tests.rs");
include!("production_source_private_memory_v18_tests.rs");
include!("production_source_native_private_v18_tests.rs");

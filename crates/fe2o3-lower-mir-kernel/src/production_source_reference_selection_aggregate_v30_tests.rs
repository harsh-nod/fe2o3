fn selected_aggregate_owner_v30() -> ProductionSemanticSsaOwnerV1 {
    let previous = selection_owner(false, false);
    let semantic = previous.source_semantic();
    let mut types = semantic.types().to_vec();
    let pointer = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(declaration(
        201,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let helper = &semantic.functions()[1];
    let mut locals = helper.locals().to_vec();
    let local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([202; 32]),
        pointer,
        SemanticLocalRoleV1::Temporary,
        provenance(),
    ));
    let mut statements = helper.blocks()[0].statements().to_vec();
    // Keep the selected external read's value in a real address-taken scalar
    // object, then feed its read into the selected external store. Promotion
    // must therefore transport a live value, not merely erase unused storage.
    assert!(matches!(
        statements[1].kind(),
        SemanticStatementKindV1::Assign(_)
    ));
    statements.insert(
        2,
        assign(
            local,
            pointer,
            SemanticRvalueKindV1::AddressOf {
                place: place(2, U32),
                mutability: SemanticMutabilityV1::Immutable,
            },
        ),
    );
    let SemanticStatementKindV1::Store(store) = statements[3].kind() else {
        panic!("genuine selected helper Store");
    };
    statements[3] = SemanticStatementV1::new(
        provenance(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            store.destination().clone(),
            SemanticOperandV1::Copy(place(2, U32)),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    let helper = rebuild(
        helper,
        locals,
        vec![block(100, statements, SemanticTerminatorKindV1::Return)],
    );
    admitted_owner(
        types,
        vec![semantic.functions()[0].clone(), helper],
        semantic.callables().to_vec(),
    )
}

#[test]
fn selected_aggregate_fixture_requires_exact_raw_pointer_backend_layout() {
    let owner = selected_aggregate_owner_v30();
    let semantic = owner.source_semantic();
    let mut types = semantic.types().to_vec();
    let pointer = types.last().unwrap();
    assert!(matches!(pointer.shape(), SemanticTypeShapeV1::Pointer(_)));
    let malformed = declaration(
        201,
        SemanticTypeLayoutV1::new(Some(8), 8).unwrap(),
        pointer.shape().clone(),
    );
    *types.last_mut().unwrap() = malformed;
    let result = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        semantic.functions().to_vec(),
        semantic.callables().to_vec(),
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default());
    assert!(
        matches!(result, Err(SemanticMirErrorV1::InvalidTypeLayout)),
        "malformed raw-pointer layout must fail semantic admission: {:?}",
        result.err()
    );
}

fn selected_aggregate_arguments_v30(
    source: &ProductionSourceOwnedViewV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<ProductionKernelArgumentAbiArgumentV18>> {
    use fe2o3_kernel_descriptor::{
        DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, LogicalArgumentV1, ScalarTypeV1,
        SourceTypeDescriptorV1, SourceTypeDescriptorV3, SourceTypeRecordV1, ValidName,
    };
    let semantic = source.source_semantic(budget)?;
    let source_record =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    let layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::U32));
    Ok(["first", "second"]
        .into_iter()
        .enumerate()
        .map(|(ordinal, name)| {
            let ty = semantic.functions()[0].abi().source_input_types()[ordinal];
            assert_eq!(ty, CARRIER);
            ProductionKernelArgumentAbiArgumentV18 {
                semantic_type_identity: semantic.types()[ty.index() as usize].identity(),
                kind: ProductionKernelArgumentAbiKindV18::Descriptor {
                    source: SourceTypeDescriptorV3::DisjointSlice(ScalarTypeV1::U32),
                    argument: LogicalArgumentV1::disjoint_slice(
                        ordinal as u16,
                        ValidName::new(name).unwrap(),
                        &source_record,
                        &layout,
                        fe2o3_kernel_descriptor::AccessMode::ReadWrite,
                        (ordinal * 16) as u32,
                    )
                    .unwrap(),
                },
            }
        })
        .collect())
}

fn with_selected_aggregate_native_v30(
    output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    run: impl FnOnce(
        &fe2o3_pliron::PendingCanonicalSelectedMemoryPoliciesV30<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30>,
) -> Result<(), ProductionAggregateSourceErrorV30> {
    fn refused() -> Result<(), ProductionAggregateSourceErrorV30> {
        Err(ProductionSourceOwnedViewErrorV18::Binding(
            "selected aggregate test native scope refused",
        )
        .into())
    }
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateMemoryLimitsV1, CanonicalRankedMetadataV18, CanonicalRankedViewErrorV1,
        build_canonical_ranked_candidate_v18, check_canonical_kir_private_memory_v18,
        with_checked_canonical_ranked_view_v18,
    };
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, FormalIndexWidth, StorageLayoutLimitsV1,
        with_canonical_selected_slice_domains_v30,
    };
    let metadata = CanonicalRankedMetadataV18::new(output.owner(), &[]);
    let metadata_storage = metadata.storage_extent(budget).unwrap();
    budget.reserve_storage(metadata_storage)?;
    let (candidate, candidate_storage) =
        build_canonical_ranked_candidate_v18(output, &metadata, budget).unwrap();
    budget.reserve_storage(candidate_storage.retained_storage())?;
    let (physical, physical_storage) = check_canonical_kir_private_memory_v18(
        output,
        CanonicalKirPrivateMemoryLimitsV1 { max_cells: 64 },
        budget,
    )
    .unwrap();
    budget.reserve_storage(physical_storage.retained_storage())?;
    let layouts = StorageLayoutLimitsV1 {
        rows: 4096,
        edges: 16384,
        containment_depth: 128,
        object_bytes: 1 << 24,
    };
    let launches = vec![
        ExplicitLaunchExtent::Exact {
            rank: 3,
            extents: [64, 1, 1]
        };
        output.functions().len()
    ];
    let denied = std::cell::Cell::new(false);
    let result = with_checked_canonical_ranked_view_v18(
        output,
        &metadata,
        &candidate,
        budget,
        |checked, budget| {
            Ok::<_, CanonicalRankedViewErrorV1>(
                fe2o3_pliron::with_pending_canonical_ranked_source_roles_v18(
                    checked,
                    layouts,
                    budget,
                    |pending, budget| {
                        Ok(with_canonical_selected_slice_domains_v30(
                            output.owner(),
                            &launches,
                            FormalIndexWidth::Bits64,
                            Default::default(),
                            budget,
                            |domains, budget| {
                                Ok(pending
                                    .with_selected_memory_observations_v30(
                                        &physical,
                                        domains,
                                        budget,
                                        |native, budget| {
                                            let result = run(native, budget);
                                            if native.owner(budget).is_err_and(|error| {
                                                ProductionOptimizedExecutionRecipesV18::policy_resource(&error)
                                                    == Some(ArgumentResourceV1::Accounting)
                                            }) {
                                                denied.set(true);
                                            }
                                            Ok(result)
                                        },
                                    )
                                    .unwrap_or_else(|_| refused()))
                            },
                        )
                        .unwrap_or_else(|_| Some(refused()))
                        .expect("complete selected final domain family"))
                    },
                )
                .unwrap_or_else(|_| refused()),
            )
        },
    )
    .unwrap_or_else(|_| refused());
    drop(physical);
    drop(candidate);
    drop(metadata);
    if !denied.get() {
        budget.release_storage(
            metadata_storage
                + candidate_storage.retained_storage()
                + physical_storage.retained_storage(),
        )?;
    }
    result
}

#[test]
fn selected_aggregate_actual_consumer_joins_nonempty_selected_graph_after_live_private_promotion() {
    let reached = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        selected_aggregate_owner_v30(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            let source = original.source;
            let arguments = selected_aggregate_arguments_v30(source, budget)?;
            let semantic = source.source_semantic(budget)?;
            let binding = *semantic.functions()[0]
                .kernel_entry()
                .unwrap()
                .kernel_binding_identity()
                .as_bytes();
            let roots = [ProductionKernelArgumentAbiRootV18 {
                kernel_binding: &binding,
                export: "issued_pointer_source",
                arguments: &arguments,
                explicit_argument_bytes: 32,
                kernarg_alignment_bytes: 8,
            }];
            let chain = source
                .aggregate_output_v30(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .unwrap();
            let output = chain.output(budget)?;
            assert!(
                output
                    .rounds()
                    .iter()
                    .any(|round| round.aggregate().promoted_allocations() > 0)
            );
            let (inventory, receipt) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(output.owner(), budget)
                    .unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            with_selected_aggregate_native_v30(&inventory, budget, |native, budget| {
                chain.with_selected_aggregate_sources_v30(native, budget, |view, budget| {
                    assert_eq!(view.root_count(budget)?, 1);
                    let (accesses, choices, external, _) = view.counts(0, budget)?;
                    assert_eq!(accesses, 2);
                    assert!(choices >= 4);
                    assert_eq!(external, [1, 1]);
                    assert!(view.memory(budget)?.promotion_count() > 0);
                    assert!(!view.runtime_requirements_are_discharged());
                    assert!(!view.whole_effect_refinement_is_complete());
                    assert!(!view.grants_artifact_or_launch_authority());
                    reached.set(true);
                    Ok(())
                })
            })
            .unwrap();
            drop(inventory);
            budget.release_storage(receipt.retained_storage())?;
            chain.discard(budget)?;
            Ok(())
        },
    );
    result.unwrap();
    assert!(reached.get(), "actual selected aggregate consumer must run");
}

fn selected_aggregate_hostile_v30(fault: u8, native_owner: u8) {
    let native_reached = std::cell::Cell::new(false);
    let consumer_reached = std::cell::Cell::new(false);
    let refusal_checked = std::cell::Cell::new(false);
    let (result, _, _) = run_selected_memory_owner(
        selected_aggregate_owner_v30(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        |original, budget| {
            let source = original.source;
            let arguments = selected_aggregate_arguments_v30(source, budget)?;
            let semantic = source.source_semantic(budget)?;
            let binding = *semantic.functions()[0]
                .kernel_entry()
                .unwrap()
                .kernel_binding_identity()
                .as_bytes();
            let roots = [ProductionKernelArgumentAbiRootV18 {
                kernel_binding: &binding,
                export: "issued_pointer_source",
                arguments: &arguments,
                explicit_argument_bytes: 32,
                kernarg_alignment_bytes: 8,
            }];
            let chain = source
                .aggregate_output_v30(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .unwrap();
            let output = chain.output(budget)?;
            assert!(
                output
                    .rounds()
                    .iter()
                    .any(|round| round.aggregate().promoted_allocations() > 0)
            );
            let foreign = if native_owner == 2 {
                Some(
                    source
                        .aggregate_output_v30(
                            ProductionKernelArgumentAbiInputV18 { roots: &roots },
                            budget,
                        )
                        .unwrap(),
                )
            } else {
                None
            };
            let owner = if native_owner == 1 {
                output.rounds()[0].scalar().owner()
            } else if let Some(foreign) = &foreign {
                let foreign = foreign.output(budget)?.owner();
                assert_eq!(foreign.canonical_bytes(), output.owner().canonical_bytes());
                foreign
            } else {
                output.owner()
            };
            if native_owner != 0 {
                assert!(!std::ptr::eq(owner, output.owner()));
            }
            let (inventory, receipt) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(owner, budget).unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            with_selected_aggregate_native_v30(&inventory, budget, |native, budget| {
                native_reached.set(true);
                let floor = budget.storage();
                let result = if native_owner != 0 {
                    chain.with_selected_aggregate_sources_v30(native, budget, |_, _| {
                        consumer_reached.set(true);
                        Ok(())
                    })
                } else {
                    chain.with_selected_aggregate_sources_fault_v30(
                        native,
                        budget,
                        fault,
                        |_, _| {
                            consumer_reached.set(true);
                            Ok(())
                        },
                    )
                };
                let error =
                    result.expect_err("changed actual stage/owner must refuse before consumption");
                let expected = if native_owner != 0 {
                    "scoped aggregate memory final owner differs"
                } else {
                    match fault {
                        0 | 1 => "selected aggregate stage order or owner differs",
                        2 => "selected aggregate final owner or stage census differs",
                        3 => "selected aggregate final endpoint order differs",
                        _ => unreachable!(),
                    }
                };
                assert!(
                    matches!(&error, ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Binding(detail)) if *detail == expected),
                    "{error:?}"
                );
                assert_eq!(budget.storage(), floor);
                assert!(!source.cleanup.is_denied());
                refusal_checked.set(true);
                Ok(())
            })
            .unwrap();
            drop(inventory);
            budget.release_storage(receipt.retained_storage())?;
            if let Some(foreign) = foreign {
                assert!(foreign.discard(budget).is_err());
            }
            assert!(chain.discard(budget).is_err());
            Ok(())
        },
    );
    assert!(result.is_err());
    assert!(native_reached.get() && refusal_checked.get());
    assert!(!consumer_reached.get());
}

#[test]
fn selected_aggregate_actual_consumer_refuses_missing_duplicate_and_stale_stages() {
    for fault in 0..4 {
        selected_aggregate_hostile_v30(fault, 0);
    }
}

#[test]
fn selected_aggregate_actual_consumer_refuses_first_scalar_owner_substitution() {
    // A legacy first-scalar completion cannot be relabeled as the aggregate
    // final owner. Even genuine selected-native reports for that owner fail.
    // The legacy V26 report type itself is not an accepted argument to this API.
    selected_aggregate_hostile_v30(0, 1);
}

#[test]
fn selected_aggregate_actual_consumer_refuses_foreign_final_chain_with_identical_bytes() {
    selected_aggregate_hostile_v30(0, 2);
}

fn selected_aggregate_paid_payload_v30(budget: &mut ArgumentBudgetV1<'_>) -> Vec<u8> {
    budget.reserve_storage(43).unwrap();
    budget.charge_work(43).unwrap();
    let mut payload = Vec::new();
    payload.try_reserve_exact(43).unwrap();
    budget.reserve_storage(payload.capacity() - 43).unwrap();
    payload.resize(43, 0x30);
    payload
}

fn selected_aggregate_payload_case_v30(disposition: u8) {
    let reached = std::cell::Cell::new(false);
    let retained_floor = std::cell::Cell::new(None);
    let (result, _, _) = scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_owner_v18(
        selected_aggregate_owner_v30(),
        SELECTED_MEMORY_LIMIT,
        SELECTED_MEMORY_LIMIT,
        &retained_floor,
        |original, budget| {
            let source = original.source;
            let arguments = selected_aggregate_arguments_v30(source, budget)?;
            let semantic = source.source_semantic(budget)?;
            let binding = *semantic.functions()[0]
                .kernel_entry()
                .unwrap()
                .kernel_binding_identity()
                .as_bytes();
            let roots = [ProductionKernelArgumentAbiRootV18 {
                kernel_binding: &binding,
                export: "issued_pointer_source",
                arguments: &arguments,
                explicit_argument_bytes: 32,
                kernarg_alignment_bytes: 8,
            }];
            let chain = source
                .aggregate_output_v30(
                    ProductionKernelArgumentAbiInputV18 { roots: &roots },
                    budget,
                )
                .unwrap();
            let (inventory, receipt) = fe2o3_kernel_analysis::CanonicalKirInventoryV18::derive_v18(
                chain.output(budget)?.owner(),
                budget,
            )
            .unwrap();
            budget.reserve_storage(receipt.retained_storage())?;
            let native_result =
                with_selected_aggregate_native_v30(&inventory, budget, |native, budget| {
                    let floor = budget.storage();
                    let diagnostic = std::cell::RefCell::new(None);
                    let poison_floor = std::cell::Cell::new(0);
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        chain.with_selected_aggregate_sources_v30(native, budget, |view, budget| {
                            assert_eq!(view.root_count(budget)?, 1);
                            assert_eq!(view.counts(0, budget)?.0, 2);
                            reached.set(true);
                            match disposition {
                                3 => assert!(view.counts(usize::MAX, budget).is_err()),
                                4 => assert!(native.report(usize::MAX, budget).is_err()),
                                5 => {
                                    let domains = native.selected_domains(budget).unwrap();
                                    assert!(
                                        domains
                                            .function_conditions(
                                                fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                                                    u32::MAX
                                                ),
                                                budget,
                                            )
                                            .is_err()
                                    );
                                }
                                _ => (),
                            }
                            if (3..=5).contains(&disposition) {
                                // Ignoring a failed query cannot restore this
                                // borrowed consumer by returning an ordinary Ok.
                                return Ok(Vec::new());
                            }
                            let payload = selected_aggregate_paid_payload_v30(budget);
                            match disposition {
                                0 => Ok(payload),
                                1 => std::panic::panic_any(payload),
                                2 => {
                                    *diagnostic.borrow_mut() = Some(payload);
                                    Err(ProductionSourceOwnedViewErrorV18::Binding(
                                        "first selected aggregate callback refusal",
                                    )
                                    .into())
                                }
                                6 | 7 => {
                                    if disposition == 6 {
                                        native.refuse_retained_custody();
                                    } else {
                                        native
                                            .selected_domains(budget)
                                            .unwrap()
                                            .refuse_retained_custody();
                                    }
                                    poison_floor.set(budget.storage());
                                    std::panic::panic_any(payload);
                                }
                                _ => unreachable!(),
                            }
                        })
                    }));
                    let payload = match (disposition, caught) {
                        (0, Ok(Ok(payload))) => payload,
                        (1, Err(payload)) => *payload.downcast::<Vec<u8>>().unwrap(),
                        (6 | 7, Err(payload)) => *payload.downcast::<Vec<u8>>().unwrap(),
                        (
                            2,
                            Ok(Err(ProductionAggregateSourceErrorV30::Source(
                                ProductionSourceOwnedViewErrorV18::Binding(
                                    "first selected aggregate callback refusal",
                                ),
                            ))),
                        ) => diagnostic.into_inner().unwrap(),
                        (3..=5, Ok(Err(_))) => Vec::new(),
                        (_, other) => {
                            panic!("wrong selected aggregate payload disposition: {other:?}")
                        }
                    };
                    if disposition >= 6 {
                        assert_eq!(payload, vec![0x30; 43]);
                        assert!(source.cleanup.is_denied());
                        assert_eq!(budget.storage(), poison_floor.get());
                        assert!(budget.storage() > floor + payload.capacity());
                        drop(payload);
                        return Err(ArgumentResourceV1::Accounting.into());
                    }
                    assert_eq!(
                        payload,
                        if disposition < 3 {
                            vec![0x30; 43]
                        } else {
                            Vec::new()
                        }
                    );
                    assert_eq!(budget.storage(), floor + payload.capacity());
                    assert!(!source.cleanup.is_denied());
                    let credit = payload.capacity();
                    drop(payload);
                    budget.release_storage(credit)?;
                    assert_eq!(budget.storage(), floor);
                    Ok(())
                });
            if disposition >= 4 {
                assert!(native_result.is_err());
            } else {
                native_result.unwrap();
            }
            drop(inventory);
            if disposition >= 6 {
                drop(chain);
                retained_floor.set(Some(budget.storage()));
                return Err(ArgumentResourceV1::Accounting.into());
            }
            budget.release_storage(receipt.retained_storage())?;
            if disposition >= 2 {
                assert!(chain.discard(budget).is_err());
            } else {
                chain.discard(budget)?;
            }
            Ok(())
        },
    );
    assert!(reached.get(), "actual selected aggregate callback must run");
    if disposition >= 2 {
        assert!(result.is_err());
    } else {
        result.unwrap();
    }
}

#[test]
fn selected_aggregate_actual_consumer_preserves_paid_success_and_panic_payloads() {
    selected_aggregate_payload_case_v30(0);
    selected_aggregate_payload_case_v30(1);
}

#[test]
fn selected_aggregate_actual_consumer_preserves_owned_error_side_payload() {
    selected_aggregate_payload_case_v30(2);
}

#[test]
fn selected_aggregate_actual_consumer_replays_ignored_view_native_and_domain_refusals() {
    for disposition in 3..=5 {
        selected_aggregate_payload_case_v30(disposition);
    }
}

#[test]
fn selected_aggregate_actual_consumer_panic_samples_native_and_domain_custody_before_refund() {
    selected_aggregate_payload_case_v30(6);
    selected_aggregate_payload_case_v30(7);
}

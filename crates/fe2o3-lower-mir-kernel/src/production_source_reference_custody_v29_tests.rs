use super::*;
use std::cell::Cell;

fn holder_replacement_owner(
    plain_entry: bool,
    field: bool,
    dead_replacement: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::UniqueRead, |types, functions| {
        let holder_type = if field { CAPTURE } else { REFERENCE };
        let outer = fixture_reference(types, holder_type, true, 70);
        let mut statements = vec![
            assign(place(2, WORD), fixture_word(11)),
            assign(place(3, WORD), fixture_word(29)),
            assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(2, WORD),
                },
            ),
        ];
        if field {
            statements.push(assign(
                place(7, CAPTURE),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Move(place(4, REFERENCE))],
                    )
                    .unwrap(),
                ),
            ));
        }
        statements.push(assign(
            place(5, outer),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(if field { 7 } else { 4 }, holder_type),
            },
        ));
        statements.push(assign(
            place(6, REFERENCE),
            if plain_entry {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, REFERENCE)))
            } else {
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(3, WORD),
                }
            },
        ));
        let mut path = vec![(SemanticProjectionKindV1::Dereference, holder_type)];
        if field {
            path.push((SemanticProjectionKindV1::Field(0), REFERENCE));
        }
        statements.push(assign(
            projected(5, &path),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(6, REFERENCE))),
        ));
        // The overwritten loan no longer keeps x live. A stale holder node
        // would incorrectly refuse here or resolve the following read to x.
        statements.push(dead(2));
        if dead_replacement {
            statements.push(dead(3));
        }
        path.push((SemanticProjectionKindV1::Dereference, WORD));
        statements.push(assign(
            place(8, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(5, &path))),
        ));
        statements.extend([dead(5), dead(if field { 7 } else { 4 }), unit()]);
        let reference_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([10; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::GpuKernel,
            SemanticExternAbiV1::GpuKernel,
            false,
            false,
            1,
            vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                REFERENCE,
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, true, false, false, true),
                        SemanticAbiExtensionV1::None,
                        8,
                        Some(8),
                    )
                    .unwrap(),
                ),
            ))],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow])
        .unwrap();
        let original = &functions[0];
        functions[0] = SemanticFunctionDeclV1::new(
            original.identity(),
            original.role(),
            original.item_definition_identity(),
            original.monomorphization_identity(),
            original.generic_type_arguments_identity(),
            original.const_generic_arguments_identity(),
            source(),
            reference_abi,
            vec![
                local(10, UNIT, SemanticLocalRoleV1::Return),
                local(11, REFERENCE, SemanticLocalRoleV1::Argument(0)),
                local(12, WORD, SemanticLocalRoleV1::Temporary),
                local(13, WORD, SemanticLocalRoleV1::Temporary),
                local(14, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(15, outer, SemanticLocalRoleV1::Temporary),
                local(16, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(17, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(18, WORD, SemanticLocalRoleV1::Temporary),
            ],
            SemanticBlockIdV1::from_index(0),
            vec![block(10, statements, SemanticTerminatorKindV1::Return)],
        )
        .unwrap()
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    })
}

#[test]
fn source_reference_projected_holder_replacement_updates_exact_referent_and_liveness() {
    for field in [false, true] {
        run_owner(
            holder_replacement_owner(false, field, false),
            |plan, budget| {
                assert_eq!(plan.loans.len(), 3);
                let old = &plan.loans[0];
                let outer = &plan.loans[1];
                let replacement = &plan.loans[2];
                assert_eq!(plan.origins[old.origin].local.index(), 2);
                assert_eq!(old.effects.referent_reads, 0);
                assert_eq!(plan.origins[replacement.origin].local.index(), 3);
                assert_eq!(replacement.effects.referent_reads, 1);
                assert_eq!(
                    replacement.representation,
                    SourceReferenceRepresentationV29::StableReferent
                );
                assert_eq!(outer.effects.referent_writes, 1);
                assert_eq!(
                    outer.representation,
                    SourceReferenceRepresentationV29::NeedsAddressable(
                        SourceReferenceCellNeedV29::ReferentWrite,
                    )
                );
                assert!(plan.require_promoted(1, budget).is_err());
                Ok(())
            },
        )
        .unwrap();
        assert!(matches!(
            run_owner(holder_replacement_owner(false, field, true), |_, _| {
                panic!("replacement referent is still borrowed")
            }),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference referent storage dies with a live loan",
                ..
            })
        ));
    }
}

#[test]
fn source_reference_plain_entry_reference_replacement_cannot_reuse_stale_loan() {
    for field in [false, true] {
        assert!(matches!(
            run_owner(holder_replacement_owner(true, field, false), |_, _| {
                panic!("entry reference has no checked dereference origin")
            }),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference dereference has no checked origin",
                ..
            })
        ));
    }
}

fn fixture_reference(
    types: &mut Vec<SemanticTypeDeclV1>,
    pointee: SemanticTypeIdV1,
    mutable: bool,
    tag: u8,
) -> SemanticTypeIdV1 {
    let ty = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                SemanticPointerKindV1::Reference,
                if mutable {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    ty
}

fn nested_mutation_owner(
    reborrow: bool,
    field_before: bool,
    field_after: bool,
    shared_outer: bool,
    bypass_outer: bool,
) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::UniqueRead, |types, functions| {
        let pair = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([44; 32]),
            SemanticLayoutIdentityV1::from_sha256([44; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![WORD, WORD]).unwrap()),
        ));
        let pointee = if field_after { pair } else { WORD };
        let inner = if field_after {
            fixture_reference(types, pair, true, 45)
        } else {
            REFERENCE
        };
        let outer_pointee = if field_before { CAPTURE } else { inner };
        let outer = fixture_reference(types, outer_pointee, !shared_outer, 46);
        let referent = if field_after {
            place(7, pair)
        } else {
            place(1, WORD)
        };
        let mut statements = Vec::new();
        if field_after {
            statements.push(assign(
                place(7, pair),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            SemanticOperandV1::Copy(place(1, WORD)),
                            SemanticOperandV1::Copy(place(1, WORD)),
                        ],
                    )
                    .unwrap(),
                ),
            ));
        }
        statements.push(assign(
            place(2, inner),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: referent,
            },
        ));
        if field_before {
            statements.push(assign(
                place(6, CAPTURE),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Move(place(2, inner))],
                    )
                    .unwrap(),
                ),
            ));
        }
        statements.push(assign(
            place(3, outer),
            SemanticRvalueKindV1::Borrow {
                kind: if shared_outer {
                    SemanticBorrowKindV1::Shared
                } else {
                    SemanticBorrowKindV1::Mutable
                },
                place: if field_before {
                    place(6, CAPTURE)
                } else {
                    place(2, inner)
                },
            },
        ));
        let mut projections = vec![(SemanticProjectionKindV1::Dereference, outer_pointee)];
        if field_before {
            projections.push((SemanticProjectionKindV1::Field(0), inner));
        }
        projections.push((SemanticProjectionKindV1::Dereference, pointee));
        if field_after {
            projections.push((SemanticProjectionKindV1::Field(1), WORD));
        }
        let target = if bypass_outer {
            projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)])
        } else {
            projected(3, &projections)
        };
        let target = if reborrow {
            statements.push(assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: target,
                },
            ));
            projected(4, &[(SemanticProjectionKindV1::Dereference, WORD)])
        } else {
            target
        };
        statements.push(assign(target, fixture_word(17)));
        if reborrow {
            statements.push(dead(4));
        }
        statements.extend([dead(3), dead(if field_before { 6 } else { 2 }), unit()]);
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, inner, SemanticLocalRoleV1::Temporary),
                local(23, outer, SemanticLocalRoleV1::Temporary),
                local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(25, WORD, SemanticLocalRoleV1::Temporary),
                local(26, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(27, pair, SemanticLocalRoleV1::Temporary),
            ],
            vec![block(20, statements, SemanticTerminatorKindV1::Return)],
        );
    })
}

#[test]
fn source_reference_nested_write_and_reborrow_resolve_innermost_origin_and_field_suffix() {
    for reborrow in [false, true] {
        for (before, after) in [(false, false), (true, false), (false, true)] {
            run_owner(
                nested_mutation_owner(reborrow, before, after, false, false),
                |plan, budget| {
                    let per_call = if reborrow { 3 } else { 2 };
                    assert_eq!(plan.loans.len(), 2 * per_call);
                    for first in [0, per_call] {
                        let inner = &plan.loans[first];
                        let outer = &plan.loans[first + 1];
                        assert_eq!(inner.effects.referent_writes, 1);
                        assert_eq!(
                            outer.effects.referent_writes, 0,
                            "nested write does not replace intermediate reference metadata"
                        );
                        assert_eq!(outer.effects.referent_reads, 1);
                        assert!(plan.require_promoted(first, budget).is_err());
                        if reborrow {
                            let child = &plan.loans[first + 2];
                            let origin = &plan.origins[child.origin];
                            assert_eq!(child.parent, Some(first));
                            assert_eq!(origin.instance, plan.origins[inner.origin].instance);
                            assert_eq!(origin.local.index(), if after { 7 } else { 1 });
                            assert_eq!(origin.ty, WORD);
                            let projections = &plan.projections[origin.projections.clone()];
                            assert_eq!(projections.len(), usize::from(after));
                            if after {
                                assert_eq!(
                                    projections[0].kind(),
                                    SemanticProjectionKindV1::Field(1)
                                );
                            }
                            assert!(projections.iter().all(|projection| projection.kind()
                                != SemanticProjectionKindV1::Dereference));
                            assert_eq!(child.effects.referent_writes, 1);
                            assert!(plan.require_promoted(first + 2, budget).is_err());
                        }
                    }
                    Ok(())
                },
            )
            .unwrap();
        }
    }
}

#[test]
fn source_reference_nested_write_and_reborrow_cannot_widen_an_intermediate_shared_loan() {
    for reborrow in [false, true] {
        for fields in [false, true] {
            assert!(matches!(
                run_owner(
                    nested_mutation_owner(reborrow, fields, false, true, false),
                    |_, _| panic!("shared path must refuse")
                ),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference traversal widens mutability",
                    ..
                })
            ));
        }
    }
}

#[test]
fn source_reference_ordered_loads_are_not_scalar_snapshots() {
    for atomic in [false, true] {
        let owner = owner_with(Case::Shared, |_, functions| {
            functions[2] = function(
                30,
                false,
                CAPTURE,
                vec![
                    local(30, UNIT, SemanticLocalRoleV1::Return),
                    local(31, CAPTURE, SemanticLocalRoleV1::Argument(0)),
                    local(32, WORD, SemanticLocalRoleV1::Temporary),
                ],
                vec![block(
                    30,
                    vec![
                        assign(
                            place(2, WORD),
                            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                projected(
                                    1,
                                    &[
                                        (SemanticProjectionKindV1::Field(0), REFERENCE),
                                        (SemanticProjectionKindV1::Dereference, WORD),
                                    ],
                                ),
                                if atomic {
                                    SemanticVolatilityV1::NonVolatile
                                } else {
                                    SemanticVolatilityV1::Volatile
                                },
                                atomic.then_some(SemanticAtomicAccessV1::new(
                                    SemanticAtomicOrderingV1::Acquire,
                                    SemanticAtomicScopeV1::Device,
                                )),
                            )),
                        ),
                        unit(),
                    ],
                    SemanticTerminatorKindV1::Return,
                )],
            );
        });
        assert!(matches!(
            run_owner(owner, |_, _| panic!(
                "ordered load must retain addressable effects"
            )),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference ordered load requires checked addressable effects",
                ..
            })
        ));
    }
}

#[test]
fn source_reference_large_aligned_result_prepays_both_live_slots() {
    use std::mem::size_of;
    #[repr(align(256))]
    struct Large<'a> {
        bytes: [u8; 4096],
        dropped: &'a Cell<usize>,
    }
    impl Drop for Large<'_> {
        fn drop(&mut self) {
            assert_eq!(self.bytes[0], 17);
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    let expected = size_of::<SourceReferenceBuilderV29<'_, '_, '_>>()
        + size_of::<SourceReferencePlanV29<'_, '_>>()
        // The common custody body, promoted/cell wrappers and original-demand entry.
        + 5 * size_of::<Result<Large<'_>, ProductionSemanticKirErrorV1>>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
        + size_of::<Result<(), Box<dyn std::any::Any + Send>>>()
        + size_of::<Vec<usize>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * size_of::<usize>()
        + size_of::<Option<ProductionSemanticKirErrorV1>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootCheckpointV29<'_, '_>>>()
        + size_of::<Result<source_storage_v29::SourceStorageRootCheckpointV29<'_, '_>, ProductionSemanticKirErrorV1>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootArenaV29<'_, '_>>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootV29<'_, '_, '_>>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootRefundV29<'_, '_>>>()
        + size_of::<(Option<ProductionSemanticKirErrorV1>, Result<source_storage_v29::SourceStorageRootRefundV29<'_, '_>, ProductionSemanticKirErrorV1>)>()
        + size_of::<Result<Large<'_>, source_storage_v29::SourceStorageRootCallbackErrorV29<'_>>>()
        + size_of::<Result<Result<Large<'_>, source_storage_v29::SourceStorageRootCallbackErrorV29<'_>>, Box<dyn std::any::Any + Send>>>()
        + size_of::<usize>()
        + size_of::<bool>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
        + size_of::<std::panic::AssertUnwindSafe<[Option<Box<dyn std::any::Any + Send>>; 2]>>()
        + 2 * size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<std::panic::AssertUnwindSafe<Box<dyn std::any::Any + Send>>>()
        + 2 * size_of::<Result<(), Box<dyn std::any::Any + Send>>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    assert_eq!(
        source_reference_headers_v29::<Large<'_>>().unwrap(),
        expected
    );
    with_instances(|instances, _| {
        for short in [false, true] {
            // Existing entry work plus one initial drop and four payload retries.
            let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
            let mut budget =
                ArgumentBudgetV1::new(&mut work, CALLER_FLOOR + expected - usize::from(short));
            budget.reserve_storage(CALLER_FLOOR).unwrap();
            let result: Result<Large<'_>, _> =
                with_source_reference_plan_v29(instances, &mut budget, |_, _| {
                    panic!("must deny before constructing R")
                });
            assert!(result.is_err());
            assert_eq!(budget.storage(), CALLER_FLOOR);
            assert_eq!(
                budget.peak_storage(),
                if short {
                    CALLER_FLOOR
                } else {
                    CALLER_FLOOR + expected
                }
            );
            assert_eq!(
                budget.failed_storage(),
                short.then_some(CALLER_FLOOR + expected)
            );
        }
        let dropped = Cell::new(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
        budget.reserve_storage(CALLER_FLOOR).unwrap();
        let result = with_source_reference_plan_v29(instances, &mut budget, |_, budget| {
            budget.release_storage(budget.storage() - CALLER_FLOOR)?;
            Ok(Large {
                bytes: [17; 4096],
                dropped: &dropped,
            })
        });
        assert!(result.is_err());
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), CALLER_FLOOR);
    });
}

const CALLER_FLOOR: usize = 1 << 20;

#[test]
fn source_reference_direct_holder_access_cannot_bypass_unique_outer_loan() {
    for reborrow in [false, true] {
        assert!(matches!(
            run_owner(
                nested_mutation_owner(reborrow, false, false, false, true),
                |_, _| panic!("outer loan must protect its holder")
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 1,
                block: Some(0),
                statement: Some(2),
                detail: "source reference access bypasses a live loan",
            })
        ));
    }
}

#[test]
fn source_reference_direct_shared_reads_remain_compatible_with_shared_loan() {
    let owner = owner_with(Case::Shared, |_, functions| {
        let mut statements = join_capture(1, SemanticBorrowKindV1::Shared);
        statements.push(assign(
            place(5, WORD),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, WORD))),
        ));
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(24, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(25, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    20,
                    statements,
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 1),
                ),
                block(21, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    });
    run_owner(owner, |plan, budget| {
        assert_eq!(plan.loans.len(), 2);
        for loan in 0..plan.loans.len() {
            assert_eq!(
                plan.require_promoted(loan, budget)?,
                SourceReferenceRepresentationV29::StableReferent
            );
        }
        Ok(())
    })
    .unwrap();
}

fn with_instances<R>(
    consume: impl for<'source, 'work> FnOnce(
        &ExecutionInstancesV29<'source>,
        &mut ArgumentBudgetV1<'work>,
    ) -> R,
) -> R {
    let owner = owner(Case::Shared);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(consume(
                instances, budget,
            ))
        },
    )
    .unwrap()
}

#[test]
fn source_reference_original_demand_lens_requires_the_live_storage_owner() {
    with_instances(|instances, budget| {
        let demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(
            instances.owner(), budget,
        ).unwrap();
        let lens = demands.root_lens(instances.owner(), 0, budget).unwrap();
        let floor = budget.storage();
        let reached = Cell::new(false);
        let result = with_source_reference_descriptor_demands_scope_v29(
            instances, SourceReferenceStorageV29::ScalarCells, None, None,
            Some(lens), budget, |_, _, _| {
                reached.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(!reached.get());
        assert_eq!(budget.storage(), floor);
        demands.discard(budget).unwrap();
    });
}

#[test]
fn source_reference_original_demand_lens_preserves_borrowed_requests_and_write_nodes() {
    with_instances(|instances, budget| {
        let demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(
            instances.owner(), budget,
        ).unwrap();
        let lens = demands.root_lens(instances.owner(), 0, budget).unwrap();
        let (original_requests, original_paths) = lens.requests(instances, budget).unwrap();
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            instances.owner(), demands.types(instances.owner(), budget).unwrap(),
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(), budget,
        ).unwrap();
        let credit = layouts.capture_emission_credit(instances.owner(), budget).unwrap();
        with_source_reference_descriptor_demands_scope_v29(
            instances, SourceReferenceStorageV29::ScalarCells, Some(&mut layouts), None,
            Some(lens), budget, |plan, root, budget| {
                assert!(root.is_some());
                let retained = plan.storage_demands.expect("original borrowed demand owner");
                let (requests, paths) = retained.requests(plan.instances, budget)?;
                assert!(std::ptr::eq(requests, original_requests));
                assert!(std::ptr::eq(paths, original_paths));
                assert!(!plan.representation_demands.is_empty());
                let mut indexed = 0;
                for (key, indices) in &plan.representation_demand_sites {
                    let borrowed: &SourceReferenceAccessIndexKeyV29 = key.as_ref();
                    let (original_key, original_indices) =
                        plan.representation_demand_sites.get_key_value(borrowed).unwrap();
                    assert!(std::ptr::eq(original_key.as_ref(), borrowed));
                    assert!(std::ptr::eq(original_indices, indices));
                    let mut foreign = *borrowed;
                    foreign.0 = usize::MAX;
                    assert!(plan.representation_demand_sites.get(&foreign).is_none());
                    indexed += indices.len();
                    for (offset, &index) in indices.iter().enumerate() {
                        let row = &plan.representation_demands[index];
                        assert!(row.node < plan.nodes.len());
                        let declaration = plan.instances.instance(row.instance).unwrap().declaration();
                        assert!((row.local.index() as usize) < declaration.locals().len());
                        assert!(row.projections.end <= plan.projections.len());
                        for &prior in &indices[..offset] {
                            let prior = &plan.representation_demands[prior];
                            assert!(prior.instance != row.instance
                                || prior.local != row.local
                                || prior.generation != row.generation
                                || prior.node != row.node
                                || prior.selector_source != row.selector_source
                                || plan.projections[prior.projections.clone()]
                                    != plan.projections[row.projections.clone()]);
                        }
                    }
                }
                assert_eq!(indexed, plan.representation_demands.len());
                Ok(())
            },
        ).unwrap();
        let extra = credit.into_root_credit(&layouts, budget).unwrap();
        assert!(layouts.permits_root_emission_refund(instances.owner(), extra, budget));
        budget.release_storage(extra).unwrap();
        layouts.release(budget).unwrap();
        demands.discard(budget).unwrap();
    });
}

#[test]
fn source_reference_original_demand_lens_rejects_foreign_source_and_budget() {
    for foreign_ledger in [false, true] {
        with_instances(|instances, budget| {
            let foreign_owner = owner(Case::Shared);
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign_budget = ArgumentBudgetV1::new(&mut foreign_work, usize::MAX);
            let actual_demands = source_storage_demands_v29::SourceStorageDemandsV29::collect(
                instances.owner(), budget,
            ).unwrap();
            let foreign_demands = if foreign_ledger {
                source_storage_demands_v29::SourceStorageDemandsV29::collect(
                    instances.owner(), &mut foreign_budget)
            } else {
                source_storage_demands_v29::SourceStorageDemandsV29::collect(&foreign_owner, budget)
            }.unwrap();
            let lens = if foreign_ledger {
                foreign_demands.root_lens(instances.owner(), 0, &mut foreign_budget)
            } else {
                foreign_demands.root_lens(&foreign_owner, 0, budget)
            }.unwrap();
            let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
                instances.owner(), actual_demands.types(instances.owner(), budget).unwrap(),
                ProductionSemanticKirLimitsV1::default().storage_layout_limits(), budget,
            ).unwrap();
            let reached = Cell::new(false);
            let result = with_source_reference_descriptor_demands_scope_v29(
                instances, SourceReferenceStorageV29::ScalarCells, Some(&mut layouts), None,
                Some(lens), budget, |_, _, _| {
                    reached.set(true);
                    Ok(())
                },
            );
            assert!(result.is_err());
            assert!(!reached.get());
        });
    }
}

struct DropCount<'a> {
    drops: &'a Cell<usize>,
    panics: bool,
}
impl Drop for DropCount<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        if self.panics {
            panic!("rejected result destructor");
        }
    }
}

#[test]
fn source_reference_custody_undercut_never_refunds_caller_credits() {
    with_instances(|instances, outer| {
        for mode in 0..4 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
            budget.reserve_storage(CALLER_FLOOR).unwrap();
            let drops = Cell::new(0);
            let result = with_source_reference_plan_v29(instances, &mut budget, |_, budget| {
                let owned = budget.storage() - CALLER_FLOOR;
                assert!(owned > 0 && owned <= CALLER_FLOOR);
                budget.release_storage(owned).unwrap();
                match mode {
                    0 | 3 => Ok(DropCount {
                        drops: &drops,
                        panics: mode == 3,
                    }),
                    1 => Err(source_reference_error_v29("first callback failure")),
                    _ => panic!("callback after retained floor loss"),
                }
            });
            assert!(result.is_err());
            if mode == 1 {
                assert!(matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "first callback failure",
                        ..
                    })
                ));
            }
            assert_eq!(drops.get(), usize::from(mode == 0 || mode == 3));
            assert_eq!(budget.storage(), CALLER_FLOOR);
        }
        assert!(outer.storage() > 0);
    });
}

#[test]
fn source_reference_custody_extra_output_storage_stays_owned_and_panics_recover() {
    with_instances(|instances, budget| {
        let floor = budget.storage();
        let returned = with_source_reference_plan_v29(instances, budget, |_, budget| {
            budget.reserve_storage(37)?;
            Ok(17_u32)
        })
        .unwrap();
        assert_eq!(returned, 17);
        assert_eq!(budget.storage(), floor + 37);
        budget.release_storage(37).unwrap();
        for panics in [false, true] {
            let result: Result<(), _> =
                with_source_reference_plan_v29(instances, budget, |_, _| {
                    if panics {
                        panic!("ordinary callback panic");
                    }
                    Err(source_reference_error_v29("first callback failure"))
                });
            assert!(
                matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == if panics { "source reference callback panicked" } else { "first callback failure" })
            );
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn source_reference_custody_panic_payload_drop_precedes_recovery_even_if_it_panics() {
    struct Payload {
        dropped: std::sync::Arc<std::sync::atomic::AtomicUsize>,
        depth: usize,
    }
    impl Drop for Payload {
        fn drop(&mut self) {
            self.dropped
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if self.depth != 0 {
                std::panic::panic_any(Payload {
                    dropped: self.dropped.clone(),
                    depth: self.depth - 1,
                });
            }
        }
    }
    with_instances(|instances, budget| {
        for depth in 0..3 {
            let dropped = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let floor = budget.storage();
            let result: Result<(), _> =
                with_source_reference_plan_v29(instances, budget, |_, _| {
                    std::panic::panic_any(Payload {
                        dropped: dropped.clone(),
                        depth,
                    });
                });
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference callback panicked",
                    ..
                })
            ));
            assert_eq!(dropped.load(std::sync::atomic::Ordering::SeqCst), depth + 1);
            assert_eq!(budget.storage(), floor);
        }
    });
}

#[test]
fn source_reference_custody_rejects_budget_moves_foreign_ledgers_and_ignored_errors() {
    with_instances(|instances, _| {
        for move_original in [false, true] {
            let mut first_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut second_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut budget = ArgumentBudgetV1::new(&mut first_work, usize::MAX);
            let mut other = ArgumentBudgetV1::new(&mut second_work, usize::MAX);
            budget.reserve_storage(CALLER_FLOOR).unwrap();
            let result = with_source_reference_plan_v29(instances, &mut budget, |plan, budget| {
                let full = budget.storage();
                other.reserve_storage(full)?;
                let accepted_work = budget.work();
                let foreign_work = other.work();
                std::mem::swap(budget, &mut other);
                let query = if move_original {
                    &mut other
                } else {
                    &mut *budget
                };
                assert!(plan.loan_at(plan.loans[0].site, query).is_err());
                assert_eq!(
                    other.work(),
                    accepted_work,
                    "wrong slot never charges original work"
                );
                assert_eq!(
                    budget.work(),
                    foreign_work,
                    "foreign ledger is never charged"
                );
                std::mem::swap(budget, &mut other);
                other.release_storage(full)?;
                assert!(
                    plan.loan_at(plan.loans[0].site, budget).is_err(),
                    "custody failure is sticky"
                );
                Ok(())
            });
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!(budget.storage(), CALLER_FLOOR);
            assert_eq!(other.storage(), 0);
        }
    });
}

#[test]
fn source_reference_custody_retains_foreign_ledger_without_query_or_cleanup() {
    with_instances(|instances, _| {
        let mut first_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut second_work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut first_work, usize::MAX);
        let mut other = ArgumentBudgetV1::new(&mut second_work, usize::MAX);
        budget.reserve_storage(CALLER_FLOOR).unwrap();
        other.reserve_storage(23).unwrap();
        let mut original_full = 0;
        let drops = Cell::new(0);
        let result = with_source_reference_plan_v29(instances, &mut budget, |_, budget| {
            original_full = budget.storage();
            std::mem::swap(budget, &mut other);
            Ok(DropCount {
                drops: &drops,
                panics: false,
            })
        });
        assert!(result.is_err());
        assert_eq!(drops.get(), 1);
        assert_eq!((budget.storage(), other.storage()), (23, original_full));
        assert_eq!(budget.work(), 0);
        // The test owns both budgets; only it may recover this abandoned scope.
        other.release_storage(original_full - CALLER_FLOOR).unwrap();
    });
}

#[test]
fn source_reference_custody_preserves_first_query_failure_and_storage_denial() {
    with_instances(|instances, budget| {
        let floor = budget.storage();
        let result = with_source_reference_plan_v29(instances, budget, |plan, budget| {
            assert!(budget.reserve_storage(usize::MAX).is_err());
            let denied = budget.failed_storage();
            assert!(budget.reserve_storage(usize::MAX).is_err());
            assert_eq!(budget.failed_storage(), denied);
            budget.charge_work(usize::MAX - budget.work())?;
            assert!(plan.loan_at(plan.loans[0].site, budget).is_err());
            Err::<(), _>(source_reference_error_v29("later callback failure"))
        });
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(_)
                )
            )
        ));
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn source_reference_fixed_header_exact_and_one_short_are_source_derived() {
    use std::mem::size_of;
    let expected = size_of::<SourceReferenceBuilderV29<'_, '_, '_>>()
        + size_of::<SourceReferencePlanV29<'_, '_>>()
        // The common custody body plus the promoted/cell entry wrappers.
        + 4 * size_of::<Result<(), ProductionSemanticKirErrorV1>>()
        // The compatibility entry delegates to the original-demand-aware scope.
        + size_of::<Result<(), ProductionSemanticKirErrorV1>>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
        + size_of::<Result<(), Box<dyn std::any::Any + Send>>>()
        + size_of::<Vec<usize>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * size_of::<usize>()
        + size_of::<Option<ProductionSemanticKirErrorV1>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootCheckpointV29<'_, '_>>>()
        + size_of::<Result<source_storage_v29::SourceStorageRootCheckpointV29<'_, '_>, ProductionSemanticKirErrorV1>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootArenaV29<'_, '_>>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootCustodyViewV29<'_, '_>>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootV29<'_, '_, '_>>>()
        + size_of::<Option<source_storage_v29::SourceStorageRootRefundV29<'_, '_>>>()
        + size_of::<(Option<ProductionSemanticKirErrorV1>, Result<source_storage_v29::SourceStorageRootRefundV29<'_, '_>, ProductionSemanticKirErrorV1>)>()
        + size_of::<Result<(), source_storage_v29::SourceStorageRootCallbackErrorV29<'_>>>()
        + size_of::<Result<Result<(), source_storage_v29::SourceStorageRootCallbackErrorV29<'_>>, Box<dyn std::any::Any + Send>>>()
        + size_of::<usize>()
        + size_of::<bool>()
        + size_of::<[Option<Box<dyn std::any::Any + Send>>; 2]>()
        + size_of::<std::panic::AssertUnwindSafe<[Option<Box<dyn std::any::Any + Send>>; 2]>>()
        + 2 * size_of::<Box<dyn std::any::Any + Send>>()
        + size_of::<std::panic::AssertUnwindSafe<Box<dyn std::any::Any + Send>>>()
        + 2 * size_of::<Result<(), Box<dyn std::any::Any + Send>>>()
        + size_of::<std::ops::Range<usize>>()
        + size_of::<usize>()
        + size_of::<bool>();
    assert_eq!(source_reference_headers_v29::<()>().unwrap(), expected);
    with_instances(|instances, _| {
        for short in [false, true] {
            // Existing entry work plus one initial drop and four payload retries.
            let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
            let limit = CALLER_FLOOR + expected - usize::from(short);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            budget.reserve_storage(CALLER_FLOOR).unwrap();
            let result: Result<(), _> =
                with_source_reference_plan_v29(instances, &mut budget, |_, _| {
                    panic!("construction must be denied")
                });
            assert!(result.is_err());
            assert_eq!(budget.storage(), CALLER_FLOOR);
            assert_eq!(
                budget.peak_storage(),
                if short {
                    CALLER_FLOOR
                } else {
                    CALLER_FLOOR + expected
                }
            );
            assert_eq!(
                budget.failed_storage(),
                short.then_some(CALLER_FLOOR + expected)
            );
        }
    });
}

fn nested_owner(dead_referent: bool, load: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |types, functions| {
        let reference = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([43; 32]),
            SemanticLayoutIdentityV1::from_sha256([43; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    CAPTURE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        let source = projected(
            4,
            &[
                (SemanticProjectionKindV1::Dereference, CAPTURE),
                (SemanticProjectionKindV1::Field(0), REFERENCE),
            ],
        );
        let read = if load {
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                source,
                SemanticVolatilityV1::NonVolatile,
                None,
            ))
        } else {
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source))
        };
        let mut statements = join_capture(1, SemanticBorrowKindV1::Shared);
        statements.extend([
            assign(
                place(4, reference),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Shared,
                    place: place(3, CAPTURE),
                },
            ),
            assign(place(5, REFERENCE), read),
            dead(4),
            dead(3),
        ]);
        if dead_referent {
            statements.push(dead(1));
        }
        statements.push(statement(SemanticStatementKindV1::StorageLive(
            SemanticLocalIdV1::from_index(3),
        )));
        statements.push(assign(
            place(3, CAPTURE),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![SemanticOperandV1::Move(place(5, REFERENCE))],
                )
                .unwrap(),
            ),
        ));
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(24, reference, SemanticLocalRoleV1::Temporary),
                local(25, REFERENCE, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    20,
                    statements,
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 1),
                ),
                block(21, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    })
}

#[test]
fn source_reference_nested_holder_extraction_preserves_loan_through_callee_use() {
    for load in [false, true] {
        run_owner(nested_owner(false, load), |plan, _| {
            assert_eq!(plan.loans.len(), 4);
            for loan in plan
                .loans
                .iter()
                .filter(|loan| plan.origins[loan.origin].ty == WORD)
            {
                assert_eq!(loan.effects.referent_reads, 1);
                assert_eq!(
                    loan.representation,
                    SourceReferenceRepresentationV29::StableReferent
                );
            }
            Ok(())
        })
        .unwrap();
        assert!(matches!(
            run_owner(nested_owner(true, load), |_, _| panic!(
                "inner loan must remain live"
            )),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference referent storage dies with a live loan",
                ..
            })
        ));
    }
}

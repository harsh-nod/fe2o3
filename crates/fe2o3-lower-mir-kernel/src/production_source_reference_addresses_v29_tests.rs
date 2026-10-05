#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AddressFlow {
    Read,
    BeforeInitialization,
    UninitializedRead,
    RestartFresh,
    RestartStale,
    InertExpiredCopy,
    BranchGenerations,
    BranchExpired,
    LoopStale,
    LoopFresh,
    SharedWrite,
    LiveReferenceKill,
    DeadReferenceKill,
    LaterLiveLoan,
    DeadLaterLoan,
    ReferenceCast,
}

fn address_literal(value: u128) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
    ))
}

fn address_owner(flow: AddressFlow) -> ProductionSemanticSsaOwnerV1 {
    let original = lifecycle_owner(false);
    let semantic = original.source_semantic();
    let mut types = semantic.types()[..2].to_vec();
    let mutability = if flow == AddressFlow::SharedWrite {
        SemanticMutabilityV1::Immutable
    } else {
        SemanticMutabilityV1::Mutable
    };
    let raw = reference(&mut types, U32, mutability, true);
    let mut locals = vec![
        local(221, UNIT, SemanticLocalRoleV1::Return),
        local(222, U32, SemanticLocalRoleV1::Argument(0)),
        local(223, U32, SemanticLocalRoleV1::Temporary),
        local(224, raw, SemanticLocalRoleV1::Temporary),
        local(225, raw, SemanticLocalRoleV1::Temporary),
        local(226, U32, SemanticLocalRoleV1::Temporary),
    ];
    let live = |id| {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(id)),
        )
    };
    let dead = |id| {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(id)),
        )
    };
    let initialize = || assign(place(2, U32), SemanticRvalueKindV1::Use(address_literal(7)));
    let address = || {
        assign(
            place(3, raw),
            SemanticRvalueKindV1::AddressOf {
                mutability,
                place: place(2, U32),
            },
        )
    };
    let copy = || {
        assign(
            place(4, raw),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(3, raw))),
        )
    };
    let target = |id| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(id),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    let read = |id| {
        assign(
            place(5, U32),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                target(id),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )
    };
    let write = |id| {
        SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                target(id),
                address_literal(9),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        )
    };
    let goto = |id| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(id),
        ))
    };
    let choose = |left, right| SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(1, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(left),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(right),
            ),
        )
        .unwrap(),
    };
    let mut setup = vec![live(2)];
    if !matches!(
        flow,
        AddressFlow::BeforeInitialization | AddressFlow::UninitializedRead
    ) {
        setup.push(initialize());
    }
    if matches!(
        flow,
        AddressFlow::LiveReferenceKill
            | AddressFlow::DeadReferenceKill
            | AddressFlow::ReferenceCast
            | AddressFlow::LaterLiveLoan
            | AddressFlow::DeadLaterLoan
    ) {
        let reference = reference(&mut types, U32, SemanticMutabilityV1::Mutable, false);
        locals.push(local(227, reference, SemanticLocalRoleV1::Temporary));
        if matches!(
            flow,
            AddressFlow::LaterLiveLoan | AddressFlow::DeadLaterLoan
        ) {
            setup.push(address());
        }
        setup.push(assign(
            place(6, reference),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: place(2, U32),
            },
        ));
        if flow == AddressFlow::ReferenceCast {
            setup.push(assign(
                place(3, raw),
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Pointer,
                    operand: SemanticOperandV1::Copy(place(6, reference)),
                },
            ));
        } else if !matches!(
            flow,
            AddressFlow::LaterLiveLoan | AddressFlow::DeadLaterLoan
        ) {
            setup.push(address());
        }
    } else {
        setup.push(address());
    }
    setup.push(copy());
    let blocks = match flow {
        AddressFlow::BranchGenerations | AddressFlow::BranchExpired => vec![
            block(231, setup, choose(1, 2)),
            block(
                232,
                vec![dead(2), live(2), initialize(), address()],
                goto(3),
            ),
            block(
                233,
                if flow == AddressFlow::BranchExpired {
                    vec![dead(2), live(2), initialize()]
                } else {
                    vec![dead(2), live(2), initialize(), address()]
                },
                goto(3),
            ),
            block(234, vec![read(3)], SemanticTerminatorKindV1::Return),
        ],
        AddressFlow::LoopStale | AddressFlow::LoopFresh => vec![
            block(231, setup, goto(1)),
            block(
                232,
                if flow == AddressFlow::LoopFresh {
                    vec![live(2), initialize(), address(), copy(), read(4)]
                } else {
                    vec![read(4), dead(2), live(2), initialize()]
                },
                choose(1, 2),
            ),
            block(233, vec![], SemanticTerminatorKindV1::Return),
        ],
        _ => {
            match flow {
                AddressFlow::BeforeInitialization => setup.push(write(4)),
                AddressFlow::RestartFresh
                | AddressFlow::RestartStale
                | AddressFlow::InertExpiredCopy => {
                    setup.extend([dead(2), live(2), initialize()]);
                    if flow == AddressFlow::RestartFresh {
                        setup.push(address());
                    }
                }
                AddressFlow::SharedWrite => setup.push(write(4)),
                AddressFlow::LiveReferenceKill | AddressFlow::DeadReferenceKill => {
                    setup.push(dead(2))
                }
                _ => {}
            }
            if flow == AddressFlow::InertExpiredCopy {
                setup.push(copy());
            } else if !matches!(
                flow,
                AddressFlow::LiveReferenceKill | AddressFlow::DeadReferenceKill
            ) {
                setup.push(read(if flow == AddressFlow::RestartFresh {
                    3
                } else {
                    4
                }));
            }
            // A genuinely future use keeps the safe holder live across the
            // forbidden kill/raw access. Unused holders now expire at their
            // final completed statement; the paired dead controls keep that
            // earlier source program and must complete.
            if matches!(
                flow,
                AddressFlow::LiveReferenceKill | AddressFlow::LaterLiveLoan
            ) {
                setup.push(assign(
                    place(5, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(target(6))),
                ));
            }
            vec![block(231, setup, SemanticTerminatorKindV1::Return)]
        }
    };
    let root = function(
        220,
        SemanticFunctionRoleV1::KernelRoot,
        abi(228, true, &[U32]),
        locals,
        blocks,
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
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

fn with_address_plan(
    flow: AddressFlow,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = address_owner(flow);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(20_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 20_000_000);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                source_storage_v29::with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |plan, _, budget| consume(plan, budget).map_err(Into::into),
                ),
            )
        })
        .unwrap();
    let safe = layouts.permits_root_emission_refund(&owner, 0, &budget);
    let cleanup = layouts.release(&mut budget);
    let demands_cleanup = if safe {
        demands.discard(&mut budget)
    } else {
        drop(demands);
        Err(ArgumentResourceV1::Accounting.into())
    };
    result.and(cleanup).and(demands_cleanup)
}

#[test]
fn original_raw_formation_copy_write_and_reference_cast_preserve_source_origins() {
    for flow in [
        AddressFlow::Read,
        AddressFlow::BeforeInitialization,
        AddressFlow::ReferenceCast,
    ] {
        let mut completed = false;
        let result = with_address_plan(flow, |plan, budget| {
            assert_eq!(plan.raw_origins.len(), 1);
            let origin = plan.raw_origins[0];
            assert_eq!(origin.instance, plan.root);
            assert_eq!(origin.local.index(), 2);
            assert_eq!(origin.ty, U32);
            assert_eq!(origin.generation, 1);
            assert_eq!(
                origin.formation == SourceReferenceRawFormationV29::ReferenceCast,
                flow == AddressFlow::ReferenceCast
            );
            assert_eq!(origin.parent.is_some(), flow == AddressFlow::ReferenceCast);
            assert!(!plan.raw_accesses.is_empty());
            for row in plan.raw_accesses.values() {
                assert_eq!(plan.raw_sets[row.set].local.index(), 2);
            }
            plan.check_owner(plan.instances, budget)?;
            completed = true;
            Ok(())
        });
        assert!(result.is_ok(), "{flow:?}: {result:?}");
        assert!(completed);
    }
}

#[test]
fn raw_formation_does_not_initialize_or_widen_or_override_a_live_reference() {
    for flow in [
        AddressFlow::UninitializedRead,
        AddressFlow::SharedWrite,
        AddressFlow::LiveReferenceKill,
        AddressFlow::LaterLiveLoan,
    ] {
        let mut entered = false;
        let result = with_address_plan(flow, |_, _| {
            entered = true;
            Ok(())
        });
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "{flow:?}: {result:?}"
        );
        assert!(!entered);
    }
}

#[test]
fn raw_formation_with_an_unused_reference_holder_keeps_completed_statement_expiry() {
    for flow in [AddressFlow::DeadReferenceKill, AddressFlow::DeadLaterLoan] {
        let mut completed = false;
        with_address_plan(flow, |plan, budget| {
            assert_eq!(plan.loans.len(), 1);
            assert_eq!(plan.raw_origins.len(), 1);
            let origin = plan.raw_origins[0];
            assert_eq!(
                (origin.instance, origin.local.index(), origin.generation),
                (plan.root, 2, 1)
            );
            assert!(origin.mutable);
            plan.check_owner(plan.instances, budget)?;
            completed = true;
            Ok(())
        })
        .unwrap();
        assert!(completed, "{flow:?}");
    }
}

#[test]
fn raw_aliases_expire_before_restart_and_never_revive_at_a_loop_site() {
    for flow in [
        AddressFlow::RestartStale,
        AddressFlow::LoopStale,
        AddressFlow::BranchExpired,
    ] {
        let mut entered = false;
        let result = with_address_plan(flow, |_, _| {
            entered = true;
            Ok(())
        });
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { .. })
            ),
            "{flow:?}: {result:?}"
        );
        assert!(!entered);
    }
    for flow in [
        AddressFlow::RestartFresh,
        AddressFlow::InertExpiredCopy,
        AddressFlow::BranchGenerations,
    ] {
        let result = with_address_plan(flow, |plan, _| {
            assert!(plan.raw_choices.iter().any(|choice| choice.expired));
            if flow == AddressFlow::BranchGenerations {
                assert!(!plan.epoch_sets.is_empty());
            }
            Ok(())
        });
        assert!(result.is_ok(), "{flow:?}: {result:?}");
    }
}

#[test]
fn repeated_original_storage_live_expires_saved_alias_before_fresh_assignment() {
    SOURCE_RAW_RESTART_TEST_V29.with(|observer| observer.set(Some((0, 0))));
    let result = with_address_plan(AddressFlow::LoopFresh, |plan, _| {
        assert!(plan.raw_choices.iter().any(|choice| choice.expired));
        Ok(())
    });
    let (same_site, expired) =
        SOURCE_RAW_RESTART_TEST_V29.with(|observer| observer.replace(None).unwrap());
    assert!(result.is_ok(), "{result:?}");
    assert!(
        same_site > 0,
        "the original StorageLive must actually repeat"
    );
    assert_eq!(
        expired, same_site,
        "old current aliases must expire before each same-site reset"
    );
}

#[test]
fn source_origin_plan_is_not_a_physical_pointer_or_foreign_owner_permit() {
    let mut completed = false;
    let result = with_address_plan(AddressFlow::Read, |plan, budget| {
        let emission = SourceReferenceEmissionV29::new(plan, budget)?;
        assert!(matches!(
            emission.finish(budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source raw address requires checked physical formation and stored-pointer correspondence",
                ..
            })
        ));
        emission.abort_scope(plan.instances, budget)?;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut foreign = ArgumentBudgetV1::new(&mut work, 1_000_000);
        assert!(plan.check_owner(plan.instances, &mut foreign).is_err());
        completed = true;
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
    assert!(completed);
}

fn numeric_address_request(
    bits: u16,
    signed: bool,
    reconstruct: Option<bool>,
) -> InertSemanticMirRequestV1 {
    let original = address_owner(AddressFlow::Read);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let numeric = if bits == 32 && !signed {
        U32
    } else {
        declaration(
            &mut types,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(u64::from(bits / 8)),
                u64::from(bits / 8),
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(signed, bits, u64::from(bits / 8)),
                    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
            None,
        )
    };
    let raw = SemanticTypeIdV1::from_index(2);
    let original_root = &semantic.functions()[0];
    let mut locals = original_root.locals().to_vec();
    locals.push(local(227, numeric, SemanticLocalRoleV1::Temporary));
    let mut statements = original_root.blocks()[0].statements()[..4].to_vec();
    statements.push(assign(
        place(6, numeric),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::PointerExposeProvenance,
            operand: SemanticOperandV1::Copy(place(4, raw)),
        },
    ));
    if let Some(constant) = reconstruct {
        statements.push(assign(
            place(3, raw),
            SemanticRvalueKindV1::Cast {
                kind: SemanticCastKindV1::PointerWithExposedProvenance,
                operand: if constant {
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        numeric,
                        SemanticConstantValueV1::Scalar(
                            SemanticScalarValueV1::new(7, u8::try_from(bits / 8).unwrap()).unwrap(),
                        ),
                    ))
                } else {
                    SemanticOperandV1::Copy(place(6, numeric))
                },
            },
        ));
    }
    InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function(
            220,
            SemanticFunctionRoleV1::KernelRoot,
            original_root.abi().clone(),
            locals,
            vec![block(231, statements, SemanticTerminatorKindV1::Return)],
        )],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
    )
    .unwrap()
}

fn numeric_address_owner(reconstruct: Option<bool>) -> ProductionSemanticSsaOwnerV1 {
    let admitted = numeric_address_request(64, false, reconstruct)
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
fn original_numeric_address_exposure_is_an_observation_not_pointer_provenance() {
    let mut completed = false;
    let result = with_live_projection_builder(numeric_address_owner(None), |builder, budget| {
        let plan = &builder.plan;
        let scalar = SemanticTypeIdV1::from_index(3);
        assert!(plan.address_observed);
        assert_eq!(plan.raw_origins.len(), 1);
        assert!(plan.loans.is_empty());
        let numeric: Vec<_> = plan.nodes.iter().filter(|node| node.ty == scalar).collect();
        assert!(!numeric.is_empty());
        assert!(
            numeric
                .iter()
                .all(|node| matches!(node.kind, SourceReferenceNodeKindV29::Plain(_)))
        );
        assert!(plan.raw_sets.iter().all(|set| set.ty != scalar));
        plan.check_owner(plan.instances, budget)?;
        completed = true;
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert!(completed);
}

#[test]
fn original_numeric_address_reconstruction_rejects_plain_values_and_constants() {
    for constant in [false, true] {
        let mut entered = false;
        let result = with_live_projection_builder(numeric_address_owner(Some(constant)), |_, _| {
            entered = true;
            Ok(())
        });
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source numeric address cannot reconstruct pointer provenance",
                    ..
                })
            ),
            "{constant}: {result:?}"
        );
        assert!(!entered);
    }
}

#[test]
fn numeric_address_width_and_signedness_negatives_are_original_mir_cast_errors() {
    for (bits, signed) in [(32, false), (64, true)] {
        let result = numeric_address_request(bits, signed, None)
            .admit_exact_v29(SemanticMirLimitsV1::default());
        assert!(
            matches!(
                result,
                Err(SemanticMirErrorV1::InvalidTypeOperation {
                    operation: SemanticTypeOperationV1::Cast,
                    ..
                })
            ),
            "{bits}/{signed}: {result:?}"
        );
    }
}

#[test]
fn numeric_address_representation_check_has_exact_work_and_no_storage_growth() {
    let owner = numeric_address_owner(None);
    // Two bounded declaration lookups, two shape projections and two scalar/
    // pointer representation checks; there is no allocation or retained row.
    const WORK: usize = 6;
    for available in [WORK, WORK - 1] {
        let mut completed = false;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(available);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_reference_check_address_exposure_v29(
            owner.source_semantic().types(),
            SemanticTypeIdV1::from_index(2),
            SemanticTypeIdV1::from_index(3),
            &mut budget,
        );
        if available == WORK {
            assert!(result.is_ok(), "{result:?}");
            assert_eq!(budget.work(), WORK);
        } else {
            assert!(
                matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ),
                "{result:?}"
            );
        }
        assert_eq!(budget.storage(), 0);
        completed = true;
        assert!(completed);
    }
}

#[test]
fn numeric_address_representation_rejects_foreign_type_and_pointer_metadata() {
    let owner = numeric_address_owner(None);
    let original = owner.source_semantic().types();
    for fault in 0..5 {
        let mut types = original.to_vec();
        let pointer = match original[2].shape() {
            SemanticTypeShapeV1::Pointer(pointer) => pointer,
            _ => panic!("original raw pointer"),
        };
        if fault < 3 {
            types[2] = SemanticTypeDeclV1::new(
                original[2].identity(),
                original[2].layout_identity(),
                original[2].layout().clone(),
                SemanticTypeShapeV1::Pointer(
                    SemanticPointerTypeV1::new_with_kind(
                        pointer.pointee(),
                        if fault == 0 {
                            SemanticPointerKindV1::Reference
                        } else {
                            SemanticPointerKindV1::Raw
                        },
                        pointer.mutability(),
                        pointer.address_space(),
                        pointer.pointer_width_bits(),
                        match fault {
                            1 => SemanticPointerMetadataV1::SliceLength,
                            2 => SemanticPointerMetadataV1::VTable,
                            _ => SemanticPointerMetadataV1::None,
                        },
                    )
                    .unwrap(),
                ),
            );
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        let result = source_reference_check_address_exposure_v29(
            &types,
            SemanticTypeIdV1::from_index(if fault == 3 { u32::MAX } else { 2 }),
            SemanticTypeIdV1::from_index(if fault == 4 { u32::MAX } else { 3 }),
            &mut budget,
        );
        assert!(
            matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source address exposure differs from its admitted scalar representation",
                    ..
                })
            ),
            "{fault}: {result:?}"
        );
        assert_eq!(budget.work(), 6);
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn numeric_address_cast_rejoins_original_operand_kind_type_and_site() {
    let mut owner = numeric_address_owner(None);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ADDRESS_TEST_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, ADDRESS_TEST_LIMIT);
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let mut completed = false;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let function = instances.instance(instances.root()).unwrap().declaration();
            let SemanticStatementKindV1::Assign(assignment) =
                function.blocks()[0].statements()[4].kind()
            else {
                panic!("original numeric exposure");
            };
            let SemanticRvalueKindV1::Cast { kind, operand } = assignment.value().kind() else {
                panic!("original numeric exposure cast");
            };
            let site = SourceReferenceSiteV29 {
                instance: instances.root(),
                block: SemanticBlockIdV1::from_index(0),
                statement: Some(4),
            };
            let clone = operand.clone();
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            for fault in 0..4 {
                let result = builder.cast_address_value(
                    if fault == 0 {
                        SourceReferenceSiteV29 {
                            statement: Some(3),
                            ..site
                        }
                    } else {
                        site
                    },
                    if fault == 1 {
                        SemanticCastKindV1::PointerWithExposedProvenance
                    } else {
                        *kind
                    },
                    if fault == 2 { &clone } else { operand },
                    if fault == 3 {
                        U32
                    } else {
                        assignment.value().result_type()
                    },
                    budget,
                );
                assert!(
                    matches!(
                        result,
                        Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                    ),
                    "{fault}: {result:?}"
                );
                assert!(builder.plan.nodes.is_empty());
                assert!(builder.plan.raw_origins.is_empty());
            }
            drop(builder);
            assert!(ledger == budget.work_ledger_identity_v1());
            assert!(budget.storage() >= floor);
            budget.release_storage(budget.storage() - floor).unwrap();
            completed = true;
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(())
        });
    assert!(result.is_ok(), "{result:?}");
    assert!(completed);
}

fn current_projection_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = address_owner(AddressFlow::Read);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let pair = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![0, 4], vec![0, 1]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
        None,
    );
    let array = declaration(
        &mut types,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            16,
            4,
            SemanticFieldsShapeV1::array(8, 2),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array {
            element: pair,
            length: 2,
        },
        None,
    );
    let root = &semantic.functions()[0];
    let mut locals = root.locals().to_vec();
    locals[2] = local(223, array, SemanticLocalRoleV1::Temporary);
    locals.push(local(227, pair, SemanticLocalRoleV1::Temporary));
    let raw = SemanticTypeIdV1::from_index(2);
    let path = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 1,
                    minimum_length: 2,
                    from_end: false,
                },
                pair,
            )
            .unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32).unwrap(),
        ],
        U32,
    )
    .unwrap();
    let statements = vec![
        root.blocks()[0].statements()[0].clone(),
        assign(
            place(6, pair),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![address_literal(7), address_literal(9)],
                )
                .unwrap(),
            ),
        ),
        assign(
            place(2, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    vec![
                        SemanticOperandV1::Copy(place(6, pair)),
                        SemanticOperandV1::Copy(place(6, pair)),
                    ],
                )
                .unwrap(),
            ),
        ),
        assign(
            place(3, raw),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Mutable,
                place: path.clone(),
            },
        ),
        root.blocks()[0].statements()[3].clone(),
        assign(path, SemanticRvalueKindV1::Use(address_literal(11))),
        root.blocks()[0].statements()[4].clone(),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function(
            220,
            SemanticFunctionRoleV1::KernelRoot,
            root.abi().clone(),
            locals,
            vec![block(231, statements, SemanticTerminatorKindV1::Return)],
        )],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
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

fn with_current_projection_builder(
    mut owner: ProductionSemanticSsaOwnerV1,
    evaluate: bool,
    consume: impl FnOnce(
        &mut SourceReferenceBuilderV29<'_, '_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    if evaluate {
        return with_live_projection_builder(owner, consume);
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ADDRESS_TEST_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, ADDRESS_TEST_LIMIT);
    budget.reserve_storage(101)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let mut builder = SourceReferenceBuilderV29::new(instances, budget).unwrap();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                builder.cfg_index(instances.root(), budget)?;
                consume(&mut builder, budget)
            }));
            drop(builder);
            assert!(ledger == budget.work_ledger_identity_v1());
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        })
        .unwrap();
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 101);
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn with_live_projection_builder(
    mut owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &mut SourceReferenceBuilderV29<'_, '_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(ADDRESS_TEST_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, ADDRESS_TEST_LIMIT);
    budget.reserve_storage(101)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage())?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let credit = layouts.capture_emission_credit(&owner, &mut budget)?;
    let result =
        with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let result = with_source_reference_descriptor_demands_scope_v29(
                instances,
                SourceReferenceStorageV29::ScalarCells,
                Some(&mut layouts),
                None,
                Some(lens),
                budget,
                |plan, root, budget| {
                    let mut builder = SourceReferenceBuilderV29::new_with_descriptor_demands(
                        plan.instances,
                        SourceReferenceStorageV29::ScalarCells,
                        root,
                        None,
                        plan.storage_demands,
                        budget,
                    )?;
                    builder.collect_storage_selectors(budget)?;
                    builder.function(builder.plan.root, None, budget)?;
                    consume(&mut builder, budget).map_err(Into::into)
                },
            );
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        })
        .unwrap();
    let scratch = credit.into_root_credit(&layouts, &budget).unwrap();
    assert!(layouts.permits_root_emission_refund(&owner, scratch, &budget));
    budget.release_storage(scratch)?;
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 101);
    result.and(cleanup).and(demand_cleanup)
}

#[test]
fn original_nested_raw_path_replays_the_retained_record_and_array_children() {
    let owner = current_projection_owner();
    let reached = std::cell::Cell::new(false);
    let result = with_live_projection_builder(owner, |builder, budget| {
        let plan = &builder.plan;
        assert_eq!(plan.raw_origins.len(), 1);
        assert_eq!(plan.raw_accesses.len(), 1);
        let access = plan.raw_accesses.values().next().unwrap();
        let path = plan.raw_projection_range(access.set, U32, budget)?;
        assert_eq!(path.len(), 2);
        assert!(matches!(
            plan.projections[path.start].kind(),
            SemanticProjectionKindV1::ConstantIndex { offset: 1, .. }
        ));
        assert_eq!(
            plan.projections[path.start + 1].kind(),
            SemanticProjectionKindV1::Field(0)
        );
        reached.set(true);
        Ok(())
    });
    assert!(result.is_ok(), "{result:?}");
    assert!(reached.get());
}

#[test]
fn current_projection_keeps_actual_children_and_rejects_absent_or_forged_shapes() {
    with_current_projection_builder(current_projection_owner(), false, |builder, budget| {
        let pair = SemanticTypeIdV1::from_index(3);
        let array = SemanticTypeIdV1::from_index(4);
        let first = builder.plain(U32, budget)?;
        let replacement = builder.plain(U32, budget)?;
        let children = builder.plan.children.len();
        emission_push_v1(&mut builder.plan.children, first, budget)?;
        emission_push_v1(&mut builder.plan.children, replacement, budget)?;
        let record = builder.node(
            pair,
            SourceReferenceNodeKindV29::Aggregate {
                first: children,
                count: 2,
            },
            budget,
        )?;
        let path = builder.plan.projections.len();
        emission_push_v1(
            &mut builder.plan.projections,
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32).unwrap(),
            budget,
        )?;
        assert_eq!(
            builder.current_projection_node(record, path..path + 1, U32, budget)?,
            replacement
        );
        builder.plan.children[children + 1] = first;
        assert_eq!(
            builder.current_projection_node(record, path..path + 1, U32, budget)?,
            first
        );
        let absent = builder.node(pair, SourceReferenceNodeKindV29::Absent, budget)?;
        assert_eq!(
            builder.current_projection_node(absent, path..path, pair, budget)?,
            absent
        );
        assert!(
            builder
                .current_projection_node(absent, path..path + 1, U32, budget)
                .is_err()
        );
        assert_eq!(builder.plan.nodes[absent].ty, pair);
        let opaque = builder.plain(pair, budget)?;
        let projected = builder.current_projection_node(opaque, path..path + 1, U32, budget)?;
        assert!(matches!(
            builder.plan.nodes[projected].kind,
            SourceReferenceNodeKindV29::Plain(None)
        ));
        assert!(builder.plan.nodes[projected].storage.is_none());
        assert!(builder.plan.nodes[projected].inactive.is_none());
        assert!(builder.plan.nodes[projected].descriptor.is_none());
        for (kind, ty) in [
            (SemanticProjectionKindV1::Field(2), U32),
            (SemanticProjectionKindV1::Field(1), pair),
            (SemanticProjectionKindV1::Dereference, U32),
            (SemanticProjectionKindV1::OpaqueCast, array),
            (SemanticProjectionKindV1::Subtype, array),
        ] {
            builder.plan.projections[path] = SemanticProjectionV1::new(kind, ty).unwrap();
            let before = builder.plan.nodes.len();
            assert!(
                builder
                    .current_projection_node(opaque, path..path + 1, ty, budget)
                    .is_err()
            );
            assert_eq!(builder.plan.nodes.len(), before);
        }
        for kind in [
            SemanticProjectionKindV1::OpaqueCast,
            SemanticProjectionKindV1::Subtype,
        ] {
            builder.plan.projections[path] = SemanticProjectionV1::new(kind, pair).unwrap();
            assert_eq!(
                builder.current_projection_node(opaque, path..path + 1, pair, budget)?,
                opaque
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn raw_path_query_rejects_changed_rosters_paths_types_and_owner_without_publication() {
    for fault in 0..10 {
        let reached = std::cell::Cell::new(false);
        let poisoned = std::cell::Cell::new(false);
        let result = with_current_projection_builder(
            address_owner(AddressFlow::BranchGenerations),
            true,
            |builder, budget| {
                let set = builder
                    .plan
                    .raw_sets
                    .iter()
                    .position(|set| set.count == 2)
                    .unwrap();
                let range = builder.plan.raw_sets[set];
                let origin = builder.plan.raw_choices[range.first + 1].origin;
                match fault {
                    0 => builder.plan.raw_sets[set].count = 0,
                    1 => builder.plan.raw_sets[set].first = usize::MAX,
                    2 => {
                        builder.plan.raw_choices[range.first + 1].origin =
                            builder.plan.raw_choices[range.first].origin
                    }
                    3 => builder.plan.raw_origins[origin].local = SemanticLocalIdV1::from_index(1),
                    4 => builder.plan.raw_origins[origin].ty = UNIT,
                    5 => builder.plan.raw_origins[origin].pointer_type = U32,
                    6 => builder.plan.raw_origins[origin].mutable = false,
                    7 => builder.plan.raw_origins[origin].parent = Some(0),
                    8 => builder.plan.raw_origins[origin].count = 1,
                    9 => builder.plan.raw_origins[origin].first = usize::MAX,
                    _ => unreachable!(),
                }
                let counts = (
                    builder.plan.nodes.len(),
                    builder.plan.raw_origins.len(),
                    builder.plan.raw_sets.len(),
                    builder.plan.raw_choices.len(),
                    builder.plan.projections.len(),
                );
                assert!(
                    builder.plan.raw_projection_range(set, U32, budget).is_err(),
                    "{fault}"
                );
                assert_eq!(
                    counts,
                    (
                        builder.plan.nodes.len(),
                        builder.plan.raw_origins.len(),
                        builder.plan.raw_sets.len(),
                        builder.plan.raw_choices.len(),
                        builder.plan.projections.len()
                    )
                );
                poisoned.set(builder.plan.failure.first_error().is_some());
                reached.set(true);
                Ok(())
            },
        );
        assert!(reached.get(), "{fault}: {result:?}");
        assert_eq!(result.is_err(), poisoned.get(), "{fault}: {result:?}");
    }
    let reached = std::cell::Cell::new(false);
    let result =
        with_current_projection_builder(address_owner(AddressFlow::Read), true, |builder, _| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(ADDRESS_TEST_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut work, ADDRESS_TEST_LIMIT);
            assert!(
                builder
                    .plan
                    .raw_projection_range(0, U32, &mut foreign)
                    .is_err()
            );
            assert_eq!(foreign.work(), 0);
            reached.set(true);
            Ok(())
        });
    assert!(reached.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}

#[test]
fn raw_unanimous_path_has_independent_work_and_actual_header_boundaries() {
    use std::mem::size_of;
    #[allow(dead_code)]
    struct QueryFrame {
        previous: Option<usize>,
        first: Option<usize>,
        pointer: Option<SemanticTypeIdV1>,
    }
    let range_headers = size_of::<std::ops::Range<usize>>()
        + 2 * size_of::<Result<std::ops::Range<usize>, ProductionSemanticKirErrorV1>>();
    let frame_headers =
        size_of::<QueryFrame>() + 2 * size_of::<Result<QueryFrame, ProductionSemanticKirErrorV1>>();
    for storage in [false, true] {
        for short in [0, 1] {
            let reached = std::cell::Cell::new(false);
            let result = with_current_projection_builder(
                address_owner(AddressFlow::BranchGenerations),
                true,
                |builder, budget| {
                    let set = builder
                        .plan
                        .raw_sets
                        .iter()
                        .position(|set| set.count == 2)
                        .unwrap();
                    // Five owner checks, three range checks and twelve checks for
                    // each of the two original zero-projection alternatives.
                    const WORK: usize = 5 + 3 + 12 * 2;
                    let headers = range_headers + frame_headers;
                    if storage {
                        budget.reserve_storage(
                            ADDRESS_TEST_LIMIT - budget.storage() - (headers - short),
                        )?;
                    } else {
                        budget.charge_work(ADDRESS_TEST_LIMIT - budget.work() - (WORK - short))?;
                    }
                    let before = (budget.work(), budget.storage());
                    let result = builder.plan.raw_projection_range(set, U32, budget);
                    if short == 0 {
                        assert!(
                            result.as_ref().is_ok_and(|range| range.is_empty()),
                            "{result:?}"
                        );
                        assert_eq!(budget.work() - before.0, WORK);
                        assert_eq!(budget.storage() - before.1, headers);
                    } else {
                        assert!(matches!(
                            result,
                            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
                        ));
                        assert_eq!(budget.work() - before.0, if storage { 5 } else { 20 });
                        assert_eq!(
                            budget.storage() - before.1,
                            if storage { range_headers } else { headers }
                        );
                        let error = format!("{result:?}");
                        let retry = builder.plan.raw_projection_range(set, U32, budget);
                        assert_eq!(format!("{retry:?}"), error);
                    }
                    reached.set(true);
                    Err(source_reference_error_v29(
                        "raw query boundary callback stop",
                    ))
                },
            );
            assert!(reached.get(), "{storage}/{short}: {result:?}");
            assert!(result.is_err());
        }
    }
}

fn projected_reference_cast_owner(mode: u8) -> ProductionSemanticSsaOwnerV1 {
    let base = current_projection_owner();
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let kind = if mode >= 2 {
        SemanticBorrowKindV1::Mutable
    } else {
        SemanticBorrowKindV1::Shared
    };
    let reference = reference(
        &mut types,
        U32,
        if mode >= 2 {
            SemanticMutabilityV1::Mutable
        } else {
            SemanticMutabilityV1::Immutable
        },
        false,
    );
    let raw = SemanticTypeIdV1::from_index(2);
    let root = &semantic.functions()[0];
    let mut locals = root.locals().to_vec();
    locals.push(local(228, reference, SemanticLocalRoleV1::Temporary));
    if mode >= 2 {
        locals.push(local(229, reference, SemanticLocalRoleV1::Temporary));
    }
    let old = root.blocks()[0].statements();
    let SemanticStatementKindV1::Assign(formation) = old[3].kind() else {
        panic!("original raw formation");
    };
    let SemanticRvalueKindV1::AddressOf { place: target, .. } = formation.value().kind() else {
        panic!("original projected target");
    };
    let mut statements = old[..3].to_vec();
    statements.push(assign(
        place(7, reference),
        SemanticRvalueKindV1::Borrow {
            kind,
            place: target.clone(),
        },
    ));
    statements.push(assign(
        place(3, raw),
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Pointer,
            operand: SemanticOperandV1::Copy(place(7, reference)),
        },
    ));
    statements.push(old[4].clone());
    let dereference = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    if mode == 1 {
        statements.push(assign(
            dereference(4),
            SemanticRvalueKindV1::Use(address_literal(17)),
        ));
    } else if mode >= 2 {
        statements.push(assign(
            place(8, reference),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: dereference(7),
            },
        ));
    }
    if mode == 3 {
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(8)),
        ));
    }
    statements.push(old[6].clone());
    if mode == 2 {
        statements.push(assign(
            place(5, U32),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(8))),
        ));
    }
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function(
            220,
            SemanticFunctionRoleV1::KernelRoot,
            root.abi().clone(),
            locals,
            vec![block(231, statements, SemanticTerminatorKindV1::Return)],
        )],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
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
fn projected_raw_reference_cast_keeps_shared_permission_and_parent_reborrow_suspension() {
    for mode in 0..4 {
        let reached = std::cell::Cell::new(false);
        let result = with_live_projection_builder(
            projected_reference_cast_owner(mode),
            |builder, budget| {
                let plan = &builder.plan;
                assert_eq!(plan.raw_origins.len(), 1);
                let origin = plan.raw_origins[0];
                assert!(origin.parent.is_some());
                assert_eq!(origin.mutable, mode >= 2);
                assert_eq!(
                    origin.formation,
                    SourceReferenceRawFormationV29::ReferenceCast
                );
                assert_eq!(plan.raw_projection_range(0, U32, budget)?.len(), 2);
                reached.set(true);
                Ok(())
            },
        );
        assert_eq!(result.is_ok(), mode == 0 || mode == 3, "{mode}: {result:?}");
        assert_eq!(reached.get(), mode == 0 || mode == 3);
        if mode == 1 {
            assert!(
                format!("{result:?}")
                    .contains("source raw pointer access widens its original permission")
            );
        } else if mode == 2 {
            assert!(
                format!("{result:?}")
                    .contains("source reference parent is suspended by a live reborrow")
            );
        }
    }
}

#[test]
fn current_projection_has_independent_empty_and_retained_field_work_and_storage_boundaries() {
    use std::mem::size_of;
    #[allow(dead_code)]
    struct CurrentFrame {
        node: usize,
        range: std::ops::Range<usize>,
        expected: SemanticTypeIdV1,
    }
    let headers = size_of::<CurrentFrame>()
        + 2 * size_of::<Result<CurrentFrame, ProductionSemanticKirErrorV1>>()
        + size_of::<(SourceReferenceNodeV29, SemanticProjectionV1)>()
        + 2 * size_of::<
            Result<(SourceReferenceNodeV29, SemanticProjectionV1), ProductionSemanticKirErrorV1>,
        >()
        + size_of::<usize>()
        + 2 * size_of::<Result<usize, ProductionSemanticKirErrorV1>>();
    for field in [false, true] {
        for storage in [false, true] {
            for short in [0, 1] {
                with_current_projection_builder(
                    current_projection_owner(),
                    false,
                    |builder, budget| {
                        let pair = SemanticTypeIdV1::from_index(3);
                        let child = builder.plain(U32, budget)?;
                        let first = builder.plan.children.len();
                        emission_push_v1(&mut builder.plan.children, child, budget)?;
                        emission_push_v1(&mut builder.plan.children, child, budget)?;
                        let root = builder.node(
                            pair,
                            SourceReferenceNodeKindV29::Aggregate { first, count: 2 },
                            budget,
                        )?;
                        let path = builder.plan.projections.len();
                        emission_push_v1(
                            &mut builder.plan.projections,
                            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32)
                                .unwrap(),
                            budget,
                        )?;
                        // Owner5, range2, final type1; a retained field adds
                        // row/projection2, original shape4, child2, child type1.
                        let work = if field { 17 } else { 8 };
                        let headers = headers
                            + if field {
                                2 * size_of::<Result<(), ProductionSemanticKirErrorV1>>()
                            } else {
                                0
                            };
                        if storage {
                            budget.reserve_storage(
                                ADDRESS_TEST_LIMIT - budget.storage() - (headers - short),
                            )?;
                        } else {
                            budget
                                .charge_work(ADDRESS_TEST_LIMIT - budget.work() - (work - short))?;
                        }
                        let before = (budget.work(), budget.storage(), builder.plan.nodes.len());
                        let result = builder.current_projection_node(
                            root,
                            path..path + usize::from(field),
                            if field { U32 } else { pair },
                            budget,
                        );
                        if short == 0 {
                            assert_eq!(result?, if field { child } else { root });
                            assert_eq!(budget.work() - before.0, work);
                            assert_eq!(budget.storage() - before.1, headers);
                        } else {
                            assert!(matches!(
                                result,
                                Err(
                                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_)
                                )
                            ));
                            assert_eq!(
                                budget.work() - before.0,
                                if storage {
                                    if field { 9 } else { 5 }
                                } else {
                                    work - 1
                                }
                            );
                            assert_eq!(
                                format!("{result:?}"),
                                format!(
                                    "{:?}",
                                    builder.current_projection_node(
                                        root,
                                        path..path + usize::from(field),
                                        if field { U32 } else { pair },
                                        budget
                                    )
                                )
                            );
                        }
                        assert_eq!(builder.plan.nodes.len(), before.2);
                        Ok(())
                    },
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn opaque_projection_never_synthesizes_pointer_facts_and_replay_cleanup_covers_error_and_panic() {
    for mode in 0..3 {
        let result = std::panic::catch_unwind(|| {
            with_current_projection_builder(current_projection_owner(), false, |builder, budget| {
                assert!(
                    source_reference_check_opaque_projection_v29(
                        builder.plan.instances.owner().source_semantic().types(),
                        SemanticTypeIdV1::from_index(2),
                        budget
                    )
                    .is_err()
                );
                let node = builder.plain(U32, budget)?;
                let end = builder.plan.projections.len();
                assert_eq!(
                    builder.current_projection_node(node, end..end, U32, budget)?,
                    node
                );
                match mode {
                    0 => Ok(()),
                    1 => Err(source_reference_error_v29(
                        "current projection callback sentinel",
                    )),
                    _ => panic!("current projection cleanup probe"),
                }
            })
        });
        match mode {
            0 => result.unwrap().unwrap(),
            1 => assert!(
                format!("{:?}", result.unwrap()).contains("current projection callback sentinel")
            ),
            _ => assert!(result.is_err()),
        }
    }
}

#[test]
fn current_projection_path_copy_prepays_its_exact_capacity_before_publication() {
    use std::mem::size_of;
    let headers = size_of::<Vec<SemanticProjectionV1>>()
        + 2 * size_of::<Result<Vec<SemanticProjectionV1>, ProductionSemanticKirErrorV1>>();
    for count in [0, 2] {
        for storage in [false, true] {
            for short in [0, 1] {
                with_current_projection_builder(
                    current_projection_owner(),
                    false,
                    |builder, budget| {
                        let first = builder.plan.projections.len();
                        for _ in 0..count {
                            emission_push_v1(
                                &mut builder.plan.projections,
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U32)
                                    .unwrap(),
                                budget,
                            )?;
                        }
                        let bytes = headers + count * size_of::<SemanticProjectionV1>();
                        // Owner5, bounds2, vector allocation3, copied components P.
                        let work = 10 + count;
                        if storage {
                            budget.reserve_storage(
                                ADDRESS_TEST_LIMIT - budget.storage() - (bytes - short),
                            )?;
                        } else {
                            budget
                                .charge_work(ADDRESS_TEST_LIMIT - budget.work() - (work - short))?;
                        }
                        let before = (
                            budget.work(),
                            budget.storage(),
                            builder.plan.projections.len(),
                        );
                        let result = builder
                            .plan
                            .current_projection_path(first..first + count, budget);
                        if short == 0 {
                            let path = result?;
                            assert_eq!(
                                path.as_slice(),
                                &builder.plan.projections[first..first + count]
                            );
                            assert_eq!(path.capacity(), count);
                            assert_eq!(budget.work() - before.0, work);
                            assert_eq!(budget.storage() - before.1, bytes);
                        } else {
                            assert!(matches!(
                                result,
                                Err(
                                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_)
                                )
                            ));
                            assert_eq!(
                                budget.work() - before.0,
                                if storage && count == 0 {
                                    7
                                } else if !storage && count == 0 {
                                    7
                                } else {
                                    10
                                }
                            );
                            assert_eq!(
                                budget.storage() - before.1,
                                if storage {
                                    if count == 0 { 0 } else { headers }
                                } else {
                                    if count == 0 { headers } else { bytes }
                                }
                            );
                            assert_eq!(
                                format!("{result:?}"),
                                format!(
                                    "{:?}",
                                    builder
                                        .plan
                                        .current_projection_path(first..first + count, budget)
                                )
                            );
                        }
                        assert_eq!(builder.plan.projections.len(), before.2);
                        Ok(())
                    },
                )
                .unwrap();
            }
        }
    }
}

fn current_projection_branch_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = current_projection_owner();
    let semantic = base.source_semantic();
    let original = &semantic.functions()[0];
    let statements = original.blocks()[0].statements();
    let go = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    let switch = SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(1, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(1),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(2),
            ),
        )
        .unwrap(),
    };
    let blocks = vec![
        block(231, statements[..3].to_vec(), switch),
        block(232, statements[3..5].to_vec(), go(3)),
        block(233, statements[3..5].to_vec(), go(3)),
        block(
            234,
            statements[5..].to_vec(),
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function(
            220,
            SemanticFunctionRoleV1::KernelRoot,
            original.abi().clone(),
            original.locals().to_vec(),
            blocks,
        )],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
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
fn raw_projected_alternatives_validate_every_original_full_path_before_selecting_one() {
    let reached = std::cell::Cell::new(false);
    let result =
        with_live_projection_builder(current_projection_branch_owner(), |builder, budget| {
            let plan = &builder.plan;
            let access = plan.raw_accesses.values().next().unwrap();
            assert_eq!(plan.raw_sets[access.set].count, 2);
            let before = budget.work();
            let path = plan.raw_projection_range(access.set, U32, budget)?;
            assert_eq!(path.len(), 2);
            assert_eq!(budget.work() - before, 5 + 3 + 12 * 2 + 2 * 2 + 2);
            assert_eq!(plan.raw_accesses.len(), 1);
            reached.set(true);
            Ok(())
        });
    assert!(result.is_ok(), "{result:?}");
    assert!(reached.get());
    let reached = std::cell::Cell::new(false);
    let result =
        with_live_projection_builder(current_projection_branch_owner(), |builder, budget| {
            let set = builder
                .plan
                .raw_sets
                .iter()
                .position(|set| set.count == 2)
                .unwrap();
            let row = builder.plan.raw_sets[set];
            let first = builder.plan.raw_choices[row.first].origin;
            let second = builder.plan.raw_choices[row.first + 1].origin;
            let first_path = builder.plan.raw_origins[first].first;
            let second_path = builder.plan.raw_origins[second].first;
            assert_ne!(first_path, second_path);
            assert_eq!(
                builder.plan.raw_projection_range(set, U32, budget)?.len(),
                2
            );
            builder.plan.projections[second_path + 1] =
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), U32).unwrap();
            let before = (builder.plan.nodes.len(), builder.plan.projections.len());
            let result = builder.plan.raw_projection_range(set, U32, budget);
            assert!(
                format!("{result:?}")
                    .contains("alternatives do not retain one original target path")
            );
            assert_eq!(
                before,
                (builder.plan.nodes.len(), builder.plan.projections.len())
            );
            assert_eq!(
                builder.plan.projections[first_path + 1].kind(),
                SemanticProjectionKindV1::Field(0)
            );
            reached.set(true);
            Ok(())
        });
    assert!(reached.get(), "{result:?}");
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn raw_empty_absent_target_keeps_original_root_type_and_projected_failure_is_atomic() {
    for projected in [false, true] {
        let reached = std::cell::Cell::new(false);
        let owner = if projected {
            current_projection_branch_owner()
        } else {
            address_owner(AddressFlow::BranchGenerations)
        };
        let result = with_live_projection_builder(owner, |builder, budget| {
            let root = builder.plan.root;
            let source = builder
                .plan
                .states
                .iter()
                .position(|state| {
                    state
                        .get(4)
                        .and_then(|local| local.node)
                        .is_some_and(|node| {
                            let SourceReferenceNodeKindV29::Address(set) =
                                builder.plan.nodes[node].kind
                            else {
                                return false;
                            };
                            let row = builder.plan.raw_sets[set];
                            builder.plan.raw_choices[row.first..row.first + row.count]
                                .iter()
                                .all(|choice| !choice.expired)
                        })
                })
                .expect("original pre-return raw holder state");
            let state = builder.clone_state(source, budget)?;
            builder.frames[root.index()] = Some(state);
            let holder = builder.plan.states[state][4];
            let holder_node = holder.node.unwrap();
            let SourceReferenceNodeKindV29::Address(set) = builder.plan.nodes[holder_node].kind
            else {
                unreachable!();
            };
            builder.plan.states[state][2].node = None;
            source_reference_emission_prepay_v29::<SourceReferencePlaceV29>(budget)?;
            let mut place = SourceReferencePlaceV29 {
                instance: root,
                local: SemanticLocalIdV1::from_index(4),
                generation: holder.generation,
                value: holder_node,
                representation_root: holder_node,
                node: holder_node,
                projections: Vec::new(),
                selector_source: None,
                anchor: None,
                loan: None,
                shared_path: false,
                traversed: Vec::new(),
            };
            let count = builder.plan.nodes.len();
            let result = builder.resolve_raw_target(
                &mut place,
                set,
                U32,
                SourceReferenceAccessV29::Address,
                budget,
            );
            let root_type = if projected {
                SemanticTypeIdV1::from_index(4)
            } else {
                U32
            };
            assert_eq!(builder.plan.nodes.len(), count + 1);
            assert_eq!(builder.plan.nodes[count].ty, root_type);
            assert!(matches!(
                builder.plan.nodes[count].kind,
                SourceReferenceNodeKindV29::Absent
            ));
            assert!(builder.plan.states[state][2].node.is_none());
            if projected {
                assert!(result.is_err());
                assert_eq!(place.local.index(), 4);
                assert_eq!(place.generation, holder.generation);
                assert_eq!(
                    (place.value, place.representation_root, place.node),
                    (holder_node, holder_node, holder_node)
                );
                assert!(place.projections.is_empty());
                assert!(place.anchor.is_none() && place.loan.is_none() && !place.shared_path);
            } else {
                result?;
                assert_eq!(place.local.index(), 2);
                assert_eq!(
                    (place.value, place.representation_root, place.node),
                    (count, count, count)
                );
                assert!(place.projections.is_empty());
            }
            builder.frames[root.index()] = None;
            reached.set(true);
            Ok(())
        });
        assert!(reached.get(), "{projected}: {result:?}");
        assert!(result.is_ok(), "{projected}: {result:?}");
    }
}

mod raw_suffix_tests {
    use super::*;
    include!("production_source_reference_raw_suffix_v29_tests.rs");
}

mod original_access_query_tests {
    use super::*;
    include!("production_source_reference_original_access_resources_v29_tests.rs");
}

include!("production_source_object_activation_scratch_v29_tests.rs");

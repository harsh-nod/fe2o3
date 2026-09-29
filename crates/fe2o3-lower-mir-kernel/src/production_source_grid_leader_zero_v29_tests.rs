use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

thread_local! {
    static ZERO_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ZERO_OBSERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ZERO_MUTATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ZERO_LOAN_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ZERO_FOREIGN_CUSTODY: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ZERO_FOREIGN_CUSTODY_OBSERVED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static ZERO_SOURCE_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ZERO_BINDING_FAULT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ZERO_BINDING_MUTATED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn observe_original_zero_binding_v29(
    lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
    block: SemanticBlockIdV1,
    statement: Option<u32>,
    local: usize,
) {
    let fault = ZERO_BINDING_FAULT.get();
    if fault == 0 || ZERO_BINDING_MUTATED.get() {
        return;
    }
    let borrowed = statement
        .and_then(|index| lowering.function.blocks().get(block.index() as usize)?.statements().get(index as usize))
        .is_some_and(|row| matches!(row.kind(), SemanticStatementKindV1::Assign(assignment)
            if matches!(assignment.value().kind(), SemanticRvalueKindV1::Borrow { kind: SemanticBorrowKindV1::Shared, .. })));
    if borrowed != (fault > 3) {
        return;
    }
    assert!(matches!(
        lowering.locals[local],
        Some(SemanticValueBindingV1::GridLeader { .. })
    ));
    lowering.locals[local] = match (fault - 1) % 3 {
        0 => None,
        1 => Some(SemanticValueBindingV1::Unit),
        2 => Some(SemanticValueBindingV1::GridLeader {
            availability: SemanticCapabilityAvailabilityV1::EnumPayload {
                local: SemanticLocalIdV1::from_index(2),
                variant: u32::MAX,
            },
        }),
        _ => unreachable!(),
    };
    ZERO_BINDING_MUTATED.set(true);
}

fn zero_edge(role: SemanticEdgeRoleV1, block: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(block))
}

fn zero_call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: SemanticPlaceV1,
    next: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                destination,
                zero_edge(SemanticEdgeRoleV1::CallReturn, next),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

fn zero_abi(
    tag: u8,
    arguments: Vec<SemanticAbiValueV1>,
    result: SemanticAbiValueV1,
) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        arguments.len() as u32,
        arguments
            .into_iter()
            .map(SemanticAbiArgumentV1::source)
            .collect(),
        result,
    )
    .unwrap()
}

fn zero_owner(outside_some: bool, nested: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = module_fixture_owner(ModuleFixture::Ordinary);
    let semantic = base.source_semantic();
    let prior = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let leader = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![UNIT, UNIT]).unwrap()),
        None,
    );
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 1),
    );
    let discr = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, u8::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 8,
        }),
        None,
    );
    let reference = declaration(
        &mut types,
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
                leader,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Immutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        None,
    );
    types[reference.index() as usize] = types[reference.index() as usize]
        .clone()
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        1,
                    )
                    .unwrap(),
                ),
                None,
            ),
        );
    let variant = |index, offsets: Vec<u64>, backend| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            1,
            1,
            SemanticFieldsShapeV1::arbitrary(offsets.clone(), (0..offsets.len() as u32).collect())
                .unwrap(),
            backend,
            None,
            false,
            None,
            1,
            0,
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap()
    };
    let optional = declaration(
        &mut types,
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            1,
            1,
            SemanticBackendReprV1::scalar(tag),
            false,
            SemanticEnumLayoutV1::new(
                vec![
                    variant(0, vec![], SemanticBackendReprV1::memory(true)),
                    variant(1, vec![1], SemanticBackendReprV1::scalar(tag)),
                ],
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Enum {
            discriminant: discr,
            variants: vec![
                SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                SemanticEnumVariantV1::new(1, SemanticAggregateTypeV1::new(vec![leader]).unwrap()),
            ]
            .into_boxed_slice(),
        },
        None,
    );
    let mut locals = prior.locals().to_vec();
    locals.extend([
        local(70, optional, SemanticLocalRoleV1::Temporary),
        local(71, discr, SemanticLocalRoleV1::Temporary),
        local(72, leader, SemanticLocalRoleV1::Temporary),
        local(73, reference, SemanticLocalRoleV1::Temporary),
        local(74, UNIT, SemanticLocalRoleV1::Temporary),
        local(75, leader, SemanticLocalRoleV1::Temporary),
    ]);
    let mut root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        prior.abi().clone(),
        locals,
        vec![
            block(
                80,
                vec![],
                zero_call(if nested { 3 } else { 2 }, vec![], place(2, optional), 1),
            ),
            block(
                81,
                vec![assign(
                    place(3, discr),
                    SemanticRvalueKindV1::Discriminant(place(2, optional)),
                )],
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(3, discr)),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            1,
                            zero_edge(
                                SemanticEdgeRoleV1::SwitchValue,
                                if outside_some { 3 } else { 2 },
                            ),
                        )],
                        zero_edge(
                            SemanticEdgeRoleV1::SwitchOtherwise,
                            if outside_some { 2 } else { 3 },
                        ),
                    )
                    .unwrap(),
                },
            ),
            block(
                82,
                vec![
                    assign(
                        place(4, leader),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                            SemanticConstantV1::new(leader, SemanticConstantValueV1::ZeroSized),
                        )),
                    ),
                    assign(
                        place(7, leader),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(4, leader))),
                    ),
                    assign(
                        place(5, reference),
                        SemanticRvalueKindV1::Borrow {
                            kind: SemanticBorrowKindV1::Shared,
                            place: place(4, leader),
                        },
                    ),
                ],
                zero_call(
                    1,
                    vec![SemanticOperandV1::Copy(place(5, reference))],
                    place(6, UNIT),
                    3,
                ),
            ),
            block(
                83,
                vec![],
                match prior.blocks()[0].terminator().kind() {
                    SemanticTerminatorKindV1::Assert {
                        condition,
                        expected,
                        message,
                        unwind,
                        ..
                    } => SemanticTerminatorKindV1::Assert {
                        condition: condition.clone(),
                        expected: *expected,
                        message: message.clone(),
                        unwind: *unwind,
                        target: zero_edge(SemanticEdgeRoleV1::AssertSuccess, 4),
                    },
                    _ => panic!("ordinary source assertion"),
                },
            ),
            block(84, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(prior.kernel_entry().unwrap().clone());
    if ZERO_SOURCE_FAULT.get() != 0 {
        let mut blocks = root.blocks().to_vec();
        let mut statements = blocks[2].statements().to_vec();
        statements.push(if ZERO_SOURCE_FAULT.get() == 1 {
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
            )
        } else {
            assign(
                place(4, leader),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    leader,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            )
        });
        blocks[2] = block(82, statements, blocks[2].terminator().kind().clone());
        root = function(
            60,
            root.role(),
            root.abi().clone(),
            root.locals().to_vec(),
            blocks,
        )
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    }
    let ref_abi = SemanticAbiValueV1::new(
        reference,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(
                    true,
                    Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                    true,
                    true,
                    false,
                    true,
                ),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    );
    let helper_abi = zero_abi(
        102,
        vec![ref_abi],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let helper = |identity, forward| {
        function(
            identity,
            SemanticFunctionRoleV1::InternalHelper,
            helper_abi.clone(),
            vec![
                local(identity + 1, UNIT, SemanticLocalRoleV1::Return),
                local(identity + 2, reference, SemanticLocalRoleV1::Argument(0)),
                local(identity + 3, UNIT, SemanticLocalRoleV1::Temporary),
            ],
            if forward {
                vec![
                    block(
                        identity + 4,
                        vec![],
                        zero_call(
                            2,
                            vec![SemanticOperandV1::Copy(place(1, reference))],
                            place(2, UNIT),
                            1,
                        ),
                    ),
                    block(identity + 5, vec![], SemanticTerminatorKindV1::Return),
                ]
            } else {
                vec![block(
                    identity + 4,
                    vec![],
                    SemanticTerminatorKindV1::Return,
                )]
            },
        )
    };
    let mut functions = vec![root, helper(100, nested)];
    if nested {
        functions.push(helper(120, false));
    }
    let mut callables: Vec<_> = (0..functions.len())
        .map(|index| {
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(index as u32))
        })
        .collect();
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([140; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([140; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([140; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([140; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([140; 32]),
            source(),
            zero_abi(
                140,
                vec![],
                SemanticAbiValueV1::new(
                    optional,
                    SemanticAbiPassModeV1::Direct(
                        SemanticAbiValueAttributesV1::new(
                            SemanticAbiRegularAttributesV1::new(
                                false, None, false, false, false, true,
                            ),
                            SemanticAbiExtensionV1::ZeroExtend,
                            0,
                            None,
                        )
                        .unwrap(),
                    ),
                ),
            ),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent {
            grid_leader: leader,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([140; 32]),
    });
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
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

fn zero_observer(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let anchors = emitted[instances.root().index()]
        .as_mut()
        .unwrap()
        .scoped_memory_anchors
        .as_mut()
        .unwrap();
    assert_eq!(anchors.zero_objects.len(), 2);
    assert_eq!(
        anchors.zero_objects[0].access,
        SourceReferenceAccessV29::Write
    );
    assert_eq!(
        anchors.zero_objects[1].access,
        SourceReferenceAccessV29::Read
    );
    for row in &anchors.zero_objects {
        assert!(
            matches!(row.endpoint.object, ScopedObjectIdentityV29::Local { local, .. } if local.index() == 4)
        );
    }
    ZERO_OBSERVATIONS.set(ZERO_OBSERVATIONS.get() + 1);
    match ZERO_FAULT.get() {
        0 => (),
        1 => {
            anchors.zero_objects.clear();
        }
        2 => {
            anchors.zero_objects[0].frame.site =
                execution_site_v29(SemanticBlockIdV1::from_index(0), Some(0));
        }
        3 => {
            anchors.zero_objects[0].endpoint.projected_type = U32;
        }
        4 => {
            anchors.zero_objects[1] = anchors.zero_objects[0];
        }
        _ => panic!("zero receipt mutation"),
    }
    if ZERO_FAULT.get() != 0 {
        ZERO_MUTATED.set(true);
    }
    Ok(())
}

fn zero_custody_observer(
    instances: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    references.plan.check_owner(instances, budget)?;
    let rows: Vec<_> = references
        .grid_leaders
        .iter()
        .enumerate()
        .filter_map(|(loan, cell)| cell.get().map(|proof| (loan, proof)))
        .collect();
    assert_eq!(
        rows.len(),
        1,
        "one original capture, including nested forwarding"
    );
    let (loan, proof) = rows[0];
    let source_type = references.plan.loans[loan].source_type;
    references.check_grid_leader_origin_v29(loan, proof.ty, source_type, budget)?;
    let exact = |result| {
        assert!(matches!(
            result,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "logical GridLeader effect differs from its original source contract",
                ..
            })
        ))
    };
    exact(references.check_grid_leader_origin_v29(usize::MAX, proof.ty, source_type, budget));
    exact(references.check_grid_leader_origin_v29(loan, U32, source_type, budget));
    exact(references.check_grid_leader_origin_v29(loan, proof.ty, U32, budget));
    let cell = &references.grid_leaders[loan];
    for fault in 0..4 {
        let mut altered = proof;
        match fault {
            0 => cell.set(None),
            1 => {
                altered.site.statement = Some(usize::MAX);
                cell.set(Some(altered));
            }
            2 => {
                altered.origin = usize::MAX;
                cell.set(Some(altered));
            }
            3 => {
                altered.ty = U32;
                cell.set(Some(altered));
            }
            _ => unreachable!(),
        }
        let result = references.check_grid_leader_origin_v29(loan, proof.ty, source_type, budget);
        cell.set(Some(proof));
        exact(result);
    }
    let saved_claim = references.claimed[loan].replace(false);
    let result = references.check_grid_leader_origin_v29(loan, proof.ty, source_type, budget);
    references.claimed[loan].set(saved_claim);
    exact(result);
    if ZERO_FOREIGN_CUSTODY.get() {
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
        foreign.reserve_storage(budget.storage()).unwrap();
        let before = (foreign.storage(), foreign.work());
        assert!(matches!(
            references.check_grid_leader_origin_v29(loan, proof.ty, source_type, &mut foreign),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((foreign.storage(), foreign.work()), before);
        let before = (budget.storage(), budget.work());
        assert!(matches!(
            references.check_grid_leader_origin_v29(loan, proof.ty, source_type, budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((budget.storage(), budget.work()), before);
        ZERO_FOREIGN_CUSTODY_OBSERVED.set(true);
        // Swallowing the local error must not revive the enclosing transaction.
        return Ok(());
    }
    references.check_grid_leader_origin_v29(loan, proof.ty, source_type, budget)?;
    ZERO_LOAN_CHECKS.set(ZERO_LOAN_CHECKS.get() + 1);
    Ok(())
}

fn run_zero(outside: bool, nested: bool, fault: u8) -> (SourceOwnedResultV18<()>, bool) {
    let (result, completed, _, _) =
        run_zero_limits(outside, nested, fault, MODULE_LIMIT, MODULE_LIMIT);
    (result, completed)
}

fn run_zero_limits(
    outside: bool,
    nested: bool,
    fault: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, bool, usize, usize) {
    struct Restore(
        u8,
        Option<ScopedSlotObserverV29>,
        Option<ScopedSlotCustodyObserverV29>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            ZERO_FAULT.set(self.0);
            SCOPED_SLOT_OBSERVER_V29.set(self.1);
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.2);
        }
    }
    let _restore = Restore(
        ZERO_FAULT.replace(fault),
        SCOPED_SLOT_OBSERVER_V29.replace(Some(zero_observer)),
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(zero_custody_observer)),
    );
    ZERO_OBSERVATIONS.set(0);
    ZERO_MUTATED.set(false);
    ZERO_LOAN_CHECKS.set(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| {
        let owner = zero_owner(outside, nested);
        let projection = zero_owner(outside, nested);
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())?;
        let prepared = with_module_fixture_view(
            &projection,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            },
        )?
        .0?;
        prepared.with_source_consumer_v18(
            &mut budget,
            |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            |relation, budget| {
                                scoped_raw_admission_v29::with_checked_source_memory_v29(
                                    relation,
                                    0,
                                    None,
                                    budget,
                                    |_, _| {
                                        completed = true;
                                        Ok(())
                                    },
                                )
                            },
                        )
                    })
                })
            },
        )
    })();
    assert_eq!(budget.storage(), MODULE_FLOOR);
    (result, completed, budget.work(), budget.peak_storage())
}

#[test]
fn original_grid_leader_zero_effect_and_nested_shared_loan_reach_source_memory() {
    for nested in [false, true] {
        let (result, completed) = run_zero(false, nested, 0);
        result.unwrap_or_else(|error| panic!("nested {nested}: {error:?}"));
        assert!(completed);
        assert!(ZERO_OBSERVATIONS.get() >= 2);
        assert!(
            ZERO_LOAN_CHECKS.get() >= 2,
            "custody assertions complete outside protected emission"
        );
    }
}

#[test]
fn original_grid_leader_foreign_ledger_refusal_is_sticky_without_debit() {
    struct Restore(bool, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ZERO_FOREIGN_CUSTODY.set(self.0);
            ZERO_FOREIGN_CUSTODY_OBSERVED.set(self.1);
        }
    }
    let _restore = Restore(
        ZERO_FOREIGN_CUSTODY.replace(true),
        ZERO_FOREIGN_CUSTODY_OBSERVED.replace(false),
    );
    for nested in [false, true] {
        ZERO_FOREIGN_CUSTODY_OBSERVED.set(false);
        let (result, completed) = run_zero(false, nested, 0);
        assert!(ZERO_FOREIGN_CUSTODY_OBSERVED.get());
        assert!(!completed);
        assert!(matches!(
            result,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ))
        ));
    }
}

#[test]
fn original_grid_leader_capture_outside_some_cannot_issue_a_zero_token() {
    let (result, completed) = run_zero(true, true, 0);
    assert!(
        matches!(&result, Err(ProductionSourceOwnedViewErrorV18::Source(
        ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
    )) if *detail == "an Option capability Some target is not uniquely controlled by its exact branch"),
        "{result:?}"
    );
    assert!(!completed);
    assert_eq!(ZERO_OBSERVATIONS.get(), 0);
}

#[test]
fn original_grid_leader_object_reads_and_borrows_require_live_logical_availability() {
    struct Restore(u8, Option<SourceGridLeaderBindingObserverV29>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ZERO_BINDING_FAULT.set(self.0);
            SOURCE_GRID_LEADER_BINDING_OBSERVER_V29.set(self.1);
            ZERO_BINDING_MUTATED.set(self.2);
        }
    }
    let _restore = Restore(
        ZERO_BINDING_FAULT.get(),
        SOURCE_GRID_LEADER_BINDING_OBSERVER_V29.replace(Some(observe_original_zero_binding_v29)),
        ZERO_BINDING_MUTATED.get(),
    );
    for fault in 1..=6 {
        ZERO_BINDING_FAULT.set(fault);
        ZERO_BINDING_MUTATED.set(false);
        let (result, completed) = run_zero(false, true, 0);
        assert!(ZERO_BINDING_MUTATED.get(), "fault {fault}: {result:?}");
        let expected = if fault % 3 == 0 {
            "capability payload is used outside its authenticated enum edge"
        } else {
            "logical GridLeader effect differs from its original source contract"
        };
        assert!(
            matches!(&result, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
        )) if *detail == expected),
            "fault {fault}: {result:?}"
        );
        assert!(!completed);
    }
}

#[test]
fn original_grid_leader_zero_census_rejects_missing_duplicate_wrong_site_and_type_receipts() {
    for fault in 1..=4 {
        let (result, completed) = run_zero(false, true, fault);
        assert!(ZERO_MUTATED.get(), "fault {fault}: {result:?}");
        assert!(result.is_err(), "fault {fault}");
        assert!(!completed);
    }
}

#[test]
fn original_grid_leader_loan_cannot_outlive_or_bypass_replaced_source_storage() {
    struct Restore(u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            ZERO_SOURCE_FAULT.set(self.0);
        }
    }
    let _restore = Restore(ZERO_SOURCE_FAULT.get());
    for (fault, expected) in [
        (1, "source reference referent storage dies with a live loan"),
        (2, "source reference access bypasses a live loan"),
    ] {
        ZERO_SOURCE_FAULT.set(fault);
        let (result, completed) = run_zero(false, true, 0);
        assert!(
            matches!(&result, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
        )) if *detail == expected),
            "fault {fault}: {result:?}"
        );
        assert!(!completed);
        assert_eq!(ZERO_OBSERVATIONS.get(), 0);
        assert_eq!(ZERO_LOAN_CHECKS.get(), 0);
    }
}

#[test]
fn original_grid_leader_zero_type_requires_registered_identity_not_layout() {
    let owner = zero_owner(false, false);
    let original = owner.source_semantic();
    let leader = original
        .callables()
        .iter()
        .find_map(|callable| match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader },
                ..
            } => Some(*grid_leader),
            _ => None,
        })
        .unwrap();
    let mut types = original.types().to_vec();
    let duplicate = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(types[leader.index() as usize].clone());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    assert!(
        source_grid_leader_zero_type_v29(&types, original.callables(), leader, &mut budget)
            .unwrap()
    );
    assert!(
        !source_grid_leader_zero_type_v29(&types, original.callables(), duplicate, &mut budget)
            .unwrap()
    );
    assert!(
        !source_grid_leader_zero_type_v29(&types, original.callables(), UNIT, &mut budget).unwrap()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn original_grid_leader_zero_constructor_requires_exact_constant_type_and_paid_identity() {
    let owner = zero_owner(false, false);
    let source = owner.source_semantic();
    let SemanticStatementKindV1::Assign(original) =
        source.functions()[0].blocks()[2].statements()[0].kind()
    else {
        panic!("original zero-sized assignment");
    };
    let leader = original.destination().ty();
    let ordinal = source
        .callables()
        .iter()
        .position(|callable| {
            matches!(callable,
        SemanticCallableDeclV1::CompilerIntrinsic {
            operation: SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader }, ..
        } if *grid_leader == leader)
        })
        .unwrap();
    // Three constant/type checks, eight nominal layout checks, two per callable.
    let exact = 3 + 8 + 2 * (ordinal + 1);
    for limit in [exact, exact - 1] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let result = source_reference_grid_leader_constant_v29(
            source.types(),
            source.callables(),
            original,
            &mut budget,
        );
        if limit == exact {
            assert_eq!(result.unwrap(), true);
            assert_eq!(budget.work(), exact);
        } else {
            let error = result.unwrap_err();
            assert!(
                matches!(error,
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(denial)) if denial.limit() == limit && denial.actual() > limit),
                "{error:?}"
            );
        }
        assert_eq!(budget.storage(), 0);
    }
    let mut types = source.types().to_vec();
    let duplicate = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(types[leader.index() as usize].clone());
    let assignment = |destination, result, operand| {
        SemanticAssignmentV1::new(
            place(4, destination),
            SemanticRvalueV1::new(result, SemanticRvalueKindV1::Use(operand)),
        )
    };
    let zero = |ty| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            ty,
            SemanticConstantValueV1::ZeroSized,
        ))
    };
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    for candidate in [
        assignment(leader, leader, SemanticOperandV1::Copy(place(4, leader))),
        assignment(UNIT, leader, zero(leader)),
        assignment(leader, UNIT, zero(leader)),
        assignment(UNIT, UNIT, zero(UNIT)),
        assignment(duplicate, duplicate, zero(duplicate)),
    ] {
        assert!(
            !source_reference_grid_leader_constant_v29(
                &types,
                source.callables(),
                &candidate,
                &mut budget,
            )
            .unwrap()
        );
    }
    assert!(
        !source_reference_grid_leader_constant_v29(&types, &[], original, &mut budget,).unwrap()
    );
    assert_eq!(budget.storage(), 0);
}

#[test]
fn original_grid_leader_zero_source_scope_settles_exact_and_short_shared_budgets() {
    let (baseline, completed, work, peak) =
        run_zero_limits(false, true, 0, MODULE_LIMIT, MODULE_LIMIT);
    baseline.unwrap();
    assert!(completed && ZERO_LOAN_CHECKS.get() >= 2);
    let (exact, completed, exact_work, exact_peak) = run_zero_limits(false, true, 0, work, peak);
    exact.unwrap();
    assert!(completed && ZERO_LOAN_CHECKS.get() >= 2);
    assert_eq!((exact_work, exact_peak), (work, peak));
    for (work_limit, storage_limit, storage) in [(work - 1, peak, false), (work, peak - 1, true)] {
        let (result, _, _, _) = run_zero_limits(false, true, 0, work_limit, storage_limit);
        let error = result.expect_err("a real outer scope must select its exact denied resource");
        let mut current: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = current.downcast_ref::<ArgumentResourceV1>() {
                break *resource;
            }
            current = current
                .source()
                .expect("typed resource retained through source-owned error chain");
        };
        match (storage, resource) {
            (true, ArgumentResourceV1::Storage(denial)) => {
                assert_eq!(denial.limit(), storage_limit);
                assert!(denial.actual() > storage_limit);
            }
            (false, ArgumentResourceV1::Work(denial)) => {
                assert_eq!(denial.limit(), work_limit);
                assert!(denial.actual() > work_limit);
            }
            _ => panic!("wrong selected resource: {resource:?}"),
        }
    }
}

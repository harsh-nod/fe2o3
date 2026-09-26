use super::*;

#[derive(Clone, Copy)]
enum Mutation {
    None,
    ClaimedBackedge,
    ForeignBackedge,
}

fn loop_owner(reference: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = cfg_owner(Shape::Cycle);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let old = &semantic.functions()[0];
    let reference_ty = SemanticTypeIdV1::from_index(types.len() as u32);
    if reference {
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([201; 32]),
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
                    U32,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
    }
    let edge =
        |role, target| SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target));
    let goto = |target| SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, target));
    let dereference = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let entry = if reference {
        vec![assign(
            place(4, reference_ty),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: place(3, U32),
            },
        )]
    } else {
        vec![]
    };
    let latch = if reference {
        vec![]
    } else {
        vec![assign(
            place(3, U32),
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(place(5, U32)),
                right: scalar(1),
            },
        )]
    };
    let root = function(
        203,
        old.role(),
        old.abi().clone(),
        vec![
            local(220, UNIT, SemanticLocalRoleV1::Return),
            local(221, CONTEXT, SemanticLocalRoleV1::Argument(0)),
            local(222, CONTEXT, SemanticLocalRoleV1::Argument(1)),
            local(223, U32, SemanticLocalRoleV1::Argument(2)),
            local(
                224,
                if reference { reference_ty } else { U32 },
                SemanticLocalRoleV1::Temporary,
            ),
            local(225, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(211, entry, goto(1)),
            block(
                212,
                vec![assign(
                    place(5, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(if reference {
                        dereference
                    } else {
                        place(3, U32)
                    })),
                )],
                goto(2),
            ),
            block(
                213,
                latch,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant: SemanticOperandV1::Copy(place(
                        if reference { 5 } else { 3 },
                        U32,
                    )),
                    targets: SemanticSwitchTargetsV1::new(
                        vec![SemanticSwitchTargetV1::new(
                            0,
                            edge(SemanticEdgeRoleV1::SwitchValue, 3),
                        )],
                        edge(SemanticEdgeRoleV1::SwitchOtherwise, 1),
                    )
                    .unwrap(),
                },
            ),
            block(214, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(old.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
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

fn with_loop_cursor(
    reference: bool,
    consume: impl FnOnce(
        &ProductionSemanticSsaOwnerV1,
        ExecutionAvailabilityV29<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = loop_owner(reference);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000_000);
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(captured.retained_storage()).unwrap();
    with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
        let floor = budget.storage();
        let result = with_source_reference_plan_v29(instances, budget, |plan, budget| {
            assert_eq!(plan.instances.instances().len(), 1);
            assert_eq!(plan.loans.len(), usize::from(reference));
            if reference {
                assert_eq!(
                    plan.loans[0].representation,
                    SourceReferenceRepresentationV29::StableReferent
                );
            }
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            with_source_reference_availability_v29(
                instances,
                instances.root(),
                Some(&references),
                budget,
                |mut cursor, budget| {
                    let seed = SemanticExecutionBindingV29::context(
                        owner.source_semantic().types(),
                        CONTEXT,
                        ProductionCallOccurrenceV1 {
                            caller: instances.root(),
                            block: SemanticBlockIdV1::from_index(0),
                        },
                        ValueId(90),
                    )
                    .unwrap();
                    cursor.entry_seeds = vec![
                        (1, SemanticValueBindingV1::Execution(seed.clone())),
                        (2, SemanticValueBindingV1::Execution(seed)),
                    ];
                    assert!(cursor.cfg.has_nominal);
                    assert_eq!(cursor.cfg.nominal_locals[1..3], [1, 1]);
                    assert_eq!(cursor.cfg.incoming, [0, 2, 1, 1]);
                    assert_eq!(cursor.cfg.edges.len(), 4);
                    let range = cursor.cfg.ranges[1].clone();
                    assert_eq!(range.len(), usize::from(reference));
                    if reference {
                        let entry = &cursor.cfg.entries[range.start];
                        assert_eq!(entry.local, 4);
                        assert!(entry.reference.is_some());
                        assert!(entry.leaves.is_empty());
                    }
                    consume(&owner, cursor, budget)
                },
            )
        });
        budget.release_storage(budget.storage() - floor).unwrap();
        Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
    })
    .unwrap()
}

fn lower_loop(reference: bool, mutation: Mutation) -> Result<(), ProductionSemanticKirErrorV1> {
    with_loop_cursor(reference, |owner, mut cursor, budget| {
        let instance = cursor.instance;
        let backedge = cursor
            .cfg
            .edges
            .iter_mut()
            .find(|edge| edge.id == SsaEdgeIdV1::new(SsaBlockIdV1::new(2), 1))
            .unwrap();
        assert_eq!(backedge.target, 1);
        assert!(!backedge.claimed);
        match mutation {
            Mutation::None => {}
            Mutation::ClaimedBackedge => backedge.claimed = true,
            Mutation::ForeignBackedge => backedge.target = 3,
        }
        let plan = LoweredFunctionPlanV1 {
            correspondence_owner: ROOT,
            semantic_function: ROOT,
            kernel_ir_function: FunctionId::new("cfg_fixture"),
            role: SemanticKirFunctionRoleV1::KernelEntry,
            parameter_declarations: vec![(2, 3, U32)],
            parameter_types: vec![Type::Scalar(ScalarType::U32)],
            parameter_values: vec![ValueId(100)],
            call_arguments: vec![],
            parameter_local_bindings: vec![PlannedParameterLocalBindingV1::Direct {
                local: 3,
                value: ValueId(100),
                ty: Type::Scalar(ScalarType::U32),
            }],
            parameter_component_bindings: vec![],
            ignored_parameter_bindings: vec![],
            result_types: vec![],
        };
        let mut private = PrivateArrayLazyBudgetV1::new(1, 1024);
        let lowered = lower_one_semantic_function_v1(
            owner.source_semantic(),
            &plan,
            owner.plan_for_function(ROOT).unwrap(),
            &BTreeMap::new(),
            &BTreeMap::new(),
            Some([64, 1, 1]),
            BTreeSet::new(),
            1,
            false,
            1024,
            None,
            &mut private,
            None,
            budget,
            SemanticEmissionPlacementV1 {
                first_block: 17,
                first_value: 100,
            },
            Some(cursor),
        )?;
        assert!(matches!(mutation, Mutation::None));
        assert_eq!(lowered.source_call_instance, Some(instance));
        let body = lowered.function.body.as_ref().unwrap();
        assert_eq!(body.blocks.len(), 4);
        assert_eq!(body.blocks[0].id, BlockId(17));
        let header = body
            .blocks
            .iter()
            .find(|block| block.id == BlockId(18))
            .unwrap();
        let lattice = owner.plan_for_function(ROOT).unwrap().plan();
        let transport = lattice.transport_variables(SsaBlockIdV1::new(1)).unwrap();
        assert_eq!(header.parameters.len(), transport.len());
        assert!(
            header
                .parameters
                .iter()
                .all(|value| value.ty == Type::Scalar(ScalarType::U32))
        );
        let observation = lowered.execution_observation.as_ref().unwrap();
        let Some(Terminator::Branch { target, arguments }) = &body.blocks[0].terminator else {
            panic!("source preheader must keep its original branch");
        };
        assert_eq!(*target, header.id);
        assert_eq!(arguments, &vec![ValueId(100); header.parameters.len()]);
        let latch = body
            .blocks
            .iter()
            .find(|block| block.id == BlockId(19))
            .unwrap();
        let arguments = match latch.terminator.as_ref().unwrap() {
            Terminator::ConditionalBranch {
                else_target,
                else_arguments,
                ..
            } => {
                assert_eq!(*else_target, header.id);
                else_arguments
            }
            Terminator::Switch {
                default_target,
                default_arguments,
                ..
            }
            | Terminator::IntegerSwitch {
                default_target,
                default_arguments,
                ..
            } => {
                assert_eq!(*default_target, header.id);
                default_arguments
            }
            _ => panic!("source latch must retain its conditional backedge"),
        };
        let mut expected = Vec::new();
        for incoming in lattice
            .edge_arguments(SsaEdgeIdV1::new(SsaBlockIdV1::new(2), 1))
            .unwrap()
        {
            match &observation.bindings[&incoming.value()] {
                SemanticValueBindingV1::Value { id, .. } => expected.push(*id),
                SemanticValueBindingV1::SourceReference(binding) => {
                    expected.extend(binding.values.iter().map(|value| value.id));
                }
                _ => panic!("unexpected original backedge binding"),
            }
        }
        assert_eq!(arguments, &expected);
        for variable in transport {
            let phi = SsaValueV1::BlockArgument {
                block: SsaBlockIdV1::new(1),
                variable: *variable,
            };
            let binding = &observation.bindings[&phi];
            if reference {
                assert_eq!(variable.get(), 4);
                assert!(matches!(
                    binding,
                    SemanticValueBindingV1::SourceReference(_)
                ));
            } else {
                assert_eq!(variable.get(), 3);
                assert!(
                    matches!(binding, SemanticValueBindingV1::Value { ty, .. } if *ty == Type::Scalar(ScalarType::U32))
                );
            }
        }
        if !reference {
            assert_eq!(transport.len(), 1);
        }
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| &block.parameters)
                .all(|value| !matches!(value.ty, Type::Execution(_)))
        );
        Ok(())
    })
}

#[test]
fn scalar_loop_header_is_independent_of_unused_nominal_abi_locals() {
    lower_loop(false, Mutation::None).unwrap();
}

#[test]
fn reference_only_loop_uses_checked_source_fixed_point_despite_unrelated_nominal_locals() {
    lower_loop(true, Mutation::None).unwrap();
}

#[test]
fn destination_local_permission_does_not_admit_duplicate_or_foreign_source_edges() {
    for reference in [false, true] {
        for mutation in [Mutation::ClaimedBackedge, Mutation::ForeignBackedge] {
            assert!(matches!(
                lower_loop(reference, mutation),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "execution CFG transport differs from its captured SSA state",
                    ..
                })
            ));
        }
    }
}

#[test]
fn actual_nominal_header_still_requires_all_predecessors_before_entry() {
    lower_cfg_fixture_with_cursor(
        Shape::Cycle,
        |_| {},
        |cursor| {
            let range = cursor.cfg.ranges[1].clone();
            assert!(
                cursor.cfg.entries[range]
                    .iter()
                    .any(|entry| !entry.leaves.is_empty())
            );
            assert_eq!(cursor.cfg.incoming[1], 2);
        },
        |_, _, result| {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "execution CFG transport differs from its captured SSA state",
                    ..
                })
            ));
        },
    );
}

#[test]
fn destination_scan_has_exact_independent_work_and_preserves_storage() {
    for reference in [false, true] {
        with_loop_cursor(reference, |_, cursor, _| {
            let required = 1 + usize::from(reference);
            for floor in [0, 19] {
                let mut exact_work = CanonicalKernelIrWorkBudgetV1::new(floor + required);
                let mut exact = ArgumentBudgetV1::new(&mut exact_work, 71);
                exact.reserve_storage(71)?;
                exact.charge_work(floor)?;
                assert!(!cursor.cfg.destination_has_nominal(1, &mut exact)?);
                assert_eq!(exact.work(), floor + required);
                assert_eq!(exact.storage(), 71);
                let mut short_work = CanonicalKernelIrWorkBudgetV1::new(floor + required - 1);
                let mut short = ArgumentBudgetV1::new(&mut short_work, 71);
                short.reserve_storage(71)?;
                short.charge_work(floor)?;
                assert!(matches!(
                    cursor.cfg.destination_has_nominal(1, &mut short),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                ));
                assert_eq!(short.storage(), 71);
                assert_eq!(short.work(), floor);
                drop(short);
                assert_eq!(short_work.failed_work(), Some(floor + required));
            }
            Ok(())
        })
        .unwrap();
    }
}

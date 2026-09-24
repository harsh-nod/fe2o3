use fe2o3_kernel_ir as kir;
use kir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

fn canonical(read: bool) -> kir::Module {
    use kir::*;
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let input_pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let value = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        value(
            10,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        value(
            11,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        value(
            12,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(10),
                rhs: ValueId(11),
            },
        ),
        value(
            13,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        value(
            14,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(13),
                offset: ValueId(10),
            },
        ),
        value(
            20,
            scalar.clone(),
            OperationKind::Constant(Constant::U32(7)),
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(12),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(1));
    let mut parameters = vec![Type::slice(
        scalar.clone(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    )];
    let mut values = vec![ValueId(0)];
    if read {
        parameters.push(Type::slice(
            scalar.clone(),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        ));
        values.push(ValueId(1));
        entry.operations.extend([
            value(
                15,
                input_pointer.clone(),
                OperationKind::SliceData { slice: ValueId(1) },
            ),
            value(
                16,
                input_pointer,
                OperationKind::GetElementPointer {
                    base: ValueId(15),
                    offset: ValueId(10),
                },
            ),
        ]);
        body.operations.extend([
            value(
                17,
                scalar.clone(),
                OperationKind::Load {
                    pointer: ValueId(16),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            value(
                18,
                scalar,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(17),
                    rhs: ValueId(20),
                },
            ),
        ]);
    }
    body.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(14),
            value: ValueId(if read { 18 } else { 20 }),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    body.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(2));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("conditional-aggregate-test");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(parameters, vec![]),
        values,
        vec![entry, body, exit],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn pending(read: bool, bad_input_bounds: bool) -> ProductionConditionalRankedAnalysisV1 {
    use ProductionSemanticExpressionV2 as Expr;
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    owner_with(
        Case::Positive,
        |entry, body| {
            for operation in entry.iter_mut() {
                if let O::View {
                    result,
                    writable,
                    shape,
                    dynamic_extents,
                    ..
                }
                | O::ViewInSpace {
                    result,
                    writable,
                    shape,
                    dynamic_extents,
                    ..
                } = operation
                    && *result == ProductionRankedValueIdV1::new(2)
                {
                    *writable = false;
                    if bad_input_bounds {
                        *shape = vec![1];
                        dynamic_extents.clear();
                    }
                }
            }
            entry.push(O::SemanticExpression {
                result: ProductionRankedValueIdV1::new(4),
                expression: Expr::Symbol { symbol: 0, scalar },
                numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
            });
            let constant = Expr::Constant { scalar, bits: 7 };
            let expression = if read {
                Expr::Binary {
                    operation: ProductionSemanticBinaryOpV2::Add,
                    scalar,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: Box::new(Expr::Load(ProductionSemanticLoadV2 {
                        block: 1,
                        operation: 0,
                        scalar,
                        allocation_origin: 2,
                        view: local(2),
                        indices: vec![local(0)].into_boxed_slice(),
                    })),
                    rhs: Box::new(constant),
                }
            } else {
                constant
            };
            body.clear();
            if read {
                body.push(O::Access {
                    kind: AccessKindAttr::Read,
                    view: local(2),
                    indices: vec![local(0)],
                });
                body.push(O::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(5),
                    expression,
                    numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
                });
            } else {
                entry.push(O::SemanticExpression {
                    result: ProductionRankedValueIdV1::new(5),
                    expression,
                    numerical_contract: ProductionNumericalContractV2::exact_for(scalar),
                });
            }
            let write = ProductionGpuWriteSiteV2::new(1, body.len() as u32);
            body.push(O::ValueAccess {
                kind: AccessKindAttr::Write,
                view: local(1),
                indices: vec![local(0)],
                value: local(5),
            });
            body.push(O::RequestEffectRefinement {
                contract: ProductionEffectRefinementContractV2::new(
                    1,
                    write,
                    ProductionReferenceOutputSiteV2::new(0, 0, 0),
                    local(1),
                    vec![local(0)],
                    vec![local(4)],
                    vec![local(4)],
                    local(3),
                    local(3),
                    local(3),
                    local(3),
                    local(5),
                    local(5),
                )
                .unwrap(),
                subjects: subjects(),
            });
        },
        stage,
    )
}

fn facts<'a>(
    module: &'a kir::Module,
    budget: &mut Budget<'_>,
) -> kir::ConditionalTotalViewFactsV1<'a> {
    let verified = kir::verify_module_ref(module).unwrap();
    let kir::ConditionalTotalViewAnalysisV1::Established(facts) =
        kir::derive_conditional_total_view_from_verified_v1(
            verified,
            &kir::KernelId::new("kernel"),
            budget,
        )
        .unwrap()
    else {
        panic!("canonical coverage");
    };
    facts
}

fn proposal(read: bool) -> ProductionConditionalRankedProposalV1 {
    ProductionConditionalRankedProposalV1 {
        index: local(0),
        extent: ProductionRankedValueV1::Argument(0),
        write: ProductionGpuWriteSiteV2::new(1, if read { 2 } else { 0 }),
        reads: if read {
            vec![ProductionConditionalReadProposalV1 {
                canonical: kir::FunctionOperationLocation::new(kir::BlockId(1), 0),
                block: 1,
                operation: 0,
            }]
        } else {
            vec![]
        },
    }
}

fn aggregate<'a>(
    module: &'a kir::Module,
    read: bool,
    budget: &mut Budget<'_>,
) -> ProductionConditionalAggregateStateV1<'a> {
    let pending = pending(read, false);
    budget
        .reserve_storage(pending.retained_analysis_storage_v1())
        .unwrap();
    let canonical = facts(module, budget);
    pending
        .verify_conditional_final_graph_v1(canonical, proposal(read), [71; 32], subjects(), budget)
        .unwrap()
        .into_aggregate_state_v1(budget)
        .unwrap()
}

#[test]
fn conditional_aggregate_consumes_actual_fill_and_independently_checked_read_expression() {
    use ProductionConditionalRuntimePremiseV1 as P;
    use kir::ConditionalTotalViewAddressDomainV1::{GlobalLaunch, GuardedOutput};
    for read in [false, true] {
        let module = canonical(read);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.charge_work(19).unwrap();
        let account = budget.work_ledger_identity_v1();
        let state = aggregate(&module, read, &mut budget);
        let floor = budget.storage();
        state
            .with_input_v1(&mut budget, |input, budget| {
                assert!(budget.work_ledger_identity_v1() == account);
                assert_eq!(input.reference_subjects(), subjects());
                assert_eq!(input.outputs().len(), 1);
                assert_eq!(input.reads().len(), usize::from(read));
                assert!(!input.typed_root_commitments().is_empty());
                assert_eq!(input.retained_policy_checked_refinement_staging().len(), 1);
                assert!(input.effect_contract(&input.outputs()[0]).is_some());
                assert!(input.premises().contains(&P::RepresentableAddress {
                    parameter: 0,
                    domain: GlobalLaunch,
                    element_bytes: 4,
                    alignment: 4
                }));
                if read {
                    assert!(input.premises().contains(&P::ReadableInput {
                        parameter: 1,
                        domain: GuardedOutput
                    }));
                    assert!(input.premises().contains(&P::RepresentableAddress {
                        parameter: 1,
                        domain: GlobalLaunch,
                        element_bytes: 4,
                        alignment: 4
                    }));
                    assert!(input.premises().contains(&P::SeparateInputOutput {
                        input: 1,
                        output: 0
                    }));
                }
                let subject = ProductionFinalRankedSubjectV1::Conditional(input);
                assert!(std::ptr::eq(subject.kernel(), input.kernel()));
                assert_eq!(subject.exact_graph_identity(), input.exact_graph_identity());
                input.require_current_graph_v1(budget).unwrap();
            })
            .unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work() > 19);
        assert_eq!(
            state
                .pending_analysis()
                .legacy_report()
                .coverage_summary()
                .total_view_proved(),
            0
        );
    }
}

#[test]
fn conditional_aggregate_rejects_unbound_receipt_and_read_or_cpu_substitution() {
    for case in 0..4 {
        let module = canonical(true);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let pending = pending(true, false);
        if case == 0 {
            let kernel = pending.kernel().unwrap();
            let mut blocks = kernel.blocks().to_vec();
            let mut body = blocks[1].operations().to_vec();
            let O::RequireEffectRefinement { contract, proof } = &body[3] else {
                panic!("staged effect");
            };
            body[3] = O::RequestEffectRefinement {
                contract: contract.clone(),
                subjects: proof.binding().subjects(),
            };
            blocks[1] = ProductionRankedBlockV1::new(body, blocks[1].terminator().clone());
            let recipe = ProductionRankedKernelV1::new(
                kernel.function_name(),
                kernel.argument_count(),
                blocks,
            )
            .unwrap();
            let construction = ProductionConstructionV1::ranked_kernel("unbound", recipe).unwrap();
            let mut session = session();
            let registered = session.register_construction(construction).unwrap();
            assert!(matches!(
                session.construct_registered(registered),
                Err(ProductionSessionErrorV1::RankedRecipe(
                    ProductionRankedKernelErrorV1::Materialization(
                        "unbound functional-refinement request cannot be materialized"
                    )
                ))
            ));
            continue;
        }
        budget
            .reserve_storage(pending.retained_analysis_storage_v1())
            .unwrap();
        let mut proposal = proposal(true);
        if case == 1 {
            proposal.reads[0].operation = 2;
        }
        if case == 2 {
            proposal.reads.clear();
        }
        let cpu = if case == 3 {
            FunctionalRefinementSubjectsV2::new(
                SafeReferenceKindV2::Mir,
                digest(91),
                DigestV1::ZERO,
                digest(2),
                digest(3),
                digest(4),
            )
            .unwrap()
        } else {
            subjects()
        };
        let canonical = facts(&module, &mut budget);
        assert!(
            pending
                .verify_conditional_final_graph_v1(canonical, proposal, [71; 32], cpu, &mut budget)
                .is_err()
        );
    }
}

#[test]
fn conditional_aggregate_does_not_replace_a_failed_input_bounds_proof_with_a_premise() {
    let module = canonical(true);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let pending = pending(true, true);
    assert!(pending.mandatory_bounds_failure().is_some());
    budget
        .reserve_storage(pending.retained_analysis_storage_v1())
        .unwrap();
    let canonical = facts(&module, &mut budget);
    assert!(
        pending
            .verify_conditional_final_graph_v1(
                canonical,
                proposal(true),
                [71; 32],
                subjects(),
                &mut budget
            )
            .is_err()
    );
}

#[test]
fn conditional_aggregate_rejects_callback_budget_substitution_and_floor_release() {
    for replace in [false, true] {
        let module = canonical(false);
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, usize::MAX);
        let state = aggregate(&module, false, &mut budget);
        let result = state.with_input_v1(&mut budget, |_, budget| {
            if replace {
                *budget = Budget::new(Box::leak(Box::new(Work::new(usize::MAX))), usize::MAX);
            } else {
                budget.release_storage(1).unwrap();
            }
        });
        assert!(matches!(
            result,
            Err(ProductionConditionalAggregateErrorV1::Resource(
                Resource::Accounting
            ))
        ));
    }
}

#[test]
fn conditional_aggregate_exhausted_original_ledger_never_exposes_subject() {
    let module = canonical(false);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let state = aggregate(&module, false, &mut budget);
    let floor = budget.storage();
    budget.charge_work(usize::MAX - budget.work()).unwrap();
    assert!(
        state
            .with_input_v1(&mut budget, |_, _| panic!("unpaid subject exposure"))
            .is_err()
    );
    assert_eq!(budget.work(), usize::MAX);
    assert_eq!(budget.storage(), floor);
}

#[test]
fn conditional_aggregate_identical_live_replacement_invalidates_epoch() {
    let module = canonical(false);
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let state = aggregate(&module, false, &mut budget);
    let pending = state.pending_analysis();
    let context = &pending._session.inner.context;
    let function = pending.analysis.payload.function();
    // Even a no-op mutable borrow must invalidate the frozen final graph.
    drop(function.deref_mut(context));
    assert!(
        state
            .with_input_v1(&mut budget, |_, _| panic!("stale subject exposure"))
            .is_err()
    );
}

#[test]
fn conditional_aggregate_refuses_value_less_write_before_staging_a_zero_contract() {
    let original = pending(false, false);
    let kernel = original.kernel().unwrap();
    let mut blocks = kernel.blocks().to_vec();
    let mut entry = blocks[0].operations().to_vec();
    for operation in &mut entry {
        if let O::SemanticExpression {
            result,
            expression: ProductionSemanticExpressionV2::Constant { bits, .. },
            ..
        } = operation
            && *result == ProductionRankedValueIdV1::new(5)
        {
            *bits = 0;
        }
    }
    blocks[0] = ProductionRankedBlockV1::new(entry, blocks[0].terminator().clone());
    let mut body = blocks[1].operations().to_vec();
    body[0] = O::Access {
        kind: AccessKindAttr::Write,
        view: local(1),
        indices: vec![local(0)],
    };
    let O::RequireEffectRefinement { contract, proof } = &body[1] else {
        panic!("staged effect");
    };
    body[1] = O::RequestEffectRefinement {
        contract: contract.clone(),
        subjects: proof.binding().subjects(),
    };
    blocks[1] = ProductionRankedBlockV1::new(body, blocks[1].terminator().clone());
    let recipe =
        ProductionRankedKernelV1::new(kernel.function_name(), kernel.argument_count(), blocks)
            .unwrap();
    let O::RequestEffectRefinement { contract, subjects } = &recipe.blocks()[1].operations()[1]
    else {
        panic!("unstaged effect");
    };
    assert_eq!(
        normalized_effect_refinement_hash_for_kernel_v2(&recipe, 1, 1, contract, *subjects),
        Err(ProductionRankedKernelErrorV1::InvalidReferenceContract)
    );
}

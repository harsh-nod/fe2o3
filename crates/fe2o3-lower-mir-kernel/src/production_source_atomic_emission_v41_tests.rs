use super::*;

// Genuine semantic owner and existing original function emitter, not a hand-made
// KIR graph. This component gate does not replace expanded-root admission or the
// independent real-rustc import gate.
#[test]
fn atomic_physical_v41_original_ten_kinds_five_orders_emit_exact_payloads() {
    use SemanticAtomicOrderingV1::*;
    use SemanticAtomicRmwOpV1::*;
    for kind in [
        Exchange,
        Add,
        Subtract,
        BitAnd,
        BitOr,
        BitXor,
        UnsignedMinimum,
        UnsignedMaximum,
        SignedMinimum,
        SignedMaximum,
    ] {
        for ordering in [
            Relaxed,
            Release,
            Acquire,
            AcquireRelease,
            SequentiallyConsistent,
        ] {
            let completed = std::cell::Cell::new(false);
            with_atomic_source_lowered(
                owner41_atomic(Fault41::None, Some((kind, ordering))),
                |plan, emission, emitted, budget| {
                    assert_eq!(plan.atomic_captures.len(), 1);
                    assert_eq!(plan.atomic_formations.len(), 6);
                    assert_eq!(plan.atomic_uses.len(), 1);
                    assert_eq!(emission.atomic_receipts.len(), 8);
                    assert!(
                        emission
                            .atomic_receipts
                            .iter()
                            .all(|row| row.get().is_some())
                    );
                    assert_eq!(plan.loans.len(), 1);
                    assert_eq!(plan.loans[0].effects.referent_writes, 0);
                    assert_eq!(plan.loans[0].effects.payload_writes, 0);
                    assert_eq!(plan.loans[0].effects.address_observations, 0);
                    assert!(matches!(
                        plan.loans[0].representation,
                        SourceReferenceRepresentationV29::ExistingAllocationBinding(_)
                    ));
                    let scalar = if matches!(kind, SignedMinimum | SignedMaximum) {
                        ScalarType::I32
                    } else {
                        ScalarType::U32
                    };
                    let mut casts = 0;
                    let mut atomics = 0;
                    let mut payloads = 0;
                    for (instance, lowered) in &emitted {
                        let source = plan.instances.instance(*instance).unwrap().declaration();
                        let occurrences = plan.instances.occurrences(*instance).unwrap();
                        let body = lowered.function.body.as_ref().unwrap();
                        for block in &body.blocks {
                            for operation in &block.operations {
                                match &operation.kind {
                                    OperationKind::Cast {
                                        kind: CastKind::PointerToGeneric,
                                        to,
                                        ..
                                    } => {
                                        casts += 1;
                                        assert_eq!(
                                            to,
                                            &Type::pointer(
                                                Type::Scalar(scalar),
                                                AddressSpace::Generic,
                                                AccessMode::ReadWrite
                                            )
                                        );
                                    }
                                    OperationKind::Atomic(value) => {
                                        atomics += 1;
                                        assert_eq!(
                                            value.kind,
                                            lower_atomic_rmw_kind(kind, scalar).unwrap()
                                        );
                                        assert_eq!(value.scope, SynchronizationScope::System);
                                        assert_eq!(value.ordering, lower_atomic_ordering(ordering));
                                        assert_eq!(
                                            value.access.address_space,
                                            AddressSpace::Generic
                                        );
                                        assert_eq!(value.access.alignment, 4);
                                        assert!(!value.access.volatile);
                                        assert_eq!(operation.results.len(), 1);
                                        assert_eq!(operation.results[0].ty, Type::Scalar(scalar));
                                    }
                                    OperationKind::Load { .. } | OperationKind::Store { .. } => {
                                        panic!("ordinary access substituted for atomic")
                                    }
                                    _ => {}
                                }
                            }
                        }
                        for anchor in &lowered.scoped_memory_anchors.as_ref().unwrap().rows {
                            if matches!(
                                anchor.kind,
                                ScopedMemoryAnchorKindV29::Access {
                                    payload: Some(ScopedMemoryPayloadV29::AtomicRmw { .. }),
                                    ..
                                }
                            ) {
                                let block =
                                    body.blocks.iter().find(|b| b.id == anchor.block).unwrap();
                                check_scoped_payload_v29(
                                    source,
                                    &occurrences,
                                    anchor,
                                    &block.operations[anchor.position],
                                    budget,
                                )?;
                                payloads += 1;
                            }
                        }
                    }
                    assert_eq!((casts, atomics, payloads), (1, 1, 1));
                    let mut module = Module::new("atomic_owned_component");
                    module.functions = emitted
                        .iter()
                        .map(|(_, row)| row.function.clone())
                        .collect();
                    module.kernels.push(Kernel::new(
                        "source_reference_component",
                        "source_reference_component",
                        LaunchDomain::D1 {
                            x: LaunchExtent::Static(64),
                        },
                    ));
                    // The legacy profile deliberately rejects Generic transport.
                    // The complete source route uses the distinct V18 storage profile.
                    assert!(
                        verify_module(&module)
                            .unwrap_err()
                            .contains(fe2o3_kernel_ir::DiagnosticCode::InvalidCast)
                    );
                    let mut verification_work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
                    let mut verification_budget =
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
                            &mut verification_work,
                            10_000_000,
                        );
                    let (verified, retained) =
                        fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::
                            from_module_ref_with_verification_budget_v18(
                                &module,
                                ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
                                &mut verification_budget,
                            ).unwrap();
                    let retained_bytes = retained.retained_storage();
                    verification_budget.reserve_storage(retained_bytes).unwrap();
                    assert_eq!(verified.module(), &module);
                    drop(verified);
                    verification_budget.release_storage(retained_bytes).unwrap();
                    assert_eq!(verification_budget.storage(), 0);
                    completed.set(true);
                    Ok(())
                },
            )
            .unwrap_or_else(|error| panic!("{kind:?}/{ordering:?}: {error:?}"));
            assert!(completed.get());
        }
    }
}

#[test]
fn atomic_physical_v41_pre_view_copy_and_move_never_grant_dereference() {
    for fault in [Fault41::PreViewCopy, Fault41::PreViewMove] {
        assert!(
            cells_tests::run_cells(owner41(fault), |_, _| Ok(())).is_err(),
            "{fault:?}"
        );
    }
}

#[test]
fn atomic_physical_v41_partial_claims_cannot_finish() {
    with_atomic_source_lowered(
        owner41_atomic(
            Fault41::None,
            Some((
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::Relaxed,
            )),
        ),
        |_, emission, _, budget| {
            assert_eq!(emission.atomic_receipts.len(), 8);
            let saved = emission.atomic_receipts[7].take().unwrap();
            assert!(emission.finish_source_claims_v29(budget).is_err());
            emission.atomic_receipts[7].set(Some(saved));
            emission.finish_source_claims_v29(budget)?;
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn atomic_physical_v41_foreign_atomic_copy_is_not_an_original_use() {
    let reached = std::cell::Cell::new(false);
    assert!(
        cells_tests::run_cells(
            owner41_atomic(
                Fault41::None,
                Some((
                    SemanticAtomicRmwOpV1::Add,
                    SemanticAtomicOrderingV1::Relaxed
                ))
            ),
            |plan, budget| {
                reached.set(true);
                let emission = SourceReferenceEmissionV29::new(plan, budget)?;
                let row = plan.atomic_uses[0];
                let function = plan
                    .instances
                    .instance(row.site.instance)
                    .unwrap()
                    .declaration();
                let SemanticStatementKindV1::AtomicRmw(original) = function.blocks()
                    [row.site.block.index() as usize]
                    .statements()[row.site.statement.unwrap()]
                .kind() else {
                    panic!("original atomic");
                };
                let denied = emission.atomic_use_v41(row.site, &original.clone(), budget);
                assert!(denied.is_err());
                denied.map(|_| ())
            }
        )
        .is_err()
    );
    assert!(
        reached.get(),
        "original-plan consumer must reach the intended refusal"
    );
}

#[test]
fn atomic_physical_v41_same_typed_plain_node_cannot_recreate_custody() {
    cells_tests::run_cells(owner41(Fault41::None), |plan, budget| {
        let raw = plan
            .nodes
            .iter()
            .enumerate()
            .find(|(_, row)| {
                row.ty == RAW41
                    && row.atomic_custody.is_none()
                    && matches!(row.kind, SourceReferenceNodeKindV29::Plain(_))
            })
            .map(|(i, _)| i)
            .unwrap();
        assert!(source_atomic_node_type_v41(plan, raw, budget)?.is_none());
        assert!(
            plan.atomic_captures
                .iter()
                .all(|capture| capture.node != raw)
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn atomic_physical_v41_denied_atomic_query_storage_publishes_no_receipt() {
    let reached = std::cell::Cell::new(false);
    assert!(
        cells_tests::run_cells(
            owner41_atomic(
                Fault41::None,
                Some((
                    SemanticAtomicRmwOpV1::Add,
                    SemanticAtomicOrderingV1::Relaxed
                ))
            ),
            |plan, budget| {
                reached.set(true);
                let emission = SourceReferenceEmissionV29::new(plan, budget)?;
                let row = plan.atomic_uses[0];
                let function = plan
                    .instances
                    .instance(row.site.instance)
                    .unwrap()
                    .declaration();
                let SemanticStatementKindV1::AtomicRmw(original) = function.blocks()
                    [row.site.block.index() as usize]
                    .statements()[row.site.statement.unwrap()]
                .kind() else {
                    panic!("original atomic");
                };
                let padding = usize::MAX - budget.storage();
                budget.reserve_storage(padding)?;
                let denied = emission.atomic_use_v41(row.site, original, budget);
                assert!(matches!(
                    denied,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                ));
                assert!(
                    emission
                        .atomic_receipts
                        .iter()
                        .all(|claim| claim.get().is_none())
                );
                budget.release_storage(padding)?;
                denied.map(|_| ())
            }
        )
        .is_err()
    );
    assert!(
        reached.get(),
        "original-plan consumer must reach the intended refusal"
    );
}

fn with_atomic_source_lowered(
    owner: ProductionSemanticSsaOwnerV1,
    inspect: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &SourceReferenceEmissionV29<'_, '_>,
        Vec<(ProductionCallInstanceIdV1, LoweredFunctionResultV1)>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let next_placement = |lowered: &LoweredFunctionResultV1| SemanticEmissionPlacementV1 {
        first_block: lowered
            .function
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .map(|block| block.id.0)
            .max()
            .unwrap()
            .checked_add(1)
            .unwrap(),
        first_value: lowered.next_value,
    };
    cells_tests::run_cells(owner, |references, budget| {
        let emission = SourceReferenceEmissionV29::new(references, budget)?;
        let instances = references.instances;
        let semantic = instances.owner().source_semantic();
        with_execution_instance_layouts_v29(references, budget, |signatures, budget| {
            with_execution_call_scope_v29(budget, |scope, budget| {
                let mut closure = ReachableClosureBudgetV1::new(16_384);
                let root = kernel_entry_plan_v1(
                    semantic,
                    ROOT,
                    ROOT,
                    FunctionId::new("source_reference_component"),
                    16_384,
                    &mut closure,
                )?;
                let mut private = PrivateArrayLazyBudgetV1::new(1, 16_384);
                let mut sink = ExecutionDefinedCallSinkV29::new(scope, instances, budget)?;
                let lowered = with_source_reference_availability_v29(
                    instances,
                    instances.root(),
                    Some(&emission),
                    budget,
                    |mut cursor, budget| {
                        lower_one_source_function_with_calls_v29(
                            semantic,
                            &root,
                            instances.instance(instances.root()).unwrap().ssa(),
                            &BTreeMap::new(),
                            signatures,
                            None,
                            BTreeSet::new(),
                            1,
                            true,
                            16_384,
                            None,
                            &mut private,
                            None,
                            budget,
                            SemanticEmissionPlacementV1::default(),
                            Some(cursor),
                            Some(&mut sink),
                            None,
                        )
                    },
                )?;
                let mut position = next_placement(&lowered);
                let mut emitted = vec![(instances.root(), lowered)];
                while let Some(pending) = sink.pop_pending() {
                    let child = pending.child;
                    let plan = signatures.instance_plan_v29(
                        instances,
                        child,
                        pending.kernel_ir_function,
                        position,
                        budget,
                    )?;
                    let (arguments, parameters) = prepare_execution_parameters_with_references_v29(
                        instances,
                        child,
                        pending.arguments,
                        &plan,
                        Some(&emission),
                        budget,
                    )?;
                    let incoming = instances.incoming(child).unwrap();
                    let caller = emitted
                        .iter()
                        .find(|(id, _)| *id == incoming.occurrence().caller)
                        .expect("original caller was emitted first");
                    let original_call = incoming.source();
                    assert_eq!(original_call.arguments().len(), 1);
                    let calls: Vec<_> = caller
                        .1
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter_map(|operation| match &operation.kind {
                            OperationKind::Call { callee, arguments }
                                if callee == &plan.kernel_ir_function =>
                            {
                                Some(arguments)
                            }
                            _ => None,
                        })
                        .collect();
                    assert_eq!(calls, [&arguments]);
                    let lowered = with_source_reference_availability_v29(
                        instances,
                        child,
                        Some(&emission),
                        budget,
                        |mut cursor, budget| {
                            lower_one_source_function_with_calls_v29(
                                semantic,
                                &plan,
                                instances.instance(child).unwrap().ssa(),
                                &BTreeMap::new(),
                                signatures,
                                None,
                                BTreeSet::new(),
                                1,
                                false,
                                16_384,
                                None,
                                &mut private,
                                None,
                                budget,
                                position,
                                Some(cursor.with_call_parameters_v29(parameters)?),
                                Some(&mut sink),
                                None,
                            )
                        },
                    )?;
                    assert_eq!(lowered.source_call_instance, Some(child));
                    position = next_placement(&lowered);
                    emitted.push((child, lowered));
                }
                sink.finish(emitted.iter().map(|(_, row)| row), instances, budget)?;
                // Component emission only: raw expanded-root admission remains a separate consuming gate.
                emission.finish_source_claims_v29(budget)?;
                assert_eq!(emitted.len(), instances.instances().len());
                inspect(references, &emission, emitted, budget)
            })
        })
    })
}

#[test]
fn atomic_physical_v41_actual_atomic_value_and_access_substitution_refuse() {
    with_atomic_source_lowered(
        owner41_atomic(
            Fault41::None,
            Some((
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::AcquireRelease,
            )),
        ),
        |plan, _, emitted, budget| {
            let row = plan.atomic_uses[0];
            let function = plan
                .instances
                .instance(row.site.instance)
                .unwrap()
                .declaration();
            let SemanticStatementKindV1::AtomicRmw(original) = function.blocks()
                [row.site.block.index() as usize]
                .statements()[row.site.statement.unwrap()]
            .kind() else {
                panic!("original atomic");
            };
            let lowered = &emitted
                .iter()
                .find(|(id, _)| *id == row.site.instance)
                .unwrap()
                .1;
            let operation = lowered
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| matches!(operation.kind, OperationKind::Atomic(_)))
                .unwrap();
            let OperationKind::Atomic(actual) = &operation.kind else {
                unreachable!()
            };
            let output = &operation.results[0];
            let rhs = actual.value.unwrap();
            check_atomic_operation_v41(
                original,
                operation,
                actual.pointer,
                rhs,
                output.id,
                &output.ty,
                actual.access,
                budget,
            )?;
            for field in 0..4 {
                let mut changed = operation.clone();
                let OperationKind::Atomic(value) = &mut changed.kind else {
                    unreachable!()
                };
                match field {
                    0 => value.value = Some(output.id), // result cannot be its own original RHS
                    1 => value.access.alignment = 8,
                    2 => value.access.volatile = true,
                    3 => value.access.address_space = AddressSpace::Global,
                    _ => unreachable!(),
                }
                assert_ne!(&changed, operation);
                assert!(
                    check_atomic_operation_v41(
                        original,
                        &changed,
                        actual.pointer,
                        rhs,
                        output.id,
                        &output.ty,
                        actual.access,
                        budget
                    )
                    .is_err(),
                    "field {field}"
                );
            }
            Ok(())
        },
    )
    .unwrap();
}

#[path = "production_source_atomic_address_relation_v41_tests.rs"]
mod final_memory_v41_tests;

#[path = "production_atomic_destination_v41_tests.rs"]
mod atomic_destination_v41_tests;

#[path = "production_source_atomic_receiver_backing_v41_tests.rs"]
mod atomic_receiver_backing_v41_tests;

#[path = "production_source_atomic_address_of_v41_tests.rs"]
mod atomic_address_of_v41_tests;

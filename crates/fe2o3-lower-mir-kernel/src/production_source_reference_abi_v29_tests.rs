use super::*;

#[path = "production_source_reference_enum_v29_tests.rs"]
mod enum_tests;

#[path = "production_execution_instance_layouts_v29_tests.rs"]
mod instance_layout_tests;
#[path = "production_execution_instance_layout_resources_v29_tests.rs"]
mod instance_layout_resource_tests;

#[test]
fn source_reference_later_addressable_instance_is_not_hidden_by_signature_cache() {
    run_owner(later_addressable_instance_owner(), |plan, budget| {
        assert_eq!(plan.instances.instances().len(), 3);
        assert_eq!(plan.loans.len(), 2);
        let first = plan.instances.id_at(1).unwrap();
        let later = plan.instances.id_at(2).unwrap();
        assert_eq!(
            plan.instances.instance(first).unwrap().function(),
            plan.instances.instance(later).unwrap().function()
        );
        assert_eq!(
            plan.loans[0].representation,
            SourceReferenceRepresentationV29::StableReferent
        );
        assert_eq!(
            plan.loans[1].representation,
            SourceReferenceRepresentationV29::NeedsAddressable(
                SourceReferenceCellNeedV29::ReferentWrite
            )
        );
        let signature = execution_function_signature_with_references_v29(
            plan.instances,
            first,
            Some(plan),
            budget,
        )?;
        assert_eq!(signature.parameter_types, [Type::Scalar(ScalarType::U64)]);
        assert!(matches!(
            execution_function_signature_with_references_v29(
                plan.instances,
                later,
                Some(plan),
                budget,
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference requires checked addressable storage and writeback",
                ..
            })
        ));
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_availability_header_and_constructor_denials_survive_recovery() {
    for constructor in [false, true] {
        let entered = std::cell::Cell::new(false);
        let result = run(Case::Shared, |plan, budget| {
            let references = SourceReferenceEmissionV29::new(plan, budget)?;
            let free = if constructor {
                source_reference_availability_headers_v29::<()>()?
            } else {
                0
            };
            let padding = usize::MAX - budget.storage() - free;
            budget.reserve_storage(padding)?;
            let before = budget.work();
            entered.set(true);
            let error = with_source_reference_availability_v29::<()>(
                plan.instances,
                plan.root,
                Some(&references),
                budget,
                |_, _| panic!("denied cursor must not reach its consumer"),
            )
            .unwrap_err();
            let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
                panic!("availability did not report its storage denial");
            };
            assert!(matches!(first, ArgumentResourceV1::Storage(_)));
            assert_eq!(budget.work() - before, if constructor { 17 } else { 5 });
            budget.release_storage(padding)?;
            assert_eq!(plan.failure.get(), Some(first));
            assert!(matches!(references.check(budget),
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
            Ok(())
        });
        assert!(entered.get());
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ));
    }
}

#[test]
fn source_reference_signature_and_instance_refuse_foreign_slot_before_debit() {
    for instance_plan in [false, true] {
        let result = run(Case::Shared, |plan, budget| {
            let child = capture_instance(plan, 0);
            let before = (budget.work(), budget.storage());
            let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
            let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
            let result = if instance_plan {
                execution_instance_plan_with_references_v29(
                    plan.instances,
                    child,
                    FunctionId::new("reference.helper"),
                    SemanticEmissionPlacementV1::default(),
                    Some(plan),
                    &mut foreign,
                )
                .map(|_| ())
            } else {
                execution_function_signature_with_references_v29(
                    plan.instances,
                    child,
                    Some(plan),
                    &mut foreign,
                )
                .map(|_| ())
            };
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            assert_eq!((budget.work(), budget.storage()), before);
            assert_eq!(plan.failure.get(), Some(ArgumentResourceV1::Accounting));
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
    }
}

#[test]
fn source_reference_signature_constructor_denial_survives_credit_recovery() {
    let result = run(Case::Shared, |plan, budget| {
        let child = capture_instance(plan, 0);
        let declaration = plan.instances.instance(child).unwrap().declaration();
        let abi = declaration.abi();
        let sources = abi.source_input_types().len();
        let adjusted = abi.adjusted_arguments().len();
        let abi_storage = sources * std::mem::size_of::<Option<SemanticLocalIdV1>>()
            + adjusted * std::mem::size_of::<SemanticLocalIdV1>();
        assert!(abi_storage > 0);
        let free = source_reference_emission_headers_v29::<LoweredFunctionSignatureV1>()?
            + source_reference_emission_headers_v29::<ExecutionFunctionLayoutV29>()?
            + abi_storage
            - 1;
        let padding = usize::MAX - budget.storage() - free;
        budget.reserve_storage(padding)?;
        let before = budget.work();
        let Err(error) = execution_function_signature_with_references_v29(
            plan.instances,
            child,
            Some(plan),
            budget,
        ) else {
            panic!("signature construction must exhaust its ABI reservation");
        };
        let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = error else {
            panic!("signature did not report its storage denial");
        };
        assert!(matches!(first, ArgumentResourceV1::Storage(_)));
        assert_eq!(
            budget.work() - before,
            25 + 8 + 2 * declaration.locals().len() + sources + adjusted
        );
        budget.release_storage(padding)?;
        assert_eq!(plan.failure.get(), Some(first));
        assert!(matches!(plan.check_owner(plan.instances, budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
        Ok(())
    });
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn source_reference_block_index_joins_original_instance_block_and_local_type() {
    run(Case::Shared, |plan, budget| {
        let mut references = SourceReferenceEmissionV29::new(plan, budget)?;
        let probes = usize::BITS as usize - references.block_sites.len().leading_zeros() as usize;
        for (ordinal, instance) in plan.instances.instances().iter().enumerate() {
            let instance_id = plan.instances.id_at(ordinal).unwrap();
            for (block, _) in instance.declaration().blocks().iter().enumerate() {
                let before = budget.work();
                let node = references.block_node(
                    instance_id,
                    SemanticBlockIdV1::from_index(block as u32),
                    SemanticLocalIdV1::from_index(0),
                    budget,
                )?;
                assert!(budget.work() - before <= 14 + 13 * probes);
                if let Some(node) = node {
                    assert_eq!(plan.nodes[node].ty, instance.declaration().locals()[0].ty());
                }
            }
        }
        assert!(references.block_sites.len() > 1);
        let key = references.block_sites[0].0;
        references.block_sites[0].1 = references.block_sites[1].1;
        assert!(
            references
                .block_node(
                    plan.instances.id_at(key.0).unwrap(),
                    SemanticBlockIdV1::from_index(key.1),
                    SemanticLocalIdV1::from_index(0),
                    budget,
                )
                .is_err()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_index_denial_stays_sticky_after_ignored_result() {
    let entered = std::cell::Cell::new(false);
    let result = run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let site = plan.loans[0].site;
        // The emission check and concrete-slot check each cost five; the
        // indexed probe costs eight. Leave seven after both checks, not enough
        // for the probe but enough for a later check without the sticky latch.
        budget.charge_work(usize::MAX - budget.work() - 17)?;
        entered.set(true);
        let first = references.loan_at(site, budget).unwrap_err();
        let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = first else {
            panic!("indexed denial did not report its resource");
        };
        assert!(matches!(first, ArgumentResourceV1::Work(_)));
        assert_eq!(plan.failure.get(), Some(first));
        let before = budget.work();
        assert!(matches!(references.check(budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
        assert_eq!(budget.work(), before);
        Ok(())
    });
    assert!(entered.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(
                _
            ))
        )
    ));
}

#[test]
fn source_reference_owned_storage_denial_survives_credit_recovery() {
    let entered = std::cell::Cell::new(false);
    let result = run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let padding = usize::MAX - budget.storage();
        budget.reserve_storage(padding)?;
        entered.set(true);
        let first = source_reference_owned_vec_v29::<u64>(plan, 1, budget).unwrap_err();
        let ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(first) = first else {
            panic!("owned storage denial did not report its resource");
        };
        assert!(matches!(first, ArgumentResourceV1::Storage(_)));
        budget.release_storage(padding)?;
        assert_eq!(plan.failure.get(), Some(first));
        assert!(matches!(references.check(budget),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
        Ok(())
    });
    assert!(entered.get());
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(_)
            )
        )
    ));
}

#[test]
fn source_reference_abi_plain_embedded_pointer_policy_is_unchanged() {
    run(Case::Shared, |plan, budget| {
        let child = capture_instance(plan, 0);
        assert!(execution_function_signature_v29(plan.instances, child, budget).is_err());
        let checked = execution_function_signature_with_references_v29(
            plan.instances,
            child,
            Some(plan),
            budget,
        )?;
        assert_eq!(checked.parameter_types, [Type::Scalar(ScalarType::U64)]);
        assert_eq!(checked.parameter_semantic_types, [CAPTURE]);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_abi_checks_later_instance_before_signature_cache_reuse() {
    run(Case::Shared, |plan, budget| {
        let first = capture_instance(plan, 0);
        let later = capture_instance(plan, 1);
        let first = execution_function_signature_with_references_v29(
            plan.instances,
            first,
            Some(plan),
            budget,
        )?;
        let mut later = execution_function_signature_with_references_v29(
            plan.instances,
            later,
            Some(plan),
            budget,
        )?;
        source_reference_same_signature_v29(&first, &later, budget)?;
        later.parameter_types[0] = Type::Scalar(ScalarType::U32);
        assert!(source_reference_same_signature_v29(&first, &later, budget).is_err());
        later.parameter_types[0] = Type::Scalar(ScalarType::U64);
        later.call_arguments[0].component = Some(1);
        assert!(source_reference_same_signature_v29(&first, &later, budget).is_err());
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_abi_call_roster_joins_original_occurrence_and_component_order() {
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let child = capture_instance(plan, 0);
        let incoming = plan.instances.incoming(child).unwrap();
        let signature = execution_function_signature_with_references_v29(
            plan.instances,
            child,
            Some(plan),
            budget,
        )?;
        let binding = capture_binding(&references, child, ValueId(913), budget);
        with_execution_call_scope_v29(budget, |scope, budget| {
            let caller = plan
                .instances
                .instance(incoming.occurrence().caller)
                .unwrap();
            assert_eq!(
                semantic_operand_type(&incoming.source().arguments()[0]),
                CAPTURE
            );
            let mut origin = PreparedExecutionCallOriginV29 {
                scope,
                ledger: budget.work_ledger_identity_v1(),
                source: ExecutionCallSourceV29::from_instances(plan.instances, budget)?,
                function: caller.function(),
                occurrence: incoming.occurrence(),
                callee: plan.instances.instance(child).unwrap().function(),
                projections: signature.call_arguments,
                parameter_types: signature.parameter_types,
            };
            assert!(source_reference_call_argument_shape_v29(
                &references,
                &origin,
                0,
                &binding,
                budget
            )?);
            origin.projections[0].component = Some(1);
            assert!(
                source_reference_call_argument_shape_v29(&references, &origin, 0, &binding, budget)
                    .is_err()
            );
            origin.projections[0].component = Some(0);
            origin.occurrence = plan
                .instances
                .incoming(capture_instance(plan, 1))
                .unwrap()
                .occurrence();
            assert!(
                source_reference_call_argument_shape_v29(&references, &origin, 0, &binding, budget)
                    .is_err()
            );
            Ok(())
        })
    })
    .unwrap();
}

#[test]
fn source_reference_emission_owned_headers_have_independent_exact_and_one_short_cuts() {
    use std::mem::size_of;
    #[repr(align(128))]
    struct Aligned([u8; 4096]);
    let _ = Aligned([0; 4096]).0[0];
    let expected =
        size_of::<Aligned>() + 2 * size_of::<Result<Aligned, ProductionSemanticKirErrorV1>>();
    assert_eq!(
        source_reference_emission_headers_v29::<Aligned>().unwrap(),
        expected
    );
    let floor = 19;
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, floor + expected - usize::from(short));
        budget.reserve_storage(floor).unwrap();
        let result = source_reference_emission_prepay_v29::<Aligned>(&mut budget);
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
            assert_eq!(budget.storage(), floor);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), floor + expected);
            budget.release_storage(expected).unwrap();
            assert_eq!(budget.storage(), floor);
        }
    }
    type Payload = Box<dyn std::any::Any + Send>;
    let cursor = size_of::<ExecutionAvailabilityV29<'_>>()
        + 2 * size_of::<Result<ExecutionAvailabilityV29<'_>, ProductionSemanticKirErrorV1>>();
    let scope = cursor
        + 2 * size_of::<Result<Aligned, ProductionSemanticKirErrorV1>>()
        + size_of::<Result<Result<Aligned, ProductionSemanticKirErrorV1>, Payload>>()
        + size_of::<[Option<Payload>; 2]>()
        + size_of::<Result<(), Payload>>()
        + size_of::<Option<usize>>()
        + size_of::<Option<ArgumentResourceV1>>()
        + size_of::<Option<CompletedExecutionAvailabilityV1<'static>>>()
        + size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>()
        + 4 * size_of::<usize>();
    assert_eq!(
        source_reference_availability_headers_v29::<Aligned>().unwrap(),
        scope
    );
}

#[test]
fn source_reference_emission_growth_prepays_replacement_without_losing_old_capacity() {
    use std::mem::size_of;
    let floor = 23;
    let headers =
        size_of::<Vec<u64>>() + 2 * size_of::<Result<Vec<u64>, ProductionSemanticKirErrorV1>>();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    let mut rows = source_reference_emission_vec_v29::<u64>(0, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor + headers);
    source_reference_emission_push_v29(&mut rows, 31, &mut budget).unwrap();
    assert_eq!(rows, [31]);
    assert_eq!(
        budget.storage(),
        floor + headers + rows.capacity() * size_of::<u64>()
    );
    let retained = budget.storage() - floor;
    drop(rows);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, floor + 2 * headers - 1);
    budget.reserve_storage(floor).unwrap();
    let mut rows = source_reference_emission_vec_v29::<u64>(0, &mut budget).unwrap();
    let before = budget.storage();
    assert!(source_reference_emission_push_v29(&mut rows, 31, &mut budget).is_err());
    assert!(rows.is_empty());
    assert_eq!(rows.capacity(), 0);
    assert_eq!(budget.storage(), before);
    drop(rows);
    budget.release_storage(headers).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn source_reference_availability_rejects_undercut_before_refunding_and_drops_output() {
    use std::cell::Cell;
    struct Output<'a>(&'a Cell<usize>);
    impl Drop for Output<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let dropped = Cell::new(0);
        let entered = Cell::new(false);
        let after_undercut = Cell::new(0);
        let result = with_source_reference_availability_v29(
            plan.instances,
            plan.root,
            Some(&references),
            budget,
            |cursor, budget| {
                entered.set(true);
                drop(cursor);
                budget.release_storage(1)?;
                after_undercut.set(budget.storage());
                Ok(Output(&dropped))
            },
        );
        assert!(entered.get());
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), after_undercut.get());
        assert!(budget.storage() >= references.floor);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_availability_preserves_callback_error_over_later_undercut() {
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let entered = std::cell::Cell::new(false);
        let result: Result<(), _> = with_source_reference_availability_v29(
            plan.instances,
            plan.root,
            Some(&references),
            budget,
            |cursor, budget| {
                entered.set(true);
                drop(cursor);
                budget.release_storage(1)?;
                Err(ArgumentResourceV1::Arithmetic.into())
            },
        );
        assert!(entered.get());
        assert!(matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Arithmetic
                )
            )
        ));
        assert_eq!(plan.failure.get(), None);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_availability_destroys_nested_panic_payloads_before_recovery() {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Payload(Arc<AtomicUsize>, bool);
    impl Drop for Payload {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
            if self.1 {
                std::panic::panic_any(Payload(self.0.clone(), false));
            }
        }
    }
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let floor = budget.storage();
        let dropped = Arc::new(AtomicUsize::new(0));
        let entered = std::cell::Cell::new(false);
        let result: Result<(), _> = with_source_reference_availability_v29(
            plan.instances,
            plan.root,
            Some(&references),
            budget,
            |cursor, _| {
                entered.set(true);
                drop(cursor);
                std::panic::panic_any(Payload(dropped.clone(), true));
            },
        );
        assert!(entered.get());
        assert!(result.is_err());
        assert_eq!(dropped.load(Ordering::SeqCst), 2);
        assert_eq!(budget.storage(), floor);
        Ok(())
    })
    .unwrap();
}

fn with_source_lowered(
    owner: ProductionSemanticSsaOwnerV1,
    inspect: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &[(ProductionCallInstanceIdV1, LoweredFunctionResultV1)],
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    load_tests::with_source_lowered_cursor(owner, |_| {}, inspect)
}

#[test]
fn source_reference_actual_lowering_joins_borrow_load_slot_parameter_and_call() {
    with_source_lowered(owner(Case::Shared), |plan, emitted, budget| {
        assert_eq!(plan.loans.len(), 2);
        let mut produced = BTreeSet::new();
        for (loan_id, loan) in plan.loans.iter().enumerate() {
            let source = plan.instances.instance(loan.site.instance).unwrap();
            let statement = &source.declaration().blocks()[loan.site.block.index() as usize]
                .statements()[loan.site.statement.unwrap()];
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                panic!("borrow assignment")
            };
            let SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place,
            } = assignment.value().kind()
            else {
                panic!("original shared borrow")
            };
            assert_eq!(place.ty(), WORD);
            assert_eq!(place.local().index(), 1);
            let (_, lowered) = emitted
                .iter()
                .find(|(id, _)| *id == loan.site.instance)
                .unwrap();
            let spans: Vec<_> = lowered
                .statement_operation_spans
                .iter()
                .filter(|span| {
                    span.semantic_block == loan.site.block
                        && span.statement_ordinal as usize == loan.site.statement.unwrap()
                })
                .collect();
            assert_eq!(spans.len(), 1);
            let span = spans[0];
            let body = lowered.function.body.as_ref().unwrap();
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == span.kernel_ir_block)
                .unwrap();
            let operations = &block.operations[span.first_operation_ordinal as usize
                ..(span.first_operation_ordinal + span.operation_count) as usize];
            let loads: Vec<_> = operations
                .iter()
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Load { pointer, access } => Some((operation, *pointer, access)),
                    _ => None,
                })
                .collect();
            assert_eq!(loads.len(), 1);
            let (load, pointer, access) = loads[0];
            assert_eq!(access.address_space, AddressSpace::Private);
            assert_eq!(load.results.len(), 1);
            let value = &load.results[0];
            assert_eq!(value.ty, Type::Scalar(ScalarType::U64));
            assert!(produced.insert(value.id));
            let slots: Vec<_> = lowered
                .scoped_slot_origins
                .as_ref()
                .unwrap()
                .iter()
                .filter(|slot| slot.legacy_local().unwrap() == place.local().index())
                .collect();
            assert_eq!(slots.len(), 1);
            assert_eq!(
                (slots[0].pointer, slots[0].semantic_type),
                (pointer, place.ty())
            );
            assert_eq!(
                source.declaration().locals()[place.local().index() as usize].role(),
                SemanticLocalRoleV1::Argument(0)
            );
            assert_eq!(
                lowered.function.signature.parameters,
                [Type::Scalar(ScalarType::U64)]
            );
            assert_eq!(body.parameters.len(), 1);
            let parameter = body.parameters[0];
            assert!(body.blocks.iter().flat_map(|block| &block.operations).any(|operation|
                matches!(operation.kind, OperationKind::Store { pointer: actual, value, .. }
                    if actual == pointer && value == parameter)));
            let observation = lowered.execution_observation.as_ref().unwrap();
            let references: Vec<_> = observation
                .bindings
                .values()
                .filter_map(|binding| match binding {
                    SemanticValueBindingV1::SourceReference(binding) if binding.origin == SourceReferenceBindingOriginV29::SingleLoan(loan_id) => {
                        Some(binding)
                    }
                    _ => None,
                })
                .collect();
            assert!(!references.is_empty());
            for binding in references {
                source_reference_validate_binding_v29(plan, binding, budget)?;
                assert_eq!(binding.values, [value.clone()]);
                assert_eq!(binding.source_type, assignment.destination().ty());
            }
            let child = plan.instances.calls(loan.site.instance).unwrap()[0]
                .child()
                .unwrap();
            let (_, lowered_child) = emitted.iter().find(|(id, _)| *id == child).unwrap();
            let calls: Vec<_> = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| match &operation.kind {
                    OperationKind::Call { callee, arguments }
                        if callee == &lowered_child.function.id =>
                    {
                        Some(arguments)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].as_slice(), [value.id]);
            let observation = lowered_child.execution_observation.as_ref().unwrap();
            let captured = observation.locals[3].as_ref().unwrap().value().unwrap();
            assert_eq!(captured.1, Type::Scalar(ScalarType::U64));
            assert_eq!(
                captured.0,
                lowered_child.function.body.as_ref().unwrap().parameters[0]
            );
            assert_eq!(
                lowered_child.function.signature.parameters,
                [Type::Scalar(ScalarType::U64)]
            );
        }
        assert_eq!(produced.len(), 2);
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_reference_retained_occurrence_does_not_mint_ssa_definition() {
    run(Case::Shared, |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        for loan in &plan.loans {
            with_source_reference_availability_v29(
                plan.instances,
                loan.site.instance,
                Some(&references),
                budget,
                |mut cursor, budget| {
                    cursor.begin_block(loan.site.block, budget)?;
                    let site = execution_site_v29(
                        loan.site.block,
                        Some(u32::try_from(loan.site.statement.unwrap()).unwrap()),
                    );
                    let operand = ExecutionOperandV29::RvaluePlace;
                    let role = ExecutionEventV29::BaseUse;
                    let event = cursor
                        .find_occurrence(site, operand, role, budget)?
                        .expect("the retained borrow has an exact source occurrence");
                    assert!(!cursor.occurrences.events()[event].is_promoted());
                    assert!(cursor.occurrences.events()[event].resolved().is_none());
                    assert!(cursor.find_event(site, operand, role, budget).is_err());
                    assert!(!cursor.claimed[event]);
                    let wrong_block = ExecutionSiteV29::Statement {
                        block: SsaBlockIdV1::new(loan.site.block.index() + 1),
                        statement: 0,
                    };
                    assert!(
                        cursor
                            .find_occurrence(wrong_block, operand, role, budget)
                            .is_err()
                    );
                    cursor.claim_events(&[event], budget)?;
                    assert!(cursor.find_occurrence(site, operand, role, budget).is_err());
                    Ok(())
                },
            )?;
        }
        Ok(())
    })
    .unwrap();
}

fn reference_loop_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Shared, |_, functions| {
        let edge = |role, target| {
            SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
        };
        functions[1] = function(
            20,
            false,
            WORD,
            vec![
                local(20, UNIT, SemanticLocalRoleV1::Return),
                local(21, WORD, SemanticLocalRoleV1::Argument(0)),
                local(22, REFERENCE, SemanticLocalRoleV1::Temporary),
                local(23, CAPTURE, SemanticLocalRoleV1::Temporary),
                local(24, WORD, SemanticLocalRoleV1::Temporary),
            ],
            vec![
                block(
                    20,
                    vec![
                        assign(
                            place(2, REFERENCE),
                            SemanticRvalueKindV1::Borrow {
                                kind: SemanticBorrowKindV1::Shared,
                                place: place(1, WORD),
                            },
                        ),
                        assign(
                            place(3, CAPTURE),
                            SemanticRvalueKindV1::Aggregate(
                                SemanticAggregateRvalueV1::new(
                                    SemanticAggregateKindV1::Tuple,
                                    vec![SemanticOperandV1::Move(place(2, REFERENCE))],
                                )
                                .unwrap(),
                            ),
                        ),
                        dead(2),
                    ],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    21,
                    vec![],
                    SemanticTerminatorKindV1::SwitchInt {
                        discriminant: SemanticOperandV1::Copy(place(1, WORD)),
                        targets: SemanticSwitchTargetsV1::new(
                            vec![SemanticSwitchTargetV1::new(
                                0,
                                edge(SemanticEdgeRoleV1::SwitchValue, 3),
                            )],
                            edge(SemanticEdgeRoleV1::SwitchOtherwise, 2),
                        )
                        .unwrap(),
                    },
                ),
                block(
                    22,
                    vec![assign(
                        place(4, WORD),
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                            3,
                            &[
                                (SemanticProjectionKindV1::Field(0), REFERENCE),
                                (SemanticProjectionKindV1::Dereference, WORD),
                            ],
                        ))),
                    )],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 1)),
                ),
                block(
                    23,
                    vec![],
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 4),
                ),
                block(24, vec![], SemanticTerminatorKindV1::Return),
            ],
        );
    })
}

#[test]
fn source_reference_cfg_header_requires_forward_value_and_bounds_predecessor_arrivals() {
    run_owner(reference_loop_owner(), |plan, budget| {
        let references = SourceReferenceEmissionV29::new(plan, budget)?;
        let instance = plan.loans[0].site.instance;
        with_source_reference_availability_v29(
            plan.instances,
            instance,
            Some(&references),
            budget,
            |mut cursor, budget| {
                let header = SemanticBlockIdV1::from_index(1);
                let block = header.index() as usize;
                assert_eq!(cursor.cfg.incoming[block], 2);
                assert!(!cursor.cfg.has_nominal);
                let enter = |cursor: &mut ExecutionAvailabilityV29<'_>,
                             budget: &mut ArgumentBudgetV1<'_>| {
                    cursor
                        .cfg
                        .enter(header, &mut cursor.current, &cursor.seen, budget)
                };
                assert!(enter(&mut cursor, budget).is_err());
                cursor.cfg.arrived[block] = 1;
                assert!(enter(&mut cursor, budget).is_err());
                let entries = &mut cursor.cfg.entries[cursor.cfg.ranges[block].clone()];
                assert!(!entries.is_empty());
                for entry in entries {
                    assert!(entry.reference.is_some());
                    assert!(entry.leaves.is_empty());
                    entry.value = Some(SsaValueV1::BlockArgument {
                        block: SsaBlockIdV1::new(header.index()),
                        variable: fe2o3_mir_model::SsaVariableIdV1::new(entry.local),
                    });
                }
                enter(&mut cursor, budget)?;
                cursor.cfg.has_nominal = true;
                // Unrelated nominal locals do not change this reference-only header.
                assert!(!cursor.cfg.destination_has_nominal(block, budget)?);
                enter(&mut cursor, budget)?;
                cursor.cfg.arrived[block] = 2;
                enter(&mut cursor, budget)?;
                cursor.cfg.has_nominal = false;
                cursor.cfg.arrived[block] = 3;
                assert!(enter(&mut cursor, budget).is_err());
                Ok(())
            },
        )
    })
    .unwrap();
}

#[test]
fn source_reference_actual_lowering_transports_invariant_loan_on_real_backedge() {
    with_source_lowered(reference_loop_owner(), |plan, emitted, budget| {
        assert_eq!(plan.loans.len(), 2);
        for (loan_id, loan) in plan.loans.iter().enumerate() {
            assert_eq!(loan.effects.referent_reads, 2);
            assert_eq!(
                plan.require_promoted(loan_id, budget)?,
                SourceReferenceRepresentationV29::StableReferent
            );
            let header = plan
                .blocks
                .iter()
                .find(|row| row.instance == loan.site.instance && row.block.index() == 1)
                .unwrap();
            let capture = plan.states[header.entry][3].node.unwrap();
            let SourceReferenceNodeKindV29::Aggregate { first, count: 1 } =
                plan.nodes[capture].kind
            else {
                panic!("same captured loan reaches loop header")
            };
            assert_eq!(
                plan.nodes[plan.children[first]].kind,
                SourceReferenceNodeKindV29::Loan(loan_id)
            );
            let (_, lowered) = emitted
                .iter()
                .find(|(id, _)| *id == loan.site.instance)
                .unwrap();
            let body = lowered.function.body.as_ref().unwrap();
            let physical = |semantic| {
                lowered
                    .blocks
                    .iter()
                    .find(|block| block.semantic_block.index() == semantic)
                    .unwrap()
                    .kernel_ir_block
            };
            let header = body
                .blocks
                .iter()
                .find(|block| block.id == physical(1))
                .unwrap();
            let latch = body
                .blocks
                .iter()
                .find(|block| block.id == physical(2))
                .unwrap();
            let Some(Terminator::Branch { target, arguments }) = &latch.terminator else {
                panic!("retained source backedge must remain a branch")
            };
            assert_eq!(*target, header.id);
            assert_eq!(arguments.len(), header.parameters.len());
            // The unchanged loan's definition dominates the loop. Pruned SSA
            // transports that definition without introducing a redundant phi.
            assert!(
                plan.instances
                    .instance(loan.site.instance)
                    .unwrap()
                    .ssa()
                    .plan()
                    .transport_variables(SsaBlockIdV1::new(1))
                    .unwrap()
                    .is_empty()
            );
            assert!(header.parameters.is_empty());
            assert!(
                header
                    .parameters
                    .iter()
                    .all(|value| value.ty == Type::Scalar(ScalarType::U64))
            );
            assert!(
                latch
                    .operations
                    .iter()
                    .all(|operation| !matches!(operation.kind, OperationKind::Load { .. }))
            );
        }
        let mut module = Module::new("source_reference_loop_component");
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
        verify_module(&module).unwrap();
        Ok(())
    })
    .unwrap();
}

#[path = "production_source_reference_load_v29_tests.rs"]
mod load_tests;

#[path = "production_source_reference_store_v29_tests.rs"]
mod store_tests;

use super::*;

pub(in crate::production_semantic_kir_v1) fn run_cells(
    owner: ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        ROOT,
        &mut budget,
        |instances, budget| {
            let floor = budget.storage();
            let result = source_storage_v29::with_source_storage_root_v29(
                &mut layouts,
                instances,
                budget,
                |plan, _, budget| consume(plan, budget).map_err(Into::into),
            );
            assert!(budget.storage() >= floor);
            let extra = budget.storage() - floor;
            if layouts.permits_root_emission_refund(&owner, extra, budget) {
                budget.release_storage(extra).unwrap();
            }
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    let settled = layouts.permits_root_emission_refund(&owner, 0, &budget);
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = if settled {
        demands.discard(&mut budget)
    } else {
        drop(demands);
        Err(ArgumentResourceV1::Accounting.into())
    };
    result.and(cleanup).and(demand_cleanup)
}

#[path = "production_source_reference_storage_transfer_v29_tests.rs"]
mod storage_transfer_tests;

#[path = "production_source_reference_storage_transfer_resource_v29_tests.rs"]
mod storage_transfer_resource_tests;

#[path = "production_source_reference_inactive_v29_tests.rs"]
mod inactive_tests;

#[path = "production_source_reference_inactive_resource_v29_tests.rs"]
mod inactive_resource_tests;

#[test]
fn scalar_cell_strategy_preserves_source_effects_and_distinct_instances() {
    run_cells(owner(Case::Writeback), |plan, budget| {
        assert_eq!(plan.storage, SourceReferenceStorageV29::ScalarCells);
        assert_eq!(plan.loans.len(), 2);
        assert_eq!(plan.cells.rows.len(), 2);
        for (index, loan) in plan.loans.iter().enumerate() {
            let origin = &plan.origins[loan.origin];
            assert!(loan.effects.referent_writes > 0);
            assert!(matches!(
                loan.representation,
                SourceReferenceRepresentationV29::NeedsAddressable(_)
            ));
            let (_, cell) = plan.scalar_cell(index, budget)?.unwrap();
            assert_eq!(
                (cell.instance, cell.local, cell.generation, cell.ty),
                (origin.instance, origin.local, origin.generation, WORD)
            );
            assert!(!source_reference_existing_value_local_v29(
                plan,
                cell.instance,
                cell.local.index(),
                budget
            )?);
        }
        assert_ne!(plan.cells.rows[0].instance, plan.cells.rows[1].instance);
        let left = capture_instance(plan, 0);
        let right = capture_instance(plan, 1);
        let a = execution_function_signature_with_references_v29(
            plan.instances,
            left,
            Some(plan),
            budget,
        )?;
        let b = execution_function_signature_with_references_v29(
            plan.instances,
            right,
            Some(plan),
            budget,
        )?;
        source_reference_same_signature_v29(&a, &b, budget)?;
        assert_eq!(
            a.parameter_types,
            [Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Private,
                AccessMode::ReadWrite
            )]
        );
        assert_eq!(a.parameter_semantic_types, [CAPTURE]);
        Ok(())
    })
    .unwrap();
}

#[test]
fn stable_scalar_references_keep_proved_value_payloads() {
    for case in [Case::Shared, Case::UniqueRead] {
        run_cells(owner(case), |plan, budget| {
            assert!(plan.cells.rows.is_empty());
            for index in 0..plan.loans.len() {
                assert!(plan.scalar_cell(index, budget)?.is_none());
                plan.require_promoted(index, budget)?;
            }
            let signature = execution_function_signature_with_references_v29(
                plan.instances,
                capture_instance(plan, 0),
                Some(plan),
                budget,
            )?;
            assert_eq!(signature.parameter_types, [Type::Scalar(ScalarType::U64)]);
            Ok(())
        })
        .unwrap();
    }
}

pub(super) fn mixed_strategy_owner() -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::UniqueRead, |_, functions| {
        let original = &functions[1];
        let mut statements = original.blocks()[0].statements().to_vec();
        statements.insert(
            1,
            assign(
                projected(2, &[(SemanticProjectionKindV1::Dereference, WORD)]),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    WORD,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(29, 8).unwrap()),
                ))),
            ),
        );
        let locals = original
            .locals()
            .iter()
            .enumerate()
            .map(|(index, old)| local(40 + index as u8, old.ty(), old.role()))
            .collect();
        functions.push(function(
            40,
            false,
            WORD,
            locals,
            vec![
                block(
                    40,
                    statements,
                    call(2, SemanticOperandV1::Move(place(3, CAPTURE)), 1),
                ),
                block(41, vec![], SemanticTerminatorKindV1::Return),
            ],
        ));
        let root = &functions[0];
        functions[0] = function(
            10,
            true,
            WORD,
            root.locals().to_vec(),
            vec![
                block(
                    10,
                    vec![],
                    call(1, SemanticOperandV1::Copy(place(1, WORD)), 1),
                ),
                block(
                    11,
                    vec![],
                    call(3, SemanticOperandV1::Copy(place(1, WORD)), 2),
                ),
                block(12, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .with_kernel_entry(root.kernel_entry().unwrap().clone());
    })
}

#[test]
fn later_mutating_instance_keeps_its_own_representation_and_object_identity() {
    run_cells(mixed_strategy_owner(), |plan, budget| {
        assert_eq!(plan.loans.len(), 2);
        assert_eq!(plan.loans[0].effects.referent_writes, 0);
        assert!(plan.loans[1].effects.referent_writes > 0);
        assert_eq!(plan.cells.rows.len(), 1);
        assert!(plan.scalar_cell(0, budget)?.is_none());
        plan.require_promoted(0, budget)?;
        let (_, right) = plan.scalar_cell(1, budget)?.unwrap();
        let left = &plan.origins[plan.loans[0].origin];
        assert_ne!(left.instance, right.instance);
        assert_eq!(right.instance, plan.origins[plan.loans[1].origin].instance);
        let a = execution_function_signature_with_references_v29(plan.instances,
            capture_instance(plan, 0), Some(plan), budget)?;
        let b = execution_function_signature_with_references_v29(plan.instances,
            capture_instance(plan, 1), Some(plan), budget)?;
        assert_eq!(a.parameter_types, [Type::Scalar(ScalarType::U64)]);
        assert!(matches!(&b.parameter_types[..], [Type::Pointer(pointer)]
            if pointer.address_space == AddressSpace::Private && pointer.access == AccessMode::ReadWrite));
        assert_eq!(a.parameter_semantic_types, b.parameter_semantic_types);
        assert_eq!(a.result_semantic_type, b.result_semantic_type);
        Ok(())
    }).unwrap();
}

fn output_capture_abi() -> SemanticFunctionAbiV1 {
    let input = abi(30, false, CAPTURE);
    let output = input.adjusted_arguments()[0].value().clone();
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        input.adjusted_arguments().to_vec(),
        output,
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap()
}

pub(in crate::production_semantic_kir_v1) fn return_alias_owner(own_frame: bool) -> ProductionSemanticSsaOwnerV1 {
    owner_with(Case::Writeback, |_, functions| {
        let mut statements: Vec<_> = functions[2].blocks()[0]
            .statements()
            .iter()
            .filter(|statement| {
                !matches!(statement.kind(), SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local() == SemanticLocalIdV1::from_index(0))
            })
            .cloned()
            .collect();
        if own_frame {
            statements.push(assign(
                place(4, REFERENCE),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(3, WORD),
                },
            ));
            statements.push(assign(
                place(0, CAPTURE),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![SemanticOperandV1::Move(place(4, REFERENCE))],
                    )
                    .unwrap(),
                ),
            ));
        } else {
            statements.push(assign(
                place(0, CAPTURE),
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(1, CAPTURE))),
            ));
        }
        let mut locals = functions[2].locals().to_vec();
        locals[0] = local(30, CAPTURE, SemanticLocalRoleV1::Return);
        functions[2] = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([30; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
            source(),
            output_capture_abi(),
            locals,
            SemanticBlockIdV1::from_index(0),
            vec![block(30, statements, SemanticTerminatorKindV1::Return)],
        )
        .unwrap();
        let worker = &functions[1];
        let mut locals = worker.locals().to_vec();
        locals.push(local(25, WORD, SemanticLocalRoleV1::Temporary));
        functions[1] = function(
            20,
            false,
            WORD,
            locals,
            vec![
                block(
                    20,
                    worker.blocks()[0].statements().to_vec(),
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(
                            SemanticCallableIdV1::from_index(2),
                            vec![SemanticOperandV1::Move(place(3, CAPTURE))],
                            Some(SemanticCallDestinationV1::new(
                                place(3, CAPTURE),
                                SemanticControlFlowEdgeV1::new(
                                    SemanticEdgeRoleV1::CallReturn,
                                    SemanticBlockIdV1::from_index(1),
                                ),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(
                    21,
                    vec![
                        assign(
                            place(4, REFERENCE),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(projected(
                                3,
                                &[(SemanticProjectionKindV1::Field(0), REFERENCE)],
                            ))),
                        ),
                        assign(
                            place(5, WORD),
                            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(projected(
                                4,
                                &[(SemanticProjectionKindV1::Dereference, WORD)],
                            ))),
                        ),
                    ],
                    SemanticTerminatorKindV1::Return,
                ),
            ],
        );
    })
}

#[test]
fn ancestor_reference_returns_keep_the_original_cell_and_instance() {
    run_cells(return_alias_owner(false), |plan, budget| {
        assert_eq!(plan.cells.rows.len(), 2);
        for ordinal in 0..2 {
            let child = capture_instance(plan, ordinal);
            let node = plan.returns[child.index()].unwrap();
            assert_eq!(plan.nodes[node].ty, CAPTURE);
            let signature = execution_function_signature_with_references_v29(
                plan.instances,
                child,
                Some(plan),
                budget,
            )?;
            assert_eq!(
                signature.result_types,
                [Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Private,
                    AccessMode::ReadWrite
                )]
            );
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn own_frame_reference_return_is_not_an_ancestor_alias() {
    let error = run_cells(return_alias_owner(true), |_, _| {
        panic!("dangling return accepted")
    })
    .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSemanticKirErrorV1::Unsupported {
                detail: "source reference return escapes its referent call frame",
                ..
            }
        ),
        "{error:?}"
    );
}

#[test]
fn ordered_stores_are_not_silently_lowered_as_plain_cell_writes() {
    for (volatility, atomic) in [
        (SemanticVolatilityV1::Volatile, None),
        (
            SemanticVolatilityV1::NonVolatile,
            Some(SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Release,
                SemanticAtomicScopeV1::SingleThread,
            )),
        ),
    ] {
        let owner = owner_with(Case::Writeback, |_, functions| {
            let old = &functions[2];
            let mut count = 0;
            let statements = old.blocks()[0]
                .statements()
                .iter()
                .map(|original| {
                    if let SemanticStatementKindV1::Assign(assignment) = original.kind()
                        && !assignment.destination().projections().is_empty()
                        && let SemanticRvalueKindV1::Use(value) = assignment.value().kind()
                    {
                        count += 1;
                        statement(SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            assignment.destination().clone(),
                            value.clone(),
                            volatility,
                            atomic,
                        )))
                    } else {
                        original.clone()
                    }
                })
                .collect();
            assert_eq!(count, 1);
            functions[2] = function(
                30,
                false,
                CAPTURE,
                old.locals().to_vec(),
                vec![block(30, statements, SemanticTerminatorKindV1::Return)],
            );
        });
        let mut entered_emission = false;
        let error = run_cells(owner, |_, _| {
            entered_emission = true;
            Ok(())
        })
        .unwrap_err();
        assert!(!entered_emission);
        assert!(
            matches!(error, ProductionSemanticKirErrorV1::Unsupported { detail, .. }
            if detail == "source reference ordered store requires checked addressable effects")
        );
    }
}

use super::*;

fn dereference(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(local),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap()
}

fn helper_abi() -> SemanticFunctionAbiV1 {
    let unique = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        4,
        Some(4),
    )
    .unwrap();
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([160; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            REFERENCE,
            SemanticAbiPassModeV1::Direct(unique),
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow])
    .unwrap()
}

fn rebuild(
    prior: &SemanticFunctionDeclV1,
    locals: Vec<SemanticLocalDeclV1>,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut result = SemanticFunctionDeclV1::new(
        prior.identity(),
        prior.role(),
        prior.item_definition_identity(),
        prior.monomorphization_identity(),
        prior.generic_type_arguments_identity(),
        prior.const_generic_arguments_identity(),
        prior.source(),
        prior.abi().clone(),
        locals,
        prior.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = prior.kernel_entry() {
        result = result.with_kernel_entry(entry.clone());
    }
    result
}

fn root_blocks_with_helper_slot(root: &SemanticFunctionDeclV1) -> Vec<SemanticBasicBlockV1> {
    // Defined callable rows precede intrinsic rows, so inserting helper 1 shifts both intrinsics.
    root.blocks()
        .iter()
        .map(|block| {
            let terminator = match block.terminator().kind() {
                SemanticTerminatorKindV1::Call(call) => {
                    assert!(matches!(call.callee().index(), 1 | 2));
                    SemanticTerminatorV1::new(
                        block.terminator().source(),
                        SemanticTerminatorKindV1::Call(
                            SemanticDirectCallV1::new_callable_with_variadic_argument_abis(
                                SemanticCallableIdV1::from_index(call.callee().index() + 1),
                                call.arguments().to_vec(),
                                call.variadic_argument_abis().to_vec(),
                                call.destination().cloned(),
                                call.unwind(),
                            )
                            .unwrap(),
                        ),
                    )
                }
                _ => block.terminator().clone(),
            };
            SemanticBasicBlockV1::new(
                block.identity(),
                block.source(),
                block.statements().to_vec(),
                terminator,
            )
            .unwrap()
        })
        .collect()
}

fn helper_owner(read: bool, write: bool, repeated: bool) -> ProductionSemanticSsaOwnerV1 {
    let base = owner_with_shape(1, 0);
    let source = base.source_semantic();
    let root = &source.functions()[0];
    let mut locals = root.locals().to_vec();
    assert_eq!(locals.len(), 8);
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([148; 32]),
        REFERENCE,
        SemanticLocalRoleV1::Temporary,
        provenance(),
    ));
    let borrow = assign(
        8,
        REFERENCE,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: dereference(7),
        },
    );
    let mut blocks = root_blocks_with_helper_slot(root);
    assert_eq!(blocks.len(), 5);
    let mut first = blocks[3].statements().to_vec();
    first.push(borrow.clone());
    blocks[3] = block(
        3,
        first,
        call(
            1,
            vec![SemanticOperandV1::Move(place(8, REFERENCE))],
            0,
            UNIT,
            if repeated { 5 } else { 4 },
        ),
    );
    if repeated {
        blocks.push(block(
            5,
            vec![borrow],
            call(
                1,
                vec![SemanticOperandV1::Move(place(8, REFERENCE))],
                0,
                UNIT,
                4,
            ),
        ));
    }
    let mut body = vec![assign(
        3,
        REFERENCE,
        SemanticRvalueKindV1::Borrow {
            kind: SemanticBorrowKindV1::Mutable,
            place: dereference(1),
        },
    )];
    if read {
        body.push(assign(
            2,
            U32,
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                dereference(3),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ));
    }
    if write {
        body.push(SemanticStatementV1::new(
            provenance(),
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                dereference(3),
                SemanticOperandV1::Constant(SemanticConstantV1::new(
                    U32,
                    SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
                )),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ));
    }
    body.extend(root.blocks()[4].statements().iter().cloned());
    let helper = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([160; 32]),
        SemanticFunctionRoleV1::InternalHelper,
        SemanticItemDefinitionIdentityV1::from_sha256([160; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([160; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([160; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([160; 32]),
        provenance(),
        helper_abi(),
        [UNIT, REFERENCE, U32, REFERENCE]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([170 + index as u8; 32]),
                    ty,
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                    provenance(),
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![block(100, body, SemanticTerminatorKindV1::Return)],
    )
    .unwrap();
    let mut callables = source.callables().to_vec();
    assert_eq!(callables.len(), 3);
    callables.insert(
        1,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    );
    admitted_owner(
        source.types().to_vec(),
        vec![rebuild(root, locals, blocks), helper],
        callables,
    )
}

fn admitted_owner(
    types: Vec<SemanticTypeDeclV1>,
    functions: Vec<SemanticFunctionDeclV1>,
    callables: Vec<SemanticCallableDeclV1>,
) -> ProductionSemanticSsaOwnerV1 {
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
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
    let mut owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut ArgumentBudgetV1::new(&mut work, usize::MAX))
        .unwrap();
    owner
}

#[test]
fn issued_external_reference_helper_read_preserves_original_origin_through_typed_reborrows() {
    run_original_owner_counts(0, (0, 1, 1), helper_owner(true, false, false));
}

#[test]
fn issued_external_reference_helper_mutation_preserves_original_origin_through_typed_reborrows() {
    run_original_owner_counts(0, (1, 0, 1), helper_owner(false, true, false));
}

#[test]
fn repeated_issued_external_reference_helpers_preserve_independent_original_call_edges() {
    run_original_owner_counts(0, (2, 2, 4), helper_owner(true, true, true));
}

fn distinct_origin_join_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = owner_with_shape(2, 0);
    let source = base.source_semantic();
    let root = &source.functions()[0];
    let mut locals = root.locals().to_vec();
    let temporary = u32::try_from(locals.len()).unwrap();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([149; 32]),
        REFERENCE,
        SemanticLocalRoleV1::Temporary,
        provenance(),
    ));
    let mut blocks = root_blocks_with_helper_slot(root);
    assert_eq!(blocks.len(), 9);
    for index in [3, 7] {
        blocks[index] = block(
            index as u8,
            blocks[index].statements().to_vec(),
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 8)),
        );
    }
    let SemanticTerminatorKindV1::SwitchInt {
        discriminant,
        targets,
    } = root.blocks()[6].terminator().kind()
    else {
        panic!("second original Some edge");
    };
    blocks[6] = block(
        6,
        blocks[6].statements().to_vec(),
        SemanticTerminatorKindV1::SwitchInt {
            discriminant: discriminant.clone(),
            targets: SemanticSwitchTargetsV1::new(
                targets.values().to_vec(),
                edge(SemanticEdgeRoleV1::SwitchOtherwise, 9),
            )
            .unwrap(),
        },
    );
    let return_statements = blocks[8].statements().to_vec();
    blocks[8] = block(
        8,
        vec![assign(
            temporary,
            REFERENCE,
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Mutable,
                place: dereference(7),
            },
        )],
        call(
            1,
            vec![SemanticOperandV1::Move(place(temporary, REFERENCE))],
            0,
            UNIT,
            9,
        ),
    );
    blocks.push(block(
        9,
        return_statements,
        SemanticTerminatorKindV1::Return,
    ));
    let mut callables = source.callables().to_vec();
    assert_eq!(callables.len(), 3);
    callables.insert(
        1,
        SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
    );
    let helper = helper_owner(true, true, false)
        .source_semantic()
        .functions()[1]
        .clone();
    admitted_owner(
        source.types().to_vec(),
        vec![rebuild(root, locals, blocks), helper],
        callables,
    )
}

#[test]
fn distinct_issued_reference_join_remains_an_explicit_original_origin_refusal() {
    run_original_owner_case(
        0,
        (0, 0, 0),
        distinct_origin_join_owner(),
        Some(EarlyIssuedSourceRefusal::Unsupported(
            "source reference dereference has no checked origin",
        )),
    );
}

thread_local! {
    static EXTERNAL_RECEIPT_CHECKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn check_external_receipts(
    _: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let plan = references.plan;
    assert_eq!(plan.external_borrows.len(), 4);
    assert!(references.external_borrows.iter().all(std::cell::Cell::get));
    let mut rows = emission_vec_v1::<SourceExternalReferenceBorrowV29>(
        plan.external_borrows.len() + 1,
        budget,
    )?;
    for (index, row) in plan.external_borrows.iter().enumerate() {
        let exact = plan
            .instances
            .borrow_at(
                row.site.instance,
                row.site.block,
                row.site.statement.unwrap(),
                budget,
            )
            .unwrap();
        assert_eq!(
            source_external_reference_borrow_record_v29(
                plan,
                row.site,
                exact.source,
                row.ty,
                row.kind,
                budget,
            )?,
            Some(index)
        );
        for fault in 0..6 {
            rows.clear();
            budget.charge_work(plan.external_borrows.len())?;
            rows.extend_from_slice(&plan.external_borrows);
            match fault {
                0 => rows.clear(),
                1 => rows[index].source ^= 1,
                2 => rows[index].ty = U32,
                3 => rows[index].kind = SemanticBorrowKindV1::Shared,
                4 => {
                    let SourceExternalReferenceOriginV29::Issued { recipe, .. } =
                        &mut rows[index].origin
                    else {
                        panic!("genuine helper argument must retain its issuer");
                    };
                    recipe.block = SemanticBlockIdV1::from_index(u32::MAX);
                }
                5 => rows.push(*row),
                _ => unreachable!(),
            }
            let refused = source_external_reference_borrow_rows_v29(
                plan,
                &rows,
                row.site,
                exact.source,
                row.ty,
                row.kind,
                budget,
            );
            assert!(
                matches!(
                    refused,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail:
                            "external reference borrow differs from its original checked origin",
                    })
                ),
                "external receipt fault {fault}: {refused:?}"
            );
            assert_eq!(
                source_external_reference_borrow_record_v29(
                    plan,
                    row.site,
                    exact.source,
                    row.ty,
                    row.kind,
                    budget,
                )?,
                Some(index)
            );
        }
        let copied = exact.source.clone();
        let refused = source_external_reference_borrow_record_v29(
            plan, row.site, &copied, row.ty, row.kind, budget,
        );
        assert!(matches!(
            refused,
            Err(ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "external reference borrow differs from its original checked origin",
            })
        ));
        assert_eq!(
            source_external_reference_borrow_record_v29(
                plan,
                row.site,
                exact.source,
                row.ty,
                row.kind,
                budget,
            )?,
            Some(index)
        );
    }
    EXTERNAL_RECEIPT_CHECKS.set(EXTERNAL_RECEIPT_CHECKS.get() + 1);
    Ok(())
}

#[test]
fn external_helper_receipts_reject_missing_forged_duplicate_and_foreign_source_rows() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
        }
    }
    let _restore = Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(check_external_receipts)));
    EXTERNAL_RECEIPT_CHECKS.set(0);
    run_original_owner_counts(0, (2, 2, 4), helper_owner(true, true, true));
    assert_eq!(EXTERNAL_RECEIPT_CHECKS.get(), 1);
}

#[test]
fn typed_external_reborrow_alias_has_independent_exact_and_one_short_resources() {
    let owner = helper_owner(true, true, false);
    let source = owner.source_semantic();
    let function = &source.functions()[0];
    let SemanticStatementKindV1::Assign(assignment) = function.blocks()[3].statements()[1].kind()
    else {
        panic!("original external reborrow");
    };
    type Alias<'a> = Option<(&'a SemanticPlaceV1, ExecutionOperandV29, bool)>;
    let headers = std::mem::size_of::<Alias<'_>>()
        + 2 * std::mem::size_of::<Result<Alias<'_>, ProductionSemanticKirErrorV1>>();
    for short in 0..3 {
        let work_limit = 8 - usize::from(short == 1);
        let storage_limit = headers - usize::from(short == 2);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        let result = source_reference_pointer_alias_v29(
            source.types(),
            function,
            execution_site_v29(SemanticBlockIdV1::from_index(3), Some(1)),
            assignment.value().kind(),
            &mut budget,
        );
        match short {
            0 => {
                let (actual, role, reborrow) = result.unwrap().unwrap();
                let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
                    unreachable!();
                };
                assert!(std::ptr::eq(actual, place));
                assert_eq!((role, reborrow), (ExecutionOperandV29::RvaluePlace, true));
                assert_eq!((budget.work(), budget.storage()), (8, headers));
            }
            1 => assert!(matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error)
                )) if error.actual() == 8 && error.limit() == 7
            )),
            2 => assert!(matches!(result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error)
                )) if error.actual() == headers && error.limit() == headers - 1
            )),
            _ => unreachable!(),
        }
        let held = budget.storage();
        budget.release_storage(held).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

fn foreign_external_receipt_query(
    _: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let plan = references.plan;
    let row = plan.external_borrows.first().unwrap();
    let exact = plan
        .instances
        .borrow_at(
            row.site.instance,
            row.site.block,
            row.site.statement.unwrap(),
            budget,
        )
        .unwrap();
    let before = (budget.work(), budget.storage());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
    foreign.reserve_storage(before.1)?;
    let refused = source_external_reference_borrow_record_v29(
        plan,
        row.site,
        exact.source,
        row.ty,
        row.kind,
        &mut foreign,
    );
    assert!(matches!(
        refused,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting,
            )
        )
    ));
    assert_eq!((foreign.work(), foreign.storage()), (0, before.1));
    let repeated = source_external_reference_borrow_record_v29(
        plan,
        row.site,
        exact.source,
        row.ty,
        row.kind,
        budget,
    );
    assert!(matches!(
        repeated,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting,
            )
        )
    ));
    assert_eq!((budget.work(), budget.storage()), before);
    foreign.release_storage(before.1)?;
    EXTERNAL_RECEIPT_CHECKS.set(EXTERNAL_RECEIPT_CHECKS.get() + 1);
    Err(refused.unwrap_err())
}

#[test]
fn external_helper_receipt_query_rejects_fully_funded_foreign_ledger_and_poison_stays_sticky() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(foreign_external_receipt_query)));
    EXTERNAL_RECEIPT_CHECKS.set(0);
    run_original_owner_case(
        0,
        (0, 0, 0),
        helper_owner(true, true, false),
        Some(EarlyIssuedSourceRefusal::Accounting),
    );
    assert_eq!(EXTERNAL_RECEIPT_CHECKS.get(), 1);
}

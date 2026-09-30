use super::*;

thread_local! {
    static ARCHIVE_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static ARCHIVE_FAULTS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn assert_archive_refusal<T>(result: Result<T, ProductionSemanticKirErrorV1>) {
    assert!(matches!(
        result,
        Err(ProductionSemanticKirErrorV1::Unsupported {
            detail: "execution archive differs from its original source definition",
            ..
        })
    ));
}

fn inspect_assignment_archive(
    lifecycle: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut assignments = 0;
    let mut helpers = 0;
    for item in slots
        .instances
        .iter()
        .filter(|item| item.function.index() == 3)
    {
        helpers += 1;
        let source = instances.instance(item.instance).unwrap().declaration();
        let lowered = emitted[item.instance.index()].as_mut().unwrap();
        let archive = lowered.execution_observation.as_mut().unwrap();
        let mut expected = 0;
        for (block, source_block) in source.blocks().iter().enumerate() {
            for (statement, original) in source_block.statements().iter().enumerate() {
                let SemanticStatementKindV1::Assign(assignment) = original.kind() else {
                    continue;
                };
                let key = (block as u32, statement as u32);
                let site = ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(block as u32),
                    statement: statement as u32,
                };
                let ty = assignment.value().result_type();
                let binding = archive.lookup_rvalue_original_v30(
                    instances,
                    item.instance,
                    site,
                    ty,
                    budget,
                )?;
                assert!(std::ptr::eq(binding, &archive.rvalues[&key].binding));
                assignments += 1;
                expected += 1;
                if !ARCHIVE_FAULTS.get() {
                    continue;
                }
                let wrong_ty = SemanticTypeIdV1::from_index(u32::MAX);
                assert_archive_refusal(archive.lookup_rvalue_original_v30(
                    instances,
                    item.instance,
                    site,
                    wrong_ty,
                    budget,
                ));
                assert_archive_refusal(archive.lookup_rvalue_original_v30(
                    instances,
                    item.instance,
                    ExecutionSiteV29::Terminator {
                        block: SsaBlockIdV1::new(block as u32),
                    },
                    ty,
                    budget,
                ));
                assert_archive_refusal(archive.lookup_rvalue_original_v30(
                    instances,
                    item.instance,
                    ExecutionSiteV29::Statement {
                        block: SsaBlockIdV1::new(block as u32),
                        statement: u32::MAX,
                    },
                    ty,
                    budget,
                ));
                let other = slots
                    .instances
                    .iter()
                    .find(|other| {
                        other.function == item.function && other.instance != item.instance
                    })
                    .unwrap();
                assert_archive_refusal(archive.lookup_rvalue_original_v30(
                    instances,
                    other.instance,
                    site,
                    ty,
                    budget,
                ));
                let held = archive.rvalues.remove(&key).unwrap();
                assert_archive_refusal(archive.lookup_rvalue_original_v30(
                    instances,
                    item.instance,
                    site,
                    ty,
                    budget,
                ));
                archive.rvalues.insert(key, held);
                archive.rvalues.get_mut(&key).unwrap().ty = wrong_ty;
                assert_archive_refusal(archive.lookup_rvalue_original_v30(
                    instances,
                    item.instance,
                    site,
                    ty,
                    budget,
                ));
                archive.rvalues.get_mut(&key).unwrap().ty = ty;
                let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
                let mut foreign = ArgumentBudgetV1::new(&mut work, LIMIT);
                foreign.reserve_storage(budget.storage())?;
                let before = (foreign.work(), foreign.storage());
                assert!(matches!(
                    archive.lookup_rvalue_original_v30(
                        instances,
                        item.instance,
                        site,
                        ty,
                        &mut foreign
                    ),
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Accounting
                        )
                    )
                ));
                assert_eq!((foreign.work(), foreign.storage()), before);
                archive.lookup_rvalue_original_v30(instances, item.instance, site, ty, budget)?;
            }
        }
        assert_eq!(archive.rvalues.len(), expected);
    }
    assert_eq!(helpers, 2);
    assert!(assignments >= 2);
    ARCHIVE_VISITS.set(ARCHIVE_VISITS.get() + 1);
    observe(lifecycle, instances, emitted, slots, budget)
}

#[test]
fn original_rvalue_archive_covers_all_assignments_in_repeated_source_instances() {
    ARCHIVE_FAULTS.set(false);
    ARCHIVE_VISITS.set(0);
    let (result, _, _) = run(
        false,
        scalar_fixture(),
        inspect_assignment_archive,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(ARCHIVE_VISITS.get(), 1);
}

#[test]
fn original_rvalue_archive_rejects_missing_type_site_instance_and_foreign_ledger() {
    ARCHIVE_FAULTS.set(true);
    ARCHIVE_VISITS.set(0);
    let (result, _, _) = run(
        false,
        scalar_fixture(),
        inspect_assignment_archive,
        LIMIT,
        LIMIT,
    );
    ARCHIVE_FAULTS.set(false);
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(ARCHIVE_VISITS.get(), 1);
}

#[test]
fn original_rvalue_archive_complete_emission_has_exact_and_one_short_resources() {
    ARCHIVE_FAULTS.set(false);
    ARCHIVE_VISITS.set(0);
    let (result, work, peak) = run(
        false,
        scalar_fixture(),
        inspect_assignment_archive,
        LIMIT,
        LIMIT,
    );
    assert!(is_stopped(&result), "{result:?}");
    let result = run(
        false,
        scalar_fixture(),
        inspect_assignment_archive,
        work,
        peak,
    )
    .0;
    assert!(is_stopped(&result), "{result:?}");
    assert_eq!(ARCHIVE_VISITS.get(), 2);
    let result = run(
        false,
        scalar_fixture(),
        inspect_assignment_archive,
        work - 1,
        peak,
    )
    .0;
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
    let result = run(
        false,
        scalar_fixture(),
        inspect_assignment_archive,
        work,
        peak - 1,
    )
    .0;
    assert!(
        matches!(
            result,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(_)
                )
            )
        ),
        "{result:?}"
    );
}

#[test]
fn assignment_payload_requires_the_exact_archived_value_type_and_actual_store() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let ty = Type::Scalar(ScalarType::U32);
    let value = ValueId(17);
    let binding = SemanticValueBindingV1::Value {
        id: value,
        ty: ty.clone(),
    };
    let mut operation = Operation {
        results: vec![],
        kind: OperationKind::Store {
            pointer: ValueId(9),
            value,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    };
    check_issued_assignment_binding_v30(&binding, value, &ty, &operation, &mut budget).unwrap();
    for (id, actual_type) in [
        (ValueId(18), ty.clone()),
        (value, Type::Scalar(ScalarType::U64)),
    ] {
        assert!(matches!(
            check_issued_assignment_binding_v30(
                &binding,
                id,
                &actual_type,
                &operation,
                &mut budget
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source issued pointer differs from its original issuer or actual guard",
                ..
            })
        ));
    }
    let OperationKind::Store { value: stored, .. } = &mut operation.kind else {
        unreachable!()
    };
    *stored = ValueId(18);
    assert!(
        check_issued_assignment_binding_v30(&binding, value, &ty, &operation, &mut budget).is_err()
    );
    let OperationKind::Store { value: stored, .. } = &mut operation.kind else {
        unreachable!()
    };
    *stored = value;
    check_issued_assignment_binding_v30(&binding, value, &ty, &operation, &mut budget).unwrap();
}

#[test]
fn rvalue_archive_entry_storage_covers_split_path_and_boxed_whole_binding() {
    for count in [0usize, 1, 2, 31, 1024] {
        let levels = count.checked_ilog2().unwrap_or(0) as usize + 2;
        let expected = levels
            * 32
            * std::mem::size_of::<((u32, u32), Box<ExecutionRvalueBindingV30>, usize)>()
            + std::mem::size_of::<ExecutionRvalueBindingV30>();
        assert_eq!(
            execution_rvalue_entry_storage_v30(count).unwrap(),
            expected + execution_rvalue_headers_v30().unwrap()
        );
    }
}

#[test]
fn rvalue_archive_fixed_frames_include_whole_bindings_and_query_results() {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    let expected = h::<ExecutionArchiveCreditV29>()
        + h::<Option<ExecutionArchiveCreditV29>>()
        + h::<ExecutionSiteV29>()
        + h::<(u32, u32)>()
        + h::<SemanticTypeIdV1>()
        + h::<SemanticValueBindingV1>()
        + h::<Box<ExecutionRvalueBindingV30>>()
        + h::<&ExecutionRvalueBindingV30>()
        + h::<&SemanticValueBindingV1>()
        + h::<&ExecutionArchiveV29>()
        + h::<&ExecutionAvailabilityV29<'_>>()
        + h::<&ExecutionInstancesV29<'_>>()
        + h::<&SemanticFunctionDeclV1>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>()
        + h::<&mut ExecutionRvalueBindingsV30>()
        + h::<&mut Option<ExecutionArchiveCreditV29>>()
        + h::<&mut dyn SemanticEmissionBudgetV1>()
        + h::<SourceIssuedActualValueV29<'_>>()
        + h::<&Type>()
        + h::<&Operation>()
        + h::<ValueId>()
        + 4 * h::<usize>()
        + h::<()>();
    assert_eq!(execution_rvalue_headers_v30().unwrap(), expected);
}

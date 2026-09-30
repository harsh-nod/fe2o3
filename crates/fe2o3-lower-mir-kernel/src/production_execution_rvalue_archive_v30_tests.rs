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
            expected
                + if count == 0 {
                    instance_correspondence_tests::rvalue_archive_frame_storage_v30()
                } else {
                    0
                }
        );
    }
}

#[test]
fn rvalue_archive_fixed_frames_include_whole_bindings_and_query_results() {
    let expected = instance_correspondence_tests::rvalue_archive_frame_storage_v30();
    assert_eq!(execution_rvalue_headers_v30().unwrap(), expected);
}

#[test]
fn rvalue_archive_sequential_captures_share_one_exact_retained_frame() {
    use std::mem::size_of;

    for count in [1usize, 2, 8, 64] {
        let mut expected = instance_correspondence_tests::rvalue_archive_frame_storage_v30();
        for previous in 0..count {
            let levels = previous.checked_ilog2().unwrap_or(0) as usize + 2;
            expected +=
                levels * 32 * size_of::<((u32, u32), Box<ExecutionRvalueBindingV30>, usize)>()
                    + size_of::<ExecutionRvalueBindingV30>();
        }
        for short in [0usize, 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, 17 + expected - short);
            budget.reserve_storage(17).unwrap();
            let mut rows = ExecutionRvalueBindingsV30::new();
            let mut credit = None;
            for index in 0..count {
                let result = archive_owned_rvalue_v30(
                    &mut rows,
                    &mut credit,
                    ExecutionSiteV29::Statement {
                        block: SsaBlockIdV1::new(0),
                        statement: index as u32,
                    },
                    SemanticTypeIdV1::from_index(0),
                    &SemanticValueBindingV1::Value {
                        id: ValueId(index as u32),
                        ty: Type::Scalar(ScalarType::U32),
                    },
                    &mut budget,
                );
                if short == 1 && index + 1 == count {
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
                    assert_eq!(rows.len(), count - 1);
                } else {
                    result.unwrap();
                }
            }
            if short == 0 {
                assert_eq!(rows.len(), count);
                assert_eq!(credit.unwrap().bytes, expected);
                assert_eq!(budget.storage(), 17 + expected);
            }
        }
    }
}

thread_local! {
    static COMPILER_ENUM_ROLES_OBSERVED_V55: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn check_compiler_enum_role_mutations_v55(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    _slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let instance = instances.root();
    let lowered = emitted[instance.index()].as_mut().unwrap();
    let archive = lowered.execution_observation.as_ref().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
    let records = anchors.compiler_enum.clone();
    let rows = anchors.rows.clone();
    // The Some branch initializes precisely the two non-Unit merge leaves.
    assert_eq!(records.len(), 2);
    assert_eq!(
        records
            .iter()
            .map(|row| (row.variant, row.field, row.component))
            .collect::<Vec<_>>(),
        vec![(1, 0, 0), (1, 2, 0)]
    );
    let body = lowered.function.body.as_ref().unwrap();
    for original in &records {
        let anchor = rows[original.anchor];
        let original_operation = body
            .blocks
            .iter()
            .find(|block| block.id == anchor.block)
            .unwrap()
            .operations[anchor.position]
            .clone();
        for fault in 0..20 {
            anchors.compiler_enum = records.clone();
            anchors.rows = rows.clone();
            let mut record = *original;
            let mut operation = original_operation.clone();
            let ScopedCompilerEnumRoleV55::Store {
                site,
                value,
                source: None,
            } = record.role
            else {
                panic!("fixture constructor Store role");
            };
            match fault {
                0 => {}
                1 => anchors
                    .compiler_enum
                    .retain(|row| row.anchor != record.anchor),
                2 => {
                    let at = anchors
                        .compiler_enum
                        .iter()
                        .position(|row| row.anchor == record.anchor)
                        .unwrap();
                    anchors.compiler_enum.insert(at, record);
                }
                3 => record.anchor += 1,
                4 => record.local = SemanticLocalIdV1::from_index(u32::MAX),
                5 => record.variant = 0,
                6 => record.field = 1,
                7 => record.component = 1,
                8 => record.pointer = ValueId(u32::MAX),
                9 => {
                    record.role = ScopedCompilerEnumRoleV55::Store {
                        site,
                        value: record.pointer,
                        source: None,
                    }
                }
                10 => {
                    record.role = ScopedCompilerEnumRoleV55::Store {
                        site: ExecutionSiteV29::Terminator {
                            block: SemanticBlockIdV1::from_index(0),
                        },
                        value,
                        source: None,
                    }
                }
                11 => {
                    record.role = ScopedCompilerEnumRoleV55::Store {
                        site,
                        value,
                        source: Some(ScopedMemoryStoreSourceV29::Assignment { site, ty: U32 }),
                    }
                }
                12 => anchors.rows[record.anchor].source = None,
                13 => {
                    anchors.rows[record.anchor].kind = ScopedMemoryAnchorKindV29::Access {
                        pointer: ValueId(u32::MAX),
                        payload: None,
                    }
                }
                14 => {
                    if let OperationKind::Store { pointer, .. } = &mut operation.kind {
                        *pointer = ValueId(u32::MAX);
                    }
                }
                15 => {
                    if let OperationKind::Store { value, .. } = &mut operation.kind {
                        *value = record.pointer;
                    }
                }
                16 => {
                    if let OperationKind::Store { access, .. } = &mut operation.kind {
                        access.alignment *= 2;
                    }
                }
                17 => {
                    if let OperationKind::Store { access, .. } = &mut operation.kind {
                        access.volatile = true;
                    }
                }
                18 => {
                    if let OperationKind::Store { access, .. } = &mut operation.kind {
                        access.address_space = AddressSpace::Global;
                    }
                }
                19 => {
                    record.role = ScopedCompilerEnumRoleV55::Load {
                        block: SemanticBlockIdV1::from_index(0),
                        result: value,
                    }
                }
                _ => unreachable!(),
            }
            if (3..12).contains(&fault) || fault == 19 {
                let at = anchors
                    .compiler_enum
                    .iter()
                    .position(|row| row.anchor == original.anchor)
                    .unwrap();
                anchors.compiler_enum[at] = record;
            }
            let floor = budget.storage();
            let result = with_canonical_call_scratch_v1(budget, |budget| {
                let checked = check_scoped_compiler_enum_access_v55(
                    instances, instance, anchors, archive, &record, &operation, budget,
                )?;
                check_scoped_compiler_enum_scalar_value_v55(&checked, budget)
            });
            anchors.compiler_enum = records.clone();
            anchors.rows = rows.clone();
            assert_eq!(budget.storage(), floor, "role query scratch fault {fault}");
            if matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
            ) {
                return result;
            }
            if fault == 0 {
                result?;
            } else {
                assert!(result.is_err(), "compiler role fault {fault} admitted");
            }
        }
    }
    scoped_raw_admission_v29::test_compiler_enum_closed_mutations_v55(
        instance,
        archive,
        &records,
        &rows,
        &lowered.function,
        budget,
    )?;
    COMPILER_ENUM_ROLES_OBSERVED_V55.set(COMPILER_ENUM_ROLES_OBSERVED_V55.get() + 1);
    Ok(())
}

#[test]
fn compiler_enum_roles_rejoin_original_values_and_reject_missing_duplicate_and_substituted_accesses()
 {
    COMPILER_ENUM_ROLES_OBSERVED_V55.set(0);
    let _restore = EnumCensusObserverGuardV55(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(check_compiler_enum_role_mutations_v55)),
    );
    probe(
        transported_enum_owner_v50,
        MODULE_LIMIT,
        MODULE_LIMIT,
        inspect_enum_spills_v48,
    )
    .0
    .unwrap();
    assert!(COMPILER_ENUM_ROLES_OBSERVED_V55.get() > 0);
}

#[test]
fn compiler_enum_roles_keep_exact_and_one_short_complete_resource_boundaries() {
    let _restore = EnumCensusObserverGuardV55(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(check_compiler_enum_role_mutations_v55)),
    );
    let measured = probe(
        transported_enum_owner_v50,
        MODULE_LIMIT,
        MODULE_LIMIT,
        inspect_enum_spills_v48,
    );
    measured.0.unwrap();
    let exact = probe(
        transported_enum_owner_v50,
        measured.1,
        measured.2,
        inspect_enum_spills_v48,
    );
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    for work_short in [false, true] {
        let work = measured.1 - usize::from(work_short);
        let storage = measured.2 - usize::from(!work_short);
        let result = probe(
            transported_enum_owner_v50,
            work,
            storage,
            inspect_enum_spills_v48,
        );
        match entrance_resource(result.0.err().expect("one-short complete role account")) {
            ArgumentResourceV1::Work(error) if work_short => {
                assert_eq!(error.actual(), measured.1);
                assert_eq!(error.limit(), work);
            }
            ArgumentResourceV1::Storage(error) if !work_short => {
                assert_eq!(error.actual(), measured.2);
                assert_eq!(error.limit(), storage);
            }
            other => panic!("wrong enum role resource failure: {other:?}"),
        }
    }
}

#[test]
fn compiler_enum_scalar_roles_replay_through_the_actual_production_optimizer() {
    run_production_optimized_consumer_v18(
        transported_enum_owner_v50,
        |original, optimized, budget| {
            assert_eq!(original.enum_spill_count_v48(0, budget)?, 2);
            let roots = original.source.root_count(budget)?;
            assert!(roots > 0);
            let floor = budget.storage();
            assert_eq!(
                original.check_optimized_source_currentness_v18(optimized, budget)?,
                roots
            );
            assert_eq!(budget.storage(), floor);
            original.with_optimized_analysis_v18(optimized, budget, |analyses, budget| {
                analyses.with_memory_versions(budget, |input, output, budget| {
                    for root in 0..roots {
                        scoped_raw_admission_v29::with_checked_optimized_source_memory_v18(
                            original,
                            optimized,
                            root,
                            input,
                            output,
                            budget,
                            |_checked, _budget| Ok::<_, ProductionSourceOwnedViewErrorV18>(()),
                        )?;
                    }
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                })
            })
        },
    );
}

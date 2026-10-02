include!("production_scoped_enum_spill_census_v55_tests.rs");
include!("production_compiler_enum_roles_v55_tests.rs");

fn transported_enum_owner_v50() -> ProductionSemanticSsaOwnerV1 {
    let previous = promoted_enum_owner_v47();
    let semantic = previous.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let original = &functions[0];
    let statements = original.blocks()[0].statements();
    let prefix = statements.len() - 4;
    let jump = |target| {
        SemanticTerminatorKindV1::Goto(SemanticControlFlowEdgeV1::new(
            SemanticEdgeRoleV1::Goto,
            SemanticBlockIdV1::from_index(target),
        ))
    };
    // Only the first enum local merges definitions; its later copy has a linear edge.
    let select = SemanticTerminatorKindV1::SwitchInt {
        discriminant: SemanticOperandV1::Copy(place(3, U32)),
        targets: SemanticSwitchTargetsV1::new(
            vec![SemanticSwitchTargetV1::new(
                0,
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::SwitchValue,
                    SemanticBlockIdV1::from_index(1),
                ),
            )],
            SemanticControlFlowEdgeV1::new(
                SemanticEdgeRoleV1::SwitchOtherwise,
                SemanticBlockIdV1::from_index(2),
            ),
        )
        .unwrap(),
    };
    functions[0] = function(
        233,
        original.role(),
        original.abi().clone(),
        original.locals().to_vec(),
        vec![
            block(234, statements[..prefix].to_vec(), select),
            block(235, vec![statements[prefix].clone()], jump(3)),
            block(236, vec![statements[prefix + 1].clone()], jump(3)),
            block(237, vec![statements[prefix + 2].clone()], jump(4)),
            block(
                238,
                vec![statements[prefix + 3].clone()],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
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

fn enum_spill_header_oracle_v48() -> usize {
    fn h<T>() -> usize {
        size_of::<T>()
            + 2 * size_of::<Result<T, ProductionSemanticKirErrorV1>>()
            + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    h::<Vec<SourceEnumSpillRowV48>>()
        + h::<SourceEnumSpillRowV48>()
        + h::<ExecutionEnumSpillV48>()
        + h::<ProductionSourceEnumSpillOriginV48>()
        + h::<ProductionSourceEnumSpillV48<'_, '_>>()
        + h::<&[SourceEnumSpillRowV48]>()
        + h::<std::slice::Iter<'_, SourceEnumSpillRowV48>>()
        + h::<(&ExecutionEnumSpillV48, &ExecutionEnumSpillV48)>()
        + h::<(&Type, &Type)>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()
        + h::<&fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'_>>()
        + h::<[usize; 6]>()
}

fn inspect_enum_spills_v48(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let semantic = relation.source.source_semantic(budget)?;
    assert_eq!(semantic.functions()[0].blocks().len(), 5);
    let enumeration = SemanticTypeIdV1::from_index((semantic.types().len() - 1) as u32);
    let first = (semantic.functions()[0].locals().len() - 3) as u32;
    let count = relation.enum_spill_count_v48(0, budget)?;
    assert_eq!(
        count, 2,
        "one enum merge local, two non-Unit payload leaves"
    );
    let mut seen = [None; 2];
    let mut operations = [None; 2];
    for ordinal in 0..count {
        let receipt = relation.enum_spill_v48(0, ordinal, budget)?;
        let origin = receipt.origin(budget)?;
        assert_eq!(origin.instance, 0);
        assert_eq!(origin.source_type, enumeration);
        assert_eq!(origin.local, first);
        assert_eq!(origin.variant, 1);
        assert_eq!(origin.component, 0);
        let expected = match origin.field {
            0 => U32,
            2 => SemanticTypeIdV1::from_index((semantic.types().len() - 3) as u32),
            field => panic!("empty and Unit fields cannot acquire scalar storage: {field}"),
        };
        assert_eq!(origin.field_type, expected);
        let key = (origin.local, origin.variant, origin.field, origin.component);
        assert!(!seen.contains(&Some(key)));
        seen[ordinal] = Some(key);
        let (definition, operation) = receipt.allocation(budget)?;
        let actual = &relation.inventory.operations()[operation];
        assert!(!operations.contains(&Some(actual.coordinate)));
        operations[ordinal] = Some(actual.coordinate);
        assert_eq!(actual.results, definition..definition + 1);
        assert!(matches!(
            actual.operation.kind,
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4
            }
        ));
        assert_eq!(
            relation.inventory.definitions()[definition].ty,
            &Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )
        );
        assert_eq!(receipt.origin(budget)?, origin);
        assert_eq!(receipt.allocation(budget)?, (definition, operation));
    }
    relation.visit_allocation_frames_v32(0, budget, |frame, _| {
        assert!(!operations.contains(&Some(frame.allocation())));
        assert!(frame.local() != first && frame.local() != first + 1);
        Ok(())
    })?;
    assert!(seen.iter().all(Option::is_some));
    Ok(())
}

#[test]
fn compiler_enum_spills_retain_exact_nominal_fields_and_actual_allocas_not_source_frames() {
    let completed = std::cell::Cell::new(false);
    probe(
        transported_enum_owner_v50,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            inspect_enum_spills_v48(relation, budget)?;
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
}

#[test]
fn compiler_enum_spill_roster_detects_missing_duplicate_and_substituted_origins() {
    let completed = std::cell::Cell::new(false);
    probe(
        transported_enum_owner_v50,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            inspect_enum_spills_v48(relation, budget)?;
            let original = relation
                .source
                .root_row(0)?
                .rvalue_results
                .as_ref()
                .unwrap();
            assert_eq!(original.enum_spills.len(), 2);
            for fault in 0..15 {
                let mut altered = OwnedSourceRvaluesV30 {
                    source: original.source,
                    ledger: original.ledger,
                    rows: original.rows.clone(),
                    values: original.values.clone(),
                    carriers: original.carriers.clone(),
                    index_readers: original.index_readers.clone(),
                    enum_spills: original.enum_spills.clone(),
                    storage: original.storage,
                };
                match fault {
                    0 => {}
                    1 => {
                        altered.enum_spills.pop();
                    }
                    2 => altered.enum_spills.push(altered.enum_spills[0].clone()),
                    3 => altered.enum_spills[0].instance += 1,
                    4 => altered.enum_spills[0].origin.local += 1,
                    5 => altered.enum_spills[0].origin.source_type = U32,
                    6 => altered.enum_spills[0].origin.variant = 0,
                    7 => altered.enum_spills[0].origin.field = 1,
                    8 => altered.enum_spills[0].origin.field_type = UNIT,
                    9 => altered.enum_spills[0].origin.component += 1,
                    10 => {
                        altered.enum_spills[0].origin.pointer =
                            altered.enum_spills[1].origin.pointer
                    }
                    11 => altered.enum_spills[0].origin.emitted_operation += 1,
                    12 => altered.enum_spills[0].origin.element = Type::Scalar(ScalarType::I32),
                    13 => altered.enum_spills[0].origin.alignment = 1,
                    14 => altered.enum_spills[0].origin.emitted_block.0 += 1,
                    _ => unreachable!(),
                }
                assert_eq!(
                    original.matches_replay_v30(&altered, budget).map_err(|_| {
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "spill test replay resource failure",
                        )
                    })?,
                    fault == 0,
                    "spill replay fault {fault}"
                );
            }
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
}

#[test]
fn compiler_enum_spill_queries_preserve_sticky_ordinal_floor_and_foreign_ledger_refusals() {
    for fault in 0..3 {
        let reached = std::cell::Cell::new(false);
        let attacked = std::cell::Cell::new(false);
        let result = probe(
            transported_enum_owner_v50,
            MODULE_LIMIT,
            MODULE_LIMIT,
            |relation, budget| {
                inspect_enum_spills_v48(relation, budget)?;
                let receipt = relation.enum_spill_v48(0, 0, budget)?;
                receipt.allocation(budget)?;
                reached.set(true);
                let error = match fault {
                    0 => relation
                        .enum_spill_v48(0, usize::MAX, budget)
                        .err()
                        .expect("absent ordinal refuses"),
                    1 => {
                        let floor = budget.storage();
                        budget.release_storage(floor)?;
                        let error = receipt
                            .origin(budget)
                            .err()
                            .expect("undercut owner refuses");
                        budget.reserve_storage(floor)?;
                        error
                    }
                    2 => {
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
                        let mut foreign = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                        foreign.reserve_storage(budget.storage())?;
                        let before = (foreign.work(), foreign.storage(), foreign.peak_storage());
                        let error = receipt
                            .allocation(&mut foreign)
                            .err()
                            .expect("foreign ledger refuses");
                        assert_eq!(
                            (foreign.work(), foreign.storage(), foreign.peak_storage()),
                            before
                        );
                        error
                    }
                    _ => unreachable!(),
                };
                if fault == 0 {
                    assert!(matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "compiler enum spill ordinal is absent"
                        )
                    ));
                } else {
                    assert!(matches!(
                        error,
                        ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                    ));
                }
                let after_first = (budget.work(), budget.storage(), budget.peak_storage());
                assert!(relation.enum_spill_count_v48(0, budget).is_err());
                assert_eq!(
                    (budget.work(), budget.storage(), budget.peak_storage()),
                    after_first
                );
                attacked.set(true);
                Err(error)
            },
        )
        .0;
        assert!(
            reached.get() && attacked.get(),
            "fault {fault} must reach its attack"
        );
        if fault == 0 {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "compiler enum spill ordinal is absent"
                ))
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Resource(
                    ArgumentResourceV1::Accounting
                ))
            ));
        }
    }
}

#[test]
fn compiler_enum_spill_capture_and_query_have_exact_complete_resource_boundaries() {
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
    for is_work in [false, true] {
        let work = measured.1 - usize::from(is_work);
        let storage = measured.2 - usize::from(!is_work);
        let error = entrance_resource(
            probe(
                transported_enum_owner_v50,
                work,
                storage,
                inspect_enum_spills_v48,
            )
            .0
            .err()
            .expect("one-short complete spill account"),
        );
        match error {
            ArgumentResourceV1::Work(error) if is_work => {
                assert_eq!(error.limit(), work);
                assert_eq!(error.actual(), measured.1);
            }
            ArgumentResourceV1::Storage(error) if !is_work => {
                assert_eq!(error.limit(), storage);
                assert_eq!(error.actual(), measured.2);
            }
            other => panic!("wrong boundary: {other:?}"),
        }
    }
}

#[test]
fn single_block_known_enum_carriers_do_not_require_compiler_spills() {
    let completed = std::cell::Cell::new(false);
    probe(
        promoted_enum_owner_v47,
        MODULE_LIMIT,
        MODULE_LIMIT,
        |relation, budget| {
            check_promoted_enum_v47(relation, budget)?;
            assert_eq!(relation.enum_spill_count_v48(0, budget)?, 0);
            completed.set(true);
            Ok(())
        },
    )
    .0
    .unwrap();
    assert!(completed.get());
}

#[test]
fn compiler_enum_spill_headers_and_type_comparison_have_independent_oracles() {
    type Fields = (
        u32,
        SemanticTypeIdV1,
        u32,
        u32,
        SemanticTypeIdV1,
        usize,
        BlockId,
        usize,
        ValueId,
        Type,
        u32,
    );
    type OwnedFields = (
        ExecutionCallSourceV29,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        Vec<SourceRvalueRowV30>,
        Vec<SourceSsaRowV30>,
        Vec<SourceSsaComponentV37>,
        Vec<SourceIndexReaderRowV35>,
        Vec<SourceEnumSpillRowV48>,
        usize,
    );
    assert_eq!(size_of::<Fields>(), size_of::<ExecutionEnumSpillV48>());
    assert_eq!(size_of::<OwnedFields>(), size_of::<OwnedSourceRvaluesV30>());
    assert_eq!(
        source_enum_spill_headers_v48().unwrap(),
        enum_spill_header_oracle_v48()
    );
    for depth in [0usize, 1, 8, 64] {
        let mut ty = Type::Scalar(ScalarType::U32);
        for _ in 0..depth {
            ty = Type::pointer(ty, AddressSpace::Private, AccessMode::ReadOnly);
        }
        for limit in [depth + 1, depth] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            let result = enum_spill_types_equal_v48(&ty, &ty, &mut budget);
            if limit == depth + 1 {
                assert!(result.unwrap());
                assert_eq!(budget.work(), limit);
                assert_eq!(budget.peak_storage(), 0);
            } else {
                assert!(
                    matches!(result, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error))) if error.limit() == limit && error.actual() == depth + 1)
                );
            }
        }
    }
}

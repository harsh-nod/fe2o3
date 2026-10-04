// Inert physical equation fixtures only. Production admission separately
// derives these claims from the original constructor and complete loan tuple.
fn compiler_reference_currentness_fixture_v55(
    fault: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize) {
    let schema = fe2o3_kernel_ir::StorageLayoutIdV1(0);
    let instance = ProductionCallInstanceIdV1(0);
    let source_ty = SemanticTypeIdV1::from_index(0);
    let local = SemanticLocalIdV1::from_index(0);
    let block_id = BlockId(77);
    let referent = ValueId(10);
    let cell = ValueId(11);
    let alias = ValueId(12);
    let element = Type::StorageObject(schema);
    let pointer = Type::pointer(element.clone(), AddressSpace::Private, AccessMode::ReadOnly);
    let alloca = |id, element: Type, alignment| {
        Operation::effect_free(
            ValueDef::new(
                id,
                Type::pointer(
                    element.clone(),
                    AddressSpace::Private,
                    AccessMode::ReadWrite,
                ),
            ),
            OperationKind::Alloca {
                element,
                count: None,
                address_space: AddressSpace::Private,
                alignment,
            },
        )
    };
    let mut block = BasicBlock::new(block_id);
    block.operations = vec![
        alloca(referent, element, 4),
        alloca(cell, pointer.clone(), 8),
        Operation::effect_free(
            ValueDef::new(alias, pointer.clone()),
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value: referent,
                to: pointer.clone(),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: cell,
                value: alias,
                access: MemoryAccess::new(AddressSpace::Private, 8),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    if fault == 6 {
        block.operations.push(block.operations[3].clone());
    }
    let function = Function::internal_helper(
        "compiler-reference-currentness",
        Signature::new(vec![], vec![]),
        vec![],
        vec![block],
    );
    let slot = ScopedSourceSlotV29 {
        instance,
        origin: ScopedSlotOriginV29 {
            identity: ScopedAllocationIdentityV29::OriginalObject {
                local: 0,
                generation: 0,
            },
            source: ScopedAllocationSourceV29::OriginalObject { cell: 0, schema },
            semantic_type: source_ty,
            pointer: referent,
        },
        representation: ScopedSlotRepresentationV29::Object {
            schema,
            bytes: 4,
            alignment: 4,
        },
        allocation: PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: block_id,
            operation: 0,
        },
    };
    let site = ExecutionSiteV29::Statement {
        block: SsaBlockIdV1::new(0),
        statement: 0,
    };
    let custody = SourceCompilerEnumReferenceV55 {
        loan: SourceSsaLoanV36 {
            loan: 0,
            kind: SemanticBorrowKindV1::Shared,
            site: SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(0),
                statement: Some(0),
            },
            origin: 0,
            origin_instance: instance,
            origin_local: local,
            origin_generation: 0,
            origin_function: SemanticFunctionIdV1::from_index(0),
            origin_type: source_ty,
            carrier: ProductionSourceReferenceCarrierV38::MemoryPointer,
        },
        slot: usize::from(fault == 4),
        backing: SourceCompilerEnumReferenceBackingV55::Object(schema),
    };
    let pending = PendingCompilerEnumMemoryV55 {
        allocations: vec![SourceEnumSpillRowV48 {
            instance: 0,
            origin: ExecutionEnumSpillV48 {
                local: 1,
                source_type: source_ty,
                variant: 1,
                field: 0,
                field_type: source_ty,
                component: 0,
                emitted_block: block_id,
                emitted_operation: 1,
                pointer: cell,
                element: pointer,
                alignment: 8,
            },
        }],
        accesses: vec![PendingCompilerEnumAccessV55 {
            instance,
            block: block_id,
            operation: 3,
            reference: (fault != 5).then_some(custody),
            record: ScopedCompilerEnumAccessV55 {
                anchor: 0,
                local: SemanticLocalIdV1::from_index(1),
                variant: 1,
                field: 0,
                component: 0,
                pointer: cell,
                role: ScopedCompilerEnumRoleV55::Store {
                    site,
                    value: alias,
                    source: Some(ScopedMemoryStoreSourceV29::Operand {
                        site,
                        role: ExecutionOperandV29::RvalueOperand(0),
                        ty: source_ty,
                        source: ScopedMemoryOperandSourceV29::Place(
                            ScopedMemoryOccurrenceV29::Promoted {
                                event: 0,
                                definition: SsaValueV1::Definition(
                                    fe2o3_mir_model::SsaDefinitionIdV1::new(0),
                                ),
                            },
                        ),
                    }),
                },
            },
        }],
    };
    let layouts = [fe2o3_kernel_ir::StorageLayoutV1 {
        size: 4,
        alignment: 4,
        kind: fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(ScalarType::U32),
    }];
    let mut lifetimes = Vec::new();
    if (1..=3).contains(&fault) || fault == 7 {
        lifetimes.push(SourceAddressLifetimeV29 {
            block: block_id,
            gap: if fault == 7 { 4 } else { 3 },
            sequence: 0,
            source_order: [0; 5],
            slot: 0,
            live: fault == 2,
        });
        if fault == 3 {
            lifetimes.push(SourceAddressLifetimeV29 {
                live: true,
                sequence: 1,
                source_order: [0, 0, 0, 0, 1],
                ..lifetimes[0]
            });
        }
    }
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(37).unwrap();
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let checked = check_compiler_enum_closed_memory_v55(&function, &pending, budget)?;
        let graph = SourceAddressMemoryV29::prepare_with_compiler(
            &function,
            &[slot],
            None,
            &[],
            &layouts,
            Some(&checked),
            budget,
        )?
        .solve(&[slot], &[], &[], budget)?;
        assert_eq!(graph.exact(alias, budget)?, Some(0));
        check_source_address_currentness_v29(
            &function,
            &graph,
            &[slot],
            &[],
            &[],
            &[true],
            &lifetimes,
            &[],
            budget,
        )
    });
    assert_eq!(budget.storage(), 37);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn compiler_object_reference_store_checks_currentness_and_closed_custody() {
    for fault in 0..8 {
        let result = compiler_reference_currentness_fixture_v55(fault, usize::MAX, usize::MAX).0;
        if fault == 0 || fault == 7 {
            result.unwrap();
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::Unsupported { .. })
                ),
                "fault {fault}: {result:?}"
            );
        }
    }
}

#[test]
fn compiler_object_reference_store_has_exact_and_one_short_resource_limits() {
    let measured = compiler_reference_currentness_fixture_v55(0, usize::MAX, usize::MAX);
    measured.0.unwrap();
    let exact = compiler_reference_currentness_fixture_v55(0, measured.1, measured.2);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (measured.1, measured.2));
    for short_work in [false, true] {
        let work = measured.1 - usize::from(short_work);
        let storage = measured.2 - usize::from(!short_work);
        let result = compiler_reference_currentness_fixture_v55(0, work, storage).0;
        match result {
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Work(error),
            )) if short_work => {
                assert_eq!((error.actual(), error.limit()), (measured.1, work));
            }
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error),
            )) if !short_work => {
                assert_eq!((error.actual(), error.limit()), (measured.2, storage));
            }
            other => panic!("wrong short-resource result: {other:?}"),
        }
    }
}

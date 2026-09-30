fn descriptor_helper_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = descriptor_source_owner(DescriptorCase::READ);
    let source = base.source_semantic();
    let root = &source.functions()[0];
    let mut types = source.types().to_vec();
    let reference = SemanticTypeIdV1::from_index(u32::try_from(types.len()).unwrap());
    types.push(
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([220; 32]),
            SemanticLayoutIdentityV1::from_sha256([220; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    U32,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        4,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let shared = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        4,
        Some(4),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([221; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            reference,
            SemanticAbiPassModeV1::Direct(shared),
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::SharedBorrow])
    .unwrap();
    let SemanticStatementKindV1::Assign(old) = root.blocks()[1].statements()[0].kind() else {
        panic!("original checked descriptor read");
    };
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(selected)) = old.value().kind() else {
        panic!("original checked descriptor source");
    };
    let mut locals = root.locals().to_vec();
    assert_eq!(locals.len(), 7);
    locals.push(local(222, reference, SemanticLocalRoleV1::Temporary));
    let mut blocks = root.blocks().to_vec();
    blocks[1] = block(
        212,
        vec![assign(
            place(7, reference),
            SemanticRvalueKindV1::Borrow {
                kind: SemanticBorrowKindV1::Shared,
                place: selected.clone(),
            },
        )],
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(1),
                vec![SemanticOperandV1::Copy(place(7, reference))],
                Some(SemanticCallDestinationV1::new(
                    place(0, UNIT),
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(2),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        ),
    );
    blocks.push(block(223, vec![], SemanticTerminatorKindV1::Return));
    let root = function(
        200,
        SemanticFunctionRoleV1::KernelRoot,
        root.abi().clone(),
        locals,
        blocks,
    )
    .with_kernel_entry(root.kernel_entry().unwrap().clone());
    let dereference = |local| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(local),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    let helper = function(
        224,
        SemanticFunctionRoleV1::InternalHelper,
        abi,
        vec![
            local(225, UNIT, SemanticLocalRoleV1::Return),
            local(226, reference, SemanticLocalRoleV1::Argument(0)),
            local(227, reference, SemanticLocalRoleV1::Temporary),
            local(228, U32, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(
            229,
            vec![
                assign(
                    place(2, reference),
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: dereference(1),
                    },
                ),
                assign(
                    place(3, U32),
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(dereference(2))),
                ),
            ],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root, helper],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
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

thread_local! {
    static EXTERNAL_DESCRIPTOR_FAULT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static EXTERNAL_DESCRIPTOR_CHECKS: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
}

fn inspect_descriptor_helper(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let references = references.unwrap();
    let observation = SOURCE_EMISSION_OBSERVATION_V29
        .get()
        .expect("source-owned emission phase");
    assert_eq!(
        observation.owner,
        std::ptr::from_ref(instances.owner()) as usize
    );
    assert_eq!(observation.slot, std::ptr::from_ref(budget) as usize);
    assert!(observation.ledger == budget.work_ledger_identity_v1());
    let phase = match observation.phase {
        SourceEmissionPhaseV29::Admission => 0,
        SourceEmissionPhaseV29::ConstructionReplay => 1,
        SourceEmissionPhaseV29::ConsumerReplay => 2,
    };
    assert_eq!(
        EXTERNAL_DESCRIPTOR_CHECKS.get(),
        match phase {
            0 => [0, 0, 0],
            1 => [1, 0, 0],
            2 => [1, 1, 0],
            _ => unreachable!(),
        }
    );
    assert_eq!(references.plan.external_borrows.len(), 2);
    assert!(references.external_borrows.iter().all(std::cell::Cell::get));
    assert!(references.plan.external_borrows.iter().all(|row| matches!(
        row.origin,
        SourceExternalReferenceOriginV29::Descriptor { descriptor: 0, .. }
    )));
    assert!(references.plan.descriptor_root.is_none());
    assert_eq!(references.plan.descriptors.len(), 1);
    let row = references.plan.descriptors[0];
    let source = row.check(instances, budget)?;
    let representation = references.plan.descriptor_value_space(
        row.instance,
        row.holder_occurrence,
        source,
        row.projection - 1,
        row.pointer_type,
        budget,
    )?;
    assert_eq!(representation, AddressSpace::Generic);
    let claim = references.descriptors[0].get().unwrap();
    let SourceReferenceSelectorProducerV29::Address {
        base,
        offset,
        pointer,
        block,
        operation,
    } = claim.producer
    else {
        panic!("original descriptor address producer");
    };
    let original = emitted[row.instance.index()].as_ref().unwrap();
    let producer = &original
        .function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)
        .unwrap()
        .operations[operation];
    assert!(
        matches!(producer.kind, OperationKind::GetElementPointer { base: actual_base, offset: actual_offset }
        if actual_base == base && actual_offset == offset)
    );
    assert_eq!(producer.results.len(), 1);
    assert_eq!(producer.results[0].id, pointer);
    assert_eq!(
        source_issued_pointer_shape_v26(&producer.results[0].ty),
        Some((ScalarType::U32, representation, AccessMode::ReadOnly))
    );
    assert_eq!(
        source_issued_global_pointer_origin_v26(&original.function, pointer, budget)?,
        None,
        "a source-authenticated Generic descriptor is not a Global issuer"
    );
    let mut loads = 0;
    let mut geps = 0;
    for operation in emitted.iter().flatten().flat_map(|function| {
        function
            .function
            .body
            .iter()
            .flat_map(|body| body.blocks.iter())
            .flat_map(|block| &block.operations)
    }) {
        match operation.kind {
            OperationKind::Load { access, .. } => {
                assert_eq!(access.address_space, AddressSpace::Generic);
                loads += 1;
            }
            OperationKind::GetElementPointer { .. } => geps += 1,
            OperationKind::Store { .. } => panic!("shared descriptor helper must not write"),
            _ => {}
        }
    }
    assert_eq!((loads, geps), (1, 1));
    if EXTERNAL_DESCRIPTOR_FAULT.get() {
        let mut claim = references.descriptors[0].get().unwrap();
        let SourceReferenceSelectorProducerV29::Address { base, offset, .. } = &mut claim.producer
        else {
            panic!("original descriptor address producer");
        };
        assert_ne!(*offset, *base, "the original index is not the data pointer");
        *offset = *base;
        references.descriptors[0].set(Some(claim));
    }
    let mut checked = EXTERNAL_DESCRIPTOR_CHECKS.get();
    checked[phase] += 1;
    EXTERNAL_DESCRIPTOR_CHECKS.set(checked);
    Ok(())
}

#[test]
fn original_readonly_descriptor_reference_helper_checks_source_archive_and_actual_pointer_transport()
 {
    struct Restore(Option<ScopedSlotCustodyObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            EXTERNAL_DESCRIPTOR_FAULT.set(false);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_descriptor_helper)));
    for fault in [false, true, false] {
        EXTERNAL_DESCRIPTOR_FAULT.set(fault);
        EXTERNAL_DESCRIPTOR_CHECKS.set([0; 3]);
        let result = run_descriptor_owner_module(
            descriptor_helper_owner(),
            DescriptorCase::READ,
            DescriptorFault::None,
            MODULE_LIMIT,
            MODULE_LIMIT,
        )
        .0;
        assert_eq!(
            EXTERNAL_DESCRIPTOR_CHECKS.get(),
            if fault { [1, 0, 0] } else { [1, 1, 1] }
        );
        if fault {
            // Index normalization rejects the substituted pointer before the
            // later descriptor GEP equations can run.
            assert!(
                matches!(
                    result,
                    Err(ScopedModuleErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            function: 0,
                            block: None,
                            statement: None,
                            detail: "execution availability differs from its source SSA instance",
                        }
                    ))
                ),
                "changed external descriptor producer: {result:?}"
            );
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn original_readonly_descriptor_reference_helper_rejects_actual_load_alignment_type_and_volatility()
{
    for fault in [
        DescriptorFault::ExternalHelperAlignment,
        DescriptorFault::ExternalHelperResultType,
        DescriptorFault::ExternalHelperVolatility,
    ] {
        run_descriptor_owner_module(
            descriptor_helper_owner(),
            DescriptorCase::READ,
            DescriptorFault::None,
            MODULE_LIMIT,
            MODULE_LIMIT,
        )
        .0
        .unwrap();
        let result = run_descriptor_owner_module(
            descriptor_helper_owner(),
            DescriptorCase::READ,
            fault,
            MODULE_LIMIT,
            MODULE_LIMIT,
        )
        .0;
        assert_eq!(DESCRIPTOR_TAMPERED.get(), 1);
        let (expected_function, expected_detail) = match fault {
            DescriptorFault::ExternalHelperAlignment => (
                0,
                "source runtime slice descriptor/index/extent correspondence differs",
            ),
            DescriptorFault::ExternalHelperResultType
            | DescriptorFault::ExternalHelperVolatility => {
                // The common scoped checker rejects these in the original
                // helper before the root-level descriptor relation runs.
                (1, "scoped memory anchors differ from their source instance")
            }
            _ => unreachable!(),
        };
        assert!(
            matches!(result,
            Err(ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                function, block: None, statement: None, detail,
            })) if function == expected_function && detail == expected_detail),
            "changed helper load {fault:?}: {result:?}"
        );
        run_descriptor_owner_module(
            descriptor_helper_owner(),
            DescriptorCase::READ,
            DescriptorFault::None,
            MODULE_LIMIT,
            MODULE_LIMIT,
        )
        .0
        .unwrap();
    }
}

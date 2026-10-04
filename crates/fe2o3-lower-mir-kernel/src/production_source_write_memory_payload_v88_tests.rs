use super::*;

const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const SHARED: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);

// An ordinary bounds-checked shared-slice read is evaluated as CallArgument(2).
// The write uses its own Thread witness and explicit destination extent check.
fn memory_write_owner_v88() -> ProductionSemanticSsaOwnerV1 {
    let template = write_owner(true, true, 0);
    let source = template.source_semantic();
    let original = &source.functions()[0];
    let mut types = source.types().to_vec();
    assert_eq!(types.len(), SLICE.index() as usize);
    types.push(declaration(
        90,
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: U32 },
    ));
    types.push(
        declaration(
            91,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::scalar_pair(
                    SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                    ),
                    integer(64),
                ),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    SLICE,
                    SemanticPointerKindV1::Reference,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::SliceLength,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    let plain = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let readonly = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
            true,
            true,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        original.abi().identity(),
        original.abi().layout_identity(),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        2,
        vec![
            original.abi().arguments()[0].clone(),
            SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
                SHARED,
                SemanticAbiPassModeV1::Pair {
                    first: readonly,
                    second: plain,
                },
            )),
        ],
        original.abi().return_value().clone(),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
        SemanticSourceArgumentOwnershipV1::SharedBorrow,
    ])
    .unwrap();
    let locals = [
        UNIT, CARRIER, SHARED, BORROW, WITNESS, BOOL, U32, INDEX, INDEX, BOOL,
    ]
    .into_iter()
    .enumerate()
    .map(|(local, ty)| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([40 + local as u8; 32]),
            ty,
            match local {
                0 => SemanticLocalRoleV1::Return,
                1 => SemanticLocalRoleV1::Argument(0),
                2 => SemanticLocalRoleV1::Argument(1),
                _ => SemanticLocalRoleV1::Temporary,
            },
            provenance(),
        )
    })
    .collect();
    let copy = |local, ty| SemanticOperandV1::Copy(place(local, ty));
    let selected = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(7)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let blocks = vec![
        block(
            0,
            vec![
                assign(
                    3,
                    BORROW,
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Mutable,
                        place: place(1, CARRIER),
                    },
                ),
                assign(
                    7,
                    INDEX,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                        SemanticConstantV1::new(
                            INDEX,
                            SemanticConstantValueV1::Scalar(
                                SemanticScalarValueV1::new(0, 8).unwrap(),
                            ),
                        ),
                    )),
                ),
            ],
            call(1, vec![], 4, WITNESS, 1),
        ),
        block(
            1,
            vec![
                assign(
                    8,
                    INDEX,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: copy(2, SHARED),
                    },
                ),
                assign(
                    9,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: copy(7, INDEX),
                        right: copy(8, INDEX),
                    },
                ),
            ],
            SemanticTerminatorKindV1::Assert {
                condition: copy(9, BOOL),
                expected: true,
                message: SemanticAssertMessageV1::BoundsCheck {
                    length: copy(8, INDEX),
                    index: copy(7, INDEX),
                },
                target: edge(SemanticEdgeRoleV1::AssertSuccess, 2),
                unwind: SemanticUnwindActionV1::Unreachable,
            },
        ),
        block(
            2,
            vec![],
            call(
                2,
                vec![
                    SemanticOperandV1::Move(place(3, BORROW)),
                    SemanticOperandV1::Move(place(4, WITNESS)),
                    SemanticOperandV1::Copy(selected),
                ],
                5,
                BOOL,
                3,
            ),
        ),
        block(
            3,
            vec![],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: copy(5, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 4),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                )
                .unwrap(),
            },
        ),
        block(
            4,
            vec![],
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 5)),
        ),
        block(
            5,
            original.blocks().last().unwrap().statements().to_vec(),
            SemanticTerminatorKindV1::Return,
        ),
    ];
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        abi,
        locals,
        original.entry(),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![function],
        source.callables().to_vec(),
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
    let mut work = CanonicalKernelIrWorkBudgetV1::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    owner
}

thread_local! {
    static MEMORY_WRITES_SEEN_V88: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

struct MemoryWriteObserverV88(Option<SourceIssuedMemoryPayloadObserverV30>, u8);

impl MemoryWriteObserverV88 {
    fn install(fault: u8) -> Self {
        MEMORY_WRITES_SEEN_V88.set(0);
        Self(
            SOURCE_ISSUED_MEMORY_PAYLOAD_OBSERVER_V30.replace(Some(observe_memory_write_v88)),
            SOURCE_ISSUED_MEMORY_PAYLOAD_QUERY_FAULT_V30.replace(fault),
        )
    }
}

impl Drop for MemoryWriteObserverV88 {
    fn drop(&mut self) {
        SOURCE_ISSUED_MEMORY_PAYLOAD_OBSERVER_V30.set(self.0);
        SOURCE_ISSUED_MEMORY_PAYLOAD_QUERY_FAULT_V30.set(self.1);
    }
}

fn observe_memory_write_v88(
    original: &SourceIssuedOriginalV29<'_, '_, '_>,
    anchor: usize,
    row: &ScopedMemoryAnchorV29,
    operation: &Operation,
    actual: &SourceIssuedActualV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert!(matches!(operation.kind, OperationKind::GuardedStore { .. }));
    let ScopedMemoryAnchorKindV29::Access {
        payload:
            Some(ScopedMemoryPayloadV29::Store {
                value,
                source:
                    ScopedMemoryStoreSourceV29::Operand {
                        role: ExecutionOperandV29::CallArgument(2),
                        source: ScopedMemoryOperandSourceV29::Memory { access, .. },
                        ..
                    },
            }),
        ..
    } = row.kind
    else {
        panic!("authentic memory-derived write payload");
    };
    assert!(access < anchor);
    let producer = actual.value(value, budget)?.operation.unwrap();
    let OperationKind::Load { pointer, .. } = producer.kind else {
        panic!("write payload must come from the original shared-slice load");
    };
    assert!(matches!(actual.value(pointer, budget)?.ty,
        Type::Pointer(pointer) if pointer.access == AccessMode::ReadOnly));
    assert!(std::ptr::eq(
        original.source_index.emitted.operation(
            original.instance,
            row.block,
            row.position,
            budget,
        )?,
        operation
    ));
    MEMORY_WRITES_SEEN_V88.set(MEMORY_WRITES_SEEN_V88.get() + 1);
    Ok(())
}

fn run_memory_write_v88(
    work: usize,
    storage: usize,
    optimized: bool,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    scoped_raw_admission_v29::issued_role_tests_v29::run_issued_role_source_v87(
        memory_write_owner_v88(),
        fe2o3_kernel_descriptor::AccessMode::WriteOnly,
        work,
        storage,
        &std::cell::Cell::new(None),
        |source, budget| {
            let reached = std::cell::Cell::new(false);
            if optimized {
                let (output, (), _) = source.with_checked_mixed_fixedpoint_optimization_v18(
                    budget,
                    |original, optimized, budget| {
                        slice_view_v1::test_optimized_writes_v87(
                            original, optimized, 1, None, &reached, budget,
                        )?;
                        Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                    },
                )?;
                assert_eq!(output.execution().policy_version(), 11);
                drop(output);
            } else {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(inventory, budget, |original, budget| {
                            scoped_raw_admission_v29::with_checked_source_memory_v29(original, 0, None, budget, |_, budget| {
                                let rows = issued_rows_v18(original);
                                assert_eq!(rows.writes.len(), 1);
                                assert_eq!(rows.writes[0].root_parameter, 0);
                                scoped_raw_admission_v29::test_issued_copied_rows_replay_v26(original, 0, rows, budget)?;
                                reached.set(true);
                                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                            })
                        })
                    })
                })?;
            }
            assert!(reached.get());
            Ok(())
        },
    )
}

#[test]
fn thread_write_memory_payload_replays_readonly_input_before_and_after_optimization() {
    for optimized in [false, true] {
        let _observer = MemoryWriteObserverV88::install(0);
        run_memory_write_v88(LIMIT, LIMIT, optimized).0.unwrap();
        assert!(MEMORY_WRITES_SEEN_V88.get() > 0);
    }
}

#[test]
fn thread_write_memory_payload_rejects_forged_producer_occurrence_value_and_role() {
    for fault in 1..=5 {
        let _observer = MemoryWriteObserverV88::install(fault);
        assert!(run_memory_write_v88(LIMIT, LIMIT, false).0.is_err());
        assert!(
            MEMORY_WRITES_SEEN_V88.get() > 0,
            "mutation {fault} must reach payload replay"
        );
    }
}

#[test]
fn thread_write_memory_payload_has_exact_and_one_short_transaction_resources() {
    let (result, work, storage) = run_memory_write_v88(LIMIT, LIMIT, true);
    result.unwrap();
    let exact = run_memory_write_v88(work, storage, true);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(run_memory_write_v88(work - 1, storage, true).0.is_err());
    assert!(run_memory_write_v88(work, storage - 1, true).0.is_err());
}

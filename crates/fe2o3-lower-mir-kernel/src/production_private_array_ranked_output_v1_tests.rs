// These lowerer components use admitted source and real compiled ranked graphs.
// The backend companion separately covers the production source projector.
fn array_output_ranked_receipt_v1(
    source: ProductionPreRankedKirOwnerV1,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let layout = source.source_launch().roots()[0].layout();
    let original = &source.correspondence.private_arrays;
    let mut operations = vec![ProductionRankedOperationV1::ExecutionLayout {
        grid_identity: layout.grid_identity(),
        global_extents: layout.global_extents(),
        workgroup_extents: layout.workgroup_extents(),
        subgroup_size: layout.subgroup_size(),
        full_physical_workgroups: layout.full_physical_workgroups(),
    }];
    let mut value = 0u32;
    let mut views = Vec::new();
    for slot in &original.slots {
        let view = ProductionRankedValueIdV1::new(value);
        value += 1;
        let origin = (1u64 << 63) + u64::from(slot.local) + 1;
        operations.push(ProductionRankedOperationV1::ViewInSpace {
            result: view,
            element_width: u32::try_from(slot.element_facts.size * 8).unwrap(),
            writable: true,
            shape: vec![slot.length],
            dynamic_extents: vec![],
            memory_space: dialect_kernel::MemorySpaceAttr::Private,
            allocation_origin: origin,
            noalias_class: origin,
        });
        views.push((slot.local, view));
    }
    let mut sources = Vec::new();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget
        .reserve_storage(source.retained_analysis_storage_v1())
        .unwrap();
    for effect in &original.effects {
        let (ordinal, offset) = match effect.original_index {
            PrivateArrayIndexV1::InitializerElement { component, .. } => {
                (component, u64::from(component))
            }
            _ => (
                0,
                source
                    .materialized_private_array_constant_index(
                        effect.owner,
                        effect.function,
                        site(effect.semantic_block, effect.semantic_statement),
                        effect.role,
                        &mut budget,
                    )
                    .unwrap()
                    .unwrap(),
            ),
        };
        let index = ProductionRankedValueIdV1::new(value);
        value += 1;
        operations.push(ProductionRankedOperationV1::IndexConstant {
            result: index,
            value: offset,
        });
        sources.push(ProductionRankedAccessSourceV1::new(
            effect.semantic_block,
            Some(effect.semantic_statement),
            ordinal,
            0,
            u32::try_from(operations.len()).unwrap(),
        ));
        operations.push(ProductionRankedOperationV1::Access {
            kind: match effect.access {
                PrivateArrayAccessV1::Write => dialect_kernel::AccessKindAttr::Write,
                PrivateArrayAccessV1::Read => dialect_kernel::AccessKindAttr::Read,
            },
            view: ProductionRankedValueV1::Local(
                views
                    .iter()
                    .find(|(local, _)| *local == effect.local)
                    .unwrap()
                    .1,
            ),
            indices: vec![ProductionRankedValueV1::Local(index)],
        });
    }
    let kernel = ProductionRankedKernelV1::new(
        "private_array_relation",
        0,
        vec![ProductionRankedBlockV1::new(
            operations,
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("private_array_relation", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    let root = ProductionRankedSemanticProjectionRootV1::new(
        ARRAY_ROOT,
        1,
        lowering,
        "structured private-array correspondence component".to_owned(),
        sources,
        vec![],
    );
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        source,
        vec![root],
    )
    .unwrap()
}

fn with_array_ranked_output_v1(
    case: ArrayCase,
    next: impl FnOnce(
        &ProductionMaterializedRankedModuleReceiptV1,
        &VerifiedCanonicalKernelIrModuleV12,
        &CheckedNeutralKernelIrOwnerV1,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let receipt = array_output_ranked_receipt_v1(array_owner(case));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let source_storage = receipt.materialized.retained_analysis_storage_v1();
    budget.reserve_storage(FLOOR + source_storage).unwrap();
    let (bound, bound_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            receipt.materialized.executable().module(),
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(bound_storage.retained_storage())
        .unwrap();
    let checked = optimize(&bound, &mut budget);
    let live = budget.storage();
    next(&receipt, &bound, &checked, &mut budget);
    assert_eq!(budget.storage(), live);
    let checked_storage = checked.storage().retained_storage();
    drop(checked);
    budget.release_storage(checked_storage).unwrap();
    drop(bound);
    budget
        .release_storage(bound_storage.retained_storage())
        .unwrap();
    drop(receipt);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn private_output_ranked_receipt_closes_every_component_and_ordinary_store() {
    for (values, float) in [
        ([11; 8], false),
        ([0, 1, 2, 3, 7, 31, 255, u32::MAX], false),
        (
            [
                0,
                0x8000_0000,
                0x3f80_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0001,
                0x7fc0_0002,
                1,
            ],
            true,
        ),
    ] {
        for repetitions in [1, 2] {
            with_array_ranked_output_v1(
                ArrayCase::Initializer {
                    values,
                    repetitions,
                    float,
                },
                |receipt, bound, output, budget| {
                    let incoming = budget.storage();
                    receipt.with_checked_private_array_output_v1(bound, output, budget, |scope, budget| {
                    assert_eq!(scope.len(budget).unwrap(), repetitions * 8 + 1);
                    assert!(!scope.grants_artifact_or_launch_authority());
                    for index in 0..repetitions * 8 + 1 {
                        let fact = scope.get(index, budget).unwrap().unwrap();
                        let key = fact.source_key();
                        assert_eq!((key[0], key[1], key[2]), (ARRAY_ROOT.index(), ARRAY_ROOT.index(), 0));
                        if index < repetitions * 8 {
                            assert_eq!((key[3], key[6]), (index as u32 / 8 + 1, index as u32 % 8));
                        } else {
                            assert_eq!((key[3], key[6]), (repetitions as u32 + 1, 0));
                        }
                        assert_eq!(fact.offset(), u64::from(key[6]));
                        assert!(!fact.grants_authority());
                        let store = fact.output().unwrap();
                        assert!(store.executable());
                        assert_eq!(store.access().effect, 0);
                        let operation = source_output_operation_v1(output.owner(), store.access().operation, budget).unwrap();
                        assert!(matches!(operation.kind, OperationKind::Store { value, .. } if value == store.value()));
                    }
                    assert!(scope.get(repetitions * 8 + 1, budget).unwrap().is_none());
                    Ok(())
                }).unwrap();
                    assert_eq!(budget.storage(), incoming);
                },
            );
        }
    }
}

#[test]
fn private_output_keeps_the_actual_retained_read_refusal() {
    with_array_ranked_output_v1(
        ArrayCase::RetainedValueRead,
        |receipt, bound, output, budget| {
            let mut called = false;
            let result =
                receipt.with_checked_private_array_output_v1(bound, output, budget, |_, _| {
                    called = true;
                    Ok(())
                });
            assert!(!called);
            assert!(matches!(
                result,
                Err(ProductionSourceOutputErrorV1::PrivateArray(
                    SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                        "checked output currently requires an ordinary private-array write"
                    )
                ))
            ));
        },
    );
}

#[test]
fn ordinary_output_private_factor_preserves_public_work_errors_and_omitted_offset() {
    for source in [
        array_owner(ArrayCase::Write { sparse: false }),
        omitted_write_owner(),
    ] {
        with_output(source, |view, budget| {
            let selected = view.private_arrays[0].key;
            let selected_site = site(selected[2], selected[3]);
            let before = budget.work();
            let public = view
                .private_array_write(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    selected_site,
                    Role::Destination,
                    budget,
                )
                .unwrap();
            let public_work = budget.work() - before;
            let before = budget.work();
            let internal = view
                .private_array_write_with_offset_v1(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    selected_site,
                    Role::Destination,
                    budget,
                )
                .unwrap();
            assert_eq!(internal.0, public);
            assert_eq!(internal.1, Some(0));
            assert_eq!(budget.work() - before, public_work);
            if matches!(
                public,
                ProductionSourceOutputPrivateArrayAccessV1::OmittedUnreachable
            ) {
                assert!(
                    !view
                        .output()
                        .module()
                        .functions
                        .iter()
                        .filter_map(|f| f.body.as_ref())
                        .flat_map(|b| &b.blocks)
                        .flat_map(|b| &b.operations)
                        .any(|op| matches!(op.kind, OperationKind::Store { .. }))
                );
            }
            let floor = budget.storage();
            for private in [false, true] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
                let mut denied = AssertOriginBudgetV1::new(&mut work, STORAGE);
                denied.reserve_storage(floor).unwrap();
                let result = if private {
                    view.private_array_write_with_offset_v1(
                        ARRAY_ROOT,
                        ARRAY_ROOT,
                        selected_site,
                        Role::Destination,
                        &mut denied,
                    )
                    .map(|x| x.0)
                } else {
                    view.private_array_write(
                        ARRAY_ROOT,
                        ARRAY_ROOT,
                        selected_site,
                        Role::Destination,
                        &mut denied,
                    )
                };
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Work(_)
                    ))
                ));
                assert_eq!((denied.work(), denied.storage()), (0, floor));
            }
        });
    }
    with_output(
        array_owner(ArrayCase::ValueRead { local_index: false }),
        |view, budget| {
            let result = view
                .private_array_write_with_offset_v1(
                    ARRAY_ROOT,
                    ARRAY_ROOT,
                    site(0, 2),
                    Role::RvalueOperand(0),
                    budget,
                )
                .unwrap();
            assert_eq!(
                result,
                (
                    ProductionSourceOutputPrivateArrayAccessV1::ProvenUnretained,
                    None
                )
            );
        },
    );
}

#[test]
fn private_output_entry_and_scoped_getter_have_literal_work_boundaries() {
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            let floor = budget.storage();
            for limit in [7, 8] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut denied = AssertOriginBudgetV1::new(&mut work, STORAGE);
                denied.reserve_storage(floor).unwrap();
                let result = receipt.with_checked_private_array_output_v1(
                    bound,
                    output,
                    &mut denied,
                    |_, _| -> Result<(), ProductionSourceOutputErrorV1> {
                        panic!("entry denial cannot call the consumer");
                    },
                );
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Work(_)
                    ))
                ));
                assert_eq!(denied.storage(), floor);
                assert_eq!(denied.work(), if limit == 7 { 0 } else { 8 });
            }
            receipt
                .with_checked_private_array_output_v1(bound, output, budget, |scope, budget| {
                    let live = budget.storage();
                    for limit in [4, 5] {
                        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                        let mut foreign = AssertOriginBudgetV1::new(&mut work, STORAGE);
                        foreign.reserve_storage(live).unwrap();
                        let result = scope.get(0, &mut foreign);
                        if limit == 4 {
                            assert!(matches!(
                                result,
                                Err(ProductionSourceOutputErrorV1::Resource(
                                    AssertOriginResourceV1::Work(_)
                                ))
                            ));
                            assert_eq!(foreign.work(), 0);
                        } else {
                            assert!(matches!(
                                result,
                                Err(ProductionSourceOutputErrorV1::Resource(
                                    AssertOriginResourceV1::Accounting
                                ))
                            ));
                            assert_eq!(foreign.work(), 5);
                        }
                        assert_eq!(foreign.storage(), live);
                    }
                    Ok(())
                })
                .unwrap();
        },
    );
}

#[test]
fn private_output_scope_preserves_error_panic_and_detects_incoming_floor_loss() {
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            for mode in 0..3 {
                let incoming = budget.storage();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    receipt.with_checked_private_array_output_v1(
                        bound,
                        output,
                        budget,
                        |_, _| -> Result<(), ProductionSourceOutputErrorV1> {
                            match mode {
                                0 => Ok(()),
                                1 => {
                                    Err(ProductionSourceOutputErrorV1::Invalid("consumer sentinel"))
                                }
                                _ => std::panic::panic_any(71u32),
                            }
                        },
                    )
                }));
                match mode {
                    0 => assert!(matches!(result, Ok(Ok(())))),
                    1 => assert!(matches!(
                        result,
                        Ok(Err(ProductionSourceOutputErrorV1::Invalid(
                            "consumer sentinel"
                        )))
                    )),
                    _ => assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 71),
                }
                assert_eq!(budget.storage(), incoming);
            }
            for mode in 0..3 {
                let incoming = budget.storage();
                let result = receipt.with_checked_private_array_output_v1(
                    bound,
                    output,
                    budget,
                    |_, budget| -> Result<(), ProductionSourceOutputErrorV1> {
                        budget
                            .release_storage(budget.storage() - incoming + 1)
                            .unwrap();
                        match mode {
                            0 => Ok(()),
                            1 => Err(ProductionSourceOutputErrorV1::Invalid("lost floor")),
                            _ => panic!("lost floor"),
                        }
                    },
                );
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Accounting
                    ))
                ));
                assert_eq!(budget.storage(), incoming - 1);
                // The test owns the lost reservation; production must not invent it.
                budget.reserve_storage(1).unwrap();
            }
        },
    );
}

fn with_private_output_view_v1(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    bound: &VerifiedCanonicalKernelIrModuleV12,
    output: &CheckedNeutralKernelIrOwnerV1,
    budget: &mut AssertOriginBudgetV1<'_>,
    next: impl FnOnce(
        &mut ProductionSourceOutputOccurrencesV1<'_, '_>,
        &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
        &mut AssertOriginBudgetV1<'_>,
    ),
) {
    let incoming = budget.storage();
    let (coordinates, storage) = check_canonical_kir_coordinate_preservation_v1(
        receipt.materialized.executable(),
        bound,
        budget,
    )
    .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (mut view, storage) =
        derive_source_output_occurrences_v1(&receipt.materialized, &coordinates, output, budget)
            .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (inventory, storage) =
        fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(output.owner(), budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    next(&mut view, &inventory, budget);
    drop(inventory);
    drop(view);
    #[allow(
        clippy::drop_non_drop,
        reason = "End the borrowed coordinate witness before its reservation"
    )]
    drop(coordinates);
    budget.release_storage(budget.storage() - incoming).unwrap();
}

#[test]
fn private_output_all_seven_component_fields_and_original_identity_are_checked() {
    with_array_ranked_output_v1(
        ArrayCase::Initializer {
            values: [19; 8],
            repetitions: 1,
            float: false,
        },
        |receipt, bound, output, budget| {
            // Private row mutations exercise the real worker, not new authority
            // constructors or a claim that malformed source passed admission.
            for field in 0..7 {
                with_private_output_view_v1(
                    receipt,
                    bound,
                    output,
                    budget,
                    |view, inventory, budget| {
                        view.private_arrays[0].key[field] = 99;
                        view.private_arrays.sort_by_key(|row| row.key);
                        let error =
                            private_array_output_workspace_v1(receipt, view, inventory, budget)
                                .err()
                                .unwrap();
                        let expected = if field < 6 {
                            "initializer output component census changed"
                        } else {
                            "initializer original occurrence identity changed"
                        };
                        assert!(
                            matches!(error, ProductionSourceOutputErrorV1::Invalid(message) if message == expected)
                        );
                    },
                );
            }
            with_private_output_view_v1(
                receipt,
                bound,
                output,
                budget,
                |view, inventory, budget| {
                    view.private_arrays[0].original_effect = view.private_arrays[1].original_effect;
                    assert!(matches!(
                        private_array_output_workspace_v1(receipt, view, inventory, budget),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "initializer original occurrence identity changed"
                        ))
                    ));
                },
            );
        },
    );
}

#[test]
fn private_output_reverse_census_rejects_private_claim_and_allocation_mutations() {
    with_array_ranked_output_v1(
        ArrayCase::Initializer {
            values: [23; 8],
            repetitions: 1,
            float: false,
        },
        |receipt, bound, output, budget| {
            for mutation in 0..8 {
                with_private_output_view_v1(
                    receipt,
                    bound,
                    output,
                    budget,
                    |view, inventory, budget| {
                        let mut workspace =
                            private_array_output_workspace_v1(receipt, view, inventory, budget)
                                .unwrap();
                        let count = workspace.facts.len();
                        let expected = match mutation {
                            0 => {
                                workspace.sources[0].consumed = false;
                                "private output unconsumed ranked source"
                            }
                            1 => {
                                workspace.ranked[1] = workspace.ranked[0];
                                "private output ranked access reused"
                            }
                            2 => {
                                workspace.stores[1].key = workspace.stores[0].key;
                                "private output Store reused"
                            }
                            3 => {
                                workspace.stores.pop();
                                "private output unclaimed physical Store"
                            }
                            4 => {
                                workspace.allocations.clear();
                                "private output unclaimed physical allocation"
                            }
                            5 => {
                                workspace.allocations[1].key[2] += 1;
                                "private output slot split across allocations"
                            }
                            6 => {
                                workspace.allocations[1].slot += 1;
                                "private output allocation shared by original slots"
                            }
                            _ => {
                                workspace.facts[0].output.as_mut().unwrap().value =
                                    ValueId(u32::MAX);
                                "private output Store operand or effect identity changed"
                            }
                        };
                        assert!(
                            matches!(private_array_output_reverse_v1(&mut workspace, inventory, count,
                        receipt.materialized.limits.max_operations, budget),
                        Err(ProductionSourceOutputErrorV1::Invalid(message)) if message == expected)
                        );
                    },
                );
            }
        },
    );
}

#[test]
fn private_output_actual_verified_inventory_rejects_unclaimed_and_empty_summary_effects() {
    use fe2o3_kernel_ir::{
        AccessMode, AssemblyConstraint, AssemblyEffect, AssemblyOperand, AssemblyOperandKind,
        AssemblySourceIdentity, InlineAssembly, InlineAssemblyTarget, MemoryAccess, Operation,
        ScalarType, Type,
    };
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            for mutation in 0..6 {
                with_private_output_view_v1(
                    receipt,
                    bound,
                    output,
                    budget,
                    |view, inventory, budget| {
                        let mut workspace =
                            private_array_output_workspace_v1(receipt, view, inventory, budget)
                                .unwrap();
                        let fact = workspace.facts[0].output.as_ref().unwrap();
                        let coordinate = fact.access.operation;
                        let mut module = output.owner().module().clone();
                        let function = &mut module.functions[coordinate.block.function.0 as usize];
                        let body = function.body.as_mut().unwrap();
                        let new_value = ValueId(
                            body.parameters
                                .iter()
                                .map(|id| id.0)
                                .chain(
                                    body.blocks
                                        .iter()
                                        .flat_map(|block| &block.parameters)
                                        .map(|def| def.id.0),
                                )
                                .chain(
                                    body.blocks
                                        .iter()
                                        .flat_map(|block| &block.operations)
                                        .flat_map(|op| &op.results)
                                        .map(|def| def.id.0),
                                )
                                .max()
                                .unwrap()
                                + 1,
                        );
                        let block = &mut body.blocks[coordinate.block.block as usize];
                        let expected = match mutation {
                            0 => {
                                let extra = block.operations[coordinate.operation as usize].clone();
                                block.operations.push(extra);
                                "private output unclaimed physical Store"
                            }
                            1 => {
                                let mut extra =
                                    block.operations[fact.allocation.operation as usize].clone();
                                assert!(matches!(extra.kind, OperationKind::Alloca { .. }));
                                extra.results[0].id = new_value;
                                block.operations.push(extra);
                                "private output unclaimed physical allocation"
                            }
                            2 => {
                                function.signature.parameters.push(Type::pointer(
                                    Type::Scalar(ScalarType::U32),
                                    AddressSpace::Global,
                                    AccessMode::ReadWrite,
                                ));
                                body.parameters.push(new_value);
                                block.operations.push(Operation::new(
                                    vec![],
                                    OperationKind::Store {
                                        pointer: new_value,
                                        value: fact.value,
                                        access: MemoryAccess::new(AddressSpace::Global, 4),
                                    },
                                ));
                                "private output unclaimed physical Store"
                            }
                            _ => {
                                let effect = match mutation {
                                    3 => AssemblyEffect::Atomic,
                                    4 => AssemblyEffect::Barrier,
                                    _ => AssemblyEffect::ControlFlow,
                                };
                                let assembly = InlineAssembly {
                                    target: InlineAssemblyTarget::AmdGpuGfx942,
                                    source: AssemblySourceIdentity::new(
                                        [1; 32], [2; 32], [3; 32], [4; 32],
                                    ),
                                    mnemonic: "s_waitcnt".to_owned(),
                                    operands: vec![AssemblyOperand {
                                        kind: AssemblyOperandKind::ImmediateI32(0),
                                        constraint: AssemblyConstraint::ImmediateI32,
                                    }],
                                    options: Default::default(),
                                    declared_effects: [effect].into_iter().collect(),
                                };
                                let capabilities = assembly.required_capabilities();
                                function
                                    .required_capabilities
                                    .extend(capabilities.iter().cloned());
                                module
                                    .required_capabilities
                                    .extend(capabilities.iter().cloned());
                                for kernel in &mut module.kernels {
                                    if kernel.entry == function.id {
                                        kernel
                                            .required_capabilities
                                            .extend(capabilities.iter().cloned());
                                    }
                                }
                                block.operations.push(Operation::new(
                                    vec![],
                                    OperationKind::InlineAssembly(assembly),
                                ));
                                "private output opcode or ordering is outside the closed subset"
                            }
                        };
                        // The modified graph is genuinely verified, but it has no
                        // checked optimization history or source-owner authority.
                        let (altered, storage) = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                    &module, budget,
                ).unwrap();
                        budget.reserve_storage(storage.retained_storage()).unwrap();
                        let (altered_inventory, storage) =
                            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(
                                &altered, budget,
                            )
                            .unwrap();
                        budget.reserve_storage(storage.retained_storage()).unwrap();
                        if mutation >= 3 {
                            let operation = altered_inventory
                                .operations()
                                .iter()
                                .find(|op| {
                                    matches!(op.operation.kind, OperationKind::InlineAssembly(_))
                                })
                                .unwrap();
                            assert!(operation.effects.is_empty());
                            assert!(operation.compiler_ordering().is_empty());
                        }
                        let count = workspace.facts.len();
                        assert!(
                            matches!(private_array_output_reverse_v1(&mut workspace, &altered_inventory, count,
                    receipt.materialized.limits.max_operations, budget),
                    Err(ProductionSourceOutputErrorV1::Invalid(message)) if message == expected)
                        );
                        drop(altered_inventory);
                        drop(altered);
                        drop(workspace);
                    },
                );
            }
        },
    );
}

fn private_output_same_slot_v1<'w>(
    receipt: &ProductionMaterializedRankedModuleReceiptV1,
    bound: &VerifiedCanonicalKernelIrModuleV12,
    output: &CheckedNeutralKernelIrOwnerV1,
    budget: &mut AssertOriginBudgetV1<'w>,
    mode: u8,
) {
    let incoming = budget.storage();
    let foreign = AssertOriginBudgetV1::new(
        Box::leak(Box::new(CanonicalKernelIrWorkBudgetV1::new(WORK))),
        STORAGE,
    );
    let mut foreign: AssertOriginBudgetV1<'w> = foreign;
    let mut displaced = None;
    let mut prefix = 0;
    let mut full = 0;
    let result =
        receipt.with_checked_private_array_output_v1(bound, output, budget, |scope, budget| {
            assert_eq!(scope.len(budget).unwrap(), 1);
            full = budget.storage();
            prefix = budget.work();
            foreign.reserve_storage(full).unwrap();
            let slot = budget as *const _ as usize;
            std::mem::swap(budget, &mut foreign);
            assert_eq!(budget as *const _ as usize, slot);
            if mode < 2 {
                let rejected = if mode == 0 {
                    scope.len(budget).map(|_| ())
                } else {
                    scope.get(0, budget).map(|_| ())
                };
                assert!(matches!(
                    rejected,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Accounting
                    ))
                ));
                assert_eq!(
                    (
                        budget.work(),
                        budget.storage(),
                        budget.peak_storage(),
                        budget.failed_storage()
                    ),
                    (5, full, full, None)
                );
                std::mem::swap(budget, &mut foreign);
                assert_eq!(budget.work(), prefix);
                assert_eq!(scope.get(0, budget).unwrap().unwrap().offset(), 0);
                foreign.release_storage(full).unwrap();
                return Ok(());
            }
            displaced = Some(foreign);
            match mode {
                2 => Ok(()),
                3 => Err(ProductionSourceOutputErrorV1::Invalid("substituted ledger")),
                _ => std::panic::panic_any("substituted ledger panic"),
            }
        });
    if mode < 2 {
        result.unwrap();
        assert_eq!(budget.storage(), incoming);
    } else {
        assert!(matches!(
            result,
            Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting
            ))
        ));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (0, full, full, None)
        );
        let mut original = displaced.take().unwrap();
        assert_eq!((original.work(), original.storage()), (prefix, full));
        std::mem::swap(budget, &mut original);
        original.release_storage(full).unwrap();
        // All scope values were dropped; only the test owns the displaced
        // reservation and may explicitly release its dead scratch now.
        budget.release_storage(full - incoming).unwrap();
    }
    receipt
        .with_checked_private_array_output_v1(bound, output, budget, |scope, budget| {
            assert_eq!(scope.len(budget).unwrap(), 1);
            Ok(())
        })
        .unwrap();
    assert_eq!(budget.storage(), incoming);
}

#[test]
fn private_output_same_slot_swap_is_rejected_by_both_getters_and_postflight() {
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            for mode in 0..5 {
                private_output_same_slot_v1(receipt, bound, output, budget, mode);
            }
        },
    );
}

#[test]
fn private_output_scope_header_and_workspace_capacities_are_accounted() {
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            let incoming = budget.storage();
            let required = incoming + std::mem::size_of::<CheckedPrivateArrayOutputRankedV1<'_>>();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut denied = AssertOriginBudgetV1::new(&mut work, required - 1);
            denied.reserve_storage(incoming).unwrap();
            assert!(
                matches!(receipt.with_checked_private_array_output_v1(bound, output, &mut denied, |_, _| Ok(())),
            Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Storage(error)))
                if error.actual() == required && error.limit() == required - 1)
            );
            assert_eq!(
                (denied.work(), denied.storage(), denied.peak_storage()),
                (8, incoming, incoming)
            );
            with_private_output_view_v1(
                receipt,
                bound,
                output,
                budget,
                |view, inventory, budget| {
                    let before = budget.storage();
                    let workspace =
                        private_array_output_workspace_v1(receipt, view, inventory, budget)
                            .unwrap();
                    let bytes = std::mem::size_of_val(&workspace)
                        + workspace.originals.capacity()
                            * std::mem::size_of::<PrivateArrayOutputOriginalV1>()
                        + workspace.sources.capacity()
                            * std::mem::size_of::<PrivateArrayOutputSourceV1>()
                        + workspace.definitions.capacity()
                            * std::mem::size_of::<PrivateArrayOutputDefinitionV1<'_>>()
                        + workspace.facts.capacity()
                            * std::mem::size_of::<PrivateArrayOutputFactV1>()
                        + workspace.stores.capacity()
                            * std::mem::size_of::<PrivateArrayOutputClaimV1>()
                        + workspace.allocations.capacity()
                            * std::mem::size_of::<PrivateArrayOutputAllocationV1>()
                        + workspace.ranked.capacity() * std::mem::size_of::<[usize; 3]>();
                    assert_eq!(budget.storage() - before, bytes);
                },
            );
        },
    );
}

#[test]
fn private_output_rejects_a_different_actual_bound_or_checked_history() {
    with_array_ranked_output_v1(
        ArrayCase::Initializer {
            values: [31; 8],
            repetitions: 1,
            float: false,
        },
        |receipt, bound, output, budget| {
            let incoming = budget.storage();
            let other = array_owner(ArrayCase::Initializer {
                values: [37; 8],
                repetitions: 1,
                float: false,
            });
            budget
                .reserve_storage(other.retained_analysis_storage_v1())
                .unwrap();
            let other_output = optimize(other.executable(), budget);
            for (actual_bound, actual_output) in
                [(other.executable(), output), (bound, &other_output)]
            {
                let floor = budget.storage();
                let mut called = false;
                let result = receipt.with_checked_private_array_output_v1(
                    actual_bound,
                    actual_output,
                    budget,
                    |_, _| {
                        called = true;
                        Ok(())
                    },
                );
                assert!(!called);
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::InputCustody)
                ));
                assert_eq!(budget.storage(), floor);
            }
            drop(other_output);
            drop(other);
            budget.release_storage(budget.storage() - incoming).unwrap();
        },
    );
}

#[test]
fn private_output_retained_floor_loss_precedes_ok_error_and_panic() {
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            for mode in 0..3 {
                let incoming = budget.storage();
                let mut entered = false;
                let result = receipt.with_checked_private_array_output_v1(
                    bound,
                    output,
                    budget,
                    |_, budget| {
                        entered = true;
                        assert!(budget.storage() > incoming);
                        budget.release_storage(1).unwrap();
                        assert!(budget.storage() >= incoming);
                        match mode {
                            0 => Ok(()),
                            1 => Err(ProductionSourceOutputErrorV1::Invalid(
                                "retained floor lost",
                            )),
                            _ => std::panic::panic_any("retained floor panic"),
                        }
                    },
                );
                assert!(entered);
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::Resource(
                        AssertOriginResourceV1::Accounting
                    ))
                ));
                assert_eq!(budget.storage(), incoming);
            }
        },
    );
}

#[test]
fn private_output_partial_workspace_and_buffer_reservation_failures_are_prepaid() {
    with_array_ranked_output_v1(
        ArrayCase::Write { sparse: false },
        |receipt, bound, output, budget| {
            with_private_output_view_v1(
                receipt,
                bound,
                output,
                budget,
                |view, inventory, budget| {
                    // The real workspace fails at the second buffer's precharge, after
                    // the first capacity is allocated/reconciled. This is a worker
                    // component: its enclosing production scope owns storage cleanup.
                    let root_count = receipt.roots.len();
                    let block_count: usize = receipt
                        .roots
                        .iter()
                        .map(|root| root.lowering.kernel().blocks().len())
                        .sum();
                    let operation_count: usize = receipt
                        .roots
                        .iter()
                        .flat_map(|root| root.lowering.kernel().blocks())
                        .map(|block| block.operations().len())
                        .sum();
                    let prefix = 3 + root_count * 3 + block_count + operation_count * 10 + 3 + 6;
                    let incoming = budget.storage();
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(prefix);
                    let mut denied = AssertOriginBudgetV1::new(&mut work, STORAGE);
                    denied.reserve_storage(incoming).unwrap();
                    assert!(
                        matches!(private_array_output_workspace_v1(receipt, view, inventory, &mut denied),
                Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(error)))
                    if error.actual() == prefix + 3 && error.limit() == prefix)
                    );
                    assert_eq!(denied.work(), prefix);
                    assert!(
                        denied.storage()
                            >= incoming
                                + std::mem::size_of::<PrivateArrayOutputWorkspaceV1<'_>>()
                                + std::mem::size_of::<PrivateArrayOutputOriginalV1>()
                    );
                    assert_eq!(denied.failed_storage(), None);
                    denied.release_storage(denied.storage() - incoming).unwrap();
                    assert_eq!(denied.storage(), incoming);
                },
            );
            // A second actual requested reservation fails while a first real Vec
            // remains live. Use its actual capacity, not allocator-slack assumptions.
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut first_budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            let first =
                private_array_output_vec_v1::<PrivateArrayOutputOriginalV1>(1, &mut first_budget)
                    .unwrap();
            let bytes = first.capacity() * std::mem::size_of::<PrivateArrayOutputOriginalV1>();
            assert_eq!(first_budget.storage(), bytes);
            // Explicit test-only reservation transfer; no scope/token is involved.
            let full = FLOOR + bytes;
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut second_budget = AssertOriginBudgetV1::new(&mut work, full);
            second_budget.reserve_storage(full).unwrap();
            first_budget.release_storage(bytes).unwrap();
            let requested = std::mem::size_of::<PrivateArrayOutputSourceV1>();
            assert!(
                matches!(private_array_output_vec_v1::<PrivateArrayOutputSourceV1>(1, &mut second_budget),
            Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Storage(error)))
                if error.actual() == full + requested && error.limit() == full)
            );
            assert_eq!(
                (
                    second_budget.work(),
                    second_budget.storage(),
                    second_budget.peak_storage()
                ),
                (3, full, full)
            );
            assert_eq!(second_budget.failed_storage(), Some(full + requested));
            drop(first);
            second_budget.release_storage(bytes).unwrap();
            assert_eq!(second_budget.storage(), FLOOR);
        },
    );
}

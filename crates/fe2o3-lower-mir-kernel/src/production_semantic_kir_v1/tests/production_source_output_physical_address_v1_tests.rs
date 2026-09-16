#[cfg(test)]
mod physical_address_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKirBlockCoordinateV1 as Block,
        CanonicalKirFunctionCoordinateV1 as Function,
    };

    fn pointer() -> SourceOutputAddressDefV1 {
        SourceOutputAddressDefV1::Result {
            operation: SourceOutputAddressOpV1 {
                block: Block {
                    function: Function(0),
                    block: 0,
                },
                operation: 2,
            },
            result: 0,
        }
    }

    fn with_pointer_inventory(
        extra: bool,
        body: impl FnOnce(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
            &mut AssertOriginBudgetV1<'_>,
        ),
    ) {
        use fe2o3_kernel_ir::{
            Function as KirFunction, VerifiedCanonicalKernelIrModuleV12 as Owner,
        };
        let scalar = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let slice = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(2), Type::INDEX),
                OperationKind::Constant(Constant::Index(3)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(3), pointer.clone()),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), pointer.clone()),
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(1),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(5), scalar),
                OperationKind::Load {
                    pointer: ValueId(4),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(4),
                    value: ValueId(5),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        if extra {
            block.operations.push(Operation::effect_free(
                ValueDef::new(ValueId(6), pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(3),
                    offset: ValueId(2),
                },
            ));
        }
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("address-component");
        module.functions.push(KirFunction::kernel_entry(
            "entry",
            Signature::new(vec![slice, Type::INDEX], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "address-component",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        let mut work = Work::new(1_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(19).unwrap();
        let (owner, owner_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (inventory, storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        body(&inventory, &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(inventory);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(owner);
        budget
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 19);
    }

    fn operation(ordinal: u32) -> SourceOutputAddressOpV1 {
        SourceOutputAddressOpV1 {
            block: Block {
                function: Function(0),
                block: 0,
            },
            operation: ordinal,
        }
    }

    #[test]
    fn address_own_operand_component_rejects_equal_typed_substitution_and_wrong_slot() {
        with_pointer_inventory(false, |inventory, budget| {
            let function = &inventory.functions()[0];
            let gep = source_output_address_operation_v1(inventory, function, operation(2), budget)
                .unwrap();
            let before = budget.work();
            let offset =
                source_output_address_operand_v1(inventory, gep, 1, ValueId(1), budget).unwrap();
            assert_eq!(
                offset.coordinate,
                SourceOutputAddressDefV1::FunctionArgument {
                    function: Function(0),
                    argument: 1
                }
            );
            assert_eq!(budget.work(), before + 7);
            for (slot, value) in [(1, ValueId(2)), (0, ValueId(1)), (2, ValueId(1))] {
                let before = budget.work();
                assert!(matches!(
                    source_output_address_operand_v1(inventory, gep, slot, value, budget),
                    Err(ProductionSourceOutputErrorV1::Invalid(_))
                ));
                assert_eq!(budget.work(), before + 7);
            }
            let mut wrong = operation(2);
            wrong.block.function = Function(1);
            assert!(matches!(
                source_output_address_operation_v1(inventory, function, wrong, budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "physical address function differs"
                ))
            ));
        });
    }

    #[test]
    fn address_pointer_census_component_accepts_shared_gep_and_rejects_every_extra_consumer() {
        for extra in [false, true] {
            with_pointer_inventory(extra, |inventory, budget| {
                for omitted in [None, Some(0), Some(1), Some(2)] {
                    let floor = budget.storage();
                    let result = source_output_global_scratch_scope_v1(budget, |budget| {
                        budget
                            .reserve_storage(std::mem::size_of::<SourceOutputAddressWorkspaceV1>())
                            .map_err(ProductionSourceOutputErrorV1::Resource)?;
                        let mut workspace = SourceOutputAddressWorkspaceV1::default();
                        // Source SliceData is consumed by one GEP; the same GEP
                        // pointer is used by both a Load and a Store.
                        for (ordinal, (definition, consumer)) in
                            [(1, 2), (2, 3), (2, 4)].into_iter().enumerate()
                        {
                            let definition = SourceOutputAddressDefV1::Result {
                                operation: operation(definition),
                                result: 0,
                            };
                            // Keep every tracked pointer in the census even
                            // when its claimed allowed-use row is omitted.
                            assert_origin_push_v1(&mut workspace.pointers, definition, budget)
                                .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                            if omitted == Some(ordinal) {
                                continue;
                            }
                            assert_origin_push_v1(
                                &mut workspace.allowed_uses,
                                (
                                    definition,
                                    SourceOutputAddressUseV1::OperationOperand {
                                        operation: operation(consumer),
                                        operand: 0,
                                    },
                                ),
                                budget,
                            )
                            .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                        }
                        source_output_address_check_consumers_v1(
                            &mut workspace,
                            inventory,
                            &inventory.functions()[0],
                            budget,
                        )
                    });
                    assert_eq!(result.is_ok(), !extra && omitted.is_none());
                    assert_eq!(budget.storage(), floor);
                }
            });
        }
    }

    #[test]
    fn shared_public_private_leaf_body_keeps_exact_work_and_refusal_prefixes() {
        // Tests the one factored numeric lookup body, not fabricated coverage.
        // Public one-root prelude is roster2+require14; private is require14.
        let definition = SourceOutputAddressDefV1::FunctionArgument {
            function: Function(0),
            argument: 2,
        };
        let rows = [SourceOutputProjectionArgumentV1 {
            ranked_value: ProductionRankedValueV1::Argument(9),
            source_local: SemanticLocalIdV1::from_index(3),
            component: ProductionProjectionArgumentComponentV1::Scalar,
            scalar: source_output_address_u64_v1(),
            origin: SourceOutputProjectionLeafOriginV1::Formal(SourceOutputProjectionFormalV1 {
                original: definition,
                output: definition,
                output_value: ValueId(3),
            }),
        }];
        for prelude in [16, 14] {
            for exact in [true, false] {
                let total = 7 + prelude + 3;
                let mut work = Work::new(total - usize::from(!exact));
                let mut budget = AssertOriginBudgetV1::new(&mut work, 19);
                budget.reserve_storage(19).unwrap();
                budget.charge_work(7).unwrap();
                // Independently declared composed prefix, not a coverage check.
                budget.charge_work(prelude).unwrap();
                let result = source_output_control_leaf_rows_v1(
                    &rows,
                    &[],
                    0..1,
                    rows[0].ranked_value,
                    &mut budget,
                );
                if exact {
                    let Some(SourceOutputAddressLeafV1::Formal(found)) = result.unwrap() else {
                        panic!("exact formal row");
                    };
                    assert_eq!(found.original(), definition);
                    assert_eq!(found.output(), definition);
                    assert_eq!(found.output_value(), ValueId(3));
                    assert_eq!(found.source_local(), rows[0].source_local);
                } else {
                    assert!(result.is_err());
                }
                assert_eq!(budget.storage(), 19);
                assert_eq!(budget.work(), total - usize::from(!exact));
                assert_eq!(work.failed_work(), if exact { None } else { Some(total) });
            }
        }
        let mut work = Work::new(100);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
        assert!(
            source_output_control_leaf_rows_v1(
                &rows,
                &[],
                0..1,
                ProductionRankedValueV1::Argument(8),
                &mut budget
            )
            .unwrap()
            .is_none()
        );
        assert_eq!(budget.work(), 3);
        assert!(matches!(
            source_output_control_leaf_rows_v1(&rows, &[], 0..2, rows[0].ranked_value, &mut budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "control argument span absent"
            ))
        ));
        assert_eq!(budget.work(), 4);
    }

    fn used() -> SourceOutputAddressUseV1 {
        SourceOutputAddressUseV1::OperationOperand {
            operation: SourceOutputAddressOpV1 {
                block: Block {
                    function: Function(0),
                    block: 0,
                },
                operation: 3,
            },
            operand: 0,
        }
    }

    #[test]
    fn address_shape_component_exact_and_under_preserve_shared_history() {
        const HISTORY: usize = 7;
        const FLOOR: usize = 19;
        // One fixed shape/extent/bit-to-byte check charges seven before inspection.
        for repeats in [1, 2] {
            for exact in [true, false] {
                let total = HISTORY + repeats * 7;
                let mut work = Work::new(total - usize::from(!exact));
                let mut budget = AssertOriginBudgetV1::new(&mut work, FLOOR);
                budget.reserve_storage(FLOOR).unwrap();
                budget.charge_work(HISTORY).unwrap();
                for index in 0..repeats {
                    let result = source_output_address_shape_v1(
                        &[0],
                        &[ProductionRankedValueV1::Argument(3)],
                        32,
                        4,
                        &mut budget,
                    );
                    assert_eq!(result.is_ok(), exact || index + 1 < repeats);
                }
                assert_eq!(budget.storage(), FLOOR);
                assert_eq!(budget.peak_storage(), FLOOR);
                if exact {
                    assert_eq!(budget.work(), total);
                    assert_eq!(work.failed_work(), None);
                } else {
                    assert_eq!(budget.work(), total - 7);
                    assert_eq!(work.failed_work(), Some(total));
                }
            }
        }
    }

    #[test]
    fn address_shape_component_never_confuses_bits_bytes_or_dynamic_dimensions() {
        let mut work = Work::new(1000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
        let extent = [ProductionRankedValueV1::Argument(3)];
        for (shape, extents, width, bytes) in [
            (&[0][..], &extent[..], 4, 4),
            (&[0][..], &extent[..], 32, 8),
            (&[1][..], &extent[..], 32, 4),
            (&[0, 0][..], &extent[..], 32, 4),
            (&[0][..], &[][..], 32, 4),
            (&[0][..], &extent[..], 0, 0),
            (&[0][..], &extent[..], u32::MAX, u32::MAX),
        ] {
            assert!(matches!(
                source_output_address_shape_v1(shape, extents, width, bytes, &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(_))
            ));
        }
        assert_eq!(budget.work(), 7 * 7);
        assert_eq!(budget.storage(), 0);
    }

    fn pair(budget: &mut AssertOriginBudgetV1<'_>) -> Result<(), ProductionSourceOutputErrorV1> {
        budget
            .reserve_storage(std::mem::size_of::<SourceOutputAddressWorkspaceV1>())
            .map_err(ProductionSourceOutputErrorV1::Resource)?;
        let mut scratch = SourceOutputAddressWorkspaceV1::default();
        source_output_address_allow_v1(&mut scratch, pointer(), used(), budget)?;
        assert_eq!(scratch.pointers, [pointer()]);
        assert_eq!(scratch.allowed_uses, [(pointer(), used())]);
        Ok(())
    }

    #[test]
    fn address_pointer_pair_component_prepays_both_payloads_and_failure_prefixes() {
        const FLOOR: usize = 19;
        const HISTORY: usize = 7;
        let header = std::mem::size_of::<SourceOutputAddressWorkspaceV1>();
        let pointer = 4 * std::mem::size_of::<SourceOutputAddressDefV1>();
        let used = 4 * std::mem::size_of::<(SourceOutputAddressDefV1, SourceOutputAddressUseV1)>();
        for (storage, peak, prefix, failed) in [
            (
                FLOOR + header + pointer - 1,
                FLOOR + header,
                1,
                Some(FLOOR + header + pointer),
            ),
            (
                FLOOR + header + pointer + used - 1,
                FLOOR + header + pointer,
                3,
                Some(FLOOR + header + pointer + used),
            ),
            (
                FLOOR + header + pointer + used,
                FLOOR + header + pointer + used,
                4,
                None,
            ),
        ] {
            let mut work = Work::new(100);
            let mut budget = AssertOriginBudgetV1::new(&mut work, storage);
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(HISTORY).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, pair);
            assert_eq!(result.is_ok(), failed.is_none());
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.peak_storage(), peak);
            assert_eq!(budget.failed_storage(), failed);
            assert_eq!(budget.work(), HISTORY + prefix);
        }
    }

    #[test]
    fn address_pointer_pair_work_exact_under_and_panic_drop_before_floor_release() {
        const FLOOR: usize = 19;
        for exact in [true, false] {
            // Two minimum-capacity reserve1+push1 pairs; no hidden per-query reset.
            let mut work = Work::new(7 + 4 - usize::from(!exact));
            let mut budget = AssertOriginBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(FLOOR).unwrap();
            budget.charge_work(7).unwrap();
            let result = source_output_global_scratch_scope_v1(&mut budget, pair);
            assert_eq!(result.is_ok(), exact);
            assert_eq!(budget.storage(), FLOOR);
            assert_eq!(budget.work(), if exact { 11 } else { 10 });
            assert_eq!(work.failed_work(), if exact { None } else { Some(11) });
        }
        let mut work = Work::new(1000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 100_000);
        budget.reserve_storage(FLOOR).unwrap();
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), ProductionSourceOutputErrorV1> =
                source_output_global_scratch_scope_v1(&mut budget, |budget| {
                    pair(budget)?;
                    panic!("address component callback");
                });
        }));
        assert!(panic.is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 4);
        source_output_global_scratch_scope_v1(&mut budget, pair).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.work(), 8);
    }

    // Verified executable-IR components, not source/checked-transition authority.
    // These reach the actual address helpers without constructing a completed
    // Store analysis or claiming that the earlier Control grammar admits them.
    fn with_negative_address_inventory(
        offset: ValueId,
        body: impl FnOnce(
            &fe2o3_kernel_analysis::CanonicalKirInventoryV1<'_>,
            &mut AssertOriginBudgetV1<'_>,
        ),
    ) {
        use fe2o3_kernel_ir::{
            Function as KirFunction, VerifiedCanonicalKernelIrModuleV12 as Owner,
        };
        let scalar = Type::Scalar(ScalarType::U32);
        let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let slice = Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
        let mut block = BasicBlock::new(BlockId(0));
        block.operations = vec![
            Operation::effect_free(
                ValueDef::new(ValueId(3), Type::INDEX),
                OperationKind::Constant(Constant::Index(0)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::INDEX),
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    lhs: ValueId(1),
                    rhs: ValueId(3),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(5), Type::INDEX),
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(2),
                    to: Type::INDEX,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(6), pointer.clone()),
                OperationKind::SliceData { slice: ValueId(0) },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(7), pointer.clone()),
                OperationKind::GetElementPointer {
                    base: ValueId(6),
                    offset,
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(8), pointer),
                OperationKind::GetElementPointer {
                    base: ValueId(7),
                    offset: ValueId(3),
                },
            ),
            Operation::new(
                vec![],
                OperationKind::Store {
                    pointer: ValueId(8),
                    value: ValueId(2),
                    access: MemoryAccess::new(AddressSpace::Global, 4),
                },
            ),
        ];
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut module = Module::new("address-negative-component");
        module.functions.push(KirFunction::kernel_entry(
            "entry",
            Signature::new(vec![slice, Type::INDEX, scalar], vec![]),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        module.kernels.push(Kernel::new(
            "address-negative-component",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        let mut work = Work::new(1_000_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(19).unwrap();
        let (owner, owner_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (inventory, storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let floor = budget.storage();
        body(&inventory, &mut budget);
        assert_eq!(budget.storage(), floor);
        drop(inventory);
        budget.release_storage(storage.retained_storage()).unwrap();
        drop(owner);
        budget
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 19);
    }

    #[test]
    fn address_scalar_grammar_direct_components_reject_arithmetic_and_nonidentity_cast() {
        let kernel = fe2o3_pliron::ProductionRankedKernelV1::new(
            "address_component_context",
            0,
            vec![fe2o3_pliron::ProductionRankedBlockV1::new(
                vec![],
                fe2o3_pliron::ProductionRankedTerminatorV1::Return,
            )],
        )
        .unwrap();
        let lowering = fe2o3_pliron::compile_ranked_kernel_for_lowering_v1(
            fe2o3_pliron::ProductionConstructionV1::ranked_kernel(
                "address_component_context",
                kernel,
            )
            .unwrap(),
            fe2o3_pliron::ProductionSessionLimitsV1::default(),
        )
        .unwrap();
        for (value, operation_index) in [(ValueId(4), 1), (ValueId(5), 2)] {
            with_negative_address_inventory(value, |inventory, budget| {
                let floor = budget.storage();
                source_output_global_scratch_scope_v1(budget, |budget| {
                    budget
                        .reserve_storage(
                            std::mem::size_of::<
                                SourceOutputControlNormalizationV1<'_, '_, '_, '_, '_>,
                            >() - std::mem::size_of::<SourceOutputAllocationScratchV1>(),
                        )
                        .map_err(ProductionSourceOutputErrorV1::Resource)?;
                    let ssa = source_output_allocation_scratch_v1(inventory, budget)?;
                    let mut context = SourceOutputControlNormalizationV1 {
                        invocation_roots: &[],
                        literal_uses: &[],
                        guard: None,
                        inner: SourceOutputScalarNormalizationV1 {
                            inventory,
                            function: &inventory.functions()[0],
                            lowering: &lowering,
                            sources: Vec::new(),
                            locations: Vec::new(),
                            sites: Vec::new(),
                            views: Vec::new(),
                            expressions: Vec::new(),
                            nodes: Vec::new(),
                            comparison: [(0, 0);
                                2 * MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                            visiting: [None; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
                            visiting_len: 0,
                            normalization_steps: 0,
                            ssa,
                            error: None,
                            budget,
                        },
                    };
                    source_output_address_scalar_grammar_v1(ValueId(1), &mut context)?;
                    source_output_address_scalar_grammar_v1(ValueId(3), &mut context)?;
                    let gep = source_output_address_operation_v1(
                        inventory,
                        &inventory.functions()[0],
                        operation(4),
                        context.inner.budget,
                    )?;
                    let definition = source_output_address_operand_v1(
                        inventory,
                        gep,
                        1,
                        value,
                        context.inner.budget,
                    )?;
                    assert_eq!(
                        definition.coordinate,
                        SourceOutputAddressDefV1::Result {
                            operation: operation(operation_index),
                            result: 0,
                        }
                    );
                    let prefix = context.inner.budget.work();
                    assert!(matches!(
                        source_output_address_scalar_grammar_v1(value, &mut context),
                        Err(ProductionSourceOutputErrorV1::Invalid(
                            "physical scalar arithmetic or ancestry unsupported"
                        ))
                    ));
                    assert!(context.inner.budget.work() > prefix);
                    Ok(())
                })
                .unwrap();
                assert_eq!(budget.storage(), floor);
            });
        }
    }

    #[test]
    fn address_pointer_census_direct_component_refuses_unaccounted_nested_gep() {
        with_negative_address_inventory(ValueId(1), |inventory, budget| {
            let nested = &inventory.operations()[5];
            assert!(matches!(
                nested.operation.kind,
                OperationKind::GetElementPointer {
                    base: ValueId(7),
                    offset: ValueId(3)
                }
            ));
            let floor = budget.storage();
            let result = source_output_global_scratch_scope_v1(budget, |budget| {
                budget
                    .reserve_storage(std::mem::size_of::<SourceOutputAddressWorkspaceV1>())
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let mut workspace = SourceOutputAddressWorkspaceV1::default();
                // Account SliceData -> first GEP, then deliberately omit the
                // first GEP's nested consumer. This is the pointer-use checker,
                // not a manufactured execution of the scoped nested-GEP branch.
                source_output_address_allow_v1(
                    &mut workspace,
                    SourceOutputAddressDefV1::Result {
                        operation: operation(3),
                        result: 0,
                    },
                    SourceOutputAddressUseV1::OperationOperand {
                        operation: operation(4),
                        operand: 0,
                    },
                    budget,
                )?;
                assert_origin_push_v1(
                    &mut workspace.pointers,
                    SourceOutputAddressDefV1::Result {
                        operation: operation(4),
                        result: 0,
                    },
                    budget,
                )
                .map_err(ProductionSourceOutputErrorV1::SourceOrigin)?;
                source_output_address_check_consumers_v1(
                    &mut workspace,
                    inventory,
                    &inventory.functions()[0],
                    budget,
                )
            });
            assert!(matches!(
                result,
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "physical pointer has an unaccounted or escaping use"
                ))
            ));
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[cfg(test)]
mod invocation_coordinate_components_v1 {
    use super::*;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use fe2o3_mir_model::{SsaDefinitionIdV1, SsaEdgeIdV1, SsaVariableIdV1};

    #[test]
    fn invocation_symbol_domain_has_no_formal_collision_or_base_overflow() {
        assert_eq!(source_output_invocation_symbol_slot_v1(0).unwrap(), 0);
        assert_eq!(source_output_invocation_symbol_slot_v1(3).unwrap(), 6);
        let maximum = ((u32::MAX - PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2) / 2) as usize;
        let slot = source_output_invocation_symbol_slot_v1(maximum).unwrap();
        assert!(
            PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                .checked_add(slot)
                .is_some()
        );
        for count in [maximum + 1, usize::MAX] {
            assert!(matches!(
                source_output_invocation_symbol_slot_v1(count),
                Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Arithmetic
                ))
            ));
        }
    }

    #[test]
    fn invocation_actual_o_root_recognizes_only_global_x_identity_transports() {
        use fe2o3_kernel_ir::{
            Axis, Function as KirFunction, IndexKind, IntrinsicKind, IntrinsicOperation,
            VerifiedCanonicalKernelIrModuleV12 as Owner,
        };
        for kind in [
            IntrinsicOperation::global_id_1d().kind,
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Y,
            },
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Global,
                axis: Axis::Z,
            },
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Local,
                axis: Axis::X,
            },
            IntrinsicKind::InvocationIndex {
                kind: IndexKind::Workgroup,
                axis: Axis::X,
            },
            IntrinsicKind::LaunchExtent { axis: Axis::X },
        ] {
            let mut block = BasicBlock::new(BlockId(0));
            block.operations = vec![
                Operation::effect_free(
                    ValueDef::new(ValueId(1), Type::INDEX),
                    OperationKind::Intrinsic(IntrinsicOperation::new(kind, Type::INDEX)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U64)),
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(1),
                        to: Type::Scalar(ScalarType::U64),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U64)),
                    OperationKind::Constant(Constant::U64(1)),
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U64)),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(2),
                        rhs: ValueId(3),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(5), Type::Scalar(ScalarType::U32)),
                    OperationKind::Cast {
                        kind: CastKind::Truncate,
                        value: ValueId(2),
                        to: Type::Scalar(ScalarType::U32),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(6), Type::Scalar(ScalarType::U64)),
                    OperationKind::Cast {
                        kind: CastKind::ZeroExtend,
                        value: ValueId(5),
                        to: Type::Scalar(ScalarType::U64),
                    },
                ),
                Operation::effect_free(
                    ValueDef::new(ValueId(7), Type::Scalar(ScalarType::I64)),
                    OperationKind::Cast {
                        kind: CastKind::Bitcast,
                        value: ValueId(2),
                        to: Type::Scalar(ScalarType::I64),
                    },
                ),
            ];
            block.terminator = Some(Terminator::Return { values: vec![] });
            let mut module = Module::new("invocation-root-component");
            module.functions.push(KirFunction::kernel_entry(
                "entry",
                Signature::new(vec![Type::Scalar(ScalarType::U64)], vec![]),
                vec![ValueId(0)],
                vec![block],
            ));
            module.kernels.push(Kernel::new(
                "invocation-root-component",
                "entry",
                LaunchDomain::D3 {
                    x: LaunchExtent::Static(1),
                    y: LaunchExtent::Static(1),
                    z: LaunchExtent::Static(1),
                },
            ));
            let mut work = Work::new(1_000_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
            budget.reserve_storage(19).unwrap();
            let (owner, storage) =
                Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let (inventory, inventory_storage) =
                fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut budget)
                    .unwrap();
            budget
                .reserve_storage(inventory_storage.retained_storage())
                .unwrap();
            let floor = budget.storage();
            let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0);
            for value in [ValueId(1), ValueId(2)] {
                let result = source_output_invocation_output_root_v1(
                    &inventory,
                    function,
                    value,
                    &mut budget,
                );
                if kind == IntrinsicOperation::global_id_1d().kind {
                    let (definition, value) = result.unwrap();
                    assert_eq!(value, ValueId(1));
                    assert!(
                        matches!(definition, SourceOutputAddressDefV1::Result { operation, result: 0 }
                        if operation.block.function == function && operation.block.block == 0 && operation.operation == 0)
                    );
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionSourceOutputErrorV1::Invalid(_))
                    ));
                }
            }
            for value in [
                ValueId(0),
                ValueId(3),
                ValueId(4),
                ValueId(5),
                ValueId(6),
                ValueId(7),
            ] {
                assert!(matches!(
                    source_output_invocation_output_root_v1(
                        &inventory,
                        function,
                        value,
                        &mut budget
                    ),
                    Err(ProductionSourceOutputErrorV1::Invalid(_))
                ));
            }
            assert_eq!(budget.storage(), floor);
            drop(inventory);
            budget
                .release_storage(inventory_storage.retained_storage())
                .unwrap();
            drop(owner);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), 19);
        }
    }

    fn source_index() -> SourceOutputInvocationSourceIndexV1 {
        // Numeric walk fixture only. This deliberately has no source owner and
        // cannot enter the source-authenticating relation constructors.
        SourceOutputInvocationSourceIndexV1 {
            source: std::ptr::null(),
            function: SemanticFunctionIdV1::from_index(0),
            canonical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(0),
            events: vec![],
            definitions: vec![(
                SsaValueV1::Definition(SsaDefinitionIdV1::new(7)),
                SourceOutputInvocationDefinitionV1::Edge(0),
            )],
            incoming: vec![],
            transports: vec![],
            statements: vec![],
            terminators: vec![],
            blocks: vec![],
            values: vec![],
        }
    }

    #[test]
    fn invocation_single_definition_walk_has_derived_work_and_storage_boundaries() {
        const HISTORY: usize = 7;
        const WORK: usize = 16 + 1 + 4;
        let index = source_index();
        let floor = 19
            + std::mem::size_of_val(&index)
            + index.definitions.capacity() * std::mem::size_of_val(&index.definitions[0]);
        let scratch = std::mem::size_of::<
            [Option<SsaValueV1>; MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1],
        >();
        for exact in [true, false] {
            let mut work = Work::new(HISTORY + WORK - usize::from(!exact));
            let mut budget = AssertOriginBudgetV1::new(&mut work, floor + scratch);
            budget.charge_work(HISTORY).unwrap();
            budget.reserve_storage(floor).unwrap();
            let result = source_output_invocation_definition_v1(
                &index,
                index.definitions[0].0,
                SemanticLocalIdV1::from_index(8),
                &mut budget,
            );
            if exact {
                assert!(
                    matches!(result, Ok((value, SourceOutputInvocationDefinitionV1::Edge(0)))
                    if value == index.definitions[0].0)
                );
                assert_eq!(budget.work(), HISTORY + WORK);
            } else {
                assert!(matches!(
                    result,
                    Err(ProductionSourceOutputErrorV1::SourceOrigin(_))
                ));
                assert_eq!(budget.work(), HISTORY + 16 + 1);
                // A denied charge records history but does not poison future
                // smaller charges. The production fallible chain must return.
                budget.charge_work(1).unwrap();
            }
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + scratch);
        }
        let mut work = Work::new(HISTORY + WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, floor + scratch - 1);
        budget.charge_work(HISTORY).unwrap();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            source_output_invocation_definition_v1(
                &index,
                index.definitions[0].0,
                SemanticLocalIdV1::from_index(8),
                &mut budget
            ),
            Err(ProductionSourceOutputErrorV1::Resource(_))
        ));
        assert_eq!(budget.work(), HISTORY);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
    }

    #[test]
    fn invocation_source_walk_requires_one_exact_incoming_transport_and_refuses_cycles() {
        let mut index = source_index();
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 0);
        let variable = SsaVariableIdV1::new(8);
        let value = SsaValueV1::BlockArgument {
            block: SsaBlockIdV1::new(1),
            variable,
        };
        index.incoming = vec![((1, edge), 0)];
        index.transports = vec![((edge, variable.get()), index.definitions[0].0)];
        let mut work = Work::new(10_000);
        let mut budget = AssertOriginBudgetV1::new(&mut work, 1_000_000);
        let floor = 19
            + std::mem::size_of_val(&index)
            + index.definitions.capacity() * std::mem::size_of_val(&index.definitions[0])
            + index.incoming.capacity() * std::mem::size_of_val(&index.incoming[0])
            + index.transports.capacity() * std::mem::size_of_val(&index.transports[0]);
        budget.reserve_storage(floor).unwrap();
        let before = budget.work();
        assert!(
            source_output_invocation_definition_v1(
                &index,
                value,
                SemanticLocalIdV1::from_index(8),
                &mut budget
            )
            .is_ok()
        );
        assert_eq!(budget.work() - before, (16 + 2 + 4 + 4) + (17 + 5));
        index.transports[0].1 = value;
        assert!(matches!(
            source_output_invocation_definition_v1(
                &index,
                value,
                SemanticLocalIdV1::from_index(8),
                &mut budget
            ),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "invocation source forwarding cycle"
            ))
        ));
        index.transports[0].1 = index.definitions[0].0;
        assert!(matches!(
            source_output_invocation_definition_v1(
                &index,
                value,
                SemanticLocalIdV1::from_index(9),
                &mut budget
            ),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "invocation forwarded variable differs"
            ))
        ));
        // Reserve the second row before growing this component-only fixture.
        assert_origin_push_v1(
            &mut index.incoming,
            ((1, SsaEdgeIdV1::new(SsaBlockIdV1::new(0), 1)), 1),
            &mut budget,
        )
        .unwrap();
        let retained = budget.storage();
        assert!(matches!(
            source_output_invocation_definition_v1(
                &index,
                value,
                SemanticLocalIdV1::from_index(8),
                &mut budget
            ),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "invocation source has multiple incoming edges"
            ))
        ));
        assert_eq!(budget.storage(), retained);
    }
}

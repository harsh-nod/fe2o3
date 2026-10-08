mod atomic_transport_tests_v1 {
    use super::*;
    use optimized_source_v18::atomic::{atomic_output_shape_v1, atomic_transport_headers_v1};

    fn operation() -> Operation {
        Operation::new(
            vec![ValueDef::new(ValueId(2), Type::Scalar(ScalarType::U32))],
            OperationKind::Atomic(Atomic {
                kind: AtomicKind::Add,
                pointer: ValueId(0),
                value: Some(ValueId(1)),
                compare: None,
                access: MemoryAccess::new(AddressSpace::Global, 4),
                scope: SynchronizationScope::System,
                ordering: MemoryOrdering::Relaxed,
                failure_ordering: None,
            }),
        )
    }

    fn checked_shape(before: &Operation, after: &Operation, ids: [u32; 3]) -> bool {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1024);
        let mut budget = ArgumentBudgetV1::new(&mut work, 37);
        budget.reserve_storage(37).unwrap();
        let result = atomic_output_shape_v1(
            before,
            after,
            ValueId(ids[0]),
            ValueId(ids[1]),
            ValueId(ids[2]),
            &mut budget,
        );
        assert_eq!(
            budget.storage(),
            37,
            "shape checking has no retained allocation"
        );
        result.is_ok()
    }

    #[test]
    fn optimized_atomic_closed_kinds_preserve_signedness() {
        for kind in [
            AtomicKind::Exchange,
            AtomicKind::Add,
            AtomicKind::Subtract,
            AtomicKind::Min,
            AtomicKind::Max,
            AtomicKind::BitAnd,
            AtomicKind::BitOr,
            AtomicKind::BitXor,
        ] {
            for scalar in [ScalarType::U32, ScalarType::I32] {
                let mut before = operation();
                let OperationKind::Atomic(atomic) = &mut before.kind else {
                    unreachable!()
                };
                atomic.kind = kind;
                before.results[0].ty = Type::Scalar(scalar);
                assert!(checked_shape(&before, &before, [0, 1, 2]));
                let mut changed = before.clone();
                changed.results[0].ty = Type::Scalar(if scalar == ScalarType::U32 {
                    ScalarType::I32
                } else {
                    ScalarType::U32
                });
                assert!(!checked_shape(&before, &changed, [0, 1, 2]));
            }
        }
    }

    #[test]
    fn optimized_atomic_ordering_and_scope_are_exact_not_lattices() {
        for ordering in [
            MemoryOrdering::Relaxed,
            MemoryOrdering::Acquire,
            MemoryOrdering::Release,
            MemoryOrdering::AcquireRelease,
            MemoryOrdering::SequentiallyConsistent,
        ] {
            for scope in [
                SynchronizationScope::Workgroup,
                SynchronizationScope::Device,
                SynchronizationScope::System,
            ] {
                let mut before = operation();
                let OperationKind::Atomic(a) = &mut before.kind else {
                    unreachable!()
                };
                a.ordering = ordering;
                a.scope = scope;
                assert!(checked_shape(&before, &before, [0, 1, 2]));
                for fault in 0..2 {
                    let mut changed = before.clone();
                    let OperationKind::Atomic(a) = &mut changed.kind else {
                        unreachable!()
                    };
                    if fault == 0 {
                        a.ordering = if ordering == MemoryOrdering::Relaxed {
                            MemoryOrdering::Acquire
                        } else {
                            MemoryOrdering::Relaxed
                        };
                    } else {
                        a.scope = if scope == SynchronizationScope::System {
                            SynchronizationScope::Workgroup
                        } else {
                            SynchronizationScope::System
                        };
                    }
                    assert!(!checked_shape(&before, &changed, [0, 1, 2]));
                }
            }
        }
    }

    #[test]
    fn optimized_atomic_output_access_and_kind_changes_refuse() {
        for fault in 0..4 {
            let before = operation();
            let mut after = before.clone();
            let OperationKind::Atomic(a) = &mut after.kind else {
                unreachable!()
            };
            match fault {
                0 => a.kind = AtomicKind::Subtract,
                1 => a.access.address_space = AddressSpace::Workgroup,
                2 => a.access.alignment = 8,
                3 => a.access.volatile = true,
                _ => unreachable!(),
            }
            assert!(!checked_shape(&before, &after, [0, 1, 2]));
        }
    }

    #[test]
    fn optimized_atomic_pointer_rhs_and_old_result_are_independent_occurrences() {
        let before = operation();
        for ids in [[9, 1, 2], [0, 9, 2], [0, 1, 9], [1, 0, 2]] {
            assert!(!checked_shape(&before, &before, ids));
        }
        let mut renamed = before.clone();
        let OperationKind::Atomic(a) = &mut renamed.kind else {
            unreachable!()
        };
        a.pointer = ValueId(10);
        a.value = Some(ValueId(11));
        renamed.results[0].id = ValueId(12);
        assert!(checked_shape(&before, &renamed, [10, 11, 12]));
        // This only tests shape joining. The public method additionally requires
        // these names to be actual checked-control output definitions.
    }

    #[test]
    fn optimized_atomic_missing_extra_or_changed_result_refuses() {
        let before = operation();
        for fault in 0..4 {
            let mut after = before.clone();
            match fault {
                0 => after.results.clear(),
                1 => after
                    .results
                    .push(ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32))),
                2 => after.results[0].ty = Type::BOOL,
                3 => after.kind = OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(0)),
                _ => unreachable!(),
            }
            assert!(!checked_shape(&before, &after, [0, 1, 2]));
        }
    }

    #[test]
    fn optimized_atomic_load_store_compare_and_volatile_do_not_enter_rmw_profile() {
        for fault in 0..6 {
            let mut input = operation();
            let OperationKind::Atomic(a) = &mut input.kind else {
                unreachable!()
            };
            match fault {
                0 => a.kind = AtomicKind::Load,
                1 => a.kind = AtomicKind::Store,
                2 => a.value = None,
                3 => a.compare = Some(ValueId(3)),
                4 => a.failure_ordering = Some(MemoryOrdering::Relaxed),
                5 => a.access.volatile = true,
                _ => unreachable!(),
            }
            assert!(!checked_shape(&input, &input, [0, 1, 2]));
        }
    }

    #[test]
    fn optimized_atomic_shape_work_exhaustion_is_not_refunded() {
        let input = operation();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(7);
        let mut budget = ArgumentBudgetV1::new(&mut work, 37);
        budget.reserve_storage(37).unwrap();
        assert!(
            atomic_output_shape_v1(
                &input,
                &input,
                ValueId(0),
                ValueId(1),
                ValueId(2),
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), 37);
        assert!(
            atomic_output_shape_v1(
                &input,
                &input,
                ValueId(0),
                ValueId(1),
                ValueId(2),
                &mut budget,
            )
            .is_err()
        );
    }

    #[test]
    fn optimized_atomic_headers_exact_and_one_short_are_precharged() {
        use fe2o3_kernel_analysis::CanonicalKirOutputUseV1 as V;
        use fe2o3_kernel_ir::{
            CanonicalKirDefinitionCoordinateV1 as D, CanonicalKirOperationCoordinateV1 as O,
            CanonicalKirUseCoordinateV1 as U,
        };
        use std::mem::size_of as h;
        let expected = h::<SourcePhysicalAccessV18<'_>>()
            + h::<Option<SourcePhysicalAccessV18<'_>>>()
            + h::<SourceOwnedResultV18<Option<SourcePhysicalAccessV18<'_>>>>()
            + h::<SourcePhysicalPayloadV18<'_>>()
            + h::<Option<SourcePhysicalPayloadV18<'_>>>()
            + h::<SourceOwnedResultV18<Option<SourcePhysicalPayloadV18<'_>>>>()
            + h::<ProductionOptimizedSourceAtomicRmwV1<'_>>()
            + h::<SourceOwnedResultV18<Option<ProductionOptimizedSourceAtomicRmwV1<'_>>>>()
            + h::<ProductionOptimizedSourceAtomicDispositionV1>()
            + 2 * h::<ScopedAtomicEffectV1>()
            + h::<SourceOwnedResultV18<ScopedAtomicEffectV1>>()
            + 3 * h::<O>()
            + h::<Option<O>>()
            + h::<SourceOwnedResultV18<Option<O>>>()
            + 2 * h::<V>()
            + h::<SourceOwnedResultV18<V>>()
            + 2 * h::<U>()
            + 2 * h::<D>()
            + 3 * h::<ValueId>()
            + 4 * h::<usize>()
            + h::<SourceOwnedResultV18<usize>>()
            + 2 * h::<&Operation>()
            + 2 * h::<&Atomic>()
            + 2 * h::<&ValueDef>()
            + 2 * h::<&ScalarType>()
            + h::<ScalarType>()
            + h::<SourceOwnedResultV18<()>>();
        assert_eq!(atomic_transport_headers_v1().unwrap(), expected);
        for limit in [expected, expected - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            let r = budget.reserve_storage(atomic_transport_headers_v1().unwrap());
            assert_eq!(r.is_ok(), limit == expected);
            assert_eq!(budget.storage(), if r.is_ok() { expected } else { 0 });
            if r.is_err() {
                assert_eq!(budget.failed_storage(), Some(expected));
            }
            assert_eq!(budget.work(), 0);
        }
    }

    #[test]
    fn optimized_atomic_query_is_nonapplicable_to_real_ordinary_source_memory() {
        run_optimized_source_v18(division_source_owner_v18, |view, budget| {
            let source = view.original_source(budget)?;
            let floor = budget.storage();
            source_scalar_normalization_scratch_v18(source.cleanup, budget, 0, |budget| {
                let mut ordinary = 0;
                for root in 0..source.root_count(budget)? {
                    let (_, function) = source.root(root, budget)?;
                    let input = view.input_inventory(budget)?;
                    for row in &input.operations()[input.functions()[function].operations.clone()] {
                        assert!(
                            view.atomic_memory_access_v1(root, row.coordinate, budget)?
                                .is_none()
                        );
                        if matches!(
                            row.operation.kind,
                            OperationKind::Load { .. }
                                | OperationKind::Store { .. }
                                | OperationKind::GuardedLoad { .. }
                                | OperationKind::GuardedStore { .. }
                        ) {
                            assert!(
                                view.scalar_memory_access(root, row.coordinate, budget)?
                                    .is_some()
                            );
                            ordinary += 1;
                        }
                    }
                }
                assert!(ordinary > 0);
                Ok(())
            })?;
            assert_eq!(budget.storage(), floor);
            Ok(())
        });
    }

    #[test]
    fn optimized_atomic_query_wrong_root_remains_a_retained_source_refusal() {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(division_source_owner_v18, &mut budget);
        let result = with_actual_optimized_source_v18(prepared, &mut budget, |view, budget| {
            let input = view.input_inventory(budget)?;
            let coordinate = input.operations()[0].coordinate;
            assert!(
                view.atomic_memory_access_v1(usize::MAX, coordinate, budget)
                    .is_err()
            );
            // Swallowing the query denial cannot make the enclosing owner pass.
            Ok(())
        });
        assert!(result.is_err());
    }

    #[test]
    fn optimized_atomic_actual_checked_control_retains_both_uses_and_old_result() {
        use fe2o3_kernel_analysis::CheckedCanonicalKirControlIndexV18;
        use fe2o3_kernel_ir::{
            BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Module, Signature,
            Terminator, VerifiedCanonicalKernelIrModuleV18,
        };
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(operation());
        block.terminator = Some(Terminator::Return { values: vec![] });
        let mut function = Function::kernel_entry(
            "atomic",
            Signature::new(
                vec![
                    Type::pointer(
                        Type::Scalar(ScalarType::U32),
                        AddressSpace::Global,
                        fe2o3_kernel_ir::AccessMode::ReadWrite,
                    ),
                    Type::Scalar(ScalarType::U32),
                ],
                vec![],
            ),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        );
        function.required_capabilities = function.derived_capabilities();
        let mut module = Module::new("atomic-checked-control-not-source-admission");
        module.functions.push(function);
        module.kernels.push(Kernel::new(
            "atomic",
            "atomic",
            LaunchDomain::D1 {
                x: LaunchExtent::Static(1),
            },
        ));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        let layouts = ProductionSemanticKirLimitsV1::default().storage_layout_limits;
        let (owner, receipt) =
            VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                &module,
                layouts,
                &mut budget,
            )
            .unwrap();
        budget
            .reserve_storage(MODULE_FLOOR + receipt.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let observed =
            fe2o3_pliron::optimize_neutral_kernel_ir_v18(&owner, layouts, &mut budget).unwrap();
        budget
            .reserve_storage(observed.storage().retained_storage())
            .unwrap();
        let adopted = observed
            .try_check_and_finish_with_v18(&mut budget, |checked, budget| {
                let (control, charge) =
                    CheckedCanonicalKirControlIndexV18::derive_v18(checked, budget).unwrap();
                budget.reserve_storage(charge.retained_storage())?;
                let before = checked
                    .input()
                    .operations()
                    .iter()
                    .find(|r| matches!(r.operation.kind, OperationKind::Atomic(_)))
                    .unwrap();
                let after = checked
                    .output()
                    .operations()
                    .iter()
                    .find(|r| matches!(r.operation.kind, OperationKind::Atomic(_)))
                    .unwrap();
                assert_eq!(
                    checked
                        .output()
                        .operations()
                        .iter()
                        .filter(|r| matches!(r.operation.kind, OperationKind::Atomic(_)))
                        .count(),
                    1
                );
                for ordinal in 0..2 {
                    let actual = control
                        .operand(
                            OptimizedUse::OperationOperand {
                                operation: before.coordinate,
                                operand: ordinal,
                            },
                            budget,
                        )
                        .unwrap()
                        .expect("reachable ordered atomic use");
                    assert_eq!(
                        actual.coordinate,
                        OptimizedUse::OperationOperand {
                            operation: after.coordinate,
                            operand: ordinal,
                        }
                    );
                }
                let OperationKind::Atomic(actual) = &after.operation.kind else {
                    unreachable!()
                };
                for (ordinal, expected) in [(0, actual.pointer), (1, actual.value.unwrap())] {
                    let mapped = control
                        .operand(
                            OptimizedUse::OperationOperand {
                                operation: before.coordinate,
                                operand: ordinal,
                            },
                            budget,
                        )
                        .unwrap()
                        .unwrap();
                    let row = checked
                        .output()
                        .definitions()
                        .iter()
                        .find(|r| r.coordinate == mapped.definition)
                        .unwrap();
                    assert_eq!(row.value, Some(expected));
                }
                assert_eq!(
                    atomic_output_shape_v1(
                        before.operation,
                        after.operation,
                        actual.pointer,
                        actual.value.unwrap(),
                        after.operation.results[0].id,
                        budget,
                    )?,
                    ScopedAtomicEffectV1::from_operation(match &before.operation.kind {
                        OperationKind::Atomic(a) => a,
                        _ => unreachable!(),
                    })
                    .unwrap()
                );
                drop(control);
                budget.release_storage(charge.retained_storage())?;
                Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
            })
            .unwrap();
        drop(adopted);
        assert_eq!(budget.storage(), floor);
        drop(owner);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

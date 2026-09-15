// Private graph-component tests, not source-admission or attachment evidence.
mod global_allocation_component_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
        CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
        VerifiedCanonicalKernelIrModuleV12 as Owner,
    };

    // This body is pinned to the complete pre-refactor production algorithm.
    fn predecessor(
        function: &Function,
        kir: &KirCorrelationIndexV1<'_>,
        value: ValueId,
        visited: &mut BTreeSet<ValueId>,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<u32> {
        let body = function.body.as_ref()?;
        let mut pending = vec![value];
        let mut origins = BTreeSet::new();
        while let Some(value) = pending.pop() {
            budget.charge()?;
            if !visited.insert(value) {
                continue;
            }
            if let Some(index) = body
                .parameters
                .iter()
                .position(|parameter| *parameter == value)
            {
                let ty = function.signature.parameters.get(index)?;
                if !matches!(ty, Type::Pointer(_) | Type::Slice(_)) {
                    return None;
                }
                origins.insert(u32::try_from(index).ok()?);
                if origins.len() > 1 {
                    return None;
                }
                continue;
            }
            if let Some(operation) = kir.definitions.get(&value) {
                match &operation.kind {
                    OperationKind::SliceData { slice } => pending.push(*slice),
                    OperationKind::GetElementPointer { base, .. } => pending.push(*base),
                    OperationKind::Cast { value, .. } => pending.push(*value),
                    OperationKind::Select {
                        true_value,
                        false_value,
                        ..
                    } => {
                        pending.push(*true_value);
                        pending.push(*false_value);
                    }
                    _ => return None,
                }
                continue;
            }
            let inputs = kir.block_parameter_inputs.get(&value)?;
            if inputs.is_empty() {
                return None;
            }
            pending.extend(inputs.iter().copied());
        }
        let mut origins = origins.into_iter();
        let origin = origins.next()?;
        origins.next().is_none().then_some(origin)
    }

    fn graph() -> Module {
        let mut module =
            value_translation_fixture(ProductionSemanticBinaryOpV2::Add, 0x3f80_0000).module;
        let rw = Type::pointer(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadWrite,
        );
        let ro = Type::pointer(
            Type::Scalar(ScalarType::F32),
            AddressSpace::Global,
            AccessMode::ReadOnly,
        );
        let function = &mut module.functions[0];
        function.signature.parameters.push(rw.clone());
        let body = function.body.as_mut().unwrap();
        body.parameters.push(ValueId(20));
        let entry = &mut body.blocks[0];
        entry.operations.extend([
            Operation::effect_free(
                ValueDef::new(ValueId(8), Type::BOOL),
                OperationKind::Constant(Constant::Bool(true)),
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(9), rw.clone()),
                OperationKind::Select {
                    condition: ValueId(8),
                    true_value: ValueId(3),
                    false_value: ValueId(4),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(10), ro.clone()),
                OperationKind::Cast {
                    kind: CastKind::RestrictPointerAccess,
                    value: ValueId(9),
                    to: ro.clone(),
                },
            ),
            Operation::effect_free(
                ValueDef::new(ValueId(12), rw),
                OperationKind::Select {
                    condition: ValueId(8),
                    true_value: ValueId(4),
                    false_value: ValueId(20),
                },
            ),
        ]);
        entry.terminator = Some(Terminator::Branch {
            target: BlockId(1),
            arguments: vec![ValueId(10)],
        });
        let mut loop_block = BasicBlock::new(BlockId(1));
        loop_block.parameters.push(ValueDef::new(ValueId(11), ro));
        loop_block.terminator = Some(Terminator::ConditionalBranch {
            condition: ValueId(8),
            then_target: BlockId(1),
            then_arguments: vec![ValueId(11)],
            else_target: BlockId(2),
            else_arguments: vec![],
        });
        let mut exit = BasicBlock::new(BlockId(2));
        exit.terminator = Some(Terminator::Return { values: vec![] });
        body.blocks.extend([loop_block, exit]);
        verify_module(&module).unwrap();
        module
    }

    #[test]
    fn legacy_allocation_adapter_preserves_results_charge_prefixes_and_visit_order() {
        let module = graph();
        let function = &module.functions[0];
        let mut indexing = UnsupportedIndexCorrelationBudgetV1 { remaining: 10_000 };
        let kir = build_kir_correlation_index(function.body.as_ref().unwrap(), 100, &mut indexing)
            .unwrap();
        for value in [0, 3, 4, 5, 9, 10, 11, 12, 20, 999] {
            for limit in 0..=20 {
                for seed in [None, Some(ValueId(3))] {
                    let mut old_visited = seed.into_iter().collect();
                    let mut new_visited = seed.into_iter().collect();
                    let mut old_budget = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                    let mut new_budget = UnsupportedIndexCorrelationBudgetV1 { remaining: limit };
                    let expected = predecessor(
                        function,
                        &kir,
                        ValueId(value),
                        &mut old_visited,
                        &mut old_budget,
                    );
                    let actual = external_allocation_parameter_v1(
                        function,
                        &kir,
                        ValueId(value),
                        &mut new_visited,
                        &mut new_budget,
                    );
                    assert_eq!(actual, expected, "value {value}, limit {limit}");
                    assert_eq!(new_budget.remaining, old_budget.remaining);
                    assert_eq!(new_visited, old_visited);
                }
            }
        }
    }

    #[test]
    fn borrowed_allocation_adapter_reuses_dense_scratch_for_slice_select_cast_and_loop() {
        let module = graph();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(23).unwrap();
        let (owner, owner_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (inventory, inventory_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        let floor = budget.storage();
        let mut scratch = source_output_allocation_scratch_v1(&inventory, &mut budget).unwrap();
        let scratch_storage = scratch.storage;
        let pointers = (
            scratch.pending.as_ptr(),
            scratch.visited.as_ptr(),
            scratch.incoming.as_ptr(),
            scratch.next.as_ptr(),
        );
        for (value, expected) in [
            (0, Some(0)),
            (3, Some(0)),
            (4, Some(0)),
            (9, Some(0)),
            (10, Some(0)),
            (11, Some(0)),
            (12, None),
            (5, None),
            (20, Some(1)),
        ] {
            assert_eq!(
                source_output_allocation_parameter_v1(
                    &inventory,
                    FunctionCoordinate(0),
                    ValueId(value),
                    &mut scratch,
                    &mut budget
                )
                .unwrap(),
                expected
            );
            assert_eq!(
                (
                    scratch.pending.as_ptr(),
                    scratch.visited.as_ptr(),
                    scratch.incoming.as_ptr(),
                    scratch.next.as_ptr()
                ),
                pointers
            );
            assert_eq!(budget.storage(), floor + scratch_storage);
        }
        assert_eq!(scratch.generation, 9);
        drop(scratch);
        budget.release_storage(scratch_storage).unwrap();
        assert_eq!(budget.storage(), floor);
        drop(inventory);
        budget
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        drop(owner);
        budget
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 23);
    }

    #[test]
    fn borrowed_allocation_scratch_prepays_the_exact_fresh_payload_before_allocation() {
        let module = graph();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        let (owner, owner_storage) =
            Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
        budget
            .reserve_storage(owner_storage.retained_storage())
            .unwrap();
        let (inventory, inventory_storage) =
            fe2o3_kernel_analysis::CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget
            .reserve_storage(inventory_storage.retained_storage())
            .unwrap();
        let d = inventory.definitions().len();
        let e = inventory.edge_arguments().len();
        let payload = std::mem::size_of::<SourceOutputAllocationScratchV1>()
            + (2 * d + e) * std::mem::size_of::<usize>()
            + (2 * d + e + 1) * std::mem::size_of::<ValueId>();
        let floor = budget.storage();
        let mut denied_work = Work::new(1_000_000);
        {
            let mut denied = Budget::new(&mut denied_work, floor + payload - 1);
            denied.charge_work(7).unwrap();
            denied.reserve_storage(floor).unwrap();
            assert!(matches!(
                source_output_allocation_scratch_v1(&inventory, &mut denied),
                Err(ProductionSourceOutputErrorV1::Resource(
                    AssertOriginResourceV1::Storage(_)
                ))
            ));
            assert_eq!(denied.work(), 19);
            assert_eq!(denied.storage(), floor);
        }
        assert_eq!(denied_work.failed_work(), None);
        drop(inventory);
        budget
            .release_storage(inventory_storage.retained_storage())
            .unwrap();
        drop(owner);
        budget
            .release_storage(owner_storage.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

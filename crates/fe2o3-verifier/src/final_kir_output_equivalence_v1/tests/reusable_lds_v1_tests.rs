use super::super::reusable_lds_v1::Transfers;
use super::*;
use fe2o3_kernel_ir::{
    ExecutionCapabilityOpV1, ExecutionCapabilityRoleV1 as Role,
    ExecutionCapabilitySourceOccurrenceV1, ExecutionElementLayoutV1, KernelContextSourceIdentityV1,
    KernelContextTypeV1, ReusableLdsConversionV1,
};

#[path = "reusable_lds_v1_tests/occurrence_tests.rs"]
mod occurrence_tests;

fn operations(module: &Module) -> &[Operation] {
    &module.functions[0].body.as_ref().unwrap().blocks[0].operations
}

fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn contract_mut(module: &mut Module, index: usize) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(value) = &mut operations_mut(module)[index].kind else {
        panic!("execution contract")
    };
    value
}

fn logical_module(borrowed: bool, addend: Option<u32>) -> Module {
    let mut module = kernel_module(addend);
    module.functions[0].id = execution_provenance().root.clone();
    module.kernels[0].entry = execution_provenance().root.clone();
    module.kernels[0].workgroup_size = Some(WorkgroupSize::new(1, 1, 1));
    let layout = ExecutionElementLayoutV1 {
        byte_size: 4,
        byte_alignment: 16,
    };
    let conversion = ReusableLdsConversionV1 {
        input: execution_identity(0x20),
        output: execution_identity(0x21),
        element: execution_identity(0x22),
        layout,
        elements: 16,
        defined_function: [3; 32],
        defined_abi: [4; 32],
        defined_body: [5; 32],
        source_binding: [6; 32],
    };
    let allocation = if borrowed {
        ExecutionCapabilityOperationV1::LdsAllocateBorrowed {
            workgroup_reference: execution_identity(0x12),
            workgroup: execution_identity(0x11),
            lds: conversion.input,
            element: conversion.element,
            layout,
            elements: 16,
        }
    } else {
        ExecutionCapabilityOperationV1::LdsAllocate {
            workgroup: execution_identity(0x11),
            lds: conversion.input,
            element: conversion.element,
            layout,
            elements: 16,
        }
    };
    let provenance = execution_provenance();
    let mut prefix = vec![
        Operation::kernel_context_issue(
            ValueId(100),
            KernelContextTypeV1::new(
                provenance.root.as_str(),
                provenance.kernel_marker,
                provenance.target_brand,
                provenance.launch_brand,
            ),
            KernelContextSourceIdentityV1::new([0x81; 32], [0x82; 32], [0x83; 32], [0x84; 32]),
        ),
        execution_operation(
            ValueDef::new(
                ValueId(101),
                execution_capability_type(execution_identity(0x11), Role::Workgroup),
            ),
            ExecutionCapabilityOperationV1::WorkgroupDerive {
                context: execution_identity(0x10),
                workgroup: execution_identity(0x11),
            },
            vec![ValueId(100)],
            &[execution_identity(0x10)],
            execution_identity(0x11),
            1,
        ),
        execution_operation(
            ValueDef::new(
                ValueId(102),
                execution_capability_type(conversion.input, conversion.input_role()),
            ),
            allocation,
            vec![ValueId(101)],
            &[execution_identity(if borrowed { 0x12 } else { 0x11 })],
            conversion.input,
            2,
        ),
        execution_operation(
            ValueDef::new(
                ValueId(103),
                execution_capability_type(conversion.output, conversion.output_role()),
            ),
            ExecutionCapabilityOperationV1::ReusableLdsConversion(conversion),
            vec![ValueId(102)],
            &[conversion.input],
            conversion.output,
            3,
        ),
    ];
    for (index, operation) in prefix.iter_mut().enumerate() {
        if let OperationKind::ExecutionCapability(contract) = &mut operation.kind {
            contract.source.occurrence =
                ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                    [0xa1; 32],
                    [0xa2; 32],
                    [0xa3; 32],
                    index as u32,
                    0,
                );
        }
    }
    prefix.append(operations_mut(&mut module));
    *operations_mut(&mut module) = prefix;
    set_requirements(&mut module);
    module
}

fn set_requirements(module: &mut Module) {
    let requirements = module.functions[0].derived_capabilities();
    module.functions[0].required_capabilities = requirements.clone();
    module.kernels[0].required_capabilities = requirements.clone();
    module.required_capabilities = requirements;
}

fn pair(addend: u32, extent: u32) -> (Module, Module) {
    let mut source = logical_module(true, None);
    let mut physical = static_lds_event_module(extent, true);
    physical.functions[0].id = source.functions[0].id.clone();
    physical.kernels[0].entry = source.kernels[0].entry.clone();
    let OperationKind::Constant(value) = &mut operations_mut(&mut physical)[1].kind else {
        panic!("preserving wrapper constant")
    };
    *value = Constant::U32(addend);
    let requirements = source.functions[0]
        .derived_capabilities()
        .into_iter()
        .chain(physical.functions[0].derived_capabilities())
        .collect::<BTreeSet<_>>();
    for module in [&mut source, &mut physical] {
        module.required_capabilities = requirements.clone();
        module.functions[0].required_capabilities = requirements.clone();
        module.kernels[0].required_capabilities = requirements.clone();
    }
    (source, physical)
}

fn initial_state() -> ExecutionStateV1 {
    ExecutionStateV1 {
        block: BlockId(0),
        values: BTreeMap::new(),
        memories: BTreeMap::new(),
        written_roots: BTreeSet::new(),
        events: Vec::new(),
        path: bool_true(),
        visits: BTreeMap::from([(BlockId(0), 1)]),
        next_private_allocation: 0,
        next_workgroup_allocation: 0,
    }
}

fn allocate(module: &Module, state: &mut ExecutionStateV1) {
    for operation in &operations(module)[..3] {
        execute_operation(
            operation,
            state,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut BTreeMap::new(),
            &BTreeMap::new(),
            module.kernels[0].workgroup_size,
            &BTreeMap::new(),
        )
        .unwrap();
    }
}

fn malformed(module: &Module) {
    assert!(matches!(
        Transfers::collect(&module.functions[0], &mut 0),
        Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
    ));
}

#[test]
fn reusable_lds_exact_owned_and_borrowed_transfers_preserve_root_memory_and_events() {
    for borrowed in [false, true] {
        let module = logical_module(borrowed, None);
        VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let mut nodes = 0;
        let transfers = Transfers::collect(&module.functions[0], &mut nodes).unwrap();
        let mut state = initial_state();
        allocate(&module, &mut state);
        let Some(SymbolicValueV1::WorkgroupView(before)) = state.values.get(&ValueId(102)).cloned()
        else {
            panic!("real allocator view")
        };
        let memories = format!("{:?}", state.memories);
        let events = format!("{:?}", state.events);
        let written = state.written_roots.clone();
        let path = format!("{:?}", state.path);
        let visits = state.visits.clone();
        let counters = (
            state.next_private_allocation,
            state.next_workgroup_allocation,
        );
        transfers
            .execute(&operations(&module)[3], &mut state, &mut nodes)
            .unwrap();
        let Some(SymbolicValueV1::ReusableLds(after)) = state.values.get(&ValueId(103)) else {
            panic!("dormant reusable handle")
        };
        assert_eq!(after.view.memory_root, before.memory_root);
        assert_eq!(after.view.layout, before.layout);
        assert_eq!(after.view.elements, before.elements);
        assert!(!after.view.initialized && !after.view.published);
        assert!(value_matches_type(
            &SymbolicValueV1::ReusableLds(after.clone()),
            &operations(&module)[3].results[0].ty
        ));
        assert!(!value_matches_type(
            &SymbolicValueV1::ReusableLds(after.clone()),
            &operations(&module)[2].results[0].ty
        ));
        assert!(!value_matches_type(
            &SymbolicValueV1::Opaque,
            &operations(&module)[3].results[0].ty
        ));
        assert!(workgroup_view_value(&state.values, ValueId(103)).is_err());
        assert!(!state.values.contains_key(&ValueId(102)));
        assert_eq!(format!("{:?}", state.memories), memories);
        assert_eq!(format!("{:?}", state.events), events);
        assert_eq!(state.written_roots, written);
        assert_eq!(format!("{:?}", state.path), path);
        assert_eq!(state.visits, visits);
        assert_eq!(
            (
                state.next_private_allocation,
                state.next_workgroup_allocation
            ),
            counters
        );
        let saved = format!("{state:?}");
        assert_eq!(
            transfers.execute(&operations(&module)[3], &mut state, &mut nodes),
            Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
        );
        assert_eq!(format!("{state:?}"), saved);
    }
}

#[test]
fn reusable_lds_real_observable_model_matches_one_physical_allocation() {
    let (source, physical) = pair(0, 16);
    let source_owner = VerifiedCanonicalKernelIrV13::from_module(source.clone()).unwrap();
    let physical_owner = VerifiedCanonicalKernelIrV13::from_module(physical.clone()).unwrap();
    assert_ne!(source_owner.identity(), physical_owner.identity());
    let left = extract_kernel_effects(
        &source.functions[0],
        &BTreeMap::new(),
        source.kernels[0].workgroup_size,
    )
    .unwrap();
    let right = extract_kernel_effects(
        &physical.functions[0],
        &BTreeMap::new(),
        physical.kernels[0].workgroup_size,
    )
    .unwrap();
    assert_eq!(left.output_roots, BTreeSet::from([1]));
    assert_eq!(right.output_roots, left.output_roots);
    assert_eq!(left.outcomes.len(), 1);
    assert_eq!(right.outcomes.len(), 1);
    assert_eq!(left.outcomes[0].events.len(), 1);
    assert_eq!(
        format!("{:?}", left.outcomes[0].events),
        format!("{:?}", right.outcomes[0].events)
    );
    assert_eq!(left.outcomes[0].memories[&1].writes.len(), 1);
    assert_eq!(right.outcomes[0].memories[&1].writes.len(), 1);
    let proof = generate_final_kir_output_equivalence_v1(&source, &physical).unwrap();
    assert_eq!(proof.output_writes, 1);
    assert_eq!(
        proof.numerical_model,
        FinalKirNumericalModelV1::ExactBitVector
    );
}

#[test]
fn pinned_verus_reusable_lds_transfer_preserves_output_and_rejects_value_or_extent_change() {
    let verus = pinned_rust_verify();
    assert!(
        verus.is_file(),
        "pinned Verus absent at {}",
        verus.display()
    );
    for (addend, extent, accepted, suffix) in [
        (0, 16, true, "reusable-lds-preserved"),
        (1, 16, false, "reusable-lds-value"),
        (0, 17, false, "reusable-lds-extent"),
    ] {
        let (source, physical) = pair(addend, extent);
        VerifiedCanonicalKernelIrV13::from_module(source.clone()).unwrap();
        VerifiedCanonicalKernelIrV13::from_module(physical.clone()).unwrap();
        let proof = generate_final_kir_output_equivalence_v1(&source, &physical).unwrap();
        let output = run_verus(&verus, proof.into_parts().0.source(), suffix);
        if accepted {
            assert_verus_accepts(output, suffix);
        } else {
            assert_verus_rejects(output, suffix);
        }
    }
}

#[test]
fn reusable_lds_missing_output_still_rejects() {
    let mut module = logical_module(true, None);
    operations_mut(&mut module).truncate(4);
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    assert_eq!(
        generate_final_kir_output_equivalence_v1(&module, &module).unwrap_err(),
        FinalKirOutputEquivalenceErrorV1::MissingOutputWrite
    );
}

#[test]
fn reusable_lds_immutable_contract_and_source_coordinate_substitutions_reject() {
    for change in 0..17 {
        let mut module = logical_module(true, None);
        let contract = contract_mut(&mut module, 3);
        let ExecutionCapabilityOperationV1::ReusableLdsConversion(value) = &mut contract.operation
        else {
            unreachable!()
        };
        match change {
            0 => value.input = execution_identity(0xff),
            1 => value.output = execution_identity(0xff),
            2 => value.element = execution_identity(0xff),
            3 => value.layout.byte_size = 8,
            4 => value.elements = 17,
            5 => contract.workgroup_brand = Some([0xff; 32]),
            6 => contract.epoch_before = Some([0xff; 32]),
            7 => contract.provenance.issuance = [0xff; 32],
            8 => {
                contract.source.occurrence =
                    ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                        [0xff; 32], [0xa2; 32], [0xa3; 32], 3, 0,
                    )
            }
            9 => contract.source.operation = [0xff; 32],
            10 => value.defined_abi = [0; 32],
            11 => value.defined_body = [0; 32],
            12 => value.source_binding = [0; 32],
            13 => contract.epoch_after = Some([0xff; 32]),
            14 => contract.source.occurrence = None,
            15 | 16 => {
                let index = if change == 15 { 3 } else { 2 };
                let Type::ExecutionCapability(ty) =
                    &mut operations_mut(&mut module)[index].results[0].ty
                else {
                    unreachable!()
                };
                ty.epoch = Some([0xff; 32]);
            }
            _ => unreachable!(),
        }
        malformed(&module);
    }
}

#[test]
fn reusable_lds_extra_uses_copies_and_unreachable_terminator_aliases_reject() {
    let mut module = logical_module(true, None);
    let mut duplicate = operations(&module)[3].clone();
    duplicate.results[0].id = ValueId(104);
    operations_mut(&mut module).insert(4, duplicate);
    malformed(&module);
    assert!(VerifiedCanonicalKernelIrV13::from_module(module).is_err());
    let mut module = logical_module(true, None);
    operations_mut(&mut module).push(Operation::effect_free(
        ValueDef::new(ValueId(104), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::Equal,
            lhs: ValueId(102),
            rhs: ValueId(102),
        },
    ));
    malformed(&module);
    let mut module = logical_module(true, None);
    let mut unreachable = BasicBlock::new(BlockId(1));
    unreachable.terminator = Some(Terminator::Return {
        values: vec![ValueId(102)],
    });
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(unreachable);
    malformed(&module);
}

#[test]
fn reusable_lds_dynamic_mutations_and_foreign_operation_leave_state_unchanged() {
    let module = logical_module(true, None);
    let transfers = Transfers::collect(&module.functions[0], &mut 0).unwrap();
    for change in 0..15 {
        let mut state = initial_state();
        allocate(&module, &mut state);
        let Some(SymbolicValueV1::WorkgroupView(view)) = state.values.get_mut(&ValueId(102)) else {
            unreachable!()
        };
        let root = view.memory_root;
        match change {
            0 => view.initialized = true,
            1 => view.published = true,
            2 => view.elements += 1,
            3 => view.layout.byte_alignment = 4,
            4 => view.memory_root += 1,
            5 => {
                state.memories.remove(&root);
            }
            6 => state.memories.get_mut(&root).unwrap().initial_parameter = Some(0),
            7 => state.memories.get_mut(&root).unwrap().bounded_elements = Some(17),
            8 => {
                state.written_roots.insert(root);
            }
            9 => {
                state.values.insert(ValueId(103), SymbolicValueV1::Opaque);
            }
            10 => state.memories.get_mut(&root).unwrap().element = Some(ScalarV1::Unsigned(32)),
            11 => state.memories.get_mut(&root).unwrap().cross_lane_width = Some(0),
            12 => state
                .memories
                .get_mut(&root)
                .unwrap()
                .writes
                .push(StoreEffectV1 {
                    index: constant_for_scalar(ScalarV1::Unsigned(64), 0).unwrap(),
                    guard: bool_true(),
                    value: constant_for_scalar(ScalarV1::Unsigned(32), 0).unwrap(),
                }),
            13 => state.next_workgroup_allocation = 0,
            14 => state.block = BlockId(1),
            _ => unreachable!(),
        }
        let saved = format!("{state:?}");
        assert_eq!(
            transfers.execute(&operations(&module)[3], &mut state, &mut 0),
            Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
        );
        assert_eq!(format!("{state:?}"), saved);
    }
    let mut state = initial_state();
    allocate(&module, &mut state);
    let saved = format!("{state:?}");
    let copied = operations(&module)[3].clone();
    assert_eq!(
        transfers.execute(&copied, &mut state, &mut 0),
        Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
    );
    assert_eq!(format!("{state:?}"), saved);
}

#[test]
fn reusable_lds_preparation_work_is_retained_and_allocation_reentry_gets_fresh_root() {
    let module = logical_module(true, None);
    let mut nodes = 0;
    let transfers = Transfers::collect(&module.functions[0], &mut nodes).unwrap();
    assert!(nodes > operations(&module).len());
    let mut exhausted = MAX_FINAL_KIR_SYMBOLIC_NODES_V1;
    assert!(matches!(
        Transfers::collect(&module.functions[0], &mut exhausted),
        Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit)
    ));
    let mut state = initial_state();
    allocate(&module, &mut state);
    let saved = format!("{state:?}");
    let mut exhausted = MAX_FINAL_KIR_SYMBOLIC_NODES_V1;
    assert_eq!(
        transfers.execute(&operations(&module)[3], &mut state, &mut exhausted),
        Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit)
    );
    assert_eq!(format!("{state:?}"), saved);
    transfers
        .execute(&operations(&module)[3], &mut state, &mut nodes)
        .unwrap();
    let blocks = module.functions[0]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect();
    let mut pending = VecDeque::new();
    enqueue_successor(state, BlockId(0), &[], &blocks, &mut pending, &mut 1).unwrap();
    let mut state = pending.pop_front().unwrap();
    allocate(&module, &mut state);
    transfers
        .execute(&operations(&module)[3], &mut state, &mut nodes)
        .unwrap();
    let Some(SymbolicValueV1::ReusableLds(handle)) = state.values.get(&ValueId(103)) else {
        unreachable!()
    };
    assert_eq!(handle.view.memory_root, WORKGROUP_MEMORY_ROOT_BASE_V1 + 1);
    assert_eq!(state.memories.len(), 2);
    assert_eq!(state.events.len(), 2);
    assert_eq!(state.next_workgroup_allocation, 2);
}

#[test]
fn reusable_lds_parameters_and_phi_handles_are_not_allocation_owners() {
    let mut module = logical_module(true, None);
    let allocated = operations_mut(&mut module).remove(2).results.remove(0);
    module.functions[0]
        .signature
        .parameters
        .push(allocated.ty.clone());
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .parameters
        .push(allocated.id);
    malformed(&module);
    assert!(VerifiedCanonicalKernelIrV13::from_module(module).is_err());

    let mut module = logical_module(true, None);
    let ty = operations(&module)[2].results[0].ty.clone();
    let mut tail = operations_mut(&mut module).split_off(3);
    let OperationKind::ExecutionCapability(conversion) = &mut tail[0].kind else {
        unreachable!()
    };
    conversion.operands[0] = ValueId(104);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(102)],
    });
    let mut joined = BasicBlock::new(BlockId(1));
    joined.parameters.push(ValueDef::new(ValueId(104), ty));
    joined.operations = tail;
    joined.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.push(joined);
    malformed(&module);
    assert!(VerifiedCanonicalKernelIrV13::from_module(module).is_err());
}

#[test]
fn reusable_lds_skipped_allocator_and_repeated_loop_consumption_reject() {
    let mut skipped = logical_module(true, None);
    let tail = operations_mut(&mut skipped).split_off(3);
    let allocation = operations_mut(&mut skipped).pop().unwrap();
    let body = skipped.functions[0].body.as_mut().unwrap();
    body.blocks[0].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(200), Type::BOOL),
        OperationKind::Constant(Constant::Bool(false)),
    ));
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(200),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut issuer = BasicBlock::new(BlockId(1));
    issuer.operations.push(allocation);
    issuer.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut consumer = BasicBlock::new(BlockId(2));
    consumer.operations = tail;
    consumer.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([issuer, consumer]);
    assert!(VerifiedCanonicalKernelIrV13::from_module(skipped.clone()).is_err());
    assert!(matches!(
        extract_kernel_effects(
            &skipped.functions[0],
            &BTreeMap::new(),
            skipped.kernels[0].workgroup_size
        ),
        Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
    ));

    let mut repeated = logical_module(true, None);
    let tail = operations_mut(&mut repeated).split_off(3);
    let body = repeated.functions[0].body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut consumer = BasicBlock::new(BlockId(1));
    consumer.operations = tail;
    consumer.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    body.blocks.push(consumer);
    assert!(matches!(
        extract_kernel_effects(
            &repeated.functions[0],
            &BTreeMap::new(),
            repeated.kernels[0].workgroup_size
        ),
        Err(FinalKirOutputEquivalenceErrorV1::MalformedValueFlow)
    ));
}

#[test]
fn reusable_lds_operand_census_has_no_fresh_budget_or_unbounded_vec_clone() {
    let mut module = logical_module(true, None);
    operations_mut(&mut module).push(Operation::effect_free(
        ValueDef::new(ValueId(200), Type::Scalar(ScalarType::U32)),
        OperationKind::Call {
            callee: fe2o3_kernel_ir::FunctionId::new("unmodeled"),
            arguments: vec![ValueId(0); MAX_FINAL_KIR_SYMBOLIC_NODES_V1],
        },
    ));
    assert!(matches!(
        Transfers::collect(&module.functions[0], &mut 0),
        Err(FinalKirOutputEquivalenceErrorV1::ResourceLimit)
    ));
}

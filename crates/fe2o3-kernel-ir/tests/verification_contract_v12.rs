use fe2o3_kernel_ir::*;

fn events() -> [WorkgroupPipelineEventKindV12; 6] {
    use WorkgroupPipelineEventKindV12::*;
    [Stage, Commit, Wait, Consume, Discard, Release]
}

fn marker(kind: WorkgroupPipelineEventKindV12, key: u32) -> Operation {
    Operation::new(
        vec![],
        OperationKind::VerificationContract(
            VerificationContractOperationV12::WorkgroupPipelineEvent {
                contract: VerificationContractKeyV12::new(key),
                kind,
                storage: ValueId(3),
                epoch: ValueId(8),
            },
        ),
    )
}

fn module() -> Module {
    let mut block = BasicBlock::new(BlockId(9));
    block.operations = events().into_iter().map(|kind| marker(kind, 17)).collect();
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("verification-contract-v12");
    module.functions.push(Function::internal_helper(
        "events",
        Signature::new(
            vec![
                Type::pointer(Type::F32, AddressSpace::Workgroup, AccessMode::ReadWrite),
                Type::INDEX,
            ],
            vec![],
        ),
        vec![ValueId(3), ValueId(8)],
        vec![block],
    ));
    module
}

#[test]
fn all_closed_events_roundtrip_and_have_no_runtime_effects() {
    let module = module();
    verify_module(&module).unwrap();
    let bytes = encode_module_v12(&module).unwrap();
    assert_eq!(decode_module_v12(&bytes).unwrap(), module);
    assert_eq!(
        VerifiedCanonicalKernelIrV12::from_module(module.clone())
            .unwrap()
            .canonical_bytes(),
        bytes
    );
    for operation in &module.functions[0].body.as_ref().unwrap().blocks[0].operations {
        assert_eq!(operation.operands(), [ValueId(3), ValueId(8)]);
        assert!(operation.memory_effects().is_empty());
        assert!(operation.effect_summary().is_pure());
        assert!(!operation.combined_effect_summary_v12().is_pure());
        assert!(
            operation
                .compiler_ordering_effects_v12()
                .has_ordered_verification_contract()
        );
        assert!(operation.required_capabilities().is_empty());
    }
}

#[test]
fn every_frozen_encoder_rejects_the_new_operation() {
    type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
    for encode in [
        encode_module_v1 as Encoder,
        encode_module_v2,
        encode_module_v3,
        encode_module_v4,
        encode_module_v5,
        encode_module_v6,
        encode_module_v7,
        encode_module_v8,
        encode_module_v9,
        encode_module_v10,
        encode_module_v11,
    ] {
        assert!(encode(&module()).is_err());
    }
}

#[test]
fn wire_tags_and_payload_have_exact_closed_bytes() {
    let bytes = encode_module_v12(&module()).unwrap();
    for (index, _) in events().iter().enumerate() {
        let mut expected = vec![30, 1];
        expected.extend_from_slice(&17_u32.to_le_bytes());
        expected.push(index as u8 + 1);
        expected.extend_from_slice(&3_u32.to_le_bytes());
        expected.extend_from_slice(&8_u32.to_le_bytes());
        assert_eq!(expected.len(), 15);
        assert_eq!(
            bytes
                .windows(expected.len())
                .filter(|window| *window == expected)
                .count(),
            1
        );
    }
}

#[test]
fn unknown_family_and_event_tags_fail_closed() {
    let bytes = encode_module_v12(&module()).unwrap();
    let offset = bytes
        .windows(7)
        .position(|window| window == [30, 1, 17, 0, 0, 0, 1])
        .unwrap();
    for (field, value) in [(1, 0), (1, 2), (6, 0), (6, 7), (6, 255)] {
        let mut hostile = bytes.clone();
        hostile[offset + field] = value;
        assert!(matches!(
            decode_module_v12(&hostile),
            Err(KernelIrDecodeError::UnknownTag { .. })
        ));
    }
    let mut old_header = bytes;
    old_header[8..10].copy_from_slice(&11_u16.to_le_bytes());
    assert!(decode_module_v11(&old_header).is_err());
}

#[test]
fn key_is_inert_and_both_key_and_epoch_are_canonically_bound() {
    let first = module();
    let mut second = first.clone();
    second.functions[0].body.as_mut().unwrap().blocks[0].operations[0] =
        marker(events()[0], u32::MAX);
    verify_module(&second).expect("structural checking cannot authenticate catalog keys");
    assert_ne!(
        encode_module_v12(&first).unwrap(),
        encode_module_v12(&second).unwrap()
    );
    assert_eq!(VerificationContractKeyV12::new(u32::MAX).index(), u32::MAX);
}

#[test]
fn results_wrong_storage_and_wrong_epoch_types_are_rejected() {
    let mut with_result = module();
    with_result.functions[0].body.as_mut().unwrap().blocks[0].operations[0]
        .results
        .push(ValueDef::new(ValueId(20), Type::INDEX));
    assert!(verify_module(&with_result).is_err());
    let mut wrong_space = module();
    wrong_space.functions[0].signature.parameters[0] =
        Type::pointer(Type::F32, AddressSpace::Global, AccessMode::ReadWrite);
    assert!(verify_module(&wrong_space).is_err());
    let mut wrong_epoch = module();
    wrong_epoch.functions[0].signature.parameters[1] = Type::Scalar(ScalarType::U64);
    assert!(verify_module(&wrong_epoch).is_err());
}

#[test]
fn undefined_and_nondominating_marker_operands_are_not_erased_metadata() {
    let mut input = module();
    let body = input.functions[0].body.as_mut().unwrap();
    body.parameters[1] = ValueId(7);
    assert!(verify_module(&input).is_err());
    let mut input = module();
    input.functions[0].signature.parameters.pop();
    let body = input.functions[0].body.as_mut().unwrap();
    body.parameters.pop();
    body.blocks[0].operations.push(Operation::new(
        vec![ValueDef::new(ValueId(8), Type::INDEX)],
        OperationKind::Constant(Constant::Index(0)),
    ));
    assert!(verify_module(&input).is_err());
}

#[test]
fn operand_visitation_is_ordered_and_stops_on_rejection() {
    let operation = marker(events()[0], 0);
    assert_eq!(operation.kind.operand_count(), 2);
    let mut visited = vec![];
    let result = operation.kind.try_visit_operands(|value| {
        visited.push(value);
        Err::<(), _>("stop")
    });
    assert_eq!(result, Err("stop"));
    assert_eq!(visited, [ValueId(3)]);
}

#[test]
fn compiler_order_propagates_through_two_call_levels_without_fake_memory() {
    let mut input = module();
    for (name, callee) in [("middle", "events"), ("outer", "middle")] {
        let mut block = BasicBlock::new(BlockId(0));
        block.operations.push(Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new(callee),
                arguments: vec![ValueId(3), ValueId(8)],
            },
        ));
        block.terminator = Some(Terminator::Return { values: vec![] });
        input.functions.push(Function::internal_helper(
            name,
            input.functions[0].signature.clone(),
            vec![ValueId(3), ValueId(8)],
            vec![block],
        ));
    }
    let effects = analyze_interprocedural_effects_v1(&input).unwrap();
    for name in ["events", "middle", "outer"] {
        let decision = effects.function(&FunctionId::new(name)).unwrap();
        assert!(decision.is_complete());
        assert!(!decision.is_complete_and_pure());
        assert!(decision.summary().effects().is_empty());
        assert!(decision.summary().memory().is_pure());
        assert!(
            decision
                .summary()
                .compiler_ordering()
                .has_ordered_verification_contract()
        );
    }
}

#[test]
fn marker_wire_extent_and_canonical_work_accept_exact_limit_only() {
    let input = module();
    let mut count = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let extent = count_module_v12_wire_extent_with_work_v1(&input, &mut count).unwrap();
    assert_eq!(
        extent.wire_bytes(),
        encode_module_v12(&input).unwrap().len()
    );
    let required = count.work();
    assert!(
        count_module_v12_wire_extent_with_work_v1(
            &input,
            &mut CanonicalKernelIrWorkBudgetV1::new(required)
        )
        .is_ok()
    );
    assert!(
        count_module_v12_wire_extent_with_work_v1(
            &input,
            &mut CanonicalKernelIrWorkBudgetV1::new(required - 1)
        )
        .is_err()
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let owner =
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(input.clone(), &mut work)
            .unwrap();
    let exact = work.work();
    assert_eq!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(
            input.clone(),
            &mut CanonicalKernelIrWorkBudgetV1::new(exact)
        )
        .unwrap()
        .canonical_bytes(),
        owner.canonical_bytes()
    );
    assert!(
        VerifiedCanonicalKernelIrV12::from_module_with_work_budget_v1(
            input,
            &mut CanonicalKernelIrWorkBudgetV1::new(exact - 1)
        )
        .is_err()
    );
}

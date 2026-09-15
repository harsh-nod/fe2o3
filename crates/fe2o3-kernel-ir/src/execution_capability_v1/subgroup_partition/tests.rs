use crate::*;
include!("fixture.rs");

#[test]
fn partition_round_trip_keeps_tokens_scalars_lanes_and_obligations() {
    let module = module();
    verify_module(&module).unwrap();
    let bytes = encode_module_v13(&module).unwrap();
    assert_eq!(decode_module_v13(&bytes).unwrap(), module);
    assert!(decode_module_v12(&bytes).is_err());
    for (index, operands) in [
        (3, vec![ValueId(2), ValueId(1)]),
        (6, vec![ValueId(3), ValueId(4)]),
        (7, vec![ValueId(3), ValueId(6), ValueId(5)]),
    ] {
        let contract = operation_contract(&operations(&module)[index]);
        assert_eq!(contract.operands, operands);
        assert!(!contract.operation.is_kernel_scoped());
        assert!(!contract.operation.transitions_epoch());
        let encoded = encode_execution_capability_contract_v1(contract).unwrap();
        assert_eq!(&encoded[..2], &[2, 24]);
        assert_eq!(
            decode_execution_capability_contract_v1(&encoded, operands.clone()),
            Some(contract.clone())
        );
        for revision in [0, 1, 3, 255] {
            let mut bad = encoded.clone();
            bad[0] = revision;
            assert!(decode_execution_capability_contract_v1(&bad, operands.clone()).is_none());
        }
        for length in 0..encoded.len() {
            assert!(
                decode_execution_capability_contract_v1(&encoded[..length], operands.clone())
                    .is_none()
            );
        }
        for operands in [vec![], vec![ValueId(3)], vec![ValueId(3); 4]] {
            assert!(decode_execution_capability_contract_v1(&encoded, operands).is_none());
        }
        let mut forged = contract.clone();
        forged.obligations = ExecutionSafetyObligationsV1::from_bits(0);
        assert!(encode_execution_capability_contract_v1(&forged).is_none());
        let mut trailing = encoded;
        trailing.push(0);
        assert!(
            decode_execution_capability_contract_v1(&trailing, contract.operands.clone()).is_none()
        );
    }
    for index in [1, 2] {
        let contract = operation_contract(&operations(&module)[index]);
        assert_eq!(
            encode_execution_capability_contract_v1(contract).unwrap()[0],
            1
        );
    }
    let Type::ExecutionCapability(partition) = &operations(&module)[3].results[0].ty else {
        unreachable!()
    };
    let encoded = encode_execution_capability_type_v1(partition).unwrap();
    assert_eq!(encoded[0], 2);
    assert_eq!(
        decode_execution_capability_type_v1(&encoded),
        Some(partition.clone())
    );
    let mut old = encoded;
    old[0] = 1;
    assert!(decode_execution_capability_type_v1(&old).is_none());
}

#[test]
fn partition_widths_and_source_lane_are_bounded() {
    for (width, partition, accepted) in [
        (64, 16, true),
        (32, 16, true),
        (64, 1, true),
        (16, 16, false),
        (128, 16, false),
        (64, 0, false),
        (64, 3, false),
        (32, 64, false),
    ] {
        assert_eq!(
            valid_subgroup_partition_widths_v1(width, partition),
            accepted
        );
    }
    for (lane, accepted) in [
        (0, true),
        (15, true),
        (16, false),
        (63, false),
        (u32::MAX, false),
    ] {
        let mut module = module();
        operations_mut(&mut module)[5].kind = OperationKind::Constant(Constant::U32(lane));
        assert_eq!(verify_module(&module).is_ok(), accepted, "lane {lane}");
    }
    let mut masked = module();
    operations_mut(&mut masked).insert(
        6,
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(5),
                rhs: ValueId(5),
            },
        ),
    );
    contract_mut(&mut masked, 8).operands[2] = ValueId(8);
    verify_module(&masked).unwrap();
}

#[test]
fn partition_rejects_substituted_authority_and_epoch() {
    for mutate in [
        (|module: &mut Module| contract_mut(module, 3).operands.clear()) as fn(&mut Module),
        |module| contract_mut(module, 3).operands.swap(0, 1),
        |module| contract_mut(module, 6).operands[0] = ValueId(2),
        |module| contract_mut(module, 7).operands.swap(1, 2),
        |module| contract_mut(module, 3).epoch_before = Some([99; 32]),
        |module| contract_mut(module, 6).workgroup_brand = Some([99; 32]),
        |module| contract_mut(module, 7).provenance.issuance = [99; 32],
        |module| {
            let mut other = operations(module)[1].clone();
            other.results[0].id = ValueId(8);
            let OperationKind::ExecutionCapability(contract) = &mut other.kind else {
                unreachable!()
            };
            contract.source.operation = [99; 32];
            operations_mut(module).insert(3, other);
            contract_mut(module, 4).operands[1] = ValueId(8);
        },
    ] {
        let mut module = module();
        mutate(&mut module);
        assert!(verify_module(&module).is_err());
    }
}

#[test]
fn partition_rejects_use_after_dominating_epoch_transition() {
    let mut module = module();
    let mut barrier = contract(
        ExecutionCapabilityOperationV1::WorkgroupBarrier {
            input_workgroup: identity(11),
            output_workgroup: identity(19),
            semantics: ExecutionMemorySemanticsV1 {
                scope: ExecutionMemoryScopeV1::Workgroup,
                ordering: ExecutionMemoryOrderingV1::AcquireRelease,
                spaces: ExecutionMemorySpacesV1::Workgroup,
            },
        },
        &[11],
        19,
        &[1],
        50,
    );
    barrier.epoch_after = Some([9; 32]);
    let mut ty = capability(19, ExecutionCapabilityRoleV1::Workgroup);
    let Type::ExecutionCapability(capability) = &mut ty else {
        unreachable!()
    };
    capability.epoch = barrier.epoch_after;
    let requirements = barrier.operation.required_capabilities();
    module.required_capabilities.extend(requirements.clone());
    module.functions[0]
        .required_capabilities
        .extend(requirements.clone());
    module.kernels[0].required_capabilities.extend(requirements);
    operations_mut(&mut module).insert(
        6,
        Operation::effect_free(
            ValueDef::new(ValueId(8), ty),
            OperationKind::ExecutionCapability(barrier),
        ),
    );
    let errors = verify_module(&module).unwrap_err();
    assert!(
        errors
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("after a dominating transition"))
    );
}

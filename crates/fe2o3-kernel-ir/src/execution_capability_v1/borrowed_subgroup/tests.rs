use crate::*;
include!("../subgroup_partition/fixture.rs");

fn borrowed_module() -> Module {
    let mut module = module();
    let contract = contract_mut(&mut module, 2);
    contract.operation = ExecutionCapabilityOperationV1::SubgroupDeriveBorrowed {
        workgroup_reference: identity(20),
        workgroup: identity(11),
        subgroup: identity(12),
        width: 64,
    };
    contract.signature =
        ExecutionCapabilitySignatureV1::new(&[identity(20)], identity(12)).unwrap();
    contract.obligations = ExecutionSafetyObligationsV1::from_bits(
        required_execution_obligations_v1(&contract.operation),
    );
    let Type::ExecutionCapability(result) = &mut operations_mut(&mut module)[2].results[0].ty
    else {
        unreachable!()
    };
    result.role = ExecutionCapabilityRoleV1::BorrowedSubgroup {
        workgroup_reference: identity(20),
        workgroup: identity(11),
        width: 64,
    };
    module
}

#[test]
fn shared_owner_survives_partition_and_codec_without_reference_erasure() {
    let module = borrowed_module();
    verify_module(&module).unwrap();
    assert_eq!(
        decode_module_v13(&encode_module_v13(&module).unwrap()).unwrap(),
        module
    );
    let contract = operation_contract(&operations(&module)[2]);
    assert_eq!(
        contract.signature.arguments().collect::<Vec<_>>(),
        [identity(20)]
    );
    assert_eq!(contract.operands, [ValueId(1)]);
    let bytes = encode_execution_capability_contract_v1(contract).unwrap();
    assert_eq!(&bytes[..2], &[3, 25]);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
        Some(contract.clone())
    );
    for revision in [1, 2, 4, 5] {
        let mut bad = bytes.clone();
        bad[0] = revision;
        assert!(decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none());
    }
    for len in 0..bytes.len() {
        assert!(
            decode_execution_capability_contract_v1(&bytes[..len], contract.operands.clone())
                .is_none()
        );
    }
    let Type::ExecutionCapability(role) = &operations(&module)[2].results[0].ty else {
        unreachable!()
    };
    let bytes = encode_execution_capability_type_v1(role).unwrap();
    assert_eq!(bytes[0], 3);
    assert_eq!(
        decode_execution_capability_type_v1(&bytes),
        Some(role.clone())
    );
    for revision in [1, 2, 4, 5] {
        let mut bad = bytes.clone();
        bad[0] = revision;
        assert!(decode_execution_capability_type_v1(&bad).is_none());
    }
}

#[test]
fn legacy_contracts_and_roles_keep_their_existing_bytes() {
    let original = module();
    let borrowed = borrowed_module();
    for index in [1, 3, 6, 7] {
        assert_eq!(
            encode_execution_capability_contract_v1(operation_contract(
                &operations(&original)[index]
            )),
            encode_execution_capability_contract_v1(operation_contract(
                &operations(&borrowed)[index]
            ))
        );
    }
    assert_eq!(
        encode_execution_capability_contract_v1(operation_contract(&operations(&original)[2]))
            .unwrap()[0],
        1
    );
}

#[test]
fn different_live_same_typed_owner_is_not_the_partition_epoch_owner() {
    for change_borrow in [false, true] {
        let mut module = borrowed_module();
        let mut other = operations(&module)[1].clone();
        other.results[0].id = ValueId(90);
        let OperationKind::ExecutionCapability(issuer) = &mut other.kind else {
            unreachable!()
        };
        issuer.source.operation = [90; 32];
        operations_mut(&mut module).insert(2, other);
        if change_borrow {
            contract_mut(&mut module, 3).operands[0] = ValueId(90);
        } else {
            contract_mut(&mut module, 4).operands[1] = ValueId(90);
        }
        assert!(verify_module(&module).is_err());
    }
}

#[test]
fn missing_shared_role_lifetime_obligation_or_nominal_reference_rejects() {
    for mutate in [
        (|module: &mut Module| {
            let contract = contract_mut(module, 2);
            contract.obligations = ExecutionSafetyObligationsV1::from_bits(
                contract.obligations.bits() & !ExecutionSafetyObligationsV1::LIFETIME_VALIDITY,
            );
        }) as fn(&mut Module),
        |module| {
            contract_mut(module, 2).signature =
                ExecutionCapabilitySignatureV1::new(&[identity(11)], identity(12)).unwrap()
        },
        |module| contract_mut(module, 2).operands.clear(),
        |module| contract_mut(module, 2).epoch_before = Some([99; 32]),
        |module| {
            let Type::ExecutionCapability(role) = &mut operations_mut(module)[2].results[0].ty
            else {
                unreachable!()
            };
            role.role = ExecutionCapabilityRoleV1::Subgroup { width: 64 };
        },
    ] {
        let mut module = borrowed_module();
        mutate(&mut module);
        assert!(verify_module(&module).is_err());
    }
}

#[test]
fn expanded_borrow_keeps_codec5_source_fields_and_rejects_other_root_custody() {
    let mut module = borrowed_module();
    for index in [1, 2, 3, 6, 7] {
        contract_mut(&mut module, index).source.occurrence = Some(
            ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                [70; 32],
                [71; 32],
                [72; 32],
                0,
                index as u32,
            )
            .unwrap(),
        );
    }
    verify_module(&module).unwrap();
    let contract = operation_contract(&operations(&module)[2]);
    let bytes = encode_execution_capability_contract_v1(contract).unwrap();
    assert_eq!(&bytes[..2], &[5, 25]);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
        Some(contract.clone())
    );
    contract_mut(&mut module, 2).source.occurrence = Some(
        ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
            [70; 32], [99; 32], [72; 32], 0, 2,
        )
        .unwrap(),
    );
    assert!(verify_module(&module).is_err());
    contract_mut(&mut module, 2).source.occurrence = None;
    assert!(verify_module(&module).is_err());
}

#[test]
fn borrowed_subgroup_cannot_consume_a_dominated_stale_epoch() {
    let mut module = borrowed_module();
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
        2,
        Operation::effect_free(
            ValueDef::new(ValueId(90), ty),
            OperationKind::ExecutionCapability(barrier),
        ),
    );
    assert!(
        verify_module(&module)
            .unwrap_err()
            .diagnostics()
            .iter()
            .any(|d| d.message.contains("after a dominating transition"))
    );
}

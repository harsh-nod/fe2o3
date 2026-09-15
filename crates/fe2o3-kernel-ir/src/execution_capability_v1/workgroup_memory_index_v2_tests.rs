use crate::*;

#[path = "workgroup_memory_index_v2_tests/fixture.rs"]
mod fixture;
use fixture::*;

#[test]
fn scoped_index_issue_and_conversion_preserve_typed_contracts() {
    let module = module();
    verify_module(&module).unwrap();
    let bytes = encode_module_v13(&module).unwrap();
    assert_eq!(decode_module_v13(&bytes).unwrap(), module);
    for (index, tag) in [(2, 27), (3, 28)] {
        let contract = operation_contract(&operations(&module)[index]);
        let bytes = encode_execution_capability_contract_v1(contract).unwrap();
        assert_eq!(bytes[1], tag);
        assert_eq!(
            decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
            Some(contract.clone())
        );
        assert!(contract.operation.memory_effects().is_empty());
        assert!(!contract.operation.is_kernel_scoped());
        assert!(!contract.operation.transitions_epoch());
        for length in 0..bytes.len() {
            assert!(
                decode_execution_capability_contract_v1(
                    &bytes[..length],
                    contract.operands.clone()
                )
                .is_none()
            );
        }
        assert!(decode_execution_capability_contract_v1(&bytes, vec![]).is_none());
        assert!(
            decode_execution_capability_contract_v1(&bytes, vec![ValueId(2), ValueId(2)]).is_none()
        );
        let mut trailing = bytes;
        trailing.push(0);
        assert!(
            decode_execution_capability_contract_v1(&trailing, contract.operands.clone()).is_none()
        );
    }
    let issue = operation_contract(&operations(&module)[2]);
    assert_eq!(issue.signature.output(), identity(13));
    let Type::ExecutionCapability(payload) = &operations(&module)[2].results[0].ty else {
        unreachable!()
    };
    assert_eq!(payload.source_type, identity(14));
    let Type::ExecutionCapability(converted) = &operations(&module)[3].results[0].ty else {
        unreachable!()
    };
    assert_eq!(converted.source_type, identity(15));
    assert_eq!(payload.provenance, converted.provenance);
    assert_eq!(payload.workgroup_brand, converted.workgroup_brand);
    assert_eq!(payload.epoch, converted.epoch);
}

#[test]
fn scoped_index_conversion_rejects_operand_scope_and_result_substitutions() {
    for mutate in [
        (|m: &mut Module| contract_mut(m, 3).operands[0] = ValueId(1)) as fn(&mut Module),
        |m| contract_mut(m, 3).operands[0] = ValueId(0),
        |m| contract_mut(m, 3).epoch_before = Some([99; 32]),
        |m| contract_mut(m, 3).epoch_after = Some([99; 32]),
        |m| contract_mut(m, 3).workgroup_brand = Some([99; 32]),
        |m| contract_mut(m, 3).provenance.issuance = [99; 32],
        |m| contract_mut(m, 3).provenance.target_brand = [99; 32],
        |m| contract_mut(m, 3).provenance.launch_brand = [99; 32],
        |m| contract_mut(m, 3).provenance.root = FunctionId::new("other_root"),
        |m| {
            contract_mut(m, 3).signature =
                ExecutionCapabilitySignatureV1::new(&[identity(13)], identity(15)).unwrap()
        },
        |m| contract_mut(m, 3).obligations = ExecutionSafetyObligationsV1::from_bits(0),
        |m| {
            operations_mut(m)[3].results[0].ty =
                capability(14, ExecutionCapabilityRoleV1::WorkgroupMemoryIndex)
        },
        |m| operations_mut(m)[3].results[0].ty = Type::INDEX,
        |m| {
            operations_mut(m)[2].results[0].ty =
                capability(13, ExecutionCapabilityRoleV1::WorkgroupMemoryIndex)
        },
        |m| {
            contract_mut(m, 2).signature =
                ExecutionCapabilitySignatureV1::new(&[identity(12)], identity(14)).unwrap()
        },
        |m| contract_mut(m, 2).operands[0] = ValueId(0),
    ] {
        let mut bad = module();
        mutate(&mut bad);
        assert!(verify_module(&bad).is_err(), "reject {bad:?}");
    }
}

#[test]
fn scoped_index_constructor_rejects_identity_collapse_and_missing_scope() {
    for operation in [
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
            workgroup_reference: identity(12),
            workgroup: identity(11),
            option: identity(14),
            witness: identity(14),
        },
        ExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint {
            input_witness: identity(14),
            output_witness: identity(14),
        },
    ] {
        assert!(!operation.is_well_formed());
    }
    for index in [2, 3] {
        let mut module = module();
        let contract = contract_mut(&mut module, index);
        contract.workgroup_brand = None;
        contract.epoch_before = None;
        assert!(!contract.is_complete());
    }
}

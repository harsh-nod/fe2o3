use fe2o3_kernel_ir::*;

#[path = "support/numerical_policy_v13.rs"]
mod fixture;
use fixture::*;

#[test]
fn numerical_policy_is_a_typed_canonical_operation_with_unresolved_obligations() {
    let module = module();
    verify_module(&module).unwrap();
    let bytes = encode_module_v13(&module).unwrap();
    assert_eq!(decode_module_v13(&bytes).unwrap(), module);
    assert!(decode_module_v12(&bytes).is_err());
    let contract = contract();
    assert_eq!(
        contract.obligations.bits(),
        ExecutionSafetyObligationsV1::TARGET_SUPPORT
            | ExecutionSafetyObligationsV1::NUMERICAL_POLICY
    );
    assert_eq!(
        contract.signature.arguments().collect::<Vec<_>>(),
        [identity(10)]
    );
    assert_eq!(contract.signature.output(), identity(11));
    assert!(contract.operation.is_kernel_scoped());
    assert!(!contract.operation.transitions_epoch());
}

#[test]
fn numerical_policy_contract_revision_and_exact_policy_are_fail_closed() {
    let contract = contract();
    let bytes = encode_execution_capability_contract_v1(&contract).unwrap();
    assert_eq!(&bytes[..2], &[2, 23]);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
        Some(contract.clone())
    );
    for revision in [0, 1, 3, 255] {
        let mut bad = bytes.clone();
        bad[0] = revision;
        assert!(decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none());
    }
    for mode in [0, 2, 3, 255] {
        let mut bad = bytes.clone();
        bad[2 + 3 * 32] = mode;
        assert!(decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none());
    }
    for length in 0..bytes.len() {
        assert!(
            decode_execution_capability_contract_v1(&bytes[..length], contract.operands.clone())
                .is_none()
        );
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(
        decode_execution_capability_contract_v1(&trailing, contract.operands.clone()).is_none()
    );
    // Source function, operation and block occupy the final 68 bytes.
    let obligations = bytes.len() - 68 - 4;
    for bits in [0u32, 1, (1 << 16), (1 << 16) | 1 | (1 << 17)] {
        let mut bad = bytes.clone();
        bad[obligations..obligations + 4].copy_from_slice(&bits.to_le_bytes());
        assert!(decode_execution_capability_contract_v1(&bad, contract.operands.clone()).is_none());
    }
    for operands in [vec![], vec![ValueId(0), ValueId(0)]] {
        assert!(decode_execution_capability_contract_v1(&bytes, operands).is_none());
    }
}

#[test]
fn numerical_policy_type_revision_rejects_relaxed_missing_and_trailing_policy() {
    let authority = authority();
    let bytes = encode_execution_capability_type_v1(&authority).unwrap();
    assert_eq!(bytes[0], 2);
    assert_eq!(decode_execution_capability_type_v1(&bytes), Some(authority));
    for revision in [0, 1, 3, 255] {
        let mut bad = bytes.clone();
        bad[0] = revision;
        assert!(decode_execution_capability_type_v1(&bad).is_none());
    }
    for mode in [0, 2, 3, 255] {
        let mut bad = bytes.clone();
        *bad.last_mut().unwrap() = mode;
        assert!(decode_execution_capability_type_v1(&bad).is_none());
    }
    for length in 0..bytes.len() {
        assert!(decode_execution_capability_type_v1(&bytes[..length]).is_none());
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(decode_execution_capability_type_v1(&trailing).is_none());
}

#[test]
fn old_execution_records_keep_revision_one_and_cannot_be_upgraded_by_header_only() {
    let mut contract = contract();
    contract.operation = ExecutionCapabilityOperationV1::WorkgroupDerive {
        context: identity(10),
        workgroup: identity(11),
    };
    contract.workgroup_brand = Some([40; 32]);
    contract.epoch_before = Some([41; 32]);
    contract.obligations = ExecutionSafetyObligationsV1::from_bits(
        required_execution_obligations_v1(&contract.operation),
    );
    let mut bytes = encode_execution_capability_contract_v1(&contract).unwrap();
    assert_eq!(bytes[0], 1);
    // Frozen pre-policy V13 WorkgroupDerive payload, including u16 obligations.
    let golden_contract = concat!(
        "01000a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0b0b0b0b0b0b0b0b0b0b0b0b0b0b",
        "0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b010a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a",
        "0a0a0a0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0500656e747279010101010101",
        "010101010101010101010101010101010101010101010101010102020202020202020202020202020202020202020202",
        "020202020202020202020303030303030303030303030303030303030303030303030303030303030303040404040404",
        "040404040404040404040404040404040404040404040404040405050505050505050505050505050505050505050505",
        "050505050505050505050606060606060606060606060606060606060606060606060606060606060606012828282828",
        "282828282828282828282828282828282828282828282828282828012929292929292929292929292929292929292929",
        "292929292929292929292929000300141414141414141414141414141414141414141414141414141414141414141415",
        "1515151515151515151515151515151515151515151515151515151515151500000000",
    );
    let from_hex = |hex: &str| {
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>()
    };
    assert_eq!(bytes, from_hex(golden_contract));
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
        Some(contract.clone())
    );
    bytes[0] = 2;
    assert!(decode_execution_capability_contract_v1(&bytes, contract.operands).is_none());
    let mut authority = authority();
    authority.role = ExecutionCapabilityRoleV1::Workgroup;
    authority.workgroup_brand = Some([40; 32]);
    authority.epoch = Some([41; 32]);
    let mut bytes = encode_execution_capability_type_v1(&authority).unwrap();
    assert_eq!(bytes[0], 1);
    let golden_type = concat!(
        "010b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0500656e7472790101010101010101",
        "010101010101010101010101010101010101010101010101020202020202020202020202020202020202020202020202",
        "020202020202020203030303030303030303030303030303030303030303030303030303030303030404040404040404",
        "040404040404040404040404040404040404040404040404050505050505050505050505050505050505050505050505",
        "050505050505050506060606060606060606060606060606060606060606060606060606060606060128282828282828",
        "282828282828282828282828282828282828282828282828280129292929292929292929292929292929292929292929",
        "2929292929292929292901",
    );
    assert_eq!(bytes, from_hex(golden_type));
    bytes[0] = 2;
    assert!(decode_execution_capability_type_v1(&bytes).is_none());
}

#[test]
fn numerical_policy_rejects_forged_result_identity_role_and_all_provenance_axes() {
    let mutations: &[fn(&mut ExecutionCapabilityTypeV1)] = &[
        |ty| ty.source_type = identity(99),
        |ty| ty.role = ExecutionCapabilityRoleV1::KernelAuthority,
        |ty| {
            ty.role = ExecutionCapabilityRoleV1::NumericalPolicy {
                policy: identity(99),
                mode: NumericalModeV1::StrictIeee,
            }
        },
        |ty| ty.provenance.root = FunctionId::new("forged"),
        |ty| ty.provenance.kernel_binding = [99; 32],
        |ty| ty.provenance.frontend_unit = [99; 32],
        |ty| ty.provenance.kernel_marker = [99; 32],
        |ty| ty.provenance.target_brand = [99; 32],
        |ty| ty.provenance.launch_brand = [99; 32],
        |ty| ty.provenance.issuance = [99; 32],
        |ty| {
            ty.workgroup_brand = Some([40; 32]);
            ty.epoch = Some([41; 32]);
        },
    ];
    for mutate in mutations {
        let mut bad = module();
        mutate(authority_mut(&mut bad));
        assert!(
            verify_module(&bad).is_err(),
            "accepted forged authority: {bad:?}"
        );
    }
}

#[test]
fn numerical_policy_rejects_wrong_context_source_signature_arity_and_certification() {
    let mutations: &[fn(&mut ExecutionCapabilityOpV1)] = &[
        |op| op.operands.clear(),
        |op| op.operands.push(ValueId(0)),
        |op| op.operands[0] = ValueId(1),
        |op| {
            op.signature =
                ExecutionCapabilitySignatureV1::new(&[identity(99)], identity(11)).unwrap()
        },
        |op| {
            op.signature =
                ExecutionCapabilitySignatureV1::new(&[identity(10)], identity(99)).unwrap()
        },
        |op| op.source.function = [0; 32],
        |op| op.source.operation = [0; 32],
        |op| op.provenance.target_brand = [99; 32],
        |op| op.obligations = ExecutionSafetyObligationsV1::from_bits(1),
    ];
    for mutate in mutations {
        let mut bad = module();
        mutate(contract_mut(&mut bad));
        assert!(verify_module(&bad).is_err());
    }
    let mut forged = module();
    issuance_mut(&mut forged).kind = OperationKind::Constant(Constant::U32(0));
    assert!(verify_module(&forged).is_err());
    for count in [0, 2] {
        let mut bad = module();
        issuance_mut(&mut bad).results = (0..count)
            .map(|i| ValueDef::new(ValueId(1 + i), Type::ExecutionCapability(authority())))
            .collect();
        assert!(verify_module(&bad).is_err());
    }
}

#[test]
fn numerical_policy_encoding_rejects_zero_or_relaxed_policy_and_fabricated_obligations() {
    for policy in [identity(0), identity(12)] {
        for mode in [
            NumericalModeV1::StrictIeee,
            NumericalModeV1::AllowContraction,
            NumericalModeV1::AllowApproximation,
        ] {
            if policy == identity(12) && mode == NumericalModeV1::StrictIeee {
                continue;
            }
            let mut op = contract();
            op.operation = ExecutionCapabilityOperationV1::NumericalPolicyIssue {
                context: identity(10),
                capability: identity(11),
                policy,
                mode,
            };
            assert!(encode_execution_capability_contract_v1(&op).is_none());
            let mut ty = authority();
            ty.role = ExecutionCapabilityRoleV1::NumericalPolicy { policy, mode };
            assert!(encode_execution_capability_type_v1(&ty).is_none());
        }
    }
}

#[test]
fn numerical_policy_identity_cannot_alias_context_or_capability() {
    for policy in [identity(10), identity(11)] {
        let mut contract = contract();
        let ExecutionCapabilityOperationV1::NumericalPolicyIssue { policy: actual, .. } =
            &mut contract.operation
        else {
            unreachable!()
        };
        *actual = policy;
        assert!(!contract.is_complete());
        assert!(encode_execution_capability_contract_v1(&contract).is_none());
    }
    let mut authority = authority();
    authority.source_type = identity(12);
    assert!(!authority.is_complete());
}

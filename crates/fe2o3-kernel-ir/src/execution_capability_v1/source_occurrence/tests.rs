use super::*;
use crate::execution_capability_v1::{
    ContractWriter, encode_execution_operation, encode_execution_provenance,
};
use crate::*;

fn id(tag: u8) -> ExecutionTypeIdentityV1 {
    ExecutionTypeIdentityV1::new([tag; 32])
}

fn fixture(revision: u8) -> ExecutionCapabilityOpV1 {
    let (operation, output, workgroup_brand, epoch_before) = match revision {
        1 => (
            ExecutionCapabilityOperationV1::WorkgroupDerive {
                context: id(10),
                workgroup: id(11),
            },
            id(11),
            Some([7; 32]),
            Some([8; 32]),
        ),
        2 => (
            ExecutionCapabilityOperationV1::NumericalPolicyIssue {
                context: id(10),
                capability: id(11),
                policy: id(12),
                mode: NumericalModeV1::StrictIeee,
            },
            id(11),
            None,
            None,
        ),
        4 => (
            ExecutionCapabilityOperationV1::NumericalPolicyMath(
                NumericalPolicyMathOperationV1::MathDerive {
                    context: id(10),
                    binding: NumericalPolicyMathBindingV1 {
                        math_reference: id(13),
                        math: id(14),
                        policy_reference: id(15),
                        capability: id(11),
                        bound: id(16),
                        bound_reference: id(17),
                        policy: id(12),
                        kernel_brand: id(18),
                        mode: NumericalModeV1::StrictIeee,
                    },
                },
            ),
            id(14),
            None,
            None,
        ),
        _ => unreachable!(),
    };
    ExecutionCapabilityOpV1 {
        operands: vec![ValueId(0)],
        signature: ExecutionCapabilitySignatureV1::new(&[id(10)], output).unwrap(),
        provenance: ExecutionCapabilityProvenanceV1 {
            root: "entry".into(),
            kernel_binding: [1; 32],
            frontend_unit: [2; 32],
            kernel_marker: [3; 32],
            target_brand: [4; 32],
            launch_brand: [5; 32],
            issuance: [6; 32],
        },
        workgroup_brand,
        epoch_before,
        epoch_after: None,
        obligations: ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(
            &operation,
        )),
        source: ExecutionCapabilitySourceV1 {
            function: [20; 32],
            operation: [21; 32],
            block: 9,
            occurrence: None,
        },
        operation,
    }
}

// Frozen pre-occurrence layout: no option tag and no extra source bytes.
fn legacy_encoding(contract: &ExecutionCapabilityOpV1, revision: u8) -> Vec<u8> {
    let mut writer = ContractWriter::default();
    writer.u8(revision);
    encode_execution_operation(&mut writer, &contract.operation);
    let arguments = contract.signature.arguments().collect::<Vec<_>>();
    writer.u8(arguments.len() as u8);
    for argument in arguments {
        writer.identity(argument);
    }
    writer.identity(contract.signature.output());
    encode_execution_provenance(&mut writer, &contract.provenance).unwrap();
    writer.optional_digest(contract.workgroup_brand);
    writer.optional_digest(contract.epoch_before);
    writer.optional_digest(contract.epoch_after);
    if revision == 1 {
        writer.u16(contract.obligations.bits() as u16);
    } else {
        writer.u32(contract.obligations.bits());
    }
    writer.digest(contract.source.function);
    writer.digest(contract.source.operation);
    writer.u32(contract.source.block);
    writer.bytes
}

fn occurrence(instance: u32, block: u32) -> ExecutionCapabilitySourceOccurrenceV1 {
    ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
        [30; 32], [31; 32], [32; 32], instance, block,
    )
    .unwrap()
}

#[test]
fn none_preserves_legacy_revisions_and_exact_source_wire_bytes() {
    for revision in [1, 2, 4] {
        let contract = fixture(revision);
        let encoded = encode_execution_capability_contract_v1(&contract).unwrap();
        assert_eq!(encoded, legacy_encoding(&contract, revision));
        assert_eq!(encoded[0], revision);
        assert_eq!(
            decode_execution_capability_contract_v1(&encoded, contract.operands.clone()),
            Some(contract)
        );
    }
}

#[test]
fn occurrence_uses_revision_five_for_each_legacy_operation_family() {
    for revision in [1, 2, 4] {
        let mut contract = fixture(revision);
        contract.source.occurrence = Some(occurrence(0, 0));
        let bytes = encode_execution_capability_contract_v1(&contract).unwrap();
        assert_eq!(bytes[0], 5);
        assert_eq!(
            decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
            Some(contract)
        );
    }
}

#[test]
fn maximal_existing_payloads_with_occurrence_fit_the_unchanged_ceiling() {
    // RawMemoryBind maximizes the scoped legacy payload: four arguments,
    // signed extent checks, index-space identity, and both scope digests.
    let mut raw = fixture(1);
    raw.provenance.root = FunctionId::new("r".repeat(256));
    raw.operands = (0..MAX_EXECUTION_CAPABILITY_OPERANDS_V1 as u32)
        .map(ValueId)
        .collect();
    raw.signature =
        ExecutionCapabilitySignatureV1::new(&[id(40), id(41), id(42), id(46)], id(43)).unwrap();
    raw.operation = ExecutionCapabilityOperationV1::RawMemoryBind {
        authority: id(40),
        pointer: id(41),
        length: id(42),
        extent: ExecutionDynamicExtentV1 {
            operand: 2,
            source_argument: 2,
            source_type: id(42),
            value_type: ScalarType::I64,
            upper_bound: i64::MAX as u64,
            bound_check_operand: 30,
            nonnegative_check_operand: Some(31),
        },
        view: id(43),
        element: id(44),
        layout: ExecutionElementLayoutV1 {
            byte_size: 8,
            byte_alignment: 8,
        },
        space: ExecutionMemoryAddressSpaceV1::Workgroup,
        access: ExecutionMemoryAccessV1::DisjointWrite,
        index_space: Some(id(45)),
        atomic_scope: None,
        unsafe_obligation: id(46),
    };
    raw.obligations =
        ExecutionSafetyObligationsV1::from_bits(required_execution_obligations_v1(&raw.operation));
    let old = encode_execution_capability_contract_v1(&raw).unwrap();
    assert_eq!(old.len(), 1030);
    raw.source.occurrence = Some(occurrence(u32::MAX, u32::MAX));
    let bytes = encode_execution_capability_contract_v1(&raw).unwrap();
    assert_eq!(bytes.len(), old.len() + 104 + 2); // Revision 1 widens obligations to u32.
    assert_eq!(MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1, 2048);
    assert!(bytes.len() <= MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, raw.operands.clone()),
        Some(raw)
    );

    // Math has the largest operation-specific identity bundle. FMA fills all
    // four signature slots, but kernel scope omits the two scope digests.
    let mut math = fixture(4);
    let ExecutionCapabilityOperationV1::NumericalPolicyMath(operation) = math.operation else {
        unreachable!()
    };
    let binding = operation.binding();
    math.operation =
        ExecutionCapabilityOperationV1::NumericalPolicyMath(NumericalPolicyMathOperationV1::F32 {
            binding,
            bound_reference: binding.bound_reference,
            element: id(40),
            function: F32MathFunction::FusedMultiplyAdd,
        });
    math.signature = ExecutionCapabilitySignatureV1::new(
        &[binding.bound_reference, id(40), id(40), id(40)],
        id(40),
    )
    .unwrap();
    math.operands = vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)];
    math.provenance.root = FunctionId::new("m".repeat(256));
    let old = encode_execution_capability_contract_v1(&math).unwrap();
    math.source.occurrence = Some(occurrence(1, 1));
    let bytes = encode_execution_capability_contract_v1(&math).unwrap();
    assert_eq!(old.len(), 1011);
    assert_eq!(bytes.len(), old.len() + 104);
    assert!(bytes.len() <= MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, math.operands.clone()),
        Some(math.clone())
    );
    math.provenance.root = FunctionId::new("m".repeat(257));
    assert!(encode_execution_capability_contract_v1(&math).is_none());
    assert!(
        decode_execution_capability_contract_v1(
            &vec![5; MAX_EXECUTION_CAPABILITY_CONTRACT_BYTES_V1 + 1],
            vec![]
        )
        .is_none()
    );
}

#[test]
fn every_family_rejects_truncations_suffixes_and_nonminimal_revision() {
    for legacy in [1, 2, 4] {
        for expanded in [false, true] {
            let mut contract = fixture(legacy);
            contract.source.occurrence = expanded.then(|| occurrence(1, 2));
            let bytes = encode_execution_capability_contract_v1(&contract).unwrap();
            for end in 0..bytes.len() {
                assert!(
                    decode_execution_capability_contract_v1(
                        &bytes[..end],
                        contract.operands.clone()
                    )
                    .is_none()
                );
            }
            for byte in 0..=u8::MAX {
                let mut suffix = bytes.clone();
                suffix.push(byte);
                assert!(
                    decode_execution_capability_contract_v1(&suffix, contract.operands.clone())
                        .is_none()
                );
                if byte != bytes[0] {
                    let mut changed = bytes.clone();
                    changed[0] = byte;
                    assert!(
                        decode_execution_capability_contract_v1(
                            &changed,
                            contract.operands.clone()
                        )
                        .is_none()
                    );
                }
            }
        }
    }
}

#[test]
fn original_coordinates_remain_distinct_from_caller_occurrence() {
    let mut first = fixture(1);
    first.source.occurrence = Some(occurrence(2, 50));
    let mut second = first.clone();
    second.source.occurrence = Some(occurrence(3, 60));
    assert_eq!(first.source.function, second.source.function);
    assert_eq!(first.source.block, second.source.block);
    assert_ne!(
        encode_execution_capability_contract_v1(&first),
        encode_execution_capability_contract_v1(&second)
    );
    let encoded = encode_execution_capability_contract_v1(&first).unwrap();
    let decoded =
        decode_execution_capability_contract_v1(&encoded, first.operands.clone()).unwrap();
    assert_eq!(decoded.source.block, 9);
    assert_eq!(decoded.source.occurrence.unwrap().caller_instance(), 2);
    assert_eq!(decoded.source.occurrence.unwrap().expanded_block(), 50);
}

#[test]
fn revision_downgrade_truncation_trailing_bytes_and_empty_identity_reject() {
    for missing in 0..3 {
        let mut identities = [[30; 32], [31; 32], [32; 32]];
        identities[missing] = [0; 32];
        assert!(
            ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
                identities[0],
                identities[1],
                identities[2],
                0,
                0
            )
            .is_none()
        );
    }
    let mut contract = fixture(2);
    contract.source.occurrence = Some(occurrence(1, 4));
    let bytes = encode_execution_capability_contract_v1(&contract).unwrap();
    for end in 0..bytes.len() {
        assert!(
            decode_execution_capability_contract_v1(&bytes[..end], contract.operands.clone())
                .is_none()
        );
    }
    for revision in [0, 1, 2, 3, 4, 6] {
        let mut changed = bytes.clone();
        changed[0] = revision;
        assert!(
            decode_execution_capability_contract_v1(&changed, contract.operands.clone()).is_none()
        );
    }
    let mut changed = bytes.clone();
    changed.push(0);
    assert!(decode_execution_capability_contract_v1(&changed, contract.operands.clone()).is_none());
    let mut legacy = legacy_encoding(&fixture(2), 2);
    legacy[0] = 5;
    assert!(decode_execution_capability_contract_v1(&legacy, contract.operands.clone()).is_none());
    let mut changed = bytes;
    let start = changed.len() - 104;
    changed[start..start + 32].fill(0);
    assert!(decode_execution_capability_contract_v1(&changed, contract.operands.clone()).is_none());
}

use super::*;
use crate::*;

mod fixture {
    use super::*;
    include!("fixture.rs");
}

#[test]
fn ordered_max_keeps_exact_borrowed_partition_issuer_and_distinct_wire_operation() {
    let mut module = fixture::module();
    verify_module(&module).unwrap();
    let canonical = encode_module_v13(&module).unwrap();
    assert_eq!(decode_module_v13(&canonical).unwrap(), module);
    let contract = fixture::reduction_mut(&mut module);
    let bytes = encode_execution_capability_contract_v1(contract).unwrap();
    assert_eq!(&bytes[..3], &[2, 24, 3]);
    assert_eq!(
        decode_execution_capability_contract_v1(&bytes, contract.operands.clone()),
        Some(contract.clone())
    );
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
    let mut sum_tag = bytes;
    sum_tag[2] = 1;
    let sum = decode_execution_capability_contract_v1(&sum_tag, contract.operands.clone()).unwrap();
    assert!(matches!(
        sum.operation,
        ExecutionCapabilityOperationV1::SubgroupPartition(
            SubgroupPartitionOperationV1::ReduceSumF32 { .. }
        )
    ));
    assert_ne!(sum.operation, contract.operation);
    let before = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    *fixture::reduction_mut(&mut module) = sum;
    let after = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
    assert_ne!(before.identity(), after.identity());
}

#[test]
fn maximum_rejects_receiver_epoch_width_lifetime_convergence_and_scalar_mutations() {
    use ExecutionSafetyObligationsV1 as O;
    for mutation in 0..9 {
        let mut module = fixture::module();
        let reduction = fixture::reduction_mut(&mut module);
        match mutation {
            0 => reduction.operands[0] = ValueId(2),
            1 => reduction.epoch_before = Some([99; 32]),
            2 => reduction.workgroup_brand = Some([99; 32]),
            3 => {
                let ExecutionCapabilityOperationV1::SubgroupPartition(
                    SubgroupPartitionOperationV1::ReduceMaxF32 {
                        partition_width, ..
                    },
                ) = &mut reduction.operation
                else {
                    unreachable!()
                };
                *partition_width = 8;
            }
            4 => {
                reduction.obligations =
                    O::from_bits(reduction.obligations.bits() & !O::LIFETIME_VALIDITY)
            }
            5 => {
                reduction.obligations =
                    O::from_bits(reduction.obligations.bits() & !O::SUBGROUP_CONVERGENCE)
            }
            6 => {
                reduction.obligations =
                    O::from_bits(reduction.obligations.bits() & !O::EXACT_PARTICIPATION)
            }
            7 => reduction.operands[1] = ValueId(5),
            8 => reduction.operands.clear(),
            _ => unreachable!(),
        }
        assert!(
            verify_module(&module)
                .unwrap_err()
                .contains(DiagnosticCode::InvalidExecutionCapability),
            "mutation {mutation}"
        );
    }
}

#[test]
fn maximum_preserves_legacy_partition_bytes_borrowed_tag_and_occurrence_codec() {
    let original = fixture::legacy_module();
    let mut changed = fixture::module();
    let old = &original.functions[0].body.as_ref().unwrap().blocks[0].operations;
    let new = &changed.functions[0].body.as_ref().unwrap().blocks[0].operations;
    for index in [1, 3, 7] {
        let OperationKind::ExecutionCapability(a) = &old[index].kind else {
            unreachable!()
        };
        let OperationKind::ExecutionCapability(b) = &new[index].kind else {
            unreachable!()
        };
        assert_eq!(
            encode_execution_capability_contract_v1(a),
            encode_execution_capability_contract_v1(b)
        );
    }
    let OperationKind::ExecutionCapability(borrowed) = &new[2].kind else {
        unreachable!()
    };
    assert_eq!(
        &encode_execution_capability_contract_v1(borrowed).unwrap()[..2],
        &[3, 25]
    );
    let reduction = fixture::reduction_mut(&mut changed);
    reduction.source.occurrence = Some(
        ExecutionCapabilitySourceOccurrenceV1::from_untrusted_parts(
            [1; 32], [2; 32], [3; 32], 4, 5,
        )
        .unwrap(),
    );
    let encoded = encode_execution_capability_contract_v1(reduction).unwrap();
    assert_eq!(&encoded[..3], &[5, 24, 3]);
    assert_eq!(
        decode_execution_capability_contract_v1(&encoded, reduction.operands.clone()),
        Some(reduction.clone())
    );
}

#[test]
fn maximum_rejects_a_partition_rebound_to_another_same_typed_workgroup_issuer() {
    let mut module = fixture::module();
    verify_module(&module).unwrap();
    let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let mut foreign = operations[1].clone();
    foreign.results[0].id = ValueId(8);
    let OperationKind::ExecutionCapability(issuer) = &mut foreign.kind else {
        unreachable!()
    };
    // Negative-only mutation avoids merely exercising duplicate-source rejection.
    issuer.source.operation[0] ^= 0x80;
    operations.insert(2, foreign);
    let OperationKind::ExecutionCapability(partition) = &mut operations[4].kind else {
        unreachable!()
    };
    assert!(matches!(
        partition.operation,
        ExecutionCapabilityOperationV1::SubgroupPartition(
            SubgroupPartitionOperationV1::Derive { .. }
        )
    ));
    partition.operands[1] = ValueId(8);
    let error = verify_module(&module).unwrap_err();
    assert!(
        error
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.message.contains("partition receiver")),
        "exact receiver chain must reject: {error:?}"
    );
}

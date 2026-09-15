fn workgroup_index_operations_v22() -> [SemanticExecutionCapabilityOperationV1; 2] {
    let id = SemanticTypeIdV1::from_index;
    [
        SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
            workgroup_reference: id(1),
            workgroup: id(2),
            option: id(3),
            witness: id(4),
        },
        SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint {
            input_witness: id(4),
            output_witness: id(5),
        },
    ]
}

#[test]
fn workgroup_index_v22_round_trip_and_old_wire_rejection() {
    for (operation, tag) in workgroup_index_operations_v22().into_iter().zip([26, 27]) {
        let contract = execution_contract(operation).unwrap();
        assert!(contract.workgroup_brand().is_some());
        assert!(contract.epoch_before().is_some());
        assert!(contract.epoch_after().is_none());
        let intrinsic = SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract };
        let encoded = compiler_intrinsic_round_trip(intrinsic, SemanticMirWireVersionV1::V22);
        assert_eq!(&encoded[..2], &[73, tag]);
        for wire in [
            SemanticMirWireVersionV1::V17,
            SemanticMirWireVersionV1::V20,
            SemanticMirWireVersionV1::V21,
        ] {
            let mut writer = CanonicalWriterV1::new(4096);
            assert!(matches!(
                encode_compiler_intrinsic_operation(&mut writer, intrinsic, wire),
                Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                    required: SemanticMirWireVersionV1::V22,
                    ..
                })
            ));
            let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
            decoder.wire_version = wire;
            assert!(decoder.compiler_intrinsic().is_err());
        }
    }
}

#[test]
fn workgroup_index_old_producer_bytes_do_not_change() {
    let legacy = SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndex {
        workgroup: SemanticTypeIdV1::from_index(1),
        witness: SemanticTypeIdV1::from_index(2),
    };
    let intrinsic = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: execution_contract(legacy).unwrap(),
    };
    let old = compiler_intrinsic_round_trip(intrinsic, SemanticMirWireVersionV1::V17);
    assert_eq!(&old[..2], &[73, 18]);
    assert_eq!(
        old,
        compiler_intrinsic_round_trip(intrinsic, SemanticMirWireVersionV1::V22)
    );
}

#[test]
fn workgroup_index_contract_rejects_payload_identity_and_epoch_substitution() {
    let [issue, convert] = workgroup_index_operations_v22();
    let valid = execution_contract(issue).unwrap();
    let SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
        workgroup_reference,
        workgroup,
        option,
        witness,
    } = issue
    else {
        unreachable!()
    };
    assert!(
        execution_contract(
            SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
                workgroup_reference,
                workgroup,
                option,
                witness: option,
            }
        )
        .is_err()
    );
    assert!(
        execution_contract(
            SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexV2 {
                workgroup_reference: workgroup,
                workgroup,
                option,
                witness,
            }
        )
        .is_err()
    );
    assert!(
        execution_contract(
            SemanticExecutionCapabilityOperationV1::WorkgroupMemoryIndexIntoDisjoint {
                input_witness: witness,
                output_witness: witness,
            }
        )
        .is_err()
    );
    let conversion = execution_contract(convert).unwrap();
    assert!(
        SemanticExecutionCapabilityContractV1::new(
            convert,
            conversion.signature(),
            conversion.provenance(),
            conversion.workgroup_brand().unwrap(),
            conversion.epoch_before().unwrap(),
            Some(SemanticTypeIdentityV1(identity(120))),
            conversion.source_identity(),
        )
        .is_err()
    );
    assert!(
        SemanticExecutionCapabilityContractV1::new_kernel_scoped(
            issue,
            valid.signature(),
            valid.provenance(),
            valid.source_identity(),
        )
        .is_err()
    );
    assert!(
        SemanticExecutionCapabilityContractV1::new(
            issue,
            SemanticExecutionCapabilitySignatureV1::new(&[workgroup_reference], witness).unwrap(),
            valid.provenance(),
            valid.workgroup_brand().unwrap(),
            valid.epoch_before().unwrap(),
            None,
            valid.source_identity(),
        )
        .is_err()
    );
}

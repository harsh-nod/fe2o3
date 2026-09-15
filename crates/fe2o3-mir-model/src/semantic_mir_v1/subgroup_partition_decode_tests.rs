fn partition_operations() -> [SemanticExecutionCapabilityOperationV1; 3] {
    use SemanticSubgroupPartitionOperationV1 as P;
    [
        P::Derive {
            subgroup_reference: SemanticTypeIdV1(1),
            subgroup: SemanticTypeIdV1(2),
            epoch: SemanticTypeIdV1(3),
            partition: SemanticTypeIdV1(4),
            width: 64,
            partition_width: 16,
        },
        P::ReduceSumF32 {
            partition_reference: SemanticTypeIdV1(5),
            partition: SemanticTypeIdV1(4),
            element: SemanticTypeIdV1(6),
            width: 64,
            partition_width: 16,
        },
        P::BroadcastF32 {
            partition_reference: SemanticTypeIdV1(5),
            partition: SemanticTypeIdV1(4),
            element: SemanticTypeIdV1(6),
            source_lane: SemanticTypeIdV1(7),
            width: 64,
            partition_width: 16,
        },
    ]
    .map(SemanticExecutionCapabilityOperationV1::SubgroupPartition)
}

#[test]
fn subgroup_partition_v18_records_keep_legacy_and_policy_wire_widths() {
    let legacy = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: execution_contract(SemanticExecutionCapabilityOperationV1::SubgroupDerive {
            workgroup: SemanticTypeIdV1(3),
            subgroup: SemanticTypeIdV1(2),
            width: 64,
        })
        .unwrap(),
    };
    let partitions = partition_operations().map(|operation| {
        SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: execution_contract(operation).unwrap(),
        }
    });
    let policy = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: numerical_contract(),
    };
    let operations = [
        legacy,
        partitions[0],
        policy,
        partitions[1],
        partitions[2],
        legacy,
    ];
    let records = operations
        .map(|operation| compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V18));
    assert_eq!(
        records.each_ref().map(|record| record.len()),
        [320, 337, 286, 333, 341, 320]
    );
    assert_eq!(
        records[0],
        compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V17)
    );
    assert_eq!(records[0], records[5]);
    let encoded = records.concat();
    let mut decoder = CanonicalDecoderV1::new(&encoded, SemanticMirLimitsV1::default());
    decoder.wire_version = SemanticMirWireVersionV1::V18;
    let mut offset = 0;
    for (operation, record) in operations.iter().zip(&records) {
        assert_eq!(decoder.compiler_intrinsic().unwrap(), *operation);
        offset += record.len();
        assert_eq!(decoder.offset, offset);
    }
    decoder.finish().unwrap();
    for operation in partitions {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            encode_compiler_intrinsic_operation(
                &mut writer,
                operation,
                SemanticMirWireVersionV1::V17
            )
            .is_err()
        );
        let bytes = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V18);
        let mut old_reader = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        old_reader.wire_version = SemanticMirWireVersionV1::V17;
        assert!(old_reader.compiler_intrinsic().is_err());
        for length in 0..bytes.len() {
            let mut reader =
                CanonicalDecoderV1::new(&bytes[..length], SemanticMirLimitsV1::default());
            reader.wire_version = SemanticMirWireVersionV1::V18;
            assert!(reader.compiler_intrinsic().is_err());
        }
        let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation
        else {
            unreachable!()
        };
        let source_offset = bytes.len() - 32;
        assert_eq!(
            &bytes[source_offset - 2..source_offset],
            &u16::try_from(contract.obligations().bits())
                .unwrap()
                .to_le_bytes()
        );
        let mut widened = bytes;
        widened.splice(source_offset..source_offset, [0, 0]);
        let mut reader = CanonicalDecoderV1::new(&widened, SemanticMirLimitsV1::default());
        reader.wire_version = SemanticMirWireVersionV1::V18;
        assert!(reader.compiler_intrinsic().is_err() || reader.finish().is_err());
    }
}

#[test]
fn subgroup_partition_requires_v18_and_retains_exact_obligations() {
    use SemanticExecutionSafetyObligationsV1 as O;
    let base = O::TARGET_SUPPORT | O::DYNAMIC_WORKGROUP_IDENTITY | O::LIFETIME_VALIDITY;
    for (operation, bits) in partition_operations().into_iter().zip([
        base,
        base | O::SUBGROUP_CONVERGENCE | O::EXACT_PARTICIPATION,
        base | O::SUBGROUP_CONVERGENCE | O::EXACT_PARTICIPATION | O::BOUNDS,
    ]) {
        let contract = execution_contract(operation).unwrap();
        assert_eq!(contract.obligations().bits(), bits);
        assert!(!operation_is_kernel_scoped(operation));
        let request = version_selection_request([
            SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract },
        ]);
        assert_eq!(
            minimum_wire_version(&request),
            SemanticMirWireVersionV1::V18
        );
    }
}

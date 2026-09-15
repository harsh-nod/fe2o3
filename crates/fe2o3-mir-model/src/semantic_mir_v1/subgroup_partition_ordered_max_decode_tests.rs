fn ordered_partition_max_intrinsic() -> SemanticCompilerIntrinsicOperationV1 {
    SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
        contract: execution_contract(SemanticExecutionCapabilityOperationV1::SubgroupPartition(
            SemanticSubgroupPartitionOperationV1::ReduceMaxF32 {
                partition_reference: SemanticTypeIdV1(5),
                partition: SemanticTypeIdV1(4),
                element: SemanticTypeIdV1(6),
                width: 64,
                partition_width: 16,
            },
        ))
        .unwrap(),
    }
}

#[test]
fn ordered_partition_max_requires_v22_without_reinterpreting_sum_or_legacy_partitions() {
    let operation = ordered_partition_max_intrinsic();
    assert_eq!(
        minimum_wire_version(&version_selection_request([operation])),
        SemanticMirWireVersionV1::V22
    );
    let bytes = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V22);
    assert_eq!(&bytes[..3], &[73, 24, 3]);
    assert_eq!(bytes.len(), 333);
    for version in [
        SemanticMirWireVersionV1::V18,
        SemanticMirWireVersionV1::V19,
        SemanticMirWireVersionV1::V20,
        SemanticMirWireVersionV1::V21,
    ] {
        let mut writer = CanonicalWriterV1::new(HARD_MAX_CANONICAL_BYTES_V1);
        assert!(
            matches!(encode_compiler_intrinsic_operation(&mut writer, operation, version),
            Err(SemanticMirErrorV1::WireVersionCannotRepresent { requested, required: SemanticMirWireVersionV1::V22 }) if requested == version)
        );
        let mut reader = CanonicalDecoderV1::new(&bytes, SemanticMirLimitsV1::default());
        reader.wire_version = version;
        assert!(reader.compiler_intrinsic().is_err());
    }
    for legacy in partition_operations() {
        let legacy = SemanticCompilerIntrinsicOperationV1::ExecutionCapability {
            contract: execution_contract(legacy).unwrap(),
        };
        assert_eq!(
            compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V18),
            compiler_intrinsic_round_trip(legacy, SemanticMirWireVersionV1::V22)
        );
    }
}

#[test]
fn ordered_partition_max_record_is_bounded_and_retains_exact_obligations() {
    let operation = ordered_partition_max_intrinsic();
    let bytes = compiler_intrinsic_round_trip(operation, SemanticMirWireVersionV1::V22);
    for length in 0..bytes.len() {
        let mut reader = CanonicalDecoderV1::new(&bytes[..length], SemanticMirLimitsV1::default());
        reader.wire_version = SemanticMirWireVersionV1::V22;
        assert!(reader.compiler_intrinsic().is_err());
    }
    let mut changed = bytes.clone();
    changed[2] = 1;
    let mut reader = CanonicalDecoderV1::new(&changed, SemanticMirLimitsV1::default());
    reader.wire_version = SemanticMirWireVersionV1::V22;
    let decoded = reader.compiler_intrinsic().unwrap();
    reader.finish().unwrap();
    assert_ne!(decoded, operation, "sum must remain a distinct operation");
    let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation else {
        unreachable!()
    };
    use SemanticExecutionSafetyObligationsV1 as O;
    assert_eq!(
        contract.obligations().bits(),
        O::TARGET_SUPPORT
            | O::DYNAMIC_WORKGROUP_IDENTITY
            | O::LIFETIME_VALIDITY
            | O::SUBGROUP_CONVERGENCE
            | O::EXACT_PARTICIPATION
    );
    assert_eq!(
        contract.signature().arguments().collect::<Vec<_>>(),
        [SemanticTypeIdV1(5), SemanticTypeIdV1(6)]
    );
}

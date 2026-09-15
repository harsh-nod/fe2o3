#[test]
fn ordered_partition_max_lowering_preserves_nominal_reference_and_exact_f32() {
    use SemanticSubgroupPartitionOperationV1 as M;
    use fe2o3_kernel_ir::SubgroupPartitionOperationV1 as K;
    let types = types();
    let ty = SemanticTypeIdV1::from_index;
    let operation = M::ReduceMaxF32 {
        partition_reference: ty(4),
        partition: ty(3),
        element: ty(5),
        width: 64,
        partition_width: 16,
    };
    let actual = lower_subgroup_partition_operation_v1(&types, operation).unwrap();
    assert_eq!(
        actual,
        K::ReduceMaxF32 {
            partition_reference: execution_type_identity_v1(&types, ty(4)).unwrap(),
            partition: execution_type_identity_v1(&types, ty(3)).unwrap(),
            element: execution_type_identity_v1(&types, ty(5)).unwrap(),
            width: 64,
            partition_width: 16,
        }
    );
    for (element, width, partition_width) in [(6, 64, 16), (5, 48, 16), (5, 64, 3), (5, 32, 64)] {
        let bad = M::ReduceMaxF32 {
            partition_reference: ty(4),
            partition: ty(3),
            element: ty(element),
            width,
            partition_width,
        };
        assert!(lower_subgroup_partition_operation_v1(&types, bad).is_err());
    }
    let sum = M::ReduceSumF32 {
        partition_reference: ty(4),
        partition: ty(3),
        element: ty(5),
        width: 64,
        partition_width: 16,
    };
    assert_ne!(
        actual,
        lower_subgroup_partition_operation_v1(&types, sum).unwrap()
    );
}

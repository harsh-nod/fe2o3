use fe2o3_hsaco_finalize::WorkerV2RawHsacoInspectionError;

#[test]
fn legacy_launch_error_variant_shapes_and_messages_remain_exact() {
    let required = WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch {
        kernel: "vecadd".to_owned(),
        actual: Some([128, 2, 1]),
    };
    assert_eq!(
        required.to_string(),
        "kernel vecadd requires Some([128, 2, 1]), expected [256, 1, 1]"
    );
    let WorkerV2RawHsacoInspectionError::RequiredWorkgroupSizeMismatch { kernel, actual } =
        required
    else {
        panic!("constructed legacy required-workgroup variant changed");
    };
    assert_eq!(kernel, "vecadd");
    assert_eq!(actual, Some([128, 2, 1]));

    let max_flat = WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch {
        kernel: "vecadd".to_owned(),
        actual: 512,
    };
    assert_eq!(
        max_flat.to_string(),
        "kernel vecadd max flat workgroup is 512, expected 256"
    );
    let WorkerV2RawHsacoInspectionError::MaxFlatWorkgroupSizeMismatch { kernel, actual } = max_flat
    else {
        panic!("constructed legacy max-flat-workgroup variant changed");
    };
    assert_eq!(kernel, "vecadd");
    assert_eq!(actual, 512);

    let metadata = WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch {
        kernel: "vecadd".to_owned(),
        actual: 32,
    };
    assert_eq!(
        metadata.to_string(),
        "kernel vecadd metadata wavefront is 32, expected 64"
    );
    let WorkerV2RawHsacoInspectionError::MetadataWavefrontSizeMismatch { kernel, actual } =
        metadata
    else {
        panic!("constructed legacy metadata-wavefront variant changed");
    };
    assert_eq!(kernel, "vecadd");
    assert_eq!(actual, 32);

    let descriptor = WorkerV2RawHsacoInspectionError::DescriptorWavefrontSizeMismatch {
        kernel: "vecadd".to_owned(),
        actual: 32,
    };
    assert_eq!(
        descriptor.to_string(),
        "kernel vecadd descriptor wavefront is 32, expected 64"
    );
    let WorkerV2RawHsacoInspectionError::DescriptorWavefrontSizeMismatch { kernel, actual } =
        descriptor
    else {
        panic!("constructed legacy descriptor-wavefront variant changed");
    };
    assert_eq!(kernel, "vecadd");
    assert_eq!(actual, 32);
}

#[test]
fn tiled_launch_error_variants_retain_dynamic_expectations() {
    let required = WorkerV2RawHsacoInspectionError::TiledGemmV1RequiredWorkgroupSizeMismatch {
        kernel: "tiled_gemm_v1".to_owned(),
        actual: Some([256, 1, 1]),
        expected: [64, 1, 1],
    };
    assert_eq!(
        required.to_string(),
        "tiled GEMM V1 kernel tiled_gemm_v1 requires Some([256, 1, 1]), expected [64, 1, 1]"
    );

    let max_flat = WorkerV2RawHsacoInspectionError::TiledGemmV1MaxFlatWorkgroupSizeMismatch {
        kernel: "tiled_gemm_v1".to_owned(),
        actual: 256,
        expected: 64,
    };
    assert_eq!(
        max_flat.to_string(),
        "tiled GEMM V1 kernel tiled_gemm_v1 max flat workgroup is 256, expected 64"
    );

    let metadata = WorkerV2RawHsacoInspectionError::TiledGemmV1MetadataWavefrontSizeMismatch {
        kernel: "tiled_gemm_v1".to_owned(),
        actual: 32,
        expected: 64,
    };
    assert_eq!(
        metadata.to_string(),
        "tiled GEMM V1 kernel tiled_gemm_v1 metadata wavefront is 32, expected 64"
    );

    let descriptor = WorkerV2RawHsacoInspectionError::TiledGemmV1DescriptorWavefrontSizeMismatch {
        kernel: "tiled_gemm_v1".to_owned(),
        actual: 32,
        expected: 64,
    };
    assert_eq!(
        descriptor.to_string(),
        "tiled GEMM V1 kernel tiled_gemm_v1 descriptor wavefront is 32, expected 64"
    );
}

#[test]
fn row_softmax_launch_errors_are_distinct_without_changing_legacy_wording() {
    let required = WorkerV2RawHsacoInspectionError::RowSoftmaxV1RequiredWorkgroupSizeMismatch {
        kernel: "row_softmax_v1".to_owned(),
        actual: Some([256, 1, 1]),
        expected: [64, 1, 1],
    };
    assert_eq!(
        required.to_string(),
        "row-softmax V1 kernel row_softmax_v1 requires Some([256, 1, 1]), expected [64, 1, 1]"
    );

    let max_flat = WorkerV2RawHsacoInspectionError::RowSoftmaxV1MaxFlatWorkgroupSizeMismatch {
        kernel: "row_softmax_v1".to_owned(),
        actual: 256,
        expected: 64,
    };
    assert_eq!(
        max_flat.to_string(),
        "row-softmax V1 kernel row_softmax_v1 max flat workgroup is 256, expected 64"
    );

    let metadata = WorkerV2RawHsacoInspectionError::RowSoftmaxV1MetadataWavefrontSizeMismatch {
        kernel: "row_softmax_v1".to_owned(),
        actual: 32,
        expected: 64,
    };
    assert_eq!(
        metadata.to_string(),
        "row-softmax V1 kernel row_softmax_v1 metadata wavefront is 32, expected 64"
    );

    let descriptor = WorkerV2RawHsacoInspectionError::RowSoftmaxV1DescriptorWavefrontSizeMismatch {
        kernel: "row_softmax_v1".to_owned(),
        actual: 32,
        expected: 64,
    };
    assert_eq!(
        descriptor.to_string(),
        "row-softmax V1 kernel row_softmax_v1 descriptor wavefront is 32, expected 64"
    );
}

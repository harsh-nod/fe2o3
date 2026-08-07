use fe2o3_compiler_ffi::{CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1};
use fe2o3_kernel_descriptor::{
    BlockSizeV1, BuildEvidenceV1, CanonicalCodeObjectDigest, CodeObjectVersion, CompilerIdentityV1,
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
    DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
    KernelId, LaunchConstraintsV1, LogicalArgumentV1, ProducerIdentityV1, ScalarTypeV1,
    SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName, encode_device_descriptor_table_v1,
};

fn table(digest: [u8; 32]) -> DeviceDescriptorTableV1 {
    let source_type = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let argument = LogicalArgumentV1::scalar(
        0,
        ValidName::new("value").unwrap(),
        &source_type,
        &layout,
        0,
    )
    .unwrap();
    let evidence = |byte| {
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([byte; 32]),
            EvidenceDigest::from_sha256_bytes([byte.wrapping_add(1); 32]),
        )
    };
    let launch = LaunchConstraintsV1::new(
        1,
        BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
        DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        256,
        0,
        0,
    )
    .unwrap();
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x11; 32]),
        ValidName::new("scale").unwrap(),
        ValidName::new("scale").unwrap(),
        ValidName::new("scale.kd").unwrap(),
        evidence(0x21),
        evidence(0x31),
        vec![],
        KernelAbiLayoutV1::new(4, 4, 4).unwrap(),
        launch,
        vec![argument],
    )
    .unwrap();

    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes(digest),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3").unwrap(),
            Text::new("test").unwrap(),
            [0x41; 20],
        ),
        ProducerIdentityV1::new(
            Text::new("rustc-codegen-fe2o3").unwrap(),
            Text::new("test").unwrap(),
        ),
        DeviceTargetV1::parse("gfx942:xnack-").unwrap(),
        vec![source_type],
        vec![layout],
        vec![kernel],
    )
    .unwrap()
}

#[test]
fn zero_digest_source_round_trips_exact_bytes_and_identity() {
    let source = CompilerDescriptorSourceV1::new(table([0; 32])).unwrap();
    let decoded = CompilerDescriptorSourceV1::decode(source.canonical_bytes()).unwrap();

    assert_eq!(decoded, source);
    assert!(source.identity().matches(source.canonical_bytes()));
    assert_eq!(source.table().device_target().to_string(), "gfx942:xnack-");
    assert_eq!(source.table().code_object_version(), CodeObjectVersion::V6);
    assert!(!source.authenticates_compiler_origin());
    assert!(!source.grants_link_authority());
    assert!(!source.grants_load_authority());
    assert!(!source.grants_launch_authority());
}

#[test]
fn finalized_descriptor_table_is_rejected_as_compiler_source() {
    assert!(matches!(
        CompilerDescriptorSourceV1::new(table([0x55; 32])),
        Err(CompilerDescriptorSourceErrorV1::FinalizedDigest)
    ));

    let encoded = encode_device_descriptor_table_v1(&table([0x55; 32])).unwrap();
    assert!(matches!(
        CompilerDescriptorSourceV1::decode(&encoded),
        Err(CompilerDescriptorSourceErrorV1::FinalizedDigest)
    ));
}

#[test]
fn malformed_and_trailing_encodings_fail_closed() {
    let source = CompilerDescriptorSourceV1::new(table([0; 32])).unwrap();
    for length in 0..source.canonical_bytes().len() {
        assert!(CompilerDescriptorSourceV1::decode(&source.canonical_bytes()[..length]).is_err());
    }

    let mut trailing = source.canonical_bytes().to_vec();
    trailing.push(0);
    assert!(CompilerDescriptorSourceV1::decode(&trailing).is_err());
}

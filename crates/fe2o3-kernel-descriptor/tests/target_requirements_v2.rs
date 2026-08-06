use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_amd_target::AmdTargetId;
use fe2o3_kernel_descriptor::*;
use sha2::{Digest, Sha256};

const KERNEL_ID: KernelId = KernelId::from_bytes([0x42; 32]);

fn name(value: &str) -> ValidName {
    ValidName::new(value).expect("valid test name")
}

fn text(value: &str) -> Text {
    Text::new(value).expect("valid test text")
}

fn launch(static_lds: u32, dynamic_lds: u32) -> LaunchConstraintsV1 {
    LaunchConstraintsV1::new(
        1,
        BlockSizeV1::AtMost(DimensionsV1::new(256, 1, 1).expect("valid block")),
        DimensionsV1::new(65_535, 1, 1).expect("valid grid"),
        256,
        static_lds,
        dynamic_lds,
    )
    .expect("valid launch constraints")
}

fn base(
    target: &str,
    static_lds: u32,
    dynamic_lds: u32,
    capabilities: Vec<CapabilityV1>,
) -> DeviceDescriptorTableV1 {
    let kernel = KernelDescriptorV1::new(
        KERNEL_ID,
        name("kernel"),
        name("kernel"),
        name("kernel.kd"),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([0x11; 32]),
            EvidenceDigest::from_sha256_bytes([0x22; 32]),
        ),
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes([0x33; 32]),
            EvidenceDigest::from_sha256_bytes([0x44; 32]),
        ),
        capabilities,
        KernelAbiLayoutV1::new(0, 0, 8).expect("valid empty ABI"),
        launch(static_lds, dynamic_lds),
        Vec::new(),
    )
    .expect("valid kernel");
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0xaa; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc"), text("nightly"), [0x55; 20]),
        ProducerIdentityV1::new(text("fe2o3"), text("0.1")),
        DeviceTargetV1::new(AmdTargetId::parse(target).expect("known target")),
        Vec::new(),
        Vec::new(),
        vec![kernel],
    )
    .expect("valid V1 base")
}

fn requirement(
    static_lds: u32,
    dynamic_lds: u32,
    wave: RequiredWavefrontWidthV2,
) -> KernelTargetRequirementsV2 {
    KernelTargetRequirementsV2::new(
        KERNEL_ID,
        LdsRequirementsV2::new(static_lds, dynamic_lds).expect("bounded LDS"),
        wave,
        true,
        SynchronizationRequirementsV2::from_bits(
            SynchronizationRequirementsV2::WAVE_BARRIER
                | SynchronizationRequirementsV2::WORKGROUP_BARRIER
                | SynchronizationRequirementsV2::SYSTEM_FENCE,
        )
        .expect("known synchronization bits"),
        AtomicRequirementsV2::from_bits(
            AtomicRequirementsV2::WORKGROUP_SCOPE
                | AtomicRequirementsV2::DEVICE_SCOPE
                | AtomicRequirementsV2::SYSTEM_SCOPE,
        )
        .expect("known atomic bits"),
    )
}

fn full_capabilities() -> Vec<CapabilityV1> {
    vec![
        CapabilityV1::AmdWave,
        CapabilityV1::Atomics,
        CapabilityV1::Subgroup,
        CapabilityV1::WorkgroupMemory,
    ]
}

fn fixture() -> DeviceDescriptorTableV2 {
    DeviceDescriptorTableV2::new(
        base("gfx1151", 1_024, 4_096, full_capabilities()),
        vec![requirement(1_024, 4_096, RequiredWavefrontWidthV2::Wave32)],
    )
    .expect("valid V2 fixture")
}

fn requirement_offset(bytes: &[u8]) -> usize {
    const V2_HEADER_BYTES: usize = 24;
    let base_len = u32::from_le_bytes(bytes[16..20].try_into().expect("base length")) as usize;
    V2_HEADER_BYTES + base_len
}

#[test]
fn golden_round_trip_preserves_the_embedded_v1_table() {
    let table = fixture();
    let encoded = encode_device_descriptor_table_v2(&table).expect("encode V2");
    let decoded = decode_device_descriptor_table_v2(&encoded).expect("decode V2");
    assert_eq!(decoded, table);
    assert_eq!(
        encode_device_descriptor_table_v2(&decoded).expect("re-encode V2"),
        encoded
    );

    let base_wire = encode_device_descriptor_table_v1(table.base()).expect("encode V1");
    let offset = requirement_offset(&encoded) - base_wire.len();
    assert_eq!(&encoded[offset..offset + base_wire.len()], base_wire);
    assert_eq!(CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2, 40);
    assert_eq!(
        &encoded
            [CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2..CANONICAL_CODE_OBJECT_DIGEST_OFFSET_V2 + 32],
        &[0xaa; 32]
    );
    assert_eq!(
        decode_device_descriptor_table_v1(&encoded),
        Err(DecodeError::UnknownVersion(DEVICE_DESCRIPTOR_VERSION_V2))
    );
    assert_eq!(
        decode_device_descriptor_table_v2(&base_wire),
        Err(DecodeError::UnknownVersion(DEVICE_DESCRIPTOR_VERSION))
    );

    assert_eq!(encoded.len(), 450);
    let digest = Sha256::digest(&encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert_eq!(
        digest,
        "d0bdbb86151ede1d2521b4e3f02940d235cca6d360ead742606808d7063ffc89"
    );
    assert_eq!(
        &encoded[requirement_offset(&encoded)..],
        &[
            0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
            0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42, 0x42,
            0x42, 0x42, 0x42, 0x42, 0x00, 0x04, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x01, 0x01,
            0x13, 0x00, 0x07, 0x00, 0x00, 0x00,
        ]
    );
    assert!(table.requirements()[0].requires_runtime_evidence());
}

#[test]
fn every_truncation_and_a_deterministic_mutation_corpus_are_panic_free() {
    let encoded = encode_device_descriptor_table_v2(&fixture()).expect("encode");
    for length in 0..encoded.len() {
        assert!(decode_device_descriptor_table_v2(&encoded[..length]).is_err());
    }
    for index in 0..encoded.len() {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = encoded.clone();
            mutated[index] ^= mask;
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    let _ = decode_device_descriptor_table_v2(&mutated);
                }))
                .is_ok(),
                "decoder panicked at byte {index} with mask {mask:#x}"
            );
        }
    }
}

#[test]
fn malformed_tags_bits_reserved_fields_and_counts_fail_closed() {
    let encoded = encode_device_descriptor_table_v2(&fixture()).expect("encode");
    let requirement = requirement_offset(&encoded);

    let mut unknown_wave = encoded.clone();
    unknown_wave[requirement + 40] = 9;
    assert!(matches!(
        decode_device_descriptor_table_v2(&unknown_wave),
        Err(DecodeError::UnknownTag {
            kind: "required wavefront width",
            tag: 9
        })
    ));

    let mut unknown_cooperative = encoded.clone();
    unknown_cooperative[requirement + 41] = 2;
    assert!(matches!(
        decode_device_descriptor_table_v2(&unknown_cooperative),
        Err(DecodeError::UnknownTag {
            kind: "cooperative launch requirement",
            tag: 2
        })
    ));

    let mut unknown_sync = encoded.clone();
    unknown_sync[requirement + 43] = 0x80;
    assert!(matches!(
        decode_device_descriptor_table_v2(&unknown_sync),
        Err(DecodeError::Validation(ValidationError::InvalidValue {
            field: "synchronization requirement bits"
        }))
    ));

    let mut unknown_atomic = encoded.clone();
    unknown_atomic[requirement + 45] = 0x80;
    assert!(matches!(
        decode_device_descriptor_table_v2(&unknown_atomic),
        Err(DecodeError::Validation(ValidationError::InvalidValue {
            field: "atomic requirement bits"
        }))
    ));

    let mut reserved = encoded.clone();
    reserved[requirement + 46] = 1;
    assert_eq!(
        decode_device_descriptor_table_v2(&reserved),
        Err(DecodeError::NonzeroReserved {
            field: "kernel target requirement"
        })
    );

    let mut excessive_count = encoded;
    excessive_count[20..22].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        decode_device_descriptor_table_v2(&excessive_count),
        Err(DecodeError::CountOutOfRange {
            field: "kernel target requirements",
            ..
        })
    ));
}

#[test]
fn duplicates_missing_records_and_v1_conflicts_are_rejected() {
    let base = base("gfx1151", 1_024, 4_096, full_capabilities());
    let exact = requirement(1_024, 4_096, RequiredWavefrontWidthV2::Wave32);
    assert_eq!(
        DeviceDescriptorTableV2::new(base.clone(), vec![exact, exact]),
        Err(ValidationError::Duplicate {
            field: "kernel target requirement"
        })
    );
    assert_eq!(
        DeviceDescriptorTableV2::new(base.clone(), Vec::new()),
        Err(ValidationError::InvalidValue {
            field: "kernel target requirement closure"
        })
    );
    assert_eq!(
        DeviceDescriptorTableV2::new(
            base,
            vec![requirement(1_025, 4_096, RequiredWavefrontWidthV2::Wave32)]
        ),
        Err(ValidationError::InvalidValue {
            field: "LDS requirements conflict with V1 launch constraints"
        })
    );
}

#[test]
fn declaration_bounds_and_capability_conflicts_are_rejected() {
    assert_eq!(
        LdsRequirementsV2::new(u32::MAX, 1),
        Err(ValidationError::Overflow {
            field: "total LDS requirement"
        })
    );
    assert!(SynchronizationRequirementsV2::from_bits(1 << 15).is_err());
    assert!(AtomicRequirementsV2::from_bits(1 << 15).is_err());

    let no_atomics = vec![
        CapabilityV1::AmdWave,
        CapabilityV1::Subgroup,
        CapabilityV1::WorkgroupMemory,
    ];
    assert_eq!(
        DeviceDescriptorTableV2::new(
            base("gfx1151", 1_024, 4_096, no_atomics),
            vec![requirement(1_024, 4_096, RequiredWavefrontWidthV2::Wave32)]
        ),
        Err(ValidationError::InvalidValue {
            field: "atomic or fence requirement requires the atomics capability"
        })
    );

    let no_wave = vec![
        CapabilityV1::Atomics,
        CapabilityV1::Subgroup,
        CapabilityV1::WorkgroupMemory,
    ];
    assert!(matches!(
        DeviceDescriptorTableV2::new(
            base("gfx1151", 1_024, 4_096, no_wave),
            vec![requirement(1_024, 4_096, RequiredWavefrontWidthV2::Wave32)]
        ),
        Err(ValidationError::InvalidValue {
            field: "exact wavefront width requires the AMD wave capability"
        })
    ));

    let no_workgroup_memory = vec![
        CapabilityV1::AmdWave,
        CapabilityV1::Atomics,
        CapabilityV1::Subgroup,
    ];
    assert!(matches!(
        DeviceDescriptorTableV2::new(
            base("gfx1151", 1_024, 4_096, no_workgroup_memory),
            vec![requirement(1_024, 4_096, RequiredWavefrontWidthV2::Wave32)]
        ),
        Err(ValidationError::InvalidValue {
            field: "LDS or workgroup barrier requires the workgroup-memory capability"
        })
    ));

    let oversized = vec![0_u8; MAX_DESCRIPTOR_TABLE_BYTES + 1];
    assert_eq!(
        decode_device_descriptor_table_v2(&oversized),
        Err(DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES
        })
    );
}

#[test]
fn exact_wave_and_lds_requirements_are_checked_against_the_declared_target() {
    assert_eq!(
        DeviceDescriptorTableV2::new(
            base("gfx942", 1_024, 4_096, full_capabilities()),
            vec![requirement(1_024, 4_096, RequiredWavefrontWidthV2::Wave32)]
        ),
        Err(ValidationError::TargetMismatch {
            field: "exact wavefront width"
        })
    );
    assert_eq!(
        DeviceDescriptorTableV2::new(
            base("gfx1151", 65_536, 1, full_capabilities()),
            vec![requirement(65_536, 1, RequiredWavefrontWidthV2::Wave32)]
        ),
        Err(ValidationError::TargetMismatch {
            field: "maximum LDS bytes per workgroup"
        })
    );
}

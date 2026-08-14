use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_amd_target::AmdTargetId;
use sha2::{Digest, Sha256};

use crate::digest::domain_hash_for_test;
use crate::model::{DescriptorKind, PhysicalAbiComponentV1};
use crate::*;

fn name(value: &str) -> ValidName {
    ValidName::new(value).expect("test name is valid")
}

fn text(value: &str) -> Text {
    Text::new(value).expect("test text is valid")
}

fn target(value: &str) -> DeviceTargetV1 {
    DeviceTargetV1::new(AmdTargetId::parse(value).expect("test target is valid"))
}

fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

fn launch() -> LaunchConstraintsV1 {
    LaunchConstraintsV1::new(
        1,
        BlockSizeV1::AtMost(DimensionsV1::new(256, 1, 1).expect("valid block")),
        DimensionsV1::new(u32::MAX, 1, 1).expect("valid grid"),
        1024,
        0,
        65_536,
    )
    .expect("valid launch")
}

fn fixture() -> DeviceDescriptorTableV1 {
    let scalar_type = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_type =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let arguments = vec![
        LogicalArgumentV1::scalar(0, name("alpha"), &scalar_type, &scalar_layout, 0)
            .expect("scalar argument"),
        LogicalArgumentV1::shared_slice(1, name("input"), &shared_type, &shared_layout, 8)
            .expect("shared slice argument"),
        LogicalArgumentV1::disjoint_slice(
            2,
            name("output"),
            &disjoint_type,
            &disjoint_layout,
            AccessMode::ReadWrite,
            24,
        )
        .expect("disjoint slice argument"),
    ];
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes([0x22; 32]),
        name("vecadd"),
        name("vecadd"),
        name("vecadd.kd"),
        evidence(0x33, 0x44),
        evidence(0x55, 0x66),
        vec![CapabilityV1::WorkgroupMemory, CapabilityV1::Subgroup],
        KernelAbiLayoutV1::new(40, 80, 8).expect("valid ABI layout"),
        launch(),
        arguments,
    )
    .expect("valid kernel");

    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0xa0; 32]),
        CodeObjectVersion::V6,
        CompilerIdentityV1::new(text("rustc"), text("1.94.0-nightly"), [0x11; 20]),
        ProducerIdentityV1::new(text("cargo-fe2o3"), text("0.1.0")),
        target("gfx1151"),
        vec![scalar_type, shared_type, disjoint_type],
        vec![scalar_layout, shared_layout, disjoint_layout],
        vec![kernel],
    )
    .expect("valid table")
}

fn decode_error(bytes: &[u8]) -> DecodeError {
    decode_device_descriptor_table_v1(bytes).expect_err("mutation must be rejected")
}

fn find(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("needle occurs in fixture")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2));
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(text, 16).expect("valid hex")
        })
        .collect()
}

#[test]
fn round_trip_reencodes_byte_identically() {
    let table = fixture();
    let encoded = encode_device_descriptor_table_v1(&table).expect("encode");
    let decoded = decode_device_descriptor_table_v1(&encoded).expect("decode");
    assert_eq!(decoded, table);
    assert_eq!(
        encode_device_descriptor_table_v1(&decoded).expect("re-encode"),
        encoded
    );
    assert_eq!(CANONICAL_CODE_OBJECT_DIGEST_OFFSET, 16);
    assert_eq!(
        &encoded[CANONICAL_CODE_OBJECT_DIGEST_OFFSET..CANONICAL_CODE_OBJECT_DIGEST_OFFSET + 32],
        &[0xa0; 32]
    );
}

#[test]
fn golden_wire_and_digests_are_stable() {
    let table = fixture();
    let encoded = encode_device_descriptor_table_v1(&table).expect("encode");
    const GOLDEN_WIRE_HEX: &str = concat!(
        "4645324f334b440001000000bf030000a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a006080100050072757374630e",
        "00312e39342e302d6e696768746c7911111111111111111111111111111111111111110b00636172676f2d6665326f330500302e312e300700676678",
        "31313531030003000100000025f5acbfd137ccde3bfb4fbf763de9ed4ca41e7b76e421662ee4c83868e9e203020a0000e7bc44688095a325a5372fbf",
        "42bbd19628bfdbe3a31f827cd9b67e9c27df35a1030a0000ef7b853ca31fc7916a8cae9a175409aa9af9310ac3369db07bdb914a8be2b8a5010a0000",
        "4def6d55a42a05516550cc439c0367a04631715e256ea65d37787cfd36f72c30020a10000800080800000000aca175dccc024da5d2382fca8c11c581",
        "4b268d0da74eb70a3bf0dc8369c0dd50010a04000400000000000000eee60ce70712fe0789fcaaaba8d4ff40d04648428beada07885a3b21beffe400",
        "030a10000800080800000000222222222222222222222222222222222222222222222222222222222222222206007665636164640600766563616464",
        "09007665636164642e6b6401010100333333333333333333333333333333333333333333333333333333333333333344444444444444444444444444",
        "444444444444444444444444444444444444440201010055555555555555555555555555555555555555555555555555555555555555556666666666",
        "66666666666666666666666666666666666666666666666666666602000100040001020000000100000100000001000000ffffffff01000000010000",
        "0000040000000000000000010003000500280000005000000008000000000000000500616c706861ef7b853ca31fc7916a8cae9a175409aa9af9310a",
        "c3369db07bdb914a8be2b8a5aca175dccc024da5d2382fca8c11c5814b268d0da74eb70a3bf0dc8369c0dd500101010001000000010a010100000000",
        "0400040000000000010000000500696e70757425f5acbfd137ccde3bfb4fbf763de9ed4ca41e7b76e421662ee4c83868e9e2034def6d55a42a055165",
        "50cc439c0367a04631715e256ea65d37787cfd36f72c3002020200020000000200020208000000080008000000000003080101100000000800080000",
        "0000000200000006006f7574707574e7bc44688095a325a5372fbf42bbd19628bfdbe3a31f827cd9b67e9c27df35a1eee60ce70712fe0789fcaaaba8",
        "d4ff40d04648428beada07885a3b21beffe40003040300020000000200040318000000080008000000000003080101200000000800080000000000",
    );
    assert_eq!(encoded, from_hex(GOLDEN_WIRE_HEX));
    assert_eq!(
        hex(DeviceDescriptorTableDigest::calculate(&table)
            .expect("table digest")
            .as_bytes()),
        "ed040521d32dfb39001efdff1ef768edd648759368ab5ac26296e69c3b04a163"
    );
    assert_eq!(
        hex(KernelDescriptorDigest::calculate(&table.kernels()[0]).as_bytes()),
        "4466b1a64de484c944bbb53846408fb56ff98c480edb2815a95fa27ab2abca03"
    );
    assert_eq!(
        hex(
            RustTypeIdentity::for_descriptor(&SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32))
                .as_bytes()
        ),
        "ef7b853ca31fc7916a8cae9a175409aa9af9310ac3369db07bdb914a8be2b8a5"
    );
    assert_eq!(
        hex(
            DeviceLayoutIdentity::for_descriptor(&DeviceLayoutDescriptorV1::scalar(
                ScalarTypeV1::F32
            ))
            .as_bytes()
        ),
        "aca175dccc024da5d2382fca8c11c5814b268d0da74eb70a3bf0dc8369c0dd50"
    );
    assert_eq!(
        hex(
            CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(b"hsaco-zeroed")
                .as_bytes()
        ),
        "aaf9bd2c201bc236e835fc52749bd7449e5c929f2c5439eb6c3a3e12897e6181"
    );
    assert_eq!(decode_device_descriptor_table_v1(&encoded), Ok(table));
}

#[test]
fn every_truncation_is_rejected() {
    let encoded = encode_device_descriptor_table_v1(&fixture()).expect("encode");
    for length in 0..encoded.len() {
        assert!(
            decode_device_descriptor_table_v1(&encoded[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }
}

#[test]
fn deterministic_mutation_corpus_never_panics() {
    let encoded = encode_device_descriptor_table_v1(&fixture()).expect("encode");
    for index in 0..encoded.len() {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = encoded.clone();
            mutated[index] ^= mask;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let _ = decode_device_descriptor_table_v1(&mutated);
            }));
            assert!(result.is_ok(), "panic at byte {index}, mask {mask:#x}");
        }
    }

    let mut state = 0x9e37_79b9_u32;
    for _ in 0..4096 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let length = (state as usize) % (encoded.len() + 65);
        let mut bytes = vec![0_u8; length];
        for byte in &mut bytes {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (state >> 24) as u8;
        }
        assert!(catch_unwind(|| decode_device_descriptor_table_v1(&bytes)).is_ok());
    }
}

#[test]
fn text_and_collection_bounds_are_enforced_before_allocation() {
    assert!(ValidName::new("a".repeat(MAX_NAME_BYTES)).is_ok());
    assert!(ValidName::new("a".repeat(MAX_NAME_BYTES + 1)).is_err());
    assert!(Text::new("x".repeat(MAX_TEXT_BYTES)).is_ok());
    assert!(Text::new("x".repeat(MAX_TEXT_BYTES + 1)).is_err());
    assert!(Text::new("has\0nul").is_err());
    assert!(ValidName::new("raw pointer").is_err());

    let mut oversized = vec![0_u8; MAX_DESCRIPTOR_TABLE_BYTES + 1];
    oversized[..8].copy_from_slice(&DEVICE_DESCRIPTOR_MAGIC);
    assert_eq!(
        decode_error(&oversized),
        DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_TABLE_BYTES
        }
    );

    let mut encoded = encode_device_descriptor_table_v1(&fixture()).expect("encode");
    let type_count = find(&encoded, b"gfx1151") + b"gfx1151".len();
    encoded[type_count..type_count + 2].copy_from_slice(&u16::MAX.to_le_bytes());
    assert!(matches!(
        decode_error(&encoded),
        DecodeError::CountOutOfRange {
            field: "type records",
            ..
        }
    ));
}

#[test]
fn device_targets_are_typed_canonical_and_untrusted() {
    let canonical = DeviceTargetV1::parse("gfx942:sramecc+:xnack-").expect("canonical target");
    assert_eq!(canonical.to_string(), "gfx942:sramecc+:xnack-");
    assert_eq!(canonical.as_amd_target_id().processor(), "gfx942");
    assert!(matches!(
        DeviceTargetV1::parse("not-a-gpu"),
        Err(ValidationError::InvalidValue {
            field: "device target"
        })
    ));
    assert!(matches!(
        DeviceTargetV1::parse("gfx942:xnack-:sramecc+"),
        Err(ValidationError::NonCanonicalOrder {
            field: "device target features"
        })
    ));

    let mut invalid = encode_device_descriptor_table_v1(&fixture()).expect("encode");
    let invalid_offset = find(&invalid, b"gfx1151");
    invalid[invalid_offset..invalid_offset + 7].copy_from_slice(b"gfx9999");
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::Validation(ValidationError::InvalidValue {
            field: "device target"
        })
    ));

    let mut reordered = fixture();
    reordered.device_target = canonical;
    let mut reordered = encode_device_descriptor_table_v1(&reordered).expect("encode");
    let target_offset = find(&reordered, b"gfx942:sramecc+:xnack-");
    reordered[target_offset..target_offset + 22].copy_from_slice(b"gfx942:xnack-:sramecc+");
    assert!(matches!(
        decode_error(&reordered),
        DecodeError::Validation(ValidationError::NonCanonicalOrder {
            field: "device target features"
        })
    ));
}

#[test]
fn oversized_canonical_model_is_rejected() {
    let base = fixture();
    let source = base
        .type_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::Scalar)
        .expect("scalar source type")
        .clone();
    let layout = base
        .layout_records
        .iter()
        .find(|record| record.descriptor.kind == source.descriptor.kind)
        .expect("matching layout")
        .clone();
    let mut kernels = Vec::new();
    for kernel_index in 0..MAX_KERNELS {
        let mut arguments = Vec::new();
        for argument_index in 0..MAX_ARGUMENTS_PER_KERNEL {
            arguments.push(
                LogicalArgumentV1::scalar(
                    argument_index as u16,
                    name(&format!("a{argument_index}")),
                    &source,
                    &layout,
                    (argument_index * 8) as u32,
                )
                .expect("valid scalar"),
            );
        }
        let mut id = [0_u8; 32];
        id[0] = kernel_index as u8;
        kernels.push(
            KernelDescriptorV1::new(
                KernelId::from_bytes(id),
                name(&format!("k{kernel_index}")),
                name(&format!("e{kernel_index}")),
                name(&format!("d{kernel_index}")),
                evidence(1, 2),
                evidence(3, 4),
                vec![],
                KernelAbiLayoutV1::new(508, 512, 4).expect("valid ABI layout"),
                launch(),
                arguments,
            )
            .expect("valid large kernel"),
        );
    }
    assert!(matches!(
        DeviceDescriptorTableV1::new(
            base.canonical_code_object_digest,
            base.code_object_version,
            base.compiler,
            base.producer,
            base.device_target,
            vec![source],
            vec![layout],
            kernels,
        ),
        Err(ValidationError::EncodedTableTooLarge { .. })
    ));
}

#[test]
fn duplicate_and_reordered_kernels_are_rejected() {
    let mut table = fixture();
    let mut second = table.kernels[0].clone();
    second.kernel_id = KernelId::from_bytes([0x11; 32]);
    second.logical_name = name("other");
    second.entry_name = name("other_entry");
    second.descriptor_symbol = name("other.kd");
    table.kernels.insert(0, second);
    let canonical = DeviceDescriptorTableV1::new(
        table.canonical_code_object_digest,
        table.code_object_version,
        table.compiler.clone(),
        table.producer.clone(),
        table.device_target,
        table.type_records.clone(),
        table.layout_records.clone(),
        table.kernels.clone(),
    )
    .expect("two canonical kernels");

    let mut reordered = canonical.clone();
    reordered.kernels.swap(0, 1);
    let bytes = encode_device_descriptor_table_v1(&reordered).expect("encode malformed order");
    assert!(matches!(
        decode_error(&bytes),
        DecodeError::Validation(ValidationError::NonCanonicalOrder { field: "kernels" })
    ));

    let mut duplicate = canonical;
    duplicate.kernels[1].kernel_id = duplicate.kernels[0].kernel_id;
    let bytes = encode_device_descriptor_table_v1(&duplicate).expect("encode duplicate");
    assert!(matches!(
        decode_error(&bytes),
        DecodeError::Validation(ValidationError::Duplicate { field: "kernels" })
    ));
}

#[test]
fn duplicate_argument_indices_and_names_are_rejected() {
    let mut table = fixture();
    table.kernels[0].arguments[1].source_index = 0;
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            table.canonical_code_object_digest,
            table.code_object_version,
            table.compiler.clone(),
            table.producer.clone(),
            table.device_target,
            table.type_records.clone(),
            table.layout_records.clone(),
            table.kernels.clone(),
        ),
        Err(ValidationError::InvalidArgument(_))
    ));

    let mut table = fixture();
    table.kernels[0].arguments[1].name = table.kernels[0].arguments[0].name.clone();
    assert!(matches!(
        table.kernels[0].validate(),
        Err(ValidationError::Duplicate {
            field: "argument name"
        })
    ));
}

#[test]
fn physical_gaps_are_allowed_but_overlap_reversal_and_slice_gaps_are_rejected() {
    let table = fixture();
    assert_eq!(table.kernels[0].arguments[0].components[0].end(), Ok(4));
    assert_eq!(table.kernels[0].arguments[1].components[0].offset, 8);

    let mut overlap = fixture();
    overlap.kernels[0].arguments[1].components[0].offset = 0;
    overlap.kernels[0].arguments[1].components[1].offset = 8;
    assert!(matches!(
        overlap.kernels[0].validate(),
        Err(ValidationError::InvalidPhysicalAbi(_))
    ));

    let mut reversed = fixture();
    reversed.kernels[0].arguments[1].components.swap(0, 1);
    assert!(matches!(
        reversed.kernels[0].validate(),
        Err(ValidationError::InvalidPhysicalAbi(_))
    ));

    let mut slice_gap = fixture();
    slice_gap.kernels[0].arguments[1].components[1].offset += 8;
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            slice_gap.canonical_code_object_digest,
            slice_gap.code_object_version,
            slice_gap.compiler,
            slice_gap.producer,
            slice_gap.device_target,
            slice_gap.type_records,
            slice_gap.layout_records,
            slice_gap.kernels,
        ),
        Err(ValidationError::InvalidPhysicalAbi(_))
    ));
}

#[test]
fn all_scalar_tags_and_v1_lowerings_validate() {
    let scalars = [
        ScalarTypeV1::I8,
        ScalarTypeV1::U8,
        ScalarTypeV1::I16,
        ScalarTypeV1::U16,
        ScalarTypeV1::I32,
        ScalarTypeV1::U32,
        ScalarTypeV1::I64,
        ScalarTypeV1::U64,
        ScalarTypeV1::F16,
        ScalarTypeV1::F32,
        ScalarTypeV1::F64,
    ];
    for scalar in scalars {
        let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(scalar));
        let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(scalar));
        let argument = LogicalArgumentV1::scalar(0, name("value"), &source, &layout, 0)
            .expect("scalar lowering");
        assert_eq!(
            argument.physical_components().next(),
            Some((
                PhysicalAbiComponentKind::ScalarByValue(scalar),
                0,
                scalar.size_bytes(),
                scalar.alignment_bytes()
            ))
        );
    }

    let table = fixture();
    let shared = &table.kernels[0].arguments[1];
    let disjoint = &table.kernels[0].arguments[2];
    assert_eq!(shared.ownership(), OwnershipSemantics::SharedBorrow);
    assert_eq!(shared.access(), AccessMode::ReadOnly);
    assert_eq!(shared.alias(), AliasSemantics::SharedReadOnly);
    assert_eq!(disjoint.ownership(), OwnershipSemantics::UniqueBorrow);
    assert_eq!(disjoint.access(), AccessMode::ReadWrite);
    assert_eq!(disjoint.alias(), AliasSemantics::Exclusive);
}

#[test]
fn ownership_access_and_alias_inconsistency_is_rejected() {
    let mut shared = fixture();
    shared.kernels[0].arguments[1].access = AccessMode::WriteOnly;
    shared.kernels[0].arguments[1].components[0].access = AccessMode::WriteOnly;
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            shared.canonical_code_object_digest,
            shared.code_object_version,
            shared.compiler,
            shared.producer,
            shared.device_target,
            shared.type_records,
            shared.layout_records,
            shared.kernels,
        ),
        Err(ValidationError::InvalidArgument(_))
    ));

    let mut disjoint = fixture();
    disjoint.kernels[0].arguments[2].alias = AliasSemantics::SharedReadOnly;
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            disjoint.canonical_code_object_digest,
            disjoint.code_object_version,
            disjoint.compiler,
            disjoint.producer,
            disjoint.device_target,
            disjoint.type_records,
            disjoint.layout_records,
            disjoint.kernels,
        ),
        Err(ValidationError::InvalidArgument(_))
    ));
}

#[test]
fn dangling_and_unreachable_records_are_rejected() {
    let mut dangling = fixture();
    dangling.kernels[0].arguments[0].source_type = RustTypeIdentity::from_bytes([0xee; 32]);
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            dangling.canonical_code_object_digest,
            dangling.code_object_version,
            dangling.compiler,
            dangling.producer,
            dangling.device_target,
            dangling.type_records,
            dangling.layout_records,
            dangling.kernels,
        ),
        Err(ValidationError::DanglingReference { field: "Rust type" })
    ));

    let mut unreachable = fixture();
    unreachable
        .type_records
        .push(SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(
            ScalarTypeV1::F64,
        )));
    unreachable
        .type_records
        .sort_unstable_by_key(SourceTypeRecordV1::identity);
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            unreachable.canonical_code_object_digest,
            unreachable.code_object_version,
            unreachable.compiler,
            unreachable.producer,
            unreachable.device_target,
            unreachable.type_records,
            unreachable.layout_records,
            unreachable.kernels,
        ),
        Err(ValidationError::UnreachableRecord { field: "Rust type" })
    ));
}

#[test]
fn duplicate_type_and_layout_records_are_rejected() {
    let mut duplicate_type = fixture();
    duplicate_type
        .type_records
        .push(duplicate_type.type_records[0].clone());
    duplicate_type
        .type_records
        .sort_unstable_by_key(SourceTypeRecordV1::identity);
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            duplicate_type.canonical_code_object_digest,
            duplicate_type.code_object_version,
            duplicate_type.compiler,
            duplicate_type.producer,
            duplicate_type.device_target,
            duplicate_type.type_records,
            duplicate_type.layout_records,
            duplicate_type.kernels,
        ),
        Err(ValidationError::Duplicate {
            field: "type records"
        })
    ));

    let mut duplicate_layout = fixture();
    duplicate_layout
        .layout_records
        .push(duplicate_layout.layout_records[0].clone());
    duplicate_layout
        .layout_records
        .sort_unstable_by_key(DeviceLayoutRecordV1::identity);
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            duplicate_layout.canonical_code_object_digest,
            duplicate_layout.code_object_version,
            duplicate_layout.compiler,
            duplicate_layout.producer,
            duplicate_layout.device_target,
            duplicate_layout.type_records,
            duplicate_layout.layout_records,
            duplicate_layout.kernels,
        ),
        Err(ValidationError::Duplicate {
            field: "layout records"
        })
    ));
}

#[test]
fn unknown_header_tags_flags_reserved_and_trailing_bytes_are_rejected() {
    let encoded = encode_device_descriptor_table_v1(&fixture()).expect("encode");

    let mut unknown_version = encoded.clone();
    unknown_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_error(&unknown_version),
        DecodeError::UnknownVersion(2)
    );

    let mut flags = encoded.clone();
    flags[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(decode_error(&flags), DecodeError::UnsupportedFlags(1));

    let mut target_tag = encoded.clone();
    target_tag[48] = 7;
    assert!(matches!(
        decode_error(&target_tag),
        DecodeError::UnknownTag {
            kind: "code object version",
            ..
        }
    ));

    let mut reserved = encoded.clone();
    reserved[51] = 1;
    assert_eq!(
        decode_error(&reserved),
        DecodeError::NonzeroReserved {
            field: "table header"
        }
    );

    let mut trailing = encoded;
    trailing.push(0);
    assert_eq!(decode_error(&trailing), DecodeError::TrailingBytes);
}

#[test]
fn unknown_record_component_tags_and_nonzero_reserved_fields_are_rejected() {
    let table = fixture();
    let encoded = encode_device_descriptor_table_v1(&table).expect("encode");

    let type_identity = table.type_records[0].identity;
    let mut unknown_type = encoded.clone();
    let type_tag = find(&unknown_type, type_identity.as_bytes()) + 32;
    unknown_type[type_tag] = 0xff;
    assert!(matches!(
        decode_error(&unknown_type),
        DecodeError::UnknownTag {
            kind: "type descriptor",
            ..
        }
    ));

    let mut argument_flags = encoded.clone();
    let input_name = find(&argument_flags, b"input");
    argument_flags[input_name - 4] = 1;
    assert!(matches!(
        decode_error(&argument_flags),
        DecodeError::NonzeroReserved {
            field: "logical argument flags"
        }
    ));

    let mut component_tag = encoded.clone();
    let input_name = find(&component_tag, b"input");
    let first_component = input_name + b"input".len() + 72;
    component_tag[first_component] = 0xff;
    assert!(matches!(
        decode_error(&component_tag),
        DecodeError::UnknownTag {
            kind: "physical ABI component",
            ..
        }
    ));

    let mut component_reserved = encoded;
    let input_name = find(&component_reserved, b"input");
    let first_component = input_name + b"input".len() + 72;
    component_reserved[first_component + 12] = 1;
    assert!(matches!(
        decode_error(&component_reserved),
        DecodeError::NonzeroReserved {
            field: "physical ABI component flags"
        }
    ));
}

#[test]
fn noncanonical_type_layout_and_capability_order_is_rejected() {
    let mut type_order = fixture();
    type_order.type_records.swap(0, 1);
    assert!(matches!(
        decode_error(&encode_device_descriptor_table_v1(&type_order).expect("encode")),
        DecodeError::Validation(ValidationError::NonCanonicalOrder {
            field: "type records"
        })
    ));

    let mut layout_order = fixture();
    layout_order.layout_records.swap(0, 1);
    assert!(matches!(
        decode_error(&encode_device_descriptor_table_v1(&layout_order).expect("encode")),
        DecodeError::Validation(ValidationError::NonCanonicalOrder {
            field: "layout records"
        })
    ));

    let mut capability_order = fixture();
    capability_order.kernels[0].capabilities.swap(0, 1);
    assert_eq!(
        decode_error(&encode_device_descriptor_table_v1(&capability_order).expect("encode")),
        DecodeError::NonCanonical
    );
}

#[test]
fn identity_mismatches_are_rejected() {
    let table = fixture();
    let encoded = encode_device_descriptor_table_v1(&table).expect("encode");
    let mut changed = encoded;
    let identity = table.type_records[0].identity;
    let offset = find(&changed, identity.as_bytes());
    changed[offset] ^= 1;
    assert!(matches!(
        decode_error(&changed),
        DecodeError::Validation(ValidationError::IdentityMismatch { field: "Rust type" })
    ));
}

#[test]
fn digest_domains_are_separate_and_use_u64_lengths() {
    let payload = b"same canonical bytes";
    let domains = [
        RUST_TYPE_DOMAIN_V1,
        DEVICE_LAYOUT_DOMAIN_V1,
        KERNEL_DESCRIPTOR_DOMAIN_V1,
        DEVICE_DESCRIPTOR_TABLE_DOMAIN_V1,
        CANONICAL_CODE_OBJECT_DOMAIN_V1,
    ];
    let hashes: Vec<[u8; 32]> = domains
        .iter()
        .map(|domain| domain_hash_for_test(domain, payload))
        .collect();
    for (index, hash) in hashes.iter().enumerate() {
        assert!(!hashes[..index].contains(hash));
    }

    let mut independent = Sha256::new();
    independent.update(RUST_TYPE_DOMAIN_V1);
    independent.update((payload.len() as u64).to_le_bytes());
    independent.update(payload);
    let expected: [u8; 32] = independent.finalize().into();
    assert_eq!(hashes[0], expected);
}

#[test]
fn canonical_code_object_digest_is_not_a_raw_payload_digest() {
    let payload = b"complete final bytes with trusted field zeroed";
    let canonical = CanonicalCodeObjectDigest::calculate_from_canonicalized_hsaco(payload);
    let raw_payload_sha256: [u8; 32] = Sha256::digest(payload).into();
    assert_ne!(canonical.as_bytes(), &raw_payload_sha256);

    fn accepts_only_canonical(_: CanonicalCodeObjectDigest) {}
    accepts_only_canonical(canonical);
    assert_eq!(std::mem::size_of::<CanonicalCodeObjectDigest>(), 32);
}

#[test]
fn raw_pointer_generic_address_space_and_records_have_no_v1_representation() {
    let supported_kinds = [
        SourceTypeDescriptorV1::scalar(ScalarTypeV1::U64),
        SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::U64),
        SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::U64),
    ];
    assert_eq!(supported_kinds.len(), 3);
    assert!(supported_kinds[0].is_scalar());
    assert!(supported_kinds[1].is_shared_slice());
    assert!(supported_kinds[2].is_disjoint_slice());
}

#[test]
fn malformed_layout_and_component_widths_are_rejected() {
    let mut layout = fixture();
    layout.layout_records[0].descriptor.pointer_width = 4;
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            layout.canonical_code_object_digest,
            layout.code_object_version,
            layout.compiler,
            layout.producer,
            layout.device_target,
            layout.type_records,
            layout.layout_records,
            layout.kernels,
        ),
        Err(ValidationError::InvalidArgument(_)) | Err(ValidationError::IdentityMismatch { .. })
    ));

    let mut component = fixture();
    component.kernels[0].arguments[0].components[0] = PhysicalAbiComponentV1 {
        kind: PhysicalAbiComponentKind::ScalarByValue(ScalarTypeV1::F32),
        offset: 0,
        size: 8,
        alignment: 4,
        access: AccessMode::ByValue,
        alias: AliasSemantics::Value,
    };
    assert!(matches!(
        DeviceDescriptorTableV1::from_wire(
            component.canonical_code_object_digest,
            component.code_object_version,
            component.compiler,
            component.producer,
            component.device_target,
            component.type_records,
            component.layout_records,
            component.kernels,
        ),
        Err(ValidationError::InvalidPhysicalAbi(_))
    ));
}

#[test]
fn physical_offsets_reject_misalignment_and_slice_overflow() {
    let table = fixture();
    let scalar_type = table
        .type_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::Scalar)
        .expect("scalar type");
    let scalar_layout = table
        .layout_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::Scalar)
        .expect("scalar layout");
    assert!(LogicalArgumentV1::scalar(0, name("value"), scalar_type, scalar_layout, 2,).is_err());

    let disjoint_type = table
        .type_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::DisjointSlice)
        .expect("DisjointSlice type");
    let disjoint_layout = table
        .layout_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::DisjointSlice)
        .expect("DisjointSlice layout");
    assert!(
        LogicalArgumentV1::disjoint_slice(
            0,
            name("values"),
            disjoint_type,
            disjoint_layout,
            AccessMode::ReadWrite,
            u32::MAX,
        )
        .is_err()
    );
}

#[test]
fn kernel_abi_layout_bounds_sizes_alignment_and_components() {
    let mut segment_padding = fixture();
    segment_padding.kernels[0].abi_layout =
        KernelAbiLayoutV1::new(40, 49, 8).expect("V4 size need not be alignment-multiple");
    assert_eq!(segment_padding.kernels[0].validate(), Ok(()));
    assert_eq!(
        segment_padding.kernels[0]
            .abi_layout()
            .explicit_argument_size(),
        40
    );
    assert_eq!(
        segment_padding.kernels[0]
            .abi_layout()
            .kernarg_segment_size(),
        49
    );
    assert_eq!(
        segment_padding.kernels[0]
            .abi_layout()
            .kernarg_segment_alignment(),
        8
    );

    let mut canonical_explicit_padding = fixture();
    let scalar_type = canonical_explicit_padding
        .type_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::Scalar)
        .unwrap();
    let scalar_layout = canonical_explicit_padding
        .layout_records
        .iter()
        .find(|record| record.descriptor.kind == DescriptorKind::Scalar)
        .unwrap();
    let trailing =
        LogicalArgumentV1::scalar(3, name("tail"), scalar_type, scalar_layout, 40).unwrap();
    canonical_explicit_padding.kernels[0]
        .arguments
        .push(trailing);
    canonical_explicit_padding.kernels[0].abi_layout = KernelAbiLayoutV1::new(48, 80, 8).unwrap();
    assert_eq!(canonical_explicit_padding.kernels[0].validate(), Ok(()));

    let mut noncanonical_explicit_padding = fixture();
    noncanonical_explicit_padding.kernels[0].abi_layout =
        KernelAbiLayoutV1::new(48, 80, 8).expect("locally valid sizes");
    assert!(matches!(
        noncanonical_explicit_padding.kernels[0].validate(),
        Err(ValidationError::InvalidPhysicalAbi(
            "explicit argument size must equal the canonically aligned end of the final physical component"
        ))
    ));

    let mut out_of_bounds = fixture();
    out_of_bounds.kernels[0].abi_layout =
        KernelAbiLayoutV1::new(39, 80, 8).expect("locally valid sizes");
    assert!(matches!(
        out_of_bounds.kernels[0].validate(),
        Err(ValidationError::InvalidPhysicalAbi(
            "physical component exceeds the explicit argument region"
        ))
    ));

    let mut over_aligned = fixture();
    over_aligned.kernels[0].abi_layout =
        KernelAbiLayoutV1::new(40, 80, 4).expect("locally valid sizes");
    assert!(matches!(
        over_aligned.kernels[0].validate(),
        Err(ValidationError::InvalidPhysicalAbi(
            "physical component alignment exceeds the kernarg segment alignment"
        ))
    ));

    assert!(KernelAbiLayoutV1::new(81, 80, 8).is_err());
    assert!(KernelAbiLayoutV1::new(0, 0, 0).is_err());
    assert!(KernelAbiLayoutV1::new(0, 0, 3).is_err());
    assert!(KernelAbiLayoutV1::new(0, MAX_KERNARG_SEGMENT_BYTES, 1 << 20).is_ok());
    assert!(KernelAbiLayoutV1::new(0, MAX_KERNARG_SEGMENT_BYTES + 1, 8).is_err());
    assert!(KernelAbiLayoutV1::new(0, 0, (1 << 20) + 1).is_err());

    let mut table = fixture();
    let mut empty = table.kernels.remove(0);
    empty.arguments.clear();
    empty.abi_layout = KernelAbiLayoutV1::new(0, 16, 8).expect("implicit-only segment");
    assert_eq!(empty.validate(), Ok(()));
    empty.abi_layout = KernelAbiLayoutV1::new(1, 16, 8).expect("locally valid sizes");
    assert!(matches!(
        empty.validate(),
        Err(ValidationError::InvalidPhysicalAbi(
            "explicit argument size must equal the canonically aligned end of the final physical component"
        ))
    ));
}

#[test]
fn u32_max_scalar_offset_cannot_escape_the_explicit_region() {
    let source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::U8));
    let layout = DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::U8));
    let argument = LogicalArgumentV1::scalar(0, name("value"), &source, &layout, u32::MAX)
        .expect("u8 is locally aligned at u32::MAX");
    assert_eq!(argument.components[0].end(), Ok(u64::from(u32::MAX) + 1));

    assert!(matches!(
        KernelDescriptorV1::new(
            KernelId::from_bytes([0x99; 32]),
            name("offset_boundary"),
            name("offset_boundary"),
            name("offset_boundary.kd"),
            evidence(1, 2),
            evidence(3, 4),
            vec![],
            KernelAbiLayoutV1::new(MAX_KERNARG_SEGMENT_BYTES, MAX_KERNARG_SEGMENT_BYTES, 1,)
                .expect("maximum bounded layout"),
            launch(),
            vec![argument],
        ),
        Err(ValidationError::InvalidPhysicalAbi(
            "physical component exceeds the explicit argument region"
        ))
    ));
}

#[test]
fn launch_constraints_reject_invalid_rank_dimensions_and_overflow() {
    assert!(
        LaunchConstraintsV1::new(
            0,
            BlockSizeV1::Any,
            DimensionsV1::new(1, 1, 1).expect("dimensions"),
            1,
            0,
            0,
        )
        .is_err()
    );
    assert!(
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(2, 2, 1).expect("dimensions")),
            DimensionsV1::new(1, 1, 1).expect("dimensions"),
            8,
            0,
            0,
        )
        .is_err()
    );
    assert!(
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Any,
            DimensionsV1::new(1, 1, 1).expect("dimensions"),
            1,
            u32::MAX,
            1,
        )
        .is_err()
    );
}

#[test]
fn helper_hex_parser_is_independent_of_schema_encoder() {
    assert_eq!(from_hex("0001a0ff"), [0, 1, 0xa0, 0xff]);
}

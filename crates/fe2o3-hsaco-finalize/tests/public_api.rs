use std::panic::{AssertUnwindSafe, catch_unwind};

use fe2o3_hsaco::{KernelBindingError, MAX_HSACO_BYTES};
use fe2o3_hsaco_finalize::{
    DEVICE_DESCRIPTOR_SECTION_ALIGNMENT, DEVICE_DESCRIPTOR_SECTION_NAME, FinalizationError,
    finalize_unfinalized, inspect_finalized, inspect_unfinalized, verify_finalized,
};
use fe2o3_kernel_descriptor::{
    AccessMode, BlockSizeV1, BuildEvidenceV1, CANONICAL_CODE_OBJECT_DIGEST_OFFSET,
    CanonicalCodeObjectDigest, CapabilityV1, CodeObjectVersion, CompilerIdentityV1,
    DeviceDescriptorTableV1, DeviceLayoutDescriptorV1, DeviceLayoutRecordV1, DeviceTargetV1,
    DimensionsV1, EvidenceDigest, EvidenceIdentity, KernelAbiLayoutV1, KernelDescriptorV1,
    KernelId, LaunchConstraintsV1, LogicalArgumentV1, MAX_DESCRIPTOR_TABLE_BYTES,
    ProducerIdentityV1, ScalarTypeV1, SourceTypeDescriptorV1, SourceTypeRecordV1, Text, ValidName,
    encode_device_descriptor_table_v1,
};
use rmpv::{Value, encode::write_value};
use sha2::{Digest, Sha256};

const ELF_HEADER_BYTES: usize = 64;
const PROGRAM_HEADER_BYTES: usize = 56;
const SECTION_HEADER_BYTES: usize = 64;
const NOTE_SECTION_INDEX: usize = 1;
const RODATA_SECTION_INDEX: usize = 2;
const TEXT_SECTION_INDEX: usize = 3;
const STRTAB_SECTION_INDEX: usize = 4;
const SYMTAB_SECTION_INDEX: usize = 5;
const DESCRIPTOR_SECTION_INDEX: usize = 6;
const TARGET: &str = "gfx1151";
const GENERAL_V3_TARGET: &str = "gfx942:xnack-";

#[derive(Clone, Copy)]
enum GeneralV3Kernel {
    Alpha,
    Zeta,
}

impl GeneralV3Kernel {
    const fn entry(self) -> &'static str {
        match self {
            Self::Alpha => "alpha",
            Self::Zeta => "zeta",
        }
    }

    const fn descriptor(self) -> &'static str {
        match self {
            Self::Alpha => "alpha.kd",
            Self::Zeta => "zeta.kd",
        }
    }
}

#[derive(Clone, Copy)]
struct KernelSpec<'a> {
    entry: &'a str,
    symbol: &'a str,
    kernarg_size: u32,
    id: u8,
}

#[derive(Clone, Copy)]
struct TableOptions {
    pointer_offset: u32,
    disjoint_access: Option<AccessMode>,
    kernarg_alignment: u32,
    static_group_size: u32,
    max_flat_workgroup_size: u32,
    block_size: BlockSizeV1,
    max_grid: DimensionsV1,
}

impl TableOptions {
    fn standard() -> Self {
        Self {
            pointer_offset: 0,
            disjoint_access: None,
            kernarg_alignment: 8,
            static_group_size: 0,
            max_flat_workgroup_size: 1024,
            block_size: BlockSizeV1::Any,
            max_grid: DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
        }
    }
}

#[derive(Debug)]
struct Fixture {
    bytes: Vec<u8>,
    descriptor_offsets: Vec<usize>,
    descriptor_headers: Vec<usize>,
    extra_headers: Vec<usize>,
    shstr_header: usize,
    kernel_descriptor_offsets: Vec<usize>,
    entry_symbols: Vec<usize>,
    descriptor_symbols: Vec<usize>,
    symtab_header: usize,
}

#[test]
fn round_trip_patches_only_the_digest_slot_and_verifies() {
    let fixture = valid_fixture();
    let before = inspect_unfinalized(&fixture.bytes).unwrap();
    assert_eq!(before.location().offset(), fixture.descriptor_offsets[0]);
    assert_eq!(
        before.location().digest_offset(),
        fixture.descriptor_offsets[0] + 16
    );
    assert!(!before.grants_launch_authority());

    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    let digest_range = before.location().digest_offset()
        ..before.location().digest_offset() + CANONICAL_CODE_OBJECT_DIGEST_OFFSET * 2;
    for (index, (input, output)) in fixture.bytes.iter().zip(finalized.as_bytes()).enumerate() {
        if digest_range.contains(&index) {
            assert_eq!(
                *output,
                finalized.inspection().digest().as_bytes()[index - digest_range.start]
            );
        } else {
            assert_eq!(
                input, output,
                "byte {index} outside the digest slot changed"
            );
        }
    }
    let reparsed = inspect_finalized(finalized.as_bytes()).unwrap();
    assert_eq!(reparsed, *finalized.inspection());
    assert_eq!(verify_finalized(finalized.as_bytes()).unwrap(), reparsed);
    assert!(!reparsed.grants_launch_authority());
    assert!(!finalized.grants_launch_authority());
}

#[test]
fn canonical_digest_matches_an_independently_assembled_domain_hash() {
    let fixture = valid_fixture();
    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(b"FE2O3/AMDHSA-CODE-OBJECT/V1\0");
    hasher.update((fixture.bytes.len() as u64).to_le_bytes());
    hasher.update(&fixture.bytes);
    let expected: [u8; 32] = hasher.finalize().into();
    assert_eq!(finalized.inspection().digest().as_bytes(), &expected);
}

#[test]
fn rejects_wrong_zero_and_finalized_digest_states_without_mutating_input() {
    let fixture = valid_fixture();
    assert_eq!(
        inspect_finalized(&fixture.bytes),
        Err(FinalizationError::ExpectedFinalizedDigest)
    );

    let finalized = finalize_unfinalized(&fixture.bytes).unwrap().into_bytes();
    let snapshot = finalized.clone();
    assert_eq!(
        finalize_unfinalized(&finalized),
        Err(FinalizationError::ExpectedZeroDigest)
    );
    assert_eq!(finalized, snapshot);
}

#[test]
fn every_truncation_is_rejected_and_input_is_never_mutated() {
    let fixture = valid_fixture();
    for length in 0..fixture.bytes.len() {
        let prefix = fixture.bytes[..length].to_vec();
        let snapshot = prefix.clone();
        assert!(
            finalize_unfinalized(&prefix).is_err(),
            "accepted prefix length {length}"
        );
        assert_eq!(prefix, snapshot);
    }
}

#[test]
fn deterministic_mutations_are_panic_free() {
    let fixture = valid_fixture();
    for index in 0..fixture.bytes.len() {
        for mask in [0x01, 0x80, 0xff] {
            let mut mutated = fixture.bytes.clone();
            mutated[index] ^= mask;
            catch_unwind(AssertUnwindSafe(|| finalize_unfinalized(&mutated)))
                .unwrap_or_else(|_| panic!("finalizer panicked at byte {index} mask {mask:#x}"))
                .ok();
        }
    }
}

#[test]
fn enforces_whole_file_and_descriptor_size_bounds() {
    assert_eq!(
        finalize_unfinalized(&vec![0; MAX_HSACO_BYTES + 1]),
        Err(FinalizationError::InputTooLarge)
    );

    let oversized = vec![0; MAX_DESCRIPTOR_TABLE_BYTES + 1];
    let fixture = build_fixture(
        &oversized,
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
    );
    assert_eq!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::DescriptorTableTooLarge)
    );
}

#[test]
fn requires_exactly_one_normative_section_name() {
    let table = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let table_bytes = encode_device_descriptor_table_v1(&table).unwrap();
    let metadata = metadata(&[("vecadd", "vecadd.kd", 272)]);

    let mut missing = build_fixture(&table_bytes, &metadata, 1, &[]);
    write_u32(&mut missing.bytes, missing.descriptor_headers[0], 0);
    assert_eq!(
        finalize_unfinalized(&missing.bytes),
        Err(FinalizationError::MissingDescriptorSection)
    );

    let duplicate = build_fixture(&table_bytes, &metadata, 2, &[]);
    assert_eq!(
        finalize_unfinalized(&duplicate.bytes),
        Err(FinalizationError::DuplicateDescriptorSection)
    );
}

#[test]
fn rejects_bad_section_type_flags_and_alignment() {
    for section_type in [3, 8] {
        let mut fixture = valid_fixture();
        write_u32(
            &mut fixture.bytes,
            fixture.descriptor_headers[0] + 4,
            section_type,
        );
        assert_eq!(
            finalize_unfinalized(&fixture.bytes),
            Err(FinalizationError::InvalidDescriptorSectionType)
        );
    }

    for flags in [1, 2, 4, 0x800, 1 << 63] {
        let mut fixture = valid_fixture();
        write_u64(&mut fixture.bytes, fixture.descriptor_headers[0] + 8, flags);
        assert_eq!(
            finalize_unfinalized(&fixture.bytes),
            Err(FinalizationError::InvalidDescriptorSectionFlags(flags))
        );
    }

    for alignment in [0, 1, 4, 16] {
        let mut fixture = valid_fixture();
        write_u64(
            &mut fixture.bytes,
            fixture.descriptor_headers[0] + 48,
            alignment,
        );
        assert_eq!(
            finalize_unfinalized(&fixture.bytes),
            Err(FinalizationError::InvalidDescriptorSectionAlignment)
        );
    }

    let mut misaligned_offset = valid_fixture();
    write_u64(
        &mut misaligned_offset.bytes,
        misaligned_offset.descriptor_headers[0] + 24,
        (misaligned_offset.descriptor_offsets[0] + 1) as u64,
    );
    assert_eq!(
        finalize_unfinalized(&misaligned_offset.bytes),
        Err(FinalizationError::InvalidDescriptorSectionAlignment)
    );
}

#[test]
fn rejects_bad_section_ranges_and_aliases() {
    let mut out_of_bounds = valid_fixture();
    write_u64(
        &mut out_of_bounds.bytes,
        out_of_bounds.descriptor_headers[0] + 24,
        u64::MAX,
    );
    assert!(matches!(
        finalize_unfinalized(&out_of_bounds.bytes),
        Err(FinalizationError::InvalidElf(_)) | Err(FinalizationError::HsacoBinding(_))
    ));

    let mut header_overlap = valid_fixture();
    write_u64(
        &mut header_overlap.bytes,
        header_overlap.descriptor_headers[0] + 24,
        0,
    );
    assert_eq!(
        finalize_unfinalized(&header_overlap.bytes),
        Err(FinalizationError::DescriptorSectionOverlaps("ELF header"))
    );

    let mut section_table_overlap = valid_fixture();
    let section_table = read_u64(&section_table_overlap.bytes, 40);
    write_u64(
        &mut section_table_overlap.bytes,
        section_table_overlap.descriptor_headers[0] + 24,
        section_table,
    );
    write_u64(
        &mut section_table_overlap.bytes,
        section_table_overlap.descriptor_headers[0] + 32,
        SECTION_HEADER_BYTES as u64,
    );
    assert_eq!(
        finalize_unfinalized(&section_table_overlap.bytes),
        Err(FinalizationError::DescriptorSectionOverlaps(
            "section header table"
        ))
    );

    let mut program_table_overlap = valid_fixture();
    write_u64(
        &mut program_table_overlap.bytes,
        program_table_overlap.descriptor_headers[0] + 24,
        ELF_HEADER_BYTES as u64,
    );
    assert_eq!(
        finalize_unfinalized(&program_table_overlap.bytes),
        Err(FinalizationError::DescriptorSectionOverlaps(
            "program header table"
        ))
    );

    let mut segment_overlap = valid_fixture();
    let third_program_header = ELF_HEADER_BYTES + 2 * PROGRAM_HEADER_BYTES;
    write_u32(&mut segment_overlap.bytes, third_program_header, 1);
    write_u32(&mut segment_overlap.bytes, third_program_header + 4, 4);
    write_u64(
        &mut segment_overlap.bytes,
        third_program_header + 8,
        segment_overlap.descriptor_offsets[0] as u64,
    );
    write_u64(
        &mut segment_overlap.bytes,
        third_program_header + 16,
        (segment_overlap.descriptor_offsets[0] + 0x2000) as u64,
    );
    let segment_size = segment_overlap
        .bytes
        .len()
        .checked_sub(segment_overlap.descriptor_offsets[0])
        .unwrap() as u64;
    write_u64(
        &mut segment_overlap.bytes,
        third_program_header + 32,
        segment_size,
    );
    write_u64(
        &mut segment_overlap.bytes,
        third_program_header + 40,
        segment_size,
    );
    assert_eq!(
        finalize_unfinalized(&segment_overlap.bytes),
        Err(FinalizationError::DescriptorSectionOverlaps(
            "a program segment"
        ))
    );

    let table = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let table_bytes = encode_device_descriptor_table_v1(&table).unwrap();
    let metadata = metadata(&[("vecadd", "vecadd.kd", 272)]);
    let mut alias = build_fixture(&table_bytes, &metadata, 1, &[".alias"]);
    let alias_header = alias.extra_headers[0];
    write_u64(
        &mut alias.bytes,
        alias_header + 24,
        alias.descriptor_offsets[0] as u64,
    );
    write_u64(
        &mut alias.bytes,
        alias_header + 32,
        table_bytes.len() as u64,
    );
    assert_eq!(
        finalize_unfinalized(&alias.bytes),
        Err(FinalizationError::DescriptorSectionOverlaps(
            "another file-backed section"
        ))
    );
}

#[test]
fn rejects_malformed_section_name_tables_and_names() {
    let mut bad_type = valid_fixture();
    write_u32(&mut bad_type.bytes, bad_type.shstr_header + 4, 1);
    assert!(matches!(
        finalize_unfinalized(&bad_type.bytes),
        Err(FinalizationError::InvalidElf(_))
    ));

    let mut bad_name_offset = valid_fixture();
    write_u32(
        &mut bad_name_offset.bytes,
        bad_name_offset.descriptor_headers[0],
        u32::MAX,
    );
    assert!(matches!(
        finalize_unfinalized(&bad_name_offset.bytes),
        Err(FinalizationError::InvalidElf(_))
    ));

    let mut invalid_utf8 = valid_fixture();
    let descriptor_name_offset =
        section_name_file_offset(&invalid_utf8.bytes, DESCRIPTOR_SECTION_INDEX);
    invalid_utf8.bytes[descriptor_name_offset] = 0xff;
    assert!(matches!(
        finalize_unfinalized(&invalid_utf8.bytes),
        Err(FinalizationError::MissingDescriptorSection)
    ));
}

#[test]
fn rejects_bad_and_noncanonical_descriptor_bytes() {
    let mut bad_magic = valid_fixture();
    bad_magic.bytes[bad_magic.descriptor_offsets[0]] ^= 1;
    assert!(matches!(
        finalize_unfinalized(&bad_magic.bytes),
        Err(FinalizationError::DescriptorDecode(_))
    ));

    let mut unsupported_flags = valid_fixture();
    let table_offset = unsupported_flags.descriptor_offsets[0];
    write_u16(&mut unsupported_flags.bytes, table_offset + 10, 1);
    assert!(matches!(
        finalize_unfinalized(&unsupported_flags.bytes),
        Err(FinalizationError::DescriptorDecode(_))
    ));

    let mut trailing_claim = valid_fixture();
    let table_offset = trailing_claim.descriptor_offsets[0];
    let declared = read_u32(&trailing_claim.bytes, table_offset + 12);
    write_u32(&mut trailing_claim.bytes, table_offset + 12, declared - 1);
    assert!(matches!(
        finalize_unfinalized(&trailing_claim.bytes),
        Err(FinalizationError::DescriptorDecode(_))
    ));

    let out_of_order = table(
        &[
            kernel("alpha", "alpha.kd", 272, 1),
            kernel("bravo", "bravo.kd", 272, 2),
        ],
        TARGET,
        CodeObjectVersion::V6,
    );
    let mut encoded = encode_device_descriptor_table_v1(&out_of_order).unwrap();
    let kernel_start = kernel_records_offset(&encoded);
    let record_len = (encoded.len() - kernel_start) / 2;
    assert_eq!((encoded.len() - kernel_start) % 2, 0);
    let first = encoded[kernel_start..kernel_start + record_len].to_vec();
    encoded.copy_within(
        kernel_start + record_len..kernel_start + 2 * record_len,
        kernel_start,
    );
    encoded[kernel_start + record_len..].copy_from_slice(&first);
    let fixture = build_fixture_for_kernels(
        &encoded,
        &metadata(&[("alpha", "alpha.kd", 272), ("bravo", "bravo.kd", 272)]),
        1,
        &[],
        &[("alpha", "alpha.kd"), ("bravo", "bravo.kd")],
        0,
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::DescriptorDecode(_))
    ));
}

#[test]
fn cross_checks_target_and_code_object_version() {
    let metadata = metadata(&[("vecadd", "vecadd.kd", 272)]);
    let target_mismatch = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        "gfx942",
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture(
        &encode_device_descriptor_table_v1(&target_mismatch).unwrap(),
        &metadata,
        1,
        &[],
    );
    assert_eq!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::DeviceTargetMismatch)
    );

    let version_mismatch = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V5,
    );
    let fixture = build_fixture(
        &encode_device_descriptor_table_v1(&version_mismatch).unwrap(),
        &metadata,
        1,
        &[],
    );
    assert_eq!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::CodeObjectVersionMismatch)
    );
}

#[test]
fn cross_checks_complete_kernel_name_and_symbol_closure() {
    let symbol_mismatch_table = table(
        &[kernel("vecadd", "wrong.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture(
        &encode_device_descriptor_table_v1(&symbol_mismatch_table).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernelDescriptorSymbolMismatch { .. })
    ));

    let name_mismatch_table = table(
        &[kernel("other", "other.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture(
        &encode_device_descriptor_table_v1(&name_mismatch_table).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::DescriptorKernelMissingInMetadata { .. })
    ));

    let two = table(
        &[
            kernel("vecadd", "vecadd.kd", 272, 1),
            kernel("extra", "extra.kd", 272, 2),
        ],
        TARGET,
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture(
        &encode_device_descriptor_table_v1(&two).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::DescriptorKernelMissingInMetadata { .. })
    ));

    let one = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture_for_kernels(
        &encode_device_descriptor_table_v1(&one).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272), ("extra", "extra.kd", 272)]),
        1,
        &[],
        &[("vecadd", "vecadd.kd"), ("extra", "extra.kd")],
        0,
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::MetadataKernelMissingInDescriptor { .. })
    ));
}

#[test]
fn cross_checks_complete_kernarg_segment_size() {
    let table = table(
        &[kernel("vecadd", "vecadd.kd", 280, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture(
        &encode_device_descriptor_table_v1(&table).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernargSegmentSizeMismatch { .. })
    ));
}

#[test]
fn reconciles_general_v3_cov6_alpha_and_zeta_explicit_metadata_sizes() {
    for (profile, explicit_size, total_size) in [
        (GeneralV3Kernel::Alpha, 40, 296),
        (GeneralV3Kernel::Zeta, 56, 312),
    ] {
        let fixture = general_v3_fixture(
            profile,
            explicit_size,
            total_size,
            8,
            8,
            CodeObjectVersion::V6,
            "typed-general-gfx942-cov6-v1",
        );
        let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
        let inspection = finalized.inspection();
        let descriptor = &inspection.descriptor_table().kernels()[0];
        let metadata = &inspection.hsaco().kernels()[0];

        assert_eq!(
            descriptor.abi_layout().explicit_argument_size(),
            explicit_size
        );
        assert_eq!(descriptor.abi_layout().kernarg_segment_size(), total_size);
        assert_eq!(descriptor.abi_layout().kernarg_segment_alignment(), 8);
        assert_eq!(metadata.kernarg_segment_size(), u64::from(explicit_size));
        assert_eq!(metadata.implicit_argument_size(), 0);
        verify_finalized(finalized.as_bytes()).unwrap();
    }
}

#[test]
fn general_v3_cov6_reconciliation_rejects_wrong_hidden_span() {
    let fixture = general_v3_fixture(
        GeneralV3Kernel::Alpha,
        40,
        300,
        8,
        8,
        CodeObjectVersion::V6,
        "typed-general-gfx942-cov6-v1",
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernargSegmentSizeMismatch {
            descriptor: 300,
            metadata: 40,
            ..
        })
    ));
}

#[test]
fn general_v3_kernarg_reconciliation_does_not_relax_cov5() {
    let fixture = general_v3_fixture(
        GeneralV3Kernel::Alpha,
        40,
        296,
        8,
        8,
        CodeObjectVersion::V5,
        "typed-general-gfx942-cov6-v1",
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernargSegmentSizeMismatch { .. })
    ));
}

#[test]
fn general_v3_cov6_reconciliation_preserves_alignment_check() {
    let fixture = general_v3_fixture(
        GeneralV3Kernel::Alpha,
        40,
        296,
        16,
        8,
        CodeObjectVersion::V6,
        "typed-general-gfx942-cov6-v1",
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernargSegmentAlignmentMismatch {
            descriptor: 16,
            metadata: 8,
            ..
        })
    ));
}

#[test]
fn general_v3_cov6_reconciliation_rejects_profile_substitution() {
    let fixture = general_v3_fixture(
        GeneralV3Kernel::Alpha,
        40,
        296,
        8,
        8,
        CodeObjectVersion::V6,
        "typed-vecadd-gfx942-cov6-v1",
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernargSegmentSizeMismatch { .. })
    ));
}

#[test]
fn general_v3_cov6_reconciliation_rejects_target_substitution() {
    let fixture = general_v3_fixture_for_target(
        GeneralV3Kernel::Alpha,
        40,
        296,
        8,
        8,
        CodeObjectVersion::V6,
        "typed-general-gfx942-cov6-v1",
        "gfx942",
    );
    assert!(matches!(
        finalize_unfinalized(&fixture.bytes),
        Err(FinalizationError::KernargSegmentSizeMismatch { .. })
    ));
}

#[test]
fn requires_real_function_object_and_descriptor_bindings() {
    let fixture = valid_fixture();
    let inspected = inspect_unfinalized(&fixture.bytes).unwrap();
    assert_eq!(inspected.kernel_bindings().bindings().len(), 1);

    let mut missing_descriptor = valid_fixture();
    write_u32(
        &mut missing_descriptor.bytes,
        missing_descriptor.descriptor_symbols[0],
        0,
    );
    assert!(matches!(
        finalize_unfinalized(&missing_descriptor.bytes),
        Err(FinalizationError::HsacoBinding(
            KernelBindingError::MissingDescriptorSymbol
        ))
    ));
    assert!(matches!(
        inspect_unfinalized(&missing_descriptor.bytes),
        Err(FinalizationError::HsacoBinding(
            KernelBindingError::MissingDescriptorSymbol
        ))
    ));

    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    let mut finalized_without_descriptor_symbol = finalized.into_bytes();
    write_u32(
        &mut finalized_without_descriptor_symbol,
        fixture.descriptor_symbols[0],
        0,
    );
    for result in [
        inspect_finalized(&finalized_without_descriptor_symbol),
        verify_finalized(&finalized_without_descriptor_symbol),
    ] {
        assert!(matches!(
            result,
            Err(FinalizationError::HsacoBinding(
                KernelBindingError::MissingDescriptorSymbol
            ))
        ));
    }

    let mut mistyped_descriptor = valid_fixture();
    mistyped_descriptor.bytes[mistyped_descriptor.descriptor_symbols[0] + 4] = 0x12;
    assert!(matches!(
        finalize_unfinalized(&mistyped_descriptor.bytes),
        Err(FinalizationError::HsacoBinding(
            KernelBindingError::InvalidDescriptorSymbol(_)
        ))
    ));

    let mut mistyped_entry = valid_fixture();
    mistyped_entry.bytes[mistyped_entry.entry_symbols[0] + 4] = 0x11;
    assert!(matches!(
        finalize_unfinalized(&mistyped_entry.bytes),
        Err(FinalizationError::HsacoBinding(
            KernelBindingError::InvalidEntrySymbol(_)
        ))
    ));

    let mut corrupt_descriptor = valid_fixture();
    corrupt_descriptor.bytes[corrupt_descriptor.kernel_descriptor_offsets[0] + 12] = 1;
    assert!(matches!(
        finalize_unfinalized(&corrupt_descriptor.bytes),
        Err(FinalizationError::HsacoBinding(
            KernelBindingError::InvalidKernelDescriptor(_)
        ))
    ));

    let mut missing_symtab = valid_fixture();
    write_u32(
        &mut missing_symtab.bytes,
        missing_symtab.symtab_header + 4,
        1,
    );
    assert!(matches!(
        finalize_unfinalized(&missing_symtab.bytes),
        Err(FinalizationError::HsacoBinding(
            KernelBindingError::InvalidSymbolTable(_)
        ))
    ));
}

#[test]
fn cross_checks_kernarg_alignment_independently_of_size() {
    let mut alignment = metadata_kernel("vecadd", "vecadd.kd", 272);
    set_field(&mut alignment, ".kernarg_segment_align", Value::from(16));
    assert!(matches!(
        finalize_unfinalized(&fixture_for_kernel(alignment, TableOptions::standard()).bytes),
        Err(FinalizationError::KernargSegmentAlignmentMismatch { .. })
    ));
}

#[test]
fn cross_checks_each_mapped_explicit_argument_fact() {
    let mut count = metadata_kernel("vecadd", "vecadd.kd", 272);
    let mut packed = vec![argument(Some("packed"), 0, 16, "by_value", None)];
    packed.extend(v5_hidden_arguments(16));
    set_field(&mut count, ".args", Value::Array(packed));
    assert!(matches!(
        finalize_unfinalized(&fixture_for_kernel(count, TableOptions::standard()).bytes),
        Err(FinalizationError::ExplicitArgumentCountMismatch { .. })
    ));

    let offset_options = TableOptions {
        pointer_offset: 8,
        ..TableOptions::standard()
    };
    assert_physical_mismatch(
        metadata_kernel("vecadd", "vecadd.kd", 272),
        offset_options,
        ".offset",
    );

    let mut size = metadata_kernel("vecadd", "vecadd.kd", 272);
    set_field(&mut arguments_mut(&mut size)[0], ".size", Value::from(4));
    assert_physical_mismatch(size, TableOptions::standard(), ".size");

    let mut order = metadata_kernel("vecadd", "vecadd.kd", 272);
    set_field(
        &mut arguments_mut(&mut order)[0],
        ".value_kind",
        Value::from("by_value"),
    );
    remove_field(&mut arguments_mut(&mut order)[0], ".address_space");
    set_field(
        &mut arguments_mut(&mut order)[1],
        ".value_kind",
        Value::from("global_buffer"),
    );
    map_mut(&mut arguments_mut(&mut order)[1])
        .push((Value::from(".address_space"), Value::from("global")));
    assert_physical_mismatch(order, TableOptions::standard(), ".value_kind");

    let mut value_kind = metadata_kernel("vecadd", "vecadd.kd", 272);
    set_field(
        &mut arguments_mut(&mut value_kind)[1],
        ".value_kind",
        Value::from("global_buffer"),
    );
    map_mut(&mut arguments_mut(&mut value_kind)[1])
        .push((Value::from(".address_space"), Value::from("global")));
    assert_physical_mismatch(value_kind, TableOptions::standard(), ".value_kind");

    let mut address_space = metadata_kernel("vecadd", "vecadd.kd", 272);
    set_field(
        &mut arguments_mut(&mut address_space)[0],
        ".address_space",
        Value::from("constant"),
    );
    assert_physical_mismatch(address_space, TableOptions::standard(), ".address_space");

    let mut access = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut access)[0])
        .push((Value::from(".access"), Value::from("write_only")));
    assert_physical_mismatch(access, TableOptions::standard(), ".access");

    let mut argument_alignment = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut argument_alignment)[0])
        .push((Value::from(".align"), Value::from(4)));
    assert_physical_mismatch(argument_alignment, TableOptions::standard(), ".align");

    let mut pointee_alignment = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut pointee_alignment)[0])
        .push((Value::from(".pointee_align"), Value::from(8)));
    assert_physical_mismatch(
        pointee_alignment,
        TableOptions::standard(),
        ".pointee_align",
    );

    let mut matching_pointee_alignment = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut matching_pointee_alignment)[0])
        .push((Value::from(".pointee_align"), Value::from(4)));
    finalize_unfinalized(
        &fixture_for_kernel(matching_pointee_alignment, TableOptions::standard()).bytes,
    )
    .unwrap();

    let mut matching_value_types = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut matching_value_types)[0])
        .push((Value::from(".value_type"), Value::from("f32")));
    map_mut(&mut arguments_mut(&mut matching_value_types)[1])
        .push((Value::from(".value_type"), Value::from("u64")));
    finalize_unfinalized(&fixture_for_kernel(matching_value_types, TableOptions::standard()).bytes)
        .unwrap();

    for (index, value_type) in [
        (0, "i32"),
        (0, "f64"),
        (1, "i64"),
        (1, "u32"),
        (1, "struct"),
    ] {
        let mut contradictory = metadata_kernel("vecadd", "vecadd.kd", 272);
        map_mut(&mut arguments_mut(&mut contradictory)[index])
            .push((Value::from(".value_type"), Value::from(value_type)));
        assert_physical_mismatch(contradictory, TableOptions::standard(), ".value_type");
    }

    let mut restrict = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut restrict)[0])
        .push((Value::from(".is_restrict"), Value::from(true)));
    assert_physical_mismatch(restrict, TableOptions::standard(), ".is_restrict");

    let mut is_const = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut arguments_mut(&mut is_const)[0])
        .push((Value::from(".is_const"), Value::from(false)));
    assert_physical_mismatch(is_const, TableOptions::standard(), ".is_const");

    for field in [".is_volatile", ".is_pipe"] {
        let mut unsupported = metadata_kernel("vecadd", "vecadd.kd", 272);
        map_mut(&mut arguments_mut(&mut unsupported)[0])
            .push((Value::from(field), Value::from(true)));
        assert_physical_mismatch(unsupported, TableOptions::standard(), field);
    }
}

#[test]
fn declared_and_actual_access_follow_contract_semantics() {
    let access_modes = [
        AccessMode::ReadOnly,
        AccessMode::WriteOnly,
        AccessMode::ReadWrite,
    ];

    for contract in access_modes {
        let options = TableOptions {
            disjoint_access: Some(contract),
            ..TableOptions::standard()
        };

        finalize_unfinalized(
            &fixture_for_kernel(metadata_kernel("vecadd", "vecadd.kd", 272), options).bytes,
        )
        .unwrap_or_else(|error| {
            panic!("absent access evidence rejected for {contract:?}: {error:?}")
        });

        for actual in access_modes {
            let mut metadata = metadata_kernel("vecadd", "vecadd.kd", 272);
            map_mut(&mut arguments_mut(&mut metadata)[0]).push((
                Value::from(".actual_access"),
                Value::from(metadata_access_name(actual)),
            ));
            let result = finalize_unfinalized(&fixture_for_kernel(metadata, options).bytes);
            let expected_ok = match contract {
                AccessMode::ReadOnly => actual == AccessMode::ReadOnly,
                AccessMode::WriteOnly => actual == AccessMode::WriteOnly,
                AccessMode::ReadWrite => true,
                AccessMode::ByValue => unreachable!(),
            };
            if expected_ok {
                result.unwrap_or_else(|error| {
                    panic!("actual {actual:?} rejected for contract {contract:?}: {error:?}")
                });
            } else {
                assert_physical_error_field(
                    result.unwrap_err(),
                    ".actual_access",
                    contract,
                    actual,
                );
            }
        }

        for declared in access_modes {
            let mut metadata = metadata_kernel("vecadd", "vecadd.kd", 272);
            map_mut(&mut arguments_mut(&mut metadata)[0]).push((
                Value::from(".access"),
                Value::from(metadata_access_name(declared)),
            ));
            let result = finalize_unfinalized(&fixture_for_kernel(metadata, options).bytes);
            if declared == contract {
                result.unwrap_or_else(|error| {
                    panic!("declared {declared:?} rejected for contract {contract:?}: {error:?}")
                });
            } else {
                assert_physical_error_field(result.unwrap_err(), ".access", contract, declared);
            }
        }
    }
}

#[test]
fn cross_checks_shared_memory_and_launch_constraints() {
    let static_group = TableOptions {
        static_group_size: 4,
        ..TableOptions::standard()
    };
    assert!(matches!(
        finalize_unfinalized(
            &fixture_for_kernel(metadata_kernel("vecadd", "vecadd.kd", 272), static_group,).bytes
        ),
        Err(FinalizationError::StaticGroupSegmentSizeMismatch { .. })
    ));

    let max_flat = TableOptions {
        max_flat_workgroup_size: 512,
        ..TableOptions::standard()
    };
    assert!(matches!(
        finalize_unfinalized(
            &fixture_for_kernel(metadata_kernel("vecadd", "vecadd.kd", 272), max_flat).bytes
        ),
        Err(FinalizationError::MaxFlatWorkgroupSizeMismatch { .. })
    ));

    let exact_block = TableOptions {
        block_size: BlockSizeV1::Exact(DimensionsV1::new(64, 1, 1).unwrap()),
        ..TableOptions::standard()
    };
    assert!(matches!(
        finalize_unfinalized(
            &fixture_for_kernel(metadata_kernel("vecadd", "vecadd.kd", 272), exact_block,).bytes
        ),
        Err(FinalizationError::RequiredWorkgroupSizeMismatch { .. })
    ));

    let mut max_workgroups = metadata_kernel("vecadd", "vecadd.kd", 272);
    map_mut(&mut max_workgroups).push((Value::from(".max_num_workgroups_x"), Value::from(7)));
    assert!(matches!(
        finalize_unfinalized(&fixture_for_kernel(max_workgroups, TableOptions::standard()).bytes),
        Err(FinalizationError::MaxWorkgroupsMismatch { .. })
    ));
}

#[test]
fn hidden_runtime_arguments_are_excluded_from_the_explicit_table_mapping() {
    let fixture = valid_fixture();
    let inspection = inspect_unfinalized(&fixture.bytes).unwrap();
    let kernel = &inspection.hsaco().kernels()[0];
    assert_eq!(kernel.explicit_arguments().len(), 2);
    assert_eq!(kernel.hidden_arguments().len(), 13);
    assert_eq!(
        inspection.descriptor_table().kernels()[0]
            .arguments()
            .iter()
            .map(|argument| argument.physical_components().len())
            .sum::<usize>(),
        2
    );
    finalize_unfinalized(&fixture.bytes).unwrap();
}

#[test]
fn fixed_name_lookup_is_bounded_for_large_shared_string_tables() {
    let descriptor = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    let fixture = build_fixture_for_kernels(
        &encode_device_descriptor_table_v1(&descriptor).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
        &[("vecadd", "vecadd.kd")],
        200,
    );
    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    assert_eq!(
        finalized.inspection().location().offset(),
        fixture.descriptor_offsets[0]
    );
}

#[test]
fn finalized_digest_detects_tampering_inside_and_outside_the_slot() {
    let fixture = valid_fixture();
    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    let digest_offset = finalized.inspection().location().digest_offset();

    let mut slot_tamper = finalized.as_bytes().to_vec();
    slot_tamper[digest_offset] ^= 1;
    assert!(matches!(
        inspect_finalized(&slot_tamper),
        Err(FinalizationError::CanonicalDigestMismatch { .. })
    ));

    let mut elsewhere = finalized.as_bytes().to_vec();
    let section_table = read_u64(&elsewhere, 40) as usize;
    elsewhere[section_table - 1] ^= 1;
    assert!(matches!(
        inspect_finalized(&elsewhere),
        Err(FinalizationError::CanonicalDigestMismatch { .. })
    ));
}

#[test]
fn public_results_explicitly_deny_authority() {
    let fixture = valid_fixture();
    let unfinalized = inspect_unfinalized(&fixture.bytes).unwrap();
    let finalized = finalize_unfinalized(&fixture.bytes).unwrap();
    assert!(!unfinalized.grants_launch_authority());
    assert!(!finalized.inspection().grants_launch_authority());
    assert!(!finalized.grants_launch_authority());
}

fn valid_fixture() -> Fixture {
    let descriptor = table(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
    );
    build_fixture(
        &encode_device_descriptor_table_v1(&descriptor).unwrap(),
        &metadata(&[("vecadd", "vecadd.kd", 272)]),
        1,
        &[],
    )
}

fn fixture_for_kernel(kernel_metadata: Value, options: TableOptions) -> Fixture {
    let descriptor = table_with_options(
        &[kernel("vecadd", "vecadd.kd", 272, 1)],
        TARGET,
        CodeObjectVersion::V6,
        options,
    );
    build_fixture(
        &encode_device_descriptor_table_v1(&descriptor).unwrap(),
        &metadata_from_values(vec![kernel_metadata]),
        1,
        &[],
    )
}

fn assert_physical_mismatch(
    kernel_metadata: Value,
    options: TableOptions,
    expected_field: &'static str,
) {
    let fixture = fixture_for_kernel(kernel_metadata, options);
    match finalize_unfinalized(&fixture.bytes).unwrap_err() {
        FinalizationError::PhysicalArgumentMismatch { field, .. } => {
            assert_eq!(field, expected_field)
        }
        error => panic!("expected physical argument mismatch, found {error:?}"),
    }
}

fn assert_physical_error_field(
    error: FinalizationError,
    expected_field: &'static str,
    contract: AccessMode,
    metadata_access: AccessMode,
) {
    match error {
        FinalizationError::PhysicalArgumentMismatch { field, .. } => {
            assert_eq!(
                field, expected_field,
                "contract {contract:?}, metadata access {metadata_access:?}"
            );
        }
        error => panic!(
            "expected {expected_field} mismatch for contract {contract:?} and metadata access \
             {metadata_access:?}, found {error:?}"
        ),
    }
}

fn metadata_access_name(access: AccessMode) -> &'static str {
    match access {
        AccessMode::ReadOnly => "read_only",
        AccessMode::WriteOnly => "write_only",
        AccessMode::ReadWrite => "read_write",
        AccessMode::ByValue => panic!("by-value is not a memory access contract"),
    }
}

fn kernel<'a>(entry: &'a str, symbol: &'a str, kernarg_size: u32, id: u8) -> KernelSpec<'a> {
    KernelSpec {
        entry,
        symbol,
        kernarg_size,
        id,
    }
}

fn table(
    kernels: &[KernelSpec<'_>],
    target: &str,
    version: CodeObjectVersion,
) -> DeviceDescriptorTableV1 {
    table_with_options(kernels, target, version, TableOptions::standard())
}

#[allow(clippy::too_many_arguments)]
fn general_v3_fixture(
    profile: GeneralV3Kernel,
    explicit_size: u32,
    total_size: u32,
    descriptor_alignment: u32,
    metadata_alignment: u32,
    version: CodeObjectVersion,
    producer_version: &str,
) -> Fixture {
    general_v3_fixture_for_target(
        profile,
        explicit_size,
        total_size,
        descriptor_alignment,
        metadata_alignment,
        version,
        producer_version,
        GENERAL_V3_TARGET,
    )
}

#[allow(clippy::too_many_arguments)]
fn general_v3_fixture_for_target(
    profile: GeneralV3Kernel,
    explicit_size: u32,
    total_size: u32,
    descriptor_alignment: u32,
    metadata_alignment: u32,
    version: CodeObjectVersion,
    producer_version: &str,
    target: &str,
) -> Fixture {
    let table = general_v3_table(
        profile,
        explicit_size,
        total_size,
        descriptor_alignment,
        version,
        producer_version,
        target,
    );
    let metadata = general_v3_metadata(profile, explicit_size, metadata_alignment, target);
    let mut fixture = build_fixture_for_kernels(
        &encode_device_descriptor_table_v1(&table).unwrap(),
        &metadata,
        1,
        &[],
        &[(profile.entry(), profile.descriptor())],
        0,
    );

    fixture.bytes[8] = match version {
        CodeObjectVersion::V4 => 2,
        CodeObjectVersion::V5 => 3,
        CodeObjectVersion::V6 => 4,
    };
    write_u32(
        &mut fixture.bytes,
        48,
        match target {
            "gfx942" => 0x54c,
            "gfx942:xnack-" => 0x64c,
            _ => panic!("unsupported general V3 test target"),
        },
    );
    for descriptor_offset in fixture.kernel_descriptor_offsets.iter().copied() {
        write_u32(&mut fixture.bytes, descriptor_offset + 8, explicit_size);
        write_u32(&mut fixture.bytes, descriptor_offset + 44, 1);
        write_u32(&mut fixture.bytes, descriptor_offset + 48, 0x00af_0081);
        write_u16(&mut fixture.bytes, descriptor_offset + 56, 0x001e);
    }
    fixture
}

fn general_v3_table(
    profile: GeneralV3Kernel,
    explicit_size: u32,
    total_size: u32,
    kernarg_alignment: u32,
    version: CodeObjectVersion,
    producer_version: &str,
    target: &str,
) -> DeviceDescriptorTableV1 {
    let scalar_source = SourceTypeRecordV1::new(SourceTypeDescriptorV1::scalar(ScalarTypeV1::F32));
    let scalar_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::scalar(ScalarTypeV1::F32));
    let shared_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let shared_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32));
    let disjoint_source =
        SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32));
    let disjoint_layout =
        DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32));

    let arguments = match profile {
        GeneralV3Kernel::Alpha => vec![
            LogicalArgumentV1::scalar(0, name("scale"), &scalar_source, &scalar_layout, 0).unwrap(),
            LogicalArgumentV1::shared_slice(1, name("input"), &shared_source, &shared_layout, 8)
                .unwrap(),
            LogicalArgumentV1::disjoint_slice(
                2,
                name("output"),
                &disjoint_source,
                &disjoint_layout,
                AccessMode::ReadWrite,
                24,
            )
            .unwrap(),
        ],
        GeneralV3Kernel::Zeta => vec![
            LogicalArgumentV1::shared_slice(0, name("left"), &shared_source, &shared_layout, 0)
                .unwrap(),
            LogicalArgumentV1::shared_slice(1, name("right"), &shared_source, &shared_layout, 16)
                .unwrap(),
            LogicalArgumentV1::scalar(2, name("bias"), &scalar_source, &scalar_layout, 32).unwrap(),
            LogicalArgumentV1::disjoint_slice(
                3,
                name("output"),
                &disjoint_source,
                &disjoint_layout,
                AccessMode::ReadWrite,
                40,
            )
            .unwrap(),
        ],
    };
    let kernel = KernelDescriptorV1::new(
        KernelId::from_bytes(
            [match profile {
                GeneralV3Kernel::Alpha => 0xa1,
                GeneralV3Kernel::Zeta => 0xb2,
            }; 32],
        ),
        name(profile.entry()),
        name(profile.entry()),
        name(profile.descriptor()),
        evidence(0x31, 0x32),
        evidence(0x33, 0x34),
        vec![CapabilityV1::AmdWave],
        KernelAbiLayoutV1::new(explicit_size, total_size, kernarg_alignment).unwrap(),
        LaunchConstraintsV1::new(
            1,
            BlockSizeV1::Exact(DimensionsV1::new(256, 1, 1).unwrap()),
            DimensionsV1::new(u32::MAX, 1, 1).unwrap(),
            256,
            0,
            0,
        )
        .unwrap(),
        arguments,
    )
    .unwrap();
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        version,
        CompilerIdentityV1::new(text("rustc-codegen-fe2o3"), text("test"), [0x21; 20]),
        ProducerIdentityV1::new(
            text("rustc-codegen-fe2o3-worker-v2"),
            text(producer_version),
        ),
        DeviceTargetV1::parse(target).unwrap(),
        vec![scalar_source, shared_source, disjoint_source],
        vec![scalar_layout, shared_layout, disjoint_layout],
        vec![kernel],
    )
    .unwrap()
}

fn table_with_options(
    kernels: &[KernelSpec<'_>],
    target: &str,
    version: CodeObjectVersion,
    options: TableOptions,
) -> DeviceDescriptorTableV1 {
    let (source, layout) = if options.disjoint_access.is_some() {
        (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::disjoint_slice(ScalarTypeV1::F32)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::disjoint_slice(ScalarTypeV1::F32)),
        )
    } else {
        (
            SourceTypeRecordV1::new(SourceTypeDescriptorV1::shared_slice(ScalarTypeV1::F32)),
            DeviceLayoutRecordV1::new(DeviceLayoutDescriptorV1::shared_slice(ScalarTypeV1::F32)),
        )
    };
    let kernels = kernels
        .iter()
        .map(|spec| {
            KernelDescriptorV1::new(
                KernelId::from_bytes([spec.id; 32]),
                name(spec.entry),
                name(spec.entry),
                name(spec.symbol),
                evidence(0x11, 0x12),
                evidence(0x13, 0x14),
                Vec::new(),
                KernelAbiLayoutV1::new(
                    options.pointer_offset + 16,
                    spec.kernarg_size,
                    options.kernarg_alignment,
                )
                .unwrap(),
                LaunchConstraintsV1::new(
                    1,
                    options.block_size,
                    options.max_grid,
                    options.max_flat_workgroup_size,
                    options.static_group_size,
                    64 * 1024,
                )
                .unwrap(),
                vec![match options.disjoint_access {
                    Some(access) => LogicalArgumentV1::disjoint_slice(
                        0,
                        name("values"),
                        &source,
                        &layout,
                        access,
                        options.pointer_offset,
                    )
                    .unwrap(),
                    None => LogicalArgumentV1::shared_slice(
                        0,
                        name("values"),
                        &source,
                        &layout,
                        options.pointer_offset,
                    )
                    .unwrap(),
                }],
            )
            .unwrap()
        })
        .collect();
    DeviceDescriptorTableV1::new(
        CanonicalCodeObjectDigest::from_bytes([0; 32]),
        version,
        CompilerIdentityV1::new(text("rustc"), text("test"), [0x21; 20]),
        ProducerIdentityV1::new(text("cargo-fe2o3"), text("test")),
        DeviceTargetV1::parse(target).unwrap(),
        vec![source],
        vec![layout],
        kernels,
    )
    .unwrap()
}

fn name(value: &str) -> ValidName {
    ValidName::new(value).unwrap()
}

fn text(value: &str) -> Text {
    Text::new(value).unwrap()
}

fn evidence(identity: u8, digest: u8) -> BuildEvidenceV1 {
    BuildEvidenceV1::new(
        EvidenceIdentity::from_opaque_bytes([identity; 32]),
        EvidenceDigest::from_sha256_bytes([digest; 32]),
    )
}

fn metadata(kernels: &[(&str, &str, u32)]) -> Vec<u8> {
    metadata_from_values(
        kernels
            .iter()
            .map(|(entry, symbol, size)| metadata_kernel(entry, symbol, *size))
            .collect(),
    )
}

fn general_v3_metadata(
    profile: GeneralV3Kernel,
    explicit_size: u32,
    kernarg_alignment: u32,
    target: &str,
) -> Vec<u8> {
    let arguments = match profile {
        GeneralV3Kernel::Alpha => vec![
            argument(Some("scale"), 0, 4, "by_value", None),
            argument(Some("input_ptr"), 8, 8, "global_buffer", Some("global")),
            argument(Some("input_len"), 16, 8, "by_value", None),
            argument(Some("output_ptr"), 24, 8, "global_buffer", Some("global")),
            argument(Some("output_len"), 32, 8, "by_value", None),
        ],
        GeneralV3Kernel::Zeta => vec![
            argument(Some("left_ptr"), 0, 8, "global_buffer", Some("global")),
            argument(Some("left_len"), 8, 8, "by_value", None),
            argument(Some("right_ptr"), 16, 8, "global_buffer", Some("global")),
            argument(Some("right_len"), 24, 8, "by_value", None),
            argument(Some("bias"), 32, 4, "by_value", None),
            argument(Some("output_ptr"), 40, 8, "global_buffer", Some("global")),
            argument(Some("output_len"), 48, 8, "by_value", None),
        ],
    };
    let kernel = Value::Map(vec![
        (Value::from(".name"), Value::from(profile.entry())),
        (Value::from(".symbol"), Value::from(profile.descriptor())),
        (Value::from(".args"), Value::Array(arguments)),
        (
            Value::from(".kernarg_segment_size"),
            Value::from(explicit_size),
        ),
        (
            Value::from(".kernarg_segment_align"),
            Value::from(kernarg_alignment),
        ),
        (Value::from(".group_segment_fixed_size"), Value::from(0)),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (Value::from(".wavefront_size"), Value::from(64)),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(11)),
        (Value::from(".agpr_count"), Value::from(3)),
        (Value::from(".sgpr_spill_count"), Value::from(2)),
        (Value::from(".vgpr_spill_count"), Value::from(4)),
        (Value::from(".max_flat_workgroup_size"), Value::from(256)),
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(vec![Value::from(256), Value::from(1), Value::from(1)]),
        ),
    ]);
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from(format!("amdgcn-amd-amdhsa--{target}")),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(vec![kernel])),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
}

fn metadata_from_values(kernels: Vec<Value>) -> Vec<u8> {
    let root = Value::Map(vec![
        (
            Value::from("amdhsa.version"),
            Value::Array(vec![Value::from(1), Value::from(2)]),
        ),
        (
            Value::from("amdhsa.target"),
            Value::from("amdgcn-amd-amdhsa--gfx1151"),
        ),
        (Value::from("amdhsa.kernels"), Value::Array(kernels)),
    ]);
    let mut encoded = Vec::new();
    write_value(&mut encoded, &root).unwrap();
    encoded
}

fn metadata_kernel(entry: &str, symbol: &str, kernarg_size: u32) -> Value {
    let mut arguments = vec![
        argument(Some("values_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("values_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(v5_hidden_arguments(16));
    Value::Map(vec![
        (Value::from(".name"), Value::from(entry)),
        (Value::from(".symbol"), Value::from(symbol)),
        (Value::from(".args"), Value::Array(arguments)),
        (
            Value::from(".kernarg_segment_size"),
            Value::from(kernarg_size),
        ),
        (Value::from(".kernarg_segment_align"), Value::from(8)),
        (Value::from(".group_segment_fixed_size"), Value::from(0)),
        (Value::from(".private_segment_fixed_size"), Value::from(0)),
        (Value::from(".wavefront_size"), Value::from(32)),
        (Value::from(".sgpr_count"), Value::from(14)),
        (Value::from(".vgpr_count"), Value::from(7)),
        (Value::from(".workgroup_processor_mode"), Value::from(1)),
        (Value::from(".max_flat_workgroup_size"), Value::from(1024)),
    ])
}

fn v5_hidden_arguments(base: u64) -> Vec<Value> {
    [
        (0, 4, "hidden_block_count_x"),
        (4, 4, "hidden_block_count_y"),
        (8, 4, "hidden_block_count_z"),
        (12, 2, "hidden_group_size_x"),
        (14, 2, "hidden_group_size_y"),
        (16, 2, "hidden_group_size_z"),
        (18, 2, "hidden_remainder_x"),
        (20, 2, "hidden_remainder_y"),
        (22, 2, "hidden_remainder_z"),
        (40, 8, "hidden_global_offset_x"),
        (48, 8, "hidden_global_offset_y"),
        (56, 8, "hidden_global_offset_z"),
        (64, 2, "hidden_grid_dims"),
    ]
    .into_iter()
    .map(|(offset, size, kind)| argument(None, base + offset, size, kind, None))
    .collect()
}

fn argument(
    name: Option<&str>,
    offset: u64,
    size: u64,
    value_kind: &str,
    address_space: Option<&str>,
) -> Value {
    let mut fields = vec![
        (Value::from(".offset"), Value::from(offset)),
        (Value::from(".size"), Value::from(size)),
        (Value::from(".value_kind"), Value::from(value_kind)),
    ];
    if let Some(name) = name {
        fields.push((Value::from(".name"), Value::from(name)));
    }
    if let Some(address_space) = address_space {
        fields.push((Value::from(".address_space"), Value::from(address_space)));
    }
    Value::Map(fields)
}

fn map_mut(value: &mut Value) -> &mut Vec<(Value, Value)> {
    match value {
        Value::Map(fields) => fields,
        _ => panic!("expected metadata map"),
    }
}

fn field_mut<'a>(value: &'a mut Value, name: &str) -> &'a mut Value {
    &mut map_mut(value)
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some(name))
        .unwrap()
        .1
}

fn set_field(value: &mut Value, name: &str, replacement: Value) {
    *field_mut(value, name) = replacement;
}

fn remove_field(value: &mut Value, name: &str) {
    map_mut(value).retain(|(key, _)| key.as_str() != Some(name));
}

fn arguments_mut(kernel: &mut Value) -> &mut Vec<Value> {
    match field_mut(kernel, ".args") {
        Value::Array(arguments) => arguments,
        _ => panic!("expected argument array"),
    }
}

fn build_fixture(
    table: &[u8],
    metadata: &[u8],
    descriptor_count: usize,
    extra_names: &[&str],
) -> Fixture {
    build_fixture_for_kernels(
        table,
        metadata,
        descriptor_count,
        extra_names,
        &[("vecadd", "vecadd.kd")],
        0,
    )
}

fn build_fixture_for_kernels(
    table: &[u8],
    metadata: &[u8],
    descriptor_count: usize,
    extra_names: &[&str],
    kernels: &[(&str, &str)],
    hostile_name_sections: usize,
) -> Fixture {
    const PROGRAM_COUNT: usize = 3;

    let note = metadata_note(metadata);
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);

    align(&mut bytes, 64);
    let rodata_offset = bytes.len();
    let mut kernel_descriptor_offsets = Vec::new();
    for _ in kernels {
        align(&mut bytes, 64);
        kernel_descriptor_offsets.push(bytes.len());
        bytes.resize(bytes.len() + 64, 0);
    }
    let rodata_end = bytes.len();

    let mut entry_offsets = Vec::new();
    for _ in kernels {
        align(&mut bytes, 256);
        entry_offsets.push(bytes.len());
        bytes.resize(bytes.len() + 64, 0xbf);
    }
    let text_offset = *entry_offsets.first().unwrap();
    let text_end = bytes.len();

    let mut strtab = vec![0];
    let symbol_names: Vec<(u32, u32)> = kernels
        .iter()
        .map(|(entry, descriptor)| {
            (
                push_name(&mut strtab, entry),
                push_name(&mut strtab, descriptor),
            )
        })
        .collect();
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(&strtab);
    align(&mut bytes, 8);
    let symtab_offset = bytes.len();
    let symbol_count = 1 + kernels.len() * 2;
    bytes.resize(symtab_offset + symbol_count * 24, 0);
    let mut entry_symbols = Vec::new();
    let mut descriptor_symbols = Vec::new();
    for (index, ((entry_name, descriptor_name), descriptor_offset)) in symbol_names
        .iter()
        .zip(&kernel_descriptor_offsets)
        .enumerate()
    {
        let entry_symbol = symtab_offset + (1 + index * 2) * 24;
        entry_symbols.push(entry_symbol);
        write_u32(&mut bytes, entry_symbol, *entry_name);
        bytes[entry_symbol + 4] = 0x12;
        bytes[entry_symbol + 5] = 3;
        write_u16(&mut bytes, entry_symbol + 6, TEXT_SECTION_INDEX as u16);
        let entry_address = (entry_offsets[index] + 0x1000) as u64;
        write_u64(&mut bytes, entry_symbol + 8, entry_address);
        write_u64(&mut bytes, entry_symbol + 16, 64);

        let descriptor_symbol = symtab_offset + (2 + index * 2) * 24;
        descriptor_symbols.push(descriptor_symbol);
        write_u32(&mut bytes, descriptor_symbol, *descriptor_name);
        bytes[descriptor_symbol + 4] = 0x11;
        write_u16(
            &mut bytes,
            descriptor_symbol + 6,
            RODATA_SECTION_INDEX as u16,
        );
        write_u64(&mut bytes, descriptor_symbol + 8, *descriptor_offset as u64);
        write_u64(&mut bytes, descriptor_symbol + 16, 64);

        write_u32(&mut bytes, *descriptor_offset, 0);
        write_u32(&mut bytes, *descriptor_offset + 4, 0);
        write_u32(&mut bytes, *descriptor_offset + 8, 272);
        write_i64(
            &mut bytes,
            *descriptor_offset + 16,
            i64::try_from(entry_address - *descriptor_offset as u64).unwrap(),
        );
        write_u32(&mut bytes, *descriptor_offset + 44, 0x40);
        write_u32(&mut bytes, *descriptor_offset + 48, 0xe0af_0000);
        write_u32(&mut bytes, *descriptor_offset + 52, 0x1390);
        write_u16(&mut bytes, *descriptor_offset + 56, 0x041e);
    }

    let mut descriptor_offsets = Vec::new();
    for _ in 0..descriptor_count {
        align(&mut bytes, DEVICE_DESCRIPTOR_SECTION_ALIGNMENT as usize);
        descriptor_offsets.push(bytes.len());
        bytes.extend_from_slice(table);
    }
    let mut extra_offsets = Vec::new();
    for _ in extra_names {
        align(&mut bytes, 8);
        extra_offsets.push(bytes.len());
        bytes.extend_from_slice(b"alias-data");
    }

    let mut shstr = vec![0];
    if hostile_name_sections != 0 {
        shstr.resize(256 * 1024, b'x');
        shstr.push(0);
    }
    let note_name = push_name(&mut shstr, ".note");
    let rodata_name = push_name(&mut shstr, ".rodata");
    let text_name = push_name(&mut shstr, ".text");
    let strtab_name = push_name(&mut shstr, ".strtab");
    let symtab_name = push_name(&mut shstr, ".symtab");
    let descriptor_name = push_name(&mut shstr, DEVICE_DESCRIPTOR_SECTION_NAME);
    let extra_name_offsets: Vec<u32> = extra_names
        .iter()
        .map(|name| push_name(&mut shstr, name))
        .collect();
    let shstr_name = push_name(&mut shstr, ".shstrtab");
    let shstr_offset = bytes.len();
    bytes.extend_from_slice(&shstr);
    align(&mut bytes, 8);
    bytes.extend_from_slice(&[0; 8]);
    let section_table_offset = bytes.len();
    let section_count = 1 + 5 + descriptor_count + extra_names.len() + hostile_name_sections + 1;
    bytes.resize(
        section_table_offset + section_count * SECTION_HEADER_BYTES,
        0,
    );

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, ELF_HEADER_BYTES as u64);
    write_u32(&mut bytes, 48, 0x4a);
    write_u64(&mut bytes, 40, section_table_offset as u64);
    write_u16(&mut bytes, 52, ELF_HEADER_BYTES as u16);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, section_count as u16);
    write_u16(&mut bytes, 62, (section_count - 1) as u16);

    let first_program_header = ELF_HEADER_BYTES;
    write_u32(&mut bytes, first_program_header, 1);
    write_u32(&mut bytes, first_program_header + 4, 4);
    write_u64(&mut bytes, first_program_header + 32, rodata_end as u64);
    write_u64(&mut bytes, first_program_header + 40, rodata_end as u64);
    write_u64(&mut bytes, first_program_header + 48, 0x1000);

    let second_program_header = first_program_header + PROGRAM_HEADER_BYTES;
    write_u32(&mut bytes, second_program_header, 1);
    write_u32(&mut bytes, second_program_header + 4, 5);
    write_u64(&mut bytes, second_program_header + 8, text_offset as u64);
    write_u64(
        &mut bytes,
        second_program_header + 16,
        (text_offset + 0x1000) as u64,
    );
    write_u64(
        &mut bytes,
        second_program_header + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(
        &mut bytes,
        second_program_header + 40,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, second_program_header + 48, 0x1000);

    let note_header = section_table_offset + NOTE_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, note_name);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let rodata_header = section_table_offset + RODATA_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, rodata_header, rodata_name);
    write_u32(&mut bytes, rodata_header + 4, 1);
    write_u64(&mut bytes, rodata_header + 8, 2);
    write_u64(&mut bytes, rodata_header + 16, rodata_offset as u64);
    write_u64(&mut bytes, rodata_header + 24, rodata_offset as u64);
    write_u64(
        &mut bytes,
        rodata_header + 32,
        (rodata_end - rodata_offset) as u64,
    );
    write_u64(&mut bytes, rodata_header + 48, 64);

    let text_header = section_table_offset + TEXT_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, text_header, text_name);
    write_u32(&mut bytes, text_header + 4, 1);
    write_u64(&mut bytes, text_header + 8, 6);
    write_u64(&mut bytes, text_header + 16, (text_offset + 0x1000) as u64);
    write_u64(&mut bytes, text_header + 24, text_offset as u64);
    write_u64(
        &mut bytes,
        text_header + 32,
        (text_end - text_offset) as u64,
    );
    write_u64(&mut bytes, text_header + 48, 256);

    let strtab_header = section_table_offset + STRTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strtab_header, strtab_name);
    write_u32(&mut bytes, strtab_header + 4, 3);
    write_u64(&mut bytes, strtab_header + 24, strtab_offset as u64);
    write_u64(&mut bytes, strtab_header + 32, strtab.len() as u64);
    write_u64(&mut bytes, strtab_header + 48, 1);

    let symtab_header = section_table_offset + SYMTAB_SECTION_INDEX * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header, symtab_name);
    write_u32(&mut bytes, symtab_header + 4, 2);
    write_u64(&mut bytes, symtab_header + 24, symtab_offset as u64);
    write_u64(&mut bytes, symtab_header + 32, (symbol_count * 24) as u64);
    write_u32(&mut bytes, symtab_header + 40, STRTAB_SECTION_INDEX as u32);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 48, 8);
    write_u64(&mut bytes, symtab_header + 56, 24);

    let mut descriptor_headers = Vec::new();
    for (position, offset) in descriptor_offsets.iter().copied().enumerate() {
        let index = DESCRIPTOR_SECTION_INDEX + position;
        let header = section_table_offset + index * SECTION_HEADER_BYTES;
        descriptor_headers.push(header);
        write_u32(&mut bytes, header, descriptor_name);
        write_u32(&mut bytes, header + 4, 1);
        write_u64(&mut bytes, header + 24, offset as u64);
        write_u64(&mut bytes, header + 32, table.len() as u64);
        write_u64(&mut bytes, header + 48, DEVICE_DESCRIPTOR_SECTION_ALIGNMENT);
    }

    let mut extra_headers = Vec::new();
    for (position, (name_offset, data_offset)) in
        extra_name_offsets.iter().zip(&extra_offsets).enumerate()
    {
        let index = DESCRIPTOR_SECTION_INDEX + descriptor_count + position;
        let header = section_table_offset + index * SECTION_HEADER_BYTES;
        extra_headers.push(header);
        write_u32(&mut bytes, header, *name_offset);
        write_u32(&mut bytes, header + 4, 1);
        write_u64(&mut bytes, header + 24, *data_offset as u64);
        write_u64(&mut bytes, header + 32, b"alias-data".len() as u64);
        write_u64(&mut bytes, header + 48, 1);
    }

    for position in 0..hostile_name_sections {
        let index = DESCRIPTOR_SECTION_INDEX + descriptor_count + extra_names.len() + position;
        let header = section_table_offset + index * SECTION_HEADER_BYTES;
        write_u32(&mut bytes, header, 1);
        write_u32(&mut bytes, header + 4, 8);
        write_u64(&mut bytes, header + 48, 1);
    }

    let shstr_header = section_table_offset + (section_count - 1) * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, shstr_header, shstr_name);
    write_u32(&mut bytes, shstr_header + 4, 3);
    write_u64(&mut bytes, shstr_header + 24, shstr_offset as u64);
    write_u64(&mut bytes, shstr_header + 32, shstr.len() as u64);
    write_u64(&mut bytes, shstr_header + 48, 1);

    Fixture {
        bytes,
        descriptor_offsets,
        descriptor_headers,
        extra_headers,
        shstr_header,
        kernel_descriptor_offsets,
        entry_symbols,
        descriptor_symbols,
        symtab_header,
    }
}

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&(owner.len() as u32).to_le_bytes());
    note.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    note.extend_from_slice(&32u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

fn push_name(strings: &mut Vec<u8>, name: &str) -> u32 {
    let offset = strings.len() as u32;
    strings.extend_from_slice(name.as_bytes());
    strings.push(0);
    offset
}

fn section_name_file_offset(bytes: &[u8], section_index: usize) -> usize {
    let section_table = read_u64(bytes, 40) as usize;
    let shstr_index = read_u16(bytes, 62) as usize;
    let shstr_header = section_table + shstr_index * SECTION_HEADER_BYTES;
    let shstr_offset = read_u64(bytes, shstr_header + 24) as usize;
    let section_header = section_table + section_index * SECTION_HEADER_BYTES;
    shstr_offset + read_u32(bytes, section_header) as usize
}

fn kernel_records_offset(bytes: &[u8]) -> usize {
    let mut offset = 52;
    for _ in 0..2 {
        offset += 2 + read_u16(bytes, offset) as usize;
    }
    offset += 20;
    for _ in 0..3 {
        offset += 2 + read_u16(bytes, offset) as usize;
    }
    let type_count = read_u16(bytes, offset) as usize;
    let layout_count = read_u16(bytes, offset + 2) as usize;
    offset + 8 + type_count * 36 + layout_count * 44
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    while !bytes.len().is_multiple_of(alignment) {
        bytes.push(0);
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn write_i64(bytes: &mut [u8], offset: usize, value: i64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

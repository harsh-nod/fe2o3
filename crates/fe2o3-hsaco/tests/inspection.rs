use std::{env, fs};

use fe2o3_hsaco::{
    ArgumentAddressSpace, COV6_IMPLICIT_ARGUMENT_BYTES, CodeObjectVersion, ExplicitValueKind,
    ExplicitValueType, Gfx1250Revision, HiddenValueKind, InspectionError, KernelBindingError,
    KernelKind, MAX_ARGUMENTS_PER_KERNEL, MAX_ELF_NOTES, MAX_ELF_SYMBOLS, MAX_HSACO_BYTES,
    MAX_KERNARG_BYTES, MAX_KERNELS, MAX_MESSAGEPACK_COLLECTION_ITEMS, MAX_MESSAGEPACK_DEPTH,
    MAX_METADATA_BYTES, MessagePackLimit, inspect, inspect_and_bind_kernel_descriptors,
};
use rmpv::{Value, encode::write_value};

const ELF_HEADER_BYTES: usize = 64;
const SECTION_HEADER_BYTES: usize = 64;
const METADATA_PROFILE_ENV: &str = "FE2O3_TEST_METADATA_PROFILE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneratedMetadataExpectation {
    max_flat_workgroup_size: u32,
    required_workgroup_size: Option<[u32; 3]>,
}

fn generated_metadata_expectation(
    profile: Option<&str>,
) -> Result<GeneratedMetadataExpectation, String> {
    match profile.unwrap_or("legacy-v1") {
        "legacy-v1" => Ok(GeneratedMetadataExpectation {
            max_flat_workgroup_size: 1024,
            required_workgroup_size: None,
        }),
        "kernel-ir-v1" => Ok(GeneratedMetadataExpectation {
            max_flat_workgroup_size: 256,
            required_workgroup_size: Some([256, 1, 1]),
        }),
        value => Err(format!(
            "{METADATA_PROFILE_ENV} must be exactly `legacy-v1` or `kernel-ir-v1`, got `{value}`"
        )),
    }
}

#[test]
fn generated_metadata_expectations_are_pipeline_specific() {
    assert_eq!(
        generated_metadata_expectation(None).unwrap(),
        GeneratedMetadataExpectation {
            max_flat_workgroup_size: 1024,
            required_workgroup_size: None,
        }
    );
    assert_eq!(
        generated_metadata_expectation(Some("legacy-v1")).unwrap(),
        GeneratedMetadataExpectation {
            max_flat_workgroup_size: 1024,
            required_workgroup_size: None,
        }
    );
    assert_eq!(
        generated_metadata_expectation(Some("kernel-ir-v1")).unwrap(),
        GeneratedMetadataExpectation {
            max_flat_workgroup_size: 256,
            required_workgroup_size: Some([256, 1, 1]),
        }
    );
}

#[test]
fn generated_metadata_expectations_reject_malformed_profiles() {
    for value in [
        "",
        "legacy",
        "kernel-ir",
        "KERNEL-IR-V1",
        "kernel-ir-v1 ",
        "256",
    ] {
        let error = generated_metadata_expectation(Some(value)).unwrap_err();
        assert!(error.contains(METADATA_PROFILE_ENV));
        assert!(error.contains(value));
    }
}

#[test]
fn inspects_bounded_physical_kernel_metadata() {
    let hsaco = valid_hsaco();
    let inspected = inspect(&hsaco).unwrap();

    assert_eq!(inspected.code_object_version(), CodeObjectVersion::V6);
    assert_eq!(inspected.metadata_version().major(), 1);
    assert_eq!(inspected.metadata_version().minor(), 2);
    assert_eq!(inspected.target().to_string(), "gfx1151");
    assert!(!inspected.has_printf_metadata());
    assert_eq!(inspected.kernels().len(), 1);

    let kernel = &inspected.kernels()[0];
    assert_eq!(kernel.name(), "vecadd");
    assert_eq!(kernel.symbol(), "vecadd.kd");
    assert_eq!(kernel.kernarg_segment_size(), 272);
    assert_eq!(kernel.kernarg_segment_alignment(), 8);
    assert_eq!(kernel.group_segment_fixed_size(), 0);
    assert_eq!(kernel.private_segment_fixed_size(), 16);
    assert_eq!(kernel.wavefront_size(), 32);
    assert_eq!(kernel.sgpr_count(), 14);
    assert_eq!(kernel.vgpr_count(), 7);
    assert_eq!(kernel.agpr_count(), Some(3));
    assert_eq!(kernel.sgpr_spill_count(), Some(2));
    assert_eq!(kernel.vgpr_spill_count(), Some(4));
    assert_eq!(kernel.max_flat_workgroup_size(), 1024);
    assert_eq!(kernel.required_workgroup_size(), None);
    assert_eq!(kernel.max_workgroups(), [None, None, None]);
    assert_eq!(kernel.cluster_dims(), None);
    assert_eq!(kernel.kind(), KernelKind::Normal);
    assert!(!kernel.uniform_work_group_size());
    assert!(!kernel.uses_dynamic_stack());
    assert_eq!(kernel.workgroup_processor_mode(), Some(true));
    assert_eq!(kernel.gfx1250_revision(), None);
    assert_eq!(kernel.device_enqueue_symbol(), None);
    assert_eq!(kernel.implicit_argument_offset(), Some(16));
    assert_eq!(kernel.implicit_argument_size(), 256);

    let explicit = kernel.explicit_arguments();
    assert_eq!(explicit.len(), 2);
    assert_eq!(explicit[0].name(), Some("a_ptr"));
    assert_eq!(explicit[0].offset(), 0);
    assert_eq!(explicit[0].size(), 8);
    assert_eq!(explicit[0].value_kind(), ExplicitValueKind::GlobalBuffer);
    assert_eq!(
        explicit[0].address_space(),
        Some(ArgumentAddressSpace::Global)
    );
    assert_eq!(explicit[0].access(), None);
    assert_eq!(explicit[0].alignment(), None);
    assert_eq!(explicit[0].type_name(), None);
    assert_eq!(explicit[0].is_const(), None);
    assert_eq!(explicit[1].name(), Some("a_len"));
    assert_eq!(explicit[1].offset(), 8);
    assert_eq!(explicit[1].address_space(), None);

    let hidden = kernel.hidden_arguments();
    assert_eq!(hidden.len(), 13);
    assert_eq!(hidden[0].offset(), 16);
    assert_eq!(hidden[0].value_kind(), HiddenValueKind::BlockCountX);
    assert_eq!(hidden[12].offset(), 80);
    assert_eq!(hidden[12].value_kind(), HiddenValueKind::GridDimensions);
}

#[test]
fn preserves_public_kernel_metadata_declaration_presence() {
    let mut present = valid_kernel("k", "k.kd");
    as_map_mut(&mut present).extend([
        (Value::from(".language"), Value::from("OpenCL C")),
        (
            Value::from(".language_version"),
            Value::Array(vec![Value::from(2), Value::from(0)]),
        ),
        (Value::from(".kind"), Value::from("normal")),
        (Value::from(".uniform_work_group_size"), Value::from(0)),
        (Value::from(".uses_dynamic_stack"), Value::from(false)),
        (
            Value::from(".workgroup_size_hint"),
            Value::Array(vec![Value::from(64), Value::from(1), Value::from(1)]),
        ),
        (Value::from(".vec_type_hint"), Value::from("float")),
    ]);
    let present = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![present])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    let present = &present.kernels()[0];

    assert_eq!(present.source_language(), Some("OpenCL C"));
    assert_eq!(present.source_language_version(), Some([2, 0]));
    assert_eq!(present.kind(), KernelKind::Normal);
    assert!(present.kind_was_emitted());
    assert_eq!(present.uniform_work_group_size_declaration(), Some(false));
    assert!(!present.uniform_work_group_size());
    assert_eq!(present.uses_dynamic_stack_declaration(), Some(false));
    assert!(!present.uses_dynamic_stack());
    assert!(present.workgroup_size_hint_was_emitted());
    assert!(present.vector_type_hint_was_emitted());
    assert!(present.arguments_were_emitted());
    assert_eq!(present.sgpr_count(), 14);
    assert_eq!(present.vgpr_count(), 7);
    assert_eq!(present.agpr_count(), Some(3));
    assert_eq!(present.sgpr_spill_count(), Some(2));
    assert_eq!(present.vgpr_spill_count(), Some(4));

    let mut absent = valid_kernel("k", "k.kd");
    set_field(&mut absent, ".kernarg_segment_size", Value::from(0));
    for field in [
        ".args",
        ".agpr_count",
        ".sgpr_spill_count",
        ".vgpr_spill_count",
    ] {
        remove_field(&mut absent, field);
    }
    let absent = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![absent])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    let absent = &absent.kernels()[0];

    assert_eq!(absent.source_language(), None);
    assert_eq!(absent.source_language_version(), None);
    assert_eq!(absent.kind(), KernelKind::Normal);
    assert!(!absent.kind_was_emitted());
    assert_eq!(absent.uniform_work_group_size_declaration(), None);
    assert!(!absent.uniform_work_group_size());
    assert_eq!(absent.uses_dynamic_stack_declaration(), None);
    assert!(!absent.uses_dynamic_stack());
    assert!(!absent.workgroup_size_hint_was_emitted());
    assert!(!absent.vector_type_hint_was_emitted());
    assert!(!absent.arguments_were_emitted());
    assert!(absent.explicit_arguments().is_empty());
    assert!(absent.hidden_arguments().is_empty());
    assert_eq!(absent.sgpr_count(), 14);
    assert_eq!(absent.vgpr_count(), 7);
    assert_eq!(absent.agpr_count(), None);
    assert_eq!(absent.sgpr_spill_count(), None);
    assert_eq!(absent.vgpr_spill_count(), None);
}

#[test]
fn supports_code_object_v4_v5_and_v6_metadata_versions() {
    for (abi, version, expected) in [
        (2, (1, 1), CodeObjectVersion::V4),
        (3, (1, 2), CodeObjectVersion::V5),
        (4, (1, 2), CodeObjectVersion::V6),
    ] {
        let kernel = if expected == CodeObjectVersion::V4 {
            valid_v4_kernel("vecadd", "vecadd.kd")
        } else {
            valid_kernel("vecadd", "vecadd.kd")
        };
        let metadata = metadata(version, vec![kernel]);
        let hsaco = hsaco(&encode(&metadata), abi, &[b"AMDGPU\0"]);
        let inspected = inspect(&hsaco).unwrap();
        assert_eq!(inspected.code_object_version(), expected);
        assert_eq!(
            inspected.kernels()[0].implicit_argument_size(),
            if expected == CodeObjectVersion::V4 {
                56
            } else {
                256
            }
        );
    }

    let wrong = metadata((1, 1), vec![valid_kernel("vecadd", "vecadd.kd")]);
    assert_eq!(
        inspect(&hsaco(&encode(&wrong), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::MetadataVersionMismatch)
    );
}

#[test]
fn every_truncation_of_a_valid_hsaco_is_rejected() {
    let hsaco = valid_hsaco();
    for length in 0..hsaco.len() {
        assert!(
            inspect(&hsaco[..length]).is_err(),
            "accepted prefix length {length}"
        );
    }
}

#[test]
fn deterministic_single_byte_mutations_are_panic_free() {
    let hsaco = valid_hsaco();
    for index in 0..hsaco.len() {
        for mask in [0x01, 0x80, 0xff] {
            let mut mutated = hsaco.clone();
            mutated[index] ^= mask;
            let _ = inspect(&mutated);
        }
    }
}

#[test]
fn rejects_wrong_elf_identity_and_unbounded_header_counts() {
    let valid = valid_hsaco();
    for (offset, value, expected) in [
        (4, 1, InspectionError::UnsupportedElfClass),
        (5, 2, InspectionError::UnsupportedEndianness),
        (7, 0, InspectionError::UnsupportedOsAbi),
        (8, 1, InspectionError::UnsupportedCodeObjectVersion),
    ] {
        let mut bytes = valid.clone();
        bytes[offset] = value;
        assert_eq!(inspect(&bytes), Err(expected));
    }

    let mut wrong_machine = valid.clone();
    write_u16(&mut wrong_machine, 18, 62);
    assert_eq!(
        inspect(&wrong_machine),
        Err(InspectionError::UnsupportedMachine)
    );

    let mut too_many_sections = valid.clone();
    write_u16(&mut too_many_sections, 60, 257);
    assert_eq!(
        inspect(&too_many_sections),
        Err(InspectionError::TooManySections)
    );

    let mut too_many_segments = valid;
    write_u16(&mut too_many_segments, 56, 65);
    assert_eq!(
        inspect(&too_many_segments),
        Err(InspectionError::TooManySegments)
    );
}

#[test]
fn rejects_hsaco_and_metadata_note_size_limits() {
    assert_eq!(
        inspect(&vec![0; MAX_HSACO_BYTES + 1]),
        Err(InspectionError::InputTooLarge)
    );

    let metadata = vec![0xc0; MAX_METADATA_BYTES + 1];
    assert_eq!(
        inspect(&hsaco(&metadata, 4, &[b"AMDGPU\0"])),
        Err(InspectionError::MetadataNoteTooLarge)
    );
}

#[test]
fn accepts_identical_owned_metadata_notes_and_limits_note_count() {
    let metadata = encode(&metadata((1, 2), vec![valid_kernel("k", "k.kd")]));
    assert_eq!(
        inspect(&hsaco(&metadata, 4, &[b"OTHER\0"])),
        Err(InspectionError::MissingMetadataNote)
    );
    let inspected = inspect(&hsaco(&metadata, 4, &[b"AMDGPU\0", b"AMDGPU\0"])).unwrap();
    assert_eq!(inspected.kernels()[0].name(), "k");

    let owners = vec![b"OTHER\0".as_slice(); MAX_ELF_NOTES + 1];
    assert_eq!(
        inspect(&hsaco(&metadata, 4, &owners)),
        Err(InspectionError::TooManyNotes)
    );
}

#[test]
fn finds_metadata_in_pt_note_when_section_headers_are_absent() {
    let metadata = encode(&metadata((1, 2), vec![valid_kernel("k", "k.kd")]));
    let inspected = inspect(&segment_only_hsaco(&metadata, 4)).unwrap();
    assert_eq!(inspected.kernels()[0].name(), "k");
}

#[test]
fn deduplicates_one_physical_note_referenced_by_section_and_segment() {
    let metadata = encode(&metadata((1, 2), vec![valid_kernel("k", "k.kd")]));
    let inspected = inspect(&hsaco_with_shared_note_views(&metadata, 4)).unwrap();
    assert_eq!(inspected.kernels()[0].name(), "k");
}

#[test]
fn rejects_distinct_metadata_notes_across_section_and_segment_views() {
    let section_metadata = encode(&metadata((1, 2), vec![valid_kernel("a", "a.kd")]));
    let segment_metadata = encode(&metadata((1, 2), vec![valid_kernel("b", "b.kd")]));
    assert_eq!(
        inspect(&hsaco_with_distinct_note_views(
            &section_metadata,
            &segment_metadata,
            4,
        )),
        Err(InspectionError::DuplicateMetadataNote)
    );
}

#[test]
fn rejects_messagepack_trailing_depth_count_and_reserved_markers() {
    let mut trailing = encode(&metadata((1, 2), vec![valid_kernel("k", "k.kd")]));
    trailing.push(0xc0);
    assert_eq!(
        inspect(&hsaco(&trailing, 4, &[b"AMDGPU\0"])),
        Err(InspectionError::TrailingMessagePack)
    );

    let mut deep = vec![0x91; MAX_MESSAGEPACK_DEPTH + 1];
    deep.push(0xc0);
    assert_eq!(
        inspect(&hsaco(&deep, 4, &[b"AMDGPU\0"])),
        Err(InspectionError::MessagePackLimit(MessagePackLimit::Depth))
    );

    let mut count = vec![0xdd];
    count.extend_from_slice(
        &u32::try_from(MAX_MESSAGEPACK_COLLECTION_ITEMS + 1)
            .unwrap()
            .to_be_bytes(),
    );
    assert_eq!(
        inspect(&hsaco(&count, 4, &[b"AMDGPU\0"])),
        Err(InspectionError::MessagePackLimit(
            MessagePackLimit::CollectionItems
        ))
    );

    assert_eq!(
        inspect(&hsaco(&[0xc1], 4, &[b"AMDGPU\0"])),
        Err(InspectionError::MalformedMessagePack)
    );
}

#[test]
fn rejects_duplicate_keys_and_malformed_required_fields() {
    let mut duplicate = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
    as_map_mut(&mut duplicate).push((
        Value::from("amdhsa.target"),
        Value::from("amdgcn-amd-amdhsa--gfx1151"),
    ));
    assert_eq!(
        inspect(&hsaco(&encode(&duplicate), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::DuplicateMapKey)
    );

    let mut missing = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
    remove_field(&mut missing, "amdhsa.target");
    assert_eq!(
        inspect(&hsaco(&encode(&missing), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::MissingField("amdhsa.target"))
    );

    let mut wrong_type = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
    set_field(&mut wrong_type, "amdhsa.target", Value::from(7));
    assert_eq!(
        inspect(&hsaco(&encode(&wrong_type), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::InvalidFieldType("amdhsa.target"))
    );

    assert_eq!(
        inspect(&hsaco(&[0x81, 0x01, 0xc0], 4, &[b"AMDGPU\0"])),
        Err(InspectionError::NonStringMapKey)
    );
    assert_eq!(
        inspect(&hsaco(&[0x81, 0xd9, 0x01, 0xff, 0xc0], 4, &[b"AMDGPU\0"])),
        Err(InspectionError::NonStringMapKey)
    );
    assert_eq!(
        inspect(&hsaco(
            &[0x81, 0xa1, b'x', 0xd9, 0x01, 0xff],
            4,
            &[b"AMDGPU\0"],
        )),
        Err(InspectionError::InvalidUtf8String)
    );
}

#[test]
fn rejects_invalid_target_prefix_and_target_id() {
    for target in ["gfx1151", "amdgcn-amd-amdhsa--gfx9999"] {
        let mut value = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
        set_field(&mut value, "amdhsa.target", Value::from(target));
        assert!(matches!(
            inspect(&hsaco(&encode(&value), 4, &[b"AMDGPU\0"])),
            Err(InspectionError::InvalidTargetPrefix | InspectionError::InvalidTargetId)
        ));
    }
}

#[test]
fn requires_canonical_target_feature_order() {
    let mut noncanonical = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
    set_field(
        &mut noncanonical,
        "amdhsa.target",
        Value::from("amdgcn-amd-amdhsa--gfx942:xnack+:sramecc-"),
    );
    assert_eq!(
        inspect(&hsaco(&encode(&noncanonical), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::NonCanonicalTargetId)
    );

    let mut canonical = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
    set_field(
        &mut canonical,
        "amdhsa.target",
        Value::from("amdgcn-amd-amdhsa--gfx942:sramecc-:xnack+"),
    );
    let mut bytes = hsaco(&encode(&canonical), 4, &[b"AMDGPU\0"]);
    write_u32(&mut bytes, 48, 0xb4c);
    assert_eq!(
        inspect(&bytes).unwrap().target().to_string(),
        "gfx942:sramecc-:xnack+"
    );
}

#[test]
fn cross_checks_metadata_target_against_independent_literal_elf_flags() {
    for (target, flags) in [
        ("gfx1151", 0x4a),
        ("gfx942", 0x54c),
        ("gfx942:sramecc+:xnack-", 0xe4c),
        ("gfx942:sramecc-:xnack+", 0xb4c),
        ("gfx950:sramecc+:xnack+", 0xf4f),
    ] {
        let mut document = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
        set_field(
            &mut document,
            "amdhsa.target",
            Value::from(format!("amdgcn-amd-amdhsa--{target}")),
        );
        let mut bytes = hsaco(&encode(&document), 4, &[b"AMDGPU\0"]);
        write_u32(&mut bytes, 48, flags);
        assert_eq!(inspect(&bytes).unwrap().target().to_string(), target);
    }

    for flags in [0x4b, 0x14a, 0x0100_004a, 0x8000_004a] {
        let mut bytes = valid_hsaco();
        write_u32(&mut bytes, 48, flags);
        assert_eq!(inspect(&bytes), Err(InspectionError::TargetFlagsMismatch));
    }
}

#[test]
fn rejects_unknown_root_and_kernel_fields() {
    let mut root = metadata((1, 2), vec![valid_kernel("k", "k.kd")]);
    as_map_mut(&mut root).push((Value::from("amdhsa.future"), Value::from(true)));
    assert_eq!(
        inspect(&hsaco(&encode(&root), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::UnknownRootField)
    );

    let mut kernel = valid_kernel("k", "k.kd");
    as_map_mut(&mut kernel).push((Value::from(".future_behavior"), Value::from(true)));
    assert_kernel_error(kernel, 4, (1, 2), InspectionError::UnknownKernelField);
}

#[test]
fn rejects_duplicate_kernel_names_and_symbols() {
    let duplicate_name = metadata(
        (1, 2),
        vec![valid_kernel("same", "a.kd"), valid_kernel("same", "b.kd")],
    );
    assert_eq!(
        inspect(&hsaco(&encode(&duplicate_name), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::DuplicateKernelName)
    );

    let duplicate_symbol = metadata(
        (1, 2),
        vec![valid_kernel("a", "same.kd"), valid_kernel("b", "same.kd")],
    );
    assert_eq!(
        inspect(&hsaco(&encode(&duplicate_symbol), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::DuplicateKernelSymbol)
    );

    let kernels = vec![Value::Nil; MAX_KERNELS + 1];
    assert_eq!(
        inspect(&hsaco(
            &encode(&metadata((1, 2), kernels)),
            4,
            &[b"AMDGPU\0"]
        )),
        Err(InspectionError::TooManyKernels)
    );
}

#[test]
fn accepts_metadata_with_no_kernels() {
    let document = metadata((1, 2), Vec::new());
    let inspected = inspect(&hsaco(&encode(&document), 4, &[b"AMDGPU\0"])).unwrap();
    assert!(inspected.kernels().is_empty());
}

#[test]
fn preserves_launch_semantics_and_printf_presence() {
    let mut kernel = valid_kernel("k", "k.kd");
    as_map_mut(&mut kernel).extend([
        (
            Value::from(".reqd_workgroup_size"),
            Value::Array(vec![Value::from(16), Value::from(8), Value::from(8)]),
        ),
        (Value::from(".max_num_workgroups_x"), Value::from(7)),
        (Value::from(".max_num_work_groups_y"), Value::from(11)),
        (Value::from(".max_num_workgroups_z"), Value::from(13)),
        (
            Value::from(".cluster_dims"),
            Value::Array(vec![Value::from(0), Value::from(7), Value::from(1)]),
        ),
        (Value::from(".kind"), Value::from("fini")),
        (Value::from(".uniform_work_group_size"), Value::from(1)),
        (Value::from(".uses_dynamic_stack"), Value::from(true)),
        (
            Value::from(".device_enqueue_symbol"),
            Value::from("enqueue.kd"),
        ),
    ]);
    let mut document = metadata((1, 2), vec![kernel]);
    as_map_mut(&mut document).push((Value::from("amdhsa.printf"), Value::Array(Vec::new())));
    let inspected = inspect(&hsaco(&encode(&document), 4, &[b"AMDGPU\0"])).unwrap();
    let kernel = &inspected.kernels()[0];

    assert!(inspected.has_printf_metadata());
    assert_eq!(kernel.required_workgroup_size(), Some([16, 8, 8]));
    assert_eq!(kernel.max_workgroups(), [Some(7), Some(11), Some(13)]);
    assert_eq!(kernel.cluster_dims(), Some([0, 7, 1]));
    assert_eq!(kernel.kind(), KernelKind::Fini);
    assert!(kernel.uniform_work_group_size());
    assert!(kernel.uses_dynamic_stack());
    assert_eq!(kernel.device_enqueue_symbol(), Some("enqueue.kd"));
}

#[test]
fn preserves_optional_execution_evidence_and_pinned_revision_rules() {
    let mut kernel = valid_kernel("k", "k.kd");
    set_field(&mut kernel, ".workgroup_processor_mode", Value::from(false));
    as_map_mut(&mut kernel).push((Value::from(".gfx1250_revision"), Value::from("B0")));
    let inspected = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![kernel])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    let kernel = &inspected.kernels()[0];
    assert_eq!(kernel.sgpr_count(), 14);
    assert_eq!(kernel.vgpr_count(), 7);
    assert_eq!(kernel.agpr_count(), Some(3));
    assert_eq!(kernel.sgpr_spill_count(), Some(2));
    assert_eq!(kernel.vgpr_spill_count(), Some(4));
    assert_eq!(kernel.workgroup_processor_mode(), Some(false));
    assert_eq!(kernel.gfx1250_revision(), Some(Gfx1250Revision::B0));

    let mut absent = valid_kernel("k", "k.kd");
    for field in [
        ".agpr_count",
        ".sgpr_spill_count",
        ".vgpr_spill_count",
        ".workgroup_processor_mode",
    ] {
        remove_field(&mut absent, field);
    }
    let inspected = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![absent])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    let kernel = &inspected.kernels()[0];
    assert_eq!(kernel.agpr_count(), None);
    assert_eq!(kernel.sgpr_spill_count(), None);
    assert_eq!(kernel.vgpr_spill_count(), None);
    assert_eq!(kernel.workgroup_processor_mode(), None);

    let mut a0 = valid_kernel("k", "k.kd");
    as_map_mut(&mut a0).push((Value::from(".gfx1250_revision"), Value::from("A0")));
    let mut document = metadata((1, 2), vec![a0]);
    set_field(
        &mut document,
        "amdhsa.target",
        Value::from("amdgcn-amd-amdhsa--gfx1250"),
    );
    let mut bytes = hsaco(&encode(&document), 4, &[b"AMDGPU\0"]);
    write_u32(&mut bytes, 48, 0x449);
    assert_eq!(
        inspect(&bytes).unwrap().kernels()[0].gfx1250_revision(),
        Some(Gfx1250Revision::A0)
    );

    let mut a0_family = valid_kernel("k", "k.kd");
    as_map_mut(&mut a0_family).push((Value::from(".gfx1250_revision"), Value::from("A0")));
    let mut document = metadata((1, 2), vec![a0_family]);
    set_field(
        &mut document,
        "amdhsa.target",
        Value::from("amdgcn-amd-amdhsa--gfx1251"),
    );
    let mut bytes = hsaco(&encode(&document), 4, &[b"AMDGPU\0"]);
    write_u32(&mut bytes, 48, 0x45a);
    assert_eq!(
        inspect(&bytes).unwrap().kernels()[0].gfx1250_revision(),
        Some(Gfx1250Revision::A0)
    );

    let mut invalid_a0 = valid_kernel("k", "k.kd");
    as_map_mut(&mut invalid_a0).push((Value::from(".gfx1250_revision"), Value::from("A0")));
    assert_kernel_error(
        invalid_a0,
        4,
        (1, 2),
        InspectionError::InvalidFieldValue(".gfx1250_revision"),
    );

    for (value, expected) in [
        (
            Value::from("C0"),
            InspectionError::InvalidFieldValue(".gfx1250_revision"),
        ),
        (
            Value::from(1),
            InspectionError::InvalidFieldType(".gfx1250_revision"),
        ),
    ] {
        let mut invalid = valid_kernel("k", "k.kd");
        as_map_mut(&mut invalid).push((Value::from(".gfx1250_revision"), value));
        assert_kernel_error(invalid, 4, (1, 2), expected);
    }
}

#[test]
fn rejects_invalid_launch_constraint_boundaries_and_aliases() {
    for dims in [[0, 1, 1], [1025, 1, 1], [u32::MAX, u32::MAX, u32::MAX]] {
        let mut kernel = valid_kernel("k", "k.kd");
        push_u32_triplet(&mut kernel, ".reqd_workgroup_size", dims);
        assert_kernel_error(
            kernel,
            4,
            (1, 2),
            InspectionError::InvalidFieldValue(".reqd_workgroup_size"),
        );
    }

    let mut zero_limit = valid_kernel("k", "k.kd");
    as_map_mut(&mut zero_limit).push((Value::from(".max_num_workgroups_x"), Value::from(0)));
    assert_kernel_error(
        zero_limit,
        4,
        (1, 2),
        InspectionError::InvalidFieldValue(".max_num_workgroups_x"),
    );

    let mut aliases = valid_kernel("k", "k.kd");
    as_map_mut(&mut aliases).extend([
        (Value::from(".max_num_workgroups_x"), Value::from(1)),
        (Value::from(".max_num_work_groups_x"), Value::from(1)),
    ]);
    assert_kernel_error(
        aliases,
        4,
        (1, 2),
        InspectionError::ConflictingFieldAliases(".max_num_workgroups_x"),
    );

    for dims in [[0, 0, 0], [1024, 1024, 1024]] {
        let mut kernel = valid_kernel("k", "k.kd");
        push_u32_triplet(&mut kernel, ".cluster_dims", dims);
        assert_kernel_error(
            kernel,
            4,
            (1, 2),
            InspectionError::InvalidFieldValue(".cluster_dims"),
        );
    }

    for (field, value, expected) in [
        (
            ".kind",
            Value::from("future"),
            InspectionError::InvalidFieldValue(".kind"),
        ),
        (
            ".uniform_work_group_size",
            Value::from(2),
            InspectionError::InvalidFieldValue(".uniform_work_group_size"),
        ),
        (
            ".device_enqueue_symbol",
            Value::from(""),
            InspectionError::InvalidFieldValue(".device_enqueue_symbol"),
        ),
    ] {
        let mut kernel = valid_kernel("k", "k.kd");
        as_map_mut(&mut kernel).push((Value::from(field), value));
        assert_kernel_error(kernel, 4, (1, 2), expected);
    }

    for (field, value) in [
        (".workgroup_processor_mode", Value::from(2)),
        (".sgpr_count", Value::from(u32::from(u16::MAX) + 1)),
        (".agpr_count", Value::from(u64::MAX)),
    ] {
        let mut kernel = valid_kernel("k", "k.kd");
        set_field(&mut kernel, field, value);
        assert_kernel_error(kernel, 4, (1, 2), InspectionError::InvalidFieldValue(field));
    }
}

#[test]
fn enforces_code_object_versioned_kernel_fields() {
    for field in [
        ".uses_dynamic_stack",
        ".workgroup_processor_mode",
        ".uniform_work_group_size",
        ".gfx1250_revision",
    ] {
        let mut kernel = valid_v4_kernel("k", "k.kd");
        let value = match field {
            ".uses_dynamic_stack" | ".workgroup_processor_mode" => Value::from(false),
            ".uniform_work_group_size" => Value::from(0),
            ".gfx1250_revision" => Value::from("A0"),
            _ => unreachable!(),
        };
        as_map_mut(&mut kernel).push((Value::from(field), value));
        assert_kernel_error(
            kernel,
            2,
            (1, 1),
            InspectionError::UnsupportedFieldForCodeObjectVersion(field),
        );
    }

    let mut v5 = valid_kernel("k", "k.kd");
    push_u32_triplet(&mut v5, ".cluster_dims", [1, 1, 1]);
    assert_kernel_error(
        v5,
        3,
        (1, 2),
        InspectionError::UnsupportedFieldForCodeObjectVersion(".cluster_dims"),
    );
}

#[test]
fn rejects_bad_argument_ranges_order_overlap_and_classification() {
    assert_argument_error(
        vec![
            argument(Some("a"), 0, 8, "by_value", None),
            argument(Some("b"), 4, 8, "by_value", None),
        ],
        InspectionError::OverlappingArguments,
    );
    assert_argument_error(
        vec![
            argument(Some("a"), 8, 8, "by_value", None),
            argument(Some("b"), 0, 8, "by_value", None),
        ],
        InspectionError::ArgumentsOutOfOrder,
    );
    assert_argument_error(
        vec![
            argument(None, 0, 8, "hidden_queue_ptr", None),
            argument(Some("a"), 8, 8, "by_value", None),
        ],
        InspectionError::ExplicitArgumentAfterHidden,
    );
    assert_argument_error(
        vec![
            argument(Some("same"), 0, 8, "by_value", None),
            argument(Some("same"), 8, 8, "by_value", None),
        ],
        InspectionError::DuplicateArgumentName,
    );
    assert_argument_error(
        vec![argument(Some("a"), 0, 0, "by_value", None)],
        InspectionError::InvalidArgumentRange,
    );

    let arguments = (0..=MAX_ARGUMENTS_PER_KERNEL)
        .map(|index| argument(None, u64::try_from(index).unwrap(), 1, "by_value", None))
        .collect();
    assert_argument_error(arguments, InspectionError::TooManyArguments);
}

#[test]
fn rejects_unknown_argument_qualifiers_and_bad_wavefront() {
    assert_argument_error(
        vec![argument(Some("a"), 0, 8, "mystery", None)],
        InspectionError::UnknownValueKind,
    );
    assert_argument_error(
        vec![argument(Some("a"), 0, 8, "global_buffer", Some("mystery"))],
        InspectionError::UnknownAddressSpace,
    );
    let mut hidden = argument(None, 0, 8, "hidden_queue_ptr", None);
    as_map_mut(&mut hidden).push((Value::from(".address_space"), Value::from(7)));
    assert_argument_error(
        vec![hidden],
        InspectionError::InvalidFieldType(".address_space"),
    );
    let mut explicit = argument(Some("a"), 0, 8, "global_buffer", Some("global"));
    as_map_mut(&mut explicit).push((Value::from(".access"), Value::from("mystery")));
    assert_argument_error(vec![explicit], InspectionError::UnknownAccess);

    let mut kernel = valid_kernel("k", "k.kd");
    set_field(&mut kernel, ".wavefront_size", Value::from(16));
    let value = metadata((1, 2), vec![kernel]);
    assert_eq!(
        inspect(&hsaco(&encode(&value), 4, &[b"AMDGPU\0"])),
        Err(InspectionError::InvalidFieldValue(".wavefront_size"))
    );
}

#[test]
fn preserves_all_recognized_explicit_argument_qualifiers() {
    let mut argument = argument(Some("buffer"), 0, 8, "global_buffer", Some("global"));
    as_map_mut(&mut argument).extend([
        (Value::from(".type_name"), Value::from("const float*")),
        (Value::from(".value_type"), Value::from("f32")),
        (Value::from(".align"), Value::from(8)),
        (Value::from(".pointee_align"), Value::from(16)),
        (Value::from(".access"), Value::from("read_only")),
        (Value::from(".actual_access"), Value::from("read_write")),
        (Value::from(".is_const"), Value::from(true)),
        (Value::from(".is_restrict"), Value::from(false)),
        (Value::from(".is_volatile"), Value::from(true)),
        (Value::from(".is_pipe"), Value::from(false)),
    ]);
    let mut kernel = valid_kernel("k", "k.kd");
    set_field(&mut kernel, ".args", Value::Array(vec![argument]));
    set_field(&mut kernel, ".kernarg_segment_size", Value::from(8));
    let document = metadata((1, 2), vec![kernel]);
    let inspected = inspect(&hsaco(&encode(&document), 4, &[b"AMDGPU\0"])).unwrap();
    let argument = &inspected.kernels()[0].explicit_arguments()[0];

    assert_eq!(argument.type_name(), Some("const float*"));
    assert_eq!(argument.value_type(), Some(ExplicitValueType::F32));
    assert_eq!(argument.alignment(), Some(8));
    assert_eq!(argument.pointee_alignment(), Some(16));
    assert_eq!(
        argument.access(),
        Some(fe2o3_hsaco::ArgumentAccess::ReadOnly)
    );
    assert_eq!(
        argument.actual_access(),
        Some(fe2o3_hsaco::ArgumentAccess::ReadWrite)
    );
    assert_eq!(argument.is_const(), Some(true));
    assert_eq!(argument.is_restrict(), Some(false));
    assert_eq!(argument.is_volatile(), Some(true));
    assert_eq!(argument.is_pipe(), Some(false));
}

#[test]
fn normalizes_every_closed_explicit_value_type_and_rejects_aliases() {
    for (spelling, expected) in [
        ("struct", ExplicitValueType::Struct),
        ("i8", ExplicitValueType::I8),
        ("u8", ExplicitValueType::U8),
        ("i16", ExplicitValueType::I16),
        ("u16", ExplicitValueType::U16),
        ("f16", ExplicitValueType::F16),
        ("i32", ExplicitValueType::I32),
        ("u32", ExplicitValueType::U32),
        ("f32", ExplicitValueType::F32),
        ("i64", ExplicitValueType::I64),
        ("u64", ExplicitValueType::U64),
        ("f64", ExplicitValueType::F64),
    ] {
        let mut argument = argument(Some("value"), 0, 8, "by_value", None);
        as_map_mut(&mut argument).push((Value::from(".value_type"), Value::from(spelling)));
        let mut kernel = valid_kernel("k", "k.kd");
        set_field(&mut kernel, ".args", Value::Array(vec![argument]));
        set_field(&mut kernel, ".kernarg_segment_size", Value::from(8));
        let inspected = inspect(&hsaco(
            &encode(&metadata((1, 2), vec![kernel])),
            4,
            &[b"AMDGPU\0"],
        ))
        .unwrap_or_else(|error| panic!("{spelling}: {error:?}"));
        assert_eq!(
            inspected.kernels()[0].explicit_arguments()[0].value_type(),
            Some(expected)
        );
    }

    for spelling in ["", "I32", "uint64", "ptr", "i128", "deprecated"] {
        let mut argument = argument(Some("value"), 0, 8, "by_value", None);
        as_map_mut(&mut argument).push((Value::from(".value_type"), Value::from(spelling)));
        assert_argument_error(vec![argument], InspectionError::UnknownValueType);
    }

    let mut duplicate = argument(Some("value"), 0, 8, "by_value", None);
    as_map_mut(&mut duplicate).extend([
        (Value::from(".value_type"), Value::from("u64")),
        (Value::from(".value_type"), Value::from("u64")),
    ]);
    assert_argument_error(vec![duplicate], InspectionError::DuplicateMapKey);
}

#[test]
fn rejects_unknown_argument_fields_that_may_change_abi_semantics() {
    let mut argument = argument(Some("a"), 0, 8, "by_value", None);
    as_map_mut(&mut argument).push((Value::from(".future_abi"), Value::from(true)));
    assert_argument_error(vec![argument], InspectionError::UnknownArgumentField);
}

#[test]
fn binds_and_preserves_the_complete_implicit_argument_span() {
    let mut trailing = valid_kernel("k", "k.kd");
    set_field(&mut trailing, ".kernarg_segment_size", Value::from(400));
    assert_kernel_error(
        trailing,
        4,
        (1, 2),
        InspectionError::InvalidImplicitArgumentSpan,
    );

    let mut aligned_gap_arguments = vec![argument(Some("value"), 0, 12, "by_value", None)];
    aligned_gap_arguments.extend(v5_hidden_arguments(16));
    let mut aligned_gap = valid_kernel("k", "k.kd");
    set_field(
        &mut aligned_gap,
        ".args",
        Value::Array(aligned_gap_arguments),
    );
    let inspected = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![aligned_gap])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    assert_eq!(inspected.kernels()[0].implicit_argument_offset(), Some(16));
    assert_eq!(inspected.kernels()[0].implicit_argument_size(), 256);

    let explicit = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    let mut no_hidden = valid_kernel("k", "k.kd");
    set_field(&mut no_hidden, ".args", Value::Array(explicit.clone()));
    set_field(&mut no_hidden, ".kernarg_segment_size", Value::from(16));
    let inspected = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![no_hidden])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    assert_eq!(inspected.kernels()[0].implicit_argument_offset(), None);
    assert_eq!(inspected.kernels()[0].implicit_argument_size(), 0);

    let mut extra_without_records = valid_kernel("k", "k.kd");
    set_field(
        &mut extra_without_records,
        ".args",
        Value::Array(explicit.clone()),
    );
    set_field(
        &mut extra_without_records,
        ".kernarg_segment_size",
        Value::from(20),
    );
    assert_kernel_error(
        extra_without_records,
        4,
        (1, 2),
        InspectionError::InvalidImplicitArgumentSpan,
    );

    let mut arbitrary_gap_arguments = explicit;
    arbitrary_gap_arguments.extend(v5_hidden_arguments(24));
    let mut arbitrary_gap = valid_kernel("k", "k.kd");
    set_field(
        &mut arbitrary_gap,
        ".args",
        Value::Array(arbitrary_gap_arguments),
    );
    set_field(
        &mut arbitrary_gap,
        ".kernarg_segment_size",
        Value::from(280),
    );
    assert_kernel_error(
        arbitrary_gap,
        4,
        (1, 2),
        InspectionError::InvalidImplicitArgumentSpan,
    );
}

#[test]
fn cov6_hidden_abi_requires_the_exact_implicit_span() {
    for implicit_bytes in [68, 252, 256, 260, MAX_KERNARG_BYTES - 16] {
        let mut kernel = valid_kernel("k", "k.kd");
        set_field(
            &mut kernel,
            ".kernarg_segment_size",
            Value::from(16 + implicit_bytes),
        );
        let result = inspect(&hsaco(
            &encode(&metadata((1, 2), vec![kernel])),
            4,
            &[b"AMDGPU\0"],
        ));
        if implicit_bytes == COV6_IMPLICIT_ARGUMENT_BYTES {
            assert_eq!(
                result.unwrap().kernels()[0].implicit_argument_size(),
                COV6_IMPLICIT_ARGUMENT_BYTES
            );
        } else {
            assert_eq!(result, Err(InspectionError::InvalidImplicitArgumentSpan));
        }
    }

    let explicit = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    let mut descriptor_free = valid_kernel("k", "k.kd");
    set_field(&mut descriptor_free, ".args", Value::Array(explicit));
    set_field(
        &mut descriptor_free,
        ".kernarg_segment_size",
        Value::from(16),
    );
    let inspected = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![descriptor_free])),
        4,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    assert_eq!(inspected.kernels()[0].implicit_argument_size(), 0);
}

#[test]
fn cov4_and_cov5_keep_their_version_specific_implicit_spans() {
    let cov4 = inspect(&hsaco(
        &encode(&metadata((1, 1), vec![valid_v4_kernel("k", "k.kd")])),
        2,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    assert_eq!(cov4.kernels()[0].implicit_argument_size(), 56);

    let mut cov5 = valid_kernel("k", "k.kd");
    set_field(&mut cov5, ".kernarg_segment_size", Value::from(400));
    let cov5 = inspect(&hsaco(
        &encode(&metadata((1, 2), vec![cov5])),
        3,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    assert_eq!(cov5.kernels()[0].implicit_argument_size(), 384);
}

#[test]
fn rejects_unaligned_or_truncated_implicit_segments() {
    let mut unaligned = valid_kernel("k", "k.kd");
    set_field(&mut unaligned, ".kernarg_segment_size", Value::from(273));
    assert_kernel_error(
        unaligned,
        4,
        (1, 2),
        InspectionError::InvalidFieldValue(".kernarg_segment_size"),
    );

    let mut truncated = valid_kernel("k", "k.kd");
    set_field(&mut truncated, ".kernarg_segment_size", Value::from(80));
    assert_kernel_error(truncated, 4, (1, 2), InspectionError::InvalidArgumentRange);
}

#[test]
fn validates_v4_hidden_layout_and_rejects_v5_only_kinds() {
    let inspected = inspect(&hsaco(
        &encode(&metadata((1, 1), vec![valid_v4_kernel("k", "k.kd")])),
        2,
        &[b"AMDGPU\0"],
    ))
    .unwrap();
    let hidden = inspected.kernels()[0].hidden_arguments();
    assert_eq!(hidden.len(), 7);
    assert_eq!(hidden[3].value_kind(), HiddenValueKind::None);
    assert_eq!(hidden[6].value_kind(), HiddenValueKind::None);
    assert_eq!(inspected.kernels()[0].implicit_argument_offset(), Some(16));
    assert_eq!(inspected.kernels()[0].implicit_argument_size(), 56);

    assert_arguments_error_for_version(
        v5_hidden_arguments(0),
        2,
        (1, 1),
        InspectionError::UnsupportedValueKindForCodeObjectVersion,
    );
    assert_arguments_error_for_version(
        vec![
            argument(None, 0, 8, "hidden_global_offset_x", None),
            argument(None, 16, 8, "hidden_global_offset_z", None),
        ],
        2,
        (1, 1),
        InspectionError::InvalidHiddenArgumentLayout,
    );
    assert_arguments_error_for_version(
        vec![argument(None, 0, 4, "hidden_global_offset_x", None)],
        2,
        (1, 1),
        InspectionError::InvalidHiddenArgumentLayout,
    );
}

#[test]
fn accepts_v4_implicit_argument_override_thresholds_and_rounding() {
    for implicit_bytes in 0u64..=17 {
        let record_count = usize::try_from((implicit_bytes / 8).min(7)).unwrap();
        let mut arguments = vec![
            argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
            argument(Some("a_len"), 8, 8, "by_value", None),
        ];
        arguments.extend(v4_hidden_arguments(16).into_iter().take(record_count));

        let mut kernel = valid_v4_kernel("k", "k.kd");
        set_field(&mut kernel, ".args", Value::Array(arguments));
        let rounded_implicit_bytes = implicit_bytes.next_multiple_of(4);
        set_field(
            &mut kernel,
            ".kernarg_segment_size",
            Value::from(16 + rounded_implicit_bytes),
        );

        let inspected = inspect(&hsaco(
            &encode(&metadata((1, 1), vec![kernel])),
            2,
            &[b"AMDGPU\0"],
        ))
        .unwrap();
        let kernel = &inspected.kernels()[0];
        assert_eq!(kernel.hidden_arguments().len(), record_count);
        assert_eq!(
            kernel.implicit_argument_offset(),
            (implicit_bytes != 0).then_some(16)
        );
        assert_eq!(kernel.implicit_argument_size(), rounded_implicit_bytes);
    }

    for (rounded_implicit_bytes, record_count, expected) in [
        (12u64, 2usize, InspectionError::InvalidArgumentRange),
        (20, 1, InspectionError::InvalidHiddenArgumentLayout),
    ] {
        let mut arguments = vec![
            argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
            argument(Some("a_len"), 8, 8, "by_value", None),
        ];
        arguments.extend(v4_hidden_arguments(16).into_iter().take(record_count));
        let mut kernel = valid_v4_kernel("k", "k.kd");
        set_field(&mut kernel, ".args", Value::Array(arguments));
        set_field(
            &mut kernel,
            ".kernarg_segment_size",
            Value::from(16 + rounded_implicit_bytes),
        );
        assert_kernel_error(kernel, 2, (1, 1), expected);
    }
}

#[test]
fn validates_v5_hidden_width_alignment_order_uniqueness_and_reserved_gaps() {
    let mut with_queue = v5_hidden_arguments(0);
    with_queue.push(argument(None, 200, 8, "hidden_queue_ptr", None));
    let inspected = inspect_arguments_for_version(with_queue, 4, (1, 2)).unwrap();
    assert_eq!(
        inspected.kernels()[0]
            .hidden_arguments()
            .last()
            .unwrap()
            .offset(),
        200
    );

    assert_arguments_error_for_version(
        v5_hidden_arguments(4),
        4,
        (1, 2),
        InspectionError::InvalidImplicitArgumentSpan,
    );

    let mut wrong_width = v5_hidden_arguments(0);
    set_field(&mut wrong_width[12], ".size", Value::from(4));
    assert_arguments_error_for_version(
        wrong_width,
        4,
        (1, 2),
        InspectionError::InvalidHiddenArgumentLayout,
    );

    let mut duplicate_kind = v5_hidden_arguments(0);
    set_field(
        &mut duplicate_kind[1],
        ".value_kind",
        Value::from("hidden_block_count_x"),
    );
    assert_arguments_error_for_version(
        duplicate_kind,
        4,
        (1, 2),
        InspectionError::InvalidHiddenArgumentLayout,
    );

    let mut explicit_qualifier = v5_hidden_arguments(0);
    as_map_mut(&mut explicit_qualifier[0]).push((Value::from(".is_const"), Value::from(true)));
    assert_arguments_error_for_version(
        explicit_qualifier,
        4,
        (1, 2),
        InspectionError::ExplicitQualifierOnHiddenArgument,
    );

    let mut hidden_none = v5_hidden_arguments(0);
    hidden_none.push(argument(None, 72, 8, "hidden_none", None));
    assert_arguments_error_for_version(
        hidden_none,
        4,
        (1, 2),
        InspectionError::InvalidHiddenArgumentLayout,
    );
}

#[test]
fn explicitly_binds_metadata_to_descriptor_and_entry_symbols() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    let inspected = inspect(&fixture.bytes).unwrap();
    assert_eq!(inspected.kernels()[0].name(), "vecadd");

    let bound = inspect_and_bind_kernel_descriptors(&fixture.bytes).unwrap();
    assert_eq!(bound.inspection(), &inspected);
    assert_eq!(bound.bindings().len(), 1);
    let binding = bound.bindings()[0];
    assert_eq!(binding.kernel_index(), 0);
    assert_eq!(binding.descriptor_address(), fixture.descriptor_address);
    assert_eq!(
        binding.descriptor_file_offset(),
        fixture.descriptor_offset as u64
    );
    assert_eq!(binding.entry_address(), fixture.entry_address);
    assert_eq!(binding.entry_file_offset(), fixture.entry_offset as u64);
    assert_eq!(binding.entry_size(), 64);

    let descriptor = binding.descriptor();
    assert_eq!(descriptor.group_segment_fixed_size(), 0);
    assert_eq!(descriptor.private_segment_fixed_size(), 16);
    assert_eq!(descriptor.kernarg_size(), 272);
    assert_eq!(descriptor.compute_pgm_rsrc3(), 0x40);
    assert_eq!(descriptor.compute_pgm_rsrc1(), 0xe0af_0000);
    assert_eq!(descriptor.compute_pgm_rsrc2(), 0x1391);
    assert_eq!(descriptor.kernel_code_properties(), 0x041e);
    assert_eq!(descriptor.kernarg_preload(), 0);
    assert_eq!(descriptor.wavefront_size(), 32);
    assert!(!descriptor.uses_dynamic_stack());
    assert!(descriptor.private_segment_enabled());
}

#[test]
fn metadata_only_inspection_does_not_require_static_symbols() {
    let bytes = valid_hsaco();
    inspect(&bytes).unwrap();
    assert_eq!(
        inspect_and_bind_kernel_descriptors(&bytes),
        Err(KernelBindingError::InvalidSymbolTable(
            "SHT_SYMTAB is missing"
        ))
    );

    let empty = metadata((1, 2), Vec::new());
    let bound =
        inspect_and_bind_kernel_descriptors(&hsaco(&encode(&empty), 4, &[b"AMDGPU\0"])).unwrap();
    assert!(bound.bindings().is_empty());
}

#[test]
fn every_truncation_of_a_bound_hsaco_is_rejected() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    for length in 0..fixture.bytes.len() {
        assert!(
            inspect_and_bind_kernel_descriptors(&fixture.bytes[..length]).is_err(),
            "accepted bound prefix length {length}"
        );
    }
}

#[test]
fn deterministic_bound_hsaco_mutations_are_panic_free() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    for index in 0..fixture.bytes.len() {
        for mask in [0x01, 0x80, 0xff] {
            let mut mutated = fixture.bytes.clone();
            mutated[index] ^= mask;
            let _ = inspect_and_bind_kernel_descriptors(&mutated);
        }
    }
}

#[test]
fn rejects_malformed_symbol_table_dimensions_links_and_names() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut missing = fixture.bytes.clone();
    write_u32(&mut missing, fixture.symtab_header + 4, 1);
    assert_binding_error(
        missing,
        KernelBindingError::InvalidSymbolTable("SHT_SYMTAB is missing"),
    );

    let mut entry_size = fixture.bytes.clone();
    write_u64(&mut entry_size, fixture.symtab_header + 56, 16);
    assert_binding_error(
        entry_size,
        KernelBindingError::InvalidSymbolTable("invalid symbol entry size or count"),
    );

    let mut table_alignment = fixture.bytes.clone();
    write_u64(&mut table_alignment, fixture.symtab_header + 48, 4);
    assert_binding_error(
        table_alignment,
        KernelBindingError::InvalidSymbolTable("invalid symbol table alignment"),
    );

    let mut string_layout = fixture.bytes.clone();
    write_u64(&mut string_layout, fixture.strtab_header + 56, 1);
    assert_binding_error(
        string_layout,
        KernelBindingError::InvalidSymbolTable("invalid symbol string-table layout"),
    );

    let mut remainder = fixture.bytes.clone();
    write_u64(&mut remainder, fixture.symtab_header + 32, 4 * 24 + 1);
    assert_binding_error(
        remainder,
        KernelBindingError::Inspection(InspectionError::InvalidElf(
            "object parser rejected the file",
        )),
    );

    let mut too_many = fixture.bytes.clone();
    too_many.resize(fixture.symtab_offset + (MAX_ELF_SYMBOLS + 1) * 24, 0);
    write_u64(
        &mut too_many,
        fixture.symtab_header + 32,
        u64::try_from((MAX_ELF_SYMBOLS + 1) * 24).unwrap(),
    );
    assert_binding_error(too_many, KernelBindingError::TooManySymbols);

    let mut bad_link = fixture.bytes.clone();
    write_u32(&mut bad_link, fixture.symtab_header + 40, 99);
    assert_binding_error(
        bad_link,
        KernelBindingError::Inspection(InspectionError::InvalidElf(
            "object parser rejected the file",
        )),
    );

    let mut wrong_link_type = fixture.bytes.clone();
    write_u32(&mut wrong_link_type, fixture.symtab_header + 40, 3);
    assert_binding_error(
        wrong_link_type,
        KernelBindingError::Inspection(InspectionError::InvalidElf(
            "object parser rejected the file",
        )),
    );

    let mut bad_info = fixture.bytes.clone();
    write_u32(&mut bad_info, fixture.symtab_header + 44, 5);
    assert_binding_error(
        bad_info,
        KernelBindingError::InvalidSymbolTable("invalid first non-local symbol index"),
    );

    let mut bad_name_offset = fixture.bytes.clone();
    write_u32(&mut bad_name_offset, fixture.descriptor_symbol, u32::MAX);
    assert_binding_error(
        bad_name_offset,
        KernelBindingError::InvalidSymbolTable("symbol name offset is out of bounds"),
    );

    let mut unterminated = fixture.bytes.clone();
    unterminated[fixture.strtab_end - 1] = b'x';
    assert_binding_error(
        unterminated,
        KernelBindingError::InvalidSymbolTable("symbol name is not NUL-terminated"),
    );

    let mut non_utf8 = fixture.bytes.clone();
    non_utf8[fixture.other_name] = 0xff;
    assert_binding_error(
        non_utf8,
        KernelBindingError::InvalidSymbolTable("symbol name is not UTF-8"),
    );

    let mut non_null_first = fixture.bytes.clone();
    non_null_first[fixture.symtab_offset + 8] = 1;
    assert_binding_error(
        non_null_first,
        KernelBindingError::InvalidSymbolTable("first symbol is not the null symbol"),
    );

    let mut nonzero_null_name = fixture.bytes.clone();
    write_u32(&mut nonzero_null_name, fixture.symtab_offset, 7);
    assert_binding_error(
        nonzero_null_name,
        KernelBindingError::InvalidSymbolTable("first symbol is not the null symbol"),
    );

    let mut bad_local_partition = fixture.bytes.clone();
    bad_local_partition[fixture.descriptor_symbol + 4] = 1;
    assert_binding_error(
        bad_local_partition,
        KernelBindingError::InvalidSymbolTable("local symbols do not match sh_info"),
    );
}

#[test]
fn requires_an_all_zero_sht_null_section_header_zero() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut section_type = fixture.bytes.clone();
    write_u32(&mut section_type, fixture.section_header_zero + 4, 1);
    assert_binding_error(
        section_type,
        KernelBindingError::InvalidSymbolTable(
            "section header zero is not an all-zero SHT_NULL record",
        ),
    );

    let mut section_offset = fixture.bytes.clone();
    write_u64(&mut section_offset, fixture.section_header_zero + 24, 1);
    assert_binding_error(
        section_offset,
        KernelBindingError::InvalidSymbolTable(
            "section header zero is not an all-zero SHT_NULL record",
        ),
    );
}

#[test]
fn rejects_missing_duplicate_and_mistyped_exact_symbols() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut missing_descriptor = fixture.bytes.clone();
    write_u32(
        &mut missing_descriptor,
        fixture.descriptor_symbol,
        fixture.other_name_index,
    );
    assert_binding_error(
        missing_descriptor,
        KernelBindingError::MissingDescriptorSymbol,
    );

    let mut duplicate_descriptor = fixture.bytes.clone();
    let descriptor =
        duplicate_descriptor[fixture.descriptor_symbol..fixture.descriptor_symbol + 24].to_vec();
    duplicate_descriptor[fixture.spare_symbol..fixture.spare_symbol + 24]
        .copy_from_slice(&descriptor);
    assert_binding_error(
        duplicate_descriptor,
        KernelBindingError::AmbiguousDescriptorSymbol,
    );

    let mut descriptor_type = fixture.bytes.clone();
    descriptor_type[fixture.descriptor_symbol + 4] = 0x12;
    assert_binding_error(
        descriptor_type,
        KernelBindingError::InvalidDescriptorSymbol("symbol type is not STT_OBJECT"),
    );

    let mut descriptor_size = fixture.bytes.clone();
    write_u64(&mut descriptor_size, fixture.descriptor_symbol + 16, 63);
    assert_binding_error(
        descriptor_size,
        KernelBindingError::InvalidDescriptorSymbol("symbol size is not 64 bytes"),
    );

    let mut descriptor_alignment = fixture.bytes.clone();
    write_u64(
        &mut descriptor_alignment,
        fixture.descriptor_symbol + 8,
        fixture.descriptor_address + 8,
    );
    assert_binding_error(
        descriptor_alignment,
        KernelBindingError::InvalidDescriptorSymbol("symbol address is not 64-byte aligned"),
    );

    let mut missing_entry = fixture.bytes.clone();
    write_u32(
        &mut missing_entry,
        fixture.entry_symbol,
        fixture.other_name_index,
    );
    assert_binding_error(missing_entry, KernelBindingError::MissingEntrySymbol);

    let mut duplicate_entry = fixture.bytes.clone();
    let entry = duplicate_entry[fixture.entry_symbol..fixture.entry_symbol + 24].to_vec();
    duplicate_entry[fixture.spare_symbol..fixture.spare_symbol + 24].copy_from_slice(&entry);
    assert_binding_error(duplicate_entry, KernelBindingError::AmbiguousEntrySymbol);

    let mut entry_type = fixture.bytes.clone();
    entry_type[fixture.entry_symbol + 4] = 0x11;
    assert_binding_error(
        entry_type,
        KernelBindingError::InvalidEntrySymbol("symbol type is not STT_FUNC"),
    );

    let mut entry_size = fixture.bytes.clone();
    write_u64(&mut entry_size, fixture.entry_symbol + 16, 0);
    assert_binding_error(
        entry_size,
        KernelBindingError::InvalidEntrySymbol("function symbol has zero size"),
    );

    let mut unsupported_other = fixture.bytes.clone();
    unsupported_other[fixture.descriptor_symbol + 5] = 4;
    assert_binding_error(
        unsupported_other,
        KernelBindingError::InvalidDescriptorSymbol(
            "descriptor or entry symbol has unsupported st_other bits",
        ),
    );

    let mut visibility = fixture.bytes.clone();
    visibility[fixture.descriptor_symbol + 5] = 1;
    assert_binding_error(
        visibility,
        KernelBindingError::InvalidDescriptorSymbol(
            "descriptor and entry symbol visibility is inconsistent",
        ),
    );
}

#[test]
fn requires_256_byte_aligned_entry_symbol_addresses() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    let mut bytes = fixture.bytes.clone();
    let unaligned_entry_address = fixture.entry_address + 4;
    write_u64(
        &mut bytes,
        fixture.entry_symbol + 8,
        unaligned_entry_address,
    );
    write_u64(
        &mut bytes,
        fixture.text_header + 16,
        unaligned_entry_address,
    );
    write_u64(
        &mut bytes,
        fixture.second_program_header + 16,
        unaligned_entry_address,
    );
    write_u64(&mut bytes, fixture.second_program_header + 48, 1);
    write_i64(
        &mut bytes,
        fixture.descriptor_offset + 16,
        i64::try_from(unaligned_entry_address - fixture.descriptor_address).unwrap(),
    );
    assert_binding_error(
        bytes,
        KernelBindingError::InvalidEntrySymbol("function address is not 256-byte aligned"),
    );
}

#[test]
fn restricts_kernel_symbols_to_compatible_global_or_weak_bindings() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut weak = fixture.bytes.clone();
    weak[fixture.entry_symbol + 4] = 0x22;
    weak[fixture.descriptor_symbol + 4] = 0x21;
    inspect_and_bind_kernel_descriptors(&weak).unwrap();

    let mut mixed_global_weak = fixture.bytes.clone();
    mixed_global_weak[fixture.descriptor_symbol + 4] = 0x21;
    assert_binding_error(
        mixed_global_weak,
        KernelBindingError::InvalidDescriptorSymbol("descriptor and entry symbol bindings differ"),
    );

    let mut local = fixture.bytes.clone();
    write_u32(&mut local, fixture.symtab_header + 44, 3);
    local[fixture.entry_symbol + 4] = 0x02;
    local[fixture.descriptor_symbol + 4] = 0x01;
    assert_binding_error(
        local,
        KernelBindingError::InvalidDescriptorSymbol("symbol binding is not STB_GLOBAL or STB_WEAK"),
    );

    let mut os_reserved = fixture.bytes.clone();
    os_reserved[fixture.descriptor_symbol + 4] = 0xa1;
    assert_binding_error(
        os_reserved,
        KernelBindingError::InvalidDescriptorSymbol("symbol binding is not STB_GLOBAL or STB_WEAK"),
    );

    let mut processor_reserved = fixture.bytes.clone();
    processor_reserved[fixture.entry_symbol + 4] = 0xd2;
    assert_binding_error(
        processor_reserved,
        KernelBindingError::InvalidEntrySymbol("symbol binding is not STB_GLOBAL or STB_WEAK"),
    );
}

#[test]
fn rejects_wrong_symbol_sections_and_load_mappings() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut descriptor_section = fixture.bytes.clone();
    write_u64(&mut descriptor_section, fixture.rodata_header + 8, 0x3);
    assert_binding_error(
        descriptor_section,
        KernelBindingError::InvalidDescriptorSymbol(
            "descriptor section is not uncompressed read-only allocated PROGBITS",
        ),
    );

    let mut descriptor_section_alignment = fixture.bytes.clone();
    write_u64(
        &mut descriptor_section_alignment,
        fixture.rodata_header + 48,
        32,
    );
    assert_binding_error(
        descriptor_section_alignment,
        KernelBindingError::InvalidDescriptorSymbol(
            "descriptor section alignment is less than 64 bytes",
        ),
    );

    let mut entry_section = fixture.bytes.clone();
    write_u64(&mut entry_section, fixture.text_header + 8, 0x2);
    assert_binding_error(
        entry_section,
        KernelBindingError::InvalidEntrySymbol(
            "entry section is not uncompressed read-only executable PROGBITS",
        ),
    );

    let mut entry_section_alignment = fixture.bytes.clone();
    write_u64(&mut entry_section_alignment, fixture.text_header + 48, 3);
    assert_binding_error(
        entry_section_alignment,
        KernelBindingError::InvalidEntrySymbol("entry section alignment is invalid"),
    );

    let mut descriptor_unmapped = fixture.bytes.clone();
    write_u64(
        &mut descriptor_unmapped,
        fixture.first_program_header + 32,
        fixture.descriptor_offset as u64 + 63,
    );
    write_u64(
        &mut descriptor_unmapped,
        fixture.first_program_header + 40,
        fixture.descriptor_offset as u64 + 63,
    );
    assert_binding_error(
        descriptor_unmapped,
        KernelBindingError::InvalidLoadMapping(
            "requested virtual range only partially intersects PT_LOAD",
        ),
    );

    let mut descriptor_executable = fixture.bytes.clone();
    write_u32(
        &mut descriptor_executable,
        fixture.first_program_header + 4,
        5,
    );
    assert_binding_error(
        descriptor_executable,
        KernelBindingError::InvalidLoadMapping("mapped PT_LOAD has inappropriate permissions"),
    );

    let mut entry_writable = fixture.bytes.clone();
    write_u32(&mut entry_writable, fixture.second_program_header + 4, 7);
    assert_binding_error(
        entry_writable,
        KernelBindingError::InvalidLoadMapping("mapped PT_LOAD has inappropriate permissions"),
    );

    let mut overlap = fixture.bytes.clone();
    write_u64(
        &mut overlap,
        fixture.text_header + 24,
        fixture.descriptor_offset as u64,
    );
    write_u64(
        &mut overlap,
        fixture.second_program_header + 8,
        fixture.descriptor_offset as u64,
    );
    write_u64(&mut overlap, fixture.second_program_header + 48, 1);
    assert_binding_error(
        overlap,
        KernelBindingError::InvalidLoadMapping("descriptor and entry file ranges overlap"),
    );
}

#[test]
fn rejects_additional_pt_load_memory_intersections_including_zero_fill() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut zero_fill_overlap = fixture.bytes.clone();
    write_zero_fill_load(
        &mut zero_fill_overlap,
        fixture.third_program_header,
        4,
        fixture.descriptor_address,
        64,
    );
    assert_binding_error(
        zero_fill_overlap,
        KernelBindingError::InvalidLoadMapping("address has ambiguous PT_LOAD memory mappings"),
    );

    let mut wrong_permissions = fixture.bytes.clone();
    write_zero_fill_load(
        &mut wrong_permissions,
        fixture.third_program_header,
        6,
        fixture.descriptor_address,
        64,
    );
    assert_binding_error(
        wrong_permissions,
        KernelBindingError::InvalidLoadMapping("mapped PT_LOAD has inappropriate permissions"),
    );

    for (address, memory_size) in [
        (fixture.descriptor_address - 64, 64),
        (fixture.descriptor_address + 64, 64),
    ] {
        let mut adjacent = fixture.bytes.clone();
        write_zero_fill_load(
            &mut adjacent,
            fixture.third_program_header,
            4,
            address,
            memory_size,
        );
        inspect_and_bind_kernel_descriptors(&adjacent).unwrap();
    }

    for (address, memory_size) in [
        (fixture.descriptor_address - 64, 65),
        (fixture.descriptor_address + 63, 64),
    ] {
        let mut one_byte_overlap = fixture.bytes.clone();
        write_zero_fill_load(
            &mut one_byte_overlap,
            fixture.third_program_header,
            4,
            address,
            memory_size,
        );
        assert_binding_error(
            one_byte_overlap,
            KernelBindingError::InvalidLoadMapping("address has ambiguous PT_LOAD memory mappings"),
        );
    }
}

#[test]
fn uses_checked_signed_entry_address_arithmetic() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));

    let mut mismatch = fixture.bytes.clone();
    write_i64(&mut mismatch, fixture.descriptor_offset + 16, 4);
    assert_binding_error(
        mismatch,
        KernelBindingError::InvalidKernelDescriptor(
            "entry offset does not resolve to the function symbol",
        ),
    );

    let mut underflow = fixture.bytes.clone();
    write_i64(&mut underflow, fixture.descriptor_offset + 16, i64::MIN);
    assert_binding_error(
        underflow,
        KernelBindingError::InvalidKernelDescriptor("entry address arithmetic overflows"),
    );

    let mut negative = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    let high_load_address = 0x4000;
    let high_descriptor_address = high_load_address + negative.descriptor_offset as u64;
    write_u64(
        &mut negative.bytes,
        negative.first_program_header + 16,
        high_load_address,
    );
    write_u64(
        &mut negative.bytes,
        negative.rodata_header + 16,
        high_descriptor_address,
    );
    write_u64(
        &mut negative.bytes,
        negative.descriptor_symbol + 8,
        high_descriptor_address,
    );
    write_i64(
        &mut negative.bytes,
        negative.descriptor_offset + 16,
        i64::try_from(negative.entry_address).unwrap()
            - i64::try_from(high_descriptor_address).unwrap(),
    );
    let bound = inspect_and_bind_kernel_descriptors(&negative.bytes).unwrap();
    assert!(
        bound.bindings()[0]
            .descriptor()
            .kernel_code_entry_byte_offset()
            < 0
    );

    let mut overflow = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    let descriptor_address = u64::MAX - 127;
    let load_address = descriptor_address - overflow.descriptor_offset as u64;
    write_u64(
        &mut overflow.bytes,
        overflow.first_program_header + 16,
        load_address,
    );
    write_u64(&mut overflow.bytes, overflow.first_program_header + 48, 1);
    write_u64(
        &mut overflow.bytes,
        overflow.rodata_header + 16,
        descriptor_address,
    );
    write_u64(
        &mut overflow.bytes,
        overflow.descriptor_symbol + 8,
        descriptor_address,
    );
    write_i64(
        &mut overflow.bytes,
        overflow.descriptor_offset + 16,
        i64::MAX,
    );
    assert_binding_error(
        overflow.bytes,
        KernelBindingError::InvalidKernelDescriptor("entry address arithmetic overflows"),
    );
}

#[test]
fn rejects_reserved_descriptor_bytes_bits_and_preload() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    for relative in (12..16).chain(24..44).chain(60..64) {
        let mut bytes = fixture.bytes.clone();
        bytes[fixture.descriptor_offset + relative] = 1;
        assert_binding_error(
            bytes,
            KernelBindingError::InvalidKernelDescriptor("reserved descriptor bytes are nonzero"),
        );
    }
    for bit in [7, 8, 9, 12, 13, 14, 15] {
        let mut bytes = fixture.bytes.clone();
        let properties = read_u16(&bytes, fixture.descriptor_offset + 56) | (1 << bit);
        write_u16(&mut bytes, fixture.descriptor_offset + 56, properties);
        assert_binding_error(
            bytes,
            KernelBindingError::InvalidKernelDescriptor(
                "reserved kernel-code-property bits are nonzero",
            ),
        );
    }

    let mut preload = fixture.bytes.clone();
    write_u16(&mut preload, fixture.descriptor_offset + 58, 1);
    assert_binding_error(
        preload,
        KernelBindingError::InvalidKernelDescriptor("kernarg preload is unsupported"),
    );

    for (field_offset, bit, reason) in [
        (
            48,
            28,
            "reserved or unsupported COMPUTE_PGM_RSRC1 bits are nonzero",
        ),
        (
            52,
            31,
            "HSA-fixed or reserved COMPUTE_PGM_RSRC2 bits are nonzero",
        ),
        (44, 12, "target-reserved COMPUTE_PGM_RSRC3 bits are nonzero"),
    ] {
        let mut bytes = fixture.bytes.clone();
        let value = read_u32(&bytes, fixture.descriptor_offset + field_offset) | (1 << bit);
        write_u32(&mut bytes, fixture.descriptor_offset + field_offset, value);
        assert_binding_error(bytes, KernelBindingError::InvalidKernelDescriptor(reason));
    }

    let mut gfx9_document = metadata((1, 2), vec![valid_kernel("vecadd", "vecadd.kd")]);
    let kernels = &mut as_map_mut(&mut gfx9_document)
        .iter_mut()
        .find(|(key, _)| key.as_str() == Some("amdhsa.kernels"))
        .unwrap()
        .1;
    let Value::Array(kernels) = kernels else {
        panic!("expected kernels array");
    };
    set_field(&mut kernels[0], ".vgpr_count", Value::from(11));
    set_field(
        &mut gfx9_document,
        "amdhsa.target",
        Value::from("amdgcn-amd-amdhsa--gfx942"),
    );
    let mut gfx9 = binding_fixture_for_document(gfx9_document);
    write_u32(&mut gfx9.bytes, 48, 0x54c);
    write_u32(&mut gfx9.bytes, gfx9.descriptor_offset + 44, 1);
    write_u32(&mut gfx9.bytes, gfx9.descriptor_offset + 48, 0x00af_0081);
    assert_binding_error(
        gfx9.bytes,
        KernelBindingError::InvalidKernelDescriptor("wave32 property is reserved before GFX10"),
    );
}

#[test]
fn cross_checks_descriptor_metadata_and_resource_invariants() {
    let fixture = binding_fixture(valid_kernel("vecadd", "vecadd.kd"));
    for (offset, value, field) in [
        (0, 1, ".group_segment_fixed_size"),
        (4, 0, ".private_segment_fixed_size"),
        (8, 271, ".kernarg_segment_size"),
    ] {
        let mut bytes = fixture.bytes.clone();
        write_u32(&mut bytes, fixture.descriptor_offset + offset, value);
        assert_binding_error(bytes, KernelBindingError::MetadataMismatch(field));
    }

    let mut wave = fixture.bytes.clone();
    write_u16(&mut wave, fixture.descriptor_offset + 56, 0x001e);
    assert_binding_error(
        wave,
        KernelBindingError::MetadataMismatch(".wavefront_size"),
    );

    let mut dynamic = fixture.bytes.clone();
    write_u16(&mut dynamic, fixture.descriptor_offset + 56, 0x0c1e);
    assert_binding_error(
        dynamic,
        KernelBindingError::MetadataMismatch(".uses_dynamic_stack"),
    );

    let mut private_enable = fixture.bytes.clone();
    write_u32(&mut private_enable, fixture.descriptor_offset + 52, 0x1390);
    assert_binding_error(
        private_enable,
        KernelBindingError::MetadataMismatch("private-segment enablement"),
    );

    let mut wgp = fixture.bytes.clone();
    write_u32(&mut wgp, fixture.descriptor_offset + 48, 0xc0af_0000);
    assert_binding_error(
        wgp,
        KernelBindingError::MetadataMismatch(".workgroup_processor_mode"),
    );

    let mut high_vgpr = valid_kernel("vecadd", "vecadd.kd");
    set_field(&mut high_vgpr, ".vgpr_count", Value::from(9));
    let high_vgpr = binding_fixture(high_vgpr);
    assert_binding_error(
        high_vgpr.bytes,
        KernelBindingError::MetadataMismatch(".vgpr_count"),
    );
}

#[test]
fn rejects_legacy_scratch_properties_on_architected_flat_scratch_targets() {
    for processor in ["gfx1151", "gfx942", "gfx950", "gfx1250"] {
        let valid = binding_fixture_for_target(processor, 14);
        inspect_and_bind_kernel_descriptors(&valid.bytes).unwrap();

        for property_bit in [0, 5] {
            let mut bytes = valid.bytes.clone();
            let properties = read_u16(&bytes, valid.descriptor_offset + 56) | (1 << property_bit);
            write_u16(&mut bytes, valid.descriptor_offset + 56, properties);
            assert_binding_error(
                bytes,
                KernelBindingError::InvalidKernelDescriptor(
                    "architected flat scratch forbids private-buffer and flat-scratch-init properties",
                ),
            );
        }
    }
}

#[test]
fn enforces_pinned_sgpr_capacity_and_pre_gfx10_encoding() {
    for processor in ["gfx1151", "gfx1250"] {
        let accepted = binding_fixture_for_target(processor, 128);
        inspect_and_bind_kernel_descriptors(&accepted.bytes).unwrap();

        let rejected = binding_fixture_for_target(processor, 129);
        assert_binding_error(
            rejected.bytes,
            KernelBindingError::MetadataMismatch(".sgpr_count"),
        );
    }

    let gfx942_boundary = binding_fixture_for_target("gfx942", 24);
    assert_eq!(
        (read_u32(
            &gfx942_boundary.bytes,
            gfx942_boundary.descriptor_offset + 48
        ) >> 6)
            & 0xf,
        2
    );
    inspect_and_bind_kernel_descriptors(&gfx942_boundary.bytes).unwrap();

    let gfx942_too_many = binding_fixture_for_target("gfx942", 25);
    assert_binding_error(
        gfx942_too_many.bytes,
        KernelBindingError::MetadataMismatch(".sgpr_count"),
    );

    let mut gfx942_max = binding_fixture_for_target("gfx942", 112);
    set_sgpr_block_field(&mut gfx942_max, 13);
    inspect_and_bind_kernel_descriptors(&gfx942_max.bytes).unwrap();

    let mut gfx942_over_max = binding_fixture_for_target("gfx942", 113);
    set_sgpr_block_field(&mut gfx942_over_max, 13);
    assert_binding_error(
        gfx942_over_max.bytes,
        KernelBindingError::MetadataMismatch(".sgpr_count"),
    );

    for processor in ["gfx600", "gfx700", "gfx803", "gfx900", "gfx942"] {
        let mut odd_field = binding_fixture_for_target(processor, 32);
        set_sgpr_block_field(&mut odd_field, 3);
        inspect_and_bind_kernel_descriptors(&odd_field.bytes)
            .unwrap_or_else(|error| panic!("{processor}: {error:?}"));

        let mut over_limit = binding_fixture_for_target(processor, 32);
        set_sgpr_block_field(&mut over_limit, 14);
        assert_binding_error(
            over_limit.bytes,
            KernelBindingError::InvalidKernelDescriptor(
                "pre-GFX10 SGPR block field exceeds the pinned 112-register limit",
            ),
        );
    }

    let unpinned = binding_fixture_for_target("gfx1310", 14);
    assert_binding_error(
        unpinned.bytes,
        KernelBindingError::InvalidKernelDescriptor("target SGPR capacity is not pinned"),
    );
}

#[test]
fn requires_a_binding_for_every_metadata_kernel() {
    let document = metadata(
        (1, 2),
        vec![
            valid_kernel("vecadd", "vecadd.kd"),
            valid_kernel("other_kernel", "other_kernel.kd"),
        ],
    );
    let fixture = binding_fixture_for_document(document);
    assert_binding_error(fixture.bytes, KernelBindingError::MissingDescriptorSymbol);
}

#[test]
fn preserves_observed_gfx1151_gfx942_and_gfx950_descriptor_words() {
    let mut local_kernel = valid_kernel("vecadd", "vecadd.kd");
    let mut local_arguments = (0..6)
        .map(|index| argument(Some(&format!("arg{index}")), index * 8, 8, "by_value", None))
        .collect::<Vec<_>>();
    local_arguments.extend(v5_hidden_arguments(48));
    set_field(&mut local_kernel, ".args", Value::Array(local_arguments));
    set_field(&mut local_kernel, ".kernarg_segment_size", Value::from(304));
    let mut fixture = binding_fixture(local_kernel);
    write_u32(&mut fixture.bytes, fixture.descriptor_offset + 8, 304);
    let local = inspect_and_bind_kernel_descriptors(&fixture.bytes).unwrap();
    let local_binding = local.bindings()[0];
    assert_eq!(local_binding.descriptor_address(), 0x9c0);
    assert_eq!(local_binding.entry_address(), 0x1a00);
    let local = local_binding.descriptor();
    assert_eq!(
        (
            local.private_segment_fixed_size(),
            local.kernarg_size(),
            local.compute_pgm_rsrc3(),
            local.compute_pgm_rsrc1(),
            local.compute_pgm_rsrc2(),
            local.kernel_code_properties(),
        ),
        (16, 304, 0x40, 0xe0af_0000, 0x1391, 0x041e)
    );
    assert_eq!(local.kernel_code_entry_byte_offset(), 0x1040);

    for target in ["gfx942", "gfx950"] {
        let mut kernel = valid_kernel("vecadd", "vecadd.kd");
        let mut arguments = (0..4)
            .map(|index| argument(Some(&format!("arg{index}")), index * 8, 8, "by_value", None))
            .collect::<Vec<_>>();
        arguments.extend(v5_hidden_arguments(32));
        set_field(&mut kernel, ".args", Value::Array(arguments));
        set_field(&mut kernel, ".private_segment_fixed_size", Value::from(0));
        set_field(&mut kernel, ".kernarg_segment_size", Value::from(288));
        set_field(&mut kernel, ".wavefront_size", Value::from(64));
        set_field(&mut kernel, ".vgpr_count", Value::from(11));
        remove_field(&mut kernel, ".workgroup_processor_mode");
        let mut document = metadata((1, 2), vec![kernel]);
        set_field(
            &mut document,
            "amdhsa.target",
            Value::from(format!("amdgcn-amd-amdhsa--{target}")),
        );
        let mut fixture = binding_fixture_for_document_at(document, 0x900);
        write_u32(
            &mut fixture.bytes,
            48,
            if target == "gfx942" { 0x54c } else { 0x54f },
        );
        write_u32(&mut fixture.bytes, fixture.descriptor_offset + 4, 0);
        write_u32(&mut fixture.bytes, fixture.descriptor_offset + 8, 288);
        write_u32(&mut fixture.bytes, fixture.descriptor_offset + 44, 1);
        write_u32(
            &mut fixture.bytes,
            fixture.descriptor_offset + 48,
            0x00af_0081,
        );
        write_u32(&mut fixture.bytes, fixture.descriptor_offset + 52, 0x1390);
        write_u16(&mut fixture.bytes, fixture.descriptor_offset + 56, 0x001e);
        let bound = inspect_and_bind_kernel_descriptors(&fixture.bytes).unwrap();
        let binding = bound.bindings()[0];
        assert_eq!(binding.descriptor_address(), 0x900);
        assert_eq!(binding.entry_address(), 0x1a00);
        let descriptor = binding.descriptor();
        assert_eq!(descriptor.private_segment_fixed_size(), 0);
        assert_eq!(descriptor.kernarg_size(), 288);
        assert_eq!(descriptor.compute_pgm_rsrc3(), 1);
        assert_eq!(descriptor.compute_pgm_rsrc1(), 0x00af_0081);
        assert_eq!(descriptor.compute_pgm_rsrc2(), 0x1390);
        assert_eq!(descriptor.kernel_code_properties(), 0x001e);
        assert_eq!(descriptor.kernel_code_entry_byte_offset(), 0x1100);
    }
}

#[test]
#[ignore = "requires FE2O3_TEST_HSACO to name a generated vecadd HSACO"]
fn inspects_real_generated_vecadd_hsaco() {
    let path = env::var("FE2O3_TEST_HSACO").expect("set FE2O3_TEST_HSACO");
    let expected_target = env::var("FE2O3_TEST_TARGET").unwrap_or_else(|_| "gfx1151".to_owned());
    let expected_wavefront = env::var("FE2O3_TEST_WAVEFRONT")
        .unwrap_or_else(|_| "32".to_owned())
        .parse::<u32>()
        .expect("FE2O3_TEST_WAVEFRONT must be a u32");
    let metadata_expectation =
        generated_metadata_expectation(env::var(METADATA_PROFILE_ENV).ok().as_deref())
            .unwrap_or_else(|error| panic!("{error}"));
    let bytes = fs::read(path).unwrap();
    let inspected = inspect(&bytes).unwrap();
    let bound = inspect_and_bind_kernel_descriptors(&bytes).unwrap();
    assert_eq!(bound.inspection(), &inspected);
    assert_eq!(bound.bindings().len(), inspected.kernels().len());
    assert_eq!(inspected.target().to_string(), expected_target);
    assert!(!inspected.has_printf_metadata());

    let kernel = inspected
        .kernels()
        .iter()
        .find(|kernel| kernel.name() == "vecadd")
        .unwrap();
    assert_eq!(kernel.symbol(), "vecadd.kd");
    assert_eq!(kernel.kernarg_segment_size(), 304);
    assert_eq!(kernel.kernarg_segment_alignment(), 8);
    assert_eq!(kernel.group_segment_fixed_size(), 0);
    assert_eq!(kernel.wavefront_size(), expected_wavefront);
    assert!(kernel.sgpr_count() > 0);
    assert!(kernel.vgpr_count() > 0);
    assert!(kernel.sgpr_spill_count().is_some());
    assert!(kernel.vgpr_spill_count().is_some());
    let processor = expected_target.split(':').next().unwrap();
    if matches!(processor, "gfx1250" | "gfx1251") {
        assert_eq!(kernel.gfx1250_revision(), Some(Gfx1250Revision::B0));
    } else {
        // The pinned LLVM source can expose its temporary global B0 switch on
        // other processors; rustc-bundled LLVM builds may omit that marker.
        assert!(matches!(
            kernel.gfx1250_revision(),
            None | Some(Gfx1250Revision::B0)
        ));
    }
    assert_eq!(
        kernel.max_flat_workgroup_size(),
        metadata_expectation.max_flat_workgroup_size
    );
    assert_eq!(
        kernel.required_workgroup_size(),
        metadata_expectation.required_workgroup_size
    );
    assert_eq!(kernel.max_workgroups(), [None, None, None]);
    assert_eq!(kernel.cluster_dims(), None);
    assert_eq!(kernel.kind(), KernelKind::Normal);
    assert!(!kernel.uniform_work_group_size());
    assert!(!kernel.uses_dynamic_stack());
    assert_eq!(kernel.device_enqueue_symbol(), None);
    assert_eq!(kernel.implicit_argument_offset(), Some(48));
    assert_eq!(kernel.implicit_argument_size(), 256);

    let descriptor = bound.bindings()[0].descriptor();
    assert_eq!(descriptor.group_segment_fixed_size(), 0);
    assert_eq!(
        u64::from(descriptor.private_segment_fixed_size()),
        kernel.private_segment_fixed_size()
    );
    assert_eq!(
        u64::from(descriptor.kernarg_size()),
        kernel.kernarg_segment_size()
    );
    assert_eq!(descriptor.wavefront_size(), expected_wavefront);
    assert_eq!(descriptor.uses_dynamic_stack(), kernel.uses_dynamic_stack());
    assert_eq!(descriptor.kernarg_preload(), 0);
    match processor {
        "gfx1151" => {
            assert_eq!(descriptor.compute_pgm_rsrc3(), 0x40);
            assert_eq!(descriptor.compute_pgm_rsrc1(), 0xe0af_0000);
            assert_eq!(descriptor.compute_pgm_rsrc2(), 0x1391);
            assert_eq!(descriptor.kernel_code_properties(), 0x041e);
        }
        "gfx942" | "gfx950" => {
            assert_eq!(descriptor.compute_pgm_rsrc3(), 1);
            assert_eq!(descriptor.compute_pgm_rsrc1(), 0x00af_0081);
            assert_eq!(descriptor.compute_pgm_rsrc2(), 0x1390);
            assert_eq!(descriptor.kernel_code_properties(), 0x001e);
        }
        _ => {}
    }

    let explicit = kernel.explicit_arguments();
    assert_eq!(explicit.len(), 6);
    assert_eq!(
        explicit.iter().map(|arg| arg.offset()).collect::<Vec<_>>(),
        [0, 8, 16, 24, 32, 40]
    );
    assert!(explicit.iter().all(|argument| argument.access().is_none()));
    for (index, argument) in explicit.iter().enumerate() {
        let expected = if index.is_multiple_of(2) {
            Some(ArgumentAddressSpace::Global)
        } else {
            None
        };
        assert_eq!(argument.address_space(), expected);
    }

    let hidden = kernel.hidden_arguments();
    assert_eq!(hidden.len(), 19);
    let expected_hidden = [
        (48, 4, HiddenValueKind::BlockCountX),
        (52, 4, HiddenValueKind::BlockCountY),
        (56, 4, HiddenValueKind::BlockCountZ),
        (60, 2, HiddenValueKind::GroupSizeX),
        (62, 2, HiddenValueKind::GroupSizeY),
        (64, 2, HiddenValueKind::GroupSizeZ),
        (66, 2, HiddenValueKind::RemainderX),
        (68, 2, HiddenValueKind::RemainderY),
        (70, 2, HiddenValueKind::RemainderZ),
        (88, 8, HiddenValueKind::GlobalOffsetX),
        (96, 8, HiddenValueKind::GlobalOffsetY),
        (104, 8, HiddenValueKind::GlobalOffsetZ),
        (112, 2, HiddenValueKind::GridDimensions),
        (128, 8, HiddenValueKind::HostcallBuffer),
        (136, 8, HiddenValueKind::MultigridSyncArgument),
        (144, 8, HiddenValueKind::HeapV1),
        (152, 8, HiddenValueKind::DefaultQueue),
        (160, 8, HiddenValueKind::CompletionAction),
        (248, 8, HiddenValueKind::QueuePointer),
    ];
    assert_eq!(
        hidden
            .iter()
            .map(|argument| (argument.offset(), argument.size(), argument.value_kind()))
            .collect::<Vec<_>>(),
        expected_hidden
    );
}

fn assert_argument_error(arguments: Vec<Value>, expected: InspectionError) {
    let mut kernel = valid_kernel("k", "k.kd");
    set_field(&mut kernel, ".args", Value::Array(arguments));
    set_field(&mut kernel, ".kernarg_segment_size", Value::from(1024));
    let value = metadata((1, 2), vec![kernel]);
    assert_eq!(
        inspect(&hsaco(&encode(&value), 4, &[b"AMDGPU\0"])),
        Err(expected)
    );
}

fn assert_kernel_error(
    kernel: Value,
    abi_version: u8,
    metadata_version: (u32, u32),
    expected: InspectionError,
) {
    let document = metadata(metadata_version, vec![kernel]);
    assert_eq!(
        inspect(&hsaco(&encode(&document), abi_version, &[b"AMDGPU\0"])),
        Err(expected)
    );
}

fn inspect_arguments_for_version(
    arguments: Vec<Value>,
    abi_version: u8,
    metadata_version: (u32, u32),
) -> Result<fe2o3_hsaco::InspectedHsaco, InspectionError> {
    let mut kernel = if abi_version == 2 {
        valid_v4_kernel("k", "k.kd")
    } else {
        valid_kernel("k", "k.kd")
    };
    set_field(&mut kernel, ".args", Value::Array(arguments));
    set_field(
        &mut kernel,
        ".kernarg_segment_size",
        Value::from(if abi_version == 2 { 1024 } else { 256 }),
    );
    let document = metadata(metadata_version, vec![kernel]);
    inspect(&hsaco(&encode(&document), abi_version, &[b"AMDGPU\0"]))
}

fn assert_arguments_error_for_version(
    arguments: Vec<Value>,
    abi_version: u8,
    metadata_version: (u32, u32),
    expected: InspectionError,
) {
    assert_eq!(
        inspect_arguments_for_version(arguments, abi_version, metadata_version),
        Err(expected)
    );
}

fn push_u32_triplet(map: &mut Value, field: &str, values: [u32; 3]) {
    as_map_mut(map).push((
        Value::from(field),
        Value::Array(values.into_iter().map(Value::from).collect()),
    ));
}

fn valid_hsaco() -> Vec<u8> {
    let metadata = metadata((1, 2), vec![valid_kernel("vecadd", "vecadd.kd")]);
    hsaco(&encode(&metadata), 4, &[b"AMDGPU\0"])
}

fn metadata(version: (u32, u32), kernels: Vec<Value>) -> Value {
    map(vec![
        (
            "amdhsa.version",
            Value::Array(vec![Value::from(version.0), Value::from(version.1)]),
        ),
        ("amdhsa.target", Value::from("amdgcn-amd-amdhsa--gfx1151")),
        ("amdhsa.kernels", Value::Array(kernels)),
    ])
}

fn valid_kernel(name: &str, symbol: &str) -> Value {
    let mut arguments = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(v5_hidden_arguments(16));
    map(vec![
        (".name", Value::from(name)),
        (".symbol", Value::from(symbol)),
        (".args", Value::Array(arguments)),
        (".kernarg_segment_size", Value::from(272)),
        (".kernarg_segment_align", Value::from(8)),
        (".group_segment_fixed_size", Value::from(0)),
        (".private_segment_fixed_size", Value::from(16)),
        (".wavefront_size", Value::from(32)),
        (".sgpr_count", Value::from(14)),
        (".vgpr_count", Value::from(7)),
        (".agpr_count", Value::from(3)),
        (".sgpr_spill_count", Value::from(2)),
        (".vgpr_spill_count", Value::from(4)),
        (".workgroup_processor_mode", Value::from(1)),
        (".max_flat_workgroup_size", Value::from(1024)),
    ])
}

fn valid_v4_kernel(name: &str, symbol: &str) -> Value {
    let mut kernel = valid_kernel(name, symbol);
    let mut arguments = vec![
        argument(Some("a_ptr"), 0, 8, "global_buffer", Some("global")),
        argument(Some("a_len"), 8, 8, "by_value", None),
    ];
    arguments.extend(v4_hidden_arguments(16));
    set_field(&mut kernel, ".args", Value::Array(arguments));
    set_field(&mut kernel, ".kernarg_segment_size", Value::from(72));
    remove_field(&mut kernel, ".workgroup_processor_mode");
    kernel
}

fn v4_hidden_arguments(base: u64) -> Vec<Value> {
    [
        "hidden_global_offset_x",
        "hidden_global_offset_y",
        "hidden_global_offset_z",
        "hidden_none",
        "hidden_none",
        "hidden_none",
        "hidden_none",
    ]
    .into_iter()
    .enumerate()
    .map(|(index, kind)| {
        argument(
            None,
            base + u64::try_from(index).unwrap() * 8,
            8,
            kind,
            None,
        )
    })
    .collect()
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

fn map(fields: Vec<(&str, Value)>) -> Value {
    Value::Map(
        fields
            .into_iter()
            .map(|(key, value)| (Value::from(key), value))
            .collect(),
    )
}

fn as_map_mut(value: &mut Value) -> &mut Vec<(Value, Value)> {
    match value {
        Value::Map(map) => map,
        _ => panic!("expected map"),
    }
}

fn set_field(map: &mut Value, key: &str, replacement: Value) {
    let (_, value) = as_map_mut(map)
        .iter_mut()
        .find(|(candidate, _)| candidate.as_str() == Some(key))
        .unwrap();
    *value = replacement;
}

fn remove_field(map: &mut Value, key: &str) {
    as_map_mut(map).retain(|(candidate, _)| candidate.as_str() != Some(key));
}

fn encode(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_value(&mut bytes, value).unwrap();
    bytes
}

fn hsaco(metadata: &[u8], abi_version: u8, owners: &[&[u8]]) -> Vec<u8> {
    let mut note = Vec::new();
    for owner in owners {
        note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
        note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
        note.extend_from_slice(&32u32.to_le_bytes());
        note.extend_from_slice(owner);
        align(&mut note, 4);
        note.extend_from_slice(metadata);
        align(&mut note, 4);
    }

    let mut bytes = vec![0; ELF_HEADER_BYTES];
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    let string_table = b"\0.note\0.shstrtab\0";
    let string_table_offset = bytes.len();
    bytes.extend_from_slice(string_table);
    align(&mut bytes, 8);
    let section_offset = bytes.len();
    bytes.resize(section_offset + 3 * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = abi_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u32(&mut bytes, 48, 0x4a);
    write_u64(&mut bytes, 40, u64::try_from(section_offset).unwrap());
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 58, 64);
    write_u16(&mut bytes, 60, 3);
    write_u16(&mut bytes, 62, 2);

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(
        &mut bytes,
        note_header + 24,
        u64::try_from(note_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        note_header + 32,
        u64::try_from(note.len()).unwrap(),
    );
    write_u64(&mut bytes, note_header + 48, 4);

    let strings_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strings_header, 7);
    write_u32(&mut bytes, strings_header + 4, 3);
    write_u64(
        &mut bytes,
        strings_header + 24,
        u64::try_from(string_table_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        strings_header + 32,
        u64::try_from(string_table.len()).unwrap(),
    );
    write_u64(&mut bytes, strings_header + 48, 1);
    bytes
}

fn segment_only_hsaco(metadata: &[u8], abi_version: u8) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&32u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);

    let note_offset = ELF_HEADER_BYTES + 56;
    let mut bytes = vec![0; note_offset];
    bytes.extend_from_slice(&note);
    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = abi_version;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u32(&mut bytes, 48, 0x4a);
    write_u64(&mut bytes, 32, u64::try_from(ELF_HEADER_BYTES).unwrap());
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, 1);
    write_u16(&mut bytes, 58, 64);

    let program_header = ELF_HEADER_BYTES;
    write_u32(&mut bytes, program_header, 4);
    write_u32(&mut bytes, program_header + 4, 4);
    write_u64(
        &mut bytes,
        program_header + 8,
        u64::try_from(note_offset).unwrap(),
    );
    write_u64(
        &mut bytes,
        program_header + 32,
        u64::try_from(note.len()).unwrap(),
    );
    write_u64(
        &mut bytes,
        program_header + 40,
        u64::try_from(note.len()).unwrap(),
    );
    write_u64(&mut bytes, program_header + 48, 4);
    bytes
}

fn hsaco_with_shared_note_views(metadata: &[u8], abi_version: u8) -> Vec<u8> {
    let mut bytes = hsaco(metadata, abi_version, &[b"AMDGPU\0"]);
    let section_offset = usize::try_from(read_u64(&bytes, 40)).unwrap();
    let note_header = section_offset + SECTION_HEADER_BYTES;
    let note_offset = usize::try_from(read_u64(&bytes, note_header + 24)).unwrap();
    let note_size = usize::try_from(read_u64(&bytes, note_header + 32)).unwrap();
    add_note_segment(&mut bytes, note_offset, note_size);
    bytes
}

fn hsaco_with_distinct_note_views(
    section_metadata: &[u8],
    segment_metadata: &[u8],
    abi_version: u8,
) -> Vec<u8> {
    let mut bytes = hsaco(section_metadata, abi_version, &[b"AMDGPU\0"]);
    let note = metadata_note(segment_metadata);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    add_note_segment(&mut bytes, note_offset, note.len());
    bytes
}

fn metadata_note(metadata: &[u8]) -> Vec<u8> {
    let owner = b"AMDGPU\0";
    let mut note = Vec::new();
    note.extend_from_slice(&u32::try_from(owner.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&u32::try_from(metadata.len()).unwrap().to_le_bytes());
    note.extend_from_slice(&32u32.to_le_bytes());
    note.extend_from_slice(owner);
    align(&mut note, 4);
    note.extend_from_slice(metadata);
    align(&mut note, 4);
    note
}

fn add_note_segment(bytes: &mut Vec<u8>, note_offset: usize, note_size: usize) {
    align(bytes, 8);
    let program_header = bytes.len();
    bytes.resize(program_header + 56, 0);
    write_u64(bytes, 32, u64::try_from(program_header).unwrap());
    write_u16(bytes, 56, 1);
    write_u32(bytes, program_header, 4);
    write_u32(bytes, program_header + 4, 4);
    write_u64(
        bytes,
        program_header + 8,
        u64::try_from(note_offset).unwrap(),
    );
    write_u64(
        bytes,
        program_header + 32,
        u64::try_from(note_size).unwrap(),
    );
    write_u64(
        bytes,
        program_header + 40,
        u64::try_from(note_size).unwrap(),
    );
    write_u64(bytes, program_header + 48, 4);
}

struct BindingFixture {
    bytes: Vec<u8>,
    descriptor_offset: usize,
    descriptor_address: u64,
    entry_offset: usize,
    entry_address: u64,
    first_program_header: usize,
    second_program_header: usize,
    third_program_header: usize,
    rodata_header: usize,
    text_header: usize,
    section_header_zero: usize,
    strtab_header: usize,
    symtab_header: usize,
    symtab_offset: usize,
    entry_symbol: usize,
    descriptor_symbol: usize,
    spare_symbol: usize,
    other_name: usize,
    other_name_index: u32,
    strtab_end: usize,
}

fn binding_fixture(kernel: Value) -> BindingFixture {
    binding_fixture_for_document(metadata((1, 2), vec![kernel]))
}

fn binding_fixture_for_target(processor: &str, sgpr_count: u32) -> BindingFixture {
    let mut kernel = valid_kernel("vecadd", "vecadd.kd");
    set_field(&mut kernel, ".sgpr_count", Value::from(sgpr_count));
    if matches!(
        processor,
        "gfx600" | "gfx700" | "gfx803" | "gfx900" | "gfx942" | "gfx950"
    ) {
        set_field(&mut kernel, ".wavefront_size", Value::from(64));
    }
    if matches!(
        processor,
        "gfx600" | "gfx700" | "gfx803" | "gfx900" | "gfx942" | "gfx950"
    ) {
        set_field(&mut kernel, ".vgpr_count", Value::from(11));
        remove_field(&mut kernel, ".workgroup_processor_mode");
    }
    let mut document = metadata((1, 2), vec![kernel]);
    set_field(
        &mut document,
        "amdhsa.target",
        Value::from(format!("amdgcn-amd-amdhsa--{processor}")),
    );
    let mut fixture = binding_fixture_for_document(document);
    let elf_flags = match processor {
        "gfx600" => 0x20,
        "gfx700" => 0x22,
        "gfx803" => 0x2a,
        "gfx900" => 0x12c,
        "gfx942" => 0x54c,
        "gfx950" => 0x54f,
        "gfx1151" => 0x4a,
        "gfx1250" => 0x449,
        "gfx1310" => 0x50,
        _ => panic!("unsupported test processor {processor}"),
    };
    write_u32(&mut fixture.bytes, 48, elf_flags);
    if matches!(processor, "gfx942" | "gfx950") {
        write_u32(&mut fixture.bytes, fixture.descriptor_offset + 44, 1);
        write_u32(
            &mut fixture.bytes,
            fixture.descriptor_offset + 48,
            0x00af_0081,
        );
        write_u16(&mut fixture.bytes, fixture.descriptor_offset + 56, 0x001e);
    } else if matches!(processor, "gfx600" | "gfx700" | "gfx803" | "gfx900") {
        write_u32(&mut fixture.bytes, fixture.descriptor_offset + 44, 0);
        write_u32(
            &mut fixture.bytes,
            fixture.descriptor_offset + 48,
            0x0000_0042,
        );
        write_u16(&mut fixture.bytes, fixture.descriptor_offset + 56, 0x001e);
    }
    fixture
}

fn set_sgpr_block_field(fixture: &mut BindingFixture, encoded_blocks: u32) {
    let offset = fixture.descriptor_offset + 48;
    let rsrc1 = (read_u32(&fixture.bytes, offset) & !(0xf << 6)) | (encoded_blocks << 6);
    write_u32(&mut fixture.bytes, offset, rsrc1);
}

fn binding_fixture_for_document(document: Value) -> BindingFixture {
    binding_fixture_for_document_at(document, 0x9c0)
}

fn binding_fixture_for_document_at(document: Value, descriptor_offset: usize) -> BindingFixture {
    const PROGRAM_HEADER_BYTES: usize = 56;
    const PROGRAM_COUNT: usize = 3;
    const SECTION_COUNT: usize = 7;

    let note = metadata_note(&encode(&document));
    let first_program_header = ELF_HEADER_BYTES;
    let second_program_header = first_program_header + PROGRAM_HEADER_BYTES;
    let third_program_header = second_program_header + PROGRAM_HEADER_BYTES;
    let mut bytes = vec![0; ELF_HEADER_BYTES + PROGRAM_COUNT * PROGRAM_HEADER_BYTES];
    align(&mut bytes, 64);
    let note_offset = bytes.len();
    bytes.extend_from_slice(&note);
    align(&mut bytes, 64);
    assert!(bytes.len() <= descriptor_offset);
    bytes.resize(descriptor_offset, 0);
    bytes.resize(descriptor_offset + 64, 0);
    align(&mut bytes, 256);
    let entry_offset = bytes.len();
    bytes.resize(entry_offset + 64, 0xbf);
    let entry_address = entry_offset as u64 + 0x1000;
    let descriptor_address = descriptor_offset as u64;

    let strtab = b"\0vecadd\0vecadd.kd\0other\0";
    let entry_name_index = 1u32;
    let descriptor_name_index = 8u32;
    let other_name_index = 18u32;
    let strtab_offset = bytes.len();
    bytes.extend_from_slice(strtab);
    let strtab_end = bytes.len();
    let other_name = strtab_offset + other_name_index as usize;
    align(&mut bytes, 8);

    let symtab_offset = bytes.len();
    bytes.resize(symtab_offset + 4 * 24, 0);
    let entry_symbol = symtab_offset + 24;
    write_u32(&mut bytes, entry_symbol, entry_name_index);
    bytes[entry_symbol + 4] = 0x12;
    bytes[entry_symbol + 5] = 3;
    write_u16(&mut bytes, entry_symbol + 6, 3);
    write_u64(&mut bytes, entry_symbol + 8, entry_address);
    write_u64(&mut bytes, entry_symbol + 16, 64);

    let descriptor_symbol = symtab_offset + 48;
    write_u32(&mut bytes, descriptor_symbol, descriptor_name_index);
    bytes[descriptor_symbol + 4] = 0x11;
    write_u16(&mut bytes, descriptor_symbol + 6, 2);
    write_u64(&mut bytes, descriptor_symbol + 8, descriptor_address);
    write_u64(&mut bytes, descriptor_symbol + 16, 64);

    let spare_symbol = symtab_offset + 72;
    write_u32(&mut bytes, spare_symbol, other_name_index);
    bytes[spare_symbol + 4] = 0x10;
    write_u16(&mut bytes, spare_symbol + 6, 0xfff1);

    let shstrtab = b"\0.note\0.rodata\0.text\0.strtab\0.symtab\0.shstrtab\0";
    let shstrtab_offset = bytes.len();
    bytes.extend_from_slice(shstrtab);
    align(&mut bytes, 8);
    let section_offset = bytes.len();
    bytes.resize(section_offset + SECTION_COUNT * SECTION_HEADER_BYTES, 0);

    bytes[..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    bytes[7] = 64;
    bytes[8] = 4;
    write_u16(&mut bytes, 16, 3);
    write_u16(&mut bytes, 18, 224);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 32, first_program_header as u64);
    write_u64(&mut bytes, 40, section_offset as u64);
    write_u32(&mut bytes, 48, 0x4a);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, PROGRAM_HEADER_BYTES as u16);
    write_u16(&mut bytes, 56, PROGRAM_COUNT as u16);
    write_u16(&mut bytes, 58, SECTION_HEADER_BYTES as u16);
    write_u16(&mut bytes, 60, SECTION_COUNT as u16);
    write_u16(&mut bytes, 62, 6);

    write_u32(&mut bytes, first_program_header, 1);
    write_u32(&mut bytes, first_program_header + 4, 4);
    write_u64(&mut bytes, first_program_header + 8, 0);
    write_u64(&mut bytes, first_program_header + 16, 0);
    write_u64(
        &mut bytes,
        first_program_header + 32,
        (descriptor_offset + 64) as u64,
    );
    write_u64(
        &mut bytes,
        first_program_header + 40,
        (descriptor_offset + 64) as u64,
    );
    write_u64(&mut bytes, first_program_header + 48, 0x1000);

    write_u32(&mut bytes, second_program_header, 1);
    write_u32(&mut bytes, second_program_header + 4, 5);
    write_u64(&mut bytes, second_program_header + 8, entry_offset as u64);
    write_u64(&mut bytes, second_program_header + 16, entry_address);
    write_u64(&mut bytes, second_program_header + 32, 64);
    write_u64(&mut bytes, second_program_header + 40, 64);
    write_u64(&mut bytes, second_program_header + 48, 0x1000);

    let note_header = section_offset + SECTION_HEADER_BYTES;
    write_u32(&mut bytes, note_header, 1);
    write_u32(&mut bytes, note_header + 4, 7);
    write_u64(&mut bytes, note_header + 8, 2);
    write_u64(&mut bytes, note_header + 16, note_offset as u64);
    write_u64(&mut bytes, note_header + 24, note_offset as u64);
    write_u64(&mut bytes, note_header + 32, note.len() as u64);
    write_u64(&mut bytes, note_header + 48, 4);

    let rodata_header = section_offset + 2 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, rodata_header, 7);
    write_u32(&mut bytes, rodata_header + 4, 1);
    write_u64(&mut bytes, rodata_header + 8, 2);
    write_u64(&mut bytes, rodata_header + 16, descriptor_address);
    write_u64(&mut bytes, rodata_header + 24, descriptor_offset as u64);
    write_u64(&mut bytes, rodata_header + 32, 64);
    write_u64(&mut bytes, rodata_header + 48, 64);

    let text_header = section_offset + 3 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, text_header, 15);
    write_u32(&mut bytes, text_header + 4, 1);
    write_u64(&mut bytes, text_header + 8, 6);
    write_u64(&mut bytes, text_header + 16, entry_address);
    write_u64(&mut bytes, text_header + 24, entry_offset as u64);
    write_u64(&mut bytes, text_header + 32, 64);
    write_u64(&mut bytes, text_header + 48, 256);

    let strtab_header = section_offset + 4 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, strtab_header, 21);
    write_u32(&mut bytes, strtab_header + 4, 3);
    write_u64(&mut bytes, strtab_header + 24, strtab_offset as u64);
    write_u64(&mut bytes, strtab_header + 32, strtab.len() as u64);
    write_u64(&mut bytes, strtab_header + 48, 1);

    let symtab_header = section_offset + 5 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, symtab_header, 29);
    write_u32(&mut bytes, symtab_header + 4, 2);
    write_u64(&mut bytes, symtab_header + 24, symtab_offset as u64);
    write_u64(&mut bytes, symtab_header + 32, 4 * 24);
    write_u32(&mut bytes, symtab_header + 40, 4);
    write_u32(&mut bytes, symtab_header + 44, 1);
    write_u64(&mut bytes, symtab_header + 48, 8);
    write_u64(&mut bytes, symtab_header + 56, 24);

    let shstrtab_header = section_offset + 6 * SECTION_HEADER_BYTES;
    write_u32(&mut bytes, shstrtab_header, 37);
    write_u32(&mut bytes, shstrtab_header + 4, 3);
    write_u64(&mut bytes, shstrtab_header + 24, shstrtab_offset as u64);
    write_u64(&mut bytes, shstrtab_header + 32, shstrtab.len() as u64);
    write_u64(&mut bytes, shstrtab_header + 48, 1);

    write_u32(&mut bytes, descriptor_offset, 0);
    write_u32(&mut bytes, descriptor_offset + 4, 16);
    write_u32(&mut bytes, descriptor_offset + 8, 272);
    write_i64(
        &mut bytes,
        descriptor_offset + 16,
        i64::try_from(entry_address - descriptor_address).unwrap(),
    );
    write_u32(&mut bytes, descriptor_offset + 44, 0x40);
    write_u32(&mut bytes, descriptor_offset + 48, 0xe0af_0000);
    write_u32(&mut bytes, descriptor_offset + 52, 0x1391);
    write_u16(&mut bytes, descriptor_offset + 56, 0x041e);

    BindingFixture {
        bytes,
        descriptor_offset,
        descriptor_address,
        entry_offset,
        entry_address,
        first_program_header,
        second_program_header,
        third_program_header,
        rodata_header,
        text_header,
        section_header_zero: section_offset,
        strtab_header,
        symtab_header,
        symtab_offset,
        entry_symbol,
        descriptor_symbol,
        spare_symbol,
        other_name,
        other_name_index,
        strtab_end,
    }
}

fn assert_binding_error(bytes: Vec<u8>, expected: KernelBindingError) {
    assert_eq!(inspect_and_bind_kernel_descriptors(&bytes), Err(expected));
}

fn write_zero_fill_load(
    bytes: &mut [u8],
    program_header: usize,
    flags: u32,
    address: u64,
    memory_size: u64,
) {
    write_u32(bytes, program_header, 1);
    write_u32(bytes, program_header + 4, flags);
    write_u64(bytes, program_header + 8, 0);
    write_u64(bytes, program_header + 16, address);
    write_u64(bytes, program_header + 32, 0);
    write_u64(bytes, program_header + 40, memory_size);
    write_u64(bytes, program_header + 48, 1);
}

fn align(bytes: &mut Vec<u8>, alignment: usize) {
    while !bytes.len().is_multiple_of(alignment) {
        bytes.push(0);
    }
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

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

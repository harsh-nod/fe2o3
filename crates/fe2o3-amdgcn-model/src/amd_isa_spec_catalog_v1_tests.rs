use super::*;
use sha2::{Digest, Sha256};

fn digest_hex(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    Sha256::digest(bytes)
        .iter()
        .flat_map(|byte| {
            [
                HEX[(byte >> 4) as usize] as char,
                HEX[(byte & 15) as usize] as char,
            ]
        })
        .collect()
}

#[test]
fn metadata_and_separate_overlay_changes_cannot_reuse_catalog_identity() {
    let a = amd_isa_catalog_for_target_v1("gfx942").unwrap();
    let b = amd_isa_catalog_for_target_v1("gfx950").unwrap();
    assert_ne!(a.identity().metadata_sha256, b.identity().metadata_sha256);
    assert_eq!(a.identity().metadata_sha256, a.metadata_sha256());
    let overlay = digest_hex(AMD_ISA_SPEC_REVIEWED_COVERAGE_JSON_V1);
    assert_eq!(a.identity().reviewed_coverage_sha256, overlay);
    assert_eq!(b.identity().reviewed_coverage_sha256, overlay);
    let mut changed = a.identity();
    changed.reviewed_coverage_sha256 = "changed-overlay-is-not-the-same-identity";
    assert_ne!(changed, a.identity());
    changed = a.identity();
    changed.metadata_sha256 = b.identity().metadata_sha256;
    assert_ne!(changed, a.identity());
    assert_eq!(changed.member_sha256, a.identity().member_sha256);
}

#[test]
fn exact_profiles_reject_features_and_unknown_targets_without_authority() {
    for (name, architecture) in [
        ("gfx942", AmdIsaArchitectureV1::Cdna3),
        ("gfx950", AmdIsaArchitectureV1::Cdna4),
    ] {
        let catalog = amd_isa_catalog_for_target_v1(name).unwrap();
        assert_eq!(catalog.identity().architecture, architecture);
        assert_eq!(catalog.identity().target_profile.unwrap().name(), name);
        assert!(!catalog.grants_authority());
    }
    for name in [
        "",
        "GFX942",
        " gfx942",
        "gfx942\n",
        "gfx942:xnack-",
        "gfx950+sramecc",
        "gfx90a",
        "gfx1100",
        "cdna3",
    ] {
        assert!(amd_isa_catalog_for_target_v1(name).is_err(), "{name}");
    }
}

#[test]
fn full_inventory_references_are_target_scoped_and_exactly_sorted() {
    for (architecture, instruction_count, encoding_count, format_count) in [
        (AmdIsaArchitectureV1::Cdna3, 1150, 32, 83),
        (AmdIsaArchitectureV1::Cdna4, 1240, 33, 98),
    ] {
        let catalog = amd_isa_spec_catalog_v1(architecture);
        assert_eq!(catalog.instruction_count(), instruction_count);
        assert_eq!(catalog.instructions().count(), instruction_count);
        assert_eq!(
            catalog.entries(AmdIsaMetadataKindV1::Encoding).len(),
            encoding_count
        );
        assert_eq!(catalog.entries(AmdIsaMetadataKindV1::OperandType).len(), 35);
        assert_eq!(
            catalog.entries(AmdIsaMetadataKindV1::DataFormat).len(),
            format_count
        );
        let mut previous = "";
        for instruction in catalog.instructions() {
            assert!(instruction.name() > previous);
            previous = instruction.name();
            assert_eq!(
                catalog.instruction(instruction.name()).unwrap().name(),
                instruction.name()
            );
            assert_eq!(instruction.catalog_identity(), catalog.identity());
            assert!(!instruction.reviewed_source_marker_family());
            for alias in instruction.aliases() {
                let matched = catalog.lookup(alias).unwrap();
                assert_eq!(matched.name(), instruction.name());
                assert_eq!(matched.queried_name(), *alias);
                assert!(catalog.instruction(alias).is_none());
                assert_eq!(matched.catalog_identity(), catalog.identity());
            }
            for encoding in instruction.encodings() {
                assert_eq!(encoding.instruction().name(), instruction.name());
                assert_eq!(encoding.definition().unwrap().name(), encoding.name());
                assert_eq!(
                    encoding.definition().unwrap().catalog_identity(),
                    catalog.identity()
                );
                for operand in encoding.operands() {
                    assert_eq!(operand.encoding().name(), encoding.name());
                    assert_eq!(
                        operand.operand_type().unwrap().name(),
                        operand.operand_type_name()
                    );
                    assert_eq!(
                        operand.data_format().unwrap().name(),
                        operand.data_format_name()
                    );
                    assert_eq!(
                        operand.operand_type().unwrap().catalog_identity(),
                        catalog.identity()
                    );
                }
            }
        }
        assert!(catalog.lookup("v_xor_b32").is_none());
        assert!(catalog.lookup(&"X".repeat(129)).is_none());
        assert!(
            catalog
                .entry(AmdIsaMetadataKindV1::Encoding, "ENC_UNKNOWN")
                .is_none()
        );
    }
}

#[test]
fn actual_metadata_hashes_and_all_tree_ranges_retain_pinned_member_identity() {
    for architecture in [AmdIsaArchitectureV1::Cdna3, AmdIsaArchitectureV1::Cdna4] {
        let catalog = amd_isa_spec_catalog_v1(architecture);
        assert_eq!(
            digest_hex(catalog.metadata_json()),
            catalog.metadata_sha256()
        );
        let expected = match architecture {
            AmdIsaArchitectureV1::Cdna3 => {
                "af24feb95c9230a87f694c814b0035e7448a57f8503b059f0282dba1e5f20d9c"
            }
            AmdIsaArchitectureV1::Cdna4 => {
                "bb2a96f3db5c67d7c9b90e34559045c272f3536acdb03d16bc455f2f5b0a1597"
            }
        };
        assert_eq!(catalog.identity().member_sha256, expected);
        for tree in catalog
            .instructions()
            .map(AmdIsaInstructionV1::metadata)
            .chain(
                [
                    AmdIsaMetadataKindV1::Encoding,
                    AmdIsaMetadataKindV1::OperandType,
                    AmdIsaMetadataKindV1::DataFormat,
                ]
                .into_iter()
                .flat_map(|kind| catalog.entries(kind).map(AmdIsaNamedMetadataV1::metadata)),
            )
        {
            assert_eq!(tree.identity(), catalog.identity());
            assert!(tree.tree_json().starts_with('[') && tree.tree_json().ends_with(']'));
            assert!(!tree.dictionary().is_empty());
            assert!(
                tree.dictionary()
                    .windows(2)
                    .all(|values| values[0] < values[1])
            );
            assert!(tree.dictionary().contains(&"CondtionExpression"));
        }
    }
}

#[test]
fn independent_gfx942_vop2_opcodes_match_two_numeric_words_without_authorizing_them() {
    let catalog = amd_isa_catalog_for_target_v1("gfx942").unwrap();
    for (name, opcode, dst, src0, src1, word) in [
        ("V_XOR_B32", 21_u32, 32, 34, 35, 0x2a40_4722),
        ("V_ADD_U32", 52_u32, 33, 32, 36, 0x6842_4920),
    ] {
        let instruction = catalog.instruction(name).unwrap();
        let encoding = instruction
            .encodings()
            .find(|row| row.name() == "ENC_VOP2")
            .unwrap();
        assert_eq!(encoding.opcode(), opcode);
        assert_eq!(encoding.condition(), "default");
        assert_eq!(encoding.condition_declaration_count(), 1);
        assert_eq!(encoding.definition().unwrap().bit_count(), Some(32));
        assert_eq!(
            (opcode << 25) | (dst << 17) | (src1 << 9) | (256 + src0),
            word
        );
        assert_eq!((word >> 25) & 63, opcode);
        assert!(!instruction.flags().is_branch());
        assert!(!instruction.flags().is_program_terminator());
        let operands = encoding.operands().collect::<Vec<_>>();
        assert_eq!(operands.len(), 3);
        assert_eq!(operands[0].field_name(), Some("VDST"));
        assert!(operands[0].is_output() && !operands[0].is_input());
        assert_eq!(operands[1].operand_type_name(), "OPR_SRC");
        assert_eq!(operands[2].field_name(), Some("VSRC1"));
        assert!(operands.iter().all(|row| row.bit_count() == 32));
        // Absence of an explicit EXEC operand does not assert that EXEC is unused.
        assert!(operands.iter().all(|row| !row.is_implicit()));
        assert!(!catalog.grants_authority());
    }
}

#[test]
fn upstream_condition_defects_are_visible_not_synthesized_or_uniqued() {
    for (architecture, missing_count) in [
        (AmdIsaArchitectureV1::Cdna3, 86),
        (AmdIsaArchitectureV1::Cdna4, 88),
    ] {
        let catalog = amd_isa_spec_catalog_v1(architecture);
        let mut missing = 0;
        let mut duplicates = 0;
        for encoding in catalog
            .instructions()
            .flat_map(AmdIsaInstructionV1::encodings)
        {
            match encoding.condition_declaration_count() {
                0 => {
                    missing += 1;
                    assert!(["ENC_FLAT_GLBL", "ENC_FLAT_SCRATCH"].contains(&encoding.name()));
                    assert_eq!(encoding.condition(), "default");
                    assert!(!encoding.condition_definition_available());
                }
                1 => assert!(encoding.condition_definition_available()),
                3 => {
                    duplicates += 1;
                    assert_eq!(encoding.name(), "ENC_FLAT");
                    assert_eq!(encoding.condition(), "default");
                }
                other => panic!("unexpected declaration count {other}"),
            }
        }
        assert_eq!(missing, missing_count);
        assert!(duplicates > 0);
    }
}

#[test]
fn source_marker_overlay_does_not_choose_encoding_or_enable_other_targets() {
    let gfx942 = amd_isa_catalog_for_target_v1("gfx942").unwrap();
    let gfx950 = amd_isa_catalog_for_target_v1("gfx950").unwrap();
    let matched = gfx942
        .instructions()
        .filter(|row| row.reviewed_source_marker_family())
        .map(AmdIsaInstructionV1::name)
        .collect::<Vec<_>>();
    assert_eq!(
        matched,
        [
            "V_ADD_U32",
            "V_AND_B32",
            "V_MOV_B32",
            "V_OR_B32",
            "V_SUB_U32",
            "V_XOR_B32"
        ]
    );
    for name in matched {
        assert!(
            !gfx950
                .instruction(name)
                .unwrap()
                .reviewed_source_marker_family()
        );
        assert!(gfx942.instruction(name).unwrap().encodings().len() > 1);
    }
    assert!(
        !gfx942
            .instruction("S_MOV_B32")
            .unwrap()
            .reviewed_source_marker_family()
    );
    assert!(
        AMD_ISA_SPEC_REVIEWED_COVERAGE_JSON_V1.contains("\"encoding_selection\": \"unavailable\"")
    );
    assert!(
        AMD_ISA_SPEC_REVIEWED_COVERAGE_JSON_V1
            .contains("\"hardware_qualification\": \"unavailable\"")
    );
}

use super::*;

fn request(args: &[&str]) -> Request {
    arguments(args.iter().map(|arg| OsString::from(*arg))).unwrap()
}

fn inspect(target: &str, name: &str) -> String {
    render(&request(&[target, name]), MAX_OUTPUT_BYTES).unwrap()
}

#[test]
fn exact_targets_summary_list_and_help_have_distinct_arguments() {
    assert_eq!(request(&["--help"]), Request::Help);
    assert_eq!(
        request(&["gfx942"]),
        Request::Inspect {
            target: "gfx942".into(),
            mode: Mode::Summary,
        }
    );
    assert_eq!(
        request(&["gfx950", "--list"]),
        Request::Inspect {
            target: "gfx950".into(),
            mode: Mode::List,
        }
    );
    assert_eq!(
        request(&["gfx942", "V_XOR_B32"]),
        Request::Inspect {
            target: "gfx942".into(),
            mode: Mode::Instruction("V_XOR_B32".into()),
        }
    );
    assert_eq!(render(&Request::Help, MAX_OUTPUT_BYTES).unwrap(), USAGE);
}

#[test]
fn target_aliases_features_case_whitespace_and_unknown_targets_refuse() {
    for name in [
        "",
        "GFX942",
        " gfx942",
        "gfx942 ",
        "gfx942\n",
        "gfx942:xnack-",
        "gfx950+sramecc",
        "gfx90a",
        "gfx1100",
        "cdna3",
        "MI300X",
        "MI350",
    ] {
        assert!(arguments([OsString::from(name)].into_iter()).is_err());
    }
    assert!(arguments(Vec::<OsString>::new().into_iter()).is_err());
    for values in [
        vec!["--help", "gfx942"],
        vec!["gfx942", "--unknown"],
        vec!["gfx942", "--list", "V_XOR_B32"],
        vec!["gfx942", ""],
    ] {
        assert!(arguments(values.into_iter().map(OsString::from)).is_err());
    }
}

#[test]
fn arguments_are_bounded_without_collecting_an_unbounded_iterator() {
    let oversized = "X".repeat(MAX_ARGUMENT_BYTES + 1);
    assert!(arguments([OsString::from(oversized.clone())].into_iter()).is_err());
    assert!(arguments([OsString::from("gfx942"), OsString::from(oversized)].into_iter()).is_err());
    // UTF-8 bytes, not characters, define the bound.
    assert!(
        arguments([OsString::from("gfx942"), OsString::from("é".repeat(65))].into_iter()).is_err()
    );
    let mut visited = 0;
    let iterator = std::iter::from_fn(|| {
        visited += 1;
        assert!(
            visited <= 3,
            "argument parsing must not consume the remainder"
        );
        Some(OsString::from("gfx942"))
    });
    assert!(arguments(iterator).is_err());
    assert_eq!(visited, 3);
}

#[cfg(unix)]
#[test]
fn non_utf8_arguments_refuse_without_lossy_conversion() {
    use std::os::unix::ffi::OsStringExt;
    assert!(arguments([OsString::from_vec(vec![0xff])].into_iter()).is_err());
    assert!(
        arguments([OsString::from("gfx942"), OsString::from_vec(vec![0xff])].into_iter()).is_err()
    );
}

#[test]
fn summaries_carry_actual_separate_catalog_and_overlay_identities() {
    for (target, families, forms, formats, missing, covered) in [
        ("gfx942", 1150, 32, 83, 86, 6),
        ("gfx950", 1240, 33, 98, 88, 0),
    ] {
        let catalog = amd_isa_catalog_for_target_v1(target).unwrap();
        let identity = catalog.identity();
        let text = render(&request(&[target]), MAX_OUTPUT_BYTES).unwrap();
        for expected in [
            format!("target_profile: {target}\n"),
            format!("catalog_format: {}\n", identity.format),
            format!("archive_sha256: {}\n", identity.archive_sha256),
            format!("member: {}\n", identity.member),
            format!("member_sha256: {}\n", identity.member_sha256),
            format!("metadata_sha256: {}\n", identity.metadata_sha256),
            format!(
                "reviewed_coverage_sha256: {}\n",
                identity.reviewed_coverage_sha256
            ),
            format!("instruction_families: {families}\n"),
            format!("encoding_forms: {forms}\n"),
            "operand_types: 35\n".to_owned(),
            format!("data_formats: {formats}\n"),
            format!("alternatives_with_missing_condition_definition: {missing}\n"),
            format!("reviewed_gfx942_u32_source_marker_families: {covered}\n"),
        ] {
            assert!(text.contains(&expected), "missing {expected}");
        }
        assert!(!text.lines().any(|line| line.starts_with("instruction ")));
        assert!(text.len() < MAX_OUTPUT_BYTES);
    }
    let a = render(&request(&["gfx942"]), MAX_OUTPUT_BYTES).unwrap();
    let b = render(&request(&["gfx950"]), MAX_OUTPUT_BYTES).unwrap();
    assert_ne!(a, b);
}

#[test]
fn inventory_lists_every_canonical_name_once_in_deterministic_order() {
    for target in ["gfx942", "gfx950"] {
        let query = request(&[target, "--list"]);
        let first = render(&query, MAX_OUTPUT_BYTES).unwrap();
        assert_eq!(first, render(&query, MAX_OUTPUT_BYTES).unwrap());
        let lines = first
            .lines()
            .filter(|line| line.starts_with("instruction "))
            .collect::<Vec<_>>();
        let catalog = amd_isa_catalog_for_target_v1(target).unwrap();
        let names = inventory(catalog).unwrap();
        assert_eq!(lines.len(), catalog.instruction_count());
        assert_eq!(lines.len(), names.len());
        for (line, instruction) in lines.iter().zip(names) {
            assert_eq!(
                *line,
                format!(
                    "instruction {:?} aliases={} alternatives={} reviewed_source_marker_family={}",
                    instruction.name(),
                    instruction.aliases().len(),
                    instruction.encodings().len(),
                    instruction.reviewed_source_marker_family()
                )
            );
        }
        assert!(
            lines.windows(2).all(|pair| pair[0] < pair[1]),
            "canonical names, not unstable array indices"
        );
        assert!(!first.contains("instruction_id:"));
    }
}

#[test]
fn detail_keeps_every_alternative_and_operand_in_explicit_deterministic_order() {
    let catalog = amd_isa_catalog_for_target_v1("gfx942").unwrap();
    let instruction = catalog.lookup("V_XOR_B32").unwrap();
    let text = inspect("gfx942", "V_XOR_B32");
    assert_eq!(text, inspect("gfx942", "V_XOR_B32"));
    assert!(text.contains("queried_name: \"V_XOR_B32\"\n"));
    assert!(text.contains("canonical_instruction: \"V_XOR_B32\"\n"));
    assert!(text.contains("lookup_kind: canonical\n"));
    assert!(text.contains("reviewed_gfx942_u32_source_marker_family: available\n"));
    let encodings = alternatives(instruction).unwrap();
    let actual = text
        .lines()
        .filter(|line| line.starts_with("encoding "))
        .collect::<Vec<_>>();
    assert_eq!(actual.len(), encodings.len());
    assert!(actual.len() > 1);
    for (line, encoding) in actual.iter().zip(&encodings) {
        assert!(line.starts_with(&format!(
            "encoding {:?} condition={:?} opcode={} ",
            encoding.name(),
            encoding.condition(),
            encoding.opcode()
        )));
    }
    assert!(text.contains(
        "encoding \"ENC_VOP2\" condition=\"default\" opcode=21 condition_declarations=1 condition_definition=available encoding_definition=available encoding_bit_count=Some(32)"
    ));
    assert!(text.contains(
        "  operand order=1 input=false output=true implicit=false binary_microcode_required=true field=Some(\"VDST\") type=\"OPR_VGPR\""
    ));
    let expected_operands: usize = encodings.iter().map(|row| row.operands().len()).sum();
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("  operand "))
            .count(),
        expected_operands
    );
    let mut previous_order = 0;
    for line in text.lines() {
        if line.starts_with("encoding ") {
            previous_order = 0;
        } else if let Some(rest) = line.strip_prefix("  operand order=") {
            let order: u8 = rest.split(' ').next().unwrap().parse().unwrap();
            assert!(order >= previous_order);
            previous_order = order;
        }
    }
}

#[test]
fn explicit_upstream_alias_retains_the_queried_and_canonical_names() {
    let text = inspect("gfx942", "BUFFER_INVL2");
    assert!(text.contains("queried_name: \"BUFFER_INVL2\"\n"));
    assert!(text.contains("canonical_instruction: \"BUFFER_INV\"\n"));
    assert!(text.contains("lookup_kind: explicit_upstream_alias\n"));
    assert!(text.contains("aliases: [\"BUFFER_INVL2\"]\n"));
    assert!(text.contains("reviewed_gfx942_u32_source_marker_family: unavailable\n"));
    let canonical = inspect("gfx942", "BUFFER_INV");
    assert!(canonical.contains("lookup_kind: canonical\n"));
    assert_eq!(
        text.lines()
            .filter(|line| line.starts_with("encoding "))
            .collect::<Vec<_>>(),
        canonical
            .lines()
            .filter(|line| line.starts_with("encoding "))
            .collect::<Vec<_>>()
    );
}

#[test]
fn unknown_or_inferred_instruction_spellings_do_not_produce_a_report() {
    for name in [
        "v_xor_b32",
        " V_XOR_B32",
        "V_XOR_B32 ",
        "V_XOR_B32_e32",
        "V_XOR_B32\n",
        "NOT_AN_INSTRUCTION",
    ] {
        assert!(render(&request(&["gfx942", name]), MAX_OUTPUT_BYTES).is_err());
    }
}

#[test]
fn missing_and_triplicate_upstream_conditions_are_neither_synthesized_nor_selected() {
    for target in ["gfx942", "gfx950"] {
        let missing = inspect(target, "GLOBAL_LOAD_DWORD");
        assert!(missing.contains(
            "encoding \"ENC_FLAT_GLBL\" condition=\"default\" opcode=20 condition_declarations=0 condition_definition=unavailable encoding_definition=available"
        ));
        let repeated = inspect(target, "FLAT_LOAD_DWORD");
        assert!(repeated.contains(
            "encoding \"ENC_FLAT\" condition=\"default\" opcode=20 condition_declarations=3 condition_definition=available encoding_definition=available"
        ));
        for text in [&missing, &repeated] {
            assert!(
                text.contains("conditions: declarations only; expressions are not evaluated\n")
            );
            assert!(text.contains("encodable: not established by this catalog;"));
            assert!(!text.contains("selected_encoding:"));
        }
    }
}

#[test]
fn six_source_marker_families_do_not_extend_to_scalar_moves_or_other_targets() {
    let gfx942 = amd_isa_catalog_for_target_v1("gfx942").unwrap();
    let covered = inventory(gfx942)
        .unwrap()
        .into_iter()
        .filter(|row| row.reviewed_source_marker_family())
        .map(|row| row.name())
        .collect::<Vec<_>>();
    assert_eq!(
        covered,
        [
            "V_ADD_U32",
            "V_AND_B32",
            "V_MOV_B32",
            "V_OR_B32",
            "V_SUB_U32",
            "V_XOR_B32"
        ]
    );
    for name in &covered {
        assert!(
            inspect("gfx942", name)
                .contains("reviewed_gfx942_u32_source_marker_family: available\n")
        );
        assert!(
            inspect("gfx950", name)
                .contains("reviewed_gfx942_u32_source_marker_family: unavailable\n")
        );
    }
    // The existing KIR validator has S_MOV_B32; that is not source-macro coverage.
    assert!(
        inspect("gfx942", "S_MOV_B32")
            .contains("reviewed_gfx942_u32_source_marker_family: unavailable\n")
    );
}

#[test]
fn all_catalogued_details_remain_bounded_and_never_become_capability_grants() {
    for target in ["gfx942", "gfx950"] {
        let catalog = amd_isa_catalog_for_target_v1(target).unwrap();
        for instruction in inventory(catalog).unwrap() {
            // Render every detail without rebuilding the complete target summary
            // thousands of times; individual command paths are exercised above.
            let mut report = BoundedText::new(MAX_OUTPUT_BYTES).unwrap();
            report.write_str(BOUNDARY).unwrap();
            report
                .line(format_args!(
                    "grants_authority: {}",
                    catalog.grants_authority()
                ))
                .unwrap();
            describe(&mut report, instruction).unwrap();
            let text = report.text;
            assert!(text.len() <= MAX_OUTPUT_BYTES);
            assert!(text.contains("grants_authority: false\n"));
            assert!(text.contains("authorable: encoding-level authoring unavailable"));
            assert!(text.contains("simulated: encoding-level simulation unavailable"));
            assert!(text.contains("qualified: proof and hardware qualification unavailable"));
            assert!(text.contains("missing explicit EXEC is not an EXEC guarantee"));
            assert!(text.contains("operand widths are not legal allocation"));
            assert!(!text.contains("grants_authority: true"));
            assert!(!text.contains("source_authenticated: true"));
        }
    }
}

#[test]
fn catalog_counters_preserve_actual_unavailable_and_multiple_condition_references() {
    for (target, missing, source_markers) in [("gfx942", 86, 6), ("gfx950", 88, 0)] {
        let catalog = amd_isa_catalog_for_target_v1(target).unwrap();
        let rows = inventory(catalog).unwrap();
        let actual = counts(&rows).unwrap();
        assert_eq!(actual.missing_conditions, missing);
        assert_eq!(actual.source_marker_families, source_markers);
        assert!(actual.repeated_conditions > 0);
        assert_eq!(
            actual.alternatives,
            rows.iter().map(|row| row.encodings().len()).sum::<usize>()
        );
    }
}

#[test]
fn output_limits_refuse_complete_reports_instead_of_truncating_them() {
    for args in [
        vec!["--help"],
        vec!["gfx942"],
        vec!["gfx950", "--list"],
        vec!["gfx942", "V_XOR_B32"],
    ] {
        let query = request(&args);
        let text = render(&query, MAX_OUTPUT_BYTES).unwrap();
        assert_eq!(render(&query, text.len()).unwrap(), text);
        assert_eq!(render(&query, text.len() - 1).unwrap_err(), OUTPUT_LIMIT);
        assert!(render(&query, 0).is_err());
        assert!(render(&query, MAX_OUTPUT_BYTES + 1).is_err());
    }
}

#[test]
fn a_rejected_append_keeps_the_existing_bounded_buffer_unchanged() {
    let mut text = BoundedText::new(4).unwrap();
    text.write_str("abcd").unwrap();
    assert!(text.write_str("e").is_err());
    assert_eq!(text.text, "abcd");
    let mut utf8 = BoundedText::new(4).unwrap();
    utf8.write_str("éé").unwrap();
    assert!(utf8.write_str("é").is_err());
    assert_eq!(utf8.text.len(), 4);
}

#[test]
fn human_text_contains_no_executable_or_portable_capture_contract() {
    let text = inspect("gfx942", "V_XOR_B32");
    for forbidden in [
        "schema:",
        "asm!",
        ".amdgcn_target",
        ".amdhsa_kernel",
        "hsaco",
        "source_sha256:",
    ] {
        assert!(!text.contains(forbidden), "{forbidden}");
    }
    assert!(USAGE.contains("not a stable wire format or an encoder"));
}

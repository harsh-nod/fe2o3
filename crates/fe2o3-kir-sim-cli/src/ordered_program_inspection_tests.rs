use super::*;
use crate::load_debug_simulation_input_bytes_v17;
use fe2o3_kernel_ir::{Gfx942OrderedProgramRegistersV1, ValueId};

#[test]
fn complete_declared_program_and_typed_steps_are_borrowed_without_microstep_claims() {
    for case in fixture::cases() {
        let module = fixture::module_with_program(true, fixture::program(&case.descriptors));
        let input = admitted(&module);
        let view = inspect(&input).unwrap();
        let mut output = FixedOutput::new();
        serde_json::to_writer(&mut output, &report(&input, &view)).unwrap();
        let json: serde_json::Value = serde_json::from_slice(output.as_bytes()).unwrap();
        assert_eq!(
            json["kind"],
            "diagnostic_ordered_program_inspection_example"
        );
        assert_eq!(json["authority"], "observation_only");
        assert_eq!(json["declared_program"]["count"], case.descriptors.len());
        let mut expected = [0_u16; 16];
        expected[..case.descriptors.len()].copy_from_slice(&case.descriptors);
        assert_eq!(
            json["declared_program"]["descriptors"],
            serde_json::json!(expected)
        );
        let steps = json["declared_instruction_steps"].as_array().unwrap();
        assert_eq!(steps.len(), case.descriptors.len());
        // Independent interpretation only for comparing declared metadata.
        // This never treats the declaration as a physical-state observation.
        let physical = [34, 35, 36, 32, 33];
        let mnemonics = [
            "v_mov_b32_e32",
            "v_add_u32_e32",
            "v_sub_u32_e32",
            "v_and_b32_e32",
            "v_or_b32_e32",
            "v_xor_b32_e32",
        ];
        for (step, &word) in steps.iter().zip(&case.descriptors) {
            let opcode = usize::from(word % 8);
            let destination = 3 + usize::from((word / 8) % 2);
            let left = usize::from((word / 16) % 8);
            let right = usize::from((word / 128) % 8);
            assert_eq!(step["instruction"], mnemonics[opcode]);
            assert_eq!(step["output"], physical[destination]);
            let inputs = if opcode == 0 {
                vec![physical[left]]
            } else {
                vec![physical[left], physical[right]]
            };
            assert_eq!(step["inputs"], serde_json::json!(inputs));
        }
        assert_eq!(json["instruction_microsteps_available"], false);
        assert_eq!(json["physical_register_values_available"], false);
        assert_eq!(
            json["logical_observation_granularity"],
            "whole_program_before_after"
        );
        assert!(output.as_bytes().len() < OUTPUT_BYTES);
        let OperationKind::Gfx942OrderedProgram(actual) = &input.module.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks[0]
            .operations[0]
            .kind
        else {
            panic!()
        };
        assert!(std::ptr::eq(view.program, actual));
        assert_eq!(
            view.program.program().active_descriptors(),
            case.descriptors
        );
    }
}

#[test]
fn canonical_declaration_edits_change_report_identity_not_authority() {
    let mut identities = std::collections::BTreeSet::new();
    for descriptors in [vec![8], vec![0, 8], vec![8, 8], vec![8, 72], vec![8; 16]] {
        let input = admitted(&fixture::module_with_program(
            true,
            fixture::program(&descriptors),
        ));
        let view = inspect(&input).unwrap();
        assert!(identities.insert(*input.module.identity().digest()));
        let report = report(&input, &view);
        assert_eq!(
            usize::from(report.declared_program.count),
            descriptors.len()
        );
        assert!(!report.source_authentication);
        assert!(!report.artifact_authority);
        assert!(!report.production_resume_authority);
        assert!(!report.hardware_execution);
    }
    assert_eq!(identities.len(), 5);
}
fn admitted(module: &Module) -> AdmittedSimulationInputV1 {
    let owner = fixture::owner(module);
    load_debug_simulation_input_bytes_v17(owner.canonical_bytes(), &fixture::request([19, 23, 42]))
        .unwrap()
}

#[test]
fn synthetic_owner_coordinates_bindings_and_truthful_output_are_observed() {
    for used in [true, false] {
        let mut module = fixture::module(used);
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        operations.swap(0, 1); // A real nonzero operation ordinal; raw block remains 7.
        if !used {
            let OperationKind::Gfx942OrderedProgram(program) = &mut operations[1].kind else {
                panic!()
            };
            *program = Gfx942OrderedProgramV1::new(
                program.source(),
                Gfx942OrderedProgramRegistersV1::new(40, 41, [42, 43, 44]).unwrap(),
                *program.inputs(),
                *program.program(),
            )
            .unwrap();
        }
        let input = admitted(&module);
        let view = inspect(&input).unwrap();
        assert_eq!(
            (
                view.block_ordinal,
                view.operation_ordinal,
                view.raw_block_id
            ),
            (0, 1, 7)
        );
        assert_eq!(view.program.inputs(), &[ValueId(0), ValueId(1), ValueId(2)]);
        assert_eq!(view.result_value_id, 4);
        let mut output = FixedOutput::new();
        serde_json::to_writer(&mut output, &report(&input, &view)).unwrap();
        let json: serde_json::Value = serde_json::from_slice(output.as_bytes()).unwrap();
        assert_eq!(json["canonical"]["wire_version"], 17);
        assert_eq!(
            json["canonical"]["bytes"],
            input.module.identity().canonical_length()
        );
        assert_eq!(json["register_plan"]["scratch"], if used { 32 } else { 40 });
        assert_eq!(json["memory_effect"], "NoMemory");
        assert_eq!(json["ordered_region_effect"], true);
        for flag in [
            "source_authentication",
            "source_map_available",
            "physical_register_values_available",
            "instruction_microsteps_available",
            "register_lifetime_or_final_allocation_proof",
            "proof_authority",
            "artifact_authority",
            "production_resume_authority",
            "hardware_execution",
            "pure_or_movable",
        ] {
            assert_eq!(json[flag], false);
        }
    }
}

#[test]
fn synthetic_structural_precheck_rejects_missing_multiple_and_over_budget_shapes() {
    let mut none = fixture::module(true);
    none.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .remove(0);
    assert!(find_program(&none).is_err());
    let mut multiple = fixture::module(true);
    let operations = &mut multiple.functions[0].body.as_mut().unwrap().blocks[0].operations;
    let mut second = operations[0].clone();
    second.results[0].id = ValueId(8);
    operations.insert(1, second);
    assert!(find_program(&multiple).is_err());
    let mut too_many = fixture::module(true);
    let operations = &mut too_many.functions[0].body.as_mut().unwrap().blocks[0].operations;
    operations.resize(MAX_OPERATIONS + 1, operations[0].clone());
    assert_eq!(
        find_program(&too_many).err(),
        Some("inspection structural limit exceeded")
    );
    let mut two_roots = fixture::module(true);
    two_roots.kernels.push(two_roots.kernels[0].clone());
    assert!(find_program(&two_roots).is_err());
    let mut counts = StructuralCounts::default();
    assert!(name(&mut counts, &"x".repeat(MAX_ID_BYTES + 1)).is_err());
    let mut total = usize::MAX;
    assert!(charge(&mut total, 1, usize::MAX).is_err());
    assert_eq!(total, usize::MAX);
}

#[test]
fn synthetic_request_mismatch_and_fixed_output_bound_fail_closed() {
    let mut input = admitted(&fixture::module(true));
    input.request.workgroup.0 = [32, 1, 1];
    assert!(inspect(&input).is_err());
    input.request.workgroup.0 = [64, 1, 1];
    input.request.grid.0 = [128, 1, 1];
    assert!(inspect(&input).is_err());
    let mut output = FixedOutput::new();
    assert!(output.write_all(&[0; OUTPUT_BYTES + 1]).is_err());
    assert!(output.as_bytes().is_empty());
    output.write_all(&[1; OUTPUT_BYTES]).unwrap();
    assert!(output.write_all(&[2]).is_err());
    assert_eq!(output.as_bytes(), &[1; OUTPUT_BYTES]);
}

#[test]
fn optional_text_argv_preserves_two_path_json_and_exact_path_bounds() {
    let parse =
        |args: &[&str]| parse_arguments(args.iter().map(|argument| OsString::from(*argument)));
    assert_eq!(parse(&["--help"]).unwrap(), InspectionCommand::Help);
    for first in ["program.kir", "--text", "--help"] {
        assert_eq!(
            parse(&[first, "request.json"]).unwrap(),
            InspectionCommand::Inspect {
                format: OutputFormat::Json,
                kir: first.into(),
                request: "request.json".into(),
            }
        );
    }
    assert_eq!(
        parse(&["--text", "program.kir", "request.json"]).unwrap(),
        InspectionCommand::Inspect {
            format: OutputFormat::Text,
            kir: "program.kir".into(),
            request: "request.json".into(),
        }
    );
    for text in [false, true] {
        for slot in 0..2 {
            for length in [4096, 4097] {
                let mut paths = ["program.kir".to_owned(), "request.json".to_owned()];
                paths[slot] = "x".repeat(length);
                let mut args = Vec::new();
                if text {
                    args.push(OsString::from("--text"));
                }
                args.extend(paths.map(OsString::from));
                assert_eq!(parse_arguments(args.into_iter()).is_ok(), length == 4096);
            }
        }
    }
    for args in [
        vec![],
        vec!["--text"],
        vec!["--json", "a", "b"],
        vec!["--text", "--text", "a", "b"],
        vec!["--text", "a", "b", "extra"],
    ] {
        assert!(parse(&args).is_err());
    }
    assert_eq!(
        parse(&["a", "b", "extra"]).err(),
        Some("exactly two paths of at most 4096 bytes are required")
    );
    use std::os::unix::ffi::OsStringExt;
    let non_utf8 = OsString::from_vec(vec![0xff]);
    for format in [OutputFormat::Json, OutputFormat::Text] {
        let mut args = Vec::new();
        if format == OutputFormat::Text {
            args.push(OsString::from("--text"));
        }
        args.extend([non_utf8.clone(), OsString::from("request.json")]);
        assert_eq!(
            parse_arguments(args.into_iter()).unwrap(),
            InspectionCommand::Inspect {
                format,
                kir: non_utf8.clone(),
                request: "request.json".into(),
            }
        );
    }
}

#[test]
fn default_json_rendering_keeps_the_original_report_bytes() {
    for case in fixture::cases() {
        let input = admitted(&fixture::module_with_program(
            true,
            fixture::program(&case.descriptors),
        ));
        let view = inspect(&input).unwrap();
        let observed = report(&input, &view);
        let mut original = FixedOutput::new();
        serde_json::to_writer(&mut original, &observed).unwrap();
        original.write_all(b"\n").unwrap();
        let current = render_output(&observed, OutputFormat::Json).unwrap();
        assert_eq!(current.as_bytes(), original.as_bytes());
    }
}

#[test]
fn text_listing_keeps_literal_one_three_sixteen_instruction_order() {
    let cases: &[(&[u16], &[&str])] = &[
        (&[8], &["  00: v_mov_b32_e32 v33, v34"]),
        (
            &[133, 307, 413],
            &[
                "  00: v_xor_b32_e32 v32, v34, v35",
                "  01: v_and_b32_e32 v32, v32, v36",
                "  02: v_xor_b32_e32 v33, v35, v32",
            ],
        ),
        (
            &[
                0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72,
            ],
            &[
                "  00: v_mov_b32_e32 v32, v34",
                "  01: v_xor_b32_e32 v33, v34, v35",
                "  02: v_and_b32_e32 v32, v33, v36",
                "  03: v_or_b32_e32 v33, v32, v35",
                "  04: v_add_u32_e32 v32, v33, v36",
                "  05: v_sub_u32_e32 v33, v32, v34",
                "  06: v_mov_b32_e32 v32, v33",
                "  07: v_xor_b32_e32 v32, v32, v35",
                "  08: v_or_b32_e32 v33, v32, v34",
                "  09: v_and_b32_e32 v33, v33, v36",
                "  10: v_add_u32_e32 v33, v33, v34",
                "  11: v_sub_u32_e32 v32, v33, v35",
                "  12: v_mov_b32_e32 v33, v32",
                "  13: v_xor_b32_e32 v33, v33, v36",
                "  14: v_mov_b32_e32 v32, v35",
                "  15: v_mov_b32_e32 v33, v33",
            ],
        ),
    ];
    for &(descriptors, expected) in cases {
        for used in [false, true] {
            let input = admitted(&fixture::module_with_program(
                used,
                fixture::program(descriptors),
            ));
            let view = inspect(&input).unwrap();
            let output = render_output(&report(&input, &view), OutputFormat::Text).unwrap();
            let text = std::str::from_utf8(output.as_bytes()).unwrap();
            assert_eq!(
                text.lines()
                    .filter(|line| line.starts_with("  "))
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(text.starts_with("Declared ordered-program syntax (not native disassembly)\n"));
            assert!(text.contains("CPU preflight only, no execution"));
            assert!(text.contains("NoMemory (not a whole-kernel effect)"));
            assert!(text.contains("Unavailable: physical values, instruction microsteps"));
        }
    }
}

#[test]
fn text_listing_uses_actual_coordinates_and_each_role_at_zero_and_sixty_three() {
    for roles in [
        [0, 1, 2, 3, 4],
        [63, 1, 2, 3, 4],
        [1, 0, 2, 3, 4],
        [0, 63, 2, 3, 4],
        [1, 2, 0, 3, 4],
        [0, 1, 63, 3, 4],
        [1, 2, 3, 0, 4],
        [0, 1, 2, 63, 4],
        [1, 2, 3, 4, 0],
        [0, 1, 2, 3, 63],
    ] {
        let mut module = fixture::module_with_program(true, fixture::program(&[8]));
        let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
        let OperationKind::Gfx942OrderedProgram(program) = &mut operations[0].kind else {
            panic!()
        };
        *program = Gfx942OrderedProgramV1::new(
            program.source(),
            Gfx942OrderedProgramRegistersV1::new(
                roles[0],
                roles[1],
                [roles[2], roles[3], roles[4]],
            )
            .unwrap(),
            *program.inputs(),
            *program.program(),
        )
        .unwrap();
        operations.swap(0, 1);
        let input = admitted(&module);
        let view = inspect(&input).unwrap();
        let output = render_output(&report(&input, &view), OutputFormat::Text).unwrap();
        let text = std::str::from_utf8(output.as_bytes()).unwrap();
        assert!(text.contains("coordinate: function=0 block=0 operation=1 raw_block_id=7"));
        assert!(text.contains("logical SSA: inputs=[0, 1, 2] result=4"));
        assert!(text.contains(&format!(
            "scratch=v{} out=v{} input0=v{} input1=v{} input2=v{}; high_water={}",
            roles[0],
            roles[1],
            roles[2],
            roles[3],
            roles[4],
            *roles.iter().max().unwrap() + 1
        )));
        assert!(text.contains(&format!("  00: v_mov_b32_e32 v{}, v{}", roles[1], roles[2])));
    }
}

#[test]
fn text_names_are_ascii_escaped_and_expansion_obeys_the_same_output_bound() {
    let mut output = FixedOutput::new();
    quoted_name(&mut output, "line\n\t\r\u{1b}\\\"é").unwrap();
    assert_eq!(
        std::str::from_utf8(output.as_bytes()).unwrap(),
        "\"line\\n\\t\\r\\u{1b}\\\\\\\"\\u{e9}\""
    );
    assert!(output.as_bytes().is_ascii());
    let mut exact = FixedOutput::new();
    quoted_name(&mut exact, &"a".repeat(OUTPUT_BYTES - 2)).unwrap();
    assert_eq!(exact.as_bytes().len(), OUTPUT_BYTES);
    assert!(exact.write_all(b"\n").is_err());
    let mut oversized = FixedOutput::new();
    assert!(quoted_name(&mut oversized, &"a".repeat(OUTPUT_BYTES - 1)).is_err());
    let input = admitted(&fixture::module(true));
    let view = inspect(&input).unwrap();
    let mut observed = report(&input, &view);
    // Renderer-only boundary: these borrowed names are not a new admitted owner.
    let expanded = "\u{1b}".repeat(MAX_ID_BYTES);
    observed.kernel = &expanded;
    observed.function = &expanded;
    assert_eq!(
        render_output(&observed, OutputFormat::Text).err(),
        Some("bounded inspection text serialization failed")
    );
}

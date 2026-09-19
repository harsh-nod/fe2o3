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

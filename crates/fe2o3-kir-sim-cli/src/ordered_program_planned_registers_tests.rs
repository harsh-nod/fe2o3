use super::super::{OUTPUT_BYTES, OutputFormat, fixture, inspect, render_output, report};
use super::*;
fn plan(words: &[u16]) -> Plan {
    let module = fixture::module_with_program(true, fixture::program(words));
    let fe2o3_kernel_ir::OperationKind::Gfx942OrderedProgram(program) =
        &module.functions[0].body.as_ref().unwrap().blocks[0].operations[0].kind
    else {
        panic!()
    };
    Plan::derive(program).unwrap()
}
fn values(p: &Plan) -> Vec<Value> {
    p.values.rows[..p.values.len]
        .iter()
        .map(|x| x.unwrap())
        .collect()
}
fn uses(p: &Plan) -> Vec<Use> {
    p.uses.rows[..p.uses.len]
        .iter()
        .map(|x| x.unwrap())
        .collect()
}
#[test]
fn exact_three_step_definition_and_use_versions() {
    let p = plan(&[133, 307, 413]);
    assert_eq!(p.result_boundary, 7);
    let v = values(&p);
    assert_eq!(
        v.iter()
            .map(|x| (x.def, x.last_use, x.overwritten))
            .collect::<Vec<_>>(),
        [
            (0, Some(1), None),
            (0, Some(5), None),
            (0, Some(3), None),
            (2, Some(3), Some(4)),
            (4, Some(5), None),
            (6, Some(7), None)
        ]
    );
    assert_eq!(
        uses(&p)
            .iter()
            .map(|x| (x.at, x.value, x.kind))
            .collect::<Vec<_>>(),
        [
            (1, 0, "left"),
            (1, 1, "right"),
            (3, 3, "left"),
            (3, 2, "right"),
            (5, 1, "left"),
            (5, 4, "right"),
            (7, 5, "region_result")
        ]
    );
}
#[test]
fn self_move_reads_old_version_before_new_definition() {
    let p = plan(&[8, 72]);
    let v = values(&p);
    assert_eq!(v[3].last_use, Some(3));
    assert_eq!(v[3].overwritten, Some(4));
    assert_eq!(v[4].def, 4);
    assert_eq!(v[4].last_use, Some(5));
    assert_eq!(
        uses(&p).iter().map(|x| (x.at, x.value)).collect::<Vec<_>>(),
        [(1, 0), (3, 3), (5, 4)]
    );
}
#[test]
fn both_binary_operands_read_same_old_destination_version() {
    let p = plan(&[8, 585]);
    let u = uses(&p);
    assert_eq!((u[1].at, u[1].value, u[1].kind), (3, 3, "left"));
    assert_eq!((u[2].at, u[2].value, u[2].kind), (3, 3, "right"));
    assert_eq!(values(&p)[4].def, 4);
    assert_eq!(u[3].value, 4);
}
#[test]
fn unused_input_and_dead_redefinitions_stay_distinct() {
    let p = plan(&[0, 0, 8]);
    let v = values(&p);
    assert_eq!(v.len(), 6);
    assert_eq!(v[1].last_use, None);
    assert_eq!(v[2].last_use, None);
    assert_eq!(v[3].last_use, None);
    assert_eq!(v[3].overwritten, Some(4));
    assert_eq!(v[4].last_use, None);
    assert_eq!(v[4].overwritten, None);
    assert_eq!(v[5].last_use, Some(7));
}
#[test]
fn full_sixteen_binary_steps_fit_exact_nineteen_values_thirty_three_uses() {
    let p = plan(&[141; 16]);
    assert_eq!(p.values.len, 19);
    assert_eq!(p.uses.len, 33);
    assert_eq!(p.result_boundary, 33);
    assert_eq!(values(&p)[18].last_use, Some(33));
    for value in &values(&p)[3..18] {
        assert_eq!(value.last_use, None);
        assert!(value.overwritten.is_some());
    }
}
#[test]
fn fixed_rows_refuse_one_past_capacity_without_append() {
    let mut rows = Rows::<u8, 2>::new();
    rows.push(1).unwrap();
    rows.push(2).unwrap();
    assert!(rows.push(3).is_err());
    assert_eq!(rows.len, 2);
    assert_eq!(rows.rows, [Some(1), Some(2)]);
}
#[test]
fn missing_definition_and_after_overwrite_use_refuse() {
    let mut p = plan(&[8, 72]);
    assert!(p.use_value(19, 5, "left").is_err());
    assert!(p.use_value(3, 5, "left").is_err());
    assert!(p.use_value(4, 3, "left").is_err());
}
#[test]
fn all_corpus_plans_preserve_uses_and_exact_per_role_bindings() {
    for case in fixture::cases() {
        let p = plan(&case.descriptors);
        let v = values(&p);
        let mut current = [Some(0_u8), Some(1), Some(2), None, None];
        let mut expected = Vec::new();
        for (i, &word) in case.descriptors.iter().enumerate() {
            let operands = if word % 8 == 0 {
                vec![((word / 16) % 8, "move")]
            } else {
                vec![((word / 16) % 8, "left"), ((word / 128) % 8, "right")]
            };
            for (role, kind) in operands {
                expected.push((2 * i as u8 + 1, current[usize::from(role)].unwrap(), kind));
            }
            let dest = 3 + usize::from((word / 8) % 2);
            current[dest] = Some((3 + i) as u8);
            assert_eq!(v[3 + i].binding, [34, 35, 36, 32, 33][dest]);
        }
        expected.push((p.result_boundary, current[4].unwrap(), "region_result"));
        assert_eq!(
            uses(&p)
                .iter()
                .map(|x| (x.at, x.value, x.kind))
                .collect::<Vec<_>>(),
            expected
        );
    }
}
#[test]
fn normalized_json_and_timeline_keep_owner_identity_and_false_authority() {
    for used in [false, true] {
        let owner = fixture::owner(&fixture::module_with_program(
            used,
            fixture::program(&[133, 307, 413]),
        ));
        let input = crate::load_debug_simulation_input_bytes_v17(
            owner.canonical_bytes(),
            &fixture::request([19, 23, 42]),
        )
        .unwrap();
        let view = inspect(&input).unwrap();
        let observed = report(&input, &view);
        let b = render_output(&observed, OutputFormat::PlannedRegistersJson).unwrap();
        let j: serde_json::Value = serde_json::from_slice(b.as_bytes()).unwrap();
        assert_eq!(j["schema"], "fe2o3-declared-register-demand-v1");
        assert_eq!(
            j["canonical"],
            serde_json::to_value(&observed.canonical).unwrap()
        );
        assert_eq!(
            j["declared_source_ids"],
            serde_json::to_value(&observed.declared_source_ids).unwrap()
        );
        assert_eq!(
            j["coordinate"],
            serde_json::to_value(&observed.coordinate).unwrap()
        );
        for k in [
            "physical_allocation_observed",
            "physical_register_values_available",
            "instruction_microsteps_available",
            "source_authentication",
            "artifact_authority",
            "production_resume_authority",
            "hardware_execution",
        ] {
            assert_eq!(j[k], false);
        }
        assert_eq!(j["plan"]["uses"][6]["kind"], "region_result");
        assert!(b.as_bytes().len() < OUTPUT_BYTES);
        let t = render_output(&observed, OutputFormat::PlannedRegisters).unwrap();
        let text = std::str::from_utf8(t.as_bytes()).unwrap();
        assert!(text.contains("not LLVM allocation"));
        assert!(text.contains("D=definition r=operand read R=result handoff"));
        assert!(text.contains("v32 scratch"));
        assert!(text.contains("not allocation/free events"));
    }
}
#[test]
fn maximum_case_and_minmax_register_bindings_fit_both_outputs() {
    let mut module = fixture::module_with_program(true, fixture::program(&[141; 16]));
    let fe2o3_kernel_ir::OperationKind::Gfx942OrderedProgram(program) =
        &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
    else {
        panic!()
    };
    *program = Gfx942OrderedProgramV1::new(
        program.source(),
        fe2o3_kernel_ir::Gfx942OrderedProgramRegistersV1::new(0, 63, [1, 2, 3]).unwrap(),
        *program.inputs(),
        *program.program(),
    )
    .unwrap();
    let owner = fixture::owner(&module);
    let input = crate::load_debug_simulation_input_bytes_v17(
        owner.canonical_bytes(),
        &fixture::request([19, 23, 42]),
    )
    .unwrap();
    let view = inspect(&input).unwrap();
    for format in [
        OutputFormat::PlannedRegisters,
        OutputFormat::PlannedRegistersJson,
    ] {
        assert!(
            render_output(&report(&input, &view), format)
                .unwrap()
                .as_bytes()
                .len()
                <= OUTPUT_BYTES
        );
    }
}
#[test]
fn escaped_names_refuse_without_successful_partial_output() {
    let owner = fixture::owner(&fixture::module(true));
    let input = crate::load_debug_simulation_input_bytes_v17(
        owner.canonical_bytes(),
        &fixture::request([19, 23, 42]),
    )
    .unwrap();
    let view = inspect(&input).unwrap();
    let mut observed = report(&input, &view);
    let name = "\u{1b}".repeat(1024);
    observed.kernel = &name;
    observed.function = &name;
    for format in [
        OutputFormat::PlannedRegisters,
        OutputFormat::PlannedRegistersJson,
    ] {
        assert!(render_output(&observed, format).is_err());
    }
}

#[test]
fn fixed_representation_accounting_is_separate_and_compiler_measured() {
    assert_eq!(PLAN_BYTES, std::mem::size_of::<Plan>());
    assert_eq!(PROJECTION_BYTES, std::mem::size_of::<Normalized<'static>>());
    assert!(PLAN_BYTES + PROJECTION_BYTES <= 4096);
    assert_eq!(super::super::CPU_PREFLIGHT_RESIDENT_BYTES, 64 * 1024 * 1024);
    let owner = fixture::owner(&fixture::module(true));
    let input = crate::load_debug_simulation_input_bytes_v17(
        owner.canonical_bytes(),
        &fixture::request([19, 23, 42]),
    )
    .unwrap();
    let view = inspect(&input).unwrap();
    let out = render_output(&report(&input, &view), OutputFormat::PlannedRegistersJson).unwrap();
    let value: serde_json::Value = serde_json::from_slice(out.as_bytes()).unwrap();
    assert_eq!(value["fixed_plan_bytes"], PLAN_BYTES);
    assert_eq!(value["fixed_projection_bytes"], PROJECTION_BYTES);
    assert_eq!(value["fixed_representations_limit_bytes"], 4096);
    assert!(
        value["accounting_scope"]
            .as_str()
            .unwrap()
            .contains("not total stack")
    );
}

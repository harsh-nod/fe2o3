use super::super::source_function::tests::with_slots;
use super::*;

const LIMIT: usize = 512 * 1024 * 1024;

fn theorem<'a>(text: &'a str, name: &str) -> &'a str {
    text.split_once(&format!("proof fn {name}("))
        .unwrap()
        .1
        .split("proof fn ")
        .next()
        .unwrap()
}

fn assert_leading_opacity_headers(theorem: &str) {
    let body = theorem
        .split_once("\n{\n")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    let mut statements = false;
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if line.starts_with("hide(") {
            assert!(
                !statements,
                "opacity header after a proof statement: {line}"
            );
        } else {
            statements = true;
        }
    }
}

pub(super) fn check_four(text: &str, name: &str, root: usize) {
    let header = theorem(text, name).split_once("\n{\n").unwrap().0;
    for conclusion in [
        format!(
            "invocation_paired_related_{root}_v36(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state)"
        ),
        format!(
            "invocation_paired_observations_related_{root}_v39(invocation_paired_source_step_{root}_v36(source).events, invocation_paired_actual_step_{root}_v36(target).events)"
        ),
        format!(
            "invocation_paired_source_step_{root}_v36(source).halted == invocation_paired_actual_step_{root}_v36(target).halted"
        ),
        format!(
            "source.machine.pc >= 0 ==> invocation_paired_control_values_{root}_v36(source, invocation_source_block_runtime_{root}_v36(source), invocation_byte_boundary_{root}_v36(target))"
        ),
    ] {
        assert!(header.contains(&conclusion), "{name}: {conclusion}");
    }
}

#[test]
fn original_mir_step_local_observation_law_keeps_validity_and_closed_operand_domain() {
    let text = include_str!("original_semantic_mir_source_entry_observations_v180.vrs");
    let predicate = text
        .split_once(") -> bool {\n")
        .unwrap()
        .1
        .split_once("\n}\n")
        .unwrap()
        .0;
    assert_eq!(
        predicate,
        concat!(
            "    observation.before.machine.valid && observation.after.machine.valid\n",
            "        && match observation.operand {\n",
            "            InvocationSourceOperandV36::Scalar {\n",
            "                value: InvocationSourceByteValueV36::Local { .. }, ..\n",
            "            } => true,\n",
            "            _ => false,\n",
            "        }",
        )
    );
    assert_eq!(text.matches("proof fn ").count(), 3);
    assert_eq!(text.matches("#[verifier::spinoff_prover]").count(), 2);
    for required in [
        "decreases observations.len(),",
        "assert(tail[i] == observations[i + 1]);",
        "invocation_source_local_observations_empty_v180(tail, little_endian);",
        "requires result.observations.len() == 0, result.source.machine.pc != -2,",
        "forall|i: int| 0 <= i < result.operands.len() ==>\n            invocation_source_local_observation_v180(result.operands[i]),",
        "ensures invocation_source_observations_v39(result, little_endian) == Seq::empty(),",
        "requires !observation.before.machine.valid || !observation.after.machine.valid",
        "ensures !invocation_source_local_observation_v180(observation),",
    ] {
        assert!(
            text.contains(required),
            "missing observation guard: {required}"
        );
    }
    for forbidden in ["assume(", "admit(", "external_body", "assume_specification"] {
        assert!(!text.contains(forbidden));
    }
}

#[test]
fn original_mir_step_partitions_use_all_authentic_roots_cuts_and_constructor_coordinates() {
    for roots in [2, 3] {
        super::super::super::invocations::tests::run_root_variant(
            LIMIT, LIMIT, true, false, roots, |plan, out| {
                with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    let paired = PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                    // Copied proof coordinates do not prevent the original owner from emitting.
                    program.emit(out)?;
                    paired.emit(out)?;
                    let copy_law = theorem(&out.text, "invocation_source_scalar_copy_valid_identity_v164");
                    assert_eq!(out.text.matches("proof fn invocation_source_scalar_copy_valid_identity_v164(").count(), 1);
                    assert!(!copy_law.contains(" requires "));
                    assert_eq!(copy_law.matches(".source.machine.valid ==>").count(), 2);
                    assert!(copy_law.contains(".source == source,"));
                    assert!(copy_law.contains(".value == InvocationSourceValueV42::Carrier(source.machine.values[local]),"));
                    assert!(!out.text.contains("proof fn invocation_source_cut_summary_"));
                    assert_eq!(out.text.matches("proof fn invocation_related_target_inputs_v96").count(), 1);
                    for (root, row) in paired.roots.iter().enumerate() {
                        let follow_fuel = generate::target_follow_fuel_for_test(&paired, root, out)?;
                        let inventory = paired.slots.correspondence(out)?.inventory(out.budget)?;
                        let mut state_summaries = 0;
                        let hints = row.step_hints.as_ref().unwrap();
                        assert!(hints.conserves_heap);
                        assert!(!out.text.contains("proof fn invocation_scalar_store_"));
                        assert!(!out.text.contains("proof fn invocation_cut_source_frame_"));
                        assert!(!out.text.contains("proof fn invocation_cut_frame_"));
                        check_four(&out.text, &format!("invocation_paired_step_{root}_v36"), root);
                        check_four(&out.text, &format!("invocation_paired_cut_{root}_terminal_all_v85"), root);
                        let dispatcher = theorem(&out.text, &format!("invocation_paired_step_{root}_v36"));
                        for (block, cut) in row.cuts.iter().enumerate() {
                            let Some(cut) = cut else { continue };
                            let pc = cut.source;
                            check_four(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_all_v85"), root);
                            assert!(dispatcher.contains(&format!("else if source.machine.pc == {pc}")));
                            if let End::Call(child) = cut.end {
                                let index = child - row.instances.start;
                                let entry = hints.entries[index].as_ref().unwrap();
                                let map = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_map_v85"));
                                let hint = hints.cuts.iter().find(|hint| hint.pc == pc).unwrap();
                                assert_eq!(entry.arguments.len(), hint.call.as_ref().unwrap().arguments.len());
                                assert_eq!(map.matches("reveal_with_fuel(invocation_source_micro_run_").count(), 1);
                                assert!(map.contains(&format!("reveal_with_fuel(invocation_source_micro_run_{root}_{}_v36, {});", hint.instance, hint.statements + 1)));
                                assert!(map.contains(&format!("reveal_with_fuel(invocation_byte_follow_{root}_v36, {follow_fuel});")));
                                let control = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_control_v85"));
                                for predicate in ["invocation_paired_source_step", "invocation_paired_source_defined", "invocation_source_byte_storage_related", "invocation_source_byte_map", "invocation_paired_control_values"] {
                                    assert!(control.contains(&format!(" hide({predicate}_{root}_v36);")));
                                }
                                let state_name = format!("invocation_constructor_states_{root}_{pc}_v162");
                                let has_state = out.text.contains(&format!("proof fn {state_name}("));
                                assert_eq!(control.contains("hide(invocation_paired_actual_step_"), has_state);
                                if has_state {
                                    state_summaries += 1;
                                    let source_declaration = format!("spec fn invocation_constructor_source_{root}_{pc}_v162(");
                                    let source_expression = out.text.split_once(&source_declaration).unwrap().1.split_once("\n}\n").unwrap().0;
                                    assert!(source_expression.contains(&format!(
                                        " let entered = invocation_source_entry_initialize_v166(source, {}, {}, {}, byte_enter_frame_v30(source.machine.frames, {}));",
                                        entry.pc, entry.locals.start, entry.locals.end, entry.owner
                                    )));
                                    assert_eq!(source_expression.matches("invocation_source_entry_initialize_v166(").count(), 1);
                                    assert!(!source_expression.contains("Seq::new("));
                                    let target_declaration = format!("spec fn invocation_constructor_target_{root}_{pc}_v162(");
                                    let target_state = out.text.split_once(&target_declaration).unwrap().1.split_once("\n}\n").unwrap().0;
                                    let mut current = row.blocks.start + block;
                                    let mut visited = std::collections::BTreeSet::new();
                                    let mut bindings = 0;
                                    loop {
                                        assert!(visited.insert(current));
                                        let actual = &inventory.blocks()[current];
                                        assert!(actual.operations.is_empty());
                                        assert!(matches!(actual.terminator, fe2o3_kernel_ir::Terminator::Branch { .. }));
                                        assert_eq!(actual.edges.len(), 1);
                                        let edge = &inventory.edges()[actual.edges.start];
                                        assert_eq!(edge.target.function, actual.coordinate.function);
                                        for transfer in &inventory.edge_arguments()[edge.bindings.clone()] {
                                            assert!(target_state.contains(&format!(".update({}, before[{}])", transfer.target_definition, transfer.incoming_definition)));
                                            bindings += 1;
                                        }
                                        current = row.blocks.start + edge.target.block as usize;
                                        if let Some(next) = &row.cuts[current - row.blocks.start] {
                                            assert_eq!(next.source, entry.pc);
                                            assert_eq!(next.instance, child);
                                            break;
                                        }
                                    }
                                    assert_eq!(target_state.matches(" let before = values;").count(), visited.len());
                                    assert_eq!(target_state.matches(".update(").count(), bindings);
                                    assert!(!target_state.contains(".update(values["));
                                    assert!(target_state.contains(&format!("MemoryStateV30 {{ pc: {current}, values, valid: true, ..target }}")));
                                    let state = theorem(&out.text, &state_name);
                                    assert_leading_opacity_headers(state);
                                    assert!(state.contains(&format!("hide(invocation_source_block_runtime_{root}_v36);")));
                                    assert!(state.contains(&format!("hide(invocation_byte_boundary_{root}_v36);")));
                                    assert!(state.contains(&format!("hide(invocation_constructor_source_{root}_{pc}_v162);")));
                                    let source_name = format!("invocation_constructor_source_state_{root}_{pc}_v165");
                                    let source_state = theorem(&out.text, &source_name);
                                    let runtime_name = format!("invocation_constructor_source_runtime_{root}_{pc}_v167");
                                    let runtime_state = theorem(&out.text, &runtime_name);
                                    let record_name = format!("invocation_constructor_source_record_{root}_{pc}_v181");
                                    let record = theorem(&out.text, &record_name);
                                    let call = hint.call.as_ref().unwrap();
                                    assert_leading_opacity_headers(source_state);
                                    assert_leading_opacity_headers(runtime_state);
                                    assert_eq!(state.matches(&format!(" {source_name}(source);")).count(), 1);
                                    assert_eq!(source_state.matches(&format!(" {runtime_name}(source);")).count(), 1);
                                    assert!(!runtime_state.contains(&format!(" {source_name}(source);")));
                                    assert!(!source_state.contains("copied_"));
                                    assert!(!source_state.contains("invocation_source_scalar_copy_valid_identity_v164("));
                                    assert_leading_opacity_headers(record);
                                    let (record_premises, record_conclusions) = record.split_once(" requires ").unwrap().1.split_once(" ensures").unwrap();
                                    assert_eq!(record_premises.trim(), format!("source.machine.pc == {pc}, invocation_source_block_runtime_{root}_v36(source).source.machine.valid,"));
                                    assert!(!record.contains("invocation_paired_source_defined_"));
                                    assert!(!record.contains("invocation_constructor_source_"));
                                    assert!(!record.contains("forall|"));
                                    assert!(record.contains(&format!("hide(invocation_source_enter_{root}_{}_v36);", call.child)));
                                    assert!(record.contains("hide(invocation_source_value_evaluate_v42);"));
                                    assert!(!record.contains("reveal(invocation_source_enter_"));
                                    assert!(!record.contains("invocation_source_entry_initialize_v166"));
                                    assert!(record_conclusions.contains(&format!("result.source == invocation_source_enter_{root}_{}_v36(copied_{}, entered_arguments, invocation_runtime_little_endian_v36())", call.child, call.arguments.len())));
                                    assert!(record_conclusions.contains(&format!("result.returned.is_none() && result.observations.len() == 0 && result.operands.len() == {}", call.arguments.len())));
                                    assert_eq!(runtime_state.matches(&format!(" {record_name}(source);")).count(), 1);
                                    for proof in [source_state, runtime_state] {
                                        assert!(!proof.contains("target"));
                                        assert!(!proof.contains("invocation_paired_related_"));
                                        assert!(!proof.contains("invocation_paired_source_preserved_"));
                                        assert!(proof.contains("hide(invocation_source_entry_initialize_v166);"));
                                        assert!(!proof.contains("reveal(invocation_source_entry_initialize_v166);"));
                                    }
                                    assert!(runtime_state.contains("hide(invocation_source_value_evaluate_v42);"));
                                    assert!(runtime_state.contains("hide(invocation_source_byte_put_local_v36);"));
                                    assert!(!runtime_state.contains("reveal(invocation_source_byte_put_local_v36);"));
                                    assert!(!source_state.contains("hide(invocation_source_byte_put_local_v36);"));
                                    assert!(runtime_state.contains("hide(invocation_source_entry_select_v167);"));
                                    assert!(!runtime_state.contains("reveal(invocation_source_entry_select_v167);"));
                                    assert!(runtime_state.contains(&format!("hide(invocation_source_entry_refuses_{root}_{}_v167);", call.child)));
                                    assert!(!runtime_state.contains(&format!("reveal(invocation_source_entry_refuses_{root}_{}_v167);", call.child)));
                                    assert!(runtime_state.contains(&format!("hide(invocation_source_entry_body_{root}_{}_v167);", call.child)));
                                    assert_eq!(runtime_state.matches("invocation_source_entry_select_success_v167(").count(), 1);
                                    assert!(runtime_state.contains(&format!("invocation_source_entry_select_success_v167(copied_{0}, invocation_source_entry_refuses_{root}_{1}_v167(copied_{0}, entered_arguments, invocation_runtime_little_endian_v36()), invocation_source_entry_body_{root}_{1}_v167(copied_{0}, entered_arguments, invocation_runtime_little_endian_v36()));", call.arguments.len(), call.child)));
                                    let pc_name = format!("invocation_constructor_source_pc_{root}_{pc}_v182");
                                    let pc_proof = theorem(&out.text, &pc_name);
                                    let (pc_contract, pc_body) = pc_proof.split_once("\n{\n").unwrap();
                                    assert_eq!(pc_contract, format!("source: InvocationSourceByteStateV36)\n ensures invocation_constructor_source_{root}_{pc}_v162(source).machine.pc == {},", entry.pc));
                                    assert_leading_opacity_headers(pc_proof);
                                    for forbidden in [" requires ", "invocation_paired_source_defined_", "invocation_source_block_runtime_", "target", "assume(", "admit("] {
                                        assert!(!pc_proof.contains(forbidden));
                                    }
                                    assert_eq!(pc_body.matches("invocation_source_entry_initialize_pc_v166(").count(), 1);
                                    assert!(pc_body.contains(&format!("invocation_source_entry_initialize_pc_v166(source, {}, {}, {}, byte_enter_frame_v30(source.machine.frames, {}));", entry.pc, entry.locals.start, entry.locals.end, entry.owner)));
                                    assert!(pc_body.contains(&format!("let installed_0 = invocation_source_entry_initialize_v166(source, {}, {}, {}, byte_enter_frame_v30(source.machine.frames, {}));", entry.pc, entry.locals.start, entry.locals.end, entry.owner)));
                                    assert_eq!(pc_body.matches("invocation_source_entry_put_local_pc_v179(").count(), call.arguments.len());
                                    for (ordinal, (destination, (local, _, _))) in entry.arguments.iter().zip(&call.arguments).enumerate() {
                                        assert!(pc_body.contains(&format!("invocation_source_entry_put_local_pc_v179(installed_{ordinal}, {destination}, source.machine.values[{local}]);\n let installed_{} = invocation_source_byte_put_local_v36(installed_{ordinal}, {destination}, source.machine.values[{local}]);", ordinal + 1)));
                                    }
                                    let no_trap = format!(" {pc_name}(source);");
                                    for consumer in [source_state, runtime_state] {
                                        assert_eq!(consumer.matches(&no_trap).count(), 1);
                                        assert!(!consumer.contains("invocation_source_entry_initialize_pc_v166("));
                                        assert!(!consumer.contains("invocation_source_entry_put_local_pc_v179("));
                                        assert!(!consumer.contains("let installed_"));
                                    }
                                    assert!(pc_body.contains(&format!("assert(invocation_constructor_source_{root}_{pc}_v162(source).machine.pc == {});", entry.pc)));
                                    let observed = format!(" invocation_source_local_block_observations_empty_v180(invocation_source_block_runtime_{root}_v36(source), invocation_runtime_little_endian_v36());");
                                    assert_eq!(runtime_state.matches(&observed).count(), 1);
                                    assert!(runtime_state.find(&no_trap).unwrap() < runtime_state.find(&observed).unwrap());
                                    assert!(runtime_state.contains(&format!(" && invocation_source_block_runtime_{root}_v36(source).observations.len() == 0")));
                                    assert!(runtime_state.contains(&format!("assert forall|i: int| 0 <= i < invocation_source_block_runtime_{root}_v36(source).operands.len() implies invocation_source_local_observation_v180(invocation_source_block_runtime_{root}_v36(source).operands[i]) by {{")));
                                    assert!(!runtime_state.contains("reveal(invocation_source_observations_v39);"));
                                    assert!(!runtime_state.contains("reveal_with_fuel(invocation_source_operands_observations_v39,"));
                                    assert!(!runtime_state.contains("reveal_with_fuel(invocation_source_statements_observations_v39,"));
                                    assert!(source_state.contains(&format!("assert(invocation_paired_source_step_{root}_v36(source).state.machine.pc >= 0\n && invocation_paired_source_step_{root}_v36(source).state.machine.pc != -2);")));
                                    assert!(runtime_state.contains(&format!("assert(invocation_paired_source_step_{root}_v36(source).state.machine.valid) by {{\n reveal_with_fuel(invocation_paired_source_defined_{root}_v36, 2);\n }}")));
                                    assert!(record.contains(&format!("reveal_with_fuel(invocation_source_micro_run_{root}_{}_v36, {});", hint.instance, hint.statements + 1)));
                                    assert!(!runtime_state.contains("reveal_with_fuel(invocation_source_micro_run_"));
                                    assert!(!state.contains("reveal_with_fuel(invocation_source_micro_run_"));
                                    assert!(!state.contains("invocation_source_scalar_copy_valid_identity_v164("));
                                    assert_eq!(runtime_state.matches(" invocation_source_scalar_copy_valid_identity_v164(").count(), call.arguments.len());
                                    for (ordinal, (local, moved, bits)) in call.arguments.iter().enumerate() {
                                        assert!(!moved);
                                        assert!(runtime_state.contains(&format!(" invocation_source_scalar_copy_valid_identity_v164(copied_{ordinal}, {local}, {bits}, {root}, {}, invocation_runtime_little_endian_v36());", hint.instance)));
                                        assert!(runtime_state.contains(&format!(" let captured_{ordinal} = invocation_source_value_evaluate_v42(copied_{ordinal}, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }}, {root}, {}, invocation_runtime_little_endian_v36());\n let copied_{} = captured_{ordinal}.source;", hint.instance, ordinal + 1)));
                                        assert!(record.contains(&format!(" let captured_{ordinal} = invocation_source_value_evaluate_v42(copied_{ordinal}, InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }}, {root}, {}, invocation_runtime_little_endian_v36());\n let copied_{} = captured_{ordinal}.source;", hint.instance, ordinal + 1)));
                                        assert!(record_conclusions.contains(&format!("result.operands[{ordinal}].before == copied_{ordinal}\n && result.operands[{ordinal}].after == copied_{}\n && result.operands[{ordinal}].value == captured_{ordinal}.value\n && result.operands[{ordinal}].operand == (InvocationSourceOperandV36::Scalar {{ value: InvocationSourceByteValueV36::Local {{ local: {local}int, moved: false }}, bits: {bits}int }})", ordinal + 1)));
                                        assert!(runtime_state.contains(&format!("if i == {ordinal} {{ assert(invocation_source_local_observation_v180(invocation_source_block_runtime_{root}_v36(source).operands[{ordinal}])); }} else")));
                                    }
                                    let arguments = (0..call.arguments.len()).map(|ordinal| format!("captured_{ordinal}.value,")).collect::<String>();
                                    assert!(runtime_state.contains(&format!(" let entered_arguments = seq![{arguments}];")));
                                    assert!(record.contains(&format!(" let entered_arguments = seq![{arguments}];")));
                                    assert_eq!(runtime_state.matches(&format!("reveal(invocation_source_block_runtime_{root}_v36);")).count(), 0);
                                    assert!(!source_state.contains(&format!("reveal(invocation_source_block_runtime_{root}_v36);")));
                                    assert!(!state.contains(&format!("reveal(invocation_source_block_runtime_{root}_v36);")));
                                    assert_eq!(state.matches(&format!("reveal(invocation_byte_boundary_{root}_v36);")).count(), 1);
                                    let captured = format!("assert(copied_{}.machine.valid) by {{\n reveal(invocation_source_enter_{root}_{}_v36);\n }}\n assert(copied_{} == source);", call.arguments.len(), call.child, call.arguments.len());
                                    assert!(runtime_state.find(&captured).unwrap() < runtime_state.find(&format!(" assert(invocation_source_block_runtime_{root}_v36(source).source ==")).unwrap());
                                    let valid = format!("assert(invocation_source_block_runtime_{root}_v36(source).source.machine.valid) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n }}");
                                    assert!(runtime_state.find(&valid).unwrap() < runtime_state.find(&format!(" assert(invocation_source_block_runtime_{root}_v36(source).source ==")).unwrap());
                                    assert!(runtime_state.find(&valid).unwrap() < runtime_state.find(&format!(" {record_name}(source);")).unwrap());
                                    assert!(runtime_state.find(&format!(" {record_name}(source);")).unwrap() < runtime_state.find(&captured).unwrap());
                                    let source_projection = runtime_state.split_once(&format!(" assert(invocation_source_block_runtime_{root}_v36(source).source ==")).unwrap().1.split_once("\n }\n").unwrap().0;
                                    assert!(!source_projection.contains(&format!("reveal(invocation_source_block_runtime_{root}_v36);")));
                                    assert!(!source_projection.contains("forall|"));
                                    assert!(source_projection.contains(&format!("reveal(invocation_source_entry_body_{root}_{}_v167);", call.child)));
                                    assert!(!source_projection.contains("invocation_byte_boundary_"));
                                    let target_projection = state.split_once(&format!(" assert(invocation_byte_boundary_{root}_v36(target).state ==")).unwrap().1.split_once("\n }\n").unwrap().0;
                                    assert!(target_projection.contains(&format!("reveal(invocation_byte_boundary_{root}_v36);")));
                                    assert!(!target_projection.contains("invocation_source_block_runtime_"));
                                    for side in ["source", "actual"] {
                                        let argument = if side == "source" { "source" } else { "target" };
                                        let proof = if side == "source" { source_state } else { state };
                                        assert!(proof.contains(&format!("assert(!invocation_paired_{side}_step_{root}_v36({argument}).halted) by {{\n reveal(invocation_paired_{side}_step_{root}_v36);\n }}")));
                                    }
                                    let (premises, conclusions) = state.split_once(" requires ").unwrap().1.split_once(" ensures").unwrap();
                                    assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1), source.machine.pc == {pc},"));
                                    let contract = conclusions.split_once("\n{\n").unwrap().0;
                                    let (source_premises, source_conclusions) = source_state.split_once(" requires ").unwrap().1.split_once(" ensures").unwrap();
                                    assert_eq!(source_premises.trim(), format!("invocation_paired_source_defined_{root}_v36(source, 1), source.machine.pc == {pc},"));
                                    let source_contract = source_conclusions.split_once("\n{\n").unwrap().0;
                                    let expected_source_contract: Vec<_> = contract.lines().filter(|line| !line.contains("target") && !line.trim().is_empty()).collect();
                                    assert_eq!(source_contract.lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>(), expected_source_contract);
                                    let (runtime_premises, runtime_conclusions) = runtime_state.split_once(" requires ").unwrap().1.split_once(" ensures").unwrap();
                                    assert_eq!(runtime_premises, source_premises);
                                    let runtime_contract = runtime_conclusions.split_once("\n{\n").unwrap().0;
                                    let observed = format!(" invocation_source_observations_v39(invocation_source_block_runtime_{root}_v36(source), invocation_runtime_little_endian_v36()) == Seq::empty(),");
                                    let mut expected_runtime: Vec<_> = source_contract.lines().filter(|line| line.contains("invocation_source_block_runtime_")).collect();
                                    expected_runtime.push(&observed);
                                    assert_eq!(runtime_contract.lines().filter(|line| !line.trim().is_empty()).collect::<Vec<_>>(), expected_runtime);
                                    for (side, argument, value) in [("source", "source", "source"), ("actual", "target", "target")] {
                                        assert!(contract.contains(&format!("invocation_paired_{side}_step_{root}_v36({argument}).state == invocation_constructor_{value}_{root}_{pc}_v162({argument})")));
                                        assert!(contract.contains(&format!("invocation_paired_{side}_step_{root}_v36({argument}).events.len() == 0")));
                                        assert!(contract.contains(&format!("!invocation_paired_{side}_step_{root}_v36({argument}).halted")));
                                    }
                                    for part in ["map", "heap", "residual", "observations", "halted", "control"] {
                                        let body = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_{part}_v85"));
                                        assert_leading_opacity_headers(body);
                                        assert_eq!(body.matches(&format!("{state_name}(source, target);")).count(), 1);
                                        for name in ["invocation_paired_source_step", "invocation_paired_actual_step", "invocation_source_block_runtime", "invocation_byte_boundary"] {
                                            assert!(body.contains(&format!("hide({name}_{root}_v36);")));
                                        }
                                    }
                                }
                                assert!(control.contains(&format!("hide(invocation_source_enter_{root}_{index}_v36);")));
                                assert!(out.text.contains(&format!("spec fn invocation_source_enter_{root}_{index}_v36(")));
                                assert_eq!(control.matches("hide(invocation_source_enter_").count(), 1);
                                if index != child {
                                    assert!(!control.contains(&format!("hide(invocation_source_enter_{root}_{child}_v36);")));
                                }
                                for predicate in ["invocation_byte_states_related_v36", "invocation_source_byte_state_well_formed_v36", "byte_memory_well_formed_v30", "byte_frame_runtime_well_formed_v30", "byte_private_frames_live_v30", "private_generation_counters_valid_v30"] {
                                    assert!(control.contains(&format!(" hide({predicate});")));
                                }
                                assert!(!control.contains("hide(byte_state_memory_well_formed_v30)"));
                                assert!(control.contains(&format!("assert(original.source.machine.valid) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n }}")));
                                assert!(control.contains(&format!("assert(invocation_source_byte_state_well_formed_v36(source) && invocation_byte_states_related_v36(source.machine, target, invocation_source_byte_map_{root}_v36(source, target))) by {{\n reveal(invocation_source_byte_storage_related_{root}_v36);\n }}")));
                                assert!(control.contains(&format!("invocation_related_target_inputs_v96(source.machine, target, invocation_source_byte_map_{root}_v36(source, target));")));
                                assert!(control.contains(&format!("assert(invocation_paired_control_values_{root}_v36(source, original, actual)) by {{\n reveal(invocation_paired_control_values_{root}_v36);\n }}")));
                                let observations = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_observations_v85"));
                                assert!(observations.contains(&format!("hide(invocation_source_enter_{root}_{index}_v36);")));
                                assert_eq!(observations.matches("hide(invocation_source_enter_").count(), 1);
                                if index != child {
                                    assert!(!observations.contains(&format!("hide(invocation_source_enter_{root}_{child}_v36);")));
                                }
                                assert!(observations.contains(&format!("assert(invocation_paired_source_step_{root}_v36(source).state.machine.pc != -2) by {{\n reveal(invocation_paired_source_step_{root}_v36);\n reveal(invocation_source_enter_{root}_{index}_v36);\n }}")));
                                assert!(observations.contains(&format!("assert(invocation_source_byte_state_well_formed_v36(source)) by {{\n reveal(invocation_source_byte_storage_related_{root}_v36);\n }}")));
                                assert!(observations.contains(&format!("assert(target.pc == {}) by {{\n reveal(invocation_paired_related_{root}_v36);\n }}", row.blocks.start + block)));
                                for side in ["source", "actual"] {
                                    let argument = if side == "source" { "source" } else { "target" };
                                    assert!(observations.contains(&format!("hide(invocation_{side}_observations_v39);")));
                                    assert!(observations.contains(&format!("assert(invocation_paired_{side}_step_{root}_v36({argument}).events == Seq::empty()) by {{\n reveal(invocation_paired_{side}_step_{root}_v36);\n reveal(invocation_{side}_observations_v39);\n }}")));
                                }
                                let premises = observations.split_once(" requires ").unwrap().1.split_once(" ensures ").unwrap().0;
                                assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},"));
                                let residual = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_residual_v85"));
                                for predicate in ["invocation_source_byte_storage_related", "invocation_paired_source_defined"] {
                                    assert!(residual.contains(&format!("hide({predicate}_{root}_v36);")));
                                }
                                for predicate in ["invocation_byte_states_related_v36", "invocation_source_byte_state_well_formed_v36", "byte_memory_well_formed_v30", "byte_frame_runtime_well_formed_v30", "byte_private_frames_live_v30", "private_generation_counters_valid_v30"] {
                                    assert!(residual.contains(&format!("hide({predicate});")));
                                }
                                assert!(residual.contains(&format!("assert(invocation_source_byte_state_well_formed_v36(source) && invocation_byte_states_related_v36(source.machine, target, invocation_source_byte_map_{root}_v36(source, target))) by {{\n reveal(invocation_source_byte_storage_related_{root}_v36);\n }}")));
                                assert!(residual.contains(&format!("invocation_related_target_inputs_v96(source.machine, target, invocation_source_byte_map_{root}_v36(source, target));")));
                                for predicate in ["invocation_source_enter_", "invocation_source_byte_map_", "invocation_paired_residual_", "byte_state_memory_well_formed_v30", "invocation_value_related_v36"] {
                                    assert!(!residual.contains(&format!("hide({predicate}")));
                                }
                                for side in ["source", "actual"] {
                                    assert_eq!(residual.contains(&format!("hide(invocation_paired_{side}_step_")), has_state);
                                }
                                let (premises, conclusion) = residual.split_once(" requires ").unwrap().1.split_once(" ensures ").unwrap();
                                assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},"));
                                assert_eq!(conclusion.split_once("\n{\n").unwrap().0.trim(), format!("invocation_paired_residual_{root}_v85(invocation_paired_source_step_{root}_v36(source).state, invocation_paired_actual_step_{root}_v36(target).state),"));
                                assert!(map.contains(&format!("invocation_source_constructor_clear_well_formed_v84(source, {}, {}, {}, {});", entry.locals.start, entry.locals.end, entry.owner, entry.pc)));
                                for (argument, local) in entry.arguments.iter().enumerate() {
                                    assert!(map.contains(&format!("invocation_source_put_local_well_formed_v78(entered, {local}, argument_{argument});")));
                                }
                            }
                        }
                        assert!(state_summaries > 0);
                        let related = out.text.split_once(&format!("spec fn invocation_paired_related_{root}_v36(")).unwrap().1.split_once("\n}").unwrap().0;
                        let residual = out.text.split_once(&format!("spec fn invocation_paired_residual_{root}_v85(")).unwrap().1.split_once("\n}").unwrap().0;
                        let storage = format!(" && invocation_source_byte_storage_related_{root}_v36(source, target)\n");
                        assert_eq!(related.matches(&storage).count(), 1);
                        assert_eq!(related.replace(&storage, ""), residual);
                    }
                    for forbidden in ["assume(", "admit(", "external_body"] {
                        assert!(!out.text.contains(forbidden));
                    }
                    Ok(())
                })
            },
        ).0.unwrap();
    }
}

#[test]
fn original_mir_step_constructor_refuses_truncated_entry_captures() {
    super::super::super::invocations::tests::run_root_variant(
        LIMIT,
        LIMIT,
        true,
        false,
        2,
        |plan, out| {
            with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let mut paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                let hints = paired.roots[0].step_hints.as_mut().unwrap();
                let call = hints.cuts.iter().find_map(|cut| cut.call.as_ref()).unwrap();
                let entry = hints.entries[call.child].as_mut().unwrap();
                assert_eq!(entry.arguments.len(), call.arguments.len());
                assert!(entry.arguments.pop().is_some());
                assert!(paired.emit(out).is_err());
                Ok(())
            })
        },
    )
    .0
    .unwrap();
}

fn run_target_follow_topology(
    work: usize,
    storage: usize,
    probe_boundaries: bool,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_root_variant(
        work,
        storage,
        false,
        true,
        3,
        |plan, out| {
            with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let mut paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                for root in 0..paired.roots.len() {
                    let count = paired.roots[root].cuts.len();
                    let fuel = generate::target_follow_fuel_for_test(&paired, root, out)?;
                    assert!((1..=count + 1).contains(&fuel));
                    if !probe_boundaries {
                        continue;
                    }
                    let inventory = paired.slots.correspondence(out)?.inventory(out.budget)?;
                    let successors: Vec<Vec<usize>> = inventory.blocks()
                        [paired.roots[root].blocks.clone()]
                    .iter()
                    .map(|block| {
                        inventory.edges()[block.edges.clone()]
                            .iter()
                            .map(|edge| {
                                assert_eq!(edge.target.function, block.coordinate.function);
                                edge.target.block as usize
                            })
                            .collect()
                    })
                    .collect();
                    // Exercise only the unfolding heuristic on altered boundary bitmaps.
                    // These synthetic locators are never emitted or admitted as a model.
                    for mask in 0..1usize << count.min(4) {
                        let row = &mut paired.roots[root];
                        for (block, cut) in row.cuts.iter_mut().enumerate() {
                            *cut = if block < 4 && mask & (1 << block) != 0 {
                                None
                            } else {
                                Some(Cut {
                                    source: 0,
                                    instance: row.instances.start,
                                    live: vec![],
                                    end: End::Ordinary,
                                })
                            };
                        }
                        let fuel = generate::target_follow_fuel_for_test(&paired, root, out)?;
                        if mask == 0 {
                            assert_eq!(fuel, 1);
                        }
                        let row = &paired.roots[root];
                        let mut active = vec![false; count];
                        for block in 0..count {
                            match independent_follow_depth(
                                block,
                                &row.cuts,
                                &successors,
                                &mut active,
                            ) {
                                Some(depth) => assert!(fuel > depth),
                                None => assert_eq!(fuel, count + 1),
                            }
                        }
                        if mask.count_ones() == 1 {
                            let block = mask.trailing_zeros() as usize;
                            if !successors[block].contains(&block) {
                                assert_eq!(fuel, 2);
                            }
                        }
                    }
                }
                Ok(())
            })
        },
    )
}

fn independent_follow_depth(
    block: usize,
    cuts: &[Option<Cut>],
    successors: &[Vec<usize>],
    active: &mut [bool],
) -> Option<usize> {
    if cuts[block].is_some() {
        return Some(0);
    }
    if active[block] {
        return None;
    }
    active[block] = true;
    let mut depth = 0;
    for next in &successors[block] {
        let Some(tail) = independent_follow_depth(*next, cuts, successors, active) else {
            active[block] = false;
            return None;
        };
        depth = depth.max(tail);
    }
    active[block] = false;
    Some(depth + 1)
}

#[test]
fn original_mir_step_follow_fuel_covers_authentic_graph_boundary_subsets() {
    run_target_follow_topology(LIMIT, LIMIT, true).0.unwrap();
}

#[test]
fn original_mir_step_follow_fuel_keeps_exact_and_one_short_accounts() {
    use super::super::super::invocations::tests::FLOOR;
    let measured = run_target_follow_topology(LIMIT, LIMIT, false);
    measured.0.unwrap();
    assert_eq!(measured.2, FLOOR);
    let exact = run_target_follow_topology(measured.1, measured.3, false);
    exact.0.unwrap();
    assert_eq!(exact.2, FLOOR);
    for (work, storage) in [(measured.1 - 1, measured.3), (measured.1, measured.3 - 1)] {
        let short = run_target_follow_topology(work, storage, false);
        assert!(short.0.is_err());
        assert_eq!(short.2, FLOOR);
    }
}

#[test]
fn original_mir_step_nonempty_blocks_and_moved_captures_keep_complete_fallback_obligations() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    for moved in [false, true] {
        super::super::super::invocations::tests::run_unit_source_transform_v85(
            LIMIT, LIMIT,
            |_, functions| {
                let statement = functions.last().unwrap().blocks()[0].statements()[0].clone();
                let original = &functions[0];
                let mut blocks = original.blocks().to_vec();
                // Transform an actual source call, before independent SSA/ranked derivation.
                let block = &blocks[1];
                let mut statements = block.statements().to_vec();
                let mut terminator = block.terminator().clone();
                if moved {
                    let SemanticTerminatorKindV1::Call(call) = terminator.kind() else { panic!("source call"); };
                    let mut arguments = call.arguments().to_vec();
                    let SemanticOperandV1::Copy(place) = &arguments[0] else { panic!("scalar copy"); };
                    arguments[0] = SemanticOperandV1::Move(place.clone());
                    terminator = SemanticTerminatorV1::new(terminator.source(), SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new_callable(call.callee(), arguments, call.destination().cloned(), call.unwind()).unwrap()
                    ));
                } else {
                    statements.push(statement);
                }
                blocks[1] = SemanticBasicBlockV1::new(block.identity(), block.source(), statements, terminator).unwrap();
                functions[0] = SemanticFunctionDeclV1::new(original.identity(), original.role(), original.item_definition_identity(), original.monomorphization_identity(), original.generic_type_arguments_identity(), original.const_generic_arguments_identity(), original.source(), original.abi().clone(), original.locals().to_vec(), original.entry(), blocks)
                    .unwrap()
                    .with_kernel_entry(original.kernel_entry().unwrap().clone());
            },
            |plan, out| with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                let paired = PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                program.emit(out)?;
                paired.emit(out)?;
                assert!(!out.text.contains("proof fn invocation_source_cut_summary_"));
                let mut found = 0;
                for (root, row) in paired.roots.iter().enumerate() {
                    check_four(&out.text, &format!("invocation_paired_step_{root}_v36"), root);
                    for hint in &row.step_hints.as_ref().unwrap().cuts {
                        if let Some(call) = &hint.call {
                            if if moved { call.arguments.iter().any(|arg| arg.1) } else { hint.statements != 0 } {
                                found += 1;
                                let pc = hint.pc;
                                check_four(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_all_v85"), root);
                                assert!(!out.text.contains(&format!("proof fn invocation_paired_cut_{root}_pc{pc}_map_v85(")));
                                let fallback = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_all_v85"));
                                assert!(fallback.contains("reveal_with_fuel(invocation_source_micro_run_"));
                                assert_eq!(fallback.matches("reveal_with_fuel(invocation_source_micro_run_").count(), 1);
                                assert!(fallback.contains(&format!("reveal_with_fuel(invocation_source_micro_run_{root}_{}_v36, {});", hint.instance, hint.statements + 1)));
                                assert!(!fallback.contains("constructor_clear_well_formed"));
                                assert!(!fallback.contains("hide(invocation_paired_control_values_"));
                                assert!(!fallback.contains("hide(invocation_source_enter_"));
                                assert!(!fallback.contains("hide(invocation_source_byte_storage_related_"));
                                assert!(!fallback.contains("invocation_related_target_inputs_v96("));
                                assert!(!out.text.contains(&format!("proof fn invocation_constructor_states_{root}_{pc}_v162(")));
                                let premises = fallback.split_once(" requires ").unwrap().1.split_once(" ensures ").unwrap().0;
                                assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},"));
                            }
                        }
                    }
                }
                assert_eq!(found, 1);
                Ok(())
            }),
        ).0.unwrap();
    }
}

#[test]
fn original_mir_step_constructor_laws_retain_generic_domains_and_no_axioms() {
    let laws = include_str!("original_semantic_mir_source_enter_laws_v85.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 3);
    assert_eq!(laws.matches("#[verifier::spinoff_prover]").count(), 3);
    for required in [
        "invocation_source_logical_clear_v38(source.logical, begin, end)",
        "source.machine.values.len()",
        "byte_enter_frame_v30(source.machine.frames, owner)",
        "invocation_source_byte_state_well_formed_v36(source)",
    ] {
        assert!(laws.contains(required), "{required}");
    }
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "Map::empty()",
        "pc == 0",
    ] {
        assert!(!laws.contains(forbidden), "{forbidden}");
    }
}

#[test]
fn original_mir_step_copied_hints_cannot_emit_on_foreign_or_refunded_accounts() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let result = super::super::super::invocations::tests::run_variant(
            LIMIT,
            LIMIT,
            true,
            |plan, out| {
                with_slots(plan, out, |slots, out| {
                    let program = SourceByteProgram::derive(plan, slots, out)?;
                    let paired =
                        PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                    assert!(paired.roots.iter().all(|root| root.step_hints.is_some()));
                    if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(out.budget.storage())?;
                        let before = (budget.work(), budget.storage(), budget.peak_storage());
                        {
                            let mut writer = Writer::new(&mut budget)?;
                            assert!(matches!(
                                paired.emit(&mut writer),
                                Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                            ));
                            assert!(writer.text.is_empty());
                        }
                        assert_eq!(
                            (budget.work(), budget.storage(), budget.peak_storage()),
                            before
                        );
                    } else {
                        assert_eq!(out.budget.storage(), paired.required);
                        out.budget.release_storage(1)?;
                    }
                    let before = (
                        out.budget.work(),
                        out.budget.storage(),
                        out.budget.peak_storage(),
                        out.text.len(),
                    );
                    for _ in 0..2 {
                        assert!(matches!(
                            paired.emit(out),
                            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                        ));
                        assert_eq!(
                            (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                                out.text.len()
                            ),
                            before
                        );
                    }
                    paired.emit(out)
                })
            },
        );
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn original_mir_step_hint_donor_root_and_call_coordinate_substitutions_refuse() {
    for donor in [false, true] {
        super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
            with_slots(plan, out, |slots, out| {
                let program = SourceByteProgram::derive(plan, slots, out)?;
                let mut paired =
                    PairedInvocations::derive(plan, &program, FormalIndexWidth::Bits64, out)?;
                if donor {
                    let (first, rest) = paired.roots.split_at_mut(1);
                    std::mem::swap(&mut first[0].step_hints, &mut rest[0].step_hints);
                } else {
                    let hints = paired.roots[0].step_hints.as_mut().unwrap();
                    let call = hints
                        .cuts
                        .iter_mut()
                        .find_map(|hint| hint.call.as_mut())
                        .unwrap();
                    call.child = 0;
                }
                assert!(matches!(paired.emit(out), Err(Error::Statement(_))));
                assert!(!out.text.contains("proof fn invocation_paired_step_0_v36("));
                Ok(())
            })
        })
        .0
        .unwrap();
    }
}

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
                    assert!(!out.text.contains("proof fn invocation_source_cut_summary_"));
                    assert_eq!(out.text.matches("proof fn invocation_related_target_inputs_v96").count(), 1);
                    for (root, row) in paired.roots.iter().enumerate() {
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
                                assert_eq!(map.matches("reveal_with_fuel(invocation_source_micro_run_").count(), 1);
                                assert!(map.contains(&format!("reveal_with_fuel(invocation_source_micro_run_{root}_{}_v36, {});", hint.instance, hint.statements + 1)));
                                let control = theorem(&out.text, &format!("invocation_paired_cut_{root}_pc{pc}_control_v85"));
                                for predicate in ["invocation_paired_source_step", "invocation_paired_source_defined", "invocation_source_byte_storage_related", "invocation_source_byte_map", "invocation_paired_control_values"] {
                                    assert!(control.contains(&format!(" hide({predicate}_{root}_v36);")));
                                }
                                assert!(!control.contains("hide(invocation_paired_actual_step_"));
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
                                    assert!(observations.contains(&format!("assert(invocation_paired_{side}_step_{root}_v36({argument}).events == Seq::empty()) by {{\n reveal(invocation_paired_{side}_step_{root}_v36);\n }}")));
                                }
                                let premises = observations.split_once(" requires ").unwrap().1.split_once(" ensures ").unwrap().0;
                                assert_eq!(premises.trim(), format!("invocation_paired_related_{root}_v36(source, target), invocation_paired_source_defined_{root}_v36(source, 1),\n source.machine.pc == {pc},"));
                                assert!(map.contains(&format!("invocation_source_constructor_clear_well_formed_v84(source, {}, {}, {}, {});", entry.locals.start, entry.locals.end, entry.owner, entry.pc)));
                                for (argument, local) in entry.arguments.iter().enumerate() {
                                    assert!(map.contains(&format!("invocation_source_put_local_well_formed_v78(entered, {local}, argument_{argument});")));
                                }
                            }
                        }
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

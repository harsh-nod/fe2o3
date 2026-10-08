//! Actual proof emission/accounting regressions, not executed Verus results.
use super::*;

const LIMIT: usize = 512 * 1024 * 1024;

fn declaration<'a>(text: &'a str, name: &str) -> &'a str {
    let marker = format!("proof fn {name}(");
    assert_eq!(text.matches(marker.as_str()).count(), 1);
    let rest = &text[text.find(marker.as_str()).unwrap()..];
    let end = ["\nproof fn ", "\nspec fn "]
        .into_iter()
        .filter_map(|next| rest.find(next))
        .min()
        .unwrap_or(rest.len());
    rest[..end].trim_end()
}

fn check_history(text: &str, root: usize, instance: usize) {
    let step = declaration(
        text,
        &format!("invocation_source_micro_step_history_{root}_{instance}_v293"),
    );
    let run = declaration(
        text,
        &format!("invocation_source_micro_run_history_{root}_{instance}_v293"),
    );
    let (step_header, step_body) = step.split_once("\n{\n").unwrap();
    let (run_header, run_body) = run.split_once("\n{\n").unwrap();
    assert_eq!(
        step_header,
        format!(
            r#"proof fn invocation_source_micro_step_history_{root}_{instance}_v293(c: InvocationSourceMicroStateV36, e: bool)
    ensures {{
        let n = invocation_source_micro_step_{root}_{instance}_v36(c, e);
        let l = c.observations.len() as int;
        n == invocation_source_micro_refused_v36(c)
        || (n.next_statement == c.next_statement + 1
            && n.observations.len() == l + 1
            && n.observations.take(l) == c.observations
            && n.observations[l].root == {root}
            && n.observations[l].instance == {instance}
            && n.observations[l].statement == c.next_statement
            && n.observations[l].before == c.source
            && n.observations[l].after == n.source)
    }},"#
        )
    );
    assert_eq!(
        run_header,
        format!(
            r#"proof fn invocation_source_micro_run_history_{root}_{instance}_v293(c: InvocationSourceMicroStateV36, f: nat, e: bool)
    ensures {{
        let out = invocation_source_micro_run_{root}_{instance}_v36(c, f, e);
        let l = c.observations.len() as int;
        let d = out.observations.len() as int - l;
        0 <= d <= f as int
        && out.observations.take(l) == c.observations
        && out.next_statement == c.next_statement + d
        && (forall|j: int| #![trigger out.observations[l + j]] 0 <= j < d ==> {{
            let row = out.observations[l + j];
            row.root == {root} && row.instance == {instance}
            && row.statement == c.next_statement + j
            && row.before == invocation_source_micro_run_{root}_{instance}_v36(c, j as nat, e).source
            && row.after == invocation_source_micro_run_{root}_{instance}_v36(c, (j + 1) as nat, e).source
        }})
        && (out.source.machine.valid ==> d == f as int)
    }},
    decreases f,"#
        )
    );
    for proof in [step, run] {
        for forbidden in [
            "requires",
            "assume(",
            "admit(",
            "external_body",
            "target",
            "well_formed",
            ".block ==",
        ] {
            assert!(!proof.contains(forbidden), "{forbidden}");
        }
    }
    assert!(step.contains(&format!(
        "reveal(invocation_source_micro_step_{root}_{instance}_v36);"
    )));
    let mut hidden = vec![
        format!("invocation_source_active_{root}_{instance}_v36"),
        format!("invocation_source_byte_event_{root}_{instance}_v36"),
        "invocation_source_byte_step_v36".to_owned(),
        "invocation_source_byte_refused_v36".to_owned(),
        "invocation_source_micro_record_v36".to_owned(),
        "invocation_source_micro_refused_v36".to_owned(),
    ];
    let dispatcher = text
        .split_once(&format!(
            "spec fn invocation_source_micro_step_{root}_{instance}_v36("
        ))
        .unwrap()
        .1
        .split_once("\nspec fn ")
        .unwrap()
        .0;
    let scalar_prefix = format!("spec fn invocation_source_scalar_{root}_{instance}_");
    for line in text.lines().filter(|line| line.starts_with(&scalar_prefix)) {
        let name = line
            .strip_prefix("spec fn ")
            .unwrap()
            .split_once('(')
            .unwrap()
            .0;
        if dispatcher.contains(&format!("{name}(cursor.source)")) {
            hidden.push(name.to_owned());
        }
    }
    let actual_hidden: Vec<_> = step_body
        .lines()
        .filter_map(|line| line.trim().strip_prefix("hide(")?.strip_suffix(");"))
        .collect();
    assert_eq!(actual_hidden, hidden);
    for name in &hidden {
        assert!(!step_body.contains(&format!("reveal({name}")));
        assert!(!step_body.contains(&format!("reveal_with_fuel({name}")));
    }
    let mut expected_body = format!(
        r#"    reveal(invocation_source_micro_step_{root}_{instance}_v36);
    let n = invocation_source_micro_step_{root}_{instance}_v36(c, e);
    if !invocation_source_active_{root}_{instance}_v36(c.source) || c.next_statement < 0 || c.next_statement != c.observations.len() {{
        assert(n == invocation_source_micro_refused_v36(c));
    }} else {{
"#
    );
    // Each proof case must replay the actual dispatcher, including its value producer.
    let lines: Vec<_> = dispatcher.lines().collect();
    let mut cases = 0;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with(" if cursor.source.machine.pc == ") {
            let adapt = |text: &str| text.replace("cursor.", "c.").replace("little_endian", "e");
            expected_body.push_str(&format!("        {}\n", adapt(line.trim())));
            assert!(lines[index + 1].starts_with(" let after = "));
            expected_body.push_str(&format!("            {}\n", adapt(lines[index + 1].trim())));
            let record = lines[index + 2].trim();
            assert!(record.starts_with("invocation_source_micro_record_v36(cursor, after, "));
            let record = record.replace("(cursor,", "(c,");
            expected_body.push_str(&format!(
                "            {};\n            assert(n == {record});\n        }} else",
                record.replacen("micro_record_v36", "micro_record_history_v348", 1)
            ));
            cases += 1;
        }
    }
    expected_body.push_str(
        " {\n            assert(n == invocation_source_micro_refused_v36(c));\n        }\n    }\n}",
    );
    assert_eq!(
        step.matches("invocation_source_micro_record_history_v348(")
            .count(),
        cases
    );
    let without_hides: Vec<_> = step_body
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with("hide("))
        .collect();
    assert_eq!(without_hides, expected_body.lines().collect::<Vec<_>>());
    let record_summary = declaration(
        SOURCE_FUNCTION_V36,
        "invocation_source_micro_record_history_v348",
    );
    assert!(!record_summary.contains("requires"));
    for fact in [
        "n.next_statement == cursor.next_statement + 1",
        "n.observations.len() == l + 1",
        "n.observations.take(l) == cursor.observations",
        "n.observations[l].root == root",
        "n.observations[l].instance == instance",
        "n.observations[l].statement == statement",
        "n.observations[l].before == cursor.source",
        "n.observations[l].after == n.source",
    ] {
        assert_eq!(record_summary.matches(fact).count(), 1, "{fact}");
    }
    assert!(record_summary.contains("reveal(invocation_source_micro_record_v36);"));
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "well_formed",
        "machine.valid",
        "byte_step",
    ] {
        assert!(!record_summary.contains(forbidden), "{forbidden}");
    }
    assert!(!run.contains("machine.pc"));
    let opaque_step = format!("hide(invocation_source_micro_step_{root}_{instance}_v36);");
    assert!(run_body.starts_with(&format!(
        "    {opaque_step}\n    reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, 2);"
    )));
    assert_eq!(run_body.matches(opaque_step.as_str()).count(), 1);
    for reveal in ["reveal", "reveal_with_fuel"] {
        assert!(!run_body.contains(&format!(
            "{reveal}(invocation_source_micro_step_{root}_{instance}_v36"
        )));
    }
    assert!(!step.contains(opaque_step.as_str()));
    assert!(
        !declaration(
            text,
            &format!("invocation_source_micro_run_composes_{root}_{instance}_v292"),
        )
        .contains(opaque_step.as_str())
    );
    assert!(run.contains("if f == 0 || !c.source.machine.valid {"));
    assert!(run.contains("if n == invocation_source_micro_refused_v36(c) {"));
    assert!(run.contains("assert(!n.source.machine.valid);"));
    assert!(run.contains(&format!(
        "invocation_source_micro_run_history_{root}_{instance}_v293(n, remaining, e);"
    )));
    for last in ["k", "k + 1"] {
        assert!(run.contains(&format!(
            "invocation_source_micro_run_composes_{root}_{instance}_v292(c, 1, {last}, e);"
        )));
    }
    let old_run = format!(
        "spec fn invocation_source_micro_run_{root}_{instance}_v36(cursor: InvocationSourceMicroStateV36, fuel: nat, little_endian: bool) -> InvocationSourceMicroStateV36\n decreases fuel\n{{ if fuel == 0 || !cursor.source.machine.valid {{ cursor }} else {{ invocation_source_micro_run_{root}_{instance}_v36(invocation_source_micro_step_{root}_{instance}_v36(cursor, little_endian), (fuel - 1) as nat, little_endian) }} }}\n"
    );
    assert_eq!(text.matches(old_run.as_str()).count(), 1);
    assert_eq!(
        text.matches(&format!(
            "proof fn invocation_source_micro_run_composes_{root}_{instance}_v292("
        ))
        .count(),
        1
    );
}

#[test]
fn source_micro_history_emits_unconditional_contracts_for_every_actual_instance() {
    for unit_return in [false, true] {
        super::super::super::invocations::tests::run_variant(
            LIMIT,
            LIMIT,
            unit_return,
            |plan, out| {
                super::tests::with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    let coordinates: Vec<_> = program
                        .functions
                        .iter()
                        .flatten()
                        .map(|row| (row.root, row.instance))
                        .collect();
                    assert_eq!(coordinates.len(), 6);
                    assert!(program.functions.iter().flatten().any(|row| {
                        row.control.iter().any(|block| block.statements > 0)
                    }));
                    assert!(program.functions.iter().flatten().any(|row| {
                        row.control.iter().any(|block| block.statements == 0)
                    }));
                    assert!(program.functions.iter().flatten().any(|row| row.blocks.start > 0));
                    program.emit(out)?;
                    for (root, instance) in coordinates {
                        check_history(&out.text, root, instance);
                    }
                    for prefix in [
                        "proof fn invocation_source_micro_step_history_",
                        "proof fn invocation_source_micro_run_history_",
                        "spec fn invocation_source_micro_run_",
                    ] {
                        assert_eq!(out.text.matches(prefix).count(), 6);
                    }
                    // Recorded block is relative; the dispatch PC uses its owner range.
                    for row in program.functions.iter().flatten() {
                        for (block, body) in row.control.iter().enumerate() {
                            if matches!(body.end, End::Unreachable) {
                                continue;
                            }
                            for statement in 0..body.statements {
                                assert!(out.text.contains(&format!(
                                    "if cursor.source.machine.pc == {} && cursor.next_statement == {statement}",
                                    row.blocks.start + block
                                )));
                                assert!(out.text.contains(&format!(
                                    "invocation_source_micro_record_v36(cursor, after, {}, {}, {block}, {statement}, invocation_source_byte_event_{}_{}_v36({block}, {statement}))",
                                    row.root, row.instance, row.root, row.instance
                                )));
                            }
                        }
                    }
                    // Structural checks, not executed failure/progress proofs.
                    assert!(SOURCE_FUNCTION_V36.contains(
                        "source: after, next_statement: cursor.next_statement + 1"
                    ));
                    assert!(SOURCE_FUNCTION_V36.contains(
                        "root, instance, block, statement, event, before: cursor.source, after"
                    ));
                    assert!(SOURCE_FUNCTION_V36.contains(
                        "next_statement: cursor.next_statement, observations: cursor.observations"
                    ));
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn source_micro_history_survives_expanded_finish_without_another_interpreter() {
    use super::super::expanded_generation::ExpandedGenerationV221;
    use fe2o3_kernel_ir::{EndiannessV2, ExecutionTileLayoutV1, FormalIndexWidth};

    for layout in [
        ExecutionTileLayoutV1::Blocked,
        ExecutionTileLayoutV1::Striped,
    ] {
        super::tile_fixture_tests::run_fixture_with_plan(
            layout,
            LIMIT,
            LIMIT,
            |plan, slots, _, out| {
                let model = ExpandedGenerationV221::derive(
                    plan,
                    slots,
                    FormalIndexWidth::Bits64,
                    EndiannessV2::Little,
                    out,
                )?;
                model.emit_support(out)?;
                let source = plan.source(out)?;
                let mut proofs = Vec::new();
                for root in 0..source.root_count(out.budget)? {
                    for instance in 0..plan.root(root, out)?.instances.len() {
                        if plan.instance(root, instance, out)?.active {
                            check_history(&out.text, root, instance);
                            for kind in ["step", "run"] {
                                let name = format!(
                                    "invocation_source_micro_{kind}_history_{root}_{instance}_v293"
                                );
                                proofs
                                    .push((name.clone(), declaration(&out.text, &name).to_owned()));
                            }
                        }
                    }
                }
                assert!(!proofs.is_empty());
                model.finish(out)?;
                for (name, body) in &proofs {
                    assert_eq!(declaration(&out.text, name), body);
                }
                assert_eq!(
                    out.text
                        .matches("proof fn invocation_source_micro_step_history_")
                        .count()
                        + out
                            .text
                            .matches("proof fn invocation_source_micro_run_history_")
                            .count(),
                    proofs.len()
                );
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn source_micro_history_has_exact_and_one_short_emission_resource_bounds() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

    for unit_return in [false, true] {
        let run = |work, storage| {
            super::super::super::invocations::tests::run_variant(
                work,
                storage,
                unit_return,
                |plan, out| {
                    super::tests::with_slots(plan, out, |slots, out| {
                        SourceByteProgram::derive(plan, slots, out)?.emit(out)?;
                        super::super::support_closure::retain_referenced(out)?;
                        for kind in ["step", "run"] {
                            assert_eq!(
                                out.text
                                    .matches(&format!(
                                        "proof fn invocation_source_micro_{kind}_history_"
                                    ))
                                    .count(),
                                6
                            );
                        }
                        Ok(())
                    })
                },
            )
        };
        let measured = run(LIMIT, LIMIT);
        measured.0.unwrap();
        let exact = run(measured.1, measured.3);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
        assert!(matches!(run(measured.1 - 1, measured.3).0,
            Err(Error::Resource(Resource::Work(error)))
                | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1));
        assert!(matches!(run(measured.1, measured.3 - 1).0,
            Err(Error::Resource(Resource::Storage(error)))
                | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1));
    }
}

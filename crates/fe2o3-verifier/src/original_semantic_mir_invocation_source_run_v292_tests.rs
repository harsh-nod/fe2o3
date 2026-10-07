//! Emission and accounting regressions, not executed Verus proof results.
use super::*;

const LIMIT: usize = 512 * 1024 * 1024;

fn expected_run(root: usize, instance: usize) -> String {
    format!(
        "spec fn invocation_source_micro_run_{root}_{instance}_v36(cursor: InvocationSourceMicroStateV36, fuel: nat, little_endian: bool) -> InvocationSourceMicroStateV36\n decreases fuel\n{{ if fuel == 0 || !cursor.source.machine.valid {{ cursor }} else {{ invocation_source_micro_run_{root}_{instance}_v36(invocation_source_micro_step_{root}_{instance}_v36(cursor, little_endian), (fuel - 1) as nat, little_endian) }} }}\n"
    )
}

fn expected_composition(root: usize, instance: usize) -> String {
    format!(
        "proof fn invocation_source_micro_run_composes_{root}_{instance}_v292(cursor: InvocationSourceMicroStateV36, first: nat, second: nat, little_endian: bool)\n ensures invocation_source_micro_run_{root}_{instance}_v36(cursor, first + second, little_endian) == invocation_source_micro_run_{root}_{instance}_v36(invocation_source_micro_run_{root}_{instance}_v36(cursor, first, little_endian), second, little_endian),\n decreases first,\n{{\n if first > 0 && cursor.source.machine.valid {{\n invocation_source_micro_run_composes_{root}_{instance}_v292(invocation_source_micro_step_{root}_{instance}_v36(cursor, little_endian), (first - 1) as nat, second, little_endian);\n }}\n}}\n"
    )
}

fn check_composition(text: &str, root: usize, instance: usize) {
    let run = expected_run(root, instance);
    let proof = expected_composition(root, instance);
    assert_eq!(text.matches(run.as_str()).count(), 1);
    assert_eq!(text.matches(proof.as_str()).count(), 1);
    let header = proof.split_once("\n{\n").unwrap().0;
    assert!(!header.contains("requires"));
    assert!(!header.contains("machine.valid"));
    assert!(!header.contains("well_formed"));
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "invocation_source_micro_refused",
        "invocation_source_statement_count",
        "target",
    ] {
        assert!(!proof.contains(forbidden));
    }
}

#[test]
fn source_micro_run_composition_covers_every_actual_instance_and_nonempty_body() {
    for unit_return in [false, true] {
        super::super::super::invocations::tests::run_variant(
            LIMIT,
            LIMIT,
            unit_return,
            |plan, out| {
                super::tests::with_slots(plan, out, |slots, out| {
                    let mut program = SourceByteProgram::derive(plan, slots, out)?;
                    assert_eq!(program.roots.len(), 2);
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
                    let start = out.text.len();
                    program.emit(out)?;
                    let generated = &out.text[start..];
                    assert_eq!(
                        generated
                            .matches("proof fn invocation_source_micro_run_composes_")
                            .count(),
                        coordinates.len()
                    );
                    for (root, instance) in coordinates {
                        check_composition(generated, root, instance);
                        if instance != 0 {
                            let dispatch = if unit_return {
                                format!("invocation_source_byte_event_{root}_{instance}_v36(0, 0)")
                            } else {
                                format!("invocation_source_scalar_{root}_{instance}_0_0_v36(cursor.source)")
                            };
                            assert!(generated.contains(&dispatch));
                        }
                    }
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn source_micro_run_composition_survives_expanded_model_finish() {
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
                let mut coordinates = Vec::new();
                for root in 0..source.root_count(out.budget)? {
                    for instance in 0..plan.root(root, out)?.instances.len() {
                        if plan.instance(root, instance, out)?.active {
                            coordinates.push((root, instance));
                        }
                    }
                }
                assert!(!coordinates.is_empty());
                let before: Vec<_> = coordinates
                    .iter()
                    .map(|&(root, instance)| {
                        check_composition(&out.text, root, instance);
                        expected_composition(root, instance)
                    })
                    .collect();
                model.finish(out)?;
                assert_eq!(
                    out.text
                        .matches("proof fn invocation_source_micro_run_composes_")
                        .count(),
                    coordinates.len()
                );
                for ((root, instance), proof) in coordinates.into_iter().zip(before) {
                    check_composition(&out.text, root, instance);
                    assert_eq!(out.text.matches(proof.as_str()).count(), 1);
                }
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn source_micro_run_composition_has_exact_and_one_short_resource_bounds() {
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
                        assert_eq!(
                            out.text
                                .matches("proof fn invocation_source_micro_run_composes_")
                                .count(),
                            6
                        );
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

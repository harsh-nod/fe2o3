//! Diagnostic complete-model boundary obligations; no executed proof result.
use super::*;
use std::fmt::Write as _;

pub(super) const TEMPLATE: &str = include_str!("semantic-boundaries.vrs.in");
pub(super) const PROOF_PREFIXES: [&str; 5] = [
    "history_zero_fuel",
    "history_constructed_invalid",
    "history_arbitrary_old_prefix",
    "history_negative_index",
    "history_mismatched_index",
];

pub(super) fn proof_names(root: usize, instance: usize) -> [String; 5] {
    PROOF_PREFIXES.map(|prefix| format!("{prefix}_{root}_{instance}_v293"))
}

pub(super) fn render(root: usize, instance: usize) -> String {
    let text = TEMPLATE.replace("_ROOT_INSTANCE_", &format!("_{root}_{instance}_"));
    assert!(!text.contains("_ROOT_INSTANCE_"));
    assert_eq!(text.matches("proof fn ").count(), 5);
    for name in proof_names(root, instance) {
        assert_eq!(text.matches(&format!("proof fn {name}(")).count(), 1);
    }
    for prohibited in ["requires", "assume(", "admit(", "external_body", "axiom"] {
        assert!(!text.contains(prohibited), "{prohibited}");
    }
    text
}

// Test-only roster/String scaffolding is not a new production metered helper.
// Owner queries and appended model bytes use the existing account and Writer.
pub(super) fn emit(
    plan: &InvocationPlan<'_, '_>,
    program: &SourceByteProgram<'_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<(usize, usize)>> {
    let source = plan.source(out)?;
    let slots = program.source_slots(out)?;
    assert!(std::ptr::eq(
        source,
        slots.correspondence(out)?.source(out.budget)?
    ));
    let roots = source.root_count(out.budget)?;
    assert_eq!(program.roots.len(), roots);
    let mut coordinates = Vec::new();
    for root in 0..roots {
        let instances = plan.root(root, out)?.instances.clone();
        assert_eq!(program.roots[root].0, instances);
        assert!(instances.end <= program.functions.len());
        for instance in 0..instances.len() {
            let row = plan.instance(root, instance, out)?;
            let function = &program.functions[instances.start + instance];
            assert_eq!(function.is_some(), row.active);
            let Some(function) = function else {
                continue;
            };
            assert_eq!((function.root, function.instance), (root, instance));
            assert_eq!(function.blocks, row.blocks);
            for declaration in [
                format!("spec fn invocation_source_micro_step_{root}_{instance}_v36("),
                format!("spec fn invocation_source_micro_run_{root}_{instance}_v36("),
                format!("proof fn invocation_source_micro_run_history_{root}_{instance}_v293("),
            ] {
                assert_eq!(out.text.matches(&declaration).count(), 1);
            }
            coordinates.push((root, instance));
            assert!(coordinates.len() <= 256);
            write!(out, "{}", render(root, instance)).map_err(|_| out.error())?;
        }
    }
    assert!(!coordinates.is_empty());
    assert_eq!(
        coordinates.len(),
        program.functions.iter().flatten().count()
    );
    assert!(coordinates.windows(2).all(|pair| pair[0] < pair[1]));
    Ok(coordinates)
}

#[test]
fn source_micro_history_boundaries_cover_every_authenticated_complete_instance() {
    run_complete(Mode::Witnesses, |witnesses, coordinates, model, census| {
        assert_eq!(coordinates.len(), 6);
        assert!(coordinates.iter().any(|&(_, instance)| instance > 0));
        for root in 0..census[0] {
            assert!(coordinates.contains(&(root, 0)));
        }
        assert!(
            witnesses
                .iter()
                .all(|row| coordinates.contains(&(row.root, row.instance)))
        );
        assert_eq!(model.matches("verus! {").count(), 1);
        assert!(model.ends_with("}\n"));
        for prefix in PROOF_PREFIXES {
            assert_eq!(
                model.matches(&format!("proof fn {prefix}_")).count(),
                coordinates.len()
            );
        }
        for &(root, instance) in coordinates {
            assert!(model.contains(&render(root, instance)));
            for name in proof_names(root, instance) {
                assert_eq!(model.matches(&format!("proof fn {name}(")).count(), 1);
            }
        }
        assert_eq!(
            model.matches("proof fn history_witness_").count(),
            witnesses.len() * 4
        );
        let mut output = Vec::new();
        write_record(&mut output, witnesses, coordinates, model, census);
        let record = String::from_utf8(output).unwrap();
        assert!(
            record.starts_with(
                "{\"kind\":\"fe2o3-source-micro-history-witness-boundary-model-v293\""
            )
        );
        assert!(record.contains("\"boundary_template_sha256\":\""));
        assert!(record.contains("\"boundary_instances\":["));
        assert_eq!(
            record.matches("\"proofs\":[").count(),
            coordinates.len() + witnesses.len()
        );
        assert!(
            record
                .contains("\"authority\":false,\"frontend_checked\":false,\"proof_success\":false")
        );
    })
    .0
    .unwrap();
}

#[test]
fn source_micro_history_boundaries_preserve_all_five_unconditional_cases() {
    run_complete(Mode::Witnesses, |_, coordinates, model, _| {
        for &(root, instance) in coordinates {
            let text = render(root, instance);
            assert!(model.contains(&text));
            assert!(text.contains(&format!(
                "invocation_source_micro_run_{root}_{instance}_v36(c, 0, e) == c"
            )));
            assert!(text.contains("source: invocation_source_byte_refused_v36(c.source)"));
            assert!(text.contains(&format!(
                "invocation_source_micro_run_{root}_{instance}_v36(invalid, f, e) == invalid"
            )));
            assert!(text.contains("out.observations.take(old.len() as int) == old"));
            assert!(text.contains("next_statement: -1"));
            assert!(text.contains("next_statement: old.len() as int + 1"));
            assert_eq!(text.matches("n.observations == old").count(), 2);
            assert_eq!(
                text.matches("n == invocation_source_micro_refused_v36(c)")
                    .count(),
                2
            );
            assert!(text.contains(&format!(
                "invocation_source_micro_run_history_{root}_{instance}_v293(c, f, e);"
            )));
            assert!(!text.contains("_ROOT_INSTANCE_"));
        }
    })
    .0
    .unwrap();
}

#[test]
fn source_micro_history_boundaries_have_exact_and_one_short_complete_emission_bounds() {
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;

    let run = |work, storage| {
        run_complete_bounded(
            work,
            storage,
            Mode::Witnesses,
            |_, coordinates, model, _| {
                assert_eq!(coordinates.len(), 6);
                for prefix in PROOF_PREFIXES {
                    assert_eq!(model.matches(&format!("proof fn {prefix}_")).count(), 6);
                }
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

const WARNING_REFERENCE_V65: &str =
    include_str!("mixed_optimizer_verus_warning_predicates_v65_tests.txt");

fn warning_definition_v65<'a>(source: &'a str, marker: &str) -> &'a str {
    let start = source.find(marker).expect("exact definition marker");
    let definition = &source[start..];
    let end = definition.find("\n}\n").expect("complete definition") + 2;
    &definition[..end]
}

fn warning_tokens_v65(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[test]
fn generated_warning_cleanup_removes_only_nine_finite_tautologies() {
    let cases: [(&str, &str, &[&str]); 4] = [
        (
            include_str!("original_semantic_mir_source_enum_values_v47.vrs"),
            "spec fn invocation_source_enum_well_formed_v47(",
            &["value.fields.dom().finite() && "],
        ),
        (
            include_str!("original_semantic_mir_source_aggregate_values_v42.vrs"),
            "spec fn invocation_source_aggregate_well_formed_v42(",
            &[" && value.leaves.dom().finite()"],
        ),
        (
            include_str!("original_semantic_mir_source_logical_locals_v38.vrs"),
            "spec fn invocation_source_logical_well_formed_v38(",
            &[
                " && logical.witnesses.dom().finite()",
                " && logical.references.dom().finite()",
                " && logical.descriptor_references.dom().finite()",
                " && logical.aggregates.dom().finite()",
                " && logical.enums.dom().finite()",
            ],
        ),
        (
            include_str!("original_semantic_mir_invocation_source_bytes_v36.rs"),
            "spec fn invocation_source_byte_state_well_formed_v36(",
            &[
                " && source.slots.dom().finite()",
                " && source.objects.dom().finite()",
            ],
        ),
    ];
    let mut removed = 0;
    for (source, marker, tautologies) in cases {
        let mut expected =
            warning_tokens_v65(warning_definition_v65(WARNING_REFERENCE_V65, marker));
        for tautology in tautologies {
            assert_eq!(expected.matches(tautology).count(), 1);
            expected = expected.replacen(tautology, "", 1);
            removed += 1;
        }
        if marker == "spec fn invocation_source_byte_state_well_formed_v36(" {
            let old = "invocation_source_logical_well_formed_v38(source.logical, source.machine.values.len())";
            assert_eq!(expected.matches(old).count(), 1);
            expected = expected.replacen(
                old,
                "invocation_source_logical_well_formed_v38(source.logical, source.machine.values.len() as int)",
                1,
            );
        }
        assert_eq!(
            warning_tokens_v65(warning_definition_v65(source, marker)),
            expected
        );
        assert!(!source.contains(".finite()"));
        assert!(!source.contains("allow(deprecated)"));
    }
    assert_eq!(removed, 9);
}

#[test]
fn generated_event_warning_allowance_preserves_every_variant_and_is_item_scoped() {
    let source = include_str!("original_semantic_mir_invocation_source_bytes_v36.rs");
    let marker = "enum InvocationSourceByteEventV36 {";
    let prior = warning_definition_v65(WARNING_REFERENCE_V65, marker);
    let descriptor = "    Descriptor(InvocationSourceDescriptorEventV51),\n";
    assert_eq!(prior.matches(descriptor).count(), 1);
    let expected = prior.replacen(
        descriptor,
        concat!(
            "    Descriptor(InvocationSourceDescriptorEventV51),\n",
            "    ThreadWrite(InvocationSourceThreadWriteV88),\n",
            "    ContextIssue(InvocationSourceContextIssueV161),\n",
            "    TileLoad(InvocationSourceTileLoadV161),\n",
            "    TileTransport(InvocationSourceTileTransportV161),\n",
        ),
        1,
    );
    assert_eq!(warning_definition_v65(source, marker), expected);
    assert_eq!(source.matches("#[allow(inconsistent_fields)]").count(), 1);
    assert!(source.contains("#[allow(inconsistent_fields)]\nenum InvocationSourceByteEventV36 {"));
    for forbidden in ["#![allow(", "allow(warnings)", "allow(deprecated)"] {
        assert!(!source.contains(forbidden));
    }
}

fn emit_warning_equations_v65(out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        "{}",
        include_str!("mixed_optimizer_verus_warning_equations_v65_tests.vrs")
    );
    Ok(())
}

#[test]
fn generated_warning_equations_keep_all_domains_and_reference_only_deprecations() {
    let source = run(37, LIMIT, LIMIT, emit_warning_equations_v65).0.unwrap();
    assert_eq!(source.matches("proof fn ").count(), 4);
    assert_eq!(source.matches(".dom().finite()").count(), 9);
    assert_eq!(source.matches("#[allow(deprecated)]\nproof fn ").count(), 4);
    for forbidden in [
        "assume(",
        "external_body",
        "ISet<",
        "allow(warnings)",
        "mod ",
    ] {
        assert!(!source.contains(forbidden));
    }
    for proof in [
        "enum_fields_finite_conjunct_v65",
        "aggregate_leaves_finite_conjunct_v65",
        "logical_locals_finite_conjuncts_v65",
        "source_byte_state_finite_conjuncts_v65",
    ] {
        assert_eq!(source.matches(&format!("proof fn {proof}<")).count(), 1);
    }
}

#[test]
fn generated_warning_equations_have_exact_and_one_short_resources() {
    let (source, work, peak) = run(37, LIMIT, LIMIT, emit_warning_equations_v65);
    let source = source.unwrap();
    let (exact, exact_work, exact_peak) = run(37, work, peak, emit_warning_equations_v65);
    assert_eq!(exact.unwrap(), source);
    assert_eq!((exact_work, exact_peak), (work, peak));
    assert!(
        matches!(run(37, work - 1, peak, emit_warning_equations_v65).0,
        Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1)
    );
    assert!(
        matches!(run(37, work, peak - 1, emit_warning_equations_v65).0,
        Err(Error::Resource(Resource::Storage(error))) if error.limit() == peak - 1)
    );
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_generated_warning_cleanup_preserves_finite_domain_equations() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};

    let source = run(37, LIMIT, LIMIT, emit_warning_equations_v65).0.unwrap();
    let source = CanonicalGeneratedVerusProofInputV3::new(source.into_bytes())
        .expect("canonical finite-domain warning equivalence program");
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("requires the actual public pinned-runtime lease");
    runtime
        .revalidate()
        .expect("revalidate before warning proof");
    let mut attempt = runtime
        .begin_attempt()
        .expect("acquire warning proof attempt");
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &source,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .expect("execute exact finite-domain conjunction equivalences");
    runtime
        .revalidate()
        .expect("revalidate after warning proof");
    attempt.complete().expect("complete warning proof attempt");
    crate::functional_refinement_receipt_v2::validate_proved_output(&output)
        .expect("all removed finite-domain conjuncts must genuinely verify");
}

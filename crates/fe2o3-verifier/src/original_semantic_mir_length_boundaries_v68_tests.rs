use super::*;
use std::fmt::Write as _;

const LENGTH_EQUATIONS_V68: &str =
    include_str!("original_semantic_mir_length_boundaries_v68_tests.vrs");
const DESCRIPTOR_EFFECT_EQUATIONS_V68: &str =
    include_str!("original_semantic_mir_descriptor_effects_v68_tests.vrs");

fn run_complete_source_model_v68(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&str, [usize; 6]),
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_variant(work, storage, true, |plan, out| {
        source_function::tests::with_slots(plan, out, |slots, out| {
            let relation = slots.correspondence(out)?;
            let launches = [ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }; 2];
            let census = generate_refinement_v36(
                relation,
                &launches,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            write!(
                out,
                "\nverus! {{\n{LENGTH_EQUATIONS_V68}\n{DESCRIPTOR_EFFECT_EQUATIONS_V68}\n}}\n"
            )
            .map_err(|_| out.error())?;
            examine(&out.text, census);
            Ok(())
        })
    })
}

#[test]
fn original_complete_source_model_keeps_signed_length_boundaries_and_all_shared_bodies() {
    let mut reached = false;
    let result = run_complete_source_model_v68(LIMIT, LIMIT, |text, census| {
        assert_eq!(census[0], 2);
        assert!(text.contains(source_bytes::SOURCE_BYTES_V36));
        assert!(text.contains(source_frames::SOURCE_FRAMES_V36));
        assert!(text.contains(source_function::SOURCE_FUNCTION_V36));
        for boundary in [
            "invocation_source_descriptor_reference_shape_v51(snapshot.reference, source.machine.values.len() as int)",
            "invocation_source_logical_well_formed_v38(source.logical, source.machine.values.len() as int)",
        ] {
            assert_eq!(text.matches(boundary).count(), 1);
            assert!(!text.contains(&boundary.replace(" as int", "")));
        }
        for retained in [
            "reference: InvocationSourceDescriptorReferenceV51, count: int,",
            "logical: InvocationSourceLogicalV38, count: int,",
            "0 <= reference.loan.origin < count",
            "logical.versions.len() == count",
            "source.logical.versions[recipe.origin] == reference.loan.version",
            "source.machine.frames.active[i] == reference.loan.frame",
            "proof fn descriptor_snapshot_does_not_rebind_a_replaced_origin_v53(",
            "proof fn descriptor_snapshot_requires_original_frame_after_return_v53(",
        ] {
            assert!(text.contains(retained), "{retained}");
        }
        assert!(text.ends_with(&format!(
            "\nverus! {{\n{LENGTH_EQUATIONS_V68}\n{DESCRIPTOR_EFFECT_EQUATIONS_V68}\n}}\n"
        )));
        assert_eq!(LENGTH_EQUATIONS_V68.matches("proof fn ").count(), 6);
        for equation in [
            "native_unsigned_external_generation_is_admitted_v77",
            "native_negative_live_generation_is_rejected_v77",
            "native_input_allocations_satisfy_frame_liveness_v77",
        ] {
            assert_eq!(text.matches(&format!("proof fn {equation}(")).count(), 1);
        }
        // Five semantic obligations and their independently checked shared helper.
        assert_eq!(
            DESCRIPTOR_EFFECT_EQUATIONS_V68.matches("proof fn ").count(),
            6
        );
        for forbidden in ["assume(", "external_body", "admit(", "assume_specification"] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
        reached = true;
    });
    result.0.unwrap();
    assert!(reached);
    assert_eq!(result.2, 37);
}

#[test]
fn original_complete_source_model_includes_operand_effect_acceptance_and_refusal_equations() {
    let result = run_complete_source_model_v68(LIMIT, LIMIT, |text, _| {
        assert!(text.contains(super::super::effects::INVOCATION_EFFECTS_V36));
        assert!(text.contains(DESCRIPTOR_EFFECT_EQUATIONS_V68));
        for equation in [
            "aggregate_or_descriptor_exact_evaluation_has_no_external_effect_v68",
            "aggregate_or_descriptor_changed_state_is_refused_v68",
            "aggregate_or_descriptor_changed_value_is_refused_v68",
            "aggregate_or_descriptor_invalid_observation_is_refused_v68",
            "descriptor_operand_mutable_copy_is_refused_v68",
        ] {
            assert_eq!(text.matches(&format!("proof fn {equation}(")).count(), 1);
        }
    });
    result.0.unwrap();
    assert_eq!(result.2, 37);
}

#[test]
fn original_complete_source_model_retains_explicit_guard_slice_and_return_witnesses() {
    let result = run_complete_source_model_v68(LIMIT, LIMIT, |text, census| {
        assert_eq!(census[0], 2);
        for equation in [
            "invocation_nested_source_guard_cannot_disappear_v40",
            "invocation_nested_target_guard_cannot_appear_without_match_v40",
            "invocation_source_slice_borrow_invalid_element_refuses_v74",
            "invocation_source_enum_return_cannot_hide_a_callee_pointer_v50",
            "invocation_source_slice_invalid_element_breaks_all_v77",
            "invocation_source_pointer_value_escapes_frame_v77",
            "aggregate_or_descriptor_singleton_effects_v77",
        ] {
            assert_eq!(text.matches(&format!("proof fn {equation}(")).count(), 1);
        }
        for witness in [
            "assert(exists|j: int| 0 <= j < target_view.guards.len()",
            "let guard = source_view.guards[0];",
            "assert(exists|i: int| 0 <= i < source_view.guards.len()",
            "let guard = target_view.guards[0];",
            "if forall|element: int| 0 <= element < slice.length ==>",
            "assert(exists|index: int| value.fields.contains_key(index)",
            "reveal(invocation_source_snapshot_escapes_frame_v42);",
        ] {
            assert_eq!(text.matches(witness).count(), 1, "{witness}");
        }
        for isolated in [
            "invocation_source_slice_borrow_invalid_element_refuses_v74",
            "invocation_source_enum_return_cannot_hide_a_callee_pointer_v50",
            "aggregate_or_descriptor_singleton_effects_v77",
        ] {
            assert!(text.contains(&format!(
                "#[verifier::spinoff_prover]\nproof fn {isolated}("
            )));
        }
        assert_eq!(
            text.matches("spec fn invocation_source_slice_borrow_step_v77(")
                .count(),
            1
        );
        assert!(
            !text.contains("#[verifier::opaque]\nspec fn invocation_source_slice_borrow_step_v77(")
        );
        assert!(text.contains(
            "&& invocation_source_value_escapes_frame_v36(value.fields[field], frame))) by (compute_only);"
        ));
        for contract in [
            "0 < source_view.guards.len(), target_view.guards.len() == 0,",
            "source_view.guards.len() == 0, 0 < target_view.guards.len(),",
            "0 <= index < slice.length,\n        !invocation_source_borrow_enabled_v36(source,",
            "value.fields[field] == MemoryValueV30::Pointer(pointer),",
        ] {
            assert!(text.contains(contract), "{contract}");
        }
    });
    result.0.unwrap();
    assert_eq!(result.2, 37);
}

#[test]
fn original_complete_source_model_length_equations_have_exact_and_one_short_resources() {
    let execute = |work, storage| {
        run_complete_source_model_v68(work, storage, |_, census| assert_eq!(census[0], 2))
    };
    let measured = execute(LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = execute(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, 37, measured.3));
    for (work, storage, work_failure) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let error = execute(work, storage).0.unwrap_err();
        let resource = match error {
            Error::Resource(resource)
            | Error::Source(fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                resource,
            )) => resource,
            other => panic!("expected complete-model resource refusal: {other:?}"),
        };
        match (work_failure, resource) {
            (true, Resource::Work(error)) => {
                assert_eq!((error.actual(), error.limit()), (measured.1, work));
            }
            (false, Resource::Storage(error)) => {
                assert_eq!((error.actual(), error.limit()), (measured.3, storage));
            }
            other => panic!("complete-model resource boundary: {other:?}"),
        }
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "diagnostic-only exact complete-model export; never executes or approves a proof"]
fn diagnostic_complete_original_source_model_export_without_execution_v77() {
    let destination = std::path::PathBuf::from(
        std::env::var_os("FE2O3_DIAGNOSTIC_COMPLETE_MODEL_EXPORT")
            .expect("owned diagnostic output path"),
    );
    assert!(destination.is_absolute());
    let mut exported = false;
    let result = run_complete_source_model_v68(LIMIT, LIMIT, |text, census| {
        assert_eq!(census[0], 2);
        assert!(!text.is_empty() && text.len() <= 16 * 1024 * 1024);
        let input = crate::CanonicalGeneratedVerusProofInputV3::new(text.as_bytes().to_vec())
            .expect("identical canonical complete source-model input");
        drop(input);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .expect("fresh diagnostic export file");
        std::io::Write::write_all(&mut file, text.as_bytes()).expect("complete diagnostic export");
        file.sync_all().expect("retain diagnostic export bytes");
        assert_eq!(std::fs::read(&destination).unwrap(), text.as_bytes());
        println!(
            "DIAGNOSTIC_ONLY_COMPLETE_MODEL_EXPORT bytes={} census={census:?}",
            text.len()
        );
        exported = true;
    });
    result.0.unwrap();
    assert!(exported);
    assert_eq!(result.2, 37);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_complete_original_source_model_preserves_signed_length_boundaries() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};

    let mut generated = None;
    let result = run_complete_source_model_v68(LIMIT, LIMIT, |text, census| {
        assert_eq!(census[0], 2);
        generated = Some(text.to_owned());
    });
    result.0.unwrap();
    assert_eq!(result.2, 37);
    let input = CanonicalGeneratedVerusProofInputV3::new(
        generated
            .expect("complete original-source proof emission")
            .into_bytes(),
    )
    .expect("canonical complete source model");
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open_pinned_contexts_v3(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("requires the actual public pinned-runtime lease");
    runtime
        .revalidate()
        .expect("revalidate before source-model proof");
    let mut attempt = runtime
        .begin_attempt()
        .expect("acquire source-model proof attempt");
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &input,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .expect("execute every emitted shared source-model obligation");
    runtime
        .revalidate()
        .expect("revalidate after source-model proof");
    attempt
        .complete()
        .expect("complete source-model proof attempt");
    crate::functional_refinement_receipt_v2::validate_proved_output(&output)
        .expect("complete model and length equations must genuinely verify");
}

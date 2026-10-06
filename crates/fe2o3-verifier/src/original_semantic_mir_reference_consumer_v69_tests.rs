use super::*;
use fe2o3_kernel_ir::EndiannessV2;

const LIMIT: usize = 100_000_000;
const EQUATIONS: &str = include_str!("original_semantic_mir_reference_coverage_v69_tests.vrs");

#[test]
fn absent_registered_references_preserve_original_bytes_and_resource_state_exactly() {
    let result =
        super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let relation = slots.correspondence(out)?;
                write!(out, "// existing original-source bytes\n").map_err(|_| out.error())?;
                let before = (
                    <[u8; 32]>::from(Sha256::digest(out.text.as_bytes())),
                    out.text.len(),
                    out.budget.work(),
                    out.budget.storage(),
                    out.budget.peak_storage(),
                );
                let ledger = out.budget.work_ledger_identity_v1();
                emit(
                    relation,
                    plan,
                    slots,
                    &[],
                    &[],
                    FormalIndexWidth::Bits64,
                    out,
                )?;
                assert_eq!(
                    before,
                    (
                        <[u8; 32]>::from(Sha256::digest(out.text.as_bytes())),
                        out.text.len(),
                        out.budget.work(),
                        out.budget.storage(),
                        out.budget.peak_storage()
                    )
                );
                assert!(out.budget.work_ledger_identity_v1() == ledger);
                Ok(())
            })
        });
    result.0.unwrap();
    assert_eq!(result.2, 37);
}

#[test]
fn registered_reference_closed_control_never_hides_a_trailing_assert_or_switch() {
    let condition = ReferenceOperandV1::Constant(ReferenceConstantV1::Scalar {
        scalar: ReferenceScalarTypeV1::Bool,
        bits: 1,
    });
    let mut ir = ReferenceEffectIrV1 {
        argument_count: 0,
        local_count: 1,
        relations: Box::new([]),
        blocks: vec![ReferenceBlockV1 {
            block: 0,
            assignments: Box::new([]),
            terminator: ReferenceTerminatorV1::Return,
        }]
        .into_boxed_slice(),
        loop_summaries: Box::new([]),
        observable_output_effects: Box::new([]),
    };
    assert!(closed_control(&ir));
    ir.blocks[0].terminator = ReferenceTerminatorV1::Goto { target: 1 };
    // This helper grants no topology authority; replay must still reject the
    // nonexistent successor, even though Goto belongs to the closed vocabulary.
    assert!(closed_control(&ir));
    for terminator in [
        ReferenceTerminatorV1::Assert {
            condition: condition.clone(),
            expected: true,
            success: 1,
            bounds_check: None,
        },
        ReferenceTerminatorV1::Switch {
            discriminant: condition,
            values: vec![(1, 1)].into_boxed_slice(),
            otherwise: 2,
        },
    ] {
        ir.blocks[0].terminator = terminator;
        assert!(!closed_control(&ir));
    }
    ir.blocks = Box::new([]);
    assert!(!closed_control(&ir));
}

fn run(
    work: usize,
    storage: usize,
    inspect: impl FnOnce(&str),
) -> (Result<()>, usize, usize, usize) {
    super::super::super::invocations::tests::run_variant(work, storage, true, |plan, out| {
        super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            let relation = slots.correspondence(out)?;
            let launches = [ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            }; 2];
            super::super::generate_refinement_v36(
                relation,
                &launches,
                FormalIndexWidth::Bits64,
                EndiannessV2::Little,
                out,
            )?;
            write!(out, "\nverus! {{\n{COVERAGE}\n{EQUATIONS}\n}}\n").map_err(|_| out.error())?;
            inspect(&out.text);
            Ok(())
        })
    })
}

#[test]
fn registered_reference_coverage_keeps_exact_effects_and_independent_output_domains() {
    let measured = run(LIMIT, LIMIT, |text| {
        assert!(text.contains(COVERAGE));
        assert!(text.contains(EQUATIONS));
        assert_eq!(EQUATIONS.matches("proof fn ").count(), 9);
        assert!(!COVERAGE.contains("expected.len() > 0"));
        for required in [
            "forall|i: int| #![trigger expected[i]]",
            "forall|i: int, j: int| #![trigger expected[i], expected[j]]",
            "forall|i: int| #![trigger observed[i]]",
            "exists|j: int| #![trigger expected[j]]",
            "invocation_reference_write_count_v69(observed, expected[i]) == 1",
            "left.argument != right.argument",
            "observed[i].before.machine.valid && observed[i].after.machine.valid",
            "reference_output_domain_keeps_zero_length_and_oversized_launch_points_v69",
            "reference_outputs_with_different_lengths_have_independent_domains_v69",
        ] {
            assert!(text.contains(required), "{required}");
        }
        for forbidden in ["assume(", "admit(", "external_body", "assume_specification"] {
            assert!(!text.contains(forbidden), "{forbidden}");
        }
    });
    measured.0.unwrap();
    assert_eq!(measured.2, 37);
}

#[test]
fn registered_reference_shared_equation_text_has_exact_and_one_short_generation_limits() {
    let measured = run(LIMIT, LIMIT, |_| {});
    measured.0.unwrap();
    let exact = run(measured.1, measured.3, |_| {});
    exact.0.unwrap();
    assert_eq!((exact.1, exact.2, exact.3), (measured.1, 37, measured.3));
    for (work, storage) in [(measured.1 - 1, measured.3), (measured.1, measured.3 - 1)] {
        let short = run(work, storage, |_| {
            panic!("one-short emission reached success")
        });
        assert!(short.0.is_err());
        assert_eq!(short.2, 37);
    }
}

#[test]
fn registered_reference_emission_headers_cover_independent_shapes() {
    type ExpressionContext<'a> = (&'a (), &'a [()], u32, u32);
    let expected = size_of::<Sha256>()
        + align_of::<Sha256>()
        + size_of::<[&[u8]; 4]>()
        + size_of::<[u8; 4]>()
        + size_of::<[u8; 32]>()
        + size_of::<Vec<bool>>()
        + size_of::<ReplayedCpuEffectsV1>()
        + size_of::<ReferenceReplayInputV1<'_>>()
        + size_of::<(String, Option<Resource>, Result<()>)>()
        + size_of::<ExpressionContext<'_>>()
        + 16 * size_of::<&()>()
        + 16 * size_of::<usize>();
    assert_eq!(
        size_of::<ExpressionContext<'_>>(),
        size_of::<expressions::Context<'_>>()
    );
    assert_eq!(headers(), expected);
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the root-owned pinned runtime; shared equations do not replace actual-source execution"]
fn protected_registered_reference_coverage_rejects_missing_extra_and_out_of_domain_writes() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};
    let mut generated = None;
    run(LIMIT, LIMIT, |text| generated = Some(text.to_owned()))
        .0
        .unwrap();
    let input = CanonicalGeneratedVerusProofInputV3::new(generated.unwrap().into_bytes()).unwrap();
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open_pinned_contexts_v3(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .unwrap();
    runtime.revalidate().unwrap();
    let mut attempt = runtime.begin_attempt().unwrap();
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &input,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .unwrap();
    runtime.revalidate().unwrap();
    attempt.complete().unwrap();
    crate::functional_refinement_receipt_v2::validate_proved_output(&output).unwrap();
}

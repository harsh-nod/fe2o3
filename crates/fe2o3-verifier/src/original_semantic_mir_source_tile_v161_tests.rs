use super::*;
use fe2o3_kernel_ir::{EndiannessV2, ExplicitLaunchExtent, FormalIndexWidth};
use std::fmt::Write as _;

const TILE_MODEL: &str = include_str!("original_semantic_mir_source_tile_v161.vrs");
const TILE_LAWS: &str = include_str!("original_semantic_mir_source_tile_v161_tests.vrs");
const LIMIT: usize = 512 * 1024 * 1024;

fn run_model(
    work: usize,
    storage: usize,
    examine: impl FnOnce(&str),
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
            write!(out, "\nverus! {{\n{TILE_LAWS}\n}}\n").map_err(|_| out.error())?;
            examine(&out.text);
            Ok(())
        })
    })
}

#[test]
fn original_tile_model_is_in_the_complete_source_step_and_effect_model() {
    let (result, _, floor, _) = run_model(LIMIT, LIMIT, |text| {
        assert_eq!(text.matches(TILE_MODEL).count(), 1);
        assert_eq!(text.matches(TILE_LAWS).count(), 1);
        for operation in ["ContextIssue", "TileLoad", "TileTransport"] {
            assert_eq!(
                text.matches(&format!("InvocationSourceByteEventV36::{operation}("))
                    .count(),
                3
            );
        }
        for forbidden in ["assume(", "admit(", "external_body", "uninterpreted"] {
            assert!(!TILE_MODEL.contains(forbidden));
            assert!(!TILE_LAWS.contains(forbidden));
        }
        // These shared equations are not evidence of a source tile constructor
        // or source-to-expanded whole-kernel refinement succeeding.
        assert!(!text.contains("let event = InvocationSourceByteEventV36::TileLoad("));
    });
    result.unwrap();
    assert_eq!(floor, 37);
}

#[test]
fn original_tile_model_generation_has_exact_and_one_short_resource_boundaries() {
    let (result, work, floor, peak) = run_model(LIMIT, LIMIT, |_| {});
    result.unwrap();
    assert_eq!(floor, 37);
    let (result, used, after, retained_peak) = run_model(work, peak, |_| {});
    result.unwrap();
    assert_eq!((used, after, retained_peak), (work, floor, peak));
    for (work, storage) in [(work - 1, peak), (work, peak - 1)] {
        let (result, _, after, _) = run_model(work, storage, |_| {});
        assert!(result.is_err());
        assert_eq!(after, floor);
    }
}

#[test]
#[ignore = "diagnostic complete shared-model export; grants no source tile or proof authority"]
fn diagnostic_complete_original_tile_shared_model_export_without_execution_v161() {
    use sha2::{Digest, Sha256};
    use std::io::{BufWriter, Write as _};
    run_model(LIMIT, LIMIT, |text| {
        assert!(text.len() <= 16 * 1024 * 1024);
        let mut output = BufWriter::new(std::io::stdout().lock());
        write!(output, "{{\"kind\":\"fe2o3-original-tile-shared-model-v161\",\"scope\":\"shared equations only\",\"bytes\":{},\"sha256\":\"", text.len()).unwrap();
        for byte in Sha256::digest(text.as_bytes()) {
            write!(output, "{byte:02x}").unwrap();
        }
        write!(output, "\",\"model_hex\":\"").unwrap();
        for byte in text.as_bytes() {
            write!(output, "{byte:02x}").unwrap();
        }
        writeln!(output, "\"}}").unwrap();
        output.flush().unwrap();
    })
    .0
    .unwrap();
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_original_tile_model_and_component_laws_verify() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};
    let mut generated = None;
    run_model(LIMIT, LIMIT, |text| generated = Some(text.to_owned()))
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

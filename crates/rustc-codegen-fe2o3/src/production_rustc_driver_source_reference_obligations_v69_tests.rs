//! Actual live-rustc reference capture; no semantic proof or publication claim.
use super::*;
use crate::production_pipeline::source_owned_v29::ReferenceObligationObservationV69;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::reference_obligation_tests::source_reference_obligation_child";

#[derive(Default)]
struct ReferenceCallbacks {
    result: Option<Result<ReferenceObligationObservationV69, String>>,
}

impl Callbacks for ReferenceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            transaction()?
                .reference_obligations_observation_for_test_v69(transaction()?)
                .map_err(|error| format!("reference foundation: {error:?}"))
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "private live-rustc child, invoked only by its owning source-capture parent"]
fn source_reference_obligation_child() {
    let args: Vec<String> = serde_json::from_slice(
        &std::fs::read(env::var_os(ARGS).expect("reference rustc arguments")).unwrap(),
    )
    .unwrap();
    let mut callbacks = ReferenceCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual reference capture callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("reference result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "reference obligation capture: {result:?}");
}

fn source(scalar: &str) -> String {
    format!(
        r#"use fe2o3_device::{{DisjointSlice, kernel, thread}};
fn alpha_reference(_point: usize, output: &mut {scalar}) {{ *output = 17 as {scalar}; }}
fn zeta_reference(_point: usize, output: &mut {scalar}) {{ *output = 23 as {scalar}; }}
#[kernel(typed, reference = alpha_reference, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn alpha(mut output: DisjointSlice<{scalar}>) {{
    if let Some(element) = output.get_mut(thread::index_1d()) {{ *element = 17 as {scalar}; }}
}}
#[kernel(typed, reference = zeta_reference, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn zeta(mut output: DisjointSlice<{scalar}>) {{
    if let Some(element) = output.get_mut(thread::index_1d()) {{ *element = 23 as {scalar}; }}
}}
"#
    )
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_registered_scalar_references_bind_original_roots_without_semantic_authority() {
    run_actual_sources::<ReferenceObligationObservationV69>(
        &[("integer", "u32"), ("float", "f32"), ("integer", "u32")],
        &[(0, 0)],
        CHILD,
        "SOURCE_REFERENCE_OBLIGATIONS_V69",
        source,
        |_, _, label, result, prior| {
            assert_eq!((result.roots, result.obligations), (2, 2));
            assert_eq!((result.substitutions, result.custody_checks), (6, 8));
            assert!(result.exact_boundaries && result.semantic_proof_still_required);
            assert!(result.work > 0 && result.work < 500_000_000);
            assert!(result.peak > 37 && result.peak <= 64_000_000);
            if let Some(previous) = prior.get(label) {
                assert_eq!(&result, previous);
            } else {
                prior.insert(label.to_owned(), result);
            }
        },
    );
}

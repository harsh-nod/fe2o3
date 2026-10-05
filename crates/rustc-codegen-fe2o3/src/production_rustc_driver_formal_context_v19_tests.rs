//! Actual compiler bindings through the fixed original/output report consumer.
use super::*;
use fe2o3_kernel_ir::{CanonicalFormalLaunchInputV19, ExplicitLaunchExtent, FormalIndexWidth};
use std::cell::Cell;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::formal_context_tests::formal_context_child";

fn program(_: &str) -> String {
    r#"use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [3, 1, 1]))]
pub fn reports_alpha(_word: usize) {}
#[kernel(typed, launch(required = [32, 1, 1], max = [32, 1, 1], max_grid = [5, 1, 1]))]
pub fn reports_beta(_word: u32) {}
"#
    .to_owned()
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    roots: usize,
    extents: Vec<[u64; 3]>,
    original: [u8; 32],
    output: [u8; 32],
    complete: bool,
    prepared: bool,
    materialized: bool,
    exact_consumer_refusal: bool,
}

#[derive(Default)]
struct FormalCallbacks {
    result: Option<Result<Observation, String>>,
}

impl Callbacks for FormalCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            start_preparation_observation_v29();
            let mut roots = 0;
            let mut extents = Vec::new();
            let mut original = None;
            let mut output = None;
            let mut complete = true;
            let continuation = transaction()?
                .with_original_source_formal_reports_v19(|before, after, _| {
                    assert_eq!(before.root_index(), roots);
                    assert_eq!(after.root_index(), roots);
                    assert!(!std::ptr::eq(
                        before.original_owner(),
                        after.original_owner()
                    ));
                    assert_eq!(before.index_width(), FormalIndexWidth::Bits64);
                    assert_eq!(after.index_width(), FormalIndexWidth::Bits64);
                    assert_eq!(before.launch_input(), after.launch_input());
                    let CanonicalFormalLaunchInputV19::PhysicalEnvelope(
                        ExplicitLaunchExtent::Exact {
                            rank: 1,
                            extents: bounds,
                        },
                    ) = before.launch_input()
                    else {
                        panic!("report lost authenticated physical coordinate bounds")
                    };
                    assert_eq!(&bounds[1..], &[1, 1]);
                    assert!(bounds[0] == 160 || bounds[0] == 192);
                    assert_eq!(
                        before.analysis().obligations().kernel(),
                        after.analysis().obligations().kernel()
                    );
                    assert_eq!(
                        before.analysis().obligations().entry(),
                        after.analysis().obligations().entry()
                    );
                    complete &= before.analysis().is_complete() && after.analysis().is_complete();
                    let input_sha: [u8; 32] =
                        Sha256::digest(before.original_owner().canonical_bytes()).into();
                    let output_sha: [u8; 32] =
                        Sha256::digest(after.original_owner().canonical_bytes()).into();
                    if let Some(previous) = original {
                        assert_eq!(previous, input_sha);
                    }
                    if let Some(previous) = output {
                        assert_eq!(previous, output_sha);
                    }
                    original = Some(input_sha);
                    output = Some(output_sha);
                    extents.push(bounds);
                    roots += 1;
                    Ok(())
                })
                .map_err(|error| format!("actual bound reports: {error:?}"))?;
            continuation.into_observation();
            let preparation =
                take_preparation_observation_v29().expect("prepared source observation");
            assert!(preparation.materialized);
            assert_eq!(roots, 2);
            extents.sort();
            assert_eq!(extents, vec![[160, 1, 1], [192, 1, 1]]);
            assert!(complete);

            let completed = Cell::new(false);
            let error = refused(transaction()?.with_original_source_formal_reports_v19(
                |before, after, _| {
                    assert_eq!(before.root_index(), 0);
                    assert_eq!(after.root_index(), 0);
                    assert_eq!(before.index_width(), FormalIndexWidth::Bits64);
                    completed.set(true);
                    Err(Error::Unsupported("actual bound reports requested refusal"))
                },
            ));
            assert!(completed.get());
            let mut cause: &(dyn std::error::Error + 'static) = &error;
            let mut exact = false;
            loop {
                if let Some(Error::Unsupported("actual bound reports requested refusal")) =
                    cause.downcast_ref::<Error>()
                {
                    exact = true;
                    break;
                }
                let Some(next) = cause.source() else {
                    break;
                };
                cause = next;
            }
            assert!(exact, "consumer error was not preserved: {error:?}");
            Ok(Observation {
                roots,
                extents,
                original: original.unwrap(),
                output: output.unwrap(),
                complete,
                prepared: true,
                materialized: preparation.materialized,
                exact_consumer_refusal: exact,
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn formal_context_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = FormalCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("bound formal report callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("bound report result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "bound reports: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_original_source_reports_bind_all_descriptors_and_legal_padded_coordinates() {
    run_actual_sources::<Observation>(
        &[("formal_context", "")],
        &[(0, 0)],
        CHILD,
        "ORIGINAL_SOURCE_FORMAL_CONTEXT_V19",
        program,
        |_, _, _, report, _| {
            assert_eq!(report.roots, 2);
            assert_eq!(report.extents, vec![[160, 1, 1], [192, 1, 1]]);
            assert!(
                report.complete
                    && report.prepared
                    && report.materialized
                    && report.exact_consumer_refusal
            );
        },
    );
}

#[test]
fn actual_formal_context_fixture_has_two_distinct_complete_launch_contracts() {
    let source = program("");
    assert_eq!(source.matches("#[kernel(").count(), 2);
    assert!(source.contains("max_grid = [3, 1, 1]"));
    assert!(source.contains("max_grid = [5, 1, 1]"));
    assert!(source.contains("_word: usize"));
    assert!(CHILD.ends_with("::formal_context_tests::formal_context_child"));
}

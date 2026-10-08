//! Genuine complete original-source preparation, not a component emitter test.
use super::*;
use crate::production_pipeline::source_owned_v29::AtomicRootObservationV41;
const ATOMIC_ROOT_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::atomic_import_v41_tests::atomic_root_v41_tests::atomic_v41_original_whole_root_child";

#[derive(Default)]
struct AtomicRootCallbacksV41 {
    result: Option<Result<AtomicRootObservationV41, String>>,
}
impl Callbacks for AtomicRootCallbacksV41 {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let original = transaction()?
                .source_owned_atomic_ssa_for_test_v41()
                .map_err(|error| format!("original atomic source: {error:?}"))?;
            let original_sha = *original.source_semantic_sha256();
            let rows = complete_atomic_census_v41(original.source_semantic());
            let mut census = [[0usize; 5]; 10];
            for row in &rows {
                let value = &mut census[usize::from(row.operation)][usize::from(row.ordering)];
                *value = value.checked_add(1).expect("bounded original census");
            }
            drop((rows, original));
            // This independently invokes the real compiler importer, ABI and
            // launch bindings, original planner and whole-root ExpandedRaw
            // completion. Any refusal remains a failed test, never a fallback.
            let result = transaction()?
                .source_owned_atomic_root_for_test_v41()
                .map_err(|error| format!("original atomic whole-root preparation: {error:?}"))?;
            assert_eq!(result.source, original_sha);
            assert_eq!(result.original, census);
            assert_eq!(result.roots, 1);
            assert!(result.instances >= result.active_instances);
            assert!(result.active_instances > 0);
            assert!(result.anchors > 0);
            assert!(result.pointer_to_generic > 0);
            assert!(result.emitted.iter().all(|row| row.iter().any(|n| *n > 0)));
            Ok(result)
        })());
        Compilation::Stop
    }
}
#[test]
#[ignore = "process helper; only its exact actual-source parent supplies arguments"]
fn atomic_v41_original_whole_root_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = AtomicRootCallbacksV41::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("original atomic whole-root callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("atomic whole-root result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "original atomic whole-root: {result:?}");
}
#[test]
#[ignore = "requires pinned nightly rust-src, authentic device dependencies and original source compilation"]
fn actual_atomic_v41_whole_root_preserves_original_rmw_contracts() {
    run_actual_sources::<AtomicRootObservationV41>(
        &[("atomic-v41-whole-root", ""), ("atomic-v41-whole-root", "")],
        &[(0, 0)],
        ATOMIC_ROOT_CHILD,
        "ATOMIC_V41_ORIGINAL_WHOLE_ROOT",
        atomic_source_v41,
        |_, _, label, result, prior| {
            assert_eq!(result.roots, 1);
            assert!(result.emitted.iter().all(|row| row.iter().any(|n| *n > 0)));
            if let Some(previous) = prior.get(label) {
                assert_eq!(&result, previous);
            } else {
                prior.insert(label.to_owned(), result);
            }
        },
    );
}

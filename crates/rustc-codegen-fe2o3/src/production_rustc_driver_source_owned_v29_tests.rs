//! Genuine live-rustc capture, not manufactured or reencoded MIR29 fixtures.
use super::*;
use crate::production_pipeline::source_owned_v29::{Error, paid_vec};
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiInputV18 as AbiInput, ProductionKernelArgumentAbiRootV18 as AbiRoot,
    ProductionSourceOwnedViewErrorV18 as SourceError,
};

const SOURCE_OWNED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::source_owned_scalar_child";

#[path = "production_rustc_driver_source_owned_target_llvm_v29_tests.rs"]
mod target_llvm_tests;

#[path = "production_rustc_driver_source_preparation_v29_tests.rs"]
mod preparation_tests;

#[path = "production_rustc_driver_source_integer_handoff_v29_tests.rs"]
mod integer_handoff_tests;

#[path = "production_rustc_driver_original_source_v18_tests.rs"]
mod original_source_tests;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ScalarResult {
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    roots: usize,
    changed: bool,
    work: usize,
    peak: usize,
    foreign_source_refused: bool,
    foreign_profile_refused: bool,
    foreign_custody_refused: bool,
    undercut_refused: bool,
    raw_payload_preserved: bool,
    exact_and_short_storage: bool,
    publication_bindings_retained: bool,
    callback_error_preserved: bool,
}

#[derive(Default)]
struct ScalarCallbacks {
    result: Option<Result<ScalarResult, String>>,
}

impl Callbacks for ScalarCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            // Both owners come from independent original rustc captures. The
            // fixed profile changes admission, not the captured source body.
            let legacy = transaction()?
                .source_owned_ssa_for_test_v29(false)
                .map_err(|error| format!("legacy source: {error:?}"))?;
            assert_eq!(
                legacy.source_semantic().wire_version(),
                SemanticMirWireVersionV1::V5
            );
            let foreign = transaction()?
                .source_owned_ssa_for_test_v29(true)
                .map_err(|error| format!("foreign source: {error:?}"))?;
            assert_eq!(
                foreign.source_semantic().wire_version(),
                SemanticMirWireVersionV1::V29
            );
            let expected_target = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
                .and_then(|target| target.authenticate_import_session(tcx))
                .map_err(|error| format!("independent target observation: {error:?}"))?;
            let continuation = transaction()?
                .with_source_owned_scalar_custody_v29(|source, handoff, budget| {
                    let original = source.canonical(budget)?;
                    let output = handoff.output(budget)?;
                    assert_eq!(
                        source.source_semantic(budget)?.wire_version(),
                        SemanticMirWireVersionV1::V29
                    );
                    assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                    assert_eq!(
                        source.source_ssa(budget)?.source_semantic_sha256(),
                        foreign.source_semantic_sha256()
                    );
                    assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                    assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    let retained = handoff.retained_storage(budget)?;
                    assert!(retained > 0 && budget.storage() >= retained);
                    let changed = output.owner().canonical_bytes() != output.input_audit_bytes();
                    Ok(ScalarResult {
                        source: *source.source_ssa(budget)?.source_semantic_sha256(),
                        original: Sha256::digest(original.canonical_bytes()).into(),
                        output: Sha256::digest(output.owner().canonical_bytes()).into(),
                        roots: original.module().kernels.len(),
                        changed,
                        work: budget.work(),
                        peak: budget.peak_storage(),
                        foreign_source_refused: false,
                        foreign_profile_refused: false,
                        foreign_custody_refused: false,
                        undercut_refused: false,
                        raw_payload_preserved: false,
                        exact_and_short_storage: false,
                        publication_bindings_retained: false,
                        callback_error_preserved: false,
                    })
                })
                .map_err(|error| format!("genuine handoff: {error:?}"))?;
            continuation.assert_retained_bindings_for_test_v29(&foreign, expected_target.profile());
            let mut report = continuation.into_observation();
            report.publication_bindings_retained = true;

            struct DropWitness<'a>(&'a std::cell::Cell<usize>);
            impl Drop for DropWitness<'_> {
                fn drop(&mut self) {
                    self.0.set(self.0.get() + 1);
                }
            }
            let callback_entered = std::cell::Cell::new(false);
            let callback_dropped = std::cell::Cell::new(0);
            let entered = &callback_entered;
            let witness = DropWitness(&callback_dropped);
            let refused =
                transaction()?.with_source_owned_scalar_custody_v29::<(), _>(move |_, _, _| {
                    let _owned = witness;
                    entered.set(true);
                    Err(Error::Unsupported("original custody callback refusal"))
                });
            assert!(callback_entered.get());
            assert_eq!(callback_dropped.get(), 1);
            assert!(matches!(
                refused,
                Err(Error::Unsupported("original custody callback refusal"))
            ));
            report.callback_error_preserved = true;

            let error = transaction()?
                .with_source_owned_scalar_handoff_v29::<(), _>(|_, handoff, budget| {
                    handoff.check_original_source(&foreign, budget)?;
                    panic!("equal bytes do not replace the original SSA owner")
                })
                .unwrap_err();
            assert!(
                matches!(error, Error::Source(SourceError::Binding(_))),
                "{error:?}"
            );
            report.foreign_source_refused = true;

            let error = transaction()?
                .with_source_owned_scalar_test_v29::<(), _>(|source, _, roots, budget| {
                    assert_eq!(roots.len(), 2);
                    assert_ne!(roots[0].kernel_binding, roots[1].kernel_binding);
                    source.require_kernel_argument_abi_v18(AbiInput { roots }, budget)?;
                    // Substitute the other genuine root's binding, with the exact
                    // original argument signature and component rows unchanged.
                    let mut foreign_roots = paid_vec(roots.len(), budget)?;
                    for (index, root) in roots.iter().enumerate() {
                        foreign_roots.push(AbiRoot {
                            kernel_binding: roots[1 - index].kernel_binding,
                            export: root.export,
                            arguments: root.arguments,
                            explicit_argument_bytes: root.explicit_argument_bytes,
                            kernarg_alignment_bytes: root.kernarg_alignment_bytes,
                        });
                    }
                    source.require_kernel_argument_abi_v18(
                        AbiInput {
                            roots: &foreign_roots,
                        },
                        budget,
                    )?;
                    panic!("foreign root profile reached the continuation")
                })
                .unwrap_err();
            assert!(
                matches!(error, Error::Source(SourceError::Binding(_))),
                "{error:?}"
            );
            report.foreign_profile_refused = true;

            let error = transaction()?
                .with_source_owned_scalar_handoff_v29::<(), _>(|_, handoff, _| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let foreign = Budget::new(&mut work, 20_000_000);
                    handoff.output(&foreign)?;
                    panic!("foreign ledger reached output")
                })
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::Source(SourceError::Resource(ResourceError::Accounting))
                ),
                "{error:?}"
            );
            report.foreign_custody_refused = true;

            let error = transaction()?
                .with_source_owned_scalar_handoff_v29::<(), _>(|_, handoff, budget| {
                    let retained = handoff.retained_storage(budget)?;
                    budget.release_storage(retained)?;
                    handoff.output(budget)?;
                    panic!("undercut output credit reached output")
                })
                .unwrap_err();
            assert!(
                matches!(
                    error,
                    Error::Source(SourceError::Resource(ResourceError::Accounting))
                ),
                "{error:?}"
            );
            report.undercut_refused = true;

            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                transaction()
                    .unwrap()
                    .with_source_owned_scalar_custody_v29::<(), _>(|_, _, _| {
                        std::panic::panic_any(0x1762_u32)
                    })
            }));
            let payload = match result {
                Err(payload) => payload,
                Ok(_) => panic!("raw callback panic returned a custody continuation"),
            };
            assert_eq!(*payload.downcast::<u32>().unwrap(), 0x1762);
            report.raw_payload_preserved = true;

            // The exact same function-item callback and result layout is used
            // for all three real imports, so the measured header is unchanged.
            fn peak<'v, 's>(
                _: &'v fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'s>,
                _: &fe2o3_lower_mir_kernel::ProductionClosedScalarOutputHandoffV18<'v, 's>,
                _: &[AbiRoot<'_>],
                budget: &mut Budget<'_>,
            ) -> Result<usize, Error> {
                Ok(budget.peak_storage())
            }
            let peak_storage = transaction()?
                .with_source_owned_scalar_test_limits_v29(20_000_000, peak)
                .map_err(|error| format!("storage census: {error:?}"))?;
            let exact = transaction()?
                .with_source_owned_scalar_test_limits_v29(peak_storage, peak)
                .map_err(|error| format!("exact storage: {error:?}"))?;
            assert_eq!(exact, peak_storage);
            let error = transaction()?
                .with_source_owned_scalar_test_limits_v29(peak_storage - 1, peak)
                .unwrap_err();
            let mut cause: &(dyn std::error::Error + 'static) = &error;
            loop {
                if let Some(ResourceError::Storage(storage)) = cause.downcast_ref::<ResourceError>()
                {
                    assert_eq!(storage.actual(), peak_storage);
                    assert_eq!(storage.limit(), peak_storage - 1);
                    break;
                }
                cause = cause
                    .source()
                    .unwrap_or_else(|| panic!("not an exact Storage refusal: {error:?}"));
            }
            report.exact_and_short_storage = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn source_owned_scalar_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ScalarCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual source-owned rustc callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("source-owned result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "source-owned continuation: {result:?}");
}

fn scalar_source(body: &str) -> String {
    format!(
        r#"use fe2o3_device::kernel;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_first(mut seed: u32) {{ {body} }}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn scalar_second(mut seed: u32) {{ {body} }}
"#
    )
}

fn check_scalar_sources(cases: &[(&str, &str)], profiles: &[(u8, u8)], changed: bool) {
    run_actual_sources::<ScalarResult>(
        cases,
        profiles,
        SOURCE_OWNED_CHILD,
        "SOURCE_OWNED_MIR29_HANDOFF",
        scalar_source,
        |_, _, label, result, prior| {
            assert_eq!(result.roots, 2);
            assert_eq!(result.changed, changed);
            assert_eq!(result.original != result.output, changed);
            assert!(result.work < 500_000_000 && result.peak <= 20_000_000);
            assert!(result.foreign_source_refused && result.foreign_profile_refused);
            assert!(result.foreign_custody_refused && result.undercut_refused);
            assert!(result.raw_payload_preserved);
            assert!(result.exact_and_short_storage);
            assert!(result.publication_bindings_retained);
            assert!(result.callback_error_preserved);
            if let Some(previous) = prior.get(label) {
                assert_eq!(&result, previous);
            } else {
                prior.insert(label.to_owned(), result);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_scalar_noop_reaches_source_owned_handoff_without_changing_legacy_profile() {
    check_scalar_sources(&[("noop", ""), ("noop", "")], &[(0, 0), (3, 2)], false);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_scalar_dead_store_reaches_changed_source_owned_handoff_and_hostile_checks() {
    check_scalar_sources(
        &[("changed", "seed = 7_u32;"), ("changed", "seed = 7_u32;")],
        &[(0, 0)],
        true,
    );
}

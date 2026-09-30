//! Genuine Current/nominal imports through the original source-owned entrance.
use super::*;

#[path = "production_rustc_driver_bound_worklist_v21_tests.rs"]
mod bound_worklist_tests;

#[path = "production_rustc_driver_mixed_worklist_v26_tests.rs"]
mod mixed_worklist_tests;

#[path = "production_rustc_driver_mixed_pure_cse_v26_tests.rs"]
mod mixed_pure_cse_tests;

#[path = "production_rustc_driver_mixed_cfg_v27_tests.rs"]
mod mixed_cfg_tests;

#[path = "production_rustc_driver_mixed_licm_v28_tests.rs"]
mod mixed_licm_tests;

#[path = "production_rustc_driver_mixed_relocation_v28_tests.rs"]
mod mixed_relocation_tests;

#[path = "production_rustc_driver_array_index_source_v18_tests.rs"]
mod array_index_tests;

#[path = "production_rustc_driver_borrowed_helper_source_v18_tests.rs"]
mod borrowed_helper_tests;
#[path = "production_rustc_driver_formal_context_v19_tests.rs"]
mod formal_context_tests;
#[path = "production_rustc_driver_formal_paths_v20_tests.rs"]
mod formal_paths_tests;
use crate::production_pipeline::source_owned_v29::{
    start_preparation_observation_v29, take_preparation_observation_v29,
};
use fe2o3_kernel_descriptor::SourceTypeDescriptorV3;
use fe2o3_lower_mir_kernel::{
    ProductionKernelArgumentAbiKindV18 as AbiKind,
    ProductionUnqualifiedIntegerOutputHandoffV18 as IntegerHandoff,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticMirErrorV1,
    SemanticRustTypeKindV1,
};

const ORIGINAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::original_source_child";
const MIXED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::original_source_tests::original_source_mixed_refusal_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct OriginalObservation {
    version: u16,
    nominal_usize_types: usize,
    nominal_isize_types: usize,
    nominal_usize_arguments: usize,
    nominal_isize_arguments: usize,
    saturation_calls: usize,
    source: [u8; 32],
    original: [u8; 32],
    output: [u8; 32],
    prepared: bool,
    materialized: bool,
    foreign_owner_refused: bool,
    incomplete_abi_refused: bool,
    foreign_ledger_refused: bool,
    callback_error_preserved: bool,
    exact_and_short_storage: bool,
}

#[derive(Default)]
struct OriginalCallbacks {
    result: Option<Result<OriginalObservation, String>>,
}

fn refused<T>(result: Result<T, Error>) -> Error {
    match result {
        Err(error) => error,
        Ok(_) => panic!("hostile original-source continuation succeeded"),
    }
}

impl Callbacks for OriginalCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            // A second genuine capture has equal identities, but is not the
            // move-only source retained by the consuming transaction below.
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("independent original capture: {error:?}"))?;
            start_preparation_observation_v29();
            let continuation = transaction()?
                .with_original_source_integer_custody_v18(|source, handoff, roots, _, budget| {
                    let semantic = source.source_semantic(budget)?;
                    assert_eq!(
                        semantic.wire_version(),
                        foreign.source_semantic().wire_version()
                    );
                    assert_eq!(source.source_ssa(budget)?.identity(), foreign.identity());
                    assert_eq!(
                        source.source_ssa(budget)?.source_semantic_sha256(),
                        foreign.source_semantic_sha256()
                    );
                    assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                    source.require_kernel_argument_abi_v18(AbiInput { roots }, budget)?;
                    handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                    let original = source.canonical(budget)?;
                    let output = handoff.output(budget)?;
                    assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                    assert!(handoff.retained_storage(budget)? > 0);
                    assert_eq!(roots.len(), 1);
                    Ok(OriginalObservation {
                        version: semantic.wire_version().as_u16(),
                        nominal_usize_types: semantic
                            .types()
                            .iter()
                            .filter(|ty| ty.rust_type_kind() == SemanticRustTypeKindV1::Usize)
                            .count(),
                        nominal_isize_types: semantic
                            .types()
                            .iter()
                            .filter(|ty| ty.rust_type_kind() == SemanticRustTypeKindV1::Isize)
                            .count(),
                        nominal_usize_arguments: roots[0]
                            .arguments
                            .iter()
                            .filter(|argument| {
                                matches!(
                                    argument.kind,
                                    AbiKind::Descriptor {
                                        source: SourceTypeDescriptorV3::Usize,
                                        ..
                                    }
                                )
                            })
                            .count(),
                        nominal_isize_arguments: roots[0]
                            .arguments
                            .iter()
                            .filter(|argument| {
                                matches!(
                                    argument.kind,
                                    AbiKind::Descriptor {
                                        source: SourceTypeDescriptorV3::Isize,
                                        ..
                                    }
                                )
                            })
                            .count(),
                        saturation_calls: semantic
                            .callables()
                            .iter()
                            .filter(|callable| {
                                matches!(
                                    callable,
                                    SemanticCallableDeclV1::CompilerIntrinsic {
                                        operation:
                                            SemanticCompilerIntrinsicOperationV1::SaturatingInteger(
                                                _
                                            ),
                                        ..
                                    }
                                )
                            })
                            .count(),
                        source: *source.source_ssa(budget)?.source_semantic_sha256(),
                        original: Sha256::digest(original.canonical_bytes()).into(),
                        output: Sha256::digest(output.owner().canonical_bytes()).into(),
                        prepared: false,
                        materialized: false,
                        foreign_owner_refused: false,
                        incomplete_abi_refused: false,
                        foreign_ledger_refused: false,
                        callback_error_preserved: false,
                        exact_and_short_storage: false,
                    })
                })
                .map_err(|error| format!("original-source handoff: {error:?}"))?;
            let preparation =
                take_preparation_observation_v29().expect("original preparation observed");
            assert!(preparation.materialized);
            assert!(!preparation.contexts);
            let mut report = continuation.into_observation();
            report.prepared = true;
            report.materialized = preparation.materialized;

            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |_, handoff, _, _, budget| {
                        handoff.check_original_source(&foreign, budget)?;
                        panic!("equal source bytes acquired owner authority")
                    },
                ),
            );
            assert!(
                matches!(error, Error::Source(SourceError::Binding(_))),
                "{error:?}"
            );
            report.foreign_owner_refused = true;

            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |source, _, roots, _, budget| {
                        assert_eq!(roots.len(), 1);
                        let root = &roots[0];
                        assert!(!root.arguments.is_empty());
                        let changed = [AbiRoot {
                            kernel_binding: root.kernel_binding,
                            export: root.export,
                            arguments: &root.arguments[..root.arguments.len() - 1],
                            explicit_argument_bytes: root.explicit_argument_bytes,
                            kernarg_alignment_bytes: root.kernarg_alignment_bytes,
                        }];
                        source.require_kernel_argument_abi_v18(
                            AbiInput { roots: &changed },
                            budget,
                        )?;
                        panic!("incomplete original ABI reached the continuation")
                    },
                ),
            );
            assert!(
                matches!(error, Error::Source(SourceError::Binding(_))),
                "{error:?}"
            );
            report.incomplete_abi_refused = true;

            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |_, handoff, _, _, _| {
                        let mut work =
                            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                        let foreign = Budget::new(&mut work, 20_000_000);
                        handoff.output(&foreign)?;
                        panic!("foreign ledger acquired output custody")
                    },
                ),
            );
            assert!(
                matches!(
                    error,
                    Error::Source(SourceError::Resource(ResourceError::Accounting))
                ),
                "{error:?}"
            );
            report.foreign_ledger_refused = true;

            let called = std::cell::Cell::new(false);
            let error = refused(
                transaction()?.with_original_source_integer_custody_v18::<(), _>(
                    |_, _, _, _, _| {
                        called.set(true);
                        Err(Error::Unsupported("original current callback refusal"))
                    },
                ),
            );
            assert!(called.get());
            assert!(matches!(
                error,
                Error::Unsupported("original current callback refusal")
            ));
            report.callback_error_preserved = true;

            fn peak<'view, 'source>(
                _: &'view fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'source>,
                _: &IntegerHandoff<'view, 'source>,
                _: &[AbiRoot<'_>],
                _: fe2o3_amd_target::ProductionAmdTargetProfileV1,
                budget: &mut Budget<'_>,
            ) -> Result<usize, Error> {
                Ok(budget.peak_storage())
            }
            // Identical function item and result type keep every callback and
            // continuation header unchanged across measurement/exact/short.
            let measured = transaction()?
                .with_original_source_integer_test_limits_v18(20_000_000, peak)
                .map_err(|error| format!("original storage measurement: {error:?}"))?
                .into_observation();
            let exact = transaction()?
                .with_original_source_integer_test_limits_v18(measured, peak)
                .map_err(|error| format!("original exact storage: {error:?}"))?
                .into_observation();
            assert_eq!(exact, measured);
            let error = refused(
                transaction()?.with_original_source_integer_test_limits_v18(measured - 1, peak),
            );
            let mut cause: &(dyn std::error::Error + 'static) = &error;
            loop {
                if let Some(ResourceError::Storage(storage)) = cause.downcast_ref::<ResourceError>()
                {
                    assert_eq!(storage.actual(), measured);
                    assert_eq!(storage.limit(), measured - 1);
                    break;
                }
                cause = cause
                    .source()
                    .unwrap_or_else(|| panic!("not exact Storage refusal: {error:?}"));
            }
            report.exact_and_short_storage = true;
            Ok(report)
        })());
        Compilation::Stop
    }
}

#[derive(Default)]
struct MixedCallbacks {
    result: Option<Result<bool, String>>,
}

impl Callbacks for MixedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            start_preparation_observation_v29();
            let called = std::cell::Cell::new(false);
            let error = refused(
                transaction.with_original_source_integer_custody_v18::<(), _>(|_, _, _, _, _| {
                    called.set(true);
                    Ok(())
                }),
            );
            assert!(!called.get());
            assert!(take_preparation_observation_v29().is_none());
            match error {
                Error::Pipeline(error) => match *error {
                    crate::production_pipeline::ProductionPipelineError::SemanticImport(
                        crate::collector::ProductionSemanticImportErrorV1::SemanticSchema(
                            SemanticMirErrorV1::WireVersionCannotRepresent {
                                requested: SemanticMirWireVersionV1::V35,
                                required: SemanticMirWireVersionV1::V30,
                            },
                        ),
                    ) => Ok(true),
                    other => Err(format!(
                        "not exact nominal/saturation schema refusal: {other:?}"
                    )),
                },
                other => Err(format!("not original import refusal: {other:?}")),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn original_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = OriginalCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("original-source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("original-source result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "original source: {result:?}");
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn original_source_mixed_refusal_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = MixedCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("mixed-source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("mixed-source result"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "mixed source: {result:?}");
}

fn original_program(body: &str) -> String {
    format!(
        "use fe2o3_device::kernel;\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\n{body}\n"
    )
}

fn check_original(body: &str, version: u16, nominal: bool, saturation: bool) {
    run_actual_sources::<OriginalObservation>(
        &[("original", body)],
        &[(0, 0)],
        ORIGINAL_CHILD,
        "ORIGINAL_SOURCE_V18",
        original_program,
        |_, _, _, report, _| {
            assert_eq!(report.version, version);
            assert_eq!(report.nominal_usize_types > 0, nominal);
            assert_eq!(report.nominal_isize_types > 0, nominal);
            assert_eq!(report.nominal_usize_arguments, usize::from(nominal));
            assert_eq!(report.nominal_isize_arguments, usize::from(nominal));
            assert_eq!(report.saturation_calls > 0, saturation);
            assert!(report.prepared && report.materialized);
            assert!(report.foreign_owner_refused && report.incomplete_abi_refused);
            assert!(report.foreign_ledger_refused && report.callback_error_preserved);
            assert!(report.exact_and_short_storage);
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_current_original_source_retains_v5_and_owner_custody() {
    check_original(
        "pub fn original_plain(_wide: u64, _signed: i64) {}",
        5,
        false,
        false,
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_nominal_original_source_retains_usize_isize_through_v18() {
    check_original(
        "pub fn original_nominal(_wide: usize, _signed: isize) {}",
        35,
        true,
        false,
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_saturating_original_source_retains_current_sibling_v30() {
    check_original(
        "pub fn original_saturation(left: u32, right: u32) { let _value = left.saturating_add(right); }",
        30,
        false,
        true,
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_mixed_nominal_saturation_refuses_without_erasing_source_types() {
    run_actual_sources::<bool>(
        &[(
            "mixed",
            "pub fn original_mixed(_word: usize, left: u32, right: u32) { let _value = left.saturating_add(right); }",
        )],
        &[(0, 0)],
        MIXED_CHILD,
        "ORIGINAL_SOURCE_MIXED_REFUSAL_V18",
        original_program,
        |_, _, _, refused, _| assert!(refused),
    );
}

#[test]
fn original_source_actual_child_identities_are_distinct_and_exact() {
    assert_ne!(ORIGINAL_CHILD, MIXED_CHILD);
    assert!(ORIGINAL_CHILD.ends_with("::original_source_tests::original_source_child"));
    assert!(MIXED_CHILD.ends_with("::original_source_tests::original_source_mixed_refusal_child"));
}

#[test]
fn original_source_program_preserves_supplied_nominal_spelling() {
    let body = "pub fn original_nominal(_wide: usize, _signed: isize) {}";
    let program = original_program(body);
    assert_eq!(program.matches(body).count(), 1);
    assert_eq!(program.matches("#[kernel(").count(), 1);
    assert!(!program.contains("u64"));
    assert!(!program.contains("i64"));
}

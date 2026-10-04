//! Actual original Rust CFG, same-owner integer output, native V18 target text.
use super::*;

const CFG_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::target_llvm_tests::scalar_cfg_tests::scalar_cfg_target_child";
const CFG_REFUSAL_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::source_owned_tests::target_llvm_tests::scalar_cfg_tests::scalar_cfg_dynamic_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct CfgObservation {
    source: [u8; 32],
    original: [u8; 32],
    optimized: [u8; 32],
    optimized_identity: [u8; 32],
    optimized_bytes: usize,
    original_cfg: [usize; 3],
    optimized_cfg: [usize; 3],
    changed: bool,
    target: String,
    text: String,
    work: usize,
    peak: usize,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum CfgOutcome {
    Llvm(CfgObservation),
    Incomplete,
}

// Independent test oracle over the actual borrowed graph. The existing CFG
// engine has its own limits; these facts are not final authority or ledger credit.
fn cfg_facts(owner: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18) -> [usize; 3] {
    let mut result = [0; 3];
    for function in &owner.module().functions {
        let body = function.body.as_ref().unwrap();
        let cfg = fe2o3_kernel_ir::analyze_control_flow(function).unwrap();
        result[0] += cfg.block_count();
        result[1] += body
            .blocks
            .iter()
            .filter(|block| cfg.incoming_edges(block.id).unwrap().len() > 1)
            .count();
        result[2] += (0..cfg.edge_count())
            .filter(|&edge| {
                let source = cfg.edge_source(edge).unwrap();
                let target = cfg.edge_target(edge).unwrap();
                cfg.is_reachable(source) && cfg.dominates(target, source)
            })
            .count();
    }
    result
}

fn has_incomplete(mut error: &(dyn std::error::Error + 'static)) -> bool {
    for _ in 0..32 {
        if let Some(fe2o3_lower_mir_kernel::ProductionScalarCfgCheckErrorV18::FormalIncomplete(
            reasons,
        )) = error.downcast_ref::<fe2o3_lower_mir_kernel::ProductionScalarCfgCheckErrorV18>()
        {
            return reasons.as_slice()
                == [fe2o3_kernel_ir::FormalMemoryIncompleteReason::LaunchExtentUnknown];
        }
        let Some(next) = error.source() else {
            return false;
        };
        error = next;
    }
    panic!("cyclic error chain")
}

#[test]
fn scalar_cfg_dynamic_refusal_oracle_requires_exact_launch_reason() {
    use fe2o3_kernel_ir::{FormalIndexWidth, FormalMemoryIncompleteReason as Reason};
    use fe2o3_lower_mir_kernel::ProductionScalarCfgCheckErrorV18 as Check;
    assert!(has_incomplete(&Check::FormalIncomplete(vec![
        Reason::LaunchExtentUnknown
    ])));
    for reasons in [
        vec![],
        vec![Reason::UnsupportedIndexWidth {
            width: FormalIndexWidth::Bits32,
        }],
        vec![
            Reason::LaunchExtentUnknown,
            Reason::UnsupportedIndexWidth {
                width: FormalIndexWidth::Bits32,
            },
        ],
        vec![Reason::LaunchExtentUnknown, Reason::LaunchExtentUnknown],
    ] {
        assert!(!has_incomplete(&Check::FormalIncomplete(reasons)));
    }
    assert!(!has_incomplete(&Check::Incomplete(
        "unrelated native obligation"
    )));
}

struct CfgCallbacks {
    dynamic: bool,
    result: Option<Result<CfgOutcome, String>>,
}

impl Callbacks for CfgCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            if self.dynamic {
                let calls = Cell::new(0);
                let error = transaction()?
                    .with_original_source_scalar_cfg_target_llvm_v18::<(), _>(|_, _, _, _| {
                        calls.set(calls.get() + 1);
                        Ok(())
                    })
                    .unwrap_err();
                assert!(
                    has_incomplete(&error),
                    "must preserve actual formal incompleteness: {error:?}"
                );
                assert_eq!(calls.get(), 0);
                return Ok(CfgOutcome::Incomplete);
            }
            let expected = crate::production_target_v1::RetainedProductionTargetV1::authenticate_live_before_collection(tcx)
                .and_then(|target| target.authenticate_import_session(tcx))
                .map_err(|error| format!("actual target: {error:?}"))?.profile();
            let foreign = transaction()?
                .original_source_ssa_for_test_v18()
                .map_err(|error| format!("independent original capture: {error:?}"))?;
            let observation = transaction()?
                .with_original_source_scalar_cfg_target_llvm_v18(
                    |source, handoff, native, budget| {
                        let original = source.canonical(budget)?;
                        let output = handoff.output(budget)?;
                        assert_eq!(
                            source.source_ssa(budget)?.source_semantic_sha256(),
                            foreign.source_semantic_sha256()
                        );
                        assert_eq!(
                            source.source_semantic(budget)?.wire_version(),
                            foreign.source_semantic().wire_version()
                        );
                        assert!(!std::ptr::eq(source.source_ssa(budget)?, &foreign));
                        handoff.check_original_source(source.source_ssa(budget)?, budget)?;
                        assert_eq!(output.input_audit_bytes(), original.canonical_bytes());
                        assert_eq!(output.map().output_identity(), output.owner().identity());
                        assert!(
                            !output.grants_authority() && !output.execution().grants_authority()
                        );
                        assert_eq!(native.target(budget)?, expected);
                        assert_eq!(original.module().kernels.len(), 2);
                        assert_eq!(output.owner().module().kernels.len(), 2);
                        let original_cfg = cfg_facts(original);
                        let optimized_cfg = cfg_facts(output.owner());
                        assert!(original_cfg[0] > 2 && original_cfg[1] >= 2);
                        assert_eq!(
                            original_cfg, optimized_cfg,
                            "integer pass must preserve actual CFG"
                        );
                        let text = native.llvm_ir(budget)?;
                        assert!(!text.is_empty());
                        let copied = paid_text(text, budget)?;
                        let target = paid_text(expected.device_target(), budget)?;
                        Ok(CfgObservation {
                            source: *source.source_ssa(budget)?.source_semantic_sha256(),
                            original: Sha256::digest(original.canonical_bytes()).into(),
                            optimized: Sha256::digest(output.owner().canonical_bytes()).into(),
                            optimized_identity: *output.owner().identity().digest(),
                            optimized_bytes: output.owner().canonical_bytes().len(),
                            original_cfg,
                            optimized_cfg,
                            changed: original.canonical_bytes() != output.owner().canonical_bytes(),
                            target,
                            text: copied,
                            work: budget.work(),
                            peak: budget.peak_storage(),
                        })
                    },
                )
                .map_err(|error| format!("actual scalar CFG target LLVM: {error:?}"))?;

            let error = transaction()?
                .with_original_source_scalar_cfg_target_llvm_v18::<(), _>(
                    |_, handoff, native, budget| {
                        let selected = handoff.check_original_source(&foreign, budget).unwrap_err();
                        assert!(matches!(
                            selected,
                            SourceError::Binding("foreign original SSA owner")
                        ));
                        assert!(matches!(
                            native.llvm_ir(budget),
                            Err(SourceError::Binding("foreign original SSA owner"))
                        ));
                        Err(selected.into())
                    },
                )
                .unwrap_err();
            assert!(matches!(
                error,
                Error::Source(SourceError::Binding("foreign original SSA owner"))
            ));
            let error = transaction()?
                .with_original_source_scalar_cfg_target_llvm_v18::<(), _>(|_, _, native, _| {
                    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(500_000_000);
                    let foreign = Budget::new(&mut work, 20_000_000);
                    native.llvm_ir(&foreign)?;
                    panic!("foreign ledger reached target text")
                })
                .unwrap_err();
            assert!(matches!(
                error,
                Error::Source(SourceError::Resource(ResourceError::Accounting))
            ));
            let error = transaction()?
                .with_original_source_scalar_cfg_target_llvm_v18::<(), _>(|_, _, _, _| {
                    Err(Error::Unsupported("selected scalar CFG target callback"))
                })
                .unwrap_err();
            assert!(matches!(
                error,
                Error::Unsupported("selected scalar CFG target callback")
            ));
            for mode in [
                Mode::Success,
                Mode::ExactStorage,
                Mode::ShortStorage,
                Mode::WorkRefusal,
                Mode::ForeignEntry,
                Mode::RestoredFloor,
            ] {
                let checked = transaction()?.with_original_source_scalar_cfg_target_llvm_v18(
                    |source, handoff, native, budget| {
                        let target = native.target(budget)?;
                        crate::production_pipeline::source_owned_v29::target_result::tests::genuine_cfg_case(
                            source, handoff, target, budget, mode,
                        ).map_err(Error::from)
                    },
                );
                if matches!(mode, Mode::Success | Mode::ExactStorage) {
                    checked.map_err(|error| format!("actual scalar CFG {mode:?}: {error:?}"))?;
                } else {
                    let error = checked.unwrap_err();
                    let mut cause: &(dyn std::error::Error + 'static) = &error;
                    let mut resource = None;
                    for _ in 0..32 {
                        if let Some(found) = cause.downcast_ref::<ResourceError>() {
                            resource = Some(*found);
                            break;
                        }
                        let Some(next) = cause.source() else {
                            break;
                        };
                        cause = next;
                    }
                    assert!(
                        matches!(
                            (mode, resource),
                            (Mode::ShortStorage, Some(ResourceError::Storage(_)))
                                | (Mode::WorkRefusal, Some(ResourceError::Work(_)))
                                | (
                                    Mode::ForeignEntry | Mode::RestoredFloor,
                                    Some(ResourceError::Accounting)
                                )
                        ),
                        "wrong actual scalar CFG {mode:?} cause: {error:?}"
                    );
                }
            }
            Ok(CfgOutcome::Llvm(observation))
        })());
        Compilation::Stop
    }
}

fn cfg_child(dynamic: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = CfgCallbacks {
        dynamic,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual scalar CFG target callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("target result path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "actual scalar CFG target: {result:?}");
}

#[test]
#[ignore = "process helper; requires exact actual-source parent request"]
fn scalar_cfg_target_child() {
    cfg_child(false);
}

#[test]
#[ignore = "process helper; requires exact actual-source parent request"]
fn scalar_cfg_dynamic_child() {
    cfg_child(true);
}

fn check_cfg_source(body: &str, looping: bool, require_changed: bool) {
    run_actual_sources::<CfgOutcome>(
        &[("scalar_cfg", body), ("scalar_cfg", body)],
        &[(0, 0)],
        CFG_CHILD,
        "ORIGINAL_SCALAR_CFG_TARGET_LLVM",
        static_source,
        |_, _, label, result, prior| {
            let CfgOutcome::Llvm(observation) = &result else {
                panic!("actual target result required")
            };
            assert_eq!(observation.original_cfg, observation.optimized_cfg);
            assert!(observation.original_cfg[0] > 2 && observation.original_cfg[1] >= 2);
            assert_eq!(observation.original_cfg[2] > 0, looping);
            assert_eq!(
                observation.original != observation.optimized,
                observation.changed
            );
            if require_changed {
                assert!(observation.changed);
            }
            assert!(observation.work < 500_000_000 && observation.peak <= 20_000_000);
            assert!(two_root_absence_matches(
                &observation.text,
                &observation.optimized_identity,
                observation.optimized_bytes,
                &observation.target
            ));
            assert!(!two_root_absence_matches(
                &observation.text,
                &observation.optimized,
                observation.optimized_bytes,
                &observation.target
            ));
            assert!(
                observation.text.contains("br "),
                "actual CFG must remain in LLVM"
            );
            parse_and_verify_target_llvm(&observation.text);
            if let Some(previous) = prior.get(label) {
                assert_eq!(previous, &result);
            } else {
                prior.insert(label.to_owned(), result);
            }
        },
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD dependencies, actual Rust and explicit FE2O3_OPT"]
fn actual_original_branch_join_reaches_same_owner_target_llvm_on_both_targets() {
    check_cfg_source(
        "if seed == 0 { seed = 7_u32; } else { seed = 9_u32; }",
        false,
        true,
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD dependencies, actual Rust and explicit FE2O3_OPT"]
fn actual_original_finite_loop_reaches_same_owner_target_llvm_on_both_targets() {
    check_cfg_source("while seed != 0 { seed = 0_u32; }", true, false);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD dependencies and actual Rust"]
fn actual_original_cfg_dynamic_launch_preserves_incomplete_formal_refusal() {
    run_actual_sources::<CfgOutcome>(
        &[(
            "dynamic_cfg",
            "if seed == 0 { seed = 7_u32; } else { seed = 9_u32; }",
        )],
        &[(0, 0)],
        CFG_REFUSAL_CHILD,
        "ORIGINAL_SCALAR_CFG_DYNAMIC_REFUSAL",
        scalar_source,
        |_, _, _, result, _| assert_eq!(result, CfgOutcome::Incomplete),
    );
}

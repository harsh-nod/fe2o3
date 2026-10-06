//! Ordinary rustc entry through the owner-bound expanded production stage.
use super::*;
use crate::production_pipeline::source_owned_v29::Error as SourceError;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, ExecutionTileLayoutV1};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_source_child";
const CASE: &str = "FE2O3_TEST_EXPANDED_SOURCE_CASE_V259";

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
enum ExpandedCase {
    Observe,
    Payload,
    Refuse,
    Overreport,
    Underreport,
    Refund,
    Panic,
}

const CASES: [ExpandedCase; 7] = [
    ExpandedCase::Observe,
    ExpandedCase::Payload,
    ExpandedCase::Refuse,
    ExpandedCase::Overreport,
    ExpandedCase::Underreport,
    ExpandedCase::Refund,
    ExpandedCase::Panic,
];

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ExpandedObservation {
    original: [u8; 32],
    neutral: [u8; 32],
    expanded: [u8; 32],
    definitions: usize,
    refused_consumer: bool,
}

struct ExpandedCallbacks {
    case: ExpandedCase,
    result: Option<Result<Option<ExpandedObservation>, String>>,
}

#[derive(Default)]
struct MultiRootCallbacks {
    result: Option<Result<bool, String>>,
}

impl Callbacks for MultiRootCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let mut calls = 0;
            let refused = transaction.with_original_source_expanded_v259::<(), _>(
                &mut budget,
                |_, _, _, _, _, _| {
                    calls += 1;
                    Ok(((), 0))
                },
            );
            assert_eq!(calls, 0);
            match refused {
                Err(SourceError::Unsupported(
                    "expanded production selection requires one complete kernel root",
                )) => Ok(true),
                Err(error) => Err(format!(
                    "multi-root refused at a different boundary: {error:?}"
                )),
                Ok(_) => Err("multi-root selection silently omitted a root".into()),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn expanded_multi_root_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = MultiRootCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("ordinary multi-root rustc callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "expanded multi-root refusal: {result:?}");
}

impl Callbacks for ExpandedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            if matches!(self.case, ExpandedCase::Observe) {
                let observation = transaction
                    .with_original_source_expanded_v259(
                        &mut budget,
                        |source, original, tile, roots, target, budget| {
                            assert_eq!(roots.len(), 1);
                            assert_eq!(source.root_count(budget)?, 1);
                            assert!(matches!(
                                target,
                                fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942
                                    | fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950
                            ));
                            assert!(std::ptr::eq(original, tile.original_source_v162(budget)?));
                            assert_eq!(tile.selections(budget)?.len(), 1);
                            assert_eq!(
                                tile.selections(budget)?[0].layout,
                                ExecutionTileLayoutV1::Blocked
                            );
                            let neutral = tile.neutral_source_v162(budget)?;
                            let observation = ExpandedObservation {
                                original: *source.canonical(budget)?.identity().digest(),
                                neutral: *neutral.output_inventory(budget)?.identity_v18().digest(),
                                expanded: *tile.output(budget)?.identity().digest(),
                                definitions: neutral.output_inventory(budget)?.definitions().len(),
                                refused_consumer: false,
                            };
                            Ok((observation, 0))
                        },
                    )
                    .map_err(|error| format!("ordinary expanded source: {error:?}"))?
                    .into_observation();
                return Ok(Some(observation));
            }
            if matches!(self.case, ExpandedCase::Payload) {
                let payload = transaction
                    .with_original_source_expanded_v259(&mut budget, |_, _, _, _, _, budget| {
                        budget.reserve_storage(17)?;
                        Ok((Box::new([9u8; 17]), 17))
                    })
                    .map_err(|error| format!("expanded owned payload: {error:?}"))?
                    .into_observation();
                assert_eq!(*payload, [9u8; 17]);
                drop(payload);
                budget.release_storage(17).unwrap();
                return Ok(None);
            }
            if matches!(self.case, ExpandedCase::Refuse) {
                let mut calls = 0;
                let refusal = transaction.with_original_source_expanded_v259::<(), _>(
                    &mut budget,
                    |_, _, _, _, _, _| {
                        calls += 1;
                        Err(SourceError::Unsupported("expanded proof is not admitted"))
                    },
                );
                assert!(refusal.is_err());
                assert_eq!(calls, 1, "the genuine owner must precede consumer refusal");
                return Ok(None);
            }
            let accounting = match self.case {
                ExpandedCase::Overreport => Some((0, 1, false)),
                ExpandedCase::Underreport => Some((1, 0, false)),
                ExpandedCase::Refund => Some((0, 0, true)),
                _ => None,
            };
            if let Some((reserve, report, refund)) = accounting {
                let mut called = false;
                let refused = transaction.with_original_source_expanded_v259::<(), _>(
                    &mut budget,
                    |_, _, _, _, _, budget| {
                        called = true;
                        if refund {
                            budget.release_storage(1)?;
                        } else {
                            budget.reserve_storage(reserve)?;
                        }
                        Ok(((), report))
                    },
                );
                assert!(called);
                assert!(refused.is_err(), "misreported or refunded owner credit");
                return Ok(None);
            }
            assert!(matches!(self.case, ExpandedCase::Panic));
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                transaction
                    .with_original_source_expanded_v259::<(), _>(&mut budget, |_, _, _, _, _, _| {
                        std::panic::panic_any(259usize)
                    })
            }))
            .err()
            .expect("expanded consumer unwind must propagate");
            assert_eq!(*panic.downcast::<usize>().unwrap(), 259);
            Ok(None)
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn expanded_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    if let Some(case) = env::var_os(CASE) {
        let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let mut callbacks = ExpandedCallbacks {
            case: serde_json::from_str(case.to_str().unwrap()).unwrap(),
            result: None,
        };
        rustc_driver::run_compiler(&args, &mut callbacks);
        let result = callbacks.result.expect("ordinary expanded rustc callback");
        std::fs::write(
            env::var_os(RESULT).unwrap(),
            serde_json::to_vec(&result).unwrap(),
        )
        .unwrap();
        assert!(result.is_ok(), "expanded source: {result:?}");
        return;
    }
    // Producer MIR is one-shot custody. Each independent accounting case must
    // start a fresh rustc session rather than reacquire the consumed producer.
    let response = PathBuf::from(env::var_os(RESULT).unwrap());
    let mut observation = None;
    for (ordinal, case) in CASES.into_iter().enumerate() {
        let case_response = response.with_extension(format!("case-{ordinal}.json"));
        assert!(!case_response.exists());
        let child = Command::new(env::current_exe().unwrap())
            .args(["--exact", CHILD, "--ignored", "--nocapture"])
            .env(CASE, serde_json::to_string(&case).unwrap())
            .env(RESULT, &case_response)
            .output()
            .unwrap();
        assert!(
            child.status.success(),
            "expanded case {case:?}: {}\n{}",
            String::from_utf8_lossy(&child.stdout),
            String::from_utf8_lossy(&child.stderr)
        );
        let result: Result<Option<ExpandedObservation>, String> =
            serde_json::from_slice(&std::fs::read(&case_response).unwrap()).unwrap();
        let result = result.unwrap();
        assert_eq!(result.is_some(), matches!(case, ExpandedCase::Observe));
        if let Some(value) = result {
            assert!(observation.replace(value).is_none());
        }
    }
    let mut observation = observation.unwrap();
    observation.refused_consumer = true;
    let result: Result<_, String> = Ok(observation);
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "expanded source: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rustc_source_retains_original_neutral_and_expanded_owners() {
    run_actual_sources::<ExpandedObservation>(
        &[("tile", ""), ("tile", "")],
        &[(0, 0), (3, 0)],
        CHILD,
        "EXPANDED_SOURCE_V259",
        |_| {
            r#"use fe2o3_device::{kernel, KernelContext};
use fe2o3_device::tile::MaskedTile1D;
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn expanded_probe(mut ctx: KernelContext<'_>, input: &[u32], base: u64) {
    let _ = ctx.with_workgroup(|wg| {
        let tile = MaskedTile1D::<u32, 64, 2, _>::load_masked(&wg, input, base as usize);
        let ([a, b], [active_a, active_b]) = tile.into_fragment().into_parts();
        if active_a && active_b { a.wrapping_add(b) } else { 0 }
    });
}
"#
            .to_owned()
        },
        |_, _, label, observation, observations| {
            assert!(observation.refused_consumer);
            assert!(observation.definitions > 0);
            assert_ne!(observation.original, [0; 32]);
            assert_ne!(observation.neutral, [0; 32]);
            assert_ne!(observation.expanded, observation.neutral);
            if let Some(previous) = observations.get(label) {
                assert_eq!(&observation, previous);
            } else {
                observations.insert(label.to_owned(), observation);
            }
        },
    );
    run_actual_sources::<bool>(
        &[("multiple", "")],
        &[(0, 0)],
        &CHILD.replace("expanded_source_child", "expanded_multi_root_child"),
        "EXPANDED_MULTI_ROOT_V259",
        |_| {
            r#"use fe2o3_device::{kernel, KernelContext};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn first(_ctx: KernelContext<'_>, _seed: u32) {}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn second(_ctx: KernelContext<'_>, _seed: u32) {}
"#
            .to_owned()
        },
        |_, _, _, refused, _| assert!(refused),
    );
}

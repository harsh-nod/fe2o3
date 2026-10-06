//! Ordinary rustc entry through the owner-bound expanded production stage.
use super::*;
use crate::collector::CollectedTileTerminalKindV259 as TileKind;
use crate::production_pipeline::source_owned_v29::Error as SourceError;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, ExecutionTileLayoutV1};

#[path = "production_rustc_driver_expanded_roots_v267_tests.rs"]
mod expanded_roots_tests;
#[path = "production_rustc_driver_tile_census_v259_tests.rs"]
mod tile_census_tests;

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_source_child";
const CASE: &str = "FE2O3_TEST_EXPANDED_SOURCE_CASE_V259";
const TILE_KINDS: [TileKind; 3] = [
    TileKind::MaskedLoad,
    TileKind::IntoFragment,
    TileKind::IntoParts,
];

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
    collected_tile_calls: [u64; 3],
    refused_consumer: bool,
}

struct ExpandedCallbacks {
    case: ExpandedCase,
    result: Option<Result<Option<ExpandedObservation>, String>>,
}

fn empty_reference_input_identity() -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let domain = b"FE2O3/EXPANDED-PAIR/REFERENCE-INPUTS/V279\0";
    let mut hash = Sha256::new();
    hash.update((domain.len() as u64).to_le_bytes());
    hash.update(domain);
    hash.update(8u64.to_le_bytes());
    hash.update(0u64.to_le_bytes());
    hash.finalize().into()
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
            let census = transaction
                .collected_tile_terminals_v259(&mut budget)
                .map_err(|error| format!("tile collected census: {error:?}"))?;
            assert!(census.has_tile_operations());
            let collected_tile_calls = TILE_KINDS.map(|kind| census.call_occurrences(kind));
            assert!(collected_tile_calls.into_iter().all(|count| count > 0));
            if matches!(self.case, ExpandedCase::Observe) {
                let observation = transaction
                    .with_original_source_expanded_v259(
                        &mut budget,
                        |source, original, tile, roots, target, pair, budget| {
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
                            pair.check(source, original, tile, budget)?;
                            let subject = pair.subject(budget)?;
                            let ssa = source.source_ssa(budget)?;
                            assert_eq!(subject.semantic, *ssa.source_semantic_sha256());
                            assert_eq!(subject.ssa, *ssa.identity().as_bytes());
                            assert_eq!(pair.reference_count(budget)?, 0);
                            assert_eq!(subject.references, empty_reference_input_identity());
                            assert_eq!(subject.graphs[0], *source.canonical(budget)?.identity());
                            assert_eq!(
                                subject.graphs[1],
                                *neutral.output_inventory(budget)?.owner().identity()
                            );
                            assert_eq!(subject.graphs[2], *tile.output(budget)?.identity());
                            assert_eq!(pair.roots(budget)?.len(), 1);
                            let row = pair.roots(budget)?[0];
                            let retained = source.source_launch(budget)?.roots()[0];
                            assert_eq!(row.source_launch, retained.source_launch());
                            assert_eq!(row.source_layout, retained.layout());
                            assert_eq!(row.source_launch.max_grid(), [u32::MAX, 1, 1]);
                            assert_eq!(row.source_layout.global_extents(), [0, 1, 1]);
                            assert_eq!(
                                row.launch,
                                fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
                                    rank: 1,
                                    extents: [64 * u64::from(u32::MAX), 1, 1],
                                }
                            );
                            assert_eq!(
                                pair.runtime(budget)?,
                                (
                                    fe2o3_amdgcn_model::production_logical_index_width_v19(),
                                    fe2o3_kernel_ir::EndiannessV2::Little,
                                )
                            );
                            let foreign = neutral.prepare_tile_expansion_with_layout_v260(
                                ExecutionTileLayoutV1::Blocked,
                                budget,
                            )?;
                            assert_eq!(
                                foreign.output(budget)?.identity(),
                                tile.output(budget)?.identity()
                            );
                            assert!(matches!(
                                pair.check(source, original, &foreign, budget),
                                Err(SourceError::Unsupported(
                                    "expanded pair input owner or runtime differs"
                                ))
                            ));
                            foreign.discard(budget)?;
                            let observation = ExpandedObservation {
                                original: *source.canonical(budget)?.identity().digest(),
                                neutral: *neutral.output_inventory(budget)?.identity_v18().digest(),
                                expanded: *tile.output(budget)?.identity().digest(),
                                definitions: neutral.output_inventory(budget)?.definitions().len(),
                                collected_tile_calls,
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
                    .with_original_source_expanded_v259(&mut budget, |_, _, _, _, _, _, budget| {
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
                    |_, _, _, _, _, _, _| {
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
                    |_, _, _, _, _, _, budget| {
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
                transaction.with_original_source_expanded_v259::<(), _>(
                    &mut budget,
                    |_, _, _, _, _, _, _| std::panic::panic_any(259usize),
                )
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
fn expanded_source_case_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let case = env::var(CASE).expect("the parent selects one explicit case");
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ExpandedCallbacks {
        case: serde_json::from_str(&case).unwrap(),
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
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn expanded_source_child() {
    if env::var_os(ARGS).is_none() {
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
            .args([
                "--exact",
                &CHILD.replace("expanded_source_child", "expanded_source_case_child"),
                "--ignored",
                "--nocapture",
            ])
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
    expanded_roots_tests::run_actual_root_matrix();
}

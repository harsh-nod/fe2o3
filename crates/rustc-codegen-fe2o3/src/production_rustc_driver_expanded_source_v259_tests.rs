//! Ordinary rustc entry through the owner-bound expanded production stage.
use super::*;
use crate::production_pipeline::source_owned_v29::Error as SourceError;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, ExecutionTileLayoutV1};

const CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_source_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct ExpandedObservation {
    original: [u8; 32],
    neutral: [u8; 32],
    expanded: [u8; 32],
    definitions: usize,
    refused_consumer: bool,
}

#[derive(Default)]
struct ExpandedCallbacks {
    result: Option<Result<ExpandedObservation, String>>,
}

impl Callbacks for ExpandedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = || {
                transaction_in_active_session_v1(
                    tcx,
                    crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
                )
            };
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let mut observation = transaction()?
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
            let mut calls = 0;
            let refusal = transaction()?.with_original_source_expanded_v259::<(), _>(
                &mut budget,
                |_, _, _, _, _, _| {
                    calls += 1;
                    Err(SourceError::Unsupported("expanded proof is not admitted"))
                },
            );
            assert!(refusal.is_err());
            assert_eq!(calls, 1, "the genuine owner must precede consumer refusal");
            observation.refused_consumer = true;
            Ok(observation)
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
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ExpandedCallbacks::default();
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
}

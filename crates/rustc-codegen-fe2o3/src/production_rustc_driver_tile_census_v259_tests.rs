//! Collection-only observations; shared tile helpers are not expansion admission.
use super::*;

const CENSUS_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::tile_census_tests::collected_tile_census_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct CensusObservation {
    roots: usize,
    calls: [u64; 3],
    has_tile_operations: bool,
}

#[derive(Default)]
struct CensusCallbacks {
    result: Option<Result<CensusObservation, String>>,
}

impl Callbacks for CensusCallbacks {
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
                .map_err(|error| format!("collected terminal census: {error:?}"))?;
            Ok(CensusObservation {
                roots: transaction.collected_root_count_for_test_v259(),
                calls: TILE_KINDS.map(|kind| census.call_occurrences(kind)),
                has_tile_operations: census.has_tile_operations(),
            })
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn collected_tile_census_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = CensusCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks.result.expect("ordinary collected rustc callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "collected census: {result:?}");
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_rustc_collected_tile_census_preserves_shared_body_counts() {
    run_actual_sources::<CensusObservation>(
        &[
            ("scalar-shared", "scalar"),
            ("tile-shared", "one"),
            ("tile-shared", "two"),
        ],
        &[(0, 0), (3, 0)],
        CENSUS_CHILD,
        "COLLECTED_TILE_CENSUS_V259",
        |case| {
            if case == "scalar" {
                return r#"use fe2o3_device::{kernel, KernelContext};
#[inline(never)]
fn shared_scalar(seed: u32) -> u32 { seed.wrapping_add(1) }
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn first(_ctx: KernelContext<'_>, seed: u32) { let _ = shared_scalar(seed); }
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn second(_ctx: KernelContext<'_>, seed: u32) { let _ = shared_scalar(seed); }
"#
                .to_owned();
            }
            assert!(matches!(case, "one" | "two"));
            let mut source = r#"use fe2o3_device::{kernel, KernelContext};
use fe2o3_device::tile::MaskedTile1D;
#[inline(never)]
fn shared_tile(mut ctx: KernelContext<'_>, input: &[u32], base: u64) {
    let _ = ctx.with_workgroup(|wg| {
        let tile = MaskedTile1D::<u32, 64, 2, _>::load_masked(&wg, input, base as usize);
        let ([a, b], [active_a, active_b]) = tile.into_fragment().into_parts();
        if active_a && active_b { a.wrapping_add(b) } else { 0 }
    });
}
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn first(ctx: KernelContext<'_>, input: &[u32], base: u64) {
    shared_tile(ctx, input, base);
}
"#
            .to_owned();
            if case == "two" {
                source.push_str(
                    r#"#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn second(ctx: KernelContext<'_>, input: &[u32], base: u64) {
    shared_tile(ctx, input, base);
}
"#,
                );
            }
            source
        },
        |_, _, label, observation, observations| {
            if label == "scalar-shared" {
                assert_eq!(observation.roots, 2);
                assert_eq!(observation.calls, [0; 3]);
                assert!(!observation.has_tile_operations);
            } else {
                assert_eq!(label, "tile-shared");
                assert!(observation.has_tile_operations);
                assert!(observation.calls.into_iter().all(|count| count > 0));
                if let Some(single_root) = observations.get(label) {
                    assert_eq!(single_root.roots, 1);
                    assert_eq!(observation.roots, 2);
                    assert_eq!(observation.calls, single_root.calls);
                } else {
                    assert_eq!(observation.roots, 1);
                    observations.insert(label.to_owned(), observation);
                }
            }
        },
    );
}

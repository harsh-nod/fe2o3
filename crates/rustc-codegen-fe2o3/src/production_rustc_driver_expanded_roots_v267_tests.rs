//! One ordinary source owner and one expansion retain the complete root module.
use super::*;
use fe2o3_kernel_ir::OperationKind;

const ROOTS_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_roots_tests::expanded_roots_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct RootsObservation {
    original: [u8; 32],
    neutral: [u8; 32],
    expanded: [u8; 32],
    roots: usize,
    selected: usize,
    helpers: usize,
    collected_tile_calls: [u64; 3],
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum RootsOutcome {
    Expanded(RootsObservation),
    TileHelperRefused,
}

#[derive(Default)]
struct RootsCallbacks {
    result: Option<Result<RootsOutcome, String>>,
}

impl Callbacks for RootsCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
            let transaction = transaction_in_active_session_v1(
                tcx,
                crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
            )?;
            let mut work = Work::new(500_000_000);
            let mut budget = Budget::new(&mut work, 20_000_000);
            let collected_roots = transaction.collected_root_count_for_test_v259();
            assert_eq!(collected_roots, 2);
            let census = transaction
                .collected_tile_terminals_v259(&mut budget)
                .map_err(|error| format!("multi-root census: {error:?}"))?;
            let collected_tile_calls = TILE_KINDS.map(|kind| census.call_occurrences(kind));
            let mut calls = 0;
            let result = transaction.with_original_source_expanded_v259(
                &mut budget,
                |source, original, tile, roots, _, budget| {
                    calls += 1;
                    assert_eq!(roots.len(), collected_roots);
                    assert_eq!(source.root_count(budget)?, collected_roots);
                    assert!(std::ptr::eq(original, tile.original_source_v162(budget)?));
                    let neutral = tile.neutral_source_v162(budget)?;
                    assert!(std::ptr::eq(source, neutral.original_source(budget)?));
                    let input = neutral.output_inventory(budget)?;
                    let output = tile.output(budget)?;
                    assert_eq!(input.owner().module().kernels, output.module().kernels);
                    assert_eq!(input.functions().len(), output.module().functions.len());
                    let selections = tile.selections(budget)?;
                    let mut selected = 0;
                    let cfg_header = std::mem::size_of::<
                        fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>,
                    >();
                    budget.reserve_storage(cfg_header)?;
                    for (root, abi) in roots.iter().enumerate() {
                        assert!(!abi.export.is_empty());
                        assert_ne!(abi.kernel_binding, &[0; 32]);
                        let cfg = neutral.output_root_cfg_v18(root, budget)?;
                        assert_eq!(cfg.root(), root);
                        assert!(
                            cfg.function()
                                .function
                                .body
                                .as_ref()
                                .unwrap()
                                .blocks
                                .iter()
                                .flat_map(|block| &block.operations)
                                .any(|operation| matches!(&operation.kind,
                                OperationKind::Call { callee, .. }
                                if callee.as_str().contains("shared_scalar"))),
                            "every authentic root calls the retained shared helper"
                        );
                        let policy = tile.root_policy_v162(root, budget)?;
                        let selection = selections
                            .iter()
                            .find(|row| row.function == cfg.function().coordinate);
                        match (policy, selection) {
                            (Some((function, layout, lanes)), Some(selection)) => {
                                assert_eq!(function, cfg.function().coordinate);
                                assert_eq!(selection.layout, layout);
                                assert_eq!((layout, lanes), (ExecutionTileLayoutV1::Blocked, 64));
                                selected += 1;
                            }
                            (None, None) => {}
                            _ => panic!("original root and whole-module policy disagree"),
                        }
                    }
                    budget.release_storage(cfg_header)?;
                    assert_eq!(selected, selections.len());
                    let mut helpers = 0;
                    for (index, function) in input.functions().iter().enumerate() {
                        assert_eq!(output.module().functions[index].id, function.function.id);
                        if !selections
                            .iter()
                            .any(|row| row.function == function.coordinate)
                        {
                            assert_eq!(&output.module().functions[index], function.function);
                        }
                        if function.function.id.as_str().contains("shared_scalar") {
                            helpers += 1;
                            let calls_to_helper = |caller: &fe2o3_kernel_ir::Function| {
                                caller
                                    .body
                                    .as_ref()
                                    .into_iter()
                                    .flat_map(|body| &body.blocks)
                                    .flat_map(|block| &block.operations)
                                    .filter(|operation| {
                                        matches!(&operation.kind,
                                        OperationKind::Call { callee, .. }
                                        if callee == &function.function.id)
                                    })
                                    .count()
                            };
                            for (before, after) in input
                                .owner()
                                .module()
                                .functions
                                .iter()
                                .zip(&output.module().functions)
                            {
                                assert_eq!(calls_to_helper(before), calls_to_helper(after));
                            }
                        }
                    }
                    assert_eq!(
                        helpers, 1,
                        "the shared scalar helper is retained exactly once"
                    );
                    assert!(!tile.grants_artifact_or_launch_authority());
                    let observation = RootsObservation {
                        original: *source.canonical(budget)?.identity().digest(),
                        neutral: *input.identity_v18().digest(),
                        expanded: *output.identity().digest(),
                        roots: roots.len(),
                        selected,
                        helpers,
                        collected_tile_calls,
                    };
                    tile.replay(budget)?;
                    Ok((observation, 0))
                },
            );
            match result {
                Ok(value) => {
                    assert_eq!(calls, 1);
                    Ok(RootsOutcome::Expanded(value.into_observation()))
                }
                Err(error) => {
                    assert_eq!(calls, 0, "refusal must precede the borrowed consumer");
                    use fe2o3_lower_mir_kernel::{
                        ProductionSourceOptimizationErrorV18 as OptimizationError,
                        ProductionSourceOwnedViewErrorV18 as OwnerError,
                    };
                    use fe2o3_pliron::KirCheckedNeutralOptimizationErrorV1 as AdoptionError;
                    if matches!(&error, SourceError::ExpandedSource(error)
                    if matches!(error.as_ref(), OptimizationError::Adoption(
                        AdoptionError::Origin(SourceError::Source(OwnerError::Binding(
                            "source tile helper requires interprocedural expansion"
                        )))
                    ))) {
                        Ok(RootsOutcome::TileHelperRefused)
                    } else {
                        Err(format!(
                            "multi-root refused at a different boundary: {error:?}"
                        ))
                    }
                }
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn expanded_roots_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = RootsCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("ordinary complete-root rustc callback");
    std::fs::write(
        env::var_os(RESULT).unwrap(),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "expanded roots: {result:?}");
}

fn root_source(case: &str) -> String {
    let mut source = r#"use fe2o3_device::{kernel, KernelContext};
use fe2o3_device::tile::MaskedTile1D;
#[inline(never)]
fn shared_scalar(seed: u32) -> u32 { seed.wrapping_add(1) }
"#
    .to_owned();
    if case == "helper" {
        source.push_str(
            r#"#[inline(never)]
fn tile_helper<Kernel>(ctx: &mut KernelContext<'_, Kernel>, input: &[u32], base: u64) {
    let _ = ctx.with_workgroup(|wg| {
        let tile = MaskedTile1D::<u32, 64, 2, _>::load_masked(&wg, input, base as usize);
        let ([a, b], [active_a, active_b]) = tile.into_fragment().into_parts();
        if active_a && active_b { a.wrapping_add(b) } else { 0 }
    });
}
"#,
        );
    }
    for (index, name) in ["first", "second"].into_iter().enumerate() {
        source.push_str("#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\n");
        source.push_str(&format!("pub fn {name}(mut ctx: KernelContext<'_>, input: &[u32], base: u64, seed: u32) {{\n    let _ = shared_scalar(seed);\n"));
        if case == "helper" {
            source.push_str("    tile_helper(&mut ctx, input, base);\n");
        } else if case == "two" || (case == "mixed" && index == 0) {
            source.push_str(
                r#"    let _ = ctx.with_workgroup(|wg| {
        let tile = MaskedTile1D::<u32, 64, 2, _>::load_masked(&wg, input, base as usize);
        let ([a, b], [active_a, active_b]) = tile.into_fragment().into_parts();
        if active_a && active_b { a.wrapping_add(b) } else { 0 }
    });
"#,
            );
        } else {
            assert!(case == "scalar" || case == "mixed");
        }
        source.push_str("}\n");
    }
    source
}

pub(super) fn run_actual_root_matrix() {
    run_actual_sources::<RootsOutcome>(
        &[
            ("two-tile", "two"),
            ("two-tile", "two"),
            ("mixed", "mixed"),
            ("scalar", "scalar"),
            ("tile-helper", "helper"),
        ],
        &[(0, 0), (3, 0)],
        ROOTS_CHILD,
        "EXPANDED_ROOTS_V267",
        root_source,
        |_, _, label, outcome, observations| {
            if label == "tile-helper" {
                assert_eq!(outcome, RootsOutcome::TileHelperRefused);
                return;
            }
            let RootsOutcome::Expanded(observation) = &outcome else {
                panic!("supported roots refused: {label}");
            };
            assert_eq!(observation.roots, 2);
            assert!(observation.helpers > 0);
            assert_eq!(
                observation.selected,
                match label {
                    "two-tile" => 2,
                    "mixed" => 1,
                    "scalar" => 0,
                    _ => unreachable!(),
                }
            );
            if label == "scalar" {
                assert_eq!(observation.collected_tile_calls, [0; 3]);
                assert_eq!(observation.expanded, observation.neutral);
            } else {
                assert!(observation.collected_tile_calls.into_iter().all(|n| n > 0));
                assert_ne!(observation.expanded, observation.neutral);
            }
            if let Some(previous) = observations.get(label) {
                assert_eq!(&outcome, previous);
            } else {
                observations.insert(label.to_owned(), outcome);
            }
        },
    );
}

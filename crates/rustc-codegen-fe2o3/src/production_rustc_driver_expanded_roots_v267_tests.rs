//! One ordinary source owner and one expansion retain the complete root module.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionSourceCorrespondenceV18, ProductionSourceOperationV18, ProductionSourceOwnedViewV18,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticFunctionIdV1, SemanticScalarTypeV1, SemanticTypeShapeV1,
};

const ROOTS_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::expanded_source_tests::expanded_roots_tests::expanded_roots_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct RootsObservation {
    original: [u8; 32],
    neutral: [u8; 32],
    expanded: [u8; 32],
    runtime_and_instances: [u8; 32],
    references: [u8; 32],
    roots: usize,
    selected: usize,
    helper_instances: [usize; 2],
    helper_mapped_operations: [usize; 2],
    collected_tile_calls: [u64; 3],
}

#[derive(Default)]
struct RootsCallbacks {
    result: Option<Result<RootsObservation, String>>,
}

pub(super) fn original_helper_instances(
    source: &ProductionSourceOwnedViewV18<'_>,
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut Budget<'_>,
) -> Result<([usize; 2], [usize; 2]), SourceError> {
    let semantic = source.source_semantic(budget)?;
    assert_ne!(source.root(0, budget)?.0, source.root(1, budget)?.0);
    assert_ne!(source.root(0, budget)?.1, source.root(1, budget)?.1);
    assert!(semantic.functions().len() < 256);
    let u32_type = |ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1| {
        matches!(
            semantic.types()[ty.index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
    };
    let mut selected = None;
    for (index, function) in semantic.functions().iter().enumerate() {
        let signature = function.abi().source_signature();
        if function.kernel_entry().is_some()
            || signature.inputs().len() != 1
            || !u32_type(signature.inputs()[0])
            || !u32_type(signature.output())
        {
            continue;
        }
        let function_id = SemanticFunctionIdV1::from_index(index.try_into().unwrap());
        let mut instances = [None; 2];
        for (root, found) in instances.iter_mut().enumerate() {
            let count = source.instance_count(root, budget)?;
            assert!(count < 256);
            for instance in 0..count {
                if source.instance(root, instance, budget)?.0 == function_id
                    && source.instance_active(root, instance, budget)?
                {
                    assert!(
                        found.replace(instance).is_none(),
                        "one helper call per root"
                    );
                }
            }
        }
        if let [Some(first), Some(second)] = instances {
            assert!(
                selected.replace((function_id, [first, second])).is_none(),
                "unique genuine shared u32 helper"
            );
        }
    }
    let (helper, instances) = selected.expect("shared original helper remains represented");
    let function = &semantic.functions()[helper.index() as usize];
    assert!(!function.blocks().is_empty());
    assert!(function.blocks().len() < 256);
    // This fixture's straight-line helper has no entry predecessors, so the
    // importer needs no synthetic invocation preheader for either instance.
    for block in function.blocks() {
        block.terminator().kind().try_for_each_edge(|edge| {
            budget.charge_work(1)?;
            assert_ne!(edge.target(), function.entry());
            Ok::<_, SourceError>(())
        })?;
    }
    let canonical = source.canonical(budget)?;
    let mut mapped_operations = [0; 2];
    for (root, instance) in instances.into_iter().enumerate() {
        let (actual, _) = source.instance(root, instance, budget)?;
        assert_eq!(actual, helper);
        let count = source.instance_count(root, budget)?;
        assert!(count < 256);
        let mut current = instance;
        let mut reached_root = false;
        for _ in 0..count {
            budget.charge_work(1)?;
            assert!(current < count);
            assert!(source.instance_active(root, current, budget)?);
            let (function, incoming) = source.instance(root, current, budget)?;
            let Some((caller, call_block)) = incoming else {
                assert_eq!(function, source.root(root, budget)?.0);
                reached_root = true;
                break;
            };
            assert!(caller < count);
            assert_ne!(caller, current);
            assert_eq!(
                original.defined_call_instance(root, caller, call_block, budget)?,
                current
            );
            current = caller;
        }
        assert!(reached_root, "helper caller chain reaches the actual root");
        assert!(source.invocation_entry(root, instance, budget)?.is_none());
        let root_function = source.root(root, budget)?.1;
        for (block, source_block) in function.blocks().iter().enumerate() {
            assert!(source_block.statements().len() < 512);
            let block = SemanticBlockIdV1::from_index(block.try_into().unwrap());
            for statement in (0..source_block.statements().len())
                .map(|i| Some(i as u32))
                .chain(std::iter::once(None))
            {
                original.visit_source_operations(
                    root,
                    instance,
                    block,
                    statement,
                    budget,
                    |operation, budget| {
                        budget.charge_work(1)?;
                        if let ProductionSourceOperationV18::Operation(coordinate) = operation {
                            assert_eq!(coordinate.block.function.0 as usize, root_function);
                            let body = canonical.module().functions[root_function]
                                .body
                                .as_ref()
                                .unwrap();
                            assert!(
                                body.blocks[coordinate.block.block as usize]
                                    .operations
                                    .get(coordinate.operation as usize)
                                    .is_some()
                            );
                            mapped_operations[root] += 1;
                        }
                        Ok(())
                    },
                )?;
            }
        }
        assert!(
            mapped_operations[root] > 0,
            "helper has genuine flattened operations"
        );
    }
    // Numeric instance ordinals are root-local; the two distinct root owners
    // above qualify their separate instances of one original helper identity.
    Ok((instances, mapped_operations))
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
                |source, original, tile, roots, _, pair, budget| {
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
                        pair.check(source, original, tile, budget)?;
                        let row = pair.roots(budget)?[root];
                        assert_eq!(row.source_root, source.root(root, budget)?.0);
                        assert_eq!(row.original_function, source.root(root, budget)?.1);
                        assert_eq!(row.instances, source.instance_count(root, budget)?);
                        let retained = source.source_launch(budget)?.roots()[root];
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
                        assert!(!abi.export.is_empty());
                        assert_ne!(abi.kernel_binding, &[0; 32]);
                        let cfg = neutral.output_root_cfg_v18(root, budget)?;
                        assert_eq!(row.target_function, cfg.function().coordinate);
                        assert_eq!(cfg.root(), root);
                        let policy = tile.root_policy_v162(root, budget)?;
                        assert_eq!(row.tile, policy.map(|(_, layout, lanes)| (layout, lanes)));
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
                    assert_eq!(selected, selections.len());
                    assert_eq!(pair.roots(budget)?.len(), roots.len());
                    let subject = pair.subject(budget)?;
                    let ssa = source.source_ssa(budget)?;
                    assert_eq!(subject.semantic, *ssa.source_semantic_sha256());
                    assert_eq!(subject.ssa, *ssa.identity().as_bytes());
                    assert_eq!(pair.reference_count(budget)?, 0);
                    assert_eq!(subject.references, empty_reference_input_identity());
                    assert_eq!(subject.graphs[0], *source.canonical(budget)?.identity());
                    assert_eq!(subject.graphs[1], *input.owner().identity());
                    assert_eq!(subject.graphs[2], *output.identity());
                    assert_eq!(
                        pair.runtime(budget)?,
                        (
                            fe2o3_amdgcn_model::production_logical_index_width_v19(),
                            fe2o3_kernel_ir::EndiannessV2::Little,
                        )
                    );
                    for (index, function) in input.functions().iter().enumerate() {
                        assert_eq!(output.module().functions[index].id, function.function.id);
                        if !selections
                            .iter()
                            .any(|row| row.function == function.coordinate)
                        {
                            assert_eq!(&output.module().functions[index], function.function);
                        }
                    }
                    budget.release_storage(cfg_header)?;
                    let (helper_instances, helper_mapped_operations) =
                        original_helper_instances(source, original, budget)?;
                    assert!(!tile.grants_artifact_or_launch_authority());
                    let observation = RootsObservation {
                        runtime_and_instances: subject.runtime_and_instances,
                        references: subject.references,
                        original: *source.canonical(budget)?.identity().digest(),
                        neutral: *input.identity_v18().digest(),
                        expanded: *output.identity().digest(),
                        roots: roots.len(),
                        selected,
                        helper_instances,
                        helper_mapped_operations,
                        collected_tile_calls,
                    };
                    tile.replay(budget)?;
                    Ok((observation, 0))
                },
            );
            match result {
                Ok(value) => {
                    assert_eq!(calls, 1);
                    Ok(value.into_observation())
                }
                Err(error) => Err(format!(
                    "ordinary multi-root source refused ({calls} consumer calls): {error:?}"
                )),
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
    root_source_with_grid(case, None)
}

pub(super) fn root_source_with_grid(case: &str, grid: Option<u32>) -> String {
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
        if let Some(grid) = grid {
            source.push_str(&format!("#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [{grid}, 1, 1]))]\n"));
        } else {
            source.push_str("#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\n");
        }
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
    run_actual_sources::<RootsObservation>(
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
            let observation = &outcome;
            assert_eq!(observation.roots, 2);
            assert!(
                observation
                    .helper_mapped_operations
                    .into_iter()
                    .all(|n| n > 0)
            );
            assert_eq!(
                observation.selected,
                match label {
                    "two-tile" | "tile-helper" => 2,
                    "mixed" => 1,
                    "scalar" => 0,
                    _ => unreachable!(),
                }
            );
            assert_eq!(
                observation.collected_tile_calls,
                [observation.selected as u64; 3]
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

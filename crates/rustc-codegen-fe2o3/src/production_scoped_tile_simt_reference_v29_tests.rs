//! Same-algorithm CPU comparison for each fixed distribution, not cross-layout
//! equivalence or default/native tile admission.
use super::*;

const BLOCKED_MIXED: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::mixed_tile_cpu_tests::simt_reference::blocked_mixed_child";
const BLOCKED_SIMT: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::mixed_tile_cpu_tests::simt_reference::blocked_simt_child";
const STRIPED_MIXED: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::mixed_tile_cpu_tests::simt_reference::striped_mixed_child";
const STRIPED_SIMT: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::mixed_tile_cpu_tests::simt_reference::striped_simt_child";

fn simt_source(order: Order) -> String {
    let offsets = match order {
        Order::Blocked => [
            "lane.wrapping_mul(3)",
            "lane.wrapping_mul(3).wrapping_add(1)",
            "lane.wrapping_mul(3).wrapping_add(2)",
        ],
        Order::Striped => ["lane", "lane.wrapping_add(64)", "lane.wrapping_add(128)"],
    };
    let mut body = String::new();
    for (name, offset, weight, bias) in [
        ("x", offsets[0], 3, 11),
        ("y", offsets[1], 5, 13),
        ("z", offsets[2], 7, 17),
    ] {
        body.push_str(&format!(
            r#"let mut {name} = 0_u32;
let mut m{name} = false;
if let Some(index) = base.checked_add({offset}) {{
    if let Some(value) = input.get(index) {{
        {name} = (*value).wrapping_mul({weight}).wrapping_add({bias});
        m{name} = true;
    }}
}}
"#
        ));
    }
    format!(
        r#"use fe2o3_device::{{kernel, thread, DisjointSlice}};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn simt_reference_probe(input: &[u32], base: usize, mut output: DisjointSlice<u32>) {{
    let lane = thread::thread_idx_x() as usize;
    {body}
    if mx || my || mz {{
        if let Some(slot) = output.get_mut(thread::index_1d()) {{
            *slot = x.wrapping_add(y).wrapping_add(z);
        }}
    }}
}}
"#
    )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Observation {
    source_cpu: u16,
    order: u8,
    mixed: bool,
    kir_version: u16,
    canonical: [u8; 32],
    outputs: [u8; 32],
    vectors: usize,
    replays: usize,
    reads: usize,
    writes: usize,
    execution_authority: bool,
}

fn observe(
    module: &sim::AdmittedSimulationModuleV1,
    source_cpu: u16,
    order: Order,
    mixed: bool,
) -> Observation {
    assert_eq!(module.module().kernels.len(), 1);
    assert!(!module.grants_execution_authority());
    assert!(
        module
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(operation.kind, kir::OperationKind::Execution(_))),
        "both CPU routes must execute scalar operations, not bypass opaque tiles"
    );
    let ((vectors, replays, reads, writes), outputs) =
        oracle::run_with_output_fingerprint(module, sim::SimulationLimitsV1::default(), order);
    Observation {
        source_cpu,
        order: order_code(order),
        mixed,
        kir_version: module.identity().wire_version(),
        canonical: *module.identity().digest(),
        outputs,
        vectors,
        replays,
        reads,
        writes,
        execution_authority: false,
    }
}

fn inspect(tcx: TyCtxt<'_>, order: Order, mixed: bool) -> Result<Observation, String> {
    let source_cpu = match tcx
        .sess
        .opts
        .cg
        .target_cpu
        .as_deref()
        .unwrap_or(tcx.sess.target.cpu.as_ref())
    {
        "gfx942" => 942,
        "gfx950" => 950,
        other => return Err(format!("unexpected actual source CPU: {other}")),
    };
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
    if !mixed {
        let bundle = transaction
            .export_simulation_bundle_v1()
            .map_err(|error| format!("genuine SIMT bundle export: {error:?}"))?;
        assert!(!bundle.grants_proof_authority());
        assert!(!bundle.grants_artifact_authority());
        assert!(!bundle.grants_compiler_authority());
        assert!(!bundle.grants_hardware_authority());
        assert!(!bundle.grants_load_authority());
        assert!(!bundle.grants_launch_authority());
        let canonical = kir::VerifiedCanonicalKernelIrV7::from_canonical_bytes(
            bundle.canonical_kir_v7().to_vec(),
        )
        .map_err(|error| format!("exact SIMT V7 owner: {error:?}"))?;
        assert_eq!(canonical.identity(), bundle.canonical_kir_v7_identity());
        let module =
            sim::AdmittedSimulationModuleV1::admit(canonical, sim::SimulationLimitsV1::default())
                .map_err(|error| format!("genuine SIMT CPU admission: {error:?}"))?;
        return Ok(observe(&module, source_cpu, order, false));
    }
    let mut observation = None;
    let outcome = transaction.observe_scoped_tile_candidate_in_order_v29(order, |view, budget| {
        assert!(observation.is_none(), "one completed mixed candidate");
        assert!(!view.grants_execution_authority());
        assert!(!view.canonical().module().storage_layouts.is_empty());
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        let resource = |error: ResourceError| PipelineError::ContextHandoff(error.into());
        let (module, receipt) =
            sim::AdmittedSimulationModuleV1::admit_v18_with_verification_budget(
                view.canonical(),
                sim::SimulationLimitsV1::default(),
                budget,
            )
            .expect("genuine mixed candidate CPU admission");
        let retained = receipt.retained_storage();
        budget.reserve_storage(retained).map_err(resource)?;
        assert_eq!(module.module(), view.canonical().module());
        let result = observe(&module, source_cpu, order, true);
        drop(module);
        budget.release_storage(retained).map_err(resource)?;
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        budget
            .reserve_storage(std::mem::size_of::<Observation>())
            .map_err(resource)?;
        observation = Some(result);
        Ok(())
    });
    match outcome {
        Err(error) if matches!(*error, PipelineError::ScopedTileObservationIncomplete) => {
            observation.ok_or_else(|| "incomplete outcome without mixed observation".into())
        }
        Err(error) => Err(format!("genuine mixed diagnostic failed: {error:?}")),
        Ok(()) => Err("mixed diagnostic unexpectedly continued compilation".into()),
    }
}

struct ReferenceCallbacks {
    order: Order,
    mixed: bool,
    result: Option<Result<Observation, String>>,
}

impl Callbacks for ReferenceCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert!(
            self.result.is_none(),
            "one transaction per compiler session"
        );
        self.result = Some(inspect(tcx, self.order, self.mixed));
        Compilation::Stop
    }
}

fn child(order: Order, mixed: bool) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = ReferenceCallbacks {
        order,
        mixed,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual source callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("actual source observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(result.is_ok(), "SIMT/mixed source observation: {result:?}");
}

#[test]
#[ignore = "process helper; requires exact source input from its parent"]
fn blocked_mixed_child() {
    child(Order::Blocked, true);
}

#[test]
#[ignore = "process helper; requires exact source input from its parent"]
fn blocked_simt_child() {
    child(Order::Blocked, false);
}

#[test]
#[ignore = "process helper; requires exact source input from its parent"]
fn striped_mixed_child() {
    child(Order::Striped, true);
}

#[test]
#[ignore = "process helper; requires exact source input from its parent"]
fn striped_simt_child() {
    child(Order::Striped, false);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic SDK and source compilation"]
fn actual_simt_and_mixed_sources_match_each_fixed_layout_algorithm() {
    let blocked = simt_source(Order::Blocked);
    let striped = simt_source(Order::Striped);
    let mut profiles = std::collections::BTreeSet::new();
    run_actual_source_variants::<Observation>(
        &[
            ("mixed", MIXED_TILE_SOURCE),
            ("mixed", MIXED_TILE_SOURCE),
            ("simt_blocked", &blocked),
            ("simt_blocked", &blocked),
            ("mixed", MIXED_TILE_SOURCE),
            ("mixed", MIXED_TILE_SOURCE),
            ("simt_striped", &striped),
            ("simt_striped", &striped),
        ],
        &[(0, 0), (3, 2)],
        &[
            BLOCKED_MIXED,
            BLOCKED_MIXED,
            BLOCKED_SIMT,
            BLOCKED_SIMT,
            STRIPED_MIXED,
            STRIPED_MIXED,
            STRIPED_SIMT,
            STRIPED_SIMT,
        ],
        "ACTUAL_SIMT_MIXED_CPU_REFERENCE",
        str::to_owned,
        |opt, mir, label, observation, previous| {
            assert!([942, 950].contains(&observation.source_cpu));
            assert!([0, 1].contains(&observation.order));
            assert_eq!(
                observation.kir_version,
                if observation.mixed { 18 } else { 7 }
            );
            assert_eq!((observation.vectors, observation.replays), (50, 100));
            assert!(observation.reads > 0 && observation.writes > 0);
            assert_ne!(observation.canonical, [0; 32]);
            assert_ne!(observation.outputs, [0; 32]);
            assert!(!observation.execution_authority);
            if !observation.mixed {
                let mixed = previous
                    .get(&format!("{}:mixed", observation.order))
                    .expect("matching fixed-layout mixed observation");
                assert_eq!(mixed.source_cpu, observation.source_cpu);
                assert_eq!(
                    mixed.outputs, observation.outputs,
                    "actual backings and masks"
                );
                assert_eq!(
                    (mixed.reads, mixed.writes),
                    (observation.reads, observation.writes)
                );
                assert_ne!(mixed.canonical, observation.canonical);
            } else if observation.order == 1 {
                let blocked = previous.get("0:mixed").expect("blocked mixed observation");
                assert_ne!(
                    blocked.outputs, observation.outputs,
                    "these algorithms are layout-specific"
                );
            }
            profiles.insert((
                observation.source_cpu,
                opt,
                mir,
                observation.order,
                observation.mixed,
            ));
            let key = format!("{}:{label}", observation.order);
            if let Some(first) = previous.get(&key) {
                assert_eq!(&observation, first, "independent fresh compiler session");
            } else {
                previous.insert(key, observation);
            }
        },
    );
    assert_eq!(profiles.len(), 16);
}

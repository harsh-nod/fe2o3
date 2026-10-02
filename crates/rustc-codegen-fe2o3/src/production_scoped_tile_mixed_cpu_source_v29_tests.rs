//! Mixed tile/SIMT source diagnostics, not terminal lowering or GPU qualification.
//! Source CPUs and simulator profiles are reported separately from graph authority.
use super::*;
use fe2o3_kernel_ir as kir;
use fe2o3_kir_sim as sim;
use fe2o3_lower_mir_kernel::{
    ProductionScopedTileCandidateViewV29 as View, ProductionScopedTileObservationOrderV29 as Order,
};

#[path = "production_scoped_tile_mixed_cpu_oracle_v29_tests.rs"]
mod oracle;

#[path = "production_scoped_tile_simt_reference_v29_tests.rs"]
mod simt_reference;

const BLOCKED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::mixed_tile_cpu_tests::scoped_tile_mixed_cpu_source_blocked_child";
const STRIPED_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::mixed_tile_cpu_tests::scoped_tile_mixed_cpu_source_striped_child";

const MIXED_TILE_SOURCE: &str = r#"use fe2o3_device::{kernel, thread, DisjointSlice, KernelContext, MaskedTile1D};
#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn mixed_tile_probe(
    mut ctx: KernelContext<'_>,
    input: &[u32],
    base: usize,
    mut output: DisjointSlice<u32>,
) {
    let ([x, y, z], [mx, my, mz]) = ctx.with_workgroup(move |workgroup| {
        let tile = MaskedTile1D::<u32, 64, 3, _>::load_masked(&workgroup, input, base);
        tile.into_fragment().into_parts()
    });
    let x = if mx { x.wrapping_mul(3).wrapping_add(11) } else { 0 };
    let y = if my { y.wrapping_mul(5).wrapping_add(13) } else { 0 };
    let z = if mz { z.wrapping_mul(7).wrapping_add(17) } else { 0 };
    if mx || my || mz {
        if let Some(slot) = output.get_mut(thread::index_1d()) {
            *slot = x.wrapping_add(y).wrapping_add(z);
        }
    }
}
"#;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct MixedCpuObservation {
    source_cpu: u16,
    order: u8,
    source: [u8; 32],
    pending: [u8; 32],
    scalar: [u8; 32],
    schedule: [u8; 32],
    canonical_bytes: u64,
    vectors: usize,
    replays: usize,
    read_events: usize,
    write_events: usize,
    original_account_stable: bool,
    execution_authority: bool,
}

fn order_code(order: Order) -> u8 {
    match order {
        Order::Blocked => 0,
        Order::Striped => 1,
    }
}

fn inspect_cpu(
    view: View<'_>,
    source_cpu: u16,
    order: Order,
    budget: &mut Budget<'_>,
) -> Result<MixedCpuObservation, PipelineError> {
    let resource = |error: ResourceError| PipelineError::ContextHandoff(error.into());
    assert!(!view.grants_execution_authority());
    let owner = view.canonical();
    assert_eq!(owner.module().kernels.len(), 1);
    assert!(!owner.module().storage_layouts.is_empty());
    assert!(
        owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(operation.kind, kir::OperationKind::Execution(_))),
        "ordinary simulator execution must not bypass an opaque tile operation"
    );
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let limits = sim::SimulationLimitsV1::default();
    let (simulation, receipt) =
        sim::AdmittedSimulationModuleV1::admit_v18_with_verification_budget(owner, limits, budget)
            .expect("actual mixed source candidate must admit with storage tables intact");
    let paid = receipt.retained_storage();
    budget.reserve_storage(paid).map_err(resource)?;
    assert_eq!(simulation.module(), owner.module());
    assert_eq!(simulation.identity().digest(), owner.identity().digest());
    assert!(!simulation.grants_execution_authority());
    let (vectors, replays, read_events, write_events) = oracle::run(&simulation, limits, order);
    drop(simulation);
    budget.release_storage(paid).map_err(resource)?;
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    // Request, oracle and replay storage is test-owned. Only the fixed summary escapes.
    budget
        .reserve_storage(std::mem::size_of::<MixedCpuObservation>())
        .map_err(resource)?;
    Ok(MixedCpuObservation {
        source_cpu,
        order: order_code(order),
        source: *view.source_semantic_sha256(),
        pending: *view.pending_identity().digest(),
        scalar: *owner.identity().digest(),
        schedule: *view.schedule_identity(),
        canonical_bytes: owner.identity().canonical_length(),
        vectors,
        replays,
        read_events,
        write_events,
        original_account_stable: true,
        execution_authority: false,
    })
}

fn inspect_order<'tcx>(tcx: TyCtxt<'tcx>, order: Order) -> Result<MixedCpuObservation, String> {
    let transaction = transaction_in_active_session_v1(
        tcx,
        crate::rustc_semantic_plan_v1::DebugSourceCaptureRequestV2::Disabled,
    )?;
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
    let mut observation = None;
    let result = transaction.observe_scoped_tile_candidate_in_order_v29(order, |view, budget| {
        assert!(observation.is_none(), "one complete mixed source candidate");
        observation = Some(inspect_cpu(view, source_cpu, order, budget)?);
        Ok(())
    });
    match result {
        Err(error) if matches!(*error, PipelineError::ScopedTileObservationIncomplete) => {
            observation.ok_or_else(|| "incomplete outcome without CPU observation".into())
        }
        Err(error) => Err(format!("actual mixed tile CPU candidate failed: {error:?}")),
        Ok(()) => Err("CPU diagnostic unexpectedly continued compilation".into()),
    }
}

struct MixedCallbacks {
    order: Order,
    result: Option<Result<MixedCpuObservation, String>>,
}

impl Callbacks for MixedCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        assert!(
            self.result.is_none(),
            "one analysis callback per compiler session"
        );
        self.result = Some(inspect_order(tcx, self.order));
        Compilation::Stop
    }
}

fn run_child(order: Order) {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = MixedCallbacks {
        order,
        result: None,
    };
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual mixed CPU callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("mixed tile CPU observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual mixed tile CPU observation: {result:?}"
    );
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn scoped_tile_mixed_cpu_source_blocked_child() {
    run_child(Order::Blocked);
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn scoped_tile_mixed_cpu_source_striped_child() {
    run_child(Order::Striped);
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_mixed_tile_source_cpu_orders_match_independent_oracles_and_replay() {
    let mut seen_profiles = std::collections::BTreeSet::new();
    // Share source and dependency identities, but never reuse the one-use MIR capture.
    run_actual_source_variants::<MixedCpuObservation>(
        &[
            ("mixed_tile", MIXED_TILE_SOURCE),
            ("mixed_tile", MIXED_TILE_SOURCE),
            ("mixed_tile", MIXED_TILE_SOURCE),
            ("mixed_tile", MIXED_TILE_SOURCE),
        ],
        &[(0, 0), (0, 2), (3, 0), (3, 2)],
        &[BLOCKED_CHILD, BLOCKED_CHILD, STRIPED_CHILD, STRIPED_CHILD],
        "ACTUAL_MIXED_TILE_CPU_ORDER",
        str::to_owned,
        |opt, mir, label, observation, observations| {
            assert!([0, 1].contains(&observation.order));
            assert!([942, 950].contains(&observation.source_cpu));
            assert_eq!(observation.vectors, 50);
            assert_eq!(observation.replays, 100);
            assert!(observation.read_events > 0 && observation.write_events > 0);
            assert!(observation.original_account_stable);
            assert!(!observation.execution_authority);
            assert!(observation.canonical_bytes > 0);
            assert_ne!(observation.pending, observation.scalar);
            assert_ne!(observation.schedule, [0; 32]);
            let key = format!("{}:{label}", observation.order);
            if observation.order == 1 {
                let blocked = observations
                    .get(&format!("0:{label}"))
                    .expect("blocked observation");
                assert_eq!(blocked.source_cpu, observation.source_cpu);
                assert_eq!(blocked.source, observation.source);
                assert_eq!(blocked.pending, observation.pending);
                assert_ne!(blocked.scalar, observation.scalar);
                assert_ne!(blocked.schedule, observation.schedule);
            }
            seen_profiles.insert((observation.source_cpu, opt, mir, observation.order));
            if let Some(previous) = observations.get(&key) {
                assert_eq!(&observation, previous, "fresh-process mixed CPU candidates");
            } else {
                observations.insert(key, observation);
            }
        },
    );
    assert_eq!(seen_profiles.len(), 16);
}

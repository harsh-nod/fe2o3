//! Actual Rust candidate CPU diagnostics; not source-proof or tutorial-pair admission.
//! Explicit simulator profiles below are caller-selected replay contexts. The
//! rustc source-session CPU is reported separately, not attached as graph authority.
use super::*;
use fe2o3_kernel_ir as kir;
use fe2o3_kir_sim as sim;
use fe2o3_lower_mir_kernel::ProductionScopedTileCandidateViewV29 as View;

#[path = "production_scoped_tile_cpu_oracle_v29_tests.rs"]
mod oracle;

const TILE_CHILD: &str = "production_rustc_driver_v1::checked_output_source_v1_tests::context_source_v29_tests::pending_source_tests::tile_cpu_tests::scoped_tile_cpu_source_child";

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct CpuObservation {
    source_cpu: u16,
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
}

fn inspect_cpu(
    view: View<'_>,
    source_cpu: u16,
    budget: &mut Budget<'_>,
) -> Result<CpuObservation, PipelineError> {
    let resource = |error: ResourceError| PipelineError::ContextHandoff(error.into());
    assert!(!view.grants_execution_authority());
    let owner = view.canonical();
    assert_eq!(owner.module().kernels.len(), 1);
    assert!(
        !owner.module().storage_layouts.is_empty(),
        "retain source-owned storage layouts"
    );
    assert!(
        owner
            .module()
            .functions
            .iter()
            .filter_map(|function| function.body.as_ref())
            .flat_map(|body| &body.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(operation.kind, kir::OperationKind::Execution(_))),
        "scalar candidate must discharge no opaque execution operation by simulator bypass"
    );
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let limits = sim::SimulationLimitsV1::default();
    let (simulation, receipt) =
        sim::AdmittedSimulationModuleV1::admit_v18_with_verification_budget(owner, limits, budget)
            .expect("actual source V18 candidate must admit without table erasure");
    let paid = receipt.retained_storage();
    budget.reserve_storage(paid).map_err(resource)?;
    assert_eq!(simulation.module(), owner.module());
    assert_eq!(simulation.identity().digest(), owner.identity().digest());
    assert!(!simulation.grants_execution_authority());
    // Oracle/request/output storage belongs to the bounded simulator/test domain.
    // Only this fixed diagnostic result escapes; its payment is caller-owned.
    let (vectors, replays, read_events, write_events) = oracle::run(&simulation, limits);
    drop(simulation);
    budget.release_storage(paid).map_err(resource)?;
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
    budget
        .reserve_storage(std::mem::size_of::<CpuObservation>())
        .map_err(resource)?;
    Ok(CpuObservation {
        source_cpu,
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
    })
}

#[derive(Default)]
struct TileCallbacks {
    result: Option<Result<CpuObservation, String>>,
}

impl Callbacks for TileCallbacks {
    fn after_analysis<'tcx>(&mut self, _: &Compiler, tcx: TyCtxt<'tcx>) -> Compilation {
        self.result = Some((|| {
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
            let result = transaction.observe_scoped_tile_candidate_v29(|view, budget| {
                assert!(observation.is_none(), "one complete source candidate");
                observation = Some(inspect_cpu(view, source_cpu, budget)?);
                Ok(())
            });
            match result {
                Err(error) if matches!(*error, PipelineError::ScopedTileObservationIncomplete) => {
                    observation.ok_or_else(|| "incomplete outcome without CPU observation".into())
                }
                Err(error) => Err(format!(
                    "actual scoped tile CPU candidate failed: {error:?}"
                )),
                Ok(()) => Err("CPU diagnostic unexpectedly continued compilation".into()),
            }
        })());
        Compilation::Stop
    }
}

#[test]
#[ignore = "process helper; requires an exact actual-source request from its parent"]
fn scoped_tile_cpu_source_child() {
    let Some(path) = env::var_os(ARGS) else {
        return;
    };
    let args: Vec<String> = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let mut callbacks = TileCallbacks::default();
    rustc_driver::run_compiler(&args, &mut callbacks);
    let result = callbacks
        .result
        .expect("actual tile CPU callback did not run");
    std::fs::write(
        env::var_os(RESULT).expect("tile CPU observation path"),
        serde_json::to_vec(&result).unwrap(),
    )
    .unwrap();
    assert!(
        result.is_ok(),
        "actual tile CPU source observation: {result:?}"
    );
}

#[test]
#[ignore = "requires pinned nightly rust-src, authentic AMD SDK dependencies and source compilation"]
fn actual_masked_tile_source_cpu_candidate_matches_oracle_and_replay() {
    run_actual_sources::<CpuObservation>(
        &[
            ("masked_tile", MASKED_TILE_SOURCE),
            ("masked_tile", MASKED_TILE_SOURCE),
        ],
        &[(0, 0), (0, 2), (3, 0), (3, 2)],
        TILE_CHILD,
        "ACTUAL_MASKED_TILE_CPU_CANDIDATE",
        str::to_owned,
        |_, _, label, observation, observations| {
            assert_eq!(observation.vectors, 28);
            assert!([942, 950].contains(&observation.source_cpu));
            assert_eq!(observation.replays, 56);
            assert!(observation.read_events > 0 && observation.write_events > 0);
            assert!(observation.original_account_stable);
            assert!(observation.canonical_bytes > 0);
            assert_ne!(observation.pending, observation.scalar);
            assert_ne!(observation.schedule, [0; 32]);
            if let Some(previous) = observations.get(label) {
                assert_eq!(
                    &observation, previous,
                    "fresh-process actual-source CPU candidate"
                );
            } else {
                observations.insert(label.to_owned(), observation);
            }
        },
    );
}

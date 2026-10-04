//! Genuine source -> retained Call/Return -> existing two-frame CPU engine.
use super::*;
use crate::production_bf16_tile_values_source_v1::SourceOwnedBf16TileValuesRegionV1;
use crate::production_rustc_driver_v1::gfx942_bf16_publication_tap_v1_tests as publication_tap;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_kir_sim::*;
use fe2o3_lower_mir_kernel::Bf16CallInstanceEmissionViewV1 as Emission;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Run {
    pattern: usize,
    output_length: usize,
    helper_values_row_major_le_hex: oracle::Words,
    caller_values_row_major_le_hex: oracle::Words,
    output_with_canaries_le_hex: oracle::Output,
    actual_allocations: [u64; 3],
    helper_lane_mask: u64,
    caller_lane_mask: u64,
    committed_store_lane_mask: u64,
    records: u64,
    steps: u64,
    storage_floor: usize,
    storage_after: usize,
    work_before: usize,
    work_after: usize,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct Negative {
    control: oracle::Control,
    observed: &'static str,
    helper_lane_mask: u64,
    caller_lane_mask: u64,
    global_writes: u64,
    floor_restored: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
pub(super) struct CpuRow {
    source: super::source_observation::Snapshot,
    sites: capture::Sites,
    canonical_sha256: [u8; 32],
    canonical_bytes: usize,
    runs: [Option<Run>; 18],
    negatives: [Option<Negative>; 16],
    attempted_runs: usize,
    same_original_ledger: bool,
    source_authority_in_copied_row: bool,
}
#[derive(Clone, Copy, Debug, Serialize)]
struct Progress {
    attempt: usize,
    observed: &'static str,
    debug_failure: Option<&'static str>,
    helper_lane_mask: u64,
    caller_lane_mask: u64,
    global_writes: u64,
    floor_restored: bool,
}
struct Events(bool);
impl SimulationEventSinkV1 for Events {
    fn record(&mut self, _: &SimulationEventV1) -> Result<(), SimulationEventSinkErrorV1> {
        if self.0 {
            Err(SimulationEventSinkErrorV1 {
                detail: "helper source CPU event control".into(),
            })
        } else {
            Ok(())
        }
    }
}
struct Sink {
    frames: capture::Frames,
    stop: bool,
}
impl SimulationDebugSinkV1 for Sink {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        if self.stop {
            SimulationDebugSinkControlV1::Stop
        } else {
            self.frames.record(record)
        }
    }
}
fn classify(run: Result<&SimulationExecutionV1, &SimulationErrorV1>) -> &'static str {
    match run {
        Ok(_) => "ok",
        Err(SimulationErrorV1::Preflight(_)) => "preflight",
        Err(SimulationErrorV1::Execution(error)) => match error.kind {
            SimulationExecutionErrorKindV1::UninitializedRead {
                offset: 510,
                bytes: 2,
                ..
            } => "uninitialized-read",
            SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
                role: MatrixInputRoleV1::A,
                lane: 63,
                component: 3,
            } => "matrix-domain-a",
            SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
                role: MatrixInputRoleV1::B,
                lane: 63,
                component: 3,
            } => "matrix-domain-b",
            SimulationExecutionErrorKindV1::StepLimit { limit: 1 } => "step-limit",
            SimulationExecutionErrorKindV1::EventSinkFailure(_) => "event-sink",
            _ => "other-execution-refusal",
        },
    }
}
pub(super) fn expected(control: oracle::Control) -> &'static str {
    use oracle::Control::*;
    match control {
        Positive => "ok",
        UninitializedA | UninitializedB => "uninitialized-read",
        NegativeZeroA | FractionalA | SubnormalA | OutsideDomainA => "matrix-domain-a",
        NegativeZeroB | FractionalB | InfiniteB => "matrix-domain-b",
        StepLimit => "step-limit",
        RecordLimit | DebugStop => "incomplete-observation",
        EventFailure => "event-sink",
        Grid63 | Grid65 | Wave32 => "profile-launch",
    }
}
fn execute(
    source: &SourceOwnedBf16TileValuesRegionV1<'_, '_>,
    emission: &Emission<'_>,
    budget: &mut Budget<'_>,
    progress: &Cell<Option<Progress>>,
    short: bool,
) -> Result<CpuRow, Error> {
    execute_with_publication_tap(source, emission, budget, progress, short, None)
}
fn execute_with_publication_tap(
    source: &SourceOwnedBf16TileValuesRegionV1<'_, '_>,
    emission: &Emission<'_>,
    budget: &mut Budget<'_>,
    progress: &Cell<Option<Progress>>,
    short: bool,
    mut stream: Option<&mut publication_tap::Stream>,
) -> Result<CpuRow, Error> {
    // Fixed copied rows are reserved by our caller; all dynamic objects stay in
    // this scope. A copied digest is diagnostic, never the source-owner join.
    budget.charge_work(65536)?;
    let snapshot = super::source_observation::snapshot(source, budget)?;
    let sites = capture::sites(emission)?;
    if !std::ptr::eq(source.relation().owner(), emission.owner().semantic_ssa())
        || source.relation().return_permutation() != sites.permutation
    {
        return Err(Error::Unavailable(
            "actual source and CPU emission owner differ",
        ));
    }
    let owner = emission.owner().executable();
    let tap_sites = publication_tap::Sites {
        root: sites.root,
        helper: Some(sites.helper),
        matrix: sites.matrix,
        call: Some(sites.call),
        store: sites.store,
        matrix_results: sites.matrix_results,
        call_results: sites.call_results,
        parameters: sites.parameters,
    };
    if let Some(stream) = stream.as_deref_mut() {
        if sites.permutation != stream.requested_permutation() {
            return Err(Error::Unavailable(
                "actual fresh Return differs from parent requested order",
            ));
        }
        stream
            .start(
                *source.source().sha256(),
                Sha256::digest(owner.canonical().canonical_bytes()).into(),
                tap_sites,
            )
            .map_err(Error::Unavailable)?;
    }
    let ledger = budget.work_ledger_identity_v1();
    let options = Bf16CallCpuObservationOptionsV1::default();
    let (admitted, receipt) = AdmittedSimulationModuleV1::admit_v12_with_verification_budget(
        owner,
        options.simulation_limits(),
        budget,
    )
    .map_err(|_| Error::Unavailable("actual nominal V12 CPU admission refused"))?;
    budget.reserve_storage(receipt.retained_storage())?;
    let mut row = CpuRow {
        source: snapshot,
        sites,
        canonical_sha256: Sha256::digest(owner.canonical().canonical_bytes()).into(),
        canonical_bytes: owner.canonical().canonical_bytes().len(),
        runs: [None; 18],
        negatives: [None; 16],
        attempted_runs: 0,
        same_original_ledger: true,
        source_authority_in_copied_row: false,
    };
    // The fixed source phase work bound covers at most 64 such attempts. This
    // gate uses 34; error/panic children stop after one completed positive.
    for attempt in 0..if short { 1 } else { 34 } {
        let (pattern, length, control) = if attempt < 18 {
            (
                attempt / 3,
                oracle::LENGTHS[attempt % 3],
                oracle::Control::Positive,
            )
        } else {
            (2, 64, oracle::NEGATIVES[attempt - 18])
        };
        let floor = budget.storage();
        let work_before = budget.work();
        let copied = budget.with_prepaid_scope(floor, 1, 8_388_608, 65536, |budget| {
            let request = oracle::request(&owner.module().kernels[0].id, pattern, length, control);
            if !publication_tap::scratch_fits::<Sink>() {
                return Err(Error::Unavailable("bounded sidecar observation scratch"));
            }
            let mut sink = publication_tap::Tee {
                inner: Sink {
                    frames: capture::Frames::new(sites, pattern, length),
                    stop: matches!(control, oracle::Control::DebugStop),
                },
                trace: if stream.is_some() && attempt < 18 {
                    Some(
                        publication_tap::Trace::new(tap_sites, pattern, length)
                            .map_err(Error::Unavailable)?,
                    )
                } else {
                    None
                },
                tap_failure: None,
            };
            let mut sidecar_memory = None;
            let mut events = Events(matches!(control, oracle::Control::EventFailure));
            let mut options = options;
            if matches!(control, oracle::Control::StepLimit) {
                options = options.with_step_limit(1).unwrap();
            }
            if matches!(control, oracle::Control::RecordLimit) {
                options = options.with_record_limit(1).unwrap();
            }
            let outcome = admitted.with_bf16_call_cpu_observation_v1(
                V12CpuObservationInputV1::new(owner, &request),
                options,
                budget,
                (&mut events, &mut sink),
                |run, original| {
                    assert!(original.work_ledger_identity_v1() == ledger);
                    let mut output = oracle::Output([0; 272]);
                    if let Ok(execution) = run {
                        assert_eq!(execution.identity().wire_version(), 12);
                        assert_eq!(
                            execution.identity().digest(),
                            owner.canonical().identity().digest()
                        );
                        assert_eq!(
                            execution.identity().canonical_length(),
                            row.canonical_bytes as u64
                        );
                        output =
                            capture::check_output(execution, pattern, length, sites.permutation);
                        if stream.is_some() && attempt < 18 {
                            sidecar_memory =
                                Some(publication_tap::Memory::capture(execution, length)?);
                        }
                    }
                    Ok::<_, &'static str>((
                        classify(run),
                        run.ok().map_or(0, SimulationExecutionV1::steps_executed),
                        output,
                    ))
                },
            );
            let (observed, steps, output) = match outcome {
                Ok(row) => row,
                Err(V12CpuObservationErrorV1::IncompleteObservation) => {
                    ("incomplete-observation", 0, oracle::Output([0; 272]))
                }
                Err(V12CpuObservationErrorV1::Profile(V12CpuObservationProfileErrorV1::Launch)) => {
                    ("profile-launch", 0, oracle::Output([0; 272]))
                }
                Err(V12CpuObservationErrorV1::Profile(_)) => {
                    ("other-profile-refusal", 0, oracle::Output([0; 272]))
                }
                Err(V12CpuObservationErrorV1::Resource(_)) => {
                    ("resource-refusal", 0, oracle::Output([0; 272]))
                }
                Err(V12CpuObservationErrorV1::Observer(_)) => {
                    ("observer-refusal", 0, oracle::Output([0; 272]))
                }
            };
            if let Some(error) = sink.tap_failure {
                return Err(Error::Unavailable(error));
            }
            let frames = &sink.frames;
            progress.set(Some(Progress {
                attempt,
                observed,
                debug_failure: frames.failure,
                helper_lane_mask: frames.matrix_mask,
                caller_lane_mask: frames.call_mask,
                global_writes: frames.writes,
                floor_restored: false,
            }));
            if observed != expected(control) || frames.failure.is_some() {
                return Err(Error::Unavailable(
                    "actual helper-source numerical outcome differs; retained progress",
                ));
            }
            if attempt < 18 {
                frames.complete();
            } else if frames.matrix_mask != 0 || frames.call_mask != 0 || frames.writes != 0 {
                // Observations delivered before refusal, not a rollback promise.
                return Err(Error::Unavailable(
                    "refused helper observation exposed completion or writes",
                ));
            }
            // Capture fixed legacy fields before moving the independent actual trace.
            let legacy = (
                frames.matrix_values,
                frames.call_values,
                output,
                frames.allocations,
                frames.matrix_mask,
                frames.call_mask,
                frames.store_mask,
                frames.records,
                steps,
                observed,
                frames.writes,
            );
            if attempt < 18 {
                if let Some(stream) = stream.as_deref_mut() {
                    stream
                        .positive(
                            sink.trace
                                .take()
                                .ok_or(Error::Unavailable("actual sidecar trace absent"))?,
                            sidecar_memory
                                .take()
                                .ok_or(Error::Unavailable("actual sidecar memory absent"))?,
                            steps,
                        )
                        .map_err(Error::Unavailable)?;
                }
            }
            Ok((
                legacy.0, legacy.1, legacy.2, legacy.3, legacy.4, legacy.5, legacy.6, legacy.7,
                legacy.8, legacy.9, legacy.10,
            ))
        })?;
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);
        if attempt >= 18 {
            if let Some(stream) = stream.as_deref_mut() {
                stream
                    .negative(attempt, copied.9, copied.4, copied.5, copied.10, true)
                    .map_err(Error::Unavailable)?;
            }
        }
        let mut last = progress.get().unwrap();
        last.floor_restored = true;
        progress.set(Some(last));
        if attempt < 18 {
            row.runs[attempt] = Some(Run {
                pattern,
                output_length: length,
                helper_values_row_major_le_hex: copied.0,
                caller_values_row_major_le_hex: copied.1,
                output_with_canaries_le_hex: copied.2,
                actual_allocations: copied.3.unwrap(),
                helper_lane_mask: copied.4,
                caller_lane_mask: copied.5,
                committed_store_lane_mask: copied.6,
                records: copied.7,
                steps: copied.8,
                storage_floor: floor,
                storage_after: budget.storage(),
                work_before,
                work_after: budget.work(),
            });
        } else {
            row.negatives[attempt - 18] = Some(Negative {
                control,
                observed: copied.9,
                helper_lane_mask: copied.4,
                caller_lane_mask: copied.5,
                global_writes: copied.10,
                floor_restored: true,
            });
        }
        row.attempted_runs += 1;
    }
    drop(admitted);
    budget.release_storage(receipt.retained_storage())?;
    if let Some(stream) = stream {
        stream.finish().map_err(Error::Unavailable)?;
    }
    Ok(row)
}
pub(super) fn observe<'tcx>(
    transaction: crate::production_pipeline::ProductionCompilation<
        'tcx,
        crate::production_pipeline::CollectedRustStage<'tcx>,
    >,
    case: &str,
) -> Value {
    let copied = Cell::new(None);
    let progress = Cell::new(None);
    let accounting = Cell::new(None);
    let callback_control = matches!(case, "identity-error" | "identity-panic");
    let (result, phase) =
        transaction.observe_bf16_call_source_cpu_for_test_v1(|source, emission, budget| {
            let floor = budget.storage();
            let work = budget.work();
            let ledger = budget.work_ledger_identity_v1();
            let reserve = 2 * std::mem::size_of::<CpuRow>() + 4096;
            // Retained fixed observations have two complete component arrays. The
            // 128KiB test-observation bound does not increase any simulator limit.
            assert!(reserve <= 131072);
            budget.reserve_storage(reserve)?;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let row =
                    budget.with_prepaid_scope(budget.storage(), 1, 65536, 131072, |budget| {
                        execute(source, emission, budget, &progress, callback_control)
                    })?;
                assert!(copied.replace(Some(row)).is_none());
                if callback_control {
                    budget.reserve_storage(23)?;
                    budget.charge_work(17)?;
                    if case == "identity-panic" {
                        panic!("genuine helper CPU source callback panic control");
                    }
                    return Err(Error::Unavailable(
                        "genuine helper CPU source callback error control",
                    ));
                }
                Ok(())
            }));
            accounting.set(Some([
                floor,
                budget.storage(),
                work,
                budget.work(),
                reserve,
                usize::from(callback_control) * 23,
            ]));
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(
                budget.storage(),
                floor + reserve + usize::from(callback_control && copied.get().is_some()) * 23
            );
            match outcome {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        });
    // Success here is the copied numerical callback, NOT the ordinary compiler
    // result: the same nominal owner must meet its explicit normal-route refusal.
    let report = json!({
        "stage":if copied.get().is_some() { "actual_helper_source_cpu_observed" } else { "actual_helper_source_cpu_refused" },
        "cpu":copied.get(), "phase":phase, "progress":progress.get(), "accounting":accounting.get(),
        "diagnostic":result.as_ref().err().map(ToString::to_string),
        "unexpected_normal_success":result.is_ok(),
        "normal_qualified":false, "hardware_observed":false,
        "source_authority_in_copied_row":false,
    });
    assert!(serde_json::to_vec(&report).unwrap().len() <= 256 * 1024);
    report
}
#[test]
fn helper_source_observation_rows_fit_fixed_test_domain() {
    assert!(2 * std::mem::size_of::<CpuRow>() + 4096 <= 131072);
    assert!(2 * std::mem::size_of::<Sink>() + 8192 <= 65536);
    assert_eq!(
        oracle::PATTERNS * oracle::LENGTHS.len() + oracle::NEGATIVES.len(),
        34
    );
}

#[derive(Clone, Copy, Debug, Serialize)]
pub(in crate::production_rustc_driver_v1) struct PublicationObservation {
    source: super::source_observation::Snapshot,
    sites: capture::Sites,
    canonical_sha256: [u8; 32],
    canonical_bytes: usize,
    helper_return: [u32; 4],
    call_components: [[u32; 4]; 4],
    formal_components: [[u32; 4]; 4],
    component_lengths: [usize; 4],
    attempted_runs: usize,
    same_original_ledger: bool,
    source_authority_in_copied_row: bool,
}
#[derive(Default, Clone, Copy, Debug, Serialize)]
pub(in crate::production_rustc_driver_v1) struct PublicationProgress {
    last: Option<Progress>,
}
pub(in crate::production_rustc_driver_v1) fn observe_for_publication(
    source: &SourceOwnedBf16TileValuesRegionV1<'_, '_>,
    emission: &Emission<'_>,
    budget: &mut Budget<'_>,
    stream: &mut publication_tap::Stream,
    progress: &mut PublicationProgress,
) -> Result<PublicationObservation, Error> {
    let held = 2 * std::mem::size_of::<PublicationObservation>()
        + 2 * std::mem::size_of::<PublicationProgress>()
        + 1024;
    budget.reserve_storage(held)?;
    let last = Cell::new(None);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.with_prepaid_scope(budget.storage(), 1, 65536, 131072, |budget| {
            budget.reserve_storage(2 * std::mem::size_of::<CpuRow>() + 4096)?;
            execute_with_publication_tap(source, emission, budget, &last, false, Some(stream))
        })
    }));
    progress.last = last.get();
    let result = match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    };
    budget.charge_work(128)?;
    let mut call_components = [[0; 4]; 4];
    let mut formal_components = [[0; 4]; 4];
    let component_lengths = [0, 4, 4, 4];
    for index in 0..4 {
        let arguments = emission
            .call_argument_components(index)
            .ok_or(Error::Unavailable("actual call components absent"))?;
        let formals = emission
            .formal_components(index)
            .ok_or(Error::Unavailable("actual formal components absent"))?;
        if arguments.len() != component_lengths[index] || formals.len() != component_lengths[index]
        {
            return Err(Error::Unavailable("actual nominal component arity"));
        }
        for (slot, value) in arguments.iter().enumerate() {
            call_components[index][slot] = value.0;
        }
        for (slot, value) in formals.iter().enumerate() {
            formal_components[index][slot] = value.0;
        }
    }
    result.map(|row| PublicationObservation {
        source: row.source,
        sites: row.sites,
        canonical_sha256: row.canonical_sha256,
        canonical_bytes: row.canonical_bytes,
        helper_return: emission.helper_return().map(|value| value.0),
        call_components,
        formal_components,
        component_lengths,
        attempted_runs: row.attempted_runs,
        same_original_ledger: row.same_original_ledger,
        source_authority_in_copied_row: false,
    })
}
#[test]
fn publication_helper_tap_fits_unchanged_request_scratch() {
    assert!(publication_tap::scratch_fits::<Sink>());
}

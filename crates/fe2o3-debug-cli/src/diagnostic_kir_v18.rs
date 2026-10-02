//! Raw V18 diagnostic CPU sessions; no source, persisted-schedule or GPU authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_kir_sim_cli::load_debug_simulation_input_v18;
use std::path::Path;

pub(super) const DIAGNOSIS_UNAVAILABLE: &str = "diagnosis V2 cannot represent raw canonical KIR V18; logical debugger/resource queries remain available";
const MAX_WORK: usize = 1 << 27;
const MAX_STORAGE: usize = 256 * 1024 * 1024;

pub(super) fn require_supported_options(
    wave: DebugWaveWidthV1,
    source_map: bool,
    replay: bool,
) -> Result<(), String> {
    if wave != DebugWaveWidthV1::Wave64 || source_map || replay {
        return Err("diagnostic KIR V18 requires wave64 and does not support source maps or persisted schedule replay".into());
    }
    Ok(())
}

pub(super) fn configuration_identity(
    input: &AdmittedSimulationInputV1,
    wave: DebugWaveWidthV1,
    capture: SimulationDebugCaptureLimitsV1,
    debugger: DebuggerLimitsV1,
) -> Result<OpaqueIdentityV1, String> {
    super::diagnostic_kir_v17::configuration_identity_for_profile(
        input,
        wave,
        capture,
        debugger,
        18,
        b"fe2o3-debug-sim-diagnostic-kir-v18-config-v1\0",
    )
}

#[derive(Debug)]
enum RunError {
    Input(fe2o3_kir_sim_cli::SimulationInputErrorV1),
    Resource(Resource),
    Session(String),
}
impl From<Resource> for RunError {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}

fn run_with_budget<R: BufRead, W: Write>(
    kir: &Path,
    request: &Path,
    wave: DebugWaveWidthV1,
    budget: &mut Budget<'_>,
    reader: &mut R,
    writer: &mut W,
) -> Result<(), RunError> {
    require_supported_options(wave, false, false).map_err(RunError::Session)?;
    let floor = budget.storage();
    budget.with_prepaid_scope(floor, 0, 0, 0, |budget| {
        let (input, storage) =
            load_debug_simulation_input_v18(kir, request, budget).map_err(RunError::Input)?;
        budget.reserve_storage(storage.retained_storage())?;
        // The generic backend owns the module and is consumed/dropped by the
        // protocol loop before this original ledger restores storage.
        run_admitted_jsonl_v1(input, wave, reader, writer).map_err(RunError::Session)
    })
}

pub(super) fn run(options: OptionsV1) -> ExitCode {
    let (ProgramInputV1::DiagnosticKirV18(kir), RequestInputV1::Path(request)) =
        (&options.program, &options.request)
    else {
        write_bootstrap_error(
            "arguments",
            "invalid_command_line",
            "V18 requires file inputs",
        );
        return ExitCode::FAILURE;
    };
    let mut work = Work::new(MAX_WORK);
    let mut budget = Budget::new(&mut work, MAX_STORAGE);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = BufReader::new(stdin.lock());
    let mut writer = BufWriter::new(stdout.lock());
    let result = run_with_budget(
        kir,
        request,
        options.wave_width,
        &mut budget,
        &mut reader,
        &mut writer,
    );
    // Release stdio locks before publishing a bounded bootstrap diagnostic.
    drop(reader);
    drop(writer);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(RunError::Input(error)) => {
            write_input_error(&error);
            ExitCode::FAILURE
        }
        Err(RunError::Session(message)) => {
            write_bootstrap_error("backend", "simulation_capture_failed", &message);
            ExitCode::FAILURE
        }
        Err(RunError::Resource(error)) => {
            let code = match error {
                Resource::Work(_) => "kir_v18_work_limit",
                Resource::Storage(_) => "kir_v18_storage_limit",
                Resource::Allocation => "kir_v18_allocation_failed",
                Resource::Accounting => "kir_v18_resource_accounting",
                Resource::Arithmetic => "kir_v18_resource_arithmetic",
            };
            write_bootstrap_error("kir_admission", code, &error.to_string());
            ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "diagnostic_kir_v18_tests.rs"]
mod tests;

//! Explicit V21 typed route. No auto-version fallback or raw capture import.
use super::*;
use fe2o3_kir_debugger::PhysicalGlobalCopyDebugSessionV21 as CopySession;
use fe2o3_kir_sim::PhysicalGlobalCopyDebugOptionsV21;
use fe2o3_kir_sim_cli::{
    PhysicalGlobalCopyDebugInputV21 as CopyInput, load_physical_global_copy_debug_input_v21,
};
fn capture(input: &CopyInput, ledger: Owned) -> Result<Backend<CopySession>, &'static str> {
    let configuration = configuration_for(
        Profile::GlobalCopyV21,
        input.canonical().identity().digest(),
        input.canonical().identity().canonical_length(),
        input.request_digest(),
        input.request_bytes(),
        input.limits(),
    )?;
    let options = PhysicalGlobalCopyDebugOptionsV21::new(
        input.limits(),
        SimulationDebugCaptureLimitsV1::new(1, 768, 8, 16384)
            .map_err(|_| "kir_v21_debug_capture_limits")?,
        RECORDS,
    )
    .ok_or("kir_v21_debug_capture_limits")?;
    let session = CopySession::capture(
        input.module(),
        input.canonical(),
        input.request(),
        options,
        ledger,
    );
    if session.capture_error().is_some()
        || session.outcome() != PhysicalEntryDebugOutcomeV20::Completed
    {
        return Err("kir_v21_debug_capture_refused");
    }
    Ok(Backend {
        session,
        configuration,
        revision: 0,
        terminated: false,
    })
}
pub(super) fn run(arguments: Vec<OsString>) -> ExitCode {
    let result = (|| {
        let (kir, request) = parse_profile(arguments, Profile::GlobalCopyV21)?;
        let mut ledger = Owned::new(Work::new(WORK), STORAGE);
        ledger
            .with_budget(|b| {
                b.reserve_storage(SCRATCH)?;
                b.charge_work(RESPONSE * 2)
            })
            .map_err(|_| "kir_v21_debug_protocol_budget")?;
        let (input, receipt) = ledger
            .with_budget(|b| load_physical_global_copy_debug_input_v21(&kir, &request, b))
            .map_err(|e| e.code())?;
        ledger
            .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
            .map_err(|_| "kir_v21_debug_input_storage")?;
        let mut backend = capture(&input, ledger)?;
        let result = serve(&mut backend);
        // All IO/response projections have dropped before inputs and their reservation.
        drop(input);
        let mut ledger = backend.session.into_budget();
        ledger
            .with_budget(|b| b.release_storage(receipt.retained_storage() + SCRATCH))
            .map_err(|_| "kir_v21_debug_teardown_accounting")?;
        result
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            write_bootstrap_error(
                "diagnostic_kir_v21",
                code,
                "typed physical-global-copy CPU debugger request refused",
            );
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
#[path = "diagnostic_kir_v21_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "diagnostic_kir_v21_actual_source_tests.rs"]
mod actual_source;

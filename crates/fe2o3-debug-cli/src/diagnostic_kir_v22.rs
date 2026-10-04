//! Explicit exact-owner V22 CPU route. No source, runtime or hardware authority.
use super::*;
use fe2o3_kir_debugger::PhysicalLdsExchangeDebugSessionV22 as LdsSession;
use fe2o3_kir_sim::PhysicalLdsExchangeDebugOptionsV22;
use fe2o3_kir_sim_cli::{
    PhysicalLdsExchangeDebugInputV22 as LdsInput, load_physical_lds_exchange_debug_input_v22,
};
#[path = "diagnostic_physical_lds_index_v1.rs"]
mod index;

struct Arguments {
    kir: PathBuf,
    request: PathBuf,
    capture_index: Option<PathBuf>,
}
fn parse(arguments: Vec<OsString>) -> Result<Arguments, &'static str> {
    let profile = Profile::LdsExchangeV22;
    if arguments.len() > 11 || arguments.first().is_none_or(|s| s != "sim") {
        return Err(profile.code(Code::Arguments));
    }
    let mut kir = None;
    let mut request = None;
    let mut capture_index = None;
    let mut wave = false;
    let mut protocol = false;
    let mut values = arguments.into_iter().skip(1);
    while let Some(option) = values.next() {
        let value = values.next().ok_or(profile.code(Code::Arguments))?;
        if option == profile.selector() && kir.is_none() {
            kir = Some(PathBuf::from(value));
        } else if option == "--request" && request.is_none() {
            request = Some(PathBuf::from(value));
        } else if option == "--capture-index" && capture_index.is_none() {
            capture_index = Some(PathBuf::from(value));
        } else if option == "--protocol" && !protocol && value == "jsonl" {
            protocol = true;
        } else if option == "--wave-width" && !wave && value == "64" {
            wave = true;
        } else {
            return Err(profile.code(Code::OptionUnavailable));
        }
    }
    Ok(Arguments {
        kir: kir.ok_or(profile.code(Code::Arguments))?,
        request: request.ok_or(profile.code(Code::Arguments))?,
        capture_index,
    })
}
fn configuration(input: &LdsInput, export: bool) -> Result<OpaqueIdentityV1, &'static str> {
    let mut hash = configuration_hash_for(
        Profile::LdsExchangeV22,
        input.canonical().identity().digest(),
        input.canonical().identity().canonical_length(),
        input.request_digest(),
        input.request_bytes(),
        input.limits(),
    );
    hash.update([u8::from(export)]);
    for value in index::CONFIGURATION_CAPS {
        hash.update((value as u64).to_le_bytes());
    }
    OpaqueIdentityV1::new(hash.finalize().into())
        .map_err(|_| Profile::LdsExchangeV22.code(Code::ConfigurationInvalid))
}
fn capture(
    input: &LdsInput,
    ledger: Owned,
    export: bool,
) -> Result<Backend<LdsSession>, &'static str> {
    let configuration = configuration(input, export)?;
    let options = PhysicalLdsExchangeDebugOptionsV22::new(
        input.limits(),
        SimulationDebugCaptureLimitsV1::new(1, 768, 8, 16384)
            .map_err(|_| "kir_v22_debug_capture_limits")?,
        Profile::LdsExchangeV22.record_limit(),
    )
    .ok_or("kir_v22_debug_capture_limits")?;
    let session = LdsSession::capture(
        input.module(),
        input.canonical(),
        input.request(),
        options,
        ledger,
    );
    if session.capture_error().is_some()
        || session.outcome() != PhysicalEntryDebugOutcomeV20::Completed
    {
        return Err("kir_v22_debug_capture_refused");
    }
    Ok(Backend {
        session,
        configuration,
        revision: 0,
        terminated: false,
    })
}
fn workspace(export: bool) -> usize {
    SCRATCH + if export { index::TEMP_STORAGE } else { 0 }
}
pub(super) fn run(arguments: Vec<OsString>) -> ExitCode {
    let result = (|| {
        let args = parse(arguments)?;
        let export = args.capture_index.is_some();
        let mut ledger = Owned::new(Work::new(WORK), STORAGE);
        ledger
            .with_budget(|b| {
                b.reserve_storage(workspace(export))?;
                b.charge_work(RESPONSE * 2)
            })
            .map_err(|_| "kir_v22_debug_protocol_budget")?;
        let (input, receipt) = ledger
            .with_budget(|b| {
                load_physical_lds_exchange_debug_input_v22(&args.kir, &args.request, b)
            })
            .map_err(|e| e.code())?;
        ledger
            .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
            .map_err(|_| "kir_v22_debug_input_storage")?;
        let mut backend = capture(&input, ledger, export)?;
        let result = (|| {
            if let Some(path) = args.capture_index {
                // The allocation is gone before serving. Its conservative prepaid
                // reservation remains at the session floor until this single teardown.
                index::export(&mut backend, &input, &path)?;
            }
            serve(&mut backend)
        })();
        drop(input);
        let mut ledger = backend.session.into_budget();
        ledger
            .with_budget(|b| b.release_storage(receipt.retained_storage() + workspace(export)))
            .map_err(|_| "kir_v22_debug_teardown_accounting")?;
        result
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            write_bootstrap_error(
                "diagnostic_kir_v22",
                code,
                "typed physical-LDS-exchange CPU debugger request refused",
            );
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
#[path = "diagnostic_kir_v22_tests.rs"]
mod tests;

//! Closed typed V20 diagnostic CLI. No source, runtime, native or deployment authority.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as Owned,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_kir_debugger::{
    PhysicalEntryDebugNavigationV20 as Navigation, PhysicalEntryDebugSessionV20 as Session,
};
use fe2o3_kir_sim::{
    PhysicalEntryDebugOptionsV20, PhysicalEntryDebugOutcomeV20,
    PhysicalEntryDebugRecordRefV20 as Record,
};
use fe2o3_kir_sim_cli::{PhysicalEntryDebugInputV20 as Input, load_physical_entry_debug_input_v20};
#[path = "diagnostic_kir_v20_protocol.rs"]
mod protocol;
#[path = "diagnostic_kir_v20_views.rs"]
mod views;
const LINE: usize = 8192;
const RESPONSE: usize = 64 * 1024;
const PAGE: usize = 64;
const COMMANDS: usize = 4096;
const WORK: usize = 1 << 29;
const STORAGE: usize = 512 * 1024 * 1024;
const RECORDS: usize = 8192;
const QUERY_WORK: usize = LINE * 128 + RESPONSE * 4;
/// Conservative reusable protocol workspace: geometric input/tree storage,
// recursive typed predicates/paths, response projections, two encoded frames,
// and stdio buffers. Each allocated JSON node consumes source syntax; every
// typed node is below CELL and the extra factor covers container overcapacity.
const CELL: usize = 1024;
const SCRATCH: usize = LINE * CELL * 4 + RESPONSE * 4 + 64 * 1024;
const _: () = assert!(std::mem::size_of::<DebugRequestV1>() <= CELL);
const _: () = assert!(std::mem::size_of::<BreakpointSpecV1>() <= CELL);
const _: () = assert!(std::mem::size_of::<WatchpointSpecV1>() <= CELL);
const _: () = assert!(std::mem::size_of::<PredicateV1>() <= CELL);
const _: () = assert!(std::mem::size_of::<ValuePathV1>() <= CELL);
fn protocol_limits() -> ProtocolLimitsV1 {
    ProtocolLimitsV1::new(LINE, RESPONSE, 64, 64, PAGE).expect("fixed V20 protocol limits")
}
struct Backend {
    session: Session,
    configuration: OpaqueIdentityV1,
    revision: u64,
    terminated: bool,
}
impl Backend {
    fn sequence(&self) -> u64 {
        self.session.cursor().map_or(0, |n| n as u64 + 1)
    }
    fn view_at(&self, sequence: u64, revision: u64, terminated: bool) -> SessionViewV1 {
        SessionViewV1 {
            backend: DebugBackendV1::CpuKirSimulator,
            execution_kind: ExecutionKindV1::CpuKirSimulation,
            state: if terminated {
                SessionStateV1::Terminated
            } else if sequence == 0 {
                SessionStateV1::Created
            } else {
                SessionStateV1::Stopped
            },
            revision,
            configuration_identity: self.configuration,
            cursor: DebugCursorV1 {
                configuration_identity: self.configuration,
                event_sequence: sequence,
                state_revision: revision,
            },
            simulated: true,
            hardware_observed: false,
            performance_prediction: false,
        }
    }
    fn view(&self) -> SessionViewV1 {
        self.view_at(self.sequence(), self.revision, self.terminated)
    }
    fn error(
        &self,
        id: Option<u64>,
        op: Option<DebugOperationNameV1>,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> DebugResponseV1 {
        DebugResponseV1::Error {
            schema: ResponseSchemaV1::V1,
            request_id: id,
            operation: op,
            session: Some(self.view()),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Session,
                code,
                message: message.into(),
                state_changed: false,
            },
        }
    }
    fn ok_at(
        &self,
        id: u64,
        op: DebugOperationNameV1,
        result: DebugResultV1,
        session: SessionViewV1,
    ) -> DebugResponseV1 {
        DebugResponseV1::Ok {
            schema: ResponseSchemaV1::V1,
            request_id: id,
            operation: op,
            session,
            result: Box::new(result),
        }
    }
    fn unavailable(
        &self,
        id: u64,
        op: DebugOperationNameV1,
        capability: DebugCapabilityNameV1,
        reason: CapabilityUnavailableReasonV1,
    ) -> DebugResponseV1 {
        DebugResponseV1::Unavailable {
            schema: ResponseSchemaV1::V1,
            request_id: id,
            operation: op,
            session: self.view(),
            unavailable: CapabilityUnavailableV1 {
                capability,
                reason,
                state_changed: false,
                detail: "diagnostic physical-entry V20 exposes bounded CPU observations only"
                    .into(),
            },
        }
    }
}
fn configuration(input: &Input) -> Result<OpaqueIdentityV1, &'static str> {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-debug-physical-entry-v20-cpu-config-v1\0");
    hash.update(input.canonical().identity().digest());
    hash.update(
        input
            .canonical()
            .identity()
            .canonical_length()
            .to_le_bytes(),
    );
    hash.update(input.request_digest());
    hash.update((input.request_bytes() as u64).to_le_bytes());
    for n in [
        LINE, RESPONSE, PAGE, COMMANDS, WORK, STORAGE, RECORDS, QUERY_WORK, CELL, SCRATCH, 1, 768,
        8, 16384,
    ] {
        hash.update((n as u64).to_le_bytes());
    }
    // Fixed CPU tooling limits have their own closed configuration domain.
    let l = input.limits();
    for n in [
        l.max_canonical_bytes,
        l.max_reachable_functions,
        l.max_reachable_operations,
        l.max_call_depth,
        l.max_ssa_values,
        l.max_allocations,
        l.max_allocation_bytes,
        l.max_total_bytes,
        l.max_resident_bytes,
        l.max_memory_access_records,
    ] {
        hash.update((n as u64).to_le_bytes());
    }
    for n in [
        l.max_invocations,
        l.max_workgroups,
        l.max_scheduled_slots,
        l.max_steps,
        l.max_events,
    ] {
        hash.update(n.to_le_bytes());
    }
    OpaqueIdentityV1::new(hash.finalize().into()).map_err(|_| "kir_v20_debug_configuration_invalid")
}
fn capture(input: &Input, ledger: Owned) -> Result<Backend, &'static str> {
    let configuration = configuration(input)?;
    let options = PhysicalEntryDebugOptionsV20::new(
        input.limits(),
        SimulationDebugCaptureLimitsV1::new(1, 768, 8, 16384)
            .map_err(|_| "kir_v20_debug_capture_limits")?,
        RECORDS,
    )
    .ok_or("kir_v20_debug_capture_limits")?;
    let session = Session::capture(
        input.module(),
        input.canonical(),
        input.request(),
        options,
        ledger,
    );
    if session.capture_error().is_some()
        || session.outcome() != PhysicalEntryDebugOutcomeV20::Completed
    {
        return Err("kir_v20_debug_capture_refused");
    }
    Ok(Backend {
        session,
        configuration,
        revision: 0,
        terminated: false,
    })
}
fn parse(arguments: Vec<OsString>) -> Result<(PathBuf, PathBuf), &'static str> {
    if arguments.len() > 9 || arguments.first().is_none_or(|s| s != "sim") {
        return Err("kir_v20_debug_arguments");
    }
    let mut kir = None;
    let mut request = None;
    let mut wave = false;
    let mut protocol = false;
    let mut values = arguments.into_iter().skip(1);
    while let Some(option) = values.next() {
        let value = values.next().ok_or("kir_v20_debug_arguments")?;
        if option == "--diagnostic-kir-v20" && kir.is_none() {
            kir = Some(PathBuf::from(value));
        } else if option == "--request" && request.is_none() {
            request = Some(PathBuf::from(value));
        } else if option == "--protocol" && !protocol && value == "jsonl" {
            protocol = true;
        } else if option == "--wave-width" && !wave && value == "64" {
            wave = true;
        } else {
            return Err("kir_v20_debug_option_unavailable");
        }
    }
    Ok((
        kir.ok_or("kir_v20_debug_arguments")?,
        request.ok_or("kir_v20_debug_arguments")?,
    ))
}
pub(super) fn run(arguments: Vec<OsString>) -> ExitCode {
    let result = (|| {
        let (kir, request) = parse(arguments)?;
        let mut ledger = Owned::new(Work::new(WORK), STORAGE);
        // Includes one emergency error response's work before command-budget denial.
        ledger
            .with_budget(|b| {
                b.reserve_storage(SCRATCH)?;
                b.charge_work(RESPONSE * 2)
            })
            .map_err(|_| "kir_v20_debug_protocol_budget")?;
        let (input, receipt) = ledger
            .with_budget(|b| load_physical_entry_debug_input_v20(&kir, &request, b))
            .map_err(|e| e.code())?;
        ledger
            .with_budget(|b| b.reserve_storage(receipt.retained_storage()))
            .map_err(|_| "kir_v20_debug_input_storage")?;
        let mut backend = capture(&input, ledger)?;
        let stdin = io::stdin();
        let stdout = io::stdout();
        let mut reader = BufReader::with_capacity(4096, stdin.lock());
        let mut writer = BufWriter::with_capacity(4096, stdout.lock());
        let result = protocol::run(&mut backend, &mut reader, &mut writer, protocol_limits());
        // Workspace and typed inputs remain owned until all IO/response buffers drop.
        drop(reader);
        drop(writer);
        drop(input);
        let mut ledger = backend.session.into_budget();
        ledger
            .with_budget(|b| b.release_storage(receipt.retained_storage() + SCRATCH))
            .map_err(|_| "kir_v20_debug_teardown_accounting")?;
        result
    })();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => {
            write_bootstrap_error(
                "diagnostic_kir_v20",
                code,
                "typed physical-entry CPU debugger request refused",
            );
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
#[path = "diagnostic_kir_v20_tests.rs"]
mod tests;

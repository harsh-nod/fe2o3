//! Explicit CPU runtime-observation launch profile; legacy launch is unchanged.
use super::*;
use fe2o3_kir_debugger::{
    RuntimeAllocationCaptureLimitsV1, RuntimeAllocationCaptureModeV1, RuntimeFrameCaptureLimitsV1,
    RuntimeFrameCaptureModeV1, RuntimeObservationOptionsV1, RuntimeOriginCaptureLimitsV1,
    RuntimeOriginCaptureModeV1, capture_debugger_observed_run_v1,
    capture_debugger_observed_scheduled_run_v1,
};
use fe2o3_kir_sim::{
    SimulationAllocationReuseV1, SimulationExecutionV1, SimulationScheduleRequestV1,
};

const ORIGIN_ROWS: usize = 65_536;
const ORIGIN_BYTES: usize = 4 * 1024 * 1024;
const FRAME_RECORDS: usize = 65_536;
const FRAME_ROWS: usize = 131_072;
const FRAME_BYTES: usize = 24 * 1024 * 1024;
const ALLOCATION_RECORDS: usize = 65_536;
const ALLOCATION_ROWS: usize = 8_192;
const ALLOCATION_BYTES: usize = 8 * 1024 * 1024;
const VALIDATION_WORK: usize = 1_000_000;
const REUSE_BYTES: usize = 8_192;

fn options() -> Result<RuntimeObservationOptionsV1, String> {
    let origin =
        RuntimeOriginCaptureLimitsV1::new(ORIGIN_ROWS, ORIGIN_BYTES).map_err(|e| e.to_string())?;
    let frames = RuntimeFrameCaptureLimitsV1::new(FRAME_RECORDS, FRAME_ROWS, FRAME_BYTES)
        .map_err(|e| e.to_string())?;
    let allocations = RuntimeAllocationCaptureLimitsV1::new_with_validation_work(
        ALLOCATION_RECORDS,
        ALLOCATION_ROWS,
        ALLOCATION_BYTES,
        VALIDATION_WORK,
    )
    .map_err(|e| e.to_string())?;
    let reuse = SimulationAllocationReuseV1::exact_private_and_workgroup(REUSE_BYTES)
        .map_err(|e| e.to_string())?;
    RuntimeObservationOptionsV1::new(
        RuntimeOriginCaptureModeV1::Enabled(origin),
        RuntimeFrameCaptureModeV1::Enabled(frames),
        RuntimeAllocationCaptureModeV1::Enabled(allocations),
        Some(reuse),
    )
    .map_err(|e| e.to_string())
}
pub(super) fn configuration(base: OpaqueIdentityV1, enabled: bool) -> OpaqueIdentityV1 {
    if !enabled {
        return base;
    }
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-cpu-runtime-observations-profile-v1\0");
    hash.update(base.as_bytes());
    for value in [
        ORIGIN_ROWS,
        ORIGIN_BYTES,
        FRAME_RECORDS,
        FRAME_ROWS,
        FRAME_BYTES,
        ALLOCATION_RECORDS,
        ALLOCATION_ROWS,
        ALLOCATION_BYTES,
        VALIDATION_WORK,
        REUSE_BYTES,
    ] {
        hash.update((value as u64).to_le_bytes());
    }
    nonzero_identity(hash.finalize().into())
}
pub(super) fn capture(
    input: &AdmittedSimulationInputV1,
    width: DebugWaveWidthV1,
    capture_limits: SimulationDebugCaptureLimitsV1,
    debugger_limits: DebuggerLimitsV1,
    schedule: Option<&SimulationScheduleRecordV1>,
    observed: bool,
) -> Result<
    (
        Result<SimulationExecutionV1, SimulationErrorV1>,
        runtime_session_owner_v1::SessionOwnerV1,
    ),
    String,
> {
    if observed {
        if input.module.identity().wire_version() == 19 {
            return Err("runtime observations v1 are not exposed for diagnostic KIR V19".into());
        }
        if matches!(input.module.identity().wire_version(), 16 | 17) {
            return Err(
                "runtime observations v1 are not exposed for diagnostic KIR V16/V17".into(),
            );
        }
        let options = options()?;
        let run = match schedule {
            Some(schedule) => capture_debugger_observed_scheduled_run_v1(
                &input.module,
                &input.request,
                input.simulation_target(),
                input.simulation_limits,
                capture_limits,
                debugger_limits,
                width,
                options,
                SimulationScheduleRequestV1::Replay(schedule),
            ),
            None => capture_debugger_observed_run_v1(
                &input.module,
                &input.request,
                input.simulation_target(),
                input.simulation_limits,
                capture_limits,
                debugger_limits,
                width,
                options,
            ),
        }
        .map_err(|e| e.to_string())?;
        let (execution, transcript) = run.into_parts();
        return Ok((execution, transcript.into_session().into()));
    }
    let run = match schedule {
        Some(schedule) => capture_debugger_replayed_run_v1(
            &input.module,
            &input.request,
            input.simulation_target(),
            input.simulation_limits,
            capture_limits,
            debugger_limits,
            width,
            schedule,
        ),
        None => capture_debugger_run_v1(
            &input.module,
            &input.request,
            input.simulation_target(),
            input.simulation_limits,
            capture_limits,
            debugger_limits,
            width,
        ),
    };
    Ok((run.execution, DebugSessionV1::new(run.transcript).into()))
}

type EmbeddedMap<'a> = (&'a [u8], OpaqueIdentityV1, OpaqueIdentityV1, bool);
pub(super) fn run_cli(
    input: AdmittedSimulationInputV1,
    width: DebugWaveWidthV1,
    mut source_map: Option<AdmittedSourceMapV1>,
    mut source_map_v2: Option<AdmittedSourceMapV2>,
    embedded: Option<EmbeddedMap<'_>>,
    schedule: Option<&SimulationScheduleRecordV1>,
) -> ExitCode {
    let backend = (|| {
        if let Some((bytes, subject, identity, v2)) = embedded {
            if input.simulation_bundle_subject() != Some(subject.as_bytes()) {
                return Err(
                    "embedded source map subject is not retained by the admitted input".to_owned(),
                );
            }
            let configuration = configuration_identity_for_input(&input, width);
            if v2 {
                source_map_v2 = Some(admit_source_map_v2(
                    bytes,
                    &input,
                    configuration,
                    subject,
                    identity,
                )?);
            } else {
                source_map = Some(admit_source_map_with_provenance_v1(
                    bytes,
                    &input,
                    configuration,
                    subject,
                    Some(identity),
                    SourceMapProvenanceV1::CompilerBundleBound,
                )?);
            }
        }
        SimulatorBackendV1::new_with_maps_schedule_and_observations(
            input,
            width,
            source_map,
            source_map_v2,
            schedule,
            true,
        )
    })();
    let backend = match backend {
        Ok(value) => value,
        Err(message) => {
            write_bootstrap_error("backend", "runtime_capture_failed", &message);
            return ExitCode::FAILURE;
        }
    };
    match run_jsonl_v1(
        backend,
        &mut BufReader::new(io::stdin().lock()),
        &mut BufWriter::new(io::stdout().lock()),
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            write_bootstrap_error("output", "protocol_stream_failed", &message);
            ExitCode::FAILURE
        }
    }
}

pub(super) fn write_runtime<W: Write>(
    writer: &mut W,
    response: &RuntimeObservationResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    let bytes = match encode_runtime_observation_response_line_v1(response, limits) {
        Ok(bytes) => bytes,
        Err(ProtocolCodecErrorV1::ResponseTooLarge) => {
            let fallback = RuntimeObservationResponseV1::Error {
                schema: RuntimeObservationResponseSchemaV1::V1,
                request_id: response.request_id(),
                session: response.session(),
                error: output_error(),
            };
            encode_runtime_observation_response_line_v1(&fallback, limits)
                .map_err(|e| e.to_string())?
        }
        Err(error) => return Err(error.to_string()),
    };
    writer
        .write_all(&bytes)
        .and_then(|()| writer.flush())
        .map_err(|e| e.to_string())
}
pub(super) fn write_resource<W: Write>(
    writer: &mut W,
    response: &ResourceResponseV2,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    let bytes = match encode_resource_response_line_v2(response, limits) {
        Ok(bytes) => bytes,
        Err(ProtocolCodecErrorV1::ResponseTooLarge) => {
            let fallback = ResourceResponseV2::Error {
                schema: ResourceResponseSchemaV2::V2,
                request_id: response.request_id(),
                operation: response.operation(),
                session: response.session(),
                error: output_error(),
            };
            encode_resource_response_line_v2(&fallback, limits).map_err(|e| e.to_string())?
        }
        Err(error) => return Err(error.to_string()),
    };
    writer
        .write_all(&bytes)
        .and_then(|()| writer.flush())
        .map_err(|e| e.to_string())
}
fn output_error() -> DebugErrorV1 {
    DebugErrorV1 {
        stage: DebugErrorStageV1::Output,
        code: DebugErrorCodeV1::ResponseTooLarge,
        message: "observed response exceeds the configured JSONL bound".into(),
        state_changed: false,
    }
}

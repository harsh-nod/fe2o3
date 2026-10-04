//! Raw KIR19 supports logical SSA debugging, never serialized source custody.
use super::*;

pub(super) const DIAGNOSIS_UNAVAILABLE: &str = "diagnosis V2 cannot represent raw canonical KIR V19; logical debugger/resource queries remain available";

pub(super) fn require_supported_options(
    wave: DebugWaveWidthV1,
    source_map: bool,
    replay: bool,
) -> Result<(), String> {
    if wave != DebugWaveWidthV1::Wave64 || source_map || replay {
        return Err("diagnostic KIR V19 requires wave64 and does not support source maps or persisted schedule replay".to_owned());
    }
    Ok(())
}

pub(super) fn configuration_identity(
    input: &AdmittedSimulationInputV1,
    wave: DebugWaveWidthV1,
    capture: SimulationDebugCaptureLimitsV1,
    debugger: DebuggerLimitsV1,
) -> Result<OpaqueIdentityV1, String> {
    // Reuse the existing bounded, typed request/limits scanner. Keep the old
    // V17 domain and bytes unchanged; V19 has its own exact local hash domain.
    super::diagnostic_kir_v17::configuration_identity_for_profile(
        input,
        wave,
        capture,
        debugger,
        19,
        b"fe2o3-debug-sim-diagnostic-kir-v19-config-v1\0",
    )
}

#[cfg(all(test, target_os = "linux"))]
#[path = "diagnostic_kir_v19_tests.rs"]
mod tests;

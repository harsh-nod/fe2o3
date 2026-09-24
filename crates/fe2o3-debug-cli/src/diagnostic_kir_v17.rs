//! Raw V17 is diagnostic CPU input, never a serialized source owner.
use super::*;

pub(super) const DIAGNOSIS_UNAVAILABLE: &str = "diagnosis V2 cannot represent raw canonical KIR V17; logical debugger/resource queries remain available";
const MAX_CONFIGURATION_WORK: usize = 64 * 1024 * 1024;

pub(super) fn require_supported_options(
    wave: DebugWaveWidthV1,
    source_map: bool,
    replay: bool,
) -> Result<(), String> {
    if wave != DebugWaveWidthV1::Wave64 || source_map || replay {
        return Err("diagnostic KIR V17 requires wave64 and does not support source maps or persisted schedule replay".to_owned());
    }
    Ok(())
}

// Fixed-width, length-delimited local identity, not a new input or wire schema.
// Raw-byte request metadata alone is insufficient: the public admitted-input
// request and limits may be changed by an embedding caller after admission.
pub(super) fn configuration_identity(
    input: &AdmittedSimulationInputV1,
    wave: DebugWaveWidthV1,
    capture: SimulationDebugCaptureLimitsV1,
    debugger: DebuggerLimitsV1,
) -> Result<OpaqueIdentityV1, String> {
    configuration_identity_for_profile(
        input,
        wave,
        capture,
        debugger,
        17,
        b"fe2o3-debug-sim-diagnostic-kir-v17-config-v1\0",
    )
}

pub(super) fn configuration_identity_for_profile(
    input: &AdmittedSimulationInputV1,
    wave: DebugWaveWidthV1,
    capture: SimulationDebugCaptureLimitsV1,
    debugger: DebuggerLimitsV1,
    version: u16,
    domain: &[u8],
) -> Result<OpaqueIdentityV1, String> {
    if input.module.identity().wire_version() != version {
        return Err(format!(
            "diagnostic configuration requires exact canonical V{version}"
        ));
    }
    input
        .simulation_limits
        .validate()
        .map_err(|error| error.to_string())?;
    let mut state = ConfigurationHash {
        hash: Sha256::new(),
        work: 0,
    };
    // Cover the fixed domain, identities and all limit fields before hashing.
    state.charge(512)?;
    state.hash.update(domain);
    state
        .hash
        .update(input.module.identity().wire_version().to_le_bytes());
    state.hash.update(input.module.identity().digest());
    state
        .hash
        .update(input.module.identity().canonical_length().to_le_bytes());
    state.hash.update(input.request_sha256);
    state.hash.update(input.request_bytes().to_le_bytes());
    state.hash.update(b"little-endian\0");
    state
        .hash
        .update([match input.simulation_target().index_width() {
            IndexWidthV1::Bits32 => 32,
            IndexWidthV1::Bits64 => 64,
        }]);
    state.hash.update(wave.lanes().to_le_bytes());
    let limits = input.simulation_limits;
    for value in [
        limits.max_canonical_bytes,
        limits.max_reachable_functions,
        limits.max_reachable_operations,
        limits.max_call_depth,
        limits.max_ssa_values,
        limits.max_allocations,
        limits.max_allocation_bytes,
        limits.max_total_bytes,
        limits.max_resident_bytes,
        limits.max_memory_access_records,
        capture.max_frames_per_checkpoint(),
        capture.max_values_per_checkpoint(),
        capture.max_allocations_per_checkpoint(),
        capture.max_memory_bytes_per_checkpoint(),
        debugger.max_records(),
        debugger.max_retained_values(),
        debugger.max_retained_memory_bytes(),
    ] {
        state.length(value)?;
    }
    for value in [
        limits.max_invocations,
        limits.max_workgroups,
        limits.max_scheduled_slots,
        limits.max_steps,
        limits.max_events,
        MAX_SESSION_COMMANDS_V1,
    ] {
        state.hash.update(value.to_le_bytes());
    }
    state.request(&input.request)?;
    Ok(nonzero_identity(state.hash.finalize().into()))
}

struct ConfigurationHash {
    hash: Sha256,
    work: usize,
}
impl ConfigurationHash {
    fn charge(&mut self, amount: usize) -> Result<(), String> {
        let total = self
            .work
            .checked_add(amount)
            .filter(|total| *total <= MAX_CONFIGURATION_WORK)
            .ok_or_else(|| {
                "diagnostic configuration scan exceeds its logical work bound".to_owned()
            })?;
        self.work = total;
        Ok(())
    }
    fn length(&mut self, value: usize) -> Result<(), String> {
        let value = u64::try_from(value)
            .map_err(|_| "diagnostic configuration length overflow".to_owned())?;
        self.hash.update(value.to_le_bytes());
        Ok(())
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.charge(
            bytes
                .len()
                .checked_add(8)
                .ok_or_else(|| "diagnostic configuration byte count overflow".to_owned())?,
        )?;
        self.length(bytes.len())?;
        self.hash.update(bytes);
        Ok(())
    }
    fn buffer(&mut self, buffer: &fe2o3_kir_sim::BufferArgumentV1) -> Result<(), String> {
        self.charge(16)?;
        self.hash
            .update([scalar_tag(buffer.element()), access_tag(buffer.access())]);
        self.hash.update(buffer.alignment().to_le_bytes());
        self.bytes(buffer.bytes())?;
        self.charge(
            buffer
                .initialized()
                .len()
                .checked_add(8)
                .ok_or_else(|| "diagnostic initialization count overflow".to_owned())?,
        )?;
        self.length(buffer.initialized().len())?;
        for initialized in buffer.initialized() {
            self.hash.update([u8::from(*initialized)]);
        }
        Ok(())
    }
    fn request(&mut self, request: &fe2o3_kir_sim::SimulationRequestV1) -> Result<(), String> {
        self.charge(64)?;
        self.bytes(request.kernel.as_str().as_bytes())?;
        for value in request.grid.0 {
            self.hash.update(value.to_le_bytes());
        }
        for value in request.workgroup.0 {
            self.hash.update(value.to_le_bytes());
        }
        self.hash.update([match request.events {
            fe2o3_kir_sim::EventPolicyV1::Disabled => 0,
            fe2o3_kir_sim::EventPolicyV1::Enabled => 1,
        }]);
        self.length(request.arguments.len())?;
        for argument in &request.arguments {
            self.charge(64)?;
            match argument {
                SimulationArgumentV1::Scalar(value) => {
                    self.hash.update([0, scalar_tag(value.ty())]);
                    self.hash.update(value.bits().to_le_bytes());
                }
                SimulationArgumentV1::Buffer(buffer) => {
                    self.hash.update([1]);
                    self.buffer(buffer)?;
                }
                SimulationArgumentV1::BufferView(view) => {
                    self.hash
                        .update([2, scalar_tag(view.element()), access_tag(view.access())]);
                    self.hash.update(view.backing().0.to_le_bytes());
                    self.hash.update(view.alignment().to_le_bytes());
                    self.length(view.byte_offset())?;
                    self.length(view.elements())?;
                }
            }
        }
        self.length(request.shared_buffers.len())?;
        for shared in &request.shared_buffers {
            self.charge(8)?;
            self.hash.update(shared.id.0.to_le_bytes());
            self.buffer(&shared.buffer)?;
        }
        Ok(())
    }
}

const fn access_tag(access: AccessMode) -> u8 {
    match access {
        AccessMode::ReadOnly => 0,
        AccessMode::WriteOnly => 1,
        AccessMode::ReadWrite => 2,
    }
}
const fn scalar_tag(scalar: ScalarType) -> u8 {
    match scalar {
        ScalarType::Bool => 0,
        ScalarType::I8 => 1,
        ScalarType::I16 => 2,
        ScalarType::I32 => 3,
        ScalarType::I64 => 4,
        ScalarType::I128 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::U128 => 10,
        ScalarType::Index => 11,
        ScalarType::F16 => 12,
        ScalarType::Bf16 => 13,
        ScalarType::F32 => 14,
        ScalarType::F64 => 15,
    }
}

#[cfg(all(test, target_os = "linux"))]
#[path = "diagnostic_kir_v17_tests.rs"]
mod tests;

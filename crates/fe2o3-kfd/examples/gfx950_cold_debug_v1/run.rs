//! Explicit one-shot preparation; no trap/runtime/queue/dispatch activation.
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use fe2o3_kfd::{DeviceSelector, Gfx950DebugColdOwnerV1, OpenedKfd};
use serde::Serialize;

#[path = "retained.rs"]
mod retained;
use retained::Retained;

const USAGE: &str = "observe_gfx950_cold_debug_v1 --allow-vm-mapping --retain-until-process-exit HSACO BYTES SHA256 KERNEL NODE UNIQUE_ID GPU_ID DEVICE_PROFILE_SHA256";
const SCHEMA: &str = "diagnostic-gfx950-cold-debug-preparation-v1";
const MAX_ARGUMENT_BYTES: usize = 4096;
const MAX_OUTPUT_BYTES: usize = 4096;

#[derive(Debug, Serialize, Eq, PartialEq)]
struct Failure {
    phase: &'static str,
    reason: &'static str,
    native_preparation_effects: &'static str,
}
impl Failure {
    const fn new(phase: &'static str, reason: &'static str) -> Self {
        Self {
            phase,
            reason,
            native_preparation_effects: "not_attempted",
        }
    }
    fn possible(mut self) -> Self {
        self.native_preparation_effects = "possible_retained_until_process_exit";
        self
    }
}
#[derive(Debug)]
struct Options {
    path: PathBuf,
    bytes: usize,
    sha256: [u8; 32],
    kernel: String,
    node: u32,
    unique_id: u64,
    gpu_id: u32,
    profile: [u8; 32],
}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}
fn hash(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let nibble = |b| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    };
    let mut result = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        result[index] = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Some(result)
}
fn parse(arguments: impl Iterator<Item = OsString>) -> Result<Options, Failure> {
    let fail = || {
        Failure::new(
            "arguments",
            "closed_grammar_and_explicit_effect_acknowledgments",
        )
    };
    let mut values = Vec::with_capacity(10);
    for value in arguments {
        if values.len() == 10 || value.as_os_str().as_bytes().len() > MAX_ARGUMENT_BYTES {
            return Err(fail());
        }
        values.push(value.into_string().map_err(|_| fail())?);
    }
    let [
        mapping,
        retention,
        path,
        bytes,
        sha,
        kernel,
        node,
        unique,
        gpu,
        profile,
    ]: [String; 10] = values.try_into().map_err(|_| fail())?;
    if mapping != "--allow-vm-mapping" || retention != "--retain-until-process-exit" {
        return Err(fail());
    }
    let bytes = usize::try_from(decimal(&bytes).ok_or_else(fail)?).map_err(|_| fail())?;
    let path = PathBuf::from(path);
    if bytes == 0
        || bytes > fe2o3_hsaco::MAX_HSACO_BYTES
        || !path.is_absolute()
        || kernel.is_empty()
        || kernel.len() > 128
        || kernel
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(fail());
    }
    Ok(Options {
        path,
        bytes,
        sha256: hash(&sha).ok_or_else(fail)?,
        kernel,
        node: u32::try_from(decimal(&node).ok_or_else(fail)?).map_err(|_| fail())?,
        unique_id: decimal(&unique)
            .filter(|value| *value != 0)
            .ok_or_else(fail)?,
        gpu_id: u32::try_from(decimal(&gpu).filter(|value| *value != 0).ok_or_else(fail)?)
            .map_err(|_| fail())?,
        profile: hash(&profile).ok_or_else(fail)?,
    })
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[usize::from(byte >> 4)] as char);
        value.push(DIGITS[usize::from(byte & 15)] as char);
    }
    value
}

#[derive(Serialize)]
struct Observation {
    target: &'static str,
    wave_width: u32,
    node: u32,
    unique_id: String,
    gpu_id: u32,
    device_profile_sha256: String,
    artifact_sha256: String,
    artifact_bytes: String,
    selected_kernel: String,
    trap_sha256: String,
    trap_bytes: usize,
    mapped_backing_bytes: String,
    metadata_retained_bytes: String,
    file_snapshot_rechecked: bool,
    native_vm_and_mappings_prepared: bool,
    retention: &'static str,
    metadata_version: u32,
    metadata_published: bool,
    trap_registered: bool,
    debug_runtime_enabled: bool,
    queue_created: bool,
    kernel_dispatched: bool,
    gpu_trap_execution_qualified: bool,
    cleanup_acknowledged: bool,
    authority: &'static str,
}

fn observe(options: Options) -> Result<Observation, Failure> {
    let retained = Retained::open(&options.path, options.bytes, options.sha256)?;
    {
        let _closure = fe2o3_amdhsa_loader::validate(
            retained.bytes(),
            fe2o3_amdhsa_loader::AdmittedProfile::Gfx950XnackOffCov6,
        )
        .map_err(|_| Failure::new("artifact_admission", "normal_loader_refused"))?
        .bind_kernel(&options.kernel)
        .map_err(|_| Failure::new("artifact_admission", "normal_kernel_selection_refused"))?;
    }
    let mut object = Vec::new();
    object
        .try_reserve_exact(options.bytes)
        .map_err(|_| Failure::new("artifact_copy", "allocation_failed"))?;
    object.extend_from_slice(retained.bytes());
    retained.recheck()?;

    let mut device = OpenedKfd::open_default()
        .map_err(|_| Failure::new("device_admission", "open_kfd"))?
        .admit_uapi()
        .map_err(|_| Failure::new("device_admission", "uapi"))?
        .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(options.unique_id))
        .map_err(|_| Failure::new("device_admission", "gfx950_binding"))?;
    device
        .check_observable_currentness()
        .map_err(|_| Failure::new("device_currentness", "device_not_current"))?;
    if device.observation().topology_node_id() != options.node
        || device.observation().unique_id() != options.unique_id
        || device.observation().kfd_gpu_id() != options.gpu_id
        || device.observation_profile_sha256_v1() != options.profile
    {
        return Err(Failure::new(
            "device_selection",
            "exact_identity_or_profile_mismatch",
        ));
    }
    retained.recheck()?;
    device
        .check_observable_currentness()
        .map_err(|_| Failure::new("device_currentness", "device_not_current"))?;

    // A failure after this boundary may retain mappings even if prepare refuses.
    // Never retry in the same process or describe such refusal as clean teardown.
    let owner = Gfx950DebugColdOwnerV1::prepare(device, object, options.kernel.clone())
        .map_err(|_| Failure::new("cold_preparation", "owner_refused").possible())?;
    let facts = owner.facts();
    retained.recheck().map_err(Failure::possible)?;
    if facts.artifact_sha256() != options.sha256
        || facts.artifact_bytes() != options.bytes
        || facts.trap_bytes() != 1116
        || facts.trap_sha256()
            != hash("4ffea893ee53e018a629c19254855721882444517155251f1f45dd3519bc81fe")
                .ok_or_else(|| Failure::new("facts", "fixed_trap_pin").possible())?
    {
        return Err(Failure::new("facts", "preparation_identity_mismatch").possible());
    }
    let result = Observation {
        target: "gfx950:xnack-",
        wave_width: 64,
        node: options.node,
        unique_id: options.unique_id.to_string(),
        gpu_id: options.gpu_id,
        device_profile_sha256: hex(&options.profile),
        artifact_sha256: hex(&facts.artifact_sha256()),
        artifact_bytes: facts.artifact_bytes().to_string(),
        selected_kernel: options.kernel,
        trap_sha256: hex(&facts.trap_sha256()),
        trap_bytes: facts.trap_bytes(),
        mapped_backing_bytes: facts.mapped_backing_bytes().to_string(),
        metadata_retained_bytes: facts.metadata_retained_bytes().to_string(),
        file_snapshot_rechecked: true,
        native_vm_and_mappings_prepared: true,
        retention: "native_resources_retained_until_process_exit",
        metadata_version: 0,
        metadata_published: false,
        trap_registered: false,
        debug_runtime_enabled: false,
        queue_created: false,
        kernel_dispatched: false,
        gpu_trap_execution_qualified: false,
        cleanup_acknowledged: false,
        authority: "inactive_preparation_facts_only",
    };
    // Inactive Drop retains actual native custody and poisons the process gate.
    // The supervisor, not this JSON, must establish process exit/reaping.
    drop(owner);
    Ok(result)
}

pub fn main() -> ExitCode {
    let result = parse(std::env::args_os().skip(1)).and_then(observe);
    let status = if result.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    };
    let value = match result {
        Ok(observation) => serde_json::json!({
            "schema": SCHEMA, "status": "prepared_inactive", "observation": observation,
        }),
        Err(failure) => serde_json::json!({
            "schema": SCHEMA, "status": "refused", "failure": failure, "usage": USAGE,
        }),
    };
    let Ok(mut bytes) = serde_json::to_vec(&value) else {
        return ExitCode::FAILURE;
    };
    if bytes.len() >= MAX_OUTPUT_BYTES {
        return ExitCode::FAILURE;
    }
    bytes.push(b'\n');
    let mut output = std::io::stdout().lock();
    if output
        .write_all(&bytes)
        .and_then(|()| output.flush())
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    status
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

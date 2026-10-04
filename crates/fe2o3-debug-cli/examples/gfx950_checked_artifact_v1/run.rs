use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::ExitCode;

use fe2o3_debug_cli::rocgdb_checked_gfx950_target_v1::{
    RocgdbCheckedGfx950ArtifactErrorV1, RocgdbCheckedGfx950ArtifactV1,
};
use fe2o3_kfd::{DeviceSelector, OpenedKfd};
use serde::Serialize;

#[path = "retained.rs"]
mod retained;
use retained::Retained;

const SCHEMA: &str = "diagnostic-gfx950-checked-artifact-v1";
const USAGE: &str =
    "observe_gfx950_checked_artifact_v1 HSACO BYTES SHA256 KERNEL NODE UNIQUE_ID LOAD_BASE";
const MAX_ARGUMENT_BYTES: usize = 4096;
const MAX_KERNEL_BYTES: usize = 128;
const MAX_OUTPUT_BYTES: usize = 4096;

#[derive(Debug, Serialize, Eq, PartialEq)]
struct Failure {
    phase: &'static str,
    reason: &'static str,
}
impl Failure {
    const fn new(phase: &'static str, reason: &'static str) -> Self {
        Self { phase, reason }
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
    load_base: u64,
}

fn decimal(value: &str) -> Option<u64> {
    if value.is_empty()
        || value.len() > 20
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    value.parse().ok()
}
fn hash(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let nibble = |b: u8| match b {
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
    let fail = || Failure::new("arguments", "closed_positional_grammar");
    let mut values = Vec::with_capacity(7);
    for value in arguments {
        if values.len() == 7 || value.as_os_str().as_bytes().len() > MAX_ARGUMENT_BYTES {
            return Err(fail());
        }
        values.push(value.into_string().map_err(|_| fail())?);
    }
    let [path, bytes, sha, kernel, node, unique, base]: [String; 7] =
        values.try_into().map_err(|_| fail())?;
    let bytes = usize::try_from(decimal(&bytes).ok_or_else(fail)?).map_err(|_| fail())?;
    if bytes == 0
        || bytes > fe2o3_hsaco::MAX_HSACO_BYTES
        || kernel.is_empty()
        || kernel.len() > MAX_KERNEL_BYTES
        || kernel.bytes().any(|b| b == 0 || b.is_ascii_control())
    {
        return Err(fail());
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(fail());
    }
    Ok(Options {
        path,
        bytes,
        sha256: hash(&sha).ok_or_else(fail)?,
        kernel,
        node: u32::try_from(decimal(&node).ok_or_else(fail)?).map_err(|_| fail())?,
        unique_id: decimal(&unique).filter(|n| *n != 0).ok_or_else(fail)?,
        load_base: decimal(&base).ok_or_else(fail)?,
    })
}
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        result.push(DIGITS[usize::from(b >> 4)] as char);
        result.push(DIGITS[usize::from(b & 15)] as char);
    }
    result
}
fn companion_failure(phase: &'static str, error: RocgdbCheckedGfx950ArtifactErrorV1) -> Failure {
    use RocgdbCheckedGfx950ArtifactErrorV1 as E;
    let reason = match error {
        E::InputBound => "input_bound",
        E::DeviceNotCurrent => "device_not_current",
        E::ArtifactInspection => "artifact_inspection",
        E::ArtifactTarget => "artifact_target",
        E::KernelSelection => "kernel_selection",
        E::KernelWaveWidth => "kernel_wave_width",
        E::ArtifactIdentity => "artifact_identity",
        E::CodeBinding => "code_binding",
        E::ReinspectionMismatch => "reinspection_mismatch",
    };
    Failure::new(phase, reason)
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
    caller_load_base: String,
    load_base_role: &'static str,
    explicit_revalidations: u8,
    file_snapshot_rechecked: bool,
    authority: &'static str,
    explicit_vm_acquisition: bool,
    queue: bool,
    dispatch: bool,
    attach: bool,
    physical_registers: bool,
}
fn observe(options: Options) -> Result<Observation, Failure> {
    // File size/hash/custody refusals happen before opening the KFD device.
    let retained = Retained::open(&options.path, options.bytes, options.sha256)?;
    let mut device = OpenedKfd::open_default()
        .map_err(|_| Failure::new("device_admission", "open_kfd"))?
        .admit_uapi()
        .map_err(|_| Failure::new("device_admission", "uapi"))?
        .bind_gfx950_xnack_minus(DeviceSelector::UniqueId(options.unique_id))
        .map_err(|_| Failure::new("device_admission", "gfx950_binding"))?;
    if device.observation().topology_node_id() != options.node
        || device.observation().unique_id() != options.unique_id
    {
        return Err(Failure::new(
            "device_selection",
            "node_or_unique_id_mismatch",
        ));
    }
    let gpu_id = device.observation().kfd_gpu_id();
    let profile = device.observation_profile_sha256_v1();
    let artifact = {
        let mut checked = RocgdbCheckedGfx950ArtifactV1::inspect(
            &mut device,
            retained.bytes(),
            &options.kernel,
            options.load_base,
        )
        .map_err(|e| companion_failure("companion_inspection", e))?;
        for _ in 0..2 {
            checked
                .revalidate()
                .map_err(|e| companion_failure("companion_revalidation", e))?;
        }
        checked.artifact()
    };
    if artifact.digest.as_bytes() != options.sha256
        || artifact.canonical_bytes != options.bytes as u64
    {
        return Err(Failure::new(
            "companion_identity",
            "expected_artifact_mismatch",
        ));
    }
    retained.recheck()?;
    device
        .check_observable_currentness()
        .map_err(|_| Failure::new("final_device_currentness", "device_not_current"))?;
    Ok(Observation {
        target: "gfx950:xnack-",
        wave_width: 64,
        node: options.node,
        unique_id: options.unique_id.to_string(),
        gpu_id,
        device_profile_sha256: hex(&profile),
        artifact_sha256: hex(&options.sha256),
        artifact_bytes: options.bytes.to_string(),
        selected_kernel: options.kernel,
        caller_load_base: options.load_base.to_string(),
        load_base_role: "caller_admission_only",
        explicit_revalidations: 2,
        file_snapshot_rechecked: true,
        authority: "checked_observation_only",
        explicit_vm_acquisition: false,
        queue: false,
        dispatch: false,
        attach: false,
        physical_registers: false,
    })
}
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum ResultRow {
    Observed { observation: Observation },
    Refused { failure: Failure },
}
#[derive(Serialize)]
struct Output {
    schema: &'static str,
    result: ResultRow,
}

pub(super) fn main() -> ExitCode {
    let result = parse(std::env::args_os().skip(1)).and_then(observe);
    let success = result.is_ok();
    let row = match result {
        Ok(observation) => ResultRow::Observed { observation },
        Err(failure) => {
            if failure.phase == "arguments" {
                eprintln!("{USAGE}");
            }
            ResultRow::Refused { failure }
        }
    };
    let Ok(bytes) = serde_json::to_vec(&Output {
        schema: SCHEMA,
        result: row,
    }) else {
        return ExitCode::FAILURE;
    };
    if bytes.len() > MAX_OUTPUT_BYTES {
        return ExitCode::FAILURE;
    }
    let mut output = std::io::stdout().lock();
    if output
        .write_all(&bytes)
        .and_then(|()| output.write_all(b"\n"))
        .is_err()
    {
        return ExitCode::FAILURE;
    }
    if success {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod linux;
mod schema;

use std::path::Path;
use std::process::ExitCode;

use fe2o3_kir_sim::{AdmittedSimulationModuleV1, SimulationLimitsV1, SimulationRequestV1};

/// Exact, strictly parsed simulator inputs admitted through the standalone
/// command's hardened file boundary.
#[derive(Debug)]
pub struct AdmittedSimulationInputV1 {
    pub module: AdmittedSimulationModuleV1,
    pub request: SimulationRequestV1,
    pub simulation_limits: SimulationLimitsV1,
    pub kir_sha256: [u8; 32],
    pub request_sha256: [u8; 32],
}

/// Bounded failure returned while securely loading debugger simulator inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SimulationInputErrorV1 {
    pub stage: String,
    pub code: String,
    pub message: String,
}

impl std::fmt::Display for SimulationInputErrorV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "simulation input admission failed at {} ({}): {}",
            self.stage, self.code, self.message
        )
    }
}

impl std::error::Error for SimulationInputErrorV1 {}

#[cfg(not(target_os = "linux"))]
use std::io::Write as _;

/// Runs the standalone command-line boundary with the process argument vector.
pub fn main() -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        linux::main()
    }
    #[cfg(not(target_os = "linux"))]
    {
        unsupported_platform()
    }
}

/// Executes an already captured exact canonical KIR V7 image with the same
/// request, result, error, and output-publication boundary as the standalone
/// CLI. The caller supplies inert compiler-custody bytes, not a path that can
/// be substituted after capture.
pub fn run_captured_kir_v7(
    canonical_kir_v7: &[u8],
    request: &Path,
    output: Option<&Path>,
) -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        linux::run_captured_kir_v7(
            canonical_kir_v7,
            request.as_os_str().to_owned(),
            None,
            output.map(|path| path.as_os_str().to_owned()),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (canonical_kir_v7, request, output);
        unsupported_platform()
    }
}

/// Executes captured KIR only if a secure reread has the same length and exact
/// bytes admitted before the build. Byte-identical pathname or inode
/// replacement is content-equivalent and remains admissible.
pub fn run_captured_kir_v7_with_bound_request(
    canonical_kir_v7: &[u8],
    request: &Path,
    expected_request: SimulationRequestIdentityV1,
    output: Option<&Path>,
) -> ExitCode {
    #[cfg(target_os = "linux")]
    {
        linux::run_captured_kir_v7(
            canonical_kir_v7,
            request.as_os_str().to_owned(),
            Some(expected_request),
            output.map(|path| path.as_os_str().to_owned()),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (canonical_kir_v7, request, expected_request, output);
        unsupported_platform()
    }
}

#[cfg(not(target_os = "linux"))]
fn unsupported_platform() -> ExitCode {
    #[derive(serde::Serialize)]
    struct PlatformError {
        schema: &'static str,
        status: &'static str,
        stage: schema::Stage,
        kind: schema::ErrorKind,
        message: &'static str,
    }
    let error = PlatformError {
        schema: "fe2o3-simulation-error-v1",
        status: "error",
        stage: schema::Stage::Platform,
        kind: schema::ErrorKind::UnsupportedPlatform,
        message: "fe2o3-kir-sim requires Linux openat2, O_TMPFILE, procfs fd links, and linkat",
    };
    let mut stderr = std::io::stderr().lock();
    let _ = serde_json::to_writer(&mut stderr, &error);
    let _ = stderr.write_all(b"\n");
    ExitCode::FAILURE
}
/// Exact pre-build identity of one admitted simulation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimulationRequestIdentityV1 {
    sha256: [u8; 32],
    length: usize,
}

impl SimulationRequestIdentityV1 {
    /// Returns the SHA-256 of the exact strict request bytes.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact request byte length.
    pub const fn length(self) -> usize {
        self.length
    }
}

/// Securely reads and strictly admits the request before a source build starts.
pub fn bind_request_v1(path: &Path) -> Result<SimulationRequestIdentityV1, String> {
    #[cfg(target_os = "linux")]
    {
        linux::bind_request_v1(path.as_os_str().to_owned())
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err("fe2o3 simulation request binding requires Linux".to_owned())
    }
}

/// Securely captures, strictly parses, and admits one exact KIR V7 image and
/// simulation request for a debugger session. This is the same ingestion path
/// used by `fe2o3-kir-sim`; callers never parse either document themselves.
pub fn load_debug_simulation_input_v1(
    kir_v7: &Path,
    request: &Path,
) -> Result<AdmittedSimulationInputV1, SimulationInputErrorV1> {
    #[cfg(target_os = "linux")]
    {
        linux::load_debug_simulation_input_v1(
            kir_v7.as_os_str().to_owned(),
            request.as_os_str().to_owned(),
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (kir_v7, request);
        Err(SimulationInputErrorV1 {
            stage: "platform".to_owned(),
            code: "unsupported_platform".to_owned(),
            message: "fe2o3 debugger simulation input admission requires Linux".to_owned(),
        })
    }
}

/// Securely captures an inert, regular-file debugger sidecar with a caller
/// supplied hard bound. The returned bytes have no pathname authority.
pub fn load_debug_sidecar_v1(
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, SimulationInputErrorV1> {
    if maximum == 0 || maximum > 4 * 1024 * 1024 {
        return Err(SimulationInputErrorV1 {
            stage: "arguments".to_owned(),
            code: "invalid_command_line".to_owned(),
            message: "debug sidecar bound must be between 1 byte and 4 MiB".to_owned(),
        });
    }
    #[cfg(target_os = "linux")]
    {
        linux::load_debug_sidecar_v1(path.as_os_str().to_owned(), maximum)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        Err(SimulationInputErrorV1 {
            stage: "platform".to_owned(),
            code: "unsupported_platform".to_owned(),
            message: "fe2o3 debugger sidecar admission requires Linux".to_owned(),
        })
    }
}

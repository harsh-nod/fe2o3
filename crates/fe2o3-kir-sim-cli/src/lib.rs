#![forbid(unsafe_code)]

#[cfg(target_os = "linux")]
mod linux;
mod schema;

use std::path::Path;
use std::process::ExitCode;

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

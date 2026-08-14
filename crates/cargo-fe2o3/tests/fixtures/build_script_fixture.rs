use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const REPORT_ENV: &str = "FE2O3_TEST_BUILD_SCRIPT_REPORT";

fn main() -> ExitCode {
    match env::args_os().nth(1).as_deref() {
        Some(mode) if mode == "ordinary" => ordinary_child(),
        Some(mode) if mode == "exec-wrapper" => exec_wrapper(),
        Some(mode) if mode == "execveat-wrapper" => execveat_wrapper(),
        Some(mode) if mode == "--crate-name" => forged_compiler(),
        _ => {
            eprintln!("build-script fixture received an unsupported mode");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "linux")]
fn execveat_wrapper() -> ExitCode {
    use std::ffi::CString;
    use std::os::fd::AsRawFd;
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        static mut environ: *mut *mut libc::c_char;
    }

    let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
    let compiler = env::current_exe().expect("locate fixture executable");
    let source = required_path(REPORT_ENV).with_extension("rs");
    fs::write(&source, "pub fn replayed() {}\n").expect("write replay source");
    let wrapper = fs::File::open(&wrapper).expect("open inherited wrapper");
    let arguments = [
        b"cargo-fe2o3".as_slice(),
        compiler.as_os_str().as_bytes(),
        b"--crate-name",
        b"replayed_execveat_build_script",
        b"--crate-type",
        b"lib",
        b"--emit=metadata",
        b"-Cmetadata=replayed-execveat-build-script",
        source.as_os_str().as_bytes(),
    ]
    .map(|argument| CString::new(argument).expect("argument has no NUL"));
    let mut argv = arguments
        .iter()
        .map(|argument| argument.as_ptr())
        .collect::<Vec<_>>();
    argv.push(std::ptr::null());
    let empty = c"";
    // SAFETY: all argument strings, the null-terminated pointer array, the open executable, and
    // the inherited environment remain live through the replacing execveat syscall.
    let result = unsafe {
        libc::syscall(
            libc::SYS_execveat,
            wrapper.as_raw_fd(),
            empty.as_ptr(),
            argv.as_ptr(),
            environ,
            libc::AT_EMPTY_PATH,
        )
    };
    eprintln!(
        "build-script fixture execveat returned {result}: {}",
        std::io::Error::last_os_error()
    );
    ExitCode::FAILURE
}

#[cfg(not(target_os = "linux"))]
fn execveat_wrapper() -> ExitCode {
    ExitCode::FAILURE
}

fn ordinary_child() -> ExitCode {
    let (backend_open, artifact_open) = descriptor_state();
    if let Err(error) = write_report("ordinary", backend_open, artifact_open) {
        eprintln!("ordinary build-script probe failed: {error}");
        return ExitCode::FAILURE;
    }
    if backend_open || artifact_open {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

#[cfg(unix)]
fn exec_wrapper() -> ExitCode {
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    let wrapper = env::var_os("RUSTC_WORKSPACE_WRAPPER")
        .unwrap_or_else(|| panic!("missing RUSTC_WORKSPACE_WRAPPER"));
    let source = required_path(REPORT_ENV).with_extension("rs");
    fs::write(&source, "pub fn replayed() {}\n").expect("write replay source");
    let error = Command::new(wrapper)
        .arg(env::current_exe().expect("locate fixture executable"))
        .args([
            "--crate-name",
            "replayed_build_script",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-Cmetadata=replayed-build-script",
        ])
        .arg(source)
        .exec();
    eprintln!("build-script fixture could not exec inherited wrapper: {error}");
    ExitCode::FAILURE
}

#[cfg(not(unix))]
fn exec_wrapper() -> ExitCode {
    ExitCode::FAILURE
}

fn forged_compiler() -> ExitCode {
    let (backend_open, artifact_open) = descriptor_state();
    if let Err(error) = write_report("exec-wrapper", backend_open, artifact_open) {
        eprintln!("replayed compiler probe failed: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn descriptor_state() -> (bool, bool) {
    (
        fs::symlink_metadata("/proc/self/fd/198").is_ok(),
        fs::symlink_metadata("/proc/self/fd/197").is_ok(),
    )
}

fn write_report(mode: &str, backend_open: bool, artifact_open: bool) -> Result<(), String> {
    fs::write(
        required_path(REPORT_ENV),
        format!("mode={mode}\nbackend_open={backend_open}\nartifact_open={artifact_open}\n"),
    )
    .map_err(|error| format!("write descriptor report: {error}"))
}

fn required_path(variable: &str) -> PathBuf {
    PathBuf::from(env::var_os(variable).unwrap_or_else(|| panic!("missing {variable}")))
}

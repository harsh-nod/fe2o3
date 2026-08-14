use std::env;
use std::ffi::{OsStr, OsString};
#[cfg(target_os = "linux")]
use std::fs::File;
use std::fs::{self, OpenOptions};
#[cfg(unix)]
use std::io::Read;
use std::io::Write;
#[cfg(target_os = "linux")]
use std::os::fd::AsRawFd;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    if env::var_os("FE2O3_TEST_EXPECT_CALLER_LOADER_ENV_SCRUBBED_V1").is_some() {
        for (name, value) in env::vars_os() {
            let name = name.to_string_lossy();
            let is_loader =
                name.starts_with("LD_") || name.starts_with("DYLD_") || name == "GLIBC_TUNABLES";
            let is_managed_rustc_runtime =
                name == "LD_LIBRARY_PATH" && value == OsStr::new("/proc/self/fd/193");
            if is_loader && !is_managed_rustc_runtime {
                eprintln!("fake Cargo inherited caller dynamic-loader variable {name}");
                return ExitCode::FAILURE;
            }
        }
    }
    let args = env::args_os().collect::<Vec<_>>();
    if args.get(1).is_none_or(|argument| argument != "config")
        && let Err(error) = record_invocation(&args)
    {
        eprintln!("fake Cargo could not record invocation: {error}");
        return ExitCode::FAILURE;
    }

    match args.get(1).and_then(|argument| argument.to_str()) {
        Some("metadata") => metadata(&args),
        Some("config") => config_get(&args),
        Some("build") if is_backend_build(&args) => backend_build(&args),
        Some("build" | "run") => build_or_run(&args),
        Some(other) => {
            eprintln!("fake Cargo received unexpected subcommand {other:?}");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("fake Cargo received no subcommand");
            ExitCode::FAILURE
        }
    }
}

fn config_get(args: &[OsString]) -> ExitCode {
    let key = args
        .last()
        .and_then(|argument| argument.to_str())
        .unwrap_or("");
    let value = if key == "build.target" {
        env::var_os("FE2O3_TEST_BUILD_TARGET_JSON")
    } else if key == "build.rustc-wrapper" {
        env::var_os("FE2O3_TEST_RUSTC_WRAPPER_JSON")
    } else if key == "build.rustc-workspace-wrapper" {
        env::var_os("FE2O3_TEST_RUSTC_WORKSPACE_WRAPPER_JSON")
    } else if key == "build.rustc" {
        env::var_os("FE2O3_TEST_RUSTC_JSON")
    } else if key == "build.rustflags" {
        env::var_os("FE2O3_TEST_BUILD_RUSTFLAGS_JSON")
    } else if key == "env" {
        env::var_os("FE2O3_TEST_ENV_CONFIG_JSON")
    } else if key == "build" {
        env::var_os("FE2O3_TEST_BUILD_CONFIG_JSON")
    } else if key.starts_with("target.") && key.ends_with(".runner") {
        env::var_os("FE2O3_TEST_CONFIG_RUNNER_JSON")
    } else if key == "target" {
        env::var_os("FE2O3_TEST_TARGET_TABLE_JSON")
    } else if key == "profile" {
        env::var_os("FE2O3_TEST_PROFILE_CONFIG_JSON")
    } else {
        None
    };
    if let Some(value) = value {
        println!("{}", value.to_string_lossy());
        ExitCode::SUCCESS
    } else {
        eprintln!("error: config value `{key}` is not set");
        ExitCode::from(101)
    }
}

fn is_backend_build(args: &[OsString]) -> bool {
    args.windows(2)
        .any(|pair| pair[0] == "-p" && pair[1] == "rustc-codegen-fe2o3")
}

fn backend_build(args: &[OsString]) -> ExitCode {
    let Some(index) = args.iter().position(|argument| argument == "--target-dir") else {
        eprintln!("fake backend build has no isolated target directory");
        return ExitCode::FAILURE;
    };
    let Some(target) = args.get(index + 1).map(PathBuf::from) else {
        eprintln!("fake backend build target directory has no value");
        return ExitCode::FAILURE;
    };
    let output = target.join("debug/librustc_codegen_fe2o3.so");
    if let Err(error) = fs::create_dir_all(output.parent().expect("backend output parent"))
        .and_then(|()| fs::write(&output, b"isolated built backend"))
    {
        eprintln!("fake backend build could not create output: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn metadata(args: &[OsString]) -> ExitCode {
    if let Some(report) = env::var_os("FE2O3_TEST_AUTHORITY_PREFLIGHT_REPORT") {
        let mut observation = format!(
            "frozen={}\noffline={}\nrustc={}\n",
            args.iter().any(|argument| argument == "--frozen"),
            args.iter().any(|argument| argument == "--offline"),
            env::var_os("RUSTC").unwrap_or_default().to_string_lossy()
        );
        for name in [
            "PATH",
            "HOME",
            "CARGO_HOME",
            "GIT_CONFIG_GLOBAL",
            "SSH_AUTH_SOCK",
            "CARGO_REGISTRIES_CRATES_IO_TOKEN",
        ] {
            observation.push_str(&format!("{name}={:?}\n", env::var_os(name)));
        }
        #[cfg(target_os = "linux")]
        observation.push_str(&format!(
            "rustc_fd={}\nlib_tree_fd={}\n",
            fs::symlink_metadata("/proc/self/fd/194").is_ok(),
            fs::symlink_metadata("/proc/self/fd/193").is_ok(),
        ));
        if let Err(error) = fs::write(report, observation) {
            eprintln!("fake Cargo could not record authority preflight: {error}");
            return ExitCode::FAILURE;
        }
    }
    let workspace_root = required_path("FE2O3_TEST_WORKSPACE_ROOT");
    let target_directory = required_path("FE2O3_TEST_TARGET_DIRECTORY");
    let record = if env::var_os("FE2O3_TEST_AUTHORITY_METADATA_V1").is_some() {
        let package_id = format!(
            "path+file://{}#external-standalone@0.1.0",
            workspace_root.display()
        );
        serde_json::json!({
            "packages": [{
                "checksum": null,
                "id": package_id,
                "links": null,
                "manifest_path": workspace_root.join("Cargo.toml"),
                "name": "external-standalone",
                "source": null,
                "targets": [{"kind": ["bin"]}],
                "version": "0.1.0",
            }],
            "resolve": {
                "nodes": [{"dependencies": [], "id": package_id}],
                "root": package_id,
            },
            "target_directory": target_directory,
            "version": 1,
            "workspace_default_members": [package_id],
            "workspace_members": [package_id],
            "workspace_root": workspace_root,
        })
    } else {
        serde_json::json!({
            "packages": [],
            "target_directory": target_directory,
            "version": 1,
            "workspace_members": [],
            "workspace_root": workspace_root,
        })
    };
    println!("{record}");
    ExitCode::SUCCESS
}

fn build_or_run(args: &[OsString]) -> ExitCode {
    #[cfg(target_os = "linux")]
    for descriptor in [197, 198] {
        if fs::symlink_metadata(format!("/proc/self/fd/{descriptor}")).is_ok() {
            eprintln!("fake Cargo inherited build capability fd {descriptor}");
            return ExitCode::FAILURE;
        }
    }
    if env::var_os("FE2O3_TEST_VERTICAL_CONTROL_DIR").is_some() {
        return vertical_worker_v2_invocation();
    }
    let mut mutated_post_spawn_input = false;
    for variable in [
        "FE2O3_TEST_MUTATE_RUSTC_RUNTIME_V1",
        "FE2O3_TEST_MUTATE_AUTHORITY_SOURCE_V1",
    ] {
        if let Some(path) = env::var_os(variable) {
            if let Err(error) = fs::write(&path, format!("mutated by {variable}\n")) {
                eprintln!("fake Cargo could not mutate {variable} input: {error}");
                return ExitCode::FAILURE;
            }
            mutated_post_spawn_input = true;
        }
    }
    if mutated_post_spawn_input {
        return ExitCode::from(23);
    }
    #[cfg(target_os = "linux")]
    if let Some(substitute) = env::var_os("FE2O3_TEST_SUBSTITUTE_RUSTC_LIB_TREE") {
        let substitute = match File::open(substitute) {
            Ok(directory) => directory,
            Err(error) => {
                eprintln!("fake Cargo could not open rustc lib-tree substitute: {error}");
                return ExitCode::FAILURE;
            }
        };
        // SAFETY: this single-threaded hostile fixture intentionally replaces its inherited fd.
        if unsafe { libc::dup2(substitute.as_raw_fd(), 193) } != 193 {
            eprintln!("fake Cargo could not replace inherited rustc lib-tree descriptor");
            return ExitCode::FAILURE;
        }
        return invoke_compile_wrapper("substituted_lib_tree", "substituted-lib-tree");
    }
    if env::var_os("FE2O3_TEST_COMPILER_CLOSURE_RUSTC_REPORT").is_some() {
        return invoke_compile_wrapper_with_environment(
            "compiler_closure",
            "compiler-closure",
            "FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1",
            "01".repeat(32),
        );
    }
    if let Some(report) = env::var_os("FE2O3_TEST_PINNED_RUSTC_REPORT") {
        let output = match Command::new(required_path("RUSTC_WORKSPACE_WRAPPER"))
            .arg(required_path("RUSTC"))
            .arg("--version")
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!("fake Cargo could not launch pinned rustc probe: {error}");
                return ExitCode::FAILURE;
            }
        };
        if !output.status.success() {
            eprintln!(
                "pinned rustc probe failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            return ExitCode::FAILURE;
        }
        if let Err(error) = fs::write(report, output.stdout) {
            eprintln!("fake Cargo could not record pinned rustc probe: {error}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    if let Some(loader_name) = env::var_os("FE2O3_TEST_WRAPPER_LOADER_NAME") {
        let loader_value = env::var_os("FE2O3_TEST_WRAPPER_LOADER_VALUE")
            .expect("loader injection fixture has a value");
        let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
        let rustc = required_path("RUSTC");
        let source = required_path("FE2O3_TEST_WORKSPACE_ROOT").join("src/main.rs");
        let output = match Command::new(wrapper)
            .arg(rustc)
            .args([
                "--crate-name",
                "loader_injection",
                "-Cmetadata=loader-injection",
            ])
            .arg(source)
            .env(loader_name, loader_value)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!("fake Cargo could not launch loader injection probe: {error}");
                return ExitCode::FAILURE;
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprint!("{stderr}");
        if output.status.success() || !stderr.contains("rejects dynamic-loader injection variable")
        {
            eprintln!("fake Cargo loader injection was not rejected by the binding wrapper");
        }
        return ExitCode::from(38);
    }
    if let Some(attacker_rustc) = env::var_os("FE2O3_TEST_SUBSTITUTE_RUSTC") {
        let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
        let source = required_path("FE2O3_TEST_WORKSPACE_ROOT").join("src/main.rs");
        let output = match Command::new(wrapper)
            .arg(attacker_rustc)
            .args([
                "--crate-name",
                "substituted_rustc",
                "-Cmetadata=substituted",
            ])
            .arg(source)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!("fake Cargo could not launch substituted rustc probe: {error}");
                return ExitCode::FAILURE;
            }
        };
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprint!("{stderr}");
        if output.status.success() || !stderr.contains("does not match the parent-pinned compiler")
        {
            eprintln!("fake Cargo rustc substitution was not rejected by compiler identity");
        }
        return ExitCode::from(37);
    }
    #[cfg(target_os = "linux")]
    if let Some(attacker_rustc) = env::var_os("FE2O3_TEST_SUBSTITUTE_RUSTC_DESCRIPTOR") {
        let attacker = match fs::read(attacker_rustc) {
            Ok(attacker) => attacker,
            Err(error) => {
                eprintln!("fake Cargo could not read descriptor substitute: {error}");
                return ExitCode::FAILURE;
            }
        };
        let image = match rustix::fs::memfd_create(
            "fe2o3-attacker-rustc",
            rustix::fs::MemfdFlags::ALLOW_SEALING,
        ) {
            Ok(image) => File::from(image),
            Err(error) => {
                eprintln!("fake Cargo could not create descriptor substitute: {error}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(error) = rustix::fs::fchmod(
            &image,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR | rustix::fs::Mode::XUSR,
        ) {
            eprintln!("fake Cargo could not populate descriptor substitute: {error}");
            return ExitCode::FAILURE;
        }
        if let Err(error) = (&image).write_all(&attacker) {
            eprintln!("fake Cargo could not write descriptor substitute: {error}");
            return ExitCode::FAILURE;
        }
        // SAFETY: both descriptors are valid in this single-threaded hostile fixture process.
        if unsafe { libc::dup2(image.as_raw_fd(), 194) } != 194 {
            eprintln!("fake Cargo could not replace inherited rustc descriptor");
            return ExitCode::FAILURE;
        }
        let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
        let source = required_path("FE2O3_TEST_WORKSPACE_ROOT").join("src/main.rs");
        let output = match Command::new(wrapper)
            .arg("/proc/self/fd/194")
            .args([
                "--crate-name",
                "substituted_rustc_descriptor",
                "-Cmetadata=substituted-descriptor",
            ])
            .arg(source)
            .output()
        {
            Ok(output) => output,
            Err(error) => {
                eprintln!("fake Cargo could not launch descriptor substitution probe: {error}");
                return ExitCode::FAILURE;
            }
        };
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        return ExitCode::from(39);
    }
    if let Some(mode) = env::var_os("FE2O3_TEST_BUILD_SCRIPT_MODE") {
        let fixture = required_path("FE2O3_TEST_BUILD_SCRIPT_FIXTURE");
        let status = if mode == "multithreaded-substitute-wrapper" {
            match multithreaded_substitute_wrapper(&fixture) {
                Ok(status) => status,
                Err(error) => {
                    eprintln!("fake Cargo could not run wrapper substitution probe: {error}");
                    return ExitCode::FAILURE;
                }
            }
        } else {
            match Command::new(fixture).arg(mode).status() {
                Ok(status) => status,
                Err(error) => {
                    eprintln!("fake Cargo could not launch build-script probe: {error}");
                    return ExitCode::FAILURE;
                }
            }
        };
        let report = required_path("FE2O3_TEST_BUILD_SCRIPT_REPORT");
        let report = fs::read_to_string(report).unwrap_or_default();
        let expected = match env::var("FE2O3_TEST_BUILD_SCRIPT_MODE").as_deref() {
            Ok("ordinary") => {
                status.success()
                    && report.contains("mode=ordinary")
                    && report.contains("backend_open=false")
                    && report.contains("artifact_open=false")
            }
            Ok("exec-wrapper" | "execveat-wrapper" | "multithreaded-substitute-wrapper") => {
                !status.success() && report.is_empty()
            }
            _ => false,
        };
        if !expected {
            eprintln!(
                "fake Cargo build-script probe produced unexpected status {status} and report {report:?}"
            );
            return ExitCode::FAILURE;
        }
    }
    let output = required_path("FE2O3_TEST_TARGET_DIRECTORY").join("fe2o3");
    if let Some(source) = env::var_os("FE2O3_TEST_REPLACE_BACKEND")
        && let Err(error) = fs::write(source, b"replacement backend")
    {
        eprintln!("fake Cargo could not replace backend source: {error}");
        return ExitCode::FAILURE;
    }
    let active = match env::var_os("FE2O3_TEST_EXCLUSIVE_ACTIVE") {
        Some(path) => {
            let path = PathBuf::from(path);
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => Some(path),
                Err(error) => {
                    eprintln!("fake Cargo observed concurrent generation: {error}");
                    return ExitCode::FAILURE;
                }
            }
        }
        None => None,
    };
    #[cfg(unix)]
    if env::var_os("FE2O3_TEST_GENERATION_CONTROL").is_some() {
        if let Err(error) = std::io::stdout()
            .write_all(b"ready")
            .and_then(|()| std::io::stdout().flush())
        {
            eprintln!("fake Cargo could not signal generation readiness: {error}");
            return ExitCode::FAILURE;
        }
        let mut release = [0_u8; 7];
        if let Err(error) = std::io::stdin().read_exact(&mut release) {
            eprintln!("fake Cargo could not receive generation release: {error}");
            return ExitCode::FAILURE;
        }
        if release != *b"release" {
            eprintln!("fake Cargo received invalid generation release");
            return ExitCode::FAILURE;
        }
    }
    if let Some(display) = env::var_os("FE2O3_TEST_SUBSTITUTE_ARTIFACT") {
        let display = PathBuf::from(display);
        let relocated = display.with_extension("relocated");
        let outside = display.with_extension("outside");
        if let Err(error) = substitute_artifact_path(&display, &relocated, &outside) {
            eprintln!("fake Cargo could not substitute artifact path: {error}");
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    let sidecar = if let Some(counter) = env::var_os("FE2O3_TEST_MUTATING_COUNTER") {
        let counter = PathBuf::from(counter);
        let next = fs::read_to_string(&counter)
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(0)
            + 1;
        if let Err(error) = fs::write(&counter, next.to_string()) {
            eprintln!("fake Cargo could not update mutation counter: {error}");
            return ExitCode::FAILURE;
        }
        format!("fixture-sidecar-{next}").into_bytes()
    } else {
        b"fixture-sidecar".to_vec()
    };
    if let Err(error) = fs::write(output.join("fixture.hsaco"), sidecar) {
        eprintln!("fake Cargo could not write sidecar: {error}");
        return ExitCode::FAILURE;
    }
    if let Some(marker) = env::var_os("FE2O3_TEST_FAIL_ONCE") {
        let marker = PathBuf::from(marker);
        if !marker.exists() {
            if let Err(error) = fs::write(&marker, b"failed") {
                eprintln!("fake Cargo could not write failure marker: {error}");
            }
            return ExitCode::from(23);
        }
    }
    if let Some(active) = active
        && let Err(error) = fs::remove_file(active)
    {
        eprintln!("fake Cargo could not clear active marker: {error}");
        return ExitCode::FAILURE;
    }
    if args.get(1).is_some_and(|argument| argument == "run")
        && let Err(error) = run_application_fixture(args)
    {
        eprintln!("fake Cargo could not execute injected runner: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(target_os = "linux")]
fn multithreaded_substitute_wrapper(
    attacker_compiler: &Path,
) -> Result<std::process::ExitStatus, String> {
    use std::time::{Duration, Instant};

    let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
    let substitute = required_path("FE2O3_TEST_WRAPPER_SUBSTITUTE");
    let displaced = required_path("FE2O3_TEST_DISPLACED_WRAPPER");
    let genuine = required_path("FE2O3_TEST_GENUINE_WRAPPER");
    let race_trace = required_path("FE2O3_TEST_WRAPPER_RACE_TRACE");
    let identity =
        fs::metadata(&genuine).map_err(|error| format!("inspect genuine wrapper: {error}"))?;
    let supervisor = u32::try_from(unsafe { libc::getppid() })
        .map_err(|_| "supervisor PID is negative".to_string())?;
    let baseline = matching_process_descriptors(supervisor, identity.dev(), identity.ino());
    let observed_wrapper = wrapper.clone();
    let racer = std::thread::spawn(move || -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let observed = matching_process_descriptors(supervisor, identity.dev(), identity.ino());
            if observed > baseline {
                fs::rename(&observed_wrapper, &displaced)
                    .map_err(|error| format!("displace observed wrapper pathname: {error}"))?;
                fs::rename(&substitute, &observed_wrapper)
                    .map_err(|error| format!("install hostile wrapper substitute: {error}"))?;
                fs::write(
                    race_trace,
                    format!(
                        "supervisor={supervisor}\nbaseline_fds={baseline}\nobserved_fds={observed}\nsubstituted=true\n"
                    ),
                )
                .map_err(|error| format!("record wrapper substitution: {error}"))?;
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(
                    "timed out waiting for the supervisor to open the wrapper image".into(),
                );
            }
            std::thread::yield_now();
        }
    });

    let source = required_path("FE2O3_TEST_BUILD_SCRIPT_REPORT").with_extension("rs");
    fs::write(&source, "pub fn replayed() {}\n")
        .map_err(|error| format!("write multithreaded replay source: {error}"))?;
    let status = Command::new(&wrapper)
        .arg(attacker_compiler)
        .args([
            "--crate-name",
            "multithreaded_substitute_replay",
            "--crate-type",
            "lib",
            "--emit=metadata",
            "-Cmetadata=multithreaded-substitute-replay",
        ])
        .arg(source)
        .env("FE2O3_TEST_MULTITHREADED_SUBSTITUTE", "1")
        .status()
        .map_err(|error| format!("launch observed wrapper pathname: {error}"))?;
    racer
        .join()
        .map_err(|_| "wrapper substitution racer panicked".to_string())??;
    Ok(status)
}

#[cfg(target_os = "linux")]
fn matching_process_descriptors(pid: u32, device: u64, inode: u64) -> usize {
    fs::read_dir(format!("/proc/{pid}/fd"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| fs::metadata(entry.path()).ok())
        .filter(|metadata| metadata.dev() == device && metadata.ino() == inode)
        .count()
}

#[cfg(not(target_os = "linux"))]
fn multithreaded_substitute_wrapper(
    _attacker_compiler: &Path,
) -> Result<std::process::ExitStatus, String> {
    Err("multithreaded wrapper substitution requires Linux".to_string())
}

fn invoke_compile_wrapper(crate_name: &str, metadata: &str) -> ExitCode {
    let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
    let rustc = required_path("RUSTC");
    let source = required_path("FE2O3_TEST_WORKSPACE_ROOT").join("src/main.rs");
    match Command::new(wrapper)
        .arg(rustc)
        .args(["--crate-name", crate_name])
        .arg(format!("-Cmetadata={metadata}"))
        .arg(source)
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(1),
        ),
        Err(error) => {
            eprintln!("fake Cargo could not launch compile wrapper: {error}");
            ExitCode::FAILURE
        }
    }
}

fn invoke_compile_wrapper_with_environment(
    crate_name: &str,
    metadata: &str,
    name: &str,
    value: String,
) -> ExitCode {
    let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
    let rustc = required_path("RUSTC");
    let source = required_path("FE2O3_TEST_WORKSPACE_ROOT").join("src/main.rs");
    match Command::new(wrapper)
        .arg(rustc)
        .args(["--crate-name", crate_name])
        .arg(format!("-Cmetadata={metadata}"))
        .arg(source)
        .env(name, value)
        .status()
    {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(1),
        ),
        Err(error) => {
            eprintln!("fake Cargo could not launch compile wrapper: {error}");
            ExitCode::FAILURE
        }
    }
}

fn vertical_worker_v2_invocation() -> ExitCode {
    let control = required_path("FE2O3_TEST_VERTICAL_CONTROL_DIR");
    if let Err(error) = fs::write(control.join("ready"), []) {
        eprintln!("fake Cargo could not publish vertical readiness: {error}");
        return ExitCode::FAILURE;
    }
    loop {
        if control.join("stop").exists() {
            return ExitCode::SUCCESS;
        }
        let request = control.join("request");
        let Ok(id) = fs::read_to_string(&request) else {
            std::thread::sleep(std::time::Duration::from_millis(2));
            continue;
        };
        let id = match id.parse::<u64>() {
            Ok(id) => id,
            Err(error) => {
                eprintln!("fake Cargo received malformed vertical request: {error}");
                return ExitCode::FAILURE;
            }
        };
        if let Err(error) = fs::remove_file(&request) {
            eprintln!("fake Cargo could not consume vertical request: {error}");
            return ExitCode::FAILURE;
        }
        if let Err(error) = execute_vertical_request(&control, id) {
            let message = format!("fake Cargo vertical request failed: {error}\n");
            if let Err(report_error) = write_vertical_result(
                &control,
                id,
                (1_i32 << 8).to_le_bytes(),
                &[],
                message.as_bytes(),
            ) {
                eprintln!("{message}fake Cargo could not report failure: {report_error}");
                return ExitCode::FAILURE;
            }
        }
    }
}

fn execute_vertical_request(control: &Path, id: u64) -> Result<(), String> {
    let restore = control.join("restore");
    if restore.exists() {
        let backup =
            fs::read_to_string(&restore).map_err(|error| format!("read restore path: {error}"))?;
        let artifact = required_path("FE2O3_TEST_TARGET_DIRECTORY").join("fe2o3");
        restore_test_directory(Path::new(&backup), &artifact)?;
    }

    let wrapper = required_path("RUSTC_WORKSPACE_WRAPPER");
    let rustc = required_path("RUSTC");
    let source = required_path("FE2O3_FIXTURE_SOURCE");
    let mode = fs::read_to_string(control.join("mode"))
        .map_err(|error| format!("read rustc mode: {error}"))?;
    let mut command = Command::new(wrapper);
    command
        .arg(rustc)
        .args(["--crate-name", "workflow_fixture"])
        .arg(source)
        .arg("-Cmetadata=worker-v2-test")
        .env("FE2O3_FIXTURE_RUSTC_MODE", mode)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if control.join("cov6").exists() {
        command.env("FE2O3_TEST_WORKER_V2_COV6", "1");
    } else {
        command.env_remove("FE2O3_TEST_WORKER_V2_COV6");
    }
    let fault = control.join("fault");
    if fault.exists() {
        command.env(
            "FE2O3_TEST_WORKER_V2_FAULT_POINT_V1",
            fs::read_to_string(fault).map_err(|error| format!("read fault point: {error}"))?,
        );
    } else {
        command.env_remove("FE2O3_TEST_WORKER_V2_FAULT_POINT_V1");
    }
    let handoff = control.join("handoff");
    if handoff.exists() {
        command.env(
            "FE2O3_FIXTURE_HANDOFF_MARKER",
            fs::read_to_string(handoff)
                .map_err(|error| format!("read handoff marker path: {error}"))?,
        );
    } else {
        command.env_remove("FE2O3_FIXTURE_HANDOFF_MARKER");
    }
    let strip = control.join("strip");
    if strip.exists() {
        let names = fs::read_to_string(strip)
            .map_err(|error| format!("read stripped environment: {error}"))?;
        for name in names.split(',').filter(|name| !name.is_empty()) {
            command.env_remove(name);
        }
    }
    scrub_test_harness_dynamic_loader_environment(&mut command);

    let child = command
        .spawn()
        .map_err(|error| format!("launch vertical rustc wrapper: {error}"))?;
    fs::write(
        vertical_result_path(control, id, "pid"),
        child.id().to_string(),
    )
    .map_err(|error| format!("record vertical wrapper PID: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("wait for vertical rustc wrapper: {error}"))?;
    #[cfg(unix)]
    let status = output.status.into_raw().to_le_bytes();
    #[cfg(not(unix))]
    let status = output.status.code().unwrap_or(-1).to_le_bytes();
    write_vertical_result(control, id, status, &output.stdout, &output.stderr)
}

fn scrub_test_harness_dynamic_loader_environment(command: &mut Command) {
    for (name, _) in env::vars_os() {
        let bytes = name.as_os_str().as_encoded_bytes();
        if bytes.starts_with(b"LD_") || bytes.starts_with(b"DYLD_") || bytes == b"GLIBC_TUNABLES" {
            command.env_remove(name);
        }
    }
}

fn write_vertical_result(
    control: &Path,
    id: u64,
    status: [u8; 4],
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), String> {
    for (suffix, bytes) in [
        ("status", status.as_slice()),
        ("stdout", stdout),
        ("stderr", stderr),
    ] {
        fs::write(vertical_result_path(control, id, suffix), bytes)
            .map_err(|error| format!("record vertical wrapper {suffix}: {error}"))?;
    }
    fs::write(vertical_result_path(control, id, "done"), [])
        .map_err(|error| format!("publish vertical wrapper result: {error}"))
}

fn vertical_result_path(control: &Path, id: u64, suffix: &str) -> PathBuf {
    control.join(format!("result-{id}.{suffix}"))
}

fn restore_test_directory(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(destination)
        .map_err(|error| format!("read destination {}: {error}", destination.display()))?
    {
        let path = entry
            .map_err(|error| format!("read destination entry: {error}"))?
            .path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("inspect destination {}: {error}", path.display()))?;
        if metadata.is_dir() {
            fs::remove_dir_all(&path)
                .map_err(|error| format!("remove destination {}: {error}", path.display()))?;
        } else {
            fs::remove_file(&path)
                .map_err(|error| format!("remove destination {}: {error}", path.display()))?;
        }
    }
    copy_test_directory(source, destination)
}

fn copy_test_directory(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source)
        .map_err(|error| format!("read source {}: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| format!("read source entry: {error}"))?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|error| format!("inspect source {}: {error}", source_path.display()))?;
        if file_type.is_dir() {
            fs::create_dir(&destination_path).map_err(|error| {
                format!(
                    "create destination directory {}: {error}",
                    destination_path.display()
                )
            })?;
            copy_test_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|error| {
                format!(
                    "copy {} to {}: {error}",
                    source_path.display(),
                    destination_path.display()
                )
            })?;
        } else {
            return Err(format!(
                "source {} is not a regular file or directory",
                source_path.display()
            ));
        }
    }
    Ok(())
}

fn run_application_fixture(args: &[OsString]) -> Result<(), String> {
    let Some(application) = env::var_os("FE2O3_TEST_RUN_APPLICATION") else {
        return Ok(());
    };
    let report = required_path("FE2O3_TEST_RUN_APPLICATION_REPORT");
    let payload = env::var_os("FE2O3_TEST_RUN_APPLICATION_PAYLOAD").unwrap_or_default();
    let runner = args
        .windows(2)
        .filter(|pair| pair[0] == "--config")
        .filter_map(|pair| pair[1].to_str())
        .filter_map(|config| config.split_once(".runner=").map(|(_, value)| value))
        .next_back()
        .ok_or_else(|| "run invocation has no injected target runner".to_string())?;
    let runner: Vec<String> = serde_json::from_str(runner)
        .map_err(|error| format!("decode injected runner configuration: {error}"))?;
    let (program, prefix) = runner
        .split_first()
        .ok_or_else(|| "injected runner configuration is empty".to_string())?;
    let status = Command::new(program)
        .args(prefix)
        .arg(application)
        .arg(report)
        .arg(payload)
        .status()
        .map_err(|error| format!("launch injected runner: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("injected runner failed with status {status}"))
    }
}

#[cfg(unix)]
fn substitute_artifact_path(
    display: &Path,
    relocated: &Path,
    outside: &Path,
) -> Result<(), String> {
    use std::os::unix::fs::symlink;

    fs::rename(display, relocated)
        .map_err(|error| format!("rename artifact directory: {error}"))?;
    fs::create_dir(outside).map_err(|error| format!("create outside directory: {error}"))?;
    fs::write(outside.join("keep"), b"outside")
        .map_err(|error| format!("write outside sentinel: {error}"))?;
    symlink(outside, display).map_err(|error| format!("install replacement symlink: {error}"))
}

#[cfg(not(unix))]
fn substitute_artifact_path(
    _display: &Path,
    _relocated: &Path,
    _outside: &Path,
) -> Result<(), String> {
    Err("artifact substitution fixture requires Unix".to_string())
}

fn required_path(variable: &str) -> PathBuf {
    PathBuf::from(env::var_os(variable).unwrap_or_else(|| panic!("missing {variable}")))
}

fn record_invocation(args: &[OsString]) -> Result<(), String> {
    let path = required_path("FE2O3_TEST_CARGO_LOG");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {}: {error}", path.display()))?;
    let cwd = env::current_dir().map_err(|error| format!("resolve cwd: {error}"))?;
    write_field(&mut file, cwd.as_os_str())?;
    write_u64(&mut file, args.len() as u64)?;
    for argument in args {
        write_field(&mut file, argument)?;
    }
    for variable in [
        "CARGO_TARGET_DIR",
        "FE2O3_HSACO_DIR",
        "FE2O3_TARGET",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "FE2O3_MANAGED_RUSTC_ARGS_V1",
    ] {
        write_field(
            &mut file,
            env::var_os(variable)
                .as_deref()
                .unwrap_or_else(|| OsStr::new("")),
        )?;
    }
    Ok(())
}

fn write_field(file: &mut impl Write, value: &OsStr) -> Result<(), String> {
    let bytes = os_bytes(value);
    write_u64(file, bytes.len() as u64)?;
    file.write_all(bytes)
        .map_err(|error| format!("write field: {error}"))
}

fn write_u64(file: &mut impl Write, value: u64) -> Result<(), String> {
    file.write_all(&value.to_le_bytes())
        .map_err(|error| format!("write length: {error}"))
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value
        .to_str()
        .expect("the Cargo fixture requires UTF-8 arguments off Unix")
        .as_bytes()
}

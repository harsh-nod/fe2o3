use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Command, ExitCode};

use fe2o3_artifact_transaction::{
    BuildAttempt, EmitError, ProducerIdentity, emit_artifact_transaction_for_attempt,
};
use fe2o3_rustc_invocation::{RustcInvocationV2, classify_rustc_invocation_v2};
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;

const BUILD_ATTEMPT_ENV: &str = "FE2O3_BUILD_ATTEMPT_V1";
const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";

fn main() -> ExitCode {
    match run() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => ExitCode::from(
            status
                .code()
                .and_then(|code| u8::try_from(code).ok())
                .unwrap_or(1),
        ),
        Err(error) => {
            eprintln!("cargo-fe2o3 rustc fixture: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<std::process::ExitStatus, String> {
    let filtered = filtered_args(env::args_os().collect());
    if filtered.len() == 2 && filtered[1] == "-vV" {
        println!(
            "rustc 1.93.0-nightly (fe2o3-fixture 2026-04-03)\n\
             binary: rustc\n\
             commit-hash: fe2o3fixture0000000000000000000000000000\n\
             commit-date: 2026-04-03\n\
             host: x86_64-unknown-linux-gnu\n\
             release: 1.93.0-nightly\n\
             LLVM version: 22.0.0"
        );
        #[cfg(unix)]
        return Ok(std::process::ExitStatus::from_raw(0));
    }
    let real_rustc = env::var_os("FE2O3_TEST_REAL_RUSTC")
        .ok_or_else(|| "missing FE2O3_TEST_REAL_RUSTC".to_string())?;
    match classify_rustc_invocation_v2(&filtered) {
        Ok(RustcInvocationV2::Compile(compile)) => {
            publish_probe(compile.crate_name(), compile.source_path())?;
            if let Some(report) = env::var_os("FE2O3_TEST_COMPILER_CLOSURE_RUSTC_REPORT") {
                let closure =
                    env::var("FE2O3_EXPECTED_COMPILER_CLOSURE_SHA256_V1").map_err(|_| {
                        "rustc fixture has no authenticated compiler closure".to_owned()
                    })?;
                fs::write(report, closure)
                    .map_err(|error| format!("write compiler closure report: {error}"))?;
            }
        }
        Ok(_) => {}
        Err(_) if env::var_os(BUILD_ATTEMPT_ENV).is_none() => {}
        Err(error) => return Err(format!("classify filtered rustc invocation: {error}")),
    }
    Command::new(real_rustc)
        .args(&filtered[1..])
        .status()
        .map_err(|error| format!("run real rustc: {error}"))
}

fn publish_probe(crate_name: &str, source: &Path) -> Result<(), String> {
    let attempt = env::var(BUILD_ATTEMPT_ENV)
        .ok()
        .and_then(|value| BuildAttempt::from_env_value(&value).ok())
        .ok_or_else(|| "compile invocation has no canonical build attempt".to_string())?;
    let output = env::var_os(HSACO_DIR_ENV)
        .ok_or_else(|| "compile invocation has no artifact directory".to_string())?;
    let backend = fs::read("/proc/self/fd/198")
        .map_err(|error| format!("read fixed backend descriptor: {error}"))?;
    if backend != b"test backend" {
        return Err("fixed backend descriptor contains substituted bytes".to_string());
    }
    if !Path::new("/proc/self/fd/197").is_dir()
        || Path::new(&output) != Path::new("/proc/self/fd/197")
    {
        return Err("artifact directory was not installed at fixed descriptor 197".to_string());
    }
    let binding = env::var(CRATE_BINDING_ID_ENV_V1)
        .map_err(|_| "compile invocation has no crate binding identity".to_string())?;
    if binding.len() != 64
        || !binding
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("crate binding identity is not canonical hexadecimal".to_string());
    }
    let kernel = format!("probe_{}", &binding[..16]);
    let producer = ProducerIdentity::from_codegen(crate_name, Some(source))
        .map_err(|error| format!("construct fixture producer: {error}"))?;
    emit_artifact_transaction_for_attempt(
        Path::new(&output),
        &producer,
        attempt,
        &[kernel.as_str()],
        |name| *name,
        |name| Ok(format!("; fixture IR for {name}\n")),
        |_llvm_ir, hsaco| {
            fs::write(hsaco.with_extension("o"), b"fixture object")?;
            fs::write(hsaco, b"fixture hsaco")?;
            Ok::<(), EmitError>(())
        },
    )
    .map_err(|error| format!("publish fixture backend output: {error}"))?;

    let report = env::var_os("FE2O3_TEST_RUSTC_CAPABILITY_REPORT")
        .ok_or_else(|| "missing capability report path".to_string())?;
    let mut report = OpenOptions::new()
        .create(true)
        .append(true)
        .open(report)
        .map_err(|error| format!("open capability report: {error}"))?;
    writeln!(report, "{crate_name}:{kernel}")
        .map_err(|error| format!("write capability report: {error}"))
}

fn filtered_args(args: Vec<OsString>) -> Vec<OsString> {
    let mut filtered = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let argument = &args[index];
        if argument == "-Zmir-enable-passes=-JumpThreading"
            || argument
                .to_str()
                .is_some_and(|value| value.starts_with("-Zcodegen-backend="))
        {
            index += 1;
            continue;
        }
        if argument == "--cfg"
            && args.get(index + 1).is_some_and(|value| {
                value
                    .to_str()
                    .is_some_and(|value| value.starts_with("fe2o3_codegen_generation=\""))
            })
        {
            index += 2;
            continue;
        }
        filtered.push(argument.clone());
        index += 1;
    }
    filtered
}

use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::ExitCode;

use fe2o3_artifact_transaction::{
    BuildAttempt, EmitError, ProducerIdentity, emit_artifact_transaction_for_attempt,
};
use fe2o3_rustc_invocation::{RustcInvocationV2, classify_rustc_invocation_v2};
use reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1;

const BUILD_ATTEMPT_ENV: &str = "FE2O3_BUILD_ATTEMPT_V1";
const HSACO_DIR_ENV: &str = "FE2O3_HSACO_DIR";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("protected release rustc fixture: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let filtered = filtered_args(env::args_os().collect());
    if filtered.len() == 2 && filtered[1] == "-vV" {
        println!(
            "rustc 1.93.0-nightly (fe2o3-release-fixture 2026-08-15)\n\
             binary: rustc\n\
             commit-hash: fe2o3releasefixture00000000000000000000000\n\
             commit-date: 2026-08-15\n\
             host: x86_64-unknown-linux-gnu\n\
             release: 1.93.0-nightly\n\
             LLVM version: 22.0.0"
        );
        return Ok(());
    }
    match classify_rustc_invocation_v2(&filtered) {
        Ok(RustcInvocationV2::Compile(compile)) => {
            publish_fixture(compile.crate_name(), compile.source_path())
        }
        Ok(_) => Ok(()),
        Err(error) => Err(format!("classify rustc invocation: {error}")),
    }
}

fn publish_fixture(crate_name: &str, source: &Path) -> Result<(), String> {
    if Path::new("/proc/self/fd/191").exists() || Path::new("/proc/self/fd/192").exists() {
        return Err("Cargo binding image descriptors survived the wrapper exec".to_owned());
    }
    let attempt = env::var(BUILD_ATTEMPT_ENV)
        .ok()
        .and_then(|value| BuildAttempt::from_env_value(&value).ok())
        .ok_or_else(|| "compile invocation has no canonical build attempt".to_owned())?;
    let output = env::var_os(HSACO_DIR_ENV)
        .ok_or_else(|| "compile invocation has no artifact directory".to_owned())?;
    if !Path::new("/proc/self/fd/197").is_dir()
        || Path::new(&output) != Path::new("/proc/self/fd/197")
    {
        return Err("artifact directory was not installed at fixed descriptor 197".to_owned());
    }
    if fs::read("/proc/self/fd/198")
        .map_err(|error| format!("read fixed backend descriptor: {error}"))?
        != b"test backend"
    {
        return Err("fixed backend descriptor contains substituted bytes".to_owned());
    }
    let binding = env::var(CRATE_BINDING_ID_ENV_V1)
        .map_err(|_| "compile invocation has no crate binding identity".to_owned())?;
    if binding.len() != 64
        || !binding
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("crate binding identity is not canonical hexadecimal".to_owned());
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
        |name| Ok(format!("; protected release fixture IR for {name}\n")),
        |_llvm_ir, hsaco| {
            fs::write(hsaco.with_extension("o"), b"fixture object")?;
            fs::write(hsaco, b"fixture hsaco")?;
            Ok::<(), EmitError>(())
        },
    )
    .map_err(|error| format!("publish fixture backend output: {error}"))?;

    let manifest = env::var_os("CARGO_MANIFEST_DIR")
        .ok_or_else(|| "compile invocation has no Cargo manifest directory".to_owned())?;
    let report = Path::new(&manifest).join("target/.fe2o3-protected-release-rustc-report-v1");
    let mut report = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&report)
        .map_err(|error| format!("open {}: {error}", report.display()))?;
    writeln!(report, "{crate_name}:{kernel}")
        .map_err(|error| format!("write release rustc report: {error}"))
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

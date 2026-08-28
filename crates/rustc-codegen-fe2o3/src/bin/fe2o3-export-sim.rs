use std::env;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use fe2o3_amd_target::ProductionAmdTargetProfileV1;

const OUTPUT_ENV: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1";
const OUTPUT_ENV_V2: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2";
const CRATE_ENV: &str = "FE2O3_EXTRACT_CRATE_V1";
const MAX_SYSROOT_OUTPUT_BYTES: u64 = 4096;

fn main() -> ExitCode {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    if matches!(args.as_slice(), [argument] if argument == "--help" || argument == "-h") {
        println!("{}", usage());
        return ExitCode::SUCCESS;
    }
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("fe2o3-export-sim: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Options {
    crate_name: String,
    output: PathBuf,
    target_dir: PathBuf,
    target_profile: ProductionAmdTargetProfileV1,
    bundle_version: u16,
    cargo_args: Vec<OsString>,
}

fn parse(args: Vec<OsString>, current_dir: &Path) -> Result<Options, String> {
    let mut crate_name = None;
    let mut output = None;
    let mut target_dir = None;
    let mut target_profile = ProductionAmdTargetProfileV1::Gfx942;
    let mut bundle_version = None;
    let mut cargo_args = Vec::new();
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        if argument == "--" {
            cargo_args.extend(args);
            break;
        }
        let Some(argument) = argument.to_str() else {
            return Err("options before `--` must be valid UTF-8".to_owned());
        };
        let value = match argument {
            "--crate" | "--output" | "--target" | "--target-dir" | "--bundle-version" => args
                .next()
                .ok_or_else(|| format!("{argument} requires a value"))?,
            "--help" | "-h" => return Err(usage().to_owned()),
            _ => return Err(format!("unknown option {argument:?}\n{}", usage())),
        };
        match argument {
            "--crate" => {
                let value = value
                    .into_string()
                    .map_err(|_| "--crate must be valid UTF-8".to_owned())?;
                if crate_name.replace(value).is_some() {
                    return Err("--crate may be specified only once".to_owned());
                }
            }
            "--output" => {
                if output.replace(PathBuf::from(value)).is_some() {
                    return Err("--output may be specified only once".to_owned());
                }
            }
            "--target" => {
                let value = value
                    .to_str()
                    .ok_or_else(|| "--target must be valid UTF-8".to_owned())?;
                target_profile = ProductionAmdTargetProfileV1::from_cpu(value)
                    .ok_or_else(|| "--target must be exactly gfx942 or gfx950".to_owned())?;
            }
            "--target-dir" => {
                if target_dir.replace(PathBuf::from(value)).is_some() {
                    return Err("--target-dir may be specified only once".to_owned());
                }
            }
            "--bundle-version" => {
                let value = value
                    .to_str()
                    .ok_or_else(|| "--bundle-version must be valid UTF-8".to_owned())?;
                let value = match value {
                    "1" => 1,
                    "2" => 2,
                    _ => return Err("--bundle-version must be exactly 1 or 2".to_owned()),
                };
                if bundle_version.replace(value).is_some() {
                    return Err("--bundle-version may be specified only once".to_owned());
                }
            }
            _ => unreachable!("closed option table"),
        }
    }
    let crate_name = crate_name.ok_or_else(|| "missing required --crate".to_owned())?;
    validate_crate_name(&crate_name)?;
    let output = absolute_path(
        current_dir,
        output.ok_or_else(|| "missing required --output".to_owned())?,
    );
    if output.file_name().is_none() {
        return Err("--output must name a file".to_owned());
    }
    if output.exists() {
        return Err(format!(
            "simulation bundle output `{}` already exists",
            output.display()
        ));
    }
    let output_parent = output
        .parent()
        .ok_or_else(|| "--output has no parent directory".to_owned())?;
    if !output_parent.is_dir() {
        return Err(format!(
            "simulation bundle output parent `{}` is not a directory",
            output_parent.display()
        ));
    }
    let target_dir = absolute_path(
        current_dir,
        target_dir.unwrap_or_else(|| PathBuf::from("target/fe2o3-sim-export")),
    );
    reject_cargo_override_args(&cargo_args)?;
    Ok(Options {
        crate_name,
        output,
        target_dir,
        target_profile,
        bundle_version: bundle_version.unwrap_or(1),
        cargo_args,
    })
}

fn validate_crate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 256
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(
            "--crate must be the nonempty rustc crate name using only ASCII letters, digits, or `_`"
                .to_owned(),
        );
    }
    Ok(())
}

fn reject_cargo_override_args(args: &[OsString]) -> Result<(), String> {
    for argument in args {
        let Some(argument) = argument.to_str() else {
            continue;
        };
        if argument == "--target"
            || argument.starts_with("--target=")
            || argument == "--target-dir"
            || argument.starts_with("--target-dir=")
            || argument == "--config"
            || argument.starts_with("--config=")
            || argument == "--release"
            || argument.starts_with("--release=")
            || argument == "-r"
            || argument == "--profile"
            || argument.starts_with("--profile=")
        {
            return Err(format!(
                "Cargo argument {argument:?} conflicts with the fixed extraction target or semantic profile"
            ));
        }
    }
    Ok(())
}

fn run(args: Vec<OsString>) -> Result<(), String> {
    reject_conflicting_environment()?;
    let current_dir = env::current_dir()
        .map_err(|error| format!("cannot identify invocation directory: {error}"))?;
    let options = parse(args, &current_dir)?;
    let current_exe = env::current_exe()
        .map_err(|error| format!("cannot identify exporter executable: {error}"))?;
    let wrapper = current_exe.with_file_name("fe2o3-rustc-extract");
    let metadata = std::fs::metadata(&wrapper).map_err(|error| {
        format!(
            "cannot locate sibling extraction compiler `{}`: {error}",
            wrapper.display()
        )
    })?;
    if !metadata.is_file() {
        return Err(format!(
            "sibling extraction compiler `{}` is not a regular file",
            wrapper.display()
        ));
    }
    let wrapper_dir = wrapper
        .parent()
        .ok_or_else(|| "sibling extraction compiler has no parent directory".to_owned())?;
    let rustc_lib = rustc_sysroot_lib_dir()?;
    let loader_path = env::join_paths([wrapper_dir, rustc_lib.as_path()])
        .map_err(|error| format!("cannot construct extraction loader path: {error}"))?;
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let output_env = if options.bundle_version == 2 {
        OUTPUT_ENV_V2
    } else {
        OUTPUT_ENV
    };
    let mut command = Command::new(cargo);
    command
        .arg("check")
        .arg("--locked")
        .arg("-Zbuild-std=core")
        .arg("--target")
        .arg(options.target_profile.rustc_target())
        .arg("--target-dir")
        .arg(&options.target_dir)
        .args(&options.cargo_args)
        .env("RUSTC_WRAPPER", "")
        .env("CARGO_BUILD_RUSTC_WRAPPER", "")
        .env("RUSTC_WORKSPACE_WRAPPER", &wrapper)
        .env("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", &wrapper)
        .env("LD_LIBRARY_PATH", loader_path)
        .env(CRATE_ENV, &options.crate_name)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env(
            options.target_profile.cargo_rustflags_env(),
            fixed_target_rustflags(options.target_profile, options.bundle_version),
        );
    for name in conflicting_extraction_environment() {
        command.env_remove(name);
    }
    command
        .env_remove(OUTPUT_ENV)
        .env_remove(OUTPUT_ENV_V2)
        .env(CRATE_ENV, &options.crate_name)
        .env(output_env, &options.output);
    let status = command
        .status()
        .map_err(|error| format!("failed to execute Cargo extraction: {error}"))?;
    if !status.success() {
        return Err(format!("Cargo extraction failed with {status}"));
    }
    if !options.output.is_file() {
        return Err(format!(
            "Cargo succeeded without publishing `{}`",
            options.output.display()
        ));
    }
    Ok(())
}

fn rustc_sysroot_lib_dir() -> Result<PathBuf, String> {
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let mut child = Command::new(&rustc)
        .args(["--print", "sysroot"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("cannot query rustc sysroot with {rustc:?}: {error}"))?;
    let mut bytes = Vec::new();
    child
        .stdout
        .take()
        .expect("piped rustc stdout")
        .take(MAX_SYSROOT_OUTPUT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("cannot read rustc sysroot: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for rustc sysroot query: {error}"))?;
    if !status.success() {
        return Err(format!("rustc sysroot query failed with {status}"));
    }
    if bytes.is_empty() || bytes.len() > MAX_SYSROOT_OUTPUT_BYTES as usize {
        return Err("rustc sysroot query returned an empty or oversized path".to_owned());
    }
    let sysroot = std::str::from_utf8(&bytes)
        .map_err(|_| "rustc sysroot path must be valid UTF-8".to_owned())?
        .trim_end_matches(['\r', '\n']);
    if sysroot.is_empty() || sysroot.contains('\r') || sysroot.contains('\n') {
        return Err("rustc sysroot query returned a malformed path".to_owned());
    }
    let rustc_lib = Path::new(sysroot).join("lib");
    if !rustc_lib.is_dir() {
        return Err(format!(
            "rustc sysroot library directory `{}` is absent",
            rustc_lib.display()
        ));
    }
    Ok(rustc_lib)
}

fn reject_conflicting_environment() -> Result<(), String> {
    for name in conflicting_extraction_environment() {
        if let Some(value) = env::var_os(name) {
            return Err(format!(
                "caller environment {name}={value:?} conflicts with explicit simulation export"
            ));
        }
    }
    Ok(())
}

const fn conflicting_extraction_environment() -> [&'static str; 7] {
    [
        OUTPUT_ENV,
        OUTPUT_ENV_V2,
        "FE2O3_EXTRACT_RANKED_MEMORY_V1",
        "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1",
        "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1",
        "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1",
    ]
}

fn fixed_target_rustflags(target: ProductionAmdTargetProfileV1, bundle_version: u16) -> String {
    let debug_info = if bundle_version == 2 {
        " -Cdebuginfo=2"
    } else {
        ""
    };
    format!(
        "-Zalways-encode-mir -Zinline-mir=no -Zmir-enable-passes=-JumpThreading -Copt-level=0 -Ctarget-cpu={} -Ctarget-feature={}",
        target.cpu(),
        target.rustc_features(),
    ) + debug_info
}

fn absolute_path(current_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        current_dir.join(path)
    }
}

const fn usage() -> &'static str {
    "usage: fe2o3-export-sim --crate <rustc-crate-name> --output <bundle.fe2sim> [--bundle-version 1|2] [--target gfx942|gfx950] [--target-dir <dir>] [-- <Cargo package/feature args>]"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_fixed_export_and_forwards_only_non_authoritative_cargo_selection() {
        let root = env::temp_dir();
        let output_name = format!("fe2o3-export-sim-{}.fe2sim", std::process::id());
        let _ = std::fs::remove_file(root.join(&output_name));
        let options = parse(
            vec![
                OsString::from("--crate"),
                OsString::from("kernel_crate"),
                OsString::from("--output"),
                OsString::from(&output_name),
                OsString::from("--bundle-version"),
                OsString::from("2"),
                OsString::from("--target"),
                OsString::from("gfx950"),
                OsString::from("--target-dir"),
                OsString::from("scratch"),
                OsString::from("--"),
                OsString::from("--package"),
                OsString::from("kernel-package"),
                OsString::from("--features"),
                OsString::from("selected"),
            ],
            &root,
        )
        .unwrap();
        assert_eq!(options.output, root.join(output_name));
        assert_eq!(options.target_dir, root.join("scratch"));
        assert_eq!(options.target_profile, ProductionAmdTargetProfileV1::Gfx950);
        assert_eq!(options.bundle_version, 2);
        assert_eq!(options.cargo_args.len(), 4);
    }

    #[test]
    fn rejects_target_overrides_and_ambiguous_names() {
        assert!(validate_crate_name("package-name").is_err());
        assert!(validate_crate_name("kernel_crate").is_ok());
        assert!(reject_cargo_override_args(&[OsString::from("--target=gfx942")]).is_err());
        assert!(reject_cargo_override_args(&[OsString::from("--features=x")]).is_ok());
        for arguments in [
            vec![
                OsString::from("--config"),
                OsString::from("net.offline=true"),
            ],
            vec![OsString::from("--config=net.offline=true")],
            vec![OsString::from("--release")],
            vec![OsString::from("--release=true")],
            vec![OsString::from("-r")],
            vec![OsString::from("--profile"), OsString::from("bench")],
            vec![OsString::from("--profile=bench")],
        ] {
            assert!(reject_cargo_override_args(&arguments).is_err());
        }
        assert_eq!(
            fixed_target_rustflags(ProductionAmdTargetProfileV1::Gfx942, 1),
            "-Zalways-encode-mir -Zinline-mir=no -Zmir-enable-passes=-JumpThreading -Copt-level=0 -Ctarget-cpu=gfx942 -Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack"
        );
        assert_eq!(
            fixed_target_rustflags(ProductionAmdTargetProfileV1::Gfx950, 1),
            "-Zalways-encode-mir -Zinline-mir=no -Zmir-enable-passes=-JumpThreading -Copt-level=0 -Ctarget-cpu=gfx950 -Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack"
        );
        assert!(
            fixed_target_rustflags(ProductionAmdTargetProfileV1::Gfx942, 2)
                .ends_with(" -Cdebuginfo=2")
        );
        assert!(
            parse(
                vec![
                    OsString::from("--crate"),
                    OsString::from("kernel_crate"),
                    OsString::from("--output"),
                    OsString::from("kernel.fe2sim"),
                    OsString::from("--bundle-version"),
                    OsString::from("3"),
                ],
                &env::temp_dir(),
            )
            .is_err()
        );
        assert!(
            parse(
                vec![
                    OsString::from("--crate"),
                    OsString::from("kernel_crate"),
                    OsString::from("--output"),
                    OsString::from("kernel.fe2sim"),
                    OsString::from("--bundle-version"),
                    OsString::from("1"),
                    OsString::from("--bundle-version"),
                    OsString::from("2"),
                ],
                &env::temp_dir(),
            )
            .is_err()
        );
    }
}

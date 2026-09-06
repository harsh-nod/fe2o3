#![feature(rustc_private)]

use std::env;
use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, CargoMetadataBuildObservationV2,
    PortablePackageIdentityV1, RustcInvocationV2, capture_cargo_package_identity_v1,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    ordered_rustc_codegen_metadata_v1, portable_rustc_metadata_v1,
};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};

const EXTRACT_CRATE_ENV_V1: &str = "FE2O3_EXTRACT_CRATE_V1";
const EXTRACT_RANKED_MEMORY_ENV_V1: &str = "FE2O3_EXTRACT_RANKED_MEMORY_V1";
const EXTRACT_AMDGPU_LLVM_PATH_ENV_V1: &str = "FE2O3_EXTRACT_AMDGPU_LLVM_PATH_V1";
const EXTRACT_GFX942_LLVM_PATH_ENV_V1: &str = "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1";
const EXTRACT_GFX942_COMPILER_HANDOFF_PATH_ENV_V1: &str =
    "FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1";
const EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1";
const EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V2: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V2";
const EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V3: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V3";
const EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V4: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V4";
const EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V5: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V5";
const EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V6: &str = "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V6";
const EXTRACT_CRATE_BINDING_PATH_ENV_V1: &str = "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1";

fn main() {
    let simulation_v1 = env::var_os(EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1);
    let simulation_v2 = env::var_os(EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V2);
    let simulation_v3 = env::var_os(EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V3);
    let simulation_v4 = env::var_os(EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V4);
    let simulation_v5 = env::var_os(EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V5);
    let simulation_v6 = env::var_os(EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V6);
    let (simulation_output, version) = match select_simulation_output(
        simulation_v1,
        simulation_v2,
        simulation_v3,
        simulation_v4,
        simulation_v5,
        simulation_v6,
    ) {
        Ok(selected) => selected,
        Err(error) => {
            eprintln!("fe2o3 rustc extraction: {error}");
            std::process::exit(1);
        }
    };
    let prepared = prepare(
        env::args_os().collect(),
        env::var_os(EXTRACT_CRATE_ENV_V1),
        env::var_os(EXTRACT_RANKED_MEMORY_ENV_V1),
        env::var_os(EXTRACT_AMDGPU_LLVM_PATH_ENV_V1),
        env::var_os(EXTRACT_GFX942_LLVM_PATH_ENV_V1),
        env::var_os(EXTRACT_GFX942_COMPILER_HANDOFF_PATH_ENV_V1),
        simulation_output,
        env::var_os(EXTRACT_CRATE_BINDING_PATH_ENV_V1),
        None,
    )
    .map(|prepared| select_simulation_mode(prepared, version));
    let code = match prepared.and_then(execute) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("fe2o3 rustc extraction: {error}");
            1
        }
    };
    std::process::exit(code);
}

#[derive(Debug)]
enum PreparedExtractionV1 {
    Passthrough {
        executable: OsString,
        forwarded_args: Vec<OsString>,
    },
    Selected(SelectedExtractionV1),
}

#[derive(Debug)]
struct SelectedExtractionV1 {
    args: Vec<String>,
    crate_binding: CrateBindingIdV1,
    metadata_observation: CargoMetadataBuildObservationV2,
    crate_binding_output: Option<PathBuf>,
    mode: ExtractionModeV1,
}

fn capture_selected_primary_package_identity_v1() -> Result<PortablePackageIdentityV1, String> {
    require_exact_primary_package_marker_v1(env::var_os("CARGO_PRIMARY_PACKAGE").as_deref())?;
    capture_cargo_package_identity_v1().map_err(|error| error.to_string())
}

fn require_exact_primary_package_marker_v1(marker: Option<&std::ffi::OsStr>) -> Result<(), String> {
    if marker == Some(std::ffi::OsStr::new("1")) {
        Ok(())
    } else {
        Err("selected terminal extraction requires Cargo's exact primary-package marker".to_owned())
    }
}

#[derive(Debug)]
enum ExtractionModeV1 {
    KernelIr,
    RankedMemory,
    AmdgpuLlvm(OsString),
    Gfx942Llvm(OsString),
    Gfx942CompilerHandoff(OsString),
    SimulationBundle(OsString),
    SimulationBundleV2(OsString),
    SimulationBundleV3(OsString),
    SimulationBundleV4(OsString),
    SimulationBundleV5(OsString),
    SimulationBundleV6(OsString),
}

fn select_simulation_output(
    v1: Option<OsString>,
    v2: Option<OsString>,
    v3: Option<OsString>,
    v4: Option<OsString>,
    v5: Option<OsString>,
    v6: Option<OsString>,
) -> Result<(Option<OsString>, u16), String> {
    let count = usize::from(v1.is_some())
        + usize::from(v2.is_some())
        + usize::from(v3.is_some())
        + usize::from(v4.is_some())
        + usize::from(v5.is_some())
        + usize::from(v6.is_some());
    if count > 1 {
        return Err(format!(
            "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1}, {EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V2}, {EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V3}, {EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V4}, {EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V5}, and {EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V6} are mutually exclusive"
        ));
    }
    match (v1, v2, v3, v4, v5, v6) {
        (Some(output), None, None, None, None, None) => Ok((Some(output), 1)),
        (None, Some(output), None, None, None, None) if output.is_empty() => Err(format!(
            "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V2} must not be empty"
        )),
        (None, Some(output), None, None, None, None) => Ok((Some(output), 2)),
        (None, None, Some(output), None, None, None) if output.is_empty() => Err(format!(
            "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V3} must not be empty"
        )),
        (None, None, Some(output), None, None, None) => Ok((Some(output), 3)),
        (None, None, None, Some(output), None, None) if output.is_empty() => Err(format!(
            "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V4} must not be empty"
        )),
        (None, None, None, Some(output), None, None) => Ok((Some(output), 4)),
        (None, None, None, None, Some(output), None) if output.is_empty() => Err(format!(
            "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V5} must not be empty"
        )),
        (None, None, None, None, Some(output), None) => Ok((Some(output), 5)),
        (None, None, None, None, None, Some(output)) if output.is_empty() => Err(format!(
            "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V6} must not be empty"
        )),
        (None, None, None, None, None, Some(output)) => Ok((Some(output), 6)),
        (None, None, None, None, None, None) => Ok((None, 1)),
        _ => unreachable!("multiple simulation output variables were rejected"),
    }
}

fn select_simulation_mode(
    mut prepared: PreparedExtractionV1,
    version: u16,
) -> PreparedExtractionV1 {
    if let PreparedExtractionV1::Selected(selected) = &mut prepared
        && let ExtractionModeV1::SimulationBundle(output) = &mut selected.mode
    {
        selected.mode = match version {
            6 => ExtractionModeV1::SimulationBundleV6(std::mem::take(output)),
            5 => ExtractionModeV1::SimulationBundleV5(std::mem::take(output)),
            4 => ExtractionModeV1::SimulationBundleV4(std::mem::take(output)),
            3 => ExtractionModeV1::SimulationBundleV3(std::mem::take(output)),
            2 => ExtractionModeV1::SimulationBundleV2(std::mem::take(output)),
            _ => return prepared,
        };
    }
    prepared
}

#[allow(clippy::too_many_arguments)]
fn prepare(
    argv: Vec<OsString>,
    selected_crate: Option<OsString>,
    ranked_memory: Option<OsString>,
    amdgpu_llvm_path: Option<OsString>,
    gfx942_llvm_path: Option<OsString>,
    gfx942_compiler_handoff_path: Option<OsString>,
    simulation_bundle_path: Option<OsString>,
    crate_binding_path: Option<OsString>,
    package_identity: Option<PortablePackageIdentityV1>,
) -> Result<PreparedExtractionV1, String> {
    let actual_rustc_argv = argv
        .get(1..)
        .filter(|argv| !argv.is_empty())
        .ok_or_else(|| "wrapper requires the actual rustc argv".to_owned())?;
    if is_cargo_stdin_probe(actual_rustc_argv) {
        return Ok(PreparedExtractionV1::Passthrough {
            executable: actual_rustc_argv[0].clone(),
            forwarded_args: actual_rustc_argv[1..].to_vec(),
        });
    }
    let invocation = classify_rustc_invocation_v2(actual_rustc_argv)
        .map_err(|error| format!("invalid rustc invocation: {error}"))?;
    let selected_crate = match selected_crate {
        None => return Ok(prepare_passthrough(invocation)),
        Some(value) => value
            .into_string()
            .map_err(|_| format!("{EXTRACT_CRATE_ENV_V1} must be valid UTF-8"))?,
    };
    if selected_crate.is_empty() {
        return Err(format!("{EXTRACT_CRATE_ENV_V1} must not be empty"));
    }

    let RustcInvocationV2::Compile(compile) = invocation else {
        return Ok(prepare_passthrough(invocation));
    };
    if compile.crate_name() != selected_crate {
        return Ok(prepare_passthrough(invocation));
    }

    let cargo_metadata = ordered_rustc_codegen_metadata_v1(compile)
        .map_err(|error| format!("invalid rustc codegen metadata: {error}"))?;
    if cargo_metadata.is_empty() {
        return Err(format!(
            "selected rustc compile for crate `{}` has no explicit -C metadata value",
            compile.crate_name()
        ));
    }
    let args = actual_rustc_argv
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "selected extraction argv must be valid UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let args = enforce_selected_overflow_checks_v1(args)?;
    let package_identity = match package_identity {
        Some(identity) => identity,
        None => capture_selected_primary_package_identity_v1()?,
    };
    let portable_metadata = portable_rustc_metadata_v1(compile, &package_identity)
        .map_err(|error| error.to_string())?;
    let args = replace_selected_codegen_metadata_v1(args, &portable_metadata)?;
    let crate_binding =
        derive_crate_binding_id_v1(compile.crate_name(), [portable_metadata.as_str()]);
    let metadata_observation = derive_cargo_metadata_build_observation_v2(&cargo_metadata);
    let selected_modes = usize::from(ranked_memory.is_some())
        + usize::from(amdgpu_llvm_path.is_some())
        + usize::from(gfx942_llvm_path.is_some())
        + usize::from(gfx942_compiler_handoff_path.is_some())
        + usize::from(simulation_bundle_path.is_some());
    if selected_modes > 1 {
        return Err(format!(
            "{EXTRACT_RANKED_MEMORY_ENV_V1}, {EXTRACT_AMDGPU_LLVM_PATH_ENV_V1}, legacy {EXTRACT_GFX942_LLVM_PATH_ENV_V1}, {EXTRACT_GFX942_COMPILER_HANDOFF_PATH_ENV_V1}, and {EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1} are mutually exclusive"
        ));
    }
    let mode = if let Some(output) = simulation_bundle_path {
        if output.is_empty() {
            return Err(format!(
                "{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1} must not be empty"
            ));
        }
        ExtractionModeV1::SimulationBundle(output)
    } else if let Some(output) = gfx942_compiler_handoff_path {
        if output.is_empty() {
            return Err(format!(
                "{EXTRACT_GFX942_COMPILER_HANDOFF_PATH_ENV_V1} must not be empty"
            ));
        }
        ExtractionModeV1::Gfx942CompilerHandoff(output)
    } else if let Some(output) = amdgpu_llvm_path {
        if output.is_empty() {
            return Err(format!(
                "{EXTRACT_AMDGPU_LLVM_PATH_ENV_V1} must not be empty"
            ));
        }
        ExtractionModeV1::AmdgpuLlvm(output)
    } else if let Some(output) = gfx942_llvm_path {
        if output.is_empty() {
            return Err(format!(
                "{EXTRACT_GFX942_LLVM_PATH_ENV_V1} must not be empty"
            ));
        }
        ExtractionModeV1::Gfx942Llvm(output)
    } else {
        match ranked_memory {
            None => ExtractionModeV1::KernelIr,
            Some(value) if value == "1" => ExtractionModeV1::RankedMemory,
            Some(_) => {
                return Err(format!(
                    "{EXTRACT_RANKED_MEMORY_ENV_V1} must be exactly `1` when present"
                ));
            }
        }
    };
    let crate_binding_output = crate_binding_path
        .map(|path| {
            if path.is_empty() {
                Err(format!(
                    "{EXTRACT_CRATE_BINDING_PATH_ENV_V1} must not be empty"
                ))
            } else {
                Ok(PathBuf::from(path))
            }
        })
        .transpose()?;
    Ok(PreparedExtractionV1::Selected(SelectedExtractionV1 {
        args,
        crate_binding,
        metadata_observation,
        crate_binding_output,
        mode,
    }))
}

/// Canonicalizes the production arithmetic policy before rustc enters the
/// selected in-process session. Overflow checks are a fixed compiler policy,
/// not a user-selectable crate-binding axis.
fn enforce_selected_overflow_checks_v1(args: Vec<String>) -> Result<Vec<String>, String> {
    const CANONICAL: &str = "-Coverflow-checks=on";

    let mut rewritten = Vec::with_capacity(args.len().saturating_add(1));
    let mut index = 0;
    let mut inserted = false;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            rewritten.push(CANONICAL.to_owned());
            inserted = true;
            rewritten.extend(args[index..].iter().cloned());
            break;
        }
        if argument == "-C" || argument == "--codegen" {
            let value = args.get(index + 1).ok_or_else(|| {
                format!("selected rustc option `{argument}` lost its validated value")
            })?;
            if let Some(value) = value.strip_prefix("overflow-checks=") {
                require_overflow_checks_enabled_v1(value)?;
                index += 2;
                continue;
            }
            rewritten.push(argument.clone());
            rewritten.push(value.clone());
            index += 2;
            continue;
        }
        let joined = argument
            .strip_prefix("-C")
            .or_else(|| argument.strip_prefix("--codegen="));
        if let Some(value) = joined.and_then(|value| value.strip_prefix("overflow-checks=")) {
            require_overflow_checks_enabled_v1(value)?;
            index += 1;
            continue;
        }
        rewritten.push(argument.clone());
        index += 1;
    }
    if !inserted {
        rewritten.push(CANONICAL.to_owned());
    }
    Ok(rewritten)
}

fn require_overflow_checks_enabled_v1(value: &str) -> Result<(), String> {
    if matches!(value, "y" | "yes" | "on" | "true") {
        Ok(())
    } else {
        Err(format!(
            "selected production kernel requires `-Coverflow-checks=on`; observed `{value}`"
        ))
    }
}

fn replace_selected_codegen_metadata_v1(
    args: Vec<String>,
    portable_metadata: &str,
) -> Result<Vec<String>, String> {
    let canonical = format!("-Cmetadata={portable_metadata}");
    let mut rewritten = Vec::with_capacity(args.len() + 1);
    let mut index = 0;
    let mut inserted = false;
    while index < args.len() {
        let argument = &args[index];
        if argument == "--" {
            rewritten.push(canonical.clone());
            inserted = true;
            rewritten.extend(args[index..].iter().cloned());
            break;
        }
        if argument == "-C" || argument == "--codegen" {
            let value = args.get(index + 1).ok_or_else(|| {
                format!("selected rustc option `{argument}` lost its validated value")
            })?;
            if value.starts_with("metadata=") {
                index += 2;
                continue;
            }
            rewritten.push(argument.clone());
            rewritten.push(value.clone());
            index += 2;
            continue;
        }
        if argument.starts_with("-Cmetadata=") || argument.starts_with("--codegen=metadata=") {
            index += 1;
            continue;
        }
        rewritten.push(argument.clone());
        index += 1;
    }
    if !inserted {
        rewritten.push(canonical);
    }
    Ok(rewritten)
}

fn is_cargo_stdin_probe(argv: &[OsString]) -> bool {
    argv.get(1).is_some_and(|argument| argument == "-")
        && argv.iter().skip(2).any(|argument| {
            argument == "--print"
                || argument
                    .to_str()
                    .is_some_and(|argument| argument.starts_with("--print="))
        })
}

fn prepare_passthrough(invocation: RustcInvocationV2<'_>) -> PreparedExtractionV1 {
    PreparedExtractionV1::Passthrough {
        executable: invocation.executable().to_owned(),
        forwarded_args: invocation.forwarded_args().to_vec(),
    }
}

fn execute(prepared: PreparedExtractionV1) -> Result<i32, String> {
    match prepared {
        PreparedExtractionV1::Passthrough {
            executable,
            forwarded_args,
        } => execute_passthrough(executable, forwarded_args),
        PreparedExtractionV1::Selected(selected) => execute_selected(selected),
    }
}

fn passthrough_command(executable: OsString, forwarded_args: Vec<OsString>) -> Command {
    let mut command = Command::new(executable);
    command
        .args(forwarded_args)
        .env_remove(CRATE_BINDING_ID_ENV_V1)
        .env_remove(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2);
    command
}

fn execute_passthrough(executable: OsString, forwarded_args: Vec<OsString>) -> Result<i32, String> {
    let mut command = passthrough_command(executable, forwarded_args);
    let status = fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
        .and_then(|mut child| child.wait())
        .map_err(|error| format!("failed to execute rustc passthrough: {error}"))?;
    Ok(exit_code(status))
}

fn execute_selected(selected: SelectedExtractionV1) -> Result<i32, String> {
    install_selected_compile_environment_before_rustc_threads_v1(
        selected.crate_binding,
        selected.metadata_observation,
    );
    match selected.mode {
        ExtractionModeV1::KernelIr => {
            rustc_codegen_fe2o3::run_production_extraction_driver_v1(&selected.args)?;
        }
        ExtractionModeV1::RankedMemory => {
            rustc_codegen_fe2o3::run_production_ranked_extraction_driver_v1(&selected.args)?;
        }
        ExtractionModeV1::AmdgpuLlvm(output) => {
            rustc_codegen_fe2o3::run_production_amdgpu_llvm_extraction_driver_v1(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::Gfx942Llvm(output) => {
            rustc_codegen_fe2o3::run_production_gfx942_llvm_extraction_driver_v1(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::Gfx942CompilerHandoff(output) => {
            rustc_codegen_fe2o3::run_production_gfx942_compiler_handoff_extraction_driver_v1(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::SimulationBundle(output) => {
            rustc_codegen_fe2o3::run_production_simulation_bundle_extraction_driver_v1(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::SimulationBundleV2(output) => {
            rustc_codegen_fe2o3::run_production_simulation_bundle_extraction_driver_v2(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::SimulationBundleV3(output) => {
            rustc_codegen_fe2o3::run_production_simulation_bundle_extraction_driver_v3(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::SimulationBundleV4(output) => {
            rustc_codegen_fe2o3::run_production_simulation_bundle_extraction_driver_v4(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::SimulationBundleV5(output) => {
            rustc_codegen_fe2o3::run_production_simulation_bundle_extraction_driver_v5(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
        ExtractionModeV1::SimulationBundleV6(output) => {
            rustc_codegen_fe2o3::run_production_simulation_bundle_extraction_driver_v6(
                &selected.args,
                std::path::Path::new(&output),
            )?;
        }
    }
    if let Some(output) = selected.crate_binding_output {
        publish_selected_crate_binding_v1(&output, selected.crate_binding)?;
    }
    Ok(0)
}

fn publish_selected_crate_binding_v1(
    output: &std::path::Path,
    crate_binding: CrateBindingIdV1,
) -> Result<(), String> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(output).map_err(|error| {
        format!(
            "failed to create new selected crate-binding output `{}`: {error}",
            output.display()
        )
    })?;
    let result = (|| {
        writeln!(file, "{}", crate_binding.to_hex())?;
        file.sync_all()
    })();
    if let Err(error) = result {
        drop(file);
        let _ = std::fs::remove_file(output);
        return Err(format!(
            "failed to publish selected crate binding to `{}`: {error}",
            output.display()
        ));
    }
    Ok(())
}

fn install_selected_compile_environment_before_rustc_threads_v1(
    crate_binding: CrateBindingIdV1,
    metadata_observation: CargoMetadataBuildObservationV2,
) {
    // SAFETY: the extraction binary calls this once on its selected path from
    // `main`, immediately before entering the in-process rustc driver. No
    // thread has been created and no library code can concurrently read the
    // process environment at this boundary.
    unsafe {
        env::set_var(CRATE_BINDING_ID_ENV_V1, crate_binding.to_hex());
        env::set_var(
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            metadata_observation.to_hex(),
        );
    }
}

fn exit_code(status: ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulation_bundle_environment_is_versioned_and_mutually_exclusive() {
        let (output, version) = select_simulation_output(
            None,
            Some(OsString::from("kernel-v2.fe2sim")),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(output, Some(OsString::from("kernel-v2.fe2sim")));
        assert_eq!(version, 2);
        assert!(
            select_simulation_output(
                Some(OsString::from("v1")),
                Some(OsString::from("v2")),
                None,
                None,
                None,
                None,
            )
            .is_err()
        );
        assert!(
            select_simulation_output(None, Some(OsString::new()), None, None, None, None).is_err()
        );
        let (_, version) = select_simulation_output(
            None,
            None,
            Some(OsString::from("kernel-v3.fe2sim")),
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(version, 3);
        let (_, version) = select_simulation_output(
            None,
            None,
            None,
            None,
            Some(OsString::from("kernel-v5.fe2sim")),
            None,
        )
        .unwrap();
        assert_eq!(version, 5);
        let (_, version) = select_simulation_output(
            None,
            None,
            None,
            None,
            None,
            Some(OsString::from("kernel-v6.fe2sim")),
        )
        .unwrap();
        assert_eq!(version, 6);
    }

    fn package_identity(version: &str, manifest_byte: u8) -> PortablePackageIdentityV1 {
        PortablePackageIdentityV1::new("package", version, [manifest_byte; 32]).unwrap()
    }

    fn compile_argv(crate_name: &str, metadata: &[&str]) -> Vec<OsString> {
        ["fe2o3-rustc-extract", "rustc", "--crate-name", crate_name]
            .into_iter()
            .map(OsString::from)
            .chain([OsString::from("unit.rs")])
            .chain(
                metadata
                    .iter()
                    .map(|value| OsString::from(format!("-Cmetadata={value}"))),
            )
            .collect()
    }

    fn selected_compile(crate_name: &str, metadata: &[&str]) -> SelectedExtractionV1 {
        let prepared = prepare(
            compile_argv(crate_name, metadata),
            Some(OsString::from(crate_name)),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap();
        let PreparedExtractionV1::Selected(selected) = prepared else {
            panic!("matching compile must be selected");
        };
        selected
    }

    fn selected_binding(crate_name: &str, metadata: &[&str]) -> CrateBindingIdV1 {
        selected_compile(crate_name, metadata).crate_binding
    }

    fn session_metadata(selected: &SelectedExtractionV1) -> Vec<String> {
        let argv = selected.args.iter().map(OsString::from).collect::<Vec<_>>();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("rewritten selected argv must remain a compile invocation");
        };
        ordered_rustc_codegen_metadata_v1(compile).unwrap()
    }

    #[test]
    fn selected_binding_is_portable_across_cargo_metadata_order_and_duplicates() {
        let binding = selected_binding("unit", &["first", "second", "first"]);
        assert_eq!(binding, selected_binding("unit", &["host-checkout"]));
        assert_eq!(binding, selected_binding("unit", &["second", "first"]));
        assert_ne!(
            binding,
            selected_binding("other", &["first", "second", "first"])
        );
    }

    #[test]
    fn selected_binding_is_derived_independently_of_a_stale_caller_value() {
        let stale_caller_value = CrateBindingIdV1::from_bytes([0x55; 32]);
        let stale_observation = derive_cargo_metadata_build_observation_v2(&["stale"]);
        let selected = selected_compile("unit", &["compiler-metadata"]);

        assert_ne!(selected.crate_binding, stale_caller_value);
        assert_ne!(selected.metadata_observation, stale_observation);
        let metadata = session_metadata(&selected);
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].len(), 64);
        assert_eq!(
            selected.crate_binding,
            derive_crate_binding_id_v1("unit", metadata.iter().map(String::as_str))
        );
        assert_eq!(
            selected.metadata_observation,
            derive_cargo_metadata_build_observation_v2(&["compiler-metadata"])
        );
    }

    #[test]
    fn original_cargo_observation_remains_exact_while_session_metadata_is_portable() {
        let ordered = selected_compile("unit", &["first", "second", "first"]);
        let reordered = selected_compile("unit", &["first", "first", "second"]);
        let deduplicated = selected_compile("unit", &["first", "second"]);

        assert_ne!(ordered.metadata_observation, reordered.metadata_observation);
        assert_ne!(
            ordered.metadata_observation,
            deduplicated.metadata_observation
        );
        assert_eq!(ordered.crate_binding, reordered.crate_binding);
        assert_eq!(ordered.crate_binding, deduplicated.crate_binding);
        assert_eq!(session_metadata(&ordered), session_metadata(&reordered));
        assert_eq!(session_metadata(&ordered), session_metadata(&deduplicated));
    }

    #[test]
    fn portable_binding_ignores_checkout_paths_but_separates_semantic_dimensions() {
        let selected = |checkout: &str, features: &[&str], target_cpu: &str, opt_level: &str| {
            let mut argv = vec![
                "fe2o3-rustc-extract".to_owned(),
                format!("{checkout}/toolchains/rustc"),
                "--crate-name".to_owned(),
                "unit".to_owned(),
                format!("{checkout}/src/lib.rs"),
                "--target=amdgcn-amd-amdhsa".to_owned(),
                "--crate-type=lib".to_owned(),
                "--edition=2024".to_owned(),
            ];
            for feature in features {
                argv.push("--cfg".to_owned());
                argv.push(format!("feature=\"{feature}\""));
            }
            argv.extend([
                format!("-Ctarget-cpu={target_cpu}"),
                "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".to_owned(),
                format!("-Copt-level={opt_level}"),
                format!("-Cmetadata={checkout}-cargo-salt"),
                format!("-Cextra-filename=-{checkout}-cargo-salt"),
                "--out-dir".to_owned(),
                format!("{checkout}/target/out"),
                "--extern".to_owned(),
                format!("dep={checkout}/target/libdep.rmeta"),
            ]);
            let argv = argv.into_iter().map(OsString::from).collect();
            let PreparedExtractionV1::Selected(selected) = prepare(
                argv,
                Some(OsString::from("unit")),
                None,
                None,
                None,
                None,
                None,
                None,
                Some(package_identity("1.0.0", 1)),
            )
            .unwrap() else {
                panic!("matching compile must be selected");
            };
            selected
        };

        let first = selected("/checkout/one", &["kernel-fp4-gemm"], "gfx950", "3");
        let relocated = selected("/different/root", &["kernel-fp4-gemm"], "gfx950", "3");
        assert_eq!(first.crate_binding, relocated.crate_binding);
        assert_eq!(session_metadata(&first), session_metadata(&relocated));
        assert_ne!(first.metadata_observation, relocated.metadata_observation);

        let feature_bindings = [
            "kernel-fp4-gemm",
            "kernel-fp8-gemm",
            "kernel-fp4-attention",
            "kernel-fp8-attention",
        ]
        .map(|feature| selected("/checkout/one", &[feature], "gfx950", "3").crate_binding);
        for index in 0..feature_bindings.len() {
            assert!(!feature_bindings[..index].contains(&feature_bindings[index]));
        }
        assert_ne!(
            first.crate_binding,
            selected("/checkout/one", &["kernel-fp4-gemm"], "gfx942", "3").crate_binding
        );
        assert_ne!(
            first.crate_binding,
            selected("/checkout/one", &["kernel-fp4-gemm"], "gfx950", "2").crate_binding
        );
        assert_eq!(
            selected(
                "/checkout/one",
                &["auxiliary", "kernel-fp4-gemm", "auxiliary"],
                "gfx950",
                "3",
            )
            .crate_binding,
            selected(
                "/checkout/one",
                &["kernel-fp4-gemm", "auxiliary"],
                "gfx950",
                "3",
            )
            .crate_binding,
        );
    }

    #[test]
    fn portable_binding_separates_package_version_and_manifest_identity() {
        let args = compile_argv("unit", &["cargo-salt"]);
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&args[1..]).unwrap()
        else {
            panic!("fixture must classify as a compile invocation");
        };
        let baseline = portable_rustc_metadata_v1(compile, &package_identity("1.0.0", 1)).unwrap();
        assert_ne!(
            baseline,
            portable_rustc_metadata_v1(compile, &package_identity("1.0.1", 1)).unwrap()
        );
        assert_ne!(
            baseline,
            portable_rustc_metadata_v1(compile, &package_identity("1.0.0", 2)).unwrap()
        );
    }

    #[test]
    fn metadata_rewrite_preserves_every_other_argument_and_its_order() {
        let args = [
            "/checkout/rustc",
            "--crate-name",
            "unit",
            "/checkout/src/lib.rs",
            "-Cmetadata=host-salt",
            "-Cextra-filename=-host-salt",
            "--out-dir",
            "/checkout/target/out",
            "--extern",
            "dep=/checkout/target/libdep.rmeta",
        ]
        .map(str::to_owned)
        .to_vec();
        let rewritten = replace_selected_codegen_metadata_v1(args, "portable").unwrap();
        assert_eq!(
            rewritten,
            [
                "/checkout/rustc",
                "--crate-name",
                "unit",
                "/checkout/src/lib.rs",
                "-Cextra-filename=-host-salt",
                "--out-dir",
                "/checkout/target/out",
                "--extern",
                "dep=/checkout/target/libdep.rmeta",
                "-Cmetadata=portable",
            ]
        );
    }

    #[test]
    fn selected_overflow_policy_is_exact_and_canonical() {
        let normalized = enforce_selected_overflow_checks_v1(
            [
                "rustc",
                "--crate-name",
                "unit",
                "unit.rs",
                "-C",
                "overflow-checks=yes",
                "--codegen=overflow-checks=true",
                "--",
                "literal",
            ]
            .map(str::to_owned)
            .to_vec(),
        )
        .unwrap();
        assert_eq!(
            normalized,
            [
                "rustc",
                "--crate-name",
                "unit",
                "unit.rs",
                "-Coverflow-checks=on",
                "--",
                "literal",
            ]
        );

        for disabled in ["off", "no", "false", "0", "invalid"] {
            let error = enforce_selected_overflow_checks_v1(vec![
                "rustc".to_owned(),
                format!("-Coverflow-checks={disabled}"),
            ])
            .unwrap_err();
            assert!(error.contains("requires `-Coverflow-checks=on`"));
            assert!(error.contains(disabled));
        }
    }

    #[test]
    fn fixed_overflow_policy_preserves_the_implicit_portable_namespace() {
        let original = compile_argv("unit", &["cargo-salt"])
            .into_iter()
            .skip(1)
            .map(|argument| argument.into_string().unwrap())
            .collect::<Vec<_>>();
        let normalized = enforce_selected_overflow_checks_v1(original.clone()).unwrap();
        assert_eq!(
            normalized
                .iter()
                .filter(|argument| argument.as_str() == "-Coverflow-checks=on")
                .count(),
            1
        );
        let original = original.into_iter().map(OsString::from).collect::<Vec<_>>();
        let normalized = normalized
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let RustcInvocationV2::Compile(original) = classify_rustc_invocation_v2(&original).unwrap()
        else {
            panic!("original fixture must classify as a compile invocation");
        };
        let RustcInvocationV2::Compile(normalized) =
            classify_rustc_invocation_v2(&normalized).unwrap()
        else {
            panic!("normalized fixture must classify as a compile invocation");
        };
        let identity = package_identity("1.0.0", 1);
        assert_eq!(
            portable_rustc_metadata_v1(original, &identity).unwrap(),
            portable_rustc_metadata_v1(normalized, &identity).unwrap(),
        );
    }

    #[test]
    fn selected_primary_package_marker_is_exact() {
        assert!(require_exact_primary_package_marker_v1(Some(std::ffi::OsStr::new("1"))).is_ok());
        for marker in [
            None,
            Some(std::ffi::OsStr::new("")),
            Some(std::ffi::OsStr::new("true")),
            Some(std::ffi::OsStr::new("01")),
        ] {
            assert!(require_exact_primary_package_marker_v1(marker).is_err());
        }
    }

    #[test]
    fn selected_session_collapses_every_metadata_spelling_to_one_token() {
        let argv = [
            "fe2o3-rustc-extract",
            "rustc",
            "--crate-name",
            "unit",
            "unit.rs",
            "-C",
            "metadata=first",
            "-Cmetadata=second",
            "--codegen",
            "metadata=third",
            "--codegen=metadata=fourth",
        ]
        .map(OsString::from)
        .to_vec();
        let PreparedExtractionV1::Selected(selected) = prepare(
            argv,
            Some(OsString::from("unit")),
            None,
            None,
            None,
            None,
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap() else {
            panic!("matching compile must be selected");
        };
        let metadata = session_metadata(&selected);
        assert_eq!(metadata.len(), 1);
        assert_eq!(metadata[0].len(), 64);
        assert_eq!(
            selected.crate_binding,
            derive_crate_binding_id_v1("unit", metadata.iter().map(String::as_str)),
        );
    }

    #[test]
    fn selected_compile_requires_metadata_and_nonempty_handoff_path() {
        let missing = prepare(
            compile_argv("unit", &[]),
            Some(OsString::from("unit")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(missing.contains("has no explicit -C metadata value"));

        let empty_path = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            None,
            None,
            None,
            None,
            Some(OsString::new()),
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap_err();
        assert_eq!(
            empty_path,
            format!("{EXTRACT_CRATE_BINDING_PATH_ENV_V1} must not be empty")
        );
    }

    #[test]
    fn extraction_outputs_are_exact_nonempty_and_mutually_exclusive() {
        let generic = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            Some(OsString::from("generic.ll")),
            None,
            None,
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap();
        let PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::AmdgpuLlvm(output),
            ..
        }) = generic
        else {
            panic!("generic AMDGPU LLVM output must select the generic driver");
        };
        assert_eq!(output, "generic.ll");

        let conflicting = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            Some(OsString::from("generic.ll")),
            Some(OsString::from("gfx942.ll")),
            None,
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap_err();
        assert!(conflicting.contains("mutually exclusive"));

        let empty_generic = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            Some(OsString::new()),
            None,
            None,
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap_err();
        assert_eq!(
            empty_generic,
            format!("{EXTRACT_AMDGPU_LLVM_PATH_ENV_V1} must not be empty")
        );

        let handoff = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            None,
            None,
            Some(OsString::from("module.handoff")),
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap();
        let PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::Gfx942CompilerHandoff(output),
            ..
        }) = handoff
        else {
            panic!("compiler handoff output must select the handoff driver");
        };
        assert_eq!(output, "module.handoff");

        let empty_handoff = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            None,
            None,
            Some(OsString::new()),
            None,
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap_err();
        assert_eq!(
            empty_handoff,
            format!("{EXTRACT_GFX942_COMPILER_HANDOFF_PATH_ENV_V1} must not be empty")
        );

        let simulation = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            None,
            None,
            None,
            Some(OsString::from("kernel.fe2sim")),
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap();
        let PreparedExtractionV1::Selected(SelectedExtractionV1 {
            mode: ExtractionModeV1::SimulationBundle(output),
            ..
        }) = simulation
        else {
            panic!("simulation bundle output must select the simulation exporter");
        };
        assert_eq!(output, "kernel.fe2sim");

        let empty_simulation = prepare(
            compile_argv("unit", &["metadata"]),
            Some(OsString::from("unit")),
            None,
            None,
            None,
            None,
            Some(OsString::new()),
            None,
            Some(package_identity("1.0.0", 1)),
        )
        .unwrap_err();
        assert_eq!(
            empty_simulation,
            format!("{EXTRACT_SIMULATION_BUNDLE_PATH_ENV_V1} must not be empty")
        );
    }

    #[test]
    fn nonselected_and_query_invocations_remove_binding_authority() {
        let prepared = prepare(
            compile_argv("dependency", &["metadata"]),
            Some(OsString::from("selected")),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let PreparedExtractionV1::Passthrough {
            executable,
            forwarded_args,
        } = prepared
        else {
            panic!("nonselected compile must pass through");
        };
        let command = passthrough_command(executable, forwarded_args);
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == CRATE_BINDING_ID_ENV_V1),
            Some((std::ffi::OsStr::new(CRATE_BINDING_ID_ENV_V1), None))
        );
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == CARGO_METADATA_BUILD_OBSERVATION_ENV_V2),
            Some((
                std::ffi::OsStr::new(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2),
                None
            ))
        );

        let query = vec![
            OsString::from("fe2o3-rustc-extract"),
            OsString::from("rustc"),
            OsString::from("--version"),
        ];
        assert!(matches!(
            prepare(
                query,
                Some(OsString::from("selected")),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            PreparedExtractionV1::Passthrough { .. }
        ));

        let managed_stdin_probe = vec![
            OsString::from("fe2o3-rustc-extract"),
            OsString::from("rustc"),
            OsString::from("-"),
            OsString::from("-Zmir-enable-passes=-JumpThreading"),
            OsString::from("--print=file-names"),
        ];
        assert!(matches!(
            prepare(
                managed_stdin_probe,
                Some(OsString::from("selected")),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap(),
            PreparedExtractionV1::Passthrough { .. }
        ));
    }

    #[test]
    fn binding_handoff_is_new_exact_and_never_overwrites_stale_output() {
        let root = std::env::temp_dir().join(format!(
            "fe2o3-binding-handoff-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let output = root.join("crate-binding-v1");
        let binding = derive_crate_binding_id_v1("unit", ["metadata"]);

        publish_selected_crate_binding_v1(&output, binding).unwrap();
        assert_eq!(
            std::fs::read_to_string(&output).unwrap(),
            format!("{}\n", binding.to_hex())
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&output).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }

        let hostile = derive_crate_binding_id_v1("unit", ["hostile"]);
        assert!(
            publish_selected_crate_binding_v1(&output, hostile)
                .unwrap_err()
                .contains("failed to create new selected crate-binding output")
        );
        assert_eq!(
            std::fs::read_to_string(&output).unwrap(),
            format!("{}\n", binding.to_hex())
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

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
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, CargoMetadataBuildObservationV2, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    ordered_rustc_codegen_metadata_v1,
};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};

const EXTRACT_CRATE_ENV_V1: &str = "FE2O3_EXTRACT_CRATE_V1";
const EXTRACT_RANKED_MEMORY_ENV_V1: &str = "FE2O3_EXTRACT_RANKED_MEMORY_V1";
const EXTRACT_GFX942_LLVM_PATH_ENV_V1: &str = "FE2O3_EXTRACT_GFX942_LLVM_PATH_V1";
const EXTRACT_CRATE_BINDING_PATH_ENV_V1: &str = "FE2O3_EXTRACT_CRATE_BINDING_PATH_V1";

fn main() {
    let prepared = prepare(
        env::args_os().collect(),
        env::var_os(EXTRACT_CRATE_ENV_V1),
        env::var_os(EXTRACT_RANKED_MEMORY_ENV_V1),
        env::var_os(EXTRACT_GFX942_LLVM_PATH_ENV_V1),
        env::var_os(EXTRACT_CRATE_BINDING_PATH_ENV_V1),
    );
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

#[derive(Debug)]
enum ExtractionModeV1 {
    KernelIr,
    RankedMemory,
    Gfx942Llvm(OsString),
}

fn prepare(
    argv: Vec<OsString>,
    selected_crate: Option<OsString>,
    ranked_memory: Option<OsString>,
    gfx942_llvm_path: Option<OsString>,
    crate_binding_path: Option<OsString>,
) -> Result<PreparedExtractionV1, String> {
    let actual_rustc_argv = argv
        .get(1..)
        .filter(|argv| !argv.is_empty())
        .ok_or_else(|| "wrapper requires the actual rustc argv".to_owned())?;
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

    let metadata = ordered_rustc_codegen_metadata_v1(compile)
        .map_err(|error| format!("invalid rustc codegen metadata: {error}"))?;
    if metadata.is_empty() {
        return Err(format!(
            "selected rustc compile for crate `{}` has no explicit -C metadata value",
            compile.crate_name()
        ));
    }
    let crate_binding =
        derive_crate_binding_id_v1(compile.crate_name(), metadata.iter().map(String::as_str));
    let metadata_observation = derive_cargo_metadata_build_observation_v2(&metadata);
    let args = actual_rustc_argv
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .map(str::to_owned)
                .ok_or_else(|| "selected extraction argv must be valid UTF-8".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if gfx942_llvm_path.is_some() && ranked_memory.is_some() {
        return Err(format!(
            "{EXTRACT_RANKED_MEMORY_ENV_V1} and {EXTRACT_GFX942_LLVM_PATH_ENV_V1} are mutually exclusive"
        ));
    }
    let mode = if let Some(output) = gfx942_llvm_path {
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
    let status = passthrough_command(executable, forwarded_args)
        .status()
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
        ExtractionModeV1::Gfx942Llvm(output) => {
            rustc_codegen_fe2o3::run_production_gfx942_llvm_extraction_driver_v1(
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

    #[test]
    fn selected_binding_uses_exact_crate_name_metadata_order_and_duplicates() {
        let binding = selected_binding("unit", &["first", "second", "first"]);
        assert_eq!(
            binding,
            derive_crate_binding_id_v1("unit", ["first", "second", "first"])
        );
        assert_ne!(
            binding,
            selected_binding("unit", &["first", "first", "second"])
        );
        assert_ne!(binding, selected_binding("unit", &["first", "second"]));
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
        assert_eq!(
            selected.crate_binding,
            derive_crate_binding_id_v1("unit", ["compiler-metadata"])
        );
        assert_eq!(
            selected.metadata_observation,
            derive_cargo_metadata_build_observation_v2(&["compiler-metadata"])
        );
    }

    #[test]
    fn selected_metadata_observation_changes_with_order_and_duplicates() {
        let ordered = selected_compile("unit", &["first", "second", "first"]);
        let reordered = selected_compile("unit", &["first", "first", "second"]);
        let deduplicated = selected_compile("unit", &["first", "second"]);

        assert_ne!(ordered.metadata_observation, reordered.metadata_observation);
        assert_ne!(
            ordered.metadata_observation,
            deduplicated.metadata_observation
        );
        assert_ne!(ordered.crate_binding, reordered.crate_binding);
        assert_ne!(ordered.crate_binding, deduplicated.crate_binding);
    }

    #[test]
    fn selected_compile_requires_metadata_and_nonempty_handoff_path() {
        let missing = prepare(
            compile_argv("unit", &[]),
            Some(OsString::from("unit")),
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
            Some(OsString::new()),
        )
        .unwrap_err();
        assert_eq!(
            empty_path,
            format!("{EXTRACT_CRATE_BINDING_PATH_ENV_V1} must not be empty")
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
            prepare(query, Some(OsString::from("selected")), None, None, None,).unwrap(),
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

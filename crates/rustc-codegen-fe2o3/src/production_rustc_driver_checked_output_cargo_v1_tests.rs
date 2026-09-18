//! Test-only Cargo capture. Dependencies really build; the selected root is
//! stopped before rustc and then replayed through the P4 callback, never a stub.
use super::corpus::Fixture;
use super::*;
use std::os::unix::{ffi::OsStringExt, fs::PermissionsExt};

const CAPTURE_ARGS: &str = "FE2O3_TEST_P4_CAPTURE_ARGS";
const CAPTURE_ENV: &str = "FE2O3_TEST_P4_CAPTURE_ENV";
const CAPTURE_MANIFEST: &str = "FE2O3_TEST_P4_CAPTURE_MANIFEST";
const CAPTURE_CRATE: &str = "FE2O3_TEST_P4_CAPTURE_CRATE";

// Values come through the environment, not shell interpolation. This selects
// the Cargo target under test, not a compiler rule keyed on a workload name.
const WRAPPER: &str = r#"#!/bin/sh
set -eu
crate=''
next=''
for arg in "$@"; do
    if [ "$next" = crate ]; then crate="$arg"; next=''; continue; fi
    case "$arg" in
        --crate-name) next=crate ;;
        --crate-name=*) crate="${arg#--crate-name=}" ;;
    esac
done
if [ "${CARGO_PRIMARY_PACKAGE-}" = 1 ] &&
   [ "${CARGO_MANIFEST_DIR-}" = "$FE2O3_TEST_P4_CAPTURE_MANIFEST" ] &&
   [ "$crate" = "$FE2O3_TEST_P4_CAPTURE_CRATE" ]; then
    if [ -e "$FE2O3_TEST_P4_CAPTURE_ARGS" ]; then
        echo 'duplicate selected Cargo root invocation' >&2
        exit 87
    fi
    env -0 > "$FE2O3_TEST_P4_CAPTURE_ENV"
    { printf '%s\000' "$PWD"; printf '%s\000' "$@"; } > "$FE2O3_TEST_P4_CAPTURE_ARGS"
    echo 'FE2O3_TEST_P4_ROOT_CAPTURED: selected root deferred to real rustc_driver callback' >&2
    exit 86
fi
exec "$@"
"#;

pub(super) struct Captured {
    pub args: Vec<String>,
    pub environment: Vec<(OsString, OsString)>,
    pub cwd: PathBuf,
    pub cfg: Vec<String>,
    pub cargo_diagnostics: String,
}

pub(super) fn diagnostics(output: &std::process::Output) -> String {
    const MAX: usize = 32 * 1024;
    let tail = |bytes: &[u8]| {
        String::from_utf8_lossy(&bytes[bytes.len().saturating_sub(MAX)..]).into_owned()
    };
    format!(
        "status={}\nstdout (tail):\n{}\nstderr (tail):\n{}",
        output.status,
        tail(&output.stdout),
        tail(&output.stderr)
    )
}

fn failed(stage: SourceStage, message: impl std::fmt::Display) -> SourceFailure {
    SourceFailure::new(stage, message)
}

fn feature_args(command: &mut Command, fixture: &Fixture) {
    if !fixture.compiler_input.default_features {
        command.arg("--no-default-features");
    }
    if !fixture.compiler_input.features.is_empty() {
        command.args(["--features", &fixture.compiler_input.features.join(",")]);
    }
}

fn records(bytes: &[u8]) -> Result<Vec<&[u8]>, SourceFailure> {
    if bytes.last() != Some(&0) {
        return Err(failed(
            SourceStage::Invocation,
            "capture is not NUL terminated",
        ));
    }
    Ok(bytes[..bytes.len() - 1].split(|byte| *byte == 0).collect())
}

fn environment(bytes: &[u8]) -> Result<Vec<(OsString, OsString)>, SourceFailure> {
    let mut result = Vec::new();
    let mut keys = std::collections::BTreeSet::new();
    for row in records(bytes)? {
        let at = row.iter().position(|b| *b == b'=').ok_or_else(|| {
            failed(
                SourceStage::Invocation,
                "environment row has no equals sign",
            )
        })?;
        if at == 0 || !keys.insert(row[..at].to_vec()) {
            return Err(failed(
                SourceStage::Invocation,
                "empty or duplicate environment key",
            ));
        }
        result.push((
            OsString::from_vec(row[..at].to_vec()),
            OsString::from_vec(row[at + 1..].to_vec()),
        ));
    }
    Ok(result)
}

fn option_values(args: &[String], name: &str) -> Result<Vec<String>, SourceFailure> {
    let mut values = Vec::new();
    let mut index = 1;
    while index < args.len() {
        if args[index] == name {
            index += 1;
            values.push(
                args.get(index)
                    .ok_or_else(|| {
                        failed(SourceStage::Invocation, format!("missing {name} value"))
                    })?
                    .clone(),
            );
        } else if let Some(value) = args[index].strip_prefix(&format!("{name}=")) {
            values.push(value.to_owned());
        }
        index += 1;
    }
    Ok(values)
}

fn redirect_output(args: &mut [String], directory: &Path) -> Result<(), SourceFailure> {
    if option_values(args, "--out-dir")?.len() != 1 {
        return Err(failed(
            SourceStage::Invocation,
            "expected one actual Cargo output directory",
        ));
    }
    let directory = directory
        .to_str()
        .ok_or_else(|| failed(SourceStage::Invocation, "non-UTF-8 test output directory"))?;
    for index in 1..args.len() {
        if args[index] == "--out-dir" {
            args[index + 1] = directory.to_owned();
            return Ok(());
        }
        if args[index].starts_with("--out-dir=") {
            args[index] = format!("--out-dir={directory}");
            return Ok(());
        }
    }
    unreachable!("single option was inspected above")
}

fn codegen_values(args: &[String], name: &str) -> Result<Vec<String>, SourceFailure> {
    let mut values = Vec::new();
    let mut index = 1;
    while index < args.len() {
        let option = if args[index] == "-C" {
            index += 1;
            args.get(index)
                .ok_or_else(|| failed(SourceStage::Invocation, "missing codegen option"))?
                .as_str()
        } else if let Some(value) = args[index].strip_prefix("-C") {
            value
        } else {
            index += 1;
            continue;
        };
        if let Some(value) = option.strip_prefix(&format!("{name}=")) {
            values.push(value.to_owned());
        }
        index += 1;
    }
    Ok(values)
}

fn make_sysroot_explicit(
    args: &mut Vec<String>,
    environment: &[(OsString, OsString)],
    cwd: &Path,
) -> Result<(), SourceFailure> {
    match option_values(args, "--sysroot")?.len() {
        1 => return Ok(()),
        0 => {}
        _ => {
            return Err(failed(
                SourceStage::Invocation,
                "ambiguous captured sysroot",
            ));
        }
    }
    let compiler = args
        .first()
        .ok_or_else(|| failed(SourceStage::Invocation, "missing captured compiler"))?;
    let output = Command::new(compiler)
        .env_clear()
        .envs(environment.iter().map(|(key, value)| (key, value)))
        .current_dir(cwd)
        .args(["--print", "sysroot"])
        .output()
        .map_err(|e| failed(SourceStage::Invocation, e))?;
    if !output.status.success() {
        return Err(failed(SourceStage::Invocation, diagnostics(&output)));
    }
    let sysroot = std::str::from_utf8(&output.stdout)
        .map_err(|e| failed(SourceStage::Invocation, e))?
        .trim();
    if sysroot.is_empty()
        || sysroot.lines().count() != 1
        || !Path::new(sysroot).is_absolute()
        || !Path::new(sysroot).is_dir()
    {
        return Err(failed(
            SourceStage::Invocation,
            "captured compiler returned an invalid sysroot",
        ));
    }
    args.extend(["--sysroot".to_owned(), sysroot.to_owned()]);
    Ok(())
}

/// Cargo performs feature expansion, dependency/build-script execution and
/// target selection. Only its exact selected root invocation is intercepted.
pub(super) fn capture(
    workspace: &Path,
    fixture: &Fixture,
    case: &Path,
    target: &Path,
) -> Result<Captured, SourceFailure> {
    let input = &fixture.compiler_input;
    let manifest = workspace
        .join(&input.package_manifest)
        .canonicalize()
        .map_err(|e| failed(SourceStage::CargoMetadata, e))?;
    let package_dir = manifest
        .parent()
        .ok_or_else(|| failed(SourceStage::CargoMetadata, "manifest has no parent"))?;
    let mut metadata_command = clean_command(env!("CARGO"));
    metadata_command
        .current_dir(workspace)
        .args([
            "metadata",
            "--offline",
            "--locked",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(&manifest);
    feature_args(&mut metadata_command, fixture);
    let output = metadata_command
        .output()
        .map_err(|e| failed(SourceStage::CargoMetadata, e))?;
    if !output.status.success() {
        return Err(failed(SourceStage::CargoMetadata, diagnostics(&output)));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| failed(SourceStage::CargoMetadata, e))?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| failed(SourceStage::CargoMetadata, "missing packages"))?;
    let matching: Vec<_> = packages
        .iter()
        .filter(|package| {
            package["manifest_path"]
                .as_str()
                .is_some_and(|path| Path::new(path) == manifest)
        })
        .collect();
    let [package] = matching.as_slice() else {
        return Err(failed(
            SourceStage::CargoMetadata,
            "expected one exact package manifest",
        ));
    };
    let targets = package["targets"]
        .as_array()
        .ok_or_else(|| failed(SourceStage::CargoMetadata, "missing targets"))?;
    let matching: Vec<_> = targets
        .iter()
        .filter(|target| {
            target["name"] == input.cargo_target.name
                && target["kind"]
                    .as_array()
                    .is_some_and(|kinds| kinds.iter().any(|kind| kind == &input.cargo_target.kind))
        })
        .collect();
    let [root_target] = matching.as_slice() else {
        return Err(failed(
            SourceStage::CargoMetadata,
            "expected one exact Cargo lib target",
        ));
    };
    let source = package_dir
        .join(&input.cargo_target.source_path)
        .canonicalize()
        .map_err(|e| failed(SourceStage::CargoMetadata, e))?;
    if root_target["src_path"].as_str().map(Path::new) != Some(source.as_path()) {
        return Err(failed(
            SourceStage::CargoMetadata,
            "Cargo source path differs from fixture",
        ));
    }
    let cargo_workspace = metadata["workspace_root"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| failed(SourceStage::CargoMetadata, "missing workspace root"))?;
    let wrapper = case.join("capture-rustc.sh");
    std::fs::write(&wrapper, WRAPPER).map_err(|e| failed(SourceStage::Invocation, e))?;
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700))
        .map_err(|e| failed(SourceStage::Invocation, e))?;
    let argv_file = case.join("cargo-root.argv");
    let env_file = case.join("cargo-root.env");
    let mut command = clean_command(env!("CARGO"));
    command.current_dir(&cargo_workspace).args([
        "check", "--offline", "--locked", "--release", "-Zbuild-std=core", "--lib",
        "--target", "amdgcn-amd-amdhsa", "--message-format=json-render-diagnostics", "--manifest-path",
    ]).arg(&manifest).arg("--target-dir").arg(target)
        .env("RUSTC_WRAPPER", &wrapper)
        .env(CAPTURE_ARGS, &argv_file).env(CAPTURE_ENV, &env_file)
        .env(CAPTURE_MANIFEST, package_dir).env(CAPTURE_CRATE, &input.cargo_target.name)
        .env("CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS", format!(
            "-Zalways-encode-mir -Ctarget-cpu={} -Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32 -Coverflow-checks=on", fixture.target));
    feature_args(&mut command, fixture);
    let output = command
        .output()
        .map_err(|e| failed(SourceStage::CargoDependencies, e))?;
    let cargo_diagnostics = diagnostics(&output);
    // A successful ordinary Cargo root compile is not P4 qualification. Capture
    // must be the intentional final failure, after its dependencies completed.
    if output.status.success()
        || !argv_file.is_file()
        || !env_file.is_file()
        || !String::from_utf8_lossy(&output.stderr).contains("FE2O3_TEST_P4_ROOT_CAPTURED:")
    {
        return Err(failed(SourceStage::CargoDependencies, cargo_diagnostics));
    }
    let raw = std::fs::read(&argv_file).map_err(|e| failed(SourceStage::Invocation, e))?;
    let rows = records(&raw)?;
    let (cwd, arguments) = rows
        .split_first()
        .ok_or_else(|| failed(SourceStage::Invocation, "empty Cargo capture"))?;
    let cwd = PathBuf::from(OsString::from_vec(cwd.to_vec()));
    if cwd
        .canonicalize()
        .map_err(|e| failed(SourceStage::Invocation, e))?
        != cargo_workspace
            .canonicalize()
            .map_err(|e| failed(SourceStage::Invocation, e))?
    {
        return Err(failed(
            SourceStage::Invocation,
            "Cargo compiler cwd differs from dependency workspace",
        ));
    }
    let mut args: Vec<String> = arguments
        .iter()
        .map(|argument| {
            String::from_utf8(argument.to_vec()).map_err(|e| failed(SourceStage::Invocation, e))
        })
        .collect::<Result<_, _>>()?;
    let actual: Vec<OsString> = args.iter().map(OsString::from).collect();
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(&actual).map_err(|e| failed(SourceStage::Invocation, e))?
    else {
        return Err(failed(
            SourceStage::Invocation,
            "captured root is not a source compilation",
        ));
    };
    if compile.crate_name() != input.cargo_target.name
        || cwd
            .join(compile.source_path())
            .canonicalize()
            .map_err(|e| failed(SourceStage::Invocation, e))?
            != source
        || option_values(&args, "--target")? != ["amdgcn-amd-amdhsa"]
        || codegen_values(&args, "target-cpu")? != [fixture.target.clone()]
        || codegen_values(&args, "target-feature")? != ["-xnack,+wavefrontsize64,-wavefrontsize32"]
    {
        return Err(failed(
            SourceStage::Invocation,
            "captured root crate/source/target mismatch",
        ));
    }
    let cfg = option_values(&args, "--cfg")?;
    for feature in &input.features {
        if !cfg.contains(&format!("feature=\"{feature}\"")) {
            return Err(failed(
                SourceStage::Invocation,
                format!("Cargo omitted requested root feature {feature}"),
            ));
        }
    }
    let metadata = ordered_rustc_codegen_metadata_v1(compile)
        .map_err(|e| failed(SourceStage::Invocation, e))?;
    if metadata.is_empty() {
        return Err(failed(
            SourceStage::Invocation,
            "Cargo omitted root codegen metadata",
        ));
    }
    let binding =
        derive_crate_binding_id_v1(compile.crate_name(), metadata.iter().map(String::as_str));
    let observation = derive_cargo_metadata_build_observation_v2(&metadata);
    let env_bytes = std::fs::read(env_file).map_err(|e| failed(SourceStage::Invocation, e))?;
    let mut environment = environment(&env_bytes)?;
    let observed_manifest = environment
        .iter()
        .find(|(key, _)| key == "CARGO_MANIFEST_DIR")
        .map(|(_, value)| PathBuf::from(value));
    if observed_manifest.as_deref() != Some(package_dir) {
        return Err(failed(
            SourceStage::Invocation,
            "captured Cargo package environment mismatch",
        ));
    }
    for (key, value) in [
        (CRATE_BINDING_ID_ENV_V1, binding.to_hex()),
        (
            CARGO_METADATA_BUILD_OBSERVATION_ENV_V2,
            observation.to_hex(),
        ),
    ] {
        environment.retain(|(existing, _)| existing != key);
        environment.push((key.into(), value.into()));
    }
    // The driver runs inside a libtest executable rather than the original
    // compiler binary. Preserve that compiler's implicit sysroot explicitly.
    make_sysroot_explicit(&mut args, &environment, &cwd)?;
    let output_directory = case.join("compiler-output");
    std::fs::create_dir(&output_directory).map_err(|e| failed(SourceStage::Invocation, e))?;
    redirect_output(&mut args, &output_directory)?;
    Ok(Captured {
        args,
        environment,
        cwd,
        cfg,
        cargo_diagnostics,
    })
}

#[test]
fn cargo_capture_keeps_platform_environment_values_and_refuses_duplicates() {
    let rows = environment(b"A=one=two\0B=line\nvalue\0").unwrap();
    assert_eq!(rows[0], ("A".into(), "one=two".into()));
    assert_eq!(rows[1], ("B".into(), "line\nvalue".into()));
    assert!(environment(b"A=x\0A=y\0").is_err());
    assert!(environment(b"A=x").is_err());
}

#[test]
fn cargo_output_redirection_preserves_all_other_root_arguments() {
    for option in [vec!["--out-dir", "/old"], vec!["--out-dir=/old"]] {
        let mut args: Vec<String> = [
            vec!["rustc", "--crate-name", "fixture"],
            option,
            vec!["--cfg", "feature=\"selected\"", "source.rs"],
        ]
        .concat()
        .into_iter()
        .map(str::to_owned)
        .collect();
        redirect_output(&mut args, Path::new("/isolated")).unwrap();
        assert_eq!(option_values(&args, "--out-dir").unwrap(), ["/isolated"]);
        assert_eq!(
            option_values(&args, "--cfg").unwrap(),
            ["feature=\"selected\""]
        );
        assert_eq!(args.last().unwrap(), "source.rs");
    }
    assert!(
        redirect_output(
            &mut ["rustc".into(), "source.rs".into()],
            Path::new("/isolated")
        )
        .is_err()
    );
    assert_eq!(
        codegen_values(
            &[
                "rustc".into(),
                "-C".into(),
                "target-cpu=gfx942".into(),
                "-Ctarget-cpu=gfx950".into()
            ],
            "target-cpu"
        )
        .unwrap(),
        ["gfx942", "gfx950"]
    );
    assert!(WRAPPER.contains("exec \"$@\""));
    let mut explicit = vec![
        "compiler-must-not-run".to_owned(),
        "--sysroot=/already-selected".to_owned(),
    ];
    let original = explicit.clone();
    make_sysroot_explicit(&mut explicit, &[], Path::new("/")).unwrap();
    assert_eq!(explicit, original);
    explicit.push("--sysroot=/second".to_owned());
    assert!(make_sysroot_explicit(&mut explicit, &[], Path::new("/")).is_err());
}

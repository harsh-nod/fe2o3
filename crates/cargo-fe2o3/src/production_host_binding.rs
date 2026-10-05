//! Original committed device identity projected into its ordinary host library.
//! This carries a macro namespace, never proof, load, or launch authority.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use fe2o3_compiler_closure_capability::CompilerExecutionClientProfileCapabilityV1;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_rustc_invocation::{
    RUSTC_SEPARATE_VALUE_OPTIONS_V2, RustcCompileInvocationV2, RustcInvocationDescriptorV3,
    RustcInvocationV2, classify_rustc_invocation_v2, ordered_rustc_codegen_metadata_v1,
};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, derive_crate_binding_id_v1,
};
use serde::{Deserialize, Serialize};

use crate::application_handoff::PinnedApplicationEnvelope;
use crate::authorized_kernel_closure::AuthorizedKernelClosureV1;
use crate::binding_check_projection::{ObjectIdentity, Projection, SealedProjection, TargetSource};
use crate::binding_check_wrapper::{
    lexical_normalize_absolute, validate_cargo_owner, validate_source_identity,
};
use crate::project::PinnedDirectory;

pub(crate) const MODE_ENV: &str = "FE2O3_PRODUCTION_HOST_BINDING_MODE_V1";
const FORMAT: &str = "fe2o3-production-host-binding-v1";
const MAX_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct UnitConfiguration {
    edition: String,
    crate_type: String,
    cfgs: Vec<String>,
    profile: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct HostProjection {
    format: String,
    source: Projection,
    crate_name: String,
    host_target: String,
    original_binding: String,
    unit: UnitConfiguration,
}

impl HostProjection {
    fn encode(&self) -> Result<Vec<u8>, String> {
        self.source.validate_and_encode()?;
        if self.format != FORMAT
            || self.source.targets.len() != 1
            || !self.source.targets[0].managed
            || self.crate_name.is_empty()
            || self.crate_name.len() > 256
            || !self
                .crate_name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || self.host_target.is_empty()
            || self.host_target.len() > 256
            || !self
                .host_target
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
            || CrateBindingIdV1::from_hex(&self.original_binding)
                .map_err(|e| e.to_string())?
                .to_hex()
                != self.original_binding
            || !matches!(
                self.unit.edition.as_str(),
                "2015" | "2018" | "2021" | "2024"
            )
            || !matches!(self.unit.crate_type.as_str(), "lib" | "rlib")
            || self.unit.cfgs.windows(2).any(|pair| pair[0] >= pair[1])
            || self.unit.cfgs.iter().any(|cfg| reserved_cfg(cfg))
        {
            return Err("invalid production host binding projection".to_owned());
        }
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("production host binding projection exceeds its byte bound".to_owned());
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err("invalid production host projection size".to_owned());
        }
        let projection: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if projection.encode()? != bytes {
            return Err("production host binding projection is not canonical".to_owned());
        }
        Ok(projection)
    }

    fn binding_for(
        &self,
        compile: RustcCompileInvocationV2<'_>,
        cwd: &Path,
        package_name: Option<&OsStr>,
        manifest_dir: Option<&OsStr>,
    ) -> Result<Option<&str>, String> {
        let source = absolute_source(compile.source_path(), cwd)?;
        let target = &self.source.targets[0];
        if source != target.source_path {
            return Ok(None);
        }
        validate_cargo_owner(target, package_name, manifest_dir)?;
        let observed = library_configuration(compile, &self.host_target)?;
        if compile.crate_name() != self.crate_name || observed != self.unit {
            return Err(format!(
                "host library differs from its original device unit: crate {:?} / {:?}, configuration {:?} / {:?}",
                compile.crate_name(),
                self.crate_name,
                observed,
                self.unit,
            ));
        }
        Ok(Some(&self.original_binding))
    }
}

struct RetainedSource {
    workspace: PinnedDirectory,
    package: PinnedDirectory,
    file: File,
    target: TargetSource,
}

impl RetainedSource {
    fn open(projection: &Projection) -> Result<Self, String> {
        let target = projection
            .targets
            .first()
            .ok_or("missing host binding source")?
            .clone();
        let workspace = PinnedDirectory::open_existing(
            projection.workspace_root.clone(),
            "host binding workspace",
        )?;
        let package =
            PinnedDirectory::open_existing(target.package_root.clone(), "host binding package")?;
        if !workspace.matches_identity(projection.workspace_device, projection.workspace_inode)
            || !package.matches_identity(target.package_device, target.package_inode)
        {
            return Err("host binding directory identity changed".to_owned());
        }
        let file = crate::example_manifest::open_contained_regular_file(
            &workspace,
            &target.source_path,
            "host binding source",
        )?;
        let retained = Self {
            workspace,
            package,
            file,
            target,
        };
        retained.revalidate()?;
        Ok(retained)
    }

    fn revalidate(&self) -> Result<(), String> {
        self.workspace.validate_path("host binding workspace")?;
        self.package.validate_path("host binding package")?;
        validate_source_identity(
            &self.target,
            ObjectIdentity::from_stat(&rustix::fs::fstat(&self.file).map_err(|e| e.to_string())?)?,
        )?;
        let current = crate::example_manifest::open_contained_regular_file(
            &self.workspace,
            &self.target.source_path,
            "host binding source",
        )?;
        validate_source_identity(
            &self.target,
            ObjectIdentity::from_stat(&rustix::fs::fstat(&current).map_err(|e| e.to_string())?)?,
        )
    }
}

pub(crate) struct CommittedHostBindingProjection<'a> {
    envelope: PinnedApplicationEnvelope<'a>,
    closure: &'a AuthorizedKernelClosureV1,
    profile: &'a CompilerExecutionClientProfileCapabilityV1,
    source: RetainedSource,
    sealed: SealedProjection,
}

impl<'a> CommittedHostBindingProjection<'a> {
    pub(crate) fn prepare(
        context: &'a crate::BackendRunContext,
        admission: &'a crate::authority_release::ProtectedReleaseAdmission,
    ) -> Result<Self, String> {
        let closure = context
            .authorized_closure
            .as_ref()
            .ok_or("host binding requires the original authorized source closure")?;
        closure.revalidate()?;
        let mut envelope = PinnedApplicationEnvelope::discover(context.generation.artifact_dir())?
            .ok_or("host binding requires a current committed application envelope")?;
        envelope.revalidate()?;
        let profile = admission.compiler_execution_profile_capability();
        validate_receipt(&envelope, profile)?;
        let outer =
            InertSemanticCompilerModuleHandoffV3::decode(envelope.wire().replay().outer_handoff())
                .map_err(|e| e.to_string())?;
        let descriptor = outer.capsule().invocation();
        if context.protected_compiler_closure.as_ref() != Some(descriptor.compiler_closure())
            || admission.compiler_closure() != *descriptor.compiler_closure()
            || descriptor.amd_target() != context.target_profile.device_target()
        {
            return Err(
                "committed host binding has a different compiler closure or target".to_owned(),
            );
        }
        let argv = descriptor
            .rustc()
            .argv()
            .map(OsString::from)
            .collect::<Vec<_>>();
        let RustcInvocationV2::Compile(compile) =
            classify_rustc_invocation_v2(&argv).map_err(|e| e.to_string())?
        else {
            return Err("committed host binding has no original device compile".to_owned());
        };
        let cwd = Path::new(descriptor.rustc().working_directory());
        if !context.build_config.as_ref().is_some_and(|config| {
            config.selects_only_unit(compile.crate_name(), compile.source_path(), cwd)
        }) {
            return Err("host binding requires the exact sole configured device unit".to_owned());
        }
        let binding = original_binding(descriptor, compile)?;
        let unit = committed_device_configuration(
            &argv,
            context.target_profile,
            &context.managed_rustc_args,
        )?;
        let package_root = PathBuf::from(environment(descriptor, "CARGO_MANIFEST_DIR")?);
        let package_name = closure.package_name_for_root(&package_root)?.to_owned();
        let package = PinnedDirectory::open_existing(package_root.clone(), "host binding package")?;
        let source_path = absolute_source(compile.source_path(), cwd)?;
        let workspace = context.project.workspace_root();
        let file = crate::example_manifest::open_contained_regular_file(
            workspace,
            &source_path,
            "host binding source",
        )?;
        let (workspace_device, workspace_inode) = workspace.identity_parts();
        let (package_device, package_inode) = package.identity_parts();
        let projection = HostProjection {
            format: FORMAT.to_owned(),
            source: Projection {
                workspace_root: workspace.display_path().to_path_buf(),
                workspace_device,
                workspace_inode,
                targets: vec![TargetSource {
                    package_name,
                    package_root,
                    package_device,
                    package_inode,
                    source_path,
                    source_identity: ObjectIdentity::from_stat(
                        &rustix::fs::fstat(&file).map_err(|e| e.to_string())?,
                    )?,
                    managed: true,
                }],
            },
            crate_name: compile.crate_name().to_owned(),
            host_target: context.host_target.clone(),
            original_binding: binding.to_hex(),
            unit,
        };
        let sealed = SealedProjection::for_production_host(&projection.encode()?)?;
        let source = RetainedSource::open(&projection.source)?;
        let mut retained = Self {
            envelope,
            closure,
            profile,
            source,
            sealed,
        };
        retained.revalidate()?;
        Ok(retained)
    }

    pub(crate) fn configure_child(
        &self,
        command: &mut Command,
        wrapper: &crate::pinned_executable::PinnedExecutable,
    ) -> Result<(), String> {
        configure_host_wrapper(command, wrapper, &self.sealed)
    }

    pub(crate) fn revalidate(&mut self) -> Result<(), String> {
        self.closure.revalidate()?;
        self.envelope.revalidate()?;
        validate_receipt(&self.envelope, self.profile)?;
        self.source.revalidate()
    }
}

fn configure_host_wrapper(
    command: &mut Command,
    wrapper: &crate::pinned_executable::PinnedExecutable,
    sealed: &SealedProjection,
) -> Result<(), String> {
    let path = wrapper
        .fixed_child_path(crate::CARGO_BINDING_CHECK_WRAPPER_CHILD_FD)
        .map_err(|e| e.to_string())?;
    wrapper
        .inherit_for_child_at(command, crate::CARGO_BINDING_CHECK_WRAPPER_CHILD_FD)
        .map_err(|e| e.to_string())?;
    sealed.inherit_for_child_at(command, crate::CARGO_BINDING_CHECK_PROJECTION_CHILD_FD)?;
    command
        .env("RUSTC_WORKSPACE_WRAPPER", &path)
        .env("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", &path)
        .env(MODE_ENV, "1")
        .env_remove(CRATE_BINDING_ID_ENV_V1)
        .env_remove(crate::binding_check_wrapper::MODE_ENV_V1)
        .env_remove(crate::binding_check_wrapper::CLIPPY_DRIVER_ENV_V1);
    Ok(())
}

fn validate_receipt(
    envelope: &PinnedApplicationEnvelope<'_>,
    profile: &CompilerExecutionClientProfileCapabilityV1,
) -> Result<(), String> {
    let subject = envelope
        .wire()
        .reconstructed_compiler_execution_subject_v1()
        .map_err(|e| e.to_string())?;
    crate::compiler_execution_boundary::validate_compiler_execution_receipt_carriage(
        profile,
        &subject,
        envelope.wire().compiler_execution_receipt(),
    )
    .map_err(|e| e.to_string())
}

fn environment<'a>(
    descriptor: &'a RustcInvocationDescriptorV3,
    name: &str,
) -> Result<&'a str, String> {
    descriptor
        .compile_environment()
        .entries()
        .iter()
        .find(|entry| entry.key() == name)
        .map(|entry| entry.value())
        .ok_or_else(|| format!("original device invocation omitted {name}"))
}

fn original_binding(
    descriptor: &RustcInvocationDescriptorV3,
    compile: RustcCompileInvocationV2<'_>,
) -> Result<CrateBindingIdV1, String> {
    let metadata = ordered_rustc_codegen_metadata_v1(compile).map_err(|e| e.to_string())?;
    if metadata.is_empty() {
        return Err("original device compile has no metadata".to_owned());
    }
    let binding =
        derive_crate_binding_id_v1(compile.crate_name(), metadata.iter().map(String::as_str));
    if environment(descriptor, CRATE_BINDING_ID_ENV_V1)? != binding.to_hex() {
        return Err("committed device binding differs from original Cargo metadata".to_owned());
    }
    Ok(binding)
}

fn absolute_source(source: &Path, cwd: &Path) -> Result<PathBuf, String> {
    lexical_normalize_absolute(&if source.is_absolute() {
        source.to_path_buf()
    } else {
        cwd.join(source)
    })
}

fn committed_device_configuration(
    argv: &[OsString],
    profile: fe2o3_amd_target::ProductionAmdTargetProfileV1,
    managed: &OsStr,
) -> Result<UnitConfiguration, String> {
    let mut suffix = profile
        .cargo_rustflags()
        .split_ascii_whitespace()
        .map(OsString::from)
        .collect::<Vec<_>>();
    suffix.extend(
        crate::binding_wrapper::decode_managed_rustc_args(managed).map_err(|e| e.to_string())?,
    );
    let cargo_argv = argv
        .strip_suffix(suffix.as_slice())
        .ok_or("committed device invocation has a different generation or target flags")?;
    let cargo_argv = without_device_build_std_gate(cargo_argv)?;
    let RustcInvocationV2::Compile(compile) =
        classify_rustc_invocation_v2(cargo_argv).map_err(|e| e.to_string())?
    else {
        return Err("original device Cargo unit is not a compile".to_owned());
    };
    library_configuration(compile, profile.rustc_target())
}

fn without_device_build_std_gate(argv: &[OsString]) -> Result<&[OsString], String> {
    // Pinned Cargo emits this exact pair after its noprelude sysroot imports for
    // our owned build-std=core phase. It is not accepted in the host invocation.
    let prefix = argv
        .strip_suffix(&["-Z".into(), "unstable-options".into()])
        .ok_or("committed device invocation lacks its exact build-std gate")?;
    let mut core = 0;
    let mut builtins = 0;
    let mut index = 1;
    while index < prefix.len() {
        let argument = prefix[index].to_str().unwrap_or("");
        if RUSTC_SEPARATE_VALUE_OPTIONS_V2.contains(&argument) {
            index += 1;
            if argument == "--extern" {
                let value = prefix
                    .get(index)
                    .and_then(|value| value.to_str())
                    .unwrap_or("");
                if value.starts_with("noprelude,nounused:core=/") {
                    core += 1;
                } else if value.starts_with("noprelude,nounused:compiler_builtins=/") {
                    builtins += 1;
                } else if has_noprelude_option(value) {
                    return Err(format!(
                        "unreviewed build-std import in committed device invocation: {value}"
                    ));
                }
            }
        } else if argument
            .strip_prefix("--extern=")
            .is_some_and(has_noprelude_option)
        {
            return Err("build-std imports differ from pinned Cargo's split syntax".to_owned());
        }
        index += 1;
    }
    if core != 1 || builtins != 1 {
        return Err("device unstable-options gate lacks exact build-std imports".to_owned());
    }
    Ok(prefix)
}

fn has_noprelude_option(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(options, _)| options.split(',').any(|option| option == "noprelude"))
}

fn reserved_cfg(value: &str) -> bool {
    value
        .split('=')
        .next()
        .is_some_and(|key| key.trim() == "fe2o3_codegen_generation")
}

fn library_configuration(
    compile: RustcCompileInvocationV2<'_>,
    expected_target: &str,
) -> Result<UnitConfiguration, String> {
    let mut edition = None;
    let mut target = None;
    let mut crate_type = None;
    let mut cfgs = Vec::new();
    let mut profile = std::collections::BTreeMap::new();
    let argv = compile.argv();
    let mut index = 1;
    while index < argv.len() {
        let argument = &argv[index];
        if argument == "--test" || argument == "--" || argument.as_encoded_bytes().starts_with(b"@")
        {
            return Err(
                "host binding requires an inspectable ordinary library invocation".to_owned(),
            );
        }
        let text = argument.to_str().unwrap_or("");
        if text.starts_with("-Z") {
            return Err(format!(
                "unreviewed unstable option in host binding unit: {text}"
            ));
        }
        let (key, value) = if RUSTC_SEPARATE_VALUE_OPTIONS_V2.contains(&text) {
            index += 1;
            (text, argv.get(index).and_then(|v| v.to_str()))
        } else if let Some(codegen) = text.strip_prefix("-C") {
            ("-C", Some(codegen))
        } else if text == "-O" {
            ("-C", Some("opt-level=3"))
        } else if text == "-g" {
            ("-C", Some("debuginfo=2"))
        } else if let Some((key, value)) = text.split_once('=') {
            (key, Some(value))
        } else {
            (text, None)
        };
        match key {
            "--edition" => set_once(&mut edition, value, "edition")?,
            "--target" => set_once(&mut target, value, "target")?,
            "--crate-type" => set_once(&mut crate_type, value, "crate type")?,
            "--cfg" => {
                let value = value.ok_or("host binding cfg is not UTF-8")?;
                if reserved_cfg(value) {
                    return Err("unexpected generation cfg in host binding unit".to_owned());
                }
                cfgs.push(value.to_owned());
            }
            "-C" | "--codegen" => {
                let value = value.ok_or("host binding codegen option is not UTF-8")?;
                let (name, setting) = value.split_once('=').unwrap_or((value, "yes"));
                let name = name.replace('_', "-");
                match name.as_str() {
                    "opt-level" | "debuginfo" | "debug-assertions" | "overflow-checks"
                    | "panic" | "lto" | "codegen-units" | "strip" | "embed-bitcode"
                    | "incremental" => {
                        let setting = if name == "incremental" {
                            "present"
                        } else {
                            setting
                        };
                        if profile.insert(name, setting.to_owned()).is_some() {
                            return Err("duplicate profile option in host binding unit".to_owned());
                        }
                    }
                    "metadata" | "extra-filename" | "target-cpu" | "target-feature"
                    | "relocation-model" | "code-model" | "link-arg" | "link-args" | "linker"
                    | "linker-flavor" => {}
                    _ => {
                        return Err(format!(
                            "unreviewed codegen option in host binding unit: {name}"
                        ));
                    }
                }
            }
            "--crate-name" | "--emit" | "--out-dir" | "--extern" | "-L" | "-o" | "--check-cfg"
            | "--error-format" | "--json" | "--color" | "--diagnostic-width" | "--allow"
            | "--deny" | "--forbid" | "--warn" | "--force-warn" | "--cap-lints" | "-A" | "-D"
            | "-F" | "-W" => {}
            _ if argument == compile.source_path().as_os_str() => {}
            _ if ["-L", "-o", "-A", "-D", "-F", "-W"]
                .iter()
                .any(|prefix| text.starts_with(prefix)) => {}
            _ => {
                return Err(format!(
                    "unreviewed rustc option in host binding unit: {text}"
                ));
            }
        }
        index += 1;
    }
    if target.as_deref() != Some(expected_target)
        || !matches!(crate_type.as_deref(), Some("lib" | "rlib"))
    {
        return Err("host binding target or ordinary-library role differs".to_owned());
    }
    cfgs.sort();
    cfgs.dedup();
    Ok(UnitConfiguration {
        edition: edition.unwrap_or_else(|| "2015".to_owned()),
        crate_type: crate_type.ok_or("missing library role")?,
        cfgs,
        profile,
    })
}

fn set_once(slot: &mut Option<String>, value: Option<&str>, name: &str) -> Result<(), String> {
    let value = value
        .filter(|v| !v.is_empty())
        .ok_or_else(|| format!("host binding {name} is missing or not UTF-8"))?;
    if slot.replace(value.to_owned()).is_some() {
        return Err(format!("duplicate host binding {name}"));
    }
    Ok(())
}

pub(crate) fn run(argv: Vec<OsString>) -> Result<ExitStatus, String> {
    if std::env::var_os(MODE_ENV).as_deref() != Some(OsStr::new("1"))
        || std::env::var_os(crate::binding_check_wrapper::MODE_ENV_V1).is_some()
        || std::env::var_os(crate::BINDING_WRAPPER_MODE_ENV).is_some()
        || std::env::var_os(CRATE_BINDING_ID_ENV_V1).is_some()
        || std::env::var_os(crate::binding_check_wrapper::CLIPPY_DRIVER_ENV_V1).is_some()
    {
        return Err("mixed or prebound production host wrapper mode".to_owned());
    }
    crate::binding_check_wrapper::reject_codegen_backend(&argv).map_err(|e| e.to_string())?;
    let classification = host_classification_argv(&argv)?;
    let invocation = classify_rustc_invocation_v2(&classification).map_err(|e| e.to_string())?;
    if invocation.executable() != OsStr::new(&format!("/proc/self/fd/{}", crate::RUSTC_CHILD_FD)) {
        return Err("production host wrapper requires the pinned rustc descriptor".to_owned());
    }
    let projection =
        HostProjection::decode(&crate::binding_check_projection::consume_inherited_host_bytes()?)?;
    let mut command = Command::new(invocation.executable());
    command
        .args(&argv[1..])
        .stdin(Stdio::null())
        .env_remove(MODE_ENV)
        .env_remove(CRATE_BINDING_ID_ENV_V1);
    crate::remove_dynamic_loader_environment(&mut command);
    command.env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{}", crate::RUSTC_LIBRARY_CHILD_FD),
    );
    let source = match invocation {
        RustcInvocationV2::Compile(compile) => {
            let binding = projection.binding_for(
                compile,
                &std::env::current_dir().map_err(|e| e.to_string())?,
                std::env::var_os("CARGO_PKG_NAME").as_deref(),
                std::env::var_os("CARGO_MANIFEST_DIR").as_deref(),
            )?;
            if let Some(binding) = binding {
                let source = RetainedSource::open(&projection.source)?;
                command.env(CRATE_BINDING_ID_ENV_V1, binding);
                Some(source)
            } else {
                None
            }
        }
        RustcInvocationV2::Terminal(_) | RustcInvocationV2::Query(_)
            if invocation.is_bootstrap_passthrough_approved() =>
        {
            None
        }
        _ => return Err("unsupported production host rustc invocation".to_owned()),
    };
    let result = crate::process_execution::status(&mut command).map_err(|e| e.to_string());
    let validation = source.as_ref().map_or(Ok(()), RetainedSource::revalidate);
    match (result, validation) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(changed)) => Err(format!("{error}; host binding revalidation: {changed}")),
    }
}

fn host_classification_argv(argv: &[OsString]) -> Result<Cow<'_, [OsString]>, String> {
    let error = match classify_rustc_invocation_v2(argv) {
        Ok(_) => return Ok(Cow::Borrowed(argv)),
        Err(error) => error,
    };
    // Classify the static application's target-information query without its exact native
    // link profile. The original argv is still forwarded, with null stdin and no binding.
    let flags = [
        "-Ctarget-feature=+crt-static",
        "-Crelocation-model=static",
        "-Clink-arg=-no-pie",
    ];
    if let Some(start) = argv.windows(flags.len()).position(|args| {
        args.iter()
            .zip(flags)
            .all(|(argument, flag)| argument == flag)
    }) {
        let mut query = argv.to_vec();
        query.drain(start..start + flags.len());
        if query.iter().any(|argument| {
            let bytes = argument.as_encoded_bytes();
            bytes.starts_with(b"-C")
                || bytes.starts_with(b"-Z")
                || bytes == b"--codegen"
                || bytes.starts_with(b"--codegen=")
        }) {
            return Err("static host query contains additional codegen controls".to_owned());
        }
        if let Ok(invocation @ RustcInvocationV2::Query(_)) = classify_rustc_invocation_v2(&query)
            && invocation.is_bootstrap_passthrough_approved()
        {
            return Ok(Cow::Owned(query));
        }
    }
    Err(error.to_string())
}

#[cfg(test)]
mod tests;

//! Feature-only Cargo qualification harness.
//!
//! This module deliberately does not participate in the public `build` or `run`
//! dispatch. A qualification-enabled `cargo-fe2o3` image may enter it only through
//! [`INTERNAL_COMMAND_ARG`], with an explicitly reviewed qualification oracle.
//! Row-softmax authority-sensitive qualification remains a debug-only validation path: it
//! authenticates the compiler and kernel closure but cannot claim a protected
//! production release launch.

use super::*;

use std::os::unix::process::CommandExt as _;

pub(super) const INTERNAL_COMMAND_ARG: &str = "__fe2o3-qualification-harness-v1";
pub(super) const INTERNAL_RUNNER_ARG: &str = "__fe2o3-qualification-runner-v1";
pub(super) const INTERNAL_SUPERVISOR_ARG: &str = "__fe2o3-qualification-application-supervisor-v1";

const QUALIFICATION_SEMANTIC_DOMAIN: &[u8] = b"fe2o3-qualification-harness-v1\0";
const RUNNER_EXPECTS_NO_ENVELOPE: &str = "none";
const QUALIFICATION_BACKEND_ENV: &str = "FE2O3_QUALIFICATION_BACKEND_V1";
const AUTHORITY_BEARING_ROW_ORACLE: &str = "collected-row-softmax-v1";

/// Runs one explicitly internal qualification Cargo operation.
pub(super) fn command(args: &[OsString]) -> ExitCode {
    match command_result(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cargo-fe2o3 qualification harness: {error}");
            ExitCode::FAILURE
        }
    }
}

fn command_result(args: &[OsString]) -> Result<(), String> {
    let (command, args) = args
        .split_first()
        .ok_or_else(|| "qualification harness requires build or run".to_owned())?;
    let command = command
        .to_str()
        .ok_or_else(|| "qualification harness command must be UTF-8".to_owned())?;
    if !matches!(command, "build" | "run") {
        return Err(format!(
            "qualification harness admits only build or run, found {command:?}"
        ));
    }

    let oracle = required_reviewed_qualification_oracle()?;
    reject_obsolete_codegen_pipeline(env::var_os(OBSOLETE_CODEGEN_PIPELINE_ENV).as_deref())?;
    reject_dynamic_loader_environment()?;
    scrub_process_dynamic_loader_environment();
    reject_preexisting_compiler_environment()?;

    // This support entry both validates the closed reviewed selector and pins any
    // envelope inputs before Cargo or a worker process can be started.
    let build_config =
        build_config::PreparedBuildConfig::from_environment_for_qualification_harness(
            oracle.as_os_str(),
        )
        .map_err(|error| format!("qualification build configuration setup failed: {error}"))?;

    let requires_authorized_closure = oracle == OsStr::new(AUTHORITY_BEARING_ROW_ORACLE);
    if requires_authorized_closure {
        // With no ProtectedReleaseAdmission, this admits only the explicit debug-build
        // validation marker. The hidden harness never manufactures a release claim.
        require_protected_authority_launch(None)?;
        reject_authority_environment_overrides(args)?;
        reject_unprotected_qualification_trampoline_environment()?;
    } else {
        reject_qualification_authority_environment()?;
    }

    let authority_rustc_sha256 = requires_authorized_closure
        .then(authority_rustc_sha256_from_environment)
        .transpose()?;
    let authority_rustc_lib_tree_sha256 = requires_authorized_closure
        .then(authority_rustc_runtime_sha256_from_environment)
        .transpose()?;
    let authority_cargo_sha256 = requires_authorized_closure
        .then(authority_cargo_sha256_from_environment)
        .transpose()?;
    let authority_backend_sha256 = requires_authorized_closure
        .then(authority_backend_sha256_from_environment)
        .transpose()?;
    let authority_backend = authority_backend_sha256
        .map(preflight_declared_qualification_authority_backend)
        .transpose()?;

    let invocation_directory = env::current_dir()
        .map_err(|error| format!("failed to resolve Cargo invocation directory: {error}"))?;
    let cargo_declaration = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let source_cargo = if requires_authorized_closure {
        let cargo_path = require_absolute_authority_tool_path(&cargo_declaration, "CARGO")?;
        reject_authority_rustup_proxy(&cargo_path, "Cargo")?;
        pinned_executable::PinnedExecutable::open(&cargo_path)
            .map_err(|error| format!("failed to pin qualification authority Cargo: {error}"))?
    } else {
        pin_default_cargo(&cargo_declaration, &invocation_directory)?
    };
    if let Some(expected) = authority_cargo_sha256
        && source_cargo.sha256() != &expected
    {
        return Err(format!(
            "qualification authority Cargo does not match {AUTHORITY_CARGO_SHA256_ENV}"
        ));
    }
    let pinned_cargo = if requires_authorized_closure {
        source_cargo
            .seal_executable_image()
            .map_err(|error| format!("failed to seal qualification authority Cargo: {error}"))?
    } else {
        source_cargo
    };
    let authority_rustc = if requires_authorized_closure {
        Some(pin_authority_rustc(
            &invocation_directory,
            authority_rustc_sha256.expect("qualification authority rustc digest parsed"),
            authority_rustc_lib_tree_sha256
                .expect("qualification authority rustc runtime-tree digest parsed"),
        )?)
    } else {
        None
    };
    let project = project::CargoProject::discover(
        args,
        Some(&pinned_cargo),
        authority_rustc.as_ref(),
        requires_authorized_closure,
    )?;
    reject_configured_compiler_selection(
        &project,
        args,
        &pinned_cargo,
        authority_rustc.as_ref(),
        requires_authorized_closure,
    )?;
    if requires_authorized_closure {
        reject_authority_config_overrides(
            &project,
            args,
            &pinned_cargo,
            authority_rustc
                .as_ref()
                .expect("qualification authority rustc pinned"),
        )?;
    }
    let pinned_rustc = match authority_rustc {
        Some(rustc) => rustc,
        None => pin_default_rustc(&project)?,
    };
    pinned_rustc.assert_lib_tree_unmutated()?;
    let authorized_closure = requires_authorized_closure
        .then(|| {
            authorized_kernel_closure::AuthorizedKernelClosureV1::observe(
                &project,
                args,
                &pinned_cargo,
                &pinned_rustc,
            )
        })
        .transpose()?;

    let mut context = QualificationRunContext::prepare(
        oracle,
        project,
        build_config,
        pinned_cargo,
        pinned_rustc,
        authority_backend,
        authorized_closure,
        args,
    )?;
    context.run(command, args)
}

fn required_reviewed_qualification_oracle() -> Result<OsString, String> {
    let oracle = env::var_os(build_config::QUALIFICATION_ORACLE_ENV).ok_or_else(|| {
        format!(
            "qualification harness requires nonempty {}",
            build_config::QUALIFICATION_ORACLE_ENV
        )
    })?;
    if oracle.is_empty() {
        return Err(format!(
            "qualification harness requires nonempty {}",
            build_config::QUALIFICATION_ORACLE_ENV
        ));
    }
    Ok(oracle)
}

fn reject_qualification_authority_environment() -> Result<(), String> {
    for name in [
        AUTHORITY_CARGO_SHA256_ENV,
        AUTHORITY_RUSTC_SHA256_ENV,
        AUTHORITY_RUSTC_PATH_ENV,
        AUTHORITY_RUSTC_RUNTIME_SHA256_ENV,
        AUTHORITY_BACKEND_SHA256_ENV,
        AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV,
        AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV,
    ] {
        if let Some(value) = env::var_os(name) {
            return Err(format!(
                "ordinary qualification harness rejects authority input {name}={value:?}"
            ));
        }
    }
    Ok(())
}

fn reject_unprotected_qualification_trampoline_environment() -> Result<(), String> {
    for name in [
        AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV,
        AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV,
    ] {
        if let Some(value) = env::var_os(name) {
            return Err(format!(
                "unprotected qualification authority rejects release trampoline input {name}={value:?}"
            ));
        }
    }
    Ok(())
}

struct QualificationRunContext {
    oracle: OsString,
    target: String,
    project: project::CargoProject,
    backend: PathBuf,
    pinned_backend: pinned_codegen_backend::PinnedCodegenBackend,
    pinned_cargo: pinned_executable::PinnedExecutable,
    pinned_rustc: PinnedRustc,
    build_config: Option<build_config::PreparedBuildConfig>,
    build_config_identity: Option<build_config::BuildConfigIdentity>,
    compiler_closure_sha256: [u8; 32],
    target_dir: project::PinnedDirectory,
    generation: generation::PreparedGeneration,
    managed_rustc_args: OsString,
    binding_wrapper_path: PathBuf,
    pinned_binding_wrapper: pinned_executable::PinnedExecutable,
    build_session: fe2o3_artifact_transaction::BuildSession,
    requires_locked_closure: bool,
    authorized_closure: Option<authorized_kernel_closure::AuthorizedKernelClosureV1>,
}

impl QualificationRunContext {
    fn prepare(
        oracle: OsString,
        project: project::CargoProject,
        build_config: Option<build_config::PreparedBuildConfig>,
        pinned_cargo: pinned_executable::PinnedExecutable,
        pinned_rustc: PinnedRustc,
        authority_backend: Option<(PathBuf, pinned_codegen_backend::PinnedCodegenBackend)>,
        authorized_closure: Option<authorized_kernel_closure::AuthorizedKernelClosureV1>,
        args: &[OsString],
    ) -> Result<Self, String> {
        if build_config
            .as_ref()
            .is_some_and(build_config::PreparedBuildConfig::is_production_compilation)
        {
            return Err("qualification harness cannot admit a production build profile".to_owned());
        }

        let target = amd_gpu_target(false);
        let target_dir = project.open_or_create_target()?;
        pinned_rustc.assert_lib_tree_unmutated()?;
        let (backend, pinned_backend) = match authority_backend {
            Some(prebuilt) => prebuilt,
            None => {
                let backend =
                    find_or_build_qualification_backend(&target_dir, &pinned_cargo, &pinned_rustc)?;
                let pinned_backend =
                    pinned_codegen_backend::PinnedCodegenBackend::open(&backend)
                        .map_err(|error| format!("failed to pin qualification backend: {error}"))?;
                (backend, pinned_backend)
            }
        };
        pinned_rustc.assert_lib_tree_unmutated()?;

        let binding_wrapper_path = env::current_exe()
            .map_err(|error| format!("failed to locate cargo-fe2o3 executable: {error}"))?;
        let pinned_binding_wrapper =
            pinned_executable::PinnedExecutable::open(&binding_wrapper_path)
                .map_err(|error| format!("failed to pin cargo-fe2o3 wrapper: {error}"))?;
        let compiler_closure_sha256 = compiler_toolchain::compiler_closure_sha256_v1(
            pinned_cargo.sha256(),
            pinned_rustc.executable.sha256(),
            pinned_rustc.lib_tree_sha256(),
            pinned_backend.sha256(),
        );
        let build_config_identity = build_config.as_ref().map(|config| config.identity());
        let build_session = random_build_session()?;

        let mut cargo_configuration = project.semantic_configuration(
            args,
            &pinned_cargo,
            authorized_closure.is_some().then_some(&pinned_rustc),
        )?;
        append_qualification_semantic_configuration(&mut cargo_configuration, &oracle)?;
        if let Some(authorized_closure) = authorized_closure.as_ref() {
            cargo_configuration.extend_from_slice(b"fe2o3-authorized-kernel-closure-v1\0");
            cargo_configuration
                .extend_from_slice(&(authorized_closure.snapshot().len() as u64).to_le_bytes());
            cargo_configuration.extend_from_slice(authorized_closure.snapshot());
        }
        let backend_reference = pinned_backend
            .fixed_child_descriptor_path(BACKEND_CHILD_FD)
            .map_err(|error| format!("failed to retain qualification backend: {error}"))?;
        let semantic = generation::semantic_identity(
            &target,
            &compiler_closure_sha256,
            build_config_identity,
            &cargo_configuration,
        )?;
        let generation = generation::PreparedGeneration::prepare(&target_dir, semantic)?;
        let managed_rustc_args =
            generation::managed_rustc_args(&backend_reference, generation.token())?;

        Ok(Self {
            oracle,
            target,
            project,
            backend,
            pinned_backend,
            pinned_cargo,
            pinned_rustc,
            build_config,
            build_config_identity,
            compiler_closure_sha256,
            target_dir,
            generation,
            managed_rustc_args,
            binding_wrapper_path,
            pinned_binding_wrapper,
            build_session,
            requires_locked_closure: authorized_closure.is_some(),
            authorized_closure,
        })
    }

    fn run(&mut self, command: &str, args: &[OsString]) -> Result<(), String> {
        self.project.validate_paths()?;
        self.target_dir.validate_path("Cargo target directory")?;
        self.generation.reject_if_substituted()?;
        eprintln!(
            "cargo-fe2o3 qualification {command}: device phase uses backend {} for target {}",
            self.backend.display(),
            self.target
        );

        let mut cargo = self
            .pinned_cargo
            .command()
            .map_err(|error| format!("failed to prepare pinned Cargo executable: {error}"))?;
        let mut forwarded_args = args.to_vec();
        if self.requires_locked_closure {
            let position = forwarded_args
                .iter()
                .position(|argument| argument == "--")
                .unwrap_or(forwarded_args.len());
            for required in ["--offline", "--frozen"] {
                if !forwarded_args.iter().any(|argument| argument == required) {
                    forwarded_args.insert(position, OsString::from(required));
                }
            }
        }
        if command == "run" {
            inject_qualification_application_runner(
                &self.project,
                &self.pinned_cargo,
                &self.pinned_rustc,
                self.generation.artifact_dir(),
                &mut forwarded_args,
                self.build_config
                    .as_ref()
                    .is_some_and(|config| config.envelope_mode().is_required()),
                self.requires_locked_closure,
            )?;
        }

        let capability_profile = if self
            .build_config
            .as_ref()
            .is_some_and(build_config::PreparedBuildConfig::requires_source_debug_profile)
        {
            capability_broker::CapabilityProfileV1::S09
        } else {
            capability_broker::CapabilityProfileV1::Ordinary
        };
        let rustc_lib_tree_stat = rustix::fs::fstat(self.pinned_rustc.lib_tree_directory().file())
            .map_err(|error| {
                format!("failed to inspect retained rustc lib-tree directory: {error}")
            })?;
        let retained_object_binding_sha256 = compiler_toolchain::retained_object_binding_sha256_v1(
            &self.compiler_closure_sha256,
            rustc_lib_tree_stat.st_dev,
            rustc_lib_tree_stat.st_ino,
            rustc_lib_tree_stat.st_mode,
        );
        let config_identity = self
            .build_config_identity
            .map(|identity| *identity.as_bytes());
        let binding = capability_broker::CapabilityBindingV3::new(
            capability_profile,
            config_identity,
            self.compiler_closure_sha256,
            *self.pinned_rustc.executable.sha256(),
            retained_object_binding_sha256,
        )?;
        let capability_broker = capability_broker::CapabilityBroker::start(
            self.build_session,
            binding,
            &self.pinned_backend,
            self.generation.artifact_dir(),
            &self.pinned_cargo,
        )?;
        let invocation_authorization = capability_broker.invocation_authorization();
        let pending_invocation_boundary =
            cargo_invocation_boundary::PendingCargoInvocationBoundary::start(
                &self.pinned_cargo,
                &self.pinned_binding_wrapper,
                None,
                invocation_authorization.clone(),
            )?;

        cargo
            .as_command_mut()
            .arg(command)
            .args(&forwarded_args)
            .current_dir(self.project.invocation_dir().child_path())
            .env_remove(HSACO_DIR_ENV)
            .env_remove(BACKEND_ENV)
            .env_remove(QUALIFICATION_BACKEND_ENV)
            .env(
                capability_broker::CAPABILITY_BROKER_ENV,
                capability_broker.route(),
            )
            .env(TARGET_ENV, &self.target)
            .env(
                build_config::QUALIFICATION_ORACLE_ENV,
                self.oracle.as_os_str(),
            )
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_BUILD_RUSTC_WRAPPER", "")
            .env("RUSTC_WORKSPACE_WRAPPER", &self.binding_wrapper_path)
            .env_remove(CARGO_PRIMARY_PACKAGE_ENV)
            .env(BINDING_WRAPPER_MODE_ENV, "1")
            .env(MANAGED_RUSTC_ARGS_ENV, &self.managed_rustc_args)
            .env(
                EXPECTED_RUSTC_SHA256_ENV,
                hex_encode(self.pinned_rustc.executable.sha256()),
            )
            .env(
                EXPECTED_COMPILER_CLOSURE_SHA256_ENV,
                hex_encode(&self.compiler_closure_sha256),
            )
            .env(BUILD_SESSION_ENV, self.build_session.to_hex())
            .env_remove(build_config::PRODUCTION_BUILD_CONFIG_ENV)
            .env_remove(build_config::PRODUCTION_BUILD_EXPECTED_ID_ENV)
            .env_remove(build_config::WORKER_V2_EXPECTED_ID_ENV)
            .env_remove(AUTHORITY_CARGO_SHA256_ENV)
            .env_remove(AUTHORITY_RUSTC_SHA256_ENV)
            .env_remove(AUTHORITY_RUSTC_PATH_ENV)
            .env_remove(AUTHORITY_RUSTC_RUNTIME_SHA256_ENV)
            .env_remove(AUTHORITY_BACKEND_SHA256_ENV)
            .env_remove(AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_ENV)
            .env_remove(AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_ENV);
        scrub_simulation_build_environment(cargo.as_command_mut());
        if self.requires_locked_closure {
            cargo.as_command_mut().env("FE2O3_HIP_SYS_DISABLE", "1");
        }
        remove_dynamic_loader_environment(cargo.as_command_mut());
        self.pinned_rustc.assert_lib_tree_unmutated()?;
        configure_pinned_rustc_child(cargo.as_command_mut(), &self.pinned_rustc)?;
        cargo.as_command_mut().env(
            "LD_LIBRARY_PATH",
            format!("/proc/self/fd/{RUSTC_LIBRARY_CHILD_FD}"),
        );
        match (self.build_config_identity, self.build_config.as_ref()) {
            (Some(identity), Some(config)) => {
                cargo
                    .as_command_mut()
                    .env(config.expected_identity_environment(), identity.to_hex());
            }
            (None, None) => {}
            _ => unreachable!("qualification config and identity have matching presence"),
        }

        pending_invocation_boundary.configure_child(cargo.as_command_mut());
        let mut cargo_child = cargo
            .spawn()
            .map_err(|error| format!("failed to run pinned qualification Cargo: {error}"))?;
        let invocation_boundary = match pending_invocation_boundary
            .complete(cargo_child.id(), invocation_authorization)
        {
            Ok(boundary) => boundary,
            Err(error) => {
                let _ = cargo_child.kill();
                let cleanup_result = cargo_child.wait().map(|_| ()).map_err(|cleanup| {
                    format!("failed to reap rejected qualification Cargo child: {cleanup}")
                });
                drop(capability_broker);
                let lib_tree_result = self.pinned_rustc.revalidate_lib_tree();
                let closure_result = self
                    .authorized_closure
                    .as_ref()
                    .map_or(Ok(()), |closure| closure.revalidate());
                return aggregate_post_spawn_results(
                    Err(error),
                    [
                        ("qualification Cargo child cleanup", cleanup_result),
                        ("rustc runtime-tree revalidation", lib_tree_result),
                        ("authorized kernel-closure revalidation", closure_result),
                    ],
                );
            }
        };
        let status = cargo_child.wait();
        let boundary_result = invocation_boundary.finish();
        drop(capability_broker);
        let lib_tree_result = self.pinned_rustc.revalidate_lib_tree();
        let closure_result = self
            .authorized_closure
            .as_ref()
            .map_or(Ok(()), |closure| closure.revalidate());
        let cargo_result = match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!(
                "qualification Cargo {command} failed with status {status}"
            )),
            Err(error) => Err(format!("failed to wait for qualification Cargo: {error}")),
        };
        aggregate_post_spawn_results(
            cargo_result,
            [
                ("Cargo invocation-boundary finish", boundary_result),
                ("rustc runtime-tree revalidation", lib_tree_result),
                ("authorized kernel-closure revalidation", closure_result),
            ],
        )?;

        self.project.validate_paths()?;
        self.target_dir.validate_path("Cargo target directory")?;
        self.generation.reject_if_substituted()?;
        self.generation.commit()
    }
}

fn append_qualification_semantic_configuration(
    configuration: &mut Vec<u8>,
    oracle: &OsStr,
) -> Result<(), String> {
    let oracle = os_bytes(oracle);
    let length = u64::try_from(oracle.len())
        .map_err(|_| "qualification oracle identity length does not fit u64".to_owned())?;
    configuration.extend_from_slice(QUALIFICATION_SEMANTIC_DOMAIN);
    configuration.extend_from_slice(&length.to_le_bytes());
    configuration.extend_from_slice(oracle);
    Ok(())
}

fn inject_qualification_application_runner(
    project: &project::CargoProject,
    pinned_cargo: &pinned_executable::PinnedExecutable,
    pinned_rustc: &PinnedRustc,
    artifact_dir: &project::PinnedDirectory,
    args: &mut Vec<OsString>,
    expects_envelope: bool,
    authority: bool,
) -> Result<(), String> {
    let (target, original_runner) =
        resolve_application_runner(project, pinned_cargo, pinned_rustc, args, authority)?;
    let executable = application_runner_executable()?;
    let (artifact_device, artifact_inode) = artifact_dir.identity_parts();
    let mut runner = vec![
        executable,
        INTERNAL_RUNNER_ARG.to_owned(),
        application_handoff::RUNNER_CONTEXT_VERSION.to_owned(),
        hex_encode(os_bytes(artifact_dir.display_path().as_os_str())),
        artifact_device.to_string(),
        artifact_inode.to_string(),
        if expects_envelope {
            application_handoff::RUNNER_EXPECTS_ENVELOPE.to_owned()
        } else {
            RUNNER_EXPECTS_NO_ENVELOPE.to_owned()
        },
        original_runner.len().to_string(),
    ];
    runner.extend(
        original_runner
            .iter()
            .map(|argument| hex_encode(os_bytes(argument))),
    );
    inject_serialized_application_runner_config(args, &target, runner)
}

/// Front-end process for the distinct qualification runner callback.
pub(super) fn run_application_frontend(args: &[OsString]) -> ExitCode {
    match application_supervisor::run_frontend_with_supervisor_arg(args, INTERNAL_SUPERVISOR_ARG) {
        Ok(status) => ExitCode::from(binding_wrapper::exit_code(status)),
        Err(error) => {
            eprintln!("cargo-fe2o3 qualification application runner: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Supervisor process for the distinct qualification runner callback.
pub(super) fn run_application_supervisor(args: &[OsString]) -> ExitCode {
    match application_supervisor::run_supervisor(
        args,
        run_application_boundary_result,
        application_handoff::application_cleanup_is_pending,
        application_handoff::finish_application_cleanup_supervisor,
    ) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("cargo-fe2o3 qualification application supervisor: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_application_boundary_result(args: &[OsString]) -> Result<ExitStatus, String> {
    if args.len() < 7 {
        return Err(
            "qualification runner requires a generation context, original-runner count, and application"
                .to_owned(),
        );
    }
    let application_timeouts = match args[0].to_str() {
        Some(application_handoff::RUNNER_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::PRODUCTION
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        Some(application_handoff::RUNNER_SHORT_TIMEOUT_TEST_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::TEST_SHORT
        }
        #[cfg(feature = "worker-v2-fault-injection-test-only")]
        Some(application_handoff::RUNNER_SCHEDULER_TOLERANT_TEST_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::TEST_SCHEDULER_TOLERANT
        }
        #[cfg(feature = "application-handoff-adversarial-fixture")]
        Some(application_handoff::RUNNER_FAST_FAILURE_TEST_CONTEXT_VERSION) => {
            application_handoff::ApplicationTimeouts::TEST_FAST_FAILURES
        }
        _ => {
            return Err(format!(
                "unsupported qualification application runner context {:?}",
                args[0]
            ));
        }
    };
    let artifact_path = PathBuf::from(hex_decode_os(&args[1])?);
    let artifact_device = parse_runner_u64(&args[2], "artifact directory device")?;
    let artifact_inode = parse_runner_u64(&args[3], "artifact directory inode")?;
    let artifact_dir = application_handoff::open_expected_generation(
        artifact_path,
        artifact_device,
        artifact_inode,
    )?;
    let expects_envelope = match args[4].to_str() {
        Some(application_handoff::RUNNER_EXPECTS_ENVELOPE) => true,
        Some(RUNNER_EXPECTS_NO_ENVELOPE) => false,
        _ => {
            return Err(format!(
                "invalid qualification application envelope expectation {:?}",
                args[4]
            ));
        }
    };
    let runner_count = args[5]
        .to_str()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid original runner argument count {:?}", args[5]))?;
    let application_index = 6_usize
        .checked_add(runner_count)
        .ok_or_else(|| "original runner argument count overflowed".to_owned())?;
    let application = args
        .get(application_index)
        .ok_or_else(|| "runner argument count does not leave an application".to_owned())?;
    let original_runner = args[6..application_index]
        .iter()
        .map(|argument| hex_decode_os(argument))
        .collect::<Result<Vec<_>, _>>()?;
    if original_runner
        .first()
        .is_some_and(|program| program.is_empty())
    {
        return Err("original Cargo runner executable may not be empty".to_owned());
    }
    reject_recursive_runner(&original_runner)?;

    let handoff = application_handoff::PinnedApplicationEnvelope::discover(&artifact_dir)?;
    match (expects_envelope, handoff) {
        (true, None) => {
            Err("qualification runner expected a canonical envelope, but none exists".to_owned())
        }
        (false, Some(_)) => Err(
            "qualification runner did not expect an envelope for this application build".to_owned(),
        ),
        (true, Some(handoff)) => {
            if !original_runner.is_empty() {
                return Err(
                    "descriptor handoff does not permit an intermediate Cargo runner".to_owned(),
                );
            }
            run_application_with_handoff(
                handoff,
                application,
                &args[application_index + 1..],
                application_timeouts,
            )
        }
        (false, None) => run_application_without_handoff(
            artifact_dir,
            &original_runner,
            application,
            &args[application_index + 1..],
        ),
    }
}

fn run_application_without_handoff(
    artifact_dir: project::PinnedDirectory,
    original_runner: &[OsString],
    application: &OsStr,
    application_args: &[OsString],
) -> Result<ExitStatus, String> {
    let mut child = if let Some(program) = original_runner.first() {
        let mut command = Command::new(program);
        command.args(&original_runner[1..]);
        command.arg(application);
        command.args(application_args);
        command
    } else {
        let mut command = Command::new(application);
        command.args(application_args);
        command
    };
    scrub_application_environment(&mut child);
    // SAFETY: the callback invokes only the descriptor-flag syscall used by the
    // production application boundary and captures no borrowed process state.
    unsafe {
        child.pre_exec(application_exec::protect_all_nonstdio_descriptors);
    }
    if original_runner.is_empty() {
        // An opaque configured runner cannot safely receive a directory capability
        // intended for a later process image. Direct execution receives only the
        // pinned generation directory at the fixed artifact descriptor.
        artifact_dir.replace_for_child_at(&mut child, ARTIFACT_CHILD_FD)?;
        child.env(HSACO_DIR_ENV, format!("/proc/self/fd/{ARTIFACT_CHILD_FD}"));
    }
    process_execution::status(&mut child)
        .map_err(|error| format!("failed to launch qualification runner/application: {error}"))
}

fn preflight_declared_qualification_authority_backend(
    expected: [u8; 32],
) -> Result<(PathBuf, pinned_codegen_backend::PinnedCodegenBackend), String> {
    if let Some(value) = env::var_os(BACKEND_ENV) {
        return Err(format!(
            "qualification authority does not admit generic {BACKEND_ENV}={value:?}; use {QUALIFICATION_BACKEND_ENV}"
        ));
    }
    let path = env::var_os(QUALIFICATION_BACKEND_ENV)
        .map(PathBuf::from)
        .ok_or_else(|| {
            format!(
                "qualification authority requires {QUALIFICATION_BACKEND_ENV} to name an explicit prebuilt qualification-enabled backend"
            )
        })?;
    if !path.is_absolute() {
        return Err(format!(
            "qualification authority requires {QUALIFICATION_BACKEND_ENV} to name an absolute prebuilt backend path"
        ));
    }
    let backend = pinned_codegen_backend::PinnedCodegenBackend::open(&path)
        .map_err(|error| format!("failed to pin qualification authority backend: {error}"))?;
    if backend.sha256() != &expected {
        return Err(format!(
            "qualification authority backend does not match {AUTHORITY_BACKEND_SHA256_ENV}"
        ));
    }
    Ok((path, backend))
}

fn find_or_build_qualification_backend(
    target_dir: &project::PinnedDirectory,
    pinned_cargo: &pinned_executable::PinnedExecutable,
    pinned_rustc: &PinnedRustc,
) -> Result<PathBuf, String> {
    if let Some(value) = env::var_os(BACKEND_ENV) {
        return Err(format!(
            "qualification harness does not admit generic {BACKEND_ENV}={value:?}; use {QUALIFICATION_BACKEND_ENV}"
        ));
    }
    if let Some(path) = env::var_os(QUALIFICATION_BACKEND_ENV).map(PathBuf::from) {
        if !path.is_absolute() {
            return Err(format!(
                "{QUALIFICATION_BACKEND_ENV} must name an absolute qualification-enabled backend"
            ));
        }
        if !path.is_file() {
            return Err(format!(
                "{QUALIFICATION_BACKEND_ENV} points to {}, but that file does not exist",
                path.display()
            ));
        }
        return Ok(path);
    }

    let source_root = fe2o3_source_root()?;
    let cargo_fe2o3_executable = env::current_exe()
        .map_err(|error| format!("failed to locate running cargo-fe2o3 executable: {error}"))?;
    let cargo_fe2o3_sha256 = fe2o3_process_identity::measure_executable_sha256_v3(
        &cargo_fe2o3_executable,
    )
    .map_err(|error| format!("failed to measure running cargo-fe2o3 executable: {error}"))?;
    let backend_target = target_dir.open_or_create_child(
        ".fe2o3-qualification-backend-build-v1",
        "isolated qualification codegen-backend build directory",
    )?;
    let backend = dylib_path(backend_target.display_path());
    eprintln!("building qualification-enabled rustc-codegen-fe2o3 backend...");
    let mut command = pinned_cargo
        .command()
        .map_err(|error| format!("failed to prepare pinned Cargo executable: {error}"))?;
    command
        .as_command_mut()
        .args(["build", "--manifest-path"])
        .arg(source_root.join("Cargo.toml"))
        .args(["--target-dir"])
        .arg(backend_target.fixed_child_path(BACKEND_BUILD_CHILD_FD)?)
        .args([
            "-p",
            "rustc-codegen-fe2o3",
            "--features",
            "qualification-oracles-test-only",
        ])
        .current_dir(&source_root)
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env_remove("CARGO_TARGET_DIR")
        .env_remove("RUSTC_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
        .env(
            "FE2O3_BUILD_CARGO_FE2O3_EXECUTABLE_SHA256_V1",
            hex_encode(&cargo_fe2o3_sha256),
        );
    remove_dynamic_loader_environment(command.as_command_mut());
    for name in [
        TARGET_ENV,
        BACKEND_ENV,
        QUALIFICATION_BACKEND_ENV,
        HSACO_DIR_ENV,
        capability_broker::CAPABILITY_BROKER_ENV,
        BINDING_WRAPPER_MODE_ENV,
        BUILD_SESSION_ENV,
        OBSOLETE_CODEGEN_PIPELINE_ENV,
        build_config::QUALIFICATION_ORACLE_ENV,
        build_config::PRODUCTION_BUILD_CONFIG_ENV,
        build_config::PRODUCTION_BUILD_EXPECTED_ID_ENV,
        build_config::WORKER_V2_CONFIG_ENV,
        build_config::WORKER_V2_EXPECTED_ID_ENV,
        AUTHORITY_CARGO_SHA256_ENV,
        AUTHORITY_RUSTC_SHA256_ENV,
        AUTHORITY_RUSTC_RUNTIME_SHA256_ENV,
        AUTHORITY_BACKEND_SHA256_ENV,
    ] {
        command.as_command_mut().env_remove(name);
    }
    configure_pinned_rustc_child(command.as_command_mut(), pinned_rustc)?;
    command.as_command_mut().env(
        "LD_LIBRARY_PATH",
        format!("/proc/self/fd/{RUSTC_LIBRARY_CHILD_FD}"),
    );
    backend_target.inherit_for_child_at(command.as_command_mut(), BACKEND_BUILD_CHILD_FD)?;
    let status = command
        .status()
        .map_err(|error| format!("failed to build qualification codegen backend: {error}"))?;
    if !status.success() {
        return Err("failed to build qualification-enabled rustc-codegen-fe2o3".to_owned());
    }
    if !backend.is_file() {
        return Err(format!(
            "qualification backend build succeeded, but {} was not produced",
            backend.display()
        ));
    }
    Ok(backend)
}

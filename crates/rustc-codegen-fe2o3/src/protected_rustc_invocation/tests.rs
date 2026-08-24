use std::ffi::{CString, OsString};
use std::fs::File;
use std::io::Write as _;
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::fs::PermissionsExt as _;
use std::sync::Mutex;

use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_rustc_invocation::{
    AmdTargetIdTextV1, BackendToolsV1, CargoIdentityV1, CargoPackageV1, CargoTargetKindV1,
    CargoTargetV1, CompileEnvironmentEntryV1, CrateTypeV1, DeviceConfigurationV1, EditionV1,
    OutputDomainV1, RustcIdentityV1, RustcInvocationDescriptorV1, RustcInvocationDescriptorV2,
    RustcInvocationDescriptorV3, RustcUnitV1, RustcUnitV2, TestStateV1, ToolIdentityV1,
    VerificationModeV1, encode_descriptor_v1, encode_descriptor_v2,
};

use super::*;

const TEST_CHILD_FD: RawFd = 711;
const RUSTC_PIN: [u8; 32] = [0x44; 32];
const BACKEND_PIN: [u8; 32] = [0x66; 32];
static FD_TEST_LOCK: Mutex<()> = Mutex::new(());

fn compiler_closure(pins: [[u8; 32]; 6]) -> CompilerClosureV2 {
    CompilerClosureV2::new(pins[0], pins[1], pins[2], pins[3], pins[4], pins[5]).unwrap()
}

fn baseline_closure() -> CompilerClosureV2 {
    compiler_closure([
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        RUSTC_PIN,
        [0x55; 32],
        BACKEND_PIN,
    ])
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn environment(
    target: &str,
    closure_observation: [u8; 32],
    backend_observation: [u8; 32],
) -> CompileEnvironmentV2 {
    CompileEnvironmentV2::from_child_environment([
        (OsString::from("FE2O3_HSACO_DIR"), OsString::from("/output")),
        (OsString::from("FE2O3_TARGET"), OsString::from(target)),
        (
            OsString::from(EXPECTED_COMPILER_CLOSURE_SHA256_ENV_V1),
            OsString::from(hex(closure_observation)),
        ),
        (
            OsString::from(CODEGEN_BACKEND_BUILD_OBSERVATION_ENV_V2),
            OsString::from(hex(backend_observation)),
        ),
        (OsString::from("LANG"), OsString::from("C.UTF-8")),
    ])
    .unwrap()
}

fn descriptor_with(
    closure: CompilerClosureV2,
    target: &str,
    closure_observation: [u8; 32],
    backend_observation: [u8; 32],
) -> RustcInvocationDescriptorV3 {
    let rustc = RustcUnitV2::new(
        "/workspace",
        vec![
            "/toolchain/bin/rustc".into(),
            "--crate-name".into(),
            "protected_fixture".into(),
            "-Zcodegen-backend=/proc/./self/fd/198".into(),
        ],
    )
    .unwrap();
    let v2 = RustcInvocationDescriptorV2::new(
        closure.rustc_executable_sha256(),
        closure.codegen_backend_sha256(),
        rustc,
        environment(target, closure_observation, backend_observation),
    )
    .unwrap();
    RustcInvocationDescriptorV3::new(v2, closure).unwrap()
}

fn baseline_descriptor() -> RustcInvocationDescriptorV3 {
    let closure = baseline_closure();
    descriptor_with(
        closure,
        EXACT_PROTECTED_TARGET_V1,
        closure.identity_sha256(),
        closure.codegen_backend_sha256(),
    )
}

fn observation(descriptor: &RustcInvocationDescriptorV3) -> RustcProcessObservationV1 {
    RustcProcessObservationV1 {
        argv: descriptor.rustc().argv().map(str::to_owned).collect(),
        canonical_working_directory: descriptor.rustc().working_directory().to_owned(),
        compile_environment: descriptor.compile_environment().clone(),
        running_rustc_sha256: RUSTC_PIN,
        running_codegen_backend_sha256: BACKEND_PIN,
    }
}

fn validate(
    descriptor: RustcInvocationDescriptorV3,
    observation: RustcProcessObservationV1,
) -> Result<AdmittedProtectedRustcInvocationV1, ProtectedRustcInvocationErrorV1> {
    validate_capability(
        RustcInvocationCapabilityV1::create(descriptor).unwrap(),
        observation,
    )
}

#[test]
fn absent_and_present_selection_is_exact_and_ordinary_compatible() {
    for pipeline in CodegenPipeline::ALL {
        assert_eq!(
            pipeline.rustc_invocation_policy(true),
            if pipeline == CodegenPipeline::ProductionV1 {
                RustcInvocationPolicyV1::ProtectedV3
            } else {
                RustcInvocationPolicyV1::QualificationObserved
            }
        );
        assert_eq!(
            pipeline.rustc_invocation_policy(false),
            if matches!(
                pipeline,
                CodegenPipeline::ProductionV1 | CodegenPipeline::CollectedRowSoftmaxV1
            ) {
                RustcInvocationPolicyV1::ProtectedV3
            } else {
                RustcInvocationPolicyV1::Unmanaged
            }
        );
    }

    assert!(
        admit_for_codegen(CodegenPipeline::LegacyV1)
            .unwrap()
            .is_none()
    );
}

#[test]
fn unprotected_routes_reject_protected_signals_without_consuming_unknown_fds() {
    let _guard = FD_TEST_LOCK.lock().unwrap();
    let descriptor_bytes =
        fe2o3_rustc_invocation::encode_descriptor_v3(&baseline_descriptor()).unwrap();

    for pipeline in CodegenPipeline::ALL {
        if pipeline.rustc_invocation_policy(false) != RustcInvocationPolicyV1::Unmanaged {
            continue;
        }
        let descriptor = sealed_image(&descriptor_bytes);
        install_inherited(&descriptor, TEST_CHILD_FD);
        assert!(matches!(
            admit_for_codegen_at(pipeline, false, TEST_CHILD_FD, false, false, false, false),
            Err(
                ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                    descriptor_present: true,
                    compiler_closure_marker_present: false,
                    backend_marker_present: false,
                    qualification_backend_marker_present: false,
                }
            )
        ));
        assert_ne!(unsafe { libc::fcntl(TEST_CHILD_FD, libc::F_GETFD) }, -1);
        assert_eq!(unsafe { libc::close(TEST_CHILD_FD) }, 0);
    }

    for (
        compiler_closure_marker_present,
        backend_marker_present,
        qualification_backend_marker_present,
    ) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
        (true, true, true),
    ] {
        assert!(matches!(
            admit_for_codegen_at(
                CodegenPipeline::LegacyV1,
                false,
                TEST_CHILD_FD,
                compiler_closure_marker_present,
                backend_marker_present,
                qualification_backend_marker_present,
                false,
            ),
            Err(ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                descriptor_present: false,
                compiler_closure_marker_present: observed_closure_marker,
                backend_marker_present: observed_backend_marker,
                qualification_backend_marker_present: observed_qualification_marker,
            }) if observed_closure_marker == compiler_closure_marker_present
                && observed_backend_marker == backend_marker_present
                && observed_qualification_marker == qualification_backend_marker_present
        ));
    }

    assert!(
        admit_for_codegen_at(
            CodegenPipeline::LegacyV1,
            false,
            TEST_CHILD_FD,
            false,
            false,
            false,
            false,
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn qualification_routes_require_paired_authenticated_observations_and_no_authority() {
    let _guard = FD_TEST_LOCK.lock().unwrap();
    let descriptor_bytes =
        fe2o3_rustc_invocation::encode_descriptor_v3(&baseline_descriptor()).unwrap();

    for pipeline in CodegenPipeline::ALL {
        if pipeline.rustc_invocation_policy(true) != RustcInvocationPolicyV1::QualificationObserved
        {
            continue;
        }
        assert!(
            admit_for_codegen_at(pipeline, true, TEST_CHILD_FD, true, false, true, true,)
                .unwrap()
                .is_none()
        );
    }

    for (compiler_closure_marker_present, qualification_backend_marker_present) in
        [(false, false), (true, false), (false, true)]
    {
        assert!(matches!(
            admit_for_codegen_at(
                CodegenPipeline::LegacyV1,
                true,
                TEST_CHILD_FD,
                compiler_closure_marker_present,
                false,
                qualification_backend_marker_present,
                false,
            ),
            Err(ProtectedRustcInvocationErrorV1::QualificationObservationsMissing)
        ));
    }
    assert!(matches!(
        admit_for_codegen_at(
            CodegenPipeline::LegacyV1,
            true,
            TEST_CHILD_FD,
            true,
            false,
            true,
            false,
        ),
        Err(ProtectedRustcInvocationErrorV1::QualificationCodegenBackendObservationMismatch)
    ));
    assert!(matches!(
        admit_for_codegen_at(
            CodegenPipeline::LegacyV1,
            true,
            TEST_CHILD_FD,
            true,
            true,
            true,
            true,
        ),
        Err(
            ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                descriptor_present: false,
                compiler_closure_marker_present: false,
                backend_marker_present: true,
                qualification_backend_marker_present: false,
            }
        )
    ));

    let descriptor = sealed_image(&descriptor_bytes);
    install_inherited(&descriptor, TEST_CHILD_FD);
    assert!(matches!(
        admit_for_codegen_at(
            CodegenPipeline::LegacyV1,
            true,
            TEST_CHILD_FD,
            true,
            false,
            true,
            true,
        ),
        Err(
            ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                descriptor_present: true,
                compiler_closure_marker_present: false,
                backend_marker_present: false,
                qualification_backend_marker_present: false,
            }
        )
    ));
    assert_ne!(unsafe { libc::fcntl(TEST_CHILD_FD, libc::F_GETFD) }, -1);
    assert_eq!(unsafe { libc::close(TEST_CHILD_FD) }, 0);

    assert!(matches!(
        admit_for_codegen_at(
            CodegenPipeline::ProductionV1,
            false,
            TEST_CHILD_FD,
            true,
            true,
            true,
            false,
        ),
        Err(
            ProtectedRustcInvocationErrorV1::UnexpectedProtectedSignals {
                descriptor_present: false,
                compiler_closure_marker_present: false,
                backend_marker_present: false,
                qualification_backend_marker_present: true,
            }
        )
    ));
}

#[test]
fn zero_kernel_protected_selection_cannot_downgrade_or_leave_an_inherited_fd() {
    let _guard = FD_TEST_LOCK.lock().unwrap();
    assert_eq!(
        CodegenPipeline::ProductionV1.rustc_invocation_policy(false),
        RustcInvocationPolicyV1::ProtectedV3,
    );
    assert_eq!(
        CodegenPipeline::CollectedRowSoftmaxV1.rustc_invocation_policy(false),
        RustcInvocationPolicyV1::ProtectedV3,
    );

    let valid = RustcInvocationCapabilityV1::create(baseline_descriptor())
        .unwrap()
        .try_clone_for_transfer()
        .unwrap();
    install_inherited(&valid, TEST_CHILD_FD);
    drop(retain_inherited_capability_at(TEST_CHILD_FD).unwrap());
    assert_eq!(unsafe { libc::fcntl(TEST_CHILD_FD, libc::F_GETFD) }, -1);

    let malformed = sealed_image(b"not-a-rustc-invocation-descriptor");
    install_inherited(&malformed, TEST_CHILD_FD);
    assert!(matches!(
        retain_inherited_capability_at(TEST_CHILD_FD),
        Err(ProtectedRustcInvocationErrorV1::Capability(_))
    ));
    assert_eq!(unsafe { libc::fcntl(TEST_CHILD_FD, libc::F_GETFD) }, -1);
}

#[test]
fn present_v3_is_retained_once_and_exposes_only_the_full_closure() {
    let _guard = FD_TEST_LOCK.lock().unwrap();
    let expected = baseline_descriptor();
    let source = RustcInvocationCapabilityV1::create(expected.clone())
        .unwrap()
        .try_clone_for_transfer()
        .unwrap();
    install_inherited(&source, TEST_CHILD_FD);

    let retained = retain_inherited_capability_at(TEST_CHILD_FD).unwrap();
    assert_eq!(unsafe { libc::fcntl(TEST_CHILD_FD, libc::F_GETFD) }, -1);
    assert!(matches!(
        retain_inherited_capability_at(TEST_CHILD_FD),
        Err(ProtectedRustcInvocationErrorV1::Capability(_))
    ));
    let admitted = validate_capability(retained, observation(&expected)).unwrap();
    assert_eq!(admitted.compiler_closure().unwrap(), baseline_closure());
}

#[test]
fn final_publication_transition_is_move_only_and_retains_exact_v3() {
    let expected = baseline_descriptor();
    let admitted = validate(expected.clone(), observation(&expected)).unwrap();
    let finished = admitted
        .finish_for_publication_with_observation(observation(&expected))
        .unwrap();

    assert_eq!(finished.descriptor(), &expected);
    assert_eq!(
        finished.descriptor().compiler_closure(),
        expected.compiler_closure()
    );
    finished.revalidate().unwrap();
}

#[test]
fn final_publication_transition_rejects_changed_process_observations() {
    let expected = baseline_descriptor();

    let mut changed = observation(&expected);
    changed.argv[0].push_str("-changed");
    assert!(matches!(
        validate(expected.clone(), observation(&expected))
            .unwrap()
            .finish_for_publication_with_observation(changed),
        Err(ProtectedRustcInvocationErrorV1::ArgumentsMismatch)
    ));

    let mut changed = observation(&expected);
    changed.canonical_working_directory = "/changed".into();
    assert!(matches!(
        validate(expected.clone(), observation(&expected))
            .unwrap()
            .finish_for_publication_with_observation(changed),
        Err(ProtectedRustcInvocationErrorV1::WorkingDirectoryMismatch)
    ));

    let mut changed = observation(&expected);
    changed.running_rustc_sha256 = [0xa1; 32];
    assert!(matches!(
        validate(expected.clone(), observation(&expected))
            .unwrap()
            .finish_for_publication_with_observation(changed),
        Err(ProtectedRustcInvocationErrorV1::RunningRustcMismatch)
    ));

    let mut changed = observation(&expected);
    changed.running_codegen_backend_sha256 = [0xa2; 32];
    assert!(matches!(
        validate(expected.clone(), observation(&expected))
            .unwrap()
            .finish_for_publication_with_observation(changed),
        Err(ProtectedRustcInvocationErrorV1::RunningCodegenBackendMismatch)
    ));

    let mut changed = observation(&expected);
    let mut entries = changed
        .compile_environment
        .entries()
        .iter()
        .map(|entry| (OsString::from(entry.key()), OsString::from(entry.value())))
        .collect::<Vec<_>>();
    entries.push((OsString::from("CHANGED"), OsString::from("1")));
    changed.compile_environment = CompileEnvironmentV2::from_child_environment(entries).unwrap();
    assert!(matches!(
        validate(expected.clone(), observation(&expected))
            .unwrap()
            .finish_for_publication_with_observation(changed),
        Err(ProtectedRustcInvocationErrorV1::CompileEnvironmentMismatch)
    ));
}

#[test]
fn argv_cwd_environment_and_target_mismatches_fail_closed() {
    let descriptor = baseline_descriptor();

    let mut changed = observation(&descriptor);
    changed.argv[0].push_str("-other");
    assert!(matches!(
        validate(descriptor.clone(), changed),
        Err(ProtectedRustcInvocationErrorV1::ArgumentsMismatch)
    ));

    let mut changed = observation(&descriptor);
    changed.canonical_working_directory = "/other".into();
    assert!(matches!(
        validate(descriptor.clone(), changed),
        Err(ProtectedRustcInvocationErrorV1::WorkingDirectoryMismatch)
    ));

    let mut changed = observation(&descriptor);
    let mut entries = changed
        .compile_environment
        .entries()
        .iter()
        .map(|entry| (OsString::from(entry.key()), OsString::from(entry.value())))
        .collect::<Vec<_>>();
    entries.push((OsString::from("UNEXPECTED"), OsString::from("1")));
    changed.compile_environment = CompileEnvironmentV2::from_child_environment(entries).unwrap();
    assert!(matches!(
        validate(descriptor.clone(), changed),
        Err(ProtectedRustcInvocationErrorV1::CompileEnvironmentMismatch)
    ));

    let closure = baseline_closure();
    let wrong_target = descriptor_with(
        closure,
        "gfx942:sramecc+:xnack-",
        closure.identity_sha256(),
        BACKEND_PIN,
    );
    assert!(matches!(
        validate(wrong_target.clone(), observation(&wrong_target)),
        Err(ProtectedRustcInvocationErrorV1::TargetMismatch { .. })
    ));
}

#[test]
fn measured_rustc_and_backend_pins_are_authoritative() {
    let descriptor = baseline_descriptor();
    let mut changed = observation(&descriptor);
    changed.running_rustc_sha256 = [0xa1; 32];
    assert!(matches!(
        validate(descriptor.clone(), changed),
        Err(ProtectedRustcInvocationErrorV1::RunningRustcMismatch)
    ));

    let mut changed = observation(&descriptor);
    changed.running_codegen_backend_sha256 = [0xa2; 32];
    assert!(matches!(
        validate(descriptor, changed),
        Err(ProtectedRustcInvocationErrorV1::RunningCodegenBackendMismatch)
    ));
}

#[test]
fn aggregate_and_backend_environment_values_are_closed_observations_only() {
    let closure = baseline_closure();
    let aggregate_mismatch =
        descriptor_with(closure, EXACT_PROTECTED_TARGET_V1, [0xa3; 32], BACKEND_PIN);
    assert!(matches!(
        validate(aggregate_mismatch.clone(), observation(&aggregate_mismatch)),
        Err(ProtectedRustcInvocationErrorV1::CompilerClosureObservationMismatch)
    ));

    let backend_mismatch = descriptor_with(
        closure,
        EXACT_PROTECTED_TARGET_V1,
        closure.identity_sha256(),
        [0xa4; 32],
    );
    assert!(matches!(
        validate(backend_mismatch.clone(), observation(&backend_mismatch)),
        Err(ProtectedRustcInvocationErrorV1::CodegenBackendObservationMismatch)
    ));
}

#[test]
fn every_full_compiler_closure_role_is_checked() {
    let baseline = [
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        RUSTC_PIN,
        [0x55; 32],
        BACKEND_PIN,
    ];
    for role in 0..baseline.len() {
        let mut pins = baseline;
        pins[role][0] ^= 1;
        let changed = compiler_closure(pins);
        let descriptor = descriptor_with(
            changed,
            EXACT_PROTECTED_TARGET_V1,
            baseline_closure().identity_sha256(),
            BACKEND_PIN,
        );
        assert!(
            validate(descriptor.clone(), observation(&descriptor)).is_err(),
            "closure role {role} was not checked"
        );
    }
}

#[test]
fn valid_v1_v2_and_malformed_images_cannot_downgrade_v3() {
    let _guard = FD_TEST_LOCK.lock().unwrap();
    let images = [
        encode_descriptor_v1(&v1_descriptor()).unwrap(),
        encode_descriptor_v2(baseline_descriptor().descriptor_v2()).unwrap(),
        b"not-a-rustc-invocation-descriptor".to_vec(),
    ];
    for bytes in images {
        let source = sealed_image(&bytes);
        install_inherited(&source, TEST_CHILD_FD);
        let result = retain_inherited_capability_at(TEST_CHILD_FD);
        assert!(matches!(
            result,
            Err(ProtectedRustcInvocationErrorV1::Capability(_))
        ));
        assert_eq!(unsafe { libc::fcntl(TEST_CHILD_FD, libc::F_GETFD) }, -1);
    }
}

fn install_inherited(source: &File, child_fd: RawFd) {
    // SAFETY: dup2 atomically installs a borrowed valid source at the test-owned descriptor.
    assert_eq!(
        unsafe { libc::dup2(source.as_raw_fd(), child_fd) },
        child_fd
    );
    assert_eq!(unsafe { libc::fcntl(child_fd, libc::F_GETFD) }, 0);
}

fn sealed_image(bytes: &[u8]) -> File {
    let name = CString::new("fe2o3-rustc-invocation-test").unwrap();
    // SAFETY: the constant flags and terminated name are valid memfd_create inputs.
    let raw = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            name.as_ptr(),
            libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING,
        ) as RawFd
    };
    assert!(
        raw >= 0,
        "memfd_create failed: {}",
        std::io::Error::last_os_error()
    );
    // SAFETY: successful memfd_create returned a new owned descriptor.
    let mut file = unsafe { File::from_raw_fd(raw) };
    file.write_all(bytes).unwrap();
    file.flush().unwrap();
    file.set_permissions(std::fs::Permissions::from_mode(0o400))
        .unwrap();
    let seals = libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
    assert_eq!(
        unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, seals) },
        0
    );
    file
}

fn tool(version: &str, byte: u8) -> ToolIdentityV1 {
    ToolIdentityV1::new(version, [byte; 32]).unwrap()
}

fn v1_descriptor() -> RustcInvocationDescriptorV1 {
    let cargo = CargoIdentityV1::new(
        tool("cargo 1.96.0-nightly", 0x11),
        CargoPackageV1::new("fixture", "0.1.0", "Cargo.toml").unwrap(),
        CargoTargetV1::new(
            "fixture",
            CargoTargetKindV1::Library,
            vec![CrateTypeV1::Lib],
            EditionV1::Rust2024,
            "src/lib.rs",
            Vec::new(),
        )
        .unwrap(),
    );
    let rustc = RustcIdentityV1::new(
        tool("rustc 1.96.0-nightly", 0x22),
        RustcUnitV1::new(
            "fixture",
            "x86_64-unknown-linux-gnu",
            "amdgcn-amd-amdhsa",
            TestStateV1::NotTest,
            vec![
                "--crate-name".into(),
                "fixture".into(),
                "src/lib.rs".into(),
                "-Zcodegen-backend=/backend.so".into(),
            ],
        )
        .unwrap(),
    );
    let tools = BackendToolsV1::new(
        tool("backend", 0x33),
        tool("clang", 0x44),
        tool("lld", 0x55),
        None,
    );
    RustcInvocationDescriptorV1::new(
        cargo,
        rustc,
        tools,
        DeviceConfigurationV1::new(
            AmdTargetIdTextV1::new(EXACT_PROTECTED_TARGET_V1).unwrap(),
            VerificationModeV1::Required,
        ),
        OutputDomainV1::new("/workspace", "/output").unwrap(),
        vec![CompileEnvironmentEntryV1::new("FE2O3_TARGET", EXACT_PROTECTED_TARGET_V1).unwrap()],
    )
    .unwrap()
}

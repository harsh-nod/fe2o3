use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::fd::{AsRawFd as _, FromRawFd as _};
use std::os::unix::fs::{
    DirBuilderExt as _, FileExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest as _, Sha256};

const PIPELINE: &str = "collected-executable-scalar-control-flow-v2";
const FIXTURE: &str = include_str!("fixtures/executable-scalar-control-flow-v1.rs");
static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(0);
static BACKEND: OnceLock<PinnedBackend> = OnceLock::new();

const REQUIRED_MEMFD_SEALS: libc::c_int =
    libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;

struct PinnedBackend {
    file: File,
    len: usize,
    sha256: [u8; 32],
}

impl PinnedBackend {
    fn load_path(&self) -> PathBuf {
        PathBuf::from(format!("/proc/self/fd/./{}", self.file.as_raw_fd()))
    }

    fn verify(&self) -> Result<(), String> {
        let descriptor_flags = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GETFD) };
        if descriptor_flags < 0 || descriptor_flags & libc::FD_CLOEXEC != 0 {
            return Err(format!(
                "backend descriptor is not inheritable by rustc: {descriptor_flags:#x}"
            ));
        }
        let seals = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_GET_SEALS) };
        if seals < 0 || seals & REQUIRED_MEMFD_SEALS != REQUIRED_MEMFD_SEALS {
            return Err(format!("backend memfd lost required seals: {seals:#x}"));
        }
        let actual_len = usize::try_from(
            self.file
                .metadata()
                .map_err(|error| error.to_string())?
                .len(),
        )
        .map_err(|_| "backend length does not fit usize".to_owned())?;
        if actual_len != self.len {
            return Err(format!(
                "sealed backend length changed: expected {}, found {actual_len}",
                self.len
            ));
        }
        let mut bytes = vec![0_u8; self.len];
        self.file
            .read_exact_at(&mut bytes, 0)
            .map_err(|error| format!("read sealed backend: {error}"))?;
        let actual: [u8; 32] = Sha256::digest(bytes).into();
        if actual != self.sha256 {
            return Err("sealed backend SHA-256 changed".to_owned());
        }
        Ok(())
    }
}

struct PrivateBuildRoot(PathBuf);

impl PrivateBuildRoot {
    fn new(workspace: &Path) -> Self {
        let parent = workspace.join("target/rustc-codegen-fe2o3-private-builds");
        std::fs::create_dir_all(&parent).expect("create private-build parent");
        for attempt in 0..64_u64 {
            let path = parent.join(format!(
                "scalar-cf-{}-{}-{attempt}",
                std::process::id(),
                NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = std::fs::DirBuilder::new();
            builder.mode(0o700);
            match builder.create(&path) {
                Ok(()) => {
                    let mode = std::fs::metadata(&path)
                        .expect("stat private build root")
                        .permissions()
                        .mode()
                        & 0o777;
                    assert_eq!(mode, 0o700, "private build root must be owner-only");
                    return Self(path);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("create private build root: {error}"),
            }
        }
        panic!("could not allocate a private backend build root")
    }
}

impl Drop for PrivateBuildRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct TestOutputDir(PathBuf);

impl TestOutputDir {
    fn new(workspace: &Path) -> Self {
        let path = workspace.join(format!(
            "target/rustc-codegen-fe2o3-test-output/collected-scalar-cf-{}-{}",
            std::process::id(),
            NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed)
        ));
        if path.exists() {
            std::fs::remove_dir_all(&path).expect("remove stale scalar-control-flow output");
        }
        let mut builder = std::fs::DirBuilder::new();
        builder.mode(0o700);
        builder
            .create(&path)
            .expect("create owner-only scalar-control-flow output");
        std::fs::create_dir(path.join("artifacts"))
            .expect("create scalar-control-flow artifact output");
        Self(path)
    }
}

impl Drop for TestOutputDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical workspace")
}

fn build_backend(workspace: &Path) -> &'static PinnedBackend {
    BACKEND.get_or_init(|| {
        let build_root = PrivateBuildRoot::new(workspace);
        let target_dir = build_root.0.join("target");
        let output = Command::new(env!("CARGO"))
            .current_dir(workspace)
            .args(["build", "--locked", "-p", "rustc-codegen-fe2o3"])
            .env("CARGO_TARGET_DIR", &target_dir)
            .output()
            .expect("build rustc backend");
        assert!(
            output.status.success(),
            "backend build failed\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        pin_backend(&target_dir.join("debug/librustc_codegen_fe2o3.so"))
            .expect("pin exact backend in a sealed memfd")
    })
}

fn pin_backend(path: &Path) -> Result<PinnedBackend, String> {
    let mut source = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|error| format!("open built backend without following symlinks: {error}"))?;
    if !source
        .metadata()
        .map_err(|error| format!("stat built backend: {error}"))?
        .file_type()
        .is_file()
    {
        return Err("built backend is not a regular file".to_owned());
    }
    let mut bytes = Vec::new();
    source
        .read_to_end(&mut bytes)
        .map_err(|error| format!("read built backend: {error}"))?;
    if bytes.is_empty() {
        return Err("built backend is empty".to_owned());
    }
    let sha256 = Sha256::digest(&bytes).into();
    let name = CString::new("fe2o3-scalar-cf-backend").expect("static memfd name");
    let raw_fd = unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_ALLOW_SEALING) };
    if raw_fd < 0 {
        return Err(format!(
            "create backend memfd: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut file = unsafe { File::from_raw_fd(raw_fd) };
    file.write_all(&bytes)
        .map_err(|error| format!("populate backend memfd: {error}"))?;
    if unsafe { libc::fcntl(raw_fd, libc::F_ADD_SEALS, REQUIRED_MEMFD_SEALS) } != 0 {
        return Err(format!(
            "seal backend memfd: {}",
            std::io::Error::last_os_error()
        ));
    }
    let pinned = PinnedBackend {
        file,
        len: bytes.len(),
        sha256,
    };
    pinned.verify()?;
    Ok(pinned)
}

fn compile(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source: &str,
    target: &str,
    pipeline: &str,
) -> Output {
    let source_path = output.0.join("fixture.rs");
    std::fs::write(&source_path, source).expect("write scalar-control-flow fixture");
    compile_path(
        workspace,
        backend,
        output,
        &source_path,
        target,
        pipeline,
        &[],
    )
}

fn compile_path(
    workspace: &Path,
    backend: &PinnedBackend,
    output: &TestOutputDir,
    source_path: &Path,
    target: &str,
    pipeline: &str,
    extra_args: &[&str],
) -> Output {
    backend
        .verify()
        .expect("sealed backend identity before rustc");
    let canonical_source = if source_path.is_absolute() {
        source_path.to_path_buf()
    } else {
        workspace.join(source_path)
    }
    .canonicalize()
    .expect("canonical scalar-control-flow fixture path");
    let mut command = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
    command
        .current_dir(workspace)
        .arg(&canonical_source)
        .arg(format!(
            "--remap-path-prefix={}=/fe2o3-reviewed-workspace/scalar-control-flow-v1.rs",
            canonical_source.display()
        ))
        .args([
            "--edition=2024",
            "--crate-name",
            "fe2o3_scalar_control_flow_v1_fixture",
            "-C",
            "overflow-checks=off",
            "-Zmir-enable-passes=-JumpThreading",
        ])
        .args(extra_args)
        .arg(format!(
            "-Zcodegen-backend={}",
            backend.load_path().display()
        ))
        .arg("-o")
        .arg(output.0.join("fixture"))
        .env("FE2O3_VERBOSE", "1")
        .env("FE2O3_DUMP_LLVM", "1")
        .env("FE2O3_TARGET", target)
        .env("FE2O3_CODEGEN_PIPELINE", pipeline)
        .env("FE2O3_HSACO_DIR", output.0.join("artifacts"));
    command
        .output()
        .expect("compile scalar-control-flow fixture")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn hex_after(text: &str, marker: &str) -> String {
    text.split_once(marker)
        .unwrap_or_else(|| panic!("missing digest marker {marker:?}\n{text}"))
        .1
        .chars()
        .take_while(|character| character.is_ascii_hexdigit())
        .take(64)
        .collect()
}

fn assert_rejected_without_fallback(output: &Output, expected: &str) {
    let stderr = stderr(output);
    assert!(!output.status.success(), "unexpected success\n{stderr}");
    assert!(
        stderr.contains(expected),
        "missing `{expected}` diagnostic\n{stderr}"
    );
    assert!(
        !stderr.contains("unsupported kernel shape for AMDGPU LLVM IR MVP")
            && !stderr.contains("selected legacy-v1")
            && !stderr.contains("emitted scalar_control_flow_v1"),
        "rejection entered a legacy/artifact fallback\n{stderr}"
    );
}

#[test]
fn authenticated_fixture_seals_semantics_then_stops_before_executable_authority() {
    let workspace = workspace();
    let backend = build_backend(&workspace);
    let output = TestOutputDir::new(&workspace);
    let fixture =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/executable-scalar-control-flow-v1.rs");
    let compiled = compile_path(
        &workspace,
        backend,
        &output,
        fixture,
        "gfx942:xnack-",
        PIPELINE,
        &[],
    );
    let baseline_stderr = stderr(&compiled);
    assert!(
        !compiled.status.success(),
        "unexpected success\n{baseline_stderr}"
    );
    assert!(
        baseline_stderr.contains("[kernel] scalar_control_flow_v1"),
        "{baseline_stderr}"
    );
    assert!(
        baseline_stderr.contains("[internal-helper]"),
        "{baseline_stderr}"
    );
    assert!(
        baseline_stderr.contains(&format!("{PIPELINE} authenticated collected KernelEntry")),
        "missing authenticated export diagnostic\n{baseline_stderr}"
    );
    assert!(baseline_stderr.contains("exact reachable InternalHelper"));
    assert!(baseline_stderr.contains("path-independent portable MIR semantics"));
    assert!(baseline_stderr.contains("compiler semantics"));
    assert!(baseline_stderr.contains("sealed collected authority"));
    assert!(baseline_stderr.contains("executable-MIR capture/import"));
    assert!(baseline_stderr.contains(
        "no executable authority, Kernel IR, LLVM, LLD, HSACO, or legacy fallback was entered"
    ));
    assert!(!baseline_stderr.contains("define amdgpu_kernel"));
    assert_eq!(
        std::fs::read_dir(output.0.join("artifacts"))
            .expect("read empty artifact directory")
            .count(),
        0,
        "admission-only slice must not claim an artifact"
    );

    let repeated_output = TestOutputDir::new(&workspace);
    let repeated = compile_path(
        &workspace,
        backend,
        &repeated_output,
        fixture,
        "gfx942:xnack-",
        PIPELINE,
        &[],
    );
    let repeated_stderr = stderr(&repeated);
    assert!(
        !repeated.status.success(),
        "unexpected success\n{repeated_stderr}"
    );
    assert_eq!(
        hex_after(&baseline_stderr, "path-independent portable MIR semantics "),
        hex_after(&repeated_stderr, "path-independent portable MIR semantics ")
    );
    assert_eq!(
        hex_after(&baseline_stderr, "sealed collected authority "),
        hex_after(&repeated_stderr, "sealed collected authority ")
    );
    assert_eq!(
        hex_after(&baseline_stderr, "exact reviewed root MIR "),
        hex_after(&repeated_stderr, "exact reviewed root MIR "),
        "canonical source remapping must make full rustc MIR identity path independent"
    );
}

#[test]
fn target_pipeline_identity_abi_and_collection_substitutions_reject_without_fallback() {
    let workspace = workspace();
    let backend = build_backend(&workspace);

    let wrong_target = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_target,
            FIXTURE,
            "gfx942:xnack+",
            PIPELINE,
        ),
        "requires exact target `gfx942:xnack-`, found `gfx942:xnack+`",
    );

    let custom_pipeline = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &custom_pipeline,
            FIXTURE,
            "gfx942:xnack-",
            "collected-executable-scalar-control-flow-v2-custom",
        ),
        "FE2O3_CODEGEN_PIPELINE must be unset or exactly",
    );

    let custom_llvm = TestOutputDir::new(&workspace);
    let fixture =
        Path::new("crates/rustc-codegen-fe2o3/tests/fixtures/executable-scalar-control-flow-v1.rs");
    assert_rejected_without_fallback(
        &compile_path(
            &workspace,
            backend,
            &custom_llvm,
            fixture,
            "gfx942:xnack-",
            PIPELINE,
            &["-Cpasses=default<O1>"],
        ),
        "rejects custom LLVM pipeline selection",
    );

    for (extra_arg, expected) in [
        (
            "-Cpanic=abort",
            "compiler semantics mismatch: panic strategy must be Unwind, found Abort",
        ),
        (
            "-Copt-level=1",
            "compiler semantics mismatch: rustc optimization must be No/0",
        ),
        (
            "-Zmir-opt-level=2",
            "compiler semantics mismatch: effective MIR optimization level must be 1",
        ),
        (
            "-Ctarget-cpu=native",
            "compiler semantics mismatch: rustc target CPU/features must be unset",
        ),
        (
            "-Coverflow-checks=on",
            "unsupported executable MIR edge `assert(",
        ),
        (
            "-Cdebug-assertions=no",
            "compiler semantics mismatch: debug assertions must be enabled",
        ),
        (
            "--remap-path-prefix=/tmp=/attacker",
            "compiler semantics mismatch: source remapping must contain exactly one canonical fixture destination",
        ),
    ] {
        let output = TestOutputDir::new(&workspace);
        assert_rejected_without_fallback(
            &compile_path(
                &workspace,
                backend,
                &output,
                fixture,
                "gfx942:xnack-",
                PIPELINE,
                &[extra_arg],
            ),
            expected,
        );
    }

    let wrong_abi_source = FIXTURE
        .replace(
            "pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u32)",
            "pub fn fe2o3_kernel_scalar_control_flow_v1(limit: u64)",
        )
        .replace(
            "nested_match_helper(limit);",
            "nested_match_helper(limit as u32);",
        )
        .replace("    fn(u32),", "    fn(u64),");
    let wrong_abi = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_abi,
            &wrong_abi_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "root ABI mismatch",
    );

    let wrong_helper_source = FIXTURE.replace("_ => sum += inner,", "_ => sum += inner + 1,");
    let wrong_helper = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_helper,
            &wrong_helper_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "helper MIR identity mismatch",
    );

    let wrong_helper_type_source = FIXTURE
        .replace(
            "fn nested_match_helper(limit: u32) -> u32",
            "fn nested_match_helper(limit: u64) -> u64",
        )
        .replace("0_u32", "0_u64")
        .replace(
            "nested_match_helper(limit);",
            "nested_match_helper(limit as u64);",
        );
    let wrong_helper_type = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &wrong_helper_type,
            &wrong_helper_type_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "helper ABI mismatch",
    );

    for changed_helper in [
        FIXTURE.replace("_ => sum += inner,", "_ => sum *= inner,"),
        FIXTURE.replace(
            "_ => sum += inner,",
            "_ => { if inner == 7 { sum += 1; } sum += inner },",
        ),
    ] {
        let output = TestOutputDir::new(&workspace);
        assert_rejected_without_fallback(
            &compile(
                &workspace,
                backend,
                &output,
                &changed_helper,
                "gfx942:xnack-",
                PIPELINE,
            ),
            "helper MIR identity mismatch",
        );
    }

    let additional_root_source = FIXTURE.replace(
        "fn main() {}",
        r#"
#[unsafe(no_mangle)]
pub fn fe2o3_kernel_scalar_control_flow_extra(_: u32) {}

#[used]
#[allow(non_upper_case_globals)]
static __fe2o3_kernel_registration_scalar_control_flow_extra: (
    u64, u16, u16, &'static str, &'static str, fn(u32),
) = (
    0x4e52_4b33_4f32_4546, 1, 1,
    "scalar_control_flow_extra", "scalar_control_flow_extra",
    fe2o3_kernel_scalar_control_flow_extra,
);

fn main() {}
"#,
    );
    let additional_root = TestOutputDir::new(&workspace);
    assert_rejected_without_fallback(
        &compile(
            &workspace,
            backend,
            &additional_root,
            &additional_root_source,
            "gfx942:xnack-",
            PIPELINE,
        ),
        "requires exactly two collected functions, found 3",
    );
}

#[test]
fn pinned_backend_descriptor_survives_same_uid_path_replacement() {
    let workspace = workspace();
    let output = TestOutputDir::new(&workspace);
    assert_eq!(
        std::fs::metadata(&output.0).unwrap().permissions().mode() & 0o777,
        0o700
    );

    let replaceable = output.0.join("replaceable-backend.so");
    let original = b"original backend bytes";
    std::fs::write(&replaceable, original).unwrap();
    let pinned = pin_backend(&replaceable).unwrap();
    std::fs::write(&replaceable, b"same-uid substituted path contents").unwrap();

    pinned.verify().unwrap();
    let expected_sha256: [u8; 32] = Sha256::digest(original).into();
    assert_eq!(pinned.sha256, expected_sha256);
    assert_eq!(
        pinned.load_path(),
        PathBuf::from(format!("/proc/self/fd/./{}", pinned.file.as_raw_fd()))
    );
    let replacement = [0_u8];
    let written = unsafe {
        libc::pwrite(
            pinned.file.as_raw_fd(),
            replacement.as_ptr().cast(),
            replacement.len(),
            0,
        )
    };
    assert_eq!(written, -1, "F_SEAL_WRITE must reject descriptor writes");
    pinned.verify().unwrap();
}

use std::ffi::{CString, OsStr, OsString};
use std::fs::File;
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestWorkspace(PathBuf);

impl TestWorkspace {
    fn new() -> Self {
        loop {
            let suffix = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "fe2o3-wrapper-managed-cli-{}-{suffix}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create test workspace: {error}"),
            }
        }
    }

    fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        command
            .env("CARGO_TARGET_DIR", self.0.join("target"))
            .current_dir(&self.0);
        command
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("test fixture parent"))
        .expect("create test fixture parent");
    std::fs::write(path, contents).expect("write test fixture");
}

fn add_package(root: &Path, name: &str, source: &str) {
    write(
        root,
        &format!("{name}/Cargo.toml"),
        &format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n"),
    );
    write(root, &format!("{name}/src/lib.rs"), source);
}

fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn fixture() -> TestWorkspace {
    let workspace = TestWorkspace::new();
    write(
        &workspace.0,
        "Cargo.toml",
        "[workspace]\nresolver = \"2\"\nmembers = [\n  \"fallback-internal\",\n  \"managed-evidence\",\n  \"managed-feature\",\n  \"managed-include\",\n  \"managed-library\",\n  \"ordinary\",\n  \"ordinary-dependent\",\n]\n",
    );
    add_package(
        &workspace.0,
        "fallback-internal",
        "#[cfg(any())] #[kernel(typed, namespace = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\")] pub fn fallback() {}\ninclude!(\"helper.rs\");\nconst _: () = assert!(option_env!(\"FE2O3_CRATE_BINDING_ID_V1\").is_none());\n",
    );
    write(
        &workspace.0,
        "fallback-internal/src/helper.rs",
        "pub fn helper() {}\n",
    );
    add_package(&workspace.0, "managed-evidence", "pub fn ordinary() {}\n");
    write(
        &workspace.0,
        "managed-evidence/tests/fixtures/verus.rs",
        "broadcast use prelude::*;\n",
    );
    add_package(&workspace.0, "managed-feature", "pub fn ordinary() {}\n");
    write(
        &workspace.0,
        "managed-feature/Cargo.toml",
        "[package]\nname = \"managed-feature\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nfallback-internal = { path = \"../fallback-internal\" }\n",
    );
    write(
        &workspace.0,
        "managed-feature/tests/shared.rs",
        "#[cfg(feature = \"gpu\")] #[renamed(typed)] pub fn managed() {}\n",
    );
    add_package(
        &workspace.0,
        "managed-include",
        "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/helper.rs\"));\n",
    );
    write(
        &workspace.0,
        "managed-include/src/helper.rs",
        "pub fn included() {}\n",
    );
    add_package(
        &workspace.0,
        "managed-library",
        "#[cfg(any())] #[kernel(typed)] pub fn managed() {}\nconst _: () = assert!(option_env!(\"FE2O3_CRATE_BINDING_ID_V1\").is_some());\npub fn value() -> u32 { 7 }\n",
    );
    add_package(&workspace.0, "ordinary", "pub fn ordinary() {}\n");
    add_package(
        &workspace.0,
        "ordinary-dependent",
        "pub fn ordinary() -> u32 { managed_library::value() }\n",
    );
    write(
        &workspace.0,
        "ordinary-dependent/Cargo.toml",
        "[package]\nname = \"ordinary-dependent\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[dependencies]\nmanaged-library = { path = \"../managed-library\" }\n",
    );

    let status = workspace
        .command(cargo())
        .args(["generate-lockfile", "--offline"])
        .status()
        .expect("run Cargo lockfile generation");
    assert!(status.success(), "fixture lockfile generation failed");
    workspace
}

#[test]
fn literal_cli_discovers_and_revalidates_the_real_exact_managed_set() {
    let workspace = fixture();
    let binary = env!("CARGO_BIN_EXE_cargo-fe2o3");
    let output = workspace
        .command(binary)
        .args(["examples", "list", "wrapper-managed"])
        .env("CARGO", cargo())
        .output()
        .expect("run literal projection discovery");
    assert!(
        output.status.success(),
        "projection discovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).expect("UTF-8 projection"),
        "managed-evidence\nmanaged-feature\nmanaged-include\nmanaged-library\n"
    );

    let status = workspace
        .command(binary)
        .args([
            "examples",
            "check-wrapper-managed",
            "managed-evidence",
            "managed-feature",
            "managed-include",
            "managed-library",
        ])
        .env("CARGO", cargo())
        .status()
        .expect("run exact projection revalidation");
    assert!(status.success(), "exact projection revalidation failed");

    let output = workspace
        .command(binary)
        .args(["examples", "check-wrapper-managed", "managed-feature"])
        .env("CARGO", cargo())
        .output()
        .expect("run hostile incomplete projection");
    assert!(
        !output.status.success(),
        "incomplete projection was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("package projection changed"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    for (case, args) in [
        (
            "managed-to-fallback",
            vec![
                "check",
                "--all-targets",
                "--locked",
                "-p",
                "managed-feature",
            ],
        ),
        (
            "ordinary-to-managed",
            vec![
                "check",
                "--all-targets",
                "--locked",
                "-p",
                "ordinary-dependent",
            ],
        ),
        (
            "whole-workspace",
            vec!["check", "--workspace", "--all-targets", "--locked"],
        ),
    ] {
        let target = workspace.0.join(format!("target-{case}"));
        std::fs::create_dir(&target).expect("create isolated target");
        let output = workspace
            .command(binary)
            .args(args)
            .env("CARGO", cargo())
            .env("CARGO_TARGET_DIR", &target)
            .env("CARGO_PRIMARY_PACKAGE", "attacker")
            .env("CARGO_PKG_NAME", "attacker")
            .env("CARGO_PKG_VERSION", "attacker")
            .env("CARGO_MANIFEST_DIR", "/attacker")
            .output()
            .expect("run package-aware dependency check");
        assert!(
            output.status.success(),
            "package-aware binding check failed {case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn literal_cli_computes_disjoint_exhaustive_cpu_test_partitions() {
    let workspace = TestWorkspace::new();
    write(
        &workspace.0,
        "Cargo.toml",
        "[workspace]\nresolver = \"2\"\nmembers = [\n  \"examples/ignored\",\n  \"examples/managed\",\n  \"examples/raw-a\",\n  \"examples/raw-b\",\n  \"examples/rocm\",\n]\n",
    );
    write(
        &workspace.0,
        "examples/regression-manifest-v1.txt",
        "fe2o3-example-regressions-v1\npackage|rustc_check|rocm_compile|gpu_smoke|artifacts\nignored|false|false|false|-\nmanaged|true|false|false|-\nraw-a|true|false|false|-\nraw-b|true|false|false|-\nrocm|true|true|true|rocm_kernel.hsaco\n",
    );
    for (package, source) in [
        ("ignored", "pub fn ignored() {}\n"),
        (
            "managed",
            "include!(concat!(env!(\"OUT_DIR\"), \"/generated.rs\"));\n",
        ),
        ("raw-a", "pub fn raw_a() {}\n"),
        ("raw-b", "pub fn raw_b() {}\n"),
        (
            "rocm",
            "#[cfg(any())] #[kernel(typed, namespace = \"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\")] pub fn rocm_kernel() {}\n",
        ),
    ] {
        add_package(&workspace.0.join("examples"), package, source);
    }
    let status = workspace
        .command(cargo())
        .args(["generate-lockfile", "--offline"])
        .status()
        .expect("generate CPU partition fixture lockfile");
    assert!(status.success());

    let binary = env!("CARGO_BIN_EXE_cargo-fe2o3");
    let list = |lane: &str| {
        let output = workspace
            .command(binary)
            .args(["examples", "list", lane])
            .env("CARGO", cargo())
            .output()
            .expect("query CPU test package partition");
        assert!(
            output.status.success(),
            "CPU partition query {lane} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("UTF-8 CPU package partition")
    };
    assert_eq!(list("cpu-test-raw"), "raw-a\nraw-b\n");
    assert_eq!(list("cpu-test-wrapper-managed"), "managed\n");
    assert_eq!(list("rustc-check"), "managed\nraw-a\nraw-b\nrocm\n");
    assert_eq!(list("rocm-compile"), "rocm\n");
    assert_eq!(list("gpu-smoke"), "rocm\n");

    let output = workspace
        .command(binary)
        .args([
            "examples",
            "check-artifacts",
            "managed",
            workspace.0.to_str().expect("UTF-8 fixture path"),
        ])
        .env("CARGO", cargo())
        .output()
        .expect("reject artifact inspection outside the ROCm compile lane");
    assert!(!output.status.success(), "non-ROCm artifacts were accepted");
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("does not participate in ROCm compilation"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = workspace
        .command(binary)
        .arg("smoke")
        .env("CARGO", cargo())
        .output()
        .expect("reject retired manifest smoke command");
    assert!(
        !output.status.success(),
        "retired smoke command was accepted"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("usage: cargo fe2o3"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let check = |packages: &[&str]| {
        workspace
            .command(binary)
            .arg("examples")
            .arg("check-cpu-test-partition")
            .args(packages)
            .env("CARGO", cargo())
            .output()
            .expect("revalidate CPU test package partition")
    };
    let output = check(&["raw-a", "raw-b", "--", "managed"]);
    assert!(
        output.status.success(),
        "exact CPU partition revalidation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for (case, packages, expected) in [
        ("missing-separator", vec!["raw-a", "managed"], "exactly one"),
        (
            "duplicate-separator",
            vec!["raw-a", "--", "--", "managed"],
            "exactly one",
        ),
        (
            "unsorted",
            vec!["raw-b", "raw-a", "--", "managed"],
            "strictly sorted and unique",
        ),
        (
            "duplicate",
            vec!["raw-a", "raw-a", "--", "managed"],
            "strictly sorted and unique",
        ),
        ("drift", vec!["raw-a", "--", "managed"], "partition changed"),
    ] {
        let output = check(&packages);
        assert!(
            !output.status.success(),
            "hostile CPU partition case {case} was accepted"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "CPU partition case {case}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn literal_cli_rejects_non_rs_declared_target_roots() {
    let workspace = TestWorkspace::new();
    write(
        &workspace.0,
        "Cargo.toml",
        "[workspace]\nresolver = \"2\"\nmembers = [\"non-rs\"]\n",
    );
    write(
        &workspace.0,
        "non-rs/Cargo.toml",
        "[package]\nname = \"non-rs\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[lib]\npath = \"src/lib.kernel\"\n",
    );
    write(
        &workspace.0,
        "non-rs/src/lib.kernel",
        "#[cfg(any())] #[kernel(typed)] pub fn managed() {}\n",
    );
    let status = workspace
        .command(cargo())
        .args(["generate-lockfile", "--offline"])
        .status()
        .expect("generate non-rs fixture lockfile");
    assert!(status.success());
    let output = workspace
        .command(env!("CARGO_BIN_EXE_cargo-fe2o3"))
        .args(["examples", "list", "wrapper-managed"])
        .env("CARGO", cargo())
        .output()
        .expect("run non-rs projection rejection");
    assert!(!output.status.success(), "non-rs Cargo target was accepted");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must be a UTF-8 .rs path"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn nonvirtual_root_package_excludes_its_generated_target_directory_on_repeat() {
    let workspace = TestWorkspace::new();
    write(
        &workspace.0,
        "Cargo.toml",
        "[package]\nname = \"root-ordinary\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    );
    write(
        &workspace.0,
        "src/lib.rs",
        "pub fn ordinary() {}\nconst _: () = assert!(option_env!(\"FE2O3_CRATE_BINDING_ID_V1\").is_none());\n",
    );
    let status = Command::new(cargo())
        .args(["generate-lockfile", "--offline"])
        .env_remove("CARGO_TARGET_DIR")
        .current_dir(&workspace.0)
        .status()
        .expect("generate root-package lockfile");
    assert!(status.success());
    let binary = env!("CARGO_BIN_EXE_cargo-fe2o3");
    for attempt in 0..2 {
        let output = Command::new(binary)
            .args(["check", "--workspace", "--all-targets", "--locked"])
            .env("CARGO", cargo())
            .env_remove("CARGO_TARGET_DIR")
            .current_dir(&workspace.0)
            .output()
            .expect("run repeated root-package binding check");
        assert!(
            output.status.success(),
            "root-package check {attempt} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        if attempt == 0 {
            write(
                &workspace.0,
                "target/generated-hostile.rs",
                "#[kernel(typed)] pub fn hidden_managed() {}\n",
            );
            std::os::unix::fs::symlink("missing", workspace.0.join("target/hostile-link"))
                .expect("add generated target symlink");
        }
    }
}

fn memfd(name: &str, sealed: bool) -> File {
    let name = CString::new(name).expect("memfd name");
    // SAFETY: memfd_create returns a fresh descriptor or a negative status.
    let descriptor =
        unsafe { libc::memfd_create(name.as_ptr(), libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING) };
    assert!(descriptor >= 0, "create test memfd");
    // SAFETY: the successful call returned a fresh owned descriptor.
    let mut file = unsafe { File::from_raw_fd(descriptor) };
    file.write_all(b"not-a-projection").expect("write memfd");
    if sealed {
        let seals =
            libc::F_SEAL_WRITE | libc::F_SEAL_GROW | libc::F_SEAL_SHRINK | libc::F_SEAL_SEAL;
        // SAFETY: F_ADD_SEALS accepts this bitset for the live sealing-enabled memfd.
        assert_eq!(
            unsafe { libc::fcntl(file.as_raw_fd(), libc::F_ADD_SEALS, seals) },
            0
        );
    }
    file
}

fn invoke_wrapper_with_projection(file: &File) -> std::process::Output {
    let source = file.as_raw_fd();
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .args(["/bin/true", "-vV"])
        .env("FE2O3_BINDING_CHECK_WRAPPER_MODE_V1", "1");
    // SAFETY: the callback only duplicates the retained fixture descriptor in the child.
    unsafe {
        command.pre_exec(move || {
            if libc::dup3(source, 201, 0) != 201 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.output().expect("run projection custody rejection")
}

#[test]
fn inherited_projection_rejects_named_unsealed_and_wrong_name_files() {
    let workspace = TestWorkspace::new();
    let named_path = workspace.0.join("named-projection");
    std::fs::write(&named_path, b"not-a-projection").expect("write named fixture");
    let named = File::open(&named_path).expect("open named fixture");
    let output = invoke_wrapper_with_projection(&named);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("bounded anonymous regular file"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let unsealed = memfd("fe2o3-binding-check-projection-v1", false);
    let output = invoke_wrapper_with_projection(&unsealed);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exact immutable seals"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let wrong_name = memfd("fe2o3-wrong-projection", true);
    let output = invoke_wrapper_with_projection(&wrong_name);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("exact named memfd"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

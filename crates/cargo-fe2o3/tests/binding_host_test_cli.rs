use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
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
                "fe2o3-binding-host-test-cli-{}-{suffix}",
                std::process::id()
            ));
            match std::fs::create_dir(&root) {
                Ok(()) => return Self(root),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create test workspace: {error}"),
            }
        }
    }
}

impl Drop for TestWorkspace {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
    std::fs::write(path, contents).expect("write fixture");
}

fn write_executable(root: &Path, relative: &str, contents: &str) -> PathBuf {
    write(root, relative, contents);
    let path = root.join(relative);
    let mut permissions = std::fs::metadata(&path)
        .expect("inspect executable fixture")
        .permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(&path, permissions).expect("make fixture executable");
    path
}

fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn host_target() -> String {
    let rustc = std::env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc"));
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("query rustc host target");
    assert!(output.status.success(), "rustc host query failed");
    String::from_utf8(output.stdout)
        .expect("UTF-8 rustc version")
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .expect("rustc host target")
}

fn fixture() -> TestWorkspace {
    let workspace = TestWorkspace::new();
    write(
        &workspace.0,
        "Cargo.toml",
        "[workspace]\nresolver = \"2\"\nmembers = [\"managed\"]\n",
    );
    write(
        &workspace.0,
        "managed/Cargo.toml",
        "[package]\nname = \"managed\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    );
    write(
        &workspace.0,
        "managed/src/lib.rs",
        r#"#[cfg(any())] #[kernel(typed)] pub fn managed() {}
const _: () = assert!(option_env!("FE2O3_CRATE_BINDING_ID_V1").is_some());

#[cfg(test)]
mod tests {
    #[test]
    fn binding_is_present_and_runner_custody_is_closed() {
        assert!(option_env!("FE2O3_CRATE_BINDING_ID_V1").is_some());
        if let Some(marker) = std::env::var_os("BINDING_TEST_EXECUTION_MARKER") {
            std::fs::write(marker, b"executed").expect("write host-test execution marker");
        }
        for (name, _) in std::env::vars_os() {
            let name = name.to_string_lossy();
            assert!(!name.starts_with("FE2O3_"), "protected variable {name}");
            assert!(
                !matches!(
                    name.as_ref(),
                    "RUSTC"
                        | "CARGO_BUILD_RUSTC"
                        | "RUSTC_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WRAPPER"
                        | "RUSTC_WORKSPACE_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                        | "RUSTDOC"
                        | "CARGO_BUILD_RUSTDOC"
                        | "RUSTDOCFLAGS"
                        | "RUSTFLAGS"
                        | "CARGO_ENCODED_RUSTFLAGS"
                        | "LD_PRELOAD"
                        | "LD_AUDIT"
                        | "GLIBC_TUNABLES"
                ),
                "tool or loader variable {name}"
            );
            assert!(!name.starts_with("DYLD_"), "loader variable {name}");
            assert!(name == "LD_LIBRARY_PATH" || !name.starts_with("LD_"), "loader variable {name}");
            assert!(!name.starts_with("CARGO_TARGET_") || !name.ends_with("_RUNNER"), "runner variable {name}");
        }

        unsafe extern "C" {
            fn fcntl(fd: i32, command: i32, ...) -> i32;
        }
        for fd in [191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201] {
            // SAFETY: F_GETFD only queries whether this integer descriptor is open.
            assert_eq!(unsafe { fcntl(fd, 1) }, -1, "fixed descriptor {fd} remained open");
            assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(9));
        }

        let executable = std::env::current_exe().expect("resolve original test executable");
        assert!(executable.exists(), "current test executable was detached from its Cargo path");
        assert!(!executable.to_string_lossy().contains("memfd:"));
        let relaunch = std::process::Command::new(executable)
            .arg("--list")
            .output()
            .expect("relaunch Cargo test executable by current_exe path");
        assert!(relaunch.status.success(), "current_exe relaunch failed");
    }
}
"#,
    );
    let status = Command::new(cargo())
        .args(["generate-lockfile", "--offline"])
        .current_dir(&workspace.0)
        .status()
        .expect("generate fixture lockfile");
    assert!(status.success(), "fixture lockfile generation failed");
    workspace
}

fn binding_test(workspace: &TestWorkspace) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
    command
        .args(["test", "--locked", "--all-targets", "-p", "managed"])
        .env("CARGO", cargo())
        .current_dir(&workspace.0);
    command
}

#[test]
fn literal_binding_host_test_executes_only_through_the_pinned_runner() {
    let workspace = fixture();
    let rustdoc_marker = workspace.0.join("hostile-rustdoc-invoked");
    let rustdoc = write_executable(
        &workspace.0,
        "hostile-rustdoc.sh",
        &format!(
            "#!/bin/sh\nprintf invoked > '{}'\nexit 98\n",
            rustdoc_marker.display()
        ),
    );
    write(
        &workspace.0,
        ".cargo/config.toml",
        &format!(
            "[env]\nRUSTDOC = {{ value = '{}', force = true }}\n",
            rustdoc.display()
        ),
    );
    let target = workspace.0.join("target-binding-host");
    let output = binding_test(&workspace)
        .env("CARGO_TARGET_DIR", target)
        .output()
        .expect("run literal binding-only host test");
    assert!(
        output.status.success(),
        "binding-only host test failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("binding_is_present_and_runner_custody_is_closed"),
        "managed host unit test did not execute: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(!rustdoc_marker.exists(), "hostile rustdoc executed");
}

#[test]
fn binding_host_test_rejects_every_runner_and_unstable_config_channel() {
    let workspace = fixture();
    let host = host_target();
    let marker = workspace.0.join("hostile-runner-invoked");
    let runner = write_executable(
        &workspace.0,
        "hostile-runner.sh",
        &format!(
            "#!/bin/sh\nprintf invoked > '{}'\nexit 97\n",
            marker.display()
        ),
    );

    for args in [
        vec![
            "test",
            "--all-targets",
            "--config",
            "target.hostile.runner='/bin/false'",
            "-p",
            "managed",
        ],
        vec![
            "test",
            "--all-targets",
            "-Zconfig-include=hostile.toml",
            "-p",
            "managed",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(args)
            .env("CARGO", cargo())
            .current_dir(&workspace.0)
            .output()
            .expect("run caller config rejection");
        assert!(!output.status.success());
    }

    let ambient = binding_test(&workspace)
        .env(
            format!(
                "CARGO_TARGET_{}_RUNNER",
                host.to_ascii_uppercase().replace('-', "_")
            ),
            &runner,
        )
        .output()
        .expect("run ambient runner rejection");
    assert!(!ambient.status.success());
    assert!(
        String::from_utf8_lossy(&ambient.stderr).contains("ambient Cargo runner selection"),
        "{}",
        String::from_utf8_lossy(&ambient.stderr)
    );

    let rustdoc_marker = workspace.0.join("ambient-rustdoc-invoked");
    let rustdoc = write_executable(
        &workspace.0,
        "ambient-rustdoc.sh",
        &format!(
            "#!/bin/sh\nprintf invoked > '{}'\nexit 96\n",
            rustdoc_marker.display()
        ),
    );
    let ambient_rustdoc = binding_test(&workspace)
        .env("RUSTDOC", &rustdoc)
        .output()
        .expect("run ambient rustdoc rejection");
    assert!(!ambient_rustdoc.status.success());
    assert!(
        String::from_utf8_lossy(&ambient_rustdoc.stderr)
            .contains("ambient rustdoc selection RUSTDOC="),
        "{}",
        String::from_utf8_lossy(&ambient_rustdoc.stderr)
    );
    assert!(!rustdoc_marker.exists(), "ambient rustdoc executed");

    for (label, configuration, expected) in [
        (
            "cfg",
            format!("[target.'cfg(unix)']\nrunner = ['{}']\n", runner.display()),
            "configured target.cfg(unix).runner".to_owned(),
        ),
        (
            "exact",
            format!("[target.{host}]\nrunner = ['{}']\n", runner.display()),
            format!("configured target.{host}.runner"),
        ),
        (
            "environment",
            format!(
                "[env]\nCARGO_TARGET_{}_RUNNER = {{ value = '{}', force = true }}\n",
                host.to_ascii_uppercase().replace('-', "_"),
                runner.display()
            ),
            "configured runner environment".to_owned(),
        ),
        (
            "protected",
            "[env]\nFE2O3_TARGET = { value = 'hostile', force = true }\n".to_owned(),
            "configured protected environment env.FE2O3_TARGET".to_owned(),
        ),
    ] {
        write(&workspace.0, ".cargo/config.toml", &configuration);
        let output = binding_test(&workspace)
            .output()
            .unwrap_or_else(|error| panic!("run {label} config rejection: {error}"));
        assert!(!output.status.success(), "{label} config was accepted");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(&expected),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!marker.exists(), "{label} runner executed");
    }

    std::fs::remove_file(workspace.0.join(".cargo/config.toml"))
        .expect("remove workspace Cargo config");
    let cargo_home = workspace.0.join("hostile-cargo-home");
    write(
        &cargo_home,
        "config.toml",
        &format!("[target.{host}]\nrunner = ['{}']\n", runner.display()),
    );
    let global = binding_test(&workspace)
        .env("CARGO_HOME", cargo_home)
        .output()
        .expect("run global Cargo-home runner rejection");
    assert!(!global.status.success());
    assert!(
        String::from_utf8_lossy(&global.stderr)
            .contains(&format!("configured target.{host}.runner")),
        "{}",
        String::from_utf8_lossy(&global.stderr)
    );
    assert!(!marker.exists(), "global Cargo-home runner executed");
}

#[test]
fn binding_host_test_rejects_nonexecuting_cargo_modes_before_test_spawn() {
    let workspace = fixture();
    let host = host_target();
    for (label, args, expected) in [
        (
            "missing-all-targets",
            vec!["test", "-p", "managed"],
            "requires exact --all-targets",
        ),
        (
            "doc",
            vec!["test", "--all-targets", "--doc", "-p", "managed"],
            "does not admit rustdoc targets",
        ),
        (
            "no-run",
            vec!["test", "--all-targets", "--no-run", "-p", "managed"],
            "must execute the selected host tests",
        ),
        (
            "target",
            vec![
                "test",
                "--all-targets",
                "--target",
                host.as_str(),
                "-p",
                "managed",
            ],
            "--target is not admitted",
        ),
    ] {
        let marker = workspace.0.join(format!("{label}-test-executed"));
        let output = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"))
            .args(args)
            .env("CARGO", cargo())
            .env("BINDING_TEST_EXECUTION_MARKER", &marker)
            .current_dir(&workspace.0)
            .output()
            .unwrap_or_else(|error| panic!("run {label} rejection: {error}"));
        assert!(!output.status.success(), "{label} was accepted");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!marker.exists(), "{label} executed the host test");
    }
}

#[test]
fn binding_host_test_rejects_forced_loader_configuration_before_construction() {
    let workspace = fixture();
    let marker = workspace.0.join("hostile-constructor-invoked");
    write(
        &workspace.0,
        "hostile-constructor.c",
        &format!(
            "#include <stdio.h>\n__attribute__((constructor)) static void hostile(void) {{ FILE *f = fopen(\"{}\", \"w\"); if (f) {{ fputs(\"invoked\", f); fclose(f); }} }}\n",
            marker.display()
        ),
    );
    let shared_object = workspace.0.join("hostile-constructor.so");
    let compile = Command::new("cc")
        .args(["-shared", "-fPIC"])
        .arg(workspace.0.join("hostile-constructor.c"))
        .arg("-o")
        .arg(&shared_object)
        .status()
        .expect("compile hostile constructor fixture");
    assert!(compile.success(), "hostile constructor fixture failed");
    write(
        &workspace.0,
        ".cargo/config.toml",
        &format!(
            "[env]\nLD_PRELOAD = {{ value = '{}', force = true }}\n",
            shared_object.display()
        ),
    );
    let output = binding_test(&workspace)
        .output()
        .expect("run configured loader rejection");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("configured dynamic-loader environment env.LD_PRELOAD"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!marker.exists(), "configured loader constructor executed");
}

#[test]
fn binding_host_test_rejects_persistent_post_query_config_swap() {
    let workspace = fixture();
    let host = host_target();
    let host_environment = host.to_ascii_uppercase().replace('-', "_");
    let runner_marker = workspace.0.join("raced-runner-invoked");
    let rustdoc_marker = workspace.0.join("raced-rustdoc-invoked");
    let runner = write_executable(
        &workspace.0,
        "raced-runner.sh",
        &format!(
            "#!/bin/sh\nprintf invoked > '{}'\nexit 94\n",
            runner_marker.display()
        ),
    );
    let rustdoc = write_executable(
        &workspace.0,
        "raced-rustdoc.sh",
        &format!(
            "#!/bin/sh\nprintf invoked > '{}'\nexit 95\n",
            rustdoc_marker.display()
        ),
    );
    write(
        &workspace.0,
        "delegating-cargo.c",
        r#"#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

static int run_real_cargo(const char *real, int argc, char **argv) {
    char **child = calloc((size_t)argc + 1, sizeof(char *));
    if (child == NULL) return 125;
    child[0] = (char *)real;
    for (int i = 1; i < argc; ++i) child[i] = argv[i];
    pid_t pid = fork();
    if (pid == 0) {
        execvp(real, child);
        _exit(126);
    }
    if (pid < 0) return 125;
    int status;
    while (waitpid(pid, &status, 0) < 0) {
        if (errno != EINTR) return 125;
    }
    if (WIFEXITED(status)) return WEXITSTATUS(status);
    if (WIFSIGNALED(status)) return 128 + WTERMSIG(status);
    return 125;
}

int main(int argc, char **argv) {
    const char *real = getenv("FE2O3_TEST_REAL_CARGO");
    if (real == NULL) return 125;
    const char *trace_path = getenv("FE2O3_TEST_CARGO_PROXY_TRACE");
    if (trace_path == NULL) return 125;
    FILE *trace = fopen(trace_path, "a");
    if (trace == NULL) return 125;
    for (int i = 1; i < argc; ++i) fprintf(trace, "%s%s", i == 1 ? "" : " ", argv[i]);
    fputc('\n', trace);
    fclose(trace);
    int status = run_real_cargo(real, argc, argv);
    if (argc >= 3 && strcmp(argv[1], "config") == 0 &&
        strcmp(argv[2], "get") == 0 && strcmp(argv[argc - 1], "env") == 0) {
        const char *count_path = getenv("FE2O3_TEST_CONFIG_QUERY_COUNT");
        const char *config_path = getenv("FE2O3_TEST_SWAP_CONFIG_PATH");
        const char *configuration = getenv("FE2O3_TEST_SWAP_CONFIG_BYTES");
        if (count_path == NULL || config_path == NULL || configuration == NULL) return 125;
        unsigned count = 0;
        FILE *count_in = fopen(count_path, "r");
        if (count_in != NULL) {
            if (fscanf(count_in, "%u", &count) != 1) return 125;
            fclose(count_in);
        }
        ++count;
        FILE *count_out = fopen(count_path, "w");
        if (count_out == NULL) return 125;
        fprintf(count_out, "%u\n", count);
        fclose(count_out);
        if (count == 2) {
            FILE *config = fopen(config_path, "w");
            if (config == NULL) return 125;
            if (fputs(configuration, config) == EOF || fclose(config) != 0) return 125;
        }
    }
    return status;
}
"#,
    );
    let proxy = workspace.0.join("delegating-cargo");
    let compile = Command::new("cc")
        .arg(workspace.0.join("delegating-cargo.c"))
        .arg("-o")
        .arg(&proxy)
        .status()
        .expect("compile delegating Cargo fixture");
    assert!(compile.success(), "delegating Cargo fixture failed");

    let configuration = format!(
        "[target.{host}]\nrunner = ['{}']\n[env]\nCARGO_TARGET_{host_environment}_RUNNER = {{ value = '{}', force = true }}\nLD_TRACE_LOADED_OBJECTS = {{ value = '1', force = true }}\nRUSTDOC = {{ value = '{}', force = true }}\n",
        runner.display(),
        runner.display(),
        rustdoc.display()
    );
    let count = workspace.0.join("config-env-query-count");
    let trace = workspace.0.join("delegating-cargo-trace");
    let config_path = workspace.0.join(".cargo/config.toml");
    std::fs::create_dir(workspace.0.join(".cargo")).expect("create raced Cargo config directory");
    let cargo_home = workspace.0.join("isolated-cargo-home");
    std::fs::create_dir(&cargo_home).expect("create isolated Cargo home");
    let target = workspace.0.join("target-raced-custody");
    let output = binding_test(&workspace)
        .env("CARGO", &proxy)
        .env("CARGO_HOME", cargo_home)
        .env("CARGO_TARGET_DIR", target)
        .env("FE2O3_TEST_REAL_CARGO", cargo())
        .env("FE2O3_TEST_CONFIG_QUERY_COUNT", &count)
        .env("FE2O3_TEST_CARGO_PROXY_TRACE", &trace)
        .env("FE2O3_TEST_SWAP_CONFIG_PATH", &config_path)
        .env("FE2O3_TEST_SWAP_CONFIG_BYTES", &configuration)
        .output()
        .expect("run post-query Cargo configuration swap");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "Cargo test configuration revalidation also failed: cargo fe2o3 rejects configured dynamic-loader environment env.LD_TRACE_LOADED_OBJECTS"
        ),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        config_path.exists(),
        "delegating Cargo did not swap configuration; trace:\n{}",
        std::fs::read_to_string(&trace).unwrap_or_else(|error| format!("<unavailable: {error}>"))
    );
    assert!(
        std::fs::read_to_string(count)
            .expect("read config query count")
            .trim()
            .parse::<u32>()
            .expect("parse config query count")
            >= 2
    );
    assert!(!runner_marker.exists(), "raced runner executed");
    assert!(!rustdoc_marker.exists(), "raced rustdoc executed");
}

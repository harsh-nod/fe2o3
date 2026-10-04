use super::*;
use std::ffi::OsStr;

fn version(host: &str) -> String {
    format!(
        "rustc test identity\nrelease: {}\ncommit-hash: {}\nLLVM version: {}\nhost: {host}\n",
        env!("FE2O3_BUILD_RUSTC_RELEASE"),
        env!("FE2O3_BUILD_RUSTC_COMMIT"),
        env!("FE2O3_BUILD_RUSTC_LLVM"),
    )
}

fn example_compiler() -> HostCompiler {
    HostCompiler {
        executable: "selected-rustc".into(),
        sysroot: PathBuf::from("/fixture/sysroot"),
        host: "x86_64-unknown-linux-gnu".into(),
    }
}

fn message(name: &str, files: &[PathBuf]) -> serde_json::Value {
    serde_json::json!({
        "reason": "compiler-artifact", "target": { "name": name }, "filenames": files
    })
}

#[test]
fn primitive_from_debug_core_requires_exact_unique_compiler_identity() {
    let valid = version("x86_64-unknown-linux-gnu");
    assert_eq!(checked_host(&valid), "x86_64-unknown-linux-gnu");
    for prefix in ["release: ", "commit-hash: ", "LLVM version: ", "host: "] {
        let missing = valid
            .lines()
            .filter(|line| !line.starts_with(prefix))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(std::panic::catch_unwind(|| checked_host(&missing)).is_err());
        let duplicate = format!("{valid}{prefix}{}\n", field(&valid, prefix));
        assert!(std::panic::catch_unwind(|| checked_host(&duplicate)).is_err());
        let wrong = valid.replace(
            &format!("{prefix}{}", field(&valid, prefix)),
            &format!(
                "{prefix}{}",
                if prefix == "host: " {
                    "--target bad"
                } else {
                    "different"
                }
            ),
        );
        assert!(std::panic::catch_unwind(|| checked_host(&wrong)).is_err());
    }
}

#[test]
fn primitive_from_debug_core_producer_and_consumer_share_explicit_host_flags() {
    let compiler = example_compiler();
    let command = build_command(
        Path::new("/fixture/workspace"),
        Path::new("/fixture/owned"),
        &compiler,
    );
    assert_eq!(
        command.get_current_dir(),
        Some(Path::new("/fixture/workspace"))
    );
    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "check",
            "--offline",
            "--locked",
            "-Zbuild-std=core",
            "--lib",
            "-p",
            "fe2o3-device",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--message-format=json-render-diagnostics",
            "--target-dir",
            "/fixture/owned",
        ]
        .map(OsStr::new)
    );
    let environment: std::collections::BTreeMap<_, _> = command.get_envs().collect();
    for (key, expected) in [
        ("RUSTC", "selected-rustc"),
        ("CARGO_BUILD_JOBS", "1"),
        ("CARGO_INCREMENTAL", "0"),
        ("CARGO_NET_OFFLINE", "true"),
        ("FE2O3_HIP_SYS_DISABLE", "1"),
        ("FE2O3_HSA_RUNTIME_DISABLE", "1"),
    ] {
        assert_eq!(
            environment.get(OsStr::new(key)),
            Some(&Some(OsStr::new(expected)))
        );
    }
    let encoded = MIR_FLAGS.join("\x1f");
    assert_eq!(
        environment.get(OsStr::new("CARGO_ENCODED_RUSTFLAGS")),
        Some(&Some(OsStr::new(&encoded)))
    );
    assert!(!environment.contains_key(OsStr::new("RUSTFLAGS")));
    assert!(!environment.contains_key(OsStr::new("RUSTC_WRAPPER")));
    assert!(!environment.contains_key(OsStr::new("RUSTC_WORKSPACE_WRAPPER")));
    let args = arguments(
        Path::new("/fixture/owned"),
        Path::new("/fixture/owned/fixture.rs"),
        &compiler,
        Path::new("/fixture/owned/deps/libcore.rmeta"),
        Path::new("/fixture/owned/deps/libcompiler_builtins.rmeta"),
    );
    for flag in MIR_FLAGS {
        assert_eq!(args.iter().filter(|arg| arg.as_str() == flag).count(), 1);
    }
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--target", "x86_64-unknown-linux-gnu"])
    );
    assert!(
        args.windows(2)
            .any(|pair| pair == ["--sysroot", "/fixture/sysroot"])
    );
    assert!(args.contains(&"--extern=noprelude:core=/fixture/owned/deps/libcore.rmeta".into()));
    assert!(
        args.contains(
            &"--extern=noprelude:compiler_builtins=/fixture/owned/deps/libcompiler_builtins.rmeta"
                .into()
        )
    );
    assert_eq!(
        args.iter()
            .filter(|arg| arg.starts_with("-Ldependency="))
            .count(),
        1
    );
    assert!(
        !args
            .iter()
            .any(|arg| arg.contains("amdgcn") || arg.contains("target-cpu"))
    );
    let separate = arguments(
        Path::new("/fixture/owned"),
        Path::new("/fixture/owned/fixture.rs"),
        &compiler,
        Path::new("/fixture/owned/core/libcore.rmeta"),
        Path::new("/fixture/owned/builtins/libcompiler_builtins.rmeta"),
    );
    assert_eq!(
        separate
            .iter()
            .filter(|arg| arg.starts_with("-Ldependency="))
            .count(),
        2
    );
}

#[test]
fn primitive_from_debug_core_artifacts_require_actual_unique_owned_files() {
    let owned = TestTempDir::create("fe2o3-from-core-artifact");
    let foreign = TestTempDir::create("fe2o3-from-foreign-artifact");
    let metadata = owned.path().join("libcore.rmeta");
    let library = owned.path().join("libcore.rlib");
    let other = owned.path().join("other.rmeta");
    let outside = foreign.path().join("libcore.rmeta");
    for path in [&metadata, &library, &other, &outside] {
        std::fs::write(path, b"test artifact selection only").unwrap();
    }
    #[cfg(unix)]
    {
        let linked = owned.path().join("escaped.rmeta");
        std::os::unix::fs::symlink(&outside, &linked).unwrap();
        let messages = [message("core", &[linked])];
        assert!(
            std::panic::catch_unwind(|| owned_artifact(&messages, "core", owned.path())).is_err()
        );
    }
    let selected = message("core", &[library.clone(), metadata.clone()]);
    assert_eq!(
        owned_artifact(std::slice::from_ref(&selected), "core", owned.path()),
        metadata.canonicalize().unwrap()
    );
    assert_eq!(
        owned_artifact(
            &[message("core", std::slice::from_ref(&library))],
            "core",
            owned.path()
        ),
        library.canonicalize().unwrap()
    );
    for messages in [
        vec![],
        vec![selected.clone(), selected],
        vec![message("core", &[metadata.clone(), other])],
        vec![message("compiler_builtins", &[metadata])],
        vec![message("core", &[outside])],
        vec![message("core", &[owned.path().join("missing.rmeta")])],
    ] {
        assert!(
            std::panic::catch_unwind(|| owned_artifact(&messages, "core", owned.path())).is_err()
        );
    }
}

#[test]
fn primitive_from_debug_core_owned_setup_cleanup_preserves_live_sibling() {
    let sibling = TestTempDir::create("fe2o3-from-core-sibling");
    let sentinel = sibling.path().join("sentinel");
    std::fs::write(&sentinel, b"retain").unwrap();
    for fail in [false, true] {
        let mut path = None;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let owned = TestTempDir::create("fe2o3-from-core-cleanup");
            path = Some(owned.path().to_owned());
            let nested = owned.path().join("debug-core/deps");
            std::fs::create_dir_all(&nested).unwrap();
            std::fs::write(nested.join("core.rmeta"), b"owned test bytes").unwrap();
            assert!(!fail, "fixture setup failure");
        }));
        assert_eq!(result.is_err(), fail);
        assert!(!path.unwrap().exists());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"retain");
    }
}

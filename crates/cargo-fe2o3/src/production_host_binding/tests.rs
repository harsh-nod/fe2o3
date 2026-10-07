use super::*;
use crate::pinned_executable_test_directory::TestDirectory;
use std::os::unix::fs::MetadataExt;

const HOST: &str = "x86_64-unknown-linux-gnu";
const DEVICE_SYSROOT: [&str; 6] = [
    "--extern",
    "noprelude,nounused:core=/target/libcore.rlib",
    "--extern",
    "noprelude,nounused:compiler_builtins=/target/libcompiler_builtins.rlib",
    "-Z",
    "unstable-options",
];

fn argv(target: &str) -> Vec<OsString> {
    [
        "/proc/self/fd/194",
        "--crate-name",
        "selected",
        "src/lib.rs",
        "--crate-type=lib",
        "--edition=2024",
        "--target",
        target,
        "-C",
        "metadata=device-salt",
        "-Copt-level=1",
        "-C",
        "panic=abort",
        "--cfg",
        "feature=\"first\"",
    ]
    .into_iter()
    .map(OsString::from)
    .collect()
}

fn compile(argv: &[OsString]) -> RustcCompileInvocationV2<'_> {
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(argv).unwrap() else {
        panic!("compile fixture")
    };
    compile
}

fn device_argv(target: &str) -> Vec<OsString> {
    let mut args = argv(target);
    args.extend(DEVICE_SYSROOT.map(OsString::from));
    args
}

fn fixture() -> (TestDirectory, HostProjection) {
    let directory = TestDirectory::new();
    std::fs::create_dir(directory.path().join("src")).unwrap();
    let source = directory.path().join("src/lib.rs");
    std::fs::write(&source, "pub fn selected() {}\n").unwrap();
    let metadata = std::fs::metadata(directory.path()).unwrap();
    let file = File::open(&source).unwrap();
    let projection = HostProjection {
        format: FORMAT.to_owned(),
        source: Projection {
            workspace_root: directory.path().to_path_buf(),
            workspace_device: metadata.dev(),
            workspace_inode: metadata.ino(),
            targets: vec![TargetSource {
                package_name: "selected-package".to_owned(),
                package_root: directory.path().to_path_buf(),
                package_device: metadata.dev(),
                package_inode: metadata.ino(),
                source_path: source,
                source_identity: ObjectIdentity::from_stat(&rustix::fs::fstat(&file).unwrap())
                    .unwrap(),
                managed: true,
            }],
        },
        crate_name: "selected".to_owned(),
        host_target: HOST.to_owned(),
        original_binding: derive_crate_binding_id_v1("selected", ["device-salt"]).to_hex(),
        unit: library_configuration(compile(&argv(HOST)), HOST).unwrap(),
    };
    (directory, projection)
}

fn binding<'a>(
    projection: &'a HostProjection,
    args: &[OsString],
) -> Result<Option<&'a str>, String> {
    let root = &projection.source.workspace_root;
    projection.binding_for(
        compile(args),
        root,
        Some(OsStr::new("selected-package")),
        Some(root.as_os_str()),
    )
}

#[test]
fn original_binding_survives_different_host_metadata_and_repeated_consumers() {
    let (_directory, projection) = fixture();
    let mut args = argv(HOST);
    *args
        .iter_mut()
        .find(|v| *v == "metadata=device-salt")
        .unwrap() = "metadata=different-host-salt".into();
    args.extend(
        [
            "--out-dir",
            "/different/output",
            "--extern",
            "dependency=/other/lib.rlib",
            "-Ctarget-feature=+crt-static",
            "-Crelocation-model=static",
            "-Clink-arg=-no-pie",
        ]
        .map(OsString::from),
    );
    let encoded = projection.encode().unwrap();
    for _ in 0..2 {
        let decoded = HostProjection::decode(&encoded).unwrap();
        assert_eq!(
            binding(&decoded, &args).unwrap(),
            Some(projection.original_binding.as_str())
        );
    }
    assert_ne!(
        projection.original_binding,
        derive_crate_binding_id_v1("selected", ["different-host-salt"]).to_hex()
    );
}

#[test]
fn static_host_probe_is_query_only_and_retains_original_compile_arguments() {
    let query = [
        "/proc/self/fd/194",
        "-",
        "--crate-name",
        "___",
        "--print=file-names",
        "-Ctarget-feature=+crt-static",
        "-Crelocation-model=static",
        "-Clink-arg=-no-pie",
        "--target",
        HOST,
        "--crate-type",
        "bin",
        "--crate-type",
        "rlib",
        "--print=sysroot",
        "--print=split-debuginfo",
        "--print=crate-name",
        "--print=cfg",
        "-Wwarnings",
    ]
    .map(OsString::from)
    .to_vec();
    let approved = |args: &[OsString]| {
        host_classification_argv(args).is_ok_and(|classified| {
            classify_rustc_invocation_v2(&classified).is_ok_and(|invocation| {
                matches!(invocation, RustcInvocationV2::Query(_))
                    && invocation.is_bootstrap_passthrough_approved()
            })
        })
    };
    assert!(approved(&query));
    for index in 5..8 {
        let mut changed = query.clone();
        changed[index].push("changed");
        assert!(!approved(&changed));
        let mut missing = query.clone();
        missing.remove(index);
        assert!(!approved(&missing));
        let mut duplicate = query.clone();
        duplicate.insert(index, query[index].clone());
        assert!(!approved(&duplicate));
    }
    for extra in [
        "--emit=link",
        "--print=link-args",
        "--extern=hostile=/tmp/lib.so",
        "--sysroot=/tmp",
        "-Zcodegen-backend=/tmp/lib.so",
        "@response",
        "--",
    ] {
        let mut changed = query.clone();
        changed.push(extra.into());
        assert!(!approved(&changed), "{extra}");
    }
    for extra in [
        vec!["-Ctarget-feature=+avx2"],
        vec!["-Ctarget-cpu=native"],
        vec!["-C", "target-feature=+crt-static"],
        vec!["--codegen", "target-feature=+crt-static"],
        vec!["-Zalways-encode-mir"],
    ] {
        let mut changed = query.clone();
        changed.extend(extra.into_iter().map(OsString::from));
        assert!(!approved(&changed));
    }
    let mut source = query.clone();
    source[1] = "src/lib.rs".into();
    assert!(!approved(&source));
    let mut ordinary = argv(HOST);
    ordinary.extend(query[5..8].iter().cloned());
    let classified = host_classification_argv(&ordinary).unwrap();
    assert!(matches!(classified, Cow::Borrowed(_)));
    assert_eq!(&*classified, ordinary);
}

#[test]
fn unrelated_units_are_unbound_and_selected_source_wrong_roles_reject() {
    let (_directory, projection) = fixture();
    for source in ["src/main.rs", "build.rs", "/unrelated/dependency.rs"] {
        let mut args = argv(HOST);
        args[3] = source.into();
        assert_eq!(binding(&projection, &args).unwrap(), None);
    }
    for extra in [
        "--test",
        "-Zunstable-options",
        "--cfg=fe2o3_codegen_generation=\"x\"",
        "-Cunknown-option=1",
    ] {
        let mut args = argv(HOST);
        args.push(extra.into());
        assert!(binding(&projection, &args).is_err(), "{extra}");
    }
    let mut wrong_crate = argv(HOST);
    wrong_crate[2] = "other".into();
    assert!(binding(&projection, &wrong_crate).is_err());
    let mut wrong_role = argv(HOST);
    wrong_role[4] = "--crate-type=bin".into();
    assert!(binding(&projection, &wrong_role).is_err());
    assert!(binding(&projection, &argv("aarch64-unknown-linux-gnu")).is_err());
}

#[test]
fn package_edition_features_and_every_profile_coordinate_reject_mismatch() {
    let (_directory, projection) = fixture();
    let args = argv(HOST);
    let root = &projection.source.workspace_root;
    assert!(
        projection
            .binding_for(
                compile(&args),
                root,
                Some(OsStr::new("wrong")),
                Some(root.as_os_str())
            )
            .is_err()
    );
    assert!(
        projection
            .binding_for(
                compile(&args),
                root,
                Some(OsStr::new("selected-package")),
                Some(OsStr::new("/wrong"))
            )
            .is_err()
    );
    let mut edition = args.clone();
    edition[5] = "--edition=2021".into();
    assert!(binding(&projection, &edition).is_err());
    let mut cfg = args.clone();
    *cfg.last_mut().unwrap() = "feature=\"different\"".into();
    assert!(binding(&projection, &cfg).is_err());
    for option in [
        "opt-level=3",
        "debuginfo=2",
        "debug-assertions=off",
        "overflow-checks=off",
        "panic=unwind",
        "lto=yes",
        "codegen-units=1",
        "strip=none",
        "embed-bitcode=yes",
        "incremental=/cache",
    ] {
        let mut changed = args.clone();
        changed.push(format!("-C{option}").into());
        assert!(binding(&projection, &changed).is_err(), "{option}");
    }
}

#[test]
fn split_joined_options_and_cfg_order_have_the_same_unit_key() {
    let joined = argv(HOST);
    let split = [
        "/proc/self/fd/194",
        "--crate-name=selected",
        "src/lib.rs",
        "--crate-type",
        "lib",
        "--edition",
        "2024",
        "--target=x86_64-unknown-linux-gnu",
        "--codegen=opt-level=1",
        "-Cpanic=abort",
        "--cfg=feature=\"first\"",
        "--cfg=feature=\"first\"",
    ]
    .map(OsString::from);
    assert_eq!(
        library_configuration(compile(&joined), HOST).unwrap(),
        library_configuration(compile(&split), HOST).unwrap()
    );
}

#[test]
fn bare_codegen_flags_and_short_profile_flags_cannot_escape_unit_comparison() {
    let (_directory, projection) = fixture();
    for option in ["overflow-checks", "debug-assertions", "lto"] {
        let mut joined = argv(HOST);
        joined.push(format!("-C{option}").into());
        let mut split = argv(HOST);
        split.extend([OsString::from("-C"), OsString::from(option)]);
        assert_eq!(
            library_configuration(compile(&joined), HOST).unwrap(),
            library_configuration(compile(&split), HOST).unwrap()
        );
        assert!(binding(&projection, &joined).is_err());
    }
    for option in ["-O", "-g"] {
        let mut changed = argv(HOST);
        changed.push(option.into());
        assert!(binding(&projection, &changed).is_err());
    }
}

#[test]
fn sysroot_native_linking_and_path_remapping_require_an_explicit_profile() {
    let (_directory, projection) = fixture();
    for options in [
        vec!["--sysroot", "/other"],
        vec!["--sysroot=/other"],
        vec!["--remap-path-prefix", "/original=/other"],
        vec!["--remap-path-prefix=/original=/other"],
        vec!["-l", "other"],
        vec!["--unknown-option"],
    ] {
        let mut changed = argv(HOST);
        changed.extend(options.into_iter().map(OsString::from));
        assert!(binding(&projection, &changed).is_err());
    }
}

#[test]
fn exact_device_suffix_preserves_cargo_profile_and_rejects_every_changed_member() {
    let profile = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942;
    let backend = PathBuf::from(format!("/proc/./self/fd/{}", crate::BACKEND_CHILD_FD));
    let managed = crate::generation::managed_rustc_args(&backend, [7; 16]).unwrap();
    let mut device = device_argv(profile.rustc_target());
    let prefix_len = device.len();
    device.extend(
        profile
            .cargo_rustflags()
            .split_ascii_whitespace()
            .map(OsString::from),
    );
    device.extend(crate::binding_wrapper::decode_managed_rustc_args(&managed).unwrap());
    let expected = library_configuration(compile(&argv(HOST)), HOST).unwrap();
    assert_eq!(
        committed_device_configuration(&device, profile, &managed).unwrap(),
        expected
    );
    for index in prefix_len..device.len() {
        let mut changed = device.clone();
        changed[index].push("changed");
        assert!(
            committed_device_configuration(&changed, profile, &managed).is_err(),
            "changed {index}"
        );
        let mut missing = device.clone();
        missing.remove(index);
        assert!(
            committed_device_configuration(&missing, profile, &managed).is_err(),
            "missing {index}"
        );
        let mut duplicated = device.clone();
        duplicated.insert(index, device[index].clone());
        assert!(
            committed_device_configuration(&duplicated, profile, &managed).is_err(),
            "duplicate {index}"
        );
        if index + 1 < device.len() {
            let mut reordered = device.clone();
            reordered.swap(index, index + 1);
            assert!(
                committed_device_configuration(&reordered, profile, &managed).is_err(),
                "reordered {index}"
            );
        }
    }
    let wrong_generation = crate::generation::managed_rustc_args(&backend, [8; 16]).unwrap();
    assert!(committed_device_configuration(&device, profile, &wrong_generation).is_err());
    let mut duplicate = device.clone();
    duplicate.insert(1, "--cfg=fe2o3_codegen_generation=\"bad\"".into());
    assert!(committed_device_configuration(&duplicate, profile, &managed).is_err());
    let gfx950 = fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950;
    let mut other = device_argv(gfx950.rustc_target());
    other.extend(
        gfx950
            .cargo_rustflags()
            .split_ascii_whitespace()
            .map(OsString::from),
    );
    other.extend(crate::binding_wrapper::decode_managed_rustc_args(&managed).unwrap());
    assert_eq!(
        committed_device_configuration(&other, gfx950, &managed).unwrap(),
        expected
    );
    assert!(committed_device_configuration(&other, profile, &managed).is_err());
}

#[test]
fn retained_source_rejects_in_place_write_and_path_substitution() {
    for replace in [false, true] {
        let (_directory, projection) = fixture();
        let retained = RetainedSource::open(&projection.source).unwrap();
        retained.revalidate().unwrap();
        let source = &projection.source.targets[0].source_path;
        if replace {
            std::fs::rename(source, source.with_extension("old")).unwrap();
        }
        std::fs::write(source, "pub fn altered() {}\n").unwrap();
        assert!(retained.revalidate().is_err());
        assert!(RetainedSource::open(&projection.source).is_err());
    }
}

#[test]
fn device_build_std_gate_requires_exact_imports_and_never_applies_to_host() {
    let device = device_argv("amdgcn-amd-amdhsa");
    let imports = DEVICE_SYSROOT.map(OsString::from);
    let stripped = without_device_build_std_gate(&device).unwrap();
    assert_eq!(
        library_configuration(compile(stripped), "amdgcn-amd-amdhsa").unwrap(),
        library_configuration(compile(&argv(HOST)), HOST).unwrap(),
    );
    let mut host = argv(HOST);
    host.extend(imports.clone());
    assert!(library_configuration(compile(&host), HOST).is_err());
    for index in [1, 3] {
        let mut wrong = device.clone();
        let position = wrong.len() - imports.len() + index;
        wrong[position] = "other=/target/libother.rlib".into();
        assert!(without_device_build_std_gate(&wrong).is_err());
    }
    let mut duplicate = device.clone();
    duplicate.splice(1..1, imports[..2].iter().cloned());
    assert!(without_device_build_std_gate(&duplicate).is_err());
    let mut without_imports = argv("amdgcn-amd-amdhsa");
    without_imports.extend(imports[4..].iter().cloned());
    assert!(without_device_build_std_gate(&without_imports).is_err());
    assert!(without_device_build_std_gate(&argv("amdgcn-amd-amdhsa")).is_err());
    for extra in [
        vec!["--extern", "noprelude:alloc=/target/liballoc.rlib"],
        vec!["--extern", "noprelude:core=/target/libcore.rlib"],
        vec!["--extern", "nounused,noprelude:core=/target/libcore.rlib"],
        vec!["--extern", "priv,noprelude:core=/target/libcore.rlib"],
        vec!["--extern=noprelude:core=/target/libcore.rlib"],
    ] {
        let mut unexpected = device.clone();
        unexpected.splice(1..1, extra.into_iter().map(OsString::from));
        assert!(without_device_build_std_gate(&unexpected).is_err());
    }
    let mut wrong_gate = device;
    *wrong_gate.last_mut().unwrap() = "force-unstable-if-unmarked".into();
    assert!(without_device_build_std_gate(&wrong_gate).is_err());
}

#[test]
fn host_codec_rejects_truncation_extra_fields_cross_domain_and_multi_source() {
    let (_directory, projection) = fixture();
    let encoded = projection.encode().unwrap();
    for size in 0..encoded.len() {
        assert!(HostProjection::decode(&encoded[..size]).is_err());
    }
    assert!(HostProjection::decode(&projection.source.validate_and_encode().unwrap()).is_err());
    let mut trailing = encoded.clone();
    trailing.push(b' ');
    assert!(HostProjection::decode(&trailing).is_err());
    let mut json: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    json["extra"] = true.into();
    assert!(HostProjection::decode(&serde_json::to_vec(&json).unwrap()).is_err());
    let mut other = projection.clone();
    other.format = "fe2o3-binding-check-target-projection-v1".into();
    assert!(other.encode().is_err());
    let mut duplicate = projection.clone();
    duplicate
        .source
        .targets
        .push(projection.source.targets[0].clone());
    assert!(duplicate.encode().is_err());
    let mut bad_identity = projection;
    bad_identity.source.targets[0].source_identity.inode = 0;
    assert!(bad_identity.encode().is_err());
}

#[test]
#[ignore = "subprocess-only inherited descriptor consumer"]
fn inherited_host_projection_child() {
    let bytes = crate::binding_check_projection::consume_inherited_host_bytes().unwrap();
    let projection = HostProjection::decode(&bytes).unwrap();
    assert_eq!(projection.crate_name, "selected");
}

fn consumer(sealed: Option<&SealedProjection>) -> Command {
    let mut child = Command::new(std::env::current_exe().unwrap());
    child.args([
        "--exact",
        "production_host_binding::tests::inherited_host_projection_child",
        "--ignored",
        "--nocapture",
    ]);
    if let Some(sealed) = sealed {
        sealed
            .inherit_for_child_at(&mut child, crate::CARGO_BINDING_CHECK_PROJECTION_CHILD_FD)
            .unwrap();
    }
    child
}

#[test]
fn sealed_projection_supports_independent_parallel_positional_consumers() {
    let (_directory, projection) = fixture();
    let sealed = SealedProjection::for_production_host(&projection.encode().unwrap()).unwrap();
    let mut first = consumer(Some(&sealed))
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    let mut second = consumer(Some(&sealed))
        .stdout(Stdio::null())
        .spawn()
        .unwrap();
    assert!(first.wait().unwrap().success());
    assert!(second.wait().unwrap().success());
}

#[test]
fn inherited_transport_rejects_wrong_domain_missing_and_malformed_descriptors() {
    let (_directory, projection) = fixture();
    let check = SealedProjection::new(&projection.source).unwrap();
    let malformed = SealedProjection::for_production_host(b"{}").unwrap();
    for transport in [None, Some(&check), Some(&malformed)] {
        let output = consumer(transport).output().unwrap();
        assert!(!output.status.success());
    }
}

#[test]
fn inherited_transport_rejects_unsealed_host_memfd() {
    use std::io::Write;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;
    let (_directory, projection) = fixture();
    let mut file = File::from(
        rustix::fs::memfd_create(
            crate::binding_check_projection::HOST_MEMFD_NAME,
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap(),
    );
    file.write_all(&projection.encode().unwrap()).unwrap();
    let mut child = consumer(None);
    // SAFETY: only this test child's reserved slot is replaced; the retained file
    // remains live through the async-signal-safe dup2 and immediately following exec.
    unsafe {
        child.pre_exec(move || {
            if libc::dup2(
                file.as_raw_fd(),
                crate::CARGO_BINDING_CHECK_PROJECTION_CHILD_FD,
            ) < 0
            {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    assert!(!child.output().unwrap().status.success());
}

#[test]
#[ignore = "requires built production wrapper and real pinned Cargo/rustc; no authority or GPU"]
fn real_cargo_warm_cache_rechecks_original_binding_and_keeps_binary_unbound() {
    let (_directory, mut projection) = fixture();
    let root = &projection.source.workspace_root;
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "selected-package"
version = "0.1.0"
edition = "2024"
[workspace]
members = ["tracked-macros"]
[lib]
name = "selected"
[[bin]]
name = "consumer"
path = "src/main.rs"
[dependencies]
tracked-macros = { path = "tracked-macros" }
[profile.dev]
opt-level = 1
debug = 0
panic = "abort"
"#,
    )
    .unwrap();
    std::fs::create_dir_all(root.join("tracked-macros/src")).unwrap();
    std::fs::write(
        root.join("tracked-macros/Cargo.toml"),
        r#"[package]
name = "tracked-macros"
version = "0.1.0"
edition = "2024"
[lib]
proc-macro = true
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("tracked-macros/src/lib.rs"),
        r#"#![feature(proc_macro_tracked_env)]
extern crate proc_macro;
#[proc_macro]
pub fn emit(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let binding = proc_macro::tracked::env_var("FE2O3_CRATE_BINDING_ID_V1").unwrap();
    format!("pub const BINDING: &str = {binding:?};").parse().unwrap()
}
"#,
    )
    .unwrap();
    std::fs::write(root.join("src/lib.rs"), "tracked_macros::emit!();\n").unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    assert!(option_env!("FE2O3_CRATE_BINDING_ID_V1").is_none());
    println!("{}", selected::BINDING);
}
"#,
    )
    .unwrap();
    let file = File::open(root.join("src/lib.rs")).unwrap();
    projection.source.targets[0].source_identity =
        ObjectIdentity::from_stat(&rustix::fs::fstat(&file).unwrap()).unwrap();
    projection.unit.cfgs.clear();
    for (key, value) in [
        ("embed-bitcode", "no"),
        ("debug-assertions", "on"),
        ("strip", "debuginfo"),
    ] {
        projection
            .unit
            .profile
            .insert(key.to_owned(), value.to_owned());
    }
    let tools = PathBuf::from(
        std::env::var_os("FE2O3_HOST_BINDING_TEST_TOOLCHAIN").expect("toolchain bin directory"),
    );
    let wrapper_path = PathBuf::from(
        std::env::var_os("FE2O3_HOST_BINDING_TEST_WRAPPER").expect("built cargo-fe2o3"),
    );
    let wrapper = crate::pinned_executable::PinnedExecutable::open(&wrapper_path)
        .unwrap()
        .seal_executable_image()
        .unwrap();
    let rustc_path = tools.join("rustc");
    let rustc = crate::PinnedRustc {
        executable: crate::pinned_executable::PinnedExecutable::open(&rustc_path).unwrap(),
        lib_tree: crate::RustcLibTree::Ordinary(
            crate::rustc_lib_tree_directory(&rustc_path).unwrap(),
        ),
    };
    let first = projection.original_binding.clone();
    let second = derive_crate_binding_id_v1("selected", ["changed-device-salt"]).to_hex();
    let target_dir = root.join("target");
    let mut selected_package = None;
    for (index, binding) in [
        first.as_str(),
        first.as_str(),
        second.as_str(),
        second.as_str(),
    ]
    .into_iter()
    .enumerate()
    {
        projection.original_binding = binding.to_owned();
        let sealed = SealedProjection::for_production_host(&projection.encode().unwrap()).unwrap();
        let mut command = Command::new(tools.join("cargo"));
        command
            .env_clear()
            .env("PATH", "/usr/bin")
            .env("HOME", root)
            .env("CARGO_HOME", root.join("cargo-home"))
            .env("CARGO_INCREMENTAL", "0")
            .env("RUSTC_WRAPPER", "")
            .env("CARGO_BUILD_RUSTC_WRAPPER", "")
            .env(
                "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
                "-Ctarget-feature=+crt-static -Crelocation-model=static -Clink-arg=-no-pie",
            )
            .env(
                "LD_LIBRARY_PATH",
                format!("/proc/self/fd/{}", crate::RUSTC_LIBRARY_CHILD_FD),
            )
            .current_dir(root)
            .args([
                "build",
                "--offline",
                "--target",
                HOST,
                "--message-format=json-render-diagnostics",
            ])
            .arg("--target-dir")
            .arg(&target_dir);
        crate::configure_pinned_rustc_child(&mut command, &rustc).unwrap();
        configure_host_wrapper(&mut command, &wrapper, &sealed).unwrap();
        let output = command.output().unwrap();
        drop(command);
        assert!(
            output.status.success(),
            "run {index}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let artifacts = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .filter(|value| value["reason"] == "compiler-artifact")
            .collect::<Vec<_>>();
        let selected = artifacts
            .iter()
            .filter(|value| {
                value["target"]["name"] == "selected"
                    && value["target"]["kind"] == serde_json::json!(["lib"])
                    && value["target"]["src_path"] == root.join("src/lib.rs").to_str().unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0]["fresh"], false,
            "selected library must re-enter the wrapper on run {index}"
        );
        let package = selected[0]["package_id"].as_str().unwrap().to_owned();
        if let Some(expected) = &selected_package {
            assert_eq!(&package, expected);
        } else {
            selected_package = Some(package);
        }
        if index != 0 {
            assert!(
                artifacts
                    .iter()
                    .any(|value| value["target"]["name"] == "tracked_macros"
                        && value["fresh"] == true),
                "warm dependency control"
            );
        }
        let deps = target_dir.join(HOST).join("debug/deps");
        let dep_info = std::fs::read_dir(&deps)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("selected-")
                    && path.extension() == Some(OsStr::new("d"))
            })
            .collect::<Vec<_>>();
        assert_eq!(dep_info.len(), 1);
        let dep_info = std::fs::read_to_string(&dep_info[0]).unwrap();
        assert!(
            dep_info
                .lines()
                .any(|line| line == format!("# env-dep:{CRATE_BINDING_ID_ENV_V1}={binding}"))
        );
        let consumer = Command::new(target_dir.join(HOST).join("debug/consumer"))
            .env_remove(CRATE_BINDING_ID_ENV_V1)
            .output()
            .unwrap();
        assert!(consumer.status.success());
        assert_eq!(String::from_utf8(consumer.stdout).unwrap().trim(), binding);
        println!(
            "host binding cache run={index} selected_fresh=false binding={binding} binary_unbound=true"
        );
    }
}

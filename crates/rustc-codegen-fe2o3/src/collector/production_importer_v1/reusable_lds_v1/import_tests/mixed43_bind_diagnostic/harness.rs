use super::*;
use fe2o3_rustc_invocation::{
    CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, PortablePackageIdentityV1, RustcInvocationV2,
    classify_rustc_invocation_v2, derive_cargo_metadata_build_observation_v2,
    portable_rustc_metadata_v1,
};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::io::Read;
use std::path::Path;

const CHILD_KEY: &str = "FE2O3_MIXED43_BIND_DIAGNOSTIC_CHILD";
const OUTPUT_DIR: &str = "FE2O3_MIXED43_BIND_DIAGNOSTIC_OUT";
const RECEIPT: &str = "diagnostic-completed.txt";
const PACKAGE: &str = "fe2o3-gfx950-advanced-systems";
const CRATE: &str = "fe2o3_gfx950_advanced_systems";
const MANIFEST_HASH: &str = "0d4f6b3309f7d7cefbd57d6088a456e76890904865c1d383f06eaef7a89f6097";

pub(super) fn hex32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64);
    assert!(value.is_ascii());
    std::array::from_fn(|i| u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).unwrap())
}

// Stream with a fixed scratch buffer; reject before reading more than the cap.
fn digest(path: &Path, maximum: u64) -> [u8; 32] {
    let mut file = std::fs::File::open(path).unwrap();
    let size = file.metadata().unwrap().len();
    assert!(
        size <= maximum,
        "bounded diagnostic input: {}",
        path.display()
    );
    let mut digest = Sha256::new();
    let mut seen = 0u64;
    let mut buffer = [0u8; 8192];
    loop {
        let read = file.read(&mut buffer).unwrap();
        if read == 0 {
            break;
        }
        seen = seen.checked_add(read as u64).unwrap();
        assert!(seen <= maximum);
        digest.update(&buffer[..read]);
    }
    assert_eq!(size, seen, "source length changed during observation");
    digest.finalize().into()
}

fn pin_inputs(snapshot: &Path) -> PathBuf {
    let package = snapshot.join("examples/gfx950_advanced_systems");
    for (relative, expected) in [
        ("Cargo.toml", MANIFEST_HASH),
        (
            "src/lib.rs",
            "0d3352c7bc338558618f55cebabcb388b2207aae4a212fcb0d6210db5fd480c5",
        ),
        (
            "src/kernel.rs",
            "35eba556fca8c3639ab2e98feaac051c9102161143c910f2530847a196198ac4",
        ),
        (
            "src/effect_reference.rs",
            "1168cb350477e0a3cdeb1365f3280ee7c6fbf8e0e03b2a2222cd19c97d48a33b",
        ),
    ] {
        assert_eq!(
            digest(&package.join(relative), 2 * 1024 * 1024),
            hex32(expected),
            "unchanged mixed43 source required: {relative}"
        );
    }
    assert_eq!(
        digest(
            &snapshot.join("crates/fe2o3-device/src/execution.rs"),
            2 * 1024 * 1024
        ),
        hex32("d5f5f01fa8f24feda48626bd4368de4989b76f557a6cf3d2f297cab8aa5497ff")
    );
    package
}

fn identity_args(case: Case, input: &Path) -> Vec<String> {
    let mut args = vec![
        "rustc".into(),
        format!("--crate-name={CRATE}"),
        "--edition=2024".into(),
        input.display().to_string(),
        "--crate-type=lib".into(),
        "-Cembed-bitcode=no".into(),
    ];
    for feature in case.features {
        args.extend(["--cfg".into(), format!("feature=\"{feature}\"")]);
    }
    args.extend([
        format!("-Cmetadata={}", case.cargo_metadata),
        "--target=amdgcn-amd-amdhsa".into(),
        "-Cstrip=debuginfo".into(),
        "-Zunstable-options".into(),
        "-Zalways-encode-mir".into(),
        "-Zinline-mir=no".into(),
        "-Zmir-enable-passes=-JumpThreading".into(),
        "-Copt-level=0".into(),
        "-Ctarget-cpu=gfx950".into(),
        "-Ctarget-feature=-wavefrontsize32,+wavefrontsize64,-xnack".into(),
        "-Cdebuginfo=2".into(),
    ]);
    args
}

fn portable(args: &[String]) -> String {
    let argv = args.iter().map(OsString::from).collect::<Vec<_>>();
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap() else {
        panic!("selected original rustc invocation");
    };
    portable_rustc_metadata_v1(
        compile,
        &PortablePackageIdentityV1::new(PACKAGE, "0.1.0", hex32(MANIFEST_HASH)).unwrap(),
    )
    .unwrap()
}

pub(super) fn run(case: Case) {
    let snapshot = configured("FE2O3_MIXED43_BIND_SNAPSHOT");
    let package = pin_inputs(&snapshot);
    let input = package.join("src/lib.rs");
    let device = configured("FE2O3_CORE_TRY_DEVICE_RMETA");
    let host = configured("FE2O3_CORE_TRY_HOST_DEPS");
    let core = configured("FE2O3_CORE_TRY_AMDGPU_CORE");
    let builtins = configured("FE2O3_CORE_TRY_AMDGPU_BUILTINS");
    let sysroot = configured("FE2O3_MIXED43_BIND_SYSROOT");
    let args = identity_args(case, &input);
    let metadata = portable(&args);
    let observation = derive_cargo_metadata_build_observation_v2(&[case.cargo_metadata]).to_hex();
    let binding =
        reserved_fe2o3_symbols::derive_crate_binding_id_v1(CRATE, [metadata.as_str()]).to_hex();
    let key = format!("{}:{binding}:{observation}", case.name);
    if std::env::var(CHILD_KEY).ok().as_deref() != Some(key.as_str()) {
        let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-mixed43-bind-diagnostic");
        let test = format!(
            "collector::production_importer_v1::reusable_lds_v1::import_tests::mixed43_bind_diagnostic::{}",
            case.name
        );
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                &test,
                "--ignored",
                "--nocapture",
                "--test-threads=1",
            ])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .env(CHILD_KEY, &key)
            .env(OUTPUT_DIR, scratch.path())
            .env("CARGO_MANIFEST_DIR", &package)
            .env("CARGO_PKG_NAME", PACKAGE)
            .env("CARGO_PKG_VERSION", "0.1.0")
            .env(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2, &observation)
            .env(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1, &binding)
            .env("FE2O3_MIXED43_BIND_SNAPSHOT", &snapshot)
            .env("FE2O3_MIXED43_BIND_SYSROOT", &sysroot)
            .env("FE2O3_CORE_TRY_DEVICE_RMETA", &device)
            .env("FE2O3_CORE_TRY_HOST_DEPS", &host)
            .env("FE2O3_CORE_TRY_AMDGPU_CORE", &core)
            .env("FE2O3_CORE_TRY_AMDGPU_BUILTINS", &builtins)
            .env("FE2O3_SIMULATION_MODE_V1", "1")
            .env(
                "FE2O3_SIMULATION_ATTEMPT_V1",
                "45454545454545454545454545454545",
            )
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
        // Keep the existing artifact process guard. Stream test output instead
        // of adding an unbounded wait_with_output buffer to this diagnostic.
        let status = fe2o3_artifact_transaction::with_artifact_process_spawn_v1(|| command.spawn())
            .unwrap()
            .wait()
            .unwrap();
        assert!(
            status.success(),
            "exact mixed43 diagnostic child must complete"
        );
        let mut receipt = String::new();
        std::fs::File::open(scratch.path().join(RECEIPT))
            .unwrap()
            .take(257)
            .read_to_string(&mut receipt)
            .unwrap();
        assert_eq!(
            receipt,
            format!("{key}\nnominal-join-only\n"),
            "zero selected tests is not completion"
        );
        pin_inputs(&snapshot);
        return;
    }
    assert_eq!(
        std::env::var(CARGO_METADATA_BUILD_OBSERVATION_ENV_V2).unwrap(),
        observation
    );
    assert_eq!(
        std::env::var(reserved_fe2o3_symbols::CRATE_BINDING_ID_ENV_V1).unwrap(),
        binding
    );
    let mut metadata_digests = Vec::with_capacity(3);
    for path in [&device, &core, &builtins] {
        let hash = digest(path, 128 * 1024 * 1024);
        eprintln!(
            "DIAGNOSTIC_INPUT path={} sha256={hash:02x?}",
            path.display()
        );
        metadata_digests.push(hash);
    }
    let mut args = args;
    let old = format!("-Cmetadata={}", case.cargo_metadata);
    let metadata_slot = args.iter_mut().find(|arg| **arg == old).unwrap();
    *metadata_slot = format!("-Cmetadata={metadata}");
    args.extend([
        "--sysroot".into(),
        sysroot.display().to_string(),
        format!("--out-dir={}", configured(OUTPUT_DIR).display()),
        "-Zno-codegen".into(),
        "--extern".into(),
        format!("fe2o3_device={}", device.display()),
        "-L".into(),
        format!("dependency={}", device.parent().unwrap().display()),
        "-L".into(),
        format!("dependency={}", host.display()),
        "--extern".into(),
        format!("noprelude,nounused:core={}", core.display()),
        "--extern".into(),
        format!(
            "noprelude,nounused:compiler_builtins={}",
            builtins.display()
        ),
    ]);
    assert_eq!(
        portable(&args),
        metadata,
        "artifact-only options cannot change the selected crate identity"
    );
    eprintln!(
        "DIAGNOSTIC_INPUT portable_metadata={metadata} crate_binding={binding} build_observation={observation}"
    );
    let mut probe = BindProbe {
        case,
        completed: false,
    };
    rustc_driver::run_compiler(&args, &mut probe);
    assert!(
        probe.completed,
        "exact source import and nominal join must complete"
    );
    pin_inputs(&snapshot);
    for (index, path) in [&device, &core, &builtins].iter().enumerate() {
        assert_eq!(
            digest(path, 128 * 1024 * 1024),
            metadata_digests[index],
            "immutable metadata changed"
        );
    }
    let receipt = format!("{key}\nnominal-join-only\n");
    assert!(receipt.len() <= 256);
    std::io::Write::write_all(
        &mut std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(configured(OUTPUT_DIR).join(RECEIPT))
            .unwrap(),
        receipt.as_bytes(),
    )
    .unwrap();
}

#[test]
fn original_metadata_salt_is_not_the_selected_crate_binding() {
    for case in [MUON, BROADCAST] {
        let mut args = identity_args(case, Path::new("/diagnostic/src/lib.rs"));
        let metadata = portable(&args);
        assert_ne!(metadata, case.cargo_metadata);
        let position = args
            .iter()
            .position(|v| v.starts_with("-Cmetadata="))
            .unwrap();
        args[position] = "-Cmetadata=other-cargo-build".into();
        assert_eq!(portable(&args), metadata);
        args.push("-Copt-level=2".into());
        assert_ne!(portable(&args), metadata);
    }
}

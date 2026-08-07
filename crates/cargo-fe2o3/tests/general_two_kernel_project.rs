use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::PathBuf;
use std::process::{self, Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

struct ExternalProject {
    fixture: PathBuf,
    temporary: PathBuf,
    target: PathBuf,
    backend: PathBuf,
    log: PathBuf,
}

impl ExternalProject {
    fn new() -> Self {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/general-two-kernel-project");
        assert!(fixture.join("Cargo.toml").is_file());
        assert!(fixture.join("src/main.rs").is_file());

        let temporary = temp_root();
        let target = temporary.join("target");
        let backend = temporary.join("librustc_codegen_fe2o3.so");
        let log = temporary.join("cargo.log");
        fs::write(&backend, b"general-two-kernel-test-backend")
            .expect("write deterministic backend fixture");

        Self {
            fixture,
            temporary,
            target,
            backend,
            log,
        }
    }

    fn manifest(&self) -> PathBuf {
        self.fixture.join("Cargo.toml")
    }

    fn command(&self, args: &[OsString]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_cargo-fe2o3"));
        command
            .args(args)
            .current_dir(self.fixture.join("src"))
            .env("CARGO", env!("CARGO_BIN_EXE_cargo-fe2o3-cargo-fixture"))
            .env("FE2O3_BACKEND", &self.backend)
            .env("FE2O3_TARGET", "gfx942")
            .env("FE2O3_TEST_CARGO_LOG", &self.log)
            .env("FE2O3_TEST_WORKSPACE_ROOT", &self.fixture)
            .env("FE2O3_TEST_TARGET_DIRECTORY", &self.target)
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS");
        command
    }

    fn run(&self, args: &[OsString]) -> Output {
        self.command(args)
            .output()
            .expect("run cargo-fe2o3 against two-kernel project")
    }

    fn invocations(&self) -> Vec<Invocation> {
        Invocation::decode_all(&fs::read(&self.log).expect("read fake Cargo log"))
    }
}

impl Drop for ExternalProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temporary);
    }
}

#[derive(Debug)]
struct Invocation {
    cwd: Vec<u8>,
    args: Vec<Vec<u8>>,
    cargo_target_dir: Vec<u8>,
    hsaco_dir: Vec<u8>,
    target: Vec<u8>,
    managed_rustc_args: Vec<u8>,
}

impl Invocation {
    fn decode_all(mut bytes: &[u8]) -> Vec<Self> {
        let mut records = Vec::new();
        while !bytes.is_empty() {
            let cwd = take_field(&mut bytes);
            let count = take_u64(&mut bytes) as usize;
            let args = (0..count).map(|_| take_field(&mut bytes)).collect();
            let cargo_target_dir = take_field(&mut bytes);
            let hsaco_dir = take_field(&mut bytes);
            let target = take_field(&mut bytes);
            let _wrapper = take_field(&mut bytes);
            let _rustflags = take_field(&mut bytes);
            let _encoded_rustflags = take_field(&mut bytes);
            let managed_rustc_args = take_field(&mut bytes);
            records.push(Self {
                cwd,
                args,
                cargo_target_dir,
                hsaco_dir,
                target,
                managed_rustc_args,
            });
        }
        records
    }
}

fn temp_root() -> PathBuf {
    loop {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = env::temp_dir().join(format!(
            "cargo-fe2o3-general-two-kernel-{}-{id}",
            process::id()
        ));
        match fs::create_dir(&path) {
            Ok(()) => return path,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => panic!("failed to create temporary test root: {error}"),
        }
    }
}

fn take_u64(bytes: &mut &[u8]) -> u64 {
    let raw: [u8; 8] = bytes[..8].try_into().expect("u64 field");
    *bytes = &bytes[8..];
    u64::from_le_bytes(raw)
}

fn take_field(bytes: &mut &[u8]) -> Vec<u8> {
    let length = take_u64(bytes) as usize;
    let field = bytes[..length].to_vec();
    *bytes = &bytes[length..];
    field
}

fn encoded(value: &OsStr) -> Vec<u8> {
    value.as_encoded_bytes().to_vec()
}

fn encoded_args(values: &[OsString]) -> Vec<Vec<u8>> {
    values.iter().map(|value| encoded(value)).collect()
}

fn without_injected_runner(arguments: &[Vec<u8>]) -> Vec<Vec<u8>> {
    let mut filtered = Vec::new();
    let mut index = 0;
    while index < arguments.len() {
        if arguments[index] == b"--config"
            && arguments.get(index + 1).is_some_and(|value| {
                value.starts_with(b"target.") && value.windows(8).any(|part| part == b".runner=")
            })
        {
            index += 2;
            continue;
        }
        filtered.push(arguments[index].clone());
        index += 1;
    }
    filtered
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn fixture_keeps_the_general_two_kernel_frontend_boundary_explicit() {
    let fixture = ExternalProject::new();
    let source = fs::read_to_string(fixture.fixture.join("src/main.rs")).expect("read fixture");

    assert_eq!(source.matches("#[kernel]").count(), 2);
    for required in [
        "fn alternating_adjust<T>",
        "while iteration < iterations",
        "if iteration & 1 == 0",
        "alternating_adjust(input[offset]",
        "math.sqrt_f32",
        "math.sin_f32",
    ] {
        assert!(source.contains(required), "fixture is missing `{required}`");
    }
    assert_eq!(
        source.matches("alternating_adjust(input[offset]").count(),
        2
    );
}

#[test]
fn repeated_external_build_reuses_one_isolated_generation() {
    let fixture = ExternalProject::new();
    let manifest = fixture.manifest();
    let args = vec![
        OsString::from("build"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--package"),
        OsString::from("general-two-kernel-project"),
        OsString::from("--target-dir"),
        fixture.target.as_os_str().to_owned(),
        OsString::from("--release"),
    ];

    assert_success(&fixture.run(&args));
    let marker = fs::read(fixture.target.join("fe2o3/.codegen-generation-v1"))
        .expect("read first generation marker");
    assert_success(&fixture.run(&args));

    let records = fixture.invocations();
    assert_eq!(records.len(), 4, "{records:#?}");
    for record in [&records[1], &records[3]] {
        assert_eq!(record.cwd, encoded(fixture.fixture.join("src").as_os_str()));
        assert_eq!(record.args[1..], encoded_args(&args));
        assert!(record.cargo_target_dir.is_empty());
        assert!(record.hsaco_dir.is_empty());
        assert_eq!(record.target, b"gfx942");
    }
    assert_eq!(
        records[1].managed_rustc_args, records[3].managed_rustc_args,
        "an identical build must reuse its generation token"
    );
    assert_eq!(
        fs::read(fixture.target.join("fe2o3/.codegen-generation-v1"))
            .expect("read reused generation marker"),
        marker
    );
    assert_eq!(
        fs::read(fixture.target.join("fe2o3/fixture.hsaco")).expect("read generated sidecar"),
        b"fixture-sidecar"
    );
    assert!(!fixture.fixture.join("target").exists());
    assert!(!fixture.fixture.join("Cargo.lock").exists());
}

#[test]
fn external_run_preserves_selection_and_application_arguments() {
    let fixture = ExternalProject::new();
    let manifest = fixture.manifest();
    let args = vec![
        OsString::from("run"),
        OsString::from("--manifest-path"),
        manifest.as_os_str().to_owned(),
        OsString::from("--package"),
        OsString::from("general-two-kernel-project"),
        OsString::from("--target-dir"),
        fixture.target.as_os_str().to_owned(),
        OsString::from("--target"),
        OsString::from("x86_64-unknown-linux-gnu"),
        OsString::from("--"),
        OsString::from("fixture-payload"),
    ];

    assert_success(&fixture.run(&args));

    let records = fixture.invocations();
    assert_eq!(records.len(), 2, "{records:#?}");
    assert_eq!(
        without_injected_runner(&records[1].args[1..]),
        encoded_args(&args)
    );
    assert_eq!(
        fs::read(fixture.target.join("fe2o3/fixture.hsaco")).expect("read run sidecar"),
        b"fixture-sidecar"
    );
    assert!(
        fixture
            .target
            .join("fe2o3/.codegen-generation-v1")
            .is_file()
    );
    assert!(!fixture.fixture.join("target").exists());
    assert!(!fixture.fixture.join("Cargo.lock").exists());
}
